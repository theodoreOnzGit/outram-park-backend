// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the isotropic viscoplastic creep laws.
//!
//! # What is checkable here, and what is not
//!
//! There is no published closed-form solution for a general creep step, so
//! these tests lean on three things that *are* exact:
//!
//! 1. **The flow rules themselves**, evaluated against the upstream Fortran
//!    expression transcribed independently. This is code-equivalence
//!    verification of the port, not validation against experiment.
//! 2. **Structural relationships** that must hold if both laws are encoded
//!    correctly — above all that Norton is the `1/m → 0` limit of Lemaitre,
//!    which upstream implements as an explicit branch and which this port
//!    reproduces as two variants that must agree in the limit.
//! 3. **Closed-form solutions of special cases**, notably pure relaxation under
//!    fixed total strain with `n = 1`, which integrates analytically.
//!
//! Nothing here is validation against code_aster output or creep data.

use approx::assert_relative_eq;
use outram_foam_basic_lib::primitives::SymmTensor;

use super::*;

/// A representative Norton law: `K = 200 MPa`, `n = 5`.
fn norton() -> ViscoplasticLaw {
    ViscoplasticLaw::Norton(NortonParameters { k: 200.0e6, n: 5.0 })
}

/// Uniaxial stress state of magnitude `s` along x.
///
/// Chosen because its von Mises equivalent is exactly `s`, so the equivalent
/// stress can be reasoned about by hand.
fn uniaxial(s: f64) -> SymmTensor {
    SymmTensor::new(s, 0.0, 0.0, 0.0, 0.0, 0.0)
}

/// **The von Mises norm matches upstream's `lcnrts`.**
///
/// *Methodology:* upstream computes `sqrt(1.5 * d·d)` over the Mandel
/// six-vector, and because of the `√2` shear scaling that dot product equals
/// the tensor double contraction `s:s`. Check the implementation against an
/// independently evaluated `sqrt(1.5 * s:s)` on a general deviator, and against
/// the hand-known result that a uniaxial stress has equivalent equal to its own
/// magnitude. Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* uniaxial 250 MPa gives `σ_eq = 250 MPa`
/// exactly; the general deviator agrees with `sqrt(1.5 s:s)` to 0.0 relative.
/// Interpretation: the `3/2` factor and the Mandel-consistent contraction are
/// both right — a shear-only state would come out wrong by `√3` if the
/// contraction convention were mismatched.
#[test]
fn the_von_mises_norm_matches_upstream() {
    let s = uniaxial(250.0e6);
    assert_relative_eq!(
        von_mises_of_deviator(deviator(s)),
        250.0e6,
        max_relative = 1e-12
    );

    let general = deviator(SymmTensor::new(120e6, 11e6, -23e6, -45e6, 37e6, 8e6));
    let independent = (1.5 * general.double_inner(general)).sqrt();
    assert_relative_eq!(
        von_mises_of_deviator(general),
        independent,
        max_relative = 1e-12
    );
}

/// **A pure shear state has the equivalent stress `√3 τ`.**
///
/// *Methodology:* the textbook result for von Mises. This is the case a wrong
/// contraction convention gets wrong while uniaxial still passes, so it earns
/// its own test. Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* `τ = 100 MPa` gives `σ_eq = 173.205 MPa`
/// against `√3 × 100 = 173.205 MPa`, agreeing to 0.0 relative.
#[test]
fn pure_shear_gives_root_three_tau() {
    let tau = 100.0e6;
    let s = SymmTensor::new(0.0, tau, 0.0, 0.0, 0.0, 0.0);
    assert_relative_eq!(
        von_mises_of_deviator(deviator(s)),
        3.0_f64.sqrt() * tau,
        max_relative = 1e-12
    );
}

/// **The Norton rate matches the upstream expression.**
///
/// *Methodology:* upstream's `norton.F90` computes `dp = (grj2v * unsurk)**n`
/// with `unsurk = 1/K`. Evaluate that expression independently and compare.
/// Pass criterion: 1e-12 relative. Code-equivalence verification, not
/// validation.
///
/// *Result (measured 2026-08-05):* at `σ_eq = 250 MPa`, `K = 200 MPa`, `n = 5`,
/// the rate is **3.0517578125e0 /s**, matching `(1.25)^5 = 3.0517578125`
/// exactly. Interpretation: the power law and the `1/K` inversion are both
/// correct.
#[test]
fn the_norton_rate_matches_the_upstream_expression() {
    let law = norton();
    let sigma_eq = 250.0e6;
    let rate = law.equivalent_strain_rate(sigma_eq, 0.0);

    let unsurk = 1.0 / 200.0e6;
    let expected = (sigma_eq * unsurk).powf(5.0);
    assert_relative_eq!(rate, expected, max_relative = 1e-12);
    assert_relative_eq!(rate, 1.25_f64.powi(5), max_relative = 1e-12);
}

/// **Norton is the `1/m → 0` limit of Lemaitre.**
///
/// *Methodology:* upstream's `ggplem.F90` branches on `unsurm == 0` and falls
/// back to the pure power law, so the two laws are one law with a switch. This
/// port keeps them as separate variants, which means the relationship is now
/// something that *could* silently break — so it is asserted. Take a Lemaitre
/// law with the same `K` and `n` and let `m` grow; its rate must approach the
/// Norton rate at fixed `p`. Pass criterion: within 0.1% at `m = 1e6`.
///
/// *Result (measured 2026-08-05):* at `σ_eq = 250 MPa`, `p = 0.01`:
///
/// | `m` | Lemaitre rate | ratio to Norton |
/// |---|---|---|
/// | 1e2 | 3.8419 | 1.2589 |
/// | 1e4 | 3.0588 | 1.0023 |
/// | 1e6 | 3.0518 | 1.0000 |
///
/// converging to Norton's 3.0517578125. Interpretation: both laws are encoded
/// consistently, and the hardening exponent enters with the sign that makes
/// large `m` mean weak hardening.
#[test]
fn norton_is_the_weak_hardening_limit_of_lemaitre() {
    let n = 5.0;
    let k = 200.0e6;
    let sigma_eq = 250.0e6;
    let p = 0.01;

    let norton_rate =
        ViscoplasticLaw::Norton(NortonParameters { k, n }).equivalent_strain_rate(sigma_eq, p);

    let far = ViscoplasticLaw::Lemaitre(LemaitreParameters { k, n, m: 1.0e6 })
        .equivalent_strain_rate(sigma_eq, p);

    assert_relative_eq!(far, norton_rate, max_relative = 1e-3);
}

/// **Lemaitre hardening slows the flow as strain accumulates.**
///
/// *Methodology:* the `p^(-n/m)` factor has a negative exponent, so more
/// accumulated strain must mean a *lower* rate at the same stress. Getting this
/// sign wrong turns a decaying primary transient into a runaway, and the
/// resulting creep curve still looks superficially plausible. Sweep `p` upward
/// and require the rate to fall monotonically. Pass criterion: strictly
/// decreasing.
///
/// *Result (measured 2026-08-05):* at `σ_eq = 250 MPa`, `K = 200 MPa`, `n = 5`,
/// `m = 10`, rates of **96.5051, 30.5176, 9.6505, 3.0518 /s** at
/// `p = 1e-3, 1e-2, 1e-1, 1`. Each decade of accumulated strain multiplies the
/// rate by 0.3162, which is `10^(-n/m) = 10^(-0.5)` — the hardening exponent
/// recovered from the measured data. Note the rate at `p = 1e-2` is exactly
/// Norton's 30.5176/10... in fact it is 10x the Norton rate of 3.0518, because
/// `p^(-0.5) = 10` there; the two laws coincide only as `m` grows, not at any
/// particular `p`.
#[test]
fn lemaitre_hardening_slows_the_flow() {
    let law = ViscoplasticLaw::Lemaitre(LemaitreParameters {
        k: 200.0e6,
        n: 5.0,
        m: 10.0,
    });
    let sigma_eq = 250.0e6;

    let rates: Vec<f64> = [1.0e-3, 1.0e-2, 1.0e-1, 1.0]
        .iter()
        .map(|p| law.equivalent_strain_rate(sigma_eq, *p))
        .collect();

    for pair in rates.windows(2) {
        assert!(
            pair[1] < pair[0],
            "hardening must reduce the rate: {rates:?}"
        );
    }

    // Each decade of p should scale the rate by 10^(-n/m).
    let expected_ratio = 10.0_f64.powf(-5.0 / 10.0);
    for pair in rates.windows(2) {
        assert_relative_eq!(pair[1] / pair[0], expected_ratio, max_relative = 1e-12);
    }
}

/// **An unloaded or untimed step produces no creep.**
///
/// *Methodology:* upstream guards with `grj2v > r8miem()` before flowing. Zero
/// stress, zero timestep, and a purely hydrostatic stress (whose deviator
/// vanishes) must all give exactly zero creep. The hydrostatic case matters
/// physically: creep is deviatoric, so pressure alone cannot drive it.
///
/// *Result (measured 2026-08-05):* all three give `Δp = 0` exactly, and the
/// hydrostatic case returns the stress unchanged.
#[test]
fn no_stress_no_time_and_no_deviator_all_give_no_creep() {
    let law = norton();

    let zero = law.integrate(uniaxial(0.0), 80.0e9, 0.0, 1.0).unwrap();
    assert_eq!(zero.equivalent_increment, 0.0);

    let untimed = law.integrate(uniaxial(250.0e6), 80.0e9, 0.0, 0.0).unwrap();
    assert_eq!(untimed.equivalent_increment, 0.0);

    let pressure = SymmTensor::new(-100e6, 0.0, 0.0, -100e6, 0.0, -100e6);
    let hydrostatic = law.integrate(pressure, 80.0e9, 0.0, 1.0).unwrap();
    assert_eq!(hydrostatic.equivalent_increment, 0.0);
    assert_relative_eq!(hydrostatic.stress.xx, -100e6, max_relative = 1e-12);
}

/// **Creep is volume-preserving.**
///
/// *Methodology:* viscoplastic flow is isochoric — the creep strain increment
/// must be deviatoric, so its trace vanishes. A non-zero trace would mean the
/// material was creeping in volume, which no metal does and no von Mises law
/// permits. Pass criterion: `|tr(Δε)| / ‖Δε‖` below 1e-14.
///
/// *Result (measured 2026-08-05):* relative trace 1.304e-16 on a general
/// multiaxial stress state — machine precision.
#[test]
fn creep_is_volume_preserving() {
    let law = norton();
    let stress = SymmTensor::new(300e6, 40e6, -20e6, 120e6, 15e6, -80e6);
    let out = law.integrate(stress, 80.0e9, 0.0, 1.0e3).unwrap();

    let relative_trace = out.strain_increment.tr().abs() / out.strain_increment.mag();
    assert!(
        relative_trace < 1e-14,
        "creep strain not deviatoric: relative trace {relative_trace:e}"
    );
}

/// **The integrated step satisfies its own constitutive statement.**
///
/// *Methodology:* the strongest available self-check. Whatever `Δp` the solver
/// returned must satisfy `Δp = Δt · ṗ(σ_eq, p_old + Δp)` with `σ_eq` the
/// *relaxed* equivalent stress, since that is the equation being solved.
/// Verifying it independently catches a solver that converged on the wrong
/// residual. Pass criterion: 1e-9 relative.
///
/// *Result (measured 2026-08-05):* for `σ_trial = 250 MPa`, `μ = 80 GPa`,
/// `Δt = 1e3 s`, the solve returned `Δp = 9.8920e-4` in 16 iterations, with a
/// relaxed `σ_eq = 1.2592e7 Pa`; the constitutive residual is 1.193e-17, i.e.
/// 1.206e-14 relative to `Δp`. Note how far the stress relaxed — from 250 MPa
/// to 12.6 MPa, a factor of 20 — which is what a stress exponent of 5 does over
/// a long step, and is exactly the regime where an explicit integration would
/// have gone unstable.
#[test]
fn the_integrated_step_satisfies_its_constitutive_statement() {
    let law = norton();
    let mu = 80.0e9;
    let dt = 1.0e3;
    let p_old = 0.0;

    let out = law.integrate(uniaxial(250.0e6), mu, p_old, dt).unwrap();

    let implied =
        dt * law.equivalent_strain_rate(out.equivalent_stress, p_old + out.equivalent_increment);
    let residual = (implied - out.equivalent_increment).abs();
    assert!(
        residual < 1e-9 * out.equivalent_increment.max(1e-30),
        "constitutive residual {residual:e} against Dp {:e}",
        out.equivalent_increment
    );
}

/// **The relaxed stress follows the radial-return relation.**
///
/// *Methodology:* radial return means the deviator shrinks without rotating, so
/// `σ_eq = σ_eq_trial - 3μ Δp` must hold exactly, and the relaxed deviator must
/// stay parallel to the trial deviator. Check both. Pass criterion: 1e-10
/// relative on the magnitude, and the normalised tensors equal to 1e-12.
///
/// *Result (measured 2026-08-05):* `σ_eq = 1.341471e7 Pa` against the
/// relation's `1.341471e7 Pa`, agreeing to 0.0 relative — the relation is
/// satisfied identically because it is how the relaxed stress is constructed.
/// The relaxed and trial deviators are parallel to within 1e-12 component-wise
/// after normalisation.
#[test]
fn the_relaxed_stress_follows_radial_return() {
    let law = norton();
    let mu = 80.0e9;
    let trial = SymmTensor::new(300e6, 40e6, -20e6, 120e6, 15e6, -80e6);

    let s_trial = deviator(trial);
    let eq_trial = von_mises_of_deviator(s_trial);

    let out = law.integrate(trial, mu, 0.0, 1.0e3).unwrap();
    let expected_eq = eq_trial - 3.0 * mu * out.equivalent_increment;
    assert_relative_eq!(out.equivalent_stress, expected_eq, max_relative = 1e-10);

    // Parallel: the normalised deviators must coincide.
    let s_new = deviator(out.stress);
    let (a, b) = (s_trial.mag(), s_new.mag());
    for (x, y) in [
        (s_trial.xx / a, s_new.xx / b),
        (s_trial.xy / a, s_new.xy / b),
        (s_trial.xz / a, s_new.xz / b),
        (s_trial.yy / a, s_new.yy / b),
        (s_trial.yz / a, s_new.yz / b),
        (s_trial.zz / a, s_new.zz / b),
    ] {
        assert_relative_eq!(x, y, max_relative = 1e-12);
    }
}

/// **Linear creep relaxes exponentially, matching the closed form.**
///
/// *Methodology:* the one case that integrates analytically. With `n = 1` the
/// Norton rate is `ṗ = σ_eq / K`, and under fixed total strain the relaxation
/// obeys `dσ/dt = -3μ σ / K`, whose solution is
///
/// `σ(t) = σ₀ exp(-3μ t / K)`
///
/// Step forward in small increments, re-using the relaxed stress as the next
/// trial, and compare against that exponential. Pass criterion: 1% relative
/// after a full relaxation time, with the error falling as the step shrinks.
///
/// *Result (measured 2026-08-05):* over `t = K/(3μ)` (one time constant), the
/// closed form is **9.1970e7 Pa**; the 200-step integration gives 9.2199e7 Pa
/// (relative error **2.4948e-3**) and the 400-step 9.2085e7 Pa (**1.2487e-3**).
/// The error halves when the step halves — first-order convergence, correct for
/// the backward-Euler integration used here. Interpretation: the integration is
/// consistent and converges to the analytic solution at the expected order;
/// the residual error is time discretisation, not a modelling mistake.
#[test]
fn linear_creep_relaxes_exponentially() {
    let k = 200.0e6;
    let mu = 80.0e9;
    let law = ViscoplasticLaw::Norton(NortonParameters { k, n: 1.0 });

    let sigma0 = 250.0e6;
    let tau = k / (3.0 * mu); // relaxation time constant
    let total = tau;

    let integrate_with = |steps: usize| -> f64 {
        let dt = total / steps as f64;
        let mut stress = uniaxial(sigma0);
        let mut p = 0.0;
        for _ in 0..steps {
            let out = law.integrate(stress, mu, p, dt).unwrap();
            stress = out.stress;
            p += out.equivalent_increment;
        }
        von_mises_of_deviator(deviator(stress))
    };

    let closed_form = sigma0 * (-total / tau).exp();

    let coarse = integrate_with(200);
    let fine = integrate_with(400);

    let err_coarse = (coarse - closed_form).abs() / closed_form;
    let err_fine = (fine - closed_form).abs() / closed_form;

    assert!(
        err_coarse < 1.0e-2,
        "200-step relative error {err_coarse:e} against the closed form"
    );
    assert!(
        err_fine < err_coarse * 0.75,
        "halving the step should roughly halve the error: {err_coarse:e} -> {err_fine:e}"
    );
}

/// **Unphysical inputs are rejected.**
///
/// *Methodology:* a non-positive shear modulus makes the radial-return bracket
/// meaningless and a negative timestep would run creep backwards. Both are
/// caller errors that must be reported rather than absorbed.
///
/// *Result (measured 2026-08-05):* both rejected.
#[test]
fn unphysical_inputs_are_rejected() {
    let law = norton();
    assert!(law.integrate(uniaxial(1e8), 0.0, 0.0, 1.0).is_err());
    assert!(law.integrate(uniaxial(1e8), 80.0e9, 0.0, -1.0).is_err());
}

/// **The ASTER names are preserved verbatim.**
///
/// *Methodology:* section 4 of the port scoping document requires the upstream
/// behaviour name to stay searchable, because it is what a code_aster user
/// types in a deck and what the literature cites.
///
/// *Result (measured 2026-08-05):* as expected.
#[test]
fn the_aster_names_are_preserved() {
    assert_eq!(norton().aster_name(), "NORTON");
    assert_eq!(
        ViscoplasticLaw::Lemaitre(LemaitreParameters {
            k: 1.0,
            n: 1.0,
            m: 1.0
        })
        .aster_name(),
        "LEMAITRE"
    );
}
