//! CRAM — the Chebyshev Rational Approximation Method matrix-exponential solver.
//!
//! CRAM computes the action of the matrix exponential
//! `n(Δt) = exp(A·Δt) · n0` — the exact solution of the Bateman depletion
//! system `dn/dt = A·n` for a constant burnup matrix `A` (units `1/s`) over a
//! timestep `Δt` (units `s`). It is the standard high-accuracy depletion solver
//! (Pusa & Leppänen); ONIX uses the order-16 variant in
//! `onix/salameche/cram.py`.
//!
//! ## Method (order-16 partial-fraction / incomplete-pole form)
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
//! infinity. This is verbatim the algorithm in ONIX `CRAM16`
//! (`onix/salameche/cram.py:6–59`).
//!
//! ## Provenance (GPLv3 relicensing of MIT upstream)
//!
//! The θ_k, α_k, and α0 coefficients below are copied **digit-for-digit** from
//! ONIX (open-source, MIT; commit `7328dc6`), `onix/salameche/cram.py`:
//!   * `theta` array — `cram.py:22–30`,
//!   * `alpha_0`      — `cram.py:32`,
//!   * `alpha` array  — `cram.py:34–42`,
//!   * solve loop     — `cram.py:44–59`.
//!
//! ONIX in turn takes these from the CRAM literature (Pusa, "Rational
//! Approximations to the Matrix Exponential in Burnup Calculations", *Nucl.
//! Sci. Eng.* 169 (2011) 155–167). ONIX stores them at `complex256` precision;
//! we hold `f64`, which is the working precision of the solve. Independent Rust
//! re-implementation; OUTRAM PARK fork relicenses under **GPL-3.0-only** (MIT is
//! GPL-3.0-compatible).

use crate::matrix::BurnupMatrix;
use num_complex::Complex64;

/// The 8 upper-half-plane poles θ_k of the order-16 CRAM approximation.
///
/// Dimensionless (poles of the rational function). Copied from ONIX
/// `onix/salameche/cram.py:22–30`.
///
/// `excessive_precision` is intentionally allowed: ONIX stores these at
/// `complex256` precision and we reproduce the digits **verbatim** for faithful
/// provenance. `f64` silently rounds them to the nearest representable value —
/// truncating the literals ourselves would break the digit-for-digit citation
/// without changing the stored value.
#[allow(clippy::excessive_precision)]
const CRAM16_THETA: [Complex64; 8] = [
    Complex64::new(-1.0843917078696988026e1, 1.9277446167181652284e1),
    Complex64::new(-5.2649713434426468895, 1.6220221473167927305e1),
    Complex64::new(5.9481522689511774808, 3.5874573620183222829),
    Complex64::new(3.5091036084149180974, 8.4361989858843750826),
    Complex64::new(6.4161776990994341923, 1.1941223933701386874),
    Complex64::new(1.4193758971856659786, 1.0925363484496722585e1),
    Complex64::new(4.9931747377179963991, 5.9968817136039422260),
    Complex64::new(-1.4139284624888862114, 1.3497725698892745389e1),
];

/// The 8 residues α_k of the order-16 CRAM approximation.
///
/// Dimensionless. Copied from ONIX `onix/salameche/cram.py:34–42`. See the
/// `#[allow(clippy::excessive_precision)]` rationale on `CRAM16_THETA`.
#[allow(clippy::excessive_precision)]
const CRAM16_ALPHA: [Complex64; 8] = [
    Complex64::new(-5.0901521865224915650e-7, -2.4220017652852287970e-5),
    Complex64::new(2.1151742182466030907e-4, 4.3892969647380673918e-3),
    Complex64::new(1.1339775178483930527e2, 1.0194721704215856450e2),
    Complex64::new(1.5059585270023467528e1, -5.7514052776421819979),
    Complex64::new(-6.4500878025539646595e1, -2.2459440762652096056e2),
    Complex64::new(-1.4793007113557999718, 1.7686588323782937906),
    Complex64::new(-6.2518392463207918892e1, -1.1190391094283228480e1),
    Complex64::new(4.1023136835410021273e-2, -1.5743466173455468191e-1),
];

/// The limit-at-infinity coefficient α0 of the order-16 CRAM approximation.
///
/// Dimensionless. Copied from ONIX `onix/salameche/cram.py:32`. See the
/// `#[allow(clippy::excessive_precision)]` rationale on `CRAM16_THETA`.
#[allow(clippy::excessive_precision)]
const CRAM16_ALPHA0: f64 = 2.1248537104952237488e-16;

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
}

/// Solve `M·x = b` for complex `M` (row-major, `n×n`) and complex `b`, by dense
/// Gaussian elimination with partial pivoting. Pure Rust, no BLAS.
///
/// Units are carried by the caller; this is a bare linear solve. Returns
/// [`CramError::SingularResolvent`] if a column has no nonzero pivot.
fn complex_solve(
    m: &mut [Complex64],
    b: &mut [Complex64],
    n: usize,
) -> Result<Vec<Complex64>, CramError> {
    // Forward elimination with partial pivoting on |pivot|.
    for col in 0..n {
        // Find the row (>= col) with the largest-magnitude entry in `col`.
        let mut pivot_row = col;
        let mut best = m[col * n + col].norm();
        for r in (col + 1)..n {
            let mag = m[r * n + col].norm();
            if mag > best {
                best = mag;
                pivot_row = r;
            }
        }
        if best == 0.0 {
            return Err(CramError::SingularResolvent { column: col });
        }
        // Swap rows `col` and `pivot_row` in both M and b.
        if pivot_row != col {
            for c in 0..n {
                m.swap(col * n + c, pivot_row * n + c);
            }
            b.swap(col, pivot_row);
        }
        // Eliminate below.
        let pivot = m[col * n + col];
        for r in (col + 1)..n {
            let factor = m[r * n + col] / pivot;
            if factor == Complex64::new(0.0, 0.0) {
                continue;
            }
            for c in col..n {
                let v = m[col * n + c];
                m[r * n + c] -= factor * v;
            }
            let bc = b[col];
            b[r] -= factor * bc;
        }
    }
    // Back substitution.
    let mut x = vec![Complex64::new(0.0, 0.0); n];
    for i in (0..n).rev() {
        let mut acc = b[i];
        for j in (i + 1)..n {
            acc -= m[i * n + j] * x[j];
        }
        x[i] = acc / m[i * n + i];
    }
    Ok(x)
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
/// Algorithm ported from ONIX `CRAM16` (`onix/salameche/cram.py:6–59`).
pub fn cram16(a: &BurnupMatrix, dt: f64, n0: &[f64]) -> Result<Vec<f64>, CramError> {
    let n = a.dim();
    if n0.len() != n {
        return Err(CramError::DimensionMismatch {
            expected: n,
            got: n0.len(),
        });
    }

    // At = A * dt (dimensionless). Build once as a complex working matrix base.
    let src = a.as_slice();

    // Accumulator for Σ_k (A·Δt − θ_k I)^{-1} (α_k n0), summed over 8 poles.
    let mut acc = vec![Complex64::new(0.0, 0.0); n];

    for k in 0..8 {
        // term1 = At - theta_k * I  (row-major complex copy)
        let mut m = vec![Complex64::new(0.0, 0.0); n * n];
        for idx in 0..(n * n) {
            m[idx] = Complex64::new(src[idx] * dt, 0.0);
        }
        for d in 0..n {
            m[d * n + d] -= CRAM16_THETA[k];
        }
        // term2 = alpha_k * n0
        let mut rhs: Vec<Complex64> = n0
            .iter()
            .map(|&v| CRAM16_ALPHA[k] * Complex64::new(v, 0.0))
            .collect();

        let y = complex_solve(&mut m, &mut rhs, n)?;
        for i in 0..n {
            acc[i] += y[i];
        }
    }

    // N = 2*Re(acc) + alpha_0 * n0   (ONIX cram.py:53–54)
    let out = (0..n)
        .map(|i| 2.0 * acc[i].re + CRAM16_ALPHA0 * n0[i])
        .collect();
    Ok(out)
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
        let reference = (-0.5f64).exp();
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

    #[test]
    fn clamp_zeros_small_and_negative() {
        let mut v = vec![-1e-30, 1e-30, 5.0];
        let c = clamp_nonnegative(&mut v, 1e-24);
        assert_eq!(c, 2);
        assert_eq!(v, vec![0.0, 0.0, 5.0]);
    }
}
