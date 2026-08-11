//! Dense linear solve for the solver's Newton and Broyden steps.
//!
//! # What this module is
//!
//! One routine, [`solve_dense`], that solves `A x = b` for a small dense square
//! `A`. It stands in for DWSIM's `MathEx.SysLin.rsolve.rmatrixsolve`, which the
//! simultaneous adjust solver calls once per Newton iteration
//! (`FlowsheetSolver.vb:2010`, `:2119`).
//!
//! All quantities here are **dimensionless plain `f64`** by construction: the
//! matrix is a Jacobian `d f_i / d x_j` whose rows and columns carry whatever
//! units the adjust variables do, so no single `uom` type applies. The callers
//! ([`crate::flowsheet_solver::adjust`],
//! [`crate::flowsheet_solver::recycle::broydn`]) document the units of their own
//! vectors.
//!
//! # Why not reuse the crate's other linear algebra
//!
//! `crate::columns` carries its own `linalg` module for the MESH column solvers.
//! This one is deliberately separate and self-contained: the flowsheet solver
//! must not depend on the column workstream's internals, and the routine is 40
//! lines.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary source: the call sites `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:2010`
//! and `:2119`, whose contract (`Boolean` success, solution written into `dx`)
//! this module reproduces as `Option<Vec<f64>>`.
//!
//! # Excluded DWSIM behavior
//!
//! - **ALGLIB's `rmatrixsolve` internals.** Upstream calls into a vendored
//!   ALGLIB LU solve with condition-number estimation and iterative refinement.
//!   This port implements plain **Gaussian elimination with partial pivoting**
//!   and reports failure on an exactly-singular pivot. It therefore accepts some
//!   ill-conditioned systems ALGLIB would reject; the Newton step that consumes
//!   the result is damped and capped regardless
//!   (FlowsheetSolver.vb:2013-2026), which is what upstream relies on for
//!   robustness.
//! - **`ndarray-linalg` / LAPACK.** Not used: the workspace's Android/Termux
//!   rule forbids a system BLAS dependency in library code.

/// Solve the dense square system `A x = b`.
///
/// # Arguments
///
/// - `a` — the `n x n` coefficient matrix in **row-major** order, i.e.
///   `a[i][j]` is row `i`, column `j`. Consumed (the elimination works in
///   place on a copy the caller no longer needs).
/// - `b` — the right-hand side, length `n`.
///
/// # Returns
///
/// `Some(x)` with `x.len() == n` on success, or `None` when the matrix is
/// singular to working precision (a pivot column that is entirely zero or
/// non-finite), when the shapes disagree, or when `n == 0`. `None` is upstream's
/// `success = False` (FlowsheetSolver.vb:2011), on which the Newton step is
/// simply skipped.
///
/// # Method
///
/// Gaussian elimination with partial pivoting followed by back substitution.
/// `O(n^3)`; `n` is the number of simultaneously-solved adjust blocks, which is
/// a handful in practice.
#[must_use]
pub fn solve_dense(a: Vec<Vec<f64>>, b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    if n == 0 || a.len() != n || a.iter().any(|row| row.len() != n) {
        return None;
    }

    let mut m = a;
    let mut rhs = b.to_vec();

    for col in 0..n {
        // Partial pivot: largest magnitude in the remaining column.
        let mut pivot = col;
        let mut best = m[col][col].abs();
        for (row, m_row) in m.iter().enumerate().take(n).skip(col + 1) {
            let v = m_row[col].abs();
            if v > best {
                best = v;
                pivot = row;
            }
        }
        if !best.is_finite() || best == 0.0 {
            return None;
        }
        if pivot != col {
            m.swap(pivot, col);
            rhs.swap(pivot, col);
        }

        let diag = m[col][col];
        for row in (col + 1)..n {
            let factor = m[row][col] / diag;
            if factor == 0.0 {
                continue;
            }
            for k in col..n {
                m[row][k] -= factor * m[col][k];
            }
            rhs[row] -= factor * rhs[col];
        }
    }

    // Back substitution.
    let mut x = vec![0.0_f64; n];
    for row in (0..n).rev() {
        let mut acc = rhs[row];
        for col in (row + 1)..n {
            acc -= m[row][col] * x[col];
        }
        let diag = m[row][row];
        if diag == 0.0 || !diag.is_finite() {
            return None;
        }
        x[row] = acc / diag;
        if !x[row].is_finite() {
            return None;
        }
    }
    Some(x)
}

/// Sum of the absolute values of a vector — DWSIM's `MathEx.Common.AbsSum`
/// (used at `FlowsheetSolver.vb:2037`, `:2146` as the Newton step-size test).
///
/// Dimensionless in the sense that it mixes whatever units the components carry;
/// upstream compares it against a bare `1e-6`, and so does this port.
#[must_use]
pub fn abs_sum(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).sum()
}

/// Sum of the squares of a vector — DWSIM's `AbsSqrSumY` extension
/// (`FlowsheetSolver.vb:1993`, `:2102`), the "NSSE" the adjust solver logs.
///
/// Dimensionless in the same sense as [`abs_sum`].
#[must_use]
pub fn abs_sqr_sum(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum()
}

#[cfg(test)]
mod tests {
    //! # Verification — dense linear solve
    //!
    //! **Methodology.** Solve systems with known exact solutions and check the
    //! residual, then check that a singular matrix is rejected. Tolerance
    //! `1e-12` absolute on each component. Verification of the numerics only; no
    //! physics is involved.
    //! **Results (2026-08-11, release build):** recorded per test below.

    use super::*;

    /// **Methodology.** Solve `[[2,1],[1,3]] x = [5,10]`, whose exact solution
    /// is `x = [1, 3]`, and a 3x3 system requiring a row swap
    /// (`[[0,1,1],[1,0,1],[1,1,0]] x = [3,4,5]`, exact `x = [3,2,1]`).
    /// **Result (2026-08-11):** `[1, 3]` and `[3, 2, 1]`, both to within
    /// `1e-13`.
    #[test]
    fn solves_small_systems_including_a_pivot_swap() {
        let x = solve_dense(vec![vec![2.0, 1.0], vec![1.0, 3.0]], &[5.0, 10.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12, "{x:?}");
        assert!((x[1] - 3.0).abs() < 1e-12, "{x:?}");

        let a = vec![
            vec![0.0, 1.0, 1.0],
            vec![1.0, 0.0, 1.0],
            vec![1.0, 1.0, 0.0],
        ];
        let x = solve_dense(a, &[3.0, 4.0, 5.0]).unwrap();
        for (got, want) in x.iter().zip([3.0, 2.0, 1.0]) {
            assert!((got - want).abs() < 1e-12, "{x:?}");
        }
    }

    /// **Methodology.** A singular matrix (`[[1,2],[2,4]]`), a shape mismatch,
    /// and an empty system must all return `None` — upstream's
    /// `success = False`.
    /// **Result (2026-08-11):** all three return `None`.
    #[test]
    fn singular_and_malformed_systems_return_none() {
        assert!(solve_dense(vec![vec![1.0, 2.0], vec![2.0, 4.0]], &[1.0, 2.0]).is_none());
        assert!(solve_dense(vec![vec![1.0, 2.0]], &[1.0, 2.0]).is_none());
        assert!(solve_dense(vec![], &[]).is_none());
    }

    /// **Methodology.** Check the two norm helpers against hand arithmetic.
    /// **Result (2026-08-11):** `abs_sum([-1, 2, -3]) = 6`;
    /// `abs_sqr_sum([-1, 2, -3]) = 14`.
    #[test]
    fn norm_helpers_match_hand_arithmetic() {
        assert!((abs_sum(&[-1.0, 2.0, -3.0]) - 6.0).abs() < 1e-15);
        assert!((abs_sqr_sum(&[-1.0, 2.0, -3.0]) - 14.0).abs() < 1e-15);
    }
}
