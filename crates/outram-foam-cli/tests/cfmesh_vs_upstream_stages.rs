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

const STEPS: [&str; 8] = [
    "templateGeneration",
    "surfaceTopology",
    "surfaceProjection",
    "patchAssignment",
    "edgeExtraction",
    "boundaryLayerGeneration",
    "meshOptimisation",
    "boundaryLayerRefinement",
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

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b && bytes[3] == 0, "gzip -n expected");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn committed_stage(short: &str, step: &str) -> PathBuf {
    crate_dir().join("verification_and_validation/upstream_cfmesh_gh1385/runs/stages").join(format!("{short}_{step}"))
}

/// The upstream `polyMesh` of a stage run: the committed gzipped one
/// (`runs/stages/<short>_<step>`) if there is one, else the uncompressed run
/// in `GH1385_UPSTREAM_RUNS` (`<run_prefix>_<step>`); `None` when neither exists.
fn upstream_stage(short: &str, run_prefix: &str, step: &str) -> Option<PolyMesh> {
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("up_{short}_{step}"));
    std::fs::create_dir_all(&dst).unwrap();
    let committed = committed_stage(short, step).join("constant/polyMesh");
    let loose = runs_dir().join(format!("{run_prefix}_{step}")).join("constant/polyMesh");
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
    Some(PolyMesh::read(&dst).expect("read upstream polyMesh"))
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
        let bad: Vec<usize> = (0..up.points.len()).filter(|&i| port.points[i].x.to_bits() != up.points[i].x.to_bits() || port.points[i].y.to_bits() != up.points[i].y.to_bits() || port.points[i].z.to_bits() != up.points[i].z.to_bits()).collect();
        if !bad.is_empty() {
            let n = up.points.len();
            let mut deciles = [0usize; 10];
            for &i in &bad {
                deciles[i * 10 / n] += 1;
            }
            line += &format!("\n    differing points per tenth of the index range: {deciles:?}; largest |d| at point {}", bad.iter().copied().max_by(|&a, &b| {
                let d = |i: usize| (port.points[i].x - up.points[i].x).abs().max((port.points[i].y - up.points[i].y).abs()).max((port.points[i].z - up.points[i].z).abs());
                d(a).partial_cmp(&d(b)).unwrap()
            }).unwrap());
        }
        for &i in bad.iter().take(4) {
            line += &format!("\n    point {i}: port ({:?}, {:?}, {:?}) upstream ({:?}, {:?}, {:?})", port.points[i].x, port.points[i].y, port.points[i].z, up.points[i].x, up.points[i].y, up.points[i].z);
        }
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
    let sizes = |m: &PolyMesh| m.patches.iter().map(|p| p.size).collect::<Vec<_>>();
    if sizes(port) != sizes(up) {
        line += &format!("\n    patch sizes port {:?} upstream {:?}", sizes(port), sizes(up));
        identical = false;
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
    let run = run_cfmesh(&dir, generator).unwrap();
    for n in run.notes.iter().filter(|n| n.starts_with(step)) {
        println!("  note: {n}");
    }
    // GH1385_CELLS=<trace.err> compares the carried cell lists with upstream's `CELL` dump
    if let Ok(path) = std::env::var("GH1385_CELLS") {
        let text = std::fs::read_to_string(path).unwrap();
        let up: Vec<Vec<usize>> = text.lines().filter(|l| l.starts_with("CELL ")).map(|l| l.split_whitespace().skip(2).map(|x| x.parse().unwrap()).collect()).collect();
        let mine = run.mesh.cells.clone().unwrap_or_default();
        let same = mine.iter().zip(&up).filter(|(a, b)| a == b).count();
        println!("  cell lists: port {} (carried {}), upstream {}, identical {same}", mine.len(), run.mesh.cells.is_some(), up.len());
        for (c, (a, b)) in mine.iter().zip(&up).enumerate().filter(|(_, (a, b))| a != b).take(3) {
            println!("    cell {c}: port {a:?} upstream {b:?}");
        }
    }
    PolyMesh::read(&dir.join("constant/polyMesh")).expect("read port polyMesh")
}

/// `(step, bit-identical, port mesh vs upstream line)` for every stage that has an upstream mesh.
fn staged(tutorial: &str, generator: Generator, short: &str, run_prefix: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for step in STEPS {
        // GH1385_ONLY=<step> runs one stage (for traces)
        if std::env::var("GH1385_ONLY").is_ok_and(|o| o != step) {
            continue;
        }
        let Some(up) = upstream_stage(short, run_prefix, step) else {
            println!("{run_prefix} after {step}: no upstream stage mesh (not committed; set GH1385_UPSTREAM_RUNS); skipped");
            continue;
        };
        let (line, same) = compare(&port_stage(tutorial, generator, step), &up);
        println!("{run_prefix} after {step}: {line}");
        out.push((step.to_string(), same));
    }
    out
}

fn matched(r: &[(String, bool)], step: &str) -> Option<bool> {
    r.iter().find(|(s, _)| s == step).map(|(_, same)| *same)
}

/// `asmoOctree` (cartesianMesh, 10 patches, local refinement): the port
/// against cfMesh `e944f6ba695f` stopped after each step.
///
/// # Results (2026-11-11, release)
///
/// | step | before the fixes | after |
/// |---|---|---|
/// | templateGeneration, surfaceTopology | identical | identical |
/// | surfaceProjection | 49577/49690 points (last bit) | identical |
/// | patchAssignment | 49541/49690, worst 6.3e-6 m | identical |
/// | edgeExtraction | 32413 cells vs 32419, 114400 faces vs 114411 | identical (points, faces) |
/// | boundaryLayerGeneration | 52997 cells vs 53003 | identical (points, faces, the 53003 cell lists) |
/// | meshOptimisation | 52997 cells vs 53003 | counts equal, faces identical, 71542/71650 points bit-identical, worst 4.9e-4 m |
///
/// The committed upstream stage meshes are `edgeExtraction` and
/// `boundaryLayerGeneration` (1.8 and 3.0 MB gzipped); the others are read from
/// `GH1385_UPSTREAM_RUNS` when it holds them (`scripts/run-case.sh`), else
/// skipped. The fixes: V&V README, "asmoOctree and elbow_90degree".
#[test]
fn asmo_octree_cartesian_mesh_stages() {
    let r = staged("cartesianMesh/asmoOctree", Generator::CartesianMesh, "asmoOctree_cartesianMesh", "stage_asmo_cartesianMesh");
    println!("{r:?}");
    for step in ["templateGeneration", "surfaceTopology", "surfaceProjection", "patchAssignment", "edgeExtraction", "boundaryLayerGeneration"] {
        if let Some(same) = matched(&r, step) {
            assert!(same, "asmoOctree after {step} must be bit-identical");
        }
    }
    assert!(matched(&r, "edgeExtraction").is_some() && matched(&r, "boundaryLayerGeneration").is_some(), "the committed stage meshes must be found");
}

/// `elbow_90degree` (cartesianMesh, `minCellSize`, `boundaryLayers`): **not
/// matched.** The first difference is `templateGeneration` (21830 cells vs
/// 21977): upstream's `minCellSize` runs the octree's automatic refinement
/// (`meshOctreeAutomaticRefinement`), which the port's octree refines
/// differently on this surface; everything after inherits it (final 108383
/// cells vs 108995). Printed only; no upstream stage mesh is committed.
#[test]
fn elbow_90degree_cartesian_mesh_stages() {
    let r = staged("cartesianMesh/elbow_90degree", Generator::CartesianMesh, "elbow_cartesianMesh", "stage_elbow_cartesianMesh");
    println!("{r:?}");
}
