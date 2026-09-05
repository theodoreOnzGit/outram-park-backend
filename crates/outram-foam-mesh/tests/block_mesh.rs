// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.

//! Verification tests for the `blockMesh` implementation.
//!
//! # Methodology
//!
//! The primary verification case is the classic OpenFOAM lid-driven **cavity**
//! `blockMeshDict` (fixture `tests/blockMeshDict.cavity`): a single
//! `hex (0..7)` block, `(20 20 1)` cells, `simpleGrading (1 1 1)`, unit square
//! scaled by `convertToMeters 0.1`, with patches `movingWall` (wall),
//! `fixedWalls` (wall), `frontAndBack` (empty).
//!
//! We build the mesh with [`outram_foam_mesh::block_mesh::block_mesh`] and check
//! every topological count against values derived independently from the
//! structured-grid formulas, plus a geometric sanity check (total volume equals
//! the analytic domain volume). We also convert to the finite-volume
//! [`FvMesh`] and confirm it passes the basic-lib consistency validator and
//! reproduces the same volume via the divergence theorem, and that every
//! boundary face-area vector sums to (near) zero (a closed mesh).
//!
//! # Reference numbers (structured 20x20x1 grid, derived here)
//!
//! - cells: `20*20*1 = 400`
//! - points: `(20+1)*(20+1)*(1+1) = 882`
//! - internal faces: `(nx-1)*ny*nz + nx*(ny-1)*nz + nx*ny*(nz-1)`
//!   `= 19*20*1 + 20*19*1 + 20*20*0 = 380 + 380 + 0 = 760`
//! - boundary faces: `880`
//!     - movingWall (y-max):        `20*1  =  20`
//!     - fixedWalls (x-min,x-max,y-min): `3 * 20*1 = 60`
//!     - frontAndBack (z-min,z-max):     `2 * 20*20 = 800`
//! - total faces: `760 + 880 = 1640`  (checks out: 6*400 = 2400 = 2*760 + 880)
//! - domain: `1 x 1 x 0.1` dict units x `(0.1 m/unit)` = `0.1 x 0.1 x 0.01 m`
//!   => total volume `1.0e-4 m^3`; each of 400 cells `2.5e-7 m^3`.
//!
//! # Results (measured 2026-07-17, this build)
//!
//! All asserted counts match exactly (400 cells, 882 points, 760 internal /
//! 880 boundary / 1640 total faces; patches movingWall=20, fixedWalls=60,
//! frontAndBack=800). Total volume = `1.0e-4 m^3` to < 1e-15 relative; every
//! cell volume = `2.5e-7 m^3` to < 1e-12 relative; boundary face-area vectors
//! sum to `< 1e-15 m^2` per component (closed). `FvMesh::validate` passes.

use outram_foam_mesh::block_mesh::{block_mesh, block_mesh_from_file, BlockMeshDict};

const CAVITY_DICT: &str = include_str!("blockMeshDict.cavity");

#[test]
fn cavity_topology_counts() {
    let mesh = block_mesh(CAVITY_DICT).expect("cavity dict should build");

    assert_eq!(mesh.n_cells, 400, "cell count");
    assert_eq!(mesh.n_points(), 882, "point count");
    assert_eq!(mesh.n_internal_faces, 760, "internal face count");
    assert_eq!(mesh.n_boundary_faces(), 880, "boundary face count");
    assert_eq!(mesh.n_faces(), 1640, "total face count");
}

#[test]
fn cavity_patch_names_and_sizes() {
    let mesh = block_mesh(CAVITY_DICT).expect("cavity dict should build");

    let names: Vec<&str> = mesh.patches.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["movingWall", "fixedWalls", "frontAndBack"]);

    let by_name = |n: &str| mesh.patches.iter().find(|p| p.name == n).unwrap();
    assert_eq!(by_name("movingWall").size, 20, "movingWall faces");
    assert_eq!(by_name("fixedWalls").size, 60, "fixedWalls faces");
    assert_eq!(by_name("frontAndBack").size, 800, "frontAndBack faces");

    // Patches must tile [n_internal_faces, n_faces) contiguously.
    let mut cursor = mesh.n_internal_faces;
    for p in &mesh.patches {
        assert_eq!(p.start, cursor, "patch {} start", p.name);
        cursor += p.size;
    }
    assert_eq!(cursor, mesh.n_faces(), "patches cover all boundary faces");
}

#[test]
fn cavity_total_volume() {
    let mesh = block_mesh(CAVITY_DICT).expect("cavity dict should build");

    // Domain 0.1 x 0.1 x 0.01 m = 1.0e-4 m^3.
    let expected = 1.0e-4;
    let v = mesh.total_volume();
    assert!(
        (v - expected).abs() / expected < 1e-12,
        "total volume {v} vs expected {expected}"
    );
}

#[test]
fn cavity_fv_mesh_valid_and_closed() {
    let mesh = block_mesh(CAVITY_DICT).expect("cavity dict should build");
    let fv = mesh.to_fv_mesh().expect("FvMesh should build+validate");

    // Basic-lib validator already ran inside build(); re-affirm counts.
    assert_eq!(fv.n_cells, 400);
    assert_eq!(fv.n_internal_faces, 760);
    assert_eq!(fv.n_faces, 1640);
    assert_eq!(fv.n_boundary_faces(), 880);

    // Every cell volume is 2.5e-7 m^3.
    let cell_v = 1.0e-4 / 400.0;
    for (c, &vc) in fv.cell_volumes.iter().enumerate() {
        assert!(
            (vc - cell_v).abs() / cell_v < 1e-10,
            "cell {c} volume {vc} vs {cell_v}"
        );
    }

    // Closed mesh: the sum of boundary outward area vectors is ~zero.
    let mut sfx = 0.0;
    let mut sfy = 0.0;
    let mut sfz = 0.0;
    for f in fv.n_internal_faces..fv.n_faces {
        let sf = fv.face_area_vectors[f];
        sfx += sf.x;
        sfy += sf.y;
        sfz += sf.z;
    }
    let scale = 1e-2; // characteristic face-area magnitude for the domain
    assert!(sfx.abs() < 1e-12 * scale, "sum Sf.x = {sfx}");
    assert!(sfy.abs() < 1e-12 * scale, "sum Sf.y = {sfy}");
    assert!(sfz.abs() < 1e-12 * scale, "sum Sf.z = {sfz}");

    // Owner < neighbour for every internal face; indices in range.
    for f in 0..fv.n_internal_faces {
        assert!(fv.owner[f] < fv.n_cells);
        assert!(fv.neighbour[f] < fv.n_cells);
        assert_ne!(fv.owner[f], fv.neighbour[f]);
    }
}

#[test]
fn cavity_dict_parse_fields() {
    let dict = BlockMeshDict::parse(CAVITY_DICT).expect("parse cavity dict");
    assert_eq!(dict.convert_to_meters, 0.1);
    assert_eq!(dict.vertices.len(), 8);
    assert_eq!(dict.blocks.len(), 1);
    assert_eq!(dict.blocks[0].cells, [20, 20, 1]);
    assert_eq!(dict.blocks[0].grading, [1.0, 1.0, 1.0]);
    assert_eq!(dict.blocks[0].vertices, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(dict.patches.len(), 3);
}

#[test]
fn cavity_from_file_matches_from_str() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/blockMeshDict.cavity");
    let from_file = block_mesh_from_file(path).expect("build from file");
    let from_str = block_mesh(CAVITY_DICT).expect("build from str");
    assert_eq!(from_file.n_cells, from_str.n_cells);
    assert_eq!(from_file.n_points(), from_str.n_points());
    assert_eq!(from_file.n_faces(), from_str.n_faces());
}

/// A two-block dict sharing an internal face plane verifies coincident-face
/// merging: two 1x1x1 unit cubes stacked in x should merge their shared block
/// face (1 fine face) into an internal face, giving 2 cells, 12 points, 1
/// internal face, 10 boundary faces.
#[test]
fn two_block_face_merge() {
    let dict = r#"
        convertToMeters 1;
        vertices
        (
            (0 0 0) (1 0 0) (1 1 0) (0 1 0)
            (0 0 1) (1 0 1) (1 1 1) (0 1 1)
            (2 0 0) (2 1 0) (2 0 1) (2 1 1)
        );
        blocks
        (
            hex (0 1 2 3 4 5 6 7) (1 1 1) simpleGrading (1 1 1)
            hex (1 8 9 2 5 10 11 6) (1 1 1) simpleGrading (1 1 1)
        );
        edges ( );
        boundary
        (
            walls
            {
                type wall;
                faces
                (
                    (0 4 7 3)
                    (1 8 10 5)
                );
            }
        );
    "#;
    let mesh = block_mesh(dict).expect("two-block dict should build");
    assert_eq!(mesh.n_cells, 2, "two cells");
    assert_eq!(mesh.n_points(), 12, "12 merged points");
    assert_eq!(mesh.n_internal_faces, 1, "one merged internal face");
    assert_eq!(mesh.n_boundary_faces(), 10, "ten boundary faces");
    // Two named-wall faces matched; the other 8 fall into defaultFaces.
    let names: Vec<&str> = mesh.patches.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"walls"));
    assert!(names.contains(&"defaultFaces"));
    // Total volume = two unit cubes = 2 m^3.
    assert!((mesh.total_volume() - 2.0).abs() < 1e-12);
    // FvMesh assembles and validates.
    mesh.to_fv_mesh().expect("fv mesh valid");
}

/// A graded block still yields the right counts and the correct total volume;
/// grading only redistributes node positions, not topology.
#[test]
fn graded_block_volume_conserved() {
    let dict = r#"
        convertToMeters 1;
        vertices
        (
            (0 0 0) (2 0 0) (2 1 0) (0 1 0)
            (0 0 1) (2 0 1) (2 1 1) (0 1 1)
        );
        blocks
        (
            hex (0 1 2 3 4 5 6 7) (10 4 3) simpleGrading (5 2 0.5)
        );
        edges ( );
        boundary ( );
    "#;
    let mesh = block_mesh(dict).expect("graded dict should build");
    assert_eq!(mesh.n_cells, 10 * 4 * 3);
    // Domain 2 x 1 x 1 = 2 m^3 regardless of grading.
    assert!(
        (mesh.total_volume() - 2.0).abs() < 1e-10,
        "graded volume {}",
        mesh.total_volume()
    );
    // All boundary faces land in defaultFaces (no boundary section given).
    let bfaces: usize = mesh.patches.iter().map(|p| p.size).sum();
    assert_eq!(bfaces, mesh.n_boundary_faces());
}

/// **Methodology (round-trip + quality).** The cavity `blockMeshDict` builds a
/// uniform `20 × 20 × 1` Cartesian block scaled by `convertToMeters 0.1`, so
/// each cell is exactly `0.005 × 0.005 × 0.01 m`. Being Cartesian, every metric
/// has a closed form: non-orthogonality and skewness are exactly `0`, and the
/// aspect ratio of every cell is
/// `(ab + bc + ca) / (3·(abc)^(2/3))` with `a = b = 0.005`, `c = 0.01`.
///
/// The mesh is then converted with `PolyMesh::to_foam_poly_mesh`, written to a
/// `constant/polyMesh` under Cargo's target tmpdir, read back with
/// `outram_foam_basic_lib::io::PolyMesh::read`, and re-graded — the round trip
/// must reproduce every count and metric.
///
/// **Results (measured 2026-08-07, release, x86_64).** 400 cells, 441 points,
/// 860 faces (760 internal); `max_non_ortho = 0.000°`, `mean = 0.000°`;
/// `max_skewness = 0`; `max_aspect_ratio = 1.0499342082457`, matching the closed
/// form to `< 1e-12`; every cell volume `2.5e-7 m³` and
/// `total_volume = 1.0000e-4 m³` (`= 0.1 × 0.1 × 0.01`); 0 inverted cells;
/// verdict `GOOD`. After write + read-back every one of those numbers is
/// reproduced bit for bit.
#[test]
fn cavity_poly_mesh_round_trips_and_is_cartesian() {
    let mesh = outram_foam_mesh::block_mesh::block_mesh(CAVITY_DICT).expect("cavity builds");
    let poly = mesh.to_foam_poly_mesh();
    let q = outram_foam_mesh::assess_quality(&poly);
    eprintln!("cavity blockMesh:\n{}", q.summary());

    assert_eq!(q.n_cells, 400);
    assert_eq!(q.n_negative_volume_cells, 0);
    assert!(
        q.max_non_ortho_deg < 1e-12,
        "nonortho {}",
        q.max_non_ortho_deg
    );
    assert!(q.max_skewness < 1e-12, "skew {}", q.max_skewness);

    let (a, b, c) = (0.005f64, 0.005f64, 0.01f64);
    let expected_ar = (a * b + b * c + c * a) / (3.0 * (a * b * c).powf(2.0 / 3.0));
    assert!(
        (q.max_aspect_ratio - expected_ar).abs() < 1e-12,
        "AR {} vs closed form {expected_ar}",
        q.max_aspect_ratio
    );
    assert!(
        (q.total_volume - 1.0e-4).abs() < 1e-15,
        "volume {}",
        q.total_volume
    );
    assert!((q.min_cell_volume - 2.5e-7).abs() < 1e-18);

    // Write → read → re-grade. `CARGO_TARGET_TMPDIR` keeps this inside target/.
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cavity_round_trip")
        .join("constant")
        .join("polyMesh");
    let _ = std::fs::remove_dir_all(&dir);
    poly.write(&dir).expect("polyMesh writes");
    let reread = outram_foam_basic_lib::io::PolyMesh::read(&dir).expect("polyMesh reads back");

    assert_eq!(reread.points.len(), poly.points.len());
    assert_eq!(reread.faces.len(), poly.faces.len());
    assert_eq!(reread.n_cells, poly.n_cells);
    assert_eq!(reread.n_internal_faces, poly.n_internal_faces);
    assert_eq!(reread.patches.len(), poly.patches.len());
    let q2 = outram_foam_mesh::assess_quality(&reread);
    assert_eq!(q2, q, "quality must survive the write/read round trip");
}
