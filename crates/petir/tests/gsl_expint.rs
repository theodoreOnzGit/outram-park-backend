//! **V&V — `expint_e1` against GSL 2.8 compiled and run.**
//!
//! `E_1` is six Chebyshev series selected by range and nothing else, so this is
//! simultaneously the verification of the port and of PETIR's Chebyshev
//! machinery under real coefficient tables. It is the first half of what ACER's
//! MF=5 `LF = 12` (Madland-Nix) needs — see GitHub #201.
//!
//! The reference is `reference-data/gsl/expint-e1-gsl-2.8-reference.txt`,
//! produced by compiling GSL's `specfunc/expint.c` with gcc and running it; the
//! driver is committed beside it so the file can be regenerated rather than
//! trusted.
//!
//! All 150 coefficients were extracted from `specfunc/expint.c` by script
//! rather than retyped, and this test is what proves the extraction landed
//! correctly.

use petir::{expint_e1, expint_e1_scaled};
use std::path::PathBuf;

fn reference() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/expint-e1-gsl-2.8-reference.txt");
    p.exists().then_some(p)
}

#[test]
fn expint_e1_reproduces_gsl_2_8() {
    let Some(path) = reference() else {
        eprintln!("skipping: reference-data/gsl/ not present");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("reference readable");

    let (mut n, mut exact) = (0usize, 0usize);
    let (mut worst_rel, mut worst_at, mut worst_kind) = (0.0f64, 0.0f64, "");
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let kind = it.next().unwrap_or("");
        let v: Vec<f64> = it.map(|s| s.parse().expect("numeric")).collect();
        if v.len() < 3 {
            continue;
        }
        let (x, gsl_val) = (v[0], v[1]);
        let ours = match kind {
            "e1" => expint_e1(x),
            "e1s" => expint_e1_scaled(x),
            _ => continue,
        };
        let (val, _err) = ours.unwrap_or_else(|e| panic!("{kind}({x}) -> {e}"));
        n += 1;
        if val.to_bits() == gsl_val.to_bits() {
            exact += 1;
        }
        let rel = if gsl_val == 0.0 {
            val.abs()
        } else {
            (val - gsl_val).abs() / gsl_val.abs()
        };
        if rel > worst_rel {
            worst_rel = rel;
            worst_at = x;
            worst_kind = if kind == "e1" { "E1" } else { "E1_scaled" };
        }
    }

    assert!(n > 400, "only {n} values compared — reference truncated?");
    println!(
        "  vs GSL 2.8: {n} values, {exact} bit-identical ({:.1} %), worst rel {worst_rel:.3e} \
         on {worst_kind} at x = {worst_at}",
        100.0 * exact as f64 / n as f64
    );
    // Bounded by the libm-vs-glibc difference in exp/log, which E1 uses outside
    // the Chebyshev series themselves.
    assert!(
        worst_rel < 1.0e-13,
        "expint_e1 differs from GSL by {worst_rel:.3e} relative on {worst_kind} at x = {worst_at}"
    );
}

/// The error estimates must be returned and finite — upstream carries them
/// through every branch and a caller may rely on them.
#[test]
fn error_estimates_are_finite_and_nonnegative() {
    for k in -40..=40 {
        let x = f64::from(k) * 0.7;
        if x == 0.0 {
            continue;
        }
        if let Ok((v, e)) = expint_e1(x) {
            assert!(v.is_finite(), "E1({x}) = {v}");
            assert!(e.is_finite() && e >= 0.0, "E1({x}) err = {e}");
        }
    }
}

/// The documented refusals, which upstream signals through its error handler.
#[test]
fn domain_and_range_failures_are_reported_not_fatal() {
    assert!(expint_e1(0.0).is_err(), "E1(0) diverges");
    // Far below -xmax: upstream's OVERFLOW_ERROR.
    assert!(expint_e1(-1.0e4).is_err(), "E1 overflows for very negative x");
}
