// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::expm` against GNU Octave 11.3.0's own `expm`,
//! run** (gh:#993).
//!
//! # Methodology
//!
//! `reference-data/octave/expm_reference_driver.m`, run by Octave 11.3.0
//! (LAPACK 3.12.0, reference BLAS), writes for fourteen matrices the input,
//! stock `expm(A)` (`R`), and `expm_nobal(A)` (`RNB`): `expm.m` copied
//! verbatim except that its balancing step and the inverse are removed —
//! the one deviation PETIR's port makes (`src/linalg/expm.rs`).
//!
//! - **PETIR vs `RNB` measures the port.** The algorithm is the same; what
//!   differs is the arithmetic underneath: PETIR's Crout LU with scaled
//!   partial pivoting against LAPACK `dgesv` (and Cholesky for a symmetric
//!   `x - y`), the `cblas_dgemm` reference loop against reference BLAS
//!   `dgemm` (the same summation order), and the `libm` crate's `exp`.
//! - **`RNB` vs `R` measures balancing**, i.e. what the deviation costs.
//! - **PETIR vs `R`** is what a user comparing against Octave would see.
//!
//! Every comparison is normwise per case, `max_ij |diff| / max_ij |R_ij|`,
//! because an entry of `exp(A)` can be a near-cancelling small number whose
//! relative error says nothing about the method. Bit-identity counts are
//! reported alongside.
//!
//! Cases: `expm.m`'s two test matrices, a scalar, a diagonal, a rotation
//! generator and a large one (`s = 6` squarings), a positive trace (the
//! shift branch), a small norm (`s = 0`), a strongly non-normal 2x2, a
//! 4-nuclide decay chain (depletion's use), a badly scaled 3x3 (where
//! balancing matters), a symmetric 4x4, and dense 6x6 and 10x10.
//!
//! Pass criterion: ~~PETIR vs `RNB`~~ **PETIR vs stock `R`** (since
//! gh:#998, when `expm` stopped deviating from `expm.m`) within `1e-13`
//! normwise on every case. The tolerance is unchanged.
//! The stock comparison is reported, not gated, because its difference is
//! the balancing deviation, which is a measured property of the port rather
//! than a defect.
//!
//! # Prediction, written before the first run (2026-10-20)
//!
//! PETIR vs `RNB`: bit-identical on the scalar and diagonal cases, ~1e-16 to
//! 1e-15 normwise elsewhere, the LU difference dominating; perhaps 1e-13 on
//! the non-normal case. `RNB` vs `R`: identical where balancing is a no-op
//! (it is on the symmetric and most small cases), and visibly different on
//! `scaled3`, perhaps 1e-10.
//!
//! # Results (2026-10-20)
//!
//! | case | PETIR vs `RNB` | PETIR vs `R` | `RNB` vs `R` (balancing) |
//! |---|---|---|---|
//! | `ward2`, `ward3` | 1.63e-16 | 1.63e-16 | 0 |
//! | `scalar`, `rot2`, `bigrot2`, `postrace2`, `tiny2`, `nonnormal2` | 0 | 0 | 0 |
//! | `diag3` | 2.21e-17 | 2.21e-17 | 0 |
//! | `chain4` | 8.7e-19 | 1.7e-18 | 8.7e-19 |
//! | `scaled3` | **0** | **1.11e-11** | **1.11e-11** |
//! | `symm4` | 6.8e-18 | 6.8e-18 | 0 |
//! | `dense6`, `dense10` | 3.2e-16, 1.6e-16 | same | 0 |
//!
//! Per entry: 127/220 bit-identical to `RNB`, 117/220 to `R`.
//!
//! The prediction held for the port (worst 3.2e-16 normwise against `RNB`,
//! gate `1e-13`) and for balancing (a no-op on thirteen cases, 1.11e-11 on
//! `scaled3`). It was wrong in one detail: `diag3` is not bit-identical,
//! because its entries are `exp` values and the `libm` crate's `exp` differs
//! from glibc's in the last bit (2.2e-17 normwise).
//!
//! **What the deviation costs:** on a badly scaled matrix, PETIR differs from
//! stock Octave by 1.1e-11 normwise (4.8e-11 on one entry), and that
//! difference is entirely the missing balancing — PETIR agrees with
//! no-balance Octave there **bit for bit**. Which of the two is closer to the
//! exact exponential was *not* determined (no multiprecision reference was
//! available in this environment).
//!
//! # gh:#998: balancing ported (LAPACK `DGEBAL`), re-measured 2026-10-20
//!
//! **Prediction, written before the run:** with `[d, p, aa] = balance (A)`
//! carried out by the `DGEBAL` port (bit-identical to Reference-LAPACK
//! 3.12.1, `tests/lapack_gebal_code_to_code.rs`), PETIR vs stock `R` on
//! `scaled3` drops from 1.11e-11 to the ~1e-16 level of the LU / `dgemm`
//! differences, and `chain4` (lower bidiagonal, so `DGEBAL` permutes it)
//! from 1.7e-18 to <= 8.7e-19; every other case unchanged, since balancing
//! is a no-op on them. The gate moves to PETIR vs stock `R`, because
//! `expm` no longer deviates from `expm.m`.
//!
//! **Result:**
//!
//! | case | PETIR vs `R`, before -> after | PETIR vs `RNB` after |
//! |---|---|---|
//! | `scaled3` | **1.11e-11 -> 0** | 1.11e-11 (the balancing, now on PETIR's side) |
//! | `chain4` | 1.7e-18 -> **0** | 8.67e-19 |
//! | all other twelve | unchanged (0 to 3.2e-16) | unchanged |
//!
//! Per entry vs stock `R`: ~~117/220~~ **128/220** bit-identical; worst
//! normwise PETIR vs `R` 3.20e-16 (`dense6`, the LU / `dgemm` difference).
//! The prediction held, and on `scaled3` and `chain4` did better than
//! predicted: both are now bit-identical to stock Octave normwise (every
//! entry of `scaled3` agrees exactly).

mod linalg_gsl_common;

use linalg_gsl_common::{load_in, Case, Score};
use petir::linalg::{expm, Matrix};

const FILE: &str = "expm-octave-11.3.0-reference.txt";

fn normwise(case: &Case, tag: &str, got: &Matrix) -> f64 {
    let rows = case.tag(tag);
    let big = rows.iter().fold(0.0_f64, |m, r| m.max(r.1.abs()));
    let diff = rows
        .iter()
        .fold(0.0_f64, |m, r| m.max((r.1 - got.get(r.0[0], r.0[1])).abs()));
    diff / big
}

/// PETIR against stock `expm` (gated since gh:#998; before it, against
/// `expm` with balancing removed), and against `RNB` (reported); `RNB`
/// against `R` (reported).
///
/// # Results (2026-10-20)
///
/// See the module table. Worst PETIR vs `RNB`: 3.20e-16 normwise (`dense6`).
/// **After gh:#998:** worst PETIR vs stock `R` 3.20e-16 (`dense6`);
/// `scaled3` 0 (was 1.11e-11).
#[test]
fn expm_matches_octave() {
    let Some(cases) = load_in("octave", FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    assert_eq!(cases.len(), 14);
    let mut s_nb = Score::default();
    let mut s_r = Score::default();
    let mut worst_st = (0.0_f64, String::new());
    for (name, case) in &cases {
        let a = case.matrix("A").unwrap();
        let r = expm(&a).unwrap();
        for (idx, v) in case.tag("RNB") {
            s_nb.add(
                || format!("{name} RNB[{}][{}]", idx[0], idx[1]),
                *v,
                r.get(idx[0], idx[1]),
            );
        }
        for (idx, v) in case.tag("R") {
            s_r.add(
                || format!("{name} R[{}][{}]", idx[0], idx[1]),
                *v,
                r.get(idx[0], idx[1]),
            );
        }
        let nb = normwise(case, "RNB", &r);
        let st = normwise(case, "R", &r);
        let rnb = case.matrix("RNB").unwrap();
        let bal = normwise(case, "R", &rnb);
        eprintln!(
            "{name:12} petir-vs-nobal {nb:9.2e}   petir-vs-octave {st:9.2e}   \
             octave-nobal-vs-octave {bal:9.2e}"
        );
        if st > worst_st.0 {
            worst_st = (st, name.clone());
        }
    }
    eprintln!(
        "{}",
        s_nb.summary("expm vs Octave expm without balancing (per entry)")
    );
    eprintln!("{}", s_r.summary("expm vs stock Octave expm (per entry)"));
    assert!(
        worst_st.0 < 1e-13,
        "worst normwise vs stock Octave: {worst_st:?}"
    );
}

/// `expm(A dt) n0` against PETIR's CRAM48 (`depletion::cram48`, ported from
/// OpenMC) on the decay chain, where both apply (gh:#993 asks for this
/// comparison).
///
/// # Methodology
///
/// The chain is `reference-data/octave`'s `chain4` (decay constants
/// `0.1, 0.03, 0.005, 0` per unit time, step 10), applied to
/// `n0 = [1, 0.5, 0.25, 0]`. CRAM48's approximation error on the negative
/// real axis is far below `f64` round-off, so the two methods should agree
/// to round-off relative to `max n`; the gate is `1e-13`.
///
/// # Results (2026-10-20)
///
/// `max |expm - cram48| / max n` = **6.8e-16**.
#[test]
fn expm_agrees_with_cram48_on_a_decay_chain() {
    use petir::depletion::{cram48, DepletionMatrix};
    let l = [1e-1, 3e-2, 5e-3, 0.0];
    let mut a = Matrix::zeros(4, 4).unwrap();
    let mut d = DepletionMatrix::zeros(4);
    for i in 0..4 {
        a.set(i, i, -l[i]);
        d.set(i, i, -l[i]).unwrap();
        if i > 0 {
            a.set(i, i - 1, l[i - 1]);
            d.set(i, i - 1, l[i - 1]).unwrap();
        }
    }
    let n0 = [1.0, 0.5, 0.25, 0.0];
    let dt = 10.0;
    let via_expm = expm(&a.scale(dt)).unwrap().mul_vec(&n0).unwrap();
    let via_cram = cram48(&d, &n0, dt).unwrap();
    let big = via_expm.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let diff = via_expm
        .iter()
        .zip(&via_cram)
        .fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
    eprintln!(
        "expm vs CRAM48 on chain4: max |diff| / max n = {:e}",
        diff / big
    );
    eprintln!("expm  {via_expm:?}\ncram  {via_cram:?}");
    assert!(diff / big < 1e-13, "{:e}", diff / big);
}
