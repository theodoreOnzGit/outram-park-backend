//! **A nuclide's processed evaluation, as plain numbers** (gh:#786).
//!
//! [`Nuclide::from_tape`](crate::material::nuclide::Nuclide::from_tape) is two
//! halves: [`Nuclide::process_evaluation`](crate::material::nuclide::Nuclide::process_evaluation)
//! (RECONR, BROADR and PURR, which is nearly all the time) and
//! [`Nuclide::from_processed`](crate::material::nuclide::Nuclide::from_processed)
//! (everything else, read off the tape). [`ProcessedEvaluation`] is what
//! passes between them, and [`ProcessedEvaluation::to_f64s`] /
//! [`ProcessedEvaluation::from_f64s`] carry it across a boundary that only
//! moves numbers: a browser Web Worker's `postMessage` (the HTR-10 demo's
//! worker pool processes each nuclide once and ships it to every other
//! worker), or a file.
//!
//! The encoding is **exact**: every `f64` is stored as itself, every integer
//! (MT, LR, counts, flags) as an `f64`, exact far beyond any value here. The
//! round trip is pinned bit for bit, in cross sections and in samples, by
//! `tests/processed_evaluation_round_trip.rs`.

use njoy_outram_park_fork::MtReaction;
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use njoy_outram_park_fork::reconr::MaterialInfo;
use njoy_outram_park_fork::reconr::{ReconrResult, ReconrSection};
use njoy_outram_park_fork::NjoyError;

/// The products of the expensive half of building a nuclide from an ENDF
/// tape, for one temperature.
#[derive(Debug, Clone)]
pub struct ProcessedEvaluation {
    /// RECONR's sections, Doppler-broadened to [`Self::temp_k`] by BROADR.
    pub recon: ReconrResult,
    /// The 0 K elastic cross section up to
    /// [`DBRC_GRID_MAX_EV`](crate::material::nuclide::DBRC_GRID_MAX_EV),
    /// `(E [eV], σ [b])`, kept for DBRC.
    pub elastic_0k: Vec<(f64, f64)>,
    /// PURR's unresolved-resonance probability tables, `None` for an
    /// evaluation with no unresolved range.
    pub urr: Option<UrrProbabilityTables>,
    /// Temperature the data were broadened to \[K\].
    pub temp_k: f64,
}

/// First word of the encoding, so a vector of something else is refused.
const MAGIC: f64 = 786.0;
/// Encoding version, bumped on any layout change.
const VERSION: f64 = 1.0;

impl ProcessedEvaluation {
    /// Every field as `f64`s. Layout: `[MAGIC, VERSION, temp_k]`, the eight
    /// fields of the material header, `resonance_upper_limit` (`NaN` for
    /// none), the sections (`n`, then per section `[mt, lr, qi, n_pairs,
    /// (E, σ)...]`), the unresolved table (`-1` for none, else its row count
    /// and rows of six), the 0 K elastic (`n`, pairs), and the URR tables
    /// (`-1` for none, else their length and
    /// [`UrrProbabilityTables::to_f64s`]).
    pub fn to_f64s(&self) -> Vec<f64> {
        let r = &self.recon;
        let m = &r.material;
        let mut v = vec![MAGIC, VERSION, self.temp_k];
        v.extend([
            m.za,
            m.awr,
            f64::from(m.lrp),
            f64::from(m.lfi),
            f64::from(m.nlib),
            m.elis,
            f64::from(m.nfor),
            m.emax,
        ]);
        v.push(r.resonance_upper_limit.unwrap_or(f64::NAN));
        v.push(r.sections.len() as f64);
        for s in &r.sections {
            v.extend([
                f64::from(s.mt.number()),
                f64::from(s.lr),
                s.qi,
                s.pairs.len() as f64,
            ]);
            for &(e, x) in &s.pairs {
                v.extend([e, x]);
            }
        }
        match &r.unresolved_table {
            None => v.push(-1.0),
            Some(rows) => {
                v.push(rows.len() as f64);
                for row in rows {
                    v.extend_from_slice(row);
                }
            }
        }
        v.push(self.elastic_0k.len() as f64);
        for &(e, x) in &self.elastic_0k {
            v.extend([e, x]);
        }
        match &self.urr {
            None => v.push(-1.0),
            Some(t) => {
                let w = t.to_f64s();
                v.push(w.len() as f64);
                v.extend(w);
            }
        }
        v
    }

    /// The inverse of [`Self::to_f64s`].
    ///
    /// # Errors
    ///
    /// [`NjoyError::EndfParse`] for a vector that is not this encoding (wrong
    /// first word or version), too short for the counts it declares, or with
    /// words left over.
    pub fn from_f64s(v: &[f64]) -> Result<Self, NjoyError> {
        let bad =
            |what: &str| NjoyError::EndfParse(format!("processed evaluation from f64s: {what}"));
        let mut at = 0usize;
        let mut take = |n: usize| -> Result<&[f64], NjoyError> {
            let s = v.get(at..at + n).ok_or_else(|| bad("too short"))?;
            at += n;
            Ok(s)
        };
        let h = take(3)?;
        if h[0] != MAGIC || h[1] != VERSION {
            return Err(bad("not a processed evaluation of this version"));
        }
        let temp_k = h[2];
        let m = take(8)?;
        let material = MaterialInfo {
            za: m[0],
            awr: m[1],
            lrp: m[2] as i32,
            lfi: m[3] as i32,
            nlib: m[4] as i32,
            elis: m[5],
            nfor: m[6] as i32,
            emax: m[7],
        };
        let rul = take(1)?[0];
        let resonance_upper_limit = (!rul.is_nan()).then_some(rul);
        let n_sec = take(1)?[0] as usize;
        let mut sections = Vec::with_capacity(n_sec);
        for _ in 0..n_sec {
            let s = take(4)?;
            let (mt, lr, qi, n) = (
                MtReaction::from_any(s[0] as i32),
                s[1] as i32,
                s[2],
                s[3] as usize,
            );
            let pairs = take(2 * n)?.chunks_exact(2).map(|p| (p[0], p[1])).collect();
            sections.push(ReconrSection { lr, mt, qi, pairs });
        }
        let n_unr = take(1)?[0];
        let unresolved_table = if n_unr < 0.0 {
            None
        } else {
            Some(
                take(6 * n_unr as usize)?
                    .chunks_exact(6)
                    .map(|r| [r[0], r[1], r[2], r[3], r[4], r[5]])
                    .collect(),
            )
        };
        let n_el = take(1)?[0] as usize;
        let elastic_0k = take(2 * n_el)?
            .chunks_exact(2)
            .map(|p| (p[0], p[1]))
            .collect();
        let n_urr = take(1)?[0];
        let urr = if n_urr < 0.0 {
            None
        } else {
            Some(UrrProbabilityTables::from_f64s(take(n_urr as usize)?)?)
        };
        if at != v.len() {
            return Err(bad("words left over"));
        }
        Ok(Self {
            recon: ReconrResult {
                material,
                sections,
                resonance_upper_limit,
                unresolved_table,
            },
            elastic_0k,
            urr,
            temp_k,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every branch of the encoding (URR tables present, an unresolved
    /// table, no resonance limit) re-encodes to the same words, and a
    /// truncated, padded or foreign vector is refused.
    #[test]
    fn the_encoding_round_trips_and_refuses_what_it_is_not() {
        // Two URR energies, two bands each.
        let urr = vec![
            1.0, 2.0e4, 1.49e5, 300.15, 2.0, 51.0, 0.0, 2.0, 2.0e4,
            1.49e5, // header, energies
            2.0, 0.4, 1.0, 1.0, 0.5, 0.1, 0.4, 1.1, 0.6, 0.1, 0.4, 0.0, 0.0, // first energy
            2.0, 0.5, 1.0, 0.9, 0.4, 0.1, 0.4, 1.2, 0.7, 0.1, 0.4, 0.0, 0.0, // second
        ];
        let p = ProcessedEvaluation {
            recon: ReconrResult {
                material: MaterialInfo {
                    za: 92238.0,
                    awr: 236.0058,
                    lrp: 1,
                    lfi: 1,
                    nlib: 0,
                    elis: 0.0,
                    nfor: 6,
                    emax: 3.0e7,
                },
                sections: vec![
                    ReconrSection {
                        lr: 0,
                        mt: MtReaction::from_any(1),
                        qi: 0.0,
                        pairs: vec![(1.0e-5, 10.0), (2.0e7, 5.0)],
                    },
                    ReconrSection {
                        lr: 22,
                        mt: MtReaction::from_any(51),
                        qi: -4.5e4,
                        pairs: vec![(4.52e4, 0.0), (2.0e7, 0.1)],
                    },
                    ReconrSection {
                        lr: 0,
                        mt: MtReaction::from_any(999),
                        qi: 1.0,
                        pairs: vec![],
                    },
                ],
                resonance_upper_limit: None,
                unresolved_table: Some(vec![[2.0e4, 1.0, 2.0, 3.0, 4.0, 5.0]]),
            },
            elastic_0k: vec![(1.0e-5, 9.0), (2.0e4, 8.0)],
            urr: Some(UrrProbabilityTables::from_f64s(&urr).expect("tables")),
            temp_k: 300.15,
        };
        let v = p.to_f64s();
        let back = ProcessedEvaluation::from_f64s(&v).expect("decode");
        let b = |w: &[f64]| w.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(b(&back.to_f64s()), b(&v));
        assert_eq!(back.recon.sections[2].mt.number(), 999);
        assert!(back.recon.resonance_upper_limit.is_none() && back.urr.is_some());
        assert!(ProcessedEvaluation::from_f64s(&v[..v.len() - 1]).is_err());
        let mut long = v.clone();
        long.push(0.0);
        assert!(ProcessedEvaluation::from_f64s(&long).is_err());
        assert!(ProcessedEvaluation::from_f64s(&[1.0, 2.0, 3.0]).is_err());
    }
}
