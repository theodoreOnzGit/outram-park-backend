// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2015-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! Snapping stages of ESI v2506 `snappyHexMesh`, translated (gh:#1467, epic #1459):
//! the plain (no feature) loop of `snappySnapDriver::doSnap`, on a polyMesh read in.
//!
//! **Upstream (OpenFOAM.com ESI v2506, `src/mesh/snappyHexMesh/snappyHexMeshDriver/snappySnapDriver.C`):**
//! `calcSnapDistance` (832), `smoothPatchDisplacement` (290), `preSmoothPatch`
//! (872), `calcNearestSurface` (1797), `outwardsDisplacement` (706),
//! `smoothDisplacement` (2134), `edgePatchDist` (631); and from
//! `src/dynamicMesh/motionSmoother/motionSmootherAlgo.C`: `setDisplacement` (467),
//! `smooth`/`avg` (`motionSmootherAlgoTemplates.C`), `curPoints` (748);
//! `src/meshTools/algorithms/PointEdgeWave/` (`PointEdgeWave.C`, `pointEdgePointI.H`);
//! `primitiveMesh::calcEdges` (edge order), `primitiveMeshTools::makeFaceCentresAndAreas`,
//! `face::centre`, `PrimitivePatch::calcMeshData` / `calcPointFaces`.
//!
//! **What is translated:** the operation order of each routine (loops over the
//! same index orders, the same operand grouping), so that results can be compared
//! bit for bit; see `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`
//! (round 3, task 2) for the stage table. **What is not:** features
//! (`snappySnapDriverFeature.C`), `detectNearSurfaces` (ESI overrides 0 of 12130
//! points on the flange), `freezeExposedPoints` (no frozen zone), baffles and
//! faceZones, parallel synchronisation (serial only). ~~`repatchToSurface`,
//! `mergePatchFaces`, and the quality check of `scaleMesh`~~ **CORRECTED 2026-11-12:**
//! `scaleMesh` with its checks and `repatch_to_surface` are in this file (round 4);
//! face merging and edge removal are in [`crate::snappy_hex_mesh::esi_merge`] (round 5).
//! Everything here is opt-in; the
//! older [`crate::snappy_hex_mesh::snapping::snap`] is untouched.
//!
//! Numerics: plain IEEE `+ - * /` and `sqrt` only, in upstream's grouping, no
//! `mul_add`; no transcendental is involved, so there is no PETIR residual here.

use std::collections::HashMap;

use outram_foam_basic_lib::primitives::Vector3;

use crate::check_mesh::{esi_check_mesh, esi_geometry, EsiCheckResult, EsiQualityControls};
use crate::snappy_hex_mesh::stl::TriangleSoup;

/// OpenFOAM double-precision constants.
const SMALL: f64 = 1e-15;
const VSMALL: f64 = 1e-300;
const GREAT: f64 = 1e15;
/// `PointEdgeWaveBase::propagationTol_`.
const PROPAGATION_TOL: f64 = 0.01;

/// A vector as three doubles, so every operation is spelled out.
pub type V = [f64; 3];

pub(crate) fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn scale(s: f64, a: V) -> V {
    [s * a[0], s * a[1], s * a[2]]
}
pub(crate) fn div(a: V, s: f64) -> V {
    [a[0] / s, a[1] / s, a[2] / s]
}
pub(crate) fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub(crate) fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn mag_sqr(a: V) -> f64 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}
pub(crate) fn mag(a: V) -> f64 {
    mag_sqr(a).sqrt()
}

/// A polyMesh (serial): the faces of the boundary patches are
/// `faces[neighbour.len()..]`, all of them adapt patches.
#[derive(Debug, Clone)]
pub struct SnapMesh {
    /// Point coordinates.
    pub points: Vec<V>,
    /// Face vertex rings, internal faces first.
    pub faces: Vec<Vec<usize>>,
    /// Owner cell of every face.
    pub owner: Vec<usize>,
    /// Neighbour cell of every internal face.
    pub neighbour: Vec<usize>,
}

impl SnapMesh {
    /// Number of internal faces.
    pub fn n_internal_faces(&self) -> usize {
        self.neighbour.len()
    }
}

/// `indirectPrimitivePatch` addressing of all boundary faces:
/// `meshPoints` in order of first appearance, `pointFaces` per local point.
#[derive(Debug, Clone)]
pub struct PatchAddr {
    /// Mesh face index of each patch face.
    pub face_ids: Vec<usize>,
    /// Mesh point of each local patch point.
    pub mesh_points: Vec<usize>,
    /// Patch faces (local indices) using each local point, ascending.
    pub point_faces: Vec<Vec<usize>>,
    /// Faces as local point indices.
    pub local_faces: Vec<Vec<usize>>,
}

/// `PrimitivePatch::calcMeshData` and `calcPointFaces` for the boundary faces.
pub fn patch_addressing(mesh: &SnapMesh) -> PatchAddr {
    let face_ids: Vec<usize> = (mesh.n_internal_faces()..mesh.faces.len()).collect();
    let mut local_of: HashMap<usize, usize> = HashMap::new();
    let mut mesh_points: Vec<usize> = Vec::new();
    for &f in &face_ids {
        for &p in &mesh.faces[f] {
            if !local_of.contains_key(&p) {
                local_of.insert(p, mesh_points.len());
                mesh_points.push(p);
            }
        }
    }
    let local_faces: Vec<Vec<usize>> =
        face_ids.iter().map(|&f| mesh.faces[f].iter().map(|p| local_of[p]).collect()).collect();
    let mut point_faces = vec![Vec::new(); mesh_points.len()];
    for (fi, f) in local_faces.iter().enumerate() {
        for &p in f {
            point_faces[p].push(fi);
        }
    }
    PatchAddr { face_ids, mesh_points, point_faces, local_faces }
}

/// `primitiveMeshTools::makeFaceCentresAndAreas` (ESI v2506): face centres and
/// area vectors.
pub fn face_centres_areas(points: &[V], faces: &[Vec<usize>]) -> (Vec<V>, Vec<V>) {
    let mut centres = Vec::with_capacity(faces.len());
    let mut areas = Vec::with_capacity(faces.len());
    for f in faces {
        let n_points = f.len();
        if n_points == 3 {
            let (a, b, c) = (points[f[0]], points[f[1]], points[f[2]]);
            centres.push(scale(1.0 / 3.0, add(add(a, b), c)));
            areas.push(scale(0.5, cross(sub(b, a), sub(c, a))));
            continue;
        }
        let mut sum_n = [0.0; 3];
        let mut sum_a = 0.0;
        let mut sum_ac = [0.0; 3];
        let mut f_centre = points[f[0]];
        for &p in &f[1..] {
            f_centre = add(f_centre, points[p]);
        }
        f_centre = div(f_centre, n_points as f64);
        for pi in 0..n_points {
            let this_point = points[f[pi]];
            let next_point = points[f[(pi + 1) % n_points]];
            let c = add(add(this_point, next_point), f_centre);
            let n = cross(sub(next_point, this_point), sub(f_centre, this_point));
            let a = mag(n);
            sum_n = add(sum_n, n);
            sum_a += a;
            sum_ac = add(sum_ac, scale(a, c));
        }
        if sum_a < 1e-150 {
            centres.push(f_centre);
            areas.push([0.0; 3]);
        } else {
            centres.push(div(scale(1.0 / 3.0, sum_ac), sum_a));
            areas.push(scale(0.5, sum_n));
        }
    }
    (centres, areas)
}

/// `face::centre(points)` (ESI v2506 `face.C`), which `smoothPatchDisplacement`
/// uses for the boundary faces (it is not the same arithmetic as the mesh's face centre).
pub fn face_centre(points: &[V], f: &[usize]) -> V {
    let n_points = f.len();
    if n_points == 3 {
        return scale(1.0 / 3.0, add(add(points[f[0]], points[f[1]]), points[f[2]]));
    }
    let mut centre_point = [0.0; 3];
    for &p in f {
        centre_point = add(centre_point, points[p]);
    }
    centre_point = div(centre_point, n_points as f64);
    let mut sum_a = 0.0;
    let mut sum_ac = [0.0; 3];
    for pi in 0..n_points {
        let next_point = points[f[(pi + 1) % n_points]];
        let ttc = add(add(points[f[pi]], next_point), centre_point);
        let ta = mag(cross(sub(points[f[pi]], centre_point), sub(next_point, centre_point)));
        sum_a += ta;
        sum_ac = add(sum_ac, scale(ta, ttc));
    }
    if sum_a > VSMALL {
        div(sum_ac, 3.0 * sum_a)
    } else {
        centre_point
    }
}

/// `calcSnapDistance` (v2506 `snappySnapDriver.C:832`): `snapTol` times the
/// longest patch edge at each patch point.
pub fn calc_snap_distance(mesh: &SnapMesh, pa: &PatchAddr, snap_tol: f64) -> Vec<f64> {
    let local_points: Vec<V> = pa.mesh_points.iter().map(|&p| mesh.points[p]).collect();
    let mut max_edge_len = vec![-GREAT; local_points.len()];
    for f in &pa.local_faces {
        for fp in 0..f.len() {
            let (a, b) = (f[fp], f[(fp + 1) % f.len()]);
            let len = mag(sub(local_points[b], local_points[a]));
            max_edge_len[a] = max_edge_len[a].max(len);
            max_edge_len[b] = max_edge_len[b].max(len);
        }
    }
    max_edge_len.iter().map(|&l| snap_tol * l).collect()
}

/// `calcNearestSurface` (v2506 `snappySnapDriver.C:1797`), one unzoned surface,
/// no region restriction: the displacement to the nearest surface point within
/// `snap_dist` (zero where none is). Returns the displacement and the number of
/// points with no surface in range.
pub fn calc_nearest_surface(
    local_points: &[V],
    snap_dist: &[f64],
    surface: &TriangleSoup,
) -> (Vec<V>, usize) {
    let mut patch_disp = vec![[0.0; 3]; local_points.len()];
    let mut missing = 0usize;
    for (i, p) in local_points.iter().enumerate() {
        let sample = Vector3::new(p[0], p[1], p[2]);
        let mut best: Option<(f64, Vector3)> = None;
        for t in &surface.triangles {
            let q = t.closest_point(sample);
            let d = [q.x - p[0], q.y - p[1], q.z - p[2]];
            let d2 = mag_sqr(d);
            if best.map(|(b, _)| d2 < b).unwrap_or(true) {
                best = Some((d2, q));
            }
        }
        match best {
            Some((d2, q)) if d2 <= snap_dist[i] * snap_dist[i] => {
                patch_disp[i] = [q.x - p[0], q.y - p[1], q.z - p[2]];
            }
            _ => missing += 1,
        }
    }
    for (i, d) in patch_disp.iter_mut().enumerate() {
        let m = mag(*d);
        if m > snap_dist[i] {
            *d = scale(snap_dist[i] / m, *d);
        }
    }
    (patch_disp, missing)
}

/// `outwardsDisplacement` (v2506 `snappySnapDriver.C:706`): the local index of the
/// first patch point whose displacement points through its surrounding patch faces
/// (ESI prints a warning and stops looking), if any. Does not change the displacement.
pub fn outwards_displacement(mesh: &SnapMesh, pa: &PatchAddr, patch_disp: &[V]) -> Option<usize> {
    let patch_faces: Vec<Vec<usize>> = pa.face_ids.iter().map(|&f| mesh.faces[f].clone()).collect();
    let (_, areas) = face_centres_areas(&mesh.points, &patch_faces);
    // PrimitivePatch::faceNormals is the unit area normal.
    let normals: Vec<V> = areas
        .iter()
        .map(|a| {
            let m = mag(*a);
            if m > VSMALL {
                div(*a, m)
            } else {
                [0.0; 3]
            }
        })
        .collect();
    for (pi, pfaces) in pa.point_faces.iter().enumerate() {
        let m = mag(patch_disp[pi]);
        if m > SMALL {
            let disp = div(patch_disp[pi], m);
            if pfaces.iter().any(|&f| dot(normals[f], disp) < SMALL) {
                return Some(pi);
            }
        }
    }
    None
}

/// `getCollocatedPoints(SMALL, points)`: which patch points coincide with another
/// (exactly equal coordinates here: `mergePoints` with tolerance 1e-15 on
/// coordinates of order 1e-2 only merges bit-identical points).
fn collocated_points(points: &[V]) -> Vec<bool> {
    let mut seen: HashMap<[u64; 3], usize> = HashMap::new();
    let mut flag = vec![false; points.len()];
    for (i, p) in points.iter().enumerate() {
        let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
        if let Some(&j) = seen.get(&key) {
            flag[i] = true;
            flag[j] = true;
        } else {
            seen.insert(key, i);
        }
    }
    flag
}

/// `smoothPatchDisplacement` (v2506 `snappySnapDriver.C:290`, serial, no baffles):
/// the displacement of each patch point towards the blend of the average of its
/// boundary-face centres and of its internal-face centres.
pub fn smooth_patch_displacement(mesh: &SnapMesh, pa: &PatchAddr) -> Vec<V> {
    let local_points: Vec<V> = pa.mesh_points.iter().map(|&p| mesh.points[p]).collect();
    let non_manifold = collocated_points(&local_points);

    // Average of boundary face centres (face::centre on the patch faces).
    let centre_of: Vec<V> = pa.face_ids.iter().map(|&f| face_centre(&mesh.points, &mesh.faces[f])).collect();
    let mut avg_boundary: Vec<(f64, V)> = vec![(0.0, [0.0; 3]); pa.mesh_points.len()];
    for (pi, pfaces) in pa.point_faces.iter().enumerate() {
        for &f in pfaces {
            avg_boundary[pi].0 += 1.0;
            avg_boundary[pi].1 = add(avg_boundary[pi].1, centre_of[f]);
        }
    }
    for b in avg_boundary.iter_mut() {
        if b.0.abs() > VSMALL {
            b.1 = div(b.1, b.0);
        }
    }

    // Average of internal face centres (mesh face centres), over all mesh points.
    let n_int = mesh.n_internal_faces();
    let (fc, _) = face_centres_areas(&mesh.points, &mesh.faces[..n_int]);
    let mut global_sum: Vec<(f64, V)> = vec![(0.0, [0.0; 3]); mesh.points.len()];
    for (fi, f) in mesh.faces[..n_int].iter().enumerate() {
        for &p in f {
            global_sum[p].0 += 1.0;
            global_sum[p].1 = add(global_sum[p].1, fc[fi]);
        }
    }
    let avg_internal: Vec<(f64, V)> = pa
        .mesh_points
        .iter()
        .map(|&mp| {
            let wp = global_sum[mp];
            if wp.0.abs() < VSMALL {
                (wp.0, wp.1)
            } else {
                (wp.0, div(wp.1, wp.0))
            }
        })
        .collect();

    // Any cell using a point (the last owner in face order).
    let mut any_cell = vec![usize::MAX; mesh.points.len()];
    for (fi, f) in mesh.faces.iter().enumerate() {
        for &p in f {
            any_cell[p] = mesh.owner[fi];
        }
    }
    let cell_centres = cell_centres(mesh);

    let mut patch_disp = vec![[0.0; 3]; pa.mesh_points.len()];
    for i in 0..pa.mesh_points.len() {
        let mesh_point = pa.mesh_points[i];
        let current_pos = mesh.points[mesh_point];
        let internal = avg_internal[i];
        let boundary = avg_boundary[i];
        let new_pos: V;
        if !non_manifold[i] {
            let internal_blend = 0.1;
            let blend = 0.1;
            let num = add(
                scale(internal_blend * internal.0, internal.1),
                scale((1.0 - internal_blend) * boundary.0, boundary.1),
            );
            let den = internal_blend * internal.0 + (1.0 - internal_blend) * boundary.0;
            let avg_pos = div(num, den);
            new_pos = add(scale(1.0 - blend, avg_pos), scale(blend, current_pos));
        } else if internal.0 == 0.0 {
            let cc = cell_centres[any_cell[mesh_point]];
            let cell_c_blend = 0.8;
            let blend = 0.1;
            let avg_pos = add(scale(1.0 - cell_c_blend, boundary.1), scale(cell_c_blend, cc));
            new_pos = add(scale(1.0 - blend, avg_pos), scale(blend, current_pos));
        } else {
            let internal_blend = 0.9;
            let blend = 0.1;
            let avg_pos = add(scale(internal_blend, internal.1), scale(1.0 - internal_blend, boundary.1));
            new_pos = add(scale(1.0 - blend, avg_pos), scale(blend, current_pos));
        }
        patch_disp[i] = sub(new_pos, current_pos);
    }
    patch_disp
}

/// `primitiveMeshTools::makeCellCentresAndVols` (v2506), cell centres only.
pub fn cell_centres(mesh: &SnapMesh) -> Vec<V> {
    cell_centres_vols(mesh).0
}

/// `primitiveMeshTools::makeCellCentresAndVols` (v2506): cell centres and volumes (the
/// accumulated `3 V` scaled by `1/3` at the end, as upstream).
pub fn cell_centres_vols(mesh: &SnapMesh) -> (Vec<V>, Vec<f64>) {
    let n_cells = mesh.owner.iter().chain(mesh.neighbour.iter()).max().map_or(0, |&m| m + 1);
    let (f_ctrs, f_areas) = face_centres_areas(&mesh.points, &mesh.faces);
    let mut c_est = vec![[0.0; 3]; n_cells];
    let mut n_cell_faces = vec![0usize; n_cells];
    for (fi, &o) in mesh.owner.iter().enumerate() {
        c_est[o] = add(c_est[o], f_ctrs[fi]);
        n_cell_faces[o] += 1;
    }
    for (fi, &n) in mesh.neighbour.iter().enumerate() {
        c_est[n] = add(c_est[n], f_ctrs[fi]);
        n_cell_faces[n] += 1;
    }
    for c in 0..n_cells {
        c_est[c] = div(c_est[c], n_cell_faces[c] as f64);
    }
    let mut cell_ctrs = vec![[0.0; 3]; n_cells];
    let mut cell_vols = vec![0.0; n_cells];
    for (fi, &o) in mesh.owner.iter().enumerate() {
        let pyr3_vol = dot(f_areas[fi], sub(f_ctrs[fi], c_est[o]));
        let pc = add(scale(3.0 / 4.0, f_ctrs[fi]), scale(1.0 / 4.0, c_est[o]));
        cell_ctrs[o] = add(cell_ctrs[o], scale(pyr3_vol, pc));
        cell_vols[o] += pyr3_vol;
    }
    for (fi, &n) in mesh.neighbour.iter().enumerate() {
        let pyr3_vol = dot(f_areas[fi], sub(c_est[n], f_ctrs[fi]));
        let pc = add(scale(3.0 / 4.0, f_ctrs[fi]), scale(1.0 / 4.0, c_est[n]));
        cell_ctrs[n] = add(cell_ctrs[n], scale(pyr3_vol, pc));
        cell_vols[n] += pyr3_vol;
    }
    for c in 0..n_cells {
        if cell_vols[c].abs() > VSMALL {
            cell_ctrs[c] = div(cell_ctrs[c], cell_vols[c]);
        } else {
            cell_ctrs[c] = c_est[c];
        }
    }
    for v in cell_vols.iter_mut() {
        *v *= 1.0 / 3.0;
    }
    (cell_ctrs, cell_vols)
}

/// One iteration of `preSmoothPatch`'s loop body when the mesh check finds no face
/// in error (`scaleMesh` with scale 1): `setDisplacement(patchDisp)` then
/// `points = oldPoints + 1 * displacement`, where the displacement is zero off the
/// patch. ESI reports "Moving mesh using displacement scaling : min:1 max:1" and
/// 0 faces in error for each of the three iterations of the flange. Returns the
/// displacement used.
pub fn pre_smooth_patch_iteration(mesh: &mut SnapMesh, pa: &PatchAddr) -> Vec<V> {
    let patch_disp = smooth_patch_displacement(mesh, pa);
    for (i, &mp) in pa.mesh_points.iter().enumerate() {
        mesh.points[mp] = add(mesh.points[mp], scale(1.0, patch_disp[i]));
    }
    patch_disp
}

/// Mesh edges and `pointEdges` in `primitiveMesh::calcEdges` order.
#[derive(Debug, Clone)]
pub struct MeshEdges {
    /// Edge end points in creation order.
    pub edges: Vec<[usize; 2]>,
    /// Edges of each point in creation order.
    pub point_edges: Vec<Vec<usize>>,
}

/// `primitiveMesh::calcEdges(false)`, serial, `nInternalPoints` unknown: edges are
/// found face by face (`getEdge` creates `(point, next)`), then renumbered by
/// increasing lower end point and, within it, increasing higher end point (the
/// `oldToNew` pass); `pointEdges` are sorted. The end-point order of each edge
/// is the creation order.
pub fn mesh_edges(mesh: &SnapMesh) -> MeshEdges {
    let mut es: Vec<[usize; 2]> = Vec::new();
    let mut pe: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for f in &mesh.faces {
        for fp in 0..f.len() {
            let (p, next) = (f[fp], f[(fp + 1) % f.len()]);
            let found = pe[p].iter().any(|&e| {
                let [a, b] = es[e];
                (a == p && b == next) || (b == p && a == next)
            });
            if !found {
                es.push([p, next]);
                let e = es.len() - 1;
                pe[p].push(e);
                pe[next].push(e);
            }
        }
    }
    let mut order: Vec<usize> = (0..es.len()).collect();
    order.sort_by_key(|&e| (es[e][0].min(es[e][1]), es[e][0].max(es[e][1])));
    let mut old_to_new = vec![0usize; es.len()];
    for (new, &old) in order.iter().enumerate() {
        old_to_new[old] = new;
    }
    let edges: Vec<[usize; 2]> = order.iter().map(|&old| es[old]).collect();
    let point_edges: Vec<Vec<usize>> = pe
        .iter()
        .map(|list| {
            let mut v: Vec<usize> = list.iter().map(|&e| old_to_new[e]).collect();
            v.sort_unstable();
            v
        })
        .collect();
    MeshEdges { edges, point_edges }
}

/// `pointEdgePoint`: the origin of the nearest wall point reached so far and the
/// squared distance to it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WallInfo {
    origin: Option<V>,
    dist_sqr: f64,
}

impl WallInfo {
    fn invalid() -> Self {
        Self { origin: None, dist_sqr: GREAT }
    }
    /// `pointEdgePoint::update(pt, w2, tol)` (`pointEdgePointI.H`).
    fn update(&mut self, pt: V, w2: &WallInfo) -> bool {
        let w2_origin = w2.origin.expect("a wave only propagates valid information");
        let dist2 = mag_sqr(sub(pt, w2_origin));
        if self.origin.is_none() {
            self.dist_sqr = dist2;
            self.origin = Some(w2_origin);
            return true;
        }
        let diff = self.dist_sqr - dist2;
        if diff < 0.0 {
            return false;
        }
        if diff < SMALL || (self.dist_sqr > SMALL && diff / self.dist_sqr < PROPAGATION_TOL) {
            false
        } else {
            self.dist_sqr = dist2;
            self.origin = Some(w2_origin);
            true
        }
    }
}

/// `edgePatchDist` (v2506 `snappySnapDriver.C:631`): for every mesh edge, the
/// distance from its centre to the origin of the wall point `PointEdgeWave`
/// propagated to it (seeds: the patch points). Returns the distances and the
/// number of wave iterations.
pub fn edge_patch_dist(mesh: &SnapMesh, me: &MeshEdges, pa: &PatchAddr) -> (Vec<f64>, usize) {
    let n_points = mesh.points.len();
    let mut point_info = vec![WallInfo::invalid(); n_points];
    let mut edge_info = vec![WallInfo::invalid(); me.edges.len()];
    let mut changed_point = vec![false; n_points];
    let mut changed_edge = vec![false; me.edges.len()];
    let mut changed_points: Vec<usize> = Vec::new();
    let mut changed_edges: Vec<usize> = Vec::new();
    for &mp in &pa.mesh_points {
        point_info[mp] = WallInfo { origin: Some(mesh.points[mp]), dist_sqr: 0.0 };
        if !changed_point[mp] {
            changed_point[mp] = true;
            changed_points.push(mp);
        }
    }
    let mut iter = 0usize;
    while iter < n_points {
        // pointToEdge
        for &pointi in &changed_points {
            let neighbour = point_info[pointi];
            for &edgei in &me.point_edges[pointi] {
                if edge_info[edgei] != neighbour {
                    let [e0, e1] = me.edges[edgei];
                    let centre = scale(0.5, add(mesh.points[e0], mesh.points[e1]));
                    if edge_info[edgei].update(centre, &neighbour) && !changed_edge[edgei] {
                        changed_edge[edgei] = true;
                        changed_edges.push(edgei);
                    }
                }
            }
            changed_point[pointi] = false;
        }
        changed_points.clear();
        if changed_edges.is_empty() {
            break;
        }
        // edgeToPoint
        for &edgei in &changed_edges {
            let neighbour = edge_info[edgei];
            for ei in 0..2 {
                let v = me.edges[edgei][ei];
                if point_info[v] != neighbour && point_info[v].update(mesh.points[v], &neighbour) && !changed_point[v] {
                    changed_point[v] = true;
                    changed_points.push(v);
                }
            }
            changed_edge[edgei] = false;
        }
        changed_edges.clear();
        if changed_points.is_empty() {
            break;
        }
        iter += 1;
    }
    (edge_info.iter().map(|i| i.dist_sqr.sqrt()).collect(), iter)
}

/// `motionSmootherAlgo::avg(fld, edgeWeight)` for a vector field (serial: every
/// edge is a master edge, taken in index order).
fn avg_field(fld: &[V], me: &MeshEdges, edge_weight: &[f64]) -> Vec<V> {
    let mut res = vec![[0.0; 3]; fld.len()];
    let mut sum_weight = vec![0.0; fld.len()];
    for (edgei, e) in me.edges.iter().enumerate() {
        let w = edge_weight[edgei];
        res[e[0]] = add(res[e[0]], scale(w, fld[e[1]]));
        sum_weight[e[0]] += w;
        res[e[1]] = add(res[e[1]], scale(w, fld[e[0]]));
        sum_weight[e[1]] += w;
    }
    for i in 0..res.len() {
        if sum_weight[i].abs() < VSMALL {
            res[i] = fld[i];
        } else {
            res[i] = div(res[i], sum_weight[i]);
        }
    }
    res
}

/// `snappySnapDriver::smoothDisplacement` (v2506 `snappySnapDriver.C:2134`):
/// `n_iter` times `motionSmootherAlgo::smooth(oldDisp, edgeGamma, disp)` with
/// `edgeGamma = 1/(edgePatchDist + SMALL)`; points off the patch move to
/// `0.5 * old + 0.5 * weighted neighbour average`, patch points keep their value.
/// `disp` is the displacement field on all mesh points.
pub fn smooth_displacement(mesh: &SnapMesh, me: &MeshEdges, pa: &PatchAddr, disp: &mut [V], n_iter: usize) {
    let (dist, _) = edge_patch_dist(mesh, me, pa);
    let edge_gamma: Vec<f64> = dist.iter().map(|&d| 1.0 / (d + SMALL)).collect();
    let mut is_internal = vec![true; mesh.points.len()];
    for &mp in &pa.mesh_points {
        is_internal[mp] = false;
    }
    for _ in 0..n_iter {
        let old = disp.to_vec();
        let avg = avg_field(&old, me, &edge_gamma);
        for i in 0..disp.len() {
            if is_internal[i] {
                disp[i] = add(scale(0.5, old[i]), scale(0.5, avg[i]));
            }
        }
    }
}

/// The displacement field handed to `smoothDisplacement`: the patch displacement on
/// the patch points (`motionSmootherAlgo::setDisplacement`), zero elsewhere.
pub fn displacement_field(mesh: &SnapMesh, pa: &PatchAddr, patch_disp: &[V]) -> Vec<V> {
    let mut d = vec![[0.0; 3]; mesh.points.len()];
    for (i, &mp) in pa.mesh_points.iter().enumerate() {
        d[mp] = patch_disp[i];
    }
    d
}

/// `motionSmootherAlgo::avg(fld, edgeWeight)` for a scalar field (serial; see [`avg_field`]).
fn avg_scalar(fld: &[f64], me: &MeshEdges, edge_weight: &[f64]) -> Vec<f64> {
    let mut res = vec![0.0; fld.len()];
    let mut sum_weight = vec![0.0; fld.len()];
    for (edgei, e) in me.edges.iter().enumerate() {
        let w = edge_weight[edgei];
        res[e[0]] += w * fld[e[1]];
        sum_weight[e[0]] += w;
        res[e[1]] += w * fld[e[0]];
        sum_weight[e[1]] += w;
    }
    for i in 0..res.len() {
        if sum_weight[i].abs() < VSMALL {
            res[i] = fld[i];
        } else {
            res[i] /= sum_weight[i];
        }
    }
    res
}

/// Which displacement the patch points get in `curPoints`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchPointScaling {
    /// `scale * displacement` on the patch points too, from the scale field's values
    /// (the reading of `scaleField(pp.meshPoints(), usedPoints, errorReduction, scale)`).
    Scaled,
    /// Patch points keep the unscaled displacement (the reading in which the scale
    /// field's boundary values, still 1, are what `curPoints` multiplies there).
    Unscaled,
}

/// `scaleMesh` controls: `errorReduction`, `nSmoothScale`, the mesh-quality controls
/// and the number of faces `scaleMesh` tolerates (`nAllowableErrors`).
#[derive(Debug, Clone)]
pub struct ScaleControls {
    /// `errorReduction`.
    pub error_reduction: f64,
    /// `nSmoothScale`.
    pub n_smooth_scale: usize,
    /// `meshQualityControls`.
    pub quality: EsiQualityControls,
    /// `nAllowableErrors` (the faces in error of the initial mesh).
    pub n_allowable_errors: usize,
    /// How patch points are displaced.
    pub patch_scaling: PatchPointScaling,
}

/// Cell-face and point-cell addressing for [`affected_faces_and_points`].
#[derive(Debug, Clone)]
pub struct ScaleTopology {
    /// Faces of each cell.
    pub cell_faces: Vec<Vec<usize>>,
    /// Cells using each point, ascending.
    pub point_cells: Vec<Vec<usize>>,
}

/// Addressing of `mesh`.
pub fn scale_topology(mesh: &SnapMesh) -> ScaleTopology {
    let cell_faces = esi_geometry(mesh).cell_faces;
    let mut point_cells: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for (c, faces) in cell_faces.iter().enumerate() {
        for &f in faces {
            for &p in &mesh.faces[f] {
                point_cells[p].push(c);
            }
        }
    }
    for pc in point_cells.iter_mut() {
        pc.sort_unstable();
        pc.dedup();
    }
    ScaleTopology { cell_faces, point_cells }
}

/// The motion smoother's state: `oldPoints_`, `displacement_`, `scale_`.
#[derive(Debug, Clone)]
pub struct MotionSmoother {
    /// Points at construction (`oldPoints_`).
    pub old_points: Vec<V>,
    /// `displacement_` on all points.
    pub displacement: Vec<V>,
    /// `scale_` on all points, initially 1.
    pub scale: Vec<f64>,
    is_internal_point: Vec<bool>,
}

impl MotionSmoother {
    /// `motionSmootherData` + `updateMesh`: scale 1, `isInternalPoint_` false on patch points.
    pub fn new(mesh: &SnapMesh, pa: &PatchAddr, displacement: Vec<V>) -> Self {
        let mut is_internal_point = vec![true; mesh.points.len()];
        for &mp in &pa.mesh_points {
            is_internal_point[mp] = false;
        }
        Self { old_points: mesh.points.clone(), displacement, scale: vec![1.0; mesh.points.len()], is_internal_point }
    }

    /// `motionSmootherAlgo::curPoints`: `oldPoints + scale * displacement`.
    pub fn cur_points(&self, pa: &PatchAddr, patch_scaling: PatchPointScaling) -> Vec<V> {
        let mut is_patch = vec![false; self.old_points.len()];
        for &mp in &pa.mesh_points {
            is_patch[mp] = true;
        }
        (0..self.old_points.len())
            .map(|i| {
                let s = if is_patch[i] && patch_scaling == PatchPointScaling::Unscaled { 1.0 } else { self.scale[i] };
                add(self.old_points[i], scale(s, self.displacement[i]))
            })
            .collect()
    }
}

/// `motionSmootherAlgo::getAffectedFacesAndPoints`: grow `wrong_faces` by `n_point_iter`
/// point-cell-face layers (faces to check next) and mark the points of the layer at
/// iteration `n_point_iter - 2` as affected (points to smooth).
pub fn affected_faces_and_points(
    mesh: &SnapMesh,
    topo: &ScaleTopology,
    n_point_iter: usize,
    wrong_faces: &[usize],
) -> (Vec<usize>, Vec<bool>) {
    let mut is_affected_point = vec![false; mesh.points.len()];
    let mut nbr_faces = vec![false; mesh.faces.len()];
    for &f in wrong_faces {
        nbr_faces[f] = true;
    }
    for i in 0..n_point_iter {
        let mut nbr_points = vec![false; mesh.points.len()];
        for f in 0..mesh.faces.len() {
            if nbr_faces[f] {
                for &p in &mesh.faces[f] {
                    nbr_points[p] = true;
                }
            }
        }
        for p in 0..mesh.points.len() {
            if nbr_points[p] {
                for &c in &topo.point_cells[p] {
                    for &f in &topo.cell_faces[c] {
                        nbr_faces[f] = true;
                    }
                }
            }
        }
        if i as isize == n_point_iter as isize - 2 {
            for f in 0..mesh.faces.len() {
                if nbr_faces[f] {
                    for &p in &mesh.faces[f] {
                        is_affected_point[p] = true;
                    }
                }
            }
        }
    }
    let affected_faces: Vec<usize> = (0..mesh.faces.len()).filter(|&f| nbr_faces[f]).collect();
    (affected_faces, is_affected_point)
}

/// What one `scaleMesh` call did.
#[derive(Debug, Clone)]
pub struct ScaleIteration {
    /// Minimum of the scale field when the mesh was moved ("displacement scaling min").
    pub scale_min: f64,
    /// Maximum of the scale field when the mesh was moved.
    pub scale_max: f64,
    /// The quality check of the moved mesh.
    pub check: EsiCheckResult,
    /// True when the mesh was accepted (`faces in error <= nAllowableErrors`).
    pub ok: bool,
}

/// `motionSmootherAlgo::scaleMesh(checkFaces, baffles, paramDict, meshQualityDict,
/// smoothMesh = true, nAllowableErrors)` (v2506 `motionSmootherAlgo.C:821`), serial, no
/// baffles: move the mesh to `oldPoints + scale * displacement`, check `check_faces`;
/// when too many faces are in error scale the displacement down at their points
/// (`errorReduction`), smooth the scale (`nSmoothScale` `minSmooth` passes over patch
/// points, then internal points) and replace `check_faces` by the faces to check next.
pub fn scale_mesh(
    mesh: &mut SnapMesh,
    ms: &mut MotionSmoother,
    pa: &PatchAddr,
    me: &MeshEdges,
    topo: &ScaleTopology,
    ctl: &ScaleControls,
    check_faces: &mut Vec<usize>,
) -> ScaleIteration {
    let scale_min = ms.scale.iter().cloned().fold(f64::INFINITY, f64::min);
    let scale_max = ms.scale.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    mesh.points = ms.cur_points(pa, ctl.patch_scaling);
    let geom = esi_geometry(mesh);
    let check = esi_check_mesh(mesh, &geom, &ctl.quality, check_faces);
    if check.wrong_faces.len() <= ctl.n_allowable_errors {
        return ScaleIteration { scale_min, scale_max, check, ok: true };
    }
    let mut wrong_faces = check.wrong_faces.clone();
    if ctl.error_reduction.abs() < SMALL {
        // Extend to the faces of the owner and neighbour cells of every wrong face.
        let mut extended = vec![false; mesh.faces.len()];
        for &f in &wrong_faces {
            extended[f] = true;
            for &fi in &topo.cell_faces[mesh.owner[f]] {
                extended[fi] = true;
            }
            if f < mesh.n_internal_faces() {
                for &fi in &topo.cell_faces[mesh.neighbour[f]] {
                    extended[fi] = true;
                }
            }
        }
        wrong_faces = (0..mesh.faces.len()).filter(|&f| extended[f]).collect();
    }
    let mut used_points = vec![false; mesh.points.len()];
    for &f in &wrong_faces {
        for &p in &mesh.faces[f] {
            used_points[p] = true;
        }
    }
    let (affected_faces, is_affected_point) = affected_faces_and_points(mesh, topo, ctl.n_smooth_scale, &wrong_faces);
    // Scale conflicting patch points, then conflicting internal points.
    for &mp in &pa.mesh_points {
        if used_points[mp] {
            ms.scale[mp] *= ctl.error_reduction;
        }
    }
    for p in 0..mesh.points.len() {
        if used_points[p] && ms.is_internal_point[p] {
            ms.scale[p] *= ctl.error_reduction;
        }
    }
    let e_weights: Vec<f64> =
        me.edges.iter().map(|e| 1.0 / (mag(sub(ms.old_points[e[1]], ms.old_points[e[0]])) + SMALL)).collect();
    for _ in 0..ctl.n_smooth_scale {
        // minSmooth on the patch points
        let old = ms.scale.clone();
        let avg = avg_scalar(&old, me, &e_weights);
        for &mp in &pa.mesh_points {
            if is_affected_point[mp] {
                ms.scale[mp] = old[mp].min(0.5 * old[mp] + 0.5 * avg[mp]);
            }
        }
        // minSmooth on the internal points
        let old = ms.scale.clone();
        let avg = avg_scalar(&old, me, &e_weights);
        for p in 0..old.len() {
            if is_affected_point[p] && ms.is_internal_point[p] {
                ms.scale[p] = old[p].min(0.5 * old[p] + 0.5 * avg[p]);
            }
        }
    }
    *check_faces = affected_faces;
    ScaleIteration { scale_min, scale_max, check, ok: false }
}

/// `snappySnapDriver::scaleMesh` (v2506 `snappySnapDriver.C:2208`): up to `2 * n_snap`
/// scaling iterations over all faces; from iteration `n_snap` on the error reduction is 0.
/// Returns every iteration's report.
pub fn scale_mesh_loop(
    mesh: &mut SnapMesh,
    ms: &mut MotionSmoother,
    pa: &PatchAddr,
    me: &MeshEdges,
    topo: &ScaleTopology,
    ctl: &ScaleControls,
    n_snap: usize,
) -> Vec<ScaleIteration> {
    let mut check_faces: Vec<usize> = (0..mesh.faces.len()).collect();
    let mut ctl = ctl.clone();
    let mut reports = Vec::new();
    for iter in 0..2 * n_snap {
        if iter == n_snap {
            ctl.error_reduction = 0.0;
        }
        let r = scale_mesh(mesh, ms, pa, me, topo, &ctl, &mut check_faces);
        let ok = r.ok;
        reports.push(r);
        if ok {
            break;
        }
    }
    reports
}

/// `snappySnapDriver::repatchToSurface` (v2506 `snappySnapDriver.C:2275`), no zoned
/// surfaces or preserved faces: every boundary face whose centre has a surface triangle
/// within `faceSnapDist` (the maximum of `calcSnapDistance` over its points, on `mesh`)
/// moves to the patch of the region of the nearest triangle (`region_patch[region]`).
/// `face_patch[i]` is the current patch of patch face `i` (`pa.face_ids[i]`); returns
/// `(mesh face, new patch)` for the faces whose patch changes, in face order.
/// `tri_region[t]` is the region of `surface.triangles[t]`. Nearest triangle: first
/// minimum of the brute-force search (upstream searches an octree; equal distances to two
/// regions are the one place the two can differ).
pub fn repatch_to_surface(
    mesh: &SnapMesh,
    pa: &PatchAddr,
    face_patch: &[usize],
    snap_tol: f64,
    surface: &TriangleSoup,
    tri_region: &[usize],
    region_patch: &[usize],
) -> Vec<(usize, usize)> {
    let snap_dist = calc_snap_distance(mesh, pa, snap_tol);
    let (centres, _) = face_centres_areas(&mesh.points, &mesh.faces);
    let mut changed = Vec::new();
    for (i, &f) in pa.face_ids.iter().enumerate() {
        let face_snap_dist = pa.local_faces[i].iter().fold(-GREAT, |m, &p| m.max(snap_dist[p]));
        let c = centres[f];
        let sample = Vector3::new(c[0], c[1], c[2]);
        let mut best: Option<(f64, usize)> = None;
        for (t, tri) in surface.triangles.iter().enumerate() {
            let q = tri.closest_point(sample);
            let d2 = mag_sqr([q.x - c[0], q.y - c[1], q.z - c[2]]);
            if best.map(|(b, _)| d2 < b).unwrap_or(true) {
                best = Some((d2, t));
            }
        }
        if let Some((d2, t)) = best {
            if d2 <= face_snap_dist * face_snap_dist {
                let patch = region_patch[tri_region[t]];
                if patch != face_patch[i] {
                    changed.push((f, patch));
                }
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two unit cubes side by side along x: points 0..12, internal face 0.
    fn two_cubes() -> SnapMesh {
        let mut points = Vec::new();
        for i in 0..3 {
            for j in 0..2 {
                for k in 0..2 {
                    points.push([i as f64, j as f64, k as f64]);
                }
            }
        }
        let p = |i: usize, j: usize, k: usize| i * 4 + j * 2 + k;
        let internal = vec![vec![p(1, 0, 0), p(1, 1, 0), p(1, 1, 1), p(1, 0, 1)]];
        let mut faces = internal;
        let mut owner = vec![0];
        for (cell, i0) in [(0usize, 0usize), (1, 1)] {
            let i1 = i0 + 1;
            let mut ring = |v: Vec<usize>| {
                faces.push(v);
                owner.push(cell);
            };
            ring(vec![p(i0, 0, 0), p(i1, 0, 0), p(i1, 0, 1), p(i0, 0, 1)]);
            ring(vec![p(i0, 1, 0), p(i0, 1, 1), p(i1, 1, 1), p(i1, 1, 0)]);
            ring(vec![p(i0, 0, 0), p(i0, 1, 0), p(i1, 1, 0), p(i1, 0, 0)]);
            ring(vec![p(i0, 0, 1), p(i1, 0, 1), p(i1, 1, 1), p(i0, 1, 1)]);
        }
        faces.push(vec![p(0, 0, 0), p(0, 0, 1), p(0, 1, 1), p(0, 1, 0)]);
        owner.push(0);
        faces.push(vec![p(2, 0, 0), p(2, 1, 0), p(2, 1, 1), p(2, 0, 1)]);
        owner.push(1);
        SnapMesh { points, faces, owner, neighbour: vec![1] }
    }

    #[test]
    fn patch_addressing_orders_points_by_first_appearance() {
        let m = two_cubes();
        let pa = patch_addressing(&m);
        assert_eq!(pa.face_ids.len(), 10);
        assert_eq!(pa.mesh_points.len(), 12);
        assert_eq!(pa.mesh_points[0], 0);
        assert!(pa.point_faces.iter().all(|f| f.windows(2).all(|w| w[0] < w[1])));
    }

    #[test]
    fn face_centre_of_a_unit_square_is_its_middle() {
        let m = two_cubes();
        let (c, a) = face_centres_areas(&m.points, &m.faces);
        assert_eq!(c[0], [1.0, 0.5, 0.5]);
        assert!((mag(a[0]) - 1.0).abs() < 1e-15);
        assert_eq!(face_centre(&m.points, &m.faces[0]), c[0]);
        let cc = cell_centres(&m);
        assert_eq!(cc[0], [0.5, 0.5, 0.5]);
        assert_eq!(cc[1], [1.5, 0.5, 0.5]);
    }

    #[test]
    fn edge_wave_distance_is_half_an_edge_when_every_point_is_a_wall_point() {
        let m = two_cubes();
        let pa = patch_addressing(&m);
        let me = mesh_edges(&m);
        assert_eq!(me.edges.len(), 20);
        let (d, _) = edge_patch_dist(&m, &me, &pa);
        assert!(d.iter().all(|&x| x == 0.5), "every edge is 1 long with a wall point at each end");
    }

    #[test]
    fn smoothing_with_every_point_on_the_patch_changes_nothing() {
        let m = two_cubes();
        let pa = patch_addressing(&m);
        let me = mesh_edges(&m);
        let mut d = displacement_field(&m, &pa, &vec![[0.1, 0.0, 0.0]; 12]);
        smooth_displacement(&m, &me, &pa, &mut d, 5);
        assert!(d.iter().all(|v| *v == [0.1, 0.0, 0.0]));
        // Patch displacement of the box with nothing to attract towards.
        let disp = smooth_patch_displacement(&m, &pa);
        assert_eq!(disp.len(), 12);
        let mut m2 = m.clone();
        let used = pre_smooth_patch_iteration(&mut m2, &pa);
        assert_eq!(used, disp);
    }

    #[test]
    fn snap_distance_nearest_surface_and_outwards_check() {
        use crate::snappy_hex_mesh::stl::Triangle;
        let m = two_cubes();
        let pa = patch_addressing(&m);
        let sd = calc_snap_distance(&m, &pa, 1.0);
        assert!(sd.iter().all(|&s| s == 1.0));
        // A plane x = -0.25 (two triangles) attracts the x = 0 points only within range 1.
        let t = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
            Triangle::new(Vector3::new(a[0], a[1], a[2]), Vector3::new(b[0], b[1], b[2]), Vector3::new(c[0], c[1], c[2]))
        };
        let s = TriangleSoup::new(
            "plane",
            vec![
                t([-0.25, -5.0, -5.0], [-0.25, 5.0, -5.0], [-0.25, 5.0, 5.0]),
                t([-0.25, -5.0, -5.0], [-0.25, 5.0, 5.0], [-0.25, -5.0, 5.0]),
            ],
        );
        let lp: Vec<V> = pa.mesh_points.iter().map(|&p| m.points[p]).collect();
        let (disp, missing) = calc_nearest_surface(&lp, &sd, &s);
        assert_eq!(missing, 8, "the x = 1 and x = 2 points are 1.25 and 2.25 away");
        assert!(disp.iter().zip(&lp).all(|(d, p)| p[0] != 0.0 || (d[0] == -0.25 && d[1] == 0.0)));
        // Point 0 moves in -x but also borders the y = 0 face, tangent to the move:
        // dot = 0 < SMALL, so upstream reports it as pointing through the faces.
        assert_eq!(outwards_displacement(&m, &pa, &disp), Some(0));
    }
}
