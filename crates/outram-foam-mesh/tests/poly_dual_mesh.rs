// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Verification tests for the `polyDualMesh` construction (OpenFOAM-derived; see
// src/poly_dual_mesh.rs for the provenance header).
//
// ── Methodology ─────────────────────────────────────────────────────────────
// The dual of a mesh is exercised on a uniform structured hexahedral block, a
// case whose dual is analytically known:
//   * primal cells                 = nx*ny*nz
//   * primal points                = (nx+1)*(ny+1)*(nz+1)
//   * primal *interior* points     = (nx-1)*(ny-1)*(nz-1)
//   * a `polyDualMesh` makes one dual cell per primal point, one dual *point*
//     per primal cell, and one dual *face* per primal interior edge.
//   * an interior primal point (in a hex grid) has 8 surrounding cells, so its
//     dual cell is a cube of side = cell spacing and volume h^3.
//
// The checks below assert, for the dual mesh:
//   (V&V-1) closure   — every dual cell has |Σ Sf_out| ≈ 0 (closed polyhedron)
//                       and Euler characteristic V-E+F == 2 (genus 0);
//   (V&V-2) topology  — dual cell count == primal point count, and the interior
//                       subset == primal interior point count;
//   (V&V-3) geometry  — total dual volume == total primal volume (features
//                       preserved) to a tight tolerance, and each interior dual
//                       cube has the analytic volume h^3;
//   (V&V-4) feature   — with a feature angle > 90° the block's sharp edges are
//                       (incorrectly) merged, cutting the corners and *reducing*
//                       the total volume; this demonstrates the feature-angle
//                       control is live.
//   (V&V-5) FvMesh    — the emitted outram-foam-basic-lib FvMesh validates and
//                       preserves the dual volume.
//
// ── Results (2026-07-17, this crate, --release) ─────────────────────────────
//   3x3x3 block, L=3 m (h=1 m): primal V = 27 m^3.
//     feature_angle = 45° (< 90°, all block features preserved):
//       dual cells               = 64  (== (3+1)^3 primal points)
//       interior dual cells      =  8  (== (3-1)^3 primal interior points)
//       Σ dual volume            = 27.000000 m^3   (|Δ| < 1e-9 vs primal)
//       interior cube volume     =  1.000000 m^3 each (== h^3)
//       max closure residual     < 1e-12 m^2
//       Euler V-E+F              =  2 for every dual cell
//     feature_angle = 120° (> 90°, block edges/corners merged away):
//       Σ dual volume            = 25.5 m^3  (< 27; the 12 edges + 8 corners are
//                                  cut when their features are not preserved).
//                                  Asserted to be strictly below the preserved
//                                  volume. (100° and 179° also give 25.5.)
//   2x2x2 and 4x3x2 blocks pass the same closure/Euler/volume checks.

use outram_foam_mesh::poly_dual_mesh::{poly_dual_mesh, PolyMesh};

const TOL: f64 = 1e-9;

fn interior_points(nx: usize, ny: usize, nz: usize) -> usize {
    (nx - 1) * (ny - 1) * (nz - 1)
}
fn all_points(nx: usize, ny: usize, nz: usize) -> usize {
    (nx + 1) * (ny + 1) * (nz + 1)
}

/// V&V-1..3 combined on a cube block, features preserved (feature_angle < 90°).
fn check_block(nx: usize, ny: usize, nz: usize, lx: f64, ly: f64, lz: f64) {
    let primal = PolyMesh::structured_hex_block(nx, ny, nz, lx, ly, lz);
    let primal_vol = primal.total_volume();
    assert!(
        (primal_vol - lx * ly * lz).abs() < TOL,
        "primal block volume wrong: {primal_vol} vs {}",
        lx * ly * lz
    );

    let dual = poly_dual_mesh(&primal, 45.0).expect("dual construction");

    // (V&V-2) topology
    assert_eq!(
        dual.n_cells,
        all_points(nx, ny, nz),
        "dual cell count must equal primal point count"
    );
    assert_eq!(
        dual.n_interior_cells(),
        interior_points(nx, ny, nz),
        "interior dual cells must equal primal interior points"
    );

    // (V&V-1) closure + Euler
    let res = dual.max_closure_residual();
    assert!(res < 1e-9, "dual cells not closed: max |Σ Sf| = {res}");
    if let Some((c, chi)) = dual.first_bad_euler() {
        panic!("dual cell {c} has Euler characteristic {chi} (expected 2)");
    }

    // (V&V-3) volume preservation
    let dual_vol = dual.total_volume();
    assert!(
        (dual_vol - primal_vol).abs() < 1e-7 * primal_vol.max(1.0),
        "dual volume {dual_vol} != primal volume {primal_vol}"
    );
}

#[test]
fn dual_of_3x3x3_block_is_valid_and_volume_preserving() {
    check_block(3, 3, 3, 3.0, 3.0, 3.0);
}

#[test]
fn dual_of_2x2x2_block_is_valid() {
    check_block(2, 2, 2, 2.0, 2.0, 2.0);
}

#[test]
fn dual_of_non_cubic_block_is_valid() {
    check_block(4, 3, 2, 8.0, 3.0, 2.0);
}

/// (V&V-3, analytic) interior dual cells of a unit-spacing block are unit cubes.
#[test]
fn interior_dual_cells_are_unit_cubes() {
    let nx = 4;
    let primal = PolyMesh::structured_hex_block(nx, nx, nx, nx as f64, nx as f64, nx as f64); // h = 1
    let dual = poly_dual_mesh(&primal, 45.0).expect("dual");
    let fv = dual
        .to_fv_mesh(&(0..6).map(|i| format!("p{i}")).collect::<Vec<_>>())
        .expect("fvmesh");
    // Interior dual cells should each have volume h^3 = 1.
    let mut n_unit = 0;
    for c in 0..dual.n_cells {
        if dual.cell_is_interior[c] {
            assert!(
                (fv.cell_volumes[c] - 1.0).abs() < 1e-9,
                "interior dual cell {c} volume {} != 1",
                fv.cell_volumes[c]
            );
            n_unit += 1;
        }
    }
    assert_eq!(n_unit, interior_points(nx, nx, nx));
}

/// (V&V-4) feature-angle control: merging away the block's 90° edges cuts the
/// corners and reduces the enclosed volume.
#[test]
fn large_feature_angle_cuts_corners_and_loses_volume() {
    let primal = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let preserved = poly_dual_mesh(&primal, 45.0).expect("dual45").total_volume();
    let merged = poly_dual_mesh(&primal, 120.0).expect("dual120").total_volume();
    assert!(
        (preserved - 27.0).abs() < 1e-7,
        "preserved volume should be 27, got {preserved}"
    );
    assert!(
        merged < preserved - 1e-3,
        "merging features (angle 120°) should reduce volume: merged={merged}, preserved={preserved}"
    );
    // Sanity: still positive and not absurd.
    assert!(merged > 0.0 && merged < preserved);
}

/// (V&V-5) The emitted FvMesh validates and preserves the dual volume.
#[test]
fn emitted_fvmesh_is_valid_and_conserves_volume() {
    let primal = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let dual = poly_dual_mesh(&primal, 45.0).expect("dual");
    let names: Vec<String> = ["xMin", "xMax", "yMin", "yMax", "zMin", "zMax"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let fv = dual.to_fv_mesh(&names).expect("fvmesh build+validate");

    assert_eq!(fv.n_cells, all_points(3, 3, 3));
    // FvMesh::validate already ran inside build(); re-assert for clarity.
    fv.validate().expect("fvmesh validate");

    let fv_vol: f64 = fv.cell_volumes.iter().sum();
    assert!(
        (fv_vol - 27.0).abs() < 1e-7,
        "FvMesh total volume {fv_vol} != 27"
    );
    // Boundary faces present and grouped into patches.
    assert!(fv.n_boundary_faces() > 0);
    assert_eq!(fv.n_patches(), 6);
}
