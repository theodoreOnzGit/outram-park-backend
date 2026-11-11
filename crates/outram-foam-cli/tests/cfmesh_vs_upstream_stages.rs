// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under the GNU General Public
// License v3; see the crate's LICENSE files.

//! gh:#1385: stage-by-stage differential of the cfMesh port against real
//! upstream cfMesh output on a **tutorial case** (asmoOctree, elbow_90degree),
//! through the same entry point a user runs (`outram_foam_cli::cfmesh::run_cfmesh`).
//!
//! # Method
//!
//! A copy of `crates/outram-park-fork-cfmesh/test_data/cfmesh_tutorials/<case>`
//! gets `workflowControls { stopAfter <step>; }` appended to its `meshDict`
//! and is run; the `constant/polyMesh` the port writes is read and compared
//! with the `constant/polyMesh` upstream cfMesh `e944f6ba695f` wrote for the
//! same case, stopped after the same step (`scripts/run-case.sh`, see
//! `verification_and_validation/upstream_cfmesh_gh1385/README.md`).
//! Upstream's stage meshes are read from the directory named by the
//! environment variable `GH1385_UPSTREAM_RUNS` (default
//! `/home/user/upstream-build/runs`), run `stage_<case>_<generator>_<step>`.
//! The test is skipped (prints, passes) when that directory is absent, since
//! the stage meshes are not committed (about 3.5 MB each).
//!
//! The comparison per stage: counts; points at the same index (bit-identical
//! count); faces at the same index with the same ring, owner and neighbour;
//! and, at the first stage that differs, the first differing face.

use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use outram_foam_cli::cfmesh::run_cfmesh;
use outram_park_fork_cfmesh::mesh_dict::Generator;
use std::path::{Path, PathBuf};

const STEPS: [&str; 7] = [
    "templateGeneration",
    "surfaceTopology",
    "surfaceProjection",
    "patchAssignment",
    "edgeExtraction",
    "boundaryLayerGeneration",
    "meshOptimisation",
];

fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../outram-park-fork-cfmesh")
}

fn runs_dir() -> PathBuf {
    PathBuf::from(std::env::var("GH1385_UPSTREAM_RUNS").unwrap_or_else(|_| "/home/user/upstream-build/runs".into()))
}

/// cfMesh writes `neighbour` for every face (-1 on boundary faces); the
/// reader wants the internal ones only (as `tests/common` of the cfMesh crate).
fn internal_neighbours_only(text: &str) -> String {
    let open = text.find("\n(\n").expect("neighbour list");
    let before = &text[..open];
    let count_start = before.rfind('\n').unwrap() + 1;
    let body = &text[open + 3..];
    let end = body.find("\n)").expect("list end");
    let vals: Vec<i64> = body[..end].split_whitespace().map(|s| s.parse().unwrap()).collect();
    let n = vals.iter().position(|&v| v < 0).unwrap_or(vals.len());
    let mut out = String::from(&text[..count_start]);
    out.push_str(&format!("{n}\n(\n"));
    for v in &vals[..n] {
        out.push_str(&format!("{v}\n"));
    }
    out.push_str(")\n");
    out
}

fn read_upstream(run: &Path) -> PolyMesh {
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("up_{}", run.file_name().unwrap().to_string_lossy()));
    std::fs::create_dir_all(&dst).unwrap();
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let mut text = std::fs::read_to_string(run.join("constant/polyMesh").join(f)).unwrap();
        if f == "neighbour" {
            text = internal_neighbours_only(&text);
        }
        std::fs::write(dst.join(f), text).unwrap();
    }
    PolyMesh::read(&dst).expect("read upstream polyMesh")
}

/// The stage report line, and whether points and faces are index-identical.
fn compare(port: &PolyMesh, up: &PolyMesh) -> (String, bool) {
    let counts_p = (port.points.len(), port.faces.len(), port.n_cells, port.n_internal_faces);
    let counts_u = (up.points.len(), up.faces.len(), up.n_cells, up.n_internal_faces);
    let mut line = format!("counts port {counts_p:?} upstream {counts_u:?}");
    let mut identical = counts_p == counts_u;
    if port.points.len() == up.points.len() {
        let same = port.points.iter().zip(&up.points).filter(|(a, b)| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits() && a.z.to_bits() == b.z.to_bits()).count();
        let worst = port.points.iter().zip(&up.points).map(|(a, b)| (a.x - b.x).abs().max((a.y - b.y).abs()).max((a.z - b.z).abs())).fold(0.0f64, f64::max);
        line += &format!("; points {same}/{} bit-identical, max |d| {worst:.3e}", up.points.len());
        identical &= same == up.points.len();
    }
    if port.faces.len() == up.faces.len() {
        let bad: Vec<usize> = (0..up.faces.len())
            .filter(|&i| port.faces[i].verts != up.faces[i].verts || port.faces[i].owner != up.faces[i].owner || port.faces[i].neighbour != up.faces[i].neighbour)
            .collect();
        line += &format!("; faces {}/{} index-identical", up.faces.len() - bad.len(), up.faces.len());
        if let Some(&i) = bad.first() {
            line += &format!("; first differing face {i}: port {:?} (o {} n {:?}) upstream {:?} (o {} n {:?})", port.faces[i].verts, port.faces[i].owner, port.faces[i].neighbour, up.faces[i].verts, up.faces[i].owner, up.faces[i].neighbour);
        }
        identical &= bad.is_empty();
    }
    (line, identical)
}

fn port_stage(tutorial: &str, generator: Generator, step: &str) -> PolyMesh {
    let src = crate_dir().join("test_data/cfmesh_tutorials").join(tutorial);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("stage_{}_{step}", tutorial.replace('/', "_")));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("system")).unwrap();
    for e in std::fs::read_dir(&src).unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
    }
    let mut dict = std::fs::read_to_string(src.join("system/meshDict")).unwrap();
    dict += &format!("\nworkflowControls\n{{\n    stopAfter {step};\n}}\n");
    std::fs::write(dir.join("system/meshDict"), dict).unwrap();
    run_cfmesh(&dir, generator).unwrap();
    PolyMesh::read(&dir.join("constant/polyMesh")).expect("read port polyMesh")
}

fn staged(tutorial: &str, generator: Generator, run_prefix: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for step in STEPS {
        let run = runs_dir().join(format!("{run_prefix}_{step}"));
        if !run.join("constant/polyMesh/points").is_file() {
            println!("{run_prefix} after {step}: no upstream stage mesh at {}; skipped", run.display());
            continue;
        }
        let (line, same) = compare(&port_stage(tutorial, generator, step), &read_upstream(&run));
        println!("{run_prefix} after {step}: {line}");
        out.push((step.to_string(), same));
    }
    out
}

#[test]
fn asmo_octree_cartesian_mesh_stages() {
    let r = staged("cartesianMesh/asmoOctree", Generator::CartesianMesh, "stage_asmo_cartesianMesh");
    println!("{r:?}");
}
