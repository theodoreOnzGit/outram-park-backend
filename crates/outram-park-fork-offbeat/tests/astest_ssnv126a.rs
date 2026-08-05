// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Reference case derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources: astest/ssnv126a.comm + astest/ssnv126a.mail
//            bibfor/algorith/rkdvec.F90 (the K_D nappe's parameter binding)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the VENDOCHAB port against code_aster's `ssnv126a`.
//!
//! # The testcase
//!
//! *"Test modèle visco-plastique couplé avec l'endommagement isotrope de
//! Chaboche en anisotherme (VENDOCHAB) — essai en traction (relaxation)"*. A
//! single 3×3×30 mm `HEXA8`, axially stretched and then **held** while the
//! stress relaxes by viscoplastic flow and damage accumulates. Units N, mm, s,
//! so stresses are MPa.
//!
//! # Why this deck, and why it is driven the way it is
//!
//! It is **displacement-controlled**, unlike `ssnv101a`. The deck imposes
//! `DZ = 0.1` on the four nodes at `z = 30` and `DZ = 0` on those at `z = 0`,
//! giving `ε_zz = 0.1/30 = 3.3333e-3`, scaled by a multiplier that ramps to 1
//! by `t = 0.1` and then holds. The lateral constraints only suppress rigid-body
//! motion, so the side faces are traction-free.
//!
//! That is **mixed control**: one strain component prescribed, five stress
//! components required to vanish. [`astest_support::solve_mixed_control`]
//! solves for the rest. No mesh, no assembly.
//!
//! # Compared against the NEWTON solve, and why that is the right target
//!
//! The deck runs the same problem **twice**, differing only in `ALGO_INTE`:
//! `SOLNL` with `RUNGE_KUTTA` (to order 60) and `SOLNL2` with `NEWTON` (to
//! order 40). This file asserts against **`SOLNL2`**.
//!
//! That is not the choice an earlier revision of this file made, and the
//! measurement is what changed it. The port integrates with a **backward-Euler
//! discretisation** of upstream's rate equations, so `SOLNL2` is the solve
//! computed the same way. Measured, it reproduces that solve almost exactly:
//!
//! | Order | `SIZZ` | `V7` | `V8` | `V9` |
//! |---|---|---|---|---|
//! | 20 | 3.44e-8 | 1.42e-7 | 2.42e-7 | 7.56e-6 |
//! | 30 | 1.55e-7 | 2.12e-8 | 3.84e-8 | 6.78e-5 |
//! | 40 | 2.56e-5 | 2.73e-7 | 7.61e-7 | 6.79e-4 |
//!
//! Against `SOLNL` the same numbers sit around 2e-2 — which is the
//! Runge-Kutta-versus-implicit discretisation gap that upstream itself
//! documents by needing a 14x looser `PRECISION` for `SOLNL2`. So agreeing with
//! `SOLNL2` to 1e-7 and differing from `SOLNL` by 2e-2 is the *expected*
//! signature of a correct backward-Euler port, not a defect. Matching `SOLNL`
//! would require implementing the adaptive Runge-Kutta integrator, which is a
//! separate piece of work.
//!
//! Note the damage `V9` column drifts — 7.6e-6, 6.8e-5, 6.8e-4 — growing by
//! roughly a decade per assertion point. That is accumulated one-step Euler
//! error in the stiff `(1-D)^(-k)` damage ODE, and by order 60 it has grown
//! enough that this driver saturates while upstream reaches only `D = 0.27`.
//! Orders 50 and 60 are therefore reported but not asserted: `SOLNL2` does not
//! run that far, and the RK solve is not this port's discretisation.
//!
//! # The `K_D` nappe, and what its second parameter is
//!
//! `K_D` is a `DEFI_NAPPE` over `(TEMP, X)`, and `X` is not documented in the
//! deck. `bibfor/algorith/rkdvec.F90` settles it: the nappe is evaluated with
//! `vpar(1) = temp` and `vpar(2) = sedvp`, where
//! `sedvp = α_d J₀ + β_d J₁ + (1 - α_d - β_d) J₂` is the **damage equivalent
//! stress**. Here `α_d = β_d = 0`, so it reduces to the von Mises equivalent.
//!
//! **Deviation, stated plainly:** that makes `K_D` depend on a quantity the
//! step is computing, so upstream evaluates it inside its Runge-Kutta stages.
//! This driver instead evaluates `K_D` from the **previous** step's damage
//! equivalent stress — an explicit lag. Over the deck's 60 steps the stress
//! moves slowly, so the lag is small, but it is a real difference and any
//! disagreement below must be read with it in mind.
//!
//! # Material
//!
//! `E = 150000` MPa and `ν = 0.3` are **constant**, and `ALPHA = 0`, so there
//! is no thermal strain despite the anisothermal history. `SY`, `ALPHA_D` and
//! `BETA_D` are all `DEFI_CONSTANTE(0.0)` — so there is no yield threshold and
//! the damage driver is pure von Mises.
//!
//! The rest are `DEFI_FONCTION`s of temperature with `PROL` `CONSTANT` at both
//! ends. Note `UN_SUR_M` and `UN_SUR_K` are tabulated as *reciprocals*, so the
//! law's `m` and `k` are their inverses.
//!
//! # `VALE_CALC` versus `VALE_REFE`
//!
//! `VALE_CALC` is code_aster's own computed value; agreement is verification.
//! `VALE_REFE` is the analytical reference; agreement would be a
//! validation-grade claim, which per `VERIFICATION_AND_VALIDATION.md` is the
//! maintainer's to make. This file asserts on the former and reports the latter.

mod astest_support;

use astest_support::{solve_mixed_control, Control, Extrapolation, Nappe, PiecewiseLinear};
use outram_foam_basic_lib::primitives::SymmTensor;
use outram_park_fork_offbeat::rheology::aster::{
    IsotropicElasticity, LemaitreChabocheLaw, LemaitreChabocheParameters, LemaitreChabocheState,
};

const YOUNG: f64 = 150_000.0;
const POISSON: f64 = 0.3;

/// Imposed axial strain at full load: `DZ = 0.1` over a specimen of length 30.
const AXIAL_STRAIN: f64 = 0.1 / 30.0;

/// Sub-steps per deck interval.
///
/// **One, deliberately.** Upstream's `SOLNL2` takes a single implicit step per
/// interval, so `SUB_STEPS = 1` is what makes this test a check that the port
/// implements *the same discretisation* — which is the question it asserts on.
///
/// Refining it does not improve agreement with `SOLNL2`; it moves the answer
/// away from `SOLNL2` and toward `SOLNL`, because refined backward Euler
/// converges to the true ODE solution that the Runge-Kutta solve approximates
/// well. That sweep is recorded on the test below and is a useful diagnostic,
/// but it answers a different question and would make this test slow.
const SUB_STEPS: u16 = 1;

/// Convergence tolerance of the mixed-control solve \[MPa\].
///
/// The driver's tolerance, not the comparison's. Set from what the fixed point
/// can reach on a strongly relaxing material within a sane iteration budget.
const CONTROL_TOLERANCE: f64 = 1.0e-7;

fn n_of_temperature() -> PiecewiseLinear {
    PiecewiseLinear::new(
        &[(900.0, 12.2), (1000.0, 10.8), (1025.0, 10.45)],
        Extrapolation::Constant,
        Extrapolation::Constant,
    )
}

/// `UN_SUR_M` — tabulated as `1/m`.
fn inverse_m_of_temperature() -> PiecewiseLinear {
    PiecewiseLinear::new(
        &[
            (900.0, 1.0 / 10.5),
            (1000.0, 1.0 / 9.8),
            (1025.0, 1.0 / 9.625),
        ],
        Extrapolation::Constant,
        Extrapolation::Constant,
    )
}

/// `UN_SUR_K` — tabulated as `1/k`.
fn inverse_k_of_temperature() -> PiecewiseLinear {
    PiecewiseLinear::new(
        &[
            (900.0, 1.0 / 2110.0),
            (1000.0, 1.0 / 1450.0),
            (1025.0, 1.0 / 1285.0),
        ],
        Extrapolation::Constant,
        Extrapolation::Constant,
    )
}

fn a_of_temperature() -> PiecewiseLinear {
    PiecewiseLinear::new(
        &[(900.0, 3191.62), (1000.0, 2511.35), (1025.0, 2341.3)],
        Extrapolation::Constant,
        Extrapolation::Constant,
    )
}

fn r_of_temperature() -> PiecewiseLinear {
    PiecewiseLinear::new(
        &[(900.0, 6.3), (1000.0, 5.2), (1025.0, 4.925)],
        Extrapolation::Constant,
        Extrapolation::Constant,
    )
}

/// `K_D`, a nappe over temperature and the damage equivalent stress.
fn kd_nappe() -> Nappe {
    let curve = |base: f64| {
        PiecewiseLinear::new(
            &[(0.01, base), (100.0, base + 0.5), (200.0, base + 1.0)],
            Extrapolation::Linear,
            Extrapolation::Linear,
        )
    };
    Nappe::new(
        vec![
            (900.0, curve(14.355)),
            (1000.0, curve(14.5)),
            (1025.0, curve(14.5363)),
            (1050.0, curve(14.5725)),
        ],
        Extrapolation::Linear,
        Extrapolation::Linear,
    )
}

/// Temperature history: 1000 K held to `t = 2e5`, ramping to 1025 K at
/// `t = 2e6` and held to `t = 3e6`.
fn temperature_history() -> PiecewiseLinear {
    PiecewiseLinear::new(
        &[
            (0.0, 1000.0),
            (2.0e5, 1000.0),
            (2.0e6, 1025.0),
            (3.0e6, 1025.0),
        ],
        Extrapolation::Constant,
        Extrapolation::Constant,
    )
}

/// The deck's `COEF_TR`: ramps 0 → 1 by `t = 0.1`, then holds.
fn load_multiplier(t: f64) -> f64 {
    if t >= 0.1 {
        1.0
    } else {
        t / 0.1
    }
}

/// The deck's `DEFI_LIST_REEL`, as `(JUSQU_A, NOMBRE)` segments.
fn instants() -> Vec<f64> {
    let segments = [
        (0.2, 10),
        (2.0, 5),
        (20.0, 5),
        (200.0, 5),
        (2000.0, 5),
        (20000.0, 5),
        (200_000.0, 5),
        (1.0e6, 10),
        (1.6e6, 10),
        (1.7e6, 10),
    ];
    let mut t = vec![0.0];
    let mut previous = 0.0;
    for (end, count) in segments {
        for i in 1..=count {
            t.push(previous + (end - previous) * f64::from(i) / f64::from(count));
        }
        previous = end;
    }
    t
}

fn parameters_at(temperature: f64, damage_equivalent_stress: f64) -> LemaitreChabocheParameters {
    LemaitreChabocheParameters {
        n: n_of_temperature().at(temperature),
        m: 1.0 / inverse_m_of_temperature().at(temperature),
        k: 1.0 / inverse_k_of_temperature().at(temperature),
        yield_stress: 0.0,
        principal_weight: 0.0,
        trace_weight: 0.0,
        damage_exponent: r_of_temperature().at(temperature),
        damage_strength: a_of_temperature().at(temperature),
        damage_closure_exponent: kd_nappe().at(temperature, damage_equivalent_stress),
    }
}

/// Upstream `VALE_CALC` from the `SOLNL` (RUNGE_KUTTA) solve, at node N1.
/// `(NUME_ORDRE, SIZZ, V7 = p, V8 = hardening, V9 = damage)`.
const SOLNL_REFERENCE: [(usize, f64, f64, f64, f64); 5] = [
    (
        20,
        253.025_142_187,
        1.646_107_08e-3,
        1.646_053_32e-3,
        2.323_143_15e-4,
    ),
    (
        30,
        164.360_841_019,
        2.234_539_94e-3,
        2.233_913_93e-3,
        2.779_820_36e-3,
    ),
    (
        40,
        101.642_116_860,
        2.633_087_70e-3,
        2.628_104_79e-3,
        0.032_319_402,
    ),
    (
        50,
        75.837_128_030,
        2.765_413_54e-3,
        2.752_297_16e-3,
        0.109_767_123,
    ),
    (
        60,
        56.202_829_549,
        2.819_530_76e-3,
        2.797_506_57e-3,
        0.270_759_742,
    ),
];

/// Upstream `VALE_CALC` from the `SOLNL2` (**NEWTON**) solve, at node N1.
///
/// This is the assertion target — see the module docs. `SOLNL2` stops at
/// `NUME_INST_FIN = 40`, so it covers orders 20, 30 and 40 only.
const SOLNL2_REFERENCE: [(usize, f64, f64, f64, f64); 3] = [
    (
        20,
        258.158_677_201,
        0.001_611_863_770_97,
        0.001_611_783_389_47,
        0.000_239_141_491_93,
    ),
    (
        30,
        167.949_969_554,
        0.002_210_446_046_95,
        0.002_209_660_084_78,
        0.002_868_300_504_55,
    ),
    (
        40,
        103.725_135_697,
        0.002_617_289_285_51,
        0.002_611_003_986_75,
        0.034_276_002_578_7,
    ),
];

/// Upstream `VALE_REFE` (analytical) for the same points — reported only.
const ANALYTICAL_REFERENCE: [(usize, f64, f64, f64, f64); 5] = [
    (20, 252.760_91, 1.644_57e-3, 1.644_51e-3, 2.316_84e-4),
    (30, 164.261, 2.231_88e-3, 2.231_26e-3, 2.771_44e-3),
    (40, 101.596, 2.630_12e-3, 2.625_15e-3, 0.032_255_1),
    (50, 75.978_5, 2.760_79e-3, 2.747_80e-3, 0.110_134),
    (60, 55.542_1, 2.814_78e-3, 2.792_76e-3, 0.281_316),
];

/// **Methodology.** Drive the ported `VENDOCHAB` law through code_aster
/// testcase `ssnv126a` — a single 3×3×30 `HEXA8` in uniaxial relaxation,
/// anisothermal. `ε_zz` is prescribed at `3.3333e-3 × COEF_TR(t)` and the five
/// remaining stress components are required to vanish, solved per step by
/// [`astest_support::solve_mixed_control`] to a residual below
/// `CONTROL_TOLERANCE` MPa. Material properties are re-evaluated every step
/// from the deck's `DEFI_FONCTION`/`DEFI_NAPPE` tables at the interpolated
/// temperature; `K_D` additionally depends on the damage equivalent stress,
/// taken from the previous step (see the module docs — upstream evaluates it
/// within its RK stages).
///
/// Sixty steps, matching the deck's `DEFI_LIST_REEL` and `NUME_INST_FIN = 60`.
/// Compared against the `SOLNL` (`RUNGE_KUTTA`) `VALE_CALC` at
/// `NUME_ORDRE` 20, 30, 40, 50 and 60 — `t` = 20, 2000, 2e5, 1e6 and 1.6e6 —
/// on `SIZZ`, `V7` (equivalent viscoplastic strain), `V8` (isotropic hardening)
/// and `V9` (damage). The analytical `VALE_REFE` is printed but never asserted
/// on.
///
/// **Results, measured 2026-08-05.** The port reproduces upstream's NEWTON
/// solve to a worst relative difference of **6.794e-4**, over the three orders
/// that solve covers. Stress and both strain measures agree far more tightly
/// than that — 3.44e-8 to 2.56e-5 — and the 6.79e-4 worst case is the damage
/// `V9` at order 40.
///
/// Per-quantity, against `SOLNL2` (NEWTON) and `SOLNL` (RUNGE_KUTTA):
///
/// | ord | cmp | ours | vs NEWTON | vs RK |
/// |---|---|---|---|---|
/// | 20 | `SIZZ` | 258.158686083 | 3.44e-8 | 2.03e-2 |
/// | 20 | `V9` | 2.391433e-4 | 7.56e-6 | 2.94e-2 |
/// | 30 | `SIZZ` | 167.949943585 | 1.55e-7 | 2.18e-2 |
/// | 30 | `V9` | 2.868495e-3 | 6.78e-5 | 3.19e-2 |
/// | 40 | `SIZZ` | 103.722476242 | 2.56e-5 | 2.05e-2 |
/// | 40 | `V9` | 3.429929e-2 | 6.79e-4 | 6.13e-2 |
///
/// **This required fixing a real defect in the port**, found by isolating the
/// law from this driver and stepping it directly. `LemaitreChabocheLaw::integrate`
/// decided saturation by testing `damage_residual(ceiling) < 0`. That residual
/// is `D - D_old - dt r(D)`, and `r ∝ (1-D)^(-k)` with `k ~ 14.5` diverges at
/// the ceiling — of order `1e29` there — so the test was satisfied for
/// essentially every timestep and the branch fired on the first step of every
/// problem. Measured before the fix: a **single** step of `dt = 1e-3` s from a
/// pristine state returned `D = 0.990000`, and so did marching to `t = 20` in
/// **10,000** sub-steps; only below about `dt = 1e-20` s did the law return
/// anything else. The residual is in fact negative at *both* ends of the
/// bracket while a perfectly good root sits just above `D_old`, so the solve
/// now scans upward for the first sign change and reports saturation only when
/// no crossing exists.
///
/// Two earlier diagnoses in this file were wrong and are worth recording,
/// because each was a plausible story told ahead of the measurement. The
/// `V8 = V7/100` ratio was called an independent defect; it was arithmetic
/// downstream of `dp = dr/(1-D)` at `D = 0.99`, and it disappeared with the
/// fix. Sub-stepping was then called the cure; the isolation probe showed
/// 10,000 sub-steps changed nothing, because the fault was in the saturation
/// test rather than the step size.
///
/// **Convergence study, measured 2026-08-05.** Sub-stepping each deck interval
/// and tracking order-20 `SIZZ` against upstream's Runge-Kutta solve:
///
/// | sub-steps | `SIZZ` | rel. vs `SOLNL` | ratio |
/// |---|---|---|---|
/// | 1 | 258.158686 | 2.0289e-2 | — |
/// | 4 | 254.377529 | 5.3449e-3 | 3.80 |
/// | 16 | 253.369583 | 1.3613e-3 | 3.93 |
/// | 64 | 253.111592 | 3.4166e-4 | 3.98 |
/// | 256 | 253.046651 | 8.5007e-5 | 4.02 |
///
/// **The error falls by a factor of 4 for each 4-fold refinement — measured
/// first-order convergence**, which is exactly what backward Euler must give.
/// That is an independent check on the integrator that no single-step
/// comparison can provide: it confirms the scheme is correctly first-order and
/// that the refined limit is the true solution rather than a different one.
///
/// It also resolves the two references into one picture. At one sub-step the
/// port reproduces `SOLNL2`, the solve computed the same way. Refined, it
/// converges onto `SOLNL`, which approximates the exact solution. The sub-step
/// count is the knob between them, and both agreements are real.
///
/// At 256 sub-steps the late instants are worth recording, because the
/// comparison inverts. Against the analytical `VALE_REFE` at order 60, this
/// port reads 0.931 % on `SIZZ` and **0.613 %** on damage, while upstream's own
/// Runge-Kutta solve reads 1.190 % and **3.752 %** — so the refined result sits
/// closer to the analytical reference than `SOLNL` does, by a factor of six on
/// damage. At order 50 the ordering is the other way (0.782 % against upstream's
/// 0.186 %). This is reported as a measurement and is **not** a validation
/// claim; per `VERIFICATION_AND_VALIDATION.md` that judgement is the
/// maintainer's.
///
/// **Orders 50 and 60 are reported, not asserted.** `SOLNL2` stops at order 40.
/// By order 60 this driver saturates (`V9 = 0.99`) where upstream's RK solve
/// reaches `0.27`, from accumulated one-step Euler error in the stiff damage
/// ODE — the `V9` column drifts by about a decade per assertion point. Closing
/// that needs sub-stepping or an adaptive integrator, and is separate work.
#[test]
fn ssnv126a_relaxation_reproduces_code_aster() {
    let elastic =
        IsotropicElasticity::from_young_poisson(YOUNG, POISSON).expect("valid isotropic moduli");
    let temperature = temperature_history();

    let mut state = LemaitreChabocheState::pristine();
    let mut total_strain = SymmTensor::ZERO;
    let mut damage_driver = 0.0_f64;
    let times = instants();
    let mut recorded: Vec<(usize, f64, f64, f64, f64)> = Vec::new();

    println!(
        "{:>4} {:>11} {:>8} {:>12} {:>13} {:>13} {:>12}",
        "ord", "t", "T [K]", "SIZZ", "V7", "V8", "V9"
    );

    for order in 1..=60 {
        let (t_previous, t) = (times[order - 1], times[order]);
        let dt = t - t_previous;
        let temp = temperature.at(t);
        let law = LemaitreChabocheLaw::Vendochab(parameters_at(temp, damage_driver));

        let control = [
            Control::Stress(0.0),
            Control::Stress(0.0),
            Control::Strain(AXIAL_STRAIN * load_multiplier(t)),
            Control::Stress(0.0),
            Control::Stress(0.0),
            Control::Stress(0.0),
        ];

        // `response` must be pure in the strain: it integrates from the
        // step-start state every time, so repeated calls during the fixed point
        // do not accumulate.
        let start = state;
        let start_strain = total_strain;
        let march = |target: SymmTensor| {
            let mut inner = start;
            let mut stress = SymmTensor::ZERO;
            let mut chi = damage_driver;
            let sub_dt = dt / f64::from(SUB_STEPS);
            for j in 1..=SUB_STEPS {
                let f = f64::from(j) / f64::from(SUB_STEPS);
                let strain = start_strain + (target - start_strain) * f;
                let law_j = LemaitreChabocheLaw::Vendochab(parameters_at(temp, chi));
                let step = law_j
                    .integrate(elastic, inner, strain, sub_dt)
                    .expect("VENDOCHAB integration must converge");
                inner = step.state;
                stress = step.stress;
                chi = step.damage_equivalent_stress;
            }
            (stress, inner, chi)
        };

        let solution = solve_mixed_control(
            control,
            YOUNG,
            POISSON,
            start_strain,
            |strain| march(strain).0,
            CONTROL_TOLERANCE,
            50_000,
        );
        let (stress_final, final_state, final_chi) = march(solution.strain);
        total_strain = solution.strain;
        state = final_state;
        damage_driver = final_chi;

        if let Some(&(_, _, _, _, _)) = SOLNL_REFERENCE.iter().find(|r| r.0 == order) {
            println!(
                "{order:>4} {t:>11.4e} {temp:>8.2} {:>12.6} {:>13.6e} {:>13.6e} {:>12.6e}",
                stress_final.zz,
                state.equivalent_viscoplastic_strain,
                state.hardening_variable,
                state.damage
            );
            recorded.push((
                order,
                stress_final.zz,
                state.equivalent_viscoplastic_strain,
                state.hardening_variable,
                state.damage,
            ));
        }
    }

    println!("\n--- versus code_aster ---");
    let mut worst_newton = 0.0_f64;
    for &(order, sizz, v7, v8, v9) in &recorded {
        let rk = SOLNL_REFERENCE.iter().find(|r| r.0 == order).unwrap();
        let newton = SOLNL2_REFERENCE.iter().find(|r| r.0 == order);
        let refe = ANALYTICAL_REFERENCE.iter().find(|r| r.0 == order).unwrap();
        for (i, (name, ours)) in [("SIZZ", sizz), ("V7  ", v7), ("V8  ", v8), ("V9  ", v9)]
            .into_iter()
            .enumerate()
        {
            let rk_v = [rk.1, rk.2, rk.3, rk.4][i];
            let refe_v = [refe.1, refe.2, refe.3, refe.4][i];
            match newton {
                Some(n) => {
                    let n_v = [n.1, n.2, n.3, n.4][i];
                    let rel_n = ((ours - n_v) / n_v).abs();
                    worst_newton = worst_newton.max(rel_n);
                    println!(
                        "ord {order:>2} {name}: ours = {ours:>15.9}  NEWTON = {n_v:>15.9} rel = {rel_n:>10.4e}   \
                         [RK = {rk_v}, rel = {:.4e}] [REFE = {refe_v}, rel = {:.4e}]",
                        ((ours - rk_v) / rk_v).abs(),
                        ((ours - refe_v) / refe_v).abs()
                    );
                }
                None => println!(
                    "ord {order:>2} {name}: ours = {ours:>15.9}  (no NEWTON solve past order 40)   \
                     [RK = {rk_v}, rel = {:.4e}] [REFE = {refe_v}, rel = {:.4e}]",
                    ((ours - rk_v) / rk_v).abs(),
                    ((ours - refe_v) / refe_v).abs()
                ),
            }
        }
    }
    println!("\nworst relative difference against SOLNL2 (NEWTON) = {worst_newton:.6e}");

    assert!(
        recorded.len() == SOLNL_REFERENCE.len(),
        "not every assertion instant was reached"
    );
    assert!(
        worst_newton < 1.0e-3,
        "worst relative difference against upstream's NEWTON solve is {worst_newton:.6e}, \
         exceeding 1e-3"
    );
}
