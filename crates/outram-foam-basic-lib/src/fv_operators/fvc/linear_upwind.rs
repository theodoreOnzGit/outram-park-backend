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

//! `linearUpwind` face interpolation — `Foam::linearUpwind<scalar>` (gh:#1362).
//!
//! Translated from ESI OpenFOAM-v2506
//! `src/finiteVolume/interpolation/surfaceInterpolation/schemes/linearUpwind/linearUpwind.C`
//! plus `upwind.H` (`weights = pos0(faceFlux)`) and
//! `surfaceInterpolationScheme::interpolate`:
//!
//! ```text
//!   face = (w (P - N) + N) + (Cf - C_up) . grad(phi)_up ,   w = pos0(flux)
//!   up   = owner if flux > 0 else neighbour
//! ```
//!
//! Checked bit for bit against upstream on two meshes (flux of both signs):
//! `tests/operators_vs_openfoam.rs`. Boundary faces carry the patch value
//! (non-coupled patches); coupled patches are not corrected.

use super::{grad, interpolate};
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::{VolScalarField, VolVectorField};

/// `linearUpwind` face values of `vol` for the face flux `flux`, with the cell
/// gradient `g` (upstream reads it from `gradSchemes` `grad(<name>)`; pass
/// [`grad`] for `Gauss linear`, `grad_least_squares` for `leastSquares`).
#[must_use]
pub fn linear_upwind_with_grad(
    vol: &VolScalarField,
    flux: &SurfaceScalarField,
    g: &VolVectorField,
) -> SurfaceScalarField {
    let mesh = &vol.mesh;
    let mut out = interpolate(vol);
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let fl = flux.internal[f];
        // upwind `weights`: `pos0(faceFlux)` (flux >= 0 -> 1)
        let w = if fl >= 0.0 { 1.0 } else { 0.0 };
        let base = w * (vol.internal[o] - vol.internal[n]) + vol.internal[n];
        // `correction`: the upwind cell is picked by `faceFlux > 0`
        let cell = if fl > 0.0 { o } else { n };
        let corr = (mesh.face_centres[f] - mesh.cell_centres[cell]).dot(g.internal[cell]);
        out.internal[f] = base + corr;
    }
    out
}

/// [`linear_upwind_with_grad`] with the `Gauss linear` gradient.
#[must_use]
pub fn linear_upwind(vol: &VolScalarField, flux: &SurfaceScalarField) -> SurfaceScalarField {
    linear_upwind_with_grad(vol, flux, &grad(vol))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::PatchField;
    use crate::fields::field::Field;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    #[test]
    fn linear_field_is_reproduced_exactly_for_either_flux_sign() {
        // 3 cells along x, phi = x: the gradient is exactly 1, so linearUpwind
        // returns the true face value x_f for both flux directions.
        let m = Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0; 3])
                .cell_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.5, 0.0, 0.0),
                    Vector3::new(2.5, 0.0, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(2.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(3.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        );
        let phi = VolScalarField::new(
            "p",
            m.clone(),
            Field::new(vec![0.5, 1.5, 2.5]),
            vec![PatchField::fixed_value(1, 0.0), PatchField::fixed_value(1, 3.0)],
        );
        for sgn in [1.0, -1.0] {
            let flux = SurfaceScalarField::new(
                "f",
                m.clone(),
                Field::new(vec![sgn; 2]),
                vec![PatchField::zero_gradient(1), PatchField::zero_gradient(1)],
            );
            let r = linear_upwind(&phi, &flux);
            assert!((r.internal[0] - 1.0).abs() < 1e-12);
            assert!((r.internal[1] - 2.0).abs() < 1e-12);
        }
    }
}
