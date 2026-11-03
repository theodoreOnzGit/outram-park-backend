//! Tests for the HTR-10 primary loop (gh:#1151). Results are recorded in
//! `crates/tampines/verification_and_validation/htr10_loop_lofc/README.md`.

use uom::si::f64::{Length, MassRate, Power, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::data::*;
use super::*;
use crate::htr10_primary_loop::kinetics::FeedbackTemperatures;

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
        ("power-density-3mw-lofc-tab2-56.csv", POWER_MAP_TAB2_56),
        ("lofc-blower-speed-c09-fig4.csv", BLOWER_SPEED_C09_FIG4),
        ("lofc-flow-coastdown-j09-fig6.csv", FLOW_COASTDOWN_J09_FIG6),
        ("lofc-power-long-c09-fig6.csv", POWER_LONG_C09_FIG6),
        ("lofc-power-short-c09-fig4.csv", POWER_SHORT_C09_FIG4),
    ] {
        let sheet = std::fs::read_to_string(format!("{dir}{name}")).expect("sheet file");
        assert_eq!(sheet, embedded, "{name} drifted from the input sheet");
    }
}

#[test]
fn tabulated_series_interpolates_and_holds() {
    let t = TabulatedSeries::new(vec![(0.0, 1.0), (10.0, 0.0)]).unwrap();
    assert_eq!(t.value_at(-1.0), 1.0);
    assert_eq!(t.value_at(5.0), 0.5);
    assert_eq!(t.value_at(20.0), 0.0);
    assert!(TabulatedSeries::new(vec![(1.0, 0.0), (1.0, 2.0)]).is_err());
    assert!(TabulatedSeries::new(vec![]).is_err());
    assert!(numeric_rows("# c\nh\n1,x\n").is_err());
}

/// Tab 2.56 integrates to the sheet's 2.991 MW; the zone fractions sum to 1.
#[test]
fn power_map_zone_fractions() {
    let map = lofc_power_map().unwrap();
    let total = map_power_in_band_w(&map, 0.0, 1000.0);
    println!("Tab 2.56 integral = {:.4} MW", total / 1e6);
    assert!((total / 1e6 - 2.991).abs() < 0.0015);
    let core = LumpedPebbleBedCore::htr10(BypassSplit::tecdoc1694(), k(600.0)).unwrap();
    let f = core.zone_power_fractions();
    println!("zone power fractions {f:?}");
    println!("zone pebbles (fuel, dummy) {:?}", core.zone_pebbles());
    assert!((f.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert_eq!(f[5], 0.0);
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

fn as_run_boundary(w: f64) -> CoreBoundary {
    CoreBoundary {
        mass_flow: MassRate::new::<kilogram_per_second>(w),
        cold_nozzle_temperature: k(485.15),
        hot_nozzle_temperature: k(900.0),
        pressure: Pressure::new::<pascal>(2.476e6),
    }
}

/// **V&V (verification): the stand-in's steady state at the as-run point
/// closes its energy balance exactly**, and its outlet temperature against
/// the measured 650 degC is reported (a check, not a gate: the flow was
/// derived assuming zero loss to the RCCS).
#[test]
fn lumped_core_steady_state_balances_energy() {
    let p = Power::new::<watt>(3.315e6);
    let mut core = CoreModel::LumpedStandIn(LumpedPebbleBedCore::htr10(BypassSplit::tecdoc1694(), k(700.0)).unwrap());
    let r = core.steady_state(p, &as_run_boundary(1.457)).unwrap();
    let to_cool = r.power_to_coolant.get::<watt>();
    let rccs = r.heat_to_rccs.get::<watt>();
    let CoreModel::LumpedStandIn(c) = &core;
    println!("node temperatures (C): {:?}", c.node_temperatures().iter().map(|t| (t - 273.15) * 10.0_f64).map(|x| x.round() / 10.0).collect::<Vec<_>>());
    println!("bed gas (C): {:?}", c.bed_gas_temperatures().iter().map(|t| t - 273.15).collect::<Vec<_>>());
    println!(
        "outlet {:.2} C; to coolant {:.1} kW; to RCCS {:.2} kW; residual {:.3e}",
        r.outlet_temperature.get::<kelvin>() - 273.15,
        to_cool / 1e3,
        rccs / 1e3,
        (to_cool + rccs - 3.315e6) / 3.315e6
    );
    let f = r.feedback;
    println!(
        "feedback: fuel {:.2} C, moderator {:.2} C, reflector {:.2} C",
        f.fuel.get::<kelvin>() - 273.15,
        f.moderator.get::<kelvin>() - 273.15,
        f.reflector.get::<kelvin>() - 273.15
    );
    let pr = Pressure::new::<pascal>(2.476e6);
    let (cb, cr, cv) = c.capacity_summary(pr).unwrap();
    println!("capacities: bed {:.3} MJ/K, reflector {:.2} MJ/K, barrel+RPV {:.2} MJ/K", cb / 1e6, cr / 1e6, cv / 1e6);
    println!("conductances (W/K) [fuel->shell, bed->refl, refl->barrel, barrel->RPV, RPV->RCCS]: {:?}", c.conductance_summary(pr).unwrap());
    println!(
        "core dP at 1.457 kg/s: {:.1} Pa; gravity head {:.2} Pa; inertance {:.1} 1/m",
        core.pressure_drop(MassRate::new::<kilogram_per_second>(1.457), pr).unwrap().get::<pascal>(),
        core.gravity_head(pr).unwrap().get::<pascal>(),
        core.inertance_per_m()
    );
    assert!(((to_cool + rccs) / 3.315e6 - 1.0).abs() < 1e-6);
    // Reversed flow balances too.
    let r2 = core.steady_state(p, &as_run_boundary(-0.3)).unwrap();
    let res2 = (r2.power_to_coolant.get::<watt>() + r2.heat_to_rccs.get::<watt>()) / 3.315e6 - 1.0;
    println!("reversed 0.3 kg/s: outlet (cold nozzle) {:.1} C, residual {res2:.3e}", r2.outlet_temperature.get::<kelvin>() - 273.15);
    assert!(res2.abs() < 1e-6);
    assert!(core.pressure_drop(MassRate::new::<kilogram_per_second>(-0.3), pr).unwrap().value < 0.0);
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

/// Geometry rule (crate `CLAUDE.md`): draw what the stand-in solver uses —
/// the zone stack from the ASSEMBLED core (heights, pebble counts, power
/// fractions, solved steady temperatures) and the radial bodies from the
/// table the solver reads — as an R-Z half-section to scale.
#[test]
fn draw_the_lumped_core_layout() {
    let mut core = LumpedPebbleBedCore::htr10(BypassSplit::tecdoc1694(), k(700.0)).unwrap();
    core.steady_state(Power::new::<watt>(3.315e6), &as_run_boundary(1.457)).unwrap();
    let px = 150.0; // per metre
    let (x0, y0) = (60.0, 140.0); // r = 0, z = 0 (TECDOC z downward)
    let mut svg = String::new();
    svg.push_str(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1300" height="900" font-family="sans-serif" font-size="11"><rect width="100%" height="100%" fill="white"/>"##);
    svg.push_str(r##"<text x="20" y="24" font-size="15">HTR-10 lumped core stand-in, as assembled (gh:#1151): R-Z half-section to scale, TECDOC z downward from the bed top</text>"##);
    let heights = core.zone_heights();
    let pebbles = core.zone_pebbles();
    let frac = core.zone_power_fractions();
    let temps = core.node_temperatures().to_vec();
    let mut z = 0.0;
    for (i, h) in heights.iter().enumerate() {
        let (r_out, colour) = if i < 5 { (0.90, "#f4a261") } else { (0.835, "#c9c9c9") };
        let (y, hh) = (y0 + z * px, h * px);
        svg.push_str(&format!(
            r##"<rect x="{x0}" y="{y:.1}" width="{:.1}" height="{hh:.1}" fill="{colour}" stroke="black"/>"##,
            r_out * px
        ));
        let label = if i < 5 {
            format!(
                "zone {i}: z {:.0}-{:.0} cm, fuel {:.0} + dummy {:.0} pebbles, P {:.1} %, T fuel/shell/dummy {:.0}/{:.0}/{:.0} C",
                z * 100.0,
                (z + h) * 100.0,
                pebbles[i].0,
                pebbles[i].1,
                100.0 * frac[i],
                temps[3 * i] - 273.15,
                temps[3 * i + 1] - 273.15,
                temps[3 * i + 2] - 273.15
            )
        } else {
            format!(
                "cone (unheated; drawn at its top-step radius 83.5 cm): {:.0} dummies, {:.0} C",
                pebbles[i].1,
                temps[15] - 273.15
            )
        };
        svg.push_str(&format!(r##"<text x="560" y="{:.1}">{label}</text>"##, y + hh / 2.0 + 4.0));
        svg.push_str(&format!(
            r##"<line x1="{:.1}" y1="{:.1}" x2="555" y2="{:.1}" stroke="#555" stroke-dasharray="3,3"/>"##,
            x0 + 0.45 * px,
            y + hh / 2.0,
            y + hh / 2.0
        ));
        z += h;
    }
    let colours = ["#6a994e", "#90be6d", "#6a994e", "#43aa8b", "#6a994e", "#7f5539", "#577590", "#277da1", "#d00000"];
    for (j, (name, a, b, h)) in LumpedPebbleBedCore::radial_layout().iter().enumerate() {
        // Reflector/brick span z -65..325 cm; taller bodies are centred on it.
        let top = -0.65 - (h - 3.90) / 2.0;
        let (x, w) = (x0 + a * px, ((b - a) * px).max(2.0));
        svg.push_str(&format!(
            r##"<rect x="{x:.1}" y="{:.1}" width="{w:.1}" height="{:.1}" fill="{}" stroke="black" stroke-width="0.5" opacity="0.8"/>"##,
            y0 + top * px,
            h * px,
            colours[j % colours.len()]
        ));
        let ty = 80.0 + 12.0 * j as f64;
        svg.push_str(&format!(
            r##"<text x="560" y="{ty:.1}" fill="{}">{name}: r {:.3}-{:.3} m, height {:.2} m</text>"##,
            colours[j % colours.len()],
            a,
            b,
            h
        ));
    }
    svg.push_str(&format!(
        r##"<text x="560" y="{:.1}">steady at as-run 3315 kW, inlet 212 C, 1.457 kg/s: reflector node {:.0} C, barrel {:.0} C, RPV {:.0} C</text>"##,
        y0 + 3.0 * px,
        temps[16] - 273.15,
        temps[17] - 273.15,
        temps[18] - 273.15
    ));
    svg.push_str(&format!(
        r##"<text x="560" y="{:.1}">Not in the stand-in: top and bottom reflectors, plena structures, discharge-tube pebbles, in-vessel natural circulation.</text></svg>"##,
        y0 + 3.2 * px
    ));
    let out = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/verification_and_validation/htr10_loop_lofc/lumped_core_layout.svg"
    );
    std::fs::write(out, svg).unwrap();
    assert_eq!(heights.len(), 6);
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
    use crate::components::helical_coil_sg_standalone::{htr10_illustrative_config, STABLE_SUBSTEP_S};
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
    let exchanger = crate::components::NodalisedCounterFlowSteamGenerator::new(cfg).unwrap();
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
