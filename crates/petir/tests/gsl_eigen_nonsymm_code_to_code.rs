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
//!
//! # Re-measured 2026-10-20 after the CORE-MATH `hypot` (gh:#997)
//!
//! **Prediction** (gh:#993/#997, before the run): with a correctly rounded
//! `hypot`, bit-identical to stock GSL. Refined before the GSL rerun, once
//! the MPFR sweep (`tests/hypot_correct_rounding.rs`) showed glibc 2.44's
//! `hypot` is itself not correctly rounded: bit-identical wherever glibc
//! rounds every `hypot` it is given correctly, otherwise not.
//!
//! **Result.** GSL relinked with PETIR's `hypot`
//! (`eigen-nonsymm-gsl-2.8-petir-hypot-reference.txt`): Hessenberg
//! 1037/1037, balancing 1062/1062, eigenvalues 320/320, all bit-identical.
//! Against STOCK GSL: eigenvalue components in order ~~104/320~~ **236/320**
//! bit-identical; multiset worst ~~4.54e-13~~ **3.73e-15** of `max|lambda|`.
//! Every remaining difference is in `rand25`, and it is glibc's: logging
//! each `hypot` call GSL makes (glibc vs PETIR vs MPFR) found five distinct
//! inputs, each called twice, where glibc 2.44 returns one ulp off the
//! correctly rounded value and PETIR returns MPFR's, e.g.
//! `hypot(-0x1.569095968f0f5p-2, 0x1.e62aaa381022ep-2)`: glibc
//! `0x1.295df708e098cp-1`, PETIR and MPFR `0x1.295df708e098bp-1`. The
//! other twelve cases are bit-identical to stock. Prediction held in its
//! refined form; the original form ("bit-identical to stock") missed on
//! `rand25`.

mod linalg_gsl_common;

use linalg_gsl_common::{load, Case, Score};
use petir::linalg::{balance_matrix, eigen_nonsymm, hessenberg};

const FILE: &str = "eigen-nonsymm-gsl-2.8-reference.txt";
/// ~~`eigen-nonsymm-gsl-2.8-libm-hypot-reference.txt`~~ **CHANGED 2026-10-20
/// (gh:#997):** GSL relinked with PETIR's CORE-MATH `hypot`
/// (`reference-data/gsl/petir_hypot_shim.rs`). Unlike QR, SVD and the
/// symmetric solver, this one does NOT equal stock GSL: glibc 2.44 misrounds
/// five distinct `hypot` calls on `rand25` (see the module doc).
const SHIM_FILE: &str = "eigen-nonsymm-gsl-2.8-petir-hypot-reference.txt";

fn evals(case: &Case, re: &str, im: &str) -> Vec<(f64, f64)> {
    case.vector(re).into_iter().zip(case.vector(im)).collect()
}

/// Hessenberg form, balancing, and both eigenvalue runs, against GSL with
/// PETIR's `hypot`.
///
/// # Results (2026-10-20)
///
/// Hessenberg 1037/1037, balancing 1062/1062, eigenvalues 320/320: **all
/// bit-identical** (against the `libm`-`hypot` run, 2026-10-20 gh:#993; and
/// again against the PETIR-`hypot` run after gh:#997).
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
        eprintln!(
            "{}",
            s.summary(&format!("{label} vs GSL 2.8 + PETIR hypot"))
        );
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
///
/// **Re-measured 2026-10-20 (gh:#997, CORE-MATH `hypot`):** in order
/// 236/320 bit-identical, worst per-value 5.18e-15 (`rand25`); as a
/// multiset worst **3.73e-15** of `max|lambda|` (`rand25`). All of it is
/// glibc misrounding `hypot` (module doc).
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
