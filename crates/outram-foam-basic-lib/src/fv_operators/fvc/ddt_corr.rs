// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolVectorField;
use super::interpolate;

/// Rhie–Chow time-derivative flux correction (Euler scheme), with OpenFOAM's
/// `fvcDdtPhiCoeff` limiter.
///
/// ```text
/// phiCorr[f] = phi_old[f] − (interpolate(U_old) · S_f)
/// coeff[f]   = 1 − min( |phiCorr[f]| / (|phi_old[f]| + SMALL),  1 )
/// ddtCorr[f] = coeff[f] · phiCorr[f] / Δt
/// ```
///
/// `phiCorr` is the discrepancy between the stored face flux (which carries the
/// pressure-driven, Rhie–Chow part of the velocity) and the flux you would get
/// by simply interpolating the cell velocity. Re-injecting it into `phiHbyA`
/// before the pressure solve (as `interpolate(rAU)·ddtCorr`) keeps the face
/// flux coupled to its own history, which is what suppresses pressure–velocity
/// (checkerboard) decoupling and lets the PISO/PIMPLE loop stay stable at
/// Courant numbers approaching 1. **Without the limiter** the raw `phiCorr/Δt`
/// can overshoot and is itself destabilising; the `coeff` blend (→ 0 where the
/// correction is large relative to the flux, → 1 where it is small) is the
/// stabilising ingredient.
///
/// The coefficient is zeroed on boundary faces — OpenFOAM zeros it on
/// fixed-value (and cyclicAMI) patches, and incompressible solvers re-impose the
/// boundary flux via `constrainHbyA`/`adjustPhi` regardless — so this returns a
/// zero boundary field.
///
/// OpenFOAM signature: `fvc::ddtCorr(U, phi)` (Euler; Δt from `runTime`). Here
/// `dt` is passed explicitly. Mirrors
/// `EulerDdtScheme<Type>::fvcDdtPhiCorr` + `ddtScheme<Type>::fvcDdtPhiCoeff`
/// (standard version, `ddtPhiCoeff_ < 0`) in
/// `src/finiteVolume/finiteVolume/ddtSchemes/`.
pub fn ddt_corr(
    u_old: &VolVectorField,
    phi_old: &SurfaceScalarField,
    dt: f64,
) -> SurfaceScalarField {
    // Matches OpenFOAM's SMALL guard in the |phiCorr|/|phi| ratio.
    const SMALL: f64 = 1e-37;
    let mesh = &u_old.mesh;
    let u_f = interpolate(u_old);
    let r_dt = 1.0 / dt;

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let u_dot_sf  = u_f.internal[f].dot(mesh.face_area_vectors[f]);
        let phi_corr  = phi_old.internal[f] - u_dot_sf;
        let coeff = 1.0 - (phi_corr.abs() / (phi_old.internal[f].abs() + SMALL)).min(1.0);
        coeff * phi_corr * r_dt
    });

    // coeff = 0 on boundaries (see doc note).
    let boundary = mesh.patches.iter()
        .map(|p| PatchField {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zeros(p.size),
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
        // phiCorr = 2 − 1 = 1; coeff = 1 − min(|1|/(|2|+SMALL), 1) = 0.5;
        // ddtCorr = coeff·phiCorr/dt = 0.5·1/1 = 0.5
        assert!((corr.internal[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn coeff_limits_large_correction_to_zero() {
        // When |phiCorr| ≥ |phi_old|, coeff → 0, so the correction is fully
        // damped (this is what keeps ddtCorr from destabilising the flux).
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        // phi_old = 0 (U·Sf = 1 → |phiCorr| = 1 ≫ |phi_old| = 0)
        let bnd: Vec<_> = m.patches.iter()
            .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::zeros(p.size) })
            .collect();
        let phi_old = SurfaceScalarField::new("phi", m.clone(), Field::zeros(1), bnd);
        let corr = ddt_corr(&u, &phi_old, 1.0);
        assert!(corr.internal[0].abs() < 1e-12, "coeff must damp the correction to ~0");
    }
}
