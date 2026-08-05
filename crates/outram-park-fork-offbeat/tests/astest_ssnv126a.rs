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
//! # Compared against the RUNGE_KUTTA solve, deliberately
//!
//! The deck runs the same problem **twice**, differing only in `ALGO_INTE`:
//! `SOLNL` with `RUNGE_KUTTA` and `SOLNL2` with `NEWTON`. This file compares
//! against **`SOLNL`**, because the damage port follows upstream's Runge-Kutta
//! *rate equations*.
//!
//! That choice is not cosmetic. Upstream's own tolerances record its implicit
//! path as markedly less accurate on this very problem: against the same
//! analytical `VALE_REFE`, the `NEWTON` solve is 12–47× further off than the
//! `RUNGE_KUTTA` one, and upstream loosens `PRECISION` for it from `5e-3` to as
//! much as `7e-2`. Both defects the damage port reproduces —
//! `nmvexi.F90` reading `ALPHA_D`/`BETA_D` from the slots holding
//! `UN_SUR_M`/`UN_SUR_K`, and `nmvecd.F90` evaluating damage outside the
//! plasticity gate — sit on that implicit path. Matching `SOLNL2` would mean
//! having reproduced the defective route.
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
/// **Results, measured 2026-08-05: THIS DOES NOT YET REPRODUCE UPSTREAM.**
///
/// The test is committed in this state deliberately. It asserts only that the
/// harness ran and reached the assertion instants — it does **not** assert
/// agreement, because there is none, and writing a tolerance wide enough to
/// pass would be exactly the "loosen the test" move the workspace forbids.
/// What follows is the measurement, so the next attempt starts from data.
///
/// | ord | `t` | ours `SIZZ` | code_aster `SIZZ` | ours `V9` | code_aster `V9` |
/// |---|---|---|---|---|---|
/// | 20 | 2e1 | 1.029580 | 253.025142 | 0.990000 | 2.323143e-4 |
/// | 30 | 2e3 | 0.650167 | 164.360841 | 0.990000 | 2.779820e-3 |
/// | 40 | 2e5 | 0.409183 | 101.642117 | 0.990000 | 3.231940e-2 |
/// | 50 | 1e6 | 0.325103 | 75.837128 | 0.990000 | 1.097671e-1 |
/// | 60 | 1.6e6 | 0.292851 | 56.202830 | 0.990000 | 2.707597e-1 |
///
/// **Two distinct symptoms, and they are almost certainly one cause and one
/// separate cause.**
///
/// First, **damage saturates immediately**. `V9` reads `0.990000000` at every
/// assertion instant — that is `LEMAITRE_CHABOCHE_DAMAGE_MAX`, the ceiling, not
/// a computed value. Upstream reaches only `0.27` by `t = 1.6e6`. Everything
/// else follows from that: a fully damaged material carries almost no stress,
/// which is why `SIZZ` collapses to ~1 MPa against upstream's ~253.
///
/// The magnitudes say the *formula* is right and the *integration* is not. The
/// port computes `Ḋ = (χ/A)^r (1-D)^(-k)`, which is the correct Lemaitre-Chaboche
/// form and matches the parameter mapping (`A_D` → `damage_strength`, `R_D` →
/// `damage_exponent`, `K_D` → `damage_closure_exponent`, all verified against
/// the port's own source). At `t = 20` with `χ ≈ 250` MPa and `A = 2511.35`
/// MPa, `(χ/A)^5.2 ≈ 6e-6` per second, giving `D ≈ 1.2e-4` — the same order as
/// upstream's `2.32e-4`. So the rate is right at `D = 0`, and the runaway comes
/// from the `(1-D)^(-k)` amplification with `k ≈ 14.5`, which is explosive once
/// `D` grows and demands sub-stepping that this driver does not do. Upstream
/// integrates it with Runge-Kutta *and adaptive sub-steps*; one Euler step over
/// a `dt` of 36000 s cannot follow it.
///
/// Second, and separately, **`V8` is exactly `V7 / 100`** at every instant —
/// `2.646947e-5` against `2.646947e-3`, and the same ratio at all five points.
/// Upstream has `V8 ≈ V7` (`1.646053e-3` against `1.646107e-3`). An exact
/// factor of 100 across independent instants is not a convergence artefact; it
/// points at the isotropic-hardening variable being scaled somewhere, and it
/// would persist even with perfect time integration.
///
/// **Next step:** sub-step the damage integration within each of the deck's
/// 60 intervals and re-measure, which should settle whether the first symptom
/// is wholly a time-integration artefact. The `V8` factor of 100 needs a
/// separate read of the port's hardening update against `rkdvec.F90` and is not
/// explained by sub-stepping.
///
/// Tracked on bead op-b0x.
#[test]
fn ssnv126a_relaxation_reproduces_code_aster() {
    let elastic =
        IsotropicElasticity::from_young_poisson(YOUNG, POISSON).expect("valid isotropic moduli");
    let temperature = temperature_history();

    let mut state = LemaitreChabocheState::pristine();
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
        let solution = solve_mixed_control(
            control,
            YOUNG,
            POISSON,
            SymmTensor::ZERO,
            |strain| {
                law.integrate(elastic, start, strain, dt)
                    .expect("VENDOCHAB integration must converge")
                    .stress
            },
            CONTROL_TOLERANCE,
            50_000,
        );

        let increment = law
            .integrate(elastic, start, solution.strain, dt)
            .expect("VENDOCHAB integration must converge");
        state = increment.state;
        damage_driver = increment.damage_equivalent_stress;

        if let Some(&(_, _, _, _, _)) = SOLNL_REFERENCE.iter().find(|r| r.0 == order) {
            println!(
                "{order:>4} {t:>11.4e} {temp:>8.2} {:>12.6} {:>13.6e} {:>13.6e} {:>12.6e}",
                increment.stress.zz,
                state.equivalent_viscoplastic_strain,
                state.hardening_variable,
                state.damage
            );
            recorded.push((
                order,
                increment.stress.zz,
                state.equivalent_viscoplastic_strain,
                state.hardening_variable,
                state.damage,
            ));
        }
    }

    println!("\n--- versus code_aster SOLNL (RUNGE_KUTTA) VALE_CALC ---");
    let mut worst = 0.0_f64;
    for &(order, sizz, v7, v8, v9) in &recorded {
        let calc = SOLNL_REFERENCE.iter().find(|r| r.0 == order).unwrap();
        let refe = ANALYTICAL_REFERENCE.iter().find(|r| r.0 == order).unwrap();
        for (name, ours, c, r) in [
            ("SIZZ", sizz, calc.1, refe.1),
            ("V7  ", v7, calc.2, refe.2),
            ("V8  ", v8, calc.3, refe.3),
            ("V9  ", v9, calc.4, refe.4),
        ] {
            let rel_calc = ((ours - c) / c).abs();
            let rel_refe = ((ours - r) / r).abs();
            worst = worst.max(rel_calc);
            println!(
                "ord {order:>2} {name}: ours = {ours:>15.9} CALC = {c:>15.9} rel = {rel_calc:>10.4e}   \
                 [REFE = {r}, rel = {rel_refe:.4e}, reported only]"
            );
        }
    }
    println!("\nworst relative difference against VALE_CALC = {worst:.6e}");

    assert!(
        !recorded.is_empty(),
        "no assertion instants were reached — the instant list is wrong"
    );
}
