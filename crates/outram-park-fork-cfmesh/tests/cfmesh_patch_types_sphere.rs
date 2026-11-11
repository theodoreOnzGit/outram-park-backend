//! gh:#1483: the surface's patch types reach the output mesh, on the sphere
//! of the #1385 differential, for the generators that take a 3D surface.
//!
//! Methodology: the committed sphere STL (`inputs/sphere_r3_24x48.stl.gz`)
//! with the regions an STL read gives (`sphere`, geometric type `empty`,
//! `triSurface` having set none), `maxCellSize 0.5`, then each of
//! `cartesian_mesh`, `tet_mesh` and `p_mesh`. The patch names and types are
//! compared with upstream cfMesh's `constant/polyMesh/boundary` for the same
//! case (`runs/sphere_<generator>/`, committed): one patch, `sphere`, type
//! `wall` (`renameBoundaryPatches::checkEmptyPatches`). A surface built
//! without types (`SurfaceRegions::single`) keeps `patch`, which the last
//! test pins as the documented API default.
//!
//! Results (2026-11-11, release): see
//! `verification_and_validation/patch_types_gh1483.md`.
use common::*;
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
use outram_park_fork_cfmesh::pipeline::p_mesh::p_mesh;
use outram_park_fork_cfmesh::pipeline::tet_mesh::{tet_mesh, TetMeshOptions};

mod common;

/// `(name, type)` of the patches in a `boundary` file's text.
fn boundary_names_and_types(text: &str) -> Vec<(String, String)> {
    let body = text.split("*/").nth(1).unwrap_or(text);
    let tokens: Vec<String> = body
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .flat_map(|l| l.replace(['{', '}', ';', '(', ')'], " ").split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect();
    let mut i = tokens.iter().position(|t| t == "object").expect("FoamFile header") + 2;
    let count: usize = tokens[i].parse().expect("patch count");
    i += 1;
    let mut out = Vec::new();
    for _ in 0..count {
        let name = tokens[i].clone();
        let mut ty = String::new();
        i += 1;
        while tokens[i] != "startFace" {
            if tokens[i] == "type" {
                ty = tokens[i + 1].clone();
            }
            i += 1;
        }
        i += 2;
        out.push((name, ty));
    }
    out
}

fn upstream_patches(run: &str) -> Vec<(String, String)> {
    let gz = std::fs::read(upstream_dir().join("runs").join(run).join("constant/polyMesh/boundary.gz")).unwrap();
    boundary_names_and_types(&String::from_utf8(gunzip(&gz)).unwrap())
}

fn stl_regions(n_tris: usize) -> SurfaceRegions {
    SurfaceRegions::single("sphere", n_tris).with_types(vec!["empty".to_string()])
}

fn port_patches(m: &outram_park_fork_cfmesh::volume_mesh::VolumeMesh, types: &[String]) -> Vec<(String, String)> {
    m.patches.iter().zip(types).map(|(p, t)| (p.name.clone(), t.clone())).collect()
}

#[test]
fn the_sphere_reference_is_one_wall_patch() {
    for run in ["sphere_cartesianMesh", "sphere_tetMesh", "sphere_pMesh"] {
        assert_eq!(upstream_patches(run), vec![("sphere".to_string(), "wall".to_string())], "{run}");
    }
}

#[test]
fn cartesian_mesh_writes_the_sphere_as_a_wall() {
    let (p, t) = read_sphere();
    let r = stl_regions(t.len());
    let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (m, rep) = cartesian_mesh(&p, &t, Some(&r), &opts).unwrap();
    assert_eq!(port_patches(&m, &rep.patch_types), upstream_patches("sphere_cartesianMesh"));
}

#[test]
fn tet_mesh_writes_the_sphere_as_a_wall() {
    let (p, t) = read_sphere();
    let r = stl_regions(t.len());
    let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (m, rep) = tet_mesh(&p, &t, Some(&r), &opts).unwrap();
    assert_eq!(port_patches(&m, &rep.patch_types), upstream_patches("sphere_tetMesh"));
}

#[test]
fn p_mesh_writes_the_sphere_as_a_wall() {
    let (p, t) = read_sphere();
    let r = stl_regions(t.len());
    let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (m, rep) = p_mesh(&p, &t, Some(&r), &opts).unwrap();
    assert_eq!(port_patches(&m, &rep.patch_types), upstream_patches("sphere_pMesh"));
}

/// Callers that pass no types (or no regions) keep the pre-gh:#1483 `patch`.
#[test]
fn a_surface_without_types_keeps_patch() {
    let (p, t) = read_sphere();
    let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (_, rep) = cartesian_mesh(&p, &t, Some(&SurfaceRegions::single("sphere", t.len())), &opts).unwrap();
    assert_eq!(rep.patch_types, vec!["patch".to_string()]);
    assert_eq!(SurfaceRegions::from_labels(&["a", "b"]).types, vec!["patch".to_string(); 2]);
    let r = SurfaceRegions::from_labels(&["a", "b"]).with_types(vec!["wall".into(), "empty".into()]);
    assert_eq!(r.types_for(3), vec!["wall".to_string(), "empty".into(), "patch".into()]);
    assert!(r.validate(2).is_ok());
    let short = SurfaceRegions { types: vec!["wall".into()], ..r };
    assert!(short.validate(2).is_err(), "one type for two names is refused");
}
