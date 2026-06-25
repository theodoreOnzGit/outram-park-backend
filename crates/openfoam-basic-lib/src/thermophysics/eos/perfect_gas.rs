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
use crate::thermophysics::constants::{P_REF, R_UNIVERSAL};
use super::traits::EquationOfState;

/// Ideal perfect gas: p = ρ·R·T.
///
/// Mirrors `Foam::perfectGas<Specie>` from
/// `src/thermophysicalModels/specie/equationOfState/perfectGas/`.
#[derive(Debug, Clone, Copy)]
pub struct PerfectGas {
    mol_weight: MolarMass,
}

impl PerfectGas {
    pub fn new(mol_weight: MolarMass) -> Self {
        Self { mol_weight }
    }
}

impl EquationOfState for PerfectGas {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }

    fn r(&self) -> SpecificHeatCapacity {
        let w = self.mol_weight.get::<kilogram_per_mole>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(R_UNIVERSAL / w)
    }

    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        // ρ = p / (R·T)
        p / (self.r() * t)
    }

    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
        // ψ = ρ / p  (same as 1/(R·T) for perfect gas; computed via uom for consistency)
        self.rho(p, t) / p
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
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(-r_val * (p_val / P_REF).ln())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use approx::assert_relative_eq;

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
