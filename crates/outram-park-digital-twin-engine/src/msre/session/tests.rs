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

// ─── gh:#1573: the salt-freeze guard and the pacer ──────────────────────────

/// A plant tripped with the radiator doors open, run guarded in 2 s ticks.
/// The guard trips on the coolant salt, and returns the freeze it held at.
fn freeze_a_tripped_plant() -> (Session, Freeze, Snapshot) {
    let mut s = Session::new(SessionConfig::default()).unwrap();
    s.apply(Command::TripFuelPump).unwrap();
    let mut last = s.snapshot();
    for _ in 0..600 {
        match s.advance_guarded(2.0).unwrap() {
            Advance::Ran(snap) => last = snap,
            Advance::Frozen(f) => return (s, f, last),
        }
    }
    panic!("no freeze within 1200 s; last {last:?}");
}

/// The guard finds the freeze from the state, names the coolant salt at its
/// coldest lump, and holds the plant at the last state with every salt above
/// its liquidus: the snapshot is the last one the page saw, bit for bit,
/// and it is liquid. The refused temperature is below the liquidus.
#[test]
fn the_guard_holds_the_plant_at_the_last_liquid_state() {
    let (s, f, last) = freeze_a_tripped_plant();
    assert!(!f.location.is_fuel(), "{f:?}");
    assert!(matches!(f.location, FreezeLocation::RadiatorCoolantOutlet | FreezeLocation::HxCoolantInlet));
    assert!(f.temperature_f < COOLANT_LIQUIDUS_F, "{f:?}");
    let held = s.snapshot();
    assert_eq!(held, last, "held at the last state shown");
    assert_eq!(held.freeze(), None, "every salt above its liquidus: {held:?}");
    assert_eq!(held.below_liquidus(), (false, false));
    assert_eq!(f.held_at_s, held.time_s);
    assert!(f.headline().contains("coolant salt"), "{}", f.headline());
}

/// Held means held: more guarded advances report the same freeze and move
/// nothing.
#[test]
fn a_frozen_plant_does_not_move() {
    let (mut s, f, _) = freeze_a_tripped_plant();
    let held = s.snapshot();
    for _ in 0..3 {
        assert_eq!(s.advance_guarded(5.0).unwrap(), Advance::Frozen(f));
    }
    assert_eq!(s.snapshot(), held);
    assert_eq!(s.frozen(), Some(f));
}

/// The thaw is consumed once: it returns the plant to the configured hot
/// steady state (8 MW, both pumps, rods critical, no salt near freezing)
/// with the clock running on; a second thaw does nothing, and the plant
/// then runs.
#[test]
fn the_thaw_is_consumed_once_and_the_clock_runs_on() {
    let (mut s, _, _) = freeze_a_tripped_plant();
    let t = s.snapshot().time_s;
    assert!(s.thaw().unwrap());
    let a = s.snapshot();
    assert_eq!(a.time_s, t, "the clock carries on");
    assert!((a.power_mw - 8.0).abs() < 1e-9 && a.fuel_flow == 1.0 && a.coolant_flow == 1.0, "{a:?}");
    assert_eq!(a.rod_pcm, 0.0);
    assert_eq!(s.frozen(), None);
    assert!(!s.thaw().unwrap(), "nothing left to thaw");
    assert_eq!(s.snapshot(), a, "a second thaw changes nothing");
    match s.advance_guarded(2.0).unwrap() {
        Advance::Ran(b) => assert!((b.time_s - t - 2.0).abs() < 1e-9, "{} vs {t}", b.time_s),
        Advance::Frozen(f) => panic!("refroze at once: {f:?}"),
    }
}

/// An untouched plant runs guarded exactly as unguarded: the guard changes
/// nothing until a salt is about to freeze (bit-identical snapshots).
#[test]
fn the_guard_is_transparent_while_the_salt_is_liquid() {
    let mut a = Session::new(SessionConfig::default()).unwrap();
    let mut b = Session::new(SessionConfig::default()).unwrap();
    for c in [&mut a, &mut b] {
        c.apply(Command::RodsPcm(10.0)).unwrap();
    }
    for _ in 0..10 {
        let x = a.advance(3.0).unwrap();
        assert_eq!(b.advance_guarded(3.0).unwrap(), Advance::Ran(x));
    }
}

/// Every freeze location survives its wire code, and a bad code is refused.
#[test]
fn freeze_locations_round_trip() {
    for l in FreezeLocation::ALL {
        assert_eq!(FreezeLocation::from_code(l.code()), Some(l));
    }
    assert_eq!(FreezeLocation::from_code(4.0), None);
    assert_eq!(FreezeLocation::from_code(-1.0), None);
    assert_eq!(FreezeLocation::from_code(0.5), None);
}

/// The pacer: nothing on the first tick, speed × wall gap after it, the
/// gap capped at MAX_WALL_GAP_S (a late tick does not leap), nothing while
/// paused or for a clock that ran backwards, and pausing does not bank time.
#[test]
fn the_pacer_turns_wall_time_into_plant_time() {
    let mut p = Pacer::new(5.0, false);
    assert_eq!(p.plant_seconds(100.0), 0.0);
    assert!((p.plant_seconds(100.1) - 0.5).abs() < 1e-9);
    assert!((p.plant_seconds(130.0) - 5.0 * MAX_WALL_GAP_S).abs() < 1e-12, "a long gap is capped");
    assert_eq!(p.plant_seconds(129.0), 0.0, "backwards");
    p.paused = true;
    assert_eq!(p.plant_seconds(129.5), 0.0);
    p.paused = false;
    assert!((p.plant_seconds(129.6) - 0.5).abs() < 1e-9, "the pause banked nothing");
    p.speed = f64::NAN;
    assert_eq!(p.plant_seconds(129.7), 0.0);
}

/// gh:#1574: the measured split reaches the model like the others, and the
/// pair arithmetic reproduces the N-3 record's coefficients from its own
/// eigenvalues (fuel −3.79, graphite −3.26 × 10⁻⁵ /°F; record README).
#[test]
fn a_measured_split_reaches_the_plant_and_the_arithmetic_is_the_records() {
    let (f, sf) = coefficient_from_pair(1.02232, 0.00026, 1.01837, 0.00029, SPLIT_STEP_F);
    let (g, _) = coefficient_from_pair(1.02232, 0.00026, 1.01892, 0.00022, SPLIT_STEP_F);
    assert!((f * 1e5 + 3.79).abs() < 0.02, "{}", f * 1e5);
    assert!((g * 1e5 + 3.26).abs() < 0.02, "{}", g * 1e5);
    assert!(sf > 0.0 && sf * 1e5 < 0.6, "{}", sf * 1e5);
    let m = Coefficients::Measured { fuel_per_f: f, fuel_sigma: sf, graphite_per_f: g, graphite_sigma: 0.0 };
    assert_eq!(m.per_f(), (f, g));
    assert!(m.describe().contains("measured in this browser"));
    let s = Session::new(SessionConfig { coefficients: m, ..Default::default() }).unwrap();
    let c = s.config().model_config();
    assert_eq!(c.fuel_temperature_coefficient, crate::msre::design::per_f(f));
    assert_eq!(c.graphite_temperature_coefficient, crate::msre::design::per_f(g));
}

/// gh:#1574, "send to plant" online: the switch keeps the plant's state
/// (bit for bit at the moment of the switch), reports the new coefficients,
/// leaves the reactivity unchanged at the reference state, and from then on
/// the plant steps exactly as one built with those coefficients from the
/// same state. Away from the reference temperatures the reactivity steps by
/// Δα·(T − T_ref), which is checked by its sign after a cooling.
#[test]
fn coefficients_switch_online_and_keep_the_state() {
    let new = Coefficients::Measured { fuel_per_f: -6.0e-5, fuel_sigma: 0.5e-5, graphite_per_f: -2.0e-5, graphite_sigma: 0.5e-5 };
    // At the initial steady state: no reactivity step, the state kept.
    let mut a = Session::new(SessionConfig::default()).unwrap();
    let before = a.snapshot();
    a.apply(Command::Coefficients(new)).unwrap();
    let after = a.snapshot();
    assert_eq!(before, after, "the state is kept at the switch");
    assert_eq!(a.config().coefficients, new);
    let (f, g) = new.per_f();
    assert_eq!(a.model.fuel_temperature_coefficient(), crate::msre::design::per_f(f));
    assert_eq!(a.model.graphite_temperature_coefficient(), crate::msre::design::per_f(g));
    // Then it steps as a plant built with the new set from the start does
    // (both at the same steady state, so the same state).
    let mut b = Session::new(SessionConfig { coefficients: new, ..Default::default() }).unwrap();
    a.apply(Command::RodsPcm(15.0)).unwrap();
    b.apply(Command::RodsPcm(15.0)).unwrap();
    for _ in 0..6 {
        assert_eq!(a.advance(5.0).unwrap(), b.advance(5.0).unwrap());
    }
    // Away from the reference: cool the salt, switch to a more negative
    // fuel coefficient; the fuel is colder than its reference, so the
    // feedback becomes more positive at the switch.
    let mut c = Session::new(SessionConfig::default()).unwrap();
    c.apply(Command::AirFraction(1.0)).unwrap();
    for _ in 0..6 {
        c.advance(10.0).unwrap();
    }
    let s0 = c.snapshot();
    assert!(s0.fuel_f < at_reference_fuel_f(), "the salt cooled: {}", s0.fuel_f);
    c.apply(Command::Coefficients(new)).unwrap();
    let s1 = c.snapshot();
    assert!(s1.feedback_pcm > s0.feedback_pcm, "{} -> {}", s0.feedback_pcm, s1.feedback_pcm);
    assert_eq!((s1.time_s, s1.fuel_f, s1.power_mw), (s0.time_s, s0.fuel_f, s0.power_mw));
    assert!(c.apply(Command::Coefficients(Coefficients::Measured { fuel_per_f: f64::NAN, fuel_sigma: 0.0, graphite_per_f: 0.0, graphite_sigma: 0.0 })).is_err());
}

/// The default plant's fuel lump temperature at its steady state, °F.
fn at_reference_fuel_f() -> f64 {
    Session::new(SessionConfig::default()).unwrap().snapshot().fuel_f
}
