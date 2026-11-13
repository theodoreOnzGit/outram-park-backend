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

use super::traits::ThermoModel;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::imports::*;
use crate::thermophysics::table::IntegratedNonUniformTable;

/// Tabulated-Cp thermodynamic model as OpenFOAM-v2506 (ESI) defines it
/// (gh:#1362 row 4). Ports `Foam::hTabulatedThermo<EOS>` of
/// `src/thermophysicalModels/specie/thermo/hTabulated/`.
///
/// One `Cp(T)` table, linearly extrapolated outside its range, from which
/// upstream integrates everything:
/// ```text
/// Cp(p,T) = Cp_table.f(T) + EOS::Cp(p,T)
/// Hs(p,T) = Cp_table.intfdT(T) + EOS::H(p,T)          (zero Cp-part at Tstd)
/// Ha(p,T) = Hs(p,T) + Hf
/// S(p,T)  = Cp_table.intfByTdT(T) + EOS::S(p,T) + Sf
/// ```
/// **Upstream cannot compile `S`** in v2506: it calls `EquationOfState::Sp`,
/// which no equation of state defines (`hTabulatedThermoI.H:106`). `S` here is
/// the evident intent with `EOS::S` for `Sp`; it is **not** verified against
/// upstream. Cp, Hs and Ha are.
///
/// This is a separate type from [`HTabulatedThermo`](crate::thermophysics::thermo::HTabulatedThermo),
/// which takes three independent tables (Cp, Ha, S) and clamps outside them;
/// both are kept.
#[derive(Debug, Clone)]
pub struct HTabulatedEsiThermo<E: EquationOfState> {
    eos: E,
    cp: IntegratedNonUniformTable,
    hf: f64,
    sf: f64,
}

impl<E: EquationOfState> HTabulatedEsiThermo<E> {
    /// `cp_table` is `(T [K], Cp [J/(kg K)])` ascending in `T`; `hf` the heat of
    /// formation `[J/kg]`; `sf` the entropy offset `[J/(kg K)]`.
    pub fn new(
        eos: E,
        cp_table: Vec<(f64, f64)>,
        hf: AvailableEnergy,
        sf: SpecificHeatCapacity,
    ) -> Self {
        Self {
            eos,
            cp: IntegratedNonUniformTable::new(cp_table),
            hf: hf.get::<joule_per_kilogram>(),
            sf: sf.get::<joule_per_kilogram_kelvin>(),
        }
    }
}

impl<E: EquationOfState> EquationOfState for HTabulatedEsiThermo<E> {
    fn mol_weight(&self) -> MolarMass {
        self.eos.mol_weight()
    }
    fn r(&self) -> SpecificHeatCapacity {
        self.eos.r()
    }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        self.eos.rho(p, t)
    }
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
        self.eos.psi(p, t)
    }
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio {
        self.eos.z(p, t)
    }
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.eos.cp_m_cv(p, t)
    }
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.eos.cp_eos(p, t)
    }
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.eos.h_eos(p, t)
    }
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.eos.e_eos(p, t)
    }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.eos.s_eos(p, t)
    }
}

impl<E: EquationOfState> ThermoModel for HTabulatedEsiThermo<E> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.cp.f(t.get::<kelvin>()))
            + self.eos.cp_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.hs(p, t) + self.hc()
    }

    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.cp.intfdt(t.get::<kelvin>()))
            + self.eos.h_eos(p, t)
    }

    fn hc(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.hf)
    }

    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
            self.cp.intf_by_tdt(t.get::<kelvin>()),
        ) + self.eos.s_eos(p, t)
            + SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.sf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;

    fn model() -> HTabulatedEsiThermo<PerfectGas> {
        HTabulatedEsiThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.96)),
            vec![(200.0, 1000.0), (400.0, 1020.0), (800.0, 1100.0), (1200.0, 1180.0), (2000.0, 1250.0)],
            AvailableEnergy::new::<joule_per_kilogram>(1.0e5),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(200.0),
        )
    }

    #[test]
    fn cp_hs_ha_consistent() {
        let m = model();
        let p = Pressure::new::<pascal>(1.0e5);
        let t = ThermodynamicTemperature::new::<kelvin>(600.0);
        assert_eq!(m.cp(p, t).get::<joule_per_kilogram_kelvin>(), 1060.0);
        let hs = m.hs(p, t).get::<joule_per_kilogram>();
        let ha = m.ha(p, t).get::<joule_per_kilogram>();
        assert!((ha - hs - 1.0e5).abs() < 1e-9);
        // Hs rises with T by about Cp dT
        let t2 = ThermodynamicTemperature::new::<kelvin>(700.0);
        let dh = m.hs(p, t2).get::<joule_per_kilogram>() - hs;
        assert!((dh - 100.0 * 1070.0).abs() < 1e-6);
        assert!(m.s(p, t2).get::<joule_per_kilogram_kelvin>() > m.s(p, t).get::<joule_per_kilogram_kelvin>());
        assert_eq!(m.hc().get::<joule_per_kilogram>(), 1.0e5);
    }
}
