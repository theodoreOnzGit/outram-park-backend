// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Minimal 3-component vector algebra (dot / cross / length / normalize). No
// published algorithm and no upstream source — written from first principles to
// keep the crate dependency-free and Android-buildable.
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

//! Minimal pure-Rust vector math for mesh authoring.
//!
//! This module deliberately reimplements only the tiny slice of vector algebra
//! the mesh layer needs, rather than pulling in a linear-algebra crate. Blender
//! uses its own `blenlib` `BLI_math_vector` routines for the same reason: mesh
//! topology work needs 3-component positions, dot/cross products, and lengths —
//! nothing that justifies a heavy dependency. Keeping it dependency-free also
//! keeps the crate trivially Android-buildable (no BLAS, no C toolchain).
//!
//! If richer linear algebra is ever needed, this is the single place to swap in
//! a pure-Rust crate such as `glam` (add it to the workspace `[dependencies]`
//! first). Positions are plain `f64` in model space; **no physical units are
//! attached here** — a mesh is dimensionless geometry until an [`crate::export`]
//! bridge assigns it a length unit for a solver.

/// A 3-component vector in model space, stored as `f64` components.
///
/// Used both for vertex positions and for direction vectors (normals, offsets).
/// Components are dimensionless model-space coordinates; the consuming solver
/// assigns a length unit at export time. `Vec3` is `Copy`, so it is passed by
/// value throughout the mesh layer (no borrows, per the workspace no-lifetimes
/// rule).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// X component (model-space coordinate or direction).
    pub x: f64,
    /// Y component (model-space coordinate or direction).
    pub y: f64,
    /// Z component (model-space coordinate or direction).
    pub z: f64,
}

impl Vec3 {
    /// The zero vector `(0, 0, 0)` — also the model-space origin.
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Construct a vector from explicit components.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// Component-wise sum, `self + other`.
    pub fn add(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// Component-wise difference, `self - other`.
    pub fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// Scale every component by `s`.
    pub fn scale(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    /// Euclidean dot product `self · other`.
    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Right-handed cross product `self × other`.
    ///
    /// For a face wound counter-clockwise when viewed from outside, the cross
    /// product of two consecutive edge vectors points along the outward normal.
    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Euclidean length (L2 norm), `sqrt(self · self)`.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Unit vector in the same direction.
    ///
    /// Returns [`Vec3::ZERO`] for a zero-length input rather than producing
    /// `NaN`, so callers building topology never propagate a non-finite
    /// coordinate.
    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len == 0.0 {
            Vec3::ZERO
        } else {
            self.scale(1.0 / len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Vec3;

    #[test]
    fn dot_and_cross_are_orthonormal_basis() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = Vec3::new(0.0, 0.0, 1.0);
        assert_eq!(x.dot(y), 0.0);
        assert_eq!(x.cross(y), z);
        assert_eq!(x.dot(x), 1.0);
    }

    #[test]
    fn normalize_zero_is_zero_not_nan() {
        let n = Vec3::ZERO.normalize();
        assert!(n.x.is_finite() && n.y.is_finite() && n.z.is_finite());
        assert_eq!(n, Vec3::ZERO);
    }

    #[test]
    fn length_of_345() {
        assert_eq!(Vec3::new(3.0, 4.0, 0.0).length(), 5.0);
    }
}
