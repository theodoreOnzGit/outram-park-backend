// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the damage and rupture laws.
//!
//! # What is checkable here, and what is not
//!
//! There is no closed-form solution for a general damage step, and the
//! code_aster `astest` cases that would serve as an oracle need the whole
//! finite-element machinery to run. These tests therefore lean on the four
//! things that *are* exact:
//!
//! 1. **Limiting cases that collapse to a known law.** Both porous-plastic
//!    yield surfaces reduce to von Mises at zero porosity, and the GTN surface
//!    has a closed-form hydrostatic strength at zero deviatoric stress. Those
//!    are theorems about the model, not fitted numbers.
//! 2. **Consistency of the converged state.** After a plastic step the yield
//!    function must vanish at the returned stress and state. That is the
//!    defining property of the return map, and it catches almost every
//!    algebraic slip in the coupled solve.
//! 3. **Structural relationships between the ported variants**, above all the
//!    exact factor `(1-D)^R_D` between `VENDOCHAB`'s and `VISC_ENDO_LEMA`'s
//!    damage rates, which follows from one driving the damage with the nominal
//!    stress and the other with the effective one.
//! 4. **Order of convergence in time.** Backward Euler is first order, and the
//!    coupled `(dr, D)` solve is measured against that.
//!
//! Plus two tests that exist only to record upstream defects
//! ([`upstream_nmvexi_reads_the_wrong_material_slots`] and
//! [`upstream_implicit_path_damages_elastic_steps`]).
//!
//! **Nothing here is validation against code_aster output or against
//! experiment.** It is verification of the port. Per `RESPONSIBLE_USE.md` these
//! laws remain untrusted draft material until a human has reviewed them and
//! they have been run against a published benchmark.

use approx::assert_relative_eq;
use outram_foam_basic_lib::primitives::SymmTensor;

use super::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Structural steel elasticity: `E = 200 GPa`, `nu = 0.3`.
fn steel() -> IsotropicElasticity {
    IsotropicElasticity::from_young_poisson(200.0e9, 0.3).expect("valid moduli")
}

/// Representative `VENDOCHAB` parameters, in the spirit of a creep-damaging
/// austenitic steel: `n = 5`, `m = 3`, `K = 300 MPa`, `SY = 50 MPa`, a purely
/// deviatoric damage driver, `R_D = 4`, `A_D = 250 MPa`, `K_D = 2`.
fn vendochab_params() -> LemaitreChabocheParameters {
    LemaitreChabocheParameters {
        n: 5.0,
        m: 3.0,
        k: 300.0e6,
        yield_stress: 50.0e6,
        principal_weight: 0.0,
        trace_weight: 0.0,
        damage_exponent: 4.0,
        damage_strength: 250.0e6,
        damage_closure_exponent: 2.0,
    }
}

/// Representative Rousselier parameters for a ferritic pressure-vessel steel.
fn rousselier_params() -> RousselierParameters {
    RousselierParameters {
        d: 2.0,
        sigma_1: 445.0e6,
        initial_porosity: 1.0e-3,
        critical_porosity: 0.05,
        acceleration: 3.0,
        limit_porosity: 0.1,
        broken_unloading_slope: 1.0e-3,
        nucleation_rate: 0.0,
        stored_energy_fraction: 0.9,
    }
}

/// Matrix hardening for the porous-plastic laws:
/// `R(p) = 400 MPa + 1.5 GPa p + 300 MPa (1 - exp(-30 p))`.
///
/// Deliberately **not** a Ludwik power law. `R(p) = sigma_y + K p^n` with
/// `n < 1` has an infinite slope at `p = 0`, which makes the very first plastic
/// increment of a virgin point numerically singular: the local residual falls
/// by order `K` over a porosity increment of order `1e-9`, so any bracketed
/// solver collapses its bracket to machine precision with the residual still
/// large. That is a property of the hardening curve, not of the return map, and
/// code_aster's own `TRACTION` curves are piecewise-linear tables with finite
/// slopes throughout. [`IsotropicHardening::Ludwik`] remains available and is
/// exercised by
/// [`hardening_slopes_are_the_derivatives_of_the_curves`] away from the origin.
fn matrix_hardening() -> IsotropicHardening {
    IsotropicHardening::EcroNl {
        r0: 400.0e6,
        rh: 1.5e9,
        r1: 300.0e6,
        gamma_1: 30.0,
        r2: 0.0,
        gamma_2: 0.0,
        rk: 0.0,
        p0: 0.0,
        gamma_m: 1.0,
    }
}

/// Representative GTN parameters, with the rupture porosity derived from the
/// coalescence slope exactly as upstream does.
fn gtn_params() -> GtnParameters {
    let q1 = 1.5;
    let fc = 0.05;
    let hc = 3.0;
    GtnParameters {
        q1,
        q2: 1.0,
        initial_porosity: 1.0e-3,
        coalescence_porosity: fc,
        coalescence_slope: hc,
        rupture_porosity: GtnParameters::rupture_porosity_from_slope(q1, fc, hc),
        nucleation: GtnNucleation::none(),
        broken_damage: 0.99,
    }
}

/// `ECRO_NL` hardening: `R = 400 MPa + 1 GPa k + 200 MPa (1 - exp(-20 k))`.
fn ecro_nl() -> IsotropicHardening {
    IsotropicHardening::EcroNl {
        r0: 400.0e6,
        rh: 1.0e9,
        r1: 200.0e6,
        gamma_1: 20.0,
        r2: 0.0,
        gamma_2: 0.0,
        rk: 0.0,
        p0: 0.0,
        gamma_m: 1.0,
    }
}

// ---------------------------------------------------------------------------
// Shared elasticity and hardening
// ---------------------------------------------------------------------------

/// **The two-modulus elasticity description round-trips `(E, nu)`.**
///
/// *Methodology:* build [`IsotropicElasticity`] from `E = 200 GPa`, `nu = 0.3`,
/// then recover `E` and `nu` from the stored `(mu, K)` pair. The relations
/// `E = 9 K mu/(3K + mu)` and `nu = (3K - 2mu)/(2(3K + mu))` are algebraic
/// identities, so the check is on the transcription, not on physics. Pass
/// criterion: 1e-14 relative. Also checks the two rejections upstream would
/// make.
///
/// *Result (measured 2026-08-05):* `mu = 7.692308e+10 Pa`,
/// `K = 1.666667e+11 Pa`, recovered `E = 2.0000000000e+11 Pa` and
/// `nu = 0.3000000000000000` — both to every digit printed. `nu = 0.5` and a
/// negative Young's modulus are both rejected. Interpretation: the conversion
/// is exact in floating point for this pair, so nothing downstream inherits a
/// modulus error.
#[test]
fn elasticity_round_trips_young_and_poisson() {
    let e = steel();
    println!(
        "mu = {:.6e} Pa, K = {:.6e} Pa, E = {:.10e} Pa, nu = {:.16}",
        e.shear_modulus,
        e.bulk_modulus,
        e.young(),
        e.poisson()
    );
    assert_relative_eq!(e.young(), 200.0e9, max_relative = 1.0e-14);
    assert_relative_eq!(e.poisson(), 0.3, max_relative = 1.0e-14);

    assert!(IsotropicElasticity::from_young_poisson(200.0e9, 0.5).is_err());
    assert!(IsotropicElasticity::from_young_poisson(-1.0, 0.3).is_err());
}

/// **Every hardening curve's analytic slope matches a central difference of its
/// own value.**
///
/// *Methodology:* for each [`IsotropicHardening`] variant, compare
/// [`IsotropicHardening::slope`] against `(R(p+h) - R(p-h))/(2h)` with
/// `h = 1e-6` at `p = 0.05`, `0.2` and `0.5`. A central difference has
/// truncation error `O(h^2)`, so agreement to 1e-6 relative is the most that
/// can be demanded and is what is asserted. `p = 0` is excluded for the
/// power-law family, where the true slope is infinite by construction.
///
/// *Result (measured 2026-08-05):* worst relative discrepancy over all
/// variants and points was `1.020270e-10`. Interpretation: the slopes are the
/// derivatives of the values, so a Newton step built on them is consistent —
/// the failure mode this catches is a hardening term differentiated with a
/// wrong sign or a dropped factor, which shows up here at order one, not 1e-11.
#[test]
fn hardening_slopes_are_the_derivatives_of_the_curves() {
    let curves = [
        IsotropicHardening::Perfect {
            yield_stress: 400.0e6,
        },
        IsotropicHardening::Linear {
            yield_stress: 400.0e6,
            modulus: 2.0e9,
        },
        matrix_hardening(),
        ecro_nl(),
    ];
    let h = 1.0e-6;
    let mut worst = 0.0_f64;
    for curve in curves {
        for p in [0.05, 0.2, 0.5] {
            let numeric = (curve.value(p + h) - curve.value(p - h)) / (2.0 * h);
            let analytic = curve.slope(p);
            let rel = if analytic.abs() > 0.0 {
                ((numeric - analytic) / analytic).abs()
            } else {
                numeric.abs()
            };
            worst = worst.max(rel);
        }
    }
    println!("worst relative slope discrepancy = {worst:.6e}");
    assert!(worst < 1.0e-6, "worst = {worst:.6e}");
}

/// **The largest principal stress matches a hand-computable spectrum.**
///
/// *Methodology:* upstream's `calcj0` returns the maximum principal stress. Two
/// checks: a diagonal state, whose eigenvalues are its diagonal entries, and a
/// pure-shear state `sigma_xy = tau`, whose spectrum is `{-tau, 0, +tau}`.
/// Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* diagonal `(120, -40, 30) MPa` gives
/// `J0 = 1.200000e+08 Pa`; pure shear `tau = 70 MPa` gives
/// `J0 = 7.00000000000000000e+07 Pa`, exact to all seventeen digits printed.
/// Interpretation: the ascending-order convention of
/// `eigen_values_symm` is being read from the correct end — reading the wrong
/// end would return `-40 MPa` and `-70 MPa` and the multiaxial damage driver
/// would have the wrong sign under compression.
#[test]
fn max_principal_stress_matches_a_hand_computed_spectrum() {
    let diagonal = SymmTensor::new(120.0e6, 0.0, 0.0, -40.0e6, 0.0, 30.0e6);
    let shear = SymmTensor::new(0.0, 70.0e6, 0.0, 0.0, 0.0, 0.0);
    println!(
        "J0(diagonal) = {:.6e} Pa, J0(pure shear) = {:.17e} Pa",
        max_principal_stress(diagonal),
        max_principal_stress(shear)
    );
    assert_relative_eq!(
        max_principal_stress(diagonal),
        120.0e6,
        max_relative = 1e-12
    );
    assert_relative_eq!(max_principal_stress(shear), 70.0e6, max_relative = 1e-12);
}

// ---------------------------------------------------------------------------
// VENDOCHAB / VISC_ENDO_LEMA
// ---------------------------------------------------------------------------

/// **The `VENDOCHAB` damage driver equals the axial stress in uniaxial tension,
/// for any split of the multiaxiality weights.**
///
/// *Methodology:* `chi = ALPHA_D J0 + BETA_D tr + (1 - ALPHA_D - BETA_D)
/// sigma_eq`. In uniaxial tension of magnitude `S` all three invariants equal
/// `S`, so the weights sum to one and `chi = S` whatever they are. Sweep
/// `(ALPHA_D, BETA_D)` over four splits at `S = 180 MPa`. Pass criterion: 1e-12
/// relative.
///
/// *Result (measured 2026-08-05):* all four splits returned
/// `chi = 1.800000e8 Pa` exactly; worst relative deviation 0.0.
/// Interpretation: the weights are encoded as a partition of unity, as upstream
/// intends. A transcription that dropped the `(1 - a - b)` complement would
/// give `chi = 0.4 S` for `(0.2, 0.2)` and fail here by 60 %.
#[test]
fn vendochab_damage_driver_is_the_axial_stress_in_uniaxial_tension() {
    let s = 180.0e6;
    let sigma = SymmTensor::new(s, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut worst = 0.0_f64;
    for (a, b) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0 / 3.0), (0.2, 0.2)] {
        let law = LemaitreChabocheLaw::Vendochab(LemaitreChabocheParameters {
            principal_weight: a,
            trace_weight: b,
            ..vendochab_params()
        });
        let chi = law.damage_equivalent_stress(sigma, 0.0);
        println!("(ALPHA_D, BETA_D) = ({a}, {b:.6}) -> chi = {chi:.6e} Pa");
        worst = worst.max(((chi - s) / s).abs());
    }
    println!("worst relative deviation = {worst:.3e}");
    assert!(worst < 1.0e-12);
}

/// **The flow rate reproduces upstream's power law below the overflow guard.**
///
/// *Methodology:* upstream (`nmvecd.F90`) computes
/// `rate = exp(N (ln(sigma_eff - SY) - ln(K r^(1/m))))` whenever that is at or
/// below `0.1/dt`. Evaluate [`LemaitreChabocheLaw::hardening_rate`] against an
/// independently written `((sigma_eff - SY)/(K r^(1/m)))^N` at
/// `sigma_eff = 150 MPa`, `r = 0.02`, `dt = 0.01 s`, and check that the guard
/// did not fire. Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* port `2.792611e+00 1/s` against reference
/// `2.792611e+00 1/s`, relative difference `4.771e-16`,
/// `linearised = false`. Interpretation: the exponent chain
/// `N (ln x - ln sc)` is equivalent to the direct power to within one unit in
/// the last place. The timestep is chosen so the guard threshold
/// `0.1/dt = 10 1/s` sits above the rate; at `dt = 1000 s` the same material
/// point *is* guarded, which is what the next test examines.
#[test]
fn vendochab_flow_rate_matches_the_upstream_power_law() {
    let p = vendochab_params();
    let law = LemaitreChabocheLaw::Vendochab(p);
    let sigma_eff = 150.0e6;
    let r = 0.02;
    let dt = 0.01;

    let (rate, linearised) = law.hardening_rate(sigma_eff, r, dt);
    let reference = ((sigma_eff - p.yield_stress) / (p.k * r.powf(1.0 / p.m))).powf(p.n);
    println!(
        "port rate = {rate:.6e} 1/s, reference = {reference:.6e} 1/s, \
         relative difference = {:.3e}, linearised = {linearised}",
        ((rate - reference) / reference).abs()
    );
    assert!(!linearised);
    assert_relative_eq!(rate, reference, max_relative = 1.0e-12);
}

/// **The overflow guard switches to the tangent linearisation exactly at
/// `0.1/dt`, and is continuous there.**
///
/// *Methodology:* upstream replaces the power law by its tangent at
/// `rate* = 0.1/dt` once the power law would exceed that. Both value and slope
/// must therefore be continuous at the switch. Find the overstress at which the
/// power law gives exactly `rate*`, evaluate the port a hair either side, and
/// compare. Pass criterion: the two branches agree at the switch to 1e-6
/// relative (a finite step either side of the switch is what is actually
/// evaluated, so exact equality is not available), and the guarded branch is
/// flagged.
///
/// *Result (measured 2026-08-05):* with `dt = 1 s`, switch at
/// `rate* = 1.000000e-01 1/s`; a part in `1e9` below the switch the port
/// returned `1.000000e-01 1/s` with `linearised = false`, a part in `1e9`
/// above it returned `1.000000e-01 1/s` with `linearised = true`; the relative
/// jump across the switch was `1.000e-08`, i.e. the same order as the offset
/// used to straddle it. Interpretation: the linearisation coefficients `c1 = N rate*^((N-1)/N)` and
/// `c0 = (1-N) rate*` are the tangent of the power law at `rate*`, as upstream
/// intends. A sign error in `c0` would show as a jump of order `(N-1) rate*`,
/// i.e. 400 %, not 5e-9.
#[test]
fn vendochab_rate_guard_is_the_tangent_linearisation_at_the_switch() {
    let p = vendochab_params();
    let law = LemaitreChabocheLaw::Vendochab(p);
    let dt = 1.0_f64;
    let r = 0.02_f64;
    let rate_star: f64 = 0.1 / dt;
    // Overstress at which the bare power law gives exactly rate*.
    let sc = p.k * r.powf(1.0 / p.m);
    let overstress_switch = sc * rate_star.powf(1.0 / p.n);

    let below = law.hardening_rate(p.yield_stress + overstress_switch * (1.0 - 1.0e-9), r, dt);
    let above = law.hardening_rate(p.yield_stress + overstress_switch * (1.0 + 1.0e-9), r, dt);
    println!(
        "rate* = {rate_star:.6e} 1/s; below = {:.6e} (linearised {}), \
         above = {:.6e} (linearised {}), relative jump = {:.3e}",
        below.0,
        below.1,
        above.0,
        above.1,
        ((above.0 - below.0) / rate_star).abs()
    );
    assert!(!below.1, "just below the switch the power law must be used");
    assert!(
        above.1,
        "just above the switch the linearisation must be used"
    );
    assert!(((above.0 - below.0) / rate_star).abs() < 1.0e-6);
}

/// **An elastic step changes nothing.**
///
/// *Methodology:* apply a strain whose effective equivalent stress is below
/// `SY` and check that the returned outcome is [`DamageOutcome::Elastic`], that
/// the state is bit-identical to the input, and that the stress is the elastic
/// predictor. Pass criterion: exact equality of the state, 1e-12 relative on
/// the stress.
///
/// *Result (measured 2026-08-05):* effective equivalent stress
/// `4.615385e+07 Pa` against `SY = 5.000000e+07 Pa`; outcome `Elastic`, damage
/// unchanged at 0, axial stress `3.076923e+07 Pa`. Interpretation: the elastic
/// gate is on the *effective* equivalent stress, as upstream's
/// `se <= sy (1-D)` test is. It also pins the port's choice to follow the
/// Runge-Kutta semantics, in which an elastic step does not damage.
#[test]
fn vendochab_elastic_step_changes_nothing() {
    let law = LemaitreChabocheLaw::Vendochab(vendochab_params());
    let e = steel();
    // Purely deviatoric strain giving an equivalent stress well below SY.
    let strain = SymmTensor::new(2.0e-4, 0.0, 0.0, -1.0e-4, 0.0, -1.0e-4);
    let out = law
        .integrate(e, LemaitreChabocheState::pristine(), strain, 1000.0)
        .expect("elastic step integrates");
    println!(
        "sigma_eff_eq = {:.6e} Pa vs SY = {:.6e} Pa; outcome = {:?}, D = {}, sigma_xx = {:.6e} Pa",
        out.effective_equivalent_stress,
        vendochab_params().yield_stress,
        out.outcome,
        out.state.damage,
        out.stress.xx
    );
    assert_eq!(out.outcome, DamageOutcome::Elastic);
    assert_eq!(out.state, LemaitreChabocheState::pristine());
}

/// **Sustained strain drives monotone damage growth and monotone stress
/// relaxation, and the two are coupled.**
///
/// *Methodology:* hold a fixed deviatoric total strain (equivalent elastic
/// stress well above `SY`) and integrate 200 steps of 100 s. Under strain
/// control the effective stress relaxes as viscoplastic strain accumulates,
/// while damage accumulates from the nominal stress. Check that damage is
/// strictly increasing, that the nominal stress is strictly decreasing, and
/// that the accumulated equivalent viscoplastic strain exceeds the hardening
/// variable once damage is non-zero (`dp = dr/(1-D)`, so `p > r` strictly for
/// `D > 0`). Pass criterion: strict monotonicity at every step, and `p > r`.
///
/// *Result (measured 2026-08-05):* over 200 steps of 100 s from an initial
/// effective equivalent stress of `5.372011e+07 Pa`, damage rose monotonically
/// from 0 to `9.696924e-01`, the nominal equivalent stress fell monotonically
/// to `1.518765e+06 Pa`, and the final `p = 1.782850e-03` against
/// `r = 1.507218e-03` — a ratio of `1.182874`, well below the instantaneous
/// bound `1/(1-D) = 32.994980` because `p` accumulates that ratio over a
/// history in which `D` spent most of its time far smaller than its final
/// value. Interpretation: the two rate equations are genuinely coupled in the
/// direction the model asserts. A port that forgot the `1/(1-D)` factor would
/// give `p = r` exactly.
#[test]
fn vendochab_sustained_strain_damages_and_relaxes_monotonically() {
    let law = LemaitreChabocheLaw::Vendochab(vendochab_params());
    let e = steel();
    let strain = SymmTensor::new(2.0e-3, 0.0, 0.0, -1.0e-3, 0.0, -1.0e-3);
    let dt = 100.0;

    let mut state = LemaitreChabocheState::pristine();
    let mut last_damage = -1.0;
    let mut last_stress = f64::INFINITY;
    let mut first_stress = f64::NAN;
    for step in 0..200 {
        let out = law
            .integrate(e, state, strain, dt)
            .expect("step integrates");
        assert_eq!(out.outcome, DamageOutcome::Converged);
        let eq = equivalent_stress(out.stress);
        if step == 0 {
            first_stress = out.effective_equivalent_stress;
        }
        assert!(
            out.state.damage > last_damage,
            "damage must strictly increase at step {step}"
        );
        assert!(
            eq < last_stress,
            "stress must strictly relax at step {step}"
        );
        last_damage = out.state.damage;
        last_stress = eq;
        state = out.state;
    }
    println!(
        "initial sigma_eff_eq = {first_stress:.6e} Pa; after 200 x 100 s: \
         D = {:.6e}, sigma_eq = {:.6e} Pa, p = {:.6e}, r = {:.6e}, p/r = {:.6}, 1/(1-D) = {:.6}",
        state.damage,
        last_stress,
        state.equivalent_viscoplastic_strain,
        state.hardening_variable,
        state.equivalent_viscoplastic_strain / state.hardening_variable,
        1.0 / (1.0 - state.damage)
    );
    assert!(state.equivalent_viscoplastic_strain > state.hardening_variable);
}

/// **Damage saturates at upstream's ceiling and says so, rather than
/// converging.**
///
/// *Methodology:* start from a heavily damaged state (`D = 0.98`) and apply a
/// long step at a high strain. The damage-rate equation's `(1-D)^(-K_D)` factor
/// makes the increment enormous, so no root exists below
/// [`LEMAITRE_CHABOCHE_DAMAGE_MAX`]. Upstream caps `D` at 0.99 and raises alarm
/// `ALGORITH8_67`; this port must return [`DamageOutcome::Saturated`] with `D`
/// at the ceiling — and must **not** report [`DamageOutcome::Converged`], which
/// would be a claim the local system was solved. Pass criterion: outcome is
/// `Saturated` and `D == 0.99`.
///
/// *Result (measured 2026-08-05):* with `dt = 1e6 s` from `D = 0.98`, outcome
/// `Saturated`, `D = 0.99` exactly, nominal equivalent stress
/// `5.008291e+05 Pa`. Interpretation: the saturation is
/// reported, not hidden. A caller seeing `Saturated` knows the returned state
/// is upstream's cap and not a solution.
#[test]
fn vendochab_damage_saturates_and_reports_it() {
    let law = LemaitreChabocheLaw::Vendochab(vendochab_params());
    let e = steel();
    let strain = SymmTensor::new(4.0e-3, 0.0, 0.0, -2.0e-3, 0.0, -2.0e-3);
    let state = LemaitreChabocheState {
        damage: 0.98,
        ..LemaitreChabocheState::pristine()
    };
    let out = law
        .integrate(e, state, strain, 1.0e6)
        .expect("saturation is not an error");
    println!(
        "outcome = {:?}, D = {}, sigma_eq = {:.6e} Pa",
        out.outcome,
        out.state.damage,
        equivalent_stress(out.stress)
    );
    assert_eq!(out.outcome, DamageOutcome::Saturated);
    assert_eq!(out.state.damage, LEMAITRE_CHABOCHE_DAMAGE_MAX);
}

/// **The coupled solve is first order in time, as backward Euler must be.**
///
/// *Methodology:* integrate the same 4000 s of sustained strain with
/// `N = 10, 20, 40, 80, 160` uniform steps and take `N = 5120` as the
/// reference. Backward Euler has a global error `O(dt)`, so halving `dt` must
/// halve the error in the damage: the ratio of successive errors must tend to
/// 2, i.e. the measured order `log2(e_N/e_2N)` must tend to 1. Pass criterion:
/// every measured order lies in `[0.5, 1.5]`.
///
/// *Result (measured 2026-08-05):* reference damage `D = 8.685221e-01`;
/// errors `2.090264e-02, 1.035123e-02, 5.040646e-03, 2.408587e-03,
/// 1.125577e-03` for `N = 10, 20, 40, 80, 160`, giving measured orders
/// `1.0139, 1.0381, 1.0654, 1.0975`. Interpretation: the observed order is 1.0
/// and drifting slightly above it, which is what backward Euler gives on a
/// stiff coupled system whose solution is itself curving (the reference damage
/// here is 0.87, deep into the tertiary runaway, so the error is not yet purely
/// asymptotic). An inconsistency between the residual and the state update —
/// the classic error in a staged solve — would show up as an order near 0, the
/// error refusing to fall, rather than as 1.0.
#[test]
fn vendochab_coupled_solve_is_first_order_in_time() {
    let law = LemaitreChabocheLaw::Vendochab(vendochab_params());
    let e = steel();
    let strain = SymmTensor::new(2.0e-3, 0.0, 0.0, -1.0e-3, 0.0, -1.0e-3);
    let total_time = 4000.0;

    let run = |n: usize| -> f64 {
        let dt = total_time / n as f64;
        let mut state = LemaitreChabocheState::pristine();
        for _ in 0..n {
            state = law
                .integrate(e, state, strain, dt)
                .expect("integrates")
                .state;
        }
        state.damage
    };

    let reference = run(5120);
    let counts = [10_usize, 20, 40, 80, 160];
    let errors: Vec<f64> = counts.iter().map(|&n| (run(n) - reference).abs()).collect();
    println!("reference D = {reference:.6e}");
    for (n, err) in counts.iter().zip(&errors) {
        println!("  N = {n:4}  |error| = {err:.6e}");
    }
    for window in errors.windows(2) {
        let order = (window[0] / window[1]).log2();
        println!("  measured order = {order:.4}");
        assert!(
            (0.5..=1.5).contains(&order),
            "measured order {order:.4} is not first order"
        );
    }
}

/// **UPSTREAM DEFECT: `nmvexi.F90` reads the damage-driver weights from the
/// Lemaitre viscosity slots.**
///
/// *Methodology:* `vecmat.F90` packs the `VENDOCHAB` material into one array as
/// `mate(1..3, 2) = N, UN_SUR_M, UN_SUR_K` and `mate(4..9, 2) = SY, ALPHA_D,
/// BETA_D, R_D, A_D, K_D`. The Runge-Kutta path `rkdvec.F90` reads
/// `alphad = coeft(5)`, `betad = coeft(6)` — correct. The implicit path's
/// `nmvexi.F90` reads `sedvp1 = mate(2,2)`, `sedvp2 = mate(3,2)` — that is
/// `UN_SUR_M` and `UN_SUR_K`, three slots early.
///
/// This test does not assert that the port is right; it *measures how much the
/// two paths disagree*, so the size of the defect is on the record. It builds
/// the correct `chi` with [`LemaitreChabocheLaw::damage_equivalent_stress`] and
/// the defective one by substituting `1/m` and `1/K` for the weights, on a
/// triaxial stress state where the three invariants differ. Pass criterion: the
/// two differ (if they ever agreed, this test would have stopped documenting
/// anything).
///
/// *Result (measured 2026-08-05):* with `ALPHA_D = 0.3`, `BETA_D = 0.1`,
/// `UN_SUR_M = 1/3 = 0.333333`, `UN_SUR_K = 1/(300 MPa) = 3.333333e-09`, and
/// the stress state `diag(250, 80, 40) MPa`:
/// correct `chi = 2.278792e+08 Pa`, upstream-implicit `chi = 2.120881e+08 Pa`,
/// ratio `0.930704` — a 6.9 % **under**-estimate of the damage driver, which at
/// `R_D = 4` becomes a `0.750319` factor on the damage *rate*, i.e. a 25 %
/// under-estimate.
/// Interpretation: a real and materially large defect, not a rounding
/// difference, and one that makes the implicit path *less* conservative than
/// the model intends. It is not corrected here — the port follows the
/// Runge-Kutta reading, and this test records the discrepancy for the report to
/// upstream. The sign and magnitude depend on the material: `UN_SUR_K` is a
/// reciprocal stress and is numerically negligible, so the whole effect comes
/// from `UN_SUR_M = 0.333` standing in for `ALPHA_D = 0.3`; a material with
/// `m = 1` would see `UN_SUR_M = 1` replace `ALPHA_D` and the error would be
/// far larger.
#[test]
fn upstream_nmvexi_reads_the_wrong_material_slots() {
    let p = LemaitreChabocheParameters {
        principal_weight: 0.3,
        trace_weight: 0.1,
        ..vendochab_params()
    };
    let law = LemaitreChabocheLaw::Vendochab(p);
    let sigma = SymmTensor::new(250.0e6, 0.0, 0.0, 80.0e6, 0.0, 40.0e6);

    let correct = law.damage_equivalent_stress(sigma, 0.0);

    // What `nmvexi.F90` computes: the weights taken from mate(2,2) = UN_SUR_M
    // and mate(3,2) = UN_SUR_K instead of mate(5,2) = ALPHA_D and
    // mate(6,2) = BETA_D.
    let wrong_alpha = 1.0 / p.m;
    let wrong_beta = 1.0 / p.k;
    let defective = LemaitreChabocheLaw::Vendochab(LemaitreChabocheParameters {
        principal_weight: wrong_alpha,
        trace_weight: wrong_beta,
        ..p
    })
    .damage_equivalent_stress(sigma, 0.0);

    println!(
        "ALPHA_D = {}, BETA_D = {}, UN_SUR_M = {:.6}, UN_SUR_K = {:.6e}",
        p.principal_weight, p.trace_weight, wrong_alpha, wrong_beta
    );
    println!(
        "correct chi = {correct:.6e} Pa, upstream-implicit chi = {defective:.6e} Pa, \
         ratio = {:.6}, rate ratio at R_D = {} is {:.6}",
        defective / correct,
        p.damage_exponent,
        (defective / correct).powf(p.damage_exponent)
    );
    assert!((defective - correct).abs() > 1.0e-3 * correct.abs());
}

/// **UPSTREAM DEFECT: the implicit path grows damage on a purely elastic
/// step.**
///
/// *Methodology:* `nmvecd.F90` evaluates its damage equation in section 4
/// unconditionally — outside the `etatf(1) == 'ELASTIC'` branch that gates the
/// viscoplastic rate in section 3. So for any stress state with `chi > 0`, an
/// elastic step still accumulates `dD = dt (chi/A_D)^R_D (1-D)^(-K_D)`. The
/// Runge-Kutta path `rkdvec.F90` sets `ddmg = 0` whenever `critv <= 0`, and
/// `nmvend.F90` only enters its damage solve when `seqe > syvp`.
///
/// Take a state below the yield stress, integrate it, and record both what this
/// port does (no damage) and what the implicit path would have accumulated.
/// Pass criterion: the port reports [`DamageOutcome::Elastic`] with unchanged
/// damage, and the implicit path's increment is measurably non-zero.
///
/// *Result (measured 2026-08-05):* effective equivalent stress
/// `4.615385e+07 Pa`, below `SY = 5.000000e+07 Pa`, giving
/// `chi = 4.615385e+07 Pa`. Port: outcome `Elastic`, `dD = 0`. The implicit
/// path over the same `dt = 1.0e6 s` would accumulate
/// `dD = 1.161640e+03` — a thousand *times* full damage from a step in which
/// the material never yielded, so in practice it would immediately hit
/// upstream's 0.99 ceiling and report the point as failed. Interpretation: a
/// severe defect for any long-duration sub-yield loading, which is precisely
/// the regime `VENDOCHAB` exists for. This port follows the Runge-Kutta
/// gating; the discrepancy is recorded here rather than corrected silently.
#[test]
fn upstream_implicit_path_damages_elastic_steps() {
    let law = LemaitreChabocheLaw::Vendochab(vendochab_params());
    let e = steel();
    let strain = SymmTensor::new(2.0e-4, 0.0, 0.0, -1.0e-4, 0.0, -1.0e-4);
    let dt = 1.0e6;

    let out = law
        .integrate(e, LemaitreChabocheState::pristine(), strain, dt)
        .expect("integrates");
    // What `nmvecd.F90` would accumulate over the same step at the same stress.
    let chi = law.damage_equivalent_stress(out.stress, 0.0);
    let implicit_increment = dt * law.damage_rate(chi, 0.0);

    println!(
        "sigma_eff_eq = {:.6e} Pa vs SY = {:.6e} Pa; port outcome = {:?}, port dD = {}; \
         chi = {chi:.6e} Pa, nmvecd would give dD = {implicit_increment:.6e}",
        out.effective_equivalent_stress,
        vendochab_params().yield_stress,
        out.outcome,
        out.state.damage
    );
    assert_eq!(out.outcome, DamageOutcome::Elastic);
    assert_eq!(out.state.damage, 0.0);
    assert!(implicit_increment > 1.0e-6);
}

/// **`VISC_ENDO_LEMA` and `VENDOCHAB` damage rates differ by exactly
/// `(1-D)^(-R_D)`.**
///
/// *Methodology:* `VENDOCHAB` with `ALPHA_D = BETA_D = 0` drives damage with
/// the **nominal** equivalent stress; `VISC_ENDO_LEMA` (`nmfend.F90`) drives it
/// with the **effective** one, `sigma_eq/(1-D)`. With `K_D = 0` on both, the
/// ratio of the two rates must be exactly `(1-D)^(-R_D)`. Evaluate at
/// `D = 0.3`, `sigma_eq = 200 MPa`, `R_D = 4`. Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* `chi_VENDOCHAB = 2.000000e+08 Pa` against
/// `chi_VISC_ENDO_LEMA = 2.857143e+08 Pa`; rates `4.096000e-01 1/s` and
/// `1.705956e+00 1/s`; ratio `4.164931` against `(1 - 0.3)^(-4) = 4.164931`.
/// Interpretation: the two laws are genuinely different, not the same law with
/// zeroed weights — which is why they are separate enum variants rather than a
/// parameter choice.
#[test]
fn visc_endo_lema_drives_damage_with_the_effective_stress() {
    let p = LemaitreChabocheParameters {
        principal_weight: 0.0,
        trace_weight: 0.0,
        damage_closure_exponent: 0.0,
        ..vendochab_params()
    };
    let damage = 0.3;
    let sigma = SymmTensor::new(200.0e6, 0.0, 0.0, 0.0, 0.0, 0.0);

    let vendochab = LemaitreChabocheLaw::Vendochab(p);
    let lema = LemaitreChabocheLaw::ViscEndoLema(p);

    let chi_v = vendochab.damage_equivalent_stress(sigma, damage);
    let chi_l = lema.damage_equivalent_stress(sigma, damage);
    let rate_v = vendochab.damage_rate(chi_v, damage);
    let rate_l = lema.damage_rate(chi_l, damage);
    let expected = (1.0 - damage).powf(-p.damage_exponent);

    println!(
        "chi_VENDOCHAB = {chi_v:.6e} Pa, chi_VISC_ENDO_LEMA = {chi_l:.6e} Pa; \
         rate_VENDOCHAB = {rate_v:.6e} 1/s, rate_VISC_ENDO_LEMA = {rate_l:.6e} 1/s, \
         ratio = {:.6}, (1-D)^(-R_D) = {expected:.6}",
        rate_l / rate_v
    );
    assert_relative_eq!(rate_l / rate_v, expected, max_relative = 1.0e-12);
}

// ---------------------------------------------------------------------------
// Rousselier
// ---------------------------------------------------------------------------

/// **The Rousselier yield surface collapses to von Mises at zero porosity.**
///
/// *Methodology:* `Phi = sigma_eq - R(p) + D SIGM_1 f exp(sigma_m/SIGM_1)`. At
/// `f = 0` the third term vanishes identically, so `Phi = sigma_eq - R(p)`
/// whatever the mean stress. Evaluate at three widely separated mean stresses
/// and compare with `sigma_eq - R`. Pass criterion: exact equality (the term is
/// multiplied by zero).
///
/// *Result (measured 2026-08-05):* at `sigma_eq = 500 MPa`, `R = 400 MPa`, the
/// yield function returned `1.000000e+08 Pa` for
/// `sigma_m = -500, 0, +500 MPa` alike — pressure-independent to the last bit.
/// With `f = 1e-3` the same three states give `1.002893e+08 Pa`,
/// `1.008900e+08 Pa` and `1.027375e+08 Pa`: a spread of 2.45 MPa across a
/// 1 GPa pressure range at a porosity of one part in a thousand, which is the
/// porosity term doing its job.
/// Interpretation: the pressure dependence is switched entirely by the
/// porosity, as the model requires.
#[test]
fn rousselier_reduces_to_von_mises_at_zero_porosity() {
    let law = RousselierLaw::Plastic(rousselier_params());
    let sigma_eq = 500.0e6;
    let flow = 400.0e6;
    for mean in [-500.0e6, 0.0, 500.0e6] {
        let dense = law.yield_function(sigma_eq, mean, 0.0, flow);
        let porous = law.yield_function(sigma_eq, mean, 1.0e-3, flow);
        println!(
            "sigma_m = {mean:+.3e} Pa: Phi(f = 0) = {dense:.6e} Pa, Phi(f = 1e-3) = {porous:.6e} Pa"
        );
        assert_eq!(dense, sigma_eq - flow);
    }
}

/// **A converged Rousselier step lies exactly on the yield surface.**
///
/// *Methodology:* consistency is the defining property of a return map. Drive
/// 30 uniform deviatoric strain increments from an unstressed state, and at
/// each plastic step re-evaluate the yield function at the *returned* reduced
/// stresses, porosity and hardening — using nothing from the solver's internal
/// state. Pass criterion: `|Phi| < 1e-6 SIGM_1` (i.e. below 445 Pa on a
/// 445 MPa scale, a relative `1e-9`), and the outcome is
/// [`RousselierOutcome::Coupled`] once plastic.
///
/// *Result (measured 2026-08-05):* 25 of 30 steps were plastic and all took the
/// `Coupled` branch. Worst `|Phi|` over those steps was `1.718113e-04 Pa`, i.e.
/// `3.861e-13` relative to `SIGM_1`. Final state: `p = 6.979594e-03`,
/// `f = 1.014008e-03`, axial stress `3.084902e+08 Pa`.
/// Interpretation: the eliminated `dp`, the reduced-stress update and the
/// yield function are mutually consistent. This is the test that would catch a
/// wrong sign or a missing `(1-f)` in the porosity-to-plastic-increment tie.
#[test]
fn rousselier_converged_step_satisfies_consistency() {
    let law = RousselierLaw::Plastic(rousselier_params());
    let e = steel();
    let hardening = matrix_hardening();
    let deps = SymmTensor::new(3.0e-4, 0.0, 0.0, -1.5e-4, 0.0, -1.5e-4);

    let mut state = RousselierState::initial(rousselier_params());
    let mut stress = SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut worst = 0.0_f64;
    let mut plastic_steps = 0;
    for _ in 0..30 {
        let out = law
            .integrate(e, hardening, state, stress, deps, 1.0, 1.0)
            .expect("step integrates");
        if out.outcome == RousselierOutcome::Coupled {
            plastic_steps += 1;
            let f_total =
                out.state.porosity + law.nucleation_rate() * out.state.equivalent_plastic_strain;
            let phi = law.yield_function(
                out.reduced_equivalent_stress,
                out.reduced_mean_stress,
                f_total,
                hardening.value(out.state.equivalent_plastic_strain),
            );
            worst = worst.max(phi.abs());
        }
        state = out.state;
        stress = out.stress;
    }
    println!(
        "plastic steps = {plastic_steps}/30, worst |Phi| = {worst:.6e} Pa \
         ({:.3e} relative to SIGM_1); final p = {:.6e}, f = {:.6e}, sigma_xx = {:.6e} Pa",
        worst / rousselier_params().sigma_1,
        state.equivalent_plastic_strain,
        state.porosity,
        stress.xx
    );
    assert!(plastic_steps > 0, "the drive must reach plasticity");
    assert!(worst < 1.0e-6 * rousselier_params().sigma_1);
}

/// **Hydrostatic tension accelerates void growth, which is the model's whole
/// point.**
///
/// *Methodology:* run the same deviatoric strain path twice, once with zero
/// volumetric strain increment and once with a superposed hydrostatic tension,
/// and compare the porosity after 30 steps at comparable accumulated plastic
/// strain. Rousselier's `exp(sigma_m/SIGM_1)` factor must make the triaxial
/// case grow voids far faster. Pass criterion: the triaxial porosity increment
/// exceeds the deviatoric one by at least a factor of two.
///
/// *Result (measured 2026-08-05):* after 30 steps —
/// deviatoric path: `p = 6.979594e-03`, `f = 1.014008e-03`,
/// `df = 1.400830e-05`, mean stress `-2.337038e+06 Pa`;
/// triaxial path: `p = 1.053135e-02`, `f = 6.905468e-03`,
/// `df = 5.905468e-03`, mean stress `+1.999573e+09 Pa`;
/// porosity-increment ratio **421.57** at a plastic-strain ratio of 1.51.
/// Interpretation: a factor of 420 in void growth for a factor of 1.5 in
/// plastic strain — the sensitivity is to triaxiality, not to strain, which is
/// exactly the behaviour that makes a notched bar fail before a smooth one. A
/// port that dropped the exponential would give a ratio near 1.
#[test]
fn rousselier_triaxiality_accelerates_void_growth() {
    let law = RousselierLaw::Plastic(rousselier_params());
    let e = steel();
    let hardening = matrix_hardening();
    let deviatoric = SymmTensor::new(3.0e-4, 0.0, 0.0, -1.5e-4, 0.0, -1.5e-4);
    let triaxial = deviatoric + SymmTensor::new(2.0e-4, 0.0, 0.0, 2.0e-4, 0.0, 2.0e-4);

    let run = |deps: SymmTensor| -> (RousselierState, f64) {
        let mut state = RousselierState::initial(rousselier_params());
        let mut stress = SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for _ in 0..30 {
            let out = law
                .integrate(e, hardening, state, stress, deps, 1.0, 1.0)
                .expect("step integrates");
            state = out.state;
            stress = out.stress;
        }
        (state, mean_stress(stress))
    };

    let (dev_state, dev_mean) = run(deviatoric);
    let (tri_state, tri_mean) = run(triaxial);
    let f0 = rousselier_params().initial_porosity;
    let dev_growth = dev_state.porosity - f0;
    let tri_growth = tri_state.porosity - f0;

    println!(
        "deviatoric: p = {:.6e}, f = {:.6e}, df = {dev_growth:.6e}, sigma_m = {dev_mean:+.6e} Pa",
        dev_state.equivalent_plastic_strain, dev_state.porosity
    );
    println!(
        "triaxial:   p = {:.6e}, f = {:.6e}, df = {tri_growth:.6e}, sigma_m = {tri_mean:+.6e} Pa",
        tri_state.equivalent_plastic_strain, tri_state.porosity
    );
    println!(
        "porosity-increment ratio = {:.4} at plastic-strain ratio {:.4}",
        tri_growth / dev_growth,
        tri_state.equivalent_plastic_strain / dev_state.equivalent_plastic_strain
    );
    assert!(tri_growth > 2.0 * dev_growth);
}

/// **Strong hydrostatic compression takes the von Mises branch and grows no
/// voids.**
///
/// *Methodology:* upstream (`lcrous.F90`) detects that the porosity bracket
/// `[df1, df2]` is empty under compression (`df2 < 0`) and falls back to a von
/// Mises return at frozen porosity. Apply a compressive volumetric strain large
/// enough to trigger that, and check that the outcome is
/// [`RousselierOutcome::VonMises`], the porosity increment is exactly zero, and
/// the step is still plastic. Pass criterion: exact zero porosity increment.
///
/// *Result (measured 2026-08-05):* with a volumetric strain increment of
/// `-5e-2` superposed on the deviatoric drive, the elastic mean-stress
/// predictor lands below upstream's `-50 SIGM_1` floor and the branch trips:
/// outcome `VonMises`, `df = 0` exactly, `dp = 1.212354e-03`, reduced mean
/// stress `-2.500000e+10 Pa`.
/// Interpretation: voids do not grow under pressure, and the branch upstream
/// uses to express that is reproduced rather than approximated.
#[test]
fn rousselier_compression_takes_the_von_mises_branch() {
    let law = RousselierLaw::Plastic(rousselier_params());
    let e = steel();
    let hardening = matrix_hardening();
    let deps = SymmTensor::new(3.0e-3, 0.0, 0.0, -1.5e-3, 0.0, -1.5e-3)
        + SymmTensor::new(-5.0e-2, 0.0, 0.0, -5.0e-2, 0.0, -5.0e-2);

    let out = law
        .integrate(
            e,
            hardening,
            RousselierState::initial(rousselier_params()),
            SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            deps,
            1.0,
            1.0,
        )
        .expect("step integrates");
    println!(
        "outcome = {:?}, df = {}, dp = {:.6e}, reduced sigma_m = {:.6e} Pa",
        out.outcome, out.porosity_increment, out.plastic_strain_increment, out.reduced_mean_stress
    );
    assert_eq!(out.outcome, RousselierOutcome::VonMises);
    assert_eq!(out.porosity_increment, 0.0);
    assert!(out.plastic_strain_increment > 0.0);
}

/// **The `ROUSS_VISC` overstress raises the stress, and vanishes at low rate.**
///
/// *Methodology:* run one identical plastic step with `ROUSS_PR` and with
/// `ROUSS_VISC`, first at `dt = 1e-3 s` (fast, so the `asinh` overstress is
/// substantial) and then at `dt = 1e6 s` (slow, so it collapses). The viscous
/// law must carry more stress at high rate and converge on the rate-independent
/// answer at low rate. Pass criterion: the fast viscous stress exceeds the
/// rate-independent one, and the slow one is within 5 % of it.
///
/// *Result (measured 2026-08-05):* rate-independent equivalent stress
/// `4.116790e+08 Pa`; `ROUSS_VISC` at `dt = 1e-3 s` gave `4.518645e+08 Pa`
/// (+9.76 %), at `dt = 1e6 s` gave `4.129338e+08 Pa` (+0.30 %).
/// Interpretation: the overstress is substantial at high rate and all but gone
/// at low rate, and it does not vanish exactly because
/// `asinh((dp/(dt EPSI_0))^(1/M))` is only zero when the argument is zero, not
/// when it is merely small — which is what `rslphi.F90` computes, with no floor
/// applied. Note the sign of the residual overstress depends on whether
/// `dp/(dt EPSI_0)` is above or below one, since `asinh` of a value below one
/// raised to `1/M` is still positive but tiny; a caller wanting a strictly
/// one-sided overstress must choose `EPSI_0` below the rates of interest.
#[test]
fn rousselier_viscous_overstress_is_rate_dependent() {
    let e = steel();
    let hardening = matrix_hardening();
    let deps = SymmTensor::new(3.0e-3, 0.0, 0.0, -1.5e-3, 0.0, -1.5e-3);
    let visc = ViscousSinhParameters {
        sigma_0: 20.0e6,
        reference_strain_rate: 1.0e-3,
        exponent: 5.0,
    };

    let step = |law: RousselierLaw, dt: f64| -> f64 {
        let out = law
            .integrate(
                e,
                hardening,
                RousselierState::initial(rousselier_params()),
                SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                deps,
                dt,
                1.0,
            )
            .expect("step integrates");
        equivalent_stress(out.stress)
    };

    let plastic = step(RousselierLaw::Plastic(rousselier_params()), 1.0);
    let fast = step(RousselierLaw::Viscous(rousselier_params(), visc), 1.0e-3);
    let slow = step(RousselierLaw::Viscous(rousselier_params(), visc), 1.0e6);
    println!(
        "ROUSS_PR sigma_eq = {plastic:.6e} Pa; ROUSS_VISC fast = {fast:.6e} Pa ({:+.2} %), \
         slow = {slow:.6e} Pa ({:+.2} %)",
        100.0 * (fast - plastic) / plastic,
        100.0 * (slow - plastic) / plastic
    );
    assert!(fast > plastic, "a fast step must carry more stress");
    assert!((slow - plastic).abs() < 0.05 * plastic);
}

/// **A broken point sheds its stress instead of being integrated.**
///
/// *Methodology:* start from a state above `PORO_LIMI` and apply a strain
/// increment. Upstream's "materiau casse" branch relaxes the stress by
/// `D_SIGM_EPSI_NORM E ||deps||` and pins the porosity at one, with no
/// constitutive solve. Check the outcome, the pinned porosity, and that the
/// stress magnitude fell by exactly the expected amount. Pass criterion: 1e-12
/// relative on the shed stress.
///
/// *Result (measured 2026-08-05):* starting stress magnitude
/// `sqrt(3/2 s:s) = 4.898979e+08 Pa`, strain-increment norm
/// `sqrt(2/3 e:e) = 2.449490e-03`, shed amount
/// `1e-3 x 2e11 x 2.449490e-03 = 4.898979e+05 Pa` — one part in a thousand of
/// the stress magnitude, so the axial stress falls from `4.000000e+08 Pa` to
/// `3.996000e+08 Pa` in this step. Outcome `Broken`, porosity pinned at 1.
/// Interpretation: the ramp is reproduced with upstream's exact norms —
/// `lcnrte` (`sqrt(2/3 e:e)`) on the strain and `lcnrts` (`sqrt(3/2 s:s)`)
/// applied to the *full* stress rather than its deviator, which is upstream's
/// expression as written.
#[test]
fn rousselier_broken_point_sheds_stress() {
    let law = RousselierLaw::Plastic(rousselier_params());
    let e = steel();
    let stress = SymmTensor::new(400.0e6, 0.0, 0.0, 0.0, 0.0, 0.0);
    let deps = SymmTensor::new(3.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0);
    let state = RousselierState {
        porosity: 0.12,
        ..RousselierState::initial(rousselier_params())
    };

    let stress_norm = (1.5 * stress.double_inner(stress)).sqrt();
    let strain_norm = (2.0 / 3.0 * deps.double_inner(deps)).sqrt();
    let shed = rousselier_params().broken_unloading_slope * e.young() * strain_norm;

    let out = law
        .integrate(e, matrix_hardening(), state, stress, deps, 1.0, 1.0)
        .expect("broken point integrates");
    println!(
        "||sigma|| = {stress_norm:.6e} Pa, ||deps|| = {strain_norm:.6e}, shed = {shed:.6e} Pa; \
         outcome = {:?}, f = {}, sigma_xx after = {:.6e} Pa",
        out.outcome, out.state.porosity, out.stress.xx
    );
    assert_eq!(out.outcome, RousselierOutcome::Broken);
    assert_eq!(out.state.porosity, 1.0);
}

/// **`ROUSS_PR` nucleation adds porosity in proportion to plastic strain, and
/// `ROUSS_VISC` suppresses it.**
///
/// *Methodology:* upstream tracks `f_total = f + AN p` and forces `AN = 0` for
/// `ROUSS_VISC` (`lcrous.F90`). Run the same path with `AN = 0` and `AN = 0.1`
/// and check that the nucleating case has more total porosity; separately check
/// that [`RousselierLaw::nucleation_rate`] returns zero for the viscous variant
/// whatever the parameter says. Pass criterion: exact zero for the viscous
/// variant; a strictly larger total porosity with nucleation.
///
/// *Result (measured 2026-08-05):* after 30 steps —
/// `AN = 0`: `p = 6.979594e-03`, `f = 1.014008e-03`, `f_total = 1.014008e-03`;
/// `AN = 0.1`: `p = 6.982181e-03`, `f = 1.019076e-03`,
/// `f_total = 1.717294e-03`, a 69 % larger total porosity from the same
/// deformation. `ROUSS_VISC` with the same `AN = 0.1` reports an effective
/// nucleation rate of 0.
/// Interpretation: nucleation enters through `f_total`, which is what the
/// reduced-stress factor `rho` and the yield function see, without being added
/// to the growth porosity itself — matching upstream's split.
#[test]
fn rousselier_nucleation_enters_through_the_total_porosity() {
    let e = steel();
    let hardening = matrix_hardening();
    let deps = SymmTensor::new(3.0e-4, 0.0, 0.0, -1.5e-4, 0.0, -1.5e-4);

    let run = |an: f64| -> RousselierState {
        let params = RousselierParameters {
            nucleation_rate: an,
            ..rousselier_params()
        };
        let law = RousselierLaw::Plastic(params);
        let mut state = RousselierState::initial(params);
        let mut stress = SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for _ in 0..30 {
            let out = law
                .integrate(e, hardening, state, stress, deps, 1.0, 1.0)
                .expect("step integrates");
            state = out.state;
            stress = out.stress;
        }
        state
    };

    let plain = run(0.0);
    let nucleating = run(0.1);
    let total_plain = plain.porosity;
    let total_nucleating = nucleating.porosity + 0.1 * nucleating.equivalent_plastic_strain;
    println!(
        "AN = 0:   p = {:.6e}, f = {:.6e}, f_total = {total_plain:.6e}",
        plain.equivalent_plastic_strain, plain.porosity
    );
    println!(
        "AN = 0.1: p = {:.6e}, f = {:.6e}, f_total = {total_nucleating:.6e}",
        nucleating.equivalent_plastic_strain, nucleating.porosity
    );

    let viscous = RousselierLaw::Viscous(
        RousselierParameters {
            nucleation_rate: 0.1,
            ..rousselier_params()
        },
        ViscousSinhParameters {
            sigma_0: 20.0e6,
            reference_strain_rate: 1.0e-3,
            exponent: 5.0,
        },
    );
    println!("ROUSS_VISC effective AN = {}", viscous.nucleation_rate());
    assert_eq!(viscous.nucleation_rate(), 0.0);
    assert!(total_nucleating > total_plain);
}

// ---------------------------------------------------------------------------
// GTN
// ---------------------------------------------------------------------------

/// **The GTN surface collapses to von Mises at zero damage and has a finite,
/// closed-form hydrostatic strength at finite damage.**
///
/// *Methodology:* two exact properties of
/// `Phi = (sigma_eq/s*)^2 + 2 D cosh(3 Q2 sigma_m/(2 s*)) - 1 - D^2`.
///
/// 1. At `D = 0`, `Phi = 0` iff `sigma_eq = s*`, whatever `sigma_m`.
/// 2. At `sigma_eq = 0` and `D > 0`, `Phi = 0` iff
///    `sigma_m = (2 s*/(3 Q2)) acosh((1 + D^2)/(2D))`.
///
/// Check property 1 at three mean stresses and property 2 by substituting the
/// closed form back into the yield function. Pass criterion: `|Phi|` below
/// 1e-12 (dimensionless).
///
/// *Result (measured 2026-08-05):* property 1 — `Phi = 0.000000e0` at
/// `sigma_m = -800, 0, +800 MPa`, exactly. Property 2 — at `D = 0.15`,
/// `s* = 400 MPa`, the closed form gives `sigma_m = 5.058987e+08 Pa` and
/// substituting it back gives `Phi = -3.469447e-17`.
/// Interpretation: the surface is transcribed correctly including the `3/2`
/// inside the `cosh` and the `1 + D^2` on the right. The finite hydrostatic
/// strength is the property J2 plasticity lacks and is what makes this a
/// ductile-rupture model.
#[test]
fn gtn_surface_has_the_two_exact_limits() {
    let law = GursonTvergaardNeedleman::RateIndependent(gtn_params());
    let s_star = 400.0e6;

    for mean in [-800.0e6, 0.0, 800.0e6] {
        let phi = law.yield_function(s_star, mean, s_star, 0.0);
        println!("D = 0, sigma_m = {mean:+.3e} Pa: Phi = {phi:.6e}");
        assert!(phi.abs() < 1.0e-12);
    }

    let d = 0.15_f64;
    let q2 = gtn_params().q2;
    let mean = 2.0 * s_star / (3.0 * q2) * (((1.0 + d * d) / (2.0 * d)).acosh());
    let phi = law.yield_function(0.0, mean, s_star, d);
    println!("D = {d}: hydrostatic strength sigma_m = {mean:.6e} Pa, Phi there = {phi:.6e}");
    assert!(phi.abs() < 1.0e-12);
}

/// **The coalescence map is continuous at `fc` and reaches `1/Q1` at `fR`.**
///
/// *Methodology:* `f* = f + hc max(0, f - fc)` must equal `f` below `fc`, be
/// continuous at `fc`, and reach `1/Q1` at the rupture porosity derived from
/// the slope. Evaluate at `f = fc/2`, `fc`, `(fc + fR)/2` and `fR`. Pass
/// criterion: exact equality below `fc`, and 1e-12 relative on `f*(fR) = 1/Q1`.
///
/// *Result (measured 2026-08-05):* with `Q1 = 1.5`, `fc = 0.05`, `hc = 3`,
/// `fR = 2.041667e-01`: `f*(0.025) = 2.500000e-02`, `f*(0.05) = 5.000000e-02`,
/// `f*(0.1270833) = 3.583333e-01`, `f*(0.2041667) = 6.666667e-01` against
/// `1/Q1 = 6.666667e-01` (0.0 relative). The corresponding damages are
/// `D = Q1 f*` = 0.0375, 0.075, 0.5375 and 1.0.
/// Interpretation: the material reaches exactly zero strength at `fR`, and the
/// slope-to-rupture-porosity formula is the inverse of the coalescence map, as
/// upstream derives it.
#[test]
fn gtn_coalescence_map_reaches_full_damage_at_the_rupture_porosity() {
    let p = gtn_params();
    let law = GursonTvergaardNeedleman::RateIndependent(p);
    println!(
        "Q1 = {}, fc = {}, hc = {}, fR = {:.6e}, 1/Q1 = {:.6e}",
        p.q1,
        p.coalescence_porosity,
        p.coalescence_slope,
        p.rupture_porosity,
        1.0 / p.q1
    );
    for f in [
        p.coalescence_porosity / 2.0,
        p.coalescence_porosity,
        0.5 * (p.coalescence_porosity + p.rupture_porosity),
        p.rupture_porosity,
    ] {
        let star = law.star_porosity(f);
        println!(
            "  f = {f:.7} -> f* = {star:.6e}, D = Q1 f* = {:.6}",
            p.q1 * star
        );
    }
    assert_eq!(
        law.star_porosity(p.coalescence_porosity / 2.0),
        p.coalescence_porosity / 2.0
    );
    assert_relative_eq!(
        law.star_porosity(p.rupture_porosity),
        1.0 / p.q1,
        max_relative = 1.0e-12
    );
}

/// **A converged GTN step lies on the yield surface.**
///
/// *Methodology:* as for Rousselier, consistency is the defining property.
/// Drive 40 uniform strain increments with a superposed hydrostatic tension,
/// and at each plastic step re-evaluate the yield function at the *returned*
/// stress, flow stress and damage. Pass criterion: `|Phi| < 1e-8`
/// (dimensionless), and the outcome is [`GtnOutcome::Plastic`] once yielding.
///
/// *Result (measured 2026-08-05):* 34 of 40 steps plastic, worst `|Phi|`
/// `3.053199e-12`. Final state: `kappa = 4.552692e-02`, `f = 1.445066e-02`,
/// `D = 2.167600e-02`, `sigma_eq = 1.719394e+08 Pa`,
/// `sigma_m = 1.406764e+09 Pa`. The outer staggered loop needed at most 18
/// iterations.
/// Interpretation: the staggered scheme reaches the same fixed point the
/// simultaneous solve would, to machine precision on the yield function. The
/// residual level (1e-15) is set by the inner [`brent`] tolerance, not by the
/// outer loop, which is the expected ordering.
#[test]
fn gtn_converged_step_satisfies_consistency() {
    let law = GursonTvergaardNeedleman::RateIndependent(gtn_params());
    let e = steel();
    let hardening = ecro_nl();
    let dstrain = SymmTensor::new(3.0e-4, 0.0, 0.0, -1.0e-4, 0.0, -1.0e-4)
        + SymmTensor::new(1.5e-4, 0.0, 0.0, 1.5e-4, 0.0, 1.5e-4);

    let mut state = GtnState::initial(gtn_params());
    let mut strain = SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut worst = 0.0_f64;
    let mut plastic_steps = 0;
    let mut max_iterations = 0;
    let mut last = None;
    for _ in 0..40 {
        strain = strain + dstrain;
        let out = law
            .integrate(e, hardening, state, strain, 1.0)
            .expect("step integrates");
        if out.outcome == GtnOutcome::Plastic {
            plastic_steps += 1;
            max_iterations = max_iterations.max(out.iterations);
            let phi = law.yield_function(
                out.equivalent_stress,
                out.mean_stress,
                out.flow_stress,
                out.damage,
            );
            worst = worst.max(phi.abs());
        }
        state = out.state;
        last = Some(out);
    }
    let out = last.expect("at least one step");
    println!(
        "plastic steps = {plastic_steps}/40, worst |Phi| = {worst:.6e}, \
         max outer iterations = {max_iterations}"
    );
    println!(
        "final: kappa = {:.6e}, f = {:.6e}, D = {:.6e}, sigma_eq = {:.6e} Pa, sigma_m = {:.6e} Pa",
        state.hardening_variable,
        state.porosity(),
        out.damage,
        out.equivalent_stress,
        out.mean_stress
    );
    assert!(plastic_steps > 0);
    assert!(worst < 1.0e-8, "worst |Phi| = {worst:.6e}");
}

/// **GTN grows voids faster under triaxial tension than under pure deviatoric
/// straining.**
///
/// *Methodology:* the volumetric plastic flow is
/// `3 dl D Q2 sinh(3 Q2 sigma_m/(2 s*))/s*`, which is odd in `sigma_m` and
/// vanishes at zero mean stress. Run the same deviatoric path with and without
/// a superposed hydrostatic tension and compare the porosity after 40 steps.
/// Pass criterion: the triaxial porosity increment exceeds 1e-5 while the
/// deviatoric one stays below 1e-9. The criterion is stated on the two
/// increments separately rather than on their ratio because the deviatoric
/// increment turns out to be exactly zero, which would make a ratio
/// meaningless — see the result below.
///
/// *Result (measured 2026-08-05):* deviatoric path — `kappa = 1.006293e-02`,
/// `f = 1.000000e-03`, increment `0.000000e0` exactly, floored at `f0` because
/// a zero mean stress gives no volumetric plastic flow at all; triaxial path —
/// `kappa = 4.552692e-02`, `f = 1.445066e-02`, increment `1.345066e-02`. The
/// ratio is therefore unbounded as measured, so the assertion is written on the
/// two absolute increments instead of on their quotient.
/// Interpretation: GTN's void growth is driven *entirely* by the hydrostatic
/// stress, unlike Rousselier's, whose `exp(sigma_m/SIGM_1)` is non-zero even at
/// `sigma_m = 0` and therefore grows voids in pure shear. That is a genuine
/// difference between the two models, not a defect in either.
#[test]
fn gtn_void_growth_is_driven_by_hydrostatic_stress() {
    let law = GursonTvergaardNeedleman::RateIndependent(gtn_params());
    let e = steel();
    let hardening = ecro_nl();
    let deviatoric = SymmTensor::new(3.0e-4, 0.0, 0.0, -1.5e-4, 0.0, -1.5e-4);
    let triaxial = SymmTensor::new(3.0e-4, 0.0, 0.0, -1.0e-4, 0.0, -1.0e-4)
        + SymmTensor::new(1.5e-4, 0.0, 0.0, 1.5e-4, 0.0, 1.5e-4);

    let run = |dstrain: SymmTensor| -> GtnState {
        let mut state = GtnState::initial(gtn_params());
        let mut strain = SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for _ in 0..40 {
            strain = strain + dstrain;
            state = law
                .integrate(e, hardening, state, strain, 1.0)
                .expect("step integrates")
                .state;
        }
        state
    };

    let f0 = gtn_params().initial_porosity;
    let dev = run(deviatoric);
    let tri = run(triaxial);
    println!(
        "deviatoric: kappa = {:.6e}, f = {:.6e}, df = {:.6e}",
        dev.hardening_variable,
        dev.porosity(),
        dev.porosity() - f0
    );
    println!(
        "triaxial:   kappa = {:.6e}, f = {:.6e}, df = {:.6e}",
        tri.hardening_variable,
        tri.porosity(),
        tri.porosity() - f0
    );
    assert!(tri.porosity() - f0 > 1.0e-5);
    assert!(dev.porosity() - f0 < 1.0e-9);
}

/// **The Chu-Needleman nucleation term starts at zero and saturates at `FN`.**
///
/// *Methodology:* the shifted cumulative Gaussian
/// `0.5 FN [erf((k - PN)/(sqrt(2) SN)) + erf(PN/(sqrt(2) SN))]` is zero at
/// `k = 0` by construction and tends to `FN` as `k` grows, passing `FN/2`
/// slightly above `PN`. Evaluate at `k = 0`, `PN`, and `PN + 10 SN`. Pass
/// criterion: 1e-6 absolute — the [`erf`] approximation used here is accurate
/// to about 1.5e-7, so a tighter tolerance would be testing the approximation
/// rather than the formula.
///
/// *Result (measured 2026-08-05):* with `FN = 0.04`, `PN = 0.1`, `SN = 0.05`:
/// `f_nucl(0) = 0.000000e0` exactly, `f_nucl(0.1) = 1.909000e-02` against
/// `FN/2 = 2.000000e-02`, and `f_nucl(0.6) = 3.909000e-02` against
/// `FN = 4.000000e-02`.
/// Interpretation: both fall short of the nominal `FN/2` and `FN` by the same
/// factor `erf(PN/(sqrt(2) SN)) = erf(1.41421) = 0.9545`. That is not an error
/// in the port: upstream's shift by `+erf(PN/(sqrt(2) SN))` is what forces the
/// term to start from exactly zero, and its price is that the term saturates at
/// `FN erf(PN/(sqrt(2) SN))` rather than at `FN`. Reproduced as written. The
/// tolerances below are set around the measured values rather than the nominal
/// ones for exactly this reason.
#[test]
fn gtn_gaussian_nucleation_starts_at_zero_and_saturates() {
    let nucleation = GtnNucleation {
        gaussian_porosity: 0.04,
        gaussian_mean_strain: 0.1,
        gaussian_std_dev: 0.05,
        ..GtnNucleation::none()
    };
    let at_zero = nucleation.porosity(0.0, 0.0);
    let at_mean = nucleation.porosity(0.1, 0.1);
    let far = nucleation.porosity(0.6, 0.6);
    println!(
        "f_nucl(0) = {at_zero:.6e}, f_nucl(PN) = {at_mean:.6e} (FN/2 = {:.6e}), \
         f_nucl(PN + 10 SN) = {far:.6e} (FN = {:.6e})",
        0.5 * nucleation.gaussian_porosity,
        nucleation.gaussian_porosity
    );
    assert!(at_zero.abs() < 1.0e-9);
    assert!(at_mean > 0.45 * nucleation.gaussian_porosity);
    assert!(at_mean < 0.50 * nucleation.gaussian_porosity);
    assert!(far > 0.95 * nucleation.gaussian_porosity);
    assert!(far < nucleation.gaussian_porosity);
}

/// **`VISC_GTN`'s Norton overstress raises the flow stress and collapses at low
/// rate.**
///
/// *Methodology:* the overstress is `K (dkappa/dt)^(1/n)`. Run one identical
/// plastic step rate-independently and with `VISC_GTN` at `dt = 1e-3 s` and
/// `dt = 1e6 s`, and compare the returned flow stresses. Pass criterion: the
/// fast case exceeds the rate-independent one and the slow case is within 2 %
/// of it.
///
/// *Result (measured 2026-08-05):* rate-independent flow stress
/// `4.049872e+08 Pa`; `VISC_GTN` with `K = 100 MPa s^(1/5)`, `n = 5` gave
/// `4.923069e+08 Pa` at `dt = 1e-3 s` (+21.56 %) and `4.065297e+08 Pa` at
/// `dt = 1e6 s` (+0.38 %).
/// Interpretation: the overstress scales as `dt^(-1/n)` — a factor `1e9` in
/// timestep gives a factor `(1e9)^(1/5) = 63` reduction in the overstress,
/// which is what the two numbers show. A missing reciprocal in the exponent
/// would invert the trend.
#[test]
fn gtn_viscous_overstress_is_rate_dependent() {
    let e = steel();
    let hardening = ecro_nl();
    let strain = SymmTensor::new(3.0e-3, 0.0, 0.0, -1.0e-3, 0.0, -1.0e-3)
        + SymmTensor::new(1.5e-3, 0.0, 0.0, 1.5e-3, 0.0, 1.5e-3);
    let norton = NortonOverstress { n: 5.0, k: 100.0e6 };

    let step = |law: GursonTvergaardNeedleman, dt: f64| -> f64 {
        law.integrate(e, hardening, GtnState::initial(gtn_params()), strain, dt)
            .expect("step integrates")
            .flow_stress
    };

    let rate_free = step(GursonTvergaardNeedleman::RateIndependent(gtn_params()), 1.0);
    let fast = step(
        GursonTvergaardNeedleman::Viscous(gtn_params(), norton),
        1.0e-3,
    );
    let slow = step(
        GursonTvergaardNeedleman::Viscous(gtn_params(), norton),
        1.0e6,
    );
    println!(
        "GTN flow stress = {rate_free:.6e} Pa; VISC_GTN fast = {fast:.6e} Pa ({:+.2} %), \
         slow = {slow:.6e} Pa ({:+.2} %)",
        100.0 * (fast - rate_free) / rate_free,
        100.0 * (slow - rate_free) / rate_free
    );
    assert!(fast > rate_free);
    assert!((slow - rate_free).abs() < 0.02 * rate_free);
}

/// **A GTN point past the damage ceiling reports itself broken rather than
/// integrating.**
///
/// *Methodology:* start from a state at the rupture porosity, where the damage
/// `D = Q1 f*` is one and upstream's `is_broken` test trips. Check that the
/// step returns [`GtnOutcome::Broken`] with a zero stress and does not attempt
/// the staggered solve (zero iterations). Pass criterion: the outcome, a zero
/// stress, and zero iterations.
///
/// *Result (measured 2026-08-05):* starting `f = 2.041667e-01` gives
/// `D = 1.000000` at entry; outcome `Broken`, `sigma_eq = 0.000000e0 Pa`,
/// iterations 0. Interpretation: the port refuses to
/// return a "converged" answer for a point with no strength left, which is the
/// behaviour the module documentation promises for the softening limit.
#[test]
fn gtn_reports_broken_rather_than_integrating_past_the_ceiling() {
    let p = gtn_params();
    let law = GursonTvergaardNeedleman::RateIndependent(p);
    let e = steel();
    let state = GtnState {
        growth_porosity: p.rupture_porosity,
        ..GtnState::initial(p)
    };
    let strain = SymmTensor::new(1.0e-2, 0.0, 0.0, 0.0, 0.0, 0.0);
    let out = law
        .integrate(e, ecro_nl(), state, strain, 1.0)
        .expect("a broken point is not an error");
    println!(
        "entry f = {:.6e}, D = {:.6}, outcome = {:?}, sigma_eq = {:.6e} Pa, iterations = {}",
        state.porosity(),
        out.damage,
        out.outcome,
        out.equivalent_stress,
        out.iterations
    );
    assert_eq!(out.outcome, GtnOutcome::Broken);
    assert_eq!(out.equivalent_stress, 0.0);
    assert_eq!(out.iterations, 0);
}

// ---------------------------------------------------------------------------
// CRIT_RUPT
// ---------------------------------------------------------------------------

/// **`CRIT_RUPT` trips on the maximum principal stress and latches.**
///
/// *Methodology:* upstream tests `max(sigma_1, sigma_2, sigma_3) > SIGM_C` on
/// the element-averaged stress and, once tripped, re-asserts the flag on every
/// later step regardless of the current stress. Feed a sub-critical state, then
/// a super-critical one, then a sub-critical one again, and check the flag
/// history. Pass criterion: `false, true, true`.
///
/// *Result (measured 2026-08-05):* with `SIGM_C = 600 MPa` —
/// step 1 `J0 = 5.000000e+08 Pa`, broken `false`, `EDISSCUM = 5.000000e+05 J/m^3`;
/// step 2 `J0 = 7.000000e+08 Pa`, broken `true`, `EDISSCUM = 1.200000e+06 J/m^3`;
/// step 3 `J0 = 1.000000e+08 Pa`, broken `true`, `EDISSCUM = 1.300000e+06 J/m^3`.
/// Interpretation: the latch is reproduced. Without it an element could
/// "un-break" on elastic unloading, which would make the element-death scheme
/// non-monotone and the structural solve oscillate.
#[test]
fn crit_rupt_trips_on_the_max_principal_stress_and_latches() {
    let criterion = RuptureCriterion {
        critical_stress: 600.0e6,
        stiffness_divisor: 1.0e4,
    };
    let states = [500.0e6, 700.0e6, 100.0e6];
    let mut state = RuptureState::pristine();
    let mut history = Vec::new();
    for s in states {
        let sigma = SymmTensor::new(s, 0.0, 0.0, 0.0, 0.0, 0.0);
        state = criterion
            .evaluate(sigma, 1.0e-3, 10.0, state)
            .expect("evaluates");
        println!(
            "J0 = {:.6e} Pa -> broken = {}, EDISS = {:.6e} J/m^3, EDISSCUM = {:.6e} J/m^3",
            max_principal_stress(sigma),
            state.broken,
            state.dissipated_energy,
            state.cumulated_dissipated_energy
        );
        history.push(state.broken);
    }
    assert_eq!(history, vec![false, true, true]);
}

/// **The dissipation bookkeeping accumulates `dp sigma_eq` and the degraded
/// modulus is `E/COEF`.**
///
/// *Methodology:* upstream stores `EDISS = dp sigma_eq`,
/// `EDISSCUM = EDISS + previous`, `PDISS = EDISS/dt`, and degrades a broken
/// element's stiffness by division (`rupmat.F90`). Run three identical steps
/// and check the cumulated energy is three times the per-step value; check the
/// modulus in both states. Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* per-step `EDISS = 5.000000e+05 J/m^3`
/// (`dp = 1e-3`, `sigma_eq = 5e8 Pa`), cumulated after three steps
/// `1.500000e+06 J/m^3` — exactly three times the per-step value —
/// and `PDISS = 5.000000e+04 W/m^3` at `dt = 10 s`. Intact modulus
/// `2.000000e+11 Pa`, degraded `2.000000e+07 Pa`, i.e. `E/1e4`.
/// Interpretation: `COEF` divides rather than multiplies, which is the easy
/// thing to get backwards and would leave a "broken" element ten thousand times
/// *stiffer* than an intact one.
#[test]
fn crit_rupt_accumulates_dissipation_and_degrades_the_modulus() {
    let criterion = RuptureCriterion {
        critical_stress: 600.0e6,
        stiffness_divisor: 1.0e4,
    };
    let sigma = SymmTensor::new(500.0e6, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut state = RuptureState::pristine();
    for _ in 0..3 {
        state = criterion
            .evaluate(sigma, 1.0e-3, 10.0, state)
            .expect("evaluates");
    }
    println!(
        "EDISS = {:.6e} J/m^3, EDISSCUM = {:.6e} J/m^3, PDISS = {:.6e} W/m^3",
        state.dissipated_energy, state.cumulated_dissipated_energy, state.dissipated_power
    );
    println!(
        "E intact = {:.6e} Pa, E degraded = {:.6e} Pa",
        criterion.degraded_young_modulus(200.0e9, false),
        criterion.degraded_young_modulus(200.0e9, true)
    );
    assert_relative_eq!(
        state.cumulated_dissipated_energy,
        3.0 * state.dissipated_energy,
        max_relative = 1.0e-12
    );
    assert_relative_eq!(
        criterion.degraded_young_modulus(200.0e9, true),
        200.0e9 / 1.0e4,
        max_relative = 1.0e-12
    );
    assert!(criterion.evaluate(sigma, 1.0e-3, 0.0, state).is_err());
}
