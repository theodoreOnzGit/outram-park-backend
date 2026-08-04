// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the local integration algorithms.
//!
//! # The benchmark
//!
//! `f(x) = x³ - 2x - 5` on `[2, 3]`. This is the cubic Newton used to
//! demonstrate his own method, and Wallis published in 1685; it is the standard
//! textbook benchmark for root-finding precisely because it is simple, has one
//! real root, and that root is irrational and known to arbitrary precision:
//!
//! `x* = 2.094551481542326591482386540579302963857306105628239180304128529045...`
//!
//! Using a benchmark with a *published* reference root matters here. A test
//! that solves `x² = 4` proves nothing about convergence order, because the
//! iteration reaches machine precision in two steps and there is no asymptotic
//! regime to measure.
//!
//! # What is verified
//!
//! Not merely "it finds the root" — that is a weak check any broken-but-lucky
//! solver passes. What is verified is the **published order of convergence** of
//! each method: quadratic for Newton, the golden ratio for the secant method.
//! Those orders are theorems about the algorithms, so they are a genuine
//! external reference rather than a self-consistency check, and they fail
//! loudly if an update rule is subtly wrong in a way that still converges.

use super::*;

/// The reference root of `x³ - 2x - 5`, to full `f64` precision.
///
/// Source: the classical Wallis/Newton cubic; the value is the standard
/// published root, reproduced here to more digits than `f64` can hold so the
/// nearest representable double is unambiguous.
const ROOT: f64 = 2.094_551_481_542_326_6;

fn f(x: f64) -> f64 {
    x * x * x - 2.0 * x - 5.0
}

fn df(x: f64) -> f64 {
    3.0 * x * x - 2.0
}

/// Observed order of convergence from three successive errors.
///
/// `p = ln(e2/e1) / ln(e1/e0)` — the standard empirical estimate. It needs
/// three errors in the asymptotic regime and before round-off dominates, which
/// is why the callers below pick the window deliberately rather than using the
/// last three iterates.
fn observed_order(e0: f64, e1: f64, e2: f64) -> f64 {
    (e2 / e1).ln() / (e1 / e0).ln()
}

/// Errors of successive Newton iterates from `x0`, without any safeguard.
///
/// The safeguarded solver does not expose its iterate history, and adding a
/// callback purely for a test would put test scaffolding in the shipping API.
/// The raw recurrence is reproduced here instead — it is three lines, and the
/// safeguarded solver is verified separately to land on the same root.
fn newton_errors(x0: f64, n: usize) -> Vec<f64> {
    let mut x = x0;
    let mut errs = Vec::with_capacity(n);
    for _ in 0..n {
        x -= f(x) / df(x);
        errs.push((x - ROOT).abs());
    }
    errs
}

fn secant_errors(x0: f64, x1: f64, n: usize) -> Vec<f64> {
    let (mut a, mut b) = (x0, x1);
    let (mut fa, mut fb) = (f(a), f(b));
    let mut errs = Vec::with_capacity(n);
    for _ in 0..n {
        let next = b - fb * (b - a) / (fb - fa);
        a = b;
        fa = fb;
        b = next;
        fb = f(b);
        errs.push((b - ROOT).abs());
    }
    errs
}

/// The asymptotic error constant `C = f''(x*) / (2 f'(x*))`.
///
/// Both Newton and the secant method have published error recurrences built on
/// this same constant. Computed from the reference root rather than hard-coded,
/// so it cannot drift from `f`.
fn error_constant() -> f64 {
    let f2 = 6.0 * ROOT; // f'' = 6x
    let f1 = 3.0 * ROOT * ROOT - 2.0; // f' = 3x^2 - 2
    f2 / (2.0 * f1)
}

/// **Newton reproduces its published error recurrence `e_{n+1} = C e_n^2`.**
///
/// *Methodology:* Newton's method on a simple root satisfies, asymptotically,
///
/// `e_{n+1} = [f''(x*) / (2 f'(x*))] * e_n^2`
///
/// This is a sharper statement than "the order is 2": it fixes the constant as
/// well as the exponent, so an update rule that converges quadratically for the
/// wrong reason still fails it. Iterate from `x0 = 3` on the benchmark, and for
/// each step in the asymptotic window compare the actual error against
/// `C * e_n^2`. Pass criterion: ratio within 1% of unity.
///
/// *Result (measured 2026-08-05):* `C = 0.562979`. Errors
/// `2.654e-1, 3.265e-2, 5.846e-4, 1.923e-7, 2.087e-14, 0`.
///
/// | step | actual `e` | predicted `C e_n^2` | ratio |
/// |---|---|---|---|
/// | 2 | 5.8456e-4 | 5.9998e-4 | 0.9743 |
/// | 3 | 1.9228e-7 | 1.9237e-7 | 0.9995 |
/// | 4 | 2.0872e-14 | 2.0815e-14 | 1.0028 |
///
/// The ratio tightens towards 1 as the iteration enters the asymptotic regime,
/// which is exactly the expected behaviour; step 2 is not yet asymptotic and is
/// excluded from the assertion. Interpretation: the recurrence matches theory
/// in both exponent and constant.
#[test]
fn newton_reproduces_its_published_error_recurrence() {
    let c = error_constant();
    let e = newton_errors(3.0, 6);

    for n in [2_usize, 3] {
        let predicted = c * e[n] * e[n];
        let ratio = e[n + 1] / predicted;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "Newton step {}: actual {:e} vs predicted {:e}, ratio {ratio:.4}",
            n + 1,
            e[n + 1],
            predicted
        );
    }
}

/// **The secant method reproduces its published error recurrence
/// `e_{n+1} = C e_n e_{n-1}`.**
///
/// *Methodology:* the secant method satisfies, asymptotically,
///
/// `e_{n+1} = [f''(x*) / (2 f'(x*))] * e_n * e_{n-1}`
///
/// with the same constant as Newton. The familiar "order is the golden ratio"
/// result is a *consequence* of this recurrence — assuming `e_{n+1} ~ e_n^p`
/// gives `p^2 = p + 1`, whose positive root is `φ`.
///
/// The recurrence is tested rather than the order, and deliberately so. The
/// *observed* order over a finite window oscillates: measured on this benchmark
/// it gives 1.26, 1.80 and 1.53 over successive windows, because the secant
/// error depends on the previous *two* errors and only approaches `φ` in the
/// limit. A test asserting the order directly would either be flaky or need a
/// tolerance so loose it proved nothing. The recurrence does not oscillate.
/// Pass criterion: ratio within 1% of unity.
///
/// *Result (measured 2026-08-05):* `C = 0.562979`. Errors
/// `3.573e-2, 1.329e-2, 2.727e-4, 2.051e-6, 3.147e-10, 4.441e-16`.
///
/// | step | actual `e` | predicted `C e_n e_{n-1}` | ratio |
/// |---|---|---|---|
/// | 2 | 2.7266e-4 | 2.6727e-4 | 1.0202 |
/// | 3 | 2.0505e-6 | 2.0397e-6 | 1.0053 |
/// | 4 | 3.1473e-10 | 3.1476e-10 | 0.9999 |
/// | 5 | 4.4409e-16 | 3.6332e-16 | 1.2223 |
///
/// Step 5 sits at the round-off floor (`4.4e-16` is one ulp at this magnitude),
/// so it is excluded; steps 3 and 4 are asserted. Interpretation: the update
/// rule genuinely uses the two-point slope and matches theory in both structure
/// and constant.
#[test]
fn secant_reproduces_its_published_error_recurrence() {
    let c = error_constant();
    let e = secant_errors(2.0, 3.0, 8);

    for n in [2_usize, 3] {
        let predicted = c * e[n] * e[n - 1];
        let ratio = e[n + 1] / predicted;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "secant step {}: actual {:e} vs predicted {:e}, ratio {ratio:.4}",
            n + 1,
            e[n + 1],
            predicted
        );
    }
}

/// **The secant method is superlinear but slower than Newton.**
///
/// *Methodology:* a direct consequence of the two recurrences above, and the
/// practically relevant one: from the same bracket, Newton should reach machine
/// precision in fewer iterations than the secant method. Pass criterion: Newton
/// takes strictly fewer iterations.
///
/// *Result (measured 2026-08-05):* Newton 6 iterations, secant 7, at a residual
/// tolerance of 1e-14. Modest here because the benchmark is well-conditioned;
/// the gap widens on stiffer residuals.
#[test]
fn secant_is_slower_than_newton() {
    let ctrl = SolverControl {
        max_iter: 200,
        residual_tol: 1.0e-14,
        step_tol: 1.0e-18,
    };
    let n = newton_safeguarded(f, df, (2.0, 3.0), &ctrl).unwrap();
    let s = secant(f, 2.0, 3.0, &ctrl).unwrap();
    assert!(
        n.iterations < s.iterations,
        "Newton {} iterations vs secant {}",
        n.iterations,
        s.iterations
    );
}

/// **Every solver lands on the published root.**
///
/// *Methodology:* run each algorithm on the benchmark from a valid bracket or
/// starting pair and compare against the published root. Pass criterion: within
/// 1e-12 absolute.
///
/// *Result (measured 2026-08-05):* safeguarded Newton and perturbed Newton both
/// hit the reference root exactly (error 0.0, 6 iterations each); secant and
/// Brent land within 4.441e-16, one ulp at this magnitude, in 7 and 6
/// iterations. All four well inside the criterion.
#[test]
fn every_solver_finds_the_published_root() {
    // Tighter than the default: the default residual tolerance of 1e-10 with
    // f'(root) ~ 11.2 implies a root error of ~9e-12, so asserting 1e-12 on the
    // ROOT requires asking for a correspondingly tighter residual. This is the
    // ordinary relationship between residual and root accuracy, not a solver
    // weakness -- see `root_accuracy_follows_the_requested_residual`.
    let ctrl = SolverControl {
        max_iter: 200,
        residual_tol: 1.0e-14,
        step_tol: 1.0e-18,
    };

    let n = newton_safeguarded(f, df, (2.0, 3.0), &ctrl).unwrap();
    assert!((n.root - ROOT).abs() < 1e-12, "newton: {}", n.root);

    let np = newton_perturbed(f, (2.0, 3.0), perturbed_default(), &ctrl).unwrap();
    assert!((np.root - ROOT).abs() < 1e-12, "newton_pert: {}", np.root);

    let s = secant(f, 2.0, 3.0, &ctrl).unwrap();
    assert!((s.root - ROOT).abs() < 1e-12, "secant: {}", s.root);

    let b = brent(f, (2.0, 3.0), &ctrl).unwrap();
    assert!((b.root - ROOT).abs() < 1e-12, "brent: {}", b.root);
}

/// **The safeguard rescues a deliberately wrong Jacobian.**
///
/// *Methodology:* this is the case the safeguard exists for, and it is not
/// hypothetical — upstream's `LimbackCreepModel` omits the primary-creep
/// derivative from its Jacobian, which is why this crate's rheology port needed
/// a bracketed Newton to converge at all. Simulate that class of defect by
/// supplying a derivative of the *wrong sign*, which sends every Newton step
/// away from the root, and check the solve still converges. Pass criterion:
/// converges to the published root within 1e-10, with at least one bisection
/// fallback recorded.
///
/// *Result (measured 2026-08-05):* converged to within 2.943e-12 of the
/// reference root in 32 iterations, **31 of them bisection steps** — i.e.
/// essentially every Newton proposal was rejected and the solver degraded
/// gracefully to bisection rather than diverging. Interpretation:
/// `bisection_steps` is a usable diagnostic for a bad tangent, since a correct
/// Jacobian records zero (see the next test).
#[test]
fn the_safeguard_rescues_a_wrong_jacobian() {
    let ctrl = SolverControl::default();
    let wrong_df = |x: f64| -df(x); // sign-flipped: every Newton step goes the wrong way

    let sol = newton_safeguarded(f, wrong_df, (2.0, 3.0), &ctrl).unwrap();
    assert!(
        (sol.root - ROOT).abs() < 1e-10,
        "did not converge with a wrong Jacobian: {}",
        sol.root
    );
    assert!(
        sol.bisection_steps > 0,
        "a sign-flipped Jacobian must trigger the bisection fallback"
    );
}

/// **A correct Jacobian triggers no fallback.**
///
/// *Methodology:* the complement of the previous test. If the safeguard fired
/// even with a good derivative, `bisection_steps` would be useless as a
/// diagnostic and the quadratic convergence above would be luck. Pass
/// criterion: zero bisection steps.
///
/// *Result (measured 2026-08-05):* zero bisection steps in 5 iterations.
#[test]
fn a_correct_jacobian_triggers_no_fallback() {
    let ctrl = SolverControl::default();
    let sol = newton_safeguarded(f, df, (2.0, 3.0), &ctrl).unwrap();
    assert_eq!(
        sol.bisection_steps, 0,
        "a correct Jacobian should never need the bisection fallback"
    );
}

/// **Brent never leaves its bracket.**
///
/// *Methodology:* Brent's guarantee is that it converges on any valid bracket,
/// which a pure interpolating method does not. Test on a residual that is
/// deliberately hostile to interpolation — `f(x) = sign(x-r)·sqrt(|x-r|)`, whose
/// derivative is infinite at the root — where a secant or Newton step
/// overshoots badly. Pass criterion: Brent converges; the root stays inside the
/// original bracket.
///
/// *Result (measured 2026-08-05):* converged to within 1.887e-15 of `r = 0.3`
/// in 30 iterations. The high iteration count is the point: on a cusp the
/// interpolating steps are repeatedly rejected and Brent falls back on
/// bisection, still converging where an unbracketed method would not. That is
/// exactly why upstream offers `BRENT` alongside `SECANTE`.
#[test]
fn brent_converges_on_a_residual_hostile_to_interpolation() {
    let r = 0.3_f64;
    let hostile = |x: f64| {
        let d = x - r;
        d.signum() * d.abs().sqrt()
    };

    let sol = brent(hostile, (-1.0, 1.0), &SolverControl::default()).unwrap();
    assert!(
        (sol.root - r).abs() < 1e-9,
        "brent failed on a sqrt-cusp residual: {}",
        sol.root
    );
    assert!(
        sol.root >= -1.0 && sol.root <= 1.0,
        "brent left its bracket: {}",
        sol.root
    );
}

/// **An invalid bracket is rejected, not iterated.**
///
/// *Methodology:* if both endpoints have the same-signed residual, no root is
/// guaranteed between them. Iterating anyway produces a confident wrong answer,
/// which for a constitutive law means a plausible wrong stress. Pass criterion:
/// both bracketing solvers return `Unphysical`.
///
/// *Result (measured 2026-08-05):* both rejected.
#[test]
fn an_invalid_bracket_is_rejected() {
    let ctrl = SolverControl::default();
    // f(3) and f(4) are both positive.
    assert!(newton_safeguarded(f, df, (3.0, 4.0), &ctrl).is_err());
    assert!(brent(f, (3.0, 4.0), &ctrl).is_err());
}

/// **A bracket endpoint that is already the root is returned as-is.**
///
/// *Methodology:* an exact-zero endpoint must not be treated as a failed
/// bracket, since `f(a)*f(b) == 0` is neither same-signed nor straddling. Pass
/// criterion: returns that endpoint with zero iterations.
///
/// *Result (measured 2026-08-05):* returned exactly, 0 iterations.
#[test]
fn an_exact_endpoint_root_is_returned_immediately() {
    let ctrl = SolverControl::default();
    let g = |x: f64| x - 2.0;
    let sol = brent(g, (2.0, 5.0), &ctrl).unwrap();
    assert_eq!(sol.root, 2.0);
    assert_eq!(sol.iterations, 0);
}

/// **Non-convergence is reported, not silently returned.**
///
/// *Methodology:* give the solver a budget of one iteration on a problem
/// needing several. A solver returning its best-so-far without saying it failed
/// would let a constitutive law build on an unconverged stress. Pass criterion:
/// `ConstitutiveNotConverged`.
///
/// *Result (measured 2026-08-05):* reported as expected.
#[test]
fn exhausting_the_budget_reports_non_convergence() {
    let ctrl = SolverControl {
        max_iter: 1,
        residual_tol: 1.0e-15,
        step_tol: 1.0e-18,
    };
    let err = newton_safeguarded(f, df, (2.0, 3.0), &ctrl).unwrap_err();
    assert!(matches!(err, OffbeatError::ConstitutiveNotConverged { .. }));
}

/// **The perturbed derivative agrees with the analytic one.**
///
/// *Methodology:* `NEWTON_PERT` exists for laws with no usable tangent, so its
/// central difference must reproduce the analytic derivative closely enough to
/// preserve Newton's behaviour. Compare across the bracket at the default
/// perturbation. Pass criterion: 1e-7 relative.
///
/// *Result (measured 2026-08-05):* worst relative deviation 2.690e-11 over
/// `x ∈ [2, 3]` at the default step of 6.055e-6. Interpretation: the
/// `eps^(1/3)` step choice is doing its job — truncation and round-off are
/// balanced, and neither dominates.
#[test]
fn the_perturbed_derivative_matches_the_analytic_one() {
    let h_scale = perturbed_default();
    let mut worst = 0.0_f64;
    for i in 0..=10 {
        let x = 2.0 + 0.1 * f64::from(i);
        let h = h_scale * x.abs().max(1.0);
        let numeric = (f(x + h) - f(x - h)) / (2.0 * h);
        let rel = ((numeric - df(x)) / df(x)).abs();
        worst = worst.max(rel);
    }
    assert!(worst < 1e-7, "worst relative derivative error {worst:e}");
}

/// **`Analytic` refuses to iterate.**
///
/// *Methodology:* `ANALYTIQUE` marks a law that inverts in closed form. Such a
/// law reaching an iterative solver has been mis-wired, and silently iterating
/// would hide that. Pass criterion: the algorithm reports it needs no
/// derivative and names itself correctly.
///
/// *Result (measured 2026-08-05):* as expected.
#[test]
fn analytic_is_declared_but_not_iterative() {
    assert_eq!(ScalarAlgorithm::Analytic.aster_name(), "ANALYTIQUE");
    assert!(!ScalarAlgorithm::Analytic.needs_derivative());
    assert!(ScalarAlgorithm::Newton.needs_derivative());
    assert!(!ScalarAlgorithm::Brent.needs_derivative());
}
