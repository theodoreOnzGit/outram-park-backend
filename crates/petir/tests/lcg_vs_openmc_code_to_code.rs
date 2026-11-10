// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! PETIR's `rng::lcg` against OpenMC's `src/random_lcg.cpp`, compiled and run
//! (gh:#1338, case 6).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_lcg/run.sh` compiles OpenMC's
//! `src/random_lcg.cpp` unmodified (commit `8f79d9d0`, MIT) with g++ -O2 and
//! `driver.cpp`, and writes `reference.txt` beside it. The driver prints, as
//! hex bit patterns:
//!
//! - the first 1000 `prn` draws from seeds 1, 42 and 2^63-1 (state and bits),
//! - `future_seed(n, seed)` for n in {0, 1, 2, 10, 1000, 2^20, 2^40, 2^62},
//! - `advance_prn_seed(n, seed)` for the same grid,
//! - `init_seed(id, offset)` under masters 1 and 42,
//! - for particle ids 0..15, the start state and an FNV-1a digest of 10^4
//!   draws, and FNV-1a digests of 10^6 sequential draws from each seed.
//!
//! Pass criterion: bit identity, no tolerance. The code under test is integer
//! arithmetic except the final conversion, which is one multiply by 2^-64 in
//! both codes, so a mismatch would be a real defect.
//!
//! **Not ported.** `future_prn`, `advance_prn_seed` and `init_particle_seeds`
//! have no PETIR function. The first two are tested here as the same
//! composition the C++ performs (`future_seed` then `prn`, and `future_seed`
//! into the state); they are NOT PETIR APIs. `init_particle_seeds` is not
//! compared, and its absence is recorded in the V&V README.
//!
//! # Results (2026-11-10)
//!
//! See the V&V README for counts and the toolchain. Every value compared
//! here is bit-identical.

use petir::rng::lcg::{future_seed, init_seed, prn, DEFAULT_STRIDE};

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_lcg/reference.txt");

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over the 8 little-endian bytes of each value, as in `driver.cpp`.
fn fnv_update(mut h: u64, v: u64) -> u64 {
    for b in 0..8 {
        h ^= (v >> (8 * b)) & 0xff;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Value of `key=` in a reference line, as text.
fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .unwrap_or_else(|| panic!("no field `{key}` in `{line}`"))
}

fn hex(line: &str, key: &str) -> u64 {
    u64::from_str_radix(field(line, key), 16)
        .unwrap_or_else(|e| panic!("bad hex `{key}` in `{line}`: {e}"))
}

fn dec(line: &str, key: &str) -> i64 {
    field(line, key)
        .parse()
        .unwrap_or_else(|e| panic!("bad int `{key}` in `{line}`: {e}"))
}

fn lines_with(prefix: &str) -> impl Iterator<Item = &'static str> + '_ {
    REFERENCE
        .lines()
        .filter(move |l| l.split_whitespace().next() == Some(prefix))
}

const SEEDS: [u64; 3] = [1, 42, (1u64 << 63) - 1];
const NS: [u64; 8] = [0, 1, 2, 10, 1000, 1 << 20, 1 << 40, 1 << 62];

/// `prn`: state and double bits for the first 1000 draws from each seed.
/// Bit-identical: 3000 of 3000 draws (state and bits).
#[test]
fn prn_matches_openmc_1000_draws_per_seed() {
    let mut checked = 0;
    for seed0 in SEEDS {
        let mut s = seed0;
        let rows: Vec<&str> = lines_with("prn")
            .filter(|l| dec(l, "seed") as u64 == seed0)
            .collect();
        assert_eq!(rows.len(), 1000, "reference rows for seed {seed0}");
        for row in rows {
            let x = prn(&mut s);
            assert_eq!(s, hex(row, "state"), "state, seed {seed0}, {row}");
            assert_eq!(x.to_bits(), hex(row, "bits"), "bits, seed {seed0}, {row}");
            checked += 1;
        }
    }
    assert_eq!(checked, 3000);
}

/// `future_seed`: every (n, seed) pair in the grid.
/// Bit-identical: 24 of 24 (8 strides x 3 seeds), including n = 2^62.
#[test]
fn future_seed_matches_openmc() {
    let rows: Vec<&str> = lines_with("future_seed").collect();
    assert_eq!(rows.len(), NS.len() * SEEDS.len());
    for row in rows {
        let n = dec(row, "n") as u64;
        let seed0 = dec(row, "seed") as u64;
        assert_eq!(future_seed(n, seed0), hex(row, "out"), "{row}");
    }
}

/// `init_seed(id, offset, master)`: 2 masters x 7 ids x 4 offsets = 56 values.
/// Bit-identical: 56 of 56. Drives the stride (`DEFAULT_STRIDE`) and offset
/// plumbing, which OpenMC reads from globals and PETIR takes as arguments.
#[test]
fn init_seed_matches_openmc() {
    assert_eq!(DEFAULT_STRIDE, 152_917);
    let rows: Vec<&str> = lines_with("init_seed").collect();
    assert_eq!(rows.len(), 2 * 7 * 4);
    for row in rows {
        let master = dec(row, "master");
        let id = dec(row, "id");
        let offset = dec(row, "offset");
        assert_eq!(init_seed(id, offset, master), hex(row, "out"), "{row}");
    }
}

/// `future_prn` is not a PETIR function; this checks the composition the C++
/// performs (`future_seed` then `prn`) against OpenMC's `future_prn`.
/// Bit-identical: 24 of 24.
#[test]
fn future_prn_composition_matches_openmc() {
    let rows: Vec<&str> = lines_with("future_prn").collect();
    assert_eq!(rows.len(), NS.len() * SEEDS.len());
    for row in rows {
        let n = dec(row, "n") as u64;
        let seed0 = dec(row, "seed") as u64;
        let mut fs = future_seed(n, seed0);
        let x = prn(&mut fs);
        assert_eq!(x.to_bits(), hex(row, "bits"), "{row}");
    }
}

/// `advance_prn_seed` is not a PETIR function; this checks the state update
/// it performs (`*seed = future_seed(n, *seed)`) against OpenMC's.
/// Bit-identical: 24 of 24.
#[test]
fn advance_prn_seed_composition_matches_openmc() {
    let rows: Vec<&str> = lines_with("advance").collect();
    assert_eq!(rows.len(), NS.len() * SEEDS.len());
    for row in rows {
        let n = dec(row, "n") as u64;
        let mut s = dec(row, "seed") as u64;
        s = future_seed(n, s);
        assert_eq!(s, hex(row, "out"), "{row}");
    }
}

/// Full streams with strides. Three checks against one OpenMC run:
///
/// 1. A single sequential stream of 10^6 draws from each seed, digested
///    draw by draw (bits), with the final state.
/// 2. Particle streams: `init_seed(id, 0, 1)` for ids 0..15 gives the start
///    state OpenMC gives, and each stream's first 10^4 draws digest to the
///    same value.
/// 3. The strides line up: walking ONE sequential stream from seed 1 with
///    `prn` and advancing exactly `DEFAULT_STRIDE` draws between particles
///    reproduces every `init_seed` start state. This needs no `future_seed`,
///    so it checks the jump-ahead against plain iteration.
///
/// Bit-identical on all three. Digests are FNV-1a 64 over the draws' bits.
#[test]
fn full_streams_and_strides_match_openmc() {
    // 1. Full sequential streams.
    for row in lines_with("digest_full") {
        let seed0 = dec(row, "seed") as u64;
        let mut s = seed0;
        let mut h = FNV_OFFSET;
        for _ in 0..1_000_000 {
            h = fnv_update(h, prn(&mut s).to_bits());
        }
        assert_eq!(h, hex(row, "digest"), "digest, {row}");
        assert_eq!(s, hex(row, "end"), "end state, {row}");
    }

    // 2. Particle streams from init_seed.
    let streams: Vec<&str> = lines_with("stream").collect();
    assert_eq!(streams.len(), 16);
    for row in &streams {
        let id = dec(row, "id");
        let mut s = init_seed(id, 0, 1);
        assert_eq!(s, hex(row, "start"), "start, {row}");
        let mut h = FNV_OFFSET;
        for _ in 0..10_000 {
            h = fnv_update(h, prn(&mut s).to_bits());
        }
        assert_eq!(h, hex(row, "digest_1e4"), "digest, {row}");
    }

    // 3. Strides against plain iteration of one stream.
    let mut walk = 1u64; // sequential stream from the default seed
    for (k, row) in streams.iter().enumerate() {
        assert_eq!(walk, hex(row, "start"), "walked start of stream {k}");
        for _ in 0..DEFAULT_STRIDE {
            prn(&mut walk);
        }
    }
}
