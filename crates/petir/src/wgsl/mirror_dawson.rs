//! `f32` mirror of `shaders/dawson.wgsl` — Dawson's integral `F(x)`.
//!
//! Generated from the same parse of [`crate::specfunc::dawson`] that produced
//! the shader, so the two cannot drift. See [`crate::wgsl::mirror`] for why a
//! mirror exists at all.
//!
//! # The first mirror here to use GSL's single-precision order
//!
//! GSL's `cheb_series` has five fields and the fifth is `order_sp`, the order
//! to use in single precision — what `GSL_MODE_SINGLE` selects inside
//! `cheb_eval_mode`. Every earlier shader in this module ships the **f64**
//! order. This one ships `order_sp`:
//!
//! | table | stored | `f64` uses | `order_sp` |
//! |---|---|---|---|
//! | `daw_data` | 21 | 16 | **10** |
//! | `daw2_data` | 45 | 33 | **22** |
//! | `dawa_data` | 75 | 35 | **13** |
//!
//! 45 coefficients where the `f64` order would need 84. That is upstream's
//! own answer for this width, so it is porting rather than an invented
//! truncation — and `crate::cheb::eval_mode`'s reduced-`order_sp` path is
//! already verified against `cheb_eval_mode` (357/369 bit-identical).
//!
//! `the_single_precision_order_costs_what_it_was_measured_to_cost` measures
//! what it costs in the **answer**, which is the only figure that matters:
//! the fit's own relative error is a misleading instrument here, because
//! these fits enter additively against an `O(1)` leading term.
//!
//! # Three machine constants, and the third cannot be written down
//!
//! | constant | `f64` | `f32` | kind |
//! |---|---|---|---|
//! | `1.225 sqrt(EPSILON)` | 1.825e-08 | 4.2295206e-04 | precision — retargeted |
//! | `1/(sqrt2 sqrt(EPSILON))` | 4.745e+07 | **2048.0** | precision — retargeted |
//! | `0.1 DBL_MAX` | 1.798e+307 | — | range guard — **deleted** |
//!
//! The second lands on exactly `2^11`, by the same algebra that put
//! [`crate::wgsl::mirror_fermi_dirac`]'s small cut on `2^-10`:
//! `2^{-1/2} · 2^{23/2} = 2^11`.
//!
//! The third is [`crate::wgsl::mirror_fermi_dirac`]'s third outcome again —
//! `0.1 * DBL_MAX` is not an `f32`, so neither keeping nor retargeting is
//! available and the branch goes. Here that **gains** answers: `0.5/x` stays
//! representable, as a denormal, for every finite `f32`.

// Under a std-linked build (`cargo test`) f32's inherent abs shadows this
// trait method, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `1.225 * sqrt(f32::EPSILON)`. Mirrors `PETIR_DAW_XSML`.
///
/// A first draft of this module wrote `4.2295465e-4`, from multiplying
/// `1.225` by a **hand-rounded** `sqrt(EPSILON) = 3.4527e-4`. The exact
/// product is `4.229520541765238e-4`; the error was 0.6 % of the constant
/// and invisible in every accuracy measurement, because both the shader and
/// the mirror used the same wrong value. The test below computes it rather
/// than trusting the literal, which is what caught it.
const XSML: f32 = 4.2295206e-4;
/// `1/(sqrt(2) * sqrt(f32::EPSILON))`, exactly `2^11`. Mirrors
/// `PETIR_DAW_XBIG`.
const XBIG: f32 = 2048.0;

/// GSL's `daw_data` at its **single-precision order**:
/// 10 of the 21 stored, where `f64` evaluates 16.
#[rustfmt::skip]
const DAW: [f32; 10] = [
    -0.006351734511554241, -0.2294071465730667, 0.02213050052523613, -0.001549265463836491,
    8.49732750793919e-05, -3.828266471828101e-06, 1.462854868350405e-07,
    -4.85198237143436e-09, 1.4214636412379633e-10, -3.728836250188605e-12,
];

/// GSL's `daw2_data` at its **single-precision order**:
/// 22 of the 45 stored, where `f64` evaluates 33.
#[rustfmt::skip]
const DAW2: [f32; 22] = [
    -0.056886542588472366, -0.3181134760379791, 0.20873846113681793, -0.12475410103797913,
    0.06786930561065674, -0.03365914523601532, 0.015260781161487103, -0.006348371040076017,
    0.0024326741695404053, -0.0008621953893452883, 0.0002837657229974866,
    -8.70575531735085e-05, 2.4986849894048646e-05, -6.7319288064027205e-06,
    1.7078579048757092e-06, -4.09175498816694e-07, 9.282829438461704e-08,
    -1.9991404087704723e-08, 4.096349037752134e-09, -8.003240847820337e-10,
    1.4938503212214016e-10, -2.668799903293717e-11,
];

/// GSL's `dawa_data` at its **single-precision order**:
/// 13 of the 75 stored, where `f64` evaluates 35.
#[rustfmt::skip]
const DAWA: [f32; 13] = [
    0.016904857009649277, 0.008683252148330212, 0.00024248640693258494,
    1.2611823876795825e-05, 1.066453364728659e-06, 1.358159806841286e-07,
    2.1710423681042812e-08, 2.8670104068595492e-09, -1.9013364493947194e-10,
    -3.097780365557412e-10, -1.0294148866663022e-10, -6.260356382598031e-12,
    8.563132841699073e-12,
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

/// [`dawson`] evaluated with the **`f64`-order** tables instead of
/// `order_sp`'s, so `tests/wgsl_gpu.rs` can run the array-length experiment
/// against the same device.
pub fn with_f64_order_for_test(x: f32) -> f32 {
    let (a, b, c) = crate::specfunc::dawson::probe_daw();
    let cast = |t: &[f64], n: usize| -> [f32; 40] {
        let mut out = [0.0_f32; 40];
        for (o, &v) in out.iter_mut().zip(t.iter().take(n)) {
            *o = v as f32;
        }
        out
    };
    let y = x.abs();
    if x.is_nan() {
        f32::NAN
    } else if y < XSML {
        x
    } else if y < 1.0 {
        x * (0.75 + cheb(&cast(&a, 16)[..16], 2.0 * y * y - 1.0))
    } else if y < 4.0 {
        x * (0.25 + cheb(&cast(&b, 33)[..33], 0.125 * y * y - 1.0))
    } else if y < XBIG {
        (0.5 + cheb(&cast(&c, 35)[..35], 32.0 / (y * y) - 1.0)) / x
    } else {
        0.5 / x
    }
}

/// Dawson's integral `F(x)` in `f32`. Mirrors `petir_dawson`.
pub fn dawson(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    let y = x.abs();
    if y < XSML {
        x
    } else if y < 1.0 {
        x * (0.75 + cheb(&DAW, 2.0 * y * y - 1.0))
    } else if y < 4.0 {
        x * (0.25 + cheb(&DAW2, 0.125 * y * y - 1.0))
    } else if y < XBIG {
        (0.5 + cheb(&DAWA, 32.0 / (y * y) - 1.0)) / x
    } else {
        // Upstream's 0.1 * DBL_MAX guard is not an f32; see the module docs.
        0.5 / x
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::dawson as f64_dawson;

    /// The same branch chain with the **`f64`-order** tables, so the cost of
    /// `order_sp` can be measured rather than argued.
    fn with_f64_order(x: f32) -> f32 {
        let cast = |t: &[f64]| -> std::vec::Vec<f32> { t.iter().map(|&v| v as f32).collect() };
        let y = x.abs();
        if y < XSML {
            x
        } else if y < 1.0 {
            x * (0.75 + cheb(&cast(&f64_dawson::PROBE_DAW[..16]), 2.0 * y * y - 1.0))
        } else if y < 4.0 {
            x * (0.25 + cheb(&cast(&f64_dawson::PROBE_DAW2[..33]), 0.125 * y * y - 1.0))
        } else if y < XBIG {
            (0.5 + cheb(&cast(&f64_dawson::PROBE_DAWA[..35]), 32.0 / (y * y) - 1.0)) / x
        } else {
            0.5 / x
        }
    }

    /// Worst relative difference against the `f64` module over a geometric
    /// sweep, and where it occurs.
    fn worst(f: fn(f32) -> f32) -> (f64, f32) {
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=200_000 {
            let x = 1e-6_f32 * (1e10_f32).powf(k as f32 / 200_000.0);
            let r = f64_dawson::dawson(x as f64);
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

    /// **GSL's single-precision order costs nothing measurable here** —
    /// 45 coefficients instead of 84, and the worst error against `f64` is
    /// **bit-for-bit the same**.
    ///
    /// Measured 2026-09-19 over 200 001 geometric probes on `[1e-6, 1e4]`:
    ///
    /// | | worst `f32` vs `f64` | at | in ulp |
    /// |---|---|---|---|
    /// | `order_sp` (shipped) | 1.1277e-06 | 3.9939 | 9.5 |
    /// | `f64` order | 1.1277e-06 | 3.9939 | 9.5 |
    /// | the two against each other | 9.1727e-07 | 3.9824 | 7.7 |
    ///
    /// The third row is the important one: where they differ at all, they
    /// differ by less than the error both already carry. Both peak in the
    /// same place — just under the `|x| = 4` branch boundary, where
    /// `x (0.25 + cheb)` has the fit and the constant nearly cancelling the
    /// curvature.
    ///
    /// **The measurement is on the ANSWER, not on the fit.** Comparing the
    /// Chebyshev sums directly makes `order_sp` look far worse than it is —
    /// across the whole crate that comparison reports up to 3.3e-04 relative
    /// on `bessel_K1` — because these fits enter additively against an `O(1)`
    /// leading term, so a fit passing near zero shows an unbounded relative
    /// figure while contributing nothing. `order_sp` targets **absolute**
    /// accuracy of the sum, which is the quantity that reaches the answer.
    #[test]
    fn the_single_precision_order_costs_what_it_was_measured_to_cost() {
        assert_eq!(
            (DAW.len(), DAW2.len(), DAWA.len()),
            (10, 22, 13),
            "the tables are documented as carrying GSL's order_sp"
        );
        assert_eq!(DAW.len() + DAW2.len() + DAWA.len(), 45);

        let (sp, sp_at) = worst(dawson);
        let (f64o, f64_at) = worst(with_f64_order);
        assert!(
            sp < 2e-6,
            "order_sp is documented at 1.128e-06 (9.5 f32 ulp); it is {sp:e} at {sp_at:e}"
        );
        assert_eq!(
            sp.to_bits(),
            f64o.to_bits(),
            "the two orders are documented as reaching the SAME worst error \
             bit for bit: order_sp {sp:e} at {sp_at:e}, f64 order {f64o:e} at \
             {f64_at:e}. If they have come apart, order_sp is no longer free \
             here and the ledger needs updating"
        );

        // And where they differ at all, by less than the error both carry.
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for k in 0..=200_000 {
            let x = 1e-6_f32 * (1e10_f32).powf(k as f32 / 200_000.0);
            let (a, b) = (dawson(x), with_f64_order(x));
            if a.abs() > 1e-30 {
                let e = (((a - b) as f64) / a as f64).abs();
                if e > w {
                    w = e;
                    at = x;
                }
            }
        }
        assert!(
            w < sp,
            "the two orders are documented as differing by LESS than the error \
             both already carry; they differ by {w:e} at {at:e} against {sp:e}"
        );
    }

    /// The two retargeted constants are the expressions they claim to be —
    /// **computed here rather than trusted**, which is what caught the first
    /// draft's hand-rounded `XSML`.
    #[test]
    fn the_retargeted_constants_are_what_they_claim() {
        // XSML: computed, not copied. A first draft had 4.2295465e-4 from a
        // hand-rounded sqrt; the exact product is 4.229520541765238e-4.
        let exact = 1.225 * f32::EPSILON.sqrt();
        assert!(
            (XSML / exact - 1.0).abs() < 1e-6,
            "XSML = {XSML:e} against 1.225*sqrt(f32::EPSILON) = {exact:e}"
        );

        // XBIG is exactly 2^11, by algebra: 2^{-1/2} * 2^{23/2} = 2^11. The
        // f32 evaluation of the same expression is one ulp off, exactly as
        // mirror_fermi_dirac's 2^-10 small cut is.
        assert_eq!(XBIG, 2048.0);
        assert_eq!(XBIG, (2.0_f32).powi(11));
        let in_f32 = 1.0 / (core::f32::consts::SQRT_2 * f32::EPSILON.sqrt());
        assert_ne!(in_f32, XBIG);
        assert!((in_f32 / XBIG - 1.0).abs() < 1e-6);
    }

    /// **Deleting upstream's underflow guard GAINS answers**, which is the
    /// opposite of every other deleted or retargeted range guard in this
    /// module.
    ///
    /// GSL refuses past `0.1 * DBL_MAX = 1.798e+307`, which is not an `f32`,
    /// so the branch cannot be kept or retargeted and goes. What is left is
    /// `0.5/x`, and that stays representable for **every** finite `f32`:
    /// `0.5 / f32::MAX = 1.469e-39`, a denormal (2026-09-19). So this kernel
    /// returns a real number everywhere its argument is real, where upstream
    /// returns zero past its bound.
    #[test]
    fn deleting_the_underflow_guard_gains_answers() {
        for x in [1e30_f32, 1e37, 3.4e38, f32::MAX] {
            let v = dawson(x);
            assert!(v > 0.0 && v.is_finite(), "F({x:e}) = {v:e}");
            assert_eq!(v, 0.5 / x, "past XBIG the answer is exactly 0.5/x");
        }
        let smallest = 0.5 / f32::MAX;
        assert!(
            smallest > 0.0 && smallest < f32::MIN_POSITIVE,
            "the smallest answer is documented as the denormal 1.469e-39; it \
             is {smallest:e}"
        );
        assert_eq!(dawson(f32::INFINITY), 0.0);
        assert_eq!(dawson(f32::NEG_INFINITY), -0.0);
    }

    /// Shape: odd, bounded by the maximum, `1/(2x)` in the tail, `NaN`
    /// propagating.
    #[test]
    fn the_shape_survives_f32() {
        assert_eq!(dawson(0.0), 0.0);
        assert!(dawson(f32::NAN).is_nan());
        for k in 1..=4000 {
            let x = 0.01 * k as f32;
            assert_eq!(dawson(-x), -dawson(x), "not odd at {x}");
        }
        // The maximum, to f32's resolution.
        let mut peak = (0.0_f32, 0.0_f32);
        for k in 1..=200_000 {
            let x = 1e-5 * k as f32;
            let v = dawson(x);
            if v > peak.0 {
                peak = (v, x);
            }
        }
        assert!(
            (peak.0 - 0.541_044_2).abs() < 1e-5 && (peak.1 - 0.924_138_9).abs() < 1e-3,
            "the maximum is documented as 0.5410442 at x = 0.9241389; it is \
             {} at {}",
            peak.0,
            peak.1
        );
        for k in 0..=100_000 {
            let x = 1e-4 * k as f32;
            assert!(dawson(x) <= peak.0 + 1e-6, "F({x}) exceeds the maximum");
        }
        // 1/(2x) + 1/(4x^3) in the tail.
        for x in [50.0_f32, 100.0, 1e3] {
            let two_term = 0.5 / x + 0.25 / (x * x * x);
            assert!(
                ((dawson(x) - two_term) / two_term).abs() < 1e-5,
                "the tail at x = {x:e} is {:e} against {two_term:e}",
                dawson(x)
            );
        }
    }

    /// The shader and this mirror agree on their constants and their table
    /// lengths, checked by parsing the WGSL source.
    #[test]
    fn the_shader_and_this_mirror_agree() {
        let src = crate::wgsl::DAWSON;
        let code: std::string::String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<std::vec::Vec<_>>()
            .join("\n");
        let read = |name: &str| -> f32 {
            let at = code
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in dawson.wgsl"));
            let rest = &code[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .expect("f32 literal")
        };
        assert_eq!(read("PETIR_DAW_XSML: f32"), XSML);
        assert_eq!(read("PETIR_DAW_XBIG: f32"), XBIG);
        // Upstream's XMAX must not be there: it is not an f32.
        assert!(
            !code.contains("XMAX"),
            "0.1 * DBL_MAX is not representable as an f32 and must stay deleted"
        );
        // And the three tables carry order_sp, not the f64 order.
        let lens: std::vec::Vec<usize> = code
            .match_indices("array<f32, ")
            .map(|(i, _)| {
                code[i + 11..]
                    .split('>')
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .expect("array length")
            })
            .collect();
        assert_eq!(
            lens,
            std::vec![DAW.len(), DAW2.len(), DAWA.len()],
            "the shader's tables are documented as carrying order_sp \
             (10, 22, 13), not the f64 order (16, 33, 35)"
        );
    }
}
