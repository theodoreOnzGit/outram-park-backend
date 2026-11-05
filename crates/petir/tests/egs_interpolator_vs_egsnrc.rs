// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **Code-to-code: `petir::interp_egs` against EGSnrc's compiled
//! `egs_interpolator.cpp`** (gh:#1289), plus the interpolator's accuracy
//! against the exact functions.
//!
//! # Methodology
//!
//! `verification_and_validation/egsnrc_interpolator/egs_interp_harness.cpp`
//! compiles EGSnrc's unmodified `egs_interpolator.cpp` (commit `bd49a47a`,
//! g++ 16.2.1, `-O3`, baseline x86-64, glibc 2.44) and builds 13
//! interpolators: three from tables passed identically (random values, a step
//! table, existing `a`/`b` coefficients), four from a function with fixed bins
//! (`exp` on `[0,5]`, `x^-1.5` on `[0.1,10]`, `1/(1+x^2)` on `[-3,3]`, and
//! `exp(-0.8u)+2` in a log variable `u = ln E`, `E` in `[1e-3, 20]`), and six
//! adaptive ones (including the `nmax <= 9` branch and the `nmax` fallback).
//! For each it records, as IEEE-754 bits: every `a[i]`, `b[i]`; every `x` the
//! function was called at, in order; and `interpolate`, `getIndex` (and, for
//! `x` in `[xmin, xmax]`, `interpolateFast`, `getIndexFast`,
//! `interpolateFast(getIndex(x), x)`) at every node, every midpoint, 15
//! special points (both ends, their neighbouring doubles, `+-1`, `+-L`,
//! `+-inf`, NaN, `+-0`) and 100 000 splitmix64 points on
//! `[xmin - 0.1L, xmax + 0.1L]`. This test replays the same cases through
//! PETIR (`real::exp`, `real::powf` for the functions) and compares FNV-1a
//! digests of each stream, explicit vectors, and the guard behaviour.
//!
//! **Pass criterion:** every digest, vector and bin count identical, i.e.
//! bit-identity on 5 941 353 recorded words: 5 915 706 evaluation outputs
//! (values and indices), 13 136 coefficient words (`a`, `b`, `xmin`, `xmax`)
//! and 12 511 function-call abscissae, plus 403 explicit vectors and 104
//! explicit coefficient pairs. The per-case table is in the V&V README.
//!
//! # Prediction (written before the first run)
//!
//! Tabulated and rational cases bit-identical; `exp`/`pow` cases bit-identical
//! on nearly all nodes, with possible 1-ulp node differences from glibc's
//! run-time FMA variants of `exp`/`pow`.
//!
//! # Results (2026-11-05, x86_64-linux, release)
//!
//! **All 13 cases bit-identical in every stream**, including the `exp`/`pow`
//! cases: PETIR's ARM-optimized-routines `exp`/`pow` and glibc 2.44's agreed
//! on every node evaluated here (12 511 calls). The `-march=native` (FMA
//! contraction) and `-ffast-math` builds of upstream differ from the `-O3`
//! reference; they are sensitivity runs, not references (V&V README).
//!
//! Accuracy (separate tests below): see each test's doc comment.

use petir::interp_egs::{DEFAULT_ACCU, DEFAULT_NMAX, EgsInterpolator};
use petir::{PetirError, real};

const REFERENCE: &str = "verification_and_validation/egsnrc_interpolator/reference.txt";

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Digest {
    h: u64,
    count: u64,
}
impl Digest {
    fn new() -> Self {
        Self {
            h: 0xcbf2_9ce4_8422_2325,
            count: 0,
        }
    }
    fn word(&mut self, w: u64) {
        for k in 0..8 {
            self.h ^= (w >> (8 * k)) & 0xff;
            self.h = self.h.wrapping_mul(0x0100_0000_01b3);
        }
        self.count += 1;
    }
}

/// The four functions, with the harness's expressions.
fn the_function(which: u8, x: f64) -> f64 {
    match which {
        0 => real::exp(x),
        1 => real::powf(x, -1.5),
        2 => 1.0 / (1.0 + x * x),
        _ => real::exp(-0.8 * x) + 2.0,
    }
}

fn random_table(seed: u64, len: usize, scale: f64, shift: f64) -> Vec<f64> {
    let mut g = SplitMix(seed);
    (0..len).map(|_| g.uniform() * scale + shift).collect()
}

const LOGVAR_LO: f64 = -6.907755278982137;
const LOGVAR_HI: f64 = 2.995732273553991;

const CASES: [&str; 13] = [
    "values_random",
    "values_steps",
    "func_exp",
    "func_powlaw",
    "func_rational",
    "func_logvar",
    "adaptive_exp",
    "adaptive_powlaw",
    "adaptive_rational_small",
    "adaptive_cap",
    "adaptive_rational",
    "adaptive_logvar",
    "coefficients",
];

/// Build case `name` as the harness does, recording the function's abscissae.
fn build(name: &str) -> (EgsInterpolator, Digest) {
    let calls = std::cell::RefCell::new(Digest::new());
    let f = {
        let rec = |which: u8| {
            let calls = &calls;
            move |x: f64| {
                calls.borrow_mut().word(x.to_bits());
                the_function(which, x)
            }
        };
        match name {
            "values_random" => {
                EgsInterpolator::from_values(33, 0.25, 7.0, &random_table(1, 33, 10.0, -5.0))
            }
            "values_steps" => EgsInterpolator::from_values(
                9,
                -1.0,
                1.0,
                &[0.0, 0.0, 1.0, 1.0, 1.0, -2.0, -2.0, 5.0, 5.0],
            ),
            "func_exp" => EgsInterpolator::from_function(65, 0.0, 5.0, rec(0)),
            "func_powlaw" => EgsInterpolator::from_function(200, 0.1, 10.0, rec(1)),
            "func_rational" => EgsInterpolator::from_function(101, -3.0, 3.0, rec(2)),
            "func_logvar" => EgsInterpolator::from_function(128, LOGVAR_LO, LOGVAR_HI, rec(3)),
            "adaptive_exp" => EgsInterpolator::adaptive(0.0, 5.0, rec(0), 1024, 1e-4),
            "adaptive_powlaw" => EgsInterpolator::adaptive(0.1, 10.0, rec(1), 4096, 1e-6),
            "adaptive_rational_small" => EgsInterpolator::adaptive(-3.0, 3.0, rec(2), 8, 1e-4),
            "adaptive_cap" => EgsInterpolator::adaptive(0.0, 30.0, rec(0), 100, 1e-8),
            "adaptive_rational" => EgsInterpolator::adaptive(-3.0, 3.0, rec(2), 1024, 1e-5),
            "adaptive_logvar" => {
                EgsInterpolator::adaptive(LOGVAR_LO, LOGVAR_HI, rec(3), 1024, 1e-4)
            }
            "coefficients" => EgsInterpolator::from_coefficients(
                21,
                1.0,
                4.0,
                random_table(2, 21, 4.0, -2.0),
                random_table(3, 21, 2.0, -1.0),
            ),
            other => panic!("unknown case {other}"),
        }
    };
    (f.expect("case builds"), calls.into_inner())
}

/// One recorded value: (kind, x bits, output bits), in the harness's order.
type Record = (&'static str, u64, u64);

fn coefficient_records(f: &EgsInterpolator) -> (Digest, Vec<Record>) {
    let mut d = Digest::new();
    let mut out = Vec::new();
    d.word(f.xmin().to_bits());
    d.word(f.xmax().to_bits());
    for i in 0..=f.bins() {
        let a = f.get_a(i).expect("a[i] stored").to_bits();
        let b = f.get_b(i).expect("b[i] stored").to_bits();
        d.word(a);
        d.word(b);
        out.push(("a", f64::from(i).to_bits(), a));
        out.push(("b", f64::from(i).to_bits(), b));
    }
    (d, out)
}

fn queries(case_index: u64, f: &EgsInterpolator) -> (Vec<f64>, usize, usize) {
    let (xmin, xmax, n) = (f.xmin(), f.xmax(), f.bins());
    let mut q = Vec::new();
    let dx = (xmax - xmin) / f64::from(n);
    for j in 0..=n {
        q.push(xmin + dx * f64::from(j));
    }
    for j in 0..n {
        q.push(xmin + dx * (f64::from(j) + 0.5));
    }
    let l = xmax - xmin;
    let special_start = q.len();
    q.extend([
        xmin,
        xmax,
        xmin.next_down(),
        xmin.next_up(),
        xmax.next_down(),
        xmax.next_up(),
        xmin - 1.0,
        xmax + 1.0,
        xmin - l,
        xmax + l,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
        0.0,
        -0.0,
    ]);
    let random_start = q.len();
    let mut g = SplitMix(12345 + case_index);
    let (lo, hi) = (xmin - 0.1 * l, xmax + 0.1 * l);
    for _ in 0..100_000 {
        q.push(lo + (hi - lo) * g.uniform());
    }
    (q, special_start, random_start)
}

fn eval_records(
    case_index: u64,
    f: &EgsInterpolator,
) -> (Digest, Vec<Record>, Vec<(u64, u64, i32)>) {
    let (q, special_start, random_start) = queries(case_index, f);
    let (xmin, xmax) = (f.xmin(), f.xmax());
    let mut d = Digest::new();
    let mut out = Vec::with_capacity(5 * q.len());
    let mut vecs = Vec::new();
    let mut put = |d: &mut Digest, kind: &'static str, x: f64, y: u64| {
        d.word(y);
        out.push((kind, x.to_bits(), y));
    };
    for (k, &x) in q.iter().enumerate() {
        let y = f.interpolate(x);
        let i = f.get_index(x);
        put(&mut d, "interpolate", x, y.to_bits());
        put(&mut d, "getIndex", x, i64::from(i) as u64);
        if x >= xmin && x <= xmax {
            // A None would be an index upstream reads out of bounds; record a
            // value no double has as bits so it can never match.
            let none = 0x7ff8_dead_beef_0001;
            put(
                &mut d,
                "interpolateFast",
                x,
                f.interpolate_fast(x).map_or(none, f64::to_bits),
            );
            put(
                &mut d,
                "getIndexFast",
                x,
                i64::from(f.get_index_fast(x)) as u64,
            );
            put(
                &mut d,
                "interpolateFastAt",
                x,
                f.interpolate_fast_at(i, x).map_or(none, f64::to_bits),
            );
        }
        let is_vec = (special_start..special_start + 15).contains(&k)
            || (random_start..random_start + 16).contains(&k);
        if is_vec {
            vecs.push((x.to_bits(), y.to_bits(), i));
        }
    }
    (d, out, vecs)
}

fn reference() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(REFERENCE);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).expect("hex")
}

#[test]
fn every_stream_of_every_case_is_bit_identical_to_upstream() {
    let text = reference();
    for (ci, name) in CASES.iter().enumerate() {
        let (f, calls) = build(name);
        let (coef, _) = coefficient_records(&f);
        let (eval, _, vecs) = eval_records(ci as u64, &f);
        let mut seen = 0;
        let mut ref_vecs = Vec::new();
        let mut ref_coefs = Vec::new();
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            match w.as_slice() {
                ["digest", c, "bins", n] if c == name => {
                    assert_eq!(f.bins(), n.parse::<i32>().unwrap(), "{name}: bins");
                    seen += 1;
                }
                ["digest", c, stream, count, h] if c == name => {
                    let got = match *stream {
                        "calls" => calls,
                        "coef" => coef,
                        "eval" => eval,
                        s => panic!("unknown stream {s}"),
                    };
                    let want = Digest {
                        h: hex(h),
                        count: count.parse().unwrap(),
                    };
                    assert_eq!(got, want, "{name}: {stream} digest");
                    seen += 1;
                }
                ["vec", c, x, y, i] if c == name => {
                    ref_vecs.push((hex(x), hex(y), i.parse::<i32>().unwrap()));
                }
                ["coef", c, i, a, b] if c == name => {
                    ref_coefs.push((i.parse::<i32>().unwrap(), hex(a), hex(b)));
                }
                _ => {}
            }
        }
        assert_eq!(seen, 4, "{name}: digest lines in the reference");
        assert_eq!(vecs, ref_vecs, "{name}: explicit vectors");
        assert!(!ref_coefs.is_empty());
        for (i, a, b) in ref_coefs {
            assert_eq!(f.get_a(i).unwrap().to_bits(), a, "{name}: a[{i}]");
            assert_eq!(f.get_b(i).unwrap().to_bits(), b, "{name}: b[{i}]");
        }
    }
}

/// Upstream's `egsFatal` cases (the harness throws from a stubbed
/// `egsFatal`), and the constructor defect: `ctor_function_defect` and
/// `ctor_coefficients_defect` are upstream's own constructors, which pass
/// `Xmax` for `Xmin` and so fail on valid input; PETIR ports the
/// `initialize` methods those constructors meant to call.
#[test]
fn guards_and_the_constructor_defect_match_upstream() {
    let text = reference();
    let raised = |name: &str| -> bool {
        let line = text
            .lines()
            .find(|l| l.starts_with(&format!("fatal {name} ")))
            .unwrap_or_else(|| panic!("no fatal line for {name}"));
        line.contains(" raised: ")
    };
    assert!(raised("values_nbin1"));
    assert_eq!(
        EgsInterpolator::from_values(1, 0.0, 1.0, &[1.0]),
        Err(PetirError::Invalid)
    );
    assert!(raised("values_equal_ends"));
    assert_eq!(
        EgsInterpolator::from_values(2, 1.0, 1.0, &[1.0, 2.0]),
        Err(PetirError::Domain)
    );
    assert!(raised("values_reversed"));
    assert_eq!(
        EgsInterpolator::from_values(2, 2.0, 1.0, &[1.0, 2.0]),
        Err(PetirError::Domain)
    );
    assert!(!raised("values_nan_xmin"));
    assert!(EgsInterpolator::from_values(2, f64::NAN, 1.0, &[1.0, 2.0]).is_ok());
    assert!(raised("adaptive_nmax1"));
    assert_eq!(
        EgsInterpolator::adaptive(0.0, 1.0, |x| x, 1, 1e-4),
        Err(PetirError::Invalid)
    );
    assert!(raised("ctor_function_defect"));
    assert!(raised("ctor_coefficients_defect"));
    assert!(!raised("initialize_function_ok"));
    assert!(EgsInterpolator::from_function(5, 0.0, 1.0, |x| 1.0 / (1.0 + x * x)).is_ok());
    assert!(
        EgsInterpolator::from_coefficients(3, 0.0, 1.0, vec![1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0])
            .is_ok()
    );
}

/// Per-value comparison against the harness's full dump, printing the V&V
/// table. Needs the dump, which is not committed (366 MB):
/// `verification_and_validation/egsnrc_interpolator/build_and_run.sh OUT`, then
/// `EGS_INTERP_FULL_DUMP=OUT/full_dump.txt cargo test --release -p petir
/// --test egs_interpolator_vs_egsnrc -- --ignored --nocapture`.
#[test]
#[ignore = "needs the uncommitted full dump from build_and_run.sh"]
fn full_dump_comparison_table() {
    let path = std::env::var("EGS_INTERP_FULL_DUMP").expect("set EGS_INTERP_FULL_DUMP");
    let text = std::fs::read_to_string(path).expect("read dump");
    let mut lines = text.lines();
    println!("| case | values compared | bit-identical | worst relative difference |");
    println!("|---|---|---|---|");
    let mut all_identical = true;
    for (ci, name) in CASES.iter().enumerate() {
        let (f, _) = build(name);
        let (_, coefs) = coefficient_records(&f);
        let (_, evals, _) = eval_records(ci as u64, &f);
        let (mut count, mut same, mut worst) = (0u64, 0u64, 0.0f64);
        for (kind, x, y) in coefs.iter().chain(evals.iter()) {
            let line = lines.next().expect("dump too short");
            let w: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(
                (w[0], w[1], hex(w[2])),
                (*name, *kind, *x),
                "dump out of step"
            );
            let want = hex(w[3]);
            count += 1;
            if want == *y {
                same += 1;
            } else if !kind.starts_with("getIndex") {
                let (a, b) = (f64::from_bits(want), f64::from_bits(*y));
                worst = worst.max(((a - b) / a).abs());
            } else {
                worst = f64::INFINITY;
            }
        }
        all_identical &= same == count;
        println!("| `{name}` | {count} | {same} | {worst:e} |");
    }
    assert!(lines.next().is_none(), "dump longer than the cases");
    assert!(all_identical);
}

/// Largest relative error of `f` against `exact` over `samples` uniform points
/// strictly inside the interval.
fn max_rel_error(f: &EgsInterpolator, exact: impl Fn(f64) -> f64, samples: u32) -> f64 {
    let (lo, hi) = (f.xmin(), f.xmax());
    (1..samples)
        .map(|k| {
            let x = lo + (hi - lo) * f64::from(k) / f64::from(samples);
            let e = exact(x);
            ((f.interpolate(x) - e) / e).abs()
        })
        .fold(0.0, f64::max)
}

/// **Accuracy, fixed bins, against the piecewise-linear bound.**
///
/// Linear interpolation on a bin of width `h` errs by at most
/// `h^2/8 * max|f''|` on that bin. For `exp` that is `h^2/8 * e^x` there, so
/// the RELATIVE error is at most `h^2/8 * e^h` (the bin's `f''` over its
/// smallest `f`); for `x^-1.5` the relative bound on a bin starting at `x0` is
/// `h^2/8 * 3.75 x0^-3.5 / (x0+h)^-1.5`, largest on the first bin.
///
/// Measured 2026-11-05 on 199 999 interior points: `exp`, 64 bins on `[0,5]`
/// (`h = 0.078125`): max relative error **7.632e-4** against the bound
/// `h^2/8 * e^h` = **8.249e-4** (and `h^2/8` = 7.629e-4, which a function
/// with constant `f''/f` attains at a midpoint); `x^-1.5`, 199 bins on
/// `[0.1,10]` (`h = 0.04975`): **7.855e-2** against **2.126e-1** (the first
/// bin's bound, loose because `f''` falls by 4.1x across that bin).
#[test]
fn fixed_bins_are_within_the_piecewise_linear_bound() {
    let (f, _) = build("func_exp");
    let h: f64 = 5.0 / 64.0;
    let bound = h * h / 8.0 * real::exp(h);
    let err = max_rel_error(&f, real::exp, 200_000);
    println!("exp: max rel {err:e}, bound {bound:e}");
    assert!(
        err <= bound && err > 0.9 * h * h / 8.0,
        "exp: {err:e} vs {bound:e}"
    );

    let (g, _) = build("func_powlaw");
    let h: f64 = 9.9 / 199.0;
    let x0: f64 = 0.1;
    let bound = h * h / 8.0 * 3.75 * real::powf(x0, -3.5) / real::powf(x0 + h, -1.5);
    let err = max_rel_error(&g, |x| real::powf(x, -1.5), 200_000);
    println!("x^-1.5: max rel {err:e}, bound {bound:e}");
    assert!(err <= bound, "x^-1.5: {err:e} vs {bound:e}");
}

/// **Accuracy, adaptive, against upstream's own criterion.**
///
/// Upstream's claim (`egs_interpolator.h`): "the relative interpolation error
/// does not exceed `accu`". What the code tests is the relative error at
/// every bin MIDPOINT of the final grid (where linear interpolation of a
/// function with slowly varying `f''/f` errs most), against the midpoint
/// value computed at `xmin + ddx*(0.5+j)`. This test checks the claim
/// everywhere, on 200 000 interior points.
///
/// Measured 2026-11-05 on 199 999 interior points:
///
/// | case | outcome | bins | max relative error | `accu` |
/// |---|---|---|---|---|
/// | `exp` on `[0,5]` | converged | 256 | 4.768e-5 | 1e-4 |
/// | `exp(-0.8u)+2`, `u = ln E` | converged | 512 | 2.969e-5 | 1e-4 |
/// | `1/(1+x^2)` on `[-3,3]` | `nmax = 1024` fallback (513 points failed) | 1023 | 8.600e-6 | 1e-5 |
/// | `exp` on `[0,30]` | `nmax = 100` fallback | 99 | 1.153e-2 | 1e-8 |
///
/// The claim holds wherever the loop converged (max 0.48 `accu`), and the
/// default `nmax`/`accu` on `exp` gives the identical interpolator. On the
/// `nmax` fallback it is not guaranteed and upstream says so ("If the number
/// of required bins becomes greater than nmax, nmax bins are used"): the
/// rational case happens to meet it at 1023 bins; the `[0,30]` case misses it
/// by six orders and sits under the fixed-bin bound `h^2/8 e^h` = 1.554e-2,
/// `h = 30/99`.
#[test]
fn adaptive_meets_its_relative_accuracy_where_it_converges() {
    for (name, exact, accu, bins) in [
        ("adaptive_exp", 0u8, 1e-4, 256),
        ("adaptive_logvar", 3, 1e-4, 512),
        // nmax fallback, which happens to meet accu; see the table.
        ("adaptive_rational", 2, 1e-5, 1023),
    ] {
        let (f, _) = build(name);
        assert_eq!(f.bins(), bins, "{name}");
        let err = max_rel_error(&f, |x| the_function(exact, x), 200_000);
        println!("{name}: max rel {err:e} vs accu {accu:e}");
        assert!(err <= accu, "{name}: {err:e} > {accu:e}");
    }
    let d = EgsInterpolator::adaptive(0.0, 5.0, real::exp, DEFAULT_NMAX, DEFAULT_ACCU).unwrap();
    let (f, _) = build("adaptive_exp");
    assert_eq!(d, f);

    let (cap, _) = build("adaptive_cap");
    assert_eq!(cap.bins(), 99);
    let h: f64 = 30.0 / 99.0;
    let err = max_rel_error(&cap, real::exp, 200_000);
    println!(
        "adaptive_cap: max rel {err:e}, fixed-bin bound {:e}",
        h * h / 8.0 * real::exp(h)
    );
    assert!(err > 1e-8 && err <= h * h / 8.0 * real::exp(h));
}
