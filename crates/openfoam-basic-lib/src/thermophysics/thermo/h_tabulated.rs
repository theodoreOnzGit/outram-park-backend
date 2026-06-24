use crate::thermophysics::imports::*;
use crate::thermophysics::eos::EquationOfState;
use crate::interpolation::interpolate_xy;
use super::traits::ThermoModel;

/// Tabulated thermodynamic model: Cp, Ha, and S stored as (T, value) lookup tables.
///
/// Mirrors `Foam::hTabulatedThermo<EOS>` from
/// `src/thermophysicalModels/specie/thermo/hTabulated/`.
///
/// All three tables use `interpolate_xy` (piecewise-linear, clamped at endpoints).
/// Separate temperature grids may be provided for each property.
///
/// `ha_table` should contain **absolute** enthalpy values (sensible + formation)
/// at each temperature.  `hc()` returns `hf` separately so that `hs = ha - hf`.
#[derive(Debug, Clone)]
pub struct HTabulatedThermo<E: EquationOfState> {
    eos: E,
    cp_ts:  Vec<f64>,  // temperature knots for Cp [K]
    cp_vs:  Vec<f64>,  // Cp values [J/(kg·K)]
    ha_ts:  Vec<f64>,  // temperature knots for Ha [K]
    ha_vs:  Vec<f64>,  // absolute enthalpy values [J/kg]
    s_ts:   Vec<f64>,  // temperature knots for S [K]
    s_vs:   Vec<f64>,  // entropy values [J/(kg·K)]
    hf:     f64,       // heat of formation = Ha at reference T [J/kg]
}

impl<E: EquationOfState> HTabulatedThermo<E> {
    /// Construct with separate (T, value) tables for Cp, Ha, and S.
    ///
    /// `ha_table` values must be absolute enthalpy (formation already included).
    /// `hf` is the formation enthalpy returned by `hc()` for the `hs = ha - hf` split.
    pub fn new(
        eos: E,
        cp_table:  (Vec<f64>, Vec<f64>),
        ha_table:  (Vec<f64>, Vec<f64>),
        s_table:   (Vec<f64>, Vec<f64>),
        hf: AvailableEnergy,
    ) -> Self {
        Self {
            eos,
            cp_ts: cp_table.0, cp_vs: cp_table.1,
            ha_ts: ha_table.0, ha_vs: ha_table.1,
            s_ts:  s_table.0,  s_vs:  s_table.1,
            hf: hf.get::<joule_per_kilogram>(),
        }
    }
}

// --- EquationOfState delegation ---

impl<E: EquationOfState> EquationOfState for HTabulatedThermo<E> {
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

impl<E: EquationOfState> ThermoModel for HTabulatedThermo<E> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
            interpolate_xy(tv, &self.cp_ts, &self.cp_vs),
        ) + self.eos.cp_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        let tv = t.get::<kelvin>();
        AvailableEnergy::new::<joule_per_kilogram>(
            interpolate_xy(tv, &self.ha_ts, &self.ha_vs),
        ) + self.eos.h_eos(p, t)
    }

    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.ha(p, t) - self.hc()
    }

    fn hc(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.hf)
    }

    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
            interpolate_xy(tv, &self.s_ts, &self.s_vs),
        ) + self.eos.s_eos(p, t)
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

    /// Constant-Cp air represented as a two-point table.
    /// ha(T) = Cp * (T - T_ref)  (T_ref = 298.15 K, Cp = 1004 J/(kg·K))
    /// S(T)  = Cp * ln(T / T_ref)
    fn air_tabulated() -> HTabulatedThermo<PerfectGas> {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let cp = 1004.0_f64;
        let t_ref = 298.15_f64;
        // Two temperature points: 200 K and 1000 K
        let ts = vec![200.0, 1000.0];
        let cps = vec![cp, cp];
        let has = vec![cp * (200.0 - t_ref), cp * (1000.0 - t_ref)];
        let ss  = vec![cp * (200.0_f64 / t_ref).ln(), cp * (1000.0_f64 / t_ref).ln()];
        HTabulatedThermo::new(
            eos,
            (ts.clone(), cps),
            (ts.clone(), has),
            (ts, ss),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        )
    }

    #[test]
    fn cp_from_table() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(600.0);
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        assert_relative_eq!(cp, 1004.0, epsilon = 1e-6);
    }

    #[test]
    fn ha_from_table_at_knot() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        // At T = 1000 K, ha = 1004 * (1000 - 298.15) = 704679.6 J/kg
        let t = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let ha = a.ha(p, t).get::<joule_per_kilogram>();
        let expected = 1004.0 * (1000.0 - 298.15);
        assert_relative_eq!(ha, expected, epsilon = 1e-4);
    }

    #[test]
    fn hs_plus_hc_equals_ha() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(500.0);
        let ha = a.ha(p, t).get::<joule_per_kilogram>();
        let hs_hc = (a.hs(p, t) + a.hc()).get::<joule_per_kilogram>();
        assert_relative_eq!(ha, hs_hc, epsilon = 1e-6);
    }

    #[test]
    fn entropy_increases_with_temperature() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(800.0);
        assert!(a.s(p, t1).get::<joule_per_kilogram_kelvin>() < a.s(p, t2).get::<joule_per_kilogram_kelvin>());
    }

    #[test]
    fn ha_roundtrip() {
        let a = air_tabulated();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_in = ThermodynamicTemperature::new::<kelvin>(700.0);
        let ha = a.ha(p, t_in);
        let t_out = a.t_from_ha(ha, p, ThermodynamicTemperature::new::<kelvin>(400.0));
        assert_relative_eq!(t_in.get::<kelvin>(), t_out.get::<kelvin>(), epsilon = 0.1);
    }
}
