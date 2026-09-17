// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! COVR covariance -> correlation math core.
//!
//! This is the numeric heart of COVR's report path: the transform, inside
//! `subroutine corr` (`covr.f90:578-718`), that turns a multigroup covariance
//! matrix into per-group **relative standard deviations** and a **correlation
//! matrix**. It is fully self-contained — no ENDF tape I/O and no plotting —
//! and is therefore the part of COVR that is verifiable against hand-computed
//! reference cases.
//!
//! # Physics
//!
//! For a covariance matrix `cov(i,j)` over energy groups `i, j`, the relative
//! standard deviation of group `i` is
//!
//! `rsd(i) = sqrt(cov(i,i))`   (`covr.f90:636-641`)
//!
//! and the correlation coefficient between groups `i` and `j` is
//!
//! `corr(i,j) = cov(i,j) / (rsd_x(i) * rsd_y(j))`   (`covr.f90:679-680`)
//!
//! where for an **auto-covariance** (same reaction against itself)
//! `rsd_x == rsd_y == rsd`, giving the familiar
//! `corr(i,j) = cov(i,j) / sqrt(cov(i,i)·cov(j,j))`. For a **cross-covariance**
//! between two different reactions, `rsd_x` is the standard-deviation vector of
//! the row reaction (MAT/MT) and `rsd_y` that of the column reaction
//! (MAT1/MT1) — see [`correlation_cross`].
//!
//! # Units
//!
//! - Covariance values are **dimensionless** when the input is relative
//!   covariances (COVR's normal case); when absolute, they carry the units of
//!   the observable squared. The transform is unit-consistent either way — the
//!   resulting correlations are always dimensionless.
//! - [`Correlation`] values are dimensionless and, for a physically consistent
//!   (positive-semidefinite) covariance matrix, lie in `[-1, 1]`. COVR does
//!   **not** clamp during the transform; the plotting stage clamps to `[-1, 1]`
//!   (`covr.f90:1371-1372`). Use [`CorrelationMatrix::clamped`] to reproduce it.
//! - [`RelativeStdDev`] values are dimensionless fractions (e.g. `0.05` = 5 %).

use crate::NjoyError;

/// A dimensionless correlation coefficient. For a positive-semidefinite
/// covariance it lies in `[-1, 1]`; the raw transform may exceed that for
/// inconsistent data (COVR clamps only when plotting).
pub type Correlation = f64;

/// A dimensionless relative standard deviation (fraction of the mean), equal
/// to `sqrt` of a diagonal covariance element.
pub type RelativeStdDev = f64;

/// A covariance-matrix element — dimensionless for relative covariances, or
/// units of the observable squared for absolute covariances.
pub type CovarianceValue = f64;

// ---------------------------------------------------------------------------
// Covariance matrix
// ---------------------------------------------------------------------------

/// A dense, square multigroup covariance matrix over `n` energy groups.
///
/// Stored **row-major**: element `(i, j)` (0-based, row `i`, column `j`) is at
/// `data[i*n + j]`. This matches the upstream `cf` layout, whose element
/// `(i, j)` (1-based) is at `cf(ixmax*(i-1)+j)` (`covr.f90:679`).
///
/// The matrix may hold either absolute or relative covariances (see
/// [`crate::covr::CovarianceForm`]); the transform below is agnostic to which.
#[derive(Debug, Clone, PartialEq)]
pub struct CovarianceMatrix {
    n: usize,
    data: Vec<CovarianceValue>,
}

impl CovarianceMatrix {
    /// Build from a row-major `n*n` buffer.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when `data.len() != n*n`.
    pub fn from_row_major(n: usize, data: Vec<CovarianceValue>) -> Result<Self, NjoyError> {
        if data.len() != n * n {
            return Err(NjoyError::EndfParse(format!(
                "covr: covariance buffer has {} elements, expected {} (n={n})",
                data.len(),
                n * n
            )));
        }
        Ok(Self { n, data })
    }

    /// Number of energy groups (matrix dimension `n`, upstream `ixmax`).
    pub fn n(&self) -> usize {
        self.n
    }

    /// Element `(i, j)` (0-based). Panics if out of range — callers hold `n`.
    pub fn get(&self, i: usize, j: usize) -> CovarianceValue {
        self.data[i * self.n + j]
    }

    /// The diagonal `cov(i,i)` for `i in 0..n` (the group variances).
    pub fn diagonal(&self) -> Vec<CovarianceValue> {
        (0..self.n).map(|i| self.get(i, i)).collect()
    }

    /// Per-group relative standard deviations `rsd(i) = sqrt(cov(i,i))`
    /// (`covr.f90:636-641`).
    ///
    /// A non-positive diagonal element yields `0.0`, exactly as upstream
    /// (`if (cf(i,i).gt.0) ... else rsd=0`, `covr.f90:636-640`). Dimensionless.
    pub fn relative_std_dev(&self) -> Vec<RelativeStdDev> {
        (0..self.n)
            .map(|i| {
                let d = self.get(i, i);
                if d > 0.0 {
                    d.sqrt()
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// True when every element is exactly zero (`izero == 0`, `covr.f90:930`).
    /// COVR suppresses output/plots for a null covariance matrix.
    pub fn is_null(&self) -> bool {
        self.data.iter().all(|&v| v == 0.0)
    }

    /// Convert an **auto-covariance** matrix (a reaction against itself) to a
    /// correlation matrix (`covr.f90:672-688` with `rsd_x == rsd_y`).
    ///
    /// This is the classic
    /// `corr(i,j) = cov(i,j) / sqrt(cov(i,i)·cov(j,j))`. Where either
    /// standard deviation is zero, or `cov(i,j)` is exactly zero, the
    /// correlation is set to `0.0` (mirroring the `cf!=0 .and. rsd*rsd!=0`
    /// guard at `covr.f90:677-685`). The diagonal is therefore `1.0` for every
    /// group with a positive variance, and `0.0` otherwise.
    pub fn to_correlation(&self) -> CorrelationMatrix {
        let rsd = self.relative_std_dev();
        // Auto-covariance: row and column standard deviations coincide.
        let data = correlate(self.n, &self.data, &rsd, &rsd);
        CorrelationMatrix { n: self.n, data }
    }
}

// ---------------------------------------------------------------------------
// Correlation matrix
// ---------------------------------------------------------------------------

/// A dense, square correlation matrix over `n` energy groups, row-major.
///
/// Values are dimensionless [`Correlation`] coefficients. Produced by
/// [`CovarianceMatrix::to_correlation`] or [`correlation_cross`].
#[derive(Debug, Clone, PartialEq)]
pub struct CorrelationMatrix {
    n: usize,
    data: Vec<Correlation>,
}

impl CorrelationMatrix {
    /// Number of energy groups (matrix dimension `n`).
    pub fn n(&self) -> usize {
        self.n
    }

    /// Element `(i, j)` (0-based). Panics if out of range.
    pub fn get(&self, i: usize, j: usize) -> Correlation {
        self.data[i * self.n + j]
    }

    /// A copy with every coefficient clamped to `[-1, 1]`, reproducing the
    /// plot-stage clamp `if (cof.gt.1) cof=1; if (cof.lt.-1) cof=-1`
    /// (`covr.f90:1371-1372`). For a consistent covariance this is a no-op.
    pub fn clamped(&self) -> Self {
        let data = self.data.iter().map(|&c| c.clamp(-1.0, 1.0)).collect();
        Self { n: self.n, data }
    }

    /// The largest correlation magnitude anywhere in the matrix, `max |corr|`.
    /// (For an auto-covariance this is `1.0` whenever any group has variance.)
    pub fn max_abs(&self) -> f64 {
        self.data.iter().fold(0.0_f64, |m, &c| m.max(c.abs()))
    }

    /// The COVR "significance" test that decides whether a correlation matrix
    /// is worth plotting: `true` if any `|corr(i,j)| >= xlev_first`
    /// (`ismall`, `covr.f90:683`). `xlev_first` is the lowest shade level,
    /// `xlev[0]` from [`crate::covr::PlotOptions::shade_levels`].
    pub fn has_plottable_correlation(&self, xlev_first: f64) -> bool {
        self.data.iter().any(|&c| c.abs() >= xlev_first)
    }
}

// ---------------------------------------------------------------------------
// Cross-reaction correlation
// ---------------------------------------------------------------------------

/// Convert a **cross-covariance** between two different reactions to a
/// correlation matrix (`covr.f90:672-688`, general case).
///
/// `cov_xy(i,j)` is the covariance between group `i` of the row reaction
/// (MAT/MT) and group `j` of the column reaction (MAT1/MT1). `rsd_x` is the
/// standard-deviation vector of the row reaction (from its own auto-covariance
/// diagonal) and `rsd_y` that of the column reaction. The result is
///
/// `corr(i,j) = cov_xy(i,j) / (rsd_x(i) * rsd_y(j))`,
///
/// set to `0.0` wherever `cov_xy(i,j)` is exactly zero or `rsd_x(i)*rsd_y(j)`
/// is zero. Unlike the auto-covariance case, the diagonal is **not** generally
/// `1.0`.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `rsd_x.len()` or `rsd_y.len()` differs from
/// `cov_xy.n()` (the "group structures do not agree" check, `covr.f90:715-716`).
pub fn correlation_cross(
    cov_xy: &CovarianceMatrix,
    rsd_x: &[RelativeStdDev],
    rsd_y: &[RelativeStdDev],
) -> Result<CorrelationMatrix, NjoyError> {
    let n = cov_xy.n();
    if rsd_x.len() != n || rsd_y.len() != n {
        return Err(NjoyError::EndfParse(format!(
            "covr: group structures do not agree (n={n}, rsd_x={}, rsd_y={})",
            rsd_x.len(),
            rsd_y.len()
        )));
    }
    let data = correlate(n, &cov_xy.data, rsd_x, rsd_y);
    Ok(CorrelationMatrix { n, data })
}

/// Core transform shared by the auto- and cross-covariance paths
/// (`covr.f90:675-687`). Row-major `n*n`.
fn correlate(
    n: usize,
    cov: &[CovarianceValue],
    rsd_x: &[RelativeStdDev],
    rsd_y: &[RelativeStdDev],
) -> Vec<Correlation> {
    let mut out = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            let c = cov[i * n + j];
            let denom = rsd_x[i] * rsd_y[j];
            // covr.f90:677-685 — guard on exact-zero numerator and denominator.
            if c != 0.0 && denom != 0.0 {
                out[i * n + j] = c / denom;
            } else {
                out[i * n + j] = 0.0;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Shade-level indexing
// ---------------------------------------------------------------------------

/// Index a correlation value `c` on the shade scale `xlev`
/// (`integer function level`, `covr.f90:1601-1619`).
///
/// Returns a **signed 1-based** level: the smallest 1-based index `i` such that
/// `|c| < xlev[i-1]`, or `nlev` (= `xlev.len()`) if `|c|` is at least every
/// threshold; the sign follows the sign of `c` (`covr.f90:1616`). Used by the
/// plotting stage to pick a shade; exposed here because it is self-contained
/// and testable.
///
/// `xlev` is the expanded, ascending shade-level array (dimensionless
/// correlation thresholds) from [`crate::covr::PlotOptions::shade_levels`].
/// Panics if `xlev` is empty.
pub fn shade_level(c: Correlation, xlev: &[f64]) -> i32 {
    assert!(!xlev.is_empty(), "covr::shade_level: empty xlev");
    let nlev = xlev.len();
    let mut ilev = nlev; // 1-based; stays nlev if never exits early.
    for (i, &x) in xlev.iter().enumerate() {
        ilev = i + 1;
        if c.abs() < x {
            break;
        }
    }
    if c < 0.0 {
        -(ilev as i32)
    } else {
        ilev as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: relative standard deviations are `sqrt` of the diagonal
    /// (`covr.f90:636-641`), with non-positive diagonals mapped to 0. Build a
    /// diagonal covariance `diag(4, 9, 16, -1, 0)`; expected rsd = `[2,3,4,0,0]`.
    /// Result (2026-07-15): exact match to 1e-12.
    #[test]
    fn rsd_is_sqrt_of_diagonal() {
        let n = 5;
        let mut data = vec![0.0; n * n];
        for (i, v) in [4.0, 9.0, 16.0, -1.0, 0.0].into_iter().enumerate() {
            data[i * n + i] = v;
        }
        let cov = CovarianceMatrix::from_row_major(n, data).unwrap();
        let rsd = cov.relative_std_dev();
        let expect = [2.0, 3.0, 4.0, 0.0, 0.0];
        for (a, b) in rsd.iter().zip(expect.iter()) {
            assert!((a - b).abs() < 1e-12, "rsd {a} vs {b}");
        }
    }

    /// Methodology: cov -> corr on a known 2x2 SPD matrix.
    /// cov = [[4, 2], [2, 9]] (SPD: det = 32 > 0). rsd = [2, 3].
    /// Hand result: corr = [[1, 2/6], [2/6, 1]] = [[1, 1/3], [1/3, 1]].
    /// Checks: diagonal == 1, off-diagonal == 1/3 exactly, |corr| <= 1,
    /// symmetry preserved. Result (2026-07-15): all pass to 1e-12.
    #[test]
    fn cov_to_corr_2x2_spd() {
        let cov = CovarianceMatrix::from_row_major(2, vec![4.0, 2.0, 2.0, 9.0]).unwrap();
        let corr = cov.to_correlation();
        assert!((corr.get(0, 0) - 1.0).abs() < 1e-12);
        assert!((corr.get(1, 1) - 1.0).abs() < 1e-12);
        let off = 1.0 / 3.0;
        assert!((corr.get(0, 1) - off).abs() < 1e-12);
        assert!((corr.get(1, 0) - off).abs() < 1e-12);
        // symmetry
        assert!((corr.get(0, 1) - corr.get(1, 0)).abs() < 1e-15);
        // bounded
        assert!(corr.max_abs() <= 1.0 + 1e-12);
    }

    /// Methodology: cov -> corr on a known 3x3 SPD matrix with a mix of signs
    /// and an exact zero.
    /// cov = [[4, 2, 0], [2, 9, -3], [0, -3, 16]]. rsd = [2, 3, 4].
    /// Hand results:
    ///   corr(0,1) = 2/(2*3)   = 1/3
    ///   corr(1,2) = -3/(3*4)  = -0.25
    ///   corr(0,2) = 0/(2*4)   = 0
    /// Diagonal == 1; matrix symmetric; every |corr| <= 1.
    /// SPD check: leading minors 4 > 0, 32 > 0, det = 4*(9*16-9) - 2*(2*16) =
    /// 4*135 - 64 = 476 > 0. Result (2026-07-15): all pass to 1e-12.
    #[test]
    fn cov_to_corr_3x3_spd() {
        let data = vec![4.0, 2.0, 0.0, 2.0, 9.0, -3.0, 0.0, -3.0, 16.0];
        let cov = CovarianceMatrix::from_row_major(3, data).unwrap();
        let corr = cov.to_correlation();

        for i in 0..3 {
            assert!((corr.get(i, i) - 1.0).abs() < 1e-12, "diag {i}");
        }
        assert!((corr.get(0, 1) - 1.0 / 3.0).abs() < 1e-12);
        assert!((corr.get(1, 2) + 0.25).abs() < 1e-12);
        assert!(corr.get(0, 2).abs() < 1e-15);

        // symmetry preserved
        for i in 0..3 {
            for j in 0..3 {
                assert!((corr.get(i, j) - corr.get(j, i)).abs() < 1e-15);
            }
        }
        // bounded in [-1, 1]
        for i in 0..3 {
            for j in 0..3 {
                assert!(corr.get(i, j).abs() <= 1.0 + 1e-12);
            }
        }
    }

    /// Methodology: a group with zero variance must yield a zero row/column of
    /// correlations (the `rsd*rsd != 0` guard, `covr.f90:677-685`), not a NaN
    /// from dividing by sqrt(0). cov = [[4, 1], [1, 0]]; group 1 has zero
    /// variance. Expected corr = [[1, 0], [0, 0]] (no NaN anywhere).
    /// Result (2026-07-15): matches; all finite.
    #[test]
    fn zero_variance_group_gives_zero_not_nan() {
        let cov = CovarianceMatrix::from_row_major(2, vec![4.0, 1.0, 1.0, 0.0]).unwrap();
        let corr = cov.to_correlation();
        assert!((corr.get(0, 0) - 1.0).abs() < 1e-12);
        assert_eq!(corr.get(0, 1), 0.0);
        assert_eq!(corr.get(1, 0), 0.0);
        assert_eq!(corr.get(1, 1), 0.0);
        for i in 0..2 {
            for j in 0..2 {
                assert!(corr.get(i, j).is_finite());
            }
        }
    }

    /// Methodology: null-matrix detection (`izero`, `covr.f90:930`). An
    /// all-zero matrix is null; any nonzero element makes it non-null.
    /// Result (2026-07-15): as expected.
    #[test]
    fn null_matrix_detection() {
        let zero = CovarianceMatrix::from_row_major(2, vec![0.0; 4]).unwrap();
        assert!(zero.is_null());
        let nonzero = CovarianceMatrix::from_row_major(2, vec![0.0, 0.0, 0.0, 1e-30]).unwrap();
        assert!(!nonzero.is_null());
    }

    /// Methodology: cross-reaction correlation (`covr.f90:679-680`, general).
    /// Take a cross-covariance cov_xy = [[6, 0], [0, 12]] with distinct
    /// standard deviations rsd_x = [2, 3], rsd_y = [3, 4].
    /// Hand results: corr(0,0) = 6/(2*3) = 1; corr(1,1) = 12/(3*4) = 1;
    /// off-diagonals zero. Also verify a mismatched-length error.
    /// Result (2026-07-15): matches to 1e-12; length mismatch errors.
    #[test]
    fn cross_reaction_correlation() {
        let cov_xy = CovarianceMatrix::from_row_major(2, vec![6.0, 0.0, 0.0, 12.0]).unwrap();
        let corr = correlation_cross(&cov_xy, &[2.0, 3.0], &[3.0, 4.0]).unwrap();
        assert!((corr.get(0, 0) - 1.0).abs() < 1e-12);
        assert!((corr.get(1, 1) - 1.0).abs() < 1e-12);
        assert_eq!(corr.get(0, 1), 0.0);
        assert_eq!(corr.get(1, 0), 0.0);

        assert!(correlation_cross(&cov_xy, &[2.0], &[3.0, 4.0]).is_err());
    }

    /// Methodology: the plot-stage clamp (`covr.f90:1371-1372`) maps values
    /// outside [-1, 1] onto the boundary. Feed a fabricated inconsistent
    /// correlation matrix [[1.4, -1.2], [-1.2, 1.0]] and confirm clamping.
    /// Result (2026-07-15): 1.4 -> 1.0, -1.2 -> -1.0, 1.0 unchanged.
    #[test]
    fn clamp_matches_plot_stage() {
        let m = CorrelationMatrix {
            n: 2,
            data: vec![1.4, -1.2, -1.2, 1.0],
        };
        let c = m.clamped();
        assert_eq!(c.get(0, 0), 1.0);
        assert_eq!(c.get(0, 1), -1.0);
        assert_eq!(c.get(1, 0), -1.0);
        assert_eq!(c.get(1, 1), 1.0);
    }

    /// Methodology: reproduce `integer function level` (`covr.f90:1601-1619`)
    /// against the default expanded shade array
    /// xlev = [0.001, 0.1, 0.2, 0.3, 0.6, 1.001] (nlev = 6).
    /// Hand results:
    ///   c =  0.05 -> smallest i with 0.05 < xlev[i-1] is i=2  -> +2
    ///   c = -0.05 ->                                              -> -2
    ///   c =  0.5  -> 0.5 < 0.6 at i=5                          -> +5
    ///   c =  2.0  -> exceeds all thresholds                    -> +6 (= nlev)
    ///   c =  0.0005 -> < 0.001 at i=1                          -> +1
    /// Result (2026-07-15): exact.
    #[test]
    fn shade_level_indexing() {
        let xlev = [0.001, 0.1, 0.2, 0.3, 0.6, 1.001];
        assert_eq!(shade_level(0.05, &xlev), 2);
        assert_eq!(shade_level(-0.05, &xlev), -2);
        assert_eq!(shade_level(0.5, &xlev), 5);
        assert_eq!(shade_level(2.0, &xlev), 6);
        assert_eq!(shade_level(0.0005, &xlev), 1);
    }

    /// Methodology: the plottability test `ismall` (`covr.f90:683`) fires when
    /// any |corr| >= xlev[0]. For the 3x3 SPD case above, max |corr| = 1, so
    /// with xlev[0] = 0.001 it is plottable; with xlev[0] = 2.0 it is not.
    /// Result (2026-07-15): as expected.
    #[test]
    fn plottability_significance_test() {
        let data = vec![4.0, 2.0, 0.0, 2.0, 9.0, -3.0, 0.0, -3.0, 16.0];
        let corr = CovarianceMatrix::from_row_major(3, data)
            .unwrap()
            .to_correlation();
        assert!(corr.has_plottable_correlation(0.001));
        assert!(!corr.has_plottable_correlation(2.0));
    }
}
