use crate::primitives::scalar::{SMALL, VSMALL};
use super::roots::{RootType, Roots};
use super::linear_eqn::LinearEqn;
use super::quadratic_eqn::QuadraticEqn;

/// Solves `a·x³ + b·x² + c·x + d = 0`. Maps to `Foam::cubicEqn`.
///
/// The root-finding algorithm uses the depressed-cubic Cardano method with
/// Kahan-compensated intermediate discriminants for numerical robustness.
/// Reference: JLM = Numerical Recipes §3, with adjustments from the OpenFOAM
/// implementation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicEqn {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

#[inline]
fn sign(x: f64) -> f64 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

impl CubicEqn {
    #[inline]
    pub fn new(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self { a, b, c, d }
    }

    /// Evaluate `a·x³ + b·x² + c·x + d` (Horner form).
    #[inline]
    pub fn value(&self, x: f64) -> f64 {
        x * (x * (x * self.a + self.b) + self.c) + self.d
    }

    /// Derivative `3a·x² + 2b·x + c` (Horner form).
    #[inline]
    pub fn derivative(&self, x: f64) -> f64 {
        x * (x * 3.0 * self.a + 2.0 * self.b) + self.c
    }

    /// Floating-point error estimate at `x`.
    #[inline]
    pub fn error(&self, x: f64) -> f64 {
        let x2 = x * x;
        SMALL * x2 * ((x * self.a).abs() + self.b.abs())
            + SMALL * x.abs() * ((x * (x * self.a + self.b)).abs() + self.c.abs())
            + SMALL * ((x * (x * (x * self.a + self.b) + self.c)).abs() + self.d.abs())
    }

    /// Roots of `a·x³ + b·x² + c·x + d = 0`.
    ///
    /// Case dispatch:
    /// - `a ≈ 0`: falls back to `QuadraticEqn` (result + Nan)
    /// - `discr < 0`: three distinct real roots (casus irreducibilis via complex cube root)
    /// - `discr > 0`: one real + one complex conjugate pair
    /// - `discr = 0, p ≠ 0`: one simple + one repeated real root
    /// - `discr = 0, p = 0`: one triple real root
    pub fn roots(&self) -> Roots<3> {
        let a = self.a;
        let b = self.b;
        let c = self.c;
        let d = self.d;

        if a.abs() < VSMALL {
            return Roots::<3>::with_tail(QuadraticEqn::new(b, c, d).roots(), RootType::Nan, 0.0);
        }

        // Kahan-compensated p = a·c − b²/3
        let w = a * c;
        let p = -(f64::mul_add(-a, c, w) + f64::mul_add(b, b / 3.0, -w));
        let q = b * b * b * 2.0 / 27.0 - b * c * a / 3.0 + d * a * a;
        let num_discr = p * p * p / 27.0 + q * q / 4.0;
        let discr = if num_discr.abs() > VSMALL { num_discr } else { 0.0 };

        let three_real = discr < 0.0;
        let one_real_two_complex = discr > 0.0;
        let two_real = p.abs() > SMALL.sqrt() && !(three_real || one_real_two_complex);

        let sqrt3 = 3.0_f64.sqrt();

        let x: f64;

        if three_real {
            let w_cb_re = -q / 2.0;
            let w_cb_im = (-discr).sqrt();
            let w_abs = w_cb_re.hypot(w_cb_im).cbrt();
            let w_arg = w_cb_im.atan2(w_cb_re) / 3.0;
            let w_re = w_abs * w_arg.cos();
            let w_im = w_abs * w_arg.sin();

            x = if b > 0.0 {
                -w_re - w_im.abs() * sqrt3 - b / 3.0
            } else {
                2.0 * w_re - b / 3.0
            };
        } else if one_real_two_complex {
            let w_cb = -q / 2.0 - sign(q) * discr.sqrt();
            let w = w_cb.cbrt();
            let t = w - p / (3.0 * w);

            if p + t * b < 0.0 {
                x = t - b / 3.0;
            } else {
                let x_re = -t / 2.0 - b / 3.0;
                let x_im = sqrt3 / 2.0 * (w + p / 3.0 / w);
                // Product-of-roots gives the third (real) root
                return Roots::concat_1_2(
                    Roots::<1>::new(
                        RootType::Real,
                        -a * d / (x_re * x_re + x_im * x_im),
                    ),
                    Roots::<2>::from_pair(
                        Roots::<1>::new(RootType::Complex, x_re),
                        Roots::<1>::new(RootType::Complex, x_im),
                    ),
                );
            }
        } else if two_real {
            if q * b > 0.0 {
                x = -2.0 * (q / 2.0).cbrt() - b / 3.0;
            } else {
                x = (q / 2.0).cbrt() - b / 3.0;
                let r = LinearEqn::new(-a, x).roots();
                return Roots::concat_2_1(
                    Roots::<2>::both(r),
                    LinearEqn::new(x * x, a * d).roots(),
                );
            }
        } else {
            // Triple root
            let r = LinearEqn::new(a, b / 3.0).roots();
            return Roots::uniform(r.root_type(0), r[0]);
        }

        // Common path for three_real, partial one_real_two_complex, and one twoReal branch
        Roots::concat_1_2(
            LinearEqn::new(-a, x).roots(),
            QuadraticEqn::new(-x * x, c * x + a * d, d * x).roots(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_roots(r: Roots<3>) -> Vec<f64> {
        let mut v: Vec<f64> = (0..3)
            .filter(|&i| r.root_type(i) == RootType::Real)
            .map(|i| r[i])
            .collect();
        v.sort_by(f64::total_cmp);
        v
    }

    #[test]
    fn three_real_integer() {
        // (x-1)(x-2)(x-3) = x³ - 6x² + 11x - 6
        let eq = CubicEqn::new(1.0, -6.0, 11.0, -6.0);
        let roots = real_roots(eq.roots());
        assert_eq!(roots.len(), 3);
        assert!((roots[0] - 1.0).abs() < 1e-10, "r0={}", roots[0]);
        assert!((roots[1] - 2.0).abs() < 1e-10, "r1={}", roots[1]);
        assert!((roots[2] - 3.0).abs() < 1e-10, "r2={}", roots[2]);
    }

    #[test]
    fn one_real_two_complex() {
        // x³ + x + 1 = 0  → one real root ≈ -0.6823...
        let eq = CubicEqn::new(1.0, 0.0, 1.0, 1.0);
        let r = eq.roots();
        let reals = real_roots(r);
        assert_eq!(reals.len(), 1);
        let x = reals[0];
        assert!(eq.value(x).abs() < 1e-10, "residual={}", eq.value(x));
    }

    #[test]
    fn triple_root() {
        // (x-2)³ = x³ - 6x² + 12x - 8
        let eq = CubicEqn::new(1.0, -6.0, 12.0, -8.0);
        let r = eq.roots();
        for i in 0..3 {
            assert_eq!(r.root_type(i), RootType::Real, "slot {i}");
            assert!((r[i] - 2.0).abs() < 1e-8, "r[{i}]={}", r[i]);
        }
    }

    #[test]
    fn degenerate_to_quadratic() {
        // 0·x³ + 1·x² - 3x + 2 = (x-1)(x-2) → roots 1, 2 plus Nan
        let eq = CubicEqn::new(0.0, 1.0, -3.0, 2.0);
        let mut reals = real_roots(eq.roots());
        reals.sort_by(f64::total_cmp);
        assert_eq!(reals.len(), 2);
        assert!((reals[0] - 1.0).abs() < 1e-13);
        assert!((reals[1] - 2.0).abs() < 1e-13);
    }

    #[test]
    fn value_at_real_roots() {
        let eq = CubicEqn::new(1.0, -6.0, 11.0, -6.0);
        for x in real_roots(eq.roots()) {
            assert!(eq.value(x).abs() < 1e-10, "residual at x={x}: {}", eq.value(x));
        }
    }
}
