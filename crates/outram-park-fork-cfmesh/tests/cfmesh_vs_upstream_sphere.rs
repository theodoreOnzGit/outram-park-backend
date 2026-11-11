//! gh:#1385: stage-by-stage differential of the port against **real upstream
//! cfMesh output**, on the sphere of the #1279 / #1299 setup S (R 3 m,
//! 24 x 48 UV, `maxCellSize 0.5`).
//!
//! # Method
//!
//! - **Reference.** `verification_and_validation/upstream_cfmesh_gh1385/`:
//!   cfMesh `e944f6ba695f` built against OpenFOAM-v2506 and run with
//!   `workflowControls { stopAfter <step>; }` after each step, for
//!   `cartesianMesh` and `pMesh`; the `polyMesh` files (ASCII, 17 digits) are
//!   in `runs/stages/<generator>_<step>/`.
//! - **Input.** The committed STL `inputs/sphere_r3_24x48.stl.gz`, read by
//!   the port's STL reader (a port of OpenFOAM's `STLReader`, so the points
//!   are the single-precision values upstream holds), not rebuilt in double.
//! - **Port run.** `cartesian_mesh` / `p_mesh` with the same `maxCellSize` and
//!   `WorkflowSettings { stop_after: Some(step) }`.
//! - **Comparison** (`tests/common`): counts; points at the same index
//!   (bit-identical count, largest absolute difference); faces as vertex sets
//!   and at the same index (ring, owner, neighbour).
//! - **Criterion.** A stage is *pinned as matching* when every point is
//!   bit-identical and every face ring, owner and neighbour is identical at
//!   the same index. A stage that differs is pinned at the numbers measured.
//!
//! # Results (2026-11-11, `cargo test --release`)
//!
//! | step | cartesianMesh points | faces | pMesh points | faces |
//! |---|---|---|---|---|
//! | templateGeneration | 895/895 | 2064/2064 | 12840/12840 | 13690/13690 |
//! | surfaceTopology | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
//! | surfaceProjection | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
//! | patchAssignment | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
//! | edgeExtraction | 895/895 | 2064/2064 | **8352/9744, max 1.091e-2** | 11494/11494 |
//! | boundaryLayerGeneration | 1425/1425 | 3648/3648 | 8352/9744, max 1.091e-2 | 11494/11494 |
//! | meshOptimisation | **562/1425, max 1.332e-15** | 3648/3648 | **0/9744, max 6.549e-3** | 11494/11494 |
//!
//! (points bit-identical / total; faces index-identical with owner and
//! neighbour / total.) Faces are index-identical at every stage of both
//! generators. First difference: `cartesianMesh` at meshOptimisation, in the
//! last bit (see the V&V README, "Rust-side differential"); `pMesh` at
//! edgeExtraction.
use common::*;
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
use outram_park_fork_cfmesh::pipeline::p_mesh::p_mesh;
use outram_park_fork_cfmesh::pipeline::tet_mesh::TetMeshOptions;
use outram_park_fork_cfmesh::workflow_controls::{WorkflowSettings, WorkflowStep};

mod common;

const STEPS: [WorkflowStep; 7] = [
    WorkflowStep::TemplateGeneration,
    WorkflowStep::SurfaceTopology,
    WorkflowStep::SurfaceProjection,
    WorkflowStep::PatchAssignment,
    WorkflowStep::EdgeExtraction,
    WorkflowStep::BoundaryLayerGeneration,
    WorkflowStep::MeshOptimisation,
];

fn stop(s: WorkflowStep) -> WorkflowSettings {
    WorkflowSettings { stop_after: Some(s), restart_from_latest_step: false }
}

fn cartesian_stage(s: WorkflowStep) -> Diff {
    let (p, t) = read_sphere();
    let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), workflow: stop(s), ..Default::default() };
    let (m, rep) = cartesian_mesh(&p, &t, None, &opts).unwrap();
    assert_eq!(rep.stopped_after, Some(s));
    compare(&m, &read_upstream(&format!("stages/cartesianMesh_{}", s.name())))
}

fn p_mesh_stage(s: WorkflowStep) -> Diff {
    let (p, t) = read_sphere();
    let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), workflow: stop(s), ..Default::default() };
    let (m, rep) = p_mesh(&p, &t, None, &opts).unwrap();
    assert_eq!(rep.stopped_after, Some(s));
    compare(&m, &read_upstream(&format!("stages/pMesh_{}", s.name())))
}

/// Every point bit-identical, every face identical at its index.
fn assert_matches(d: &Diff, what: &str) {
    println!("{what}: {}", d.summary());
    assert!(d.counts_equal, "{what}: counts {:?}", d.counts);
    let n = (d.counts.1).0;
    assert_eq!(d.index_points, Some((n, 0.0)), "{what}: points");
    assert_eq!(d.index_faces_identical, Some((d.counts.1).1), "{what}: faces");
}

/// Same counts, faces identical at their index.
fn assert_same_topology(d: &Diff, what: &str) {
    println!("{what}: {}", d.summary());
    assert!(d.counts_equal, "{what}: counts {:?}", d.counts);
    assert_eq!(d.index_faces_identical, Some((d.counts.1).1), "{what}: faces");
}

/// `cartesianMesh`: the first six stages are bit-identical (points, faces,
/// owner, neighbour). Fixes that made them so, with the number each moved:
/// the V&V README, "Rust-side differential".
#[test]
fn cartesian_mesh_matches_upstream_through_boundary_layer_generation() {
    for s in &STEPS[..6] {
        assert_matches(&cartesian_stage(*s), &format!("cartesianMesh after {}", s.name()));
    }
}

/// `cartesianMesh` meshOptimisation: counts and faces are identical, 562 of
/// 1425 points are bit-identical (the 530 layer-outer points, the optimiser
/// leaves them where `optimizeSurface` put them, and 32 more) and the worst
/// difference is 1.332e-15 m (last bit of a coordinate near 2-3 m, 6e-16
/// relative). Cause under test: `optimizeMeshFV`'s Laplacian sums over the
/// faces of a cell in upstream's cell face order, which `VolumeMesh` (faces
/// with owner/neighbour, no cell lists) does not carry; the port sums in
/// ascending face order. Pinned from above, not matched.
#[test]
fn cartesian_mesh_mesh_optimisation_is_within_the_last_bit() {
    let d = cartesian_stage(WorkflowStep::MeshOptimisation);
    assert_same_topology(&d, "cartesianMesh after meshOptimisation");
    let (same, worst) = d.index_points.unwrap();
    assert!(same >= 562, "bit-identical points fell from 562 to {same}");
    assert!(worst < 2e-15, "worst difference {worst:e}");
}

/// `pMesh`: template, surfaceTopology, surfaceProjection and patchAssignment
/// are bit-identical.
#[test]
fn p_mesh_matches_upstream_through_patch_assignment() {
    for s in &STEPS[..4] {
        assert_matches(&p_mesh_stage(*s), &format!("pMesh after {}", s.name()));
    }
}

/// `pMesh` edgeExtraction, boundaryLayerGeneration and meshOptimisation: the
/// topology is identical, the points are not (8352/9744 bit-identical, worst
/// 1.091e-2 m after edgeExtraction; 0/9744, worst 6.549e-3 m after
/// meshOptimisation). This is a **known difference**, pinned from above so it
/// cannot get worse; `meshSurfaceEdgeExtractorFUN` is the first suspect (see
/// the README).
#[test]
fn p_mesh_after_edge_extraction_differs_in_points_only() {
    let d = p_mesh_stage(WorkflowStep::EdgeExtraction);
    assert_same_topology(&d, "pMesh after edgeExtraction");
    let (same, worst) = d.index_points.unwrap();
    assert!(same >= 8352, "bit-identical points fell from 8352 to {same}");
    assert!(worst < 1.2e-2, "worst difference {worst:e}");
    let d = p_mesh_stage(WorkflowStep::MeshOptimisation);
    assert_same_topology(&d, "pMesh after meshOptimisation");
    assert!(d.index_points.unwrap().1 < 7e-3);
}
