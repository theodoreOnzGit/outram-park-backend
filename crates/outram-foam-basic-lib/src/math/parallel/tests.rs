// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Tests for [`crate::math::parallel`] — the batched root finders.
//!
//! Four groups, in file order:
//!
//! 1. **Analytic verification** — every iterative method against roots that are
//!    known in closed form, so the oracle is exact rather than another
//!    implementation.
//! 2. **Determinism** — bitwise equality between `Serial` and `CpuMulti` at 1,
//!    2, 4 and 8 worker threads, on a batch built to have wildly uneven per-lane
//!    iteration counts.
//! 3. **Non-convergence** — every [`RootStatus`] failure path, and the
//!    all-or-nothing [`RootBatch::roots`] error.
//! 4. **Measurement** — `#[ignore]`d crossover benchmarks whose printed output
//!    is the source of every number in the constants' documentation.

use super::*;
use crate::polynomial::RootType;

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

/// The three real roots of a cubic, sorted, from a [`Roots<3>`].
fn real_roots_of(r: &Roots<3>) -> Vec<f64> {
    let mut v: Vec<f64> = (0..3)
        .filter(|&k| r.root_type(k) == RootType::Real)
        .map(|k| r.get(k))
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

/// `n` cubics built from known roots on `[-8, 8)`, plus those roots.
fn sample_cubics(n: usize, seed: u64) -> (Vec<CubicEqn>, Vec<[f64; 3]>) {
    let mut rng = Xorshift::new(seed);
    let mut eqns = Vec::with_capacity(n);
    let mut roots = Vec::with_capacity(n);
    for _ in 0..n {
        let mut r = [
            rng.next_in(-8.0, 8.0),
            rng.next_in(-8.0, 8.0),
            rng.next_in(-8.0, 8.0),
        ];
        r.sort_by(f64::total_cmp);
        // (x - r0)(x - r1)(x - r2) expanded.
        let b = -(r[0] + r[1] + r[2]);
        let c = r[0] * r[1] + r[0] * r[2] + r[1] * r[2];
        let d = -(r[0] * r[1] * r[2]);
        eqns.push(CubicEqn::new(1.0, b, c, d));
        roots.push(r);
    }
    (eqns, roots)
}

/// `n` quadratics built from known roots on `[-8, 8)`.
fn sample_quadratics(n: usize, seed: u64) -> Vec<QuadraticEqn> {
    let mut rng = Xorshift::new(seed);
    (0..n)
        .map(|_| {
            let r0 = rng.next_in(-8.0, 8.0);
            let r1 = rng.next_in(-8.0, 8.0);
            QuadraticEqn::new(1.0, -(r0 + r1), r0 * r1)
        })
        .collect()
}

// ── 1. Analytic verification ─────────────────────────────────────────────────

/// **Methodology.** `(x - 1)(x - 2)(x - 3)` has the exactly-known real roots 1,
/// 2 and 3. Three lanes each isolate one of them with a unit-wide bracket, and
/// both derivative-free methods are run on the same problems.
///
/// **Pass criterion.** `status == Converged` on every lane and
/// `|x_computed - x_analytic| <= 1e-12`.
///
/// **Results, measured 2026-08-12 (release):** worst absolute error
/// `0.000000e0` for **both** [`RootMethod::Brent`] and
/// [`RootMethod::Bisection`] — every lane recovered 1.0, 2.0 and 3.0 exactly.
/// Printed by this test under `--nocapture`.
///
/// **Interpretation.** These roots are exactly representable in `f64` and both
/// methods land on them bit for bit, so the iteration contributes no error of
/// its own here at all.
#[test]
fn bracketed_matches_analytic_cubic_roots() {
    let cubic = |x: f64| (x - 1.0) * (x - 2.0) * (x - 3.0);
    let problems = [
        RootProblem::new(0.5, 1.5),
        RootProblem::new(1.5, 2.5),
        RootProblem::new(2.5, 3.5),
    ];
    let analytic = [1.0, 2.0, 3.0];

    for method in [RootMethod::Bisection, RootMethod::Brent] {
        let batch = solve_bracketed_batch(
            &problems,
            method,
            RootSettings::default(),
            ComputeBackend::CpuMulti,
            |_, x| cubic(x),
        );
        let roots = batch.roots().unwrap_or_else(|e| panic!("{method:?}: {e}"));
        let mut worst = 0.0_f64;
        for (got, want) in roots.iter().zip(analytic) {
            worst = worst.max((got - want).abs());
        }
        println!("{method:?}: worst |x - x_analytic| = {worst:.6e}");
        assert!(
            worst <= 1e-12,
            "{method:?} worst error {worst:.6e} exceeds 1e-12"
        );
    }
}

/// **Methodology.** 64 lanes solving `x^2 - k = 0` for `k = 1 ..= 64` on the
/// bracket `[0, 65]`, against the analytic root `sqrt(k)`.
///
/// **Pass criterion.** All lanes converged, worst `|x - sqrt(k)| <= 1e-12`.
///
/// **Result, measured 2026-08-12 (release):** worst absolute error
/// `1.776357e-15`, which is 8 units in the last place at `k = 64`.
/// **Interpretation.** Brent lands at the rounding floor for every lane; the
/// batching is transparent to accuracy.
#[test]
fn brent_matches_analytic_sqrt() {
    let problems: Vec<RootProblem> = (0..64).map(|_| RootProblem::new(0.0, 65.0)).collect();
    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::CpuMulti,
        |i, x| x * x - (i as f64 + 1.0),
    );
    let roots = batch.roots().expect("all 64 lanes converge");
    let mut worst = 0.0_f64;
    for (i, got) in roots.iter().enumerate() {
        worst = worst.max((got - (i as f64 + 1.0).sqrt()).abs());
    }
    println!("brent sqrt: worst |x - sqrt(k)| = {worst:.6e}");
    assert!(worst <= 1e-12, "worst error {worst:.6e}");
}

/// **Methodology.** The same 64 `sqrt(k)` lanes and the three cubic roots, but
/// through [`solve_newton_batch`] with the exact analytic derivative.
///
/// **Pass criterion.** All lanes converged, worst error `<= 1e-12` in both
/// families.
///
/// **Results, measured 2026-08-12 (release):** worst `|x - sqrt(k)|`
/// `8.881784e-16`; worst cubic-root error `0.000000e0`.
/// **Interpretation.** Safeguarded Newton matches the derivative-free methods'
/// accuracy and in fact halves the worst `sqrt(k)` error; the safeguard costs
/// accuracy nothing.
#[test]
fn newton_matches_analytic_roots() {
    let problems: Vec<RootProblem> = (0..64).map(|_| RootProblem::new(0.0, 65.0)).collect();
    let batch = solve_newton_batch(
        &problems,
        RootSettings::default(),
        ComputeBackend::CpuMulti,
        |i, x| (x * x - (i as f64 + 1.0), 2.0 * x),
    );
    let roots = batch.roots().expect("all 64 sqrt lanes converge");
    let mut worst_sqrt = 0.0_f64;
    for (i, got) in roots.iter().enumerate() {
        worst_sqrt = worst_sqrt.max((got - (i as f64 + 1.0).sqrt()).abs());
    }

    let cubic_problems = [
        RootProblem::new(0.5, 1.5),
        RootProblem::new(1.5, 2.5),
        RootProblem::new(2.5, 3.5),
    ];
    let cubic_batch = solve_newton_batch(
        &cubic_problems,
        RootSettings::default(),
        ComputeBackend::CpuMulti,
        |_, x| {
            let v = (x - 1.0) * (x - 2.0) * (x - 3.0);
            let d = 3.0 * x * x - 12.0 * x + 11.0;
            (v, d)
        },
    );
    let cubic_roots = cubic_batch.roots().expect("all 3 cubic lanes converge");
    let mut worst_cubic = 0.0_f64;
    for (got, want) in cubic_roots.iter().zip([1.0, 2.0, 3.0]) {
        worst_cubic = worst_cubic.max((got - want).abs());
    }

    println!(
        "newton: worst |x - sqrt(k)| = {worst_sqrt:.6e}, worst cubic error = {worst_cubic:.6e}"
    );
    assert!(worst_sqrt <= 1e-12, "sqrt worst {worst_sqrt:.6e}");
    assert!(worst_cubic <= 1e-12, "cubic worst {worst_cubic:.6e}");
}

/// **Methodology.** The 64 `sqrt(k)` lanes solved by both safeguarded Newton and
/// Brent under identical settings; mean iterations per lane compared.
///
/// **Pass criterion.** Both converge everywhere, and Newton's mean iteration
/// count is strictly lower than Brent's — the reason to pay for a derivative.
///
/// **Result, measured 2026-08-12 (release):** mean iterations `7.73` (Newton)
/// against `13.14` (Brent). **Interpretation.** 59% of Brent's cost on a smooth
/// residual, consistent with quadratic versus superlinear convergence. Both
/// counts include the safeguard's bisection steps.
#[test]
fn newton_beats_brent_on_iterations() {
    let problems: Vec<RootProblem> = (0..64).map(|_| RootProblem::new(0.0, 65.0)).collect();

    let newton = solve_newton_batch(
        &problems,
        RootSettings::default(),
        ComputeBackend::Serial,
        |i, x| (x * x - (i as f64 + 1.0), 2.0 * x),
    );
    let brent = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::Serial,
        |i, x| x * x - (i as f64 + 1.0),
    );

    let mean = |b: &RootBatch| -> f64 {
        b.solutions()
            .iter()
            .map(|s| s.iterations() as f64)
            .sum::<f64>()
            / b.len() as f64
    };
    let (mn, mb) = (mean(&newton), mean(&brent));
    println!("mean iterations: newton {mn:.2}, brent {mb:.2}");
    assert!(newton.all_converged() && brent.all_converged());
    assert!(mn < mb, "newton {mn:.2} should beat brent {mb:.2}");
}

/// **Methodology.** 4 096 cubics built by expanding `(x - r1)(x - r2)(x - r3)`
/// with pseudorandom real roots on `[-8, 8)` (fixed-seed xorshift64\*), so the
/// analytic answer is known exactly. Two checks: every `Real`-tagged root's
/// residual, and its distance to the nearest constructed root. Plus bitwise
/// equality against the per-equation scalar [`CubicEqn::roots`].
///
/// **Pass criteria.** Residual `<= 1e-6`, distance to the nearest constructed
/// root `<= 1e-6`, and bit-for-bit equality with the scalar solver.
///
/// **Results, measured 2026-08-12 (release):** worst residual `3.979039e-13`,
/// worst root distance `2.079981e-10` over 12 288 real roots, bitwise equality
/// on every lane. **Interpretation.** Both figures are the scalar Cardano
/// solver's own conditioning — the bitwise half of the test proves the batch
/// contributes exactly zero additional error. The root displacement exceeding
/// the residual by three orders is the signature of the near-coincident root
/// triples that random construction occasionally produces, where the cubic is
/// locally flat.
#[test]
fn cubic_batch_matches_analytic_construction() {
    let (eqns, wanted) = sample_cubics(4096, 0x5eed_1234);
    let batch = cubic_roots_batch(&eqns, ComputeBackend::CpuMulti);
    let scalar: Vec<Roots<3>> = eqns.iter().map(CubicEqn::roots).collect();

    let mut worst_residual = 0.0_f64;
    let mut worst_distance = 0.0_f64;
    let mut real_count = 0_usize;

    for (lane, (r, eq)) in batch.iter().zip(eqns.iter()).enumerate() {
        // Bitwise equality with the scalar solver, values and tags.
        for k in 0..3 {
            assert_eq!(
                r.get(k).to_bits(),
                scalar[lane].get(k).to_bits(),
                "lane {lane} slot {k} differs from the scalar solver"
            );
            assert_eq!(r.root_type(k), scalar[lane].root_type(k));
        }
        for x in real_roots_of(r) {
            real_count += 1;
            worst_residual = worst_residual.max(eq.value(x).abs());
            let d = wanted[lane]
                .iter()
                .map(|w| (x - w).abs())
                .fold(f64::INFINITY, f64::min);
            worst_distance = worst_distance.max(d);
        }
    }

    println!(
        "cubic batch: {real_count} real roots, worst residual {worst_residual:.6e}, \
         worst distance to constructed root {worst_distance:.6e}"
    );
    assert!(worst_residual <= 1e-6, "residual {worst_residual:.6e}");
    assert!(worst_distance <= 1e-6, "distance {worst_distance:.6e}");
}

/// **Methodology.** 4 096 quadratics built from known real roots; every
/// `Real`-tagged root substituted back, and every returned bit compared against
/// the per-equation scalar [`QuadraticEqn::roots`].
///
/// **Pass criteria.** Residual `<= 1e-9`, bitwise equality with the scalar
/// solver.
///
/// **Result, measured 2026-08-12 (release):** worst residual `7.105427e-15` over
/// 8 192 real roots; bitwise equality held everywhere.
#[test]
fn quadratic_batch_residuals_are_analytic() {
    let eqns = sample_quadratics(4096, 0x1357_9bdf);
    let batch = quadratic_roots_batch(&eqns, ComputeBackend::CpuMulti);
    let scalar: Vec<Roots<2>> = eqns.iter().map(QuadraticEqn::roots).collect();

    let mut worst = 0.0_f64;
    let mut count = 0_usize;
    for (lane, (r, eq)) in batch.iter().zip(eqns.iter()).enumerate() {
        for k in 0..2 {
            assert_eq!(r.get(k).to_bits(), scalar[lane].get(k).to_bits());
            assert_eq!(r.root_type(k), scalar[lane].root_type(k));
            if r.root_type(k) == RootType::Real {
                count += 1;
                worst = worst.max(eq.value(r.get(k)).abs());
            }
        }
    }
    println!("quadratic batch: {count} real roots, worst residual {worst:.6e}");
    assert!(worst <= 1e-9, "residual {worst:.6e}");
}

/// Linear lanes are exact and match the scalar solver bit for bit, including the
/// degenerate `a ≈ 0` case that must be tagged `Nan` rather than returned as a
/// number.
#[test]
fn linear_batch_matches_scalar_including_degenerate() {
    let eqns = [
        LinearEqn::new(2.0, -4.0),
        LinearEqn::new(3.0, 9.0),
        LinearEqn::new(0.0, 1.0),
    ];
    let batch = linear_roots_batch(&eqns, ComputeBackend::CpuMulti);
    for (lane, eq) in eqns.iter().enumerate() {
        let s = eq.roots();
        assert_eq!(batch[lane].get(0).to_bits(), s.get(0).to_bits());
        assert_eq!(batch[lane].root_type(0), s.root_type(0));
    }
    assert_eq!(batch[0].get(0), 2.0);
    assert_eq!(batch[1].get(0), -3.0);
    assert_eq!(batch[2].root_type(0), RootType::Nan);
}

// ── 2. Determinism ───────────────────────────────────────────────────────────

/// A batch whose lanes take wildly different numbers of iterations, which is the
/// case that would expose any schedule dependence.
///
/// Half the lanes get a bracket that already nearly pins the root (a few
/// iterations); half get `[0, 1e6]` (many). Returns `(problems, targets)`.
fn imbalanced_problems(n: usize) -> (Vec<RootProblem>, Vec<f64>) {
    let mut rng = Xorshift::new(0xabcd_0f0f);
    let mut problems = Vec::with_capacity(n);
    let mut targets = Vec::with_capacity(n);
    for i in 0..n {
        let k = rng.next_in(1.0, 3.0);
        let root = k.sqrt();
        if i % 2 == 0 {
            problems.push(RootProblem::new(root - 1e-6, root + 1e-6));
        } else {
            problems.push(RootProblem::new(0.0, 1.0e6));
        }
        targets.push(k);
    }
    (problems, targets)
}

/// Compare two batches bit for bit, on every field a caller can observe.
fn assert_bitwise_equal(a: &RootBatch, b: &RootBatch, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: length");
    for (i, (x, y)) in a.solutions().iter().zip(b.solutions()).enumerate() {
        assert_eq!(x.status(), y.status(), "{what}: lane {i} status");
        assert_eq!(
            x.last_iterate().to_bits(),
            y.last_iterate().to_bits(),
            "{what}: lane {i} iterate {} vs {}",
            x.last_iterate(),
            y.last_iterate()
        );
        assert_eq!(
            x.residual().to_bits(),
            y.residual().to_bits(),
            "{what}: lane {i} residual"
        );
        assert_eq!(
            x.bracket_width().to_bits(),
            y.bracket_width().to_bits(),
            "{what}: lane {i} bracket width"
        );
        assert_eq!(
            x.iterations(),
            y.iterations(),
            "{what}: lane {i} iterations"
        );
    }
}

/// Serial and CpuMulti agree bit for bit on an imbalanced batch, with the size
/// floor forced to zero so the parallel path really runs.
///
/// **Methodology.** 2 048 lanes of `x^2 - k = 0`, half with a 2e-6-wide bracket
/// and half with `[0, 1e6]`, so per-lane iteration counts differ by an order of
/// magnitude. Both methods and the Newton solver are compared.
///
/// **Pass criterion.** Every observable field of every lane is bit-identical.
///
/// **Result, measured 2026-08-12 (release, `--features parallel`):** identical
/// on all 2 048 lanes for all three solvers. With the feature off the parallel
/// request resolves to serial and the test is a tautology that still guards the
/// resolve path.
#[test]
fn bitwise_identical_across_backends() {
    let (problems, targets) = imbalanced_problems(2048);
    let settings = RootSettings::default();

    for method in [RootMethod::Bisection, RootMethod::Brent] {
        let serial = solve_bracketed_batch_min(
            &problems,
            method,
            settings,
            ComputeBackend::Serial,
            0,
            |i, x| x * x - targets[i],
        );
        let multi = solve_bracketed_batch_min(
            &problems,
            method,
            settings,
            ComputeBackend::CpuMulti,
            0,
            |i, x| x * x - targets[i],
        );
        assert_bitwise_equal(&serial, &multi, &format!("{method:?}"));
    }

    let serial = solve_newton_batch_min(&problems, settings, ComputeBackend::Serial, 0, |i, x| {
        (x * x - targets[i], 2.0 * x)
    });
    let multi = solve_newton_batch_min(&problems, settings, ComputeBackend::CpuMulti, 0, |i, x| {
        (x * x - targets[i], 2.0 * x)
    });
    assert_bitwise_equal(&serial, &multi, "newton");
}

/// The same batch at 1, 2, 4 and 8 worker threads is bit-identical to serial.
///
/// This is the claim that matters: a root batch has no cross-lane arithmetic, so
/// unlike a reduction its result cannot depend on the thread count. Without the
/// `parallel` feature there is no pool to build and the test degenerates to the
/// serial comparison above, so it is compiled only with the feature on.
///
/// **Result, measured 2026-08-12 (release, `--features parallel`, 4 logical
/// cores):** identical at every one of the four thread counts, for Brent and for
/// safeguarded Newton.
#[cfg(feature = "parallel")]
#[test]
fn bitwise_identical_across_thread_counts() {
    let (problems, targets) = imbalanced_problems(2048);
    let settings = RootSettings::default();

    let serial = solve_bracketed_batch_min(
        &problems,
        RootMethod::Brent,
        settings,
        ComputeBackend::Serial,
        0,
        |i, x| x * x - targets[i],
    );
    let serial_newton =
        solve_newton_batch_min(&problems, settings, ComputeBackend::Serial, 0, |i, x| {
            (x * x - targets[i], 2.0 * x)
        });

    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        pool.install(|| {
            let multi = solve_bracketed_batch_min(
                &problems,
                RootMethod::Brent,
                settings,
                ComputeBackend::CpuMulti,
                0,
                |i, x| x * x - targets[i],
            );
            assert_bitwise_equal(&serial, &multi, &format!("brent @ {threads} threads"));

            let multi_newton =
                solve_newton_batch_min(&problems, settings, ComputeBackend::CpuMulti, 0, |i, x| {
                    (x * x - targets[i], 2.0 * x)
                });
            assert_bitwise_equal(
                &serial_newton,
                &multi_newton,
                &format!("newton @ {threads} threads"),
            );
        });
    }
}

/// The closed-form polynomial kernels are bit-identical across backends and
/// thread counts too.
///
/// **Result, measured 2026-08-12 (release, `--features parallel`):** identical
/// on all 4 096 cubics at 1, 2, 4 and 8 workers.
#[test]
fn poly_batch_bitwise_identical_across_backends() {
    let (eqns, _) = sample_cubics(4096, 0x9999_1111);
    let serial = cubic_roots_batch_min(&eqns, ComputeBackend::Serial, 0);

    let check = |multi: &[Roots<3>], what: &str| {
        for (lane, (a, b)) in serial.iter().zip(multi.iter()).enumerate() {
            for k in 0..3 {
                assert_eq!(
                    a.get(k).to_bits(),
                    b.get(k).to_bits(),
                    "{what}: lane {lane} slot {k}"
                );
                assert_eq!(
                    a.root_type(k),
                    b.root_type(k),
                    "{what}: lane {lane} tag {k}"
                );
            }
        }
    };

    check(
        &cubic_roots_batch_min(&eqns, ComputeBackend::CpuMulti, 0),
        "ambient pool",
    );

    #[cfg(feature = "parallel")]
    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        pool.install(|| {
            check(
                &cubic_roots_batch_min(&eqns, ComputeBackend::CpuMulti, 0),
                &format!("{threads} threads"),
            );
        });
    }
}

/// Lane order is preserved by the parallel path — lane `i` of the result is the
/// answer to `problems[i]`, not to some other lane.
#[test]
fn lane_order_is_preserved() {
    let n = 512;
    let problems: Vec<RootProblem> = (0..n).map(|_| RootProblem::new(0.0, 1024.0)).collect();
    let batch = solve_bracketed_batch_min(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::CpuMulti,
        0,
        |i, x| x * x - (i as f64 + 1.0),
    );
    let roots = batch.roots().expect("all lanes converge");
    for (i, r) in roots.iter().enumerate() {
        assert!(
            (r - (i as f64 + 1.0).sqrt()).abs() < 1e-9,
            "lane {i} got {r}"
        );
    }
}

// ── 3. Non-convergence reporting ─────────────────────────────────────────────

/// A bracket whose ends share a sign is reported, not guessed at — and the
/// reported iterate is `NaN`, never a clamped bracket endpoint.
#[test]
fn not_bracketed_is_reported_and_not_clamped() {
    let problems = [RootProblem::new(3.0, 4.0)];
    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::Serial,
        |_, x| x * x - 2.0,
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), RootStatus::NotBracketed);
    assert!(s.root().is_none());
    assert!(
        s.last_iterate().is_nan(),
        "a non-bracketed lane must not report an endpoint as a root; got {}",
        s.last_iterate()
    );
    assert_ne!(s.last_iterate().to_bits(), 3.0_f64.to_bits());
    assert_ne!(s.last_iterate().to_bits(), 4.0_f64.to_bits());
}

/// Running out of iterations reports [`RootStatus::MaxIterations`], keeps the
/// best iterate available for diagnosis, and still refuses to call it a root.
#[test]
fn max_iterations_is_reported() {
    let settings = RootSettings {
        max_iterations: 3,
        ..RootSettings::default()
    };
    let problems = [RootProblem::new(0.0, 1.0e12)];
    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Bisection,
        settings,
        ComputeBackend::Serial,
        |_, x| x - 1.0,
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), RootStatus::MaxIterations);
    assert_eq!(s.iterations(), 3);
    assert!(s.root().is_none());
    assert!(s.last_iterate().is_finite(), "diagnostic iterate is kept");
}

/// A non-finite bracket end is rejected before any evaluation.
#[test]
fn invalid_bracket_is_reported() {
    let problems = [
        RootProblem::new(f64::NAN, 1.0),
        RootProblem::new(0.0, f64::INFINITY),
    ];
    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::Serial,
        |_, _| panic!("must not be evaluated on an invalid bracket"),
    );
    for s in batch.solutions() {
        assert_eq!(s.status(), RootStatus::InvalidBracket);
        assert!(s.last_iterate().is_nan());
    }
}

/// A residual that evaluates to `NaN` stops its lane instead of propagating.
#[test]
fn not_finite_residual_is_reported() {
    let problems = [RootProblem::new(-1.0, 1.0)];
    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::Serial,
        // NaN at the left end.
        |_, x| if x < -0.5 { f64::NAN } else { x },
    );
    assert_eq!(batch.solutions()[0].status(), RootStatus::NotFinite);
    assert!(batch.solutions()[0].root().is_none());
}

/// A mixed batch — 3 failures in 10 000 lanes — is reported by count and by
/// index, and [`RootBatch::roots`] refuses to return an array.
///
/// This is the scenario the module documentation calls out: a plausible-looking
/// `Vec<f64>` must not come back from a batch that contained failures.
#[test]
fn partial_failure_in_a_large_batch_is_reported() {
    let n = 10_000;
    let mut problems: Vec<RootProblem> = (0..n).map(|_| RootProblem::new(0.0, 4.0)).collect();
    // Three lanes get a bracket that does not straddle sqrt(2).
    for &bad in &[17_usize, 4_242, 9_999] {
        problems[bad] = RootProblem::new(3.0, 4.0);
    }

    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::CpuMulti,
        |_, x| x * x - 2.0,
    );

    assert!(!batch.all_converged());
    assert_eq!(batch.failure_count(), 3);
    assert_eq!(
        batch.failures().iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![17, 4_242, 9_999]
    );

    let err = batch.roots().expect_err("three lanes failed");
    assert_eq!(err.total, n);
    assert_eq!(err.failure_count, 3);
    assert_eq!(err.first_index, 17);
    assert_eq!(err.first_status, RootStatus::NotBracketed);
    println!("failure report: {err}");

    // The 9 997 good lanes remain individually readable.
    assert_eq!(
        batch
            .solutions()
            .iter()
            .filter(|s| s.root().is_some())
            .count(),
        n - 3
    );
}

/// A root sitting exactly on a bracket endpoint is a root, reported with zero
/// iterations rather than as a degenerate bracket.
#[test]
fn root_on_endpoint_converges_immediately() {
    let problems = [RootProblem::new(2.0, 5.0), RootProblem::new(-5.0, 2.0)];
    let batch = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::Serial,
        |_, x| x - 2.0,
    );
    for s in batch.solutions() {
        assert_eq!(s.status(), RootStatus::Converged);
        assert_eq!(s.root(), Some(2.0));
        assert_eq!(s.iterations(), 0);
    }
}

/// An empty batch is empty, converged, and never calls the residual.
#[test]
fn empty_batch_is_vacuously_converged() {
    let batch = solve_bracketed_batch(
        &[],
        RootMethod::Brent,
        RootSettings::default(),
        ComputeBackend::CpuMulti,
        |_, _: f64| panic!("must not be called"),
    );
    assert!(batch.is_empty());
    assert!(batch.all_converged());
    assert_eq!(batch.roots().expect("vacuously ok"), Vec::<f64>::new());
    assert!(cubic_roots_batch(&[], ComputeBackend::CpuMulti).is_empty());
}

/// Newton survives a zero derivative by bisecting that iteration rather than
/// diverging: `f(x) = x^3` has `f'(0) = 0` exactly at the root.
#[test]
fn newton_survives_zero_derivative() {
    let problems = [RootProblem::with_guess(-1.0, 2.0, 0.0)];
    let batch = solve_newton_batch(
        &problems,
        RootSettings {
            x_tol_abs: 1e-8,
            ..RootSettings::default()
        },
        ComputeBackend::Serial,
        |_, x| (x * x * x, 3.0 * x * x),
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), RootStatus::Converged, "{s:?}");
    assert!(s.root().expect("converged").abs() < 1e-6, "{s:?}");
}

/// A Newton guess outside the bracket, or non-finite, is replaced by the
/// midpoint rather than rejected — a bad guess is a performance problem, not a
/// correctness one.
#[test]
fn newton_ignores_an_out_of_bracket_guess() {
    let settings = RootSettings::default();
    let f = |_: usize, x: f64| (x * x - 2.0, 2.0 * x);

    let good = solve_newton_batch(
        &[RootProblem::new(0.0, 4.0)],
        settings,
        ComputeBackend::Serial,
        f,
    );
    for guess in [-1.0e9, 1.0e9, f64::NAN, f64::INFINITY] {
        let batch = solve_newton_batch(
            &[RootProblem::with_guess(0.0, 4.0, guess)],
            settings,
            ComputeBackend::Serial,
            f,
        );
        assert_bitwise_equal(&good, &batch, &format!("guess {guess}"));
    }
}

/// Bisection and Brent find the same root to tolerance on a residual with a
/// kink, where the interpolation has nothing smooth to work with.
#[test]
fn both_methods_agree_on_a_kinked_residual() {
    // |x - 1.25| - 0.5 has roots at 0.75 and 1.75; bracket isolates 1.75.
    let f = |_: usize, x: f64| (x - 1.25_f64).abs() - 0.5;
    let problems = [RootProblem::new(1.3, 3.0)];
    let settings = RootSettings::default();

    let a = solve_bracketed_batch(
        &problems,
        RootMethod::Bisection,
        settings,
        ComputeBackend::Serial,
        f,
    );
    let b = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        settings,
        ComputeBackend::Serial,
        f,
    );
    let (ra, rb) = (
        a.roots().expect("bisection converges")[0],
        b.roots().expect("brent converges")[0],
    );
    assert!((ra - 1.75).abs() < 1e-9, "bisection got {ra}");
    assert!((rb - 1.75).abs() < 1e-9, "brent got {rb}");
}

// ── Dispatch policy ──────────────────────────────────────────────────────────

/// The size floors gate `CpuMulti` exactly as documented, and never promise a
/// backend that is not available.
#[test]
fn dispatch_respects_the_size_floors() {
    assert_eq!(
        root_batch_backend_for(ComputeBackend::CpuMulti, ROOT_BATCH_MIN_PROBLEMS - 1),
        ComputeBackend::Serial
    );
    assert_eq!(
        root_batch_backend_for(ComputeBackend::CpuMulti, ROOT_BATCH_MIN_PROBLEMS),
        if cfg!(feature = "parallel") {
            ComputeBackend::CpuMulti
        } else {
            ComputeBackend::Serial
        }
    );
    assert_eq!(
        poly_roots_backend_for(ComputeBackend::CpuMulti, POLY_ROOTS_MIN_EQUATIONS - 1),
        ComputeBackend::Serial
    );
    // A Gpu request never claims a GPU here — there is no GPU kernel yet.
    assert_ne!(
        root_batch_backend_for(ComputeBackend::Gpu, 1 << 20),
        ComputeBackend::Gpu
    );
    assert!(root_batch_backend_for(ComputeBackend::Gpu, 1 << 20).is_available());
    // Serial stays serial at any size.
    assert_eq!(
        root_batch_backend_for(ComputeBackend::Serial, 1 << 20),
        ComputeBackend::Serial
    );
}

/// Requesting a backend never changes the answer, only the wall clock — checked
/// through the *public* entry points at their real size floors.
#[test]
fn public_entry_points_agree_across_requested_backends() {
    let n = ROOT_BATCH_MIN_PROBLEMS * 2;
    let problems: Vec<RootProblem> = (0..n).map(|_| RootProblem::new(0.0, 1.0e4)).collect();
    let f = |i: usize, x: f64| x * x - (i as f64 + 1.0);

    let settings = RootSettings::default();
    let serial = solve_bracketed_batch(
        &problems,
        RootMethod::Brent,
        settings,
        ComputeBackend::Serial,
        f,
    );
    for backend in [ComputeBackend::CpuMulti, ComputeBackend::Gpu] {
        let other = solve_bracketed_batch(&problems, RootMethod::Brent, settings, backend, f);
        assert_bitwise_equal(&serial, &other, &format!("{backend:?}"));
    }
}

// ── 4. Measurement (ignored: too slow for the ordinary suite) ────────────────

/// Crossover benchmark for the **iterative** solvers — the source of the table
/// on [`ROOT_BATCH_MIN_PROBLEMS`].
///
/// `#[ignore]`d because it is a measurement, not a correctness check.
/// **Measured wall clock for the whole test: 0.90 s** on the development machine
/// (4 logical cores, release, `--features parallel`).
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 root_batch_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check; ~0.9 s wall clock. Run with --ignored --nocapture"]
fn root_batch_crossover_benchmark() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!("ROOT_BATCH_MIN_PROBLEMS = {ROOT_BATCH_MIN_PROBLEMS}");
    println!(
        "{:>10} {:>14} {:>14} {:>9} {:>15} {:>15} {:>9}",
        "problems",
        "cheap ser[us]",
        "cheap mul[us]",
        "speedup",
        "costly ser[us]",
        "costly mul[us]",
        "speedup"
    );

    let settings = RootSettings::default();

    for n in [16_usize, 32, 64, 128, 256, 512, 1024, 4096, 16_384, 65_536] {
        let mut rng = Xorshift::new(0x1234_5678);
        let targets: Vec<f64> = (0..n).map(|_| rng.next_in(1.0, 3.0)).collect();
        let problems: Vec<RootProblem> = (0..n).map(|_| RootProblem::new(0.0, 4.0)).collect();

        // Cheap residual: two flops.
        let cheap = |i: usize, x: f64| x * x - targets[i];
        // Costly residual: a transcendental chain standing in for a JANAF
        // enthalpy evaluation. Same root, so the iteration counts are
        // comparable.
        let costly = |i: usize, x: f64| {
            let s = (1.0 + x * x).ln().exp().sqrt();
            s * s - 1.0 - targets[i]
        };

        let time = |backend: ComputeBackend, costly_mode: bool| -> f64 {
            let run = || {
                if costly_mode {
                    solve_bracketed_batch_min(
                        &problems,
                        RootMethod::Brent,
                        settings,
                        backend,
                        0,
                        costly,
                    )
                } else {
                    solve_bracketed_batch_min(
                        &problems,
                        RootMethod::Brent,
                        settings,
                        backend,
                        0,
                        cheap,
                    )
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
}

/// Crossover benchmark for the **closed-form polynomial** kernels — the source
/// of the table on [`POLY_ROOTS_MIN_EQUATIONS`].
///
/// `#[ignore]`d. **Measured wall clock for the whole test: 0.67 s** on the
/// development machine (4 logical cores, release, `--features parallel`).
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 poly_batch_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check; ~0.7 s wall clock. Run with --ignored --nocapture"]
fn poly_batch_crossover_benchmark() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!("POLY_ROOTS_MIN_EQUATIONS = {POLY_ROOTS_MIN_EQUATIONS}, POLY_BLOCK = {POLY_BLOCK}");
    println!(
        "{:>10} {:>14} {:>14} {:>9}",
        "cubics", "serial [us]", "cpumulti [us]", "speedup"
    );

    for n in [256_usize, 512, 1024, 2048, 4096, 16_384, 65_536, 262_144] {
        let (eqns, _) = sample_cubics(n, 0x7777_3333);
        let time = |backend: ComputeBackend| -> f64 {
            std::hint::black_box(cubic_roots_batch_min(&eqns, backend, 0));
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = cubic_roots_batch_min(&eqns, backend, 0);
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

/// Thread-scaling measurement for the iterative solvers on a fixed batch, with
/// the bitwise-identity claim re-asserted at each thread count.
///
/// `#[ignore]`d. **Measured wall clock for the whole test: 0.53 s** on the
/// development machine (4 logical cores, release, `--features parallel`).
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 root_batch_thread_scaling_benchmark
/// ```
#[cfg(feature = "parallel")]
#[test]
#[ignore = "measurement, not a correctness check; ~0.5 s wall clock. Run with --ignored --nocapture"]
fn root_batch_thread_scaling_benchmark() {
    use std::time::Instant;

    let n = 65_536_usize;
    let (problems, targets) = imbalanced_problems(n);
    let settings = RootSettings::default();
    let f = |i: usize, x: f64| x * x - targets[i];

    let reference = solve_bracketed_batch_min(
        &problems,
        RootMethod::Brent,
        settings,
        ComputeBackend::Serial,
        0,
        f,
    );

    let t = Instant::now();
    std::hint::black_box(solve_bracketed_batch_min(
        &problems,
        RootMethod::Brent,
        settings,
        ComputeBackend::Serial,
        0,
        f,
    ));
    let mut serial_us = t.elapsed().as_secs_f64() * 1.0e6;
    for _ in 0..6 {
        let t = Instant::now();
        let out = solve_bracketed_batch_min(
            &problems,
            RootMethod::Brent,
            settings,
            ComputeBackend::Serial,
            0,
            f,
        );
        std::hint::black_box(&out);
        serial_us = serial_us.min(t.elapsed().as_secs_f64() * 1.0e6);
    }

    println!(
        "available_parallelism() = {}",
        std::thread::available_parallelism()
            .map(|c| c.get())
            .unwrap_or(1)
    );
    println!("batch = {n} imbalanced lanes, Brent, best of 7");
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
            let first = solve_bracketed_batch_min(
                &problems,
                RootMethod::Brent,
                settings,
                ComputeBackend::CpuMulti,
                0,
                f,
            );
            let identical = first == reference;
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = solve_bracketed_batch_min(
                    &problems,
                    RootMethod::Brent,
                    settings,
                    ComputeBackend::CpuMulti,
                    0,
                    f,
                );
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
