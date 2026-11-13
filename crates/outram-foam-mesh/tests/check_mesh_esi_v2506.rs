// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506 `primitiveMeshTools` /
// `primitiveMesh::check*`; the frozen hashes and numbers come from runs of that
// release's `dumpQuality` probe and `checkMesh`.

//! ESI v2506 `checkMesh` metric pins (gh:#1364, row 6): the ESI-form fields
//! of [`outram_foam_mesh::check_mesh::esi_quality_fields`] against OpenFOAM.com
//! v2506 on four small meshes written by that release's tools
//! (`tests/fixtures/esi_checkmesh/<case>/`: `blockMesh` or `polyDualMesh` output
//! at `writePrecision 17`).
//!
//! **Method and results:**
//! `verification_and_validation/esi_v2506_differential_gh1364/README.md`. The
//! `dumpQuality` probe (same directory) calls `primitiveMeshTools` on the mesh
//! and writes every field with `%.17g`; `esi_quality_hashes.txt` holds the
//! FNV-1a of the IEEE-754 bit patterns per field, so a pass means every one of
//! the thousands of values is bit-identical. The aggregate numbers are those
//! ESI's `checkMesh -allGeometry -allTopology` printed at 17 digits.
//!
//! Cases: `cavity` (2D, `empty` patches: `meshD = (1 1 -1)`, aspect ratio has no
//! `pow` term), `warped3d` (3D hex, non-orthogonal and skew), `edgegrading3d`
//! (3D graded hex, aspect ratio 11.7) and `dual_warped3d` (polyhedral
//! `polyDualMesh`, 45 concave cells in ESI's report).

use outram_foam_basic_lib::mesh::PatchKind;
use outram_foam_mesh::check_mesh::{esi_check_summary, esi_geometric_d, esi_geometry, esi_quality_fields};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::SnapMesh;

fn dir(case: &str) -> String {
    format!("{}/tests/fixtures/esi_checkmesh/{case}", env!("CARGO_MANIFEST_DIR"))
}

fn fnv(values: &[f64]) -> u64 {
    values.iter().fold(0xcbf29ce484222325u64, |h, v| (h ^ v.to_bits()).wrapping_mul(0x100000001b3))
}

fn flat(v: &[[f64; 3]]) -> Vec<f64> {
    v.iter().flat_map(|p| p.iter().copied()).collect()
}

fn load(case: &str) -> (SnapMesh, [i32; 3]) {
    let io = outram_foam_basic_lib::io::PolyMesh::read(&dir(case)).expect("read polyMesh");
    let mesh = SnapMesh {
        points: io.points.iter().map(|p| [p.x, p.y, p.z]).collect(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
    };
    let areas = esi_geometry(&mesh).face_areas;
    let empty: Vec<usize> = io
        .patches
        .iter()
        .filter(|p| p.kind == PatchKind::Empty)
        .flat_map(|p| p.start..p.start + p.size)
        .collect();
    (mesh, esi_geometric_d(&areas, &empty))
}

/// Every field of every case is bit-identical to the ESI probe.
///
/// Predictions C1 to C7 (committed before the runs, `PREDICTIONS.md`): all held.
/// C4 predicted a minority of 1-ulp differences in the 3D aspect ratio through
/// `pow(v, 2/3)` (glibc against petir): **MISS in the good direction**, 0 of
/// 120 / 450 / 210 / 26820 (T3A, run in the example) cells differ.
#[test]
fn esi_check_mesh_fields_are_bit_identical_to_the_probe() {
    let table = std::fs::read_to_string(format!("{}/esi_quality_hashes.txt", dir(""))).unwrap();
    for case in ["cavity", "warped3d", "edgegrading3d", "dual_warped3d"] {
        let (mesh, mesh_d) = load(case);
        let q = esi_quality_fields(&mesh, mesh_d);
        let g = &q.geometry;
        let ours: Vec<(&str, Vec<f64>)> = vec![
            ("faceCentres", flat(&g.face_centres)),
            ("faceAreas", flat(&g.face_areas)),
            ("cellCentres", flat(&g.cell_centres)),
            ("cellVolumes", g.cell_volumes.clone()),
            ("ortho", q.orthogonality.clone()),
            ("skew", q.skewness.clone()),
            ("ownPyr", q.own_pyramid.clone()),
            ("neiPyr", q.nei_pyramid.clone()),
            ("openness", q.openness.clone()),
            ("aspect", q.aspect_ratio.clone()),
            ("flatness", q.flatness.clone()),
            ("concavity", q.concavity.clone()),
            ("determinant", q.determinant.clone()),
        ];
        let mut checked = 0;
        for (field, values) in &ours {
            let line = table
                .lines()
                .find(|l| {
                    let t: Vec<&str> = l.split_whitespace().collect();
                    t.len() == 4 && t[0] == case && t[1] == *field
                })
                .unwrap_or_else(|| panic!("{case} {field} not in the table"));
            let t: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(values.len(), t[2].parse::<usize>().unwrap(), "{case} {field}: length");
            let hash = u64::from_str_radix(t[3].trim_start_matches("0x"), 16).unwrap();
            assert_eq!(fnv(values), hash, "{case} {field}: not bit-identical to OpenFOAM.com v2506");
            checked += 1;
        }
        assert_eq!(checked, 13);
    }
}

/// The aggregate numbers `checkMesh -allGeometry` printed (17 digits) are
/// reproduced exactly.
#[test]
fn esi_check_mesh_summary_equals_the_printed_numbers() {
    // (case, meshD, nonOrth max, nonOrth avg, max skew, max aspect, min det, avg det,
    //  total volume, min face area)
    let cases: [(&str, [i32; 3], [f64; 8]); 4] = [
        ("cavity", [1, 1, -1], [0.0, 0.0, 1.8873791418627412e-14, 1.0000000000000071, 0.125, 0.45124999999999998, 0.00010000000000000063, 2.4999999999999835e-05]),
        ("warped3d", [1, 1, 1], [16.431405462812609, 6.3796077999581682, 0.37332106677434906, 2.1281586647160515, 0.10229250097415273, 0.439696649192652, 1.241708333333333, 0.033333333333333305]),
        ("edgegrading3d", [1, 1, 1], [22.781209074273114, 6.7327673944327477, 0.44295694406712516, 11.666666666666721, 0.0010022324085916762, 0.23987742684226576, 3.0, 0.0032746533082437961]),
        ("dual_warped3d", [1, 1, 1], [20.171844265181999, 7.205579645029073, 1.274168098170319, 4.0614137598947568, 0.098288582343558778, 0.5974404212032427, 1.2417086104533346, 0.0083333333333333176]),
    ];
    for (case, want_d, want) in cases {
        let (mesh, mesh_d) = load(case);
        assert_eq!(mesh_d, want_d, "{case}: geometricD");
        let s = esi_check_summary(&esi_quality_fields(&mesh, mesh_d));
        let got = [
            s.max_non_orthogonality_deg,
            s.avg_non_orthogonality_deg,
            s.max_skewness,
            s.max_aspect_ratio,
            s.min_determinant,
            s.avg_determinant,
            s.total_volume,
            s.min_face_area,
        ];
        for (k, (g, w)) in got.iter().zip(want).enumerate() {
            assert_eq!(g.to_bits(), w.to_bits(), "{case}: summary[{k}] {g:e} vs {w:e}");
        }
    }
}

/// `geometricD` follows `polyMesh::calcDirections`: no empty patch leaves all
/// three directions; the face areas of an empty patch remove its normal.
#[test]
fn geometric_d_removes_the_empty_patch_normal() {
    let areas = vec![[0.0, 0.0, 2.0], [0.0, 0.0, -2.0], [1.0, 0.0, 0.0]];
    assert_eq!(esi_geometric_d(&areas, &[]), [1, 1, 1]);
    assert_eq!(esi_geometric_d(&areas, &[0, 1]), [1, 1, -1]);
    assert_eq!(esi_geometric_d(&areas, &[0, 1, 2]), [-1, 1, -1]);
}
