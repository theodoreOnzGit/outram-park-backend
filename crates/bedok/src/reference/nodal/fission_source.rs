//! Fission-source extrapolation for source-iteration acceleration.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `fiss_src_extrapolatexyz.m` (function `fiss_src_extrapolatexyz`).
//!
//! The method is the fixed-weight extrapolation of B. R. Bandini, *A
//! three-dimensional transient neutronics routine for the TRAC-PF1 reactor
//! thermal hydraulic computer code*, PhD thesis, Pennsylvania State University,
//! 1990, p. 51 — cited in the MATLAB header. The dominance ratio is estimated
//! from successive fission-source differences and turned into an extrapolation
//! weight `w = d/(1-d)`.

use super::flux_history::FluxHistory;
use super::sparse::SparseMatrix;

/// What the extrapolation decided to do, for the caller's diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrapolationOutcome {
    /// The flux and fission source were extrapolated.
    Applied,
    /// Skipped: a dominance-ratio denominator was exactly zero, which happens
    /// when the fission source is stagnant (typically the first few
    /// iterations).
    ZeroDenominator,
    /// Skipped: the two successive weight estimates disagreed by more than
    /// 10%, so the iteration is not yet in its asymptotic regime.
    NotAsymptotic,
}

/// Extrapolates the flux and fission source — `fiss_src_extrapolatexyz.m`.
///
/// Reads the four newest columns of `history`, forms their fission sources, and
/// estimates the dominance ratio \[dimensionless, physically in `(0,1)`\] as
///
/// ```text
/// d      = ||fs   - fs_1|| / ||fs_1 - fs_2||
/// d_prev = ||fs_1 - fs_2|| / ||fs_2 - fs_3||
/// ```
///
/// Both are clamped to `[0, 0.99]`, converted to weights `w = d/(1-d)`, and
/// capped at `w = 5`. If the two weights agree to within 10% the current
/// iterate is extrapolated in place:
///
/// ```text
/// fs      <- fs  + w*(fs  - fs_1)
/// phi(:,1) <- phi + w*(phi - phi_1)
/// ```
///
/// Returns the (possibly extrapolated) fission source and what was decided.
/// The history's current column is updated in place; older columns are left
/// alone.
///
/// # Guards, and whose they are
///
/// The clamping, the `w <= 5` cap, the zero-denominator check and the `+1e-14`
/// in the agreement test are **all Yan Ren's own additions**, each with a
/// comment explaining the failure it prevents (`domir >= 1` producing a
/// negative or infinite weight; division by a near-zero weight). They are
/// reproduced exactly, including the asymmetry that `w` is capped but the
/// agreement test then compares the capped values, so two very different
/// uncapped ratios can be judged "asymptotic" once both saturate at 5.
///
/// # Panics
///
/// If the history has fewer than four columns, or its column length does not
/// match the fission operator.
#[must_use]
pub fn extrapolate(
    fission_operator: &SparseMatrix,
    history: &mut FluxHistory,
) -> (Vec<f64>, ExtrapolationOutcome) {
    assert!(
        history.depth() >= 4,
        "fission-source extrapolation reads four flux iterates"
    );

    let phi = history.column(0).to_vec();
    let phi_old = history.column(1).to_vec();
    let phi_older = history.column(2).to_vec();
    let phi_oldest = history.column(3).to_vec();

    let fs_oldest = fission_operator.mul_vec(&phi_oldest);
    let fs_older = fission_operator.mul_vec(&phi_older);
    let fs_old = fission_operator.mul_vec(&phi_old);
    let fs = fission_operator.mul_vec(&phi);

    let norm_diff = norm2_difference(&fs_old, &fs_older);
    let norm_diff_old = norm2_difference(&fs_older, &fs_oldest);

    if norm_diff == 0.0 || norm_diff_old == 0.0 {
        return (fs, ExtrapolationOutcome::ZeroDenominator);
    }

    let domir = norm2_difference(&fs, &fs_old) / norm_diff;
    let domir_old = norm_diff / norm_diff_old;

    let domir_safe = domir.clamp(0.0, 0.99);
    let domir_old_safe = domir_old.clamp(0.0, 0.99);

    let wn_max = 5.0;
    let wn = (domir_safe / (1.0 - domir_safe)).min(wn_max);
    let wn_old = (domir_old_safe / (1.0 - domir_old_safe)).min(wn_max);

    if (wn - wn_old).abs() < 0.1 * wn.abs().max(wn_old.abs()) + 1e-14 {
        let fs_extrapolated: Vec<f64> = fs
            .iter()
            .zip(&fs_old)
            .map(|(&a, &b)| a + wn * (a - b))
            .collect();
        let phi_extrapolated: Vec<f64> = phi
            .iter()
            .zip(&phi_old)
            .map(|(&a, &b)| a + wn * (a - b))
            .collect();
        history.set_current(phi_extrapolated);
        (fs_extrapolated, ExtrapolationOutcome::Applied)
    } else {
        (fs, ExtrapolationOutcome::NotAsymptotic)
    }
}

/// Euclidean norm of `a - b` — MATLAB `norm(a-b)`.
fn norm2_difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identity fission operator, so the fission source *is* the flux and
    /// the arithmetic can be checked by hand.
    fn identity(n: usize) -> SparseMatrix {
        SparseMatrix::identity(n, 1.0)
    }

    #[test]
    fn a_stagnant_source_is_not_extrapolated() {
        let mut h = FluxHistory::filled(2, 5, 1.0);
        let (fs, outcome) = extrapolate(&identity(2), &mut h);
        assert_eq!(outcome, ExtrapolationOutcome::ZeroDenominator);
        assert_eq!(fs, vec![1.0, 1.0]);
        assert_eq!(h.current(), &[1.0, 1.0]);
    }

    #[test]
    fn a_clean_geometric_sequence_is_extrapolated_to_its_limit() {
        // phi_k = 1 + r^k with r = 0.5, newest first: 1.125, 1.25, 1.5, 2.
        // Both dominance-ratio estimates are exactly 0.5, so w = 1 and
        // phi <- 1.125 + 1*(1.125 - 1.25) = 1.0, the exact limit.
        let mut h = FluxHistory::from_columns(vec![
            vec![1.125],
            vec![1.25],
            vec![1.5],
            vec![2.0],
            vec![3.0],
        ]);
        let (fs, outcome) = extrapolate(&identity(1), &mut h);
        assert_eq!(outcome, ExtrapolationOutcome::Applied);
        assert!((h.current()[0] - 1.0).abs() < 1e-14);
        assert!((fs[0] - 1.0).abs() < 1e-14);
    }

    #[test]
    fn a_non_asymptotic_sequence_is_left_alone() {
        // Ratios 0.9 then 0.1: weights 9 (capped to 5) and 1/9, far apart.
        let mut h =
            FluxHistory::from_columns(vec![vec![0.0], vec![0.9], vec![1.0], vec![11.0], vec![0.0]]);
        let before = h.current().to_vec();
        let (_, outcome) = extrapolate(&identity(1), &mut h);
        assert_eq!(outcome, ExtrapolationOutcome::NotAsymptotic);
        assert_eq!(h.current(), &before[..]);
    }

    #[test]
    fn a_diverging_sequence_saturates_the_weight_rather_than_exploding() {
        // Ratios above 1 clamp to 0.99 -> w = 99 -> capped to 5.
        let mut h = FluxHistory::from_columns(vec![
            vec![1000.0],
            vec![100.0],
            vec![10.0],
            vec![1.0],
            vec![0.0],
        ]);
        let (fs, outcome) = extrapolate(&identity(1), &mut h);
        assert_eq!(outcome, ExtrapolationOutcome::Applied);
        // 1000 + 5*(1000 - 100) = 5500, not an overflow.
        assert!((fs[0] - 5500.0).abs() < 1e-9);
    }

    #[test]
    #[should_panic(expected = "four flux iterates")]
    fn a_short_history_is_rejected() {
        let mut h = FluxHistory::filled(1, 3, 1.0);
        let _ = extrapolate(&identity(1), &mut h);
    }
}
