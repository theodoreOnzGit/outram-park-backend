// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::Svd` against GSL 2.8 compiled and run**
//! (gh:#993).
//!
//! # Methodology
//!
//! `reference-data/gsl/svd_reference_driver.c` runs `gsl_linalg_SV_decomp`
//! and `gsl_linalg_SV_solve` on twelve matrices chosen to reach every branch
//! of `linalg/svdstep.c`: the `N == 1` column, a 2x2 (`svd2`), a rank-one
//! 3x3 and a zero middle column (`chase_out_intermediate_zero`), a zero last
//! column (`chase_out_trailing_zero`), the identity (no sweep at all), entries
//! of `1e-160` and `1e160` (the block rescaling, both directions), Hilbert
//! 6x6, and random 4x3, 8x5 and 30x20. The inputs and every entry of `U`,
//! `S`, `V` and the solve are written with `%.17e`.
//!
//! As for the symmetric eigensolver, the driver is run twice: against stock
//! GSL (`svd-gsl-2.8-reference.txt`) and relinked with the `libm` crate's
//! `hypot`, the one PETIR calls (`svd-gsl-2.8-libm-hypot-reference.txt`,
//! `reference-data/gsl/libm_shim.rs`). The two GSL runs differ in 1179 of
//! 2312 lines, so `hypot` matters here as it did there.
//!
//! Pass criteria: every value bit-identical to the `libm`-`hypot` run; against
//! stock GSL, singular values within `1e-13` normwise (relative to `s_max`)
//! and each singular-vector pair within its gap-scaled perturbation bound,
//! as in `gsl_eigen_symm_code_to_code.rs`.
//!
//! # Prediction, written before the first run (2026-10-20)
//!
//! Bit-identical against the `libm`-`hypot` run, every branch. Against stock:
//! singular values ~1e-15 normwise; vectors inside their bounds; the
//! rank-deficient cases' null-space vectors free to differ (zero singular
//! values form a cluster and are compared by span).
//!
//! # Results (2026-10-20)
//!
//! 1428/1428 bit-identical against the `libm`-`hypot` run; against stock GSL,
//! singular values 1.12e-15 normwise and vectors within 0.22 of their bound.
//! Details in the two tests.

mod linalg_gsl_common;

use linalg_gsl_common::{load, Score};
use petir::linalg::Svd;

const FILE: &str = "svd-gsl-2.8-reference.txt";
const SHIM_FILE: &str = "svd-gsl-2.8-libm-hypot-reference.txt";

/// Every `U`, `S`, `V` and solve entry against GSL with PETIR's `hypot`.
///
/// # Results (2026-10-20)
///
/// 1428 values over the twelve cases (`U`, `S`, `V`, solve), **1428
/// bit-identical (100 %)** — every branch of `svdstep.c` included. The
/// prediction held.
#[test]
fn svd_matches_gsl_with_the_same_hypot_bit_for_bit() {
    let Some(cases) = load(SHIM_FILE) else {
        eprintln!("skipping: {SHIM_FILE} not present");
        return;
    };
    let mut s = Score::default();
    for (name, case) in &cases {
        assert_eq!(case.scalar("STATUS"), Some(0.0), "{name}");
        let a = case.matrix("A").unwrap();
        let svd = Svd::new(&a).unwrap();
        for (idx, v) in case.tag("U") {
            let (i, j) = (idx[0], idx[1]);
            s.add(|| format!("{name} U[{i}][{j}]"), *v, svd.u().get(i, j));
        }
        for (idx, v) in case.tag("S") {
            s.add(|| format!("{name} S[{}]", idx[0]), *v, svd.s()[idx[0]]);
        }
        for (idx, v) in case.tag("V") {
            let (i, j) = (idx[0], idx[1]);
            s.add(|| format!("{name} V[{i}][{j}]"), *v, svd.v().get(i, j));
        }
        let x = svd.solve(&case.vector("B")).unwrap();
        for (idx, v) in case.tag("X") {
            s.add(|| format!("{name} X[{}]", idx[0]), *v, x[idx[0]]);
        }
    }
    eprintln!("{}", s.summary("SVD vs GSL 2.8 + libm hypot"));
    assert_eq!(cases.len(), 12);
    assert_eq!(s.identical, s.compared, "{}", s.summary("SVD"));
}

/// Against stock GSL (glibc `hypot`).
///
/// # Methodology
///
/// Per-value score printed, not gated. Gated: singular values normwise
/// (`|diff| / s_max`) below `1e-13`; for each `j`, the pair `(u_j, v_j)` up
/// to a common sign within `n * DBL_EPSILON * s_max / gap_j` normwise, unless
/// `s_j` lies within `1e-8 * s_max` of another singular value, where `v_j`
/// must lie in the span of GSL's `V` columns for that cluster.
///
/// # Results (2026-10-20)
///
/// Singular values per value (not gated): 27/57 bit-identical, worst
/// relative 4.5e-12 on Hilbert's smallest (`1.08e-7`, eight orders below
/// `s_max`). Gated: singular values worst normwise **1.12e-15**; singular
/// vectors worst **0.22** of their bound (`rand85`, pair 0); 5 columns in
/// clustered (zero) singular values checked by span. The prediction held.
#[test]
fn svd_agrees_with_stock_gsl_as_a_decomposition() {
    let Some(cases) = load(FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    let mut raw = Score::default();
    let mut worst_s = 0.0_f64;
    let mut worst_ratio = 0.0_f64;
    let mut worst_where = String::new();
    let mut clustered = 0usize;
    for (name, case) in &cases {
        let a = case.matrix("A").unwrap();
        let (m, n) = (a.rows(), a.cols());
        let svd = Svd::new(&a).unwrap();
        let gu = case.matrix("U").unwrap();
        let gv = case.matrix("V").unwrap();
        let gs = case.vector("S");
        for (idx, v) in case.tag("S") {
            raw.add(|| format!("{name} S[{}]", idx[0]), *v, svd.s()[idx[0]]);
        }
        let smax = gs.iter().fold(0.0_f64, |x, y| x.max(y.abs()));
        for j in 0..n {
            worst_s = worst_s.max((gs[j] - svd.s()[j]).abs() / smax);
            let cluster: Vec<usize> = (0..n)
                .filter(|&k| (gs[k] - gs[j]).abs() <= 1e-8 * smax)
                .collect();
            if cluster.len() > 1 {
                clustered += 1;
                let mut r: Vec<f64> = (0..n).map(|i| svd.v().get(i, j)).collect();
                for &k in &cluster {
                    let dot: f64 = (0..n).map(|i| gv.get(i, k) * svd.v().get(i, j)).sum();
                    for (i, ri) in r.iter_mut().enumerate() {
                        *ri -= dot * gv.get(i, k);
                    }
                }
                let res = r.iter().fold(0.0_f64, |x, y| x.max(y.abs()));
                assert!(
                    res < 1e-10,
                    "{name} v_{j} off its singular subspace by {res:e}"
                );
                continue;
            }
            let gap = (0..n)
                .filter(|&k| k != j)
                .fold(f64::INFINITY, |x, k| x.min((gs[k] - gs[j]).abs()));
            let bound = n as f64 * f64::EPSILON * smax / gap;
            let col =
                |g: &petir::linalg::Matrix, p: &petir::linalg::Matrix, rows: usize, sign: f64| {
                    (0..rows).fold(0.0_f64, |x, i| {
                        x.max((g.get(i, j) - sign * p.get(i, j)).abs())
                    })
                };
            let d = [1.0, -1.0]
                .iter()
                .map(|&sg| col(&gu, svd.u(), m, sg).max(col(&gv, svd.v(), n, sg)))
                .fold(f64::INFINITY, f64::min);
            if d / bound > worst_ratio {
                worst_ratio = d / bound;
                worst_where = format!("{name} pair {j} (diff {d:e}, bound {bound:e})");
            }
        }
    }
    eprintln!(
        "{}",
        raw.summary("singular values vs stock GSL 2.8 (not gated)")
    );
    eprintln!(
        "SVD vs stock GSL: singular values worst normwise {worst_s:e}; singular vectors worst \
         fraction of bound {worst_ratio:e} at {worst_where}; {clustered} clustered columns by span"
    );
    assert!(worst_s < 1e-13, "singular values {worst_s:e}");
    assert!(
        worst_ratio < 1.0,
        "singular vectors outside bound: {worst_where}"
    );
}
