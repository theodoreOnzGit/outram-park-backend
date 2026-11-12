// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
//
// Upstream: OpenFOAM.com ESI v2506, `src/mesh/snappyHexMesh/externalDisplacementMeshMover/`
// `medialAxisMeshMover.C`, `fieldSmoother/fieldSmoother.C`, `fieldSmootherTemplates.C`;
// `snappyHexMeshDriver/snappyLayerDriver.C` (`getPatchDisplacement`, `syncPatchDisplacement`);
// `meshRefinement.C` (`calculateEdgeWeights`), `meshRefinementTemplates.C` (`weightedSum`);
// `OpenFOAM/meshes/primitiveMesh/PatchTools/PatchToolsNormals.C` (`pointNormals`);
// `meshTools/algorithms/PointEdgeWave` with `PointData` and `pointEdgePoint`.

//! gh:#1467 round 8, stage 2: the displacement of ESI `medialAxisMeshMover` for one layer
//! iteration: `update` (wall distance, medial axis, normal smoothing), `calculateDisplacement`
//! (thickness truncation, isolated regions, thickness smoothing, displacement) and the scaling
//! loop `shrinkMesh` (the `motionSmootherAlgo::scaleMesh` of `esi_snap`).
//!
//! Opt-in, serial, no faceZones, no non-adapt patches with points (ESI's slip/fixedValue
//! patches are not modelled; the flange has only the empty `allBoundary`). Methodology,
//! predictions P52-P54 and results: `verification_and_validation/snappy_hex_mesh_esi_v2506/
//! README.md`, "Round 8".

use super::esi_layers::{ExtrudeMode, ExtrudeState, LayerPatch, GREAT, SMALL};
use super::esi_snap::{
    add, div, dot, mag, mag_sqr, scale, sub, MeshEdges, MotionSmoother, PatchAddr, PatchPointScaling, ScaleControls,
    ScaleIteration, ScaleTopology, SnapMesh, V,
};

/// OpenFOAM `VSMALL`.
const VSMALL: f64 = 1e-300;
/// OpenFOAM `ROOTVSMALL`.
const ROOT_V_SMALL: f64 = 1e-150;
/// `PointEdgeWave::propagationTol_`.
const PROPAGATION_TOL: f64 = 0.01;

/// `normalised(v)`: zero when `|v| < ROOTVSMALL`.
pub fn normalised(v: V) -> V {
    let s = mag(v);
    if s < ROOT_V_SMALL {
        [0.0; 3]
    } else {
        div(v, s)
    }
}

/// `PatchTools::pointNormals(mesh, pp)` (serial): the face normals summed in face order,
/// divided by `mag + VSMALL`.
pub fn patch_point_normals(pp: &LayerPatch) -> Vec<V> {
    let mut n = vec![[0.0; 3]; pp.n_points()];
    for (fi, f) in pp.local_faces.iter().enumerate() {
        for &p in f {
            n[p] = add(n[p], pp.face_normals[fi]);
        }
    }
    for v in n.iter_mut() {
        let m = mag(*v) + VSMALL;
        *v = div(*v, m);
    }
    n
}

/// `meshTools::visNormal`.
pub fn vis_normal(n: V, face_normals: &[V], faces: &[usize]) -> bool {
    faces.iter().all(|&f| dot(face_normals[f], n) >= SMALL)
}

// ---------------------------------------------------------------------------------
// snappyLayerDriver::getPatchDisplacement / syncPatchDisplacement
// ---------------------------------------------------------------------------------

/// `snappyLayerDriver::syncPatchDisplacement` (serial: the coupled syncs are identities).
pub fn sync_patch_displacement(min_thickness: &[f64], st: &mut ExtrudeState) {
    loop {
        let mut n_changed = 0;
        for i in 0..st.disp.len() {
            if mag(st.disp[i]) < min_thickness[i] && st.unmark_point(i) {
                n_changed += 1;
            }
        }
        if n_changed == 0 {
            break;
        }
    }
}

/// What `getPatchDisplacement` reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchDisplacementCounts {
    /// "points with point normal pointing through faces".
    pub n_no_vis_normal: usize,
    /// "points reset to average of surrounding points".
    pub n_extrude_remove: usize,
}

/// `snappyLayerDriver::getPatchDisplacement`: `patchDisp = thickness * pointNormals`, points
/// without a visible normal take the average of their extruded neighbours, then
/// `syncPatchDisplacement`.
pub fn get_patch_displacement(
    pp: &LayerPatch,
    points: &[V],
    thickness: &[f64],
    min_thickness: &[f64],
    st: &mut ExtrudeState,
) -> PatchDisplacementCounts {
    let point_normals = patch_point_normals(pp);
    let n = pp.n_points();
    for i in 0..n {
        st.disp[i] = scale(thickness[i], point_normals[i]);
    }
    let mut n_no_vis = 0;
    for i in 0..n {
        if st.status[i] == ExtrudeMode::NoExtrude {
            st.n_layers[i] = 0;
            st.disp[i] = [0.0; 3];
        } else if !vis_normal(point_normals[i], &pp.face_normals, &pp.point_faces[i]) {
            st.status[i] = ExtrudeMode::ExtrudeRemove;
            n_no_vis += 1;
        }
    }
    let local_points: Vec<V> = pp.mesh_points.iter().map(|&p| points[p]).collect();
    let mut n_remove = 0;
    for i in 0..n {
        if st.status[i] == ExtrudeMode::ExtrudeRemove {
            let mut avg = [0.0; 3];
            let mut n_points = 0usize;
            for &ei in &pp.point_edges[i] {
                let e = pp.edges[ei];
                let other = if e[0] == i { e[1] } else { e[0] };
                if st.status[other] != ExtrudeMode::NoExtrude {
                    avg = add(avg, add(local_points[other], st.disp[other]));
                    n_points += 1;
                }
            }
            if n_points > 0 {
                st.disp[i] = sub(div(avg, n_points as f64), local_points[i]);
                n_remove += 1;
            }
        }
    }
    sync_patch_displacement(min_thickness, st);
    PatchDisplacementCounts { n_no_vis_normal: n_no_vis, n_extrude_remove: n_remove }
}

// ---------------------------------------------------------------------------------
// PointEdgeWave with PointData
// ---------------------------------------------------------------------------------

/// `PointData<Type>` (`pointEdgePoint` plus carried data; scalar data lives in component 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveInfo {
    /// Origin of the nearest source (`None`: not valid, `point::max`).
    pub origin: Option<V>,
    /// Squared distance to the origin.
    pub dist_sqr: f64,
    /// Carried data.
    pub data: V,
}

impl WaveInfo {
    /// The default-constructed (invalid) `PointData`.
    pub fn invalid() -> Self {
        Self { origin: None, dist_sqr: -GREAT, data: [0.0; 3] }
    }

    /// `pointEdgePoint::valid`.
    pub fn valid(&self) -> bool {
        self.origin.is_some()
    }

    /// `pointEdgePoint::update(pt, w2, tol)` and, when it returns true, the data copy of
    /// `PointData::updatePoint/updateEdge`.
    fn update(&mut self, pt: V, w2: &WaveInfo) -> bool {
        let w2_origin = w2.origin.expect("a wave only propagates valid information");
        let dist2 = mag_sqr(sub(pt, w2_origin));
        if !self.valid() {
            self.dist_sqr = dist2;
            self.origin = Some(w2_origin);
            self.data = w2.data;
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
            self.data = w2.data;
            true
        }
    }
}

/// `PointEdgeWave<..>` serial: seeds `(point, info)` overwrite the point information; the wave
/// runs for at most `max_iter` iterations. Returns the point information and the iterations
/// done.
pub fn point_edge_wave(points: &[V], me: &MeshEdges, seeds: &[(usize, WaveInfo)], max_iter: usize) -> (Vec<WaveInfo>, usize) {
    point_edge_wave_from(points, me, vec![WaveInfo::invalid(); points.len()], seeds, max_iter)
}

/// As [`point_edge_wave`] but starting from existing point information (ESI's medial wave
/// pre-assigns its seeds into `allPointInfo`).
pub fn point_edge_wave_from(
    points: &[V],
    me: &MeshEdges,
    mut point_info: Vec<WaveInfo>,
    seeds: &[(usize, WaveInfo)],
    max_iter: usize,
) -> (Vec<WaveInfo>, usize) {
    let n_points = points.len();
    let mut edge_info = vec![WaveInfo::invalid(); me.edges.len()];
    let mut changed_point = vec![false; n_points];
    let mut changed_edge = vec![false; me.edges.len()];
    let mut changed_points: Vec<usize> = Vec::new();
    let mut changed_edges: Vec<usize> = Vec::new();
    for &(p, info) in seeds {
        point_info[p] = info;
        if !changed_point[p] {
            changed_point[p] = true;
            changed_points.push(p);
        }
    }
    let mut iter = 0usize;
    while iter < max_iter {
        for &pointi in &changed_points {
            let neighbour = point_info[pointi];
            for &edgei in &me.point_edges[pointi] {
                if edge_info[edgei] != neighbour {
                    let [e0, e1] = me.edges[edgei];
                    let centre = scale(0.5, add(points[e0], points[e1]));
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
        for &edgei in &changed_edges {
            let neighbour = edge_info[edgei];
            for ei in 0..2 {
                let v = me.edges[edgei][ei];
                if point_info[v] != neighbour && point_info[v].update(points[v], &neighbour) && !changed_point[v] {
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
    (point_info, iter)
}

// ---------------------------------------------------------------------------------
// fieldSmoother
// ---------------------------------------------------------------------------------

/// `meshRefinement::weightedSum` (serial, every edge a master edge) for vector data.
fn weighted_sum_v(n_points: usize, edges: &[[usize; 2]], w: &[f64], data: &[V]) -> Vec<V> {
    let mut sum = vec![[0.0; 3]; n_points];
    for (ei, e) in edges.iter().enumerate() {
        let (v0, v1) = (e[0], e[1]);
        sum[v0] = add(sum[v0], scale(w[ei], data[v1]));
        sum[v1] = add(sum[v1], scale(w[ei], data[v0]));
    }
    sum
}

/// `meshRefinement::weightedSum` for scalar data.
fn weighted_sum_s(n_points: usize, edges: &[[usize; 2]], w: &[f64], data: &[f64]) -> Vec<f64> {
    let mut sum = vec![0.0; n_points];
    for (ei, e) in edges.iter().enumerate() {
        let (v0, v1) = (e[0], e[1]);
        sum[v0] += w[ei] * data[v1];
        sum[v1] += w[ei] * data[v0];
    }
    sum
}

/// `meshRefinement::calculateEdgeWeights`: `edgeWeights = 1/max(SMALL, |edge|)`, and the inverse
/// of the per-point sum of the weights (left at its value when not positive).
/// `edges` index into `mesh_points`.
pub fn calculate_edge_weights(points: &[V], mesh_points: &[usize], edges: &[[usize; 2]]) -> (Vec<f64>, Vec<f64>) {
    let w: Vec<f64> = edges
        .iter()
        .map(|e| {
            let e_mag = SMALL.max(mag(sub(points[mesh_points[e[1]]], points[mesh_points[e[0]]])));
            1.0 / e_mag
        })
        .collect();
    let ones = vec![1.0; mesh_points.len()];
    let mut inv = weighted_sum_s(mesh_points.len(), edges, &w, &ones);
    for v in inv.iter_mut() {
        if *v > 0.0 {
            *v = 1.0 / *v;
        }
    }
    (w, inv)
}

/// `meshRefinement::gAverage` over every element (serial, every element a master).
fn g_average(values: &[f64]) -> f64 {
    let mut sum = 0.0;
    for &v in values {
        sum += v;
    }
    sum / values.len() as f64
}

/// `fieldSmoother::smoothPatchNormals`: `n_iter` times `normal = normalised(0.5 (normal +
/// average))`. Returns the residual printed at iteration 0 (`NaN` for `n_iter = 0`).
pub fn smooth_patch_normals(points: &[V], pp: &LayerPatch, n_iter: usize, normals: &mut [V]) -> f64 {
    let (w, inv) = calculate_edge_weights(points, &pp.mesh_points, &pp.edges);
    let mut resid0 = f64::NAN;
    for iter in 0..n_iter {
        let mut average = weighted_sum_v(pp.n_points(), &pp.edges, &w, normals);
        for i in 0..average.len() {
            average[i] = scale(inv[i], average[i]);
        }
        if iter % 10 == 0 {
            let r = g_average(&(0..normals.len()).map(|i| mag(sub(normals[i], average[i]))).collect::<Vec<_>>());
            if iter == 0 {
                resid0 = r;
            }
        }
        for i in 0..average.len() {
            average[i] = scale(0.5, add(normals[i], average[i]));
            normals[i] = normalised(average[i]);
        }
    }
    resid0
}

/// `fieldSmoother::smoothNormals`: as above over all mesh points and edges; the points of
/// `fixed_points` keep their value. Returns the iteration-0 residual.
pub fn smooth_normals(points: &[V], me: &MeshEdges, n_iter: usize, fixed_points: &[usize], normals: &mut [V]) -> f64 {
    let n = points.len();
    let mut is_fixed = vec![false; n];
    for &p in fixed_points {
        is_fixed[p] = true;
    }
    let mesh_points: Vec<usize> = (0..n).collect();
    let (w, inv) = calculate_edge_weights(points, &mesh_points, &me.edges);
    let mut resid0 = f64::NAN;
    for iter in 0..n_iter {
        let mut average = weighted_sum_v(n, &me.edges, &w, normals);
        for i in 0..n {
            average[i] = scale(inv[i], average[i]);
        }
        if iter % 10 == 0 {
            let r = g_average(&(0..n).map(|i| mag(sub(normals[i], average[i]))).collect::<Vec<_>>());
            if iter == 0 {
                resid0 = r;
            }
        }
        for i in 0..n {
            if !is_fixed[i] {
                average[i] = scale(0.5, add(normals[i], average[i]));
                normals[i] = normalised(average[i]);
            }
        }
    }
    resid0
}

/// `fieldSmoother::minSmoothField` for a scalar field: a point takes the blend of itself and its
/// neighbours' weighted average only if that is smaller in magnitude than its value and not
/// below `field_min_mag`. Returns the iteration-0 residual.
pub fn min_smooth_field(points: &[V], pp: &LayerPatch, n_iter: usize, field_min_mag: &[f64], field: &mut [f64]) -> f64 {
    let (w, inv) = calculate_edge_weights(points, &pp.mesh_points, &pp.edges);
    let mut resid0 = f64::NAN;
    for iter in 0..n_iter {
        let mut average = weighted_sum_s(pp.n_points(), &pp.edges, &w, field);
        for i in 0..average.len() {
            average[i] *= inv[i];
        }
        for i in 0..field.len() {
            average[i] = 0.5 * (field[i] + average[i]);
            if average[i].abs() < field[i].abs() && average[i].abs() >= field_min_mag[i].abs() {
                field[i] = average[i];
            }
        }
        if iter % 10 == 0 {
            let r = g_average(&(0..field.len()).map(|i| (field[i] - average[i]).abs()).collect::<Vec<_>>());
            if iter == 0 {
                resid0 = r;
            }
        }
    }
    resid0
}

// ---------------------------------------------------------------------------------
// medialAxisMeshMover
// ---------------------------------------------------------------------------------

/// The `update()` controls (`addLayersControls` merged with the mesh-quality dictionary).
#[derive(Debug, Clone)]
pub struct MedialParams {
    /// `nSmoothSurfaceNormals`.
    pub n_smooth_surface_normals: usize,
    /// `cos(degToRad(minMedialAxisAngle))`.
    pub min_medial_axis_angle_cos: f64,
    /// `nSmoothNormals`.
    pub n_smooth_normals: usize,
    /// `nMedialAxisIter` (default: the number of mesh points).
    pub n_medial_axis_iter: usize,
    /// `disableWallEdges` (default false).
    pub disable_wall_edges: bool,
}

/// The fields `update()` builds: `medialRatio_`, `medialDist_`, `medialVec_`, `dispVec_`.
#[derive(Debug, Clone)]
pub struct MedialAxis {
    /// `medialRatio_`.
    pub medial_ratio: Vec<f64>,
    /// `pointMedialDist` (`medialDist_`).
    pub medial_dist: Vec<f64>,
    /// `medialVec_`.
    pub medial_vec: Vec<V>,
    /// `dispVec_`.
    pub disp_vec: Vec<V>,
    /// Residual printed by `smoothPatchNormals` at iteration 0.
    pub patch_normal_residual: f64,
    /// Residual printed by `smoothNormals` at iteration 0.
    pub interior_normal_residual: f64,
}

/// `medialAxisMeshMover::isMaxEdge`.
fn is_max_edge(points: &[V], e: [usize; 2], wall: &[WaveInfo], min_cos: f64, disable_wall_edges: bool) -> bool {
    if disable_wall_edges {
        let v0 = sub(points[e[0]], wall[e[0]].origin.unwrap());
        if mag(v0) < SMALL {
            return false;
        }
        let v1 = sub(points[e[1]], wall[e[1]].origin.unwrap());
        if mag(v1) < SMALL {
            return false;
        }
    }
    dot(wall[e[0]].data, wall[e[1]].data) < min_cos
}

impl MedialAxis {
    /// `medialAxisMeshMover::update` on the mesh `points`, with `pp` the layer patch.
    pub fn update(points: &[V], me: &MeshEdges, pp: &LayerPatch, p: &MedialParams) -> Self {
        let n_points = points.len();
        let mesh_points = &pp.mesh_points;
        // surface normals, smoothed
        let mut point_normals = patch_point_normals(pp);
        let patch_normal_residual = smooth_patch_normals(points, pp, p.n_smooth_surface_normals, &mut point_normals);
        // distance to the wall, carrying the normal
        let wall_seeds: Vec<(usize, WaveInfo)> = mesh_points
            .iter()
            .enumerate()
            .map(|(i, &mp)| (mp, WaveInfo { origin: Some(points[mp]), dist_sqr: 0.0, data: point_normals[i] }))
            .collect();
        let (wall_dist, _) = point_edge_wave(points, me, &wall_seeds, p.n_medial_axis_iter);
        // medial axis seeds
        let mut medial: Vec<WaveInfo> = vec![WaveInfo::invalid(); n_points];
        let mut max_points: Vec<usize> = Vec::new();
        let mut max_info: Vec<WaveInfo> = Vec::new();
        let seed = |pt: usize, origin: V, d2: f64, medial: &mut Vec<WaveInfo>, mp: &mut Vec<usize>, mi: &mut Vec<WaveInfo>| {
            let info = WaveInfo { origin: Some(origin), dist_sqr: d2, data: [0.0; 3] };
            mp.push(pt);
            mi.push(info);
            medial[pt] = info;
        };
        for e in me.edges.iter().copied() {
            if !wall_dist[e[0]].valid() || !wall_dist[e[1]].valid() {
                for pt in e {
                    if !medial[pt].valid() {
                        seed(pt, points[pt], 0.0, &mut medial, &mut max_points, &mut max_info);
                    }
                }
            } else if is_max_edge(points, e, &wall_dist, p.min_medial_axis_angle_cos, p.disable_wall_edges) {
                let mut e_vec = sub(points[e[1]], points[e[0]]);
                let e_mag = mag(e_vec);
                if e_mag > VSMALL {
                    e_vec = div(e_vec, e_mag);
                    let p0 = points[e[0]];
                    let origin0 = wall_dist[e[0]].origin.unwrap();
                    let p1 = points[e[1]];
                    let origin1 = wall_dist[e[1]].origin.unwrap();
                    let dist0 = dot(sub(p0, origin0), e_vec);
                    let dist1 = dot(sub(origin1, p1), e_vec);
                    let s = 0.5 * (dist1 + e_mag + dist0);
                    let mut medial_axis_pt: Option<V> = None;
                    if s <= dist0 {
                        if mag_sqr(sub(p0, origin0)) > SMALL * SMALL {
                            medial_axis_pt = Some(p0);
                        }
                    } else if s >= dist0 + e_mag {
                        if mag_sqr(sub(p1, origin1)) > SMALL * SMALL {
                            medial_axis_pt = Some(p1);
                        }
                    } else {
                        medial_axis_pt = Some(add(p0, scale(s - dist0, e_vec)));
                    }
                    if let Some(m) = medial_axis_pt {
                        for pt in e {
                            if !medial[pt].valid() {
                                let d2 = mag_sqr(sub(points[pt], m));
                                seed(pt, m, d2, &mut medial, &mut max_points, &mut max_info);
                            }
                        }
                    }
                }
            }
        }
        let seeds: Vec<(usize, WaveInfo)> = max_points.iter().copied().zip(max_info.iter().copied()).collect();
        let (medial_dist_info, _) = point_edge_wave_from(points, me, medial, &seeds, 2 * p.n_medial_axis_iter);
        let mut medial_dist = vec![0.0; n_points];
        let mut medial_vec = vec![[0.0; 3]; n_points];
        for i in 0..n_points {
            if medial_dist_info[i].valid() {
                medial_dist[i] = medial_dist_info[i].dist_sqr.sqrt();
                medial_vec[i] = medial_dist_info[i].origin.unwrap();
            } else {
                medial_dist[i] = 0.0;
                medial_vec[i] = [1.0, 0.0, 0.0];
            }
        }
        let mut disp_vec: Vec<V> =
            (0..n_points).map(|i| if wall_dist[i].valid() { wall_dist[i].data } else { [1.0, 0.0, 0.0] }).collect();
        let interior_normal_residual = smooth_normals(points, me, p.n_smooth_normals, mesh_points, &mut disp_vec);
        let medial_ratio: Vec<f64> = (0..n_points)
            .map(|i| {
                if !wall_dist[i].valid() {
                    0.0
                } else {
                    let w_dist2 = wall_dist[i].dist_sqr;
                    let m_dist = medial_dist[i];
                    if w_dist2 < SMALL * SMALL && m_dist < SMALL {
                        0.0
                    } else {
                        m_dist / (w_dist2.sqrt() + m_dist)
                    }
                }
            })
            .collect();
        Self { medial_ratio, medial_dist, medial_vec, disp_vec, patch_normal_residual, interior_normal_residual }
    }
}

/// `calculateDisplacement` controls.
#[derive(Debug, Clone)]
pub struct DisplacementParams {
    /// `maxThicknessToMedialRatio`.
    pub max_thickness_to_medial_ratio: f64,
    /// `cos(degToRad(layerTerminationAngle))`, default angle `0.5 * featureAngle`.
    pub min_cos_layer_termination: f64,
    /// `nSmoothThickness`.
    pub n_smooth_thickness: usize,
    /// `nMedialAxisIter`.
    pub n_medial_axis_iter: usize,
    /// `detectExtrusionIsland` (default false).
    pub detect_extrusion_island: bool,
    /// `nSmoothDisplacement` (default 0; non-zero is not ported).
    pub n_smooth_displacement: usize,
}

/// What `calculateDisplacement` reported.
#[derive(Debug, Clone)]
pub struct DisplacementReport {
    /// "Reducing layer thickness at N nodes".
    pub n_thickness_reduced: usize,
    /// "Number of isolated points extrusion stopped".
    pub n_isolated: usize,
    /// Residual of `minSmoothField` at iteration 0.
    pub thickness_residual: f64,
    /// The smoothed thickness per patch point.
    pub thickness: Vec<f64>,
}

/// `medialAxisMeshMover::handleFeatureAngleLayerTerminations`.
fn handle_feature_angle_layer_terminations(pp: &LayerPatch, min_cos: f64, st: &mut ExtrudeState, counter: &mut usize) {
    let mut extruded_faces = vec![true; pp.local_faces.len()];
    for (fi, f) in pp.local_faces.iter().enumerate() {
        if f.iter().any(|&p| st.status[p] == ExtrudeMode::NoExtrude) {
            extruded_faces[fi] = false;
        }
    }
    for (ei, ef) in pp.edge_faces.iter().enumerate() {
        if ef.len() == 2 {
            let e = pp.edges[ei];
            let (v0, v1) = (e[0], e[1]);
            if st.status[v0] != ExtrudeMode::NoExtrude || st.status[v1] != ExtrudeMode::NoExtrude {
                if !extruded_faces[ef[0]] || !extruded_faces[ef[1]] {
                    let n0 = pp.face_normals[ef[0]];
                    let n1 = pp.face_normals[ef[1]];
                    if dot(n0, n1) < min_cos {
                        if st.unmark_point(v0) {
                            *counter += 1;
                        }
                        if st.unmark_point(v1) {
                            *counter += 1;
                        }
                    }
                }
            }
        }
    }
}

/// `medialAxisMeshMover::findIsolatedRegions`. Returns the number of points stopped.
fn find_isolated_regions(
    pp: &LayerPatch,
    min_cos_layer_termination: f64,
    detect_extrusion_island: bool,
    min_thickness: &[f64],
    st: &mut ExtrudeState,
) -> usize {
    let mut counter = 0usize;
    loop {
        if min_cos_layer_termination > -1.0 {
            handle_feature_angle_layer_terminations(pp, min_cos_layer_termination, st, &mut counter);
            sync_patch_displacement(min_thickness, st);
        }
        let mut kept = vec![false; pp.n_points()];
        if detect_extrusion_island {
            let mut island_point = vec![-1i64; pp.local_faces.len()];
            for (fi, f) in pp.local_faces.iter().enumerate() {
                for &p in f {
                    if st.status[p] != ExtrudeMode::NoExtrude {
                        if island_point[fi] == -1 {
                            island_point[fi] = p as i64;
                        } else if island_point[fi] != -2 {
                            island_point[fi] = -2;
                        }
                    }
                }
            }
            for p in 0..pp.n_points() {
                if st.status[p] != ExtrudeMode::NoExtrude && pp.point_faces[p].iter().any(|&f| island_point[f] != p as i64) {
                    kept[p] = true;
                }
            }
        } else {
            let extruded_faces: Vec<bool> =
                pp.local_faces.iter().map(|f| !f.iter().any(|&p| st.status[p] == ExtrudeMode::NoExtrude)).collect();
            for p in 0..pp.n_points() {
                if pp.point_faces[p].iter().any(|&f| extruded_faces[f]) {
                    kept[p] = true;
                }
            }
        }
        let mut n_changed = 0;
        for p in 0..pp.n_points() {
            if !kept[p] && st.unmark_point(p) {
                counter += 1;
                n_changed += 1;
            }
        }
        if n_changed == 0 {
            break;
        }
    }
    let mut isolated = vec![0i64; pp.n_points()];
    for e in &pp.edges {
        let (v0, v1) = (e[0], e[1]);
        if st.status[v1] != ExtrudeMode::NoExtrude {
            isolated[v0] += 1;
        }
        if st.status[v0] != ExtrudeMode::NoExtrude {
            isolated[v1] += 1;
        }
    }
    for fi in 0..pp.local_faces.len() {
        let f = pp.local_faces[fi].clone();
        let failed = f.iter().any(|&p| isolated[p] > 2);
        if !failed && f.iter().all(|&p| st.status[p] != ExtrudeMode::NoExtrude) {
            for &p in &f {
                if st.unmark_point(p) {
                    counter += 1;
                }
            }
        }
    }
    counter
}

/// `medialAxisMeshMover::calculateDisplacement` on the mesh `points`. `st.disp` is the patch
/// displacement as handed to `move` (pointing into the domain), `st.status` the extrusion
/// status as `move` derives it; both are updated. Returns the displacement of **all** mesh
/// points and the report.
pub fn calculate_displacement(
    points: &[V],
    me: &MeshEdges,
    pp: &LayerPatch,
    medial: &MedialAxis,
    min_thickness: &[f64],
    dp: &DisplacementParams,
    st: &mut ExtrudeState,
) -> (Vec<V>, DisplacementReport) {
    assert_eq!(dp.n_smooth_displacement, 0, "nSmoothDisplacement > 0 (smoothLambdaMuDisplacement) is not ported");
    let n = pp.n_points();
    let mut thickness: Vec<f64> = st.disp.iter().map(|&d| mag(d)).collect();
    for i in 0..n {
        if st.status[i] == ExtrudeMode::NoExtrude {
            thickness[i] = 0.0;
        }
    }
    let mut n_reduce = 0usize;
    for (i, &mp) in pp.mesh_points.iter().enumerate() {
        if st.status[i] != ExtrudeMode::NoExtrude {
            let m_dist = medial.medial_dist[mp];
            let mut thickness_ratio = thickness[i] / (m_dist + VSMALL);
            let nrm = normalised(st.disp[i]);
            let m_vec = normalised(sub(medial.medial_vec[mp], points[mp]));
            thickness_ratio *= dot(nrm, m_vec);
            if thickness_ratio > dp.max_thickness_to_medial_ratio {
                thickness[i] = 0.5 * (min_thickness[i] + thickness[i]);
                st.disp[i] = scale(thickness[i], nrm);
                n_reduce += 1;
            }
        }
    }
    let n_isolated =
        find_isolated_regions(pp, dp.min_cos_layer_termination, dp.detect_extrusion_island, min_thickness, st);
    for i in 0..n {
        if st.status[i] == ExtrudeMode::NoExtrude {
            thickness[i] = 0.0;
        }
    }
    let thickness_residual = min_smooth_field(points, pp, dp.n_smooth_thickness, min_thickness, &mut thickness);
    // distance to the patch, carrying the thickness
    let seeds: Vec<(usize, WaveInfo)> = pp
        .mesh_points
        .iter()
        .enumerate()
        .map(|(i, &mp)| (mp, WaveInfo { origin: Some(points[mp]), dist_sqr: 0.0, data: [thickness[i], 0.0, 0.0] }))
        .collect();
    let (wall, _) = point_edge_wave(points, me, &seeds, dp.n_medial_axis_iter);
    let mut displacement = vec![[0.0; 3]; points.len()];
    for i in 0..points.len() {
        if wall[i].valid() {
            let s = -medial.medial_ratio[i] * wall[i].data[0];
            displacement[i] = scale(s, medial.disp_vec[i]);
        }
    }
    (displacement, DisplacementReport { n_thickness_reduced: n_reduce, n_isolated, thickness_residual, thickness })
}

/// What one `move` did.
#[derive(Debug, Clone)]
pub struct MoveReport {
    /// Report of `calculateDisplacement`.
    pub displacement: DisplacementReport,
    /// The `scaleMesh` iterations of `shrinkMesh`.
    pub scale_iterations: Vec<ScaleIteration>,
}

/// `medialAxisMeshMover::move` (+ `shrinkMesh`): `patch_disp` is the displacement handed to the
/// mover (`-patchDisp` of the layer driver, into the domain); the mesh `mesh` is moved in place
/// to the shrunk position. Returns the report; `st.disp` afterwards holds the truncated patch
/// displacement of `calculateDisplacement`.
#[allow(clippy::too_many_arguments)]
pub fn move_mesh(
    mesh: &mut SnapMesh,
    me: &MeshEdges,
    topo: &ScaleTopology,
    pp: &LayerPatch,
    medial: &MedialAxis,
    min_thickness: &[f64],
    dp: &DisplacementParams,
    patch_disp: &[V],
    quality: &ScaleControls,
    n_relax_iter: usize,
) -> MoveReport {
    let n = pp.n_points();
    let mut st = ExtrudeState::new(n);
    st.disp = patch_disp.to_vec();
    for i in 0..n {
        if mag(st.disp[i]) <= min_thickness[i] + SMALL {
            st.status[i] = ExtrudeMode::NoExtrude;
        }
    }
    let points = mesh.points.clone();
    let (displacement, rep) = calculate_displacement(&points, me, pp, medial, min_thickness, dp, &mut st);
    let pa = PatchAddr {
        face_ids: pp.addressing.clone(),
        mesh_points: pp.mesh_points.clone(),
        point_faces: pp.point_faces.clone(),
        local_faces: pp.local_faces.clone(),
    };
    let mut ms = MotionSmoother::new(mesh, &pa, displacement);
    let mut ctl = quality.clone();
    ctl.patch_scaling = PatchPointScaling::Scaled;
    let iterations = super::esi_snap::scale_mesh_loop(mesh, &mut ms, &pa, me, topo, &ctl, n_relax_iter);
    MoveReport { displacement: rep, scale_iterations: iterations }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube of 2x2x2 hexes is not needed: a chain of four points along x is enough to
    /// check the wave, the patch-normal smoothing and `minSmoothField` mechanics.
    fn chain() -> (Vec<V>, MeshEdges) {
        let pts: Vec<V> = (0..4).map(|i| [i as f64, 0.0, 0.0]).collect();
        let edges = vec![[0, 1], [1, 2], [2, 3]];
        let mut pe = vec![Vec::new(); 4];
        for (i, e) in edges.iter().enumerate() {
            pe[e[0]].push(i);
            pe[e[1]].push(i);
        }
        (pts, MeshEdges { edges, point_edges: pe })
    }

    #[test]
    fn wave_carries_data_from_the_nearest_seed() {
        let (pts, me) = chain();
        let seeds = vec![
            (0, WaveInfo { origin: Some(pts[0]), dist_sqr: 0.0, data: [1.0, 0.0, 0.0] }),
            (3, WaveInfo { origin: Some(pts[3]), dist_sqr: 0.0, data: [2.0, 0.0, 0.0] }),
        ];
        let (info, iters) = point_edge_wave(&pts, &me, &seeds, 10);
        assert!(iters >= 1);
        assert_eq!(info[1].data[0], 1.0);
        assert_eq!(info[2].data[0], 2.0);
        assert_eq!(info[1].dist_sqr, 1.0);
        assert!(info.iter().all(|i| i.valid()));
        // the iteration limit stops the walk
        let (limited, _) = point_edge_wave(&pts, &me, &seeds[..1], 1);
        assert!(limited[1].valid() && !limited[3].valid());
    }

    #[test]
    fn edge_weights_and_scalar_smoothing_respect_the_minimum() {
        use super::super::esi_merge::MergeMesh;
        // two quads in a strip as the patch; thickness spike at point 4 is smoothed down but
        // never below the minimum field
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ];
        let faces = vec![vec![0, 3, 4, 1], vec![1, 4, 5, 2]];
        let mesh = SnapMesh { points: points.clone(), faces, owner: vec![0, 1], neighbour: vec![] };
        let m = MergeMesh::from_patch_starts(mesh, &[0, 2]);
        let pp = LayerPatch::new(&m, &[0]);
        let (w, inv) = calculate_edge_weights(&points, &pp.mesh_points, &pp.edges);
        assert_eq!(w.len(), pp.edges.len());
        assert!(inv.iter().all(|&x| x > 0.0 && x < 1.0));
        let mut field = vec![1.0; 6];
        field[4] = 3.0;
        let min = vec![0.9; 6];
        min_smooth_field(&points, &pp, 5, &min, &mut field);
        assert!(field[4] < 3.0 && field[4] >= 0.9);
        assert!(field.iter().all(|&f| f <= 3.0 && f >= 0.9));
        // the strip faces wind clockwise seen from +z: normals are -z and stay so
        let mut n = patch_point_normals(&pp);
        assert!(n.iter().all(|v| (v[2] + 1.0).abs() < 1e-12));
        let r = smooth_patch_normals(&points, &pp, 1, &mut n);
        assert!(r.abs() < 1e-12);
        assert!(vis_normal(n[0], &pp.face_normals, &pp.point_faces[0]));
    }

    #[test]
    fn normalised_zero_vector_is_zero() {
        assert_eq!(normalised([0.0; 3]), [0.0; 3]);
        assert_eq!(normalised([0.0, 2.0, 0.0]), [0.0, 1.0, 0.0]);
    }
}
