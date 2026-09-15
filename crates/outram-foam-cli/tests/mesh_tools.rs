//! Integration tests for the mesh-tool CLI binaries (`op-x8x.1`).
//!
//! ## Methodology
//!
//! The three mesh-tool binaries live under `src/bin/` (no exported library
//! surface), so each is pulled in here with `#[path]` as a module and its
//! `pub fn run(...)` is driven directly on a temporary case. Fixtures under
//! `tests/cases/` are copied into `CARGO_TARGET_TMPDIR` first, so the tools may
//! write `constant/polyMesh` without touching the committed fixtures.
//!
//! Each test round-trips through the on-disk `polyMesh`: it runs the tool, then
//! re-reads the written mesh with the *independent*
//! [`outram_foam_basic_lib::io::poly_mesh::PolyMesh::read`] parser and asserts
//! the cell count and that the mesh reconstructs a valid `FvMesh`. Passing this
//! proves the tool's mesh-crate → I/O bridge, the writer, and the reader all
//! agree.
//!
//! ## Results (2026-07-17)
//!
//! - `block_mesh_cavity_roundtrip`: cavity `blockMeshDict` (20x20x1) → 400
//!   cells, 882 points, 3 patches; re-read gives 400 cells and a valid FvMesh.
//! - `poly_dual_mesh_box_roundtrip`: 3x3x3 block (27 cells, 64 points) →
//!   polyDualMesh → 64 dual cells; re-read gives a valid FvMesh.
//! - `ideas_unv_to_foam_single_hex`: single-hex `.unv` → 1 cell, 6 boundary
//!   faces; re-read gives 1 cell.

use std::fs;
use std::path::{Path, PathBuf};

use outram_foam_basic_lib::io::poly_mesh::PolyMesh as IoPolyMesh;
use outram_foam_cli::CaseArgs;

#[allow(dead_code)]
#[path = "../src/bin/block_mesh.rs"]
mod block_mesh;

#[allow(dead_code)]
#[path = "../src/bin/ideas_unv_to_foam.rs"]
mod ideas_unv_to_foam;

#[allow(dead_code)]
#[path = "../src/bin/poly_dual_mesh.rs"]
mod poly_dual_mesh;

/// Copy a `tests/cases/<fixture>/system/blockMeshDict` fixture into a fresh
/// temporary case directory and return that case path.
fn setup_block_case(fixture: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cases")
        .join(fixture)
        .join("system/blockMeshDict");
    let case = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{fixture}_case"));
    let _ = fs::remove_dir_all(&case);
    let sysdir = case.join("system");
    fs::create_dir_all(&sysdir).unwrap();
    fs::copy(&src, sysdir.join("blockMeshDict")).unwrap();
    case
}

#[test]
fn block_mesh_cavity_roundtrip() {
    let case = setup_block_case("cavity");
    let args = CaseArgs { case: case.clone() };

    block_mesh::run(&args).expect("blockMesh run");

    let pm_dir = case.join("constant/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        assert!(
            pm_dir.join(f).is_file(),
            "expected polyMesh/{f} to be written"
        );
    }

    let mesh = IoPolyMesh::read(&pm_dir).expect("re-read polyMesh");
    assert_eq!(mesh.n_cells, 400, "cavity 20x20x1 should have 400 cells");
    assert_eq!(
        mesh.n_points(),
        882,
        "cavity should have 21*21*2 = 882 points"
    );
    assert_eq!(mesh.patches.len(), 3, "cavity has 3 boundary patches");

    // The re-read connectivity must reconstruct a valid finite-volume mesh.
    let fv = mesh.to_fv_mesh().expect("polyMesh -> FvMesh");
    assert_eq!(fv.n_cells, 400);
}

#[test]
fn poly_dual_mesh_box_roundtrip() {
    let case = setup_block_case("box");
    let args = CaseArgs { case: case.clone() };

    // First mesh the primal with blockMesh, then dualise it.
    block_mesh::run(&args).expect("blockMesh run");
    poly_dual_mesh::run(&args).expect("polyDualMesh run");

    let dual_dir = case
        .join("constant")
        .join(poly_dual_mesh::DUAL_OUTPUT_SUBDIR);
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        assert!(
            dual_dir.join(f).is_file(),
            "expected dualMesh/{f} to be written"
        );
    }

    let dual = IoPolyMesh::read(&dual_dir).expect("re-read dual polyMesh");
    // The dual has one cell per primal point (64 for a 3x3x3 block).
    assert_eq!(dual.n_cells, 64, "dual cells == primal points (4^3)");
    dual.to_fv_mesh().expect("dual polyMesh -> FvMesh");
}

#[test]
fn ideas_unv_to_foam_single_hex() {
    let case = Path::new(env!("CARGO_TARGET_TMPDIR")).join("unv_case");
    let _ = fs::remove_dir_all(&case);
    fs::create_dir_all(&case).unwrap();
    let args = CaseArgs { case: case.clone() };

    let unv = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/unv/cube.unv");
    ideas_unv_to_foam::run_with_unv(&args, Some(unv)).expect("ideasUnvToFoam run");

    let pm_dir = case.join("constant/polyMesh");
    let mesh = IoPolyMesh::read(&pm_dir).expect("re-read polyMesh");
    assert_eq!(mesh.n_cells, 1, "single hexahedron -> 1 cell");
    assert_eq!(mesh.n_internal_faces, 0, "one cell has no internal faces");
    assert_eq!(mesh.n_boundary_faces(), 6, "a hex has 6 boundary faces");
}
