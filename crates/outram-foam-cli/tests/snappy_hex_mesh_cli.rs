//! gh:#1470: the `snappyHexMesh` binary, end to end on a temporary case.
//!
//! ## Methodology
//! Case: `blockMesh` unit box 8x8x8 (the `box` fixture with its divisions
//! changed), an ASCII STL of a 24x24 UV sphere r = 0.3 centred (0.5, 0.5, 0.5)
//! in `constant/geometry/sphere.stl`, and a `snappyHexMeshDict` in the dev
//! tutorial's shape (`level (2 2)`, `insidePoint`, `nSolveIter`,
//! `addLayers true`, a `features` entry and a `mergeTolerance`). The binary's
//! `run` is driven directly; the written mesh is re-read with the independent
//! `PolyMesh::read` and must rebuild a valid `FvMesh`.
//!
//! ## Predictions (written before the first run, 2026-11-11)
//! 1. Final cell count strictly between 512 and 512 * 64.
//! 2. The report names `mergeTolerance` and `castellatedMeshControls.features`
//!    under `Not ported` and says layers were not added.
//! 3. `1/polyMesh` is written without `-overwrite`, and `constant/polyMesh`
//!    still holds the 512-cell background.
//!
//! ## Results
//! See `crates/outram-foam-mesh/verification_and_validation/snappy_hex_mesh_1470/README.md`.

use std::fs;
use std::path::Path;

use outram_foam_basic_lib::io::poly_mesh::PolyMesh as IoPolyMesh;
use outram_foam_cli::CaseArgs;
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;

#[allow(dead_code)]
#[path = "../src/bin/block_mesh.rs"]
mod block_mesh;

#[allow(dead_code)]
#[path = "../src/bin/snappy_hex_mesh.rs"]
mod snappy_hex_mesh;

fn ascii_stl(s: &TriangleSoup) -> String {
    let mut t = String::from("solid sphere\n");
    for tri in &s.triangles {
        t.push_str("facet normal 0 0 0\n outer loop\n");
        for v in [tri.a, tri.b, tri.c] {
            t.push_str(&format!("  vertex {:e} {:e} {:e}\n", v.x, v.y, v.z));
        }
        t.push_str(" endloop\nendfacet\n");
    }
    t.push_str("endsolid sphere\n");
    t
}

const DICT: &str = r#"
FoamFile { format ascii; class dictionary; object snappyHexMeshDict; }
castellatedMesh true;
snap true;
addLayers true;
geometry { sphere { type triSurface; file "sphere.stl"; } };
castellatedMeshControls
{
    nCellsBetweenLevels 1;
    minRefinementCells 0;
    features ( { file "sphere.eMesh"; level 0; } );
    refinementSurfaces { sphere { level (2 2); } }
    insidePoint (0.02 0.02 0.02);
}
snapControls { nSmoothPatch 2; nSolveIter 10; }
mergeTolerance 1e-6;
"#;

#[test]
fn snappy_hex_mesh_sphere_case() {
    let case = Path::new(env!("CARGO_TARGET_TMPDIR")).join("snappy_sphere_case");
    let _ = fs::remove_dir_all(&case);
    fs::create_dir_all(case.join("system")).unwrap();
    fs::create_dir_all(case.join("constant/geometry")).unwrap();

    let box_dict = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/box/system/blockMeshDict"),
    )
    .unwrap()
    .replace("(3 3 3)", "(8 8 8)");
    fs::write(case.join("system/blockMeshDict"), box_dict).unwrap();
    block_mesh::run(&CaseArgs { case: case.clone() }).unwrap();

    let sphere = TriangleSoup::uv_sphere(
        outram_foam_basic_lib::primitives::Vector3::new(0.5, 0.5, 0.5),
        0.3,
        24,
        24,
    );
    fs::write(case.join("constant/geometry/sphere.stl"), ascii_stl(&sphere)).unwrap();
    fs::write(case.join("system/snappyHexMeshDict"), DICT).unwrap();

    let args = snappy_hex_mesh::SnappyArgs {
        case: CaseArgs { case: case.clone() },
        dict: None,
        overwrite: false,
    };
    let report = snappy_hex_mesh::run(&args).expect("snappyHexMesh run");

    let mesh = IoPolyMesh::read(case.join("1/polyMesh")).expect("re-read 1/polyMesh");
    assert!(mesh.n_cells > 512 && mesh.n_cells < 512 * 64, "{}", mesh.n_cells);
    mesh.to_fv_mesh().expect("valid FvMesh");
    assert!(report.contains("mergeTolerance"));
    assert!(report.contains("castellatedMeshControls.features"));
    assert!(report.contains("layers are NOT added"));
    let bg = IoPolyMesh::read(case.join("constant/polyMesh")).unwrap();
    assert_eq!(bg.n_cells, 512);
}
