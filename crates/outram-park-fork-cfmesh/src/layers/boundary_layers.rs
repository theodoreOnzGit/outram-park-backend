// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f):
//   meshLibrary/utilities/boundaryLayers/boundaryLayers/
//     boundaryLayers.{C,H}              (addLayerForPatch, the constructor's
//                                        blHeightFactor_ 0.5, blMinHeight_ 0)
//     boundaryLayersCreateVertices.C    (findPatchVertices, createNewVertex,
//                                        createNewVertices(boolList))
//     boundaryLayersFacesAndCells.C     (createNewFacesAndCells)
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

//! **cfMesh's `boundaryLayers` creation route** (gh:#1270): one layer of
//! cells on the named patches, as `boundaryLayers::addLayerForPatch` builds
//! it. Opt-in, beside the crate's three layer strategies
//! ([`super::LayerQualityStrategy`]); nothing calls it by default.
//!
//! # What upstream does, and what this port reuses
//!
//! Upstream creates one new point per boundary point of the treated
//! patches (`createNewVertex`), then **swaps** the two: the original
//! point moves to the new position (so the cells it belongs to shrink) and
//! the new label takes the old position on the boundary. Each treated face
//! then gets one cell: its face (now internal), the face of new labels (the
//! boundary), and one quad per edge (`createNewFacesAndCells`); a quad
//! whose edge borders an untreated face becomes a boundary face of that
//! face's patch, so the untreated patch is re-partitioned, its points
//! sliding in its plane.
//!
//! That is a column of one layer per wall point, from the old position to
//! the new one, which is what [`super::build_layered_variable`] assembles:
//! this route uses it (and [`super::layer_setup`]'s classification of the
//! wall points into `PATCHNODE`, `EDGENODE` with one or two other patches,
//! and its refusal of three) and adds only upstream's **point placement**:
//!
//! - **`PATCHNODE`** (only treated patches at the point): along the unit
//!   point normal of the surface engine (`pointNormals`), inward, by
//!   `blHeightFactor` (0.5) x the shortest distance to a neighbouring
//!   boundary point (`pointPoints`).
//! - **`EDGENODE`, one other patch:** along the treated faces' area-vector
//!   sum with its component along the other patch's unit normal sum
//!   removed (so the point stays in that patch's plane), by 0.5 x the
//!   smallest `|(q − p)·n|` over neighbours on no treated patch; then, for
//!   each face of the other patch at the point, if the distance `dst` to
//!   the segment between the point's two neighbours in that face is below
//!   the current distance, the distance becomes `0.9 dst` (upstream's
//!   test is `dst < dist`, its value `0.9 dst`; ported as written).
//! - **`EDGENODE`, two other patches:** towards the first neighbouring
//!   boundary point that lies on both (`pointPoints` order), half way
//!   (the normal is `(p − q) / (2 · 0.5 |p − q|)`, so the move is half the
//!   edge for any height factor; ported as written), then the same 0.9 cap.
//! - Every distance is at least `max(blMinHeight, VSMALL)`; a NaN or
//!   infinite position leaves the point where it is.
//!
//! Where this differs from the crate's [`super::add_boundary_layers_adaptive_on_patches`]:
//! that route takes the thickness from the request (capped at 0.5 x the
//! shortest wall edge at the point), smooths the interior normals and backs
//! off globally until the mesh passes upstream's quality limits; this one
//! places one layer by the local edge length, along the unsmoothed point
//! normal, with no quality check. Split it into several layers with
//! [`super::refine_boundary_layers`].
//!
//! **Numbering, not geometry, differs from upstream:** the columns are
//! numbered as [`super::layer_columns`] numbers them (non-wall points
//! first), where upstream appends the boundary copies at the end; and a
//! point's column runs from the old position `x` to `x + |d| (d/|d|)`
//! rather than `x + d`, equal to round-off. Patch-wise layers at feature
//! edges between treated patches of different groups
//! (`findPatchesToBeTreatedTogether`, `createNewVertices(labelList)`,
//! `createLayerCells`, `otherVrts_`), `addWrapperLayer`,
//! `checkTopologyOfBoundaryFaces`, the 2D mode and the MPI branches are
//! not ported: this route treats all named patches as one group, which is
//! upstream's `addLayerForPatch` when they are (one patch, or patches
//! meeting at convex edges under 135°).

use super::{build_layered_variable_with_columns, distance_to_segment, layer_setup};
use crate::math::Vec3;
use crate::surface_engine::{SurfaceEngine, SurfacePartitioner, VSMALL};
use crate::surface_optimizer::check_inverted_vertices;
use crate::topology::TopoMesh;
use crate::volume_mesh::VolumeMesh;
use std::collections::{BTreeMap, BTreeSet};
use petir::mathf::RealMath;

/// `boundaryLayers` settings (the `boundaryLayers` sub-dictionary of
/// `meshDict` this upstream line reads).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryLayersControls {
    /// `boundaryLayerHeightFactor` (default 0.5: half the distance to the
    /// nearest neighbour).
    pub height_factor: f64,
    /// `boundaryLayerMinHeight` [m] (default 0: no minimum).
    pub min_height: f64,
}

impl Default for BoundaryLayersControls {
    fn default() -> Self {
        BoundaryLayersControls { height_factor: 0.5, min_height: 0.0 }
    }
}

/// One wall point of a [`boundary_layers_add_layer`] layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPoint {
    /// Mesh point (in the input mesh).
    pub point: usize,
    /// Position before (on the wall).
    pub wall: Vec3,
    /// Position upstream computes (`p − dist · normal`).
    pub moved: Vec3,
    /// Unit direction of the column (`moved − wall`, normalised; zero for
    /// a point that does not move).
    pub direction: Vec3,
    /// Column length [m]: the column's top is `wall + direction · length`.
    pub length: f64,
    /// Number of untreated patches at the point (0: `PATCHNODE`; 1 or 2:
    /// `EDGENODE`).
    pub n_other_patches: usize,
}

impl LayerPoint {
    /// Column length [m].
    pub fn thickness(&self) -> f64 {
        self.length
    }
}

/// What [`boundary_layers_add_layer`] built.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryLayersOutcome {
    /// Every wall point, ascending.
    pub points: Vec<LayerPoint>,
    /// Treated boundary faces (one layer cell each), input mesh labels.
    pub wall_faces: Vec<usize>,
    /// Cell of the output mesh owning each layer: the layer cells are
    /// the last `wall_faces.len()` cells, in wall-face order.
    pub first_layer_cell: usize,
}

/// cfMesh `boundaryLayers::addLayerForPatch` on the named patches, treated
/// as one group (see the module documentation). Returns `None` where
/// upstream would stop: no treated face, or a point that touches three or
/// more untreated patches, or two with no shared neighbour. The output
/// keeps the input's patch names, each face in the patch upstream gives it.
pub fn boundary_layers_add_layer(mesh: &VolumeMesh, layer_patches: &[&str], c: &BoundaryLayersControls) -> Option<(VolumeMesh, BoundaryLayersOutcome)> {
    let s = layer_setup(mesh, layer_patches, 1, 1.0, 1.0)?;
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    let treated: Vec<bool> = (0..=mesh.patches.len()).map(|pi| pi < mesh.patches.len() && layer_patches.contains(&mesh.patches[pi].name.as_str())).collect();
    let patch_vertex = find_patch_vertices(&part, &treated);

    let mut inward: BTreeMap<usize, Vec3> = BTreeMap::new();
    let mut totals: BTreeMap<usize, f64> = BTreeMap::new();
    let mut out_points = Vec::new();
    for &wp in &s.wall_pts {
        let b = engine.bp[wp];
        let p = mesh.points[wp];
        let (new_p, n_other) = create_new_vertex(&engine, &part, &mesh.points, b, &treated, &patch_vertex, c).ok()?;
        let disp = new_p.sub(p);
        let len = disp.length();
        inward.insert(wp, if len > 0.0 { disp.scale(1.0 / len) } else { Vec3::ZERO });
        totals.insert(wp, len);
        out_points.push(LayerPoint { point: wp, wall: p, moved: new_p, direction: inward[&wp], length: len, n_other_patches: n_other });
    }

    let counts: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&wp| (wp, 1)).collect();
    let (layered, column) = build_layered_variable_with_columns(mesh, &s.wall_faces, &s.wall_pts, &inward, &[0.0, 1.0], &totals, &counts);

    // createNewFacesAndCells' patches: a treated face's copy keeps its
    // patch; a side quad on an edge to an untreated face takes that face's
    // patch; every other boundary face keeps its own (its wall points now
    // the tops of their columns).
    let key = |ring: &[usize]| -> Vec<usize> {
        let mut k = ring.to_vec();
        k.sort_unstable();
        k
    };
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    let mut next = 0usize;
    for (op, slot) in new_of.iter_mut().enumerate() {
        if !s.wall_pts.contains(&op) {
            *slot = next;
            next += 1;
        }
    }
    let top = |p: usize| if s.wall_pts.contains(&p) { *column[&p].last().unwrap() } else { new_of[p] };
    let mut patch_of_key: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    for bf in 0..engine.boundary_faces.len() {
        let pi = engine.face_patches[bf];
        let ring = &engine.face_rings[bf];
        if treated[pi] {
            patch_of_key.insert(key(&ring.iter().map(|&p| column[&p][0]).collect::<Vec<_>>()), pi);
            let k = ring.len();
            for i in 0..k {
                let e = engine.face_edges[bf][i];
                let ef = &engine.edge_faces[e];
                if ef.len() != 2 {
                    continue;
                }
                let nei = if ef[0] == bf { ef[1] } else { ef[0] };
                if !treated[engine.face_patches[nei]] {
                    let (a, b2) = (ring[i], ring[(i + 1) % k]);
                    patch_of_key.insert(key(&[column[&a][0], column[&b2][0], column[&b2][1], column[&a][1]]), engine.face_patches[nei]);
                }
            }
        } else {
            patch_of_key.insert(key(&ring.iter().map(|&p| top(p)).collect::<Vec<_>>()), pi);
        }
    }
    let mut w = TopoMesh::from_volume_mesh(&layered);
    w.patch_names = mesh.patches.iter().map(|p| p.name.clone()).collect();
    for f in 0..w.faces.len() {
        w.region[f] = if w.neighbour[f].is_none() { patch_of_key.get(&key(&w.faces[f])).copied() } else { None };
    }
    let (out, _) = w.into_volume_mesh();
    let outcome = BoundaryLayersOutcome { points: out_points, wall_faces: s.wall_faces.clone(), first_layer_cell: mesh.n_cells };
    Some((out, outcome))
}

/// `findPatchVertices` flag: the point is on a treated patch.
pub const PATCHNODE: u8 = 1;
/// `findPatchVertices` flag: the point is also on an untreated patch.
pub const EDGENODE: u8 = 2;

/// Where upstream's `boundaryLayers` stops with `FatalError` (serial
/// branches only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryLayersError {
    /// `createNewVertex`: "There are more than 3 patches meeting at this
    /// vertex!" (three or more untreated patches at a treated point).
    MoreThanThreePatches {
        /// Mesh point (`usize::MAX`: the one-group route refused).
        point: usize,
    },
    /// `createNewVertex`: "Cannot find moving vertex!" (two untreated
    /// patches with no neighbour on both), or "Face does not contains this
    /// vertex!".
    CannotFindMovingVertex {
        /// Mesh point.
        point: usize,
    },
    /// `createNewVertices(labelList)`: "Boundary node is not at an edge!"
    /// (a point with layer vertices from more than three groups).
    NotAtAnEdge {
        /// Mesh point.
        point: usize,
        /// Groups at the point.
        n_groups: usize,
    },
    /// `checkTopologyOfBoundaryFaces`: a face was cut and its cell would be
    /// decomposed; upstream stops there with "Critical. Not tested".
    CriticalNotTested {
        /// Cells upstream would decompose.
        n_decomposed: usize,
    },
    /// The patch named in `addLayerForPatch` is not a patch of the mesh
    /// (upstream skips it silently; reported here).
    UnknownPatch(String),
}

/// cfMesh `findPatchVertices`: per boundary point, [`PATCHNODE`] if any of
/// its patches is treated, plus [`EDGENODE`] if any is not (the
/// `PARALLELBOUNDARY` flag is MPI only).
pub fn find_patch_vertices(part: &SurfacePartitioner, treat: &[bool]) -> Vec<u8> {
    let treated = |q: usize| treat.get(q).copied().unwrap_or(false);
    part.point_patches
        .iter()
        .map(|pp| {
            if pp.iter().any(|&q| treated(q)) {
                PATCHNODE | if pp.iter().any(|&q| !treated(q)) { EDGENODE } else { 0 }
            } else {
                0
            }
        })
        .collect()
}

/// cfMesh `createNewVertex` for boundary point `b` (the rules in the module
/// documentation), on `points` as they are before any layer point is
/// moved. Returns the new position and the number of untreated patches at
/// the point (0, 1 or 2). The one copy both routes use.
pub fn create_new_vertex(
    engine: &SurfaceEngine,
    part: &SurfacePartitioner,
    points: &[Vec3],
    b: usize,
    treat: &[bool],
    patch_vertex: &[u8],
    c: &BoundaryLayersControls,
) -> Result<(Vec3, usize), BoundaryLayersError> {
    let wp = engine.boundary_points[b];
    let p = points[wp];
    let treated = |pi: usize| treat.get(pi).copied().unwrap_or(false);
    let mut others: Vec<usize> = Vec::new();
    if patch_vertex[b] & EDGENODE != 0 {
        for &q in &part.point_patches[b] {
            if !treated(q) && !others.contains(&q) {
                others.push(q);
            }
        }
    }
    let (normal, mut dist) = match others.len() {
        0 => {
            let mut dist = f64::MAX;
            for &q in &engine.point_points[b] {
                dist = dist.min(c.height_factor * points[engine.boundary_points[q]].sub(p).length());
            }
            (engine.point_normals[b], dist)
        }
        1 => {
            let (mut n, mut v) = (Vec3::ZERO, Vec3::ZERO);
            for &bf in &engine.point_faces[b] {
                if treated(engine.face_patches[bf]) {
                    n = n.add(engine.face_normals[bf]);
                } else {
                    v = v.add(engine.face_normals[bf]);
                }
            }
            let v = v.scale(1.0 / (v.length() + VSMALL));
            let n = n.sub(v.scale(n.dot(v)));
            let n = n.scale(1.0 / (n.length() + VSMALL));
            let mut dist = f64::MAX;
            for &q in &engine.point_points[b] {
                if patch_vertex[q] != 0 {
                    continue;
                }
                dist = dist.min(c.height_factor * points[engine.boundary_points[q]].sub(p).dot(n).abs());
            }
            (n, dist)
        }
        2 => {
            let q = *engine.point_points[b].iter().find(|&&q| others.iter().all(|o| part.point_patches[q].contains(o))).ok_or(BoundaryLayersError::CannotFindMovingVertex { point: wp })?;
            let d = p.sub(points[engine.boundary_points[q]]);
            let dist = c.height_factor * d.length() + VSMALL;
            (d.scale(1.0 / (2.0 * dist)), dist)
        }
        _ => return Err(BoundaryLayersError::MoreThanThreePatches { point: wp }),
    };
    if !others.is_empty() {
        // limit distances
        for &bf in &engine.point_faces[b] {
            if !others.contains(&engine.face_patches[bf]) {
                continue;
            }
            let ring = &engine.face_rings[bf];
            let k = ring.len();
            let pos = ring.iter().position(|&x| x == wp).ok_or(BoundaryLayersError::CannotFindMovingVertex { point: wp })?;
            let (e1, e2) = (points[ring[(pos + k - 1) % k]], points[ring[(pos + 1) % k]]);
            let dst = distance_to_segment(p, e1, e2);
            if dst < dist {
                dist = 0.9 * dst;
            }
        }
    }
    dist = dist.max(c.min_height.max(VSMALL));
    let new_p = p.sub(normal.scale(dist));
    let new_p = if new_p.x.is_finite() && new_p.y.is_finite() && new_p.z.is_finite() { new_p } else { p };
    Ok((new_p, others.len()))
}

/// OpenFOAM `tetrahedron::mag()`: the signed volume of `(a, b, c, d)`.
fn tet_mag(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    b.sub(a).cross(c.sub(a)).dot(d.sub(a)) / 6.0
}

/// For each point of `f1` that `f2` shares: `f1`'s and `f2`'s corner
/// triangles at the point (the point, its next and its previous) and the
/// signed volume of the tet of `f2`'s triangle with the first point of
/// `f1`'s that `f2`'s lacks (0 if none). The common body of
/// `help::isSharedEdgeConvex` and `help::angleBetweenFaces`.
fn shared_point_tets(points: &[Vec3], f1: &[usize], f2: &[usize]) -> Vec<([usize; 3], [usize; 3], f64)> {
    let (k1, k2) = (f1.len(), f2.len());
    let mut out = Vec::new();
    for (pi, &v) in f1.iter().enumerate() {
        let Some(pos) = f2.iter().position(|&q| q == v) else { continue };
        let tri_nei = [f2[pos], f2[(pos + 1) % k2], f2[(pos + k2 - 1) % k2]];
        let tri_own = [f1[pi], f1[(pi + 1) % k1], f1[(pi + k1 - 1) % k1]];
        let mut vol = 0.0;
        for &o in &tri_own {
            if !tri_nei.contains(&o) {
                vol = tet_mag(points[tri_nei[0]], points[tri_nei[1]], points[tri_nei[2]], points[o]);
                break;
            }
        }
        out.push((tri_own, tri_nei, vol));
    }
    out
}

/// cfMesh `help::isSharedEdgeConvex`: every shared point's tet volume is
/// at most `-VSMALL`.
pub fn is_shared_edge_convex(points: &[Vec3], f1: &[usize], f2: &[usize]) -> bool {
    shared_point_tets(points, f1, f2).iter().all(|t| t.2 <= -VSMALL)
}

/// cfMesh `help::angleBetweenFaces` [rad]: the mean over the shared points
/// of the angle between the faces' corner triangles, in `[π, 2π)` where
/// the tet volume is above `-VSMALL` and `[0, π)` otherwise. `None` where
/// upstream stops ("do no share an edge").
pub fn angle_between_faces(points: &[Vec3], f1: &[usize], f2: &[usize]) -> Option<f64> {
    let tets = shared_point_tets(points, f1, f2);
    if tets.is_empty() {
        return None;
    }
    let mut angle = 0.0;
    for (own, nei, vol) in &tets {
        let n_own = points[own[1]].sub(points[own[0]]).cross(points[own[2]].sub(points[own[0]]));
        let n_own = n_own.scale(1.0 / (n_own.length() + VSMALL));
        let n_nei = points[nei[1]].sub(points[nei[0]]).cross(points[nei[2]].sub(points[nei[0]]));
        let n_nei = n_nei.scale(1.0 / (n_nei.length() + VSMALL));
        let dot = n_own.dot(n_nei).clamp(-1.0, 1.0);
        angle += if *vol > -VSMALL { dot.r_acos() + std::f64::consts::PI } else { (-dot).r_acos() };
    }
    Some(angle / tets.len() as f64)
}

/// Convex and concave edge counts of each pair of patches (`(lower,
/// higher)` patch index), as `findPatchesToBeTreatedTogether` classifies
/// them: an edge is concave if it is not convex by
/// [`is_shared_edge_convex`] or its [`angle_between_faces`] exceeds
/// 0.75 π.
pub type EdgeClassification = BTreeMap<(usize, usize), (usize, usize)>;

/// cfMesh `findPatchesToBeTreatedTogether` (serial): for every patch, the
/// patches its layer is built with (itself included). Patches are merged
/// at corners with more than three feature edges, and across any patch
/// pair with at least one concave edge (edges at an inverted vertex
/// skipped, except in 2D); a pair is not merged if either patch is
/// already treated. The merged groups are closed (connected components,
/// ascending). Returns the groups and the edge counts.
pub fn find_patches_to_be_treated_together(mesh: &VolumeMesh, treated_patch: &[bool], is_2d: bool) -> (Vec<Vec<usize>>, EdgeClassification) {
    let n = treated_patch.len();
    let mut with: Vec<Vec<usize>> = (0..n).map(|p| vec![p]).collect();
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    // corners where more than three feature edges meet
    for &b in &part.corners {
        if part.n_edges_at_point[b] <= 3 {
            continue;
        }
        let mut all: Vec<usize> = Vec::new();
        for &pp in part.point_patches[b].iter().filter(|&&pp| pp < n) {
            for &q in &with[pp] {
                if !all.contains(&q) {
                    all.push(q);
                }
            }
        }
        for &pp in part.point_patches[b].iter().filter(|&&pp| pp < n) {
            with[pp] = all.clone();
        }
    }
    // concave edges
    let inverted = if is_2d { BTreeSet::new() } else { check_inverted_vertices(&engine, &part, &mesh.points, None) };
    let mut classes: EdgeClassification = BTreeMap::new();
    for (e, ef) in engine.edge_faces.iter().enumerate() {
        if ef.len() != 2 {
            continue;
        }
        let ed = engine.edges[e];
        if !is_2d && (inverted.contains(&ed[0]) || inverted.contains(&ed[1])) {
            continue;
        }
        let (p0, p1) = (engine.face_patches[ef[0]], engine.face_patches[ef[1]]);
        if p0 == p1 || p0 >= n || p1 >= n {
            continue;
        }
        let (f1, f2) = (&engine.face_rings[ef[0]], &engine.face_rings[ef[1]]);
        let concave = !is_shared_edge_convex(&mesh.points, f1, f2) || angle_between_faces(&mesh.points, f1, f2).unwrap_or(0.0) > 0.75 * std::f64::consts::PI;
        let entry = classes.entry((p0.min(p1), p0.max(p1))).or_insert((0, 0));
        if concave {
            entry.1 += 1;
        } else {
            entry.0 += 1;
        }
    }
    for (&(p0, p1), &(_, n_concave)) in &classes {
        if n_concave == 0 || treated_patch[p0] || treated_patch[p1] {
            continue;
        }
        with[p0].push(p1);
        with[p1].push(p0);
    }
    // final adjusting: every member of a connected group gets the group
    let mut confirmed = vec![false; n];
    for pi in 0..n {
        if with[pi].len() <= 1 {
            confirmed[pi] = true;
            continue;
        }
        if confirmed[pi] {
            continue;
        }
        let mut common: BTreeSet<usize> = BTreeSet::from([pi]);
        let mut front = vec![pi];
        confirmed[pi] = true;
        while let Some(fp) = front.pop() {
            for i in 0..with[fp].len() {
                let pj = with[fp][i];
                if confirmed[pj] {
                    continue;
                }
                front.push(pj);
                confirmed[pj] = true;
                common.insert(pj);
                common.extend(with[pj].iter().copied());
            }
        }
        let list: Vec<usize> = common.iter().copied().collect();
        for &pj in &common {
            with[pj] = list.clone();
        }
    }
    (with, classes)
}

/// One boundary point of a patch-wise layer: each group's displacement at
/// it (`createNewVertex` with that group treated).
#[derive(Debug, Clone, PartialEq)]
pub struct PatchWisePoint {
    /// Mesh point (moved inward by the sum of its groups' displacements).
    pub point: usize,
    /// Its position on the wall before the layer.
    pub wall: Vec3,
    /// Displacement by group key.
    pub displacements: BTreeMap<usize, Vec3>,
}

/// What [`BoundaryLayers::add_layer_for_all_patches`] built.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PatchWiseOutcome {
    /// The groups as patch names, by group key.
    pub groups: Vec<Vec<String>>,
    /// Every boundary point that got a layer vertex, ascending.
    pub points: Vec<PatchWisePoint>,
    /// Layer cells over boundary faces (one per treated face).
    pub n_face_cells: usize,
    /// Cells at feature edges between groups (`createNewCellFromEdge`).
    pub n_edge_cells: usize,
    /// Cells at corners of three groups (`createNewCellFromNode`).
    pub n_corner_cells: usize,
    /// First added cell (the layer cells are the last ones).
    pub first_layer_cell: usize,
    /// Boundary faces of the assembled mesh that upstream's boundary list
    /// does not name, plus listed faces that did not come out on the
    /// boundary (0 when the cells close as upstream builds them).
    pub n_unmatched_boundary_faces: usize,
}

/// `otherVrts_`: per mesh point, its layer vertices by `(key, key)` (one
/// group) or `(key a, key b)` (two groups at a three-group corner).
type OtherVrts = BTreeMap<usize, BTreeMap<(usize, usize), usize>>;

/// cfMesh `findNewNodeLabel`.
fn find_new_node_label(other: &OtherVrts, new_label: &[Option<usize>], point: usize, p_key: usize) -> usize {
    if let Some(m) = other.get(&point) {
        if m.len() == 2 {
            if let Some((_, &v)) = m.iter().find(|(k, _)| k.0 != p_key) {
                return v;
            }
        } else if let Some((_, &v)) = m.iter().find(|(k, _)| k.0 != p_key && k.1 != p_key && k.0 != k.1) {
            return v;
        }
    }
    new_label[point].expect("findNewNodeLabel: a point with a layer vertex")
}

/// cfMesh `createNewCellFromEdge` for the edge `(s, e)`, oriented as it
/// runs in the face of key `ki`.
fn create_new_cell_from_edge(other: &OtherVrts, new_label: &[Option<usize>], s: usize, e: usize, ki: usize, kj: usize) -> [[usize; 4]; 6] {
    let ends = |pt: usize| -> (usize, usize, usize) {
        let m = &other[&pt];
        if m.len() == 2 {
            (m[&(ki, ki)], m[&(kj, kj)], new_label[pt].expect("edge point"))
        } else {
            let (mut p0, mut p1, mut n) = (usize::MAX, usize::MAX, usize::MAX);
            for (k, &v) in m {
                if k.0 != kj && k.1 != kj && k.0 != k.1 {
                    p0 = v;
                } else if k.0 != ki && k.1 != ki && k.0 != k.1 {
                    p1 = v;
                } else if k.0 == k.1 && k.0 != ki && k.0 != kj {
                    n = v;
                }
            }
            (p0, p1, n)
        }
    };
    let (p0s, p1s, ns) = ends(s);
    let (p0e, p1e, ne) = ends(e);
    [[e, s, p1s, p1e], [p0e, ne, ns, p0s], [s, e, p0e, p0s], [p1s, ns, ne, p1e], [e, p1e, ne, p0e], [s, p0s, ns, p1s]]
}

/// cfMesh `createNewCellFromNode` at `point` with the three group keys.
fn create_new_cell_from_node(other: &OtherVrts, new_label: &[Option<usize>], point: usize, keys: [usize; 3]) -> [[usize; 4]; 6] {
    let m = &other[&point];
    let n = new_label[point].expect("corner point");
    let pair = |a: usize, b: usize| m.get(&(a, b)).or_else(|| m.get(&(b, a))).copied().expect("corner pair");
    let (p00, p11, p22) = (m[&(keys[0], keys[0])], m[&(keys[1], keys[1])], m[&(keys[2], keys[2])]);
    let (p01, p02, p12) = (pair(keys[0], keys[1]), pair(keys[0], keys[2]), pair(keys[1], keys[2]));
    [[point, p02, p00, p01], [p12, p11, n, p22], [point, p01, p11, p12], [p02, p22, n, p00], [point, p12, p22, p02], [p01, p00, n, p11]]
}

/// Back to a [`VolumeMesh`] with every patch of `names` kept, empty ones
/// too (upstream's `boundaries()` never drops one; `TopoMesh::into_volume_mesh`
/// drops empty patches, which would shift the patch indices the class
/// state is kept by).
fn into_mesh_keeping_patches(w: TopoMesh, names: &[String]) -> VolumeMesh {
    let (mut m, _) = w.into_volume_mesh();
    let mut patches = Vec::with_capacity(names.len());
    let mut start = m.n_internal_faces();
    for name in names {
        match m.patches.iter().find(|p| &p.name == name) {
            Some(p) => {
                start = p.start_face + p.n_faces;
                patches.push(p.clone());
            }
            None => patches.push(crate::volume_mesh::BoundaryPatch { name: name.clone(), start_face: start, n_faces: 0 }),
        }
    }
    patches.extend(m.patches.iter().filter(|p| !names.contains(&p.name)).cloned());
    m.patches = patches;
    m
}

/// cfMesh's `boundaryLayers` class (serial): the state that persists
/// between `addLayerForPatch` calls (which patches are treated, the
/// groups), on a mesh it owns. Construct, optionally
/// [`BoundaryLayers::activate_2d_mode`] or
/// [`BoundaryLayers::create_o_topology_layers`], then add layers.
#[derive(Debug, Clone)]
pub struct BoundaryLayers {
    /// The mesh, changed by each call.
    pub mesh: VolumeMesh,
    /// `patchNames_`.
    names: Vec<String>,
    /// `patchWiseLayers_` (default `true`).
    patch_wise: bool,
    /// `is2DMesh_`.
    is_2d: bool,
    /// `treatedPatch_`.
    treated_patch: Vec<bool>,
    /// `treatPatchesWithPatch_` (empty until analysed).
    treat_with: Vec<Vec<usize>>,
    /// `geometryAnalysed_`.
    geometry_analysed: bool,
    /// `blHeightFactor_`, `blMinHeight_`.
    controls: BoundaryLayersControls,
    /// The edge classification of the analysis.
    pub edge_classification: EdgeClassification,
}

impl BoundaryLayers {
    /// The constructor: patch-wise layers, nothing treated.
    pub fn new(mesh: VolumeMesh, controls: BoundaryLayersControls) -> Self {
        let names: Vec<String> = mesh.patches.iter().map(|p| p.name.clone()).collect();
        let n = names.len();
        BoundaryLayers { mesh, names, patch_wise: true, is_2d: false, treated_patch: vec![false; n], treat_with: vec![Vec::new(); n], geometry_analysed: false, controls, edge_classification: BTreeMap::new() }
    }

    /// `createOTopologyLayers`: all untreated patches in one group.
    pub fn create_o_topology_layers(&mut self) {
        self.patch_wise = false;
    }

    /// `activate2DMode`: a patch all of whose points are at the smallest z,
    /// or all at the largest (not both), is marked treated (the empty
    /// patches get no layer).
    pub fn activate_2d_mode(&mut self) -> Result<(), String> {
        let e2d = crate::poly_mesh_gen_2d_engine::PolyMeshGen2DEngine::new(&self.mesh)?;
        let n = self.names.len();
        let (mut all_min, mut all_max) = (vec![true; n], vec![true; n]);
        for (pi, p) in self.mesh.patches.iter().enumerate().take(n) {
            for f in p.start_face..p.start_face + p.n_faces {
                for &v in &self.mesh.faces[f] {
                    all_min[pi] &= e2d.z_min_point[v];
                    all_max[pi] &= e2d.z_max_point[v];
                }
            }
        }
        for pi in 0..n {
            if all_min[pi] ^ all_max[pi] {
                self.treated_patch[pi] = true;
            }
        }
        let treated = self.treated_patch.clone();
        for list in &mut self.treat_with {
            list.retain(|&q| !treated[q]);
        }
        self.is_2d = true;
        Ok(())
    }

    /// The groups (`treatPatchesWithPatch_`), by patch index.
    pub fn treat_patches_with_patch(&mut self) -> &[Vec<usize>] {
        self.analyse();
        &self.treat_with
    }

    /// `treatedPatch_`.
    pub fn treated_patches(&self) -> &[bool] {
        &self.treated_patch
    }

    fn analyse(&mut self) {
        if self.geometry_analysed {
            return;
        }
        let (with, classes) = find_patches_to_be_treated_together(&self.mesh, &self.treated_patch, self.is_2d);
        self.treat_with = with;
        self.edge_classification = classes;
        self.geometry_analysed = true;
    }

    /// `addLayerForPatch(word)`: the group of the named patch (patch-wise),
    /// or every untreated patch (O-topology), by the one-group route
    /// ([`boundary_layers_add_layer`]); nothing for a treated or empty
    /// patch. Returns the one-group outcome, `None` if nothing was done.
    pub fn add_layer_for_patch(&mut self, name: &str) -> Result<Option<BoundaryLayersOutcome>, BoundaryLayersError> {
        self.analyse();
        let pi = self.names.iter().position(|n| n == name).ok_or_else(|| BoundaryLayersError::UnknownPatch(name.to_string()))?;
        if self.treated_patch[pi] || self.mesh.patches[pi].n_faces == 0 {
            return Ok(None);
        }
        let treat: Vec<usize> = if self.patch_wise { self.treat_with[pi].clone() } else { (0..self.names.len()).filter(|&q| !self.treated_patch[q]).collect() };
        let refs: Vec<&str> = treat.iter().map(|&q| self.names[q].as_str()).collect();
        let (m, out) = boundary_layers_add_layer(&self.mesh, &refs, &self.controls).ok_or(BoundaryLayersError::MoreThanThreePatches { point: usize::MAX })?;
        let mut w = TopoMesh::from_volume_mesh(&m);
        w.patch_names = m.patches.iter().map(|p| p.name.clone()).collect();
        self.mesh = into_mesh_keeping_patches(w, &self.names);
        for q in treat {
            self.treated_patch[q] = true;
        }
        Ok(Some(out))
    }

    /// `addLayerForAllPatches`. O-topology: the first non-empty untreated
    /// patch's `addLayerForPatch` treats them all as one group. Patch-wise
    /// (default): `createNewVertices` and `createLayerCells` over every
    /// untreated patch, each group with its own layer vertices and edge
    /// and corner cells where groups meet (upstream does not mark the
    /// patches treated here; neither does the port).
    pub fn add_layer_for_all_patches(&mut self) -> Result<PatchWiseOutcome, BoundaryLayersError> {
        self.analyse();
        let first = self.mesh.n_cells;
        if !self.patch_wise {
            let untreated: Vec<String> = (0..self.names.len()).filter(|&q| !self.treated_patch[q]).map(|q| self.names[q].clone()).collect();
            for name in self.names.clone() {
                if let Some(o) = self.add_layer_for_patch(&name)? {
                    return Ok(PatchWiseOutcome { groups: vec![untreated], n_face_cells: o.wall_faces.len(), first_layer_cell: first, ..Default::default() });
                }
            }
            return Ok(PatchWiseOutcome { first_layer_cell: first, ..Default::default() });
        }
        let labels: Vec<usize> = (0..self.names.len()).filter(|&q| !self.treated_patch[q]).collect();
        let (m, o) = patch_wise_layer(&self.mesh, &self.names, &labels, &self.treat_with, &self.treated_patch, &self.controls)?;
        self.mesh = m;
        Ok(o)
    }

    /// `addWrapperLayer`: O-topology, and unless patch 0 is treated, one
    /// layer of **zero thickness** on every patch: each boundary point gets
    /// a copy at its own position and `createNewFacesAndCells` closes the
    /// cells (upstream: no caller in cfMesh). **Upstream defect, ported to
    /// its intent:** upstream writes the copies' labels to a local
    /// `newLabelForVertex` that shadows the member `newLabelForVertex_`
    /// the face builder reads, so its cells are built from stale labels;
    /// here the copies are used. Returns the number of layer cells.
    pub fn add_wrapper_layer(&mut self) -> usize {
        self.create_o_topology_layers();
        if self.treated_patch.first().copied().unwrap_or(true) {
            return 0;
        }
        let all: Vec<&str> = self.names.iter().map(|s| s.as_str()).collect();
        let Some(s) = layer_setup(&self.mesh, &all, 1, 1.0, 1.0) else { return 0 };
        let inward: BTreeMap<usize, Vec3> = s.wall_pts.iter().map(|&p| (p, Vec3::ZERO)).collect();
        let totals: BTreeMap<usize, f64> = s.wall_pts.iter().map(|&p| (p, 0.0)).collect();
        let counts: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&p| (p, 1)).collect();
        let (layered, column) = build_layered_variable_with_columns(&self.mesh, &s.wall_faces, &s.wall_pts, &inward, &[0.0, 1.0], &totals, &counts);
        let key = |ring: &[usize]| -> Vec<usize> {
            let mut k = ring.to_vec();
            k.sort_unstable();
            k
        };
        let mut patch_of_key: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
        for (pi, p) in self.mesh.patches.iter().enumerate() {
            for f in p.start_face..p.start_face + p.n_faces {
                patch_of_key.insert(key(&self.mesh.faces[f].iter().map(|v| column[v][0]).collect::<Vec<_>>()), pi);
            }
        }
        let mut w = TopoMesh::from_volume_mesh(&layered);
        w.patch_names = self.names.clone();
        for f in 0..w.faces.len() {
            w.region[f] = if w.neighbour[f].is_none() { patch_of_key.get(&key(&w.faces[f])).copied() } else { None };
        }
        self.mesh = into_mesh_keeping_patches(w, &self.names);
        for t in &mut self.treated_patch {
            *t = true;
        }
        s.wall_faces.len()
    }

    /// cfMesh `checkTopologyOfBoundaryFaces` (patch-wise only; upstream: no
    /// caller). A boundary face of the given patches that meets a patch of
    /// its own group (a "terminated pair") across an edge is cut there:
    /// that edge, and the neighbouring edges on its side, are removed
    /// (`help::removeEdgesFromFace`), and the removed part becomes a second
    /// face (`createFaceFromRemovedPart`); repeated until nothing changes.
    /// Where both parts have more than two points the owner cell would be
    /// decomposed, and upstream stops ("Critical. Not tested"): so does
    /// this port ([`BoundaryLayersError::CriticalNotTested`]). Returns the
    /// number of faces cut.
    pub fn check_topology_of_boundary_faces(&mut self, patch_labels: &[usize]) -> Result<usize, BoundaryLayersError> {
        if !self.patch_wise {
            return Ok(0);
        }
        self.analyse();
        let used: BTreeSet<usize> = patch_labels.iter().copied().collect();
        let mut terminated: BTreeSet<(usize, usize)> = BTreeSet::new();
        for (pi, others) in self.treat_with.iter().enumerate() {
            for &q in others {
                if q != pi {
                    terminated.insert((pi.min(q), pi.max(q)));
                }
            }
        }
        let mut n_decomposed = 0;
        loop {
            let mut changed = false;
            let engine = SurfaceEngine::new(&self.mesh);
            let (mut faces, mut owners, mut patches) = (Vec::new(), Vec::new(), Vec::new());
            for bf in 0..engine.boundary_faces.len() {
                let ring = &engine.face_rings[bf];
                let fp = engine.face_patches[bf];
                let mut keep = |r: Vec<usize>| {
                    faces.push(r);
                    owners.push(engine.face_owners[bf]);
                    patches.push(fp);
                };
                if !used.contains(&fp) {
                    keep(ring.clone());
                    continue;
                }
                let k = ring.len();
                let nei: Vec<usize> = (0..k)
                    .map(|e| {
                        let ef = &engine.edge_faces[engine.face_edges[bf][e]];
                        if ef.len() == 2 {
                            engine.face_patches[if ef[0] == bf { ef[1] } else { ef[0] }]
                        } else {
                            fp
                        }
                    })
                    .collect();
                let is_terminated = |q: usize| terminated.contains(&(fp.min(q), fp.max(q)));
                let mut stored = false;
                for e in 0..k {
                    if nei[e] == fp || !is_terminated(nei[e]) {
                        continue;
                    }
                    let (prev, next) = ((e + k - 1) % k, (e + 1) % k);
                    let use_prev = nei[prev] == fp || !is_terminated(nei[prev]);
                    let use_next = nei[next] == fp || !is_terminated(nei[next]);
                    let edge = |i: usize| (ring[i], ring[(i + 1) % k]);
                    let remove: Vec<(usize, usize)> = match (use_prev, use_next) {
                        (true, true) => vec![edge(prev), edge(e), edge(next)],
                        (false, true) => vec![edge(next), edge(e)],
                        (true, false) => vec![edge(prev), edge(e)],
                        (false, false) => Vec::new(),
                    };
                    let cut = crate::topology::remove_edges_from_face(ring, &remove);
                    let rest = crate::topology::create_face_from_removed_part(ring, &cut);
                    let (a, r) = (cut.len() > 2, rest.len() > 2);
                    if a {
                        keep(cut.clone());
                    }
                    if r {
                        keep(rest);
                    }
                    if a && r {
                        changed = true;
                        n_decomposed += 1;
                    }
                    stored = true;
                    break;
                }
                if !stored {
                    keep(ring.clone());
                }
            }
            if !changed {
                break;
            }
            let mut w = TopoMesh::from_volume_mesh(&self.mesh);
            w.replace_boundary(self.names.clone(), faces, owners, patches);
            self.mesh = into_mesh_keeping_patches(w, &self.names);
        }
        if n_decomposed != 0 {
            return Err(BoundaryLayersError::CriticalNotTested { n_decomposed });
        }
        Ok(0)
    }
}

/// Which `boundaryLayers` calls a workflow makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerRequest {
    /// `addLayerForAllPatches` (`cartesianMesh` always; `tetMesh`, `pMesh`,
    /// `generateBoundaryLayers` with `nLayers > 0` or with no dict).
    AllPatches,
    /// `addLayerForPatch` for each name in turn (`patchBoundaryLayers`).
    Patches(Vec<String>),
}

/// What [`generate_layer`] did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeneratedLayer {
    /// The `addLayerForAllPatches` outcome.
    pub all: Option<PatchWiseOutcome>,
    /// Each `addLayerForPatch` call: the patch and the one-group outcome
    /// (`None`: already treated, or empty).
    pub per_patch: Vec<(String, Option<BoundaryLayersOutcome>)>,
    /// Cells added.
    pub n_layer_cells: usize,
}

impl GeneratedLayer {
    /// One line for a workflow report.
    pub fn summary(&self) -> String {
        match &self.all {
            Some(o) => format!(
                "addLayerForAllPatches: groups {:?}; {} layer cells ({} face, {} edge, {} corner), {} unmatched boundary faces",
                o.groups, self.n_layer_cells, o.n_face_cells, o.n_edge_cells, o.n_corner_cells, o.n_unmatched_boundary_faces
            ),
            None => format!("addLayerForPatch {:?}: {} layer cells", self.per_patch.iter().map(|(n, o)| (n.clone(), o.as_ref().map_or(0, |x| x.wall_faces.len()))).collect::<Vec<_>>(), self.n_layer_cells),
        }
    }
}

/// The `boundaryLayers` part of `generateBoundaryLayers`' `generateLayer`
/// and of `tetMeshGenerator` / `voronoiMeshGenerator` /
/// `cartesianMeshGenerator` / `cartesian2DMeshGenerator`'s
/// `generateBoundaryLayers`: a [`BoundaryLayers`] on `mesh` (patch-wise),
/// `activate2DMode` if asked, then the request.
pub fn generate_layer(mesh: &VolumeMesh, request: &LayerRequest, c: &BoundaryLayersControls, layers_2d: bool) -> Result<(VolumeMesh, GeneratedLayer), String> {
    let n0 = mesh.n_cells;
    let mut bl = BoundaryLayers::new(mesh.clone(), *c);
    if layers_2d {
        bl.activate_2d_mode()?;
    }
    let mut out = GeneratedLayer::default();
    match request {
        LayerRequest::AllPatches => out.all = Some(bl.add_layer_for_all_patches().map_err(|e| format!("boundaryLayers: {e:?}"))?),
        LayerRequest::Patches(names) => {
            for n in names {
                // upstream's addLayerForPatch(word) walks the patches and
                // does nothing for a name no patch has (gh:#1476: a
                // `patchBoundaryLayers` regex such as "orifice.*" reaches
                // here verbatim)
                let o = match bl.add_layer_for_patch(n) {
                    Err(BoundaryLayersError::UnknownPatch(_)) => None,
                    r => r.map_err(|e| format!("boundaryLayers: {e:?}"))?,
                };
                out.per_patch.push((n.clone(), o));
            }
        }
    }
    out.n_layer_cells = bl.mesh.n_cells - n0;
    Ok((bl.mesh, out))
}

/// `polyMeshGenModifier::addCells(cellsToAdd)`, `reorderBoundaryFaces()` and
/// `replaceBoundary(patchNames, boundaryFaces, owners, patches)` as
/// `boundaryLayers::createLayerCells` calls them (gh:#1385), so the faces
/// come out in upstream's order:
///
/// 1. the old faces keep their places; each layer cell, in order, looks each
///    of its faces up among the faces so far (same vertices) and appends the
///    ones it does not find;
/// 2. owner = the lower cell, neighbour = the higher, of the cells using the
///    face (`calculateOwnersAndNeighbours`);
/// 3. the boundary faces among the first `nInternalFaces` are swapped, in
///    order, with the internal faces among the last ones;
/// 4. the faces after the internal ones are replaced by `bnd`, patch by
///    patch, in list order.
///
/// `cells` holds the old cells first (`first_layer_cell` of them). Returns
/// the mesh and the number of boundary faces that do not match `bnd` (a face
/// on one side only, or listed and not a boundary face).
fn add_cells_upstream(mesh: &VolumeMesh, points: Vec<Vec3>, cells: &[Vec<Vec<usize>>], first_layer_cell: usize, bnd: &[(Vec<usize>, usize)], names: &[String]) -> (VolumeMesh, usize) {
    use std::collections::HashMap;
    let key = |ring: &[usize]| -> Vec<usize> {
        let mut k = ring.to_vec();
        k.sort_unstable();
        k
    };
    let mut faces: Vec<Vec<usize>> = mesh.faces.clone();
    let mut owner: Vec<Option<usize>> = mesh.owner.iter().map(|&o| Some(o)).collect();
    let mut neighbour: Vec<Option<usize>> = mesh.neighbour.clone();
    let mut index: HashMap<Vec<usize>, usize> = HashMap::new();
    for (f, ring) in faces.iter().enumerate() {
        index.insert(key(ring), f);
    }
    for (c, cell) in cells.iter().enumerate().skip(first_layer_cell) {
        for ring in cell {
            let k = key(ring);
            let f = match index.get(&k) {
                Some(&f) => f,
                None => {
                    faces.push(ring.clone());
                    owner.push(None);
                    neighbour.push(None);
                    index.insert(k, faces.len() - 1);
                    faces.len() - 1
                }
            };
            if owner[f].is_none() {
                owner[f] = Some(c);
            } else if neighbour[f].is_none() {
                neighbour[f] = Some(c);
            }
        }
    }
    let owner: Vec<usize> = owner.into_iter().map(|o| o.expect("every face has a cell")).collect();
    let mut owner_of: HashMap<Vec<usize>, (usize, bool)> = HashMap::new();
    for (f, ring) in faces.iter().enumerate() {
        owner_of.insert(key(ring), (owner[f], neighbour[f].is_some()));
    }
    let (mut faces, mut owner, mut neighbour) = (faces, owner, neighbour);
    crate::cartesian_mesh::reorder_boundary_faces(&mut faces, &mut owner, &mut neighbour);
    let n_int = neighbour.iter().filter(|x| x.is_some()).count();
    faces.truncate(n_int);
    owner.truncate(n_int);
    neighbour.truncate(n_int);
    let mut n_unmatched = 0;
    let mut patches = Vec::with_capacity(names.len());
    let mut listed: BTreeSet<Vec<usize>> = BTreeSet::new();
    for (pi, name) in names.iter().enumerate() {
        let start = faces.len();
        for (ring, patch) in bnd {
            if *patch != pi {
                continue;
            }
            let k = key(ring);
            match owner_of.get(&k) {
                Some(&(o, false)) => {
                    faces.push(ring.clone());
                    owner.push(o);
                    neighbour.push(None);
                    listed.insert(k);
                }
                _ => n_unmatched += 1,
            }
        }
        patches.push(crate::volume_mesh::BoundaryPatch { name: name.clone(), start_face: start, n_faces: faces.len() - start });
    }
    n_unmatched += owner_of.iter().filter(|(k, &(_, internal))| !internal && !listed.contains(*k)).count();
    (VolumeMesh { points, faces, owner, neighbour, n_cells: cells.len(), patches }, n_unmatched)
}

/// cfMesh `createNewVertices(labelList)` and `createLayerCells` on `mesh`
/// for the untreated patches `labels` with the groups `treat_with` (see
/// [`BoundaryLayers::add_layer_for_all_patches`]). The cells are built as
/// upstream lists their faces (`addCells`) and assembled by matching
/// faces; the boundary faces take the patches of upstream's
/// `replaceBoundary` list. **Not through `build_layered_variable`:** its
/// columns have one direction per wall point and no edge or corner
/// cells, which is exactly what this route adds.
fn patch_wise_layer(
    mesh: &VolumeMesh,
    names: &[String],
    labels: &[usize],
    treat_with: &[Vec<usize>],
    treated_patch: &[bool],
    c: &BoundaryLayersControls,
) -> Result<(VolumeMesh, PatchWiseOutcome), BoundaryLayersError> {
    let n_patches = names.len();
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    let nbp = engine.boundary_points.len();
    let mut points = mesh.points.clone();
    let mut new_label: Vec<Option<usize>> = vec![None; points.len()];
    let mut other: OtherVrts = BTreeMap::new();
    let mut patch_key: Vec<Option<usize>> = vec![None; n_patches + 1];
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut displacements: BTreeMap<usize, BTreeMap<usize, Vec3>> = BTreeMap::new();

    // createNewVertices(labelList)
    for (pi_idx, &pl) in labels.iter().enumerate() {
        let mut treat = vec![false; n_patches + 1];
        let mut ok = true;
        for &op in &treat_with[pl] {
            treat[op] = true;
            if patch_key[op].is_none() {
                patch_key[op] = Some(pi_idx);
            } else {
                ok = false;
            }
        }
        if !ok {
            continue;
        }
        let p_key = patch_key[pl].expect("key just set");
        if groups.len() <= p_key {
            groups.resize(p_key + 1, Vec::new());
        }
        groups[p_key] = treat_with[pl].iter().map(|&q| names[q].clone()).collect();
        let pv = find_patch_vertices(&part, &treat);
        let mut patch_points = Vec::new();
        for (b, &flag) in pv.iter().enumerate() {
            if flag == 0 {
                continue;
            }
            patch_points.push(b);
            let pt = engine.boundary_points[b];
            let label = points.len();
            points.push(Vec3::ZERO);
            if flag & EDGENODE != 0 {
                other.entry(pt).or_default().insert((p_key, p_key), label);
            } else {
                new_label[pt] = Some(label);
            }
        }
        for &b in &patch_points {
            let pt = engine.boundary_points[b];
            let (np, _) = create_new_vertex(&engine, &part, &mesh.points, b, &treat, &pv, c)?;
            displacements.entry(pt).or_default().insert(p_key, np.sub(mesh.points[pt]));
            let label = if pv[b] & EDGENODE != 0 { other[&pt][&(p_key, p_key)] } else { new_label[pt].expect("set above") };
            points[label] = np;
        }
    }
    // the missing vertices at edges and corners
    for b in 0..nbp {
        let pt = engine.boundary_points[b];
        let Some(m) = other.get_mut(&pt) else { continue };
        let p = points[pt];
        let (mut used, mut lab, mut pen): (Vec<usize>, Vec<usize>, Vec<Vec3>) = (Vec::new(), Vec::new(), Vec::new());
        for &pp in &part.point_patches[b] {
            let Some(k) = patch_key.get(pp).copied().flatten() else { continue };
            if let Some(&l) = m.get(&(k, k)) {
                if !used.contains(&k) {
                    used.push(k);
                    lab.push(l);
                    pen.push(points[l].sub(p));
                }
            }
        }
        let finite = |q: Vec3| if q.x.is_finite() && q.y.is_finite() && q.z.is_finite() { q } else { p };
        match lab.len() {
            1 => {
                new_label[pt] = Some(lab[0]);
                other.remove(&pt);
            }
            2 => {
                points.push(finite(p.add(pen[0]).add(pen[1])));
                new_label[pt] = Some(points.len() - 1);
            }
            3 => {
                let mut np = p;
                for i in 0..3 {
                    np = np.add(pen[i]);
                    for j in i + 1..3 {
                        points.push(finite(p.add(pen[i]).add(pen[j])));
                        m.insert((used[i], used[j]), points.len() - 1);
                    }
                }
                points.push(finite(np));
                new_label[pt] = Some(points.len() - 1);
            }
            n => return Err(BoundaryLayersError::NotAtAnEdge { point: pt, n_groups: n }),
        }
    }
    // swap the coordinates of the new and the old points
    for b in 0..nbp {
        let pt = engine.boundary_points[b];
        if let Some(l) = new_label[pt] {
            points.swap(l, pt);
        }
    }

    // createLayerCells
    let mut treat = vec![false; n_patches + 1];
    for &pl in labels {
        for &q in &treat_with[pl] {
            treat[q] = true;
        }
    }
    let mut cells: Vec<Vec<Vec<usize>>> = crate::volume_mesh::cells_faces(mesh);
    let first_layer_cell = cells.len();
    let mut bnd: Vec<(Vec<usize>, usize)> = Vec::new();
    let mut n_face_cells = 0;
    for bf in 0..engine.boundary_faces.len() {
        let fp = engine.face_patches[bf];
        let f = &engine.face_rings[bf];
        if !treat[fp] {
            bnd.push((f.clone(), fp));
            continue;
        }
        let key = patch_key[fp].expect("a treated patch has a key");
        let k = f.len();
        let mut cf: Vec<Vec<usize>> = Vec::with_capacity(k + 2);
        let mut rev = vec![f[0]];
        rev.extend(f[1..].iter().rev());
        cf.push(rev);
        let top: Vec<usize> = f.iter().map(|&v| find_new_node_label(&other, &new_label, v, key)).collect();
        cf.push(top.clone());
        bnd.push((top, fp));
        for i in 0..k {
            let quad = vec![f[i], f[(i + 1) % k], find_new_node_label(&other, &new_label, f[(i + 1) % k], key), find_new_node_label(&other, &new_label, f[i], key)];
            let ef = &engine.edge_faces[engine.face_edges[bf][i]];
            if ef.len() == 2 {
                let nei = if ef[0] == bf { ef[1] } else { ef[0] };
                if !treat[engine.face_patches[nei]] {
                    bnd.push((quad.clone(), engine.face_patches[nei]));
                }
            }
            cf.push(quad);
        }
        cells.push(cf);
        n_face_cells += 1;
    }
    // cells at edges
    let mut n_edge_cells = 0;
    let unused_patch = |pt: usize, pi: usize, pj: usize| -> Option<usize> {
        let mut unused = None;
        for &q in &part.point_patches[engine.bp[pt]] {
            if q == pi || q == pj {
                continue;
            }
            if unused.is_some() {
                return None;
            }
            unused = Some(q);
        }
        unused
    };
    for (e, ef) in engine.edge_faces.iter().enumerate() {
        if ef.len() != 2 {
            continue;
        }
        let (pi, pj) = (engine.face_patches[ef[0]], engine.face_patches[ef[1]]);
        if pi == pj {
            continue;
        }
        let (Some(ki), Some(kj)) = (patch_key[pi], patch_key[pj]) else { continue };
        if ki == kj {
            continue;
        }
        let ed = engine.edges[e];
        if !other.contains_key(&ed[0]) || !other.contains_key(&ed[1]) {
            continue;
        }
        // the edge as it runs in the face of key ki (upstream's edges run so
        // in their first edge face)
        let ring = &engine.face_rings[ef[0]];
        let kr = ring.len();
        let forward = (0..kr).any(|i| ring[i] == ed[0] && ring[(i + 1) % kr] == ed[1]);
        let (s, t) = if forward { (ed[0], ed[1]) } else { (ed[1], ed[0]) };
        let cf = create_new_cell_from_edge(&other, &new_label, s, t, ki, kj);
        bnd.push((cf[1].to_vec(), pj));
        bnd.push((cf[3].to_vec(), pi));
        if let Some(u) = unused_patch(s, pi, pj) {
            if treated_patch.get(u).copied().unwrap_or(false) {
                bnd.push((cf[5].to_vec(), u));
            }
        }
        if let Some(u) = unused_patch(t, pi, pj) {
            if treated_patch.get(u).copied().unwrap_or(false) {
                bnd.push((cf[4].to_vec(), u));
            }
        }
        cells.push(cf.iter().map(|q| q.to_vec()).collect());
        n_edge_cells += 1;
    }
    // cells at corners
    let mut n_corner_cells = 0;
    let corner_points: Vec<usize> = other.iter().filter(|(_, m)| m.len() != 2).map(|(&p, _)| p).collect();
    for pt in corner_points {
        let b = engine.bp[pt];
        let mut pf: Vec<usize> = engine.point_faces[b].clone();
        for i in 0..pf.len() {
            let bf = &engine.face_rings[pf[i]];
            let kb = bf.len();
            let pos = bf.iter().position(|&v| v == pt).expect("point face");
            let (es, ee) = (bf[pos], bf[(pos + 1) % kb]);
            for j in i + 1..pf.len() {
                let obf = &engine.face_rings[pf[j]];
                if obf.contains(&es) && obf.contains(&ee) {
                    pf.swap(i + 1, j);
                    break;
                }
            }
        }
        let mut ids: Vec<usize> = Vec::new();
        for &bf in &pf {
            let q = engine.face_patches[bf];
            if !ids.contains(&q) {
                ids.push(q);
            }
        }
        let mut keys: Vec<usize> = Vec::new();
        for &q in &ids {
            if let Some(k) = patch_key.get(q).copied().flatten() {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
        if keys.len() != 3 {
            continue;
        }
        let cf = create_new_cell_from_node(&other, &new_label, pt, [keys[0], keys[1], keys[2]]);
        bnd.push((cf[1].to_vec(), ids[0]));
        bnd.push((cf[3].to_vec(), ids[1]));
        bnd.push((cf[5].to_vec(), ids[2]));
        cells.push(cf.iter().map(|q| q.to_vec()).collect());
        n_corner_cells += 1;
    }

    // addCells, reorderBoundaryFaces, replaceBoundary: in upstream's order
    let (out, n_unmatched) = add_cells_upstream(mesh, points, &cells, first_layer_cell, &bnd, names);
    let pts = displacements.into_iter().map(|(point, d)| PatchWisePoint { point, wall: mesh.points[point], displacements: d }).collect();
    Ok((out, PatchWiseOutcome { groups, points: pts, n_face_cells, n_edge_cells, n_corner_cells, first_layer_cell, n_unmatched_boundary_faces: n_unmatched }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::checks::check_quality;
    use crate::patches::{assign_patches_by_region, SurfaceRegions};
    use crate::shapes::box_surface;

    /// The unit box carved at `h`, `walls` on its four y/z sides, `inlet`
    /// at x = 0 and `outlet` at x = 1 (the `layers` doc example).
    pub(crate) fn named_box(h: f64) -> VolumeMesh {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&["walls", "walls", "walls", "walls", "walls", "walls", "walls", "walls", "outlet", "outlet", "inlet", "inlet"]);
        assign_patches_by_region(&carve_box(&p, &t, h), &p, &t, &regions).unwrap()
    }

    fn patch_faces(m: &VolumeMesh, name: &str) -> Vec<usize> {
        m.patches.iter().find(|q| q.name == name).map_or(Vec::new(), |q| (q.start_face..q.start_face + q.n_faces).collect())
    }

    /// V&V (gh:#1270) — cfMesh's layer on the carved box, by hand.
    /// Methodology: unit box carved at h 0.25 m (64 hexes), `walls` on the
    /// four y/z sides (64 faces, 80 points), `inlet`/`outlet` at x = 0 / 1;
    /// `boundary_layers_add_layer(walls)` with upstream's defaults.
    /// Predicted before the first run (from `boundaryLayersCreateVertices.C`,
    /// re-read for this test):
    /// - 128 cells (one per wall face), volume 1 to 1e-12, closed, no
    ///   inverted cell;
    /// - `walls` 64 faces; `inlet` and `outlet` 16 + 16 each (the
    ///   rim's side quads join them), every one of their points still at
    ///   x = 0 / x = 1 **exactly** (re-partitioned, not moved);
    /// - column lengths: 0.5 h = 0.125 at the 36 flat points, the 12
    ///   points on the wall–wall edges (along the diagonal) and the 24
    ///   rim points (`EDGENODE`, one other patch); at the 8 corners every
    ///   neighbour is a patch vertex, so the distance comes only from the
    ///   cap 0.9 x h/sqrt(2) = 0.15910 (the segment between the corner's
    ///   two inlet neighbours).
    ///
    /// Result (2026-11-05): every prediction held (48 `PATCHNODE`, 32
    /// `EDGENODE`; `inlet`/`outlet` points at x = 0 / 1 bit for bit).
    #[test]
    fn cfmesh_layer_on_a_carved_box_by_hand() {
        let h = 0.25;
        let m = named_box(h);
        assert_eq!((m.cell_count(), patch_faces(&m, "walls").len()), (64, 64));
        let (l, o) = boundary_layers_add_layer(&m, &["walls"], &BoundaryLayersControls::default()).unwrap();
        assert_eq!(l.cell_count(), 128);
        assert_eq!(o.wall_faces.len(), 64);
        assert_eq!(o.points.len(), 80);
        l.validate().unwrap();
        assert!((l.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(check_quality(&l).n_negative_volume_cells, 0);
        assert_eq!(patch_faces(&l, "walls").len(), 64);
        for (name, x) in [("inlet", 0.0), ("outlet", 1.0)] {
            let fs = patch_faces(&l, name);
            assert_eq!(fs.len(), 32, "{name}");
            assert!(fs.iter().all(|&f| l.faces[f].iter().all(|&v| l.points[v].x == x)), "{name} moved");
        }
        let corner = 0.9 * h / 2f64.sqrt();
        let mut n = [0usize; 3];
        for p in &o.points {
            let at_end = p.wall.x == 0.0 || p.wall.x == 1.0;
            let on_edge = (p.wall.y == 0.0 || p.wall.y == 1.0) && (p.wall.z == 0.0 || p.wall.z == 1.0);
            let want = if at_end && on_edge { corner } else { 0.5 * h };
            assert!((p.thickness() - want).abs() < 1e-12, "{p:?}");
            n[p.n_other_patches] += 1;
        }
        assert_eq!(n, [48, 32, 0]);
    }

    /// V&V (gh:#1270) — differential: cfMesh's layer against the crate's
    /// `BackOffOnly` layer (`add_boundary_layers_adaptive_on_patches`'s
    /// body) on the same box, one layer, the crate asked for 0.125 m (= 0.5
    /// h, so its cap 0.5 x the shortest wall edge does not bind).
    /// Predicted before the first run:
    /// - both conserve the volume (1e-12) and give 128 cells; the crate
    ///   passes on its first try (no back-off);
    /// - column lengths agree (0.125, 1e-12) at the 72 points off the
    ///   corners; at the 8 corners cfMesh's is 0.159 and the crate's 0.125
    ///   (the crate takes the smaller of its cap and upstream's border cap);
    /// - the directions agree at the rim and corners (both use upstream's
    ///   projected normal, held fixed by the crate's smoothing) and at the
    ///   wall–wall edge points (diagonal by symmetry), and differ at the
    ///   flat points next to an edge (the crate smooths, cfMesh does not):
    ///   24 of the 36 flat points (y or z = 0.25, 0.75); the 12 on a wall's
    ///   mid-line (0.5) keep the flat normal by symmetry in both;
    /// - neither has an inverted cell.
    ///
    /// Result (2026-11-05): every prediction held. Lengths equal at 72
    /// points, the 8 corners 0.159 against 0.125, directions differing at
    /// 24 flat points; 128 cells each, volume 1 (1e-15), the crate's first
    /// try; max non-orthogonality 23.9° (cfMesh) and 21.7°, skewness 1.28
    /// and 1.37.
    #[test]
    fn cfmesh_layer_against_back_off_on_the_box() {
        let m = named_box(0.25);
        let (l, o) = boundary_layers_add_layer(&m, &["walls"], &BoundaryLayersControls::default()).unwrap();
        let (b, ob) = super::super::add_boundary_layers_back_off(&m, &["walls"], 1, 0.125, 1.0);
        let setup = super::super::layer_setup(&m, &["walls"], 1, 0.125, 1.0).unwrap();
        let (qc, qb) = (check_quality(&l), check_quality(&b));
        println!("cfMesh: {} cells, V {:.15}, non-ortho {:.2}, skew {:.3}; back-off: {} cells, V {:.15}, non-ortho {:.2}, skew {:.3}, tries {}", l.cell_count(), l.total_volume(), qc.max_non_orthogonality_deg, qc.max_skewness, b.cell_count(), b.total_volume(), qb.max_non_orthogonality_deg, qb.max_skewness, ob.iterations);
        assert_eq!((l.cell_count(), b.cell_count()), (128, 128));
        assert!((b.total_volume() - 1.0).abs() < 1e-12 && (l.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(ob.iterations, 1);
        assert_eq!((qc.n_negative_volume_cells, qb.n_negative_volume_cells), (0, 0));
        let (mut same_len, mut corner_diff, mut dir_diff) = (0, 0, 0);
        for (p, q) in o.points.iter().zip(&ob.points) {
            assert_eq!(p.wall, q.0);
            if (p.thickness() - q.2).abs() < 1e-12 {
                same_len += 1;
            } else {
                assert!((p.thickness() - 0.9 * 0.25 / 2f64.sqrt()).abs() < 1e-12 && (q.2 - 0.125).abs() < 1e-12, "{p:?} {q:?}");
                corner_diff += 1;
            }
            if p.direction.sub(setup.inward[&p.point]).length() > 1e-12 {
                dir_diff += 1;
                let mid = |c: f64| (c - 0.5).abs() < 1e-12;
                assert!(p.n_other_patches == 0 && !mid(p.wall.y) && !mid(p.wall.z), "{p:?}");
            }
        }
        println!("column lengths equal at {same_len}, corners differing {corner_diff}, directions differing {dir_diff}");
        assert_eq!((same_len, corner_diff, dir_diff), (72, 8, 24));
    }

    /// The bare pipe (R 10 mm, 48 facets, default dual) of length `l` at
    /// cell size `h`, with its `walls`, `inlet`, `outlet`.
    pub(crate) fn bare_pipe(l: f64, h: f64) -> VolumeMesh {
        use crate::pipeline::tests::pipe_surface;
        use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
        let (p, t, regions) = pipe_surface(0.01, l, 48);
        let o = TetDualOptions { cell_size: h, n_layers: 0, ..Default::default() };
        surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap().0
    }

    /// One line of the pipe differential: both routes on `bare`.
    fn pipe_differential(bare: &VolumeMesh, label: &str) -> (usize, usize, usize, usize, f64, f64, f64) {
        let n_wall = patch_faces(bare, "walls").len();
        let (l, o) = boundary_layers_add_layer(bare, &["walls"], &BoundaryLayersControls::default()).unwrap();
        let (b, ob) = super::super::add_boundary_layers_back_off(bare, &["walls"], 1, 1.0, 1.0);
        let v0 = bare.total_volume();
        let (qc, qb) = (check_quality(&l), check_quality(&b));
        let stats = |t: &[f64]| {
            let (mn, mx) = t.iter().fold((f64::MAX, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
            (mn, t.iter().sum::<f64>() / t.len().max(1) as f64, mx)
        };
        let tc: Vec<f64> = o.points.iter().map(|p| p.thickness()).collect();
        let tb: Vec<f64> = ob.points.iter().filter(|p| p.1 > 0).map(|p| p.2).collect();
        // PATCHNODE ratio crate / cfMesh.
        let ratios: Vec<f64> = o.points.iter().zip(&ob.points).filter(|(p, q)| p.n_other_patches == 0 && q.1 > 0).map(|(p, q)| q.2 / p.thickness()).collect();
        let (rmin, _, rmax) = stats(&ratios);
        let rim: Vec<(f64, f64)> = o.points.iter().zip(&ob.points).filter(|(p, _)| p.n_other_patches > 0).map(|(p, q)| (p.thickness(), q.2)).collect();
        let (cs, bs) = (stats(&tc), stats(&tb));
        println!("{label}: bare {} cells, {n_wall} wall faces, {} wall points ({} on the rim)", bare.cell_count(), o.points.len(), rim.len());
        println!(
            "  cfMesh: {} cells, dV/V {:.2e}, first layer min/mean/max {:.3}/{:.3}/{:.3} mm, non-ortho {:.1}, skew {:.2}, inverted {}, neg pyramids {}, min det {:.3}",
            l.cell_count(),
            (l.total_volume() - v0) / v0,
            cs.0 * 1e3,
            cs.1 * 1e3,
            cs.2 * 1e3,
            qc.max_non_orthogonality_deg,
            qc.max_skewness,
            qc.n_negative_volume_cells,
            qc.n_negative_pyramid_faces,
            qc.min_cell_determinant
        );
        println!(
            "  back-off: {} cells, dV/V {:.2e}, first layer min/mean/max {:.3}/{:.3}/{:.3} mm, non-ortho {:.1}, skew {:.2}, inverted {}, neg pyramids {}, min det {:.3}, tries {}",
            b.cell_count(),
            (b.total_volume() - v0) / v0,
            bs.0 * 1e3,
            bs.1 * 1e3,
            bs.2 * 1e3,
            qb.max_non_orthogonality_deg,
            qb.max_skewness,
            qb.n_negative_volume_cells,
            qb.n_negative_pyramid_faces,
            qb.min_cell_determinant,
            ob.iterations
        );
        let rim_c = stats(&rim.iter().map(|r| r.0).collect::<Vec<_>>());
        let rim_b = stats(&rim.iter().map(|r| r.1).collect::<Vec<_>>());
        println!(
            "  PATCHNODE length ratio back-off/cfMesh min {rmin:.15} max {rmax:.15}; rim cfMesh {:.3}/{:.3}/{:.3} mm, back-off {:.3}/{:.3}/{:.3} mm",
            rim_c.0 * 1e3,
            rim_c.1 * 1e3,
            rim_c.2 * 1e3,
            rim_b.0 * 1e3,
            rim_b.1 * 1e3,
            rim_b.2 * 1e3
        );
        // Planarity of the end patches: the spread of their points along
        // the axis, before and after (a re-partitioned face that is not
        // planar changes the centroid-fan volume).
        for (name, m) in [("bare", bare), ("cfMesh", &l), ("back-off", &b)] {
            for end in ["inlet", "outlet"] {
                let zs: Vec<f64> = patch_faces(m, end).iter().flat_map(|&f| m.faces[f].iter().map(|&v| m.points[v].z)).collect();
                let (lo, hi) = zs.iter().fold((f64::MAX, f64::MIN), |(a, c), &z| (a.min(z), c.max(z)));
                let dv: f64 = (0..m.face_count()).filter(|&f| m.neighbour[f].is_none()).map(|f| m.face_centroid(f).dot(m.face_area_vector(f))).sum::<f64>() / 3.0;
                println!("  {name} {end}: z spread {:.3e} m ({} faces); volume {dv:.12e}", hi - lo, patch_faces(m, end).len());
            }
        }
        let wall_v = |m: &VolumeMesh, pname: &str| -> f64 { patch_faces(m, pname).iter().map(|&f| m.face_centroid(f).dot(m.face_area_vector(f))).sum::<f64>() / 3.0 };
        for pname in ["walls", "inlet", "outlet"] {
            println!("  patch {pname}: Σ c·S/3 bare {:.12e}, cfMesh {:.12e}, back-off {:.12e}", wall_v(bare, pname), wall_v(&l, pname), wall_v(&b, pname));
        }
        (l.cell_count(), b.cell_count(), bare.cell_count() + n_wall, ob.iterations, rmin, rmax, 0.6f64.powi(ob.iterations.max(1) as i32 - 1))
    }

    /// V&V (gh:#1270) — differential on the pipe: cfMesh's layer against
    /// the crate's `BackOffOnly`, one layer, the crate asked for 1 m so its
    /// cap (0.5 x the shortest wall edge at the point) decides. Methodology:
    /// the demo pipe shortened to L 60 mm, h 6 mm, default dual, bare.
    /// Predicted before the first run:
    /// - cfMesh conserves the volume (1e-12 relative) and gives bare +
    ///   wall-face cells; so does the crate, unless it backs off to nothing;
    /// - at every `PATCHNODE` the crate's column is cfMesh's times the
    ///   crate's uniform back-off factor 0.6^(tries − 1), to 1e-12: both
    ///   take 0.5 x the shortest edge to a neighbouring boundary point (all
    ///   wall points there), and that is all either does to the length;
    /// - at the rim the two differ (cfMesh: 0.5 x the shortest
    ///   wall-normal distance to an inlet-interior neighbour, then its 0.9
    ///   cap; the crate: 0.5 x its shortest wall edge, then the cap);
    /// - cfMesh has no quality check: its skewness may exceed the crate's
    ///   limit (4), and it may have inverted cells; both are recorded.
    ///
    /// Results (2026-11-05): 605 bare cells, 1 420 wall faces, 1 456 wall
    /// points (72 on the rim). Both give 2 025 cells and conserve the
    /// volume (2e-16, 4e-16). The crate backed off twice (3 tries): its
    /// `PATCHNODE` columns are cfMesh's x 0.36 = 0.6² at every point, to
    /// 1e-15 (held). First layer min/mean/max: cfMesh 0.002/0.609/1.179 mm,
    /// back-off 0.001/0.220/0.424 mm; rim cfMesh 0.002/0.432/1.044 mm,
    /// back-off 0.001/0.164/0.340 mm (they differ, as predicted). cfMesh:
    /// **56 inverted cells**, 716 non-positive face pyramids, max
    /// non-orthogonality 106.9°, skewness 51.3, min determinant 0.014; the
    /// back-off: 0, 0, 80.9°, 0.92, 0.076. Upstream's layer of half the
    /// local edge, along the unsmoothed point normal, folds cells of the
    /// curved dual wall; upstream repairs that later (`meshOptimizer`,
    /// `boundaryLayerOptimisation`), and this route does not.
    #[test]
    fn cfmesh_layer_against_back_off_on_the_pipe() {
        let bare = bare_pipe(0.06, 0.006);
        let (cc, bc, want, tries, rmin, rmax, scale) = pipe_differential(&bare, "pipe L 60 mm h 6 mm");
        let (l, _) = boundary_layers_add_layer(&bare, &["walls"], &BoundaryLayersControls::default()).unwrap();
        assert!(((l.total_volume() - bare.total_volume()) / bare.total_volume()).abs() < 1e-12);
        assert_eq!(cc, want);
        if bc == want {
            assert!((rmin - scale).abs() < 1e-12 && (rmax - scale).abs() < 1e-12, "{rmin} {rmax} {scale} ({tries} tries)");
        }
    }

    /// Diagnostic (gh:#1270), `--ignored`: the same differential on the
    /// Foam Studio pipe (R 10 mm, L 200 mm, 48 facets, h 5 mm, bare).
    ///
    /// Results (2026-11-05): 3 113 bare cells, 7 632 wall faces, 7 688 wall
    /// points (112 on the rim); both 10 745 cells. Volume: cfMesh +2.45e-7,
    /// back-off −4.72e-7 relative, **not** conserved to 1e-12. Located: the
    /// wall patch's Σ c·S/3 is bit for bit the bare one; all of the change
    /// is in `inlet` and `outlet`, whose points spread 0.47 mm along the
    /// axis on this dual (they are not planar), so sliding the rim points
    /// in their averaged plane moves the end surfaces (the `layers` module
    /// docs are corrected). The crate backed off twice (`PATCHNODE` ratio
    /// 0.36 at every point, 1e-15). First layer min/mean/max: cfMesh
    /// 0.032/0.491/0.857 mm, back-off 0.011/0.176/0.309 mm. cfMesh: 304
    /// inverted cells, 3 964 non-positive pyramids, non-orthogonality
    /// 102.5°, skewness 8.42; back-off: 0, 0, 78.6°, 1.01.
    #[test]
    #[ignore]
    fn diagnose_cfmesh_layer_on_the_foam_studio_pipe() {
        let bare = bare_pipe(0.2, 0.005);
        pipe_differential(&bare, "Foam Studio pipe L 200 mm h 5 mm");
    }

    /// The unit box carved at `h` with one patch per side (`z0`, `z1`,
    /// `y0`, `y1`, `x1`, `x0` in `box_surface`'s triangle order).
    pub(crate) fn six_patch_box(h: f64) -> VolumeMesh {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&["z0", "z0", "z1", "z1", "y0", "y0", "y1", "y1", "x1", "x1", "x0", "x0"]);
        assign_patches_by_region(&carve_box(&p, &t, h), &p, &t, &regions).unwrap()
    }

    fn n_nonpositive(m: &VolumeMesh) -> usize {
        crate::checks::cell_geometry(m).0.iter().filter(|&&v| v <= 0.0).count()
    }

    /// V&V (gh:#1270 remainder), prediction P1: the patch-wise layer on the
    /// six-patch box (record: `verification_and_validation/patch_wise_layers_gh1270/`).
    #[test]
    fn patch_wise_layer_on_the_six_patch_box() {
        let m = six_patch_box(0.25);
        let mut bl = BoundaryLayers::new(m, BoundaryLayersControls::default());
        let o = bl.add_layer_for_all_patches().unwrap();
        let l = &bl.mesh;
        println!("B6: groups {:?}; cells {} (face {}, edge {}, corner {}), unmatched {}, V {:.15}, non-positive {}", o.groups, l.n_cells, o.n_face_cells, o.n_edge_cells, o.n_corner_cells, o.n_unmatched_boundary_faces, l.total_volume(), n_nonpositive(l));
        assert_eq!(o.groups.len(), 6);
        assert!(o.groups.iter().all(|g| g.len() == 1));
        assert_eq!((l.n_cells, o.n_face_cells, o.n_edge_cells, o.n_corner_cells, o.n_unmatched_boundary_faces), (216, 96, 48, 8, 0));
        l.validate().unwrap();
        assert!((l.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(n_nonpositive(l), 0);
        for p in &o.points {
            for d in p.displacements.values() {
                assert!((d.length() - 0.125).abs() < 1e-12, "{p:?}");
            }
        }
        let grid = [0.0, 0.125, 0.25, 0.5, 0.75, 0.875, 1.0];
        for q in &l.points {
            for c in [q.x, q.y, q.z] {
                assert!(grid.iter().any(|g| (g - c).abs() < 1e-12), "{q:?}");
            }
        }
        assert!(l.patches.iter().all(|p| p.n_faces == 36), "{:?}", l.patches);
    }

    /// V&V (gh:#1270 remainder), prediction P2: the box with `walls`,
    /// `inlet`, `outlet`, patch-wise against O-topology.
    #[test]
    fn patch_wise_against_o_topology_on_the_named_box() {
        let m = named_box(0.25);
        let mut bl = BoundaryLayers::new(m.clone(), BoundaryLayersControls::default());
        let o = bl.add_layer_for_all_patches().unwrap();
        let l = &bl.mesh;
        println!("B3 patch-wise: groups {:?}; cells {} (face {}, edge {}, corner {}), unmatched {}, V {:.15}, non-positive {}", o.groups, l.n_cells, o.n_face_cells, o.n_edge_cells, o.n_corner_cells, o.n_unmatched_boundary_faces, l.total_volume(), n_nonpositive(l));
        assert_eq!(o.groups.len(), 3);
        assert_eq!((l.n_cells, o.n_edge_cells, o.n_corner_cells, o.n_unmatched_boundary_faces), (192, 32, 0, 0));
        l.validate().unwrap();
        assert!((l.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(n_nonpositive(l), 0);
        let mut ot = BoundaryLayers::new(m.clone(), BoundaryLayersControls::default());
        ot.create_o_topology_layers();
        let oo = ot.add_layer_for_all_patches().unwrap();
        let (one, _) = boundary_layers_add_layer(&m, &["walls", "inlet", "outlet"], &BoundaryLayersControls::default()).unwrap();
        println!("B3 O-topology: {} cells ({} face cells)", ot.mesh.n_cells, oo.n_face_cells);
        assert_eq!(ot.mesh.n_cells, 160);
        assert_eq!((&ot.mesh.points, &ot.mesh.faces, &ot.mesh.owner), (&one.points, &one.faces, &one.owner));
    }

    /// V&V (gh:#1270 remainder), prediction P3: on the pipe, each group's
    /// displacement against the one-group route on that group alone.
    #[test]
    fn patch_wise_displacements_equal_the_one_group_route_on_the_pipe() {
        let bare = bare_pipe(0.06, 0.006);
        let mut bl = BoundaryLayers::new(bare.clone(), BoundaryLayersControls::default());
        let o = bl.add_layer_for_all_patches().unwrap();
        let l = &bl.mesh;
        let nb: usize = bare.patches.iter().map(|p| p.n_faces).sum();
        let v0 = bare.total_volume();
        println!(
            "P patch-wise: groups {:?}; bare {} cells, {nb} boundary faces; {} cells (face {}, edge {}, corner {}), unmatched {}, dV/V {:.3e}, non-positive {}",
            o.groups,
            bare.n_cells,
            l.n_cells,
            o.n_face_cells,
            o.n_edge_cells,
            o.n_corner_cells,
            o.n_unmatched_boundary_faces,
            (l.total_volume() - v0) / v0,
            n_nonpositive(l)
        );
        let q = check_quality(l);
        println!("  non-ortho {:.1}, skew {:.2}, neg pyramids {}", q.max_non_orthogonality_deg, q.max_skewness, q.n_negative_pyramid_faces);
        println!("  edge classification (convex, concave) by patch pair: {:?} (patches {:?})", bl.edge_classification, bare.patches.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
        // why the rim edges are concave: the tet test and the angle, per edge
        let engine = SurfaceEngine::new(&bare);
        let (mut not_convex, mut wide, mut angles) = (0, 0, Vec::new());
        for ef in &engine.edge_faces {
            if ef.len() != 2 || engine.face_patches[ef[0]] == engine.face_patches[ef[1]] {
                continue;
            }
            let (f1, f2) = (&engine.face_rings[ef[0]], &engine.face_rings[ef[1]]);
            let a = angle_between_faces(&bare.points, f1, f2).unwrap();
            angles.push(a.to_degrees());
            not_convex += usize::from(!is_shared_edge_convex(&bare.points, f1, f2));
            wide += usize::from(a > 0.75 * std::f64::consts::PI);
        }
        angles.sort_by(f64::total_cmp);
        println!("  rim edges {}: not convex by the tet test {not_convex}, angle > 135° {wide}; angle min/median/max {:.1}/{:.1}/{:.1}°", angles.len(), angles[0], angles[angles.len() / 2], angles[angles.len() - 1]);
        let (mut n_cmp, mut n_bit) = (0, 0);
        for (key, g) in o.groups.iter().enumerate() {
            let refs: Vec<&str> = g.iter().map(|s| s.as_str()).collect();
            let (_, one) = boundary_layers_add_layer(&bare, &refs, &BoundaryLayersControls::default()).unwrap();
            for lp in &one.points {
                let pw = o.points.iter().find(|p| p.point == lp.point).expect("point in the patch-wise layer");
                let d = pw.displacements[&key];
                n_cmp += 1;
                if d == lp.moved.sub(lp.wall) {
                    n_bit += 1;
                }
            }
        }
        let (_, onew) = boundary_layers_add_layer(&bare, &["walls"], &BoundaryLayersControls::default()).unwrap();
        let mut ow = bare.clone();
        ow = boundary_layers_add_layer(&ow, &["walls"], &BoundaryLayersControls::default()).unwrap().0;
        println!("  displacements bit for bit {n_bit} of {n_cmp}; one-group walls: {} points, non-positive {}", onew.points.len(), n_nonpositive(&ow));
        assert_eq!(n_bit, n_cmp);
        assert_eq!((o.n_corner_cells, o.n_unmatched_boundary_faces), (0, 0));
        assert_eq!(l.n_cells, bare.n_cells + nb + o.n_edge_cells);
        l.validate().unwrap();
        assert!(((l.total_volume() - v0) / v0).abs() < 1e-5);
    }

    /// V&V (gh:#1270 remainder), prediction P4: `addLayerForPatch` in
    /// sequence on the six-patch box.
    #[test]
    fn add_layer_for_patch_in_sequence_on_the_box() {
        let mut bl = BoundaryLayers::new(six_patch_box(0.25), BoundaryLayersControls::default());
        bl.add_layer_for_patch("x0").unwrap().unwrap();
        let n1 = bl.mesh.n_cells;
        bl.add_layer_for_patch("y0").unwrap().unwrap();
        let n2 = bl.mesh.n_cells;
        assert!(bl.add_layer_for_patch("x0").unwrap().is_none());
        println!("x0 then y0: {n1} then {n2} cells, V {:.15}", bl.mesh.total_volume());
        assert_eq!((n1, n2), (80, 100));
        bl.mesh.validate().unwrap();
        assert!((bl.mesh.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(bl.add_layer_for_patch("nowhere"), Err(BoundaryLayersError::UnknownPatch("nowhere".into())));
    }

    /// gh:#1476: `generateBoundaryLayers` passes `patchBoundaryLayers`
    /// names verbatim; a name no patch has (a regex such as `"x.*"`) adds
    /// nothing, as upstream's `addLayerForPatch(word)`, instead of failing.
    #[test]
    fn generate_layer_skips_a_name_no_patch_has() {
        let m = six_patch_box(0.25);
        let req = LayerRequest::Patches(vec!["x.*".into(), "x0".into()]);
        let (out, g) = generate_layer(&m, &req, &BoundaryLayersControls::default(), false).unwrap();
        assert_eq!(g.per_patch.len(), 2);
        assert!(g.per_patch[0].1.is_none());
        assert_eq!(out.n_cells, 80);
    }

    /// V&V (gh:#1270 remainder), prediction P5: `addWrapperLayer`.
    #[test]
    fn wrapper_layer_has_zero_thickness() {
        let mut bl = BoundaryLayers::new(six_patch_box(0.25), BoundaryLayersControls::default());
        let n = bl.add_wrapper_layer();
        let (vols, _) = crate::checks::cell_geometry(&bl.mesh);
        let n_zero = vols.iter().filter(|v| v.abs() < 1e-15).count();
        println!("wrapper: {n} layer cells, {} cells, {n_zero} of zero volume, V {:.15}", bl.mesh.n_cells, bl.mesh.total_volume());
        assert_eq!((n, bl.mesh.n_cells, n_zero), (96, 160, 96));
        assert!((bl.mesh.total_volume() - 1.0).abs() < 1e-12);
        bl.mesh.validate().unwrap();
        assert_eq!(bl.add_wrapper_layer(), 0);
    }

    /// V&V (gh:#1270 remainder), prediction P6:
    /// `checkTopologyOfBoundaryFaces` with groups forced.
    #[test]
    fn check_topology_of_boundary_faces_on_forced_groups() {
        let m = six_patch_box(0.25);
        let mut bl = BoundaryLayers::new(m.clone(), BoundaryLayersControls::default());
        bl.analyse();
        let (x0, y0) = (m.patches.iter().position(|p| p.name == "x0").unwrap(), m.patches.iter().position(|p| p.name == "y0").unwrap());
        bl.treat_with[x0] = vec![x0, y0];
        bl.treat_with[y0] = vec![x0, y0];
        assert_eq!(bl.check_topology_of_boundary_faces(&[x0]), Ok(0));
        assert_eq!((&bl.mesh.points, &bl.mesh.faces), (&m.points, &m.faces));

        let bare = bare_pipe(0.06, 0.006);
        let mut bp = BoundaryLayers::new(bare.clone(), BoundaryLayersControls::default());
        bp.analyse();
        let (w, i) = (bare.patches.iter().position(|p| p.name == "walls").unwrap(), bare.patches.iter().position(|p| p.name == "inlet").unwrap());
        bp.treat_with[w] = vec![w.min(i), w.max(i)];
        bp.treat_with[i] = vec![w.min(i), w.max(i)];
        let r = bp.check_topology_of_boundary_faces(&[w]);
        println!("pipe, walls+inlet forced: {r:?}");
        assert!(matches!(r, Err(BoundaryLayersError::CriticalNotTested { n_decomposed }) if n_decomposed > 0));
    }

    /// `isSharedEdgeConvex` and `angleBetweenFaces` by hand: two unit
    /// squares at a convex 90° edge (angle π/2) and folded the other way
    /// (concave, 3π/2).
    #[test]
    fn edge_convexity_by_hand() {
        let p = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0)];
        // bottom face z = 0 wound down (outward -z of a body above), side
        // face y = 0 wound outward -y: convex edge (0, 1)
        let bottom = [0, 3, 2, 1];
        let side = [0, 1, 5, 4];
        assert!(is_shared_edge_convex(&p, &bottom, &side));
        assert!((angle_between_faces(&p, &bottom, &side).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let fold = [1, 7, 6, 0];
        assert!(!is_shared_edge_convex(&p, &bottom, &fold));
        println!("folded angle {}", angle_between_faces(&p, &bottom, &fold).unwrap());
        assert!((angle_between_faces(&p, &bottom, &fold).unwrap() - 1.5 * std::f64::consts::PI).abs() < 1e-12);
    }
}
