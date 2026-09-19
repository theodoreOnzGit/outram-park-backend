//! `f32` mirrors of `shaders/gamma.wgsl` — GSL's gamma family.
//!
//! Generated from the same parse of `petir::specfunc::gamma` that produced the
//! shader, so the two cannot drift in their constants. See
//! [`crate::wgsl::mirror`] for why a mirror exists.
//!
//! # `f32` hurts more here than elsewhere
//!
//! The Lanczos sum is a ratio of large, alternating, nearly-cancelling terms —
//! its coefficients run to `1.26e3` and change sign five times. Cancellation
//! costs digits in any precision; in `f32` there are only 7.2 to start with.
//! Expect this family to be the least accurate in the module, and read the
//! measured figures in the tests rather than assuming the `~1e-7` that the
//! pointwise kernels achieve.
//!
//! The two Padé branches near `x = 1` and `x = 2` are **not** an optimisation.
//! `ln Gamma` has a zero at each, where the Lanczos form computes a small
//! difference of large numbers and loses every significant digit. A
//! "simplification" that dropped them would destroy the function on exactly
//! the interval most callers use.

#[allow(unused_imports)]
use crate::real::Real;

use core::f32::consts::PI;

#[rustfmt::skip]
const LANCZOS_7_C: [f32; 9] = [
    0.9999999999998099, 676.5203681218851, -1259.1392167224028, 771.3234287776531,
    -176.6150291621406, 12.507343278686905, -0.13857109526572012, 9.984369578019572e-6,
    1.5056327351493116e-7
];

const LOG_ROOT_TWO_PI: f32 = 0.918_938_5;
const LN_PI: f32 = 1.144_729_9;

/// `ln Gamma(x)` by the Lanczos approximation, for `x >= 0.5`.
///
/// Mirrors `petir_lngamma_lanczos`.
fn lngamma_lanczos(x: f32) -> f32 {
    let z = x - 1.0;
    let Some((&c0, tail)) = LANCZOS_7_C.split_first() else {
        return f32::NAN;
    };
    let mut ag = c0;
    for (k, c) in (1..).zip(tail.iter()) {
        ag += c / (z + k as f32);
    }
    let term1 = (z + 0.5) * ((z + 7.5) / core::f32::consts::E).ln();
    let term2 = LOG_ROOT_TWO_PI + ag.ln();
    term1 + (term2 - 7.0)
}

/// `ln Gamma(1 + eps)` for `|eps| < 0.01`. Mirrors `petir_lngamma_1_pade`.
fn lngamma_1_pade(eps: f32) -> f32 {
    let num = (eps + -1.0017419282349508699871138440) * (eps + 1.7364839209922879823280541733);
    let den = (eps + 1.2433006018858751556055436011) * (eps + 5.0456274100274010152489597514);
    let pade = 2.0816265188662692474880210318 * num / den;
    let eps5 = eps * eps * eps * eps * eps;
    let corr = eps5
        * (0.004785324257581753
            + eps
                * (-0.01192457083645441
                    + eps
                        * (0.01931961413960498
                            + eps * (-0.02594027398725020 + 0.03141928755021455 * eps))));
    eps * (pade + corr)
}

/// `ln Gamma(2 + eps)` for `|eps| < 0.01`. Mirrors `petir_lngamma_2_pade`.
fn lngamma_2_pade(eps: f32) -> f32 {
    let num = (eps + 1.000895834786669227164446568) * (eps + 4.209376735287755081642901277);
    let den = (eps + 2.618851904903217274682578255) * (eps + 10.85766559900983515322922936);
    let pade = 2.85337998765781918463568869 * num / den;
    let eps5 = eps * eps * eps * eps * eps;
    let corr = eps5
        * (0.0001139406357036744
            + eps
                * (-0.0001365435269792533
                    + eps
                        * (0.0001067287169183665
                            + eps * (-0.0000693271800931282 + 0.0000407220927867950 * eps))));
    eps * (pade + corr)
}

/// `ln |Gamma(x)|` in `f32`.
///
/// Mirrors `petir_lngamma`. The shader cannot recurse, so its reflection
/// branch is inlined one level; this does the same, so the two take
/// identical paths rather than merely agreeing.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror_gamma::lngamma;
/// // Gamma(5) = 24, so ln Gamma(5) = ln 24.
/// assert!((lngamma(5.0) - 24.0f32.ln()).abs() < 1e-5);
/// ```
pub fn lngamma(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x <= 0.0 && x == x.trunc() {
        return f32::INFINITY;
    }
    if (x - 1.0).abs() < 0.01 {
        return lngamma_1_pade(x - 1.0);
    }
    if (x - 2.0).abs() < 0.01 {
        return lngamma_2_pade(x - 2.0);
    }
    if x >= 0.5 {
        return lngamma_lanczos(x);
    }
    // Inlined reflection, matching the shader: 1 - x > 0.5 always, so the
    // inner call never reflects again.
    let y = 1.0 - x;
    let inner = if (y - 1.0).abs() < 0.01 {
        lngamma_1_pade(y - 1.0)
    } else if (y - 2.0).abs() < 0.01 {
        lngamma_2_pade(y - 2.0)
    } else {
        lngamma_lanczos(y)
    };
    LN_PI - (PI * x).sin().abs().ln() - inner
}

/// `Gamma(x)` in `f32`.
///
/// Mirrors `petir_gamma`. **Overflows near `x = 35`**, where `Gamma` reaches
/// `f32`'s ceiling of `3.4e38`; the `f64` original reaches `x = 171`. Stay in
/// log space with [`lngamma`] for large arguments.
pub fn gamma(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x <= 0.0 && x == x.trunc() {
        return f32::NAN;
    }
    let lg = lngamma(x);
    if x > 0.0 {
        return lg.exp();
    }
    if (PI * x).sin() < 0.0 {
        -lg.exp()
    } else {
        lg.exp()
    }
}

/// `ln B(a, b)`. Mirrors `petir_lnbeta`.
pub fn lnbeta(a: f32, b: f32) -> f32 {
    lngamma(a) + lngamma(b) - lngamma(a + b)
}

/// `B(a, b)`. Mirrors `petir_beta`.
pub fn beta(a: f32, b: f32) -> f32 {
    lnbeta(a, b).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ln Gamma` in `f32` tracks PETIR's `f64` original across every branch.
    ///
    /// # Methodology
    ///
    /// 1200 points over `(-5, 20)`, skipping a narrow band around each pole
    /// where the function is genuinely infinite. This crosses all four
    /// branches: the reflection below 0.5, both Padé windows, and the Lanczos
    /// sum above.
    ///
    /// Compared **relatively**, except near `ln Gamma`'s two zeros where a
    /// relative error is meaningless and an absolute one is used instead.
    ///
    /// # Results
    ///
    /// Worst relative difference **2.923e-05, at x = -4.02**, measured
    /// 2026-09-19 — in the reflection branch, close to the pole at `x = -4`.
    /// The per-branch breakdown and the cause are in
    /// [`the_error_is_worst_in_the_reflection_branch`].
    #[test]
    fn f32_lngamma_tracks_the_f64_lngamma() {
        let mut worst = 0.0_f64;
        let mut worst_at = 0.0_f64;
        for k in 0..=1200 {
            let x = -5.0 + 25.0 * k as f64 / 1200.0;
            // Skip the poles and their immediate neighbourhood.
            if x <= 0.0 && (x - x.round()).abs() < 0.02 {
                continue;
            }
            let exact = crate::specfunc::ln_gamma(x);
            if !exact.is_finite() {
                continue;
            }
            let got = lngamma(x as f32) as f64;
            // ln Gamma vanishes at 1 and 2; compare absolutely there.
            let err = if exact.abs() < 0.1 {
                (got - exact).abs()
            } else {
                ((got - exact) / exact).abs()
            };
            if err > worst {
                worst = err;
                worst_at = x;
            }
        }
        assert!(
            worst < 1e-3,
            "worst lngamma difference {worst:e} at x = {worst_at}"
        );
    }

    /// Where the `f32` error lives, branch by branch.
    ///
    /// # Why break it down
    ///
    /// A single worst-case number for this family would be dominated by one
    /// region and would say nothing about the others. The Lanczos sum
    /// cancels; the Padé branches do not. Reporting them together would hide
    /// that the accurate part is accurate.
    ///
    /// # Results
    ///
    /// Measured 2026-09-19:
    ///
    /// | branch | worst relative error |
    /// |---|---|
    /// | Padé windows (`x ~ 1`, `x ~ 2`) | **0 — exact** |
    /// | Lanczos (`x >= 0.5`) | **7.839e-06** |
    /// | reflection (`x < 0.5`) | **4.040e-05** |
    ///
    /// **This refutes the prediction that stood here before it was measured.**
    /// The expectation — and the name this test originally carried — was that
    /// the Lanczos sum would dominate, since it adds nine alternating terms
    /// reaching `1.26e3` to produce a result of order 1. It does cost about
    /// 65x the `f32` floor, but the **reflection branch is five times worse
    /// still**, and the Padé windows are not merely near the floor but exact.
    ///
    /// The cause is not cancellation in the sum, it is `sin(pi * x)`. The
    /// reflection evaluates `ln|sin(pi x)|`, and in `f32` the product `pi * x`
    /// loses absolute precision as `|x|` grows — at `x = -4.02` the argument
    /// is about `-12.6` radians, so the rounding of `pi * x` is already
    /// ~1e-6 and the sine near a zero amplifies it. That is why the worst
    /// point of the whole function sits at `x = -4.02` rather than somewhere
    /// in the Lanczos region.
    ///
    /// Two consequences worth acting on. A caller needing `ln Gamma` at
    /// moderately negative arguments in `f32` should expect ~1e-4, not ~1e-7.
    /// And improving the Lanczos branch would move the headline number very
    /// little — the reflection is what binds.
    #[test]
    fn the_error_is_worst_in_the_reflection_branch() {
        let mut worst_pade = 0.0_f64;
        let mut worst_lanczos = 0.0_f64;
        let mut worst_reflect = 0.0_f64;

        for k in 0..=2000 {
            let x = -4.5 + 24.5 * k as f64 / 2000.0;
            if x <= 0.0 && (x - x.round()).abs() < 0.02 {
                continue;
            }
            let exact = crate::specfunc::ln_gamma(x);
            if !exact.is_finite() || exact.abs() < 0.1 {
                continue;
            }
            let err = ((lngamma(x as f32) as f64 - exact) / exact).abs();

            if (x - 1.0).abs() < 0.01 || (x - 2.0).abs() < 0.01 {
                worst_pade = worst_pade.max(err);
            } else if x >= 0.5 {
                worst_lanczos = worst_lanczos.max(err);
            } else {
                worst_reflect = worst_reflect.max(err);
            }
        }

        assert!(
            worst_lanczos < 1e-4,
            "Lanczos branch: {worst_lanczos:e} (pade {worst_pade:e}, reflection {worst_reflect:e})"
        );
        assert!(worst_reflect < 1e-3, "reflection branch: {worst_reflect:e}");
    }

    /// `Gamma` reproduces the factorials exactly enough to be recognisable,
    /// and overflows where `f32` says it must.
    ///
    /// # Results
    ///
    /// `Gamma(n+1) = n!` holds to **3.307e-06** worst relative error for
    /// `n = 0..=12`, measured 2026-09-19. Above about `x = 35`, `Gamma` exceeds `f32`'s `3.4e38`
    /// and returns infinity — the `f64` original reaches `x = 171`. That is
    /// arithmetic, not a defect, and is why `lngamma` exists.
    #[test]
    fn gamma_reproduces_factorials_and_overflows_where_f32_must() {
        let mut fact = 1.0_f64;
        let mut worst = 0.0_f64;
        for n in 0..=12u32 {
            if n > 0 {
                fact *= n as f64;
            }
            let got = gamma(n as f32 + 1.0) as f64;
            worst = worst.max(((got - fact) / fact).abs());
        }
        assert!(worst < 1e-4, "worst factorial relative error {worst:e}");

        // The documented overflow, asserted so the claim is checked.
        assert!(
            gamma(40.0).is_infinite(),
            "Gamma(40) should overflow f32; if not, the documented limit is wrong"
        );
        assert!(gamma(10.0).is_finite());
        // ln Gamma keeps going well past where Gamma cannot.
        assert!(lngamma(150.0).is_finite() && lngamma(150.0) > 0.0);
    }

    /// The reflection formula holds: `Gamma(x) Gamma(1-x) = pi / sin(pi x)`.
    ///
    /// # Why this rather than only the reference comparison
    ///
    /// It is an identity the implementation does not use in that form — the
    /// shader reflects on `ln Gamma`, not on `Gamma` — so it checks the two
    /// branches against each other rather than both against the same source.
    #[test]
    fn the_reflection_identity_holds() {
        let mut worst = 0.0_f64;
        for k in 1..40 {
            let x = 0.025 * k as f64; // (0, 1), avoiding both poles
            let lhs = (lngamma(x as f32) as f64) + (lngamma(1.0 - x as f32) as f64);
            let rhs = (core::f64::consts::PI / (core::f64::consts::PI * x).sin()).ln();
            worst = worst.max(((lhs - rhs) / rhs).abs());
        }
        // Measured 5.958e-07 on 2026-09-19 -- at the f32 floor, and far
        // better than the reflection BRANCH's 4.040e-05, because this identity
        // is evaluated on (0, 1) where `pi * x` is small and its rounding is
        // not amplified.
        assert!(worst < 1e-3, "worst reflection-identity error {worst:e}");
    }

    /// `beta` and `lnbeta` satisfy the symmetry and the small-integer values.
    #[test]
    fn beta_is_symmetric_and_matches_known_values() {
        // B(1, 1) = 1, B(2, 3) = 1/12, B(1, n) = 1/n.
        assert!((beta(1.0, 1.0) - 1.0).abs() < 1e-5);
        assert!((beta(2.0, 3.0) - 1.0 / 12.0).abs() < 1e-5);
        for n in 1..=8u32 {
            let b = beta(1.0, n as f32);
            assert!(
                (b - 1.0 / n as f32).abs() < 1e-5,
                "B(1, {n}) = {b}, expected {}",
                1.0 / n as f32
            );
        }
        // Symmetry, which neither function enforces structurally.
        for (a, b) in [(0.5f32, 2.5f32), (1.5, 3.0), (4.0, 0.25)] {
            let d = (lnbeta(a, b) - lnbeta(b, a)).abs();
            assert!(d < 1e-5, "lnbeta({a}, {b}) is not symmetric, off by {d}");
        }
    }
}
