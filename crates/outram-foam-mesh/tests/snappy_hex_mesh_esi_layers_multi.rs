// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1467 round 10: the `addLayers` loop of OpenFOAM.com ESI v2506 `snappyHexMesh` on the flange
//! for three cases the one-layer test (`snappy_hex_mesh_esi_layers.rs`) does not cover:
//!
//! * `base`: the round-9 case (`snappyHexMeshDict.layers`) from the REBUILT ESI's time-81 points
//!   (`layer_points_81_r10.bin.gz`; the old points differ by 1 ulp in 16613 components);
//! * `layers3`: `nSurfaceLayers 3`, `expansionRatio 1.2` (`snappyHexMeshDict.layers3`): the `offset`
//!   branches of `getEdgeString` / `addSideFace` for faces next to points with fewer layers;
//! * `relaxed`: `nRelaxedIter 1` (`snappyHexMeshDict.layers_relaxed`): iterations from 1 use
//!   `meshQualityControls.relaxed`.
//!
//! References are digests (FNV-1a 64, `digest_layer_mesh.py`) of ESI's layer and shrunk meshes
//! (faces as `owner neighbour ring` text lines, points as little-endian doubles), so a match is
//! equality of every face ring, owner, neighbour, patch and every point bit. Set `LAYER_DUMP_DIR`
//! to write the port's meshes in the extractor's format for a diff against a fresh ESI run.
//! Methodology, predictions P60-P65 and results: `verification_and_validation/
//! snappy_hex_mesh_esi_v2506/README.md`, "Round 10".

use outram_foam_mesh::check_mesh::EsiQualityControls;
use outram_foam_mesh::snappy_hex_mesh::esi_layer_loop::{add_layers_loop, layer_data_table, LayerIteration, LayerLoopInput};
use outram_foam_mesh::snappy_hex_mesh::esi_layers::{
    calculate_layer_thickness, set_point_num_layers, ExtrudeState, LayerDict, LayerNumbers, LayerParameters, LayerPatch,
    LayerThickness, PatchLayerDict, ThicknessModel,
};
use outram_foam_mesh::snappy_hex_mesh::esi_medial::{DisplacementParams, MedialAxis, MedialParams};
use outram_foam_mesh::snappy_hex_mesh::esi_merge::{merge_edges_undo, merge_patch_faces_undo, MergeMesh};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{mesh_edges, SnapMesh, V};
use petir::mathf::RealMath;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");

/// `level0Edge` of the ESI run (`81/polyMesh/level0Edge`), identical in the rebuilt ESI.
const EDGE0_LEN: f64 = 0.0029999999999999784;

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

fn rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

/// FNV-1a, 64 bit (`digest_layer_mesh.py`).
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The bytes of ESI's `layer_points_TIME.bin`.
fn point_bytes(points: &[V]) -> Vec<u8> {
    let mut out = Vec::with_capacity(points.len() * 24);
    for p in points {
        for c in p {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    out
}

/// The bytes of ESI's `layer_mesh_TIME.txt`: `owner neighbour ring` per face, a boundary
/// face's neighbour being `-1 - k` for the k-th boundary patch of the written mesh.
/// `patch_offset` is 1 while ESI's empty `allBoundary` is still patch 0, 0 after it is removed.
fn face_bytes(m: &MergeMesh, patch_offset: i64) -> Vec<u8> {
    let n_int = m.mesh.neighbour.len();
    let mut s = String::new();
    for f in 0..m.mesh.faces.len() {
        let nbr = if f < n_int { m.mesh.neighbour[f] as i64 } else { -1 - (m.face_patch[f] as i64 + patch_offset) };
        s.push_str(&format!("{} {}", m.mesh.owner[f], nbr));
        for p in &m.mesh.faces[f] {
            s.push_str(&format!(" {p}"));
        }
        s.push('\n');
    }
    s.into_bytes()
}

/// Write the port's mesh in the extractor's format when `LAYER_DUMP_DIR` is set.
fn dump(tag: &str, time: &str, faces: Option<&[u8]>, points: &[u8]) {
    if let Ok(dir) = std::env::var("LAYER_DUMP_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(f) = faces {
            std::fs::write(format!("{dir}/{tag}_layer_mesh_{time}.txt"), f).unwrap();
        }
        std::fs::write(format!("{dir}/{tag}_layer_points_{time}.bin"), points).unwrap();
    }
}

/// ESI's snapped mesh of the rebuilt environment (time 81): faces as in round 9 (unchanged),
/// points from `layer_points_81_r10.bin.gz`.
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
    let mesh = SnapMesh { points: vectors("layer_points_81_r10.bin.gz"), faces, owner, neighbour };
    MergeMesh::from_patch_starts(mesh, &[51058, 58275, 59449, 59689, 60813])
}

fn feature_cos() -> f64 {
    rad(30.0).r_cos()
}

/// `doLayers`: `mergePatchFacesUndo` (concave limit 90 deg) then `mergeEdgesUndo`.
fn mesh83() -> MergeMesh {
    let m = mesh81();
    let controls = EsiQualityControls::default();
    let preserve = vec![false; m.mesh.faces.len()];
    let patches: Vec<usize> = (0..4).collect();
    let (m1, _) =
        merge_patch_faces_undo(&m, feature_cos(), rad(90.0).r_cos(), &patches, &controls, &preserve, false).expect("no undo");
    let (m2, _) = merge_edges_undo(&m1, feature_cos(), &controls).expect("no undo");
    m2
}

/// What differs between the three cases.
struct Case {
    tag: &'static str,
    n_layers: i64,
    expansion: f64,
    n_relaxed_iter: usize,
}

struct Run {
    its: Vec<LayerIteration>,
    pp: LayerPatch,
    lt: LayerThickness,
    n_layers_total: i64,
}

fn run_case(c: &Case) -> Run {
    let m = mesh83();
    let cell_level: Vec<usize> = lines("layer_cell_level_81.txt.gz").iter().map(|v| v[0] as usize).collect();
    let patch_ids: Vec<usize> = (0..4).collect();
    let dict = LayerDict {
        relative_sizes: true,
        numbers: LayerNumbers { first: -123.0, final_: 0.3, total: -123.0, expansion: c.expansion },
        model: ThicknessModel::FinalAndExpansion,
        min_thickness: 0.25,
        feature_angle: 30.0,
        n_grow: 0,
        max_face_thickness_ratio: 0.5,
        n_buffer_cells_no_extrude: 0,
        n_layer_iter: 50,
        n_relaxed_iter: c.n_relaxed_iter,
        n_outer_iter: 1,
    };
    let overrides: Vec<(usize, PatchLayerDict)> =
        patch_ids.iter().map(|&p| (p, PatchLayerDict { n_surface_layers: c.n_layers, ..Default::default() })).collect();
    let params = LayerParameters::new(&dict, 4, &overrides);
    let pp = LayerPatch::new(&m, &patch_ids);
    let (st, _) = set_point_num_layers(&m, &params, &patch_ids, &pp, feature_cos(), EDGE0_LEN, &cell_level);
    let n_layers_total: i64 = st.n_layers.iter().sum();
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
    // `meshQualityControls.relaxed { maxNonOrtho 75; }` merged over the base keys
    let relaxed = EsiQualityControls { max_non_ortho: 75.0, ..EsiQualityControls::default() };
    let inp = loop_input(&m, &pp, &medial, &lt, st, c.n_relaxed_iter, relaxed);
    let its = add_layers_loop(&inp);
    Run { its, pp, lt, n_layers_total }
}

fn loop_input<'a>(
    m: &'a MergeMesh,
    pp: &'a LayerPatch,
    medial: &'a MedialAxis,
    lt: &'a LayerThickness,
    st: ExtrudeState,
    n_relaxed_iter: usize,
    relaxed: EsiQualityControls,
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
        relaxed_quality: relaxed,
        n_layer_iter: 50,
        n_relaxed_iter,
        n_grow: 0,
        n_buffer_cells_no_extrude: 0,
        n_relax_iter: 5,
        n_allowable_errors: 0,
    }
}

/// Expected digests of one ESI layer iteration: shrunk mesh points, layer mesh faces and points.
struct Ref {
    shrunk_points: u64,
    layer_faces: u64,
    layer_points: u64,
}

/// Compare every iteration with ESI's digests; returns the per-item mismatches (printed as well).
fn compare(c: &Case, run: &Run, refs: &[Ref], final_faces: u64) -> Vec<String> {
    let mut bad = Vec::new();
    for (k, it) in run.its.iter().enumerate() {
        let sp = point_bytes(&it.shrunk.points);
        let lp = point_bytes(&it.layers.mesh.mesh.points);
        let lf = face_bytes(&it.layers.mesh, 1);
        dump(c.tag, &format!("shrunk{k}"), None, &sp);
        dump(c.tag, &format!("layer{k}"), Some(&lf), &lp);
        let want = &refs[k];
        for (what, got, exp) in [("shrunk points", fnv1a(&sp), want.shrunk_points), ("layer faces", fnv1a(&lf), want.layer_faces), ("layer points", fnv1a(&lp), want.layer_points)] {
            eprintln!("{}: iteration {k} {what}: port {got:016x} ESI {exp:016x} {}", c.tag, if got == exp { "equal" } else { "DIFFERENT" });
            if got != exp {
                bad.push(format!("{} iteration {k} {what}", c.tag));
            }
        }
    }
    let last = &run.its.last().unwrap().layers.mesh;
    let ff = face_bytes(last, 0);
    dump(c.tag, "final", Some(&ff), &point_bytes(&last.mesh.points));
    let got = fnv1a(&ff);
    eprintln!("{}: final faces: port {got:016x} ESI {final_faces:016x}", c.tag);
    if got != final_faces {
        bad.push(format!("{} final faces", c.tag));
    }
    bad
}

/// P60: the round-9 case from the rebuilt ESI's snapped mesh.
#[test]
fn base_case_from_rebuilt_esi() {
    let c = Case { tag: "base", n_layers: 1, expansion: 1.0, n_relaxed_iter: 20 };
    let run = run_case(&c);
    assert_eq!(run.n_layers_total, 8521);
    let refs = [
        Ref { shrunk_points: 0xb4fd754fb8f23dae, layer_faces: 0x8dcf90c292fdaf51, layer_points: 0x52cf490a1135ee4d },
        Ref { shrunk_points: 0x1bb86ecda2ea1423, layer_faces: 0x0c7c2aa6517ef763, layer_points: 0x10bb462d20687711 },
        Ref { shrunk_points: 0xe9b3255b3b2372eb, layer_faces: 0xb74d31243d7a99e7, layer_points: 0xfcc9d627e5f1201f },
    ];
    assert_eq!(run.its.len(), 3, "ESI: three layer iterations");
    let bad = compare(&c, &run, &refs, 0x915ab24c5e104db8);
    assert!(bad.is_empty(), "{bad:?}");
    // the final mesh points are those of the last layer mesh
    assert_eq!(fnv1a(&point_bytes(&run.its[2].layers.mesh.mesh.points)), 0xfcc9d627e5f1201f);
}

/// P61-P64: three layers, expansion ratio 1.2.
#[test]
fn three_layers_expansion_1p2_against_esi() {
    let c = Case { tag: "layers3", n_layers: 3, expansion: 1.2, n_relaxed_iter: 20 };
    let run = run_case(&c);
    // P61: "Layers to add in current iteration : 25563" and the thickness table
    assert_eq!(run.n_layers_total, 25563, "P61 ESI: Layers to add in current iteration");
    let want = [(7209usize, "1.75e-4", "4.77e-4"), (1174, "1.83e-4", "4.35e-4"), (240, "1.77e-4", "4.64e-4"), (1124, "1.70e-4", "4.99e-4")];
    for (row, (nf, nw, ov)) in run.lt.rows.iter().zip(want) {
        eprintln!("{} faces {} near-wall {:.2e} overall {:.2e}", row.patch, row.n_faces, row.avg_near_wall, row.avg_thickness);
        assert_eq!(row.n_faces, nf);
        assert_eq!(format!("{:.2e}", row.avg_near_wall), nw, "P61 near-wall");
        assert_eq!(format!("{:.2e}", row.avg_thickness), ov, "P61 overall");
    }
    // P62: the iteration log
    let removed: Vec<usize> = run.its.iter().map(|i| i.n_changed).collect();
    let extruded: Vec<usize> = run.its.iter().map(|i| i.n_extruded_faces).collect();
    let added: Vec<usize> = run.its.iter().map(|i| i.n_added_cells).collect();
    eprintln!("layers3: removed {removed:?} extruded {extruded:?} added cells {added:?}");
    assert_eq!(removed, vec![12, 4, 2, 0], "P62 removed faces");
    assert_eq!(extruded, vec![8769, 8733, 8723, 8723], "P62 extruded faces");
    assert_eq!(added, vec![18918, 18767, 18727, 18711], "P62 added cells");
    // P62-P64: meshes
    let refs = [
        Ref { shrunk_points: 0xf7f8ffcc4699474c, layer_faces: 0xd59160e6823bdfaf, layer_points: 0xf0e49305ad073a37 },
        Ref { shrunk_points: 0x0d207d0795d48042, layer_faces: 0x6657d6c941e7b8e4, layer_points: 0x8795833c7f7d8bac },
        Ref { shrunk_points: 0x22298d344eaa20c4, layer_faces: 0x98941e07d1293cbc, layer_points: 0xdd3ab2f475d24038 },
        Ref { shrunk_points: 0x25f949c838a1c256, layer_faces: 0x607c348fa0cb407d, layer_points: 0x454f49281bcc621f },
    ];
    let bad = compare(&c, &run, &refs, 0xfe3b90ebac36bf2c);
    assert!(bad.is_empty(), "{bad:?}");
    let last = run.its.last().unwrap();
    assert_eq!(
        (last.layers.mesh.mesh.faces.len(), last.layers.mesh.mesh.points.len()),
        (117798, 43968),
        "ESI: faces 117798, points 43968"
    );
    // P64: printLayerData
    let patch_ids: Vec<usize> = (0..4).collect();
    let rows = layer_data_table(last, &run.pp, &run.lt.thickness, &patch_ids, &[3, 3, 3, 3]);
    let esi_table = [(7209usize, "1.86", "0.000256", "49"), (1174, "2.03", "0.000308", "59.7"), (240, "1.93", "0.000296", "57.8"), (1124, "2.21", "0.000272", "49.9")];
    let sig3 = |x: f64| format!("{:.2e}", x);
    let sig3s = |s: &str| format!("{:.2e}", s.parse::<f64>().unwrap());
    for (r, e) in rows.iter().zip(&esi_table) {
        eprintln!("patch {}: {} faces, layers {:.4}, thickness {:.4e}, {:.3} %", r.patch, r.n_faces, r.avg_layers, r.avg_real, r.avg_fraction_percent);
        assert_eq!(r.n_faces, e.0);
        assert_eq!(sig3(r.avg_layers), sig3s(e.1), "patch {} layers", r.patch);
        assert_eq!(sig3(r.avg_real), sig3s(e.2), "patch {} thickness [m]", r.patch);
        assert_eq!(sig3(r.avg_fraction_percent), sig3s(e.3), "patch {} thickness [%]", r.patch);
    }
}

/// P65: `nRelaxedIter 1`, iterations from 1 use `meshQualityControls.relaxed`.
#[test]
fn relaxed_quality_path_against_esi() {
    let c = Case { tag: "relaxed", n_layers: 1, expansion: 1.0, n_relaxed_iter: 1 };
    let run = run_case(&c);
    let removed: Vec<usize> = run.its.iter().map(|i| i.n_changed).collect();
    let extruded: Vec<usize> = run.its.iter().map(|i| i.n_extruded_faces).collect();
    eprintln!("relaxed: removed {removed:?} extruded {extruded:?}; checks {:?}", run.its.iter().map(|i| i.check.new_faces.clone()).collect::<Vec<_>>());
    assert_eq!(removed, vec![17, 0], "ESI: 17 removed, then 0 under the relaxed controls (base case: 17, 2, 0)");
    assert_eq!(extruded, vec![8724, 8660], "ESI log");
    let refs = [
        Ref { shrunk_points: 0xb4fd754fb8f23dae, layer_faces: 0x8dcf90c292fdaf51, layer_points: 0x52cf490a1135ee4d },
        Ref { shrunk_points: 0x1bb86ecda2ea1423, layer_faces: 0x0c7c2aa6517ef763, layer_points: 0x10bb462d20687711 },
    ];
    let bad = compare(&c, &run, &refs, 0x1fa36a7e47db0a38);
    assert!(bad.is_empty(), "{bad:?}");
}
