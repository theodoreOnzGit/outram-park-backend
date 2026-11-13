// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # V&V: a heat-structure surface facing a gas, with condensation (gh:#1096)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! `tampines::components::wall_condensation` (MELCOR 2.2 HS RM §2.6.1,
//! §2.7, moved verbatim from `sembawang`). The bit-for-bit comparison with
//! `sembawang`'s original, and the drywell wall on `sembawang`'s inputs, are
//! in `crates/sembawang/tests/wall_condensation_bit_identity.rs` (`sembawang`
//! depends on `tampines`, never the reverse). Record:
//! `crates/tampines/verification_and_validation/wall_condensation.md`.
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. **Natural-convection limit, no steam, by hand.** N2 at 0.1 MPa and
//!    400 K on a 300 K wall. Film 350 K: `mu = 3.9539785e-7 350^0.67288 =
//!    2.036e-5` Pa s, `k = 3.704689e-4 350^0.74842 = 0.02970` W/(m K); bulk
//!    `rho = 0.8423` kg/m³; `c_p = (c_v(400) + R)/M = 1028` J/(kg K) (`c_v =
//!    1117 − 2.88e5/400 + 5.348e7/400² = 731.25` J/(kg K)); `Pr = 0.705`.
//!    At `L = 0.5` m, `Ra = 4.22e8` (laminar), `Nu = 0.59 Ra^1/4 = 84.6`,
//!    `h = 5.03` W/(m² K); at `L = 5` m, `Ra = 4.22e11` (turbulent),
//!    `Nu = 0.10 Ra^1/3 = 750`, `h = 4.46` W/(m² K). The code within 1 % of
//!    each pencil number, and within 1e-12 of the same equations coded
//!    independently in the test; no condensation. Against Churchill and
//!    Chu's vertical-plate correlation (Incropera Eq 9.26) at `L = 0.5` m,
//!    `Nu = 94.3`: MELCOR's laminar form about 10 % lower, inside ±15 %.
//! 2. **Energy closure.** A steel slab (3 axial cells) facing 420 K gas of
//!    0.15 MPa steam, 0.12 MPa N2 and 0.03 MPa H2, its surface at 320 K
//!    (dew point 384.5 K: condensing): the gas's loss (sensible plus `ṁ h_g`)
//!    equals the wall's stored-energy gain plus the condensate's enthalpy,
//!    to < 1e-12 relative, on both enthalpy conventions; the latent heat
//!    dominates the sensible heat (ratio > 3).
//! 3. **The step limit.** With about 1 g of steam in the gas the condensation is
//!    scaled to exactly 90 % of it in the step (1e-15 relative).
//! 4. **Ablations.** The fixed-coefficient ablation condenses nothing and its
//!    heat is `−heat_to_fluid` exactly; the latent heat per kilogram on the
//!    containment's constant-`c_v` convention is within 0.5 % of IF97's at
//!    the state of 2 (by hand: IF97 `h_g(420 K, 0.15 MPa) ≈ 2.764e6`,
//!    `h_l(320 K) ≈ 1.97e5`; containment `2.770e6` and `1.961e5` J/kg).

use petir::mathf::RealMath;
use tampines::components::heat_structure::*;
use tampines::components::wall_condensation::*;
use uom::si::amount_of_substance::mole;
use uom::si::f64::*;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;

const R: f64 = 8.314_462;
const M_N2: f64 = 0.028_014;

fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}
fn pa(p: f64) -> Pressure {
    Pressure::new::<pascal>(p)
}
fn m(l: f64) -> Length {
    Length::new::<meter>(l)
}

/// A steel slab: 2 cm in 4 cells, k = 50 W/(m K), ρc = 3.9e6 J/(m³ K), 10 m
/// wide, `n_ax` axial cells of 2 m, uniformly at `t0`.
fn steel(n_ax: usize, t0: f64) -> HeatStructure {
    HeatStructure::new(
        StructureShape::Slab { width: m(10.0) },
        &[ConductionZone {
            thickness: m(0.02),
            n_cells: 4,
            conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(50.0),
            volumetric_heat_capacity: VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(3.9e6),
        }],
        n_ax,
        m(2.0),
        kel(t0),
    )
    .unwrap()
}

/// **Prediction 1, the natural-convection limit by hand.** **Result
/// (2026-11-01), held:** `L = 0.5` m: `h = 5.02480` W/(m² K) against the
/// pencil 5.03 (−0.10 %), `Ra = 4.2249e8`, `Nu = 84.587`; `L = 5` m:
/// `h = 4.45744` against 4.46 (−0.06 %), `Ra = 4.2249e11`, `Nu = 750.364`;
/// the independent coding agrees to < 1e-12; no condensation. Churchill and
/// Chu give `Nu = 94.225`: MELCOR's laminar form −10.23 % (predicted about
/// −10 %, inside ±15 %).
#[test]
fn with_no_steam_the_wall_sees_melcors_natural_convection_by_hand() {
    let gas = WallGas::from_partial_pressures(kel(400.0), pa(0.0), pa(1.0e5), pa(0.0), Volume::new::<cubic_meter>(100.0));
    let model = WallCondensation::new();
    // The same equations, coded here independently of the module.
    let independent = |l: f64| {
        let (tg, tw) = (400.0_f64, 300.0_f64);
        let tf = 0.5 * (tg + tw);
        let mu = 3.953_978_5e-7 * tf.r_powf(0.672_88);
        let k = 3.704_689e-4 * tf.r_powf(0.748_42);
        let rho = 1.0e5 * M_N2 / (R * tg);
        let cv = (1117.0 - 2.880e5 / tg + 5.348e7 / (tg * tg)) * M_N2;
        let cp = (cv + R) / M_N2;
        let ra = 9.81 * (tg - tw) / tf * l * l * l * rho * rho / (mu * mu) * (mu * cp / k);
        let nu = if ra <= 1e9 { 0.59 * ra.r_powf(0.25) } else if ra >= 1e10 { 0.10 * ra.r_powf(1.0 / 3.0) } else { f64::NAN };
        (nu * k / l, ra, nu, mu, k, cp)
    };
    for (l, pencil) in [(0.5, 5.03), (5.0, 4.46)] {
        let (h, m_flux) = model.surface(&gas, kel(300.0), m(l));
        let h = h.get::<watt_per_square_meter_kelvin>();
        let (h_ind, ra, nu, mu, k, cp) = independent(l);
        println!("L = {l} m: h = {h:.5} W/(m2 K) (pencil {pencil}, independent {h_ind:.5}); Ra = {ra:.4e}, Nu = {nu:.3}, mu {mu:.5e}, k {k:.5}, cp {cp:.2}");
        assert_eq!(m_flux, 0.0);
        assert!(((h - h_ind) / h_ind).abs() < 1e-12, "{h} {h_ind}");
        assert!(((h - pencil) / pencil).abs() < 0.01, "{h} against the pencil {pencil}");
    }
    // Churchill and Chu (Incropera Eq 9.26) at L = 0.5 m.
    let (h, ra, _, mu, k, cp) = independent(0.5);
    let pr = mu * cp / k;
    let cc = (0.825 + 0.387 * ra.r_powf(1.0 / 6.0) / (1.0 + (0.492 / pr).r_powf(9.0 / 16.0)).r_powf(8.0 / 27.0)).r_powf(2.0);
    let nu = h * 0.5 / k;
    println!("Churchill-Chu Nu = {cc:.3}; MELCOR {nu:.3} ({:+.2} %)", 100.0 * (nu / cc - 1.0));
    assert!((nu / cc - 1.0).abs() < 0.15);
}

fn humid_gas(steam_pa: f64) -> WallGas {
    WallGas::from_partial_pressures(kel(420.0), pa(steam_pa), pa(1.2e5), pa(0.3e5), Volume::new::<cubic_meter>(1000.0))
}

/// **Prediction 2, energy closure** (and the reported condensate). **Result
/// (2026-11-01), held:** IF97: `ṁ = 0.341902` kg/s, sensible 73 054 W,
/// latent to the wall 878 717 W, condensate 67 157 W, gas loss 1 018 928 W;
/// closure −1.5e-14 relative. Containment convention: the same `ṁ` and
/// sensible heat, latent 880 058 W, closure −8.9e-14. Latent/sensible 12.0
/// (> 3). `h = 12.183` W/(m² K).
#[test]
fn the_gas_loses_what_the_wall_gains_plus_the_condensate_carries() {
    let gas = humid_gas(1.5e5);
    for model in [WallCondensation::new(), WallCondensation::new().without_real_steam()] {
        let mut wall = steel(3, 320.0);
        let dt = Time::new::<second>(1.0);
        let x = model.exchange(&wall, StructureSide::Outer, &gas, m(6.0), dt).unwrap();
        assert!(!x.limited && x.condensation_rate.get::<kilogram_per_second>() > 0.0, "{x:?}");
        let e0 = wall.stored_energy();
        wall.step(dt, &StructureBoundary::Adiabatic, &x.boundary).unwrap();
        let gain = wall.stored_energy() - e0;
        let loss = x.gas_energy_loss.get::<watt>();
        let carried = x.condensate_enthalpy_flow.get::<watt>();
        let (conv, lat) = (x.convective_power.get::<watt>(), x.latent_power_to_wall.get::<watt>());
        let closure = (loss - gain - carried) / loss;
        let parts = (conv + lat + carried - loss) / loss;
        println!(
            "{:?}: ṁ = {:.6} kg/s, h_g = {:.1} J/kg, sensible {conv:.2} W, latent to wall {lat:.2} W, condensate {carried:.2} W, gas loss {loss:.2} W, wall gain {gain:.2} J; closure {closure:.2e}, parts {parts:.2e}; h = {:.4} W/(m2 K)",
            model.condensate_enthalpy(),
            x.condensation_rate.get::<kilogram_per_second>(),
            x.steam_enthalpy.value,
            x.heat_transfer_coefficient[0].get::<watt_per_square_meter_kelvin>()
        );
        assert!(closure.abs() < 1e-12 && parts.abs() < 1e-12);
        assert!(lat > 3.0 * conv && conv > 0.0);
        let per_cell: f64 = x.condensation_per_cell.iter().map(|r| r.get::<kilogram_per_second>()).sum();
        assert_eq!(per_cell, x.condensation_rate.get::<kilogram_per_second>());
    }
    assert_eq!(WallCondensation::default(), WallCondensation::new());
    assert_eq!(WallCondensation::new().surface_model(), SurfaceModel::Melcor);
}

/// **Prediction 3, the 90 % step limit.** **Result (2026-11-01), held**
/// (after the instrument fix below): limited; 6.964414e-4 kg condensed of
/// 7.738238e-4 kg, 90 % to < 1e-15.
#[test]
fn condensation_is_limited_to_ninety_percent_of_the_steam_in_a_step() {
    // The same composition in a 1 L gas space: about 0.77 g of steam. (The
    // first run instead cut the steam's moles alone, which also cut its mole
    // fraction to nothing, so nothing condensed and the limit was never
    // reached: an instrument error, corrected without touching the
    // criterion.)
    let gas = WallGas::from_partial_pressures(kel(420.0), pa(1.5e5), pa(1.2e5), pa(0.3e5), Volume::new::<cubic_meter>(1.0e-3));
    assert!(gas.steam.get::<mole>() > 0.0);
    let x = WallCondensation::new().exchange(&steel(3, 320.0), StructureSide::Outer, &gas, m(6.0), Time::new::<second>(1.0)).unwrap();
    let condensed = x.condensation_rate.get::<kilogram_per_second>() * 1.0;
    let available = steam_mass(&gas).get::<kilogram>();
    println!("limited {}: condensed {condensed:.6e} kg of {available:.6e} kg", x.limited);
    assert!(x.limited && ((condensed - 0.9 * available) / available).abs() < 1e-15);
}

/// **Prediction 4, the ablations.** **Result (2026-11-01), held:** the
/// fixed coefficient condenses nothing and its heat is `−heat_to_fluid` bit
/// for bit; latent heat per kilogram 2 570 082 J/kg (IF97, `h_g =
/// 2 766 503`) against 2 574 004 J/kg (containment, `h_g = 2 770 118`),
/// +0.153 % (inside 0.5 %).
#[test]
fn the_ablations_do_what_they_say() {
    let gas = humid_gas(1.5e5);
    let wall = steel(3, 320.0);
    let dt = Time::new::<second>(1.0);
    let h = HeatTransfer::new::<watt_per_square_meter_kelvin>(10.0);
    let fixed = WallCondensation::new().without_melcor_surface(h);
    assert_eq!(fixed.surface_model(), SurfaceModel::FixedCoefficient(h));
    let x = fixed.exchange(&wall, StructureSide::Outer, &gas, m(6.0), dt).unwrap();
    assert_eq!(x.condensation_rate.get::<kilogram_per_second>(), 0.0);
    let StructureBoundary::HeatInflow(q) = &x.boundary else { panic!() };
    for (j, qj) in q.iter().enumerate() {
        assert_eq!(*qj, -wall.heat_to_fluid(StructureSide::Outer, j, 420.0, 10.0));
    }
    // Latent heat per kilogram on the two conventions.
    let per_kg = |model: WallCondensation| {
        let x = model.exchange(&wall, StructureSide::Outer, &gas, m(6.0), dt).unwrap();
        (x.latent_power_to_wall.get::<watt>() / x.condensation_rate.get::<kilogram_per_second>(), x.steam_enthalpy.value)
    };
    let (if97, hg_if97) = per_kg(WallCondensation::new());
    let (cv, hg_cv) = per_kg(WallCondensation::new().without_real_steam());
    println!("latent per kg: IF97 {if97:.1} (h_g {hg_if97:.1}), containment {cv:.1} (h_g {hg_cv:.1}) J/kg, {:+.3} %", 100.0 * (cv / if97 - 1.0));
    assert!((cv / if97 - 1.0).abs() < 0.005);
    assert_eq!(WallCondensation::new().without_real_steam().condensate_enthalpy(), CondensateEnthalpy::ContainmentConstantCv);
    // Invalid input is refused.
    assert!(WallCondensation::new().exchange(&wall, StructureSide::Outer, &gas, m(0.0), dt).is_err());
}
