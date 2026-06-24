use std::sync::Arc;

use crate::fields::vol_field::VolVectorField;
use crate::mesh::fv_mesh::FvMesh;
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;

/// Implicit Euler ddt for a `VolVectorField`:
/// `∂U/∂t ≈ (U − U_old) / Δt`
///
/// Assembles:
/// - `diag[c] += V[c] / dt`
/// - `source[c] += V[c] / dt * U_old[c]`
///
/// Mirrors `fvm::ddt(volVectorField, dt)` Euler scheme.
pub fn ddt_vec(
    u: &VolVectorField,
    u_old: &VolVectorField,
    dt: f64,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let n = mesh.n_cells;
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..n {
        let coeff = mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += coeff;
        mat.source[c] = mat.source[c] + u_old.internal[c] * coeff;
    }
    let _ = u; // unused; type ensures correct VolVectorField is passed
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::ldu_matrix::fv_matrix::SolverSettings;
    use crate::primitives::Vector3;

    fn unit_mesh() -> Arc<FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0, 1.0])
            .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(0.5, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
            ])
            .build().unwrap())
    }

    #[test]
    fn ddt_vec_recovers_constant_field() {
        // If U_old == U_new == const, solving (V/dt * U = V/dt * U_old) gives back U_old.
        let m = unit_mesh();
        let u_old = VolVectorField::uniform("Uold", m.clone(), Vector3::new(1.0, 2.0, 3.0));
        let u     = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 2.0, 3.0));
        let mat = ddt_vec(&u, &u_old, 0.1, m);
        let (u_new, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "not converged");
        assert!((u_new.internal[0].x - 1.0).abs() < 1e-8);
        assert!((u_new.internal[0].y - 2.0).abs() < 1e-8);
        assert!((u_new.internal[0].z - 3.0).abs() < 1e-8);
    }
}
