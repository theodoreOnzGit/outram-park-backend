//! CRAM — the Chebyshev Rational Approximation Method matrix-exponential solver.
//!
//! CRAM computes the action of the matrix exponential
//! `n(Δt) = exp(A·Δt) · n0` — the exact solution of the Bateman depletion
//! system `dn/dt = A·n` for a constant burnup matrix `A` (units `1/s`) over a
//! timestep `Δt` (units `s`). It is the standard high-accuracy depletion solver
//! (Pusa & Leppänen); ONIX uses the order-16 variant in
//! `onix/salameche/cram.py`.
//!
//! ## Method (order-16 partial-fraction form)
//!
//! The rational approximation `r(x) ≈ exp(x)` of order 16 is written as a sum
//! over its 8 conjugate pole pairs:
//!
//! ```text
//!   exp(A·Δt)·n0 ≈ α0·n0 + 2·Re{ Σ_{k=1..8} α_k · (A·Δt − θ_k·I)^{-1} · n0 }
//! ```
//!
//! Each term is one **complex resolvent solve**: `(A·Δt − θ_k·I) y_k = α_k·n0`.
//! Taking twice the real part of the 8 upper-half-plane poles accounts for
//! their complex-conjugate partners. `α0` is the value of the approximation at
//! infinity. This is the algorithm in ONIX `CRAM16`
//! (`onix/salameche/cram.py:6–59`).
//!
//! ## Where the arithmetic lives (since 2026-10-20, gh:#999)
//!
//! The solver itself, coefficients and complex elimination included, is
//! [`petir::depletion::onix_cram::cram16_pfd`]. It was ported here first
//! (2026-08-04) and MOVED to PETIR at the maintainer's direction "The cram
//! from Onix can go to petir and attribute onix"; ONIX's MIT notice and
//! attribution moved with it (PETIR's `NOTICE`, "DERIVED WORK — ONIX", and
//! the file header). [`cram16`] here is a thin wrapper: it copies the
//! [`BurnupMatrix`] into PETIR's `DepletionMatrix` (the same row-major values)
//! and maps PETIR's error onto [`CramError`]. `tests/cram16_move_fingerprints.rs`
//! pins the move bit for bit.
//!
//! It is NOT PETIR's other CRAM16 (`petir::depletion::cram16`, OpenMC's IPF
//! form): the poles agree to within 3 ulps, the residues belong to a
//! different algorithm, and the results agree to round-off only (6.9e-15 on
//! unit inventories). `petir::depletion::onix_cram`'s module docs record the
//! comparison.
//!
//! ## Provenance (MIT upstream)
//!
//! ONIX (open-source, MIT; commit `7328dc6`), `onix/salameche/cram.py`:
//! `CRAM16` (`cram.py:6–59`) for the solver, now in PETIR, and
//! `CRAM_density_check` (`cram.py:127–163`) for [`clamp_nonnegative`], which
//! stays here: it is a physicality filter applied to CRAM's output (with a
//! threshold in atoms·cm⁻³), not part of the approximation. ONIX in turn
//! takes the coefficients from the CRAM literature (Pusa, "Rational
//! Approximations to the Matrix Exponential in Burnup Calculations", *Nucl.
//! Sci. Eng.* 169 (2011) 155–167). Independent Rust re-implementation; this
//! crate is AGPL-3.0-only (MIT is compatible).

use crate::matrix::BurnupMatrix;
use petir::PetirError;
use petir::depletion::DepletionMatrix;

/// Errors raised by the CRAM solver.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CramError {
    /// The initial-inventory vector length did not equal the matrix dimension.
    #[error("inventory length {got} does not match matrix dimension {expected}")]
    DimensionMismatch {
        /// Matrix dimension `n`.
        expected: usize,
        /// Length of the supplied `n0`.
        got: usize,
    },
    /// A complex resolvent `(A·Δt − θ_k·I)` was singular to working precision —
    /// Gaussian elimination found a zero pivot. This should not happen for a
    /// physical depletion matrix (the θ_k sit off the real axis), so it usually
    /// signals a malformed matrix.
    #[error("resolvent matrix is singular at pivot column {column}")]
    SingularResolvent {
        /// The column at which no nonzero pivot could be found.
        column: usize,
    },
    /// Any other error from PETIR's solver. Not expected: PETIR's remaining
    /// checks are on shapes that a [`BurnupMatrix`] always satisfies. Added
    /// 2026-10-20 (gh:#999) so the error mapping is total instead of
    /// relabelling an unexpected error as one of the two above.
    #[error("PETIR CRAM16 failed: {0}")]
    Petir(PetirError),
}

impl From<PetirError> for CramError {
    fn from(e: PetirError) -> Self {
        match e {
            PetirError::LengthMismatch { expected, found } => CramError::DimensionMismatch {
                expected,
                got: found,
            },
            PetirError::Singular { col } => CramError::SingularResolvent { column: col },
            other => CramError::Petir(other),
        }
    }
}

/// Order-16 CRAM: `n(Δt) = exp(A·Δt)·n0`.
///
/// * `a` — burnup matrix `A` (units `1/s`), see [`BurnupMatrix`].
/// * `dt` — timestep Δt in **seconds** (`>= 0`).
/// * `n0` — initial number densities (atoms, or atoms·cm⁻³); length must equal
///   `a.dim()`.
///
/// Returns the depleted inventory `n(Δt)` in the same units as `n0`. Negative
/// entries (a known CRAM artefact for species that should be exactly zero) are
/// **not** clamped here — see [`clamp_nonnegative`] for the optional
/// physicality filter ONIX applies in `CRAM_density_check`
/// (`onix/salameche/cram.py:127`).
///
/// Algorithm: ONIX `CRAM16` (`onix/salameche/cram.py:6–59`), computed by
/// [`petir::depletion::onix_cram::cram16_pfd`] (moved there 2026-10-20,
/// gh:#999, with results bit-identical to the pre-move code).
pub fn cram16(a: &BurnupMatrix, dt: f64, n0: &[f64]) -> Result<Vec<f64>, CramError> {
    let n = a.dim();
    if n0.len() != n {
        return Err(CramError::DimensionMismatch {
            expected: n,
            got: n0.len(),
        });
    }
    let mut m = DepletionMatrix::zeros(n);
    m.as_mut_slice().copy_from_slice(a.as_slice());
    Ok(petir::depletion::cram16_pfd(&m, n0, dt)?)
}

/// Clamp physically-impossible values to zero, mirroring ONIX
/// `CRAM_density_check` (`onix/salameche/cram.py:127–163`).
///
/// CRAM produces small negative densities for species that should be exactly
/// zero (an artefact of the rational approximation); these have no physical
/// meaning. This filter sets any entry `< threshold` (including negatives) to
/// `0.0`. ONIX uses `threshold = 1e-24` atoms·cm⁻³ by default. Pass the
/// threshold in the same units as the inventory. Mutates `n` in place and
/// returns the count of entries zeroed.
pub fn clamp_nonnegative(n: &mut [f64], threshold: f64) -> usize {
    let mut count = 0;
    for v in n.iter_mut() {
        if *v < threshold {
            *v = 0.0;
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::mathf::RealMath;

    /// Verification (implementation-correctness): a single decaying nuclide
    /// `dn/dt = -λ n` has the analytic solution `n(t) = n0·exp(-λt)`. CRAM16
    /// must reproduce it. Methodology: λ = 1e-3 /s, Δt = 500 s, n0 = 1.0;
    /// reference `exp(-0.5)`. Measured on 2026-08-04: the assertion (abs error
    /// < 1e-12) passes with float round-off error only; the full analytic
    /// three-member Bateman tie-out with recorded errors lives in
    /// `tests/vv_bateman.rs`.
    #[test]
    fn single_nuclide_exponential_decay() {
        let mut a = BurnupMatrix::zeros(1);
        a.set(0, 0, -1e-3);
        let n = cram16(&a, 500.0, &[1.0]).unwrap();
        let reference = (-0.5f64).r_exp();
        assert!(
            (n[0] - reference).abs() < 1e-12,
            "got {}, ref {}",
            n[0],
            reference
        );
    }

    #[test]
    fn dimension_mismatch_is_reported() {
        let a = BurnupMatrix::zeros(2);
        let err = cram16(&a, 1.0, &[1.0]).unwrap_err();
        assert_eq!(
            err,
            CramError::DimensionMismatch {
                expected: 2,
                got: 1
            }
        );
    }

    /// PETIR's errors map onto this crate's variants: a singular resolvent
    /// (the first pole's rotation block, see PETIR's
    /// `a_singular_resolvent_is_an_error_not_a_nan`) keeps its column, and
    /// any other PETIR error is carried, not relabelled.
    #[test]
    #[allow(clippy::excessive_precision)]
    fn petir_errors_map_onto_cram_errors() {
        let (re, im) = (-1.0843917078696988026e1, 1.9277446167181652284e1);
        let mut a = BurnupMatrix::zeros(2);
        a.set(0, 0, re);
        a.set(0, 1, -im);
        a.set(1, 0, im);
        a.set(1, 1, re);
        assert_eq!(
            cram16(&a, 1.0, &[1.0, 1.0]).unwrap_err(),
            CramError::SingularResolvent { column: 1 }
        );
        assert_eq!(
            CramError::from(PetirError::Invalid),
            CramError::Petir(PetirError::Invalid)
        );
    }

    #[test]
    fn clamp_zeros_small_and_negative() {
        let mut v = vec![-1e-30, 1e-30, 5.0];
        let c = clamp_nonnegative(&mut v, 1e-24);
        assert_eq!(c, 2);
        assert_eq!(v, vec![0.0, 0.0, 5.0]);
    }
}
