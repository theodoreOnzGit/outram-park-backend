// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **Run-to-run determinism of the mesh operators (gh #1002).**
//!
//! # Methodology
//!
//! `std::collections::HashMap` and `HashSet` use `RandomState`: every map gets
//! its own SipHash keys, drawn from a per-process random seed and stepped for
//! each new map in a thread. Iterating one therefore visits its entries in an
//! order that differs from map to map, so it differs between two calls in one
//! process as well as between processes. Before gh #1002 several operators
//! summed floating-point values, numbered new vertices, or emitted faces in
//! that order (ARAP's one-ring covariances, the Laplacian diagonals, decimate's
//! boundary quadrics, Loop subdivision's neighbour sums, cap and rim faces).
//!
//! This test computes a 64-bit FNV-1a digest of each operator's output (every
//! position's bit pattern, every face's vertex list) and checks it three ways:
//!
//! 1. **in process, twice**: each call builds fresh hash maps with fresh keys,
//!    so the old code fails here on its own;
//! 2. **in two child processes** (this test binary re-run with
//!    `OUTRAM_BLENDER_DETERMINISM_CHILD` set, which prints the digests instead
//!    of asserting): each process draws a new random seed, which covers any
//!    order that is fixed within a process but not across processes;
//! 3. **pinned**: the ARAP bar case's output digest and energy are compared
//!    against bit patterns recorded on 2026-10-20, so a later change of
//!    summation order is caught too, not only nondeterminism. The ARAP path
//!    uses only `+ - * /`, `sqrt`, `mul_add` and PETIR's own `hypot` port, all
//!    exactly specified, so the pin is not expected to depend on the platform.
//!
//! Pass criterion: every digest identical across the two in-process calls and
//! the two child processes, and the pinned values bit-identical.
//!
//! # Results (2026-10-20, x86-64 Linux)
//!
//! **Negative control**: this file run against the pre-gh #1002 sources fails
//! at check 1 on its first operator (`arap_deform differs between two calls
//! in one process`). A scratch harness with the same digests (six calls per
//! operator in each of seven processes, old code) found almost every call
//! distinct for every operator listed here except `parameterize`, which was
//! stable on the regular grid it used (exact sums); e.g. `arap_deform` 11 of
//! 11 calls, `decimate` 6 of 6, `merge_edges` 3 of 6. `parameterize` stays
//! in as a guard, since its sums are order-sensitive on irregular meshes.
//! After the fix every operator gives one digest across all calls and
//! processes, and this test passes. Moved numbers and timings: gh #1002.

use std::process::Command;

use outram_blender::arap::{arap_deform, arap_energy};
use outram_blender::laplacian::{
    laplacian_smooth, laplacian_triplets, taubin_smooth, LaplacianWeighting,
};
use outram_blender::math::Vec3;
use outram_blender::mesh::{EdgeId, FaceId, Mesh, VertexId};
use outram_blender::parameterize::{parameterize, BoundaryShape};
use outram_blender::*;

const CHILD_ENV: &str = "OUTRAM_BLENDER_DETERMINISM_CHILD";
const TEST_NAME: &str = "mesh_operators_are_identical_across_calls_and_processes";

/// FNV-1a over the bytes of `x`.
fn fnv(h: &mut u64, x: u64) {
    for b in x.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

fn digest_points(h: &mut u64, ps: &[Vec3]) {
    for p in ps {
        fnv(h, p.x.to_bits());
        fnv(h, p.y.to_bits());
        fnv(h, p.z.to_bits());
    }
}

/// Digest of a mesh: positions by bit pattern, then every face's vertex list.
fn digest_mesh(m: &Mesh) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325;
    digest_points(&mut h, &m.positions());
    for f in m.polygons() {
        fnv(&mut h, f.len() as u64);
        for v in f {
            fnv(&mut h, v.0 as u64);
        }
    }
    h
}

/// The ARAP bar of the crate's `arap_bar_bends_without_stretching...` test:
/// `grid(16, 2, 4.0)`, the `x = -2` column pinned, the `x = +2` column lifted
/// by `+1.5 z`.
fn bar_and_handles() -> (Mesh, Vec<(VertexId, Vec3)>) {
    let bar = primitives::grid(16, 2, 4.0);
    let ps = bar.positions();
    let mut handles = Vec::new();
    for (i, p) in ps.iter().enumerate() {
        if (p.x + 2.0).abs() < 1e-9 {
            handles.push((VertexId(i), *p));
        } else if (p.x - 2.0).abs() < 1e-9 {
            handles.push((VertexId(i), Vec3::new(p.x, p.y, p.z + 1.5)));
        }
    }
    (bar, handles)
}

/// A unit icosphere with a fixed radial perturbation, so cotangent weights are
/// irregular and sums are order-sensitive.
fn noisy_sphere() -> Mesh {
    let s = primitives_extra::icosphere(3, 1.0);
    let mut seed = 88_172_645_463_325_252u64;
    let pos: Vec<Vec3> = s
        .positions()
        .iter()
        .map(|p| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let r = 1.0 + 0.02 * ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5);
            p.scale(r)
        })
        .collect();
    let faces: Vec<Vec<usize>> = s
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();
    Mesh::from_polygons(&pos, &faces)
}

/// `(operator, digest)` for every operator whose output order or sums used
/// to follow a hash map's iteration order.
fn digests() -> Vec<(&'static str, u64)> {
    let (bar, handles) = bar_and_handles();
    let sphere = noisy_sphere();
    let ico = primitives_extra::icosphere(2, 1.0);
    let grid = primitives::grid(6, 6, 2.0);
    let gridt = triangulate::triangulate(&grid);
    let cube = primitives::cube(2.0);
    let uvs = primitives::uv_sphere(12, 8, 1.0);
    let holed = dissolve::delete(
        &uvs,
        dissolve::DeleteMode::Faces,
        &[],
        &[],
        &[FaceId(3), FaceId(40)],
    );

    let mut out = Vec::new();
    let arap = arap_deform(&bar, &handles, 30).expect("arap solves");
    out.push(("arap_deform", digest_mesh(&arap)));
    out.push((
        "arap_energy",
        arap_energy(&bar, &arap.positions()).to_bits(),
    ));
    let smooth =
        laplacian_smooth(&sphere, LaplacianWeighting::Cotangent, 0.5, 2).expect("smoothing solves");
    out.push(("laplacian_smooth", digest_mesh(&smooth)));
    out.push((
        "taubin_smooth",
        digest_mesh(&taubin_smooth(
            &sphere,
            LaplacianWeighting::Cotangent,
            0.5,
            -0.53,
            5,
        )),
    ));
    let (_, mut trips) = laplacian_triplets(&sphere, LaplacianWeighting::Cotangent);
    // Sorted so only the values, not the triplet order, are compared here.
    trips.sort_by_key(|&(i, j, _)| (i, j));
    let mut h = 0xcbf2_9ce4_8422_2325;
    for (i, j, w) in trips {
        fnv(&mut h, i as u64);
        fnv(&mut h, j as u64);
        fnv(&mut h, w.to_bits());
    }
    out.push(("laplacian_triplets", h));
    let disk = primitives::grid(8, 8, 2.0);
    let uv = parameterize(&disk, LaplacianWeighting::Cotangent, BoundaryShape::Circle)
        .expect("parameterization solves");
    let mut h = 0xcbf2_9ce4_8422_2325;
    for (u, v) in uv {
        fnv(&mut h, u.to_bits());
        fnv(&mut h, v.to_bits());
    }
    out.push(("parameterize", h));

    out.push(("decimate", digest_mesh(&decimate::decimate(&ico, 60))));
    out.push((
        "decimate_open",
        digest_mesh(&decimate::decimate(&gridt, 20)),
    ));
    out.push((
        "loop_subdivide",
        digest_mesh(&loop_subdivision::loop_subdivide(&gridt, 1)),
    ));
    out.push((
        "bevel",
        digest_mesh(&bevel::bevel(
            &cube,
            bevel::BevelOptions {
                segments: 2,
                ..Default::default()
            },
        )),
    ));
    out.push((
        "bevel_edges",
        digest_mesh(&edge_bevel::bevel_edges(&cube, 0.1)),
    ));
    out.push((
        "convex_hull",
        digest_mesh(&convex_hull::convex_hull(&uvs.positions()).expect("hull")),
    ));
    out.push(("fill_holes", digest_mesh(&fill_holes::fill_holes(&holed))));
    out.push((
        "tris_to_quads",
        digest_mesh(&poke_quads::tris_to_quads(&gridt, 1.0)),
    ));
    out.push((
        "split_faces",
        digest_mesh(&rip_split::split_faces(
            &grid,
            &[FaceId(7), FaceId(8), FaceId(14)],
        )),
    ));
    let mut h = 0u64;
    for part in
        rip_split::separate_loose_parts(&rip_split::split_faces(&grid, &[FaceId(7), FaceId(30)]))
    {
        h = h.rotate_left(7) ^ digest_mesh(&part);
    }
    out.push(("separate_loose_parts", h));
    out.push(("solidify", digest_mesh(&solidify::solidify(&grid, 0.1))));
    out.push((
        "extrude_faces_along_normals",
        digest_mesh(&extrude::extrude_faces_along_normals(
            &uvs,
            &[FaceId(5), FaceId(6), FaceId(17)],
            0.2,
        )),
    ));
    out.push((
        "dissolve_faces",
        digest_mesh(&dissolve::dissolve_faces(
            &grid,
            &[FaceId(7), FaceId(8), FaceId(13), FaceId(14)],
        )),
    ));
    out.push((
        "dissolve_edges",
        digest_mesh(&dissolve::dissolve_edges(
            &grid,
            &[EdgeId(10), EdgeId(30), EdgeId(31)],
        )),
    ));
    out.push((
        "bevel_vertices",
        digest_mesh(&ops::bevel_vertices(&cube, 0.2)),
    ));
    out.push((
        "bevel_vertices_rounded",
        digest_mesh(&ops::bevel_vertices_rounded(&cube, 0.2, 3)),
    ));
    out.push((
        "merge_edges",
        digest_mesh(&merge::merge_edges(
            &grid,
            &[EdgeId(10), EdgeId(11), EdgeId(12), EdgeId(40)],
        )),
    ));
    out
}

fn format(ds: &[(&str, u64)]) -> String {
    ds.iter()
        .map(|(n, d)| format!("{n}={d:016x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// See the module docs. The child mode prints one `DIGESTS ...` line.
#[test]
fn mesh_operators_are_identical_across_calls_and_processes() {
    let first = digests();
    if std::env::var_os(CHILD_ENV).is_some() {
        println!("DIGESTS {}", format(&first));
        return;
    }

    // 1. Twice in this process: fresh maps, fresh hash keys.
    let second = digests();
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(a, b, "{} differs between two calls in one process", a.0);
    }

    // 2. In two child processes, each with its own random hash seed.
    let want = format(&first);
    let exe = std::env::current_exe().expect("test binary path");
    for run in 0..2 {
        let out = Command::new(&exe)
            .args([TEST_NAME, "--exact", "--nocapture", "--test-threads=1"])
            .env(CHILD_ENV, "1")
            .output()
            .expect("re-run the test binary");
        assert!(out.status.success(), "child run {run} failed");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let got = stdout
            .lines()
            // libtest may print `test <name> ... ` on the same line first.
            .find_map(|l| l.split_once("DIGESTS ").map(|(_, d)| d.trim_end()))
            .expect("child printed its digests");
        assert_eq!(got, want, "child process {run} gave different digests");
    }

    // 3. Pinned ARAP bar result (bit patterns recorded 2026-10-20).
    let get = |name: &str| first.iter().find(|(n, _)| *n == name).unwrap().1;
    assert_eq!(
        get("arap_deform"),
        PINNED_ARAP_DIGEST,
        "ARAP bar positions moved"
    );
    assert_eq!(
        f64::from_bits(get("arap_energy")).to_bits(),
        PINNED_ARAP_ENERGY_BITS,
        "ARAP bar energy moved: {:e}",
        f64::from_bits(get("arap_energy"))
    );
}

/// `digest_mesh` of the ARAP bar after 30 iterations, 2026-10-20.
const PINNED_ARAP_DIGEST: u64 = 0xef6f_a80b_6661_76ae;
/// `arap_energy` of that result as bits, 2026-10-20.
const PINNED_ARAP_ENERGY_BITS: u64 = 0x3fc3_05f6_e62d_a09e; // 1.4861952056094635e-1
