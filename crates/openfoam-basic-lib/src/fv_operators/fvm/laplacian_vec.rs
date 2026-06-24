use std::sync::Arc;

use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::mesh::fv_mesh::FvMesh;
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;
use crate::fv_operators::fvc::interpolate;

/// Implicit vector Laplacian: `−∇·(γ ∇U)`.
///
/// Assembles the same LDU coefficients as the scalar `fvm::laplacian`,
/// but with a vector source (for Dirichlet boundary conditions on U).
///
/// OpenFOAM convention (positive sign convention):
/// `laplacian_vec(gamma, U)` represents `∇·(γ ∇U)` (note: positive,
/// matching the momentum diffusion term; subtract when assembling
/// `fvm::ddt(U) - fvm::laplacian(nu, U)`).
pub fn laplacian_vec(
    gamma: &VolScalarField,
    u: &VolVectorField,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let gamma_f = interpolate(gamma);
    let mut mat = FvVectorMatrix::new(mesh.clone());

    // Internal faces
    for f in 0..mesh.n_internal_faces {
        let o  = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let sf = mesh.face_area_vectors[f];
        let delta = (mesh.cell_centres[nb] - mesh.cell_centres[o]).mag();
        let area  = sf.mag();
        let coeff = gamma_f.internal[f] * area / delta;

        mat.ldu.diag[o]    += coeff;
        mat.ldu.diag[nb]   += coeff;
        mat.ldu.upper[f]   -= coeff;
        mat.ldu.lower[f]   -= coeff;
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf    = patch.start + fi;
            let owner = mesh.owner[gf];
            let sf    = mesh.face_area_vectors[gf];
            let area  = sf.mag();
            let cf    = mesh.face_centres[gf];
            let delta = (cf - mesh.cell_centres[owner]).mag().max(1e-30);
            let coeff = gamma_f.boundary[pi].values[fi] * area / delta;

            match &u.boundary[pi].bc {
                BoundaryCondition::FixedValue(_) | BoundaryCondition::FixedField(_) | BoundaryCondition::Calculated(_) => {
                    // Dirichlet-type: wall value from boundary.values[fi]
                    let u_wall = u.boundary[pi].values[fi];
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner]   = mat.source[owner] + u_wall * coeff;
                }
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry | BoundaryCondition::Empty => {}
            }
        }
    }

    let _ = u; // consumed for BCs above; suppress dead-code lint
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::ldu_matrix::fv_matrix::SolverSettings;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use approx::assert_relative_eq;

    fn line_mesh(n: usize) -> Arc<FvMesh> {
        let dx = 1.0 / n as f64;
        let n_int = n - 1;
        let mut owner     = (0..n_int).collect::<Vec<_>>();
        let neighbour = (1..n).collect::<Vec<_>>();
        owner.push(n - 1); // right boundary
        owner.push(0);     // left boundary
        // neighbour list is only for internal faces
        let cell_volumes    = vec![dx; n];
        let cell_centres    = (0..n).map(|i| Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.0)).collect();
        let face_centres: Vec<_> = (0..n_int).map(|f| Vector3::new((f as f64 + 1.0) * dx, 0.0, 0.0))
            .chain([Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0)]).collect();
        let face_area_vectors: Vec<_> = (0..n_int).map(|_| Vector3::new(1.0, 0.0, 0.0))
            .chain([Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0)]).collect();
        Arc::new(FvMeshBuilder::new()
            .n_cells(n).n_internal_faces(n_int)
            .owner(owner).neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("right", n_int,     1, PatchKind::Wall),
                BoundaryPatch::new("left",  n_int + 1, 1, PatchKind::Wall),
            ])
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build().unwrap())
    }

    #[test]
    fn laplacian_vec_1d_linear_profile() {
        // Solve ∇²U = 0 with U=0 at x=0, U=(1,0,0) at x=1 → linear solution
        let n = 4;
        let m = line_mesh(n);
        let gamma = VolScalarField::uniform("nu", m.clone(), 1.0);

        // Build BCs: right (x=1): U=(1,0,0), left (x=0): U=(0,0,0)
        let u_bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::new(1.0, 0.0, 0.0)),
                values: Field::new(vec![Vector3::new(1.0, 0.0, 0.0)]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::ZERO),
                values: Field::new(vec![Vector3::ZERO]),
            },
        ];
        let u_init = VolVectorField::new(
            "U", m.clone(),
            Field::new(vec![Vector3::ZERO; n]),
            u_bc,
        );
        let mat = laplacian_vec(&gamma, &u_init, m.clone());
        let (u, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        for c in 0..n {
            let x = (c as f64 + 0.5) / n as f64;
            assert_relative_eq!(u.internal[c].x, x, epsilon = 1e-2);
        }
    }
}
