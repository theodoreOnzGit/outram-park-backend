// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under the GNU General Public
// License v3; see the crate's LICENSE files.

//! gh:#1476: the cfMesh executables run on an **unmodified upstream case**
//! (cfMesh `e944f6ba695f` `tutorials/cartesian2DMesh/hatOctree`, copied to
//! `crates/outram-park-fork-cfmesh/test_data/cfmesh_tutorials/`).
//!
//! Methodology: copy the case to a scratch directory, run
//! `outram_foam_cli::cfmesh::run_cfmesh` (what `cartesian2DMesh -case`
//! runs), read the written `constant/polyMesh` back with
//! `outram_foam_basic_lib::io::PolyMesh`. Then the same with
//! `workflowControls { stopAfter edgeExtraction; }`, followed by a restart
//! (`restartFromLatestStep 1`) from the mesh that run wrote; and a
//! `meshDict` with a misspelt keyword, which must be refused.
//!
//! Results (2026-11-11, release, one run): 18 676 cells, 38 090 points,
//! 75 072 faces; patches `left` 210, `bottom` 109, `right` 154, `top0` 154,
//! `top1` 109 faces, `bottomEmptyFaces` / `topEmptyFaces` 18 676 each;
//! 1.7 s. The restarted run gives the same cell count as the straight one.
//! Interpretation: the reader, the 2D workflow and the writer connect; the
//! mesh is not compared with upstream cfMesh here (gh:#1385).

use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use outram_foam_cli::cfmesh::run_cfmesh;
use outram_park_fork_cfmesh::mesh_dict::Generator;
use std::path::{Path, PathBuf};

fn hat_source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../outram-park-fork-cfmesh/test_data/cfmesh_tutorials/cartesian2DMesh/hatOctree")
}

/// A copy of the hat case under the temp directory, removed on drop.
struct Case(PathBuf);

impl Case {
    fn new(tag: &str, mesh_dict_extra: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cfmesh_cli_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("system")).unwrap();
        std::fs::copy(hat_source().join("geom.fms"), dir.join("geom.fms")).unwrap();
        let d = std::fs::read_to_string(hat_source().join("system/meshDict")).unwrap();
        std::fs::write(dir.join("system/meshDict"), format!("{d}\n{mesh_dict_extra}\n")).unwrap();
        Case(dir)
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cartesian_2d_mesh_runs_the_hat_tutorial_and_restarts() {
    let c = Case::new("hat", "");
    let r = run_cfmesh(&c.0, Generator::Cartesian2DMesh).unwrap();
    assert!(r.reported.is_empty(), "{:?}", r.reported);
    assert_eq!(r.stopped_after, None);
    let pm = PolyMesh::read(c.0.join("constant/polyMesh")).unwrap();
    assert_eq!(pm.n_cells, r.mesh.n_cells);
    let names: Vec<&str> = pm.patches.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["left", "bottom", "right", "top0", "top1", "bottomEmptyFaces", "topEmptyFaces"]);
    println!("hatOctree: {} cells, {:.2} s", r.mesh.n_cells, r.seconds);

    let s = Case::new("hat_stop", "workflowControls { stopAfter edgeExtraction; }");
    let stopped = run_cfmesh(&s.0, Generator::Cartesian2DMesh).unwrap();
    assert_eq!(stopped.stopped_after.map(|x| x.name()), Some("edgeExtraction"));
    // restart from what the stopped run wrote
    let d = std::fs::read_to_string(hat_source().join("system/meshDict")).unwrap();
    std::fs::write(s.0.join("system/meshDict"), format!("{d}\nworkflowControls {{ restartFromLatestStep 1; }}\n")).unwrap();
    let restarted = run_cfmesh(&s.0, Generator::Cartesian2DMesh).unwrap();
    assert!(restarted.notes.iter().any(|n| n.starts_with("restartFromLatestStep")), "{:?}", restarted.notes);
    assert_eq!(restarted.stopped_after, None);
    assert_eq!(restarted.mesh.n_cells, r.mesh.n_cells);
}

#[test]
fn a_misspelt_mesh_dict_keyword_is_refused() {
    let c = Case::new("typo", "boundaryCelSize 0.005;");
    let e = run_cfmesh(&c.0, Generator::Cartesian2DMesh).unwrap_err().to_string();
    assert!(e.contains("boundaryCelSize"), "{e}");
}
