// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! V&V of the variable flow area `A(x)` (gh:#1058) in
//! [`super::drift_flux::DriftFlux1d`] and [`super::two_fluid::TwoFluid1d`].
//! Record: `crates/tampines/verification_and_validation/multiphase_1d_issues_1058_1060_1062_1076.md`.
//! Verification only.

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_multiphase::drift_flux::SlipModel;
use tampines_steam_tables::hem::AreaProfile;
use uom::si::angle::radian;
use uom::si::f64::{Angle, Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::drift_flux::{AxialBoundary, DriftFlux1d};
use super::geometry::Pipe1d;
use super::properties::SaturatedProperties;
use super::two_fluid::{TwoFluid1d, TwoFluidBoundary};

fn pipe(l: f64, d: f64, incline: f64, n: usize) -> Pipe1d {
    Pipe1d::circular(
        Length::new::<meter>(l),
        Length::new::<meter>(d),
        Angle::new::<radian>(incline),
        n,
    )
    .expect("pipe")
}

fn no_slip() -> SlipModel {
    SlipModel::ZuberFindlay { c0: 1.0, vgj: Vector3::new(0.0, 0.0, 0.0) }
}

/// **Methodology.** An explicit `AreaProfile::Uniform(A)` must reproduce the
/// constant-section pipe bit for bit in both solvers: the two-fluid
/// demonstrator geometry (8 cells, 7 MPa saturated, `α_g = 0.1`, a choked
/// outlet ramp) and a drift-flux blowdown of 500 K water at 7 MPa through a
/// prescribed `5 m/s` outlet, 100 steps each. **Prediction:** every pressure,
/// void and velocity bit-identical (the area paths reduce to the old
/// arithmetic when the face areas are equal: `A/A = 1`, `√1 = 1`).
///
/// **Result (2026-11-01, release):** bit-identical in both solvers. The
/// constant-section pins elsewhere (two-fluid bit pin against 43cb3eb5d,
/// Edwards GS-1 RMSE 28.2 psia, Marviken 23) are unchanged.
#[test]
fn a_uniform_area_profile_is_bit_identical_to_the_constant_section() {
    let base = pipe(4.096, 0.073, 0.0, 8);
    let a = base.area_si();
    let profiled = base.clone().with_area_profile(AreaProfile::Uniform(a)).expect("profile");
    assert_eq!(profiled.area_profile(), Some(AreaProfile::Uniform(a)));
    let sat = SaturatedProperties::at(7.0e6).expect("IF97");
    let run_tf = |p: Pipe1d| {
        let mut s = TwoFluid1d::bubbly(
            p,
            Pressure::new::<pascal>(7.0e6),
            ThermodynamicTemperature::new::<kelvin>(sat.t_sat),
            Time::new::<second>(3.0e-5),
        )
        .expect("solver");
        s.set_initial_void_fraction(0.1).expect("void");
        s.set_temperature_profile(&vec![ThermodynamicTemperature::new::<kelvin>(sat.t_sat); 8])
            .expect("profile");
        s.set_right_boundary(TwoFluidBoundary::ChokedOutlet {
            area_fraction: 0.5,
            ambient_pressure: 1.0e5,
        });
        for _ in 0..100 {
            s.step().expect("step");
        }
        (s.pressure().to_vec(), s.void_fraction().to_vec(), s.vapour_face_velocity().to_vec())
    };
    assert_eq!(run_tf(base.clone()), run_tf(profiled.clone()));
    let run_df = |p: Pipe1d| {
        let mut s = DriftFlux1d::new(
            p,
            no_slip(),
            Pressure::new::<pascal>(7.0e6),
            ThermodynamicTemperature::new::<kelvin>(500.0),
            Time::new::<second>(1.0e-4),
        )
        .expect("solver");
        s.set_right_boundary(AxialBoundary::PrescribedVelocity(5.0));
        for _ in 0..100 {
            s.step().expect("step");
        }
        (s.pressure().to_vec(), s.void_fraction().to_vec(), s.face_velocity().to_vec())
    };
    assert_eq!(run_df(base), run_df(profiled));
}

/// **Methodology.** Mass conservation with a varying area: a closed,
/// **vertical** converging–diverging pipe (1 m, inlet and outlet 50 mm
/// diameter, throat area half the inlet at mid-length, 16 cells) relaxes
/// toward hydrostatics under gravity for 200 steps, so there is flow through
/// every face. A closed pipe can neither gain nor lose mass, and the
/// donor-cell fluxes telescope with the face areas on both sides.
/// **Prediction:** both solvers keep their inventory to `1e-12` relative.
///
/// **Result (2026-11-01, release):** two-fluid inventory drift `-1.3e-16`
/// with `max |u_l| = 1.7e-2 m/s`. The drift-flux part was weakened while
/// writing, before any run: `DriftFlux1d::inventory` reports the EOS density,
/// not the transported continuity density, so it is not a conserved quantity.
/// What is checked instead is that both end-face velocities stay exactly
/// zero. The interior telescoping is the same face-area expression the
/// two-fluid check covers. **A drift-flux mass-conservation check with a
/// varying area is therefore weaker than the brief asked for.**
#[test]
fn a_closed_varying_area_pipe_conserves_mass_in_both_solvers() {
    let base = pipe(1.0, 0.05, std::f64::consts::FRAC_PI_2, 16);
    let a = base.area_si();
    let nozzle = AreaProfile::ConvergingDiverging {
        inlet_m2: a,
        throat_m2: 0.5 * a,
        outlet_m2: a,
        throat_fraction: 0.5,
    };
    let p = base.with_area_profile(nozzle).expect("profile");
    assert!((p.face_area_si(8) - 0.5 * a).abs() < 1e-15 && p.face_area_si(0) == a);
    let sat = SaturatedProperties::at(7.0e6).expect("IF97");
    let mut tf = TwoFluid1d::bubbly(
        p.clone(),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(sat.t_sat),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    tf.set_initial_void_fraction(0.1).expect("void");
    tf.set_temperature_profile(&vec![ThermodynamicTemperature::new::<kelvin>(sat.t_sat); 16])
        .expect("profile");
    let m0 = tf.inventory().value;
    for _ in 0..200 {
        tf.step().expect("two-fluid step");
    }
    let rel_tf = (tf.inventory().value - m0) / m0;
    let mut df = DriftFlux1d::new(
        p,
        no_slip(),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    // DriftFlux1d's reported inventory is the EOS density; its conserved
    // quantity is the continuity density, so mass is checked through the
    // face fluxes instead: with both ends closed they must vanish.
    for _ in 0..200 {
        df.step().expect("drift-flux step");
    }
    let u_max = tf.liquid_face_velocity().iter().fold(0.0_f64, |m, u| m.max(u.abs()));
    println!(
        "closed nozzle: two-fluid inventory drift {rel_tf:.3e} (max |u_l| {u_max:.3e} m/s); \
         drift-flux end-face velocities {:e}, {:e}",
        df.face_velocity()[0],
        df.face_velocity()[16]
    );
    assert!(u_max > 0.0, "nothing moved; the test would be vacuous");
    assert!(rel_tf.abs() < 1.0e-12, "two-fluid inventory drift {rel_tf:e}");
    assert_eq!((df.face_velocity()[0], df.face_velocity()[16]), (0.0, 0.0));
}

/// **Methodology — smooth nozzle against the quasi-1-D steady analytic.**
/// A horizontal 0.2 m drift-flux nozzle, 100 mm inlet diameter, throat area
/// half the inlet at mid-length, outlet back to the inlet area, 40 cells, fed
/// by a reservoir at 7.02 MPa stagnation and 500 K and discharging to
/// 7.0 MPa; 0.5 s at 0.1 ms. Subcooled water is nearly incompressible, so the
/// steady quasi-1-D solution is `ṁ = ρ u A` constant and Bernoulli between
/// two sections, `p_1 − p_t = ½ ρ (u_t² − u_1²)`.
///
/// **Predictions (before the run):** (1) the face mass flows `ρ_donor u A`
/// agree with the outlet's to `1e-6` relative (steady, conservative);
/// (2) the drop from the first cell to the throat cell agrees with Bernoulli
/// at those cells' areas to within **5 %**, with the measured drop the larger
/// (wall friction over 0.1 m adds `f (L/D) ½ ρ u² ≈ 1–2 %` of it, and the
/// first-order donor convection on 40 cells another few per cent).
///
/// **Result (2026-11-01, release):** `ṁ = 41.11 kg/s`, face mass flows agree
/// to `8.3e-10`; `p_1 − p_t = 50.24 kPa` against Bernoulli `48.21 kPa`
/// (`u_1 = 6.42`, `u_t = 12.52 m/s`): **+4.19 %**, measured larger as
/// predicted, inside the 5 % bound. The split between friction and
/// discretisation was not measured.
#[test]
fn a_smooth_nozzle_matches_the_quasi_one_d_steady_analytic() {
    let base = pipe(0.2, 0.1, 0.0, 40);
    let a = base.area_si();
    let p = base
        .with_area_profile(AreaProfile::ConvergingDiverging {
            inlet_m2: a,
            throat_m2: 0.5 * a,
            outlet_m2: a,
            throat_fraction: 0.5,
        })
        .expect("profile");
    let h0 = {
        use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
        use uom::si::available_energy::joule_per_kilogram;
        h_tp_1(ThermodynamicTemperature::new::<kelvin>(500.0), Pressure::new::<pascal>(7.02e6))
            .get::<joule_per_kilogram>()
    };
    let mut s = DriftFlux1d::new(
        p.clone(),
        no_slip(),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        Time::new::<second>(1.0e-4),
    )
    .expect("solver");
    s.set_left_boundary(AxialBoundary::ReservoirInlet {
        stagnation_pressure: 7.02e6,
        stagnation_enthalpy: h0,
    });
    s.set_right_boundary(AxialBoundary::PressureOutlet { pressure: 7.0e6 });
    for _ in 0..5000 {
        s.step().expect("step");
    }
    let n = 40;
    let rho = s.density();
    let u = s.face_velocity();
    let m_out = rho[n - 1] * u[n] * p.face_area_si(n);
    let worst = (1..n)
        .map(|j| ((rho[j - 1] * u[j] * p.face_area_si(j) - m_out) / m_out).abs())
        .fold(0.0_f64, f64::max);
    let (i1, it) = (0, 19);
    let u1 = m_out / (rho[i1] * p.cell_area_si(i1));
    let ut = m_out / (rho[it] * p.cell_area_si(it));
    let rho_m = 0.5 * (rho[i1] + rho[it]);
    let bernoulli = 0.5 * rho_m * (ut * ut - u1 * u1);
    let measured = s.pressure()[i1] - s.pressure()[it];
    let rel = (measured - bernoulli) / bernoulli;
    println!(
        "nozzle: m_dot = {m_out:.6e} kg/s, worst face-flow spread {worst:.3e}; \
         p1 - pt = {measured:.6e} Pa vs Bernoulli {bernoulli:.6e} Pa (rel {rel:+.4e}); \
         u1 = {u1:.4} m/s, ut = {ut:.4} m/s"
    );
    assert!(worst < 1.0e-6, "face mass flows disagree by {worst:e}");
    assert!(rel.abs() < 0.05, "Bernoulli miss {rel:e}");
}
