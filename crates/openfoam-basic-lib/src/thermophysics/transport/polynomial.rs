use crate::thermophysics::imports::*;
use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::thermo::ThermoModel;
use crate::polynomial::Polynomial;
use super::traits::TransportModel;

/// Polynomial transport model: μ(T) and κ(T) evaluated from `Polynomial<N>`.
///
/// Mirrors `Foam::polynomialTransport<Thermo, PolySize>` from
/// `src/thermophysicalModels/specie/transport/polynomial/`.
///
/// Both mu and kappa are independent polynomials in T [K], returning Pa·s and
/// W/(m·K) respectively.  The same degree N is used for both.
#[derive(Debug, Clone)]
pub struct PolynomialTransport<T: ThermoModel, const N: usize> {
    thermo: T,
    mu_poly: Polynomial<N>,
    kappa_poly: Polynomial<N>,
}

impl<T: ThermoModel, const N: usize> PolynomialTransport<T, N> {
    pub fn new(thermo: T, mu_poly: Polynomial<N>, kappa_poly: Polynomial<N>) -> Self {
        Self { thermo, mu_poly, kappa_poly }
    }
}

// --- EquationOfState delegation ---

impl<T: ThermoModel, const N: usize> EquationOfState for PolynomialTransport<T, N> {
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

impl<T: ThermoModel, const N: usize> ThermoModel for PolynomialTransport<T, N> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.cp(p, t) }
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.ha(p, t) }
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.hs(p, t) }
    fn hc(&self) -> AvailableEnergy { self.thermo.hc() }
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.s(p, t) }
}

// --- TransportModel ---

impl<T: ThermoModel, const N: usize> TransportModel for PolynomialTransport<T, N> {
    fn mu(&self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(self.mu_poly.value(t.get::<kelvin>()))
    }

    fn kappa(&self, _p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity {
        ThermalConductivity::new::<watt_per_meter_kelvin>(self.kappa_poly.value(t.get::<kelvin>()))
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

    fn air_poly_transport() -> PolynomialTransport<HConstThermo<PerfectGas>, 2> {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        // Approx linear fit for air viscosity near 300 K:
        //   mu(T) ≈ -6e-6 + 5e-8·T  Pa·s   (gives ~9e-6 at 300 K, ~1.25e-5 at 350 K)
        //   kappa(T) ≈ -9e-3 + 7.5e-5·T  W/(m·K)
        PolynomialTransport::new(
            thermo,
            Polynomial::new([-6e-6_f64, 5e-8]),
            Polynomial::new([-9e-3_f64, 7.5e-5]),
        )
    }

    #[test]
    fn mu_at_300k() {
        let a = air_poly_transport();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let mu = a.mu(p, t).get::<pascal_second>();
        // From poly: -6e-6 + 5e-8 * 300 = -6e-6 + 15e-6 = 9e-6
        assert_relative_eq!(mu, 9e-6, epsilon = 1e-12);
    }

    #[test]
    fn kappa_at_300k() {
        let a = air_poly_transport();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let kappa = a.kappa(p, t).get::<watt_per_meter_kelvin>();
        // From poly: -9e-3 + 7.5e-5 * 300 = -9e-3 + 22.5e-3 = 13.5e-3
        assert_relative_eq!(kappa, 13.5e-3, epsilon = 1e-12);
    }

    #[test]
    fn mu_increases_with_temperature() {
        let a = air_poly_transport();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(400.0);
        assert!(
            a.mu(p, t1).get::<pascal_second>() < a.mu(p, t2).get::<pascal_second>(),
            "gas viscosity must increase with T"
        );
    }

    #[test]
    fn alpha_h_is_kappa_over_cp() {
        let a = air_poly_transport();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(350.0);
        let kappa = a.kappa(p, t).get::<watt_per_meter_kelvin>();
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        let alpha_expected = kappa / cp;
        use uom::si::dynamic_viscosity::pascal_second;
        let alpha_actual = a.alpha_h(p, t).get::<pascal_second>();
        assert_relative_eq!(alpha_actual, alpha_expected, epsilon = 1e-12);
    }
}
