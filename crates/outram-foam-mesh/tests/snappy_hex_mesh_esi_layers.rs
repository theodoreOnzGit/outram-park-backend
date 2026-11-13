// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1467 round 8: `addLayers` of OpenFOAM.com ESI v2506 `snappyHexMesh` on the flange,
//! stage by stage, from ESI's own snapped mesh (time 81 of `snappyHexMeshDict.layers`).
//!
//! Methodology, predictions P49-P51 (committed before the port code) and the results:
//! `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`, "Round 8". Reference
//! data: `layer_points_81.bin.gz` (exact doubles), `esi_feat_final_faces.txt.gz` (time-81
//! faces), `layer_mesh_83.txt.gz` (faces after the pre-layer merges), `layer_cell_level_81`,
//! and the numbers of the ESI log.

use outram_foam_mesh::check_mesh::EsiQualityControls;
use outram_foam_mesh::snappy_hex_mesh::esi_layers::{
    calculate_layer_thickness, set_point_num_layers, ExtrudeMode, LayerDict, LayerNumbers, LayerParameters, LayerPatch,
    PatchLayerDict, ThicknessModel,
};
use outram_foam_mesh::snappy_hex_mesh::esi_medial::{get_patch_displacement, move_mesh, DisplacementParams, MedialAxis, MedialParams};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{mesh_edges, scale_topology, PatchPointScaling, ScaleControls};
use outram_foam_mesh::snappy_hex_mesh::esi_merge::{merge_edges_undo, merge_patch_faces_undo, MergeMesh};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{SnapMesh, V};
use petir::mathf::RealMath;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b, "not gzip");
    assert_eq!(bytes[3], 0, "gzip -n expected");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn fixture(name: &str) -> Vec<u8> {
    gunzip(&std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture"))
}

fn vectors(name: &str) -> Vec<V> {
    let raw = fixture(name);
    assert_eq!(raw.len() % 24, 0);
    raw.chunks_exact(24)
        .map(|c| {
            let f = |i: usize| f64::from_le_bytes(c[8 * i..8 * i + 8].try_into().unwrap());
            [f(0), f(1), f(2)]
        })
        .collect()
}

fn lines(name: &str) -> Vec<Vec<i64>> {
    String::from_utf8(fixture(name))
        .unwrap()
        .lines()
        .map(|l| l.split_whitespace().map(|x| x.parse().unwrap()).collect())
        .collect()
}

/// ESI's snapped mesh (time 81): the four flange patches (ESI's empty `allBoundary` is not
/// modelled, so patch `k` here is `flange_patch{k+1}`).
fn mesh81() -> MergeMesh {
    let mut faces = Vec::new();
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for v in lines("esi_feat_final_faces.txt.gz") {
        owner.push(v[0] as usize);
        if v[1] >= 0 {
            neighbour.push(v[1] as usize);
        }
        faces.push(v[2..].iter().map(|&p| p as usize).collect());
    }
    let mesh = SnapMesh { points: vectors("layer_points_81.bin.gz"), faces, owner, neighbour };
    MergeMesh::from_patch_starts(mesh, &[51058, 58275, 59449, 59689, 60813])
}

fn feature_cos() -> f64 {
    (30.0 * std::f64::consts::PI / 180.0).r_cos()
}

/// `doLayers`: `mergePatchFacesUndo` then `mergeEdgesUndo` (same `cos 30 deg`), on the snapped mesh.
fn mesh83() -> (MergeMesh, usize, usize) {
    let m = mesh81();
    let controls = EsiQualityControls::default();
    let preserve = vec![false; m.mesh.faces.len()];
    let patches: Vec<usize> = (0..4).collect();
    let min_cos = feature_cos();
    // `layerParameters::concaveAngle` defaults to 90 degrees (`concaveCos = cos(degToRad(90))`);
    // the snap driver used 30 degrees, which is why this merge finds sets the snap's did not.
    let concave_cos = (90.0f64 * std::f64::consts::PI / 180.0).r_cos();
    let (m1, st1) = merge_patch_faces_undo(&m, min_cos, concave_cos, &patches, &controls, &preserve, false).expect("no undo");
    assert_eq!(st1.error_faces_per_iteration, vec![0]);
    let (m2, st2) = merge_edges_undo(&m1, min_cos, &controls).expect("no undo");
    assert_eq!(st2.error_faces_per_iteration, vec![0]);
    (m2, st1.face_sets.len(), st2.n_points_removed)
}

/// P49: the merges before the layers equal ESI's time 83 per face, point and numbering.
#[test]
fn pre_layer_merge_against_esi() {
    let (m, n_sets, n_points_removed) = mesh83();
    eprintln!("P49: {n_sets} face sets, {n_points_removed} straight-edge points removed, {} faces, {} points", m.mesh.faces.len(), m.mesh.points.len());
    assert_eq!(n_sets, 6, "ESI: Merging 6 sets of faces");
    assert_eq!(n_points_removed, 4, "ESI: Removing 4 straight edge points");
    assert_eq!((m.mesh.faces.len(), m.mesh.points.len()), (60805, 24305));
    // surviving points are time 81's minus the six removed (the face step removes 2 unused ones)
    let removed: Vec<usize> = lines("layer_83_removed81.txt.gz").iter().map(|v| v[0] as usize).collect();
    let m81 = mesh81();
    let kept: Vec<usize> = (0..m81.mesh.points.len()).filter(|p| !removed.contains(p)).collect();
    assert_eq!(kept.len(), 24305);
    for (i, &p81) in kept.iter().enumerate() {
        assert_eq!(m.mesh.points[i], m81.mesh.points[p81], "point {i} coordinates bit-identical");
    }
    let esi = lines("layer_mesh_83.txt.gz");
    assert_eq!(esi.len(), 60805);
    let (mut ring, mut own, mut nbr, mut pat) = (0, 0, 0, 0);
    let n_int = m.mesh.neighbour.len();
    for f in 0..esi.len() {
        let want: Vec<usize> = esi[f][2..].iter().map(|&p| p as usize).collect();
        ring += (m.mesh.faces[f] == want) as usize;
        own += (m.mesh.owner[f] == esi[f][0] as usize) as usize;
        if f < n_int {
            nbr += (m.mesh.neighbour[f] as i64 == esi[f][1]) as usize;
        } else {
            // ESI's patch 0 is the empty allBoundary
            pat += (m.face_patch[f] as i64 + 1 == -1 - esi[f][1]) as usize;
        }
    }
    eprintln!("P49: ring {ring}, owner {own}, neighbour {nbr} of {n_int}, patch {pat} of {}", esi.len() - n_int);
    assert_eq!(ring, 60805);
    assert_eq!(own, 60805);
    assert_eq!(nbr, n_int);
    assert_eq!(pat, esi.len() - n_int);
}

fn layer_dict() -> LayerDict {
    LayerDict {
        relative_sizes: true,
        numbers: LayerNumbers { first: -123.0, final_: 0.3, total: -123.0, expansion: 1.0 },
        model: ThicknessModel::FinalAndExpansion,
        min_thickness: 0.25,
        feature_angle: 30.0,
        n_grow: 0,
        max_face_thickness_ratio: 0.5,
        n_buffer_cells_no_extrude: 0,
        n_layer_iter: 50,
        n_relaxed_iter: 20,
        n_outer_iter: 1,
    }
}

/// `level0Edge` of the ESI run (`81/polyMesh/level0Edge`).
const EDGE0_LEN: f64 = 0.0029999999999999784;

/// P50, P51: layer parameters, the extrusion set-up and the thickness table.
#[test]
fn layer_parameters_and_point_set_up_against_esi() {
    let (m, _, _) = mesh83();
    let cell_level: Vec<usize> = lines("layer_cell_level_81.txt.gz").iter().map(|v| v[0] as usize).collect();
    let patch_ids: Vec<usize> = (0..4).collect();
    let overrides: Vec<(usize, PatchLayerDict)> =
        patch_ids.iter().map(|&p| (p, PatchLayerDict { n_surface_layers: 1, ..Default::default() })).collect();
    let params = LayerParameters::new(&layer_dict(), 4, &overrides);
    assert_eq!(params.layer_models, vec![ThicknessModel::FinalAndExpansion; 4]);
    assert_eq!(params.num_layers, vec![1; 4]);
    let pp = LayerPatch::new(&m, &patch_ids);
    assert_eq!(pp.addressing.len(), 9747, "ESI: 9747 faces");
    let (st, rep) = set_point_num_layers(&m, &params, &patch_ids, &pp, feature_cos(), EDGE0_LEN, &cell_level);
    eprintln!("pp points {}, report {rep:?}, extruding {}", pp.n_points(), st.n_extruding());
    assert_eq!(rep.n_feature_edges, 1269, "ESI: Set displacement to zero for points on 1269 feature edges");
    assert_eq!(rep.non_manifold.n_non_manifold_points, 0);
    assert_eq!(rep.n_warped, Some(0), "ESI: 0 warped faces");
    assert_eq!(st.n_extruding(), 8521, "ESI: Layers to add in current iteration : 8521");
    assert_eq!(st.n_layers.iter().sum::<i64>(), 8521);
    assert!(st.status.iter().all(|&s| s != ExtrudeMode::ExtrudeRemove));
    let lt = calculate_layer_thickness(&m, &pp, &patch_ids, &params, &cell_level, &st.n_layers, EDGE0_LEN);
    // ESI's table, `setprecision(3)`:
    let want = [(7209usize, 0.000225, 0.000189), (1174, 0.000225, 0.000172), (240, 0.000225, 0.000184), (1124, 0.000225, 0.000198)];
    for (row, (nf, nw, ov)) in lt.rows.iter().zip(want) {
        eprintln!("{} faces {} near-wall {:.2e} overall {:.2e}", row.patch, row.n_faces, row.avg_near_wall, row.avg_thickness);
        assert_eq!(row.n_faces, nf);
        assert_eq!(format!("{:.2e}", row.avg_near_wall), format!("{nw:.2e}"));
        assert_eq!(format!("{:.2e}", row.avg_thickness), format!("{ov:.2e}"));
    }
}

fn rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

/// P52-P54: `medialAxisMeshMover` for the first layer iteration against ESI's time 85.
#[test]
fn medial_axis_first_iteration_against_esi() {
    let (m, _, _) = mesh83();
    let cell_level: Vec<usize> = lines("layer_cell_level_81.txt.gz").iter().map(|v| v[0] as usize).collect();
    let patch_ids: Vec<usize> = (0..4).collect();
    let overrides: Vec<(usize, PatchLayerDict)> =
        patch_ids.iter().map(|&p| (p, PatchLayerDict { n_surface_layers: 1, ..Default::default() })).collect();
    let params = LayerParameters::new(&layer_dict(), 4, &overrides);
    let pp = LayerPatch::new(&m, &patch_ids);
    let (mut st, _) = set_point_num_layers(&m, &params, &patch_ids, &pp, feature_cos(), EDGE0_LEN, &cell_level);
    let lt = calculate_layer_thickness(&m, &pp, &patch_ids, &params, &cell_level, &st.n_layers, EDGE0_LEN);
    let counts = get_patch_displacement(&pp, &m.mesh.points, &lt.thickness, &lt.min_thickness, &mut st);
    eprintln!("getPatchDisplacement: {counts:?}");
    assert_eq!((counts.n_no_vis_normal, counts.n_extrude_remove), (0, 0));
    let patch_disp: Vec<V> = st.disp.iter().map(|d| [-d[0], -d[1], -d[2]]).collect();

    let me = mesh_edges(&m.mesh);
    let n_points = m.mesh.points.len();
    let mp = MedialParams {
        n_smooth_surface_normals: 1,
        min_medial_axis_angle_cos: rad(90.0).r_cos(),
        n_smooth_normals: 3,
        n_medial_axis_iter: n_points,
        disable_wall_edges: false,
    };
    let medial = MedialAxis::update(&m.mesh.points, &me, &pp, &mp);
    eprintln!(
        "P52: patch-normal residual {:?} (ESI 0.07867287997577262), interior {:?} (ESI 0.1764300260321269)",
        medial.patch_normal_residual, medial.interior_normal_residual
    );
    let dp = DisplacementParams {
        max_thickness_to_medial_ratio: 0.3,
        min_cos_layer_termination: rad(0.5 * 30.0).r_cos(),
        n_smooth_thickness: 10,
        n_medial_axis_iter: n_points,
        detect_extrusion_island: false,
        n_smooth_displacement: 0,
    };
    let mut mesh = m.mesh.clone();
    let topo = scale_topology(&mesh);
    let ctl = ScaleControls {
        error_reduction: 0.75,
        n_smooth_scale: 4,
        quality: EsiQualityControls::default(),
        n_allowable_errors: 0,
        patch_scaling: PatchPointScaling::Scaled,
    };
    let rep = move_mesh(&mut mesh, &me, &topo, &pp, &medial, &lt.min_thickness, &dp, &patch_disp, &ctl, 5);
    eprintln!(
        "reduced {} nodes (ESI 20), isolated {} (ESI 975), thickness residual {:?} (ESI 9.162494450008907e-06)",
        rep.displacement.n_thickness_reduced, rep.displacement.n_isolated, rep.displacement.thickness_residual
    );
    let schedule: Vec<(f64, usize)> =
        rep.scale_iterations.iter().map(|r| (r.scale_min, r.check.wrong_faces.len())).collect();
    eprintln!("scaling (min, faces in error): {schedule:?}");
    let esi = vectors("layer_points_85.bin.gz");
    assert_eq!(esi.len(), mesh.points.len());
    let mut same = 0usize;
    let mut worst = 0.0f64;
    let mut n_diff_points = 0usize;
    for (a, b) in mesh.points.iter().zip(&esi) {
        let mut d = false;
        for c in 0..3 {
            if a[c].to_bits() == b[c].to_bits() {
                same += 1;
            } else {
                d = true;
                worst = worst.max((a[c] - b[c]).abs());
            }
        }
        n_diff_points += d as usize;
    }
    eprintln!("P54: components bit-identical {same} of {}, points differing {n_diff_points}, worst {worst:e}", 3 * esi.len());
    assert_eq!(rep.displacement.n_thickness_reduced, 20, "P53");
    assert_eq!(rep.displacement.n_isolated, 975, "P53");
    let want = [(1.0, 8), (0.75, 8), (0.5625, 8), (0.421875, 6), (0.31640625, 6), (0.2373046875, 4), (0.0, 0)];
    assert_eq!(schedule, want.to_vec(), "P53");
    assert_eq!(n_diff_points, 0, "P54");
}

fn raw_f64(path: &str) -> Vec<f64> {
    let b = std::fs::read(path).expect("dump");
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

fn cmp_f(name: &str, a: &[f64], b: &[f64]) {
    let n = a.len().min(b.len());
    let diff = (0..n).filter(|&i| a[i].to_bits() != b[i].to_bits()).count();
    let worst = (0..n).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max);
    eprintln!("STAGE {name}: {} values, differing {diff}, worst {worst:e}", n);
}

/// Diagnostic against the ESI dump of `medialAxisMeshMover` (set `LAYER_DUMP_DIR`).
#[test]
fn medial_stages_against_esi_dump() {
    let Ok(dir) = std::env::var("LAYER_DUMP_DIR") else { return };
    let (m, _, _) = mesh83();
    let cell_level: Vec<usize> = lines("layer_cell_level_81.txt.gz").iter().map(|v| v[0] as usize).collect();
    let patch_ids: Vec<usize> = (0..4).collect();
    let overrides: Vec<(usize, PatchLayerDict)> =
        patch_ids.iter().map(|&p| (p, PatchLayerDict { n_surface_layers: 1, ..Default::default() })).collect();
    let params = LayerParameters::new(&layer_dict(), 4, &overrides);
    let pp = LayerPatch::new(&m, &patch_ids);
    let (mut st, _) = set_point_num_layers(&m, &params, &patch_ids, &pp, feature_cos(), EDGE0_LEN, &cell_level);
    let lt = calculate_layer_thickness(&m, &pp, &patch_ids, &params, &cell_level, &st.n_layers, EDGE0_LEN);
    get_patch_displacement(&pp, &m.mesh.points, &lt.thickness, &lt.min_thickness, &mut st);
    let me = mesh_edges(&m.mesh);
    let n_points = m.mesh.points.len();
    let mp = MedialParams {
        n_smooth_surface_normals: 1,
        min_medial_axis_angle_cos: rad(90.0).r_cos(),
        n_smooth_normals: 3,
        n_medial_axis_iter: n_points,
        disable_wall_edges: false,
    };
    let medial = MedialAxis::update(&m.mesh.points, &me, &pp, &mp);
    let flat = |v: &[V]| -> Vec<f64> { v.iter().flat_map(|x| x.iter().copied()).collect() };
    cmp_f("medialRatio", &medial.medial_ratio, &raw_f64(&format!("{dir}/dump_medialRatio.bin")));
    cmp_f("medialDist", &medial.medial_dist, &raw_f64(&format!("{dir}/dump_medialDist.bin")));
    cmp_f("medialVec", &flat(&medial.medial_vec), &raw_f64(&format!("{dir}/dump_medialVec.bin")));
    cmp_f("dispVec", &flat(&medial.disp_vec), &raw_f64(&format!("{dir}/dump_dispVec.bin")));
    let dp = DisplacementParams {
        max_thickness_to_medial_ratio: 0.3,
        min_cos_layer_termination: rad(0.5 * 30.0).r_cos(),
        n_smooth_thickness: 10,
        n_medial_axis_iter: n_points,
        detect_extrusion_island: false,
        n_smooth_displacement: 0,
    };
    let mut st2 = outram_foam_mesh::snappy_hex_mesh::esi_layers::ExtrudeState::new(pp.n_points());
    st2.disp = st.disp.iter().map(|d| [-d[0], -d[1], -d[2]]).collect();
    for i in 0..pp.n_points() {
        let mg = (st2.disp[i][0].powi(2) + st2.disp[i][1].powi(2) + st2.disp[i][2].powi(2)).sqrt();
        if mg <= lt.min_thickness[i] + 1e-15 {
            st2.status[i] = ExtrudeMode::NoExtrude;
        }
    }
    let (disp, rep) = outram_foam_mesh::snappy_hex_mesh::esi_medial::calculate_displacement(
        &m.mesh.points,
        &me,
        &pp,
        &medial,
        &lt.min_thickness,
        &dp,
        &mut st2,
    );
    let n_pp = pp.n_points();
    cmp_f("patchDispAfterIsolated", &flat(&st2.disp), &raw_f64(&format!("{dir}/dump_patchDispAfterIsolated.bin"))[..3 * n_pp]);
    cmp_f("thicknessSmoothed", &rep.thickness, &raw_f64(&format!("{dir}/dump_thicknessSmoothed.bin"))[..n_pp]);
    cmp_f("displacement", &flat(&disp), &raw_f64(&format!("{dir}/dump_displacement.bin"))[..3 * n_points]);
}

// ---------------------------------------------------------------------------------
// Round 9: the layer loop (truncateDisplacement, addPatchCellLayer, checkAndUnmark)
// ---------------------------------------------------------------------------------

use outram_foam_mesh::snappy_hex_mesh::esi_layer_loop::{add_layers_loop, LayerIteration, LayerLoopInput};
use outram_foam_mesh::snappy_hex_mesh::esi_layers::{ExtrudeState, LayerThickness};

fn loop_input<'a>(
    m: &'a MergeMesh,
    pp: &'a LayerPatch,
    medial: &'a MedialAxis,
    lt: &'a LayerThickness,
    st: ExtrudeState,
    n_layer_iter: usize,
) -> LayerLoopInput<'a> {
    let n_points = m.mesh.points.len();
    LayerLoopInput {
        m,
        pp,
        medial,
        disp_params: DisplacementParams {
            max_thickness_to_medial_ratio: 0.3,
            min_cos_layer_termination: rad(0.5 * 30.0).r_cos(),
            n_smooth_thickness: 10,
            n_medial_axis_iter: n_points,
            detect_extrusion_island: false,
            n_smooth_displacement: 0,
        },
        thickness: &lt.thickness,
        min_thickness: &lt.min_thickness,
        expansion_ratio: &lt.expansion_ratio,
        state: st,
        quality: EsiQualityControls::default(),
        relaxed_quality: EsiQualityControls::default(),
        n_layer_iter,
        n_relaxed_iter: 20,
        n_grow: 0,
        n_buffer_cells_no_extrude: 0,
        n_relax_iter: 5,
        n_allowable_errors: 0,
    }
}

/// Compare a mesh with ESI's `layer_mesh_TIME`/`layer_points_TIME`: counts of (points
/// bit-identical, ring, owner, neighbour, patch) and the first differing face.
fn compare_layer_mesh(m: &MergeMesh, time: &str) -> [usize; 5] {
    let esi = lines(&format!("layer_mesh_{time}.txt.gz"));
    let pts = vectors(&format!("layer_points_{time}.bin.gz"));
    eprintln!(
        "mesh {time}: port {} points {} faces, ESI {} points {} faces",
        m.mesh.points.len(),
        m.mesh.faces.len(),
        pts.len(),
        esi.len()
    );
    let mut c = [0usize; 5];
    let n_int = m.mesh.neighbour.len();
    let mut first_bad: Option<usize> = None;
    for (a, b) in m.mesh.points.iter().zip(&pts) {
        c[0] += (a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())) as usize;
    }
    for f in 0..esi.len().min(m.mesh.faces.len()) {
        let want: Vec<usize> = esi[f][2..].iter().map(|&p| p as usize).collect();
        let ok_ring = m.mesh.faces[f] == want;
        let ok_own = m.mesh.owner[f] == esi[f][0] as usize;
        let ok_nbr = if f < n_int {
            m.mesh.neighbour[f] as i64 == esi[f][1]
        } else {
            m.face_patch[f] as i64 + 1 == -1 - esi[f][1]
        };
        c[1] += ok_ring as usize;
        c[2] += ok_own as usize;
        if f < n_int {
            c[3] += ok_nbr as usize;
        } else {
            c[4] += ok_nbr as usize;
        }
        if !(ok_ring && ok_own && ok_nbr) && first_bad.is_none() {
            first_bad = Some(f);
        }
    }
    eprintln!(
        "mesh {time}: points {} / {}, ring {}, owner {}, neighbour {} (of {n_int}), patch {} of {}",
        c[0],
        pts.len(),
        c[1],
        c[2],
        c[3],
        c[4],
        esi.len() - n_int
    );
    if let Some(f) = first_bad {
        eprintln!(
            "  first differing face {f}: port ring {:?} own {} | ESI {:?} own {} nbr/patch {}",
            m.mesh.faces[f],
            m.mesh.owner[f],
            &esi[f][2..],
            esi[f][0],
            esi[f][1]
        );
    }
    c
}

/// P55-P59: the layer loop against ESI times 85-91.
#[test]
fn layer_loop_against_esi() {
    let (m, _, _) = mesh83();
    let cell_level: Vec<usize> = lines("layer_cell_level_81.txt.gz").iter().map(|v| v[0] as usize).collect();
    let patch_ids: Vec<usize> = (0..4).collect();
    let overrides: Vec<(usize, PatchLayerDict)> =
        patch_ids.iter().map(|&p| (p, PatchLayerDict { n_surface_layers: 1, ..Default::default() })).collect();
    let params = LayerParameters::new(&layer_dict(), 4, &overrides);
    let pp = LayerPatch::new(&m, &patch_ids);
    let (st, _) = set_point_num_layers(&m, &params, &patch_ids, &pp, feature_cos(), EDGE0_LEN, &cell_level);
    let lt = calculate_layer_thickness(&m, &pp, &patch_ids, &params, &cell_level, &st.n_layers, EDGE0_LEN);
    let me = mesh_edges(&m.mesh);
    let n_points = m.mesh.points.len();
    let mp = MedialParams {
        n_smooth_surface_normals: 1,
        min_medial_axis_angle_cos: rad(90.0).r_cos(),
        n_smooth_normals: 3,
        n_medial_axis_iter: n_points,
        disable_wall_edges: false,
    };
    let medial = MedialAxis::update(&m.mesh.points, &me, &pp, &mp);
    // nLayerIter 50: the loop ends by itself when `checkAndUnmark` removes nothing
    let its: Vec<LayerIteration> = add_layers_loop(&loop_input(&m, &pp, &medial, &lt, st, 50));
    assert_eq!(its.len(), 3, "ESI: three layer iterations");
    let shrunk_ids = ["85", "87", "89"];
    let layer_ids = ["86", "88", "90"];
    // ESI log: faces removed by checkAndUnmark, extruded faces, added cells, truncateDisplacement passes
    let want_changed = [17, 2, 0];
    let want_extruded = [8724, 8658, 8642];
    let want_added = [8743, 8660, 8642];
    let want_passes: [Vec<(usize, usize)>; 3] = [vec![(0, 0)], vec![(2, 0), (0, 0)], vec![(0, 0)]];
    for (k, it) in its.iter().enumerate() {
        eprintln!(
            "iteration {k}: truncate passes {:?}, faces with layers {}, points with layers {}, added cells {}, check {:?}, changed {}, extruded {}",
            it.truncate.passes,
            it.n_face_layers_positive,
            it.n_point_layers_positive,
            it.n_added_cells,
            it.check.new_faces,
            it.n_changed,
            it.n_extruded_faces
        );
        assert_eq!(it.truncate.passes, want_passes[k], "P55/P58 truncateDisplacement log");
        assert_eq!(it.n_changed, want_changed[k], "P57/P58");
        assert_eq!(it.n_extruded_faces, want_extruded[k], "P57/P58");
        assert_eq!(it.n_added_cells, want_added[k], "P55/P58 'Added N out of 9747 cells'");
        let esi = vectors(&format!("layer_points_{}.bin.gz", shrunk_ids[k]));
        assert_eq!(esi.len(), it.shrunk.points.len());
        for (i, (a, b)) in it.shrunk.points.iter().zip(&esi).enumerate() {
            assert!(a.iter().zip(b.iter()).all(|(x, y)| x.to_bits() == y.to_bits()), "shrunk mesh {} point {i}", shrunk_ids[k]);
        }
        let c = compare_layer_mesh(&it.layers.mesh, layer_ids[k]);
        let n = it.layers.mesh.mesh.faces.len();
        let n_int = it.layers.mesh.mesh.neighbour.len();
        assert_eq!(c, [it.layers.mesh.mesh.points.len(), n, n, n_int, n - n_int], "layer mesh {} equals ESI per index", layer_ids[k]);
    }
    assert_eq!(its[0].n_face_layers_positive, 8743, "P55");
    assert_eq!(its[0].n_point_layers_positive, 7437, "P55: 31742 - 24305 added points");
    assert_eq!(its[0].check.new_faces[0].1, 2, "P57 non-orthogonality");
    assert_eq!(its[0].check.new_faces[2].1, 15, "P57 face tets");
    // the final mesh (time 91): the faces of time 90 with ESI's empty allBoundary patch dropped
    let last = &its[2];
    let esi91 = lines("layer_mesh_91.txt.gz");
    assert_eq!(esi91.len(), last.layers.mesh.mesh.faces.len());
    let n_int = last.layers.mesh.mesh.neighbour.len();
    for (f, e) in esi91.iter().enumerate() {
        let want: Vec<usize> = e[2..].iter().map(|&p| p as usize).collect();
        assert_eq!(last.layers.mesh.mesh.faces[f], want, "final face {f} ring");
        assert_eq!(last.layers.mesh.mesh.owner[f], e[0] as usize);
        if f < n_int {
            assert_eq!(last.layers.mesh.mesh.neighbour[f] as i64, e[1]);
        } else {
            assert_eq!(last.layers.mesh.face_patch[f] as i64, -1 - e[1], "final face {f} patch");
        }
    }
    let p91 = vectors("layer_points_90.bin.gz");
    assert!(last.layers.mesh.mesh.points.iter().zip(&p91).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())));
    // printLayerData: faces, average layers, thickness [m], thickness [%] to ESI's 3 figures
    let rows = outram_foam_mesh::snappy_hex_mesh::esi_layer_loop::layer_data_table(last, &pp, &lt.thickness, &patch_ids, &[1, 1, 1, 1]);
    let esi_table: [(usize, &str, &str, &str); 4] = [
        (7209, "0.874", "0.000155", "78.8"),
        (1174, "0.983", "0.000178", "92.2"),
        (240, "0.933", "0.000162", "82.3"),
        (1124, "0.859", "0.000173", "81.7"),
    ];
    let sig3 = |x: f64| format!("{:.2e}", x);
    let sig3s = |s: &str| format!("{:.2e}", s.parse::<f64>().unwrap());
    for (r, e) in rows.iter().zip(&esi_table) {
        assert_eq!(r.n_faces, e.0);
        assert_eq!(sig3(r.avg_layers), sig3s(e.1), "patch {} layers", r.patch);
        assert_eq!(sig3(r.avg_real), sig3s(e.2), "patch {} thickness [m]", r.patch);
        assert_eq!(sig3(r.avg_fraction_percent), sig3s(e.3), "patch {} thickness [%] ({})", r.patch, r.avg_fraction_percent);
    }
}
