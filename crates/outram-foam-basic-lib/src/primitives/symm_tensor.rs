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

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

use super::spherical_tensor::SphericalTensor;
use super::vector::Vector3;

/// Symmetric 3×3 tensor stored in upper-triangle order: xx, xy, xz, yy, yz, zz.
/// Maps to `Foam::symmTensor` (`Foam::SymmTensor<scalar>`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SymmTensor {
    pub xx: f64,
    pub xy: f64,
    pub xz: f64,
    pub yy: f64,
    pub yz: f64,
    pub zz: f64,
}

impl SymmTensor {
    pub const ZERO: Self = Self { xx: 0.0, xy: 0.0, xz: 0.0, yy: 0.0, yz: 0.0, zz: 0.0 };
    pub const IDENTITY: Self = Self { xx: 1.0, xy: 0.0, xz: 0.0, yy: 1.0, yz: 0.0, zz: 1.0 };

    #[inline]
    pub fn new(xx: f64, xy: f64, xz: f64, yy: f64, yz: f64, zz: f64) -> Self {
        Self { xx, xy, xz, yy, yz, zz }
    }

    /// Construct from diagonal only (off-diagonal = 0).
    #[inline]
    pub fn from_diag(xx: f64, yy: f64, zz: f64) -> Self {
        Self { xx, xy: 0.0, xz: 0.0, yy, yz: 0.0, zz }
    }

    /// Row vectors (yx = xy, zx = xz, zy = yz because symmetric)
    #[inline]
    pub fn row_x(self) -> Vector3 { Vector3::new(self.xx, self.xy, self.xz) }
    #[inline]
    pub fn row_y(self) -> Vector3 { Vector3::new(self.xy, self.yy, self.yz) }
    #[inline]
    pub fn row_z(self) -> Vector3 { Vector3::new(self.xz, self.yz, self.zz) }

    /// Diagonal as a vector
    #[inline]
    pub fn diag(self) -> Vector3 { Vector3::new(self.xx, self.yy, self.zz) }

    /// Trace: xx + yy + zz
    #[inline]
    pub fn tr(self) -> f64 { self.xx + self.yy + self.zz }

    /// Spherical (isotropic) part: (tr/3) * I
    #[inline]
    pub fn sph(self) -> SphericalTensor { SphericalTensor::new(self.tr() / 3.0) }

    /// Deviatoric part: self - (tr/3)*I
    #[inline]
    pub fn dev(self) -> Self { self - self.sph() }

    /// Two-thirds deviatoric part: self - (2*tr/3)*I
    #[inline]
    pub fn dev2(self) -> Self { self - 2.0 * self.sph() }

    /// Determinant
    #[inline]
    pub fn det(self) -> f64 {
        let Self { xx, xy, xz, yy, yz, zz } = self;
        xx * yy * zz + xy * yz * xz + xz * xy * yz
            - xx * yz * yz - xy * xy * zz - xz * yy * xz
    }

    /// Adjunct (= cofactor matrix, same as adjunct because symmetric)
    #[inline]
    pub fn adjunct(self) -> Self {
        let Self { xx, xy, xz, yy, yz, zz } = self;
        Self {
            xx: yy * zz - yz * yz,
            xy: xz * yz - xy * zz,
            xz: xy * yz - xz * yy,
            yy: xx * zz - xz * xz,
            yz: xy * xz - xx * yz,
            zz: xx * yy - xy * xy,
        }
    }

    /// Inverse = adjunct / det. Panics if singular in debug builds.
    #[inline]
    pub fn inv(self) -> Self {
        let d = self.det();
        debug_assert!(d.abs() > 0.0, "SymmTensor is singular");
        self.adjunct() / d
    }

    /// Inverse with fallback: returns ZERO if nearly singular.
    pub fn safe_inv(self) -> Self {
        use super::scalar::SMALL;
        let diag_sqr = self.xx * self.xx + self.yy * self.yy + self.zz * self.zz;
        let threshold = SMALL * diag_sqr;
        let mut work = self;
        let small_xx = self.xx * self.xx < threshold;
        let small_yy = self.yy * self.yy < threshold;
        let small_zz = self.zz * self.zz < threshold;
        if small_xx { work.xx += 1.0; }
        if small_yy { work.yy += 1.0; }
        if small_zz { work.zz += 1.0; }
        let d = work.det();
        if d.abs() < super::scalar::ROOT_VSMALL { return Self::ZERO; }
        let mut result = work.adjunct() / d;
        if small_xx { result.xx -= 1.0; }
        if small_yy { result.yy -= 1.0; }
        if small_zz { result.zz -= 1.0; }
        result
    }

    /// Frobenius norm squared (off-diagonal counted twice, matching OpenFOAM)
    #[inline]
    pub fn mag_sqr(self) -> f64 {
        let Self { xx, xy, xz, yy, yz, zz } = self;
        xx * xx + 2.0 * xy * xy + 2.0 * xz * xz + yy * yy + 2.0 * yz * yz + zz * zz
    }

    #[inline]
    pub fn mag(self) -> f64 { self.mag_sqr().sqrt() }

    /// Sum of squared diagonal entries (not Frobenius)
    #[inline]
    pub fn diag_sqr(self) -> f64 { self.xx * self.xx + self.yy * self.yy + self.zz * self.zz }

    /// Self² as a SymmTensor (S·S where both factors are symmetric)
    #[inline]
    pub fn inner_sqr(self) -> Self {
        let Self { xx, xy, xz, yy, yz, zz } = self;
        Self {
            xx: xx * xx + xy * xy + xz * xz,
            xy: xx * xy + xy * yy + xz * yz,
            xz: xx * xz + xy * yz + xz * zz,
            yy: xy * xy + yy * yy + yz * yz,
            yz: xy * xz + yy * yz + yz * zz,
            zz: xz * xz + yz * yz + zz * zz,
        }
    }

    /// Double contraction (Frobenius inner product). C++ `operator&&`.
    /// Note: off-diagonal elements contribute twice because the tensor is symmetric.
    #[inline]
    pub fn double_inner(self, rhs: Self) -> f64 {
        self.xx * rhs.xx
            + 2.0 * self.xy * rhs.xy
            + 2.0 * self.xz * rhs.xz
            + self.yy * rhs.yy
            + 2.0 * self.yz * rhs.yz
            + self.zz * rhs.zz
    }

    /// Matrix multiply SymmTensor·Vector → Vector. C++ `operator&(SymmTensor, Vector)`.
    #[inline]
    pub fn mat_vec(self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.xx * v.x + self.xy * v.y + self.xz * v.z,
            self.xy * v.x + self.yy * v.y + self.yz * v.z,
            self.xz * v.x + self.yz * v.y + self.zz * v.z,
        )
    }

    /// Hodge dual: returns the axial vector. C++ `operator*(SymmTensor)`.
    #[inline]
    pub fn hodge_dual(self) -> Vector3 {
        Vector3::new(self.yz, -self.xz, self.xy)
    }

    /// Outer (dyadic) product of a vector with itself: v ⊗ v → SymmTensor.
    /// C++ `sqr(Vector)`.
    #[inline]
    pub fn from_outer(v: Vector3) -> Self {
        Self {
            xx: v.x * v.x, xy: v.x * v.y, xz: v.x * v.z,
            yy: v.y * v.y, yz: v.y * v.z,
            zz: v.z * v.z,
        }
    }

    /// First invariant: trace
    #[inline]
    pub fn invariant_i(self) -> f64 { self.tr() }

    /// Second invariant: (xx*yy + yy*zz + xx*zz) - (xy² + yz² + xz²)
    #[inline]
    pub fn invariant_ii(self) -> f64 {
        self.xx * self.yy + self.yy * self.zz + self.xx * self.zz
            - self.xy * self.xy - self.yz * self.yz - self.xz * self.xz
    }

    /// Third invariant: determinant
    #[inline]
    pub fn invariant_iii(self) -> f64 { self.det() }

    /// Linear interpolation
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        let ot = 1.0 - t;
        Self {
            xx: ot * a.xx + t * b.xx, xy: ot * a.xy + t * b.xy,
            xz: ot * a.xz + t * b.xz, yy: ot * a.yy + t * b.yy,
            yz: ot * a.yz + t * b.yz, zz: ot * a.zz + t * b.zz,
        }
    }

    /// True if the tensor is (approximately) the identity.
    pub fn is_identity(self, tol: f64) -> bool {
        (self.xx - 1.0).abs() < tol && (self.yy - 1.0).abs() < tol
            && (self.zz - 1.0).abs() < tol
            && self.xy.abs() < tol && self.xz.abs() < tol && self.yz.abs() < tol
    }
}

// --- Conversions ---

impl From<SphericalTensor> for SymmTensor {
    #[inline]
    fn from(st: SphericalTensor) -> Self {
        Self { xx: st.ii, xy: 0.0, xz: 0.0, yy: st.ii, yz: 0.0, zz: st.ii }
    }
}

// --- Arithmetic operators ---

impl Neg for SymmTensor {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self { xx: -self.xx, xy: -self.xy, xz: -self.xz, yy: -self.yy, yz: -self.yz, zz: -self.zz }
    }
}

impl Add for SymmTensor {
    type Output = Self;
    #[inline]
    fn add(self, r: Self) -> Self {
        Self {
            xx: self.xx + r.xx, xy: self.xy + r.xy, xz: self.xz + r.xz,
            yy: self.yy + r.yy, yz: self.yz + r.yz, zz: self.zz + r.zz,
        }
    }
}

impl Sub for SymmTensor {
    type Output = Self;
    #[inline]
    fn sub(self, r: Self) -> Self {
        Self {
            xx: self.xx - r.xx, xy: self.xy - r.xy, xz: self.xz - r.xz,
            yy: self.yy - r.yy, yz: self.yz - r.yz, zz: self.zz - r.zz,
        }
    }
}

impl Mul<f64> for SymmTensor {
    type Output = Self;
    #[inline]
    fn mul(self, s: f64) -> Self {
        Self {
            xx: self.xx * s, xy: self.xy * s, xz: self.xz * s,
            yy: self.yy * s, yz: self.yz * s, zz: self.zz * s,
        }
    }
}

impl Mul<SymmTensor> for f64 {
    type Output = SymmTensor;
    #[inline]
    fn mul(self, st: SymmTensor) -> SymmTensor { st * self }
}

impl Div<f64> for SymmTensor {
    type Output = Self;
    #[inline]
    fn div(self, s: f64) -> Self {
        Self {
            xx: self.xx / s, xy: self.xy / s, xz: self.xz / s,
            yy: self.yy / s, yz: self.yz / s, zz: self.zz / s,
        }
    }
}

impl AddAssign for SymmTensor {
    #[inline]
    fn add_assign(&mut self, r: Self) { *self = *self + r; }
}

impl SubAssign for SymmTensor {
    #[inline]
    fn sub_assign(&mut self, r: Self) { *self = *self - r; }
}

impl MulAssign<f64> for SymmTensor {
    #[inline]
    fn mul_assign(&mut self, s: f64) { *self = *self * s; }
}

// SphericalTensor ± SymmTensor → SymmTensor

impl Add<SymmTensor> for SphericalTensor {
    type Output = SymmTensor;
    #[inline]
    fn add(self, st: SymmTensor) -> SymmTensor {
        SymmTensor {
            xx: self.ii + st.xx, xy: st.xy, xz: st.xz,
            yy: self.ii + st.yy, yz: st.yz,
            zz: self.ii + st.zz,
        }
    }
}

impl Add<SphericalTensor> for SymmTensor {
    type Output = Self;
    #[inline]
    fn add(self, spt: SphericalTensor) -> Self { spt + self }
}

impl Sub<SymmTensor> for SphericalTensor {
    type Output = SymmTensor;
    #[inline]
    fn sub(self, st: SymmTensor) -> SymmTensor {
        SymmTensor {
            xx: self.ii - st.xx, xy: -st.xy, xz: -st.xz,
            yy: self.ii - st.yy, yz: -st.yz,
            zz: self.ii - st.zz,
        }
    }
}

impl Sub<SphericalTensor> for SymmTensor {
    type Output = Self;
    #[inline]
    fn sub(self, spt: SphericalTensor) -> Self {
        Self {
            xx: self.xx - spt.ii, xy: self.xy, xz: self.xz,
            yy: self.yy - spt.ii, yz: self.yz,
            zz: self.zz - spt.ii,
        }
    }
}

// SphericalTensor & SymmTensor (inner product) → SymmTensor (scalar times self)
impl Mul<SymmTensor> for SphericalTensor {
    type Output = SymmTensor;
    #[inline]
    fn mul(self, st: SymmTensor) -> SymmTensor { st * self.ii }
}

impl Mul<SphericalTensor> for SymmTensor {
    type Output = Self;
    #[inline]
    fn mul(self, spt: SphericalTensor) -> Self { self * spt.ii }
}

// --- Free functions ---

#[inline]
pub fn tr(st: SymmTensor) -> f64 { st.tr() }

#[inline]
pub fn det(st: SymmTensor) -> f64 { st.det() }

#[inline]
pub fn inv(st: SymmTensor) -> SymmTensor { st.inv() }

#[inline]
pub fn dev(st: SymmTensor) -> SymmTensor { st.dev() }

#[inline]
pub fn dev2(st: SymmTensor) -> SymmTensor { st.dev2() }

/// Symmetric part of a SymmTensor is itself.
#[inline]
pub fn symm(st: SymmTensor) -> SymmTensor { st }

/// Twice the symmetric part of a SymmTensor.
#[inline]
pub fn two_symm(st: SymmTensor) -> SymmTensor { st * 2.0 }

/// dev(symm(st)) — deviatoric of symmetric part (same as dev for SymmTensor).
#[inline]
pub fn dev_symm(st: SymmTensor) -> SymmTensor { st.dev() }

/// dev(2*symm(st))
#[inline]
pub fn dev_two_symm(st: SymmTensor) -> SymmTensor { st.dev2() }

/// Outer (dyadic) product v ⊗ v as a SymmTensor. C++ `sqr(Vector)`.
#[inline]
pub fn sqr(v: Vector3) -> SymmTensor { SymmTensor::from_outer(v) }

#[inline]
pub fn mag_sqr(st: SymmTensor) -> f64 { st.mag_sqr() }

#[inline]
pub fn lerp(a: SymmTensor, b: SymmTensor, t: f64) -> SymmTensor { SymmTensor::lerp(a, b, t) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_properties() {
        let i = SymmTensor::IDENTITY;
        assert_eq!(i.tr(), 3.0);
        assert_eq!(i.det(), 1.0);
        let inv = i.inv();
        assert!((inv.xx - 1.0).abs() < 1e-14);
        assert!((inv.yy - 1.0).abs() < 1e-14);
        assert!((inv.zz - 1.0).abs() < 1e-14);
    }

    #[test]
    fn dev_removes_spherical() {
        let st = SymmTensor::new(4.0, 0.0, 0.0, 2.0, 0.0, 3.0);
        let d = st.dev();
        assert!((d.tr()).abs() < 1e-14);
    }

    #[test]
    fn from_outer_is_symmetric() {
        let v = Vector3::new(1.0, 2.0, 3.0);
        let s = SymmTensor::from_outer(v);
        assert_eq!(s.xx, 1.0);
        assert_eq!(s.xy, 2.0);
        assert_eq!(s.xz, 3.0);
        assert_eq!(s.yy, 4.0);
        assert_eq!(s.yz, 6.0);
        assert_eq!(s.zz, 9.0);
    }

    #[test]
    fn mat_vec() {
        let i = SymmTensor::IDENTITY;
        let v = Vector3::new(1.0, 2.0, 3.0);
        assert_eq!(i.mat_vec(v), v);
    }

    #[test]
    fn dev2_regression() {
        // T = diag(6, 3, 3), tr = 12 → (2/3)*tr = 8
        // dev2 = T − 8·I = diag(6-8, 3-8, 3-8) = diag(-2, -5, -5)
        let t = SymmTensor::from_diag(6.0, 3.0, 3.0);
        let d = t.dev2();
        assert!((d.xx - (-2.0)).abs() < 1e-14, "xx={}", d.xx);
        assert!((d.yy - (-5.0)).abs() < 1e-14, "yy={}", d.yy);
        assert!((d.zz - (-5.0)).abs() < 1e-14, "zz={}", d.zz);
        assert!(d.xy.abs() < 1e-14);
        assert!(d.xz.abs() < 1e-14);
        assert!(d.yz.abs() < 1e-14);
        // Confirm dev2 is NOT trace-free (tr(dev2) = tr(T) - 2·tr(T) = -tr)
        assert!((d.tr() - (-12.0)).abs() < 1e-14, "tr(dev2)={}", d.tr());
    }

    #[test]
    fn inv_roundtrip_is_identity() {
        // T · T⁻¹ == I for a well-conditioned symmetric tensor
        use crate::primitives::Tensor;
        let t = SymmTensor::new(4.0, 2.0, 1.0, 3.0, 0.5, 2.0);
        let product = Tensor::from(t).mat_mul(Tensor::from(t.inv()));
        assert!(product.is_identity(1e-12), "T·T⁻¹ is not identity");
    }
}
