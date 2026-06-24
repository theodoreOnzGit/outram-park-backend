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
