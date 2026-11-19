// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506 `polyDualMesh`/`meshDualiser`;
// the frozen hashes below come from runs of that release's `polyDualMesh`.

//! ESI v2506 `polyDualMesh` pins (gh:#1364, gh:#1460): four multi-block dicts
//! (`polyLine`, `spline`, `BSpline` edges; `edgeGrading` with multi-section
//! grading), each dualised with `polyDualMesh 60` and the options
//! `splitAllFaces` and `concaveMultiCells`, against OpenFOAM.com v2506
//! (`/home/user/upstream-build`, `writePrecision 17`).
//!
//! The primal is built by this crate's ESI-flavour `blockMesh` (bit-identical
//! to the binary's primal for these four dicts, see `block_mesh_esi_v2506.rs`),
//! so the whole chain dict -> primal -> dual is pinned. Each case's dual
//! (points, faces, owner, neighbour) is byte-identical to the binary's
//! (`compare_polymesh`, 2026-11-13: 12 of 12 here, 27 of 27 with the five older
//! cases); the points hash is the binary's, the topology hash was computed from
//! the port's dual after that comparison reported identical loops, owner and
//! neighbour.
//!
//! Method, predictions A1/A2 and results:
//! `verification_and_validation/esi_v2506_differential_gh1364/README.md`.

use outram_foam_basic_lib::io::PolyMesh as FoamMesh;
use outram_foam_mesh::block_mesh::BlockMeshDict;
use outram_foam_mesh::block_projection::ProjectionFlavour;
use outram_foam_mesh::poly_dual_mesh::PolyMesh as Primal;
use outram_foam_mesh::poly_dual_mesh_upstream::{poly_dual_mesh_upstream, UpstreamOptions};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn fnv(it: impl Iterator<Item = u64>) -> u64 {
    it.fold(0xcbf29ce484222325u64, |h, b| (h ^ b).wrapping_mul(0x100000001b3))
}

fn points_hash(m: &FoamMesh) -> u64 {
    fnv(m.points.iter().flat_map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]))
}

fn topology_hash(m: &FoamMesh) -> u64 {
    let faces = m.faces.iter().flat_map(|f| {
        std::iter::once(f.verts.len() as u64).chain(f.verts.iter().map(|&v| v as u64))
    });
    let owner = m.faces.iter().map(|f| f.owner as u64);
    let nei = m.faces.iter().filter_map(|f| f.neighbour).map(|n| n as u64);
    fnv(faces.chain(owner).chain(nei))
}

fn dual(dict: &str, split: bool, concave: bool) -> FoamMesh {
    let io = BlockMeshDict::parse_with_flavour(dict, ProjectionFlavour::Esi)
        .unwrap()
        .build()
        .unwrap()
        .to_foam_poly_mesh();
    let primal = Primal {
        points: io.points.clone(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
        n_internal_faces: io.n_internal_faces,
        patches: io.patches.clone(),
        n_cells: io.n_cells,
    };
    let mut o = UpstreamOptions::new(60.0).with_esi2506_flavour();
    if split {
        o = o.with_split_all_faces();
    }
    if concave {
        o = o.with_concave_multi_cells();
    }
    let d = poly_dual_mesh_upstream(&primal, &o).unwrap();
    let names: Vec<String> = io.patches.iter().map(|p| p.name.clone()).collect();
    d.to_foam_poly_mesh(&names).unwrap()
}

/// 12 case x variant duals byte-identical to ESI v2506 `polyDualMesh`.
///
/// Prediction A2 (committed before the runs): byte-identical, held on all 12
/// (and on the 15 older/other cases of the same comparison).
#[test]
fn esi_poly_dual_mesh_matches_the_binary_on_curved_and_graded_blocks() {
    // (dict, variant, points, faces, ESI points hash, topology hash)
    let cases: [(&str, bool, bool, usize, usize, u64, u64); 12] = [
        ("bm_polyline.dict", false, false, 647, 1136, 0x6281f4cc39799a39, 0x5a06753b3fc6f307),
        ("bm_polyline.dict", true, false, 1555, 2396, 0x519b4eab0ef3cd14, 0xb9baed2072a9fe75),
        ("bm_polyline.dict", false, true, 647, 1136, 0x6281f4cc39799a39, 0x5a06753b3fc6f307),
        ("bm_spline.dict", false, false, 647, 1136, 0x63591135d215dd26, 0x5a06753b3fc6f307),
        ("bm_spline.dict", true, false, 1555, 2396, 0xe2f8339b2b0fa54b, 0xb9baed2072a9fe75),
        ("bm_spline.dict", false, true, 647, 1136, 0x63591135d215dd26, 0x5a06753b3fc6f307),
        ("bm_bspline.dict", false, false, 647, 1136, 0xf1eecfe212b1b5c1, 0x5a06753b3fc6f307),
        ("bm_bspline.dict", true, false, 1555, 2396, 0x074d5f2fb13b2250, 0xb9baed2072a9fe75),
        ("bm_bspline.dict", false, true, 647, 1136, 0xf1eecfe212b1b5c1, 0x5a06753b3fc6f307),
        ("bm_edgegrading.dict", false, false, 1452, 2746, 0x798a0a975c9dd183, 0xabec1ec67fc404a3),
        ("bm_edgegrading.dict", true, false, 4045, 6276, 0xda92fdfc517c1a90, 0x0ecc88a9deb50d8b),
        ("bm_edgegrading.dict", false, true, 1452, 2746, 0x798a0a975c9dd183, 0xabec1ec67fc404a3),
    ];
    for (dict, split, concave, n_pts, n_faces, pts_hash, topo) in cases {
        let m = dual(&fixture(dict), split, concave);
        let name = format!("{dict} split={split} concave={concave}");
        assert_eq!(m.points.len(), n_pts, "{name}: points");
        assert_eq!(m.faces.len(), n_faces, "{name}: faces");
        assert_eq!(points_hash(&m), pts_hash, "{name}: dual points differ from OpenFOAM.com v2506");
        assert_eq!(topology_hash(&m), topo, "{name}: dual faces/owner/neighbour differ");
    }
}
