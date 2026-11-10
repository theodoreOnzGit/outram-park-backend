//! Pump-trip and flow-change tests of [`super::MsreModel`] (gh:#1332).
//! Predictions were posted on gh:#1332 before any of these ran; results are
//! in each test's doc and in `docs/msre_library.md`.

use super::tests::{f, model_at, temperatures};
use super::*;

/// ORNL-4233 Fig. 24 (report p. 53, ORNL-DWG 67-12323), "Control-Rod
/// Response to Fuel-Pump Startup and Coastdown": rod No. 1 position
/// (in. withdrawn) against time (s) while the flux servo held the reactor
/// critical at zero power. Digitised for gh:#1332 from a 300 dpi render of
/// the MSRE repository's scan, by eye, one point per second (axes 14-20
/// in., 0-70 s; reading uncertainty about ±0.03 in.). Coast-down: the
/// open circles.
const FIG24_COASTDOWN: [(f64, f64); 30] = [
    (0.0, 18.32),
    (1.0, 18.33),
    (2.0, 18.36),
    (3.0, 17.99),
    (4.0, 17.68),
    (5.0, 17.21),
    (6.0, 17.16),
    (7.0, 16.95),
    (8.0, 16.75),
    (9.0, 16.47),
    (10.0, 16.37),
    (11.0, 16.19),
    (12.0, 16.16),
    (13.0, 16.09),
    (14.0, 16.05),
    (15.0, 16.00),
    (16.0, 15.86),
    (17.0, 15.75),
    (18.0, 15.79),
    (19.0, 15.69),
    (20.0, 15.60),
    (25.0, 15.31),
    (30.0, 15.35),
    (35.0, 15.14),
    (40.0, 15.09),
    (45.0, 15.00),
    (50.0, 14.83),
    (55.0, 14.86),
    (60.0, 14.70),
    (65.0, 14.65),
];

/// The coast-down's final rod position: the mean of the points from 62 to
/// 70 s, read as 14.64-14.70 in.
const FIG24_COASTDOWN_FINAL: f64 = 14.66;

/// ORNL-4233 Fig. 24, start-up (filled points), digitised as above.
const FIG24_STARTUP: [(f64, f64); 25] = [
    (0.0, 14.49),
    (1.0, 14.44),
    (2.0, 14.55),
    (3.0, 14.36),
    (4.0, 14.90),
    (5.0, 15.32),
    (6.0, 15.88),
    (7.0, 16.40),
    (8.0, 16.93),
    (9.0, 17.56),
    (10.0, 18.04),
    (11.0, 18.65),
    (12.0, 19.08),
    (13.0, 19.56),
    (14.0, 19.55),
    (15.0, 19.53),
    (16.0, 19.02),
    (17.0, 18.54),
    (18.0, 18.63),
    (19.0, 18.59),
    (20.0, 18.48),
    (25.0, 18.28),
    (30.0, 18.22),
    (35.0, 18.47),
    (40.0, 18.40),
];

/// The start-up's final rod position, the mean of 21-46 s (18.35 in.).
const FIG24_STARTUP_FINAL: f64 = 18.35;

/// Fig. 24's scale bar: "~0.1 % δk/k" over 15.99 to 17.77 in.
const FIG24_PERCENT_PER_INCH: f64 = 0.1 / 1.78;

/// A zero-power model (10 W, as in ORNL's rod-servo tests) with `model`.
fn zero_power(model: CirculationModel) -> MsreModel {
    let mut c = MsreConfig::tm1070(Power::new::<megawatt>(1.0e-5));
    c.circulation_model = model;
    MsreModel::new(c).expect("MSRE model")
}

/// Step `m` for `seconds` with an ideal flux servo, returning the servo's
/// rod reactivity (δk/k) at each whole second, from 0.
fn servo_trace(m: &mut MsreModel, seconds: usize) -> Vec<f64> {
    let mut out = vec![m.rod_reactivity().get::<ratio>()];
    for _ in 0..seconds {
        for _ in 0..100 {
            let rod = m.critical_rod_reactivity();
            m.set_rod_reactivity(rod);
            m.step(Time::new::<second>(INTERNAL_TIMESTEP_S)).expect("step");
        }
        out.push(m.rod_reactivity().get::<ratio>());
    }
    out
}

/// **Validation, gh:#1332: the zero-power fuel-pump coast-down against
/// ORNL-4233 Fig. 24.**
///
/// Methodology: the model at 10 W, critical, fuel pump tripped with
/// ORNL-4233 Fig. 23's fuel-pump speed as the fuel flow fraction (an
/// assumption, `pumps` module docs); an ideal servo sets the rod each step
/// ([`MsreModel::critical_rod_reactivity`]), as ORNL's flux servo did.
/// Compared: the fraction of the final rod reactivity change reached at
/// each digitised time, against the measured fraction of rod travel (which
/// assumes a linear rod worth over 14.66-18.33 in., stated), and the total
/// change against Fig. 24's scale bar (3.67 in. × 0.1 %/1.78 in. ≈ 0.206 %,
/// about ±10 %). Both circulation models. Predictions (gh:#1332, before
/// running): fraction at 9 s 0.50 ± 0.10 (well-stirred) and 0.50 ± 0.12
/// (line); at 40 s 0.85 ± 0.05 and 0.85 ± 0.06.
///
/// Results (2026-11-10). Fraction of the final change (model normalised at
/// 600 s, measured at its last points, 62-70 s):
///
/// | | well-stirred | line | measured |
/// |---|---|---|---|
/// | total Δρ (% δk/k) | 0.2467 (+20 % on the scale bar) | 0.2291 (+11 %) | ≈ 0.206 |
/// | fraction at 5 s | 0.276 | 0.297 | 0.303 |
/// | fraction at 9 s | **0.443** (predicted 0.50 ± 0.10: met) | **0.475** (0.50 ± 0.12: met) | 0.505 |
/// | fraction at 20 s | 0.694 | 0.700 | 0.743 |
/// | fraction at 40 s | **0.842** (0.85 ± 0.05: met) | **0.845** (0.85 ± 0.06: met) | 0.883 |
/// | fraction at 60 s | 0.907 | 0.909 | 0.989 |
/// | half-way time | 11 s | 10 s | 9 s |
/// | rms against the 30 points | 0.053 | 0.047 | |
///
/// Both models lag the measurement slightly after 15 s. Part of that is the
/// normalisation: the measured trace stops at 70 s, before the longest group
/// (80 s mean life) has finished, so its "final" is not the asymptote.
/// Normalised at 65 s as the measurement is (a secondary instrument, chosen
/// after seeing the result and reported only as an explanation), the model
/// reads 0.483 / 0.516 at 9 s, 0.917 / 0.918 at 40 s, 0.988 / 0.988 at 60 s.
/// The largest difference is at 2 s (+0.11): the servo trace shows no motion
/// for the first 2 s (servo lag, or the fuel flow lagging the pump speed,
/// the assumption the fuel schedule rests on). The total is high by the
/// circulation-loss residual of gh:#1330, more for the well-stirred model.
#[test]
fn the_fuel_pump_coastdown_against_ornl4233_fig24() {
    let travel = FIG24_COASTDOWN[0].1 - FIG24_COASTDOWN_FINAL;
    let measured_total = travel * FIG24_PERCENT_PER_INCH;
    println!("measured: rod travel {travel:.2} in., total {measured_total:.3} % dk/k (scale bar)");
    for model in [CirculationModel::WellStirredTm1070, CirculationModel::LineTm1626] {
        let mut m = zero_power(model);
        let loss = m.circulation_loss().get::<ratio>();
        m.trip_fuel_pump(FlowSchedule::ornl4233_fuel_pump_speed_coastdown());
        let trace = servo_trace(&mut m, 600);
        let total = trace[0] - trace[600];
        let frac = |t: usize| (trace[0] - trace[t]) / total;
        let mut sq = 0.0;
        let mut worst: (f64, f64) = (0.0, 0.0);
        for &(t, x) in &FIG24_COASTDOWN {
            let measured = (FIG24_COASTDOWN[0].1 - x) / travel;
            let d = frac(t as usize) - measured;
            sq += d * d;
            if d.abs() > worst.1.abs() {
                worst = (t, d);
            }
        }
        let rms = (sq / FIG24_COASTDOWN.len() as f64).sqrt();
        let half = (0..=600).find(|&t| frac(t) >= 0.5).unwrap_or(600);
        println!(
            "{model:?}: total {:.4} % (loss {:.4} %; vs scale bar {:+.1} %), fraction at 9 s {:.3} (measured 0.505), 40 s {:.3} (measured 0.880), half at {half} s (measured 9 s), rms vs Fig. 24 {rms:.3}, worst {:+.3} at {} s; flow {:.3}",
            100.0 * total,
            100.0 * loss,
            100.0 * (100.0 * total / measured_total - 1.0),
            frac(9),
            frac(40),
            worst.1,
            worst.0,
            m.fuel_flow_fraction().get::<ratio>()
        );
        let frac65 = |t: usize| (trace[0] - trace[t]) / (trace[0] - trace[65]);
        println!(
            "  secondary (normalised at 65 s like the measured final): 9 s {:.3}, 40 s {:.3}, 60 s {:.3}",
            frac65(9),
            frac65(40),
            frac65(60)
        );
        for t in [2usize, 5, 9, 15, 20, 30, 40, 60] {
            let measured = FIG24_COASTDOWN.iter().find(|p| p.0 as usize == t).map(|p| (FIG24_COASTDOWN[0].1 - p.1) / travel);
            println!("  t {t:>2} s: model {:.3}, measured {:?}", frac(t), measured.map(|v| (v * 1000.0).round() / 1000.0));
        }
        // The servo inserts the whole circulation loss: the kinetics give it
        // back once the fuel stops (to the longest group's decay).
        assert!((total / loss - 1.0).abs() < 1e-3, "{total} vs {loss}");
        assert!(trace.iter().all(|r| r.is_finite()));
        assert!((m.power().get::<megawatt>() / 1.0e-5 - 1.0).abs() < 1e-9);
        assert_eq!(m.fuel_flow_fraction().get::<ratio>(), 0.0);
    }
}

/// **Validation, gh:#1332: the zero-power fuel-pump start-up against
/// ORNL-4233 Fig. 24.**
///
/// Methodology: from a stationary-fuel critical state (fuel flow 0 for
/// 1500 s under the servo), the fuel pump started with TM-0728 §5.4.2's
/// calculated flow ramp; the servo as above. Compared: the time of the
/// rod's largest withdrawal (the extremum before the recirculated
/// precursors return) and the overshoot above the final value as a
/// fraction of the total change, against Fig. 24 (13-15 s; (19.55 −
/// 18.35)/(18.35 − 14.49) = 0.31, about 0.07 % δk/k). Predictions
/// (gh:#1332): extremum at 17 ± 2 s (well-stirred) and 14 ± 2 s (line);
/// overshoot 0.03-0.10 % δk/k.
///
/// Results (2026-11-10):
///
/// | | well-stirred | line | measured |
/// |---|---|---|---|
/// | largest withdrawal at | **17 s** (predicted 17 ± 2: met) | **14 s** (14 ± 2: met) | 13-15 s |
/// | overshoot above final, % δk/k | **0.024** (0.03-0.10: **missed**, low) | **0.059** (met) | ≈ 0.068 |
/// | overshoot / total change | 0.10 | 0.26 | 0.31 |
///
/// The line model reproduces the timing and size of the recirculated
/// precursors' return ("the positive reactivity effect of the recirculated
/// precursors entering the core 13 sec after pump startup", ORNL-4233
/// p. 54); the well-stirred model returns them 3-4 s late (one loop transit,
/// 16.7 s, after they leave a core it treats as mixed) and with a third of
/// the overshoot, because a mixed core sends out its mean concentration at
/// once instead of the plug of high-concentration salt from the top of the
/// core.
#[test]
fn the_fuel_pump_startup_against_ornl4233_fig24() {
    let swing = FIG24_STARTUP_FINAL - FIG24_STARTUP[0].1;
    let peak = FIG24_STARTUP.iter().cloned().fold((0.0, 0.0), |a, p| if p.1 > a.1 { p } else { a });
    println!(
        "measured: swing {swing:.2} in. ({:.3} %), peak {:.2} in. at {} s, overshoot {:.2} of the swing ({:.3} %)",
        swing * FIG24_PERCENT_PER_INCH,
        peak.1,
        peak.0,
        (peak.1 - FIG24_STARTUP_FINAL) / swing,
        (peak.1 - FIG24_STARTUP_FINAL) * FIG24_PERCENT_PER_INCH
    );
    for model in [CirculationModel::WellStirredTm1070, CirculationModel::LineTm1626] {
        let mut m = zero_power(model);
        m.set_fuel_flow_fraction(Ratio::new::<ratio>(0.0)).expect("flow");
        servo_trace(&mut m, 1500);
        m.start_fuel_pump(FlowSchedule::tm0728_fuel_flow_startup());
        let trace = servo_trace(&mut m, 200);
        let (t_peak, r_peak) = trace.iter().enumerate().fold((0, f64::MIN), |a, (t, &r)| if r > a.1 { (t, r) } else { a });
        let total = trace[200] - trace[0];
        let overshoot = r_peak - trace[200];
        println!(
            "{model:?}: total {:.4} %, largest withdrawal at {t_peak} s, overshoot {:.4} % ({:.2} of the total); rod at 5/10/20/40 s as fraction: {:.2} {:.2} {:.2} {:.2}",
            100.0 * total,
            100.0 * overshoot,
            overshoot / total,
            (trace[5] - trace[0]) / total,
            (trace[10] - trace[0]) / total,
            (trace[20] - trace[0]) / total,
            (trace[40] - trace[0]) / total
        );
        assert!(trace.iter().all(|r| r.is_finite()));
        assert!(total > 0.0 && overshoot >= 0.0);
        assert_eq!(m.fuel_flow_fraction().get::<ratio>(), 1.0);
    }
}

/// **Energy balance across the core at reduced fuel flow, exact.** At fuel
/// flow `f` the steady core rise is `q τ_F / (f MC_F)`: the transit scaling
/// of the core lumps. Checked on the steady state of the network built at
/// fuel flow 0.5 (5 MW, 50 % air flow), to round-off; then the transient
/// route, a held flow change of the stepping model at 5 MW under the
/// servo, is printed after 4000 s. Harness check of the flow-dependent
/// network, not V&V.
///
/// Results (2026-11-10): steady state at fuel flow 0.5, core rise 56.400000
/// °F against 56.400000 °F (−5.6e-16). The stepping model after 4000 s is
/// within −7.7e-4 of it (graphite 954.8 °F against the steady 956.2 °F: the
/// plant level is still settling).
#[test]
fn the_core_rise_scales_with_one_over_the_fuel_flow() {
    let mut m = model_at(5.0);
    let hx = m.loop_parameters().hx;
    let rad = HxSection::radiator_at(5.0);
    let net = Network::at_flows(&hx, &rad, Flows { fuel: 0.5, coolant: 1.0 });
    let x = steady_state(&net, 5.0, 100.0).expect("steady state");
    // The core inlet is the HX fuel outlet (a pipe in steady state).
    let rise = x[F2] - x[HX + 4];
    let expected = 5.0 * TAU_F / (0.5 * MC_F);
    println!("steady state at fuel flow 0.5, 5 MW: core rise {rise:.9} F, q tau_F/(f MC_F) {expected:.9} F, ratio-1 {:e}", rise / expected - 1.0);
    assert!((rise / expected - 1.0).abs() < 1e-10);
    m.set_fuel_flow_fraction(Ratio::new::<ratio>(0.5)).expect("flow");
    servo_trace(&mut m, 4000);
    let rise_t = f(m.core_outlet_temperature()) - f(m.core_inlet_temperature());
    let q = m.heat_deposition().get::<megawatt>();
    println!(
        "stepping model after 4000 s at fuel flow 0.5: core rise {rise_t:.4} F against q tau_F/(f MC_F) {:.4} F ({:+.2e}); graphite {:.2} F, lumping valid {}",
        q * TAU_F / (0.5 * MC_F),
        rise_t / (q * TAU_F / (0.5 * MC_F)) - 1.0,
        f(m.graphite_temperature()),
        m.thermal_lumping_within_validity()
    );
    assert_eq!(m.fuel_flow_fraction().get::<ratio>(), 0.5);
    assert!(m.thermal_lumping_within_validity());
    assert!(m.set_fuel_flow_fraction(Ratio::new::<ratio>(-0.1)).is_err());
    assert!(m.set_coolant_flow_fraction(Ratio::new::<ratio>(f64::NAN)).is_err());
    m.set_coolant_flow_fraction(Ratio::new::<ratio>(0.5)).expect("flow");
    m.step(Time::new::<second>(0.01)).expect("step");
    assert_eq!(m.coolant_flow_fraction().get::<ratio>(), 0.5);
}

/// **Harness, gh:#1332: pump trips at 8 MW without a scram.** Fuel-pump
/// trip (Fig. 23 fuel-pump speed), coolant-pump trip (Fig. 23 measured
/// coolant flow), and the measured coolant-pump speed and `tampines`'
/// inertial coast-down as alternatives; 300 s each, no rod motion.
/// Checked: every state finite, the lumping-validity flag drops when the
/// fuel flow falls below 24 % (predicted), power and temperatures printed.
/// Prediction (gh:#1332): power falls after either trip. No measurement
/// exists to compare with; this is not V&V.
///
/// Results (2026-11-10), 300 s after the trip at 8 MW, no rod motion:
///
/// | trip | P at 300 s | peak P | lumping invalid from |
/// |---|---|---|---|
/// | fuel pump, Fig. 23 speed | 0.025 MW | 8.17 MW at 3 s | 4 s |
/// | fuel pump, inertial (round numbers) | 0.035 MW | 8.18 MW at 3 s | 2 s |
/// | coolant pump, Fig. 23 measured flow | 0.014 MW | 8.00 MW at 0 s | 12 s |
/// | coolant pump, Fig. 23 speed | 0.016 MW | 8.00 MW at 0 s | 15 s |
///
/// Prediction (power falls) met. After a fuel-pump trip the power first
/// rises 2 % (the precursors kept in the core), then the stagnant, uncooled
/// fuel heats (feedback −0.0061) and the reactor shuts itself down. **Not
/// modelled and shown by this run:** the coolant loop keeps losing heat
/// through the radiator at full air flow and reaches 471 °F, far below the
/// salts' 840 °F liquidus (TM-0728 Table 2.1); the plant would have closed
/// the radiator doors and the salt would freeze. After about 2-15 s every
/// case is outside the lumping's validity (module docs).
#[test]
fn pump_trips_at_power_stay_finite() {
    use tampines::components::pump_coastdown::InertialPumpCoastdown;
    use uom::si::angular_velocity::revolution_per_minute;
    use uom::si::f64::{AngularVelocity, MomentOfInertia, Torque};
    use uom::si::moment_of_inertia::kilogram_square_meter;
    use uom::si::torque::newton_meter;
    // Round numbers, not the MSRE pump (no inertia is published).
    let inertial = InertialPumpCoastdown::new(
        MomentOfInertia::new::<kilogram_square_meter>(2.0),
        AngularVelocity::new::<revolution_per_minute>(1150.0),
        Torque::new::<newton_meter>(400.0),
    )
    .expect("inertial");
    let cases: [(&str, bool, FlowSchedule); 4] = [
        ("fuel pump, Fig. 23 speed", true, FlowSchedule::ornl4233_fuel_pump_speed_coastdown()),
        ("coolant pump, Fig. 23 flow", false, FlowSchedule::ornl4233_coolant_salt_flow_coastdown()),
        ("coolant pump, Fig. 23 speed", false, FlowSchedule::ornl4233_coolant_pump_speed_coastdown()),
        ("fuel pump, inertial (round numbers)", true, FlowSchedule::Inertial(inertial)),
    ];
    for (name, fuel, schedule) in cases {
        let mut m = model_at(8.0);
        assert!(m.thermal_lumping_within_validity());
        if fuel {
            m.trip_fuel_pump(schedule);
        } else {
            m.trip_coolant_pump(schedule);
        }
        let mut invalid_at = None;
        let mut peak = (0.0, 8.0);
        for k in 1..=300 {
            m.step(Time::new::<second>(1.0)).expect("step");
            let p = m.power().get::<megawatt>();
            if p > peak.1 {
                peak = (k as f64, p);
            }
            if invalid_at.is_none() && !m.thermal_lumping_within_validity() {
                invalid_at = Some(k);
            }
            for t in temperatures(&m) {
                assert!(t.is_finite(), "{name}: temperature not finite at {k} s");
            }
            assert!(p.is_finite() && p > 0.0, "{name}: power {p} at {k} s");
        }
        println!(
            "{name}: P(300 s) {:.3} MW (peak {:.3} MW at {} s), fuel lump {:.1} F, graphite {:.1} F, core out {:.1} F, HX coolant out {:.1} F, feedback {:+.5}, flows {:.3}/{:.3}, lumping invalid from {:?} s",
            m.power().get::<megawatt>(),
            peak.1,
            peak.0,
            f(m.fuel_lump_temperature()),
            f(m.graphite_temperature()),
            f(m.core_outlet_temperature()),
            f(m.hx_coolant_outlet_temperature()),
            m.feedback_reactivity().get::<ratio>(),
            m.fuel_flow_fraction().get::<ratio>(),
            m.coolant_flow_fraction().get::<ratio>(),
            invalid_at
        );
    }
}

/// Cost per 0.01 s step of the line model, and of the default model during
/// a fuel-pump coast-down (the network is refactored every step while the
/// flow changes). Machine-dependent; printed, not asserted.
///
/// Result (2026-11-10, shared development container): line model 38 µs
/// per step (about 260 × real time); well-stirred during a coast-down
/// 4.8 µs per step (about 2100 ×), against 0.70 µs at constant flow.
#[test]
fn cost_of_the_line_model_and_of_a_coastdown_is_printed() {
    let mut c = MsreConfig::tm1070(Power::new::<megawatt>(8.0));
    c.circulation_model = CirculationModel::LineTm1626;
    let mut m = MsreModel::new(c).expect("model");
    let t = std::time::Instant::now();
    m.step(Time::new::<second>(100.0)).expect("step");
    let line_us = t.elapsed().as_secs_f64() * 1e6 / 10_000.0;
    let mut w = model_at(8.0);
    w.trip_fuel_pump(FlowSchedule::ornl4233_fuel_pump_speed_coastdown());
    let t = std::time::Instant::now();
    w.step(Time::new::<second>(12.0)).expect("step");
    let trip_us = t.elapsed().as_secs_f64() * 1e6 / 1_200.0;
    println!(
        "cost: line model {line_us:.2} us per step ({:.0}x real time); well-stirred during a coast-down {trip_us:.2} us per step ({:.0}x)",
        INTERNAL_TIMESTEP_S * 1e6 / line_us,
        INTERNAL_TIMESTEP_S * 1e6 / trip_us
    );
    assert!(m.power().get::<megawatt>().is_finite() && w.power().get::<megawatt>().is_finite());
}
