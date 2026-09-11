// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The ERRORR-tape half of `covard` and the tape scan of `expndo`.
//!
//! Upstream `covard` (`covr.f90:720-937`) walks an ERRORR output tape with
//! `repoz`/`finds`/`contio`/`listio`/`moreio` to collect, for one reaction
//! pair `(mat,mt)` x `(mat1,mt1)`:
//!
//! 1. the group structure — the LIST of `MF=1/MT=451` (`covr.f90:746-780`);
//! 2. the two coarse-group cross-section vectors — the LIST of
//!    `MF=mf35/MT=mt` for `mat` and `MF=mf35/MT=mt1` for `mat1`
//!    (`covr.f90:783-808`);
//! 3. the covariance sub-block — inside `MF=mf3x/MT=mt` of `mat`, the
//!    subsection whose CONT carries `(mat1,mt1)`, one LIST per non-empty
//!    matrix row (`covr.f90:813-886`).
//!
//! This module ports exactly that walk against the crate's parsed
//! [`Tape`], producing the in-memory [`ErrorrCovarianceSection`] that
//! [`ErrorrCovarianceSection::to_dense`] (the numeric half of `covard`,
//! `covr.f90:895-935`) already consumes. It also ports the `expndo` tape scan
//! (`covr.f90:526-556`) that lists the MTs present in `MF=mf35`.
//!
//! # Tape flags (`covr.f90:314-333`)
//!
//! The HEAD of the first `MF=1/MT=451` carries `mfflg` in `N1`: `-11`
//! (ERRORR `mfcov=33`) and `-14` (`mfcov=40`) put the cross sections in
//! `MF=3`; `-12` (`mfcov=35`) puts the spectra in `MF=5`. Anything else is
//! "illegal errorr output tape for covr". The covariance file `mf3x` is then
//! `33`, or `40` when `mfflg=-14`, `35` when the vectors live in `MF=5`, and
//! `34` for `mt=251` (`covr.f90:813-816`).

use crate::covr::covard::{CovarianceRowBlock, ErrorrCovarianceSection};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// What the first two records of an ERRORR tape say about its layout
/// (`covr.f90:319-333`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorrTapeKind {
    /// `mfflg` — `N1` of the first `MF=1/MT=451` HEAD (`-11`, `-12`, `-14`).
    pub mfflg: i32,
    /// `mf35` — the file holding the per-reaction vectors (`3` or `5`).
    pub mf35: i32,
}

impl ErrorrTapeKind {
    /// Read `mfflg` from the first section of `tape` and classify it
    /// (`covr.f90:319-333`).
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when the first section is not `MF=1/MT=451`
    /// ("illegal input tape") or `mfflg` is not one of `-11/-12/-14`
    /// ("illegal errorr output tape for covr").
    pub fn from_tape(tape: &Tape) -> Result<Self, NjoyError> {
        let first = tape
            .sections()
            .first()
            .ok_or_else(|| NjoyError::EndfParse("covr: illegal input tape (empty)".into()))?;
        if first.key.mf != 1 || first.key.mt != 451 {
            return Err(NjoyError::EndfParse("covr: illegal input tape".into()));
        }
        let head = SectionCursor::new(&first.rows).read_cont()?;
        let mfflg = head.n1;
        let mf35 = match mfflg {
            -11 | -14 => 3,
            -12 => 5,
            _ => {
                return Err(NjoyError::EndfParse(format!(
                    "covr: illegal errorr output tape for covr (mfflg={mfflg})"
                )))
            }
        };
        Ok(Self { mfflg, mf35 })
    }

    /// The covariance file to search for `(mat,mt)` (`covr.f90:813-816`).
    pub fn mf3x(&self, mt: i32) -> i32 {
        let mut mf3x = 33;
        if self.mfflg == -14 {
            mf3x = 40;
        }
        if self.mf35 == 5 {
            mf3x = 35;
        }
        if mt == 251 {
            mf3x = 34;
        }
        mf3x
    }
}

/// The group structure of one material on an ERRORR tape
/// (`covr.f90:746-780`): `ixmax` groups, `ixmax+1` boundaries in eV.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupStructure {
    /// `ixmax` — `L1` of the `MF=1/MT=451` LIST.
    pub ixmax: usize,
    /// `y(1..ixp)` — the boundaries \[eV\], `ixp = N1` of that LIST.
    pub boundaries: Vec<f64>,
    /// `iverf` — `L1` of the HEAD (ENDF format era).
    pub iverf: i32,
    /// `iza` — `ZA` from the HEAD.
    pub iza: i32,
}

/// Read the group structure of `mat` (`covr.f90:746-780`).
///
/// # Errors
/// [`NjoyError::SectionNotFound`] when `mat` has no `MF=1/MT=451`
/// (`finds` would abort); [`NjoyError::EndfParse`] on a malformed LIST.
pub fn read_group_structure(tape: &Tape, mat: i32) -> Result<GroupStructure, NjoyError> {
    let sec = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound {
            mat,
            mf: 1,
            mt: 451,
        })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let list = cur.read_list()?;
    let ixmax = usize::try_from(list.head.l1)
        .map_err(|_| NjoyError::EndfParse("covard: negative group count".into()))?;
    let ixp = usize::try_from(list.head.n1)
        .map_err(|_| NjoyError::EndfParse("covard: negative boundary count".into()))?;
    if ixp != ixmax + 1 || list.data.len() < ixp {
        return Err(NjoyError::EndfParse(format!(
            "covard: MF=1/451 of mat {mat} has ixmax={ixmax} but {ixp} boundaries ({} values)",
            list.data.len()
        )));
    }
    Ok(GroupStructure {
        ixmax,
        boundaries: list.data[..ixp].to_vec(),
        iverf: head.l1,
        iza: head.c1.round() as i32,
    })
}

/// Read the `ixmax` coarse-group values of `MF=mf35/MT=mt` for `mat`
/// (`covr.f90:783-808`). For `mf35=5` the LIST's `C2` is the incident energy
/// `einc`, returned alongside (0 otherwise).
///
/// # Errors
/// [`NjoyError::SectionNotFound`] when the section is absent (upstream
/// `finds` aborts); [`NjoyError::EndfParse`] when it holds fewer than `ixmax`
/// values.
pub fn read_vector(
    tape: &Tape,
    mat: i32,
    mf35: i32,
    mt: i32,
    ixmax: usize,
) -> Result<(Vec<f64>, f64), NjoyError> {
    let sec = tape
        .section(mat, mf35, mt)
        .ok_or(NjoyError::SectionNotFound { mat, mf: mf35, mt })?;
    let list = SectionCursor::new(&sec.rows).read_list()?;
    if list.data.len() < ixmax {
        return Err(NjoyError::EndfParse(format!(
            "covard: MF={mf35}/MT={mt} of mat {mat} has {} values, need {ixmax}",
            list.data.len()
        )));
    }
    let einc = if mf35 == 5 { list.head.c2 } else { 0.0 };
    Ok((list.data[..ixmax].to_vec(), einc))
}

/// The sparse covariance rows of the `(mat1,mt1)` subsection of
/// `MF=mf3x/MT=mt` for `mat` (`covr.f90:813-886`), or `None` when the section
/// is a lumped-component placeholder (`nmt = N2 = 0`, `covr.f90:820`).
///
/// # Errors
/// [`NjoyError::SectionNotFound`] when `(mat,mf3x,mt)` is absent;
/// [`NjoyError::EndfParse`] when no subsection carries `(mat1,mt1)`
/// (`covr.f90:827-831`: "did not find file 33 subsection") or a record is
/// malformed.
pub fn read_covariance_rows(
    tape: &Tape,
    kind: ErrorrTapeKind,
    mat: i32,
    mt: i32,
    mat1: i32,
    mt1: i32,
) -> Result<Option<Vec<CovarianceRowBlock>>, NjoyError> {
    let mf3x = kind.mf3x(mt);
    let sec = tape
        .section(mat, mf3x, mt)
        .ok_or(NjoyError::SectionNotFound { mat, mf: mf3x, mt })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let nmt = head.n2;
    if nmt == 0 {
        return Ok(None);
    }
    // covr.f90:823-861 — search for the desired subsection of this mt.
    for _ in 0..nmt {
        let c = cur.read_cont()?;
        let (mut mat1x, mtx) = if mf3x == 34 {
            (mat, c.l1)
        } else {
            (c.l1, c.l2)
        };
        if mat1x == 0 {
            mat1x = mat;
        }
        let kgp = c.n2;
        let wanted = mat1x == mat1 && mtx == mt1;
        let mut blocks = Vec::new();
        // One LIST per stored row, the last one carrying kl >= kgp
        // (covr.f90:851-861 skip, :862-886 collect).
        loop {
            let l = cur.read_list()?;
            let kl = l.head.n2;
            if wanted {
                let row = usize::try_from(kl)
                    .map_err(|_| NjoyError::EndfParse("covard: negative row index".into()))?;
                let first_col = usize::try_from(l.head.l2)
                    .map_err(|_| NjoyError::EndfParse("covard: negative column index".into()))?;
                blocks.push(CovarianceRowBlock {
                    row,
                    first_col,
                    values: l.data,
                });
            }
            if kl >= kgp {
                break;
            }
        }
        if wanted {
            return Ok(Some(blocks));
        }
    }
    Err(NjoyError::EndfParse(format!(
        "covard: did not find file {mf3x} subsection for mt={mt1} mat={mat1} (in MT={mt} of mat {mat})"
    )))
}

/// Assemble the full [`ErrorrCovarianceSection`] for one reaction pair — the
/// tape half of `covard` (`covr.f90:740-886`) — ready for
/// [`ErrorrCovarianceSection::to_dense`].
///
/// The group structure is read from `mat`'s `MF=1/MT=451` (the material of
/// the *first* argument, as upstream's `finds(mat,1,451,nin)` does — for the
/// column-auto call inside `corr` that is `mat1`). The row vector `xx` is
/// `MF=mf35/MT=mt` of `mat`, the column vector `xy` is `MF=mf35/MT=mt1` of
/// `mat1` (`xy = xx` for an auto pair, `covr.f90:797-807`).
///
/// Returns `None` for a placeholder section (`nmt=0`), where upstream leaves
/// `cf` zero and skips the zero-test (`covr.f90:820`); callers treat that as a
/// null matrix.
///
/// # Errors
/// Any of [`read_group_structure`], [`read_vector`], [`read_covariance_rows`].
pub fn read_covariance_section(
    tape: &Tape,
    kind: ErrorrTapeKind,
    mat: i32,
    mt: i32,
    mat1: i32,
    mt1: i32,
) -> Result<Option<ErrorrCovarianceSection>, NjoyError> {
    let gs = read_group_structure(tape, mat)?;
    let ixmax = gs.ixmax;
    let (xx, _einc) = read_vector(tape, mat, kind.mf35, mt, ixmax)?;
    let xy = if mat == mat1 && mt == mt1 {
        xx.clone()
    } else {
        read_vector(tape, mat1, kind.mf35, mt1, ixmax)?.0
    };
    let Some(blocks) = read_covariance_rows(tape, kind, mat, mt, mat1, mt1)? else {
        return Ok(None);
    };
    Ok(Some(ErrorrCovarianceSection {
        ixmax,
        xx,
        xy,
        group_boundaries: gs.boundaries,
        blocks,
    }))
}

/// The MTs present in `MF=mf35` for `mat`, in tape order — the scan half of
/// `expndo` (`covr.f90:526-556`) before stripping.
///
/// Upstream positions at `(mat,mf35,0)` and reads LIST records until the file
/// number changes, so every section of that file counts; lumped-component
/// placeholders never appear here because ERRORR writes no `MF=3` LIST for
/// them.
pub fn present_mts(tape: &Tape, mat: i32, mf35: i32) -> Vec<i32> {
    tape.sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == mf35)
        .map(|s| s.key.mt)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::covr::input::CovarianceForm;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;

    /// A two-group ERRORR-layout tape for MAT 1: MF=1/451 (mfflg=-11), MF=3
    /// for MT=1 and MT=2, MF=33/MT=1 with subsections (1,1) and (1,2), and
    /// MF=33/MT=2 with (1,2)'s auto block; MT=5 is a lumped placeholder.
    fn tiny_tape() -> Tape {
        let key = |mf, mt| EndfKey { mat: 1, mf, mt };
        let sections = vec![
            Section {
                key: key(1, 451),
                rows: vec![
                    [1001.0, 0.9992, 6.0, 0.0, -11.0, 0.0],
                    [293.6, 0.0, 2.0, 0.0, 3.0, 0.0],
                    [1.0e-5, 1.0e3, 2.0e7, 0.0, 0.0, 0.0],
                ],
            },
            Section {
                key: key(3, 1),
                rows: vec![
                    [1001.0, 0.0, 0.0, 0.0, 2.0, 0.0],
                    [10.0, 20.0, 0.0, 0.0, 0.0, 0.0],
                ],
            },
            Section {
                key: key(3, 2),
                rows: vec![
                    [1001.0, 0.0, 0.0, 0.0, 2.0, 0.0],
                    [3.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                ],
            },
            Section {
                key: key(33, 1),
                rows: vec![
                    [1001.0, 0.9992, 0.0, 0.0, 0.0, 2.0],
                    // subsection (mat1=0 -> mat, mt1=1), kgp=2
                    [0.0, 0.0, 0.0, 1.0, 0.0, 2.0],
                    [0.0, 0.0, 2.0, 1.0, 2.0, 1.0], // row 1 from col 1: 2 values
                    [0.04, 0.01, 0.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 2.0, 1.0, 2.0], // row 2 from col 2: 1 value
                    [0.09, 0.0, 0.0, 0.0, 0.0, 0.0],
                    // subsection (mat1=0, mt1=2), kgp=2
                    [0.0, 0.0, 0.0, 2.0, 0.0, 2.0],
                    [0.0, 0.0, 1.0, 1.0, 1.0, 2.0], // only the last row (2), col 1
                    [0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
                ],
            },
            Section {
                key: key(33, 2),
                rows: vec![
                    [1001.0, 0.9992, 0.0, 0.0, 0.0, 1.0],
                    [0.0, 0.0, 0.0, 2.0, 0.0, 2.0],
                    [0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 2.0, 1.0, 2.0],
                    [2.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                ],
            },
            Section {
                key: key(33, 5),
                rows: vec![[1001.0, 0.9992, 0.0, 4.0, 0.0, 0.0]],
            },
        ];
        Tape::from_sections(String::new(), sections)
    }

    /// Methodology: the header classification (`covr.f90:319-333`): `N1=-11`
    /// on the first `MF=1/451` HEAD means the vectors live in MF=3 and the
    /// covariances in MF=33 (MF=34 for MT=251). Result (2026-09-10): as
    /// expected.
    #[test]
    fn classifies_mfcov33_tape() {
        let k = ErrorrTapeKind::from_tape(&tiny_tape()).unwrap();
        assert_eq!(
            k,
            ErrorrTapeKind {
                mfflg: -11,
                mf35: 3
            }
        );
        assert_eq!(k.mf3x(102), 33);
        assert_eq!(k.mf3x(251), 34);
        assert_eq!(
            ErrorrTapeKind {
                mfflg: -14,
                mf35: 3
            }
            .mf3x(1),
            40
        );
        assert_eq!(
            ErrorrTapeKind {
                mfflg: -12,
                mf35: 5
            }
            .mf3x(18),
            35
        );
    }

    /// Methodology: walk the auto pair (1,1)x(1,1) exactly as `covard` does
    /// and check the assembled section: 2 groups, boundaries, `xx = xy`
    /// from MF=3/MT=1, and the sparse rows `(1,1,[.04,.01])`, `(2,2,[.09])`.
    /// Then `to_dense` gives `[[.04,.01],[0,.09]]`. Result (2026-09-10):
    /// exact.
    #[test]
    fn reads_auto_section_and_densifies() {
        let tape = tiny_tape();
        let kind = ErrorrTapeKind::from_tape(&tape).unwrap();
        let sec = read_covariance_section(&tape, kind, 1, 1, 1, 1)
            .unwrap()
            .expect("not a placeholder");
        assert_eq!(sec.ixmax, 2);
        assert_eq!(sec.group_boundaries, vec![1.0e-5, 1.0e3, 2.0e7]);
        assert_eq!(sec.xx, vec![10.0, 20.0]);
        assert_eq!(sec.xy, sec.xx);
        assert_eq!(sec.blocks.len(), 2);
        assert_eq!(
            sec.blocks[1],
            CovarianceRowBlock {
                row: 2,
                first_col: 2,
                values: vec![0.09]
            }
        );
        let d = sec.to_dense(CovarianceForm::Relative).unwrap();
        assert_eq!(d.matrix.get(0, 1), 0.01);
        assert_eq!(d.matrix.get(1, 0), 0.0);
    }

    /// Methodology: the cross pair (1,1)x(1,2) must skip the (1,1)
    /// subsection's two LISTs and pick the (1,2) one (`covr.f90:840-861`);
    /// `xy` comes from MF=3/MT=2, whose group 2 is zero, so `to_dense`
    /// zeroes nothing here (the single stored element is at (2,1) where
    /// `xx(2)*xy(1) = 20*3 != 0`) and the matrix is non-null.
    /// Result (2026-09-10): as expected.
    #[test]
    fn reads_cross_section_skipping_other_subsections() {
        let tape = tiny_tape();
        let kind = ErrorrTapeKind::from_tape(&tape).unwrap();
        let sec = read_covariance_section(&tape, kind, 1, 1, 1, 2)
            .unwrap()
            .unwrap();
        assert_eq!(sec.xy, vec![3.0, 0.0]);
        assert_eq!(
            sec.blocks,
            vec![CovarianceRowBlock {
                row: 2,
                first_col: 1,
                values: vec![0.5]
            }]
        );
        let d = sec.to_dense(CovarianceForm::Relative).unwrap();
        assert_eq!(d.matrix.get(1, 0), 0.5);
        assert!(!d.is_null);
    }

    /// Methodology: a lumped-component placeholder (`N2=0` on the HEAD,
    /// `covr.f90:820`) yields `None`; a pair whose subsection is absent
    /// errors with the upstream "did not find" message (`covr.f90:827-831`).
    /// Result (2026-09-10): as expected.
    #[test]
    fn placeholder_is_none_and_missing_subsection_errors() {
        let tape = tiny_tape();
        let kind = ErrorrTapeKind::from_tape(&tape).unwrap();
        assert!(read_covariance_rows(&tape, kind, 1, 5, 1, 5)
            .unwrap()
            .is_none());
        match read_covariance_rows(&tape, kind, 1, 2, 1, 1) {
            Err(NjoyError::EndfParse(m)) => assert!(m.contains("did not find file 33"), "{m}"),
            other => panic!("expected EndfParse, got {other:?}"),
        }
    }

    /// Methodology: `expndo`'s scan lists the MF=3 MTs in tape order
    /// (`covr.f90:534-556`); the placeholder MT=5 has no MF=3 LIST and is
    /// absent. Result (2026-09-10): `[1, 2]`.
    #[test]
    fn present_mts_lists_mf3_sections_in_order() {
        assert_eq!(present_mts(&tiny_tape(), 1, 3), vec![1, 2]);
    }
}
