//! # V&V: wall heat and volumetric heating in `HemPipe` (gh:#1062)
//!
//! Predictions written before the first run (2026-11-01): in a closed rigid
//! pipe (walls at both ends) the total energy rises by exactly the heat put
//! in, to round-off: `q''·P·L·t` for a prescribed flux, `r·V·t` for a
//! volumetric source, and `h·P·L·(T_w − T)·dt` per step for a wall
//! temperature. Adiabatic (the default) adds nothing.

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

fn closed_gas_pipe() -> HemPipe<IdealGasHem> {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        20,
    )
    .unwrap();
    let s = PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(1.0e5),
        density: MassDensity::new::<kilogram_per_cubic_meter>(1.16),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Uniform(s),
        HemNumerics::default(),
    )
    .unwrap()
}

/// **Energy balance of each heat source in a closed pipe.** Air, 1 m, area
/// 0.01 m² (wall area per length `P = 2√(πA) = 0.35449` m), 20 cells, walls.
/// Flux 1e4 W/m² for 1 ms: `ΔE = q''·P·L·t`. Volumetric 1e6 W/m³ for 1 ms:
/// `ΔE = r·A·L·t`. Wall at 400 K, `h = 1000` W/(m²K), one 1e-5 s
/// forward-Euler step from a uniform 300.4 K gas: `ΔE = h·P·L·(T_w − T)·dt`.
/// **Result (2026-11-01):** each asserted to 1e-9 relative.
#[test]
fn each_heat_source_adds_exactly_its_heat() {
    let p_wall = 2.0 * (std::f64::consts::PI * 0.01_f64).sqrt();

    let mut flux = closed_gas_pipe();
    assert_eq!(flux.wall_heat(), WallHeat::Adiabatic);
    flux.set_wall_heat(WallHeat::HeatFlux(HeatFluxDensity::new::<
        watt_per_square_meter,
    >(1.0e4)))
        .unwrap();
    let e0 = flux.total_energy().value;
    flux.run_until(Time::new::<second>(1.0e-3)).unwrap();
    let de = flux.total_energy().value - e0;
    let expect = 1.0e4 * p_wall * 1.0 * 1.0e-3;
    println!("flux: dE {de:.6} J against {expect:.6} J");
    assert!((de / expect - 1.0).abs() < 1e-9);

    let mut vol = closed_gas_pipe();
    vol.set_volumetric_heating(VolumetricPowerDensity::new::<watt_per_cubic_meter>(1.0e6))
        .unwrap();
    let e0 = vol.total_energy().value;
    vol.run_until(Time::new::<second>(1.0e-3)).unwrap();
    let de = vol.total_energy().value - e0;
    let expect = 1.0e6 * 0.01 * 1.0 * 1.0e-3;
    println!("volumetric: dE {de:.6} J against {expect:.6} J");
    assert!((de / expect - 1.0).abs() < 1e-9);

    let numerics = HemNumerics {
        time_integrator: TimeIntegrator::ForwardEuler,
        ..HemNumerics::default()
    };
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        20,
    )
    .unwrap();
    let s = PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(1.0e5),
        density: MassDensity::new::<kilogram_per_cubic_meter>(1.16),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    let mut wall = HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Uniform(s),
        numerics,
    )
    .unwrap();
    let t_gas = wall.cell_state(0).unwrap().temperature.get::<kelvin>();
    wall.set_wall_heat(WallHeat::WallTemperature {
        temperature: ThermodynamicTemperature::new::<kelvin>(400.0),
        heat_transfer_coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(1000.0),
    })
    .unwrap();
    let e0 = wall.total_energy().value;
    wall.step_by(Time::new::<second>(1.0e-5)).unwrap();
    let de = wall.total_energy().value - e0;
    let expect = 1000.0 * p_wall * 1.0 * (400.0 - t_gas) * 1.0e-5;
    println!("wall temperature: T_gas {t_gas:.3} K, dE {de:.6e} J against {expect:.6e} J");
    assert!((de / expect - 1.0).abs() < 1e-9);

    let mut bad = closed_gas_pipe();
    assert!(bad
        .set_wall_heat(WallHeat::HeatFlux(HeatFluxDensity::new::<
            watt_per_square_meter,
        >(f64::NAN)))
        .is_err());
    assert!(bad
        .set_wall_heat(WallHeat::WallTemperature {
            temperature: ThermodynamicTemperature::new::<kelvin>(-1.0),
            heat_transfer_coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(1.0),
        })
        .is_err());
    assert!(bad
        .set_volumetric_heating(VolumetricPowerDensity::new::<watt_per_cubic_meter>(
            f64::INFINITY
        ))
        .is_err());
}
