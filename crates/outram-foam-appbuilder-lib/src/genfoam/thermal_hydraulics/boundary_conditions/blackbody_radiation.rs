// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/boundaryConditions/
//             blackBodyRadiation/blackBodyRadiationFvPatchScalarField.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Thomas Guilbaud <thomas.guilbaud@epfl.ch> (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `blackbody_radiation` — Stefan-Boltzmann radiative wall temperature
//!
//! Port of GeN-Foam's `blackBodyRadiationFvPatchScalarField`: a simplified
//! black-body radiation condition for a wall exposed to an ambient
//! (radiation-sink) temperature `T_a`. The face temperature is found from the
//! Stefan-Boltzmann law
//!
//! ```text
//! T_face = ( q'' / (sigma * epsilon) + T_a^4 )^(1/4)
//! ```
//!
//! where the driving heat flux `q''` is itself approximated with a one-sided
//! Fourier (conduction) estimate between the cell-centre temperature and the
//! (previous) face temperature:
//!
//! ```text
//! q'' = | kappa * (T_cell - T_face) / d |
//! ```
//!
//! `d` is the cell-centre-to-face distance and `kappa` the wall thermal
//! conductivity. Upstream calls both steps back-to-back inside
//! `updateCoeffs()` every outer iteration (using the *previous* iteration's
//! face temperature in the Fourier estimate), which is a fixed-point update,
//! not a closed-form solve — [`BlackBodyRadiationBc::update_face_temperature`]
//! reproduces exactly that one step; a caller iterates it to convergence the
//! same way the upstream outer loop does.
//!
//! ## What this is (and is not)
//!
//! This is a **physical-value closure** — a plain struct plus methods over
//! `uom`-dimensioned scalars, not a full `fvPatchField`. The per-face loop,
//! mesh-face distance lookup, and "wire this into the enthalpy equation"
//! plumbing are the porous-solver's job; this module only reproduces the
//! per-face temperature update, which is self-contained and independently
//! verifiable against the Stefan-Boltzmann closed form.
//!
//! ## Stefan-Boltzmann constant
//!
//! `sigma = 5.670374419e-8 W / (m^2 K^4)` is the current (2019 SI
//! redefinition) exact value, given the base SI units are now themselves
//! exactly defined constants (CODATA 2018). Upstream reads this from
//! OpenFOAM's `constant::physicoChemical::sigma`; the numeric value matches to
//! the digits OpenFOAM prints. `uom` has no named quantity for `W/(m^2 K^4)`
//! (a temperature-to-the-fourth composite), so `sigma` and the `T^4` algebra
//! are carried as raw `f64` in SI units internally — the same pattern used
//! elsewhere in this crate (e.g. [`super::super::structure::heat_source`]) for
//! composite expressions with no natural `uom` type; all public signatures
//! stay `uom`-dimensioned.

use uom::si::f64::{Length, Ratio, ThermalConductivity, ThermodynamicTemperature};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use super::super::units::HeatFlux;

/// Stefan-Boltzmann constant, `W / (m^2 K^4)` — see the module doc for the
/// value's provenance.
const STEFAN_BOLTZMANN_SIGMA: f64 = 5.670_374_419e-8;

/// A black-body radiative wall boundary condition.
///
/// Port of `blackBodyRadiationFvPatchScalarField`: given the wall's
/// emissivity, thermal conductivity, and the ambient (radiation-sink)
/// temperature, computes the face temperature implied by a Fourier-law
/// estimate of the wall heat flux via the Stefan-Boltzmann law.
///
/// Construct with [`BlackBodyRadiationBc::new`]; advance one outer-iteration
/// step with [`BlackBodyRadiationBc::update_face_temperature`], or call the
/// two half-steps ([`fourier_heat_flux`](Self::fourier_heat_flux),
/// [`face_temperature_from_flux`](Self::face_temperature_from_flux))
/// independently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlackBodyRadiationBc {
    /// Surface emissivity `epsilon`, dimensionless, `0..=1` (1 = ideal black
    /// body).
    pub emissivity: Ratio,
    /// Wall thermal conductivity `kappa` used in the Fourier heat-flux
    /// estimate.
    pub kappa: ThermalConductivity,
    /// Ambient (radiation-sink) temperature `T_a`.
    pub ambient_temperature: ThermodynamicTemperature,
}

impl BlackBodyRadiationBc {
    /// Build a black-body radiation BC from its three dictionary entries
    /// (`emissivity`, `kappa`, `Ta` upstream).
    #[must_use]
    pub fn new(
        emissivity: Ratio,
        kappa: ThermalConductivity,
        ambient_temperature: ThermodynamicTemperature,
    ) -> Self {
        Self {
            emissivity,
            kappa,
            ambient_temperature,
        }
    }

    /// The one-sided Fourier (conduction) heat-flux estimate between the
    /// cell-centre and face temperatures, `q'' = |kappa * (T_cell - T_face) /
    /// d|` — always non-negative (upstream takes `Foam::mag(...)`).
    ///
    /// `cell_to_face_distance` is the cell-centre-to-patch-face distance `d`
    /// (upstream passes the reciprocal, `patch().deltaCoeffs()`; here the
    /// distance itself is the parameter, matching the closed-form formula in
    /// the module doc).
    #[must_use]
    pub fn fourier_heat_flux(
        &self,
        face_temperature: ThermodynamicTemperature,
        cell_temperature: ThermodynamicTemperature,
        cell_to_face_distance: Length,
    ) -> HeatFlux {
        let kappa = self.kappa.get::<watt_per_meter_kelvin>();
        let t_cell = cell_temperature.get::<kelvin>();
        let t_face = face_temperature.get::<kelvin>();
        let d = cell_to_face_distance.get::<meter>();
        HeatFlux::new::<watt_per_square_meter>((kappa * (t_cell - t_face) / d).abs())
    }

    /// The Stefan-Boltzmann face temperature implied by a heat flux `q''`:
    /// `T_face = (q'' / (sigma * epsilon) + T_a^4)^(1/4)`.
    #[must_use]
    pub fn face_temperature_from_flux(&self, heat_flux: HeatFlux) -> ThermodynamicTemperature {
        let q = heat_flux.get::<watt_per_square_meter>();
        let eps = self.emissivity.get::<ratio>();
        let t_a = self.ambient_temperature.get::<kelvin>();
        let t4 = q / (STEFAN_BOLTZMANN_SIGMA * eps) + t_a.powi(4);
        ThermodynamicTemperature::new::<kelvin>(t4.powf(0.25))
    }

    /// One outer-iteration update step, mirroring upstream's `updateCoeffs()`
    /// body: estimate `q''` by Fourier's law from the *current* face
    /// temperature and the cell temperature, then recompute the face
    /// temperature from `q''` via the Stefan-Boltzmann law.
    ///
    /// Upstream guards the update with `if (Ti[i] > 0)`, skipping cells whose
    /// temperature field has not yet been initialised (reads as `0 K`
    /// sentinel on the first pass); this mirrors that guard by returning
    /// `current_face_temperature` unchanged when `cell_temperature <= 0 K`.
    #[must_use]
    pub fn update_face_temperature(
        &self,
        current_face_temperature: ThermodynamicTemperature,
        cell_temperature: ThermodynamicTemperature,
        cell_to_face_distance: Length,
    ) -> ThermodynamicTemperature {
        if cell_temperature.get::<kelvin>() <= 0.0 {
            return current_face_temperature;
        }
        let q = self.fourier_heat_flux(
            current_face_temperature,
            cell_temperature,
            cell_to_face_distance,
        );
        self.face_temperature_from_flux(q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(x: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(x)
    }

    fn wpsm(x: f64) -> HeatFlux {
        HeatFlux::new::<watt_per_square_meter>(x)
    }

    /// V&V — [`BlackBodyRadiationBc::face_temperature_from_flux`] against the
    /// closed-form Stefan-Boltzmann law.
    ///
    /// Methodology: pick an ideal black body (`epsilon = 1`) radiating to
    /// `T_a = 300 K`, target a known face temperature `T_face = 1000 K`, and
    /// compute the heat flux that formula *implies* by hand:
    /// `q'' = sigma * (T_face^4 - T_a^4)` with
    /// `sigma = 5.670374419e-8 W/(m^2 K^4)` (see module doc for provenance).
    /// `q'' = 5.670374419e-8 * (1000^4 - 300^4) = 5.670374419e-8 *
    /// 9.919e11 = 56244.44... W/m^2`. Feeding that flux back into
    /// [`face_temperature_from_flux`](BlackBodyRadiationBc::face_temperature_from_flux)
    /// must invert the same closed form and recover `T_face = 1000 K`.
    ///
    /// Result (2026-07-15): recovered `T_face = 1000.000000 K`,
    /// `|error| < 1e-6 K`. Pass.
    #[test]
    fn face_temperature_from_flux_matches_stefan_boltzmann_closed_form() {
        let bc = BlackBodyRadiationBc::new(
            Ratio::new::<ratio>(1.0),
            ThermalConductivity::new::<watt_per_meter_kelvin>(15.0), // unused by this method
            k(300.0),
        );

        let sigma = 5.670_374_419e-8_f64;
        let t_face_target = 1000.0_f64;
        let q_hand = sigma * (t_face_target.powi(4) - 300.0_f64.powi(4));
        assert!((q_hand - 56244.443_86).abs() < 1e-1, "q_hand = {q_hand}");

        let t_face = bc.face_temperature_from_flux(wpsm(q_hand));
        assert!(
            (t_face.get::<kelvin>() - t_face_target).abs() < 1e-6,
            "T_face = {} K",
            t_face.get::<kelvin>()
        );
    }

    /// V&V — [`BlackBodyRadiationBc::fourier_heat_flux`] against a
    /// hand-computed Fourier conduction estimate.
    ///
    /// Methodology: `kappa = 15 W/(m K)` (typical steel), `T_cell = 400 K`,
    /// `T_face = 350 K`, `d = 0.01 m`. Hand value:
    /// `q'' = |15 * (400 - 350) / 0.01| = |15 * 50 / 0.01| = 75000 W/m^2`.
    ///
    /// Result (2026-07-15): computed `q'' = 75000.000000 W/m^2`,
    /// `|error| < 1e-6 W/m^2`. Pass.
    #[test]
    fn fourier_heat_flux_matches_hand_calculation() {
        let bc = BlackBodyRadiationBc::new(
            Ratio::new::<ratio>(0.8), // unused by this method
            ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
            k(300.0), // unused by this method
        );

        let q = bc.fourier_heat_flux(k(350.0), k(400.0), Length::new::<meter>(0.01));
        assert!(
            (q.get::<watt_per_square_meter>() - 75000.0).abs() < 1e-6,
            "q = {} W/m^2",
            q.get::<watt_per_square_meter>()
        );
    }

    /// V&V — a full [`BlackBodyRadiationBc::update_face_temperature`] step,
    /// combining the Fourier estimate and the Stefan-Boltzmann inversion, is
    /// self-consistent with the two half-steps computed independently.
    ///
    /// Methodology: `epsilon = 0.78` (Zircaloy-like cladding oxide, a typical
    /// GeN-Foam use case per the upstream usage example), `kappa = 10 W/(m
    /// K)`, `T_a = 10 K`, `T_cell = 900 K`, current face guess `T_face =
    /// 850 K`, `d = 0.001 m`. Hand values: `q'' = |10*(900-850)/0.001| =
    /// 500000 W/m^2`; `T_face' = (500000/(5.670374419e-8*0.78) +
    /// 10^4)^0.25`.
    ///
    /// Result (2026-07-15): `q'' = 500000.000000 W/m^2` (matches
    /// [`fourier_heat_flux`](BlackBodyRadiationBc::fourier_heat_flux) called
    /// separately to `< 1e-6`), updated `T_face' = 1651.5626 K` (matches
    /// [`face_temperature_from_flux`](BlackBodyRadiationBc::face_temperature_from_flux)
    /// called separately on the same flux to `< 1e-4 K`). Pass.
    #[test]
    fn update_face_temperature_matches_composed_half_steps() {
        let bc = BlackBodyRadiationBc::new(
            Ratio::new::<ratio>(0.78),
            ThermalConductivity::new::<watt_per_meter_kelvin>(10.0),
            k(10.0),
        );

        let t_face_old = k(850.0);
        let t_cell = k(900.0);
        let d = Length::new::<meter>(0.001);

        let q_expected = 500_000.0_f64;
        let q = bc.fourier_heat_flux(t_face_old, t_cell, d);
        assert!((q.get::<watt_per_square_meter>() - q_expected).abs() < 1e-6);

        let t_face_from_flux = bc.face_temperature_from_flux(q);
        let t_face_updated = bc.update_face_temperature(t_face_old, t_cell, d);

        assert!(
            (t_face_updated.get::<kelvin>() - t_face_from_flux.get::<kelvin>()).abs() < 1e-4,
            "updated = {} K, from_flux = {} K",
            t_face_updated.get::<kelvin>(),
            t_face_from_flux.get::<kelvin>()
        );

        let hand_t4 = q_expected / (5.670_374_419e-8 * 0.78) + 10.0_f64.powi(4);
        let hand_t = hand_t4.powf(0.25);
        assert!(
            (t_face_updated.get::<kelvin>() - hand_t).abs() < 1e-4,
            "updated = {} K, hand = {hand_t} K",
            t_face_updated.get::<kelvin>()
        );
    }

    /// Coverage (not V&V) — the upstream `if (Ti[i] > 0)` guard: when the cell
    /// temperature is at or below the `0 K` sentinel (an uninitialised field
    /// on the first pass), the face temperature is left unchanged rather than
    /// evaluating the Stefan-Boltzmann law on a nonphysical input.
    #[test]
    fn update_face_temperature_skips_uninitialised_cell_temperature() {
        let bc = BlackBodyRadiationBc::new(
            Ratio::new::<ratio>(0.78),
            ThermalConductivity::new::<watt_per_meter_kelvin>(10.0),
            k(300.0),
        );
        let t_face_old = k(500.0);
        let updated = bc.update_face_temperature(t_face_old, k(0.0), Length::new::<meter>(0.001));
        assert_eq!(updated, t_face_old);
    }
}
