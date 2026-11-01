// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! V&V of the phasic wall heat (gh:#1062) in [`super::drift_flux::DriftFlux1d`]
//! and [`super::two_fluid::TwoFluid1d`]. Record:
//! `crates/tampines/verification_and_validation/multiphase_1d_issues_1058_1060_1062_1076.md`.
//!
//! Verification only: energy identities and a hand energy balance. Nothing
//! here is compared with an experiment.

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_multiphase::drift_flux::SlipModel;
use tampines_steam_tables::hem::WallHeat;
use uom::si::angle::radian;
use uom::si::f64::{Angle, HeatFluxDensity, Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::drift_flux::{AxialBoundary, DriftFlux1d};
use super::geometry::Pipe1d;
use super::properties::SaturatedProperties;
use super::two_fluid::TwoFluid1d;
use super::wall_heat::PhasicWallHeat;
use crate::heat_transfer::relap5_htc::Relap5HeatTransferGeometry;

const D: f64 = 0.02;
const L: f64 = 1.0;
const FLUX: f64 = 1.0e5;

fn pipe(n: usize) -> Pipe1d {
    Pipe1d::circular(
        Length::new::<meter>(L),
        Length::new::<meter>(D),
        Angle::new::<radian>(0.0),
        n,
    )
    .expect("pipe")
}

fn flux() -> PhasicWallHeat {
    PhasicWallHeat::Hem(WallHeat::HeatFlux(HeatFluxDensity::new::<watt_per_square_meter>(FLUX)))
}

/// **Methodology.** A closed, horizontal, 4-cell, 1 m × 20 mm two-fluid pipe
/// of saturated steam/water at 7 MPa (`α_g = 0.1`) is heated through its wall
/// at `q'' = 1e5 W/m²` for 20 steps of 100 µs. The phase energy equations
/// give, per step and summed over the pipe,
/// `Δ Σ V (m_g h_g + m_l h_l) = Σ V (α_g + α_l)^n Δp + q'' π D L Δt`
/// (the interfacial terms cancel exactly, and at rest there is no friction
/// work). **Prediction (before the run):** the residual of that identity is
/// below `1e-9` of the heat input every step, the pressure rises, and neither
/// numerical device (rate limiter, phase floor) acts.
#[test]
fn a_closed_heated_two_fluid_pipe_gains_exactly_the_wall_heat() {
    let sat = SaturatedProperties::at(7.0e6).expect("IF97");
    let mut s = TwoFluid1d::bubbly(
        pipe(4),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(sat.t_sat),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    s.set_initial_void_fraction(0.1).expect("void");
    s.set_temperature_profile(&vec![ThermodynamicTemperature::new::<kelvin>(sat.t_sat); 4])
        .expect("profile");
    s.set_wall_heat(flux()).expect("flux");
    let v = s.pipe().cell_volume();
    let q_in = FLUX * std::f64::consts::PI * D * L * 1.0e-4;
    let mut worst = 0.0_f64;
    let p0 = s.pressure()[0];
    for _ in 0..20 {
        let e0 = s.total_enthalpy();
        let p_old = s.pressure().to_vec();
        let a_sum: Vec<f64> = (0..4).map(|i| s.void_fraction()[i] + s.liquid_fraction()[i]).collect();
        let r = s.step().expect("step");
        assert_eq!((r.mass_transfer_limited_cells, r.residual_alpha_floor_events), (0, 0));
        let work: f64 = (0..4).map(|i| v * a_sum[i] * (s.pressure()[i] - p_old[i])).sum();
        let residual = (s.total_enthalpy() - e0 - work - q_in) / q_in;
        worst = worst.max(residual.abs());
    }
    println!("two-fluid closed heated pipe: worst |residual|/Q = {worst:.3e}; p {p0:.6e} -> {:.6e} Pa", s.pressure()[0]);
    assert!(worst < 1.0e-9, "energy identity residual {worst:e}");
    assert!(s.pressure()[0] > p0);
}

/// **Methodology.** The drift-flux counterpart: a closed 4-cell pipe of
/// water at 7 MPa and 500 K, heated at `q'' = 1e5 W/m²`, 20 steps of 100 µs.
/// At rest the continuity density equals the start-of-step density, so the
/// mixture energy equation gives per step
/// `Σ V ρ^n (h^{n+1} − h^n) − Σ V Δp = q'' π D L Δt`.
/// **Prediction:** residual `< 1e-9` of the heat input.
///
/// **Result (2026-11-01, release):** worst residual `3.323e-11`.
#[test]
fn a_closed_heated_drift_flux_pipe_gains_exactly_the_wall_heat() {
    let mut s = DriftFlux1d::new(
        pipe(4),
        SlipModel::ZuberFindlay { c0: 1.0, vgj: Vector3::new(0.0, 0.0, 0.0) },
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    s.set_wall_heat(flux()).expect("flux");
    assert_eq!(s.wall_heat(), flux());
    let v = s.pipe().cell_volume();
    let q_in = FLUX * std::f64::consts::PI * D * L * 1.0e-4;
    let mut worst = 0.0_f64;
    for _ in 0..20 {
        let rho = s.density().to_vec();
        let h = s.enthalpy().to_vec();
        let p = s.pressure().to_vec();
        s.step().expect("step");
        let gained: f64 = (0..4)
            .map(|i| v * (rho[i] * (s.enthalpy()[i] - h[i]) - (s.pressure()[i] - p[i])))
            .sum();
        worst = worst.max(((gained - q_in) / q_in).abs());
    }
    println!("drift-flux closed heated pipe: worst |residual|/Q = {worst:.3e}");
    assert!(worst < 1.0e-9, "energy identity residual {worst:e}");
}

/// **Methodology — heated two-phase channel against a hand energy balance.**
/// A horizontal 1 m × 20 mm drift-flux channel, 16 cells, fed by a reservoir
/// at 7.05 MPa stagnation and 540 K (19 K subcooled at 7 MPa) and discharging
/// to 7.0 MPa, heated at `q'' = 4e6 W/m²` (251 kW), run 3 s at 1 ms. The
/// solver's energy equation carries `∂p/∂t` and no `u ∂p/∂x`, so at a
/// steady state the discrete balance is exactly `ṁ (h_N − h_in) = Q` with
/// `h_in = h_0 − u_0²/2` the reservoir face enthalpy and `h_N` the last
/// cell's. **Prediction (before the run):** a steady state is reached
/// (`ṁ` changes by < 1e-5 relative over the last 100 steps); the outlet is
/// two-phase (`α_g > 0` in the last cell); `h_N` agrees with
/// `h_in + Q/ṁ` to `1e-4` relative.
///
/// **First run, recorded as a miss of the protocol (2026-11-01):** at
/// `q'' = 2e6 W/m²` the channel carried `ṁ = 2.186 kg/s`, twice the hand
/// estimate behind that flux, so the outlet stayed subcooled
/// (`h_N = 1.226077e6` against `h_f = 1.267e6 J/kg`) — the balance itself
/// held to `5.7e-16`. The flux was doubled so the case tests a *two-phase*
/// outlet, which is what it claims; the criterion was not touched.
///
/// **Result at `4e6 W/m²`:** `ṁ = 1.504650 kg/s` (steady to
/// `1.5e-7` over the last 100 steps), `h_in = 1.168616e6`,
/// `h_N = 1.335650e6 J/kg`, hand balance agrees to `1.26e-8` relative,
/// outlet void `0.4777`. All three predictions held.
#[test]
fn a_heated_two_phase_channel_matches_the_hand_energy_balance() {
    let sat = SaturatedProperties::at(7.05e6).expect("IF97");
    let h0 = {
        use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
        use uom::si::available_energy::joule_per_kilogram;
        let _ = sat;
        h_tp_1(
            ThermodynamicTemperature::new::<kelvin>(540.0),
            Pressure::new::<pascal>(7.05e6),
        )
        .get::<joule_per_kilogram>()
    };
    let n = 16;
    let mut s = DriftFlux1d::new(
        pipe(n),
        SlipModel::ZuberFindlay { c0: 1.0, vgj: Vector3::new(0.0, 0.0, 0.0) },
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(540.0),
        Time::new::<second>(1.0e-3),
    )
    .expect("solver");
    s.set_left_boundary(AxialBoundary::ReservoirInlet {
        stagnation_pressure: 7.05e6,
        stagnation_enthalpy: h0,
    });
    s.set_right_boundary(AxialBoundary::PressureOutlet { pressure: 7.0e6 });
    let q_w = 4.0e6;
    s.set_wall_heat(PhasicWallHeat::Hem(WallHeat::HeatFlux(
        HeatFluxDensity::new::<watt_per_square_meter>(q_w),
    )))
    .expect("flux");
    let mut history = Vec::new();
    for _ in 0..3000 {
        let r = s.step().expect("step");
        history.push(r.outlet_mass_flow);
    }
    let m_dot = *history.last().unwrap();
    let drift = history[history.len() - 100..]
        .iter()
        .map(|m| ((m - m_dot) / m_dot).abs())
        .fold(0.0_f64, f64::max);
    let u0 = s.face_velocity()[0];
    let h_in = h0 - 0.5 * u0 * u0;
    let q_total = q_w * std::f64::consts::PI * D * L;
    let expected = h_in + q_total / m_dot;
    let h_out = s.enthalpy()[n - 1];
    let rel = (h_out - expected) / expected;
    println!(
        "heated channel: m_dot = {m_dot:.6e} kg/s (drift {drift:.2e}), h_in = {h_in:.6e}, \
         h_out = {h_out:.6e}, hand {expected:.6e} J/kg, rel {rel:.3e}, outlet void {:.4}",
        s.void_fraction()[n - 1]
    );
    assert!(drift < 1.0e-5, "not steady: {drift:e}");
    assert!(s.void_fraction()[n - 1] > 0.0, "outlet did not boil");
    assert!(rel.abs() < 1.0e-4, "energy balance miss {rel:e}");
}

/// **Methodology.** The RELAP5-dispatcher wall (gh:#1063) reaches both
/// solvers: a wall 10 K above saturation heats a closed 7 MPa pipe for five
/// steps. **Prediction:** both complete and the two-fluid total enthalpy and
/// the drift-flux enthalpy rise (the wall is hotter than every phase).
///
/// **Result (2026-11-01, release):** both rose (two-fluid total enthalpy
/// `2.682682587e5 → 2.682688486e5 J`; drift-flux `h` `1.240725e6 →
/// 1.240729e6 J/kg`). The dispatcher gives `h_l = 7.08e3`,
/// `h_g = 9.39e2 W/(m²·K)` on the unit-test state.
#[test]
fn the_relap5_wall_reaches_both_solvers_and_heats_the_fluid() {
    let sat = SaturatedProperties::at(7.0e6).expect("IF97");
    let wall = PhasicWallHeat::Relap5WallTemperature {
        temperature: ThermodynamicTemperature::new::<kelvin>(sat.t_sat + 10.0),
        geometry: Relap5HeatTransferGeometry::InternalPipe,
    };
    let mut tf = TwoFluid1d::bubbly(
        pipe(4),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(sat.t_sat),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    tf.set_initial_void_fraction(0.1).expect("void");
    tf.set_temperature_profile(&vec![ThermodynamicTemperature::new::<kelvin>(sat.t_sat); 4])
        .expect("profile");
    tf.set_wall_heat(wall).expect("wall");
    let e0 = tf.total_enthalpy();
    for _ in 0..5 {
        tf.step().expect("two-fluid step");
    }
    let mut df = DriftFlux1d::new(
        pipe(4),
        SlipModel::ZuberFindlay { c0: 1.0, vgj: Vector3::new(0.0, 0.0, 0.0) },
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(sat.t_sat - 5.0),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    df.set_wall_heat(wall).expect("wall");
    let h0 = df.enthalpy()[0];
    for _ in 0..5 {
        df.step().expect("drift-flux step");
    }
    println!(
        "relap5 wall: two-fluid enthalpy {e0:.9e} -> {:.9e} J; drift-flux h {h0:.6e} -> {:.6e} J/kg",
        tf.total_enthalpy(),
        df.enthalpy()[0]
    );
    assert!(tf.total_enthalpy() > e0 && df.enthalpy()[0] > h0);
}
