use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::primitives::Vector3;
use super::interpolate;

/// Gauss gradient: `∇φ|_O = (1/V_O) · Σ_f φ_f · S_f`.
///
/// Uses `fvc::interpolate` for face values, then accumulates face-area-vector
/// contributions to each cell and divides by cell volume.
pub fn grad(vol: &VolScalarField) -> VolVectorField {
    let mesh = &vol.mesh;
    let phi_f = interpolate(vol);

    let mut g = vec![Vector3::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let contrib = mesh.face_area_vectors[f] * phi_f.internal[f];
        g[o] = g[o] + contrib;
        g[n] = g[n] - contrib;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let contrib = mesh.face_area_vectors[gf] * phi_f.boundary[pi].values[fi];
            g[owner] = g[owner] + contrib;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("grad({})", vol.name),
        vol.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| g[c] * (1.0 / mesh.cell_volumes[c])),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};
    use approx::assert_relative_eq;

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
    fn uniform_field_zero_grad() {
        let m = unit_mesh();
        let p = VolScalarField::uniform("p", m, 1.0);
        let gp = grad(&p);
        assert_relative_eq!(gp.internal[0].x, 0.0, epsilon = 1e-12);
        assert_relative_eq!(gp.internal[1].x, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn linear_field_constant_x_grad() {
        // p = x: p[0]=0.25, p[1]=0.75; wall BCs p(x=1)=1, p(x=0)=0
        // → Gauss gradient should give ∂p/∂x = 1 in both cells.
        use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
        let m = unit_mesh();
        let boundary = vec![
            PatchField { bc: BoundaryCondition::FixedValue(1.0), values: Field::new(vec![0.0]) },
            PatchField { bc: BoundaryCondition::FixedValue(0.0), values: Field::new(vec![0.0]) },
        ];
        let mut p = VolScalarField::new("p", m.clone(), Field::new(vec![0.0; 2]), boundary);
        p.internal[0] = 0.25;
        p.internal[1] = 0.75;
        let gp = grad(&p);
        assert_relative_eq!(gp.internal[0].x, 1.0, epsilon = 1e-10);
        assert_relative_eq!(gp.internal[1].x, 1.0, epsilon = 1e-10);
    }
}
