use crate::fields::vol_field::VolScalarField;
use crate::ldu_matrix::fv_matrix::FvMatrix;

/// Implicit first-order Euler time derivative: `∂φ/∂t ≈ (φ − φ_old) / Δt`.
///
/// Assembles `V/Δt` on the diagonal and `V·φ_old/Δt` into the source vector,
/// so that solving `A·φ = b` yields the updated field.
///
/// To represent `∂(ρφ)/∂t`, pass `rho.internal[c] * V[c] / dt` as the
/// diagonal coefficient per cell — or call `ddt_rho` (future Layer 3 addition).
pub fn ddt(phi: &VolScalarField, phi_old: &VolScalarField, dt: f64) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        let coeff = mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += coeff;
        mat.source[c] += coeff * phi_old.internal[c];
    }
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![0.5, 0.5])
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
    fn ddt_diagonal_is_volume_over_dt() {
        let m = unit_mesh();
        let phi = VolScalarField::uniform("p", m.clone(), 0.0);
        let phi_old = VolScalarField::uniform("p_old", m.clone(), 2.0);
        let mat = ddt(&phi, &phi_old, 0.1);
        // diag = V/dt = 0.5/0.1 = 5
        assert!((mat.ldu.diag[0] - 5.0).abs() < 1e-12);
        // source = V/dt * phi_old = 5 * 2 = 10
        assert!((mat.source[0] - 10.0).abs() < 1e-12);
    }

    #[test]
    fn ddt_solves_to_old_when_no_spatial_terms() {
        // ddt only: A·φ = V/dt·φ_old  →  φ = φ_old
        let m = unit_mesh();
        let phi = VolScalarField::zeros("p", m.clone());
        let phi_old = VolScalarField::uniform("p_old", m.clone(), 3.0);
        let mat = ddt(&phi, &phi_old, 1.0);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("p", settings);
        assert!(perf.converged, "Gauss-Seidel did not converge");
        assert!((result.internal[0] - 3.0).abs() < 1e-8);
        assert!((result.internal[1] - 3.0).abs() < 1e-8);
    }
}
