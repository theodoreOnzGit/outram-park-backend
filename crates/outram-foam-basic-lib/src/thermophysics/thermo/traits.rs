// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

use crate::thermophysics::imports::*;
use crate::thermophysics::constants::{T_MIN, T_MAX};
use crate::thermophysics::error::ThermoError;
use crate::thermophysics::eos::EquationOfState;

/// Per-species thermodynamic model — sensible/absolute enthalpy, entropy, and
/// Newton-iteration T-solvers.
///
/// Mirrors the `thermo` layer in
/// `src/thermophysicalModels/specie/thermo/thermo/`.
///
/// Implementors must provide `cp`, `ha`, `hs`, `hc`, `s`.
/// `cv`, `t_from_ha`, `t_from_hs`, and `t_from_e` have default implementations.
pub trait ThermoModel: EquationOfState {
    /// Specific heat at constant pressure Cp  [J/(kg·K)].
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// Absolute specific enthalpy (sensible + formation + EOS departure)  [J/kg].
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// Sensible specific enthalpy: `ha − hc`  [J/kg].
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// Heat of formation (= chemical enthalpy at reference T)  [J/kg].
    fn hc(&self) -> AvailableEnergy;

    /// Specific entropy  [J/(kg·K)].
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// Specific heat at constant volume: Cv = Cp − cp_m_cv.
    fn cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.cp(p, t) - self.cp_m_cv(p, t)
    }

    /// Find T such that `ha(p, T) == ha_target`.  Newton iteration, max 50 steps.
    ///
    /// Returns `Err(ThermoError::NonConvergent)` if the iteration does not
    /// meet the tolerance `|ΔT/T| < 1e-6` within `MAX_ITER = 50` steps.
    fn t_from_ha(
        &self,
        ha_target: AvailableEnergy,
        p: Pressure,
        t0: ThermodynamicTemperature,
    ) -> Result<ThermodynamicTemperature, ThermoError> {
        newton_t(
            |t| self.ha(p, ThermodynamicTemperature::new::<kelvin>(t)),
            |t| self.cp(p, ThermodynamicTemperature::new::<kelvin>(t)),
            ha_target.get::<joule_per_kilogram>(),
            t0.get::<kelvin>(),
        )
    }

    /// Find T such that `hs(p, T) == hs_target`.
    fn t_from_hs(
        &self,
        hs_target: AvailableEnergy,
        p: Pressure,
        t0: ThermodynamicTemperature,
    ) -> Result<ThermodynamicTemperature, ThermoError> {
        // hs = ha - hc  →  ha_target = hs_target + hc
        let ha_target = hs_target + self.hc();
        self.t_from_ha(ha_target, p, t0)
    }

    /// Find T such that internal energy `ea(p, T) == e_target`, where
    /// `ea = ha − p/ρ`.
    fn t_from_e(
        &self,
        e_target: AvailableEnergy,
        p: Pressure,
        t0: ThermodynamicTemperature,
    ) -> Result<ThermodynamicTemperature, ThermoError> {
        newton_t(
            |t| {
                let tt = ThermodynamicTemperature::new::<kelvin>(t);
                let rho = self.rho(p, tt);
                let ea = self.ha(p, tt) - p / rho;
                ea
            },
            |t| self.cv(p, ThermodynamicTemperature::new::<kelvin>(t)),
            e_target.get::<joule_per_kilogram>(),
            t0.get::<kelvin>(),
        )
    }
}

/// Shared Newton iteration for T-inversion.
///
/// Finds `T` such that `f(T) = target` using `dfdT` as the derivative.
/// `T` is clamped to `[T_MIN, T_MAX]` at every step.
/// Returns `Err(ThermoError::NonConvergent)` if the tolerance
/// `|ΔT/T| < 1e-6` is not met within `MAX_ITER = 50` iterations.
/// Matches OpenFOAM's `species::thermo<T>::T()`.
#[allow(non_snake_case)]
fn newton_t(
    f: impl Fn(f64) -> AvailableEnergy,
    dfdT: impl Fn(f64) -> SpecificHeatCapacity,
    target: f64,
    t0: f64,
) -> Result<ThermodynamicTemperature, ThermoError> {
    const DTMAX: f64 = 500.0;
    const MAX_ITER: usize = 50;

    let mut t = t0.clamp(T_MIN, T_MAX);
    for _ in 0..MAX_ITER {
        let f_val = f(t).get::<joule_per_kilogram>();
        let cp_val = dfdT(t).get::<joule_per_kilogram_kelvin>();
        let dt = (-(f_val - target) / cp_val).clamp(-DTMAX, DTMAX);
        t = (t + dt).clamp(T_MIN, T_MAX);
        if dt.abs() / t < 1e-6 {
            return Ok(ThermodynamicTemperature::new::<kelvin>(t));
        }
    }
    Err(ThermoError::NonConvergent { max_iter: MAX_ITER, last_t: t })
}
