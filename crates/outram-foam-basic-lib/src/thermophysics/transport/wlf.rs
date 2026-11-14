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

//! `WLFTransport` of OpenFOAM-v2506 (ESI), gh:#1378: Williams-Landel-Ferry
//! viscosity `mu = mu0 exp(-C1 (T - Tr)/(C2 + T - Tr))`, `kappa = Cp mu / Pr`,
//! `alphah = mu / Pr`. Translated from `specie/transport/WLF/WLFTransportI.H`
//! (v2506), including `kappa = Cp * mu * rPr` in that operation order.

use super::traits::TransportModel;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::imports::*;
use crate::thermophysics::thermo::ThermoModel;
use petir::mathf::RealMath;

/// WLF viscosity with a constant Prandtl number.
#[derive(Debug, Clone)]
pub struct WlfTransport<T: ThermoModel> {
    thermo: T,
    mu0: f64,
    tr: f64,
    c1: f64,
    c2: f64,
    rpr: f64,
}

impl<T: ThermoModel> WlfTransport<T> {
    /// `mu0` `[Pa s]`, `tr` `[K]`, `c1` `[-]`, `c2` `[K]`, `pr` `[-]`
    /// (dictionary `mu0`, `Tr`, `C1`, `C2`, `Pr`).
    pub fn new(thermo: T, mu0: DynamicViscosity, tr: ThermodynamicTemperature, c1: f64, c2: f64, pr: Ratio) -> Self {
        Self {
            thermo,
            mu0: mu0.get::<pascal_second>(),
            tr: tr.get::<kelvin>(),
            c1,
            c2,
            rpr: 1.0 / pr.get::<ratio>(),
        }
    }
}

impl<T: ThermoModel> EquationOfState for WlfTransport<T> {
    fn mol_weight(&self) -> MolarMass {
        self.thermo.mol_weight()
    }
    fn r(&self) -> SpecificHeatCapacity {
        self.thermo.r()
    }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        self.thermo.rho(p, t)
    }
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
        self.thermo.psi(p, t)
    }
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio {
        self.thermo.z(p, t)
    }
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.thermo.cp_m_cv(p, t)
    }
    fn cv_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.thermo.cv_eos(p, t)
    }
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.thermo.cp_eos(p, t)
    }
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.thermo.h_eos(p, t)
    }
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.thermo.e_eos(p, t)
    }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.thermo.s_eos(p, t)
    }
}

impl<T: ThermoModel> ThermoModel for WlfTransport<T> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.thermo.cp(p, t)
    }
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.thermo.ha(p, t)
    }
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.thermo.hs(p, t)
    }
    fn hc(&self) -> AvailableEnergy {
        self.thermo.hc()
    }
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.thermo.s(p, t)
    }
}

impl<T: ThermoModel> TransportModel for WlfTransport<T> {
    fn mu(&self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        let t = t.get::<kelvin>();
        DynamicViscosity::new::<pascal_second>(
            self.mu0 * (-self.c1 * (t - self.tr) / (self.c2 + t - self.tr)).r_exp(),
        )
    }

    fn kappa(&self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity {
        let cp = self.thermo.cp(p, t).get::<joule_per_kilogram_kelvin>();
        let mu = self.mu(p, t).get::<pascal_second>();
        ThermalConductivity::new::<watt_per_meter_kelvin>(cp * mu * self.rpr)
    }

    fn alpha_h(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(self.mu(p, t).get::<pascal_second>() * self.rpr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::RhoConst;
    use crate::thermophysics::thermo::HConstThermo;

    #[test]
    fn mu_is_mu0_at_tr_and_falls_with_t() {
        let eos = RhoConst::new(
            MolarMass::new::<gram_per_mole>(100.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1100.0),
        );
        let th = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1800.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        let w = WlfTransport::new(
            th,
            DynamicViscosity::new::<pascal_second>(1.0e3),
            ThermodynamicTemperature::new::<kelvin>(300.0),
            17.44,
            51.6,
            Ratio::new::<ratio>(10.0),
        );
        let p = Pressure::new::<pascal>(1.0e5);
        let t = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        assert_eq!(w.mu(p, t(300.0)).get::<pascal_second>(), 1.0e3);
        assert!(w.mu(p, t(320.0)) < w.mu(p, t(300.0)));
        let mu = w.mu(p, t(320.0)).get::<pascal_second>();
        assert_eq!(w.alpha_h(p, t(320.0)).get::<pascal_second>(), mu * (1.0 / 10.0));
        assert!((w.kappa(p, t(320.0)).get::<watt_per_meter_kelvin>() - 1800.0 * mu * 0.1).abs() < 1e-9 * mu);
        let _ = (w.mol_weight(), w.r(), w.rho(p, t(300.0)), w.psi(p, t(300.0)), w.z(p, t(300.0)));
        let _ = (w.cp_m_cv(p, t(300.0)), w.cv_eos(p, t(300.0)), w.cp_eos(p, t(300.0)));
        let _ = (w.h_eos(p, t(300.0)), w.e_eos(p, t(300.0)), w.s_eos(p, t(300.0)));
        let _ = (w.ha(p, t(300.0)), w.hs(p, t(300.0)), w.hc(), w.s(p, t(300.0)));
    }
}
