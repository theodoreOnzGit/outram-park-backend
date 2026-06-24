use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolVectorField;
use crate::primitives::{Tensor, Vector3};

/// Full tensor inverse when det is large enough, otherwise fall back to
/// component-wise pseudo-inverse (handles degenerate 1-D / 2-D mesh tests).
#[inline]
fn pseudo_inv_mv(m: Tensor, rhs: Vector3) -> Vector3 {
    let det = m.det();
    let scale = m.xx.abs().max(m.yy.abs()).max(m.zz.abs());
    if scale > 0.0 && det.abs() > 1e-20 * scale.powi(3) {
        m.inv().mat_vec(rhs)
    } else {
        // Degenerate: each non-zero diagonal component is solved independently.
        Vector3::new(
            if m.xx.abs() > 1e-20 { rhs.x / m.xx } else { 0.0 },
            if m.yy.abs() > 1e-20 { rhs.y / m.yy } else { 0.0 },
            if m.zz.abs() > 1e-20 { rhs.z / m.zz } else { 0.0 },
        )
    }
}

/// Reconstruct a `VolVectorField` from a face flux field.
///
/// Solves the per-cell least-squares problem:
/// ```text
/// (Σ_f Sf⊗Sf) · U[c] = Σ_f phi[f] · Sf[f]
/// ```
/// which on an orthogonal mesh reduces to component-wise division.
///
/// This is the inverse of `fvc::flux`: if `phi = U · Sf` everywhere, then
/// `reconstruct(phi) ≈ U` (exact for orthogonal meshes; approximate otherwise).
///
/// Used in PISO after the pressure solve to correct the velocity:
/// ```text
/// U = fvc::reconstruct(phiHbyA - rAUf * fvc::snGrad(p) * mesh.magSf())
/// ```
///
/// Mirrors `fvc::reconstruct` from
/// `src/finiteVolume/finiteVolume/fvc/fvcReconstruct.C`.
pub fn reconstruct(phi: &SurfaceScalarField) -> VolVectorField {
    let mesh = &phi.mesh;
    let n = mesh.n_cells;

    // Accumulate per-cell: rhs = Σ phi·Sf,  M = Σ Sf⊗Sf
    let mut rhs = vec![Vector3::ZERO; n];
    let mut m   = vec![Tensor::ZERO; n];

    // For each internal face f with flux phi[f] (positive = outward from owner):
    //   owner  sees outward-flux  = +phi[f],  outward-normal = +Sf → +phi[f]*Sf
    //   neighbour sees outward-flux = -phi[f], outward-normal = -Sf → (-phi)*(-Sf) = +phi[f]*Sf
    // Both add the SAME signed contribution, so no minus sign for neighbour.
    for f in 0..mesh.n_internal_faces {
        let o  = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let sf = mesh.face_area_vectors[f];
        let sf_sf = sf * sf;   // outer product Sf⊗Sf (Vector3 * Vector3 → Tensor)

        rhs[o]  = rhs[o]  + sf * phi.internal[f];
        rhs[nb] = rhs[nb] + sf * phi.internal[f];
        m[o]    = m[o]  + sf_sf;
        m[nb]   = m[nb] + sf_sf;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf    = patch.start + fi;
            let owner = mesh.owner[gf];
            let sf    = mesh.face_area_vectors[gf];
            rhs[owner] = rhs[owner] + sf * phi.boundary[pi].values[fi];
            m[owner]   = m[owner]  + sf * sf;
        }
    }

    let boundary = mesh.patches.iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("reconstruct({})", phi.name),
        phi.mesh.clone(),
        Field::from_fn(n, |c| pseudo_inv_mv(m[c], rhs[c])),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::fields::boundary::bc::BoundaryCondition;
    use crate::fields::vol_field::VolVectorField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::fv_operators::fvc::flux;
    use approx::assert_relative_eq;

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
    fn flux_then_reconstruct_roundtrip() {
        // For a uniform field, reconstruct(flux(U)) ≈ U
        let m = unit_mesh();
        let u_in = VolVectorField::uniform("U", m.clone(), Vector3::new(3.0, 0.0, 0.0));
        let phi = flux(&u_in);
        let u_out = reconstruct(&phi);
        assert_relative_eq!(u_out.internal[0].x, 3.0, epsilon = 1e-8);
        assert_relative_eq!(u_out.internal[1].x, 3.0, epsilon = 1e-8);
    }

    #[test]
    fn reconstruct_consistent_phi() {
        use crate::fields::field::Field;
        let m = unit_mesh();
        // phi from a uniform U=(2,0,0):
        //   internal f=0: phi = 2 (Sf=(1,0,0))
        //   right bnd gf=1: phi = 2 (Sf=(1,0,0))
        //   left bnd gf=2: phi = -2 (Sf=(-1,0,0))
        let bnd = vec![
            PatchField { bc: BoundaryCondition::FixedValue(2.0), values: Field::new(vec![2.0]) },
            PatchField { bc: BoundaryCondition::FixedValue(-2.0), values: Field::new(vec![-2.0]) },
        ];
        let phi = SurfaceScalarField::new(
            "phi", m.clone(),
            Field::new(vec![2.0]),
            bnd,
        );
        let u = reconstruct(&phi);
        // Should recover U.x ≈ 2 for both cells
        assert_relative_eq!(u.internal[0].x, 2.0, epsilon = 1e-6);
        assert_relative_eq!(u.internal[1].x, 2.0, epsilon = 1e-6);
    }
}
