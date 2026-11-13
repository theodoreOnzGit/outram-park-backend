// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506 `blockMesh` (spline, BSpline,
// arc, lineDivide); the frozen hashes below come from runs of that release's
// `blockMesh`.

//! ESI v2506 `blockMesh` pins (gh:#1364, gh:#1458): the port in its ESI flavour
//! (`ProjectionFlavour::Esi`) against a real OpenFOAM.com v2506 `blockMesh`
//! (`/home/user/upstream-build`, `writePrecision 17`).
//!
//! **Method and results:**
//! `verification_and_validation/esi_v2506_differential_gh1364/README.md`. The
//! binary is not needed here: this file holds FNV-1a hashes of the binary's
//! `points` (bit patterns) and of the topology (`faces`, `owner`, `neighbour`).
//!
//! - Dicts `bm_polyline`, `bm_spline`, `bm_bspline`, `bm_edgegrading`,
//!   `pitzDailySteady` (with `convertToMeters 1`): points bit-identical.
//! - `bm_curved_graded` (arcs plus grading) and T3A (8 arcs): points differ
//!   from the binary only through petir `cos`/`sin`/`acos` against glibc; the
//!   binary's bit patterns of the differing points are in
//!   `tests/fixtures/esi_trig_ulp_points.txt`, and the port's points with those
//!   replaced must hash to the binary's points hash.
//!
//! Two port defects found by this comparison are pinned here: the flavour was
//! dropped for a dict without a `geometry` section, and the ESI spline / arc
//! forms did not exist.

use outram_foam_basic_lib::io::PolyMesh as FoamMesh;
use outram_foam_mesh::block_mesh::BlockMeshDict;
use outram_foam_mesh::block_projection::ProjectionFlavour;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn fnv(it: impl Iterator<Item = u64>) -> u64 {
    it.fold(0xcbf29ce484222325u64, |h, b| (h ^ b).wrapping_mul(0x100000001b3))
}

fn topology_hash(m: &FoamMesh) -> u64 {
    let faces = m.faces.iter().flat_map(|f| {
        std::iter::once(f.verts.len() as u64).chain(f.verts.iter().map(|&v| v as u64))
    });
    let owner = m.faces.iter().map(|f| f.owner as u64);
    let nei = m.faces.iter().filter_map(|f| f.neighbour).map(|n| n as u64);
    fnv(faces.chain(owner).chain(nei))
}

fn esi_mesh(dict: &str) -> FoamMesh {
    BlockMeshDict::parse_with_flavour(dict, ProjectionFlavour::Esi)
        .unwrap()
        .build()
        .unwrap()
        .to_foam_poly_mesh()
}

fn bits(m: &FoamMesh) -> Vec<[u64; 3]> {
    m.points.iter().map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]).collect()
}

fn hash_bits(b: &[[u64; 3]]) -> u64 {
    fnv(b.iter().flat_map(|p| p.iter().copied()))
}

/// Five dicts whose points are bit-identical to ESI v2506 `blockMesh`.
///
/// Predictions (committed before the runs, `PREDICTIONS.md`): B1 polyLine
/// 100 %: held. B2 spline >= 95 %, worst < 1e-14: **MISS in the better
/// direction after the fix** (before the fix 121 of 288 points, worst 4.3e-2:
/// the dev form iterates a distance re-parameterisation, v2506 does not); now
/// 288 of 288. B3 BSpline: same, 138 -> 288. B4 edgeGrading and multi-section
/// grading >= 99 %: before the fix 388 of 728 (the ESI `lineDivide` pow was
/// never selected, because the flavour was dropped for dicts without a
/// `geometry` section), now 728 of 728. pitzDailySteady: 24797 of 25012 in the
/// dev flavour (gh:#1364 earlier record), now 25012 of 25012.
#[test]
fn esi_block_mesh_is_bit_identical_on_curves_and_grading() {
    // (name, dict, points, faces, ESI points hash, topology hash)
    let cases = [
        ("bm_polyline", fixture("bm_polyline.dict"), 288usize, 598usize, 0xb619de59520796ceu64),
        ("bm_spline", fixture("bm_spline.dict"), 288, 598, 0x745f2a903461372e),
        ("bm_bspline", fixture("bm_bspline.dict"), 288, 598, 0xe89856638c19aa14),
        ("bm_edgegrading", fixture("bm_edgegrading.dict"), 728, 1593, 0xb2044c90aa8ecdab),
        ("pitzDailySteady", fixture("pitzDailySteady_convertToMeters1.dict"), 25012, 49180, 0x62c233233850c4a9),
    ];
    for (name, dict, n_pts, n_faces, hash) in cases {
        let m = esi_mesh(&dict);
        assert_eq!(m.points.len(), n_pts, "{name}: points");
        assert_eq!(m.faces.len(), n_faces, "{name}: faces");
        assert_eq!(hash_bits(&bits(&m)), hash, "{name}: points not bit-identical to OpenFOAM.com v2506");
    }
}

/// Topology (faces, owner, neighbour) of the same dicts and of the two arc
/// dicts equals the binary's (`compare_polymesh`: identical loops, owner and
/// neighbour on all seven); pinned as one hash per dict.
#[test]
fn esi_block_mesh_topology_matches_the_binary() {
    let cases = [
        ("bm_polyline", fixture("bm_polyline.dict"), 598usize, 392usize, 0x04b674df65182581u64),
        ("bm_spline", fixture("bm_spline.dict"), 598, 392, 0x04b674df65182581),
        ("bm_bspline", fixture("bm_bspline.dict"), 598, 392, 0x04b674df65182581),
        ("bm_edgegrading", fixture("bm_edgegrading.dict"), 1593, 1107, 0x2ea026cc62699a15),
        ("bm_curved_graded", fixture("bm_curved_graded.dict"), 944, 592, 0x95ec5bf5324d1d1b),
        ("T3A", fixture("T3A_blockMeshDict"), 107707, 53213, 0x679f3117dfb43b37),
    ];
    for (name, dict, n_faces, n_internal, topo) in cases {
        let m = esi_mesh(&dict);
        assert_eq!(m.faces.len(), n_faces, "{name}: faces");
        assert_eq!(m.n_internal_faces, n_internal, "{name}: internal faces");
        // the owner/neighbour/loop equality with the binary was established by
        // `compare_polymesh`; the hash makes a regression visible.
        assert_eq!(topology_hash(&m), topo, "{name}: faces/owner/neighbour differ from OpenFOAM.com v2506");
    }
}

/// Arc dicts: points differ from the binary only at the pinned petir-trig
/// points (count and worst difference pinned).
///
/// Cause established by experiment (2026-11-13): with `std` `cos`/`sin`/`acos`
/// in `ArcEdgeEsi` instead of petir's, `bm_curved_graded` is 459 of 459
/// bit-identical (the experiment was reverted; this crate must not call the
/// platform libm). Before the ESI arc form existed, 58 of 459 were identical
/// and the worst difference 2.9e-15; T3A was 49965 of 54496 in the dev flavour.
#[test]
fn esi_block_mesh_arc_dicts_differ_only_at_pinned_trig_points() {
    let table = fixture("esi_trig_ulp_points.txt");
    // (name, dict, ESI points hash, differing points, worst abs diff)
    let cases = [
        ("bm_curved_graded", fixture("bm_curved_graded.dict"), 0xce65fcf0f41dbc85u64, 21usize, 2.220446049250313e-16f64),
        ("T3A", fixture("T3A_blockMeshDict"), 0x2af4d4f544750f5c, 215, 1.1102230246251565e-16),
    ];
    for (name, dict, esi_hash, n_diff, worst_pin) in cases {
        let m = esi_mesh(&dict);
        let mut b = bits(&m);
        let (mut n, mut worst) = (0usize, 0.0f64);
        for line in table.lines().filter(|l| !l.starts_with('#')) {
            let t: Vec<&str> = line.split_whitespace().collect();
            if t.len() != 5 || t[0] != name {
                continue;
            }
            let i: usize = t[1].parse().unwrap();
            let esi: Vec<u64> = t[2..5].iter().map(|h| u64::from_str_radix(h, 16).unwrap()).collect();
            for k in 0..3 {
                worst = worst.max((f64::from_bits(b[i][k]) - f64::from_bits(esi[k])).abs());
            }
            assert_ne!(b[i], [esi[0], esi[1], esi[2]], "{name} point {i} is bit-identical, drop it from the table");
            b[i] = [esi[0], esi[1], esi[2]];
            n += 1;
        }
        assert_eq!(n, n_diff, "{name}: differing points");
        assert_eq!(worst, worst_pin, "{name}: worst difference");
        assert_eq!(hash_bits(&b), esi_hash, "{name}: points outside the table are not bit-identical");
    }
}

/// The ESI flavour must reach a dict WITHOUT a `geometry` section: the dev and
/// ESI forms of the same spline dict give different points (the dev form
/// iterates a distance re-parameterisation), by well over round-off.
#[test]
fn esi_flavour_reaches_dicts_without_geometry() {
    let dict = fixture("bm_spline.dict");
    let dev = BlockMeshDict::parse(&dict).unwrap().build().unwrap().to_foam_poly_mesh();
    let esi = esi_mesh(&dict);
    let worst = dev
        .points
        .iter()
        .zip(&esi.points)
        .map(|(a, b)| (a.x - b.x).abs().max((a.y - b.y).abs()).max((a.z - b.z).abs()))
        .fold(0.0f64, f64::max);
    assert!(worst > 1e-3 && worst < 0.1, "dev vs ESI spline worst difference {worst:e}");
    // 4.34e-2 measured against the binary before the fix (2026-11-13)
}
