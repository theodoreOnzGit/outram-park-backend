//! Tests of the operator session (gh:#1570). Harness checks, not physics V&V:
//! the physics is `MsreModel`'s, tested in `model/tests.rs`.

use super::*;
use crate::msre::model::CirculationModel;

/// The default session starts at 8 MW in steady state and stays there with
/// no command: the snapshot after a minute equals the first to 1e-6.
#[test]
fn an_untouched_plant_stays_in_steady_state() {
    let mut s = Session::new(SessionConfig::default()).unwrap();
    let a = s.snapshot();
    assert!((a.power_mw - 8.0).abs() < 1e-9, "{}", a.power_mw);
    let b = s.advance(MAX_ADVANCE_S).unwrap();
    for _ in 0..5 {
        s.advance(MAX_ADVANCE_S).unwrap();
    }
    let c = s.snapshot();
    assert!((b.time_s - MAX_ADVANCE_S).abs() < 1e-9);
    assert!((c.time_s - 6.0 * MAX_ADVANCE_S).abs() < 1e-6, "{}", c.time_s);
    assert!((c.power_mw / a.power_mw - 1.0).abs() < 1e-6, "{} vs {}", c.power_mw, a.power_mw);
    assert!((c.core_outlet_f - a.core_outlet_f).abs() < 1e-4, "{} vs {}", c.core_outlet_f, a.core_outlet_f);
    assert!(a.core_outlet_f > a.core_inlet_f, "the core heats the salt");
    assert_eq!(c.fuel_flow, 1.0);
    assert_eq!(c.servo, 0.0);
}

/// The headless CSV is deterministic (byte-identical on a second run), has
/// the stable header and the right number of rows, and stays in loose
/// physical bounds through the demo transient (rods +10 pcm, then a fuel
/// pump trip). Bounds are a harness check against divergence, not V&V.
#[test]
fn the_headless_demo_is_deterministic_and_bounded() {
    let cfg = SessionConfig::default();
    let a = headless_csv(cfg, &demo_script(), 1200.0, 10.0).unwrap();
    let b = headless_csv(cfg, &demo_script(), 1200.0, 10.0).unwrap();
    assert_eq!(a, b);
    let mut lines = a.lines();
    assert_eq!(lines.next().unwrap(), Snapshot::FIELDS.join(","));
    let rows: Vec<Vec<f64>> = lines.map(|l| l.split(',').map(|x| x.parse().unwrap()).collect()).collect();
    assert_eq!(rows.len(), 121);
    for r in &rows {
        let s = Snapshot::from_slice(r).unwrap();
        assert!(s.power_mw.is_finite() && s.power_mw >= 0.0 && s.power_mw < 50.0, "power {} at {}", s.power_mw, s.time_s);
        // The floor is the second law: nothing below the 100 °F air inlet,
        // the coldest sink. The ceiling is a loose divergence catch.
        for t in [s.core_inlet_f, s.core_outlet_f, s.fuel_f, s.graphite_f, s.hx_coolant_in_f, s.hx_coolant_out_f, s.radiator_coolant_out_f] {
            assert!(t >= AIR_INLET_F - 1e-6 && t < 1500.0, "temperature {t} °F at {} s", s.time_s);
        }
    }
    let at = |t: f64| Snapshot::from_slice(rows.iter().find(|r| (r[0] - t).abs() < 1e-6).unwrap()).unwrap();
    // The +10 pcm step (at 60 s) raises the power before feedback turns it
    // back: the peak in the next four minutes exceeds the steady 8 MW. (At
    // 8 MW the MSRE oscillates with a ~2 min natural period, TM-1647, so a
    // fixed sample time can sit in the swing below 8 MW.)
    let peak = rows.iter().filter(|r| r[0] > 60.0 && r[0] <= 300.0).map(|r| r[1]).fold(0.0, f64::max);
    assert!(peak > at(50.0).power_mw * 1.001, "peak {peak} vs {}", at(50.0).power_mw);
    assert!((at(120.0).rod_pcm - 10.0).abs() < 1e-9);
    // The fuel pump trip (at 600 s) has run the fuel flow down by the end.
    assert_eq!(at(590.0).fuel_flow, 1.0);
    assert!(at(1200.0).fuel_flow < 0.2, "{}", at(1200.0).fuel_flow);
    // Nothing is below the liquidus before the trip.
    assert_eq!(at(590.0).below_liquidus(), (false, false));
    assert!((at(1200.0).air_flow - 0.01).abs() < 1e-12);
}

/// Leaving the radiator at full air after a fuel pump trip takes the
/// coolant below its liquidus: what the GUI's freeze warning is for, and
/// why ORNL closed the radiator doors. The second-law floor still holds.
#[test]
fn a_trip_with_the_doors_open_freezes_the_coolant() {
    let csv = headless_csv(SessionConfig::default(), &[(0.0, Command::TripFuelPump)], 600.0, 600.0).unwrap();
    let last: Vec<f64> = csv.lines().last().unwrap().split(',').map(|x| x.parse().unwrap()).collect();
    let s = Snapshot::from_slice(&last).unwrap();
    assert!(s.below_liquidus().1, "{s:?}");
    assert!(s.hx_coolant_in_f >= AIR_INLET_F - 1e-6, "{s:?}");
}

/// The liquidus constants are the cited values, in °F.
#[test]
fn the_liquidus_constants_are_the_cited_ones() {
    assert_eq!(FUEL_LIQUIDUS_F, 840.0);
    assert!((COOLANT_LIQUIDUS_F - 856.4).abs() < 1e-9, "{COOLANT_LIQUIDUS_F}");
}

/// With the ideal flux servo on, more radiator air (more cooling) does not
/// change the power: the rods follow the feedback. Without it, the same
/// change raises the power (negative coefficients, colder salt).
#[test]
fn the_servo_holds_power_against_a_cooling_change() {
    let run = |servo: bool| {
        let mut s = Session::new(SessionConfig::default()).unwrap();
        s.apply(Command::Servo(servo)).unwrap();
        s.apply(Command::AirFraction(1.0)).unwrap();
        let mut last = s.snapshot();
        for _ in 0..12 {
            last = s.advance(MAX_ADVANCE_S).unwrap();
        }
        last
    };
    let held = run(true);
    let free = run(false);
    assert!((held.power_mw / 8.0 - 1.0).abs() < 1e-3, "servo power {}", held.power_mw);
    assert_eq!(held.servo, 1.0);
    assert!(free.power_mw > 8.05, "free power {}", free.power_mw);
    // A manual rod command turns the servo off.
    let mut s = Session::new(SessionConfig::default()).unwrap();
    s.apply(Command::Servo(true)).unwrap();
    s.apply(Command::RodsPcm(-5.0)).unwrap();
    assert_eq!(s.snapshot().servo, 0.0);
}

/// Out-of-range inputs are refused; `advance` caps and ignores bad spans.
#[test]
fn bad_inputs_are_refused_and_spans_are_capped() {
    let mut s = Session::new(SessionConfig::default()).unwrap();
    assert!(s.apply(Command::RodsPcm(f64::NAN)).is_err());
    assert!(s.apply(Command::AirFraction(0.0)).is_err());
    assert_eq!(s.advance(-3.0).unwrap().time_s, 0.0);
    assert_eq!(s.advance(f64::INFINITY).unwrap().time_s, 0.0);
    let t = s.advance(1e6).unwrap().time_s;
    assert!((t - MAX_ADVANCE_S).abs() < 1e-9, "{t}");
    assert!(headless_csv(SessionConfig::default(), &[], 10.0, 0.0).is_err());
}

/// Every pump command reaches the model: a trip runs the flow down, a start
/// from a trip brings it back toward full.
#[test]
fn pump_commands_reach_the_model() {
    let mut s = Session::new(SessionConfig::default()).unwrap();
    s.apply(Command::TripCoolantPump).unwrap();
    s.apply(Command::TripFuelPump).unwrap();
    for _ in 0..6 {
        s.advance(MAX_ADVANCE_S).unwrap();
    }
    let tripped = s.snapshot();
    assert!(tripped.fuel_flow < 0.5 && tripped.coolant_flow < 0.5, "{tripped:?}");
    s.apply(Command::StartFuelPump).unwrap();
    s.apply(Command::StartCoolantPump).unwrap();
    for _ in 0..6 {
        s.advance(MAX_ADVANCE_S).unwrap();
    }
    let started = s.snapshot();
    assert!(started.fuel_flow > tripped.fuel_flow && started.coolant_flow > tripped.coolant_flow, "{started:?}");
}

/// The snapshot crosses to the page and back unchanged.
#[test]
fn a_snapshot_round_trips_through_numbers() {
    let s = Session::new(SessionConfig::default()).unwrap().snapshot();
    let v = s.to_vec();
    assert_eq!(v.len(), Snapshot::FIELDS.len());
    assert_eq!(Snapshot::from_slice(&v), Some(s));
    assert_eq!(Snapshot::from_slice(&v[..5]), None);
}

/// The configuration picks the coefficients and the circulation model.
#[test]
fn the_configuration_reaches_the_model() {
    assert_eq!(Coefficients::Ornl1965.per_f(), (ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F));
    assert_eq!(Coefficients::MonteCarloN3.per_f(), (MC_N3_FUEL_PER_F, MC_N3_GRAPHITE_PER_F));
    let mc = SessionConfig { coefficients: Coefficients::MonteCarloN3, ..Default::default() };
    assert_eq!(mc.model_config().fuel_temperature_coefficient, per_f(MC_N3_FUEL_PER_F));
    let ws = SessionConfig { line_model: false, ..Default::default() };
    assert_eq!(ws.model_config().circulation_model, CirculationModel::WellStirredTm1070);
    assert_eq!(SessionConfig::default().model_config().circulation_model, CirculationModel::LineTm1626);
    let s = Session::new(ws).unwrap();
    assert_eq!(s.config(), ws);
}

/// The period table has TM-1647's seven powers, the measured column read
/// unchanged, and a model period at each with ORNL's 1965 coefficients
/// (the T-1 record: −11 % to +12 % on the old default's coefficients;
/// here only that the table is complete and positive is asserted).
#[test]
fn the_period_table_is_complete() {
    let rows = natural_periods(ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F);
    assert_eq!(rows.len(), MEASURED_PERIODS_TM1647.len());
    for (row, &(p, m)) in rows.iter().zip(MEASURED_PERIODS_TM1647.iter()) {
        assert_eq!((row.0, row.1), (p, m));
        let model = row.2.expect("a resonance at every TM-1647 power");
        assert!(model > 0.5 && model < 100.0, "{model} min at {p} MW");
    }
}
