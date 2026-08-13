// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Tests for [`crate::math::minimise`] — the batched golden-section search.
//!
//! Five groups, in file order:
//!
//! 1. **Analytic verification** — against minima known in closed form, so the
//!    oracle is exact rather than another implementation. Includes a
//!    deliberately flat minimum, whose job is to *expose* the `sqrt(eps)`
//!    accuracy limit rather than hide it, and the theoretical contraction rate.
//! 2. **The unimodality precondition** — the multimodal and monotone cases,
//!    written down as tests so the documented failure mode is a measured fact
//!    rather than a warning nobody checked.
//! 3. **Determinism** — bitwise equality between `Serial` and `CpuMulti` at 1, 2,
//!    4 and 8 worker threads, on a batch built to have wildly uneven per-lane
//!    iteration counts.
//! 4. **Non-convergence** — every [`MinStatus`] failure path, and the
//!    all-or-nothing [`MinBatch::extrema`] error.
//! 5. **Measurement** — `#[ignore]`d benchmarks whose printed output is the
//!    source of every number in the constants' documentation.

use super::*;

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

/// `n` parabola vertices spread over `[-3, 3)`.
fn sample_vertices(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = Xorshift::new(seed);
    (0..n).map(|_| rng.next_in(-3.0, 3.0)).collect()
}

// ── 1. Analytic verification ─────────────────────────────────────────────────

/// **Methodology.** `f(x) = 1 + (x - x0)^2` has the exactly-known minimiser `x0`
/// and minimum value `1`. 64 lanes with vertices spread over `[-3, 3)` are
/// bracketed on `[-5, 5]` and minimised with [`MinSettings::default`]. The `+1`
/// baseline is deliberate and is not cosmetic — see
/// [`flat_minimum_exposes_the_sqrt_eps_limit`], which measures how much it
/// matters. It makes this the *realistic* case: a physical objective (a mass
/// flux, a residual with a floor) has an order-unity value at its extremum, so
/// the value differences that drive the contraction are computed against an
/// order-unity baseline and lose precision accordingly.
///
/// **Pass criterion.** `status == Converged` on every lane and
/// `|x_located - x0| <= 1e-6` — two orders of margin over the `sqrt(eps) ≈
/// 1.5e-8` floor derived in the module docs.
///
/// **Results, measured 2026-08-13 (release):** worst absolute abscissa error
/// **1.151892e-8** over the 64 lanes, worst error in the objective value at the
/// located minimiser **2.220446e-16**, iterations **40..=47**. Printed by this
/// test under `--nocapture`.
///
/// **Interpretation.** The worst abscissa error is 0.77x the `sqrt(eps) =
/// 1.490116e-8` floor, i.e. the method delivers exactly the accuracy the
/// arithmetic allows and no more — as it should, since the default `x_tol_rel`
/// *is* that floor and the answer is the bracket midpoint. The value error is one
/// ULP at `f = 1`, which is the module's argument about convergence criteria made
/// concrete: the objective value is already correct to the last bit while the
/// abscissa is still wrong in its 9th significant digit, so a value-based
/// stopping test would have declared success far too early.
///
/// The iteration count varies across lanes only because
/// [`MinSettings::default`]'s tolerance scales with `|x_mid|`, so lanes with
/// larger `|x0|` stop at a wider bracket. Under a purely absolute tolerance the
/// count is identical on every lane — golden section's contraction is fixed by
/// geometry, not by the objective — which
/// [`bracket_contracts_at_the_golden_ratio`] verifies directly.
#[test]
fn golden_section_matches_analytic_minima() {
    let vertices = sample_vertices(64, 0x51ce_9a11);
    let problems: Vec<MinProblem> = (0..64).map(|_| MinProblem::new(-5.0, 5.0)).collect();

    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |i, x| 1.0 + (x - vertices[i]) * (x - vertices[i]),
    );

    let located = batch.extrema().expect("every lane converges");
    let values = batch.extremal_values().expect("every lane converges");

    let mut worst_x = 0.0_f64;
    let mut worst_f = 0.0_f64;
    for i in 0..64 {
        worst_x = worst_x.max((located[i] - vertices[i]).abs());
        worst_f = worst_f.max((values[i] - 1.0).abs());
    }
    let iterations: Vec<u32> = batch.solutions().iter().map(|s| s.iterations()).collect();
    let min_it = iterations.iter().copied().min().unwrap();
    let max_it = iterations.iter().copied().max().unwrap();

    println!("quadratic: worst |x - x0|   = {worst_x:.6e}");
    println!("quadratic: worst |f(x*) - 1| = {worst_f:.6e}");
    println!("quadratic: iterations       = {min_it}..={max_it}");
    println!("quadratic: sqrt(eps)        = {SQRT_EPSILON:.6e}");

    assert!(
        worst_x <= 1e-6,
        "worst abscissa error {worst_x:.6e} exceeds 1e-6"
    );
}

/// The accuracy limit the module documents, measured on three objectives whose
/// minimisers are all the *same* and known exactly, so the only thing that varies
/// is how much precision the comparison of two nearby function values retains.
///
/// **Methodology.** 64 lanes, vertices spread over `[-3, 3)`, bracket `[-5, 5]`,
/// and — critically — a tolerance driven **far below** any arithmetic floor
/// (`x_tol_abs = 1e-15`, `x_tol_rel = 0`, `max_iterations = 300`) so that what
/// binds is the arithmetic and not the stopping rule. Running this at
/// [`MinSettings::default`] would measure the default tolerance instead and show
/// all three families agreeing, which is exactly the wrong conclusion. Three
/// objectives:
///
/// | Objective | Minimum value | Predicted abscissa floor |
/// |---|---|---|
/// | `1 + (x - x0)^2` | `1` (order unity) | `sqrt(eps) ≈ 1.49e-8` |
/// | `1 + (x - x0)^4` | `1` (order unity) | `eps^(1/4) ≈ 1.22e-4` |
/// | `(x - x0)^2` | `0`, and evaluated as `d*d` | none of the above |
///
/// The prediction comes from the module docs: two probes a distance `d` apart
/// around `x*` differ in value by `~ 0.5 f'' d^2` (or `~ d^4`), and the
/// comparison becomes meaningless once that falls below the rounding noise in `f`
/// itself, which is `eps * |f(x*)|`.
///
/// The third row is the control, and it is the reason this test exists in this
/// shape rather than as a single assertion: when `f(x*) = 0` **and** the
/// objective is evaluated so that its relative precision survives near the
/// minimum (`d*d` is exact to a half-ULP in `d`), there is no order-unity
/// baseline to swamp the difference and the search keeps its precision all the
/// way down. The `sqrt(eps)` floor is therefore a statement about the
/// **objective's relative precision near its extremum**, not a universal
/// constant of golden section.
///
/// **Pass criterion.** Every lane `Converged` in all three families;
/// `worst_quadratic <= 1e-6`; `worst_quartic <= 1e-2`; and the quartic family at
/// least **100x** worse than the offset-quadratic family, which is the effect
/// being demonstrated.
///
/// **Results, measured 2026-08-13 (release), 64 lanes each, 77 iterations on
/// every lane in all three families:**
///
/// | Objective | Predicted floor | Measured worst `\|x - x0\|` |
/// |---|---|---|
/// | `1 + (x - x0)^2` | `sqrt(eps) = 1.490116e-8` | **1.053671e-8** |
/// | `1 + (x - x0)^4` | `eps^(1/4) = 1.220703e-4` | **1.026485e-4** |
/// | `(x - x0)^2` | *none* | **8.881784e-16** |
///
/// Quartic/quadratic degradation: **9 742x**. Printed by this test under
/// `--nocapture`.
///
/// **Interpretation.** The two offset families land at 0.71x and 0.84x of their
/// predicted floors, so the `sqrt(eps)` / `eps^(1/4)` theory is a usable bound
/// here rather than a story — and the ~10 000x gap between them is the cost of a
/// minimum that is flatter than quadratic, measured on identical lanes, brackets
/// and settings. Every one of those quartic lanes still reported `Converged` with
/// a final bracket width of ~1e-15; a caller reading only `bracket_width()` would
/// conclude the answer was good to ~1e-15 when it is good to ~1e-4, and **no
/// status flag says otherwise**. That is why the module documents the accuracy
/// limit at length instead of leaving it to the status field.
///
/// The zero-baseline control reaching **8.9e-16** — seven orders below the
/// "floor" — is the result that stops `sqrt(eps)` being memorised as a property
/// of golden section. It is a property of the objective's relative precision near
/// its extremum. A physical objective is the first row, not the third.
#[test]
fn flat_minimum_exposes_the_sqrt_eps_limit() {
    let vertices = sample_vertices(64, 0x51ce_9a11);
    let problems: Vec<MinProblem> = (0..64).map(|_| MinProblem::new(-5.0, 5.0)).collect();
    // Below every arithmetic floor, so the arithmetic binds and not the tolerance.
    let settings = MinSettings {
        x_tol_abs: 1e-15,
        x_tol_rel: 0.0,
        max_iterations: 300,
    };

    let run = |f: &(dyn Fn(usize, f64) -> f64 + Sync)| {
        let batch = golden_section_batch(
            &problems,
            Sense::Minimise,
            settings,
            ComputeBackend::Serial,
            |i, x| f(i, x),
        );
        assert!(batch.all_converged());
        let located = batch.extrema().expect("every lane converges");
        let worst = (0..64).fold(0.0_f64, |acc, i| acc.max((located[i] - vertices[i]).abs()));
        let iterations = batch
            .solutions()
            .iter()
            .map(|s| s.iterations())
            .max()
            .unwrap();
        (worst, iterations)
    };

    let (worst_quadratic, it_quadratic) = run(&|i, x| {
        let d = x - vertices[i];
        1.0 + d * d
    });
    let (worst_quartic, it_quartic) = run(&|i, x| {
        let d = x - vertices[i];
        1.0 + d * d * d * d
    });
    let (worst_zero_baseline, it_zero) = run(&|i, x| {
        let d = x - vertices[i];
        d * d
    });

    println!("1 + (x-x0)^2 : worst |x - x0| = {worst_quadratic:.6e}  (max it {it_quadratic})");
    println!("1 + (x-x0)^4 : worst |x - x0| = {worst_quartic:.6e}  (max it {it_quartic})");
    println!("    (x-x0)^2 : worst |x - x0| = {worst_zero_baseline:.6e}  (max it {it_zero})");
    println!(
        "quartic / quadratic degradation = {:.0}x",
        worst_quartic / worst_quadratic
    );
    println!(
        "sqrt(eps) = {SQRT_EPSILON:.6e}, eps^(1/4) = {:.6e}",
        f64::EPSILON.powf(0.25)
    );

    assert!(
        worst_quadratic <= 1e-6,
        "offset quadratic worst {worst_quadratic:.6e}"
    );
    assert!(
        worst_quartic <= 1e-2,
        "offset quartic worst {worst_quartic:.6e}"
    );
    assert!(
        worst_quartic / worst_quadratic >= 100.0,
        "the flat minimum must be orders of magnitude worse, got {:.1}x",
        worst_quartic / worst_quadratic
    );
    assert!(
        worst_zero_baseline < worst_quadratic,
        "the zero-baseline control must beat the offset quadratic, \
         since it has no order-unity value to lose precision against"
    );
}

/// **Methodology.** `f(x) = x ln x` is unimodal on `(0, inf)` with
/// `f'(x) = ln x + 1`, so its minimiser is exactly `1/e =
/// 0.36787944117144233` and its minimum value exactly `-1/e`. One lane on the
/// bracket `[0.05, 3]`, [`MinSettings::default`]. Unlike the polynomial families
/// this objective is transcendental, so the oracle exercises the search rather
/// than a case where the answer happens to be a bracket-symmetric point.
///
/// **Pass criterion.** `status == Converged`, `|x_located - 1/e| <= 1e-6` and
/// `|f_located - (-1/e)| <= 1e-12`.
///
/// **Results, measured 2026-08-13 (release):** abscissa error **1.171529e-9**,
/// value error **5.551115e-17**, **38** iterations. Printed by this test under
/// `--nocapture`.
///
/// **Interpretation.** Same picture as the quadratic family and for the same
/// reason: the abscissa lands at the `sqrt(eps)` floor while the value is correct
/// to the last couple of bits, because the value error is quadratic in the
/// abscissa error near a smooth minimum.
#[test]
fn golden_section_matches_analytic_transcendental_minimum() {
    let x_star = std::f64::consts::E.recip();
    let f_star = -x_star;

    let problems = [MinProblem::new(0.05, 3.0)];
    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| x * x.ln(),
    );

    let s = batch.solutions()[0];
    let dx = (s.extremum().expect("converged") - x_star).abs();
    let df = (s.extremal_value().expect("converged") - f_star).abs();
    println!("x ln x: |x - 1/e|   = {dx:.6e}");
    println!("x ln x: |f - (-1/e)| = {df:.6e}");
    println!("x ln x: iterations   = {}", s.iterations());

    assert!(dx <= 1e-6, "abscissa error {dx:.6e}");
    assert!(df <= 1e-12, "value error {df:.6e}");
}

/// **Methodology.** `f(x) = sin x` on `[0, pi]` has the exactly-known maximiser
/// `pi/2` and maximum value `1`. Run under [`Sense::Maximise`], so the sense
/// switch is checked against an analytic answer rather than against the
/// minimisation path — a mirror test would pass even if both senses were wrong in
/// the same way.
///
/// **Pass criterion.** `status == Converged`, `|x_located - pi/2| <= 1e-6`,
/// `|f_located - 1| <= 1e-12`, and the reported value is **positive**, i.e. the
/// caller's own sign is preserved and nothing was negated internally.
///
/// **Results, measured 2026-08-13 (release):** abscissa error **1.052771e-9**,
/// value error **0.000000e0** — the located maximum reproduced `1.0` exactly —
/// and **39** iterations. Printed by this test under `--nocapture`.
///
/// **Interpretation.** Maximisation reaches the same `sqrt(eps)`-floor abscissa
/// accuracy as minimisation, which it must, since [`Sense`] changes one
/// comparison and nothing else. The exact `1.0` is the quadratic-flatness effect
/// again: at 1e-9 from the peak, `sin` differs from 1 by ~5e-19, well below the
/// last bit of a `f64` near unity.
#[test]
fn maximise_matches_analytic_sine_peak() {
    let x_star = std::f64::consts::FRAC_PI_2;

    let problems = [MinProblem::new(0.0, std::f64::consts::PI)];
    let batch = golden_section_batch(
        &problems,
        Sense::Maximise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| x.sin(),
    );

    let s = batch.solutions()[0];
    let dx = (s.extremum().expect("converged") - x_star).abs();
    let df = (s.extremal_value().expect("converged") - 1.0).abs();
    println!("sin max: |x - pi/2| = {dx:.6e}");
    println!("sin max: |f - 1|    = {df:.6e}");
    println!("sin max: iterations = {}", s.iterations());

    assert!(dx <= 1e-6, "abscissa error {dx:.6e}");
    assert!(df <= 1e-12, "value error {df:.6e}");
    assert!(
        s.extremal_value().unwrap() > 0.0,
        "the caller's sign must be preserved; nothing is negated internally"
    );
}

/// **Methodology.** Golden section's defining property is that the bracket
/// contracts by exactly [`GOLDEN_RATIO`] per iteration, *independently of the
/// objective*. With the tolerance set to zero the lane can never converge, so it
/// runs exactly `max_iterations` iterations and reports the resulting bracket
/// width; that width is compared against the closed form `W0 * gr^k` for
/// `k = 1, 2, 5, 10, 20, 30, 40, 50, 60, 80` on a bracket of width `W0 = 10`.
///
/// **Pass criterion.** `status == MaxIterations` and `iterations == k` at every
/// `k` (the tolerance is unreachable by construction); relative deviation from
/// `W0 * gr^k` at most `1e-13` for `k <= 20` and at most `1e-5` over the whole
/// range. The two-tier criterion is not a fudge — it is the measured effect of
/// probe reuse, quantified below.
///
/// **Results, measured 2026-08-13 (release), `W0 = 10`:** relative deviation of
/// the measured bracket width from `W0 * gr^k` —
///
/// | `k` | 1 | 2 | 5 | 10 | 20 | 30 | 40 | 50 | 60 | 80 |
/// |---|---|---|---|---|---|---|---|---|---|---|
/// | rel. dev. | 0 | 2.2e-16 | 4.4e-16 | 1.6e-15 | 2.7e-14 | 1.3e-12 | 2.3e-11 | 4.0e-10 | 1.9e-8 | 2.3e-6 |
///
/// Worst for `k <= 20`: **2.675637e-14**. Worst over the whole range:
/// **2.334157e-6**, at `k = 80`. Printed by this test under `--nocapture`.
///
/// **Interpretation.** The contraction rate itself is exact — the deviation is
/// accumulated *rounding*, and it is the measured price of the probe-reuse
/// optimisation this module makes over its `tampines-steam-tables` original. The
/// retained probe carries an absolute rounding error fixed at the scale of the
/// older, wider bracket, while the bracket keeps shrinking, so the relative
/// deviation grows roughly geometrically. A reference implementation that
/// recomputes both probes each iteration — the original's form — holds the
/// deviation at `<= 6.7e-16` at every one of these `k` (checked out-of-band in a
/// scratch reimplementation, not by this test).
///
/// **It does not affect any answer.** At `k = 80` the width is 1.9e-16, so a
/// 2.3e-6 relative deviation is 4e-22 absolute — orders below the `sqrt(eps)`-
/// scale accuracy the located extremum can have at all — and 80 iterations is
/// about twice what [`MinSettings::default`] ever reaches (40-47, measured by
/// [`golden_section_matches_analytic_minima`]). What it does mean is that
/// `W0 * gr^k` stops being an exact predictor of
/// [`MinSolution::bracket_width`] at large `k`, which is why this test's
/// criterion is tiered rather than uniform.
#[test]
fn bracket_contracts_at_the_golden_ratio() {
    let w0 = 10.0_f64;
    let mut worst = 0.0_f64;
    let mut worst_small_k = 0.0_f64;

    for k in [1_u32, 2, 5, 10, 20, 30, 40, 50, 60, 80] {
        let settings = MinSettings {
            x_tol_abs: 0.0,
            x_tol_rel: 0.0,
            max_iterations: k,
        };
        let problems = [MinProblem::new(-5.0, 5.0)];
        let batch = golden_section_batch(
            &problems,
            Sense::Minimise,
            settings,
            ComputeBackend::Serial,
            |_, x| x * x,
        );
        let s = batch.solutions()[0];
        assert_eq!(
            s.status(),
            MinStatus::MaxIterations,
            "a zero tolerance cannot be met"
        );
        assert_eq!(s.iterations(), k);

        let expected = w0 * GOLDEN_RATIO.powi(k as i32);
        let deviation = (s.bracket_width() / expected - 1.0).abs();
        worst = worst.max(deviation);
        if k <= 20 {
            worst_small_k = worst_small_k.max(deviation);
        }
        println!(
            "k = {k:>2}: width = {:.17e}, W0*gr^k = {expected:.17e}, rel dev = {deviation:.6e}",
            s.bracket_width()
        );
    }

    println!("worst relative deviation, k <= 20 = {worst_small_k:.6e}");
    println!("worst relative deviation, all k   = {worst:.6e}");
    assert!(
        worst_small_k <= 1e-13,
        "contraction rate deviates by {worst_small_k:.6e} for k <= 20"
    );
    assert!(
        worst <= 1e-5,
        "contraction rate deviates by {worst:.6e} over the whole range"
    );
}

// ── 2. The unimodality precondition ──────────────────────────────────────────

/// The documented failure mode, written down as a measurement: on a **multimodal**
/// bracket golden section returns *a* local minimum, with `Converged` status and
/// no warning, and it can be the wrong one.
///
/// **Methodology.** The objective has two wells on `[-2, 2]`:
///
/// ```text
/// f(x) = -exp(-100*(x + 1.5)^2) - 0.5*exp(-(x - 1.0)^2)
/// ```
///
/// a **deep, narrow** global minimum near `x = -1.5` (depth 1, width ~0.1) and a
/// **shallow, wide** local minimum near `x = +1.0` (depth 0.5). The first pair of
/// interior probes sits at `x ≈ ∓0.472`, neither of which can see the narrow well
/// at all, so the first contraction discards it irrecoverably.
///
/// **Pass criterion.** This test asserts the *documented* behaviour, not
/// correctness: the lane must report `Converged`, must land on the shallow local
/// minimum near `+1.0`, and must **not** find the global minimum near `-1.5`.
///
/// **Results, measured 2026-08-13 (release):** located `x = 1.000000007e0`,
/// value `-5.0000000000000000e-1`, status `Converged`, **41** iterations. The
/// global minimum near `x = -1.5`, whose value is `-1.000965`, was never visited.
/// Printed by this test under `--nocapture`.
///
/// **Interpretation.** The returned answer is a genuine local minimum reported
/// with full confidence and a tight bracket, and it is **half as deep** as the
/// true global one. Nothing in [`MinSolution`] distinguishes this from the
/// correct answer, because nothing can: golden section evaluates the objective
/// `k + 3` times and a well of width 0.1 in a bracket of width 4 is invisible to
/// that budget. This is why the module documents unimodality as a caller
/// precondition and why the steam-tables production caller coarse-scans 1500
/// points before refining.
#[test]
fn multimodal_bracket_returns_a_local_extremum_without_warning() {
    let f = |x: f64| {
        let narrow = -(-100.0 * (x + 1.5) * (x + 1.5)).exp();
        let wide = -0.5 * (-(x - 1.0) * (x - 1.0)).exp();
        narrow + wide
    };

    let problems = [MinProblem::new(-2.0, 2.0)];
    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| f(x),
    );

    let s = batch.solutions()[0];
    println!(
        "multimodal: status = {}, x = {:.9e}, f = {:.16e}, iterations = {}",
        s.status().label(),
        s.last_iterate(),
        s.last_value(),
        s.iterations()
    );
    println!(
        "multimodal: true global minimum is near x = -1.5 with f = {:.6}",
        f(-1.5)
    );

    assert_eq!(s.status(), MinStatus::Converged);
    let x = s.extremum().expect("it converges -- that is the problem");
    assert!(
        (x - 1.0).abs() < 1e-6,
        "expected the shallow local minimum near +1.0, got {x}"
    );
    assert!(
        (x + 1.5).abs() > 1.0,
        "the deep global minimum near -1.5 must have been missed"
    );
    assert!(
        s.extremal_value().unwrap() > f(-1.5),
        "the located minimum must be shallower than the global one"
    );
}

/// A **monotone** objective on the bracket is the benign special case: the
/// contraction walks to the appropriate endpoint and converges there.
///
/// **Methodology.** `f(x) = x` on `[0, 1]` under [`Sense::Minimise`] and under
/// [`Sense::Maximise`], with [`MinSettings::default`]. The minimum of a monotone
/// increasing function on a closed interval is its left endpoint and the maximum
/// its right endpoint, both known exactly.
///
/// **Pass criterion.** Both lanes report `Converged`; the minimising lane lands
/// within `1e-6` of `0.0` and the maximising lane within `1e-6` of `1.0`.
///
/// **Results, measured 2026-08-13 (release):** minimising lane located
/// `3.781698226100e-13`, maximising lane located `9.999999942794e-1`. Printed by
/// this test under `--nocapture`.
///
/// **Interpretation.** This is the intended behaviour, and it is what the
/// steam-tables original relies on: its doc comment records that "on a monotone
/// stretch it converges to the appropriate endpoint, which is the intended
/// behaviour when the real peak lies in the neighbouring stretch". Note the
/// residual offset from the exact endpoint — the answer is the midpoint of a
/// final bracket that is anchored at the endpoint but has nonzero width, so it
/// sits half a tolerance inside. That is a truthful report of what the bracket
/// establishes, not an error.
#[test]
fn monotone_bracket_walks_to_the_endpoint() {
    let problems = [MinProblem::new(0.0, 1.0)];

    for (sense, want) in [(Sense::Minimise, 0.0_f64), (Sense::Maximise, 1.0)] {
        let batch = golden_section_batch(
            &problems,
            sense,
            MinSettings::default(),
            ComputeBackend::Serial,
            |_, x| x,
        );
        let s = batch.solutions()[0];
        let got = s.extremum().expect("converged");
        println!("monotone {}: located {got:.12e}", sense.label());
        assert!(
            (got - want).abs() < 1e-6,
            "{}: got {got}, want {want}",
            sense.label()
        );
    }
}

// ── 3. Determinism ───────────────────────────────────────────────────────────

/// A batch whose lanes take wildly different numbers of iterations, which is the
/// case that would expose any schedule dependence.
///
/// Half the lanes get a bracket that already nearly pins the minimiser (a few
/// iterations); half get `[-1e6, 1e6]` (many more). Returns
/// `(problems, vertices)`.
fn imbalanced_problems(n: usize) -> (Vec<MinProblem>, Vec<f64>) {
    let mut rng = Xorshift::new(0xabcd_0f0f);
    let mut problems = Vec::with_capacity(n);
    let mut vertices = Vec::with_capacity(n);
    for i in 0..n {
        let x0 = rng.next_in(-3.0, 3.0);
        if i % 2 == 0 {
            problems.push(MinProblem::new(x0 - 1e-6, x0 + 1e-6));
        } else {
            problems.push(MinProblem::new(-1.0e6, 1.0e6));
        }
        vertices.push(x0);
    }
    (problems, vertices)
}

/// Compare two batches bit for bit, on every field a caller can observe.
fn assert_bitwise_equal(a: &MinBatch, b: &MinBatch, what: &str) {
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
            x.last_value().to_bits(),
            y.last_value().to_bits(),
            "{what}: lane {i} value"
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
/// **Methodology.** 2 048 lanes of `(x - x0)^2`, half with a 2e-6-wide bracket
/// and half with `[-1e6, 1e6]`, so per-lane iteration counts differ by an order
/// of magnitude. Both senses are compared.
///
/// **Pass criterion.** Every observable field of every lane is bit-identical.
///
/// **Result, measured 2026-08-13 (release, `--features parallel`):** identical on
/// all 2 048 lanes for both [`Sense::Minimise`] and [`Sense::Maximise`]. With the
/// feature off the parallel request resolves to serial and the test is a
/// tautology that still guards the resolve path.
#[test]
fn bitwise_identical_across_backends() {
    let (problems, vertices) = imbalanced_problems(2048);
    let settings = MinSettings::default();

    for sense in [Sense::Minimise, Sense::Maximise] {
        let serial = golden_section_batch_min(
            &problems,
            sense,
            settings,
            ComputeBackend::Serial,
            0,
            |i, x| (x - vertices[i]) * (x - vertices[i]),
        );
        let multi = golden_section_batch_min(
            &problems,
            sense,
            settings,
            ComputeBackend::CpuMulti,
            0,
            |i, x| (x - vertices[i]) * (x - vertices[i]),
        );
        assert_bitwise_equal(&serial, &multi, sense.label());
    }
}

/// The same batch at 1, 2, 4 and 8 worker threads is bit-identical to serial.
///
/// This is the claim that matters: a golden-section batch has no cross-lane
/// arithmetic, so unlike a reduction its result cannot depend on the thread
/// count. Without the `parallel` feature there is no pool to build and the test
/// degenerates to the serial comparison above, so it is compiled only with the
/// feature on.
///
/// **Result, measured 2026-08-13 (release, `--features parallel`, 4 logical
/// cores):** identical at every one of the four thread counts, on all 2 048
/// lanes.
#[cfg(feature = "parallel")]
#[test]
fn bitwise_identical_across_thread_counts() {
    let (problems, vertices) = imbalanced_problems(2048);
    let settings = MinSettings::default();

    let serial = golden_section_batch_min(
        &problems,
        Sense::Minimise,
        settings,
        ComputeBackend::Serial,
        0,
        |i, x| (x - vertices[i]) * (x - vertices[i]),
    );

    for threads in [1_usize, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        pool.install(|| {
            let multi = golden_section_batch_min(
                &problems,
                Sense::Minimise,
                settings,
                ComputeBackend::CpuMulti,
                0,
                |i, x| (x - vertices[i]) * (x - vertices[i]),
            );
            assert_bitwise_equal(&serial, &multi, &format!("minimise @ {threads} threads"));
        });
    }
}

/// Lane `i` of the result corresponds to `problems[i]`, on both backends.
///
/// Guards the ordering contract [`MinBatch`] documents. Each lane is given a
/// vertex equal to its own index, so a permutation would be immediately visible.
#[test]
fn lane_order_is_preserved() {
    let n = 512;
    let problems: Vec<MinProblem> = (0..n).map(|_| MinProblem::new(-1.0, 600.0)).collect();
    for backend in [ComputeBackend::Serial, ComputeBackend::CpuMulti] {
        let batch = golden_section_batch_min(
            &problems,
            Sense::Minimise,
            MinSettings::default(),
            backend,
            0,
            |i, x| {
                let d = x - i as f64;
                d * d
            },
        );
        let located = batch.extrema().expect("every lane converges");
        for (i, x) in located.iter().enumerate() {
            assert!(
                (x - i as f64).abs() < 1e-5,
                "{}: lane {i} located {x}",
                backend.label()
            );
        }
    }
}

// ── 4. Non-convergence reporting ─────────────────────────────────────────────

/// A lane that runs out of iterations reports [`MinStatus::MaxIterations`], and
/// [`MinSolution::extremum`] refuses to hand out its iterate.
#[test]
fn max_iterations_is_reported() {
    let settings = MinSettings {
        x_tol_abs: 0.0,
        x_tol_rel: 0.0,
        max_iterations: 5,
    };
    let problems = [MinProblem::new(-5.0, 5.0)];
    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        settings,
        ComputeBackend::Serial,
        |_, x| x * x,
    );

    let s = batch.solutions()[0];
    assert_eq!(s.status(), MinStatus::MaxIterations);
    assert_eq!(s.iterations(), 5);
    assert!(s.extremum().is_none(), "a failed lane yields no extremum");
    assert!(s.extremal_value().is_none());
    // The diagnostic iterate is still there, behind its deliberately blunt name.
    assert!(s.last_iterate().is_finite());
    assert!(s.bracket_width() > 0.0);
    assert!(batch.extrema().is_err());
}

/// A non-finite bracket end is [`MinStatus::InvalidBracket`] with a `NaN`
/// iterate — never a silently clamped endpoint.
#[test]
fn invalid_bracket_is_reported() {
    let problems = [
        MinProblem::new(f64::NAN, 1.0),
        MinProblem::new(0.0, f64::INFINITY),
        MinProblem::new(f64::NEG_INFINITY, f64::INFINITY),
    ];
    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| x * x,
    );

    for (i, s) in batch.solutions().iter().enumerate() {
        assert_eq!(s.status(), MinStatus::InvalidBracket, "lane {i}");
        assert!(s.last_iterate().is_nan(), "lane {i} must report NaN");
        assert!(s.extremum().is_none(), "lane {i}");
        assert_eq!(s.iterations(), 0, "lane {i}");
    }
    let err = batch.extrema().expect_err("all three lanes fail");
    assert_eq!(err.failure_count, 3);
    assert_eq!(err.total, 3);
    assert_eq!(err.first_index, 0);
    assert_eq!(err.first_status, MinStatus::InvalidBracket);
}

/// A non-finite objective value stops the lane at the offending abscissa with
/// [`MinStatus::NotFinite`], rather than propagating a `NaN` into a
/// plausible-looking answer.
#[test]
fn not_finite_objective_is_reported() {
    // Non-finite immediately, at the very first probe.
    let problems = [MinProblem::new(-1.0, 1.0)];
    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, _| f64::NAN,
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), MinStatus::NotFinite);
    assert_eq!(s.iterations(), 0);
    assert!(s.last_value().is_nan());
    assert!(s.extremum().is_none());

    // Non-finite only once the contraction has moved in, so the failure happens
    // mid-iteration rather than during setup.
    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| if x.abs() < 0.1 { f64::INFINITY } else { x * x },
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), MinStatus::NotFinite);
    assert!(s.iterations() > 0, "the failure must be mid-iteration");
    assert!(s.last_value().is_infinite());
    assert!(s.extremum().is_none());
}

/// A handful of failures inside a large batch are individually reachable, and the
/// all-or-nothing accessor refuses the whole batch while naming the first one.
#[test]
fn partial_failure_in_a_large_batch_is_reported() {
    let n = 10_000_usize;
    let bad = [17_usize, 4_242, 9_999];
    let problems: Vec<MinProblem> = (0..n)
        .map(|i| {
            if bad.contains(&i) {
                MinProblem::new(f64::NAN, 1.0)
            } else {
                MinProblem::new(-5.0, 5.0)
            }
        })
        .collect();

    let batch = golden_section_batch(
        &problems,
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::CpuMulti,
        |_, x| x * x,
    );

    assert!(!batch.all_converged());
    assert_eq!(batch.failure_count(), 3);
    assert_eq!(batch.first_failure().map(|(i, _)| i), Some(17));
    assert_eq!(
        batch.failures().iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        bad.to_vec()
    );

    let err = batch.extrema().expect_err("three lanes failed");
    assert_eq!(err.total, n);
    assert_eq!(err.failure_count, 3);
    assert_eq!(err.first_index, 17);
    // The message names both the scale and a specific lane to look at.
    let text = err.to_string();
    assert!(text.contains("3 of 10000"), "message was: {text}");
    assert!(text.contains("lane 17"), "message was: {text}");

    // Every good lane is still individually readable.
    assert!(batch.solutions()[0].extremum().is_some());
    assert!(batch.solutions()[17].extremum().is_none());
}

/// An empty batch is vacuously converged, allocates nothing, and calls the
/// objective zero times.
#[test]
fn empty_batch_is_vacuously_converged() {
    use std::sync::atomic::{AtomicU32, Ordering};
    let calls = AtomicU32::new(0);
    let batch = golden_section_batch(
        &[],
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x: f64| {
            calls.fetch_add(1, Ordering::Relaxed);
            x
        },
    );
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
    assert!(batch.all_converged());
    assert_eq!(batch.extrema().unwrap(), Vec::<f64>::new());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(batch.get(0).is_none());
}

/// A bracket given the "wrong" way round is normalised, not rejected — matching
/// [`crate::math::parallel::RootProblem`]'s contract.
#[test]
fn reversed_bracket_is_normalised() {
    let forward = golden_section_batch(
        &[MinProblem::new(-5.0, 5.0)],
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| (x - 1.5) * (x - 1.5),
    );
    let reversed = golden_section_batch(
        &[MinProblem::new(5.0, -5.0)],
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| (x - 1.5) * (x - 1.5),
    );
    assert_eq!(forward, reversed, "orientation must not change the answer");
    assert!((forward.extrema().unwrap()[0] - 1.5).abs() < 1e-6);
}

/// A zero-width bracket converges immediately at that point, with zero
/// iterations — truthful, because a zero-width bracket has already met any
/// non-negative tolerance.
#[test]
fn degenerate_bracket_converges_immediately() {
    let batch = golden_section_batch(
        &[MinProblem::new(2.0, 2.0)],
        Sense::Minimise,
        MinSettings::default(),
        ComputeBackend::Serial,
        |_, x| (x - 1.5) * (x - 1.5),
    );
    let s = batch.solutions()[0];
    assert_eq!(s.status(), MinStatus::Converged);
    assert_eq!(s.iterations(), 0);
    assert_eq!(s.extremum(), Some(2.0));
    assert_eq!(s.bracket_width(), 0.0);
    // It reports the point it was given, not the true minimiser at 1.5 -- the
    // bracket is the caller's assertion about where the answer is.
    assert_eq!(s.extremal_value(), Some(0.25));
}

// ── Dispatch policy ──────────────────────────────────────────────────────────

/// [`minimise_backend_for`] applies the size floor, and reports what would
/// actually run.
#[test]
fn dispatch_respects_the_size_floor() {
    assert_eq!(
        minimise_backend_for(ComputeBackend::CpuMulti, MINIMISE_BATCH_MIN_PROBLEMS - 1),
        ComputeBackend::Serial
    );
    assert_eq!(
        minimise_backend_for(ComputeBackend::Serial, 1 << 20),
        ComputeBackend::Serial
    );
    // Never Gpu: there is no GPU kernel here yet.
    assert_ne!(
        minimise_backend_for(ComputeBackend::Gpu, 1 << 20),
        ComputeBackend::Gpu
    );
    // Whatever it picks is runnable.
    assert!(minimise_backend_for(ComputeBackend::CpuMulti, 1 << 20).is_available());

    let expected_multi = cfg!(feature = "parallel");
    assert_eq!(
        minimise_backend_for(ComputeBackend::CpuMulti, MINIMISE_BATCH_MIN_PROBLEMS)
            == ComputeBackend::CpuMulti,
        expected_multi
    );
}

/// The public entry point gives the same answer for every requested backend,
/// including one whose feature may be off.
#[test]
fn public_entry_point_agrees_across_requested_backends() {
    let vertices = sample_vertices(1024, 0x0bad_c0de);
    let problems: Vec<MinProblem> = (0..1024).map(|_| MinProblem::new(-8.0, 8.0)).collect();

    let run = |backend| {
        golden_section_batch(
            &problems,
            Sense::Minimise,
            MinSettings::default(),
            backend,
            |i, x| (x - vertices[i]) * (x - vertices[i]),
        )
    };
    let serial = run(ComputeBackend::Serial);
    assert_bitwise_equal(&serial, &run(ComputeBackend::CpuMulti), "public cpu-multi");
    assert_bitwise_equal(&serial, &run(ComputeBackend::Gpu), "public gpu-request");
}

// ── 5. Measurement (ignored: too slow for the ordinary suite) ────────────────

/// Crossover benchmark — the source of the table on
/// [`MINIMISE_BATCH_MIN_PROBLEMS`].
///
/// `#[ignore]`d because it is a measurement, not a correctness check.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 minimise_batch_crossover_benchmark
/// ```
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn minimise_batch_crossover_benchmark() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!("MINIMISE_BATCH_MIN_PROBLEMS = {MINIMISE_BATCH_MIN_PROBLEMS}");
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

    let settings = MinSettings::default();

    for n in [16_usize, 32, 64, 128, 256, 512, 1024, 4096, 16_384, 65_536] {
        let vertices = sample_vertices(n, 0x1234_5678);
        let problems: Vec<MinProblem> = (0..n).map(|_| MinProblem::new(-5.0, 5.0)).collect();

        // Cheap objective: a two-flop parabola.
        let cheap = |i: usize, x: f64| (x - vertices[i]) * (x - vertices[i]);
        // Costly objective: a transcendental chain standing in for an
        // equation-of-state flash. Same minimiser, so iteration counts match.
        let costly = |i: usize, x: f64| {
            let d = x - vertices[i];
            let s = (1.0 + d * d).ln().exp().sqrt();
            s * s - 1.0
        };

        let time = |backend: ComputeBackend, costly_mode: bool| -> f64 {
            let run = || {
                if costly_mode {
                    golden_section_batch_min(
                        &problems,
                        Sense::Minimise,
                        settings,
                        backend,
                        0,
                        costly,
                    )
                } else {
                    golden_section_batch_min(
                        &problems,
                        Sense::Minimise,
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

/// Thread-scaling measurement on a fixed batch, with the bitwise-identity claim
/// re-asserted at each thread count.
///
/// `#[ignore]`d.
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     -- --ignored --nocapture --test-threads=1 minimise_batch_thread_scaling_benchmark
/// ```
#[cfg(feature = "parallel")]
#[test]
#[ignore = "measurement, not a correctness check. Run with --ignored --nocapture"]
fn minimise_batch_thread_scaling_benchmark() {
    use std::time::Instant;

    let n = 65_536_usize;
    let (problems, vertices) = imbalanced_problems(n);
    let settings = MinSettings::default();
    let f = |i: usize, x: f64| (x - vertices[i]) * (x - vertices[i]);

    let reference = golden_section_batch_min(
        &problems,
        Sense::Minimise,
        settings,
        ComputeBackend::Serial,
        0,
        f,
    );

    let mut serial_us = f64::INFINITY;
    for _ in 0..7 {
        let t = Instant::now();
        let out = golden_section_batch_min(
            &problems,
            Sense::Minimise,
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
    println!("batch = {n} imbalanced lanes, golden section, best of 7");
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
            let first = golden_section_batch_min(
                &problems,
                Sense::Minimise,
                settings,
                ComputeBackend::CpuMulti,
                0,
                f,
            );
            let identical = first == reference;
            let mut best = f64::INFINITY;
            for _ in 0..7 {
                let t = Instant::now();
                let out = golden_section_batch_min(
                    &problems,
                    Sense::Minimise,
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
