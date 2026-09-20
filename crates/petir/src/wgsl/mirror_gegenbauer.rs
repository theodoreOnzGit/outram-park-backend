//! `f32` mirror of `shaders/gegenbauer.wgsl` — the Gegenbauer
//! (ultraspherical) polynomials `C_n^lambda(x)`.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # The fourth table-free shader, and the simplest kernel here
//!
//! Three closed forms and a three-term recurrence. **A recurrence is not a
//! series**: there is no truncation, no convergence test and no tolerance
//! anywhere in the file. The loop runs exactly `n - 3` times and stops, so
//! the trip count is a uniform function of one integer parameter and a
//! workgroup evaluating one order never diverges. That is the friendliest
//! shape a kernel can have on a GPU, and it is why this one has no
//! retargeted constant of any kind — there is no constant to retarget.
//!
//! What it costs instead is accumulated rounding. That is measured across
//! orders rather than as a single worst case, because a single number would
//! hide the only interesting thing about this kernel — and the measurement
//! refused the obvious claim: the error is bounded by a **linear-in-`n`
//! envelope** but is **not monotone** in `n`, because the worst case is
//! taken over `lambda` and `x` too and the polynomial's zeros move with the
//! order. See `the_f32_cost_is_bounded_by_a_linear_in_n_envelope`.
//!
//! # `lambda = 0` is a separate branch and it is upstream's normalisation
//!
//! `C_n^lambda` vanishes identically at `lambda = 0` for `n >= 1`. What
//! upstream returns is the limit of `C_n^lambda / lambda`, which is
//! `2 T_n(x) / n` — the Chebyshev polynomials of the first kind up to
//! normalisation. Its low-order closed forms follow the same convention,
//! which is why [`gegenpoly_1`] gives `2x` and not `0` there.
//!
//! A reader who assumed the other reading would think this module wrong, so
//! `the_lambda_zero_branch_is_upstreams_normalisation` pins it.

// Under a std-linked build (`cargo test`) f32's inherent acos/cos shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `C_1^lambda(x)`. Mirrors `petir_gegenpoly_1`.
pub fn gegenpoly_1(lambda: f32, x: f32) -> f32 {
    if lambda == 0.0 {
        2.0 * x
    } else {
        2.0 * lambda * x
    }
}

/// `C_2^lambda(x)`. Mirrors `petir_gegenpoly_2`.
pub fn gegenpoly_2(lambda: f32, x: f32) -> f32 {
    if lambda == 0.0 {
        -1.0 + 2.0 * x * x
    } else {
        lambda * (-1.0 + 2.0 * (1.0 + lambda) * x * x)
    }
}

/// `C_3^lambda(x)`. Mirrors `petir_gegenpoly_3`.
pub fn gegenpoly_3(lambda: f32, x: f32) -> f32 {
    if lambda == 0.0 {
        x * (-2.0 + 4.0 / 3.0 * x * x)
    } else {
        let c = 4.0 + lambda * (6.0 + 2.0 * lambda);
        2.0 * lambda * x * (-1.0 - lambda + c * x * x / 3.0)
    }
}

/// `C_n^lambda(x)` for any order. Mirrors `petir_gegenpoly_n`.
///
/// `NaN` for `lambda <= -1/2`, which is upstream's `DOMAIN_ERROR`.
pub fn gegenpoly_n(n: u32, lambda: f32, x: f32) -> f32 {
    if lambda.is_nan() || x.is_nan() || lambda <= -0.5 {
        return f32::NAN;
    }
    match n {
        0 => return 1.0,
        1 => return gegenpoly_1(lambda, x),
        2 => return gegenpoly_2(lambda, x),
        3 => return gegenpoly_3(lambda, x),
        _ => {}
    }
    if lambda == 0.0 && (-1.0..=1.0).contains(&x) {
        let z = n as f32 * x.acos();
        return 2.0 * z.cos() / n as f32;
    }
    let mut gkm2 = gegenpoly_2(lambda, x);
    let mut gkm1 = gegenpoly_3(lambda, x);
    let mut gk = 0.0_f32;
    for k in 4..=n {
        let kf = k as f32;
        gk = (2.0 * (kf + lambda - 1.0) * x * gkm1 - (kf + 2.0 * lambda - 2.0) * gkm2) / kf;
        gkm2 = gkm1;
        gkm1 = gk;
    }
    gk
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::gegenbauer as f64_gegen;

    /// **What `f32` costs, as a function of order** — bounded by a
    /// linear-in-`n` envelope, and **not monotone**, which is the part worth
    /// recording.
    ///
    /// Measured 2026-09-20, worst over four `lambda` and 61 arguments in
    /// `[-1, 1]`, relative to the magnitude the polynomial actually reaches:
    ///
    /// | `n` | worst |
    /// |---|---|
    /// | 4 | 1.709e-06 |
    /// | 8 | 2.014e-06 |
    /// | 16 | 4.840e-06 |
    /// | 24 | 6.490e-06 |
    /// | **32** | **2.654e-05** |
    /// | 48 | 2.408e-06 |
    /// | 64 | 1.629e-05 |
    ///
    /// The trend is upward and the envelope holds, but `n = 48` is an order
    /// better than `n = 32`. **A draft of this test asserted monotone
    /// growth** — "the error grows like `n`, because that is what a
    /// recurrence does" — and the measurement does not support that. The
    /// worst case is taken over `lambda` and `x` as well as `n`, and where
    /// it lands depends on whether the sweep happens to pass near a zero of
    /// the polynomial at that order; the zeros move with `n`, so the
    /// envelope is smooth and the maximum inside it is not.
    ///
    /// What is asserted is therefore the envelope, plus the weaker and true
    /// statement that the high orders are worse than the low ones.
    #[test]
    fn the_f32_cost_is_bounded_by_a_linear_in_n_envelope() {
        let mut row = std::vec::Vec::new();
        for n in [4u32, 8, 16, 24, 32, 48, 64] {
            let mut worst = 0.0_f64;
            for &lambda in &[0.25_f32, 0.5, 1.0, 2.5] {
                for i in 0..=60 {
                    let x = -1.0 + 2.0 * i as f32 / 60.0;
                    let a = gegenpoly_n(n, lambda, x) as f64;
                    let b = f64_gegen::gegenpoly_n(n, lambda as f64, x as f64);
                    // Scaled by the magnitude reached: these grow with
                    // lambda and n, so a bare absolute figure would track
                    // that growth rather than the error.
                    let scale = b.abs().max(1.0);
                    worst = worst.max((a - b).abs() / scale);
                }
            }
            row.push((n, worst));
        }
        for (n, w) in &row {
            assert!(
                *w < 40.0 * *n as f64 * f64::from(f32::EPSILON),
                "n = {n}: {w:e} exceeds the documented 40 n eps envelope"
            );
        }
        // The high orders are worse than the low ones, taken as groups --
        // which is true where point-by-point monotonicity is not.
        let lo: f64 = row.iter().take(3).map(|(_, w)| *w).sum::<f64>() / 3.0;
        let hi: f64 = row.iter().rev().take(3).map(|(_, w)| *w).sum::<f64>() / 3.0;
        assert!(
            hi > lo,
            "the error is documented as worse at high order in aggregate: \
             mean {lo:e} over n = 4..16 against {hi:e} over n = 32..64"
        );
    }

    /// The three special cases survive `f32`.
    #[test]
    fn the_special_cases_survive_f32() {
        // lambda = 1/2 is Legendre, and P_n(+/-1) = (+/-1)^n EXACTLY -- the
        // recurrence reproduces that at f32 too, which is the same property
        // the f64 module has and the explicit-coefficient form does not.
        for n in 0..=40u32 {
            assert_eq!(gegenpoly_n(n, 0.5, 1.0), 1.0, "P_{n}(1)");
            assert_eq!(
                gegenpoly_n(n, 0.5, -1.0),
                if n % 2 == 0 { 1.0 } else { -1.0 },
                "P_{n}(-1)"
            );
        }
        // lambda = 1 is U_n.
        for n in 0..=20u32 {
            for i in 1..30 {
                let x = -1.0 + 2.0 * i as f32 / 30.0;
                let theta = x.acos();
                let want = ((n as f32 + 1.0) * theta).sin() / theta.sin();
                assert!(
                    (gegenpoly_n(n, 1.0, x) - want).abs() < 1e-3 * (1.0 + want.abs()),
                    "U_{n}({x}): {} against {want}",
                    gegenpoly_n(n, 1.0, x)
                );
            }
        }
    }

    /// `lambda = 0` is `2 T_n / n`, upstream's normalisation and not zero.
    #[test]
    fn the_lambda_zero_branch_is_upstreams_normalisation() {
        for n in 4..=20u32 {
            for i in 0..=30 {
                let x = -1.0 + 2.0 * i as f32 / 30.0;
                let want = 2.0 * (n as f32 * x.acos()).cos() / n as f32;
                assert!(
                    (gegenpoly_n(n, 0.0, x) - want).abs() < 1e-5,
                    "C_{n}^0({x}): {} against 2 T_n/n = {want}",
                    gegenpoly_n(n, 0.0, x)
                );
            }
        }
        assert_eq!(gegenpoly_1(0.0, 0.7), 1.4);
        // Outside [-1, 1] the acos branch is not taken and the recurrence
        // runs instead -- upstream's own fallthrough.
        assert!(gegenpoly_n(6, 0.0, 1.5).is_finite());
    }

    /// The refusals.
    #[test]
    fn the_refusals_match_upstream() {
        for lambda in [-0.5_f32, -0.6, -1.0] {
            assert!(gegenpoly_n(5, lambda, 0.3).is_nan(), "lambda = {lambda}");
        }
        assert!(gegenpoly_n(5, f32::NAN, 0.3).is_nan());
        assert!(gegenpoly_n(5, 0.5, f32::NAN).is_nan());
        for lambda in [0.0_f32, 0.5, 3.0] {
            assert_eq!(gegenpoly_n(0, lambda, 0.37), 1.0);
        }
    }

    /// The shader defines every function this mirror does, with the same
    /// branch structure.
    ///
    /// There are **no numeric constants at all** in this kernel — no
    /// tolerance, no cut, no machine constant — so unlike every other
    /// hand-written mirror here there is nothing to pin by parsing. What can
    /// still be checked is that the two sides have not drifted apart
    /// structurally, so this asserts the shader's branch points are the ones
    /// this file takes.
    #[test]
    fn the_shader_has_the_same_branches_and_no_constants() {
        let src = crate::wgsl::GEGENBAUER;
        for needle in [
            "fn petir_gegenpoly_1(",
            "fn petir_gegenpoly_2(",
            "fn petir_gegenpoly_3(",
            "fn petir_gegenpoly_n(",
            "lambda <= -0.5",
            "lambda == 0.0 && x >= -1.0 && x <= 1.0",
            "2.0 * cos(z) / f32(n)",
        ] {
            assert!(
                src.contains(needle),
                "gegenbauer.wgsl no longer contains {needle:?}; the shader \
                 and this mirror have drifted"
            );
        }
        // The claim that there is nothing to retarget: no `const` at all.
        assert!(
            !src.contains("\nconst "),
            "gegenbauer.wgsl is documented as having no constants to \
             retarget; it now declares one, which needs a decision recording \
             in docs/wgsl-coverage.md's taxonomy"
        );
    }
}
