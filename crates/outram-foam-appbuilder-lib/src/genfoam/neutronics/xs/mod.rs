// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/
//                    (XS.{C,H}, nuclearDataOneEnergy.{C,H},
//                     include/readNuclearData.H, setNeutronicsConstants.H,
//                     setNeutronicsVariables.H, setPrecConst.H) and
//                    src/classes/common/radialBasisFunctionInterpolation/
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

//! # `genfoam::neutronics::xs` — deterministic multigroup cross-section data
//!
//! Rust port of `GeN-Foam/src/classes/neutronics/XS` — the cross-section data
//! model the deterministic neutronics solvers (diffusion / SP3 / SN) consume.
//! It stores per-material-zone multigroup constants and interpolates them to the
//! current reactor feedback conditions (fuel/coolant temperature, coolant
//! density, expansion, ...).
//!
//! Cross sections are read from GeN-Foam `nuclearData` dictionaries and are
//! **deterministic and self-contained**: this module does not depend on
//! `njoy-outram-park-fork` or `outram-mc-libs`.
//!
//! ## What lives here
//!
//! - [`CrossSectionData`] — the top-level store: energy/precursor group counts,
//!   the ordered feedback variables ([`XsVariable`]), and one
//!   [`ZoneNuclearData`] per material zone. Built from a parsed
//!   [`NuclearDataInput`] and queried for typed [`GroupConstants`] /
//!   [`PrecursorConstants`] / a scattering [`SquareMatrix`].
//! - [`ZoneNuclearData`] — one zone's full group-wise data and constants.
//! - [`NuclearDataOneEnergy`] — one interpolated scalar value across states.
//!   Its polyharmonic-spline RBF kernel lives in
//!   [`crate::genfoam::common::rbf`] (shared with `multi_region`).
//! - [`units`] — named `uom` aliases (`MacroscopicCrossSection`,
//!   `DiffusionCoefficient`, `InverseVelocity`, ...).
//! - [`input`] — the plain-data mirror of the `nuclearData` dictionary.
//!
//! ## The parametrisation, end to end
//!
//! A `nuclearData` file lists reactor *states*: a mandatory `reference` state
//! plus perturbed states (`Tfuel1200K`, `TfuelAndRhoHot`, ...). Each state gives
//! the cross sections at a point in feedback-parameter space. This module fits a
//! polyharmonic spline through those points per (zone, energy group, quantity),
//! so a solver can ask for the cross sections at *arbitrary* feedback conditions
//! via [`CrossSectionData::evaluate_group_constants`]. Each variable's raw value
//! is passed through its [`variables::VariableLaw`] (`lin` / `log` / `sqrt`)
//! first, so the fit is done in a linearising space (e.g. `ln(T_fuel)` for
//! fast-spectrum Doppler).
//!
//! ## Scope of this port — data model only
//!
//! This is the **mesh-free data layer**: the zone-level interpolators and their
//! evaluation at a feedback point. GeN-Foam additionally *materialises* these
//! onto an `fvMesh` (filling one `volScalarField` per group by looping over the
//! cells of each `cellZone`), and handles control-rod driveline motion and
//! discontinuity-factor flux adjustment — all of which need the mesh /
//! multi-region layers and so are not implemented *in this module*. Mesh
//! materialisation has since landed next door, in
//! [`DiffusionXsFields`](crate::genfoam::neutronics::diffusion::DiffusionXsFields)
//! and its SP3 counterpart. **Control-rod driveline motion and
//! discontinuity-factor adjustment remain deferred** (tracked in beads under
//! the `neutronics` epic). The evaluation methods below are exactly the per-cell `.get(...)` call
//! GeN-Foam performs in `setNeutronicsVariables.H`, lifted out of the cell loop.
//!
//! ## Dependency on `genfoam::common`
//!
//! The polyharmonic-spline RBF kernel this module fits with lives in
//! [`crate::genfoam::common::rbf`], shared with
//! [`crate::genfoam::multi_region::rbf_mapping`] so there is a single
//! implementation of the algorithm.

pub mod error;
pub mod group_constants;
pub mod input;
pub mod nuclear_data_one_energy;
pub mod units;
pub mod variables;
pub mod zone_data;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

pub use error::XsError;
pub use group_constants::{GroupConstants, PrecursorConstants};
pub use input::{NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput};
pub use nuclear_data_one_energy::NuclearDataOneEnergy;
pub use variables::{VariableLaw, XsVariable};
pub use zone_data::ZoneNuclearData;

use outram_foam_basic_lib::matrix::SquareMatrix;

use units::{
    diffusion_coefficient, dimensionless, energy_release_cross_section, inverse_velocity,
    macroscopic_cross_section, precursor_decay_constant,
};

/// The deterministic multigroup cross-section store for a whole reactor.
///
/// Holds one [`ZoneNuclearData`] per material zone (in reference-state order),
/// the global group counts, and the ordered feedback variables. Construct it
/// with [`CrossSectionData::from_input`], then evaluate group constants at any
/// feedback state.
///
/// All public evaluation methods return `uom`-typed quantities; the parameter
/// vectors they take are **raw physical** values (K, kg/m^3, ...) in the order
/// of [`CrossSectionData::variable_names`] — the transform laws are applied
/// internally.
#[derive(Debug, Clone)]
pub struct CrossSectionData {
    energy_groups: usize,
    prec_groups: usize,
    legendre_moments: usize,
    fast_neutrons: bool,
    xs_variables: Vec<XsVariable>,
    do_not_parametrize: Vec<usize>,
    zones: Vec<ZoneNuclearData>,
    zone_index: HashMap<String, usize>,
}

impl CrossSectionData {
    /// Build the cross-section store from a parsed `nuclearData` dictionary.
    ///
    /// Mirrors GeN-Foam `XS::XS` + `readNuclearData.H` + `XS::init`: validates
    /// the `reference`-first state ordering, reads each zone's constants from the
    /// reference state, registers every state's variable cross sections as a
    /// polyharmonic-spline data point, and solves all interpolators. A perturbed
    /// state may reference a subset of the zones; a zone name not present in the
    /// reference state is skipped (as upstream warns and continues).
    ///
    /// # Errors
    ///
    /// Any [`XsError`]: empty/misordered states, a missing reference variable or
    /// zone-constants block, a length-mismatched data array, an unknown transform
    /// law, or a singular RBF solve.
    pub fn from_input(input: &NuclearDataInput) -> Result<Self, XsError> {
        // --- validate the states list -------------------------------------
        let reference = input.states.first().ok_or(XsError::NoStates)?;
        if reference.name != "reference" {
            return Err(XsError::ReferenceStateNotFirst {
                found: reference.name.clone(),
            });
        }

        let n_parameters = input.xs_variables.len();

        // Reference (untransformed) value of each variable — mandatory.
        let mut reference_values = Vec::with_capacity(n_parameters);
        for var in &input.xs_variables {
            let value = reference
                .parameters
                .get(&var.name)
                .copied()
                .ok_or_else(|| XsError::MissingReferenceVariable {
                    name: var.name.clone(),
                })?;
            reference_values.push(value);
        }

        // --- create zones from the reference state ------------------------
        let mut zones = Vec::with_capacity(reference.zones.len());
        let mut zone_index = HashMap::new();
        for (i, zone) in reference.zones.iter().enumerate() {
            zones.push(ZoneNuclearData::new_reference(
                zone,
                input.energy_groups,
                input.prec_groups,
                input.legendre_moments,
                input.poly_spline_mode,
                n_parameters,
            )?);
            zone_index.insert(zone.name.clone(), i);
        }

        let mut data = Self {
            energy_groups: input.energy_groups,
            prec_groups: input.prec_groups,
            legendre_moments: input.legendre_moments,
            fast_neutrons: input.fast_neutrons,
            xs_variables: input.xs_variables.clone(),
            do_not_parametrize: input.do_not_parametrize.clone(),
            zones,
            zone_index,
        };

        // --- register each state's data points ----------------------------
        for state in &input.states {
            let parameters = data.transform_state_parameters(state, &reference_values);
            for zone in &state.zones {
                if let Some(&zi) = data.zone_index.get(&zone.name) {
                    data.zones[zi].add_state_data(zone, &parameters)?;
                }
                // A zone absent from the reference state is skipped, matching
                // GeN-Foam's warn-and-continue behaviour.
            }
        }

        // --- solve every interpolator -------------------------------------
        for zone in &mut data.zones {
            zone.build()?;
        }

        Ok(data)
    }

    /// Apply each variable's transform law to a state's parameters, filling
    /// missing variables from the reference values (upstream `readNuclearData`
    /// transform loop). Returns the transformed vector in variable order.
    fn transform_state_parameters(&self, state: &StateInput, reference_values: &[f64]) -> Vec<f64> {
        self.xs_variables
            .iter()
            .zip(reference_values)
            .map(|(var, &ref_value)| {
                let raw = state
                    .parameters
                    .get(&var.name)
                    .copied()
                    .unwrap_or(ref_value);
                var.law.transform(raw)
            })
            .collect()
    }

    /// Transform a caller-supplied vector of **raw physical** parameter values
    /// (in [`Self::variable_names`] order) into interpolation space by applying
    /// each variable's law. This is the same transform used on the stored
    /// reference states, so the query lands in the fitted coordinate system.
    ///
    /// # Panics
    ///
    /// Panics if `raw_parameters.len()` does not equal the number of variables.
    #[must_use]
    pub fn transform_parameters(&self, raw_parameters: &[f64]) -> Vec<f64> {
        assert_eq!(
            raw_parameters.len(),
            self.xs_variables.len(),
            "raw parameter vector must have one entry per xsVariable"
        );
        self.xs_variables
            .iter()
            .zip(raw_parameters)
            .map(|(var, &raw)| var.law.transform(raw))
            .collect()
    }

    /// Whether energy group `group` is parametrised (has feedback applied), i.e.
    /// it is **not** in the `doNotParametrize` list.
    #[must_use]
    fn is_parametrized(&self, group: usize) -> bool {
        !self.do_not_parametrize.contains(&group)
    }

    // ---- metadata -----------------------------------------------------------

    /// Number of energy groups `G`.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.energy_groups
    }
    /// Number of delayed-neutron precursor groups.
    #[must_use]
    pub fn prec_groups(&self) -> usize {
        self.prec_groups
    }
    /// Number of stored Legendre scattering moments (`1 + legendreMoments`).
    #[must_use]
    pub fn legendre_moments(&self) -> usize {
        self.legendre_moments
    }
    /// Fast-spectrum flag as read from the dictionary (metadata only).
    #[must_use]
    pub fn fast_neutrons(&self) -> bool {
        self.fast_neutrons
    }
    /// The feedback variables in order; the layout of every parameter vector.
    #[must_use]
    pub fn variable_names(&self) -> Vec<&str> {
        self.xs_variables.iter().map(|v| v.name.as_str()).collect()
    }
    /// The feedback variables (name + law) in order.
    #[must_use]
    pub fn variables(&self) -> &[XsVariable] {
        &self.xs_variables
    }
    /// Number of material zones.
    #[must_use]
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }
    /// Look up a zone's index by name.
    #[must_use]
    pub fn zone_index(&self, name: &str) -> Option<usize> {
        self.zone_index.get(name).copied()
    }
    /// Borrow a zone by index.
    #[must_use]
    pub fn zone(&self, index: usize) -> Option<&ZoneNuclearData> {
        self.zones.get(index)
    }

    // ---- typed evaluation ---------------------------------------------------

    /// Feedback-interpolated [`GroupConstants`] for zone `zone`, energy group
    /// `group`, at the **raw physical** feedback point `raw_parameters` (in
    /// [`Self::variable_names`] order).
    ///
    /// This is the port of GeN-Foam's per-cell `.get(...)` block in
    /// `setNeutronicsVariables.H`, lifted out of the mesh loop. Groups listed in
    /// `doNotParametrize` return their reference values unchanged.
    ///
    /// # Errors
    ///
    /// [`XsError::IndexOutOfRange`] if `zone` or `group` is out of range.
    pub fn evaluate_group_constants(
        &self,
        zone: usize,
        group: usize,
        raw_parameters: &[f64],
    ) -> Result<GroupConstants, XsError> {
        let z = self.checked_zone(zone)?;
        self.check_group(group)?;
        let params = self.transform_parameters(raw_parameters);
        let parametrize = self.is_parametrized(group);

        Ok(GroupConstants {
            diffusion_coefficient: diffusion_coefficient(z.diffusion_coefficient(
                group,
                &params,
                parametrize,
            )),
            nu_sigma_f: macroscopic_cross_section(z.nu_sigma_f(group, &params, parametrize)),
            sigma_pow: energy_release_cross_section(z.sigma_pow(group, &params, parametrize)),
            sigma_removal: macroscopic_cross_section(z.sigma_removal(group, &params, parametrize)),
            chi_prompt: dimensionless(z.chi_prompt(group, &params, parametrize)),
            chi_delayed: dimensionless(z.chi_delayed(group, &params, parametrize)),
            inverse_velocity: inverse_velocity(z.inverse_velocity(group)),
            disc_factor: dimensionless(z.disc_factor(group)),
        })
    }

    /// The reference (feedback-free) [`GroupConstants`] for zone `zone`, group
    /// `group` — the values from the `reference` state (GeN-Foam
    /// `setNeutronicsConstants.H` `getRef()` pass).
    ///
    /// # Errors
    ///
    /// [`XsError::IndexOutOfRange`] if `zone` or `group` is out of range.
    pub fn reference_group_constants(
        &self,
        zone: usize,
        group: usize,
    ) -> Result<GroupConstants, XsError> {
        let z = self.checked_zone(zone)?;
        self.check_group(group)?;
        // getRef() == get() with parametrisation disabled.
        let no_params: [f64; 0] = [];
        Ok(GroupConstants {
            diffusion_coefficient: diffusion_coefficient(
                z.diffusion_coefficient(group, &no_params, false),
            ),
            nu_sigma_f: macroscopic_cross_section(z.nu_sigma_f(group, &no_params, false)),
            sigma_pow: energy_release_cross_section(z.sigma_pow(group, &no_params, false)),
            sigma_removal: macroscopic_cross_section(z.sigma_removal(group, &no_params, false)),
            chi_prompt: dimensionless(z.chi_prompt(group, &no_params, false)),
            chi_delayed: dimensionless(z.chi_delayed(group, &no_params, false)),
            inverse_velocity: inverse_velocity(z.inverse_velocity(group)),
            disc_factor: dimensionless(z.disc_factor(group)),
        })
    }

    /// The feedback-interpolated scattering matrix `Sigma_{s, j->i}` (m^-1) for
    /// zone `zone` and Legendre `moment`, as an
    /// `outram_foam_basic_lib::matrix::SquareMatrix` of order `energy_groups`.
    ///
    /// Entry `(j, i)` is the scatter from group `j` into group `i`. Parametrised
    /// per **destination** group `i` (matching GeN-Foam's `sigmaFromTo` member,
    /// which checks `energyI`), so a destination group in `doNotParametrize`
    /// keeps its reference scattering.
    ///
    /// # Errors
    ///
    /// [`XsError::IndexOutOfRange`] if `zone` or `moment` is out of range.
    pub fn scattering_matrix(
        &self,
        zone: usize,
        moment: usize,
        raw_parameters: &[f64],
    ) -> Result<SquareMatrix, XsError> {
        let z = self.checked_zone(zone)?;
        if moment >= self.legendre_moments {
            return Err(XsError::IndexOutOfRange {
                what: "Legendre moment",
                index: moment,
                count: self.legendre_moments,
            });
        }
        let params = self.transform_parameters(raw_parameters);
        let mut matrix = SquareMatrix::new(self.energy_groups);
        for j in 0..self.energy_groups {
            for i in 0..self.energy_groups {
                let value = z.scattering(moment, j, i, &params, self.is_parametrized(i));
                matrix.set(j, i, value);
            }
        }
        Ok(matrix)
    }

    /// The delayed-neutron precursor constants for zone `zone`
    /// (`beta_k`, `beta_tot`, `lambda_k`).
    ///
    /// # Errors
    ///
    /// [`XsError::IndexOutOfRange`] if `zone` is out of range.
    pub fn precursor_constants(&self, zone: usize) -> Result<PrecursorConstants, XsError> {
        let z = self.checked_zone(zone)?;
        let beta = (0..self.prec_groups)
            .map(|k| dimensionless(z.beta(k)))
            .collect();
        let lambda = (0..self.prec_groups)
            .map(|k| precursor_decay_constant(z.lambda(k)))
            .collect();
        Ok(PrecursorConstants {
            beta,
            beta_tot: dimensionless(z.beta_tot()),
            lambda,
        })
    }

    // ---- internal bounds checks --------------------------------------------

    fn checked_zone(&self, zone: usize) -> Result<&ZoneNuclearData, XsError> {
        self.zones.get(zone).ok_or(XsError::IndexOutOfRange {
            what: "zone",
            index: zone,
            count: self.zones.len(),
        })
    }

    fn check_group(&self, group: usize) -> Result<(), XsError> {
        if group < self.energy_groups {
            Ok(())
        } else {
            Err(XsError::IndexOutOfRange {
                what: "energy group",
                index: group,
                count: self.energy_groups,
            })
        }
    }
}
