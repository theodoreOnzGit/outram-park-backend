// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under the GNU General Public
// License v3; see the crate's LICENSE files.

//! gh:#1385 round 5: stage-by-stage differential of the port against real
//! upstream cfMesh output on the tutorial `cartesianMesh/singleOrifice`
//! (9 STL regions, an `anisotropicSources` plane, `localRefinement`,
//! per-patch `patchBoundaryLayers`, `renameBoundary`).
//!
//! # Method
//!
//! The tutorial's `meshDict` (a byte copy in `test_data/cfmesh_tutorials`) is
//! read with the port's `meshDict` reader exactly as `outram-foam-cli`'s
//! `run_cfmesh` does for `cartesianMesh` (this file repeats that path so the
//! test runs from this crate alone), and the generator is stopped after each
//! `workflowControls` step. The result is compared with the `polyMesh` upstream
//! cfMesh `e944f6ba695f` (ESI v2506, serial, `OMP_NUM_THREADS=1`,
//! `writePrecision 17`) wrote for the same case stopped after the same step
//! (`scripts/run-stages.sh`, V&V README "Round 5"). Upstream stage meshes are
//! read from `runs/stages/single_cartesianMesh_<step>` (committed, gzipped) or,
//! if absent, from the loose run `$GH1385_UPSTREAM_RUNS/stage_single_cartesianMesh_<step>`
//! (default `/home/user/upstream-build/runs`); a stage with neither is skipped
//! and said so.
//!
//! The comparison is `tests/common::compare`: counts; points at the same index
//! (bit-identical count and worst |d|); faces index-identical with owner and
//! neighbour.
use common::*;
use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use outram_park_fork_cfmesh::mesh_dict::{read_mesh_dict, GeneratorOptions, Generator};
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::cartesian_mesh;
use outram_park_fork_cfmesh::tri_surf_io::read_surface_bytes;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use outram_park_fork_cfmesh::workflow_controls::{WorkflowSettings, WorkflowStep};
use std::path::PathBuf;

mod common;

const STEPS: [WorkflowStep; 8] = [
    WorkflowStep::TemplateGeneration,
    WorkflowStep::SurfaceTopology,
    WorkflowStep::SurfaceProjection,
    WorkflowStep::PatchAssignment,
    WorkflowStep::EdgeExtraction,
    WorkflowStep::BoundaryLayerGeneration,
    WorkflowStep::MeshOptimisation,
    WorkflowStep::BoundaryLayerRefinement,
];

/// `GH1385_CASE` names another case directory (an ablation: same STL, edited
/// `meshDict`), with `GH1385_PREFIX` the matching upstream stage-run prefix.
fn case_dir() -> PathBuf {
    if let Ok(c) = std::env::var("GH1385_CASE") {
        return PathBuf::from(c);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_data/cfmesh_tutorials/cartesianMesh/singleOrifice")
}

/// The port's mesh and patch types after `step` (`None`: the whole run).
fn port_run(step: Option<WorkflowStep>) -> (VolumeMesh, Vec<String>) {
    let case = case_dir();
    let dict = FoamFile::read(&case.join("system/meshDict")).unwrap().dict;
    let reading = read_mesh_dict(&dict, Generator::CartesianMesh).unwrap();
    let load = |f: &str| read_surface_bytes(f, &std::fs::read(case.join(f)).unwrap());
    let s = load(&reading.surface_file).unwrap().surface;
    let (opts, _notes) = reading.resolve(&s, |f| load(f).map(|r| r.surface)).unwrap();
    let regions = s.surface_regions();
    let GeneratorOptions::CartesianMesh(mut o) = opts else { panic!("cartesianMesh options") };
    o.workflow = WorkflowSettings { stop_after: step, restart_from_latest_step: false };
    let (m, r) = cartesian_mesh(&s.points, &s.triangles, Some(&regions), &o).unwrap();
    (m, r.patch_types)
}

/// Upstream's stage mesh, committed or loose; `None` when neither exists.
fn upstream(step: &str) -> Option<PolyMesh> {
    let prefix = std::env::var("GH1385_PREFIX").unwrap_or_else(|_| "single_cartesianMesh".into());
    let name = format!("{prefix}_{step}");
    let committed = upstream_dir().join("runs/stages").join(&name).join("constant/polyMesh");
    let loose = PathBuf::from(std::env::var("GH1385_UPSTREAM_RUNS").unwrap_or_else(|_| "/home/user/upstream-build/runs".into()))
        .join(format!("stage_{name}"))
        .join("constant/polyMesh");
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("up_{name}"));
    std::fs::create_dir_all(&dst).unwrap();
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let text = if committed.join(format!("{f}.gz")).is_file() {
            String::from_utf8(gunzip(&std::fs::read(committed.join(format!("{f}.gz"))).unwrap())).unwrap()
        } else if loose.join(f).is_file() {
            std::fs::read_to_string(loose.join(f)).unwrap()
        } else {
            return None;
        };
        std::fs::write(dst.join(f), if f == "neighbour" { internal_neighbours_only(&text) } else { text }).unwrap();
    }
    Some(PolyMesh::read(&dst).unwrap())
}

#[test]
fn single_orifice_stage_table() {
    for step in STEPS {
        // GH1385_ONLY=<step> runs one stage
        if std::env::var("GH1385_ONLY").is_ok_and(|o| o != step.name()) {
            continue;
        }
        let Some(up) = upstream(step.name()) else {
            println!("singleOrifice after {}: no upstream stage mesh; skipped", step.name());
            continue;
        };
        let (m, _) = port_run(Some(step));
        let d = compare(&m, &up);
        let sizes_up: Vec<usize> = up.patches.iter().map(|p| p.size).collect();
        println!("singleOrifice after {}: {}; patch sizes upstream {sizes_up:?}", step.name(), d.summary());
    }
}
