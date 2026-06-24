use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::primitives::Vector3;
use super::interpolate;

/// Face flux `φ_f = U_f · S_f` — dot interpolated velocity with face area vector.
///
/// Returns a `SurfaceScalarField` (units m³/s for incompressible, kg/s for
/// compressible).  Boundary faces use the BC-evaluated face velocity.
///
/// Mirrors `fvc::flux(U)` from `src/finiteVolume/finiteVolume/fvc/fvcFlux.C`.
pub fn flux(u: &VolVectorField) -> SurfaceScalarField {
    let mesh = &u.mesh;
    let u_f = interpolate(u);

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        u_f.internal[f].dot(mesh.face_area_vectors[f])
    });

    let boundary = mesh.patches.iter().zip(u_f.boundary.iter())
        .enumerate()
        .map(|(_, (patch, bc_patch))| {
            let values = Field::from_fn(patch.size, |fi| {
                let gf = patch.start + fi;
                bc_patch.values[fi].dot(mesh.face_area_vectors[gf])
            });
            PatchField { bc: BoundaryCondition::ZeroGradient, values }
        })
        .collect();

    SurfaceScalarField::new(format!("phi({})", u.name), u.mesh.clone(), internal, boundary)
}

/// Buoyancy face flux: `φ_b[f] = ρ_f · (g · S_f)`.
///
/// Used to assemble the buoyancy-driven pressure source in the momentum and
/// pressure equations of `chtMultiRegionFoam` and `buoyantFoam`:
/// ```text
/// // In pEqn:
/// phi += fvc::buoyancy_flux(&rho, g);
/// ```
pub fn buoyancy_flux(rho: &VolScalarField, g: Vector3) -> SurfaceScalarField {
    let mesh = &rho.mesh;
    let rho_f = interpolate(rho);

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        rho_f.internal[f] * g.dot(mesh.face_area_vectors[f])
    });

    let boundary = mesh.patches.iter().zip(rho_f.boundary.iter())
        .enumerate()
        .map(|(_, (patch, rho_bc))| {
            let values = Field::from_fn(patch.size, |fi| {
                let gf = patch.start + fi;
                rho_bc.values[fi] * g.dot(mesh.face_area_vectors[gf])
            });
            PatchField { bc: BoundaryCondition::ZeroGradient, values }
        })
        .collect();

    SurfaceScalarField::new("buoyancyFlux", rho.mesh.clone(), internal, boundary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

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
    fn uniform_x_velocity_flux_equals_area() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m, Vector3::new(2.0, 0.0, 0.0));
        let phi = flux(&u);
        // U·Sf = (2,0,0)·(1,0,0) = 2 for all faces pointing in +x
        assert!((phi.internal[0] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn buoyancy_flux_downward_gravity() {
        let m = unit_mesh();
        let rho = crate::fields::vol_field::VolScalarField::uniform("rho", m, 1.2);
        let g = Vector3::new(0.0, -9.81, 0.0);
        let phi_b = buoyancy_flux(&rho, g);
        // g·Sf = (0,-9.81,0)·(1,0,0) = 0 for x-aligned faces
        assert!(phi_b.internal[0].abs() < 1e-12);
    }
}
