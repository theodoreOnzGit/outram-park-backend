//! `f32` mirror of `shaders/expint3.wgsl` — the cubic exponential integral
//! `Ei_3(x) = integral_0^x e^{-t^3} dt`.
//!
//! Generated from the same parse of [`crate::specfunc::expint3`] that
//! produced the shader, so the two cannot drift. Both tables carry **GSL's
//! single-precision order**, as [`crate::wgsl::mirror_dawson`] introduced:
//! 27 coefficients where the `f64` order needs 47.
//!
//! # Three machine constants, all precision, all retargeted
//!
//! | constant | `f64` | `f32` |
//! |---|---|---|
//! | `1.6 ROOT3_DBL_EPSILON` | 9.689e-06 | 7.8745065e-03 |
//! | `(-LOG_DBL_EPSILON)^{1/3}` | 3.303261 | 2.5168138 |
//! | `val_infinity = Gamma(4/3)` | 0.89297951156924921 | 0.8929795 |
//!
//! The third is not a threshold but the saturation **value**; upstream's
//! literal is the correctly rounded `f64` and this is its correctly rounded
//! `f32`.
//!
//! **The `f64` version of the second cut is exactly placed** — straddling it
//! by one part in 1e13 gives a bit-identical answer, because `e^{-x^3}` has
//! already fallen below one ulp of `Gamma(4/3)` there. That is the standard
//! the retargeted cut is held to in
//! `the_retargeted_saturation_cut_is_as_exactly_placed_as_upstreams`, and it
//! is worth stating because this crate has the counter-example on file:
//! [`crate::wgsl::mirror_fermi_dirac`]'s `F_2` carries the analogous cut
//! derived from the wrong root and pays 2005 `f32` ulp for it.
//!
//! There is no underflow or overflow guard to decide about — the function is
//! bounded by `Gamma(4/3)` and its only refusal is the `x < 0` domain error.

// Under a std-linked build (`cargo test`) f32's inherent exp shadows this
// trait method, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `1.6 * cbrt(f32::EPSILON)`. Mirrors `PETIR_E3_SMALL_CUT`.
const SMALL_CUT: f32 = 7.8745065e-3;
/// `(-ln(f32::EPSILON))^{1/3}`. Mirrors `PETIR_E3_LOG_CUT`.
const LOG_CUT: f32 = 2.5168138;
/// `Gamma(4/3)`. Mirrors `PETIR_E3_VAL_INFINITY`.
const VAL_INFINITY: f32 = 0.8929795;

/// GSL's `expint3_data` at its **single-precision order**:
/// 16 of the 24 stored, where `f64` evaluates 24.
#[rustfmt::skip]
const EXPINT3: [f32; 16] = [
    1.2691984176635742, -0.24884644150733948, 0.08052621781826019, -0.025772733613848686,
    0.007599879056215286, -0.002030695555731654, 0.0004908345872536302,
    -0.00010768223728518933, 2.155172660422977e-05, -3.956705313612474e-06,
    6.699240771013137e-07, -1.0513218029473137e-07, 1.5362580541022908e-08,
    -2.0990960081235244e-09, 2.6921095908072346e-10, -3.25195252670607e-11,
];

/// GSL's `expint3a_data` at its **single-precision order**:
/// 11 of the 23 stored, where `f64` evaluates 23.
#[rustfmt::skip]
const EXPINT3A: [f32; 11] = [
    1.927046537399292, -0.03492935746908188, 0.001450338400900364, -8.925336442189291e-05,
    7.054239176795818e-06, -6.671727419416129e-07, 7.242675792440423e-08,
    -8.782582661126526e-09, 1.1672234290216466e-09, -1.676631333769052e-10,
    2.5755016175299517e-11,
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

/// `Ei_3(x)` in `f32`. Mirrors `petir_expint_3`.
pub fn expint_3(x: f32) -> f32 {
    e3_with(x, LOG_CUT)
}

/// [`expint_3`] with the saturation cut supplied, so its placement can be
/// **measured** rather than argued. [`LOG_CUT`] ships.
fn e3_with(x: f32, cut: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x < SMALL_CUT {
        x
    } else if x <= 2.0 {
        x * cheb(&EXPINT3, x * x * x / 4.0 - 1.0)
    } else if x < cut {
        let t = 16.0 / (x * x * x) - 1.0;
        let s = (-(x * x * x)).exp() / (3.0 * x * x);
        VAL_INFINITY - cheb(&EXPINT3A, t) * s
    } else {
        VAL_INFINITY
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::expint3 as f64_e3;

    /// Upstream's `f64` saturation cut, `(-GSL_LOG_DBL_EPSILON)^{1/3}`, which
    /// this module deliberately does not use. Kept so the comparison can be
    /// measured.
    const F64_LOG_CUT: f32 = 3.3032613;

    /// **What `f32` costs: 1.718e-07, about 1.4 ulp** — the smallest figure
    /// of any kernel in this module, measured 2026-09-19 over 200 001
    /// geometric probes on `[1e-6, 10]` at `x = 0.2683`.
    ///
    /// There is nothing here to lose precision to: no cancellation, no
    /// oscillation, and a single bounded Chebyshev branch over most of the
    /// domain. The contrast with
    /// [`crate::wgsl::mirror_fermi_dirac`]'s 8.5e-06 and
    /// [`crate::wgsl::mirror_synchrotron`]'s 6.1e-04 is entirely about what
    /// the upstream *formula* does, not about the transcriptions.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=200_000 {
            let x = 1e-6_f32 * (1e7_f32).powf(k as f32 / 200_000.0);
            let r = f64_e3::expint_3(x as f64);
            if r > 1e-30 {
                let e = (((expint_3(x) as f64) - r) / r).abs();
                if e > worst {
                    worst = e;
                    at = x;
                }
            }
        }
        assert!(
            worst < 5e-7,
            "Ei_3 f32 vs f64: {worst:e} at {at:e}. Documented at 1.718e-07, \
             about 1.4 f32 ulp"
        );
    }

    /// **The retargeted saturation cut is as exactly placed as upstream's,
    /// and the two are bit-identical** — so this is a work-only retargeting,
    /// [`crate::wgsl::mirror_atanint`]'s category.
    ///
    /// Measured 2026-09-19 over 200 001 probes on `[2, 4]`, which contains
    /// both cuts:
    ///
    /// | cut | worst vs `f64` | in ulp | probes differing |
    /// |---|---|---|---|
    /// | 2.5168138 (retargeted, shipped) | 4.3333e-08 | 0.36 | — |
    /// | 3.3032613 (upstream's `f64`) | 4.3333e-08 | 0.36 | **0 of 200 001** |
    ///
    /// Zero. Between 2.5168 and 3.3033 the correction
    /// `cheb(16/x^3-1) e^{-x^3}/(3x^2)` is already below one `f32` ulp of
    /// `Gamma(4/3)` — about 7e-09 relative at the lower cut — so dropping it
    /// changes nothing and only saves an `exp` on that stretch.
    ///
    /// Worth asserting because the crate has the counter-example: the
    /// analogous cut in [`crate::wgsl::mirror_fermi_dirac`]'s `F_2` is
    /// derived from the wrong root and costs 2005 `f32` ulp. Both are
    /// precision constants; only one of them is free.
    #[test]
    fn the_retargeted_saturation_cut_is_as_exactly_placed_as_upstreams() {
        assert!(
            (LOG_CUT / (-f32::EPSILON.ln()).powf(1.0 / 3.0) - 1.0).abs() < 1e-6,
            "LOG_CUT = {LOG_CUT} is documented as (-ln(f32::EPSILON))^(1/3)"
        );
        assert!(LOG_CUT < F64_LOG_CUT, "the retargeted cut fires earlier");

        let mut differ = 0_u32;
        let total = 200_001;
        for k in 0..total {
            let x = 2.0_f32 + 2.0 * k as f32 / (total - 1) as f32;
            if e3_with(x, LOG_CUT).to_bits() != e3_with(x, F64_LOG_CUT).to_bits() {
                differ += 1;
            }
        }
        assert_eq!(
            differ, 0,
            "the two cuts are documented as giving BIT-IDENTICAL answers over \
             [2, 4]; {differ} of {total} differed. If they now disagree the \
             retargeting has become observable"
        );

        // The cut itself is invisible: no step across it.
        let below = e3_with(f32::from_bits(LOG_CUT.to_bits() - 1), LOG_CUT);
        let above = e3_with(LOG_CUT, LOG_CUT);
        assert_eq!(below.to_bits(), above.to_bits(), "the cut must not step");
        assert_eq!(above, VAL_INFINITY);
    }

    /// Monotone, bounded by `Gamma(4/3)`, linear at the origin, and the
    /// documented domain restriction.
    #[test]
    fn the_shape_and_the_domain_survive_f32() {
        assert_eq!(expint_3(0.0), 0.0);
        assert!(expint_3(f32::NAN).is_nan());
        for x in [-1e-30_f32, -1.0, -1e30, f32::NEG_INFINITY] {
            assert!(expint_3(x).is_nan(), "Ei_3({x:e}) should be NaN");
        }

        // NON-DECREASING TO WITHIN ONE ULP, not strictly. The function is
        // flat to f32's resolution as it approaches Gamma(4/3), so the
        // Chebyshev branch wobbles by a single bit there -- measured at
        // x = 1.9429, 8.9292616e-1 following 8.929262e-1. That is f32
        // rounding on a flat function, not a defect, and a first draft of
        // this test demanded strict monotonicity and failed on it. The f64
        // module IS strictly monotone and asserts so.
        let mut prev = 0.0_f32;
        let mut dips = 0_u32;
        for k in 0..=200_000 {
            let x = 1e-4 * k as f32;
            let v = expint_3(x);
            if v < prev {
                dips += 1;
                assert_eq!(
                    prev.to_bits() - v.to_bits(),
                    1,
                    "Ei_3 dipped by more than one ulp at {x}: {v:e} after {prev:e}"
                );
            }
            assert!(v <= VAL_INFINITY, "Ei_3({x}) = {v:e} exceeds Gamma(4/3)");
            prev = prev.max(v);
        }
        assert!(
            dips < 50,
            "Ei_3 is documented as wobbling by at most one ulp on the flat \
             approach to Gamma(4/3); it dipped {dips} times in 200 001 probes"
        );
        assert_eq!(prev, VAL_INFINITY);
        assert_eq!(expint_3(f32::INFINITY), VAL_INFINITY);

        // Linear at the origin: Ei_3(x) = x - x^4/4 + O(x^7).
        for x in [1e-5_f32, 1e-3, 5e-3] {
            let two_term = x - x * x * x * x / 4.0;
            assert!(
                ((expint_3(x) - two_term) / two_term).abs() < 1e-6,
                "Ei_3({x:e}) = {:e} against {two_term:e}",
                expint_3(x)
            );
        }
    }

    /// `VAL_INFINITY` is the correctly rounded `f32` of the correctly rounded
    /// `f64` upstream carries, and the tables carry `order_sp`.
    #[test]
    fn the_constants_and_the_orders_are_right() {
        assert_eq!(VAL_INFINITY, 0.892_979_511_569_249_211_f64 as f32);
        assert_eq!(VAL_INFINITY, f64_e3::probe_val_infinity() as f32);
        assert_eq!(SMALL_CUT, (1.6 * f32::EPSILON.cbrt() as f64) as f32);

        // GSL's order_sp, not its f64 order: 16 and 11 against 24 and 23.
        assert_eq!((EXPINT3.len(), EXPINT3A.len()), (16, 11));
        assert_eq!(EXPINT3.len() + EXPINT3A.len(), 27);

        // The shader agrees.
        let src = crate::wgsl::EXPINT3;
        let code: std::string::String = src
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
        assert_eq!(read("PETIR_E3_SMALL_CUT: f32"), SMALL_CUT);
        assert_eq!(read("PETIR_E3_LOG_CUT: f32"), LOG_CUT);
        assert_eq!(read("PETIR_E3_VAL_INFINITY: f32"), VAL_INFINITY);
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
            std::vec![16, 11],
            "the shader must carry order_sp too"
        );
    }
}
