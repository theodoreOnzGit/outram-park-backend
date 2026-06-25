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
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::thermo::ThermoModel;
use crate::interpolation::interpolate_xy;
use super::traits::TransportModel;

/// Tabulated transport model: μ(T) and κ(T) stored as (T, value) lookup tables.
///
/// Mirrors `Foam::tabulatedTransport<Thermo>` from
/// `src/thermophysicalModels/specie/transport/tabulated/`.
///
/// Both tables use `interpolate_xy` (piecewise-linear, clamped at endpoints).
/// Separate temperature grids may be provided for μ and κ.
#[derive(Debug, Clone)]
pub struct TabulatedTransport<T: ThermoModel> {
    thermo: T,
    mu_ts: Vec<f64>,    // temperature knots for μ [K]
    mu_vs: Vec<f64>,    // dynamic viscosity values [Pa·s]
    kappa_ts: Vec<f64>, // temperature knots for κ [K]
    kappa_vs: Vec<f64>, // thermal conductivity values [W/(m·K)]
}

impl<T: ThermoModel> TabulatedTransport<T> {
    /// `mu_table` = `(temperatures_K, viscosities_Pa_s)`.
    /// `kappa_table` = `(temperatures_K, conductivities_W_per_m_K)`.
    pub fn new(
        thermo: T,
        mu_table: (Vec<f64>, Vec<f64>),
        kappa_table: (Vec<f64>, Vec<f64>),
    ) -> Self {
        Self {
            thermo,
            mu_ts: mu_table.0,    mu_vs: mu_table.1,
            kappa_ts: kappa_table.0, kappa_vs: kappa_table.1,
        }
    }
}

// --- EquationOfState delegation ---

impl<T: ThermoModel> EquationOfState for TabulatedTransport<T> {
    fn mol_weight(&self) -> MolarMass                    { self.thermo.mol_weight() }
    fn r(&self) -> SpecificHeatCapacity                  { self.thermo.r() }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { self.thermo.rho(p, t) }
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { self.thermo.psi(p, t) }
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { self.thermo.z(p, t) }
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.cp_m_cv(p, t) }
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.cp_eos(p, t) }
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.h_eos(p, t) }
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.e_eos(p, t) }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.s_eos(p, t) }
}

// --- ThermoModel delegation ---

impl<T: ThermoModel> ThermoModel for TabulatedTransport<T> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.cp(p, t) }
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.ha(p, t) }
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.hs(p, t) }
    fn hc(&self) -> AvailableEnergy { self.thermo.hc() }
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.s(p, t) }
}

// --- TransportModel ---

impl<T: ThermoModel> TransportModel for TabulatedTransport<T> {
    fn mu(&self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(
            interpolate_xy(t.get::<kelvin>(), &self.mu_ts, &self.mu_vs),
        )
    }

    fn kappa(&self, _p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity {
        ThermalConductivity::new::<watt_per_meter_kelvin>(
            interpolate_xy(t.get::<kelvin>(), &self.kappa_ts, &self.kappa_vs),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;
    use crate::thermophysics::thermo::HConstThermo;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;
    use approx::assert_relative_eq;

    fn air_tabulated() -> TabulatedTransport<HConstThermo<PerfectGas>> {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        // Air viscosity at 2 points: 1.82e-5 Pa·s at 293 K, 2.20e-5 Pa·s at 373 K
        let mu_ts    = vec![293.0, 373.0];
        let mu_vs    = vec![1.82e-5, 2.20e-5];
        // Air conductivity at 2 points: 0.0257 at 293 K, 0.0311 at 373 K
        let kappa_ts = vec![293.0, 373.0];
        let kappa_vs = vec![0.0257, 0.0311];
        TabulatedTransport::new(thermo, (mu_ts, mu_vs), (kappa_ts, kappa_vs))
    }

    #[test]
    fn mu_at_knot() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(293.0);
        assert_relative_eq!(a.mu(p, t).get::<pascal_second>(), 1.82e-5, epsilon = 1e-10);
    }

    #[test]
    fn kappa_at_knot() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(373.0);
        assert_relative_eq!(a.kappa(p, t).get::<watt_per_meter_kelvin>(), 0.0311, epsilon = 1e-10);
    }

    #[test]
    fn mu_interpolated() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        // Midpoint of table: T = 333 K → mu = (1.82e-5 + 2.20e-5) / 2 = 2.01e-5
        let t = ThermodynamicTemperature::new::<kelvin>(333.0);
        assert_relative_eq!(a.mu(p, t).get::<pascal_second>(), 2.01e-5, epsilon = 1e-10);
    }

    #[test]
    fn mu_increases_with_temperature() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(360.0);
        assert!(a.mu(p, t1).get::<pascal_second>() < a.mu(p, t2).get::<pascal_second>());
    }

    #[test]
    fn alpha_h_is_kappa_over_cp() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(333.0);
        let kappa = a.kappa(p, t).get::<watt_per_meter_kelvin>();
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        use uom::si::dynamic_viscosity::pascal_second;
        let alpha_actual = a.alpha_h(p, t).get::<pascal_second>();
        assert_relative_eq!(alpha_actual, kappa / cp, epsilon = 1e-12);
    }
}
