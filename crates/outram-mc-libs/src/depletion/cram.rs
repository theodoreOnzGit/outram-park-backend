//! Chebyshev Rational Approximation Method (CRAM) matrix-exponential solver.
//!
//! **MOVED 2026-10-17 to `petir::depletion::cram` (gh:#950, maintainer
//! direction "All math moved to petir").** The solver, its OpenMC provenance
//! (`openmc/deplete/cram.py`, MIT), the Pusa (2016) coefficients and the
//! analytic tests all live there now, together with upstream's `substeps`
//! option, which this crate's copy lacked.
//!
//! What stays here is the signature callers in this crate use: [`cram16`] and
//! [`cram48`] return the evolved densities directly and **panic** on a
//! length mismatch, exactly as before the move. PETIR's versions return a
//! `Result`. The arithmetic is the same, bit for bit:
//! `tests/depletion_move_fingerprints.rs` pins it against fingerprints printed
//! on the pre-move code.

pub use petir::depletion::cram::{ipf_cram, Complex64, CramSolver, DepSystemSolver, LuFactor};

use super::matrix::DepletionMatrix;

/// Order-16 IPF CRAM, `N(t + dt) = exp(A dt) N(t)` — see
/// `petir::depletion::cram16`.
///
/// Units: `a` in `1/s`, `dt_seconds` in seconds, `n0` in any consistent
/// density unit. `dt_seconds == 0`, a zero-order or an all-zero matrix return
/// `n0` unchanged.
///
/// # Panics
///
/// If `n0.len() != a.order()` (the pre-move contract, kept so callers in this
/// crate are unchanged), or if a pivot is exactly zero (PETIR reports it;
/// before the move it produced non-finite densities silently).
pub fn cram16(a: &DepletionMatrix, n0: &[f64], dt_seconds: f64) -> Vec<f64> {
    match petir::depletion::cram16(a, n0, dt_seconds) {
        Ok(n) => n,
        Err(e) => panic!(
            "cram16 on an order-{} matrix with {} densities: {e}",
            a.order(),
            n0.len()
        ),
    }
}

/// Order-48 IPF CRAM — see `petir::depletion::cram48`. Same contract as
/// [`cram16`].
///
/// # Panics
///
/// As [`cram16`].
pub fn cram48(a: &DepletionMatrix, n0: &[f64], dt_seconds: f64) -> Vec<f64> {
    match petir::depletion::cram48(a, n0, dt_seconds) {
        Ok(n) => n,
        Err(e) => panic!(
            "cram48 on an order-{} matrix with {} densities: {e}",
            a.order(),
            n0.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wrappers are PETIR's functions, unchanged.
    #[test]
    fn wrappers_are_petir() {
        let mut a = DepletionMatrix::zeros(2);
        a.set(0, 0, -1.0e-3).unwrap();
        a.set(1, 0, 1.0e-3).unwrap();
        let n0 = [1.0, 0.0];
        assert_eq!(
            cram16(&a, &n0, 500.0),
            petir::depletion::cram16(&a, &n0, 500.0).unwrap()
        );
        assert_eq!(
            cram48(&a, &n0, 500.0),
            petir::depletion::cram48(&a, &n0, 500.0).unwrap()
        );
    }

    /// A length mismatch still panics, as it did before the move.
    #[test]
    #[should_panic(expected = "cram16 on an order-2 matrix with 1 densities")]
    fn length_mismatch_panics_as_before() {
        cram16(&DepletionMatrix::zeros(2), &[1.0], 1.0);
    }
}
