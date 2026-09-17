//! Virtual lattice — a uniform-grid accelerator for cells packed with many
//! explicit TRISO particles.
//!
//! # Provenance
//!
//! Ported from the OpenMC fork `liangjg/openmc`, branch `virtual_lattice`,
//! commit `be04e2804f9dc563d53429d97368c5d905070978` (2025-10-27), vendored
//! read-only at `upstream_source/OpenMC-virtual-lattice/`. The feature is not
//! in upstream `openmc-dev/openmc`; the branch is 14 commits ahead of
//! `develop`, contributed by Liang Jingang, Li Ruihan, and `cn-skywalker`.
//!
//! Reference sites, all in the vendored tree:
//!   - `src/cell.cpp:39`  `generate_triso_distribution` -> [`VirtualLattice::build`]
//!   - `src/cell.cpp:665` `CSGCell::distance_in_virtual_lattice` ->
//!     [`VirtualLattice::distance`] (in [`traversal`])
//!   - `src/universe.cpp:68` `Universe::find_cell_in_virtual_lattice` ->
//!     [`VirtualLattice::find_containing`]
//!   - `src/surface.cpp:724` `SurfaceSphere::triso_in_mesh` ->
//!     [`crate::geometry::surface::SurfaceKind::overlaps_voxel`]
//!
//! ```text
//! Copyright (c) 2011-2025 Massachusetts Institute of Technology, UChicago
//! Argonne LLC, and OpenMC contributors
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to
//! deal in the Software without restriction, including without limitation the
//! rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
//! sell copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in
//! all copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
//! IN THE SOFTWARE.
//! ```
//!
//! OpenMC is MIT-licensed; this Rust translation is GPL-3.0-only per the
//! workspace default. This is an independent translation, not an OpenMC
//! release, and is not endorsed by or affiliated with the OpenMC project.
//!
//! # What problem this solves
//!
//! A TRISO fuel compact is one CSG cell whose region names tens of thousands
//! of explicit spheres. Evaluating `distance_to_boundary` for such a cell costs
//! O(N) surface-distance evaluations per flight — the dominant cost in a
//! doubly-heterogeneous calculation.
//!
//! The virtual lattice overlays a uniform Cartesian grid ("voxels") on the
//! cell and records, per voxel, only the spheres that overlap it. A ray then
//! walks the grid voxel by voxel (a 3-D DDA) and tests only the handful of
//! spheres registered in each voxel it enters, stopping as soon as the nearest
//! hit is closer than the exit point of the current voxel. Cost drops from
//! O(N) to roughly O(spheres per voxel x voxels crossed).
//!
//! This is the *explicit-geometry* alternative to Woodcock/delta tracking
//! ([`crate::pebble_beds::delta_tracking`]), which avoids surface tests
//! entirely at the price of rejected collisions. The two are complementary:
//! delta tracking wins when the majorant is tight, the virtual lattice wins
//! when exact surface crossings are needed (e.g. per-layer TRISO tallies).
//!
//! # Units and conventions
//!
//! Lengths are in **cm** (the crate-wide convention, see the crate `CLAUDE.md`);
//! `f64` throughout, no `uom` — this is inner-loop transport code.
//!
//! Voxel `(i, j, k)` spans
//! `[lower_left[d] + i*pitch[d], lower_left[d] + (i+1)*pitch[d]]` per axis `d`,
//! and is stored at flat index `i + j*shape[0] + k*shape[0]*shape[1]`
//! (x fastest — matching the upstream indexing exactly).
//!
//! # Assumptions and limitations
//!
//! - **Spheres only.** Bucket membership is decided by
//!   [`SurfaceKind::overlaps_voxel`], which upstream implements for
//!   `Sphere` alone; every other surface type returns `false` there. See that
//!   method's docs. A non-sphere surface handed to [`VirtualLattice::build`]
//!   is therefore silently registered in no voxel, and a ray will never see it.
//! - **The grid must enclose every sphere.** Spheres whose centre falls outside
//!   the `shape` bounds are clipped by the neighbourhood scan and may be
//!   partially or wholly unregistered. [`VirtualLattice::build`] reports these
//!   through [`BuildReport::unregistered`] rather than failing, mirroring
//!   upstream's silent behaviour but making it observable.
//! - **A sphere is registered in every voxel it overlaps**, found by scanning
//!   the 3x3x3 neighbourhood of the voxel containing its centre. A sphere with
//!   radius larger than the pitch can therefore reach voxels the scan misses;
//!   [`VirtualLattice::build`] flags that case in
//!   [`BuildReport::radius_exceeds_pitch`]. Upstream has the same limitation
//!   and does not check for it.
//!
//! # Verification status
//!
//! **Unverified.** Unit tests in `tests.rs` check the bucket build, the
//! traversal, and equivalence with a brute-force scan over the same surfaces.
//! No k-eigenvalue validation against the upstream `triso_virtual_lattice`
//! regression case has been run — that is tracked as a separate bead.

use super::position::Position;
use super::surface::SurfaceKind;

pub mod traversal;

#[cfg(test)]
mod tests;

/// Diagnostics from [`VirtualLattice::build`].
///
/// The upstream builder silently drops geometry it cannot place. This port
/// keeps the same behaviour (so results match) but reports what happened, so a
/// caller can assert the grid actually covers the packing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildReport {
    /// Surface indices that landed in no voxel at all — either not a sphere
    /// (see [`SurfaceKind::overlaps_voxel`]) or entirely outside the grid.
    pub unregistered: Vec<usize>,
    /// Surface indices whose radius exceeds half the smallest pitch, so the
    /// 3x3x3 neighbourhood scan may not have found every voxel they overlap.
    pub radius_exceeds_pitch: Vec<usize>,
    /// Total (voxel, surface) registrations made — the memory cost of the grid.
    pub registrations: usize,
}

/// A uniform Cartesian grid recording which surfaces overlap each voxel.
///
/// Build it once per cell with [`VirtualLattice::build`], then use
/// [`VirtualLattice::distance`] for ray traversal and
/// [`VirtualLattice::find_containing`] for point location.
///
/// Maps to the `vl_*` fields of `openmc::CSGCell`
/// (`include/openmc/cell.h`, vendored fork).
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualLattice {
    /// Lower-left corner of the grid, cm. Maps to `CSGCell::vl_lower_left_`.
    pub lower_left: [f64; 3],
    /// Voxel edge lengths, cm. Maps to `CSGCell::vl_pitch_`. All strictly > 0.
    pub pitch: [f64; 3],
    /// Voxel counts per axis. Maps to `CSGCell::vl_shape_`. All >= 1.
    pub shape: [usize; 3],
    /// Per-voxel surface indices, flat-indexed x-fastest.
    /// Maps to `CSGCell::vl_triso_distribution_`.
    buckets: Vec<Vec<usize>>,
}

impl VirtualLattice {
    /// Build the grid, registering each surface in every voxel it overlaps.
    ///
    /// Mirrors `generate_triso_distribution` (`src/cell.cpp:39`): for each
    /// surface, locate the voxel containing its centre, then test the 3x3x3
    /// neighbourhood of that voxel for overlap.
    ///
    /// `surface_indices` are indices into `surfaces`; only those are considered
    /// (a cell's region typically names a subset of the global surface array).
    ///
    /// # Panics
    ///
    /// Panics if any `pitch` component is not strictly positive, any `shape`
    /// component is zero, or a `surface_indices` entry is out of bounds — all
    /// programming errors rather than recoverable input conditions.
    pub fn build(
        lower_left: [f64; 3],
        pitch: [f64; 3],
        shape: [usize; 3],
        surface_indices: &[usize],
        surfaces: &[SurfaceKind],
    ) -> (Self, BuildReport) {
        assert!(
            pitch.iter().all(|p| *p > 0.0),
            "virtual lattice pitch must be strictly positive, got {pitch:?}"
        );
        assert!(
            shape.iter().all(|n| *n > 0),
            "virtual lattice shape must be at least 1 per axis, got {shape:?}"
        );

        let n_voxels = shape[0] * shape[1] * shape[2];
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); n_voxels];
        let mut report = BuildReport::default();

        for &si in surface_indices {
            assert!(
                si < surfaces.len(),
                "surface index {si} out of bounds ({} surfaces)",
                surfaces.len()
            );
            let Some((centre, radius)) = surfaces[si].sphere_centre_radius() else {
                // Not a sphere: upstream's triso_in_mesh returns false for
                // every other surface type, so it can never be registered.
                report.unregistered.push(si);
                continue;
            };

            if radius > 0.5 * pitch.iter().cloned().fold(f64::INFINITY, f64::min) {
                report.radius_exceeds_pitch.push(si);
            }

            // Voxel containing the sphere centre (may be outside the grid).
            let mut centre_ijk = [0i64; 3];
            for d in 0..3 {
                centre_ijk[d] = ((centre[d] - lower_left[d]) / pitch[d]).floor() as i64;
            }

            let mut placed = false;
            for i in (centre_ijk[0] - 1)..=(centre_ijk[0] + 1) {
                for j in (centre_ijk[1] - 1)..=(centre_ijk[1] + 1) {
                    for k in (centre_ijk[2] - 1)..=(centre_ijk[2] + 1) {
                        if i < 0
                            || j < 0
                            || k < 0
                            || i >= shape[0] as i64
                            || j >= shape[1] as i64
                            || k >= shape[2] as i64
                        {
                            continue;
                        }
                        let voxel_centre = [
                            (i as f64 + 0.5) * pitch[0] + lower_left[0],
                            (j as f64 + 0.5) * pitch[1] + lower_left[1],
                            (k as f64 + 0.5) * pitch[2] + lower_left[2],
                        ];
                        if surfaces[si].overlaps_voxel(voxel_centre, pitch) {
                            let flat = i as usize
                                + j as usize * shape[0]
                                + k as usize * shape[0] * shape[1];
                            buckets[flat].push(si);
                            report.registrations += 1;
                            placed = true;
                        }
                    }
                }
            }
            let _ = radius;
            if !placed {
                report.unregistered.push(si);
            }
        }

        (
            Self {
                lower_left,
                pitch,
                shape,
                buckets,
            },
            report,
        )
    }

    /// Flat index of voxel `(i, j, k)`, x-fastest. Matches upstream's
    /// `i + j*shape[0] + k*shape[0]*shape[1]`.
    #[inline]
    pub fn flat_index(&self, ijk: [usize; 3]) -> usize {
        ijk[0] + ijk[1] * self.shape[0] + ijk[2] * self.shape[0] * self.shape[1]
    }

    /// Total number of voxels.
    #[inline]
    pub fn n_voxels(&self) -> usize {
        self.shape[0] * self.shape[1] * self.shape[2]
    }

    /// Surface indices registered in voxel `(i, j, k)`.
    /// Empty for a voxel no sphere reaches.
    #[inline]
    pub fn surfaces_in_voxel(&self, ijk: [usize; 3]) -> &[usize] {
        &self.buckets[self.flat_index(ijk)]
    }

    /// Voxel indices containing `r`, unclamped — components may be negative or
    /// >= `shape` when `r` lies outside the grid.
    #[inline]
    pub fn indices_at(&self, r: Position) -> [i64; 3] {
        let p = [r.x, r.y, r.z];
        let mut ijk = [0i64; 3];
        for d in 0..3 {
            ijk[d] = ((p[d] - self.lower_left[d]) / self.pitch[d]).floor() as i64;
        }
        ijk
    }

    /// Voxel indices containing `r`, clamped into the grid.
    ///
    /// Mirrors the `max(min(floor(...), shape-1), 0)` clamp at the top of
    /// `Universe::find_cell_in_virtual_lattice` (`src/universe.cpp:68`).
    #[inline]
    pub fn clamped_indices_at(&self, r: Position) -> [usize; 3] {
        let raw = self.indices_at(r);
        let mut ijk = [0usize; 3];
        for d in 0..3 {
            ijk[d] = raw[d].clamp(0, self.shape[d] as i64 - 1) as usize;
        }
        ijk
    }

    /// True when `ijk` addresses a voxel inside the grid.
    #[inline]
    pub fn contains_indices(&self, ijk: [i64; 3]) -> bool {
        (0..3).all(|d| ijk[d] >= 0 && ijk[d] < self.shape[d] as i64)
    }

    /// Index of the sphere strictly containing `r`, searching only the voxel
    /// `r` falls in.
    ///
    /// Mirrors the containment half of `Universe::find_cell_in_virtual_lattice`
    /// (`src/universe.cpp:68`): the point-location counterpart of
    /// [`VirtualLattice::distance`]. Returns `None` when `r` is in the matrix
    /// between particles — upstream's "fall through to the base cell" case.
    ///
    /// Containment is strict (`d^2 < r^2`), matching upstream. A point exactly
    /// on a sphere surface is therefore *not* inside it; surface-crossing
    /// bookkeeping is the caller's job, as it is upstream.
    pub fn find_containing(&self, r: Position, surfaces: &[SurfaceKind]) -> Option<usize> {
        let ijk = self.clamped_indices_at(r);
        for &si in self.surfaces_in_voxel(ijk) {
            let Some((centre, radius)) = surfaces[si].sphere_centre_radius() else {
                continue;
            };
            let dx = r.x - centre[0];
            let dy = r.y - centre[1];
            let dz = r.z - centre[2];
            if dx * dx + dy * dy + dz * dz < radius * radius {
                return Some(si);
            }
        }
        None
    }
}
