// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **V&V — `petir::multiroots` code-to-code against GSL 2.8 compiled and run
//! (gh:#1338 case 2, epic gh:#1336).**
//!
//! # What is ported, so what is compared
//!
//! `petir::multiroots` ports **only** `gsl_multiroot_fdjacobian`
//! (`multiroots/fdjac.c`). The solvers `dnewton`, `hybrid(s)`, `hybridj(s)`,
//! `newton`, `gnewton` and `broyden` are **not ported** (module docs), so
//! there is nothing to compare for them; this file does not pretend
//! otherwise. What is compared:
//!
//! 1. **`fdjacobian`, unit level.** Reference
//!    `reference-data/gsl/multiroots-fdjac-gsl-2.8-reference.txt`, written by
//!    `multiroots_fdjac_reference_driver.c`, which calls the real
//!    `gsl_multiroot_fdjacobian` on the problems of GSL's own
//!    `multiroots/test_funcs.c` (Rosenbrock, Roth, Powell badly scaled, Brown
//!    badly scaled, Powell singular, Wood, Helical) at the test initial point
//!    scaled by 1, 10, 0.1 and 0 (the all-zero `x`, which exercises the
//!    `dx = epsrel` branch), for `epsrel = sqrt(DBL_EPSILON)` and `1e-4`
//!    (56 Jacobians), plus two functions that give a null column. Every `x`,
//!    `f(x)` and every Jacobian entry is compared as IEEE bits, and GSL's
//!    status (`GSL_ESING` = 21) against petir's `null_columns > 0`.
//!    The problems' right-hand sides are re-typed in Rust here; the `F` rows
//!    of the reference check each transcription bit for bit before the
//!    Jacobian is trusted.
//! 2. **Integration: a whole `dnewton` solve.** GSL's
//!    `gsl_multiroot_fsolver_dnewton` is run iterate by iterate (`x`, `f`,
//!    `dx`, 15 iterations on six problems). The test re-composes `dnewton`
//!    (`dnewton.c:95-170`: fdjacobian, LU decompose, LU solve, `x -= dx`,
//!    re-evaluate, fdjacobian) from petir's `fdjacobian` and
//!    `linalg::lu_decomp/lu_solve`; the 12-line step is test-local
//!    transcription, so this pins the composition of the two ported pieces,
//!    not a ported solver.
//!
//! # Predicted agreement (written before running, 2026-11-10)
//!
//! Bit-identical everywhere. `fdjacobian` is `(g1 - g0) / dx` with the same
//! `dx`; the right-hand sides use `sqrt`, `exp`, `atan`, `pow(.,2.0)` which in
//! both languages resolve to the platform glibc (Rust `std` on
//! `x86_64-unknown-linux-gnu`), and gcc `-O2` without `-march` emits no FMA.
//! petir's LU is already bit-identical to GSL's (`gsl_lu_code_to_code.rs`).
//! A difference would therefore be a real finding. One designed difference
//! that no GSL test function triggers: petir returns `Domain` for a
//! non-finite perturbed `f` where GSL asks the *function* for a status.
//!
//! # Results (2026-11-10, rustc via `rust-toolchain.toml`, GSL 2.8 gcc -O2)
//!
//! See each test's doc comment; the aggregate is in
//! `crates/petir/docs/verification-summary.md`.
//!
//! # Skipping
//!
//! There is none: the reference is `include_str!`-ed, so a missing file is a
//! compile error, not a silent pass.

use std::collections::BTreeMap;

use petir::linalg::{lu_decomp, lu_solve, Matrix};
use petir::multiroots::{fdjacobian, GSL_SQRT_DBL_EPSILON};

const REFERENCE: &str = include_str!("../../../reference-data/gsl/multiroots-fdjac-gsl-2.8-reference.txt");

type Rows = Vec<(String, Vec<String>)>;

fn load() -> BTreeMap<String, Rows> {
    let mut out: BTreeMap<String, Rows> = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("CASE") => {
                let name = f[1].to_string();
                out.insert(name.clone(), Vec::new());
                cur = Some(name);
            }
            Some("END") => cur = None,
            Some(tag) => {
                if let Some(c) = &cur {
                    out.get_mut(c)
                        .unwrap()
                        .push((tag.to_string(), f[1..].iter().map(|s| s.to_string()).collect()));
                }
            }
            None => {}
        }
    }
    out
}

fn bits(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}

fn col(rows: &Rows, tag: &str) -> Vec<f64> {
    rows.iter().filter(|(t, _)| t == tag).map(|(_, v)| bits(&v[0])).collect()
}

fn status(rows: &Rows) -> i32 {
    rows.iter().find(|(t, _)| t == "STATUS").unwrap().1[0].parse().unwrap()
}

#[derive(Default)]
struct Tally {
    n: usize,
    same: usize,
    worst: f64,
}
impl Tally {
    fn check(&mut self, what: &str, got: f64, want: f64) {
        self.n += 1;
        if got.to_bits() == want.to_bits() {
            self.same += 1;
        } else {
            let d = (got - want).abs() / got.abs().max(want.abs());
            self.worst = self.worst.max(d);
            eprintln!("DIFF {what}: petir {got:e} gsl {want:e}");
        }
    }
}

// ---- the GSL test functions (multiroots/test_funcs.c), transcribed ----

type Fun = fn(&[f64], &mut [f64]);

fn rosenbrock(x: &[f64], f: &mut [f64]) {
    f[0] = 1.0 - x[0];
    f[1] = 10.0 * (x[1] - x[0] * x[0]);
}
fn roth(x: &[f64], f: &mut [f64]) {
    f[0] = -13.0 + x[0] + ((5.0 - x[1]) * x[1] - 2.0) * x[1];
    f[1] = -29.0 + x[0] + ((x[1] + 1.0) * x[1] - 14.0) * x[1];
}
fn powellscal(x: &[f64], f: &mut [f64]) {
    f[0] = 10000.0 * x[0] * x[1] - 1.0;
    f[1] = (-x[0]).exp() + (-x[1]).exp() - 1.0001;
}
fn brownscal(x: &[f64], f: &mut [f64]) {
    f[0] = x[0] - 1e6;
    f[1] = x[0] * x[1] - 2.0;
}
fn powellsing(x: &[f64], f: &mut [f64]) {
    f[0] = x[0] + 10.0 * x[1];
    f[1] = 5.0_f64.sqrt() * (x[2] - x[3]);
    f[2] = (x[1] - 2.0 * x[2]).powf(2.0);
    f[3] = 10.0_f64.sqrt() * (x[0] - x[3]).powf(2.0);
}
fn wood(x: &[f64], f: &mut [f64]) {
    let t1 = x[1] - x[0] * x[0];
    let t2 = x[3] - x[2] * x[2];
    f[0] = -200.0 * x[0] * t1 - (1.0 - x[0]);
    f[1] = 200.0 * t1 + 20.2 * (x[1] - 1.0) + 19.8 * (x[3] - 1.0);
    f[2] = -180.0 * x[2] * t2 - (1.0 - x[2]);
    f[3] = 180.0 * t2 + 20.2 * (x[3] - 1.0) + 19.8 * (x[1] - 1.0);
}
fn helical(x: &[f64], f: &mut [f64]) {
    let t1 = if x[0] > 0.0 {
        (x[1] / x[0]).atan() / (2.0 * std::f64::consts::PI)
    } else if x[0] < 0.0 {
        0.5 + (x[1] / x[0]).atan() / (2.0 * std::f64::consts::PI)
    } else {
        0.25 * if x[1] > 0.0 { 1.0 } else { -1.0 }
    };
    let t2 = (x[0] * x[0] + x[1] * x[1]).sqrt();
    f[0] = 10.0 * (x[2] - 10.0 * t1);
    f[1] = 10.0 * (t2 - 1.0);
    f[2] = x[2];
}
fn nullcol(x: &[f64], f: &mut [f64]) {
    f[0] = 2.0 * x[0];
    f[1] = -x[0];
}
fn absorb(x: &[f64], f: &mut [f64]) {
    f[0] = 1e30 + x[0];
    f[1] = x[1] * x[1];
}

fn fun_for(prefix: &str) -> Fun {
    match prefix {
        "rosenbrock" => rosenbrock,
        "roth" => roth,
        "powellscal" => powellscal,
        "brownscal" => brownscal,
        "powellsing" => powellsing,
        "wood" => wood,
        "helical" => helical,
        "nullcol" => nullcol,
        "absorb" => absorb,
        other => panic!("unknown problem {other}"),
    }
}

/// Compare every `fdjacobian` case of one problem; returns the tally.
fn fdjac_family(prefix: &str) -> Tally {
    let cases = load();
    let mut t = Tally::default();
    let mut ncases = 0;
    for (name, rows) in &cases {
        let Some(rest) = name.strip_prefix(prefix) else { continue };
        if !rest.starts_with("_s") {
            continue; // dnewton_* trajectories and other families
        }
        ncases += 1;
        let eps: f64 = {
            // the header's 4th field is epsrel with %.17g; re-read it from the name's text
            let line = REFERENCE.lines().find(|l| l.split_whitespace().nth(1) == Some(name)).unwrap();
            line.split_whitespace().nth(3).unwrap().parse().unwrap()
        };
        let f = fun_for(prefix);
        let x = col(rows, "X");
        let fx_ref = col(rows, "F");
        let n = x.len();
        let mut fx = vec![0.0; n];
        f(&x, &mut fx);
        for i in 0..n {
            t.check(&format!("{name} f[{i}] (transcription)"), fx[i], fx_ref[i]);
        }
        let mut j = Matrix::zeros(n, n).unwrap();
        let rep = fdjacobian(f, &x, &fx, eps, &mut j).unwrap();
        let jref = col(rows, "J");
        for r in 0..n {
            for c in 0..n {
                t.check(&format!("{name} J[{r}][{c}]"), j.get(r, c), jref[r * n + c]);
            }
        }
        // GSL_ESING (21) <=> a null column
        let gsl_esing = status(rows) == 21;
        t.n += 1;
        if gsl_esing == (rep.null_columns > 0) {
            t.same += 1;
        } else {
            panic!("{name}: GSL status {} vs petir null_columns {}", status(rows), rep.null_columns);
        }
    }
    assert!(ncases > 0, "no reference cases for {prefix}");
    eprintln!(
        "fdjacobian {prefix}: {ncases} cases, {}/{} bit-identical, worst rel {:e}",
        t.same, t.n, t.worst
    );
    t
}

fn assert_bit_identical(t: &Tally) {
    assert_eq!(t.same, t.n, "worst relative difference {:e}", t.worst);
}

/// Rosenbrock, `fdjacobian`: 8 cases (4 scales x 2 `epsrel`), bit-identical
/// (results above 2026-11-10: see `verification-summary.md`).
#[test]
fn fdjacobian_rosenbrock_matches_gsl() {
    assert_bit_identical(&fdjac_family("rosenbrock"));
}
/// Roth, `fdjacobian`: 8 cases, bit-identical.
#[test]
fn fdjacobian_roth_matches_gsl() {
    assert_bit_identical(&fdjac_family("roth"));
}
/// Powell badly scaled (`exp`), `fdjacobian`: 8 cases, bit-identical.
#[test]
fn fdjacobian_powell_badly_scaled_matches_gsl() {
    assert_bit_identical(&fdjac_family("powellscal"));
}
/// Brown badly scaled, `fdjacobian`: 8 cases, bit-identical, including the
/// all-zero `x` where GSL returns `GSL_ESING` (a null column) and petir
/// reports `null_columns > 0`.
#[test]
fn fdjacobian_brown_badly_scaled_matches_gsl() {
    assert_bit_identical(&fdjac_family("brownscal"));
}
/// Powell singular (`sqrt`, `pow`), `fdjacobian`: 8 cases, bit-identical.
#[test]
fn fdjacobian_powell_singular_matches_gsl() {
    assert_bit_identical(&fdjac_family("powellsing"));
}
/// Wood, `fdjacobian`: 8 cases, bit-identical.
#[test]
fn fdjacobian_wood_matches_gsl() {
    assert_bit_identical(&fdjac_family("wood"));
}
/// Helical valley (`atan`, `sqrt`; the `x0 == 0` branch is hit by the
/// all-zero point), `fdjacobian`: 8 cases, bit-identical.
#[test]
fn fdjacobian_helical_matches_gsl() {
    assert_bit_identical(&fdjac_family("helical"));
}
/// The `GSL_ESING` paths: a function independent of `x[1]` and one where
/// `1e30 + x0` absorbs the step. Both leave a zero column; GSL returns 21
/// after filling the whole matrix, petir reports `null_columns` and fills it
/// the same way. Bit-identical Jacobians.
#[test]
fn fdjacobian_null_column_cases_match_gsl() {
    let a = fdjac_family("nullcol");
    let b = fdjac_family("absorb");
    assert_bit_identical(&a);
    assert_bit_identical(&b);
}

/// Integration: GSL's `dnewton` solve, iterate by iterate, against the same
/// algorithm composed from petir's `fdjacobian` + LU (see the file header).
/// Six problems x 15 iterations x (`x`, `f`, `dx`).
#[test]
fn dnewton_trajectories_match_gsl() {
    let cases = load();
    let mut t = Tally::default();
    for (name, rows) in &cases {
        let Some(prob) = name.strip_prefix("dnewton_") else { continue };
        let f = fun_for(prob);
        // n from the header
        let line = REFERENCE.lines().find(|l| l.split_whitespace().nth(1) == Some(name)).unwrap();
        let n: usize = line.split_whitespace().nth(2).unwrap().parse().unwrap();
        // initial point of the GSL tests
        let mut x: Vec<f64> = match prob {
            "rosenbrock" => vec![-1.2, 1.0],
            "roth" => vec![4.5, 3.5],
            "powellscal" => vec![0.0, 1.0],
            "powellsing" => vec![3.0, -1.0, 0.0, 1.0],
            "wood" => vec![-3.0, -1.0, -3.0, -1.0],
            "helical" => vec![-1.0, 0.0, 0.0],
            _ => unreachable!(),
        };
        let xs = col(rows, "X");
        let fs = col(rows, "F");
        let dxs = col(rows, "DX");
        let iters = xs.len() / n;
        assert!(iters >= 1, "{name}: GSL iterated nothing");

        let mut fv = vec![0.0; n];
        f(&x, &mut fv);
        let mut jac = Matrix::zeros(n, n).unwrap();
        fdjacobian(f, &x, &fv, GSL_SQRT_DBL_EPSILON, &mut jac).unwrap();
        for k in 0..iters {
            let mut lu = Matrix::zeros(n, n).unwrap();
            for r in 0..n {
                for c in 0..n {
                    lu.set(r, c, jac.get(r, c));
                }
            }
            let p = lu_decomp(&mut lu).unwrap();
            let e = lu_solve(&lu, &p, &fv).unwrap();
            let mut dx = vec![0.0; n];
            for i in 0..n {
                dx[i] = -e[i];
                x[i] -= e[i];
            }
            f(&x, &mut fv);
            fdjacobian(f, &x, &fv, GSL_SQRT_DBL_EPSILON, &mut jac).unwrap();
            for i in 0..n {
                t.check(&format!("{name} it{} x[{i}]", k + 1), x[i], xs[k * n + i]);
                t.check(&format!("{name} it{} f[{i}]", k + 1), fv[i], fs[k * n + i]);
                t.check(&format!("{name} it{} dx[{i}]", k + 1), dx[i], dxs[k * n + i]);
            }
        }
    }
    eprintln!("dnewton: {}/{} bit-identical, worst rel {:e}", t.same, t.n, t.worst);
    assert!(t.n > 0);
    assert_bit_identical(&t);
}
