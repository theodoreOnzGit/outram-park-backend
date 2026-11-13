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
use crate::polynomial::Polynomial;
use crate::thermophysics::constants::{P_REF, T_STD};
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::imports::*;

/// Polynomial Cp thermodynamic model.
///
/// Mirrors `Foam::hPolynomialThermo<EOS, PolySize>` from
/// `src/thermophysicalModels/specie/thermo/hPolynomial/`.
///
/// Formulas (matching `hPolynomialThermoI.H`):
/// ```text
/// Cp(p,T) = cps.value(T) + EOS::Cp(p,T)
/// Ha(p,T) = hf + cps.integral(T_std, T) + EOS::H(p,T)
/// Hc()    = hf
/// Hs(p,T) = Ha(p,T) − Hc()
/// S(p,T)  = sf + cps.integral_minus1(0).value(T)
///               − cps.integral_minus1(0).value(T_std)
///               + EOS::S(p,T)
/// ```
/// where `T_std = 298.15 K` and `cps.integral_minus1(0)` is the antiderivative
/// of `Cp/T` (activating the `log_coeff·ln(T)` term).
///
/// # Two conventions, both kept (gh:#1362 row 4)
/// [`HPolynomialThermo::new`] is the **Foundation** form (`Ha(Tstd) = hf`,
/// `S(Tstd) = sf`). [`HPolynomialThermo::new_esi_v2506`] is the **ESI v2506**
/// form, a translation of `hPolynomialThermo.C`: the enthalpy and entropy
/// polynomials are offset to `Href` and `Sref` at `Tref`, `Ha = hCoeffs(T)`
/// with **no** `Hf` added, `Hs = Ha - Hf`, and `Sf` is read but unused. In the
/// ESI form `Hs(Tref) = Href - Hf`, not `Href`. Measured on the 10 x 4 grid,
/// the two forms differ by `Hf` in `Ha` (`1e5 J/kg` on the test inputs).
#[derive(Debug, Clone)]
pub struct HPolynomialThermo<E: EquationOfState, const N: usize> {
    eos: E,
    cps: Polynomial<N>,
    hf: f64, // heat of formation [J/kg]
    sf: f64, // specific entropy at T_std [J/(kg·K)]
    esi: Option<EsiOffsets<N>>,
}

/// The ESI-form enthalpy offset and entropy polynomial (see [`HPolynomialThermo`]).
#[derive(Debug, Clone, Copy)]
struct EsiOffsets<const N: usize> {
    /// Constant term of `hCoeffs` (`integral()` of Cp with its `[0]` offset).
    h0: f64,
    /// `sCoeffs = integralMinus1()` of Cp with its `[0]` offset.
    s_poly: Polynomial<N>,
}

impl<E: EquationOfState, const N: usize> HPolynomialThermo<E, N> {
    /// Construct a polynomial-Cp thermo model wrapping `eos`, with the Cp(T)
    /// polynomial `cps` `[J/(kg·K), T in K]`, heat of formation `hf` `[J/kg]`, and
    /// reference specific entropy `sf` `[J/(kg·K)]` at T_std = 298.15 K.
    pub fn new(eos: E, cps: Polynomial<N>, hf: AvailableEnergy, sf: SpecificHeatCapacity) -> Self {
        Self {
            eos,
            cps,
            hf: hf.get::<joule_per_kilogram>(),
            sf: sf.get::<joule_per_kilogram_kelvin>(),
            esi: None,
        }
    }

    /// Value of upstream's `hCoeffs = CpCoeffs.integral()` at `x` with constant
    /// term `c0`: the operations of `Polynomial::value` on the shifted
    /// coefficients `[c0, a0/1, a1/2, ...]`.
    fn int_poly_value(cps: &Polynomial<N>, c0: f64, x: f64) -> f64 {
        let mut val = c0;
        let mut pow_x = x;
        for (i, &a_i) in cps.coeffs().iter().enumerate() {
            val += a_i / (i as f64 + 1.0) * pow_x;
            pow_x *= x;
        }
        val
    }

    /// ESI v2506 convention (`hPolynomialThermo.C`, dictionary constructor):
    /// `Cp` coefficients `cps`, heat of formation `hf`, and the reference state
    /// `tref` (upstream default `Tstd`), `href` (default 0), `sref` (default 0).
    /// `Sf` and `Pref` are read upstream but unused, so they are not parameters.
    pub fn new_esi_v2506(
        eos: E,
        cps: Polynomial<N>,
        hf: AvailableEnergy,
        tref: ThermodynamicTemperature,
        href: AvailableEnergy,
        sref: SpecificHeatCapacity,
    ) -> Self {
        let tr = tref.get::<kelvin>();
        let pstd = Pressure::new::<pascal>(P_REF);
        let h_eos_ref = eos.h_eos(pstd, tref).get::<joule_per_kilogram>();
        let s_eos_ref = eos.s_eos(pstd, tref).get::<joule_per_kilogram_kelvin>();
        let h0 = 0.0
            + (href.get::<joule_per_kilogram>() - Self::int_poly_value(&cps, 0.0, tr) - h_eos_ref);
        let s_unshifted = cps.integral_minus1(0.0).value(tr);
        let s0 = 0.0 + (sref.get::<joule_per_kilogram_kelvin>() - s_unshifted - s_eos_ref);
        Self {
            eos,
            cps,
            hf: hf.get::<joule_per_kilogram>(),
            sf: 0.0,
            esi: Some(EsiOffsets { h0, s_poly: cps.integral_minus1(s0) }),
        }
    }
}

// --- EquationOfState delegation ---

impl<E: EquationOfState, const N: usize> EquationOfState for HPolynomialThermo<E, N> {
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

// --- ThermoModel ---

impl<E: EquationOfState, const N: usize> ThermoModel for HPolynomialThermo<E, N> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.cps.value(tv))
            + self.eos.cp_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        let tv = t.get::<kelvin>();
        if let Some(esi) = &self.esi {
            let h = Self::int_poly_value(&self.cps, esi.h0, tv);
            return AvailableEnergy::new::<joule_per_kilogram>(h) + self.eos.h_eos(p, t);
        }
        let ha_val = self.hf + self.cps.integral(T_STD, tv);
        AvailableEnergy::new::<joule_per_kilogram>(ha_val) + self.eos.h_eos(p, t)
    }

    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.ha(p, t) - self.hc()
    }

    fn hc(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.hf)
    }

    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>();
        if let Some(esi) = &self.esi {
            return SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(esi.s_poly.value(tv))
                + self.eos.s_eos(p, t);
        }
        let cp_int = self.cps.integral_minus1(0.0);
        let s_poly = cp_int.value(tv) - cp_int.value(T_STD);
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.sf + s_poly)
            + self.eos.s_eos(p, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;
    use approx::assert_relative_eq;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Constant-Cp air via a single-term polynomial — should behave identically
    /// to `HConstThermo` (with zero hf, sf, tref = T_STD, hsref = 0).
    fn air_const_poly() -> HPolynomialThermo<PerfectGas, 1> {
        HPolynomialThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97)),
            Polynomial::new([1004.0_f64]), // Cp = 1004 J/(kg·K)
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0),
        )
    }

    /// Linear Cp polynomial for air: Cp ≈ 1000 + 0.1·T  `[J/(kg·K)]`
    fn air_linear_poly() -> HPolynomialThermo<PerfectGas, 2> {
        HPolynomialThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97)),
            Polynomial::new([1000.0_f64, 0.1]),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0),
        )
    }

    #[test]
    fn const_poly_cp() {
        let a = air_const_poly();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(500.0);
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        assert_relative_eq!(cp, 1004.0, epsilon = 1e-10);
    }

    #[test]
    fn linear_poly_cp() {
        let a = air_linear_poly();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        assert_relative_eq!(cp, 1000.0 + 0.1 * 300.0, epsilon = 1e-8);
    }

    #[test]
    fn hs_plus_hc_equals_ha() {
        let a = air_linear_poly();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(600.0);
        let ha = a.ha(p, t).get::<joule_per_kilogram>();
        let hs_hc = (a.hs(p, t) + a.hc()).get::<joule_per_kilogram>();
        assert_relative_eq!(ha, hs_hc, epsilon = 1e-6);
    }

    #[test]
    fn ha_roundtrip() {
        let a = air_linear_poly();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_in = ThermodynamicTemperature::new::<kelvin>(700.0);
        let ha = a.ha(p, t_in);
        let t_out = a
            .t_from_ha(ha, p, ThermodynamicTemperature::new::<kelvin>(400.0))
            .unwrap();
        assert_relative_eq!(t_in.get::<kelvin>(), t_out.get::<kelvin>(), epsilon = 0.01);
    }

    #[test]
    fn entropy_increases_with_temperature() {
        let a = air_linear_poly();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(600.0);
        let s1 = a.s(p, t1).get::<joule_per_kilogram_kelvin>();
        let s2 = a.s(p, t2).get::<joule_per_kilogram_kelvin>();
        assert!(s2 > s1, "entropy must increase with T: s1={s1}, s2={s2}");
    }
}

#[cfg(test)]
mod gh1362_tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    #[test]
    fn esi_form_offsets_to_href_and_leaves_hf_out_of_ha() {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.96));
        let cps = Polynomial::<3>::new([1000.0, 0.1, -2.0e-5]);
        let hf = AvailableEnergy::new::<joule_per_kilogram>(1.0e5);
        let tstd = ThermodynamicTemperature::new::<kelvin>(298.15);
        let found = HPolynomialThermo::new(
            eos,
            cps,
            hf,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(200.0),
        );
        let esi = HPolynomialThermo::new_esi_v2506(
            eos,
            cps,
            hf,
            tstd,
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0),
        );
        let p = Pressure::new::<pascal>(1.0e5);
        // Foundation: Ha(Tstd) = Hf.  ESI: Ha(Tstd) = Href = 0, so Hs(Tstd) = -Hf.
        assert!((found.ha(p, tstd).get::<joule_per_kilogram>() - 1.0e5).abs() < 1e-6);
        assert!(esi.ha(p, tstd).get::<joule_per_kilogram>().abs() < 1e-6);
        assert!((esi.hs(p, tstd).get::<joule_per_kilogram>() + 1.0e5).abs() < 1e-6);
        // the two differ by exactly Hf in Ha, and agree in Cp and in dHa/dT
        let t2 = ThermodynamicTemperature::new::<kelvin>(500.0);
        let d = found.ha(p, t2).get::<joule_per_kilogram>() - esi.ha(p, t2).get::<joule_per_kilogram>();
        assert!((d - 1.0e5).abs() < 1e-6);
        assert_eq!(found.cp(p, t2), esi.cp(p, t2));
        // entropy: ESI zero at Tref (+ perfect-gas pressure part, zero at 1e5), Foundation sf
        assert!(esi.s(p, tstd).get::<joule_per_kilogram_kelvin>().abs() < 1e-9);
        assert!((found.s(p, tstd).get::<joule_per_kilogram_kelvin>() - 200.0).abs() < 1e-9);
    }
}
