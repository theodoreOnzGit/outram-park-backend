// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the crate source headers for the full
// GPL-3.0 notice and the OpenFOAM provenance of the converter under test.

//! Verification tests for the `ideasUnvToFoam` UNV → polyMesh converter.
//!
//! # Methodology
//!
//! Two hand-written I-DEAS Universal Files with analytically-known geometry are
//! converted and the resulting `polyMesh` is checked against counts and
//! closed-form geometry:
//!
//! 1. **`two_hex.unv`** (fixture) — two unit hexahedra (1 m cubes) sharing one
//!    face, laid out along x over the box `[0,2] × [0,1] × [0,1]`. 12 nodes,
//!    2 brick elements (descriptor 115), 2 quad surface elements (descriptor
//!    44) grouped into named patches `inlet` (the x = 0 face) and `outlet`
//!    (the x = 2 face) via a 2467 permanent-group dataset. Expected topology:
//!    12 points; 2 cells; 11 faces (6 + 6 − 1 shared); exactly 1 internal
//!    face; 10 boundary faces partitioned into `inlet` (1) + `outlet` (1) +
//!    `defaultFaces` (8). Each cell volume = 1 m³, total = 2 m³. Reference
//!    values are exact (unit cubes).
//!
//! 2. **single tetrahedron** (inline string) — the reference tet with vertices
//!    `(0,0,0), (1,0,0), (0,1,0), (0,0,1)` (descriptor 111). Expected volume
//!    `1/6 m³` (closed form), 4 boundary faces, 0 internal faces, 1 cell.
//!
//! Additional invariants checked on both meshes: `FvMesh::validate()` passes
//! (patch coverage, array lengths); the internal-face `owner < neighbour`
//! upper-triangular convention holds; and each cell's outward face-area
//! vectors sum to zero (a closed polyhedron — Gauss divergence of a constant),
//! confirming consistent outward orientation.
//!
//! # Results (2026-07-17, converter v0.1.0)
//!
//! All assertions below pass. Measured `two_hex` total volume = 2.0 m³ (exact
//! to < 1e-12); tet volume = 0.16666666666… m³ (= 1/6, to < 1e-12); per-cell
//! face-area-vector closure |Σ Sf| < 1e-12 m². These confirm the parser
//! (2411/2412/2467), owner/neighbour face merging, patch grouping, and the
//! pyramid-decomposition cell geometry are implemented correctly for the
//! hex and tet paths.

use approx::assert_abs_diff_eq;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::ideas_unv_to_foam::{convert, convert_file, UnvPolyMesh};

/// Sum the outward face-area vectors of every cell; a closed polyhedron must
/// give ~zero for each. Uses `owner`/`neighbour`: the face normal points out of
/// the owner and into the neighbour, so it is subtracted from the neighbour's sum.
fn max_cell_area_closure(m: &UnvPolyMesh) -> f64 {
    let fv = &m.fv_mesh;
    let mut sums = vec![Vector3::ZERO; fv.n_cells];
    for f in 0..fv.n_faces {
        let sf = fv.face_area_vectors[f];
        sums[fv.owner[f]] += sf;
        if fv.is_internal_face(f) {
            sums[fv.neighbour[f]] -= sf;
        }
    }
    sums.iter().map(|v| v.mag()).fold(0.0, f64::max)
}

#[test]
fn two_hex_counts_patches_and_volume() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/two_hex.unv");
    let m = convert_file(path).expect("two_hex.unv should convert");
    let fv = &m.fv_mesh;

    // ── Topology counts ──
    assert_eq!(m.points.len(), 12, "12 nodes");
    assert_eq!(fv.n_cells, 2, "2 hex cells");
    assert_eq!(fv.n_faces, 11, "6 + 6 - 1 shared face");
    assert_eq!(fv.n_internal_faces, 1, "the shared x=1 face");
    assert_eq!(fv.n_boundary_faces(), 10, "10 exterior faces");
    assert_eq!(m.faces.len(), fv.n_faces, "face list matches n_faces");
    assert_eq!(m.n_skipped_elements, 0, "no deferred elements");

    // Mesh self-consistency (patch coverage, array lengths).
    fv.validate().expect("FvMesh must validate");

    // ── Named patches ──
    assert_eq!(fv.n_patches(), 3, "inlet + outlet + defaultFaces");
    let inlet = fv.patches.iter().find(|p| p.name == "inlet").expect("inlet patch");
    let outlet = fv.patches.iter().find(|p| p.name == "outlet").expect("outlet patch");
    let deflt = fv
        .patches
        .iter()
        .find(|p| p.name == "defaultFaces")
        .expect("defaultFaces patch");
    assert_eq!(inlet.size, 1, "inlet has the single x=0 quad");
    assert_eq!(outlet.size, 1, "outlet has the single x=2 quad");
    assert_eq!(deflt.size, 8, "remaining 8 exterior faces");

    // Named patches must precede defaultFaces and cover the boundary contiguously.
    assert_eq!(inlet.start, fv.n_internal_faces);

    // ── Geometry ──
    // Each cell is a unit cube ⇒ 1 m³; total 2 m³ (exact).
    for (c, v) in fv.cell_volumes.iter().enumerate() {
        assert_abs_diff_eq!(*v, 1.0, epsilon = 1e-12);
        let _ = c;
    }
    assert_abs_diff_eq!(m.total_volume(), 2.0, epsilon = 1e-12);

    // Internal face upper-triangular convention: owner < neighbour.
    for f in 0..fv.n_internal_faces {
        assert!(fv.owner[f] < fv.neighbour[f], "owner < neighbour");
    }

    // Inlet face area = 1 m² (unit square), normal along -x (outward at x=0).
    let inlet_f = inlet.start;
    assert_abs_diff_eq!(fv.face_areas[inlet_f], 1.0, epsilon = 1e-12);
    assert!(
        fv.face_area_vectors[inlet_f].x < 0.0,
        "inlet outward normal points -x, got {:?}",
        fv.face_area_vectors[inlet_f]
    );

    // Closed-cell area closure: each cell's outward Sf sum to ~0.
    assert!(
        max_cell_area_closure(&m) < 1e-12,
        "cell area-vector closure must vanish"
    );
}

/// A single reference tetrahedron: volume 1/6 m³, 4 triangular boundary faces.
const TET_UNV: &str = "\
    -1
  2411
         1         1         1        11
   0.0000000000E+00   0.0000000000E+00   0.0000000000E+00
         2         1         1        11
   1.0000000000E+00   0.0000000000E+00   0.0000000000E+00
         3         1         1        11
   0.0000000000E+00   1.0000000000E+00   0.0000000000E+00
         4         1         1        11
   0.0000000000E+00   0.0000000000E+00   1.0000000000E+00
    -1
    -1
  2412
         1       111         1         1         1         4
         1         2         3         4
    -1
";

#[test]
fn single_tet_volume_and_faces() {
    let m = convert(TET_UNV).expect("tet UNV should convert");
    let fv = &m.fv_mesh;

    assert_eq!(m.points.len(), 4);
    assert_eq!(fv.n_cells, 1);
    assert_eq!(fv.n_faces, 4, "tet has 4 triangular faces");
    assert_eq!(fv.n_internal_faces, 0, "single cell ⇒ all faces boundary");
    assert_eq!(fv.n_boundary_faces(), 4);
    // No named groups ⇒ everything in defaultFaces.
    assert_eq!(fv.n_patches(), 1);
    assert_eq!(fv.patches[0].name, "defaultFaces");
    assert_eq!(fv.patches[0].size, 4);

    fv.validate().expect("tet FvMesh must validate");

    // Closed-form tet volume = 1/6.
    assert_abs_diff_eq!(fv.cell_volumes[0], 1.0 / 6.0, epsilon = 1e-12);
    assert_abs_diff_eq!(m.total_volume(), 1.0 / 6.0, epsilon = 1e-12);

    // Every triangular boundary face has area 0.5 except the slanted one
    // (area sqrt(3)/2); just check the closure invariant instead.
    assert!(max_cell_area_closure(&m) < 1e-12);
}

/// Millimetre-authored file converts with a metre scale factor.
#[test]
fn scale_factor_applies() {
    // Reuse the tet but interpret its coordinates as millimetres.
    let m = outram_foam_mesh::ideas_unv_to_foam::convert_scaled(TET_UNV, 1.0e-3)
        .expect("scaled convert");
    // Volume scales by scale^3 = 1e-9 ⇒ (1/6) * 1e-9 m³.
    assert_abs_diff_eq!(m.total_volume(), (1.0 / 6.0) * 1.0e-9, epsilon = 1e-21);
}
