// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # The direction-norm guard of the rotation kernels (gh:#1497)
//!
//! ## What failed
//!
//! `nee_soon`'s `htr10_single_pebble_vs_dh_universe` panicked in `NuBar::at`
//! on a NaN energy. Instrumenting the DH arm (`keff_delta`, HTR-10 pebble,
//! graphite S(alpha,beta) by LEAPR, 4000 x \[10 + 50\], seed 20261013) found
//! the NaN's origin on 2026-11-11:
//!
//! - every C-12 collision below the S(alpha,beta) cutoff rotates the
//!   direction with `rotate_angle` and nothing ever rebuilds it from a
//!   velocity, so the direction's norm error `eps = |u|^2 - 1` is never reset;
//! - `rotate_angle` multiplies `eps` by about `1 + a^2/b^2`
//!   (`a = sqrt(1 - mu^2)`, `b = sqrt(1 - w^2)`). The instrumented run logged
//!   that gain at 10 to 1000 on directions with `|w|` = 0.97 to 0.9999, e.g.
//!   `eps` -8.3e-11 -> -5.4e-9 in one C-12 collision;
//! - within one history `|u|` reached 2.9 (`u = (1.441, 1.230, -2.224)`).
//!   With `w^2 > 1` the near-pole branch takes `sqrt(1 - v^2)` with `|v| > 1`,
//!   so the direction is NaN; the next free-gas elastic collision builds the
//!   neutron velocity from it and the energy is NaN.
//!
//! OpenMC's `rotate_angle` (`src/math_functions.cpp:772`) and `sab_scatter`
//! (`src/physics.cpp:871`) never renormalise either: the exposure is
//! upstream's, and the legacy `rotate_direction` has the same main branch.
//! The guard (`physics::scatter::renormalise_if_drifted`, new work) leaves a
//! direction alone while `| |u|^2 - 1 |` is within `sqrt(f64::EPSILON)` and
//! renormalises it otherwise, on the input and the output of both kernels.
//!
//! ## Methodology and pass criteria
//!
//! 1. **The recorded case.** The exact direction the failing history carried
//!    into its last collision: the unguarded upstream kernel returns NaN (this
//!    pins the mechanism); the guarded kernels return a finite unit vector.
//! 2. **The amplification.** A direction 1e-9 off unit length, 1e-4 rad from
//!    the pole, rotated by `mu = 0`: unguarded, `eps` grows past the
//!    tolerance; guarded, the output is within it.
//! 3. **A long walk.** 1e6 guarded rotations by uniform cosines through the
//!    LCG: every direction finite and within the tolerance.
//! 4. **Bit-identity on healthy directions.** 2e5 rotations of a walk whose
//!    norm stays healthy: guarded and unguarded outputs (same seed) identical
//!    bit for bit, for both kernels, so the guard moves no pinned stream.
//!
//! ## Results (2026-11-11, Rust 1.99.1, release)
//!
//! Before the guard, cases 1 and 2 fail (NaN, and `eps` = O(1)); after it all
//! four pass, and case 4 finds 0 differing outputs of 200 000 per kernel.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::physics::scatter::{
    renormalise_if_drifted, rotate_angle_openmc, rotate_angle_openmc_unguarded, rotate_direction,
    rotate_direction_unguarded, DIRECTION_NORM_DRIFT_TOL,
};
use outram_mc_libs::rng::lcg::prn;

fn eps(u: Direction) -> f64 {
    u.u * u.u + u.v * u.v + u.w * u.w - 1.0
}

fn finite(u: Direction) -> bool {
    u.u.is_finite() && u.v.is_finite() && u.w.is_finite()
}

/// Case 1: the direction recorded on the failing history (gh:#1497).
#[test]
fn the_recorded_drifted_direction_no_longer_gives_nan() {
    let u = Direction::new(1.4411021851706467, 1.2302772951790852, -2.2244624822013663);
    let mu = 0.45437094438762427;
    let mut s = 0x1497_u64;
    let raw = rotate_angle_openmc_unguarded(u, mu, None, &mut s.clone());
    assert!(!finite(raw), "the mechanism: upstream's rotate_angle gives NaN here, got {raw:?}");
    for out in [
        rotate_angle_openmc(u, mu, None, &mut s),
        rotate_direction(u, mu, &mut 0x1497_u64),
    ] {
        assert!(finite(out), "guarded rotation must be finite, got {out:?}");
        assert!(eps(out).abs() <= DIRECTION_NORM_DRIFT_TOL, "|u|^2 - 1 = {:e}", eps(out));
    }
}

/// Case 2: a near-pole rotation amplifies a 1e-9 norm error.
#[test]
fn a_near_pole_rotation_cannot_amplify_the_norm_error() {
    let b = 1.0e-4_f64;
    let w = -(1.0 - b * b).sqrt();
    let k = (1.0 + 1.0e-9_f64).sqrt();
    let u = Direction::new(b * k, 0.0, w * k);
    let e_in = eps(u);
    assert!(e_in.abs() > 5e-10 && e_in.abs() < DIRECTION_NORM_DRIFT_TOL, "input eps {e_in:e}");
    let raw = rotate_angle_openmc_unguarded(u, 0.0, Some(0.3), &mut 1);
    assert!(eps(raw).abs() > DIRECTION_NORM_DRIFT_TOL, "the mechanism: eps {e_in:e} -> {:e}", eps(raw));
    let raw_legacy = rotate_direction_unguarded(u, 0.0, &mut 1);
    assert!(eps(raw_legacy).abs() > DIRECTION_NORM_DRIFT_TOL, "legacy kernel: {:e}", eps(raw_legacy));
    let g = rotate_angle_openmc(u, 0.0, Some(0.3), &mut 1);
    let gl = rotate_direction(u, 0.0, &mut 1);
    for out in [g, gl] {
        assert!(eps(out).abs() <= DIRECTION_NORM_DRIFT_TOL, "guarded eps {:e}", eps(out));
    }
}

/// Case 3: a long guarded walk stays finite and unit.
#[test]
fn a_long_guarded_walk_stays_unit() {
    let mut seed = 20_261_111_u64;
    let mut u = Direction::new(0.0, 0.6, 0.8);
    let mut uo = u;
    for i in 0..1_000_000 {
        let mu = 2.0 * prn(&mut seed) - 1.0;
        u = rotate_angle_openmc(u, mu, None, &mut seed);
        uo = rotate_direction(uo, mu, &mut seed);
        assert!(finite(u) && eps(u).abs() <= DIRECTION_NORM_DRIFT_TOL, "step {i}: {u:?}");
        assert!(finite(uo) && eps(uo).abs() <= DIRECTION_NORM_DRIFT_TOL, "step {i}: {uo:?}");
    }
}

/// Case 4: on healthy directions the guard is a no-op, bit for bit.
#[test]
fn the_guard_is_bit_identical_on_healthy_directions() {
    let mut seed = 77_u64;
    let mut u = Direction::new(0.0, 0.0, 1.0);
    let mut diff = 0usize;
    for _ in 0..200_000 {
        let mu = 2.0 * prn(&mut seed) - 1.0;
        let (mut s1, mut s2) = (seed, seed);
        let a = rotate_angle_openmc(u, mu, None, &mut s1);
        let b = rotate_angle_openmc_unguarded(u, mu, None, &mut s2);
        let (mut s3, mut s4) = (seed, seed);
        let c = rotate_direction(u, mu, &mut s3);
        let d = rotate_direction_unguarded(u, mu, &mut s4);
        if a != b || s1 != s2 || c != d || s3 != s4 {
            diff += 1;
        }
        assert_eq!(renormalise_if_drifted(b), b);
        u = b;
        seed = s2;
    }
    assert_eq!(diff, 0, "the guard fired on a healthy direction");
}
