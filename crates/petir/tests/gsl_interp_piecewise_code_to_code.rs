//! **V&V: `petir::interp_piecewise` (Steffen, Akima, periodic Akima) against
//! compiled GSL 2.8** (gh:#1341).
//!
//! Methodology, reference and prediction: see
//! `verification_and_validation/interp_piecewise_gh1341.md`. Each row of
//! `reference-data/gsl/interp-piecewise-gsl-2.8-reference.txt` is replayed
//! through the port and compared as a bit pattern; the tables come from the
//! file's own `T` rows (GSL's inputs). Pass criterion: every row identical.
//! Results are in the V&V record (date 2026-11-13).

use petir::interp_piecewise::{PiecewiseInterp, PiecewiseKind};
use std::collections::BTreeMap;

const REFERENCE: &str =
    include_str!("../../../reference-data/gsl/interp-piecewise-gsl-2.8-reference.txt");

fn kind_of(name: &str) -> PiecewiseKind {
    match name {
        "steffen" => PiecewiseKind::Steffen,
        "akima" => PiecewiseKind::Akima,
        "akima_periodic" => PiecewiseKind::AkimaPeriodic,
        other => panic!("unknown method {other}"),
    }
}

#[test]
fn steffen_and_akima_are_bit_identical_to_gsl() {
    let mut tables: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    let mut interps: BTreeMap<(String, String), PiecewiseInterp> = BTreeMap::new();
    // (method, op) -> (rows, exact)
    let mut tally: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
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
        let (m, t, op) = (f[0], f[1], f[2]);
        let (xs, ys) = tables.get(t).expect("table row first");
        let it = interps
            .entry((m.to_string(), t.to_string()))
            .or_insert_with(|| PiecewiseInterp::new(kind_of(m), xs, ys).unwrap());
        let nums: Vec<f64> = f[3..].iter().map(|s| s.parse().unwrap()).collect();
        let (got, want) = match op {
            "E" => (it.eval(nums[0]).unwrap(), nums[1]),
            "D" => (it.eval_deriv(nums[0]).unwrap(), nums[1]),
            "D2" => (it.eval_deriv2(nums[0]).unwrap(), nums[1]),
            "I" => (it.eval_integ(nums[0], nums[1]).unwrap(), nums[2]),
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
    println!("  interp_piecewise vs GSL 2.8: {exact}/{rows} bit-identical");
    assert_eq!(exact, rows, "every row must be bit-identical (prediction P1)");
}

/// P2: Steffen stays inside the data range of a monotone table, on a grid
/// that shares nothing with the reference file.
#[test]
fn steffen_is_monotone_on_a_monotone_table() {
    let x = [0.0, 0.5, 2.0, 2.2, 5.0, 9.0];
    let y = [0.0, 0.1, 0.1, 3.0, 3.1, 10.0];
    let s = PiecewiseInterp::new(PiecewiseKind::Steffen, &x, &y).unwrap();
    let mut prev = f64::NEG_INFINITY;
    for k in 0..=9000 {
        let v = s.eval(k as f64 * 0.001).unwrap();
        assert!(v >= prev - 1e-13 && (0.0..=10.0).contains(&v));
        prev = v;
    }
}
