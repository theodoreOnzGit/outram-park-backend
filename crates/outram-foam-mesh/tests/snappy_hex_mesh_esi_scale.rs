// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1467 / #1459 round 4, tasks 1 and 2: the nine `meshQualityControls` face checks
//! of ESI v2506 (`motionSmootherAlgo::checkMesh`, mesh overload) and `scaleMesh`
//! (`errorReduction`), on the reduced flange, against ESI's own meshes.
//!
//! Methodology, predictions P26-P28 (committed before the port code) and results:
//! `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`, "Round 4".
//! References: exact binary doubles of the v2506 binary
//! (`tests/fixtures/esi_v2506_flange/snap_*.bin.gz`) and the per-check face sets of
//! `checkMesh -meshQuality -time 35` (`esi_t35_wrong_faces_by_check.txt.gz`).

use outram_foam_mesh::check_mesh::{
    esi_check_cell_determinant, esi_check_face_angles, esi_check_face_pyramids, esi_check_face_skewness,
    esi_check_face_tets, esi_check_face_twist, esi_check_face_weights, esi_check_mesh,
    esi_check_non_orthogonality, esi_check_vol_ratio, esi_geometry, EsiCheck, EsiQualityControls,
};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{
    mesh_edges, patch_addressing, scale_mesh_loop, scale_topology, MotionSmoother, PatchPointScaling,
    ScaleControls, SnapMesh, V,
};

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

/// ESI's castellated flange mesh, with the points of `points_fixture`.
fn esi_mesh(points_fixture: &str) -> SnapMesh {
    let text = String::from_utf8(fixture("esi_final_faces.txt.gz")).unwrap();
    let mut faces = Vec::new();
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for l in text.lines() {
        let v: Vec<i64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
        owner.push(v[0] as usize);
        if v[1] >= 0 {
            neighbour.push(v[1] as usize);
        }
        faces.push(v[2..].iter().map(|&p| p as usize).collect());
    }
    SnapMesh { points: vectors(points_fixture), faces, owner, neighbour }
}

/// Per-check face sets of `checkMesh -meshQuality -time 35`.
fn esi_sets() -> Vec<(String, Vec<usize>)> {
    String::from_utf8(fixture("esi_t35_wrong_faces_by_check.txt.gz"))
        .unwrap()
        .lines()
        .map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next().unwrap().to_string();
            (name, it.map(|x| x.parse().unwrap()).collect())
        })
        .collect()
}

fn set_of<'a>(sets: &'a [(String, Vec<usize>)], name: &str) -> &'a Vec<usize> {
    &sets.iter().find(|(n, _)| n == name).expect("set").1
}

/// P26: each of the nine checks alone, on ESI's time-35 mesh, equals the face set ESI's
/// `checkMesh -meshQuality` finds with only that check enabled; the union is the 8 faces
/// of the ESI log; the castellated (time 32) and snapped (time 36) meshes are clean.
#[test]
fn nine_face_checks_against_esi() {
    let sets = esi_sets();
    let mesh = esi_mesh("snap_points_35.bin.gz");
    let geom = esi_geometry(&mesh);
    let all: Vec<usize> = (0..mesh.faces.len()).collect();
    let c = EsiQualityControls::default();
    let got: Vec<(&str, Vec<usize>)> = vec![
        ("nonortho", esi_check_non_orthogonality(&mesh, &geom, c.max_non_ortho, &all)),
        ("pyramids", esi_check_face_pyramids(&mesh, &geom, c.min_vol, &all)),
        ("tets", esi_check_face_tets(&mesh, &geom, c.min_tet_quality, &all)),
        ("concave", esi_check_face_angles(&mesh, &geom, c.max_concave, &all)),
        ("skew", esi_check_face_skewness(&mesh, &geom, c.max_internal_skewness, c.max_boundary_skewness, &all)),
        ("weights", esi_check_face_weights(&mesh, &geom, c.min_face_weight, &all)),
        ("volratio", esi_check_vol_ratio(&mesh, &geom, c.min_vol_ratio, &all)),
        ("twist", esi_check_face_twist(&mesh, &geom, c.min_twist, &all)),
        ("det", esi_check_cell_determinant(&mesh, &geom, c.min_determinant, &all)),
    ];
    for (name, faces) in &got {
        let mut f = faces.clone();
        f.sort_unstable();
        f.dedup();
        eprintln!("time 35, {name}: port {f:?}, ESI {:?}", set_of(&sets, name));
        assert_eq!(&f, set_of(&sets, name), "check {name}");
    }
    // Pyramids at 1e-15 (the polyMeshGeometry overload's threshold): same set on this mesh.
    assert_eq!(&esi_check_face_pyramids(&mesh, &geom, 1e-15, &all), set_of(&sets, "pyr15"));
    // The combined run and its cumulative-new counts (ESI log: 0, 4, +2, 0, 0, 0, 0, +2, 0).
    let r = esi_check_mesh(&mesh, &geom, &c, &all);
    assert_eq!(r.wrong_faces.len(), 8);
    let counts: Vec<(EsiCheck, usize)> = r.new_faces.clone();
    assert_eq!(
        counts,
        vec![
            (EsiCheck::NonOrthogonality, 0),
            (EsiCheck::FacePyramids, 4),
            (EsiCheck::FaceTets, 2),
            (EsiCheck::Concavity, 0),
            (EsiCheck::Skewness, 0),
            (EsiCheck::FaceWeights, 0),
            (EsiCheck::VolRatio, 0),
            (EsiCheck::FaceTwist, 2),
            (EsiCheck::CellDeterminant, 0),
        ]
    );
    // Only the checks that find faces: restrict to a face subset (the first iteration's
    // checkFaces is all faces, later ones a subset; faces outside it are not reported).
    let subset: Vec<usize> = vec![56918, 15275, 3];
    let r = esi_check_mesh(&mesh, &geom, &c, &subset);
    assert_eq!(r.wrong_faces, vec![15275, 56918]);
    // Clean meshes.
    for name in ["snap_points_32.bin.gz", "snap_points_36.bin.gz"] {
        let m = esi_mesh(name);
        let g = esi_geometry(&m);
        let all: Vec<usize> = (0..m.faces.len()).collect();
        let r = esi_check_mesh(&m, &g, &c, &all);
        eprintln!("{name}: {} faces in error", r.wrong_faces.len());
        assert!(r.wrong_faces.is_empty(), "{name}: {:?}", r.new_faces);
    }
    // All checks disabled flags nothing even on the failing mesh.
    let r = esi_check_mesh(&mesh, &geom, &EsiQualityControls::all_disabled(), &all);
    assert!(r.wrong_faces.is_empty() && r.new_faces.is_empty());
}

/// (bit-identical components, components, worst difference).
fn compare(a: &[V], b: &[V]) -> (usize, usize, f64) {
    assert_eq!(a.len(), b.len());
    let mut same = 0usize;
    let mut worst = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        for c in 0..3 {
            if x[c].to_bits() == y[c].to_bits() {
                same += 1;
            }
            worst = worst.max((x[c] - y[c]).abs());
        }
    }
    (same, 3 * a.len(), worst)
}

/// Run `scaleMesh` from the time-33 points and `34/displacement`; returns the snapped
/// points and the iterations.
fn run_scale(patch_scaling: PatchPointScaling) -> (SnapMesh, MotionSmoother, Vec<outram_foam_mesh::snappy_hex_mesh::esi_snap::ScaleIteration>) {
    let mut mesh = esi_mesh("snap_points_33_n3.bin.gz");
    let pa = patch_addressing(&mesh);
    let disp = vectors("snap_disp_34.bin.gz");
    let mut ms = MotionSmoother::new(&mesh, &pa, disp);
    let me = mesh_edges(&mesh);
    let topo = scale_topology(&mesh);
    let ctl = ScaleControls {
        error_reduction: 0.75,
        n_smooth_scale: 4,
        quality: EsiQualityControls::default(),
        n_allowable_errors: 0,
        patch_scaling,
    };
    let reports = scale_mesh_loop(&mut mesh, &mut ms, &pa, &me, &topo, &ctl, 5);
    (mesh, ms, reports)
}

/// P27 and P28: `scaleMesh` against ESI's time-35 (iteration 0) and time-36 (snapped)
/// points, and the checks on the result.
#[test]
fn scale_mesh_against_esi() {
    // Iteration 0 moves the mesh to oldPoints + 1 * displacement = ESI's time 35.
    let mesh33 = esi_mesh("snap_points_33_n3.bin.gz");
    let pa = patch_addressing(&mesh33);
    let ms0 = MotionSmoother::new(&mesh33, &pa, vectors("snap_disp_34.bin.gz"));
    let p35 = ms0.cur_points(&pa, PatchPointScaling::Scaled);
    let (same, n, worst) = compare(&p35, &vectors("snap_points_35.bin.gz"));
    eprintln!("iteration 0 points vs ESI time 35: bit-identical {same} of {n}, worst {worst:e}");
    assert_eq!(same, n, "oldPoints + displacement is ESI's time-35 mesh");

    let esi36 = vectors("snap_points_36.bin.gz");
    // Measured 2026-11-12 (release): patch points scaled by the scale field.
    let (mesh, ms, reports) = run_scale(PatchPointScaling::Scaled);
    let (same, n, worst) = compare(&mesh.points, &esi36);
    eprintln!("{} iterations; bit-identical {same} of {n}, worst {worst:e}", reports.len());
    assert_eq!(reports.len(), 2, "iteration 0 fails, iteration 1 is accepted");
    assert_eq!((reports[0].scale_min, reports[0].scale_max), (1.0, 1.0));
    assert_eq!(reports[0].check.wrong_faces.len(), 8);
    assert_eq!(reports[0].check.wrong_faces, vec![15275, 28221, 51965, 56915, 56918, 58032, 58040, 58067]);
    assert!(!reports[0].ok);
    assert_eq!((reports[1].scale_min, reports[1].scale_max), (0.75, 1.0));
    assert!(reports[1].check.wrong_faces.is_empty() && reports[1].ok, "P28 during the move");
    assert_eq!(same, n, "P27: bit-identical with ESI's snapped mesh (time 36)");
    assert_eq!(ms.scale.iter().cloned().fold(f64::INFINITY, f64::min), 0.75, "final minimum scale");
    // P28: the checks on the scaled mesh.
    let g = esi_geometry(&mesh);
    let all: Vec<usize> = (0..mesh.faces.len()).collect();
    assert!(esi_check_mesh(&mesh, &g, &EsiQualityControls::default(), &all).wrong_faces.is_empty());
}

/// The reading in which patch points keep the unscaled displacement (the scale field's
/// boundary values stay 1) is refuted by ESI's time 36: it never converges to ESI's mesh
/// (measured: 10 iterations, 77664 of 79986 components bit-identical, worst 1.2e-4 m).
#[test]
fn unscaled_patch_points_reading_is_refuted() {
    let (mesh, _ms, reports) = run_scale(PatchPointScaling::Unscaled);
    let (same, n, worst) = compare(&mesh.points, &vectors("snap_points_36.bin.gz"));
    assert_eq!(reports.len(), 10);
    assert!(same < n && worst > 1e-5, "{same} of {n}, {worst:e}");
}
