//! `f32` mirror of `shaders/lambert.wgsl` — GSL's Lambert `W`, both real
//! branches.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Written by hand, like `mirror_dilog`
//!
//! The only table is the 12-term series near the branch point, so there is
//! nothing for a generator to extract and both sides are written out.
//! `the_shader_and_this_mirror_agree_on_their_constants` parses the WGSL
//! source and compares, so the check is real rather than a restatement.
//!
//! # The convergence tolerance IS retargeted, and that one is load-bearing
//!
//! Upstream stops when `|t| < 10 * DBL_EPSILON * max(|w|, 1/(|p| e^w))`.
//! `DBL_EPSILON` is 2.2e-16, and an `f32` iterate cannot come within that of
//! anything — so keeping the constant means the loop never stops early, burns
//! its whole budget, and keeps stepping after `t` has become pure rounding
//! noise.
//!
//! This is the opposite call to [`crate::wgsl::mirror_airy`], where GSL's
//! `f64` overflow threshold was kept because retargeting it *destroyed*
//! representable answers. The difference is what the constant is for: Airy's
//! is a **range guard**, which `f32`'s own overflow enforces anyway; this one
//! is a **convergence criterion**, which nothing else enforces. Measured
//! consequences are in
//! `the_f64_tolerance_would_never_terminate_the_iteration`.
//!
//! The iteration counts (10 and 32) are upstream's and are kept: in `f32`
//! convergence arrives far sooner, so they are ceilings that are not
//! approached rather than tuning.

// Under a std-linked build (`cargo test`) f32's inherent exp/ln/sqrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// The iteration's stopping tolerance.
///
/// `upstream` selects GSL's own rule, `10 eps max(|w|, 1/(|p| e^w))`, so the
/// departure below can be **measured** rather than argued. What ships is the
/// first arm.
fn tolerance_with(w: f32, p: f32, e: f32, eps: f32, upstream: bool) -> f32 {
    if upstream {
        10.0 * eps * w.abs().max(1.0 / (p.abs() * e))
    } else {
        10.0 * eps * w.abs()
    }
}

/// `1/e`. Mirrors `PETIR_LAMBERT_ONE_OVER_E`.
const ONE_OVER_E: f32 = 0.36787945;
/// `e`. Mirrors `PETIR_LAMBERT_E`.
const E: f32 = 2.7182817;

const C: [f32; 12] = [
    -1.0, 2.331644, -1.8121879, 1.9366311, -2.3535512, 3.066859, -4.1753354, 5.8580236, -8.401032,
    12.250754, -18.100697, 27.029045,
];

/// `C[i]`, without a fallible subscript — `tests/no_panic_gate.rs` rejects a
/// runtime index even where the caller bounds it.
#[cfg(test)]
fn series_coefficient(i: usize) -> f32 {
    match C.get(i) {
        Some(&v) => v,
        None => f32::NAN,
    }
}

/// The series near the branch point. Mirrors `petir_lambert_series`.
fn series_eval(r: f32) -> f32 {
    let t8 = C[8] + r * (C[9] + r * (C[10] + r * C[11]));
    let t5 = C[5] + r * (C[6] + r * (C[7] + r * t8));
    let t1 = C[1] + r * (C[2] + r * (C[3] + r * (C[4] + r * t5)));
    C[0] + r * t1
}

/// Upstream's mixed Newton/Halley iteration. Mirrors `petir_lambert_halley`.
///
/// `eps` is the tolerance constant, a parameter rather than a literal so the
/// choice of it can be **measured** — see
/// `the_f64_tolerance_would_never_terminate_the_iteration`. `f32::EPSILON` is
/// what ships, and what the shader hard-codes.
fn halley(x: f32, w_initial: f32, max_iters: u32, eps: f32) -> (f32, u32) {
    halley_with(x, w_initial, max_iters, eps, false)
}

fn halley_with(x: f32, w_initial: f32, max_iters: u32, eps: f32, upstream_tol: bool) -> (f32, u32) {
    let mut w = w_initial;
    for i in 0..max_iters {
        let e = w.exp();
        let p = w + 1.0;
        let mut t = w * e - x;
        if w > 0.0 {
            t = (t / p) / e;
        } else {
            t /= e * p - 0.5 * (p + 1.0) * t / p;
        }
        w -= t;
        // UPSTREAM'S RULE IS `10 * eps * max(|w|, 1/(|p| e^w))`. The second
        // term is dropped here -- see the module documentation. It is what
        // makes the tolerance O(1) at f32 width once `w` is very negative.
        let tol = tolerance_with(w, p, e, eps, upstream_tol);
        if t.abs() < tol {
            return (w, i + 1);
        }
    }
    (f32::NAN, max_iters)
}

/// `W_0(x)` in `f32`. Mirrors `petir_lambert_w0`.
pub fn lambert_w0(x: f32) -> f32 {
    w0_with(x, f32::EPSILON).0
}

fn w0_with(x: f32, eps: f32) -> (f32, u32) {
    w0_full(x, eps, false)
}

fn w0_full(x: f32, eps: f32, upstream_tol: bool) -> (f32, u32) {
    if x.is_nan() {
        return (f32::NAN, 0);
    }
    let q = x + ONE_OVER_E;
    if x == 0.0 {
        return (0.0, 0);
    }
    if q < 0.0 {
        return (f32::NAN, 0);
    }
    if q == 0.0 {
        return (-1.0, 0);
    }
    if q < 1.0e-3 {
        return (series_eval(q.sqrt()), 0);
    }
    let w = if x < 1.0 {
        let p = (2.0 * E * q).sqrt();
        -1.0 + p * (1.0 + p * (-1.0 / 3.0 + p * 11.0 / 72.0))
    } else {
        let mut w = x.ln();
        if x > 3.0 {
            w -= w.ln();
        }
        w
    };
    halley_with(x, w, 10, eps, upstream_tol)
}

/// `W_{-1}(x)` in `f32`. Mirrors `petir_lambert_wm1`.
pub fn lambert_wm1(x: f32) -> f32 {
    wm1_with(x, f32::EPSILON).0
}

fn wm1_with(x: f32, eps: f32) -> (f32, u32) {
    wm1_full(x, eps, false)
}

fn wm1_full(x: f32, eps: f32, upstream_tol: bool) -> (f32, u32) {
    if x.is_nan() {
        return (f32::NAN, 0);
    }
    if x > 0.0 {
        return w0_full(x, eps, upstream_tol);
    }
    if x == 0.0 {
        return (0.0, 0);
    }
    let q = x + ONE_OVER_E;
    if q < 0.0 {
        return (f32::NAN, 0);
    }
    let w;
    if x < -1.0e-6 {
        let v = series_eval(-q.sqrt());
        if q < 3.0e-3 {
            return (v, 0);
        }
        w = v;
    } else {
        let l1 = (-x).ln();
        let l2 = (-l1).ln();
        w = l1 - l2 + l2 / l1;
    }
    halley_with(x, w, 32, eps, upstream_tol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::lambert as f64_lambert;

    /// **The defining identity `W(x) e^{W(x)} = x`, in `f32`.**
    ///
    /// The same check the `f64` module uses, and for the same reason: the
    /// returned `w` is fed back through `w e^w`, which the module never
    /// evaluates except inside the iteration's own residual.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst relative `|w e^w - x| / |x|`:
    ///
    /// | branch | window | worst |
    /// |---|---|---|
    /// | `W_0` | `[-1/e, 100]` | 3.071e-07 |
    /// | `W_{-1}` | `[-1/e, -1e-07)` | 9.982e-07 |
    ///
    /// `W_{-1}`'s figure is the one that depends on the corrected stopping
    /// rule — under upstream's it is 4.769e-03, which
    /// `upstreams_stopping_rule_fails_twice_over_in_f32` measures side by
    /// side.
    #[test]
    fn both_branches_satisfy_the_defining_identity_in_f32() {
        let (mut worst0, mut at0) = (0.0_f32, 0.0_f32);
        for k in 0..=2000 {
            let x = -ONE_OVER_E + (100.0 + ONE_OVER_E) * (k as f32 / 2000.0);
            let w = lambert_w0(x);
            assert!(w.is_finite(), "W_0({x}) = {w}");
            if x != 0.0 {
                let r = ((w * w.exp() - x) / x).abs();
                if r > worst0 {
                    worst0 = r;
                    at0 = x;
                }
            }
        }
        assert!(worst0 < 1e-4, "W_0 identity in f32: {worst0:e} at {at0}");

        let (mut worst1, mut at1) = (0.0_f32, 0.0_f32);
        for k in 0..=3000 {
            let t = k as f32 / 3000.0;
            let x = -ONE_OVER_E * (1e-7_f32 / ONE_OVER_E).powf(t);
            let w = lambert_wm1(x);
            assert!(w.is_finite(), "W_-1({x:e}) = {w}");
            let r = ((w * w.exp() - x) / x).abs();
            if r > worst1 {
                worst1 = r;
                at1 = x;
            }
        }
        assert!(worst1 < 1e-5, "W_-1 identity in f32: {worst1:e} at {at1:e}");
    }

    /// **Two things are wrong with upstream's stopping rule at `f32` width,
    /// and they are different in kind.**
    ///
    /// The rule is `|t| < 10 eps max(|w|, 1/(|p| e^w))`.
    ///
    /// 1. **`eps` is a precision constant and is retargeted.** With
    ///    `DBL_EPSILON` (2.2e-16) an `f32` iterate can never satisfy the
    ///    test. Measured over 200 probes on `W_0`: 142 burn the full
    ///    iteration budget, and the other 58 stop only because their step
    ///    became **exactly zero**, which satisfies any positive tolerance —
    ///    all 58, checked. So the rule never actually terminates the loop;
    ///    the loop runs out, or the arithmetic does. `f32::EPSILON` ships.
    /// 2. **The `1/(|p| e^w)` term makes the tolerance `O(1)` and is
    ///    dropped.** Once `w` is very negative `e^w` is tiny, so that term
    ///    explodes: at `x = -2.1e-06`, `w` is about -15.8 and the tolerance
    ///    comes out at **5.947e-01** against a shipped 1.885e-05 — more than
    ///    a hundred times the 0.005 step it then accepts as convergence,
    ///    stopping one iteration early with `w` wrong in the third decimal.
    ///    (Across that whole decade the term reaches ~12.) In `f64` the same
    ///    expression gives 2.9e-10 and is harmless, which is why upstream
    ///    can carry it.
    ///
    /// Measured over the whole `W_{-1}` domain down to `x = -1e-07`, and
    /// `W_0` over `[-1/e, 100]`, worst relative `|w e^w - x| / |x|`:
    ///
    /// | tolerance rule | `W_0` | `W_{-1}` | max iterations |
    /// |---|---|---|---|
    /// | upstream's, `eps` retargeted | 3.071e-07 | **4.769e-03** | 7 |
    /// | `10 eps |w|` (**shipped**) | 3.071e-07 | **9.982e-07** | 8 |
    ///
    /// One extra iteration for three and a half orders of magnitude, and
    /// `W_0` is untouched. Dropping the term is safe near the branch point
    /// because the iteration is not entered there at all — the `sqrt(q)`
    /// series handles it.
    ///
    /// **This is a third kind of constant decision, and it is worth keeping
    /// the three apart.** [`crate::wgsl::mirror_airy`] KEEPS GSL's `f64`
    /// overflow threshold, because that is a *range guard* and retargeting
    /// it discards representable answers. Point 1 here retargets a
    /// *precision constant*, because the `f64` value is unreachable. Point 2
    /// changes the *formula*, because it is numerically inadequate at `f32`
    /// width — the same call [`crate::wgsl::mirror_debye`] makes about
    /// carrying `xk` by subtraction.
    #[test]
    fn upstreams_stopping_rule_fails_twice_over_in_f32() {
        const DBL_EPSILON: f32 = 2.220_446e-16;

        // 1. With DBL_EPSILON the iteration never meets its tolerance.
        let (mut probes, mut hit_ceiling) = (0, 0);
        let mut worst_iters_shipped = 0;
        for k in 1..=200 {
            let x = -0.3 + 100.3 * (k as f32 / 200.0);
            let (_, n32) = w0_with(x, f32::EPSILON);
            let (_, n64) = w0_with(x, DBL_EPSILON);
            if n32 == 0 {
                continue; // a series branch; no iteration happened
            }
            probes += 1;
            worst_iters_shipped = worst_iters_shipped.max(n32);
            if n64 >= 10 {
                hit_ceiling += 1;
            }
        }
        assert!(probes > 100, "the sweep must actually reach the iteration");
        assert!(
            worst_iters_shipped <= 8,
            "with f32::EPSILON the iteration is documented as converging in \
             at most 8 steps, and took {worst_iters_shipped}"
        );
        // 142 of 200 burn the full budget. The other 58 stop, but NOT
        // because the tolerance was met -- their Newton/Halley step became
        // exactly zero, which satisfies any positive tolerance. Measured:
        // all 58. So the rule is never what terminates the loop.
        assert!(
            hit_ceiling >= 140,
            "with upstream's DBL_EPSILON the iteration is documented as \
             burning its full budget on 142 of 200 probes, and did so on \
             {hit_ceiling}"
        );
        let mut zero_step = 0;
        let mut early = 0;
        for k in 1..=200 {
            let x = -0.3 + 100.3 * (k as f32 / 200.0);
            let (_, n32) = w0_with(x, f32::EPSILON);
            let (_, n64) = w0_with(x, DBL_EPSILON);
            if n32 == 0 || n64 >= 10 {
                continue;
            }
            early += 1;
            // Re-form the step the converged iterate would take next.
            let (w, _) = w0_full(x, DBL_EPSILON, false);
            let e = w.exp();
            let p = w + 1.0;
            let mut t = w * e - x;
            if w > 0.0 {
                t = (t / p) / e;
            } else {
                t /= e * p - 0.5 * (p + 1.0) * t / p;
            }
            if t == 0.0 {
                zero_step += 1;
            }
        }
        assert_eq!(
            zero_step, early,
            "every probe that stops early under DBL_EPSILON is documented as \
             doing so on an EXACTLY ZERO step rather than on the tolerance -- \
             {zero_step} of {early} did. If some now meet the tolerance \
             genuinely, the claim that the f64 constant is unreachable in f32 \
             needs re-deriving"
        );

        // 2. And upstream's second term stops W_{-1} early.
        let sweep = |upstream_tol: bool| {
            let mut worst = 0.0_f32;
            let mut iters = 0;
            for k in 0..=3000 {
                let t = k as f32 / 3000.0;
                let x = -ONE_OVER_E * (1e-7_f32 / ONE_OVER_E).powf(t);
                let (w, n) = wm1_full(x, f32::EPSILON, upstream_tol);
                if !w.is_finite() {
                    continue;
                }
                worst = worst.max(((w * w.exp() - x) / x).abs());
                iters = iters.max(n);
            }
            (worst, iters)
        };
        let (upstream_worst, upstream_iters) = sweep(true);
        let (shipped_worst, shipped_iters) = sweep(false);
        assert!(
            upstream_worst > 100.0 * shipped_worst,
            "upstream's 1/(|p| e^w) term is documented as costing W_-1 three \
             and a half orders in f32: upstream {upstream_worst:e}, shipped \
             {shipped_worst:e}. If they now agree the term is harmless and \
             this departure should be withdrawn"
        );
        assert!(
            shipped_iters <= upstream_iters + 2,
            "the correction is documented as costing about one extra \
             iteration: {upstream_iters} against {shipped_iters}"
        );

        // The tolerance really does blow up, at the point named above.
        let w = -15.815_725_f32;
        let up = tolerance_with(w, w + 1.0, w.exp(), f32::EPSILON, true);
        let ours = tolerance_with(w, w + 1.0, w.exp(), f32::EPSILON, false);
        assert!(
            up > 0.1 && ours < 1e-4,
            "at w = {w} upstream's tolerance is documented as 5.947e-01 -- \
             O(1), and more than a hundred times the 0.005 step it wrongly \
             accepts -- against a shipped 1.885e-05: got {up:e} and {ours:e}"
        );
        // The step it accepts is the one that leaves w wrong by 0.005.
        assert!(
            up > 100.0 * 0.005,
            "upstream's tolerance is documented as comfortably admitting the \
             0.005 step that stops the iteration early, and is {up:e}"
        );
    }

    /// The two branches are distinct roots in `f32` too — without this, a
    /// defect returning the principal value from [`lambert_wm1`] would pass
    /// the identity test, since both roots satisfy it.
    #[test]
    fn the_two_branches_are_distinct_roots_in_f32() {
        for k in 1..=150 {
            let x = -ONE_OVER_E + ONE_OVER_E * (k as f32 / 151.0);
            let (a, b) = (lambert_w0(x), lambert_wm1(x));
            assert!(a >= -1.0001, "W_0({x}) = {a}");
            assert!(b <= -0.9999, "W_-1({x}) = {b}");
            assert!(b < a + 1e-6, "branches not distinct at {x}: {a}, {b}");
        }
    }

    /// Against the `f64` module — what `f32` costs.
    ///
    /// Worst relative 1.161e-07 at `x = 0.142`, measured 2026-09-19: one
    /// `f32` ulp. The iteration converges to the same root; `f32` only
    /// decides how finely it can be located.
    #[test]
    fn the_f32_cost_is_bounded() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for k in 1..=2000 {
            let x = -0.36 + 100.36 * (k as f32 / 2000.0);
            let r = f64_lambert::lambert_w0(x as f64);
            if !r.is_finite() || r.abs() < 1e-3 {
                continue;
            }
            let e = (((lambert_w0(x) as f64) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-5, "W_0 vs f64: {worst:e} at {at}");
    }

    /// The known values and the refusals, in `f32`.
    #[test]
    fn the_known_values_and_refusals_survive_f32() {
        assert_eq!(lambert_w0(0.0), 0.0);
        assert!((lambert_w0(1.0) - 0.567_143_3).abs() < 1e-6, "omega");
        assert!((lambert_w0(E) - 1.0).abs() < 1e-5, "W_0(e)");
        assert!((lambert_w0(-ONE_OVER_E) + 1.0).abs() < 1e-3, "W_0(-1/e)");
        for x in [-0.4_f32, -1.0, -1e3] {
            assert!(lambert_w0(x).is_nan(), "W_0({x})");
            assert!(lambert_wm1(x).is_nan(), "W_-1({x})");
        }
        assert!(lambert_w0(f32::NAN).is_nan());
        assert!(lambert_wm1(f32::NAN).is_nan());
        for k in 1..=50 {
            let x = 0.2 * k as f32;
            assert_eq!(lambert_wm1(x), lambert_w0(x), "delegation at {x}");
        }
    }

    /// The shader and this mirror agree on their constants, checked by
    /// parsing the WGSL source rather than restating it.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::LAMBERT;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in lambert.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_LAMBERT_ONE_OVER_E: f32"), ONE_OVER_E);
        assert_eq!(read("PETIR_LAMBERT_E: f32"), E);
        assert_eq!(read("PETIR_LAMBERT_EPS: f32"), f32::EPSILON);
        // The twelve series coefficients, in order. Named literally rather
        // than built with `format!`, which this no_std crate does not have.
        const NAMES: [&str; 12] = [
            "let c0  =",
            "let c1  =",
            "let c2  =",
            "let c3  =",
            "let c4  =",
            "let c5  =",
            "let c6  =",
            "let c7  =",
            "let c8  =",
            "let c9  =",
            "let c10 =",
            "let c11 =",
        ];
        const WANT: [f32; 12] = [
            -1.0, 2.331644, -1.8121879, 1.9366311, -2.3535512, 3.066859, -4.1753354, 5.8580236,
            -8.401032, 12.250754, -18.100697, 27.029045,
        ];
        for i in 0..12 {
            assert_eq!(
                read(NAMES[i]).to_bits(),
                WANT[i].to_bits(),
                "series coefficient {i}"
            );
            // And the mirror holds the same value.
            assert_eq!(WANT[i].to_bits(), series_coefficient(i).to_bits());
        }
    }
}
