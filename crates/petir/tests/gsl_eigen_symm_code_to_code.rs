// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::{SymmTd, eigen_symm, eigen_symmv,
//! eigen_symmv_sort}` against GSL 2.8 compiled and run** (gh:#993).
//!
//! # Methodology
//!
//! `reference-data/gsl/eigen_symm_reference_driver.c` runs
//! `gsl_linalg_symmtd_decomp` / `_unpack`, `gsl_eigen_symm`,
//! `gsl_eigen_symmv` and `gsl_eigen_symmv_sort(VAL_ASC)` on nine symmetric
//! matrices and writes the inputs and every output with `%.17e`. The test
//! rebuilds each input from the file, runs the port, and scores every value:
//! bit-identity count and worst relative difference. Eigenvalues are
//! compared **unordered**, in the order GSL returned them, because that order
//! is the trace of the QR iteration's deflation sequence and so identifies the
//! algorithm; the sorted output is compared too.
//!
//! Cases: 1x1, 2x2, a 4x4, Hilbert 8x8, Wilkinson W21+ (eigenvalues in pairs
//! agreeing to 1e-14 relative and worse), a block-diagonal 6x6 (deflation
//! splits it), `5 I + 1 1^T` (a four-fold repeated eigenvalue), a random
//! symmetric 10x10 and a 30x30 Gram matrix.
//!
//! The driver is run twice: against stock GSL
//! (`eigen-symm-gsl-2.8-reference.txt`), and linked with
//! `reference-data/gsl/libm_shim.rs`, which replaces glibc's `hypot` with the
//! `libm` crate's — the one PETIR calls
//! (`eigen-symm-gsl-2.8-libm-hypot-reference.txt`).
//!
//! # Prediction, written before the first run (2026-10-20)
//!
//! Mostly bit-identical, last-ulp differences from `hypot` (libm vs glibc) and
//! from `nrm2`. Eigenvalues ~1e-15; vectors ~1e-14 except the W21+ pairs.
//!
//! # Results (2026-10-20) — the prediction MISSED, and why
//!
//! Against stock GSL, as first gated (per value, unordered, `1e-13`): **failed**.
//! Decomposition 1652/2656 bit-identical (62.2 %), eigenvalues 112/261
//! (42.9 %), vectors 992/3174 (31.3 %), and on `rankone5` a structural
//! difference: GSL's tridiagonal form has exact zeros below the first
//! subdiagonal entry (`TD[1][0] = -2`, `tau[1] = tau[2] = 0`) where PETIR had
//! `-1.99999999999999978` and `O(1e-15)` entries with non-zero `tau`.
//!
//! Cause, isolated rather than guessed: `householder::transform` computes
//! `beta = -hypot(1, sqrt 3)`. The exact value rounds to `2.0`; glibc returns
//! `2.0` and the `libm` crate returns `2 - 2^-52`. Relinking the **same GSL
//! driver** with the `libm` crate's `hypot` (the shim) reproduces PETIR **bit
//! for bit on every value of every case** — see the first test. So the port
//! is exact, and the whole stock difference is the `hypot` PETIR takes from
//! `libm`, the same deliberate cross-platform trade
//! `reference-data/gsl/README.md` records for `cos` in `cheb`.
//! `nrm2`, the other suspect, contributes nothing here.
//!
//! The stock comparison is now gated as an eigen-decomposition (sorted, up to
//! sign, span inside a repeated eigenvalue — second test), because a one-ulp
//! `hypot` legitimately changes the deflation order and the basis chosen
//! inside a degenerate eigenspace. That instrument was chosen **after** the
//! miss, and is labelled so; the bit-identity test is what carries the
//! verification.

mod linalg_gsl_common;

use linalg_gsl_common::{load, Case, Score};
use petir::linalg::{eigen_symm, eigen_symmv, eigen_symmv_sort, EigenSort, Matrix, SymmTd};

const FILE: &str = "eigen-symm-gsl-2.8-reference.txt";
const SHIM_FILE: &str = "eigen-symm-gsl-2.8-libm-hypot-reference.txt";

fn input(case: &Case) -> Matrix {
    case.matrix("A").unwrap()
}

/// Decomposition, `tau`, `Q`, `T`, unordered and sorted eigenvalues and
/// eigenvectors, against GSL linked with PETIR's `hypot`: every value
/// bit-identical.
///
/// # Results (2026-10-20)
///
/// 2656 decomposition values, 261 eigenvalues and 3174 eigenvector entries,
/// **all bit-identical (100 %)**, every case including the degenerate ones.
#[test]
fn everything_matches_gsl_with_the_same_hypot_bit_for_bit() {
    let Some(cases) = load(SHIM_FILE) else {
        eprintln!("skipping: {SHIM_FILE} not present");
        return;
    };
    let mut td_s = Score::default();
    let mut ev_s = Score::default();
    let mut vec_s = Score::default();
    for (name, case) in &cases {
        let a = input(case);
        if a.rows() >= 2 {
            let td = SymmTd::new(&a).unwrap();
            for (idx, v) in case.tag("TD") {
                let (i, j) = (idx[0], idx[1]);
                td_s.add(|| format!("{name} TD[{i}][{j}]"), *v, td.packed().get(i, j));
            }
            for (idx, v) in case.tag("TAU") {
                let i = idx[0];
                td_s.add(|| format!("{name} tau[{i}]"), *v, td.tau()[i]);
            }
            let (q, d, sd) = td.unpack().unwrap();
            for (idx, v) in case.tag("Q") {
                let (i, j) = (idx[0], idx[1]);
                td_s.add(|| format!("{name} Q[{i}][{j}]"), *v, q.get(i, j));
            }
            for (idx, v) in case.tag("D") {
                td_s.add(|| format!("{name} D[{}]", idx[0]), *v, d[idx[0]]);
            }
            for (idx, v) in case.tag("SD") {
                td_s.add(|| format!("{name} SD[{}]", idx[0]), *v, sd[idx[0]]);
            }
        }
        let ev = eigen_symm(&a).unwrap();
        for (idx, v) in case.tag("EVAL") {
            ev_s.add(|| format!("{name} EVAL[{}]", idx[0]), *v, ev[idx[0]]);
        }
        let (vv, vecs) = eigen_symmv(&a).unwrap();
        for (idx, v) in case.tag("VVAL") {
            ev_s.add(|| format!("{name} VVAL[{}]", idx[0]), *v, vv[idx[0]]);
        }
        for (idx, v) in case.tag("VVEC") {
            let (i, j) = (idx[0], idx[1]);
            vec_s.add(|| format!("{name} VVEC[{i}][{j}]"), *v, vecs.get(i, j));
        }
        let (mut sv, mut svec) = (vv.clone(), vecs.clone());
        eigen_symmv_sort(&mut sv, &mut svec, EigenSort::ValAsc).unwrap();
        for (idx, v) in case.tag("SVAL") {
            ev_s.add(|| format!("{name} SVAL[{}]", idx[0]), *v, sv[idx[0]]);
        }
        for (idx, v) in case.tag("SVEC") {
            let (i, j) = (idx[0], idx[1]);
            vec_s.add(|| format!("{name} SVEC[{i}][{j}]"), *v, svec.get(i, j));
        }
    }
    for (s, label) in [
        (&td_s, "symmtd (packed, tau, Q, T)"),
        (&ev_s, "eigenvalues (symm, symmv, sorted)"),
        (&vec_s, "eigenvectors (unordered, sorted)"),
    ] {
        eprintln!("{}", s.summary(&format!("{label} vs GSL 2.8 + libm hypot")));
        assert!(s.compared > 0);
        assert_eq!(s.identical, s.compared, "{}", s.summary(label));
    }
}

/// Against stock GSL (glibc `hypot`): sorted eigenvalues, and sorted
/// eigenvectors compared as eigenvectors.
///
/// # Methodology
///
/// The unordered per-value score is printed but not gated (see the module
/// doc). Gated: sorted eigenvalues normwise (`|diff| / max|lambda|`) below
/// `1e-13`; each sorted eigenvector column, **up to sign**, normwise within
/// the first-order perturbation bound for an eigenvector,
/// `n * DBL_EPSILON * max|lambda| / gap_j` (`gap_j` the distance from
/// `lambda_j` to the nearest other eigenvalue; Golub & Van Loan, 3rd ed.,
/// section 8.1), unless its eigenvalue lies within `1e-8 * max|lambda|` of
/// another, where it must instead lie in the span of GSL's columns for that
/// cluster (projection residual below `1e-10`) — inside a repeated eigenvalue
/// no algorithm determines the basis.
///
/// # Results (2026-10-20)
///
/// Unordered eigenvalues (printed, not gated): 37/87 bit-identical, the order
/// differing on `rankone5`. Sorted eigenvalues: worst normwise **1.73e-15**.
/// Eigenvectors: worst column difference **4.69e-11** (`hilbert8`, column 1,
/// `lambda ~ 1e-8`), and worst fraction of its perturbation bound **0.55**
/// (`rankone5`, column 4); 10 columns in repeated eigenvalues checked by span.
///
/// A fixed `1e-12` column bound was tried first and **breached** by that
/// Hilbert column. Its eigenvalue is ~`1e-8` from its neighbour, so a
/// one-ulp change in `A` may move the vector by ~`3e-8`; `4.7e-11` is well
/// inside that. The bound now scales with the gap, which is what the
/// perturbation theory says the difference should scale with.
#[test]
fn sorted_eigenpairs_agree_with_stock_gsl() {
    let Some(cases) = load(FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    let mut raw = Score::default();
    let mut worst_val = 0.0_f64;
    let mut worst_col = 0.0_f64;
    let mut worst_col_where = String::new();
    let mut clustered = 0usize;
    let mut worst_ratio = 0.0_f64;
    let mut worst_ratio_where = String::new();
    for (name, case) in &cases {
        let a = input(case);
        let n = a.rows();
        let ev = eigen_symm(&a).unwrap();
        for (idx, v) in case.tag("EVAL") {
            raw.add(|| format!("{name} EVAL[{}]", idx[0]), *v, ev[idx[0]]);
        }
        let (mut vals, mut vecs) = eigen_symmv(&a).unwrap();
        eigen_symmv_sort(&mut vals, &mut vecs, EigenSort::ValAsc).unwrap();
        let g = case.matrix("SVEC").unwrap();
        let gv = case.vector("SVAL");
        let scale = gv.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
        for j in 0..n {
            worst_val = worst_val.max((gv[j] - vals[j]).abs() / scale);
            let cluster: Vec<usize> = (0..n)
                .filter(|&k| (gv[k] - vals[j]).abs() <= 1e-8 * scale)
                .collect();
            let gap = (0..n)
                .filter(|&k| k != j)
                .fold(f64::INFINITY, |m, k| m.min((gv[k] - gv[j]).abs()));
            let bound = n as f64 * f64::EPSILON * scale / gap;
            let d = if cluster.len() > 1 {
                clustered += 1;
                let mut r: Vec<f64> = (0..n).map(|i| vecs.get(i, j)).collect();
                for &k in &cluster {
                    let dot: f64 = (0..n).map(|i| g.get(i, k) * vecs.get(i, j)).sum();
                    for (i, ri) in r.iter_mut().enumerate() {
                        *ri -= dot * g.get(i, k);
                    }
                }
                let res = r.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
                assert!(res < 1e-10, "{name} col {j}: off its eigenspace by {res:e}");
                0.0
            } else {
                let plus = (0..n).fold(0.0_f64, |m, i| m.max((g.get(i, j) - vecs.get(i, j)).abs()));
                let minus =
                    (0..n).fold(0.0_f64, |m, i| m.max((g.get(i, j) + vecs.get(i, j)).abs()));
                plus.min(minus)
            };
            if d > worst_col {
                worst_col = d;
                worst_col_where = format!("{name} column {j}");
            }
            if d / bound > worst_ratio {
                worst_ratio = d / bound;
                worst_ratio_where = format!("{name} column {j} (diff {d:e}, bound {bound:e})");
            }
        }
    }
    eprintln!(
        "{}",
        raw.summary("unordered eigenvalues vs stock GSL 2.8 (not gated)")
    );
    eprintln!(
        "sorted eigenvalues vs stock GSL: worst normwise {worst_val:e}; eigenvectors: worst \
         column {worst_col:e} at {worst_col_where}; worst fraction of the perturbation bound \
         {worst_ratio:e} at {worst_ratio_where}; {clustered} clustered columns checked by span"
    );
    assert!(worst_val < 1e-13, "sorted eigenvalues {worst_val:e}");
    assert!(
        worst_ratio < 1.0,
        "eigenvector outside its bound: {worst_ratio_where}"
    );
}
