//! The steam dome's internals as a heat structure (gh:#1037, plan item F6).
//!
//! The gas leaving the top of the core passes the shroud head, the steam
//! separators and the dryer before it reaches the steam lines, the leak
//! paths and the containment. Those structures are tens of tonnes of steel
//! at saturation temperature, and they cool a superheated gas. MELCOR
//! models them as heat structures in the upper-plenum and dome volumes
//! (Vol. 2, SAND2023-10994 O, HS package; no source used); here they are one
//! lumped node.
//!
//! The gas (steam and hydrogen, heat-capacity rate `C_g`) leaves at
//!
//! ```text
//! T_out = T_s + (T_in - T_s) exp(-NTU),   NTU = h A / C_g
//! ```
//!
//! and the structure takes `C_g (T_in - T_out)`, which the vessel's energy
//! gives up (`Plant::finish`). When the gas is cooler than the structure,
//! the same exchange gives the heat back. The structure's enthalpy is
//! 304L's (`core::steel_h`), as for the steam lines.
//!
//! Its mass, area and heat transfer coefficient are all \[A\]
//! (`params::INTERNALS_*`): Unit 1's internals are not in a held source.
//! The ablation,
//! [`Mechanisms::without_dome_internals`](super::Mechanisms), passes the
//! core's exit gas straight through: the model before 2026-10-29.

use super::core::{steel_h, steel_t};
use super::params::Params;
use petir::mathf::RealMath;
use tampines_steam_tables::prelude::cp_tp_eqm_single_phase;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// The internals' temperature, K.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DomeInternals {
    pub t_k: f64,
}

impl DomeInternals {
    pub fn new(t_k: f64) -> Self {
        Self { t_k }
    }

    /// Pass a gas of `w_steam` and `w_h2` kg/s at `t_in` K (vessel pressure
    /// `p`; hydrogen's `c_p` `cp_h2`, J/(kg K)) for `dt` s. Returns the gas
    /// temperature out, K, and the heat the structure took, W (negative
    /// when it gives heat back).
    pub fn step(&mut self, prm: &Params, w_steam: f64, w_h2: f64, cp_h2: f64, t_in: f64, p: f64, dt: f64) -> (f64, f64) {
        let t_film = (0.5 * (t_in + self.t_k)).clamp(300.0, 2200.0);
        let cp_s = cp_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(t_film), Pressure::new::<pascal>(p.max(1.0e4)))
            .get::<joule_per_kilogram_kelvin>();
        let c_gas = w_steam * cp_s + w_h2 * cp_h2;
        if c_gas <= 0.0 {
            return (t_in, 0.0);
        }
        let ntu = prm.internals_h * prm.internals_area_m2 / c_gas;
        let t_out = self.t_k + (t_in - self.t_k) * (-ntu).r_exp();
        let q = c_gas * (t_in - t_out);
        self.t_k = steel_t(steel_h(self.t_k) + q * dt / prm.internals_kg);
        (t_out, q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NTU against a hand value, and the energy balance: with h A = 10 x
    /// 2500 = 25 kW/K and a 3 kg/s steam flow (c_p about 2.6 kJ/(kg K) near
    /// 1000 K film at 7 MPa), NTU is about 3.2 and the gas leaves within
    /// about 4 % of the structure's temperature; what the gas loses, the
    /// structure gains; no flow, no exchange.
    #[test]
    fn the_dome_internals_cool_the_gas_and_keep_the_heat() {
        let prm = Params::default();
        let mut s = DomeInternals::new(560.0);
        let h0 = steel_h(560.0);
        let (t_out, q) = s.step(&prm, 3.0, 0.0, 0.0, 1500.0, 7.0e6, 1.0);
        let frac = (t_out - 560.0) / (1500.0 - 560.0);
        assert!(frac > 0.02 && frac < 0.06, "{frac} {t_out}");
        assert!(((steel_h(s.t_k) - h0) * prm.internals_kg - q).abs() < 1e-6 * q, "{q}");
        let before = s;
        assert_eq!(s.step(&prm, 0.0, 0.0, 0.0, 1500.0, 7.0e6, 1.0), (1500.0, 0.0));
        assert_eq!(s, before);
    }
}
