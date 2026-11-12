// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2015-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! Boundary face merging and straight-edge point removal of ESI v2506 `snappyHexMesh`,
//! translated (gh:#1467, epic #1459, round 5): `meshRefinement::mergePatchFacesUndo`,
//! `meshRefinement::mergeEdgesUndo` and the pieces they stand on.
//!
//! **Upstream (OpenFOAM.com ESI v2506):**
//! `src/mesh/snappyHexMesh/meshRefinement/meshRefinementMerge.C` (`mergePatchFacesUndo`,
//! `mergeEdgesUndo`, `doRemovePoints`, `growFaceCellFace`);
//! `src/dynamicMesh/polyTopoChange/polyTopoChange/combineFaces.C` (`getMergeSets`,
//! `regioniseFaces`, `faceNeighboursValid`, `validFace`, `convexFace`, `getOutsideFace`,
//! `setRefinement`); `removePoints.C` (`countPointUsage`, `setRefinement`);
//! `src/meshTools/polyTopoChange/polyTopoChange.C` (`getFaceOrder`, `compact`: the
//! numbering a change of the mesh leaves); `PrimitivePatchAddressing.C`
//! (`calcAddressing`), `PrimitivePatchEdgeLoops.C`, `PrimitivePatchCheck.C`
//! (`checkPointManifold`); `HashTable.C` / `HashTableCore.C` (the iteration order that
//! picks the master face of a merge set); `primitiveMeshEdges.C` (`cellEdges`).
//!
//! **What is translated:** the operation order, including the one thing that decides
//! the final NUMBERING: the master face of a set is the first face of its region in the
//! iteration order of the OpenFOAM `Map<label>` `faceRegion` (capacity a power of two
//! at least 8, identity hash, insertion at the chain head), whose insertion order
//! follows the iteration order of the `labelHashSet` of `cellEdges` (capacity 128).
//! [`FoamHashMap`] reproduces that. **What is not:** `combineFaces::setUnrefinement` and
//! `removePoints::setUnrefinement` (the restoring of merged faces whose merge made a
//! face check fail): the loops run the checks and, when anything would have to be
//! restored, return an error instead of restoring. ESI restores nothing on the flange.
//! Parallel synchronisation, face zones, coupled patches and baffles are not handled
//! (serial, no zones); a patch can be marked as a constraint patch. Opt-in; no existing
//! path calls this module.
//!
//! Numerics: plain IEEE arithmetic in upstream's grouping; the only transcendental is
//! `cos(degToRad(angle))` at the call site, through `petir::mathf::RealMath`.

use std::collections::HashMap;

use crate::check_mesh::{esi_check_mesh, esi_face_area_normal, esi_geometry, EsiCheckResult, EsiGeometry, EsiQualityControls};
use crate::snappy_hex_mesh::esi_snap::{self, mesh_edges, SnapMesh, V};

/// `ROOTVSMALL` and `VSMALL`, `SMALL` of double precision OpenFOAM.
const ROOT_V_SMALL: f64 = 1e-150;
const V_SMALL: f64 = 1e-300;
const SMALL: f64 = 1e-15;

// ---------------------------------------------------------------------------------
// OpenFOAM hash table iteration order
// ---------------------------------------------------------------------------------

/// `HashTableCore::canonicalSize`: 0 for a request below 1, else a power of two of at
/// least 8.
pub fn canonical_size(requested: usize) -> usize {
    if requested < 1 {
        return 0;
    }
    let mut p = 8usize;
    while p < requested {
        p <<= 1;
    }
    p
}

/// An OpenFOAM `HashTable<T, label>` with the identity hash `Hash<label>` of v2506: the
/// bucket of a key is `key & (capacity - 1)`, a new entry goes to the head of its
/// chain, the table doubles when `0.8*capacity < size`, a rehash walks the old buckets
/// in order and each chain from its head, and iteration is bucket by bucket, each chain
/// from its head. Only keys, not values, decide the order; this exists to reproduce it.
#[derive(Debug, Clone)]
pub struct FoamHashMap<T> {
    capacity: usize,
    size: usize,
    /// Chains with the head LAST.
    table: Vec<Vec<(usize, T)>>,
}

impl<T> FoamHashMap<T> {
    /// `HashTable(initialCapacity)`; `0` allocates nothing until the first insertion
    /// (which then sets the capacity to 128).
    pub fn new(initial_capacity: usize) -> Self {
        let capacity = canonical_size(initial_capacity);
        let mut table = Vec::new();
        table.resize_with(capacity, Vec::new);
        Self { capacity, size: 0, table }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.size
    }

    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// `clear()`: removes the entries, keeps the capacity.
    pub fn clear(&mut self) {
        for chain in self.table.iter_mut() {
            chain.clear();
        }
        self.size = 0;
    }

    /// `insert(key, value)`: false (and no change) when the key is present.
    pub fn insert(&mut self, key: usize, value: T) -> bool {
        if self.capacity == 0 {
            self.capacity = 128;
            self.table.resize_with(128, Vec::new);
        }
        let idx = key & (self.capacity - 1);
        if self.table[idx].iter().any(|(k, _)| *k == key) {
            return false;
        }
        self.table[idx].push((key, value));
        self.size += 1;
        if 0.8 * (self.capacity as f64) < self.size as f64 {
            self.set_capacity(2 * self.capacity);
        }
        true
    }

    fn set_capacity(&mut self, new_capacity: usize) {
        let new_capacity = canonical_size(new_capacity);
        if new_capacity == self.capacity {
            return;
        }
        let old = std::mem::take(&mut self.table);
        self.capacity = new_capacity;
        self.table.resize_with(new_capacity, Vec::new);
        for chain in old {
            for (k, v) in chain.into_iter().rev() {
                let idx = k & (new_capacity - 1);
                self.table[idx].push((k, v));
            }
        }
    }

    /// Value of `key`.
    pub fn get(&self, key: usize) -> Option<&T> {
        if self.capacity == 0 {
            return None;
        }
        self.table[key & (self.capacity - 1)].iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Mutable value of `key`.
    pub fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        if self.capacity == 0 {
            return None;
        }
        let idx = key & (self.capacity - 1);
        self.table[idx].iter_mut().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Entries in OpenFOAM iteration order.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        self.table.iter().flat_map(|chain| chain.iter().rev().map(|(k, v)| (*k, v)))
    }

    /// Keys in OpenFOAM iteration order.
    pub fn keys(&self) -> Vec<usize> {
        self.iter().map(|(k, _)| k).collect()
    }

    /// Mutable values in OpenFOAM iteration order.
    pub fn for_each_value_mut<F: FnMut(&mut T)>(&mut self, mut f: F) {
        for chain in self.table.iter_mut() {
            for (_, v) in chain.iter_mut().rev() {
                f(v);
            }
        }
    }
}

// ---------------------------------------------------------------------------------
// PrimitivePatch of a set of faces
// ---------------------------------------------------------------------------------

fn same_edge(a: [usize; 2], b: [usize; 2]) -> bool {
    (a[0] == b[0] && a[1] == b[1]) || (a[0] == b[1] && a[1] == b[0])
}

/// `indirectPrimitivePatch` addressing of a list of faces: local points in order of
/// first appearance, `calcAddressing` edges (internal first, in the order the walk over
/// the faces creates them, then boundary edges), `edgeFaces`, `faceEdges`, `pointEdges`.
#[derive(Debug, Clone)]
pub struct PrimPatch {
    /// Mesh label of each local point.
    pub mesh_points: Vec<usize>,
    /// Faces in local points.
    pub local_faces: Vec<Vec<usize>>,
    /// Faces of each local point, ascending.
    pub point_faces: Vec<Vec<usize>>,
    /// Edges in local points, as the face that creates them directs them.
    pub edges: Vec<[usize; 2]>,
    /// Number of internal edges (they come first).
    pub n_internal_edges: usize,
    /// Faces of each edge.
    pub edge_faces: Vec<Vec<usize>>,
    /// Edges of each face (edge `i` joins vertex `i` and `i+1`).
    pub face_edges: Vec<Vec<usize>>,
    /// Edges of each local point, ascending.
    pub point_edges: Vec<Vec<usize>>,
}

impl PrimPatch {
    /// `PrimitivePatch::calcMeshData`, `calcPointFaces` and `calcAddressing`.
    pub fn new(faces: &[&[usize]]) -> Self {
        let mut local_of: HashMap<usize, usize> = HashMap::new();
        let mut mesh_points: Vec<usize> = Vec::new();
        let mut local_faces: Vec<Vec<usize>> = Vec::with_capacity(faces.len());
        for f in faces {
            let mut lf = Vec::with_capacity(f.len());
            for &p in f.iter() {
                let l = *local_of.entry(p).or_insert_with(|| {
                    mesh_points.push(p);
                    mesh_points.len() - 1
                });
                lf.push(l);
            }
            local_faces.push(lf);
        }
        let mut point_faces: Vec<Vec<usize>> = vec![Vec::new(); mesh_points.len()];
        for (fi, f) in local_faces.iter().enumerate() {
            for &p in f {
                point_faces[p].push(fi);
            }
        }
        let face_into_edges: Vec<Vec<[usize; 2]>> = local_faces
            .iter()
            .map(|f| (0..f.len()).map(|i| [f[i], f[(i + 1) % f.len()]]).collect())
            .collect();
        let mut face_edges: Vec<Vec<i64>> = local_faces.iter().map(|f| vec![-1; f.len()]).collect();
        let mut edges: Vec<[usize; 2]> = Vec::new();
        let mut edge_faces: Vec<Vec<usize>> = Vec::new();
        for facei in 0..local_faces.len() {
            let cur_edges = &face_into_edges[facei];
            let n = local_faces[facei].len();
            let mut nei_faces: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut edge_of_nei_face: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut n_neighbours = 0usize;
            for edge_i in 0..cur_edges.len() {
                if face_edges[facei][edge_i] >= 0 {
                    continue;
                }
                let e = cur_edges[edge_i];
                let mut found = false;
                for &cur_nei in &point_faces[e[0]] {
                    if cur_nei > facei {
                        for (nei_edge_i, se) in face_into_edges[cur_nei].iter().enumerate() {
                            if same_edge(*se, e) {
                                found = true;
                                nei_faces[edge_i].push(cur_nei);
                                edge_of_nei_face[edge_i].push(nei_edge_i);
                            }
                        }
                    }
                }
                if found {
                    n_neighbours += 1;
                }
            }
            for _ in 0..n_neighbours {
                let mut next_nei: Option<usize> = None;
                let mut min_nei = local_faces.len();
                for (nf_i, nf) in nei_faces.iter().enumerate() {
                    if !nf.is_empty() && nf[0] < min_nei {
                        next_nei = Some(nf_i);
                        min_nei = nf[0];
                    }
                }
                let next_nei = next_nei.expect("error in internal edge insertion");
                let n_edges = edges.len();
                edges.push(cur_edges[next_nei]);
                face_edges[facei][next_nei] = n_edges as i64;
                let mut cur_ef = vec![facei];
                for (k, &nf) in nei_faces[next_nei].iter().enumerate() {
                    face_edges[nf][edge_of_nei_face[next_nei][k]] = n_edges as i64;
                    cur_ef.push(nf);
                }
                edge_faces.push(cur_ef);
                nei_faces[next_nei].clear();
                edge_of_nei_face[next_nei].clear();
            }
        }
        let n_internal_edges = edges.len();
        for facei in 0..local_faces.len() {
            for edge_i in 0..face_edges[facei].len() {
                if face_edges[facei][edge_i] < 0 {
                    face_edges[facei][edge_i] = edges.len() as i64;
                    edges.push(face_into_edges[facei][edge_i]);
                    edge_faces.push(vec![facei]);
                }
            }
        }
        let face_edges: Vec<Vec<usize>> =
            face_edges.into_iter().map(|v| v.into_iter().map(|x| x as usize).collect()).collect();
        let mut point_edges: Vec<Vec<usize>> = vec![Vec::new(); mesh_points.len()];
        for (ei, e) in edges.iter().enumerate() {
            point_edges[e[0]].push(ei);
            point_edges[e[1]].push(ei);
        }
        Self { mesh_points, local_faces, point_faces, edges, n_internal_edges, edge_faces, face_edges, point_edges }
    }

    /// `PrimitivePatch::calcEdgeLoops`: the boundary edge loops, each a list of local
    /// points; a loop starts at the first unvisited boundary edge and walks to the first
    /// unvisited boundary edge of each point it reaches.
    pub fn edge_loops(&self) -> Vec<Vec<usize>> {
        let n_int = self.n_internal_edges;
        let n_bdry = self.edges.len() - n_int;
        let mut loops: Vec<Vec<usize>> = Vec::new();
        if n_bdry == 0 {
            return loops;
        }
        let mut unvisited = vec![true; n_bdry];
        while let Some(bnd) = unvisited.iter().position(|&u| u) {
            let mut current_edge = bnd + n_int;
            let mut current_vert = self.edges[current_edge][0];
            let mut lp: Vec<usize> = Vec::new();
            loop {
                lp.push(current_vert);
                unvisited[current_edge - n_int] = false;
                let e = self.edges[current_edge];
                current_vert = if e[0] == current_vert { e[1] } else { e[0] };
                let next = self.point_edges[current_vert].iter().copied().find(|&ei| ei >= n_int && unvisited[ei - n_int]);
                match next {
                    Some(ei) => current_edge = ei,
                    None => break,
                }
            }
            loops.push(lp);
        }
        loops
    }

    fn visit_point_region(&self, pointi: usize, p_faces: &[usize], start_face: usize, start_edge: usize, visited: &mut [bool]) {
        let Some(index) = p_faces.iter().position(|&f| f == start_face) else {
            return;
        };
        if visited[index] {
            return;
        }
        visited[index] = true;
        let mut next_edge: Option<usize> = None;
        for &fe in &self.face_edges[start_face] {
            let e = self.edges[fe];
            if fe != start_edge && (e[0] == pointi || e[1] == pointi) {
                next_edge = Some(fe);
                break;
            }
        }
        let next_edge = next_edge.expect("no edge out of the face that uses the point");
        for &ef in &self.edge_faces[next_edge] {
            if ef != start_face {
                self.visit_point_region(pointi, p_faces, ef, next_edge, visited);
            }
        }
    }

    /// `PrimitivePatch::checkPointManifold(false, nullptr)`: true when some point uses
    /// faces that are not connected through the edges around it.
    pub fn check_point_manifold(&self) -> bool {
        for pointi in 0..self.point_faces.len() {
            let p_faces = &self.point_faces[pointi];
            let mut visited = vec![false; p_faces.len()];
            let start_edge = self.point_edges[pointi][0];
            for &ef in &self.edge_faces[start_edge] {
                self.visit_point_region(pointi, p_faces, ef, start_edge, &mut visited);
            }
            if visited.iter().any(|&v| !v) {
                return true;
            }
        }
        false
    }

    /// `combineFaces::getOutsideFace`: the single boundary loop as a face in mesh point
    /// labels, oriented like the faces of the patch. `None` when there is not exactly
    /// one loop.
    pub fn outside_face(&self) -> Option<Vec<usize>> {
        let loops = self.edge_loops();
        if loops.len() != 1 {
            return None;
        }
        let b_edge = self.n_internal_edges;
        let e = self.edges[b_edge];
        let e_faces = &self.edge_faces[b_edge];
        assert_eq!(e_faces.len(), 1, "boundary edge with {} faces", e_faces.len());
        let outside = &loops[0];
        let n = outside.len();
        let index0 = outside.iter().position(|&p| p == e[0]).expect("boundary edge not in loop");
        let index1 = outside.iter().position(|&p| p == e[1]).expect("boundary edge not in loop");
        let edge_loop_consistent = if index1 == (index0 + 1) % n {
            true
        } else if index0 == (index1 + 1) % n {
            false
        } else {
            panic!("boundary edge not on consecutive points of its loop");
        };
        let local_f = &self.local_faces[e_faces[0]];
        let index = self.face_edges[e_faces[0]].iter().position(|&x| x == b_edge).expect("edge not in face");
        let face_edge_consistent = if local_f[index] == e[0] && local_f[(index + 1) % local_f.len()] == e[1] {
            true
        } else if local_f[index] == e[1] && local_f[(index + 1) % local_f.len()] == e[0] {
            false
        } else {
            panic!("boundary edge not found in its face");
        };
        let mut mesh_face: Vec<usize> = outside.iter().map(|&l| self.mesh_points[l]).collect();
        if face_edge_consistent != edge_loop_consistent {
            mesh_face.reverse();
        }
        Some(mesh_face)
    }
}

// ---------------------------------------------------------------------------------
// The mesh with patches, and polyTopoChange's numbering
// ---------------------------------------------------------------------------------

/// A polyMesh with its boundary patches (serial, no zones).
#[derive(Debug, Clone)]
pub struct MergeMesh {
    /// Points, faces, owner, neighbour; boundary faces are grouped by patch.
    pub mesh: SnapMesh,
    /// Patch of every face, `usize::MAX` for an internal face.
    pub face_patch: Vec<usize>,
    /// Number of patches (some may be empty).
    pub n_patches: usize,
    /// `polyPatch::constraintType`: faces of such a patch are never merged.
    pub constraint: Vec<bool>,
}

impl MergeMesh {
    /// From `patch_starts` (start face of each patch, then the number of faces).
    pub fn from_patch_starts(mesh: SnapMesh, patch_starts: &[usize]) -> Self {
        let n_patches = patch_starts.len() - 1;
        let mut face_patch = vec![usize::MAX; mesh.faces.len()];
        for p in 0..n_patches {
            for f in patch_starts[p]..patch_starts[p + 1] {
                face_patch[f] = p;
            }
        }
        Self { mesh, face_patch, n_patches, constraint: vec![false; n_patches] }
    }

    /// `whichPatch`: `None` for an internal face.
    pub fn which_patch(&self, f: usize) -> Option<usize> {
        if self.face_patch[f] == usize::MAX {
            None
        } else {
            Some(self.face_patch[f])
        }
    }

    /// Start face of every patch, then the number of faces.
    pub fn patch_starts(&self) -> Vec<usize> {
        let mut starts = vec![0usize; self.n_patches + 1];
        let mut sizes = vec![0usize; self.n_patches];
        for &p in &self.face_patch {
            if p != usize::MAX {
                sizes[p] += 1;
            }
        }
        let mut s = self.mesh.n_internal_faces();
        for p in 0..self.n_patches {
            starts[p] = s;
            s += sizes[p];
        }
        starts[self.n_patches] = s;
        starts
    }
}

/// What `polyTopoChange::changeMesh(mesh, false, true)` did to the numbering.
#[derive(Debug, Clone)]
pub struct TopoMap {
    /// New label of each old face, `None` when removed.
    pub reverse_face: Vec<Option<usize>>,
    /// New label of each old point, `None` when removed.
    pub reverse_point: Vec<Option<usize>>,
}

/// A set of topology changes on a mesh, in the vocabulary of `polyModifyFace`,
/// `polyRemoveFace` and `polyRemovePoint`.
#[derive(Debug, Clone, Default)]
pub struct TopoChange {
    /// New vertex ring of a modified face (old point labels).
    pub new_ring: HashMap<usize, Vec<usize>>,
    /// New patch of a modified boundary face.
    pub new_patch: HashMap<usize, usize>,
    /// Faces to remove.
    pub remove_face: Vec<bool>,
    /// Points to remove.
    pub remove_point: Vec<bool>,
}

impl TopoChange {
    /// Empty change for `mesh`.
    pub fn new(mesh: &MergeMesh) -> Self {
        Self {
            new_ring: HashMap::new(),
            new_patch: HashMap::new(),
            remove_face: vec![false; mesh.mesh.faces.len()],
            remove_point: vec![false; mesh.mesh.points.len()],
        }
    }
}

/// `polyTopoChange::changeMesh(mesh, inflate=false, syncParallel=true)` for a change
/// that removes and modifies faces and removes points, with cells and points not
/// reordered: retained points keep their order; internal faces are renumbered by
/// (lower cell, higher cell) (`getFaceOrder`); boundary faces by patch and then by old
/// face label.
pub fn change_mesh(m: &MergeMesh, change: &TopoChange) -> (MergeMesh, TopoMap) {
    let mesh = &m.mesh;
    let mut reverse_point: Vec<Option<usize>> = vec![None; mesh.points.len()];
    let mut points: Vec<V> = Vec::new();
    for (p, &x) in mesh.points.iter().enumerate() {
        if !change.remove_point[p] {
            reverse_point[p] = Some(points.len());
            points.push(x);
        }
    }
    let n_int = mesh.n_internal_faces();
    let mut internal: Vec<(usize, usize, usize)> = Vec::new();
    for f in 0..n_int {
        if !change.remove_face[f] {
            internal.push((mesh.owner[f], mesh.neighbour[f], f));
        }
    }
    internal.sort_unstable();
    let mut order: Vec<usize> = internal.iter().map(|t| t.2).collect();
    let patch_of = |f: usize| change.new_patch.get(&f).copied().unwrap_or(m.face_patch[f]);
    for p in 0..m.n_patches {
        for f in n_int..mesh.faces.len() {
            if !change.remove_face[f] && patch_of(f) == p {
                order.push(f);
            }
        }
    }
    let mut reverse_face: Vec<Option<usize>> = vec![None; mesh.faces.len()];
    let mut faces = Vec::with_capacity(order.len());
    let mut owner = Vec::with_capacity(order.len());
    let mut neighbour = Vec::new();
    let mut face_patch = Vec::with_capacity(order.len());
    for (new, &old) in order.iter().enumerate() {
        reverse_face[old] = Some(new);
        let ring = change.new_ring.get(&old).unwrap_or(&mesh.faces[old]);
        faces.push(ring.iter().map(|&p| reverse_point[p].expect("face uses a removed point")).collect::<Vec<usize>>());
        owner.push(mesh.owner[old]);
        if old < n_int {
            neighbour.push(mesh.neighbour[old]);
            face_patch.push(usize::MAX);
        } else {
            face_patch.push(patch_of(old));
        }
    }
    let out = MergeMesh {
        mesh: SnapMesh { points, faces, owner, neighbour },
        face_patch,
        n_patches: m.n_patches,
        constraint: m.constraint.clone(),
    };
    (out, TopoMap { reverse_face, reverse_point })
}

/// Apply boundary face patch changes (`repatchToSurface`'s `polyTopoChange`): the
/// faces move to their new patch and are numbered by patch then old label.
pub fn repatch(m: &MergeMesh, changes: &[(usize, usize)]) -> (MergeMesh, TopoMap) {
    let mut c = TopoChange::new(m);
    for &(f, p) in changes {
        c.new_patch.insert(f, p);
    }
    change_mesh(m, &c)
}

// ---------------------------------------------------------------------------------
// Mesh addressing the merging reads
// ---------------------------------------------------------------------------------

/// Addressing of a mesh as `primitiveMesh` computes it.
#[derive(Debug, Clone)]
pub struct MeshTopo {
    /// `cells()`: owned faces ascending, then neighbour faces ascending.
    pub cells: Vec<Vec<usize>>,
    /// `edges()` in `calcEdges` order.
    pub edges: Vec<[usize; 2]>,
    /// `faceEdges()`: edge `i` of a face joins vertex `i` and `i+1`.
    pub face_edges: Vec<Vec<usize>>,
    /// `edgeFaces()`, ascending faces.
    pub edge_faces: Vec<Vec<usize>>,
    /// `pointFaces()`, ascending.
    pub point_faces: Vec<Vec<usize>>,
    /// Geometry (face areas and centres) as `polyMesh::movePoints` leaves it.
    pub geom: EsiGeometry,
}

impl MeshTopo {
    /// Addressing and geometry of `mesh`.
    pub fn new(mesh: &SnapMesh) -> Self {
        let me = mesh_edges(mesh);
        let geom = esi_geometry(mesh);
        let mut face_edges: Vec<Vec<usize>> = Vec::with_capacity(mesh.faces.len());
        let mut edge_faces: Vec<Vec<usize>> = vec![Vec::new(); me.edges.len()];
        for (fi, f) in mesh.faces.iter().enumerate() {
            let mut fe = Vec::with_capacity(f.len());
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                let e = me.point_edges[a]
                    .iter()
                    .copied()
                    .find(|&e| same_edge(me.edges[e], [a, b]))
                    .expect("face edge not among the mesh edges");
                fe.push(e);
                edge_faces[e].push(fi);
            }
            face_edges.push(fe);
        }
        let mut point_faces: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
        for (fi, f) in mesh.faces.iter().enumerate() {
            for &p in f {
                point_faces[p].push(fi);
            }
        }
        Self { cells: geom.cell_faces.clone(), edges: me.edges, face_edges, edge_faces, point_faces, geom }
    }
}

fn face_on_cell(mesh: &SnapMesh, celli: usize, facei: usize) -> bool {
    mesh.owner[facei] == celli || (facei < mesh.n_internal_faces() && mesh.neighbour[facei] == celli)
}

/// `meshTools::getEdgeFaces`: the first two faces of the edge that are on the cell.
fn get_edge_faces(mesh: &SnapMesh, topo: &MeshTopo, celli: usize, edge_i: usize) -> (usize, usize) {
    let mut face0: Option<usize> = None;
    for &facei in &topo.edge_faces[edge_i] {
        if face_on_cell(mesh, celli, facei) {
            match face0 {
                None => face0 = Some(facei),
                Some(f0) => return (f0, facei),
            }
        }
    }
    panic!("cannot find faces using edge {edge_i} on cell {celli}");
}

fn normalised(v: V) -> V {
    let s = esi_snap::mag(v);
    if s < ROOT_V_SMALL {
        [0.0; 3]
    } else {
        esi_snap::div(v, s)
    }
}

// ---------------------------------------------------------------------------------
// combineFaces
// ---------------------------------------------------------------------------------

/// `combineFaces::convexFace`: the face is convex up to angles whose cosine is at least
/// `min_concave_cos`.
pub fn convex_face(min_concave_cos: f64, points: &[V], f: &[usize]) -> bool {
    use esi_snap::{cross, dot, div, mag, sub};
    let area_norm = esi_face_area_normal(points, f);
    let n = f.len();
    let mut e_prev = sub(points[f[0]], points[f[n - 1]]);
    let mut mag_e_prev = mag(e_prev);
    e_prev = div(e_prev, mag_e_prev + V_SMALL);
    for fp0 in 0..n {
        let mut e10 = sub(points[f[(fp0 + 1) % n]], points[f[fp0]]);
        let mag_e10 = mag(e10);
        e10 = div(e10, mag_e10 + V_SMALL);
        if mag_e_prev > SMALL && mag_e10 > SMALL {
            let edge_normal = cross(e_prev, e10);
            if dot(edge_normal, area_norm) < 0.0 && dot(e_prev, e10) < min_concave_cos {
                return false;
            }
        }
        e_prev = e10;
        mag_e_prev = mag_e10;
    }
    true
}

/// `combineFaces::validFace`: one outside loop, point-manifold, convex.
pub fn valid_face(min_concave_cos: f64, points: &[V], faces: &[&[usize]]) -> bool {
    let patch = PrimPatch::new(faces);
    if patch.edge_loops().len() > 1 {
        return false;
    }
    if patch.check_point_manifold() {
        return false;
    }
    match patch.outside_face() {
        Some(f) => convex_face(min_concave_cos, points, &f),
        None => false,
    }
}

/// `combineFaces::regioniseFaces` for one cell, `c_edges` in the order given.
fn regionise_faces(
    m: &MergeMesh,
    topo: &MeshTopo,
    min_cos: f64,
    merge_across_patches: bool,
    celli: usize,
    c_edges: &[usize],
    face_region: &mut FoamHashMap<usize>,
) {
    for &edge_i in c_edges {
        let (f0, f1) = get_edge_faces(&m.mesh, topo, celli, edge_i);
        let a0 = topo.geom.face_areas[f0];
        let a1 = topo.geom.face_areas[f1];
        let (p0, p1) = (m.which_patch(f0), m.which_patch(f1));
        let (Some(p0), Some(p1)) = (p0, p1) else {
            continue;
        };
        if m.constraint[p0] || m.constraint[p1] {
            continue;
        }
        if !merge_across_patches && p0 != p1 {
            continue;
        }
        let f0_normal = normalised(a0);
        let f1_normal = normalised(a1);
        if esi_snap::dot(f0_normal, f1_normal) > min_cos {
            let region0 = face_region.get(f0).copied();
            let region1 = face_region.get(f1).copied();
            match (region0, region1) {
                (None, None) => {
                    let use_region = face_region.len();
                    face_region.insert(f0, use_region);
                    face_region.insert(f1, use_region);
                }
                (None, Some(r1)) => {
                    face_region.insert(f0, r1);
                }
                (Some(r0), None) => {
                    face_region.insert(f1, r0);
                }
                (Some(r0), Some(r1)) => {
                    if r0 != r1 {
                        let use_region = r0.min(r1);
                        let free_region = r0.max(r1);
                        face_region.for_each_value_mut(|v| {
                            if *v == free_region {
                                *v = use_region;
                            }
                        });
                    }
                }
            }
        }
    }
}

/// `combineFaces::faceNeighboursValid`: no face of the cell may be left with fewer
/// than three neighbours (faces and regions) by the regions.
fn face_neighbours_valid(m: &MergeMesh, topo: &MeshTopo, celli: usize, face_region: &FoamHashMap<usize>) -> bool {
    if face_region.len() <= 1 {
        return true;
    }
    for &facei in &topo.cells[celli] {
        if face_region.get(facei).is_none() {
            let mut neighbour_faces: Vec<usize> = Vec::new();
            let mut neighbour_regions: Vec<usize> = Vec::new();
            for &edge_i in &topo.face_edges[facei] {
                let (f0, f1) = get_edge_faces(&m.mesh, topo, celli, edge_i);
                let nbr = if f0 == facei { f1 } else { f0 };
                match face_region.get(nbr) {
                    Some(&r) => {
                        if !neighbour_regions.contains(&r) {
                            neighbour_regions.push(r);
                        }
                    }
                    None => {
                        if !neighbour_faces.contains(&nbr) {
                            neighbour_faces.push(nbr);
                        }
                    }
                }
            }
            if neighbour_faces.len() + neighbour_regions.len() < 3 {
                return false;
            }
        }
    }
    true
}

/// `combineFaces::getMergeSets(featureCos, minConcaveCos, boundaryCells, mergeAcrossPatches)`:
/// `boundary_cells` in the iteration order of the `labelHashSet` upstream builds
/// (see [`boundary_cells_of`]). Returns the sets of faces, the first face of a set
/// being its master.
pub fn get_merge_sets(
    m: &MergeMesh,
    topo: &MeshTopo,
    feature_cos: f64,
    min_concave_cos: f64,
    boundary_cells: &[usize],
    merge_across_patches: bool,
) -> Vec<Vec<usize>> {
    let mut all_face_sets: Vec<Vec<usize>> = Vec::new();
    let mut set: FoamHashMap<()> = FoamHashMap::new(0);
    for &celli in boundary_cells {
        let c_faces = &topo.cells[celli];
        set.clear();
        for &f in c_faces {
            for &e in &topo.face_edges[f] {
                set.insert(e, ());
            }
        }
        let c_edges = set.keys();
        let mut face_region: FoamHashMap<usize> = FoamHashMap::new(c_faces.len());
        regionise_faces(m, topo, feature_cos, merge_across_patches, celli, &c_edges, &mut face_region);
        if !face_neighbours_valid(m, topo, celli, &face_region) {
            continue;
        }
        let mut region_to_faces: FoamHashMap<Vec<usize>> = FoamHashMap::new(face_region.len());
        for (facei, &region) in face_region.iter() {
            if let Some(v) = region_to_faces.get_mut(region) {
                v.push(facei);
            } else {
                region_to_faces.insert(region, vec![facei]);
            }
        }
        for (_, face_ids) in region_to_faces.iter() {
            let refs: Vec<&[usize]> = face_ids.iter().map(|&f| m.mesh.faces[f].as_slice()).collect();
            if valid_face(min_concave_cos, &m.mesh.points, &refs) {
                let mut ids = face_ids.clone();
                if merge_across_patches {
                    let mut max_index = 0;
                    let mut max_mag_sqr = esi_snap::mag_sqr(topo.geom.face_areas[ids[0]]);
                    for i in 1..ids.len() {
                        let a2 = esi_snap::mag_sqr(topo.geom.face_areas[ids[i]]);
                        if a2 > max_mag_sqr {
                            max_mag_sqr = a2;
                            max_index = i;
                        }
                    }
                    if max_index != 0 {
                        ids.swap(0, max_index);
                    }
                }
                all_face_sets.push(ids);
            }
        }
    }
    all_face_sets
}

/// The candidate cells of `mergePatchFacesUndo`: owners of the faces of the non-coupled
/// `patch_ids`, in the iteration order of the `labelHashSet(nBoundaryFaces)` upstream
/// fills.
pub fn boundary_cells_of(m: &MergeMesh, patch_ids: &[usize]) -> Vec<usize> {
    let n_boundary = m.mesh.faces.len() - m.mesh.n_internal_faces();
    let mut set: FoamHashMap<()> = FoamHashMap::new(n_boundary);
    for &p in patch_ids {
        for f in m.mesh.n_internal_faces()..m.mesh.faces.len() {
            if m.face_patch[f] == p {
                set.insert(m.mesh.owner[f], ());
            }
        }
    }
    set.keys()
}

/// `combineFaces::setRefinement` (not undoable bookkeeping): the master face of each set
/// takes the outside ring and stays on its patch, the other faces go, and every point
/// that no remaining face uses goes.
pub fn combine_faces_set_refinement(m: &MergeMesh, topo: &MeshTopo, face_sets: &[Vec<usize>]) -> TopoChange {
    let mut change = TopoChange::new(m);
    let mut n_point_faces: Vec<i64> = topo.point_faces.iter().map(|v| v.len() as i64).collect();
    for set_faces in face_sets {
        let refs: Vec<&[usize]> = set_faces.iter().map(|&f| m.mesh.faces[f].as_slice()).collect();
        let patch = PrimPatch::new(&refs);
        let outside = patch.outside_face().expect("faces to be merged do not form a single big face");
        let master = set_faces[0];
        change.new_ring.insert(master, outside.clone());
        for &f in &set_faces[1..] {
            change.remove_face[f] = true;
        }
        for &f in set_faces {
            for &p in &m.mesh.faces[f] {
                n_point_faces[p] -= 1;
            }
        }
        for &p in &outside {
            n_point_faces[p] += 1;
        }
    }
    for (p, &n) in n_point_faces.iter().enumerate() {
        if n == 0 {
            change.remove_point[p] = true;
        }
    }
    change
}

// ---------------------------------------------------------------------------------
// removePoints
// ---------------------------------------------------------------------------------

/// `removePoints::countPointUsage`: points used by exactly two mesh edges that make a
/// cosine above `min_cos`, and points no edge uses; points of faces that would
/// collapse below three vertices are protected.
pub fn count_point_usage(m: &MergeMesh, topo: &MeshTopo, min_cos: f64) -> Vec<bool> {
    const NOT_FILLED: i64 = -1;
    const MANY: i64 = -2;
    let n_points = m.mesh.points.len();
    let mut edge0 = vec![NOT_FILLED; n_points];
    let mut edge1 = vec![NOT_FILLED; n_points];
    for (edge_i, e) in topo.edges.iter().enumerate() {
        for &pointi in e {
            if edge0[pointi] == MANY {
            } else if edge0[pointi] == NOT_FILLED {
                edge0[pointi] = edge_i as i64;
            } else if edge1[pointi] == NOT_FILLED {
                edge1[pointi] = edge_i as i64;
            } else {
                edge0[pointi] = MANY;
                edge1[pointi] = MANY;
            }
        }
    }
    let points = &m.mesh.points;
    let mut can_delete = vec![false; n_points];
    for pointi in 0..n_points {
        if edge0[pointi] >= 0 && edge1[pointi] >= 0 {
            let e0 = topo.edges[edge0[pointi] as usize];
            let e1 = topo.edges[edge1[pointi] as usize];
            let common = if e0[0] == e1[0] || e0[0] == e1[1] { e0[0] } else { e0[1] };
            let v_left = if e0[0] == common { e0[1] } else { e0[0] };
            let v_right = if e1[0] == common { e1[1] } else { e1[0] };
            let e0_vec = normalised(esi_snap::sub(points[common], points[v_left]));
            let e1_vec = normalised(esi_snap::sub(points[v_right], points[common]));
            if esi_snap::dot(e0_vec, e1_vec) > min_cos {
                can_delete[pointi] = true;
            }
        } else if edge0[pointi] == NOT_FILLED {
            can_delete[pointi] = true;
        }
    }
    for f in &m.mesh.faces {
        let mut n_collapse = f.iter().filter(|&&p| can_delete[p]).count();
        if f.len() < n_collapse + 3 {
            for &p in f {
                if can_delete[p] {
                    can_delete[p] = false;
                    n_collapse -= 1;
                    if n_collapse == 0 {
                        break;
                    }
                }
            }
        }
    }
    can_delete
}

/// `removePoints::setRefinement`: the points go and every face using one of them is
/// modified to the ring without them (same patch, same owner and neighbour).
pub fn remove_points_set_refinement(m: &MergeMesh, topo: &MeshTopo, can_delete: &[bool]) -> TopoChange {
    let mut change = TopoChange::new(m);
    let mut affected = vec![false; m.mesh.faces.len()];
    for (p, &d) in can_delete.iter().enumerate() {
        if d {
            change.remove_point[p] = true;
            for &f in &topo.point_faces[p] {
                affected[f] = true;
            }
        }
    }
    for (f, &a) in affected.iter().enumerate() {
        if a {
            let ring: Vec<usize> = m.mesh.faces[f].iter().copied().filter(|&p| !can_delete[p]).collect();
            change.new_ring.insert(f, ring);
        }
    }
    change
}

// ---------------------------------------------------------------------------------
// The drivers with the undo loop
// ---------------------------------------------------------------------------------

/// What a merging step did.
#[derive(Debug, Clone)]
pub struct MergeStats {
    /// Merge sets (`mergePatchFacesUndo`; empty for the edge step), master first, in
    /// the mesh before the merge.
    pub face_sets: Vec<Vec<usize>>,
    /// Points removed (by the faces step the unused ones, by the edge step the
    /// straight-edge points).
    pub n_points_removed: usize,
    /// Labels (in the mesh before the step) of the points removed, ascending.
    pub removed_points: Vec<usize>,
    /// Faces in error after each undo iteration (all checks, ESI's `checkMesh`).
    pub error_faces_per_iteration: Vec<usize>,
    /// The result of the last check (counts per check).
    pub last_check: Option<EsiCheckResult>,
}

/// Why a merging step could not finish.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeError {
    /// The face checks found faces in error whose merge would have to be undone
    /// (`setUnrefinement`), which is not ported. Holds the number of faces in error.
    UndoNeeded(usize),
}

fn all_faces(m: &MergeMesh) -> Vec<usize> {
    (0..m.mesh.faces.len()).collect()
}

/// `meshRefinement::mergePatchFacesUndo` (`combineFaces(mesh, true)`, no coupled
/// patches): merge the sets of boundary faces of `patch_ids`, then run
/// the nine face checks and look for masters to restore. `preserve[f]` true keeps a
/// face out of any set (`duplicateFace != -1`).
pub fn merge_patch_faces_undo(
    m: &MergeMesh,
    min_cos: f64,
    concave_cos: f64,
    patch_ids: &[usize],
    controls: &EsiQualityControls,
    preserve: &[bool],
    merge_across_patches: bool,
) -> Result<(MergeMesh, MergeStats), MergeError> {
    let topo = MeshTopo::new(&m.mesh);
    let boundary_cells = boundary_cells_of(m, patch_ids);
    let mut sets = get_merge_sets(m, &topo, min_cos, concave_cos, &boundary_cells, merge_across_patches);
    sets.retain(|s| !s.iter().any(|&f| preserve[f]));
    if sets.is_empty() {
        let stats = MergeStats { face_sets: sets, n_points_removed: 0, removed_points: vec![], error_faces_per_iteration: vec![], last_check: None };
        return Ok((m.clone(), stats));
    }
    let change = combine_faces_set_refinement(m, &topo, &sets);
    let removed_points: Vec<usize> = (0..change.remove_point.len()).filter(|&p| change.remove_point[p]).collect();
    let n_removed = removed_points.len();
    let (merged, map) = change_mesh(m, &change);
    let mut errors = Vec::new();
    let mut last_check = None;
    for _ in 0..100 {
        let geom = esi_geometry(&merged.mesh);
        let check = esi_check_mesh(&merged.mesh, &geom, controls, &all_faces(&merged));
        errors.push(check.wrong_faces.len());
        let has_errors = !check.wrong_faces.is_empty();
        let mut in_error = vec![false; merged.mesh.faces.len()];
        for &f in &check.wrong_faces {
            in_error[f] = true;
        }
        last_check = Some(check);
        if !has_errors {
            break;
        }
        let mut n_restore = 0usize;
        for set in &sets {
            if let Some(master) = map.reverse_face[set[0]] {
                let cell = merged.mesh.owner[master];
                if geom.cell_faces[cell].iter().any(|&f| in_error[f]) {
                    n_restore += 1;
                }
            }
        }
        if n_restore == 0 {
            break;
        }
        return Err(MergeError::UndoNeeded(n_restore));
    }
    Ok((merged, MergeStats { face_sets: sets, n_points_removed: n_removed, removed_points, error_faces_per_iteration: errors, last_check }))
}

/// `meshRefinement::mergeEdgesUndo`: remove the points `countPointUsage` selects,
/// then run the face checks (an error on a face that was changed would need
/// `removePoints::setUnrefinement`, not ported).
pub fn merge_edges_undo(
    m: &MergeMesh,
    min_cos: f64,
    controls: &EsiQualityControls,
) -> Result<(MergeMesh, MergeStats), MergeError> {
    let topo = MeshTopo::new(&m.mesh);
    let can_delete = count_point_usage(m, &topo, min_cos);
    let n_remove = can_delete.iter().filter(|&&x| x).count();
    if n_remove == 0 {
        let stats = MergeStats { face_sets: vec![], n_points_removed: 0, removed_points: vec![], error_faces_per_iteration: vec![], last_check: None };
        return Ok((m.clone(), stats));
    }
    let change = remove_points_set_refinement(m, &topo, &can_delete);
    let (merged, _map) = change_mesh(m, &change);
    let mut errors = Vec::new();
    let geom = esi_geometry(&merged.mesh);
    let check = esi_check_mesh(&merged.mesh, &geom, controls, &all_faces(&merged));
    errors.push(check.wrong_faces.len());
    if !check.wrong_faces.is_empty() {
        return Err(MergeError::UndoNeeded(check.wrong_faces.len()));
    }
    Ok((merged, MergeStats { face_sets: vec![], n_points_removed: n_remove, removed_points: (0..can_delete.len()).filter(|&p| can_delete[p]).collect(), error_faces_per_iteration: errors, last_check: Some(check) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Iteration order of a capacity-8 table (identity hash, head insertion): by
    /// `key & 7`, within a bucket the newest first.
    #[test]
    fn hash_map_iteration_order_follows_bucket_then_chain_head() {
        let mut m: FoamHashMap<()> = FoamHashMap::new(6);
        for k in [9usize, 1, 17, 2, 8] {
            m.insert(k, ());
        }
        // capacity 8: 9 and 1 and 17 share bucket 1 (head = last inserted = 17)
        assert_eq!(m.keys(), vec![8, 17, 1, 9, 2]);
        assert!(!m.insert(1, ()));
        let mut g: FoamHashMap<()> = FoamHashMap::new(0);
        g.insert(130, ());
        g.insert(2, ());
        assert_eq!(g.keys(), vec![2, 130]);
        assert_eq!(canonical_size(0), 0);
        assert_eq!(canonical_size(7), 8);
        assert_eq!(canonical_size(12136), 16384);
    }

    /// Growth: the seventh entry of a capacity-8 table (0.8*8 < 7) doubles it to 16
    /// and the rehash reverses chains that share a bucket.
    #[test]
    fn hash_map_growth_rehashes_from_chain_heads() {
        let mut m: FoamHashMap<()> = FoamHashMap::new(8);
        for k in [1usize, 9, 17, 3, 4, 5] {
            m.insert(k, ());
        }
        assert_eq!(m.keys(), vec![17, 9, 1, 3, 4, 5]);
        m.insert(25, ()); // size 7 > 6.4: capacity 16
        // old bucket 1, head first: 25, 17, 9, 1. Walked in that order with head
        // insertion into 16 buckets: bucket 9 = [9, 25], bucket 1 = [1, 17].
        assert_eq!(m.keys(), vec![1, 17, 3, 4, 5, 9, 25]);
    }

    /// Two unit squares sharing an edge: one internal edge, a single six-vertex loop.
    #[test]
    fn prim_patch_two_squares_has_one_loop_and_outside_face() {
        // points: 0 1 2 / 3 4 5 on a 2x1 strip
        let f0: Vec<usize> = vec![0, 3, 4, 1];
        let f1: Vec<usize> = vec![1, 4, 5, 2];
        let p = PrimPatch::new(&[&f0, &f1]);
        assert_eq!(p.n_internal_edges, 1);
        assert_eq!(p.edges.len(), 7);
        let loops = p.edge_loops();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 6);
        assert!(!p.check_point_manifold());
        let out = p.outside_face().unwrap();
        assert_eq!(out.len(), 6);
        // orientation follows the faces: the outside ring contains 0, 3 in the same sense
        let i0 = out.iter().position(|&x| x == 0).unwrap();
        assert_eq!(out[(i0 + 1) % 6], 3);
        let pts: Vec<V> = vec![
            [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0],
        ];
        assert!(convex_face(0.5, &pts, &out));
        assert!(valid_face(0.5, &pts, &[&f0, &f1]));
    }

    /// Two squares touching only at a corner are not a single loop: not mergeable.
    #[test]
    fn prim_patch_corner_touch_is_not_a_valid_face() {
        let f0: Vec<usize> = vec![0, 1, 2, 3];
        let f1: Vec<usize> = vec![2, 4, 5, 6];
        let pts: Vec<V> = (0..7).map(|i| [i as f64, 0.0, 0.0]).collect();
        assert!(!valid_face(0.5, &pts, &[&f0, &f1]));
    }

    /// A concave L-shape of three squares is rejected by `convexFace`.
    #[test]
    fn concave_l_shape_is_not_convex() {
        // L: (0,0)(2,0)(2,1)(1,1)(1,2)(0,2), counter-clockwise
        let pts: Vec<V> = vec![
            [0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 2.0, 0.0], [0.0, 2.0, 0.0],
        ];
        let l: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        assert!(!convex_face(0.5, &pts, &l));
        let sq: Vec<usize> = vec![0, 1, 2, 5];
        assert!(convex_face(0.5, &pts, &sq));
    }
}
