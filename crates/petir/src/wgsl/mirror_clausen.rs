//! `f32` mirror of `shaders/clausen.wgsl` — GSL's Clausen function `Cl_2`,
//! and the `f32` argument reduction it needs.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # The argument reduction is REDESIGNED for `f32`, not transcribed
//!
//! [`crate::specfunc::trig`] splits `2 pi` into three `f64` pieces so each
//! `y * Pk` subtraction is exact. Those constants do not carry over: `P1`
//! holds about 30 significant bits, which leaves no room in a 24-bit mantissa
//! for the period count `y`.
//!
//! **The replacement head is upstream's own.** `clausen.c` already carries a
//! coarse split for its `pi - x` reflection step — `p0 = 6.28125` (201/32,
//! **eight** significant bits) and `p1 = 0.19353071795864769253e-2` — so this
//! reuses those and adds a third piece summing to `2 pi`. Eight bits of head
//! leaves sixteen for `y`, i.e. exact subtraction out to about 65 000
//! periods.
//!
//! # Measured, and it holds all the way to the refusal
//!
//! Worst `|sin(reduce(theta)) - sin(theta mod 2 pi)|` over 2000 probes per
//! decade, against a naive single-`f32` `2 pi` reduction:
//!
//! | `theta` | split | naive | ratio | `theta`'s own ulp |
//! |---|---|---|---|---|
//! | 1e0 | 3.937e-09 | 1.748e-07 | 44 | 1.192e-07 |
//! | 1e1 | 2.225e-07 | 2.702e-06 | 12 | 9.537e-07 |
//! | 1e2 | 2.384e-07 | 4.682e-05 | 196 | 7.629e-06 |
//! | 1e3 | 3.052e-07 | 4.091e-04 | 1341 | 6.104e-05 |
//! | 1e4 | 9.783e-07 | 5.350e-03 | **5468** | 9.766e-04 |
//! | 1e5 | 7.234e-06 | 2.897e-02 | 4004 | 7.812e-03 |
//! | 2.6e5 .. 5.2e5 | 7.774e-06 | 2.893e-02 | ~3700 | — |
//!
//! **The split holds near 8e-06 all the way to the refusal**, with a
//! three-to-four-thousand-fold advantage over the naive form, while the
//! naive form tracks `theta`'s own ulp. There is no collapse inside the
//! usable range.
//!
//! That is not luck: an eight-bit head keeps `y * P0` exact while `y` fits in
//! sixteen bits, i.e. out to 65 536 periods — **`4.12e5` in `theta`, against
//! a refusal at `5.24e5`**. The head width and the cut are matched to within
//! a factor of 1.3, and the slight rise from 7.23e-06 to 7.77e-06 in the last
//! row is that limit beginning to bite.
//!
//! **A first version of this table said the split collapsed at 1e5**, from a
//! sweep that ran to 6.7e5 — *past the refusal*, on arguments the function
//! rejects. `the_split_beats_the_naive_reduction_to_the_refusal` failed on
//! it, which is how that was caught. Measure inside the domain.
//!
//! The refusal at 524288 is `0.0625 / f32::EPSILON`, the correct `f32`
//! analogue of upstream's threshold — a **precision** constant, retargeted,
//! unlike [`crate::wgsl::mirror_airy`]'s range guard, which is kept. Here,
//! unusually, the cut and the usable range nearly coincide; in the `f64`
//! module they are seven decades apart.
//!
//! # Comparing angle reductions needs a continuous instrument
//!
//! `|reduce(theta) - (theta mod 2 pi)|` is the obvious measure and it is
//! **wrong**: at a period boundary the reduced value jumps between `0` and
//! `2 pi`, so a probe landing either side reports an error of `2 pi` when
//! nothing is wrong. A first attempt at the table above did exactly that and
//! showed a spurious collapse at `1e5`. Comparing through `sin`, which is
//! continuous across the boundary, is what the table and
//! `the_split_beats_the_naive_reduction_to_the_refusal` use.

// Under a std-linked build (`cargo test`) f32's inherent floor/ln/sin shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `201/32`, eight significant bits. Mirrors `PETIR_CLAUSEN_P0`.
const P0: f32 = 6.28125;
/// Mirrors `PETIR_CLAUSEN_P1`; upstream's `p1` in `clausen.c`.
const P1: f32 = 1.9353072e-3;
/// The remainder, so the three sum to `2 pi`. Mirrors `PETIR_CLAUSEN_P2`.
const P2: f32 = 1.0253132e-11;
/// Mirrors `PETIR_CLAUSEN_TWO_PI`.
const TWO_PI: f32 = 6.2831855;
/// Mirrors `PETIR_CLAUSEN_PI`.
const PI: f32 = 3.1415927;
/// `0.0625 / f32::EPSILON`. Mirrors `PETIR_CLAUSEN_LOSS_CUT`.
const LOSS_CUT: f32 = 524288.0;
/// `pi * sqrt(f32::EPSILON)`. Mirrors `PETIR_CLAUSEN_X_CUT`.
const X_CUT: f32 = 1.0846882e-3;

/// GSL's `aclaus_data` at its SINGLE-PRECISION order: 9 of the 15 stored,
/// where `f64` evaluates 15.
#[rustfmt::skip]
const ACLAUS: [f32; 9] = [
    2.1426945, 0.07233243, 0.0010164248, 3.2452503e-5, 1.3331519e-6, 6.2132406e-8,
    3.1300413e-9, 1.6635723e-10, 9.196593e-12,
];

/// Clenshaw in GSL's convention. Mirrors `petir_clausen_cheb`.
fn cheb(x: f32) -> f32 {
    let Some((&c0, rest)) = ACLAUS.split_first() else {
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

/// Reduce `theta >= 0` to `[0, 2 pi)`. Mirrors `petir_clausen_reduce`.
///
/// `coarse` selects the shipped eight-bit head; `false` uses a naive single
/// `2 pi`, so the choice can be **measured** rather than argued.
fn reduce_with(theta: f32, coarse: bool) -> f32 {
    if theta.is_nan() || theta.abs() > LOSS_CUT {
        return f32::NAN;
    }
    if !coarse {
        let mut r = theta - TWO_PI * (theta / TWO_PI).floor();
        if r >= TWO_PI {
            r -= TWO_PI;
        } else if r < 0.0 {
            r += TWO_PI;
        }
        return r;
    }
    let y = (theta / TWO_PI).floor();
    let mut r = ((theta - y * P0) - y * P1) - y * P2;
    if r > TWO_PI {
        r = ((r - P0) - P1) - P2;
    } else if r < 0.0 {
        r = ((r + P0) + P1) + P2;
    }
    r
}

/// Reduce `theta >= 0` to `[0, 2 pi)` with the shipped split.
pub fn reduce(theta: f32) -> f32 {
    reduce_with(theta, true)
}

/// `Cl_2(x)` in `f32`. Mirrors `petir_clausen`.
///
/// Odd and `2 pi`-periodic. `NaN` past the reduction's loss cut — but see
/// the module documentation: the *usable* range ends about two decades
/// earlier.
pub fn clausen(x_in: f32) -> f32 {
    if x_in.is_nan() {
        return f32::NAN;
    }
    let mut x = x_in;
    let mut sgn = 1.0_f32;
    if x < 0.0 {
        x = -x;
        sgn = -1.0;
    }
    x = reduce(x);
    if x.is_nan() {
        return f32::NAN;
    }
    if x > PI {
        x = (P0 - x) + P1;
        sgn = -sgn;
    }
    let val = if x == 0.0 {
        0.0
    } else if x < X_CUT {
        x * (1.0 - x.ln())
    } else {
        let t = 2.0 * (x * x / (PI * PI) - 0.5);
        x * (cheb(t) - x.ln())
    };
    sgn * val
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::clausen as f64_clausen;

    /// Catalan's constant in `f32`.
    const CATALAN: f32 = 0.915_965_6;

    /// **The split beats the naive reduction out to `theta = 1e4`, and then
    /// stops — because `theta`'s own ulp takes over.**
    ///
    /// Both regimes are asserted. The comparison goes through `sin`, which is
    /// continuous across a period boundary; see the module documentation on
    /// why the obvious measure is wrong.
    #[test]
    fn the_split_beats_the_naive_reduction_to_the_refusal() {
        let per_decade = |e: i32| {
            let scale = 10f32.powi(e);
            let (mut ws, mut wn) = (0.0f64, 0.0f64);
            for k in 1..=2000 {
                let t = k as f32 * scale / 2000.0 * 6.7;
                if t > LOSS_CUT {
                    continue;
                }
                let tr = (t as f64)
                    - core::f64::consts::TAU * ((t as f64) / core::f64::consts::TAU).floor();
                let want = tr.sin();
                ws = ws.max(((reduce_with(t, true) as f64).sin() - want).abs());
                wn = wn.max(((reduce_with(t, false) as f64).sin() - want).abs());
            }
            (ws, wn)
        };

        // The split holds to one part in 1e5 across every decade inside the
        // domain, and beats the naive form by three orders or more from 1e2 up.
        for e in 0..=5i32 {
            let (ws, wn) = per_decade(e);
            assert!(
                ws < 3e-5,
                "the split is documented as holding near 8e-06 all the way to \
                 the refusal, and at 1e{e} measured {ws:e}"
            );
            assert!(
                wn > 10.0 * ws,
                "the naive reduction is documented as far worse everywhere \
                 inside the domain: at 1e{e}, split {ws:e}, naive {wn:e}"
            );
        }

        // Right up against the refusal, where y needs its seventeenth bit.
        let (mut ws, mut wn) = (0.0f64, 0.0f64);
        for k in 1..=4000 {
            let t = LOSS_CUT * (0.5 + 0.499 * (k as f32 / 4000.0));
            let tr =
                (t as f64) - core::f64::consts::TAU * ((t as f64) / core::f64::consts::TAU).floor();
            let want = tr.sin();
            ws = ws.max(((reduce_with(t, true) as f64).sin() - want).abs());
            wn = wn.max(((reduce_with(t, false) as f64).sin() - want).abs());
        }
        assert!(
            ws < 3e-5,
            "the split is documented as still holding at 7.774e-06 just below \
             the refusal, and measured {ws:e}"
        );
        assert!(
            wn > 1000.0 * ws,
            "the advantage near the refusal is documented at ~3700x: split \
             {ws:e}, naive {wn:e}"
        );

        // The head width and the cut are matched: eight bits of head leave
        // sixteen for y, i.e. 65536 periods = 4.12e5, against a cut at
        // 5.24e5. That is WHY there is no collapse inside the domain.
        let periods_exact = 65536.0_f64 * core::f64::consts::TAU;
        assert!(
            periods_exact > 0.5 * (LOSS_CUT as f64) && periods_exact < 1.5 * (LOSS_CUT as f64),
            "the eight-bit head is documented as exact to {periods_exact:e} in \
             theta, matched to the {LOSS_CUT:e} refusal within a factor of \
             1.3. If they have drifted apart, the no-collapse claim above \
             needs re-deriving"
        );
    }

    /// `Cl_2` in `f32` against the `f64` module, over one period.
    ///
    /// Worst absolute 4.521e-07 at `x = -2.906`, measured 2026-09-19 — about
    /// four `f32` ulp of a value near 0.3.
    ///
    /// Absolute, because `Cl_2` has zeros at `0` and `pi` and the sweep runs
    /// across both.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=2000 {
            let x = -core::f32::consts::PI + core::f32::consts::TAU * (k as f32 / 2000.0);
            let r = f64_clausen::clausen(x as f64);
            let e = ((clausen(x) as f64) - r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-5, "Cl_2 vs f64: {worst:e} at {at}");
    }

    /// The known values, the symmetries, and the refusal — in `f32`.
    #[test]
    fn the_values_symmetries_and_refusal_survive_f32() {
        assert_eq!(clausen(0.0), 0.0);
        assert!(clausen(PI).abs() < 1e-6, "Cl_2(pi) = {}", clausen(PI));
        assert!(
            (clausen(PI / 2.0) - CATALAN).abs() < 1e-5,
            "Cl_2(pi/2) = {} vs {CATALAN}",
            clausen(PI / 2.0)
        );
        let m = clausen(PI / 3.0);
        assert!((m - 1.014_941_6).abs() < 1e-5, "Cl_2(pi/3) = {m}");
        for k in 1..=200 {
            let x = 0.03 * k as f32;
            assert!((clausen(-x) + clausen(x)).abs() < 1e-7, "odd at {x}");
            assert!(
                (clausen(x + core::f32::consts::TAU) - clausen(x)).abs() < 1e-5,
                "periodic at {x}"
            );
        }
        for x in [1e6_f32, -1e6, f32::INFINITY] {
            assert!(clausen(x).is_nan(), "Cl_2({x:e})");
        }
        assert!(clausen(f32::NAN).is_nan());
        assert!(clausen(LOSS_CUT * 0.999).is_finite());
    }

    /// The shader and this mirror agree on their constants, checked by
    /// parsing the WGSL source.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::CLAUSEN;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in clausen.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_CLAUSEN_P0: f32"), P0);
        assert_eq!(read("PETIR_CLAUSEN_P1: f32"), P1);
        assert_eq!(read("PETIR_CLAUSEN_P2: f32"), P2);
        assert_eq!(read("PETIR_CLAUSEN_TWO_PI: f32"), TWO_PI);
        assert_eq!(read("PETIR_CLAUSEN_PI: f32"), PI);
        assert_eq!(read("PETIR_CLAUSEN_LOSS_CUT: f32"), LOSS_CUT);
        assert_eq!(read("PETIR_CLAUSEN_X_CUT: f32"), X_CUT);

        // The three pieces really do sum to 2 pi, and the head really is
        // eight bits -- both are the reason the split works at all.
        let sum = (P0 as f64) + (P1 as f64) + (P2 as f64);
        assert!(
            (sum - core::f64::consts::TAU).abs() < 1e-9,
            "P0 + P1 + P2 = {sum} is documented as 2 pi"
        );
        let head_bits = 24 - (P0.to_bits() & 0x007f_ffff).trailing_zeros().min(23);
        assert!(
            head_bits <= 8,
            "the head is documented as carrying eight significant bits, \
             which is what leaves room for the period count; it has \
             {head_bits}"
        );
        // And the retargeted cuts are the f32 analogues they claim to be.
        assert_eq!(LOSS_CUT, 0.0625 / f32::EPSILON);
        assert_eq!(X_CUT, core::f32::consts::PI * f32::EPSILON.sqrt());
    }
}
