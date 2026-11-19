// SPDX-License-Identifier: AGPL-3.0-only
//! **The HTR-10 pour, pinned bit for bit** (gh:#1656).
//!
//! `htr10_fill` is being refactored onto a shared axisymmetric-vessel core
//! that `outram-mc-libs`' `pack_in_region` also uses. These fingerprints
//! were printed by the code BEFORE that refactor (commit 3c1372f65, 2026-11-19)
//! and must not move: the seed (`seed_loose`), and the centres and
//! velocities of a 600-pebble pour after 1500 steps on two threads.
//!
//! Method: FNV-1a 64 over `f64::to_bits` of every coordinate, in insertion
//! order. Pass criterion: equality. Run with `--nocapture` to print the
//! values (that is how the pins below were obtained).

use outram_park_fork_liggghts::compute::ThreadCount;
use outram_park_fork_liggghts::htr10_fill::{seed_loose, Htr10Fill, Htr10FillSettings};
use outram_park_fork_liggghts::particle::Vec3;

fn fnv(acc: &mut u64, x: f64) {
    for b in x.to_bits().to_le_bytes() {
        *acc ^= u64::from(b);
        *acc = acc.wrapping_mul(0x0000_0100_0000_01B3);
    }
}

fn fingerprint(v: &[Vec3]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for p in v {
        fnv(&mut h, p.x);
        fnv(&mut h, p.y);
        fnv(&mut h, p.z);
    }
    h
}

/// Printed before the refactor; see the module docs.
const SEED_3000_7: u64 = 0x0225_e92f_efc2_c433;
const POUR_600_CENTRES: u64 = 0x0955_920f_4a32_ef2e;
const POUR_600_VELOCITIES: u64 = 0x8168_d300_d57a_7a6e;

#[test]
fn the_htr10_seed_and_pour_are_unchanged() {
    let seed = fingerprint(&seed_loose(3000, 7));
    let mut fill = Htr10Fill::new(Htr10FillSettings { n_pebbles: 600, threads: ThreadCount::Fixed(2), ..Htr10FillSettings::default() }).expect("fill");
    fill.advance(1500);
    let centres = fingerprint(&fill.centres());
    let vel: Vec<Vec3> = fill.system().particles().iter().map(|p| p.velocity).collect();
    let velocities = fingerprint(&vel);
    println!("SEED_3000_7 = {seed:#018x}\nPOUR_600_CENTRES = {centres:#018x}\nPOUR_600_VELOCITIES = {velocities:#018x}");
    assert_eq!(seed, SEED_3000_7, "seed_loose moved");
    assert_eq!(centres, POUR_600_CENTRES, "pour centres moved");
    assert_eq!(velocities, POUR_600_VELOCITIES, "pour velocities moved");
}
