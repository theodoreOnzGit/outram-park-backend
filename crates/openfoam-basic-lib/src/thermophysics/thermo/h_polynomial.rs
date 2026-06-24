use uom::si::f64::{MassDensity, MolarMass, Pressure, Ratio, AvailableEnergy, SpecificHeatCapacity, ThermodynamicTemperature};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::polynomial::Polynomial;
use crate::thermophysics::constants::T_STD;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::quantities::Compressibility;
use super::traits::ThermoModel;

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
#[derive(Debug, Clone)]
pub struct HPolynomialThermo<E: EquationOfState, const N: usize> {
    eos: E,
    cps: Polynomial<N>,
    hf: f64,  // heat of formation [J/kg]
    sf: f64,  // specific entropy at T_std [J/(kg·K)]
}

impl<E: EquationOfState, const N: usize> HPolynomialThermo<E, N> {
    pub fn new(
        eos: E,
        cps: Polynomial<N>,
        hf: AvailableEnergy,
        sf: SpecificHeatCapacity,
    ) -> Self {
        Self {
            eos,
            cps,
            hf: hf.get::<joule_per_kilogram>(),
            sf: sf.get::<joule_per_kilogram_kelvin>(),
        }
    }
}

// --- EquationOfState delegation ---

impl<E: EquationOfState, const N: usize> EquationOfState for HPolynomialThermo<E, N> {
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

impl<E: EquationOfState, const N: usize> ThermoModel for HPolynomialThermo<E, N> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.cps.value(tv))
            + self.eos.cp_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        let tv = t.get::<kelvin>();
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
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use approx::assert_relative_eq;

    /// Constant-Cp air via a single-term polynomial — should behave identically
    /// to `HConstThermo` (with zero hf, sf, tref = T_STD, hsref = 0).
    fn air_const_poly() -> HPolynomialThermo<PerfectGas, 1> {
        HPolynomialThermo::new(
            PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97)),
            Polynomial::new([1004.0_f64]),  // Cp = 1004 J/(kg·K)
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0),
        )
    }

    /// Linear Cp polynomial for air: Cp ≈ 1000 + 0.1·T  [J/(kg·K)]
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
        let t_out = a.t_from_ha(ha, p, ThermodynamicTemperature::new::<kelvin>(400.0));
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
