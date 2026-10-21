// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **Code-to-code: `petir::rng` against the Rand project's own crates, draw
//! for draw** (gh:#995).
//!
//! # Methodology
//!
//! `reference-data/rand/rand-0.10.3-reference.txt` was produced by compiling
//! and running `reference-data/rand/rand_reference_driver.rs` against the
//! exact crates boon-lay's RustBCA copy used before gh:#995 — `rand` 0.10.3
//! (`ChaCha8Rng` = `chacha20` 0.10.2) and `rand_distr` 0.6.0, built with
//! boon-lay's declared features (`rand`: `chacha`, `alloc`; `rand_distr`:
//! `alloc`; so `rand_distr`'s `exp`/`ln` are `libm`). See
//! `reference-data/rand/README.md` for the build and the platform.
//!
//! For 8 (seed, stream) cases — seeds 0, 1, 42, 876, 20261016 and
//! `u64::MAX`; streams 0, 876000001 and `u64::MAX` — this test replays, with
//! PETIR, every operation RustBCA performs and the raw ones under them:
//!
//! - 160 `next_u32`; 1 `next_u32` then 160 `next_u64` (crossing and
//!   straddling the 64-word refill);
//! - `fill_bytes` with lengths 1, 3, 5, 7, 32, 13, 64, 250, then one
//!   `next_u32` (partial words consumed whole);
//! - `from_rng`: a child seeded from its parent, 40 child `next_u64` and the
//!   parent's next;
//! - 160 `random::<f64>()`;
//! - 160 samples of `Uniform::new(lo, hi)` for 3 ranges;
//! - 160 samples of `Normal::new(mean, sd)` for 2 parameter sets;
//! - 160 steps of an interleaved cycle (u32, u64, f64, uniform, normal);
//!
//! and compares every value's bits. It then draws 1 000 000 values of each
//! of u32, u64, f64, uniform, standard normal and the interleaved cycle and
//! compares an FNV-1a digest of their bits with upstream's; the normal line
//! also carries upstream's count of tail draws (`|z| > 3.654`), which PETIR
//! must reproduce, so the tail branch is known to have run.
//!
//! **Pass criterion:** every value and every digest identical. No tolerance.
//!
//! # Results (2026-10-20, x86_64 Linux, release)
//!
//! **14 864 / 14 864 values bit-identical, 48 / 48 digests identical** (over
//! 48 000 000 draws: 6 methods x 8 cases x 10^6), and upstream's 2 017
//! tail draws reproduced. Run time 0.47 s. `RESULTS` below asserts those
//! counts, so a truncated reference cannot pass by comparing less.
//!
//! **The platform-libm build differs, and by how much.** Regenerating the
//! reference with `rand_distr/std_math` (its `exp`/`ln` on glibc, which is
//! what boon-lay's build actually ran, because `num-traits/std` was unified
//! on there) changes 2 of the 8 cases' normal and mixed digests. Dumping all
//! 8 x 10^6 standard-normal draws both ways: **9 draws differ, each by 1 ulp,
//! every one a tail draw** (`|z|` from 3.72 to 4.08, i.e. `ln` in
//! `zero_case`); no accept/reject decision flipped, so no later draw moved.
//! That is 9 of the 2 017 tail draws (0.45 %), 1.1e-6 of all draws. PETIR
//! matches the `libm` build exactly, as `rand_distr` declares itself.

use std::fs;
use std::path::PathBuf;

use petir::rng::{ChaCha8Rng, Normal, Rng, RngExt, SeedableRng, Uniform};

fn reference() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/rand/rand-0.10.3-reference.txt");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// FNV-1a over the 8 little-endian bytes of each value (as the driver).
struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn add(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

const ZIG_NORM_R: f64 = 3.654152885361008796;

fn rng_for(seed: u64, stream: u64) -> ChaCha8Rng {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    r.set_stream(stream);
    r
}

fn mixed_step(r: &mut ChaCha8Rng, k: usize, u: &Uniform, n: &Normal) -> u64 {
    match k % 5 {
        0 => u64::from(r.next_u32()),
        1 => r.next_u64(),
        2 => r.random::<f64>().to_bits(),
        3 => u.sample(r).to_bits(),
        _ => n.sample(r).to_bits(),
    }
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap_or_else(|e| panic!("bad hex {s:?}: {e}"))
}

/// Parse `<tag words...> <n> <v1> ... <vn>`, where the tag has `tag_words`
/// words; returns (tag tokens, values).
fn values<'a>(line: &'a str, tag_words: usize) -> (Vec<&'a str>, Vec<u64>) {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let n: usize = toks[tag_words].parse().unwrap();
    let vals: Vec<u64> = toks[tag_words + 1..].iter().map(|t| hex(t)).collect();
    assert_eq!(
        vals.len(),
        n,
        "count mismatch in {}",
        &line[..line.len().min(60)]
    );
    (toks[..tag_words].to_vec(), vals)
}

#[derive(Default, Debug)]
struct Tally {
    values: usize,
    digests: usize,
    long_draws: usize,
    tail_draws: usize,
}

fn check_case(seed: u64, stream: u64, lines: &[&str], t: &mut Tally) {
    let ctx = format!("seed {seed} stream {stream}");
    let u_mixed = Uniform::new(-2.5, 7.25).unwrap();
    let n_std = Normal::new(0.0, 1.0).unwrap();
    for line in lines {
        let tag = line.split_whitespace().next().unwrap();
        let (got, want): (Vec<u64>, Vec<u64>) = match tag {
            "u32" => {
                let (_, want) = values(line, 1);
                let mut r = rng_for(seed, stream);
                (
                    (0..want.len()).map(|_| u64::from(r.next_u32())).collect(),
                    want,
                )
            }
            "u32+u64" => {
                let (_, want) = values(line, 1);
                let mut r = rng_for(seed, stream);
                let mut v = vec![u64::from(r.next_u32())];
                v.extend((1..want.len()).map(|_| r.next_u64()));
                (v, want)
            }
            "bytes" => {
                let (_, want) = values(line, 1);
                let mut r = rng_for(seed, stream);
                let mut v = Vec::new();
                for len in [1usize, 3, 5, 7, 32, 13, 64, 250] {
                    let mut buf = vec![0u8; len];
                    r.fill_bytes(&mut buf);
                    v.extend(buf.iter().map(|b| u64::from(*b)));
                }
                v.push(u64::from(r.next_u32()));
                (v, want)
            }
            "from_rng" => {
                let (_, want) = values(line, 1);
                let mut parent = rng_for(seed, stream);
                let mut child = ChaCha8Rng::from_rng(&mut parent);
                let mut v: Vec<u64> = (1..want.len()).map(|_| child.next_u64()).collect();
                v.push(parent.next_u64());
                (v, want)
            }
            "f64" => {
                let (_, want) = values(line, 1);
                let mut r = rng_for(seed, stream);
                (
                    (0..want.len())
                        .map(|_| r.random::<f64>().to_bits())
                        .collect(),
                    want,
                )
            }
            "uniform" => {
                let (tag, want) = values(line, 3);
                let u =
                    Uniform::new(f64::from_bits(hex(tag[1])), f64::from_bits(hex(tag[2]))).unwrap();
                let mut r = rng_for(seed, stream);
                (
                    (0..want.len())
                        .map(|_| u.sample(&mut r).to_bits())
                        .collect(),
                    want,
                )
            }
            "normal" => {
                let (tag, want) = values(line, 3);
                let n =
                    Normal::new(f64::from_bits(hex(tag[1])), f64::from_bits(hex(tag[2]))).unwrap();
                let mut r = rng_for(seed, stream);
                (
                    (0..want.len())
                        .map(|_| n.sample(&mut r).to_bits())
                        .collect(),
                    want,
                )
            }
            "mixed" => {
                let (_, want) = values(line, 1);
                let mut r = rng_for(seed, stream);
                (
                    (0..want.len())
                        .map(|k| mixed_step(&mut r, k, &u_mixed, &n_std))
                        .collect(),
                    want,
                )
            }
            "digest" => {
                let toks: Vec<&str> = line.split_whitespace().collect();
                let what = toks[1];
                let n: usize = toks[2].parse().unwrap();
                let want = hex(toks[3]);
                let mut r = rng_for(seed, stream);
                let mut h = Fnv::new();
                match what {
                    "u32" => (0..n).for_each(|_| h.add(u64::from(r.next_u32()))),
                    "u64" => (0..n).for_each(|_| h.add(r.next_u64())),
                    "f64" => (0..n).for_each(|_| h.add(r.random::<f64>().to_bits())),
                    "uniform" => (0..n).for_each(|_| h.add(u_mixed.sample(&mut r).to_bits())),
                    "normal" => {
                        let want_tail: usize = toks[4].parse().unwrap();
                        let mut tail = 0usize;
                        for _ in 0..n {
                            let z = n_std.sample(&mut r);
                            if z.abs() > ZIG_NORM_R {
                                tail += 1;
                            }
                            h.add(z.to_bits());
                        }
                        assert_eq!(tail, want_tail, "{ctx}: tail count");
                        t.tail_draws += tail;
                    }
                    "mixed" => (0..n).for_each(|k| h.add(mixed_step(&mut r, k, &u_mixed, &n_std))),
                    other => panic!("unknown digest {other}"),
                }
                assert_eq!(
                    h.0, want,
                    "{ctx}: digest {what} over {n} draws differs from upstream"
                );
                t.digests += 1;
                t.long_draws += n;
                continue;
            }
            other => panic!("unknown record {other}"),
        };
        for (k, (g, w)) in got.iter().zip(want.iter()).enumerate() {
            assert_eq!(
                g, w,
                "{ctx}: {tag} draw {k}: petir {g:016x}, upstream {w:016x}"
            );
        }
        t.values += want.len();
    }
}

/// Every value and every digest of the reference, replayed with PETIR.
#[test]
fn petir_rng_streams_equal_rand_draw_for_draw() {
    let text = reference();
    let mut cases: Vec<(u64, u64, Vec<&str>)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("case ") {
            let mut it = rest.split_whitespace();
            cases.push((hex(it.next().unwrap()), hex(it.next().unwrap()), Vec::new()));
        } else {
            cases
                .last_mut()
                .expect("record before any case")
                .2
                .push(line);
        }
    }
    assert_eq!(cases.len(), 8, "expected 8 cases in the reference");
    let mut t = Tally::default();
    for (seed, stream, lines) in &cases {
        check_case(*seed, *stream, lines, &mut t);
    }
    eprintln!("rng_vs_rand: {t:?}");
    assert_eq!(t.values, RESULTS.values);
    assert_eq!(t.digests, RESULTS.digests);
    assert_eq!(t.tail_draws, RESULTS.tail_draws);
}

/// What the 2026-10-20 run compared, asserted so a truncated reference file
/// cannot pass by comparing less.
struct Expected {
    values: usize,
    digests: usize,
    tail_draws: usize,
}
const RESULTS: Expected = Expected {
    values: 14_864,
    digests: 48,
    tail_draws: 2_017,
};
