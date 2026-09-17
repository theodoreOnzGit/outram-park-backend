// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! A dense rectangular `m x n` matrix of `f64`, row-major.
//!
//! # Lineage
//!
//! **Neither ported nor lifted — crate-local plumbing.** It computes nothing;
//! it is storage plus bounds-checked access, in the same category as
//! [`crate::zip`]. The crate `CLAUDE.md` rule that numerics must be ported
//! from a real library governs *algorithms*, and there is no algorithm here.
//! Everything that does arithmetic on one of these — [`crate::linalg::qr`] —
//! is a port, and says so.
//!
//! # Why a second matrix type
//!
//! [`SquareMatrix`](crate::linalg::SquareMatrix) is square, and is a verbatim
//! lift from `outram-foam-basic-lib` that must stay byte-identical to its
//! source. A least-squares fit is inherently rectangular: `m` sample points
//! against `n` basis functions, with `m >= n`. Widening the lift to cover that
//! would break its provenance, so this is a separate type and the two do not
//! share storage.
//!
//! Checked before writing it, per the workspace "search first" rule: neither
//! `outram-foam-basic-lib/src/matrix/` nor any other crate in this workspace
//! has a rectangular dense matrix, a QR, or an SVD (searched 2026-09-15).
//! `fvc::grad_least_squares` is a fixed 3x3 cell gradient and
//! `outram-park-fork-dwsim-libs`' `lm.rs` solves Levenberg-Marquardt's normal
//! equations — neither is a general rectangular least-squares.
//!
//! # Out-of-range access does not panic
//!
//! [`get`](Matrix::get) returns `NaN` and [`set`](Matrix::set) discards, for
//! the same reason the rest of the crate does: PETIR targets bare metal, where
//! a panic ends the program. See `tests/no_panic_gate.rs`.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

use crate::{PetirError, Result};

/// A dense `rows x cols` matrix of `f64`, stored row-major.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    /// Allocate a `rows x cols` matrix of zeros.
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] if either dimension is zero, or if
    /// `rows * cols` overflows `usize`.
    pub fn zeros(rows: usize, cols: usize) -> Result<Self> {
        if rows == 0 || cols == 0 {
            return Err(PetirError::Invalid);
        }
        let Some(len) = rows.checked_mul(cols) else {
            return Err(PetirError::Invalid);
        };
        Ok(Self {
            rows,
            cols,
            data: vec![0.0; len],
        })
    }

    /// Wrap an existing row-major buffer.
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `data.len() != rows * cols`,
    /// [`PetirError::Invalid`] for a zero dimension.
    pub fn from_row_major(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self> {
        if rows == 0 || cols == 0 {
            return Err(PetirError::Invalid);
        }
        let Some(len) = rows.checked_mul(cols) else {
            return Err(PetirError::Invalid);
        };
        if data.len() != len {
            return Err(PetirError::LengthMismatch {
                expected: len,
                found: data.len(),
            });
        }
        Ok(Self { rows, cols, data })
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Row-major offset of `(i, j)`, saturating rather than wrapping.
    ///
    /// Saturation matters: `i * cols` can overflow `usize` for an absurd `i`,
    /// and an overflow panics under `debug_assertions`. Saturating sends it
    /// past the end of `data`, where the bounds check turns it into the
    /// documented out-of-range behaviour.
    #[inline]
    fn offset(&self, i: usize, j: usize) -> usize {
        if j >= self.cols {
            // Without this, (i, cols + k) would silently alias (i + 1, k).
            return usize::MAX;
        }
        i.saturating_mul(self.cols).saturating_add(j)
    }

    /// Element at row `i`, column `j`; `NaN` if either index is out of range.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        match self.data.get(self.offset(i, j)) {
            Some(&v) => v,
            None => f64::NAN,
        }
    }

    /// Set the element at row `i`, column `j`. Out of range is discarded.
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        let idx = self.offset(i, j);
        if let Some(slot) = self.data.get_mut(idx) {
            *slot = v;
        }
    }

    /// Row `i` as a slice, or `None` if `i >= rows`.
    pub fn row(&self, i: usize) -> Option<&[f64]> {
        let start = i.checked_mul(self.cols)?;
        self.data.get(start..start.checked_add(self.cols)?)
    }

    /// Every row in order.
    pub fn row_iter(&self) -> impl Iterator<Item = &[f64]> {
        self.data.chunks_exact(self.cols)
    }

    /// Copy `column[from_row ..]` into a fresh vector.
    ///
    /// Row-major storage makes a column strided, so the QR port works on a
    /// copy rather than on a view. That costs `O(m)` per Householder step
    /// against the `O(m n)` the step already does, and it keeps the algorithm
    /// readable against `gsl_matrix_subcolumn`.
    ///
    /// Returns an empty vector if the request is out of range.
    pub fn column_from(&self, col: usize, from_row: usize) -> Vec<f64> {
        if col >= self.cols || from_row >= self.rows {
            return Vec::new();
        }
        (from_row..self.rows).map(|i| self.get(i, col)).collect()
    }

    /// Write `values` back into `column[from_row ..]`, stopping when either
    /// runs out.
    pub fn set_column_from(&mut self, col: usize, from_row: usize, values: &[f64]) {
        for (i, &v) in (from_row..self.rows).zip(values.iter()) {
            self.set(i, col, v);
        }
    }

    /// Matrix-vector product `A x`.
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `x.len() != cols`.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::linalg::Matrix;
    /// // [[1, 2], [3, 4]] * [1, 1] = [3, 7]
    /// let a = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    /// assert_eq!(a.mul_vec(&[1.0, 1.0]).unwrap(), vec![3.0, 7.0]);
    /// ```
    pub fn mul_vec(&self, x: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.cols {
            return Err(PetirError::LengthMismatch {
                expected: self.cols,
                found: x.len(),
            });
        }
        let mut out = Vec::with_capacity(self.rows);
        for row in self.row_iter() {
            let mut acc = 0.0_f64;
            for (&a, &xi) in row.iter().zip(x.iter()) {
                acc += a * xi;
            }
            out.push(acc);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_and_element_access_round_trip() {
        let mut m = Matrix::zeros(2, 3).unwrap();
        assert_eq!((m.rows(), m.cols()), (2, 3));
        m.set(1, 2, 7.0);
        assert_eq!(m.get(1, 2), 7.0);
        assert_eq!(m.row(1), Some(&[0.0, 0.0, 7.0][..]));
    }

    /// A column index past `cols` must NOT wrap round into the next row.
    ///
    /// Row-major storage makes `(i, cols + k)` and `(i + 1, k)` the same flat
    /// offset, so a naive `i * cols + j` would read a real element and report
    /// it as being somewhere it is not. That is worse than the panic this
    /// crate is removing, because it is silent.
    #[test]
    fn an_out_of_range_column_does_not_alias_the_next_row() {
        let mut m = Matrix::zeros(2, 2).unwrap();
        m.set(1, 0, 5.0);
        assert_eq!(m.get(1, 0), 5.0);
        assert!(m.get(0, 2).is_nan(), "(0, 2) must not alias (1, 0)");
        m.set(0, 2, 99.0);
        assert_eq!(
            m.get(1, 0),
            5.0,
            "an out-of-range set must not touch (1, 0)"
        );
    }

    #[test]
    fn out_of_range_reads_nan_and_writes_are_discarded() {
        let mut m = Matrix::zeros(2, 2).unwrap();
        assert!(m.get(5, 0).is_nan());
        assert!(m.get(0, 5).is_nan());
        m.set(5, 0, 1.0);
        assert!(m.row_iter().all(|r| r.iter().all(|v| *v == 0.0)));
    }

    #[test]
    fn malformed_construction_is_reported() {
        assert!(matches!(Matrix::zeros(0, 3), Err(PetirError::Invalid)));
        assert!(matches!(Matrix::zeros(3, 0), Err(PetirError::Invalid)));
        assert!(matches!(
            Matrix::from_row_major(2, 2, vec![1.0]),
            Err(PetirError::LengthMismatch {
                expected: 4,
                found: 1
            })
        ));
    }

    #[test]
    fn column_extraction_and_write_back_round_trip() {
        let a = Matrix::from_row_major(3, 2, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        assert_eq!(a.column_from(0, 0), vec![1.0, 3.0, 5.0]);
        assert_eq!(a.column_from(1, 1), vec![4.0, 6.0]);
        assert!(a.column_from(9, 0).is_empty());

        let mut b = a.clone();
        b.set_column_from(0, 1, &[30.0, 50.0]);
        assert_eq!(b.column_from(0, 0), vec![1.0, 30.0, 50.0]);
    }

    #[test]
    fn matrix_vector_product_matches_the_hand_computed_value() {
        let a = Matrix::from_row_major(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        assert_eq!(a.mul_vec(&[1.0, 0.0, -1.0]).unwrap(), vec![-2.0, -2.0]);
        assert!(a.mul_vec(&[1.0]).is_err());
    }
}
