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
        let z = x.powf(1.0 / 3.0);
        return 2.1495283 * z * (1.0 - 0.84381276 * z * z);
    }
    if x <= 4.0 {
        let px = x.powf(1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        return px * cheb(&SYNCH1, t) - pow_int(px, 11) * cheb(&SYNCH2, t) - PI_OVER_SQRT3 * x;
    }
    if x < cut {
        let t = (12.0 - x) / (x + 4.0);
        return x.sqrt() * cheb(&SYNCH1A, t) * (LOG_SQRT_PI_2 - x).exp();
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
        let z = x.powf(1.0 / 3.0);
        return 1.0747641 * z * (1.0 - 1.1776716 * z * x);
    }
    if x <= 4.0 {
        let px = x.powf(1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        return px * cheb(&SYNCH21, t) - pow_int(px, 5) * cheb(&SYNCH22, t);
    }
    if x < cut {
        let t = (10.0 - x) / (x + 2.0);
        return x.sqrt() * (LOG_SQRT_PI_2 - x).exp() * cheb(&SYNCH2A, t);
    }
    0.0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::synchrotron as f64_synch;

    /// Worst relative difference against the `f64` module over a geometric
    /// sweep, and where it occurs.
    fn worst(f: fn(f32) -> f32, g: fn(f64) -> f64, lo: f32, hi: f32, n: u32) -> (f64, f32) {
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=n {
            let x = lo * (hi / lo).powf(k as f32 / n as f32);
            let r = g(x as f64);
            if r > 1e-30 {
                let e = (((f(x) as f64) - r) / r).abs();
                if e > w {
                    w = e;
                    at = x;
                }
            }
        }
        (w, at)
    }

    /// What `f32` costs, against the `f64` module: **6.09e-04 for `S_1` and
    /// 6.13e-04 for `S_2`**, both at `x = 3.976`, over 200 000 geometric
    /// probes on `[1e-6, 100]` (measured 2026-09-19).
    ///
    /// That is far worse than the roughly one-ulp figure the other mirrors in
    /// this module report, and it is **not** a transcription defect — see
    /// `the_f32_cost_is_cancellation_at_the_branch_boundary`, which measures
    /// the mechanism. Away from that boundary the mirror is ordinary: the
    /// small-argument branch is good to 8.6e-07 and the exponential tail to
    /// 6.4e-05.
    ///
    /// The exact worst value is grid-dependent — a denser sweep finds 7.3e-04
    /// at `x = 3.9985` — so the bound, not the number, is what is asserted.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let (w1, at1) = worst(
            synchrotron_1,
            f64_synch::synchrotron_1,
            1e-6,
            100.0,
            200_000,
        );
        let (w2, at2) = worst(
            synchrotron_2,
            f64_synch::synchrotron_2,
            1e-6,
            100.0,
            200_000,
        );
        assert!(w1 < 1e-3, "S_1 f32 vs f64: {w1:e} at {at1:e}");
        assert!(w2 < 1e-3, "S_2 f32 vs f64: {w2:e} at {at2:e}");
        // And the worst is documented as sitting just under the branch
        // boundary at x = 4, where the Chebyshev form cancels.
        for at in [at1, at2] {
            assert!(
                (3.5..=4.0).contains(&at),
                "the worst point is documented as just below the x = 4 branch \
                 boundary; it is at {at:e}. If it has moved, the cancellation \
                 explanation in this module no longer covers the measurement"
            );
        }

        // Away from that boundary the mirror behaves like the others here.
        let (s1, _) = worst(
            synchrotron_1,
            f64_synch::synchrotron_1,
            1e-6,
            9.7e-4,
            200_000,
        );
        let (s2, _) = worst(
            synchrotron_2,
            f64_synch::synchrotron_2,
            1e-6,
            9.7e-4,
            200_000,
        );
        assert!(
            s1 < 1e-6 && s2 < 1e-6,
            "small-argument branch: {s1:e}, {s2:e}"
        );
        let (t1, _) = worst(synchrotron_1, f64_synch::synchrotron_1, 4.0, 100.0, 200_000);
        let (t2, _) = worst(synchrotron_2, f64_synch::synchrotron_2, 4.0, 100.0, 200_000);
        assert!(t1 < 1e-4 && t2 < 1e-4, "exponential tail: {t1:e}, {t2:e}");
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
    /// The assertion is that the measured error in units of `f32::EPSILON`
    /// stays within a small factor of the cancellation ratio. If a future
    /// change makes the error *large* relative to the cancellation, that is a
    /// transcription defect and this test is what distinguishes the two.
    #[test]
    fn the_f32_cost_is_cancellation_at_the_branch_boundary() {
        for x in [1.0_f32, 2.0, 3.0, 3.9, 3.99, 4.0] {
            let px = x.powf(1.0 / 3.0);
            let t = x * x / 8.0 - 1.0;

            let (a, b, c) = (
                px * cheb(&SYNCH1, t),
                pow_int(px, 11) * cheb(&SYNCH2, t),
                PI_OVER_SQRT3 * x,
            );
            let r = a - b - c;
            let cancel = (a.abs().max(b.abs()).max(c.abs()) / r.abs()) as f64;
            let true1 = f64_synch::synchrotron_1(x as f64);
            let ulps =
                (((synchrotron_1(x) as f64) - true1) / true1).abs() / f64::from(f32::EPSILON);
            assert!(
                ulps < 4.0 * cancel,
                "S_1 at x = {x}: the branch cancels by {cancel:e} and the error \
                 is {ulps:e} ulps. The error is documented as tracking the \
                 cancellation; an error far above it is a transcription defect, \
                 not arithmetic"
            );

            let (a, b) = (px * cheb(&SYNCH21, t), pow_int(px, 5) * cheb(&SYNCH22, t));
            let r = a - b;
            let cancel = (a.abs().max(b.abs()) / r.abs()) as f64;
            let true2 = f64_synch::synchrotron_2(x as f64);
            let ulps =
                (((synchrotron_2(x) as f64) - true2) / true2).abs() / f64::from(f32::EPSILON);
            assert!(
                ulps < 4.0 * cancel,
                "S_2 at x = {x}: cancels by {cancel:e}, error {ulps:e} ulps"
            );
        }

        // And the cancellation really does worsen toward the boundary, which
        // is why the worst point above sits there.
        let ratio = |x: f32| {
            let px = x.powf(1.0 / 3.0);
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
