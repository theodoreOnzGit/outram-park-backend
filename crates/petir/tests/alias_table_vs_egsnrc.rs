// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **`rng::alias_table` against EGSnrc's own `egs_alias_table.cpp`, compiled
//! and run** (gh:#1289).
//!
//! # Methodology
//!
//! `verification_and_validation/egsnrc_alias_table/harness.cpp` links
//! upstream's unmodified `egs_alias_table.cpp` (EGSnrc commit
//! `bd49a47aae3938f49f4100b8e018091861408b19`, built with
//! `g++ -O2 -ffp-contract=off`) and drives it with a replay
//! `EGS_RandomGenerator` returning a fixed list of uniforms `k * 2^-53`. This
//! file builds the same 23 tables from the same input recipes and replays the
//! same list through PETIR's `Rng` trait ([`Replay`]: `next_u64 = k << 11`,
//! so `random::<f64>()` is exactly `k * 2^-53`).
//!
//! Cases: discrete, histogram and linear tables, each uniform, steep
//! (`2^-i`), with zeros, a single bin and 10^4 bins; a type-3 table; two
//! function-driven tables (one meeting `accu`, one stopped at `nmax`); and
//! `EGS_SimpleAliasTable` uniform, steep, with zeros, single and 10^4 bins.
//! Per alias table: every table entry, 10^5 `sample()` values and 10^5
//! `sampleBin()` indices (the stream restarted). Per simple table: every
//! entry and 10^5 `sample()` indices.
//!
//! **Pass criterion: bit-identity.** [`digests_match_upstream`] compares
//! FNV-1a digests of every table entry and every sample against
//! `reference_summary.txt` (the harness's output, committed), plus the full
//! `wi`/`bin` arrays of every table with at most 40 bins and the first 16
//! samples, as exact hex. [`full_dump_matches_upstream_entry_by_entry`]
//! compares the harness's full dumps value by value when
//! `EGS_ALIAS_DUMP_DIR` points at them (69 MB, not committed); it is how the
//! table in the V&V record was produced.
//!
//! [`sampled_frequencies_pass_chi_squared`] is the statistical check: the
//! same 10^5 samples, binned (bins for discrete and simple tables; four
//! sub-intervals per bin for histogram and linear values), against the
//! input distribution's exact cell probabilities, cells pooled in order
//! until each expects at least 5, at a fixed significance `alpha = 0.001`.
//!
//! # Results (2026-11-05, x86-64, glibc g++ 15 / Rust 1.99.1)
//!
//! Prediction written before running: bit-identical everywhere. Measured:
//! **bit-identical everywhere** -- 23/23 cases, every table entry and all
//! 4.1 million samples and bins; the one NaN (`linear_zeros`' average, upstream's
//! `0*(..)/0` for a bin zero at both edges) has the same bits
//! `0xfff8000000000000` on both sides. All 36 chi-squared tests pass at
//! `alpha = 0.001`; see the V&V record for the table.
//!
//! One upstream behaviour the cases expose, kept by the port: a bin with
//! zero probability is returned when `r1 * np` is exactly the bin index
//! (`aj = 0`, and `0 > wi[j] = 0` is false). The stream's leading `0.0`
//! triggers it once in `discrete_zeros` and `histogram_zeros`; the
//! chi-squared test accounts for those draws explicitly
//! ([`exact_integer_hits`]).

use petir::{erfc, gamma_inc_p};
use petir::rng::{AccuracyGoal, AliasTableKind, EgsAliasTable, EgsSimpleAliasTable, Rng, RngExt};
use std::collections::HashMap;

const NSAMPLE: usize = 100_000;
const STREAM_LEN: usize = 250_000;
const ALPHA: f64 = 1e-3;

const REFERENCE: &str =
    include_str!("../verification_and_validation/egsnrc_alias_table/reference_summary.txt");

// ------------------------------------------------------------------ recipes

fn splitmix64(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The harness's uniform stream, as `k` in `k * 2^-53`.
fn uniform_stream() -> Vec<u64> {
    let top = (1u64 << 53) - 1;
    let mut k = vec![0, top, 1 << 52, 1, top - 1];
    let mut s = 1289u64;
    while k.len() < STREAM_LEN {
        k.push(splitmix64(&mut s) >> 11);
    }
    k
}

/// Test-only replay generator: PETIR's `Rng` over a fixed list.
struct Replay {
    k: Vec<u64>,
    pos: usize,
}
impl Replay {
    fn new() -> Self {
        Replay {
            k: uniform_stream(),
            pos: 0,
        }
    }
}
impl Rng for Replay {
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    fn next_u64(&mut self) -> u64 {
        let k = self.k[self.pos % self.k.len()];
        self.pos += 1;
        k << 11
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for b in dest {
            *b = self.next_u64() as u8;
        }
    }
}

fn cubed(seed: u64, n: usize) -> Vec<f64> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            let u = (splitmix64(&mut s) >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
            u * u * u
        })
        .collect()
}
fn seq(n: usize, scale: f64) -> Vec<f64> {
    (0..n).map(|i| scale * i as f64).collect()
}
fn sq(n: usize) -> Vec<f64> {
    (0..n).map(|i| 0.01 * ((i * i) as i32) as f64).collect()
}
fn ones(n: usize) -> Vec<f64> {
    vec![1.0; n]
}
fn steep(n: usize) -> Vec<f64> {
    (0..n as u64)
        .map(|i| f64::from_bits((1023 - i) << 52))
        .collect()
}
const ZEROS: [f64; 13] = [
    0.0, 3.0, 0.0, 0.0, 1.0, 0.0, 7.0, 0.0, 2.0, 0.0, 0.0, 5.0, 0.0,
];
fn zeros(n: usize) -> Vec<f64> {
    ZEROS[..n].to_vec()
}

/// One case: an alias table with its inputs, or a simple table with its `f`.
enum Case {
    Alias {
        name: &'static str,
        table: EgsAliasTable,
        egs_type: i32,
        func_rc: i32,
    },
    Simple {
        name: &'static str,
        table: EgsSimpleAliasTable,
        f: Vec<f64>,
    },
}

fn alias(name: &'static str, t: i32, x: Vec<f64>, f: Vec<f64>) -> Case {
    let kind = AliasTableKind::from_egs_type(t).unwrap();
    Case::Alias {
        name,
        table: EgsAliasTable::new(&x, &f, kind).unwrap(),
        egs_type: t,
        func_rc: -1,
    }
}

fn cases() -> Vec<Case> {
    let mut c = vec![
        alias("discrete_uniform", 0, seq(10, 1.0), ones(10)),
        alias("discrete_steep", 0, seq(40, 0.25), steep(40)),
        alias("discrete_zeros", 0, seq(12, 1.0), zeros(12)),
        alias("discrete_single", 0, vec![2.5], vec![4.0]),
        alias("discrete_many", 0, seq(10000, 0.5), cubed(101, 10000)),
        alias("histogram_uniform", 1, seq(11, 0.1), ones(10)),
        alias("histogram_steep", 1, sq(33), steep(32)),
        alias("histogram_zeros", 1, seq(13, 1.0), zeros(12)),
        alias("histogram_single", 1, vec![1.0, 3.0], vec![0.5]),
        alias("histogram_many", 1, seq(10001, 1e-3), cubed(202, 10000)),
        alias("linear_uniform", 2, seq(11, 0.1), ones(11)),
        alias("linear_steep", 2, sq(33), steep(33)),
        alias("linear_zeros", 2, seq(13, 1.0), zeros(13)),
        alias("linear_single", 2, vec![0.0, 1.0], vec![1.0, 1.1]),
        alias("linear_many", 2, seq(10001, 1e-3), cubed(303, 10001)),
        alias("linear_type3", 3, sq(33), steep(33)),
    ];
    let (t, g) =
        EgsAliasTable::from_function(0.0, 4.0, 1e-3, 1000, |x| 1.0 / (1.0 + x * x)).unwrap();
    c.push(Case::Alias {
        name: "func_lorentz",
        table: t,
        egs_type: 2,
        func_rc: if g == AccuracyGoal::Met { 0 } else { 1 },
    });
    let (t, g) = EgsAliasTable::from_function(0.0, 2.0, 1e-8, 50, |x| x * x * x + 0.01).unwrap();
    c.push(Case::Alias {
        name: "func_nmax",
        table: t,
        egs_type: 2,
        func_rc: if g == AccuracyGoal::Met { 0 } else { 1 },
    });
    for (name, f) in [
        ("simple_uniform", ones(10)),
        ("simple_steep", steep(40)),
        ("simple_zeros", zeros(12)),
        ("simple_single", vec![4.0]),
        ("simple_many", cubed(404, 10000)),
    ] {
        c.push(Case::Simple {
            name,
            table: EgsSimpleAliasTable::new(&f).unwrap(),
            f,
        });
    }
    c
}

// ------------------------------------------------------------------ digests

struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn add(&mut self, v: u64) {
        for i in 0..8 {
            self.0 ^= (v >> (8 * i)) & 0xff;
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
}

/// What this port produces for one alias table, in the harness's layout.
struct AliasOut {
    table_digest: u64,
    samples: Vec<f64>,
    bins: Vec<usize>,
}

fn run_alias(t: &EgsAliasTable) -> AliasOut {
    let np = t.wi().len();
    let mut td = Fnv::new();
    td.add(t.xi().len() as u64);
    td.add(np as u64);
    td.add(t.average().to_bits());
    t.xi().iter().for_each(|v| td.add(v.to_bits()));
    t.fi().iter().for_each(|v| td.add(v.to_bits()));
    t.wi().iter().for_each(|v| td.add(v.to_bits()));
    t.bins().iter().for_each(|&b| td.add(b as u64));
    let mut r = Replay::new();
    let samples: Vec<f64> = (0..NSAMPLE).map(|_| t.sample(&mut r)).collect();
    let mut r = Replay::new();
    let bins: Vec<usize> = (0..NSAMPLE).map(|_| t.sample_bin(&mut r)).collect();
    AliasOut {
        table_digest: td.0,
        samples,
        bins,
    }
}

fn run_simple(t: &EgsSimpleAliasTable) -> (u64, Vec<usize>) {
    let mut td = Fnv::new();
    td.add(t.wi().len() as u64);
    t.wi().iter().for_each(|v| td.add(v.to_bits()));
    t.bins().iter().for_each(|&b| td.add(b as u64));
    let mut r = Replay::new();
    (td.0, (0..NSAMPLE).map(|_| t.sample(&mut r)).collect())
}

fn digest_f64(v: &[f64]) -> u64 {
    let mut d = Fnv::new();
    v.iter().for_each(|x| d.add(x.to_bits()));
    d.0
}
fn digest_usize(v: &[usize]) -> u64 {
    let mut d = Fnv::new();
    v.iter().for_each(|&x| d.add(x as u64));
    d.0
}

/// `reference_summary.txt`, case name -> key -> value text.
fn reference() -> HashMap<String, HashMap<String, String>> {
    let mut out = HashMap::new();
    let mut cur: Option<(String, HashMap<String, String>)> = None;
    for line in REFERENCE.lines() {
        let (k, v) = line.split_once(' ').unwrap_or((line, ""));
        match k {
            "case" => cur = Some((v.to_string(), HashMap::new())),
            "end" => {
                let (n, m) = cur.take().unwrap();
                out.insert(n, m);
            }
            _ => {
                cur.as_mut().unwrap().1.insert(k.to_string(), v.to_string());
            }
        }
    }
    out
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// Bit-identity against upstream, through the committed digests and small
/// vectors. See the module docs.
#[test]
fn digests_match_upstream() {
    let refs = reference();
    let cs = cases();
    assert_eq!(
        refs.len(),
        cs.len(),
        "case count differs from the reference"
    );
    for c in &cs {
        match c {
            Case::Alias {
                name,
                table,
                egs_type,
                func_rc,
            } => {
                let r = &refs[*name];
                assert_eq!(r["type"], egs_type.to_string(), "{name}: type");
                assert_eq!(
                    r["func_rc"],
                    func_rc.to_string(),
                    "{name}: initialize() return"
                );
                assert_eq!(r["warnings"], "0", "{name}: upstream warned");
                assert!(!table.aliasing_incomplete(), "{name}");
                assert_eq!(r["n"], table.xi().len().to_string(), "{name}: n");
                assert_eq!(
                    hex(&r["average"]),
                    table.average().to_bits(),
                    "{name}: average"
                );
                let o = run_alias(table);
                assert_eq!(hex(&r["table_digest"]), o.table_digest, "{name}: table");
                assert_eq!(
                    hex(&r["sample_digest"]),
                    digest_f64(&o.samples),
                    "{name}: samples"
                );
                assert_eq!(hex(&r["bin_digest"]), digest_usize(&o.bins), "{name}: bins");
                let first: Vec<u64> = r["first"].split_whitespace().map(hex).collect();
                let mine: Vec<u64> = o.samples[..16].iter().map(|x| x.to_bits()).collect();
                assert_eq!(first, mine, "{name}: first samples");
                if let Some(wi) = r.get("wi") {
                    let wi: Vec<u64> = wi.split_whitespace().map(hex).collect();
                    let mine: Vec<u64> = table.wi().iter().map(|x| x.to_bits()).collect();
                    assert_eq!(wi, mine, "{name}: wi");
                    let bin: Vec<usize> = r["bin"]
                        .split_whitespace()
                        .map(|s| s.parse().unwrap())
                        .collect();
                    assert_eq!(bin, table.bins(), "{name}: bin");
                }
            }
            Case::Simple { name, table, .. } => {
                let r = &refs[*name];
                assert_eq!(r["warnings"], "0", "{name}: upstream warned");
                assert!(!table.aliasing_incomplete(), "{name}");
                let (td, s) = run_simple(table);
                assert_eq!(hex(&r["table_digest"]), td, "{name}: table");
                assert_eq!(hex(&r["bin_digest"]), digest_usize(&s), "{name}: samples");
                let first: Vec<usize> = r["first"]
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect();
                assert_eq!(first, s[..16], "{name}: first samples");
                if let Some(wi) = r.get("wi") {
                    let wi: Vec<u64> = wi.split_whitespace().map(hex).collect();
                    let mine: Vec<u64> = table.wi().iter().map(|x| x.to_bits()).collect();
                    assert_eq!(wi, mine, "{name}: wi");
                    let bin: Vec<usize> = r["bin"]
                        .split_whitespace()
                        .map(|s| s.parse().unwrap())
                        .collect();
                    assert_eq!(bin, table.bins(), "{name}: bins");
                }
            }
        }
    }
}

// -------------------------------------------------------- full dump compare

/// Compare one f64 list: (count, bit-identical, worst relative difference).
fn cmp_f64(mine: &[f64], up: &[u64]) -> (usize, usize, f64) {
    assert_eq!(mine.len(), up.len());
    let mut same = 0;
    let mut worst = 0.0f64;
    for (&a, &b) in mine.iter().zip(up) {
        if a.to_bits() == b {
            same += 1;
        } else {
            let b = f64::from_bits(b);
            let d = if b == 0.0 {
                (a - b).abs()
            } else {
                ((a - b) / b).abs()
            };
            worst = worst.max(if d.is_nan() { f64::INFINITY } else { d });
        }
    }
    (mine.len(), same, worst)
}

/// Entry-by-entry comparison against the harness's full dumps, when
/// `EGS_ALIAS_DUMP_DIR` names them (`build_and_run.sh OUTDIR` writes them).
/// Prints the comparison table of the V&V record. Without the variable it
/// returns at once: the dumps are 69 MB and not committed, and
/// [`digests_match_upstream`] covers the same values through digests.
#[test]
fn full_dump_matches_upstream_entry_by_entry() {
    let Ok(dir) = std::env::var("EGS_ALIAS_DUMP_DIR") else {
        println!("EGS_ALIAS_DUMP_DIR not set: full-dump comparison not run");
        return;
    };
    println!("| case | quantity | compared | bit-identical | worst rel. diff |");
    println!("|---|---|---|---|---|");
    let mut all_same = true;
    for c in cases() {
        let name = match &c {
            Case::Alias { name, .. } | Case::Simple { name, .. } => *name,
        };
        let text = std::fs::read_to_string(format!("{dir}/{name}.txt")).unwrap();
        let mut f: HashMap<&str, Vec<&str>> = HashMap::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let k = it.next().unwrap();
            let v = it.last().unwrap();
            f.entry(k).or_default().push(v);
        }
        let hexes = |k: &str| -> Vec<u64> {
            f.get(k)
                .map(|v| v.iter().map(|s| hex(s)).collect())
                .unwrap_or_default()
        };
        let ints = |k: &str| -> Vec<usize> {
            f.get(k)
                .map(|v| v.iter().map(|s| s.parse().unwrap()).collect())
                .unwrap_or_default()
        };
        let mut rows: Vec<(&str, usize, usize, f64)> = Vec::new();
        let int_row = |mine: &[usize], up: &[usize]| -> (usize, usize, f64) {
            assert_eq!(mine.len(), up.len());
            let same = mine.iter().zip(up).filter(|(a, b)| a == b).count();
            (
                mine.len(),
                same,
                if same == mine.len() { 0.0 } else { f64::NAN },
            )
        };
        match &c {
            Case::Alias { table, .. } => {
                let o = run_alias(table);
                let (n, s, w) = cmp_f64(&[table.average()], &hexes("average"));
                rows.push(("average", n, s, w));
                let (n, s, w) = cmp_f64(table.xi(), &hexes("xi"));
                rows.push(("xi", n, s, w));
                let (n, s, w) = cmp_f64(table.fi(), &hexes("fi"));
                rows.push(("fi", n, s, w));
                let (n, s, w) = cmp_f64(table.wi(), &hexes("wi"));
                rows.push(("wi", n, s, w));
                let (n, s, w) = int_row(table.bins(), &ints("bin"));
                rows.push(("bin", n, s, w));
                let (n, s, w) = cmp_f64(&o.samples, &hexes("s"));
                rows.push(("sample()", n, s, w));
                let (n, s, w) = int_row(&o.bins, &ints("b"));
                rows.push(("sampleBin()", n, s, w));
            }
            Case::Simple { table, .. } => {
                let (_, smp) = run_simple(table);
                let (n, s, w) = cmp_f64(table.wi(), &hexes("wi"));
                rows.push(("wi", n, s, w));
                let (n, s, w) = int_row(table.bins(), &ints("bin"));
                rows.push(("bins", n, s, w));
                let (n, s, w) = int_row(&smp, &ints("b"));
                rows.push(("sample()", n, s, w));
            }
        }
        for (q, n, s, w) in rows {
            all_same &= n == s;
            println!("| {name} | {q} | {n} | {s} | {w:e} |");
        }
    }
    assert!(
        all_same,
        "a value differs from upstream; see the table above"
    );
}

// --------------------------------------------------------------- chi-squared

/// Draws of the replayed stream for which `r1 * np` is exactly an integer
/// `j` with `wi[j] == 0`: upstream (and this port) return such a
/// zero-probability bin, because `aj = 0` and `0 > 0` is false.
fn exact_integer_hits(t: &EgsAliasTable, draws_per_sample: usize) -> usize {
    let np = t.wi().len() as f64;
    let mut r = Replay::new();
    let mut hits = 0;
    for _ in 0..NSAMPLE {
        let r1: f64 = r.random();
        if draws_per_sample == 2 {
            let _: f64 = r.random();
        }
        let aj = r1 * np;
        let j = aj as usize;
        if aj == j as f64 && t.wi()[j] == 0.0 {
            hits += 1;
        }
    }
    hits
}

/// Pool cells in order until each expects at least 5; return (chi2, dof).
/// Cells with zero expectation must be empty (checked by the caller).
fn chi_squared(obs: &[f64], expect: &[f64]) -> (f64, usize) {
    let mut pooled: Vec<(f64, f64)> = Vec::new();
    let (mut o, mut e) = (0.0, 0.0);
    for (&oi, &ei) in obs.iter().zip(expect) {
        if ei == 0.0 {
            continue;
        }
        o += oi;
        e += ei;
        if e >= 5.0 {
            pooled.push((o, e));
            o = 0.0;
            e = 0.0;
        }
    }
    if e > 0.0 {
        match pooled.last_mut() {
            Some(last) => {
                last.0 += o;
                last.1 += e;
            }
            None => pooled.push((o, e)),
        }
    }
    let chi2 = pooled.iter().map(|&(o, e)| (o - e) * (o - e) / e).sum();
    (chi2, pooled.len().saturating_sub(1))
}

/// Upper-tail p-value of chi-squared with `dof` degrees of freedom:
/// `1 - P(dof/2, chi2/2)` through PETIR's GSL port, or, where that port
/// refuses the branch (`Invalid`: its `gammastar_ser` branch for `a >= 10`,
/// i.e. `dof >= 20`, is deliberately not ported), the Wilson-Hilferty normal
/// approximation, whose error in `p` at `dof >= 20` is far below the margin
/// between any measured `p` and `alpha`. The table marks which was used.
fn p_value(chi2: f64, dof: usize) -> (f64, &'static str) {
    if dof == 0 {
        return (1.0, "-");
    }
    let k = dof as f64;
    match gamma_inc_p(k / 2.0, chi2 / 2.0) {
        Ok((p, _)) => (1.0 - p, "P"),
        Err(_) => {
            let v = 2.0 / (9.0 * k);
            let z = ((chi2 / k).cbrt() - (1.0 - v)) / v.sqrt();
            (0.5 * erfc(z / std::f64::consts::SQRT_2), "WH")
        }
    }
}

/// Exact cell probabilities of an alias table's INPUT distribution: one cell
/// per bin, or `sub` equal sub-intervals per bin for histogram and linear.
fn cell_probabilities(kind: AliasTableKind, x: &[f64], f: &[f64], sub: usize) -> Vec<f64> {
    let mut p = Vec::new();
    match kind {
        AliasTableKind::Discrete => p.extend_from_slice(f),
        AliasTableKind::Histogram => {
            for (w, &fi) in x.windows(2).zip(f) {
                for _ in 0..sub {
                    p.push(fi * (w[1] - w[0]) / sub as f64);
                }
            }
        }
        AliasTableKind::LinearInterpolation => {
            for (w, g) in x.windows(2).zip(f.windows(2)) {
                let h = (w[1] - w[0]) / sub as f64;
                for s in 0..sub {
                    let a = s as f64 / sub as f64;
                    let b = (s + 1) as f64 / sub as f64;
                    let fa = g[0] + (g[1] - g[0]) * a;
                    let fb = g[0] + (g[1] - g[0]) * b;
                    p.push(0.5 * (fa + fb) * h);
                }
            }
        }
    }
    let total: f64 = p.iter().sum();
    p.iter().map(|v| v / total).collect()
}

/// Cell index of a continuous sample: bin by edge search, then sub-interval.
fn cell_of(x: &[f64], v: f64, sub: usize) -> usize {
    let np = x.len() - 1;
    let j = x.partition_point(|&e| e <= v).saturating_sub(1).min(np - 1);
    let frac = (v - x[j]) / (x[j + 1] - x[j]);
    let s = ((frac * sub as f64) as usize).min(sub - 1);
    j * sub + s
}

/// Sampled frequencies against the input distribution, chi-squared at
/// `alpha = 0.001` (fixed in advance). Prints the table of the V&V record
/// (`--nocapture`). Inputs are rebuilt here from the recipes; a
/// function-driven table is judged against its own (normalised, linear)
/// table, which is what it samples.
#[test]
fn sampled_frequencies_pass_chi_squared() {
    const SUB: usize = 4;
    println!("| case | sampler | cells (pooled) | dof | chi2 | p | zero-prob hits (explained) |");
    println!("|---|---|---|---|---|---|---|");
    println!("(p: P = 1 - gamma_inc_p, WH = Wilson-Hilferty)");
    let inputs: HashMap<&str, (Vec<f64>, Vec<f64>)> = [
        ("discrete_uniform", (seq(10, 1.0), ones(10))),
        ("discrete_steep", (seq(40, 0.25), steep(40))),
        ("discrete_zeros", (seq(12, 1.0), zeros(12))),
        ("discrete_single", (vec![2.5], vec![4.0])),
        ("discrete_many", (seq(10000, 0.5), cubed(101, 10000))),
        ("histogram_uniform", (seq(11, 0.1), ones(10))),
        ("histogram_steep", (sq(33), steep(32))),
        ("histogram_zeros", (seq(13, 1.0), zeros(12))),
        ("histogram_single", (vec![1.0, 3.0], vec![0.5])),
        ("histogram_many", (seq(10001, 1e-3), cubed(202, 10000))),
        ("linear_uniform", (seq(11, 0.1), ones(11))),
        ("linear_steep", (sq(33), steep(33))),
        ("linear_zeros", (seq(13, 1.0), zeros(13))),
        ("linear_single", (vec![0.0, 1.0], vec![1.0, 1.1])),
        ("linear_many", (seq(10001, 1e-3), cubed(303, 10001))),
        ("linear_type3", (sq(33), steep(33))),
    ]
    .into_iter()
    .collect();
    let mut failures = Vec::new();
    let mut check = |name: &str, sampler: &str, obs: &[f64], p_cell: &[f64], explained: usize| {
        let expect: Vec<f64> = p_cell.iter().map(|p| p * NSAMPLE as f64).collect();
        let zero_hits: f64 = obs
            .iter()
            .zip(&expect)
            .filter(|(_, &e)| e == 0.0)
            .map(|(&o, _)| o)
            .sum();
        let (chi2, dof) = chi_squared(obs, &expect);
        let (p, how) = p_value(chi2, dof);
        println!(
            "| {name} | {sampler} | {} | {dof} | {chi2:.1} | {p:.3} {how} | {} ({explained}) |",
            p_cell.len(),
            zero_hits as usize
        );
        if p <= ALPHA || zero_hits != explained as f64 {
            failures.push(format!("{name} {sampler}: p = {p}, zero hits {zero_hits}"));
        }
    };
    for c in cases() {
        match c {
            Case::Alias { name, table, .. } => {
                let kind = table.kind();
                let (x, f) = match inputs.get(name) {
                    Some((x, f)) => (x.clone(), f.clone()),
                    None => (table.xi().to_vec(), table.fi().to_vec()),
                };
                let o = run_alias(&table);
                // sampleBin: one cell per bin.
                let pbin = cell_probabilities(kind, &x, &f, 1);
                let mut obs = vec![0.0; pbin.len()];
                o.bins.iter().for_each(|&b| obs[b] += 1.0);
                check(
                    name,
                    "sampleBin",
                    &obs,
                    &pbin,
                    exact_integer_hits(&table, 1),
                );
                if kind == AliasTableKind::Discrete {
                    continue;
                }
                let pc = cell_probabilities(kind, &x, &f, SUB);
                let mut obs = vec![0.0; pc.len()];
                o.samples
                    .iter()
                    .for_each(|&v| obs[cell_of(&x, v, SUB)] += 1.0);
                check(name, "sample", &obs, &pc, exact_integer_hits(&table, 2));
            }
            Case::Simple { name, table, f } => {
                let (_, s) = run_simple(&table);
                let total: f64 = f.iter().sum();
                let p: Vec<f64> = f.iter().map(|v| v / total).collect();
                let mut obs = vec![0.0; p.len()];
                s.iter().for_each(|&b| obs[b] += 1.0);
                check(name, "sample", &obs, &p, 0);
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
