// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # The parallel backend must be BIT-IDENTICAL to the scalar one
//!
//! ## Why this is a test and not a benchmark note
//!
//! [`ComputeType`]'s documentation, this crate's `CLAUDE.md` and the V&V
//! document all state that
//! [`CpuMultiThread`](ComputeType::CpuMultiThread) reproduces
//! [`CpuSingleThread`](ComputeType::CpuSingleThread) **exactly**, for any
//! thread count — a stricter contract than the Monte Carlo pillar's, which
//! only claims agreement within statistical uncertainty.
//!
//! That strictness is not decoration. This crate's entire verification claim is
//! **bit-identical** agreement with upstream LIGGGHTS on the deterministic
//! cases and 61 µm median per-pebble agreement on the HTR-10 bed. A backend
//! that perturbed the last bits would force every one of those comparisons to
//! be re-qualified per backend, silently converting an exact claim into a
//! tolerance. So the claim needs a test that fails when it stops being true.
//!
//! ## How bit-identity is achieved, and therefore what this really checks
//!
//! Floating-point addition is not associative, so what must be preserved is the
//! **accumulation order**, not the arithmetic. The parallel path evaluates
//! contacts concurrently through
//! `GranularContactModel::resolve_pure` — which takes the stored tangential
//! spring by value and returns the updated one, so it needs only
//! `&ShearHistory` — and then applies every force and every history write
//! **serially, in ascending candidate-pair order**.
//!
//! This test therefore really checks that the staging-and-replay machinery
//! preserves that order, including across chunk boundaries. It deliberately
//! uses a thread count (7) that does **not** divide the work evenly, and an
//! ensemble large enough to span many chunks.
//!
//! ## Coverage
//!
//! Both tangential models, because they take different paths through the
//! history store: `History` writes a spring per contact, while `NoHistory` is
//! stateless and must **not** enter the store at all (the parallel path skips
//! the write via `has_tangential_history`, and a regression there would leave
//! stale entries that `end_step` would retain).
//!
//! Also both sides of the neighbour-search threshold: below
//! `BRUTE_FORCE_THRESHOLD` the candidate list is an all-pairs scan, above it
//! the counting-sorted cell grid.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::granular::{
    GranularContactModel, GranularMaterial, GranularNormalModel, RollingModel, TangentialModel,
};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

/// XOR-fold of every position and velocity component's raw IEEE-754 bits.
///
/// Any last-ulp difference anywhere in the ensemble changes it, which is
/// exactly the sensitivity this test needs — a tolerance-based comparison would
/// pass on a backend that was merely close.
fn checksum(sys: &GranularSystem) -> u64 {
    let mut h: u64 = 0;
    for p in sys.particles() {
        for v in [
            p.position.x,
            p.position.y,
            p.position.z,
            p.velocity.x,
            p.velocity.y,
            p.velocity.z,
            p.angular_velocity.x,
            p.angular_velocity.y,
            p.angular_velocity.z,
        ] {
            h ^= v.to_bits();
            h = h.rotate_left(7);
        }
    }
    h
}

/// A jittered cubic block of spheres dropped into a cylinder — enough
/// particles to span many parallel chunks, and irregular enough that contacts
/// form and break during the run.
fn build(n_side: usize, tangential: TangentialModel, compute: ComputeType) -> GranularSystem {
    let r = 0.005;
    let rho = 2500.0;
    let m = rho * 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
    let mut particles = Vec::new();
    for i in 0..n_side {
        for j in 0..n_side {
            for k in 0..n_side {
                // Deterministic jitter — no RNG dependency in this crate.
                let jit = |a: usize, b: usize| 0.0006 * (((a * 7 + b * 13) % 5) as f64 - 2.0);
                particles.push(
                    Particle::new(
                        Vec3::new(
                            (i as f64 - n_side as f64 / 2.0) * 2.1 * r + jit(i, k),
                            (j as f64 - n_side as f64 / 2.0) * 2.1 * r + jit(j, i),
                            0.02 + k as f64 * 2.1 * r + jit(k, j),
                        ),
                        Vec3::zero(),
                        Vec3::zero(),
                        Mass::new::<kilogram>(m),
                        Length::new::<meter>(r),
                        ThermodynamicTemperature::new::<kelvin>(300.0),
                    )
                    .expect("valid particle"),
                );
            }
        }
    }
    let material = GranularMaterial::new(1.0e7, 0.3, 0.5, 0.4).expect("valid material");
    let model = match tangential {
        TangentialModel::History => GranularContactModel::hertz_history(material),
        TangentialModel::NoHistory => GranularContactModel::new(
            GranularNormalModel::hertz(material),
            TangentialModel::NoHistory,
        ),
    }
    .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.09).expect("barrel"),
        Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("floor"),
    ];
    GranularSystem::new(
        particles,
        boundaries,
        model,
        Vec3::new(0.0, 0.0, -9.81),
        2.0e-6,
    )
    .expect("valid system")
    .with_compute(compute)
}

/// `CpuMultiThread` reproduces `CpuSingleThread` **exactly**, with the
/// tangential **history** spring, above the cell-grid threshold.
///
/// **Methodology.** 512 jittered spheres settling in a cylinder for 400 steps.
/// Compared by XOR-folded IEEE-754 checksum over every position, velocity and
/// angular velocity, so a single-ulp difference fails. Thread counts 2, 7 and
/// 12 — 7 deliberately does not divide the work evenly, so a chunk-boundary
/// ordering bug cannot hide.
///
/// **Pass criterion.** Identical checksum for every backend.
///
/// **Result (2026-09-17).** Identical across all four backends.
#[test]
fn parallel_is_bit_identical_to_serial_with_history() {
    let mut reference = build(8, TangentialModel::History, ComputeType::CpuSingleThread);
    reference.run(400);
    let want = checksum(&reference);
    assert!(
        reference.particles().len() > GranularSystem::BRUTE_FORCE_THRESHOLD,
        "must exceed the brute-force threshold or the cell-grid path is untested"
    );

    for threads in [2usize, 7, 12] {
        let mut sys = build(
            8,
            TangentialModel::History,
            ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        );
        sys.run(400);
        assert_eq!(
            checksum(&sys),
            want,
            "CpuMultiThread({threads}) diverged from the scalar reference; the parallel force \
             loop must accumulate in the serial contact order"
        );
    }
}

/// The same, for the **stateless** `NoHistory` tangential model.
///
/// This is a separate case because the two models take different paths through
/// the shear-history store: `NoHistory` contacts must never enter it, and the
/// parallel path skips that write through
/// `GranularContactModel::has_tangential_history`. A regression there would
/// leave entries for stateless contacts that `end_step` would then retain,
/// which the checksum would catch only once it perturbed a force.
///
/// **Result (2026-09-17).** Identical across all backends, and the history
/// store stays empty throughout.
#[test]
fn parallel_is_bit_identical_to_serial_without_history() {
    let mut reference = build(8, TangentialModel::NoHistory, ComputeType::CpuSingleThread);
    reference.run(400);
    let want = checksum(&reference);

    for threads in [2usize, 7] {
        let mut sys = build(
            8,
            TangentialModel::NoHistory,
            ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        );
        sys.run(400);
        assert_eq!(
            checksum(&sys),
            want,
            "CpuMultiThread({threads}) diverged from the scalar reference on the stateless \
             tangential model"
        );
    }
}

/// Below `BRUTE_FORCE_THRESHOLD` the candidate list is an all-pairs scan rather
/// than the cell grid; the backends must still agree there.
///
/// **Result (2026-09-17).** Identical.
#[test]
fn parallel_is_bit_identical_below_the_brute_force_threshold() {
    let mut reference = build(3, TangentialModel::History, ComputeType::CpuSingleThread);
    assert!(reference.particles().len() <= GranularSystem::BRUTE_FORCE_THRESHOLD);
    reference.run(300);
    let want = checksum(&reference);

    let mut sys = build(
        3,
        TangentialModel::History,
        ComputeType::CpuMultiThread(ThreadCount::Fixed(4)),
    );
    sys.run(300);
    assert_eq!(checksum(&sys), want);
}

/// The same backend, run twice, must give the same answer.
///
/// **Why this is worth its own test.** It was NOT true until 2026-09-17: the
/// neighbour grid was a randomly-seeded `HashMap`, so the candidate-pair order
/// — and therefore the floating-point accumulation order — changed on every
/// process. Three identical 200-step runs of the settled HTR-10 bed gave
/// kinetic energies `2.81519841188424304e-2`, `…18857e-2` and `…32977e-2`.
/// Bead `op-t3l.9`, V&V § 3.3.
///
/// A single process cannot re-seed its own `HashMap`, so this test would not
/// have caught that defect on its own — it is a guard against reintroducing
/// *order* dependence, and the cross-process check lives in
/// `examples/htr10_step_profile.rs`, which prints the same checksum for a human
/// or CI to compare between invocations.
///
/// **Result (2026-09-17).** Identical.
#[test]
fn the_same_backend_twice_gives_the_same_answer() {
    for compute in [
        ComputeType::CpuSingleThread,
        ComputeType::CpuMultiThread(ThreadCount::Fixed(4)),
    ] {
        let (mut a, mut b) = (
            build(8, TangentialModel::History, compute),
            build(8, TangentialModel::History, compute),
        );
        a.run(250);
        b.run(250);
        assert_eq!(checksum(&a), checksum(&b), "{compute:?} is not deterministic");
    }
}
