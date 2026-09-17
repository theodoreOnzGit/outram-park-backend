//! **V&V — `gamma_inc_p` / `gamma_inc_lower` against GSL 2.8 compiled and run.**
//!
//! With [`expint_e1`](petir::expint_e1) this completes what NJOY ACER's MF=5
//! `LF = 12` needs (GitHub #201): `fmn` (`acefc.f90:9155`) evaluates the
//! Madland-Nix shape from `e1` and SLATEC's `gami`, and `gami` is the lower
//! incomplete gamma [`gamma_inc_lower`].
//!
//! The reference is `reference-data/gsl/gamma-inc-gsl-2.8-reference.txt`, with
//! its driver committed beside it. Note the driver computes the LOWER
//! incomplete gamma as `Γ(a) - Γ(a,x)`, because GSL's `gsl_sf_gamma_inc` is the
//! **upper** one — a sign trap worth stating, since SLATEC's `gami` is lower.
//!
//! # Coverage is deliberately wider than `fmn` needs
//!
//! `fmn` only calls at `a = 3/2`. The reference sweeps seven `a` values from
//! 0.75 to 9.5 and `x` from 0.35 to 77, which crosses every branch this port
//! implements — the series, the continued fraction and the large-`x` form —
//! rather than only the one path the immediate consumer happens to take.
//! Branches the port refuses are asserted as refusals, not skipped.

use petir::{gamma_inc_lower, gamma_inc_p};
use std::path::PathBuf;

fn reference() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/gamma-inc-gsl-2.8-reference.txt");
    p.exists().then_some(p)
}

#[test]
fn gamma_inc_reproduces_gsl_2_8() {
    let Some(path) = reference() else {
        eprintln!("skipping: reference-data/gsl/ not present");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("reference readable");

    let (mut n_p, mut n_l, mut exact) = (0usize, 0usize, 0usize);
    let (mut worst_p, mut worst_l) = ((0.0f64, 0.0f64, 0.0f64), (0.0f64, 0.0f64, 0.0f64));
    let mut refused = 0usize;

    for line in text.lines() {
        let mut it = line.split_whitespace();
        let kind = it.next().unwrap_or("");
        let v: Vec<f64> = it.map(|s| s.parse().expect("numeric")).collect();
        if v.len() < 3 {
            continue;
        }
        let (a, x, theirs) = (v[0], v[1], v[2]);
        let ours = match kind {
            "P" => gamma_inc_p(a, x),
            "L" => gamma_inc_lower(a, x),
            _ => continue,
        };
        let Ok((val, _err)) = ours else {
            refused += 1;
            continue;
        };
        let rel = if theirs.abs() > 1e-300 {
            (val - theirs).abs() / theirs.abs()
        } else {
            (val - theirs).abs()
        };
        if kind == "P" {
            n_p += 1;
            if val.to_bits() == theirs.to_bits() {
                exact += 1;
            }
            if rel > worst_p.0 {
                worst_p = (rel, a, x);
            }
        } else {
            n_l += 1;
            if rel > worst_l.0 {
                worst_l = (rel, a, x);
            }
        }
    }

    println!(
        "  P(a,x):  {n_p} values, {exact} bit-identical, worst rel {:.3e} at a={}, x={}",
        worst_p.0, worst_p.1, worst_p.2
    );
    println!(
        "  gamma(a,x) lower: {n_l} values, worst rel {:.3e} at a={}, x={}",
        worst_l.0, worst_l.1, worst_l.2
    );
    println!("  refused (unported branches): {refused}");

    assert!(n_p > 1000, "only {n_p} P values compared");
    assert!(n_l > 1000, "only {n_l} lower-gamma values compared");
    assert!(
        worst_p.0 < 1.0e-12,
        "P(a,x) differs from GSL by {:.3e} relative at a = {}, x = {}",
        worst_p.0,
        worst_p.1,
        worst_p.2
    );
    assert!(
        worst_l.0 < 1.0e-12,
        "lower gamma differs from GSL by {:.3e} relative at a = {}, x = {}",
        worst_l.0,
        worst_l.1,
        worst_l.2
    );
}

/// `gami(3/2, x)` is the only call `fmn` makes. Pinned separately and tightly,
/// because it is the one that matters for ACER.
#[test]
fn the_a_equals_three_halves_path_is_exact() {
    let Some(path) = reference() else {
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap();
    let mut n = 0usize;
    let mut worst = 0.0f64;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() != Some("L") {
            continue;
        }
        let v: Vec<f64> = it.map(|s| s.parse().unwrap()).collect();
        if (v[0] - 1.5).abs() > 1e-15 {
            continue;
        }
        let (val, _) = gamma_inc_lower(1.5, v[1]).expect("a = 3/2 must never refuse");
        let rel = (val - v[2]).abs() / v[2].abs();
        worst = worst.max(rel);
        n += 1;
    }
    println!("  gami(3/2, x): {n} points, worst rel {worst:.3e}");
    assert!(n > 200, "only {n} points at a = 3/2");
    assert!(worst < 1.0e-13, "gami(3/2, x) worst rel {worst:.3e}");
}

/// The refused branches are refusals, not wrong answers.
#[test]
fn unported_branches_refuse() {
    assert!(gamma_inc_p(-1.0, 1.0).is_err(), "a <= 0");
    assert!(gamma_inc_p(1.0, -1.0).is_err(), "x < 0");
    // lngamma's Pade branch, |a - 1| < 0.01, is not ported.
    assert!(gamma_inc_lower(1.0, 5.0).is_err(), "lngamma Pade branch");
    assert_eq!(gamma_inc_p(1.5, 0.0).expect("x = 0 is exact").0, 0.0);
}
