// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f),
// the serial parts only:
//   meshLibrary/utilities/surfaceTools/meshSurfaceEngine/
//     meshSurfaceEngine.{C,H}, meshSurfaceEngineI.H
//     meshSurfaceEngineCalculateBoundaryNodesAndFaces.C
//       (calculateBoundaryFaces, calculateBoundaryOwners,
//        calculateBoundaryNodes, calculateBoundaryFacePatches,
//        calculatePointFaces, calculatePointPoints, calculatePointNormals,
//        calculateFaceNormals, calculateFaceCentres,
//        calculateEdgesAndAddressing, calculateFaceEdgesAddressing,
//        calculateEdgeFacesAddressing, calculateEdgePatchesAddressing,
//        calculateFaceFacesAddressing)
//     meshSurfaceEngineModifier.{C,H}
//       (moveBoundaryVertexNoUpdate, moveBoundaryVertex, updateGeometry)
//   meshLibrary/utilities/surfaceTools/meshSurfacePartitioner/
//     meshSurfacePartitioner.{C,H}, meshSurfacePartitionerFunctions.C
//       (calculateCornersEdgesAndAddressing)
// Not ported: meshSurfaceEngineParallelAddressing.C and every
// `Pstream::parRun()` branch (MPI), and the `activePatch` constructor
// (no caller in this crate).
// Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! **The mesh surface as one addressing object** (gh:#1257): cfMesh's
//! `meshSurfaceEngine` (serial) and `meshSurfacePartitioner`.
//!
//! Before this module, three stages each built their own boundary maps
//! (`layers::layer_setup`'s point → faces, `correct_edges`' edge → faces
//! and feature-edge counts, `surface_mapper`'s point → faces and patches);
//! they now read them from here.
//!
//! # Labels
//!
//! As upstream, three numberings appear:
//! - **mesh labels**: indices into [`VolumeMesh::points`] / `faces`;
//! - **boundary-face labels** `bf`: position in [`SurfaceEngine::boundary_faces`]
//!   (the boundary faces in ascending mesh-face order; upstream's
//!   `boundaryFaces()` sub-list, which is contiguous at the end of the face
//!   list there and need not be here);
//! - **boundary-point labels** `bp`: position in
//!   [`SurfaceEngine::boundary_points`], numbered in ascending mesh-point
//!   order (`calculateBoundaryNodes`); [`SurfaceEngine::bp`] maps back.
//!
//! [`SurfaceEngine::edges`] hold **mesh** point labels, as upstream's
//! `edgeList`; every other graph holds `bf` or `bp` labels.
//!
//! # Geometry
//!
//! Upstream computes everything on demand from a reference to the mesh. A
//! Rust struct without a lifetime cannot hold that reference, so the
//! addressing is built eagerly at construction and the engine owns copies
//! of the boundary face rings; the caller owns the points and passes them to
//! every geometry call ([`SurfaceEngine::move_boundary_vertex`],
//! [`SurfaceEngine::update_geometry`]: upstream's `meshSurfaceEngineModifier`).
//! - Face normals are **area vectors** (OpenFOAM `face::normal`, which the
//!   cfMesh sources call as the area-weighted normal), by Newell's sum, as
//!   [`VolumeMesh::face_area_vector`]: equal to OpenFOAM's triangle-fan sum
//!   analytically, not bit for bit.
//! - Face centres are OpenFOAM's `face::centre`
//!   ([`crate::correct_edges::foam_face_centre`]).
//! - Point normals are the unit sum of the area vectors of the point's
//!   faces, zero when that sum is below `VSMALL` (`calculatePointNormals`).

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::volume_mesh::VolumeMesh;
use std::collections::BTreeSet;

/// OpenFOAM `VSMALL` for `double`.
pub(crate) const VSMALL: f64 = 1.0e-300;

/// Marks a mesh point that is not on the boundary in [`SurfaceEngine::bp`]
/// (upstream's `-1`).
pub const NOT_BOUNDARY: usize = usize::MAX;

/// cfMesh `meshSurfaceEngine` (serial): the boundary of a mesh as faces,
/// points and edges with their addressing, and the geometry of each. See
/// the module documentation for the labels.
#[derive(Debug, Clone)]
pub struct SurfaceEngine {
    /// Mesh face of each boundary face.
    pub boundary_faces: Vec<usize>,
    /// The ring of each boundary face (mesh point labels), as in the mesh.
    pub face_rings: Vec<Vec<usize>>,
    /// Patch of each boundary face (`boundaryFacePatches`).
    pub face_patches: Vec<usize>,
    /// Owner cell of each boundary face (`faceOwners`).
    pub face_owners: Vec<usize>,
    /// Mesh point of each boundary point (`boundaryPoints`).
    pub boundary_points: Vec<usize>,
    /// Boundary point of each mesh point, [`NOT_BOUNDARY`] off the boundary
    /// (`bp`).
    pub bp: Vec<usize>,
    /// Boundary faces at each boundary point, in boundary-face order
    /// (`pointFaces`).
    pub point_faces: Vec<Vec<usize>>,
    /// Position of the point in each of those faces (`pointInFaces`).
    pub point_in_faces: Vec<Vec<usize>>,
    /// Boundary points joined to each boundary point by a boundary edge, in
    /// the order upstream appends them (`pointPoints`).
    pub point_points: Vec<Vec<usize>>,
    /// Boundary edges, mesh point labels; edge `[a, b]` was found at `a`,
    /// with `bp[b] > bp[a]` (`edges`).
    pub edges: Vec<[usize; 2]>,
    /// Edges at each boundary point, ascending (`boundaryPointEdges`).
    pub point_edges: Vec<Vec<usize>>,
    /// Boundary faces on each edge; with two, the first holds the edge as
    /// `a -> b` (`edgeFaces`).
    pub edge_faces: Vec<Vec<usize>>,
    /// Edge of each side of each boundary face, side `i` from ring point `i`
    /// to `i + 1` (`faceEdges`).
    pub face_edges: Vec<Vec<usize>>,
    /// Patches of each edge's faces (`edgePatches`).
    pub edge_patches: Vec<Vec<usize>>,
    /// Boundary faces across each two-face edge of each face, in edge order
    /// (`faceFaces`).
    pub face_faces: Vec<Vec<usize>>,
    /// Area vector of each boundary face (`faceNormals`).
    pub face_normals: Vec<Vec3>,
    /// OpenFOAM centre of each boundary face (`faceCentres`).
    pub face_centres: Vec<Vec3>,
    /// Unit normal of each boundary point (`pointNormals`).
    pub point_normals: Vec<Vec3>,
}

/// OpenFOAM `face::areaNormal` (OpenFOAM-v2506 `face.C`; cfMesh's
/// `face::normal`), operation for operation: a triangle is
/// `0.5*((p1 - p0)^(p2 - p0))`; any other face the sum of the triangle area
/// normals `(p_i, p_{i+1}, centre)` about the vertex average
/// (`centrePoint /= nPoints`, the centre last). gh:#1385: the earlier
/// `0.5 * sum(p_i x p_{i+1})` is the same vector up to round-off, and not
/// bit for bit.
pub fn ring_area_vector(points: &[Vec3], ring: &[usize]) -> Vec3 {
    let k = ring.len();
    let tri_area_normal = |p0: Vec3, p1: Vec3, p2: Vec3| p1.sub(p0).cross(p2.sub(p0)).scale(0.5);
    if k == 3 {
        return tri_area_normal(points[ring[0]], points[ring[1]], points[ring[2]]);
    }
    let centre = ring.iter().fold(Vec3::ZERO, |a, &v| a.add(points[v])).div(k as f64);
    let mut n = Vec3::ZERO;
    for i in 0..k {
        let next = points[ring[(i + 1) % k]];
        n = n.add(tri_area_normal(points[ring[i]], next, centre));
    }
    n
}

fn unit_or_zero(n: Vec3) -> Vec3 {
    let d = n.length();
    if d > VSMALL {
        n.div(d)
    } else {
        Vec3::ZERO
    }
}

impl SurfaceEngine {
    /// The surface of `mesh`, each boundary face in the patch whose range
    /// holds it ([`VolumeMesh::patches`]); a boundary face in no patch is
    /// given patch `mesh.patches.len()`.
    pub fn new(mesh: &VolumeMesh) -> Self {
        let mut patch_of = vec![mesh.patches.len(); mesh.face_count()];
        for (pi, p) in mesh.patches.iter().enumerate() {
            for f in p.start_face..p.start_face + p.n_faces {
                patch_of[f] = pi;
            }
        }
        Self::from_parts(&mesh.points, &mesh.faces, &mesh.owner, &mesh.neighbour, &patch_of)
    }

    /// The surface of a mesh given as parts, boundary face `f` (a face with
    /// no neighbour) in patch `patch_of_face[f]` (indexed by mesh face; the
    /// entries of internal faces are not read).
    pub fn from_parts(points: &[Vec3], faces: &[Vec<usize>], owner: &[usize], neighbour: &[Option<usize>], patch_of_face: &[usize]) -> Self {
        // calculateBoundaryFaces, calculateBoundaryOwners,
        // calculateBoundaryFacePatches.
        let boundary_faces: Vec<usize> = (0..faces.len()).filter(|&f| neighbour[f].is_none()).collect();
        let face_rings: Vec<Vec<usize>> = boundary_faces.iter().map(|&f| faces[f].clone()).collect();
        let face_patches: Vec<usize> = boundary_faces.iter().map(|&f| patch_of_face[f]).collect();
        let face_owners: Vec<usize> = boundary_faces.iter().map(|&f| owner[f]).collect();

        // calculateBoundaryNodes: numbered in ascending mesh-point order.
        let mut is_bnd = vec![false; points.len()];
        for ring in &face_rings {
            for &v in ring {
                is_bnd[v] = true;
            }
        }
        let mut bp = vec![NOT_BOUNDARY; points.len()];
        let mut boundary_points = Vec::new();
        for (v, &b) in is_bnd.iter().enumerate() {
            if b {
                bp[v] = boundary_points.len();
                boundary_points.push(v);
            }
        }
        let nbp = boundary_points.len();

        // calculatePointFaces (serial: faces in order).
        let mut point_faces = vec![Vec::new(); nbp];
        let mut point_in_faces = vec![Vec::new(); nbp];
        for (bf, ring) in face_rings.iter().enumerate() {
            for (pos, &v) in ring.iter().enumerate() {
                point_faces[bp[v]].push(bf);
                point_in_faces[bp[v]].push(pos);
            }
        }

        // calculatePointPoints: next then previous, appendIfNotIn.
        let mut point_points = vec![Vec::new(); nbp];
        for b in 0..nbp {
            let mut pp: Vec<usize> = Vec::new();
            for (&bf, &pos) in point_faces[b].iter().zip(&point_in_faces[b]) {
                let ring = &face_rings[bf];
                let k = ring.len();
                for q in [ring[(pos + 1) % k], ring[(pos + k - 1) % k]] {
                    if !pp.contains(&bp[q]) {
                        pp.push(bp[q]);
                    }
                }
            }
            point_points[b] = pp;
        }

        // calculateEdgesAndAddressing: at each point the edges to points of
        // no lower boundary label, sorted as std::set<pair<label, label>>.
        let mut edges: Vec<[usize; 2]> = Vec::new();
        for b in 0..nbp {
            let mut at: BTreeSet<(usize, usize)> = BTreeSet::new();
            for (&bf, &pos) in point_faces[b].iter().zip(&point_in_faces[b]) {
                let ring = &face_rings[bf];
                let k = ring.len();
                let (next, prev) = (ring[(pos + 1) % k], ring[(pos + k - 1) % k]);
                if bp[next] >= b {
                    at.insert((ring[pos], next));
                }
                if bp[prev] >= b {
                    at.insert((ring[pos], prev));
                }
            }
            edges.extend(at.into_iter().map(|(a, c)| [a, c]));
        }
        // reverseAddressing(bp, edges).
        let mut point_edges = vec![Vec::new(); nbp];
        for (e, &[a, c]) in edges.iter().enumerate() {
            point_edges[bp[a]].push(e);
            point_edges[bp[c]].push(e);
        }
        let same = |e: [usize; 2], a: usize, c: usize| (e[0] == a && e[1] == c) || (e[0] == c && e[1] == a);

        // calculateFaceEdgesAddressing.
        let face_edges: Vec<Vec<usize>> = face_rings
            .iter()
            .map(|ring| {
                let k = ring.len();
                (0..k)
                    .map(|i| {
                        let (a, c) = (ring[i], ring[(i + 1) % k]);
                        *point_edges[bp[a]].iter().find(|&&e| same(edges[e], a, c)).expect("every face side is a boundary edge")
                    })
                    .collect()
            })
            .collect();

        // calculateEdgeFacesAddressing: the faces at the edge's start point
        // holding it; with two, the one holding start -> end first.
        let edge_faces: Vec<Vec<usize>> = edges
            .iter()
            .map(|&[s, t]| {
                let mut ef: Vec<usize> = point_faces[bp[s]]
                    .iter()
                    .copied()
                    .filter(|&bf| {
                        let ring = &face_rings[bf];
                        let k = ring.len();
                        (0..k).any(|i| same([ring[i], ring[(i + 1) % k]], s, t))
                    })
                    .collect();
                if ef.len() == 2 {
                    let ring = &face_rings[ef[1]];
                    let k = ring.len();
                    let pos = ring.iter().position(|&q| q == s).unwrap();
                    if ring[(pos + 1) % k] == t {
                        ef.swap(0, 1);
                    }
                }
                ef
            })
            .collect();

        // calculateEdgePatchesAddressing.
        let edge_patches: Vec<Vec<usize>> = edge_faces
            .iter()
            .map(|ef| {
                let mut ps: Vec<usize> = Vec::new();
                for &bf in ef {
                    if !ps.contains(&face_patches[bf]) {
                        ps.push(face_patches[bf]);
                    }
                }
                ps
            })
            .collect();

        // calculateFaceFacesAddressing (serial: only two-face edges).
        let mut face_faces = vec![Vec::new(); face_rings.len()];
        for ef in &edge_faces {
            if ef.len() == 2 {
                face_faces[ef[0]].push(ef[1]);
                face_faces[ef[1]].push(ef[0]);
            }
        }

        let mut engine = SurfaceEngine {
            boundary_faces,
            face_rings,
            face_patches,
            face_owners,
            boundary_points,
            bp,
            point_faces,
            point_in_faces,
            point_points,
            edges,
            point_edges,
            edge_faces,
            face_edges,
            edge_patches,
            face_faces,
            face_normals: Vec::new(),
            face_centres: Vec::new(),
            point_normals: Vec::new(),
        };
        engine.face_normals = vec![Vec3::ZERO; engine.face_rings.len()];
        engine.face_centres = vec![Vec3::ZERO; engine.face_rings.len()];
        engine.point_normals = vec![Vec3::ZERO; engine.boundary_points.len()];
        engine.update_geometry_all(points);
        engine
    }

    /// The boundary edge joining mesh points `a` and `b`, if there is one
    /// (a search of `boundaryPointEdges`, as upstream's callers do).
    pub fn find_edge(&self, a: usize, b: usize) -> Option<usize> {
        let ba = *self.bp.get(a)?;
        if ba == NOT_BOUNDARY {
            return None;
        }
        self.point_edges[ba].iter().copied().find(|&e| {
            let [s, t] = self.edges[e];
            (s == a && t == b) || (s == b && t == a)
        })
    }

    /// Number of boundary points.
    pub fn n_boundary_points(&self) -> usize {
        self.boundary_points.len()
    }

    /// Number of boundary faces.
    pub fn n_boundary_faces(&self) -> usize {
        self.face_rings.len()
    }

    /// The point normal of boundary point `b` from the current face
    /// normals (`calculatePointNormals`).
    fn point_normal(&self, b: usize) -> Vec3 {
        unit_or_zero(self.point_faces[b].iter().fold(Vec3::ZERO, |n, &bf| n.add(self.face_normals[bf])))
    }

    /// `meshSurfaceEngineModifier::moveBoundaryVertexNoUpdate`: move
    /// boundary point `b` to `p`, leaving the engine's geometry stale.
    pub fn move_boundary_vertex_no_update(&self, points: &mut [Vec3], b: usize, p: Vec3) {
        points[self.boundary_points[b]] = p;
    }

    /// `meshSurfaceEngineModifier::moveBoundaryVertex`: move boundary point
    /// `b` to `p` and update the centres and normals of its faces, its own
    /// normal and those of its `pointPoints`.
    pub fn move_boundary_vertex(&mut self, points: &mut [Vec3], b: usize, p: Vec3) {
        points[self.boundary_points[b]] = p;
        for i in 0..self.point_faces[b].len() {
            let bf = self.point_faces[b][i];
            self.face_centres[bf] = foam_face_centre(points, &self.face_rings[bf]);
            self.face_normals[bf] = ring_area_vector(points, &self.face_rings[bf]);
        }
        self.point_normals[b] = self.point_normal(b);
        for i in 0..self.point_points[b].len() {
            let q = self.point_points[b][i];
            self.point_normals[q] = self.point_normal(q);
        }
    }

    /// `meshSurfaceEngineModifier::updateGeometry(updateBndNodes)`: the
    /// centres and normals of every face at a point of `moved` (boundary
    /// labels), then the normals of every point of those faces.
    pub fn update_geometry(&mut self, points: &[Vec3], moved: &[usize]) {
        let mut update_face = vec![false; self.face_rings.len()];
        for &b in moved {
            for &bf in &self.point_faces[b] {
                update_face[bf] = true;
            }
        }
        for bf in 0..self.face_rings.len() {
            if update_face[bf] {
                self.face_centres[bf] = foam_face_centre(points, &self.face_rings[bf]);
                self.face_normals[bf] = ring_area_vector(points, &self.face_rings[bf]);
            }
        }
        let mut update_point = vec![false; self.boundary_points.len()];
        for &b in moved {
            for &bf in &self.point_faces[b] {
                for &v in &self.face_rings[bf] {
                    update_point[self.bp[v]] = true;
                }
            }
        }
        for b in 0..self.boundary_points.len() {
            if update_point[b] {
                self.point_normals[b] = self.point_normal(b);
            }
        }
    }

    /// `meshSurfaceEngineModifier::updateGeometry()`: every boundary point.
    pub fn update_geometry_all(&mut self, points: &[Vec3]) {
        let all: Vec<usize> = (0..self.boundary_points.len()).collect();
        self.update_geometry(points, &all);
    }
}

/// cfMesh `meshSurfacePartitioner` (serial): the surface split into
/// patches, with its feature edges (edges between two faces in different
/// patches), corners (boundary points with more than two feature edges)
/// and edge points (exactly two).
#[derive(Debug, Clone)]
pub struct SurfacePartitioner {
    /// Patch of each boundary face: the engine's, or the one given.
    pub face_patches: Vec<usize>,
    /// Patches at each boundary point, in `pointFaces` order
    /// (`pointPatches`).
    pub point_patches: Vec<Vec<usize>>,
    /// Corner boundary points (`corners`).
    pub corners: BTreeSet<usize>,
    /// Edge boundary points (`edgePoints`).
    pub edge_points: BTreeSet<usize>,
    /// Patches sharing a feature edge with each patch (`patchPatches`).
    pub patch_patches: Vec<BTreeSet<usize>>,
    /// Feature edges at each boundary point
    /// (`numberOfFeatureEdgesAtPoint`).
    pub n_edges_at_point: Vec<usize>,
    /// Feature edges, engine edge labels (`featureEdges`).
    pub feature_edges: BTreeSet<usize>,
}

impl SurfacePartitioner {
    /// Partition by the engine's own face patches.
    pub fn new(engine: &SurfaceEngine) -> Self {
        Self::with_face_patches(engine, engine.face_patches.clone())
    }

    /// Partition by `face_patches` (one per boundary face), upstream's
    /// `meshSurfacePartitioner(meshSurface, facePatch)`.
    pub fn with_face_patches(engine: &SurfaceEngine, face_patches: Vec<usize>) -> Self {
        let n_patches = face_patches.iter().copied().max().map_or(1, |m| m + 1);
        let mut patch_patches = vec![BTreeSet::new(); n_patches];
        let mut n_edges_at_point = vec![0usize; engine.boundary_points.len()];
        let mut feature_edges = BTreeSet::new();
        for (e, ef) in engine.edge_faces.iter().enumerate() {
            if ef.len() != 2 {
                continue;
            }
            let (p0, p1) = (face_patches[ef[0]], face_patches[ef[1]]);
            if p0 != p1 {
                let [a, b] = engine.edges[e];
                n_edges_at_point[engine.bp[a]] += 1;
                n_edges_at_point[engine.bp[b]] += 1;
                patch_patches[p0].insert(p1);
                patch_patches[p1].insert(p0);
                feature_edges.insert(e);
            }
        }
        let mut corners = BTreeSet::new();
        let mut edge_points = BTreeSet::new();
        for (b, &n) in n_edges_at_point.iter().enumerate() {
            if n > 2 {
                corners.insert(b);
            } else if n == 2 {
                edge_points.insert(b);
            }
        }
        let point_patches = engine
            .point_faces
            .iter()
            .map(|pf| {
                let mut ps: Vec<usize> = Vec::new();
                for &bf in pf {
                    if !ps.contains(&face_patches[bf]) {
                        ps.push(face_patches[bf]);
                    }
                }
                ps
            })
            .collect();
        SurfacePartitioner { face_patches, point_patches, corners, edge_points, patch_patches, n_edges_at_point, feature_edges }
    }

    /// The engine's per-boundary-face patches from a per-mesh-face
    /// assignment (`None` for internal faces), as the crate's callers hold
    /// it; a boundary face with `None` gets `unassigned`.
    pub fn face_patches_from_mesh_faces(engine: &SurfaceEngine, face_region: &[Option<usize>], unassigned: usize) -> Vec<usize> {
        engine.boundary_faces.iter().map(|&f| face_region[f].unwrap_or(unassigned)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;
    use std::collections::BTreeMap;

    /// Patch of each boundary face of a box by its outward axis: 0..6.
    fn axis_patches(m: &VolumeMesh, e: &SurfaceEngine) -> Vec<usize> {
        e.boundary_faces
            .iter()
            .map(|&f| {
                let s = m.face_area_vector(f);
                let ax = [s.x.abs(), s.y.abs(), s.z.abs()];
                let i = (0..3).max_by(|&a, &b| ax[a].partial_cmp(&ax[b]).unwrap()).unwrap();
                2 * i + usize::from([s.x, s.y, s.z][i] > 0.0)
            })
            .collect()
    }

    /// V&V (gh:#1257) — the engine on a carved 4×4×4 box, against counts
    /// worked out by hand before the first run (written 2026-11-05):
    /// `carve_box(box_surface(0, 2), 0.5)` is 64 hexahedra; its boundary has
    /// 6 × 16 = 96 quads, 5³ − 3³ = 98 points, 96 × 4 / 2 = 192 edges
    /// (Euler: 98 − 192 + 96 = 2); every edge has two faces, the one holding
    /// it start → end first; with one patch per side there are 12 × 4 = 48
    /// feature edges, 8 corners (three each) and 12 × 3 = 36 edge points;
    /// every point off a box edge has the side's axis normal to 1e-12, a
    /// corner the normalised (±1, ±1, ±1) (equal-area faces).
    #[test]
    fn engine_and_partitioner_on_a_carved_box_match_hand_counts() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let m = carve_box(&p, &t, 0.5);
        assert_eq!(m.cell_count(), 64);
        let e = SurfaceEngine::new(&m);
        assert_eq!(e.n_boundary_faces(), 96);
        assert_eq!(e.n_boundary_points(), 98);
        assert_eq!(e.edges.len(), 192);
        assert_eq!(e.n_boundary_points() + e.n_boundary_faces(), e.edges.len() + 2);
        assert!(e.face_patches.iter().all(|&q| q == 0));
        for (ed, ef) in e.edge_faces.iter().enumerate() {
            assert_eq!(ef.len(), 2);
            let [s, t] = e.edges[ed];
            let ring = &e.face_rings[ef[0]];
            let pos = ring.iter().position(|&q| q == s).unwrap();
            assert_eq!(ring[(pos + 1) % ring.len()], t, "edge {ed}: first face holds start -> end");
        }
        for (bf, fe) in e.face_edges.iter().enumerate() {
            assert_eq!(fe.len(), 4);
            for &ed in fe {
                assert!(e.edge_faces[ed].contains(&bf));
            }
            assert_eq!(e.face_faces[bf].len(), 4);
        }
        for b in 0..e.n_boundary_points() {
            assert_eq!(e.point_in_faces[b].len(), e.point_faces[b].len());
            for (&bf, &pos) in e.point_faces[b].iter().zip(&e.point_in_faces[b]) {
                assert_eq!(e.face_rings[bf][pos], e.boundary_points[b]);
            }
            assert_eq!(e.point_points[b].len(), e.point_edges[b].len());
        }
        // One patch: no feature edge.
        let one = SurfacePartitioner::new(&e);
        assert!(one.feature_edges.is_empty() && one.corners.is_empty() && one.edge_points.is_empty());

        let part = SurfacePartitioner::with_face_patches(&e, axis_patches(&m, &e));
        assert_eq!(part.feature_edges.len(), 48);
        assert_eq!(part.corners.len(), 8);
        assert_eq!(part.edge_points.len(), 36);
        assert!(part.corners.iter().all(|&b| part.n_edges_at_point[b] == 3 && part.point_patches[b].len() == 3));
        assert!(part.patch_patches.iter().all(|s| s.len() == 4));
        for b in 0..e.n_boundary_points() {
            let n = e.point_normals[b];
            let x = m.points[e.boundary_points[b]];
            let on = [x.x, x.y, x.z].map(|c| c.abs() < 1e-12 || (c - 2.0).abs() < 1e-12);
            let n_on = on.iter().filter(|&&o| o).count();
            if n_on == 1 {
                let i = on.iter().position(|&o| o).unwrap();
                let want = if [x.x, x.y, x.z][i] < 1.0 { -1.0 } else { 1.0 };
                let got = [n.x, n.y, n.z];
                for (j, &g) in got.iter().enumerate() {
                    let w = if j == i { want } else { 0.0 };
                    assert!((g - w).abs() < 1e-12, "point {x:?} normal {n:?}");
                }
            } else if n_on == 3 {
                let s = 1.0 / 3f64.sqrt();
                for (c, g) in [x.x, x.y, x.z].iter().zip([n.x, n.y, n.z]) {
                    let w = if *c < 1.0 { -s } else { s };
                    assert!((g - w).abs() < 1e-12, "corner {x:?} normal {n:?}");
                }
            }
        }
    }

    /// The modifier: moving one boundary point updates its faces' centres
    /// and normals and its own and its `pointPoints`' normals exactly as a
    /// fresh engine on the moved mesh would; `update_geometry` after a
    /// no-update move matches the fresh engine everywhere. First run
    /// (2026-11-05) asserted the same of `moveBoundaryVertex` at every
    /// point and failed: upstream's `moveBoundaryVertex` leaves the normal
    /// of a point that shares a face but no edge with the moved one (the
    /// diagonal of a quad) stale, and so does the port; pinned below.
    #[test]
    fn moving_a_vertex_updates_geometry_like_a_fresh_engine() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let mut m = carve_box(&p, &t, 0.5);
        let mut e = SurfaceEngine::new(&m);
        let b = (0..e.n_boundary_points()).find(|&b| e.point_faces[b].len() == 4).unwrap();
        let q = m.points[e.boundary_points[b]].add(Vec3::new(0.05, -0.03, 0.02));
        let mut e2 = e.clone();
        let mut pts2 = m.points.clone();
        e.move_boundary_vertex(&mut m.points, b, q);
        e2.move_boundary_vertex_no_update(&mut pts2, b, q);
        e2.update_geometry(&pts2, &[b]);
        let fresh = SurfaceEngine::new(&m);
        for bf in 0..e.n_boundary_faces() {
            assert_eq!(e.face_centres[bf], fresh.face_centres[bf]);
            assert_eq!(e.face_normals[bf], fresh.face_normals[bf]);
            assert_eq!(e2.face_centres[bf], fresh.face_centres[bf]);
        }
        let mut n_stale = 0;
        for c in 0..e.n_boundary_points() {
            assert_eq!(e2.point_normals[c], fresh.point_normals[c], "point {c}");
            if c == b || e.point_points[b].contains(&c) {
                assert_eq!(e.point_normals[c], fresh.point_normals[c], "point {c}");
            } else if e.point_normals[c] != fresh.point_normals[c] {
                n_stale += 1;
            }
        }
        // The four quad diagonals of an interior point of a side.
        assert_eq!(n_stale, 4);
    }

    /// Differential (gh:#1257), before the callers move onto the engine:
    /// on the 48-sided pipe meshed by the default pipeline (h 5 mm, bare),
    /// the engine's `pointFaces` against the map `layers::layer_setup` built
    /// (point → boundary faces, in face order), and the partitioner's
    /// feature edges, their count per point, corners and edge points against
    /// the maps `correct_edges` built (sorted edge → boundary faces, feature
    /// when two faces in different patches). Predicted (2026-11-05, before
    /// the run): identical, element for element, since both follow the
    /// same upstream definitions in face order.
    #[test]
    fn engine_matches_the_crates_earlier_boundary_maps_on_the_pipe() {
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.06, 48);
        let o = crate::pipeline::TetDualOptions { cell_size: 0.005, n_layers: 0, ..Default::default() };
        let (m, _) = crate::pipeline::surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        let e = SurfaceEngine::new(&m);
        // layer_setup's map.
        let mut patch_of = vec![usize::MAX; m.face_count()];
        for (pi, q) in m.patches.iter().enumerate() {
            for f in q.start_face..q.start_face + q.n_faces {
                patch_of[f] = pi;
            }
        }
        let mut point_faces: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for f in 0..m.face_count() {
            if patch_of[f] != usize::MAX {
                for &v in &m.faces[f] {
                    point_faces.entry(v).or_default().push(f);
                }
            }
        }
        assert_eq!(point_faces.len(), e.n_boundary_points());
        for (b, &v) in e.boundary_points.iter().enumerate() {
            let mine: Vec<usize> = e.point_faces[b].iter().map(|&bf| e.boundary_faces[bf]).collect();
            assert_eq!(mine, point_faces[&v]);
        }
        // correct_edges' maps.
        let mut ef: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for f in 0..m.face_count() {
            if m.neighbour[f].is_none() {
                let ring = &m.faces[f];
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    ef.entry((a.min(b), a.max(b))).or_default().push(f);
                }
            }
        }
        let mut n_feat: BTreeMap<usize, usize> = BTreeMap::new();
        let mut feature: BTreeSet<(usize, usize)> = BTreeSet::new();
        for (&k, fs) in &ef {
            if fs.len() == 2 && patch_of[fs[0]] != patch_of[fs[1]] {
                feature.insert(k);
                *n_feat.entry(k.0).or_default() += 1;
                *n_feat.entry(k.1).or_default() += 1;
            }
        }
        let part = SurfacePartitioner::new(&e);
        let mine: BTreeSet<(usize, usize)> = part.feature_edges.iter().map(|&ed| {
            let [a, b] = e.edges[ed];
            (a.min(b), a.max(b))
        }).collect();
        assert_eq!(e.edges.len(), ef.len());
        assert_eq!(mine, feature);
        let corners: BTreeSet<usize> = n_feat.iter().filter(|&(_, &n)| n > 2).map(|(&v, _)| e.bp[v]).collect();
        let edge_pts: BTreeSet<usize> = n_feat.iter().filter(|&(_, &n)| n == 2).map(|(&v, _)| e.bp[v]).collect();
        assert_eq!(part.corners, corners);
        assert_eq!(part.edge_points, edge_pts);
        println!(
            "pipe h 5 mm: {} boundary points, {} faces, {} edges, {} feature edges, {} corners, {} edge points",
            e.n_boundary_points(),
            e.n_boundary_faces(),
            e.edges.len(),
            part.feature_edges.len(),
            part.corners.len(),
            part.edge_points.len()
        );
    }
}
