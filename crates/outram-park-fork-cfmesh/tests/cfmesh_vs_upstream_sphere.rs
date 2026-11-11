//! gh:#1385 exploratory per-stage differential on the sphere (prints the table).
mod common;
use common::*;
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
use outram_park_fork_cfmesh::pipeline::p_mesh::p_mesh;
use outram_park_fork_cfmesh::pipeline::tet_mesh::TetMeshOptions;
use outram_park_fork_cfmesh::workflow_controls::{WorkflowSettings, WorkflowStep};

const STEPS: [WorkflowStep; 7] = [
    WorkflowStep::TemplateGeneration,
    WorkflowStep::SurfaceTopology,
    WorkflowStep::SurfaceProjection,
    WorkflowStep::PatchAssignment,
    WorkflowStep::EdgeExtraction,
    WorkflowStep::BoundaryLayerGeneration,
    WorkflowStep::MeshOptimisation,
];

#[test]
fn stage_table_cartesian_mesh() {
    let (p, t) = read_sphere();
    println!("sphere: {} points {} tris", p.len(), t.len());
    for s in STEPS {
        let opts = CartesianMeshGeneratorOptions {
            dict: MeshDict::with_max_cell_size(0.5),
            workflow: WorkflowSettings { stop_after: Some(s), restart_from_latest_step: false },
            ..Default::default()
        };
        let (m, rep) = cartesian_mesh(&p, &t, None, &opts).unwrap();
        let u = read_upstream(&format!("stages/cartesianMesh_{}", s.name()));
        println!("cartesianMesh after {:24} stopped={:?} {:?}", s.name(), rep.stopped_after, compare(&m, &u));
    }
}

#[test]
fn stage_table_p_mesh() {
    let (p, t) = read_sphere();
    for s in STEPS {
        let opts = TetMeshOptions {
            dict: MeshDict::with_max_cell_size(0.5),
            workflow: WorkflowSettings { stop_after: Some(s), restart_from_latest_step: false },
            ..Default::default()
        };
        let (m, rep) = p_mesh(&p, &t, None, &opts).unwrap();
        let u = read_upstream(&format!("stages/pMesh_{}", s.name()));
        println!("pMesh after {:24} stopped={:?} {:?}", s.name(), rep.stopped_after, compare(&m, &u));
    }
}

#[test]
fn debug_detail() {
    let (p, t) = read_sphere();
    let stage = std::env::var("GH1385_STAGE").unwrap_or("surfaceProjection".into());
    let gen = std::env::var("GH1385_GEN").unwrap_or("cartesianMesh".into());
    let s = WorkflowStep::from_name(&stage).unwrap();
    let wf = WorkflowSettings { stop_after: Some(s), restart_from_latest_step: false };
    let m = if gen == "cartesianMesh" {
        cartesian_mesh(&p, &t, None, &CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), workflow: wf, ..Default::default() }).unwrap().0
    } else {
        p_mesh(&p, &t, None, &TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), workflow: wf, ..Default::default() }).unwrap().0
    };
    let u = read_upstream(&format!("stages/{gen}_{stage}"));
    let mut on_bnd = vec![false; m.points.len()];
    for f in m.n_internal_faces()..m.faces.len() {
        for &q in &m.faces[f] {
            on_bnd[q] = true;
        }
    }
    let mut n = 0;
    for (i, (a, b)) in m.points.iter().zip(&u.points).enumerate() {
        let d = (a.x - b.x).abs().max((a.y - b.y).abs()).max((a.z - b.z).abs());
        if d > 0.0 && n < 25 {
            println!("pt {i} bnd={} d={d:.3e} port=({:.17},{:.17},{:.17}) up=({:.17},{:.17},{:.17})", on_bnd[i], a.x, a.y, a.z, b.x, b.y, b.z);
            n += 1;
        }
    }
    if m.points.len() == u.points.len() && m.faces.len() == u.faces.len() {
        let mut idx = std::collections::HashMap::new();
        for (i, f) in m.faces.iter().enumerate() {
            let mut v = f.clone();
            v.sort_unstable();
            idx.insert(v, i);
        }
        let perm: Vec<usize> = u
            .faces
            .iter()
            .map(|f| {
                let mut v = f.verts.clone();
                v.sort_unstable();
                *idx.get(&v).unwrap_or(&usize::MAX)
            })
            .collect();
        println!("upstream face -> port face: {:?}", &perm[..60.min(perm.len())]);
        let ring_same = u.faces.iter().zip(&perm).filter(|(f, &j)| j != usize::MAX && f.verts == m.faces[j]).count();
        let own_same = u.faces.iter().zip(&perm).filter(|(f, &j)| j != usize::MAX && f.owner == m.owner[j]).count();
        println!("faces whose ring is identical in the matched face: {ring_same}; owner identical: {own_same}");
        println!("boundary start up={} port={}", u.n_internal_faces, m.n_internal_faces());
    }
    for i in 0..4 {
        println!("face {i} port={:?} own={} nb={:?} | up={:?} own={} nb={:?}", m.faces[i], m.owner[i], m.neighbour[i], u.faces[i].verts, u.faces[i].owner, u.faces[i].neighbour);
    }
}
