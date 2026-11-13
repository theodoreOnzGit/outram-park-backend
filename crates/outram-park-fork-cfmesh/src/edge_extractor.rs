// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/surfaceTools/edgeExtraction/edgeExtractor/
//     edgeExtractor.{C,H}       (serial parts; see the module docs)
//     edgeExtractorCorners.C    (faceEvaluator; see the module docs)
//   meshLibrary/utilities/helperFunctions/helperFunctionsTopologyManipulationI.H
//     (help::areElementsInChain)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **`edgeExtractor`** (gh:#1269): the `patchAssignment` step of the
//! `tetMesh`, `pMesh` and `cartesianMesh` workflows. It distributes the
//! boundary faces of a volume mesh, already projected onto the surface, into
//! the surface's patches, and refines that distribution so the patch borders
//! can be mapped onto the surface's feature edges.
//!
//! The workflows call, in this order (`tetMeshGenerator::extractPatches`):
//!
//! 1. the constructor: `calculateValence`, `calculateSingleCellEdge`,
//!    `findPatchesNearSurfaceFace`, `findFeatureEdgesNearEdge`;
//! 2. `extractEdges`:
//!    - `distributeBoundaryFaces`: each boundary point and face to the
//!      patch of the surface point nearest to it (the face by its OpenFOAM
//!      centre), through the octree's `findNearestSurfacePoint`;
//!    - `distributeBoundaryFacesNormalAlignment` (at most 5 passes): a
//!      face next to another patch moves to the neighbouring patch whose
//!      nearest triangle best matches its normal, weighted by
//!      `sqrt(maxDSq / dSq)`;
//!    - `checkFacePatchesTopology` (at most 4 passes): a face moves to the
//!      patch most of its edges border, if those edges form one chain and
//!      its points are not all mapped into its own patch; the
//!      `faceEvaluator` then undoes moves that would oscillate;
//!    - `checkFacePatchesGeometry` (at most 4 mappings): the points are
//!      mapped onto the patches (`meshSurfaceMapper::
//!      mapVerticesOntoSurfacePatches`), inverted points are untangled
//!      (`meshSurfaceOptimizer::untangleSurface`, one layer), and the faces
//!      at inverted points move to the patch of least deformation energy;
//! 3. `updateMeshPatches`: `replaceBoundary` with one patch per surface
//!    patch (empty ones kept, as upstream), types `patch`.
//!
//! Also ported (public upstream, no caller in `meshLibrary/`):
//! `moveVerticesTowardsDiscontinuities`, `projectDeterminedFeatureVertices`,
//! `untangleSurface`, `surfaceWithPatches` (both), `markPatchPoints`.
//!
//! **Not ported**, with the reason:
//! - `findEdgeCandidates`: no caller; it only prints its result, needs
//!   `triSurfacePartitioner` (not ported), and indexes the mesh's edge list
//!   with surface edge labels (`sp[edges[seI].start()]`, an upstream defect).
//! - `checkConcaveEdgeCells`: no caller; needs `triSurfaceClassifyEdges`
//!   (not ported).
//! - `findCornerCandidates`: no caller; it discards what it computes and
//!   always returns `false`; needs `triSurfacePartitioner`.
//! - `checkCorners`: no caller; it writes debug surfaces and calls
//!   `::exit(0)` part-way (`edgeExtractorCorners.C` line 1173): unfinished
//!   upstream code that ends the process.
//! - `cornerEvaluator` (`improveCorners`, `sortedFacesAtPoint`): never
//!   constructed anywhere upstream.
//! - every `Pstream::parRun()` branch (MPI).
//!
//! # Where the port has to choose
//!
//! - **Undefined reads.** Upstream reads unset entries in two places, both
//!   counted in [`EdgeExtractorReport`] (zero on every recorded run):
//!   `neiPatches` of an edge without exactly two faces (left uninitialised in
//!   serial; here [`NO_PATCH`], skipped where upstream would use it), and
//!   the nearest triangle of a region search that found nothing
//!   (`findNearestSurfacePointInRegion` leaves it unset; here the patch gets
//!   alignment 0).
//! - **`Map<label>` iteration order.** `checkFacePatchesTopology` and
//!   `faceEvaluator::bestPatchTopological` pick among tied patches in
//!   OpenFOAM `HashTable` order. [`foam_map_order`] emulates it with an
//!   identity label hash, power-of-two capacity (at least 8), insertion at
//!   the head of a bucket, and doubling above 0.8 load; with fewer than 8
//!   patches that is ascending patch order. **Not re-checked** against an
//!   OpenFOAM build.
//! - **The live `newBoundaryPatches`.** `faceEvaluator` holds a pointer to
//!   the list the loop writes into, so later faces see earlier decisions;
//!   upstream's OpenMP loop in `checkFacePatchesTopology` races on it. The
//!   serial order (ascending boundary face) is used.
//! - **Face normals** are Newell area vectors
//!   ([`crate::surface_engine::ring_area_vector`]), analytically OpenFOAM's
//!   `face::normal`; centres are OpenFOAM's `face::centre`
//!   ([`foam_face_centre`]).
//! - **Mapping and untangling** use the crate's ports (the brute-force
//!   [`SurfaceQueries`] the mapper and untangler already use); the
//!   distribution queries use the octree as upstream does.

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::mesh_octree::{mag_sqr, BoundBox, MeshOctree, TriSurf, SMALL, VGREAT, VSMALL};
use crate::mesh_octree_queries::{NearestSearch, TriangleProjection};
use crate::patches::SurfaceRegions;
use crate::surface_engine::{ring_area_vector, SurfaceEngine, SurfacePartitioner, NOT_BOUNDARY};
use crate::surface_mapper::{map_points_onto_patches, SurfaceQueries};
use crate::surface_optimizer::{check_inverted_vertices, MeshSurfaceOptimizer};
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use petir::mathf::RealMath;
use std::collections::BTreeSet;

/// A patch upstream leaves unset (see the module docs).
pub const NO_PATCH: usize = usize::MAX;

/// `edgeExtractor::edgeClassifier_` flags of a boundary edge.
pub mod edge_class {
    /// `NONE`.
    pub const NONE: u8 = 0;
    /// `SINGLECELLEDGE`: both faces of the edge belong to one cell.
    pub const SINGLECELLEDGE: u8 = 1;
    /// `FITSATCONVEXEDGE`.
    pub const FITSATCONVEXEDGE: u8 = 2;
    /// `FITSATCONCAVEEDGE`.
    pub const FITSATCONCAVEEDGE: u8 = 4;
    /// `ATTACHEDTOCORNER`.
    pub const ATTACHEDTOCORNER: u8 = 8;
    /// `CANDIDATE`.
    pub const CANDIDATE: u8 = 16;
}

/// What [`EdgeExtractor::extract_edges`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EdgeExtractorReport {
    /// Faces moved per pass of `distributeBoundaryFacesNormalAlignment`.
    pub normal_alignment_moves: Vec<usize>,
    /// `checkFacePatchesTopology`: faces proposed per pass (`nCorrected`).
    pub topology_proposed: Vec<usize>,
    /// `checkFacePatchesTopology`: faces whose patch differs after the
    /// `faceEvaluator`, per pass.
    pub topology_kept: Vec<usize>,
    /// `checkFacePatchesGeometry`: inverted points found per pass.
    pub geometry_inverted: Vec<usize>,
    /// `checkFacePatchesGeometry`: faces moved per pass (after the
    /// evaluator).
    pub geometry_moves: Vec<usize>,
    /// Return values of the two checks (upstream's `Info` lines).
    pub topology_changed: bool,
    /// `checkFacePatchesGeometry`'s return value (upstream returns `false`
    /// when a pass finds no inverted point, even after earlier changes).
    pub geometry_changed: bool,
    /// Reads of the patch over an edge without two faces (unset upstream).
    pub n_unset_neighbour_patch_reads: usize,
    /// Region searches that found no triangle (unset triangle upstream).
    pub n_failed_region_searches: usize,
    /// Faces whose final patch differs from `distributeBoundaryFaces`'.
    pub n_faces_moved_from_initial: usize,
}

/// cfMesh `edgeExtractor` (serial) on a volume mesh, the surface given by
/// the generator's octree (see the module docs). The struct owns the mesh
/// while it works; [`EdgeExtractor::update_mesh_patches`] returns it.
#[derive(Debug, Clone)]
pub struct EdgeExtractor {
    /// The mesh (points move in `checkFacePatchesGeometry`).
    pub mesh: VolumeMesh,
    /// Its surface (`meshSurfaceEngine`), geometry kept current.
    pub engine: SurfaceEngine,
    queries: SurfaceQueries,
    names: Vec<String>,
    /// The surface patches' `geometricType()` (gh:#1483), parallel to `names`.
    types: Vec<String>,
    /// Boundary faces at each boundary point (`pointValence_`).
    pub point_valence: Vec<usize>,
    /// Patch of the surface point nearest each boundary point
    /// (`pointPatch_`).
    pub point_patch: Vec<usize>,
    /// Patch of each boundary face (`facePatch_`).
    pub face_patch: Vec<usize>,
    /// Cells at each boundary edge (`nCellsAtEdge_`).
    pub n_cells_at_edge: Vec<usize>,
    /// [`edge_class`] flags of each boundary edge (`edgeType_`).
    pub edge_type: Vec<u8>,
    /// Surface patches near each boundary face (`patchesNearFace_`).
    pub patches_near_face: Vec<Vec<usize>>,
    /// Surface feature edges near each boundary edge
    /// (`featureEdgesNearEdge_`).
    pub feature_edges_near_edge: Vec<Vec<usize>>,
    /// What was done.
    pub report: EdgeExtractorReport,
}

/// `help::areElementsInChain`: the selected entries of the cyclic list
/// `sel` form one contiguous run.
pub fn are_elements_in_chain(sel: &[bool]) -> bool {
    let n = sel.len();
    let mut in_chain = vec![false; n];
    for e in 0..n {
        if sel[e] {
            in_chain[e] = true;
            loop {
                let mut found = false;
                for j in 0..n {
                    if !in_chain[j] && sel[j] && (in_chain[(j + 1) % n] || in_chain[(j + n - 1) % n]) {
                        found = true;
                        in_chain[j] = true;
                    }
                }
                if !found {
                    break;
                }
            }
            break;
        }
    }
    (0..n).all(|e| !sel[e] || in_chain[e])
}

/// The iteration order of an OpenFOAM `Map<label>` built as
/// `Map<label> m(requested); for k in keys: m.insert(k, 0)` (see the module
/// docs): identity hash, capacity a power of two of at least 8, new
/// entries at the head of their bucket, capacity doubled when the load
/// exceeds 0.8 (entries moved bucket by bucket, each to the head of its new
/// bucket). Duplicate keys are not inserted twice.
pub fn foam_map_order(requested: usize, keys: &[usize]) -> Vec<usize> {
    let mut cap = 8usize;
    while cap < requested {
        cap *= 2;
    }
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); cap];
    let mut size = 0usize;
    for &k in keys {
        if buckets[k & (cap - 1)].contains(&k) {
            continue;
        }
        buckets[k & (cap - 1)].insert(0, k);
        size += 1;
        if size as f64 / cap as f64 > 0.8 {
            let old = std::mem::take(&mut buckets);
            cap *= 2;
            buckets = vec![Vec::new(); cap];
            for b in old {
                for x in b {
                    buckets[x & (cap - 1)].insert(0, x);
                }
            }
        }
    }
    buckets.into_iter().flatten().collect()
}

/// `appendIfNotIn`.
fn push_unique(v: &mut Vec<usize>, x: usize) {
    if !v.contains(&x) {
        v.push(x);
    }
}

/// The count-and-pick of `checkFacePatchesTopology` and
/// `bestPatchTopological`: the patch most entries of `nei` hold, ties going
/// to `current` and otherwise to the first in `Map` order. Returns the patch
/// (`None` when nothing was counted) and its count.
fn dominant_patch(nei: &[usize], all: &[usize], current: usize) -> (Option<usize>, usize) {
    let mut new_patch = None;
    let mut n_nei_edges = 0usize;
    for k in foam_map_order(all.len(), all) {
        let c = nei.iter().filter(|&&x| x == k).count();
        if c > n_nei_edges {
            new_patch = Some(k);
            n_nei_edges = c;
        } else if c == n_nei_edges && k == current {
            new_patch = Some(k);
        }
    }
    (new_patch, n_nei_edges)
}

impl EdgeExtractor {
    /// `edgeExtractor(mesh, octree)`: the constructor's four calculations.
    /// `regions` names the octree surface's regions (its `patches()`).
    pub fn new(mesh: VolumeMesh, octree: &MeshOctree, regions: &SurfaceRegions) -> Result<Self, String> {
        let surf = &octree.surface;
        if regions.region_of_tri != surf.regions {
            return Err("edgeExtractor: the regions do not label the octree's surface".into());
        }
        let engine = SurfaceEngine::new(&mesh);
        let queries = SurfaceQueries::new(&surf.points, &surf.triangles, regions).with_octree(std::sync::Arc::new(octree.clone()));
        let mut ex = EdgeExtractor {
            engine,
            queries,
            names: regions.names.clone(),
            types: regions.types_for(regions.names.len()),
            point_valence: Vec::new(),
            point_patch: Vec::new(),
            face_patch: Vec::new(),
            n_cells_at_edge: Vec::new(),
            edge_type: Vec::new(),
            patches_near_face: Vec::new(),
            feature_edges_near_edge: Vec::new(),
            report: EdgeExtractorReport::default(),
            mesh,
        };
        ex.calculate_valence();
        ex.calculate_single_cell_edge();
        ex.find_patches_near_surface_face(octree);
        ex.find_feature_edges_near_edge(octree);
        Ok(ex)
    }

    fn n_patches(&self) -> usize {
        self.names.len()
    }

    /// `calculateValence`.
    fn calculate_valence(&mut self) {
        let e = &self.engine;
        let mut v = vec![0usize; e.n_boundary_points()];
        for ring in &e.face_rings {
            for &p in ring {
                v[e.bp[p]] += 1;
            }
        }
        self.point_valence = v;
    }

    /// `calculateSingleCellEdge`: `SINGLECELLEDGE` and `nCellsAtEdge_`.
    fn calculate_single_cell_edge(&mut self) {
        let e = &self.engine;
        let m = &self.mesh;
        let mut et = vec![edge_class::NONE; e.edges.len()];
        for (ei, ef) in e.edge_faces.iter().enumerate() {
            if ef.len() == 2 && e.face_owners[ef[0]] == e.face_owners[ef[1]] {
                et[ei] |= edge_class::SINGLECELLEDGE;
            }
        }
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); m.n_cells];
        for f in 0..m.face_count() {
            cells[m.owner[f]].push(f);
            if let Some(n) = m.neighbour[f] {
                cells[n].push(f);
            }
        }
        let mut n_at = vec![0usize; e.edges.len()];
        for c in &cells {
            let mut found: Vec<usize> = Vec::new();
            for &f in c {
                let ring = &m.faces[f];
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    let bps = e.bp[a];
                    if bps == NOT_BOUNDARY {
                        continue;
                    }
                    for &be in &e.point_edges[bps] {
                        let [x, y] = e.edges[be];
                        if ((x == a && y == b) || (x == b && y == a)) && !found.contains(&be) {
                            found.push(be);
                            n_at[be] += 1;
                        }
                    }
                }
            }
        }
        self.edge_type = et;
        self.n_cells_at_edge = n_at;
    }

    /// `findPatchesNearSurfaceFace`.
    fn find_patches_near_surface_face(&mut self, octree: &MeshOctree) {
        let pts = &self.mesh.points;
        let regions = &octree.surface.regions;
        self.patches_near_face = self
            .engine
            .face_rings
            .iter()
            .map(|ring| {
                let c = foam_face_centre(pts, ring);
                let mut d = 0.0f64;
                for &p in ring {
                    d = d.max(c.sub(pts[p]).length());
                }
                d = 2.0 * d + VSMALL;
                let h = Vec3::new(d, d, d);
                let mut near = Vec::new();
                for t in octree.find_triangles_in_box(&BoundBox::new(c.sub(h), c.add(h))) {
                    push_unique(&mut near, regions[t]);
                }
                near
            })
            .collect();
    }

    /// `findFeatureEdgesNearEdge` (duplicates removed with DynList's
    /// swap-with-last, as upstream).
    fn find_feature_edges_near_edge(&mut self, octree: &MeshOctree) {
        let pts = &self.mesh.points;
        self.feature_edges_near_edge = self
            .engine
            .edges
            .iter()
            .map(|&[a, b]| {
                let c = pts[a].add(pts[b]).scale(0.5);
                let d = 1.5 * pts[b].sub(pts[a]).length();
                let h = Vec3::new(d, d, d);
                let mut near = octree.find_edges_in_box(&BoundBox::new(c.sub(h), c.add(h)));
                let mut i = near.len();
                while i > 0 {
                    i -= 1;
                    let pos = near.iter().position(|&x| x == near[i]).unwrap_or(i);
                    if pos < i {
                        let last = near.len() - 1;
                        near.swap(i, last);
                        near.pop();
                    }
                }
                near
            })
            .collect();
    }

    /// `markPatchPoints`: `true` for a boundary point whose faces are all
    /// in one patch (`facePatch_`), `false` at a patch border or a
    /// non-manifold edge.
    pub fn mark_patch_points(&self) -> Vec<bool> {
        let e = &self.engine;
        let mut pp = vec![true; e.n_boundary_points()];
        for (be, ef) in e.edge_faces.iter().enumerate() {
            let border = match ef.len() {
                2 => self.face_patch[ef[0]] != self.face_patch[ef[1]],
                1 => false, // a parallel interface: MPI only
                _ => true,
            };
            if border {
                let [a, b] = e.edges[be];
                pp[e.bp[a]] = false;
                pp[e.bp[b]] = false;
            }
        }
        pp
    }

    /// The patch across each edge of boundary face `bf` under `fp`
    /// (`findNeiPatches`, `faceEvaluator::neiPatchesOverEdges`);
    /// [`NO_PATCH`] across an edge without two faces, counted.
    fn nei_patches(&mut self, bf: usize, fp: &[usize]) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.engine.face_edges[bf].len());
        for &be in &self.engine.face_edges[bf] {
            let ef = &self.engine.edge_faces[be];
            if ef.len() == 2 {
                let nei = if ef[0] == bf { ef[1] } else { ef[0] };
                out.push(fp[nei]);
            } else {
                self.report.n_unset_neighbour_patch_reads += 1;
                out.push(NO_PATCH);
            }
        }
        out
    }

    /// `findFaceCandidates`: the boundary faces with neighbours over their
    /// edges in more than one patch.
    pub fn find_face_candidates(&self, fp: &[usize]) -> Vec<usize> {
        let e = &self.engine;
        (0..e.n_boundary_faces())
            .filter(|&bf| {
                let mut all = Vec::new();
                for &be in &e.face_edges[bf] {
                    let ef = &e.edge_faces[be];
                    if ef.len() == 2 {
                        let nei = if ef[0] == bf { ef[1] } else { ef[0] };
                        push_unique(&mut all, fp[nei]);
                    }
                }
                all.len() > 1
            })
            .collect()
    }

    /// `findNearestSurfacePointInRegion` through the octree; `None` counted.
    fn nearest_in_region(&mut self, octree: &MeshOctree, p: Vec3, region: usize) -> Option<(Vec3, f64, usize)> {
        match octree.find_nearest_surface_point_with(p, NearestSearch::Upstream, TriangleProjection::Upstream, Some(region)) {
            Some(h) => Some((h.point, h.dist_sq, h.triangle)),
            None => {
                self.report.n_failed_region_searches += 1;
                None
            }
        }
    }

    /// `distributeBoundaryFaces`: each boundary point and face (by its
    /// OpenFOAM centre) to the patch of the nearest surface point. An error
    /// where upstream stops ("Cannot distribute a face ...").
    pub fn distribute_boundary_faces(&mut self, octree: &MeshOctree) -> Result<(), String> {
        let n_patches = self.n_patches();
        let pts = &self.mesh.points;
        let near = |p: Vec3| -> Result<usize, String> {
            match octree.find_nearest_surface_point(p) {
                Some(h) if h.region < n_patches => Ok(h.region),
                _ => Err(format!("edgeExtractor: cannot distribute {p:?} into any surface patch")),
            }
        };
        let mut point_patch = Vec::with_capacity(self.engine.n_boundary_points());
        for &v in &self.engine.boundary_points {
            point_patch.push(near(pts[v])?);
        }
        let mut face_patch = Vec::with_capacity(self.engine.n_boundary_faces());
        for ring in &self.engine.face_rings {
            face_patch.push(near(foam_face_centre(pts, ring))?);
        }
        self.point_patch = point_patch;
        self.face_patch = face_patch;
        Ok(())
    }

    /// `distributeBoundaryFacesNormalAlignment` (at most 5 passes).
    pub fn distribute_boundary_faces_normal_alignment(&mut self, octree: &MeshOctree) -> bool {
        let mut changed = false;
        let surf = octree.surface.clone();
        for _ in 0..5 {
            let mut new_patches = self.face_patch.clone();
            let mut n_corrected = 0;
            let fp = self.face_patch.clone();
            for bf in self.find_face_candidates(&fp) {
                let nei = self.nei_patches(bf, &fp);
                let mut all = Vec::new();
                for &p in &nei {
                    if p != NO_PATCH {
                        push_unique(&mut all, p);
                    }
                }
                if all.len() == 1 && all[0] == fp[bf] {
                    continue;
                }
                let ring = self.engine.face_rings[bf].clone();
                let c = foam_face_centre(&self.mesh.points, &ring);
                let mut fnrm = ring_area_vector(&self.mesh.points, &ring);
                fnrm = fnrm.scale(1.0 / (fnrm.length() + SMALL));
                let mut align = vec![0.0; all.len()];
                let mut dist = vec![VGREAT; all.len()];
                let mut max_d = 0.0f64;
                for (i, &patch) in all.iter().enumerate() {
                    if let Some((_, d, t)) = self.nearest_in_region(octree, c, patch) {
                        max_d = max_d.max(d);
                        let tr = surf.triangles[t];
                        let sp = &surf.points;
                        let mut tn = sp[tr[1]].sub(sp[tr[0]]).cross(sp[tr[2]].sub(sp[tr[0]])).scale(0.5);
                        tn = tn.scale(1.0 / (tn.length() + VSMALL));
                        align[i] = tn.dot(fnrm).abs();
                        dist[i] = d;
                    } else {
                        // upstream: dSq stays VGREAT, the triangle unset
                        max_d = max_d.max(VGREAT);
                    }
                }
                let mut best = 0.0;
                let mut new_patch = None;
                for i in 0..all.len() {
                    let metric = (max_d / (dist[i] + VSMALL)).sqrt() * align[i];
                    if metric > best {
                        best = metric;
                        new_patch = Some(all[i]);
                    }
                }
                if let Some(np) = new_patch {
                    if np != fp[bf] {
                        new_patches[bf] = np;
                        n_corrected += 1;
                    }
                }
            }
            self.report.normal_alignment_moves.push(n_corrected);
            if n_corrected == 0 {
                break;
            }
            changed = true;
            self.face_patch = new_patches;
        }
        changed
    }

    /// `faceEvaluator::bestPatchTopological(neiPatches, currentPatch)`.
    pub fn best_patch_topological(nei: &[usize], current: usize) -> usize {
        let mut all = Vec::new();
        for &p in nei {
            if p != NO_PATCH {
                push_unique(&mut all, p);
            }
        }
        if all.len() == 1 && all[0] == current {
            return current;
        }
        let (new_patch, n_nei_edges) = dominant_patch(nei, &all, current);
        if n_nei_edges + 1 < nei.len() {
            return current;
        }
        let Some(np) = new_patch else { return current };
        if np == current {
            return current;
        }
        let shared: Vec<bool> = nei.iter().map(|&p| p == np).collect();
        if are_elements_in_chain(&shared) {
            np
        } else {
            current
        }
    }

    /// `faceEvaluator::bestPatchAfterModification(bf)` with the evaluator's
    /// `newBoundaryPatches` being `new` (read live, see the module docs).
    fn best_patch_after_modification(&mut self, bf: usize, new: &[usize]) -> usize {
        let patch = new[bf];
        if patch == self.face_patch[bf] {
            return patch;
        }
        let old_fp = self.face_patch.clone();
        let mut new_nei = self.nei_patches(bf, new);
        let old_nei = self.nei_patches(bf, &old_fp);
        for (i, &be) in self.engine.face_edges[bf].iter().enumerate() {
            let ef = &self.engine.edge_faces[be];
            if ef.len() == 2 {
                let nf = if ef[0] == bf { ef[1] } else { ef[0] };
                if nf > bf && new_nei[i] != old_nei[i] {
                    new_nei[i] = old_nei[i];
                }
            }
            // an edge without two faces: neiProcs is this processor, so the
            // patch is kept (serial)
        }
        Self::best_patch_topological(&new_nei, patch)
    }

    /// `checkFacePatchesTopology` (at most 4 passes).
    pub fn check_face_patches_topology(&mut self) -> bool {
        let mut changed = false;
        let mut n_iter = 0;
        loop {
            let fp = self.face_patch.clone();
            let mut new = fp.clone();
            let mut n_corrected = 0;
            let candidates = self.find_face_candidates(&fp);
            for &bf in &candidates {
                let ring = &self.engine.face_rings[bf];
                if ring.iter().all(|&v| self.point_patch[self.engine.bp[v]] == fp[bf]) {
                    continue;
                }
                let nei = self.nei_patches(bf, &fp);
                let mut all = Vec::new();
                for &p in &nei {
                    if p != NO_PATCH {
                        push_unique(&mut all, p);
                    }
                }
                if all.len() == 1 && all[0] == fp[bf] {
                    continue;
                }
                let (new_patch, _) = dominant_patch(&nei, &all, fp[bf]);
                let Some(np) = new_patch else { continue };
                if np == fp[bf] {
                    continue;
                }
                let shared: Vec<bool> = nei.iter().map(|&p| p == np).collect();
                if are_elements_in_chain(&shared) {
                    n_corrected += 1;
                    new[bf] = np;
                }
            }
            self.report.topology_proposed.push(n_corrected);
            if n_corrected != 0 {
                for &bf in &candidates {
                    let best = self.best_patch_after_modification(bf, &new);
                    new[bf] = best;
                }
                self.report.topology_kept.push((0..new.len()).filter(|&i| new[i] != fp[i]).count());
                changed = true;
                self.face_patch = new;
            }
            // `while (nCorrected != 0 && (nIter++ < 3))`
            let go_on = n_corrected != 0 && n_iter < 3;
            if n_corrected != 0 {
                n_iter += 1;
            }
            if !go_on {
                break;
            }
        }
        self.report.topology_changed = changed;
        changed
    }

    /// `calculateDeformationMetricForEdge`.
    fn deformation_metric_for_edge(&mut self, octree: &MeshOctree, be: usize, p0: usize, p1: usize) -> f64 {
        let [a, b] = self.engine.edges[be];
        let (ps, pe) = (self.mesh.points[a], self.mesh.points[b]);
        let mut ev = pe.sub(ps);
        let mag_e = ev.length() + VSMALL;
        ev = ev.scale(1.0 / mag_e);
        let patches = [p0, p1];
        let (mps, d_s) = octree.find_nearest_point_to_patches(ps, &patches, 1e-4).unwrap_or((ps, 0.0));
        let (mpe, d_e) = octree.find_nearest_point_to_patches(pe, &patches, 1e-4).unwrap_or((pe, 0.0));
        let mut fv = mpe.sub(mps);
        fv = fv.scale(1.0 / (fv.length() + VSMALL));
        let c = fv.dot(ev).clamp(-1.0, 1.0);
        let angle = c.r_acos();
        0.5 * (d_s.sqrt() + d_e.sqrt()) + mag_e * angle
    }

    /// `calculateAlignmentForEdge`: `0.5 (1 + e·f)` for the boundary edge
    /// and its image on the feature edge between `p0` and `p1`.
    pub fn alignment_for_edge(&self, octree: &MeshOctree, be: usize, p0: usize, p1: usize) -> f64 {
        let [a, b] = self.engine.edges[be];
        let (ps, pe) = (self.mesh.points[a], self.mesh.points[b]);
        let mut ev = pe.sub(ps);
        ev = ev.scale(1.0 / (ev.length() + VSMALL));
        let patches = [p0, p1];
        let (mps, _) = octree.find_nearest_point_to_patches(ps, &patches, 1e-4).unwrap_or((ps, 0.0));
        let (mpe, _) = octree.find_nearest_point_to_patches(pe, &patches, 1e-4).unwrap_or((pe, 0.0));
        let mut fv = mpe.sub(mps);
        fv = fv.scale(1.0 / (fv.length() + VSMALL));
        (0.5 * (1.0 + ev.dot(fv))).clamp(0.0, 1.0)
    }

    /// `calculateDeformationMetricForFace` with the face in `patch0`.
    fn deformation_metric_for_face(&mut self, octree: &MeshOctree, bf: usize, nei: &[usize], patch0: usize) -> f64 {
        let mut e_norm = 0.0;
        let edges = self.engine.face_edges[bf].clone();
        for (i, &be) in edges.iter().enumerate() {
            if nei[i] == patch0 {
                continue;
            }
            if nei[i] == NO_PATCH {
                self.report.n_unset_neighbour_patch_reads += 1;
                continue;
            }
            e_norm += self.deformation_metric_for_edge(octree, be, patch0, nei[i]);
        }
        e_norm
    }

    /// `checkFacePatchesGeometry` (at most 4 mappings; see the module docs).
    pub fn check_face_patches_geometry(&mut self, octree: &MeshOctree) -> bool {
        let mut changed = false;
        let nbp = self.engine.n_boundary_points();
        let mut active = vec![true; nbp];
        let mut iter = 0;
        loop {
            let fp = self.face_patch.clone();
            let part = SurfacePartitioner::with_face_patches(&self.engine, fp.clone());
            map_points_onto_patches(&self.engine, &part, &self.queries, &mut self.mesh.points, Some(&active));
            iter += 1;
            if iter > 3 {
                break;
            }
            self.engine.update_geometry_all(&self.mesh.points);
            let inverted: BTreeSet<usize> = check_inverted_vertices(&self.engine, &part, &self.mesh.points, Some(&active));
            self.report.geometry_inverted.push(inverted.len());
            if inverted.is_empty() {
                self.report.geometry_changed = false;
                return false;
            }
            active = vec![false; nbp];
            let mut labels = Vec::with_capacity(inverted.len());
            for &v in &inverted {
                labels.push(self.engine.bp[v]);
                active[self.engine.bp[v]] = true;
            }
            let mut engine = self.engine.clone();
            engine.face_patches = fp.clone();
            let mut mso = MeshSurfaceOptimizer::new(engine, part, Some(self.queries.clone()));
            mso.untangle_surface_selected(&mut self.mesh.points, &labels, 1);
            self.engine.update_geometry_all(&self.mesh.points);

            let candidates: Vec<usize> =
                (0..self.engine.n_boundary_faces()).filter(|&bf| self.engine.face_rings[bf].iter().any(|v| inverted.contains(v))).collect();
            let mut new = fp.clone();
            let mut n_corrected = 0;
            for &bf in &candidates {
                let nei = self.nei_patches(bf, &fp);
                let mut all = Vec::new();
                for &p in &nei {
                    if p != NO_PATCH {
                        push_unique(&mut all, p);
                    }
                }
                let mut min_e = VGREAT;
                let mut min_patch = None;
                for &p in &all {
                    let e = self.deformation_metric_for_face(octree, bf, &nei, p);
                    if e < min_e {
                        min_e = e;
                        min_patch = Some(p);
                    }
                }
                // upstream compares a possible -1 with the face's patch
                if let Some(mp) = min_patch {
                    if mp != fp[bf] {
                        new[bf] = mp;
                        n_corrected += 1;
                    }
                }
            }
            if n_corrected == 0 {
                self.report.geometry_moves.push(0);
                break;
            }
            let mut n_changed = 0;
            for bf in 0..new.len() {
                if new[bf] != fp[bf] {
                    let p = self.best_patch_after_modification(bf, &new);
                    new[bf] = p;
                    if p != fp[bf] {
                        n_changed += 1;
                    }
                }
            }
            self.report.geometry_moves.push(n_changed);
            if n_changed == 0 {
                break;
            }
            changed = true;
            self.face_patch = new;
        }
        self.report.geometry_changed = changed;
        changed
    }

    /// `extractEdges`: the four steps of the module docs.
    pub fn extract_edges(&mut self, octree: &MeshOctree) -> Result<(), String> {
        self.distribute_boundary_faces(octree)?;
        let initial = self.face_patch.clone();
        self.distribute_boundary_faces_normal_alignment(octree);
        self.check_face_patches_topology();
        self.check_face_patches_geometry(octree);
        self.report.n_faces_moved_from_initial = (0..initial.len()).filter(|&i| initial[i] != self.face_patch[i]).count();
        Ok(())
    }

    /// `moveVerticesTowardsDiscontinuities(nIterations)`: each boundary
    /// point moved by the average displacement of its faces' centres onto
    /// the surface, then onto the nearest surface point.
    pub fn move_vertices_towards_discontinuities(&mut self, octree: &MeshOctree, n_iterations: usize) {
        for _ in 0..n_iterations {
            let pts = &self.mesh.points;
            let disp: Vec<Vec3> = self
                .engine
                .face_rings
                .iter()
                .map(|ring| {
                    let c = foam_face_centre(pts, ring);
                    octree.find_nearest_surface_point(c).map_or(Vec3::ZERO, |h| h.point.sub(c))
                })
                .collect();
            let mut new_pts = Vec::with_capacity(self.engine.n_boundary_points());
            for b in 0..self.engine.n_boundary_points() {
                let pf = &self.engine.point_faces[b];
                let s = pf.iter().fold(Vec3::ZERO, |a, &bf| a.add(disp[bf]));
                let mp = pts[self.engine.boundary_points[b]].add(s.scale(1.0 / pf.len() as f64));
                new_pts.push(octree.find_nearest_surface_point(mp).map_or(mp, |h| h.point));
            }
            for (b, p) in new_pts.into_iter().enumerate() {
                self.mesh.points[self.engine.boundary_points[b]] = p;
            }
            self.engine.update_geometry_all(&self.mesh.points);
        }
    }

    /// `projectDeterminedFeatureVertices`: points at two patches onto the
    /// feature edge, at three or more onto the corner, keeping the averaged
    /// patch projection where the octree's edge or corner is more than 1.1×
    /// as far (squared). A failed edge/corner search (unset upstream) uses
    /// the averaged projection.
    pub fn project_determined_feature_vertices(&mut self, octree: &MeshOctree) {
        let e = &self.engine;
        let mut point_patches: Vec<Vec<usize>> = vec![Vec::new(); e.n_boundary_points()];
        for (bf, ring) in e.face_rings.iter().enumerate() {
            for &v in ring {
                push_unique(&mut point_patches[e.bp[v]], self.face_patch[bf]);
            }
        }
        let mut moves = Vec::new();
        for (b, pp) in point_patches.iter().enumerate() {
            if pp.len() < 2 {
                continue;
            }
            let p = self.mesh.points[self.engine.boundary_points[b]];
            let exact = if pp.len() == 2 { octree.find_nearest_edge_point(p, pp) } else { octree.find_nearest_corner(p, pp) };
            let (mut new_p, d_exact) = match exact {
                Some((q, d, _)) => (q, d),
                None => {
                    self.report.n_failed_region_searches += 1;
                    (p, VGREAT)
                }
            };
            let mut q = p;
            for _ in 0..20 {
                let mut inp = Vec3::ZERO;
                for &r in pp {
                    let np = self.nearest_in_region(octree, q, r).map_or(q, |x| x.0);
                    inp = inp.add(np);
                }
                inp = inp.scale(1.0 / pp.len() as f64);
                let curr = mag_sqr(inp.sub(q));
                q = inp;
                if curr < 1e-2 * d_exact {
                    break;
                }
            }
            if d_exact > 1.1 * mag_sqr(q.sub(p)) {
                new_p = q;
            }
            moves.push((b, new_p));
        }
        for (b, p) in moves {
            self.mesh.points[self.engine.boundary_points[b]] = p;
        }
        self.engine.update_geometry_all(&self.mesh.points);
    }

    /// `untangleSurface`: `meshSurfaceOptimizer(mse, octree)
    /// .untangleSurface()` on the mesh's own patches (2 layers).
    pub fn untangle_surface(&mut self) -> bool {
        let part = SurfacePartitioner::new(&self.engine);
        let mut mso = MeshSurfaceOptimizer::new(self.engine.clone(), part, Some(self.queries.clone()));
        let r = mso.untangle_surface(&mut self.mesh.points, 2);
        self.engine.update_geometry_all(&self.mesh.points);
        r.changed
    }

    /// `surfaceWithPatches()`: the boundary as triangles (each face fanned
    /// from its first point, upstream's order), region = face patch.
    pub fn surface_with_patches(&self) -> TriSurf {
        let e = &self.engine;
        let pts: Vec<Vec3> = e.boundary_points.iter().map(|&v| self.mesh.points[v]).collect();
        let (mut tris, mut regs) = (Vec::new(), Vec::new());
        for (bf, ring) in e.face_rings.iter().enumerate() {
            let mut i = ring.len() - 2;
            while i > 0 {
                tris.push([e.bp[ring[0]], e.bp[ring[i]], e.bp[ring[i + 1]]]);
                regs.push(self.face_patch[bf]);
                i -= 1;
            }
        }
        TriSurf::new(pts, tris, Some(regs))
    }

    /// `surfaceWithPatches(bpI)`: the faces at boundary point `b` only.
    pub fn surface_with_patches_at(&self, b: usize) -> TriSurf {
        let e = &self.engine;
        let mut new_label = vec![usize::MAX; self.mesh.point_count()];
        let mut n = 0;
        let (mut tris, mut regs) = (Vec::new(), Vec::new());
        for &bf in &e.point_faces[b] {
            let ring = &e.face_rings[bf];
            for &v in ring {
                if new_label[v] == usize::MAX {
                    new_label[v] = n;
                    n += 1;
                }
            }
            let mut i = ring.len() - 2;
            while i > 0 {
                tris.push([new_label[ring[0]], new_label[ring[i]], new_label[ring[i + 1]]]);
                regs.push(self.face_patch[bf]);
                i -= 1;
            }
        }
        let mut pts = vec![Vec3::ZERO; n];
        for (v, &l) in new_label.iter().enumerate() {
            if l != usize::MAX {
                pts[l] = self.mesh.points[v];
            }
        }
        TriSurf::new(pts, tris, Some(regs))
    }

    /// `updateMeshPatches`: `replaceBoundary` with one patch per surface
    /// patch, in patch order (empty patches kept), boundary faces in
    /// boundary-face order within each. Returns the mesh and the patch
    /// types: each surface patch's `geometricType()`
    /// (`boundaries[patchI].patchType() = surfPatches[patchI].geometricType()`,
    /// gh:#1483; `patch` for a surface built without types).
    pub fn update_mesh_patches(&self) -> (VolumeMesh, Vec<String>) {
        let m = &self.mesh;
        let e = &self.engine;
        let internal: Vec<usize> = (0..m.face_count()).filter(|&f| m.neighbour[f].is_some()).collect();
        let mut faces: Vec<Vec<usize>> = internal.iter().map(|&f| m.faces[f].clone()).collect();
        let mut owner: Vec<usize> = internal.iter().map(|&f| m.owner[f]).collect();
        let mut neighbour: Vec<Option<usize>> = internal.iter().map(|&f| m.neighbour[f]).collect();
        let mut patches = Vec::with_capacity(self.n_patches());
        let n_internal = internal.len();
        for (pi, name) in self.names.iter().enumerate() {
            let start = faces.len();
            for bf in 0..e.n_boundary_faces() {
                if self.face_patch[bf] == pi {
                    faces.push(e.face_rings[bf].clone());
                    owner.push(e.face_owners[bf]);
                    neighbour.push(None);
                }
            }
            patches.push(BoundaryPatch { name: name.clone(), start_face: start, n_faces: faces.len() - start });
        }
        // `replaceBoundary` also rewrites the cell lists (gh:#1385)
        let cells = if m.carried_cells_are_consistent() && internal.iter().enumerate().all(|(i, &f)| i == f) {
            Some(crate::cartesian_mesh::replace_boundary_cell_lists(m.cells.as_ref().unwrap_or(&Vec::new()), n_internal, &owner[n_internal..]))
        } else {
            None
        };
        let out = VolumeMesh { points: m.points.clone(), faces, owner, neighbour, n_cells: m.n_cells, patches, cells };
        (out, self.types.clone())
    }

    /// The patch of every face of [`Self::update_mesh_patches`]'s mesh
    /// would be: per mesh face of the current mesh, `None` inside.
    pub fn face_regions(&self) -> Vec<Option<usize>> {
        let mut out = vec![None; self.mesh.face_count()];
        for (bf, &f) in self.engine.boundary_faces.iter().enumerate() {
            out[f] = Some(self.face_patch[bf]);
        }
        out
    }
}

/// The gh:#1269 differential on one mesh: the patch assignment by the
/// nearest triangle and by `edgeExtractor`, each followed by
/// `mapVerticesOntoSurfacePatches` (both on the same face numbering).
#[derive(Debug, Clone)]
pub struct AssignmentDifferential {
    /// Nearest-triangle assignment, then the patch mapping.
    pub nearest: VolumeMesh,
    /// Its patch per mesh face (`None` inside).
    pub nearest_regions: Vec<Option<usize>>,
    /// `edgeExtractor`'s assignment (and point moves), then the mapping.
    pub extracted: VolumeMesh,
    /// Its patch per mesh face.
    pub extracted_regions: Vec<Option<usize>>,
    /// Faces whose patch differs: (mesh face, nearest, extractor).
    pub changed: Vec<(usize, usize, usize)>,
    /// The extractor's report.
    pub report: EdgeExtractorReport,
}

/// Run the differential of [`AssignmentDifferential`] on `mesh` (already
/// projected onto the surface), the surface `(points, tris, regions)` and
/// its octree.
pub fn assignment_differential(mesh: &VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions, octree: &MeshOctree) -> Result<AssignmentDifferential, String> {
    let nearest_regions = crate::surface_mapper::boundary_face_regions(mesh, points, tris, regions);
    let nearest = crate::surface_mapper::map_vertices_onto_surface_patches_given(mesh, points, tris, regions, &nearest_regions);
    let mut ex = EdgeExtractor::new(mesh.clone(), octree, regions)?;
    ex.extract_edges(octree)?;
    let extracted_regions = ex.face_regions();
    let extracted = crate::surface_mapper::map_vertices_onto_surface_patches_given(&ex.mesh, points, tris, regions, &extracted_regions);
    let changed = (0..mesh.face_count())
        .filter_map(|f| match (nearest_regions[f], extracted_regions[f]) {
            (Some(a), Some(b)) if a != b => Some((f, a, b)),
            _ => None,
        })
        .collect();
    Ok(AssignmentDifferential { nearest, nearest_regions, extracted, extracted_regions, changed, report: ex.report })
}

/// The faces of patch `region` (by `face_region`), their projected area
/// `|Σ S_z|`, and the rim: the patch's edges used by one of its faces only.
pub fn patch_area_and_rim(m: &VolumeMesh, face_region: &[Option<usize>], region: usize) -> (f64, Vec<[usize; 2]>) {
    let mut a_z = 0.0;
    let mut count: std::collections::BTreeMap<(usize, usize), usize> = Default::default();
    for f in 0..m.face_count() {
        if face_region[f] != Some(region) {
            continue;
        }
        a_z += m.face_area_vector(f).z;
        let ring = &m.faces[f];
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let rim = count.into_iter().filter(|&(_, c)| c == 1).map(|((a, b), _)| [a, b]).collect();
    (a_z.abs(), rim)
}

/// `tetMeshGenerator::extractPatches`: `edgeExtractor(mesh, octree)`,
/// `extractEdges`, `updateMeshPatches`. Returns the mesh with its new
/// patches, their types, and the report.
pub fn extract_patches(mesh: &VolumeMesh, octree: &MeshOctree, regions: &SurfaceRegions) -> Result<(VolumeMesh, Vec<String>, EdgeExtractorReport), String> {
    let mut ex = EdgeExtractor::new(mesh.clone(), octree, regions)?;
    ex.extract_edges(octree)?;
    let (m, types) = ex.update_mesh_patches();
    Ok((m, types, ex.report))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::mesh_octree_creator::MeshDict;
    use crate::pipeline::tet_mesh::create_octree;
    use crate::shapes::box_surface;

    #[test]
    fn chain_and_map_order_helpers() {
        assert!(are_elements_in_chain(&[true, true, false, false]));
        assert!(are_elements_in_chain(&[true, false, false, true])); // cyclic
        assert!(!are_elements_in_chain(&[true, false, true, false]));
        assert!(are_elements_in_chain(&[false, false, false]));
        // fewer than 8 keys: ascending
        assert_eq!(foam_map_order(3, &[2, 0, 1]), vec![0, 1, 2]);
        // 9 and 1 share bucket 1 of 8: the later insert first
        assert_eq!(foam_map_order(2, &[1, 9]), vec![9, 1]);
        // ties go to the current patch, else the first in map order
        assert_eq!(dominant_patch(&[0, 1, 1, 0], &[0, 1], 1), (Some(1), 2));
        assert_eq!(dominant_patch(&[0, 1, 1, 0], &[0, 1], 5), (Some(0), 2));
        assert_eq!(EdgeExtractor::best_patch_topological(&[1, 1, 0], 0), 1);
        assert_eq!(EdgeExtractor::best_patch_topological(&[1, 0, 1, 0], 0), 0);
    }

    /// The box of `box_surface` (12 triangles face by face: -Z, +Z, -Y, +Y,
    /// +X, -X) with three named patches, meeting at the corners (1, 0, 1)
    /// and (0, 1, 1): `bottom` (-Z, -Y, -X), `top` (+Z), `sides` (+Y, +X).
    pub(crate) fn three_patch_box() -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&[
            "bottom", "bottom", "top", "top", "bottom", "bottom", "sides", "sides", "sides", "sides", "bottom", "bottom",
        ]);
        (p, t, regions)
    }

    /// V&V (gh:#1269), the plan's test: on a carved box with three named
    /// patches meeting at a corner, every boundary face takes the patch of
    /// the side it lies on, by `edgeExtractor` and by the nearest triangle.
    /// Prediction (written before the run): on an axis-aligned carve both
    /// rules are exact (a face centre lies on its own side, nearer to it
    /// than to any other), 0 faces misassigned by either; the extractor
    /// moves no point (none inverted) and changes nothing after
    /// `distributeBoundaryFaces`. Results: see the asserts (as predicted).
    #[test]
    fn box_with_three_patches_every_face_on_its_side() {
        let (p, t, r) = three_patch_box();
        let m = carve_box(&p, &t, 0.25);
        let o = create_octree(&p, &t, Some(&r), &MeshDict::with_max_cell_size(0.25));
        let mut ex = EdgeExtractor::new(m.clone(), &o, &r).unwrap();
        ex.extract_edges(&o).unwrap();
        let wrong_ex = ex.engine.boundary_faces.iter().enumerate().filter(|&(bf, &f)| r.names[ex.face_patch[bf]] != side_of(m.face_area_vector(f))).count();
        let named = crate::patches::assign_patches_by_region(&m, &p, &t, &r).unwrap();
        let mut wrong_nt = 0;
        for pa in &named.patches {
            for f in pa.start_face..pa.start_face + pa.n_faces {
                if side_of(named.face_area_vector(f)) != pa.name {
                    wrong_nt += 1;
                }
            }
        }
        println!("box: {} boundary faces; misassigned extractor {wrong_ex}, nearest triangle {wrong_nt}; report {:?}", ex.engine.n_boundary_faces(), ex.report);
        assert_eq!(wrong_ex, 0);
        assert_eq!(wrong_nt, 0);
        assert!(ex.report.normal_alignment_moves.iter().all(|&x| x == 0));
        assert_eq!(ex.report.geometry_inverted, vec![0]);
        assert_eq!(ex.report.n_unset_neighbour_patch_reads, 0);
        // points stay on the box
        let (out, types) = ex.update_mesh_patches();
        out.validate().unwrap();
        assert_eq!(types.len(), 3);
        assert!((out.total_volume() - 1.0).abs() < 1e-12);
        let names: Vec<&str> = out.patches.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, r.names.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(out.patches.iter().map(|x| x.n_faces).sum::<usize>(), ex.engine.n_boundary_faces());
        // the auxiliary data of the constructor
        // valence: 4 inside a side, 2 on a box edge, 3 at a box corner
        assert!(ex.point_valence.iter().all(|&v| (2..=4).contains(&v)));
        assert_eq!(ex.point_valence.iter().filter(|&&v| v == 3).count(), 8);
        // one cell at a box-edge edge, two elsewhere
        assert_eq!(ex.n_cells_at_edge.iter().filter(|&&n| n == 1).count(), 4 * 12);
        assert!(ex.n_cells_at_edge.iter().all(|&n| n == 1 || n == 2));
        assert_eq!(ex.edge_type.iter().filter(|&&x| x & edge_class::SINGLECELLEDGE != 0).count(), 4 * 12);
        assert!(ex.patches_near_face.iter().all(|x| !x.is_empty()));
        assert!(ex.feature_edges_near_edge.iter().any(|x| !x.is_empty()));
        // patch borders lie on 8 of the 12 box edges (all but the +X/+Y,
        // -X/-Y, -X/-Z and -Y/-Z edges): 3 inner points each, plus the 7
        // box corners other than (0, 0, 0)
        let border_points = ex.mark_patch_points().iter().filter(|&&x| !x).count();
        assert_eq!(border_points, 8 * 3 + 7);
    }

    /// V&V (gh:#1269): the steps upstream does not call from a workflow, on
    /// the same box, and the topology repair on two planted errors.
    /// Predictions (from reading upstream, before the run): a `top` face on
    /// the x = 1 rim planted in `sides` goes back to `top` in
    /// `checkFacePatchesTopology` (one proposal, then a pass with none); an
    /// interior `top` face planted in `bottom` stays wrong, because
    /// `findFaceCandidates` lists only faces whose neighbours are in more
    /// than one patch, and all of its neighbours are `top`; on the
    /// exact box `moveVerticesTowardsDiscontinuities`,
    /// `projectDeterminedFeatureVertices` and `untangleSurface` move no
    /// point by more than 1e-12; a box-edge edge between `top` and `sides`
    /// has alignment 1 with its feature edge; the triangulated surface has
    /// two triangles per quad.
    #[test]
    fn box_auxiliary_steps_and_planted_error() {
        let (p, t, r) = three_patch_box();
        let m = carve_box(&p, &t, 0.25);
        let o = create_octree(&p, &t, Some(&r), &MeshDict::with_max_cell_size(0.25));
        let mut ex = EdgeExtractor::new(m.clone(), &o, &r).unwrap();
        ex.distribute_boundary_faces(&o).unwrap();
        let top = r.names.iter().position(|n| n == "top").unwrap();
        let bottom = r.names.iter().position(|n| n == "bottom").unwrap();
        let sides = r.names.iter().position(|n| n == "sides").unwrap();
        let face_at = |c0: Vec3| {
            (0..ex.engine.n_boundary_faces()).find(|&bf| foam_face_centre(&m.points, &ex.engine.face_rings[bf]).sub(c0).length() < 1e-9).unwrap()
        };
        // an interior top face (centre (0.375, 0.375, 1)) planted in
        // `bottom`: all its neighbours are `top`, so findFaceCandidates
        // does not list it and upstream leaves it (a limitation as written)
        let island = face_at(Vec3::new(0.375, 0.375, 1.0));
        // a top face on the x = 1 rim planted in `sides`: a candidate,
        // three of its edges border `top` in one chain, so it goes back
        let rim = face_at(Vec3::new(0.875, 0.375, 1.0));
        ex.face_patch[island] = bottom;
        ex.face_patch[rim] = sides;
        assert!(ex.check_face_patches_topology());
        println!("planted: report {:?}", ex.report);
        assert_eq!(ex.face_patch[rim], top);
        assert_eq!(ex.face_patch[island], bottom);
        assert_eq!(ex.report.topology_proposed, vec![1, 0]);
        ex.face_patch[island] = top;

        let before = ex.mesh.points.clone();
        let moved = |a: &[Vec3], b: &[Vec3]| a.iter().zip(b).map(|(x, y)| x.sub(*y).length()).fold(0.0, f64::max);
        ex.move_vertices_towards_discontinuities(&o, 2);
        assert!(moved(&before, &ex.mesh.points) < 1e-12, "{}", moved(&before, &ex.mesh.points));
        ex.project_determined_feature_vertices(&o);
        assert!(moved(&before, &ex.mesh.points) < 1e-12, "{}", moved(&before, &ex.mesh.points));
        ex.untangle_surface();
        assert!(moved(&before, &ex.mesh.points) < 1e-12);
        // a top/sides edge at x = 1, z = 1
        let be = (0..ex.engine.edges.len())
            .find(|&e| {
                let [a, b] = ex.engine.edges[e];
                let (pa, pb) = (m.points[a], m.points[b]);
                (pa.x - 1.0).abs() < 1e-12 && (pb.x - 1.0).abs() < 1e-12 && (pa.z - 1.0).abs() < 1e-12 && (pb.z - 1.0).abs() < 1e-12
            })
            .unwrap();
        assert!((ex.alignment_for_edge(&o, be, top, sides) - 1.0).abs() < 1e-9);
        let s = ex.surface_with_patches();
        assert_eq!(s.triangles.len(), 2 * ex.engine.n_boundary_faces());
        let s1 = ex.surface_with_patches_at(0);
        assert_eq!(s1.triangles.len(), 2 * ex.engine.point_faces[0].len());
        assert_eq!(ex.report.n_failed_region_searches, 0);
    }

    /// The bare Foam Studio pipe dual (`CellCentre` + `Corners`) at `h`,
    /// projected onto the surface (`mapMeshToSurface`), with its octree.
    #[cfg(feature = "cell-centre-dual")]
    pub(crate) fn foam_studio_premapped(h: f64) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions, VolumeMesh, MeshOctree) {
        use crate::dual::{DualKind, FeaturePointRule};
        use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
        let (p, t, r) = crate::pipeline::tet_mesh::tests::pipe_surface(0.01, 0.2, 48);
        let o = TetDualOptions {
            cell_size: h,
            n_layers: 0,
            dual_kind: DualKind::CellCentre,
            dual_feature_points: FeaturePointRule::Corners,
            feature_edges: false,
            ..Default::default()
        };
        let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &r, &o).unwrap();
        let (pre, _) = crate::surface_optimizer::map_mesh_to_surface(&m, &p, &t, &r);
        let oct = create_octree(&p, &t, Some(&r), &MeshDict::with_max_cell_size(h));
        (p, t, r, pre, oct)
    }

    /// V&V (gh:#1269), differential A and predictions 3–5 of
    /// `verification_and_validation/edge_extractor_gh1269/README.md`:
    /// nearest triangle against `edgeExtractor` on the Foam Studio pipe
    /// (`CellCentre` + `Corners`, h 4 / 5 / 6 mm, `CFMESH_HS` to choose),
    /// each followed by the patch mapping. Prints the faces that change
    /// patch (and how far from the nearer end), inlet `|Σ S_z| / πR²`, the
    /// rim, validity and the extractor's report. Results: the README.
    #[cfg(feature = "cell-centre-dual")]
    #[test]
    #[ignore]
    fn diagnose_foam_studio_pipe_differential() {
        let hs: Vec<f64> = std::env::var("CFMESH_HS").ok().map_or(vec![0.004, 0.005, 0.006], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        let (r_pipe, l) = (0.01, 0.2);
        let section = std::f64::consts::PI * r_pipe * r_pipe;
        for h in hs {
            let t0 = std::time::Instant::now();
            let (p, t, r, pre, oct) = foam_studio_premapped(h);
            let d = assignment_differential(&pre, &p, &t, &r, &oct).unwrap();
            let inlet = r.names.iter().position(|n| n == "inlet").unwrap();
            let mut far = 0;
            let mut by_kind: std::collections::BTreeMap<(String, String), usize> = Default::default();
            for &(f, a, b) in &d.changed {
                let z = pre.face_centroid(f).z;
                if z.min(l - z) > h {
                    far += 1;
                }
                *by_kind.entry((r.names[a].clone(), r.names[b].clone())).or_default() += 1;
            }
            for (name, m, reg) in [("nearest", &d.nearest, &d.nearest_regions), ("edgeExtractor", &d.extracted, &d.extracted_regions)] {
                let (az, rim) = patch_area_and_rim(m, reg, inlet);
                let rim_pts: BTreeSet<usize> = rim.iter().flat_map(|e| e.iter().copied()).collect();
                let rad: Vec<f64> = rim_pts.iter().map(|&v| m.points[v].x.hypot(m.points[v].y)).collect();
                let rmean = rad.iter().sum::<f64>() / rad.len().max(1) as f64;
                let rmin = rad.iter().cloned().fold(f64::MAX, f64::min);
                let n_in = reg.iter().filter(|x| **x == Some(inlet)).count();
                let q = crate::checks::check_quality(m);
                println!(
                    "h {:.0} mm {name}: inlet {n_in} faces, |sum Sz|/piR2 {:.5} (48-gon 0.99715); rim {} edges {} points, r mean {:.4} min {:.4} mm; valid {}, negative cells {}, max non-orth {:.1}",
                    h * 1e3,
                    az / section,
                    rim.len(),
                    rim_pts.len(),
                    rmean * 1e3,
                    rmin * 1e3,
                    m.validate().is_ok(),
                    q.n_negative_volume_cells,
                    q.max_non_orthogonality_deg
                );
            }
            println!(
                "h {:.0} mm: {} boundary faces change patch ({far} more than h from an end): {by_kind:?}; report {:?}; {:.1} s",
                h * 1e3,
                d.changed.len(),
                d.report,
                t0.elapsed().as_secs_f64()
            );
            assert_eq!(d.report.n_unset_neighbour_patch_reads, 0);
            assert_eq!(d.report.n_failed_region_searches, 0);
        }
    }

    /// Patch per mesh face from the patch ranges and `names`.
    fn regions_of(m: &VolumeMesh, names: &[String]) -> Vec<Option<usize>> {
        let mut out = vec![None; m.face_count()];
        for pa in &m.patches {
            let r = names.iter().position(|n| *n == pa.name);
            for x in out.iter_mut().skip(pa.start_face).take(pa.n_faces) {
                *x = r;
            }
        }
        out
    }

    /// V&V (gh:#1269), differential B and predictions 6–7: `tet_mesh` and
    /// `p_mesh` on the pipe (R 10 mm, L 60 mm, 48 sides, 2 mm) with
    /// `edgeExtractor` (default) and with the nearest triangle (ablation).
    /// Prints volume error against the 48-gon prism, cells, non-positive
    /// cells, max non-orthogonality, patch sizes, inlet `|Σ S_z| / πR²` and
    /// the notes. Results: the README.
    #[test]
    #[ignore]
    fn diagnose_edge_extractor_in_tet_and_p_mesh() {
        use crate::pipeline::tet_mesh::{tet_mesh, PatchAssignment, TetMeshOptions};
        let (p, t, r) = crate::pipeline::tet_mesh::tests::pipe_surface(0.01, 0.06, 48);
        let exact = 0.5 * 48.0 * 0.01f64.powi(2) * (2.0 * std::f64::consts::PI / 48.0).sin() * 0.06;
        let section = std::f64::consts::PI * 1e-4;
        for kind in ["tet_mesh", "p_mesh"] {
            for pa in [PatchAssignment::EdgeExtractor, PatchAssignment::NearestTriangle] {
                let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.002), patch_assignment: pa, ..Default::default() };
                let t0 = std::time::Instant::now();
                let (m, rep) = if kind == "tet_mesh" { tet_mesh(&p, &t, Some(&r), &opts).unwrap() } else { crate::pipeline::p_mesh::p_mesh(&p, &t, Some(&r), &opts).unwrap() };
                let dt = t0.elapsed().as_secs_f64();
                let q = crate::checks::check_quality(&m);
                let (vols, _) = crate::checks::cell_geometry(&m);
                let reg = regions_of(&m, &r.names);
                let inlet = r.names.iter().position(|n| n == "inlet").unwrap();
                let (az, rim) = patch_area_and_rim(&m, &reg, inlet);
                println!(
                    "{kind} {pa:?}: cells {} volume error {:+.3} % non-positive {} valid {} max non-orth {:.2} skew {:.3}; patches {:?}; inlet |sum Sz|/piR2 {:.5}, rim {} edges; {dt:.1} s",
                    m.n_cells,
                    (m.total_volume() - exact) / exact * 100.0,
                    vols.iter().filter(|&&v| v <= 0.0).count(),
                    m.validate().is_ok(),
                    q.max_non_orthogonality_deg,
                    q.max_skewness,
                    m.patches.iter().map(|x| (x.name.clone(), x.n_faces)).collect::<Vec<_>>(),
                    az / section,
                    rim.len()
                );
                for n in &rep.notes {
                    println!("    {n}");
                }
            }
        }
    }

    /// V&V (gh:#1269): `tet_mesh` and `p_mesh` with `edgeExtractor` wired in
    /// (default), on the coarse pipe of the restart test (24 sides, 4 mm):
    /// closed, every cell positive, three named patches, no undefined read,
    /// and the extractor's report attached.
    #[test]
    fn tet_and_p_mesh_run_the_edge_extractor() {
        use crate::pipeline::tet_mesh::{tet_mesh, TetMeshOptions};
        let (p, t, r) = crate::pipeline::tet_mesh::tests::pipe_surface(0.01, 0.06, 24);
        let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.004), ..Default::default() };
        for p_kind in [false, true] {
            let (m, rep) = if p_kind { crate::pipeline::p_mesh::p_mesh(&p, &t, Some(&r), &opts).unwrap() } else { tet_mesh(&p, &t, Some(&r), &opts).unwrap() };
            m.validate().unwrap();
            let (vols, _) = crate::checks::cell_geometry(&m);
            assert!(vols.iter().all(|&v| v > 0.0));
            let er = rep.edge_extractor.as_ref().expect("edgeExtractor ran");
            println!("p_mesh {p_kind}: {:?}; {:?}", m.patches.iter().map(|x| (x.name.clone(), x.n_faces)).collect::<Vec<_>>(), er);
            assert_eq!(er.n_unset_neighbour_patch_reads, 0);
            assert_eq!(er.n_failed_region_searches, 0);
            assert_eq!(m.patches.len(), 3);
            assert!(m.patches.iter().all(|x| x.n_faces > 0));
            assert!(rep.notes.iter().any(|n| n.starts_with("patchAssignment: edgeExtractor")));
            if p_kind {
                // gh:#1281: FUN now builds its fundamental sheets.
                assert!(rep.notes.iter().any(|n| n.contains("meshSurfaceEdgeExtractorFUN: createFundamentalSheetsJFS")));
            }
        }
    }

    fn side_of(s: Vec3) -> String {
        if s.z > 0.5 * s.length() {
            "top".into()
        } else if s.x > 0.5 * s.length() || s.y > 0.5 * s.length() {
            "sides".into()
        } else {
            "bottom".into()
        }
    }
}
