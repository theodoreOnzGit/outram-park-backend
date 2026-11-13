// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! V&V of the passive non-condensable species field of
//! [`super::two_fluid::TwoFluid1d`] and of its entropy diagnostic (gh:#1064,
//! RELAP-7 Eqs 18, 23, 128–132). Record:
//! `crates/tampines/verification_and_validation/species_entropy.md`.
//! Verification only: conservation, advection and a second-law monotonicity
//! check, compared against no experiment.

use std::f64::consts::FRAC_PI_2;

use uom::si::angle::radian;
use uom::si::f64::{Angle, Length, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::geometry::Pipe1d;
use super::properties::SaturatedProperties;
use super::two_fluid::{TwoFluid1d, TwoFluidBoundary};

/// A 7 MPa two-phase pipe, both phases at `T_sat − subcooling`.
fn pipe_solver(
    length: f64,
    n: usize,
    incline: f64,
    void: f64,
    subcooling: f64,
    dt: f64,
) -> TwoFluid1d {
    let pipe = Pipe1d::circular(
        Length::new::<meter>(length),
        Length::new::<meter>(0.05),
        Angle::new::<radian>(incline),
        n,
    )
    .expect("pipe");
    let sat = SaturatedProperties::at(7.0e6).expect("IF97");
    let t = sat.t_sat - subcooling;
    let mut s = TwoFluid1d::bubbly(
        pipe,
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(t),
        Time::new::<second>(dt),
    )
    .expect("solver");
    s.set_initial_void_fraction(void).expect("void");
    s.set_temperature_profile(&vec![ThermodynamicTemperature::new::<kelvin>(t); n])
        .expect("profile");
    s
}

/// **Methodology.** RELAP-7 Eqs 128–132 transport the non-condensable mass
/// `c = α_g ρ_g Y` conservatively with the gas, so in a closed pipe its
/// inventory `Σ c V` may change only by round-off. A vertical 1 m, 10-cell,
/// 50 mm pipe, closed at both ends, starts at 7 MPa with `α_g = 0.3` and the
/// liquid 3 K subcooled, so buoyancy drives slip and the vapour condenses
/// (the species must stay behind in the gas, not condense with it). `Y` is a
/// step: `0.05` in the lower half, `0` above. 300 steps of 1 ms.
///
/// **Prediction (before the run):** relative inventory drift below `1e-13`
/// at every step; `c ≥ 0` everywhere; the species moves (the profile
/// changes), and the gas-phase `Y` rises where vapour condenses.
///
/// **Result (2026-11-01, release):** worst relative inventory drift
/// `6.0e-16` over 300 steps; minimum `c = 0` (the initially clean upper
/// half; never negative); the profile moved (largest `|c − c0|` 0.159
/// kg/m³); the summed gas mass fell from 109.57 to 104.01 kg/m³ by
/// condensation, and `Y` in the bottom cell rose from 0.0500 to 0.0528 — the
/// species stays in the gas as the steam condenses. Passes.
#[test]
fn noncondensable_mass_is_conserved_to_round_off_in_a_closed_pipe() {
    let n = 10;
    let mut s = pipe_solver(1.0, n, FRAC_PI_2, 0.3, 3.0, 1.0e-3);
    let y0: Vec<f64> = (0..n).map(|i| if i < n / 2 { 0.05 } else { 0.0 }).collect();
    s.set_noncondensable_mass_fraction(&y0).expect("profile");
    let c0 = s.noncondensable_concentration().to_vec();
    let m0 = s.noncondensable_inventory().value;
    let gas0: f64 = (0..n)
        .map(|i| s.void_fraction()[i] * s.vapour_density()[i])
        .sum();
    let mut worst = 0.0_f64;
    let mut min_c = f64::INFINITY;
    for k in 0..300 {
        s.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        let m = s.noncondensable_inventory().value;
        worst = worst.max(((m - m0) / m0).abs());
        min_c = min_c.min(s.noncondensable_concentration().iter().copied().fold(f64::INFINITY, f64::min));
    }
    let moved: f64 = s
        .noncondensable_concentration()
        .iter()
        .zip(&c0)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let gas1: f64 = (0..n)
        .map(|i| s.void_fraction()[i] * s.vapour_density()[i])
        .sum();
    let y = s.noncondensable_mass_fraction();
    println!(
        "closed pipe: worst |dM_nc|/M_nc = {worst:.3e}; min c = {min_c:.3e} kg/m3; \
         max |c - c0| = {moved:.3e} kg/m3; gas mass sum {gas0:.4} -> {gas1:.4} kg/m3; \
         Y bottom {:.5} top {:.5} max {:.5}",
        y[0],
        y[n - 1],
        y.iter().copied().fold(0.0, f64::max)
    );
    assert!(worst < 1.0e-13, "non-condensable inventory drifted by {worst:e}");
    assert!(min_c >= 0.0, "negative concentration {min_c}");
    assert!(moved > 1.0e-6, "the species did not move");
}

/// **Methodology.** The species rides on the gas velocity. A horizontal
/// 4 m, 80-cell pipe of saturated steam/water at 7 MPa (`α_g = 0.5`, no
/// subcooling, so `Γ ≈ 0`) is driven by a 1 m/s prescribed-velocity inlet
/// into a 7 MPa pressure outlet. A Gaussian pulse of `Y` (peak `0.01`,
/// σ = 0.1 m) starts at `x = 0.7 m`; after 0.6 s (240 steps of 2.5 ms) the
/// displacement of its centroid `x̄ = Σ c x V / Σ c V` is compared with the
/// time integral of the mean gas face velocity over the pulse region
/// (cells 5–35), sampled every step.
///
/// **Prediction (before the run):** for donor-cell transport at uniform
/// velocity the centroid moves exactly with the carrier, so the ratio
/// displacement / `∫ ū_g dt` is `1` within `1 %` (the residual from the
/// velocity's small non-uniformity). The pulse spreads (first-order
/// numerical diffusion) and its mass is conserved while it is inside.
///
/// **Result (2026-11-01, release):** centroid 0.700 → 1.307 m, displacement
/// 0.6071 m against `∫ ū_g dt` = 0.6048 m, ratio **1.0039**; pulse mass
/// change `2.8e-10` relative. Passes. **Two earlier runs missed the mass
/// criterion for protocol reasons, recorded here:** with the pulse at 0.5 m
/// in a 2 m pipe the mass rose `8.1e-6` (the prescribed-velocity inlet
/// re-injects the adjacent cell's `Y`, so the Gaussian tail at `x = 0`
/// re-entered); moved to 0.7 m in the same pipe it fell `5.5e-4` (the
/// diffused tail left through the outlet). Lengthening the pipe to 4 m,
/// so the pulse is genuinely inside, fixed the protocol; the `1e-6`
/// criterion was not changed.
#[test]
fn a_noncondensable_pulse_advects_with_the_gas_velocity() {
    let n = 80;
    let mut s = pipe_solver(4.0, n, 0.0, 0.5, 0.0, 2.5e-3);
    s.set_left_boundary(TwoFluidBoundary::PrescribedVelocity(1.0));
    s.set_right_boundary(TwoFluidBoundary::PressureOutlet { pressure: 7.0e6 });
    let x: Vec<f64> = (0..n).map(|i| s.pipe().cell_centre(i)).collect();
    let y0: Vec<f64> = x
        .iter()
        .map(|xi| 0.01 * petir::mathf::RealMath::r_exp(-0.5 * petir::mathf::RealMath::r_powi((xi - 0.7) / 0.1, 2)))
        .collect();
    s.set_noncondensable_mass_fraction(&y0).expect("profile");
    s.set_inflow_noncondensable_mass_fraction(Ratio::new::<ratio>(0.0))
        .expect("inflow");
    let centroid = |s: &TwoFluid1d| -> (f64, f64) {
        let c = s.noncondensable_concentration();
        let m: f64 = c.iter().sum();
        (c.iter().zip(&x).map(|(ci, xi)| ci * xi).sum::<f64>() / m, m)
    };
    let (x0, m0) = centroid(&s);
    let mut travel = 0.0;
    let dt = 2.5e-3;
    for k in 0..240 {
        let u_mean: f64 = s.vapour_face_velocity()[5..=35].iter().sum::<f64>() / 31.0;
        s.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        let u_new: f64 = s.vapour_face_velocity()[5..=35].iter().sum::<f64>() / 31.0;
        travel += 0.5 * (u_mean + u_new) * dt;
    }
    let (x1, m1) = centroid(&s);
    let speed_ratio = (x1 - x0) / travel;
    let ul_mean: f64 = s.liquid_face_velocity()[5..=35].iter().sum::<f64>() / 31.0;
    println!(
        "pulse: centroid {x0:.5} -> {x1:.5} m, displacement {:.5} m, int u_g dt {travel:.5} m, \
         ratio {speed_ratio:.5}; mass {m0:.6e} -> {m1:.6e} (rel {:.2e}); final mean u_l {ul_mean:.4} m/s",
        x1 - x0,
        (m1 - m0) / m0
    );
    assert!((speed_ratio - 1.0).abs() < 0.01, "centroid speed / gas speed = {speed_ratio}");
    assert!(((m1 - m0) / m0).abs() < 1.0e-6, "pulse mass changed while inside");
}

/// **Methodology.** The area-averaged entropy diagnostic (RELAP-7 Eqs 18,
/// 23), `S = Σ V (α_g ρ_g s_g + α_l ρ_l s_l)`, with IF97 phasic entropies.
/// A closed, horizontal, adiabatic 1 m, 4-cell pipe at 7 MPa, `α_g = 0.1`,
/// liquid 5 K subcooled against saturated vapour (0.2 mm bubbles of the
/// default set), relaxes by condensation for 200 steps of 0.1 ms. Nothing
/// crosses the boundary and no heat enters, so the second law says `S` may
/// not fall.
///
/// **Prediction (before the run):** `S` is non-decreasing at every step, to
/// within a round-off allowance of `1e-12 S` per step, and rises in total.
///
/// **Result (2026-11-01, release):** `S` rose by `1.89e-3` J/K (of
/// 4113.9 J/K) and fell at no step (worst single-step drop exactly 0).
/// Passes.
#[test]
fn the_entropy_inventory_does_not_fall_in_a_closed_adiabatic_pipe() {
    let mut s = pipe_solver(1.0, 4, 0.0, 0.1, 5.0, 1.0e-4);
    let s0 = s.entropy_inventory().expect("S");
    let mut prev = s0;
    let mut worst_drop = 0.0_f64;
    for k in 0..200 {
        s.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        let now = s.entropy_inventory().expect("S");
        worst_drop = worst_drop.max(prev - now);
        prev = now;
    }
    println!(
        "entropy: S0 = {s0:.9e} J/K, S_end = {prev:.9e} J/K, rise {:.4e} J/K; worst \
         single-step drop {worst_drop:.3e} J/K ({:.2e} of S)",
        prev - s0,
        worst_drop / s0
    );
    assert!(prev > s0, "S did not rise");
    assert!(worst_drop <= 1.0e-12 * s0, "S fell by {worst_drop:e} in one step");
}

/// The species API refuses malformed input.
#[test]
fn the_species_setters_refuse_bad_input() {
    let mut s = pipe_solver(1.0, 4, 0.0, 0.1, 0.0, 1.0e-4);
    assert!(s.set_noncondensable_mass_fraction(&[0.0; 3]).is_err());
    assert!(s.set_noncondensable_mass_fraction(&[0.0, 0.0, 1.5, 0.0]).is_err());
    assert!(s
        .set_inflow_noncondensable_mass_fraction(Ratio::new::<ratio>(-0.1))
        .is_err());
    assert_eq!(s.noncondensable_inventory().value, 0.0);
}
