// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Unit checks of the HTR-10 R-Z model's inputs: the region map against the
//! cited extents, the power map, the closures as tabulated, the boundary
//! emissivities, and the energy balance of a coarse solve of each case.
//! The physics cases themselves (with predictions and results) are in
//! `tests/htr10_rz_solid.rs` and the V&V record
//! `verification_and_validation/htr10_rz_solid/README.md`.

use uom::si::f64::{Length, Power, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;

use super::geometry::{idx, region_at_cm, region_rects, N_REGIONS};
use super::materials::SIGMA;
use super::power_map::{fpic_power_map, map_power_w};
use super::*;
use crate::genfoam::thermal_hydraulics::solid_conduction::SolidBoundary;
use crate::htgr_closures::{BedConductivityModel, ZbsBed};

fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

/// The embedded power map is byte-identical to the input sheet's CSV.
#[test]
fn power_map_copy_matches_the_input_sheet() {
    let sheet = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reactor-scoping/htr10-th-data/power-density-fpic-10mw-tab2-6.csv");
    let ours = include_str!("data/power-density-fpic-10mw-tab2-6.csv");
    match std::fs::read_to_string(&sheet) {
        Ok(s) => assert_eq!(s, ours, "the embedded Tab 2.6 copy drifted from the input sheet"),
        Err(_) => println!("input sheet not present in this checkout; copy not compared"),
    }
}

/// 110 cells and the input sheet's Derived integral, 9.984 MW; the mapping
/// onto the model mesh preserves it.
#[test]
fn power_map_integral_and_mapping() {
    let map = fpic_power_map();
    assert_eq!(map.len(), 110);
    let p = map_power_w(&map);
    println!("Tab 2.6 integral {p:.1} W");
    assert!((p / 9.984e6 - 1.0).abs() < 1e-3, "{p}");
    for h in [None, Some(10.0)] {
        let opts = Htr10RzOptions {
            max_cell: h.map(Length::new::<centimeter>),
            ..Htr10RzOptions::default()
        };
        let m = Htr10RzModel::build(
            opts,
            Htr10RzCase::DecayPower {
                power: Power::new::<watt>(2.0e5),
            },
        )
        .unwrap();
        let g = m.field().energy_balance().generated.get::<watt>();
        assert!((g / 2.0e5 - 1.0).abs() < 1e-12, "mapped power {g}");
    }
}

/// Points well inside each cited rectangle land in the intended region, on
/// the assembled mesh (cell lookup, not the painting function alone).
#[test]
fn assembled_region_map_matches_the_cited_extents() {
    let opts = Htr10RzOptions::default();
    let mesh = build_mesh(&opts).unwrap();
    let at = |r: f64, z: f64| {
        let c = mesh
            .cell_at(Length::new::<centimeter>(r), Length::new::<centimeter>(z))
            .unwrap();
        mesh.cell_region(c)
    };
    let cases = [
        (45.0, 90.0, idx::BED),
        (80.0, 184.0, idx::BED),
        (86.0, 184.0, idx::BOTTOM_REFL_SOLID),
        (10.0, 400.0, idx::DISCHARGE_TUBE),
        (10.0, 495.0, idx::DISCHARGE_THROTTLE),
        (45.0, -20.0, idx::CORE_ENTRY_PLENUM),
        (45.0, -50.0, idx::TOP_FLOW_ZONE),
        (45.0, -70.0, idx::TOP_PLENUM),
        (10.0, -70.0, idx::TOP_BOTTOM_GRAPHITE),
        (45.0, -100.0, idx::TOP_BOTTOM_GRAPHITE),
        (45.0, -150.0, idx::CARBON_BRICK),
        (45.0, -250.0, idx::TOP_HE_CAVITY),
        (45.0, -347.0, idx::RPV),
        (92.0, 90.0, idx::SIDE_INNER_GRAPHITE),
        (100.0, 90.0, idx::CR_RING),
        (100.0, 225.0, idx::CR_EXIT_THROTTLE),
        (120.0, 90.0, idx::SIDE_GRAPHITE),
        (144.0, 90.0, idx::RISERS),
        (144.0, 443.0, idx::RISERS),
        (160.0, 90.0, idx::SIDE_GRAPHITE),
        (180.0, 90.0, idx::CARBON_BRICK),
        (192.0, 90.0, idx::GAP_BRICK_VESSEL),
        (195.5, 90.0, idx::CORE_VESSEL),
        (205.0, 90.0, idx::GAP_VESSEL_RPV),
        (214.0, 90.0, idx::RPV),
        (205.0, 310.0, idx::HE_ENTRY_PATH),
        (205.0, 400.0, idx::ANNULAR_PATH),
        (50.0, 210.0, idx::UPPER_BOTTOM_REFL),
        (30.0, 250.0, idx::BOTTOM_REFL_SOLID),
        (60.0, 230.0, idx::SMALL_PLENUM),
        (60.0, 260.0, idx::LOWER_BOTTOM_REFL),
        (60.0, 310.0, idx::HOT_PLENUM),
        (100.0, 330.0, idx::TOP_BOTTOM_GRAPHITE),
        (45.0, 400.0, idx::LEAK_STEEL),
        (100.0, 400.0, idx::CARBON_BRICK),
        (100.0, 443.0, idx::BOTTOM_STEEL_2),
        (100.0, 450.0, idx::BOTTOM_ROUNDING_HE),
        (100.0, 480.0, idx::BOTTOM_STEEL_1),
        (45.0, 480.0, idx::BOTTOM_ANNULAR_HE),
        (30.0, 480.0, idx::BOTTOM_STEEL_2),
        (100.0, 600.0, idx::BOTTOM_HE_CAVITY),
        (100.0, 661.0, idx::RPV),
    ];
    for (r, z, want) in cases {
        assert_eq!(at(r, z), want, "region at r {r}, z {z} cm");
    }
    // Every model region is present in the assembled mesh.
    let mut seen = [false; N_REGIONS];
    for c in 0..mesh.n_cells() {
        seen[mesh.cell_region(c)] = true;
    }
    assert!(seen.iter().all(|s| *s), "missing regions: {seen:?}");
    // The Tab 2.3 barrel placement moves the vessel to r 191..194.
    let (d, rects) = region_rects(CoreVesselPlacement::Tab23);
    assert_eq!(region_at_cm(&rects, d, 192.5, 90.0), idx::CORE_VESSEL);
    assert_eq!(region_at_cm(&rects, d, 195.5, 90.0), idx::GAP_VESSEL_RPV);
}

/// Derived check: the core and cone bed cells hold the Tab 2.3 core volume,
/// 5 m^3 "including the cone-shaped zone" (the R-Z steps give 5.04 m^3).
#[test]
fn core_volume_matches_tab_2_3() {
    let mesh = build_mesh(&Htr10RzOptions::default()).unwrap();
    let v: f64 = (0..mesh.n_cells())
        .filter(|c| {
            mesh.cell_region(*c) == idx::BED
        })
        .map(|c| mesh.mesh().cell_volumes[c])
        .sum();
    let hand = std::f64::consts::PI
        * (0.9 * 0.9 * 1.8
            + 0.075 * (0.835f64.powi(2) + 0.705f64.powi(2) + 0.6f64.powi(2) + 0.5f64.powi(2) + 0.39f64.powi(2)));
    println!("core + cone volume {v:.4} m^3 (hand {hand:.4}, Tab 2.3: 5 m^3)");
    assert!((v / hand - 1.0).abs() < 1e-12);
    assert!((v / 5.0 - 1.0).abs() < 0.01);
}

/// The tabulated properties reproduce the input sheet's Derived values and
/// the holds behave as documented.
#[test]
fn tabulated_properties_reproduce_the_input_sheet() {
    let o = MaterialOptions::default();
    let k = |m: RegionMaterial, t: f64| m.conductivity_model(&o).unwrap().at(kel(t)).get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
    // Breitbach bed (input sheet Sec 4.2 table): 300/500/800/1000 C.
    for (tc, want) in [(300.0, 2.44), (500.0, 4.79), (800.0, 9.39), (1000.0, 13.11)] {
        let got = k(RegionMaterial::PebbleBed, tc + 273.15);
        assert!((got - want).abs() < 0.006, "bed {tc} C: {got} vs {want}");
    }
    // Side reflector, D = 0 (Sec 4.4): 500 C 77.5, 800 C 75.4; held at the
    // 300 C value (84.2) below 300 C by default.
    let side = RegionMaterial::SideReflector { solid_fraction: 1.0 };
    assert!((k(side, 773.15) - 77.5).abs() < 0.06);
    assert!((k(side, 1073.15) - 75.4).abs() < 0.06);
    let at300 = k(side, 573.15);
    assert!((at300 - 84.2).abs() < 0.06);
    assert_eq!(k(side, 473.15), at300);
    let o2 = MaterialOptions {
        side_reflector_below_300c: SideReflectorBelow300C::TopBottomFit,
        ..o
    };
    let tb = side.conductivity_model(&o2).unwrap().at(kel(473.15)).get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
    let tb_direct = k(RegionMaterial::TopBottomReflector { solid_fraction: 1.0 }, 473.15);
    assert!((tb - tb_direct).abs() < 1e-9, "{tb} vs {tb_direct}");
    // Carbon brick (Sec 4.5): 5.0 at 0 C, 8.0 at 1000 C.
    assert!((k(RegionMaterial::CarbonBrick, 273.15) - 5.0).abs() < 1e-9);
    assert!((k(RegionMaterial::CarbonBrick, 1273.15) - 8.0).abs() < 1e-9);
    // The gh:#1150 brick ablation: a constant replaces Tab 2.12.
    let ob = MaterialOptions {
        carbon_brick: CarbonBrickConductivity::Constant(
            uom::si::f64::ThermalConductivity::new::<uom::si::thermal_conductivity::watt_per_meter_kelvin>(30.0),
        ),
        ..o
    };
    assert_eq!(RegionMaterial::CarbonBrick.conductivity_si(700.0, &ob).unwrap(), 30.0);
    // Cavity: helium plus 4 sigma T^3 delta / (2/eps - 1), by hand at a table node.
    let cav = RegionMaterial::HeliumCavity {
        emissivity: 0.85,
        width_m: 0.13,
    };
    let he = crate::htgr_closures::KtaHelium::new(
        uom::si::f64::Pressure::new::<uom::si::pressure::bar>(30.0),
        kel(803.15),
    )
    .unwrap()
    .thermal_conductivity()
    .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
    let hand = he + 4.0 * SIGMA * 803.15f64.powi(3) * 0.13 / (2.0 / 0.85 - 1.0);
    assert!((k(cav, 803.15) / hand - 1.0).abs() < 1e-12);
    // ZBS bed sits above Breitbach (gh:#1148 record: +32..107 %).
    let oz = MaterialOptions {
        bed: BedConductivityModel::Zbs(ZbsBed::htr10()),
        ..o
    };
    let z = RegionMaterial::PebbleBed.conductivity_si(1073.15, &oz).unwrap();
    assert!(z > 1.3 * 9.39, "{z}");
}

/// Hand values of the boundary emissivities.
#[test]
fn boundary_emissivities_hand_values() {
    let direct = RccsView::DirectToPanels.effective_emissivity();
    let hand = 1.0 / (1.0 / 0.85 + 2.18 / 2.854 * (1.0 / 0.85 - 1.0));
    assert!((direct - hand).abs() < 1e-15);
    assert!((direct - 0.7626).abs() < 5e-5, "{direct}");
    let shield = RccsView::InsulatingPlateShield.effective_emissivity();
    println!("eps_eff direct {direct:.4}, with shield {shield:.4}");
    // A thin two-sided shield of the same emissivity: hand value 0.4252.
    assert!((shield - 0.4252).abs() < 5e-5, "{shield}");
}

/// The base case is the documented default (D3, the 300 C hold, the
/// direct RCCS view, radiating heads, Fig 2.5 barrel, 5 cm cells).
#[test]
fn base_case_defaults_are_pinned() {
    let o = Htr10RzOptions::default();
    assert_eq!(o.materials.bed, BedConductivityModel::Breitbach);
    assert_eq!(o.materials.side_reflector_below_300c, SideReflectorBelow300C::HoldAt300C);
    assert_eq!(o.materials.helium_pressure_bar, 30.0);
    assert_eq!(o.rccs, RccsView::DirectToPanels);
    assert_eq!(o.heads, HeadBoundary::RadiateTo50C);
    assert_eq!(o.vessel, CoreVesselPlacement::Fig25);
    assert_eq!(o.max_cell.unwrap().get::<centimeter>(), 5.0);
    // gh:#1173: cavity natural convection is ON by default, ablatable.
    assert!(o.cavity_convection);
    assert!(matches!(o.rpv_side_boundary(), SolidBoundary::RadiativeAndConvective { .. }));
    assert!(matches!(o.without_cavity_convection().rpv_side_boundary(), SolidBoundary::Radiative { .. }));
    // Through the ordinary build and solve: the side wall carries more of
    // the same 100 kW with convection, at a cooler RPV, and the balance
    // (radiation + convection per face) closes.
    let mut side = Vec::new();
    for opts in [o, o.without_cavity_convection()] {
        let mut m = Htr10RzModel::build(
            Htr10RzOptions { max_cell: None, ..opts },
            Htr10RzCase::DecayPower { power: Power::new::<watt>(1.0e5) },
        )
        .unwrap();
        let r = m.solve_steady().unwrap();
        assert!(r.relative_imbalance < 1e-6, "{}", r.relative_imbalance);
        side.push(r.rccs_side.get::<watt>());
    }
    println!("coarse 100 kW: RCCS side {:.2} kW with cavity convection, {:.2} kW without", side[0] / 1e3, side[1] / 1e3);
    assert!(side[0] > side[1]);
}

/// gh:#1173: the RPV-cavity natural-convection coefficient against a hand
/// evaluation of MacGregor & Emery (1969) `Nu_L = 0.046 Ra_L^(1/3)` with
/// air from Incropera & DeWitt 6th ed. Tab A.4, linearly interpolated to
/// the 373.15 K film (nu 23.46e-6 m^2/s, alpha 33.79e-6 m^2/s, k 31.76e-3
/// W/(m K)): RPV 150 deg C, panels 50 deg C, `L = 0.674 m` ->
/// `Ra_L = 1.015e9`, `Nu_L = 46.2`, plane `h = 2.18`, curvature factor
/// `2/(1 + 2.18/2.854) = 1.134` -> **2.47 W/(m^2 K)**. The model's air is
/// CoolProp's, not the table's, hence the 3 % tolerance. Also: no
/// difference, no convection beyond the conduction floor (`Nu = 1`).
#[test]
fn rpv_cavity_convection_hand_value() {
    let e = rpv_cavity_enclosure();
    let h = e.coefficient_si(423.15, 323.15).unwrap();
    let ra = e.rayleigh_number(423.15, 323.15).unwrap();
    println!("RPV cavity: Ra_L {ra:.3e}, h {h:.3} W/(m^2 K) at 150/50 deg C");
    for (ts, label) in [(373.15, "100"), (473.15, "200"), (523.15, "250")] {
        println!("  RPV {label} C: h {:.3} W/(m^2 K), Ra_L {:.3e}", e.coefficient_si(ts, 323.15).unwrap(), e.rayleigh_number(ts, 323.15).unwrap());
    }
    assert!((ra / 1.015e9 - 1.0).abs() < 0.05, "{ra:e}");
    assert!((h / 2.471 - 1.0).abs() < 0.03, "{h}");
    let floor = e.coefficient_si(323.15, 323.15).unwrap();
    let k_air = 0.0280; // Tab A.4 at 323 K, W/(m K)
    assert!((floor / (k_air / 0.674 * 2.0 / (1.0 + 2.18 / 2.854)) - 1.0).abs() < 0.05, "{floor}");
}

/// gh:#1173: the in-crate air properties (Sutherland / White, ideal gas,
/// Incropera Tab A.4 `c_p`) against CoolProp's `Air` (Helmholtz EOS and
/// its transport model, the workspace port) at 1 atm, 300-600 K. Measured
/// 2026-11-03: worst `nu` / `alpha` / `k` difference printed below; gate
/// 3 % each (the correlation's own uncertainty is far larger).
#[test]
fn air_properties_against_coolprop() {
    use crate::genfoam::thermal_hydraulics::solid_conduction::air_properties;
    use outram_park_fork_coolprop::{conductivity, state_pt, viscosity, Fluid};
    let mut worst = [0.0_f64; 3];
    for t in [300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0] {
        let (nu, alpha, k) = air_properties(t, 101_325.0).unwrap();
        let st = state_pt(Fluid::Air, t, 101_325.0).unwrap();
        let mu_c = viscosity(Fluid::Air, t, st.density).unwrap();
        let k_c = conductivity(Fluid::Air, t, st.density).unwrap();
        let (nu_c, alpha_c) = (mu_c / st.density, k_c / (st.density * st.cp));
        let d = [nu / nu_c - 1.0, alpha / alpha_c - 1.0, k / k_c - 1.0];
        println!("{t} K: nu {:+.2} %, alpha {:+.2} %, k {:+.2} %", 100.0 * d[0], 100.0 * d[1], 100.0 * d[2]);
        for i in 0..3 {
            worst[i] = worst[i].max(d[i].abs());
        }
    }
    println!("worst |diff|: nu {:.2} %, alpha {:.2} %, k {:.2} %", 100.0 * worst[0], 100.0 * worst[1], 100.0 * worst[2]);
    assert!(worst.iter().all(|w| *w < 0.03), "{worst:?}");
    assert!(air_properties(0.0, 101_325.0).is_err());
}

/// Both cases solve on the coarse Fig 2.5 mesh and close their energy
/// balance; bad inputs are refused.
#[test]
fn coarse_solves_close_the_energy_balance() {
    let opts = Htr10RzOptions {
        max_cell: None,
        ..Htr10RzOptions::default()
    };
    let mut held = Htr10RzModel::build(opts, Htr10RzCase::HeldBed { temperature: kel(950.0) }).unwrap();
    let r = held.solve_steady().unwrap();
    println!(
        "coarse held 950 K: {} cells, RCCS {:.1} kW, heads {:.1}/{:.1} kW, supplied {:.1} kW, imbalance {:.1e}, picard {}",
        r.n_cells,
        r.rccs_side.get::<watt>() / 1e3,
        r.top_head.get::<watt>() / 1e3,
        r.bottom_head.get::<watt>() / 1e3,
        r.supplied_by_held_bed.get::<watt>() / 1e3,
        r.relative_imbalance,
        r.picard.iterations
    );
    assert!(r.relative_imbalance < 1e-6);
    assert!((r.max_temperature.get::<kelvin>() - 950.0).abs() < 1e-6);
    assert!(held.temperature_at_cm(45.0, 90.0).is_some());
    let mut dec = Htr10RzModel::build(
        opts,
        Htr10RzCase::DecayPower {
            power: Power::new::<watt>(2.0e5),
        },
    )
    .unwrap();
    let r = dec.solve_steady().unwrap();
    println!(
        "coarse 200 kW: RCCS {:.1} kW, max {:.1} K at {:?}, flags {}",
        r.rccs_side.get::<watt>() / 1e3,
        r.max_temperature.get::<kelvin>(),
        r.max_at_cm,
        r.range_flags().len()
    );
    assert!(r.relative_imbalance < 1e-6);
    let mut mean = Htr10RzModel::build(opts, Htr10RzCase::BedMeanTemperature { temperature: kel(950.0) }).unwrap();
    let r = mean.solve_steady().unwrap();
    println!(
        "coarse mean 950 K: power {:.2} kW in {} secant passes, bed mean {:.5} K",
        r.generated.get::<watt>() / 1e3,
        r.power_iterations,
        r.bed_mean_temperature.get::<kelvin>()
    );
    assert!((r.bed_mean_temperature.get::<kelvin>() - 950.0).abs() < 1e-4);
    assert!(r.relative_imbalance < 1e-6);
    assert!(Htr10RzModel::build(opts, Htr10RzCase::BedMeanTemperature { temperature: kel(300.0) }).is_err());
    assert!(Htr10RzModel::build(opts, Htr10RzCase::DecayPower { power: Power::new::<watt>(-1.0) }).is_err());
    assert!(Htr10RzModel::build(opts, Htr10RzCase::HeldBed { temperature: kel(0.0) }).is_err());
    assert_eq!(dec.regions().len(), N_REGIONS);
    assert!(dec.options().max_cell.is_none());
}

// ------------------------------------------------------------ gh:#1150 --

use super::flow::{gas, solve_gas, BypassSplit, GasNetwork, OperatingPoint};
use super::fpic::thermocouples;
use super::pebble::{pebble_profile, PebbleMatrix};

/// The embedded thermocouple table is byte-identical to the input sheet's
/// CSV, and parses to 22 rows with the sheet's spot values.
#[test]
fn thermocouple_copy_matches_the_input_sheet_and_parses() {
    let sheet = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reactor-scoping/htr10-th-data/fpic-thermocouples-tab7-1.csv");
    let ours = include_str!("data/fpic-thermocouples-tab7-1.csv");
    match std::fs::read_to_string(&sheet) {
        Ok(s) => assert_eq!(s, ours, "the embedded Tab 7.1 copy drifted from the input sheet"),
        Err(_) => println!("input sheet not present in this checkout; copy not compared"),
    }
    let tcs = thermocouples();
    assert_eq!(tcs.len(), 22);
    assert_eq!((tcs[4].r_cm, tcs[4].z_cm), (117.0, 80.0));
    assert_eq!(tcs[21].measured_c, 881.7);
    assert_eq!(tcs[0].participants[10], None, "TC 1 has no UK-WIMSTER value");
    assert_eq!(tcs[20].participant_range(), (379.0, 820.0));
}

/// The pebble sub-model: the port run in the `r_core -> 0` limit reproduces
/// the closed-form full sphere (unheated shell `Q/(4 pi k)(1/r_m - 1/r_s)`,
/// heated sphere `q r_m^2/(6 k)`) to 1e-9 relative, and the TRISO step and
/// the Maxwell reduction have the right sign.
#[test]
fn pebble_profile_is_the_full_sphere_limit() {
    let (ts, q) = (1000.0, 2.84e6);
    let h = pebble_profile(ts, q, PebbleMatrix::Tab29Homogeneous).unwrap();
    let k = h.fuel_zone_conductivity;
    let pp = q * 4.0 / 3.0 * std::f64::consts::PI * 0.03f64.powi(3) / 0.61;
    assert!((h.pebble_power / pp - 1.0).abs() < 1e-12);
    let shell = pp / (4.0 * std::f64::consts::PI * k) * (1.0 / 0.025 - 1.0 / 0.03);
    let qm = pp / (4.0 / 3.0 * std::f64::consts::PI * 0.025f64.powi(3));
    let fz = qm * 0.025 * 0.025 / (6.0 * k);
    println!(
        "pebble at 2.84 W/cm^3, surface 1000 K: P {pp:.1} W, k {k:.2}, shell {shell:.3} K, fuel zone {fz:.3} K, kernel step {:.3} K",
        h.kernel_max - h.centre
    );
    assert!(((h.fuel_zone_surface - ts) / shell - 1.0).abs() < 1e-9);
    assert!(((h.centre - h.fuel_zone_surface) / fz - 1.0).abs() < 1e-9);
    assert!(h.benchmark_centre < h.centre && h.centre - h.benchmark_centre < 0.1);
    assert!(h.kernel_max > h.centre);
    let m = pebble_profile(ts, q, PebbleMatrix::MaxwellOnTab29).unwrap();
    assert!(m.fuel_zone_conductivity < k, "Maxwell lowers the matrix conductivity");
    assert!(m.centre > h.centre);
}

fn coarse_net(split: BypassSplit) -> (Htr10RzModel, GasNetwork) {
    let m = Htr10RzModel::build(
        Htr10RzOptions {
            max_cell: None,
            ..Htr10RzOptions::default()
        },
        Htr10RzCase::DecayPower {
            power: Power::new::<watt>(1.0e7),
        },
    )
    .unwrap();
    let net = GasNetwork::build(m.field().mesh(), split).unwrap();
    (m, net)
}

/// The gas network on the Fig 2.5 mesh: it assembles, a bad split is
/// refused, an isothermal solid at the inlet temperature leaves the gas at
/// the inlet temperature with no exchange, and a uniformly hotter solid
/// heats it, with the enthalpy rise equal to the exchanged heat (mass and
/// energy conservation of the network).
#[test]
fn gas_network_conserves_mass_and_energy() {
    let (m, net) = coarse_net(BypassSplit::default());
    println!("Fig 2.5 mesh: {} 2-D gas cells, {} chain nodes", net.n_gas_cells(), net.n_chain_nodes());
    assert!(net.n_gas_cells() > 50 && net.n_chain_nodes() > 10);
    let mut bad = BypassSplit::default();
    bad.leakage = uom::si::f64::Ratio::new::<uom::si::ratio::ratio>(0.2);
    assert!(GasNetwork::build(m.field().mesh(), bad).is_err());
    let op = OperatingPoint::fpic_specification();
    let n = m.field().mesh().n_cells();
    let t_in = op.inlet.get::<kelvin>();
    let g = solve_gas(&net, &op, &vec![t_in; n], None).unwrap();
    println!(
        "isothermal: outlet {:.9} K, heat {:.3e} W, bed dp {:.1} Pa, Picard {:?}",
        g.outlet_temperature, g.heat_to_gas, g.core_pressure_drop, g.flow_picard
    );
    assert!((g.outlet_temperature - t_in).abs() < 1e-6);
    // Round-off of sum h a_v V (T_s - T_g) over ~1e5 W/K of exchange.
    assert!(g.heat_to_gas.abs() < 1.0, "{}", g.heat_to_gas);
    assert!(g.core_pressure_drop > 0.0);
    let g = solve_gas(&net, &op, &vec![t_in + 200.0; n], Some(&g)).unwrap();
    println!(
        "solid +200 K: outlet {:.3} K, heat {:.1} W, W cp dT {:.1} W",
        g.outlet_temperature, g.heat_to_gas, g.enthalpy_rise
    );
    assert!(g.outlet_temperature > t_in && g.outlet_temperature < t_in + 200.0);
    assert!((g.heat_to_gas / g.enthalpy_rise - 1.0).abs() < 1e-6);
    // Helium at the inlet: density from the Tab 2.8 fit (bar reading).
    let he = gas(523.15, 30.0).unwrap();
    assert!((he.rho - 2.74).abs() < 0.05, "{}", he.rho);
}

/// The coupled FPIC case on the Fig 2.5 mesh converges and closes the
/// plant energy balance (10 MW = RCCS + heads + W c_p dT).
#[test]
fn coarse_fpic_closes_the_plant_energy_balance() {
    let mut f = super::Htr10FpicModel::build(super::FpicOptions {
        solid: Htr10RzOptions {
            max_cell: None,
            ..Htr10RzOptions::default()
        },
        ..super::FpicOptions::default()
    })
    .unwrap();
    let r = f.solve().unwrap();
    println!(
        "coarse FPIC: {} passes, outlet {:.2} C, loss {:.1} kW, imbalance {:.2e}, max fuel {:.1} C",
        r.outer_iterations,
        r.outlet_temperature.get::<kelvin>() - 273.15,
        r.heat_to_rccs_and_heads.get::<watt>() / 1e3,
        r.plant_imbalance,
        r.max_fuel_centre.0.benchmark_centre - 273.15
    );
    assert!(r.plant_imbalance < 1e-5, "{}", r.plant_imbalance);
    assert_eq!(r.thermocouples.len(), 22);
}
