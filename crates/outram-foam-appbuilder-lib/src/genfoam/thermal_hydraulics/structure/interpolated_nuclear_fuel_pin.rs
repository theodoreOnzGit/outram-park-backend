// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerModels/interpolatedNuclearFuelPin/interpolatedNuclearFuelPin.{C,H} (and powerModel.{C,H})
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman, Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interpolatedNuclearFuelPin` — a pin whose heat flux is imposed from outside (gh:#1230)
//!
//! Translation of GeN-Foam's `powerModels::interpolatedNuclearFuelPin`, an algebraic model: the heat flux
//! `interpolatedHeatFlux` leaving the pin surface is a field some other solver maps in (no shipped tutorial
//! selects the class); the surface temperature follows from the convective balance,
//! `T = (q'' + HT)/H`, and the equivalent volumetric power is `q''' = q'' iA / alpha` with `iA = 2 alpha/r_fo`.
//!
//! **A deliberate difference.** Upstream evaluates `interpolatedHeatFlux*iA/alpha` over the WHOLE mesh, so a
//! cell outside every structure (`alpha = 0`) divides `0/0`; with the default `trapFpe 1` the real binary
//! aborts with SIGFPE there (found by the round-35 driver, which runs this case with `FOAM_SIGFPE=false`).
//! This port evaluates the model's own cells only, so the NaN of the unzoned cells never exists.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::radial_solve::RadialError;

/// One `interpolatedNuclearFuelPin` region.
#[derive(Debug, Clone, PartialEq)]
pub struct InterpolatedRegion {
    /// Cells of the zone in cell-zone order.
    pub cells: Vec<usize>,
    /// `fuelOuterRadius` [m].
    pub fuel_outer_radius: f64,
    /// `T`: the initial surface temperature [K].
    pub t0: f64,
    /// The powerModel `volumeFraction` if given.
    pub volume_fraction: Option<f64>,
}

/// The `interpolatedNuclearFuelPin` power model over a whole mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct InterpolatedNuclearFuelPin {
    cell_list: Vec<usize>,
    ia: Vec<f64>,
    alpha: Vec<f64>,
    heat_flux: Vec<f64>,
    power_density: Vec<f64>,
    t: Vec<f64>,
}

impl InterpolatedNuclearFuelPin {
    /// The constructor: `T` from the dictionary, the heat flux and power density zero, `iA = 2 alpha/r_fo`.
    ///
    /// # Errors
    /// [`RadialError::Invalid`] for a non-positive radius.
    pub fn new(n_cells: usize, regions: &[InterpolatedRegion], structure_alpha: &[f64]) -> Result<Self, RadialError> {
        let mut me = Self {
            cell_list: Vec::new(),
            ia: vec![0.0; n_cells],
            alpha: vec![0.0; n_cells],
            heat_flux: vec![0.0; n_cells],
            power_density: vec![0.0; n_cells],
            t: vec![0.0; n_cells],
        };
        for reg in regions {
            if reg.fuel_outer_radius <= 0.0 {
                return Err(RadialError::Invalid("interpolatedNuclearFuelPin: fuelOuterRadius must be positive"));
            }
            for &c in &reg.cells {
                me.cell_list.push(c);
                me.alpha[c] = reg.volume_fraction.unwrap_or(structure_alpha[c]);
                me.t[c] = reg.t0;
                me.ia[c] = 2.0 * me.alpha[c] / reg.fuel_outer_radius;
            }
        }
        Ok(me)
    }

    /// Set the externally mapped `interpolatedHeatFlux` [W/m^2] (every cell of the mesh).
    pub fn set_heat_flux(&mut self, heat_flux: &[f64]) {
        self.heat_flux.clear();
        self.heat_flux.extend_from_slice(heat_flux);
    }

    /// `correct(HT, H)`.
    pub fn correct(&mut self, ht_sum: &[f64], h_sum: &[f64]) {
        for &c in &self.cell_list {
            self.power_density[c] = self.heat_flux[c] * self.ia[c] / self.alpha[c];
            self.t[c] = (self.heat_flux[c] + ht_sum[c]) / h_sum[c];
        }
    }

    /// `correctT`: the surface temperature.
    #[must_use]
    pub fn surface_temperature(&self, cell: usize) -> f64 {
        self.t[cell]
    }

    /// `interpolatedPowerDensity` of a cell.
    #[must_use]
    pub fn power_density(&self, cell: usize) -> f64 {
        self.power_density[cell]
    }

    /// `volumetricArea.powerModel` of a cell.
    #[must_use]
    pub fn interfacial_area(&self, cell: usize) -> f64 {
        self.ia[cell]
    }

    /// The model's cells in `cellList_` order.
    #[must_use]
    pub fn cells(&self) -> &[usize] {
        &self.cell_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `T = (q'' + HT)/H` and `q''' = q'' iA/alpha = 2 q''/r_fo`, by hand.
    #[test]
    fn surface_temperature_and_power_density_by_hand() {
        let reg = InterpolatedRegion { cells: vec![1], fuel_outer_radius: 0.005, t0: 700.0, volume_fraction: None };
        let mut p = InterpolatedNuclearFuelPin::new(2, &[reg.clone()], &[0.0, 0.4]).unwrap();
        assert_eq!(p.surface_temperature(1), 700.0);
        p.set_heat_flux(&[0.0, 1.0e5]);
        p.correct(&[0.0, 1.0e4 * 600.0], &[0.0, 1.0e4]);
        assert!((p.surface_temperature(1) - 610.0).abs() < 1e-12);
        assert!((p.power_density(1) - 2.0e5 / 0.005).abs() < 1e-6);
        assert!((p.interfacial_area(1) - 160.0).abs() < 1e-12);
        assert_eq!(p.cells(), &[1]);
        let mut bad = reg;
        bad.fuel_outer_radius = 0.0;
        assert!(InterpolatedNuclearFuelPin::new(2, &[bad], &[0.0, 0.4]).is_err());
    }
}
