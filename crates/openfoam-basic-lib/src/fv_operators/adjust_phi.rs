use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolVectorField;

/// Adjust face fluxes to satisfy global mass balance.
///
/// Sums all boundary fluxes.  Any imbalance (net outflow ≠ 0 in a closed
/// domain) is distributed uniformly over **adjustable** boundary faces — those
/// whose corresponding velocity BC is `ZeroGradient` (i.e. free-stream /
/// outlet faces where the flux is not prescribed).  Fixed-value faces are left
/// untouched.
///
/// This matches `adjustPhi(phi, U, p)` in OpenFOAM
/// (`src/finiteVolume/cfdTools/general/adjustPhi/adjustPhi.C`).
///
/// # Returns
/// `true` if any adjustment was made, `false` if already balanced.
pub fn adjust_phi(
    phi: &mut SurfaceScalarField,
    u: &VolVectorField,
) -> bool {
    let mesh = &phi.mesh;

    // 1. Count adjustable boundary faces and sum all boundary fluxes.
    let mut total_flux    = 0.0_f64;
    let mut adjustable_n  = 0usize;

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            total_flux += phi.boundary[pi].values[fi];
            if matches!(u.boundary[pi].bc, BoundaryCondition::ZeroGradient) {
                adjustable_n += 1;
            }
        }
    }

    if adjustable_n == 0 || total_flux.abs() < 1e-30 {
        return false;
    }

    // 2. Distribute correction over adjustable faces.
    let correction = -total_flux / adjustable_n as f64;
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if matches!(u.boundary[pi].bc, BoundaryCondition::ZeroGradient) {
            for fi in 0..patch.size {
                phi.boundary[pi].values[fi] += correction;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::vol_field::VolVectorField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;

    fn two_cell_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
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
    fn balances_net_outflow() {
        let m = two_cell_mesh();
        // Inlet (left) fixed at phi=-1, outlet (right) ZeroGradient at phi=0.5 (imbalanced)
        let bnd = vec![
            PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![0.5]) },
            PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![-1.0]) },
        ];
        let u_bnd = vec![
            crate::fields::boundary::bc::PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![Vector3::ZERO]) },
            crate::fields::boundary::bc::PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::new(-1.0, 0.0, 0.0)),
                values: Field::new(vec![Vector3::new(-1.0, 0.0, 0.0)]),
            },
        ];
        let u = VolVectorField::new("U", m.clone(), Field::new(vec![Vector3::ZERO; 2]), u_bnd);
        let mut phi = SurfaceScalarField::new("phi", m.clone(), Field::new(vec![0.0]), bnd);

        let adjusted = adjust_phi(&mut phi, &u);
        assert!(adjusted);
        // net = 0.5 + (-1) = -0.5 → correction = 0.5 → outlet becomes 0.5+0.5=1.0
        let net: f64 = phi.boundary.iter().flat_map(|p| p.values.iter().cloned()).sum();
        assert!(net.abs() < 1e-12, "net flux = {net}");
    }

    #[test]
    fn no_adjustment_when_balanced() {
        let m = two_cell_mesh();
        let bnd = vec![
            PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![1.0]) },
            PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![-1.0]) },
        ];
        let u_bnd = vec![
            PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![Vector3::ZERO]) },
            PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::new(vec![Vector3::ZERO]) },
        ];
        let u = VolVectorField::new("U", m.clone(), Field::new(vec![Vector3::ZERO; 2]), u_bnd);
        let mut phi = SurfaceScalarField::new("phi", m.clone(), Field::new(vec![0.0]), bnd);
        let adjusted = adjust_phi(&mut phi, &u);
        assert!(!adjusted);
    }
}
