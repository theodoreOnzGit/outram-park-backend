// SPDX-License-Identifier: GPL-3.0-only
//! The `polyDualMesh` command line (gh:#1471): `featureAngle`, `-overwrite`,
//! `-region`, `-splitAllFaces`, `-concaveMultiCells`, `-doNotPreserveFaceZones`,
//! the new-time output, and face-zone reading. Each test builds a case with
//! `blockMesh` (the crate's own) and runs the utility's `run_with`.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use outram_foam_basic_lib::io::poly_mesh::PolyMesh as IoPolyMesh;
use outram_foam_cli::CaseArgs;

#[allow(dead_code)]
#[path = "../src/bin/block_mesh.rs"]
mod block_mesh;

#[allow(dead_code)]
#[path = "../src/bin/poly_dual_mesh.rs"]
mod poly_dual_mesh;

use poly_dual_mesh::PolyDualArgs;

const CONTROL_DICT: &str = "FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }\n\
startFrom startTime;\nstartTime 0;\ndeltaT 0.005;\n";

/// A freshly blockMeshed `box` case (3x3x3) with a `controlDict`.
fn box_case(tag: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/box/system/blockMeshDict");
    let case = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pdm_cli_{tag}"));
    let _ = fs::remove_dir_all(&case);
    fs::create_dir_all(case.join("system")).unwrap();
    fs::copy(&src, case.join("system/blockMeshDict")).unwrap();
    fs::write(case.join("system/controlDict"), CONTROL_DICT).unwrap();
    block_mesh::run(&CaseArgs { case: case.clone() }).expect("blockMesh");
    case
}

fn parse(case: &Path, rest: &[&str]) -> PolyDualArgs {
    let mut argv: Vec<std::ffi::OsString> = vec!["polyDualMesh".into(), "-case".into(), case.into()];
    argv.extend(rest.iter().map(std::ffi::OsString::from));
    PolyDualArgs::parse_from(outram_foam_cli::openfoam_argv(argv))
}

#[test]
fn writes_a_new_time_directory_by_default() {
    let case = box_case("newtime");
    poly_dual_mesh::run_with(&parse(&case, &["60"])).expect("run");
    let dual = IoPolyMesh::read(case.join("0.005/polyMesh")).expect("dual in the new time");
    assert_eq!(dual.n_cells, 64);
    // the primal is untouched
    assert_eq!(IoPolyMesh::read(case.join("constant/polyMesh")).unwrap().n_cells, 27);
}

#[test]
fn overwrite_replaces_the_primal_and_drops_stale_zone_files() {
    let case = box_case("overwrite");
    fs::write(case.join("constant/polyMesh/cellZones"), "0()\n").unwrap();
    poly_dual_mesh::run_with(&parse(&case, &["60", "-overwrite"])).expect("run");
    assert_eq!(IoPolyMesh::read(case.join("constant/polyMesh")).unwrap().n_cells, 64);
    assert!(!case.join("constant/polyMesh/cellZones").exists());
    assert!(!case.join("0.005").exists(), "no new time with -overwrite");
}

#[test]
fn upstream_options_need_upstream_mode_and_the_angle_is_checked() {
    let case = box_case("reject");
    for flag in ["-splitAllFaces", "-concaveMultiCells", "-doNotPreserveFaceZones"] {
        let e = poly_dual_mesh::run_with(&parse(&case, &["60", flag])).unwrap_err();
        assert!(e.to_string().contains("-upstream"), "{flag}: {e}");
    }
    assert!(poly_dual_mesh::run_with(&parse(&case, &["200"])).is_err());
    assert!(poly_dual_mesh::run_with(&parse(&case, &["60", "-upstream", "-flavour", "bogus"])).is_err());
}

#[test]
fn upstream_split_all_faces_writes_a_valid_polyhedral_mesh() {
    let case = box_case("split");
    poly_dual_mesh::run_with(&parse(&case, &["60", "-upstream", "-splitAllFaces"])).expect("run");
    let dual = IoPolyMesh::read(case.join("0.005/polyMesh")).expect("dual");
    assert_eq!(dual.n_cells, 64);
    assert_eq!(dual.n_internal_faces, 324);
    dual.to_fv_mesh().expect("valid FvMesh");
    // esi centres and concave multi-cells both run
    poly_dual_mesh::run_with(&parse(&case, &["60", "-upstream", "-flavour", "esi", "-concaveMultiCells", "-overwrite"]))
        .expect("run esi");
}

#[test]
fn region_option_reads_and_writes_under_the_region() {
    let case = box_case("region");
    let region_dir = case.join("constant/fluid");
    fs::create_dir_all(&region_dir).unwrap();
    fs::rename(case.join("constant/polyMesh"), region_dir.join("polyMesh")).unwrap();
    poly_dual_mesh::run_with(&parse(&case, &["60", "-region", "fluid"])).expect("run");
    assert_eq!(IoPolyMesh::read(case.join("0.005/fluid/polyMesh")).unwrap().n_cells, 64);
}

#[test]
fn face_zones_are_read_from_the_primal_and_preserved() {
    let case = box_case("zones");
    // A face zone of one internal face (face 0), in the file format blockMesh writes.
    fs::write(
        case.join("constant/polyMesh/faceZones"),
        "FoamFile { version 2.0; format ascii; class regIOobject; object faceZones; }\n\
         1\n(\nmid\n{\n    type faceZone;\n    faceLabels List<label> 2(0 1);\n    flipMap List<bool> 2{0};\n}\n)\n",
    )
    .unwrap();
    let zones = poly_dual_mesh::read_face_zones(&case.join("constant/polyMesh/faceZones")).unwrap();
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].name, "mid");
    assert_eq!(zones[0].faces, vec![0, 1]);
    let kept = poly_dual_mesh::run_with(&parse(&case, &["60", "-upstream"]));
    assert!(kept.is_ok());
    let with_zone = IoPolyMesh::read(case.join("0.005/polyMesh")).unwrap().n_cells;
    poly_dual_mesh::run_with(&parse(&case, &["60", "-upstream", "-doNotPreserveFaceZones", "-overwrite"])).unwrap();
    let without = IoPolyMesh::read(case.join("constant/polyMesh")).unwrap().n_cells;
    assert!(with_zone > without, "zone preserved: {with_zone} dual cells vs {without}");
    // a missing file is no zones
    assert!(poly_dual_mesh::read_face_zones(&case.join("nope")).unwrap().is_empty());
}

#[test]
fn time_names_follow_openfoam_general_format() {
    assert_eq!(poly_dual_mesh::time_name(0.005), "0.005");
    assert_eq!(poly_dual_mesh::time_name(1.0), "1");
    assert_eq!(poly_dual_mesh::time_name(0.0), "0");
    assert_eq!(poly_dual_mesh::time_name(12.5), "12.5");
    assert_eq!(poly_dual_mesh::time_name(1e-6), "1e-06");
    assert_eq!(poly_dual_mesh::time_name(1234567.0), "1.23457e+06");
}

#[test]
fn next_time_needs_a_control_dict_with_delta_t() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pdm_cli_nocd");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("system")).unwrap();
    assert!(poly_dual_mesh::next_time_name(&dir).is_err());
    fs::write(dir.join("system/controlDict"), "startTime 2;\ndeltaT 0.5;\n").unwrap();
    assert_eq!(poly_dual_mesh::next_time_name(&dir).unwrap(), "2.5");
}
