// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Verification tests for the cross-section data model
//!
//! These are **verification** tests (is the interpolation/storage implemented
//! correctly?), not validation against a criticality benchmark — a `k_eff`
//! comparison needs the diffusion/SP3/SN solver, which is a later slice. The
//! references used here are analytic: exact reproduction of the input at the
//! sampled states, and the closed-form behaviour of a `mode = 1` polyharmonic
//! spline (which reduces to exact linear interpolation for two states), applied
//! through the `log` Doppler transform.
//!
//! The synthetic 2-group data set in [`two_group_input`] is illustrative (not a
//! published evaluation); it exercises every quantity while keeping the expected
//! interpolation results hand-computable.

use super::*;
use crate::genfoam::common::rbf;
use crate::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};
use crate::genfoam::neutronics::xs::nuclear_data_one_energy::NuclearDataOneEnergy;
use crate::genfoam::neutronics::xs::variables::{VariableLaw, XsVariable};
use approx::assert_relative_eq;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// RBF basis-function unit checks
// ---------------------------------------------------------------------------

/// Methodology: evaluate `polyharmonic_spline_function(r^2, mode)` for the four
/// GeN-Foam spline modes at `r^2 = 4` (`r = 2`) and compare against the
/// closed forms `r`, `r^2 ln r`, `r^3`, `r^4 ln r`.
///
/// Result (2026-07-15): mode 1 -> 2.0; mode 2 -> 4 ln 2 = 2.7726; mode 3 -> 8.0;
/// mode 4 -> 16 ln 2 = 11.0904; out-of-range mode -> 0.0. All match to 1e-12.
#[test]
fn rbf_basis_function_closed_forms() {
    let r2 = 4.0_f64;
    let r = r2.sqrt();
    assert_relative_eq!(rbf::polyharmonic_spline_function(r2, 1), r, epsilon = 1e-12);
    assert_relative_eq!(
        rbf::polyharmonic_spline_function(r2, 2),
        r2 * r.ln(),
        epsilon = 1e-12
    );
    assert_relative_eq!(
        rbf::polyharmonic_spline_function(r2, 3),
        r * r2,
        epsilon = 1e-12
    );
    assert_relative_eq!(
        rbf::polyharmonic_spline_function(r2, 4),
        r2 * r2 * r.ln(),
        epsilon = 1e-12
    );
    assert_eq!(rbf::polyharmonic_spline_function(r2, 7), 0.0);
}

// ---------------------------------------------------------------------------
// NuclearDataOneEnergy — interpolation properties
// ---------------------------------------------------------------------------

/// Methodology: an interpolant must reproduce its own data. Build a
/// single-parameter `NuclearDataOneEnergy` (mode 1) with three states at
/// parameter values 0, 1, 3 and values 10, 12, 9, then query at each node.
///
/// Result (2026-07-15): `get` returns 10, 12, 9 at the three nodes to a relative
/// 1e-9; `get_ref` returns 10 (the first/reference value). Interpolation
/// reproduction property verified.
#[test]
fn one_energy_reproduces_nodes() {
    let mut nd = NuclearDataOneEnergy::new(1, 1);
    let nodes = [(0.0, 10.0), (1.0, 12.0), (3.0, 9.0)];
    for (x, v) in nodes {
        nd.add_data(v, &[x]);
    }
    nd.build().unwrap();

    assert_relative_eq!(nd.get_ref(), 10.0, epsilon = 1e-12);
    for (x, v) in nodes {
        assert_relative_eq!(nd.get(&[x], true), v, epsilon = 1e-9);
    }
}

/// Methodology: a `mode = 1` polyharmonic spline through **two** points has
/// vanishing radial weights (the orthogonality constraints force
/// `w_0 = w_1 = 0`), leaving the exact linear polynomial through the two nodes.
/// Build two states at `x = 100, 200` with values `2.0, 5.0` and query the
/// interior point `x = 140`.
///
/// Result (2026-07-15): expected linear value `2 + (5-2)*(140-100)/(200-100) =
/// 3.2`; `get` returns 3.2 to 1e-9. Exact-linear-interpolation property
/// verified.
#[test]
fn one_energy_two_point_is_linear() {
    let mut nd = NuclearDataOneEnergy::new(1, 1);
    nd.add_data(2.0, &[100.0]);
    nd.add_data(5.0, &[200.0]);
    nd.build().unwrap();

    let x = 140.0;
    let expected = 2.0 + (5.0 - 2.0) * (x - 100.0) / (200.0 - 100.0);
    assert_relative_eq!(nd.get(&[x], true), expected, epsilon = 1e-9);
    assert_relative_eq!(expected, 3.2, epsilon = 1e-12);
}

/// Methodology: a parameter that is identical across all states cannot be
/// interpolated (it would make the RBF matrix singular). Build two states whose
/// single parameter is the same (`5.0`) but whose values differ (`1.0, 2.0`);
/// after `build`, the value must be treated as constant.
///
/// Result (2026-07-15): `get` returns the reference value 1.0 regardless of
/// query point; the reduced-parameter guard is exercised. Also confirms
/// `is_parametrize = false` returns the reference value.
#[test]
fn one_energy_constant_parameter_falls_back_to_reference() {
    let mut nd = NuclearDataOneEnergy::new(1, 1);
    nd.add_data(1.0, &[5.0]);
    nd.add_data(2.0, &[5.0]);
    nd.build().unwrap();
    assert_relative_eq!(nd.get(&[5.0], true), 1.0, epsilon = 1e-12);
    assert_relative_eq!(nd.get(&[999.0], true), 1.0, epsilon = 1e-12);
    // Parametrisation switched off always yields the reference value.
    assert_relative_eq!(nd.get(&[5.0], false), 1.0, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// A small documented 2-group data set
// ---------------------------------------------------------------------------

/// Reference-state fuel temperature (K).
const T_REF: f64 = 900.0;
/// Perturbed-state fuel temperature (K).
const T_HOT: f64 = 1800.0;
/// Reference / perturbed group-0 `nu Sigma_f` (m^-1) — the only Doppler-varied
/// quantity in the synthetic set.
const NUSF0_REF: f64 = 0.006_00;
const NUSF0_HOT: f64 = 0.005_70;
/// Reference-state scattering `Sigma_{s, 0->1}` (m^-1).
const SCATTER_01: f64 = 0.015;

/// A synthetic 2-group / 1-precursor / 1-zone `nuclearData` set with a single
/// Doppler feedback variable (`Tfuel`, `log` law) and two reactor states.
fn two_group_input() -> NuclearDataInput {
    let scattering_ref = vec![vec![
        vec![0.0, SCATTER_01], // j = 0 : to group 0 (0), to group 1 (down-scatter)
        vec![0.0, 0.0],        // j = 1 : none
    ]];
    let scattering_hot = scattering_ref.clone();

    let core_ref = ZoneStateInput {
        name: "core".to_string(),
        d: vec![0.015, 0.008],
        nu_sigma_eff: vec![NUSF0_REF, 0.120],
        sigma_pow: vec![1.0e-11, 2.0e-11],
        sigma_removal: vec![0.020, 0.100],
        chi_prompt: vec![1.0, 0.0],
        chi_delayed: vec![1.0, 0.0],
        scattering: scattering_ref,
        constants: Some(ZoneConstantsInput {
            fuel_fraction: 1.0,
            secondary_power_volume_fraction: 1.0,
            fraction_to_secondary_power: 0.0,
            df_adjust: true,
            iv: vec![1.0e-7, 1.0e-6],
            disc_factor: vec![1.0, 1.0],
            integral_flux: vec![1.0, 1.0],
            beta: vec![0.0065],
            lambda: vec![0.08],
        }),
    };

    let core_hot = ZoneStateInput {
        name: "core".to_string(),
        d: vec![0.015, 0.008],
        nu_sigma_eff: vec![NUSF0_HOT, 0.120],
        sigma_pow: vec![1.0e-11, 2.0e-11],
        sigma_removal: vec![0.020, 0.100],
        chi_prompt: vec![1.0, 0.0],
        chi_delayed: vec![1.0, 0.0],
        scattering: scattering_hot,
        constants: None, // perturbed state: no constants block
    };

    let mut ref_params = BTreeMap::new();
    ref_params.insert("Tfuel".to_string(), T_REF);
    let mut hot_params = BTreeMap::new();
    hot_params.insert("Tfuel".to_string(), T_HOT);

    NuclearDataInput {
        energy_groups: 2,
        prec_groups: 1,
        legendre_moments: 1,
        poly_spline_mode: 1,
        fast_neutrons: true,
        xs_variables: vec![XsVariable::new("Tfuel", VariableLaw::Log)],
        do_not_parametrize: Vec::new(),
        states: vec![
            StateInput {
                name: "reference".to_string(),
                parameters: ref_params,
                zones: vec![core_ref],
            },
            StateInput {
                name: "Tfuel1800".to_string(),
                parameters: hot_params,
                zones: vec![core_hot],
            },
        ],
    }
}

/// Methodology: build the [`two_group_input`] set and read the reference group
/// constants for zone `core`. Compare each `uom`-typed field's base-SI value
/// against the dictionary input.
///
/// Result (2026-07-15): group 0 gives D = 0.015 m, `nu Sigma_f` = 0.006 m^-1,
/// `kappa Sigma_f` = 1e-11 J/m, `Sigma_r` = 0.020 m^-1, `chi_p` = 1.0,
/// `1/v` = 1e-7 s/m; group 1 `nu Sigma_f` = 0.120 m^-1. All match to 1e-12.
/// Reference readout + `uom` wrapping verified.
#[test]
fn reference_group_constants_match_input() {
    let xs = CrossSectionData::from_input(&two_group_input()).unwrap();
    let zi = xs.zone_index("core").unwrap();

    let g0 = xs.reference_group_constants(zi, 0).unwrap();
    assert_relative_eq!(g0.diffusion_coefficient.value, 0.015, epsilon = 1e-12);
    assert_relative_eq!(g0.nu_sigma_f.value, NUSF0_REF, epsilon = 1e-12);
    assert_relative_eq!(g0.sigma_pow.value, 1.0e-11, epsilon = 1e-23);
    assert_relative_eq!(g0.sigma_removal.value, 0.020, epsilon = 1e-12);
    assert_relative_eq!(g0.chi_prompt.value, 1.0, epsilon = 1e-12);
    assert_relative_eq!(g0.inverse_velocity.value, 1.0e-7, epsilon = 1e-19);

    let g1 = xs.reference_group_constants(zi, 1).unwrap();
    assert_relative_eq!(g1.nu_sigma_f.value, 0.120, epsilon = 1e-12);
    assert_relative_eq!(g1.chi_prompt.value, 0.0, epsilon = 1e-12);
}

/// Methodology: evaluate group-0 `nu Sigma_f` at the two sampled temperatures
/// and at the geometric mean `sqrt(T_ref * T_hot)`. Because the transform is
/// `log`, the geometric mean maps to the midpoint of the interpolation
/// coordinate, so a mode-1 (linear) two-point spline must return the arithmetic
/// mean of the two values.
///
/// Result (2026-07-15): at T = 900 K -> 0.00600 m^-1; at T = 1800 K ->
/// 0.00570 m^-1; at T = sqrt(900*1800) = 1272.79 K -> 0.00585 m^-1 =
/// (0.00600 + 0.00570)/2, to 1e-9. Doppler `log`-law interpolation verified.
#[test]
fn doppler_log_interpolation_hits_arithmetic_mean_at_geometric_mean_temperature() {
    let xs = CrossSectionData::from_input(&two_group_input()).unwrap();
    let zi = xs.zone_index("core").unwrap();

    let at_ref = xs.evaluate_group_constants(zi, 0, &[T_REF]).unwrap();
    assert_relative_eq!(at_ref.nu_sigma_f.value, NUSF0_REF, epsilon = 1e-9);

    let at_hot = xs.evaluate_group_constants(zi, 0, &[T_HOT]).unwrap();
    assert_relative_eq!(at_hot.nu_sigma_f.value, NUSF0_HOT, epsilon = 1e-9);

    let t_mid = (T_REF * T_HOT).sqrt();
    let at_mid = xs.evaluate_group_constants(zi, 0, &[t_mid]).unwrap();
    assert_relative_eq!(
        at_mid.nu_sigma_f.value,
        0.5 * (NUSF0_REF + NUSF0_HOT),
        epsilon = 1e-9
    );
}

/// Methodology: put energy group 0 in `doNotParametrize` and query at the hot
/// temperature. The suppressed group must return its reference value while an
/// un-suppressed group still interpolates.
///
/// Result (2026-07-15): group 0 `nu Sigma_f` stays at 0.00600 m^-1 (reference)
/// at T = 1800 K instead of the interpolated 0.00570; group 1 (whose data is
/// constant across states) returns 0.120 m^-1. `doNotParametrize` gating
/// verified.
#[test]
fn do_not_parametrize_suppresses_feedback() {
    let mut input = two_group_input();
    input.do_not_parametrize = vec![0];
    let xs = CrossSectionData::from_input(&input).unwrap();
    let zi = xs.zone_index("core").unwrap();

    let hot0 = xs.evaluate_group_constants(zi, 0, &[T_HOT]).unwrap();
    assert_relative_eq!(hot0.nu_sigma_f.value, NUSF0_REF, epsilon = 1e-12);

    let hot1 = xs.evaluate_group_constants(zi, 1, &[T_HOT]).unwrap();
    assert_relative_eq!(hot1.nu_sigma_f.value, 0.120, epsilon = 1e-9);
}

/// Methodology: request the P0 scattering matrix (an
/// `outram_foam_basic_lib::matrix::SquareMatrix`) at the reference temperature
/// and check the down-scatter entry and the zeros.
///
/// Result (2026-07-15): a 2x2 matrix with `(0,1) = 0.015 m^-1` and all other
/// entries 0; ordering `(j, i) = Sigma_{s, j->i}`. Scattering-matrix assembly
/// via `SquareMatrix` verified.
#[test]
fn scattering_matrix_reproduces_reference() {
    let xs = CrossSectionData::from_input(&two_group_input()).unwrap();
    let zi = xs.zone_index("core").unwrap();
    let m = xs.scattering_matrix(zi, 0, &[T_REF]).unwrap();

    assert_eq!(m.n(), 2);
    assert_relative_eq!(m.get(0, 1), SCATTER_01, epsilon = 1e-9);
    assert_relative_eq!(m.get(0, 0), 0.0, epsilon = 1e-12);
    assert_relative_eq!(m.get(1, 0), 0.0, epsilon = 1e-12);
    assert_relative_eq!(m.get(1, 1), 0.0, epsilon = 1e-12);
}

/// Methodology: read the precursor constants and check `beta_tot = sum_k beta_k`
/// and the decay constant.
///
/// Result (2026-07-15): one precursor group with `beta = 0.0065`, hence
/// `beta_tot = 0.0065` and `lambda = 0.08 s^-1`. Precursor readout verified.
#[test]
fn precursor_constants_sum_beta() {
    let xs = CrossSectionData::from_input(&two_group_input()).unwrap();
    let zi = xs.zone_index("core").unwrap();
    let prec = xs.precursor_constants(zi).unwrap();

    assert_eq!(prec.beta.len(), 1);
    assert_relative_eq!(prec.beta[0].value, 0.0065, epsilon = 1e-12);
    assert_relative_eq!(prec.beta_tot.value, 0.0065, epsilon = 1e-12);
    assert_relative_eq!(prec.lambda[0].value, 0.08, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// Error paths (never paper over bad input)
// ---------------------------------------------------------------------------

/// Methodology: a `nuclearData` set whose first state is not `reference` must be
/// rejected (GeN-Foam `readNuclearData.H` fatal check).
///
/// Result (2026-07-15): `from_input` returns
/// `XsError::ReferenceStateNotFirst { found: "notReference" }`.
#[test]
fn rejects_non_reference_first_state() {
    let mut input = two_group_input();
    input.states[0].name = "notReference".to_string();
    let err = CrossSectionData::from_input(&input).unwrap_err();
    assert!(matches!(err, XsError::ReferenceStateNotFirst { .. }));
}

/// Methodology: a variable declared in `xsVariables` but absent from the
/// reference state has no reference value; upstream requires it.
///
/// Result (2026-07-15): `from_input` returns
/// `XsError::MissingReferenceVariable { name: "rhoCool" }`.
#[test]
fn rejects_missing_reference_variable() {
    let mut input = two_group_input();
    input
        .xs_variables
        .push(XsVariable::new("rhoCool", VariableLaw::Linear));
    let err = CrossSectionData::from_input(&input).unwrap_err();
    assert!(matches!(err, XsError::MissingReferenceVariable { .. }));
}

/// Methodology: a group array of the wrong length must be rejected rather than
/// silently truncated.
///
/// Result (2026-07-15): a 1-element `nu_sigma_eff` in a 2-group set yields
/// `XsError::LengthMismatch { field: "nuSigmaEff", expected: 2, found: 1, .. }`.
#[test]
fn rejects_length_mismatch() {
    let mut input = two_group_input();
    input.states[0].zones[0].nu_sigma_eff = vec![NUSF0_REF]; // wrong length
    let err = CrossSectionData::from_input(&input).unwrap_err();
    assert!(matches!(
        err,
        XsError::LengthMismatch {
            field: "nuSigmaEff",
            ..
        }
    ));
}

/// Methodology: an out-of-range zone/group index into an accessor must error,
/// not panic or read stale memory.
///
/// Result (2026-07-15): zone index 5 (only 1 zone) and group index 9 (only 2
/// groups) both return `XsError::IndexOutOfRange`.
#[test]
fn rejects_out_of_range_indices() {
    let xs = CrossSectionData::from_input(&two_group_input()).unwrap();
    assert!(matches!(
        xs.reference_group_constants(5, 0).unwrap_err(),
        XsError::IndexOutOfRange { what: "zone", .. }
    ));
    assert!(matches!(
        xs.reference_group_constants(0, 9).unwrap_err(),
        XsError::IndexOutOfRange {
            what: "energy group",
            ..
        }
    ));
}
