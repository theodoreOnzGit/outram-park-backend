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

use petir::mathf::RealMath;
use super::traits::EquationOfState;
use crate::thermophysics::constants::{GasConstant, P_REF};
use crate::thermophysics::imports::*;

/// Ideal perfect gas: p = ρ·R·T.
///
/// Mirrors `Foam::perfectGas<Specie>` from
/// `src/thermophysicalModels/specie/equationOfState/perfectGas/`.
#[derive(Debug, Clone, Copy)]
pub struct PerfectGas {
    mol_weight: MolarMass,
    gas_constant: GasConstant,
}

impl PerfectGas {
    /// Construct an ideal perfect-gas EOS from the species molar mass W `[kg/mol]`.
    pub fn new(mol_weight: MolarMass) -> Self {
        Self { mol_weight, gas_constant: GasConstant::Codata2019 }
    }

    /// Select the universal gas constant (default [`GasConstant::Codata2019`];
    /// [`GasConstant::EsiV2506`] reproduces OpenFOAM-v2506 bit for bit).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }
}

impl EquationOfState for PerfectGas {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }

    fn r(&self) -> SpecificHeatCapacity {
        let w = self.mol_weight.get::<kilogram_per_mole>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.gas_constant.specific(w))
    }

    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        // ρ = p / (R·T)
        p / (self.r() * t)
    }

    fn psi(&self, _p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
        // ψ = 1/(R·T), exactly upstream's `1.0/(this->R()*T)` (v2506 perfectGasI.H).
        // ~~`self.rho(p, t) / p`~~ **CORRECTED 2026-11-13 (gh:#1362 row 4):** the
        // old form is mathematically equal but rounds differently (27/40 of the
        // grid was 1 ulp off upstream).
        let rt = self.r().get::<joule_per_kilogram_kelvin>() * t.get::<kelvin>();
        MassDensity::new::<kilogram_per_cubic_meter>(1.0 / rt) / Pressure::new::<pascal>(1.0)
    }

    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(1.0)
    }

    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.r()
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

    fn s_eos(&self, p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        // S_eos = −R·ln(p / p_std)
        let p_val = p.get::<pascal>();
        let r_val = self.r().get::<joule_per_kilogram_kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(-r_val * (p_val / P_REF).r_ln())
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

    fn air() -> PerfectGas {
        PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97))
    }

    #[test]
    fn air_r() {
        // R_air ≈ 287.05 J/(kg·K)
        let r = air().r().get::<joule_per_kilogram_kelvin>();
        assert_relative_eq!(r, 287.05, epsilon = 0.2);
    }

    #[test]
    fn air_density_at_stp() {
        // ρ = 101325 / (287.05 × 293.15) ≈ 1.205 kg/m³
        let a = air();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(293.15);
        let rho = a.rho(p, t).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(rho, 1.205, epsilon = 0.005);
    }

    #[test]
    fn air_z_is_one() {
        let a = air();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        assert_relative_eq!(a.z(p, t).get::<ratio>(), 1.0, epsilon = 1e-10);
    }

    #[test]
    fn psi_times_p_equals_rho() {
        // ψ·p = ρ  (by definition; checks uom dimension algebra)
        let a = air();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let rho_direct = a.rho(p, t).get::<kilogram_per_cubic_meter>();
        let rho_via_psi = (a.psi(p, t) * p).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(rho_direct, rho_via_psi, epsilon = 1e-10);
    }
}

#[cfg(test)]
mod gh1362_tests {
    use super::*;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    #[test]
    fn psi_is_one_over_rt_and_gas_constant_option_scales_r() {
        let a = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.96));
        let b = a.with_gas_constant(GasConstant::EsiV2506);
        let p = Pressure::new::<pascal>(1.0e5);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let r = a.r().get::<joule_per_kilogram_kelvin>();
        assert_eq!(a.psi(p, t).value, 1.0 / (r * 300.0));
        let ratio_r = b.r().get::<joule_per_kilogram_kelvin>() / r;
        assert!((ratio_r - 1.0 - 9.0e-7).abs() < 2e-8);
        // entropy reference pressure is 1e5 Pa: S_eos vanishes there
        assert_eq!(a.s_eos(p, t).get::<joule_per_kilogram_kelvin>(), 0.0);
    }
}
