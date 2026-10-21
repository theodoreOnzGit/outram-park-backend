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

//! Element types, length units and patch kinds of the neutral
//! [`UnstructuredMesh`](super::UnstructuredMesh).
//!
//! # Local node orderings
//!
//! The finite-element kinds use the node orderings of `farrer-park`'s
//! `element::ElementType` (which are the VTK / Abaqus conventions), so a
//! converted mesh needs no renumbering:
//!
//! - [`ElementKind::Tet4`] — the fourth node lies on the positive side of the
//!   plane of the first three (`(n1 - n0) x (n2 - n0) . (n3 - n0) > 0`).
//! - [`ElementKind::Hex8`] — nodes 0-3 are the `zeta = -1` face,
//!   counter-clockwise seen from `+zeta`; nodes 4-7 the matching `zeta = +1`
//!   face.
//! - [`ElementKind::Tri3`] / [`ElementKind::Quad4`] — corners counter-clockwise
//!   in the `z = 0` plane; [`ElementKind::Tri6`] adds the midsides of edges
//!   0-1, 1-2, 2-0 as nodes 3, 4, 5.
//!
//! The local face tables below list each face **wound outward** (right-hand
//! normal pointing out of the element). The tables were checked by hand on the
//! reference elements; `face_tables_are_outward` in the tests checks them
//! numerically.

/// The unit the mesh's point coordinates are stored in.
///
/// The neutral mesh stores raw `f64` coordinates (the documented raw-`f64`
/// exception of `csg` and `spatial_mesh`: point location sits on the Monte
/// Carlo hot path) and carries the unit explicitly so no consumer has to
/// guess. Monte Carlo works in **cm**, `outram-foam-*` and `farrer-park` in
/// **m**; the converters scale.
///
/// This mirrors OpenMC's `UnstructuredMesh::length_multiplier_`
/// (`include/openmc/mesh.h:820`), which scales a mesh file's units to cm
/// (`LibMesh::get_bin`, `src/mesh.cpp:3973`, divides the query point by it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    /// Centimetres (the Monte Carlo convention).
    Centimetre,
    /// Metres (the finite-volume and finite-element convention).
    Metre,
    /// Millimetres (CAD exports).
    Millimetre,
}

impl LengthUnit {
    /// Centimetres per one stored unit.
    #[inline]
    pub fn cm_per_unit(self) -> f64 {
        match self {
            LengthUnit::Centimetre => 1.0,
            LengthUnit::Metre => 100.0,
            LengthUnit::Millimetre => 0.1,
        }
    }

    /// Metres per one stored unit.
    #[inline]
    pub fn metres_per_unit(self) -> f64 {
        self.cm_per_unit() * 0.01
    }
}

/// The type of one mesh cell.
///
/// The FV world sees every cell as a polyhedron (faces + owner/neighbour);
/// the FE world needs a **typed** element with an ordered node list. Both are
/// carried: every cell has its faces, and a typed cell also has its nodes in
/// the local ordering above. A polyhedral cell (an OpenFOAM `polyMesh` cell
/// that is not recognised as a tet or a hex, e.g. a cfMesh dual cell) has no
/// FE ordering and is [`ElementKind::Polyhedron`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementKind {
    /// 3-node linear triangle (2-D).
    Tri3,
    /// 6-node quadratic triangle (2-D). Geometry here uses the straight-sided
    /// corner triangle; the midside nodes are carried for the FE converter.
    Tri6,
    /// 4-node bilinear quadrilateral (2-D).
    Quad4,
    /// 4-node linear tetrahedron.
    Tet4,
    /// 8-node trilinear hexahedron.
    Hex8,
    /// A general polygon (2-D) with no FE ordering.
    Polygon,
    /// A general polyhedron (3-D) with no FE ordering.
    Polyhedron,
}

impl ElementKind {
    /// Spatial dimension of the cell: 2 or 3.
    #[inline]
    pub fn dim(self) -> usize {
        match self {
            ElementKind::Tri3 | ElementKind::Tri6 | ElementKind::Quad4 | ElementKind::Polygon => 2,
            ElementKind::Tet4 | ElementKind::Hex8 | ElementKind::Polyhedron => 3,
        }
    }

    /// Nodes per element, or `None` for the untyped polygon / polyhedron.
    #[inline]
    pub fn n_nodes(self) -> Option<usize> {
        match self {
            ElementKind::Tri3 => Some(3),
            ElementKind::Tri6 => Some(6),
            ElementKind::Quad4 => Some(4),
            ElementKind::Tet4 => Some(4),
            ElementKind::Hex8 => Some(8),
            ElementKind::Polygon | ElementKind::Polyhedron => None,
        }
    }

    /// Whether this is a typed finite element (has a fixed node ordering).
    #[inline]
    pub fn is_typed(self) -> bool {
        self.n_nodes().is_some()
    }

    /// Local faces of a typed element, each wound outward, as indices into
    /// the element's node list. In 2-D a "face" is an edge (two corner nodes,
    /// ordered so that `(dy, -dx)` is the outward normal of a
    /// counter-clockwise element).
    ///
    /// Returns an empty slice for the untyped kinds, whose faces are given
    /// explicitly.
    pub fn local_faces(self) -> &'static [&'static [usize]] {
        match self {
            ElementKind::Tri3 | ElementKind::Tri6 => &[&[0, 1], &[1, 2], &[2, 0]],
            ElementKind::Quad4 => &[&[0, 1], &[1, 2], &[2, 3], &[3, 0]],
            ElementKind::Tet4 => &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[0, 3, 2]],
            ElementKind::Hex8 => &[
                &[0, 3, 2, 1],
                &[4, 5, 6, 7],
                &[0, 1, 5, 4],
                &[1, 2, 6, 5],
                &[2, 3, 7, 6],
                &[3, 0, 4, 7],
            ],
            ElementKind::Polygon | ElementKind::Polyhedron => &[],
        }
    }

    /// Number of corner (vertex) nodes: the leading nodes that define the
    /// straight-sided geometry. Equal to [`Self::n_nodes`] except for `Tri6`.
    #[inline]
    pub fn n_corners(self) -> Option<usize> {
        match self {
            ElementKind::Tri6 => Some(3),
            other => other.n_nodes(),
        }
    }
}

/// Boundary-patch type, the union of what the FV converters need.
///
/// Mirrors `outram_foam_basic_lib::mesh::PatchKind` (OpenFOAM's
/// `polyPatch` types). A finite-element consumer ignores the kind and uses the
/// patch as a named facet set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatchKind {
    /// Generic boundary.
    Patch,
    /// Wall.
    Wall,
    /// Symmetry plane.
    Symmetry,
    /// 2-D reduced case (OpenFOAM `empty`).
    Empty,
    /// Axisymmetric wedge.
    Wedge,
    /// Conformal periodic pair; `partner` is the patch index of the other
    /// half when known.
    Cyclic {
        /// Patch index of the partner half, if resolved.
        partner: Option<usize>,
    },
    /// Non-conformal periodic pair (OpenFOAM `cyclicAMI`). The AMI weights
    /// are not part of the description; the FV side recomputes them.
    CyclicAmi {
        /// Patch index of the partner half, if resolved.
        partner: Option<usize>,
    },
    /// Inter-processor decomposition seam.
    Processor,
}

/// A named set of boundary faces.
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    /// Patch name (`"inlet"`, `"walls"`, `"defaultFaces"`, ...).
    pub name: String,
    /// Patch type.
    pub kind: PatchKind,
    /// Global face indices (boundary faces only), ascending.
    pub faces: Vec<usize>,
}

/// A named set of cells: a material region, a solver region, a tally group.
///
/// OpenFOAM `cellZone`, Exodus element block, MOAB material set. Zones may
/// overlap and need not cover the mesh;
/// [`UnstructuredMesh::zone_of`](super::UnstructuredMesh::zone_of) reports
/// the first zone a cell is in.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// Zone name (`"fuel"`, `"reflector"`, ...).
    pub name: String,
    /// Cell indices, ascending.
    pub cells: Vec<usize>,
}

/// One input element for
/// [`UnstructuredMesh::from_elements`](super::UnstructuredMesh::from_elements):
/// a typed cell given by its nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Element type; must be a typed kind (not `Polygon`/`Polyhedron`).
    pub kind: ElementKind,
    /// Node indices in the kind's local ordering.
    pub nodes: Vec<usize>,
}

/// One input boundary facet set for
/// [`UnstructuredMesh::from_elements`](super::UnstructuredMesh::from_elements).
///
/// Each facet is a corner-node list; it is matched to a mesh boundary face by
/// its **set** of nodes, so the winding given here does not matter.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundarySpec {
    /// Patch name.
    pub name: String,
    /// Patch type.
    pub kind: PatchKind,
    /// Corner-node lists of the facets in this patch.
    pub facets: Vec<Vec<usize>>,
}

/// One input face for
/// [`UnstructuredMesh::from_polyhedral`](super::UnstructuredMesh::from_polyhedral).
#[derive(Debug, Clone, PartialEq)]
pub struct PolyFace {
    /// Vertex loop, wound so the normal points out of `owner` (into
    /// `neighbour`), the OpenFOAM convention.
    pub verts: Vec<usize>,
    /// Owner cell.
    pub owner: usize,
    /// Neighbour cell, `None` on the boundary.
    pub neighbour: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    }
    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    /// Every local face of the 3-D reference elements is wound outward: its
    /// right-hand normal points away from the element centroid.
    #[test]
    fn face_tables_are_outward() {
        let tet = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let hex = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        for (kind, pts) in [(ElementKind::Tet4, &tet[..]), (ElementKind::Hex8, &hex[..])] {
            let n = pts.len() as f64;
            let c = pts.iter().fold([0.0; 3], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]);
            for f in kind.local_faces() {
                let nrm = cross(sub(pts[f[1]], pts[f[0]]), sub(pts[f[2]], pts[f[0]]));
                assert!(dot(nrm, sub(pts[f[0]], c)) > 0.0, "{kind:?} face {f:?} is inward");
            }
        }
    }

    #[test]
    fn units_scale_consistently() {
        assert_eq!(LengthUnit::Metre.cm_per_unit(), 100.0);
        assert_eq!(LengthUnit::Centimetre.metres_per_unit(), 0.01);
        assert!((LengthUnit::Millimetre.metres_per_unit() - 0.001).abs() < 1e-18);
    }
}
