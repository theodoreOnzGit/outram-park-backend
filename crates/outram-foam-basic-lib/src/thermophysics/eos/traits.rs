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

/// Per-species equation of state — mesh-independent kernel.
///
/// Maps to the OpenFOAM `equationOfState` template layer in
/// `src/thermophysicalModels/specie/equationOfState/`.
///
/// All methods take `(p, T)` and return the corresponding property.
/// Enthalpy/entropy departure methods return the EOS *contribution* only;
/// the full quantity is assembled in `ThermoModel`.
pub trait EquationOfState {
    fn mol_weight(&self) -> MolarMass;

    /// Specific gas constant R = R_universal / W  [J/(kg·K)].
    fn r(&self) -> SpecificHeatCapacity;

    /// Density ρ = ρ(p, T)  [kg/m³].
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity;

    /// Isentropic compressibility ψ = ∂ρ/∂p|_T  [s²/m²].
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility;

    /// Compressibility factor Z = p·v / (R·T)  [-].
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio;

    /// Cp − Cv = R for ideal gas; Maxwell relation correction for real gas.
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// EOS contribution to Cp  [J/(kg·K)].  Zero for perfect/incompressible gas.
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// Enthalpy departure from ideal gas  [J/kg].
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// Internal-energy departure from ideal gas  [J/kg].
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// EOS contribution to specific entropy  [J/(kg·K)].
    /// For perfect gas: `−R·ln(p/p_ref)`.
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
}
