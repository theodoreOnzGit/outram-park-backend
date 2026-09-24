// SPDX-License-Identifier: GPL-3.0

//! **A two-stage surface-source run reproduces the single-stage answer** —
//! gh:#264 acceptance bullet 1, *"A two-stage surface-source run reproduces the
//! single-stage analog answer within statistics on a case where both are
//! affordable. This is the check that the recorded crossings carry the full
//! phase-space state and weight correctly."*
//!
//! # What was missing, and it was not just the test
//!
//! Before 2026-09-24 the two halves of this workflow existed and **nothing
//! joined them**: `transport_csg` recorded crossings into a `SurfaceSource`,
//! and `SurfaceSource::sample` could hand back a site — but `FixedSource` had
//! only `Point` and `Box`, so no run could be driven from a recorded bank.
//! Stage two was unreachable, so the acceptance criterion was not merely
//! untested, it was unrunnable.
//!
//! `FixedSource::Surface(Arc<SurfaceSource>)` closes that, and with it
//! `FixedSource::sample` now returns a **birth weight** rather than an implicit
//! `1.0`. That is the load-bearing part: a recorded particle crossed with the
//! weight it had, and starting the replay at 1.0 would discard exactly the
//! information the recording exists to preserve. Under variance reduction the
//! recorded weights are nowhere near 1, so stage two would silently answer a
//! rescaled problem.
//!
//! # Geometry
//!
//! Two concentric spheres in **void**, so transport is analytic and the test
//! costs milliseconds: `r = 5` (transmissive, the recording surface) inside
//! `r = 10` (vacuum). The tally is track-length flux in the **outer shell**,
//! i.e. entirely beyond the recording surface, so every contribution to it must
//! have crossed the recorded surface exactly once.
//!
//! The source sits at `(2, 0, 0)`, **deliberately off centre**. From the origin
//! every crossing would be radial and every shell path exactly `R2 - R1`,
//! making both tallies degenerate and the comparison vacuous. Off centre the
//! chord lengths vary, so stage two is a genuine statistical estimate of the
//! same quantity.
//!
//! # The normalisation, which is where this test could fool itself
//!
//! `SurfaceSource::sample` draws **uniformly among the `K` recorded crossings**
//! and returns each one's recorded weight. So `M` stage-two histories sample
//! `M/K` of the bank, and the stage-one-equivalent estimate is
//!
//! ```text
//!     T_two_stage = T2 * K / M
//! ```
//!
//! Getting that factor wrong is the obvious way to make this test pass or fail
//! for the wrong reason, so it is derived here rather than tuned: with `M = K`
//! it reduces to `T1 == T2`, which is the case the test runs.
//!
//! # Results (2026-09-24)
//!
//! Printed by the tests. Both agree well within their combined statistics, and
//! the recorded weight is shown to propagate by a separate synthetic case that
//! replays a hand-built bank at weight `0.25` and gets a quarter of the answer.

use std::sync::Arc;

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source, run_fixed_source_traced, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::source::extra::{SurfaceCrossing, SurfaceSource};
use outram_mc_libs::tally::filter::{CellFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const R1: f64 = 5.0;
const R2: f64 = 10.0;
const E0: f64 = 1.0e6;
const N_BATCHES: usize = 20;

/// Void: inner ball (cell 0) inside an outer shell (cell 1), vacuum at `R2`.
/// The `R1` sphere is **transmissive** — the surface whose crossings are
/// recorded.
fn two_shell_void() -> Geometry {
    Geometry {
        surfaces: vec![
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: R1,
                bc: BoundaryType::Transmissive,
            }),
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: R2,
                bc: BoundaryType::Vacuum,
            }),
        ],
        cells: vec![
            Cell::fill(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                CellFill::Void,
                Position::ZERO,
            ),
            Cell::fill(
                2,
                vec![
                    RegionToken::HalfSpace {
                        surface_idx: 1,
                        sense: HalfSpaceSense::Inside,
                    },
                    RegionToken::HalfSpace {
                        surface_idx: 0,
                        sense: HalfSpaceSense::Outside,
                    },
                    RegionToken::Intersection,
                ],
                CellFill::Void,
                Position::ZERO,
            ),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Track-length flux in the **outer shell only** (cell index 1) — entirely
/// beyond the recording surface.
fn shell_flux_tally() -> Tally {
    Tally {
        id: 1,
        name: "shell flux".into(),
        filters: vec![FilterKind::Cell(CellFilter {
            cell_indices: vec![1],
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 1],
    }
}

fn settings(n_particles: usize, seed: u64) -> FixedSourceSettings {
    FixedSourceSettings {
        n_particles,
        n_batches: N_BATCHES,
        seed,
        ..Default::default()
    }
}

/// Off-centre, so the recorded directions are not radial and the shell chord
/// lengths vary.
fn source() -> FixedSource {
    FixedSource::Point {
        r: Position::new(2.0, 0.0, 0.0),
        energy_ev: E0,
    }
}

/// **The acceptance check.** Single-stage against record-then-replay.
#[test]
fn a_two_stage_surface_source_run_reproduces_the_single_stage_answer() {
    let geom = two_shell_void();
    let n: usize = 40_000;

    // ---- single stage -------------------------------------------------------
    let mut single = shell_flux_tally();
    run_fixed_source(
        &geom,
        &[],
        &[],
        &source(),
        &settings(n, 11_027),
        Some(&mut single),
    );
    let t1 = single.bins[0].sum;
    let r1 = single.bins[0].rel_std_dev(N_BATCHES as u64);

    // ---- stage one: same run, recording crossings of surface 0 --------------
    let mut ss = SurfaceSource::recording(vec![0], 10 * n);
    let mut stage1 = shell_flux_tally();
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &source(),
        &settings(n, 11_027),
        Some(&mut stage1),
        None,
        Some(&mut ss),
        None,
    );
    let k = ss.len();
    assert!(k > 0, "stage one recorded no crossings at all");
    // In a void every source particle streams out and crosses R1 exactly once.
    assert_eq!(
        k, n,
        "in a void each of the {n} source particles must cross R1 exactly once; \
         recorded {k}. A mismatch means the recording hook missed crossings or \
         double-counted them, which would silently rescale stage two."
    );
    assert!(
        (ss.total_weight() - n as f64).abs() < 1.0e-9 * n as f64,
        "recorded total weight {} against {n} analog particles at weight 1",
        ss.total_weight()
    );
    // Recording must not perturb the run it records.
    assert_eq!(
        stage1.bins[0].sum.to_bits(),
        t1.to_bits(),
        "recording changed the run: {} against {t1}",
        stage1.bins[0].sum
    );

    // ---- stage two: replay the bank -----------------------------------------
    let m: usize = k; // so the K/M factor is exactly 1
    let replay = FixedSource::Surface(Arc::new(ss));
    let mut stage2 = shell_flux_tally();
    run_fixed_source(
        &geom,
        &[],
        &[],
        &replay,
        &settings(m, 98_317),
        Some(&mut stage2),
    );
    let t2 = stage2.bins[0].sum * k as f64 / m as f64;
    let r2 = stage2.bins[0].rel_std_dev(N_BATCHES as u64);

    // ---- compare ------------------------------------------------------------
    let sigma = ((t1 * r1).powi(2) + (t2 * r2).powi(2)).sqrt();
    let diff = (t2 - t1).abs();
    println!(
        "single stage {t1:.6e} (R={r1:.4}); two stage {t2:.6e} (R={r2:.4}); \
         difference {diff:.3e} = {:.2} sigma of the combined {sigma:.3e}",
        if sigma > 0.0 { diff / sigma } else { 0.0 }
    );
    println!(
        "  recorded K={k} crossings of total weight {:.1}; replayed M={m} histories",
        k as f64
    );
    assert!(
        sigma > 0.0,
        "both tallies report zero uncertainty, so this comparison proves nothing"
    );
    assert!(
        diff <= 4.0 * sigma,
        "two-stage {t2:.6e} against single-stage {t1:.6e} is {:.1} sigma of the \
         combined {sigma:.3e} — past the 4 sigma budget. The recorded crossings do \
         not reproduce the single-stage answer.",
        diff / sigma
    );
    // A sanity floor: a comparison of two numbers that are both ~0 would pass
    // the sigma test and mean nothing.
    assert!(
        t1 > 0.0 && t2 > 0.0,
        "one of the arms scored nothing: {t1} and {t2}"
    );
}

/// **The recorded weight propagates.** A hand-built bank at weight `0.25` must
/// give a quarter of the answer a bank at weight `1.0` gives — the plumbing
/// `FixedSource::sample`'s birth weight was added for.
///
/// This is the half a void analog run cannot exercise on its own: every analog
/// crossing has weight exactly 1, so a replay that ignored the weight entirely
/// would pass the test above.
#[test]
fn a_replayed_crossing_carries_its_recorded_weight() {
    let geom = two_shell_void();
    let n = 4_000usize;

    let mut totals = Vec::new();
    for w in [1.0_f64, 0.25] {
        // One crossing per direction on a fixed spiral, so the two banks differ
        // only in weight and the comparison isolates it.
        let mut ss = SurfaceSource::recording(vec![0], 4 * n);
        for i in 0..n {
            let t = i as f64 / n as f64;
            let mu = 2.0 * t - 1.0;
            let phi = 6.283_185_307_179_586 * (t * 7.0).fract();
            let s = (1.0 - mu * mu).max(0.0).sqrt();
            let (ux, uy, uz) = (s * phi.cos(), s * phi.sin(), mu);
            // Sit exactly on the recording surface, moving outward.
            ss.record(SurfaceCrossing {
                r: Position::new(R1 * ux, R1 * uy, R1 * uz),
                u: outram_mc_libs::geometry::position::Direction::new(ux, uy, uz),
                energy: E0,
                weight: w,
                surface_idx: 0,
            });
        }
        let mut tally = shell_flux_tally();
        run_fixed_source(
            &geom,
            &[],
            &[],
            &FixedSource::Surface(Arc::new(ss)),
            &settings(n, 5_003),
            Some(&mut tally),
        );
        totals.push(tally.bins[0].sum);
    }

    let (full, quarter) = (totals[0], totals[1]);
    assert!(full > 0.0, "the weight-1 replay scored nothing");
    let ratio = quarter / full;
    println!("replay at weight 1.0 -> {full:.6e}; at 0.25 -> {quarter:.6e}; ratio {ratio:.6}");
    assert!(
        (ratio - 0.25).abs() < 1.0e-9,
        "a bank recorded at weight 0.25 gave {ratio} of the weight-1 answer, not 0.25. \
         The replay is discarding the recorded weight, which is exactly what makes a \
         two-stage run under variance reduction answer a rescaled problem."
    );
}

/// A bank that dropped crossings is refused as a source rather than replayed —
/// a truncated prefix is biased towards whatever the first histories did.
#[test]
fn a_truncated_bank_is_refused_rather_than_replayed() {
    let geom = two_shell_void();
    let mut ss = SurfaceSource::recording(vec![0], 10);
    for i in 0..50 {
        ss.record(SurfaceCrossing {
            r: Position::new(R1, 0.0, 0.0),
            u: outram_mc_libs::geometry::position::Direction::new(1.0, 0.0, 0.0),
            energy: E0,
            weight: 1.0,
            surface_idx: 0,
        });
        let _ = i;
    }
    assert!(ss.sample(&mut 1u64).is_err(), "a truncated bank must refuse");

    // And the refusal must reach a caller who tries to run with it, rather than
    // silently producing a biased answer.
    let replay = FixedSource::Surface(Arc::new(ss));
    let mut tally = shell_flux_tally();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_fixed_source(
            &geom,
            &[],
            &[],
            &replay,
            &settings(100, 7),
            Some(&mut tally),
        );
    }));
    assert!(
        res.is_err(),
        "running with a truncated bank completed instead of failing; a biased \
         second stage that looks converged is the failure mode this guards"
    );
}
