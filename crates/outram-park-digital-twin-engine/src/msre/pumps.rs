//! # MSRE pump flow schedules: coast-down and start-up (gh:#1332)
//!
//! A [`FlowSchedule`] gives a loop's flow, as a fraction of full flow, at a
//! time after a pump event. [`super::model::MsreModel`] reads it each step
//! (`trip_fuel_pump`, `trip_coolant_pump`, `start_fuel_pump`,
//! `start_coolant_pump`).
//!
//! ## Sources (read 2026-11-10 from the scans in the MSRE repository,
//! `literature/moltensalt-org/`)
//!
//! - **ORNL-4233** (Prince, Engel, Haubenreich et al., *Zero-Power Physics
//!   Experiments on the MSRE*, 1968), §5.6 "Transient Flow-Rate Tests",
//!   report pp. 51-53, **Fig. 23** "Pump Speed and Flow Coastdown
//!   Transients" (p. 53, ORNL-DWG 65-8942A): measured fuel-pump speed,
//!   coolant-pump speed and coolant-salt flow, 0-20 s. Digitised here from a
//!   300 dpi render: circle centres located by a pixel scan at the odd
//!   seconds and read by eye at the even seconds, axes from the grid lines;
//!   reading uncertainty about ±1 % of full scale. The coolant flow is the
//!   oscillograph trace (filled points) to 8 s, which ORNL says is direct
//!   (the logger "lag ... is due to the response characteristics of the
//!   EMF-to-current converter"), then the logger trace, which has merged
//!   with it by then.
//! - **ORNL-TM-0728** (Robertson, *MSRE Design and Operations Report Part
//!   I*, 1965) §5.4.2: "the flow startup times for the primary circulation
//!   system were estimated ... 50 % of full flow is attained after about
//!   3/4 sec, 75 % after 1-1/4 sec, 90 % after 1-3/4 sec, and 100 % is
//!   reached in about 3 sec" (a calculation, not a measurement). Read from
//!   this workspace's OCR
//!   (`crates/kovan-literature/generated/markdown/open/msre-design-and-operation.md`),
//!   whose digits are garbled ("5@", "759", "gCY$", "106"); **not re-checked
//!   against the scan.**
//!
//! ## What ORNL could not measure, and the assumption made here
//!
//! The fuel loop had no flowmeter. ORNL hoped the coolant flow would follow
//! the coolant-pump speed, so that the fuel flow could be read off the
//! fuel-pump speed; "this was not the case, however, and attempts to infer
//! the fuel-flow coastdown transient were abandoned" (ORNL-4233 p. 52).
//! [`FlowSchedule::ornl4233_fuel_pump_speed_coastdown`] nevertheless uses
//! the fuel-pump **speed** as the fuel **flow** fraction (the affinity law
//! at a fixed homologous point). It is an assumption, the only fuel-side
//! data there is, and Fig. 23's coolant curves show its size: the coolant
//! flow stays above the coolant-pump speed for the first 5 s (79.5 against
//! 45.6 % at 2 s) and falls below it after 6 s.
//!
//! No MSRE rotor inertia is given in TM-0728, ORNL-4233, ORNL-TM-2987 or
//! ORNL-TM-1070 (searched 2026-11-10), so the inertial model
//! (`tampines::components::pump_coastdown::InertialPumpCoastdown`, reused
//! through [`FlowSchedule::Inertial`]) takes it from the caller.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only.

use tampines::components::pump_coastdown::InertialPumpCoastdown;
use uom::si::f64::{Ratio, Time};
use uom::si::ratio::ratio;
use uom::si::time::second;

/// ORNL-4233 Fig. 23, fuel-pump speed during coast-down: `(t s, % of full)`.
pub const ORNL4233_FIG23_FUEL_PUMP_SPEED: [(f64, f64); 13] = [
    (0.0, 100.0),
    (1.0, 57.0),
    (2.0, 35.2),
    (3.0, 25.6),
    (4.0, 20.2),
    (5.0, 15.5),
    (6.0, 13.0),
    (7.0, 9.2),
    (8.0, 8.2),
    (9.0, 6.5),
    (10.0, 4.5),
    (11.0, 3.0),
    (12.0, 0.0),
];

/// ORNL-4233 Fig. 23, coolant-pump speed during coast-down: `(t s, %)`.
pub const ORNL4233_FIG23_COOLANT_PUMP_SPEED: [(f64, f64); 17] = [
    (0.0, 100.0),
    (1.0, 65.1),
    (2.0, 45.6),
    (3.0, 34.0),
    (4.0, 27.6),
    (5.0, 22.4),
    (6.0, 19.0),
    (7.0, 16.2),
    (8.0, 14.5),
    (9.0, 12.2),
    (10.0, 11.0),
    (11.0, 9.3),
    (12.0, 8.4),
    (13.0, 7.0),
    (14.0, 5.6),
    (15.0, 3.7),
    (16.0, 1.3),
];

/// ORNL-4233 Fig. 23, coolant-salt flow during coast-down: `(t s, %)`,
/// the oscillograph to 8 s, the logger after.
pub const ORNL4233_FIG23_COOLANT_SALT_FLOW: [(f64, f64); 18] = [
    (0.0, 100.0),
    (0.75, 98.5),
    (1.0, 96.0),
    (2.0, 79.5),
    (3.0, 57.5),
    (4.0, 41.8),
    (5.0, 29.0),
    (6.0, 15.8),
    (7.0, 12.8),
    (8.0, 12.0),
    (9.0, 9.2),
    (10.0, 8.1),
    (11.0, 5.7),
    (12.0, 4.5),
    (13.0, 3.6),
    (14.0, 2.6),
    (15.0, 1.5),
    (16.0, 1.0),
];

/// ORNL-TM-0728 §5.4.2, calculated fuel-flow start-up: `(t s, %)`.
pub const TM0728_FUEL_FLOW_STARTUP: [(f64, f64); 5] =
    [(0.0, 0.0), (0.75, 50.0), (1.25, 75.0), (1.75, 90.0), (3.0, 100.0)];

/// A flow history after a pump event, as a fraction of full flow.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowSchedule {
    /// A table `(t s, fraction)`, linear between points, the first value
    /// before the first point and the last value after the last.
    Table(Vec<(f64, f64)>),
    /// A tripped pump's inertial run-down; the flow is the speed ratio
    /// (affinity law), from `tampines`.
    Inertial(InertialPumpCoastdown),
}

impl FlowSchedule {
    /// A caller's table of `(time after the event, flow fraction)` points.
    /// Errors unless there is at least one point, times strictly increase
    /// from `t ≥ 0`, and every fraction is finite and `≥ 0`.
    pub fn table(points: &[(Time, Ratio)]) -> Result<Self, String> {
        let pts: Vec<(f64, f64)> = points.iter().map(|(t, f)| (t.get::<second>(), f.get::<ratio>())).collect();
        if pts.is_empty() {
            return Err("a flow table needs at least one point".to_string());
        }
        for (i, &(t, f)) in pts.iter().enumerate() {
            if !(t.is_finite() && t >= 0.0 && f.is_finite() && f >= 0.0) {
                return Err(format!("flow table point {i}: time and fraction must be finite and >= 0, got ({t}, {f})"));
            }
            if i > 0 && t <= pts[i - 1].0 {
                return Err(format!("flow table times must increase, point {i} at {t} s"));
            }
        }
        Ok(Self::Table(pts))
    }

    /// The fuel-pump speed coast-down of ORNL-4233 Fig. 23, used as the fuel
    /// flow fraction (an assumption: see the module docs).
    pub fn ornl4233_fuel_pump_speed_coastdown() -> Self {
        percent_table(&ORNL4233_FIG23_FUEL_PUMP_SPEED)
    }

    /// The coolant-pump speed coast-down of ORNL-4233 Fig. 23, as a flow
    /// fraction (the same assumption; the measured coolant flow,
    /// [`Self::ornl4233_coolant_salt_flow_coastdown`], is the better choice
    /// for the coolant loop).
    pub fn ornl4233_coolant_pump_speed_coastdown() -> Self {
        percent_table(&ORNL4233_FIG23_COOLANT_PUMP_SPEED)
    }

    /// The measured coolant-salt flow coast-down of ORNL-4233 Fig. 23.
    pub fn ornl4233_coolant_salt_flow_coastdown() -> Self {
        percent_table(&ORNL4233_FIG23_COOLANT_SALT_FLOW)
    }

    /// ORNL-TM-0728's calculated fuel-flow start-up (§5.4.2).
    pub fn tm0728_fuel_flow_startup() -> Self {
        percent_table(&TM0728_FUEL_FLOW_STARTUP)
    }

    /// The flow fraction `t` after the event.
    pub fn flow_fraction(&self, t: Time) -> Ratio {
        let t = t.get::<second>();
        let f = match self {
            Self::Table(p) => interpolate(p, t),
            Self::Inertial(c) => c.flow_ratio(Time::new::<second>(t)).get::<ratio>(),
        };
        Ratio::new::<ratio>(f.max(0.0))
    }
}

/// A table from `(t, %)` points.
fn percent_table(points: &[(f64, f64)]) -> FlowSchedule {
    FlowSchedule::Table(points.iter().map(|&(t, p)| (t, p / 100.0)).collect())
}

/// Linear interpolation, flat beyond the ends.
fn interpolate(p: &[(f64, f64)], t: f64) -> f64 {
    if t <= p[0].0 {
        return p[0].1;
    }
    for w in p.windows(2) {
        if t <= w[1].0 {
            let r = (t - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + r * (w[1].1 - w[0].1);
        }
    }
    p[p.len() - 1].1
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angular_velocity::revolution_per_minute;
    use uom::si::f64::{AngularVelocity, MomentOfInertia, Torque};
    use uom::si::moment_of_inertia::kilogram_square_meter;
    use uom::si::torque::newton_meter;

    fn at(s: &FlowSchedule, t: f64) -> f64 {
        s.flow_fraction(Time::new::<second>(t)).get::<ratio>()
    }

    /// The cited curves are transcribed as digitised, interpolate linearly,
    /// and hold their end values; a caller table is validated.
    #[test]
    fn the_schedules_reproduce_their_tables() {
        let fuel = FlowSchedule::ornl4233_fuel_pump_speed_coastdown();
        assert_eq!(at(&fuel, -1.0), 1.0);
        assert!((at(&fuel, 2.0) - 0.352).abs() < 1e-12);
        assert!((at(&fuel, 1.5) - 0.5 * (0.57 + 0.352)).abs() < 1e-12);
        assert_eq!(at(&fuel, 30.0), 0.0);
        assert!((at(&FlowSchedule::ornl4233_coolant_pump_speed_coastdown(), 4.0) - 0.276).abs() < 1e-12);
        assert!((at(&FlowSchedule::ornl4233_coolant_salt_flow_coastdown(), 2.0) - 0.795).abs() < 1e-12);
        let up = FlowSchedule::tm0728_fuel_flow_startup();
        assert_eq!(at(&up, 0.0), 0.0);
        assert!((at(&up, 0.75) - 0.5).abs() < 1e-12);
        assert_eq!(at(&up, 10.0), 1.0);
        let t = |s: f64| Time::new::<second>(s);
        let r = |f: f64| Ratio::new::<ratio>(f);
        let ok = FlowSchedule::table(&[(t(0.0), r(1.0)), (t(5.0), r(0.2))]).expect("valid");
        assert!((at(&ok, 2.5) - 0.6).abs() < 1e-12);
        assert!(FlowSchedule::table(&[]).is_err());
        assert!(FlowSchedule::table(&[(t(1.0), r(1.0)), (t(1.0), r(0.5))]).is_err());
        assert!(FlowSchedule::table(&[(t(0.0), r(-0.1))]).is_err());
    }

    /// The inertial schedule is `tampines`' coast-down, unchanged: the flow
    /// is its speed ratio, `1/(1 + t/τ)` without friction.
    #[test]
    fn the_inertial_schedule_is_tampines_coastdown() {
        // Round numbers, not the MSRE pump (whose inertia is not published).
        let c = InertialPumpCoastdown::new(
            MomentOfInertia::new::<kilogram_square_meter>(2.0),
            AngularVelocity::new::<revolution_per_minute>(1150.0),
            Torque::new::<newton_meter>(400.0),
        )
        .expect("valid");
        let tau = c.time_constant().get::<second>();
        let s = FlowSchedule::Inertial(c);
        assert!((at(&s, tau) - 0.5).abs() < 1e-12);
        assert!((at(&s, 3.0 * tau) - 0.25).abs() < 1e-12);
    }
}
