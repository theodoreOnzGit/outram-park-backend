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
use crate::thermophysics::constants::T_STD;
use crate::thermophysics::eos::EquationOfState;
use super::traits::ThermoModel;

/// Constant-Cp thermodynamic model.
///
/// Mirrors `Foam::hConstThermo<EOS>` from
/// `src/thermophysicalModels/specie/thermo/hConst/`.
///
/// Formulas (following OpenFOAM `hConstThermoI.H`):
/// ```text
/// Cp(p,T)  = cp_ + EOS::Cp(p,T)
/// Hs(p,T)  = cp_·(T − tref_) + hsref_ + EOS::H(p,T)
/// Ha(p,T)  = Hs(p,T) + Hf_
/// S(p,T)   = cp_·ln(T / T_std) + EOS::S(p,T)
/// ```
#[derive(Debug, Clone)]
pub struct HConstThermo<E: EquationOfState> {
    eos: E,
    cp: f64,      // Cp contribution [J/(kg·K)]
    hf: f64,      // heat of formation [J/kg]
    tref: f64,    // reference T for sensible enthalpy [K]
    hsref: f64,   // sensible enthalpy at Tref [J/kg]
}

impl<E: EquationOfState> HConstThermo<E> {
    pub fn new(
        eos: E,
        cp: SpecificHeatCapacity,
        hf: AvailableEnergy,
        tref: ThermodynamicTemperature,
        hsref: AvailableEnergy,
    ) -> Self {
        use uom::si::available_energy::joule_per_kilogram;
        Self {
            eos,
            cp: cp.get::<joule_per_kilogram_kelvin>(),
            hf: hf.get::<joule_per_kilogram>(),
            tref: tref.get::<kelvin>(),
            hsref: hsref.get::<joule_per_kilogram>(),
        }
    }
}

// --- EquationOfState delegation ---

impl<E: EquationOfState> EquationOfState for HConstThermo<E> {
    fn mol_weight(&self) -> MolarMass           { self.eos.mol_weight() }
    fn r(&self) -> SpecificHeatCapacity         { self.eos.r() }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { self.eos.rho(p, t) }
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { self.eos.psi(p, t) }
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { self.eos.z(p, t) }
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.eos.cp_m_cv(p, t) }
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.eos.cp_eos(p, t) }
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.eos.h_eos(p, t) }
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.eos.e_eos(p, t) }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.eos.s_eos(p, t) }
}

// --- ThermoModel ---

impl<E: EquationOfState> ThermoModel for HConstThermo<E> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.cp)
            + self.eos.cp_eos(p, t)
    }

    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        use uom::si::available_energy::joule_per_kilogram;
        let t_val = t.get::<kelvin>();
        let hs_val = self.cp * (t_val - self.tref) + self.hsref;
        AvailableEnergy::new::<joule_per_kilogram>(hs_val) + self.eos.h_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.hs(p, t) + self.hc()
    }

    fn hc(&self) -> AvailableEnergy {
        use uom::si::available_energy::joule_per_kilogram;
        AvailableEnergy::new::<joule_per_kilogram>(self.hf)
    }

    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let t_val = t.get::<kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.cp * (t_val / T_STD).ln())
            + self.eos.s_eos(p, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::available_energy::joule_per_kilogram;
    use approx::assert_relative_eq;

    fn air_thermo() -> HConstThermo<PerfectGas> {
        // Air: Cp ≈ 1004 J/(kg·K), zero formation enthalpy, Tref = 298.15 K
        HConstThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97)),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        )
    }

    #[test]
    fn cp_is_constant() {
        let a = air_thermo();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(600.0);
        let cp1 = a.cp(p, t1).get::<joule_per_kilogram_kelvin>();
        let cp2 = a.cp(p, t2).get::<joule_per_kilogram_kelvin>();
        assert_relative_eq!(cp1, cp2, epsilon = 1e-10);
        assert_relative_eq!(cp1, 1004.0, epsilon = 1e-10);
    }

    #[test]
    fn ha_roundtrip() {
        let a = air_thermo();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_in = ThermodynamicTemperature::new::<kelvin>(450.0);
        let ha = a.ha(p, t_in);
        let t_out = a.t_from_ha(ha, p, ThermodynamicTemperature::new::<kelvin>(300.0)).unwrap();
        assert_relative_eq!(t_in.get::<kelvin>(), t_out.get::<kelvin>(), epsilon = 1e-4);
    }

    #[test]
    fn hs_plus_hc_equals_ha() {
        let a = air_thermo();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(400.0);
        let ha = a.ha(p, t).get::<joule_per_kilogram>();
        let hs_hc = (a.hs(p, t) + a.hc()).get::<joule_per_kilogram>();
        assert_relative_eq!(ha, hs_hc, epsilon = 1e-6);
    }
}
