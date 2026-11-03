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
