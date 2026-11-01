// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! V&V of the two-fluid pressure and inflow boundaries and their
//! time-dependent versions (gh:#1076). Record:
//! `crates/tampines/verification_and_validation/multiphase_1d_issues_1058_1060_1062_1076.md`.
//! Verification only.

use tampines_steam_tables::hem::{TimeTable, WallHeat};
use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
use uom::si::angle::radian;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Angle, HeatFluxDensity, Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::geometry::Pipe1d;
use super::properties::{PhaseState, SaturatedProperties};
use super::two_fluid::{
    TwoFluid1d, TwoFluidBoundary, TwoFluidInflow, TwoFluidInflowThermo,
    TwoFluidTimeDependentBoundary,
};
use super::wall_heat::PhasicWallHeat;

const D: f64 = 0.02;

fn solver(l: f64, n: usize, dt: f64) -> TwoFluid1d {
    let pipe = Pipe1d::circular(
        Length::new::<meter>(l),
        Length::new::<meter>(D),
        Angle::new::<radian>(0.0),
        n,
    )
    .expect("pipe");
    let sat = SaturatedProperties::at(7.0e6).expect("IF97");
    let mut s = TwoFluid1d::bubbly(
        pipe,
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(sat.t_sat),
        Time::new::<second>(dt),
    )
    .expect("solver");
    s.set_initial_void_fraction(0.1).expect("void");
    s.set_temperature_profile(&vec![ThermodynamicTemperature::new::<kelvin>(sat.t_sat); n])
        .expect("profile");
    s
}

/// **Methodology.** A velocity inlet (`u = 1 m/s`, liquid at 540 K, inflow
/// void 0.05) feeds a 4-cell, 1 m × 20 mm two-fluid pipe of saturated
/// steam/water at 7 MPa, which discharges through a 7 MPa pressure outlet.
/// For 20 steps of 100 µs the mass and enthalpy the solver reports through
/// the inlet face are compared with the hand values from the prescribed
/// state at the first cell's start-of-step pressure `p`:
/// `ṁ = (α ρ_g^sat + (1 − α) ρ_l(p, h_l)) u A`,
/// `Ė = (α ρ_g^sat h_g^sat + (1 − α) ρ_l h_l) u A`, `h_l = h(p, 540 K)`.
/// **Prediction (before the run):** both agree to `1e-12` relative every
/// step.
///
/// **Result (2026-11-01, release):** worst mass `1.2e-16`, worst energy
/// `2.1e-16`.
#[test]
fn a_velocity_inlet_delivers_exactly_the_prescribed_mass_and_energy() {
    let mut s = solver(1.0, 4, 1.0e-4);
    let inflow = TwoFluidInflow {
        thermo: TwoFluidInflowThermo::LiquidTemperature(540.0),
        void_fraction: 0.05,
    };
    s.set_left_boundary(TwoFluidBoundary::VelocityInlet { velocity: 1.0, inflow });
    s.set_right_boundary(TwoFluidBoundary::PressureOutlet { pressure: 7.0e6 });
    let a = s.pipe().face_area_si(0);
    let (mut worst_m, mut worst_e) = (0.0_f64, 0.0_f64);
    for _ in 0..20 {
        let p = s.pressure()[0];
        let sat = SaturatedProperties::at(p).expect("IF97");
        let h_l = h_tp_1(
            ThermodynamicTemperature::new::<kelvin>(540.0),
            Pressure::new::<pascal>(p),
        )
        .get::<joule_per_kilogram>();
        let rho_l = PhaseState::liquid_at(p, h_l, sat).expect("liquid").density;
        let (m_g, m_l) = (0.05 * sat.rho_g, 0.95 * rho_l);
        let m_hand = (m_g + m_l) * a;
        let e_hand = (m_g * sat.h_g + m_l * h_l) * a;
        let r = s.step().expect("step");
        worst_m = worst_m.max(((r.inlet_mass_flow - m_hand) / m_hand).abs());
        worst_e = worst_e.max(((r.inlet_energy_flow - e_hand) / e_hand).abs());
    }
    println!("velocity inlet: worst mass {worst_m:.3e}, worst energy {worst_e:.3e}");
    assert!(worst_m < 1.0e-12 && worst_e < 1.0e-12);
}

/// **Methodology.** A time-dependent boundary built from constant
/// [`TimeTable`]s must be the static boundary exactly: a pressure inlet
/// (7.02 MPa, liquid at 555 K, void 0.1) and a pressure outlet (7.0 MPa),
/// 50 steps, static against time-dependent. Also the evaluation itself: a
/// pressure ramp `7 → 8 MPa` over 1 s read at `t = 0.25 s`.
/// **Prediction:** bit-identical fields; the ramp reads `7.25 MPa` to
/// round-off.
///
/// **Result (2026-11-01, release):** every report and field bit-identical
/// over 50 steps; the ramp reads 7.25 MPa; the 20 kPa drop drives
/// `u_l = 0.132 m/s` into the pipe after 50 steps.
#[test]
fn a_constant_time_dependent_boundary_equals_the_static_one() {
    let inflow = TwoFluidInflow {
        thermo: TwoFluidInflowThermo::LiquidTemperature(555.0),
        void_fraction: 0.1,
    };
    let mut a = solver(1.0, 4, 1.0e-4);
    a.set_left_boundary(TwoFluidBoundary::PressureInlet { pressure: 7.02e6, inflow });
    a.set_right_boundary(TwoFluidBoundary::PressureOutlet { pressure: 7.0e6 });
    let mut b = solver(1.0, 4, 1.0e-4);
    b.set_left_time_dependent_boundary(TwoFluidTimeDependentBoundary::PressureInlet {
        pressure: TimeTable::constant(7.02e6),
        liquid_temperature: TimeTable::constant(555.0),
        void_fraction: TimeTable::constant(0.1),
    });
    b.set_right_time_dependent_boundary(TwoFluidTimeDependentBoundary::PressureOutlet {
        pressure: TimeTable::constant(7.0e6),
    });
    for _ in 0..50 {
        let ra = a.step().expect("static");
        let rb = b.step().expect("time-dependent");
        assert_eq!(ra, rb);
    }
    assert_eq!(a.pressure(), b.pressure());
    assert_eq!(a.void_fraction(), b.void_fraction());
    assert_eq!(a.vapour_face_velocity(), b.vapour_face_velocity());
    assert_eq!(a.liquid_face_velocity(), b.liquid_face_velocity());
    let ramp = TwoFluidTimeDependentBoundary::PressureOutlet {
        pressure: TimeTable::new(&[(0.0, 7.0e6), (1.0, 8.0e6)]).expect("table"),
    };
    match ramp.at(Time::new::<second>(0.25)) {
        TwoFluidBoundary::PressureOutlet { pressure } => {
            assert!((pressure - 7.25e6).abs() < 1e-6, "{pressure}")
        }
        other => panic!("wrong shape {other:?}"),
    }
    let vin = TwoFluidTimeDependentBoundary::VelocityInlet {
        velocity: TimeTable::constant(2.0),
        liquid_temperature: TimeTable::constant(540.0),
        void_fraction: TimeTable::constant(0.0),
    };
    assert_eq!(
        vin.at(Time::new::<second>(3.0)),
        TwoFluidBoundary::VelocityInlet {
            velocity: 2.0,
            inflow: TwoFluidInflow {
                thermo: TwoFluidInflowThermo::LiquidTemperature(540.0),
                void_fraction: 0.0
            }
        }
    );
    println!(
        "first value now in force on the left: {:?}; u_l[0] = {:.6e} m/s",
        b.left_boundary(),
        b.liquid_face_velocity()[0]
    );
    assert!(b.liquid_face_velocity()[0] > 0.0, "the 20 kPa drop drove no inflow");
}

/// **Methodology — pressure-driven heated two-phase channel.** A 0.5 m ×
/// 20 mm two-fluid channel, 4 cells, fed through a pressure inlet at
/// 7.02 MPa (saturated liquid at that pressure, inflow void 0.1) and leaving
/// through a 7.0 MPa pressure outlet, heated at `q'' = 2e5 W/m²`
/// (`Q = 6.283 kW`), `Δt = 100 µs`. The phase energy equations summed over
/// the pipe give, every step,
/// `ΔE/Δt − Σ V α Δp/Δt = Ė_in − Ė_out + Q + D`, with `D ≥ 0` the drag
/// dissipation of gh:#1060; at a steady state `Ė_out − Ė_in = Q + D`.
///
/// **Predictions (before the first run):** the march completes; the flow is
/// positive and steady (`|ṁ_in − ṁ_out|/ṁ < 1e-3`, `ṁ` of order 1–2 kg/s);
/// after 3000 steps `Ė_out − Ė_in` exceeds `Q` by less than 1 % (slip of
/// order 0.1 m/s, so `D` negligible).
///
/// **First run (2026-11-01), recorded as a miss:** after 3000 steps (0.3 s)
/// `ṁ = 2.58 kg/s` with the in/out spread `4.4e-4`, but
/// `Ė_out − Ė_in = 5.13 kW` against `Q = 6.28 kW` (**−18 %**). A per-step
/// budget printed during diagnosis closed to within ~240 W every step. The
/// gap was therefore **storage**, not a defect: the channel's enthalpy was
/// still relaxing, about 1.2 kW at 0.3 s and decaying. The run was too
/// short. While diagnosing I attributed the ~240 W per-step residual to drag
/// dissipation (3.8 % of `Q`). **The longer run shows that was wrong:** at
/// steady state the residual is 0.23 W (`3.7e-5 Q`). The 240 W was a
/// transient effect whose origin was **not** diagnosed. The slip is 0.40 m/s,
/// not the 0.1 m/s I predicted.
///
/// **Protocol as revised after that run:** 15 000 steps (1.5 s), keeping the
/// original gates (steady to `1e-3`, excess `< 1 %` of `Q`). Two gates are
/// added: (1) the per-step budget residual equals `D` and so must be
/// non-negative; (2) the steady excess equals the end-of-run residual `D/Q`
/// within `1e-2`.
///
/// **Result:** `ṁ = 3.181829 kg/s`, in/out spread `4.1e-8`;
/// `Ė_out − Ė_in = 6.283289 kW` against `Q = 6.283185 kW`, an excess of
/// `+1.64e-5`; end-of-run `D/Q = 3.7e-5`; smallest per-step residual
/// `+5.4e-5 W` (never negative); max void 0.105, slip 0.395 m/s; no limiter
/// or floor activation. With the longer run, the original predictions held.
#[test]
fn pressure_boundaries_drive_a_heated_channel_that_balances_its_energy() {
    let mut s = solver(0.5, 4, 1.0e-4);
    let t_in = SaturatedProperties::at(7.02e6).expect("IF97").t_sat;
    s.set_left_boundary(TwoFluidBoundary::PressureInlet {
        pressure: 7.02e6,
        inflow: TwoFluidInflow {
            thermo: TwoFluidInflowThermo::LiquidTemperature(t_in),
            void_fraction: 0.1,
        },
    });
    s.set_right_boundary(TwoFluidBoundary::PressureOutlet { pressure: 7.0e6 });
    let q_w = 2.0e5;
    s.set_wall_heat(PhasicWallHeat::Hem(WallHeat::HeatFlux(
        HeatFluxDensity::new::<watt_per_square_meter>(q_w),
    )))
    .expect("flux");
    let q = q_w * std::f64::consts::PI * D * 0.5;
    let v = s.pipe().cell_volume();
    let dt = 1.0e-4;
    let mut last = None;
    let mut min_residual = f64::INFINITY;
    let mut d_end = 0.0;
    for k in 0..15_000 {
        let e0 = s.total_enthalpy();
        let p_old = s.pressure().to_vec();
        let a_sum: Vec<f64> =
            (0..4).map(|i| s.void_fraction()[i] + s.liquid_fraction()[i]).collect();
        let r = s.step().unwrap_or_else(|e| panic!("step {k} refused: {e}"));
        let work: f64 = (0..4).map(|i| v * a_sum[i] * (s.pressure()[i] - p_old[i])).sum();
        let storage = (s.total_enthalpy() - e0 - work) / dt;
        let residual = storage - (r.inlet_energy_flow - r.outlet_energy_flow + q);
        min_residual = min_residual.min(residual);
        d_end = residual;
        last = Some(r);
    }
    let r = last.expect("ran");
    let m_spread = (r.inlet_mass_flow - r.outlet_mass_flow).abs() / r.outlet_mass_flow;
    let excess = (r.outlet_energy_flow - r.inlet_energy_flow - q) / q;
    println!(
        "heated two-fluid channel: m_in = {:.6e}, m_out = {:.6e} kg/s (spread {m_spread:.3e}); \
         E_out - E_in = {:.6e} W vs Q = {q:.6e} W (excess {excess:+.4e}); D_end/Q = {:.4e}, \
         min per-step residual {min_residual:.3e} W; max void {:.4}, slip {:.4e} m/s; limited/floored {}/{}",
        r.inlet_mass_flow,
        r.outlet_mass_flow,
        r.outlet_energy_flow - r.inlet_energy_flow,
        d_end / q,
        r.max_void_fraction,
        r.max_slip,
        r.mass_transfer_limited_cells,
        r.residual_alpha_floor_events
    );
    assert!(r.outlet_mass_flow > 0.0 && m_spread < 1.0e-3, "not steady: {m_spread:e}");
    assert!(min_residual >= -1.0e-6 * q, "budget residual went negative: {min_residual:e} W");
    assert!(excess.abs() < 1.0e-2, "steady excess {excess:e} above the predicted 1 %");
    assert!((excess - d_end / q).abs() < 1.0e-2, "steady balance miss: {excess:e} vs D/Q {:e}", d_end / q);
}
