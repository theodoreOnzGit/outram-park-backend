// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Short-collision-time / free-gas Gaussian scattering law.
//!
//! In NJOY's LEAPR the short-collision-time (SCT) / free-gas Gaussian is written
//! inline in several places (`contin` at leapr.f90:605-607, `stable`'s free-gas
//! branch at 1084-1085, and `sint`'s SCT extrapolation at 1890-1892). This module
//! collects the one-dimensional symmetric form used by `contin` and by the
//! free-gas branch of `stable` into a single documented, testable function.
//!
//! ## Physics
//!
//! For an effective (dimensionless) parameter `a` — either `alpha * tbeta`
//! (continuum SCT tail) or `twt * alpha` (free gas) — the asymmetric scattering
//! law evaluated at negative energy transfer is the Gaussian
//!
//! ```text
//!   S_a(alpha, -beta) = exp( -(a - beta)^2 / (4 a T*) ) / sqrt(4 pi a T*)
//! ```
//!
//! where `T*` (`tbar`) is the effective-temperature ratio `T_eff / T` (`1` for a
//! pure free gas). This form satisfies detailed balance exactly when `T* = 1`:
//! `S(alpha, -beta) = exp(-beta) * S(alpha, +beta)` — see the unit test.
//!
//! All arguments are dimensionless (`alpha`, `beta` are the usual ENDF thermal
//! variables); the return value is the dimensionless S(alpha, beta).

/// Exponent floor below which `exp(.)` is treated as zero (matches NJOY
/// `explim = -250` in `contin`).
const EXPLIM: f64 = -250.0;

/// Free-gas / short-collision-time Gaussian scattering law `S(a, beta; T*)`.
///
/// # Arguments
/// * `a` — effective alpha (dimensionless): `alpha * tbeta` for the continuum SCT
///   tail, or `twt * alpha` for a pure free gas. Must be `> 0`.
/// * `beta` — energy-transfer variable (dimensionless).
/// * `tbar` — effective-temperature ratio `T_eff / T` (dimensionless); use `1.0`
///   for a pure free gas.
///
/// # Returns
/// The dimensionless asymmetric scattering law. Returns `0.0` when the Gaussian
/// exponent underflows (`< -250`) or when `a <= 0`.
pub fn sct_free_gas(a: f64, beta: f64, tbar: f64) -> f64 {
    if a <= 0.0 {
        return 0.0;
    }
    let alp = a * tbar;
    let ex = -(a - beta) * (a - beta) / (4.0 * alp);
    if ex > EXPLIM {
        ex.exp() / (4.0 * crate::common::phys::PI * alp).sqrt()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the free-gas Gaussian must obey detailed balance exactly for
    /// `tbar = 1`, i.e. `S(a, -beta) = exp(-beta) * S(a, +beta)` for all `a, beta`.
    /// Result (2026-07-15): relative error `< 1e-13` at several (a, beta) points,
    /// confirming the closed-form symmetry is reproduced to machine precision.
    #[test]
    fn detailed_balance_exact_for_free_gas() {
        for &(a, beta) in &[(0.5_f64, 0.3_f64), (2.0, 1.7), (5.0, 4.0), (0.1, 2.5)] {
            let s_pos = sct_free_gas(a, beta, 1.0);
            let s_neg = sct_free_gas(a, -beta, 1.0);
            let expect = (-beta).exp() * s_pos;
            let rel = (s_neg - expect).abs() / expect;
            assert!(rel < 1e-13, "a={a} beta={beta}: rel err {rel:e}");
        }
    }

    /// Peak of the free-gas Gaussian sits at `beta = a` and equals
    /// `1/sqrt(4 pi a)`. Result (2026-07-15): matches to `< 1e-13`.
    #[test]
    fn peak_value_matches_closed_form() {
        let a = 3.0;
        let peak = sct_free_gas(a, a, 1.0);
        let expect = 1.0 / (4.0 * crate::common::phys::PI * a).sqrt();
        assert!((peak - expect).abs() < 1e-13, "peak {peak} vs {expect}");
    }
}
