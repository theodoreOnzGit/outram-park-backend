// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Meshes for the steady conduction solver (gh:#949).
//!
//! - [`radial_mesh`]: a one-dimensional slab, cylinder or sphere in nested
//!   shells, with the true face areas (`A`, `2 pi r H`, `4 pi r^2`) and shell
//!   volumes. The verification mesh: [`super::closed_form`] is exact on it.
//! - [`voxel_mesh`]: cubic voxels over a box, keeping the voxels whose centre
//!   a caller-supplied function puts inside the body. Any shape, at the cost
//!   of a staircase surface. This is the beginner thermal profile's mesh.
//!
//! Both put faces in OpenFOAM's upper-triangular order (`owner < neighbour`,
//! sorted by owner), which the DIC-preconditioned conjugate gradient needs.
//!
//! **Mesh-nexus note (gh:#492).** `outram_blender::unstructured` is the
//! workspace's neutral FV/FE/MC mesh, but this crate may not depend on
//! `outram-blender` (#486 rule 1), and a structured voxel grid needs only the
//! [`FvMesh`] built here directly. Exporting a [`VoxelMesh`] to the neutral
//! mesh is left to the crate that owns that dependency.

use petir::mathf::RealMath;
use std::sync::Arc;

use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

use super::SteadyConductionError;
use crate::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use crate::primitives::Vector3;

/// A mesh plus the zone (material region) of every cell.
#[derive(Debug, Clone)]
pub struct ConductionMesh {
    /// The finite-volume mesh, lengths in metres.
    pub mesh: Arc<FvMesh>,
    /// Zone index of each cell; zones carry the heat source and conductivity.
    pub cell_zone: Vec<usize>,
    /// Number of zones (`max(cell_zone) + 1`, or more if a zone has no cell).
    pub n_zones: usize,
}

/// The shape of a one-dimensional radial body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialGeometry {
    /// A plate, symmetric about its mid-plane; the radius is the distance from
    /// the mid-plane, and `area` is the plate's face area.
    Slab {
        /// Face area of the plate.
        area: Area,
    },
    /// A cylinder of the given height, insulated at its ends.
    Cylinder {
        /// Height (axial length).
        height: Length,
    },
    /// A sphere.
    Sphere,
}

impl RadialGeometry {
    /// The exponent `m` of the measure `r^m`: slab 0, cylinder 1, sphere 2.
    #[must_use]
    pub fn exponent(self) -> i32 {
        match self {
            Self::Slab { .. } => 0,
            Self::Cylinder { .. } => 1,
            Self::Sphere => 2,
        }
    }

    /// Face area at radius `r` (m), m^2.
    fn area_at(self, r: f64) -> f64 {
        match self {
            Self::Slab { area } => area.get::<square_meter>(),
            Self::Cylinder { height } => 2.0 * std::f64::consts::PI * r * height.get::<meter>(),
            Self::Sphere => 4.0 * std::f64::consts::PI * r * r,
        }
    }

    /// Volume between radii `r1 < r2` (m), m^3.
    fn volume_between(self, r1: f64, r2: f64) -> f64 {
        match self {
            Self::Slab { area } => area.get::<square_meter>() * (r2 - r1),
            Self::Cylinder { height } => {
                std::f64::consts::PI * height.get::<meter>() * (r2 * r2 - r1 * r1)
            }
            Self::Sphere => 4.0 / 3.0 * std::f64::consts::PI * (r2.r_powi(3) - r1.r_powi(3)),
        }
    }
}

/// One shell of a radial mesh: out to `outer_radius`, in `cells` equal steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialShell {
    /// Outer radius of the shell.
    pub outer_radius: Length,
    /// Number of equal-thickness cells across the shell (at least 1).
    pub cells: usize,
}

/// Name of the outer-surface patch of a [`radial_mesh`] (patch index 0).
pub const RADIAL_SURFACE_PATCH: &str = "surface";
/// Name of the centre (symmetry) patch of a [`radial_mesh`] (patch index 1).
/// Its face has zero area for a cylinder or sphere.
pub const RADIAL_CENTRE_PATCH: &str = "centre";

/// Build a one-dimensional radial mesh of nested shells (innermost first).
///
/// Shell `j` becomes zone `j`. Cell centres sit at the mid-radius of each cell,
/// laid along the x axis; faces carry the true area at their radius and cells
/// the true shell volume, so the finite-volume Laplacian is the standard
/// conservative curvilinear scheme. Patches: index 0 [`RADIAL_SURFACE_PATCH`]
/// (outer face), index 1 [`RADIAL_CENTRE_PATCH`] (`r = 0`).
///
/// # Errors
/// [`SteadyConductionError::InvalidInput`] for no shells, a shell with zero
/// cells, or radii not strictly increasing; [`SteadyConductionError::Mesh`]
/// if the assembled mesh fails validation.
pub fn radial_mesh(
    geometry: RadialGeometry,
    shells: &[RadialShell],
) -> Result<ConductionMesh, SteadyConductionError> {
    if shells.is_empty() {
        return Err(SteadyConductionError::InvalidInput(
            "radial mesh needs a shell".into(),
        ));
    }
    // Cell edges and zones.
    let mut edges = vec![0.0_f64];
    let mut cell_zone = Vec::new();
    for (j, shell) in shells.iter().enumerate() {
        let r_in = edges[edges.len() - 1];
        let r_out = shell.outer_radius.get::<meter>();
        if shell.cells == 0 || !(r_out > r_in) || !r_out.is_finite() {
            return Err(SteadyConductionError::InvalidInput(format!(
                "shell {j}: needs at least one cell and an outer radius above {r_in} m (got {} cells, {r_out} m)",
                shell.cells
            )));
        }
        let dr = (r_out - r_in) / shell.cells as f64;
        for i in 1..=shell.cells {
            edges.push(if i == shell.cells {
                r_out
            } else {
                r_in + dr * i as f64
            });
            cell_zone.push(j);
        }
    }
    let n = cell_zone.len();
    let axis = |x: f64| Vector3::new(x, 0.0, 0.0);

    let cell_volumes: Vec<f64> = (0..n)
        .map(|c| geometry.volume_between(edges[c], edges[c + 1]))
        .collect();
    let cell_centres: Vec<Vector3> = (0..n)
        .map(|c| axis(0.5 * (edges[c] + edges[c + 1])))
        .collect();

    // Faces: internal 0..n-1 (between cell c and c+1, at edges[c+1]), then
    // the surface face (owner n-1), then the centre face (owner 0).
    let mut owner: Vec<usize> = (0..n - 1).collect();
    let neighbour: Vec<usize> = (1..n).collect();
    let mut face_area_vectors: Vec<Vector3> = (0..n - 1)
        .map(|c| axis(geometry.area_at(edges[c + 1])))
        .collect();
    let mut face_centres: Vec<Vector3> = (0..n - 1).map(|c| axis(edges[c + 1])).collect();
    owner.push(n - 1);
    face_area_vectors.push(axis(geometry.area_at(edges[n])));
    face_centres.push(axis(edges[n]));
    owner.push(0);
    face_area_vectors.push(axis(-geometry.area_at(0.0)));
    face_centres.push(axis(0.0));

    let centre_kind = match geometry {
        RadialGeometry::Slab { .. } => PatchKind::Symmetry,
        _ => PatchKind::Patch,
    };
    let mesh = FvMeshBuilder::new()
        .n_cells(n)
        .n_internal_faces(n - 1)
        .owner(owner)
        .neighbour(neighbour)
        .patches(vec![
            BoundaryPatch::new(RADIAL_SURFACE_PATCH, n - 1, 1, PatchKind::Patch),
            BoundaryPatch::new(RADIAL_CENTRE_PATCH, n, 1, centre_kind),
        ])
        .cell_volumes(cell_volumes)
        .cell_centres(cell_centres)
        .face_area_vectors(face_area_vectors)
        .face_centres(face_centres)
        .build()
        .map_err(|e| SteadyConductionError::Mesh(format!("{e:?}")))?;
    Ok(ConductionMesh {
        mesh: Arc::new(mesh),
        cell_zone,
        n_zones: shells.len(),
    })
}

/// A box of cubic voxels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelGrid {
    /// Lowest corner of the box (x, y, z).
    pub origin: [Length; 3],
    /// Edge length of every voxel.
    pub spacing: Length,
    /// Voxel counts along x, y and z.
    pub counts: [usize; 3],
}

impl VoxelGrid {
    /// Flat index of voxel `(i, j, k)`, x fastest.
    #[must_use]
    pub fn flat(&self, ijk: [usize; 3]) -> usize {
        ijk[0] + self.counts[0] * (ijk[1] + self.counts[1] * ijk[2])
    }

    /// Centre of voxel `(i, j, k)`.
    #[must_use]
    pub fn centre(&self, ijk: [usize; 3]) -> [Length; 3] {
        let h = self.spacing;
        [0, 1, 2].map(|a| self.origin[a] + h * (ijk[a] as f64 + 0.5))
    }

    /// The voxel containing point `p`, or `None` outside the box.
    #[must_use]
    pub fn voxel_of(&self, p: [Length; 3]) -> Option<[usize; 3]> {
        let h = self.spacing.get::<meter>();
        let mut ijk = [0usize; 3];
        for a in 0..3 {
            let s = (p[a] - self.origin[a]).get::<meter>() / h;
            if !(s >= 0.0) || s >= self.counts[a] as f64 {
                return None;
            }
            ijk[a] = s as usize;
        }
        Some(ijk)
    }
}

/// A voxel mesh: the conduction mesh plus the map between voxels and cells.
///
/// All boundary faces (body to outside, and body to box edge) form one patch,
/// index 0, named [`VOXEL_SURFACE_PATCH`].
#[derive(Debug, Clone)]
pub struct VoxelMesh {
    /// The mesh and the zone of each cell.
    pub conduction: ConductionMesh,
    /// The grid it was cut from.
    pub grid: VoxelGrid,
    /// Mesh cell of each voxel (flat index, x fastest), `None` outside the body.
    pub voxel_cell: Vec<Option<usize>>,
    /// Voxel `(i, j, k)` of each mesh cell.
    pub cell_voxel: Vec<[usize; 3]>,
    /// Outward unit axis normal of each surface face (patch order).
    pub surface_normal: Vec<[f64; 3]>,
}

/// Name of the single boundary patch of a [`voxel_mesh`].
pub const VOXEL_SURFACE_PATCH: &str = "surface";

impl VoxelMesh {
    /// The mesh cell containing point `p`, or `None` outside the body.
    #[must_use]
    pub fn cell_at(&self, p: [Length; 3]) -> Option<usize> {
        self.grid
            .voxel_of(p)
            .and_then(|ijk| self.voxel_cell[self.grid.flat(ijk)])
    }

    /// Number of surface faces (the size of patch 0).
    #[must_use]
    pub fn n_surface_faces(&self) -> usize {
        self.surface_normal.len()
    }
}

/// Cut a voxel mesh of a body out of `grid`.
///
/// `zone_at(p)` returns the zone of the body at point `p` (a voxel centre), or
/// `None` where there is no conducting body (coolant, void, outside). Every
/// voxel with a zone becomes a cell; every face between a cell and a non-cell
/// (or the box edge) is a surface face.
///
/// The surface is a **staircase**: its area does not converge to the true
/// area under refinement (a voxelised sphere's tends to 3/2 of it). A caller
/// that cools the surface convectively should pass the per-face
/// film-area fraction `|n_axis . n_true|` to the solver (see
/// [`super::PatchCondition::with_film_area_fraction`]); the sum of
/// `area x fraction` then converges to the true area.
///
/// # Errors
/// [`SteadyConductionError::InvalidInput`] for a zero count or non-positive
/// spacing, or when no voxel is inside the body;
/// [`SteadyConductionError::Mesh`] if the mesh fails validation.
pub fn voxel_mesh<F>(grid: &VoxelGrid, zone_at: F) -> Result<VoxelMesh, SteadyConductionError>
where
    F: Fn([Length; 3]) -> Option<usize>,
{
    let h = grid.spacing.get::<meter>();
    if grid.counts.contains(&0) || !(h > 0.0) || !h.is_finite() {
        return Err(SteadyConductionError::InvalidInput(format!(
            "voxel grid needs non-zero counts and a positive spacing (got {:?}, {h} m)",
            grid.counts
        )));
    }
    let [nx, ny, nz] = grid.counts;
    let n_vox = nx * ny * nz;
    let mut voxel_cell = vec![None; n_vox];
    let mut cell_voxel = Vec::new();
    let mut cell_zone = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if let Some(z) = zone_at(grid.centre([i, j, k])) {
                    voxel_cell[grid.flat([i, j, k])] = Some(cell_voxel.len());
                    cell_voxel.push([i, j, k]);
                    cell_zone.push(z);
                }
            }
        }
    }
    let n = cell_voxel.len();
    if n == 0 {
        return Err(SteadyConductionError::InvalidInput(
            "no voxel centre lies inside the body; refine the grid or check the box".into(),
        ));
    }
    let n_zones = cell_zone.iter().copied().max().unwrap_or(0) + 1;
    let o = grid.origin.map(|l| l.get::<meter>());
    let centre = |ijk: [usize; 3]| {
        Vector3::new(
            o[0] + h * (ijk[0] as f64 + 0.5),
            o[1] + h * (ijk[1] as f64 + 0.5),
            o[2] + h * (ijk[2] as f64 + 0.5),
        )
    };
    let unit = |a: usize, s: f64| {
        let mut v = [0.0; 3];
        v[a] = s;
        v
    };
    let area = h * h;

    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    let mut sf = Vec::new();
    let mut cf = Vec::new();
    // Boundary faces collected separately and appended after the internal ones.
    let mut b_owner = Vec::new();
    let mut b_sf = Vec::new();
    let mut b_cf = Vec::new();
    let mut surface_normal = Vec::new();
    for (c, ijk) in cell_voxel.iter().enumerate() {
        let cc = centre(*ijk);
        for a in 0..3 {
            for s in [-1.0_f64, 1.0] {
                let n_hat = unit(a, s);
                let n_vec = Vector3::new(n_hat[0], n_hat[1], n_hat[2]);
                let neighbour_voxel = if s < 0.0 {
                    ijk[a].checked_sub(1).map(|v| {
                        let mut t = *ijk;
                        t[a] = v;
                        t
                    })
                } else if ijk[a] + 1 < grid.counts[a] {
                    let mut t = *ijk;
                    t[a] += 1;
                    Some(t)
                } else {
                    None
                };
                let nb_cell = neighbour_voxel.and_then(|t| voxel_cell[grid.flat(t)]);
                match nb_cell {
                    // Internal face: stored once, from the lower-index cell
                    // (the +direction neighbour always has the higher index).
                    Some(nb) if s > 0.0 => {
                        owner.push(c);
                        neighbour.push(nb);
                        sf.push(n_vec * area);
                        cf.push(cc + n_vec * (0.5 * h));
                    }
                    Some(_) => {}
                    None => {
                        b_owner.push(c);
                        b_sf.push(n_vec * area);
                        b_cf.push(cc + n_vec * (0.5 * h));
                        surface_normal.push(n_hat);
                    }
                }
            }
        }
    }
    let n_internal = owner.len();
    let n_boundary = b_owner.len();
    owner.extend(b_owner);
    sf.extend(b_sf);
    cf.extend(b_cf);
    let mesh = FvMeshBuilder::new()
        .n_cells(n)
        .n_internal_faces(n_internal)
        .owner(owner)
        .neighbour(neighbour)
        .patches(vec![BoundaryPatch::new(
            VOXEL_SURFACE_PATCH,
            n_internal,
            n_boundary,
            PatchKind::Patch,
        )])
        .cell_volumes(vec![h * h * h; n])
        .cell_centres(cell_voxel.iter().map(|v| centre(*v)).collect())
        .face_area_vectors(sf)
        .face_centres(cf)
        .build()
        .map_err(|e| SteadyConductionError::Mesh(format!("{e:?}")))?;
    Ok(VoxelMesh {
        conduction: ConductionMesh {
            mesh: Arc::new(mesh),
            cell_zone,
            n_zones,
        },
        grid: *grid,
        voxel_cell,
        cell_voxel,
        surface_normal,
    })
}
