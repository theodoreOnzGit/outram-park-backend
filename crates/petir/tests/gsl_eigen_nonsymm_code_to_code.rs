// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::{hessenberg, balance_matrix, eigen_nonsymm}`
//! against GSL 2.8 compiled and run** (gh:#993).
//!
//! # Methodology
//!
//! `reference-data/gsl/eigen_nonsymm_reference_driver.c` runs
//! `gsl_linalg_hessenberg_decomp`, `gsl_linalg_balance_matrix` and
//! `gsl_eigen_nonsymm` (eigenvalues only, unbalanced and balanced) on
//! thirteen matrices: 1x1, a rotation (`+-i`), a real 2x2, the 3- and 4-cycle
//! permutations (roots of unity, which stall plain Wilkinson shifts and so
//! reach the exceptional shift), a companion matrix with a complex pair, an
//! upper-triangular 3x3, the Frank matrix of order 8 (ill-conditioned small
//! eigenvalues), a badly scaled 3x3 (where balancing changes the arithmetic),
//! a block-diagonal 6x6 (deflation splits mid-matrix, exercising the
//! recursion), and random 6x6, 12x12 and 25x25. Every value is written with
//! `%.17e`; eigenvalues in GSL's (deflation) order.
//!
//! The driver is run against stock GSL and relinked with the `libm` crate's
//! `hypot` (`reference-data/gsl/libm_shim.rs`). Only
//! `householder_transform`'s `hypot` is affected: `francis.c` calls GSL's
//! own `gsl_hypot`, which PETIR ports.
//!
//! Pass criteria: every value bit-identical to the `libm`-`hypot` run,
//! eigenvalues in the same order; against stock GSL, the eigenvalues as a
//! multiset (nearest-match pairing) within `1e-8 * max|lambda|` — loose on
//! purpose, because an eigenvalue of a non-normal matrix (Frank) may move by
//! `cond * eps * ||A||` and its condition number is not bounded; the
//! bit-identity test is what carries the verification.
//!
//! # Prediction, written before the first run (2026-10-20)
//!
//! Bit-identical against the `libm`-`hypot` run, every case, both balancing
//! settings. Against stock: well-conditioned eigenvalues to ~1e-15, Frank's
//! smallest to ~1e-10 relative.
//!
//! # Results (2026-10-20)
//!
//! Against the `libm`-`hypot` run: Hessenberg 1037/1037, balancing
//! 1062/1062, eigenvalues 320/320 (both settings, same order), **all
//! bit-identical**. Against stock GSL: 104/320 eigenvalue components
//! bit-identical in order (not gated); as a multiset, worst difference
//! **4.54e-13 of `max|lambda|`** (Frank 8, balanced, `lambda = 0.1025`).
//! The prediction held. That the cyclic cases reach the exceptional shift
//! is pinned by `eigen_nonsymm::tests::cyclic_permutations_reach_the_exceptional_shift`.

mod linalg_gsl_common;

use linalg_gsl_common::{load, Case, Score};
use petir::linalg::{balance_matrix, eigen_nonsymm, hessenberg};

const FILE: &str = "eigen-nonsymm-gsl-2.8-reference.txt";
const SHIM_FILE: &str = "eigen-nonsymm-gsl-2.8-libm-hypot-reference.txt";

fn evals(case: &Case, re: &str, im: &str) -> Vec<(f64, f64)> {
    case.vector(re).into_iter().zip(case.vector(im)).collect()
}

/// Hessenberg form, balancing, and both eigenvalue runs, against GSL with
/// PETIR's `hypot`.
///
/// # Results (2026-10-20)
///
/// Hessenberg 1037/1037, balancing 1062/1062, eigenvalues 320/320: **all
/// bit-identical**.
#[test]
fn nonsymm_matches_gsl_with_the_same_hypot_bit_for_bit() {
    let Some(cases) = load(SHIM_FILE) else {
        eprintln!("skipping: {SHIM_FILE} not present");
        return;
    };
    let mut hs = Score::default();
    let mut bs = Score::default();
    let mut es = Score::default();
    for (name, case) in &cases {
        let a = case.matrix("A").unwrap();
        let (h, tau) = hessenberg(&a).unwrap();
        for (idx, v) in case.tag("HESS") {
            let (i, j) = (idx[0], idx[1]);
            hs.add(|| format!("{name} HESS[{i}][{j}]"), *v, h.get(i, j));
        }
        for (idx, v) in case.tag("HTAU") {
            hs.add(|| format!("{name} HTAU[{}]", idx[0]), *v, tau[idx[0]]);
        }
        let mut b = a.clone();
        let d = balance_matrix(&mut b);
        for (idx, v) in case.tag("BAL") {
            let (i, j) = (idx[0], idx[1]);
            bs.add(|| format!("{name} BAL[{i}][{j}]"), *v, b.get(i, j));
        }
        for (idx, v) in case.tag("BALD") {
            bs.add(|| format!("{name} D[{}]", idx[0]), *v, d[idx[0]]);
        }
        for (bal, st, re, im) in [
            (false, "STATUS0", "EV0R", "EV0I"),
            (true, "STATUS1", "EV1R", "EV1I"),
        ] {
            assert_eq!(case.scalar(st), Some(0.0), "{name} {st}");
            let ev = eigen_nonsymm(&a, bal).unwrap();
            let g = evals(case, re, im);
            assert_eq!(ev.len(), g.len(), "{name}");
            for (k, (z, (gr, gi))) in ev.iter().zip(g).enumerate() {
                es.add(|| format!("{name} {re}[{k}]"), gr, z.re);
                es.add(|| format!("{name} {im}[{k}]"), gi, z.im);
            }
        }
    }
    for (s, label) in [
        (&hs, "Hessenberg (packed, tau)"),
        (&bs, "balancing (matrix, D)"),
        (&es, "eigenvalues (re, im; unbalanced and balanced)"),
    ] {
        eprintln!("{}", s.summary(&format!("{label} vs GSL 2.8 + libm hypot")));
        assert!(s.compared > 0);
        assert_eq!(s.identical, s.compared, "{}", s.summary(label));
    }
}

/// Against stock GSL: eigenvalues as a multiset.
///
/// # Results (2026-10-20)
///
/// In order (not gated): 104/320 bit-identical; the worst per-value figure
/// is a `1e-16` eigenvalue of the 4-cycle, i.e. rounding noise around zero.
/// As a multiset: worst **4.54e-13** of `max|lambda|` (Frank 8, balanced).
#[test]
fn nonsymm_eigenvalues_agree_with_stock_gsl() {
    let Some(cases) = load(FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    let mut raw = Score::default();
    let mut worst = 0.0_f64;
    let mut worst_where = String::new();
    for (name, case) in &cases {
        let a = case.matrix("A").unwrap();
        for (bal, re, im) in [(false, "EV0R", "EV0I"), (true, "EV1R", "EV1I")] {
            let ev = eigen_nonsymm(&a, bal).unwrap();
            let g = evals(case, re, im);
            for (k, (z, (gr, gi))) in ev.iter().zip(g.iter()).enumerate() {
                raw.add(|| format!("{name} {re}[{k}]"), *gr, z.re);
                raw.add(|| format!("{name} {im}[{k}]"), *gi, z.im);
            }
            let scale = g.iter().fold(0.0_f64, |m, (r, i)| m.max(r.hypot(*i)));
            let mut used = vec![false; g.len()];
            for z in &ev {
                let (best, dist) = g
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| !used[*k])
                    .map(|(k, (r, i))| (k, (z.re - r).hypot(z.im - i)))
                    .fold(
                        (usize::MAX, f64::INFINITY),
                        |b, c| if c.1 < b.1 { c } else { b },
                    );
                assert!(best != usize::MAX, "{name}: unmatched eigenvalue");
                used[best] = true;
                let d = dist / scale;
                if d > worst {
                    worst = d;
                    worst_where = format!("{name} {re} ({}, {})", z.re, z.im);
                }
            }
        }
    }
    eprintln!(
        "{}",
        raw.summary("nonsymm eigenvalues vs stock GSL 2.8, in order (not gated)")
    );
    eprintln!("nonsymm vs stock GSL: worst matched eigenvalue difference {worst:e} of max|lambda| at {worst_where}");
    assert!(worst < 1e-8, "{worst:e} at {worst_where}");
}
