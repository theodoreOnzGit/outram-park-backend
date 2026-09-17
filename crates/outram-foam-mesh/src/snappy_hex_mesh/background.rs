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

//! Background hex mesh for `snappyHexMesh` castellation.
//!
//! `snappyHexMesh` never starts from nothing: it refines and carves an existing
//! all-hex background mesh (in OpenFOAM, the output of `blockMesh`). This module
//! provides the minimal thing castellation needs — an axis-aligned box divided
//! into a uniform `nx × ny × nz` grid of hexahedral cells. Everything downstream
//! (the octree) subdivides these level-0 cells.

use outram_foam_basic_lib::primitives::Vector3;

/// Axis-aligned bounding box `[m]`.
///
/// Stored as the two extreme corners `min` (lowest x, y, z) and `max`
/// (highest). Used both for the background-mesh extent and for individual
/// octree cell boxes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Lower corner (min x, y, z) `[m]`.
    pub min: Vector3,
    /// Upper corner (max x, y, z) `[m]`.
    pub max: Vector3,
}

impl Bounds {
    /// Construct from two corner points; the components are sorted so `min` is
    /// genuinely the lower corner regardless of the argument order.
    pub fn new(a: Vector3, b: Vector3) -> Self {
        Self {
            min: Vector3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            max: Vector3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
        }
    }

    /// Uniformly grow the box by `pad` `[m]` on every side (used to wrap a
    /// surface in a background domain with a margin).
    pub fn expanded(&self, pad: f64) -> Self {
        let p = Vector3::new(pad, pad, pad);
        Self {
            min: self.min - p,
            max: self.max + p,
        }
    }

    /// Box centre `[m]`.
    pub fn centre(&self) -> Vector3 {
        (self.min + self.max) * 0.5
    }

    /// Side lengths `(dx, dy, dz)` `[m]`.
    pub fn span(&self) -> Vector3 {
        self.max - self.min
    }

    /// Volume `dx·dy·dz` `[m³]`.
    pub fn volume(&self) -> f64 {
        let s = self.span();
        s.x * s.y * s.z
    }

    /// Length of the space diagonal `[m]`.
    pub fn diagonal(&self) -> f64 {
        self.span().mag()
    }
}

/// A uniform hexahedral background mesh: a box split into `nx × ny × nz`
/// equal cells.
///
/// This is the level-0 grid that castellation refines. Cell `(i, j, k)` (with
/// `0 ≤ i < nx`, etc.) spans
/// `[min + (i,j,k)·h, min + (i+1,j+1,k+1)·h]` where `h = span / (nx,ny,nz)`.
#[derive(Debug, Clone)]
pub struct BackgroundMesh {
    /// Overall domain extent `[m]`.
    pub bounds: Bounds,
    /// Cell divisions along x, y, z.
    pub nx: usize,
    /// Cell divisions along y.
    pub ny: usize,
    /// Cell divisions along z.
    pub nz: usize,
}

impl BackgroundMesh {
    /// Build a uniform background mesh over `bounds` with `nx × ny × nz` cells.
    ///
    /// # Panics
    /// If any division count is zero.
    pub fn uniform(bounds: Bounds, nx: usize, ny: usize, nz: usize) -> Self {
        assert!(
            nx > 0 && ny > 0 && nz > 0,
            "division counts must be positive"
        );
        Self { bounds, nx, ny, nz }
    }

    /// Total number of level-0 cells (`nx·ny·nz`).
    pub fn n_cells(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Level-0 cell size `(dx, dy, dz)` `[m]`.
    pub fn cell_size(&self) -> Vector3 {
        let s = self.bounds.span();
        Vector3::new(
            s.x / self.nx as f64,
            s.y / self.ny as f64,
            s.z / self.nz as f64,
        )
    }
}
