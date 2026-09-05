// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Minimal complex arithmetic for pole/zero mapping.
//!
//! # What belongs here
//!
//! Exactly the operations the z-domain conversions need: add/sub/mul/div,
//! `exp`, principal-branch `ln`, magnitude. All values are dimensionless
//! `f64` pairs — poles and zeros in rad/s (continuous) or on the z-plane
//! (dimensionless), so `uom` typing does not apply.
//!
//! # What does not belong here
//!
//! A general complex-number library. This is deliberately private
//! (`pub(crate)`) and tiny so the crate does not grow a new dependency for
//! four arithmetic operators and two transcendental functions.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A complex number `re + i*im` (dimensionless `f64` components).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cplx {
    /// Real part (dimensionless).
    pub re: f64,
    /// Imaginary part (dimensionless).
    pub im: f64,
}

impl Cplx {
    /// Builds `re + i*im`.
    pub fn new(re: f64, im: f64) -> Self {
        Cplx { re, im }
    }

    /// Builds a purely real complex number.
    pub fn real(re: f64) -> Self {
        Cplx { re, im: 0.0 }
    }

    /// Magnitude `|z| = sqrt(re^2 + im^2)`.
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Complex exponential `exp(z) = exp(re) (cos im + i sin im)`.
    pub fn exp(self) -> Self {
        let r = self.re.exp();
        Cplx::new(r * self.im.cos(), r * self.im.sin())
    }

    /// Principal-branch complex logarithm,
    /// `ln(z) = ln|z| + i atan2(im, re)`. Diverges (returns `-inf` real
    /// part) at `z = 0`; callers guard against that.
    pub fn ln(self) -> Self {
        Cplx::new(self.abs().ln(), self.im.atan2(self.re))
    }

    /// Complex square root (principal branch), used by the quadratic
    /// formula. `sqrt(z) = sqrt(|z|) * exp(i * arg(z) / 2)`.
    pub fn sqrt(self) -> Self {
        let r = self.abs().sqrt();
        let half_arg = self.im.atan2(self.re) / 2.0;
        Cplx::new(r * half_arg.cos(), r * half_arg.sin())
    }

    /// True if either component is NaN or infinite.
    pub fn is_non_finite(self) -> bool {
        !(self.re.is_finite() && self.im.is_finite())
    }
}

impl Add for Cplx {
    type Output = Cplx;
    fn add(self, rhs: Cplx) -> Cplx {
        Cplx::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for Cplx {
    type Output = Cplx;
    fn sub(self, rhs: Cplx) -> Cplx {
        Cplx::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for Cplx {
    type Output = Cplx;
    fn mul(self, rhs: Cplx) -> Cplx {
        Cplx::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Div for Cplx {
    type Output = Cplx;
    fn div(self, rhs: Cplx) -> Cplx {
        let denom = rhs.re * rhs.re + rhs.im * rhs.im;
        Cplx::new(
            (self.re * rhs.re + self.im * rhs.im) / denom,
            (self.im * rhs.re - self.re * rhs.im) / denom,
        )
    }
}

impl Neg for Cplx {
    type Output = Cplx;
    fn neg(self) -> Cplx {
        Cplx::new(-self.re, -self.im)
    }
}

impl Mul<f64> for Cplx {
    type Output = Cplx;
    fn mul(self, rhs: f64) -> Cplx {
        Cplx::new(self.re * rhs, self.im * rhs)
    }
}
