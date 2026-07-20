// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/functionObjects/TBulk/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `TBulk` — flow-weighted (bulk / mixing-cup) temperature over a boundary
//! patch.
//!
//! Port of `functionObjects::TBulk` (`TBulk.C`/`.H`), **patch region kind
//! only** — see [`super::pressure_drop`] for why the `faceSet`/`faceZone`
//! variants (which upstream serves via `fvc::interpolate`) are out of scope.
//!
//! ```text
//! T_bulk = sum_faces(|alphaRhoPhi_f| * Cp_f * T_f) / sum_faces(|alphaRhoPhi_f| * Cp_f)
//! ```
//! the enthalpy-flux-weighted mean temperature — physically, the temperature
//! a perfectly-mixed downstream plenum would settle to. `Cp` weights both the
//! numerator and denominator to the same power, so its absolute scale/units
//! cancel in the ratio; only self-consistency between the two sums matters.
//! Upstream takes `thermo.Cp()`, a `volScalarField` with real `J/(kg K)`
//! units — any per-cell heat-capacity field works here as long as it's the
//! same field used to weight both sums (which [`bulk_temperature`] guarantees
//! by construction). The denominator is floored at `1e-9`, exactly matching
//! upstream's `Tb = hDot/max(hDotByT, 1e-9)`, to avoid a NaN when the patch
//! carries no flow.

use outram_foam_basic_lib::fields::{SurfaceScalarField, VolScalarField};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Floor on the enthalpy-flux weight sum — matches upstream's
/// `max(hDotByT, 1e-9)`.
const MIN_ENTHALPY_FLUX_WEIGHT: f64 = 1e-9;

/// Flow-weighted (bulk) temperature over boundary patch `patch_index`.
///
/// `temperature` and `specific_heat` are per-cell fields; their **boundary**
/// values on `patch_index` are used (upstream's `T.boundaryField()[patchID]`
/// / `Cp.boundaryField()[patchID]`). `alpha_rho_phi` is the area-integrated
/// mass-flux face field (see [`super::mass_flow::total_mass_flow`]) — its
/// boundary values on the same patch supply the flow weight, by magnitude
/// (matching upstream's `mag(alphaRhoPhip[i])`).
///
/// No mesh geometry (face areas) is needed here: unlike
/// [`super::pressure_drop::patch_average_pressure`], the weighting is already
/// carried by the area-integrated mass flux, not a raw field needing a
/// separate `|Sf|` multiply.
///
/// # Panics
/// If `patch_index` is out of range for any of the three fields' boundary
/// vectors, or the three patches disagree in face count (they must all
/// describe the same boundary patch of the same mesh).
pub fn bulk_temperature(
    patch_index: usize,
    temperature: &VolScalarField,
    specific_heat: &VolScalarField,
    alpha_rho_phi: &SurfaceScalarField,
) -> ThermodynamicTemperature {
    let t_vals = &temperature.boundary[patch_index].values;
    let cp_vals = &specific_heat.boundary[patch_index].values;
    let mdot_vals = &alpha_rho_phi.boundary[patch_index].values;
    assert_eq!(
        t_vals.len(),
        cp_vals.len(),
        "T/Cp patch face count mismatch"
    );
    assert_eq!(
        t_vals.len(),
        mdot_vals.len(),
        "T/alphaRhoPhi patch face count mismatch"
    );

    let mut h_dot = 0.0;
    let mut h_dot_by_t = 0.0;
    for i in 0..mdot_vals.len() {
        let weight = mdot_vals[i].abs() * cp_vals[i];
        h_dot_by_t += weight;
        h_dot += weight * t_vals[i];
    }

    ThermodynamicTemperature::new::<kelvin>(h_dot / h_dot_by_t.max(MIN_ENTHALPY_FLUX_WEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::{BoundaryCondition, Field, PatchField, VolField};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;
    use std::sync::Arc;

    /// One cell, three boundary faces (unit area each) all on patch
    /// "outlet", no internal faces — same fixture shape as
    /// `mass_flow::tests::single_cell_three_face_mesh`.
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

    /// **V&V** — `bulk_temperature` against a hand-computed enthalpy-flux
    /// weighted mean on a tiny constructed mesh.
    ///
    /// Methodology: the 1-cell/3-boundary-face "outlet" patch fixture
    /// (`single_cell_three_face_mesh`). `T` boundary values `[300, 350, 400]`
    /// K; `Cp` boundary values uniform `4200` J/(kg K) (chosen uniform
    /// specifically so the reference computation reduces to a plain
    /// mass-flux-weighted mean, isolating the `Cp`-cancellation claimed in
    /// this file's module doc); `alphaRhoPhi` boundary values
    /// `[2.0, -3.0, 5.0]` kg/s (mixed sign, magnitude-weighted, matching the
    /// `mass_flow` V&V fixture). Reference value by hand:
    /// `T_bulk = (2*300 + 3*350 + 5*400) / (2+3+5) = 3650/10 = 365.0` K.
    /// Pass criterion: exact match to machine precision (pure weighted-mean
    /// arithmetic, no iterative solve).
    ///
    /// Result (2026-07-15): computed `T_bulk = 365.0` K exactly — matches the
    /// hand-computed reference to `< 1e-9` K, and independently confirms the
    /// `Cp`-uniform-cancels-out claim (dividing the uniform `Cp = 4200` out of
    /// both sums recovers the same plain-weighted-mean formula).
    #[test]
    fn bulk_temperature_matches_hand_computed_enthalpy_weighted_mean() {
        let mesh = single_cell_three_face_mesh();

        let t = VolField::new(
            "T",
            Arc::clone(&mesh),
            Field::uniform(1, 300.0),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::new(vec![300.0, 350.0, 400.0])),
                values: Field::new(vec![300.0, 350.0, 400.0]),
            }],
        );
        let cp = VolField::new(
            "Cp",
            Arc::clone(&mesh),
            Field::uniform(1, 4200.0),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::uniform(3, 4200.0)),
                values: Field::uniform(3, 4200.0),
            }],
        );
        let alpha_rho_phi = SurfaceScalarField::new(
            "alphaRhoPhi",
            Arc::clone(&mesh),
            Field::new(vec![]),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::new(vec![2.0, -3.0, 5.0])),
                values: Field::new(vec![2.0, -3.0, 5.0]),
            }],
        );

        let t_bulk = bulk_temperature(0, &t, &cp, &alpha_rho_phi);
        let expected = (2.0 * 300.0 + 3.0 * 350.0 + 5.0 * 400.0) / (2.0 + 3.0 + 5.0);
        assert!(
            (t_bulk.get::<kelvin>() - expected).abs() < 1e-9,
            "T_bulk = {} K, expected {} K",
            t_bulk.get::<kelvin>(),
            expected
        );
    }

    #[test]
    fn zero_flow_patch_floors_at_minimum_weight_without_nan() {
        let mesh = single_cell_three_face_mesh();
        let t = VolField::new(
            "T",
            Arc::clone(&mesh),
            Field::uniform(1, 300.0),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::uniform(3, 300.0)),
                values: Field::uniform(3, 300.0),
            }],
        );
        let cp = VolField::new(
            "Cp",
            Arc::clone(&mesh),
            Field::uniform(1, 4200.0),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::uniform(3, 4200.0)),
                values: Field::uniform(3, 4200.0),
            }],
        );
        let alpha_rho_phi = SurfaceScalarField::new(
            "alphaRhoPhi",
            Arc::clone(&mesh),
            Field::new(vec![]),
            vec![PatchField {
                bc: BoundaryCondition::FixedField(Field::zeros(3)),
                values: Field::zeros(3),
            }],
        );

        let t_bulk = bulk_temperature(0, &t, &cp, &alpha_rho_phi);
        assert!(t_bulk.get::<kelvin>().is_finite());
    }
}
