//! **Cubic TRISO array clipped to the HTR-10 fuel zone** — `bn:op-867c.11`, gh #214.
//!
//! The RMC benchmark specifies a cubic array embedded in the fuel sphere,
//! keeping only whole particles, "the number verified to be 8335".
//!
//! # Results (2026-09-17) — 8335 exactly is NOT attainable
//!
//! Sweeping the pitch over ±3 % of the continuum estimate, for HTR-10's zone
//! (ball 2.5 cm, particle 0.0455 cm):
//!
//! | offset | best count | error vs 8335 |
//! |---|---|---|
//! | `[0.5, 0, 0]` | **8330** | −0.060 % |
//! | `[0.5, 0.5, 0]` | **8340** | +0.060 % |
//! | `[0, 0, 0]` | 8289 / 8385 | −0.55 % / +0.60 % |
//!
//! The count moves in **symmetry shells** — whole orbits of lattice points
//! cross the boundary together — so it jumps, e.g. 8336 → 8240 in one step for
//! the half-offset arrangement. 8335 falls in a gap for every offset tried.
//!
//! **Interpretation.** ±0.060 % in particle count is ±0.060 % in fuel volume
//! and packing fraction (0.050218 or 0.050278 against the 0.050248 implied by
//! 8335). Negligible for `k`, but it means the paper's arrangement is not
//! exactly this one — its zone radius, particle radius, or rejection rule must
//! differ somewhere in the last digit. Stated rather than rounded away, because
//! a reader comparing our geometry to theirs needs to know it.

use outram_mc_libs::pebble_beds::sphere_packing::{cubic_array_in_ball, cubic_pitch_for_count};

const R_ZONE: f64 = 2.5;
const R_PART: f64 = 0.0455; // TrisoSpec::HTR10_LI2014 / TrisoRadii::HTR10 (adjudicated)
const STATED: usize = 8335;

/// Every particle kept must lie **wholly** inside the ball — the benchmark's
/// own rule, and the one that makes the count meaningful.
#[test]
fn every_kept_particle_lies_wholly_inside() {
    let (pitch, _) = cubic_pitch_for_count(R_PART, R_ZONE, STATED, [0.5, 0.5, 0.0]);
    let arr = cubic_array_in_ball(R_PART, R_ZONE, pitch, [0.5, 0.5, 0.0]);
    assert!(!arr.is_empty());
    for s in &arr {
        assert!(
            s.center.norm() + R_PART <= R_ZONE + 1.0e-12,
            "particle at {:?} pokes out of the fuel zone",
            s.center
        );
        assert!((s.radius - R_PART).abs() < 1.0e-15);
    }
    // And no two particles overlap: a cubic array with pitch > 2r cannot, but
    // that is worth asserting rather than assuming, since pitch is a parameter.
    assert!(
        pitch > 2.0 * R_PART,
        "pitch {pitch:.6} must exceed one particle diameter {:.6}",
        2.0 * R_PART
    );
}

/// **The headline finding: the stated 8335 is not reachable.**
///
/// This test asserts the *gap*, not a lucky hit. If a future change makes 8335
/// attainable it will fail, which is the correct outcome — that would mean the
/// arrangement changed and the discrepancy note needs revisiting.
#[test]
fn the_stated_particle_count_is_not_attainable_and_here_is_how_close() {
    let offsets: [[f64; 3]; 4] = [
        [0.0, 0.0, 0.0],
        [0.5, 0.0, 0.0],
        [0.5, 0.5, 0.0],
        [0.5, 0.5, 0.5],
    ];
    let mut best_err = usize::MAX;
    println!(
        "{:<20} {:>8} {:>10} {:>10}",
        "offset", "count", "err", "pitch"
    );
    for off in offsets {
        let (pitch, count) = cubic_pitch_for_count(R_PART, R_ZONE, STATED, off);
        let err = count.abs_diff(STATED);
        println!(
            "{:<20} {count:>8} {:>+10} {pitch:>10.6}",
            format!("{off:?}"),
            count as i64 - STATED as i64
        );
        assert_ne!(
            count, STATED,
            "offset {off:?} reached exactly {STATED}. That contradicts the recorded \
             finding that the count moves in symmetry shells and {STATED} falls in a \
             gap -- revisit the discrepancy note in cubic_array_in_ball's docs."
        );
        best_err = best_err.min(err);
    }
    println!(
        "closest approach: {best_err} particles ({:.3} %)",
        100.0 * best_err as f64 / STATED as f64
    );
    assert!(
        best_err <= 10,
        "the closest attainable count should be within ~10 particles of {STATED}; \
         got {best_err}. A larger gap means the pitch search or the rejection rule \
         has changed."
    );
}

/// The packing fraction that follows must match the value
/// `TrisoSpec::HTR10_LI2014` carries, to the same ±0.060 % the count is off by.
#[test]
fn the_realised_packing_fraction_matches_the_spec_within_the_count_error() {
    const PF_SPEC: f64 = 0.050_248_114; // derived from 8335 in a 2.5 cm zone
    let (pitch, count) = cubic_pitch_for_count(R_PART, R_ZONE, STATED, [0.5, 0.5, 0.0]);
    let pf = count as f64 * R_PART.powi(3) / R_ZONE.powi(3);
    let rel = (pf / PF_SPEC - 1.0) * 100.0;
    println!("pitch {pitch:.6} cm, {count} particles, pf {pf:.9} ({rel:+.3} % vs spec)");
    assert!(
        rel.abs() < 0.1,
        "realised packing fraction {pf:.9} is {rel:+.3} % from the spec's \
         {PF_SPEC:.9}; the count error alone should account for under 0.1 %"
    );
}
