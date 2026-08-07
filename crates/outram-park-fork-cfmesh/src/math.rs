// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code. Not derived from any upstream project
// — a three-component f64 vector with the usual operations, written from
// scratch so the volume-meshing core has no linear-algebra dependency. Shaped
// after `outram-blender`'s `math::Vec3` (same workspace, GPL-3.0-only) for
// consistency, not copied from it.
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

//! Minimal fixed-size 3-vector for mesh geometry.
//!
//! A small, dependency-free [`Vec3`] (three `f64`s) — positions, edge vectors,
//! face area vectors, cell centres. Kept local to the crate (like
//! `outram-blender`'s `math::Vec3`) so the volume-meshing core stays pure Rust
//! and Android-buildable with no external linear-algebra dependency. The large
//! solves the mesher never needs; all of its geometry is these fixed-size ops.

/// A 3-component vector `(x, y, z)` of `f64`s. `Copy`, so pass by value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Vec3 {
    /// The zero vector `(0, 0, 0)`.
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    /// Construct from components.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// Component-wise sum `self + other`.
    pub fn add(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// Component-wise difference `self - other`.
    pub fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// Scalar multiple `self * s`.
    pub fn scale(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    /// Dot product `self · other`.
    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product `self × other`.
    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Euclidean length `|self|`.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Unit vector in the same direction, or [`Vec3::ZERO`] for a zero-length
    /// input (never produces `NaN`).
    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len == 0.0 {
            Vec3::ZERO
        } else {
            self.scale(1.0 / len)
        }
    }
}
