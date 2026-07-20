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
use crate::thermophysics::thermo::ThermoModel;

/// Per-species transport model — dynamic viscosity and thermal conductivity.
///
/// Mirrors the `transport` layer in
/// `src/thermophysicalModels/specie/transport/`.
///
/// `alpha_h` (thermal diffusivity = κ/Cp, units kg/(m·s) = same as DynamicViscosity)
/// has a default implementation via `kappa / cp`.
pub trait TransportModel: ThermoModel {
    /// Dynamic viscosity μ  [Pa·s = kg/(m·s)].
    fn mu(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;

    /// Thermal conductivity κ  [W/(m·K)].
    fn kappa(&self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity;

    /// Thermal diffusivity αh = κ / Cp  [kg/(m·s)]  (same dimension as DynamicViscosity).
    fn alpha_h(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        // κ / Cp:  [W/(m·K)] / [J/(kg·K)] = kg/(m·s)
        self.kappa(p, t) / self.cp(p, t)
    }
}
