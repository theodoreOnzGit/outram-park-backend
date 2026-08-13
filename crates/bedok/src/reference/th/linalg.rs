//! Small dense linear solver standing in for MATLAB's sparse backslash.
//!
//! # Provenance
//!
//! Supporting code for the translation of Than Yan Ren's (SNRSI) MATLAB,
//! snapshot sha256 `e45cd6f57be2087c…`. It has no `.m` counterpart: it
//! replaces the `laplc \ bvec` line that closes both
//! `fuelrodheat_1dcylnd.m` and `fuelrodheattime_1dcylnd.m`.
//!
//! # Why a dense solve
//!
//! MATLAB assembles the rod-conduction operator with `sparse(...)` and solves
//! it with `\`, which for a square sparse matrix runs UMFPACK's LU with
//! partial pivoting. The matrix is `maxid x maxid` — **24 x 24** for the
//! NEACRP layout — so sparsity buys nothing, and a dense LU with partial
//! pivoting is the same algorithm on the same data. Keeping it in-crate also
//! keeps the factorisation deterministic and inspectable, which matters for a
//! reference implementation whose job is to be diffed against.
//!
//! Row ordering, pivoting rule and elimination order are the only things that
//! can move the last bits of the answer; they are documented at
//! [`solve_dense_lu`].

use super::{ThError, ThResult};

/// A dense square matrix in row-major order, assembled from triplets.
///
/// Mirrors MATLAB's `sparse(rows, cols, values, n, n)`: **duplicate
/// `(row, col)` entries are summed**, not overwritten. The rod-conduction
/// assembly relies on that behaviour for its diagonal.
#[derive(Debug, Clone, PartialEq)]
pub struct DenseMatrix {
    /// Side length of the square matrix.
    order: usize,
    /// `order * order` entries, row-major.
    values: Vec<f64>,
}

impl DenseMatrix {
    /// An `order` x `order` matrix of zeros.
    #[must_use]
    pub fn zeros(order: usize) -> Self {
        Self {
            order,
            values: vec![0.0; order * order],
        }
    }

    /// Side length of the matrix.
    #[must_use]
    pub const fn order(&self) -> usize {
        self.order
    }

    /// Add `value` to entry `(row, col)`, both 0-based.
    ///
    /// This is *accumulate*, not *assign*, so that a triplet list translates
    /// straight across from MATLAB's `sparse` constructor.
    ///
    /// # Panics
    ///
    /// If `row` or `col` is out of range.
    pub fn accumulate(&mut self, row: usize, col: usize, value: f64) {
        assert!(row < self.order, "row {row} >= order {}", self.order);
        assert!(col < self.order, "col {col} >= order {}", self.order);
        self.values[row * self.order + col] += value;
    }

    /// Overwrite entry `(row, col)` with `value`, both 0-based.
    ///
    /// # Panics
    ///
    /// If `row` or `col` is out of range.
    pub fn set(&mut self, row: usize, col: usize, value: f64) {
        assert!(row < self.order, "row {row} >= order {}", self.order);
        assert!(col < self.order, "col {col} >= order {}", self.order);
        self.values[row * self.order + col] = value;
    }

    /// Entry `(row, col)`, both 0-based.
    ///
    /// # Panics
    ///
    /// If `row` or `col` is out of range.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        assert!(row < self.order, "row {row} >= order {}", self.order);
        assert!(col < self.order, "col {col} >= order {}", self.order);
        self.values[row * self.order + col]
    }

    /// Matrix-vector product `A * x`.
    ///
    /// # Panics
    ///
    /// If `x.len() != order`.
    #[must_use]
    pub fn multiply(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(x.len(), self.order, "vector length must equal matrix order");
        (0..self.order)
            .map(|row| {
                (0..self.order)
                    .map(|col| self.values[row * self.order + col] * x[col])
                    .sum()
            })
            .collect()
    }
}

/// Solve `a * x = b` by Gaussian elimination with partial pivoting.
///
/// The pivot at step `k` is the row at or below `k` with the largest absolute
/// entry in column `k`; ties keep the earlier row. Elimination proceeds
/// column by column, then back substitution runs from the last row upward.
/// This is the same algorithm MATLAB's `\` uses for a general square system,
/// so results agree to within the usual reordering-of-additions noise.
///
/// # Arguments
///
/// - `a` — the system matrix; consumed, since it is factorised in place.
/// - `b` — right-hand side, length `a.order()`.
/// - `what` — a label carried into the error, for diagnostics.
///
/// # Returns
///
/// The solution vector `x`, length `a.order()`.
///
/// # Errors
///
/// [`ThError::SingularMatrix`] if the largest available pivot in some column
/// is exactly zero. A `NaN` pivot is **not** treated as singular: the MATLAB
/// lets NaN propagate into `results` and its callers test for it afterwards
/// (`if any(isnan(results))`), so this does too.
///
/// # Panics
///
/// If `b.len() != a.order()`.
pub fn solve_dense_lu(mut a: DenseMatrix, b: &[f64], what: &'static str) -> ThResult<Vec<f64>> {
    let n = a.order();
    assert_eq!(b.len(), n, "right-hand side length must equal matrix order");
    let mut x = b.to_vec();

    for k in 0..n {
        // Partial pivot: largest magnitude at or below the diagonal.
        let mut pivot_row = k;
        let mut pivot_magnitude = a.get(k, k).abs();
        for row in (k + 1)..n {
            let magnitude = a.get(row, k).abs();
            if magnitude > pivot_magnitude {
                pivot_magnitude = magnitude;
                pivot_row = row;
            }
        }
        if pivot_magnitude == 0.0 {
            return Err(ThError::SingularMatrix { what, pivot: k });
        }
        if pivot_row != k {
            for col in 0..n {
                let here = a.get(k, col);
                let there = a.get(pivot_row, col);
                a.set(k, col, there);
                a.set(pivot_row, col, here);
            }
            x.swap(k, pivot_row);
        }

        let pivot = a.get(k, k);
        for row in (k + 1)..n {
            let factor = a.get(row, k) / pivot;
            if factor == 0.0 {
                continue;
            }
            a.set(row, k, 0.0);
            for col in (k + 1)..n {
                let updated = a.get(row, col) - factor * a.get(k, col);
                a.set(row, col, updated);
            }
            x[row] -= factor * x[k];
        }
    }

    // Back substitution.
    for row in (0..n).rev() {
        let mut sum = x[row];
        for col in (row + 1)..n {
            sum -= a.get(row, col) * x[col];
        }
        x[row] = sum / a.get(row, row);
    }

    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_small_system_exactly() {
        // [2 1; 1 3] x = [5; 10]  ->  x = [1; 3]
        let mut a = DenseMatrix::zeros(2);
        a.set(0, 0, 2.0);
        a.set(0, 1, 1.0);
        a.set(1, 0, 1.0);
        a.set(1, 1, 3.0);
        let x = solve_dense_lu(a, &[5.0, 10.0], "test").expect("nonsingular");
        assert!((x[0] - 1.0).abs() < 1e-12, "{x:?}");
        assert!((x[1] - 3.0).abs() < 1e-12, "{x:?}");
    }

    #[test]
    fn pivots_when_the_leading_entry_is_zero() {
        // [0 1; 1 0] x = [2; 3]  ->  x = [3; 2]
        let mut a = DenseMatrix::zeros(2);
        a.set(0, 1, 1.0);
        a.set(1, 0, 1.0);
        let x = solve_dense_lu(a, &[2.0, 3.0], "test").expect("nonsingular");
        assert!((x[0] - 3.0).abs() < 1e-12, "{x:?}");
        assert!((x[1] - 2.0).abs() < 1e-12, "{x:?}");
    }

    #[test]
    fn reproduces_the_right_hand_side_on_a_tridiagonal_system() {
        // A 1-D Laplacian with Dirichlet ends: residual must vanish.
        let n = 40;
        let mut a = DenseMatrix::zeros(n);
        for i in 0..n {
            a.accumulate(i, i, 2.0);
            if i > 0 {
                a.accumulate(i, i - 1, -1.0);
            }
            if i + 1 < n {
                a.accumulate(i, i + 1, -1.0);
            }
        }
        let b: Vec<f64> = (0..n).map(|i| (i as f64) * 0.25 + 1.0).collect();
        let x = solve_dense_lu(a.clone(), &b, "test").expect("nonsingular");
        let residual = a.multiply(&x);
        for (got, want) in residual.iter().zip(b.iter()) {
            assert!((got - want).abs() < 1e-9, "got {got}, want {want}");
        }
    }

    #[test]
    fn duplicate_triplets_accumulate_like_matlab_sparse() {
        let mut a = DenseMatrix::zeros(2);
        a.accumulate(0, 0, 1.5);
        a.accumulate(0, 0, 2.5);
        assert_eq!(a.get(0, 0), 4.0);
    }

    #[test]
    fn a_singular_matrix_is_reported_not_silently_solved() {
        let mut a = DenseMatrix::zeros(2);
        a.set(0, 0, 1.0);
        a.set(0, 1, 2.0);
        a.set(1, 0, 2.0);
        a.set(1, 1, 4.0);
        let err = solve_dense_lu(a, &[1.0, 2.0], "test").unwrap_err();
        assert!(
            matches!(err, ThError::SingularMatrix { pivot: 1, .. }),
            "{err}"
        );
    }
}
