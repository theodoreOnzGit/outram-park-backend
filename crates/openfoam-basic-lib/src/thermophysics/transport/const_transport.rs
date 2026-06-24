use uom::si::f64::{
    DynamicViscosity, MassDensity, MolarMass, Pressure, Ratio, AvailableEnergy,
    SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature,
};
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

use crate::thermophysics::eos::EquationOfState;
use crate::thermophysics::quantities::Compressibility;
use crate::thermophysics::thermo::ThermoModel;
use super::traits::TransportModel;

/// Constant-viscosity / constant-Prandtl-number transport model.
///
/// Mirrors `Foam::constTransport<Thermo>` from
/// `src/thermophysicalModels/specie/transport/const/`.
///
/// Fields: `mu_` (constant dynamic viscosity), `rPr_` (1/Pr, reciprocal Prandtl).
/// ```text
/// mu(p,T)    = mu_
/// kappa(p,T) = Cp(p,T) · mu_ / Pr  = Cp · mu_ · rPr_
/// alphah     = kappa / Cp = mu_ · rPr_       (default from TransportModel)
/// ```
#[derive(Debug, Clone)]
pub struct ConstTransport<T: ThermoModel> {
    thermo: T,
    mu: f64,   // [Pa·s]
    rpr: f64,  // 1/Pr (dimensionless)
}

impl<T: ThermoModel> ConstTransport<T> {
    pub fn new(thermo: T, mu: DynamicViscosity, pr: Ratio) -> Self {
        use uom::si::ratio::ratio;
        Self {
            thermo,
            mu: mu.get::<pascal_second>(),
            rpr: 1.0 / pr.get::<ratio>(),
        }
    }
}

// --- EquationOfState delegation ---

impl<T: ThermoModel> EquationOfState for ConstTransport<T> {
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

impl<T: ThermoModel> ThermoModel for ConstTransport<T> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.cp(p, t) }
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.ha(p, t) }
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.thermo.hs(p, t) }
    fn hc(&self) -> AvailableEnergy { self.thermo.hc() }
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.thermo.s(p, t) }
}

// --- TransportModel ---

impl<T: ThermoModel> TransportModel for ConstTransport<T> {
    fn mu(&self, _p: Pressure, _t: ThermodynamicTemperature) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(self.mu)
    }

    fn kappa(&self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity {
        use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
        let cp = self.thermo.cp(p, t).get::<joule_per_kilogram_kelvin>();
        ThermalConductivity::new::<watt_per_meter_kelvin>(cp * self.mu * self.rpr)
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
    use uom::si::ratio::ratio;
    use approx::assert_relative_eq;

    fn air() -> ConstTransport<HConstThermo<PerfectGas>> {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.0),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        ConstTransport::new(
            thermo,
            DynamicViscosity::new::<pascal_second>(1.82e-5), // air at 300 K
            Ratio::new::<ratio>(0.71),                       // Pr for air
        )
    }

    #[test]
    fn mu_is_constant() {
        let a = air();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(800.0);
        assert_relative_eq!(
            a.mu(p, t1).get::<pascal_second>(),
            a.mu(p, t2).get::<pascal_second>(),
            epsilon = 1e-12
        );
    }

    #[test]
    fn kappa_from_pr() {
        // κ = Cp·μ/Pr → check at 300 K
        let a = air();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        // κ ≈ 1004 × 1.82e-5 / 0.71 ≈ 0.0257 W/(m·K)
        let kappa = a.kappa(p, t).get::<watt_per_meter_kelvin>();
        assert_relative_eq!(kappa, 0.0257, epsilon = 0.002);
    }

    #[test]
    fn alpha_h_equals_kappa_over_cp() {
        use uom::si::dynamic_viscosity::pascal_second;
        let a = air();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let alpha = a.alpha_h(p, t).get::<pascal_second>();
        let kappa = a.kappa(p, t).get::<watt_per_meter_kelvin>();
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        assert_relative_eq!(alpha, kappa / cp, epsilon = 1e-10);
    }
}
