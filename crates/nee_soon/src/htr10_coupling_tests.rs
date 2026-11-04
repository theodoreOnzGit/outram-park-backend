// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Unit tests of the HTR-10 direct coupling's pieces (gh:#1186). The coupled
//! Monte Carlo run itself is `examples/htr10_direct_coupling.rs`: one Picard
//! iteration processes the nuclear data at new temperatures (minutes) and
//! transports the whole core, far over a test's budget (measured costs in
//! `verification_and_validation/htr10_direct_coupling/README.md`).

use super::*;
use uom::si::f64::{HeatCapacity, ThermalConductance};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::megawatt;
use uom::si::thermal_conductance::watt_per_kelvin;

fn k(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}

/// Relaxation, rounding to the quantum and the range clamp.
#[test]
fn relaxation_rounds_and_clamps() {
    let t_in = ZoneTemperatures::uniform(k(600.0));
    let t_th = t_in.with(Htr10Zone::FuelPebble, k(1000.6)).with(Htr10Zone::Coolant, k(5000.0));
    let one = TemperatureInterval::new::<temperature_interval::kelvin>(1.0);
    let out = relax_and_quantise(&t_in, &t_th, Ratio::new::<ratio>(0.5), one);
    assert_eq!(out.kelvin(Htr10Zone::FuelPebble), 800.0); // 800.3 -> 800
    assert_eq!(out.kelvin(Htr10Zone::ModeratorPebble), 600.0);
    assert_eq!(out.kelvin(Htr10Zone::Coolant), max_temperature().get::<kelvin>());
    let exact = relax_and_quantise(&t_in, &t_th, Ratio::new::<ratio>(1.0), TemperatureInterval::new::<temperature_interval::kelvin>(0.0));
    assert_eq!(exact.kelvin(Htr10Zone::FuelPebble), 1000.6);
    assert_eq!(max_difference(&t_in, &exact), max_temperature().get::<kelvin>() - 600.0);
}

/// The R-Z regions fall into the Monte Carlo zones by height.
#[test]
fn rz_regions_map_to_monte_carlo_zones() {
    assert_eq!(rz_zone_of(idx::TOP_BOTTOM_GRAPHITE, -100.0), Some(Htr10Zone::TopReflector));
    assert_eq!(rz_zone_of(idx::SIDE_GRAPHITE, 90.0), Some(Htr10Zone::SideReflector));
    assert_eq!(rz_zone_of(idx::BOTTOM_REFL_SOLID, 250.0), Some(Htr10Zone::BottomReflector));
    assert_eq!(rz_zone_of(idx::CARBON_BRICK, 0.0), Some(Htr10Zone::BoronatedBricks));
    assert_eq!(rz_zone_of(idx::BED, 50.0), None);
    assert_eq!(rz_zone_of(idx::RPV, 50.0), None);
}

/// **The TH half of an iteration on the R-Z core** (coarse mesh): a Monte
/// Carlo-like shape is put on the heated cells with the power conserved, the
/// steady state is solved, and the zone temperatures come back ordered as a
/// heated core must order them.
#[test]
fn rz_target_receives_the_power_and_returns_zone_temperatures() {
    let study = ParametricStudy::default();
    let target = ThermalTarget::htr10_rz_fpic_coarse().unwrap();
    let mut c = Htr10DirectCoupling::new(study, target, Power::new::<megawatt>(10.0));
    // A cosine over the N = 17 bed (half-height 86.283 cm).
    let h = Length::new::<centimeter>(86.283);
    let shape = PowerShape::J0Cosine {
        centre: Length::new::<centimeter>(0.0),
        extrapolated_radius: Length::new::<centimeter>(110.0),
        extrapolated_height: Length::new::<centimeter>(230.0),
    };
    let t_in = ZoneTemperatures::uniform(k(600.0));
    let (t, peaking) = c.thermal(&shape, FrameMap::new(h), &t_in).unwrap();
    let ThermalTarget::Rz(target) = c.target() else { panic!() };
    let core = &target.core;
    let vol = &core.solid().field().mesh().mesh().cell_volumes;
    let p: f64 = core
        .heated_cells()
        .iter()
        .map(|&c| core.cell_power_density(c).get::<watt_per_cubic_meter>() * vol[c])
        .sum();
    assert!((p / 1.0e7 - 1.0).abs() < 1e-10, "{p} W");
    let kz = |z| t.kelvin(z);
    println!("zones: {:?}, peaking {:?}", t.to_kelvin(), peaking);
    assert!(kz(Htr10Zone::FuelPebble) > kz(Htr10Zone::ModeratorPebble));
    assert!(kz(Htr10Zone::ModeratorPebble) > kz(Htr10Zone::SideReflector));
    assert!(kz(Htr10Zone::SideReflector) > kz(Htr10Zone::BoronatedBricks));
    // Gas: the mean of 250 deg C and the outlet.
    assert!(kz(Htr10Zone::Coolant) > 523.15 && kz(Htr10Zone::Coolant) < 1000.0);
    assert!(peaking.get::<ratio>() > 1.0);
}

/// **The lumped target**: GeN-Foam's node marched to its steady state,
/// `T = T_sink + P/G`, given to the fuel and the moderator pebbles.
#[test]
fn lumped_target_reaches_its_steady_state() {
    let g = 2.0e4;
    let thermal = LumpedThermal::new(
        "core",
        HeatCapacity::new::<joule_per_kelvin>(1.0e6),
        ThermalConductance::new::<watt_per_kelvin>(g),
        k(500.0),
        k(500.0),
        5.0,
    );
    let target = ThermalTarget::Lumped {
        thermal,
        volume: Volume::new::<cubic_meter>(5.0),
        time_step: Time::new::<second>(50.0),
    };
    let mut c = Htr10DirectCoupling::new(ParametricStudy::default(), target, Power::new::<megawatt>(10.0));
    let t_in = ZoneTemperatures::uniform(k(600.0));
    let (t, _) = c.thermal(&PowerShape::Uniform, FrameMap::new(Length::new::<centimeter>(60.0)), &t_in).unwrap();
    let want = 500.0 + 1.0e7 / g;
    assert!((t.kelvin(Htr10Zone::FuelPebble) - want).abs() < 1e-3, "{}", t.kelvin(Htr10Zone::FuelPebble));
    assert_eq!(t.kelvin(Htr10Zone::ModeratorPebble), t.kelvin(Htr10Zone::FuelPebble));
    assert_eq!(t.kelvin(Htr10Zone::SideReflector), 600.0);
}

/// The default power mesh covers the cylindrical bed, and a coupling with a
/// relaxation outside `(0, 1]` is refused before any work.
#[test]
fn power_mesh_and_refusals() {
    let (r, z) = Htr10DirectCoupling::default_power_mesh(Length::new::<centimeter>(61.788));
    assert_eq!(r.first().unwrap().get::<centimeter>(), 0.0);
    assert_eq!(r.last().unwrap().get::<centimeter>(), 90.0);
    assert!((z.first().unwrap().get::<centimeter>() + 61.788).abs() < 1e-12);
    assert!((z.last().unwrap().get::<centimeter>() - 61.788).abs() < 1e-12);
    assert_eq!(z.len(), 13);
    let target = ThermalTarget::Lumped {
        thermal: LumpedThermal::new(
            "c",
            HeatCapacity::new::<joule_per_kelvin>(1.0),
            ThermalConductance::new::<watt_per_kelvin>(1.0),
            k(500.0),
            k(500.0),
            1.0,
        ),
        volume: Volume::new::<cubic_meter>(1.0),
        time_step: Time::new::<second>(1.0),
    };
    let mut c = Htr10DirectCoupling::new(ParametricStudy::default(), target, Power::new::<megawatt>(1.0))
        .with_relaxation(Ratio::new::<ratio>(1.5));
    assert!(c.solve(Convergence::default(), |_| {}).is_err());
    assert_eq!(c.cached_jobs(), 0);
}

/// The history picture is written.
#[test]
fn history_draws() {
    let t = ZoneTemperatures::uniform(k(600.0));
    let it = |i: usize, kk: f64| PicardIteration {
        iteration: i,
        zone_temperatures_in: t,
        k_eff: Ratio::new::<ratio>(kk),
        k_std: Ratio::new::<ratio>(0.001),
        zone_temperatures_th: t.with(Htr10Zone::FuelPebble, k(900.0)),
        zone_temperatures_out: t.with(Htr10Zone::FuelPebble, k(750.0)),
        temperature_residual: TemperatureInterval::new::<temperature_interval::kelvin>(300.0),
        k_change: None,
        peaking: Ratio::new::<ratio>(1.4),
        majorant_violations: 0,
        jobs_processed: 0,
        jobs_cached: 0,
        processing_time: Time::new::<second>(0.0),
        transport_time: Time::new::<second>(0.0),
        thermal_time: Time::new::<second>(0.0),
    };
    let dir = std::env::temp_dir().join("nee_soon_htr10_coupling_test");
    std::fs::create_dir_all(&dir).unwrap();
    write_history_png(&[it(1, 1.01), it(2, 1.0)], dir.join("h.png")).unwrap();
    assert!(std::fs::metadata(dir.join("h.png")).unwrap().len() > 100);
}
