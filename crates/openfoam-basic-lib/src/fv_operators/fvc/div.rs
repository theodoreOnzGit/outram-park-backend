use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use super::interpolate;

/// `∇·φ_f = (1/V_O) · Σ_f φ_f` — net volumetric flux per unit volume.
///
/// Used to evaluate the continuity residual `∇·U` or `∇·(ρU)/ρ`.
pub fn div_flux(phi: &SurfaceScalarField) -> VolScalarField {
    let mesh = &phi.mesh;
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        d[mesh.owner[f]] += phi.internal[f];
        d[mesh.neighbour[f]] -= phi.internal[f];
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            d[mesh.owner[patch.start + fi]] += phi.boundary[pi].values[fi];
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({})", phi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·ψ) = (1/V_O) · Σ_f φ_f · ψ_f` — convective scalar flux.
///
/// `phi` is the face mass flux (SurfaceScalarField); `psi` is the transported
/// scalar (VolScalarField).  Face values of `psi` are obtained by linear
/// interpolation.
pub fn div(phi: &SurfaceScalarField, psi: &VolScalarField) -> VolScalarField {
    let mesh = &phi.mesh;
    let psi_f = interpolate(psi);
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let flux = phi.internal[f] * psi_f.internal[f];
        d[mesh.owner[f]] += flux;
        d[mesh.neighbour[f]] -= flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let flux = phi.boundary[pi].values[fi] * psi_f.boundary[pi].values[fi];
            d[mesh.owner[patch.start + fi]] += flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({},{})", phi.name, psi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·U) = (1/V_O) · Σ_f φ_f · U_f` — convective vector flux.
///
/// `phi` is the face mass flux; `U` is the velocity (VolVectorField).
pub fn div_vec(phi: &SurfaceScalarField, u: &VolVectorField) -> VolVectorField {
    use crate::primitives::Vector3;

    let mesh = &phi.mesh;
    let u_f = interpolate(u);
    let mut d = vec![Vector3::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let flux = u_f.internal[f] * phi.internal[f];
        d[mesh.owner[f]] = d[mesh.owner[f]] + flux;
        d[mesh.neighbour[f]] = d[mesh.neighbour[f]] - flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let flux = u_f.boundary[pi].values[fi] * phi.boundary[pi].values[fi];
            d[mesh.owner[patch.start + fi]] = d[mesh.owner[patch.start + fi]] + flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("div({},{})", phi.name, u.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] * (1.0 / mesh.cell_volumes[c])),
        boundary,
    )
}
