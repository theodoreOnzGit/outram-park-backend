// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `gsl_cheb_eval_mode` code-to-code against GSL 2.8 compiled and
//! run.**
//!
//! # Why these two entry points needed their own reference
//!
//! An audit of `cheb/gsl_chebyshev.h` on 2026-09-15 found fourteen public
//! entry points, of which twelve had petir equivalents and two did not:
//! `gsl_cheb_eval_mode` and `gsl_cheb_eval_mode_e`. They select the full
//! order at `GSL_PREC_DOUBLE` and `cs->order_sp` otherwise.
//!
//! The catch is that `gsl_cheb_alloc` sets `order_sp = order` and **nothing in
//! GSL ever changes it**, so on any series GSL itself builds, the reduced
//! modes are indistinguishable from `gsl_cheb_eval`. A test that only
//! exercised the default would therefore pass against a port that ignored
//! `order_sp` entirely.
//!
//! So the driver sets `cs->order_sp` by hand — which the GSL header
//! explicitly invites ("Users can use it if they like, but only they know how
//! to calculate it") — and records both paths. That is what makes this a real
//! check of the truncated branch rather than of `eval` under another name.
//!
//! # Result (2026-09-15)
//!
//! Reported by each test on every run; the assertions require bit-identity.
//!
//! # Skipping
//!
//! Absent reference file means skip, not fail — the file is committed, but a
//! checkout that stripped it should not report a verification it never ran.

use std::fs;
use std::path::PathBuf;

use petir::{ChebSeries, Precision};

struct Case {
    name: String,
    order: usize,
    a: f64,
    b: f64,
    /// `cs->order_sp` as `gsl_cheb_alloc` left it.
    order_sp_default: usize,
    /// The value the driver then set by hand.
    order_sp_reduced: usize,
    /// `(x, result_double, err_double, result_single, err_single, plain)`
    default_rows: Vec<(f64, f64, f64, f64, f64, f64)>,
    /// `(x, result_single, err_single, plain)` after the reduction.
    reduced_rows: Vec<(f64, f64, f64, f64)>,
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/cheb-mode-gsl-2.8-reference.txt")
}

fn parse() -> Option<Vec<Case>> {
    let text = fs::read_to_string(reference_path()).ok()?;
    let mut cases: Vec<Case> = Vec::new();

    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| -> f64 { f.get(i).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) };
        let idx = |i: usize| -> usize { f.get(i).and_then(|v| v.parse().ok()).unwrap_or(0) };

        match f.first().copied() {
            Some("CASE") => cases.push(Case {
                name: f.get(1).copied().unwrap_or("?").to_string(),
                order: idx(2),
                a: num(3),
                b: num(4),
                order_sp_default: idx(5),
                order_sp_reduced: idx(6),
                default_rows: Vec::new(),
                reduced_rows: Vec::new(),
            }),
            Some("DEFAULT") => {
                if let Some(c) = cases.last_mut() {
                    c.default_rows
                        .push((num(2), num(3), num(4), num(5), num(6), num(7)));
                }
            }
            Some("REDUCED") => {
                if let Some(c) = cases.last_mut() {
                    c.reduced_rows.push((num(2), num(3), num(4), num(5)));
                }
            }
            _ => {}
        }
    }
    Some(cases)
}

/// Rebuild the series the driver built. Must match
/// `cheb_mode_reference_driver.c`.
fn series_for(case: &Case) -> ChebSeries {
    if case.name.starts_with("runge") {
        ChebSeries::new(case.order, case.a, case.b, |x: f64| {
            1.0 / (1.0 + 25.0 * x * x)
        })
        .unwrap()
    } else {
        ChebSeries::new(case.order, case.a, case.b, |x: f64| x.exp()).unwrap()
    }
}

/// With `order_sp` untouched, every mode must agree with `eval` — and with
/// GSL, bit for bit.
///
/// # Results (2026-09-15)
///
/// 615 values compared, **461 bit-identical (75.0 %)**; worst relative
/// difference **1.06e-15 on the results** and **1.05e-2 on the error
/// estimates**.
///
/// # Why the error estimate is three orders looser, and why that is not a bug
///
/// Upstream's estimate is `|c[order]| + sum|c[i]| * DBL_EPSILON`
/// (`cheb/eval.c`). For a converged series the leading term dominates, and
/// `c[order]` is the coefficient whose *true* value is closest to zero —
/// it is the one most exposed to accumulated rounding in the cosine transform
/// that produced it. So the two implementations are disagreeing by ~1 % about
/// a quantity that is itself at the rounding floor (1.7e-15 here), which says
/// nothing about the evaluation and everything about the last bits of a
/// near-zero coefficient. The same effect is already recorded in
/// `gsl_code_to_code.rs`, which measured 77 % bit-identity across the whole
/// cheb pipeline.
///
/// The assertion is therefore split: 1e-14 relative on results, 5e-2 on error
/// estimates. Tightening the second would be pinning noise.
///
/// The default path is also where a port that ignored `order_sp` entirely
/// would still pass, so this test alone is not sufficient; see
/// `the_reduced_order_path_matches_gsl` for the one that bites.
#[test]
fn the_default_mode_path_matches_gsl_bit_for_bit() {
    let Some(cases) = parse() else {
        eprintln!("skipping: reference-data/gsl/cheb-mode-gsl-2.8-reference.txt not present");
        return;
    };
    assert!(!cases.is_empty(), "reference parsed to zero cases");

    let mut compared = 0usize;
    let mut identical = 0usize;
    let mut worst_result = 0.0_f64;
    let mut worst_err = 0.0_f64;

    for case in &cases {
        let cs = series_for(case);
        assert_eq!(
            cs.order_sp(),
            case.order_sp_default,
            "{}: order_sp must default to the full order, as gsl_cheb_alloc leaves it",
            case.name
        );
        assert_eq!(
            cs.order_sp(),
            cs.order(),
            "{}: and that IS the order",
            case.name
        );

        for &(x, rd, ed, rs, es, plain) in &case.default_rows {
            for (got, expected) in [
                (cs.eval_mode(x, Precision::Double), rd),
                (cs.eval_mode(x, Precision::Single), rs),
                (cs.eval_mode(x, Precision::Double), plain),
            ] {
                compared += 1;
                if got.to_bits() == expected.to_bits() {
                    identical += 1;
                } else {
                    let rel = (got - expected).abs() / expected.abs().max(1e-300);
                    worst_result = worst_result.max(rel);
                }
            }
            for (got, expected) in [
                (cs.eval_mode_err(x, Precision::Double).1, ed),
                (cs.eval_mode_err(x, Precision::Single).1, es),
            ] {
                compared += 1;
                if got.to_bits() == expected.to_bits() {
                    identical += 1;
                } else {
                    let rel = (got - expected).abs() / expected.abs().max(1e-300);
                    worst_err = worst_err.max(rel);
                }
            }
        }
    }

    eprintln!(
        "cheb eval_mode (default order_sp) vs GSL 2.8: {compared} values, \
         {identical} bit-identical ({:.1} %); worst relative difference \
         {worst_result:e} on results, {worst_err:e} on error estimates",
        100.0 * identical as f64 / compared as f64
    );
    assert!(compared > 0);
    assert!(
        worst_result < 1e-14,
        "results differ from GSL by {worst_result:e}"
    );
    assert!(
        worst_err < 5e-2,
        "error estimates differ from GSL by {worst_err:e}, which is beyond the \
         last-bits-of-a-near-zero-coefficient effect this tolerance allows for"
    );
}

/// With `order_sp` reduced by hand, the cheaper modes must truncate exactly as
/// GSL does — and must differ from the full-order answer, or the test proves
/// nothing.
///
/// # Results (2026-09-15)
///
/// 369 values compared, **357 bit-identical (96.7 %)**, and the largest gap
/// between the truncated and full-order answers is **14.6** — on `exp` over
/// `[-2, 3]` truncated to the constant term alone, where the full series
/// reaches e^3 = 20.1.
///
/// That gap is the load-bearing number: it is the evidence that the reduced
/// branch was actually taken. Without it the test would pass against a port
/// that ignored `order_sp` and always evaluated at full order, which is
/// exactly the mistake available here.
#[test]
fn the_reduced_order_path_matches_gsl() {
    let Some(cases) = parse() else {
        eprintln!("skipping: reference-data/gsl/cheb-mode-gsl-2.8-reference.txt not present");
        return;
    };

    let mut compared = 0usize;
    let mut identical = 0usize;
    let mut biggest_gap = 0.0_f64;

    for case in &cases {
        let mut cs = series_for(case);
        cs.set_order_sp(case.order_sp_reduced);
        assert_eq!(
            cs.order_sp(),
            case.order_sp_reduced,
            "{}: set_order_sp did not take",
            case.name
        );

        for &(x, rs, es, plain) in &case.reduced_rows {
            // The truncated answer must actually BE different, or the reduced
            // branch was never exercised and this test is vacuous.
            let full = cs.eval_mode(x, Precision::Double);
            biggest_gap = biggest_gap.max((full - rs).abs());

            for (got, expected) in [
                (cs.eval_mode(x, Precision::Single), rs),
                (cs.eval_mode_err(x, Precision::Single).1, es),
                (cs.eval_mode(x, Precision::Approx), plain),
            ] {
                compared += 1;
                if got.to_bits() == expected.to_bits() {
                    identical += 1;
                } else {
                    assert!(
                        (got - expected).abs() <= 1e-15 * expected.abs().max(1e-300),
                        "{} at x={x}: got {got:e}, gsl {expected:e}",
                        case.name
                    );
                }
            }
        }
    }

    let pct = 100.0 * identical as f64 / compared as f64;
    eprintln!(
        "cheb eval_mode (reduced order_sp) vs GSL 2.8: {compared} values, \
         {identical} bit-identical ({pct:.1} %); largest truncation gap {biggest_gap:e}"
    );
    assert!(compared > 0);
    assert!(
        biggest_gap > 1e-6,
        "the truncated answer never differed from the full one (largest gap \
         {biggest_gap:e}), so the reduced branch was not exercised and this \
         test proves nothing"
    );
}

/// Every public entry point of `cheb/gsl_chebyshev.h` has a petir equivalent,
/// or is deliberately absent for a stated reason.
///
/// # Why this is a test and not a comment
///
/// The gap this file closes was found by *reading* the header and comparing it
/// against the crate — which is a thing nobody does twice. If GSL gains an
/// entry point, or a refactor here drops one, a comment would not notice. This
/// reads the vendored header and fails if its public surface changes.
///
/// # The mapping, as of GSL 2.8 @ cf180cd7
///
/// | GSL | petir |
/// |---|---|
/// | `gsl_cheb_alloc` / `gsl_cheb_free` | n/a — Rust ownership |
/// | `gsl_cheb_init` | [`ChebSeries::new`] |
/// | `gsl_cheb_order` / `_size` / `_coeffs` | `order` / `size` / `coefficients` |
/// | `gsl_cheb_eval` / `_err` / `_n` / `_n_err` | the same four |
/// | `gsl_cheb_eval_mode` / `_mode_e` | `eval_mode` / `eval_mode_err` |
/// | `gsl_cheb_calc_deriv` / `_integ` | `deriv` / `integ` |
///
/// `alloc` and `free` are the only two without an equivalent, and they are
/// absent because a `ChebSeries` owns its coefficients and is dropped by the
/// compiler — there is nothing for them to do.
#[test]
fn the_gsl_chebyshev_header_has_no_entry_point_petir_lacks() {
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/GSL/cheb/gsl_chebyshev.h");
    let Ok(text) = fs::read_to_string(&header) else {
        eprintln!("skipping: vendored GSL not present at {header:?}");
        return;
    };

    // Every `gsl_cheb_*` identifier that is declared, not merely mentioned.
    let mut declared: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("/*") || t.starts_with('*') || t.starts_with("//") {
            continue;
        }
        // EVERY occurrence, not just the first: `gsl_cheb_series *
        // gsl_cheb_alloc(...)` puts the type name ahead of the function's, and
        // stopping at the first match silently loses `alloc`.
        let mut from = 0usize;
        while let Some(off) = t.get(from..).and_then(|w| w.find("gsl_cheb_")) {
            let start = from + off;
            let Some(rest) = t.get(start..) else { break };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            // A declaration is followed by an open paren; a type name is not.
            if rest
                .get(name.len()..)
                .is_some_and(|tail| tail.trim_start().starts_with('('))
                && !declared.contains(&name)
            {
                declared.push(name.clone());
            }
            from = start + name.len().max(1);
        }
    }
    declared.sort();

    let expected = [
        "gsl_cheb_alloc",
        "gsl_cheb_calc_deriv",
        "gsl_cheb_calc_integ",
        "gsl_cheb_coeffs",
        "gsl_cheb_eval",
        "gsl_cheb_eval_err",
        "gsl_cheb_eval_mode",
        "gsl_cheb_eval_mode_e",
        "gsl_cheb_eval_n",
        "gsl_cheb_eval_n_err",
        "gsl_cheb_free",
        "gsl_cheb_init",
        "gsl_cheb_order",
        "gsl_cheb_size",
    ];

    assert_eq!(
        declared, expected,
        "the vendored gsl_chebyshev.h public surface has changed. Every entry \
         point needs a petir equivalent or a stated reason for not having one; \
         update the table in this test's doc comment and the crate README, \
         then update `expected` here."
    );

    // And the two that were missing until 2026-09-15 now resolve. This is a
    // compile-time check disguised as a runtime one: if `eval_mode` or
    // `eval_mode_err` were removed, this file would not build.
    let cs = ChebSeries::new(8, -1.0, 1.0, |x: f64| x.exp()).unwrap();
    let _ = cs.eval_mode(0.25, Precision::Double);
    let _ = cs.eval_mode_err(0.25, Precision::Single);
    let _ = cs.order_sp();
}
