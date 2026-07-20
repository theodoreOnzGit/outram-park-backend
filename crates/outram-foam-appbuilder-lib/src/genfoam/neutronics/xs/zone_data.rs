// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/XS/XS.{C,H} and
//                    include/readNuclearData.H (the per-cellzone data lists:
//                    DList_/nuSigmaEffList_/sigmaFromToList_/IVList_/BetaList_...)
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

//! # `ZoneNuclearData` — one material zone's full cross-section set
//!
//! Ports the per-`cellZone` data lists of GeN-Foam's `XS` class: for a single
//! material region it holds one [`NuclearDataOneEnergy`] interpolator per energy
//! group for each variable quantity (`D`, `nu Sigma_f`, `kappa Sigma_f`,
//! `Sigma_r`, `chi_p`, `chi_d`), the full per-Legendre-moment scattering matrix
//! of interpolators, and the feedback-independent constants (`1/v`, `beta`,
//! `lambda`, discontinuity factors, fuel/secondary-power fractions).
//!
//! Everything here is bare `f64` in base-SI units; the `uom` wrapping happens in
//! [`super::CrossSectionData`]. Zones are assembled from the reference state,
//! fed one data point per reactor state, then [`ZoneNuclearData::build`] solves
//! every interpolator.

use crate::genfoam::neutronics::xs::input::ZoneStateInput;
use crate::genfoam::neutronics::xs::nuclear_data_one_energy::NuclearDataOneEnergy;
use crate::genfoam::neutronics::xs::XsError;

/// A single-material zone's group-wise cross sections and precursor constants.
#[derive(Debug, Clone)]
pub struct ZoneNuclearData {
    name: String,
    energy_groups: usize,
    prec_groups: usize,
    legendre_moments: usize,

    // Variable (feedback-interpolated) group data, indexed `[energy_group]`.
    d: Vec<NuclearDataOneEnergy>,
    nu_sigma_eff: Vec<NuclearDataOneEnergy>,
    sigma_pow: Vec<NuclearDataOneEnergy>,
    sigma_removal: Vec<NuclearDataOneEnergy>,
    chi_prompt: Vec<NuclearDataOneEnergy>,
    chi_delayed: Vec<NuclearDataOneEnergy>,
    // Scattering interpolators, indexed `[moment][energy_j][energy_i]`.
    scattering: Vec<Vec<Vec<NuclearDataOneEnergy>>>,

    // Feedback-independent constants.
    fuel_fraction: f64,
    secondary_power_volume_fraction: f64,
    fraction_to_secondary_power: f64,
    df_adjust: bool,
    iv: Vec<f64>,
    disc_factor: Vec<f64>,
    integral_flux: Vec<f64>,
    beta: Vec<f64>,
    lambda: Vec<f64>,
    beta_tot: f64,
}

impl ZoneNuclearData {
    /// Build a zone's empty interpolator grid and its constants from the
    /// reference-state zone entry.
    ///
    /// The zone's `constants` block is mandatory here (it carries `IV`, `Beta`,
    /// `lambda`, ...). Interpolators are created empty; call
    /// [`Self::add_state_data`] once per reactor state (reference first), then
    /// [`Self::build`].
    ///
    /// # Errors
    ///
    /// [`XsError::MissingZoneConstants`] if the reference zone has no constants,
    /// or [`XsError::LengthMismatch`] if any constant array has the wrong length.
    pub fn new_reference(
        zone: &ZoneStateInput,
        energy_groups: usize,
        prec_groups: usize,
        legendre_moments: usize,
        poly_spline_mode: usize,
        n_parameters: usize,
    ) -> Result<Self, XsError> {
        let constants = zone
            .constants
            .as_ref()
            .ok_or_else(|| XsError::MissingZoneConstants {
                zone: zone.name.clone(),
            })?;

        check_len("IV", &zone.name, constants.iv.len(), energy_groups)?;
        check_len(
            "discFactor",
            &zone.name,
            constants.disc_factor.len(),
            energy_groups,
        )?;
        check_len(
            "integralFlux",
            &zone.name,
            constants.integral_flux.len(),
            energy_groups,
        )?;
        check_len("Beta", &zone.name, constants.beta.len(), prec_groups)?;
        check_len("lambda", &zone.name, constants.lambda.len(), prec_groups)?;

        let make_group = || {
            (0..energy_groups)
                .map(|_| NuclearDataOneEnergy::new(poly_spline_mode, n_parameters))
                .collect::<Vec<_>>()
        };

        let scattering = (0..legendre_moments)
            .map(|_| (0..energy_groups).map(|_| make_group()).collect::<Vec<_>>())
            .collect::<Vec<_>>();

        Ok(Self {
            name: zone.name.clone(),
            energy_groups,
            prec_groups,
            legendre_moments,
            d: make_group(),
            nu_sigma_eff: make_group(),
            sigma_pow: make_group(),
            sigma_removal: make_group(),
            chi_prompt: make_group(),
            chi_delayed: make_group(),
            scattering,
            fuel_fraction: constants.fuel_fraction,
            secondary_power_volume_fraction: constants.secondary_power_volume_fraction,
            fraction_to_secondary_power: constants.fraction_to_secondary_power,
            df_adjust: constants.df_adjust,
            iv: constants.iv.clone(),
            disc_factor: constants.disc_factor.clone(),
            integral_flux: constants.integral_flux.clone(),
            beta: constants.beta.clone(),
            lambda: constants.lambda.clone(),
            beta_tot: constants.beta.iter().sum(),
        })
    }

    /// Register one reactor state's variable cross sections as a data point,
    /// tagged with the (already transformed) feedback coordinates `parameters`.
    ///
    /// # Errors
    ///
    /// [`XsError::LengthMismatch`] if any of the zone's group arrays or its
    /// scattering matrices have the wrong dimensions.
    pub fn add_state_data(
        &mut self,
        zone: &ZoneStateInput,
        parameters: &[f64],
    ) -> Result<(), XsError> {
        let g = self.energy_groups;
        check_len("D", &self.name, zone.d.len(), g)?;
        check_len("nuSigmaEff", &self.name, zone.nu_sigma_eff.len(), g)?;
        check_len("sigmaPow", &self.name, zone.sigma_pow.len(), g)?;
        check_len("sigmaRemoval", &self.name, zone.sigma_removal.len(), g)?;
        check_len("chiPrompt", &self.name, zone.chi_prompt.len(), g)?;
        check_len("chiDelayed", &self.name, zone.chi_delayed.len(), g)?;
        check_len(
            "scattering (moments)",
            &self.name,
            zone.scattering.len(),
            self.legendre_moments,
        )?;

        for energy_i in 0..g {
            self.d[energy_i].add_data(zone.d[energy_i], parameters);
            self.nu_sigma_eff[energy_i].add_data(zone.nu_sigma_eff[energy_i], parameters);
            self.sigma_pow[energy_i].add_data(zone.sigma_pow[energy_i], parameters);
            self.sigma_removal[energy_i].add_data(zone.sigma_removal[energy_i], parameters);
            self.chi_prompt[energy_i].add_data(zone.chi_prompt[energy_i], parameters);
            self.chi_delayed[energy_i].add_data(zone.chi_delayed[energy_i], parameters);
        }

        for (moment_i, matrix) in zone.scattering.iter().enumerate() {
            check_len("scatteringMatrix rows", &self.name, matrix.len(), g)?;
            for (energy_j, row) in matrix.iter().enumerate() {
                check_len("scatteringMatrix cols", &self.name, row.len(), g)?;
                for (energy_i, &value) in row.iter().enumerate() {
                    self.scattering[moment_i][energy_j][energy_i].add_data(value, parameters);
                }
            }
        }
        Ok(())
    }

    /// Solve every interpolator in this zone.
    ///
    /// # Errors
    ///
    /// Propagates [`XsError::SingularRbfMatrix`] from any interpolator whose
    /// saddle-point system is singular.
    pub fn build(&mut self) -> Result<(), XsError> {
        for energy_i in 0..self.energy_groups {
            self.d[energy_i].build()?;
            self.nu_sigma_eff[energy_i].build()?;
            self.sigma_pow[energy_i].build()?;
            self.sigma_removal[energy_i].build()?;
            self.chi_prompt[energy_i].build()?;
            self.chi_delayed[energy_i].build()?;
        }
        for moment in &mut self.scattering {
            for row in moment.iter_mut() {
                for cell in row.iter_mut() {
                    cell.build()?;
                }
            }
        }
        Ok(())
    }

    // ---- metadata -----------------------------------------------------------

    /// The zone's cell-zone name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Number of energy groups.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.energy_groups
    }
    /// Number of precursor groups.
    #[must_use]
    pub fn prec_groups(&self) -> usize {
        self.prec_groups
    }
    /// Number of stored Legendre scattering moments.
    #[must_use]
    pub fn legendre_moments(&self) -> usize {
        self.legendre_moments
    }
    /// Fuel volume fraction `alpha_fuel`.
    #[must_use]
    pub fn fuel_fraction(&self) -> f64 {
        self.fuel_fraction
    }
    /// Volume fraction of secondary power-producing structure.
    #[must_use]
    pub fn secondary_power_volume_fraction(&self) -> f64 {
        self.secondary_power_volume_fraction
    }
    /// Fraction of total power going to the secondary structure.
    #[must_use]
    pub fn fraction_to_secondary_power(&self) -> f64 {
        self.fraction_to_secondary_power
    }
    /// Whether this zone's discontinuity factors are adjusted at runtime.
    #[must_use]
    pub fn df_adjust(&self) -> bool {
        self.df_adjust
    }

    // ---- raw (f64, base-SI) accessors --------------------------------------

    /// Interpolated diffusion coefficient `D_g` (m) at `parameters`.
    #[must_use]
    pub fn diffusion_coefficient(
        &self,
        group: usize,
        parameters: &[f64],
        parametrize: bool,
    ) -> f64 {
        self.d[group].get(parameters, parametrize)
    }
    /// Interpolated `nu Sigma_{f,g}` (m^-1) at `parameters`.
    #[must_use]
    pub fn nu_sigma_f(&self, group: usize, parameters: &[f64], parametrize: bool) -> f64 {
        self.nu_sigma_eff[group].get(parameters, parametrize)
    }
    /// Interpolated `kappa Sigma_{f,g}` (J/m) at `parameters`.
    #[must_use]
    pub fn sigma_pow(&self, group: usize, parameters: &[f64], parametrize: bool) -> f64 {
        self.sigma_pow[group].get(parameters, parametrize)
    }
    /// Interpolated removal cross section `Sigma_{r,g}` (m^-1) at `parameters`.
    #[must_use]
    pub fn sigma_removal(&self, group: usize, parameters: &[f64], parametrize: bool) -> f64 {
        self.sigma_removal[group].get(parameters, parametrize)
    }
    /// Interpolated prompt spectrum `chi_{p,g}` at `parameters`.
    #[must_use]
    pub fn chi_prompt(&self, group: usize, parameters: &[f64], parametrize: bool) -> f64 {
        self.chi_prompt[group].get(parameters, parametrize)
    }
    /// Interpolated delayed spectrum `chi_{d,g}` at `parameters`.
    #[must_use]
    pub fn chi_delayed(&self, group: usize, parameters: &[f64], parametrize: bool) -> f64 {
        self.chi_delayed[group].get(parameters, parametrize)
    }
    /// Interpolated scattering `Sigma_{s, j->i}` (m^-1) for Legendre `moment`.
    #[must_use]
    pub fn scattering(
        &self,
        moment: usize,
        energy_j: usize,
        energy_i: usize,
        parameters: &[f64],
        parametrize: bool,
    ) -> f64 {
        self.scattering[moment][energy_j][energy_i].get(parameters, parametrize)
    }

    /// Reference (feedback-free) value of a group quantity via a selector.
    #[must_use]
    pub fn diffusion_coefficient_ref(&self, group: usize) -> f64 {
        self.d[group].get_ref()
    }

    /// Constant inverse velocity `1/v_g` (s/m).
    #[must_use]
    pub fn inverse_velocity(&self, group: usize) -> f64 {
        self.iv[group]
    }
    /// Constant discontinuity factor `gamma_g`.
    #[must_use]
    pub fn disc_factor(&self, group: usize) -> f64 {
        self.disc_factor[group]
    }
    /// Constant integral flux `Phi_g`.
    #[must_use]
    pub fn integral_flux(&self, group: usize) -> f64 {
        self.integral_flux[group]
    }
    /// Constant delayed-neutron fraction `beta_k`.
    #[must_use]
    pub fn beta(&self, prec: usize) -> f64 {
        self.beta[prec]
    }
    /// Total delayed-neutron fraction `beta_tot`.
    #[must_use]
    pub fn beta_tot(&self) -> f64 {
        self.beta_tot
    }
    /// Constant precursor decay constant `lambda_k` (s^-1).
    #[must_use]
    pub fn lambda(&self, prec: usize) -> f64 {
        self.lambda[prec]
    }
}

/// Return [`XsError::LengthMismatch`] unless `found == expected`.
fn check_len(
    field: &'static str,
    zone: &str,
    found: usize,
    expected: usize,
) -> Result<(), XsError> {
    if found == expected {
        Ok(())
    } else {
        Err(XsError::LengthMismatch {
            field,
            zone: zone.to_string(),
            found,
            expected,
        })
    }
}
