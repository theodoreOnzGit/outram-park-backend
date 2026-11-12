// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V: `petir::poly::{eval, eval_derivs, DividedDifference}` code-to-code
//! against GSL 2.8 compiled and run** (gh:#1338, case 4).
//!
//! # Methodology
//!
//! GSL 2.8's `gsl_poly_eval` (inline, `poly/gsl_poly.h`), `gsl_poly_eval_derivs`
//! (`poly/deriv.c`), `gsl_poly_dd_init`, `gsl_poly_dd_eval` (inline) and
//! `gsl_poly_dd_taylor` (`poly/dd.c`), built as a static library from the
//! vendored tree, were run by `reference-data/gsl/poly_reference_driver.c`. The
//! output is committed at `reference-data/gsl/poly-gsl-2.8-reference.txt`; it
//! carries the coefficients, abscissae and evaluation points as bit patterns,
//! so this test reads its inputs from the file.
//!
//! Inputs are exact rationals (small integers, dyadic fractions, or single
//! correctly rounded divisions); no libm is called on either side.
//!
//! Compared: every `gsl_poly_eval` value; every `gsl_poly_eval_derivs` output
//! for `lenres` below, equal to, and above `lenc`; the divided-difference
//! table `dd` from `gsl_poly_dd_init`; `gsl_poly_dd_eval` at each point; and the
//! Taylor coefficients `gsl_poly_dd_taylor` at each expansion point.
//!
//! **Not ported, and so not compared:** `gsl_poly_complex_eval`,
//! `gsl_complex_poly_complex_eval`, and `gsl_poly_dd_hermite_init`.
//!
//! **Pass criterion:** bit-identical for `eval`, `dd_init` and `dd_eval`.
//! For `eval_derivs` and `dd_taylor` the criterion is recorded at the
//! measured level, not forced: see the prediction below.
//!
//! **Prediction, written before the Rust test was run:**
//! - `eval` and `dd_init`/`dd_eval`: bit-identical. Same operations in the same
//!   order (Horner in `c[i-1] + x*ans` form; the divided-difference sweep with
//!   the same operand order).
//! - `eval_derivs`: NOT expected to be bit-identical. GSL runs one Horner sweep
//!   over all coefficients, updating `res[l]` in place, and multiplies by `n!`
//!   at the end. PETIR scales each coefficient by its falling factorial before
//!   Horner. Those are different rounding sequences, so differences of a few
//!   ulps are predicted where the result is not exact, and larger where
//!   cancellation occurs.
//! - `dd_taylor`: NOT expected to be bit-identical. PETIR's `to_taylor` is a
//!   direct accumulation `d[k] * prod(u + (xp - xa[j]))`, not GSL's reversed
//!   index recurrence on `w`. The mathematical result is the same polynomial,
//!   so differences of a few ulps are predicted.
//!
//! # Results
//!
//! **Result (2026-11-10, release, `CARGO_BUILD_JOBS=1`, GSL 2.8 `cf180cd7`
//! static build, gcc -O2):**
//!
//! | family | bit-identical | worst difference |
//! |---|---|---|
//! | `gsl_poly_eval` (18 values, degree 4, 1, 19) | 18/18 | 0 |
//! | `gsl_poly_dd_init` (30 values) | 30/30 | 0 |
//! | `gsl_poly_dd_eval` (23 values) | 23/23 | 0 |
//! | `gsl_poly_dd_taylor` (56 values, 2 to 10 nodes) | 56/56 | 0 |
//! | `gsl_poly_eval_derivs` (70 values, lenres below, equal and above lenc) | 56/70 | 3 ulp, 5.8e-16 relative |
//!
//! Not compared (not ported): complex eval and Hermite divided differences.
//!
//! Predictions against the run: `eval`, `dd_init` and `dd_eval` held. The
//! `eval_derivs` prediction held (the 14 differing values are the round-off
//! of the different operation order). **The `dd_taylor` prediction was
//! refuted:** PETIR's direct accumulation reproduces GSL's reversed-index
//! recurrence bit for bit on all 56 values. That is recorded as measured, not
//! as a deviation to hide; the two sequences happen to round the same on these
//! inputs, and no claim is made beyond them.
//!
//! # Skipping
//!
//! If the reference file is absent the test skips rather than fails.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use petir::poly::{eval, eval_derivs, DividedDifference};

#[derive(Default)]
struct Case {
    name: String,
    c: Vec<f64>,
    xs: Vec<f64>,
    eval: Vec<u64>,
    derivs: Vec<u64>,
    lenres: usize,
    xa: Vec<f64>,
    ya: Vec<f64>,
    dd: Vec<u64>,
    dd_eval: Vec<u64>,
    xp: Vec<f64>,
    taylor: Vec<Vec<u64>>,
    cur: Vec<u64>,
    deriv_x: f64,
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/poly-gsl-2.8-reference.txt")
}

fn bits(s: &str) -> u64 {
    u64::from_str_radix(s, 16).expect("hex bit pattern")
}

fn load() -> Option<Vec<Case>> {
    let text = fs::read_to_string(reference_path()).ok()?;
    let mut cases: Vec<Case> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(&tag) = f.first() else { continue };
        if tag == "CASE" {
            cases.push(Case {
                name: f[1].to_string(),
                ..Case::default()
            });
            continue;
        }
        let Some(c) = cases.last_mut() else { continue };
        match tag {
            "C" => c.c.push(f64::from_bits(bits(f[2]))),
            "XS" if c.name.starts_with("derivs") => c.deriv_x = f64::from_bits(bits(f[2])),
            "XS" => c.xs.push(f64::from_bits(bits(f[2]))),
            "XA" => c.xa.push(f64::from_bits(bits(f[2]))),
            "YA" => c.ya.push(f64::from_bits(bits(f[2]))),
            "EVAL" => c.eval.push(bits(f[2])),
            "DERIV" => c.derivs.push(bits(f[2])),
            "LENRES" => c.lenres = f[1].parse().unwrap(),
            "DD" => c.dd.push(bits(f[2])),
            "XP" => c.xp.push(f64::from_bits(bits(f[2]))),
            "TAYLOR" => c.cur.push(bits(f[2])),
            "TAYLOR_END" => c.taylor.push(std::mem::take(&mut c.cur)),
            _ => {}
        }
    }
    for c in &mut cases {
        // EVAL lines in dd cases are the dd_eval values, in eval cases the eval
        // values. Route them by case kind.
        if c.name.starts_with("dd_") {
            c.dd_eval = std::mem::take(&mut c.eval);
        }
    }
    Some(cases)
}

fn ulps(a: f64, b: f64) -> u64 {
    // Distance in units in the last place, over the ordered bit patterns.
    let key = |x: f64| {
        let b = x.to_bits() as i64;
        if b < 0 { i64::MIN.wrapping_sub(b) } else { b }
    };
    key(a).abs_diff(key(b))
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[derive(Default)]
struct Tally {
    total: usize,
    identical: usize,
    worst_rel: f64,
    worst_ulp: u64,
}

impl Tally {
    fn add(&mut self, got: f64, want_bits: u64) {
        let want = f64::from_bits(want_bits);
        self.total += 1;
        if got.to_bits() == want_bits {
            self.identical += 1;
        }
        self.worst_rel = self.worst_rel.max(rel(got, want));
        self.worst_ulp = self.worst_ulp.max(ulps(got, want));
    }
    fn report(&self, label: &str) {
        println!(
            "{label:>22}: {}/{} bit-identical, worst {} ulp, worst relative {:e}",
            self.identical, self.total, self.worst_ulp, self.worst_rel
        );
    }
}

/// Run every case once and tally each family separately.
fn run_all(cases: &[Case]) -> BTreeMap<&'static str, Tally> {
    let mut t: BTreeMap<&'static str, Tally> = BTreeMap::new();
    for c in cases {
        if c.name.starts_with("eval_") {
            let e = t.entry("eval").or_default();
            for (k, &x) in c.xs.iter().enumerate() {
                e.add(eval(&c.c, x), c.eval[k]);
            }
        } else if c.name.starts_with("derivs_") {
            let e = t.entry("eval_derivs").or_default();
            let got = eval_derivs(&c.c, c.deriv_x, c.lenres - 1);
            assert_eq!(got.len(), c.derivs.len(), "{}: length", c.name);
            for (&g, &w) in got.iter().zip(c.derivs.iter()) {
                e.add(g, w);
            }
        } else if c.name.starts_with("dd_") {
            let dd = DividedDifference::new(&c.xa, &c.ya).expect("dd_init");
            let e = t.entry("dd_init").or_default();
            for (&g, &w) in dd.coefficients().iter().zip(c.dd.iter()) {
                e.add(g, w);
            }
            let e = t.entry("dd_eval").or_default();
            for (k, &x) in c.xs.iter().enumerate() {
                e.add(dd.eval(x), c.dd_eval[k]);
            }
            let e = t.entry("dd_taylor").or_default();
            for (i, &xp) in c.xs_taylor().iter().enumerate() {
                let got = dd.to_taylor(xp);
                for (&g, &w) in got.iter().zip(c.taylor[i].iter()) {
                    e.add(g, w);
                }
            }
        }
    }
    t
}

impl Case {
    /// Expansion points for the Taylor blocks, in driver order.
    fn xs_taylor(&self) -> Vec<f64> {
        self.xp.clone()
    }
}

#[test]
fn poly_reproduces_compiled_gsl() {
    let Some(cases) = load() else {
        eprintln!("SKIP: {} not found", reference_path().display());
        return;
    };
    assert!(!cases.is_empty());
    let t = run_all(&cases);
    for (k, v) in &t {
        v.report(k);
    }
    // eval, dd_init, dd_eval and dd_taylor are pinned bit-identical. The
    // dd_taylor prediction (not identical) was refuted by the run; see above.
    for k in ["eval", "dd_init", "dd_eval", "dd_taylor"] {
        let v = &t[k];
        assert_eq!(v.identical, v.total, "{k}: must be bit-identical");
    }
    // eval_derivs is the one family that is not bit-identical. The bound is
    // round-off from the operation order (see the doc comment), and the
    // criterion is 1e-8 relative, as CLAUDE.md allows for explained round-off.
    let d = &t["eval_derivs"];
    assert!(d.worst_rel < 1e-8, "eval_derivs worst relative {:e}", d.worst_rel);
}
