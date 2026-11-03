//! Loop-only thermal-hydraulic tests for the HTR-10 primary loop and the
//! lumped stand-in (gh:#1151), with the power a fixed input. The coupled
//! tests (kinetics, LOFC) moved to `nee_soon::htr10_th_transient` on
//! 2026-11-03; see `crates/tampines/verification_and_validation/htr10_primary_loop/README.md`.

use uom::si::f64::{MassRate, Power, Pressure, ThermodynamicTemperature};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use super::data::*;
use super::*;

fn k(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

/// The embedded copies are the input sheet's files, byte for byte.
#[test]
fn data_copies_match_the_input_sheet() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reactor-scoping/htr10-th-data/");
    for (name, embedded) in [
        ("power-density-3mw-lofc-tab2-56.csv", POWER_MAP_TAB2_56),
        ("lofc-blower-speed-c09-fig4.csv", BLOWER_SPEED_C09_FIG4),
        ("lofc-flow-coastdown-j09-fig6.csv", FLOW_COASTDOWN_J09_FIG6),
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
fn bypass_splits() {
    for b in [BypassSplit::tecdoc1694(), BypassSplit::g02(), BypassSplit::s5_control_rods(), BypassSplit::no_leakage()] {
        assert!(LumpedPebbleBedCore::htr10(b, k(600.0)).is_ok());
    }
    let bad = BypassSplit { bed: 0.5, discharge_tube: 0.0, control_rods: 0.0, leakage: 0.0 };
    assert!(LumpedPebbleBedCore::htr10(bad, k(600.0)).is_err());
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
    let CoreModel::LumpedStandIn(c) = &core else { panic!("lumped core expected") };
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
        "/verification_and_validation/htr10_primary_loop/lumped_core_layout.svg"
    );
    std::fs::write(out, svg).unwrap();
    assert_eq!(heights.len(), 6);
}

