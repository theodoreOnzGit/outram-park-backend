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

use std::ops::{Add, Div, Mul, Neg, Sub};

/// Isotropic diagonal tensor: represents `ii * I` where `I` is the 3×3 identity.
/// Maps to `Foam::SphericalTensor<scalar>` (`SphericalTensorI.H`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SphericalTensor {
    pub ii: f64,
}

impl SphericalTensor {
    pub const ZERO: Self = Self { ii: 0.0 };
    pub const IDENTITY: Self = Self { ii: 1.0 };

    #[inline]
    pub fn new(ii: f64) -> Self {
        Self { ii }
    }

    /// Trace = 3 * ii
    #[inline]
    pub fn tr(self) -> f64 {
        3.0 * self.ii
    }

    /// Frobenius norm squared = 3 * ii²
    #[inline]
    pub fn mag_sqr(self) -> f64 {
        3.0 * self.ii * self.ii
    }

    #[inline]
    pub fn mag(self) -> f64 {
        self.mag_sqr().sqrt()
    }

    /// Diagonal norm squared (sum of squared diagonal entries = 3*ii²)
    #[inline]
    pub fn diag_sqr(self) -> f64 {
        3.0 * self.ii * self.ii
    }

    /// Determinant = ii³
    #[inline]
    pub fn det(self) -> f64 {
        self.ii * self.ii * self.ii
    }

    /// Inverse: SphericalTensor(1/ii)
    #[inline]
    pub fn inv(self) -> Self {
        Self { ii: 1.0 / self.ii }
    }

    /// Double inner-product with itself: 3 * ii²
    #[inline]
    pub fn double_inner(self, rhs: Self) -> f64 {
        3.0 * self.ii * rhs.ii
    }

    /// Linear interpolation
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        Self { ii: (1.0 - t) * a.ii + t * b.ii }
    }
}

// --- Arithmetic operators ---

impl Neg for SphericalTensor {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self { Self { ii: -self.ii } }
}

impl Add for SphericalTensor {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self { Self { ii: self.ii + rhs.ii } }
}

impl Sub for SphericalTensor {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self { Self { ii: self.ii - rhs.ii } }
}

impl Mul<f64> for SphericalTensor {
    type Output = Self;
    #[inline]
    fn mul(self, s: f64) -> Self { Self { ii: self.ii * s } }
}

impl Mul<SphericalTensor> for f64 {
    type Output = SphericalTensor;
    #[inline]
    fn mul(self, st: SphericalTensor) -> SphericalTensor { st * self }
}

impl Div<f64> for SphericalTensor {
    type Output = Self;
    #[inline]
    fn div(self, s: f64) -> Self { Self { ii: self.ii / s } }
}

/// `scalar / SphericalTensor` — maps to C++ `operator/(Cmpt, SphericalTensor)`
impl Div<SphericalTensor> for f64 {
    type Output = SphericalTensor;
    #[inline]
    fn div(self, st: SphericalTensor) -> SphericalTensor {
        SphericalTensor { ii: self / st.ii }
    }
}

// --- Free functions mirroring OpenFOAM globals ---

#[inline]
pub fn tr(st: SphericalTensor) -> f64 { st.tr() }

#[inline]
pub fn det(st: SphericalTensor) -> f64 { st.det() }

#[inline]
pub fn inv(st: SphericalTensor) -> SphericalTensor { st.inv() }

#[inline]
pub fn mag_sqr(st: SphericalTensor) -> f64 { st.mag_sqr() }

#[inline]
pub fn lerp(a: SphericalTensor, b: SphericalTensor, t: f64) -> SphericalTensor {
    SphericalTensor::lerp(a, b, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_properties() {
        let i = SphericalTensor::IDENTITY;
        assert_eq!(i.tr(), 3.0);
        assert_eq!(i.det(), 1.0);
        assert_eq!(i.inv().ii, 1.0);
        assert_eq!(i.mag_sqr(), 3.0);
    }

    #[test]
    fn arithmetic() {
        let a = SphericalTensor::new(2.0);
        let b = SphericalTensor::new(3.0);
        assert_eq!((a + b).ii, 5.0);
        assert_eq!((b - a).ii, 1.0);
        assert_eq!((a * 3.0).ii, 6.0);
        assert_eq!((3.0 * a).ii, 6.0);
        assert_eq!((a / 2.0).ii, 1.0);
    }

    #[test]
    fn double_inner() {
        let a = SphericalTensor::new(2.0);
        let b = SphericalTensor::new(3.0);
        assert_eq!(a.double_inner(b), 3.0 * 2.0 * 3.0);
    }
}
