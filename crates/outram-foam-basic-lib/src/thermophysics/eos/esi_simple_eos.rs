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

//! Six more specie equations of state of OpenFOAM-v2506 (ESI), gh:#1378:
//! `incompressiblePerfectGas`, `Boussinesq`, `adiabaticPerfectFluid`,
//! `perfectFluid`, `rPolynomial` and `linear`.
//!
//! Each is a line-for-line translation of the `*I.H` of
//! `src/thermophysicalModels/specie/equationOfState/<name>/` in ESI v2506
//! (read, not recalled). Every departure (`Cp`, `H`, `E`, `Cv`) is `0`; the
//! members that differ are `rho`, `psi`, `Z`, `CpMCv` and `S`, listed on each
//! type. `Pstd` is `1e5` Pa (`P_REF`). Differential against the real classes:
//! `verification_and_validation/upstream_thermo_gh1362/README.md` (gh:#1378
//! section).

use super::traits::EquationOfState;
use crate::thermophysics::constants::{GasConstant, P_REF};
use crate::thermophysics::imports::*;
use petir::mathf::RealMath;

fn jkgk(v: f64) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(v)
}

fn psi_of(v: f64) -> Compressibility {
    MassDensity::new::<kilogram_per_cubic_meter>(v) / Pressure::new::<pascal>(1.0)
}

fn rho_of(v: f64) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(v)
}

fn zero_energy() -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(0.0)
}

fn spec_r(mol_weight: MolarMass, gc: GasConstant) -> SpecificHeatCapacity {
    jkgk(gc.specific(mol_weight.get::<kilogram_per_mole>()))
}

/// `incompressiblePerfectGas`: `rho = pRef/(R T)`; `psi = Z = CpMCv = S = 0`.
#[derive(Debug, Clone, Copy)]
pub struct IncompressiblePerfectGas {
    mol_weight: MolarMass,
    p_ref: f64,
    gas_constant: GasConstant,
}

impl IncompressiblePerfectGas {
    /// `p_ref` is the fixed reference pressure `[Pa]` of the dictionary entry `pRef`.
    pub fn new(mol_weight: MolarMass, p_ref: Pressure) -> Self {
        Self { mol_weight, p_ref: p_ref.get::<pascal>(), gas_constant: GasConstant::Codata2019 }
    }

    /// Select the universal gas constant (default CODATA 2019).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }
}

impl EquationOfState for IncompressiblePerfectGas {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }
    fn r(&self) -> SpecificHeatCapacity {
        spec_r(self.mol_weight, self.gas_constant)
    }
    fn rho(&self, _p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        rho_of(self.p_ref / (self.r().get::<joule_per_kilogram_kelvin>() * t.get::<kelvin>()))
    }
    fn psi(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        psi_of(0.0)
    }
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(0.0)
    }
    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn s_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
}

/// `Boussinesq`: `rho = rho0 (1 - beta (T - T0))`; `psi = Z = CpMCv = S = 0`.
#[derive(Debug, Clone, Copy)]
pub struct BoussinesqEos {
    mol_weight: MolarMass,
    rho0: f64,
    t0: f64,
    beta: f64,
    gas_constant: GasConstant,
}

impl BoussinesqEos {
    /// `rho0` `[kg/m^3]`, `t0` `[K]`, `beta` `[1/K]` (dictionary `rho0`, `T0`, `beta`).
    pub fn new(
        mol_weight: MolarMass,
        rho0: MassDensity,
        t0: ThermodynamicTemperature,
        beta: f64,
    ) -> Self {
        Self {
            mol_weight,
            rho0: rho0.get::<kilogram_per_cubic_meter>(),
            t0: t0.get::<kelvin>(),
            beta,
            gas_constant: GasConstant::Codata2019,
        }
    }

    /// Select the universal gas constant (default CODATA 2019).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }
}

impl EquationOfState for BoussinesqEos {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }
    fn r(&self) -> SpecificHeatCapacity {
        spec_r(self.mol_weight, self.gas_constant)
    }
    fn rho(&self, _p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        rho_of(self.rho0 * (1.0 - self.beta * (t.get::<kelvin>() - self.t0)))
    }
    fn psi(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        psi_of(0.0)
    }
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(0.0)
    }
    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn s_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
}

/// `adiabaticPerfectFluid`: `rho = rho0 ((p + B)/(p0 + B))^(1/gamma)`,
/// `Z = 1`, `CpMCv = 0`, with upstream's `psi` and `S`.
#[derive(Debug, Clone, Copy)]
pub struct AdiabaticPerfectFluid {
    mol_weight: MolarMass,
    p0: f64,
    rho0: f64,
    gamma: f64,
    b: f64,
    gas_constant: GasConstant,
}

impl AdiabaticPerfectFluid {
    /// Dictionary `p0` `[Pa]`, `rho0` `[kg/m^3]`, `gamma` `[-]`, `B` `[Pa]`.
    pub fn new(
        mol_weight: MolarMass,
        p0: Pressure,
        rho0: MassDensity,
        gamma: f64,
        b: Pressure,
    ) -> Self {
        Self {
            mol_weight,
            p0: p0.get::<pascal>(),
            rho0: rho0.get::<kilogram_per_cubic_meter>(),
            gamma,
            b: b.get::<pascal>(),
            gas_constant: GasConstant::Codata2019,
        }
    }

    /// Select the universal gas constant (default CODATA 2019).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }
}

impl EquationOfState for AdiabaticPerfectFluid {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }
    fn r(&self) -> SpecificHeatCapacity {
        spec_r(self.mol_weight, self.gas_constant)
    }
    fn rho(&self, p: Pressure, _t: ThermodynamicTemperature) -> MassDensity {
        let p = p.get::<pascal>();
        rho_of(self.rho0 * ((p + self.b) / (self.p0 + self.b)).r_powf(1.0 / self.gamma))
    }
    fn psi(&self, p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        let p = p.get::<pascal>();
        psi_of(
            (self.rho0 / (self.gamma * (self.p0 + self.b)))
                * ((p + self.b) / (self.p0 + self.b)).r_powf(1.0 / self.gamma - 1.0),
        )
    }
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(1.0)
    }
    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let p = p.get::<pascal>();
        let t = t.get::<kelvin>();
        let n = 1.0 - 1.0 / self.gamma;
        jkgk(
            -(self.p0 + self.b).r_powf(1.0 / self.gamma)
                * ((p + self.b).r_powf(n) - (P_REF + self.b).r_powf(n))
                / (self.rho0 * t * n),
        )
    }
}

/// `perfectFluid`: `rho = rho0 + p/(R T)` with the dictionary's own `R`
/// (upstream overrides `R()`), `psi = 1/(R T)`, `Z = 1`,
/// `CpMCv = R (p/(rho R T))^2`, `S = -R ln(p/Pstd)`.
#[derive(Debug, Clone, Copy)]
pub struct PerfectFluid {
    mol_weight: MolarMass,
    r: f64,
    rho0: f64,
}

impl PerfectFluid {
    /// `r` is the dictionary entry `R` `[J/(kg K)]`, used instead of `RR/W`.
    pub fn new(mol_weight: MolarMass, r: SpecificHeatCapacity, rho0: MassDensity) -> Self {
        Self {
            mol_weight,
            r: r.get::<joule_per_kilogram_kelvin>(),
            rho0: rho0.get::<kilogram_per_cubic_meter>(),
        }
    }

    fn rho_raw(&self, p: f64, t: f64) -> f64 {
        self.rho0 + p / (self.r * t)
    }
}

impl EquationOfState for PerfectFluid {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }
    fn r(&self) -> SpecificHeatCapacity {
        jkgk(self.r)
    }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        rho_of(self.rho_raw(p.get::<pascal>(), t.get::<kelvin>()))
    }
    fn psi(&self, _p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
        psi_of(1.0 / (self.r * t.get::<kelvin>()))
    }
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(1.0)
    }
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let (p, t) = (p.get::<pascal>(), t.get::<kelvin>());
        let r = self.r;
        let rho = self.rho_raw(p, t);
        let x = p / (rho * r * t);
        jkgk(r * (x * x))
    }
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn s_eos(&self, p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(-self.r * (p.get::<pascal>() / P_REF).r_ln())
    }
}

/// `rPolynomial`: `1/rho = C0 + C1 T + C2 T^2 - C3 p - C4 p T`,
/// `psi = rho^2 (C3 + C4 T)`, `Z = 1`, `CpMCv = S = 0`.
#[derive(Debug, Clone, Copy)]
pub struct RPolynomialEos {
    mol_weight: MolarMass,
    c: [f64; 5],
    gas_constant: GasConstant,
}

impl RPolynomialEos {
    /// `c` is the dictionary list `C` (five coefficients, SI).
    pub fn new(mol_weight: MolarMass, c: [f64; 5]) -> Self {
        Self { mol_weight, c, gas_constant: GasConstant::Codata2019 }
    }

    /// Select the universal gas constant (default CODATA 2019).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }

    fn rho_raw(&self, p: f64, t: f64) -> f64 {
        let c = &self.c;
        1.0 / (c[0] + (c[1] + c[2] * t - c[4] * p) * t - c[3] * p)
    }
}

impl EquationOfState for RPolynomialEos {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }
    fn r(&self) -> SpecificHeatCapacity {
        spec_r(self.mol_weight, self.gas_constant)
    }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        rho_of(self.rho_raw(p.get::<pascal>(), t.get::<kelvin>()))
    }
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
        let (p, t) = (p.get::<pascal>(), t.get::<kelvin>());
        let rho = self.rho_raw(p, t);
        psi_of((rho * rho) * (self.c[3] + self.c[4] * t))
    }
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(1.0)
    }
    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn s_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
}

/// `linear`: `rho = rho0 + psi p`, `psi` constant, `Z = 1`, `CpMCv = 0`,
/// `S = -ln((rho0 + psi p)/(rho0 + psi Pstd))/(T psi)`.
#[derive(Debug, Clone, Copy)]
pub struct LinearEos {
    mol_weight: MolarMass,
    psi: f64,
    rho0: f64,
    gas_constant: GasConstant,
}

impl LinearEos {
    /// `psi` `[s^2/m^2]` and `rho0` `[kg/m^3]` of the dictionary.
    pub fn new(mol_weight: MolarMass, psi: Compressibility, rho0: MassDensity) -> Self {
        Self {
            mol_weight,
            psi: psi.value,
            rho0: rho0.get::<kilogram_per_cubic_meter>(),
            gas_constant: GasConstant::Codata2019,
        }
    }

    /// Select the universal gas constant (default CODATA 2019).
    pub fn with_gas_constant(mut self, gas_constant: GasConstant) -> Self {
        self.gas_constant = gas_constant;
        self
    }
}

impl EquationOfState for LinearEos {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }
    fn r(&self) -> SpecificHeatCapacity {
        spec_r(self.mol_weight, self.gas_constant)
    }
    fn rho(&self, p: Pressure, _t: ThermodynamicTemperature) -> MassDensity {
        rho_of(self.rho0 + self.psi * p.get::<pascal>())
    }
    fn psi(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        psi_of(self.psi)
    }
    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(1.0)
    }
    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cv_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        jkgk(0.0)
    }
    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        zero_energy()
    }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let p = p.get::<pascal>();
        let t = t.get::<kelvin>();
        jkgk(-((self.rho0 + self.psi * p) / (self.rho0 + self.psi * P_REF)).r_ln() / (t * self.psi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> MolarMass {
        MolarMass::new::<gram_per_mole>(28.96)
    }
    fn p(v: f64) -> Pressure {
        Pressure::new::<pascal>(v)
    }
    fn t(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }
    fn jk(v: SpecificHeatCapacity) -> f64 {
        v.get::<joule_per_kilogram_kelvin>()
    }

    #[test]
    fn incompressible_gas_and_boussinesq_closed_forms() {
        let ipg = IncompressiblePerfectGas::new(w(), p(1.0e5));
        let r = jk(ipg.r());
        assert_eq!(ipg.rho(p(3e6), t(300.0)).get::<kilogram_per_cubic_meter>(), 1.0e5 / (r * 300.0));
        assert_eq!(ipg.psi(p(1e5), t(300.0)).value, 0.0);
        assert_eq!(ipg.z(p(1e5), t(300.0)).get::<ratio>(), 0.0);
        assert_eq!(jk(ipg.cv_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(ipg.cp_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(ipg.cp_m_cv(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(ipg.s_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(ipg.h_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(ipg.e_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert!(ipg.mol_weight().get::<kilogram_per_mole>() > 0.0);
        assert!(jk(ipg.with_gas_constant(GasConstant::EsiV2506).r()) > r);

        let b = BoussinesqEos::new(
            w(),
            MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
            t(300.0),
            2e-4,
        );
        assert_eq!(
            b.rho(p(1e5), t(310.0)).get::<kilogram_per_cubic_meter>(),
            1000.0 * (1.0 - 2e-4 * 10.0)
        );
        assert_eq!(b.z(p(1e5), t(310.0)).get::<ratio>(), 0.0);
        assert_eq!(b.psi(p(1e5), t(310.0)).value, 0.0);
        assert_eq!(b.h_eos(p(1e5), t(310.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(b.e_eos(p(1e5), t(310.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(jk(b.s_eos(p(1e5), t(310.0))), 0.0);
        assert_eq!(jk(b.cp_eos(p(1e5), t(310.0))), 0.0);
        assert_eq!(jk(b.cv_eos(p(1e5), t(310.0))), 0.0);
        assert_eq!(jk(b.cp_m_cv(p(1e5), t(310.0))), 0.0);
        assert!(b.mol_weight().get::<kilogram_per_mole>() > 0.0);
        assert!(jk(b.with_gas_constant(GasConstant::EsiV2506).r()) > 287.0);
    }

    #[test]
    fn adiabatic_fluid_reference_state_and_psi_is_drho_dp() {
        let a = AdiabaticPerfectFluid::new(
            w(),
            p(1.0e5),
            MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
            7.15,
            p(3.0e8),
        );
        let rho_ref = a.rho(p(1.0e5), t(300.0)).get::<kilogram_per_cubic_meter>();
        assert!((rho_ref - 1000.0).abs() < 1e-9);
        // psi = d rho / d p by central difference, independent of the closed form
        let h = 10.0;
        let fd = (a.rho(p(2.0e6 + h), t(300.0)) - a.rho(p(2.0e6 - h), t(300.0)))
            .get::<kilogram_per_cubic_meter>()
            / (2.0 * h);
        let psi = a.psi(p(2.0e6), t(300.0)).value;
        assert!((fd - psi).abs() / psi < 1e-6, "{fd} {psi}");
        assert_eq!(a.z(p(1e5), t(300.0)).get::<ratio>(), 1.0);
        assert!(jk(a.s_eos(p(2.0e6), t(300.0))).is_finite());
        assert_eq!(jk(a.cv_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(a.cp_m_cv(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(a.cp_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(a.h_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(a.e_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert!(jk(a.with_gas_constant(GasConstant::EsiV2506).r()) > 287.0);
        assert!(a.mol_weight().get::<kilogram_per_mole>() > 0.0);
    }

    #[test]
    fn perfect_fluid_linear_and_rpolynomial_consistency() {
        let r = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(3000.0);
        let pf = PerfectFluid::new(w(), r, MassDensity::new::<kilogram_per_cubic_meter>(900.0));
        let fd = (pf.rho(p(2.0e6 + 1.0), t(300.0)) - pf.rho(p(2.0e6 - 1.0), t(300.0)))
            .get::<kilogram_per_cubic_meter>()
            / 2.0;
        assert!((fd - pf.psi(p(2e6), t(300.0)).value).abs() / fd < 1e-6);
        assert!(jk(pf.cp_m_cv(p(2e6), t(300.0))) > 0.0);
        assert_eq!(jk(pf.s_eos(p(1.0e5), t(300.0))), 0.0);
        assert_eq!(pf.z(p(1e5), t(300.0)).get::<ratio>(), 1.0);
        assert_eq!(jk(pf.cv_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(pf.cp_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(pf.h_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(pf.e_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(jk(pf.r()), 3000.0);
        assert!(pf.mol_weight().get::<kilogram_per_mole>() > 0.0);

        let lin = LinearEos::new(
            w(),
            MassDensity::new::<kilogram_per_cubic_meter>(4.5e-7) / Pressure::new::<pascal>(1.0),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
        );
        assert_eq!(
            lin.rho(p(2e6), t(300.0)).get::<kilogram_per_cubic_meter>(),
            998.0 + 4.5e-7 * 2e6
        );
        assert_eq!(lin.psi(p(2e6), t(300.0)).value, 4.5e-7);
        assert_eq!(jk(lin.s_eos(p(1.0e5), t(300.0))), 0.0);
        assert!(jk(lin.s_eos(p(2e6), t(300.0))) < 0.0);
        assert_eq!(lin.h_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(lin.e_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert!(lin.mol_weight().get::<kilogram_per_mole>() > 0.0);
        assert!(jk(lin.with_gas_constant(GasConstant::EsiV2506).r()) > 287.0);
        assert_eq!(jk(lin.cv_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(lin.cp_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(lin.cp_m_cv(p(1e5), t(300.0))), 0.0);
        assert_eq!(lin.z(p(1e5), t(300.0)).get::<ratio>(), 1.0);

        let rp = RPolynomialEos::new(w(), [1.0e-3, 2.0e-7, 1.0e-9, 4.0e-13, 1.0e-16]);
        let fd = (rp.rho(p(2.0e6 + 10.0), t(300.0)) - rp.rho(p(2.0e6 - 10.0), t(300.0)))
            .get::<kilogram_per_cubic_meter>()
            / 20.0;
        assert!((fd - rp.psi(p(2e6), t(300.0)).value).abs() / fd < 1e-6);
        assert_eq!(rp.h_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(rp.e_eos(p(1e5), t(300.0)).get::<joule_per_kilogram>(), 0.0);
        assert!(rp.mol_weight().get::<kilogram_per_mole>() > 0.0);
        assert!(jk(rp.with_gas_constant(GasConstant::EsiV2506).r()) > 287.0);
        assert_eq!(jk(rp.cv_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(rp.cp_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(rp.cp_m_cv(p(1e5), t(300.0))), 0.0);
        assert_eq!(jk(rp.s_eos(p(1e5), t(300.0))), 0.0);
        assert_eq!(rp.z(p(1e5), t(300.0)).get::<ratio>(), 1.0);
    }
}
