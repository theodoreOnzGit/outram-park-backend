// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Tests for [`crate::math::differentiate`], in five groups.
//!
//! 1. **Contract** — step-size rule, `x = 0`, statuses, `Option` accessors,
//!    backend reduction. Cheap, no numerics.
//! 2. **Verification against analytic derivatives** — a quadratic system, a
//!    trigonometric system and a stiff pair, all of whose Jacobians can be
//!    written down exactly, so they are perfect oracles.
//! 3. **Convergence order and the accuracy floor** — the measurements quoted in
//!    the module documentation. These *print* their tables; the assertions are
//!    the bounds the prose claims.
//! 4. **Determinism** — bitwise identity of serial against `rayon` at 1/2/4/8
//!    workers.
//! 5. **Measurement** — `#[ignore]`d benchmarks whose printed output is the
//!    source of the crossover tables.

use super::*;
use crate::ode::{OdeSystem, Rosenbrock23};

// ── Oracle systems: analytic Jacobians written down exactly ──────────────────

/// `f(x0, x1) = [x0^2 + x1, x0 * x1^2]`.
///
/// `J = [[2*x0, 1], [x1^2, 2*x0*x1]]` — polynomial, so a central difference is
/// exact up to round-off in the *third* derivative, which is zero here for the
/// first component.
fn quadratic_f(v: &[f64], out: &mut Vec<f64>) {
    out.clear();
    out.push(v[0] * v[0] + v[1]);
    out.push(v[0] * v[1] * v[1]);
}

fn quadratic_jacobian(v: &[f64]) -> [[f64; 2]; 2] {
    [[2.0 * v[0], 1.0], [v[1] * v[1], 2.0 * v[0] * v[1]]]
}

/// `f(x0, x1) = [sin(x0) * cos(x1), exp(x0) * x1]`.
///
/// `J = [[cos(x0)*cos(x1), -sin(x0)*sin(x1)], [exp(x0)*x1, exp(x0)]]`. Chosen
/// because every entry is order unity and *no* derivative of any order
/// vanishes, so the truncation term is genuinely present at every order.
fn trig_f(v: &[f64], out: &mut Vec<f64>) {
    out.clear();
    out.push(v[0].sin() * v[1].cos());
    out.push(v[0].exp() * v[1]);
}

fn trig_jacobian(v: &[f64]) -> [[f64; 2]; 2] {
    [
        [v[0].cos() * v[1].cos(), -v[0].sin() * v[1].sin()],
        [v[0].exp() * v[1], v[0].exp()],
    ]
}

/// A stiff linear pair: `f = [-1000*y0 + y1, y0 - y1]`, `J = [[-1000, 1], [1, -1]]`.
///
/// Constant Jacobian with a 1000:1 entry spread — the case where a *single*
/// absolute step size would be wrong for one row or the other, and the case a
/// Rosenbrock solver actually meets.
fn stiff_f(v: &[f64], out: &mut Vec<f64>) {
    out.clear();
    out.push(-1000.0 * v[0] + v[1]);
    out.push(v[0] - v[1]);
}

const STIFF_JACOBIAN: [[f64; 2]; 2] = [[-1000.0, 1.0], [1.0, -1.0]];

/// The stiff pair as an [`OdeSystem`] with **no** `jacobian` override — the
/// exact shape that makes [`Rosenbrock23`] panic today.
struct StiffPairNoJacobian;

impl OdeSystem for StiffPairNoJacobian {
    fn n_eqns(&self) -> usize {
        2
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        stiff_f(y, dydx);
    }
}

/// Scalar stiff decay `dy/dx = -1000 y`, no `jacobian` override.
struct StiffDecayNoJacobian;

impl OdeSystem for StiffDecayNoJacobian {
    fn n_eqns(&self) -> usize {
        1
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx.clear();
        dydx.push(-1000.0 * y[0]);
    }
}

/// Van der Pol, which *does* have an analytic Jacobian — used as the oracle for
/// checking the numerical one, and to show the wrapper reproduces it.
struct VanDerPol {
    mu: f64,
}

impl OdeSystem for VanDerPol {
    fn n_eqns(&self) -> usize {
        2
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx.clear();
        dydx.push(y[1]);
        dydx.push(self.mu * (1.0 - y[0] * y[0]) * y[1] - y[0]);
    }
    fn jacobian(&self, _x: f64, y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdx[1] = 0.0;
        dfdy.set(0, 0, 0.0);
        dfdy.set(0, 1, 1.0);
        dfdy.set(1, 0, -2.0 * self.mu * y[0] * y[1] - 1.0);
        dfdy.set(1, 1, self.mu * (1.0 - y[0] * y[0]));
    }
}

const SCHEMES: [DiffScheme; 4] = [
    DiffScheme::Forward,
    DiffScheme::Backward,
    DiffScheme::Central,
    DiffScheme::Central4th,
];

// ── 1. Contract ──────────────────────────────────────────────────────────────

#[test]
fn epsilon_root_constants_match_their_definitions() {
    assert_eq!(
        CBRT_EPSILON,
        f64::EPSILON.cbrt(),
        "CBRT_EPSILON = {:.17e}",
        f64::EPSILON.cbrt()
    );
    assert_eq!(
        FIFTH_ROOT_EPSILON,
        f64::EPSILON.powf(0.2),
        "FIFTH_ROOT_EPSILON = {:.17e}",
        f64::EPSILON.powf(0.2)
    );
    assert_eq!(
        DiffScheme::Forward.default_relative_step(),
        crate::math::minimise::SQRT_EPSILON
    );
}

#[test]
fn step_rule_is_relative_where_x_is_large_and_absolute_where_it_is_not() {
    let s = DiffSettings::central();
    assert_eq!(s.step_for(1000.0), s.relative_step * 1000.0);
    assert_eq!(s.step_for(-1000.0), s.relative_step * 1000.0);
    // The floor is what makes x = 0 work at all.
    assert_eq!(s.step_for(0.0), s.relative_step);
    assert!(s.step_for(0.0) > 0.0);
    // A caller working in pascals can raise the floor.
    let p = DiffSettings {
        min_scale: 1.0e5,
        ..DiffSettings::central()
    };
    assert_eq!(p.step_for(0.0), p.relative_step * 1.0e5);
}

#[test]
fn derivative_at_exactly_zero_is_correct_not_nan() {
    // d/dx cos(x) at x = 0 is 0; d/dx (x^3 + 2x) at x = 0 is 2.
    for scheme in SCHEMES {
        let s = derivative(0.0, DiffSettings::with_scheme(scheme), |x: f64| {
            x * x * x + 2.0 * x
        });
        let d = s
            .derivative()
            .unwrap_or_else(|| panic!("{} failed at x = 0", scheme.label()));
        assert!(
            (d - 2.0).abs() < 1e-6,
            "{}: got {d} at x = 0, want 2",
            scheme.label()
        );
    }
}

#[test]
fn a_step_that_rounds_away_reports_degenerate_rather_than_a_wrong_number() {
    // relative_step 1e-20 at x = 1 gives h = 1e-20, and 1.0 + 1e-20 == 1.0
    // exactly. The realised step is zero, so the quotient would be 0/0. That
    // must be reported, not returned as a derivative of NaN or 0.
    let s = DiffSettings {
        relative_step: 1.0e-20,
        ..DiffSettings::central()
    };
    let out = derivative(1.0, s, |x: f64| x * x);
    assert_eq!(out.status(), DiffStatus::DegenerateStep);
    assert!(out.derivative().is_none());

    // A zero or non-finite relative step is caught before any evaluation.
    for bad_step in [0.0_f64, -1.0, f64::NAN, f64::INFINITY] {
        let s = DiffSettings {
            relative_step: bad_step,
            ..DiffSettings::central()
        };
        assert_eq!(
            derivative(1.0, s, |x: f64| x * x).status(),
            DiffStatus::DegenerateStep,
            "relative_step = {bad_step}"
        );
    }
}

#[test]
fn a_denormal_point_still_gets_a_usable_step() {
    // x = 1e-300 is 250 decades above the smallest normal, so even a purely
    // relative step is representable there. This is NOT the degenerate case,
    // and the module must not pretend it is.
    let s = DiffSettings {
        min_scale: 0.0,
        ..DiffSettings::central()
    };
    // f = 3x, not x^2: at x = 1e-300 the square itself underflows to zero, so
    // the derivative of the *representable* function really is 0 and the test
    // would be measuring the wrong thing.
    let out = derivative(1.0e-300, s, |x: f64| 3.0 * x);
    assert_eq!(out.status(), DiffStatus::Ok);
    let d = out.derivative().expect("representable step");
    assert!(
        (d - 3.0).abs() < 1e-9,
        "d/dx 3x at 1e-300 should be 3, got {d}"
    );
}

#[test]
fn non_finite_evaluations_are_reported_not_swallowed() {
    // sqrt at x = 0: the central stencil samples x - h < 0, which is NaN.
    let bad = derivative(0.0, DiffSettings::central(), |x: f64| x.sqrt());
    assert_eq!(bad.status(), DiffStatus::NotFinite);
    assert!(bad.derivative().is_none());
    assert!(bad.raw_value().is_nan());

    let invalid = derivative(f64::NAN, DiffSettings::central(), |x: f64| x);
    assert_eq!(invalid.status(), DiffStatus::InvalidPoint);
    assert!(invalid.derivative().is_none());

    // Overflow in the quotient is caught too.
    let overflow = derivative(1.0, DiffSettings::central(), |x: f64| {
        if x > 1.0 {
            f64::MAX
        } else {
            -f64::MAX
        }
    });
    assert_eq!(overflow.status(), DiffStatus::NotFinite);
}

/// A documented limitation, asserted so it cannot drift: a **symmetric stencil
/// straddles a pole without noticing**.
///
/// `1/x` at `x = 0` is sampled at `+h` and `-h`, both perfectly finite, so the
/// central difference returns `1/h^2` with [`DiffStatus::Ok`]. Nothing in a
/// finite-difference kernel can detect this — it never evaluates at the pole.
/// The forward stencil *does* see it, because it evaluates at `x` itself.
#[test]
fn a_symmetric_stencil_straddles_a_pole_without_noticing() {
    let straddled = derivative(0.0, DiffSettings::central(), |x: f64| 1.0 / x);
    assert_eq!(
        straddled.status(),
        DiffStatus::Ok,
        "the central stencil cannot see the pole it steps over"
    );
    let h = DiffSettings::central().step_for(0.0);
    let d = straddled.derivative().expect("both samples are finite");
    // (1/h - (-1/h)) / (2h) = 1/h^2 -- a large, confident, meaningless number.
    assert!(
        (d - 1.0 / (h * h)).abs() < 1e-3 / (h * h),
        "got {d}, expected about {}",
        1.0 / (h * h)
    );

    // The one-sided stencil evaluates AT the pole and therefore does see it.
    let seen = derivative(0.0, DiffSettings::forward(), |x: f64| 1.0 / x);
    assert_eq!(seen.status(), DiffStatus::NotFinite);
}

#[test]
fn a_bad_lane_never_contaminates_the_all_or_nothing_accessor() {
    let points = [1.0, 0.0, 3.0];
    let batch = derivative_batch_min(
        &points,
        DiffSettings::central(),
        ComputeBackend::Serial,
        0,
        |i, x: f64| if i == 1 { x.sqrt() } else { x * x },
    );
    assert!(!batch.all_ok());
    assert_eq!(batch.failure_count(), 1);
    let err = batch.values().expect_err("lane 1 diverges");
    assert_eq!(err.first_index, 1);
    assert_eq!(err.total, 3);
    assert_eq!(err.first_status, DiffStatus::NotFinite);
    // The good lanes are still individually readable.
    assert!(batch.get(0).unwrap().derivative().is_some());
    assert!(batch.get(2).unwrap().derivative().is_some());
}

#[test]
fn a_non_square_function_is_rejected_rather_than_padded() {
    let s = jacobian(
        &[1.0, 2.0],
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, _v: &[f64], out: &mut Vec<f64>| {
            out.push(1.0); // only one component for a 2-D point
        },
    );
    assert_eq!(s.status(), DiffStatus::DimensionMismatch);
    assert!(s.matrix().is_none());
}

#[test]
fn a_failing_column_names_the_variable_that_failed() {
    // Blows up only when x1 is perturbed below zero.
    // The base point is fine for both components (ln(1e-8) is finite); only
    // perturbing x1 downwards by h = 6.06e-6 takes it negative.
    let s = jacobian(
        &[1.0, 1.0e-8],
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, v: &[f64], out: &mut Vec<f64>| {
            out.push(v[0]);
            out.push(v[1].ln());
        },
    );
    assert!(!s.is_ok());
    assert_eq!(s.first_bad_column(), 1);
    assert!(s.matrix().is_none());
    assert!(s.raw_matrix().get(1, 1).is_nan());
    // The good column is still in the raw matrix.
    assert_eq!(s.raw_matrix().get(0, 0), 1.0);
}

#[test]
fn empty_and_malformed_batches_are_empty_not_panics() {
    let empty = jacobian_batch(
        &[],
        2,
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, _: &[f64], _: &mut Vec<f64>| {},
    );
    assert!(empty.is_empty());

    let zero_dim = jacobian_batch(
        &[1.0],
        0,
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, _: &[f64], _: &mut Vec<f64>| {},
    );
    assert!(zero_dim.is_empty());

    // 5 values with n = 2 is not a whole number of lanes.
    let ragged = jacobian_batch(
        &[1.0; 5],
        2,
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, _: &[f64], _: &mut Vec<f64>| {},
    );
    assert!(ragged.is_empty());

    let no_points = derivative_batch(
        &[],
        DiffSettings::central(),
        ComputeBackend::Serial,
        |_, x: f64| x,
    );
    assert!(no_points.is_empty());
    assert!(no_points.all_ok());
    assert_eq!(no_points.values().unwrap().len(), 0);
}

#[test]
fn backend_reduction_never_reports_gpu_and_respects_the_size_floors() {
    for f in [
        derivative_backend_for as fn(ComputeBackend, usize) -> ComputeBackend,
        jacobian_batch_backend_for,
        jacobian_column_backend_for,
    ] {
        assert_eq!(f(ComputeBackend::CpuMulti, 1), ComputeBackend::Serial);
        assert_eq!(
            f(ComputeBackend::Serial, usize::MAX),
            ComputeBackend::Serial
        );
        assert_ne!(f(ComputeBackend::Gpu, usize::MAX), ComputeBackend::Gpu);
        assert!(f(ComputeBackend::Gpu, usize::MAX).is_available());
    }
    assert_eq!(
        derivative_backend_for(ComputeBackend::CpuMulti, DERIVATIVE_BATCH_MIN_POINTS - 1),
        ComputeBackend::Serial
    );
}

#[test]
fn evaluation_counts_match_the_documented_cost() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for (scheme, want) in [
        (DiffScheme::Forward, 4_usize), // n + 1 with n = 3
        (DiffScheme::Backward, 4),
        (DiffScheme::Central, 6),     // 2n
        (DiffScheme::Central4th, 12), // 4n
    ] {
        assert_eq!(scheme.evaluations_per_jacobian(3), want);
        let calls = AtomicUsize::new(0_usize);
        let _ = jacobian(
            &[1.0, 2.0, 3.0],
            DiffSettings::with_scheme(scheme),
            ComputeBackend::Serial,
            |_, v: &[f64], out: &mut Vec<f64>| {
                calls.fetch_add(1, Ordering::Relaxed);
                out.clear();
                out.extend_from_slice(v);
            },
        );
        assert_eq!(
            calls.load(Ordering::Relaxed),
            want,
            "{} made {} calls, documented cost is {want}",
            scheme.label(),
            calls.load(Ordering::Relaxed)
        );
    }
}

// ── 2. Verification against analytic derivatives ─────────────────────────────

/// Worst relative error of a numerical Jacobian against its analytic oracle.
fn jacobian_worst_relative_error(
    point: &[f64],
    settings: DiffSettings,
    f: fn(&[f64], &mut Vec<f64>),
    exact: [[f64; 2]; 2],
) -> f64 {
    let s = jacobian(
        point,
        settings,
        ComputeBackend::Serial,
        |_, v: &[f64], out: &mut Vec<f64>| f(v, out),
    );
    let m = s.matrix().expect("smooth oracle system");
    let mut worst = 0.0_f64;
    for i in 0..2 {
        for j in 0..2 {
            let want = exact[i][j];
            let err = (m.get(i, j) - want).abs() / want.abs().max(1.0);
            worst = worst.max(err);
        }
    }
    worst
}

#[test]
fn quadratic_system_jacobian_matches_the_analytic_one() {
    let point = [1.5_f64, -2.25];
    let exact = quadratic_jacobian(&point);
    for scheme in SCHEMES {
        let err = jacobian_worst_relative_error(
            &point,
            DiffSettings::with_scheme(scheme),
            quadratic_f,
            exact,
        );
        let tol = match scheme {
            DiffScheme::Forward | DiffScheme::Backward => 1e-7,
            _ => 1e-10,
        };
        assert!(
            err < tol,
            "{}: worst relative error {err:.6e} exceeds {tol:.0e}",
            scheme.label()
        );
    }
}

#[test]
fn trigonometric_system_jacobian_matches_the_analytic_one() {
    let point = [0.7_f64, 1.3];
    let exact = trig_jacobian(&point);
    for scheme in SCHEMES {
        let err =
            jacobian_worst_relative_error(&point, DiffSettings::with_scheme(scheme), trig_f, exact);
        let tol = match scheme {
            DiffScheme::Forward | DiffScheme::Backward => 1e-7,
            DiffScheme::Central => 1e-10,
            DiffScheme::Central4th => 1e-11,
        };
        assert!(
            err < tol,
            "{}: worst relative error {err:.6e} exceeds {tol:.0e}",
            scheme.label()
        );
    }
}

/// A linear system has **zero** truncation error, so whatever error remains is
/// pure cancellation — which makes this the cleanest measurement of the price a
/// 1000:1 entry spread costs. Prints the per-entry table quoted in the module
/// documentation.
#[test]
fn stiff_pair_jacobian_matches_the_analytic_one_across_a_1000_to_1_entry_spread() {
    let point = [0.4_f64, -0.9];
    println!("\nLinear stiff pair J = [[-1000, 1], [1, -1]] at y = [0.4, -0.9]");
    println!("absolute error per entry -- truncation is exactly zero here");
    println!(
        "{:>12} {:>14} {:>14} {:>14} {:>14}",
        "scheme", "J[0][0]", "J[0][1]", "J[1][0]", "J[1][1]"
    );
    for scheme in SCHEMES {
        let settings = DiffSettings::with_scheme(scheme);
        let s = jacobian(
            &point,
            settings,
            ComputeBackend::Serial,
            |_, v: &[f64], out: &mut Vec<f64>| stiff_f(v, out),
        );
        let m = s.matrix().expect("linear system is finite everywhere");
        let e = |i: usize, j: usize| (m.get(i, j) - STIFF_JACOBIAN[i][j]).abs();
        println!(
            "{:>12} {:>14.6e} {:>14.6e} {:>14.6e} {:>14.6e}",
            scheme.label(),
            e(0, 0),
            e(0, 1),
            e(1, 0),
            e(1, 1)
        );

        let err = jacobian_worst_relative_error(&point, settings, stiff_f, STIFF_JACOBIAN);
        // Measured worst is 1.778424e-9 (central, release, 2026-08-13); the
        // bound is one decade of headroom above it, not a guess.
        assert!(
            err < 1e-8,
            "{}: worst relative error {err:.6e} on a LINEAR system",
            scheme.label()
        );
    }
}

#[test]
fn ode_jacobian_reproduces_van_der_pols_hand_coded_one() {
    let system = VanDerPol { mu: 5.0 };
    let (x, y) = (0.3_f64, [1.7_f64, -0.6]);

    let mut analytic_dfdx = vec![0.0; 2];
    let mut analytic_dfdy = SquareMatrix::new(2);
    system.jacobian(x, &y, &mut analytic_dfdx, &mut analytic_dfdy);

    let mut dfdx = Vec::new();
    let mut dfdy = SquareMatrix::new(0);
    let status = ode_system_jacobian(
        &system,
        x,
        &y,
        DiffSettings::central(),
        &mut dfdx,
        &mut dfdy,
    );
    assert_eq!(status, DiffStatus::Ok);
    assert_eq!(dfdx.len(), 2);
    assert_eq!(dfdy.n(), 2);

    for i in 0..2 {
        assert!(
            (dfdx[i] - analytic_dfdx[i]).abs() < 1e-9,
            "dfdx[{i}]: {} vs {}",
            dfdx[i],
            analytic_dfdx[i]
        );
        for j in 0..2 {
            let want = analytic_dfdy.get(i, j);
            assert!(
                (dfdy.get(i, j) - want).abs() < 1e-7 * want.abs().max(1.0),
                "dfdy[{i}][{j}]: {} vs {want}",
                dfdy.get(i, j)
            );
        }
    }
}

#[test]
fn ode_jacobian_captures_explicit_dependence_on_the_independent_variable() {
    // dy/dx = [sin(x) * y1, x^2] -- dfdx = [cos(x) * y1, 2x], NOT zero.
    struct TimeDependent;
    impl OdeSystem for TimeDependent {
        fn n_eqns(&self) -> usize {
            2
        }
        fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx.clear();
            dydx.push(x.sin() * y[1]);
            dydx.push(x * x);
        }
    }

    let (x, y) = (0.8_f64, [0.0_f64, 2.5]);
    let mut dfdx = Vec::new();
    let mut dfdy = SquareMatrix::new(2);
    let status = ode_system_jacobian(
        &TimeDependent,
        x,
        &y,
        DiffSettings::central(),
        &mut dfdx,
        &mut dfdy,
    );
    assert_eq!(status, DiffStatus::Ok);
    assert!(
        (dfdx[0] - x.cos() * y[1]).abs() < 1e-8,
        "dfdx[0] = {}",
        dfdx[0]
    );
    assert!((dfdx[1] - 2.0 * x).abs() < 1e-8, "dfdx[1] = {}", dfdx[1]);
    assert!(dfdy.get(0, 0).abs() < 1e-8);
    assert!((dfdy.get(0, 1) - x.sin()).abs() < 1e-8);
}

#[test]
fn ode_jacobian_resizes_wrongly_sized_output_buffers() {
    let mut dfdx = vec![0.0; 7];
    let mut dfdy = SquareMatrix::new(5);
    let status = ode_system_jacobian(
        &StiffPairNoJacobian,
        0.0,
        &[1.0, 1.0],
        DiffSettings::central(),
        &mut dfdx,
        &mut dfdy,
    );
    assert_eq!(status, DiffStatus::Ok);
    assert_eq!(dfdx.len(), 2);
    assert_eq!(dfdy.n(), 2);
}

// ── The concrete consumer: Rosenbrock23 with no hand-coded Jacobian ──────────

#[test]
fn rosenbrock23_integrates_a_stiff_system_that_has_no_hand_coded_jacobian() {
    let system = NumericalJacobian::new(StiffDecayNoJacobian, DiffSettings::central());
    let mut solver = Rosenbrock23::new(1, 1e-10, 1e-10);
    let mut y = vec![1.0_f64];
    let mut dx = 1e-5;
    solver
        .integrate(&system, 0.0, 0.01, &mut y, &mut dx)
        .expect("integrates without a hand-coded Jacobian");

    let exact = (-10.0_f64).exp();
    let rel = (y[0] - exact).abs() / exact;
    assert!(
        rel < 1e-4,
        "y = {}, exact = {exact}, rel err = {rel:.3e}",
        y[0]
    );
    assert_eq!(system.non_finite_jacobians(), 0);
}

#[test]
fn rosenbrock23_integrates_the_stiff_pair_with_a_numerical_jacobian() {
    // y' = [-1000 y0 + y1, y0 - y1]; eigenvalues approximately -1000.001 and
    // -0.999. From y(0) = [1, 1] the slow mode dominates almost immediately.
    let system = NumericalJacobian::new(StiffPairNoJacobian, DiffSettings::central());
    let mut solver = Rosenbrock23::new(2, 1e-10, 1e-10);
    let mut y = vec![1.0_f64, 1.0];
    let mut dx = 1e-6;
    solver
        .integrate(&system, 0.0, 1.0, &mut y, &mut dx)
        .expect("integrates");

    // Closed form: the exact matrix exponential of [[-1000, 1], [1, -1]].
    let (a, b, c, d) = (-1000.0_f64, 1.0_f64, 1.0_f64, -1.0_f64);
    let tr = a + d;
    let det = a * d - b * c;
    let disc = (tr * tr - 4.0 * det).sqrt();
    let (l1, l2) = (0.5 * (tr + disc), 0.5 * (tr - disc));
    // v_k = [b, l_k - a]; y(0) = alpha v1 + beta v2.
    let (v1, v2) = ([b, l1 - a], [b, l2 - a]);
    let det_v = v1[0] * v2[1] - v2[0] * v1[1];
    let alpha = (1.0 * v2[1] - v2[0] * 1.0) / det_v;
    let beta = (v1[0] * 1.0 - 1.0 * v1[1]) / det_v;
    let exact0 = alpha * v1[0] * l1.exp() + beta * v2[0] * l2.exp();
    let exact1 = alpha * v1[1] * l1.exp() + beta * v2[1] * l2.exp();

    assert!(
        (y[0] - exact0).abs() < 1e-6 * exact0.abs().max(1.0),
        "y0 = {}, exact = {exact0}",
        y[0]
    );
    assert!(
        (y[1] - exact1).abs() < 1e-6 * exact1.abs().max(1.0),
        "y1 = {}, exact = {exact1}",
        y[1]
    );
    assert_eq!(system.non_finite_jacobians(), 0);
}

#[test]
fn the_wrapper_forwards_n_eqns_and_derivatives_verbatim() {
    let system = NumericalJacobian::new(VanDerPol { mu: 2.0 }, DiffSettings::central());
    assert_eq!(system.n_eqns(), 2);
    assert_eq!(system.inner().mu, 2.0);
    assert_eq!(system.settings().scheme, DiffScheme::Central);

    let y = [0.5_f64, -1.5];
    let (mut wrapped, mut bare) = (Vec::new(), Vec::new());
    system.derivatives(0.25, &y, &mut wrapped);
    VanDerPol { mu: 2.0 }.derivatives(0.25, &y, &mut bare);
    assert_eq!(wrapped, bare);

    let inner = system.into_inner();
    assert_eq!(inner.mu, 2.0);
}

#[test]
fn a_jacobian_that_cannot_be_differenced_is_counted_and_reaches_the_solver_as_nan() {
    // derivatives() returns a non-finite value everywhere, so no column can be
    // formed. The wrapper must count it AND write NaN, so the solver fails
    // loudly rather than integrating a fabricated Jacobian.
    struct AlwaysNaN;
    impl OdeSystem for AlwaysNaN {
        fn n_eqns(&self) -> usize {
            1
        }
        fn derivatives(&self, _x: f64, _y: &[f64], dydx: &mut Vec<f64>) {
            dydx.clear();
            dydx.push(f64::NAN);
        }
    }

    let system = NumericalJacobian::new(AlwaysNaN, DiffSettings::central());
    let mut dfdx = vec![0.0; 1];
    let mut dfdy = SquareMatrix::new(1);
    system.jacobian(0.0, &[1.0], &mut dfdx, &mut dfdy);

    assert_eq!(system.non_finite_jacobians(), 1);
    assert!(
        dfdy.get(0, 0).is_nan(),
        "a failed entry must be NaN, not 0.0"
    );
    assert!(dfdx[0].is_nan());

    // What the SOLVER then does with it -- measured, not assumed. See the
    // module-level "A NaN Jacobian is not reported by the solver" section.
    let mut solver = Rosenbrock23::new(1, 1e-8, 1e-8);
    let mut y = vec![1.0_f64];
    let mut dx = 1e-4;
    let outcome = solver.integrate(&system, 0.0, 1.0, &mut y, &mut dx);

    // `Rosenbrock23` returns Ok(()) -- it does NOT detect the NaN, because
    // `ode::normalize_error` folds the per-equation errors with `f64::max`,
    // which discards NaN and yields 0.0, so every step looks perfectly
    // converged. The state is nonsense and the return value says otherwise.
    assert!(
        outcome.is_ok(),
        "recorded behaviour: the solver does not detect a NaN Jacobian"
    );
    assert!(
        y[0].is_nan(),
        "the NaN reaches the state even though the solver reported success"
    );
    // THIS counter is therefore the only in-band signal that anything failed,
    // which is exactly why it exists.
    assert!(
        system.non_finite_jacobians() > 1,
        "the counter is the only report of the failure"
    );
}

// ── 3. Convergence order and the accuracy floor ──────────────────────────────

/// Observed convergence order of each scheme, measured on `sin` at `x = 1`.
///
/// Prints the table quoted in the module documentation.
#[test]
fn observed_convergence_order_matches_theory() {
    let x = 1.0_f64;
    let exact = x.cos();
    println!("\nConvergence order, d/dx sin(x) at x = 1, exact = {exact:.17e}");
    println!(
        "{:>12} {:>14} {:>14} {:>14} {:>14}",
        "rel step", "forward", "backward", "central", "central-4th"
    );

    // Truncation-dominated decades, where the order is measurable.
    let steps = [1e-1_f64, 1e-2, 1e-3, 1e-4];
    let mut errors = [[0.0_f64; 4]; 4];
    for (r, &step) in steps.iter().enumerate() {
        for (c, scheme) in SCHEMES.iter().enumerate() {
            let settings = DiffSettings {
                relative_step: step,
                ..DiffSettings::with_scheme(*scheme)
            };
            let d = derivative(x, settings, |t: f64| t.sin())
                .derivative()
                .expect("sin is finite");
            errors[r][c] = (d - exact).abs();
        }
        println!(
            "{step:>12.0e} {:>14.6e} {:>14.6e} {:>14.6e} {:>14.6e}",
            errors[r][0], errors[r][1], errors[r][2], errors[r][3]
        );
    }

    println!(
        "{:>12} {:>14} {:>14} {:>14} {:>14}",
        "order", "", "", "", ""
    );
    let want = [1.0_f64, 1.0, 2.0, 4.0];
    let mut orders = [0.0_f64; 4];
    for c in 0..4 {
        // Slope over the first decade pair, where round-off is still negligible.
        orders[c] = (errors[0][c] / errors[1][c]).log10();
    }
    println!(
        "{:>12} {:>14.4} {:>14.4} {:>14.4} {:>14.4}",
        "1e-1->1e-2", orders[0], orders[1], orders[2], orders[3]
    );

    for c in 0..4 {
        assert!(
            (orders[c] - want[c]).abs() < 0.15,
            "{}: observed order {:.4}, theory {}",
            SCHEMES[c].label(),
            orders[c],
            want[c]
        );
    }
}

/// The accuracy floor of each scheme at its own default step, over a spread of
/// smooth functions and points.
///
/// Prints the table quoted in the module documentation. The assertion is the
/// bound the prose claims, one order of magnitude loose so it is a bound rather
/// than a transcription of one machine's noise.
#[test]
fn accuracy_floor_at_the_default_step() {
    // (name, f, f', points)
    struct Case {
        name: &'static str,
        f: fn(f64) -> f64,
        d: fn(f64) -> f64,
    }
    let cases = [
        Case {
            name: "sin",
            f: |x| x.sin(),
            d: |x| x.cos(),
        },
        Case {
            name: "exp",
            f: |x| x.exp(),
            d: |x| x.exp(),
        },
        Case {
            name: "x^3-2x",
            f: |x| x * x * x - 2.0 * x,
            d: |x| 3.0 * x * x - 2.0,
        },
        Case {
            name: "1/(1+x^2)",
            f: |x| 1.0 / (1.0 + x * x),
            d: |x| -2.0 * x / ((1.0 + x * x) * (1.0 + x * x)),
        },
        Case {
            name: "tanh",
            f: |x| x.tanh(),
            d: |x| 1.0 - x.tanh() * x.tanh(),
        },
    ];
    let points = [0.25_f64, 0.5, 1.0, 1.7, 2.5, 3.3];

    println!("\nAccuracy floor at each scheme's default relative step");
    println!("(worst relative error over 6 points in [0.25, 3.3])");
    println!(
        "{:>12} {:>14} {:>14} {:>14} {:>14}",
        "function", "forward", "backward", "central", "central-4th"
    );

    let mut worst_overall = [0.0_f64; 4];
    for case in &cases {
        let mut row = [0.0_f64; 4];
        for (c, scheme) in SCHEMES.iter().enumerate() {
            let settings = DiffSettings::with_scheme(*scheme);
            for &x in &points {
                let got = derivative(x, settings, case.f)
                    .derivative()
                    .expect("smooth case");
                let want = (case.d)(x);
                let rel = (got - want).abs() / want.abs().max(1.0);
                row[c] = row[c].max(rel);
            }
            worst_overall[c] = worst_overall[c].max(row[c]);
        }
        println!(
            "{:>12} {:>14.6e} {:>14.6e} {:>14.6e} {:>14.6e}",
            case.name, row[0], row[1], row[2], row[3]
        );
    }
    println!(
        "{:>12} {:>14.6e} {:>14.6e} {:>14.6e} {:>14.6e}",
        "WORST", worst_overall[0], worst_overall[1], worst_overall[2], worst_overall[3]
    );
    println!(
        "{:>12} {:>14.6e} {:>14.6e} {:>14.6e} {:>14.6e}",
        "theory",
        crate::math::minimise::SQRT_EPSILON,
        crate::math::minimise::SQRT_EPSILON,
        f64::EPSILON.powf(2.0 / 3.0),
        f64::EPSILON.powf(0.8)
    );

    // Each scheme must land within 10x of its predicted floor.
    let predicted = [
        crate::math::minimise::SQRT_EPSILON,
        crate::math::minimise::SQRT_EPSILON,
        f64::EPSILON.powf(2.0 / 3.0),
        f64::EPSILON.powf(0.8),
    ];
    for c in 0..4 {
        assert!(
            worst_overall[c] < 10.0 * predicted[c],
            "{}: worst {:.6e} exceeds 10x the predicted floor {:.6e}",
            SCHEMES[c].label(),
            worst_overall[c],
            predicted[c]
        );
    }
    // And the ordering must be the one the theory predicts.
    assert!(
        worst_overall[3] < worst_overall[2],
        "central-4th ({:.3e}) should beat central ({:.3e})",
        worst_overall[3],
        worst_overall[2]
    );
    assert!(
        worst_overall[2] < worst_overall[0],
        "central ({:.3e}) should beat forward ({:.3e})",
        worst_overall[2],
        worst_overall[0]
    );
}

/// The round-off wall: making the step *smaller* than the optimum makes the
/// answer worse, which is the single most common misunderstanding about finite
/// differences.
#[test]
fn a_step_far_below_the_optimum_is_worse_not_better() {
    let x = 1.0_f64;
    let exact = x.cos();
    println!("\nRound-off wall for the central difference, d/dx sin(x) at x = 1");
    println!("{:>12} {:>16}", "rel step", "rel error");

    let mut at_optimum = f64::NAN;
    let mut at_1e_12 = f64::NAN;
    for step in [1e-2_f64, CBRT_EPSILON, 1e-8, 1e-10, 1e-12, 1e-14] {
        let settings = DiffSettings {
            relative_step: step,
            ..DiffSettings::central()
        };
        let d = derivative(x, settings, |t: f64| t.sin())
            .derivative()
            .expect("finite");
        let rel = (d - exact).abs() / exact.abs();
        println!("{step:>12.4e} {rel:>16.6e}");
        if step == CBRT_EPSILON {
            at_optimum = rel;
        }
        if step == 1e-12 {
            at_1e_12 = rel;
        }
    }
    assert!(
        at_1e_12 > 100.0 * at_optimum,
        "a 1e-12 step ({at_1e_12:.3e}) should be far worse than the optimum ({at_optimum:.3e})"
    );
}

// ── 4. Determinism ───────────────────────────────────────────────────────────

fn bitwise_same_derivatives(a: &DerivativeBatch, b: &DerivativeBatch) -> bool {
    a.len() == b.len()
        && a.solutions().iter().zip(b.solutions()).all(|(p, q)| {
            p.raw_value().to_bits() == q.raw_value().to_bits()
                && p.realised_step().to_bits() == q.realised_step().to_bits()
                && p.status() == q.status()
        })
}

fn bitwise_same_jacobians(a: &JacobianBatch, b: &JacobianBatch) -> bool {
    a.len() == b.len()
        && a.solutions().iter().zip(b.solutions()).all(|(p, q)| {
            let (pm, qm) = (p.raw_matrix(), q.raw_matrix());
            p.status() == q.status()
                && pm.n() == qm.n()
                && (0..pm.n())
                    .all(|i| (0..pm.n()).all(|j| pm.get(i, j).to_bits() == qm.get(i, j).to_bits()))
        })
}

/// A batch whose lanes have wildly different magnitudes, so the per-lane step
/// differs and a mis-shared buffer would show up immediately.
fn imbalanced_points(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let t = (i % 41) as f64;
            (1.0 + t) * 10.0_f64.powi((i % 7) as i32 - 3)
        })
        .collect()
}

#[test]
fn derivative_batch_is_bitwise_identical_across_backends() {
    let points = imbalanced_points(2048);
    for scheme in SCHEMES {
        let settings = DiffSettings::with_scheme(scheme);
        let f = |i: usize, x: f64| (x * (1.0 + i as f64 * 1e-3)).sin() + x.exp() * 1e-2;
        let serial = derivative_batch_min(&points, settings, ComputeBackend::Serial, 0, f);
        let multi = derivative_batch_min(&points, settings, ComputeBackend::CpuMulti, 0, f);
        assert!(
            bitwise_same_derivatives(&serial, &multi),
            "{} differed between backends",
            scheme.label()
        );
    }
}

#[test]
fn jacobian_batch_is_bitwise_identical_across_backends() {
    let n = 4_usize;
    let lanes = 512_usize;
    let points: Vec<f64> = imbalanced_points(lanes * n);
    let f = |i: usize, v: &[f64], out: &mut Vec<f64>| {
        let k = 1.0 + (i % 13) as f64;
        out.clear();
        out.push(k * v[0] * v[1]);
        out.push(v[2].sin() + v[3] * v[3]);
        out.push(v[0].exp() * 1e-2 - v[2]);
        out.push(k * v[3] * v[1] * v[1]);
    };
    for scheme in SCHEMES {
        let settings = DiffSettings::with_scheme(scheme);
        let serial = jacobian_batch_min(&points, n, settings, ComputeBackend::Serial, 0, f);
        let multi = jacobian_batch_min(&points, n, settings, ComputeBackend::CpuMulti, 0, f);
        assert!(
            bitwise_same_jacobians(&serial, &multi),
            "{} differed between backends",
            scheme.label()
        );
    }
}

#[test]
fn column_parallel_jacobian_is_bitwise_identical_to_the_serial_one() {
    // A single large Jacobian, columns spread across threads.
    let n = 96_usize;
    let x: Vec<f64> = (0..n).map(|i| 0.5 + (i % 11) as f64 * 0.25).collect();
    let f = |_: usize, v: &[f64], out: &mut Vec<f64>| {
        out.clear();
        for i in 0..v.len() {
            let prev = v[(i + v.len() - 1) % v.len()];
            let next = v[(i + 1) % v.len()];
            out.push(v[i].sin() * next + prev * prev * 0.5);
        }
    };
    for scheme in SCHEMES {
        let settings = DiffSettings::with_scheme(scheme);
        let serial = jacobian_columns_min(0, &x, settings, ComputeBackend::Serial, 0, &f);
        let multi = jacobian_columns_min(0, &x, settings, ComputeBackend::CpuMulti, 0, &f);
        let (a, b) = (serial.raw_matrix(), multi.raw_matrix());
        assert_eq!(serial.status(), multi.status());
        for i in 0..n {
            for j in 0..n {
                assert_eq!(
                    a.get(i, j).to_bits(),
                    b.get(i, j).to_bits(),
                    "{} differed at ({i}, {j})",
                    scheme.label()
                );
            }
        }
    }
}

#[cfg(feature = "parallel")]
#[test]
fn bitwise_identical_across_thread_counts() {
    let points = imbalanced_points(2048);
    let settings = DiffSettings::central_4th();
    let f = |i: usize, x: f64| (x * (1.0 + i as f64 * 1e-3)).sin() + x.exp() * 1e-2;
    let reference = derivative_batch_min(&points, settings, ComputeBackend::Serial, 0, f);

    let n = 4_usize;
    let jac_points = imbalanced_points(512 * n);
    let jf = |i: usize, v: &[f64], out: &mut Vec<f64>| {
        let k = 1.0 + (i % 13) as f64;
        out.clear();
        out.push(k * v[0] * v[1]);
        out.push(v[2].sin() + v[3] * v[3]);
        out.push(v[0].exp() * 1e-2 - v[2]);
        out.push(k * v[3] * v[1] * v[1]);
    };
    let jac_reference = jacobian_batch_min(&jac_points, n, settings, ComputeBackend::Serial, 0, jf);

    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        pool.install(|| {
            let got = derivative_batch_min(&points, settings, ComputeBackend::CpuMulti, 0, f);
            assert!(
                bitwise_same_derivatives(&reference, &got),
                "derivative batch differed at {threads} threads"
            );
            let jgot =
                jacobian_batch_min(&jac_points, n, settings, ComputeBackend::CpuMulti, 0, jf);
            assert!(
                bitwise_same_jacobians(&jac_reference, &jgot),
                "jacobian batch differed at {threads} threads"
            );
        });
    }
}

#[test]
fn the_single_point_form_agrees_with_a_one_element_batch_bit_for_bit() {
    for scheme in SCHEMES {
        let settings = DiffSettings::with_scheme(scheme);
        let single = derivative(1.7, settings, |x: f64| x.sin() * x.exp());
        let batched =
            derivative_batch_min(&[1.7], settings, ComputeBackend::Serial, 0, |_, x: f64| {
                x.sin() * x.exp()
            });
        assert_eq!(
            single.raw_value().to_bits(),
            batched.get(0).unwrap().raw_value().to_bits(),
            "{}",
            scheme.label()
        );
    }
}

#[test]
fn the_single_jacobian_form_agrees_with_a_one_lane_batch_bit_for_bit() {
    let x = [1.5_f64, -2.25];
    for scheme in SCHEMES {
        let settings = DiffSettings::with_scheme(scheme);
        let f = |_: usize, v: &[f64], out: &mut Vec<f64>| quadratic_f(v, out);
        let single = jacobian(&x, settings, ComputeBackend::Serial, f);
        let batched = jacobian_batch_min(&x, 2, settings, ComputeBackend::Serial, 0, f);
        let (a, b) = (single.raw_matrix(), batched.get(0).unwrap().raw_matrix());
        for i in 0..2 {
            for j in 0..2 {
                assert_eq!(
                    a.get(i, j).to_bits(),
                    b.get(i, j).to_bits(),
                    "{} at ({i}, {j})",
                    scheme.label()
                );
            }
        }
    }
}

// ── 5. Measurement (ignored: too slow for the ordinary suite) ────────────────

/// Crossover benchmark for [`DERIVATIVE_BATCH_MIN_POINTS`] and
/// [`JACOBIAN_BATCH_MIN_PROBLEMS`].
///
/// `#[ignore]`d because it is a measurement, not a correctness check.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 differentiate_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn differentiate_crossover_benchmark() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!("DERIVATIVE_BATCH_MIN_POINTS = {DERIVATIVE_BATCH_MIN_POINTS}");
    println!("JACOBIAN_BATCH_MIN_PROBLEMS = {JACOBIAN_BATCH_MIN_PROBLEMS}");

    let settings = DiffSettings::central();

    println!("\n-- scalar derivative_batch, central, best of 7 --");
    println!(
        "{:>10} {:>14} {:>14} {:>9} {:>15} {:>15} {:>9}",
        "points",
        "cheap ser[us]",
        "cheap mul[us]",
        "speedup",
        "costly ser[us]",
        "costly mul[us]",
        "speedup"
    );
    for n in [16_usize, 32, 64, 128, 256, 512, 1024, 4096, 16_384, 65_536] {
        let points = imbalanced_points(n);
        let cheap = |_: usize, x: f64| x * x - 3.0 * x;
        let costly = |_: usize, x: f64| (1.0 + x * x).ln().exp().sqrt().tanh();

        let time = |backend: ComputeBackend, costly_mode: bool| -> f64 {
            let run = || {
                if costly_mode {
                    derivative_batch_min(&points, settings, backend, 0, costly)
                } else {
                    derivative_batch_min(&points, settings, backend, 0, cheap)
                }
            };
            std::hint::black_box(run());
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = run();
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            best
        };

        let cs = time(ComputeBackend::Serial, false);
        let cm = time(ComputeBackend::CpuMulti, false);
        let xs = time(ComputeBackend::Serial, true);
        let xm = time(ComputeBackend::CpuMulti, true);
        println!(
            "{n:>10} {cs:>14.2} {cm:>14.2} {:>9.2} {xs:>15.2} {xm:>15.2} {:>9.2}",
            cs / cm,
            xs / xm
        );
    }

    println!("\n-- jacobian_batch, n = 4, central (8 evals/lane), best of 7 --");
    println!(
        "{:>10} {:>14} {:>14} {:>9} {:>15} {:>15} {:>9}",
        "lanes",
        "cheap ser[us]",
        "cheap mul[us]",
        "speedup",
        "costly ser[us]",
        "costly mul[us]",
        "speedup"
    );
    let n = 4_usize;
    for lanes in [4_usize, 8, 16, 32, 64, 128, 256, 1024, 4096, 16_384] {
        let points = imbalanced_points(lanes * n);
        let cheap = |i: usize, v: &[f64], out: &mut Vec<f64>| {
            let k = 1.0 + (i % 13) as f64;
            out.clear();
            out.push(k * v[0] * v[1]);
            out.push(v[2] + v[3] * v[3]);
            out.push(v[0] * 1e-2 - v[2]);
            out.push(k * v[3] * v[1] * v[1]);
        };
        // A transcendental chain per component, standing in for a real
        // property evaluation inside a residual.
        let costly = |i: usize, v: &[f64], out: &mut Vec<f64>| {
            let k = 1.0 + (i % 13) as f64;
            out.clear();
            for c in 0..4 {
                let z = v[c];
                out.push(k * (1.0 + z * z).ln().exp().sqrt().tanh() + z.sin() * z.exp());
            }
        };
        let time = |backend: ComputeBackend, costly_mode: bool| -> f64 {
            let run = || {
                if costly_mode {
                    jacobian_batch_min(&points, n, settings, backend, 0, costly)
                } else {
                    jacobian_batch_min(&points, n, settings, backend, 0, cheap)
                }
            };
            std::hint::black_box(run());
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = run();
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            best
        };
        let cs = time(ComputeBackend::Serial, false);
        let cm = time(ComputeBackend::CpuMulti, false);
        let xs = time(ComputeBackend::Serial, true);
        let xm = time(ComputeBackend::CpuMulti, true);
        println!(
            "{lanes:>10} {cs:>14.2} {cm:>14.2} {:>9.2} {xs:>15.2} {xm:>15.2} {:>9.2}",
            cs / cm,
            xs / xm
        );
    }
}

/// Crossover benchmark for [`JACOBIAN_COLUMN_MIN_DIMENSION`] — the *other*
/// parallel axis, spreading one Jacobian's columns.
///
/// `#[ignore]`d.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 jacobian_column_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn jacobian_column_crossover_benchmark() {
    use std::time::Instant;

    println!(
        "available_parallelism() = {}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    );
    println!("JACOBIAN_COLUMN_MIN_DIMENSION = {JACOBIAN_COLUMN_MIN_DIMENSION}");
    println!("one Jacobian, columns spread across threads, central, best of 7");
    println!(
        "{:>10} {:>16} {:>14} {:>14} {:>9}",
        "dimension", "f evals", "serial[us]", "multi[us]", "speedup"
    );

    let settings = DiffSettings::central();
    for n in [4_usize, 8, 16, 32, 64, 128, 256, 512] {
        let x: Vec<f64> = (0..n).map(|i| 0.5 + (i % 11) as f64 * 0.25).collect();
        // O(n) work per component, so one evaluation is O(n^2) -- the shape a
        // real coupled residual has.
        let f = |_: usize, v: &[f64], out: &mut Vec<f64>| {
            out.clear();
            let m = v.len();
            for i in 0..m {
                let mut acc = 0.0;
                for k in 0..m {
                    acc += v[k] * ((i + k) % 7) as f64 * 0.001;
                }
                out.push(v[i].sin() + acc);
            }
        };
        let time = |backend: ComputeBackend| -> f64 {
            let run = || jacobian_columns_min(0, &x, settings, backend, 0, &f);
            std::hint::black_box(run());
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = run();
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            best
        };
        let s = time(ComputeBackend::Serial);
        let m = time(ComputeBackend::CpuMulti);
        println!(
            "{n:>10} {:>16} {s:>14.2} {m:>14.2} {:>9.2}",
            settings.scheme.evaluations_per_jacobian(n),
            s / m
        );
    }
}

/// Thread-scaling measurement with the bitwise-identity claim re-asserted at
/// each thread count.
///
/// `#[ignore]`d.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 differentiate_thread_scaling_benchmark
/// ```
#[cfg(feature = "parallel")]
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn differentiate_thread_scaling_benchmark() {
    use std::time::Instant;

    let n = 65_536_usize;
    let points = imbalanced_points(n);
    let settings = DiffSettings::central_4th();
    let f = |i: usize, x: f64| (x * (1.0 + i as f64 * 1e-3)).sin() + x.exp() * 1e-2;

    let reference = derivative_batch_min(&points, settings, ComputeBackend::Serial, 0, f);
    let mut serial_us = f64::INFINITY;
    for _ in 0..7 {
        let t = Instant::now();
        let out = derivative_batch_min(&points, settings, ComputeBackend::Serial, 0, f);
        std::hint::black_box(&out);
        serial_us = serial_us.min(t.elapsed().as_secs_f64() * 1.0e6);
    }

    println!(
        "available_parallelism() = {}",
        std::thread::available_parallelism()
            .map(|c| c.get())
            .unwrap_or(1)
    );
    println!("batch = {n} points, central-4th (4 evals/lane), best of 7");
    println!(
        "{:>8} {:>14} {:>9} {:>10}",
        "threads", "time [us]", "speedup", "bitwise"
    );
    println!(
        "{:>8} {serial_us:>14.2} {:>9.2} {:>10}",
        0, 1.0, "reference"
    );

    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        let (best, identical) = pool.install(|| {
            let first = derivative_batch_min(&points, settings, ComputeBackend::CpuMulti, 0, f);
            let identical = bitwise_same_derivatives(&first, &reference);
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = derivative_batch_min(&points, settings, ComputeBackend::CpuMulti, 0, f);
                std::hint::black_box(&out);
                best = best.min(t.elapsed().as_secs_f64() * 1.0e6);
            }
            (best, identical)
        });
        println!(
            "{threads:>8} {best:>14.2} {:>9.2} {:>10}",
            serial_us / best,
            if identical { "identical" } else { "DIFFERENT" }
        );
        assert!(identical, "thread count {threads} changed the result");
    }
}

/// The cost of a numerical Jacobian against a hand-coded one, inside a real
/// [`Rosenbrock23`] integration.
///
/// `#[ignore]`d.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release \
///     -- --ignored --nocapture --test-threads=1 numerical_jacobian_overhead_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn numerical_jacobian_overhead_benchmark() {
    use std::time::Instant;

    println!("Van der Pol mu = 5, y0 = [2, 0], x in [0, 10], tol 1e-8/1e-8");
    println!(
        "{:>16} {:>14} {:>14} {:>12}",
        "jacobian", "time [us]", "y0(10)", "vs analytic"
    );

    let time_run = |label: &str, run: &dyn Fn() -> (f64, f64), baseline: f64| -> f64 {
        let mut best = f64::INFINITY;
        let mut last = (0.0, 0.0);
        for _ in 0..5 {
            let t = Instant::now();
            last = run();
            best = best.min(t.elapsed().as_secs_f64() * 1.0e6);
        }
        println!(
            "{label:>16} {best:>14.2} {:>14.8} {:>12}",
            last.0,
            if baseline.is_finite() {
                format!("{:.2}x", best / baseline)
            } else {
                "1.00x".to_string()
            }
        );
        best
    };

    let analytic = || {
        let system = VanDerPol { mu: 5.0 };
        let mut solver = Rosenbrock23::new(2, 1e-8, 1e-8);
        let mut y = vec![2.0_f64, 0.0];
        let mut dx = 1e-4;
        let _ = solver.integrate(&system, 0.0, 10.0, &mut y, &mut dx);
        (y[0], y[1])
    };
    let base = time_run("analytic", &analytic, f64::INFINITY);

    for scheme in SCHEMES {
        let run = move || {
            let system =
                NumericalJacobian::new(VanDerPol { mu: 5.0 }, DiffSettings::with_scheme(scheme));
            let mut solver = Rosenbrock23::new(2, 1e-8, 1e-8);
            let mut y = vec![2.0_f64, 0.0];
            let mut dx = 1e-4;
            let _ = solver.integrate(&system, 0.0, 10.0, &mut y, &mut dx);
            (y[0], y[1])
        };
        time_run(scheme.label(), &run, base);
    }
}
