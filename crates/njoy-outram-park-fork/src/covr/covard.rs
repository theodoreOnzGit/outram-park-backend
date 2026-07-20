// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! COVR's ERRORR-covariance reader (`covard`) and MT-scan (`expndo`).
//!
//! `subroutine covard` (`covr.f90:720-937`) reads the ENDF-like covariance
//! records that ERRORR writes and turns them into an *engineering* matrix — a
//! full, dense group-by-group covariance matrix with the zeroes filled in. This
//! module ports that transformation, plus the `expndo` MT-pair enumeration
//! (`covr.f90:508-576`).
//!
//! # What is ported here vs the ENDF tape I/O
//!
//! Upstream `covard` performs two intertwined jobs:
//!
//! 1. **ENDF tape I/O** — `repoz`/`finds`/`contio`/`listio`/`moreio` walk an
//!    ERRORR output tape (unit `nin`) to locate the group structure, the
//!    per-reaction cross sections, and the covariance sub-blocks
//!    (`covr.f90:740-886`).
//! 2. **The covariance transform** — scatter the sparse row-blocks into a dense
//!    matrix, zero out spurious covariances where a cross section is zero, and
//!    convert absolute covariances to relative (`covr.f90:895-935`).
//!
//! This crate does **not** yet have an ERRORR covariance *tape* to read: the
//! ERRORR module ([`crate::errorr`]) ports the card deck and group structures
//! but not the covariance-output kernels (`covout`/`colaps`), so there is no
//! on-tape byte stream to parse. This port therefore models job (2) — the
//! numeric transform — operating on an **in-memory** representation of an
//! ERRORR covariance subsection ([`ErrorrCovarianceSection`]) rather than
//! decoding `contio`/`listio` records off a physical tape. When the ERRORR
//! covariance-output path is ported, its in-memory product should adopt (or be
//! adapted to) [`ErrorrCovarianceSection`] so this reader can consume it
//! directly. The ENDF-record-decoding half (job 1) stays a documented gap.
//!
//! # The auto- vs cross-covariance rsd sourcing (human verify point)
//!
//! In `subroutine corr` (`covr.f90:597-711`) the standard-deviation vectors that
//! normalise a covariance into a correlation are **not** taken from the
//! covariance matrix being normalised. For a cross-covariance between the row
//! reaction `(mat,mt)` and the column reaction `(mat1,mt1)`:
//!
//! - `rsd_y` (columns) is `sqrt(diag)` of the **column reaction's own
//!   auto-covariance** — `covard(nin, mat1,mt1, mat1,mt1, …)` at
//!   `covr.f90:607,636-648`;
//! - `rsd_x` (rows) is `sqrt(diag)` of the **row reaction's own
//!   auto-covariance** — `covard(nin, mat,mt, mat,mt, …)` at
//!   `covr.f90:653,658-666`;
//! - the cross matrix itself — `covard(nin, mat,mt, mat1,mt1, …)` at
//!   `covr.f90:669` — supplies only the numerators.
//!
//! [`correlation_from_auto_and_cross`] wires exactly that sourcing, which is the
//! documented point a human must confirm against a real ERRORR tape's data flow.

use crate::covr::correlation::{correlation_cross, CorrelationMatrix, CovarianceMatrix};
use crate::covr::input::CovarianceForm;
use crate::NjoyError;

/// A group-boundary energy in **electron-volts (eV)**, matching the ERRORR /
/// GROUPR convention (`crate::errorr::groups` returns eV boundaries).
pub type GroupBoundaryEv = f64;

/// A cross-section value in **barns**, as carried on the ERRORR covariance tape
/// alongside each covariance subsection (`covr.f90:800-807`). Used only to
/// convert absolute covariances to relative and to zero spurious covariances in
/// zero-cross-section regions.
pub type CrossSectionBarn = f64;

/// A covariance-matrix element — dimensionless for relative covariances, or the
/// units of the observable squared for absolute covariances.
pub type CovarianceElement = f64;

// ---------------------------------------------------------------------------
// In-memory ERRORR covariance subsection (mirrors the tape records covard reads)
// ---------------------------------------------------------------------------

/// One row-block of an ERRORR covariance subsection — the in-memory analogue of
/// a single `listio` LIST record inside `covard` (`covr.f90:862-871`).
///
/// ERRORR stores a covariance matrix sparsely: each record carries one matrix
/// **row** `row` (1-based energy group `kl`), the 1-based column group
/// `first_col` (`lgp1`) at which its data begins, and the contiguous covariance
/// values from that column onward. `covard` scatters them into the dense matrix
/// at `cf(ixmax*(row-1) + first_col-1 + k)` for `k = 1..values.len()`
/// (`covr.f90:876-879`).
#[derive(Debug, Clone, PartialEq)]
pub struct CovarianceRowBlock {
    /// 1-based row (energy-group) index `kl` of this block (`covr.f90:864`).
    pub row: usize,
    /// 1-based starting column (energy-group) index `lgp1` (`covr.f90:863`).
    pub first_col: usize,
    /// Contiguous covariance values for columns `first_col..first_col+len`.
    pub values: Vec<CovarianceElement>,
}

/// An ERRORR-produced covariance subsection for one reaction pair, in the
/// sparse row-block form `covard` reads (`covr.f90:819-886`).
///
/// This is the in-memory stand-in for the ENDF-like records on an ERRORR output
/// tape (see the module docs): the group count `ixmax`, the two per-reaction
/// cross-section vectors, and the sparse covariance row-blocks. `covard`
/// ([`Self::to_dense`]) turns it into a full dense matrix.
///
/// - `ixmax` — number of energy groups (`ixmax`, `covr.f90:749`).
/// - `xx` — cross section of the **row** reaction `(mat,mt)`, one per group,
///   barns (`covr.f90:800-802`).
/// - `xy` — cross section of the **column** reaction `(mat1,mt1)`, one per
///   group, barns (`covr.f90:805-807`); equals `xx` for an auto-covariance.
/// - `group_boundaries` — the `ixmax+1` group-boundary energies in eV
///   (`covr.f90:777-778`). Read by `covard` but used only by the (unported)
///   plot path; retained here for completeness / round-tripping.
/// - `blocks` — the sparse covariance row-blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorrCovarianceSection {
    /// Number of energy groups `ixmax`.
    pub ixmax: usize,
    /// Row-reaction cross section per group (barns), length `ixmax`.
    pub xx: Vec<CrossSectionBarn>,
    /// Column-reaction cross section per group (barns), length `ixmax`.
    pub xy: Vec<CrossSectionBarn>,
    /// Group boundaries (eV), length `ixmax + 1`. May be empty if unavailable.
    pub group_boundaries: Vec<GroupBoundaryEv>,
    /// Sparse covariance row-blocks.
    pub blocks: Vec<CovarianceRowBlock>,
}

/// The result of running `covard` on one covariance subsection.
///
/// Carries the dense covariance matrix plus the two diagnostics `covard`
/// tracks: how many elements were zeroed because a cross section was zero
/// (`ipflag`, `covr.f90:918`), and whether the resulting matrix is entirely
/// zero (`izero == 0`, `covr.f90:930,897`).
#[derive(Debug, Clone, PartialEq)]
pub struct CovardResult {
    /// The dense group-by-group covariance matrix (relative if converted).
    pub matrix: CovarianceMatrix,
    /// Count of elements zeroed due to a zero cross section (`ipflag`).
    pub zeroed_for_zero_xsec: usize,
    /// True when every element is zero after processing (`izero == 0`).
    pub is_null: bool,
}

impl ErrorrCovarianceSection {
    /// Validate the shape of the subsection (the checks `covard` relies on the
    /// ENDF tape to have satisfied).
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when `ixmax == 0`, when either cross-section
    /// vector is not length `ixmax`, when `group_boundaries` is non-empty but
    /// not length `ixmax+1`, or when any block references a row/column outside
    /// `1..=ixmax`.
    fn validate(&self) -> Result<(), NjoyError> {
        if self.ixmax == 0 {
            return Err(NjoyError::EndfParse("covard: ixmax must be > 0".into()));
        }
        if self.xx.len() != self.ixmax || self.xy.len() != self.ixmax {
            return Err(NjoyError::EndfParse(format!(
                "covard: cross-section vectors must have length ixmax={} (xx={}, xy={})",
                self.ixmax,
                self.xx.len(),
                self.xy.len()
            )));
        }
        if !self.group_boundaries.is_empty() && self.group_boundaries.len() != self.ixmax + 1 {
            return Err(NjoyError::EndfParse(format!(
                "covard: group_boundaries must have length ixmax+1={} (got {})",
                self.ixmax + 1,
                self.group_boundaries.len()
            )));
        }
        for b in &self.blocks {
            if b.row < 1 || b.row > self.ixmax {
                return Err(NjoyError::EndfParse(format!(
                    "covard: block row {} out of range 1..={}",
                    b.row, self.ixmax
                )));
            }
            let last_col = b.first_col + b.values.len().saturating_sub(1);
            if b.first_col < 1 || last_col > self.ixmax {
                return Err(NjoyError::EndfParse(format!(
                    "covard: block columns {}..={} out of range 1..={}",
                    b.first_col, last_col, self.ixmax
                )));
            }
        }
        Ok(())
    }

    /// Run `covard` on this subsection: scatter the sparse row-blocks into a
    /// full dense matrix, zero out spurious covariances where a cross section
    /// is zero, and (for [`CovarianceForm::Absolute`]) divide by the cross
    /// sections to produce relative covariances (`covr.f90:815-935`).
    ///
    /// `form` is the `irelco` flag: [`CovarianceForm::Relative`] (`irelco==1`)
    /// leaves the data as read; [`CovarianceForm::Absolute`] (`irelco==0`)
    /// applies `cf(k,n) /= xx(k)*xy(n)` (`covr.f90:929`).
    ///
    /// Element `(k, n)` (1-based groups) is zeroed when `xx(k)*xy(n) == 0`,
    /// regardless of `form` (`covr.f90:915-926`), since a covariance in a region
    /// of zero cross section is spurious.
    ///
    /// # Errors
    /// Propagates [`Self::validate`].
    pub fn to_dense(&self, form: CovarianceForm) -> Result<CovardResult, NjoyError> {
        self.validate()?;
        let n = self.ixmax;
        let mut cf = vec![0.0_f64; n * n];

        // Scatter row-blocks into the dense matrix (covr.f90:876-879).
        for b in &self.blocks {
            let row0 = b.row - 1;
            for (k, &v) in b.values.iter().enumerate() {
                let col0 = (b.first_col - 1) + k;
                cf[row0 * n + col0] = v;
            }
        }

        // Zero spurious covariances + absolute->relative conversion
        // (covr.f90:895-935). Upstream indexes xx by the row k, xy by column n.
        let mut zeroed = 0usize;
        let mut izero = false;
        for k in 0..n {
            for nn in 0..n {
                let idx = k * n + nn;
                let denom = self.xx[k] * self.xy[nn];
                if denom == 0.0 {
                    // covr.f90:915-926 — spurious covariance in zero-xsec region.
                    if cf[idx] != 0.0 {
                        cf[idx] = 0.0;
                        zeroed += 1;
                    }
                } else {
                    // covr.f90:929 — convert to relative unless already relative.
                    if form == CovarianceForm::Absolute {
                        cf[idx] /= denom;
                    }
                    if cf[idx] != 0.0 {
                        izero = true;
                    }
                }
            }
        }

        let matrix = CovarianceMatrix::from_row_major(n, cf)?;
        Ok(CovardResult {
            matrix,
            zeroed_for_zero_xsec: zeroed,
            is_null: !izero,
        })
    }
}

// ---------------------------------------------------------------------------
// corr's auto/cross rsd sourcing (covr.f90:597-711)
// ---------------------------------------------------------------------------

/// Build a correlation matrix from a **cross-covariance** and the two reactions'
/// **own auto-covariances**, reproducing the standard-deviation sourcing in
/// `subroutine corr` (`covr.f90:597-711`).
///
/// This is the documented human-verify point (see the module docs): the row
/// standard deviations come from `row_auto`'s diagonal and the column standard
/// deviations from `col_auto`'s diagonal — **not** from `cross`'s diagonal —
/// and then `corr(i,j) = cross(i,j) / (rsd_x(i) * rsd_y(j))`.
///
/// For an auto-covariance case pass the same matrix for all three arguments;
/// the result then matches [`CovarianceMatrix::to_correlation`].
///
/// # Errors
/// [`NjoyError::EndfParse`] when the three matrices do not share one group
/// count `n` (the "group structures do not agree" check, `covr.f90:715-716`),
/// via [`correlation_cross`].
pub fn correlation_from_auto_and_cross(
    row_auto: &CovarianceMatrix,
    col_auto: &CovarianceMatrix,
    cross: &CovarianceMatrix,
) -> Result<CorrelationMatrix, NjoyError> {
    if row_auto.n() != cross.n() || col_auto.n() != cross.n() {
        return Err(NjoyError::EndfParse(format!(
            "covard: group structures do not agree (cross={}, row_auto={}, col_auto={})",
            cross.n(),
            row_auto.n(),
            col_auto.n()
        )));
    }
    let rsd_x = row_auto.relative_std_dev(); // covr.f90:658-666
    let rsd_y = col_auto.relative_std_dev(); // covr.f90:636-648
    correlation_cross(cross, &rsd_x, &rsd_y)
}

// ---------------------------------------------------------------------------
// expndo — MT-pair enumeration (covr.f90:508-576)
// ---------------------------------------------------------------------------

/// Enumerate all ENDF-ordered MT-MT1 reaction pairs from the set of MT numbers
/// present, dropping stripped MTs — the pure logic of `subroutine expndo`
/// (`covr.f90:557-569`).
///
/// `present` is the list of MT numbers found on the tape, in the order ERRORR
/// wrote them (ENDF order); `mstrip` is the three-element strip list
/// (`covr.f90:529-531`) applied via [`crate::covr::is_mt_stripped`]. The result
/// is every pair `(mt_i, mt_j)` with `i <= j` over the surviving MTs, in the
/// same order upstream builds `imtx`/`imtx1` (`covr.f90:559-568`): an outer loop
/// over `im`, inner over `jm >= im`.
///
/// The tape-scanning half of `expndo` (`repoz`/`finds`/`listio` collecting the
/// present MTs off unit `nin`, `covr.f90:526-556`) is **not** ported — it needs
/// the ENDF tape I/O layer; callers supply the already-scanned `present` list.
///
/// `present` MT numbers and `mstrip` entries are ENDF MT numbers (dimensionless).
pub fn expand_mt_pairs(present: &[i32], mstrip: [i32; 3]) -> Vec<(i32, i32)> {
    // covr.f90:546-556 — filter out stripped MTs, preserving order.
    let kept: Vec<i32> = present
        .iter()
        .copied()
        .filter(|&mt| !crate::covr::is_mt_stripped(mt, mstrip))
        .collect();

    // covr.f90:559-568 — expand to all (im, jm>=im) combinations.
    let mut pairs = Vec::new();
    for (i, &mt_i) in kept.iter().enumerate() {
        for &mt_j in &kept[i..] {
            pairs.push((mt_i, mt_j));
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the classic 2x2 relative-covariance section as ERRORR would, with
    /// unit cross sections so no conversion/zeroing happens.
    fn section_2x2(v00: f64, v01: f64, v11: f64) -> ErrorrCovarianceSection {
        ErrorrCovarianceSection {
            ixmax: 2,
            xx: vec![1.0, 1.0],
            xy: vec![1.0, 1.0],
            group_boundaries: vec![1.0, 10.0, 100.0],
            blocks: vec![
                CovarianceRowBlock {
                    row: 1,
                    first_col: 1,
                    values: vec![v00, v01],
                },
                CovarianceRowBlock {
                    row: 2,
                    first_col: 2,
                    values: vec![v11],
                },
            ],
        }
    }

    /// Methodology: `covard`'s scatter (`covr.f90:876-879`) must place each
    /// sparse row-block value at `(row-1, first_col-1+k)` in the dense matrix,
    /// filling unspecified entries with zero. Row 1 starts at column 1 with
    /// `[4, 2]`; row 2 starts at column 2 with `[9]` (upper triangle only).
    /// Expected dense (row-major) = `[4, 2, 0, 9]`. Relative form (unit xsec)
    /// leaves values untouched. Result (2026-07-15): exact.
    #[test]
    fn covard_scatters_sparse_blocks_into_dense() {
        let sec = section_2x2(4.0, 2.0, 9.0);
        let out = sec.to_dense(CovarianceForm::Relative).unwrap();
        assert_eq!(out.matrix.get(0, 0), 4.0);
        assert_eq!(out.matrix.get(0, 1), 2.0);
        assert_eq!(out.matrix.get(1, 0), 0.0); // lower triangle unset -> 0
        assert_eq!(out.matrix.get(1, 1), 9.0);
        assert_eq!(out.zeroed_for_zero_xsec, 0);
        assert!(!out.is_null);
    }

    /// Methodology: absolute->relative conversion (`covr.f90:929`),
    /// `cf(k,n) /= xx(k)*xy(n)`. Absolute covariance `[[8, 6], [0, 18]]` with
    /// cross sections `xx = xy = [2, 3]`. Expected relative:
    ///   (0,0)=8/(2*2)=2; (0,1)=6/(2*3)=1; (1,1)=18/(3*3)=2.
    /// Result (2026-07-15): exact to 1e-12.
    #[test]
    fn covard_converts_absolute_to_relative() {
        let mut sec = section_2x2(8.0, 6.0, 18.0);
        sec.xx = vec![2.0, 3.0];
        sec.xy = vec![2.0, 3.0];
        let out = sec.to_dense(CovarianceForm::Absolute).unwrap();
        assert!((out.matrix.get(0, 0) - 2.0).abs() < 1e-12);
        assert!((out.matrix.get(0, 1) - 1.0).abs() < 1e-12);
        assert!((out.matrix.get(1, 1) - 2.0).abs() < 1e-12);
        // Relative form on the same data would NOT divide:
        let rel = sec.to_dense(CovarianceForm::Relative).unwrap();
        assert_eq!(rel.matrix.get(0, 0), 8.0);
    }

    /// Methodology: spurious-covariance zeroing (`covr.f90:915-926`). A group
    /// with zero cross section must have its whole row/column of covariances
    /// zeroed, and the count reported via `ipflag`. Give group 2 a zero cross
    /// section (`xy[1]=0`, `xx[1]=0`); the (0,1) element (value 2) is zeroed.
    /// Row 2's (1,1) is also touched via xx[1]=0. Expected: (0,1)->0 counted;
    /// (1,1)->0 counted; remaining matrix `[[4,0],[0,0]]`.
    /// Result (2026-07-15): 2 zeroed, matrix as expected.
    #[test]
    fn covard_zeros_spurious_covariance_in_zero_xsec() {
        let mut sec = section_2x2(4.0, 2.0, 9.0);
        sec.xx = vec![1.0, 0.0];
        sec.xy = vec![1.0, 0.0];
        let out = sec.to_dense(CovarianceForm::Relative).unwrap();
        assert_eq!(out.matrix.get(0, 0), 4.0);
        assert_eq!(out.matrix.get(0, 1), 0.0);
        assert_eq!(out.matrix.get(1, 1), 0.0);
        assert_eq!(out.zeroed_for_zero_xsec, 2);
        assert!(!out.is_null);
    }

    /// Methodology: null-matrix detection (`izero`, `covr.f90:897,930`). An
    /// all-zero section yields `is_null == true`. Result (2026-07-15): as
    /// expected.
    #[test]
    fn covard_flags_null_matrix() {
        let sec = section_2x2(0.0, 0.0, 0.0);
        let out = sec.to_dense(CovarianceForm::Relative).unwrap();
        assert!(out.is_null);
        assert_eq!(out.zeroed_for_zero_xsec, 0);
    }

    /// Methodology: shape validation — a block whose columns run past `ixmax`
    /// must error (the tape-consistency guard). Result (2026-07-15): EndfParse.
    #[test]
    fn covard_rejects_out_of_range_block() {
        let sec = ErrorrCovarianceSection {
            ixmax: 2,
            xx: vec![1.0, 1.0],
            xy: vec![1.0, 1.0],
            group_boundaries: vec![],
            blocks: vec![CovarianceRowBlock {
                row: 1,
                first_col: 2,
                values: vec![1.0, 2.0], // columns 2..3, but ixmax=2
            }],
        };
        assert!(matches!(
            sec.to_dense(CovarianceForm::Relative),
            Err(NjoyError::EndfParse(_))
        ));
    }

    /// Methodology: the auto-vs-cross rsd sourcing (`covr.f90:597-711`). The row
    /// standard deviations come from `row_auto`'s diagonal, the column ones from
    /// `col_auto`'s diagonal, NOT from the cross matrix. Use distinct diagonals:
    ///   row_auto diag = [4, 9]  -> rsd_x = [2, 3]
    ///   col_auto diag = [9, 16] -> rsd_y = [3, 4]
    ///   cross = [[6, 0], [0, 12]]
    /// Expected corr(0,0)=6/(2*3)=1; corr(1,1)=12/(3*4)=1; off-diagonals 0.
    /// A naive normalisation by the cross matrix's own (zero) diagonal would
    /// divide by zero -> this test pins that the *auto* diagonals are used.
    /// Result (2026-07-15): exact to 1e-12.
    #[test]
    fn correlation_uses_auto_covariance_diagonals_for_rsd() {
        let row_auto = CovarianceMatrix::from_row_major(2, vec![4.0, 0.0, 0.0, 9.0]).unwrap();
        let col_auto = CovarianceMatrix::from_row_major(2, vec![9.0, 0.0, 0.0, 16.0]).unwrap();
        let cross = CovarianceMatrix::from_row_major(2, vec![6.0, 0.0, 0.0, 12.0]).unwrap();
        let corr = correlation_from_auto_and_cross(&row_auto, &col_auto, &cross).unwrap();
        assert!((corr.get(0, 0) - 1.0).abs() < 1e-12);
        assert!((corr.get(1, 1) - 1.0).abs() < 1e-12);
        assert_eq!(corr.get(0, 1), 0.0);
        assert_eq!(corr.get(1, 0), 0.0);
    }

    /// Methodology: for an auto-covariance (row_auto == col_auto == cross), the
    /// sourcing helper must agree with `CovarianceMatrix::to_correlation`.
    /// cov = [[4, 2], [2, 9]] -> corr = [[1, 1/3], [1/3, 1]].
    /// Result (2026-07-15): identical to the correlation module, 1e-12.
    #[test]
    fn correlation_auto_case_matches_to_correlation() {
        let cov = CovarianceMatrix::from_row_major(2, vec![4.0, 2.0, 2.0, 9.0]).unwrap();
        let via_helper = correlation_from_auto_and_cross(&cov, &cov, &cov).unwrap();
        let via_direct = cov.to_correlation();
        for i in 0..2 {
            for j in 0..2 {
                assert!((via_helper.get(i, j) - via_direct.get(i, j)).abs() < 1e-12);
            }
        }
    }

    /// Methodology: mismatched group counts must raise the "group structures do
    /// not agree" error (`covr.f90:715-716`). Result (2026-07-15): EndfParse.
    #[test]
    fn correlation_rejects_mismatched_group_counts() {
        let cross = CovarianceMatrix::from_row_major(2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();
        let row_auto = CovarianceMatrix::from_row_major(1, vec![1.0]).unwrap();
        let col_auto = CovarianceMatrix::from_row_major(2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();
        assert!(matches!(
            correlation_from_auto_and_cross(&row_auto, &col_auto, &cross),
            Err(NjoyError::EndfParse(_))
        ));
    }

    /// Methodology: `expndo`'s pair enumeration (`covr.f90:559-568`) — all
    /// `(im, jm>=im)` combinations in order, no stripping. Present MTs
    /// `[1, 2, 102]` with no strippers (`[0,0,0]`) expand to the 6 upper-triangle
    /// pairs. Result (2026-07-15): exact list.
    #[test]
    fn expndo_enumerates_upper_triangle_pairs() {
        let pairs = expand_mt_pairs(&[1, 2, 102], [0, 0, 0]);
        assert_eq!(
            pairs,
            vec![(1, 1), (1, 2), (1, 102), (2, 2), (2, 102), (102, 102)]
        );
    }

    /// Methodology: stripping in `expndo` (`covr.f90:546-556`). A stripper of
    /// `2` removes MT=2, and (because any active stripper strips MT=1,
    /// `covr.f90:548`) also removes MT=1. Present `[1, 2, 102]`, mstrip
    /// `[2, 0, 0]` -> only MT=102 survives -> single pair `(102, 102)`.
    /// Result (2026-07-15): exact.
    #[test]
    fn expndo_applies_strip_list() {
        let pairs = expand_mt_pairs(&[1, 2, 102], [2, 0, 0]);
        assert_eq!(pairs, vec![(102, 102)]);
    }

    /// Methodology: discrete-level band stripping (`covr.f90:551-552`). A
    /// stripper `s=51` strips every MT in `52..=90`. Present `[51, 52, 90, 102]`,
    /// mstrip `[51, 0, 0]` strips 52 and 90 (both in `51<mt<=90`) and 51 itself
    /// (exact match), leaving `[102]` -> `(102, 102)`.
    /// Result (2026-07-15): exact.
    #[test]
    fn expndo_strips_discrete_level_band() {
        let pairs = expand_mt_pairs(&[51, 52, 90, 102], [51, 0, 0]);
        assert_eq!(pairs, vec![(102, 102)]);
    }
}
