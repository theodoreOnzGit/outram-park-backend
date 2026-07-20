// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/XS/include/readNuclearData.H
//                    (the `states`/`zones` nuclearData dictionary schema)
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

//! # Plain-data mirror of the `nuclearData` dictionary
//!
//! GeN-Foam reads its cross sections from an OpenFOAM `IOdictionary` named
//! `nuclearData` (a `states` list, each with a `zones` list). This crate keeps
//! **dictionary parsing** in the `io` layer; the structs here are the parsed,
//! in-memory representation that [`super::CrossSectionData::from_input`]
//! consumes. They are deterministic and self-contained — no mesh, no OpenFOAM
//! I/O, no NJOY/Monte-Carlo data.
//!
//! ## Schema (mirrors the upstream `nuclearData` file)
//!
//! - [`NuclearDataInput`] — top-level scalars, the `xsVariables` list, and the
//!   ordered `states`.
//! - [`StateInput`] — one reactor state (`reference` first), its perturbed
//!   feedback-parameter values, and its per-zone data.
//! - [`ZoneStateInput`] — the group-wise **variable** cross sections for one
//!   zone in one state, plus (reference state only) the zone's constant data.
//! - [`ZoneConstantsInput`] — the feedback-independent per-zone constants
//!   (`IV`, `Beta`, `lambda`, `discFactor`, ...), read from the reference state.
//!
//! All cross sections are in MKSA (per-metre) units, matching the `nuclearData`
//! convention.

use crate::genfoam::neutronics::xs::variables::XsVariable;
use std::collections::BTreeMap;

/// Top-level parsed `nuclearData` dictionary.
#[derive(Debug, Clone)]
pub struct NuclearDataInput {
    /// Number of energy groups `G`.
    pub energy_groups: usize,
    /// Number of delayed-neutron precursor groups.
    pub prec_groups: usize,
    /// Number of stored Legendre scattering moments, `1 + legendreMoments`
    /// (GeN-Foam always stores at least the `P0` moment, hence the `+1`; supply
    /// the already-incremented count here).
    pub legendre_moments: usize,
    /// Polyharmonic-spline mode (basis order), GeN-Foam `polyharmonicSplineMode`
    /// (default `1`).
    pub poly_spline_mode: usize,
    /// Fast-spectrum flag (`fastNeutrons`). Retained as metadata; in current
    /// GeN-Foam the Doppler transform is chosen per variable via its
    /// [`super::variables::VariableLaw`] (`log` vs `sqrt`), not by this flag.
    pub fast_neutrons: bool,
    /// Ordered feedback variables (`xsVariables`). Their order fixes the layout
    /// of every parameter vector used thereafter.
    pub xs_variables: Vec<XsVariable>,
    /// Energy-group indices excluded from parametrisation (`doNotParametrize`):
    /// these groups always return their reference cross sections.
    pub do_not_parametrize: Vec<usize>,
    /// Reactor states in dictionary order; `states[0]` must be `reference`.
    pub states: Vec<StateInput>,
}

/// One reactor state (`reference` or a perturbed state).
#[derive(Debug, Clone)]
pub struct StateInput {
    /// State name; the first state must be `"reference"`.
    pub name: String,
    /// Perturbed feedback-parameter values, keyed by variable name (raw physical
    /// value, *before* any transform law). Missing variables default to the
    /// reference-state value.
    pub parameters: BTreeMap<String, f64>,
    /// Per-zone data for this state.
    pub zones: Vec<ZoneStateInput>,
}

/// The cross-section data for one material zone within one state.
#[derive(Debug, Clone)]
pub struct ZoneStateInput {
    /// Cell-zone name (must match a zone declared in the reference state).
    pub name: String,
    /// Diffusion coefficient `D_g`, length `energy_groups` (m).
    pub d: Vec<f64>,
    /// Fission-neutron production `nu Sigma_{f,g}`, length `energy_groups`
    /// (m^-1).
    pub nu_sigma_eff: Vec<f64>,
    /// Fission energy release `kappa Sigma_{f,g}`, length `energy_groups`
    /// (J/m).
    pub sigma_pow: Vec<f64>,
    /// Removal cross section `Sigma_{r,g}`, length `energy_groups` (m^-1).
    pub sigma_removal: Vec<f64>,
    /// Prompt spectrum `chi_{p,g}`, length `energy_groups` (dimensionless).
    pub chi_prompt: Vec<f64>,
    /// Delayed spectrum `chi_{d,g}`, length `energy_groups` (dimensionless).
    pub chi_delayed: Vec<f64>,
    /// Scattering matrices, one per Legendre moment. `scattering[moment][j][i]`
    /// is `Sigma_{s, j -> i}` for moment `moment`; each matrix is
    /// `energy_groups x energy_groups` and there must be `legendre_moments` of
    /// them (`scatteringMatrixP0`, `scatteringMatrixP1`, ...).
    pub scattering: Vec<Vec<Vec<f64>>>,
    /// Feedback-independent constants for this zone. **Required in the reference
    /// state**, ignored (may be `None`) in perturbed states.
    pub constants: Option<ZoneConstantsInput>,
}

/// The feedback-independent constant data for one zone, read from the reference
/// state (GeN-Foam reads these only in the reference-zone pass).
#[derive(Debug, Clone)]
pub struct ZoneConstantsInput {
    /// Fuel volume fraction `alpha_fuel` per lattice volume (default `1`).
    pub fuel_fraction: f64,
    /// Volume fraction of secondary power-producing structure (default `1`).
    pub secondary_power_volume_fraction: f64,
    /// Fraction of total power going to the secondary structure (default `0`).
    pub fraction_to_secondary_power: f64,
    /// Whether discontinuity factors are adjusted for this zone (`dfAdjust`,
    /// default `true`).
    pub df_adjust: bool,
    /// Inverse velocity `1/v_g`, length `energy_groups` (s/m).
    pub iv: Vec<f64>,
    /// Discontinuity factors `gamma_g`, length `energy_groups` (dimensionless).
    pub disc_factor: Vec<f64>,
    /// Integral fluxes `Phi_g` used to adapt discontinuity factors, length
    /// `energy_groups`.
    pub integral_flux: Vec<f64>,
    /// Delayed-neutron fractions `beta_k`, length `prec_groups` (dimensionless).
    pub beta: Vec<f64>,
    /// Precursor decay constants `lambda_k`, length `prec_groups` (s^-1).
    pub lambda: Vec<f64>,
}

impl Default for ZoneConstantsInput {
    /// The GeN-Foam defaults for the scalar factors, with empty group/precursor
    /// arrays (which the caller must fill to the right length).
    fn default() -> Self {
        Self {
            fuel_fraction: 1.0,
            secondary_power_volume_fraction: 1.0,
            fraction_to_secondary_power: 0.0,
            df_adjust: true,
            iv: Vec::new(),
            disc_factor: Vec::new(),
            integral_flux: Vec::new(),
            beta: Vec::new(),
            lambda: Vec::new(),
        }
    }
}
