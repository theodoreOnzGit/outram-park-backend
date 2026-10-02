// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/mesh.cpp, include/openmc/mesh.h
// Structured mesh DESCRIPTION: bounds, edges, bin counts, volumes.
// Moved here unchanged in substance from `outram-mc-libs`
// (`src/geometry/`) on 2026-10-02, GitHub issue #486: outram-blender owns the
// CSG description and its pure navigation kernel, outram-mc-libs keeps the
// transport-state work and re-exports these items under its old paths.
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

//! **Spatial mesh description** — the structured meshes a Monte Carlo tally
//! (and, later, the deterministic MGXS path) bins space on.
//!
//! Moved here from `outram-mc-libs` (`src/tally/mesh.rs`) on 2026-10-02,
//! GitHub issue #486 (maintainer scope addition): the **description** —
//! bounds, edges, dimensions, bin counts, per-bin volumes and the flat-index
//! convention ([`unflatten`]) — lives here; **bin lookup and scoring** (which
//! bin a point falls in, fission-site counting, Shannon entropy, mesh-surface
//! crossings) stay in outram-mc-libs on `*Ext` traits, and outram-mc-libs
//! re-exports everything here as `outram_mc_libs::tally::mesh::*`.
//!
//! Units: cm and radians, raw `f64`, as in OpenMC.
//!
//! ~~The unstructured mesh family is not here yet; it is GitHub #492, which
//! will add it as a [`MeshKind`] variant.~~ **CORRECTED 2026-10-03 (GitHub
//! #492):** it is [`MeshKind::Unstructured`], holding an
//! `Arc<`[`UnstructuredMesh`]`>` from [`crate::unstructured`] — the same
//! object an FV or FE solver is built from. Point location and the
//! track-length estimator across its cells live in outram-mc-libs
//! (`UnstructuredMeshExt`).

use std::sync::Arc;

use crate::csg::position::Position;
pub use crate::unstructured::UnstructuredMesh;

/// Axis-aligned regular (equal-spacing) Cartesian mesh.
///
/// Maps to `openmc::RegularMesh` (`src/mesh.cpp`). The mesh spans the box
/// `[lower_left, upper_right]` \[cm\] and is divided into `dimension[i]` equal
/// cells along each axis `i ∈ {x, y, z}`. A point is binned into the grid cell
/// containing it; points outside the box are unbinned (`None`).
///
/// # Fields (all lengths in cm)
/// - `lower_left` — the low corner `[x0, y0, z0]` of the meshed box.
/// - `upper_right` — the high corner `[x1, y1, z1]`; each `upper_right[i]` must
///   exceed `lower_left[i]`.
/// - `dimension` — number of cells `[nx, ny, nz]` along each axis (each ≥ 1). A
///   flat 2-D mesh sets one dimension to 1 (e.g. `[4, 4, 1]`).
#[derive(Debug, Clone, PartialEq)]
pub struct RegularMesh {
    /// Low corner `[x0, y0, z0]` of the meshed box \[cm\].
    pub lower_left: [f64; 3],
    /// High corner `[x1, y1, z1]` of the meshed box \[cm\].
    pub upper_right: [f64; 3],
    /// Number of equal cells `[nx, ny, nz]` along each axis.
    pub dimension: [usize; 3],
}

impl RegularMesh {
    /// Cell width `[wx, wy, wz]` \[cm\] along each axis
    /// (`(upper_right - lower_left) / dimension`).
    #[inline]
    pub fn width(&self) -> [f64; 3] {
        [
            (self.upper_right[0] - self.lower_left[0]) / self.dimension[0].max(1) as f64,
            (self.upper_right[1] - self.lower_left[1]) / self.dimension[1].max(1) as f64,
            (self.upper_right[2] - self.lower_left[2]) / self.dimension[2].max(1) as f64,
        ]
    }

    /// Total number of mesh cells = `nx · ny · nz`.
    ///
    /// Maps to `StructuredMesh::n_bins` (`src/mesh.cpp:1081`).
    #[inline]
    pub fn n_bins(&self) -> usize {
        self.dimension[0] * self.dimension[1] * self.dimension[2]
    }
}

/// **Rectilinear** mesh: explicit, non-uniform bin edges on each axis.
///
/// `openmc::RectilinearMesh`. This is the cheap one, and it is what a radial
/// power profile with finer edge binning actually needs.
#[derive(Debug, Clone, PartialEq)]
pub struct RectilinearMesh {
    /// Ascending bin edges along x, y, z. Each needs at least two entries.
    pub grid: [Vec<f64>; 3],
}

impl RectilinearMesh {
    /// Number of bins along each axis.
    #[inline]
    pub fn dimension(&self) -> [usize; 3] {
        [
            self.grid[0].len().saturating_sub(1),
            self.grid[1].len().saturating_sub(1),
            self.grid[2].len().saturating_sub(1),
        ]
    }

    /// Total bins.
    #[inline]
    pub fn n_bins(&self) -> usize {
        let d = self.dimension();
        d[0] * d[1] * d[2]
    }

    /// Volume of bin `(i, j, k)` \[cm^3\]: `src/mesh.cpp:1867`, the product of
    /// the three edge differences.
    #[inline]
    pub fn volume(&self, ijk: [usize; 3]) -> f64 {
        (0..3)
            .map(|a| self.grid[a][ijk[a] + 1] - self.grid[a][ijk[a]])
            .product()
    }
}

/// **Cylindrical** `(r, phi, z)` mesh about `origin`.
///
/// `openmc::CylindricalMesh`. This is the natural tally geometry for every core
/// model in this repository — the workspace's standing correction is that
/// reactor cores are R-Z, not slabs.
///
/// `phi` is measured from the +x axis and is mapped into `[0, 2 pi)`, matching
/// `src/mesh.cpp:1932`. `z` is absolute (relative to `origin.z`).
#[derive(Debug, Clone, PartialEq)]
pub struct CylindricalMesh {
    /// Ascending radial edges \[cm\].
    pub r_grid: Vec<f64>,
    /// Ascending azimuthal edges \[rad\], within `[0, 2 pi]`.
    pub phi_grid: Vec<f64>,
    /// Ascending axial edges \[cm\], relative to `origin`.
    pub z_grid: Vec<f64>,
    /// Mesh origin \[cm\].
    pub origin: Position,
}

impl CylindricalMesh {
    #[inline]
    pub fn dimension(&self) -> [usize; 3] {
        [
            self.r_grid.len().saturating_sub(1),
            self.phi_grid.len().saturating_sub(1),
            self.z_grid.len().saturating_sub(1),
        ]
    }

    #[inline]
    pub fn n_bins(&self) -> usize {
        let d = self.dimension();
        d[0] * d[1] * d[2]
    }

    /// Volume of bin `(i, j, k)` \[cm^3\]: `src/mesh.cpp:2159`,
    /// `0.5 (r_o^2 - r_i^2) (phi_o - phi_i) (z_o - z_i)`.
    #[inline]
    pub fn volume(&self, ijk: [usize; 3]) -> f64 {
        let (ri, ro) = (self.r_grid[ijk[0]], self.r_grid[ijk[0] + 1]);
        let (pi_, po) = (self.phi_grid[ijk[1]], self.phi_grid[ijk[1] + 1]);
        let (zi, zo) = (self.z_grid[ijk[2]], self.z_grid[ijk[2] + 1]);
        0.5 * (ro * ro - ri * ri) * (po - pi_) * (zo - zi)
    }
}

/// **Spherical** `(r, theta, phi)` mesh about `origin`.
///
/// `openmc::SphericalMesh`. `theta` is the **polar** angle from +z in
/// `[0, pi]`; `phi` the azimuth from +x in `[0, 2 pi)`. That ordering is
/// upstream's (`src/mesh.cpp:2230`) and is the opposite of the physics
/// convention some texts use, which is exactly the kind of thing that produces
/// a mesh that looks right and bins wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct SphericalMesh {
    /// Ascending radial edges \[cm\].
    pub r_grid: Vec<f64>,
    /// Ascending polar edges \[rad\], within `[0, pi]`.
    pub theta_grid: Vec<f64>,
    /// Ascending azimuthal edges \[rad\], within `[0, 2 pi]`.
    pub phi_grid: Vec<f64>,
    /// Mesh origin \[cm\].
    pub origin: Position,
}

impl SphericalMesh {
    #[inline]
    pub fn dimension(&self) -> [usize; 3] {
        [
            self.r_grid.len().saturating_sub(1),
            self.theta_grid.len().saturating_sub(1),
            self.phi_grid.len().saturating_sub(1),
        ]
    }

    #[inline]
    pub fn n_bins(&self) -> usize {
        let d = self.dimension();
        d[0] * d[1] * d[2]
    }

    /// Volume of bin `(i, j, k)` \[cm^3\]: `src/mesh.cpp:2493`,
    /// `(1/3) (r_o^3 - r_i^3) (cos theta_i - cos theta_o) (phi_o - phi_i)`.
    ///
    /// Note the cosine difference is `inner - outer`: `cos` decreases on
    /// `[0, pi]`, so that ordering is what keeps the volume positive.
    #[inline]
    pub fn volume(&self, ijk: [usize; 3]) -> f64 {
        let (ri, ro) = (self.r_grid[ijk[0]], self.r_grid[ijk[0] + 1]);
        let (ti, to) = (self.theta_grid[ijk[1]], self.theta_grid[ijk[1] + 1]);
        let (pi_, po) = (self.phi_grid[ijk[2]], self.phi_grid[ijk[2] + 1]);
        (1.0 / 3.0) * (ro * ro * ro - ri * ri * ri) * (ti.cos() - to.cos()) * (po - pi_)
    }
}

/// **Enum dispatch over every tally mesh type** — the form
/// outram-mc-libs' `tally::filter::MeshFilter` holds so one filter serves all
/// of them. ~~every structured mesh type ... all four~~ **CORRECTED
/// 2026-10-03:** four structured kinds plus [`MeshKind::Unstructured`].
///
/// Enum rather than a trait object, per the workspace Rust design rule
/// (`docs/claude-md/rust-design-rules.md`: dispatch with enums, no `Box<dyn>`).
/// Upstream uses virtual dispatch off a `Mesh` base class; the enum is the
/// faithful equivalent here and costs no indirection.
#[derive(Debug, Clone, PartialEq)]
///
/// **Not `#[non_exhaustive]`, deliberately** (GitHub #486 / #492). ~~An
/// `Unstructured(Arc<..>)` variant is planned (#492)~~ **It landed
/// 2026-10-03 (#492)**: MC tally meshes are the same mesh objects the
/// deterministic solver uses for MGXS. Every exhaustive `match` on this enum
/// in the workspace was updated by hand when it landed, which is what the
/// absence of a wildcard-forcing attribute guaranteed; a future variant will
/// force the same review.
pub enum MeshKind {
    Regular(RegularMesh),
    Rectilinear(RectilinearMesh),
    Cylindrical(CylindricalMesh),
    Spherical(SphericalMesh),
    /// A neutral [`UnstructuredMesh`] (3-D only for tallies; see
    /// outram-mc-libs' `UnstructuredMeshExt`). Bins are cells, in cell order.
    Unstructured(Arc<UnstructuredMesh>),
}

impl MeshKind {
    /// Total bins.
    #[inline]
    pub fn n_bins(&self) -> usize {
        match self {
            Self::Regular(m) => m.n_bins(),
            Self::Rectilinear(m) => m.n_bins(),
            Self::Cylindrical(m) => m.n_bins(),
            Self::Spherical(m) => m.n_bins(),
            Self::Unstructured(m) => m.n_cells(),
        }
    }

    /// Volume of flat bin `bin` \[cm^3\], or `None` if out of range.
    ///
    /// This is what a **flux normalisation** needs: a track-length tally scores
    /// `cm` and dividing by the bin volume is what turns it into a flux. It is
    /// non-trivial for the curvilinear cases, which is why it is carried here
    /// rather than left to the caller to work out per mesh type.
    #[inline]
    pub fn bin_volume(&self, bin: usize) -> Option<f64> {
        if bin >= self.n_bins() {
            return None;
        }
        Some(match self {
            Self::Regular(m) => {
                let w = m.width();
                w[0] * w[1] * w[2]
            }
            Self::Rectilinear(m) => m.volume(unflatten(bin, m.dimension())),
            Self::Cylindrical(m) => m.volume(unflatten(bin, m.dimension())),
            Self::Spherical(m) => m.volume(unflatten(bin, m.dimension())),
            // The mesh stores volumes in its own unit; tallies are in cm.
            Self::Unstructured(m) => {
                let s = m.unit().cm_per_unit();
                m.cell_volume(bin) * s * s * s
            }
        })
    }
}

/// Flat bin index back to `(i, j, k)`, first axis fastest — the inverse of
/// `i + d0 (j + d1 k)`, which every mesh here uses.
#[inline]
pub fn unflatten(bin: usize, d: [usize; 3]) -> [usize; 3] {
    let i = bin % d[0];
    let j = (bin / d[0]) % d[1];
    let k = bin / (d[0] * d[1]);
    [i, j, k]
}
