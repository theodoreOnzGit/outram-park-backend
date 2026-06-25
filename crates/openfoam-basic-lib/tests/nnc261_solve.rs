//! Verification test for `SquareMatrix::solve` against NNC261.
//!
//! NNC261 is a 261×261 real non-symmetric sparse matrix (1 500 non-zeros)
//! from the Harwell-Boeing Sparse Matrix Collection, shipped here as
//! `tests/nnc261.mtx` in Matrix Market coordinate format.
//!
//! Citation:
//!   Duff, I. S., Grimes, R. G., & Lewis, J. G. (1989).
//!   Sparse matrix test problems.
//!   ACM Transactions on Mathematical Software (TOMS), 15(1), 1–14.
//!   https://doi.org/10.1145/58846.58847
//!
//! Test strategy
//! -------------
//! Only matrix A is shipped; no reference RHS or solution vector is provided.
//! We therefore construct our own verification problem:
//!
//!   1. Load A from the .mtx file into a dense `SquareMatrix`.
//!   2. Choose a reference solution  x_ref[i] = i + 1  (i = 0 … 260).
//!   3. Compute b = A · x_ref  via dense matrix-vector multiply.
//!   4. Solve A · x = b  with `SquareMatrix::solve` (Crout LU, scaled partial pivoting).
//!   5. Assert the relative residual  ‖A·x − b‖∞ / ‖b‖∞  < 1e-10  (primary check).
//!   6. Assert the relative solution error  ‖x − x_ref‖∞ / ‖x_ref‖∞  < 0.05.
//!
//! Note on tolerances
//! ------------------
//! NNC261 is highly ill-conditioned: entries span from ~1e-8 to ~2.3e2, giving
//! an estimated condition number κ(A) ≈ 1e13.  With IEEE 754 double precision
//! (ε_mach ≈ 2.2e-16), the expected relative solution error is κ · ε_mach ≈ 2e-3,
//! so the x-comparison uses a 5 % bound rather than the usual 1e-6.
//! The residual check ‖A·x − b‖∞ / ‖b‖∞ is the stronger correctness criterion:
//! a small residual means the solver found a solution to the floating-point system
//! it was given, regardless of matrix conditioning.  In practice this reaches ~7e-16.
//!
//! Run with:
//!   cargo test -p openfoam-basic-lib --test nnc261_solve -- --nocapture

use openfoam_basic_lib::matrix::SquareMatrix;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Parse a Matrix Market coordinate file (real, general) into a dense
/// `SquareMatrix`. Comment lines beginning with `%` are skipped.
/// Row and column indices in the file are 1-based.
fn load_matrix_market(contents: &str) -> SquareMatrix {
    let mut lines = contents.lines().filter(|l| !l.starts_with('%'));

    // First non-comment line: "nrows ncols nnz"
    let header = lines.next().expect("missing dimension line");
    let mut parts = header.split_whitespace();
    let nrows: usize = parts.next().unwrap().parse().unwrap();
    let ncols: usize = parts.next().unwrap().parse().unwrap();
    assert_eq!(nrows, ncols, "expected a square matrix");

    let mut mat = SquareMatrix::new(nrows);

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let row: usize = parts.next().unwrap().parse::<usize>().unwrap() - 1;
        let col: usize = parts.next().unwrap().parse::<usize>().unwrap() - 1;
        let val: f64 = parts.next().unwrap().parse().unwrap();
        mat.add(row, col, val);
    }

    mat
}

/// Dense matrix-vector product b = A · x.
fn mat_vec(a: &SquareMatrix, x: &[f64]) -> Vec<f64> {
    let n = a.n();
    (0..n)
        .map(|i| (0..n).map(|j| a.get(i, j) * x[j]).sum::<f64>())
        .collect()
}

/// Infinity norm (max absolute value).
fn norm_inf(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).fold(0.0_f64, f64::max)
}

// ── test ─────────────────────────────────────────────────────────────────────

#[test]
fn nnc261_lu_roundtrip() {
    let mtx_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("nnc261.mtx");
    let contents =
        std::fs::read_to_string(&mtx_path).expect("could not read tests/nnc261.mtx");

    let a = load_matrix_market(&contents);
    assert_eq!(a.n(), 261, "expected a 261×261 matrix");

    // Reference solution: x_ref[i] = i + 1  (1, 2, …, 261)
    let n = a.n();
    let x_ref: Vec<f64> = (1..=n).map(|i| i as f64).collect();

    // Forward pass: b = A · x_ref
    let b = mat_vec(&a, &x_ref);

    // Solve A · x = b
    let x_solved = a.solve(&b).expect("NNC261 must be non-singular");

    // ── check 1 (primary): relative residual ‖A·x_solved − b‖∞ / ‖b‖∞ < 1e-10 ──
    //
    // How it is computed:
    //   ax[i]       = Σ_j  A[i,j] · x_solved[j]   (dense mat-vec via `mat_vec`)
    //   residual[i] = ax[i] − b[i]                 (how much A·x misses the RHS)
    //   rel_residual = max_i|residual[i]| / max_i|b[i]|   (∞-norm ratio)
    //
    // A small residual means x_solved nearly satisfies A·x = b in floating-point
    // arithmetic, regardless of conditioning.  NNC261 routinely reaches ~7e-16
    // (near machine precision ε_mach ≈ 2.2e-16) with scaled partial pivoting.
    let ax = mat_vec(&a, &x_solved);
    let residual: Vec<f64> = ax.iter().zip(b.iter()).map(|(a, b)| a - b).collect();
    let rel_residual = norm_inf(&residual) / norm_inf(&b);
    eprintln!("NNC261 relative residual:      {rel_residual:.3e}  (bound 1e-10)");
    assert!(
        rel_residual < 1e-10,
        "relative residual {rel_residual:.3e} exceeds 1e-10"
    );

    // ── check 2 (secondary): relative solution error ‖x_solved − x_ref‖∞ / ‖x_ref‖∞ < 0.05 ──
    //
    // How it is computed:
    //   err[i]    = x_solved[i] − x_ref[i]         (per-component difference from the
    //                                                known exact solution x_ref[i] = i+1)
    //   rel_err   = max_i|err[i]| / max_i|x_ref[i]|   (∞-norm ratio)
    //
    // Unlike the residual, this measures actual solution accuracy.  For an
    // ill-conditioned system the two can differ enormously: a small residual
    // ‖Ax−b‖ does not guarantee a small solution error ‖x−x_ref‖ when κ(A)
    // is large.  NNC261 has κ(A) ≈ 1e13, so the expected rounding error is
    // κ · ε_mach ≈ 2e-3; in practice it runs ~1.2%, comfortably inside 5%.
    let err: Vec<f64> = x_solved
        .iter()
        .zip(x_ref.iter())
        .map(|(a, b)| a - b)
        .collect();
    let rel_err = norm_inf(&err) / norm_inf(&x_ref);
    eprintln!("NNC261 relative solution error: {rel_err:.3e}  (bound 0.05, κ ≈ 1e13)");
    assert!(
        rel_err < 0.05,
        "relative solution error {rel_err:.3e} exceeds 0.05"
    );
}
