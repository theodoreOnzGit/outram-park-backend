/// Timed comparison: `SquareMatrix::solve` (pure-Rust LU with scaled partial
/// pivoting) vs `ndarray-linalg` LAPACK (`Array2::solve`, OpenBLAS/Intel-MKL)
/// across matrix sizes representative of the TUAS conductance matrices (5–200×200).
///
/// Run with:
///   cargo test -p openfoam-basic-lib --test matrix_bench --release -- --nocapture
///
/// ## Results (release mode, Linux x86-64, OpenBLAS, 2026-06-24)
///
/// ```text
///      n   sq_mat (ns)  ndarray (ns)       ratio     iters
/// ----------------------------------------------------------
///      5           193           371        0.52     20000
///     10           352           512        0.69     20000
///     20          1446          1614        0.90      8000
///     50         17018          7891        2.16      2000
///    100        135705         27845        4.87       400
///    200       1112109        357281        3.11        80
/// ```
///
/// ratio < 1.0 → SquareMatrix faster; ratio > 1.0 → ndarray-linalg faster.
///
/// ## Key findings
///
/// - **n ≤ 10 (typical TUAS small networks):** `SquareMatrix` is **1.5–1.9×
///   faster** than LAPACK. OpenBLAS's DGESV has per-call FFI overhead
///   (~300–400 ns) that dominates at small n; pure-Rust eliminates it.
/// - **n ≈ 20 (medium TUAS arrays):** Within measurement noise — essentially
///   equal at ~1.5 µs per solve.
/// - **n ≥ 50:** OpenBLAS pulls ahead via cache-blocked BLAS-3 kernels.
///   At n = 200 it is ~3× faster than pure Rust.
///
/// ## Implication for TUAS migration
///
/// TUAS conductance matrices are typically 10–50×10–50 (nodes in a 1D
/// array section). In this range `SquareMatrix` is competitive (≤2.2×) and
/// eliminates the system OpenBLAS build dependency entirely. The crossover
/// where LAPACK becomes significantly faster is around n ≈ 50.
use std::time::Instant;

use ndarray::{Array1, Array2};
use ndarray_linalg::Solve;
use openfoam_basic_lib::matrix::SquareMatrix;

/// Build an n×n diagonally-dominant matrix and an n-vector RHS.
///
/// Off-diagonal entries are a deterministic pseudo-random pattern;
/// the diagonal is boosted by 20·n so the matrix is well-conditioned
/// at all sizes (condition number ≈ O(1), not O(n²) like a Hilbert matrix).
fn make_problem(n: usize) -> (SquareMatrix, Array2<f64>, Vec<f64>) {
    let mut sq = SquareMatrix::new(n);
    let mut nd = Array2::<f64>::zeros((n, n));

    for i in 0..n {
        for j in 0..n {
            let v = ((i.wrapping_mul(7).wrapping_add(j.wrapping_mul(13)).wrapping_add(1)) % 17 + 1)
                as f64;
            sq.set(i, j, v);
            nd[[i, j]] = v;
        }
        let diag_boost = 20.0 * n as f64;
        sq.add(i, i, diag_boost);
        nd[[i, i]] += diag_boost;
    }

    let rhs: Vec<f64> = (0..n).map(|i| (i + 1) as f64).collect();
    (sq, nd, rhs)
}

fn iters(n: usize) -> usize {
    match n {
        0..=10  => 20_000,
        11..=20 => 8_000,
        21..=50 => 2_000,
        51..=100 => 400,
        _       => 80,
    }
}

#[test]
fn bench_lu_solve_vs_ndarray_linalg() {
    let sizes: &[usize] = &[5, 10, 20, 50, 100, 200];

    eprintln!();
    eprintln!(
        "{:>6}  {:>12}  {:>12}  {:>10}  {:>8}",
        "n", "sq_mat (ns)", "ndarray (ns)", "ratio", "iters"
    );
    eprintln!("{}", "-".repeat(58));

    for &n in sizes {
        let reps = iters(n);
        let (sq, nd_ref, rhs) = make_problem(n);
        let nd_rhs = Array1::from(rhs.clone());

        // ---- warm-up (avoids JIT / cache cold-start artefacts) ----
        for _ in 0..5 {
            let _ = sq.solve(&rhs);
            let _ = nd_ref.solve(&nd_rhs).unwrap();
        }

        // ---- SquareMatrix (pure-Rust LU) ----
        let t0 = Instant::now();
        for _ in 0..reps {
            let _ = sq.solve(&rhs);
        }
        let sq_ns = t0.elapsed().as_nanos() / reps as u128;

        // ---- ndarray-linalg (LAPACK via OpenBLAS / Intel-MKL) ----
        // `solve` borrows self so no per-iter clone of the matrix is needed,
        // matching what TUAS does (`M.solve(&S)` on a freshly-built Array2).
        let t0 = Instant::now();
        for _ in 0..reps {
            let _ = nd_ref.solve(&nd_rhs).unwrap();
        }
        let nd_ns = t0.elapsed().as_nanos() / reps as u128;

        let ratio = sq_ns as f64 / nd_ns as f64;
        eprintln!(
            "{:>6}  {:>12}  {:>12}  {:>10.2}  {:>8}",
            n, sq_ns, nd_ns, ratio, reps
        );
    }

    eprintln!();
    eprintln!("ratio < 1.0 → SquareMatrix faster; ratio > 1.0 → ndarray-linalg faster");
}
