// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! PETIR's RANMAR and RANLUX against EGSnrc's own code, compiled and run
//! (gh:#1289).
//!
//! # Methodology
//!
//! `verification_and_validation/egsnrc_rndm/harness/` builds upstream
//! unmodified from EGSnrc commit `bd49a47a` (2026-09-28): `egs++/egs_rndm.cpp`
//! (RANMAR) with g++ -O3, and `src/ranlux.mortran` + `ranlux.macros`
//! (RANLUX) through EGSnrc's Mortran3 and gfortran with EGSnrc's shipped
//! flags (`-O3 -ffast-math -fPIC`). It writes `reference.txt` beside it:
//! per case, the exact bits of draws 0-47, 120-136, 990-1010 and the last
//! 10, FNV-1a-64 digests of the bits of the first 10^3, 10^5 and all draws
//! (10^6 per stream; 2x10^5 for the Gaussian and azimuth streams), the
//! final counters (RANMAR) or `get_ranlux_state` (RANLUX).
//!
//! `reference_streams_are_bit_identical` regenerates every stream with the
//! port and requires every recorded value and digest to match exactly: a
//! digest match over 10^6 draws is a bit-for-bit match of all of them (a
//! collision is ~2^-64). It also requires every uniform stream drawn through
//! PETIR's `Rng` as `random::<f64>()` to equal the `get_uniform` stream bit
//! for bit (the path ported EGSnrc code uses). Pass criterion: bit identity,
//! no tolerance.
//!
//! `full_dumps_are_bit_identical` (ignored; set `EGSNRC_RNDM_DUMP_DIR` to the
//! harness's output folder) compares every draw of every binary dump and
//! prints the table recorded in the V&V README.
//!
//! # Results (2026-11-05)
//!
//! 44 cases, 40.4 million draws: **every one bit-identical** (28 RANLUX
//! streams over all five luxury levels, 13 RANMAR streams in standard and
//! high resolution with array sizes 1, 97, 128 and 1000, the `getGaussian`
//! and `getAzimuth` streams, and the three state cases). Worst difference 0.

use std::fs;
use std::path::PathBuf;

use petir::rng::{Ranlux, Ranmar, RngExt};

/// One case of `reference.txt`.
#[derive(Debug, Default)]
struct Case {
    name: String,
    kind: String,
    params: Vec<(String, i64)>,
    values: Vec<(usize, u64)>,
    digests: Vec<(usize, u64)>,
    count: Option<(i64, i64)>,
    state: Option<Vec<i64>>,
    lines: Vec<(String, i64, i64)>,
}

impl Case {
    fn param(&self, key: &str) -> i64 {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("{}: no parameter {key}", self.name))
    }
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/egsnrc_rndm/reference.txt")
}

fn parse_reference() -> Vec<Case> {
    let p = reference_path();
    let text =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
    let mut cases = Vec::new();
    let mut cur: Option<Case> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "CASE" => {
                let mut c = Case {
                    name: f[1].to_string(),
                    kind: f[2].to_string(),
                    ..Case::default()
                };
                for kv in &f[3..] {
                    let (k, v) = kv.split_once('=').expect("key=value");
                    c.params.push((k.to_string(), v.parse().expect("integer")));
                }
                cur = Some(c);
            }
            "V" => cur.as_mut().expect("in a case").values.push((
                f[1].parse().expect("index"),
                u64::from_str_radix(f[2], 16).expect("hex"),
            )),
            "FNV" => cur.as_mut().expect("in a case").digests.push((
                f[1].parse().expect("count"),
                u64::from_str_radix(f[2], 16).expect("hex"),
            )),
            "COUNT" => {
                cur.as_mut().expect("in a case").count =
                    Some((f[1].parse().expect("gen"), f[2].parse().expect("used")))
            }
            "STATE" => {
                cur.as_mut().expect("in a case").state =
                    Some(f[1..].iter().map(|x| x.parse().expect("int")).collect())
            }
            "END" => cases.push(cur.take().expect("in a case")),
            other => cur.as_mut().expect("in a case").lines.push((
                other.to_string(),
                f[1].parse().expect("int"),
                f[2].parse().expect("int"),
            )),
        }
    }
    cases
}

/// The generator a case names.
enum Gen {
    Ranmar(Ranmar),
    Ranlux(Ranlux),
}

fn build(c: &Case) -> Gen {
    if c.kind == "ranlux" {
        // The not-initialised path of ranlux.mortran is the default.
        if c.param("uninit") == 1 {
            return Gen::Ranlux(Ranlux::default());
        }
        return Gen::Ranlux(Ranlux::new(c.param("lux") as i32, c.param("seed") as i32));
    }
    if c.params.iter().any(|(k, _)| k == "sequence") {
        return Gen::Ranmar(Ranmar::default_rng(c.param("sequence") as i32).expect("sequence"));
    }
    let mut r = Ranmar::with_array_size(
        c.param("ixx") as i32,
        c.param("jxx") as i32,
        c.param("n") as usize,
    )
    .expect("array size");
    r.set_high_resolution(c.param("hr") == 1);
    Gen::Ranmar(r)
}

/// The port's stream for a case: `n` draws of the case's kind.
fn stream(c: &Case, n: usize) -> (Gen, Vec<f64>) {
    let mut g = build(c);
    let mut xs = Vec::with_capacity(n);
    match (&mut g, c.kind.as_str()) {
        (Gen::Ranmar(r), "ranmar_gaussian") => xs.extend((0..n).map(|_| r.get_gaussian())),
        (Gen::Ranmar(r), "ranmar_azimuth") => {
            for _ in 0..n / 2 {
                let (cp, sp) = r.get_azimuth();
                xs.push(cp);
                xs.push(sp);
            }
        }
        (Gen::Ranmar(r), _) => xs.extend((0..n).map(|_| r.get_uniform())),
        (Gen::Ranlux(r), _) => xs.extend((0..n).map(|_| r.get_uniform())),
    }
    (g, xs)
}

/// The same uniform stream drawn through PETIR's `Rng` trait as
/// `random::<f64>()`, the path ported EGSnrc code (the alias table) uses.
fn stream_through_rng(c: &Case, n: usize) -> Vec<f64> {
    match build(c) {
        Gen::Ranmar(mut r) => (0..n).map(|_| r.random::<f64>()).collect(),
        Gen::Ranlux(mut r) => (0..n).map(|_| r.random::<f64>()).collect(),
    }
}

fn fnv1a(h: u64, x: f64) -> u64 {
    let mut h = h;
    for b in x.to_bits().to_le_bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The state cases: replay the harness's protocol and compare.
fn check_state_case(c: &Case) {
    let skip = c.param("skip") as usize;
    let after: Vec<u64>;
    let mut ok = true;
    if c.name == "ranlux_state_roundtrip" {
        let mut r = Ranlux::new(c.param("lux") as i32, c.param("seed") as i32);
        for _ in 0..skip {
            r.get_uniform();
        }
        let s = r.state();
        let mut st: Vec<i64> = s.seeds.iter().map(|&x| i64::from(x)).collect();
        st.push(i64::from(s.rng_seed));
        assert_eq!(Some(&st), c.state.as_ref(), "{}: state", c.name);
        after = (0..300).map(|_| r.get_uniform().to_bits()).collect();
        r.set_state(&s).expect("restore");
        let again: Vec<u64> = (0..300).map(|_| r.get_uniform().to_bits()).collect();
        ok &= again == after;
    } else {
        let mut r = Ranmar::with_array_size(1802, 9373, 128).expect("n");
        for _ in 0..skip {
            r.get_uniform();
        }
        r.save_state();
        after = (0..300).map(|_| r.get_uniform().to_bits()).collect();
        r.reset_state();
        let mut copy = r.get_copy();
        let reset: Vec<u64> = (0..300).map(|_| r.get_uniform().to_bits()).collect();
        let copied: Vec<u64> = (0..300).map(|_| copy.get_uniform().to_bits()).collect();
        ok &= reset == after && copied == after;
    }
    assert!(ok, "{}: replay after restore differs", c.name);
    for &(i, bits) in &c.values {
        assert_eq!(after[i], bits, "{}: value {i}", c.name);
    }
    // Upstream's own replay agreed with itself on all 300 (SAME lines).
    for (tag, same, of) in &c.lines {
        assert_eq!(same, of, "{}: upstream {tag}", c.name);
    }
}

#[test]
fn reference_streams_are_bit_identical() {
    let cases = parse_reference();
    assert_eq!(cases.len(), 44, "case count in reference.txt");
    let mut compared = 0usize;
    for c in &cases {
        if c.params.iter().any(|(k, _)| k == "skip") {
            check_state_case(c);
            continue;
        }
        let n = c.digests.iter().map(|d| d.0).max().expect("digests");
        let (g, xs) = stream(c, n);
        for &(i, bits) in &c.values {
            assert_eq!(xs[i].to_bits(), bits, "{}: value {i}", c.name);
        }
        let mut h = 0xcbf29ce484222325u64;
        for (k, &x) in xs.iter().enumerate() {
            h = fnv1a(h, x);
            if let Some(&(_, want)) = c.digests.iter().find(|d| d.0 == k + 1) {
                assert_eq!(h, want, "{}: digest of first {}", c.name, k + 1);
            }
        }
        compared += n;
        if c.kind == "ranmar" || c.kind == "ranlux" {
            let via = stream_through_rng(c, n);
            let same = via
                .iter()
                .zip(&xs)
                .filter(|(a, b)| a.to_bits() == b.to_bits())
                .count();
            assert_eq!(
                same, n,
                "{}: random::<f64>() differs from get_uniform",
                c.name
            );
        }
        match g {
            Gen::Ranmar(r) => {
                if let Some(count) = c.count {
                    assert_eq!(
                        (r.numbers_generated(), r.numbers_used()),
                        count,
                        "{}: counters",
                        c.name
                    );
                }
            }
            Gen::Ranlux(r) => {
                let s = r.state();
                let mut st: Vec<i64> = s.seeds.iter().map(|&x| i64::from(x)).collect();
                st.push(i64::from(s.rng_seed));
                assert_eq!(Some(&st), c.state.as_ref(), "{}: final state", c.name);
            }
        }
    }
    assert_eq!(compared, 40_400_000);
}

/// Every draw of every binary dump the harness wrote. Run with
/// `EGSNRC_RNDM_DUMP_DIR=<harness output> cargo test --release -p petir
/// --test egsnrc_rndm_vs_upstream -- --ignored --nocapture`.
#[test]
#[ignore = "needs the harness's binary dumps (EGSNRC_RNDM_DUMP_DIR)"]
fn full_dumps_are_bit_identical() {
    let dir = PathBuf::from(std::env::var("EGSNRC_RNDM_DUMP_DIR").expect("EGSNRC_RNDM_DUMP_DIR"));
    let mut all_same = true;
    println!("| case | compared | bit-identical | worst relative difference |");
    println!("|---|---|---|---|");
    for c in parse_reference() {
        let path = dir.join(format!("dump_{}.bin", c.name));
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let up: Vec<f64> = bytes
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().expect("8 bytes")))
            .collect();
        let (_, xs) = stream(&c, up.len());
        let mut same = 0usize;
        let mut worst = 0.0f64;
        for (a, b) in xs.iter().zip(&up) {
            if a.to_bits() == b.to_bits() {
                same += 1;
            } else {
                worst = worst.max((a - b).abs() / b.abs().max(f64::MIN_POSITIVE));
            }
        }
        all_same &= same == up.len();
        println!("| {} | {} | {} | {:e} |", c.name, up.len(), same, worst);
    }
    assert!(all_same);
}
