//! `f32` mirror of `shaders/atanint.wgsl` — GSL's inverse-tangent integral
//! `Ti_2`.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Two machine constants are retargeted, and the large one reshapes the
//! # domain
//!
//! Both are precision constants, so by the taxonomy in
//! `docs/wgsl-coverage.md` both must be retargeted:
//!
//! | constant | `f64` | `f32` |
//! |---|---|---|
//! | `0.5 sqrt(EPSILON)` | 7.45e-09 | 1.7263349e-04 |
//! | `1 / sqrt(EPSILON)` | 6.71e+07 | 2896.3093 |
//!
//! # A prediction recorded and refuted, in the sharpest form yet
//!
//! The large cut was expected to **reshape the domain**: above it the
//! Chebyshev series at `1/x^2` has collapsed to its leading term and the
//! answer is just `(pi/2) ln|x| + 1/x`, and in `f32` that threshold arrives
//! **23 170 times sooner** than in `f64`. A far larger share of the domain
//! therefore takes the closed-form branch — a change in *which code runs*.
//!
//! **Measured, it changes which code runs and nothing else.** Over 4000
//! probes spanning the whole disputed window, `2896.3` to `6.711e+07`:
//!
//! | | |
//! |---|---|
//! | answers that differ | **0 of 4000** |
//! | worst relative vs `f64`, shipped cut | 1.594855996429876e-07 |
//! | worst relative vs `f64`, GSL's cut | 1.594855996429876e-07 |
//!
//! Bit-identical, and the accuracy figures agree to the last digit.
//!
//! **The reason is not rounding**, which two drafts of this note claimed
//! before the test refuted each. The series argument `2(1/x^2 - 1/2)` never
//! reaches exactly `-1` near the cut — `-0.99999976` at `x = 2900`, still
//! `-0.99999994` at `x = 6000`, because the `f32` spacing just below `0.5` is
//! half that just above it.
//!
//! What actually makes the branches agree is that the series enters as
//! `cheb(t)/|x|` against a `(pi/2) ln|x|` term that dominates, and the gap
//! between `cheb(t)` and `cheb(-1)`, divided by `|x|`, falls **below one ulp
//! of that sum**. The series is not collapsing; its variation is simply being
//! rounded away by the addition.
//!
//! So retargeting is correct and its only effect is that the closed-form
//! branch skips a 21-term Clenshaw it would have spent on a foregone
//! conclusion — the same **work-only** category as
//! [`crate::wgsl::mirror_transport`], established here by bit equality rather
//! than by a bound.
//! `the_large_cut_changes_which_branch_runs_and_nothing_else` pins it.
//!
//! # What `f32` costs
//!
//! Worst relative against the `f64` module over `x` in `[1e-05, 1e+05]`,
//! geometrically: **1.595e-07** at `x = 4.545e+04` — one `f32` ulp.

// Under a std-linked build (`cargo test`) f32's inherent abs/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `0.5 sqrt(f32::EPSILON)`. Mirrors `PETIR_ATANINT_SMALL_CUT`.
const SMALL_CUT: f32 = 1.7263349e-4;
/// `1 / sqrt(f32::EPSILON)`. Mirrors `PETIR_ATANINT_LARGE_CUT`.
const LARGE_CUT: f32 = 2896.3093;
/// `pi/2`. Mirrors `PETIR_ATANINT_PI_2`.
const PI_2: f32 = 1.5707964;

/// GSL's `atanint_data` at its SINGLE-PRECISION order: 11 of the 21 stored,
/// where `f64` evaluates 21.
#[rustfmt::skip]
const ATANINT: [f32; 11] = [
    1.9104036092758179, -0.041763514280319214, 0.002753925509750843,
    -0.00025051808916032314, 2.666981345100794e-05, -3.118905169685604e-06,
    3.8833852045172534e-07, -5.0572744214605336e-08, 6.812252983934286e-09,
    -9.421255997565936e-10, 1.3307878410362406e-10,
];

/// Clenshaw in GSL's convention. Mirrors `petir_atanint_cheb`.
fn cheb(x: f32) -> f32 {
    let Some((&c0, rest)) = ATANINT.split_first() else {
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

/// `Ti_2(x)` in `f32` for all real `x`. Mirrors `petir_atanint`.
pub fn atanint(x: f32) -> f32 {
    atanint_with(x, LARGE_CUT)
}

/// [`atanint`] with the large cut supplied, so the retargeting can be
/// **measured** against keeping GSL's `f64` value. [`LARGE_CUT`] ships.
fn atanint_with(x: f32, large_cut: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    let ax = x.abs();
    let sgn = if x < 0.0 { -1.0 } else { 1.0 };
    if ax == 0.0 {
        0.0
    } else if ax < SMALL_CUT {
        x
    } else if ax <= 1.0 {
        x * cheb(2.0 * (x * x - 0.5))
    } else if ax < large_cut {
        sgn * (PI_2 * ax.ln() + cheb(2.0 * (1.0 / (x * x) - 0.5)) / ax)
    } else {
        sgn * (PI_2 * ax.ln() + 1.0 / ax)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::atanint as f64_atanint;

    /// GSL's `1/sqrt(DBL_EPSILON)`, the value this module does **not** use.
    const F64_LARGE_CUT: f32 = 6.7108864e7;

    /// What `f32` costs, against the `f64` module: **1.595e-07** at
    /// `x = 4.545e+04`, one `f32` ulp. The sweep is geometric so it spans the
    /// small cut, both Chebyshev branches and the closed form.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for k in 1..=4000 {
            // Geometric, so the sweep spans the small cut, both Chebyshev
            // branches and the closed form.
            let t = k as f32 / 4000.0;
            let x = 1e-5_f32 * (1e10_f32).powf(t);
            let r = f64_atanint::atanint(x as f64);
            if !r.is_finite() || r.abs() < 1e-6 {
                continue;
            }
            let e = (((atanint(x) as f64) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-5, "Ti_2 f32 vs f64: {worst:e} at {at:e}");
        assert!(
            (1e4..1e5).contains(&at),
            "the worst point is documented at x = 4.545e+04; it is at {at:e}"
        );
    }

    /// **The retargeted large cut changes which branch runs, and nothing
    /// else** — the refutation of this module's own first draft.
    ///
    /// Three claims, all asserted:
    ///
    /// 1. The cut moves by a factor of 23 170, so a far larger share of the
    ///    domain takes the closed-form branch.
    /// 2. Over the whole disputed window the two give **bit-identical**
    ///    answers — 0 of 4000 probes differ.
    /// 3. And the worst error against `f64` is identical bit for bit.
    ///
    /// **The mechanism is not rounding**, and two drafts of this test said it
    /// was. The series argument does not reach exactly `-1` near the cut —
    /// `-0.99999976` at `x = 2900`, still `-0.99999994` at `x = 6000`,
    /// because the `f32` spacing just below `0.5` is half that just above.
    ///
    /// The real reason the branches coincide is that the series enters as
    /// `cheb(t)/|x|` against a `(pi/2) ln|x|` term that dominates, and the
    /// gap between `cheb(t)` and `cheb(-1)` divided by `|x|` falls **below
    /// one ulp of that sum**. That is what is asserted.
    #[test]
    fn the_large_cut_changes_which_branch_runs_and_nothing_else() {
        // 1. The cut really does move by four orders of magnitude.
        let ratio = F64_LARGE_CUT / LARGE_CUT;
        assert!(
            (2.0e4..3.0e4).contains(&ratio),
            "the cut is documented as moving by a factor of 23 170, and moved \
             by {ratio:e}"
        );

        // 2. Bit-identical across the whole disputed window.
        let mut differ = 0;
        let mut total = 0;
        for k in 1..=4000 {
            let t = k as f32 / 4000.0;
            let x = LARGE_CUT * ratio.powf(t);
            total += 1;
            if atanint_with(x, LARGE_CUT).to_bits() != atanint_with(x, F64_LARGE_CUT).to_bits() {
                differ += 1;
            }
        }
        assert_eq!(
            differ, 0,
            "the two cuts are documented as giving BIT-IDENTICAL answers \
             across the disputed window, and {differ} of {total} differed. If \
             they now disagree the retargeting has become observable and the \
             module docs need rewriting"
        );

        // THE MECHANISM IS NOT ROUNDING, and two drafts of this test got
        // that wrong before the assertion below was written. The series
        // argument 2(1/x^2 - 1/2) does NOT reach exactly -1 anywhere near
        // the cut -- at x = 2900 it is -0.99999976 and at x = 6000 still
        // -0.99999994, because the f32 spacing just below 0.5 is half that
        // just above it. Chasing that threshold is beside the point.
        for x in [2.9e3_f32, 6.0e3] {
            let t = 2.0 * (1.0 / (x * x) - 0.5);
            assert!(
                t != -1.0 && (t + 1.0).abs() < 1e-6,
                "near the cut the series argument is documented as close to \
                 but NOT equal to -1; at x = {x:e} it is {t}"
            );
        }

        // THE ACTUAL REASON the two branches coincide everywhere in the
        // window: whatever the series contributes, it enters as
        // cheb(t)/|x| against a term (pi/2) ln|x| that dominates, and the
        // DIFFERENCE between cheb(t) and cheb(-1) divided by |x| is far
        // below one ulp of the sum.
        for x in [2.9e3_f32, 4e3, 5e3, 5.7e3] {
            let t = 2.0 * (1.0 / (x * x) - 0.5);
            let gap = ((cheb(t) - cheb(-1.0)) / x).abs();
            let ulp_of_sum = {
                let v = PI_2 * x.ln();
                f32::from_bits(v.to_bits() + 1) - v
            };
            assert!(
                gap < ulp_of_sum,
                "at x = {x:e} the series' contribution differs from the \
                 closed form's by {gap:e}, which is documented as below one \
                 ulp of the dominant (pi/2) ln|x| term ({ulp_of_sum:e}). That \
                 is why the two branches give the same f32."
            );
        }

        // 3. Identical accuracy, bit for bit.
        let worst = |cut: f32| {
            let mut w = 0.0f64;
            for k in 1..=4000 {
                let t = k as f32 / 4000.0;
                let x = 1e-5_f32 * (1e10_f32).powf(t);
                let r = crate::specfunc::atanint::atanint(x as f64);
                if !r.is_finite() || r.abs() < 1e-6 {
                    continue;
                }
                w = w.max((((atanint_with(x, cut) as f64) - r) / r).abs());
            }
            w
        };
        assert_eq!(
            worst(LARGE_CUT).to_bits(),
            worst(F64_LARGE_CUT).to_bits(),
            "the worst error is documented as identical with either cut"
        );
    }

    /// Catalan's constant, oddness and the logarithmic tail, in `f32`.
    #[test]
    fn the_known_values_and_shape_survive_f32() {
        assert_eq!(atanint(0.0), 0.0);
        assert!(
            (atanint(1.0) - 0.915_965_6).abs() < 1e-6,
            "Ti_2(1) = {} vs Catalan",
            atanint(1.0)
        );
        for k in 1..=200 {
            let x = 0.05 * k as f32;
            assert_eq!(atanint(-x), -atanint(x), "odd at {x}");
        }
        let mut prev = 0.0_f32;
        for k in 1..=400 {
            let x = 0.1 * k as f32;
            let v = atanint(x);
            assert!(v > prev, "not increasing at {x}");
            prev = v;
        }
        assert!(atanint(f32::NAN).is_nan());
        // Logarithmic, so no overflow anywhere.
        for x in [1e6_f32, 1e20, f32::MAX] {
            assert!(atanint(x).is_finite(), "Ti_2({x:e})");
        }
    }

    /// The shader and this mirror agree on their constants, checked by
    /// parsing the WGSL source.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::ATANINT;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in atanint.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_ATANINT_SMALL_CUT: f32"), SMALL_CUT);
        assert_eq!(read("PETIR_ATANINT_LARGE_CUT: f32"), LARGE_CUT);
        assert_eq!(read("PETIR_ATANINT_PI_2: f32"), PI_2);
        // And both cuts are the f32 analogues they claim to be.
        assert_eq!(SMALL_CUT, 0.5 * f32::EPSILON.sqrt());
        assert_eq!(LARGE_CUT, 1.0 / f32::EPSILON.sqrt());
    }
}
