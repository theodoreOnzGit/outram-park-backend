// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under the GNU General Public
// License v3; see the crate's LICENSE files.

//! gh:#1483: the cfMesh generators carry the surface's patch types into
//! `constant/polyMesh/boundary`, as upstream does.
//!
//! Methodology: run `outram_foam_cli::cfmesh::run_cfmesh` on a copy of the
//! unmodified upstream tutorial case (cfMesh `e944f6ba695f`, copied to
//! `crates/outram-park-fork-cfmesh/test_data/cfmesh_tutorials/`), read the
//! `boundary` file it wrote, and compare patch names **and types**, in order,
//! with the `boundary` upstream cfMesh wrote for the same case (committed,
//! gzipped, in `verification_and_validation/upstream_cfmesh_gh1385/runs/`).
//! Face counts are printed, not compared: only the sphere is bit-identical to
//! upstream so far (gh:#1385). Pass criterion: names and types equal.
//!
//! Results (2026-11-11, release): see
//! `verification_and_validation/patch_types_gh1483.md`.

use outram_foam_cli::cfmesh::run_cfmesh;
use outram_park_fork_cfmesh::mesh_dict::Generator;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../outram-park-fork-cfmesh")
}

/// A copy of `test_data/cfmesh_tutorials/<tutorial>` under the temp directory.
struct Case(PathBuf);

impl Case {
    fn new(tag: &str, tutorial: &str) -> Self {
        let src = crate_dir().join("test_data/cfmesh_tutorials").join(tutorial);
        let dir = std::env::temp_dir().join(format!("cfmesh_types_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("system")).unwrap();
        for e in std::fs::read_dir(&src).unwrap() {
            let e = e.unwrap();
            if e.path().is_file() {
                std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
            }
        }
        std::fs::copy(src.join("system/meshDict"), dir.join("system/meshDict")).unwrap();
        Case(dir)
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `(name, type, nFaces)` of each patch of a `boundary` file's text: the
/// tokens after the patch-count line, `name { type t; nFaces n; startFace s; }`.
fn boundary_patches(text: &str) -> Vec<(String, String, usize)> {
    let body = text.split("*/").nth(1).unwrap_or(text);
    let tokens: Vec<String> = body
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .flat_map(|l| l.replace(['{', '}', ';', '(', ')'], " ").split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect();
    // header dictionary (FoamFile ... object boundary) comes first, then the count
    let obj = tokens.iter().position(|t| t == "object").expect("FoamFile header");
    let mut i = obj + 2;
    let count: usize = tokens[i].parse().expect("patch count");
    i += 1;
    let mut out = Vec::new();
    for _ in 0..count {
        let name = tokens[i].clone();
        i += 1;
        let (mut ty, mut n) = (String::new(), 0);
        while tokens[i] != "startFace" {
            match tokens[i].as_str() {
                "type" => ty = tokens[i + 1].clone(),
                "nFaces" => n = tokens[i + 1].parse().unwrap(),
                _ => {}
            }
            i += 1;
        }
        i += 2;
        out.push((name, ty, n));
    }
    out
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b && bytes[3] == 0, "gzip -n expected");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn upstream_boundary(run: &str) -> Vec<(String, String, usize)> {
    let p = crate_dir().join("verification_and_validation/upstream_cfmesh_gh1385/runs").join(run).join("constant/polyMesh/boundary.gz");
    boundary_patches(&String::from_utf8(gunzip(&std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))).unwrap())
}

fn check(tag: &str, tutorial: &str, generator: Generator, run: &str) {
    let c = Case::new(tag, tutorial);
    let r = run_cfmesh(&c.0, generator).unwrap();
    let ours = boundary_patches(&std::fs::read_to_string(c.0.join("constant/polyMesh/boundary")).unwrap());
    let theirs = upstream_boundary(run);
    println!("{tag}: {} cells", r.mesh.n_cells);
    for (a, b) in ours.iter().zip(theirs.iter()) {
        println!("  port {:<22} {:<10} {:>7} faces | upstream {:<22} {:<10} {:>7} faces", a.0, a.1, a.2, b.0, b.1, b.2);
    }
    let key = |v: &[(String, String, usize)]| v.iter().map(|p| (p.0.clone(), p.1.clone())).collect::<Vec<_>>();
    assert_eq!(key(&ours), key(&theirs), "{tag}: patch names and types");
    // the report's types are what was written
    assert_eq!(r.patch_types, ours.iter().map(|p| p.1.clone()).collect::<Vec<_>>());
}

/// STL solids carry `empty`, which `renameBoundaryPatches` turns into `wall`.
#[test]
fn asmo_octree_cartesian_mesh_writes_wall_patches() {
    check("asmo", "cartesianMesh/asmoOctree", Generator::CartesianMesh, "asmoOctree");
}

#[test]
fn cut_cube_tet_mesh_writes_wall_patches() {
    check("cutcube", "tetMesh/cutCubeOctree", Generator::TetMesh, "cutCube_tetMesh");
}

/// `.fms` types (`wall`) and the two `empty` patches of the 2D generator.
#[test]
fn hat_octree_cartesian_2d_mesh_writes_wall_and_empty_patches() {
    check("hat", "cartesian2DMesh/hatOctree", Generator::Cartesian2DMesh, "hatOctree");
}

/// `renameBoundary` gives `patch`, `patch`, `wall`: unchanged by the fix.
#[test]
fn elbow_cartesian_mesh_keeps_its_renamed_types() {
    check("elbow", "cartesianMesh/elbow_90degree", Generator::CartesianMesh, "elbow_90degree");
}

#[test]
fn boundary_reader_reads_the_committed_upstream_files() {
    let b = upstream_boundary("sphere_cartesianMesh");
    assert_eq!(b, vec![("sphere".to_string(), "wall".to_string(), 528)]);
    let h = upstream_boundary("hatOctree");
    assert_eq!(h.len(), 7);
    assert_eq!(h[5], ("bottomEmptyFaces".to_string(), "empty".to_string(), 18676));
}
