// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification tests for the `VISCOCHAB` port.
//!
//! Every test here is *verification* — an independent check that the
//! transcription of `rkdcha.F90` is faithful and self-consistent. None is
//! *validation*: nothing is compared against code_aster output or against a
//! measured creep-fatigue curve, and no agreement with either is claimed.
//!
//! Measured numbers quoted in the doc comments were **printed by the tests
//! themselves** and transcribed, on 2026-08-05, from
//! `cargo test -p outram-park-fork-offbeat --lib --release
//! rheology::aster::viscochab -- --nocapture`.

use super::*;
use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem};

/// `sqrt(1.5)`, the same constant the port uses.
const SQRT_1_5: f64 = 1.224_744_871_391_589_0;

/// A deliberately synthetic coefficient set — **not** a real material.
///
/// Chosen so that every mechanism is switched on and none dominates:
/// millisecond-scale flow at ~150 MPa, both back stresses active, isotropic
/// hardening saturating, and the memory surface able to grow.
fn exercise_parameters() -> ViscoplasticChabocheParameters {
    ViscoplasticChabocheParameters::from_aster_coefficients([
        100.0e6, // K_0
        0.2,     // A_K
        1.0,     // A_R  (unused by the explicit path)
        50.0e6,  // K
        5.0,     // N
        0.0,     // ALP
        20.0,    // B
        2.0,     // M_R
        0.0,     // G_R
        10.0,    // MU
        60.0e6,  // Q_M
        10.0e6,  // Q_0
        5.0e6,   // QR_0
        0.5,     // ETA
        150.0e9, // C1
        2.0,     // M_1
        0.5,     // D1
        0.0,     // G_X1
        2000.0,  // G1_0
        20.0e9,  // C2
        2.0,     // M_2
        0.5,     // D2
        0.0,     // G_X2
        200.0,   // G2_0
        0.3,     // A_I
    ])
}

/// Coefficients with every mechanism except the first back stress switched off,
/// so that `X₁` has an exact Armstrong-Frederick saturation limit `C1/γ₁`.
///
/// `A_I = 1` freezes `γ₁ = G1_0`; `B = G_R = 0` and `MU = 0` freeze `R` at
/// zero; `G_X1 = G_X2 = 0` remove static recovery; `C2 = 0` decouples the
/// second back stress from the stress.
fn saturation_parameters(c1: f64, gamma1: f64) -> ViscoplasticChabocheParameters {
    ViscoplasticChabocheParameters::from_aster_coefficients([
        10.0e6, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0e6, 0.0, 0.0, 1.0, c1, 1.0, 0.5,
        0.0, gamma1, 0.0, 1.0, 0.5, 0.0, 0.0, 1.0,
    ])
}

/// A uniaxial Mandel stress with only `σ_xx` non-zero.
fn uniaxial_stress(sigma_xx: f64) -> AsterVoigt {
    AsterVoigt::from_components([sigma_xx, 0.0, 0.0, 0.0, 0.0, 0.0])
}

/// A 27-equation system that holds the stress **fixed**, for stress-controlled
/// verification. Owned by value, so [`OdeIntegrator::typed`] stores it without
/// a lifetime.
#[derive(Debug, Clone, Copy)]
struct FixedStressSystem {
    law: ViscoplasticChabocheWithMemory,
    stress: AsterVoigt,
}

impl OdeSystem for FixedStressSystem {
    fn n_eqns(&self) -> usize {
        ODE_EQUATION_COUNT
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        let state = ViscoplasticChabocheState::from_ode_state(y);
        let rates = self.law.internal_variable_rates(self.stress, &state);
        if dydx.len() < ODE_EQUATION_COUNT {
            dydx.resize(ODE_EQUATION_COUNT, 0.0);
        }
        rates.write_ode_derivatives(dydx);
    }
}

/// Von Mises equivalent of a Mandel six-vector, `√(3/2 · t:t)`.
fn von_mises(t: AsterVoigt) -> f64 {
    (1.5 * t.dot(t)).sqrt()
}

/// Deviator of a Mandel six-vector.
fn deviator(t: AsterVoigt) -> AsterVoigt {
    let c = t.components();
    let mean = (c[0] + c[1] + c[2]) / 3.0;
    AsterVoigt::from_components([c[0] - mean, c[1] - mean, c[2] - mean, c[3], c[4], c[5]])
}

// ── Parameter map ────────────────────────────────────────────────────────────

/// **Methodology.** `cvmmat.F90` names 28 material keywords in `nomc(1..28)`
/// and fills `materf(1..25, 2)` from `nomc(4..28)` via
/// `call rcvalt(..., 25, materf(1,2), cerr(4), 2)`; `rkdcha.F90` then reads
/// those 25 slots as `coeft(1..25)`. This test re-transcribes `nomc(4..28)`
/// from upstream independently of [`ASTER_COEFFICIENT_NAMES`] and requires the
/// two to agree element by element, then round-trips a coefficient array
/// through [`ViscoplasticChabocheParameters`] and requires bit equality. Pass
/// criterion: both comparisons exact.
///
/// This also re-checks the specific slots `rkdcha.F90` reads by name —
/// `k0 = coeft(1)`, `ak = coeft(2)`, `k = coeft(4)`, … `ai = coeft(25)` — which
/// is where a one-off slip would land. Note `coeft(3)` (`A_R`) is read by
/// `cvmcvx.F90` but **not** by `rkdcha.F90`.
///
/// **Results (2026-08-05).** All 25 names matched. The round-trip returned the
/// input array bit-for-bit (`1.0, 2.0, …, 25.0`), and the named-slot spot
/// checks printed `K_0 = 1, A_K = 2, K = 4, N = 5, C1 = 15, D1 = 17,
/// C2 = 20, D2 = 22, A_I = 25`, matching `rkdcha.F90` lines 45-70 exactly.
#[test]
fn aster_coefficient_map_matches_cvmmat() {
    // Transcribed from cvmmat.F90 nomc(4) .. nomc(28).
    let upstream_nomc_4_to_28 = [
        "K_0", "A_K", "A_R", "K", "N", "ALP", "B", "M_R", "G_R", "MU", "Q_M", "Q_0", "QR_0", "ETA",
        "C1", "M_1", "D1", "G_X1", "G1_0", "C2", "M_2", "D2", "G_X2", "G2_0", "A_I",
    ];
    assert_eq!(ASTER_COEFFICIENT_NAMES, upstream_nomc_4_to_28);

    let coeft: [f64; 25] = std::array::from_fn(|i| (i + 1) as f64);
    let p = ViscoplasticChabocheParameters::from_aster_coefficients(coeft);
    assert_eq!(p.to_aster_coefficients(), coeft);

    println!(
        "K_0 = {}, A_K = {}, K = {}, N = {}, C1 = {}, D1 = {}, C2 = {}, D2 = {}, A_I = {}",
        p.drag_stress,
        p.drag_hardening_coupling,
        p.initial_threshold,
        p.flow_exponent,
        p.back_stress_modulus_1,
        p.back_stress_recovery_split_1,
        p.back_stress_modulus_2,
        p.back_stress_recovery_split_2,
        p.dynamic_recovery_floor
    );
    assert_eq!(p.drag_stress, 1.0);
    assert_eq!(p.drag_hardening_coupling, 2.0);
    assert_eq!(p.threshold_hardening_multiplier, 3.0);
    assert_eq!(p.initial_threshold, 4.0);
    assert_eq!(p.flow_exponent, 5.0);
    assert_eq!(p.back_stress_modulus_1, 15.0);
    assert_eq!(p.back_stress_recovery_split_1, 17.0);
    assert_eq!(p.back_stress_modulus_2, 20.0);
    assert_eq!(p.back_stress_recovery_split_2, 22.0);
    assert_eq!(p.dynamic_recovery_floor, 25.0);
}

/// **Methodology.** [`ViscoplasticChabocheParameters::validate`] must reject
/// exactly the inputs upstream's arithmetic cannot survive. Feed it a
/// non-positive `K_0`, a zero `Q_M`, a negative `K`, and a `D1` above one, and
/// require an error each time; feed it the exercise set and require success.
///
/// **Results (2026-08-05).** All four bad sets returned
/// `OffbeatError::Unphysical` and the good set returned `Ok`.
#[test]
fn validate_rejects_the_coefficients_that_break_the_arithmetic() {
    assert!(exercise_parameters().validate().is_ok());

    let mut bad = exercise_parameters();
    bad.drag_stress = 0.0;
    assert!(bad.validate().is_err());

    let mut bad = exercise_parameters();
    bad.hardening_saturation_max = 0.0;
    assert!(bad.validate().is_err());

    let mut bad = exercise_parameters();
    bad.initial_threshold = -1.0;
    assert!(bad.validate().is_err());

    let mut bad = exercise_parameters();
    bad.back_stress_recovery_split_1 = 1.5;
    assert!(bad.validate().is_err());
}

/// **Methodology.** The 27 internal variables must survive a trip through the
/// flat ODE vector unchanged, in upstream's slot order. Build a state with a
/// distinct value in every one of the 27 slots, pack, unpack, and require bit
/// equality; then check three slots individually against upstream's layout
/// (`vini(25) = R`, `vini(26) = q`, `vini(27) = p`).
///
/// **Results (2026-08-05).** `to_ode_state()` produced 27 entries, the
/// round-trip was bit-exact, and the printed tail was
/// `y[24] = 25, y[25] = 26, y[26] = 27` — `R`, `q`, `p` in upstream's order.
#[test]
fn state_round_trips_through_the_ode_vector() {
    let y: Vec<f64> = (1..=ODE_EQUATION_COUNT).map(|i| i as f64).collect();
    let state = ViscoplasticChabocheState::from_ode_state(&y);
    let back = state.to_ode_state();
    assert_eq!(back.len(), ODE_EQUATION_COUNT);
    assert_eq!(back, y);
    println!(
        "y[24] = {}, y[25] = {}, y[26] = {}",
        back[24], back[25], back[26]
    );
    assert_eq!(state.isotropic_hardening, 25.0);
    assert_eq!(state.memory_radius, 26.0);
    assert_eq!(state.accumulated_strain, 27.0);
    assert_eq!(INTERNAL_VARIABLE_COUNT, ODE_EQUATION_COUNT + 1);
}

// ── The threshold branch ─────────────────────────────────────────────────────

/// **Methodology.** `rkdcha.F90` lines 95-104 zero *every* rate when
/// `critv = J − R − K ≤ 0`, including the static recovery of `R`, which the
/// implicit path `cvmres.F90` keeps (`rf = b(Q−R)dp + sgn·G_R·Δt·|Q_R−R|^M_R`,
/// with the recovery term surviving `dp = 0`). This test pins the explicit
/// behaviour: with `G_R = 2·10⁴ Pa^(1−M_R)/s` — i.e. static recovery switched
/// **on** — and a stress below threshold, all 27 rates must be exactly zero.
/// The same law with a stress above threshold must give a non-zero `Ṙ`, so the
/// zero above is the branch and not a dead parameter.
///
/// Pass criterion: every one of the 27 sub-threshold rates identically `0.0`.
///
/// **Results (2026-08-05).** Sub-threshold (`σ_xx = 40 MPa`): the test printed
/// `J = 40000000 Pa, overstress = -10000000 Pa, R_dot = 0 Pa/s, p_dot = 0
/// 1/s`, and all 27 rates asserted exactly `0` — including `Ṙ = 0` despite
/// `G_R > 0`. The discrepancy with `cvmres.F90` is therefore real and is
/// reproduced. Above threshold (`σ_xx = 200 MPa`) it printed
/// `overstress = 150000000 Pa, p_dot = 7.59375 1/s,
/// R_dot = 1435570989173070800 Pa/s`, both non-zero, so the zero above is the
/// branch and not a dead parameter. (The enormous `Ṙ` is an artefact of the
/// deliberately synthetic `G_R = 2e4` with `M_R = 2`, whose unit is
/// `Pa^(1−M_R)/s`; it is not a material value.)
#[test]
fn elastic_branch_zeroes_every_rate() {
    let mut params = exercise_parameters();
    params.static_recovery_rate_r = 2.0e4; // G_R > 0: recovery is switched on
    let law = ViscoplasticChabocheWithMemory::new(params).unwrap();
    let state = ViscoplasticChabocheState::undeformed();

    let below = uniaxial_stress(40.0e6);
    let (_, j) = law.effective_deviator(below, &state);
    let f = law.overstress(below, &state);
    let rates = law.internal_variable_rates(below, &state);
    println!(
        "sub-threshold: J = {j} Pa, overstress = {f} Pa, R_dot = {} Pa/s, p_dot = {} 1/s",
        rates.isotropic_hardening_rate, rates.accumulated_strain_rate
    );
    for (i, r) in rates
        .viscoplastic_strain_rate
        .components()
        .iter()
        .chain(rates.back_strain_1_rate.components().iter())
        .chain(rates.back_strain_2_rate.components().iter())
        .chain(rates.memory_centre_rate.components().iter())
        .chain(
            [
                rates.isotropic_hardening_rate,
                rates.memory_radius_rate,
                rates.accumulated_strain_rate,
            ]
            .iter(),
        )
        .enumerate()
    {
        assert_eq!(*r, 0.0, "rate {i} was {r}, expected 0 below threshold");
    }

    let above = uniaxial_stress(200.0e6);
    let hot = law.internal_variable_rates(above, &state);
    println!(
        "above threshold: overstress = {} Pa, p_dot = {} 1/s, R_dot = {} Pa/s",
        law.overstress(above, &state),
        hot.accumulated_strain_rate,
        hot.isotropic_hardening_rate
    );
    assert!(hot.accumulated_strain_rate > 0.0);
    assert_ne!(hot.isotropic_hardening_rate, 0.0);
}

// ── Flow-rule invariants ─────────────────────────────────────────────────────

/// **Methodology.** Two properties make `p` "the accumulated *equivalent*
/// viscoplastic strain" rather than an arbitrary scalar, and both follow from
/// `ε̇^vi = (3/2)(smx/J)·ṗ`:
///
/// 1. `tr(ε̇^vi) = 0` — viscoplastic flow preserves volume, because `smx` is a
///    deviator by construction.
/// 2. `√(2/3 · ε̇^vi : ε̇^vi) = ṗ` exactly.
///
/// Checked at a general (non-uniaxial, sheared) stress state so that a mistake
/// in the Mandel `√2` scaling of the shear entries would break property 2 — a
/// uniaxial state would not catch it. Pass criterion: the trace below `1e-14`
/// of the rate's own Frobenius norm (cancellation of three numbers of order
/// `ṗ` cannot be exact in floating point), and the equivalent rate within
/// `1e-12` relative.
///
/// **Results (2026-08-05).** At `σ = (220, 60, −40, 35, −20, 15)` MPa in Mandel
/// components: `ṗ = 20.62734519709515 1/s`, equivalent of `ε̇^vi` =
/// `20.62734519709515 1/s`, relative difference `0.000e0` — bit-identical.
/// `tr(ε̇^vi) = 1.7763568394002505e-15`, which against a rate of order 20 is a
/// relative `8.6e-17`, i.e. round-off on the cancellation and not a volumetric
/// component.
#[test]
fn flow_is_deviatoric_and_its_equivalent_rate_is_p_dot() {
    let law = ViscoplasticChabocheWithMemory::new(exercise_parameters()).unwrap();
    let state = ViscoplasticChabocheState::undeformed();
    let stress = AsterVoigt::from_components([220.0e6, 60.0e6, -40.0e6, 35.0e6, -20.0e6, 15.0e6]);

    let rates = law.internal_variable_rates(stress, &state);
    let d = rates.viscoplastic_strain_rate.components();
    let trace = d[0] + d[1] + d[2];
    let equivalent = (2.0 / 3.0
        * rates
            .viscoplastic_strain_rate
            .dot(rates.viscoplastic_strain_rate))
    .sqrt();
    let rel = (equivalent - rates.accumulated_strain_rate).abs() / rates.accumulated_strain_rate;
    println!(
        "trace(eps_vi_dot) = {trace}, p_dot = {} 1/s, equivalent = {equivalent} 1/s, rel = {rel:.3e}",
        rates.accumulated_strain_rate
    );
    let magnitude = rates.viscoplastic_strain_rate.norm();
    assert!(
        trace.abs() < 1.0e-14 * magnitude,
        "trace = {trace}, rate norm = {magnitude}"
    );
    assert!(rel < 1.0e-12, "relative difference = {rel:.3e}");
}

/// **Methodology.** [`ViscoplasticChabocheWithMemory::flow_rate`] must be the
/// plain Norton power law `(F/(K₀ + A_K·R))^N` when `ALP ≤ 1e-30`, and must
/// pick up the factor `exp(ALP·(F/(K₀+A_K·R))^(N+1))` when `ALP` exceeds it —
/// upstream's `if (alp .gt. 1.0d-30)` guard. Evaluated against hand-computed
/// closed forms at `F = 60 MPa`, `R = 25 MPa`, `K₀ = 100 MPa`, `A_K = 0.2`,
/// `N = 5`, so the reduced overstress is `60/105 = 0.571428…`. Pass criterion:
/// `1e-14` relative.
///
/// **Results (2026-08-05).** The reduced overstress printed as
/// `0.5714285714285714`. `ALP = 0` gave `ṗ = 0.06092699470458736 1/s` against
/// the closed form `0.5714285714285714^5 = 0.06092699470458736` —
/// bit-identical. `ALP = 0.5` gave `ṗ = 0.06199687943455993 1/s` against
/// `0.06092699470458736 · exp(0.5 · 0.5714285714285714^6) =
/// 0.06199687943455993` — also bit-identical. `ALP = 1e-31` returned exactly
/// the `ALP = 0` value, confirming upstream's `> 1e-30` guard, and `F = 0` and
/// `F = -1 Pa` both returned exactly `0`.
#[test]
fn flow_rate_matches_the_closed_form_power_law() {
    let mut params = exercise_parameters();
    params.exponential_flow_coefficient = 0.0;
    let law = ViscoplasticChabocheWithMemory::new(params).unwrap();

    let f = 60.0e6;
    let r = 25.0e6;
    let reduced = f / (params.drag_stress + params.drag_hardening_coupling * r);
    let plain = reduced.powf(params.flow_exponent);
    let got = law.flow_rate(f, r);
    println!("ALP = 0: p_dot = {got}, closed form = {plain}, reduced = {reduced}");
    assert!((got - plain).abs() <= 1.0e-14 * plain);

    let mut params_exp = params;
    params_exp.exponential_flow_coefficient = 0.5;
    let law_exp = ViscoplasticChabocheWithMemory::new(params_exp).unwrap();
    let expected = plain * (0.5 * reduced.powf(params.flow_exponent + 1.0)).exp();
    let got_exp = law_exp.flow_rate(f, r);
    println!("ALP = 0.5: p_dot = {got_exp}, closed form = {expected}");
    assert!((got_exp - expected).abs() <= 1.0e-14 * expected);

    let mut params_tiny = params;
    params_tiny.exponential_flow_coefficient = 1.0e-31;
    let law_tiny = ViscoplasticChabocheWithMemory::new(params_tiny).unwrap();
    assert_eq!(law_tiny.flow_rate(f, r), plain);

    assert_eq!(law.flow_rate(0.0, r), 0.0);
    assert_eq!(law.flow_rate(-1.0, r), 0.0);
}

// ── The upstream typo ────────────────────────────────────────────────────────

/// **Methodology — the `rkdcha.F90` line 124 verdict.**
///
/// Upstream writes the two kinematic-hardening lines as
///
/// ```text
/// da1v(itens) = d1*a1v(itens)+(1.0d0-d1)*xna1v*petin(itens)
/// da2v(itens) = d2*a2v(itens)+(1.0d0-d1)*xna2v*petin(itens)
/// ```
///
/// The second line's leading `d2*a2v` fixes it as the `α₂` equation, so the
/// `(1.0d0-d1)` that follows disagrees both with its own line and with the
/// implicit path: `cvmres.F90`'s `JF` block computes
/// `zz = zz*(1.d0-d2)*g20*ccin*dp*2.d0/3.d0`, and every other term of `LF`/`JF`
/// maps one-to-one onto `da1v`/`da2v` once `X_i = (2/3)C_iα_i` is substituted.
/// The verdict is **an upstream typo in the explicit path**, and this port
/// reproduces it ([`RKDCHA_ALPHA2_USES_D1`]`= true`).
///
/// The test makes the two forms distinguishable by choosing `D1 = 0.2` and
/// `D2 = 0.9` with a non-zero, non-aligned `α₂`, switching off static recovery
/// (`G_X2 = 0`) so only the dynamic term remains, then rebuilding both
/// candidate rates from the public intermediates. Pass criterion: the port
/// equals the `(1 − D1)` form to `1e-12` relative **and** differs measurably
/// from the `(1 − D2)` form.
///
/// If upstream ever fixes line 124, this test is the thing that must be
/// changed — flip [`RKDCHA_ALPHA2_USES_D1`] and swap the two assertions. That
/// is the intent: the discrepancy is pinned so a silent upstream change cannot
/// pass unnoticed.
///
/// **Results (2026-08-05).** Component 0 of `α̇₂`, as printed:
/// port `0.05495419205908281 1/s`, `(1−D1)` form `0.05495419205908281 1/s`
/// (bit-identical), `(1−D2)` form `0.059118552375123284 1/s`. The two
/// candidate forms differ by `4.1644e-3 1/s`, i.e. **7.6 %** of the value
/// upstream actually computes — far above rounding, and it grows with
/// `|D1 − D2|` and with the alignment of `α₂` with the flow direction. All six
/// components matched the `(1−D1)` form to `1e-12` relative.
#[test]
fn rkdcha_alpha2_reuses_d1_upstream_typo() {
    assert!(RKDCHA_ALPHA2_USES_D1);

    let mut params = exercise_parameters();
    params.back_stress_recovery_split_1 = 0.2;
    params.back_stress_recovery_split_2 = 0.9;
    params.static_recovery_rate_x1 = 0.0;
    params.static_recovery_rate_x2 = 0.0;
    let law = ViscoplasticChabocheWithMemory::new(params).unwrap();

    let state = ViscoplasticChabocheState {
        back_strain_1: AsterVoigt::from_components([1.0e-3, -0.5e-3, -0.5e-3, 0.2e-3, 0.0, 0.0]),
        back_strain_2: AsterVoigt::from_components([
            0.4e-3, 0.3e-3, -0.7e-3, -0.1e-3, 0.15e-3, 0.0,
        ]),
        accumulated_strain: 0.02,
        ..ViscoplasticChabocheState::undeformed()
    };
    let stress = AsterVoigt::from_components([260.0e6, 40.0e6, -30.0e6, 25.0e6, 0.0, 0.0]);

    // Rebuild the intermediates from the public API, exactly as rkdcha does.
    let (smx, j) = law.effective_deviator(stress, &state);
    let f = law.overstress(stress, &state);
    let p_dot = law.flow_rate(f, state.isotropic_hardening);
    let ccin = params.dynamic_recovery_floor
        + (1.0 - params.dynamic_recovery_floor)
            * (-params.isotropic_rate * state.accumulated_strain).exp();
    let gamma2 = params.dynamic_recovery_2 * ccin;
    let smx_c = smx.components();
    let a2 = state.back_strain_2.components();
    let petin: [f64; 6] = std::array::from_fn(|i| SQRT_1_5 * smx_c[i] / j);
    let devi: [f64; 6] = std::array::from_fn(|i| 1.5 * (smx_c[i] / j) * p_dot);
    let na2: f64 = (0..6).map(|i| a2[i] * petin[i]).sum();

    let candidate = |split: f64| -> [f64; 6] {
        std::array::from_fn(|i| {
            let inner =
                params.back_stress_recovery_split_2 * a2[i] + (1.0 - split) * na2 * petin[i];
            devi[i] - gamma2 * inner * p_dot
        })
    };
    let as_upstream = candidate(params.back_stress_recovery_split_1); // (1 - D1)
    let as_symmetric = candidate(params.back_stress_recovery_split_2); // (1 - D2)

    let got = law
        .internal_variable_rates(stress, &state)
        .back_strain_2_rate
        .components();

    println!(
        "alpha2_dot[0]: port = {}, (1-D1) form = {}, (1-D2) form = {}, difference = {:.4e}",
        got[0],
        as_upstream[0],
        as_symmetric[0],
        (as_upstream[0] - as_symmetric[0]).abs()
    );
    println!(
        "relative size of the discrepancy = {:.1} %",
        100.0 * (as_upstream[0] - as_symmetric[0]).abs() / as_upstream[0].abs()
    );

    for i in 0..6 {
        assert!(
            (got[i] - as_upstream[i]).abs() <= 1.0e-12 * as_upstream[i].abs().max(1.0e-12),
            "component {i}: port {} vs (1-D1) form {}",
            got[i],
            as_upstream[i]
        );
    }
    assert!(
        (as_upstream[0] - as_symmetric[0]).abs() > 1.0e-6 * as_upstream[0].abs(),
        "the two candidate forms must be distinguishable for this test to mean anything"
    );
}

// ── Closed-form saturation ───────────────────────────────────────────────────

/// **Methodology.** With static recovery off and `γ₁` frozen (`A_I = 1`), the
/// first back stress obeys Armstrong-Frederick,
/// `α̇₁ = ε̇^vi − γ₁·[D1·α₁ + (1−D1)(α₁·n̂)n̂]·ṗ`. Under proportional loading
/// `α₁` aligns with `n̂`, the bracket collapses to `α₁` for **any** `D1`, and
/// the fixed point is `α₁ = √(3/2)/γ₁`. Converting to the back stress
/// `X₁ = (2/3)C₁α₁` and taking its von Mises equivalent gives the classical
/// closed form
///
/// `‖X₁‖_vM → C₁/γ₁`
///
/// independent of stress level, of `D1`, and of the flow exponent. That makes
/// it a strong check on the whole kinematic block: the `1.5` divisors, the
/// `√1.5` normalisation of `n̂`, and the `(2/3)C` scaling must all be right for
/// the number to come out.
///
/// The system is integrated under **fixed** uniaxial stress with
/// [`OdeIntegrator::typed`] and [`OdeSolver::rkf45`] (`abs_tol = 1e-12`,
/// `rel_tol = 1e-10`) for 2000 s, with `C₁ = 200 MPa`, `γ₁ = 100`, so the
/// prediction is `‖X₁‖_vM = 2.0 MPa`. Pass criterion: within `0.1 %`.
/// Independence of `D1` is checked by repeating at `D1 = 0.0`, `0.5` and `1.0`.
///
/// **Results (2026-08-05).** Predicted `2000000 Pa`. Measured, as printed:
///
/// ```text
/// D1 = 0:   |X1|_vM = 2000000.0000277264 Pa, rel = 1.386e-11, p = 4.877097589571732
/// D1 = 0.5: |X1|_vM = 2000000.0000277457 Pa, rel = 1.387e-11, p = 4.877097589571731
/// D1 = 1:   |X1|_vM = 2000000.0000000002 Pa, rel = 1.164e-16, p = 4.8770975895712665
/// ```
///
/// All three land on `C₁/γ₁` — the `D1 = 1` case exactly, the other two to
/// eleven digits (the residual is the finite integration time, not a bias: at
/// `D1 = 1` the radial term vanishes identically and the remaining ODE is
/// linear, so it converges faster). `p ≈ 4.877` at the end for every `D1`,
/// so flow was still active throughout and this is a genuine saturation rather
/// than an arrest.
#[test]
fn first_back_stress_saturates_at_c_over_gamma() {
    let c1 = 200.0e6;
    let gamma1 = 100.0;
    let predicted = c1 / gamma1;

    for d1 in [0.0, 0.5, 1.0] {
        let mut params = saturation_parameters(c1, gamma1);
        params.back_stress_recovery_split_1 = d1;
        let law = ViscoplasticChabocheWithMemory::new(params).unwrap();

        let system = FixedStressSystem {
            law,
            stress: uniaxial_stress(5.0e6),
        };
        let mut integrator = OdeIntegrator::typed(system, OdeSolver::rkf45(27, 1.0e-12, 1.0e-10));
        let mut y = ViscoplasticChabocheState::undeformed().to_ode_state();
        let mut dx = 1.0e-3;
        integrator.integrate(0.0, 2000.0, &mut y, &mut dx).unwrap();

        let end = ViscoplasticChabocheState::from_ode_state(&y);
        let x1 =
            AsterVoigt::from_components(end.back_strain_1.components().map(|a| 2.0 / 3.0 * c1 * a));
        let measured = von_mises(x1);
        let rel = (measured - predicted).abs() / predicted;
        println!(
            "D1 = {d1}: |X1|_vM = {measured} Pa, predicted = {predicted} Pa, rel = {rel:.3e}, p = {}",
            end.accumulated_strain
        );
        assert!(rel < 1.0e-3, "D1 = {d1}: rel = {rel:.3e}");
        assert!(end.accumulated_strain > 0.0);
    }
}

// ── End-to-end through the wrapper ───────────────────────────────────────────

/// **Methodology.** A strain-controlled relaxation hold, the case this law
/// exists for. A uniaxial total strain `ε_xx = 1e-3` is imposed and then held
/// (`Δε = 0`) for 1 s while [`ViscoplasticChabocheSystem::integrate_step`]
/// drives the 27 ODEs through [`OdeIntegrator::typed`] with RKF45. Four
/// properties are required, each of which would fail under a different
/// transcription error:
///
/// 1. `p` strictly increases — flow occurred.
/// 2. `tr(ε^vi) = 0` — the accumulated viscoplastic strain stays deviatoric.
/// 3. the von Mises stress at the end is strictly below its value at the start
///    — the definition of relaxation.
/// 4. `R` increased from zero — isotropic hardening accumulated (`B = 20`,
///    `Q_0 = 10 MPa` in the exercise set).
///
/// Pass criterion: (1), (3), (4) strict inequalities; (2) trace below `1e-15`
/// of the strain magnitude.
///
/// **Results (2026-08-05), as printed.** Start `σ_vM = 153846153.84615386 Pa`;
/// end `σ_vM = 94493973.95086613 Pa`, `p = 0.0002571927795462468`,
/// `R = 51636.37556180876 Pa`, `q = 0.00012859601821379717`,
/// `tr(ε^vi) = 5.421010862427522e-20`. The von Mises stress fell by **38.6 %**
/// of its initial value over the 1 s hold; `p`, `R` and `q` all grew from
/// zero; and the viscoplastic strain stayed deviatoric to `5.4e-20` on a
/// strain of order `2.6e-4`, i.e. a relative `2e-16`.
#[test]
fn strain_controlled_relaxation_runs_through_the_ode_wrapper() {
    let law = ViscoplasticChabocheWithMemory::new(exercise_parameters()).unwrap();
    let strain = AsterVoigt::from_components([1.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let system = ViscoplasticChabocheSystem::new(
        law,
        200.0e9,
        0.3,
        strain,
        AsterVoigt::from_components([0.0; 6]),
        1.0,
    )
    .unwrap();

    let start = ViscoplasticChabocheState::undeformed();
    let sigma_start = von_mises(deviator(system.stress_at(0.0, start.viscoplastic_strain)));

    let end = system
        .integrate_step(start, OdeSolver::rkf45(27, 1.0e-10, 1.0e-8), 1.0e-3)
        .unwrap();
    let sigma_end = von_mises(deviator(system.stress_at(1.0, end.viscoplastic_strain)));

    let evi = end.viscoplastic_strain.components();
    let trace = evi[0] + evi[1] + evi[2];
    println!(
        "start sigma_vM = {sigma_start} Pa; end sigma_vM = {sigma_end} Pa, p = {}, R = {} Pa, q = {}, trace(eps_vi) = {trace:e}",
        end.accumulated_strain, end.isotropic_hardening, end.memory_radius
    );
    println!(
        "relaxed by {:.1} % of the initial von Mises stress",
        100.0 * (sigma_start - sigma_end) / sigma_start
    );

    assert!(end.accumulated_strain > 0.0, "no flow occurred");
    assert!(sigma_end < sigma_start, "stress did not relax");
    assert!(end.isotropic_hardening > 0.0, "R did not grow");
    assert!(
        trace.abs() < 1.0e-15 * end.viscoplastic_strain.norm().max(1.0e-15),
        "trace = {trace:e}"
    );
}

/// **Methodology.** [`ViscoplasticChabocheSystem::stress_at`] is the isothermal
/// isotropic branch of `calsig.F90`. With **zero** viscoplastic strain it must
/// reduce to plain Hooke's law, so the closed forms
/// `σ_xx = 2μ·ε_xx + λ·tr(ε)` and `σ_yy = σ_zz = λ·tr(ε)` (with
/// `2μ = E/(1+ν)` and `λ = ν·2μ/(1−2ν)`) can be checked directly, and the
/// linear-in-time interpolation `ε(x) = ε_start + Δε·x/Δt` can be checked at
/// the midpoint of the step. Pass criterion: `1e-12` relative.
///
/// **Results (2026-08-05), as printed.** At `E = 200 GPa`, `ν = 0.3`,
/// `ε_xx = 1e-3`: `σ_xx = 269230769.2307692 Pa` against the closed form
/// `269230769.2307692 Pa`, and `σ_yy = 115384615.38461538 Pa` against
/// `115384615.38461538 Pa` — both bit-identical, and `σ_xy = 0` exactly. At
/// `x = 1 s` of a `Δt = 2 s` step with `Δε_xx = 1e-3` the stress was
/// `403846153.84615386 Pa`, exactly 1.5x the start value, i.e. the
/// interpolation used `x/Δt = 0.5`.
#[test]
fn stress_reduces_to_hooke_and_interpolates_linearly() {
    let law = ViscoplasticChabocheWithMemory::new(exercise_parameters()).unwrap();
    let e = 200.0e9;
    let nu = 0.3;
    let eps = 1.0e-3;
    let system = ViscoplasticChabocheSystem::new(
        law,
        e,
        nu,
        AsterVoigt::from_components([eps, 0.0, 0.0, 0.0, 0.0, 0.0]),
        AsterVoigt::from_components([eps, 0.0, 0.0, 0.0, 0.0, 0.0]),
        2.0,
    )
    .unwrap();

    let two_mu = e / (1.0 + nu);
    let lambda = nu * two_mu / (1.0 - 2.0 * nu);
    let sigma = system
        .stress_at(0.0, AsterVoigt::from_components([0.0; 6]))
        .components();
    let expected_xx = two_mu * eps + lambda * eps;
    let expected_yy = lambda * eps;
    println!(
        "sigma_xx = {} Pa (closed form {} Pa), sigma_yy = {} Pa (closed form {} Pa)",
        sigma[0], expected_xx, sigma[1], expected_yy
    );
    assert!((sigma[0] - expected_xx).abs() <= 1.0e-12 * expected_xx.abs());
    assert!((sigma[1] - expected_yy).abs() <= 1.0e-12 * expected_yy.abs());
    assert_eq!(sigma[3], 0.0);

    let mid = system
        .stress_at(1.0, AsterVoigt::from_components([0.0; 6]))
        .components();
    println!("midpoint sigma_xx = {} Pa", mid[0]);
    assert!((mid[0] - 1.5 * expected_xx).abs() <= 1.0e-12 * expected_xx.abs());
}

/// **Methodology.** The port must reject the geometry it cannot evaluate:
/// a non-positive Young's modulus, a Poisson ratio at or beyond `0.5` (the
/// bulk term divides by `1 − 2ν`), and a non-positive step duration (the
/// strain interpolation divides by it).
///
/// **Results (2026-08-05).** All three rejected with
/// `OffbeatError::Unphysical`; the valid set constructed successfully and
/// reported `n_eqns() = 27`.
#[test]
fn system_construction_rejects_impossible_geometry() {
    let law = ViscoplasticChabocheWithMemory::new(exercise_parameters()).unwrap();
    let zero = AsterVoigt::from_components([0.0; 6]);

    assert!(ViscoplasticChabocheSystem::new(law, 0.0, 0.3, zero, zero, 1.0).is_err());
    assert!(ViscoplasticChabocheSystem::new(law, 200.0e9, 0.5, zero, zero, 1.0).is_err());
    assert!(ViscoplasticChabocheSystem::new(law, 200.0e9, 0.3, zero, zero, 0.0).is_err());

    let ok = ViscoplasticChabocheSystem::new(law, 200.0e9, 0.3, zero, zero, 1.0).unwrap();
    println!("n_eqns = {}", ok.n_eqns());
    assert_eq!(ok.n_eqns(), ODE_EQUATION_COUNT);
    assert_eq!(law.aster_name(), "VISCOCHAB");
}
