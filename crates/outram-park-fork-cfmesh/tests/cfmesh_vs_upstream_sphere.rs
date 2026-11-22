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
//! | edgeExtraction | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
//! | boundaryLayerGeneration | 1425/1425 | 3648/3648 | 9744/9744 | 11494/11494 |
//! | meshOptimisation | ~~623/1425, max 1.332e-15~~ **1425/1425** | 3648/3648 | **3435/9744, max 1.332e-15** | 11494/11494 |
//!
//! (points bit-identical / total; faces index-identical with owner and
//! neighbour / total.) Faces are index-identical at every stage of both
//! generators. First difference, for both: meshOptimisation, in the last bit
//! (see the V&V README, "Rust-side differential").
use common::*;
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
use outram_park_fork_cfmesh::pipeline::p_mesh::p_mesh;
use outram_park_fork_cfmesh::pipeline::tet_mesh::{tet_mesh, TetMeshOptions};
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

/// `cartesianMesh`: all seven stages are bit-identical (points, faces, owner,
/// neighbour). Fixes that made them so, with the number each moved: the V&V
/// README, "Rust-side differential". The last one (gh:#1385, 2026-11-11):
/// `VolumeMesh` carries cfMesh's cell face lists (`VolumeMesh::cells`), so
/// `meshOptimisation`'s Laplacian sums run in upstream's order: points
/// ~~623/1425, worst 1.332e-15 m~~ 1425/1425.
#[test]
fn cartesian_mesh_matches_upstream_through_mesh_optimisation() {
    for s in &STEPS {
        assert_matches(&cartesian_stage(*s), &format!("cartesianMesh after {}", s.name()));
    }
}

/// `pMesh`: all seven stages are bit-identical. History (gh:#1385,
/// 2026-11-11): edgeExtraction needed `meshSurfaceEdgeExtractorFUN` to run on
/// a surface of one region (before: 8352/9744 points, worst 1.091e-2 m);
/// meshOptimisation needed the octree-backed `meshSurfaceOptimizer` and the
/// cell lists to be carried through `surfaceMorpherCells` (before:
/// ~~3435/9744, worst 1.332e-15 m~~ 9744/9744).
#[test]
fn p_mesh_matches_upstream_through_mesh_optimisation() {
    for s in &STEPS {
        assert_matches(&p_mesh_stage(*s), &format!("pMesh after {}", s.name()));
    }
}

fn final_cartesian() -> Diff {
    let (p, t) = read_sphere();
    let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (m, _) = cartesian_mesh(&p, &t, None, &opts).unwrap();
    compare(&m, &read_upstream("sphere_cartesianMesh"))
}

fn final_tet() -> Diff {
    let (p, t) = read_sphere();
    let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (m, _) = tet_mesh(&p, &t, None, &opts).unwrap();
    compare(&m, &read_upstream("sphere_tetMesh"))
}

fn final_p() -> Diff {
    let (p, t) = read_sphere();
    let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
    let (m, _) = p_mesh(&p, &t, None, &opts).unwrap();
    compare(&m, &read_upstream("sphere_pMesh"))
}

/// gh:#1385: the FINAL meshes (after `renumberMesh` and
/// `renameBoundaryPatches`) of all three generators are bit-identical to
/// upstream's written `polyMesh`: points, faces, owner and neighbour. Before
/// the cell lists were carried: `cartesianMesh` points 623/1425 and 2/3648
/// faces index-identical; `tetMesh` points 2/2651 (worst 0.19 m), faces not
/// matched.
#[test]
fn final_meshes_match_upstream() {
    assert_matches(&final_cartesian(), "cartesianMesh final");
    assert_matches(&final_tet(), "tetMesh final");
    assert_matches(&final_p(), "pMesh final");
}

fn tet_stage(s: WorkflowStep) -> Diff {
    let (p, t) = read_sphere();
    let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.5), workflow: stop(s), ..Default::default() };
    let (m, rep) = tet_mesh(&p, &t, None, &opts).unwrap();
    assert_eq!(rep.stopped_after, Some(s));
    compare(&m, &read_upstream(&format!("stages/tetMesh_{}", s.name())))
}

/// `tetMesh` against the staged upstream runs added in gh:#1385
/// (`runs/stages/tetMesh_<step>`): all seven stages bit-identical. History:
/// `surfaceTopology` had the right points and 819/26580 faces at their index
/// (`TopoMesh::remove_cells` ordered faces by a stable sort where upstream's
/// `reorderBoundaryFaces` swaps pairwise, and reversed a ring wholesale where
/// `face::reverseFace` keeps the first point); `meshOptimisation` needed the
/// octree-backed `optimizeSurface` and the carried cell lists (before:
/// 2/2651 final points).
#[test]
fn tet_mesh_matches_upstream_through_mesh_optimisation() {
    for s in &STEPS {
        assert_matches(&tet_stage(*s), &format!("tetMesh after {}", s.name()));
    }
}
