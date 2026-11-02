//! RCIC and HPCI: steam-turbine-driven pumps (gh:#1109), for Fukushima
//! Daiichi Units 2 and 3, which had them in place of Unit 1's isolation
//! condensers (USNRC HRTD, GE BWR/4 Advanced Technology, ch. 7.1, §7.1.2.1
//! "Units 2-5", p. 7.1-4: "a steam driven ... Reactor Core Isolation Cooling
//! (RCIC) System that is backed up by the steam driven High Pressure Coolant
//! Injection (HPCI) System ... RCIC is significantly smaller than HPCI").
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! ## The flow path (HRTD Figure 7.1-5a, "BWR 4 RCIC")
//!
//! Steam is drawn from the vessel's steam line through the steam-supply,
//! trip-throttle and turbine control valves into the turbine; the turbine's
//! exhaust line discharges under the suppression pool; the pump, on the
//! turbine's shaft, takes water from the condensate storage tank (or the
//! pool) and discharges it into the vessel through the feedwater line. A
//! flow element and a flow-indicating controller (FIC) set the turbine
//! control valve, so with DC power the pump is governed to a set flow; a
//! test line returns flow to the condensate storage tank.
//!
//! ## The model (physics-derived, its coefficients marked)
//!
//! A governed pump of rated flow `W_r` raises its water by `Δp = p_RPV -
//! p_suction`; its shaft work per kilogram is `w_p = Δp / (ρ η_p)`. The
//! turbine expands vessel steam from `h_g(p_RPV)` to the exhaust pressure
//! (the wetwell's plus the submergence head) and delivers `w_t = η_t Δh_s`
//! per kilogram of steam, `Δh_s` the isentropic drop from IF97 (`s(p, h)`
//! then `h(p, s)`, through `tampines_steam_tables`' checked interface). The
//! shaft balance fixes the turbine's steam: `W_s = W_p w_p / w_t`.
//!
//! - **Energy is conserved by construction.** The vessel gives up `W_in`
//!   at the inlet enthalpy; the exhaust leaves at that enthalpy less the
//!   shaft work per kilogram; the pumped water gains the shaft work (`h =
//!   u + p v` carries the pressure rise, the rest is friction heat), so
//!   the work returns to the vessel with the water it injects and to the
//!   storage tank with what the test line recirculates (stated: that share
//!   leaves the model).
//! - **Carry-over (the self-regulation, stated as a hypothesis).** When the
//!   swollen level reaches the steam line's nozzle the turbine draws a
//!   mixture: the steam fraction `f` falls linearly from 1 to 0 across
//!   [`Params::carryover_band_m`] above [`Params::msl_nozzle_above_taf_m`]
//!   (both \[A\]). The liquid does no work, so the pump's flow falls with
//!   `f`: the level then holds near the nozzle. This is one mechanism the
//!   literature proposes for Unit 2's RCIC running for about three days
//!   without DC power; the mechanism is not established, and nothing here
//!   was adjusted to a measured level.
//! - **Drives** ([`Drive`]): `Injecting` sends the whole flow to the vessel
//!   and, while DC lasts, trips at the high level L8; `Throttled` (the
//!   operators holding the level) runs the turbine at rated flow and sends
//!   to the vessel only what a proportional demand between L2 and L8 calls
//!   for, the rest round the test line. Without DC, `Throttled` falls back
//!   to `Injecting` with no trip (no controller, no trip logic).
//! - **Low steam pressure.** Below [`Params::turbine_min_pa`] \[A\] the
//!   turbine stops (isolation); it runs again above it.
//!
//! **Not in the model (stated):** suction from the torus (the water comes
//! from the storage tank at [`Params::injection_temperature_k`]; `tampines`
//! has no pool sink, gh:#1121); the turbine's speed dynamics and
//! the governor's own response (the flow is the governed flow at once);
//! the hydrogen in the vessel's gas (the turbine takes steam only); pump
//! head losses in the feedwater line; the exhaust's back-pressure trip.

use super::params::Params;
use tampines_steam_tables::prelude::checked::{try_h_ps_eqm, try_s_ph_eqm};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, Pressure};
use uom::si::pressure::pascal;

/// Cold water's density at the pump's suction, kg/m^3: IF97 at 293.15 K
/// and one atmosphere is 998.2 (the injection temperature's).
pub const RHO_SUCTION: f64 = 998.2;
/// Liquid water's `c_p`, J/(kg K), as the plant's injection enthalpy.
const CP_WATER: f64 = 4186.0;

/// Which system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pump {
    Rcic,
    Hpci,
}

impl Pump {
    /// Its rated (governed) flow, kg/s.
    pub fn rated_kg_s(self, prm: &Params) -> f64 {
        match self {
            Pump::Rcic => prm.rcic_rated_kg_s,
            Pump::Hpci => prm.hpci_rated_kg_s,
        }
    }
}

/// How the pump is being run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drive {
    /// Not running: never started, tripped, or lost.
    Off,
    /// The whole flow into the vessel; trips at L8 while DC lasts.
    Injecting,
    /// The operators hold the level: rated flow, the vessel's share by a
    /// proportional demand between L2 and L8, the rest round the test line.
    Throttled,
}

/// One turbine-driven pump's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurbinePump {
    pub pump: Pump,
    pub drive: Drive,
    /// Lost for good (could not be restarted): no later start applies.
    pub lost: bool,
    /// The plant time of its last automatic trip at L8, s (`None`: never).
    pub tripped_at_s: Option<f64>,
}

/// What a pump sees at the start of a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PumpInputs {
    /// The vessel's pressure, Pa, and its saturated enthalpies, J/kg.
    pub p_vessel_pa: f64,
    pub h_g: f64,
    pub h_f: f64,
    /// The collapsed level (the instrument's) and the swollen level, m
    /// above the top of active fuel.
    pub level_above_taf_m: f64,
    pub swollen_above_taf_m: f64,
    /// The turbine's exhaust pressure, Pa (the wetwell's plus the
    /// submergence head), and the pump's suction pressure, Pa.
    pub p_exhaust_pa: f64,
    pub p_suction_pa: f64,
    /// DC power for the governor, the level control and the trip logic.
    pub dc_power: bool,
    /// The vessel has boiled dry (no turbine steam).
    pub dry: bool,
}

/// One pump's flows over a step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PumpFlows {
    /// Water into the vessel, kg/s, and its specific enthalpy, J/kg (the
    /// storage tank's plus the shaft work per kilogram).
    pub inject_kg_s: f64,
    pub inject_h: f64,
    /// The pump's whole flow (into the vessel and round the test line),
    /// kg/s.
    pub pumped_kg_s: f64,
    /// The turbine's inlet from the vessel, kg/s (steam and any carried-over
    /// water), and its specific enthalpy, J/kg.
    pub turbine_kg_s: f64,
    pub turbine_h: f64,
    /// The exhaust's specific enthalpy, J/kg (into the pool, `turbine_kg_s`).
    pub exhaust_h: f64,
    /// Shaft power, W.
    pub shaft_w: f64,
    /// The steam fraction of the turbine's inlet (1 without carry-over).
    pub steam_fraction: f64,
}

/// The isentropic enthalpy drop of saturated steam at `h_g` from `p_in_pa`
/// to `p_out_pa`, J/kg, by IF97 (`s(p, h)`, then `h(p, s)`); `None` outside
/// the tables' range or for no expansion.
pub fn isentropic_drop(p_in_pa: f64, h_g: f64, p_out_pa: f64) -> Option<f64> {
    if !(p_in_pa > p_out_pa && p_out_pa > 0.0) {
        return None;
    }
    let s = try_s_ph_eqm(Pressure::new::<pascal>(p_in_pa), AvailableEnergy::new::<joule_per_kilogram>(h_g)).ok()?;
    let h_out = try_h_ps_eqm(Pressure::new::<pascal>(p_out_pa), s).ok()?;
    let dh = h_g - h_out.get::<joule_per_kilogram>();
    (dh > 0.0 && dh.is_finite()).then_some(dh)
}

/// The steam fraction of the turbine's inlet for the swollen level `z`, m
/// above the top of active fuel: 1 below the steam line's nozzle, falling
/// linearly to 0 across the carry-over band above it.
pub fn steam_fraction(prm: &Params, swollen_above_taf_m: f64) -> f64 {
    let band = prm.carryover_band_m.max(1e-6);
    (1.0 - (swollen_above_taf_m - prm.msl_nozzle_above_taf_m) / band).clamp(0.0, 1.0)
}

/// The operators' demand on a throttled pump: 1 at or below L2, 0 at or
/// above L8, linear between.
pub fn throttle_demand(prm: &Params, level_above_taf_m: f64) -> f64 {
    let span = (prm.level_l8_above_taf_m - prm.level_l2_above_taf_m).max(1e-6);
    ((prm.level_l8_above_taf_m - level_above_taf_m) / span).clamp(0.0, 1.0)
}

impl TurbinePump {
    /// A pump that has not been started.
    pub fn new(pump: Pump) -> Self {
        Self { pump, drive: Drive::Off, lost: false, tripped_at_s: None }
    }

    /// Start it in `drive` (an operator's or an automatic start); nothing
    /// once it is lost.
    pub fn start(&mut self, drive: Drive) {
        if !self.lost {
            self.drive = drive;
        }
    }

    /// Lost for good.
    pub fn lose(&mut self) {
        self.lost = true;
        self.drive = Drive::Off;
    }

    /// This step's flows (see the module docs).
    pub fn flows(&self, prm: &Params, inp: &PumpInputs) -> PumpFlows {
        if self.drive == Drive::Off || self.lost || inp.dry || inp.p_vessel_pa < prm.turbine_min_pa {
            return PumpFlows::default();
        }
        let Some(dh_s) = isentropic_drop(inp.p_vessel_pa, inp.h_g, inp.p_exhaust_pa) else {
            return PumpFlows::default();
        };
        let w_t = prm.turbine_efficiency * dh_s;
        let w_p = (inp.p_vessel_pa - inp.p_suction_pa).max(0.0) / (RHO_SUCTION * prm.pump_efficiency);
        let f = steam_fraction(prm, inp.swollen_above_taf_m);
        let pumped = self.pump.rated_kg_s(prm) * f;
        if pumped <= 0.0 || w_t <= 0.0 {
            return PumpFlows { steam_fraction: f, ..PumpFlows::default() };
        }
        // The inlet: the steam the shaft needs, and the water carried with
        // it (the inlet's mass held at the rated steam demand).
        let turbine = self.pump.rated_kg_s(prm) * w_p / w_t;
        let turbine_h = f * inp.h_g + (1.0 - f) * inp.h_f;
        let shaft = pumped * w_p;
        let to_vessel = match self.drive {
            Drive::Throttled if inp.dc_power => throttle_demand(prm, inp.level_above_taf_m),
            _ => 1.0,
        };
        let h_tank = CP_WATER * (prm.injection_temperature_k - 273.15);
        PumpFlows {
            inject_kg_s: pumped * to_vessel,
            inject_h: h_tank + w_p,
            pumped_kg_s: pumped,
            turbine_kg_s: turbine,
            turbine_h,
            exhaust_h: turbine_h - shaft / turbine,
            shaft_w: shaft,
            steam_fraction: f,
        }
    }
}

impl TurbinePump {
    /// The high-level trip (L8), checked at the start of a step at time
    /// `t_s`: an injecting pump trips at `prm`'s L8 while DC powers its
    /// logic. Returns whether it tripped now.
    pub fn check_trip(&mut self, prm: &Params, level_above_taf_m: f64, dc_power: bool, t_s: f64) -> bool {
        if self.drive == Drive::Injecting && dc_power && level_above_taf_m >= prm.level_l8_above_taf_m {
            self.drive = Drive::Off;
            self.tripped_at_s = Some(t_s);
            return true;
        }
        false
    }
}

/// The shaft balance's steam per kilogram pumped, `w_p / w_t`, for a vessel
/// at `p_vessel_pa` exhausting to `p_exhaust_pa` (a diagnostic the V&V doc
/// quotes; `None` where [`isentropic_drop`] is).
pub fn steam_per_kg_pumped(prm: &Params, p_vessel_pa: f64, h_g: f64, p_exhaust_pa: f64, p_suction_pa: f64) -> Option<f64> {
    let dh = isentropic_drop(p_vessel_pa, h_g, p_exhaust_pa)?;
    let w_p = (p_vessel_pa - p_suction_pa).max(0.0) / (RHO_SUCTION * prm.pump_efficiency);
    Some(w_p / (prm.turbine_efficiency * dh))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(_prm: &Params) -> PumpInputs {
        PumpInputs {
            p_vessel_pa: 7.0e6,
            h_g: 2.7726e6,
            h_f: 1.2673e6,
            level_above_taf_m: 4.0,
            swollen_above_taf_m: 4.5,
            p_exhaust_pa: 0.15e6,
            p_suction_pa: super::super::params::P_ATM,
            dc_power: true,
            dry: false,
        }
    }

    /// IF97's isentropic drop from saturated vapour at 7 MPa to 0.15 MPa.
    /// Prediction written before the run, by hand from steam-table values
    /// (h_g 2772.6 kJ/kg, s_g 5.8133 kJ/(kg K); at 0.15 MPa s_f 1.4337,
    /// s_fg 5.7894, h_f 467.1, h_fg 2226.0, so x 0.7565): 621.6 kJ/kg.
    /// Result (2026-11-02): see the printed value; within 5 kJ/kg.
    #[test]
    fn the_isentropic_drop_matches_a_hand_calculation() {
        let dh = isentropic_drop(7.0e6, 2.7726e6, 0.15e6).unwrap();
        println!("isentropic drop 7 -> 0.15 MPa: {:.1} kJ/kg", dh / 1e3);
        assert!((dh - 621.6e3).abs() < 5.0e3, "{dh}");
        assert!(isentropic_drop(0.1e6, 2.6e6, 0.2e6).is_none());
        let prm = Params::unit2();
        let r = steam_per_kg_pumped(&prm, 7.0e6, 2.7726e6, 0.15e6, 1.013e5).unwrap();
        println!("steam per kg pumped at 7 MPa: {r:.4}");
        assert!((0.025..0.04).contains(&r), "{r}");
    }

    /// The flows conserve energy: the turbine gives the shaft what the
    /// pump puts into its water; the inlet less the exhaust is the shaft.
    /// Carry-over halves the pump's flow at mid-band, and the throttled
    /// mode sends the proportional demand to the vessel.
    #[test]
    fn the_flows_conserve_energy_and_carry_over_limits_them() {
        let prm = Params::unit2();
        let mut p = TurbinePump::new(Pump::Rcic);
        assert_eq!(p.flows(&prm, &inputs(&prm)), PumpFlows::default());
        p.start(Drive::Injecting);
        let f = p.flows(&prm, &inputs(&prm));
        assert!((f.pumped_kg_s - prm.rcic_rated_kg_s).abs() < 1e-12 && f.inject_kg_s == f.pumped_kg_s);
        let w_p = (7.0e6 - 1.013e5) / (RHO_SUCTION * prm.pump_efficiency);
        assert!((f.shaft_w - f.pumped_kg_s * w_p).abs() < 1e-6 * f.shaft_w);
        assert!((f.turbine_kg_s * (f.turbine_h - f.exhaust_h) - f.shaft_w).abs() < 1e-6 * f.shaft_w);
        assert!((f.inject_h - (4186.0 * (prm.injection_temperature_k - 273.15) + w_p)).abs() < 1e-9);
        // Mid-band carry-over: half the steam, half the flow, the same inlet.
        let mid = PumpInputs { swollen_above_taf_m: prm.msl_nozzle_above_taf_m + 0.5 * prm.carryover_band_m, ..inputs(&prm) };
        let g = p.flows(&prm, &mid);
        assert!((g.steam_fraction - 0.5).abs() < 1e-12 && (g.pumped_kg_s - 0.5 * f.pumped_kg_s).abs() < 1e-9);
        assert!((g.turbine_kg_s - f.turbine_kg_s).abs() < 1e-12 && g.turbine_h < f.turbine_h);
        assert_eq!(steam_fraction(&prm, prm.msl_nozzle_above_taf_m + prm.carryover_band_m + 1.0), 0.0);
        // Throttled: half way between L2 and L8, half to the vessel.
        p.start(Drive::Throttled);
        let half = PumpInputs { level_above_taf_m: 0.5 * (prm.level_l2_above_taf_m + prm.level_l8_above_taf_m), ..inputs(&prm) };
        let h = p.flows(&prm, &half);
        assert!((h.inject_kg_s - 0.5 * h.pumped_kg_s).abs() < 1e-9 && h.shaft_w == f.shaft_w);
        assert_eq!(throttle_demand(&prm, prm.level_l2_above_taf_m - 1.0), 1.0);
        // Without DC the throttle is gone: the whole flow.
        let h = p.flows(&prm, &PumpInputs { dc_power: false, ..half });
        assert_eq!(h.inject_kg_s, h.pumped_kg_s);
        // Low steam pressure and a dry vessel stop it.
        assert_eq!(p.flows(&prm, &PumpInputs { p_vessel_pa: 0.4e6, ..inputs(&prm) }), PumpFlows::default());
        assert_eq!(p.flows(&prm, &PumpInputs { dry: true, ..inputs(&prm) }), PumpFlows::default());
        // HPCI is about ten times RCIC.
        let hp = TurbinePump { pump: Pump::Hpci, ..p };
        assert!(hp.flows(&prm, &PumpInputs { dc_power: false, ..inputs(&prm) }).pumped_kg_s > 10.0 * prm.rcic_rated_kg_s);
    }

    /// The L8 trip: an injecting pump trips with DC, not without; a
    /// throttled one never; a lost one cannot be started again.
    #[test]
    fn l8_trips_an_injecting_pump_only_while_dc_lasts() {
        let prm = Params::unit2();
        let l8 = prm.level_l8_above_taf_m;
        let mut p = TurbinePump::new(Pump::Rcic);
        p.start(Drive::Injecting);
        assert!(!p.check_trip(&prm, l8 - 0.01, true, 10.0));
        assert!(!p.check_trip(&prm, l8, false, 20.0) && p.drive == Drive::Injecting);
        assert!(p.check_trip(&prm, l8, true, 30.0) && p.drive == Drive::Off && p.tripped_at_s == Some(30.0));
        p.start(Drive::Throttled);
        assert!(!p.check_trip(&prm, l8 + 1.0, true, 40.0));
        p.lose();
        p.start(Drive::Injecting);
        assert!(p.lost && p.drive == Drive::Off);
    }
}
