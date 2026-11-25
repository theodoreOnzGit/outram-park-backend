//! Tests of the EBR-II operator session (gh:#1741). Harness checks of the
//! bookkeeping and the headless trace, **not V&V**: the model's own V&V is
//! `ebr2::model_tests` and `docs/ebr2_library.md`.

use super::*;

/// Parse a headless CSV into its rows of numbers (header skipped).
fn rows(csv: &str) -> Vec<Vec<f64>> {
    csv.lines().skip(1).map(|l| l.split(',').map(|x| x.parse::<f64>().expect("a number")).collect()).collect()
}

fn col(name: &str) -> usize {
    Snapshot::FIELDS.iter().position(|f| *f == name).expect("a column")
}

/// The headless rule: the same config and script give byte-identical CSVs,
/// with a stable header and one row per sample. Loose physical bounds over
/// both whole tests (900 s, every 10 s), which catch divergence and are
/// **not validation**: every sodium node inside TUAS's 400-1000 K range
/// (127-727 °C, below boiling), power positive and finite, flow
/// non-negative and never above 110 % of the model's own initial flow (the
/// pumps only coast down), and the feedback terms summing to the total.
#[test]
fn the_headless_trace_is_deterministic_and_bounded() {
    for test in [ShrtTest::Shrt17, ShrtTest::Shrt45r] {
        let cfg = SessionConfig { test, ..Default::default() };
        let a = headless_csv(cfg, &demo_script(), 900.0, 10.0).expect("runs");
        let b = headless_csv(cfg, &demo_script(), 900.0, 10.0).expect("runs");
        assert_eq!(a, b, "{} not deterministic", test.name());
        assert_eq!(a.lines().next(), Some(Snapshot::FIELDS.join(",").as_str()));
        let r = rows(&a);
        assert_eq!(r.len(), 91);
        for row in &r {
            assert!(row.iter().all(|x| x.is_finite() || x.is_nan()), "{row:?}");
            for c in ["core_inlet_c", "core_mean_c", "core_outlet_c", "ihx_outlet_c"] {
                let t = row[col(c)];
                assert!((126.85..=726.85).contains(&t), "{} {c} = {t} °C at {} s", test.name(), row[0]);
            }
            assert!(row[col("total_mw")] > 0.0 && row[col("total_mw")].is_finite());
            assert!(row[col("primary_flow_kg_s")] >= 0.0);
            assert!(row[col("flow_fraction")] <= 1.1, "{row:?}");
            let s = Snapshot::from_slice(row).expect("a snapshot");
            let sum: f64 = s.reactivity_terms().iter().map(|t| t.1).sum();
            assert!((sum - s.rho_total_pcm).abs() < 1e-3, "{sum} vs {}", s.rho_total_pcm);
        }
        // The pumps coast down in both tests: the flow at 300 s is a small
        // fraction of the initial (the V&V record has 2-6 %).
        let at300 = r[30][col("flow_fraction")];
        assert!(at300 < 0.2, "{} flow at 300 s {at300}", test.name());
        // SHRT-17 is tabulated (no kinetics, no feedback); SHRT-45R is not.
        let k = r[0][col("kinetics")];
        assert_eq!(k, if test == ShrtTest::Shrt45r { 1.0 } else { 0.0 });
    }
}

/// The headless CSV refuses a bad sampling, and a scripted command lands at
/// its time.
#[test]
fn headless_rejects_bad_input_and_applies_its_script() {
    assert!(headless_csv(SessionConfig::default(), &[], 10.0, 0.0).is_err());
    assert!(headless_csv(SessionConfig::default(), &[], f64::NAN, 1.0).is_err());
    let csv = headless_csv(SessionConfig::default(), &[(5.0, Command::PumpSpeedRpm(Some(800.0)))], 60.0, 5.0).expect("runs");
    let r = rows(&csv);
    assert!(r[0][col("pump_override")] == 0.0 && r[1][col("pump_override")] == 1.0, "{:?}", &r[..2]);
    assert_eq!(r[12][col("pump1_rpm")], 800.0);
}

/// A snapshot and an initial comparison survive their wire form.
#[test]
fn snapshots_round_trip() {
    let s = Session::new(SessionConfig::default()).expect("a plant");
    let snap = s.snapshot();
    assert_eq!(Snapshot::from_slice(&snap.to_vec()), Some(snap));
    assert_eq!(snap.to_vec().len(), Snapshot::FIELDS.len());
    assert!(Snapshot::from_slice(&[0.0; 3]).is_none());
    let i = s.initial();
    assert_eq!(Initial::from_slice(&i.to_vec()), Some(i));
    assert!(Initial::from_slice(&[1.0]).is_none());
    // The cited SHRT-45R inlet, 650.8 °F, in °C; the model's own steady
    // state is computed, not forced (the V&V record: 621.36 K = 348.21 °C).
    assert!((i.cited_core_inlet_c - (650.8 - 32.0) / 1.8).abs() < 1e-9);
    assert!((i.model_core_inlet_c - 348.21).abs() < 0.05, "{}", i.model_core_inlet_c);
    for t in [ShrtTest::Shrt17, ShrtTest::Shrt45r] {
        assert_eq!(test_from_code(test_code(t)), t);
    }
}

/// Commands: the pump override holds the flow up where the table would coast
/// it down, and releasing it returns to the table; the intermediate flow
/// override is the input in force; external reactivity reaches the
/// reactivity sum on SHRT-45R and is refused on SHRT-17 (tabulated power);
/// bad values are refused, changing nothing.
#[test]
fn commands_reach_the_model() {
    let mut s = Session::new(SessionConfig::default()).expect("a plant");
    s.apply(Command::PumpSpeedRpm(Some(792.0))).expect("applies");
    for _ in 0..6 {
        s.advance(10.0).expect("steps");
    }
    let held_up = s.snapshot();
    assert_eq!(held_up.pump_override, 1.0);
    assert!(held_up.flow_fraction > 0.9, "{}", held_up.flow_fraction);
    // Release early in the coast-down (the table's SHRT-45R pump 1 is 511
    // rpm at 4.5 s, 398 at 7 s). Released at 60 s, the speed drops at once
    // from 792 to the table's ~53 rpm at full power: the model has no pump or
    // loop inertia, and a sodium node passes 1000 K within a second
    // (measured 2026-11-24; recorded on gh:#1741).
    let mut r = Session::new(SessionConfig::default()).expect("a plant");
    r.apply(Command::PumpSpeedRpm(Some(792.0))).expect("applies");
    r.advance(4.0).expect("steps");
    r.apply(Command::PumpSpeedRpm(None)).expect("applies");
    r.advance(1.0).expect("steps");
    let released = r.snapshot();
    assert_eq!(released.pump_override, 0.0);
    assert!((400.0..520.0).contains(&released.pump1_rpm), "the table's speed at 5 s: {}", released.pump1_rpm);

    s.apply(Command::IntermediateFlowKgS(Some(150.0))).expect("applies");
    s.advance(1.0).expect("steps");
    assert_eq!(s.snapshot().intermediate_flow_kg_s, 150.0);
    assert_eq!(s.snapshot().intermediate_override, 1.0);

    s.apply(Command::ExternalPcm(-50.0)).expect("applies");
    s.advance(0.5).expect("steps");
    assert!((s.snapshot().rho_external_pcm + 50.0).abs() < 1e-9);

    for bad in [Command::PumpSpeedRpm(Some(-1.0)), Command::IntermediateFlowKgS(Some(f64::NAN)), Command::ExternalPcm(f64::INFINITY)] {
        assert!(s.apply(bad).is_err(), "{bad:?}");
    }
    let mut s17 = Session::new(SessionConfig { test: ShrtTest::Shrt17, ..Default::default() }).expect("a plant");
    assert!(s17.apply(Command::ExternalPcm(10.0)).is_err());
    assert!(Session::new(SessionConfig { thermal_centre_height_m: 0.0, ..Default::default() }).is_err());
}

/// The guard: both pumps stopped dead at t = 0 at full power (no
/// coast-down; the model has no loop inertia, so the flow falls at once to
/// natural circulation) drives the sodium out of the model's 400-1000 K
/// range within seconds. (+1000 pcm with the pumps running does not: the
/// feedback limits it, measured 2026-11-24.) The guarded advance refuses the
/// failing piece, holds the plant at the last state that stepped (every node
/// still in range, at a time on the guard's 0.5 s grid), reports the hold on
/// every advance until a command, and a command (the pumps back on) lets it
/// run on.
#[test]
fn a_failed_step_holds_the_plant_until_a_command() {
    let mut s = Session::new(SessionConfig::default()).expect("a plant");
    s.apply(Command::PumpSpeedRpm(Some(0.0))).expect("applies");
    let mut hold = None;
    for _ in 0..20 {
        if let Advance::Held(h) = s.advance_guarded(10.0) {
            hold = Some(h);
            break;
        }
    }
    let h = hold.expect("the excursion is held within 200 s");
    let snap = s.snapshot();
    assert_eq!(snap.time_s, h.held_at_s);
    assert!(((h.held_at_s / GUARD_STRETCH_S).round() * GUARD_STRETCH_S - h.held_at_s).abs() < 1e-9, "{}", h.held_at_s);
    for t in [snap.core_inlet_c, snap.core_outlet_c, snap.ihx_outlet_c] {
        assert!((126.85..=726.85).contains(&t), "held state out of range: {t}");
    }
    assert!(h.headline().contains("Held at"));
    assert_eq!(s.advance_guarded(1.0), Advance::Held(h.clone()));
    assert_eq!(s.held(), Some(&h));
    s.apply(Command::PumpSpeedRpm(Some(792.0))).expect("applies");
    assert!(s.held().is_none());
    match s.advance_guarded(2.0) {
        Advance::Ran(after) => assert!((after.time_s - h.held_at_s - 2.0).abs() < 1e-9),
        Advance::Held(again) => panic!("still held with the pumps back on: {again:?}"),
    }
}

/// The pacer is MSRE's, re-exported: the first tick reads the clock, the
/// next runs speed times the wall gap.
#[test]
fn the_pacer_is_reused() {
    let mut p = Pacer::new(5.0, false);
    assert_eq!(p.plant_seconds(10.0), 0.0);
    assert!((p.plant_seconds(10.1) - 0.5).abs() < 1e-12);
    assert!(MAX_WALL_GAP_S > 0.0);
}
