// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr`, l.425-444 — the `nendf = 999` card path.
//   - `subroutine covadd`, l.6907-6966 — insert dummy MF=33 sections.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The `999` option: give an evaluation that carries MF=32 but no MF=33
//! the empty MF=33 sections ERRORR needs to have output blocks for the
//! resonance-parameter covariance to land in (the NJOY2016 test-suite
//! deck for Cl-35, `tests/20/input`, opens with it).
//!
//! Each dummy section is `HEAD (ZA, AWR, 0, 0, 0, NL=1)`, a `CONT (0, 0,
//! MAT1=0, MT1=mt, NC=0, NI=1)` and one `LB=5`, `LS=1` LIST with `NT=3`,
//! `NE=2`: the two energies `1e-5, 2e7` and a single zero element. The
//! MF=1/451 dictionary gains one `(33, mt, 4, 0)` line per section and
//! `NXC` grows by the count. Upstream writes the sections right after the
//! MF=32 FEND, so a tape without MF=32 gets none — the port refuses that
//! case instead of silently returning the input.

use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::NjoyError;

/// Insert dummy MF=33 sections for `mts` (at most five, as upstream's
/// `iaddmt(5)`) into material `mat` of `tape`.
///
/// # Errors
/// `EndfParse` when the material has no MF=1/451 or no MF=32 section, or
/// more than five reactions are asked for.
pub fn covadd(tape: &Tape, mat: i32, mts: &[i32]) -> Result<Tape, NjoyError> {
    if mts.len() > 5 {
        return Err(NjoyError::EndfParse(
            "errorr: errorr in 999 option (more than five mts)".into(),
        ));
    }
    let head = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound {
            mat,
            mf: 1,
            mt: 451,
        })?;
    let (za, awr) = head
        .rows
        .first()
        .map(|r| (r[0], r[1]))
        .ok_or_else(|| NjoyError::EndfParse("errorr::covadd: empty MF=1/451".into()))?;
    if tape
        .sections()
        .iter()
        .all(|s| !(s.key.mat == mat && s.key.mf == 32))
    {
        return Err(NjoyError::EndfParse(
            "errorr::covadd: the material has no MF=32 (upstream writes the dummy MF=33 after its FEND)"
                .into(),
        ));
    }
    let last_mf32 = tape
        .sections()
        .iter()
        .rposition(|s| s.key.mat == mat && s.key.mf == 32)
        .unwrap();

    let mut out: Vec<Section> = Vec::with_capacity(tape.sections().len() + mts.len());
    for (i, sec) in tape.sections().iter().enumerate() {
        if sec.key.mat == mat && sec.key.mf == 1 && sec.key.mt == 451 {
            let mut rows = sec.rows.clone();
            if rows.len() >= 4 {
                rows[3][5] += mts.len() as f64; // NXC
            }
            for &mt in mts {
                rows.push([0.0, 0.0, 33.0, f64::from(mt), 4.0, 0.0]);
            }
            out.push(Section { key: sec.key, rows });
        } else {
            out.push(Section {
                key: sec.key,
                rows: sec.rows.clone(),
            });
        }
        if i == last_mf32 {
            for &mt in mts {
                out.push(Section {
                    key: EndfKey { mat, mf: 33, mt },
                    rows: vec![
                        [za, awr, 0.0, 0.0, 0.0, 1.0],
                        [0.0, 0.0, 0.0, f64::from(mt), 0.0, 1.0],
                        [0.0, 0.0, 1.0, 5.0, 3.0, 2.0],
                        [1.0e-5, 2.0e7, 0.0, 0.0, 0.0, 0.0],
                    ],
                });
            }
        }
    }
    let mut new = Tape::from_sections(tape.tpid.clone(), out);
    new.copy_raw_mf32_from(tape);
    Ok(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tape with MF=1/451, MF=2 and MF=32 gains the dummy sections after
    /// MF=32, in the order asked, with `NXC` bumped; one without MF=32 is
    /// refused.
    #[test]
    fn dummy_sections_follow_mf32() {
        let key = |mf, mt| EndfKey { mat: 9, mf, mt };
        let secs = vec![
            Section {
                key: key(1, 451),
                rows: vec![
                    [9000.0, 8.9, 0.0, 0.0, 0.0, 0.0],
                    [0.0; 6],
                    [0.0; 6],
                    [0.0, 0.0, 0.0, 0.0, 1.0, 2.0],
                    [0.0; 6],
                    [0.0, 0.0, 2.0, 151.0, 3.0, 0.0],
                    [0.0, 0.0, 32.0, 151.0, 3.0, 0.0],
                ],
            },
            Section {
                key: key(2, 151),
                rows: vec![[0.0; 6]],
            },
            Section {
                key: key(32, 151),
                rows: vec![[0.0; 6]],
            },
            Section {
                key: key(34, 2),
                rows: vec![[0.0; 6]],
            },
        ];
        let tape = Tape::from_sections("t".into(), secs);
        let out = covadd(&tape, 9, &[1, 102]).unwrap();
        let keys: Vec<(i32, i32)> = out.sections().iter().map(|s| (s.key.mf, s.key.mt)).collect();
        assert_eq!(
            keys,
            vec![(1, 451), (2, 151), (32, 151), (33, 1), (33, 102), (34, 2)]
        );
        let h = out.section(9, 1, 451).unwrap();
        assert_eq!(h.rows[3][5], 4.0);
        assert_eq!(h.rows[h.rows.len() - 1], [0.0, 0.0, 33.0, 102.0, 4.0, 0.0]);
        let d = out.section(9, 33, 102).unwrap();
        assert_eq!(d.rows[0], [9000.0, 8.9, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(d.rows[2], [0.0, 0.0, 1.0, 5.0, 3.0, 2.0]);
        assert_eq!(d.rows[3][1], 2.0e7);

        let no32 = Tape::from_sections(
            "t".into(),
            vec![Section {
                key: key(1, 451),
                rows: vec![[9000.0, 8.9, 0.0, 0.0, 0.0, 0.0]; 4],
            }],
        );
        assert!(covadd(&no32, 9, &[1]).is_err());
    }
}
