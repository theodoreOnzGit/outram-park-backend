// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// No upstream code is translated in this file: it converts this crate's
// `VolumeMesh` to and from `outram-foam-mesh`'s `poly_dual_mesh` types, whose
// construction is the port of OpenFOAM's `polyDualMesh` (see that file's
// provenance header: OpenFOAM-dev 852527377c7e, src/conversion/polyDualMesh/
// and applications/utilities/mesh/manipulation/polyDualMesh/; Copyright (C)
// 2011-2026 OpenFOAM Foundation, GPL-3.0-or-later).
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

//! **The cell-centre dual of a `VolumeMesh`** (gh:#1240, Route B), through
//! `outram-foam-mesh`'s port of OpenFOAM's `polyDualMesh`
//! ([`outram_foam_mesh::poly_dual_mesh::poly_dual_mesh`]). Built with this
//! crate's `cell-centre-dual` feature; the pipeline selects it with
//! [`crate::dual::DualKind::CellCentre`].
//!
//! Reused, not re-written: this module only converts. The primal goes in with
//! its faces reordered as OpenFOAM orders them (internal first, then the
//! boundary grouped by `mesh.patches`, any boundary face outside every patch
//! last, in an extra patch); the dual comes back the same way, its patches
//! named after the primal's. The dual's corners are the primal **cell
//! centroids**, plus boundary-face centres, boundary-edge midpoints and the
//! feature points on the boundary, where the median dual
//! ([`crate::dual`]) puts them at edge midpoints and face and cell centroids.
//!
//! On a tetrahedral primal the dual's interior faces are the rings of the
//! centroids of the tets around each interior edge; nothing guarantees such a
//! ring is planar or that its dual cells are convex, so the pipeline keeps the
//! result only if it is closed and has no inverted cell.

use crate::math::Vec3;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch as FoamPatch, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use crate::dual::FeaturePointRule;
use outram_foam_mesh::poly_dual_mesh::{poly_dual_mesh_with, DualMesh, FeaturePoints, PolyMesh};

/// The primal `mesh` as `outram-foam-mesh`'s [`PolyMesh`], faces in OpenFOAM
/// order, and the patch names (the last is `"unpatched"` when a boundary face
/// lies in no patch of `mesh`).
pub fn to_primal(mesh: &VolumeMesh) -> (PolyMesh, Vec<String>) {
    let n_faces = mesh.face_count();
    let mut patch_of = vec![usize::MAX; n_faces];
    for (pi, pa) in mesh.patches.iter().enumerate() {
        for f in pa.start_face..(pa.start_face + pa.n_faces).min(n_faces) {
            patch_of[f] = pi;
        }
    }
    let mut order: Vec<usize> = (0..n_faces).filter(|&f| mesh.neighbour[f].is_some()).collect();
    let n_internal = order.len();
    let mut names: Vec<String> = mesh.patches.iter().map(|p| p.name.clone()).collect();
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); names.len() + 1];
    for f in 0..n_faces {
        if mesh.neighbour[f].is_none() {
            let g = if patch_of[f] == usize::MAX { names.len() } else { patch_of[f] };
            groups[g].push(f);
        }
    }
    if groups[names.len()].is_empty() {
        groups.pop();
    } else {
        names.push("unpatched".into());
    }
    let mut patches = Vec::new();
    for (g, fs) in groups.iter().enumerate() {
        patches.push(FoamPatch::new(names[g].clone(), order.len(), fs.len(), PatchKind::Wall));
        order.extend_from_slice(fs);
    }
    let primal = PolyMesh {
        points: mesh.points.iter().map(|p| Vector3::new(p.x, p.y, p.z)).collect(),
        faces: order.iter().map(|&f| mesh.faces[f].clone()).collect(),
        owner: order.iter().map(|&f| mesh.owner[f]).collect(),
        neighbour: order[..n_internal].iter().map(|&f| mesh.neighbour[f].expect("internal")).collect(),
        n_internal_faces: n_internal,
        patches,
        n_cells: mesh.n_cells,
    };
    (primal, names)
}

/// `outram-foam-mesh`'s [`DualMesh`] as a [`VolumeMesh`]: internal faces
/// first, then the boundary grouped by source patch, named by `names`
/// (`"walls"` for a face with no source patch).
pub fn from_dual(dual: &DualMesh, names: &[String]) -> VolumeMesh {
    let nf = dual.faces.len();
    let mut order: Vec<usize> = (0..nf).filter(|&f| dual.neighbour[f].is_some()).collect();
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); names.len() + 1];
    for f in 0..nf {
        if dual.neighbour[f].is_none() {
            groups[dual.face_patch[f].unwrap_or(names.len()).min(names.len())].push(f);
        }
    }
    let mut patches = Vec::new();
    for (g, fs) in groups.iter().enumerate() {
        if fs.is_empty() {
            continue;
        }
        let name = names.get(g).cloned().unwrap_or_else(|| "walls".into());
        patches.push(BoundaryPatch { name, start_face: order.len(), n_faces: fs.len() });
        order.extend_from_slice(fs);
    }
    VolumeMesh {
        points: dual.points.iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect(),
        faces: order.iter().map(|&f| dual.faces[f].clone()).collect(),
        owner: order.iter().map(|&f| dual.owner[f]).collect(),
        neighbour: order.iter().map(|&f| dual.neighbour[f]).collect(),
        n_cells: dual.n_cells,
        patches,
        cells: None,
    }
}

/// The cell-centre (`polyDualMesh`) dual of `mesh`, with features at
/// `feature_angle_deg` and every patch seam of `mesh.patches` (see
/// [`outram_foam_mesh::poly_dual_mesh`]). Errors when the boundary is not
/// manifold (the port refuses it, as upstream does).
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, cell_centre_dual::cell_centre_dual};
///
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let hex = carve_box(&p, &t, 0.5);
/// let dual = cell_centre_dual(&hex, 45.0).unwrap();
/// assert_eq!(dual.cell_count(), hex.point_count());
/// assert!((dual.total_volume() - 1.0).abs() < 1e-9);
/// assert!(dual.validate().is_ok());
/// ```
pub fn cell_centre_dual(mesh: &VolumeMesh, feature_angle_deg: f64) -> Result<VolumeMesh, String> {
    cell_centre_dual_with(mesh, feature_angle_deg, FeaturePointRule::OnFeatureEdge)
}

/// [`cell_centre_dual`] with the feature-point rule named
/// ([`FeaturePointRule`], mapped onto `outram-foam-mesh`'s `FeaturePoints`).
pub fn cell_centre_dual_with(mesh: &VolumeMesh, feature_angle_deg: f64, rule: FeaturePointRule) -> Result<VolumeMesh, String> {
    let (primal, names) = to_primal(mesh);
    let rule = match rule {
        FeaturePointRule::OnFeatureEdge => FeaturePoints::OnFeatureEdge,
        FeaturePointRule::Corners => FeaturePoints::Corners,
    };
    let dual = poly_dual_mesh_with(&primal, feature_angle_deg, rule).map_err(|e| e.to_string())?;
    Ok(from_dual(&dual, &names))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;

    /// V&V (gh:#1240) — the conversion is faithful: the cell-centre dual of
    /// a carved 2×2×2 hex block (uniform, so its dual is known: the interior
    /// vertex's cell is the unit-spacing cube, 0.5³ m³) through this module
    /// equals `outram-foam-mesh`'s dual of the same block. Predictions
    /// (written 2026-11-04 before the first run): 27 cells; total volume 1
    /// to 1e-12; the interior cell's volume 0.125; closed.
    #[test]
    fn cell_centre_dual_of_a_carved_block() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 0.5);
        let dual = cell_centre_dual(&hex, 45.0).unwrap();
        dual.validate().unwrap();
        assert_eq!(dual.cell_count(), 27);
        assert!((dual.total_volume() - 1.0).abs() < 1e-12, "{}", dual.total_volume());
        let corners = cell_centre_dual_with(&hex, 45.0, FeaturePointRule::Corners).unwrap();
        assert!((corners.total_volume() - 1.0).abs() < 1e-12, "{}", corners.total_volume());
        let centre = hex.points.iter().position(|q| q.sub(Vec3::new(0.5, 0.5, 0.5)).length() < 1e-12).unwrap();
        let (vols, _) = crate::checks::cell_geometry(&dual);
        assert!((vols[centre] - 0.125).abs() < 1e-12, "{}", vols[centre]);
    }
}
