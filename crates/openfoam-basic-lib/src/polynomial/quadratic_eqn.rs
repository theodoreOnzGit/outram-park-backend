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

use crate::primitives::scalar::{SMALL, VSMALL};
use super::roots::{RootType, Roots};
use super::linear_eqn::LinearEqn;

/// Solves `a·x² + b·x + c = 0`. Maps to `Foam::quadraticEqn`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadraticEqn {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

#[inline]
fn sign(x: f64) -> f64 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

impl QuadraticEqn {
    #[inline]
    pub fn new(a: f64, b: f64, c: f64) -> Self {
        Self { a, b, c }
    }

    /// Evaluate `a·x² + b·x + c` (Horner form).
    #[inline]
    pub fn value(&self, x: f64) -> f64 {
        x * (x * self.a + self.b) + self.c
    }

    /// Derivative `2a·x + b`.
    #[inline]
    pub fn derivative(&self, x: f64) -> f64 {
        x * 2.0 * self.a + self.b
    }

    /// Floating-point error estimate at `x`.
    #[inline]
    pub fn error(&self, x: f64) -> f64 {
        SMALL * x.abs() * ((x * self.a).abs() + self.b.abs())
            + SMALL * ((x * (x * self.a + self.b)).abs() + self.c.abs())
    }

    /// Roots of `a·x² + b·x + c = 0`.
    ///
    /// Uses Kahan-compensated discriminant for numerical accuracy (JLM §3.3).
    /// Returns:
    /// - two `Real` roots if discriminant > 0
    /// - one `Real` + one `Nan` if `a ≈ 0` (falls back to linear)
    /// - two `Complex` roots (Re, Im pair) if discriminant < 0
    /// - two identical `Real` roots if discriminant = 0
    pub fn roots(&self) -> Roots<2> {
        let a = self.a;
        let b = self.b;
        let c = self.c;

        if a.abs() < VSMALL {
            return Roots::<2>::with_tail(LinearEqn::new(b, c).roots(), RootType::Nan, 0.0);
        }

        // Numerically accurate discriminant b²/4 − a·c via FMA compensation
        let w = a * c;
        let num_discr = f64::mul_add(-a, c, w) + f64::mul_add(b, b / 4.0, -w);
        let discr = if num_discr.abs() > VSMALL { num_discr } else { 0.0 };

        if discr > 0.0 {
            // Two distinct real roots — use numerically stable form
            let x = -b / 2.0 - sign(b) * discr.sqrt();
            Roots::from_pair(
                LinearEqn::new(-a, x).roots(),
                LinearEqn::new(-x, c).roots(),
            )
        } else if discr < 0.0 {
            // Complex conjugate pair: Re ± Im·i
            let x_re = Roots::<1>::new(RootType::Complex, -b / 2.0 / a);
            let x_im = Roots::<1>::new(
                RootType::Complex,
                sign(b) * (-discr).sqrt() / a,
            );
            Roots::from_pair(x_re, x_im)
        } else {
            // One repeated real root
            let r = LinearEqn::new(a, b / 2.0).roots();
            Roots::both(r)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_real_roots(r: Roots<2>) -> Vec<f64> {
        (0..2)
            .filter(|&i| r.root_type(i) == RootType::Real)
            .map(|i| r[i])
            .collect()
    }

    #[test]
    fn two_distinct_real() {
        // (x-1)(x-2) = x² - 3x + 2
        let eq = QuadraticEqn::new(1.0, -3.0, 2.0);
        let mut roots = collect_real_roots(eq.roots());
        roots.sort_by(f64::total_cmp);
        assert!((roots[0] - 1.0).abs() < 1e-13);
        assert!((roots[1] - 2.0).abs() < 1e-13);
    }

    #[test]
    fn two_complex_roots() {
        // x² + 1 = 0  →  ±i
        let eq = QuadraticEqn::new(1.0, 0.0, 1.0);
        let r = eq.roots();
        assert_eq!(r.root_type(0), RootType::Complex);
        assert_eq!(r.root_type(1), RootType::Complex);
        assert!((r[0]).abs() < 1e-14);   // Re = 0
        assert!((r[1] - 1.0).abs() < 1e-14); // Im = 1
    }

    #[test]
    fn repeated_real_root() {
        // (x - 3)² = x² - 6x + 9
        let eq = QuadraticEqn::new(1.0, -6.0, 9.0);
        let r = eq.roots();
        assert_eq!(r.root_type(0), RootType::Real);
        assert_eq!(r.root_type(1), RootType::Real);
        assert!((r[0] - 3.0).abs() < 1e-13);
        assert!((r[1] - 3.0).abs() < 1e-13);
    }

    #[test]
    fn degenerate_falls_back_to_linear() {
        // 0·x² + 2x - 4 = 0  →  x = 2
        let eq = QuadraticEqn::new(0.0, 2.0, -4.0);
        let r = eq.roots();
        assert_eq!(r.root_type(0), RootType::Real);
        assert!((r[0] - 2.0).abs() < 1e-14);
        assert_eq!(r.root_type(1), RootType::Nan);
    }

    #[test]
    fn value_at_root() {
        let eq = QuadraticEqn::new(1.0, -3.0, 2.0);
        for r in collect_real_roots(eq.roots()) {
            assert!(eq.value(r).abs() < 1e-12);
        }
    }
}
