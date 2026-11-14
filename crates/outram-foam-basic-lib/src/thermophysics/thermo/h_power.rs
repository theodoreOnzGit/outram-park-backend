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

//! `hPowerThermo` of OpenFOAM-v2506 (ESI), gh:#1378: `Cp = c0 (T/Tref)^n0`.
//!
//! Translated from `specie/thermo/hPower/hPowerThermoI.H` (v2506), read not
//! recalled: `Hs` and `S` are the closed integrals of the power law from
//! `Tstd`, with no `Hsref`, and `S` has no `ln` term. Differential:
//! `verification_and_validation/upstream_thermo_gh1362/README.md`.

use super::traits::ThermoModel;
use crate::thermophysics::constants::T_STD;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::imports::*;
use petir::mathf::RealMath;

/// Power-law heat capacity `Cp = c0 (T/Tref)^n0` over an equation of state.
#[derive(Debug, Clone)]
pub struct HPowerThermo<E: EquationOfState> {
    eos: E,
    c0: f64,
    n0: f64,
    tref: f64,
    hf: f64,
}

impl<E: EquationOfState> HPowerThermo<E> {
    /// `c0` `[J/(kg K)]`, exponent `n0` `[-]`, `tref` `[K]`, `hf` `[J/kg]`
    /// (dictionary `C0`, `n0`, `Tref`, `Hf`).
    pub fn new(
        eos: E,
        c0: SpecificHeatCapacity,
        n0: f64,
        tref: ThermodynamicTemperature,
        hf: AvailableEnergy,
    ) -> Self {
        Self {
            eos,
            c0: c0.get::<joule_per_kilogram_kelvin>(),
            n0,
            tref: tref.get::<kelvin>(),
            hf: hf.get::<joule_per_kilogram>(),
        }
    }

    fn hs_power(&self, t: f64) -> f64 {
        let (c0, n0, tref) = (self.c0, self.n0, self.tref);
        c0 * (t.r_powf(n0 + 1.0) - T_STD.r_powf(n0 + 1.0)) / (tref.r_powf(n0) * (n0 + 1.0))
    }

    fn s_power(&self, t: f64) -> f64 {
        let (c0, n0, tref) = (self.c0, self.n0, self.tref);
        c0 * (t.r_powf(n0) - T_STD.r_powf(n0)) / (tref.r_powf(n0) * n0)
    }
}

impl<E: EquationOfState> EquationOfState for HPowerThermo<E> {
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
    fn cv_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.eos.cv_eos(p, t)
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

impl<E: EquationOfState> ThermoModel for HPowerThermo<E> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
            self.c0 * (tv / self.tref).r_powf(self.n0),
        ) + self.eos.cp_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.hs(p, t) + self.hc()
    }

    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.hs_power(t.get::<kelvin>()))
            + self.eos.h_eos(p, t)
    }

    fn hc(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.hf)
    }

    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.s_power(t.get::<kelvin>()))
            + self.eos.s_eos(p, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;

    #[test]
    fn power_law_integrates_to_hs_and_s() {
        let th = HPowerThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.96)),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1000.0),
            0.7,
            ThermodynamicTemperature::new::<kelvin>(300.0),
            AvailableEnergy::new::<joule_per_kilogram>(1.0e5),
        );
        let p = Pressure::new::<pascal>(1.0e5);
        let t = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        // dHs/dT = Cp and dS/dT = Cp/T, by central difference (independent of the closed forms)
        let h = 0.01;
        let dh = (th.hs(p, t(500.0 + h)) - th.hs(p, t(500.0 - h))).get::<joule_per_kilogram>() / (2.0 * h);
        let cp = th.cp(p, t(500.0)).get::<joule_per_kilogram_kelvin>();
        assert!((dh - cp).abs() / cp < 1e-8, "{dh} {cp}");
        let ds = (th.s(p, t(500.0 + h)) - th.s(p, t(500.0 - h))).get::<joule_per_kilogram_kelvin>() / (2.0 * h);
        assert!((ds - cp / 500.0).abs() / (cp / 500.0) < 1e-8);
        assert_eq!(th.hs(p, t(T_STD)).get::<joule_per_kilogram>(), 0.0);
        let d = th.ha(p, t(400.0)) - th.hs(p, t(400.0));
        assert_eq!(d.get::<joule_per_kilogram>(), 1.0e5);
        assert!(th.rho(p, t(300.0)).get::<kilogram_per_cubic_meter>() > 0.0);
        let _ = (th.mol_weight(), th.r(), th.psi(p, t(300.0)), th.z(p, t(300.0)), th.cp_m_cv(p, t(300.0)));
        let _ = (th.cv_eos(p, t(300.0)), th.cp_eos(p, t(300.0)), th.h_eos(p, t(300.0)), th.e_eos(p, t(300.0)), th.s_eos(p, t(300.0)));
    }
}
