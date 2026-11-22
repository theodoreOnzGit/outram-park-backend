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
//! (`scripts/run-stages.sh`, V&V README "Round 5"). Committed upstream meshes:
//! `runs/stages/single_cartesianMesh_boundaryLayerGeneration` and the full run
//! `runs/singleOrifice` (gzipped); every other stage is read from the loose run
//! `$GH1385_UPSTREAM_RUNS/stage_single_cartesianMesh_<step>` (default
//! `/home/user/upstream-build/runs`) when it exists and is skipped, and said so,
//! otherwise.
//!
//! The comparison is `tests/common::compare`: counts; points at the same index
//! (bit-identical count and worst |d|); faces index-identical with owner and
//! neighbour; plus the patch names, types and sizes.
//!
//! # Results (2026-11-13, release, PETIR `pow`)
//!
//! | step | points bit-identical | faces index-identical |
//! |---|---|---|
//! | templateGeneration | 29274/29274 | 65621/65621 |
//! | surfaceTopology, surfaceProjection, patchAssignment | 29173/29173 | 65413/65413 |
//! | edgeExtraction | 29691/29691 | 72272/72272 |
//! | boundaryLayerGeneration | 43283/43283 | 111722/111722 |
//! | meshOptimisation, ~~before the fixes: 1420/43283, worst 6.9e-2 m, faces not matched~~ | **41968/43283, worst 1.776e-15 m** | 111722/111722 |
//! | final mesh (`renumberMesh`, `renameBoundary`) | 81184/82961, worst 1.776e-15 m | 227372/227372 |
//!
//! The 1315 / 1777 points left are the `pow` ulp of PETIR's no-FMA `pow` against
//! glibc's FMA `pow` (maintainer: "Keep 1 ulp", gh:#1385, 2026-11-11), counted
//! and pinned **from above** here; with the platform `pow` swapped in (an
//! experiment, not committed) the mesh is bit-identical in all 43283 points.
//! Two port defects found by this case, both in the anisotropic back-scaling
//! of `meshOptimisation`: `projectSurfaceAfterBackScaling` mapped to the exact
//! nearest point and optimised without upstream's new octree, and
//! `untangleMeshFV`'s boundary loop ran 3 iterations where upstream's calls
//! pass a `bool` into the `nIterations` parameter (V&V README, "Round 5").
use common::*;
use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use outram_park_fork_cfmesh::mesh_dict::{read_mesh_dict, Generator, GeneratorOptions};
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::cartesian_mesh;
use outram_park_fork_cfmesh::tri_surf_io::read_surface_bytes;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use outram_park_fork_cfmesh::workflow_controls::{WorkflowSettings, WorkflowStep};
use std::path::{Path, PathBuf};

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

/// Read the five `polyMesh` files of `dir` (gzipped or loose), `None` when absent.
fn read_poly_mesh_dir(dir: &Path, tag: &str) -> Option<PolyMesh> {
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("up_{tag}"));
    std::fs::create_dir_all(&dst).unwrap();
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let text = if dir.join(format!("{f}.gz")).is_file() {
            String::from_utf8(gunzip(&std::fs::read(dir.join(format!("{f}.gz"))).unwrap())).unwrap()
        } else if dir.join(f).is_file() {
            std::fs::read_to_string(dir.join(f)).unwrap()
        } else {
            return None;
        };
        std::fs::write(dst.join(f), if f == "neighbour" { internal_neighbours_only(&text) } else { text }).unwrap();
    }
    Some(PolyMesh::read(&dst).unwrap())
}

/// Upstream's stage mesh, committed or loose; `None` when neither exists.
fn upstream(step: &str) -> Option<PolyMesh> {
    let prefix = std::env::var("GH1385_PREFIX").unwrap_or_else(|_| "single_cartesianMesh".into());
    let name = format!("{prefix}_{step}");
    let committed = upstream_dir().join("runs/stages").join(&name).join("constant/polyMesh");
    let loose = PathBuf::from(std::env::var("GH1385_UPSTREAM_RUNS").unwrap_or_else(|_| "/home/user/upstream-build/runs".into()))
        .join(format!("stage_{name}"))
        .join("constant/polyMesh");
    read_poly_mesh_dir(&committed, &name).or_else(|| read_poly_mesh_dir(&loose, &name))
}

/// `(name, type, nFaces)` of every patch in a `boundary` file's text.
fn boundary_entries(text: &str) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    let toks: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i + 1 < toks.len() {
        if toks[i + 1] == "{" && toks[i] != "FoamFile" {
            let name = toks[i].to_string();
            let (mut ty, mut n) = (String::new(), 0usize);
            let mut j = i + 2;
            while toks[j] != "}" {
                match toks[j] {
                    "type" => ty = toks[j + 1].trim_end_matches(';').to_string(),
                    "nFaces" => n = toks[j + 1].trim_end_matches(';').parse().unwrap(),
                    _ => {}
                }
                j += 1;
            }
            out.push((name, ty, n));
            i = j;
        }
        i += 1;
    }
    out
}

/// (points that differ, worst |d|, faces not index-identical); counts must be equal.
fn deviation(port: &VolumeMesh, up: &PolyMesh) -> (usize, f64, usize) {
    let d = compare(port, up);
    assert!(d.counts_equal, "counts {:?}", d.counts);
    let (same, worst) = d.index_points.expect("equal point counts");
    (port.points.len() - same, worst, port.faces.len() - d.index_faces_identical.expect("equal face counts"))
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
        if step == WorkflowStep::BoundaryLayerGeneration && std::env::var("GH1385_CASE").is_err() {
            // committed upstream stage: every point and face identical
            assert_eq!(deviation(&m, &up), (0, 0.0, 0), "boundaryLayerGeneration must be bit-identical");
        }
    }
}

/// The whole run against upstream's committed final mesh (after
/// `meshOptimisation`'s anisotropic back-scaling, `refineBoundaryLayers`,
/// `renumberMesh` and `renameBoundaryPatches`): every face index-identical with
/// owner and neighbour, patch names, types and sizes equal; the points differ
/// only by the `pow` ulp, pinned from above (measured 1777 points, worst
/// 1.776e-15 m, 2026-11-13).
#[test]
fn single_orifice_final_mesh_equals_upstream_up_to_the_pow_ulp() {
    let committed = upstream_dir().join("runs/singleOrifice/constant/polyMesh");
    let up = read_poly_mesh_dir(&committed, "singleOrifice_final").expect("committed upstream final mesh");
    let (m, types) = port_run(None);
    let (n_diff, worst, n_faces_bad) = deviation(&m, &up);
    println!("singleOrifice final: {n_diff} of {} points differ, worst {worst:.3e}; faces not index-identical {n_faces_bad}", m.points.len());
    assert_eq!(n_faces_bad, 0, "faces must be index-identical");
    assert!(n_diff <= 1777, "{n_diff} points differ (pinned at 1777)");
    assert!(worst <= 1.8e-15, "worst {worst:e} (pinned at 1.776e-15)");
    let boundary = String::from_utf8(gunzip(&std::fs::read(committed.join("boundary.gz")).unwrap())).unwrap();
    let want = boundary_entries(&boundary);
    let got: Vec<(String, String, usize)> = m.patches.iter().zip(&types).map(|(p, t)| (p.name.clone(), t.clone(), p.n_faces)).collect();
    assert_eq!(got, want, "patch names, types and sizes");
    assert_eq!(want.iter().map(|w| w.0.as_str()).collect::<Vec<_>>(), ["orificeRegion", "inlet", "outlet", "fixedWalls"]);
}

/// `untangleMeshFV`'s boundary loop follows upstream's calls by default
/// (gh:#1385 round 5): upstream passes a `bool` into `nIterations`.
#[test]
fn untangle_boundary_moves_follow_upstream_calls_by_default() {
    use outram_park_fork_cfmesh::mesh_optimizer::{BoundaryMoveCalls, UntangleControls};
    assert_eq!(UntangleControls::default().boundary_calls, BoundaryMoveCalls::Upstream);
    assert_eq!(BoundaryMoveCalls::default(), BoundaryMoveCalls::Upstream);
}
