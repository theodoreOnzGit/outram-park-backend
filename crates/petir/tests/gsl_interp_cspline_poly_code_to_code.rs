// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::interp_cspline_poly` (periodic cubic spline, polynomial)
//! against compiled GSL 2.8** (gh:#1341). Methodology, reference and
//! predictions P7-P8: `verification_and_validation/interp_piecewise_gh1341.md`.
//!
//! Replays `reference-data/gsl/interp-cspline-poly-gsl-2.8-reference.txt`
//! (driver `interp_cspline_poly_reference_driver.c`). Each `INIT` row checks
//! that construction fails exactly when `gsl_interp_init` does (`per2`: GSL's
//! cyclic solver refuses a size-1 system, status 19); each `E/D/D2/I` row is
//! compared as a bit pattern. The polynomial integral calls `pow`, which is
//! glibc in GSL and PETIR's in the port, so its rows are counted and the
//! count pinned; every other row must be bit-identical.

use petir::interp_cspline_poly::{PeriodicSpline, PolynomialInterp};
use std::collections::BTreeMap;

const REFERENCE: &str =
    include_str!("../../../reference-data/gsl/interp-cspline-poly-gsl-2.8-reference.txt");

enum Model {
    Spline(PeriodicSpline),
    Poly(PolynomialInterp),
}

impl Model {
    fn eval(&self, x: f64) -> f64 {
        match self {
            Model::Spline(s) => s.eval(x).unwrap(),
            Model::Poly(p) => p.eval(x).unwrap(),
        }
    }
    fn deriv(&self, x: f64) -> f64 {
        match self {
            Model::Spline(s) => s.eval_deriv(x).unwrap(),
            Model::Poly(p) => p.eval_deriv(x).unwrap(),
        }
    }
    fn deriv2(&self, x: f64) -> f64 {
        match self {
            Model::Spline(s) => s.eval_deriv2(x).unwrap(),
            Model::Poly(p) => p.eval_deriv2(x).unwrap(),
        }
    }
    fn integ(&self, a: f64, b: f64) -> f64 {
        match self {
            Model::Spline(s) => s.eval_integ(a, b).unwrap(),
            Model::Poly(p) => p.eval_integ(a, b).unwrap(),
        }
    }
}

#[test]
fn periodic_spline_and_polynomial_match_compiled_gsl() {
    let mut tables: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    let mut models: BTreeMap<(String, String), Model> = BTreeMap::new();
    let mut tally: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
    let mut inits = 0;
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[0] == "T" {
            let n: usize = f[2].parse().unwrap();
            let v: Vec<f64> = f[4..].iter().map(|s| s.parse().unwrap()).collect();
            assert_eq!(v.len(), 2 * n);
            let xs: Vec<f64> = v.iter().step_by(2).copied().collect();
            let ys: Vec<f64> = v.iter().skip(1).step_by(2).copied().collect();
            tables.insert(f[1].to_string(), (xs, ys));
            continue;
        }
        if f[0] == "INIT" {
            let (m, t) = (f[1], f[2]);
            let status: i32 = f[3].parse().unwrap();
            let (xs, ys) = tables.get(t).expect("table row first");
            let built = match m {
                "cspline_periodic" => PeriodicSpline::new(xs, ys).map(Model::Spline),
                "polynomial" => PolynomialInterp::new(xs, ys).map(Model::Poly),
                other => panic!("unknown method {other}"),
            };
            assert_eq!(built.is_err(), status != 0, "{m} {t} init status {status}");
            if let Ok(model) = built {
                models.insert((m.to_string(), t.to_string()), model);
            }
            inits += 1;
            continue;
        }
        let (m, t, op) = (f[0], f[1], f[2]);
        let model = models.get(&(m.to_string(), t.to_string())).expect("model");
        let nums: Vec<f64> = f[3..].iter().map(|s| s.parse().unwrap()).collect();
        let (got, want) = match op {
            "E" => (model.eval(nums[0]), nums[1]),
            "D" => (model.deriv(nums[0]), nums[1]),
            "D2" => (model.deriv2(nums[0]), nums[1]),
            "I" => (model.integ(nums[0], nums[1]), nums[2]),
            other => panic!("unknown op {other}"),
        };
        let e = tally.entry((m.to_string(), op.to_string())).or_default();
        e.0 += 1;
        if got.to_bits() == want.to_bits() {
            e.1 += 1;
        } else if std::env::var_os("SPECFUNC_SHOW_DIFFS").is_some() {
            println!("DIFF {line}: ours {got:e}");
        }
    }
    let (mut rows, mut exact) = (0, 0);
    for ((m, op), (r, x)) in &tally {
        println!("  {m} {op}: {x}/{r} bit-identical");
        rows += r;
        exact += x;
    }
    println!("  interp_cspline_poly vs GSL 2.8: {exact}/{rows} bit-identical; {inits} INIT rows");
    assert_eq!(inits, 11);
    for ((m, op), (r, x)) in &tally {
        if m == "polynomial" && op == "I" {
            continue; // pow: counted, pinned below
        }
        assert_eq!(x, r, "{m} {op}: every row must be bit-identical");
    }
    // The polynomial integral calls `pow(b, i + 1)`. Measured 2026-11-13:
    // 1024 of 1155 rows are bit-identical (the prediction was >= 90 %: it
    // FAILED at 88.7 %). DEMONSTRATED to be `pow`: GSL's driver relinked with
    // PETIR's `pow` alone (diagnostic shim, not committed) makes all 9 726
    // rows bit-identical to this port. Accepted per AGENTS.md; pinned.
    assert_eq!(
        tally.get(&("polynomial".to_string(), "I".to_string())),
        Some(&(1155, 1024))
    );
}
