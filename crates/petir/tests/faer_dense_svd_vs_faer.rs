// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **Code-to-code: `petir::faer_dense` against faer's own compiled SVD.**
//!
//! # Methodology
//!
//! `petir::faer_dense::svd_square` is a scalar port of the path faer 0.24.4's
//! `Mat::svd()` runs for a small square `f64` matrix (gh #996). The
//! reference is faer itself, a dev-dependency, called exactly as
//! `outram-blender`'s `arap::closest_rotation` called it before the port
//! (`faer::mat![..].svd()`). Every entry of `U`, `s` and `V` is compared by
//! **bit pattern** (`f64::to_bits`, so `-0.0` and `+0.0` count as
//! different); the pass criterion is zero differing bits on every case.
//!
//! Inputs:
//!
//! 1. **Every distinct `3 x 3` covariance `closest_rotation` received** while
//!    `outram-blender`'s lib and integration tests ran, recorded with their
//!    bit patterns in two files under `reference-data/faer/`:
//!    `closest_rotation_inputs_pre_gh1002.txt`, one run of the pre-port code
//!    (2026-10-20, faer path), whose covariances varied from run to run, and
//!    `closest_rotation_inputs.txt`, re-captured after gh #1002 made the
//!    suite deterministic (2026-10-20, port path; two runs gave the identical
//!    13 928-call sequence, so a re-capture reproduces the file).
//! 2. **Random matrices of every order 1..=16** from a fixed xorshift stream,
//!    with entries spread over 1e-3..1e3, half of them with random exponents
//!    over 1e-150..1e150.
//! 3. **Structured cases**: zero, identity, `-I`, permutations, repeated and
//!    zero singular values, rank one, a reflection, signed zeros, subnormal and
//!    near-overflow scales, and non-finite input (both must fail).
//!
//! faer's result depends on the instruction set it detects at run time; the
//! port models `pulp` V3 (AVX2 + FMA). On another instruction set this test
//! reports the differences instead of passing; see the module docs.
//!
//! # Results (2026-10-20, faer 0.24.4, x86-64 AVX2+FMA, i9-13900K)
//!
//! | inputs | cases | differing bits |
//! |---|---|---|
//! | `outram-blender` `closest_rotation` covariances, pre-gh #1002 run | 6 134 distinct | 0 |
//! | `outram-blender` `closest_rotation` covariances, after gh #1002 | 3 586 distinct (472 shared with the line above) | 0 |
//! | random, order 3 | 20 000 | 0 |
//! | random, orders 1, 2, 4..=16 | 400 each (6 000) | 0 |
//! | structured (17, incl. NaN / inf, which both reject) | 17 | 0 |
//!
//! **The test can fail; two negative controls were run (2026-10-20) and
//! reverted.** Summing the lanes as `(l0 + l1) + (l2 + l3)` instead of
//! pulp's `(l0 + l2) + (l1 + l3)`: 4 494 of the 26 000 random matrices
//! differ (all of order >= 4; at order 3 no reduction spans more than two
//! lanes). Replacing the fused `vfmadd` by `a * b + c`: 4 157 of the 6 134
//! blender inputs, 18 779 of the random matrices and 4 structured cases
//! differ.
//!
//! Interpretation: the port reproduces faer's dense SVD exactly on this
//! instruction set, so swapping it into `outram-blender` changes no rotation.
//! Checked there too: ~~the blender suite's covariances differ in the last
//! bits from run to run (`HashMap`-order sums, gh #1002)~~ **(fixed by gh #1002,
//! 2026-10-20; the suite is now deterministic and the second input file is
//! its exact, reproducible set)**, so its dumps could not be diffed line by
//! line; instead all 22 334 distinct covariances pooled from one faer run and
//! three port runs were fed through this test (one-off, 2026-10-20): 0
//! differing bits, and the 963 covariances both paths met gave identical
//! rotations. Only the first run's 6 134 are committed, as the pre-gh #1002
//! file.

use std::path::PathBuf;

use faer::Mat;
use petir::faer_dense::{svd_square, MAX_ORDER};

/// faer's SVD of a column-major `n x n` matrix, as `(U, s, V)` column-major,
/// or `None` where faer returns an error.
fn faer_svd(a: &[f64], n: usize) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let m = Mat::<f64>::from_fn(n, n, |i, j| a[j * n + i]);
    let svd = m.svd().ok()?;
    let u = svd.U();
    let v = svd.V();
    let s = svd.S();
    let mut uu = Vec::with_capacity(n * n);
    let mut vv = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            uu.push(u[(i, j)]);
            vv.push(v[(i, j)]);
        }
    }
    let ss = (0..n).map(|i| s[i]).collect();
    Some((uu, ss, vv))
}

/// Number of differing bit patterns between the port and faer on one input
/// (`usize::MAX` if exactly one of them failed).
fn compare(a: &[f64], n: usize) -> usize {
    let theirs = faer_svd(a, n);
    let ours = svd_square(a, n).ok();
    match (theirs, ours) {
        (None, None) => 0,
        (Some((u, s, v)), Some(r)) => {
            let diff = |x: &[f64], y: &[f64]| {
                x.iter()
                    .zip(y)
                    .filter(|(p, q)| p.to_bits() != q.to_bits())
                    .count()
            };
            diff(&u, &r.u) + diff(&s, &r.s) + diff(&v, &r.v)
        }
        _ => usize::MAX,
    }
}

/// Row-major 3x3 from a reference file in `reference-data/faer/` ->
/// column-major.
fn parse_inputs(file: &str) -> Vec<[f64; 9]> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/faer")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let w: Vec<f64> = line
            .split_whitespace()
            .map(|h| f64::from_bits(u64::from_str_radix(h, 16).unwrap()))
            .collect();
        assert_eq!(w.len(), 9, "bad line {line}");
        let mut cm = [0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                cm[j * 3 + i] = w[i * 3 + j];
            }
        }
        out.push(cm);
    }
    out
}

/// Every covariance `outram-blender`'s tests gave `closest_rotation`, from
/// both captures (see the module docs).
///
/// Result 2026-10-20: 6 134 distinct pre-gh #1002 inputs and 3 586 distinct
/// post-gh #1002 inputs, **0 differing bits** on each.
#[test]
fn blender_closest_rotation_inputs_are_bit_identical() {
    for file in [
        "closest_rotation_inputs_pre_gh1002.txt",
        "closest_rotation_inputs.txt",
    ] {
        let inputs = parse_inputs(file);
        assert!(inputs.len() > 100, "{file}: only {} inputs", inputs.len());
        let mut bad = 0;
        for a in &inputs {
            let d = compare(a, 3);
            if d != 0 {
                bad += 1;
                if bad < 5 {
                    eprintln!("{file}: differs ({d}): {a:?}");
                }
            }
        }
        assert_eq!(bad, 0, "{file}: {bad} of {} inputs differ", inputs.len());
    }
}

/// xorshift64*, fixed seed: reproducible without `rand`.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn entry(&mut self, wide: bool) -> f64 {
        let sign = if self.next() & 1 == 0 { 1.0 } else { -1.0 };
        let span = if wide { 150.0 } else { 3.0 };
        let e = (self.unit() * 2.0 - 1.0) * span;
        sign * (0.5 + self.unit()) * 10f64.powf(e)
    }
}

/// Random matrices of every supported order.
///
/// Result 2026-10-20: 20 000 `3 x 3` and 400 of each other order 1..=16
/// (26 000 matrices), **0 differing bits**.
#[test]
fn random_matrices_of_every_order_are_bit_identical() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut cases = 0;
    let mut bad = Vec::new();
    for n in 1..=MAX_ORDER {
        let count = if n == 3 { 20_000 } else { 400 };
        for c in 0..count {
            let wide = c % 2 == 1;
            let a: Vec<f64> = (0..n * n).map(|_| rng.entry(wide)).collect();
            let d = compare(&a, n);
            cases += 1;
            if d != 0 {
                bad.push((n, c, d));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {cases} differ, first {:?}",
        bad.len(),
        &bad[..bad.len().min(10)]
    );
}

/// Structured and degenerate inputs.
///
/// Result 2026-10-20: every case **0 differing bits**, and the non-finite
/// inputs fail in both.
#[test]
fn structured_cases_are_bit_identical() {
    let r = |rows: [[f64; 3]; 3]| {
        let mut cm = [0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                cm[j * 3 + i] = rows[i][j];
            }
        }
        cm
    };
    let (s, c) = 0.7f64.sin_cos();
    let cases: Vec<(&str, [f64; 9])> = vec![
        ("zero", [0.0; 9]),
        ("negative zero", [-0.0; 9]),
        (
            "identity",
            r([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        ),
        (
            "minus identity",
            r([[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]),
        ),
        (
            "reflection",
            r([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]),
        ),
        (
            "permutation",
            r([[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]),
        ),
        ("rotation", r([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])),
        (
            "repeated values",
            r([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]]),
        ),
        (
            "rank one",
            r([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [3.0, 6.0, 9.0]]),
        ),
        (
            "rank two",
            r([[1.0, 0.0, 1.0], [0.0, 1.0, 1.0], [1.0, 1.0, 2.0]]),
        ),
        (
            "one nonzero",
            r([[0.0, 0.0, 0.0], [0.0, 0.0, 5.0], [0.0, 0.0, 0.0]]),
        ),
        (
            "zero column",
            r([[1.0, 0.0, 2.0], [3.0, 0.0, 4.0], [5.0, 0.0, 6.0]]),
        ),
        (
            "subnormal",
            r([
                [1e-310, 2e-310, 0.0],
                [0.0, 3e-310, 1e-310],
                [4e-310, 0.0, 5e-310],
            ]),
        ),
        (
            "near overflow",
            r([
                [1e307, -2e307, 3e306],
                [5e306, 1e307, -1e307],
                [2e307, 4e306, 6e306],
            ]),
        ),
        (
            "mixed scales",
            r([
                [1e200, 1.0, 1e-200],
                [1.0, 1e-200, 1e200],
                [1e-200, 1e200, 1.0],
            ]),
        ),
        (
            "nan",
            r([[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        ),
        (
            "inf",
            r([[f64::INFINITY, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        ),
    ];
    let mut bad = Vec::new();
    for (name, a) in &cases {
        let d = compare(a, 3);
        if d != 0 {
            bad.push((*name, d));
        }
    }
    assert!(bad.is_empty(), "differ: {bad:?}");
    assert!(svd_square(&cases[15].1, 3).is_err() && faer_svd(&cases[15].1, 3).is_none());
}
