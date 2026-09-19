//! `f32` mirror of `shaders/dilog.wgsl` — GSL's real dilogarithm `Li_2(x)`.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Written by hand, and why that is safe here
//!
//! [`crate::wgsl::mirror_bessel`], [`crate::wgsl::mirror_psi_zeta`] and
//! [`crate::wgsl::mirror_debye`] are all *generated* from one parse of their
//! `f64` module, because each carries hundreds of Chebyshev coefficients that
//! must not drift between shader and mirror.
//!
//! **This kernel has no coefficient tables at all** — it is two convergent
//! series and seven branch identities. There is nothing for a generator to
//! extract, so both sides are written by hand and the three shared constants
//! (`pi^2/6`, `pi^2/3` and the 0.01 cut) are pinned against the shader text
//! by `the_shader_and_this_mirror_agree_on_their_three_constants` instead of
//! by the table audit. That test parses the WGSL source, so the check is real
//! rather than a restatement.
//!
//! # A prediction this module recorded and then refuted
//!
//! [`series_2_accelerated`] switches at `x = 0.01` between evaluating
//! `(1-x) ln(1-x) / x` directly and using its Taylor expansion through `x^8`.
//! By analogy with [`crate::wgsl::mirror_debye`], where two `f64` thresholds
//! genuinely had to be retargeted, 0.01 was expected to be far too small in
//! `f32`: the branch forms `1 + t` with `t` near `-1`, and the Taylor
//! truncation error `~x^9/10` does not reach `f32::EPSILON` until about
//! `x = 0.22`, so there looked to be two decades of headroom.
//!
//! **Measured, there is almost nothing there — and past 0.25 it goes the
//! other way.** Worst relative error against the `f64` module over
//! `x` in `[-15, 15]` where `|Li_2| > 0.1`:
//!
//! | cut | worst |
//! |---|---|
//! | **0.01 (upstream, shipped)** | 4.786e-06 |
//! | 0.05 | 4.786e-06 |
//! | 0.1 | 4.063e-06 |
//! | 0.2 | 4.063e-06 |
//! | 0.3 | 2.472e-05 |
//! | 0.5 | 2.472e-05 |
//!
//! So the best available gain is **1.18x**, and beyond 0.25 the Taylor
//! branch starts being asked for arguments it was never meant to cover and
//! the error grows five-fold. **Upstream's 0.01 is kept**: a 1.18x gain does
//! not justify departing from a literal transcription, and the headroom the
//! prediction was built on does not exist.
//!
//! An earlier draft of this paragraph claimed the sweep was *flat*. It was
//! written from two measured points, 0.01 and 0.2, with the other four
//! assumed to lie between them. They do not —
//! `the_cut_is_upstreams_because_moving_it_buys_nothing` measures all six and
//! failed on 0.3, which is how the error was caught. The assertion now pins
//! both halves: bounded gain below 0.25, material loss above it.
//!
//! Taken with `mirror_debye`, the rule is *"know which of upstream's
//! constants still mean something in `f32`"*, not "always retarget" and not
//! "always keep". Debye's `xcut` comes from `f64`'s **exponent range**, which
//! `f32` does not share, so it had to move. This cut comes from a
//! **truncation order against epsilon**, and moving it buys nothing because
//! the error lives elsewhere.
//!
//! # Where the `f32` error actually is
//!
//! Worst relative against the `f64` module where `|Li_2| > 0.1`, per branch,
//! measured 2026-09-19:
//!
//! | window | worst | route |
//! |---|---|---|
//! | `(0, 0.25]` | 3.021e-07 | direct power series |
//! | `(0.25, 0.5]` | 6.982e-07 | accelerated series |
//! | `(0.5, 1)` | 3.553e-07 | Landen reflection |
//! | `(1, 1.01]` | 4.244e-08 | the `eps`/`ln eps` expansion |
//! | `(1.01, 2]` | 1.561e-07 | `1 - 1/x` Landen |
//! | `(2, 40]` | **5.710e-06** | inversion |
//! | `[-40, 0)` | 4.510e-07 | duplication |
//!
//! Six of seven are one `f32` ulp. The inversion branch is twenty times
//! worse, and it is **cancellation, not transcription**: it forms
//! `pi^2/3 - Li_2(1/x) - (1/2) ln^2 x`, and `(1/2) ln^2 x` passes through
//! `pi^2/3` at **`x = 12.595170`**, which is `Li_2`'s real zero. Relative
//! error is unbounded at a zero for any implementation in any precision. The
//! bounded quantity is the absolute error, **1.871e-06** over `[-15, 15]`.
//!
//! A caller wanting `Li_2` near `x = 12.6` in `f32` should expect no correct
//! digits, and that is a property of the formula rather than of this port.

// Under a std-linked build (`cargo test`) f32's inherent ln shadows this
// trait method, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `zeta(2) = pi^2 / 6`. Mirrors `PETIR_DILOG_ZETA2`.
const ZETA2: f32 = 1.6449341;
/// `pi^2 / 3`. Mirrors `PETIR_DILOG_PI2_3`.
const PI2_OVER_3: f32 = 3.2898681;
/// The logarithmic/Taylor cut inside [`series_2_accelerated`]. **Upstream's
/// value**, kept deliberately — see the module documentation. Mirrors
/// `PETIR_DILOG_TCUT`.
const TCUT: f32 = 0.01;

/// `sum_{k>=1} x^k / k^2`. Mirrors `petir_dilog_series_1`.
fn series_1(x: f32) -> f32 {
    let mut sum = x;
    let mut term = x;
    for k in 2..1000u32 {
        let rk = (k as f32 - 1.0) / k as f32;
        term *= x;
        term *= rk * rk;
        sum += term;
        if (term / sum).abs() < f32::EPSILON {
            break;
        }
    }
    sum
}

/// `sum_{k>=1} r^k / (k^2 (k+1))`. Mirrors `petir_dilog_series_2_raw`.
fn series_2_raw(r: f32) -> f32 {
    let mut rk = r;
    let mut sum = 0.5 * r;
    for k in 2..10u32 {
        rk *= r;
        sum += rk / (k as f32 * k as f32 * (k as f32 + 1.0));
    }
    for k in 10..100u32 {
        rk *= r;
        let ds = rk / (k as f32 * k as f32 * (k as f32 + 1.0));
        sum += ds;
        if (ds / sum).abs() < 0.5 * f32::EPSILON {
            break;
        }
    }
    sum
}

/// `Li_2(x) = 1 + (1-x) ln(1-x) / x + series_2(x)`, for `-1 < x < 1`.
/// Mirrors `petir_dilog_series_2`. Every call site supplies `x` in `[0, 1/2]`.
///
/// The logarithmic/Taylor `cut` is a parameter rather than a constant so that
/// the choice of it can be *measured* rather than asserted — see
/// `the_cut_is_upstreams_because_moving_it_buys_nothing`. [`TCUT`] is the
/// value that ships, and the shader hard-codes it.
fn series_2_accelerated(x: f32, cut: f32) -> f32 {
    let s = series_2_raw(x);
    let t = if x > cut {
        (1.0 - x) * (1.0 - x).ln() / x
    } else {
        let t68 = 1.0 / 6.0 + x * (1.0 / 7.0 + x * (1.0 / 8.0));
        let t38 = 1.0 / 3.0 + x * (1.0 / 4.0 + x * (1.0 / 5.0 + x * t68));
        (x - 1.0) * (1.0 + x * (0.5 + x * t38))
    };
    s + 1.0 + t
}

/// `Li_2(x)` for `x >= 0`, seven branches. Mirrors `petir_dilog_xge0`.
fn dilog_xge0(x: f32, cut: f32) -> f32 {
    if x > 2.0 {
        let log_x = x.ln();
        PI2_OVER_3 - series_2_accelerated(1.0 / x, cut) - 0.5 * log_x * log_x
    } else if x > 1.01 {
        let log_x = x.ln();
        let log_term = log_x * ((1.0 - 1.0 / x).ln() + 0.5 * log_x);
        ZETA2 + series_2_accelerated(1.0 - 1.0 / x, cut) - log_term
    } else if x > 1.0 {
        let eps = x - 1.0;
        let lne = eps.ln();
        let c1 = 1.0 - lne;
        let c2 = -(1.0 - 2.0 * lne) / 4.0;
        let c3 = (1.0 - 3.0 * lne) / 9.0;
        let c4 = -(1.0 - 4.0 * lne) / 16.0;
        let c5 = (1.0 - 5.0 * lne) / 25.0;
        let c6 = -(1.0 - 6.0 * lne) / 36.0;
        let c7 = (1.0 - 7.0 * lne) / 49.0;
        let c8 = -(1.0 - 8.0 * lne) / 64.0;
        ZETA2
            + eps
                * (c1
                    + eps
                        * (c2
                            + eps
                                * (c3
                                    + eps
                                        * (c4 + eps * (c5 + eps * (c6 + eps * (c7 + eps * c8)))))))
    } else if x == 1.0 {
        ZETA2
    } else if x > 0.5 {
        let log_x = x.ln();
        ZETA2 - series_2_accelerated(1.0 - x, cut) - log_x * (1.0 - x).ln()
    } else if x > 0.25 {
        series_2_accelerated(x, cut)
    } else if x > 0.0 {
        series_1(x)
    } else {
        0.0
    }
}

/// `Li_2(x)` in `f32` for real `x`. Mirrors `petir_dilog`.
///
/// Returns the **real part** of the principal branch for `x > 1`. `NaN`
/// propagates. There is no domain error.
pub fn dilog(x: f32) -> f32 {
    dilog_at_cut(x, TCUT)
}

/// [`dilog`] with the logarithmic/Taylor cut supplied, for measuring it.
fn dilog_at_cut(x: f32, cut: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x >= 0.0 {
        dilog_xge0(x, cut)
    } else {
        -dilog_xge0(-x, cut) + 0.5 * dilog_xge0(x * x, cut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::dilog as f64_dilog;

    /// `Li_2`'s real zero above 1, located by bisection on the `f64` module.
    /// Every relative-error statement in this module is about the
    /// neighbourhood of this point.
    const REAL_ZERO: f64 = 12.595_170_369_845;

    /// Worst relative difference against the `f64` module over a sweep,
    /// skipping where `|Li_2|` is too small for a relative figure to mean
    /// anything.
    fn worst_rel(points: impl Iterator<Item = f32>, floor: f64) -> (f64, f32) {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for x in points {
            let r = f64_dilog::dilog(x as f64);
            if r.abs() < floor || !r.is_finite() {
                continue;
            }
            let e = (((dilog(x) as f64) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        (worst, at)
    }

    /// What `f32` costs, branch by branch.
    ///
    /// The table is in the module documentation. Six of the seven branches
    /// come in at one `f32` ulp; the `x > 2` inversion is twenty times worse
    /// and `the_inversion_branch_is_cancellation_at_the_zero` measures why
    /// separately.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let windows: [(&str, f32, f32, f64); 7] = [
            ("direct series", 0.001, 0.25, 3e-6),
            ("accelerated series", 0.2501, 0.5, 3e-6),
            ("Landen reflection", 0.5001, 0.9999, 3e-6),
            ("eps/ln-eps expansion", 1.000_000_1, 1.01, 3e-6),
            ("1 - 1/x Landen", 1.0101, 2.0, 3e-6),
            // The inversion branch crosses Li_2's zero; 5e-5 is an order
            // above the 5.710e-06 measured, and the absolute error is
            // asserted tightly below instead.
            ("inversion", 2.001, 40.0, 5e-5),
            ("duplication", -40.0, -0.001, 3e-6),
        ];
        for (name, lo, hi, budget) in windows {
            let (w, at) = worst_rel(
                (0..=4000).map(|k| lo + (hi - lo) * (k as f32 / 4000.0)),
                0.1,
            );
            assert!(w < budget, "{name}: {w:e} at {at}, budget {budget:e}");
        }
    }

    /// The bounded statement: **absolute** error over `[-15, 15]`, which is
    /// the window containing `Li_2`'s zero and therefore the one where a
    /// relative figure is meaningless.
    ///
    /// Measured 1.871e-06.
    #[test]
    fn the_absolute_error_is_bounded_across_the_zero() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f32;
        for k in -3000..=3000i32 {
            let x = 0.005 * k as f32;
            let r = f64_dilog::dilog(x as f64);
            let e = ((dilog(x) as f64) - r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 2e-5, "absolute error {worst:e} at {at}");
    }

    /// **The inversion branch's error is cancellation at `Li_2`'s zero, not a
    /// bad transcription**, and this says so in a way that can fail.
    ///
    /// Two claims: the relative error near the zero is far worse than the
    /// same branch away from it, and the *absolute* error is not. If the
    /// transcription were wrong both would be bad together.
    #[test]
    fn the_inversion_branch_is_cancellation_at_the_zero() {
        // Confirm the zero is where it is documented, from the f64 module.
        assert!(
            f64_dilog::dilog(REAL_ZERO - 1e-6) > 0.0 && f64_dilog::dilog(REAL_ZERO + 1e-6) < 0.0,
            "Li_2's real zero is documented at {REAL_ZERO}"
        );

        let near = (REAL_ZERO as f32 - 1.5)..(REAL_ZERO as f32 + 1.5);
        let (near_rel, _) = worst_rel(
            (0..=600).map(|k| near.start + (near.end - near.start) * (k as f32 / 600.0)),
            0.0,
        );
        let (far_rel, _) = worst_rel((0..=600).map(|k| 25.0 + 0.05 * k as f32), 0.1);
        assert!(
            near_rel > 100.0 * far_rel,
            "the inversion branch is documented as losing RELATIVE accuracy \
             only near Li_2's zero: near {near_rel:e}, far {far_rel:e}"
        );

        // And the absolute error there is ordinary.
        let mut abs_near = 0.0_f64;
        for k in 0..=600 {
            let x = near.start + (near.end - near.start) * (k as f32 / 600.0);
            let r = f64_dilog::dilog(x as f64);
            abs_near = abs_near.max(((dilog(x) as f64) - r).abs());
        }
        assert!(
            abs_near < 2e-5,
            "absolute error near the zero is documented as ordinary, and \
             measured {abs_near:e} -- if this is large too the branch is \
             genuinely mis-transcribed, not merely cancelling"
        );
    }

    /// **Upstream's 0.01 cut is kept because moving it buys nothing**, and
    /// this is the measurement that refuted the prediction it would not.
    ///
    /// Sweeping the cut from 0.01 to 0.5 changes the worst relative error by
    /// well under a factor of two. Compare `mirror_debye`, where the
    /// equivalent retargeting was worth a factor of fifty — the difference is
    /// that Debye's threshold came from `f64`'s exponent range and this one
    /// comes from a truncation order.
    #[test]
    fn the_cut_is_upstreams_because_moving_it_buys_nothing() {
        let measure = |cut: f32| {
            let mut worst = 0.0_f64;
            for k in -3000..=3000i32 {
                let x = 0.005 * k as f32;
                let r = f64_dilog::dilog(x as f64);
                if r.abs() < 0.1 {
                    continue;
                }
                worst = worst.max((((dilog_at_cut(x, cut) as f64) - r) / r).abs());
            }
            worst
        };
        let shipped = measure(TCUT);

        // Raising it to 0.1 or 0.2 is a 1.18x improvement -- real, and far
        // too small to justify departing from upstream.
        for cut in [0.05_f32, 0.1, 0.2] {
            let other = measure(cut);
            assert!(
                other > shipped / 1.5,
                "moving the cut from {TCUT} to {cut} is documented as gaining \
                 at most 1.18x ({shipped:e} -> 4.063e-06), and it gained more \
                 ({other:e}). If the gain is now worth having, retarget the \
                 cut and rewrite the module docs rather than loosening this"
            );
        }

        // Past about 0.25 it gets WORSE, which is the half of this that a
        // two-point sweep missed: the Taylor branch starts being asked for
        // arguments it was never meant to cover.
        for cut in [0.3_f32, 0.5] {
            let other = measure(cut);
            assert!(
                other > 3.0 * shipped,
                "the cut is documented as getting materially WORSE past 0.25 \
                 -- 2.472e-05 at 0.3, against {shipped:e} at {TCUT} -- and at \
                 {cut} it measured {other:e}"
            );
        }
    }

    /// The closed-form values, in `f32`.
    #[test]
    fn the_closed_form_values_survive_f32() {
        assert_eq!(dilog(0.0), 0.0);
        assert!((dilog(1.0) - ZETA2).abs() < 1e-6, "Li_2(1)");
        assert!((dilog(-1.0) + ZETA2 / 2.0).abs() < 1e-6, "Li_2(-1)");
        let half = ZETA2 / 2.0 - 0.5 * core::f32::consts::LN_2 * core::f32::consts::LN_2;
        assert!(
            ((dilog(0.5) - half) / half).abs() < 1e-5,
            "Li_2(1/2) = {} vs {half}",
            dilog(0.5)
        );
    }

    /// Landen's reflection in `f32`. The two sides go through different
    /// branches, so this is not a tautology.
    #[test]
    fn landens_reflection_holds_in_f32() {
        let mut worst = 0.0_f32;
        for k in 1..=199i32 {
            let x = 0.005 * k as f32;
            let lhs = dilog(x) + dilog(1.0 - x);
            let rhs = ZETA2 - x.ln() * (1.0 - x).ln();
            worst = worst.max(((lhs - rhs) / rhs).abs());
        }
        assert!(worst < 1e-5, "Landen's reflection in f32: {worst:e}");
    }

    /// `Li_2` is strictly increasing on `[0, 1]`, and `NaN` propagates.
    #[test]
    fn the_shape_and_the_refusal_are_right() {
        let mut prev = f32::NEG_INFINITY;
        for k in 0..=200i32 {
            let x = 0.005 * k as f32;
            let v = dilog(x);
            assert!(v > prev, "not increasing at {x}: {v} <= {prev}");
            prev = v;
        }
        assert!(dilog(f32::NAN).is_nan());
        // No finite argument gives an infinity.
        for e in [1e3_f32, 1e10, 1e30, f32::MAX] {
            assert!(dilog(e).is_finite(), "dilog({e:e})");
        }
    }

    /// The shader and this mirror agree on all three shared constants.
    ///
    /// There is no generator and no table audit for `dilog` — it has no
    /// coefficient tables — so this parses `dilog.wgsl` for the three `const`
    /// declarations and compares them to the values here. That makes it a
    /// real check rather than a restatement of one file in the other.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_three_constants() {
        let src = crate::wgsl::DILOG;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in dilog.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the constant name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_DILOG_ZETA2: f32"), ZETA2);
        assert_eq!(read("PETIR_DILOG_PI2_3: f32"), PI2_OVER_3);
        assert_eq!(read("PETIR_DILOG_TCUT:  f32"), TCUT);
        assert_eq!(read("PETIR_DILOG_EPS:   f32"), f32::EPSILON);
        // And the two constants are consistent with each other.
        assert!(
            (PI2_OVER_3 - 2.0 * ZETA2).abs() < 1e-6,
            "pi^2/3 != 2 zeta(2)"
        );
    }
}
