// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::Cholesky` against GSL 2.8 compiled and run**
//! (gh:#993).
//!
//! # Methodology
//!
//! `reference-data/gsl/cholesky_reference_driver.c` calls
//! `gsl_linalg_cholesky_decomp1`, `_solve` and `_invert` on six matrices and
//! writes the inputs and every output entry with `%.17e`. This test rebuilds
//! each input from the file, runs the port, and compares the packed factor
//! (`LLT`, both triangles), the solution and the full inverse, value by value:
//! bit-identity count and worst relative difference. The indefinite case must
//! be refused as GSL refuses it (`GSL_EDOM`).
//!
//! Cases: a 3x3 with an exact integer factor, Hilbert 6x6 (cond ~1.5e7),
//! pseudo-random Gram matrices 8x8 and 24x24 (24 is GSL's
//! `CROSSOVER_CHOLESKY`, the largest size it runs at Level 2), a 30x30 above
//! the crossover, and a 2x2 indefinite matrix.
//!
//! Pass criteria:
//!
//! - every case at or below the crossover is **bit-identical** to stock GSL
//!   (the arithmetic is the same, so anything less is a porting defect);
//! - **every** case, the 30x30 included, is bit-identical to GSL rebuilt with
//!   its Level-3 recursion switched off
//!   (`cholesky-gsl-2.8-level2-reference.txt`, made by
//!   `reference-data/gsl/level2_rebuild.sh`, which only raises `CROSSOVER` in
//!   `linalg/recurse.h`). That isolates the one documented DEVIATION
//!   (`src/linalg/cholesky.rs`: PETIR runs the Level-2 algorithm at every
//!   size) as the *whole* cause of any difference from stock GSL;
//! - against stock GSL above the crossover, the difference is reported and
//!   bounded normwise (below).
//!
//! # Prediction, written before the first run (2026-10-20)
//!
//! `n <= 24`: 100 % bit-identical — same loops, and `sqrt` is correctly
//! rounded in both glibc and `libm`. 30x30: most entries differ in the last
//! one or two ulps; worst relative difference ~1e-15 on the factor, larger on
//! the inverse (its entries are sums that cancel).
//!
//! # Results (2026-10-20)
//!
//! | comparison | values | bit-identical | worst relative difference |
//! |---|---|---|---|
//! | stock GSL, `n <= 24` (5 cases, one a refusal) | 1411 | 1411 (100 %) | 0 |
//! | GSL rebuilt at Level 2, all 6 cases | 3241 | 3241 (100 %) | 0 |
//! | stock GSL, 30x30 (Level-3 upstream) | 1830 | 741 (40.5 %) | 1.39e-12 per entry; normwise in the test doc |
//!
//! The prediction held for `n <= 24`. **It missed for the 30x30 inverse:** a
//! per-entry `1e-12` relative bound, set before the run, was breached at
//! `1.39e-12` by `INV[13][18] = -1.877e-5`, an entry four to five orders of
//! magnitude below the matrix's largest. A per-entry relative difference on
//! a near-cancelling entry measures the cancellation, not the port, so that
//! comparison is now bounded normwise (`max |diff| / max |entry|` per output)
//! and the per-entry figure is reported rather than gated. The Level-2
//! rebuild is what carries the verification: there the 30x30 is
//! bit-identical too, so nothing but the blocking separates PETIR from
//! GSL.

mod linalg_gsl_common;

use linalg_gsl_common::{load, Score};
use petir::linalg::Cholesky;
use petir::PetirError;

const FILE: &str = "cholesky-gsl-2.8-reference.txt";

/// Score one case's factor, solve and inverse.
fn score_case(name: &str, case: &linalg_gsl_common::Case, s: &mut Score) {
    let a = case.matrix("A").unwrap();
    let b = case.vector("B");
    let c = Cholesky::new(&a).unwrap_or_else(|e| panic!("{name}: {e}"));
    for (idx, v) in case.tag("LLT") {
        let (i, j) = (idx[0], idx[1]);
        s.add(|| format!("{name} LLT[{i}][{j}]"), *v, c.packed().get(i, j));
    }
    let x = c.solve(&b).unwrap();
    for (idx, v) in case.tag("X") {
        let i = idx[0];
        s.add(|| format!("{name} X[{i}]"), *v, x[i]);
    }
    let inv = c.invert().unwrap();
    for (idx, v) in case.tag("INV") {
        let (i, j) = (idx[0], idx[1]);
        s.add(|| format!("{name} INV[{i}][{j}]"), *v, inv.get(i, j));
    }
}

/// Every case at or below GSL's Level-2 crossover is bit-identical.
///
/// # Results (2026-10-20)
///
/// 1411 values over five cases (factor, solve, inverse; the indefinite case
/// refused as `PetirError::Domain`, GSL's `GSL_EDOM`), **1411 bit-identical
/// (100 %)**.
#[test]
fn cholesky_matches_gsl_bit_for_bit_at_or_below_the_crossover() {
    let Some(cases) = load(FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    let mut s = Score::default();
    let mut seen = 0;
    for (name, case) in &cases {
        let n = case.dims[0];
        if n > 24 {
            continue;
        }
        if case.scalar("STATUS") != Some(0.0) {
            assert_eq!(
                Cholesky::new(&case.matrix("A").unwrap()),
                Err(PetirError::Domain),
                "{name}: GSL refused with status {:?}",
                case.scalar("STATUS")
            );
            seen += 1;
            continue;
        }
        score_case(name, case, &mut s);
        seen += 1;
    }
    eprintln!("{}", s.summary("Cholesky n <= 24 vs GSL 2.8"));
    assert_eq!(seen, 5, "expected five cases at or below the crossover");
    assert_eq!(s.identical, s.compared, "{}", s.summary("Cholesky"));
}

/// Every case, the 30x30 included, against GSL rebuilt with its Level-3
/// recursion disabled.
///
/// # Results (2026-10-20)
///
/// 3241 values over six cases, **3241 bit-identical (100 %)**. With stock
/// GSL the 30x30 matched in 741 of 1830; with the blocking switched off it
/// matches in all of them, so the blocking is the entire difference.
#[test]
fn cholesky_matches_gsl_at_level_2_bit_for_bit_at_every_size() {
    let Some(cases) = load("cholesky-gsl-2.8-level2-reference.txt") else {
        eprintln!("skipping: Level-2 reference not present");
        return;
    };
    let mut s = Score::default();
    for (name, case) in &cases {
        if case.scalar("STATUS") == Some(0.0) {
            score_case(name, case, &mut s);
        }
    }
    eprintln!("{}", s.summary("Cholesky vs GSL 2.8 at Level 2"));
    assert!(cases.iter().any(|(_, c)| c.dims[0] > 24));
    assert_eq!(s.identical, s.compared, "{}", s.summary("Cholesky Level 2"));
}

/// The 30x30 case against stock GSL, which blocks above `n = 24`.
///
/// # Methodology
///
/// Per output (factor, solution, inverse), the normwise difference
/// `max_ij |gsl - petir| / max_ij |gsl|`; per-entry relative differences are
/// printed but not gated (see the module doc for why, and for the bound that
/// was breached before this instrument was chosen).
///
/// # Results (2026-10-20)
///
/// Per entry: 1830 values, 741 bit-identical (40.5 %), worst 1.39e-12
/// (`INV[13][18]`). Normwise (max abs diff over max abs entry): factor 1.12e-16, inverse 3.80e-16, solution 2.88e-16, gated at 1e-14.
#[test]
fn cholesky_above_the_crossover_agrees_with_stock_gsl_normwise() {
    let Some(cases) = load(FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    let mut s = Score::default();
    for (name, case) in cases.iter().filter(|(_, c)| c.dims[0] > 24) {
        score_case(name, case, &mut s);
        let a = case.matrix("A").unwrap();
        let c = Cholesky::new(&a).unwrap();
        let inv = c.invert().unwrap();
        let x = c.solve(&case.vector("B")).unwrap();
        let llt = c.packed().clone();
        for (tag, got) in [
            ("LLT", llt),
            ("INV", inv),
            (
                "X",
                petir::linalg::Matrix::from_row_major(x.len(), 1, x).unwrap(),
            ),
        ] {
            let rows = case.tag(tag);
            let big = rows.iter().fold(0.0_f64, |m, r| m.max(r.1.abs()));
            let diff = rows.iter().fold(0.0_f64, |m, r| {
                let j = r.0.get(1).copied().unwrap_or(0);
                m.max((r.1 - got.get(r.0[0], j)).abs())
            });
            let normwise = diff / big;
            eprintln!("{name} {tag}: normwise difference {normwise:e}");
            assert!(normwise < 1e-14, "{name} {tag}: normwise {normwise:e}");
        }
    }
    eprintln!(
        "{}",
        s.summary("Cholesky 30x30 vs stock GSL 2.8 (per entry, not gated)")
    );
    assert!(s.compared > 0);
}
