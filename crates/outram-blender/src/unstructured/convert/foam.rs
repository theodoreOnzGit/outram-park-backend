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

//! Neutral mesh <-> `outram-foam-basic-lib` (feature `foam-export`): the
//! finite-volume family.
//!
//! - [`to_poly_mesh`] — the on-disk `polyMesh` form (points in metres, faces
//!   in OpenFOAM order: internal faces upper-triangular, then boundary faces
//!   patch by patch).
//! - [`to_fv_mesh`] — the solver form, via `PolyMesh::to_fv_mesh`, so the FV
//!   geometry is computed by the FV crate itself and not by this one.
//! - [`from_poly_mesh`] — back again (cells recognised as Tet4 / Hex8 where
//!   they are, so a `polyMesh` hex mesh can go on to `farrer-park`).
//!
//! **What does not survive the trip:** cell zones (`PolyMesh` has no
//! `cellZones`); FE node orderings of typed cells (they are recovered by
//! recognition on the way back, for tets and hexes only); `Tri6` midside
//! nodes (a 2-D mesh is refused anyway: `polyMesh` is 3-D).

use outram_foam_basic_lib::io::{MeshFace, PolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, PatchKind as FoamKind};
use outram_foam_basic_lib::primitives::Vector3;

use super::ConvertError;
use crate::unstructured::element::{LengthUnit, Patch, PatchKind, PolyFace};
use crate::unstructured::mesh::UnstructuredMesh;

fn kind_to_foam(k: PatchKind) -> (FoamKind, Option<usize>) {
    match k {
        PatchKind::Patch => (FoamKind::Patch, None),
        PatchKind::Wall => (FoamKind::Wall, None),
        PatchKind::Symmetry => (FoamKind::Symmetry, None),
        PatchKind::Empty => (FoamKind::Empty, None),
        PatchKind::Wedge => (FoamKind::Wedge, None),
        PatchKind::Cyclic { partner } => (FoamKind::Cyclic, partner),
        PatchKind::CyclicAmi { partner } => (FoamKind::CyclicAmi, partner),
        PatchKind::Processor => (FoamKind::Processor, None),
    }
}

fn kind_from_foam(k: FoamKind, partner: Option<usize>) -> PatchKind {
    match k {
        FoamKind::Patch => PatchKind::Patch,
        FoamKind::Wall => PatchKind::Wall,
        FoamKind::Symmetry => PatchKind::Symmetry,
        FoamKind::Empty => PatchKind::Empty,
        FoamKind::Wedge => PatchKind::Wedge,
        FoamKind::Cyclic => PatchKind::Cyclic { partner },
        FoamKind::CyclicAmi => PatchKind::CyclicAmi { partner },
        FoamKind::Processor => PatchKind::Processor,
    }
}

/// Neutral mesh -> `polyMesh` (points scaled to metres).
///
/// Internal faces are emitted with `owner < neighbour` (a face stored the
/// other way round is flipped: owner and neighbour swapped, loop reversed) and
/// sorted by `(owner, neighbour)`, the upper-triangular order OpenFOAM's LDU
/// addressing expects; boundary faces follow, patch by patch in the neutral
/// mesh's patch order, each patch contiguous.
///
/// # Errors
/// [`ConvertError::Dimension`] for a 2-D mesh.
pub fn to_poly_mesh(mesh: &UnstructuredMesh) -> Result<PolyMesh, ConvertError> {
    if mesh.dim() != 3 {
        return Err(ConvertError::Dimension { dim: mesh.dim(), target: "an OpenFOAM polyMesh" });
    }
    let s = mesh.unit().metres_per_unit();
    let points = mesh.points().iter().map(|p| Vector3::new(p[0] * s, p[1] * s, p[2] * s)).collect();

    let mut internal: Vec<MeshFace> = (0..mesh.n_faces())
        .filter_map(|f| {
            let n = mesh.neighbour(f)?;
            let o = mesh.owner(f);
            let mut verts = mesh.face(f).to_vec();
            if o < n {
                Some(MeshFace { verts, owner: o, neighbour: Some(n) })
            } else {
                verts.reverse();
                Some(MeshFace { verts, owner: n, neighbour: Some(o) })
            }
        })
        .collect();
    internal.sort_by_key(|f| (f.owner, f.neighbour));
    let n_internal = internal.len();

    let mut faces = internal;
    let mut patches = Vec::with_capacity(mesh.patches().len());
    for p in mesh.patches() {
        let start = faces.len();
        for &f in &p.faces {
            faces.push(MeshFace { verts: mesh.face(f).to_vec(), owner: mesh.owner(f), neighbour: None });
        }
        let (kind, partner) = kind_to_foam(p.kind);
        let mut bp = BoundaryPatch::new(p.name.clone(), start, p.faces.len(), kind);
        bp.cyclic_partner = partner;
        patches.push(bp);
    }
    Ok(PolyMesh { points, faces, n_internal_faces: n_internal, n_cells: mesh.n_cells(), patches })
}

/// Neutral mesh -> solver-ready `FvMesh`, through [`to_poly_mesh`] and
/// `PolyMesh::to_fv_mesh` (which computes the FV geometry and validates).
///
/// # Errors
/// As [`to_poly_mesh`], or the FV crate's validation message.
pub fn to_fv_mesh(mesh: &UnstructuredMesh) -> Result<FvMesh, ConvertError> {
    to_poly_mesh(mesh)?.to_fv_mesh().map_err(|e| ConvertError::Solver(e.to_string()))
}

/// `polyMesh` -> neutral mesh, in metres.
///
/// # Errors
/// Any construction error of [`UnstructuredMesh::from_polyhedral`].
pub fn from_poly_mesh(poly: &PolyMesh) -> Result<UnstructuredMesh, ConvertError> {
    let points = poly.points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let faces = poly
        .faces
        .iter()
        .map(|f| PolyFace { verts: f.verts.clone(), owner: f.owner, neighbour: f.neighbour })
        .collect();
    let patches = poly
        .patches
        .iter()
        .map(|p| Patch {
            name: p.name.clone(),
            kind: kind_from_foam(p.kind, p.cyclic_partner),
            faces: (p.start..p.start + p.size).collect(),
        })
        .collect();
    Ok(UnstructuredMesh::from_polyhedral(LengthUnit::Metre, points, faces, poly.n_cells, patches, Vec::new())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unstructured::element::ElementKind;
    use crate::unstructured::one_d::one_d_column;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;

    /// The neutral 1-D column, converted to an `FvMesh`, reproduces
    /// `create_one_d_mesh` cell for cell: volumes, centres, and the area
    /// vectors of the internal faces and of the `right`/`left` patches. The
    /// only difference allowed is the extra `sides` patch (type `empty`).
    #[test]
    fn one_d_column_matches_create_one_d_mesh() {
        let (l, a, n) = (2.0, 0.04, 8);
        let ours = to_fv_mesh(&one_d_column(LengthUnit::Metre, l, a, n).unwrap()).unwrap();
        let theirs =
            create_one_d_mesh(Length::new::<meter>(l), Area::new::<square_meter>(a), n as i64).unwrap();
        assert_eq!(ours.n_cells, theirs.n_cells);
        assert_eq!(ours.n_internal_faces, theirs.n_internal_faces);
        for c in 0..n {
            assert!((ours.cell_volumes[c] - theirs.cell_volumes[c]).abs() < 1e-15);
            assert!((ours.cell_centres[c].x - theirs.cell_centres[c].x).abs() < 1e-14);
        }
        for f in 0..theirs.n_internal_faces {
            assert!((ours.face_area_vectors[f].x - theirs.face_area_vectors[f].x).abs() < 1e-15);
            assert_eq!(ours.owner[f], theirs.owner[f]);
            assert_eq!(ours.neighbour[f], theirs.neighbour[f]);
        }
        for name in ["right", "left"] {
            let po = ours.patches.iter().find(|p| p.name == name).unwrap();
            let pt = theirs.patches.iter().find(|p| p.name == name).unwrap();
            assert_eq!(po.size, 1);
            let (fo, ft) = (po.start, pt.start);
            assert!((ours.face_area_vectors[fo].x - theirs.face_area_vectors[ft].x).abs() < 1e-15);
            assert_eq!(ours.owner[fo], theirs.owner[ft]);
        }
        let sides = ours.patches.iter().find(|p| p.name == "sides").unwrap();
        assert_eq!(sides.kind, FoamKind::Empty);
        assert_eq!(sides.size, 4 * n);
    }

    /// polyMesh -> neutral -> polyMesh keeps the topology, and the hexes are
    /// recognised on the way in.
    #[test]
    fn poly_mesh_round_trip() {
        let m = one_d_column(LengthUnit::Metre, 1.0, 1.0, 3).unwrap();
        let p = to_poly_mesh(&m).unwrap();
        let back = from_poly_mesh(&p).unwrap();
        assert_eq!(back.n_cells(), 3);
        assert!((0..3).all(|c| back.cell_kind(c) == ElementKind::Hex8));
        assert!((back.total_volume() - 1.0).abs() < 1e-14);
        let p2 = to_poly_mesh(&back).unwrap();
        assert_eq!(p2.faces, p.faces);
        assert_eq!(p2.patches, p.patches);
    }
}
