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
use super::traits::TransportModel;

/// Sutherland's law viscosity model.
///
/// Mirrors `Foam::sutherlandTransport<Thermo>` from
/// `src/thermophysicalModels/specie/transport/sutherland/`.
///
/// ```text
/// μ(T)  = As · √T / (1 + Ts/T)
/// κ(p,T) = μ(T) · Cv(p,T) · (1.32 + 1.77 · R/Cv(p,T))    [Eucken relation]
/// ```
///
/// `As` has implicit SI units kg/(m·s·K^½) and `Ts` is in K.
/// Both are stored as raw f64 rather than custom uom quantities.
#[derive(Debug, Clone)]
pub struct SutherlandTransport<T: ThermoModel> {
    thermo: T,
    as_: f64,  // Sutherland coefficient As [kg/(m·s·K^0.5)]
    ts: f64,   // Sutherland temperature Ts [K]
}

impl<T: ThermoModel> SutherlandTransport<T> {
    /// Construct directly from Sutherland coefficients As [kg/(m·s·K^0.5)] and Ts [K].
    pub fn new(thermo: T, as_: f64, ts: f64) -> Self {
        Self { thermo, as_, ts }
    }

    /// Construct from two viscosity reference points (μ₁, T₁) and (μ₂, T₂).
    ///
    /// Solves the 2×2 Sutherland system for As and Ts.
    pub fn from_two_points(
        thermo: T,
        mu1: DynamicViscosity,
        t1: ThermodynamicTemperature,
        mu2: DynamicViscosity,
        t2: ThermodynamicTemperature,
    ) -> Self {
        let mu1 = mu1.get::<pascal_second>();
        let mu2 = mu2.get::<pascal_second>();
        let t1 = t1.get::<kelvin>();
        let t2 = t2.get::<kelvin>();
        // μ = As√T/(1+Ts/T)  →  μ(1+Ts/T)/√T = As
        // Dividing: μ₁(1+Ts/T₁)/√T₁ = μ₂(1+Ts/T₂)/√T₂
        // Solve for Ts:
        //   μ₁/√T₁ + μ₁·Ts/(T₁^(3/2)) = μ₂/√T₂ + μ₂·Ts/(T₂^(3/2))
        //   Ts·(μ₁/T₁^(3/2) - μ₂/T₂^(3/2)) = μ₂/√T₂ - μ₁/√T₁
        let a = mu1 / t1.powf(1.5) - mu2 / t2.powf(1.5);
        let b = mu2 / t2.sqrt() - mu1 / t1.sqrt();
        let ts = b / a;
        let as_ = mu1 * (1.0 + ts / t1) / t1.sqrt();
        Self { thermo, as_, ts }
    }
}

// --- EquationOfState delegation ---

impl<T: ThermoModel> EquationOfState for SutherlandTransport<T> {
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

impl<T: ThermoModel> ThermoModel for SutherlandTransport<T> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.cp(p, t) }
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.ha(p, t) }
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.hs(p, t) }
    fn hc(&self) -> AvailableEnergy { self.thermo.hc() }
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.s(p, t) }
}

// --- TransportModel ---

impl<T: ThermoModel> TransportModel for SutherlandTransport<T> {
    fn mu(&self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        let tv = t.get::<kelvin>();
        DynamicViscosity::new::<pascal_second>(self.as_ * tv.sqrt() / (1.0 + self.ts / tv))
    }

    fn kappa(&self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity {
        // Eucken relation: κ = μ·Cv·(1.32 + 1.77·R/Cv)
        let mu = self.mu(p, t).get::<pascal_second>();
        let cv = self.thermo.cv(p, t).get::<joule_per_kilogram_kelvin>();
        let r = self.thermo.r().get::<joule_per_kilogram_kelvin>();
        ThermalConductivity::new::<watt_per_meter_kelvin>(mu * cv * (1.32 + 1.77 * r / cv))
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
    use approx::assert_relative_eq;

    fn air_sutherland() -> SutherlandTransport<HConstThermo<PerfectGas>> {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        // Standard Sutherland coefficients for air:
        // As = 1.458e-6 kg/(m·s·K^0.5),  Ts = 110.4 K
        SutherlandTransport::new(thermo, 1.458e-6, 110.4)
    }

    #[test]
    fn mu_at_293k() {
        // Air at 293 K: μ ≈ 1.82e-5 Pa·s
        let a = air_sutherland();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(293.0);
        let mu = a.mu(p, t).get::<pascal_second>();
        assert_relative_eq!(mu, 1.82e-5, epsilon = 2e-7);
    }

    #[test]
    fn mu_increases_with_temperature() {
        let a = air_sutherland();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(600.0);
        assert!(a.mu(p, t1).get::<pascal_second>() < a.mu(p, t2).get::<pascal_second>());
    }

    #[test]
    fn two_point_reconstruction() {
        // Construct from two points and verify mu at those points
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        let mu1_val = 1.716e-5_f64;
        let t1_val = 273.11_f64;
        let mu2_val = 1.987e-5_f64;
        let t2_val = 373.11_f64;
        let s = SutherlandTransport::from_two_points(
            thermo,
            DynamicViscosity::new::<pascal_second>(mu1_val),
            ThermodynamicTemperature::new::<kelvin>(t1_val),
            DynamicViscosity::new::<pascal_second>(mu2_val),
            ThermodynamicTemperature::new::<kelvin>(t2_val),
        );
        let p = Pressure::new::<pascal>(101_325.0);
        let got1 = s.mu(p, ThermodynamicTemperature::new::<kelvin>(t1_val)).get::<pascal_second>();
        let got2 = s.mu(p, ThermodynamicTemperature::new::<kelvin>(t2_val)).get::<pascal_second>();
        assert_relative_eq!(got1, mu1_val, epsilon = 1e-8);
        assert_relative_eq!(got2, mu2_val, epsilon = 1e-8);
    }
}
