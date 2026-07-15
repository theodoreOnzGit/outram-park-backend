// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/massFlow/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `massFlow` — total mass flow rate through a boundary patch or an
//! arbitrary set of faces.
//!
//! Port of `functionObjects::massFlow` (`massFlow.C`/`.H`). Upstream sums the
//! magnitude of an already area-integrated mass-flux face field
//! (`alphaRhoPhi`, OpenFOAM's `phi` convention: `rho_f * U_f . Sf`, units
//! kg/s per face) over the faces of a patch, faceSet, or faceZone:
//!
//! ```text
//! mdot = sum_faces |alphaRhoPhi_f|
//! ```
//!
//! Because `alphaRhoPhi` is already area-integrated, no separate multiply by
//! face area is needed here (contrast [`super::pressure_drop`] and
//! [`super::t_bulk`], which area-weight a *non*-integrated field).
//!
//! Upstream's `faceSet`/`faceZone` region kinds both reduce to "an arbitrary
//! list of face indices" (`labelList faces_`) — this port collapses them into
//! [`MassFlowRegion::Faces`]. Upstream also supports an optional
//! `scaleFactor` (default 1.0) applied to the final result; that isn't
//! modelled as a parameter here since [`MassRate`] is a `uom` quantity and
//! already supports `mdot * k` for a plain `f64` scale `k`.

use outram_foam_basic_lib::fields::SurfaceScalarField;
use outram_foam_basic_lib::mesh::FvMesh;
use uom::si::area::square_meter;
use uom::si::f64::{Area, MassRate};
use uom::si::mass_rate::kilogram_per_second;

use super::patch_geometry::patch_area;

/// Region a [`total_mass_flow`] (or [`region_area`]) integral is evaluated
/// over.
///
/// Mirrors upstream `massFlow::regionType` (`patch` / `faceSet` / `faceZone`);
/// the latter two both reduce to an explicit face-index list here, since
/// basic-lib has no named face-set/face-zone object — the caller resolves the
/// face list itself.
#[derive(Debug, Clone)]
pub enum MassFlowRegion {
    /// A named boundary patch, by index into [`FvMesh::patches`].
    Patch(usize),
    /// An arbitrary list of global face indices (internal or boundary),
    /// mirroring upstream `faceSet`/`faceZone`.
    Faces(Vec<usize>),
}

/// Total mass flow rate `mdot = sum_faces |alphaRhoPhi_f|` through `region`.
///
/// `alpha_rho_phi` must be the already area-integrated mass-flux face field
/// (`rho_f * U_f . Sf`, kg/s per face — OpenFOAM's `phi` convention),
/// evaluated on the same mesh `region` refers into. No separate `&FvMesh`
/// argument is needed: `alpha_rho_phi` already carries its own `Arc<FvMesh>`
/// (used internally by [`SurfaceScalarField::face_value`] for the `Faces`
/// case), and the `Patch` case only ever touches `alpha_rho_phi.boundary`.
/// Faces contribute by magnitude, matching upstream's
/// `mag(alphaRhoPhip[i])` — so flow direction does not cancel across faces of
/// a patch with mixed in/outflow, exactly as in GeN-Foam.
///
/// # Panics
/// If `region` names a patch index out of range for `alpha_rho_phi.boundary`,
/// or a face index out of range for the field's mesh.
pub fn total_mass_flow(alpha_rho_phi: &SurfaceScalarField, region: &MassFlowRegion) -> MassRate {
    let sum: f64 = match region {
        MassFlowRegion::Patch(pi) => alpha_rho_phi.boundary[*pi]
            .values
            .iter()
            .map(|v| v.abs())
            .sum(),
        MassFlowRegion::Faces(faces) => faces
            .iter()
            .map(|&f| alpha_rho_phi.face_value(f).abs())
            .sum(),
    };
    MassRate::new::<kilogram_per_second>(sum)
}

/// Total area of `region`'s faces — the `S` diagnostic upstream logs
/// alongside `mdot` (`"... massFlow = mDot kg/s over S m2"`).
///
/// # Panics
/// Same conditions as [`total_mass_flow`].
pub fn region_area(mesh: &FvMesh, region: &MassFlowRegion) -> Area {
    match region {
        MassFlowRegion::Patch(pi) => patch_area(mesh, *pi),
        MassFlowRegion::Faces(faces) => {
            let sum: f64 = faces.iter().map(|&f| mesh.face_areas[f]).sum();
            Area::new::<square_meter>(sum)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{BoundaryCondition, Field, PatchField};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    /// One cell, three boundary faces (unit area each) all on patch
    /// "outlet", no internal faces.
    fn single_cell_three_face_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .owner(vec![0, 0, 0])
                .neighbour(vec![])
                .patches(vec![BoundaryPatch::new("outlet", 0, 3, PatchKind::Patch)])
                .cell_volumes(vec![1.0])
                .cell_centres(vec![Vector3::new(0.5, 0.0, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.5, 0.0),
                    Vector3::new(1.0, -0.5, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    /// **V&V** — `total_mass_flow` against a hand-computed sum on a tiny
    /// constructed mesh.
    ///
    /// Methodology: a 1-cell mesh with three boundary faces on patch
    /// "outlet" (`single_cell_three_face_mesh`). `alphaRhoPhi` (already
    /// area-integrated mass flux, kg/s per face) is set to `[2.0, -3.0, 5.0]`
    /// kg/s — mixed in/outflow, deliberately including a negative face to
    /// exercise the `mag()` in upstream's `mDot += mag(alphaRhoPhip[i])`.
    /// Reference value by hand: `mdot = |2.0| + |-3.0| + |5.0| = 10.0` kg/s.
    /// Pass criterion: exact match to machine precision (pure summation, no
    /// iterative solve).
    ///
    /// Result (2026-07-15): computed `mdot = 10.0` kg/s exactly — matches the
    /// hand-computed reference to `< 1e-12` kg/s. Confirms magnitude-summation
    /// semantics (sign-insensitive) are ported correctly.
    #[test]
    fn total_mass_flow_matches_hand_computed_sum() {
        let mesh = single_cell_three_face_mesh();
        let values = Field::new(vec![2.0, -3.0, 5.0]);
        let alpha_rho_phi = SurfaceScalarField::new(
            "alphaRhoPhi",
            Arc::clone(&mesh),
            Field::new(vec![]),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(values.clone()),
                values,
            }],
        );

        let mdot = total_mass_flow(&alpha_rho_phi, &MassFlowRegion::Patch(0));
        let expected = 2.0_f64.abs() + (-3.0_f64).abs() + 5.0_f64.abs();
        assert!(
            (mdot.get::<kilogram_per_second>() - expected).abs() < 1e-12,
            "mdot = {} kg/s, expected {} kg/s",
            mdot.get::<kilogram_per_second>(),
            expected
        );
    }

    #[test]
    fn region_area_sums_three_unit_faces() {
        let mesh = single_cell_three_face_mesh();
        let a = region_area(&mesh, &MassFlowRegion::Patch(0));
        assert!((a.get::<square_meter>() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn faces_region_matches_patch_region_for_same_faces() {
        let mesh = single_cell_three_face_mesh();
        let values = Field::new(vec![2.0, -3.0, 5.0]);
        let alpha_rho_phi = SurfaceScalarField::new(
            "alphaRhoPhi",
            Arc::clone(&mesh),
            Field::new(vec![]),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(values.clone()),
                values,
            }],
        );

        let via_patch = total_mass_flow(&alpha_rho_phi, &MassFlowRegion::Patch(0));
        let via_faces = total_mass_flow(&alpha_rho_phi, &MassFlowRegion::Faces(vec![0, 1, 2]));
        assert!(
            (via_patch.get::<kilogram_per_second>() - via_faces.get::<kilogram_per_second>()).abs()
                < 1e-12
        );
    }
}
