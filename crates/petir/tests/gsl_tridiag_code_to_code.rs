// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V: `petir::linalg::solve_symm_tridiag` code-to-code against GSL 2.8
//! compiled and run** (gh:#1338, case 3).
//!
//! # Methodology
//!
//! GSL 2.8's `gsl_linalg_solve_symm_tridiag` (`linalg/tridiag.c`, the Cholesky
//! `A = L D L^T` sweep with forward and back substitution), built as a static
//! library from the vendored tree, was run on seven cases by
//! `reference-data/gsl/tridiag_reference_driver.c`. The output is committed at
//! `reference-data/gsl/tridiag-gsl-2.8-reference.txt`, and it carries every
//! input (diagonal, off-diagonal, right-hand side) as IEEE bit patterns, so this
//! test reads the inputs from the file rather than regenerating them.
//!
//! Cases: diagonally dominant systems of order 3, 5, 10, 50 and 200 (inputs
//! from an integer LCG divided by a power of two, so exact); a near-singular
//! order-10 Neumann Laplacian with its last diagonal lifted by `2^-40`
//! (condition number about `2^40`); and an exact zero leading pivot.
//!
//! Only the symmetric solver is ported. GSL's nonsymmetric and cyclic
//! tridiagonal routines have no PETIR counterpart and are not compared.
//!
//! **Pass criterion:** for every case with `STATUS 0`, every solution element
//! is bit-identical to GSL's; for the zero-pivot case, PETIR must return an
//! error where GSL returns a nonzero status.
//!
//! **Prediction, written before the Rust test was run:** bit-identical in every
//! solution element. The routine calls no transcendental function, and the port
//! performs the same operations in the same order (`alpha`, `gamma`, forward
//! sweep, `c = z / alpha`, back sweep), with the same operand order for each
//! multiply and subtract. The zero-pivot case is expected to agree on the error.
//!
//! # Results
//!
//! **Result (2026-11-10, release, `CARGO_BUILD_JOBS=1`, GSL 2.8 `cf180cd7`
//! static build, gcc -O2):** 278/278 solution elements **bit-identical**
//! across all six solved cases (n = 3, 5, 10, 50, 200 and the near-singular
//! n = 10, pivot 2^-40). The zero-pivot case returns GSL status 12
//! (`GSL_EZERODIV`) and PETIR an error, so they agree.
//! The prediction held; no difference to explain.
//!
//! Interpretation: the symmetric solver port reproduces GSL's arithmetic, not
//! only its answer, including the sign and order of the forward and back
//! sweeps. The nonsymmetric and cyclic GSL routines are not ported and are not
//! covered by this result.
//!
//! # Skipping
//!
//! If the reference file is absent the test skips rather than fails.

use std::fs;
use std::path::PathBuf;

use petir::linalg::solve_symm_tridiag;

#[derive(Default)]
struct Case {
    name: String,
    n: usize,
    diag: Vec<u64>,
    off: Vec<u64>,
    rhs: Vec<u64>,
    status: i32,
    x: Vec<u64>,
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/tridiag-gsl-2.8-reference.txt")
}

fn parse_bits(s: &str) -> u64 {
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
                n: f[2].parse().unwrap(),
                ..Case::default()
            });
            continue;
        }
        let Some(c) = cases.last_mut() else { continue };
        match tag {
            "D" => c.diag.push(parse_bits(f[2])),
            "O" => c.off.push(parse_bits(f[2])),
            "B" => c.rhs.push(parse_bits(f[2])),
            "STATUS" => c.status = f[1].parse().unwrap(),
            "X" => c.x.push(parse_bits(f[2])),
            _ => {}
        }
    }
    Some(cases)
}

fn f(bits: &[u64]) -> Vec<f64> {
    bits.iter().map(|&b| f64::from_bits(b)).collect()
}

/// Compare every ported routine's output with GSL's for each reference case.
#[test]
fn symm_tridiag_reproduces_compiled_gsl() {
    let Some(cases) = load() else {
        eprintln!("SKIP: {} not found", reference_path().display());
        return;
    };
    assert_eq!(cases.len(), 7, "the driver emits seven cases");

    let (mut total, mut identical) = (0_usize, 0_usize);
    for c in &cases {
        assert_eq!(c.diag.len(), c.n, "{}: diagonal length", c.name);
        assert_eq!(c.rhs.len(), c.n, "{}: rhs length", c.name);
        let diag = f(&c.diag);
        let off = f(&c.off);
        let rhs = f(&c.rhs);

        let got = solve_symm_tridiag(&diag, &off, &rhs);
        if c.status != 0 {
            assert!(got.is_err(), "{}: GSL status {} but PETIR returned Ok", c.name, c.status);
            println!("{:>18} n={:>3}: GSL status {}, PETIR error (agree)", c.name, c.n, c.status);
            continue;
        }
        let x = got.unwrap_or_else(|e| panic!("{}: PETIR error {e:?}, GSL status 0", c.name));
        assert_eq!(x.len(), c.x.len(), "{}: solution length", c.name);

        let mut case_identical = 0;
        for (i, (&g, &want)) in x.iter().zip(c.x.iter()).enumerate() {
            total += 1;
            if g.to_bits() == want {
                identical += 1;
                case_identical += 1;
            } else {
                panic!("{}: x[{i}] differs: got {g:e}, GSL {:e}", c.name, f64::from_bits(want));
            }
        }
        println!(
            "{:>18} n={:>3}: {}/{} bit-identical",
            c.name,
            c.n,
            case_identical,
            c.x.len()
        );
    }
    println!("symm tridiag: {identical}/{total} solution elements bit-identical");
    assert_eq!(identical, total, "every solution element must be bit-identical");
}

/// The single-case entry points, one test fn per reference case, so a failure
/// names the case.
fn check_one(name: &str) {
    let Some(cases) = load() else {
        eprintln!("SKIP: {} not found", reference_path().display());
        return;
    };
    let c = cases
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"));
    let got = solve_symm_tridiag(&f(&c.diag), &f(&c.off), &f(&c.rhs));
    if c.status != 0 {
        assert!(got.is_err(), "{name}: expected error");
        return;
    }
    let x = got.expect("PETIR solve must succeed where GSL does");
    for (i, (&g, &w)) in x.iter().zip(c.x.iter()).enumerate() {
        assert_eq!(g.to_bits(), w, "{name}: x[{i}]");
    }
}

#[test]
fn rand3() {
    check_one("rand3");
}

#[test]
fn rand5() {
    check_one("rand5");
}

#[test]
fn rand10() {
    check_one("rand10");
}

#[test]
fn rand50() {
    check_one("rand50");
}

#[test]
fn rand200() {
    check_one("rand200");
}

#[test]
fn near_singular10() {
    check_one("near_singular10");
}

#[test]
fn zero_pivot3() {
    check_one("zero_pivot3");
}
