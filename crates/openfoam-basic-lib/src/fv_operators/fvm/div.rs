use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use crate::ldu_matrix::fv_matrix::FvMatrix;

/// Implicit first-order upwind convection: assembles the matrix for `∇·(φ·ψ)`.
///
/// `phi` is the face flux field (SurfaceScalarField); `psi` is the transported
/// scalar (VolScalarField). The upwind scheme selects the donor cell:
///
/// - `φ_f ≥ 0`: flux comes from the **owner** cell → coefficient on `diag[O]`.
/// - `φ_f < 0`: flux comes from the **neighbour** cell → coefficient on `upper[f]`.
///
/// ## Boundary conditions
///
/// - `ZeroGradient` / `Symmetry`: boundary value equals owner cell → flux goes
///   entirely to `diag[owner]` regardless of sign.
/// - `FixedValue(v)`: inflow (`φ_f < 0`) uses the fixed value → explicit source;
///   outflow (`φ_f ≥ 0`) remains on the diagonal (upwind from owner).
pub fn div(phi: &SurfaceScalarField, psi: &VolScalarField) -> FvMatrix {
    let mesh = psi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());

    // Internal faces: upwind
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let phi_f = phi.internal[f];

        // Owner row O: outflow contributes diag, inflow contributes upper (N column)
        mat.ldu.diag[o] += phi_f.max(0.0);
        mat.ldu.upper[f] += phi_f.min(0.0);

        // Neighbour row N: inflow from O contributes diag, outflow contributes lower (O column)
        mat.ldu.diag[n] -= phi_f.min(0.0);
        mat.ldu.lower[f] -= phi_f.max(0.0);
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let owner = mesh.owner[patch.start + fi];
            let phi_f = phi.boundary[pi].values[fi];
            match &psi.boundary[pi].bc {
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {
                    // psi_face = psi_owner (zero gradient) → always on diagonal
                    mat.ldu.diag[owner] += phi_f;
                }
                BoundaryCondition::FixedValue(v) => {
                    if phi_f >= 0.0 {
                        // Outflow: upwind donor is owner cell
                        mat.ldu.diag[owner] += phi_f;
                    } else {
                        // Inflow: known boundary value → explicit
                        mat.source[owner] -= phi_f * v;
                    }
                }
                BoundaryCondition::FixedField(ff) => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f;
                    } else {
                        mat.source[owner] -= phi_f * ff[fi];
                    }
                }
                _ => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f;
                    }
                }
            }
        }
    }

    mat
}
