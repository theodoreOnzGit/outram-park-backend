// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::lu_decomp` code-to-code against GSL 2.8 compiled
//! and run** (gh:#992).
//!
//! # Methodology
//!
//! GSL 2.8's `gsl_linalg_LU_decomp` and `gsl_linalg_LU_solve`, built from the
//! vendored tree as a static library with GSL's own CBLAS, were run on nine
//! cases by `reference-data/gsl/lu_reference_driver.c`; the output is
//! committed at `reference-data/gsl/lu-gsl-2.8-reference.txt`. The file
//! carries the inputs too, so this test reads `A` and `b` from it rather than
//! regenerating them. For each case it compares, value by value, the packed
//! LU factor, the permutation, `signum`, the status and the solution.
//!
//! Cases: a 2x2; a 3x3 with a zero leading pivot (row swap); a 3x3 with a tie
//! in the pivot column (`idamax` must take the first row); a 4x4 with rows
//! six orders of magnitude apart (where scaled and unscaled pivoting choose
//! differently); a 2x2 whose pivot is below `DBL_MIN` (GSL divides rather
//! than multiplying by the reciprocal); an exactly singular 3x3; and
//! unstructured 6x6, 24x24 and 30x30 matrices.
//!
//! **Pass criterion, fixed before the first run:** every case with
//! `n <= 24` — GSL's own Level-2 path, which is what is ported — is
//! **bit-identical** in every value. The 30x30 case goes through GSL's
//! recursive Level-3 `LU_decomp_L3`, which is not ported (the documented
//! deviation in `src/linalg/lu_decomp.rs`); there the permutation and
//! `signum` must match exactly and the solution must agree to `1e-10`
//! relative, and the measured difference is recorded below.
//!
//! **Prediction, written before the first run:** all `n <= 24` cases
//! bit-identical, because the loop orders of `cblas/source_ger.h` and
//! `source_trsv_r.h` are reproduced and nothing is fused; the 30x30 differs in
//! the last one or two ulps in some factor entries, with the same pivots.
//!
//! # Results (2026-10-20)
//!
//! ```text
//!           square2 n= 2:   6/6   bit-identical
//!             swap3 n= 3:  12/12  bit-identical
//!              tie3 n= 3:  12/12  bit-identical
//!           scaled4 n= 4:  20/20  bit-identical
//!        subnormal2 n= 2:   6/6   bit-identical
//!         singular3 n= 3:   9/9   bit-identical (status 3, solve refused)
//!           filled6 n= 6:  42/42  bit-identical
//!          filled24 n=24: 600/600 bit-identical
//!   filled30_level3 n=30: 930/930 bit-identical
//!   all cases: 1637/1637 bit-identical, worst relative difference 0
//! ```
//!
//! The `n <= 24` prediction held. **The 30x30 prediction was refuted**: the
//! Level-3 path is bit-identical too. Reading why: `LU_decomp_L3` splits at
//! 16, runs `LU_decomp_L2` on the 30x16 panel, then `dtrsm` and `dgemm` on the
//! rest; GSL's own reference CBLAS walks both with the inner index ascending,
//! which is the same sequence of rank-1 updates the Level-2 code performs.
//! So on GSL's own CBLAS the deviation is measured as zero **on this case**;
//! it is not proven zero in general, and against a blocked BLAS (OpenBLAS,
//! MKL) it would not be.
//!
//! Interpretation: the port reproduces GSL's arithmetic, not merely its
//! answer, including the pivot choice on ties and the `DBL_MIN` division
//! branch.
//!
//! # Why the reference is not the system libgsl
//!
//! Arch's `gsl 2.8-1` links netlib `libcblas`/`libblas`, not GSL's own CBLAS.
//! netlib's `DTRSV` walks the back-substitution inner loop in the opposite
//! order to `cblas/source_trsv_r.h`, so it rounds differently and is not the
//! code this module ports. Measured: the same driver linked against the
//! system library differs from the static vendored build on **44 of the
//! file's lines**. The committed reference is from the static vendored build.
//!
//! # Skipping
//!
//! If the reference file is absent the test skips rather than fails.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use petir::linalg::{lu_decomp, lu_solve, Matrix};

#[derive(Default)]
struct Case {
    n: usize,
    a: BTreeMap<(usize, usize), f64>,
    b: Vec<f64>,
    status: i32,
    signum: i32,
    perm: Vec<usize>,
    lu: BTreeMap<(usize, usize), f64>,
    solve_ok: bool,
    x: Vec<f64>,
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/lu-gsl-2.8-reference.txt")
}

fn load() -> Option<Vec<(String, Case)>> {
    let text = fs::read_to_string(reference_path()).ok()?;
    let mut cases: Vec<(String, Case)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(&tag) = f.first() else { continue };
        if tag == "CASE" {
            let case = Case {
                n: f[2].parse().unwrap(),
                ..Case::default()
            };
            cases.push((f[1].to_string(), case));
            continue;
        }
        let Some((_, c)) = cases.last_mut() else {
            continue;
        };
        match tag {
            "A" => {
                c.a.insert(
                    (f[1].parse().unwrap(), f[2].parse().unwrap()),
                    f[3].parse().unwrap(),
                );
            }
            "B" => c.b.push(f[2].parse().unwrap()),
            "STATUS" => c.status = f[1].parse().unwrap(),
            "SIGNUM" => c.signum = f[1].parse().unwrap(),
            "P" => c.perm.push(f[2].parse().unwrap()),
            "LU" => {
                c.lu.insert(
                    (f[1].parse().unwrap(), f[2].parse().unwrap()),
                    f[3].parse().unwrap(),
                );
            }
            "SOLVE" => c.solve_ok = f[1] == "0",
            "X" => c.x.push(f[2].parse().unwrap()),
            _ => {}
        }
    }
    Some(cases)
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[test]
fn lu_decomp_and_solve_reproduce_compiled_gsl() {
    let Some(cases) = load() else {
        eprintln!("SKIP: {} not found", reference_path().display());
        return;
    };
    assert_eq!(cases.len(), 9, "the driver emits nine cases");

    let (mut total, mut identical) = (0_usize, 0_usize);
    for (name, c) in &cases {
        let n = c.n;
        let mut data = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                data.push(c.a[&(i, j)]);
            }
        }
        let mut a = Matrix::from_row_major(n, n, data).unwrap();
        let p = lu_decomp(&mut a).unwrap();

        // Status, sign and permutation: exact in every case.
        let status = p.zero_pivot.map_or(0, |k| k as i32 + 1);
        assert_eq!(status, c.status, "{name}: status");
        assert_eq!(p.signum, c.signum, "{name}: signum");
        assert_eq!(p.perm, c.perm, "{name}: permutation");

        let (mut case_total, mut case_identical, mut worst_lu) = (0, 0, 0.0_f64);
        for (&(i, j), &want) in &c.lu {
            let got = a.get(i, j);
            case_total += 1;
            if got.to_bits() == want.to_bits() {
                case_identical += 1;
            }
            worst_lu = worst_lu.max(rel(got, want));
        }

        let solved = lu_solve(&a, &p, &c.b);
        assert_eq!(solved.is_ok(), c.solve_ok, "{name}: solve status");
        let mut worst_x = 0.0_f64;
        if let Ok(x) = solved {
            for (&got, &want) in x.iter().zip(c.x.iter()) {
                case_total += 1;
                if got.to_bits() == want.to_bits() {
                    case_identical += 1;
                }
                worst_x = worst_x.max(rel(got, want));
            }
        }
        println!(
            "{name:>16} n={n:>2}: {case_identical}/{case_total} bit-identical, \
             worst rel LU {worst_lu:.3e}, worst rel x {worst_x:.3e}"
        );
        total += case_total;
        identical += case_identical;

        if n <= 24 {
            assert_eq!(
                case_identical, case_total,
                "{name}: GSL's own Level-2 path must be reproduced bit for bit"
            );
        } else {
            assert!(
                worst_x <= 1e-10,
                "{name}: Level-3 solution differs by {worst_x:e}"
            );
        }
    }
    println!("all cases: {identical}/{total} bit-identical");
}
