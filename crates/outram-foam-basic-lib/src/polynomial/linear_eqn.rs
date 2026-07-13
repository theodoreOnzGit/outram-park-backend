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

use crate::primitives::scalar::{SMALL, VSMALL, VGREAT};
use super::roots::{RootType, Roots};

/// Solves `a·x + b = 0`. Maps to `Foam::linearEqn`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearEqn {
    pub a: f64,
    pub b: f64,
}

/// Sign of x: +1 if x ≥ 0, else −1. Matches OpenFOAM `sign()`.
#[inline]
fn sign(x: f64) -> f64 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

impl LinearEqn {
    #[inline]
    pub fn new(a: f64, b: f64) -> Self {
        Self { a, b }
    }

    /// Evaluate `a·x + b`.
    #[inline]
    pub fn value(&self, x: f64) -> f64 {
        x * self.a + self.b
    }

    /// Derivative = `a` (constant).
    #[inline]
    pub fn derivative(&self, _x: f64) -> f64 {
        self.a
    }

    /// Floating-point error estimate at `x`.
    #[inline]
    pub fn error(&self, x: f64) -> f64 {
        SMALL * ((x * self.a).abs() + self.b.abs())
    }

    /// Return the single root of `a·x + b = 0`.
    ///
    /// - `Nan`    if `|a| < VSMALL` (degenerate)
    /// - `±Inf`   if `b` overflows `a` in magnitude
    /// - `Real`   otherwise: `−b/a`
    pub fn roots(&self) -> Roots<1> {
        let a = self.a;
        let b = self.b;

        if a.abs() < VSMALL {
            return Roots::new(RootType::Nan, 0.0);
        }
        if (b / VGREAT).abs() >= a.abs() {
            let t = if sign(a) == sign(b) {
                RootType::NegInf
            } else {
                RootType::PosInf
            };
            return Roots::new(t, 0.0);
        }

        Roots::new(RootType::Real, -b / a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_root() {
        // 2x - 4 = 0 → x = 2
        let eq = LinearEqn::new(2.0, -4.0);
        let r = eq.roots();
        assert_eq!(r.root_type(0), RootType::Real);
        assert!((r[0] - 2.0).abs() < 1e-14);
    }

    #[test]
    fn degenerate_zero_leading() {
        // 0·x + 5 = 0 → no solution
        let r = LinearEqn::new(0.0, 5.0).roots();
        assert_eq!(r.root_type(0), RootType::Nan);
    }

    #[test]
    fn overflow_root() {
        // a = 1e-300 (tiny), b = 1.0 → root would be -1/1e-300 ≈ -∞
        let r = LinearEqn::new(1e-300, 1.0).roots();
        assert_eq!(r.root_type(0), RootType::NegInf);
    }

    #[test]
    fn value_and_error() {
        let eq = LinearEqn::new(3.0, -6.0);
        assert_eq!(eq.value(2.0), 0.0);
        assert_eq!(eq.derivative(0.0), 3.0);
        assert!(eq.error(2.0) >= 0.0);
    }
}
