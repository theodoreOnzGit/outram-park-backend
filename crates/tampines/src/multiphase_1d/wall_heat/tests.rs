// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Closed-form verification of [`super::PhasicWallHeat`] (gh:#1062).

use super::*;
use uom::si::f64::{HeatFluxDensity, HeatTransfer};

fn phase(t: f64, rho: f64, u: f64) -> WallPhaseState {
    WallPhaseState {
        temperature: t,
        density: rho,
        viscosity: 1.0e-4,
        conductivity: 0.6,
        cp: 4500.0,
        expansion: 1.0e-3,
        velocity: u,
    }
}

/// **Methodology.** Hand closed forms at `D_h = 0.05 m` (`4/D_h = 80 1/m`),
/// `α_g = 0.3`: a `1e5 W/m²` flux gives `8e6 W/m³`, split `0.3 / 0.7` by the
/// reused linear contact partition, and the shares sum to the prescribed heat
/// to round-off; a `500 K` wall with `h = 1e4 W/(m²·K)` against a `480 K`
/// vapour and a `490 K` liquid gives `0.3·1e4·20·80 = 4.8e6` and
/// `0.7·1e4·10·80 = 5.6e6 W/m³`; the adiabatic wall gives `(0, 0)`.
/// **Prediction:** each to `1e-12` relative.
#[test]
fn heat_flux_and_wall_temperature_split_by_the_linear_contact_partition() {
    let (v, l) = (phase(480.0, 30.0, 2.0), phase(490.0, 750.0, 1.0));
    let q = PhasicWallHeat::Hem(WallHeat::HeatFlux(HeatFluxDensity::new::<watt_per_square_meter>(1.0e5)));
    let (qg, ql) = q.phasic_heat(0.3, 0.05, v, l).expect("flux");
    assert!((qg - 0.3 * 8.0e6).abs() < 1e-12 * 8.0e6 && (ql - 0.7 * 8.0e6).abs() < 1e-12 * 8.0e6);
    assert!(((qg + ql) - 8.0e6).abs() < 1e-9);
    let tw = PhasicWallHeat::Hem(WallHeat::WallTemperature {
        temperature: ThermodynamicTemperature::new::<kelvin>(500.0),
        heat_transfer_coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(1.0e4),
    });
    let (qg, ql) = tw.phasic_heat(0.3, 0.05, v, l).expect("wall T");
    assert!((qg - 4.8e6).abs() < 1e-6 && (ql - 5.6e6).abs() < 1e-6);
    assert_eq!(PhasicWallHeat::default().phasic_heat(0.3, 0.05, v, l).unwrap(), (0.0, 0.0));
    assert!(PhasicWallHeat::default().is_adiabatic() && !tw.is_adiabatic());
    assert!(tw.validate().is_ok());
    let bad = PhasicWallHeat::Hem(WallHeat::HeatFlux(HeatFluxDensity::new::<watt_per_square_meter>(f64::NAN)));
    assert!(matches!(bad.validate(), Err(TampinesError::InvalidInput(_))));
}

/// **Methodology.** The RELAP5 variant must hand each phase's own properties
/// to the gh:#1063 dispatcher and nothing else: the liquid share must equal
/// `(1 − α) h_l (T_w − T_l) 4/D_h` with `h_l` from a direct
/// `relap5_wall_heat_transfer` call on the same inputs. **Prediction:**
/// bit-identical.
#[test]
fn the_relap5_variant_uses_the_dispatcher_on_each_phase() {
    let (v, l) = (phase(480.0, 30.0, 2.0), phase(490.0, 750.0, 1.0));
    let bc = PhasicWallHeat::Relap5WallTemperature {
        temperature: ThermodynamicTemperature::new::<kelvin>(500.0),
        geometry: Relap5HeatTransferGeometry::InternalPipe,
    };
    let (qg, ql) = bc.phasic_heat(0.3, 0.05, v, l).expect("relap5");
    let h_l = relap5_h(Relap5HeatTransferGeometry::InternalPipe, 500.0, 0.05, l).unwrap();
    let h_g = relap5_h(Relap5HeatTransferGeometry::InternalPipe, 500.0, 0.05, v).unwrap();
    assert_eq!(ql, 0.7 * h_l * 10.0 * 80.0);
    assert_eq!(qg, 0.3 * h_g * 20.0 * 80.0);
    println!("RELAP5 h_l = {h_l:.6e}, h_g = {h_g:.6e} W/(m^2 K)");
    assert!(h_l > 0.0 && h_g > 0.0);
}
