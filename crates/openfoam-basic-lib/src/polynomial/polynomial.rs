// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

use crate::primitives::scalar::VSMALL;

/// Fixed-degree polynomial with an optional log term.
///
/// Represents `sum(coeffs[i] · xⁱ, i=0..N-1) + log_coeff · ln(x)`.
///
/// Maps to `Foam::Polynomial<N>` (`Polynomial.H`, `Polynomial.C`).
///
/// The log term is activated only via `integral_minus1`, which models
/// integrals of polynomials whose lowest-order term is `coeffs[0] · x⁻¹`.
/// The `integral() -> Polynomial<{N+1}>` form (returning one higher degree)
/// is not implemented because it requires `generic_const_exprs` (nightly);
/// use the scalar `integral(x1, x2) -> f64` form instead.
#[derive(Debug, Clone, Copy)]
pub struct Polynomial<const N: usize> {
    coeffs: [f64; N],
    log_coeff: f64,
    log_active: bool,
}

impl<const N: usize> Polynomial<N> {
    /// Construct from coefficient array (constant term first).
    /// `poly(x) = coeffs[0] + coeffs[1]·x + coeffs[2]·x² + …`
    #[inline]
    pub fn new(coeffs: [f64; N]) -> Self {
        Self { coeffs, log_coeff: 0.0, log_active: false }
    }

    #[inline]
    pub fn coeffs(&self) -> &[f64; N] {
        &self.coeffs
    }

    #[inline]
    pub fn log_coeff(&self) -> f64 {
        self.log_coeff
    }

    #[inline]
    pub fn log_active(&self) -> bool {
        self.log_active
    }

    /// Evaluate the polynomial at `x` (Horner-like accumulation, matching C++).
    pub fn value(&self, x: f64) -> f64 {
        let mut val = self.coeffs[0];
        let mut pow_x = x;
        for i in 1..N {
            val += self.coeffs[i] * pow_x;
            pow_x *= x;
        }
        if self.log_active {
            val += self.log_coeff * x.ln();
        }
        val
    }

    /// Derivative of the polynomial at `x`.
    pub fn derivative(&self, x: f64) -> f64 {
        let mut deriv = 0.0;
        if N > 1 {
            deriv = self.coeffs[1];
            let mut pow_x = x;
            for i in 2..N {
                deriv += (i as f64) * self.coeffs[i] * pow_x;
                pow_x *= x;
            }
        }
        if self.log_active {
            deriv += self.log_coeff / x;
        }
        deriv
    }

    /// Definite integral from `x1` to `x2`.
    ///
    /// When `log_active`, adds the `log_coeff · [(x·ln(x) − x)]_{x1}^{x2}` term.
    pub fn integral(&self, x1: f64, x2: f64) -> f64 {
        let mut pow_x1 = x1;
        let mut pow_x2 = x2;
        let mut integ = self.coeffs[0] * (x2 - x1);
        for i in 1..N {
            pow_x1 *= x1;
            pow_x2 *= x2;
            integ += self.coeffs[i] / (i as f64 + 1.0) * (pow_x2 - pow_x1);
        }
        if self.log_active {
            integ += self.log_coeff
                * ((x2 * x2.ln() - x2) - (x1 * x1.ln() - x1));
        }
        integ
    }

    /// Integrate a polynomial whose base starts at order −1.
    ///
    /// Treats `coeffs[0]` as the coefficient of `x⁻¹`, activating the log
    /// term in the result. All higher-order terms are shifted down by one.
    /// Returns a polynomial of the **same size** (constant term =`int_constant`).
    ///
    /// Maps to C++ `Polynomial<N>::integralMinus1()`.
    pub fn integral_minus1(&self, int_constant: f64) -> Self {
        let mut new_coeffs = [0.0f64; N];
        new_coeffs[0] = int_constant;
        for i in 1..N {
            new_coeffs[i] = self.coeffs[i] / (i as f64);
        }
        let log_coeff = self.coeffs[0];
        let log_active = log_coeff.abs() > VSMALL;
        Self { coeffs: new_coeffs, log_coeff, log_active }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_poly() {
        let p = Polynomial::new([5.0_f64; 1]);
        assert_eq!(p.value(3.0), 5.0);
        assert_eq!(p.derivative(3.0), 0.0);
        assert!((p.integral(0.0, 2.0) - 10.0).abs() < 1e-14);
    }

    #[test]
    fn linear_poly() {
        // p(x) = 1 + 2x
        let p = Polynomial::new([1.0, 2.0]);
        assert!((p.value(3.0) - 7.0).abs() < 1e-14);
        assert!((p.derivative(0.0) - 2.0).abs() < 1e-14);
        // ∫₀¹ (1 + 2x)dx = [x + x²]₀¹ = 2
        assert!((p.integral(0.0, 1.0) - 2.0).abs() < 1e-14);
    }

    #[test]
    fn quadratic_poly() {
        // p(x) = 1 + 2x + 3x²
        let p = Polynomial::new([1.0, 2.0, 3.0]);
        assert!((p.value(2.0) - 17.0).abs() < 1e-14);  // 1 + 4 + 12
        assert!((p.derivative(2.0) - 14.0).abs() < 1e-14); // 2 + 12
        // ∫₀¹ = [x + x² + x³]₀¹ = 3
        assert!((p.integral(0.0, 1.0) - 3.0).abs() < 1e-14);
    }

    #[test]
    fn integral_minus1_log_term() {
        // p(x) = 1/x + 2  (order-(-1) base) → integralMinus1 gives ln(x) + 2x
        // Here coeffs = [1.0, 2.0] means c₋₁=1, c₀=2 — after integration:
        // result: coeffs=[0, 2], log_coeff=1
        let p = Polynomial::new([1.0, 2.0]);
        let pi = p.integral_minus1(0.0);
        assert!(pi.log_active());
        assert!((pi.log_coeff() - 1.0).abs() < 1e-14);
        // pi.value(e) = 0 + 2*e + 1*ln(e) = 2e + 1
        let e = std::f64::consts::E;
        assert!((pi.value(e) - (2.0 * e + 1.0)).abs() < 1e-12);
    }

    #[test]
    fn integral_additivity() {
        // ∫₀¹ + ∫₁² = ∫₀²
        let p = Polynomial::new([1.0, 2.0, 3.0]);
        let i01 = p.integral(0.0, 1.0);
        let i12 = p.integral(1.0, 2.0);
        let i02 = p.integral(0.0, 2.0);
        assert!((i01 + i12 - i02).abs() < 1e-12);
    }
}
