use crate::thermophysics::imports::*;
use crate::thermophysics::constants::R_UNIVERSAL;
use crate::polynomial::Polynomial;
use super::traits::EquationOfState;

/// Incompressible polynomial EOS: `v(T) = poly(T)`, so `ρ = 1 / poly(T)`.
///
/// Mirrors `Foam::icoPolynomial<Specie, PolySize>` from
/// `src/thermophysicalModels/specie/equationOfState/icoPolynomial/`.
///
/// The polynomial gives specific volume as a function of T.  ψ = 0 (incompressible).
/// h_eos = p·v = p/ρ  (enthalpy departure for incompressible EOS).
#[derive(Debug, Clone, Copy)]
pub struct IcoPolynomial<const N: usize> {
    mol_weight: MolarMass,
    poly: Polynomial<N>,
}

impl<const N: usize> IcoPolynomial<N> {
    /// `poly` coefficients give specific volume [m³/kg] as a polynomial in T [K].
    pub fn new(mol_weight: MolarMass, poly: Polynomial<N>) -> Self {
        Self { mol_weight, poly }
    }
}

impl<const N: usize> EquationOfState for IcoPolynomial<N> {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }

    fn r(&self) -> SpecificHeatCapacity {
        let w = self.mol_weight.get::<kilogram_per_mole>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(R_UNIVERSAL / w)
    }

    fn rho(&self, _p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(1.0 / self.poly.value(t.get::<kelvin>()))
    }

    fn psi(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        MassDensity::new::<kilogram_per_cubic_meter>(0.0) / Pressure::new::<pascal>(1.0)
    }

    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio {
        // Z = p·v / (R·T) = p · poly(T) / (R·T)
        use uom::si::ratio::ratio;
        let p_v = p.get::<pascal>() * self.poly.value(t.get::<kelvin>());
        let r_t = self.r().get::<joule_per_kilogram_kelvin>() * t.get::<kelvin>();
        Ratio::new::<ratio>(p_v / r_t)
    }

    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        // h_eos = p/ρ = p · v = p · poly(T)
        AvailableEnergy::new::<joule_per_kilogram>(
            p.get::<pascal>() * self.poly.value(t.get::<kelvin>()),
        )
    }

    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(0.0)
    }

    fn s_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::pressure::pascal;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::available_energy::joule_per_kilogram;
    use approx::assert_relative_eq;

    /// Two-term specific-volume polynomial for water, valid near 300 K.
    /// Fit: v(T) ≈ 1/998.0 + 4e-7·(T−298.15)·(1/998.0)²  (simplified)
    /// Here we use a constant polynomial (single term) as a sanity check.
    fn water_const() -> IcoPolynomial<1> {
        // v = 1/998.0 m³/kg  (constant density ≈ RhoConst)
        IcoPolynomial::new(
            MolarMass::new::<gram_per_mole>(18.015),
            Polynomial::new([1.0_f64 / 998.0]),
        )
    }

    fn water_linear() -> IcoPolynomial<2> {
        // v(T) ≈ 1/998.0 + b·T  where b gives ~1% variation over 10 K
        // Small slope: b = 2e-7 m³/(kg·K)
        IcoPolynomial::new(
            MolarMass::new::<gram_per_mole>(18.015),
            Polynomial::new([1.0_f64 / 998.0, 2e-7]),
        )
    }

    #[test]
    fn constant_poly_density() {
        let w = water_const();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let rho = w.rho(p, t).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(rho, 998.0, epsilon = 0.01);
    }

    #[test]
    fn psi_is_zero() {
        let w = water_linear();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let psi_times_p = (w.psi(p, t) * p).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(psi_times_p, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn h_eos_equals_p_over_rho() {
        let w = water_linear();
        let p = Pressure::new::<pascal>(1e6);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let h_eos = w.h_eos(p, t).get::<joule_per_kilogram>();
        let p_over_rho = p.get::<pascal>() / w.rho(p, t).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(h_eos, p_over_rho, epsilon = 1e-6);
    }

    #[test]
    fn density_decreases_with_temperature() {
        let w = water_linear();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(310.0);
        let rho1 = w.rho(p, t1).get::<kilogram_per_cubic_meter>();
        let rho2 = w.rho(p, t2).get::<kilogram_per_cubic_meter>();
        assert!(rho1 > rho2, "rho should decrease with T for positive slope poly");
    }
}
