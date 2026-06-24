use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use crate::ldu_matrix::fv_matrix::FvMatrix;

/// Implicit Gauss-orthogonal Laplacian: assembles the matrix for `−∇·(Γ∇φ)`.
///
/// ## Sign convention (matches OpenFOAM)
///
/// The returned matrix has **positive** diagonal and **negative** off-diagonals,
/// so the matrix–vector product `A·φ` approximates `−∇·(Γ∇φ)`.  Use the matrix
/// with a minus sign in the PDE to add the diffusion term:
///
/// ```text
/// // ∂φ/∂t − ∇·(Γ∇φ) = S
/// let eqn = fvm::ddt(&phi, &phi_old, dt) - fvm::laplacian(&gamma_f, &phi);
/// ```
///
/// ## Boundary conditions
///
/// - `ZeroGradient` / `Symmetry`: no contribution (zero normal flux).
/// - `FixedValue(v)`: adds `coeff` to diagonal and `coeff·v` to source.
pub fn laplacian(gamma: &SurfaceScalarField, phi: &VolScalarField) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());

    // Internal faces: Gauss orthogonal
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let delta = (mesh.cell_centres[n] - mesh.cell_centres[o]).mag();
        if delta < 1e-300 {
            continue;
        }
        let coeff = gamma.internal[f] * mesh.face_areas[f] / delta;
        mat.ldu.diag[o] += coeff;
        mat.ldu.diag[n] += coeff;
        mat.ldu.upper[f] = -coeff;
        mat.ldu.lower[f] = -coeff;
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let d = (mesh.face_centres[gf] - mesh.cell_centres[owner]).mag();
            if d < 1e-300 {
                continue;
            }
            let coeff = gamma.boundary[pi].values[fi] * mesh.face_areas[gf] / d;
            match &phi.boundary[pi].bc {
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {}
                BoundaryCondition::FixedValue(v) => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * v;
                }
                BoundaryCondition::FixedField(ff) => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * ff[fi];
                }
                _ => {}
            }
        }
    }

    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
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

    fn uniform_gamma(m: Arc<crate::mesh::fv_mesh::FvMesh>, val: f64) -> SurfaceScalarField {
        let _n_faces = m.owner.len();
        let internal = Field::uniform(m.n_internal_faces, val);
        let boundary = m.patches.iter()
            .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::uniform(p.size, val) })
            .collect();
        SurfaceScalarField::new("gamma", m, internal, boundary)
    }

    #[test]
    fn laplacian_symmetric_matrix() {
        // unit gamma: upper[f] == lower[f] and both are -coeff
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let phi = VolScalarField::uniform("T", m.clone(), 0.0);
        let mat = laplacian(&gamma, &phi);
        // internal face: |C_N - C_O| = 0.5, area = 1 → coeff = 1/0.5 = 2
        assert!((mat.ldu.upper[0] - (-2.0)).abs() < 1e-10);
        assert!((mat.ldu.lower[0] - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn laplacian_solves_linear_dirichlet() {
        // −∇²T = 0, T(0)=0, T(1)=1 → T is linear: T[0]=0.25, T[1]=0.75
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let t_bc = vec![
            PatchField { bc: BoundaryCondition::FixedValue(1.0), values: Field::new(vec![0.0]) },
            PatchField { bc: BoundaryCondition::FixedValue(0.0), values: Field::new(vec![0.0]) },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged, "Gauss-Seidel did not converge");
        assert!((result.internal[0] - 0.25).abs() < 1e-6, "T[0] = {}", result.internal[0]);
        assert!((result.internal[1] - 0.75).abs() < 1e-6, "T[1] = {}", result.internal[1]);
    }
}
