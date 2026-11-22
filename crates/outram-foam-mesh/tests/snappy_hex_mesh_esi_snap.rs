// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1467 / #1459 round 3: the snapping stages of OpenFOAM.com ESI v2506
//! `snappyHexMesh` on the flange, one stage at a time, from ESI's own castellated
//! mesh read in (so numbering and summation order are ESI's).
//!
//! Methodology, case, predictions P21-P25 (committed before the port code) and the
//! results: `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`,
//! "Round 3, task 2". Reference data: exact binary doubles from the v2506 binary
//! (`tests/fixtures/esi_v2506_flange/snap_*.bin.gz`, `extract_snap_fixtures.py`).

use outram_foam_mesh::snappy_hex_mesh::esi_snap::{
    calc_nearest_surface, calc_snap_distance, displacement_field, mesh_edges, outwards_displacement,
    patch_addressing, pre_smooth_patch_iteration, smooth_displacement, SnapMesh, V,
};
use outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes;
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");

/// Inflate a `gzip -n` file (10-byte header, 8-byte trailer).
fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b, "not gzip");
    assert_eq!(bytes[3], 0, "gzip -n expected");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn fixture(name: &str) -> Vec<u8> {
    gunzip(&std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture"))
}

/// Little-endian float64 triples.
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

/// ESI's castellated flange mesh (final cells of task 1): faces, owner, neighbour
/// from `esi_final_faces`, exact points from `snap_points_32`.
fn esi_castellated() -> SnapMesh {
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
    SnapMesh { points: vectors("snap_points_32.bin.gz"), faces, owner, neighbour }
}

fn upstream_surface() -> TriangleSoup {
    TriangleSoup::from_zoned(&read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap())
}

/// (number of values bit-identical, number of values compared, largest |difference|).
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

/// P22: `preSmoothPatch`, three iterations, against ESI's points after
/// `nSmoothPatch` = 1, 2, 3 (three ESI runs).
#[test]
fn pre_smooth_patch_iterations_against_esi() {
    let mut mesh = esi_castellated();
    assert_eq!((mesh.points.len(), mesh.faces.len(), mesh.n_internal_faces()), (26662, 63194, 51058));
    let pa = patch_addressing(&mesh);
    assert_eq!((pa.face_ids.len(), pa.mesh_points.len()), (12136, 12130));
    for (iter, name) in ["snap_points_33_n1.bin.gz", "snap_points_33_n2.bin.gz", "snap_points_33_n3.bin.gz"].iter().enumerate() {
        pre_smooth_patch_iteration(&mut mesh, &pa);
        let (same, n, worst) = compare(&mesh.points, &vectors(name));
        eprintln!("preSmooth iteration {}: bit-identical {same} of {n}, worst {worst:e}", iter + 1);
        assert_eq!(same, n, "iteration {} is not bit-identical (worst {worst:e})", iter + 1);
    }
}

/// P21: the surface attraction from the patch-smoothed mesh against the patch
/// points of `34/displacement`.
#[test]
fn surface_displacement_against_esi() {
    let mesh0 = esi_castellated();
    let pa = patch_addressing(&mesh0);
    // calcSnapDistance is evaluated on the castellated mesh, before smoothing.
    let snap_dist = calc_snap_distance(&mesh0, &pa, 1.0);
    let mut mesh = mesh0.clone();
    mesh.points = vectors("snap_points_33_n3.bin.gz");
    let local: Vec<V> = pa.mesh_points.iter().map(|&p| mesh.points[p]).collect();
    let (disp, missing) = calc_nearest_surface(&local, &snap_dist, &upstream_surface());
    assert_eq!(missing, 0, "every patch point finds the surface within snapDist");
    let mag: Vec<f64> = disp.iter().map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).collect();
    let mean = mag.iter().sum::<f64>() / mag.len() as f64;
    eprintln!(
        "wanted displacement: mean {mean:e} min {:e} max {:e} (ESI 1.688540950712846e-4, 6.548561578869996e-8, 7.516794439290358e-4)",
        mag.iter().cloned().fold(f64::MAX, f64::min),
        mag.iter().cloned().fold(0.0, f64::max)
    );
    let esi_disp = vectors("snap_disp_34.bin.gz");
    let esi_patch: Vec<V> = pa.mesh_points.iter().map(|&p| esi_disp[p]).collect();
    let (same, n, worst) = compare(&disp, &esi_patch);
    eprintln!("surface displacement: bit-identical {same} of {n}, worst {worst:e}");
    assert!(worst <= 1e-12, "P21: worst difference {worst:e}");
    // Measured 2026-11-12: all but one of the 36390 components are bit-identical; the one is a
    // difference of 2.2e-19 m (the nearest-point routine's operation order).
    assert_eq!(same, n - 1, "P21 measured: one component differs in the last bits");
    assert!(worst < 3e-19);
    // The wanted displacement of ESI's log: min and max exactly, mean to the last printed digit.
    assert_eq!(mag.iter().cloned().fold(f64::MAX, f64::min), 6.548561578869996e-8);
    assert_eq!(mag.iter().cloned().fold(0.0, f64::max), 7.516794439290358e-4);
    assert!((mean - 1.688540950712846e-4).abs() < 1e-18);
    // ESI's warning: mesh point 16952 is the first displacement through its faces.
    let first = outwards_displacement(&mesh, &pa, &disp).map(|pi| pa.mesh_points[pi]);
    assert_eq!(first, Some(16952), "ESI: Displacement (-9.95603976288225e-05 0 0) at mesh point 16952");
}

/// P23: `smoothDisplacement`, 300 iterations, from ESI's patch displacement, against
/// the whole `34/displacement` field.
#[test]
fn smooth_displacement_against_esi() {
    let mut mesh = esi_castellated();
    mesh.points = vectors("snap_points_33_n3.bin.gz");
    let pa = patch_addressing(&mesh);
    let esi_disp = vectors("snap_disp_34.bin.gz");
    let patch_disp: Vec<V> = pa.mesh_points.iter().map(|&p| esi_disp[p]).collect();
    let mut disp = displacement_field(&mesh, &pa, &patch_disp);
    let me = mesh_edges(&mesh);
    smooth_displacement(&mesh, &me, &pa, &mut disp, 300);
    let (same, n, worst) = compare(&disp, &esi_disp);
    let largest = esi_disp.iter().flat_map(|d| d.iter()).fold(0.0f64, |m, &x| m.max(x.abs()));
    eprintln!("smoothDisplacement: bit-identical {same} of {n} ({:.2} %), worst {worst:e}, largest {largest:e}", 100.0 * same as f64 / n as f64);
    assert!(worst <= 1e-12 * largest, "P23: worst {worst:e}");
    assert_eq!(same, n, "P23: bit-identical (first run, before the edge renumbering fix: 47.98 %, worst 4.76e-7)");
}
