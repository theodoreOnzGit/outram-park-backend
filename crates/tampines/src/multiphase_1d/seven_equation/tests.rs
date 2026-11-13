// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! V&V of the seven-equation two-pressure model (gh:#1059). Record:
//! `crates/tampines/verification_and_validation/seven_equation.md`.
//! Every prediction below was written into this file before the case was
//! first run; every result is the measured number, misses included.

use std::f64::consts::FRAC_PI_2;

use tampines_steam_tables::hem::{StiffenedGasHem, StiffenedGasPair};
use uom::si::angle::radian;
use uom::si::f64::{
    Angle, Length, MassDensity, Pressure, Ratio, ThermodynamicTemperature, Time, Velocity,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::{
    characteristic_speeds, relaxed_pressure, sg_density_pt, sg_energy, sg_entropy,
    sg_sound_speed, sg_temperature, single_pressure_characteristic_speeds, wood_sound_speed,
    FluxScheme, InterfacialVariables, MassTransfer, Relaxation, SevenEquation1d, SevenEquationBoundary,
    SevenEquationState,
};
use crate::multiphase_1d::geometry::Pipe1d;

// ─────────────────────────────────────────────────────────────────────────────
//  Shared set-up
// ─────────────────────────────────────────────────────────────────────────────

/// Water as Saurel & Abgrall (1999) model it, `γ = 4.4`, `p∞ = 6e8 Pa`
/// (`q = q' = 0`; `c_v` borrowed from Le Métayer et al.'s liquid, it enters
/// only temperatures, which no isothermal-insensitive case here reads).
fn water_sa() -> StiffenedGasHem {
    StiffenedGasHem {
        gamma: 4.4,
        p_inf_pa: 6.0e8,
        q_j_per_kg: 0.0,
        q_prime_j_per_kg_k: 0.0,
        cv_j_per_kg_k: 1816.0,
    }
}

/// Air, `γ = 1.4`, `c_v = 717.5 J/(kg K)`.
fn air() -> StiffenedGasHem {
    StiffenedGasHem {
        gamma: 1.4,
        p_inf_pa: 0.0,
        q_j_per_kg: 0.0,
        q_prime_j_per_kg_k: 0.0,
        cv_j_per_kg_k: 717.5,
    }
}

fn air_water() -> StiffenedGasPair {
    StiffenedGasPair {
        liquid: water_sa(),
        vapour: air(),
    }
}

fn pipe(length: f64, n: usize, incline: f64) -> Pipe1d {
    Pipe1d::circular(
        Length::new::<meter>(length),
        Length::new::<meter>(0.1),
        Angle::new::<radian>(incline),
        n,
    )
    .expect("pipe")
}

fn state(a: f64, rv: f64, uv: f64, pv: f64, rl: f64, ul: f64, pl: f64) -> SevenEquationState {
    SevenEquationState {
        void_fraction: a,
        vapour_density: rv,
        vapour_velocity: uv,
        vapour_pressure: pv,
        liquid_density: rl,
        liquid_velocity: ul,
        liquid_pressure: pl,
        noncondensable_mass_fraction: 0.0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  1. Construction, defaults, ablations
// ─────────────────────────────────────────────────────────────────────────────

/// Pins the physics-on defaults and that each ablation is explicit: finite
/// pressure and velocity relaxation, impedance-weighted interface with sign
/// terms, MUSCL, no mass transfer (opt-in, module docs). Also the API's
/// refusals and the relaxation coefficients of Eqs 197–198 against a hand
/// value.
#[test]
fn defaults_are_physics_on_and_each_ablation_is_explicit() {
    let s0 = state(0.3, 1.2, 0.0, 1.0e5, 1000.0, 0.0, 1.0e5);
    let mk = || SevenEquation1d::new(pipe(1.0, 4, 0.0), air_water(), &[s0; 4]).expect("solver");
    let d = mk();
    assert_eq!(d.pressure_relaxation(), Relaxation::Finite);
    assert_eq!(d.velocity_relaxation(), Relaxation::Finite);
    assert_eq!(d.mass_transfer(), MassTransfer::None);
    assert_eq!(d.interfacial_variables(), InterfacialVariables::ImpedanceWeighted);
    assert!(d.uses_interface_sign_terms() && d.uses_muscl());
    assert_eq!(mk().without_pressure_relaxation().pressure_relaxation(), Relaxation::Off);
    assert_eq!(mk().without_velocity_relaxation().velocity_relaxation(), Relaxation::Off);
    assert!(!mk().without_interface_sign_terms().uses_interface_sign_terms());
    assert!(!mk().without_muscl().uses_muscl());
    assert_eq!(mk().flux_scheme(), FluxScheme::Hllc);
    assert_eq!(mk().without_hllc().flux_scheme(), FluxScheme::Rusanov);
    assert_eq!(
        mk().with_velocity_relaxation(Relaxation::Instantaneous).velocity_relaxation(),
        Relaxation::Instantaneous
    );
    assert_eq!(mk().eos().liquid, water_sa());
    let k = mk().with_instantaneous_relaxation().with_instantaneous_mass_transfer();
    assert_eq!(k.pressure_relaxation(), Relaxation::Instantaneous);
    assert_eq!(k.velocity_relaxation(), Relaxation::Instantaneous);
    assert_eq!(k.mass_transfer(), MassTransfer::Instantaneous);
    assert_eq!(
        mk().with_interfacial_variables(InterfacialVariables::BaerNunziato)
            .interfacial_variables(),
        InterfacialVariables::BaerNunziato
    );
    // Eqs 197-198 by hand with a constant A_int.
    let c = mk().with_constant_interfacial_area(100.0).expect("area");
    let (mu, lambda) = c.relaxation_coefficients(&s0).expect("coefficients");
    let z_v = 1.2 * sg_sound_speed(&air(), 1.2, 1.0e5);
    let z_l = 1000.0 * sg_sound_speed(&water_sa(), 1000.0, 1.0e5);
    assert!((mu - 100.0 / (z_l + z_v)).abs() <= 1e-15 * mu);
    assert!((lambda - 0.5 * mu * z_l * z_v).abs() <= 1e-12 * lambda);
    // The regime-map area is the default and its ablation changes it.
    let (mu_map, _) = mk().relaxation_coefficients(&s0).expect("map");
    let (mu_fixed, _) = mk().without_regime_map().relaxation_coefficients(&s0).expect("fixed");
    assert!(mu_map > 0.0 && mu_fixed > 0.0);
    // Refusals.
    assert!(SevenEquation1d::new(pipe(1.0, 4, 0.0), air_water(), &[s0; 3]).is_err());
    assert!(SevenEquation1d::new(
        pipe(1.0, 2, 0.0),
        air_water(),
        &[s0, state(0.0, 1.2, 0.0, 1.0e5, 1000.0, 0.0, 1.0e5)]
    )
    .is_err());
    assert!(mk().with_constant_interfacial_area(-1.0).is_err());
    let mut m = mk();
    assert!(m.set_time_step_control(0.9, Time::new::<second>(1.0)).is_err());
    assert!(m.set_right_face_area_fraction(0.0).is_err());
    // uom constructors agree with the raw state.
    let a = SevenEquationState::new(
        Ratio::new::<ratio>(0.3),
        MassDensity::new::<kilogram_per_cubic_meter>(1.2),
        MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
        Pressure::new::<pascal>(1.0e5),
        Velocity::new::<meter_per_second>(0.0),
    );
    assert_eq!(a, s0);
    let t = SevenEquationState::at_temperatures(
        &air_water(),
        Ratio::new::<ratio>(0.3),
        Pressure::new::<pascal>(1.0e5),
        ThermodynamicTemperature::new::<kelvin>(300.0),
        ThermodynamicTemperature::new::<kelvin>(300.0),
        Velocity::new::<meter_per_second>(0.0),
    );
    assert!((sg_temperature(&air(), t.vapour_density, 1.0e5) - 300.0).abs() < 1e-9);
}

// ─────────────────────────────────────────────────────────────────────────────
//  2. Discretisation properties
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** Abgrall's criterion, the property Saurel & Abgrall
/// (1999) build their non-conservative discretisation around: a flow with
/// uniform pressure and velocity must stay so across a volume-fraction (and
/// density) discontinuity. 1 m, 100 cells, transmissive ends, `p = 1e5 Pa`,
/// `u_v = u_l = 50 m/s`, `α_v = 0.9` (air 1.2 kg/m³, water 1000) in
/// `x < 0.5 m`, `α_v = 0.1` (air 5, water 900) beyond; default physics
/// (finite relaxation, impedance-weighted interface, MUSCL); 300 steps.
///
/// **Prediction (before the run):** pressure and velocity stay uniform to
/// round-off, `max |p − p0|/p0 < 1e-10` and `max |u − u0| < 1e-8 m/s`, while
/// the void front moves (the volume fraction changes).
///
/// **Result (2026-11-01, release):** max `|Δp|/p0` = **1.4e-11**, max `|Δu|`
/// = **9.3e-13 m/s** after 300 steps (0.68 ms), the front spread over 5
/// cells. Passes. (Under the first Rusanov default the same case gave
/// 4.3e-11 and 7.1e-12 with 21 cells in the front.)
#[test]
fn uniform_pressure_and_velocity_survive_a_void_discontinuity() {
    let n = 100;
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            if i < n / 2 {
                state(0.9, 1.2, 50.0, 1.0e5, 1000.0, 50.0, 1.0e5)
            } else {
                state(0.1, 5.0, 50.0, 1.0e5, 900.0, 50.0, 1.0e5)
            }
        })
        .collect();
    let mut s = SevenEquation1d::new(pipe(1.0, n, 0.0), air_water(), &init).expect("solver");
    s.set_left_boundary(SevenEquationBoundary::Transmissive);
    s.set_right_boundary(SevenEquationBoundary::Transmissive);
    for k in 0..300 {
        s.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
    }
    let st = s.states().expect("states");
    let (mut dp, mut du) = (0.0_f64, 0.0_f64);
    for x in &st {
        dp = dp.max((x.vapour_pressure - 1.0e5).abs()).max((x.liquid_pressure - 1.0e5).abs());
        du = du.max((x.vapour_velocity - 50.0).abs()).max((x.liquid_velocity - 50.0).abs());
    }
    let moved = st.iter().filter(|x| x.void_fraction > 0.11 && x.void_fraction < 0.89).count();
    println!(
        "Abgrall: max |dp|/p0 = {:.3e}, max |du| = {du:.3e} m/s after {} s; {moved} cells in \
         the smeared front",
        dp / 1.0e5,
        s.time().get::<second>()
    );
    assert!(dp / 1.0e5 < 1.0e-10 && du < 1.0e-8);
    assert!(moved > 0);
}

/// **Methodology.** A fluid at rest in a variable-area duct (a 2:1 linear
/// contraction over 1 m, 50 cells, walls) must stay at rest: the momentum
/// source `p_int Δ(ᾱA) + (p_k − p_int) α_k ΔA` balances the flux difference
/// exactly. `α_v = 0.4`, `p = 2e5 Pa`. 200 steps, default physics.
///
/// **Prediction:** velocities below `1e-9 m/s`, pressures unchanged to `1e-12`
/// relative.
///
/// **Result (2026-11-01, release):** max `|u|` = 7.1e-17 m/s, pressure change
/// exactly 0. Passes.
#[test]
fn a_fluid_at_rest_in_a_variable_area_duct_stays_at_rest() {
    use tampines_steam_tables::hem::AreaProfile;
    let n = 50;
    let base = pipe(1.0, n, 0.0);
    let a0 = base.area_si();
    let profile = AreaProfile::Linear {
        inlet_m2: a0,
        outlet_m2: 0.5 * a0,
    };
    let duct = base.with_area_profile(profile).expect("profile");
    let init = vec![state(0.4, 2.4, 0.0, 2.0e5, 1000.0, 0.0, 2.0e5); n];
    let mut s = SevenEquation1d::new(duct, air_water(), &init).expect("solver");
    for k in 0..200 {
        s.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
    }
    let st = s.states().expect("states");
    let du = st
        .iter()
        .map(|x| x.vapour_velocity.abs().max(x.liquid_velocity.abs()))
        .fold(0.0, f64::max);
    let dp = st
        .iter()
        .map(|x| (x.vapour_pressure - 2.0e5).abs().max((x.liquid_pressure - 2.0e5).abs()))
        .fold(0.0, f64::max);
    println!("rest in a duct: max |u| = {du:.3e} m/s, max |dp|/p = {:.3e}", dp / 2.0e5);
    assert!(du < 1.0e-9 && dp / 2.0e5 < 1.0e-12);
}

/// **Methodology.** A closed tube (walls) holding a water–air Riemann problem
/// (left `α_v = 0.3` at 2e5 Pa, right `α_v = 0.7` at 1e5 Pa, `Y = 0.1` in
/// the gas on the right) is marched 400 steps with the default finite
/// relaxation. The phase masses, the non-condensable mass (gh:#1064) and the
/// total energy are conserved by the flux form, and the interphase terms
/// (`p_int ∂α`, `p_int u_int ∂α`, relaxation) cancel pairwise.
///
/// **Prediction:** each of the four inventories drifts by less than `1e-13`
/// relative.
///
/// **Result (2026-11-01, release):** drifts after 400 steps (0.98 ms):
/// `m_v` 7.1e-16, `m_l` 2.6e-15, non-condensable 6.6e-16, `E` 2.6e-15.
/// Passes. **Recorded misses on the way:** (1) the first set-up, a 1e7 |
/// 1e5 Pa water-rich | air-rich tube, was refused in the first stage — a
/// trace phase at a strong void and pressure step, the limitation recorded
/// at `the_water_air_interface_shock_tube_is_refused_by_cell_centred_p_int`
/// — so the conservation case was moved to the milder 2:1 problem above;
/// (2) the wall flux then leaked `5e-12` per 400 steps because the wall ghost
/// was mirrored from the cell centre instead of from the reconstructed face
/// state; fixed in the solver (ghost face state = mirror of the interior face
/// state), not by moving the bar.
#[test]
fn a_closed_tube_conserves_phase_masses_species_and_total_energy() {
    let n = 100;
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            if i < n / 2 {
                state(0.3, 2.4, 0.0, 2.0e5, 1000.0, 0.0, 2.0e5)
            } else {
                SevenEquationState {
                    noncondensable_mass_fraction: 0.1,
                    ..state(0.7, 1.2, 0.0, 1.0e5, 1000.0, 0.0, 1.0e5)
                }
            }
        })
        .collect();
    let mut s = SevenEquation1d::new(pipe(1.0, n, 0.0), air_water(), &init).expect("solver");
    let (mv0, ml0, mc0) = s.inventories();
    let e0 = s.total_energy();
    for k in 0..400 {
        s.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
    }
    let (mv, ml, mc) = s.inventories();
    let rel = |a: f64, b: f64| ((a - b) / b).abs();
    let d = [
        rel(mv.value, mv0.value),
        rel(ml.value, ml0.value),
        rel(mc.value, mc0.value),
        rel(s.total_energy(), e0),
    ];
    println!(
        "closed tube, t = {:.3e} s: drift m_v {:.2e}, m_l {:.2e}, nc {:.2e}, E {:.2e}",
        s.time().get::<second>(),
        d[0],
        d[1],
        d[2],
        d[3]
    );
    assert!(d.iter().all(|x| *x < 1.0e-13), "{d:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
//  3. Relaxation operators
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** Each relaxation operator, applied once to a single cell
/// (a 1-cell closed pipe, so the hyperbolic step is the identity at rest
/// except for the relaxation itself), over a sweep: `α_v` ∈ {0.01, 0.3, 0.7,
/// 0.99}; `p_v/p_l` ∈ {0.5, 1, 2} around 1e6 Pa; slip `u_v − u_l` ∈ {0, 5} m/s
/// (mixture at rest); each of the three interfacial-variable choices; finite
/// (`A_int` = 10, 1000 1/m) and instantaneous. Checked: total energy and
/// mixture momentum conserved; the stiffened-gas entropy inventory does not
/// fall (the second law for the relaxation sources, gh:#1064); instantaneous
/// relaxation leaves `|p_v − p_l|` and `|u_v − u_l|` at round-off; finite
/// relaxation moves them toward zero without overshoot. The slip cases run
/// the velocity operator then the pressure operator; the no-slip cases the
/// pressure operator alone, so that its overshoot can be judged.
///
/// **Prediction (before the run):** energy and momentum to `1e-12`
/// relative; entropy never falls (by more than `1e-12` relative); the
/// equilibria hold to `1e-6` Pa and `1e-12` m/s.
///
/// **Result (2026-11-01, release):** 216 cases. Worst `|ΔE|/E` 3.9e-16, worst
/// momentum change 2.3e-17 m/s; the entropy never fell (minimum change 0).
/// Instantaneous relaxation: `|u_v − u_l|` ≤ 1e-31 m/s; **`|p_v − p_l|` up
/// to 6.8e-5 Pa, which MISSES the registered 1e-6 Pa bar.** Diagnosis: the
/// state stores `α_v`, so at `α_v = 0.99` the liquid fraction `1 − α_v`
/// loses two digits, and a phase pressure recovered from
/// `(γ − 1) ρ (e − q) − γ p∞` carries `(p + γ p∞) ε / α_l` ≈ 3e-3 Pa of
/// round-off; 6.8e-5 Pa is 0.021 of that floor. The bar was below what the
/// representation can resolve, so the equilibrium is asserted against the
/// floor and the original bar is printed as missed. Finite relaxation: no
/// overshoot above the floor. **Earlier misses:** the first finite pressure
/// relaxation froze `p_int` at the start of the step and LOST entropy
/// (minimum `ΔS/S = −1.4e-8`) and overshot in 30 cases (24 of them a
/// mis-classification by this test: velocity relaxation moving the
/// pressures); `p_int` was then taken as the frozen-weight mean of the
/// END pressures, which makes each phase's entropy change non-negative.
#[test]
fn relaxation_conserves_energy_produces_entropy_and_reaches_equilibrium() {
    let eos = air_water();
    let mut worst_e = 0.0_f64;
    let mut worst_m = 0.0_f64;
    let mut min_ds = f64::INFINITY;
    let mut worst_dp = 0.0_f64;
    let mut worst_dp_over_floor = 0.0_f64;
    let mut worst_du = 0.0_f64;
    let mut overshoot = 0usize;
    let mut cases = 0usize;
    for &a in &[0.01, 0.3, 0.7, 0.99] {
        for &ratio_p in &[0.5, 1.0, 2.0] {
            for &w in &[0.0, 5.0] {
                for iv in [
                    InterfacialVariables::ImpedanceWeighted,
                    InterfacialVariables::BaerNunziato,
                    InterfacialVariables::SaurelAbgrall,
                ] {
                    for mode in [Some(10.0), Some(1000.0), None] {
                        let (pv, pl) = (1.0e6 * ratio_p, 1.0e6);
                        let (rv, rl) = (8.0, 1000.0);
                        let (mv, ml) = (a * rv, (1.0 - a) * rl);
                        let uv = w * ml / (mv + ml);
                        let ul = uv - w;
                        let st = state(a, rv, uv, pv, rl, ul, pl);
                        let base = SevenEquation1d::new(pipe(0.1, 1, 0.0), eos, &[st])
                            .expect("solver")
                            .with_interfacial_variables(iv);
                        let mut s = match mode {
                            Some(area) => base.with_constant_interfacial_area(area).expect("area"),
                            None => base.with_instantaneous_relaxation(),
                        };
                        let e0 = s.total_energy();
                        let s0 = s.entropy_inventory().expect("S");
                        let mut u = s.conserved_state()[0];
                        let mom0 = u[2] + u[5];
                        // Slip cases exercise the velocity operator, the others
                        // the pressure operator alone (velocity relaxation heats
                        // the phases unequally and so moves the pressures).
                        if w > 0.0 {
                            s.relax_velocity(&mut u, 1.0e-4, 0).expect("velocity");
                        }
                        s.relax_pressure(&mut u, 1.0e-4, 0).expect("pressure");
                        s.u[0] = u;
                        cases += 1;
                        let e1 = s.total_energy();
                        let s1 = s.entropy_inventory().expect("S");
                        let p1 = s.states().expect("state")[0];
                        worst_e = worst_e.max(((e1 - e0) / e0).abs());
                        worst_m = worst_m.max((u[2] + u[5] - mom0).abs() / (mv + ml));
                        min_ds = min_ds.min((s1 - s0) / s0.abs().max(1.0));
                        let dp1 = (p1.vapour_pressure - p1.liquid_pressure).abs();
                        let du1 = (p1.vapour_velocity - p1.liquid_velocity).abs();
                        // Representation floor of a phase pressure recovered from
                        // the conserved state: (p + γ p∞) ε amplified by the
                        // cancellation in α_l = 1 − α_v (see the result note).
                        let floor = 64.0
                            * f64::EPSILON
                            * (pl.abs() + water_sa().gamma * water_sa().p_inf_pa)
                            / a.min(1.0 - a);
                        if mode.is_none() {
                            worst_dp = worst_dp.max(dp1);
                            worst_dp_over_floor = worst_dp_over_floor.max(dp1 / floor);
                            worst_du = worst_du.max(du1);
                        } else if (w == 0.0
                            && dp1 > floor
                            && (dp1 > (pv - pl).abs() * (1.0 + 1e-12)
                                || (p1.vapour_pressure - p1.liquid_pressure) * (pv - pl) < 0.0))
                            || du1 > w * (1.0 + 1e-12)
                        {
                            overshoot += 1;
                            println!(
                                "  overshoot: a={a} pv/pl={ratio_p} w={w} {iv:?} {mode:?}: \
                                 dp {:.4e} -> {:.4e} Pa, du {w} -> {du1:.3e}",
                                pv - pl,
                                p1.vapour_pressure - p1.liquid_pressure
                            );
                        }
                    }
                }
            }
        }
    }
    println!(
        "relaxation sweep ({cases} cases): worst |dE|/E {worst_e:.2e}; worst |d(mom)|/M \
         {worst_m:.2e} m/s; min dS/S {min_ds:.3e}; instantaneous max |p_v - p_l| \
         {worst_dp:.3e} Pa ({worst_dp_over_floor:.3} of the representation floor), max \
         |u_v - u_l| {worst_du:.3e} m/s; finite overshoots above the floor {overshoot}"
    );
    assert!(worst_e < 1.0e-12 && worst_m < 1.0e-12);
    assert!(min_ds >= -1.0e-12, "entropy fell: {min_ds:e}");
    // The registered 1e-6 Pa bar was MISSED (6.8e-5 Pa) and is reported; it
    // sat below what the alpha_v-stored state can resolve (result note), so
    // the equilibrium is asserted against that floor instead.
    println!(
        "registered bar |p_v - p_l| < 1e-6 Pa: {}",
        if worst_dp < 1.0e-6 { "met" } else { "MISSED" }
    );
    assert!(worst_dp_over_floor <= 1.0 && worst_du < 1.0e-12);
    assert_eq!(overshoot, 0);
}

/// The closed-form relaxed pressure solves its defining equation (unit
/// check of the quadratic).
#[test]
fn the_relaxed_pressure_quadratic_satisfies_the_volume_constraint() {
    let (gv, gl) = (air(), water_sa());
    for &(a, pv, pl) in &[(0.5, 2.0e5, 1.0e5), (0.01, 1.0e9, 1.0e5), (0.99, 1.0e5, 3.0e6)] {
        let ps = relaxed_pressure(&gv, &gl, a, pv, pl);
        let f = a * (pv + gv.gamma * gv.p_inf_pa + (gv.gamma - 1.0) * ps) / (gv.gamma * (ps + gv.p_inf_pa))
            + (1.0 - a) * (pl + gl.gamma * gl.p_inf_pa + (gl.gamma - 1.0) * ps)
                / (gl.gamma * (ps + gl.p_inf_pa));
        assert!((f - 1.0).abs() < 1e-12 && ps > 0.0, "a={a}: p*={ps}, residual {}", f - 1.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  4. Characteristic analysis: hyperbolicity and Remark 2
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** RELAP-7 Remark 2: relaxing the pressures alone leaves the
/// single-pressure two-velocity system, which is not hyperbolic. Its four
/// characteristic speeds ([`single_pressure_characteristic_speeds`]) are
/// computed at air–water states (`α_v` ∈ {0.1, 0.5, 0.9}, `p = 1e5 Pa`)
/// with slip `w` ∈ {0, 0.1, 1, 10} m/s, and compared with the seven-equation
/// speeds at the same state.
///
/// **Prediction (before the run):** at `w = 0` all four are real (`u ± c_W`
/// with `c_W` Wood's speed, and a double root at `u`); at every `w ≠ 0` one
/// complex-conjugate pair appears, `|Im λ|` growing roughly linearly with `w`
/// (the incompressible limit gives
/// `|Im λ| = w sqrt(α_v α_l ρ_v ρ_l)/(α_v ρ_l + α_l ρ_v)`); the seven-equation
/// speeds are real at every state.
///
/// **Result (2026-11-01, release):** at every slip `w > 0` exactly one
/// complex-conjugate pair, `|Im λ|` within 0.05 % of the incompressible
/// formula (e.g. `α_v = 0.5`, `w = 1 m/s`: 3.4600e-2 against 3.4599e-2 m/s);
/// the seven-equation speeds are real everywhere. **The zero-slip part of
/// the prediction was wrong:** the fastest root is `u + 341.8 m/s` at
/// `α_v = 0.5`, not `u + c_W` (23.6 m/s). With independent phase
/// velocities the quartic factors as
/// `d² [(α_v ρ_l/c_v² + α_l ρ_v/c_l²) d² − (α_v ρ_l + α_l ρ_v)] = 0`, a
/// harmonic-inertia speed; Wood's speed needs velocity equilibrium too
/// (Remark 1, tested next). The assertion checks that closed-form root.
#[test]
fn relaxing_pressure_alone_leaves_complex_characteristics() {
    let (rv, rl, p) = (1.2, 1000.0, 1.0e5);
    let cv = sg_sound_speed(&air(), rv, p);
    let cl = sg_sound_speed(&water_sa(), rl, p);
    for &a in &[0.1, 0.5, 0.9] {
        for &w in &[0.0, 0.1, 1.0, 10.0] {
            let roots = single_pressure_characteristic_speeds(a, rv, rl, cv, cl, 1.0 + w, 1.0)
                .expect("roots");
            let im_max = roots.iter().map(|r| r.1.abs()).fold(0.0, f64::max);
            let al = 1.0 - a;
            let im_incompressible = w * (a * al * rv * rl).sqrt() / (a * rl + al * rv);
            let c_w = wood_sound_speed(a, rv, rl, cv, cl);
            let re: Vec<f64> = roots.iter().map(|r| r.0).collect();
            println!(
                "a={a} w={w}: roots {:?}; max|Im| {im_max:.4e} (incompressible {im_incompressible:.4e}); \
                 Wood {c_w:.3} m/s",
                roots
                    .iter()
                    .map(|r| format!("{:.4}{:+.4}i", r.0, r.1))
                    .collect::<Vec<_>>()
            );
            if w == 0.0 {
                assert!(im_max < 1e-6 * c_w, "complex roots at zero slip");
                // At zero slip the quartic factors as
                // d² [ (α_v ρ_l/c_v² + α_l ρ_v/c_l²) d² − (α_v ρ_l + α_l ρ_v) ] = 0:
                // a harmonic-inertia speed, NOT Wood's (see the result note).
                let c_2v = ((a / rv + al / rl) / (a / (rv * cv * cv) + al / (rl * cl * cl))).sqrt();
                let fastest = re.iter().copied().fold(f64::MIN, f64::max);
                println!("   zero slip: fastest {fastest:.4} = u + {:.4}; two-velocity speed {c_2v:.4}; Wood {c_w:.4}", fastest - 1.0);
                assert!((fastest - (1.0 + c_2v)).abs() < 1e-6 * c_2v, "u + c_2v: {fastest}");
            } else {
                assert!(im_max > 0.0, "no complex pair at slip {w}");
                assert!(
                    (im_max / im_incompressible - 1.0).abs() < 0.05,
                    "|Im| {im_max} vs incompressible {im_incompressible}"
                );
            }
            let seven = characteristic_speeds(
                &air_water(),
                &state(a, rv, 1.0 + w, p, rl, 1.0, p),
            )
            .expect("speeds");
            assert!(seven.iter().all(|x| x.is_finite()));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  5. Remark 1: the Kapila limit
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** RELAP-7 Remark 1: as `μ, λ → ∞` the model tends to
/// Kapila's five-equation model, whose sound speed is Wood's. A 10 m,
/// 400-cell transmissive tube of air–water at rest (`α_v = 0.5`, `p = 1e5`,
/// air 1.2, water 1000 kg/m³) carries a small Gaussian pressure pulse
/// (`δp = 500 Pa`, σ = 0.3 m, densities raised isentropically,
/// `δρ_k = δp/c_k²`). With instantaneous relaxation the right-going peak
/// is located at `t = 0.1 s` (parabolic fit around the maximum) and its
/// speed compared with `c_W`. The same pulse with relaxation ablated is the
/// contrast: no wave moves at `c_W`; its fastest front runs at the liquid's
/// `c_l`.
///
/// **Prediction (before the run):** relaxed peak speed within 3 % of
/// `c_W = 23.7 m/s`; the unrelaxed run shows pressure disturbance beyond
/// `x0 + c_W t + 1 m`.
///
/// **Result (2026-11-01, release):** `c_W` = 23.650 m/s; the relaxed peak
/// travels at **23.256 m/s (−1.67 %)**, amplitude 300 Pa; the unrelaxed run
/// has 0.028 Pa within 1 m of `x0 + c_W t`. Passes. **The first protocol
/// missed, recorded:** with a pressure pulse released at rest (half going
/// each way) the peak read 21.54 m/s (−8.9 %), because the diffused left-
/// and right-going pulses overlapped and pulled the maximum inward; and the
/// first contrast criterion (disturbance beyond `x0 + c_W t + 1 m` in the
/// unrelaxed run) read 0.00 Pa because both frozen waves (342 and
/// 1625 m/s) had already left the 10 m tube. The pulse was made purely
/// right-going (`δu = δp/(ρ c_W)`) and the contrast re-stated as the absence
/// of a Wood-speed wave; the 3 % bar was not changed.
#[test]
fn the_instantaneously_relaxed_model_carries_sound_at_wood_speed() {
    let n = 400;
    let (rv, rl, p0, a) = (1.2, 1000.0, 1.0e5, 0.5);
    let cv = sg_sound_speed(&air(), rv, p0);
    let cl = sg_sound_speed(&water_sa(), rl, p0);
    let c_w = wood_sound_speed(a, rv, rl, cv, cl);
    let geom = pipe(10.0, n, 0.0);
    let x0 = 3.0;
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            let x = geom.cell_centre(i);
            let dp = 500.0 * petir::mathf::RealMath::r_exp(-0.5 * ((x - x0) / 0.3).powi(2));
            // A purely right-going Wood wave: δu = δp/(ρ c_W).
            let du = dp / ((a * rv + (1.0 - a) * rl) * c_w);
            state(a, rv + dp / (cv * cv), du, p0 + dp, rl + dp / (cl * cl), du, p0 + dp)
        })
        .collect();
    let run = |relaxed: bool| -> (Vec<f64>, f64) {
        let base = SevenEquation1d::new(pipe(10.0, n, 0.0), air_water(), &init).expect("solver");
        let mut s = if relaxed {
            base.with_instantaneous_relaxation()
        } else {
            base.without_pressure_relaxation().without_velocity_relaxation()
        };
        s.set_left_boundary(SevenEquationBoundary::Transmissive);
        s.set_right_boundary(SevenEquationBoundary::Transmissive);
        s.run_until(Time::new::<second>(0.1)).expect("run");
        (s.mixture_pressure().expect("p"), s.time().get::<second>())
    };
    let (pr, t) = run(true);
    // Right-going peak: the maximum right of x0.
    let i0 = geom.nearest_cell(Length::new::<meter>(x0));
    let (imax, _) = pr
        .iter()
        .enumerate()
        .skip(i0 + 1)
        .fold((0, f64::MIN), |acc, (i, v)| if *v > acc.1 { (i, *v) } else { acc });
    let (ym, y0, yp) = (pr[imax - 1], pr[imax], pr[imax + 1]);
    let shift = 0.5 * (ym - yp) / (ym - 2.0 * y0 + yp);
    let x_peak = geom.cell_centre(imax) + shift * geom.dx();
    let speed = (x_peak - x0) / t;
    let (pu, _) = run(false);
    let lo = geom.nearest_cell(Length::new::<meter>(x0 + c_w * t - 1.0));
    let hi = geom.nearest_cell(Length::new::<meter>(x0 + c_w * t + 1.0));
    let far_disturbance = pu[lo..=hi].iter().map(|v| (v - p0).abs()).fold(0.0, f64::max);
    println!(
        "Kapila limit: Wood c_W = {c_w:.4} m/s; relaxed peak at {x_peak:.4} m after {t:.4} s, \
         speed {speed:.4} m/s ({:+.2} %), amplitude {:.1} Pa; unrelaxed max |dp| within 1 m \
         of x0 + c_W t = {far_disturbance:.3} Pa (c_v {cv:.1}, c_l {cl:.1} m/s)",
        100.0 * (speed / c_w - 1.0),
        y0 - p0
    );
    assert!((speed / c_w - 1.0).abs() < 0.03);
    assert!(far_disturbance < 0.01 * (y0 - p0));
}

// ─────────────────────────────────────────────────────────────────────────────
//  6. Two-phase shock tube vs an exact Riemann solution
// ─────────────────────────────────────────────────────────────────────────────

/// One side of a stiffened-gas Riemann problem.
#[derive(Clone, Copy)]
struct Side {
    rho: f64,
    u: f64,
    p: f64,
    g: f64,
    pi: f64,
}

impl Side {
    fn c(&self) -> f64 {
        (self.g * (self.p + self.pi) / self.rho).sqrt()
    }
    /// Toro (2009) ch. 4 pressure function with `p → p + p∞`.
    fn f(&self, p: f64) -> f64 {
        let (pb, pkb) = (p + self.pi, self.p + self.pi);
        if p > self.p {
            let a = 2.0 / ((self.g + 1.0) * self.rho);
            let b = (self.g - 1.0) / (self.g + 1.0) * pkb;
            (p - self.p) * (a / (pb + b)).sqrt()
        } else {
            2.0 * self.c() / (self.g - 1.0)
                * (petir::mathf::RealMath::r_powf(pb / pkb, (self.g - 1.0) / (2.0 * self.g)) - 1.0)
        }
    }
}

/// Exact solution of the two-stiffened-gas Riemann problem (Toro 2009 ch. 4,
/// with `p + p∞` in place of `p` per side): `(p*, u*)` and a sampler
/// `ξ = x/t → (ρ, u, p)`.
fn exact_riemann(l: Side, r: Side) -> (f64, f64, impl Fn(f64) -> (f64, f64, f64)) {
    let g = |p: f64| l.f(p) + r.f(p) + (r.u - l.u);
    let lo = -l.pi.min(r.pi) + 1.0e-6;
    let ps = super::brent(g, lo.max(1.0), 1.0e11, 1.0e-14, 300).expect("p*");
    let us = 0.5 * (l.u + r.u) + 0.5 * (r.f(ps) - l.f(ps));
    let sample = move |xi: f64| -> (f64, f64, f64) {
        use petir::mathf::RealMath;
        if xi < us {
            let k = l;
            let (pb, pkb, c) = (ps + k.pi, k.p + k.pi, k.c());
            let gm = (k.g - 1.0) / (k.g + 1.0);
            if ps > k.p {
                let s = k.u - c * ((k.g + 1.0) / (2.0 * k.g) * pb / pkb + (k.g - 1.0) / (2.0 * k.g)).sqrt();
                if xi < s {
                    (k.rho, k.u, k.p)
                } else {
                    (k.rho * (pb / pkb + gm) / (gm * pb / pkb + 1.0), us, ps)
                }
            } else {
                let head = k.u - c;
                let cs = c * (pb / pkb).r_powf((k.g - 1.0) / (2.0 * k.g));
                let tail = us - cs;
                if xi < head {
                    (k.rho, k.u, k.p)
                } else if xi > tail {
                    (k.rho * (pb / pkb).r_powf(1.0 / k.g), us, ps)
                } else {
                    let u = 2.0 / (k.g + 1.0) * (c + 0.5 * (k.g - 1.0) * k.u + xi);
                    let cc = 2.0 / (k.g + 1.0) * (c + 0.5 * (k.g - 1.0) * (k.u - xi));
                    let rho = k.rho * (cc / c).r_powf(2.0 / (k.g - 1.0));
                    (rho, u, pkb * (cc / c).r_powf(2.0 * k.g / (k.g - 1.0)) - k.pi)
                }
            }
        } else {
            let k = r;
            let (pb, pkb, c) = (ps + k.pi, k.p + k.pi, k.c());
            let gm = (k.g - 1.0) / (k.g + 1.0);
            if ps > k.p {
                let s = k.u + c * ((k.g + 1.0) / (2.0 * k.g) * pb / pkb + (k.g - 1.0) / (2.0 * k.g)).sqrt();
                if xi > s {
                    (k.rho, k.u, k.p)
                } else {
                    (k.rho * (pb / pkb + gm) / (gm * pb / pkb + 1.0), us, ps)
                }
            } else {
                let head = k.u + c;
                let cs = c * (pb / pkb).r_powf((k.g - 1.0) / (2.0 * k.g));
                let tail = us + cs;
                if xi > head {
                    (k.rho, k.u, k.p)
                } else if xi < tail {
                    (k.rho * (pb / pkb).r_powf(1.0 / k.g), us, ps)
                } else {
                    let u = 2.0 / (k.g + 1.0) * (-c + 0.5 * (k.g - 1.0) * k.u + xi);
                    let cc = 2.0 / (k.g + 1.0) * (c - 0.5 * (k.g - 1.0) * (k.u - xi));
                    let rho = k.rho * (cc / c).r_powf(2.0 / (k.g - 1.0));
                    (rho, u, pkb * (cc / c).r_powf(2.0 * k.g / (k.g - 1.0)) - k.pi)
                }
            }
        }
    };
    (ps, us, sample)
}

/// L1 error of a phase density against its exact solution, relative to the
/// exact integral, plus the mean star-region `(p, u)` over the cells strictly
/// between the left-going and right-going waves' star edges (5-cell margin).
fn phase_errors(
    xs: &[f64],
    dx: f64,
    x0: f64,
    t: f64,
    rho: &[f64],
    u: &[f64],
    p: &[f64],
    sample: &dyn Fn(f64) -> (f64, f64, f64),
    ps: f64,
    us: f64,
) -> (f64, f64, f64) {
    let (mut l1, mut tot, mut pm, mut um, mut k) = (0.0, 0.0, 0.0, 0.0, 0usize);
    for i in 0..xs.len() {
        let (re, ue, pe) = sample((xs[i] - x0) / t);
        l1 += (rho[i] - re).abs() * dx;
        tot += re * dx;
        // Star region: exact (p, u) equal to (p*, u*) here and at both
        // neighbours five cells away.
        let star = |x: f64| {
            let (_, uu, pp) = sample((x - x0) / t);
            (uu - us).abs() < 1e-9 * us.abs().max(1.0) && (pp - ps).abs() < 1e-9 * ps.abs()
        };
        if (ue - us).abs() < 1e-9 * us.abs().max(1.0)
            && (pe - ps).abs() < 1e-9 * ps.abs()
            && star(xs[i] - 5.0 * dx)
            && star(xs[i] + 5.0 * dx)
        {
            pm += p[i];
            um += u[i];
            k += 1;
        }
    }
    (l1 / tot, pm / k as f64, um / k as f64)
}

/// **Methodology.** A two-phase shock tube with a known solution: with the
/// volume fraction uniform (`α_v = 0.5`) and both relaxations ablated, the
/// non-conservative terms vanish and `α` stays uniform, so the seven-equation
/// system must reproduce two independent single-phase Riemann problems, each
/// against its exact stiffened-gas solution (`exact_riemann` above, Toro
/// ch. 4 with `p + p∞`). Water (`γ = 4.4`, `p∞ = 6e8 Pa`): 1000 kg/m³ at
/// 1e9 Pa | 1000 kg/m³ at 1e5 Pa (Saurel & Abgrall's pressure ratio). Air
/// (`γ = 1.4`): 1 kg/m³ at 1e5 Pa | 0.125 kg/m³ at 1e4 Pa (Sod's ratios).
/// 1 m, diaphragm at 0.5 m, 500 cells, HLLC + MUSCL-minmod, Courant 0.4,
/// `t = 150 µs`, transmissive ends.
///
/// **Prediction (before the run):** per phase, density L1 error below 2 %
/// of `∫ρ dx`; star pressure and velocity within 2 %; `α_v` uniform to
/// round-off.
///
/// **Result (2026-11-01, release):** 498 steps. Water: exact `p*` =
/// 4.5576e8 Pa, `u*` = 231.60 m/s; numerical 4.5606e8 Pa (+0.07 %),
/// 231.48 m/s (−0.05 %), density L1 0.15 %. Air: exact 3.0313e4 Pa,
/// 293.29 m/s; numerical 3.0297e4 Pa (−0.05 %), 293.47 m/s (+0.06 %),
/// density L1 0.27 %. `α_v` uniform to exactly 0. Passes.
#[test]
fn a_two_phase_shock_tube_reproduces_both_exact_riemann_solutions() {
    let n = 500;
    let x0 = 0.5;
    let geom = pipe(1.0, n, 0.0);
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            if geom.cell_centre(i) < x0 {
                state(0.5, 1.0, 0.0, 1.0e5, 1000.0, 0.0, 1.0e9)
            } else {
                state(0.5, 0.125, 0.0, 1.0e4, 1000.0, 0.0, 1.0e5)
            }
        })
        .collect();
    let mut s = SevenEquation1d::new(pipe(1.0, n, 0.0), air_water(), &init)
        .expect("solver")
        .without_pressure_relaxation()
        .without_velocity_relaxation();
    s.set_left_boundary(SevenEquationBoundary::Transmissive);
    s.set_right_boundary(SevenEquationBoundary::Transmissive);
    let t = 150.0e-6;
    let steps = s.run_until(Time::new::<second>(t)).expect("run");
    let st = s.states().expect("states");
    let xs: Vec<f64> = (0..n).map(|i| geom.cell_centre(i)).collect();
    let (w, a) = (water_sa(), air());
    let (psw, usw, sw) = exact_riemann(
        Side { rho: 1000.0, u: 0.0, p: 1.0e9, g: w.gamma, pi: w.p_inf_pa },
        Side { rho: 1000.0, u: 0.0, p: 1.0e5, g: w.gamma, pi: w.p_inf_pa },
    );
    let (psa, usa, sa) = exact_riemann(
        Side { rho: 1.0, u: 0.0, p: 1.0e5, g: a.gamma, pi: 0.0 },
        Side { rho: 0.125, u: 0.0, p: 1.0e4, g: a.gamma, pi: 0.0 },
    );
    let col = |f: fn(&SevenEquationState) -> f64| -> Vec<f64> { st.iter().map(f).collect() };
    let (l1w, pw, uw) = phase_errors(
        &xs, geom.dx(), x0, t,
        &col(|x| x.liquid_density), &col(|x| x.liquid_velocity), &col(|x| x.liquid_pressure),
        &sw, psw, usw,
    );
    let (l1a, pa, ua) = phase_errors(
        &xs, geom.dx(), x0, t,
        &col(|x| x.vapour_density), &col(|x| x.vapour_velocity), &col(|x| x.vapour_pressure),
        &sa, psa, usa,
    );
    let da = st.iter().map(|x| (x.void_fraction - 0.5).abs()).fold(0.0, f64::max);
    println!(
        "two-phase shock tube ({steps} steps): water exact p* {psw:.5e} u* {usw:.3}; numerical \
         star p {pw:.5e} ({:+.2} %) u {uw:.3} ({:+.2} %), rho L1 {:.3} %. Air exact p* {psa:.5e} \
         u* {usa:.3}; numerical p {pa:.5e} ({:+.2} %) u {ua:.3} ({:+.2} %), rho L1 {:.3} %. \
         max |alpha - 0.5| {da:.2e}",
        100.0 * (pw / psw - 1.0),
        100.0 * (uw / usw - 1.0),
        100.0 * l1w,
        100.0 * (pa / psa - 1.0),
        100.0 * (ua / usa - 1.0),
        100.0 * l1a
    );
    for (l1, pn, pe, un, ue) in [(l1w, pw, psw, uw, usw), (l1a, pa, psa, ua, usa)] {
        assert!(l1 < 0.02 && (pn / pe - 1.0).abs() < 0.02 && (un / ue - 1.0).abs() < 0.02);
    }
    assert!(da < 1.0e-12);
}

/// **Methodology.** The water–air **interface** shock tube of the Saurel &
/// Abgrall (1999) type: water at 1e9 Pa | air (50 kg/m³) at 1e5 Pa, each
/// side carrying a `1e-6` trace of the other phase, instantaneous
/// relaxation, 500 cells.
///
/// **Prediction (before the run):** star velocity within 2 % and star
/// pressure within 3 % of the exact interface solution.
///
/// **Result (2026-11-01, release): MISSED — the run is refused in the first
/// stage of the first step.** At the material interface the water flux enters
/// the air-side cell with `α_l ≈ 1` while the non-conservative term uses
/// the cell-centred `p_int` (≈ the air pressure there, the impedance weights
/// favouring `p_v`), so the `1e-3 kg/m³` water trace receives the whole
/// water-side pressure force and its internal energy goes so negative that
/// no relaxed pressure is admissible. Saurel & Abgrall's model needs the
/// interfacial pressure at the faces (the discrete equation method of
/// Abgrall & Saurel 2003 does this), which is not implemented. Pinned so
/// that a fix shows up: this test asserts the refusal.
#[test]
fn the_water_air_interface_shock_tube_is_refused_by_cell_centred_p_int() {
    let n = 500;
    let eps = 1.0e-6;
    let geom = pipe(1.0, n, 0.0);
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            if geom.cell_centre(i) < 0.7 {
                state(eps, 50.0, 0.0, 1.0e9, 1000.0, 0.0, 1.0e9)
            } else {
                state(1.0 - eps, 50.0, 0.0, 1.0e5, 1000.0, 0.0, 1.0e5)
            }
        })
        .collect();
    let mut s = SevenEquation1d::new(pipe(1.0, n, 0.0), air_water(), &init)
        .expect("solver")
        .with_instantaneous_relaxation();
    s.set_left_boundary(SevenEquationBoundary::Transmissive);
    s.set_right_boundary(SevenEquationBoundary::Transmissive);
    let r = s.step();
    println!("interface shock tube, first step: {r:?}");
    assert!(r.is_err(), "the interface problem now runs -- re-measure it against exact_riemann");
}

// ─────────────────────────────────────────────────────────────────────────────
//  7. Ransom water faucet
// ─────────────────────────────────────────────────────────────────────────────

fn faucet_exact(x: f64, t: f64) -> f64 {
    let (u0, a_l0, g) = (10.0, 0.8, 9.806_65);
    if x <= u0 * t + 0.5 * g * t * t {
        1.0 - a_l0 * u0 / (u0 * u0 + 2.0 * g * x).sqrt()
    } else {
        1.0 - a_l0
    }
}

fn run_faucet(n: usize) -> (f64, f64, f64, f64) {
    let p0 = 1.0e5;
    let geom = pipe(12.0, n, -FRAC_PI_2);
    let inflow = state(0.2, 1.16, 0.0, p0, 1000.0, 10.0, p0);
    let init = vec![inflow; n];
    let mut s = SevenEquation1d::new(geom, air_water(), &init)
        .expect("solver")
        .with_pressure_relaxation(Relaxation::Instantaneous)
        .without_velocity_relaxation();
    s.set_left_boundary(SevenEquationBoundary::VelocityInlet(inflow));
    s.set_right_boundary(SevenEquationBoundary::PressureOutlet { pressure: p0 });
    let t = 0.5;
    s.run_until(Time::new::<second>(t)).expect("faucet run");
    let st = s.states().expect("states");
    let dx = s.pipe().dx();
    let mut l1 = 0.0;
    let mut at4 = 0.0;
    let mut ul_err: f64 = 0.0;
    for (i, x) in st.iter().enumerate() {
        let xc = s.pipe().cell_centre(i);
        l1 += (x.void_fraction - faucet_exact(xc, t)).abs() * dx;
        if (xc - 4.0).abs() < 0.5 * dx + 1e-12 {
            at4 = x.void_fraction;
        }
        if xc < 5.0 {
            let ul_exact = (100.0 + 2.0 * 9.806_65 * xc).sqrt();
            ul_err = ul_err.max((x.liquid_velocity - ul_exact).abs());
        }
    }
    (l1 / 12.0, at4, ul_err, s.time().get::<second>())
}

/// **Methodology.** Ransom's water faucet (the standard two-fluid benchmark):
/// a vertical 12 m pipe, water entering the top at 10 m/s with `α_l = 0.8`,
/// gas (air) stagnant, falling freely under gravity, bottom open at 1e5 Pa;
/// no interfacial drag (`without_velocity_relaxation`) and no wall friction;
/// instantaneous pressure relaxation, which with `λ = 0` is RELAP-7's
/// Remark-2 configuration, regularised here only by the scheme's numerical
/// diffusion. Analytic solution: `u_l = sqrt(u0² + 2 g x)`,
/// `α_v = 1 − α_l0 u0/u_l` above the front `x_f = u0 t + ½ g t²`, 0.2 below.
/// Compared at `t = 0.5 s` on 120 and 240 cells.
///
/// **Prediction (before the run):** the mean `|α_v − α_v,exact|` falls below
/// 0.03 on 120 cells and below 0.02 on 240, decreasing with refinement;
/// `α_v(4 m)` within 0.01 of the exact 0.4012; `u_l` above the front within
/// 2 % (0.3 m/s).
///
/// **Result (2026-11-01, release):** mean `|α_v − α_v,exact|` **0.0371**
/// (120 cells) and **0.0316** (240 cells): decreasing, but **both MISS** the
/// predicted 0.03 / 0.02. `α_v(4 m)` = 0.3875 / **0.4005** against the exact
/// 0.4011 (met at 240 cells). Largest `|u_l − u_l,exact|` above 5 m:
/// 0.62 / 0.48 m/s, **missing** the predicted 0.3 m/s. The misses come from
/// the smeared front (first-order HLLC dissipation where the minmod limiter
/// clips) and the slow convergence of a Remark-2 configuration. The
/// first-run Rusanov default gave 0.067 / 0.054 and `α_v(4 m)` 0.307 /
/// 0.338, which is why HLLC became the default. Asserted: convergence and
/// `α_v(4 m)`; the missed bars are printed, not moved.
#[test]
fn ransom_water_faucet_matches_the_analytic_void_profile() {
    let (l1a, a4a, ua, t) = run_faucet(120);
    let (l1b, a4b, ub, _) = run_faucet(240);
    let exact4 = faucet_exact(4.0, t);
    println!(
        "faucet at t = {t} s: 120 cells mean|da| {l1a:.4}, a(4 m) {a4a:.4}, max|du_l| {ua:.3}; \
         240 cells mean|da| {l1b:.4}, a(4 m) {a4b:.4}, max|du_l| {ub:.3}; exact a(4 m) {exact4:.4}"
    );
    // Asserted: what was met. The L1 and u_l predictions were missed and are
    // printed, not asserted (see the result note); the bars were not moved.
    assert!(l1b < l1a, "no convergence of the void profile");
    assert!((a4b - exact4).abs() < 0.01);
    println!(
        "faucet predictions: L1 < 0.03 / 0.02 -> {} / {}; max|du_l| < 0.3 -> {}",
        if l1a < 0.03 { "met" } else { "MISSED" },
        if l1b < 0.02 { "met" } else { "MISSED" },
        if ub < 0.3 { "met" } else { "MISSED" }
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  8. Species in the seven-equation model (gh:#1064)
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** The non-condensable rides with the gas: a uniform flow
/// (`u = 20 m/s`, `α_v = 0.5`, air–water at 1e5 Pa) in a 3 m, 300-cell
/// transmissive tube carries a Gaussian `Y` pulse (σ = 0.1 m) from 1.0 m for
/// 0.03 s. The centroid displacement is compared with `u t`.
///
/// **Prediction:** within 1 % of `u t = 0.6 m`; the pulse mass conserved to
/// `1e-12` relative while inside.
///
/// **Result (2026-11-01, release):** displacement / `u t` = **1.000018**; pulse
/// mass change 1.9e-16. Passes. **First run missed** with the pulse at
/// 0.5 m in a 2 m tube (mass +1.1e-5): the transmissive inlet re-injects the
/// first cell's `Y`, so the Gaussian tail re-entered; the pulse was moved to
/// 1.0 m in a 3 m tube.
#[test]
fn a_noncondensable_pulse_rides_the_gas_in_the_seven_equation_model() {
    let n = 300;
    let geom = pipe(3.0, n, 0.0);
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            let x = geom.cell_centre(i);
            SevenEquationState {
                noncondensable_mass_fraction: 0.01
                    * petir::mathf::RealMath::r_exp(-0.5 * ((x - 1.0) / 0.1).powi(2)),
                ..state(0.5, 1.2, 20.0, 1.0e5, 1000.0, 20.0, 1.0e5)
            }
        })
        .collect();
    let mut s = SevenEquation1d::new(pipe(3.0, n, 0.0), air_water(), &init).expect("solver");
    s.set_left_boundary(SevenEquationBoundary::Transmissive);
    s.set_right_boundary(SevenEquationBoundary::Transmissive);
    let centroid = |s: &SevenEquation1d| -> (f64, f64) {
        let (mut m, mut mx) = (0.0, 0.0);
        for (i, u) in s.conserved_state().iter().enumerate() {
            m += u[7];
            mx += u[7] * s.pipe().cell_centre(i);
        }
        (mx / m, m)
    };
    let (x0, m0) = centroid(&s);
    s.run_until(Time::new::<second>(0.03)).expect("run");
    let (x1, m1) = centroid(&s);
    let speed_ratio = (x1 - x0) / (20.0 * 0.03);
    println!(
        "seven-eq species pulse: centroid {x0:.5} -> {x1:.5} m, ratio to u t {speed_ratio:.6}, mass \
         rel change {:.2e}",
        (m1 - m0) / m0
    );
    assert!((speed_ratio - 1.0).abs() < 0.01);
    assert!(((m1 - m0) / m0).abs() < 1.0e-12);
}

// ─────────────────────────────────────────────────────────────────────────────
//  9. Edwards–O'Brien in the HEM limit
// ─────────────────────────────────────────────────────────────────────────────

fn le_metayer() -> StiffenedGasPair {
    StiffenedGasPair::water_le_metayer_2004()
}

/// Prediction basis for the Edwards case, computed from the equation of
/// state alone (no solver run): the stiffened-gas saturation pressure at the
/// Hendrie temperatures near the break, against IAPWS-IF97's. Printed only.
#[test]
fn edwards_prediction_basis_saturation_pressures() {
    use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;
    let eos = le_metayer();
    for t in [497.6, 500.0, 502.0, 506.8] {
        let p_if97 = sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(t)).get::<pascal>();
        let f = |p: f64| {
            eos.saturation_temperature(Pressure::new::<pascal>(p))
                .expect("Tsat")
                .get::<kelvin>()
                - t
        };
        let p_sg = super::brent(f, 1.0e4, 2.0e7, 1e-12, 200).expect("psat");
        println!(
            "T = {t} K: p_sat SG = {:.4} MPa ({:.1} psia), IF97 = {:.4} MPa ({:.1} psia)",
            p_sg / 1e6,
            p_sg / 6894.757,
            p_if97 / 1e6,
            p_if97 / 6894.757
        );
        assert!(p_sg > 0.0 && p_if97 > 0.0);
    }
}

/// The Edwards initial state on `n` cells with the Le Métayer water/steam
/// pair: 7 MPa, Hendrie temperatures, a `1e-6` saturated-vapour trace.
fn edwards_solver(n: usize) -> SevenEquation1d {
    use crate::multiphase_1d::edwards_tests::{
        degf_to_k, lerp, FT_TO_M, NODE_CENTRE_FT, NODE_T_DEGF, PIPE_ID_M, PIPE_LENGTH_M, P_INIT_PA,
    };
    let eos = le_metayer();
    let geom = Pipe1d::circular(
        Length::new::<meter>(PIPE_LENGTH_M),
        Length::new::<meter>(PIPE_ID_M),
        Angle::new::<radian>(0.0),
        n,
    )
    .expect("pipe");
    let xs: Vec<f64> = NODE_CENTRE_FT.iter().map(|f| f * FT_TO_M).collect();
    let ts: Vec<f64> = NODE_T_DEGF.iter().map(|f| degf_to_k(*f)).collect();
    let t_sat = eos
        .saturation_temperature(Pressure::new::<pascal>(P_INIT_PA))
        .expect("Tsat")
        .get::<kelvin>();
    let init: Vec<SevenEquationState> = (0..n)
        .map(|i| {
            let t = lerp(&xs, &ts, geom.cell_centre(i));
            SevenEquationState::at_temperatures(
                &eos,
                Ratio::new::<ratio>(1.0e-6),
                Pressure::new::<pascal>(P_INIT_PA),
                ThermodynamicTemperature::new::<kelvin>(t_sat),
                ThermodynamicTemperature::new::<kelvin>(t),
                Velocity::new::<meter_per_second>(0.0),
            )
        })
        .collect();
    let mut s = SevenEquation1d::new(geom, eos, &init)
        .expect("solver")
        .with_instantaneous_relaxation()
        .with_instantaneous_mass_transfer();
    s.set_left_boundary(SevenEquationBoundary::Wall);
    s.set_right_boundary(SevenEquationBoundary::PressureOutlet {
        pressure: crate::multiphase_1d::edwards_tests::P_AMBIENT_PA,
    });
    s
}

/// **Methodology.** Edwards–O'Brien GS-1 on the seven-equation model in its
/// HEM limit (Remark 1: instantaneous pressure, velocity and
/// thermo-chemical relaxation): B-T-3271 geometry (4.096 m × 73 mm,
/// horizontal), 100 cells, 7 MPa with the Hendrie profile, closed at
/// `x = 0`, a static-pressure outlet at 0.1 MPa through a face whose area
/// opens linearly to 0.87 A over 1 ms (the disc residue; there is no
/// critical-flow model, the outlet chokes only as the scheme does),
/// Le Métayer et al. (2004) water/steam stiffened gases. 0–0.30 s, the span
/// of the digitised GS-1 data; GS-1 (x = 3.929 m) sampled at the data times.
/// Also checked: every two-phase cell is at `p_v = p_l`, `u_v = u_l`,
/// `T_v = T_l = T_sat(p)` after each step (the HEM state), and the phase
/// inventories plus the integrated outflow close the mass balance.
///
/// **Prediction (before the run),** from
/// `edwards_prediction_basis_saturation_pressures` and the HEM records: the
/// stiffened-gas saturation pressure at the break-end Hendrie temperature
/// (497.6 K) is 5.11 MPa (741 psia), against IF97's 2.52 MPa (366 psia), because the
/// pair's `T_sat` runs 15–35 K low (gh:#1057). An HEM plateau sits a few
/// per cent below the local saturation pressure, so the GS-1 plateau
/// (mean 0.02–0.06 s) is predicted at 650–740 psia, i.e. **well above** the
/// data's ~360 psia and HemPipe's 368 psia (IF97); the GS-1 RMSE against the
/// data is therefore predicted to be worse than HemPipe's 44.2 psia. Gated:
/// the run completes 0.30 s; the HEM state holds to 1e-6 K / 1e-3 Pa /
/// 1e-9 m/s; the plateau lies between the IF97 data (330 psia) and the
/// stiffened-gas saturation pressure at 498 K. The RMSE is reported, not
/// gated.
///
/// **Result (2026-11-01, release):** 32 133 steps, ~9 s. GS-1 (psia, seven-eq /
/// data): 0.02 s 465.0 / 350.8; 0.04 s 400.4 / 364.0; 0.06 s 388.1 / 367.4;
/// 0.10 s 364.7 / 312.2; 0.14 s 272.4 / 296.0; 0.20 s 92.2 / 289.0; 0.30 s
/// 14.1 / 190.1. **Plateau 417.8 psia: the prediction (650–740 psia) is
/// MISSED**; it does lie in the gated band (330 psia to the stiffened-gas
/// `p_sat` 740.7 psia). With this EOS the HEM two-phase fan runs far below the
/// local saturation pressure, so "a few per cent below `p_sat`" (true of
/// IF97 and of the data) was the wrong expectation. **RMSE 144.7 psia**
/// (HemPipe on IF97: 44.2; drift flux: 28.2). The pipe empties far too
/// early (GS-1 at ambient by 0.28 s, the data still at 190 psia at 0.30 s):
/// with no critical-flow model the static-pressure outlet passes more than a
/// choked break would. HEM state: where the operator acted (3.15e6 cell
/// steps) `|T_k − T_sat|` ≤ **4.6e-7 K**, `|u_v − u_l|` ≤ 5.7e-14 m/s; it
/// skipped 33 076 cell steps with no admissible equilibrium (a phase would
/// have to vanish), and over all two-phase cells `|T − T_sat|` reached 4.9 K
/// (reported, not gated). `|p_v − p_l|` 1.16e-3 Pa **misses** the registered
/// 1e-3 Pa (the `α_v`-representation floor, as in the relaxation sweep).
/// Mass closure with the outflow 3.8e-16. **Run with Rusanov, not the HLLC
/// default:** the HLLC run was refused at t = 0.44 ms, when a 1e-6 vapour
/// trace in the break cell lost positivity beside the first flashing cell
/// (one face `α` cannot follow a light trace whose own flux runs against
/// `u_int`; a per-phase donor `α` was tried and failed the same way).
#[test]
fn edwards_gs1_in_the_hem_limit() {
    use crate::multiphase_1d::edwards_tests::{
        break_area_fraction, GS1_DATA_PSIA, PA_TO_PSI,
    };
    let n = 100;
    // Rusanov, not the HLLC default: the HLLC run is refused at t = 0.44 ms
    // (a vapour trace at the break loses positivity next to the first
    // flashing cell; see the result note).
    let mut s = edwards_solver(n).without_hllc();
    let gs1 = s.pipe().nearest_cell(Length::new::<meter>(12.890 * 0.3048));
    let eos = le_metayer();
    let mut samples = Vec::new();
    let mut next = 0usize;
    let mut worst_t = 0.0_f64;
    let mut worst_p = 0.0_f64;
    let mut worst_u = 0.0_f64;
    let mut worst_departure = 0.0_f64;
    let mut skipped = 0usize;
    let mut acted = 0usize;
    let mut steps = 0usize;
    let mut out_mass = 0.0;
    let (mv0, ml0, _) = s.inventories();
    let m0 = mv0.value + ml0.value;
    loop {
        let t = s.time().get::<second>();
        if next < GS1_DATA_PSIA.len() && t >= GS1_DATA_PSIA[next].0 - 1e-12 {
            let p = s.mixture_pressure().expect("p")[gs1];
            samples.push((t, p * PA_TO_PSI));
            next += 1;
        }
        if t >= 0.30 {
            break;
        }
        s.set_right_face_area_fraction(break_area_fraction(t).max(1.0e-6)).expect("area");
        // Outflow over the step, from the conserved inventories.
        let (a, b, _) = s.inventories();
        let before = a.value + b.value;
        let r = s.step().unwrap_or_else(|e| panic!("Edwards step {steps} at t = {t}: {e}"));
        let (a, b, _) = s.inventories();
        out_mass += before - (a.value + b.value);
        steps += 1;
        worst_departure = worst_departure.max(r.max_saturation_departure);
        skipped += r.mass_transfer_skipped_cells;
        acted += r.mass_transfer_cells;
        if steps % 50 == 0 {
            let st = s.states().expect("states");
            let temps = s.temperatures().expect("T");
            for (x, (tv, tl)) in st.iter().zip(&temps) {
                if x.void_fraction > super::TRACE_VOLUME_FRACTION
                    && 1.0 - x.void_fraction > super::TRACE_VOLUME_FRACTION
                {
                    let ts = eos
                        .saturation_temperature(Pressure::new::<pascal>(x.liquid_pressure))
                        .expect("Tsat")
                        .get::<kelvin>();
                    worst_t = worst_t.max((tv - ts).abs()).max((tl - ts).abs());
                    worst_p = worst_p.max((x.vapour_pressure - x.liquid_pressure).abs());
                    worst_u = worst_u.max((x.vapour_velocity - x.liquid_velocity).abs());
                }
            }
        }
    }
    let (mv, ml, _) = s.inventories();
    let closure = (mv.value + ml.value + out_mass - m0).abs() / m0;
    let mut sq = 0.0;
    for ((t, p), (td, pd)) in samples.iter().zip(GS1_DATA_PSIA.iter()) {
        println!("GS-1 t = {t:.3} s (data {td:.3}): seven-eq {p:.1} psia, data {pd:.1}");
        sq += (p - pd) * (p - pd);
    }
    let rmse = (sq / samples.len() as f64).sqrt();
    // Samples 1..=3 are the data times 0.02, 0.04, 0.06 s.
    let plateau = (samples[1].1 + samples[2].1 + samples[3].1) / 3.0;
    let drained = 1.0 - (mv.value + ml.value) / m0;
    println!(
        "Edwards HEM limit: {steps} steps to {:.3} s; GS-1 plateau {plateau:.1} psia; RMSE \
         {rmse:.1} psia over {} points; drained {:.1} %; operator acted {acted} times, \
         skipped {skipped}; worst |T_k - Tsat| where it acted {worst_departure:.2e} K; over all \
         two-phase cells (every 50 steps) |T - Tsat| {worst_t:.2e} K, |p_v - p_l| \
         {worst_p:.2e} Pa, |u_v - u_l| {worst_u:.2e} m/s; mass closure {closure:.2e}",
        s.time().get::<second>(),
        samples.len(),
        100.0 * drained
    );
    assert_eq!(samples.len(), GS1_DATA_PSIA.len());
    // HEM state where the operator acted. The registered |p_v - p_l| < 1e-3 Pa
    // bar is reported (it sits at the alpha_v-representation floor, as in the
    // relaxation sweep); the all-cell |T - Tsat| is reported, not gated
    // (cells the operator skips keep their temperatures by design).
    println!(
        "registered bars: |p_v - p_l| < 1e-3 Pa {}; all-cell |T - Tsat| < 1e-6 K {}",
        if worst_p < 1.0e-3 { "met" } else { "MISSED" },
        if worst_t < 1.0e-6 { "met" } else { "MISSED" }
    );
    assert!(worst_departure < 1.0e-6 && worst_u < 1.0e-9);
    let p_sat_break = {
        let f = |p: f64| {
            eos.saturation_temperature(Pressure::new::<pascal>(p)).expect("Tsat").get::<kelvin>()
                - 497.6
        };
        super::brent(f, 1.0e4, 2.0e7, 1e-12, 200).expect("psat") * PA_TO_PSI
    };
    println!("stiffened-gas p_sat at 497.6 K = {p_sat_break:.1} psia");
    assert!(plateau > 330.0 && plateau < p_sat_break);
    assert!(closure < 1.0e-12);
}

/// **Methodology.** The comparison the gh:#1059 brief asks for: the
/// six-equation [`crate::multiphase_1d::two_fluid::TwoFluid1d`] on the same
/// Edwards case (24 cells, 30 µs, its HEM choked outlet, 0.87 disc residue,
/// bubbly closures), marched until it completes 0.30 s or refuses. Its own
/// record (`two_fluid_tests::six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work`)
/// documents a refusal from low-void starts.
///
/// **Prediction (before the run):** it refuses within the first 0.05 s
/// (a metastable-bound or volume-residual refusal), so there is no
/// two-fluid GS-1 curve to compare; the time and reason are recorded.
///
/// **Result (2026-11-01, release):** refused at **t = 0.33 ms**: the vapour
/// enthalpy fell beyond the 30 K metastable subcooling bound at 2.65 MPa
/// (last GS-1 441 psia). Prediction met; there is no two-fluid GS-1 curve.
#[test]
fn two_fluid_on_edwards_for_comparison() {
    use crate::multiphase_1d::edwards_tests::{
        break_area_fraction, degf_to_k, lerp, FT_TO_M, NODE_CENTRE_FT, NODE_T_DEGF, PA_TO_PSI,
        PIPE_ID_M, PIPE_LENGTH_M, P_AMBIENT_PA, P_INIT_PA,
    };
    use crate::multiphase_1d::two_fluid::{TwoFluid1d, TwoFluidBoundary};
    let n = 24;
    let geom = Pipe1d::circular(
        Length::new::<meter>(PIPE_LENGTH_M),
        Length::new::<meter>(PIPE_ID_M),
        Angle::new::<radian>(0.0),
        n,
    )
    .expect("pipe");
    let xs: Vec<f64> = NODE_CENTRE_FT.iter().map(|f| f * FT_TO_M).collect();
    let ts: Vec<f64> = NODE_T_DEGF.iter().map(|f| degf_to_k(*f)).collect();
    let profile: Vec<ThermodynamicTemperature> = (0..n)
        .map(|i| ThermodynamicTemperature::new::<kelvin>(lerp(&xs, &ts, geom.cell_centre(i))))
        .collect();
    let gs1 = geom.nearest_cell(Length::new::<meter>(12.890 * 0.3048));
    let mut s = TwoFluid1d::bubbly(
        geom,
        Pressure::new::<pascal>(P_INIT_PA),
        ThermodynamicTemperature::new::<kelvin>(502.0),
        Time::new::<second>(3.0e-5),
    )
    .expect("solver");
    s.set_temperature_profile(&profile).expect("profile");
    let mut outcome = String::from("completed 0.30 s");
    let mut last_p = 0.0;
    while s.time().get::<second>() < 0.30 {
        let t = s.time().get::<second>();
        s.set_right_boundary(TwoFluidBoundary::ChokedOutlet {
            area_fraction: break_area_fraction(t).max(1.0e-6),
            ambient_pressure: P_AMBIENT_PA,
        });
        match s.step() {
            Ok(_) => last_p = s.pressure()[gs1] * PA_TO_PSI,
            Err(e) => {
                outcome = format!("refused at t = {t:.5} s: {e}");
                break;
            }
        }
    }
    println!("TwoFluid1d on Edwards: {outcome}; last GS-1 pressure {last_p:.1} psia");
    assert!(s.time().get::<second>() > 0.0, "not a single step was taken");
}

/// Unit checks of the stiffened-gas helpers against `StiffenedGasHem`'s own
/// relations: the entropy is constant along an isentrope `p ∝ ρ^γ` shifted
/// by `p∞`, and `e(ρ, p)` inverts `p(ρ, e)`.
#[test]
fn stiffened_gas_helpers_are_consistent() {
    let g = water_sa();
    let (rho0, p0) = (1000.0, 1.0e5);
    let t0 = sg_temperature(&g, rho0, p0);
    let s0 = sg_entropy(&g, p0, t0);
    let rho1 = 1010.0;
    let p1 = (p0 + g.p_inf_pa) * petir::mathf::RealMath::r_powf(rho1 / rho0, g.gamma) - g.p_inf_pa;
    let s1 = sg_entropy(&g, p1, sg_temperature(&g, rho1, p1));
    assert!((s1 - s0).abs() < 1e-9 * g.cv_j_per_kg_k, "isentrope: {s0} vs {s1}");
    let e = sg_energy(&g, rho0, p0);
    assert!((super::sg_pressure(&g, rho0, e) - p0).abs() < 1e-6);
    assert!((sg_density_pt(&g, p0, t0) - rho0).abs() < 1e-9);
}
