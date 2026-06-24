use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolVectorField;
use super::interpolate;

/// PISO flux correction: `ddtCorr[f] = (φ_old[f] − U_old_f · S_f) / Δt`.
///
/// Accounts for the discrepancy between the stored face flux `phi` (from the
/// previous time step) and the flux implied by the interpolated old velocity
/// field.  Adding this to `phiHbyA` before the pressure solve ensures the
/// PISO correction is consistent with the old-time flux.
///
/// OpenFOAM signature:
/// ```text
/// fvc::ddtCorr(U, phi)   // Euler scheme; dt comes from runTime.deltaT()
/// ```
/// Here we take `dt` explicitly so the function is self-contained.
///
/// Mirrors `fvc::ddtCorr` from
/// `src/finiteVolume/finiteVolume/fvc/fvcDdt.C`, Euler specialisation.
pub fn ddt_corr(
    u_old: &VolVectorField,
    phi_old: &SurfaceScalarField,
    dt: f64,
) -> SurfaceScalarField {
    let mesh = &u_old.mesh;
    let u_f = interpolate(u_old);

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let u_dot_sf = u_f.internal[f].dot(mesh.face_area_vectors[f]);
        (phi_old.internal[f] - u_dot_sf) / dt
    });

    let boundary = mesh.patches.iter()
        .zip(u_f.boundary.iter())
        .zip(phi_old.boundary.iter())
        .map(|((patch, u_bc), phi_bc)| {
            let values = Field::from_fn(patch.size, |fi| {
                let gf = patch.start + fi;
                let u_dot_sf = u_bc.values[fi].dot(mesh.face_area_vectors[gf]);
                (phi_bc.values[fi] - u_dot_sf) / dt
            });
            PatchField { bc: BoundaryCondition::ZeroGradient, values }
        })
        .collect();

    SurfaceScalarField::new("ddtCorr", phi_old.mesh.clone(), internal, boundary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::fields::vol_field::VolVectorField;
    use crate::fields::surface_field::SurfaceScalarField;
    use crate::fields::field::Field;
    use crate::fields::boundary::bc::PatchField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::fv_operators::fvc::flux;

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
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
    fn consistent_phi_gives_zero_correction() {
        // If phi_old == flux(U_old), ddtCorr == 0
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let phi = flux(&u);
        let corr = ddt_corr(&u, &phi, 0.1);
        assert!(corr.internal[0].abs() < 1e-12);
        assert!(corr.boundary[0].values[0].abs() < 1e-12);
    }

    #[test]
    fn nonzero_correction_for_inconsistent_phi() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        // phi_old = 2 everywhere (doesn't match U·Sf = 1)
        let bnd: Vec<_> = m.patches.iter()
            .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::uniform(p.size, 2.0) })
            .collect();
        let phi_old = SurfaceScalarField::new("phi", m.clone(), Field::uniform(1, 2.0), bnd);
        let corr = ddt_corr(&u, &phi_old, 1.0);
        // (phi_old - U·Sf) / dt = (2 - 1) / 1 = 1
        assert!((corr.internal[0] - 1.0).abs() < 1e-12);
    }
}
