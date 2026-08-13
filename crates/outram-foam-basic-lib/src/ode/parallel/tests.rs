// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Tests for [`crate::ode::parallel`] — batched ODE ensembles and batched
//! quadrature.
//!
//! Four groups, in file order:
//!
//! 1. **Analytic verification** — every kernel against a closed form, so the
//!    oracle is exact rather than another implementation: exponential decay, the
//!    harmonic oscillator, a decoupled stiff pair, polynomial exactness of the
//!    Gauss and Simpson rules, and three integrals with known values.
//! 2. **Determinism** — bitwise equality between `Serial` and `CpuMulti` at 1,
//!    2, 4 and 8 worker threads, on batches built to have wildly uneven per-lane
//!    cost.
//! 3. **Failure reporting** — every [`OdeLaneStatus`] and every
//!    [`QuadratureStatus`] failure path, and the all-or-nothing
//!    [`OdeEnsemble::states`] / [`QuadratureBatch::values`] errors.
//! 4. **Measurement** — `#[ignore]`d crossover benchmarks whose printed output
//!    is the source of every number in the constants' documentation.

use super::*;
use crate::matrix::SquareMatrix;
use crate::ode::{OdeSolver, OdeSystem};

// ── Deterministic pseudorandom source (fixed seed, no `rand` dependency) ─────

/// xorshift64\* — a fixed-seed generator so every test is reproducible.
struct Xorshift(u64);

impl Xorshift {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform on `[0, 1)`.
    fn next_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Uniform on `[lo, hi)`.
    fn next_in(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_unit()
    }
}

// ── Test systems, all with closed-form solutions ─────────────────────────────

/// `dy/dx = -k y`, `y(0) = y0` — closed form `y(x) = y0 exp(-k x)`.
struct Decay {
    k: f64,
}

impl OdeSystem for Decay {
    fn n_eqns(&self) -> usize {
        1
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -self.k * y[0];
    }
    fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdy.set(0, 0, -self.k);
    }
}

/// `y1' = -y2`, `y2' = y1` — a rotation; `y(pi/2) = (-y2(0), y1(0))`.
struct Harmonic;

impl OdeSystem for Harmonic {
    fn n_eqns(&self) -> usize {
        2
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -y[1];
        dydx[1] = y[0];
    }
    fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdx[1] = 0.0;
        dfdy.set(0, 0, 0.0);
        dfdy.set(0, 1, -1.0);
        dfdy.set(1, 0, 1.0);
        dfdy.set(1, 1, 0.0);
    }
}

/// A **decoupled stiff pair** with a closed form: `y1' = -y1`, `y2' = -s y2`.
///
/// The eigenvalues are `-1` and `-s`; with `s = 1e5` the stiffness ratio is
/// `1e5`, which is far beyond what an explicit stepper can span in a bounded
/// number of steps. Closed form `y(x) = (exp(-x), exp(-s x))`.
struct StiffPair {
    s: f64,
}

impl OdeSystem for StiffPair {
    fn n_eqns(&self) -> usize {
        2
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -y[0];
        dydx[1] = -self.s * y[1];
    }
    fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdx[1] = 0.0;
        dfdy.set(0, 0, -1.0);
        dfdy.set(0, 1, 0.0);
        dfdy.set(1, 0, 0.0);
        dfdy.set(1, 1, -self.s);
    }
}

/// `dy/dx = 1 / (x - blow_up)` scaled to overflow — used only to drive a lane
/// non-finite on purpose.
struct Blowup;

impl OdeSystem for Blowup {
    fn n_eqns(&self) -> usize {
        1
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        // y' = y^2 blows up in finite x from y(0) = 1 at x = 1.
        dydx[0] = y[0] * y[0];
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. Analytic verification
// ═════════════════════════════════════════════════════════════════════════════

/// Every stepper, driven through the ensemble, reproduces `exp(-k x)`.
///
/// **Methodology.** 64 lanes of `dy/dx = -k y`, `y(0) = 1`, with `k` spread
/// evenly over `[0.5, 8]`, integrated from `x = 0` to `x = 1` and compared
/// against the closed form `exp(-k)`. Solvers: `Euler` (`abs_tol = 1e-3`,
/// `rel_tol = 1e-2` — a first-order stepper cannot reach the high-order
/// tolerances inside the default 10 000-step budget on the fastest lanes, and
/// asking it to would test the budget rather than the stepper), `Rkf45` and
/// `Rosenbrock23` (both `1e-10` / `1e-8`).
///
/// **Pass criterion.** Worst absolute error over the 64 lanes below `5e-2` for
/// first-order `Euler` and below `1e-8` for the two high-order steppers, with
/// [`OdeLaneStatus::Completed`] on every lane.
///
/// **Results** — see the module doc on [`crate::ode::parallel::integrate_ensemble`],
/// which transcribes this test's printed output.
#[test]
fn ensemble_matches_analytic_decay() {
    let n = 64;
    let ks: Vec<f64> = (0..n)
        .map(|i| 0.5 + 7.5 * i as f64 / (n as f64 - 1.0))
        .collect();
    let lanes: Vec<OdeLane<Decay>> = ks
        .iter()
        .map(|&k| OdeLane::new(Decay { k }, vec![1.0], 0.0, 1.0, 0.05))
        .collect();

    for (solver, tol) in [
        (OdeSolver::euler(1, 1e-3, 1e-2), 5e-2),
        (OdeSolver::rkf45(1, 1e-10, 1e-8), 1e-8),
        (OdeSolver::rosenbrock23(1, 1e-10, 1e-8), 1e-8),
    ] {
        let name = solver.name();
        let ensemble = integrate_ensemble(&lanes, &solver, ComputeBackend::CpuMulti);
        assert!(
            ensemble.all_completed(),
            "{name}: {:?}",
            ensemble.first_failure().map(|(i, l)| (i, l.status()))
        );
        let states = ensemble.states().expect("all lanes complete");
        let mut worst = 0.0_f64;
        for (k, s) in ks.iter().zip(&states) {
            worst = worst.max((s[0] - (-k).exp()).abs());
        }
        println!(
            "decay ensemble: {name} worst |error| = {worst:.6e} over {n} lanes, \
             total steps = {}, max lane steps = {}",
            ensemble.total_steps(),
            ensemble.max_steps_taken()
        );
        assert!(worst < tol, "{name}: worst error {worst:.6e} >= {tol:.1e}");
    }
}

/// The two-equation harmonic oscillator, through the ensemble.
///
/// **Methodology.** 16 lanes of `y1' = -y2`, `y2' = y1` from `y(0) = (1, 0)`,
/// each integrated to a different endpoint `x_end = m * pi/2` for `m = 1..=16`.
/// The closed form is a rotation by `x`, so `y(x) = (cos x, sin x)`.
///
/// **Pass criterion.** Worst absolute error below `1e-8` with `Rkf45` at
/// `abs_tol = 1e-12`, `rel_tol = 1e-10`, on every lane and both components.
///
/// **Results** — transcribed onto
/// [`crate::ode::parallel::integrate_ensemble`] from this test's printed output.
#[test]
fn ensemble_matches_harmonic_oscillator() {
    let n = 16;
    let ends: Vec<f64> = (1..=n)
        .map(|m| m as f64 * std::f64::consts::FRAC_PI_2)
        .collect();
    let lanes: Vec<OdeLane<Harmonic>> = ends
        .iter()
        .map(|&x_end| OdeLane::new(Harmonic, vec![1.0, 0.0], 0.0, x_end, 0.05))
        .collect();

    let ensemble = integrate_ensemble(
        &lanes,
        &OdeSolver::rkf45(2, 1e-12, 1e-10),
        ComputeBackend::CpuMulti,
    );
    let states = ensemble.states().expect("all lanes complete");

    let mut worst = 0.0_f64;
    for (x, s) in ends.iter().zip(&states) {
        worst = worst.max((s[0] - x.cos()).abs()).max((s[1] - x.sin()).abs());
    }
    println!("harmonic ensemble: worst |error| = {worst:.6e} over {n} lanes");
    assert!(worst < 1e-8, "worst error {worst:.6e}");
}

/// A stiff pair is integrated correctly by `Rosenbrock23` and **visibly fails**
/// under `Rkf45` — stiffness is reported, never swallowed.
///
/// **Methodology.** `y1' = -y1`, `y2' = -1e5 y2` from `y(0) = (1, 1)` to
/// `x = 1`; closed form `(exp(-1), exp(-1e5))`, the second of which underflows
/// to exactly `0` in `f64`. Two ensembles over identical lanes, one with
/// `Rosenbrock23` and one with `Rkf45`, both at `abs_tol = 1e-10`,
/// `rel_tol = 1e-8` and the default `max_steps = 10 000`.
///
/// **Pass criterion.** The `Rosenbrock23` ensemble completes with `|y1 -
/// exp(-1)| < 1e-7` and `|y2| < 1e-30`. The `Rkf45` ensemble does **not**
/// complete, and its failure status is one of the two budget failures
/// ([`OdeLaneStatus::MaxStepsExceeded`] or
/// [`OdeLaneStatus::StepSizeUnderflow`]) rather than a silently wrong
/// completion. An explicit stepper needs a step of roughly `2.8 / 1e5` for
/// stability, i.e. about 36 000 steps to span the interval, which the default
/// budget cannot supply.
///
/// **Results.** Transcribed from this test's printed output, release build,
/// 2026-08-13 — see the assertion messages below and the summary in the report.
#[test]
fn stiff_pair_completes_stiff_and_fails_visibly_explicit() {
    let lanes = vec![OdeLane::new(
        StiffPair { s: 1.0e5 },
        vec![1.0, 1.0],
        0.0,
        1.0,
        1.0e-3,
    )];

    let stiff = integrate_ensemble(
        &lanes,
        &OdeSolver::rosenbrock23(2, 1e-10, 1e-8),
        ComputeBackend::Serial,
    );
    let l = &stiff.lanes()[0];
    println!(
        "stiff pair / Rosenbrock23: status = {}, steps = {}, y = {:?}",
        l.status().label(),
        l.steps(),
        l.last_state()
    );
    let state = stiff.states().expect("Rosenbrock23 spans the stiff interval");
    assert!(
        (state[0][0] - (-1.0_f64).exp()).abs() < 1e-7,
        "y1 = {}",
        state[0][0]
    );
    assert!(state[0][1].abs() < 1e-30, "y2 = {}", state[0][1]);

    let explicit = integrate_ensemble(
        &lanes,
        &OdeSolver::rkf45(2, 1e-10, 1e-8),
        ComputeBackend::Serial,
    );
    let l = &explicit.lanes()[0];
    println!(
        "stiff pair / Rkf45:        status = {}, steps = {}, x_reached = {:.6e}",
        l.status().label(),
        l.steps(),
        l.x_reached()
    );
    assert!(!l.completed(), "Rkf45 must not claim to span a stiff interval");
    assert!(
        matches!(
            l.status(),
            OdeLaneStatus::MaxStepsExceeded | OdeLaneStatus::StepSizeUnderflow
        ),
        "unexpected status {:?}",
        l.status()
    );
    // The partial state is genuine, not NaN, and sits on the true trajectory.
    assert!(l.last_state().iter().all(|v| v.is_finite()));
    assert!(l.x_reached() > 0.0 && l.x_reached() < 1.0);
    assert!(explicit.states().is_err(), "all-or-nothing must refuse");
}

/// An `n`-point Gauss-Legendre rule integrates every polynomial of degree
/// `2n - 1` or less exactly.
///
/// **Methodology.** This is the defining property of the rule, so it is a
/// complete self-check of the computed nodes and weights and of the composite
/// mapping onto an arbitrary interval. For each [`GaussOrder`], each degree
/// `d` from `0` to `exact_degree()`, and each of the intervals `[0, 1]` and
/// `[-2, 3]`, integrate `x^d` with a **single** panel and compare against the
/// closed form `(b^(d+1) - a^(d+1)) / (d + 1)`.
///
/// **Pass criterion.** Relative error below `1e-13`.
///
/// **Results** — transcribed onto
/// [`crate::ode::parallel::quadrature_batch`] from this test's printed output.
#[test]
fn gauss_legendre_is_exact_to_its_degree() {
    let mut worst = 0.0_f64;
    for order in [
        GaussOrder::G2,
        GaussOrder::G3,
        GaussOrder::G4,
        GaussOrder::G5,
        GaussOrder::G8,
    ] {
        for d in 0..=order.exact_degree() {
            for (a, b) in [(0.0_f64, 1.0_f64), (-2.0, 3.0)] {
                let batch = quadrature_batch_min(
                    &[QuadratureInterval::new(a, b)],
                    QuadratureRule::GaussLegendre { order, panels: 1 },
                    ComputeBackend::Serial,
                    0,
                    |_, x| x.powi(d as i32),
                );
                let got = batch.values().expect("evaluates")[0];
                let exact =
                    (b.powi(d as i32 + 1) - a.powi(d as i32 + 1)) / (d as f64 + 1.0);
                let rel = (got - exact).abs() / exact.abs().max(1.0);
                worst = worst.max(rel);
                assert!(
                    rel < 1e-13,
                    "{order:?} degree {d} on [{a}, {b}]: {got} vs {exact}, rel {rel:.6e}"
                );
            }
        }
    }
    println!("gauss exactness: worst relative error = {worst:.6e}");
}

/// The computed 8-point Gauss-Legendre nodes and weights agree with the
/// Abramowitz & Stegun 25.4.30 values already carried in this workspace.
///
/// **Methodology.** `crates/raffles/src/distributions.rs` carries the positive
/// half of the 8-point abscissae and weights, cited to A&S 25.4.30, for its
/// `integrate_open_unit`. This crate computes its nodes by Newton iteration on
/// the Legendre polynomial instead of transcribing a table, so the two are
/// genuinely independent routes to the same constants and comparing them checks
/// the computation against a published reference without adding a dependency on
/// `raffles` (which the workspace layering forbids — `outram-foam-basic-lib` has
/// no internal deps). The four values are reproduced literally below.
///
/// **Pass criterion.** Absolute agreement to `1e-15` on every node and weight,
/// and weights summing to `2`.
///
/// **Results** — printed by this test; see the report.
#[test]
fn gauss_nodes_match_the_in_workspace_abramowitz_stegun_values() {
    // A&S 25.4.30, as carried in crates/raffles/src/distributions.rs.
    const AS_NODES: [f64; 4] = [
        0.183_434_642_495_649_8,
        0.525_532_409_916_329_0,
        0.796_666_477_413_626_7,
        0.960_289_856_497_536_3,
    ];
    const AS_WEIGHTS: [f64; 4] = [
        0.362_683_783_378_362_0,
        0.313_706_645_877_887_3,
        0.222_381_034_453_374_5,
        0.101_228_536_290_376_3,
    ];

    let nodes = gauss_legendre_nodes(8);
    assert_eq!(nodes.len(), 8);
    // Ascending order, so the positive half is the last four, in the same order
    // as the A&S table.
    let mut worst_node = 0.0_f64;
    let mut worst_weight = 0.0_f64;
    for (k, &(x, w)) in nodes[4..].iter().enumerate() {
        worst_node = worst_node.max((x - AS_NODES[k]).abs());
        worst_weight = worst_weight.max((w - AS_WEIGHTS[k]).abs());
    }
    let weight_sum: f64 = nodes.iter().map(|&(_, w)| w).sum();
    println!(
        "GL8 vs A&S 25.4.30: worst |node diff| = {worst_node:.6e}, \
         worst |weight diff| = {worst_weight:.6e}, weight sum = {weight_sum:.17}"
    );
    assert!(worst_node < 1e-15, "node mismatch {worst_node:.6e}");
    assert!(worst_weight < 1e-15, "weight mismatch {worst_weight:.6e}");
    assert!((weight_sum - 2.0).abs() < 1e-15, "weight sum {weight_sum}");

    // Symmetry: nodes come in +/- pairs with equal weights.
    for k in 0..4 {
        assert!((nodes[k].0 + nodes[7 - k].0).abs() < 1e-15);
        assert!((nodes[k].1 - nodes[7 - k].1).abs() < 1e-15);
    }
}

/// Composite Simpson is exact for cubics and trapezoid for linears.
///
/// **Methodology.** Integrate `x^d` over `[0, 1]` and `[-2, 3]`, with Simpson
/// over 3 panels for `d <= 3` and trapezoid over 5 panels for `d <= 1`, against
/// the same closed form used by
/// [`gauss_legendre_is_exact_to_its_degree`].
///
/// **Pass criterion.** Relative error below `1e-13`.
///
/// **Results** — transcribed onto
/// [`crate::ode::parallel::quadrature_batch`] from this test's printed output.
#[test]
fn simpson_and_trapezoid_are_exact_to_their_degree() {
    let mut worst_simpson = 0.0_f64;
    let mut worst_trapezoid = 0.0_f64;
    for (a, b) in [(0.0_f64, 1.0_f64), (-2.0, 3.0)] {
        let exact = |d: usize| (b.powi(d as i32 + 1) - a.powi(d as i32 + 1)) / (d as f64 + 1.0);
        for d in 0..=3usize {
            let batch = quadrature_batch_min(
                &[QuadratureInterval::new(a, b)],
                QuadratureRule::Simpson { panels: 3 },
                ComputeBackend::Serial,
                0,
                |_, x| x.powi(d as i32),
            );
            let got = batch.values().unwrap()[0];
            let rel = (got - exact(d)).abs() / exact(d).abs().max(1.0);
            worst_simpson = worst_simpson.max(rel);
            assert!(rel < 1e-13, "simpson degree {d}: {got} vs {}", exact(d));
        }
        for d in 0..=1usize {
            let batch = quadrature_batch_min(
                &[QuadratureInterval::new(a, b)],
                QuadratureRule::Trapezoid { panels: 5 },
                ComputeBackend::Serial,
                0,
                |_, x| x.powi(d as i32),
            );
            let got = batch.values().unwrap()[0];
            let rel = (got - exact(d)).abs() / exact(d).abs().max(1.0);
            worst_trapezoid = worst_trapezoid.max(rel);
            assert!(rel < 1e-13, "trapezoid degree {d}: {got} vs {}", exact(d));
        }
    }
    println!(
        "simpson worst relative error (degree <= 3) = {worst_simpson:.6e}; \
         trapezoid worst relative error (degree <= 1) = {worst_trapezoid:.6e}"
    );
}

/// The fixed rules reproduce a transcendental integral with a closed form, and
/// the order hierarchy behaves as theory requires.
///
/// **Methodology.** `integral of exp(-x) sin(x) dx from 0 to pi` has the closed
/// form `(1 + exp(-pi)) / 2`. Evaluated with `G8` over 8 panels (64
/// evaluations), Simpson over 64 panels (129 evaluations) and trapezoid over
/// 128 panels (129 evaluations).
///
/// **Pass criterion.** `G8` below `1e-12` absolute; Simpson below `1e-7`;
/// trapezoid below `1e-4`; and `G8` strictly more accurate than Simpson, which
/// is strictly more accurate than trapezoid.
///
/// **Results** — transcribed onto
/// [`crate::ode::parallel::quadrature_batch`] from this test's printed output.
#[test]
fn fixed_rules_match_a_transcendental_reference() {
    let exact = (1.0 + (-std::f64::consts::PI).exp()) / 2.0;
    let iv = [QuadratureInterval::new(0.0, std::f64::consts::PI)];
    let f = |_: usize, x: f64| (-x).exp() * x.sin();

    let run = |rule: QuadratureRule| -> (f64, f64) {
        let batch = quadrature_batch_min(&iv, rule, ComputeBackend::Serial, 0, f);
        let v = batch.values().expect("evaluates")[0];
        (v, (v - exact).abs())
    };

    let (g8, e_g8) = run(QuadratureRule::GaussLegendre {
        order: GaussOrder::G8,
        panels: 8,
    });
    let (si, e_si) = run(QuadratureRule::Simpson { panels: 64 });
    let (tr, e_tr) = run(QuadratureRule::Trapezoid { panels: 128 });

    println!("closed form                = {exact:.17}");
    println!("G8 x 8 panels  (64 evals)  = {g8:.17}, error {e_g8:.6e}");
    println!("Simpson x 64   (129 evals) = {si:.17}, error {e_si:.6e}");
    println!("Trapezoid x 128 (129 evals)= {tr:.17}, error {e_tr:.6e}");

    assert!(e_g8 < 1e-12, "G8 error {e_g8:.6e}");
    assert!(e_si < 1e-7, "Simpson error {e_si:.6e}");
    assert!(e_tr < 1e-4, "trapezoid error {e_tr:.6e}");
    assert!(e_g8 < e_si, "G8 must beat Simpson");
    assert!(e_si < e_tr, "Simpson must beat trapezoid");
}

/// Adaptive quadrature reproduces three closed forms, including one a fixed rule
/// handles badly.
///
/// **Methodology.** Three lanes, `abs_tol = 1e-11`, `rel_tol = 1e-10`,
/// `max_subdivisions = 100 000`:
///
/// 1. `integral of exp(-x) sin(x) from 0 to pi = (1 + exp(-pi)) / 2` — smooth.
/// 2. `integral of sqrt(x) from 0 to 1 = 2/3` — bounded, but with an infinite
///    derivative at the lower limit, which is the classic case a uniform panel
///    layout resolves slowly.
/// 3. `integral of 1 / (1 + 400 (x - 1/2)^2) from 0 to 1 = atan(10) / 10` — a
///    narrow peak occupying about a twentieth of the interval.
///
/// **Pass criterion.** Absolute error below `1e-9` on every lane, every lane
/// [`QuadratureStatus::Evaluated`], and the reported
/// [`QuadratureSolution::error_estimate`] no more than 100x smaller than the
/// true error (an error estimate that badly understates the error would be worse
/// than none).
///
/// **Results** — transcribed onto
/// [`crate::ode::parallel::adaptive_quadrature_batch`] from this test's printed
/// output.
#[test]
fn adaptive_matches_closed_forms() {
    let pi = std::f64::consts::PI;
    let intervals = [
        QuadratureInterval::new(0.0, pi),
        QuadratureInterval::new(0.0, 1.0),
        QuadratureInterval::new(0.0, 1.0),
    ];
    let exact = [
        (1.0 + (-pi).exp()) / 2.0,
        2.0 / 3.0,
        (10.0_f64).atan() / 10.0,
    ];
    let settings = AdaptiveSettings {
        abs_tol: 1e-11,
        rel_tol: 1e-10,
        max_subdivisions: 100_000,
    };

    let batch = adaptive_quadrature_batch_min(
        &intervals,
        settings,
        ComputeBackend::Serial,
        0,
        |i, x| match i {
            0 => (-x).exp() * x.sin(),
            1 => x.sqrt(),
            _ => 1.0 / (1.0 + 400.0 * (x - 0.5) * (x - 0.5)),
        },
    );

    let names = ["exp(-x) sin(x)", "sqrt(x)", "narrow peak"];
    for (i, s) in batch.solutions().iter().enumerate() {
        let err = (s.last_value() - exact[i]).abs();
        println!(
            "adaptive {:<15} value = {:.17}, error = {err:.6e}, \
             estimate = {:.6e}, evals = {}, status = {}",
            names[i],
            s.last_value(),
            s.error_estimate(),
            s.evaluations(),
            s.status().label()
        );
        assert!(s.evaluated(), "lane {i} status {:?}", s.status());
        assert!(err < 1e-9, "lane {i} error {err:.6e}");
        assert!(
            s.error_estimate() * 100.0 >= err || err == 0.0,
            "lane {i}: estimate {:.6e} badly understates error {err:.6e}",
            s.error_estimate()
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Determinism
// ═════════════════════════════════════════════════════════════════════════════

/// An ensemble whose lanes take wildly different numbers of steps, which is the
/// case that would expose any schedule dependence.
///
/// Half the lanes are benign (`k` near 1, a handful of steps), half are 60x
/// faster-decaying and take many more.
fn imbalanced_lanes(n: usize) -> Vec<OdeLane<Decay>> {
    let mut rng = Xorshift::new(0x5eed_1234);
    (0..n)
        .map(|i| {
            let k = if i % 2 == 0 {
                rng.next_in(0.5, 1.5)
            } else {
                rng.next_in(50.0, 70.0)
            };
            OdeLane::new(Decay { k }, vec![1.0], 0.0, 1.0, 0.1)
        })
        .collect()
}

/// Compare two ensembles bit for bit, on every field a caller can observe.
fn assert_ensembles_bitwise_equal(a: &OdeEnsemble, b: &OdeEnsemble, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: length");
    for (i, (x, y)) in a.lanes().iter().zip(b.lanes()).enumerate() {
        assert_eq!(x.status(), y.status(), "{what}: lane {i} status");
        assert_eq!(x.steps(), y.steps(), "{what}: lane {i} steps");
        assert_eq!(
            x.x_reached().to_bits(),
            y.x_reached().to_bits(),
            "{what}: lane {i} x_reached"
        );
        assert_eq!(
            x.dx_final().to_bits(),
            y.dx_final().to_bits(),
            "{what}: lane {i} dx_final"
        );
        assert_eq!(
            x.last_state().len(),
            y.last_state().len(),
            "{what}: lane {i} state length"
        );
        for (c, (p, q)) in x.last_state().iter().zip(y.last_state()).enumerate() {
            assert_eq!(
                p.to_bits(),
                q.to_bits(),
                "{what}: lane {i} component {c}: {p} vs {q}"
            );
        }
    }
}

/// Compare two quadrature batches bit for bit.
fn assert_batches_bitwise_equal(a: &QuadratureBatch, b: &QuadratureBatch, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: length");
    for (i, (x, y)) in a.solutions().iter().zip(b.solutions()).enumerate() {
        assert_eq!(x.status(), y.status(), "{what}: lane {i} status");
        assert_eq!(
            x.evaluations(),
            y.evaluations(),
            "{what}: lane {i} evaluations"
        );
        assert_eq!(
            x.last_value().to_bits(),
            y.last_value().to_bits(),
            "{what}: lane {i} value {} vs {}",
            x.last_value(),
            y.last_value()
        );
        assert_eq!(
            x.error_estimate().to_bits(),
            y.error_estimate().to_bits(),
            "{what}: lane {i} error estimate"
        );
    }
}

/// Serial and CpuMulti agree bit for bit on an imbalanced ensemble, with the
/// size floor forced to zero so the parallel path really runs.
///
/// **Methodology.** 512 lanes of `dy/dx = -k y`, half with `k` near 1 and half
/// with `k` near 60, so per-lane step counts differ by more than an order of
/// magnitude. All three steppers are compared.
///
/// **Pass criterion.** Every observable field of every lane is bit-identical.
///
/// **Result, measured 2026-08-13 (release, `--features parallel`):** identical
/// on all 512 lanes for all three steppers. With the feature off the parallel
/// request resolves to serial and the test is a tautology that still guards the
/// resolve path.
#[test]
fn ensemble_bitwise_identical_across_backends() {
    let lanes = imbalanced_lanes(512);
    for solver in [
        OdeSolver::euler(1, 1e-6, 1e-5),
        OdeSolver::rkf45(1, 1e-10, 1e-8),
        OdeSolver::rosenbrock23(1, 1e-10, 1e-8),
    ] {
        let name = solver.name();
        let serial =
            integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::Serial, 0);
        let multi =
            integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::CpuMulti, 0);
        assert_ensembles_bitwise_equal(&serial, &multi, name);
    }
}

/// The same ensemble at 1, 2, 4 and 8 worker threads is bit-identical to serial.
///
/// This is the claim that matters: an ODE ensemble has no cross-lane arithmetic
/// and each lane owns its own stepper clone, so unlike a reduction its result
/// cannot depend on the thread count. Without the `parallel` feature there is no
/// pool to build, so the test is compiled only with the feature on.
///
/// **Result, measured 2026-08-13 (release, `--features parallel`, 4 logical
/// cores):** identical at every one of the four thread counts, for `Rkf45` and
/// for `Rosenbrock23`.
#[cfg(feature = "parallel")]
#[test]
fn ensemble_bitwise_identical_across_thread_counts() {
    let lanes = imbalanced_lanes(512);
    for solver in [
        OdeSolver::rkf45(1, 1e-10, 1e-8),
        OdeSolver::rosenbrock23(1, 1e-10, 1e-8),
    ] {
        let name = solver.name();
        let serial =
            integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::Serial, 0);
        for threads in [1_usize, 2, 4, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("thread pool");
            pool.install(|| {
                let multi =
                    integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::CpuMulti, 0);
                assert_ensembles_bitwise_equal(
                    &serial,
                    &multi,
                    &format!("{name} @ {threads} threads"),
                );
            });
        }
    }
}

/// Both quadrature paths are bit-identical across backends and thread counts.
///
/// **Methodology.** 512 lanes of `exp(-a_i x) sin(b_i x)` over pseudorandom
/// intervals inside `[0, 4]`, evaluated with `G5` over 16 panels and, for the
/// adaptive path, with `abs_tol = 1e-11`. The adaptive lanes deliberately differ
/// in difficulty, so their evaluation counts differ by an order of magnitude.
///
/// **Pass criterion.** Every observable field bit-identical against serial, at
/// 1, 2, 4 and 8 workers.
///
/// **Result, measured 2026-08-13 (release, `--features parallel`, 4 logical
/// cores):** identical everywhere, for both the fixed and the adaptive path.
#[test]
fn quadrature_bitwise_identical_across_backends_and_threads() {
    let n = 512;
    let mut rng = Xorshift::new(0xfeed_beef);
    let mut intervals = Vec::with_capacity(n);
    let mut a = Vec::with_capacity(n);
    let mut b = Vec::with_capacity(n);
    for _ in 0..n {
        let lo = rng.next_in(0.0, 1.0);
        intervals.push(QuadratureInterval::new(lo, lo + rng.next_in(0.5, 3.0)));
        a.push(rng.next_in(0.1, 3.0));
        b.push(rng.next_in(1.0, 30.0));
    }
    let f = |i: usize, x: f64| (-a[i] * x).exp() * (b[i] * x).sin();
    let rule = QuadratureRule::GaussLegendre {
        order: GaussOrder::G5,
        panels: 16,
    };
    let settings = AdaptiveSettings {
        abs_tol: 1e-11,
        ..AdaptiveSettings::default()
    };

    let fixed_serial =
        quadrature_batch_min(&intervals, rule, ComputeBackend::Serial, 0, f);
    let fixed_multi =
        quadrature_batch_min(&intervals, rule, ComputeBackend::CpuMulti, 0, f);
    assert_batches_bitwise_equal(&fixed_serial, &fixed_multi, "fixed backends");

    let adaptive_serial =
        adaptive_quadrature_batch_min(&intervals, settings, ComputeBackend::Serial, 0, f);
    let adaptive_multi =
        adaptive_quadrature_batch_min(&intervals, settings, ComputeBackend::CpuMulti, 0, f);
    assert_batches_bitwise_equal(&adaptive_serial, &adaptive_multi, "adaptive backends");

    #[cfg(feature = "parallel")]
    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        pool.install(|| {
            let fixed =
                quadrature_batch_min(&intervals, rule, ComputeBackend::CpuMulti, 0, f);
            assert_batches_bitwise_equal(
                &fixed_serial,
                &fixed,
                &format!("fixed @ {threads} threads"),
            );
            let adaptive = adaptive_quadrature_batch_min(
                &intervals,
                settings,
                ComputeBackend::CpuMulti,
                0,
                f,
            );
            assert_batches_bitwise_equal(
                &adaptive_serial,
                &adaptive,
                &format!("adaptive @ {threads} threads"),
            );
        });
    }
}

/// Lane order is preserved by the parallel path — lane `i` of the result belongs
/// to input `i`, not to whichever thread finished first.
#[test]
fn lane_order_is_preserved() {
    let n = 256;
    let lanes: Vec<OdeLane<Decay>> = (0..n)
        .map(|i| OdeLane::new(Decay { k: 1.0 }, vec![i as f64], 0.0, 0.0, 0.1))
        .collect();
    let ensemble = integrate_ensemble_min(
        &lanes,
        |_| OdeSolver::rkf45(1, 1e-10, 1e-8),
        ComputeBackend::CpuMulti,
        0,
    );
    for (i, s) in ensemble.states().unwrap().iter().enumerate() {
        assert_eq!(s[0], i as f64, "lane {i} out of order");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Failure reporting
// ═════════════════════════════════════════════════════════════════════════════

/// A lane that runs out of sub-steps reports it, keeps its genuine partial
/// state, and is excluded from the all-or-nothing accessor.
#[test]
fn max_steps_exceeded_is_reported() {
    let lanes = vec![OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 100.0, 1e-6)];
    let mut solver = OdeSolver::rkf45(1, 1e-12, 1e-12);
    solver.config_mut().max_steps = 5;
    solver.config_mut().max_scale = 1.0; // stop the controller growing the step

    let ensemble = integrate_ensemble(&lanes, &solver, ComputeBackend::Serial);
    let l = &ensemble.lanes()[0];
    assert_eq!(l.status(), OdeLaneStatus::MaxStepsExceeded);
    assert!(l.state().is_none(), "a failed lane must not offer a state");
    assert!(l.last_state()[0].is_finite());
    assert!(l.x_reached() < 100.0);
    assert!(l.steps() > 0);

    let err = ensemble.states().expect_err("must refuse");
    assert_eq!(err.total, 1);
    assert_eq!(err.failure_count, 1);
    assert_eq!(err.first_index, 0);
    assert_eq!(err.first_status, OdeLaneStatus::MaxStepsExceeded);
}

/// Every way a lane can be invalid is rejected before integration, with a `NaN`
/// state rather than a silently untouched initial condition.
#[test]
fn invalid_lanes_are_reported() {
    let solver = OdeSolver::rkf45(1, 1e-10, 1e-8);
    let cases: Vec<(&str, OdeLane<Decay>)> = vec![
        (
            "wrong state length",
            OdeLane::new(Decay { k: 1.0 }, vec![1.0, 2.0], 0.0, 1.0, 0.1),
        ),
        (
            "non-finite x_end",
            OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, f64::INFINITY, 0.1),
        ),
        (
            "non-finite y0",
            OdeLane::new(Decay { k: 1.0 }, vec![f64::NAN], 0.0, 1.0, 0.1),
        ),
        (
            "zero dx0",
            OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 1.0, 0.0),
        ),
        (
            "negative dx0",
            OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 1.0, -0.1),
        ),
        (
            "reversed interval",
            OdeLane::new(Decay { k: 1.0 }, vec![1.0], 1.0, 0.0, 0.1),
        ),
    ];

    for (what, lane) in cases {
        let ensemble = integrate_ensemble(std::slice::from_ref(&lane), &solver, ComputeBackend::Serial);
        let l = &ensemble.lanes()[0];
        assert_eq!(l.status(), OdeLaneStatus::InvalidLane, "{what}");
        assert!(l.state().is_none(), "{what}");
        assert!(
            l.last_state().iter().all(|v| v.is_nan()),
            "{what}: state should be all NaN, got {:?}",
            l.last_state()
        );
        assert!(l.x_reached().is_nan(), "{what}");
        assert_eq!(l.steps(), 0, "{what}");
    }
}

/// A zero-length interval is a legitimate no-op, not a failure.
#[test]
fn zero_length_interval_completes_as_a_no_op() {
    let lanes = vec![OdeLane::new(Decay { k: 3.0 }, vec![7.5], 2.0, 2.0, 0.1)];
    let ensemble = integrate_ensemble(
        &lanes,
        &OdeSolver::rkf45(1, 1e-10, 1e-8),
        ComputeBackend::Serial,
    );
    let l = &ensemble.lanes()[0];
    assert_eq!(l.status(), OdeLaneStatus::Completed);
    assert_eq!(l.steps(), 0);
    assert_eq!(l.x_reached(), 2.0);
    assert_eq!(l.state().unwrap(), &[7.5]);
}

/// A model that blows up is reported as a failure, not as a completed lane
/// carrying an infinity.
#[test]
fn a_diverging_model_is_reported() {
    // y' = y^2, y(0) = 1 has a pole at x = 1; integrating to x = 2 cannot work.
    let lanes = vec![OdeLane::new(Blowup, vec![1.0], 0.0, 2.0, 0.01)];
    let ensemble = integrate_ensemble(
        &lanes,
        &OdeSolver::rkf45(1, 1e-10, 1e-8),
        ComputeBackend::Serial,
    );
    let l = &ensemble.lanes()[0];
    println!(
        "diverging model: status = {}, steps = {}, x_reached = {:.6}, y = {:?}",
        l.status().label(),
        l.steps(),
        l.x_reached(),
        l.last_state()
    );
    assert!(!l.completed(), "a lane through a pole must not complete");
    assert!(l.state().is_none());
    assert!(ensemble.states().is_err());
}

/// A partial failure inside a large ensemble is counted and located, and the
/// surviving lanes remain individually readable.
#[test]
fn partial_failure_in_a_large_ensemble_is_reported() {
    let n = 300;
    let lanes: Vec<OdeLane<Decay>> = (0..n)
        .map(|i| {
            if i % 100 == 7 {
                // Reversed interval: rejected before integration.
                OdeLane::new(Decay { k: 1.0 }, vec![1.0], 1.0, 0.0, 0.1)
            } else {
                OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 1.0, 0.1)
            }
        })
        .collect();

    let ensemble = integrate_ensemble(
        &lanes,
        &OdeSolver::rkf45(1, 1e-10, 1e-8),
        ComputeBackend::CpuMulti,
    );
    assert_eq!(ensemble.failure_count(), 3);
    assert_eq!(ensemble.first_failure().map(|(i, _)| i), Some(7));
    assert_eq!(
        ensemble.failures().iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![7, 107, 207]
    );
    let err = ensemble.states().expect_err("must refuse");
    assert_eq!(err.total, n);
    assert_eq!(err.failure_count, 3);
    assert_eq!(err.first_index, 7);
    // A surviving lane is still readable individually.
    assert!(ensemble.lanes()[0].state().is_some());
    assert!(!ensemble.all_completed());
}

/// An empty ensemble and an empty quadrature batch are vacuously successful.
#[test]
fn empty_batches_are_vacuously_ok() {
    let lanes: Vec<OdeLane<Decay>> = Vec::new();
    let ensemble = integrate_ensemble(
        &lanes,
        &OdeSolver::rkf45(1, 1e-10, 1e-8),
        ComputeBackend::CpuMulti,
    );
    assert!(ensemble.is_empty());
    assert!(ensemble.all_completed());
    assert_eq!(ensemble.states().unwrap().len(), 0);
    assert_eq!(ensemble.total_steps(), 0);
    assert_eq!(ensemble.max_steps_taken(), 0);

    let batch = quadrature_batch(
        &[],
        QuadratureRule::Simpson { panels: 4 },
        ComputeBackend::CpuMulti,
        |_, x| x,
    );
    assert!(batch.is_empty());
    assert!(batch.all_evaluated());
    assert_eq!(batch.values().unwrap().len(), 0);
    assert_eq!(batch.total_evaluations(), 0);
}

/// `integrate_ensemble_mixed` really reaches a different stepper per lane.
///
/// Three lanes of the same problem with three different steppers; the two
/// high-order lanes must agree with each other far more closely than either
/// agrees with the first-order one, which is what proves the selection is not
/// silently collapsing to one stepper.
#[test]
fn mixed_stepper_selection_reaches_each_lane() {
    let lanes: Vec<OdeLane<Decay>> = (0..3)
        .map(|_| OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 1.0, 0.1))
        .collect();
    let ensemble = integrate_ensemble_mixed(
        &lanes,
        |i| match i {
            0 => OdeSolver::euler(1, 1e-3, 1e-2),
            1 => OdeSolver::rkf45(1, 1e-10, 1e-8),
            _ => OdeSolver::rosenbrock23(1, 1e-10, 1e-8),
        },
        ComputeBackend::Serial,
    );
    let s = ensemble.states().expect("all complete");
    let exact = (-1.0_f64).exp();
    let e_euler = (s[0][0] - exact).abs();
    let e_rkf = (s[1][0] - exact).abs();
    let e_rb = (s[2][0] - exact).abs();
    println!("mixed selection: euler {e_euler:.6e}, rkf45 {e_rkf:.6e}, rosenbrock23 {e_rb:.6e}");
    assert!(e_euler > 1e-5, "euler lane looks too accurate: {e_euler:.6e}");
    assert!(e_rkf < 1e-8, "rkf45 lane: {e_rkf:.6e}");
    assert!(e_rb < 1e-8, "rosenbrock23 lane: {e_rb:.6e}");
}

/// A non-finite quadrature limit is reported, not integrated over.
#[test]
fn quadrature_invalid_interval_is_reported() {
    for iv in [
        QuadratureInterval::new(f64::NAN, 1.0),
        QuadratureInterval::new(0.0, f64::INFINITY),
        QuadratureInterval::new(f64::NEG_INFINITY, 0.0),
    ] {
        for batch in [
            quadrature_batch(
                &[iv],
                QuadratureRule::Simpson { panels: 4 },
                ComputeBackend::Serial,
                |_, x| x,
            ),
            adaptive_quadrature_batch(
                &[iv],
                AdaptiveSettings::default(),
                ComputeBackend::Serial,
                |_, x| x,
            ),
        ] {
            let s = batch.solutions()[0];
            assert_eq!(s.status(), QuadratureStatus::InvalidInterval, "{iv:?}");
            assert!(s.value().is_none());
            assert!(s.last_value().is_nan());
            assert!(batch.values().is_err());
        }
    }
}

/// A non-finite integrand sample is reported, not silently summed.
#[test]
fn quadrature_not_finite_is_reported() {
    let iv = [QuadratureInterval::new(0.0, 1.0)];
    let batch = quadrature_batch(
        &iv,
        QuadratureRule::Trapezoid { panels: 4 },
        ComputeBackend::Serial,
        |_, x| 1.0 / x, // f(0) = +inf
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), QuadratureStatus::NotFinite);
    assert!(s.value().is_none());
    assert!(s.last_value().is_nan());
    assert!(s.evaluations() > 0, "the evaluations it did make are reported");
}

/// An adaptive lane that exhausts its subdivision budget says so, and its best
/// estimate is excluded from the all-or-nothing accessor.
#[test]
fn adaptive_tolerance_not_met_is_reported() {
    // A very narrow peak with a tiny budget: 2 bisections cannot resolve it.
    let iv = [QuadratureInterval::new(0.0, 1.0)];
    let settings = AdaptiveSettings {
        abs_tol: 1e-14,
        rel_tol: 0.0,
        max_subdivisions: 2,
    };
    let batch = adaptive_quadrature_batch(&iv, settings, ComputeBackend::Serial, |_, x| {
        1.0 / (1.0 + 10_000.0 * (x - 0.5) * (x - 0.5))
    });
    let s = batch.solutions()[0];
    println!(
        "adaptive budget exhausted: status = {}, value = {:.12}, estimate = {:.6e}, evals = {}",
        s.status().label(),
        s.last_value(),
        s.error_estimate(),
        s.evaluations()
    );
    assert_eq!(s.status(), QuadratureStatus::ToleranceNotMet);
    assert!(s.value().is_none(), "must not offer an untrustworthy value");
    assert!(
        s.last_value().is_finite(),
        "but the best estimate is still readable"
    );
    let err = batch.values().expect_err("must refuse");
    assert_eq!(err.failure_count, 1);
    assert_eq!(err.first_status, QuadratureStatus::ToleranceNotMet);
}

/// Quadrature conventions: a zero-length interval integrates to exactly zero,
/// and a reversed interval negates.
#[test]
fn quadrature_interval_conventions() {
    let rule = QuadratureRule::GaussLegendre {
        order: GaussOrder::G4,
        panels: 2,
    };
    let f = |_: usize, x: f64| x * x * x + 2.0 * x;

    let zero = quadrature_batch(
        &[QuadratureInterval::new(1.25, 1.25)],
        rule,
        ComputeBackend::Serial,
        f,
    );
    assert_eq!(zero.values().unwrap()[0], 0.0);
    assert_eq!(zero.solutions()[0].evaluations(), 0);

    let forward = quadrature_batch(
        &[QuadratureInterval::new(0.0, 2.0)],
        rule,
        ComputeBackend::Serial,
        f,
    );
    let reversed = quadrature_batch(
        &[QuadratureInterval::new(2.0, 0.0)],
        rule,
        ComputeBackend::Serial,
        f,
    );
    let (a, b) = (forward.values().unwrap()[0], reversed.values().unwrap()[0]);
    // Closed form over [0, 2]: 2^4/4 + 2^2 = 8.
    assert!((a - 8.0).abs() < 1e-13, "forward {a}");
    assert_eq!(a.to_bits(), (-b).to_bits(), "reversed must negate exactly");

    // A zero panel count is treated as one, not as a division by zero.
    let clamped = quadrature_batch(
        &[QuadratureInterval::new(0.0, 2.0)],
        QuadratureRule::Trapezoid { panels: 0 },
        ComputeBackend::Serial,
        |_, _| 1.0,
    );
    assert_eq!(clamped.values().unwrap()[0], 2.0);
    assert_eq!(clamped.solutions()[0].evaluations(), 2);
}

/// A fixed rule offers no error estimate, and says so with `NaN` rather than a
/// falsely reassuring zero.
#[test]
fn a_fixed_rule_has_no_error_estimate() {
    let batch = quadrature_batch(
        &[QuadratureInterval::new(0.0, 1.0)],
        QuadratureRule::Simpson { panels: 4 },
        ComputeBackend::Serial,
        |_, x| x * x,
    );
    assert!(batch.solutions()[0].error_estimate().is_nan());
    assert!(batch.solutions()[0].value().is_some());
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Dispatch policy
// ═════════════════════════════════════════════════════════════════════════════

/// The size floors gate `CpuMulti` exactly as documented, and never promise a
/// backend that is not available.
#[test]
fn dispatch_respects_the_size_floors() {
    assert_eq!(
        ensemble_backend_for(ComputeBackend::CpuMulti, ODE_ENSEMBLE_MIN_LANES - 1),
        ComputeBackend::Serial
    );
    assert_eq!(
        ensemble_backend_for(ComputeBackend::CpuMulti, ODE_ENSEMBLE_MIN_LANES),
        if cfg!(feature = "parallel") {
            ComputeBackend::CpuMulti
        } else {
            ComputeBackend::Serial
        }
    );
    assert_eq!(
        quadrature_backend_for(ComputeBackend::CpuMulti, QUADRATURE_MIN_INTERVALS - 1),
        ComputeBackend::Serial
    );
    assert_eq!(
        quadrature_backend_for(ComputeBackend::CpuMulti, QUADRATURE_MIN_INTERVALS),
        if cfg!(feature = "parallel") {
            ComputeBackend::CpuMulti
        } else {
            ComputeBackend::Serial
        }
    );
    // A Gpu request never claims a GPU here — there is no GPU kernel.
    assert_ne!(
        ensemble_backend_for(ComputeBackend::Gpu, 1 << 20),
        ComputeBackend::Gpu
    );
    assert!(ensemble_backend_for(ComputeBackend::Gpu, 1 << 20).is_available());
    assert_ne!(
        quadrature_backend_for(ComputeBackend::Gpu, 1 << 20),
        ComputeBackend::Gpu
    );
    // Serial stays serial at any size.
    assert_eq!(
        ensemble_backend_for(ComputeBackend::Serial, 1 << 20),
        ComputeBackend::Serial
    );
    assert_eq!(
        quadrature_backend_for(ComputeBackend::Serial, 1 << 20),
        ComputeBackend::Serial
    );
}

/// Requesting a backend never changes the answer, only the wall clock — checked
/// through the *public* entry points at their real size floors.
#[test]
fn public_entry_points_agree_across_requested_backends() {
    let lanes = imbalanced_lanes(ODE_ENSEMBLE_MIN_LANES * 4);
    let solver = OdeSolver::rkf45(1, 1e-10, 1e-8);
    let serial = integrate_ensemble(&lanes, &solver, ComputeBackend::Serial);
    for backend in [ComputeBackend::CpuMulti, ComputeBackend::Gpu] {
        let other = integrate_ensemble(&lanes, &solver, backend);
        assert_ensembles_bitwise_equal(&serial, &other, &format!("{backend:?}"));
    }

    let n = QUADRATURE_MIN_INTERVALS * 2;
    let intervals: Vec<QuadratureInterval> = (0..n)
        .map(|i| QuadratureInterval::new(0.0, 1.0 + i as f64 / n as f64))
        .collect();
    let rule = QuadratureRule::GaussLegendre {
        order: GaussOrder::G5,
        panels: 8,
    };
    let f = |i: usize, x: f64| (-(i as f64 + 1.0) * x).exp();
    let q_serial = quadrature_batch(&intervals, rule, ComputeBackend::Serial, f);
    for backend in [ComputeBackend::CpuMulti, ComputeBackend::Gpu] {
        let other = quadrature_batch(&intervals, rule, backend, f);
        assert_batches_bitwise_equal(&q_serial, &other, &format!("{backend:?}"));
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. Measurement (ignored: too slow for the ordinary suite)
// ═════════════════════════════════════════════════════════════════════════════

/// Crossover benchmark for the ODE ensemble — the source of the table on
/// [`ODE_ENSEMBLE_MIN_LANES`].
///
/// `#[ignore]`d because it is a measurement, not a correctness check.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 ensemble_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn ensemble_crossover_benchmark() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!("ODE_ENSEMBLE_MIN_LANES = {ODE_ENSEMBLE_MIN_LANES}");
    println!(
        "{:>10} {:>14} {:>14} {:>9} {:>12}",
        "lanes", "serial [us]", "cpumulti [us]", "speedup", "steps/lane"
    );

    for n in [8_usize, 16, 32, 64, 128, 256, 1024, 4096, 16_384] {
        let lanes = imbalanced_lanes(n);
        let solver = OdeSolver::rkf45(1, 1e-10, 1e-8);
        let time = |backend: ComputeBackend| -> f64 {
            std::hint::black_box(integrate_ensemble_min(
                &lanes,
                |_| solver.clone(),
                backend,
                0,
            ));
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = integrate_ensemble_min(&lanes, |_| solver.clone(), backend, 0);
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            best
        };
        let reference = integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::Serial, 0);
        let s = time(ComputeBackend::Serial);
        let m = time(ComputeBackend::CpuMulti);
        println!(
            "{n:>10} {s:>14.2} {m:>14.2} {:>9.2} {:>12.1}",
            s / m,
            reference.total_steps() as f64 / n as f64
        );
    }
}

/// Crossover benchmark for the fixed-rule quadrature path — the source of the
/// table on [`QUADRATURE_MIN_INTERVALS`].
///
/// `#[ignore]`d.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 quadrature_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn quadrature_crossover_benchmark() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!("QUADRATURE_MIN_INTERVALS = {QUADRATURE_MIN_INTERVALS}");
    let rule = QuadratureRule::GaussLegendre {
        order: GaussOrder::G5,
        panels: 16,
    };
    println!(
        "rule = {} ({} evaluations per lane)",
        rule.label(),
        rule.evaluations()
    );
    println!(
        "{:>10} {:>14} {:>14} {:>9}",
        "intervals", "serial [us]", "cpumulti [us]", "speedup"
    );

    for n in [16_usize, 32, 64, 128, 256, 1024, 4096, 16_384] {
        let mut rng = Xorshift::new(0x1357_9bdf);
        let mut intervals = Vec::with_capacity(n);
        let mut a = Vec::with_capacity(n);
        let mut b = Vec::with_capacity(n);
        for _ in 0..n {
            let lo = rng.next_in(0.0, 1.0);
            intervals.push(QuadratureInterval::new(lo, lo + rng.next_in(0.5, 3.0)));
            a.push(rng.next_in(0.1, 3.0));
            b.push(rng.next_in(1.0, 30.0));
        }
        let f = |i: usize, x: f64| (-a[i] * x).exp() * (b[i] * x).sin();

        let time = |backend: ComputeBackend| -> f64 {
            std::hint::black_box(quadrature_batch_min(&intervals, rule, backend, 0, f));
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = quadrature_batch_min(&intervals, rule, backend, 0, f);
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            best
        };
        let s = time(ComputeBackend::Serial);
        let m = time(ComputeBackend::CpuMulti);
        println!("{n:>10} {s:>14.2} {m:>14.2} {:>9.2}", s / m);
    }
}

/// Thread-scaling measurement for the ODE ensemble on a fixed batch, with the
/// bitwise-identity claim re-asserted at each thread count.
///
/// `#[ignore]`d.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 ensemble_thread_scaling_benchmark
/// ```
#[cfg(feature = "parallel")]
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn ensemble_thread_scaling_benchmark() {
    use std::time::Instant;

    let n = 4096_usize;
    let lanes = imbalanced_lanes(n);
    let solver = OdeSolver::rkf45(1, 1e-10, 1e-8);
    let reference =
        integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::Serial, 0);

    let mut serial_us = f64::INFINITY;
    for _ in 0..7 {
        let t = Instant::now();
        let out = integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::Serial, 0);
        std::hint::black_box(&out);
        serial_us = serial_us.min(t.elapsed().as_secs_f64() * 1.0e6);
    }

    println!(
        "available_parallelism() = {}",
        std::thread::available_parallelism()
            .map(|c| c.get())
            .unwrap_or(1)
    );
    println!(
        "ensemble = {n} imbalanced lanes, Rkf45, best of 7; total steps {}, max lane {}",
        reference.total_steps(),
        reference.max_steps_taken()
    );
    println!(
        "{:>8} {:>14} {:>9} {:>10}",
        "threads", "time [us]", "speedup", "bitwise"
    );
    println!("{:>8} {serial_us:>14.2} {:>9.2} {:>10}", 0, 1.0, "reference");

    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        let (best, identical) = pool.install(|| {
            let first =
                integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::CpuMulti, 0);
            let identical = first == reference;
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out =
                    integrate_ensemble_min(&lanes, |_| solver.clone(), ComputeBackend::CpuMulti, 0);
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
