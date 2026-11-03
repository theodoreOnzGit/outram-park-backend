//! Tests of the HTR-10 coupled transient (gh:#1151), moved from
//! `tampines::htr10_primary_loop` on 2026-11-03 unchanged. Results are
//! recorded in `crates/nee_soon/verification_and_validation/htr10_loop_lofc/README.md`.

use uom::si::f64::{Length, Power, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::data::*;
use super::*;
use tampines::htr10_primary_loop::data::TabulatedSeries;
use tampines::htr10_primary_loop::{
    BypassSplit, CoastdownModel, CoreModel, LumpedPebbleBedCore,
};

fn k(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}
fn s(t: f64) -> Time {
    Time::new::<second>(t)
}

/// The embedded copies are the input sheet's files, byte for byte.
#[test]
fn data_copies_match_the_input_sheet() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reactor-scoping/htr10-th-data/");
    for (name, embedded) in [
        ("lofc-power-long-c09-fig6.csv", POWER_LONG_C09_FIG6),
        ("lofc-power-short-c09-fig4.csv", POWER_SHORT_C09_FIG4),
    ] {
        let sheet = std::fs::read_to_string(format!("{dir}{name}")).expect("sheet file");
        assert_eq!(sheet, embedded, "{name} drifted from the input sheet");
    }
}

#[test]
fn delayed_sets_and_bypass_splits() {
    use uom::si::ratio::ratio;
    let beta = uom::si::f64::Ratio::new::<ratio>(0.00726);
    let mut means = Vec::new();
    for set in [DelayedNeutronSet::KeepinU235Thermal, DelayedNeutronSet::TehOPrkeU235, DelayedNeutronSet::OneGroupKeepin] {
        let (b, l) = set.groups(beta);
        let sum: f64 = b.iter().map(|x| x.get::<ratio>()).sum();
        assert!((sum - 0.00726).abs() < 1e-15, "{set:?}");
        let m: f64 = b.iter().zip(l.iter()).map(|(b, l)| b.get::<ratio>() / l.get::<uom::si::frequency::hertz>()).sum();
        means.push(m);
    }
    println!("sum beta_i/lambda_i: Keepin {:.5} s, teh-o-prke {:.5} s, one-group {:.5} s", means[0], means[1], means[2]);
    assert!((means[0] - means[2]).abs() < 1e-12);
    for b in [BypassSplit::tecdoc1694(), BypassSplit::g02(), BypassSplit::s5_control_rods(), BypassSplit::no_leakage()] {
        assert!(LumpedPebbleBedCore::htr10(b, k(600.0)).is_ok());
    }
    let bad = BypassSplit { bed: 0.5, discharge_tube: 0.0, control_rods: 0.0, leakage: 0.0 };
    assert!(LumpedPebbleBedCore::htr10(bad, k(600.0)).is_err());
    let c = FeedbackCoefficients::g02_initial_core();
    assert!(c.moderator.value < 0.0 && FeedbackCoefficients::g02_equilibrium_core().reflector.value > 0.0);
}

/// Zero reactivity holds the power; xenon builds after a shutdown.
#[test]
fn kinetics_is_critical_at_reference_and_xenon_builds() {
    let refs = FeedbackTemperatures { fuel: k(900.0), moderator: k(850.0), reflector: k(500.0) };
    let mut kin = Htr10Kinetics::new(KineticsParameters::htr10_base(), Power::new::<watt>(3.315e6), refs).unwrap();
    let p0 = kin.total_power().get::<watt>();
    println!(
        "initial: total {p0:.1} W, fission {:.1} W, decay {:.1} W; equilibrium Xe worth {:.1} pcm",
        kin.fission_power().get::<watt>(),
        kin.decay_power().get::<watt>(),
        kin.equilibrium_xenon_worth() * 1e5
    );
    assert!((p0 - 3.315e6).abs() < 1e-3);
    for _ in 0..1000 {
        kin.step(s(0.1), &refs).unwrap();
    }
    assert!((kin.total_power().get::<watt>() / p0 - 1.0).abs() < 1e-9);
    // Cold shutdown: drop the reactivity by 1000 pcm through the moderator.
    let cold = FeedbackTemperatures { moderator: k(850.0 + 1000.0 / 16.2), ..refs };
    for _ in 0..3600 {
        kin.step(s(1.0), &cold).unwrap();
    }
    let r = kin.reactivity(&cold);
    println!("after 1 h shut down: relative Xe {:.4}, Xe reactivity {:.1} pcm", kin.relative_xenon(), r.xenon * 1e5);
    assert!(kin.relative_xenon() > 1.0 && r.xenon < 0.0);
    let mut off = KineticsParameters::htr10_base();
    off.xenon = XenonModel::Off;
    let k2 = Htr10Kinetics::new(off, Power::new::<watt>(3.315e6), refs).unwrap();
    assert_eq!(k2.equilibrium_xenon_worth(), 0.0);
    assert!(k2.delayed_mean_life_weight() > 0.0);
    assert_eq!(k2.parameters().xenon, XenonModel::Off);
}

/// Before the trip the coupled system is a steady state: no trip, 60 s,
/// flow and power unchanged.
#[test]
fn coupled_initial_state_is_steady() {
    let mut case = LofcCase::base().unwrap();
    case.loop_config.trip_time = None;
    let mut sim = CoupledTransient::new(&case).unwrap();
    let r0 = sim.initial_response();
    println!(
        "initial: head {:.1} Pa, SG outlet {:.3} C, circulator rise {:.4} K, outlet {:.2} C",
        sim.primary_loop().initial_head().get::<pascal>(),
        sim.primary_loop().sg_outlet_temperature().get::<kelvin>() - 273.15,
        sim.primary_loop().circulator_rise_kelvin(),
        r0.outlet_temperature.get::<kelvin>() - 273.15
    );
    let mut last = None;
    for _ in 0..600 {
        last = Some(sim.step(s(0.1)).unwrap());
    }
    let l = last.unwrap();
    println!("after 60 s: flow {:.6} kg/s, total {:.3} W", l.mass_flow_kg_s, l.total_power_w);
    assert!((l.mass_flow_kg_s / 1.457 - 1.0).abs() < 1e-4);
    assert!((l.total_power_w / 3.315e6 - 1.0).abs() < 1e-4);
    assert!(!sim.primary_loop().baffle_closed());
    assert!(sim.kinetics().relative_xenon() > 0.99);
    assert!(matches!(sim.core(), CoreModel::LumpedStandIn(_)));
    assert!(sim.initial_fission_power_w() > 0.0);
}

/// The flow follows the speed until the baffle closes at 12 s, and the
/// derived inertance does not matter at the coastdown time scale (x10,
/// a bound on the Unknown duct and SG lengths, moves the flow < 1 %).
#[test]
fn coastdown_flow_follows_speed_then_baffle_closes() {
    let run = |mult: f64| {
        let mut case = LofcCase::base().unwrap();
        case.loop_config.inertance_multiplier = mult;
        let mut sim = CoupledTransient::new(&case).unwrap();
        let mut out = Vec::new();
        for i in 1..=150 {
            let smp = sim.step(s(0.1)).unwrap();
            if i % 10 == 0 {
                out.push(smp);
            }
        }
        out
    };
    let a = run(1.0);
    let b = run(10.0);
    for x in &a {
        println!("t {:5.1} s  speed {:.4}  W/W0 {:.4}", x.time_s, x.speed, x.mass_flow_kg_s / 1.457);
    }
    for (x, y) in a.iter().zip(b.iter()).take(11) {
        assert!((x.mass_flow_kg_s - y.mass_flow_kg_s).abs() / 1.457 < 0.01, "inertance x10 at {} s", x.time_s);
        assert!((x.mass_flow_kg_s / 1.457 - x.speed).abs() < 0.03, "W/W0 vs speed at {} s", x.time_s);
    }
    assert_eq!(a.last().unwrap().mass_flow_kg_s, 0.0);
    let inert = CoastdownModel::Inertial { time_constant: s(49.0) };
    assert!((inert.speed(49.0) - 0.5).abs() < 1e-12 && inert.speed(-1.0) == 1.0);
    assert_eq!(CoastdownModel::Instantaneous.speed(1.0), 0.0);
    let j = CoastdownModel::j09_flow().unwrap();
    println!("J09 flow fraction at 10 s {:.3}, 60 s {:.3}", j.speed(10.0), j.speed(60.0));
}

/// With the baffle left open and the SG thermal centre placed above or
/// level with the RPV nozzles (both hypothetical: the elevation is
/// Unknown), the loop momentum balance settles to a natural-circulation
/// flow. Recorded, not gated.
#[test]
fn natural_circulation_with_the_baffle_open() {
    for h in [0.0, 5.0] {
        let mut case = LofcCase::base().unwrap();
        case.loop_config.baffle_close_after_trip = None;
        case.loop_config.isolation_after_trip = None;
        case.loop_config.sg_thermal_centre_height = Length::new::<meter>(h);
        case.duration = s(600.0);
        let (_, trace) = run_lofc(&case).unwrap();
        let at = |t: f64| trace.iter().find(|x| x.time_s >= t).unwrap();
        println!(
            "H_sg = {h} m: W(100 s) {:.4}, W(300 s) {:.4}, W(600 s) {:.4} kg/s; speed(600 s) {:.3}",
            at(100.0).mass_flow_kg_s,
            at(300.0).mass_flow_kg_s,
            trace.last().unwrap().mass_flow_kg_s,
            trace.last().unwrap().speed
        );
        assert!(trace.last().unwrap().mass_flow_kg_s.abs() < 1.457);
    }
}

fn summarise(name: &str, trace: &[TransientSample]) -> LofcMetrics {
    let m = lofc_metrics(trace);
    let p0 = trace[0].fission_power_w;
    println!(
        "| {name} | {} | {:.0} pcm @ {:.0} s | {} | {} | {} | {:.0} kW |",
        m.time_to_one_percent_s.map_or("-".into(), |t| format!("{t:.0} s")),
        m.min_reactivity.0 * 1e5,
        m.min_reactivity.1,
        m.reactivity_zero_crossing_s.map_or("none".into(), |t| format!("{t:.0} s")),
        m.power_return_s.map_or("none".into(), |t| format!("{t:.0} s")),
        m.first_peak.map_or("none".into(), |(v, t)| format!("{:.1} % @ {t:.0} s", 100.0 * v)),
        m.late_total_power_w / 1e3,
    );
    let _ = p0;
    m
}

/// **V&V: LOFC ATWS, base case, lumped stand-in.** Gate (written with the
/// predictions, before the first run): the time for the fission power to
/// fall to 1 % lies within 330 +- 120 s of the measured 330 s ([C09] Sec 5).
/// Every other metric is recorded, not gated (README Sec 1 says why).
#[test]
fn lofc_atws_base_case_against_the_measured_trace() {
    let case = LofcCase::base().unwrap();
    let (_, trace) = run_lofc(&case).unwrap();
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min |");
    let m = summarise(&case.name, &trace);
    // Short-term comparison against the digitised measured power.
    let short = TabulatedSeries::from_csv_columns(POWER_SHORT_C09_FIG4, 0, 1).unwrap();
    let long = TabulatedSeries::from_csv_columns(POWER_LONG_C09_FIG6, 0, 1).unwrap();
    let p0 = trace[0].fission_power_w;
    println!("t (s) | model P/P0 | measured P/P0 | fuel C | moderator C | reflector C | Xe pcm | rho pcm");
    for t in [10.0, 20.0, 30.0, 50.0, 75.0, 100.0, 150.0, 200.0, 300.0, 400.0] {
        let x = trace.iter().find(|x| x.time_s >= t).unwrap();
        println!(
            "{t:6.0} | {:.4} | {:.4} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1}",
            x.fission_power_w / p0,
            short.value_at(t),
            x.fuel_k - 273.15,
            x.moderator_k - 273.15,
            x.reflector_k - 273.15,
            x.reactivity.xenon * 1e5,
            x.reactivity.total() * 1e5
        );
    }
    for t in [1000.0, 2000.0, 3000.0, 4000.0, 4400.0, 5000.0, 6000.0, 7200.0, 9000.0, 10_800.0] {
        let x = trace.iter().find(|x| x.time_s >= t - 1e-9).unwrap_or(trace.last().unwrap());
        println!(
            "{t:6.0} | {:.4} | {:.4} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | decay {:.1} kW, RCCS {:.1} kW",
            x.fission_power_w / p0,
            long.value_at(t),
            x.fuel_k - 273.15,
            x.moderator_k - 273.15,
            x.reflector_k - 273.15,
            x.reactivity.xenon * 1e5,
            x.reactivity.total() * 1e5,
            x.decay_power_w / 1e3,
            x.heat_to_rccs_w / 1e3
        );
    }
    let t1 = m.time_to_one_percent_s.expect("power reaches 1 %");
    assert!((t1 - 330.0).abs() <= 120.0, "time to 1 % = {t1} s");
}

/// **V&V: the named sensitivity cases** (input sheet Sec 9.2), each
/// changing one input from BASE. Recorded, not gated; every case is
/// reported whichever way it moves the answer.
#[test]
fn lofc_atws_sensitivity_cases() {
    let base = LofcCase::base().unwrap();
    let mut cases: Vec<LofcCase> = Vec::new();
    cases.push(base.clone());
    let mut c = base.clone().named("BASE-6H");
    c.duration = s(21_600.0);
    cases.push(c);
    // gh:#1151 part B: the start-up source is ON by default since then;
    // these two reproduce part A's source-free kinetics.
    let mut c = base.clone().named("SRC-OFF");
    c.kinetics = c.kinetics.without_neutron_source();
    cases.push(c);
    let mut c = base.clone().named("BASE-6H-SRC-OFF");
    c.duration = s(21_600.0);
    c.kinetics = c.kinetics.without_neutron_source();
    cases.push(c);
    let mut c = base.clone().named("KIN-LAMBDA-H06");
    c.kinetics.generation_time = s(1.68e-4);
    cases.push(c);
    let mut c = base.clone().named("KIN-DN-TEHO");
    c.kinetics.delayed_set = DelayedNeutronSet::TehOPrkeU235;
    cases.push(c);
    let mut c = base.clone().named("KIN-DN-1G");
    c.kinetics.delayed_set = DelayedNeutronSet::OneGroupKeepin;
    cases.push(c);
    let mut c = base.clone().named("KIN-ALPHA-G02-IC");
    c.kinetics.coefficients = FeedbackCoefficients::g02_initial_core();
    cases.push(c);
    let mut c = base.clone().named("KIN-ALPHA-G02-EQ");
    c.kinetics.coefficients = FeedbackCoefficients::g02_equilibrium_core();
    cases.push(c);
    let mut c = base.clone().named("XE-OFF");
    c.kinetics.xenon = XenonModel::Off;
    cases.push(c);
    let mut c = base.clone().named("DH-HALF");
    c.kinetics.decay_heat_scale = 0.5;
    cases.push(c);
    for (name, b) in [
        ("BYP-G02", BypassSplit::g02()),
        ("BYP-S5-CR", BypassSplit::s5_control_rods()),
        ("BYP-NOLEAK", BypassSplit::no_leakage()),
    ] {
        let mut c = base.clone().named(name);
        c.bypass = b;
        cases.push(c);
    }
    cases.push(base.clone().named("LOFC-SPEC").with_coastdown(CoastdownModel::Instantaneous));
    let mut c = base.clone().named("LOFC-J09-FLOW").with_coastdown(CoastdownModel::j09_flow().unwrap());
    c.loop_config.baffle_close_after_trip = None;
    cases.push(c);
    cases.push(
        base.clone()
            .named("LOFC-INERTIAL-49S")
            .with_coastdown(CoastdownModel::Inertial { time_constant: s(49.0) }),
    );
    cases.push(base.clone().named("TR-HU").with_initial(InitialState::hu2006()));
    cases.push(base.clone().named("TR-SPEC").with_initial(InitialState::spec()));
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min |");
    println!("|---|---|---|---|---|---|---|");
    let mut base_t1 = None;
    for case in &cases {
        let (_, trace) = run_lofc(case).unwrap();
        let m = summarise(&case.name, &trace);
        if case.name == "BASE" {
            base_t1 = m.time_to_one_percent_s;
        }
    }
    assert!(base_t1.is_some());
}

/// The helical-coil SG in the loop: the as-run initial state with the
/// workspace exchanger (illustrative UA 4.26e4 W/K, `htr10_illustrative_config`)
/// and a feedwater set by energy balance to the design steam state (104 degC
/// feed, 440 degC / 4.0 MPa steam, plant-data Sec 6; the 30 %-power secondary
/// state is Unknown). Records the helium temperature the exchanger delivers
/// against the measured 212 degC, then runs the first 20 s of the LOFC.
///
/// Gated behind `long-tests` (workspace rule: over 5 minutes): the
/// exchanger runs at its stable 0.0125 s substep with up to 8 coupling
/// iterations, and the 300 s settle took 1553 s wall on 2026-11-03.
#[cfg(feature = "long-tests")]
#[test]
fn helical_coil_steam_generator_in_the_loop() {
    use tampines::htr10_primary_loop::SteamGeneratorModel;
    use uom::si::f64::{MassRate, Pressure};
    use uom::si::mass_rate::kilogram_per_second;
    use tampines::components::helical_coil_sg_standalone::{htr10_illustrative_config, STABLE_SUBSTEP_S};
    use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
    use tampines_steam_tables::region_2_vapour::intensive_properties::h_tp_2;
    use uom::si::available_energy::joule_per_kilogram;
    let p_steam = Pressure::new::<pascal>(4.0e6);
    let h_feed = h_tp_1(k(377.15), p_steam);
    let h_steam = h_tp_2(k(713.15), p_steam);
    let feed = 3.315e6 / (h_steam - h_feed).get::<joule_per_kilogram>();
    let mut cfg = htr10_illustrative_config(STABLE_SUBSTEP_S, 8);
    cfg.hot_pressure = Pressure::new::<pascal>(2.476e6);
    cfg.initial_hot_end_temperature = k(923.15);
    cfg.initial_cold_end_temperature = k(400.0);
    let exchanger = tampines::components::NodalisedCounterFlowSteamGenerator::new(cfg).unwrap();
    let sg = SteamGeneratorModel::HelicalCoil {
        exchanger,
        feedwater_enthalpy: h_feed,
        feedwater_flow: MassRate::new::<kilogram_per_second>(feed),
    };
    let case = LofcCase::base().unwrap();
    let core = CoreModel::LumpedStandIn(LumpedPebbleBedCore::htr10(BypassSplit::tecdoc1694(), k(700.0)).unwrap());
    let mut sim = CoupledTransient::with_parts(&case, core, sg).unwrap();
    let hot = sim.initial_response().outlet_temperature;
    let started = std::time::Instant::now();
    let mut loop_ref = sim.settle_steam_generator(hot, s(300.0)).unwrap().get::<kelvin>();
    println!(
        "feedwater {feed:.4} kg/s; SG helium outlet after 300 s settle at the as-run hot inlet {:.2} C (measured RPV inlet 212 C); {:.1} s wall",
        loop_ref - 273.15,
        started.elapsed().as_secs_f64()
    );
    for _ in 0..200 {
        sim.step(s(0.1)).unwrap();
    }
    loop_ref = sim.primary_loop().sg_outlet_temperature().get::<kelvin>();
    println!("after 20 s of LOFC: SG helium outlet {:.2} C, flow {:.4} kg/s", loop_ref - 273.15, sim.primary_loop().mass_flow().get::<kilogram_per_second>());
    assert!(loop_ref.is_finite());
}
