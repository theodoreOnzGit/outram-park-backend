//! `f32` mirrors of `shaders/sinint.wgsl` — the sine and cosine integrals
//! `Si(x)` and `Ci(x)`.
//!
//! Generated from the same parse of [`crate::specfunc::sinint`] that produced
//! the shader. All six tables carry **GSL's single-precision order**, as
//! [`crate::wgsl::mirror_dawson`] introduced: 81 coefficients where the `f64`
//! order needs 129.
//!
//! # The one kernel here whose accuracy is set by `sin` and `cos`
//!
//! Past `x = 4` both are written through two auxiliary functions,
//!
//! ```text
//!     Si(x) = pi/2 - f(x) cos x - g(x) sin x
//!     Ci(x) =        f(x) sin x - g(x) cos x
//! ```
//!
//! with `f ~ 1/x` and `g ~ 1/x^2`, both smooth. The oscillation lives
//! entirely in the `sin`/`cos` factors — which is what makes `f` and `g`
//! fittable, and which means everything this kernel can promise at large `x`
//! is whatever those two can deliver.
//!
//! # The argument reduction was tried and is absent on the evidence
//!
//! [`crate::wgsl::mirror_clausen::reduce`] already implements the `f32`
//! three-way `2 pi` split, and reusing it here looked obviously right.
//! Measured, it is worse on both sides.
//!
//! Against the `f64` module, `Ci`'s error relative to its own `1/x`
//! envelope (2026-09-19):
//!
//! | range | direct `sin`/`cos` | through the reduction |
//! |---|---|---|
//! | `[4, 1e2]` | 2.104e-07 | 2.722e-07 |
//! | `[1e2, 1e4]` | 3.635e-07 | 3.693e-07 |
//! | `[1e4, 1e5]` | 1.955e-07 | **1.182e-06** |
//! | `[1e5, 5.2e5]` | 1.652e-07 | **7.993e-06** |
//! | `[5.2e5, 1e7]` | 1.749e-07 | **7.458e-06** |
//!
//! `Si` is unaffected either way — flat at 1.0e-07 to 1.8e-07 absolute
//! across every one of those ranges — because there `sin` and `cos` are
//! multiplied by `f ~ 1/x` against a leading `pi/2`. `Ci` **is**
//! `f sin - g cos`, so it carries their error at full weight. That asymmetry
//! is why `Ci` is the instrument and `Si` is not.
//!
//! On the device the same holds: llvmpipe's `sin` is within 1e-07 of `f64`
//! to about `x = 1e7`, and the reduction returns `NaN` past its own loss cut
//! of 524288 — so above `5.2e5` it is not even available, and below it it is
//! the worse of the two. Both libraries already reduce properly internally.
//!
//! `the_argument_reduction_was_measured_and_rejected` keeps the refuted
//! alternative runnable so the comparison stays checkable.
//!
//! # The honest range
//!
//! What ends it is neither implementation: at `x = 1e7` one `f32` ulp of the
//! **argument** is 1 radian, so `sin(x)` is not determined by the `f32` `x`
//! at all. `the_usable_range_ends_where_the_argument_does` measures where.

// Under a std-linked build (`cargo test`) f32's inherent abs/ln/sin/cos
// shadow these trait methods, leaving the import formally unused. See
// crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `sqrt(f32::EPSILON)`, below which `Si(x) = x`. Mirrors
/// `PETIR_SI_SMALL_CUT`.
const SMALL_CUT: f32 = 3.4526698e-4;
/// `sqrt(50)`, where `f` and `g` change fit. Upstream writes the rounded
/// decimal 7.07106781187 rather than the square root, and it is carried
/// verbatim because it is a **branch boundary**: rounding it differently
/// moves which fit runs. Mirrors `PETIR_SI_XBND`.
const XBND: f32 = 7.0710678;
/// `1/sqrt(f32::EPSILON)`, past which `f = 1/x` and `g = 1/x^2` exactly.
/// Mirrors `PETIR_SI_XBIG`.
const XBIG: f32 = 2896.3093;

/// GSL's `f1_data` at its **single-precision order**:
/// 11 of the 20 stored, where `f64` evaluates 20.
#[rustfmt::skip]
const F1: [f32; 11] = [
    -0.11910820007324219, -0.024782314896583557, 0.0011910281609743834,
    -9.270277223549783e-05, 9.337313713331241e-06, -1.1058288009735406e-06,
    1.464772054760033e-07, -2.1069450184540983e-08, 3.229349232469758e-09,
    -5.20652965185775e-10, 8.748789193102624e-11,
];

/// GSL's `f2_data` at its **single-precision order**:
/// 15 of the 29 stored, where `f64` evaluates 29.
#[rustfmt::skip]
const F2: [f32; 15] = [
    -0.03484092652797699, -0.016684221103787422, 0.0006752901244908571,
    -5.350666106096469e-05, 6.269342065934325e-06, -9.526638677925803e-07,
    1.7456292766837578e-07, -3.687954119868664e-08, 8.720268063200365e-09,
    -2.260197140557807e-09, 6.324624712839011e-10, -1.8889119435261392e-10,
    5.967746435908694e-11, -1.980443066484927e-11, 6.86413946515696e-12,
];

/// GSL's `g1_data` at its **single-precision order**:
/// 14 of the 21 stored, where `f64` evaluates 21.
#[rustfmt::skip]
const G1: [f32; 14] = [
    -0.30405786633491516, -0.056689098477363586, 0.0039046157617121935,
    -0.00037460759631358087, 4.3543153878999874e-05, -5.741729637520621e-06,
    8.282552244054386e-07, -1.2782459180016303e-07, 2.079783456565565e-08,
    -3.531320569294394e-09, 6.210824077257371e-10, -1.1252154763496947e-10,
    2.0908890938087232e-11, -3.971583045075944e-12,
];

/// GSL's `g2_data` at its **single-precision order**:
/// 21 of the 34 stored, where `f64` evaluates 34.
#[rustfmt::skip]
const G2: [f32; 21] = [
    -0.09673293679952621, -0.04520779103040695, 0.0028190005104988813,
    -0.0002899167884606868, 4.074446769664064e-05, -7.105638360371813e-06,
    1.453472350476659e-06, -3.364116594184452e-07, 8.597744027838417e-08,
    -2.3843766072673134e-08, 7.08319047859618e-09, -2.2318067394166974e-09,
    7.401087520619853e-10, -2.567171197842555e-10, 9.267070444352044e-11,
    -3.4669329906922286e-11, 1.339505763253701e-11, -5.32907528869031e-12,
    2.177531158858992e-12, -9.118620572165503e-13, 3.9058640761806263e-13,
];

/// GSL's `si_data` at its **single-precision order**:
/// 10 of the 12 stored, where `f64` evaluates 12.
#[rustfmt::skip]
const SI: [f32; 10] = [
    -0.131564661860466, -0.2776578664779663, 0.035441406071186066, -0.002563163172453642,
    0.0001162365369964391, -3.5904326978197787e-06, 8.023421571579092e-08,
    -1.356299739185829e-09, 1.794407157584832e-11, -1.90838704973266e-13,
];

/// GSL's `ci_data` at its **single-precision order**:
/// 10 of the 13 stored, where `f64` evaluates 13.
#[rustfmt::skip]
const CI: [f32; 10] = [
    -0.3400428295135498, -1.0330216884613037, 0.19388222694396973, -0.019182603806257248,
    0.0011078924871981144, -4.1572344343876466e-05, 1.0927852827080642e-06,
    -2.123285902655425e-08, 3.1733482508400357e-10, -3.761415658110057e-12,
];

/// Clenshaw in GSL's convention.
fn cheb(c: &[f32], x: f32) -> f32 {
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut d, mut dd) = (0.0_f32, 0.0_f32);
    let y2 = 2.0 * x;
    for &ci in rest.iter().rev() {
        let temp = d;
        d = y2 * d - dd + ci;
        dd = temp;
    }
    x * d - dd + 0.5 * c0
}

/// `(sin x, cos x)`. `reduced` selects the **refuted** path through
/// [`crate::wgsl::mirror_clausen::reduce`], kept so the comparison in
/// `the_argument_reduction_was_measured_and_rejected` stays runnable. The
/// shipped path is `false`. Mirrors `petir_si_sin_cos`, which does not
/// reduce.
fn sin_cos_with(x: f32, reduced: bool) -> (f32, f32) {
    if !reduced {
        return (x.sin(), x.cos());
    }
    let r = crate::wgsl::mirror_clausen::reduce(x);
    if r.is_nan() {
        // Past the reduction's loss cut there is nothing better to do than
        // hand the raw argument to the library.
        return (x.sin(), x.cos());
    }
    (r.sin(), r.cos())
}

/// The asymptotic pair `(f, g)` for `x >= 4`. Mirrors `petir_si_fg_asymp`.
///
/// **Upstream's two far-field guards are absent**, and that is the
/// [`crate::wgsl::mirror_fermi_dirac`] third outcome once more: GSL zeroes
/// `f` past `1/DBL_MIN` and `g` past `1/sqrt(DBL_MIN)`, neither of which is
/// an `f32`. The arithmetic reaches the same zeros by itself — `x*x`
/// overflows to infinity above 1.8e19 so `1/x^2` becomes 0 — and does so
/// **later**, so deleting them gains answers.
fn fg_asymp(x: f32) -> (f32, f32) {
    let x2 = x * x;
    if x <= XBND {
        let t = (1.0 / x2 - 0.04125) / 0.02125;
        ((1.0 + cheb(&F1, t)) / x, (1.0 + cheb(&G1, t)) / x2)
    } else if x <= XBIG {
        let t = 100.0 / x2 - 1.0;
        ((1.0 + cheb(&F2, t)) / x, (1.0 + cheb(&G2, t)) / x2)
    } else {
        (1.0 / x, 1.0 / x2)
    }
}

/// `Si(x)` in `f32`. Mirrors `petir_si`.
pub fn si(x: f32) -> f32 {
    si_with(x, false)
}

/// [`si`] with the reduction selectable.
fn si_with(x: f32, reduced: bool) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    let ax = x.abs();
    if ax < SMALL_CUT {
        return x;
    }
    if ax <= 4.0 {
        return x * (0.75 + cheb(&SI, (x * x - 8.0) * 0.125));
    }
    let (f, g) = fg_asymp(ax);
    let (s, c) = sin_cos_with(ax, reduced);
    let v = core::f32::consts::FRAC_PI_2 - f * c - g * s;
    if x < 0.0 {
        -v
    } else {
        v
    }
}

/// `Ci(x)` in `f32`. Mirrors `petir_ci`. `NaN` for `x <= 0`.
pub fn ci(x: f32) -> f32 {
    ci_with(x, false)
}

/// [`ci`] with the reduction selectable.
fn ci_with(x: f32, reduced: bool) -> f32 {
    if x.is_nan() || x <= 0.0 {
        return f32::NAN;
    }
    if x <= 4.0 {
        let lx = x.ln();
        return lx - 0.5 + cheb(&CI, (x * x - 8.0) * 0.125);
    }
    let (f, g) = fg_asymp(x);
    let (s, c) = sin_cos_with(x, reduced);
    f * s - g * c
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::sinint as f64_si;

    /// `Ci` oscillates through zeros with an envelope of `1/x`, so its error
    /// is measured **against that envelope** rather than against its value.
    /// A relative figure near a zero measures where the probe grid fell, and
    /// a first draft of this module's probe reported 2.9e-04 at `x = 9.5255`
    /// for exactly that reason — that abscissa sits on `Ci`'s fourth zero.
    fn ci_envelope_error(x: f32) -> f64 {
        (((ci(x) as f64) - f64_si::ci(x as f64)) * x as f64).abs()
    }

    /// **What `f32` costs, and the instrument differs per function.**
    /// Measured 2026-09-19.
    ///
    /// | | figure | worst | at |
    /// |---|---|---|---|
    /// | `Si` on `(0, 4]` | relative | 1.511e-07 | 3.483 |
    /// | `Si` on `[4, 1e9]` | absolute | 1.770e-07 | 5.668 |
    /// | `Ci` on `(0, 4]` | absolute | 9.472e-07 | 1.157e-06 |
    /// | `Ci` on `[4, 1e9]` | relative to `1/x` | 3.635e-07 | 2911 |
    ///
    /// `Si` is bounded and `O(1)`, so absolute and relative agree on it past
    /// the origin. `Ci` is unbounded below at the origin — it goes as
    /// `ln x` — and oscillates with a `1/x` envelope above, so neither a
    /// single relative nor a single absolute figure covers it; the two halves
    /// take different instruments.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        // Si, relative, on the Chebyshev branch.
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=100_000 {
            let x = 1e-5_f32 * (4e5_f32).powf(k as f32 / 100_000.0);
            let t = f64_si::si(x as f64);
            if t.abs() > 1e-6 {
                let e = (((si(x) as f64) - t) / t).abs();
                if e > w {
                    w = e;
                    at = x;
                }
            }
        }
        assert!(
            w < 5e-7,
            "Si relative on (0, 4]: {w:e} at {at:e}, documented 1.511e-07"
        );

        // Si, absolute, across the asymptotic branch and far beyond.
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=200_000 {
            let x = 4.0_f32 * (2.5e8_f32).powf(k as f32 / 200_000.0);
            let e = ((si(x) as f64) - f64_si::si(x as f64)).abs();
            if e > w {
                w = e;
                at = x;
            }
        }
        assert!(
            w < 5e-7,
            "Si absolute on [4, 1e9]: {w:e} at {at:e}, documented 1.770e-07"
        );

        // Ci, absolute, on the Chebyshev branch (it is unbounded at 0).
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=200_000 {
            let x = 1e-6_f32 * (4e6_f32).powf(k as f32 / 200_000.0);
            let e = ((ci(x) as f64) - f64_si::ci(x as f64)).abs();
            if e > w {
                w = e;
                at = x;
            }
        }
        assert!(
            w < 5e-6,
            "Ci absolute on (0, 4]: {w:e} at {at:e}, documented 9.472e-07"
        );

        // Ci, against its envelope, on the asymptotic branch.
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=200_000 {
            let x = 4.0_f32 * (2.5e8_f32).powf(k as f32 / 200_000.0);
            let e = ci_envelope_error(x);
            if e > w {
                w = e;
                at = x;
            }
        }
        assert!(
            w < 1e-6,
            "Ci vs its 1/x envelope on [4, 1e9]: {w:e} at {at:e}"
        );
    }

    /// **The argument reduction was tried and rejected on the evidence.**
    ///
    /// [`crate::wgsl::mirror_clausen::reduce`] is the `f32` three-way `2 pi`
    /// split, and reusing it here looked obviously right. It is worse.
    /// Measured 2026-09-19, `Ci` against its `1/x` envelope:
    ///
    /// | range | direct | reduced |
    /// |---|---|---|
    /// | `[1e4, 1e5]` | 1.955e-07 | **1.182e-06** |
    /// | `[1e5, 5.2e5]` | 1.652e-07 | **7.993e-06** |
    ///
    /// and `Si` is unaffected either way, because there `sin` and `cos` are
    /// multiplied by `f ~ 1/x` against a leading `pi/2` while `Ci` **is**
    /// `f sin - g cos`. That asymmetry is why `Ci` is the instrument.
    ///
    /// Both `libm` and llvmpipe already reduce properly inside `sin`; the
    /// three-term split is a coarser reduction than either, and past its own
    /// loss cut of 524288 it returns `NaN` and is not available at all.
    #[test]
    fn the_argument_reduction_was_measured_and_rejected() {
        let worst = |lo: f32, hi: f32, reduced: bool| {
            let mut w = 0.0_f64;
            for k in 0..=50_000 {
                let x = lo * (hi / lo).powf(k as f32 / 50_000.0);
                let e = (((ci_with(x, reduced) as f64) - f64_si::ci(x as f64)) * x as f64).abs();
                if e > w {
                    w = e;
                }
            }
            w
        };
        for (lo, hi) in [(1e4_f32, 1e5_f32), (1e5, 5.2e5)] {
            let (direct, reduced) = (worst(lo, hi, false), worst(lo, hi, true));
            assert!(
                reduced > 4.0 * direct,
                "over [{lo:e}, {hi:e}] the reduction is documented as several \
                 times WORSE than calling sin/cos directly: direct {direct:e}, \
                 reduced {reduced:e}. If it has become better, the shader \
                 should be reconsidered"
            );
        }
        // Si is untouched by the choice, which is the asymmetry above.
        for x in [1e4_f32, 1e5, 4e5] {
            assert_eq!(
                si_with(x, false).to_bits(),
                si_with(x, true).to_bits(),
                "Si is documented as insensitive to the reduction; it differs at {x:e}"
            );
        }
        // And the reduction is simply unavailable past its loss cut.
        assert!(crate::wgsl::mirror_clausen::reduce(6e5).is_nan());
    }

    /// **The kernel stays faithful to its argument at every magnitude; what
    /// degrades is the argument.**
    ///
    /// The error against `f64` is flat — 1.0e-07 to 1.8e-07 for `Si`, and
    /// 1.3e-07 to 3.6e-07 for `Ci` against its envelope — from `x = 4` all
    /// the way to `1e9`. That is because both sides are handed the *same*
    /// `f32` value, which is the right comparison for a transcription.
    ///
    /// It is not the right comparison for a caller. At large `x` one ulp of
    /// the argument is a large fraction of a period (2026-09-19):
    ///
    /// | `x` | `ulp(x)` | `Ci` changes by | envelope `1/x` |
    /// |---|---|---|---|
    /// | 1e5 | 7.81e-03 rad | 7.81e-08 | 1e-05 |
    /// | 1e6 | 6.25e-02 rad | 5.92e-08 | 1e-06 |
    /// | **1e7** | **1.0 rad** | 9.57e-08 | 1e-07 |
    ///
    /// At `x = 1e7` moving to the next representable argument changes `Ci` by
    /// the whole envelope. The answer is right for the number it was given;
    /// the number is no longer the one the caller meant. **The oscillating
    /// branch is honest to about `x = 1e6`**, and no reduction can extend
    /// that, which is the real reason one is not shipped.
    #[test]
    fn the_kernel_is_faithful_but_the_argument_is_not() {
        // Flat faithfulness across nine decades.
        for (lo, hi) in [
            (4.0_f32, 1e2_f32),
            (1e2, 1e4),
            (1e4, 1e6),
            (1e6, 1e8),
            (1e8, 1e9),
        ] {
            let mut w = 0.0_f64;
            for k in 0..=20_000 {
                let x = lo * (hi / lo).powf(k as f32 / 20_000.0);
                w = w.max(ci_envelope_error(x));
            }
            assert!(
                w < 1e-6,
                "Ci's envelope-relative error is documented as FLAT across \
                 every decade; over [{lo:e}, {hi:e}] it is {w:e}"
            );
        }

        // And the argument's own resolution, which is what actually ends the
        // usable range.
        for (x, want_rad) in [(1e5_f32, 7.8125e-3_f32), (1e6, 6.25e-2), (1e7, 1.0)] {
            let next = f32::from_bits(x.to_bits() + 1);
            assert_eq!(
                next - x,
                want_rad,
                "ulp({x:e}) is documented as {want_rad} rad"
            );
            let swing = (ci(next) - ci(x)).abs() as f64;
            let envelope = 1.0 / x as f64;
            if x >= 1e7 {
                assert!(
                    swing > 0.5 * envelope,
                    "at x = {x:e} one ulp of the argument is a radian, so Ci is \
                     documented as swinging by an appreciable fraction of its \
                     envelope; it swung {swing:e} against {envelope:e}"
                );
            } else {
                assert!(
                    swing < 0.2 * envelope,
                    "at x = {x:e} one ulp is a small fraction of a period, so Ci \
                     should barely move; it swung {swing:e} against {envelope:e}"
                );
            }
        }
    }

    /// **Both far-field guards are deleted, and that gains answers.**
    ///
    /// GSL zeroes `f` past `1/DBL_MIN = 4.494e+307` and `g` past
    /// `1/sqrt(DBL_MIN) = 6.704e+153`. Neither is an `f32` — the type stops
    /// at 3.4e38 — so this is [`crate::wgsl::mirror_fermi_dirac`]'s third
    /// outcome again: keep and retarget are both unavailable, the branch
    /// goes, and the arithmetic reaches the same zeros by itself.
    ///
    /// Here it is strictly a gain: `1/x` stays representable for every finite
    /// `f32`, and `1/x^2` becomes zero only once `x*x` overflows above
    /// 1.8e19 — later than the `f32` analogue of either guard would fire.
    #[test]
    fn the_far_field_guards_are_deleted_and_that_gains_answers() {
        // f survives to the end of the type.
        let (f, _) = fg_asymp(f32::MAX);
        assert!(f > 0.0 && f.is_finite(), "f(f32::MAX) = {f:e}");
        assert_eq!(f, 1.0 / f32::MAX);

        // g goes to zero only when x*x overflows, which is where it should.
        let (_, g_ok) = fg_asymp(1.8e19);
        assert!(g_ok > 0.0, "g(1.8e19) = {g_ok:e} should still be non-zero");
        let (_, g_gone) = fg_asymp(2e19);
        assert_eq!(g_gone, 0.0, "past 1.8e19, x*x overflows and g is 0");

        // The f32 analogues of upstream's guards -- which are NOT used --
        // would have fired earlier than that.
        let f32_xmaxg = 1.0 / f32::MIN_POSITIVE.sqrt();
        assert!(
            f32_xmaxg < 1.8e19,
            "upstream's g guard, retargeted, would fire at {f32_xmaxg:e}, before \
             the arithmetic does"
        );

        // So Ci and Si keep answering as far as the type goes.
        assert!(ci(1e30).is_finite() && ci(1e30) != 0.0);
        assert!((si(1e30) - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    /// Shape, symmetry and the domain restriction.
    #[test]
    fn the_shape_and_the_domain_survive_f32() {
        assert_eq!(si(0.0), 0.0);
        assert!(si(f32::NAN).is_nan() && ci(f32::NAN).is_nan());

        // Si is odd, exactly.
        for k in 1..=4000 {
            let x = 0.01 * k as f32;
            assert_eq!(si(-x), -si(x), "Si not odd at {x}");
        }
        // And bounded by its maximum at pi.
        let cap = si(core::f32::consts::PI);
        assert!((cap - 1.851_937_1).abs() < 1e-5, "Si(pi) = {cap}");
        for k in 0..=200_000 {
            let x = 0.001 * k as f32;
            assert!(si(x) <= cap + 1e-6, "Si({x}) exceeds Si(pi)");
        }

        // Ci's domain: upstream's DOMAIN_ERROR at and below zero.
        for x in [0.0_f32, -1e-30, -1.0, -1e30, f32::NEG_INFINITY] {
            assert!(ci(x).is_nan(), "Ci({x:e}) should be NaN");
        }
        // Its first zero, where the literature puts it.
        let (mut lo, mut hi) = (0.1_f32, 1.5_f32);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if ci(m) < 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        assert!(
            (0.5 * (lo + hi) - 0.616_505_5).abs() < 1e-4,
            "Ci's first zero is documented at 0.6165055; it is {}",
            0.5 * (lo + hi)
        );
    }

    /// The shader and this mirror agree on their constants and orders, and
    /// the shader carries **no** reduction.
    #[test]
    fn the_shader_and_this_mirror_agree() {
        let code: std::string::String = crate::wgsl::SININT
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<std::vec::Vec<_>>()
            .join("\n");
        let read = |name: &str| -> f32 {
            let at = code.find(name).unwrap_or_else(|| panic!("{name} missing"));
            let rest = &code[at..];
            let eq = rest.find('=').expect("no =");
            let end = rest[eq..].find(';').expect("no ;");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .expect("f32 literal")
        };
        assert_eq!(read("PETIR_SI_SMALL_CUT: f32"), SMALL_CUT);
        assert_eq!(read("PETIR_SI_XBND: f32"), XBND);
        assert_eq!(read("PETIR_SI_XBIG: f32"), XBIG);
        assert!(
            !code.contains("petir_clausen_reduce"),
            "the reduction was measured and rejected; the shader must not call it"
        );
        assert!(
            !code.contains("XMAXF") && !code.contains("XMAXG"),
            "upstream's far-field guards are not f32 and must stay deleted"
        );

        // All six tables at order_sp: 11, 15, 14, 21, 10, 10 -- 81 against
        // the f64 order's 129.
        let lens: std::vec::Vec<usize> = code
            .match_indices("array<f32, ")
            .map(|(i, _)| {
                code[i + 11..]
                    .split('>')
                    .next()
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            lens,
            std::vec![F1.len(), F2.len(), G1.len(), G2.len(), SI.len(), CI.len()]
        );
        assert_eq!(lens.iter().sum::<usize>(), 81);
    }
}
