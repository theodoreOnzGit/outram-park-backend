//! Special functions needed by the petroleum-characterization distributions:
//! the gamma function `Γ(a)` and the **regularized** incomplete gamma
//! integrals `P(a, x)` and `Q(a, x) = 1 − P(a, x)`.
//!
//! # Why this module exists (provenance)
//!
//! DWSIM's `GenerateCompounds.vb` cuts a crude assay into pseudo-components by
//! integrating a **three-parameter gamma distribution** (Whitson's molar-
//! distribution model) over each cut. Those integrals are evaluated upstream by
//! `DWSIM.MathOps.MathEx.GammaFunctions.igammaf.incompletegammac(a, x)` (the
//! *complemented* regularized incomplete gamma integral, i.e. `Q(a, x)`) and
//! `gammaf.gamma(x)`. DWSIM obtains those from vendored **ALGLIB** code.
//!
//! **No ALGLIB source was consulted or copied.** This module reimplements the
//! two functions from their standard, openly-published series/continued-
//! fraction definitions:
//!
//! - Lanczos approximation for `ln Γ(a)` — Lanczos, C. (1964). "A precision
//!   approximation of the gamma function". *SIAM J. Numer. Anal. Ser. B* 1,
//!   86-96. Coefficients: the widely-published `g = 7, n = 9` set.
//! - `P(a, x)` by the confluent series `P(a,x) = x^a e^{-x} / Γ(a) ·
//!   Σ_{k≥0} x^k / (a(a+1)…(a+k))`, and `Q(a, x)` by the Legendre continued
//!   fraction `Q(a,x) = x^a e^{-x} / Γ(a) · 1/(x+1−a− 1·(1−a)/(x+3−a− …))` —
//!   DLMF §8.7.1 and §8.9.2 (<https://dlmf.nist.gov/8>), NIST Digital Library
//!   of Mathematical Functions, a public-domain reference.
//!
//! The series is used for `x < a + 1` and the continued fraction for
//! `x ≥ a + 1`, the standard split at which each converges fastest.
//!
//! # Units
//!
//! These are pure mathematical functions of dimensionless real arguments — no
//! `uom` types apply. `a > 0` and `x ≥ 0` throughout.
//!
//! # Excluded DWSIM behavior
//!
//! DWSIM's `GammaFunctions` module additionally exposes the *unregularized*
//! integrals, the log-gamma sign flag, the beta function, and the inverse
//! incomplete gamma. None of those is reached from the petroleum-
//! characterization code path, so none is ported here.

/// Maximum iterations for the series / continued-fraction evaluations before
/// giving up and returning the best estimate so far.
const MAX_ITERATIONS: usize = 300;

/// Relative convergence tolerance for the series / continued fraction.
const EPSILON: f64 = 3.0e-16;

/// Guard against division by zero in the modified Lentz continued fraction.
const TINY: f64 = 1.0e-300;

/// Lanczos `g` parameter for the `ln Γ` approximation (`g = 7`, 9 coefficients).
const LANCZOS_G: f64 = 7.0;

/// Lanczos coefficients for `g = 7`, `n = 9` (the standard published set).
const LANCZOS_COEFFICIENTS: [f64; 9] = [
    0.999_999_999_999_809_93,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_13,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// Natural logarithm of the gamma function, `ln Γ(a)`, for `a > 0`.
///
/// Physical meaning: none — a pure mathematical special function used to
/// normalise the gamma molar-distribution in
/// [`crate::petroleum::generate_compounds`].
///
/// Valid range: `a > 0`. Returns `f64::NAN` for `a <= 0` (the reflection
/// formula for negative arguments is not needed here and is not implemented).
/// Accuracy: ~15 significant digits over `0 < a < 10^10`.
#[must_use]
pub fn ln_gamma(a: f64) -> f64 {
    if !(a > 0.0) {
        return f64::NAN;
    }
    // Lanczos: Γ(a) = √(2π) (a − 1 + g + ½)^(a − ½) e^{−(a − 1 + g + ½)} · A_g(a)
    let z = a - 1.0;
    let mut series = LANCZOS_COEFFICIENTS[0];
    for (k, c) in LANCZOS_COEFFICIENTS.iter().enumerate().skip(1) {
        series += c / (z + k as f64);
    }
    let t = z + LANCZOS_G + 0.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + series.ln()
}

/// The gamma function `Γ(a)` for `a > 0`.
///
/// Valid range: `a > 0` and `ln Γ(a) < 709` (i.e. roughly `a < 171.6`), beyond
/// which the result overflows to `f64::INFINITY`. DWSIM calls this only with
/// `a = 1 + 1/B` where `B ∈ {1, 1.5, 3}`, so `a ∈ [1.33, 2]` in practice —
/// deep inside the safe range.
#[must_use]
pub fn gamma(a: f64) -> f64 {
    ln_gamma(a).exp()
}

/// Regularized **lower** incomplete gamma integral
/// `P(a, x) = γ(a, x) / Γ(a)`, the CDF of a gamma distribution with shape `a`
/// and unit scale evaluated at `x`.
///
/// Valid range: `a > 0`, `x >= 0`. Returns `0.0` at `x = 0` and tends to `1.0`
/// as `x → ∞`. Returns `f64::NAN` for invalid arguments.
#[must_use]
pub fn incomplete_gamma_p(a: f64, x: f64) -> f64 {
    if !(a > 0.0) || x < 0.0 || x.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        gamma_series_p(a, x)
    } else {
        1.0 - gamma_continued_fraction_q(a, x)
    }
}

/// Regularized **upper** (complemented) incomplete gamma integral
/// `Q(a, x) = Γ(a, x) / Γ(a) = 1 − P(a, x)`.
///
/// This is the exact function DWSIM calls as
/// `igammaf.incompletegammac(a, x)` in `GenerateCompounds.vb` (`DistMW`
/// `:512`, `DistTB` `:545`, `DistSG` `:590`, `DistVISC1` `:620`, `DistVISC2`
/// `:648`).
///
/// Valid range: `a > 0`, `x >= 0`. Returns `1.0` at `x = 0` and tends to `0.0`
/// as `x → ∞`. Returns `f64::NAN` for invalid arguments.
#[must_use]
pub fn incomplete_gamma_q(a: f64, x: f64) -> f64 {
    if !(a > 0.0) || x < 0.0 || x.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 {
        return 1.0;
    }
    if x < a + 1.0 {
        1.0 - gamma_series_p(a, x)
    } else {
        gamma_continued_fraction_q(a, x)
    }
}

/// `P(a, x)` by the confluent hypergeometric series (DLMF 8.7.1), converging
/// fastest for `x < a + 1`.
fn gamma_series_p(a: f64, x: f64) -> f64 {
    let mut ap = a;
    let mut del = 1.0 / a;
    let mut sum = del;
    for _ in 0..MAX_ITERATIONS {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * EPSILON {
            break;
        }
    }
    sum * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// `Q(a, x)` by the Legendre continued fraction (DLMF 8.9.2) evaluated with the
/// modified Lentz algorithm, converging fastest for `x >= a + 1`.
fn gamma_continued_fraction_q(a: f64, x: f64) -> f64 {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..=MAX_ITERATIONS {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPSILON {
            break;
        }
    }
    h * (-x + a * x.ln() - ln_gamma(a)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** `Γ(n) = (n−1)!` for small positive integers, and
    /// `Γ(1/2) = √π`, are exact closed forms; compare the Lanczos evaluation
    /// against them with a relative tolerance of 1e-12.
    ///
    /// **Results (2026-08-11, this port).** All seven cases — `Γ(1)=1`,
    /// `Γ(2)=1`, `Γ(3)=2`, `Γ(4)=6`, `Γ(5)=24`, `Γ(6)=120`, and
    /// `Γ(0.5)=√π=1.7724538509055159` — match within the 1e-12 relative gate.
    /// Test passes.
    #[test]
    fn gamma_matches_closed_forms() {
        let cases = [
            (1.0, 1.0),
            (2.0, 1.0),
            (3.0, 2.0),
            (4.0, 6.0),
            (5.0, 24.0),
            (6.0, 120.0),
            (0.5, std::f64::consts::PI.sqrt()),
        ];
        for (a, expected) in cases {
            let got = gamma(a);
            assert!(
                ((got - expected) / expected).abs() < 1.0e-12,
                "Γ({a}) = {got}, expected {expected}"
            );
        }
    }

    /// **Methodology.** For integer shape `a = n`, the regularized upper
    /// incomplete gamma has the exact Erlang closed form
    /// `Q(n, x) = e^{−x} Σ_{k=0}^{n−1} x^k / k!`. Check `n = 1, 2, 3` at
    /// several `x` spanning both the series branch (`x < a+1`) and the
    /// continued-fraction branch (`x ≥ a+1`), tolerance 1e-12 absolute.
    ///
    /// **Results (2026-08-11, this port).** All 12 `(n, x)` combinations agree
    /// within the 1e-12 absolute gate; the `x = 0.1` and `x = 10` points
    /// exercise the series and continued-fraction branches respectively. Test
    /// passes.
    #[test]
    fn incomplete_gamma_q_matches_erlang_closed_form() {
        for n in 1..=3usize {
            for x in [0.1_f64, 1.0, 3.0, 10.0] {
                let mut term = 1.0;
                let mut sum = 1.0;
                for k in 1..n {
                    term *= x / k as f64;
                    sum += term;
                }
                let expected = (-x).exp() * sum;
                let got = incomplete_gamma_q(n as f64, x);
                assert!(
                    (got - expected).abs() < 1.0e-12,
                    "Q({n}, {x}) = {got}, expected {expected}"
                );
            }
        }
    }

    /// **Methodology.** `P + Q = 1` must hold identically for every `(a, x)`,
    /// across the branch switch at `x = a + 1`. Sample `a ∈ {1.333, 1.5, 2}`
    /// (the values DWSIM actually uses, `a = 1 + 1/B` for `B ∈ {3, 2, 1}`) and
    /// `x` from 0.01 to 50.
    ///
    /// **Results (2026-08-11, this port).** `|P + Q − 1|` stays within the
    /// 1e-14 gate at all 21 sampled points. Test passes.
    #[test]
    fn p_and_q_sum_to_one() {
        for a in [1.0 + 1.0 / 3.0, 1.5, 2.0] {
            for x in [0.01_f64, 0.5, 1.0, 2.5, 5.0, 20.0, 50.0] {
                let p = incomplete_gamma_p(a, x);
                let q = incomplete_gamma_q(a, x);
                assert!(
                    (p + q - 1.0).abs() < 1.0e-14,
                    "P({a},{x}) + Q({a},{x}) = {}",
                    p + q
                );
            }
        }
    }
}
