// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/powerModels/powerModel.{C,H}
//             (the run-time selection table of the four radial-conduction models)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman, Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `RadialPowerModel` — enum dispatch over the four radial-conduction power models (gh:#1230)
//!
//! GeN-Foam selects a `powerModel` at run time by name. [`super::power_model::PowerModel`] is the
//! per-cell lumped family (`fixedPower`, `fixedTemperature`); these four own a per-cell radial state over a
//! whole mesh (as `nuclearSteadyStatePebble` does), so they get their own closed enum rather than a variant
//! of the per-cell one (a change of the `PowerModel` signatures would touch every caller of the lumped pair).
//! No `dyn`, no `Box` (the variants are held by value; they are large but few).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::heated_pin::HeatedPin;
use super::interpolated_nuclear_fuel_pin::InterpolatedNuclearFuelPin;
use super::lumped_nuclear_structure::LumpedNuclearStructure;
use super::nuclear_fuel_pin::NuclearFuelPin;
use super::radial_solve::RadialError;

/// The per-step inputs a structure hands to `powerModel::correct` besides `HT` and `H`.
#[derive(Debug, Clone, PartialEq)]
pub struct StepInputs {
    /// `mesh.time().deltaT()` [s].
    pub dt: f64,
    /// `mesh.time().timeIndex()`.
    pub time_index: i64,
    /// `structure.powerDensityNeutronics()` per cell [W/m^3].
    pub q_neutronics: Vec<f64>,
    /// `fvc::laplacian(Tmatrix)` per cell (used by `lumpedNuclearStructure` only).
    pub laplacian_t_matrix: Vec<f64>,
    /// Cell volumes `mesh.V()`.
    pub volumes: Vec<f64>,
}

/// A radial-conduction power model.
#[derive(Debug, Clone, PartialEq)]
pub enum RadialPowerModel {
    /// `heatedPin`.
    HeatedPin(HeatedPin),
    /// `nuclearFuelPin`.
    NuclearFuelPin(NuclearFuelPin),
    /// `lumpedNuclearStructure`.
    LumpedNuclearStructure(LumpedNuclearStructure),
    /// `interpolatedNuclearFuelPin`.
    InterpolatedNuclearFuelPin(InterpolatedNuclearFuelPin),
}

impl RadialPowerModel {
    /// The upstream type name (`TypeName(...)`).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::HeatedPin(_) => "heatedPin",
            Self::NuclearFuelPin(_) => "nuclearFuelPin",
            Self::LumpedNuclearStructure(_) => "lumpedNuclearStructure",
            Self::InterpolatedNuclearFuelPin(_) => "interpolatedNuclearFuelPin",
        }
    }

    /// `powerModel::correct(HT, H)`.
    ///
    /// # Errors
    /// A singular radial matrix.
    pub fn correct(&mut self, ht_sum: &[f64], h_sum: &[f64], step: &StepInputs) -> Result<(), RadialError> {
        match self {
            Self::HeatedPin(m) => m.correct(ht_sum, h_sum, &step.volumes, step.dt, step.time_index),
            Self::NuclearFuelPin(m) => {
                m.correct(ht_sum, h_sum, &step.q_neutronics, &step.volumes, step.dt, step.time_index)
            }
            Self::LumpedNuclearStructure(m) => {
                m.correct(ht_sum, h_sum, &step.q_neutronics, &step.laplacian_t_matrix, step.dt, step.time_index)
            }
            Self::InterpolatedNuclearFuelPin(m) => {
                m.correct(ht_sum, h_sum);
                Ok(())
            }
        }
    }

    /// `powerModel::correctT(T)` for one cell of the model: the structure surface temperature.
    #[must_use]
    pub fn surface_temperature(&self, cell: usize) -> f64 {
        match self {
            Self::HeatedPin(m) => m.surface_temperature(cell),
            Self::NuclearFuelPin(m) => m.surface_temperature(cell),
            Self::LumpedNuclearStructure(m) => m.surface_temperature(cell),
            Self::InterpolatedNuclearFuelPin(m) => m.surface_temperature(cell),
        }
    }

    /// The model's cells in `cellList_` order.
    #[must_use]
    pub fn cells(&self) -> &[usize] {
        match self {
            Self::HeatedPin(m) => m.cells(),
            Self::NuclearFuelPin(m) => m.cells(),
            Self::LumpedNuclearStructure(m) => m.cells(),
            Self::InterpolatedNuclearFuelPin(m) => m.cells(),
        }
    }

    /// `iA` of a cell of the model.
    #[must_use]
    pub fn interfacial_area(&self, cell: usize) -> f64 {
        match self {
            Self::HeatedPin(m) => m.interfacial_area(cell),
            Self::NuclearFuelPin(m) => m.interfacial_area(cell),
            Self::LumpedNuclearStructure(m) => m.interfacial_area(cell),
            Self::InterpolatedNuclearFuelPin(m) => m.interfacial_area(cell),
        }
    }

    /// `(TFuelAv, TCladAv)` the model writes into the structure for the neutronics (0, 0 for the two models
    /// that do not set them).
    #[must_use]
    pub fn coupling_temperatures(&self, cell: usize) -> (f64, f64) {
        match self {
            Self::HeatedPin(_) | Self::InterpolatedNuclearFuelPin(_) => (0.0, 0.0),
            Self::NuclearFuelPin(m) => m.averages(cell),
            Self::LumpedNuclearStructure(m) => m.coupling_temperatures(cell),
        }
    }
}

/// The structure's `heatFlux = H * Twall - HT` of `structure::correct` (a diagnostic, used by Shah pool boiling).
#[must_use]
pub fn structure_heat_flux(h: f64, t_wall: f64, ht: f64) -> f64 {
    h * t_wall - ht
}

#[cfg(test)]
mod tests {
    use super::super::interpolated_nuclear_fuel_pin::InterpolatedRegion;
    use super::*;

    /// The enum dispatches (an interpolated pin, the cheapest) and the heat-flux diagnostic is `H T - HT`.
    #[test]
    fn dispatch_reaches_the_model() {
        let reg = InterpolatedRegion { cells: vec![0], fuel_outer_radius: 0.005, t0: 700.0, volume_fraction: Some(0.4) };
        let mut m = RadialPowerModel::InterpolatedNuclearFuelPin(
            InterpolatedNuclearFuelPin::new(1, &[reg], &[0.4]).unwrap(),
        );
        let step = StepInputs { dt: 0.5, time_index: 1, q_neutronics: vec![0.0], laplacian_t_matrix: vec![0.0], volumes: vec![1.0] };
        m.correct(&[6.0e6], &[1.0e4], &step).unwrap();
        assert_eq!(m.type_name(), "interpolatedNuclearFuelPin");
        assert_eq!(m.surface_temperature(0), 600.0);
        assert_eq!(m.cells(), &[0]);
        assert!(m.interfacial_area(0) > 0.0);
        assert_eq!(m.coupling_temperatures(0), (0.0, 0.0));
        assert_eq!(structure_heat_flux(2.0, 3.0, 5.0), 1.0);
    }
}
