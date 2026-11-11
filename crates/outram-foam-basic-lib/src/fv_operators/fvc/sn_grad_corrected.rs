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

//! `corrected` and `limited corrected` surface-normal gradient —
//! `Foam::fv::correctedSnGrad` and `Foam::fv::limitedSnGrad` (gh:#1362).
//!
//! Translated from ESI OpenFOAM-v2506
//! `src/finiteVolume/finiteVolume/snGradSchemes/correctedSnGrad/correctedSnGrad.C`,
//! `.../limitedSnGrad/limitedSnGrad.C` and
//! `src/finiteVolume/fvMesh/fvGeometryScheme/basic/basicFvGeometryScheme.C`
//! (`nonOrthCorrectionVectors`). Checked bit for bit against the upstream
//! libraries on two meshes: `tests/operators_vs_openfoam.rs`,
//! `verification_and_validation/upstream_ops_gh1362/README.md`.
//!
//! ```text
//!   snGrad_f = nonOrthDeltaCoeffs_f (phi_N - phi_P)  +  k_f . ((w (g_P - g_N) + g_N))
//!   k_f      = Sf/|Sf| - d nonOrthDeltaCoeffs_f ,     g = grad(phi)  (Gauss linear)
//! ```
//!
//! On a non-coupled boundary face upstream's `snGrad` is the patch's own
//! `snGrad()` and the correction vector is zero. Coupled (cyclic / AMI) faces
//! are not corrected here.

use super::{grad, linear_weight, sn_grad_with_delta};
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::fv_operators::fvm::{DeltaCoeff, NonOrthoScheme};
use crate::mesh::fv_mesh::FvMesh;
use crate::primitives::Vector3;

/// Upstream `SMALL` for `double` (1e-15), the floor in `limitedSnGrad`'s limiter.
const SMALL: f64 = 1.0e-15;

/// Non-orthogonal correction vectors on the **internal** faces: upstream
/// `basicFvGeometryScheme::nonOrthCorrectionVectors`,
/// `Sf/magSf - (C_N - C_P) nonOrthDeltaCoeffs`. The non-coupled boundary vectors
/// are zero upstream and are not stored here.
#[must_use]
pub fn non_orth_correction_vectors(mesh: &FvMesh) -> Vec<Vector3> {
    (0..mesh.n_internal_faces)
        .map(|f| {
            let sf = mesh.face_area_vectors[f];
            let mag_sf = mesh.face_areas[f];
            let unit_area = Vector3::new(sf.x / mag_sf, sf.y / mag_sf, sf.z / mag_sf);
            let delta = mesh.cell_centres[mesh.neighbour[f]] - mesh.cell_centres[mesh.owner[f]];
            let dc = DeltaCoeff::NonOrthogonal.interior_delta_coeff(mesh, f);
            unit_area - Vector3::new(delta.x * dc, delta.y * dc, delta.z * dc)
        })
        .collect()
}

/// `correctedSnGrad::fullGradCorrection` on the internal faces, with a caller
/// supplied cell gradient: `k_f . (w (g_P - g_N) + g_N)`.
fn full_grad_correction(vol: &VolScalarField, g: &VolVectorField) -> Vec<f64> {
    let mesh = &vol.mesh;
    let ncv = non_orth_correction_vectors(mesh);
    (0..mesh.n_internal_faces)
        .map(|f| {
            let w = linear_weight(mesh, f);
            let gp = g.internal[mesh.owner[f]];
            let gn = g.internal[mesh.neighbour[f]];
            let d = gp - gn;
            let gf = Vector3::new(w * d.x, w * d.y, w * d.z) + gn;
            ncv[f].dot(gf)
        })
        .collect()
}

/// The explicit non-orthogonal correction of `scheme` on the internal faces,
/// from the cell gradient `g` (upstream builds it from `gradSchemes`
/// `grad(<field>)`; pass [`grad`] for `Gauss linear`, `grad_least_squares` for
/// `leastSquares`).
///
/// - [`NonOrthoScheme::Orthogonal`]: zero (upstream `orthogonal`).
/// - [`NonOrthoScheme::Corrected`]: `fullGradCorrection` (upstream `corrected`).
/// - [`NonOrthoScheme::Limited`]`(c)`: upstream `limited corrected c`,
///   `limiter*corr` with `limiter = min(c |snGrad| / ((1 - c) |corr| + SMALL), 1)`
///   and `snGrad = nonOrthDeltaCoeffs (phi_N - phi_P)` the uncorrected normal
///   gradient. ~~`Limited(c)` multiplied the correction by `c`~~ **CORRECTED
///   2026-11-11 (gh:#1362)**: that is a different scheme (O(1) apart from
///   upstream's), read from `limitedSnGrad.C`.
#[must_use]
pub fn sn_grad_correction_with_grad(
    vol: &VolScalarField,
    g: &VolVectorField,
    scheme: NonOrthoScheme,
) -> Vec<f64> {
    let mesh = &vol.mesh;
    match scheme {
        NonOrthoScheme::Orthogonal => vec![0.0; mesh.n_internal_faces],
        NonOrthoScheme::Corrected => full_grad_correction(vol, g),
        NonOrthoScheme::Limited(c) => {
            let c = c.clamp(0.0, 1.0);
            let corr = full_grad_correction(vol, g);
            (0..mesh.n_internal_faces)
                .map(|f| {
                    let dc = DeltaCoeff::NonOrthogonal.interior_delta_coeff(mesh, f);
                    let sn = dc * (vol.internal[mesh.neighbour[f]] - vol.internal[mesh.owner[f]]);
                    let x = c * sn.abs() / ((1.0 - c) * corr[f].abs() + SMALL);
                    let limiter = if x < 1.0 { x } else { 1.0 };
                    limiter * corr[f]
                })
                .collect()
        }
    }
}

/// Surface-normal gradient with the non-orthogonal correction of `scheme`
/// (`snGradSchemes`: `corrected`, `limited corrected c`), the Gauss-linear
/// gradient supplying the correction. [`NonOrthoScheme::Orthogonal`] returns
/// [`sn_grad`](super::sn_grad) (the `orthogonal` scheme, `deltaCoeffs`); the
/// other two use `nonOrthDeltaCoeffs` for the implicit part, as upstream.
#[must_use]
pub fn sn_grad_corrected(vol: &VolScalarField, scheme: NonOrthoScheme) -> SurfaceScalarField {
    let mode = match scheme {
        NonOrthoScheme::Orthogonal => DeltaCoeff::Orthogonal,
        _ => DeltaCoeff::NonOrthogonal,
    };
    let mut sn = sn_grad_with_delta(vol, mode);
    let g = grad(vol);
    let corr = sn_grad_correction_with_grad(vol, &g, scheme);
    for (f, c) in corr.iter().enumerate() {
        sn.internal[f] += *c;
    }
    sn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::PatchField;
    use crate::fields::field::Field;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use std::sync::Arc;

    /// Two cells whose centre-to-centre vector is tilted against the face normal
    /// (a sheared pair), so the correction vector is non-zero.
    fn sheared_pair() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.2, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.1, 0.0),
                    Vector3::new(1.0, 0.2, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    fn field(mesh: &Arc<FvMesh>) -> VolScalarField {
        VolScalarField::new(
            "T",
            mesh.clone(),
            Field::new(vec![1.0, 3.0]),
            vec![PatchField::fixed_value(1, 4.0), PatchField::fixed_value(1, 0.5)],
        )
    }

    #[test]
    fn correction_vector_is_zero_on_an_orthogonal_face_and_not_on_a_tilted_one() {
        let m = sheared_pair();
        let k = non_orth_correction_vectors(&m);
        assert!(k[0].mag() > 1e-3, "tilted face must have a correction vector");
    }

    #[test]
    fn limited_scheme_is_bounded_by_the_corrected_one_and_zero_limit_is_uncorrected() {
        let m = sheared_pair();
        let t = field(&m);
        let g = grad(&t);
        let full = sn_grad_correction_with_grad(&t, &g, NonOrthoScheme::Corrected);
        let lim = sn_grad_correction_with_grad(&t, &g, NonOrthoScheme::Limited(0.33));
        let zero = sn_grad_correction_with_grad(&t, &g, NonOrthoScheme::Limited(0.0));
        assert!(lim[0].abs() <= full[0].abs() + 1e-15);
        assert_eq!(zero[0], 0.0);
        let sn = sn_grad_corrected(&t, NonOrthoScheme::Corrected);
        let un = sn_grad_with_delta(&t, DeltaCoeff::NonOrthogonal);
        assert_eq!(sn.internal[0], un.internal[0] + full[0]);
        let orth = sn_grad_corrected(&t, NonOrthoScheme::Orthogonal);
        assert_eq!(orth.internal[0], super::super::sn_grad(&t).internal[0]);
    }
}
