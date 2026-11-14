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

use super::traits::EquationOfState;
use crate::thermophysics::constants::GasConstant;
use crate::thermophysics::imports::*;

/// Constant-density (incompressible) equation of state: ρ = const.
///
/// Mirrors `Foam::rhoConst<Specie>` from
/// `src/thermophysicalModels/specie/equationOfState/rhoConst/`.
#[derive(Debug, Clone, Copy)]
pub struct RhoConst {
    mol_weight: MolarMass,
    rho0: MassDensity,
    gas_constant: GasConstant,
}

impl RhoConst {
    /// Construct a constant-density EOS from molar mass W `[kg/mol]` and the fixed
    /// density ρ₀ `[kg/m³]` returned at every `(p, T)`.
    pub fn new(mol_weight: MolarMass, rho0: MassDensity) -> Self {
        Self { mol_weight, rho0, gas_constant: GasConstant::Codata2019 }
    }

    /// Select the universal gas constant (default [`GasConstant::Codata2019`];
    /// [`GasConstant::EsiV2506`] reproduces OpenFOAM-v2506 bit for bit).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }
}

impl EquationOfState for RhoConst {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }

    fn r(&self) -> SpecificHeatCapacity {
        let w = self.mol_weight.get::<kilogram_per_mole>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.gas_constant.specific(w))
    }

    fn rho(&self, _p: Pressure, _t: ThermodynamicTemperature) -> MassDensity {
        self.rho0
    }

    fn psi(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        // Incompressible: ∂ρ/∂p = 0.  Construct zero via dimension arithmetic.
        MassDensity::new::<kilogram_per_cubic_meter>(0.0) / Pressure::new::<pascal>(1.0)
    }

    /// Upstream `rhoConst::Z` returns `0` (a placeholder, as `icoPolynomial`'s
    /// does). ~~`p / (rho0 R T)`~~ **CORRECTED 2026-11-13 (gh:#1362 row 4):** the
    /// physical `p v / (R T)` was a port invention; v2506 (and the Foundation
    /// line) return 0. No consumer in the workspace reads it.
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(0.0)
    }

    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        // Incompressible: Cp = Cv (Maxwell relation gives 0)
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    /// Upstream `Cv` departure is `0` (the liquid-like EOS has no `R` in `CpMCv`).
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(0.0)
    }

    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(0.0)
    }

    fn s_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    fn water() -> RhoConst {
        RhoConst::new(
            MolarMass::new::<gram_per_mole>(18.015),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
        )
    }

    #[test]
    fn constant_density() {
        let w = water();
        let p1 = Pressure::new::<pascal>(101_325.0);
        let p2 = Pressure::new::<pascal>(10e6);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        assert_relative_eq!(
            w.rho(p1, t).get::<kilogram_per_cubic_meter>(),
            w.rho(p2, t).get::<kilogram_per_cubic_meter>(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn zero_compressibility() {
        let w = water();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let psi_times_p = (w.psi(p, t) * p).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(psi_times_p, 0.0, epsilon = 1e-10);
    }
}

#[cfg(test)]
mod gh1362_tests {
    use super::*;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    #[test]
    fn z_is_zero_as_upstream_and_gas_constant_option_applies() {
        let w = RhoConst::new(
            MolarMass::new::<gram_per_mole>(18.015),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
        );
        let p = Pressure::new::<pascal>(1.0e5);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        assert_eq!(w.z(p, t).get::<ratio>(), 0.0);
        let e = w.with_gas_constant(GasConstant::EsiV2506);
        assert!(e.r().get::<joule_per_kilogram_kelvin>() > w.r().get::<joule_per_kilogram_kelvin>());
    }
}
