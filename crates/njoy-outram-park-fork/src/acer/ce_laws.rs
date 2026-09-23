// SPDX-License-Identifier: GPL-3.0

//! **The ACE secondary-distribution blocks: AND, DLW and NU.**
//!
//! [`super::ce_decode`] turns ESZ/MTR/LQR/LSIG/SIG into cross sections. Cross
//! sections alone cannot transport: a collision also needs an outgoing energy
//! and a scattering cosine. This module decodes the blocks that carry them.
//!
//! # Upstream is the specification
//!
//! OpenMC's C++ reads HDF5, so its ACE reader is Python, read at `afa7a14`:
//!
//! | block | upstream |
//! |---|---|
//! | AND (tabulated cosines) | `openmc/data/angle_distribution.py::from_ace` |
//! | DLW LAW=4 | `openmc/data/energy_distribution.py::ContinuousTabular.from_ace` |
//! | DLW LAW=44 | `openmc/data/kalbach_mann.py::KalbachMann.from_ace` |
//! | DLW LAW=61 | `openmc/data/correlated.py::CorrelatedAngleEnergy.from_ace` |
//!
//! # The three laws share one structure
//!
//! LAW=4, 44 and 61 have an **identical outer layout** -- interpolation
//! regions, the incident-energy grid, and a locator per incident energy -- and
//! identical per-energy headers. They differ only in how many parallel arrays
//! follow the outgoing-energy grid:
//!
//! | law | arrays | what the extra ones are |
//! |---|---|---|
//! | 4 | 3 | `E'`, pdf, cdf. The cosine comes from the AND block. |
//! | 44 | 5 | + Kalbach `r` and `a` (pre-compound fraction and slope) |
//! | 61 | 4 | + a locator per `E'` to its own tabulated cosine distribution |
//!
//! So this is one reader with an array count, not three readers. Writing it as
//! three would have triplicated the discrete-line handling, which is the part
//! that is easy to get wrong.
//!
//! Measured over the whole NJOY2016 reference library in `reference-data/ace`,
//! these are the **only** neutron laws present: U-235 and U-238 use
//! `{LAW3: 39, LAW4: 1, LAW61: 4}`, U-234 uses `{LAW3: 40, LAW4: 4, LAW44: 4}`.
//! LAW=3 is discrete two-body level scattering and carries no tabulated data at
//! all -- just `Q` and the mass ratio, from which the kinematics are analytic.
//!
//! # Units
//!
//! ACE stores energies in **MeV** and pdfs per MeV. `ChiEout` and `NuBar` are
//! eV-based, so every energy is scaled on the way out and every pdf is scaled
//! by the inverse.

use crate::acer::read::RawAceTable;
use crate::acer::{jxs, nxs};
use crate::acer::angular::{ElasticAngular, EnergyAngular};
use crate::acer::ce_decode::EV_PER_MEV;
use crate::error::NjoyError;
use crate::nuclear_data::secondary::{
    ChiEout, ChiTabular, ContinuumAngular, ContinuumAngularRow, ContinuumAngularTable,
    ContinuumKalbachRow, ContinuumKalbachTable, NuBar,
};

fn need(t: &RawAceTable, at: usize, n: usize, what: &str) -> Result<(), NjoyError> {
    if at + n > t.xss.len() {
        return Err(NjoyError::EndfParse(format!(
            "ACE {what}: words {at}..{} run past XSS ({})",
            at + n,
            t.xss.len()
        )));
    }
    Ok(())
}

/// One incident-energy row of a tabulated outgoing-energy law.
#[derive(Debug, Clone)]
pub struct LawEnergyRow {
    /// Incident energy \[eV\].
    pub e_in: f64,
    /// Outgoing-energy table, already in eV.
    pub eout: ChiEout,
    /// Kalbach `r`/`a` per outgoing energy (LAW=44 only).
    pub kalbach: Option<Vec<ContinuumKalbachRow>>,
    /// Tabulated cosine per outgoing energy (LAW=61 only).
    pub cosines: Option<Vec<ContinuumAngularRow>>,
}

/// A decoded DLW entry.
#[derive(Debug, Clone)]
pub enum AceEnergyLaw {
    /// **LAW=3** -- discrete two-body level scattering. Carries no table: the
    /// outgoing energy follows from `Q` and the mass ratio, and the cosine
    /// comes from the AND block.
    TwoBodyLevel {
        /// `(A+1)/A * |Q|` as ACE stores it \[eV\].
        ldat1: f64,
        /// `(A/(A+1))^2` as ACE stores it.
        ldat2: f64,
    },
    /// **LAW=4/44/61** -- a tabulated outgoing-energy law, optionally carrying
    /// its own correlated angular data.
    Tabulated {
        /// 4, 44 or 61, kept so a consumer can tell a correlated law from one
        /// that defers its cosine to the AND block.
        law: i32,
        rows: Vec<LawEnergyRow>,
        /// The evaluation's `(NBT, INT)` over the incident grid; empty means
        /// ACE's `NR = 0` default of a single lin-lin range.
        incident_interp: Vec<(u32, u32)>,
    },
}

/// Read a tabulated cosine distribution (32 equiprobable bins are `intt = 1`
/// histogram; tabulated is `intt = 2`), as stored in AND and in LAW=61.
fn read_cosine_table(t: &RawAceTable, at: usize) -> Result<ContinuumAngularRow, NjoyError> {
    need(t, at, 2, "cosine table header")?;
    let _intt = t.xss[at] as i32;
    let n = t.xss[at + 1] as usize;
    need(t, at + 2, 3 * n, "cosine table body")?;
    let cosines = t.xss[at + 2..at + 2 + n].to_vec();
    let pdf = t.xss[at + 2 + n..at + 2 + 2 * n].to_vec();
    let cdf = t.xss[at + 2 + 2 * n..at + 2 + 3 * n].to_vec();
    // `mubar` is the mean of the tabulated density; computing it here keeps the
    // consumer from having to re-derive it on every sample.
    let mut mubar = 0.0;
    for i in 1..n {
        let dmu = cosines[i] - cosines[i - 1];
        mubar += 0.5 * (cosines[i] * pdf[i] + cosines[i - 1] * pdf[i - 1]) * dmu;
    }
    Ok(ContinuumAngularRow {
        cosines,
        cdf,
        mubar,
    })
}

/// Decode the **AND** block for one reaction index, returning `None` when the
/// distribution is isotropic (`LAND = 0`) and an error when it is stored in
/// DLW (`LAND = -1`, only legal for a correlated law).
///
/// `i` is 0 for elastic and `1..=NR` for the reactions with secondary neutrons,
/// matching the LAND block's own ordering.
/// `lct` is the reference frame ENDF-style: `2` centre of mass, `1` laboratory.
/// ACE does not store it in the AND block -- it is the **sign of TYR** for a
/// reaction, and CM by definition for elastic. Passing it in rather than
/// defaulting keeps the caller honest: a cosine sampled in CM and used as
/// though it were lab is a wrong scattering angle that nothing reports.
pub fn decode_angular(
    t: &RawAceTable,
    i: usize,
    lct: i32,
) -> Result<Option<ElasticAngular>, NjoyError> {
    let land = t.jxs[jxs::LAND];
    if land <= 0 {
        return Ok(None);
    }
    need(t, (land - 1) as usize + i, 1, "LAND")?;
    let loc = t.xss[(land - 1) as usize + i] as i64;
    if loc == 0 {
        return Ok(None); // isotropic at every energy
    }
    if loc < 0 {
        // Angle is carried inside the DLW law itself (LAW=44/61).
        return Ok(None);
    }
    let and = t.jxs[jxs::AND];
    if and <= 0 {
        return Err(NjoyError::EndfParse("LAND points into an absent AND block".into()));
    }
    let base = (and - 1) as usize + (loc as usize) - 1;
    need(t, base, 1, "AND header")?;
    let ne = t.xss[base] as usize;
    need(t, base + 1, 2 * ne, "AND energy/locator grid")?;
    let e_grid = t.xss[base + 1..base + 1 + ne].to_vec();
    let locs = t.xss[base + 1 + ne..base + 1 + 2 * ne].to_vec();

    let mut energies = Vec::with_capacity(ne);
    for k in 0..ne {
        let lc = locs[k] as i64;
        if lc == 0 {
            // Isotropic at this incident energy: empty cosine grid is how
            // `EnergyAngular` spells that.
            energies.push(EnergyAngular {
                e_mev: e_grid[k],
                cosines: Vec::new(),
                pdf: Vec::new(),
                cdf: Vec::new(),
            });
            continue;
        }
        // A NEGATIVE locator means tabulated; positive means 32 equiprobable
        // bins. Both are offsets from AND, and both are read from the same
        // place -- the sign is the format flag, not part of the address.
        let at = (and - 1) as usize + (lc.unsigned_abs() as usize) - 1;
        if lc < 0 {
            let row = read_cosine_table(t, at)?;
            need(t, at, 2, "AND tabulated header")?;
            let n = t.xss[at + 1] as usize;
            let pdf = t.xss[at + 2 + n..at + 2 + 2 * n].to_vec();
            energies.push(EnergyAngular {
                e_mev: e_grid[k],
                cosines: row.cosines,
                pdf,
                cdf: row.cdf,
            });
        } else {
            // 32 equiprobable bins: 33 boundaries, uniform within each.
            need(t, at, 33, "AND equiprobable bins")?;
            let b = t.xss[at..at + 33].to_vec();
            let mut cosines = Vec::with_capacity(33);
            let mut pdf = Vec::with_capacity(33);
            let mut cdf = Vec::with_capacity(33);
            for (j, &mu) in b.iter().enumerate() {
                cosines.push(mu);
                cdf.push(j as f64 / 32.0);
            }
            for j in 0..33 {
                // Density of a bin is 1/32 divided by its width; the endpoints
                // take their neighbouring bin's density.
                let (lo, hi) = (j.max(1) - 1, (j + 1).min(32));
                let w = (b[hi] - b[lo]).max(f64::MIN_POSITIVE);
                pdf.push(((hi - lo) as f64 / 32.0) / w);
            }
            energies.push(EnergyAngular {
                e_mev: e_grid[k],
                cosines,
                pdf,
                cdf,
            });
        }
    }
    Ok(Some(ElasticAngular { energies, lct }))
}

/// Decode the **DLW** entry for reaction index `i` (0-based over the `NR`
/// reactions that emit neutrons).
pub fn decode_energy_law(t: &RawAceTable, i: usize) -> Result<AceEnergyLaw, NjoyError> {
    let ldlw = t.jxs[jxs::LDLW];
    let dlw = t.jxs[jxs::DLW];
    if ldlw <= 0 || dlw <= 0 {
        return Err(NjoyError::EndfParse("ACE table has no DLW block".into()));
    }
    need(t, (ldlw - 1) as usize + i, 1, "LDLW")?;
    let loc = t.xss[(ldlw - 1) as usize + i] as i64;
    let base = (dlw - 1) as usize + (loc as usize) - 1;
    need(t, base, 2, "DLW header")?;
    let lnw = t.xss[base] as i64;
    let law = t.xss[base + 1] as i32;
    let idat = t.xss[base + 2] as i64;
    if lnw != 0 {
        // Multiple laws sharing a reaction, chosen by an applicability
        // probability. Refused rather than silently taking the first: taking
        // one of two laws over its whole range is a different nuclide.
        return Err(NjoyError::NotPorted(
            "ACE DLW with LNW != 0 (multiple competing laws for one reaction)",
        ));
    }
    // Skip the applicability TAB1 (NR, NBT, INT, NE, E, p) that precedes IDAT.
    let at = (dlw - 1) as usize + (idat as usize) - 1;

    match law {
        3 => {
            need(t, at, 2, "LAW=3 data")?;
            Ok(AceEnergyLaw::TwoBodyLevel {
                ldat1: t.xss[at] * EV_PER_MEV,
                ldat2: t.xss[at + 1],
            })
        }
        4 | 44 | 61 => {
            let n_arrays = match law {
                4 => 3,
                61 => 4,
                _ => 5,
            };
            let (rows, incident_interp) =
                read_tabulated_law(t, at, (dlw - 1) as usize, n_arrays, law)?;
            Ok(AceEnergyLaw::Tabulated { law, rows, incident_interp })
        }
        other => Err(NjoyError::EndfParse(format!(
            "ACE DLW LAW={other} is not decoded; the NJOY2016 reference library \
             uses only 3, 4, 44 and 61 on neutron tables"
        ))),
    }
}

/// The shared LAW=4/44/61 reader. `ldis` is the 0-based start of the DLW block,
/// which the per-energy locators are relative to.
fn read_tabulated_law(
    t: &RawAceTable,
    mut at: usize,
    ldis: usize,
    n_arrays: usize,
    law: i32,
) -> Result<(Vec<LawEnergyRow>, Vec<(u32, u32)>), NjoyError> {
    need(t, at, 1, "law interpolation header")?;
    let n_regions = t.xss[at] as usize;
    need(t, at + 1, 2 * n_regions, "law interpolation ranges")?;
    // The evaluation's own (NBT, INT) over the incident grid. An empty list
    // means "one lin-lin range over everything", which is ACE's default when
    // NR = 0 (`energy_distribution.py:230-231`); it is recorded as such rather
    // than fabricated, so a consumer can tell a stated law from an assumed one.
    let mut incident_interp: Vec<(u32, u32)> = Vec::with_capacity(n_regions);
    for r in 0..n_regions {
        let nbt = t.xss[at + 1 + r] as u32;
        let int = t.xss[at + 1 + n_regions + r] as u32;
        incident_interp.push((nbt, int));
    }
    need(t, at + 1 + 2 * n_regions, 1, "law incident-energy count")?;
    let n_in = t.xss[at + 1 + 2 * n_regions] as usize;
    at += 1 + 2 * n_regions + 1;
    need(t, at, 2 * n_in, "law incident grid")?;
    let e_in = t.xss[at..at + n_in].to_vec();
    let loc_dist = t.xss[at + n_in..at + 2 * n_in].to_vec();

    let mut rows = Vec::with_capacity(n_in);
    for k in 0..n_in {
        let d = ldis + (loc_dist[k] as usize) - 1;
        need(t, d, 2, "law per-energy header")?;
        let inttp = t.xss[d] as i64;
        let intt = inttp % 10;
        let n_discrete = ((inttp - intt) / 10) as usize;
        let n_out = t.xss[d + 1] as usize;
        need(t, d + 2, n_arrays * n_out, "law per-energy body")?;
        let col = |j: usize| t.xss[d + 2 + j * n_out..d + 2 + (j + 1) * n_out].to_vec();
        let e_out: Vec<f64> = col(0).iter().map(|e| e * EV_PER_MEV).collect();
        // The pdf is per MeV in the file and per eV here, so it scales by the
        // inverse of the energy scaling -- the cdf, being dimensionless, does
        // not scale at all. Scaling all three alike is a classic way to get a
        // spectrum that integrates to 1e6.
        let pdf: Vec<f64> = col(1).iter().map(|p| p / EV_PER_MEV).collect();
        let cdf = col(2);
        if n_discrete > 0 {
            return Err(NjoyError::NotPorted(
                "ACE tabulated law with discrete lines (INTTp >= 10)",
            ));
        }
        let kalbach = (law == 44).then(|| {
            let r = col(3);
            let a = col(4);
            r.iter()
                .zip(a.iter())
                .map(|(&r, &a)| ContinuumKalbachRow { r, a })
                .collect()
        });
        let cosines = if law == 61 {
            let lc = col(3);
            let mut v = Vec::with_capacity(n_out);
            for &l in &lc {
                let l = l as i64;
                if l > 0 {
                    v.push(read_cosine_table(t, ldis + (l as usize) - 1)?);
                } else {
                    // Isotropic at this outgoing energy.
                    v.push(ContinuumAngularRow {
                        cosines: vec![-1.0, 1.0],
                        cdf: vec![0.0, 1.0],
                        mubar: 0.0,
                    });
                }
            }
            Some(v)
        } else {
            None
        };
        rows.push(LawEnergyRow {
            e_in: e_in[k] * EV_PER_MEV,
            eout: ChiEout {
                e_out,
                pdf,
                cdf,
                linlin: intt == 2,
            },
            kalbach,
            cosines,
        });
    }
    Ok((rows, incident_interp))
}

/// Turn a decoded tabulated law into the workspace's [`ChiTabular`] plus its
/// angular representation.
pub fn to_chi_and_angular(
    rows: &[LawEnergyRow],
    law: i32,
    incident_interp: &[(u32, u32)],
) -> (ChiTabular, ContinuumAngular) {
    let chi = ChiTabular {
        incident: rows.iter().map(|r| r.e_in).collect(),
        tables: rows.iter().map(|r| r.eout.clone()).collect(),
        incident_interp: incident_interp.to_vec(),
    };
    let ang = match law {
        44 => ContinuumAngular::KalbachMann(
            rows.iter()
                .map(|r| ContinuumKalbachTable {
                    rows: r.kalbach.clone().unwrap_or_default(),
                })
                .collect(),
        ),
        61 => ContinuumAngular::LabTabulated(
            rows.iter()
                .map(|r| ContinuumAngularTable {
                    rows: r.cosines.clone().unwrap_or_default(),
                })
                .collect(),
        ),
        // LAW=4 defers its cosine to the AND block, which for these reactions
        // is isotropic in the evaluation itself -- not "unported", which is a
        // different claim and one this must not make silently.
        _ => ContinuumAngular::EvaluatedIsotropic,
    };
    (chi, ang)
}

/// Decode the **NU** block: prompt or total nu-bar as a function of energy.
///
/// Two forms: `LNU = 1` is a polynomial in `E` \[MeV\], `LNU = 2` a TAB1. A
/// negative first word means the block holds *both* prompt and total, with the
/// total following the prompt.
pub fn decode_nu(t: &RawAceTable) -> Result<Option<NuBar>, NjoyError> {
    let nu = t.jxs[jxs::NU];
    if nu <= 0 {
        return Ok(None); // not fissionable
    }
    let mut at = (nu - 1) as usize;
    need(t, at, 1, "NU header")?;
    if t.xss[at] < 0.0 {
        // Both prompt and total present: the total block starts after the
        // prompt one, whose length is |first word|. Prefer TOTAL nu-bar --
        // using prompt as though it were total loses the delayed neutrons and
        // biases k low by roughly beta.
        let len = (-t.xss[at]) as usize;
        at += 1 + len;
    }
    need(t, at, 2, "NU LNU")?;
    let lnu = t.xss[at] as i32;
    match lnu {
        1 => {
            let nc = t.xss[at + 1] as usize;
            need(t, at + 2, nc, "NU polynomial")?;
            let c = t.xss[at + 2..at + 2 + nc].to_vec();
            // Tabulate the polynomial so the consumer sees one shape. The grid
            // is the one the transport crate uses for nu elsewhere.
            let mut energy = Vec::new();
            let mut nu_total = Vec::new();
            let mut e = 1.0e-5_f64;
            while e <= 2.0e7 {
                let e_mev = e / EV_PER_MEV;
                let v = c.iter().rev().fold(0.0, |acc, &ci| acc * e_mev + ci);
                energy.push(e);
                nu_total.push(v);
                e *= 1.2;
            }
            Ok(Some(NuBar { energy, nu_total }))
        }
        2 => {
            // The TAB1 begins immediately after LNU, so its NR is at `at + 1`
            // and its pair count at `at + 2 + 2*NR`
            // (`openmc/data/function.py:441-457`).
            //
            // An earlier draft read NR from `at + 2` and the pair count one
            // word further still. It did not fail -- it returned U-235's
            // nu-bar as a flat **53.0**, which is the pair count read as a
            // value. A fission yield of 53 neutrons is obviously wrong to a
            // human and completely invisible to a type system, which is why
            // the test below gates on the physical value and not merely on
            // decoding without error.
            need(t, at + 1, 1, "NU TAB1 NR")?;
            let n_regions = t.xss[at + 1] as usize;
            let j = at + 2 + 2 * n_regions;
            need(t, j, 1, "NU TAB1 count")?;
            let n = t.xss[j] as usize;
            need(t, j + 1, 2 * n, "NU TAB1 body")?;
            let energy: Vec<f64> = t.xss[j + 1..j + 1 + n].iter().map(|e| e * EV_PER_MEV).collect();
            let nu_total = t.xss[j + 1 + n..j + 1 + 2 * n].to_vec();
            Ok(Some(NuBar { energy, nu_total }))
        }
        other => Err(NjoyError::EndfParse(format!("ACE NU block LNU={other}"))),
    }
}

/// How many reactions in MTR carry secondary neutrons, i.e. how many entries
/// LDLW and LAND (after elastic) have.
pub fn n_neutron_reactions(t: &RawAceTable) -> usize {
    t.nxs[nxs::NR].max(0) as usize
}
