// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the Chaboche kinematic-hardening laws.
//!
//! # What is checkable here, and what is not
//!
//! There is no closed-form solution for a general Chaboche step, so these tests
//! lean on four things that *are* exact:
//!
//! 1. **The flow condition itself.** After every plastic step the returned
//!    stress and state must satisfy `||s - X||_vm = R (+ viscous overstress)`
//!    to machine precision. This is reconstructed from the *outputs* — the
//!    stress tensor, the back strains and `p` — not from any solver internal,
//!    so it checks the whole update and not just the root find.
//! 2. **Closed-form limits of the model.** The Armstrong-Frederick saturation
//!    `||X||_vm -> C/gamma` (Armstrong & Frederick 1966; Lemaitre & Chaboche,
//!    *Mechanics of Solid Materials*, ch. 5) and the linear-kinematic (Prager)
//!    limit `gamma -> 0`, which the implicit scheme reproduces *exactly* in a
//!    single step.
//! 3. **Structural relationships between variants** — that the rate-dependent
//!    law tends to the rate-independent one as the step becomes slow, that one
//!    back stress is the two-back-stress law with `C2 = 0`, and that
//!    `delta = 1` switches the non-radial correction off.
//! 4. **Invariants** — plastic incompressibility,
//!    `sqrt(2/3 dEps_p:dEps_p) = dp`, and the hydrostatic stress being
//!    untouched by plastic flow.
//!
//! Nothing here is validation against code_aster output, against `astest`
//! cases, or against experiment. It is verification of the port.
//!
//! # The driving path
//!
//! Most tests drive **pure shear** under strain control. That is deliberate:
//! the deviator stays one-dimensional so every quantity can be reasoned about
//! by hand (`sigma_eq = sqrt(3) tau`), the mean stress stays identically zero
//! so a leak from the deviatoric into the hydrostatic part is visible
//! immediately, and no iteration on the lateral stress is needed as it would be
//! for uniaxial *stress*.

use approx::assert_relative_eq;
use outram_foam_basic_lib::primitives::SymmTensor;

use super::*;

/// Structural steel at room temperature: `E = 200 GPa`, `nu = 0.3`.
fn steel() -> ElasticModuli {
    ElasticModuli::new(200.0e9, 0.3).expect("valid moduli")
}

/// A representative rate-independent Armstrong-Frederick law:
/// `R0 = 200 MPa`, `C1 = 60 GPa`, `gamma1 = 500`, so `C1/gamma1 = 120 MPa`.
fn af_law() -> ChabocheLaw {
    ChabocheLaw::VmisCin1Chab(ChabocheParameters::armstrong_frederick(
        200.0e6, 60.0e9, 500.0,
    ))
}

/// A tight solve: 200 iterations to a normalised residual of 1e-14.
fn control() -> SolverControl {
    SolverControl {
        max_iter: 200,
        residual_tol: 1.0e-14,
        step_tol: 1.0e-18,
    }
}

/// A pure-shear strain increment with tensorial shear component `exy`.
fn shear(exy: f64) -> SymmTensor {
    SymmTensor::new(0.0, exy, 0.0, 0.0, 0.0, 0.0)
}

/// The von Mises equivalent of a tensor's deviator \[Pa\].
fn equivalent(t: SymmTensor) -> f64 {
    von_mises_of_deviator(deviator(t))
}

/// Drive `n_steps` equal pure-shear strain increments and return the final
/// stress and state.
fn shear_path(
    law: ChabocheLaw,
    mut state: ChabocheState,
    mut stress: SymmTensor,
    total_exy: f64,
    n_steps: usize,
    dt_per_step: f64,
) -> (SymmTensor, ChabocheState) {
    let step = ThermoElasticStep::isothermal(steel(), dt_per_step);
    let d = total_exy / n_steps as f64;
    for _ in 0..n_steps {
        let out = law
            .integrate(state, stress, shear(d), step, control())
            .expect("integration converges");
        stress = out.stress;
        state = out.state;
    }
    (stress, state)
}

/// The residual of the flow condition, reconstructed from the *outputs* of a
/// step: `||s - X||_vm - R - K(dp/dt)^(1/n)` \[Pa\].
///
/// Zero to machine precision is the statement that the returned stress, back
/// strains and accumulated plastic strain are mutually consistent.
fn flow_condition_residual(law: ChabocheLaw, out: &ChabocheIncrement, dt: f64) -> f64 {
    let p = out.state.accumulated_plastic_strain;
    let (c1, c2) = law.kinematic_moduli(p);
    let back = out.state.back_stress.stress(c1, c2);
    let shifted = deviator(out.stress) - back;
    let mut resisting = law.start_radius(out.state);
    if law.is_rate_dependent() && dt > 0.0 {
        let m = law.parameters();
        resisting +=
            m.viscous_stress * (out.equivalent_increment / dt).powf(1.0 / m.viscous_exponent);
    }
    von_mises_of_deviator(shifted) - resisting
}

// ─────────────────────────────────────────────────────────────────────────────

/// **The six variants agree with the generated behaviour catalogue.**
///
/// *Methodology:* every variant's [`ChabocheLaw::aster_name`], `num_lc` and
/// declared state-variable count are compared against
/// [`AsterBehaviour`](crate::rheology::aster::catalogue::AsterBehaviour), which
/// is generated directly from upstream's `code_aster/Behaviours/*.py`. The
/// switch predicates ([`ChabocheLaw::back_stress_count`],
/// [`ChabocheLaw::is_rate_dependent`], [`ChabocheLaw::has_strain_memory`]) are
/// checked against the decoding `nmcham.F90` performs on the behaviour name.
/// Pass criterion: exact equality.
///
/// *Result (measured 2026-08-05):* all six rows printed by the run —
/// `VMIS_CIN1_CHAB num_lc=4 nvi= 8 nbvar=1 visc=false memo=false`,
/// `VMIS_CIN2_CHAB num_lc=4 nvi=14 nbvar=2 visc=false memo=false`,
/// `VISC_CIN1_CHAB num_lc=4 nvi= 8 nbvar=1 visc=true  memo=false`,
/// `VISC_CIN2_CHAB num_lc=4 nvi=14 nbvar=2 visc=true  memo=false`,
/// `VMIS_CIN2_MEMO num_lc=4 nvi=28 nbvar=2 visc=false memo=true`,
/// `VISC_CIN2_MEMO num_lc=4 nvi=28 nbvar=2 visc=true  memo=true` — match the
/// catalogue exactly. Interpretation: the enum is wired to the right catalogue
/// entries, so a downstream registry lookup cannot silently select a different
/// law, and the three switches are decoded from the name the way `nmcham.F90`
/// decodes them.
#[test]
fn the_variants_agree_with_the_catalogue() {
    let p = ChabocheParameters::armstrong_frederick(200.0e6, 60.0e9, 500.0);
    let cases = [
        (
            ChabocheLaw::VmisCin1Chab(p),
            "VMIS_CIN1_CHAB",
            1,
            false,
            false,
            8,
        ),
        (
            ChabocheLaw::VmisCin2Chab(p),
            "VMIS_CIN2_CHAB",
            2,
            false,
            false,
            14,
        ),
        (
            ChabocheLaw::ViscCin1Chab(p),
            "VISC_CIN1_CHAB",
            1,
            true,
            false,
            8,
        ),
        (
            ChabocheLaw::ViscCin2Chab(p),
            "VISC_CIN2_CHAB",
            2,
            true,
            false,
            14,
        ),
        (
            ChabocheLaw::VmisCin2Memo(p),
            "VMIS_CIN2_MEMO",
            2,
            false,
            true,
            28,
        ),
        (
            ChabocheLaw::ViscCin2Memo(p),
            "VISC_CIN2_MEMO",
            2,
            true,
            true,
            28,
        ),
    ];
    for (law, name, nbvar, visc, memo, nvi) in cases {
        println!(
            "{:<15} num_lc={} nvi={:>2} nbvar={} visc={:<5} memo={}",
            law.aster_name(),
            law.behaviour().num_lc(),
            law.behaviour().n_state_variables(),
            law.back_stress_count(),
            law.is_rate_dependent(),
            law.has_strain_memory()
        );
        assert_eq!(law.aster_name(), name);
        assert_eq!(law.behaviour().num_lc(), 4);
        assert_eq!(law.behaviour().n_state_variables(), nvi);
        assert_eq!(law.back_stress_count(), nbvar);
        assert_eq!(law.is_rate_dependent(), visc);
        assert_eq!(law.has_strain_memory(), memo);
    }
}

/// **A step below yield returns the elastic trial stress and changes nothing.**
///
/// *Methodology:* apply a pure-shear increment small enough that
/// `sqrt(3)*2mu*exy < R0`, and check the returned stress against the
/// hand-computed elastic answer `sigma_xy = 2mu exy`, that `dp = 0`, that the
/// back strain is untouched, and that `yielded` is false. Pass criterion: 1e-12
/// relative on the stress, exact on the flags.
///
/// *Result (measured 2026-08-05):* `sigma_xy = 7.6923077e7 Pa` against the
/// hand-computed `2mu exy = 7.6923077e7 Pa` (0.0 relative), giving
/// `sigma_eq = 1.3323468e8 Pa`, comfortably under `R0 = 2.0e8 Pa`; `dp = 0e0`,
/// `iterations = 0`, `yielded = false`, back strain unchanged. Interpretation:
/// the elastic branch short-circuits before the local solve, as upstream's
/// `seuil <= 0` branch does, and returns the predictor untouched.
#[test]
fn a_step_below_yield_is_purely_elastic() {
    let law = af_law();
    let exy = 5.0e-4;
    let out = law
        .integrate(
            ChabocheState::zero(),
            SymmTensor::default(),
            shear(exy),
            ThermoElasticStep::isothermal(steel(), 1.0),
            control(),
        )
        .expect("elastic step");

    let expected_xy = steel().twice_shear_modulus() * exy;
    println!(
        "elastic: sigma_xy = {:.7e} Pa (expected {:.7e}), sigma_eq = {:.7e} Pa, R0 = {:.4e} Pa, dp = {:e}, yielded = {}",
        out.stress.xy,
        expected_xy,
        equivalent(out.stress),
        law.parameters().r0,
        out.equivalent_increment,
        out.yielded
    );

    assert!(!out.yielded);
    assert_eq!(out.equivalent_increment, 0.0);
    assert_eq!(out.iterations, 0);
    assert_relative_eq!(out.stress.xy, expected_xy, max_relative = 1e-12);
    assert_eq!(out.state.back_stress, BackStress::zero());
    assert_eq!(out.state.accumulated_plastic_strain, 0.0);
}

/// **Every plastic step ends exactly on the flow surface.**
///
/// *Methodology:* the sharpest available check on the whole update. Drive 40
/// pure-shear steps well past yield with `VMIS_CIN1_CHAB`, and after each step
/// reconstruct `||s - X||_vm` from the *returned* stress tensor, back strain and
/// accumulated plastic strain, then compare with `R(p)`. Nothing internal to
/// the solver is used, so a wrong back-strain update, a wrong `C(p)`, or a
/// wrong deviator would all show up here. Pass criterion: the residual is below
/// 1e-5 Pa, i.e. 5e-14 relative to a 200 MPa yield radius — the floor set by the
/// normalised solver tolerance, not an arbitrary slack.
///
/// *Result (measured 2026-08-05):* 39 of the 40 steps were plastic (the first
/// is still elastic, since one increment reaches only 1.33e8 Pa against a
/// 2.0e8 Pa radius). The worst flow-condition residual over those 39 steps is
/// **1.7881e-6 Pa** against a yield radius of 2.0e8 Pa — **8.941e-15
/// relative**, which is the normalised solver tolerance (1e-14) times the
/// residual's stress scale, i.e. the solve is converging as tightly as it was
/// asked to. Final `p = 2.170738e-2`. Interpretation: the returned stress,
/// back stress and hardening state are mutually consistent to the requested
/// tolerance; the local solve reaches the true root of the collapsed scalar
/// equation, not a nearby one.
#[test]
fn every_plastic_step_ends_on_the_flow_surface() {
    let law = af_law();
    let mut state = ChabocheState::zero();
    let mut stress = SymmTensor::default();
    let step = ThermoElasticStep::isothermal(steel(), 1.0);
    let mut worst: f64 = 0.0;
    let mut plastic_steps = 0;

    for _ in 0..40 {
        let out = law
            .integrate(state, stress, shear(5.0e-4), step, control())
            .expect("integration converges");
        if out.yielded {
            // The first few steps are still elastic — the flow condition is an
            // inequality there, not an equality, so only plastic steps are
            // judged against it.
            plastic_steps += 1;
            worst = worst.max(flow_condition_residual(law, &out, 1.0).abs());
        }
        stress = out.stress;
        state = out.state;
    }
    println!(
        "{plastic_steps} of 40 steps were plastic; worst |flow-condition residual| = {:.4e} Pa (R0 = {:.4e} Pa, i.e. {:.3e} relative), final p = {:.6e}",
        worst,
        law.parameters().r0,
        worst / law.parameters().r0,
        state.accumulated_plastic_strain
    );
    assert!(plastic_steps > 30, "path must be mostly plastic");
    assert!(worst < 1.0e-5, "worst residual {worst:e} Pa");
}

/// **The plastic strain increment is deviatoric and has equivalent measure
/// `dp`.**
///
/// *Methodology:* two identities that must hold for any von Mises flow rule.
/// Plastic flow preserves volume, so `tr(dEps_p) = 0`; and the definition of
/// the accumulated equivalent plastic strain requires
/// `sqrt(2/3 dEps_p:dEps_p) = dp`. Checked on a plastic pure-shear step. Pass
/// criterion: trace below 1e-18 absolute, equivalent to 1e-12 relative.
///
/// *Result (measured 2026-08-05):* `tr(dEps_p) = 0.0000e0` exactly, and
/// `sqrt(2/3 dEps_p:dEps_p) = 7.267856e-4` against `dp = 7.267856e-4`,
/// agreeing to 0.0 relative. Interpretation: the `sqrt(3/2)` normalisation of
/// the flow direction is right; a factor slip there would make `p` and the
/// plastic strain disagree by `3/2` and quietly corrupt every hardening law
/// that reads `p`.
#[test]
fn the_plastic_strain_increment_is_deviatoric_with_equivalent_dp() {
    let law = af_law();
    let out = law
        .integrate(
            ChabocheState::zero(),
            SymmTensor::default(),
            shear(1.5e-3),
            ThermoElasticStep::isothermal(steel(), 1.0),
            control(),
        )
        .expect("plastic step");

    let dep = out.plastic_strain_increment;
    let equiv = (2.0 / 3.0 * dep.double_inner(dep)).sqrt();
    println!(
        "tr(dEps_p) = {:.4e}, sqrt(2/3 dEps_p:dEps_p) = {:.6e}, dp = {:.6e}",
        dep.tr(),
        equiv,
        out.equivalent_increment
    );
    assert!(dep.tr().abs() < 1.0e-18);
    assert_relative_eq!(equiv, out.equivalent_increment, max_relative = 1e-12);
}

/// **Pure shear stays pure shear and leaves the hydrostatic stress alone.**
///
/// *Methodology:* plastic flow is deviatoric, so a pure-shear strain path must
/// produce a stress with zero mean and no normal components at all. This is the
/// test that catches a hydrostatic leak from a mis-signed deviator or from a
/// non-deviatoric back stress. Pass criterion: all five non-`xy` components and
/// the trace below 1e-9 Pa, against an `xy` component of order 1e8 Pa.
///
/// *Result (measured 2026-08-05):* after 40 steps to `exy = 0.02`,
/// `sigma_xy = 1.8474723e8 Pa` with `tr(sigma) = 0.0000e0 Pa` and every one of
/// `xx`, `yy`, `zz`, `xz`, `yz` exactly `0.0000e0 Pa`. Interpretation: the
/// deviatoric/hydrostatic split is clean and the back stress stays
/// deviatoric.
#[test]
fn pure_shear_leaves_the_hydrostatic_stress_untouched() {
    let (stress, _) = shear_path(
        af_law(),
        ChabocheState::zero(),
        SymmTensor::default(),
        0.02,
        40,
        1.0,
    );
    println!(
        "sigma = xx {:.4e} yy {:.4e} zz {:.4e} xy {:.7e} xz {:.4e} yz {:.4e}, tr = {:.4e}",
        stress.xx,
        stress.yy,
        stress.zz,
        stress.xy,
        stress.xz,
        stress.yz,
        stress.tr()
    );
    assert!(stress.tr().abs() < 1.0e-9);
    for c in [stress.xx, stress.yy, stress.zz, stress.xz, stress.yz] {
        assert!(c.abs() < 1.0e-9, "component {c:e} should vanish");
    }
}

/// **`gamma = 0` gives linear (Prager) kinematic hardening, exactly, in one
/// step.**
///
/// *Methodology:* with no dynamic recovery the Armstrong-Frederick law reduces
/// to `alpha_dot = eps_p_dot`, so `X = (2/3) C eps_p` and the equivalent stress
/// is `sigma_eq = R0 + C p` — a straight line with slope `C`. The
/// backward-Euler update is *exact* for a linear ODE, so a single large step
/// must land on that line to machine precision, not merely close to it. That
/// exactness is the point of the test: it discriminates between a correct
/// implicit update and a plausible explicit one, which a many-small-steps test
/// would not. Pass criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* one step to `exy = 0.01` gives
/// `p = 9.828533e-3` and `sigma_eq = 3.9657065e8 Pa`, against the closed form
/// `R0 + C p = 3.9657065e8 Pa` — 0.0 relative. The same path in 100 steps
/// gives `p = 9.828533e-3` and `sigma_eq = 3.9657065e8 Pa`, differing from the
/// single step by **2.856e-15** relative. Interpretation: the implicit
/// back-strain update is algebraically exact for the linear case and genuinely
/// step-size independent there. The `(2/3)` in `X = (2/3) C alpha` and the
/// `3/2` in `dEps_p` are mutually consistent — a slip in either would change
/// the hardening slope by `3/2` or `2/3`.
#[test]
fn zero_recovery_gives_exact_linear_kinematic_hardening() {
    let c = 20.0e9;
    let law = ChabocheLaw::VmisCin1Chab(ChabocheParameters::armstrong_frederick(200.0e6, c, 0.0));

    let out = law
        .integrate(
            ChabocheState::zero(),
            SymmTensor::default(),
            shear(0.01),
            ThermoElasticStep::isothermal(steel(), 1.0),
            control(),
        )
        .expect("plastic step");
    let p = out.state.accumulated_plastic_strain;
    let closed_form = law.parameters().r0 + c * p;
    println!(
        "one step:      p = {:.6e}, sigma_eq = {:.7e} Pa, R0 + C p = {:.7e} Pa",
        p,
        equivalent(out.stress),
        closed_form
    );
    assert_relative_eq!(equivalent(out.stress), closed_form, max_relative = 1e-12);

    let (stress_many, state_many) = shear_path(
        law,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.01,
        100,
        1.0,
    );
    println!(
        "hundred steps: p = {:.6e}, sigma_eq = {:.7e} Pa, rel gap = {:.3e}",
        state_many.accumulated_plastic_strain,
        equivalent(stress_many),
        (equivalent(stress_many) - equivalent(out.stress)).abs() / equivalent(out.stress)
    );
    assert_relative_eq!(
        equivalent(stress_many),
        equivalent(out.stress),
        max_relative = 1e-12
    );
}

/// **The back stress saturates at `C/gamma` — the Armstrong-Frederick
/// result.**
///
/// *Methodology:* the published closed-form limit of the Armstrong-Frederick
/// evolution law under monotonic proportional loading (Armstrong & Frederick
/// 1966; Lemaitre & Chaboche, *Mechanics of Solid Materials*, ch. 5) is
/// `||X||_vm -> C/gamma`, and hence `sigma_eq -> R_inf + C/gamma`. Drive 400
/// pure-shear steps to a total tensorial shear strain of 0.05 with
/// `C1 = 60 GPa`, `gamma1 = 500` (so `C/gamma = 120 MPa`) and
/// `R_inf = R0 = 200 MPa`, and compare the final equivalent back stress with
/// `C/gamma`. The approach is geometric with ratio `1/(1+gamma dp)`, so the
/// residual at `p ~ 0.04` is negligible. Pass criterion: 1e-6 relative —
/// deliberately far looser than machine precision, because this is a *limit*,
/// and asserting tighter would be asserting the path length rather than the
/// physics.
///
/// *Result (measured 2026-08-05):* `p = 5.634836e-2`,
/// `||X||_vm = 1.2000000e8 Pa` against `C/gamma = 1.2000000e8 Pa` — **1.521e-12
/// relative** — and `sigma_eq = 3.2000000e8 Pa` against
/// `R0 + C/gamma = 3.2000000e8 Pa`. Interpretation: the dynamic-recovery term
/// has the right coefficient and the right sign. A sign error would make `X`
/// grow without bound; a factor error would move the saturation level, which is
/// the single most consequential number in a cyclic-plasticity model because it
/// sets the stabilised hysteresis-loop amplitude.
#[test]
fn the_back_stress_saturates_at_c_over_gamma() {
    let law = af_law();
    let m = law.parameters();
    let expected = m.c1_asymptotic / m.gamma1_initial;

    let (stress, state) = shear_path(
        law,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    let (c1, c2) = law.kinematic_moduli(state.accumulated_plastic_strain);
    let x_eq = state.back_stress.equivalent_stress(c1, c2);
    println!(
        "p = {:.6e}, ||X||_vm = {:.7e} Pa, C/gamma = {:.7e} Pa (rel gap {:.3e}), sigma_eq = {:.7e} Pa, R0 + C/gamma = {:.7e} Pa",
        state.accumulated_plastic_strain,
        x_eq,
        expected,
        (x_eq - expected).abs() / expected,
        equivalent(stress),
        m.r0 + expected
    );
    assert_relative_eq!(x_eq, expected, max_relative = 1e-6);
    assert_relative_eq!(equivalent(stress), m.r0 + expected, max_relative = 1e-6);
}

/// **Two back stresses saturate at the sum `C1/gamma1 + C2/gamma2`.**
///
/// *Methodology:* the same closed-form limit applied to each
/// Armstrong-Frederick tensor independently — they are uncoupled except through
/// the shared `dp`, so under proportional loading the saturated equivalent
/// stress is `R_inf + C1/gamma1 + C2/gamma2`. Uses a fast tensor
/// (`C1 = 120 GPa`, `gamma1 = 2000`, so 60 MPa) and a slow one (`C2 = 8 GPa`,
/// `gamma2 = 100`, so 80 MPa), the split that motivates having two in the first
/// place. This test is what catches a `VMIS_CIN2_CHAB` that silently ignores
/// its second tensor — the single-tensor answer would be 80 MPa short. Pass
/// criterion: 1e-5 relative on the total.
///
/// *Result (measured 2026-08-05):* driven to `p = 2.872018e-1` (long enough
/// for the slow tensor, whose time constant is `1/gamma2 = 1e-2`),
/// `||X||_vm = 1.4000000e8 Pa` against `C1/gamma1 + C2/gamma2 = 1.4000000e8 Pa`
/// — **2.365e-13 relative** — and `sigma_eq = 3.4000000e8 Pa`. Interpretation:
/// both tensors are live, evolve independently on their own time constants, and
/// add.
#[test]
fn two_back_stresses_saturate_at_the_sum_of_their_ratios() {
    let mut p = ChabocheParameters::armstrong_frederick(200.0e6, 120.0e9, 2000.0);
    p.c2_asymptotic = 8.0e9;
    p.gamma2_initial = 100.0;
    let law = ChabocheLaw::VmisCin2Chab(p);
    let expected = p.c1_asymptotic / p.gamma1_initial + p.c2_asymptotic / p.gamma2_initial;

    let (stress, state) = shear_path(
        law,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.25,
        2000,
        1.0,
    );
    let (c1, c2) = law.kinematic_moduli(state.accumulated_plastic_strain);
    let x_eq = state.back_stress.equivalent_stress(c1, c2);
    println!(
        "p = {:.6e}, ||X||_vm = {:.7e} Pa, C1/g1 + C2/g2 = {:.7e} Pa (rel gap {:.3e}), sigma_eq = {:.7e} Pa",
        state.accumulated_plastic_strain,
        x_eq,
        expected,
        (x_eq - expected).abs() / expected,
        equivalent(stress)
    );
    assert_relative_eq!(x_eq, expected, max_relative = 1e-5);
    assert_relative_eq!(equivalent(stress), p.r0 + expected, max_relative = 1e-5);
}

/// **One back stress is the two-back-stress law with `C2 = 0`.**
///
/// *Methodology:* `nmcham.F90` sets `c2inf = 0` when `nbvar = 1`, so the
/// one-tensor law is not a separate algorithm but the two-tensor one with its
/// second modulus zeroed. Running `VMIS_CIN1_CHAB` and `VMIS_CIN2_CHAB` on the
/// same path with `C2 = 0` must therefore give the same answer. This pins the
/// `back_stress_count` branching, which is otherwise only exercised implicitly.
/// Pass criterion: 1e-14 relative.
///
/// *Result (measured 2026-08-05):* `VMIS_CIN1_CHAB` gives
/// `sigma_eq = 3.2000000e8 Pa` at `p = 5.634836e-2`; `VMIS_CIN2_CHAB` with
/// `C2 = 0` gives `sigma_eq = 3.2000000e8 Pa` at `p = 5.634836e-2` — 0.0
/// relative on both. Interpretation: the `nbvar` branches are consistent;
/// neither adds nor drops a term.
#[test]
fn one_back_stress_is_two_with_the_second_modulus_zero() {
    let p = ChabocheParameters::armstrong_frederick(200.0e6, 60.0e9, 500.0);
    let one = ChabocheLaw::VmisCin1Chab(p);
    let two = ChabocheLaw::VmisCin2Chab(p); // c2_asymptotic is 0 by construction

    let (s1, st1) = shear_path(
        one,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    let (s2, st2) = shear_path(
        two,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    println!(
        "CIN1: sigma_eq = {:.7e} Pa, p = {:.6e} | CIN2 with C2=0: sigma_eq = {:.7e} Pa, p = {:.6e}",
        equivalent(s1),
        st1.accumulated_plastic_strain,
        equivalent(s2),
        st2.accumulated_plastic_strain
    );
    assert_relative_eq!(equivalent(s1), equivalent(s2), max_relative = 1e-14);
    assert_relative_eq!(
        st1.accumulated_plastic_strain,
        st2.accumulated_plastic_strain,
        max_relative = 1e-14
    );
}

/// **The Bauschinger effect: reverse yield happens at `C/gamma - R0`.**
///
/// *Methodology:* this is the phenomenon kinematic hardening exists to
/// reproduce, so it deserves a direct test rather than an inference. Load to
/// back-stress saturation in forward shear, then reverse the strain path in
/// small steps and record the equivalent stress at the last elastic point
/// before flow resumes. With the surface centred on `X`, reverse yield must
/// occur at signed equivalent stress
/// `sqrt(3) sigma_xy = ||X||_vm - R0 = C/gamma - R0`, i.e. at a *negative*
/// stress with these parameters — the surface has translated so far that the
/// material re-yields in compression at a stress far smaller in magnitude than
/// the one it reached in tension. An isotropic model would predict reverse
/// yield at minus the forward stress or beyond. Pass criterion: within one
/// reversal step of the analytical value, i.e. 2 MPa.
///
/// *Result (measured 2026-08-05):* forward `sqrt(3) sigma_xy = 3.2000000e8 Pa`;
/// on reversal the last elastic point is at **-7.9704033e7 Pa** against the
/// analytical `C/gamma - R0 = -8.0000000e7 Pa`, a gap of **2.9597e5 Pa** which
/// is well inside the 1.65e6 Pa the stress moves per reversal step. The
/// measured elastic span is **3.9970403e8 Pa** against the theoretical
/// `2 R0 = 4.0000000e8 Pa`. Interpretation: the yield surface really has
/// translated by `C/gamma` and kept its size. The material re-yields in
/// compression at 79.7 MPa having reached 320 MPa in tension — the Bauschinger
/// effect with the right magnitude, not merely the right sign.
#[test]
fn the_bauschinger_effect_reverses_yield_at_the_back_stress_minus_the_radius() {
    let law = af_law();
    let m = law.parameters();
    let (mut stress, mut state) = shear_path(
        law,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    let forward = 3.0_f64.sqrt() * stress.xy;

    let step = ThermoElasticStep::isothermal(steel(), 1.0);
    let d = -2.0e-5;
    let mut last_elastic = forward;
    let mut reverse_yield = f64::NAN;
    for _ in 0..400 {
        let out = law
            .integrate(state, stress, shear(d), step, control())
            .expect("reversal converges");
        if out.yielded {
            reverse_yield = last_elastic;
            break;
        }
        stress = out.stress;
        state = out.state;
        last_elastic = 3.0_f64.sqrt() * stress.xy;
    }
    let analytic = m.c1_asymptotic / m.gamma1_initial - m.r0;
    println!(
        "forward sqrt(3)sigma_xy = {:.7e} Pa, reverse yield at {:.7e} Pa, analytic C/gamma - R0 = {:.7e} Pa (gap {:.4e} Pa), elastic span = {:.7e} Pa (2 R0 = {:.7e} Pa)",
        forward,
        reverse_yield,
        analytic,
        (reverse_yield - analytic).abs(),
        forward - reverse_yield,
        2.0 * m.r0
    );
    assert!((reverse_yield - analytic).abs() < 2.0e6);
}

/// **The viscous overstress follows the Norton relation `K (p_dot)^(1/n)`.**
///
/// *Methodology:* for `VISC_CIN1_CHAB` the flow condition is
/// `||s - X||_vm = R + K (dp/dt)^(1/n)` rather than `= R`. Reconstruct the left
/// side from the returned stress and back strain and compare with the right
/// side computed independently from the returned `dp` and the step duration.
/// Repeat over four decades of strain rate, from `dt = 1e3 s` to `dt = 1e-1 s`,
/// so that the *rate dependence itself* is exercised and not merely one point.
/// Pass criterion: residual below 1e-6 Pa, and the overstress strictly
/// increasing with rate.
///
/// *Result (measured 2026-08-05):* the flow-condition residual is at worst
/// **1.1921e-7 Pa** across the four decades. The overstress rises monotonically
/// — `4.06788e7 Pa` at `dt = 1e3 s` (`dp = 2.151662e-3`), `6.41783e7 Pa` at
/// `1e1 s` (`dp = 2.055755e-3`), `8.05302e7 Pa` at `1e0 s`
/// (`dp = 1.989176e-3`), `1.00951e8 Pa` at `1e-1 s` (`dp = 1.906224e-3`) —
/// giving `sigma_eq` from `3.0287068e8` to `3.5951014e8 Pa`. The overstress
/// ratio over the four decades is **2.481**; the `n = 10` Norton exponent
/// predicts `10^(4/10) = 2.512` at fixed `dp`, and correcting for the measured
/// `dp` falling by the factor 0.886 gives `2.512 * 0.886^0.1 = 2.482`.
/// Interpretation: the viscous branch carries the right exponent and the right
/// `1/K` inversion, to three significant figures on an independent estimate.
#[test]
fn the_viscous_overstress_follows_the_norton_relation() {
    let mut p = ChabocheParameters::armstrong_frederick(200.0e6, 60.0e9, 500.0);
    p.viscous_exponent = 10.0;
    p.viscous_stress = 150.0e6;
    let law = ChabocheLaw::ViscCin1Chab(p);

    let mut worst: f64 = 0.0;
    let mut previous = 0.0;
    for dt in [1.0e3, 1.0e1, 1.0e0, 1.0e-1] {
        let out = law
            .integrate(
                ChabocheState::zero(),
                SymmTensor::default(),
                shear(3.0e-3),
                ThermoElasticStep::isothermal(steel(), dt),
                control(),
            )
            .expect("viscous step converges");
        let residual = flow_condition_residual(law, &out, dt).abs();
        worst = worst.max(residual);
        let overstress =
            p.viscous_stress * (out.equivalent_increment / dt).powf(1.0 / p.viscous_exponent);
        println!(
            "dt = {:>8.1e} s: dp = {:.6e}, overstress = {:.5e} Pa, sigma_eq = {:.7e} Pa, |residual| = {:.4e} Pa",
            dt,
            out.equivalent_increment,
            overstress,
            equivalent(out.stress),
            residual
        );
        assert!(
            overstress > previous,
            "overstress must rise with strain rate"
        );
        previous = overstress;
    }
    assert!(worst < 1.0e-6, "worst residual {worst:e} Pa");
}

/// **The rate-independent law is the slow limit of the rate-dependent one.**
///
/// *Methodology:* the viscous overstress `K (dp/dt)^(1/n)` vanishes as
/// `dt -> infinity`, so `VISC_CIN1_CHAB` must converge to `VMIS_CIN1_CHAB` on
/// the same path with the same parameters. This is the cross-variant structural
/// check that the two branches of `nmchcr.F90`'s `rppmdp` differ *only* by that
/// term. Run the same 20-step path at `dt = 1e2 s` and `dt = 1e80 s` per step
/// and compare with the rate-independent answer. Pass criterion: the slow case
/// within 1e-6 relative of the rate-independent one, and the fast case strictly
/// above it.
///
/// *Result (measured 2026-08-05):* rate-independent
/// `sigma_eq = 3.1866061e8 Pa`; the viscous law at `dt = 1e2 s` gives
/// **3.6342516e8 Pa**, 14.0 % higher — that is the overstress — and at
/// `dt = 1e80 s` gives **3.1866061e8 Pa**, within **2.227e-9** relative of the
/// rate-independent answer. The enormous `dt` is not an artefact: with
/// `n = 10` the overstress decays only as the tenth root of the rate, so four
/// decades of `dt` buy only a factor 2.5. Interpretation: the two variants
/// share one kernel and the viscous term is the only difference, as upstream's
/// code structure implies.
#[test]
fn the_rate_independent_law_is_the_slow_limit_of_the_viscous_law() {
    let mut p = ChabocheParameters::armstrong_frederick(200.0e6, 60.0e9, 500.0);
    p.viscous_exponent = 10.0;
    p.viscous_stress = 150.0e6;

    let rate_free = ChabocheLaw::VmisCin1Chab(p);
    let viscous = ChabocheLaw::ViscCin1Chab(p);

    let (s0, _) = shear_path(
        rate_free,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.01,
        20,
        1.0,
    );
    let (fast, _) = shear_path(
        viscous,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.01,
        20,
        1.0e2,
    );
    let (slow, _) = shear_path(
        viscous,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.01,
        20,
        1.0e80,
    );
    println!(
        "rate-independent sigma_eq = {:.7e} Pa | viscous dt=1e2 {:.7e} Pa | viscous dt=1e80 {:.7e} Pa (rel gap {:.3e})",
        equivalent(s0),
        equivalent(fast),
        equivalent(slow),
        (equivalent(slow) - equivalent(s0)).abs() / equivalent(s0)
    );
    assert!(equivalent(fast) > equivalent(s0));
    assert_relative_eq!(equivalent(slow), equivalent(s0), max_relative = 1e-6);
}

/// **The flow direction follows the *shifted* deviator, not the deviator.**
///
/// *Methodology:* this is the architectural test — the one that distinguishes a
/// real kinematic-hardening law from an isotropic one with an extra unused
/// state variable. Build a back stress by shearing in the `xy` plane, then
/// apply a strain increment in the `xz` plane. If flow followed `s` the
/// direction would be that of the trial deviator; because it follows `s - X` it
/// must instead be that of the shifted deviator, which has a large `xy`
/// component the trial increment did not put there. Pass criterion: the flow
/// direction is parallel to the shifted deviator to 1e-12, and its cosine with
/// the trial deviator is below 0.95.
///
/// *Result (measured 2026-08-05):* after elastically unloading the shear
/// stress to zero and then loading out of plane, the flow direction has cosine
/// **1.000000000000** with the shifted deviator and only **0.826346** with the
/// trial deviator — the flow is **34.3 degrees** away from where an isotropic
/// law would send it. Interpretation: the back stress genuinely steers the
/// flow. This is also the case that would be silently wrong if the shifted
/// deviator were computed once from the elastic predictor and then frozen.
#[test]
fn the_flow_direction_follows_the_shifted_deviator() {
    let law = af_law();
    let (stress, state) = shear_path(
        law,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.02,
        200,
        1.0,
    );

    // Unload elastically back to zero shear stress. The back stress is a state
    // variable and survives, so the next increment starts from a trial deviator
    // that owes nothing to the direction X points in.
    let step = ThermoElasticStep::isothermal(steel(), 1.0);
    let unloaded = law
        .integrate(
            state,
            stress,
            shear(-stress.xy / steel().twice_shear_modulus()),
            step,
            control(),
        )
        .expect("elastic unload");
    assert!(!unloaded.yielded, "the unloading step must stay elastic");
    let (stress, state) = (unloaded.stress, unloaded.state);

    let out_of_plane = SymmTensor::new(0.0, 0.0, 6.5e-4, 0.0, 0.0, 0.0);
    let predictor = law
        .elastic_predictor(state, stress, out_of_plane, step)
        .expect("predictor");
    let out = law
        .integrate(state, stress, out_of_plane, step, control())
        .expect("non-proportional step converges");
    let local = law.local_state(predictor, out.equivalent_increment);

    let cos = |a: SymmTensor, b: SymmTensor| {
        a.double_inner(b) / (a.double_inner(a).sqrt() * b.double_inner(b).sqrt())
    };
    let with_shifted = cos(local.flow_direction, local.effective_deviator);
    let with_trial = cos(local.flow_direction, predictor.trial_deviator);
    println!(
        "cos(flow, shifted deviator) = {:.12}, cos(flow, trial deviator) = {:.6}, angle to trial = {:.1} deg",
        with_shifted,
        with_trial,
        with_trial.acos().to_degrees()
    );
    assert_relative_eq!(with_shifted, 1.0, max_relative = 1e-12);
    assert!(
        with_trial < 0.95,
        "flow must not follow the trial deviator: cos = {with_trial}"
    );
}

/// **Isotropic hardening alone saturates at `R_inf`.**
///
/// *Methodology:* with `C1 = 0` there is no back stress and the law degenerates
/// to von Mises plasticity with the Voce isotropic law
/// `R(p) = R_inf + (R0 - R_inf) e^{-b p}`. Driven far enough, `sigma_eq ->
/// R_inf`. This isolates the isotropic branch, which every other test here has
/// held constant at `R_inf = R0`. Pass criterion: 1e-6 relative.
///
/// *Result (measured 2026-08-05):* with `R0 = 200 MPa`, `R_inf = 350 MPa`,
/// `b = 200`, driving to `p = 1.716884e-1` gives `sigma_eq = 3.5000000e8 Pa`
/// against `R_inf = 3.5000000e8 Pa` — **5.109e-16 relative**, with
/// `exp(-b p) = 1.2227e-15` confirming genuine saturation rather than a lucky
/// tolerance. Interpretation: the Voce expression and its sign are right.
#[test]
fn isotropic_hardening_alone_saturates_at_r_infinity() {
    let mut p = ChabocheParameters::armstrong_frederick(200.0e6, 0.0, 0.0);
    p.r_asymptotic = 350.0e6;
    p.b = 200.0;
    let law = ChabocheLaw::VmisCin1Chab(p);

    let (stress, state) = shear_path(
        law,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.15,
        1200,
        1.0,
    );
    println!(
        "p = {:.6e}, sigma_eq = {:.7e} Pa, R_inf = {:.7e} Pa (rel gap {:.3e}), exp(-b p) = {:.4e}",
        state.accumulated_plastic_strain,
        equivalent(stress),
        p.r_asymptotic,
        (equivalent(stress) - p.r_asymptotic).abs() / p.r_asymptotic,
        (-p.b * state.accumulated_plastic_strain).exp()
    );
    assert_relative_eq!(equivalent(stress), p.r_asymptotic, max_relative = 1e-6);
}

/// **The strain-memory surface grows with the plastic strain and raises the
/// hardening.**
///
/// *Methodology:* `VMIS_CIN2_MEMO` replaces the Voce isotropic law by an
/// integrated radius `R = R0 + R_v` chasing a target
/// `Q(q) = Q_M + (Q0 - Q_M) e^{-2 mu q}` that itself rises as the memory
/// surface of radius `q` is dragged outward by the plastic strain. Three things
/// must therefore hold: `q` must be non-decreasing and bounded by the
/// equivalent plastic strain that produced it; `R_v` must stay within
/// `[0, Q_M]`; and the memory variant must harden more than the plain one on
/// the same path. All are checked while driving to `p ~ 0.04`. Pass criterion:
/// monotonicity exact, bounds strict.
///
/// *Result (measured 2026-08-05):* `q` rose monotonically to **2.808757e-2**
/// while `p = 5.617513e-2` — exactly half, which is what `eta = 0.5` should
/// give on a proportional path. The isotropic increment reached
/// **3.9975747e7 Pa** of its `Q_M = 1.0e8 Pa` ceiling, and
/// `sigma_eq = 3.5997575e8 Pa` against the plain `VMIS_CIN2_CHAB` answer of
/// **3.2000000e8 Pa** on the same path — the memory surface added
/// **3.9976e7 Pa** of extra isotropic hardening, matching the increment
/// exactly. Interpretation: the memory branch is live, is bounded by `Q_M` as
/// designed, and feeds through to the stress rather than being computed and
/// discarded.
#[test]
fn the_strain_memory_surface_raises_the_isotropic_hardening() {
    let mut p = ChabocheParameters::armstrong_frederick(200.0e6, 60.0e9, 500.0);
    p.b = 200.0;
    p.memory_eta = 0.5;
    p.memory_q0 = 0.0;
    p.memory_qm = 100.0e6;
    p.memory_mu = 10.0;
    let memo = ChabocheLaw::VmisCin2Memo(p);
    let plain = ChabocheLaw::VmisCin2Chab(p);

    let mut state = ChabocheState::zero();
    let mut stress = SymmTensor::default();
    let step = ThermoElasticStep::isothermal(steel(), 1.0);
    let mut previous_q = 0.0;
    for _ in 0..400 {
        let out = memo
            .integrate(state, stress, shear(0.05 / 400.0), step, control())
            .expect("memo step converges");
        stress = out.stress;
        state = out.state;
        assert!(
            state.memory.memory_radius >= previous_q - 1e-18,
            "memory radius must not shrink"
        );
        previous_q = state.memory.memory_radius;
    }

    let (plain_stress, _) = shear_path(
        plain,
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    println!(
        "p = {:.6e}, q = {:.6e}, R_v = {:.7e} Pa (Q_M = {:.4e} Pa), sigma_eq memo = {:.7e} Pa, plain = {:.7e} Pa",
        state.accumulated_plastic_strain,
        state.memory.memory_radius,
        state.memory.isotropic_increment,
        p.memory_qm,
        equivalent(stress),
        equivalent(plain_stress)
    );
    assert!(state.memory.memory_radius > 0.0);
    assert!(state.memory.memory_radius <= state.accumulated_plastic_strain);
    assert!(state.memory.isotropic_increment > 0.0);
    assert!(state.memory.isotropic_increment <= p.memory_qm);
    assert!(equivalent(stress) > equivalent(plain_stress));
}

/// **`delta = 1` switches the non-radial correction off exactly.**
///
/// *Methodology:* upstream's `nmcham.F90` sets its `idelta` switch to zero when
/// both `DELTA1` and `DELTA2` equal one, and `nmchcr.F90` then leaves
/// `n1 = n2 = 1`. This port makes the same decision per-coefficient. Check that
/// an explicitly-set `delta = 1` gives exactly the default answer, and report
/// the `n1` factor a genuine `delta = 0.5` produces, so the branch is shown to
/// be both correctly bypassed and actually reachable. Pass criterion: 1e-14
/// relative for `delta = 1`.
///
/// *Result (measured 2026-08-05):* the default parameters and an explicit
/// `delta = 1` both give `sigma_eq = 3.2000000e8 Pa` at **0.000e0** relative
/// difference, so the bypass is exact. `delta1 = 0.5` also gives
/// `3.2000000e8 Pa` (1.863e-16 relative) — expected, and worth stating
/// plainly: at saturation the back strain satisfies `alpha:n/sqrt(3/2) = 1/gamma`,
/// which makes `n1` collapse to `delta` and cancels `delta` out of the
/// saturated back stress entirely, so **a radial path cannot discriminate on
/// the stress**. The probe confirms the branch is nevertheless reached and
/// returns a different factor: `n1 = 1.000000` for `delta = 1` and
/// `n1 = 0.500000` for `delta = 0.5`, the latter equal to `delta` itself as the
/// algebra predicts. Interpretation: the non-radial branch is wired correctly
/// and is inert exactly where theory says it should be. Exercising its effect
/// on the stress needs a non-proportional path, and none is asserted here.
#[test]
fn delta_one_switches_the_non_radial_correction_off() {
    let base = ChabocheParameters::armstrong_frederick(200.0e6, 60.0e9, 500.0);
    let mut explicit = base;
    explicit.delta1 = 1.0;
    let mut nonradial = base;
    nonradial.delta1 = 0.5;

    let (s_base, st_base) = shear_path(
        ChabocheLaw::VmisCin1Chab(base),
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    let (s_explicit, _) = shear_path(
        ChabocheLaw::VmisCin1Chab(explicit),
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );
    let (s_nr, st_nr) = shear_path(
        ChabocheLaw::VmisCin1Chab(nonradial),
        ChabocheState::zero(),
        SymmTensor::default(),
        0.05,
        400,
        1.0,
    );

    let probe = |law: ChabocheLaw, state: ChabocheState, stress: SymmTensor| {
        let step = ThermoElasticStep::isothermal(steel(), 1.0);
        let predictor = law
            .elastic_predictor(state, stress, shear(1.25e-4), step)
            .expect("predictor");
        let out = law
            .integrate(state, stress, shear(1.25e-4), step, control())
            .expect("step");
        law.local_state(predictor, out.equivalent_increment)
            .non_radial_factor[0]
    };
    println!(
        "sigma_eq: default {:.7e} Pa, delta=1 {:.7e} Pa (rel gap {:.3e}), delta=0.5 {:.7e} Pa (rel gap {:.3e})",
        equivalent(s_base),
        equivalent(s_explicit),
        (equivalent(s_explicit) - equivalent(s_base)).abs() / equivalent(s_base),
        equivalent(s_nr),
        (equivalent(s_nr) - equivalent(s_base)).abs() / equivalent(s_base)
    );
    println!(
        "n1 at the next step: delta=1 {:.6}, delta=0.5 {:.6}",
        probe(ChabocheLaw::VmisCin1Chab(explicit), st_base, s_base),
        probe(ChabocheLaw::VmisCin1Chab(nonradial), st_nr, s_nr)
    );
    assert_relative_eq!(
        equivalent(s_explicit),
        equivalent(s_base),
        max_relative = 1e-14
    );
}

/// **A change of elastic moduli across the step rescales the incoming stress.**
///
/// *Methodology:* `nmchab.F90` rescales the start-of-step stress by the ratio
/// of the new to the old moduli — deviatorically by `2mu+/2mu-` and
/// hydrostatically by `3K+/3K-` — so that the *elastic strain* the stress
/// represents is preserved when `E(T)` changes. With a zero strain increment
/// the whole answer is that rescaling and can be checked in closed form. Halve
/// Young's modulus across the step at fixed `nu` and compare. Pass criterion:
/// 1e-12 relative.
///
/// *Result (measured 2026-08-05):* starting from `sigma_xy = 1.0000e8 Pa` and
/// `tr(sigma)/3 = 5.0000e7 Pa`, with `E` halved from 200 GPa to 100 GPa at
/// fixed `nu = 0.3`, the step returns `sigma_xy = 5.0000000e7 Pa` and
/// `tr(sigma)/3 = 2.5000000e7 Pa` — both exactly halved, 0.0 relative.
/// Interpretation: the rescaling applies the right ratio to the right part.
/// Getting this wrong would inject a spurious stress increment into every step
/// of a thermal transient, which is exactly the regime this crate targets.
#[test]
fn a_change_of_moduli_rescales_the_incoming_stress() {
    let law = af_law();
    let hot = ElasticModuli::new(100.0e9, 0.3).expect("valid moduli");
    let start = SymmTensor::new(50.0e6, 100.0e6, 0.0, 50.0e6, 0.0, 50.0e6);

    let out = law
        .integrate(
            ChabocheState::zero(),
            start,
            SymmTensor::default(),
            ThermoElasticStep {
                start: steel(),
                end: hot,
                thermal_strain_increment: 0.0,
                dt: 1.0,
            },
            control(),
        )
        .expect("rescaling step");
    println!(
        "start: sigma_xy = {:.4e} Pa, mean = {:.4e} Pa | after halving E: sigma_xy = {:.7e} Pa, mean = {:.7e} Pa",
        start.xy,
        start.tr() / 3.0,
        out.stress.xy,
        out.stress.tr() / 3.0
    );
    assert_relative_eq!(out.stress.xy, 0.5 * start.xy, max_relative = 1e-12);
    assert_relative_eq!(
        out.stress.tr() / 3.0,
        0.5 * start.tr() / 3.0,
        max_relative = 1e-12
    );
}

/// **A free thermal expansion produces no stress.**
///
/// *Methodology:* free thermal expansion is stress-free. Applying a total
/// strain increment equal to the thermal strain increment must therefore leave
/// the stress exactly where it was. This checks the sign of the
/// `depsth = deps - coef*kron` subtraction, which a sign slip would turn into a
/// doubled thermal stress rather than a cancelled one. Pass criterion: 1e-12
/// relative on both parts of the stress.
///
/// *Result (measured 2026-08-05):* with `dEps_th = 1e-3` applied as an
/// isotropic total strain increment of the same size, the step returns
/// `sigma_xy = 1.0000000e8 Pa` and `tr(sigma)/3 = 5.0000000e7 Pa`, unchanged
/// from the inputs `1.0000e8 Pa` and `5.0000e7 Pa` to 0.0 relative.
/// Interpretation: the thermal term is subtracted rather than added, and only
/// from the volumetric part.
#[test]
fn free_thermal_expansion_produces_no_stress() {
    let law = af_law();
    let start = SymmTensor::new(50.0e6, 100.0e6, 0.0, 50.0e6, 0.0, 50.0e6);
    let dth = 1.0e-3;

    let out = law
        .integrate(
            ChabocheState::zero(),
            start,
            SymmTensor::from_diag(dth, dth, dth),
            ThermoElasticStep {
                start: steel(),
                end: steel(),
                thermal_strain_increment: dth,
                dt: 1.0,
            },
            control(),
        )
        .expect("thermal step");
    println!(
        "thermal-only step: sigma_xy = {:.7e} Pa (was {:.4e}), mean = {:.7e} Pa (was {:.4e})",
        out.stress.xy,
        start.xy,
        out.stress.tr() / 3.0,
        start.tr() / 3.0
    );
    assert_relative_eq!(out.stress.xy, start.xy, max_relative = 1e-12);
    assert_relative_eq!(
        out.stress.tr() / 3.0,
        start.tr() / 3.0,
        max_relative = 1e-12
    );
}

/// **Unphysical inputs are rejected rather than propagated.**
///
/// *Methodology:* three guards, each mirroring an upstream fatal error or an
/// invariant the algebra depends on — a non-positive Young's modulus, a Poisson
/// ratio at the incompressible limit (where `3K` is infinite), and a negative
/// timestep. Pass criterion: each returns the documented `OffbeatError` variant
/// rather than a number.
///
/// *Result (measured 2026-08-05):* all three rejected. A negative Young's
/// modulus returns `OffbeatError::Unphysical`, `nu = 0.5` returns
/// `OffbeatError::OutOfRange`, and the negative timestep returns
/// `unphysical input for timestep: -1 s (must not be negative)`.
/// Interpretation: the failure modes that would otherwise surface as an
/// infinity or a NaN deep inside the local solve are caught at the boundary.
#[test]
fn unphysical_inputs_are_rejected() {
    assert!(matches!(
        ElasticModuli::new(-1.0, 0.3),
        Err(OffbeatError::Unphysical { .. })
    ));
    assert!(matches!(
        ElasticModuli::new(200.0e9, 0.5),
        Err(OffbeatError::OutOfRange { .. })
    ));
    let bad = ThermoElasticStep::isothermal(steel(), -1.0);
    let err = af_law()
        .integrate(
            ChabocheState::zero(),
            SymmTensor::default(),
            shear(1.0e-3),
            bad,
            control(),
        )
        .expect_err("negative timestep must be rejected");
    println!("rejected: {err}");
    assert!(matches!(err, OffbeatError::Unphysical { .. }));
}
