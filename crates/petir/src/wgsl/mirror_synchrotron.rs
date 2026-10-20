//! `f32` mirrors of `shaders/synchrotron.wgsl` — GSL's synchrotron radiation
//! functions `S_1` and `S_2`.
//!
//! Generated from the same parse of [`crate::specfunc::synchrotron`] that
//! produced the shader, so the two cannot drift in their 91 coefficients.
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # One constant is retargeted, one is kept — and the first answer was wrong
//!
//! | constant | `f64` | `f32` | kind |
//! |---|---|---|---|
//! | `2 sqrt(2) sqrt(EPSILON)` | 4.21e-08 | 9.765625e-04 (`2^-10`) | precision — **retargeted** |
//! | `-8 ln(MIN)/7` | 809.5959 | 809.5959 | range guard — **kept** |
//!
//! **The range guard was retargeted first, to 99.813194, and that was
//! wrong.** Measured in `f32`:
//!
//! | | |
//! |---|---|
//! | retargeted guard fires at | 99.8132 |
//! | `exp(c0 - x)` reaches zero at | **104.198** |
//! | value just below the retargeted guard | 5.65e-43 (a representable denormal) |
//! | answers changed by moving the guard out, `x` in `[95, 107]` | **1461 of 4001** |
//!
//! Between 99.81 and 104.20 the function still returns representable
//! denormals — `f32` holds down to 1.4e-45 — so the retargeted guard was
//! **discarding answers that exist**. That is
//! [`crate::wgsl::mirror_airy`]'s case exactly, and the taxonomy already says
//! it: a guard derived from the **exponent range** is enforced by the
//! arithmetic anyway, so retargeting it can only take away.
//!
//! The `f64` module reaches the opposite conclusion about the same constant
//! for the same reason — there the guard sits at 809.5959 while the
//! arithmetic reaches zero at 745.3590, making it 64 units of dead code.
//! **Dead at `f64`, harmful if retargeted at `f32`, correct as written at
//! both.** `the_range_guard_is_kept_because_retargeting_it_discards_answers`
//! measures every row above.
//!
//! # And the GPU puts it in a third place again
//!
//! Measured on llvmpipe (LLVM 20.1.2), 2026-09-19:
//!
//! | | `f64` CPU | `f32` CPU | `f32` GPU |
//! |---|---|---|---|
//! | upstream's guard fires at | 809.5959 | 809.5959 | 809.5959 |
//! | the arithmetic reaches zero at | 745.3590 | 104.1979 | **87.57** |
//!
//! Not denormal flushing in general — `x * 1e-30` returns 1e-44 on that
//! device. It is **`exp` alone**, which returns exactly zero the moment its
//! own result would be denormal (`exp(-87)` = 1.6458e-38, `exp(-88)` = 0).
//! Since the exponential is multiplied by a prefactor of order 10, answers
//! that are ordinary normal `f32` are lost: at `x = 87.57` this mirror gives
//! 1.1010e-37 and the device gives 0.
//!
//! **Three backends, three underflow points, none of them 809.5959** — which
//! is the whole argument for keeping upstream's constant. A guard retargeted
//! to any one of them is wrong on the other two; one that never fires lets
//! each backend underflow wherever its own arithmetic does.
//! `tests/wgsl_gpu.rs` asserts the tail as a shape for that reason.

// Under a std-linked build (`cargo test`) f32's inherent powf/exp/sqrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `2 sqrt(2) sqrt(f32::EPSILON)`, exactly `2^-10`. Mirrors
/// `PETIR_SYNCH_SMALL_CUT`.
const SMALL_CUT: f32 = 0.0009765625;
/// GSL's `-8 ln(DBL_MIN)/7`, **kept at its `f64` value on purpose** — see
/// the module documentation. Mirrors `PETIR_SYNCH_UNDERFLOW_CUT`.
const UNDERFLOW_CUT: f32 = 809.5959;
/// The `f32` analogue of that bound, which this module deliberately does
/// **not** use. Kept so the comparison can be measured.
#[cfg(test)]
const F32_UNDERFLOW_CUT: f32 = 99.813194;
/// `pi / sqrt(3)`. Mirrors `PETIR_SYNCH_PI_OVER_SQRT3`.
const PI_OVER_SQRT3: f32 = 1.8137994;
/// `ln(sqrt(pi/2))`. Mirrors `PETIR_SYNCH_LOG_SQRT_PI_2`.
const LOG_SQRT_PI_2: f32 = 0.22579135;

/// GSL's `synch1_cs`, 13 coefficients.
#[rustfmt::skip]
const SYNCH1: [f32; 13] = [
    30.364683151245117, 17.079395294189453, 4.560132026672363, 0.5492812395095825,
    0.037297606468200684, 0.00161362427752465, 4.8191675887210295e-05, 1.0512425205888576e-06,
    1.7463850809917858e-08, 2.2815486999672174e-10, 2.4044308211124132e-12,
    2.0865880773430004e-14, 1.5166999746804084e-16,
];

/// GSL's `synch2_cs`, 12 coefficients.
#[rustfmt::skip]
const SYNCH2: [f32; 12] = [
    0.4490721523761749, 0.08983536809682846, 0.008104457519948483, 0.0004261716967448592,
    1.4760963495064061e-05, 3.628633749030996e-07, 6.663480878188466e-09,
    9.490771363251937e-11, 1.079124962131972e-12, 1.0021999811255736e-14,
    7.69999999087401e-17, 5.000000229068525e-19,
];

/// GSL's `synch1a_cs`, 23 coefficients.
#[rustfmt::skip]
const SYNCH1A: [f32; 23] = [
    2.1329305171966553, 0.07413528859615326, 0.008696810342371464, 0.0011703826021403074,
    0.00016451058036182076, 2.4020102500799112e-05, 3.5827756619255524e-06,
    5.447747639664158e-07, 8.388028760464294e-08, 1.3069882953686829e-08,
    2.0530990241240943e-09, 3.2518754355947976e-10, 5.179140449840247e-11,
    8.30029881632166e-12, 1.3352728211318832e-12, 2.1591500018024873e-13,
    3.4996701039511843e-14, 5.6994000990039095e-15, 9.290999578105198e-16,
    1.519999962103352e-16, 2.4899999248585602e-17, 4.100000115457887e-18,
    6.9999999071056285e-19,
];

/// GSL's `synch21_cs`, 13 coefficients.
#[rustfmt::skip]
const SYNCH21: [f32; 13] = [
    38.61783981323242, 23.037715911865234, 5.380249977111816, 0.6156793832778931,
    0.040668800473213196, 0.0017296274891123176, 5.106125900056213e-05,
    1.1045959809052874e-06, 1.8235530419019597e-08, 2.370769691673047e-10,
    2.4887294915870717e-12, 2.1529000844978767e-14, 1.5600000342462522e-16,
];

/// GSL's `synch22_cs`, 13 coefficients.
#[rustfmt::skip]
const SYNCH22: [f32; 13] = [
    7.906314849853516, 3.1353464126586914, 0.4854879379272461, 0.039481665939092636,
    0.0019661621190607548, 6.59078941680491e-05, 1.5857560811127769e-06,
    2.8686530484378636e-08, 4.0412023727398605e-10, 4.556844485081868e-12,
    4.2045898961942316e-14, 3.2320000587343883e-16, 2.100000023830477e-18,
];

/// GSL's `synch2a_cs`, 17 coefficients.
#[rustfmt::skip]
const SYNCH2A: [f32; 17] = [
    2.0203371047973633, 0.010956237092614174, 0.0008542384603060782, 7.234302756842226e-05,
    6.312443019851344e-06, 5.648192882290459e-07, 5.128324787051497e-08,
    4.719653112772448e-09, 4.3807441008070214e-10, 4.102681389062113e-11,
    3.862307175472868e-12, 3.66132308932815e-13, 3.4802300201853403e-14,
    3.3301000794931515e-15, 3.189999930044784e-16, 3.070000110662777e-17,
    3.0000000340435383e-18,
];

/// `x^n` for the small integer powers. Mirrors `petir_synch_pow_int`.
fn pow_int(x: f32, n: u32) -> f32 {
    let mut out = 1.0_f32;
    for _ in 0..n {
        out *= x;
    }
    out
}

/// Clenshaw in GSL's convention, on one of the six tables.
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

/// `S_1(x)` in `f32`. Mirrors `petir_synchrotron_1`.
pub fn synchrotron_1(x: f32) -> f32 {
    s1_with(x, UNDERFLOW_CUT)
}

/// [`synchrotron_1`] with the underflow cut supplied, so its role can be
/// **measured** rather than argued. [`UNDERFLOW_CUT`] ships.
fn s1_with(x: f32, cut: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x < SMALL_CUT {
        let z = Real::powf(x, 1.0 / 3.0);
        return 2.1495283 * z * (1.0 - 0.84381276 * z * z);
    }
    if x <= 4.0 {
        let px = Real::powf(x, 1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        return px * cheb(&SYNCH1, t) - pow_int(px, 11) * cheb(&SYNCH2, t) - PI_OVER_SQRT3 * x;
    }
    if x < cut {
        let t = (12.0 - x) / (x + 4.0);
        return Real::sqrt(x) * cheb(&SYNCH1A, t) * Real::exp(LOG_SQRT_PI_2 - x);
    }
    0.0
}

/// `S_2(x)` in `f32`. Mirrors `petir_synchrotron_2`.
pub fn synchrotron_2(x: f32) -> f32 {
    s2_with(x, UNDERFLOW_CUT)
}

fn s2_with(x: f32, cut: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x < SMALL_CUT {
        let z = Real::powf(x, 1.0 / 3.0);
        return 1.0747641 * z * (1.0 - 1.1776716 * z * x);
    }
    if x <= 4.0 {
        let px = Real::powf(x, 1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        return px * cheb(&SYNCH21, t) - pow_int(px, 5) * cheb(&SYNCH22, t);
    }
    if x < cut {
        let t = (10.0 - x) / (x + 2.0);
        return Real::sqrt(x) * Real::exp(LOG_SQRT_PI_2 - x) * cheb(&SYNCH2A, t);
    }
    0.0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::synchrotron as f64_synch;
    // The shared budget machinery, moved out of this module (gh #1003).
    use crate::wgsl::log_budget::{cheb_budget, log_err, ulp_over, ulps_up, worst_ulps, Sh, U32};

    /// The geometric probe grid on `[lo, hi]`, `n + 1` points. ~~Built with
    /// `f32::powf` (method form)~~ **CHANGED 2026-10-20 (gh #1003):** with
    /// `Real::powf` (UFCS), because the method form resolved to glibc in this
    /// std-linked test build (gh #1001), so the probe points depended on the
    /// platform. The points move by an ulp here and there; nothing measured
    /// depends on exactly where they fall.
    fn grid(lo: f32, hi: f32, n: u32) -> impl Iterator<Item = f32> {
        (0..=n).map(move |k| lo * Real::powf(hi / lo, k as f32 / n as f32))
    }

    /// The small-argument branch, `x < 2^-10`, as a running error bound.
    /// `k` moves `powf(x, 1/3)` by `k` ulps (the negative control's defect).
    ///
    /// Besides the arithmetic it carries the TRUNCATION of upstream's
    /// two-term form, which the `f64` reference does not share (it uses the
    /// Chebyshev branch down to `4.2e-8`). From the Bessel series:
    ///
    /// - `S_2 = x K_{2/3}(x) = (pi/sqrt 3)[2^{2/3} x^{1/3}/Gamma(1/3) (1 +
    ///   3x^2/4 + ...) - 2^{-2/3} x^{5/3}/Gamma(5/3) (1 + 3x^2/20 + ...)]`,
    ///   so the two-term form `c z (1 - c' z x)` leaves out `3x^2/4`
    ///   relative, less `(3/20) c' x^{10/3}`.
    /// - `S_1 = 2 x K_{2/3}(x) - (pi/sqrt 3) x + x int_0^x K_{1/3}` (from
    ///   `K_{5/3} = -2 K'_{2/3} - K_{1/3}` and `int_0^inf K_{1/3} =
    ///   pi/sqrt 3`): the `x^{5/3}` terms cancel exactly, and the first one
    ///   left out is `x^{7/3}` with coefficient `(pi/sqrt 3)(3/4)(2^{5/3}/
    ///   Gamma(1/3) - 2^{-1/3}/Gamma(4/3)) = 0.4030`, i.e. `(3/16) x^2`
    ///   relative.
    ///
    /// Bounded by `c x^2 + x^{10/3}` relative (`c` = 3/16, 3/4), and TESTED
    /// against the `f64` module in `f64` arithmetic.
    fn small_budget(x: f32, s1: bool, k: i32) -> Sh {
        let xs = Sh::exact(x);
        let z = xs.powf(1.0 / 3.0, 1.0 / 3.0).bump(k);
        let one = Sh::exact(1.0);
        let r = if s1 {
            let c = Sh::constant(2.1495283, 2.149_528_241_534_478_636_71);
            let c2 = Sh::constant(0.84381276, 8.438_127_628_132_05e-1);
            c.mul(z).mul(one.sub(c2.mul(z).mul(z)))
        } else {
            let c = Sh::constant(1.0747641, 1.074_764_120_767_239_318_36);
            let c2 = Sh::constant(1.1776716, 1.177_671_565_102_35);
            c.mul(z).mul(one.sub(c2.mul(z).mul(xs)))
        };
        Sh {
            e: r.e + r.v.abs() * small_truncation(f64::from(x), s1),
            ..r
        }
    }

    /// The relative truncation bound of the two-term small-`x` form (see
    /// [`small_budget`]).
    fn small_truncation(x: f64, s1: bool) -> f64 {
        let c = if s1 { 3.0 / 16.0 } else { 0.75 };
        c * x * x + Real::powf(x, 10.0 / 3.0)
    }

    /// The exponential tail, `x > 4`, as a running error bound. `k` moves the
    /// exp ARGUMENT `ln sqrt(pi/2) - x` by `k` ulps (the negative control's
    /// defect: one more rounding of the exponent than the code makes).
    fn tail_budget(x: f32, s1: bool, k: i32) -> Sh {
        let xs = Sh::exact(x);
        let arg = Sh::constant(LOG_SQRT_PI_2, f64_synch::PROBE_LOG_SQRT_PI_2)
            .sub(xs)
            .bump(k);
        if s1 {
            let t = Sh::exact(12.0).sub(xs).div(xs.add(Sh::exact(4.0)));
            xs.sqrt()
                .mul(t.cheb(&SYNCH1A, &f64_synch::PROBE_SYNCH1A))
                .mul(arg.exp())
        } else {
            let t = Sh::exact(10.0).sub(xs).div(xs.add(Sh::exact(2.0)));
            xs.sqrt()
                .mul(arg.exp())
                .mul(t.cheb(&SYNCH2A, &f64_synch::PROBE_SYNCH2A))
        }
    }

    /// What `f32` costs, against the `f64` module, over 200 000 geometric
    /// probes on `[1e-6, 100]`.
    ///
    /// ~~**6.09e-04 for `S_1` and 6.13e-04 for `S_2`**, both at `x = 3.976`
    /// (measured 2026-09-19)~~ **Re-measured 2026-10-20 through the shipped
    /// `libm` (gh #1001): 6.82e-04 for `S_1` at `x = 3.9895` and 6.13e-04
    /// for `S_2` at `x = 3.9759`.** The 2026-09-19 figures went through
    /// glibc's `powf`: before gh #1001 std shadowed the mirror's maths in the
    /// test build.
    ///
    /// That is far worse than the roughly one-ulp figure the other mirrors in
    /// this module report, and it is **not** a transcription defect — see
    /// `the_f32_cost_is_cancellation_at_the_branch_boundary`, which measures
    /// the mechanism. Away from that boundary the mirror is ordinary: the
    /// small-argument branch is good to 8.6e-07 (`S_2`, unchanged when
    /// re-measured; `S_1` 3.4e-07) and the exponential tail to ~~6.4e-05~~
    /// **8.2e-07. CORRECTED 2026-10-20:** the 6.4e-05 "tail" figure (6.39e-05
    /// re-measured) was the point `x = 4` itself, which the sweep on
    /// `[4, 100]` included and which belongs to the cancelling `x <= 4`
    /// branch. The tail proper, `x > 4`, is measured below.
    ///
    /// ~~The exact worst value is grid-dependent — a denser sweep finds
    /// 7.3e-04 at `x = 3.9985` — so the bound, not the number, is what is
    /// asserted.~~ **Re-measured 2026-10-20 over EVERY `f32` in `[3.9, 4]`
    /// through libm: `S_1` 8.90e-04 at 3.99126, `S_2` 8.58e-04 at 3.99410**
    /// (0.349 and 0.306 of their log-error budgets). The worst is still
    /// grid-dependent; what is asserted now is the budget at every probe.
    ///
    /// # The instrument: log error against derived budgets (gh #1003)
    ///
    /// ~~Asserted: relative error under `1e-3` over the whole range, under
    /// `1e-6` on the small-argument branch (`[1e-6, 9.7e-4]`), under `1e-4`
    /// on `[4, 100]`.~~ **REPLACED 2026-10-20 (gh #1003).** All three bounds
    /// were set a little above measured numbers, not derived (`S_2`'s small
    /// branch sat at 8.6e-7 against 1e-6), and measured through glibc. The
    /// tail is `sqrt(x) C(t) exp(ln sqrt(pi/2) - x)`: rounding the exp
    /// argument costs `u |argument|`, up to `69 u` of the result's log at
    /// `x = 69`, which a relative bound fixed in advance cannot follow.
    /// Maintainer direction 2026-10-20: exp error propagation is measured in
    /// log error.
    ///
    /// **The metric now** is `|ln S_f32 - ln S_f64|` at every probe (where
    /// the `f64` value exceeds 1e-30, as before), against a budget derived
    /// for the branch the probe falls in. This test covers the WHOLE range,
    /// one budget per branch:
    ///
    /// - **`x < 2^-10`** ([`small_budget`]): `z = powf(x, 1/3)` with the
    ///   exponent's rounding times `|ln x|` (up to 13.8 here, so `1.4e-7`,
    ///   the largest single term) plus `powf`'s own ulp; the four roundings
    ///   of `c z (1 - c' z z)`; both constants' rounding; and the two-term
    ///   form's truncation, `(3/16) x^2` for `S_1` and `(3/4) x^2` for `S_2`.
    ///   That last is upstream's: `2 sqrt(2) sqrt(eps)` makes `x^2/8 = eps`,
    ///   right for `S_1`'s `3/16` but six times short for `S_2`'s `3/4`,
    ///   which is why `S_2`'s small branch is the worse of the two.
    /// - **`2^-10 <= x <= 4`**: `branch_log_budget`, derived and documented
    ///   in `the_f32_cost_is_cancellation_at_the_branch_boundary`, applied
    ///   here at every probe of the geometric sweep.
    /// - **`x > 4`** ([`tail_budget`]): `t`'s three roundings through
    ///   `|C'(t)|`, Clenshaw, `sqrt`, two multiplies, and `exp` of an
    ///   argument carrying `|ln sqrt(pi/2)' - ln sqrt(pi/2)| + u |argument|`.
    ///
    /// The assumptions, TESTED below: `powf(x, f32(1/3))` faithful on
    /// `[1e-6, 2^-10]` (the cancellation test checks `[2^-10, 4]`), `expf`
    /// faithful on `[-87, -3.7]`, and the two truncation coefficients,
    /// against the `f64` module at `x = 2^-10, 2^-11, 2^-12`.
    ///
    /// **Prediction, written before the run.** Small branch: budget about
    /// 6.7e-7 for `S_1` (truncation 1.7e-7, `powf` 2.6e-7, roundings 2.4e-7)
    /// and 1.26e-6 for `S_2` (truncation 7.0e-7), ratios about 0.5 and 0.7
    /// at the top of the branch. Tail: budget about `77 u = 4.6e-6` at
    /// `x = 69`, dominated by `u |argument|`, and about `10 u` just above 4;
    /// ratios about 0.5. Middle branch: as in the cancellation test, worst
    /// about 0.38. Negative controls: the tail with its exp argument one ulp
    /// off (twice `u |argument|`) is caught; the small branch with `powf`
    /// two ulps off is caught.
    ///
    /// # Results, 2026-10-20, through the shipped `libm` (gh #1001)
    ///
    /// Every probe holds, and the small-branch and tail budgets' `f32`
    /// shadows are bit-identical to the shipped functions at every probe.
    ///
    /// | branch | | worst log error / budget | at | largest log error | its budget | ratio |
    /// |---|---|---|---|---|---|---|
    /// | `x < 2^-10` | `S_1` | 0.715 | 2.537e-5 | 3.415e-7 at 9.163e-4 | 4.967e-7 | 0.687 |
    /// | | `S_2` | **0.834** | 9.640e-4 | 8.602e-7 at 9.640e-4 | 1.031e-6 | 0.834 |
    /// | `2^-10 <= x <= 4` | `S_1` | 0.349 | 3.485 | 6.818e-4 at 3.9895 | 2.542e-3 | 0.268 |
    /// | | `S_2` | 0.362 | 0.06515 | 6.132e-4 at 3.9759 | 2.703e-3 | 0.227 |
    /// | `x > 4` | `S_1` | 0.602 | 8.262 | 7.844e-7 at 64.80 | 4.243e-6 | 0.185 |
    /// | | `S_2` | 0.601 | 8.290 | 8.201e-7 at 67.24 | 4.395e-6 | 0.187 |
    ///
    /// **The exponential tail is good to 8.2e-7, not 6.4e-5**: the old
    /// figure was the branch boundary, as noted above.
    ///
    /// **Against the prediction.** Small-branch budgets came out lower than
    /// predicted (4.97e-7 against 6.7e-7, 1.03e-6 against 1.26e-6: the
    /// `powf` term is smaller at the top of the branch, where `|ln x|` is
    /// 7, than the 13.8 I used) and the ratios higher (0.69 and 0.83 against
    /// 0.5 and 0.7); `S_2`'s, at 0.834, is the tightest in this module, and
    /// its budget is 68 % truncation. Tail budgets as predicted (4.2e-6 to
    /// 4.4e-6 at `x ~ 65`, `u |argument|` the bulk), but the ratio there is
    /// 0.19, not 0.5: the argument's actual rounding was small at those
    /// points. The tail's worst RATIO, 0.60, is near `x = 8`, where the
    /// budget is about `15 u`. Middle branch 0.35 to 0.36, as predicted.
    ///
    /// **Negative controls: the budgets can fail.** Ratio of log error to
    /// budget with one defect injected, everything else as shipped:
    ///
    /// | defect | at | -3 ulp | -2 ulp | -1 ulp | +1 ulp | +2 ulp |
    /// |---|---|---|---|---|---|---|
    /// | `S_1` tail, exp argument | 64.80 | 5.573 | 3.772 | **1.971** | **1.632** | 3.433 |
    /// | `S_2` tail, exp argument | 67.24 | 5.386 | 3.646 | **1.906** | **1.575** | 3.315 |
    /// | `S_1` tail, exp argument | 8.262 | 2.668 | **1.622** | 0.575 | **1.517** | 2.694 |
    /// | `S_1` small, `powf` | 2.537e-5 | 1.044 | **1.044** | 0.715 | 0.386 | 0.057 |
    /// | `S_2` small, `powf` | 9.640e-4 | **1.038** | 0.970 | 0.902 | 0.766 | 0.630 |
    ///
    /// So one ulp of the tail's exp argument (two roundings' worth, 7.6e-6
    /// at `x = 65`) is caught in either direction where the error is
    /// largest; near `x = 8` it is caught in one direction. A `powf` two
    /// (`S_1`) or three (`S_2`) ulps low is caught in the small branch, one
    /// is not. The `+1` tail rows at 64.80 and 67.24 and the two small-branch
    /// rows' first bold cells are asserted.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        // The assumptions the budgets rest on.
        let third = 1.0_f32 / 3.0;
        let (worst_ulp, at) = worst_ulps(
            |x| Real::powf(x, third),
            |x| Real::powf(x, f64::from(third)),
            1e-6,
            SMALL_CUT,
            7,
        );
        assert!(
            worst_ulp < 1.0,
            "powf(x, 1/3) is assumed faithful; it is {worst_ulp} ulp off at {at:e}"
        );
        let (worst_ulp, at) = worst_ulps(<f32 as Real>::exp, <f64 as Real>::exp, -87.0, -3.7, 3);
        assert!(
            worst_ulp < 1.0,
            "expf is assumed faithful; it is {worst_ulp} ulp off at {at:e}"
        );
        for x in [2.0_f64.powi(-10), 2.0_f64.powi(-11), 2.0_f64.powi(-12)] {
            let z = Real::powf(x, 1.0 / 3.0);
            let two_term = [
                2.149_528_241_534_478_636_71 * z * (1.0 - 8.438_127_628_132_05e-1 * z * z),
                1.074_764_120_767_239_318_36 * z * (1.0 - 1.177_671_565_102_35 * z * x),
            ];
            let full = [f64_synch::synchrotron_1(x), f64_synch::synchrotron_2(x)];
            for (i, c) in [3.0 / 16.0, 0.75].into_iter().enumerate() {
                let measured = (full[i] - two_term[i]) / two_term[i];
                assert!(
                    (measured / (c * x * x) - 1.0).abs() < 0.05,
                    "the two-term form of S_{} is derived to leave out {c} x^2 \
                     relative; at x = {x:e} it leaves out {measured:e}, which is \
                     {} x^2",
                    i + 1,
                    measured / (x * x)
                );
            }
        }

        let mut largest = [(0.0_f64, 0.0_f32); 2];
        for x in grid(1e-6, 100.0, 200_000) {
            for (i, (f, g)) in [
                (
                    synchrotron_1 as fn(f32) -> f32,
                    f64_synch::synchrotron_1 as fn(f64) -> f64,
                ),
                (synchrotron_2, f64_synch::synchrotron_2),
            ]
            .into_iter()
            .enumerate()
            {
                let s1 = i == 0;
                let r = g(f64::from(x));
                if r <= 1e-30 {
                    continue;
                }
                let l = log_err(f(x), r);
                let b = if x < SMALL_CUT {
                    let sh = small_budget(x, s1, 0);
                    assert_eq!(sh.f.to_bits(), f(x).to_bits(), "S_{} at {x}", i + 1);
                    sh.log_bound()
                } else if x <= 4.0 {
                    branch_log_budget(x, s1).0
                } else {
                    let sh = tail_budget(x, s1, 0);
                    assert_eq!(sh.f.to_bits(), f(x).to_bits(), "S_{} at {x}", i + 1);
                    sh.log_bound()
                };
                assert!(
                    l <= b,
                    "S_{} at x = {x}: log error {l:e} against a derived budget of \
                     {b:e}. More error than f32 arithmetic on this formula can \
                     produce: a transcription defect, or a libm routine worse than \
                     faithful",
                    i + 1
                );
                if l > largest[i].0 {
                    largest[i] = (l, x);
                }
            }
        }
        // The largest error is documented as sitting just under the branch
        // boundary at x = 4, where the Chebyshev form cancels.
        for (l, at) in largest {
            assert!(
                (3.5..=4.0).contains(&at),
                "the largest log error ({l:e}) is documented as just below the \
                 x = 4 branch boundary; it is at {at:e}. If it has moved, the \
                 cancellation explanation in this module no longer covers the \
                 measurement"
            );
        }

        // Negative controls: the budgets can fail. The tail with its exp
        // argument one ulp off, at each function's largest tail error; the
        // small branch with powf k ulps low, at the points recorded above.
        let controls: [(&str, f32, bool, bool, i32, f64); 4] = [
            ("S_1 tail", 64.797_78, true, true, 1, 1.632),
            ("S_2 tail", 67.241_91, false, true, 1, 1.575),
            ("S_1 small", 2.537_464_6e-5, true, false, -2, 1.044),
            ("S_2 small", 9.640_067_6e-4, false, false, -3, 1.038),
        ];
        for (name, x, s1, tail, k, documented) in controls {
            let budget = |k| {
                if tail {
                    tail_budget(x, s1, k)
                } else {
                    small_budget(x, s1, k)
                }
            };
            let g = if s1 {
                f64_synch::synchrotron_1
            } else {
                f64_synch::synchrotron_2
            };
            let b = budget(0).log_bound();
            let l = log_err(budget(k).f, g(f64::from(x)));
            assert!(
                l > b,
                "negative control, {name} at x = {x}: the defect ({k} ulp) was \
                 measured at {documented} of the budget; it now reads {:.3}, so the \
                 budget can no longer see it",
                l / b
            );
        }
    }

    /// The log error `f32` can deliver for the `x <= 4` branch of `S_1`
    /// (`s1 = true`) or `S_2`, derived from the propagation analysis in
    /// `the_f32_cost_is_cancellation_at_the_branch_boundary`, as a bound on
    /// `|ln S_f32 - ln S_f64|`. Also returns the cancellation ratio.
    fn branch_log_budget(x: f32, s1: bool) -> (f64, f64) {
        let xd = f64::from(x);
        // (1) px = powf(x, a') = exp(a' ln x): the exponent's rounding
        // a' - 1/3 enters the exp argument multiplied by |ln x|, and powf
        // adds at most one ulp of its own (faithful; checked below).
        let a_err = (f64::from(1.0_f32 / 3.0) - 1.0 / 3.0).abs();
        let p = Real::powf(xd, 1.0 / 3.0);
        let l_p = a_err * Real::ln(xd).abs() + ulp_over(p as f32);
        // (3) t = x*x/8 - 1: x is exact, x*x rounds, /8 is exact, -1 rounds.
        let t = xd * xd / 8.0 - 1.0;
        let dt = U32 * (xd * xd / 8.0) + U32 * t.abs();
        let (lo, hi, n) = if s1 {
            (&SYNCH1[..], &SYNCH2[..], 11_i32)
        } else {
            (&SYNCH21[..], &SYNCH22[..], 5_i32)
        };
        let (c_lo, e_lo) = cheb_budget(lo, t, dt);
        let (c_hi, e_hi) = cheb_budget(hi, t, dt);
        let pi_s3 = core::f64::consts::PI / Real::sqrt(3.0);
        let a = p * c_lo;
        let b = Real::powi(p, n) * c_hi;
        let c = if s1 { pi_s3 * xd } else { 0.0 };
        let r = a - b - c;
        // (2), (4) log errors ADD through products: px^n is n l_p plus its
        // n - 1 rounded multiplies; each term adds its series and one
        // rounded multiply.
        let l_a = l_p + e_lo / c_lo.abs() + U32;
        let l_b = f64::from(n) * l_p + f64::from(n - 1) * U32 + e_hi / c_hi.abs() + U32;
        let l_c = (f64::from(PI_OVER_SQRT3) - pi_s3).abs() / pi_s3 + U32;
        // (5) the sum turns each term's log error into an absolute one,
        // |T| l_T, plus the two rounded subtractions.
        let abs_err =
            a.abs() * l_a + b.abs() * l_b + c.abs() * l_c + U32 * (a - b).abs() + U32 * r.abs();
        let rel = abs_err / r.abs();
        let cancel = a.abs().max(b.abs()).max(c.abs()) / r.abs();
        (-Real::ln(1.0 - rel), cancel)
    }

    /// The shipped `x <= 4` branch of `S_1` with `px` supplied, so the
    /// negative control can degrade it. Asserted bit-equal to
    /// [`synchrotron_1`] when given the shipped `px`.
    fn s1_branch_given_px(x: f32, px: f32) -> f32 {
        let t = x * x / 8.0 - 1.0;
        px * cheb(&SYNCH1, t) - pow_int(px, 11) * cheb(&SYNCH2, t) - PI_OVER_SQRT3 * x
    }

    /// `S_2`'s counterpart of [`s1_branch_given_px`].
    fn s2_branch_given_px(x: f32, px: f32) -> f32 {
        let t = x * x / 8.0 - 1.0;
        px * cheb(&SYNCH21, t) - pow_int(px, 5) * cheb(&SYNCH22, t)
    }

    /// **The `f32` cost above is cancellation in upstream's own formula, not
    /// anything this transcription did**, and that is measured here rather
    /// than asserted in prose.
    ///
    /// On `x <= 4` GSL evaluates
    ///
    /// ```text
    ///     S_1 = x^{1/3} C_1(t)  -  x^{11/3} C_2(t)  -  (pi/sqrt 3) x
    ///     S_2 = x^{1/3} C_21(t) -  x^{5/3} C_22(t)
    /// ```
    ///
    /// and the terms grow while the answer falls. Measured at `x = 4`
    /// (2026-09-19):
    ///
    /// | | `S_1` | `S_2` |
    /// |---|---|---|
    /// | largest term | 59.385 | 76.806 |
    /// | result | 0.052824 | 0.046921 |
    /// | ratio | **1124** | **1637** |
    ///
    /// A ratio of `R` costs about `log2 R` bits, so `f32`'s 24 become about
    /// 13-14 — which is the 1e-4 seen above. `f64` runs the identical
    /// cancellation and simply has 29 more bits to spend, so this is a
    /// property of the series, not of the width.
    ///
    /// # The instrument: log error against a propagated budget (gh #1003)
    ///
    /// ~~The assertion is that the measured error in units of `f32::EPSILON`
    /// stays within a small factor of the cancellation ratio~~ (`ulps < 4 *
    /// cancel`, with `ulps = |S_f32 - S_f64| / |S_f64| / f32::EPSILON` and
    /// `cancel = max|term| / |S|`). **REPLACED 2026-10-20 (gh #1003).** The
    /// factor 4 was never derived. It was measured, and through glibc's
    /// `powf`, because std shadowed `Real::powf` in the test build (gh
    /// #1001). Through the shipped `libm::powf` the mirror read **4.12** at
    /// `x = 3.99` (`S_1` error 4535 ulps against a cancellation of 1101.6),
    /// and the test failed. The cause was one input: `libm::powf(3.99, 1/3)`
    /// returns `0x3fcb0494`, 0.64 ulp from the true value, where glibc
    /// returns the correctly rounded `0x3fcb0493`. ~~(within musl's documented
    /// 0.82-ulp bound, so not an upstream bug)~~ **CORRECTED 2026-10-20:**
    /// `libm` 0.2.16's `powf` is FreeBSD's `e_powf.c`, not musl's newer
    /// 0.82-ulp routine, and it documents no ulp bound ("nearly rounded").
    /// Measured below on this branch's domain it is faithful, worst **0.840
    /// ulp** (0.853 on a denser stride); the 0.82 figure does not apply.
    ///
    /// A uniform factor of the cancellation was the wrong instrument because
    /// the terms are not alike. `x^{11/3} = exp((11/3) ln x)`: an error in
    /// an exponential's argument comes out as a **log** error, `|ln f_f32 -
    /// ln f_ref|`, and log errors add through products. So `x^{11/3}`
    /// carries eleven times `x^{1/3}`'s log error, which no factor shared
    /// with the other two terms can express. Maintainer direction,
    /// 2026-10-20: "for exp error propagation you should measure by log error
    /// rather than error, to be fair."
    ///
    /// **The metric now** is the log error of the result, `|ln S_f32 - ln
    /// S_f64|`, and the bound is what `f32` can deliver on this formula,
    /// derived term by term. Everything is first order in `u = 2^-24` and
    /// worst case, every rounding at its bound
    /// (`branch_log_budget`, `cheb_budget`):
    ///
    /// 1. **`px = powf(x, a')`** with `a' = f32(1/3)`, i.e. `exp(a' ln x)`.
    ///    The exponent's rounding enters the exp argument multiplied by
    ///    `|ln x|`, the `|argument| * eps` term:
    ///    `|a' - 1/3| |ln x| = 9.93e-9 |ln x|`. `powf` adds at most one ulp
    ///    of its own. So `l_p <= |a' - 1/3| |ln x| + ulp(px)/px`.
    /// 2. **`px^n` by `n - 1` rounded multiplies** (`pow_int`):
    ///    `n l_p + (n - 1) u`.
    /// 3. **Each Chebyshev series** (Clenshaw, GSL's form): three roundings
    ///    per step, each reaching the result multiplied by `T_k(t)`, which
    ///    is at most 1 on `[-1, 1]`; the coefficients' rounding to `f32`,
    ///    `u sum |c_k|`; and `t`'s own rounding, `|C'(t)| u (x^2/8 + |t|)`.
    ///    These are evaluated in `f64` from the recurrence's values, which
    ///    are condition numbers, not errors.
    /// 4. **Each term** `T = prefactor * C`: `l_T = l_prefactor + dC/|C| +
    ///    u`. The constant term: `pi/sqrt 3`'s rounding plus `u`.
    /// 5. **The sum** turns each term's log error into an absolute one:
    ///    `|dS| <= sum |T_i| l_i + u |a - b| + u |S|`, and the log-error
    ///    bound is `-ln(1 - |dS|/|S|)`.
    ///
    /// Left out: the `f64` reference's own error, about `cancel * 2^-52`
    /// times a few, 1e-12 against budgets of 1e-5 to 3e-3.
    ///
    /// **Prediction, written before the run:** at `x = 3.99` the budget would
    /// be dominated by step 1 through `x^{11/3}`, `11 l_p |b|/|S|`, roughly
    /// 9 000 eps, so the measured 4 535 eps would sit at 0.3 to 0.5 of it.
    /// **Result:** the budget is 21 342 eps and the measured error 0.213 of
    /// it. The estimate left out step 2's rounded multiplies and step 3's
    /// Clenshaw roundings. At `x = 3.99` the `S_1` budget splits as `powf`
    /// (step 1) 41 %, rounded multiplies 27 %, Clenshaw 31 %, the rest 1 %.
    ///
    /// # Results, 2026-10-20, through the shipped `libm::powf`
    ///
    /// Log error and budget in units of `f32::EPSILON`:
    ///
    /// | `x` | cancel `S_1` | `S_1` error / budget | ratio | cancel `S_2` | `S_2` error / budget | ratio |
    /// |---|---|---|---|---|---|---|
    /// | 1 | 4.0 | 4.1 / 87.6 | 0.046 | 4.0 | 11.0 / 176.7 | 0.063 |
    /// | 2 | 20.4 | 8.8 / 363.7 | 0.024 | 28.7 | 38.7 / 660.9 | 0.058 |
    /// | 3 | 142.8 | 222.9 / 2 380 | 0.094 | 217.8 | 325.5 / 3 099 | 0.105 |
    /// | 3.9 | 913.2 | 213.0 / 17 524 | 0.012 | 1 338 | 1 303 / 19 402 | 0.067 |
    /// | 3.99 | 1 101 | **4 536 / 21 342** | **0.213** | 1 604 | 4 959 / 23 341 | 0.212 |
    /// | 4 | 1 124 | 536.2 / 21 814 | 0.025 | 1 637 | 434.3 / 23 825 | 0.018 |
    ///
    /// Over a further 100 001 linear probes on `[2^-10, 4]` the worst ratio is
    /// **0.361** for `S_1` (at `x = 3.294`) and **0.380** for `S_2` (at
    /// `x = 0.1496`). The old metric at `x = 3.99` read 4 535 / 1 101.6 =
    /// 4.12 against an asserted 4. The new one reads 0.213 against 1.
    ///
    /// **This bound is looser than the old one where the old one held.** At
    /// `x = 3.99` the new budget allows 21 342 eps where `4 * cancel` allowed
    /// 4 404. That is what a first-order worst case costs: it is derived,
    /// so it can be argued with line by line, but it is not tight.
    ///
    /// **Negative control: the budget can fail.** Move the shipped `px` by
    /// `k` ulps, as a `powf` worse than faithful would, and keep everything
    /// else as shipped. Ratio of log error to budget:
    ///
    /// | `x` | `k = 1` | `k = 2` | `k = 3` | `k = 4` |
    /// |---|---|---|---|---|
    /// | 3.9, `S_1` | 0.233 | 0.486 | 0.834 | **1.087** |
    /// | 3.99, `S_1` | 0.550 | 0.804 | **1.114** | **1.368** |
    /// | 4, `S_1` | 0.247 | 0.497 | 0.803 | **1.053** |
    /// | 3.99, `S_2` | 0.386 | 0.618 | 0.734 | 0.909 |
    ///
    /// So a `powf` three ulps off is caught at `x = 3.99`, and one or two ulps
    /// off is not: that is this instrument's resolution, stated rather than
    /// hidden. A single extra `f32` rounding of `px`'s exp argument (at most
    /// `u * |ln x|/3 = 0.46 u` at `x = 4`) is far below it. Bit-identity
    /// with the shader and the transcription tests carry the finer checks.
    /// The `k = 3` row at `x = 3.99` is asserted below.
    ///
    /// The test also asserts the assumption the budget rests on: `powf(x,
    /// 1/3)` is faithful over `[2^-10, 4]` (every 61st `f32`).
    #[test]
    fn the_f32_cost_is_cancellation_at_the_branch_boundary() {
        let eps = f64::from(f32::EPSILON);

        // The assumption the budget rests on: libm 0.2.16's powf (FreeBSD
        // e_powf.c, which documents no ulp bound) is faithful, under one
        // ulp, for exponent f32(1/3) on this branch's domain.
        let (mut worst_ulp, mut at) = (0.0_f64, 0.0_f32);
        let mut bits = SMALL_CUT.to_bits();
        while bits <= 4.0_f32.to_bits() {
            let x = f32::from_bits(bits);
            let exact = Real::powf(f64::from(x), f64::from(1.0_f32 / 3.0));
            let got = f64::from(Real::powf(x, 1.0 / 3.0));
            let e = (got - exact).abs() / (ulp_over(exact as f32) * exact);
            if e > worst_ulp {
                worst_ulp = e;
                at = x;
            }
            bits += 61;
        }
        assert!(
            worst_ulp < 1.0,
            "powf(x, 1/3) is assumed faithful; it is {worst_ulp} ulp off at {at:e}"
        );

        // The six points this test has always checked, then a sweep.
        let mut xs = std::vec![1.0_f32, 2.0, 3.0, 3.9, 3.99, 4.0];
        let n = 100_000_u32;
        xs.extend((0..=n).map(|k| SMALL_CUT + (4.0 - SMALL_CUT) * (k as f32 / n as f32)));
        for &x in &xs {
            let x = x.min(4.0);
            let px = Real::powf(x, 1.0 / 3.0); // as shipped (gh #1001)
            assert_eq!(
                s1_branch_given_px(x, px).to_bits(),
                synchrotron_1(x).to_bits()
            );
            assert_eq!(
                s2_branch_given_px(x, px).to_bits(),
                synchrotron_2(x).to_bits()
            );

            let (b1, k1) = branch_log_budget(x, true);
            let l1 = log_err(synchrotron_1(x), f64_synch::synchrotron_1(f64::from(x)));
            let (b2, k2) = branch_log_budget(x, false);
            let l2 = log_err(synchrotron_2(x), f64_synch::synchrotron_2(f64::from(x)));
            assert!(
                l1 <= b1,
                "S_1 at x = {x}: log error {:.1} eps against a propagated budget of \
                 {:.1} eps (cancellation {k1:.1}). Above the budget is more error \
                 than f32 arithmetic on this formula can produce: a transcription \
                 defect, or a powf worse than faithful",
                l1 / eps,
                b1 / eps
            );
            assert!(
                l2 <= b2,
                "S_2 at x = {x}: log error {:.1} eps against a budget of {:.1} eps \
                 (cancellation {k2:.1})",
                l2 / eps,
                b2 / eps
            );
        }

        // Negative control: the budget can fail. A px three ulps off, as a
        // powf worse than faithful would give, breaches it at x = 3.99.
        let x = 3.99_f32;
        let px = ulps_up(Real::powf(x, 1.0 / 3.0), 3);
        let (b1, _) = branch_log_budget(x, true);
        let l1 = log_err(
            s1_branch_given_px(x, px),
            f64_synch::synchrotron_1(f64::from(x)),
        );
        assert!(
            l1 > b1,
            "negative control: px three ulps off at x = 3.99 was measured at \
             1.114 of the budget; it now reads {:.3}, so the budget can no \
             longer see a powf three ulps off",
            l1 / b1
        );

        // And the cancellation really does worsen toward the boundary, which
        // is why the worst point above sits there.
        let ratio = |x: f32| {
            let px = Real::powf(x, 1.0 / 3.0); // as shipped (gh #1001)
            let t = x * x / 8.0 - 1.0;
            let (a, b, c) = (
                px * cheb(&SYNCH1, t),
                pow_int(px, 11) * cheb(&SYNCH2, t),
                PI_OVER_SQRT3 * x,
            );
            a.abs().max(b.abs()).max(c.abs()) / (a - b - c).abs()
        };
        assert!(
            ratio(1.0) < 10.0 && ratio(4.0) > 1000.0,
            "the cancellation is documented as growing from about 4 at x = 1 \
             to about 1124 at x = 4; measured {} and {}",
            ratio(1.0),
            ratio(4.0)
        );
    }

    /// **The range guard is kept at its `f64` value, and retargeting it to
    /// the `f32` analogue would discard 1461 real answers.** This is the
    /// measurement that reversed this module's first draft; every row of the
    /// table in the module documentation is asserted here.
    ///
    /// The distinction the taxonomy in `docs/wgsl-coverage.md` draws: a
    /// constant derived from the **exponent range** guards a value that would
    /// not be representable, and the arithmetic enforces it anyway — so
    /// moving it in can only remove answers the arithmetic was willing to
    /// give. A constant derived from **precision** is the opposite case and
    /// does need retargeting; `SMALL_CUT` is that one, and is checked below.
    #[test]
    fn the_range_guard_is_kept_because_retargeting_it_discards_answers() {
        // The retargeted value is what it claims to be: -8 ln(f32::MIN)/7,
        // upstream's expression read at the narrower width.
        assert!(
            (F32_UNDERFLOW_CUT - (-8.0 * f32::MIN_POSITIVE.ln() / 7.0)).abs() < 1e-3,
            "F32_UNDERFLOW_CUT = {F32_UNDERFLOW_CUT} is documented as \
             -8 ln(f32::MIN)/7"
        );
        assert!(
            UNDERFLOW_CUT > 8.0 * F32_UNDERFLOW_CUT,
            "the kept f64 bound is documented as far out of f32's way"
        );

        // 1. With the f64 bound kept, the arithmetic reaches zero on its own
        //    at 104.1979 -- well short of the 809.5959 guard, which is
        //    therefore dead code in f32 exactly as it is in f64.
        let first_zero = |f: fn(f32) -> f32| {
            let mut x = 100.0_f32;
            while x < 130.0 {
                if f(x) == 0.0 {
                    return x;
                }
                x = f32::from_bits(x.to_bits() + 1);
            }
            f32::NAN
        };
        for (name, z) in [
            ("S_1", first_zero(synchrotron_1)),
            ("S_2", first_zero(synchrotron_2)),
        ] {
            assert!(
                (104.19..104.21).contains(&z),
                "{name} is documented as first reaching zero at 104.1979 under \
                 the kept guard; it reaches it at {z}"
            );
        }

        // 2. Between the retargeted guard and that point the function still
        //    returns representable denormals -- which is exactly what
        //    retargeting would have thrown away.
        let below = f32::from_bits(F32_UNDERFLOW_CUT.to_bits() - 1);
        for (name, v) in [("S_1", synchrotron_1(below)), ("S_2", synchrotron_2(below))] {
            assert!(
                v > 0.0 && v < f32::MIN_POSITIVE,
                "just below the retargeted guard {name} is documented as a \
                 representable DENORMAL near 5.6e-43; it is {v:e}"
            );
            assert!(
                (5.0e-43..6.0e-43).contains(&v),
                "{name} just below the retargeted guard is documented as \
                 5.6e-43; it is {v:e}"
            );
        }

        // 3. The cost of getting this wrong, counted: 1461 of 4001 answers
        //    over x in [95, 107].
        let mut changed = [0_u32; 2];
        let total = 4001;
        for k in 0..total {
            let x = 95.0 + 12.0 * (k as f32) / (total - 1) as f32;
            if s1_with(x, UNDERFLOW_CUT).to_bits() != s1_with(x, F32_UNDERFLOW_CUT).to_bits() {
                changed[0] += 1;
            }
            if s2_with(x, UNDERFLOW_CUT).to_bits() != s2_with(x, F32_UNDERFLOW_CUT).to_bits() {
                changed[1] += 1;
            }
        }
        assert_eq!(
            changed,
            [1461, 1461],
            "retargeting the guard is documented as changing 1461 of {total} \
             answers over [95, 107] for each of S_1 and S_2; it changed \
             {changed:?}. Every one of them is a value the arithmetic was \
             willing to produce"
        );

        // 4. And every changed answer changes in the same direction: the
        //    retargeted guard returns 0 where the kept one returns a real
        //    number. It never gains an answer.
        for k in 0..total {
            let x = 95.0 + 12.0 * (k as f32) / (total - 1) as f32;
            let (kept, retargeted) = (s1_with(x, UNDERFLOW_CUT), s1_with(x, F32_UNDERFLOW_CUT));
            if kept.to_bits() != retargeted.to_bits() {
                assert!(
                    retargeted == 0.0 && kept > 0.0,
                    "at x = {x} retargeting gave {retargeted:e} against the \
                     kept {kept:e}; it is documented as only ever replacing a \
                     real value with zero"
                );
            }
        }
    }

    /// The precision constant, by contrast, **is** retargeted — and lands on
    /// exactly `2^-10`.
    #[test]
    fn the_precision_constant_is_retargeted() {
        // Exactly 2^-10, by algebra: 2 sqrt(2) = 2^{3/2} and
        // sqrt(f32::EPSILON) = sqrt(2^-23) = 2^{-23/2}.
        assert_eq!(SMALL_CUT, 0.0009765625);
        assert_eq!(SMALL_CUT.to_bits(), (0.5_f32).powi(10).to_bits());

        // Evaluating the same expression IN f32 does not land on it -- it
        // gives 9.765_624_4e-4, one ulp low, because neither sqrt(2) nor
        // sqrt(2^-23) is representable and the two roundings do not cancel.
        // The constant is the exact value, not the f32 round trip.
        let in_f32 = 2.0 * core::f32::consts::SQRT_2 * f32::EPSILON.sqrt();
        assert_ne!(in_f32, SMALL_CUT);
        assert_eq!(in_f32.to_bits() + 1, SMALL_CUT.to_bits());

        // Four orders of magnitude out from upstream's f64 value, 4.21e-08.
        assert!(SMALL_CUT / 4.214_684_851e-8 > 2.0e4);
    }

    /// The spectrum's shape survives `f32`: a single broad peak, rising like
    /// `x^{1/3}` and falling like `sqrt(x) e^{-x}`.
    ///
    /// Measured peaks (2026-09-19): `S_1` reaches **0.91801 at x = 0.2855**
    /// and `S_2` **0.60705 at x = 0.4160**. The published values are
    /// `S_1(0.29) ≈ 0.918` at the peak of the synchrotron spectrum and
    /// `S_2` peaking near `x = 0.42`.
    #[test]
    fn the_shape_survives_f32() {
        assert_eq!(synchrotron_1(0.0), 0.0);
        assert_eq!(synchrotron_2(0.0), 0.0);
        assert!(synchrotron_1(f32::NAN).is_nan() && synchrotron_2(f32::NAN).is_nan());
        assert!(synchrotron_1(-1.0).is_nan() && synchrotron_2(-1.0).is_nan());

        let peak = |f: fn(f32) -> f32| {
            let mut best = (0.0_f32, 0.0_f32);
            for k in 1..=20_000 {
                let x = 0.0005 * k as f32;
                let v = f(x);
                if v > best.0 {
                    best = (v, x);
                }
            }
            best
        };
        let (v1, x1) = peak(synchrotron_1);
        let (v2, x2) = peak(synchrotron_2);
        assert!(
            (0.9179..0.9182).contains(&v1) && (0.284..0.287).contains(&x1),
            "S_1 peak is documented as 0.91801 at x = 0.2855; got {v1} at {x1}"
        );
        assert!(
            (0.6069..0.6072).contains(&v2) && (0.414..0.418).contains(&x2),
            "S_2 peak is documented as 0.60705 at x = 0.4160; got {v2} at {x2}"
        );

        // Monotone up to the peak and down after it, on a coarse enough grid
        // that the 1e-4 cancellation near x = 4 cannot flip a step.
        for (f, xp) in [
            (synchrotron_1 as fn(f32) -> f32, x1),
            (synchrotron_2 as fn(f32) -> f32, x2),
        ] {
            let mut prev = 0.0_f32;
            let mut x = 0.01_f32;
            while x < xp {
                let v = f(x);
                assert!(v > prev, "not rising at {x}");
                prev = v;
                x += 0.01;
            }
            let mut prev = f(xp);
            let mut x = xp + 0.05;
            while x < 60.0 {
                let v = f(x);
                assert!(v < prev, "not falling at {x}: {v:e} vs {prev:e}");
                prev = v;
                x += 0.05;
            }
        }

        // Nothing overflows or goes negative anywhere in range.
        for k in 0..=100_000 {
            let x = 1e-6_f32 * (1e12_f32).powf(k as f32 / 100_000.0);
            for v in [synchrotron_1(x), synchrotron_2(x)] {
                assert!(v.is_finite() && v >= 0.0, "at x = {x:e}: {v:e}");
            }
        }
    }

    /// The shader and this mirror agree on their constants, checked by
    /// parsing the WGSL source — including that the shader ships the **kept**
    /// guard and not the retargeted one.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::SYNCHROTRON;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in synchrotron.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_SYNCH_SMALL_CUT: f32"), SMALL_CUT);
        assert_eq!(read("PETIR_SYNCH_UNDERFLOW_CUT: f32"), UNDERFLOW_CUT);
        assert_eq!(read("PETIR_SYNCH_PI_OVER_SQRT3: f32"), PI_OVER_SQRT3);
        assert_eq!(read("PETIR_SYNCH_LOG_SQRT_PI_2: f32"), LOG_SQRT_PI_2);
        assert_ne!(
            read("PETIR_SYNCH_UNDERFLOW_CUT: f32"),
            F32_UNDERFLOW_CUT,
            "the shader is documented as shipping the KEPT f64 guard; if it \
             now carries the f32 analogue it is discarding 1461 answers"
        );
        // Every coefficient table reaches the shader with the same count.
        for (name, n) in [
            ("petir_synch_cheb_synch1(", SYNCH1.len()),
            ("petir_synch_cheb_synch2(", SYNCH2.len()),
            ("petir_synch_cheb_synch1a(", SYNCH1A.len()),
            ("petir_synch_cheb_synch21(", SYNCH21.len()),
            ("petir_synch_cheb_synch22(", SYNCH22.len()),
            ("petir_synch_cheb_synch2a(", SYNCH2A.len()),
        ] {
            let at = src.find(name).unwrap_or_else(|| panic!("{name} missing"));
            let decl = &src[at..];
            let want = std::format!("array<f32, {n}>");
            assert!(
                decl[..400.min(decl.len())].contains(&want),
                "{name} is documented as carrying {n} coefficients; the shader \
                 does not declare `{want}` near its definition"
            );
        }
    }
}
