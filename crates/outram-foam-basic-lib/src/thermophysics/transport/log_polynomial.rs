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

//! `logPolynomialTransport` of OpenFOAM-v2506 (ESI), gh:#1378:
//! `mu = exp(P_mu(ln T))`, `kappa = exp(P_kappa(ln T))`, `alphah = kappa/Cp`.
//! Translated from `specie/transport/logPolynomial/logPolynomialTransportI.H`
//! (v2506). Upstream's dictionary keys are `muLogCoeffs<N>` / `kappaLogCoeffs<N>`.

use super::traits::TransportModel;
use crate::polynomial::Polynomial;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::imports::*;
use crate::thermophysics::thermo::ThermoModel;
use petir::mathf::RealMath;

/// Log-polynomial transport: polynomials in `ln T`, exponentiated.
#[derive(Debug, Clone)]
pub struct LogPolynomialTransport<T: ThermoModel, const N: usize> {
    thermo: T,
    mu_poly: Polynomial<N>,
    kappa_poly: Polynomial<N>,
}

impl<T: ThermoModel, const N: usize> LogPolynomialTransport<T, N> {
    /// `mu_poly` and `kappa_poly` are polynomials of `ln(T/K)` whose value is `ln` of
    /// the property in SI (`Pa s`, `W/(m K)`).
    pub fn new(thermo: T, mu_poly: Polynomial<N>, kappa_poly: Polynomial<N>) -> Self {
        Self { thermo, mu_poly, kappa_poly }
    }
}

impl<T: ThermoModel, const N: usize> EquationOfState for LogPolynomialTransport<T, N> {
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

impl<T: ThermoModel, const N: usize> ThermoModel for LogPolynomialTransport<T, N> {
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

impl<T: ThermoModel, const N: usize> TransportModel for LogPolynomialTransport<T, N> {
    fn mu(&self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(
            self.mu_poly.value(t.get::<kelvin>().r_ln()).r_exp(),
        )
    }

    fn kappa(&self, _p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity {
        ThermalConductivity::new::<watt_per_meter_kelvin>(
            self.kappa_poly.value(t.get::<kelvin>().r_ln()).r_exp(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;
    use crate::thermophysics::thermo::HConstThermo;

    #[test]
    fn exp_of_log_polynomial_is_a_power_law_for_a_linear_poly() {
        let th = HConstThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.96)),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.5),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        // ln mu = ln(2e-6) + 0.7 ln T  =>  mu = 2e-6 T^0.7
        let lt = LogPolynomialTransport::new(
            th,
            Polynomial::<2>::new([(2.0e-6f64).ln(), 0.7]),
            Polynomial::<2>::new([(1.0e-4f64).ln(), 0.8]),
        );
        let p = Pressure::new::<pascal>(1.0e5);
        let t = ThermodynamicTemperature::new::<kelvin>(400.0);
        let mu = lt.mu(p, t).get::<pascal_second>();
        assert!((mu - 2.0e-6 * 400f64.powf(0.7)).abs() / mu < 1e-12);
        let k = lt.kappa(p, t).get::<watt_per_meter_kelvin>();
        assert!((k - 1.0e-4 * 400f64.powf(0.8)).abs() / k < 1e-12);
        let a = lt.alpha_h(p, t).get::<pascal_second>();
        assert!(a > 0.0);
        let _ = (lt.mol_weight(), lt.r(), lt.rho(p, t), lt.psi(p, t), lt.z(p, t), lt.cp_m_cv(p, t));
        let _ = (lt.cv_eos(p, t), lt.cp_eos(p, t), lt.h_eos(p, t), lt.e_eos(p, t), lt.s_eos(p, t));
        let _ = (lt.ha(p, t), lt.hs(p, t), lt.hc(), lt.s(p, t));
    }
}
