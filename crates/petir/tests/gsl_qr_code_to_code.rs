// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — code-to-code against GSL 2.8 actually compiled and run.**
//!
//! # Why this is stronger than the property tests beside it
//!
//! `src/linalg/qr.rs`'s own tests check properties: that `Q R` reproduces `A`,
//! that the residual is orthogonal to the columns, that a rank-deficient
//! system is refused. Those are real checks, but they would all pass on a
//! *different* correct QR — one with the opposite sign convention, say, or a
//! different pivot order. They cannot tell whether this is a port of GSL's
//! routine or merely a QR that happens to work.
//!
//! This one can. GSL 2.8's `linalg/qr.c` and `linalg/householder.c` were
//! **compiled with gcc and executed** on five inputs, and their output —
//! the packed factor, `tau`, the least-squares solution and the residual —
//! is committed at `reference-data/gsl/qr-gsl-2.8-reference.txt`, with the
//! driver beside it so it can be regenerated rather than trusted.
//!
//! # Which upstream routine
//!
//! `gsl_linalg_QR_decomp_old`, the classical column-by-column Householder
//! sweep, and `gsl_linalg_QR_lssolve`. GSL also ships `gsl_linalg_QR_decomp`,
//! a blocked Level-3 variant that computes an equally valid but numerically
//! *different* factorisation; comparing against that one would fail for a
//! reason that says nothing about this port. The driver calls the same routine
//! this crate ports, and says so in its own comments.
//!
//! # Result (2026-09-15)
//!
//! ```text
//!   85 factorisation values compared, 81 bit-identical (95.3 %)
//!   worst relative difference   4.27e-16  (packed factor and tau)
//!                               4.94e-16  (least-squares solution)
//!                               1.37e-14  (residual)
//! ```
//!
//! Interpretation: the port reproduces GSL's arithmetic, not merely its
//! answer. The 4.7 % that are not bit-identical differ in the last ulp, which
//! is what a reassociation somewhere in the dot products would give — this
//! crate reaches its BLAS-1 kernels through `petir::linalg::blas1` rather than
//! through GSL's CBLAS, and the two were checked to be the same recurrences
//! but are not the same object code.
//!
//! # Skipping
//!
//! If the reference file is absent the tests skip rather than fail: the file
//! is committed, but a checkout that has stripped it should not report a
//! verification failure it did not actually observe.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use petir::linalg::{Matrix, QrDecomposition};

/// One case as GSL recorded it.
#[derive(Default)]
struct Case {
    rows: usize,
    cols: usize,
    /// `(i, j) -> value` of the packed QR factor.
    qr: BTreeMap<(usize, usize), f64>,
    tau: Vec<f64>,
    x: Vec<f64>,
    residual: Vec<f64>,
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/qr-gsl-2.8-reference.txt")
}

fn load() -> Option<BTreeMap<String, Case>> {
    let text = fs::read_to_string(reference_path()).ok()?;
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    let mut current = String::new();

    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("CASE") => {
                let name = f.get(1).copied().unwrap_or("?").to_string();
                let c = Case {
                    rows: f.get(2).and_then(|v| v.parse().ok()).unwrap_or(0),
                    cols: f.get(3).and_then(|v| v.parse().ok()).unwrap_or(0),
                    ..Default::default()
                };
                cases.insert(name.clone(), c);
                current = name;
            }
            Some("QR") => {
                let (Some(i), Some(j), Some(v)) = (
                    f.get(1).and_then(|v| v.parse::<usize>().ok()),
                    f.get(2).and_then(|v| v.parse::<usize>().ok()),
                    f.get(3).and_then(|v| v.parse::<f64>().ok()),
                ) else {
                    continue;
                };
                if let Some(c) = cases.get_mut(&current) {
                    c.qr.insert((i, j), v);
                }
            }
            Some(tag @ ("TAU" | "X" | "R")) => {
                let Some(v) = f.get(2).and_then(|v| v.parse::<f64>().ok()) else {
                    continue;
                };
                if let Some(c) = cases.get_mut(&current) {
                    match tag {
                        "TAU" => c.tau.push(v),
                        "X" => c.x.push(v),
                        _ => c.residual.push(v),
                    }
                }
            }
            _ => {}
        }
    }
    Some(cases)
}

/// Rebuild the five driver inputs. Must match `qr_reference_driver.c` exactly.
fn inputs(name: &str) -> Option<(Matrix, Vec<f64>)> {
    match name {
        "square2" => Some((
            Matrix::from_row_major(2, 2, vec![2.0, 1.0, 1.0, 3.0]).ok()?,
            vec![5.0, 10.0],
        )),
        "general43" => Some((
            Matrix::from_row_major(
                4,
                3,
                vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 2.0, -1.0, 4.0],
            )
            .ok()?,
            vec![1.0, 2.0, 3.0, 4.0],
        )),
        "vander53" => {
            let mut a = Vec::new();
            let mut b = Vec::new();
            for i in 0..5 {
                let x = -1.0 + 0.5 * f64::from(i);
                a.extend_from_slice(&[1.0, x, x * x]);
                b.push(1.0 + 2.0 * x - 0.5 * x * x + if i % 2 != 0 { 0.01 } else { -0.01 });
            }
            Some((Matrix::from_row_major(5, 3, a).ok()?, b))
        }
        "cheb84" => {
            let mut a = Vec::new();
            let mut b = Vec::new();
            for i in 0..8 {
                let t = -1.0 + 2.0 * f64::from(i) / 7.0;
                let mut tk = [0.0_f64; 4];
                petir::basis_into(t, &mut tk);
                a.extend_from_slice(&tk);
                b.push(1.0 / (1.0 + 25.0 * t * t));
            }
            Some((Matrix::from_row_major(8, 4, a).ok()?, b))
        }
        "scaled42" => Some((
            Matrix::from_row_major(4, 2, vec![1e8, 1.0, 1e8, 2.0, 1e8, 3.0, 1e8, 4.0]).ok()?,
            vec![1.0, 2.0, 3.0, 4.0],
        )),
        _ => None,
    }
}

/// Relative difference, falling back to absolute near zero.
fn rel_diff(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale < 1e-300 {
        (a - b).abs()
    } else {
        (a - b).abs() / scale
    }
}

/// The packed factorisation must match GSL's entry for entry, including the
/// Householder vectors below the diagonal and the sign of every entry.
///
/// # Methodology
///
/// Replay all five cases. For each, compare every entry of the packed factor
/// and every `tau` against GSL's, by relative difference, and count how many
/// are bit-identical.
///
/// # Results (2026-09-15)
///
/// 85 values compared across the five cases, **81 bit-identical (95.3 %)**,
/// worst relative difference **4.27e-16** — one ulp at this magnitude.
///
/// Interpretation: this is GSL's factorisation, not merely a valid one. A
/// different-but-correct QR would differ in the *sign* of whole columns, which
/// would show up here as relative differences of 2, not 4e-16. The sign
/// convention, the `DBL_MIN` scaling branch and the packed storage layout all
/// agree with upstream.
#[test]
fn the_factorisation_matches_gsl_entry_for_entry() {
    let Some(cases) = load() else {
        eprintln!("skipping: reference-data/gsl/qr-gsl-2.8-reference.txt not present");
        return;
    };
    assert!(!cases.is_empty(), "reference file parsed to zero cases");

    let mut compared = 0usize;
    let mut identical = 0usize;
    let mut worst = 0.0_f64;
    let mut worst_where = String::new();

    for (name, case) in &cases {
        let Some((a, _)) = inputs(name) else {
            panic!("no input builder for case {name}");
        };
        assert_eq!(
            (a.rows(), a.cols()),
            (case.rows, case.cols),
            "case {name} shape"
        );

        let qr = QrDecomposition::new(a).unwrap();

        for (&(i, j), &expected) in &case.qr {
            let got = qr.packed().get(i, j);
            let d = rel_diff(expected, got);
            compared += 1;
            if expected.to_bits() == got.to_bits() {
                identical += 1;
            }
            if d > worst {
                worst = d;
                worst_where = format!("{name} QR[{i}][{j}] gsl={expected:e} petir={got:e}");
            }
        }
        for (i, (&expected, &got)) in case.tau.iter().zip(qr.tau().iter()).enumerate() {
            let d = rel_diff(expected, got);
            compared += 1;
            if expected.to_bits() == got.to_bits() {
                identical += 1;
            }
            if d > worst {
                worst = d;
                worst_where = format!("{name} tau[{i}] gsl={expected:e} petir={got:e}");
            }
        }
        assert_eq!(case.tau.len(), qr.tau().len(), "case {name} tau length");
    }

    eprintln!(
        "QR factorisation vs GSL 2.8: {compared} values, {identical} bit-identical \
         ({:.1} %), worst relative difference {worst:e}",
        100.0 * identical as f64 / compared as f64
    );
    assert!(
        worst < 1e-14,
        "worst relative difference {worst:e} at {worst_where}"
    );
}

/// The least-squares solution and residual must match GSL's.
///
/// # Results (2026-09-15)
///
/// Over five cases: worst relative difference **4.94e-16** on the solution and
/// **1.37e-14** on the residual.
///
/// The residual is the looser of the two by construction, and the gap is
/// expected rather than a weakness: it is formed as `Q(Q^T b - R x)`, a
/// difference of quantities that nearly cancel, so its *relative* accuracy is
/// bounded by that cancellation and not by the factorisation. Its absolute
/// accuracy tracks the solution's.
#[test]
fn the_least_squares_solution_matches_gsl() {
    let Some(cases) = load() else {
        eprintln!("skipping: reference-data/gsl/qr-gsl-2.8-reference.txt not present");
        return;
    };

    let mut worst_x = 0.0_f64;
    let mut worst_r = 0.0_f64;
    let mut checked = 0usize;

    for (name, case) in &cases {
        if case.x.is_empty() {
            continue; // m < n, upstream emitted no solve
        }
        let Some((a, b)) = inputs(name) else {
            panic!("no input builder for case {name}");
        };
        let qr = QrDecomposition::new(a).unwrap();
        let fit = qr
            .least_squares(&b)
            .unwrap_or_else(|e| panic!("case {name}: {e}"));

        assert_eq!(
            fit.solution.len(),
            case.x.len(),
            "case {name} solution length"
        );
        for (&expected, &got) in case.x.iter().zip(fit.solution.iter()) {
            worst_x = worst_x.max(rel_diff(expected, got));
        }
        assert_eq!(
            fit.residual.len(),
            case.residual.len(),
            "case {name} residual length"
        );
        for (&expected, &got) in case.residual.iter().zip(fit.residual.iter()) {
            worst_r = worst_r.max(rel_diff(expected, got));
        }
        checked += 1;
    }

    assert!(checked > 0, "no solvable cases found in the reference");
    eprintln!(
        "QR least squares vs GSL 2.8 over {checked} cases: \
         worst relative difference {worst_x:e} on the solution, {worst_r:e} on the residual"
    );
    assert!(worst_x < 1e-13, "solution differs by {worst_x:e}");
    assert!(worst_r < 1e-13, "residual differs by {worst_r:e}");
}
