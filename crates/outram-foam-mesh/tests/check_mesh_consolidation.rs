// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the GPL-3.0 notice in `src/lib.rs`.

//! gh:#1480 V&V: consolidating the three copies of OpenFOAM's checkMesh into
//! `outram_foam_mesh::check_mesh` changed no number.
//!
//! **Methodology.** The numbers `mesh_quality::assess_quality` and
//! `PolyPatchMesh::quality` returned from the *pre-consolidation* code
//! (commit `f05ac6013`, release, x86_64) were recorded as `f64::to_bits`
//! below on four meshes: the castellated and the snapped unit-sphere meshes
//! (8x8x8 background, level 2), each as built and with every point displaced
//! by a deterministic pseudo-random amount (so skewness, boundary skewness
//! and non-orthogonality are all non-trivial). After the callers were
//! switched to `check_mesh` (options `CheckOptions::foam_mesh_legacy()`) the
//! same quantities are asserted to be **bit-identical**.
//!
//! **Prediction (written before the first run of the new code).** Bit-identical
//! on all four meshes: the options reproduce each old formula operation for
//! operation, and the geometry is still supplied by the same providers.
//!
//! **Results.** See `verification_and_validation/check_mesh_1480/README.md`.

use outram_foam_mesh::mesh_quality::assess_quality;
use outram_foam_mesh::snappy_hex_mesh::{
    background::{BackgroundMesh, Bounds},
    castellate,
    castellation::CastellationControls,
    snapping::{snap, SnapControls},
    stl::TriangleSoup,
    PolyPatchMesh,
};
use outram_foam_basic_lib::primitives::Vector3;

/// Deterministic displacement of every point by up to `amp` per component
/// (LCG, wrapping arithmetic on purpose).
fn jitter(topo: &PolyPatchMesh, amp: f64) -> PolyPatchMesh {
    let mut t = topo.clone();
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2.0 * amp
    };
    for p in t.points.iter_mut() {
        *p = Vector3::new(p.x + next(), p.y + next(), p.z + next());
    }
    t
}

/// The recorded quantities of one mesh, as bits.
fn fingerprint(topo: &PolyPatchMesh) -> Vec<u64> {
    let q = topo.quality();
    let r = assess_quality(&topo.to_foam_poly_mesh());
    vec![
        q.max_non_ortho_deg.to_bits(),
        q.max_skewness.to_bits(),
        q.min_cell_volume.to_bits(),
        q.n_negative_volume_cells as u64,
        r.total_volume.to_bits(),
        r.min_cell_volume.to_bits(),
        r.max_cell_volume.to_bits(),
        r.n_negative_volume_cells as u64,
        r.max_non_ortho_deg.to_bits(),
        r.mean_non_ortho_deg.to_bits(),
        r.n_severely_non_ortho_faces as u64,
        r.max_skewness.to_bits(),
        r.max_boundary_skewness.to_bits(),
        r.max_aspect_ratio.to_bits(),
        r.mean_aspect_ratio.to_bits(),
    ]
}

#[test]
fn check_mesh_consolidation_is_bit_identical() {
    let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
    let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
    let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
    let controls = CastellationControls::new(bg, 2, Vector3::new(-1.9, -1.9, -1.9));
    let cast = castellate(&surface, &controls).expect("castellation succeeds");
    let snapped = snap(&cast, &surface, &SnapControls::default()).expect("snap succeeds");

    let got: Vec<Vec<u64>> = vec![
        fingerprint(&cast.topology),
        fingerprint(&jitter(&cast.topology, 0.01)),
        fingerprint(&snapped.topology),
        fingerprint(&jitter(&snapped.topology, 0.01)),
    ];
    for (i, g) in got.iter().enumerate() {
        println!("FINGERPRINT {i}: {g:?}");
    }
    let golden: Vec<Vec<u64>> = GOLDEN.iter().map(|r| r.to_vec()).collect();
    if golden.is_empty() {
        panic!("golden not recorded yet; copy the FINGERPRINT lines above");
    }
    assert_eq!(got, golden, "checkMesh consolidation moved a number");
}

/// Recorded from the pre-consolidation code (see the module docs).
const GOLDEN: &[[u64; 15]] = &[
    [4630870725461926740, 4603579539098121020, 4566650022153682944, 0, 4633605882238730240, 4566650022153682944, 4593671619917905921, 0, 4630870725461926739, 4620984120952070689, 0, 4603579539098121021, 0, 4607182418800017409, 4607182418800017408],
    [4631769194255498751, 4604609797187169248, 4565165687359455681, 0, 4633609514943814411, 4565165687359455681, 4593857074292446589, 0, 4631769194255498752, 4621961914770878120, 0, 4604609797187169251, 0, 4607720791276330784, 4607461772302342007],
    [4631099029776639508, 4604519433029593308, 4557934330492182302, 0, 4633625882928107340, 4557934330492182302, 4593671619917905921, 0, 4631099029776639511, 4623189588032193336, 0, 4604519433029593314, 0, 4611247260216717816, 4607958705872027145],
    [4631750130471070640, 4605372853515770281, 4555815530652897019, 0, 4633629485892493040, 4555815530652897020, 4593857074292446589, 0, 4631750130471070639, 4623607761324907480, 0, 4605372853515770274, 0, 4612379904213361041, 4608104599563170273],
];
