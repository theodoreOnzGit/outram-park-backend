//! gh:#1385: the cause of the last-bit difference of `cartesianMesh`'s
//! meshOptimisation on the sphere, proven.
//!
//! # Method
//!
//! `meshOptimizer::optimizeMeshFV` starts with 5 iterations of
//! `laplaceSmoother::laplacianPC` on the inside points: each moves to the
//! average of the centres of the cells around it. The centre of a cell
//! (`polyMeshGenAddressing::makeCellCentresAndVols`) sums over **the cell's
//! faces in the order of its cell list**, which is the history of the mesh
//! (faces in discovery order, `reorderBoundaryFaces` swaps labels without
//! touching the lists, `replaceBoundary` appends boundary faces). 491 of the
//! 1128 cells of the layered sphere have a face list that is not ascending in
//! face label (cell 433: `1461 1513 1529 1534 1535 0`). `VolumeMesh` has no
//! cell lists, and the port sums in ascending face order.
//!
//! This test replays the 5 Laplacian iterations with the port's operations
//! (`pmg_face_centre_and_area`, the cell sums of
//! `foam_cell_centres_and_vols`) but over **upstream's own cell lists**, dumped
//! from an instrumented upstream run
//! (`traces/sphere_cartesianMesh_meshOptimisation_cells_surface_fv.txt.gz`:
//! the `CELL` lines, the points before `optimizeMeshFV` (`fm_surface`) and
//! after it (`fm_fv`); patch `patches/gh1385-trace-instrumentation.patch`).
//! Input: the port's mesh after `boundaryLayerGeneration` (bit-identical to
//! upstream's) and `optimizeSurface` (bit-identical to `fm_surface`).
//!
//! # Result (2026-11-11)
//!
//! With the cell lists upstream's: all 1425 points of `fm_fv` bit-identical.
//! With ascending face order (what the port does): 623 of 1425, worst
//! 1.332e-15. So the cell face order is the whole of the difference.
use common::*;
use outram_park_fork_cfmesh::checks::pmg_face_centre_and_area;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
use outram_park_fork_cfmesh::pipeline::tet_mesh::create_octree;
use outram_park_fork_cfmesh::surface_optimizer::optimise_mesh_surface_octree;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use outram_park_fork_cfmesh::workflow_controls::{WorkflowSettings, WorkflowStep};
use std::collections::BTreeMap;

mod common;

/// `makeCellCentresAndVols` over the given cell face lists.
fn centres_with_cell_lists(mesh: &VolumeMesh, cells: &[Vec<usize>]) -> Vec<Vec3> {
    let (fc, fa): (Vec<Vec3>, Vec<Vec3>) = mesh.faces.iter().map(|r| pmg_face_centre_and_area(&mesh.points, r)).unzip();
    cells
        .iter()
        .enumerate()
        .map(|(c, faces)| {
            let mut est = Vec3::ZERO;
            for &f in faces {
                est = est.add(fc[f]);
            }
            let est = est.div(faces.len() as f64);
            let (mut centre, mut vol) = (Vec3::ZERO, 0.0);
            for &f in faces {
                let mut p3 = fa[f].dot(fc[f].sub(est));
                if mesh.owner[f] != c {
                    p3 *= -1.0;
                }
                p3 = p3.max(1e-300);
                let pc = fc[f].scale(3.0 / 4.0).add(est.scale(1.0 / 4.0));
                centre = centre.add(pc.scale(p3));
                vol += p3;
            }
            centre.div(vol)
        })
        .collect()
}

/// `laplacianPC`, `n` iterations, on the inside points.
fn laplacian_pc(mesh: &mut VolumeMesh, cells: &[Vec<usize>], n: usize) {
    let mut on_boundary = vec![false; mesh.points.len()];
    for f in mesh.n_internal_faces()..mesh.face_count() {
        for &v in &mesh.faces[f] {
            on_boundary[v] = true;
        }
    }
    // pointCells: cells in ascending order
    let mut pc: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for (c, faces) in cells.iter().enumerate() {
        for &f in faces {
            for &v in &mesh.faces[f] {
                if pc[v].last() != Some(&c) {
                    pc[v].push(c);
                }
            }
        }
    }
    for _ in 0..n {
        let centres = centres_with_cell_lists(mesh, cells);
        for p in 0..mesh.points.len() {
            if on_boundary[p] || pc[p].is_empty() {
                continue;
            }
            let mut new_p = Vec3::ZERO;
            for &c in &pc[p] {
                new_p = new_p.add(centres[c]);
            }
            mesh.points[p] = new_p.div(pc[p].len() as f64);
        }
    }
}

fn load_trace() -> (Vec<Vec<usize>>, Vec<Vec3>, Vec<Vec3>) {
    let gz = std::fs::read(upstream_dir().join("traces/sphere_cartesianMesh_meshOptimisation_cells_surface_fv.txt.gz")).unwrap();
    let text = String::from_utf8(gunzip(&gz)).unwrap();
    let mut cells: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let (mut surface, mut fv): (BTreeMap<usize, Vec3>, BTreeMap<usize, Vec3>) = (BTreeMap::new(), BTreeMap::new());
    for l in text.lines() {
        let t: Vec<&str> = l.split_whitespace().collect();
        if t[0] == "CELL" {
            cells.insert(t[1].parse().unwrap(), t[2..].iter().map(|x| x.parse().unwrap()).collect());
        } else if t[0] == "TRACE" {
            let p = Vec3::new(t[4].parse().unwrap(), t[5].parse().unwrap(), t[6].parse().unwrap());
            let i: usize = t[3].parse().unwrap();
            if t[1] == "fm_surface" {
                surface.insert(i, p);
            } else if t[1] == "fm_fv" {
                fv.insert(i, p);
            }
        }
    }
    (cells.into_values().collect(), surface.into_values().collect(), fv.into_values().collect())
}

fn identical(a: &[Vec3], b: &[Vec3]) -> usize {
    a.iter().zip(b).filter(|(p, q)| p.x.to_bits() == q.x.to_bits() && p.y.to_bits() == q.y.to_bits() && p.z.to_bits() == q.z.to_bits()).count()
}

#[test]
fn laplacian_over_upstream_cell_lists_is_bit_identical() {
    let (p, t) = read_sphere();
    let dict = MeshDict::with_max_cell_size(0.5);
    let opts = CartesianMeshGeneratorOptions {
        dict: dict.clone(),
        workflow: WorkflowSettings { stop_after: Some(WorkflowStep::BoundaryLayerGeneration), restart_from_latest_step: false },
        ..Default::default()
    };
    let (mut m, _) = cartesian_mesh(&p, &t, None, &opts).unwrap();
    let o = create_octree(&p, &t, None, &dict);
    let reg = SurfaceRegions::single("defaultFaces", t.len());
    let given: Vec<Option<usize>> = (0..m.faces.len()).map(|f| m.neighbour[f].is_none().then_some(0)).collect();
    optimise_mesh_surface_octree(&mut m, &o, &p, &t, &reg, &given);
    let (cells, surface, fv) = load_trace();
    assert_eq!(cells.len(), m.n_cells);
    assert_eq!(identical(&m.points, &surface), surface.len(), "optimizeSurface must match fm_surface");
    let ascending: Vec<Vec<usize>> = {
        let mut c = vec![Vec::new(); m.n_cells];
        for f in 0..m.face_count() {
            c[m.owner[f]].push(f);
            if let Some(n) = m.neighbour[f] {
                c[n].push(f);
            }
        }
        c
    };
    let n_not_ascending = cells.iter().filter(|c| c.windows(2).any(|w| w[0] > w[1])).count();
    let mut with_upstream = m.clone();
    laplacian_pc(&mut with_upstream, &cells, 5);
    let mut with_ascending = m.clone();
    laplacian_pc(&mut with_ascending, &ascending, 5);
    let (a, b) = (identical(&with_upstream.points, &fv), identical(&with_ascending.points, &fv));
    println!("cells with a non-ascending face list: {n_not_ascending} of {}", cells.len());
    println!("laplacianPC x5 over upstream's cell lists: {a}/{} bit-identical to fm_fv; over ascending lists: {b}", fv.len());
    assert_eq!(n_not_ascending, 491);
    assert_eq!(a, fv.len());
    assert_eq!(b, 623);
}

#[test]
fn carried_state_per_stage() {
    let (p, t) = read_sphere();
    for s in [WorkflowStep::TemplateGeneration, WorkflowStep::SurfaceTopology, WorkflowStep::SurfaceProjection, WorkflowStep::PatchAssignment, WorkflowStep::EdgeExtraction, WorkflowStep::BoundaryLayerGeneration, WorkflowStep::MeshOptimisation] {
        let opts = CartesianMeshGeneratorOptions {
            dict: MeshDict::with_max_cell_size(0.5),
            workflow: WorkflowSettings { stop_after: Some(s), restart_from_latest_step: false },
            ..Default::default()
        };
        let (m, _) = cartesian_mesh(&p, &t, None, &opts).unwrap();
        println!("after {}: carried {} consistent {}", s.name(), m.cells.is_some(), m.carried_cells_are_consistent());
        let popts = outram_park_fork_cfmesh::pipeline::tet_mesh::TetMeshOptions {
            dict: MeshDict::with_max_cell_size(0.5),
            workflow: WorkflowSettings { stop_after: Some(s), restart_from_latest_step: false },
            ..Default::default()
        };
        let (m, _) = outram_park_fork_cfmesh::pipeline::p_mesh::p_mesh(&p, &t, None, &popts).unwrap();
        println!("pMesh after {}: carried {} consistent {}", s.name(), m.cells.is_some(), m.carried_cells_are_consistent());
    }
}

/// gh:#1385: the cell lists the port now carries after
/// `boundaryLayerGeneration` are upstream's, label for label and in order:
/// all 1128 cells of the dumped trace.
#[test]
fn carried_cell_lists_equal_upstream_after_layers() {
    let (p, t) = read_sphere();
    let opts = CartesianMeshGeneratorOptions {
        dict: MeshDict::with_max_cell_size(0.5),
        workflow: WorkflowSettings { stop_after: Some(WorkflowStep::BoundaryLayerGeneration), restart_from_latest_step: false },
        ..Default::default()
    };
    let (m, _) = cartesian_mesh(&p, &t, None, &opts).unwrap();
    let (cells, _, _) = load_trace();
    assert!(m.carried_cells_are_consistent(), "the carried lists must be consistent with owner/neighbour");
    let mine = m.cells.as_ref().unwrap();
    let n_same = mine.iter().zip(&cells).filter(|(a, b)| a == b).count();
    println!("cell lists identical to upstream's: {n_same} of {}", cells.len());
    for (c, (a, b)) in mine.iter().zip(&cells).enumerate().filter(|(_, (a, b))| a != b).take(6) {
        println!("cell {c}: port {a:?} upstream {b:?}");
    }
    assert_eq!(mine.len(), cells.len());
    assert_eq!(n_same, cells.len());
}
