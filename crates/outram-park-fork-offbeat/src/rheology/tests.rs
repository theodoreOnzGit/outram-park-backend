// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Exercises the port of `offbeatLib/rheology/`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Tests for the rheology layer.
//!
//! # What kind of test each of these is
//!
//! Per the workspace V&V rule, every test below states its **methodology**
//! (inputs, reference, tolerance, pass criterion) and its **measured result**.
//! They fall into two classes, and the class is named in each test's doc
//! comment:
//!
//! - **Verification against a closed form.** The reference is an exact
//!   analytical solution of the *same* equations the code claims to solve —
//!   the radial-return consistency condition, the backward-Euler creep
//!   increment for a linear creep law. These check that the implementation is
//!   correct, not that the model is right.
//! - **Self-consistency check.** No independent reference exists, so the test
//!   asserts a property the answer must have: monotonicity, a conservation, a
//!   limit, an inequality that a converged return mapping cannot violate.
//!
//! **None of these is a validation.** Nothing here compares against experiment
//! or against a published benchmark, and no result below may be cited as
//! evidence that the correlations reproduce reality. Validation of the Limbäck
//! and MATPRO correlations against their source data is separate, unstarted
//! work.

use std::sync::Arc;

use outram_foam_basic_lib::primitives::SymmTensor;

use crate::error::OffbeatError;
use crate::materials::MaterialState;
use crate::mechanics::LinearElastic;

use super::*;

/// Young's modulus \[Pa\] used throughout: a round 100 GPa, close enough to
/// Zircaloy at temperature to be recognisable and round enough to check by
/// hand.
const YOUNG: f64 = 100.0e9;
/// Poisson's ratio \[-\].
const POISSON: f64 = 0.3;

fn elastic() -> LinearElastic {
    LinearElastic::new(YOUNG, POISSON).expect("100 GPa / 0.3 is a valid elastic pair")
}

/// A pure-shear strain state of magnitude `gamma_half` in the xy component.
///
/// Chosen because it is purely deviatoric, so the hydrostatic stress is exactly
/// zero and any non-zero trace in the answer is a bug rather than round-off.
fn pure_shear(gamma_half: f64) -> SymmTensor {
    SymmTensor::new(0.0, gamma_half, 0.0, 0.0, 0.0, 0.0)
}

fn quasi_static(strain: SymmTensor) -> RheologyInputs {
    RheologyInputs::quasi_static(elastic(), strain, MaterialState::fresh(600.0))
}

// ---------------------------------------------------------------------------
// Elasticity
// ---------------------------------------------------------------------------

/// **Verification against a closed form.** Hooke's law below yield.
///
/// *Methodology.* Isotropic material, E = 100 GPa, ν = 0.3 (μ = 38.4615 GPa,
/// K = 83.3333 GPa). Mechanical strain = pure shear ε_xy = 1e-4 plus a
/// volumetric part ε_xx = ε_yy = ε_zz = 1e-5. Law: [`ConstitutiveLaw::Elastic`],
/// no creep, dt = 0. Reference: `σ = K tr(ε) I + 2μ dev(ε)` evaluated
/// analytically. Pass criterion: every stress component within 1e-6 relative.
///
/// *Result.* σ_xy = 7.692308 MPa (analytic 2μ·1e-4 = 7.692308 MPa) and
/// σ_xx = σ_yy = σ_zz = 2.500000 MPa (analytic 3K·1e-5 = 2.5 MPa), both to
/// better than 1e-12 relative. The elastic law is exactly Hooke's law, as it
/// must be.
#[test]
fn elastic_law_reproduces_hookes_law_exactly() {
    let e = elastic();
    let strain = pure_shear(1.0e-4) + SymmTensor::from_diag(1.0e-5, 1.0e-5, 1.0e-5);
    let inputs = quasi_static(strain);
    let out = ConstitutiveLaw::Elastic
        .correct(0, &inputs, &RheologyState::pristine())
        .expect("elastic law cannot fail");

    let expected_shear = 2.0 * e.shear_modulus() * 1.0e-4;
    let expected_normal = e.three_k() * 1.0e-5;

    assert!((out.stress.xy - expected_shear).abs() < 1.0e-6 * expected_shear);
    assert!((out.stress.xx - expected_normal).abs() < 1.0e-6 * expected_normal);
    assert!((out.stress.yy - expected_normal).abs() < 1.0e-6 * expected_normal);
    assert!((out.stress.zz - expected_normal).abs() < 1.0e-6 * expected_normal);
    assert!(!out.yielding);
    assert_eq!(out.equivalent_plastic_strain_increment, 0.0);
    assert_eq!(out.equivalent_creep_strain_increment, 0.0);
}

/// **Verification against a closed form.** A plastic law below yield is
/// indistinguishable from an elastic one.
///
/// *Methodology.* Same strain state as above, but with
/// [`ConstitutiveLaw::perfectly_plastic`] at σ_y = 300 MPa. The trial von Mises
/// stress is sqrt(3)·2μ·1e-4 = 13.32347 MPa, well below yield. Reference: the
/// stress returned by [`ConstitutiveLaw::Elastic`] for the identical input.
/// Pass criterion: component-wise identity to 1e-9 Pa, and `yielding == false`.
///
/// *Result.* Identical to the last bit; the elastic branch of the return map
/// does not touch the stress. This is the branch a fuel rod spends almost all
/// of its life in, which is why it is checked to be exactly free.
#[test]
fn plasticity_below_yield_returns_the_elastic_stress_unchanged() {
    let strain = pure_shear(1.0e-4) + SymmTensor::from_diag(1.0e-5, 1.0e-5, 1.0e-5);
    let inputs = quasi_static(strain);
    let state = RheologyState::pristine();

    let elastic_out = ConstitutiveLaw::Elastic
        .correct(0, &inputs, &state)
        .unwrap();
    let plastic_out = ConstitutiveLaw::perfectly_plastic(300.0e6)
        .correct(0, &inputs, &state)
        .unwrap();

    assert!(plastic_out.von_mises_stress() < 300.0e6);
    assert!(!plastic_out.yielding);
    assert!((plastic_out.stress.xy - elastic_out.stress.xy).abs() < 1.0e-9);
    assert!((plastic_out.stress.xx - elastic_out.stress.xx).abs() < 1.0e-9);
    assert!((plastic_out.stress.zz - elastic_out.stress.zz).abs() < 1.0e-9);
}

// ---------------------------------------------------------------------------
// Plasticity
// ---------------------------------------------------------------------------

/// **Verification against a closed form.** Perfect plasticity returns exactly
/// onto the yield surface.
///
/// *Methodology.* Pure shear ε_xy = 1e-2, giving a trial von Mises stress of
/// sqrt(3)·2μ·1e-2 = 1332.347 MPa, far above the σ_y = 300 MPa yield stress.
/// Reference: classical J2 radial return with zero hardening, for which the
/// converged von Mises stress equals σ_y *exactly*. Pass criterion:
/// |q − σ_y| / σ_y < 1e-10.
///
/// *Result.* q = 3.000000000e+08 Pa against the 300 MPa yield surface, relative
/// error below 1e-15. Δε_p,eq = 8.947005e-03, matching the closed-form
/// sqrt(2/3)·f_trial/(2μ) to better than 1e-12 relative. Two Newton iterations
/// — one to reach the answer, one to confirm the residual — as expected for a
/// non-hardening curve.
#[test]
fn perfect_plasticity_returns_exactly_onto_the_yield_surface() {
    let sigma_y = 300.0e6;
    let inputs = quasi_static(pure_shear(1.0e-2));
    let out = ConstitutiveLaw::perfectly_plastic(sigma_y)
        .correct(0, &inputs, &RheologyState::pristine())
        .unwrap();

    assert!(out.yielding);
    let q = out.von_mises_stress();
    assert!(
        (q - sigma_y).abs() / sigma_y < 1.0e-10,
        "von Mises stress {q} does not lie on the {sigma_y} Pa yield surface"
    );

    // Closed-form plastic multiplier for zero hardening.
    let mu = elastic().shear_modulus();
    let s_trial = 2.0 * mu * inputs.mechanical_strain.dev();
    let f_trial = s_trial.mag() - (2.0f64 / 3.0).sqrt() * sigma_y;
    let expected_eq = (2.0f64 / 3.0).sqrt() * f_trial / (2.0 * mu);
    assert!((out.equivalent_plastic_strain_increment - expected_eq).abs() < 1.0e-12 * expected_eq);
}

/// **Verification against a closed form.** Linear hardening reproduces the
/// analytic plastic multiplier.
///
/// *Methodology.* Pure shear ε_xy = 1e-2 into a linearly hardening material:
/// σ_y = 300 MPa at zero plastic strain rising to 800 MPa at ε_p,eq = 0.5, i.e.
/// H = 1.0 GPa. Reference: the exact solution of the consistency condition for
/// constant H, Δλ = f_trial / (2μ + 2H/3). Pass criterion: 1e-10 relative on
/// the equivalent plastic increment, and the von Mises stress on the *updated*
/// yield surface to 1e-10 relative.
///
/// *Result.* Δε_p,eq = 8.8701309e-03 against the closed form 8.8701309e-03
/// (relative difference below 1e-15), and q = 3.0887013e+08 Pa against the
/// updated σ_y = 3.0887013e+08 Pa. Newton converged in 2 iterations, which is
/// the expected count for a linear curve (one step to the answer, one to
/// confirm the residual).
#[test]
fn linear_hardening_matches_the_closed_form_return_map() {
    let curve = HardeningCurve::new(vec![(0.0, 300.0e6), (0.5, 800.0e6)]).unwrap();
    let hardening_modulus = (800.0e6 - 300.0e6) / 0.5; // 1.0 GPa
    let law = ConstitutiveLaw::MisesPlasticity {
        yield_stress: YieldStressModel::Hardening { curve },
    };

    let inputs = quasi_static(pure_shear(1.0e-2));
    let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();

    let mu = elastic().shear_modulus();
    let s_trial = 2.0 * mu * inputs.mechanical_strain.dev();
    let f_trial = s_trial.mag() - (2.0f64 / 3.0).sqrt() * 300.0e6;
    let d_lambda = f_trial / (2.0 * mu + (2.0 / 3.0) * hardening_modulus);
    let expected_eq = (2.0f64 / 3.0).sqrt() * d_lambda;

    assert!(
        (out.equivalent_plastic_strain_increment - expected_eq).abs() < 1.0e-10 * expected_eq,
        "measured {} vs closed form {expected_eq}",
        out.equivalent_plastic_strain_increment
    );

    let sigma_y_new = 300.0e6 + hardening_modulus * expected_eq;
    assert!((out.von_mises_stress() - sigma_y_new).abs() < 1.0e-10 * sigma_y_new);
    assert!((out.yield_stress - sigma_y_new).abs() < 1.0e-10 * sigma_y_new);
}

/// **Self-consistency check.** The von Mises stress never exceeds the yield
/// surface after a converged return mapping, at any load level.
///
/// *Methodology.* Sweep the applied pure shear over five decades,
/// ε_xy ∈ {1e-5, 1e-4, 1e-3, 1e-2, 1e-1}, through the same hardening curve as
/// above. Reference: none — this is the defining inequality of the yield
/// criterion, which a converged return map cannot violate. Pass criterion:
/// q ≤ σ_y(1 + 1e-9) for every load.
///
/// *Result.* Satisfied at every load. Below yield (1e-5) the stress is strictly
/// inside; at and above 1e-4 it sits on the surface to within 1e-15 relative.
#[test]
fn von_mises_stress_never_exceeds_the_yield_surface() {
    let curve = HardeningCurve::new(vec![(0.0, 300.0e6), (0.5, 800.0e6)]).unwrap();
    let law = ConstitutiveLaw::MisesPlasticity {
        yield_stress: YieldStressModel::Hardening { curve },
    };

    for exponent in 1..=5 {
        let gamma = 10.0f64.powi(-exponent);
        let inputs = quasi_static(pure_shear(gamma));
        let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();
        assert!(
            out.von_mises_stress() <= out.yield_stress * (1.0 + 1.0e-9),
            "at gamma = {gamma}: q = {} exceeds sigma_y = {}",
            out.von_mises_stress(),
            out.yield_stress
        );
    }
}

/// **Self-consistency check.** Plastic flow is volume preserving and leaves the
/// hydrostatic stress untouched.
///
/// *Methodology.* A strain state with both a large deviatoric part
/// (ε_xy = 1e-2, well past yield) and a volumetric part (ε_xx = ε_yy = ε_zz =
/// 1e-3), through a perfectly plastic law at σ_y = 300 MPa. Reference: none —
/// this is a structural property of J2 plasticity, whose flow direction is
/// deviatoric by construction. Pass criterion: |tr(Δε_p)| < 1e-18, and the
/// hydrostatic stress equal to the purely elastic value K·tr(ε) to 1e-9
/// relative.
///
/// *Result.* tr(Δε_p) = 0 to machine zero, and the hydrostatic stress is
/// 2.500000e+08 Pa (= K·tr(ε) = 83.3333 GPa × 3e-3), identical to the elastic
/// answer. Yielding a material does not change how hard it is to compress.
#[test]
fn plastic_flow_is_deviatoric_and_leaves_the_hydrostatic_stress_alone() {
    let strain = pure_shear(1.0e-2) + SymmTensor::from_diag(1.0e-3, 1.0e-3, 1.0e-3);
    let inputs = quasi_static(strain);
    let out = ConstitutiveLaw::perfectly_plastic(300.0e6)
        .correct(0, &inputs, &RheologyState::pristine())
        .unwrap();

    assert!(out.yielding);
    assert!(out.plastic_strain_increment.tr().abs() < 1.0e-18);

    let expected_hydrostatic = elastic().three_k() / 3.0 * strain.tr();
    assert!(
        (out.hydrostatic_stress() - expected_hydrostatic).abs()
            < 1.0e-9 * expected_hydrostatic.abs()
    );
}

/// **Self-consistency check.** A softening curve steeper than −3μ is reported
/// as non-convergence, not silently returned.
///
/// *Methodology.* A hardening curve that *falls* from 300 MPa to 1 MPa over
/// Δε_p,eq = 1e-3, i.e. H = −2.99e11 Pa. The return-map Jacobian is
/// −2μ − (2/3)H = −7.69e10 + 1.993e11 > 0, so the consistency condition has no
/// stable root and the material is past the point of localisation. Reference:
/// none — the pass criterion is that the failure is *reported*. Pass criterion:
/// [`OffbeatError::ConstitutiveNotConverged`] naming the offending cell.
///
/// *Result.* `ConstitutiveNotConverged { cell: 7, .. }` as required. The
/// alternative — returning an unconverged stress outside the yield surface —
/// would propagate into the momentum balance and look like a stiffer material.
#[test]
fn catastrophic_softening_is_reported_rather_than_returned() {
    let curve = HardeningCurve::new(vec![(0.0, 300.0e6), (1.0e-3, 1.0e6)]).unwrap();
    let law = ConstitutiveLaw::MisesPlasticity {
        yield_stress: YieldStressModel::Hardening { curve },
    };
    let inputs = quasi_static(pure_shear(1.0e-2));

    match law.correct(7, &inputs, &RheologyState::pristine()) {
        Err(OffbeatError::ConstitutiveNotConverged { cell, .. }) => assert_eq!(cell, 7),
        other => panic!("expected ConstitutiveNotConverged, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Creep — closed-form verification
// ---------------------------------------------------------------------------

/// A Norton power law with exponent 1, i.e. a linear (Maxwell) creep law
/// `ε̇ = q/η`.
///
/// With `b` in 1/hr and `sigma_c` in MPa, the equivalent viscosity is
/// `η = 3600 σ_c·1e6 / b` \[Pa·s\]. Exponent 1 is the only case whose implicit
/// increment has a closed form, which is exactly why the verification tests use
/// it.
fn linear_creep(b: f64, sigma_c_mpa: f64) -> CreepModel {
    CreepModel::PowerLaw {
        b,
        sigma_c: sigma_c_mpa,
        n: 1.0,
    }
}

/// Equivalent viscosity \[Pa·s\] of [`linear_creep`].
fn linear_creep_viscosity(b: f64, sigma_c_mpa: f64) -> f64 {
    3600.0 * sigma_c_mpa * 1.0e6 / b
}

/// **Verification against a closed form.** The implicit creep increment matches
/// the analytic backward-Euler solution for a linear creep law.
///
/// *Methodology.* Linear creep `ε̇ = q/η` with b = 0.01 /hr and σ_C = 1 MPa, so
/// η = 3.6e11 Pa·s. Pure shear ε_xy = 1e-4, giving q_trial = 13.32347 MPa;
/// dt = 1 s; μ = 38.46154 GPa. The consistency condition
/// `Δε_c = Δt (q_trial − 3μ Δε_c)/η` is linear and inverts exactly to
/// `Δε_c = Δt q_trial / (η + 3μ Δt)`. Reference: that expression. Pass
/// criterion: 1e-10 relative on Δε_c, and the same on the relaxed von Mises
/// stress `q_trial − 3μ Δε_c`.
///
/// *Result.* Δε_c,eq = 2.8026712e-05 against the closed form 2.8026712e-05
/// (identical to the last bit), and q = 1.0089616e+07 Pa against the same
/// closed form. The safeguarded Newton iteration converged in 2 iterations, as
/// expected for a linear rate law.
#[test]
fn linear_creep_increment_matches_the_closed_form_implicit_solution() {
    let b = 0.01;
    let sigma_c = 1.0;
    let eta = linear_creep_viscosity(b, sigma_c);
    let mu = elastic().shear_modulus();

    let mut inputs = quasi_static(pure_shear(1.0e-4));
    inputs.dt = 1.0;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant {
            sigma_y: 1.0e12, // effectively elastic: isolate creep
        },
        creep: linear_creep(b, sigma_c),
    };
    let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();

    let s_trial = 2.0 * mu * inputs.mechanical_strain.dev();
    let q_trial = von_mises(s_trial);
    let expected = inputs.dt * q_trial / (eta + 3.0 * mu * inputs.dt);

    assert!(
        (out.equivalent_creep_strain_increment - expected).abs() < 1.0e-10 * expected,
        "measured {} vs closed form {expected}",
        out.equivalent_creep_strain_increment
    );

    let expected_q = q_trial - 3.0 * mu * expected;
    assert!((out.von_mises_stress() - expected_q).abs() < 1.0e-10 * expected_q);
    assert!(!out.yielding);
}

/// **Verification against a closed form.** Stress relaxation at fixed total
/// strain follows the analytic geometric decay of backward Euler.
///
/// *Methodology.* Hold the mechanical strain fixed at pure shear ε_xy = 1e-4
/// and take 20 timesteps of dt = 1 s with the same linear creep law
/// (η = 3.6e11 Pa·s), committing the state with [`RheologyState::advance`] after
/// each. For a linear creep law, backward Euler gives exactly
/// `q_n = q_0 · r^n` with `r = η / (η + 3μ dt) = 0.757282`. Reference: that
/// geometric sequence. Pass criterion: 1e-9 relative at every step.
///
/// *Result.* After 20 steps q = 5.125838e+04 Pa against the closed form
/// 5.125838e+04 Pa (relative difference below 1e-13); every intermediate step
/// agreed to the same precision. This also demonstrates the continuum limit —
/// `r^n → exp(−3μ t/η)` as dt → 0 — but the test checks the discrete solution,
/// because that is what the code actually integrates.
#[test]
fn stress_relaxation_follows_the_analytic_geometric_decay() {
    let b = 0.01;
    let sigma_c = 1.0;
    let eta = linear_creep_viscosity(b, sigma_c);
    let mu = elastic().shear_modulus();
    let dt = 1.0;
    let ratio = eta / (eta + 3.0 * mu * dt);

    let mut inputs = quasi_static(pure_shear(1.0e-4));
    inputs.dt = dt;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: linear_creep(b, sigma_c),
    };

    let q0 = von_mises(2.0 * mu * inputs.mechanical_strain.dev());
    let mut state = RheologyState::pristine();

    for step in 1..=20 {
        let out = law.correct(0, &inputs, &state).unwrap();
        state.advance(&out);
        let expected = q0 * ratio.powi(step);
        assert!(
            (out.von_mises_stress() - expected).abs() < 1.0e-9 * expected,
            "step {step}: measured {} vs closed form {expected}",
            out.von_mises_stress()
        );
    }
}

// ---------------------------------------------------------------------------
// Creep — self-consistency
// ---------------------------------------------------------------------------

/// **Self-consistency check.** A zero timestep produces a zero creep increment.
///
/// *Methodology.* The Limbäck cladding creep model under a realistic PWR
/// environment (T = 620 K, fast flux 1e18 n/m²/s, fluence 5e25 n/m²) at a trial
/// stress well inside the yield surface, with dt = 0. Reference: none — creep
/// is a rate, so no time means no creep. Pass criterion: exact zero on both the
/// equivalent increment and every component of the tensor increment.
///
/// *Result.* Both exactly zero, and the returned stress equals the elastic
/// stress to the last bit. This is the property that lets a caller reuse the
/// same law for a rate-independent equilibrium check.
#[test]
fn zero_timestep_produces_zero_creep_increment() {
    let mut inputs = quasi_static(pure_shear(1.0e-4));
    inputs.material.temperature = 620.0;
    inputs.material.fast_fluence = 5.0e25;
    inputs.irradiation.fast_flux = 1.0e18;
    inputs.dt = 0.0;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: CreepModel::Limback {
            clad_type: ZircaloyCladType::Sra,
        },
    };
    let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();

    assert_eq!(out.equivalent_creep_strain_increment, 0.0);
    assert_eq!(out.creep_strain_increment, SymmTensor::ZERO);

    let elastic_out = ConstitutiveLaw::Elastic
        .correct(0, &inputs, &RheologyState::pristine())
        .unwrap();
    assert_eq!(out.stress, elastic_out.stress);
}

/// **Self-consistency check.** Stress relaxes monotonically under a sustained
/// strain with creep active, and a fully relaxed state is deviatorically
/// stress-free.
///
/// *Methodology.* Hold pure shear ε_xy = 1e-4 and take 400 steps of dt = 1 s
/// with the linear creep law (η = 3.6e11 Pa·s, so the relaxation time
/// η/(3μ) = 3.12 s). Reference: none — monotone relaxation and the zero-stress
/// asymptote are structural properties of any creep law with a non-negative
/// rate. Pass criteria: (a) the von Mises stress is non-increasing at every
/// step; (b) after 400 steps it is below 1e-6 of its initial value; (c) the
/// accumulated equivalent creep strain approaches the applied equivalent strain
/// 1.1547005e-04.
///
/// *Result.* Strictly decreasing at every step. Final von Mises stress
/// 2.734796e-09 Pa, i.e. 2.05e-16 of the initial 1.332347e+07 Pa. Final
/// accumulated equivalent creep strain 1.1547005e-04 against the applied
/// equivalent strain 1.1547005e-04 — the whole of the applied deviatoric strain
/// has become permanent, which is the definition of full relaxation.
#[test]
fn stress_relaxes_monotonically_to_a_deviatorically_stress_free_state() {
    let b = 0.01;
    let sigma_c = 1.0;
    let mut inputs = quasi_static(pure_shear(1.0e-4));
    inputs.dt = 1.0;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: linear_creep(b, sigma_c),
    };

    let first = von_mises(2.0 * elastic().shear_modulus() * inputs.mechanical_strain.dev());
    let mut state = RheologyState::pristine();
    let mut previous = f64::INFINITY;

    for step in 0..400 {
        let out = law.correct(0, &inputs, &state).unwrap();
        let q = out.von_mises_stress();
        assert!(
            q <= previous,
            "step {step}: stress rose from {previous} to {q}"
        );
        previous = q;
        state.advance(&out);
    }

    assert!(
        previous < 1.0e-6 * first,
        "final q = {previous}, initial {first}"
    );

    let applied_eq = equivalent_strain(inputs.mechanical_strain);
    assert!(
        (state.equivalent_creep_strain - applied_eq).abs() < 1.0e-6 * applied_eq,
        "accumulated creep {} vs applied equivalent strain {applied_eq}",
        state.equivalent_creep_strain
    );
}

/// **Self-consistency check.** Creep is volume preserving.
///
/// *Methodology.* A strain state with both deviatoric (ε_xy = 1e-4) and
/// volumetric (ε_xx = ε_yy = ε_zz = 1e-3) parts, one step of dt = 100 s with
/// the linear creep law. Reference: none — Prandtl–Reuss flow is deviatoric by
/// construction. Pass criterion: |tr(Δε_c)| < 1e-18 and the hydrostatic stress
/// unchanged from the elastic value to 1e-12 relative.
///
/// *Result.* tr(Δε_c) = 0 to machine zero; hydrostatic stress 2.500000e+08 Pa,
/// identical to the elastic value. A creeping solid does not change volume, so
/// creep cannot relax a purely hydrostatic load — which is why an unvented,
/// fully constrained pellet builds hydrostatic pressure that creep cannot
/// shed.
#[test]
fn creep_is_volume_preserving() {
    let strain = pure_shear(1.0e-4) + SymmTensor::from_diag(1.0e-3, 1.0e-3, 1.0e-3);
    let mut inputs = quasi_static(strain);
    inputs.dt = 100.0;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: linear_creep(0.01, 1.0),
    };
    let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();

    assert!(out.equivalent_creep_strain_increment > 0.0);
    assert!(out.creep_strain_increment.tr().abs() < 1.0e-18);

    let expected_hydrostatic = elastic().three_k() / 3.0 * strain.tr();
    assert!(
        (out.hydrostatic_stress() - expected_hydrostatic).abs()
            < 1.0e-12 * expected_hydrostatic.abs()
    );
}

// ---------------------------------------------------------------------------
// Limbäck Zircaloy creep
// ---------------------------------------------------------------------------

/// Typical PWR cladding conditions: 620 K, 1e18 n/m²/s fast flux,
/// 5e25 n/m² accumulated fluence.
fn pwr_cladding_inputs(strain: SymmTensor, dt: f64) -> RheologyInputs {
    let mut material = MaterialState::fresh(620.0);
    material.fast_fluence = 5.0e25;
    let mut inputs = RheologyInputs::quasi_static(elastic(), strain, material);
    inputs.irradiation = IrradiationState {
        fast_flux: 1.0e18,
        ..IrradiationState::default()
    };
    inputs.dt = dt;
    inputs
}

/// **Verification against a closed form.** The Limbäck irradiation-creep rate
/// scales exactly as `φ^0.85` in the fast flux, and the full increment rises
/// monotonically with flux.
///
/// *Methodology.* Two parts.
/// (a) *Rate scaling.* Evaluate
/// [`CreepModel::rate_and_derivative`] directly at a fixed von Mises stress of
/// 1 MPa, T = 570 K, SRA cladding, with dt = 0 so the primary-creep transient
/// is excluded and only the secondary (irradiation + thermal) rate remains.
/// Those conditions are chosen to make the thermal term negligible — it is
/// 2.03e-16 /s against 1.97e-12 /s for irradiation creep, i.e. one part in
/// 10⁴ — so the ratio between fluxes of 4e18 and 1e18 n/m²/s must be the
/// correlation's own exponent, `4^0.85 = 3.24900959`. Pass criterion: 1e-3
/// relative — loose enough to absorb the residual thermal term, tight enough
/// to catch a wrong exponent (0.8 or 0.9 would be 12 % away).
/// (b) *Monotonicity.* The full one-day increment at fluxes 0, 1e18 and
/// 4e18 n/m²/s must be strictly increasing.
///
/// *Result.* (a) The measured rate ratio is 3.24877824 against 4^0.85 =
/// 3.24900959, agreeing to 7.1e-5 relative; the small deficit is exactly the
/// thermal term, which does not scale with flux. (b) Δε_c,eq over one day =
/// 8.96569e-06 at zero flux (thermal plus its primary transient), 5.01604e-05
/// at 1e18 n/m²/s and 7.53936e-05 at 4e18 n/m²/s — strictly increasing. Note
/// that the *increment* does not scale as φ^0.85: at one day the primary
/// transient supplies 97 % of it, and the driving stress relaxes as creep
/// proceeds. Both compress the spread, which is why the exponent is verified
/// on the rate and not on the increment.
#[test]
fn limback_irradiation_creep_scales_with_fast_flux() {
    let sra = CreepModel::Limback {
        clad_type: ZircaloyCladType::Sra,
    };
    let state = RheologyState::pristine();

    // (a) Rate scaling at a fixed low stress and temperature, where thermal
    // creep is negligible; primary transient excluded via dt = 0.
    let rate_at = |flux: f64| {
        let mut inputs = pwr_cladding_inputs(SymmTensor::ZERO, 0.0);
        inputs.material.temperature = 570.0;
        inputs.irradiation.fast_flux = flux;
        sra.rate_and_derivative(1.0e6, &inputs, &state).unwrap().0
    };
    let ratio = rate_at(4.0e18) / rate_at(1.0e18);
    let expected = 4.0f64.powf(0.85);
    assert!(
        (ratio - expected).abs() < 1.0e-3 * expected,
        "flux scaling {ratio} vs phi^0.85 = {expected}"
    );

    // (b) Monotonicity of the full one-day increment.
    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: sra,
    };
    let mut increments = Vec::new();
    for flux in [0.0, 1.0e18, 4.0e18] {
        let mut inputs = pwr_cladding_inputs(pure_shear(1.0e-4), 86_400.0);
        inputs.irradiation.fast_flux = flux;
        let out = law.correct(0, &inputs, &state).unwrap();
        increments.push(out.equivalent_creep_strain_increment);
    }
    assert!(increments[0] >= 0.0);
    assert!(increments[1] > increments[0], "{increments:?}");
    assert!(increments[2] > increments[1], "{increments:?}");
}

/// **Self-consistency check.** Thermal creep is strongly temperature
/// activated; irradiation creep is not.
///
/// *Methodology.* [`CreepModel::rate_and_derivative`] at a fixed 100 MPa von
/// Mises stress with dt = 0 (so only the secondary rate is seen), SRA cladding,
/// at 620 K and 720 K. The thermal term is isolated by evaluating at zero flux;
/// the irradiation term is isolated by subtracting that zero-flux rate from the
/// rate at 1e18 n/m²/s. Reference: none — this checks the sign and rough
/// magnitude of the `exp(−Q/RT)` factor with Q = 201 kJ/mol, whose Arrhenius
/// factor alone predicts `exp(−Q/R·(1/720 − 1/620)) = 224.9`. Pass criteria:
/// the thermal rate rises by at least a factor of 50 between 620 K and 720 K,
/// while the irradiation term changes by less than 1e-9 relative.
///
/// *Result.* Thermal-only rate 6.19516e-11 /s at 620 K and 1.31040e-08 /s at
/// 720 K, a factor of 211.5 — just short of the 224.9 the Arrhenius factor
/// alone gives, the difference coming from the temperature dependence of the
/// modulus `E = 1.148e5 − 59.9 T`, which enters both the `E/T` prefactor and
/// the `sinh` argument. The irradiation term is 1.971430e-10 /s at *both*
/// temperatures, differing by 2.1e-15 relative (floating-point round-off): as
/// implemented it contains no temperature at all, which is the defining feature
/// of irradiation creep and the reason cold cladding creeps down at all.
#[test]
fn limback_thermal_creep_is_temperature_activated() {
    let sra = CreepModel::Limback {
        clad_type: ZircaloyCladType::Sra,
    };
    let state = RheologyState::pristine();

    let rate_at = |temperature: f64, flux: f64| {
        let mut inputs = pwr_cladding_inputs(SymmTensor::ZERO, 0.0);
        inputs.material.temperature = temperature;
        inputs.irradiation.fast_flux = flux;
        sra.rate_and_derivative(100.0e6, &inputs, &state).unwrap().0
    };

    let thermal_cold = rate_at(620.0, 0.0);
    let thermal_hot = rate_at(720.0, 0.0);
    assert!(
        thermal_hot > 50.0 * thermal_cold,
        "720 K thermal rate {thermal_hot} is not strongly activated over 620 K {thermal_cold}"
    );

    let irradiation_cold = rate_at(620.0, 1.0e18) - thermal_cold;
    let irradiation_hot = rate_at(720.0, 1.0e18) - thermal_hot;
    assert!(
        (irradiation_hot - irradiation_cold).abs() < 1.0e-9 * irradiation_cold,
        "irradiation creep must be athermal: {irradiation_cold} -> {irradiation_hot}"
    );
}

/// **Self-consistency check.** The Limbäck constants match the upstream source.
///
/// *Methodology.* A provenance check, not a physics one: compare the constants
/// this port returns for each heat treatment against the literal values in
/// upstream `LimbackCreepModel.C` (constructor initialiser list and the
/// `cladType` branches). Reference: that file at commit 80e8445. Pass
/// criterion: exact equality.
///
/// *Result.* All four heat treatments match: SRA (A = 1.08e9, Q = 201 kJ/mol,
/// n = 2.0, C0 = 3.557e-24), RXA (5.47e8, 198 kJ/mol, 3.5, 1.654e-24), PRA
/// (7.06e8, 199 kJ/mol, 2.3, 2.714e-24), ZIRLO (8.64e8, 201 kJ/mol, 2.846e-24
/// with a stress-dependent exponent).
#[test]
fn limback_constants_match_upstream() {
    use ZircaloyCladType::{Pra, Rxa, Sra, Zirlo};

    assert_eq!(Sra.thermal_prefactor(), 1.08e9);
    assert_eq!(Sra.activation_energy(), 201e3);
    assert_eq!(Sra.stress_exponent(0.0), 2.0);
    assert_eq!(Sra.irradiation_coefficient(), 3.557e-24);

    assert_eq!(Rxa.thermal_prefactor(), 5.47e8);
    assert_eq!(Rxa.activation_energy(), 198e3);
    assert_eq!(Rxa.stress_exponent(0.0), 3.5);
    assert_eq!(Rxa.irradiation_coefficient(), 1.654e-24);

    assert_eq!(Pra.thermal_prefactor(), 7.06e8);
    assert_eq!(Pra.activation_energy(), 199e3);
    assert_eq!(Pra.stress_exponent(0.0), 2.3);
    assert_eq!(Pra.irradiation_coefficient(), 2.714e-24);

    assert_eq!(Zirlo.thermal_prefactor(), 8.64e8);
    assert_eq!(Zirlo.activation_energy(), 201e3);
    assert_eq!(Zirlo.irradiation_coefficient(), 2.846e-24);
    // ZIRLO's exponent is piecewise in stress.
    assert_eq!(Zirlo.stress_exponent(100.0e6), 2.0);
    assert_eq!(Zirlo.stress_exponent(300.0e6), 2.6);
    assert!((Zirlo.stress_exponent(500.0e6) - (1.2667 + 3.333e-3 * 500.0)).abs() < 1.0e-12);
}

/// **Self-consistency check.** The Limbäck modulus fit is refused where it
/// turns negative.
///
/// *Methodology.* Evaluate the Limbäck rate at T = 2000 K, above the
/// T = 1916 K root of `E = 1.148e5 − 59.9 T`. Reference: none — the criterion
/// is that a meaningless expression is reported rather than evaluated. Pass
/// criterion: [`OffbeatError::OutOfRange`] naming the modulus.
///
/// *Result.* `OutOfRange { value: 2000.0, low: 0.0, high: 1916.0, unit: "K" }`.
/// Note that the correlation stops being *physically* meaningful far earlier,
/// around 1100 K where Zircaloy begins its α→β transformation; that softer
/// limit is documented on [`CreepModel::Limback`] but not enforced, because
/// enforcing it would abort a whole-rod transient over a handful of cells.
#[test]
fn limback_refuses_a_temperature_where_its_modulus_fit_is_negative() {
    let mut inputs = pwr_cladding_inputs(pure_shear(1.0e-4), 3600.0);
    inputs.material.temperature = 2000.0;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: CreepModel::Limback {
            clad_type: ZircaloyCladType::Sra,
        },
    };
    match law.correct(0, &inputs, &RheologyState::pristine()) {
        Err(OffbeatError::OutOfRange { value, high, .. }) => {
            assert_eq!(value, 2000.0);
            assert_eq!(high, 1916.0);
        }
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// MATPRO fuel creep
// ---------------------------------------------------------------------------

/// Typical LWR fuel-pellet conditions: 1200 K, 95 % of theoretical density,
/// 5 µm grain radius, 1e19 fissions/m³/s.
fn fuel_inputs(strain: SymmTensor, dt: f64) -> RheologyInputs {
    let mut material = MaterialState::fresh(1200.0);
    material.porosity = 0.05;
    let mut inputs = RheologyInputs::quasi_static(elastic(), strain, material);
    inputs.irradiation = IrradiationState {
        fast_flux: 0.0,
        fission_rate: 1.0e19,
        grain_radius: 5.0e-6,
    };
    inputs.dt = dt;
    inputs
}

/// **Self-consistency check.** MATPRO fuel creep increases with temperature,
/// with fission rate and with decreasing grain size.
///
/// *Methodology.* Pure shear ε_xy = 1e-4 (trial von Mises stress 13.32347 MPa)
/// over one day at the reference fuel conditions, varying one input at a time:
/// temperature 1200 K → 1500 K, fission rate 1e19 → 2e19 /m³/s, grain radius
/// 5 µm → 2.5 µm. Reference: none — these are the qualitative dependences the
/// correlation is built from (Arrhenius activation, fission enhancement,
/// 1/G² diffusional scaling). Pass criterion: each perturbation strictly
/// increases the creep increment.
///
/// *Result.* Baseline Δε_c,eq = 8.40524e-06, converged in 3 Newton iterations.
/// Raising T to 1500 K gives 7.52554e-05 (×8.95); doubling the fission rate
/// gives 1.56511e-05 (×1.86); halving the grain radius gives 8.68607e-06
/// (×1.03). All three increased, as required.
///
/// The last two factors are worth reading carefully, because they are *not* the
/// naive ×2 and ×4 a reader might expect. At these conditions the athermal
/// Sakai term `A7·F·σ`, which carries no grain-size dependence at all, supplies
/// most of the rate; so doubling the fission rate falls short of doubling the
/// total (the diffusional term's fission enhancement `A1 + A2·F` is only partly
/// linear in F), and halving the grain radius moves only the small `1/G²`
/// term. A test asserting ×2 and ×4 would be asserting a model this
/// correlation does not have.
#[test]
fn matpro_fuel_creep_responds_to_temperature_fission_rate_and_grain_size() {
    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: CreepModel::Matpro {
            sakai_correction: true,
        },
    };
    let day = 86_400.0;
    let state = RheologyState::pristine();

    let base = law
        .correct(0, &fuel_inputs(pure_shear(1.0e-4), day), &state)
        .unwrap()
        .equivalent_creep_strain_increment;

    let mut hotter = fuel_inputs(pure_shear(1.0e-4), day);
    hotter.material.temperature = 1500.0;
    let hotter = law
        .correct(0, &hotter, &state)
        .unwrap()
        .equivalent_creep_strain_increment;

    let mut fissile = fuel_inputs(pure_shear(1.0e-4), day);
    fissile.irradiation.fission_rate = 2.0e19;
    let fissile = law
        .correct(0, &fissile, &state)
        .unwrap()
        .equivalent_creep_strain_increment;

    let mut fine_grained = fuel_inputs(pure_shear(1.0e-4), day);
    fine_grained.irradiation.grain_radius = 2.5e-6;
    let fine_grained = law
        .correct(0, &fine_grained, &state)
        .unwrap()
        .equivalent_creep_strain_increment;

    assert!(base > 0.0);
    assert!(hotter > base, "{hotter} should exceed {base}");
    assert!(fissile > base, "{fissile} should exceed {base}");
    assert!(fine_grained > base, "{fine_grained} should exceed {base}");
}

/// **Self-consistency check.** MATPRO is refused below 90.5 % of theoretical
/// density, where its own denominators change sign.
///
/// *Methodology.* The reference fuel case with porosity raised to 0.12, i.e.
/// 88 % of theoretical density. The correlation's second term carries
/// `(D% − 90.5)` in a denominator, so it returns a *negative* creep rate there.
/// Reference: none; the criterion is that the invalid range is reported.
/// Pass criterion: [`OffbeatError::OutOfRange`] with `low == 90.5`.
///
/// *Result.* `OutOfRange { value: 88.0, low: 90.5, high: 100.0, unit: "% of
/// theoretical density" }`. Upstream does not guard this and would return a
/// negative creep rate, i.e. a material that spontaneously un-creeps.
#[test]
fn matpro_refuses_a_density_below_its_valid_range() {
    let mut inputs = fuel_inputs(pure_shear(1.0e-4), 86_400.0);
    inputs.material.porosity = 0.12;

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: CreepModel::Matpro {
            sakai_correction: true,
        },
    };
    match law.correct(0, &inputs, &RheologyState::pristine()) {
        Err(OffbeatError::OutOfRange { value, low, .. }) => {
            assert!((value - 88.0).abs() < 1.0e-9);
            assert_eq!(low, 90.5);
        }
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

/// **Self-consistency check.** An unstressed cell does not creep under MATPRO.
///
/// *Methodology.* The reference fuel conditions with a purely *hydrostatic*
/// mechanical strain (ε_xx = ε_yy = ε_zz = 1e-4), so the deviatoric — and hence
/// the von Mises — stress is exactly zero, over one day. Reference: none; the
/// criterion is that creep, being deviatoric flow driven by deviatoric stress,
/// must vanish. Pass criterion: an exactly zero creep increment.
///
/// *Result.* Δε_c,eq = 0 exactly. This is the test that upstream would fail:
/// `MatproCreepModel.C` clamps the effective stress with
/// `max(min(sigmaEff, 1e10), 1e5)`, so an unstressed cell is treated as
/// carrying 0.1 MPa and creeps indefinitely. That lower clamp is deliberately
/// not reproduced; see [`CreepModel::Matpro`].
#[test]
fn matpro_does_not_creep_an_unstressed_cell() {
    let strain = SymmTensor::from_diag(1.0e-4, 1.0e-4, 1.0e-4);
    let inputs = fuel_inputs(strain, 86_400.0);

    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 1.0e12 },
        creep: CreepModel::Matpro {
            sakai_correction: true,
        },
    };
    let out = law.correct(0, &inputs, &RheologyState::pristine()).unwrap();
    assert_eq!(out.equivalent_creep_strain_increment, 0.0);
    assert!(out.von_mises_stress() < 1.0e-6);
}

// ---------------------------------------------------------------------------
// Yield-stress models
// ---------------------------------------------------------------------------

/// **Verification against a closed form.** The hardening curve interpolates
/// linearly and is held flat outside its range.
///
/// *Methodology.* Curve (0, 300 MPa), (0.1, 400 MPa), (0.3, 420 MPa).
/// Reference: linear interpolation by hand. Pass criterion: 1 Pa absolute.
///
/// *Result.* σ_y(0.05) = 350.000000 MPa, σ_y(0.2) = 410.000000 MPa,
/// σ_y(−1) = 300 MPa (held flat below), σ_y(10) = 420 MPa (held flat above);
/// slopes 1.0 GPa, 100 MPa, and 0 outside. Holding flat rather than
/// extrapolating matters: a linear extrapolation of that last segment would
/// invent 1.4 GPa of strength at ε_p,eq = 10.
#[test]
fn hardening_curve_interpolates_linearly_and_holds_flat_outside() {
    let curve = HardeningCurve::new(vec![(0.0, 300.0e6), (0.1, 400.0e6), (0.3, 420.0e6)]).unwrap();

    assert!((curve.yield_stress(0.05) - 350.0e6).abs() < 1.0);
    assert!((curve.yield_stress(0.2) - 410.0e6).abs() < 1.0);
    assert!((curve.yield_stress(-1.0) - 300.0e6).abs() < 1.0);
    assert!((curve.yield_stress(10.0) - 420.0e6).abs() < 1.0);

    assert!((curve.slope(0.05) - 1.0e9).abs() < 1.0);
    assert!((curve.slope(0.2) - 100.0e6).abs() < 1.0);
    assert_eq!(curve.slope(10.0), 0.0);
}

/// **Self-consistency check.** A malformed hardening curve is rejected at
/// construction.
///
/// *Methodology.* Three malformed curves: empty, negative yield stress, and two
/// points at the same plastic strain. Reference: none. Pass criterion:
/// [`OffbeatError::Unphysical`] for each.
///
/// *Result.* All three rejected. Catching them here rather than per cell per
/// timestep is the point: a duplicated abscissa would otherwise give an
/// infinite hardening slope somewhere deep in a Newton iteration.
#[test]
fn malformed_hardening_curves_are_rejected() {
    assert!(matches!(
        HardeningCurve::new(vec![]),
        Err(OffbeatError::Unphysical { .. })
    ));
    assert!(matches!(
        HardeningCurve::new(vec![(0.0, -1.0)]),
        Err(OffbeatError::Unphysical { .. })
    ));
    assert!(matches!(
        HardeningCurve::new(vec![(0.1, 300.0e6), (0.1, 400.0e6)]),
        Err(OffbeatError::Unphysical { .. })
    ));
}

/// **Self-consistency check.** FRAPTRAN Zircaloy: irradiation hardens, heat
/// softens, and yielding hardens.
///
/// *Methodology.* [`YieldStressModel::Fraptran`] with E = 100 GPa, evaluated at
/// zero plastic strain for (T = 600 K, Φ = 0), (T = 600 K, Φ = 5e25 n/m²) and
/// (T = 900 K, Φ = 0); then at ε_p,eq = 0.01 for the unirradiated 600 K case.
/// Reference: none — this checks the sign of each dependence the correlation
/// encodes, not its calibration. Pass criteria: irradiated > unirradiated;
/// hotter < cooler; hardened ≥ virgin; and every value inside a sanity band of
/// 30–1500 MPa for Zircaloy.
///
/// *Result.* σ_y = 2.628498e+08 Pa (262.85 MPa) unirradiated at 600 K;
/// 4.841876e+08 Pa (484.19 MPa) at Φ = 5e25 n/m², i.e. irradiation hardening by
/// ×1.842, the right order for LWR cladding; 5.019579e+07 Pa (50.20 MPa) at
/// 900 K (thermal softening); and 3.099774e+08 Pa (309.98 MPa) at
/// ε_p,eq = 0.01 unirradiated (work hardening). All inside the sanity band,
/// though the 900 K value sits near its lower edge.
///
/// *Caveat.* These are the port's own numbers, not values traced to a FRAPTRAN
/// document; the band check is a sanity screen, not a validation.
#[test]
fn fraptran_yield_stress_hardens_with_fluence_and_softens_with_temperature() {
    let model = YieldStressModel::Fraptran;
    let strain = pure_shear(0.0);

    let evaluate = |temperature: f64, fluence: f64, eq_plastic: f64| {
        let mut material = MaterialState::fresh(temperature);
        material.fast_fluence = fluence;
        let inputs = RheologyInputs::quasi_static(elastic(), strain, material);
        model.yield_stress(eq_plastic, &inputs).unwrap()
    };

    let virgin = evaluate(600.0, 0.0, 0.0);
    let irradiated = evaluate(600.0, 5.0e25, 0.0);
    let hot = evaluate(900.0, 0.0, 0.0);
    let hardened = evaluate(600.0, 0.0, 0.01);

    for value in [virgin, irradiated, hot, hardened] {
        assert!(
            (30.0e6..=1500.0e6).contains(&value),
            "{value} Pa is outside the Zircaloy sanity band"
        );
    }
    assert!(irradiated > virgin, "{irradiated} should exceed {virgin}");
    assert!(hot < virgin, "{hot} should be below {virgin}");
    assert!(hardened >= virgin, "{hardened} should be at least {virgin}");
}

/// **Self-consistency check.** FRAPTRAN is refused above the top of its
/// strength-coefficient fit.
///
/// *Methodology.* Evaluate at T = 2200 K, above the 2100 K upper bound of the
/// last `K(T)` branch. Reference: none. Pass criterion:
/// [`OffbeatError::OutOfRange`] with `high == 2100`.
///
/// *Result.* `OutOfRange { value: 2200.0, high: 2100.0, unit: "K" }`. Upstream
/// has no branch there either, but silently leaves whatever value the field
/// already held — which for a fresh field is zero, i.e. a material with no
/// strength at all.
#[test]
fn fraptran_refuses_a_temperature_above_its_fit() {
    let inputs =
        RheologyInputs::quasi_static(elastic(), SymmTensor::ZERO, MaterialState::fresh(2200.0));
    match YieldStressModel::Fraptran.yield_stress(0.0, &inputs) {
        Err(OffbeatError::OutOfRange { value, high, .. }) => {
            assert_eq!(value, 2200.0);
            assert_eq!(high, 2100.0);
        }
        other => panic!("expected OutOfRange, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// State bookkeeping and per-material dispatch
// ---------------------------------------------------------------------------

/// **Self-consistency check.** Committing an increment accumulates it exactly
/// once.
///
/// *Methodology.* Take one plastic step, commit it with
/// [`RheologyState::advance`], and compare the committed state against the
/// increment the step reported. Reference: none — this is the bookkeeping
/// contract. Pass criterion: exact equality on all six accumulators.
///
/// *Result.* Exact. The property matters because calling `advance` inside the
/// outer mechanics corrector loop instead of after it would double-count the
/// inelastic strain, which is the standard way a creep model silently
/// over-predicts.
#[test]
fn advancing_the_state_accumulates_the_increment_exactly_once() {
    let mut inputs = quasi_static(pure_shear(1.0e-2));
    inputs.dt = 3600.0;
    let law = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 300.0e6 },
        creep: linear_creep(0.01, 1.0),
    };

    let mut state = RheologyState::pristine();
    let out = law.correct(0, &inputs, &state).unwrap();
    state.advance(&out);

    assert_eq!(state.plastic_strain, out.plastic_strain_increment);
    assert_eq!(
        state.equivalent_plastic_strain,
        out.equivalent_plastic_strain_increment
    );
    assert_eq!(state.creep_strain, out.creep_strain_increment);
    assert_eq!(
        state.equivalent_creep_strain,
        out.equivalent_creep_strain_increment
    );
    assert_eq!(state.yield_stress, out.yield_stress);
    assert_eq!(
        state.inelastic_strain(),
        out.plastic_strain_increment + out.creep_strain_increment
    );
}

/// **Self-consistency check.** Per-material dispatch sends each cell to its own
/// law, and rejects a malformed zone map.
///
/// *Methodology.* Two zones — a purely elastic one and a perfectly plastic one
/// at σ_y = 300 MPa — over four cells mapped `[0, 0, 1, 1]`. Load all four with
/// pure shear ε_xy = 1e-2, far past yield. Reference: none. Pass criteria: the
/// elastic cells return the full elastic stress and the plastic cells return
/// 300 MPa; an out-of-range zone index and an empty law list are rejected at
/// construction; an out-of-range cell index is rejected at use.
///
/// *Result.* Cells 0–1: q = 1332.347 MPa (elastic). Cells 2–3: q = 300.00 MPa
/// (on the yield surface). Both malformed constructions and the bad cell index
/// return [`OffbeatError::Mesh`].
#[test]
fn per_material_dispatch_applies_the_right_law_to_each_cell() {
    let laws = vec![
        ConstitutiveLaw::Elastic,
        ConstitutiveLaw::perfectly_plastic(300.0e6),
    ];
    let rheology =
        RheologyByMaterial::new(laws, Arc::new(vec![0, 0, 1, 1])).expect("valid zone map");
    assert_eq!(rheology.n_materials(), 2);
    assert_eq!(rheology.n_cells(), 4);

    let inputs = quasi_static(pure_shear(1.0e-2));
    let state = RheologyState::pristine();

    for cell in 0..2 {
        let out = rheology.correct(cell, &inputs, &state).unwrap();
        assert!(!out.yielding);
        assert!(out.von_mises_stress() > 1.0e9);
    }
    for cell in 2..4 {
        let out = rheology.correct(cell, &inputs, &state).unwrap();
        assert!(out.yielding);
        assert!((out.von_mises_stress() - 300.0e6).abs() < 1.0);
    }

    assert!(matches!(
        rheology.correct(9, &inputs, &state),
        Err(OffbeatError::Mesh(_))
    ));
    assert!(matches!(
        RheologyByMaterial::new(vec![], Arc::new(vec![0])),
        Err(OffbeatError::Mesh(_))
    ));
    assert!(matches!(
        RheologyByMaterial::new(vec![ConstitutiveLaw::Elastic], Arc::new(vec![0, 3])),
        Err(OffbeatError::Mesh(_))
    ));
}

/// **Self-consistency check.** The [`Rheology`] enum dispatches to its single
/// registered driver.
///
/// *Methodology.* Wrap a uniform perfectly plastic [`RheologyByMaterial`] in
/// [`Rheology::ByMaterial`] and check it produces the same answer as calling
/// the inner driver directly. Reference: none. Pass criterion: identical
/// stress.
///
/// *Result.* Identical. The enum exists so that a second driver becomes a
/// compile error at every match site rather than a runtime branch; upstream
/// registers exactly one implementation today.
#[test]
fn rheology_enum_dispatches_to_the_by_material_driver() {
    let inner = RheologyByMaterial::uniform(ConstitutiveLaw::perfectly_plastic(300.0e6), 3)
        .expect("three cells");
    let wrapped = Rheology::ByMaterial(inner.clone());
    assert_eq!(wrapped.n_cells(), 3);

    let inputs = quasi_static(pure_shear(1.0e-2));
    let state = RheologyState::pristine();
    assert_eq!(
        wrapped.correct(1, &inputs, &state).unwrap().stress,
        inner.correct(1, &inputs, &state).unwrap().stress
    );
}

/// **Verification against a closed form.** The creep timestep control inverts
/// the increment limits exactly.
///
/// *Methodology.* Limits of 1e-4 (average) and 1e-3 (maximum) against a
/// previous step of 3600 s that produced a 2e-4 average and a 5e-3 maximum
/// increment. Reference: `dt = min(limit / rate)` evaluated by hand, i.e.
/// `min(1e-4/(2e-4/3600), 1e-3/(5e-3/3600)) = min(1800, 720) = 720 s`. Pass
/// criterion: 1e-9 s absolute.
///
/// *Result.* 720.000000 s, the maximum-increment limit binding as expected. An
/// unlimited control returns infinity, so a caller can `min` it against every
/// other physics module's suggestion without a special case.
#[test]
fn creep_time_step_control_inverts_the_increment_limits() {
    let control = CreepTimeStepControl {
        max_average_increment: 1.0e-4,
        max_maximum_increment: 1.0e-3,
    };
    let dt = control.next_time_step(2.0e-4, 5.0e-3, 3600.0);
    assert!((dt - 720.0).abs() < 1.0e-9, "measured {dt}");

    assert_eq!(
        CreepTimeStepControl::default().next_time_step(1.0e-3, 1.0e-2, 3600.0),
        f64::INFINITY
    );
    assert_eq!(control.next_time_step(1.0e-4, 1.0e-3, 0.0), f64::INFINITY);
}

/// **Self-consistency check.** Creep before plasticity, not the other way
/// round.
///
/// *Methodology.* One step of dt = 10 s at a strain (pure shear ε_xy = 1e-2)
/// whose trial von Mises stress, 1332.29 MPa, is far past a σ_y = 300 MPa yield
/// surface, with the linear creep law (η = 3.6e11 Pa·s) active. The step length
/// is chosen so that creep relaxes the trial stress to roughly 317 MPa — still
/// above yield, so plasticity is genuinely active in both cases and the
/// comparison is not trivially between "plastic" and "nothing". Compare the
/// plastic increment against the same step with creep switched off. Reference:
/// none — the ordering is a modelling decision, and this asserts its
/// consequence. Pass criteria: the plastic increment with creep active is
/// strictly smaller, and both cases land on the yield surface.
///
/// *Result.* Δε_p,eq = 8.94701e-03 without creep and 1.45934e-04 with creep, a
/// factor of 61.3 smaller; the creep step supplied Δε_c,eq = 8.80107e-03 of the
/// same total, relaxing almost all of the overstress before the return map saw
/// it. Both cases end on the 300 MPa yield surface to within 1 Pa. Reversing
/// the order would over-predict plastic strain, which matters because
/// cladding-failure criteria are written against plastic strain.
#[test]
fn creep_relaxes_stress_before_the_plastic_return_map_sees_it() {
    let mut inputs = quasi_static(pure_shear(1.0e-2));
    inputs.dt = 10.0;
    let state = RheologyState::pristine();

    let without = ConstitutiveLaw::perfectly_plastic(300.0e6)
        .correct(0, &inputs, &state)
        .unwrap();
    let with = ConstitutiveLaw::MisesPlasticCreep {
        yield_stress: YieldStressModel::Constant { sigma_y: 300.0e6 },
        creep: linear_creep(0.01, 1.0),
    }
    .correct(0, &inputs, &state)
    .unwrap();

    assert!(with.equivalent_creep_strain_increment > 0.0);
    assert!(
        with.equivalent_plastic_strain_increment < without.equivalent_plastic_strain_increment,
        "with creep {} should be below without creep {}",
        with.equivalent_plastic_strain_increment,
        without.equivalent_plastic_strain_increment
    );
    // Both still land on the yield surface.
    assert!((with.von_mises_stress() - 300.0e6).abs() < 1.0);
    assert!((without.von_mises_stress() - 300.0e6).abs() < 1.0);
}
