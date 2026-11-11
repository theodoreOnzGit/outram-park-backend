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
    if !step.is_empty() {
        dict += &format!("\nworkflowControls\n{{\n    stopAfter {step};\n}}\n");
    }
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
fn staged(tutorial: &str, generator: Generator, short: &str, run_prefix: &str) -> Vec<(String, bool, bool)> {
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
        let port = port_stage(tutorial, generator, step);
        let (line, same) = compare(&port, &up);
        println!("{run_prefix} after {step}: {line}");
        let counts = (port.points.len(), port.faces.len(), port.n_cells, port.n_internal_faces) == (up.points.len(), up.faces.len(), up.n_cells, up.n_internal_faces);
        out.push((step.to_string(), same, counts));
    }
    out
}

fn matched(r: &[(String, bool, bool)], step: &str) -> Option<bool> {
    r.iter().find(|(s, _, _)| s == step).map(|(_, same, _)| *same)
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
/// | meshOptimisation | 52997 cells vs 53003 | counts equal, faces identical, 71542/71650 points bit-identical, worst 4.9e-4 m; **round 3: 71649/71650, worst 4.1e-25 m** (the pow ulp, see `asmo_octree_mesh_optimisation_is_within_one_pow_ulp`) |
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

/// The points of `port` that differ from `up`'s in any bit, with the worst
/// absolute difference (counts must agree).
fn differing_points(port: &PolyMesh, up: &PolyMesh) -> (Vec<usize>, f64) {
    let bad: Vec<usize> = (0..up.points.len())
        .filter(|&i| port.points[i].x.to_bits() != up.points[i].x.to_bits() || port.points[i].y.to_bits() != up.points[i].y.to_bits() || port.points[i].z.to_bits() != up.points[i].z.to_bits())
        .collect();
    let worst = bad.iter().map(|&i| (port.points[i].x - up.points[i].x).abs().max((port.points[i].y - up.points[i].y).abs()).max((port.points[i].z - up.points[i].z).abs())).fold(0.0f64, f64::max);
    (bad, worst)
}

/// The upstream mesh of the whole run (no `stopAfter`), read from
/// `GH1385_UPSTREAM_RUNS/<run>/constant/polyMesh`; `None` when absent.
fn upstream_final(run: &str) -> Option<PolyMesh> {
    let loose = runs_dir().join(run).join("constant/polyMesh");
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("up_final_{run}"));
    std::fs::create_dir_all(&dst).unwrap();
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        if !loose.join(f).is_file() {
            return None;
        }
        let text = std::fs::read_to_string(loose.join(f)).unwrap();
        std::fs::write(dst.join(f), if f == "neighbour" { internal_neighbours_only(&text) } else { text }).unwrap();
    }
    Some(PolyMesh::read(&dst).expect("read upstream polyMesh"))
}

/// `asmoOctree`, `meshOptimisation` (gh:#1385 round 3): counts, faces and
/// owners/neighbours identical, and **all but one point bit-identical**.
///
/// # Result (2026-11-11, release)
///
/// 71649/71650 points bit-identical (was 71542/71650, worst 4.944e-4 m); the
/// one difference is point 51898, `y = 2.3293821067756857e-9` against
/// upstream's `2.3293821067756861e-9` (4.1e-25 m, 1 ulp of that coordinate).
/// **Cause, proven** (traces of both codes, `GH1385_BP=2069`): the difference
/// first appears in the last `smoothSurfaceOptimizer` pass of
/// `optimiseFinalMesh`'s `optimizeSurface`, where the Hessian diagonal of
/// `surfaceOptimizer::evaluateGradients` differs by 1 ulp, because
/// `pow(stab, 3)` differs by 1 ulp for `stab = 0.03166163363177029`: this
/// machine's glibc 2.39 `pow` is its FMA build (ifunc), PETIR's `powf` is the
/// non-FMA ARM routine by design (`petir::fast_exp` module docs: it tracks
/// the no-FMA row on purpose, glibc's FMA variant differs on ~0.05 % of
/// inputs by 1 ulp). Reproduced outside both codes: Python's `math.pow`
/// (glibc) in upstream's operation order gives upstream's Hessian
/// (886.6570495551824, 1224.6298733174792); `x*x*x` gives the port's
/// (...823, ...479). Below 1e-8 relative with a known round-off cause, so
/// pinned from above rather than closed: the count may not fall and the
/// difference may not grow. **Flagged for the maintainer.**
#[test]
fn asmo_octree_mesh_optimisation_is_within_one_pow_ulp() {
    let Some(up) = upstream_stage("asmoOctree_cartesianMesh", "stage_asmo_cartesianMesh", "meshOptimisation") else {
        println!("no upstream meshOptimisation stage mesh (set GH1385_UPSTREAM_RUNS); skipped");
        return;
    };
    let port = port_stage("cartesianMesh/asmoOctree", Generator::CartesianMesh, "meshOptimisation");
    let (line, _) = compare(&port, &up);
    println!("{line}");
    assert_eq!((port.points.len(), port.faces.len(), port.n_cells), (up.points.len(), up.faces.len(), up.n_cells));
    let (bad, worst) = differing_points(&port, &up);
    assert!(bad.len() <= 1, "at most the one pow-ulp point may differ, got {} ({line})", bad.len());
    assert!(worst <= 4.2e-25, "worst difference {worst:e}");
    let faces_same = (0..up.faces.len()).all(|i| port.faces[i].verts == up.faces[i].verts && port.faces[i].owner == up.faces[i].owner && port.faces[i].neighbour == up.faces[i].neighbour);
    assert!(faces_same, "faces must be index-identical");
}

/// `asmoOctree`, the whole run (after `renumberMesh` and
/// `renameBoundaryPatches`) against `GH1385_UPSTREAM_RUNS/asmoOctree`
/// (53 003 cells; not committed, about 12 MB uncompressed).
#[test]
fn asmo_octree_final_mesh() {
    let Some(up) = upstream_final("asmoOctree") else {
        println!("no upstream final mesh (set GH1385_UPSTREAM_RUNS); skipped");
        return;
    };
    let port = port_stage("cartesianMesh/asmoOctree", Generator::CartesianMesh, "");
    let (line, same) = compare(&port, &up);
    println!("asmoOctree final: {line} (identical: {same})");
    assert_eq!((port.points.len(), port.faces.len(), port.n_cells), (up.points.len(), up.faces.len(), up.n_cells));
    let (bad, worst) = differing_points(&port, &up);
    assert!(bad.len() <= 1 && worst <= 4.2e-25, "{} points differ, worst {worst:e}", bad.len());
    let faces_same = (0..up.faces.len()).all(|i| port.faces[i].verts == up.faces[i].verts && port.faces[i].owner == up.faces[i].owner && port.faces[i].neighbour == up.faces[i].neighbour);
    assert!(faces_same, "faces must be index-identical");
}

/// The whole run of `tutorial` through `generator` against
/// `GH1385_UPSTREAM_RUNS/<run>` (final meshes, after `renumberMesh`): prints
/// the comparison line, returns `(counts equal, bit-identical)`; `None` when
/// the upstream run is absent. **A status record, not a gate** (gh:#1385,
/// round 3: no staged upstream runs exist for these cases yet).
fn final_status(tutorial: &str, generator: Generator, run: &str) -> Option<(bool, bool)> {
    let up = upstream_final(run)?;
    let port = port_stage(tutorial, generator, "");
    let (line, same) = compare(&port, &up);
    println!("{run} final: {line} (identical: {same})");
    Some(((port.points.len(), port.faces.len(), port.n_cells) == (up.points.len(), up.faces.len(), up.n_cells), same))
}

/// `cutCube` (tetMesh `cutCubeOctree`'s surface) through the three generators,
/// and `hatOctree` through `cartesian2DMesh`, final meshes against upstream
/// (`runs/cutCube_{tetMesh,pMesh,cartesianMesh}`, `runs/hatOctree`).
///
/// # Status (2026-11-11, release, V&V README round 3)
///
/// Printed by this test; see the README for the numbers. These runs are not
/// staged yet (`workflowControls stopAfter` runs of upstream for each step,
/// as for the sphere and asmoOctree), so a difference cannot be localised
/// from this test alone: the generator-level traces (`GH1385_TRACE`) are the
/// way.
#[test]
fn cut_cube_and_hat_octree_final_meshes_status() {
    for (tutorial, generator, run) in [
        ("tetMesh/cutCubeOctree", Generator::TetMesh, "cutCube_tetMesh"),
        ("tetMesh/cutCubeOctree", Generator::PMesh, "cutCube_pMesh"),
        ("tetMesh/cutCubeOctree", Generator::CartesianMesh, "cutCube_cartesianMesh"),
        ("cartesian2DMesh/hatOctree", Generator::Cartesian2DMesh, "hatOctree"),
    ] {
        match final_status(tutorial, generator, run) {
            Some((counts, same)) => println!("{run}: counts equal {counts}, bit-identical {same}"),
            None => println!("{run}: no upstream run (set GH1385_UPSTREAM_RUNS); skipped"),
        }
    }
}

/// `elbow_90degree` (cartesianMesh, `minCellSize`, `boundaryLayers`,
/// `renameBoundary`), the port against cfMesh `e944f6ba695f` stopped after
/// each of the eight steps (`GH1385_UPSTREAM_RUNS/stage_elbow_cartesianMesh_<step>`;
/// not committed, about 3.5 MB each; skipped when absent).
///
/// # Results (2026-11-11, release; V&V README, "round 3")
///
/// | step | round 2 | now (PETIR `powf`) | now, with this machine's glibc `pow` (experiment, not committed) |
/// |---|---|---|---|
/// | templateGeneration | 21830 cells vs 21977 | identical | identical |
/// | surfaceTopology, surfaceProjection, patchAssignment | not reached | identical | identical |
/// | edgeExtraction | | counts equal, 34255/34508 points, worst 1.4e-14 m | identical |
/// | boundaryLayerGeneration | | counts equal, 48845/49382 points | identical |
/// | meshOptimisation | 43606/49382 points, worst 0.21 m (3 bad faces vs 2) | counts equal, 43613/49382 points, worst 6.6e-5 m | identical |
/// | boundaryLayerRefinement | | counts equal (108 995 cells); 107 600/121 398 points, worst 6.6e-5 m; faces not index-identical | 121 333/121 398 points, worst 2.7e-15 m; faces not index-identical |
///
/// **The remaining differences have two causes.** (1) The last-bit
/// differences of `edgeExtraction` onward are the `pow(stab, 3)` of
/// `surfaceOptimizer::evaluateGradients`: glibc's FMA `pow` against PETIR's
/// non-FMA ARM `pow` (proof in `asmo_octree_mesh_optimisation_is_within_one_pow_ulp`
/// and the README). ~~(2) The final mesh's face order differs because
/// `refineBoundaryLayers` returns the refined mesh through
/// `from_cell_faces_wound`, not in upstream's assembly order.~~ **FIXED
/// 2026-11-11 (gh:#1385 round 4):** `refineBoundaryLayers` assembles the mesh
/// as upstream does (original internal pieces, new internal faces in creation
/// order, boundary faces per patch; the first child keeps the label, the others
/// are appended; prism children from the far side; `refineEdgeHexCell`'s order),
/// and the final mesh's **339 265 faces are index-identical** (ring, owner,
/// neighbour; the in-flight mesh before the untangler too, checked against the
/// `RFF` trace). With this machine's glibc `pow` swapped in (experiment, not
/// committed) every stage and the final mesh are bit-identical: 121 398/121 398
/// points, so the 2.7e-15 m of round 3 was the `pow` ulp as well.
///
/// Asserted: bit-identical through `patchAssignment`, equal counts at every
/// step, and (`elbow_90degree_refined_faces_are_index_identical`) the final
/// mesh's faces index-identical.
#[test]
fn elbow_90degree_cartesian_mesh_stages() {
    let r = staged("cartesianMesh/elbow_90degree", Generator::CartesianMesh, "elbow_cartesianMesh", "stage_elbow_cartesianMesh");
    println!("{r:?}");
    for step in ["templateGeneration", "surfaceTopology", "surfaceProjection", "patchAssignment"] {
        if let Some(same) = matched(&r, step) {
            assert!(same, "elbow after {step} must be bit-identical");
        }
    }
    for (step, _, counts) in &r {
        assert!(*counts, "elbow after {step}: counts must equal upstream's");
    }
}

/// The elbow's final mesh (after `refineBoundaryLayers`, `renumberMesh`):
/// every face index-identical to upstream's (ring, owner, neighbour) and the
/// patch sizes equal; the points differ only by the `pow` ulp (see
/// `elbow_90degree_cartesian_mesh_stages`). The upstream faces, owner,
/// neighbour and boundary are committed (gzipped).
#[test]
fn elbow_90degree_refined_faces_are_index_identical() {
    // The committed upstream files are faces, owner, neighbour and boundary
    // (3.0 MB gzipped; the points would add 3.3 MB). The reader wants points:
    // the port's own are put beside them, and only faces are compared.
    let committed = committed_stage("elbow_cartesianMesh", "boundaryLayerRefinement").join("constant/polyMesh");
    let port = port_stage("cartesianMesh/elbow_90degree", Generator::CartesianMesh, "boundaryLayerRefinement");
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("up_elbow_refined_faces");
    std::fs::create_dir_all(&dst).unwrap();
    for f in ["faces", "owner", "neighbour", "boundary"] {
        let text = String::from_utf8(gunzip(&std::fs::read(committed.join(format!("{f}.gz"))).expect("committed upstream stage file"))).unwrap();
        std::fs::write(dst.join(f), if f == "neighbour" { internal_neighbours_only(&text) } else { text }).unwrap();
    }
    let port_points = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("stage_cartesianMesh_elbow_90degree_boundaryLayerRefinement/constant/polyMesh/points");
    std::fs::copy(&port_points, dst.join("points")).expect("the port's points file");
    let up = PolyMesh::read(&dst).expect("read upstream faces");
    assert_eq!((port.points.len(), port.faces.len(), port.n_cells, port.n_internal_faces), (up.points.len(), up.faces.len(), up.n_cells, up.n_internal_faces));
    let bad = (0..up.faces.len()).filter(|&i| port.faces[i].verts != up.faces[i].verts || port.faces[i].owner != up.faces[i].owner || port.faces[i].neighbour != up.faces[i].neighbour).count();
    assert_eq!(bad, 0, "{bad} faces differ");
    let sizes = |m: &PolyMesh| m.patches.iter().map(|p| p.size).collect::<Vec<_>>();
    assert_eq!(sizes(&port), sizes(&up));
}

/// `cutCube` through `pMesh`, the port against cfMesh stopped after each step
/// (`GH1385_UPSTREAM_RUNS/stage_cutCube_pMesh_<step>`, gh:#1385 round 4):
/// prints the stage table; the asserts are in the round-4 README.
#[test]
fn cut_cube_p_mesh_stages() {
    let r = staged("tetMesh/cutCubeOctree", Generator::PMesh, "cutCube_pMesh", "stage_cutCube_pMesh");
    println!("{r:?}");
}

/// `hatOctree` through `cartesian2DMesh`, the port against cfMesh stopped after
/// each step (`GH1385_UPSTREAM_RUNS/stage_hatOctree_cartesian2DMesh_<step>`;
/// not committed, skipped when absent).
///
/// # Results (2026-11-11, release; V&V README, "round 4")
///
/// | step | before | after |
/// |---|---|---|
/// | templateGeneration, surfaceTopology | identical | identical |
/// | surfaceProjection | 29008/29498 points, worst 7.8e-4 m | identical |
/// | patchAssignment ... meshOptimisation | 25460/29498 ... 22662/30890 | identical |
/// | boundaryLayerRefinement (final) | 25342/38090 points, faces 1091/75072 | **38090/38090 points, 75072/75072 faces bit-identical** |
///
/// Causes: (1) `meshSurfaceMapper2D::preMapVertices` defaults to **2**
/// iterations (the port ran 3, the 3D routine's default); (2) the template's
/// `createEmptyPatches` and `distributeBoundaryFaces` are `replaceBoundary`
/// calls: the internal faces stay in each cell's list and the new boundary
/// labels are appended in the old boundary order, so the carried cell lists
/// (which the final `renumberMesh` walk reads) were dropped and rebuilt
/// ascending. No pow ulp on this case.
#[test]
fn hat_octree_cartesian_2d_mesh_stages() {
    let r = staged("cartesian2DMesh/hatOctree", Generator::Cartesian2DMesh, "hatOctree_cartesian2DMesh", "stage_hatOctree_cartesian2DMesh");
    println!("{r:?}");
    for (step, same, counts) in &r {
        assert!(*counts, "hat after {step}: counts must equal upstream's");
        assert!(*same, "hat after {step} must be bit-identical (points and faces)");
    }
}
