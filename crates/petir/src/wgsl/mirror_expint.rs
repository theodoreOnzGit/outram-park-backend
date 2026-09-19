//! `f32` mirror of `shaders/expint.wgsl` — the exponential integrals `E_1`
//! and `Ei`, and the hyperbolic sine and cosine integrals `Shi` and `Chi`.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Four functions, one branch tree
//!
//! `Ei(x) = -E_1(-x)` is upstream's entire definition of `Ei`, and
//! `Shi = (Ei + E_1)/2`, `Chi = (Ei - E_1)/2` are upstream's entire
//! definition of those — except for `Shi`'s own small-argument series, where
//! `E_1` and `Ei` each diverge logarithmically while their sum does not.
//! So the whole module is `E_1`'s six branches plus one seven-term series,
//! and a defect anywhere in `E_1` shows up in all four.
//!
//! That is worth stating because it changes what the tests below are
//! measuring: `the_four_entry_points_agree_with_the_f64_modules` is four
//! sweeps over largely the same code, while
//! `the_identities_hold_in_f32` checks the *combinations*, which is where a
//! transcription slip in the sum or difference would show.
//!
//! # `Chi - Shi = -E_1` cancels, and that is the formula's doing
//!
//! `Shi` and `Chi` both tend to `Ei/2` as `E_1` dies away, so their
//! difference is a small number formed from two large ones. At `x = 4.9`
//! both are about `12` while `E_1` is `1.3e-03`: `f32`'s seven digits leave
//! the difference with about three. The identity is therefore asserted
//! **absolutely, against the size of the operands** — asserting it
//! relatively would be asserting something false about the port.
//!
//! The sum, `Shi + Chi = Ei`, does not cancel and is asserted relatively.
//! The asymmetry between the two is the tell that this is arithmetic rather
//! than a transcription defect: a mis-transcribed branch would spoil both.
//!
//! # `Shi` is exactly odd and `Chi` is exactly even
//!
//! Not to a tolerance — as bit equalities. Both follow from
//! `Ei(-x) = -E_1(x)`, which this implementation satisfies by construction,
//! and `Shi`'s small-argument branch is `x` times a series in `x^2`, which
//! is odd exactly too.
//!
//! `Chi` being **even** is worth stating because it is not a property of
//! `Chi` the function — `Chi` is real only on the positive axis, and what
//! this returns for `x < 0` is the real part of its analytic continuation.
//! It is also what explains this module's single worst GPU figure,
//! `2.327e-05` for `Chi` at `x = -0.525`: that is minus `Chi`'s real zero at
//! `0.5238226`, and relative error is unbounded at a zero for any
//! implementation in any precision. The absolute error there is `6e-08`,
//! one `f32` ulp of the operands.
//!
//! # `order_sp`, and the first series where it buys nothing
//!
//! Six of the seven Chebyshev series ship at GSL's single-precision order:
//! **99 coefficients where the `f64` order needs 157**.
//!
//! | series | `f64` | `order_sp` |
//! |---|---|---|
//! | `AE11` | 39 | 21 |
//! | `AE12` | 25 | 16 |
//! | `E11` | 19 | 14 |
//! | `E12` | 16 | 11 |
//! | `AE13` | 25 | 16 |
//! | `AE14` | 26 | 14 |
//! | **`shi`** | **7** | **7** |
//!
//! `shi_cs` is the first series PETIR has ported whose `order_sp` equals its
//! `f64` order. There is nothing to cut: the series is seven terms long and
//! its last coefficient is `4.67e-22`. Recorded here and asserted in
//! `crate::specfunc::shint`'s
//! `the_single_precision_order_is_the_full_order_and_that_is_upstreams`, so
//! that a full-length table reads as a decision rather than a missed
//! truncation.
//!
//! # Machine constants
//!
//! **`xmax` is retargeted and is a range guard.** `E_1`'s unscaled tail is
//! bounded by `-LOG_MIN - ln(-LOG_MIN)`: `705.342` in `f64`, `82.867` in
//! `f32`. **That number was first written as `82.967`** — an arithmetic slip
//! in the retargeting — and
//! `the_shader_and_this_mirror_agree_on_their_machine_constants` caught it
//! because it *recomputes* the formula instead of restating the literal. A
//! constant that both the shader and the mirror get wrong together is
//! invisible to every accuracy comparison between them; this is the second
//! time that check has earned its place, after `mirror_synchrotron`'s
//! hand-rounded `XSML`. It is kept rather than deleted — unlike `dawson`'s and `sinint`'s
//! far-field guards, which were deleted as unrepresentable — because `E_1`'s
//! prefactor is `exp(-x)/x`, so where the answer underflows depends on `x`
//! twice over and no single multiply reaches zero first.
//!
//! **`xsml` is retargeted and is a precision constant**: `Shi`'s
//! small-argument cut, `sqrt(EPSILON)`, `1.490e-8` in `f64` and `3.453e-4`
//! here.
//!
//! **Prefer the scaled entry points on a GPU, for two reasons and not
//! one.** The documented reason is range: `petir_expint_e1` underflows to
//! zero past `x ~ 83` and `Ei` overflows past `x ~ -83`, while the scaled
//! forms do neither, anywhere.
//!
//! The reason that only showed up on the device is **accuracy**. Measured on
//! `llvmpipe`, the unscaled forms reach `3.3e-06` against the mirror at
//! large `x` where the scaled forms stay at `4.0e-07` — an eight-fold gap,
//! and it is structural rather than incidental. Above `x = 4` the unscaled
//! branch is `exp(-x)/x * (1 + cheb)` and the scaled one is `1/x * (1 +
//! cheb)`: the same arithmetic with one `exp` removed. WGSL specifies its
//! transcendental builtins to an ULP bound rather than to correct rounding,
//! so that one call is the entire difference. The scaled form is not merely
//! safer at the extremes; it is the more accurate function everywhere the
//! two differ.
//!
//! # What `f32` costs
//!
//! Worst relative difference against the `f64` modules, measured 2026-09-20
//! over 400 geometric points in `(0.8, 80]` and 300 linear points in
//! `[-15, 0)`:
//!
//! | entry point | worst relative | at |
//! |---|---|---|
//! | `E_1`, `x > 0` | 1.762e-07 | 55.99 |
//! | `E_1`, `x < 0` | 5.654e-07 | -0.35 |
//! | `E_1` scaled | 1.271e-07 | 3.148 |
//! | `Ei` | 4.834e-07 | 1.067 |
//! | `Shi` | 4.635e-07 | 1.067 |
//! | `Chi` | 5.075e-07 | 1.067 |
//!
//! **Every one is within five `f32` ulps.** `Ei`, `Shi` and `Chi` share
//! their worst point exactly, at `x = 1.067`, and that is not a
//! coincidence: all three are dominated by `Ei(x) = -E_1(-x)` there, which
//! puts the argument just past `-1` into `E_1`'s `E11` branch — the one
//! whose `order_sp` cut is proportionally the smallest (19 to 14). The same
//! point being worst for three functions is what a shared implementation
//! looks like, and is the reason the identity test below exists separately.

// Under a std-linked build (`cargo test`) f32's inherent exp/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `-LOG_MIN - ln(-LOG_MIN)` at `f32`'s range. Mirrors
/// `PETIR_EXPINT_XMAX`.
const XMAX: f32 = 82.866776;

/// `sqrt(f32::EPSILON)`, `Shi`'s small-argument cut. Mirrors
/// `PETIR_EXPINT_SHI_XSML`.
const XSML: f32 = 3.4526698e-4;

/// GSL's `AE11_cs` at its single-precision order, 21 coefficients.
/// Mirrors `petir_expint_cheb_ae11`.
const AE11: [f32; 21] = [
    0.12150324136018753,
    -0.06508877873420715,
    0.004897651262581348,
    -0.000649237830657512,
    9.384043369209394e-05,
    4.2023637547572434e-07,
    -8.11337486084085e-06,
    2.804247742460575e-06,
    5.648716339123894e-08,
    -3.448091661084618e-07,
    5.820927384547758e-08,
    3.871142695288654e-08,
    -1.2453234887743747e-08,
    -5.1185047311719245e-09,
    2.1487716050927474e-09,
    8.684599150932115e-10,
    -3.4365010836978627e-10,
    -1.7979660815736764e-10,
    4.7442060696623045e-11,
    4.0423282776647085e-11,
    -3.543927954915982e-12,
];

/// GSL's `AE12_cs` at its single-precision order, 16 coefficients.
/// Mirrors `petir_expint_cheb_ae12`.
const AE12: [f32; 16] = [
    0.5824174880981445,
    -0.15834884345531464,
    -0.00676427548751235,
    0.005125843919813633,
    0.00043523250496946275,
    -0.00014361336070578545,
    -4.1801322367973626e-05,
    -2.7133958155900473e-06,
    1.1513818662933772e-06,
    4.2065002503477444e-07,
    6.658190443431522e-08,
    6.621437842468936e-10,
    -2.8441049515492978e-09,
    -9.407241652326093e-10,
    -1.7747660285838407e-10,
    -1.5830222549473305e-11,
];

/// GSL's `E11_cs` at its single-precision order, 14 coefficients.
/// Mirrors `petir_expint_cheb_e11`.
const E11: [f32; 14] = [
    -16.113462448120117,
    7.79407262802124,
    -1.955405831336975,
    0.3733729422092438,
    -0.056925032287836075,
    0.007211077958345413,
    -0.000781049020588398,
    7.388093217741698e-05,
    -6.2028620959608816e-06,
    4.6816001031402266e-07,
    -3.209288834682411e-08,
    2.015199784821675e-09,
    -1.1673687017044188e-10,
    6.2762707357666425e-12,
];

/// GSL's `E12_cs` at its single-precision order, 11 coefficients.
/// Mirrors `petir_expint_cheb_e12`.
const E12: [f32; 11] = [
    -0.03739021345973015,
    0.04272398725152016,
    -0.13031820952892303,
    0.014419124461710453,
    -0.0013461707858368754,
    0.00010731029033195227,
    -7.4299996413174085e-06,
    4.537732536391559e-07,
    -2.4764172934510498e-08,
    1.2207658217633366e-09,
    -5.4851415076662136e-11,
];

/// GSL's `AE13_cs` at its single-precision order, 16 coefficients.
/// Mirrors `petir_expint_cheb_ae13`.
const AE13: [f32; 16] = [
    -0.6057732701301575,
    -0.11253524571657181,
    0.013432266190648079,
    -0.001926845172420144,
    0.00030911833164282143,
    -5.3564133850159124e-05,
    9.827813300944399e-06,
    -1.8853689880415914e-06,
    3.7494319826691935e-07,
    -7.682345426474058e-08,
    1.6143269832014084e-08,
    -3.466802178664352e-09,
    7.587542261155988e-10,
    -1.6886433917839838e-10,
    3.814570534443895e-11,
    -8.733025587404075e-12,
];

/// GSL's `AE14_cs` at its single-precision order, 14 coefficients.
/// Mirrors `petir_expint_cheb_ae14`.
const AE14: [f32; 14] = [
    -0.1892918050289154,
    -0.08648117631673813,
    0.007224101573228836,
    -0.0008097559330053627,
    0.0001099913424695842,
    -1.717332997941412e-05,
    2.9856275887141237e-06,
    -5.659649104927666e-07,
    1.1526808663120391e-07,
    -2.4950304933213374e-08,
    5.692324389627856e-09,
    -1.3599577020073639e-09,
    3.384662827787821e-10,
    -8.737852802420676e-11,
];

/// GSL's `SHI_cs` at its single-precision order, 7 coefficients.
/// Mirrors `petir_expint_cheb_shi`.
const SHI: [f32; 7] = [
    0.007837268523871899,
    0.003922766540199518,
    4.134678874834208e-06,
    2.470748050598104e-09,
    9.379295300496193e-13,
    2.4518170692597655e-16,
    4.6700000133587036e-20,
];

/// Clenshaw in GSL's convention — the same helper every generated mirror
/// here uses, taking the series as a slice so no index can be out of range.
///
/// Mirrors the seven `petir_expint_cheb_*` functions, which are separate in
/// WGSL only because the language has no slices: each embeds its own array.
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

/// `E_1(x)` in `f32`. Mirrors `petir_expint_e1`.
///
/// `NaN` at `x = 0` where `E_1` diverges, and `0` past [`XMAX`] where the
/// `f32` arithmetic has underflowed. Overflows to an infinity below
/// `x ~ -83`, which is where upstream reports `OVERFLOW_ERROR`; prefer
/// [`e1_scaled`] there.
pub fn e1(x: f32) -> f32 {
    e1_impl(x, false)
}

/// `exp(x) E_1(x)`. Mirrors `petir_expint_e1_scaled`. In range everywhere
/// [`e1`] is not.
pub fn e1_scaled(x: f32) -> f32 {
    e1_impl(x, true)
}

fn e1_impl(x: f32, scale: bool) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x <= -10.0 {
        let s = 1.0 / x * if scale { 1.0 } else { (-x).exp() };
        return s * (1.0 + cheb(&AE11, 20.0 / x + 1.0));
    }
    if x <= -4.0 {
        let s = 1.0 / x * if scale { 1.0 } else { (-x).exp() };
        return s * (1.0 + cheb(&AE12, (40.0 / x + 7.0) / 3.0));
    }
    if x <= -1.0 {
        let ln_term = -x.abs().ln();
        let sf = if scale { x.exp() } else { 1.0 };
        return sf * (ln_term + cheb(&E11, (2.0 * x + 5.0) / 3.0));
    }
    if x == 0.0 {
        return f32::NAN;
    }
    if x <= 1.0 {
        let ln_term = -x.abs().ln();
        let sf = if scale { x.exp() } else { 1.0 };
        // The 0.6875 is upstream's; it folds the series' own offset.
        return sf * (ln_term - 0.6875 + x + cheb(&E12, x));
    }
    if x <= 4.0 {
        let s = 1.0 / x * if scale { 1.0 } else { (-x).exp() };
        return s * (1.0 + cheb(&AE13, (8.0 / x - 5.0) / 3.0));
    }
    if x <= XMAX || scale {
        let s = 1.0 / x * if scale { 1.0 } else { (-x).exp() };
        return s * (1.0 + cheb(&AE14, 8.0 / x - 1.0));
    }
    0.0
}

/// `Ei(x) = -E_1(-x)`, upstream's whole definition. Mirrors
/// `petir_expint_ei`.
pub fn ei(x: f32) -> f32 {
    -e1(-x)
}

/// `exp(-x) Ei(x)`. Mirrors `petir_expint_ei_scaled`.
pub fn ei_scaled(x: f32) -> f32 {
    -e1_scaled(-x)
}

/// `Shi(x) = integral_0^x sinh(t)/t dt`, odd in `x`. Mirrors `petir_shi`.
pub fn shi(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    let ax = x.abs();
    if ax < XSML {
        return x;
    }
    if ax <= 0.375 {
        return x * (1.0 + cheb(&SHI, 128.0 * x * x / 9.0 - 1.0));
    }
    0.5 * (ei(x) + e1(x))
}

/// `Chi(x)`. Real for `x > 0`; upstream returns a value for `x < 0` too and
/// that is carried. Mirrors `petir_chi`.
pub fn chi(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    0.5 * (ei(x) - e1(x))
}

/// The six entry points behind one selector, in the order `E_1`,
/// `E_1` scaled, `Ei`, `Ei` scaled, `Shi`, `Chi`. Mirrors
/// `petir_expint_family`, which is what the GPU test dispatches.
pub fn expint_family(which: u32, x: f32) -> f32 {
    match which {
        0 => e1(x),
        1 => e1_scaled(x),
        2 => ei(x),
        3 => ei_scaled(x),
        4 => shi(x),
        5 => chi(x),
        _ => f32::NAN,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::shint as f64_shint;

    /// Worst relative difference of an `f32` surface against its `f64`
    /// counterpart over a sweep, skipping where the reference is too small
    /// for a relative figure to mean anything.
    fn worst<F, G>(pts: &[f32], f: F, g: G, floor: f64) -> (f64, f32)
    where
        F: Fn(f32) -> f32,
        G: Fn(f64) -> Option<f64>,
    {
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for &x in pts {
            let Some(r) = g(x as f64) else { continue };
            if !r.is_finite() || r.abs() < floor {
                continue;
            }
            let v = f(x);
            if !v.is_finite() {
                continue;
            }
            let e = ((v as f64 - r) / r).abs();
            if e > w {
                w = e;
                at = x;
            }
        }
        (w, at)
    }

    /// **What `f32` costs, per entry point.**
    ///
    /// Filled in from the measured run; the budget asserted is an order
    /// above each so this fails on a regression rather than on the last bit.
    #[test]
    fn the_four_entry_points_agree_with_the_f64_modules() {
        // Positive side, geometric across all four positive branches
        // (x <= 1, x <= 4, x <= XMAX) and into the tail.
        let pos: std::vec::Vec<f32> = (1..=400)
            .map(|i| (0.01_f32).powf(1.0 - i as f32 / 400.0) * 80.0)
            .collect();
        // Negative side, where E_1 takes its other three branches.
        let neg: std::vec::Vec<f32> = (1..=300).map(|i| -0.05 * i as f32).collect();

        let (we1p, ae1p) = worst(
            &pos,
            e1,
            |x| crate::expint::expint_e1(x).ok().map(|t| t.0),
            0.0,
        );
        let (we1n, ae1n) = worst(
            &neg,
            e1,
            |x| crate::expint::expint_e1(x).ok().map(|t| t.0),
            0.0,
        );
        let (wes, aes) = worst(
            &pos,
            e1_scaled,
            |x| crate::expint::expint_e1_scaled(x).ok().map(|t| t.0),
            0.0,
        );
        let (wei, aei) = worst(
            &pos,
            ei,
            |x| crate::expint::expint_ei(x).ok().map(|t| t.0),
            0.0,
        );
        let (wshi, ashi) = worst(&pos, shi, |x| Some(f64_shint::shi(x)), 0.0);
        let (wchi, achi) = worst(&pos, chi, |x| Some(f64_shint::chi(x)), 1e-3);

        std::println!(
            "E_1+ {we1p:e}@{ae1p}  E_1- {we1n:e}@{ae1n}  E_1s {wes:e}@{aes}  \
             Ei {wei:e}@{aei}  Shi {wshi:e}@{ashi}  Chi {wchi:e}@{achi}"
        );
        for (name, w, at, budget) in [
            ("E_1 (x > 0)", we1p, ae1p, 1e-4),
            ("E_1 (x < 0)", we1n, ae1n, 1e-4),
            ("E_1 scaled", wes, aes, 1e-4),
            ("Ei", wei, aei, 1e-3),
            ("Shi", wshi, ashi, 1e-4),
            ("Chi", wchi, achi, 1e-4),
        ] {
            assert!(w < budget, "{name}: {w:e} at {at}, budget {budget:e}");
        }
    }

    /// **The identities hold in `f32`**, which is the part the four sweeps
    /// above cannot check: they all run largely the same code, so a
    /// transcription slip in the *combination* would be invisible to them.
    ///
    /// `Ei(x) = -E_1(-x)` is exact by construction here and is asserted as
    /// bit equality, not a tolerance — if it ever fails, the two entry
    /// points have stopped sharing an implementation.
    #[test]
    fn the_identities_hold_in_f32() {
        for i in 1..=200 {
            let x = 0.05 * i as f32;
            // Exact by construction.
            assert_eq!(ei(x), -e1(-x), "Ei(x) = -E_1(-x) at {x}");
            // Shi + Chi = Ei, and Chi - Shi = -E_1.
            let (s, c, e) = (shi(x), chi(x), ei(x));
            if e.is_finite() && e.abs() > 1e-3 {
                assert!(
                    ((s + c - e) / e).abs() < 1e-4,
                    "Shi + Chi = Ei at {x}: {} against {e}",
                    s + c
                );
            }
            // Chi - Shi = -E_1, but measured ABSOLUTELY and against the
            // scale of the operands. Chi and Shi both tend to Ei/2 as E_1
            // dies, so their difference is a small number formed from two
            // large ones: at x = 4.9, Shi and Chi are both about 12 while
            // E_1 is 1.3e-03, so f32's 7 digits leave the difference with
            // about 3. That is CANCELLATION IN THE IDENTITY, not error in
            // either function, and asserting it relatively would be
            // asserting something false about the port.
            let e1v = e1(x);
            if e1v.is_finite() {
                let scale = s.abs().max(c.abs()).max(1.0);
                assert!(
                    (c - s + e1v).abs() < 1e-5 * scale,
                    "Chi - Shi = -E_1 at {x}: {} against {}, operands of size {scale:e}",
                    c - s,
                    -e1v
                );
            }
        }
        // Shi is odd and Chi is even -- EXACTLY, not to a tolerance.
        //
        // Both follow from Ei(-x) = -E_1(x) and E_1(-x) = -Ei(x), which this
        // implementation satisfies by construction:
        //     Shi(-x) = (Ei(-x) + E_1(-x))/2 = -(E_1(x) + Ei(x))/2 = -Shi(x)
        //     Chi(-x) = (Ei(-x) - E_1(-x))/2 =  (Ei(x) - E_1(x))/2 =  Chi(x)
        // and Shi's own small-argument branch is x times a series in x^2,
        // which is odd exactly as well. So these are bit equalities, and a
        // tolerance here would be hiding the fact.
        //
        // Chi being EVEN is also what explains its worst GPU figure, at
        // x = -0.525: that is minus its real zero, where relative error is
        // unbounded for any implementation at any width.
        for i in 1..=50 {
            let x = 0.2 * i as f32;
            assert_eq!(shi(-x), -shi(x), "Shi is exactly odd; at {x}");
            assert_eq!(chi(-x), chi(x), "Chi is exactly even; at {x}");
        }
        // Including across the small-argument branch, where Shi takes its
        // own series rather than the E_1/Ei combination.
        for i in 1..=40 {
            let x = 0.005 * i as f32;
            assert_eq!(shi(-x), -shi(x), "Shi is exactly odd below 0.375; at {x}");
        }
    }

    /// **The scaled forms are in range everywhere the unscaled ones are
    /// not**, which is the reason to prefer them on a GPU and is therefore
    /// asserted rather than merely documented.
    #[test]
    fn the_scaled_forms_stay_in_range_where_the_plain_ones_do_not() {
        // E_1 underflows to zero past XMAX; its scaled form does not.
        assert_eq!(e1(XMAX + 1.0), 0.0);
        assert!(
            e1_scaled(XMAX + 1.0) > 0.0 && e1_scaled(XMAX + 1.0).is_finite(),
            "E_1 scaled past XMAX is {}",
            e1_scaled(XMAX + 1.0)
        );
        // And far beyond, where exp(-x) has no chance at all.
        for x in [200.0_f32, 1e4, 1e30] {
            assert_eq!(e1(x), 0.0, "E_1({x})");
            assert!(e1_scaled(x) > 0.0, "E_1 scaled({x}) = {}", e1_scaled(x));
            // Ei is the mirror image: it overflows where E_1 underflows.
            assert!(ei_scaled(-x).is_finite(), "Ei scaled({}) ", -x);
        }
        // The scaled form is asymptotically 1/x, which is the check that it
        // is not merely finite but right.
        for x in [1e3_f32, 1e5, 1e7] {
            let r = e1_scaled(x) * x;
            assert!(
                (r - 1.0).abs() < 1e-3,
                "x E_1_scaled(x) -> 1; at {x} it is {r}"
            );
        }
    }

    /// The refusals, which are upstream's.
    #[test]
    fn the_refusals_match_the_f64_modules() {
        assert!(e1(0.0).is_nan(), "E_1(0)");
        assert!(chi(0.0).is_nan(), "Chi(0)");
        assert!(ei(0.0).is_nan(), "Ei(0)");
        // Shi's own series covers the origin, so it does not refuse there.
        assert_eq!(shi(0.0), 0.0);
        for f in [e1 as fn(f32) -> f32, e1_scaled, ei, ei_scaled, shi, chi] {
            assert!(f(f32::NAN).is_nan());
        }
        // The dispatcher agrees with what it dispatches to, and refuses an
        // unknown selector.
        for x in [0.5_f32, 2.0, 30.0] {
            assert_eq!(expint_family(0, x), e1(x));
            assert_eq!(expint_family(1, x), e1_scaled(x));
            assert_eq!(expint_family(2, x), ei(x));
            assert_eq!(expint_family(3, x), ei_scaled(x));
            assert_eq!(expint_family(4, x), shi(x));
            assert_eq!(expint_family(5, x), chi(x));
            assert!(expint_family(6, x).is_nan());
        }
    }

    /// The shader and this mirror agree on their two machine constants, and
    /// both are what their formulae give at `f32`'s range.
    ///
    /// The tables are generated from one parse of the `f64` modules, so they
    /// are covered by `every_generated_shader_and_its_mirror_hold_the_same_constants`
    /// in `tests/wgsl_validation.rs`; these two are hand-written and are not.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_machine_constants() {
        let src = crate::wgsl::EXPINT;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in expint.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the constant name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_EXPINT_XMAX: f32"), XMAX);
        assert_eq!(read("PETIR_EXPINT_SHI_XSML: f32"), XSML);

        // And both are the formulae, recomputed rather than restated.
        let log_min = f64::from(f32::MIN_POSITIVE).ln();
        let xmaxt = -log_min;
        let want = xmaxt - xmaxt.ln();
        assert!(
            ((XMAX as f64 - want) / want).abs() < 1e-6,
            "XMAX is {XMAX:e}, -LOG_MIN - ln(-LOG_MIN) gives {want:e}"
        );
        let want_xsml = f64::from(f32::EPSILON).sqrt();
        assert!(
            ((XSML as f64 - want_xsml) / want_xsml).abs() < 1e-6,
            "XSML is {XSML:e}, sqrt(f32::EPSILON) gives {want_xsml:e}"
        );
    }
}
