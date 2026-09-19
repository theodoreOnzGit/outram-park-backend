//! `f32` mirrors of `shaders/debye.wgsl` — GSL's Debye functions `D_1`..`D_6`.
//!
//! Generated from the same parse of [`crate::specfunc::debye`] that produced
//! the shader, so the two cannot drift in their 102 coefficients. See
//! [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # One departure from upstream's arithmetic, and a prediction it refuted
//!
//! GSL's exponential-sum branch carries `xk` down the loop by subtraction
//! (`xk -= x`), so that `xk` is `rk * x` at every step. In `f64` the drift
//! over a few hundred subtractions is ~1e-13 and cannot matter.
//!
//! **In `f32` it is the dominant error.** At `x = 4.1`, `n = 6`, `xk` starts
//! near 705 where one ulp is 6.1e-05; 171 subtractions leave it wrong by
//! ~1e-04 by the time `rk` reaches the small values where the
//! falling-factorial polynomial is steepest. The branch then subtracts two
//! nearly equal quantities — the leading term is 0.9170 and the sum
//! contributes 0.8012 — so an 8x cancellation amplifies that drift straight
//! into the answer.
//!
//! Measured against the `f64` module over `x` in `(4.1, 15.2]` at 0.01
//! spacing, worst relative, for all four combinations of the two departures
//! this module makes (`the_recomputed_counter_beats_the_decremented_one`
//! re-measures the whole table):
//!
//! | order | `xk -= x`, `f64` cuts | `xk -= x`, `f32` cuts | `xk = rk * x`, **either** cut |
//! |---|---|---|---|
//! | `D_1` | 4.093e-06 | 1.159e-07 | 1.028e-07 |
//! | `D_2` | 3.994e-05 | 5.397e-07 | 1.423e-07 |
//! | `D_3` | 1.887e-04 | 2.640e-06 | 2.569e-07 |
//! | `D_4` | 6.720e-04 | 8.860e-06 | 3.725e-07 |
//! | `D_5` | 2.120e-03 | 2.785e-05 | 8.841e-07 |
//! | `D_6` | 6.401e-03 | 8.375e-05 | **1.940e-06** |
//!
//! The first column is GSL's own arithmetic transcribed literally, and at
//! `D_6` it is wrong in the third significant figure. So this recomputes —
//! a departure from a line-by-line transcription, taken because the `f64`
//! formulation is not numerically adequate at `f32` width.
//!
//! # The machine constants are retargeted too, and the two are not independent
//!
//! The first version of this module claimed the retargeting alone was what
//! made the difference, on the reasoning that the extra 151 iterations
//! "multiply zero by zero". **That was wrong**: the extra iterations were
//! not inert, they were accumulating `xk` drift. The table above shows both
//! halves of the correction — while `xk` is decremented the retargeting is
//! worth a factor of ~50 (a shorter loop drifts less), and once `xk` is
//! recomputed the `f32` and `f64` cuts give *bit-identical* answers, so the
//! choice is purely about doing 21 iterations instead of 172.
//!
//! | constant | `f64` | `f32` |
//! |---|---|---|
//! | `xcut` | 708.4 | 87.33654 |
//! | sum → closed form | 35.3505 | 15.24924 |
//! | small-argument cut | `2 sqrt(DBL_EPSILON)` | `2 sqrt(f32::EPSILON)` |
//!
//! # This mirror is NOT bit-identical to the GPU, although its worst branch
//! # is pure arithmetic
//!
//! The `x <= 4` Chebyshev branch calls no transcendental builtin, so by
//! `docs/wgsl-coverage.md`'s general rule it should match the device exactly.
//! It does not — up to 4 ulp. The cause is not the recurrence but the inline
//! `array<f32, 17>` literal the shader holds its coefficients in: fed the
//! same 17 values through a storage buffer instead, the same Clenshaw is
//! bit-exact at 64 of 64 points, and inline it is exact at 34.
//! `the_inline_coefficient_array_is_what_costs_bit_identity` in
//! `tests/wgsl_gpu.rs` is that A/B, and the ledger's rule is corrected there.
//!
//! `mirror_bessel` keeps GSL's `f64` thresholds, and that is not an
//! inconsistency: there they degenerate harmlessly, so keeping them is the
//! more literal transcription at no cost. Here they cost an eight-fold longer
//! loop.

// Under a std-linked build (`cargo test`) f32's inherent exp shadows this
// trait method, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

#[rustfmt::skip]
const ADEB1: [f32; 10] = [
    2.40065972, 0.193721304, -0.00623291246, 0.000351117477, -2.28222467e-05,
    1.58054679e-06, -1.1353782e-07, 8.35833612e-09, -6.26442479e-10, 4.76033489e-11,
];

#[rustfmt::skip]
const ADEB2: [f32; 11] = [
    2.59438102, 0.28633572, -0.0102062656, 0.000604910978, -4.05257659e-05, 2.86338263e-06,
    -2.0863943e-07, 1.55237876e-08, -1.17312801e-09, 8.97358589e-11, -6.9317614e-12,
];

#[rustfmt::skip]
const ADEB3: [f32; 11] = [
    2.70773707, 0.340068135, -0.0129451502, 0.000796375538, -5.4636001e-05, 3.92430196e-06,
    -2.89403282e-07, 2.17317614e-08, -1.65421e-09, 1.27279619e-10, -9.8796346e-12,
];

#[rustfmt::skip]
const ADEB4: [f32; 11] = [
    2.78186942, 0.374976784, -0.0149409074, 0.000945679811, -6.61329161e-05, 4.81563298e-06,
    -3.58808396e-07, 2.71601187e-08, -2.08070991e-09, 1.60938387e-10, -1.25470979e-11,
];

#[rustfmt::skip]
const ADEB5: [f32; 11] = [
    2.83402695, 0.399409886, -0.0164566765, 0.00106521383, -7.56730375e-05, 5.57459852e-06,
    -4.19069233e-07, 3.19456144e-08, -2.46133182e-09, 1.91280163e-10, -1.49720049e-11,
];

#[rustfmt::skip]
const ADEB6: [f32; 11] = [
    2.87267271, 0.417437535, -0.0176453849, 0.00116298527, -8.37118027e-05, 6.22836116e-06,
    -4.71864447e-07, 3.61950398e-08, -2.8030368e-09, 2.18768198e-10, -1.71857387e-11,
];

/// `-ln(f32::MIN_POSITIVE)` — the `f32` analogue of GSL's `xcut`.
const XCUT: f32 = 87.33654470871734;
/// `-(ln 2 + ln f32::EPSILON)` — the sum/closed-form cut in `f32`.
const SUMCUT: f32 = 15.249237968550476;
/// `sqrt(f32::EPSILON)`.
const SQRT_EPS: f32 = 3.4526698e-4;

/// Clenshaw in GSL's convention. Mirrors `petir_debye_cheb{n}`.
fn cheb(x: f32, c: &[f32]) -> f32 {
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

/// Per-order constants, mirroring the shader's `petir_debye_*` selectors.
struct Order {
    vinf: f32,
    lin: f32,
    quad: f32,
    cheb: &'static [f32],
    fall: &'static [f32],
    small_cut: f32,
}

const ORDERS: [Order; 6] = [
    Order {
        vinf: 1.64493407,
        lin: 0.25,
        quad: 0.0277777778,
        cheb: &ADEB1,
        fall: &[1.0, 1.0],
        small_cut: 2.0 * SQRT_EPS,
    },
    Order {
        vinf: 4.80822761,
        lin: 0.333333333,
        quad: 0.0416666667,
        cheb: &ADEB2,
        fall: &[1.0, 2.0, 2.0],
        small_cut: 2.0 * 1.414_213_6 * SQRT_EPS,
    },
    Order {
        vinf: 19.4818182,
        lin: 0.375,
        quad: 0.05,
        cheb: &ADEB3,
        fall: &[1.0, 3.0, 6.0, 6.0],
        small_cut: 2.0 * 1.414_213_6 * SQRT_EPS,
    },
    Order {
        vinf: 99.5450645,
        lin: 0.4,
        quad: 0.0555555556,
        cheb: &ADEB4,
        fall: &[1.0, 4.0, 12.0, 24.0, 24.0],
        small_cut: 2.0 * 1.414_213_6 * SQRT_EPS,
    },
    Order {
        vinf: 610.405837,
        lin: 0.416666667,
        quad: 0.0595238095,
        cheb: &ADEB5,
        fall: &[1.0, 5.0, 20.0, 60.0, 120.0, 120.0],
        small_cut: 2.0 * 1.414_213_6 * SQRT_EPS,
    },
    Order {
        vinf: 4356.06888,
        lin: 0.428571429,
        quad: 0.0625,
        cheb: &ADEB6,
        fall: &[1.0, 6.0, 30.0, 120.0, 360.0, 720.0, 720.0],
        small_cut: 2.0 * 1.414_213_6 * SQRT_EPS,
    },
];

/// `x^n` for the small `n` here. Mirrors `petir_debye_pow_n`.
fn pow_n(x: f32, n: u32) -> f32 {
    let mut out = 1.0;
    for _ in 0..n {
        out *= x;
    }
    out
}

/// `D_n(x)` in `f32` for `n` in `1 ..= 6`. Mirrors `petir_debye`.
///
/// `NaN` for `x < 0`, for `NaN`, and for an untabulated order.
pub fn debye_n(n: u32, x: f32) -> f32 {
    if n == 0 || n > 6 {
        return f32::NAN;
    }
    let Some(o) = ORDERS.get((n - 1) as usize) else {
        return f32::NAN;
    };
    if !(x >= 0.0) {
        return f32::NAN;
    }
    if x < o.small_cut {
        return 1.0 - o.lin * x + o.quad * x * x;
    }
    if x <= 4.0 {
        return cheb(x * x / 8.0 - 1.0, o.cheb) - o.lin * x;
    }
    if x < SUMCUT {
        let nexp = (XCUT / x).floor();
        let ex = (-x).exp();
        let mut sum = 0.0_f32;
        let mut rk = nexp;
        let mut i = nexp;
        while i >= 1.0 {
            // xk = rk * x, recomputed rather than decremented -- see the
            // module documentation.
            let u = 1.0 / (rk * x);
            let mut poly = 0.0_f32;
            for &c in o.fall.iter().rev() {
                poly = poly * u + c;
            }
            sum *= ex;
            sum += poly / rk;
            rk -= 1.0;
            i -= 1.0;
        }
        return o.vinf / pow_n(x, n) - n as f32 * sum * ex;
    }
    if x < XCUT {
        let mut poly = 0.0_f32;
        for &c in o.fall.iter() {
            poly = poly * x + c;
        }
        return (o.vinf - n as f32 * poly * (-x).exp()) / pow_n(x, n);
    }
    o.vinf / pow_n(x, n)
}

// Everything above this line is generated; the test module below is written
// by hand, so re-running the generator would drop it. Same arrangement as
// `mirror_bessel` and `mirror_psi_zeta`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::{debye as f64_debye, zeta as f64_zeta};

    /// Worst relative difference of the mirror against the `f64` module over
    /// a sweep, with the location.
    fn worst_vs_f64(n: u32, points: impl Iterator<Item = f32>) -> (f64, f32) {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for x in points {
            let r = f64_debye::debye_n(n, x as f64);
            let m = debye_n(n, x);
            if r == 0.0 || !r.is_finite() || !m.is_finite() {
                continue;
            }
            let e = (((m as f64) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        (worst, at)
    }

    /// What `f32` costs, measured against PETIR's own `f64` `debye`.
    ///
    /// # Results, measured 2026-09-19, over `x` in `[0.01, 16]` at 0.01
    /// spacing — which crosses all five branches.
    ///
    /// | order | worst relative | at |
    /// |---|---|---|
    /// | `D_1` | 2.784e-07 | 3.81 |
    /// | `D_2` | 5.213e-07 | 3.71 |
    /// | `D_3` | 9.052e-07 | 3.86 |
    /// | `D_4` | 7.549e-07 | 3.91 |
    /// | `D_5` | 1.245e-06 | 3.90 |
    /// | `D_6` | 1.940e-06 | 4.20 |
    ///
    /// Two to sixteen `f32` ulp, growing with order, and the worst point of
    /// every order sits within 0.3 of `x = 4` — the Chebyshev/sum join, where
    /// both branches are subtracting comparable quantities. That is where the
    /// conditioning is worst, not at either end.
    ///
    /// The bound asserted is 1e-05, loose enough not to be a
    /// hardware-rounding tripwire and tight enough to catch a wrong
    /// coefficient or a mis-taken branch.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        for n in 1..=6u32 {
            let (w, at) = worst_vs_f64(n, (1..=1600).map(|k| 0.01 * k as f32));
            assert!(w < 1e-5, "D_{n}: {w:e} at {at}");
        }
        // The documented shape: the error grows with order, and D_6 is the
        // worst of the six. Stated as an assertion so the claim can fail.
        let w1 = worst_vs_f64(1, (1..=1600).map(|k| 0.01 * k as f32)).0;
        let w6 = worst_vs_f64(6, (1..=1600).map(|k| 0.01 * k as f32)).0;
        assert!(
            w6 > w1,
            "D_6 ({w6:e}) is documented as costing more f32 error than D_1 \
             ({w1:e}); re-measure and rewrite the table rather than deleting \
             this"
        );
    }

    /// A copy of the exponential-sum branch carrying upstream's two choices
    /// — `xk -= x` and GSL's `f64` machine constants — so that this module's
    /// two departures from a literal transcription can each be *measured*
    /// rather than asserted.
    ///
    /// `recompute = false, f64_cuts = true` is GSL's own arithmetic.
    fn sum_branch_variant(n: u32, x: f32, recompute: bool, f64_cuts: bool) -> f32 {
        let Some(o) = ORDERS.get((n - 1) as usize) else {
            return f32::NAN;
        };
        // -GSL_LOG_DBL_MIN and -(ln 2 + ln DBL_EPSILON), rounded to f32.
        let xcut = if f64_cuts { 708.396_4_f32 } else { XCUT };
        let nexp = (xcut / x).floor();
        let ex = (-x).exp();
        let mut sum = 0.0_f32;
        let mut xk = nexp * x;
        let mut rk = nexp;
        let mut i = nexp;
        while i >= 1.0 {
            let u = if recompute { 1.0 / (rk * x) } else { 1.0 / xk };
            let mut poly = 0.0_f32;
            for &c in o.fall.iter().rev() {
                poly = poly * u + c;
            }
            sum *= ex;
            sum += poly / rk;
            xk -= x;
            rk -= 1.0;
            i -= 1.0;
        }
        o.vinf / pow_n(x, n) - n as f32 * sum * ex
    }

    /// The two departures this module makes, each measured against the
    /// literal transcription it replaces — and the finding that they are
    /// **not independent**.
    ///
    /// Over `x` in `(4.1, 15.2]` at 0.01 spacing, worst relative against the
    /// `f64` module (measured 2026-09-19):
    ///
    /// | order | `xk -= x`, `f64` cuts | `xk -= x`, `f32` cuts | `xk = rk * x` |
    /// |---|---|---|---|
    /// | `D_1` | 4.093e-06 | 1.159e-07 | 1.028e-07 |
    /// | `D_3` | 1.887e-04 | 2.640e-06 | 2.569e-07 |
    /// | `D_6` | 6.401e-03 | 8.375e-05 | 1.940e-06 |
    ///
    /// The first column is GSL's own arithmetic, and at `D_6` it is wrong in
    /// the **third significant figure** — which is why this is a departure
    /// worth making rather than pedantry about a last ulp.
    ///
    /// The right-hand column is one column because the `f32` and `f64` cuts
    /// give **bit-identical** answers once `xk` is recomputed. That is
    /// asserted below with `assert_eq!` on the `f32` values, and it is the
    /// entire content of the claim that the retargeting is only a
    /// performance choice. While `xk` is decremented it is not: a shorter
    /// loop accumulates less drift, so the retargeting is worth a factor of
    /// ~50 there.
    #[test]
    fn the_recomputed_counter_beats_the_decremented_one() {
        for n in 1..=6u32 {
            let mut worst = [0.0_f64; 3];
            for k in 1..=1110 {
                let x = 4.1 + 0.01 * k as f32;
                let r = f64_debye::debye_n(n, x as f64);
                let dec_f64 = sum_branch_variant(n, x, false, true);
                let dec_f32 = sum_branch_variant(n, x, false, false);
                let rec_f32 = sum_branch_variant(n, x, true, false);
                let rec_f64 = sum_branch_variant(n, x, true, true);

                // The bit-identity claim, at every single point.
                assert_eq!(
                    rec_f32.to_bits(),
                    rec_f64.to_bits(),
                    "D_{n} at x = {x}: the f32 and f64 cuts are documented as \
                     giving bit-identical answers once xk is recomputed, but \
                     they gave {rec_f32:e} and {rec_f64:e}"
                );

                for (slot, v) in [dec_f64, dec_f32, rec_f32].iter().enumerate() {
                    let e = (((*v as f64) - r) / r).abs();
                    if e > worst[slot] {
                        worst[slot] = e;
                    }
                }
            }
            // Recomputing is at least as good as decrementing, at every order.
            assert!(
                worst[2] <= worst[1],
                "D_{n}: recomputing xk ({:e}) is documented as no worse than \
                 decrementing it ({:e})",
                worst[2],
                worst[1]
            );
            // And the f64 cuts are what make decrementing genuinely bad.
            assert!(
                worst[0] > worst[1],
                "D_{n}: GSL's f64 cuts ({:e}) are documented as WORSE than \
                 the f32 ones ({:e}) while xk is decremented, because a \
                 longer loop drifts further",
                worst[0],
                worst[1]
            );
        }

        // The headline: GSL's own arithmetic, transcribed literally, is
        // wrong in the third significant figure at D_6.
        let mut gsl_worst = 0.0_f64;
        let mut shipped_worst = 0.0_f64;
        for k in 1..=1110 {
            let x = 4.1 + 0.01 * k as f32;
            let r = f64_debye::debye_n(6, x as f64);
            gsl_worst =
                gsl_worst.max((((sum_branch_variant(6, x, false, true) as f64) - r) / r).abs());
            shipped_worst = shipped_worst.max((((debye_n(6, x) as f64) - r) / r).abs());
        }
        assert!(
            gsl_worst > 1e-3,
            "D_6 under GSL's literal arithmetic is documented at 6.401e-03 — \
             wrong in the third significant figure — and measured {gsl_worst:e}"
        );
        assert!(
            shipped_worst < gsl_worst / 1000.0,
            "the shipped branch ({shipped_worst:e}) is documented as more \
             than three orders better than the literal transcription \
             ({gsl_worst:e})"
        );
    }

    /// The five branches join. Each join evaluates the **two formulas** at
    /// one argument and compares them to each other — not the function's
    /// value on either side of the cut, which would measure its slope rather
    /// than a discontinuity.
    #[test]
    fn the_branches_join() {
        for n in 1..=6u32 {
            let Some(o) = ORDERS.get((n - 1) as usize) else {
                unreachable!()
            };
            let small = |x: f32| 1.0 - o.lin * x + o.quad * x * x;
            let chebyshev = |x: f32| cheb(x * x / 8.0 - 1.0, o.cheb) - o.lin * x;
            let closed = |x: f32| {
                let mut poly = 0.0_f32;
                for &c in o.fall.iter() {
                    poly = poly * x + c;
                }
                (o.vinf - n as f32 * poly * (-x).exp()) / pow_n(x, n)
            };
            let asymptote = |x: f32| o.vinf / pow_n(x, n);

            // small <-> Chebyshev, at the small-argument cut.
            let x = o.small_cut;
            let (a, b) = (small(x), chebyshev(x));
            assert!(
                ((a - b) / b).abs() < 1e-6,
                "D_{n} small/Chebyshev join at {x}: {a:e} vs {b:e}"
            );

            // Chebyshev <-> exponential sum, at x = 4.
            let x = 4.0_f32;
            let (a, b) = (chebyshev(x), sum_branch_variant(n, x, true, false));
            assert!(
                ((a - b) / b).abs() < 1e-5,
                "D_{n} Chebyshev/sum join at 4: {a:e} vs {b:e}"
            );

            // exponential sum <-> closed form, at SUMCUT.
            let x = SUMCUT;
            let (a, b) = (sum_branch_variant(n, x, true, false), closed(x));
            assert!(
                ((a - b) / b).abs() < 1e-5,
                "D_{n} sum/closed join at {x}: {a:e} vs {b:e}"
            );

            // closed form <-> asymptote, at XCUT, where exp(-x) is 1.4e-38.
            let x = XCUT;
            let (a, b) = (closed(x), asymptote(x));
            assert!(
                ((a - b) / b).abs() < 1e-6,
                "D_{n} closed/asymptote join at {x}: {a:e} vs {b:e}"
            );
        }
    }

    /// `x^n D_n(x) -> n * n! * zeta(n+1)`, which is what `vinf` holds.
    ///
    /// Not `n! zeta(n+1)` — an earlier draft of the `f64` module's docs said
    /// that, and it is wrong by a factor of `n`. Checked here against
    /// [`crate::specfunc::zeta`], which shares nothing with this module.
    #[test]
    fn the_asymptotic_constant_is_n_times_n_factorial_times_zeta() {
        let mut factorial = 1.0_f64;
        for n in 1..=6u32 {
            factorial *= n as f64;
            let Some(o) = ORDERS.get((n - 1) as usize) else {
                unreachable!()
            };
            let expect = n as f64 * factorial * f64_zeta::zeta(n as f64 + 1.0);
            let rel = ((o.vinf as f64 - expect) / expect).abs();
            assert!(
                rel < 1e-6,
                "D_{n} val_infinity {} vs n * n! * zeta({}) = {expect}",
                o.vinf,
                n + 1
            );
            // And the function actually reaches it.
            let x = 40.0_f32;
            let reached = (debye_n(n, x) as f64) * (x as f64).powi(n as i32);
            assert!(
                ((reached - expect) / expect).abs() < 1e-5,
                "D_{n}(40) * 40^{n} = {reached} vs {expect}"
            );
        }
    }

    /// `D_n(0) = 1` for every order, and each is monotonically decreasing.
    #[test]
    fn every_order_starts_at_one_and_decreases() {
        for n in 1..=6u32 {
            assert_eq!(debye_n(n, 0.0), 1.0, "D_{n}(0)");
            let mut prev = 1.0_f32;
            for k in 1..=1500 {
                let x = 0.01 * k as f32;
                let v = debye_n(n, x);
                assert!(v < prev, "D_{n} not decreasing at {x}: {v} >= {prev}");
                assert!(v > 0.0, "D_{n}({x}) = {v}");
                prev = v;
            }
        }
    }

    /// The domain and order refusals, spelled `!(x >= 0.0)` so that `NaN`
    /// propagates rather than falling into the small-argument branch.
    #[test]
    fn the_refusals_match_the_f64_module() {
        for n in 1..=6u32 {
            assert!(debye_n(n, -1.0).is_nan(), "D_{n}(-1)");
            assert!(debye_n(n, -1e-9).is_nan(), "D_{n}(-1e-9)");
            assert!(debye_n(n, f32::NAN).is_nan(), "D_{n}(NaN)");
            assert!(debye_n(n, f32::INFINITY) == 0.0, "D_{n}(inf)");
        }
        for n in [0_u32, 7, 100] {
            assert!(debye_n(n, 1.0).is_nan(), "order {n}");
        }
    }
}
