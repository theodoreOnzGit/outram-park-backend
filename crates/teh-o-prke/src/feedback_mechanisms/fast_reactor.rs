// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da (read 2026-11-10)
//   Upstream source: src/classes/neutronics/pointKinetics/
//                    include/correctReactivity.H (the feedback sum),
//                    pointKineticNeutronics.C (calcDrivelineReactivity,
//                    the coefficient names), pointKineticNeutronics.H
//                    (the term definitions)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Stefan Radman, Thomas Guilbaud (EPFL)
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

//! # Fast-reactor reactivity feedback (gh:#1326, epic #1312)
//!
//! The terms a sodium-cooled, metal-fuelled fast core needs beside the
//! RELAP5 separable model ([`super::separable`]): fast-spectrum Doppler,
//! axial fuel expansion, cladding (in-assembly structure) expansion, coolant
//! temperature, coolant density (sodium void), radial core expansion (grid
//! plate, above-core load pad) and control-rod driveline expansion.
//!
//! ## Upstream: GeN-Foam's point-kinetics feedback sum
//!
//! `correctReactivity.H` (commit 652b3da) sums, each term against a
//! reference value taken at the start of the transient:
//!
//! ```text
//! rho = -K_D ln(T_fuel / T_fuel,ref)            fast-spectrum Doppler
//!     + a_fuel   (T_fuel   - T_fuel,ref)        axial fuel expansion
//!     + a_clad   (T_clad   - T_clad,ref)        cladding / in-assembly structure
//!     + a_cool   (T_cool   - T_cool,ref)        coolant temperature
//!     + a_rho    (rho_cool - rho_cool,ref)      coolant density (void)
//!     + a_struct (T_struct - T_struct,ref)      core radial expansion
//!     + rho_driveline( c_dl (T_dl - T_dl,ref) ) driveline expansion, from a map
//! ```
//!
//! (`pointKineticNeutronics.H` documents `a_fuel` as "fuel axial expansion",
//! `a_struct` as "core radial expansion", and the driveline coefficient
//! `c_dl` as an *absolute* expansion coefficient in m/K.) GeN-Foam's
//! structure-mechanics term (`TStructMech`, a deformed-mesh temperature) is
//! not here: it is the same linear form as the radial term and needs a
//! mechanics mesh this crate does not have; use a radial-structure term.
//!
//! **Generalisation.** Upstream has one structure temperature. A fast core's
//! radial expansion is driven by more than one structure (the grid plate
//! at the core-inlet temperature, the above-core load pad at the outlet
//! temperature), so [`FastReactorFeedback::radial_expansion`] holds any
//! number of linear structure terms, each with its own temperature. With
//! one entry it is upstream's term.
//!
//! **Sign convention: upstream's, the same as [`super::separable`]**:
//! `rho = a * deviation`, so a *negative* coefficient is stabilising for a
//! temperature rise. (The appbuilder's mesh port,
//! `outram_foam_appbuilder_lib::genfoam::multi_region::reactivity_feedback`,
//! negates its coefficients; this module does not.) The one exception is
//! upstream's own: the Doppler constant enters as `-K_D ln(T/T_ref)`, so a
//! *positive* `K_D` is stabilising. `K_D` is the magnitude of the textbook
//! Doppler constant `T d(rho)/dT`.
//!
//! **Driveline thermal lag: none, as upstream.** GeN-Foam evaluates the
//! driveline temperature as the flux-squared-weighted structure temperature
//! in the driveline zones (`computeFeedbackFieldValues.H`); there is no
//! first-order lag in the point-kinetics feedback. Any lag belongs to the
//! thermal-hydraulic model of the driveline (a heat structure), which
//! supplies [`FastReactorState::driveline_temperature`].
//!
//! **Driveline map, as upstream: zero outside it.**
//! `calcDrivelineReactivity` interpolates linearly between map points
//! (either order of the expansion axis) and returns **zero** for an
//! expansion outside the map, and also exactly at the first point of an
//! ascending map or the last point of a descending one (its half-open
//! intervals). This port keeps that behaviour so that it agrees with
//! upstream; make the map cover the expansion range the transient reaches
//! ([`RodWorthMap::covers`] checks a value).
//!
//! **Not double counting.** This module has its own Doppler term; a model
//! that also gives the separable model a fuel-temperature table or linear
//! term must not give the same physics to both.
//!
//! **Every coefficient is the caller's input** (uom quantities); there are
//! no built-in values and no defaults other than "term absent".
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use petir::mathf::RealMath;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::ConstZero;

use super::separable::SeparableReactivity;
use crate::teh_o_prke_error::TehOPrkeError;

/// Reactivity per unit coolant density, `d(rho)/d(rho_cool)` in m³/kg (the
/// unit of upstream's `feedbackCoeffRhoCool`): a [`SpecificVolume`].
pub type CoolantDensityCoefficient = SpecificVolume;

/// OpenFOAM's `SMALL` for double precision (`doubleScalar.H`,
/// `doubleScalarSmall = 1.0e-15`), the floor upstream puts under both
/// temperatures in the Doppler logarithm.
const OPENFOAM_SMALL: f64 = 1.0e-15;

/// **A linear temperature term**, `a (T - T_ref)`: axial fuel expansion,
/// cladding, coolant temperature or one radial-expansion structure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearTemperatureTerm {
    /// `a`, reactivity per kelvin (negative is stabilising).
    pub coefficient: TemperatureCoefficient,
    /// `T_ref`, the temperature at which the term is zero (the start of the
    /// transient).
    pub reference_temperature: ThermodynamicTemperature,
}

impl LinearTemperatureTerm {
    /// `a (T - T_ref)`, in the arithmetic of the separable model's linear
    /// terms (`a` in 1/K times the kelvin difference), so the two agree bit
    /// for bit.
    pub fn reactivity(&self, temperature: ThermodynamicTemperature) -> Ratio {
        let delta_t_k = temperature.get::<kelvin>() - self.reference_temperature.get::<kelvin>();
        Ratio::new::<ratio>(self.coefficient.get::<per_kelvin>() * delta_t_k)
    }
}

/// **Fast-spectrum Doppler**, `-K_D ln(T / T_ref)` (upstream
/// `correctReactivity.H`, `fastNeutrons` branch), each temperature floored
/// at OpenFOAM's `SMALL` (1e-15 K) as upstream does.
///
/// Its slope at `T_ref` is `-K_D / T_ref`, so for small excursions it is the
/// linear term with `a = -K_D / T_ref`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FastDopplerTerm {
    /// `K_D`, the Doppler constant's magnitude (positive is stabilising).
    pub doppler_constant: Ratio,
    /// `T_ref`, the fuel temperature at which the term is zero.
    pub reference_temperature: ThermodynamicTemperature,
}

impl FastDopplerTerm {
    /// `-K_D ln(max(T, SMALL) / max(T_ref, SMALL))`.
    pub fn reactivity(&self, fuel_temperature: ThermodynamicTemperature) -> Ratio {
        let t = fuel_temperature.get::<kelvin>().max(OPENFOAM_SMALL);
        let t_ref = self
            .reference_temperature
            .get::<kelvin>()
            .max(OPENFOAM_SMALL);
        Ratio::new::<ratio>(-self.doppler_constant.get::<ratio>() * (t / t_ref).r_ln())
    }
}

/// **Coolant density (sodium void)**, `a_rho (rho - rho_ref)` (upstream
/// `rhoCoolReactivity`). A core with a positive void worth has a *negative*
/// `a_rho`: losing density adds reactivity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoolantDensityTerm {
    /// `a_rho`, reactivity per kg/m³.
    pub coefficient: CoolantDensityCoefficient,
    /// `rho_ref`, the coolant density at which the term is zero.
    pub reference_density: MassDensity,
}

impl CoolantDensityTerm {
    /// `a_rho (rho - rho_ref)`.
    pub fn reactivity(&self, density: MassDensity) -> Ratio {
        let delta = density.get::<kilogram_per_cubic_meter>()
            - self.reference_density.get::<kilogram_per_cubic_meter>();
        Ratio::new::<ratio>(self.coefficient.get::<cubic_meter_per_kilogram>() * delta)
    }
}

/// **Control-rod reactivity against driveline expansion** (upstream
/// `controlRodReactivityMap`: pairs of expansion in metres and reactivity).
///
/// The sign of the expansion axis is the caller's: upstream's tutorial map
/// gives negative reactivity for positive expansion (an expanding,
/// top-supported driveline pushes the rods in).
#[derive(Clone, Debug, PartialEq)]
pub struct RodWorthMap {
    /// Expansion \[m\], strictly increasing or strictly decreasing.
    expansion_m: Vec<f64>,
    /// Reactivity at each expansion.
    reactivity: Vec<f64>,
}

impl RodWorthMap {
    /// A map of at least two points, of equal count, finite, with the
    /// expansions strictly monotonic (either order, as upstream allows).
    /// Upstream takes any list and returns zero for fewer than two points;
    /// here such a map is refused instead, since it can only ever give zero.
    pub fn new(expansions: Vec<Length>, reactivities: Vec<Ratio>) -> Result<Self, TehOPrkeError> {
        let x: Vec<f64> = expansions.iter().map(|l| l.get::<meter>()).collect();
        let y: Vec<f64> = reactivities.iter().map(|r| r.get::<ratio>()).collect();
        if x.len() != y.len() || x.len() < 2 {
            return Err(TehOPrkeError::GenericStringError(format!(
                "rod worth map needs at least two (expansion, reactivity) points of equal count; got {} and {}",
                x.len(),
                y.len()
            )));
        }
        if x.iter().chain(y.iter()).any(|v| !v.is_finite()) {
            return Err(TehOPrkeError::GenericStringError(
                "rod worth map values must be finite".to_string(),
            ));
        }
        let increasing = x.windows(2).all(|w| w[1] > w[0]);
        let decreasing = x.windows(2).all(|w| w[1] < w[0]);
        if !(increasing || decreasing) {
            return Err(TehOPrkeError::GenericStringError(
                "rod worth map expansions must be strictly increasing or strictly decreasing"
                    .to_string(),
            ));
        }
        Ok(Self {
            expansion_m: x,
            reactivity: y,
        })
    }

    /// Whether `expansion` falls in an interval of the map, i.e. whether
    /// [`Self::reactivity`] interpolates rather than returning upstream's
    /// zero.
    pub fn covers(&self, expansion: Length) -> bool {
        self.interval(expansion.get::<meter>()).is_some()
    }

    /// The interval upstream's loop would pick, if any.
    fn interval(&self, x: f64) -> Option<usize> {
        let n = self.expansion_m.len();
        let descending = self.expansion_m[0] > self.expansion_m[n - 1];
        (0..n - 1).find(|&i| {
            let (x0, x1) = (self.expansion_m[i], self.expansion_m[i + 1]);
            if descending {
                x <= x0 && x > x1
            } else {
                x > x0 && x <= x1
            }
        })
    }

    /// Reactivity at driveline expansion `expansion`: upstream's
    /// `calcDrivelineReactivity`, `m (x - x0) + y0` with
    /// `m = (y1 - y0) / (x1 - x0)` on the first interval that holds `x`
    /// (half-open, `x0` excluded on an ascending map, `x1` excluded on a
    /// descending one), and **zero** when no interval holds it.
    pub fn reactivity(&self, expansion: Length) -> Ratio {
        let x = expansion.get::<meter>();
        let r = match self.interval(x) {
            Some(i) => {
                let (x0, x1) = (self.expansion_m[i], self.expansion_m[i + 1]);
                let (y0, y1) = (self.reactivity[i], self.reactivity[i + 1]);
                let m = (y1 - y0) / (x1 - x0);
                m * (x - x0) + y0
            }
            None => 0.0,
        };
        Ratio::new::<ratio>(r)
    }
}

/// **Control-rod driveline expansion**: the driveline (the part of it whose
/// temperature changes, e.g. its sodium-washed length) grows by
/// `dL = L alpha (T - T_ref)`, and the rods move with it; the reactivity is
/// the rod worth at that displacement ([`RodWorthMap`]).
///
/// Upstream takes the product `c_dl = L alpha` directly
/// (`absoluteDrivelineExpansionCoeff`, m/K); here the two factors are given
/// separately so each can be cited, and multiplied first, as upstream's
/// coefficient would be.
#[derive(Clone, Debug, PartialEq)]
pub struct DrivelineExpansionTerm {
    /// `L`, the length of driveline whose temperature is `T`.
    pub length: Length,
    /// `alpha`, the driveline material's linear thermal expansion
    /// coefficient.
    pub linear_expansion_coefficient: TemperatureCoefficient,
    /// `T_ref`, the driveline temperature at which the expansion is zero.
    pub reference_temperature: ThermodynamicTemperature,
    /// Rod worth against expansion.
    pub worth: RodWorthMap,
}

impl DrivelineExpansionTerm {
    /// `dL = (L alpha) (T - T_ref)`.
    pub fn expansion(&self, temperature: ThermodynamicTemperature) -> Length {
        let absolute_coefficient_m_per_k =
            self.length.get::<meter>() * self.linear_expansion_coefficient.get::<per_kelvin>();
        let delta_t_k = temperature.get::<kelvin>() - self.reference_temperature.get::<kelvin>();
        Length::new::<meter>(absolute_coefficient_m_per_k * delta_t_k)
    }

    /// The rod worth at [`Self::expansion`].
    pub fn reactivity(&self, temperature: ThermodynamicTemperature) -> Ratio {
        self.worth.reactivity(self.expansion(temperature))
    }
}

/// **The fast-reactor feedback set.** A `None` term (or an empty
/// `radial_expansion`) contributes zero; that is "not supplied", not a
/// default value of the physics.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FastReactorFeedback {
    /// Fast-spectrum Doppler, on the fuel temperature.
    pub doppler: Option<FastDopplerTerm>,
    /// Axial fuel expansion, on the fuel temperature.
    pub axial_fuel_expansion: Option<LinearTemperatureTerm>,
    /// Cladding / in-assembly structure (clad and wire) expansion, on the
    /// cladding temperature.
    pub cladding: Option<LinearTemperatureTerm>,
    /// Coolant temperature (density effects excluded), on the coolant
    /// temperature.
    pub coolant_temperature: Option<LinearTemperatureTerm>,
    /// Coolant density (sodium void).
    pub coolant_density: Option<CoolantDensityTerm>,
    /// Radial core expansion: one term per structure (grid plate, load
    /// pad, ...), each on its own temperature
    /// ([`FastReactorState::radial_structure_temperatures`], same order).
    pub radial_expansion: Vec<LinearTemperatureTerm>,
    /// Control-rod driveline expansion.
    pub driveline: Option<DrivelineExpansionTerm>,
}

/// The temperatures and density the feedback set is evaluated at.
#[derive(Clone, Debug, PartialEq)]
pub struct FastReactorState {
    /// Fuel temperature (Doppler and axial expansion).
    pub fuel_temperature: ThermodynamicTemperature,
    /// Cladding temperature.
    pub cladding_temperature: ThermodynamicTemperature,
    /// Coolant temperature.
    pub coolant_temperature: ThermodynamicTemperature,
    /// Coolant density.
    pub coolant_density: MassDensity,
    /// One temperature per [`FastReactorFeedback::radial_expansion`] term.
    pub radial_structure_temperatures: Vec<ThermodynamicTemperature>,
    /// Driveline temperature.
    pub driveline_temperature: ThermodynamicTemperature,
}

/// The feedback set broken into its terms, so each can be checked or
/// plotted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FastReactorReactivity {
    /// `-K_D ln(T_fuel / T_ref)`
    pub doppler: Ratio,
    /// `a_fuel dT_fuel`
    pub axial_fuel_expansion: Ratio,
    /// `a_clad dT_clad`
    pub cladding: Ratio,
    /// `a_cool dT_cool`
    pub coolant_temperature: Ratio,
    /// `a_rho d(rho_cool)`
    pub coolant_density: Ratio,
    /// `sum_s a_s dT_s`
    pub radial_expansion: Ratio,
    /// rod worth at the driveline expansion
    pub driveline: Ratio,
}

impl FastReactorReactivity {
    /// The sum, in upstream's order (`correctReactivity.H`: Doppler, fuel,
    /// clad, coolant temperature, coolant density, structure, driveline).
    pub fn total(&self) -> Ratio {
        self.doppler
            + self.axial_fuel_expansion
            + self.cladding
            + self.coolant_temperature
            + self.coolant_density
            + self.radial_expansion
            + self.driveline
    }

    /// The total in dollars, `rho / beta`.
    pub fn total_in_dollars(&self, beta: Ratio) -> Ratio {
        self.total() / beta.get::<ratio>()
    }

    /// The separable model's total (scram, moderator, fuel tables and
    /// linear terms) plus this set's: the two are independent sums of
    /// independent effects, so they compose by addition.
    pub fn total_with_separable(&self, separable: &SeparableReactivity) -> Ratio {
        separable.total() + self.total()
    }
}

impl FastReactorFeedback {
    /// Evaluate every term at `state`. Errors when
    /// `state.radial_structure_temperatures` does not have one entry per
    /// [`Self::radial_expansion`] term.
    pub fn evaluate(
        &self,
        state: &FastReactorState,
    ) -> Result<FastReactorReactivity, TehOPrkeError> {
        if state.radial_structure_temperatures.len() != self.radial_expansion.len() {
            return Err(TehOPrkeError::ShapeMismatch(format!(
                "fast-reactor feedback: {} radial-expansion structures but {} structure temperatures",
                self.radial_expansion.len(),
                state.radial_structure_temperatures.len()
            )));
        }
        let linear = |term: &Option<LinearTemperatureTerm>, t: ThermodynamicTemperature| {
            term.map_or(Ratio::ZERO, |term| term.reactivity(t))
        };
        let radial_expansion = self
            .radial_expansion
            .iter()
            .zip(&state.radial_structure_temperatures)
            .fold(Ratio::ZERO, |acc, (term, t)| acc + term.reactivity(*t));
        Ok(FastReactorReactivity {
            doppler: self
                .doppler
                .map_or(Ratio::ZERO, |d| d.reactivity(state.fuel_temperature)),
            axial_fuel_expansion: linear(&self.axial_fuel_expansion, state.fuel_temperature),
            cladding: linear(&self.cladding, state.cladding_temperature),
            coolant_temperature: linear(&self.coolant_temperature, state.coolant_temperature),
            coolant_density: self
                .coolant_density
                .map_or(Ratio::ZERO, |c| c.reactivity(state.coolant_density)),
            radial_expansion,
            driveline: self
                .driveline
                .as_ref()
                .map_or(Ratio::ZERO, |d| d.reactivity(state.driveline_temperature)),
        })
    }
}

#[cfg(test)]
mod tests;
