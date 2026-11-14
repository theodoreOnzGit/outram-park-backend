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

//! `eConstThermo` of OpenFOAM-v2506 (ESI), gh:#1378: constant `Cv`.
//!
//! **What v2506 can instantiate.** `eConstThermoI.H` defines only `limit`, `Cv`,
//! `Es`, `Ea`, `Hc`, `Gstd` and `dCpdT`; `S` calls an undefined `Cp`, so
//! `eConstThermo::S`, `Cp`, `Hs`, `Ha` do not exist upstream (same class of
//! defect as `hTabulatedThermo::S`, gh:#1362). Those four, required by the
//! port's [`ThermoModel`], are given here in the Foundation reading
//! (`Cp = Cv + CpMCv`, `Hs = Es + p/rho`, `S = Cp ln(T/Tstd) + S_eos`) and are
//! NOT compared with ESI. The comparable members are the inherent
//! [`EConstThermo::cv`], [`EConstThermo::es`], [`EConstThermo::ea`] and
//! [`ThermoModel::hc`].

use super::traits::ThermoModel;
use crate::thermophysics::constants::T_STD;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::imports::*;
use petir::mathf::RealMath;

/// Constant-`Cv` thermo over an equation of state.
#[derive(Debug, Clone)]
pub struct EConstThermo<E: EquationOfState> {
    eos: E,
    cv: f64,
    hf: f64,
    tref: f64,
    esref: f64,
}

impl<E: EquationOfState> EConstThermo<E> {
    /// `cv` `[J/(kg K)]`, `hf` `[J/kg]`, `tref` `[K]` (upstream default `Tstd`),
    /// `esref` `[J/kg]` (dictionary `Eref`, default 0).
    pub fn new(
        eos: E,
        cv: SpecificHeatCapacity,
        hf: AvailableEnergy,
        tref: ThermodynamicTemperature,
        esref: AvailableEnergy,
    ) -> Self {
        Self {
            eos,
            cv: cv.get::<joule_per_kilogram_kelvin>(),
            hf: hf.get::<joule_per_kilogram>(),
            tref: tref.get::<kelvin>(),
            esref: esref.get::<joule_per_kilogram>(),
        }
    }

    /// Upstream `Cv(p, T) = Cv_ + EquationOfState::Cv(p, T)`.
    pub fn cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.cv) + self.eos.cv_eos(p, t)
    }

    /// Upstream `Es(p, T) = Cv_ (T - Tref_) + Esref_ + EquationOfState::E(p, T)`.
    pub fn es(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(
            self.cv * (t.get::<kelvin>() - self.tref) + self.esref,
        ) + self.eos.e_eos(p, t)
    }

    /// Upstream `Ea(p, T) = Es(p, T) + Hc()`.
    pub fn ea(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.es(p, t) + self.hc()
    }
}

impl<E: EquationOfState> EquationOfState for EConstThermo<E> {
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

impl<E: EquationOfState> ThermoModel for EConstThermo<E> {
    /// Foundation reading (`Cp = Cv + CpMCv`); not in ESI v2506.
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.cv(p, t) + self.eos.cp_m_cv(p, t)
    }

    /// Foundation reading (`Ha = Es + p/rho + Hc`); not in ESI v2506.
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.hs(p, t) + self.hc()
    }

    /// Foundation reading (`Hs = Es + p/rho`); not in ESI v2506.
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.es(p, t) + p / self.eos.rho(p, t)
    }

    fn hc(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.hf)
    }

    /// Foundation reading (`S = Cp ln(T/Tstd) + S_eos`); not in ESI v2506.
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let cp = self.cp(p, t).get::<joule_per_kilogram_kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
            cp * (t.get::<kelvin>() / T_STD).r_ln(),
        ) + self.eos.s_eos(p, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::{PerfectGas, RhoConst};

    fn p() -> Pressure {
        Pressure::new::<pascal>(1.0e5)
    }
    fn t(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }
    fn th<E: EquationOfState>(eos: E) -> EConstThermo<E> {
        EConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(718.0),
            AvailableEnergy::new::<joule_per_kilogram>(2.5e5),
            t(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        )
    }

    #[test]
    fn es_is_cv_integral_and_gas_cp_is_cv_plus_r() {
        let g = th(PerfectGas::new(MolarMass::new::<gram_per_mole>(28.96)));
        let r = g.r().get::<joule_per_kilogram_kelvin>();
        assert_eq!(g.cv(p(), t(500.0)).get::<joule_per_kilogram_kelvin>(), 718.0);
        assert_eq!(g.cp(p(), t(500.0)).get::<joule_per_kilogram_kelvin>(), 718.0 + r);
        assert_eq!(g.es(p(), t(298.15)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(g.es(p(), t(398.15)).get::<joule_per_kilogram>(), 718.0 * (398.15 - 298.15));
        assert_eq!(
            (g.ea(p(), t(400.0)) - g.es(p(), t(400.0))).get::<joule_per_kilogram>(),
            2.5e5
        );
        // Hs = Es + p/rho = Es + R T for a perfect gas
        let hs = g.hs(p(), t(400.0)).get::<joule_per_kilogram>();
        let es = g.es(p(), t(400.0)).get::<joule_per_kilogram>();
        assert!((hs - es - r * 400.0).abs() < 1e-6);
        assert!(g.ha(p(), t(400.0)).get::<joule_per_kilogram>() > hs);
        assert!(g.s(p(), t(400.0)).get::<joule_per_kilogram_kelvin>() > 0.0);
        let _ = (g.mol_weight(), g.rho(p(), t(300.0)), g.psi(p(), t(300.0)), g.z(p(), t(300.0)));
        let _ = (g.cp_m_cv(p(), t(300.0)), g.cv_eos(p(), t(300.0)), g.cp_eos(p(), t(300.0)));
        let _ = (g.h_eos(p(), t(300.0)), g.e_eos(p(), t(300.0)), g.s_eos(p(), t(300.0)));
    }

    #[test]
    fn liquid_cv_has_no_departure() {
        let l = th(RhoConst::new(
            MolarMass::new::<gram_per_mole>(18.0),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
        ));
        assert_eq!(l.cv(p(), t(300.0)).get::<joule_per_kilogram_kelvin>(), 718.0);
    }
}
