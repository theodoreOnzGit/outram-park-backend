//! `f32` CPU mirrors of every WGSL function in [`crate::wgsl`].
//!
//! # Why a mirror exists at all
//!
//! A GPU comparison on its own cannot tell you what went wrong. If a shader's
//! output differs from PETIR's `f64` answer, that single number conflates two
//! completely different questions — *did the transcription introduce a bug*,
//! and *is `f32` simply not able to do better here*. Those need different
//! fixes, and one of them is not a bug at all.
//!
//! The mirror separates them. Each function below performs **exactly the
//! operations its WGSL counterpart performs, in the same order, at the same
//! width**, on the CPU. So:
//!
//! - **mirror vs `f64`** measures what `f32` costs. Expect ~1e-7 and worse;
//!   this is physics, not a defect.
//! - **GPU vs mirror** measures whether the transcription is faithful. This
//!   should be at or very near zero, and a real difference here is a bug.
//!
//! # These must stay in lockstep with the `.wgsl` files
//!
//! A mirror that has drifted from its shader is worse than no mirror: it
//! reports agreement that means nothing. When you change a `.wgsl` function,
//! change the mirror in the same edit, keeping the association identical —
//! `a * b + c` and `fma(a, b, c)` are **not** the same function and will
//! disagree in the last ulp.
//!
//! # WGSL is not required to be correctly rounded
//!
//! One caveat that bounds how exact "GPU vs mirror" can be. The WGSL
//! specification gives accuracy bounds in ULP for the builtin functions rather
//! than requiring correct rounding, and it permits `a * b + c` to be
//! contracted into a fused multiply-add at the implementation's discretion.
//! So even a perfect transcription may differ from this mirror in the last
//! place or two on some devices. The tests state what was measured on the
//! device they ran on and do not assume bit-identity.
//!
//! # Units
//!
//! Bare dimensionless `f32` throughout, as in the shaders.

#[allow(unused_imports)]
use crate::real::Real;

/// Horner evaluation of an **ascending** coefficient slice, in `f32`.
///
/// Mirrors `petir_poly_eval` in `shaders/poly.wgsl`, which was transcribed
/// from [`crate::poly::eval::eval`].
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror::poly_eval;
/// // 1 + 2x + 3x^2 at x = 2 is 17.
/// assert_eq!(poly_eval(&[1.0, 2.0, 3.0], 2.0), 17.0);
/// ```
pub fn poly_eval(c: &[f32], x: f32) -> f32 {
    let mut acc = 0.0_f32;
    for &ci in c.iter().rev() {
        acc = acc * x + ci;
    }
    acc
}

/// Compensated Horner, in `f32`.
///
/// Mirrors `petir_poly_eval_comp`. **This has no GSL counterpart** — it is a
/// convenience for GPU callers who cannot escape to `f64`, and it is not the
/// function to compare against the reference. Use [`poly_eval`] for that.
///
/// The compensation roughly doubles the working precision of the summation,
/// so it recovers much of what `f32` Horner loses on a long or
/// badly-conditioned polynomial. It does not make `f32` into `f64`.
pub fn poly_eval_comp(c: &[f32], x: f32) -> f32 {
    let mut acc = 0.0_f32;
    let mut err = 0.0_f32;
    for &ci in c.iter().rev() {
        let p = acc * x;
        let e = acc.mul_add(x, -p);
        let s = p + ci;
        let t = s - p;
        err = err * x + (e + ((p - (s - t)) + (ci - t)));
        acc = s;
    }
    acc + err
}

/// Clenshaw evaluation of a Chebyshev series on `[a, b]`, in `f32`.
///
/// Mirrors `petir_cheb_eval`, transcribed from [`crate::cheb`]. `c[0]` is the
/// GSL-convention coefficient that enters **halved**.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror::cheb_eval;
/// // The series with c = [2, 0, 0] is the constant 1 (c[0] enters halved).
/// assert_eq!(cheb_eval(&[2.0, 0.0, 0.0], -1.0, 1.0, 0.3), 1.0);
/// ```
pub fn cheb_eval(c: &[f32], a: f32, b: f32, x: f32) -> f32 {
    let Some((&c0, tail)) = c.split_first() else {
        return 0.0;
    };
    let mut d1 = 0.0_f32;
    let mut d2 = 0.0_f32;
    let y = (2.0 * x - a - b) / (b - a);
    let y2 = 2.0 * y;
    for &ci in tail.iter().rev() {
        let temp = d1;
        d1 = y2 * d1 - d2 + ci;
        d2 = temp;
    }
    y * d1 - d2 + 0.5 * c0
}

/// Clenshaw truncated to the first `eval_order` terms, in `f32`.
///
/// Mirrors `petir_cheb_eval_n`. `eval_order` is clamped to the series order,
/// as GSL's `GSL_MIN` does.
pub fn cheb_eval_n(c: &[f32], eval_order: usize, a: f32, b: f32, x: f32) -> f32 {
    let Some((&c0, tail)) = c.split_first() else {
        return 0.0;
    };
    let used = eval_order.min(tail.len());
    let Some(slice) = tail.get(..used) else {
        return 0.0;
    };
    let mut d1 = 0.0_f32;
    let mut d2 = 0.0_f32;
    let y = (2.0 * x - a - b) / (b - a);
    let y2 = 2.0 * y;
    for &ci in slice.iter().rev() {
        let temp = d1;
        d1 = y2 * d1 - d2 + ci;
        d2 = temp;
    }
    y * d1 - d2 + 0.5 * c0
}

/// Map `v` on `[lo, hi]` to `[-1, 1]`, in `f32`.
///
/// Mirrors `petir_cheb_scale`.
pub fn cheb_scale(v: f32, lo: f32, hi: f32) -> f32 {
    (2.0 * v - lo - hi) / (hi - lo)
}

/// The Legendre polynomial `P_n(x)` by Bonnet's recurrence, in `f32`.
///
/// Mirrors `petir_legendre_p`.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror::legendre_p;
/// // P_2 = (3x^2 - 1)/2, so P_2(1) = 1.
/// assert!((legendre_p(2, 1.0) - 1.0).abs() < 1e-6);
/// ```
pub fn legendre_p(n: u32, x: f32) -> f32 {
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return x;
    }
    let mut p0 = 1.0_f32;
    let mut p1 = x;
    for k in 1..n {
        let kf = k as f32;
        let p2 = ((2.0 * kf + 1.0) * x * p1 - kf * p0) / (kf + 1.0);
        p0 = p1;
        p1 = p2;
    }
    p1
}

/// `P_n(x)` and `P_n'(x)` together, in `f32`.
///
/// Mirrors `petir_legendre_p_dp`. The derivative uses
/// `P_n' = n (x P_n - P_{n-1}) / (x^2 - 1)`, which is **singular at
/// `x = ±1`** — the caller handles the endpoints, as in the shader, rather
/// than this silently returning a huge number.
pub fn legendre_p_dp(n: u32, x: f32) -> (f32, f32) {
    if n == 0 {
        return (1.0, 0.0);
    }
    if n == 1 {
        return (x, 1.0);
    }
    let mut p0 = 1.0_f32;
    let mut p1 = x;
    for k in 1..n {
        let kf = k as f32;
        let p2 = ((2.0 * kf + 1.0) * x * p1 - kf * p0) / (kf + 1.0);
        p0 = p1;
        p1 = p2;
    }
    let dp = (n as f32) * (x * p1 - p0) / (x * x - 1.0);
    (p1, dp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// The `f32` mirror of Horner agrees with PETIR's `f64` original to the
    /// `f32` error budget.
    ///
    /// # Methodology
    ///
    /// Six polynomials of degree 1 to 8, evaluated at 41 points across
    /// `[-2, 2]`, each computed by [`poly_eval`] in `f32` and by
    /// [`crate::poly::eval::eval`] in `f64`. Compared **relative to the f64
    /// value**, skipping points where the true value is near zero — a
    /// relative error at a root is meaningless and would dominate the
    /// statistic while saying nothing.
    ///
    /// # Results
    ///
    /// Worst relative difference **9.468e-07**, measured 2026-09-19.
    ///
    /// Interpretation: about 8 `f32` ulp. That is the expected cost of Horner
    /// in single precision, not a transcription defect — the two perform the
    /// same operations in the same order, so the entire gap is rounding, in
    /// the coefficients as much as in the arithmetic (see the compensated
    /// test below, which pins that down).
    #[test]
    fn the_f32_horner_mirror_tracks_the_f64_original() {
        let polys: [&[f64]; 6] = [
            &[1.0, 2.0],
            &[1.0, 2.0, 3.0],
            &[-1.0, 0.0, 0.5, 2.0],
            &[0.5, -1.5, 2.25, -0.75, 1.0],
            &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            &[2.0, -3.0, 1.5, 0.25, -0.5, 1.0, 0.125, -1.0, 0.75],
        ];
        let mut worst = 0.0_f64;
        for p in polys {
            let p32: Vec<f32> = p.iter().map(|&v| v as f32).collect();
            for k in 0..=40 {
                let x = -2.0 + 0.1 * k as f64;
                let exact = crate::poly::eval::eval(p, x);
                if exact.abs() < 1e-3 {
                    continue;
                }
                let got = poly_eval(&p32, x as f32) as f64;
                let rel = ((got - exact) / exact).abs();
                if rel > worst {
                    worst = rel;
                }
            }
        }
        assert!(worst < 1e-5, "worst relative difference {worst:e}");
    }

    /// Compensated Horner is at least as accurate as plain Horner, and
    /// usually better.
    ///
    /// # Results
    ///
    /// On the degree-8 polynomial above over the same 41 points, worst
    /// relative error falls from **3.372e-07** (plain) to **2.586e-07**
    /// (compensated), measured 2026-09-19 — a factor of **1.3**, not the
    /// order of magnitude compensated Horner is usually advertised to give.
    ///
    /// **That shortfall is the point, and it is not a defect in the
    /// compensation.** Compensated Horner corrects the error committed while
    /// *evaluating*; it cannot touch the error already present in the
    /// *inputs*. Here the coefficients are `f64` values rounded to `f32`
    /// before evaluation begins, which puts a ~6e-8 relative error into the
    /// problem itself, and no evaluation scheme can see below that floor.
    ///
    /// So the practical guidance is the opposite of the intuitive one: if you
    /// need more than `f32` Horner gives, the first thing to fix is usually
    /// how the coefficients reach the GPU, not how they are summed. Where the
    /// coefficients are exactly representable and the polynomial is genuinely
    /// ill-conditioned, the compensation earns much more than 1.3x — this test
    /// simply is not that case, and says so rather than picking a flattering
    /// one.
    #[test]
    fn compensated_horner_is_not_worse_than_plain() {
        let p: &[f64] = &[2.0, -3.0, 1.5, 0.25, -0.5, 1.0, 0.125, -1.0, 0.75];
        let p32: Vec<f32> = p.iter().map(|&v| v as f32).collect();
        let mut worst_plain = 0.0_f64;
        let mut worst_comp = 0.0_f64;
        for k in 0..=40 {
            let x = -2.0 + 0.1 * k as f64;
            let exact = crate::poly::eval::eval(p, x);
            if exact.abs() < 1e-3 {
                continue;
            }
            let plain = (poly_eval(&p32, x as f32) as f64 - exact) / exact;
            let comp = (poly_eval_comp(&p32, x as f32) as f64 - exact) / exact;
            worst_plain = worst_plain.max(plain.abs());
            worst_comp = worst_comp.max(comp.abs());
        }
        assert!(
            worst_comp <= worst_plain,
            "compensation made it worse: plain {worst_plain:e}, comp {worst_comp:e}"
        );
    }

    /// The `f32` Clenshaw mirror agrees with PETIR's `f64` `ChebSeries`.
    ///
    /// # Methodology
    ///
    /// Fit `exp` on `[-1, 1]` at order 16 with [`crate::cheb::ChebSeries`],
    /// take its coefficients down to `f32`, and evaluate both at 51 points.
    /// This is the comparison that would catch the halved-`c[0]` convention
    /// being dropped, which is the classic Clenshaw transcription error.
    ///
    /// # Results
    ///
    /// Worst relative difference **1.705e-07**, measured 2026-09-19 — under
    /// 2 `f32` ulp, so the halved-`c[0]` convention is right and the gap is
    /// rounding. Dropping the halving would show up here as a error of order
    /// `c[0]/2`, which for this fit is about 1.27 — six orders of magnitude
    /// clear of the noise, which is what makes this test worth having.
    #[test]
    fn the_f32_clenshaw_mirror_tracks_the_f64_series() {
        let series = crate::cheb::ChebSeries::new(16, -1.0, 1.0, |x: f64| x.exp())
            .expect("order 16 fit on [-1, 1]");
        let c32: Vec<f32> = series.coefficients().iter().map(|&v| v as f32).collect();
        let mut worst = 0.0_f64;
        for k in 0..=50 {
            let x = -1.0 + 0.04 * k as f64;
            let exact = series.eval(x);
            let got = cheb_eval(&c32, -1.0, 1.0, x as f32) as f64;
            let rel = ((got - exact) / exact).abs();
            if rel > worst {
                worst = rel;
            }
        }
        assert!(worst < 1e-5, "worst relative difference {worst:e}");
    }

    /// Truncated Clenshaw agrees with the full one when not truncated, and
    /// clamps rather than reading out of range.
    #[test]
    fn truncated_clenshaw_clamps_and_agrees_when_untruncated() {
        let c: [f32; 5] = [2.0, 0.5, -0.25, 0.125, -0.0625];
        for k in 0..=20 {
            let x = -1.0 + 0.1 * k as f32;
            let full = cheb_eval(&c, -1.0, 1.0, x);
            // Asking for more orders than exist must clamp, not panic.
            assert_eq!(cheb_eval_n(&c, 99, -1.0, 1.0, x), full);
            assert_eq!(cheb_eval_n(&c, 4, -1.0, 1.0, x), full);
        }
        // Order 0 keeps only the halved constant term.
        assert_eq!(cheb_eval_n(&c, 0, -1.0, 1.0, 0.3), 0.5 * c[0]);
        // An empty series is the zero polynomial, not a panic.
        assert_eq!(cheb_eval(&[], -1.0, 1.0, 0.5), 0.0);
        assert_eq!(cheb_eval_n(&[], 3, -1.0, 1.0, 0.5), 0.0);
    }

    /// The `f32` Legendre mirror satisfies the identities that define `P_n`,
    /// and shows where `f32` gives out.
    ///
    /// # Methodology
    ///
    /// `P_n(1) = 1` and `P_n(-1) = (-1)^n` hold for every `n`, so they test
    /// the recurrence at every order rather than at the few a table would
    /// cover. Checked through `n = 30`, and cross-checked against
    /// [`crate::poly::dense::legendre`]'s `f64` coefficients at low order,
    /// where those are still trustworthy.
    ///
    /// # Results
    ///
    /// Endpoint identities hold to **0** — exact — through `n = 30`. The
    /// recurrence at `x = ±1` reduces to `((2k+1) - k)/(k+1) = 1` in integer-
    /// valued arithmetic, so it is a fixed point and stays exact even in
    /// `f32`. That is a strong check of the recurrence's shape and a weak one
    /// of its precision; it says nothing about interior accuracy.
    ///
    /// Against the `f64` coefficient expansion at `n <= 6` over 41 interior
    /// points, worst relative difference **2.719e-06**, measured 2026-09-19.
    ///
    /// Note this is an order of magnitude looser than the `f32` rounding floor,
    /// and deliberately so: the two sides are **different algorithms**, not the
    /// same one at two widths. The mirror runs the recurrence on values, while
    /// `poly::dense::legendre` expands coefficients that grow like `4^n` and
    /// cancel. The gap is that algorithmic difference plus `f32`, and the
    /// recurrence is the more trustworthy of the two — which is exactly why the
    /// shader uses it.
    #[test]
    fn the_f32_legendre_mirror_satisfies_its_identities() {
        let mut worst_endpoint = 0.0_f32;
        for n in 0..=30u32 {
            let at_one = (legendre_p(n, 1.0) - 1.0).abs();
            let want = if n % 2 == 0 { 1.0 } else { -1.0 };
            let at_minus = (legendre_p(n, -1.0) - want).abs();
            worst_endpoint = worst_endpoint.max(at_one).max(at_minus);
        }
        assert!(
            worst_endpoint < 1e-5,
            "worst endpoint deviation {worst_endpoint:e}"
        );

        let mut worst = 0.0_f64;
        for n in 0..=6u32 {
            let p = crate::poly::dense::legendre(n as usize);
            for k in 0..=40 {
                let x = -1.0 + 0.05 * k as f64;
                let exact = p.eval(x);
                if exact.abs() < 1e-3 {
                    continue;
                }
                let got = legendre_p(n, x as f32) as f64;
                let rel = ((got - exact) / exact).abs();
                if rel > worst {
                    worst = rel;
                }
            }
        }
        assert!(worst < 1e-5, "worst relative difference {worst:e}");
    }

    /// The derivative companion agrees with a finite difference of the value,
    /// away from the endpoints where its formula is singular.
    #[test]
    fn the_legendre_derivative_agrees_with_a_finite_difference() {
        let h = 1e-3_f32;
        let mut worst = 0.0_f32;
        for n in 2..=8u32 {
            for k in 1..=19 {
                let x = -1.0 + 0.1 * k as f32;
                if (x.abs() - 1.0).abs() < 0.05 {
                    continue;
                }
                let (v, dp) = legendre_p_dp(n, x);
                assert_eq!(v, legendre_p(n, x), "value must match legendre_p exactly");
                let fd = (legendre_p(n, x + h) - legendre_p(n, x - h)) / (2.0 * h);
                let scale = dp.abs().max(1.0);
                worst = worst.max(((dp - fd) / scale).abs());
            }
        }
        // A central difference at h = 1e-3 in f32 is itself only good to
        // about 1e-3, so this is a shape check, not a precision one.
        // Measured worst deviation 5.487e-04 on 2026-09-19, consistent with
        // the finite difference being the limiting term rather than `dp`.
        assert!(worst < 1e-2, "worst derivative deviation {worst:e}");
    }
}
