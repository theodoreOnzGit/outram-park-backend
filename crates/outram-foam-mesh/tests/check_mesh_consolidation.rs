// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the GPL-3.0 notice in `src/lib.rs`.

//! gh:#1480 V&V: consolidating the three copies of OpenFOAM's checkMesh into
//! `outram_foam_mesh::check_mesh` changed no number.
//!
//! **Methodology.** `mesh_quality::assess_quality` and
//! `PolyPatchMesh::quality` were run on the *pre-consolidation* code (commit
//! `27df6f6c9`, release, x86_64) on two fully deterministic meshes: a
//! 6x5x4 hexahedral grid whose points are displaced by up to 0.1 and 0.25
//! cell widths (splitmix64 keyed on the grid index, so skewness,
//! non-orthogonality and non-planar faces are all non-trivial). Every
//! reported quantity was recorded as `f64::to_bits` in `GOLDEN`. After the
//! callers were switched to `check_mesh` (`CheckOptions::foam_mesh_legacy()`),
//! the same quantities are asserted **bit-identical**.
//!
//! The meshes are built by hand rather than by castellation because
//! castellation numbers points and faces in `HashMap` order (gh:#1004): even
//! the *unchanged* code gave ulp-different sums run to run on it (measured:
//! three runs of the old code, `mean_non_ortho_deg` differed in the last bit),
//! which makes a bitwise comparison on those meshes meaningless.
//!
//! **Prediction (written before the first run of the new code).**
//! Bit-identical on both meshes: the options reproduce each old formula
//! operation for operation, and the geometry providers are untouched.
//!
//! **Results.** See `verification_and_validation/check_mesh_1480/README.md`.

use outram_foam_basic_lib::io::{MeshFace, PolyMesh};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::mesh_quality::assess_quality;
use outram_foam_mesh::snappy_hex_mesh::PolyPatchMesh;

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// An `nx x ny x nz` hex grid (unit cells), OpenFOAM face order (internal
/// faces first), every point displaced by up to `amp` per component keyed on
/// its grid index.
fn jittered_grid(nx: usize, ny: usize, nz: usize, amp: f64) -> PolyMesh {
    let pid = |i: usize, j: usize, k: usize| (i * (ny + 1) + j) * (nz + 1) + k;
    let cid = |i: usize, j: usize, k: usize| (i * ny + j) * nz + k;
    let unit = |h: u64| ((h >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2.0 * amp;
    let mut points = Vec::new();
    for i in 0..=nx {
        for j in 0..=ny {
            for k in 0..=nz {
                let key = mix(pid(i, j, k) as u64);
                points.push(Vector3::new(
                    i as f64 + unit(mix(key)),
                    j as f64 + unit(mix(key ^ 1)),
                    k as f64 + unit(mix(key ^ 2)),
                ));
            }
        }
    }
    // Quads wound outward from the lower-index (owner) cell.
    let xf = |i: usize, j: usize, k: usize| vec![pid(i, j, k), pid(i, j + 1, k), pid(i, j + 1, k + 1), pid(i, j, k + 1)];
    let yf = |i: usize, j: usize, k: usize| vec![pid(i, j, k), pid(i, j, k + 1), pid(i + 1, j, k + 1), pid(i + 1, j, k)];
    let zf = |i: usize, j: usize, k: usize| vec![pid(i, j, k), pid(i + 1, j, k), pid(i + 1, j + 1, k), pid(i, j + 1, k)];
    let rev = |mut v: Vec<usize>| {
        v.reverse();
        v
    };
    let mut internal = Vec::new();
    let mut boundary = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let c = cid(i, j, k);
                if i + 1 < nx {
                    internal.push(MeshFace { verts: xf(i + 1, j, k), owner: c, neighbour: Some(cid(i + 1, j, k)) });
                } else {
                    boundary.push(MeshFace { verts: xf(i + 1, j, k), owner: c, neighbour: None });
                }
                if j + 1 < ny {
                    internal.push(MeshFace { verts: rev(yf(i, j + 1, k)), owner: c, neighbour: Some(cid(i, j + 1, k)) });
                } else {
                    boundary.push(MeshFace { verts: rev(yf(i, j + 1, k)), owner: c, neighbour: None });
                }
                if k + 1 < nz {
                    internal.push(MeshFace { verts: zf(i, j, k + 1), owner: c, neighbour: Some(cid(i, j, k + 1)) });
                } else {
                    boundary.push(MeshFace { verts: zf(i, j, k + 1), owner: c, neighbour: None });
                }
                if i == 0 {
                    boundary.push(MeshFace { verts: rev(xf(i, j, k)), owner: c, neighbour: None });
                }
                if j == 0 {
                    boundary.push(MeshFace { verts: yf(i, j, k), owner: c, neighbour: None });
                }
                if k == 0 {
                    boundary.push(MeshFace { verts: rev(zf(i, j, k)), owner: c, neighbour: None });
                }
            }
        }
    }
    let n_internal_faces = internal.len();
    let mut faces = internal;
    faces.extend(boundary);
    PolyMesh { points, faces, n_internal_faces, n_cells: nx * ny * nz, patches: vec![] }
}

/// The same mesh as a `PolyPatchMesh`.
fn as_topology(m: &PolyMesh) -> PolyPatchMesh {
    PolyPatchMesh {
        points: m.points.clone(),
        faces: m.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: m.faces.iter().map(|f| f.owner).collect(),
        neighbour: m.faces[..m.n_internal_faces].iter().map(|f| f.neighbour.unwrap()).collect(),
        n_internal_faces: m.n_internal_faces,
        n_cells: m.n_cells,
        patches: vec![],
    }
}

/// The recorded quantities of one mesh, as bits.
fn fingerprint(m: &PolyMesh) -> Vec<u64> {
    let q = as_topology(m).quality();
    let r = assess_quality(m);
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
    let got: Vec<Vec<u64>> = vec![
        fingerprint(&jittered_grid(6, 5, 4, 0.1)),
        fingerprint(&jittered_grid(6, 5, 4, 0.25)),
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
    [4640530293357096275, 4592195435802148369, 4598891050049060411, 0, 4630823016642702415, 4598891050049060412, 4600531591143900104, 0, 4640530293357096340, 4633966143174668569, 96, 4592195435802148173, 0, 4612524342856439624, 4612299538080947449],
    [4640530202927595516, 4598725284036278017, 4597320068721917155, 0, 4630818735654090567, 4597320068721917165, 4601921444732207905, 0, 4640530202927595645, 4634292697182472994, 96, 4598725284036278119, 0, 4613913500488831879, 4612985407370224963],
];
