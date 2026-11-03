// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Unit and verification checks of the transient R-Z core (gh:#1151
//! part B), on the Fig 2.5 mesh (21 x 39 cells) so they run in seconds.
//! The coupled LOFC / CRW validation is in `tampines`
//! (`verification_and_validation/htr10_rz_transients/README.md`).

use uom::si::energy::joule;
use uom::si::f64::{MassRate, Power, Pressure, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::power_map::{crw_power_map, lofc_power_map, map_power_w};
use super::transient::{
    dummy_pebble_conductance, fuel_pebble_conductance, DummyPebbleModel, FuelZoneModel, Htr10RzTransientCore, ModeratorTemperature,
    ReflectorAverage, TransientCoreOptions,
};
use super::OperatingPoint;

fn as_run(power_w: f64, flow: f64) -> OperatingPoint {
    OperatingPoint {
        power: Power::new::<watt>(power_w),
        inlet: ThermodynamicTemperature::new::<kelvin>(485.15),
        pressure: Pressure::new::<pascal>(2.476e6),
        mass_flow: MassRate::new::<kilogram_per_second>(flow),
    }
}

fn coarse(o: TransientCoreOptions) -> TransientCoreOptions {
    let mut o = o;
    o.solid.max_cell = None;
    o
}

/// The embedded Tab 2.56 / 2.61 copies are the input sheet's, byte for
/// byte, and integrate to the sheet's Derived 2.991 / 2.999 MW.
#[test]
fn three_megawatt_power_maps_match_the_input_sheet() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reactor-scoping/htr10-th-data/");
    for (name, ours) in [
        ("power-density-3mw-lofc-tab2-56.csv", include_str!("data/power-density-3mw-lofc-tab2-56.csv")),
        ("power-density-3mw-crw-tab2-61.csv", include_str!("data/power-density-3mw-crw-tab2-61.csv")),
    ] {
        match std::fs::read_to_string(format!("{dir}{name}")) {
            Ok(s) => assert_eq!(s, ours, "{name} drifted from the input sheet"),
            Err(_) => println!("input sheet not present; {name} not compared"),
        }
    }
    let (a, b) = (map_power_w(&lofc_power_map()), map_power_w(&crw_power_map()));
    println!("Tab 2.56 {:.4} MW, Tab 2.61 {:.4} MW", a / 1e6, b / 1e6);
    assert!((a / 1e6 - 2.991).abs() < 0.0015 && (b / 1e6 - 2.999).abs() < 0.0015);
}

/// Correct physics is the default (workspace HARD RULE): natural
/// circulation (buoyancy) ON, the lagged fuel zone ON, the reflector
/// average over all graphite; the ablations are explicit builders.
#[test]
fn transient_core_defaults_are_pinned() {
    let o = TransientCoreOptions::htr10_lofc();
    assert!(o.buoyancy);
    assert_eq!(o.fuel_zone, FuelZoneModel::Lagged);
    assert_eq!(o.reflector, ReflectorAverage::AllGraphite);
    assert!((o.fuel_pebble_fraction - 13_651.0 / 23_949.0).abs() < 1e-12);
    assert!(!o.without_buoyancy().buoyancy);
    assert_eq!(o.with_quasi_steady_fuel().fuel_zone, FuelZoneModel::QuasiSteady);
    // gh:#1174: the two-temperature bed and the all-graphite moderator.
    assert_eq!(o.dummy_pebbles, DummyPebbleModel::Lagged);
    assert_eq!(o.moderator, ModeratorTemperature::AllPebbleGraphite);
    assert_eq!(o.without_dummy_pebble_lag().dummy_pebbles, DummyPebbleModel::InBedCell);
    assert_eq!(o.with_moderator_outside_fuel_zones().moderator, ModeratorTemperature::OutsideFuelZones);
    assert_eq!(TransientCoreOptions::htr10_crw().dummy_pebbles, DummyPebbleModel::Lagged);
    let core = Htr10RzTransientCore::build(coarse(o)).unwrap();
    assert!(core.network().has_buoyancy());
    let core = Htr10RzTransientCore::build(coarse(o.without_buoyancy())).unwrap();
    assert!(!core.network().has_buoyancy());
    let mut bad = o;
    bad.fuel_pebble_fraction = 0.0;
    assert!(Htr10RzTransientCore::build(coarse(bad)).is_err());
}

/// Fuel-pebble conductance against a hand evaluation: `k = 20 W/(m K)`,
/// `R_zone = 1/(20 pi 20 0.025) = 0.031831`, `R_shell = (40 - 33.333)/(4 pi 20)
/// = 0.026526` K/W, `G = 17.136 W/K`.
#[test]
fn fuel_pebble_conductance_hand_value() {
    let g = fuel_pebble_conductance(20.0);
    println!("G(k = 20) = {g:.6} W/K");
    assert!((g - 1.0 / (0.031_830_99 + 0.026_525_82)).abs() < 1e-4);
}

/// **Verification: the steady state closes the plant energy balance, and
/// a stopped-loop march conserves energy.**
///
/// Methodology: the as-run point (3315 kW, 212 deg C, 2476 kPa,
/// 1.457 kg/s), Tab 2.56 shape, Fig 2.5 mesh. Steady: `P = W c_p dT +
/// Q_RCCS+heads` to the outer tolerance. Then 20 steps of 10 s at 200 kW
/// with the loop stopped: the stored energy (solid and fuel nodes) must
/// change by `sum (P - Q_RCCS+heads - Q_gas) dt`. Prediction (before the
/// run): steady imbalance below 1e-4 of the power; the march to the Picard
/// tolerance, below 1e-4 relative of the energy deposited.
#[test]
fn transient_core_conserves_energy() {
    let mut core = Htr10RzTransientCore::build(coarse(TransientCoreOptions::htr10_lofc())).unwrap();
    let op = as_run(3.315e6, 1.457);
    let r = core.solve_steady(&op).unwrap();
    let rise = 1.457 * super::flow::HELIUM_CP * (r.outlet_temperature.get::<kelvin>() - 485.15);
    let loss = r.heat_to_rccs_and_heads.get::<watt>();
    let imb = (3.315e6 - rise - loss) / 3.315e6;
    let f = r.feedback;
    println!(
        "steady: outlet {:.2} C, RCCS+heads {:.1} kW, imbalance {imb:.2e}; fuel {:.1} C, moderator {:.1} C, reflector {:.1} C",
        r.outlet_temperature.get::<kelvin>() - 273.15,
        loss / 1e3,
        f.fuel.get::<kelvin>() - 273.15,
        f.moderator.get::<kelvin>() - 273.15,
        f.reflector.get::<kelvin>() - 273.15
    );
    assert!(imb.abs() < 1e-4);
    assert!(f.fuel > f.moderator);
    let reference = ThermodynamicTemperature::new::<kelvin>(300.0);
    let e0 = core.stored_energy(reference).get::<joule>();
    let stopped = as_run(2.0e5, 0.0);
    let mut budget = 0.0;
    let mut up = 0.0;
    for _ in 0..20 {
        let s = core.step(Time::new::<second>(10.0), &stopped).unwrap();
        budget += 10.0 * (2.0e5 - s.heat_to_rccs_and_heads.get::<watt>() - s.heat_to_gas.get::<watt>());
        up = s.circulation.bed_upflow.get::<kilogram_per_second>();
    }
    let gained = core.stored_energy(reference).get::<joule>() - e0;
    let rel = (gained - budget) / (2.0e5 * 200.0);
    println!("march: gained {gained:.6e} J, budget {budget:.6e} J, relative to deposited {rel:.2e}; bed upflow {up:.4} kg/s");
    assert!(rel.abs() < 1e-4, "{rel}");
}

/// **The in-vessel natural circulation exists only through buoyancy.**
/// With the loop stopped, the bed carries an upward and a downward stream
/// across its mid-height (mass-balanced); with the `NC-OFF` ablation both
/// vanish. Prediction (before the run): with buoyancy, an upflow of order
/// 0.01-0.3 kg/s (Darcy KTA estimate in the V&V README); without, below
/// 1e-9 kg/s.
#[test]
fn natural_circulation_needs_buoyancy() {
    let op = as_run(3.315e6, 1.457);
    let stopped = as_run(2.0e5, 0.0);
    let mut out = Vec::new();
    for opts in [TransientCoreOptions::htr10_lofc(), TransientCoreOptions::htr10_lofc().without_buoyancy()] {
        let mut core = Htr10RzTransientCore::build(coarse(opts)).unwrap();
        core.solve_steady(&op).unwrap();
        let mut s = None;
        for _ in 0..5 {
            s = Some(core.step(Time::new::<second>(10.0), &stopped).unwrap());
        }
        let c = s.unwrap().circulation;
        let (u, d) = (c.bed_upflow.get::<kilogram_per_second>(), c.bed_downflow.get::<kilogram_per_second>());
        println!(
            "buoyancy {}: bed upflow {u:.5} kg/s, downflow {d:.5} kg/s, max |G| {:.4} kg/(m2 s), enthalpy up through bed top {:.1} kW",
            opts.buoyancy,
            c.max_bed_mass_flux,
            c.enthalpy_up_through_bed_top.get::<watt>() / 1e3
        );
        out.push(u);
    }
    assert!(out[0] > 1e-3, "{}", out[0]);
    assert!(out[1] < 1e-9, "{}", out[1]);
}

/// The lagged fuel zone: the same steady state as the quasi-steady
/// ablation, and after a power step its fuel temperature lags. Prediction:
/// steady fuel temperatures equal to 1e-6 K; 2 s after a step to 2x power
/// the lagged rise is below the quasi-steady one (time constant ~7 s).
#[test]
fn lagged_fuel_zone_lags_and_agrees_at_steady_state() {
    let op = as_run(3.315e6, 1.457);
    let mut rises = Vec::new();
    let mut steady = Vec::new();
    for opts in [TransientCoreOptions::htr10_lofc(), TransientCoreOptions::htr10_lofc().with_quasi_steady_fuel()] {
        let mut core = Htr10RzTransientCore::build(coarse(opts)).unwrap();
        let r = core.solve_steady(&op).unwrap();
        steady.push(r.feedback.fuel.get::<kelvin>());
        let s = core.step(Time::new::<second>(2.0), &as_run(6.63e6, 1.457)).unwrap();
        rises.push(s.feedback.fuel.get::<kelvin>() - r.feedback.fuel.get::<kelvin>());
    }
    println!("steady fuel {steady:?} K; fuel rise in 2 s at 2x power: lagged {:.4} K, quasi-steady {:.4} K", rises[0], rises[1]);
    assert!((steady[0] - steady[1]).abs() < 1e-6);
    assert!(rises[0] > 0.0 && rises[0] < rises[1]);
}

/// gh:#1175 regression: on the 10 cm mesh the gas-flow Picard converges
/// with the loop stopped. Before the fix it stalled at the first stopped
/// step (step 12 here) with a per-cell flux change of 6.7e-7 against the
/// 1e-7 tolerance, after 200 passes: the flux measure's round-off floor
/// (see `flow::resistance_change`). Measured after the fix (2026-11-03):
/// 27 passes at step 12, 13-19 per step from then on. The tolerance is
/// unchanged; the assertion is that every step converges (an error
/// otherwise) in well under the 200-pass cap.
#[test]
fn gas_picard_converges_on_the_10cm_mesh_with_the_loop_stopped() {
    use uom::si::f64::Length;
    use uom::si::length::centimeter;
    let o = TransientCoreOptions::htr10_lofc().with_max_cell(Length::new::<centimeter>(10.0));
    let mut core = Htr10RzTransientCore::build(o).unwrap();
    core.solve_steady(&as_run(3.315e6, 1.457)).unwrap();
    let mut most = 0;
    for i in 0..20 {
        let flow = if i < 12 { 1.457 * (1.0 - i as f64 / 12.0) } else { 0.0 };
        let r = core
            .step(Time::new::<second>(1.0), &as_run(3.0e5, flow))
            .unwrap_or_else(|e| panic!("step {i} (flow {flow:.3} kg/s): {e}"));
        let (passes, change) = core.gas().unwrap().flow_picard;
        assert!(change < 1e-7, "step {i}: last change {change:e}");
        most = most.max(r.gas_picard_iterations);
        println!("step {i}: flow {flow:.3} kg/s, gas Picard {passes} passes, last resistance change {change:.2e}");
    }
    assert!(most < 150, "{most} passes");
}

/// gh:#1174: dummy-pebble conductance against a hand evaluation:
/// `k_g = 20`, `k_bed = 10 W/(m K)`, no gas film: `R_int = 1/(20 pi 20
/// 0.03) = 0.026526` K/W, `G_ext = 2 pi 0.06 10 = 3.76991` W/K, `G =
/// 1/(0.026526 + 0.265258) = 3.42720 W/K`; a gas film adds in parallel to
/// `G_ext`.
#[test]
fn dummy_pebble_conductance_hand_value() {
    let g = dummy_pebble_conductance(20.0, 10.0, 0.0);
    println!("G_dummy(k_g 20, k_bed 10) = {g:.5} W/K");
    assert!((g - 3.427_20).abs() < 1e-4, "{g}");
    assert!(dummy_pebble_conductance(20.0, 10.0, 3.0) > g);
}

/// gh:#1174: the two-temperature bed. Prediction (before the run): the
/// dummies sit at the bed cell temperature at steady state; after a step
/// to 2x power they lag the cell (time constant 45-100 s, hand estimate in
/// the V&V README Sec 8), the cell (now the fuel pebbles' shells only)
/// heats faster than the one-temperature cell, and the march conserves
/// energy with the dummy nodes counted.
#[test]
fn dummy_pebbles_lag_the_bed_and_conserve_energy() {
    let op = as_run(3.315e6, 1.457);
    let mut core = Htr10RzTransientCore::build(coarse(TransientCoreOptions::htr10_lofc())).unwrap();
    core.solve_steady(&op).unwrap();
    let c = *core
        .heated_cells()
        .iter()
        .max_by(|a, b| core.cell_power_density(**a).value.total_cmp(&core.cell_power_density(**b).value))
        .unwrap();
    let cell = |k: &Htr10RzTransientCore| k.solid().field().temperature(c).get::<kelvin>();
    let d0 = core.dummy_temperature(c).unwrap().get::<kelvin>();
    assert!((d0 - cell(&core)).abs() < 1e-9, "steady dummy {d0} vs cell {}", cell(&core));
    let (c0, f0) = (cell(&core), core.fuel_temperature(c).unwrap().get::<kelvin>());
    for _ in 0..5 {
        core.step(Time::new::<second>(2.0), &as_run(6.63e6, 1.457)).unwrap();
    }
    let dc = cell(&core) - c0;
    let dd = core.dummy_temperature(c).unwrap().get::<kelvin>() - d0;
    let df = core.fuel_temperature(c).unwrap().get::<kelvin>() - f0;
    println!("hottest cell, 10 s at 2x power: fuel zone +{df:.3} K, shells (cell) +{dc:.3} K, dummy +{dd:.3} K");
    assert!(dd > 0.0 && dd < dc && dc < df);
    // Without the node the cell carries the dummies' capacity: it heats less.
    let mut flat = Htr10RzTransientCore::build(coarse(TransientCoreOptions::htr10_lofc().without_dummy_pebble_lag())).unwrap();
    flat.solve_steady(&op).unwrap();
    assert!(flat.dummy_temperature(c).is_none());
    let c1 = flat.solid().field().temperature(c).get::<kelvin>();
    for _ in 0..5 {
        flat.step(Time::new::<second>(2.0), &as_run(6.63e6, 1.457)).unwrap();
    }
    let dflat = flat.solid().field().temperature(c).get::<kelvin>() - c1;
    println!("same, dummies in the cell: cell +{dflat:.3} K");
    assert!(dflat < dc);
    // Energy over a stopped-loop march with the dummy nodes.
    let reference = ThermodynamicTemperature::new::<kelvin>(300.0);
    let e0 = core.stored_energy(reference).get::<joule>();
    let mut budget = 0.0;
    for _ in 0..10 {
        let s = core.step(Time::new::<second>(10.0), &as_run(2.0e5, 0.0)).unwrap();
        budget += 10.0 * (2.0e5 - s.heat_to_rccs_and_heads.get::<watt>() - s.heat_to_gas.get::<watt>());
    }
    let rel = (core.stored_energy(reference).get::<joule>() - e0 - budget) / (2.0e5 * 100.0);
    println!("stopped march with dummies: energy error relative to deposited {rel:.2e}");
    assert!(rel.abs() < 1e-4, "{rel}");
}

/// gh:#1174: the two moderator readings at steady state (dummies at the
/// cell temperature). `OutsideFuelZones` reads the cell; `AllPebbleGraphite`
/// reads `w_f T_f + (1 - w_f) T_cell`, power-weighted, with
/// `w_f = x (2.5/3)^3 = 0.330`, i.e. hotter by `w_f (T_f - T_m,outside)`.
#[test]
fn moderator_readings_at_steady_state() {
    let op = as_run(3.315e6, 1.457);
    let mut m = Vec::new();
    for o in [TransientCoreOptions::htr10_lofc(), TransientCoreOptions::htr10_lofc().with_moderator_outside_fuel_zones()] {
        let mut core = Htr10RzTransientCore::build(coarse(o)).unwrap();
        let r = core.solve_steady(&op).unwrap();
        m.push((r.feedback.fuel.get::<kelvin>(), r.feedback.moderator.get::<kelvin>()));
    }
    let wf = 13_651.0 / 23_949.0 * (2.5_f64 / 3.0).powi(3);
    println!(
        "steady fuel {:.3} K; moderator all-graphite {:.3} K, outside fuel zones {:.3} K; w_f {wf:.4}",
        m[0].0, m[0].1, m[1].1
    );
    assert!((m[0].1 - (m[1].1 + wf * (m[1].0 - m[1].1))).abs() < 1e-6);
}
