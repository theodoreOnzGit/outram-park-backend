//! Tests of [`super::MsreModel`]. Predictions were posted on gh:#1330 before
//! any of these ran; results are recorded in each test's doc and in
//! `crates/outram-park-digital-twin-engine/docs/msre_library.md`.

use super::*;
use crate::msre::design::{measured_circulation_loss_tm1626, MsreDesignPoint, BETA, MEASURED_PERIODS_TM1647};
use petir::mathf::RealMath;
use uom::si::angular_velocity::radian_per_second;
use uom::si::f64::AngularVelocity;

pub(super) fn model_at(p_mw: f64) -> MsreModel {
    MsreModel::new(MsreConfig::tm1070(Power::new::<megawatt>(p_mw))).expect("MSRE model")
}

pub(super) fn f(t: ThermodynamicTemperature) -> f64 {
    t.get::<degree_fahrenheit>()
}

/// Every lump, in °F, for comparisons.
pub(super) fn temperatures(m: &MsreModel) -> [f64; 9] {
    [
        f(m.core_inlet_temperature()),
        f(m.core_outlet_temperature()),
        f(m.fuel_lump_temperature()),
        f(m.graphite_temperature()),
        f(m.hx_fuel_outlet_temperature()),
        f(m.hx_coolant_inlet_temperature()),
        f(m.hx_coolant_outlet_temperature()),
        f(m.radiator_coolant_outlet_temperature()),
        f(m.radiator_air_outlet_temperature()),
    ]
}

/// **Harness check, not V&V.** Built at 8 MW, the model is in steady state
/// and critical: over 300 s with no input, power holds to 1e-9 relative and
/// every temperature to 1e-8 °F; the net reactivity starts at zero.
#[test]
fn the_initial_state_is_steady_and_critical() {
    let mut m = model_at(8.0);
    assert_eq!(m.reactivity().get::<ratio>(), 0.0);
    let t0 = temperatures(&m);
    let mut worst_p: f64 = 0.0;
    let mut worst_t: f64 = 0.0;
    for _ in 0..300 {
        m.step(Time::new::<second>(1.0)).unwrap();
        worst_p = worst_p.max((m.power().get::<megawatt>() / 8.0 - 1.0).abs());
        for (a, b) in temperatures(&m).iter().zip(t0.iter()) {
            worst_t = worst_t.max((a - b).abs());
        }
    }
    println!("steady 300 s: worst |P/P0 - 1| = {worst_p:.3e}, worst |dT| = {worst_t:.3e} F");
    assert!(worst_p <= 1e-9);
    assert!(worst_t <= 1e-8);
    assert!((m.time().get::<second>() - 300.0).abs() < 1e-9);
}

/// **Energy balance across the core, exact.** In steady state the fuel's
/// rise across the core is `P τ_F / MC_F` (the two fuel lumps carry all the
/// power, the graphite's share returning through hA), to round-off, at
/// three powers. This is the core's energy conservation with the
/// TM-1070 heat-capacity rate (which `design` cross-checks against TUAS).
#[test]
fn the_core_rise_is_the_power_over_the_fuel_heat_capacity_rate() {
    for p in [0.5, 5.0, 10.0] {
        let m = model_at(p);
        let rise = f(m.core_outlet_temperature()) - f(m.core_inlet_temperature());
        let expected = p * TAU_F / MC_F;
        println!("{p} MW: core rise {rise:.9} F, P tau_F / MC_F = {expected:.9} F");
        assert!((rise / expected - 1.0).abs() < 1e-10);
        // At power the graphite is hotter than the fuel lump it heats.
        assert!(m.graphite_temperature() > m.fuel_lump_temperature());
    }
}

/// **MSRE N-4 number, end to end, uncalibrated.** The steady circulation
/// loss of the model is the analytic one with TM-1070's transits (τ_c
/// 8.46 s, τ_L 16.73 s) and App. B's groups.
///
/// Prediction (gh:#1330): 0.2467 % δk/k, the same equation as `nee_soon`'s
/// `circulation_loss`, to round-off; +16 % against the measured
/// 0.212 ± 0.004 % (ORNL-TM-1626).
///
/// Result (2026-11-10): **0.2467041 % δk/k**, equal to `nee_soon`'s
/// within the 1e-12 criterion; **+0.0347 % δk/k (+16.4 %, +8.7σ)** against the
/// measurement. As predicted. Not tuned: the lumped, flat-importance core
/// counts the plenum-born delayed neutrons as worthless and so reads high;
/// N-4 (gh:#1130) records 0.2301 % for the same model with TM-1626's
/// transits, and closing it needs spatial importance.
///
/// Also checked: in the circulating steady state the delayed-neutron
/// sources sum to `β − Δρ_circ`, to 1e-12.
#[test]
fn the_circulation_loss_is_tm1070s_and_reads_high_against_tm1626() {
    let m = model_at(8.0);
    let loss = m.circulation_loss().get::<ratio>();
    let nee = m.loop_parameters().circulation_loss();
    let measured = measured_circulation_loss_tm1626().get::<ratio>();
    println!(
        "circulation loss: model {:.7} %, nee_soon {:.7} %, measured {:.3} %: {:+.4} % dk/k ({:+.1} %)",
        100.0 * loss,
        100.0 * nee,
        100.0 * measured,
        100.0 * (loss - measured),
        100.0 * (loss / measured - 1.0)
    );
    assert!((loss / nee - 1.0).abs() < 1e-12);
    // Regression pin of the recorded value (not a criterion against the
    // measurement).
    assert!((100.0 * loss - 0.2467041).abs() < 1e-7);
    let beta: f64 = BETA.iter().sum();
    let sources: f64 = m.delayed_neutron_sources().iter().map(|r| r.get::<ratio>()).sum();
    println!("sum of delayed sources {sources:.9}, beta - loss {:.9}", beta - loss);
    assert!((sources - (beta - loss)).abs() < 1e-12);
}

/// **The new physics, uncalibrated: absolute temperatures.** At 10 MW with
/// the design air flow and TM-0728's 100 °F air inlet, the steady state
/// follows from TM-1070's lumps alone. It is compared with TM-0728's design
/// heat balance.
///
/// Prediction (gh:#1330): core outlet within ±75 °F of 1225 °F; radiator
/// air outlet within ±50 °F of 300 °F.
///
/// Result (2026-11-10), both predictions met:
///
/// | | model [°F] | TM-0728 [°F] | model − design [°F] |
/// |---|---|---|---|
/// | core inlet (fuel) | 1201.33 | 1175 | +26.33 |
/// | core outlet (fuel) | 1257.73 | 1225 | +32.73 |
/// | HX coolant inlet | 1068.46 | 1025 | +43.46 |
/// | HX coolant outlet | 1153.46 | 1100 | +53.46 |
/// | radiator air outlet | 309.75 | 300 | +9.75 |
/// | graphite (nuclear average, §2.4.1 estimate) | 1259.53 | ~1255 | +4.5 |
///
/// The whole plant sits 26-53 °F hot. Part is structural and known: TM-1070's
/// fuel heat-capacity rate gives a 56.4 °F core rise where TM-0728's heat
/// balance has 50 °F, and the HX coolant rises 85 °F against 75 °F. The
/// rest is the level the TM-1070 radiator coefficients put the coolant at
/// over a 100 °F air inlet. Not tuned: no coefficient was adjusted to move
/// the level. The values are pinned to 0.01 °F as a regression guard, not as
/// a criterion.
#[test]
fn the_uncalibrated_steady_state_against_the_tm0728_design_point() {
    let m = model_at(10.0);
    let d = MsreDesignPoint::tm0728();
    let rows = [
        ("core inlet (fuel)", f(m.core_inlet_temperature()), f(d.fuel_reactor_inlet)),
        ("core outlet (fuel)", f(m.core_outlet_temperature()), f(d.fuel_reactor_outlet)),
        ("HX coolant inlet", f(m.hx_coolant_inlet_temperature()), f(d.coolant_hx_inlet)),
        ("HX coolant outlet", f(m.hx_coolant_outlet_temperature()), f(d.coolant_hx_outlet)),
        ("radiator air outlet", f(m.radiator_air_outlet_temperature()), f(d.air_outlet)),
        // TM-0728 §2.4.1: the graphite's nuclear-average temperature is
        // "estimated to be about 1255 °F" (an estimate, not a design value).
        ("graphite (nuclear average)", f(m.graphite_temperature()), 1255.0),
    ];
    println!("| | model [F] | TM-0728 [F] | model - design [F] |");
    for (name, model, design) in rows {
        println!("| {name} | {model:.2} | {design:.0} | {:+.2} |", model - design);
    }
    println!(
        "fuel lump {:.2} F, graphite {:.2} F, HX fuel outlet {:.2} F",
        f(m.fuel_lump_temperature()),
        f(m.graphite_temperature()),
        f(m.hx_fuel_outlet_temperature())
    );
    let pins = PINNED_10MW_STEADY_STATE_F;
    for (got, pin) in temperatures(&m).iter().zip(pins.iter()) {
        assert!((got - pin).abs() < 0.01, "steady state moved: {got} vs pinned {pin}");
    }
}

/// The 10 MW steady state of [`the_uncalibrated_steady_state_against_the_tm0728_design_point`],
/// in the order of [`temperatures`], recorded 2026-11-10.
const PINNED_10MW_STEADY_STATE_F: [f64; 9] =
    [1201.3285, 1257.7285, 1229.5285, 1259.5285, 1201.3285, 1068.4561, 1153.4641, 1068.4561, 309.7466];

/// Drive the model at `p_mw` with rod reactivity `A sin(ωt)` and return
/// `(|δP/P₀|/A, phase [deg])` from a projection over whole periods after
/// `settle` seconds. `ω` is moved to the nearest period that is a whole
/// number of steps; the `ω` used is returned too.
fn driven_response(p_mw: f64, omega: f64, settle: f64, periods: usize) -> (f64, f64, f64) {
    let amp_rho = 1.0e-5;
    let steps_per_period = (2.0 * core::f64::consts::PI / omega / INTERNAL_TIMESTEP_S).round() as usize;
    let omega = 2.0 * core::f64::consts::PI / (steps_per_period as f64 * INTERNAL_TIMESTEP_S);
    let settle_steps = ((settle / INTERNAL_TIMESTEP_S).round() as usize / steps_per_period + 1) * steps_per_period;
    let mut m = model_at(p_mw);
    let dt = Time::new::<second>(INTERNAL_TIMESTEP_S);
    let (mut a, mut b) = (0.0, 0.0);
    let total = settle_steps + periods * steps_per_period;
    for k in 1..=total {
        let t = k as f64 * INTERNAL_TIMESTEP_S;
        m.set_rod_reactivity(Ratio::new::<ratio>(amp_rho * (omega * t).r_sin()));
        m.step(dt).unwrap();
        if k > settle_steps {
            let y = m.power().get::<megawatt>() / p_mw - 1.0;
            a += y * (omega * t).r_sin();
            b += y * (omega * t).r_cos();
        }
    }
    let n = (periods * steps_per_period) as f64;
    let (a, b) = (2.0 * a / n, 2.0 * b / n);
    ((a * a + b * b).sqrt() / amp_rho, b.r_atan2(a).to_degrees(), omega)
}

/// **Code-to-code: time domain against `nee_soon`'s frequency domain.**
///
/// Methodology: at 7.306 MW and 1.004 MW (two of TM-1647's measured
/// powers), drive the rod reactivity sinusoidally (1e-5 δk/k) at the
/// frequency of `nee_soon`'s resonance peak, let the start-up transient die
/// (3000 s at 7.3 MW, 5000 s at 1 MW), and project the power onto sin/cos
/// over four whole periods. Compare `|δP/P₀/δρ|` and phase with
/// `MsreLoopModel::amplitude_phase` at the same ω, same coefficients
/// (TM-1070's). The two share equations and coefficients, not code: this
/// checks the time integration, the absolute-temperature form, the
/// operator splitting and the nonlinearity together.
///
/// Prediction (gh:#1330, before running): within 2 % and 2°.
///
/// Result (2026-11-10), well inside the prediction:
///
/// | P [MW] | ω [rad/s] | time domain \|H\|, phase | `nee_soon` \|H\|, phase | ratio, Δphase |
/// |---|---|---|---|---|
/// | 7.306 | 0.061109 | 1219.79, +10.703° | 1219.45, +10.687° | 1.00027, +0.016° |
/// | 1.004 | 0.015466 | 5011.19, +19.361° | 5012.74, +19.374° | 0.99969, −0.013° |
///
/// The residual (3e-4) is the size expected of first-order backward Euler
/// and splitting at `ωΔt` ≈ 6e-4 plus the 1e-5 drive's nonlinearity; it was
/// not driven down further. The 2 % / 2° criterion stays as posted.
#[test]
fn the_time_domain_response_reproduces_the_frequency_response() {
    for (p, settle) in [(7.306, 3000.0), (1.004, 5000.0)] {
        let lin = MsreLoopModel::tm1070(
            Power::new::<megawatt>(p),
            per_f(crate::msre::design::ORNL_1965_FUEL_PER_F),
            per_f(crate::msre::design::ORNL_1965_GRAPHITE_PER_F),
        );
        let t_peak = lin.natural_period(400).expect("resonance").get::<second>();
        let (amp, phase, omega) = driven_response(p, 2.0 * core::f64::consts::PI / t_peak, settle, 4);
        let (amp_lin, phase_lin) = lin.amplitude_phase(AngularVelocity::new::<radian_per_second>(omega));
        println!(
            "{p} MW, omega {omega:.6} rad/s (period {:.2} s): time domain |H| {amp:.4}, phase {phase:+.3} deg; nee_soon |H| {amp_lin:.4}, phase {phase_lin:+.3} deg; ratio {:.5}, dphase {:+.3} deg",
            2.0 * core::f64::consts::PI / omega,
            amp / amp_lin,
            phase - phase_lin
        );
        assert!((amp / amp_lin - 1.0).abs() < 0.02);
        assert!((phase - phase_lin).abs() < 2.0);
    }
}

/// **MSRE T-1's number, end to end, with the right instrument.** TM-1647's
/// natural period is `2π/ω` at the peak of the measured frequency response,
/// so it is measured here the same way: the time-domain amplitude at five
/// frequencies around `nee_soon`'s peak (periods 0.85 to 1.15 of it), a
/// parabola in `ln ω` through the three highest, and its vertex.
///
/// Prediction (gh:#1330): within 2 % of `nee_soon`'s 1.71 min, hence within
/// a few percent of TM-1647's measured 1.71 min at 7.306 MW. (The free
/// oscillation after a step, the other instrument posted on gh:#1330, is
/// printed for the record; it is the damped period, not the resonance,
/// and is not the quantity TM-1647 measured.)
///
/// Result (2026-11-10): time domain **1.713 min**, `nee_soon` 1.714 min,
/// TM-1647 measured 1.71 min: **+0.2 %** against the measurement (the
/// digitised value is good to about ±2 %). With TM-1070's 1965 predicted
/// coefficients; with N-3's Monte Carlo coefficients `nee_soon` records
/// 2.03 min (+18.8 %) at this power (gh:#1132), which this model would
/// reproduce in the same way. One power only; the other six are
/// `nee_soon`'s record.
///
/// **A prediction missed, on the other instrument.** The free-oscillation
/// peak spacing after a +1e-5 step was predicted within ±15 % of 1.71 min;
/// it is **2.346 min (+37 %)**. The prediction was wrong, not the model:
/// the oscillation is heavily damped, so the spacing of the first two maxima
/// of a step response is neither the resonance period nor even a clean
/// damped period. It is kept as a printed record, not a criterion.
#[test]
fn the_natural_period_at_7_3_mw_against_tm1647() {
    let (p, measured_min) = MEASURED_PERIODS_TM1647[6];
    let lin = MsreLoopModel::tm1070(
        Power::new::<megawatt>(p),
        per_f(crate::msre::design::ORNL_1965_FUEL_PER_F),
        per_f(crate::msre::design::ORNL_1965_GRAPHITE_PER_F),
    );
    let t_lin = lin.natural_period(400).expect("resonance").get::<second>();
    let mut pts: Vec<(f64, f64)> = [0.85, 0.92, 1.0, 1.08, 1.15]
        .iter()
        .map(|s| {
            let (amp, _, omega) = driven_response(p, 2.0 * core::f64::consts::PI / (t_lin * s), 3000.0, 4);
            (omega.r_ln(), amp)
        })
        .collect();
    pts.sort_by(|x, y| y.1.total_cmp(&x.1));
    let top = &pts[..3];
    // Vertex of the parabola through the three highest points.
    let (x1, y1) = top[0];
    let (x2, y2) = top[1];
    let (x3, y3) = top[2];
    let den = (x1 - x2) * (x1 - x3) * (x2 - x3);
    let a = (x3 * (y2 - y1) + x2 * (y1 - y3) + x1 * (y3 - y2)) / den;
    let b = (x3 * x3 * (y1 - y2) + x2 * x2 * (y3 - y1) + x1 * x1 * (y2 - y3)) / den;
    let w_peak = (-b / (2.0 * a)).r_exp();
    let t_td = 2.0 * core::f64::consts::PI / w_peak / 60.0;
    // The other instrument, for the record: free oscillation after a step.
    let mut m = model_at(p);
    m.set_rod_reactivity(Ratio::new::<ratio>(1.0e-5));
    let mut trace = Vec::new();
    for _ in 0..60_000 {
        m.step(Time::new::<second>(INTERNAL_TIMESTEP_S)).unwrap();
        trace.push(m.power().get::<megawatt>());
    }
    let w = 1000; // ±10 s window for a local maximum
    let maxima: Vec<usize> = (w..trace.len() - w)
        .filter(|&k| trace[k - w..=k + w].iter().all(|&v| v <= trace[k]) && k > 500)
        .collect();
    let free = if maxima.len() >= 2 { Some((maxima[1] - maxima[0]) as f64 * INTERNAL_TIMESTEP_S / 60.0) } else { None };
    println!(
        "7.306 MW natural period: time domain {t_td:.3} min, nee_soon {:.3} min, TM-1647 measured {measured_min:.2} min ({:+.1} %); free-oscillation peak spacing {free:?} min",
        t_lin / 60.0,
        100.0 * (t_td / measured_min - 1.0)
    );
    assert!((t_td / (t_lin / 60.0) - 1.0).abs() < 0.02);
}

/// **Physical invariants in a load transient (harness check, not V&V).** At
/// 10 MW, the radiator air flow is cut to half. With the rods fixed the
/// temperatures rise, the negative coefficients pull the power down, and
/// the plant settles at a lower power. At every step: the fuel entering the
/// exchanger is hotter than the coolant leaving it and the fuel leaving it
/// hotter than the coolant entering (no heat against the gradient in the
/// counterflow exchanger), the coolant leaving the radiator is hotter than
/// the air leaving it, and every temperature stays between the air inlet and
/// 1500 °F. After 3000 s the power is below 10 MW and the net reactivity has
/// returned near zero.
#[test]
fn a_load_cut_lowers_the_power_without_breaking_the_second_law() {
    let mut m = model_at(10.0);
    m.set_radiator_air_flow_fraction(Ratio::new::<ratio>(0.5)).unwrap();
    assert_eq!(m.radiator_air_flow_fraction().get::<ratio>(), 0.5);
    for _ in 0..3000 {
        m.step(Time::new::<second>(1.0)).unwrap();
        assert!(f(m.hx_fuel_outlet_temperature()) > f(m.hx_coolant_inlet_temperature()));
        assert!(f(m.hx_fuel_inlet_temperature()) > f(m.hx_coolant_outlet_temperature()));
        assert!(f(m.radiator_coolant_outlet_temperature()) > f(m.radiator_air_outlet_temperature()));
        for t in temperatures(&m) {
            assert!(t > 100.0 && t < 1500.0, "{t} F");
        }
    }
    let p = m.power().get::<megawatt>();
    println!(
        "after 3000 s at half air flow: P = {p:.4} MW, heat {:.4} MW, rho = {:+.3e}, core out {:.2} F",
        m.heat_deposition().get::<megawatt>(),
        m.reactivity().get::<ratio>(),
        f(m.core_outlet_temperature())
    );
    assert!(p < 10.0 && p > 1.0);
    assert!(m.reactivity().get::<ratio>().abs() < 1e-5);
    assert!(m.feedback_reactivity().get::<ratio>().abs() < 1e-5);
}

/// A rod withdrawal raises the power at once (prompt jump) and the fuel
/// coefficient then turns it; the getters report the inputs and the
/// coefficients as set.
#[test]
fn a_rod_withdrawal_is_turned_by_feedback() {
    let mut cfg = MsreConfig::tm1070(Power::new::<megawatt>(5.0));
    cfg.fuel_temperature_coefficient = per_f(-3.82e-5);
    cfg.graphite_temperature_coefficient = per_f(-3.77e-5);
    let mut m = MsreModel::new(cfg).unwrap();
    assert!((m.fuel_temperature_coefficient().get::<per_kelvin>() - (-3.82e-5 * 1.8)).abs() < 1e-18);
    assert!((m.graphite_temperature_coefficient().get::<per_kelvin>() - (-3.77e-5 * 1.8)).abs() < 1e-18);
    m.set_rod_reactivity(Ratio::new::<ratio>(2.0e-4));
    assert_eq!(m.rod_reactivity().get::<ratio>(), 2.0e-4);
    m.step(Time::new::<second>(1.0)).unwrap();
    let early = m.power().get::<megawatt>();
    let mut peak = early;
    for _ in 0..600 {
        m.step(Time::new::<second>(1.0)).unwrap();
        peak = peak.max(m.power().get::<megawatt>());
    }
    println!("rod +20 pcm at 5 MW: 1 s {early:.3} MW, peak {peak:.3} MW, 601 s {:.3} MW, feedback {:+.3e}", m.power().get::<megawatt>(), m.feedback_reactivity().get::<ratio>());
    assert!(early > 5.0);
    assert!(m.feedback_reactivity().get::<ratio>() < 0.0);
    assert!(m.power().get::<megawatt>() < peak);
}

/// A lower air inlet temperature cools the plant; a steady state at a
/// colder inlet sits lower by the same amount everywhere (the network is
/// linear in temperature).
#[test]
fn the_air_inlet_shifts_the_plant_uniformly() {
    let base = model_at(8.0);
    let mut cfg = MsreConfig::tm1070(Power::new::<megawatt>(8.0));
    cfg.radiator_air_inlet = ThermodynamicTemperature::new::<degree_fahrenheit>(50.0);
    let cold = MsreModel::new(cfg).unwrap();
    for (a, b) in temperatures(&base).iter().zip(temperatures(&cold).iter()) {
        assert!(((a - b) - 50.0).abs() < 1e-8);
    }
    let mut m = model_at(8.0);
    m.set_radiator_air_inlet(ThermodynamicTemperature::new::<degree_fahrenheit>(50.0));
    m.step(Time::new::<second>(600.0)).unwrap();
    assert!(m.power().get::<megawatt>() > 8.0, "colder plant, negative coefficients: power rises");
}

/// **Determinism (headless rule):** the same inputs give bit-identical
/// states, and `step` carries the remainder: 0.004 s is none, and 0.006 s more makes one.
#[test]
fn stepping_is_deterministic_and_carries_the_remainder() {
    let run = || {
        let mut m = model_at(6.0);
        m.set_rod_reactivity(Ratio::new::<ratio>(5.0e-5));
        m.step(Time::new::<second>(123.456)).unwrap();
        m.set_radiator_air_flow_fraction(Ratio::new::<ratio>(0.7)).unwrap();
        m.step(Time::new::<second>(77.0)).unwrap();
        (m.power().get::<megawatt>().to_bits(), temperatures(&m).map(f64::to_bits))
    };
    assert_eq!(run(), run());
    let mut m = model_at(6.0);
    m.step(Time::new::<second>(0.004)).unwrap();
    assert_eq!(m.time().get::<second>(), 0.0);
    m.step(Time::new::<second>(0.006)).unwrap();
    assert!((m.time().get::<second>() - 0.01).abs() < 1e-15);
}

/// Inputs out of range are refused with an error, never a panic or a NaN.
#[test]
fn out_of_range_inputs_are_refused() {
    assert!(MsreModel::new(MsreConfig::tm1070(Power::new::<megawatt>(0.0))).is_err());
    let mut cfg = MsreConfig::tm1070(Power::new::<megawatt>(1.0));
    cfg.radiator_air_flow_fraction = Ratio::new::<ratio>(0.0);
    assert!(MsreModel::new(cfg).is_err());
    let mut m = model_at(1.0);
    assert!(m.step(Time::new::<second>(-1.0)).is_err());
    assert!(m.step(Time::new::<second>(f64::NAN)).is_err());
    assert!(m.set_radiator_air_flow_fraction(Ratio::new::<ratio>(-0.1)).is_err());
    assert!(whole_steps("lag", 0.015).is_err());
}

/// **Cost per step (measured, recorded; no criterion: timing is
/// machine-dependent).** 1000 s of plant time at 8 MW.
#[test]
fn cost_per_step_is_printed() {
    let mut m = model_at(8.0);
    let t = std::time::Instant::now();
    m.step(Time::new::<second>(1000.0)).unwrap();
    let us = t.elapsed().as_secs_f64() * 1e6 / 100_000.0;
    println!("cost: {us:.3} us per 0.01 s step ({:.0}x real time)", INTERNAL_TIMESTEP_S * 1e6 / us);
}
