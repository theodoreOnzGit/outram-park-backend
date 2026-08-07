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
    let preserved = poly_dual_mesh(&primal, 45.0)
        .expect("dual45")
        .total_volume();
    let merged = poly_dual_mesh(&primal, 120.0)
        .expect("dual120")
        .total_volume();
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

/// **Methodology (V&V-6).** The dual mesh is converted with the new
/// `DualMesh::to_foam_poly_mesh` and graded with `assess_quality`, on uniform
/// `n×n×n` blocks of unit cell spacing. The dual of a *uniform* hexahedral grid
/// is itself a Cartesian grid — its cells are the cubes, half-slabs, quarter-
/// bars and eighth-corners around each primal point — so every metric has an
/// exact closed form:
///
/// * every dual face normal is axis-aligned and parallel to the line joining
///   the two dual cell centres ⇒ **non-orthogonality exactly 0°**, mean 0°;
/// * every dual face centre lies on that line ⇒ **skewness exactly 0**;
/// * cell volumes are `1`, `1/2`, `1/4`, `1/8` m³ ⇒ `min = 0.125`, `max = 1`;
/// * the worst aspect ratio is the face-slab cell (`0.5 × 1 × 1`):
///   `AR = (1/3)(0.5 + 1 + 0.5)/0.5^(2/3) = 2/(3·0.5^(2/3)) = 1.05827`;
/// * total dual volume equals the primal volume (`n³` m³ for these blocks).
///
/// **Results (measured 2026-08-07, release, x86_64).** `3×3×3`: 64 dual cells,
/// 221 points, 288 faces (144 internal); `max_non_ortho = 0.000°`,
/// `max_skewness = 0`, `max_aspect_ratio = 1.0583` (closed form `1.058267…`,
/// agreement `< 1e-12`), `min/max cell volume = 0.125 / 1.000 m³`,
/// `total_volume = 27.000 m³`, 0 inverted cells, verdict `GOOD`. `4×4×4`: 125
/// cells, `total_volume = 64.000 m³`, all other metrics identical.
///
/// **Interpretation, worth stating plainly:** dualisation does *not* by itself
/// introduce non-orthogonality. The dual of a good hex mesh is a good hex mesh.
/// A polyhedral dual that measures 80°+ inherited that from its primal — a
/// tetrahedral or highly irregular primal — not from the dual construction.
#[test]
fn dual_quality_is_exactly_orthogonal_on_a_uniform_block() {
    for n in [3usize, 4] {
        let primal = PolyMesh::structured_hex_block(n, n, n, n as f64, n as f64, n as f64);
        let dual = poly_dual_mesh(&primal, 45.0).unwrap();
        let names: Vec<String> = (0..6).map(|i| format!("p{i}")).collect();
        let pm = dual
            .to_foam_poly_mesh(&names)
            .expect("dual converts to a polyMesh");
        let q = outram_foam_mesh::assess_quality(&pm);
        eprintln!("dual of {n}^3 block:\n{}", q.summary());

        assert_eq!(q.n_cells, all_points(n, n, n));
        assert_eq!(q.n_negative_volume_cells, 0);
        assert!(
            q.max_non_ortho_deg < 1e-12,
            "the dual of a uniform grid must be orthogonal: {} deg",
            q.max_non_ortho_deg
        );
        assert!(q.mean_non_ortho_deg < 1e-12);
        assert!(q.max_skewness < 1e-12, "skew {}", q.max_skewness);

        let expected_ar = 2.0 / (3.0 * 0.5f64.powf(2.0 / 3.0));
        assert!(
            (q.max_aspect_ratio - expected_ar).abs() < 1e-12,
            "AR {} vs closed form {expected_ar}",
            q.max_aspect_ratio
        );
        assert!((q.min_cell_volume - 0.125).abs() < TOL);
        assert!((q.max_cell_volume - 1.0).abs() < TOL);
        assert!(
            (q.total_volume - (n * n * n) as f64).abs() < TOL,
            "dual volume {} vs primal {}",
            q.total_volume,
            n * n * n
        );
    }
}
