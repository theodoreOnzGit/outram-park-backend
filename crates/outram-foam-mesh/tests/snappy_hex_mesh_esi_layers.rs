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
const EDGE0_LEN: f64 = 0.002999999999999978;

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
