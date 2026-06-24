use crate::thermophysics::imports::*;
use crate::thermophysics::constants::T_MIN;
use crate::thermophysics::eos::EquationOfState;

/// Per-species thermodynamic model — sensible/absolute enthalpy, entropy, and
/// Newton-iteration T-solvers.
///
/// Mirrors the `thermo` layer in
/// `src/thermophysicalModels/specie/thermo/thermo/`.
///
/// Implementors must provide `cp`, `ha`, `hs`, `hc`, `s`.
/// `cv`, `t_from_ha`, `t_from_hs`, and `t_from_e` have default implementations.
pub trait ThermoModel: EquationOfState {
    /// Specific heat at constant pressure Cp  [J/(kg·K)].
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// Absolute specific enthalpy (sensible + formation + EOS departure)  [J/kg].
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// Sensible specific enthalpy: `ha − hc`  [J/kg].
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy;

    /// Heat of formation (= chemical enthalpy at reference T)  [J/kg].
    fn hc(&self) -> AvailableEnergy;

    /// Specific entropy  [J/(kg·K)].
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;

    /// Specific heat at constant volume: Cv = Cp − cp_m_cv.
    fn cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        self.cp(p, t) - self.cp_m_cv(p, t)
    }

    /// Find T such that `ha(p, T) == ha_target`.  Newton iteration, max 50 steps.
    fn t_from_ha(
        &self,
        ha_target: AvailableEnergy,
        p: Pressure,
        t0: ThermodynamicTemperature,
    ) -> ThermodynamicTemperature {
        newton_t(
            |t| self.ha(p, ThermodynamicTemperature::new::<kelvin>(t)),
            |t| self.cp(p, ThermodynamicTemperature::new::<kelvin>(t)),
            ha_target.get::<joule_per_kilogram>(),
            t0.get::<kelvin>(),
        )
    }

    /// Find T such that `hs(p, T) == hs_target`.
    fn t_from_hs(
        &self,
        hs_target: AvailableEnergy,
        p: Pressure,
        t0: ThermodynamicTemperature,
    ) -> ThermodynamicTemperature {
        // hs = ha - hc  →  ha_target = hs_target + hc
        let ha_target = hs_target + self.hc();
        self.t_from_ha(ha_target, p, t0)
    }

    /// Find T such that internal energy `ea(p, T) == e_target`, where
    /// `ea = ha − p/ρ`.
    fn t_from_e(
        &self,
        e_target: AvailableEnergy,
        p: Pressure,
        t0: ThermodynamicTemperature,
    ) -> ThermodynamicTemperature {
        newton_t(
            |t| {
                let tt = ThermodynamicTemperature::new::<kelvin>(t);
                let rho = self.rho(p, tt);
                let ea = self.ha(p, tt) - p / rho;
                ea
            },
            |t| self.cv(p, ThermodynamicTemperature::new::<kelvin>(t)),
            e_target.get::<joule_per_kilogram>(),
            t0.get::<kelvin>(),
        )
    }
}

/// Shared Newton iteration for T-inversion.
///
/// Finds `T` such that `f(T) = target` using `dfdT` as the derivative.
/// Matches OpenFOAM's `species::thermo<T>::T()`.
fn newton_t(
    f: impl Fn(f64) -> AvailableEnergy,
    dfdT: impl Fn(f64) -> SpecificHeatCapacity,
    target: f64,
    t0: f64,
) -> ThermodynamicTemperature {
    const DTMAX: f64 = 500.0;
    const MAX_ITER: usize = 50;

    let mut t = t0.max(T_MIN);
    for _ in 0..MAX_ITER {
        let f_val = f(t).get::<joule_per_kilogram>();
        let cp_val = dfdT(t).get::<joule_per_kilogram_kelvin>();
        let dt = (-(f_val - target) / cp_val).clamp(-DTMAX, DTMAX);
        t += dt;
        if dt.abs() / t < 1e-6 {
            break;
        }
    }
    ThermodynamicTemperature::new::<kelvin>(t)
}
