// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// `Matrix::mul` PORTS GSL 2.8's cblas/source_gemm_r.h (RowMajor,
// NoTrans/NoTrans branch, alpha = 1, beta = 0) and `Norm::One` PORTS
// matrix/prop_source.c:166 gsl_matrix_norm1 with cblas/source_asum_r.h;
// commit cf180cd7fbd06039a577f9c9ff0b428784765ac1, GPL-3.0-or-later,
// Copyright (C) 1996-2000, 2007 Gerard Jungman, Brian Gough. Everything else
// here delegates to an existing port or is definitional (see the module
// docs).

//! An Octave-style vocabulary on [`Matrix`] (gh:#993), so that code ported
//! from MATLAB or Octave reads like its source.
//!
//! # Lineage: a vocabulary, not new numerics
//!
//! PETIR rule 2 says numerics are ported, never written. This module obeys
//! it by doing one of three things in each function:
//!
//! - **delegating** to a routine already ported and verified against GSL
//!   (`inv`, `det`, `solve`, `norm(2)`);
//! - **porting** the GSL kernel for it (`mul` is `cblas_dgemm`'s loop,
//!   `norm(1)` is `gsl_matrix_norm1`, `norm(fro)` is the ported `dnrm2` run
//!   over every entry);
//! - being **definitional**, with no arithmetic beyond what the definition
//!   states and no choice of algorithm to get wrong: `eye`, `zeros`, `ones`,
//!   `diag` (placement only), `transpose` (copy only), `kron` (one product
//!   per entry, `a_ij * b_kl`), and `trace` (the diagonal summed in index
//!   order).
//!
//! # Octave to PETIR
//!
//! | Octave | PETIR | lineage |
//! |---|---|---|
//! | `eye(n)`, `eye(m, n)` | [`Matrix::eye`]`(n, n)`, `(m, n)` | definitional |
//! | `zeros(m, n)` | [`Matrix::zeros`] | definitional |
//! | `ones(m, n)` | [`Matrix::ones`] | definitional |
//! | `diag(v)` (vector to matrix) | [`Matrix::diag`] | definitional |
//! | `diag(A)` (matrix to vector) | [`Matrix::diag_vec`] | definitional (copy) |
//! | `kron(A, B)` | [`Matrix::kron`] | definitional |
//! | `A'`, `A.'`, `transpose(A)` | [`Matrix::transpose`] | definitional (copy) |
//! | `trace(A)` | [`Matrix::trace`] | definitional |
//! | `norm(A, 1)` | [`Matrix::norm`]`(Norm::One)` | port of `gsl_matrix_norm1` |
//! | `norm(A)`, `norm(A, 2)` | [`Matrix::norm`]`(Norm::Two)` | largest singular value, [`Svd`] |
//! | `norm(A, Inf)` | [`Matrix::norm`]`(Norm::Inf)` | `gsl_matrix_norm1` of `A'` |
//! | `norm(A, "fro")` | [`Matrix::norm`]`(Norm::Fro)` | ported `dnrm2` ([`blas1::nrm2`]) over all entries |
//! | `inv(A)` | [`Matrix::inv`] | [`lu::inverse`] |
//! | `det(A)` | [`Matrix::det`] | [`lu::det`] |
//! | `A \ b` | [`Matrix::solve`] | square: [`SquareMatrix::solve`] (LU); tall: [`QrDecomposition::least_squares`] |
//! | `A * B` | [`Matrix::mul`] | port of `cblas_dgemm` |
//! | `A * x` | [`Matrix::mul_vec`] | existing |
//!
//! # Where the answers differ from Octave's, and why
//!
//! Octave calls LAPACK; PETIR calls its GSL ports and the OpenFOAM lift.
//! The *algorithms* named above are the same family but not the same code,
//! so results agree to rounding, not bit for bit:
//!
//! - `det`, `inv` and square `\`: Octave uses LAPACK `dgetrf` (Doolittle,
//!   partial pivoting); PETIR uses the Crout factorisation with **scaled**
//!   partial pivoting lifted from OpenFOAM ([`SquareMatrix`]).
//! - tall `\`: Octave uses `dgelsd` (SVD, minimum-norm, so it also solves a
//!   rank-deficient system); PETIR uses Householder QR and **refuses** a
//!   rank-deficient system with [`PetirError::Singular`].
//! - wide `\` (`m < n`): Octave returns the minimum-norm solution; PETIR
//!   returns [`PetirError::Unimplemented`].
//! - `norm(A, "fro")`: Octave's own scaled accumulator; PETIR's `dnrm2`.
//!   Both avoid overflow; they may differ in the last bit.
//!
//! [`Svd`]: crate::linalg::Svd
//! [`blas1::nrm2`]: crate::linalg::blas1::nrm2
//! [`lu::inverse`]: crate::linalg::lu::inverse
//! [`lu::det`]: crate::linalg::lu::det
//! [`SquareMatrix`]: crate::linalg::SquareMatrix
//! [`SquareMatrix::solve`]: crate::linalg::SquareMatrix::solve
//! [`QrDecomposition::least_squares`]: crate::linalg::QrDecomposition::least_squares

use alloc::vec::Vec;

use crate::linalg::square_matrix::MatrixError;
use crate::linalg::{blas1, lu, Matrix, QrDecomposition, SquareMatrix, Svd};
use crate::{PetirError, Result};

/// Which matrix norm, Octave's `p` argument to `norm(A, p)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Norm {
    /// `norm(A, 1)`: the largest column sum of absolute values.
    One,
    /// `norm(A)` or `norm(A, 2)`: the largest singular value.
    Two,
    /// `norm(A, Inf)`: the largest row sum of absolute values.
    Inf,
    /// `norm(A, "fro")`: the square root of the sum of squared entries.
    Fro,
}

impl Matrix {
    /// Octave `eye(m, n)`: ones on the main diagonal.
    ///
    /// # Errors
    ///
    /// As [`Matrix::zeros`].
    pub fn eye(rows: usize, cols: usize) -> Result<Self> {
        let mut m = Self::zeros(rows, cols)?;
        for i in 0..rows.min(cols) {
            m.set(i, i, 1.0);
        }
        Ok(m)
    }

    /// Octave `ones(m, n)`.
    ///
    /// # Errors
    ///
    /// As [`Matrix::zeros`].
    pub fn ones(rows: usize, cols: usize) -> Result<Self> {
        let mut m = Self::zeros(rows, cols)?;
        for i in 0..rows {
            for j in 0..cols {
                m.set(i, j, 1.0);
            }
        }
        Ok(m)
    }

    /// Octave `diag(v)`: the square matrix with `v` on its diagonal.
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] for an empty `v`.
    pub fn diag(v: &[f64]) -> Result<Self> {
        let n = v.len();
        let mut m = Self::zeros(n, n)?;
        for (i, &x) in v.iter().enumerate() {
            m.set(i, i, x);
        }
        Ok(m)
    }

    /// Octave `diag(A)`: the main diagonal, `min(m, n)` entries.
    pub fn diag_vec(&self) -> Vec<f64> {
        (0..self.rows().min(self.cols()))
            .map(|i| self.get(i, i))
            .collect()
    }

    /// Octave `A'` (real, so also `A.'` and `transpose(A)`).
    pub fn transpose(&self) -> Self {
        let (m, n) = (self.rows(), self.cols());
        let mut t = self.clone();
        // reshape: a fresh n x m with the same length cannot fail
        if let Ok(fresh) = Self::zeros(n, m) {
            t = fresh;
        }
        for i in 0..m {
            for j in 0..n {
                t.set(j, i, self.get(i, j));
            }
        }
        t
    }

    /// Octave `kron(A, B)`: the `(m p) x (n q)` block matrix whose `(i, j)`
    /// block is `a_ij B`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] if the product dimensions overflow.
    pub fn kron(&self, b: &Matrix) -> Result<Self> {
        let (m, n) = (self.rows(), self.cols());
        let (p, q) = (b.rows(), b.cols());
        let (Some(rows), Some(cols)) = (m.checked_mul(p), n.checked_mul(q)) else {
            return Err(PetirError::Invalid);
        };
        let mut out = Self::zeros(rows, cols)?;
        for i in 0..m {
            for j in 0..n {
                let aij = self.get(i, j);
                for k in 0..p {
                    for l in 0..q {
                        out.set(i * p + k, j * q + l, aij * b.get(k, l));
                    }
                }
            }
        }
        Ok(out)
    }

    /// Octave `trace(A)`: the diagonal summed in index order.
    ///
    /// # Errors
    ///
    /// [`PetirError::NotSquare`] — Octave refuses a non-square matrix too.
    pub fn trace(&self) -> Result<f64> {
        self.require_square()?;
        let mut s = 0.0_f64;
        for i in 0..self.rows() {
            s += self.get(i, i);
        }
        Ok(s)
    }

    /// Octave `norm(A, p)`; see [`Norm`] and the module table for how each
    /// is computed.
    ///
    /// # Errors
    ///
    /// For [`Norm::Two`], whatever [`Svd::new`](crate::linalg::Svd::new)
    /// returns (a wide matrix is decomposed through its transpose, which has
    /// the same singular values).
    pub fn norm(&self, p: Norm) -> Result<f64> {
        match p {
            Norm::One => Ok(norm1(self)),
            Norm::Inf => Ok(norm1(&self.transpose())),
            Norm::Fro => Ok(blas1::nrm2(self.as_slice())),
            Norm::Two => {
                let svd = if self.rows() >= self.cols() {
                    Svd::new(self)?
                } else {
                    Svd::new(&self.transpose())?
                };
                Ok(svd.s().first().copied().unwrap_or(0.0))
            }
        }
    }

    /// Octave `inv(A)`, through [`lu::inverse`](crate::linalg::lu::inverse).
    ///
    /// # Errors
    ///
    /// [`PetirError::NotSquare`], or [`PetirError::Singular`] from the LU.
    pub fn inv(&self) -> Result<Self> {
        let s = self.to_square()?;
        Self::from_square(&lu::inverse(&s)?)
    }

    /// Octave `det(A)`, through [`lu::det`](crate::linalg::lu::det).
    ///
    /// # Errors
    ///
    /// [`PetirError::NotSquare`], or [`PetirError::Singular`] from the LU
    /// (a singular matrix is an error, not `0.0`; see `lu`'s docs).
    pub fn det(&self) -> Result<f64> {
        lu::det(&self.to_square()?)
    }

    /// Octave `A \ b`: LU solve for a square `A`, Householder least squares
    /// for a tall one. See the module docs for how this differs from Octave.
    ///
    /// # Errors
    ///
    /// - [`PetirError::LengthMismatch`] if `b.len() != rows`.
    /// - [`PetirError::Singular`] for a singular square or rank-deficient
    ///   tall `A`.
    /// - [`PetirError::Unimplemented`] for a wide `A`.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::linalg::Matrix;
    /// let a = Matrix::from_row_major(2, 2, vec![2.0, 1.0, 1.0, 3.0]).unwrap();
    /// let x = a.solve(&[3.0, 5.0]).unwrap(); // x = A \ b
    /// assert!((x[0] - 0.8).abs() < 1e-15 && (x[1] - 1.4).abs() < 1e-15);
    /// ```
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>> {
        let (m, n) = (self.rows(), self.cols());
        if b.len() != m {
            return Err(PetirError::LengthMismatch {
                expected: m,
                found: b.len(),
            });
        }
        if m == n {
            self.to_square()?.solve(b).map_err(|e| match e {
                MatrixError::Singular { col } => PetirError::Singular { col },
            })
        } else if m > n {
            Ok(QrDecomposition::new(self.clone())?
                .least_squares(b)?
                .solution)
        } else {
            Err(PetirError::Unimplemented)
        }
    }

    /// Octave `A * B`, `cblas_dgemm(RowMajor, NoTrans, NoTrans, alpha = 1,
    /// beta = 0)` (`source_gemm_r.h`): `C` zeroed, then for each `k` and row
    /// `i`, `temp = A[i][k]` and, **only if `temp != 0`**, `C[i][j] += temp *
    /// B[k][j]`. The skip is upstream's, and means an infinite or NaN entry
    /// of `B` meeting an exact zero of `A` does not propagate.
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `self.cols() != b.rows()`.
    pub fn mul(&self, b: &Matrix) -> Result<Self> {
        let (m, kk, n) = (self.rows(), self.cols(), b.cols());
        if kk != b.rows() {
            return Err(PetirError::LengthMismatch {
                expected: kk,
                found: b.rows(),
            });
        }
        let mut c = Self::zeros(m, n)?;
        for k in 0..kk {
            for i in 0..m {
                let temp = 1.0 * self.get(i, k);
                if temp != 0.0 {
                    for j in 0..n {
                        c.set(i, j, c.get(i, j) + temp * b.get(k, j));
                    }
                }
            }
        }
        Ok(c)
    }

    /// Copy into the square type the LU routines take.
    ///
    /// # Errors
    ///
    /// [`PetirError::NotSquare`].
    pub fn to_square(&self) -> Result<SquareMatrix> {
        self.require_square()?;
        let n = self.rows();
        let mut s = SquareMatrix::new(n);
        for i in 0..n {
            for j in 0..n {
                s.set(i, j, self.get(i, j));
            }
        }
        Ok(s)
    }

    /// Copy a [`SquareMatrix`] into a `Matrix`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] for an order-0 matrix, which `Matrix` cannot
    /// represent.
    pub fn from_square(s: &SquareMatrix) -> Result<Self> {
        let n = s.n();
        let mut m = Self::zeros(n, n)?;
        for i in 0..n {
            for j in 0..n {
                m.set(i, j, s.get(i, j));
            }
        }
        Ok(m)
    }

    fn require_square(&self) -> Result<()> {
        if self.rows() == self.cols() {
            Ok(())
        } else {
            Err(PetirError::NotSquare {
                rows: self.rows(),
                cols: self.cols(),
            })
        }
    }
}

/// `gsl_matrix_norm1` (`matrix/prop_source.c:166`): the largest column
/// `dasum` (`r += fabs(x)` in index order).
fn norm1(a: &Matrix) -> f64 {
    let mut value = 0.0_f64;
    for j in 0..a.cols() {
        let mut sum = 0.0_f64;
        for i in 0..a.rows() {
            sum += a.get(i, j).abs();
        }
        if sum > value {
            value = sum;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn a23() -> Matrix {
        Matrix::from_row_major(2, 3, vec![1.0, -2.0, 3.0, 4.0, 5.0, -6.0]).unwrap()
    }

    #[test]
    fn constructors() {
        let e = Matrix::eye(2, 3).unwrap();
        assert_eq!(e.as_slice(), &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        assert!(Matrix::ones(2, 2)
            .unwrap()
            .as_slice()
            .iter()
            .all(|x| *x == 1.0));
        let d = Matrix::diag(&[2.0, 3.0]).unwrap();
        assert_eq!(d.as_slice(), &[2.0, 0.0, 0.0, 3.0]);
        assert_eq!(a23().diag_vec(), vec![1.0, 5.0]);
        assert!(Matrix::diag(&[]).is_err());
    }

    #[test]
    fn transpose_kron_trace() {
        let t = a23().transpose();
        assert_eq!((t.rows(), t.cols()), (3, 2));
        assert_eq!(t.get(2, 1), -6.0);
        let k = Matrix::eye(2, 2)
            .unwrap()
            .kron(&Matrix::diag(&[3.0, 4.0]).unwrap())
            .unwrap();
        assert_eq!(k.diag_vec(), vec![3.0, 4.0, 3.0, 4.0]);
        assert_eq!(k.get(0, 2), 0.0);
        assert_eq!(
            Matrix::diag(&[1.0, 2.0, 4.0]).unwrap().trace().unwrap(),
            7.0
        );
        assert!(a23().trace().is_err());
    }

    #[test]
    fn norms() {
        let a = a23();
        assert_eq!(a.norm(Norm::One).unwrap(), 9.0);
        assert_eq!(a.norm(Norm::Inf).unwrap(), 15.0);
        assert!((a.norm(Norm::Fro).unwrap() - 91.0_f64.sqrt()).abs() < 1e-14);
        // the 2-norm of a wide matrix goes through its transpose
        let s = Svd::new(&a.transpose()).unwrap().s()[0];
        assert_eq!(a.norm(Norm::Two).unwrap(), s);
        let d = Matrix::diag(&[3.0, -7.0]).unwrap();
        assert!((d.norm(Norm::Two).unwrap() - 7.0).abs() < 1e-15);
    }

    #[test]
    fn inv_det_solve_mul() {
        let a = Matrix::from_row_major(2, 2, vec![4.0, 7.0, 2.0, 6.0]).unwrap();
        assert!((a.det().unwrap() - 10.0).abs() < 1e-13);
        let ai = a.inv().unwrap();
        let p = a.mul(&ai).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((p.get(i, j) - e).abs() < 1e-14);
            }
        }
        let x = a.solve(&[1.0, 2.0]).unwrap();
        assert_eq!(a.mul_vec(&x).unwrap().len(), 2);
        // tall: least squares through QR
        let tall = Matrix::from_row_major(3, 2, vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        let ls = tall.solve(&[1.0, 2.0, 3.0]).unwrap();
        assert!((ls[0] - 1.0).abs() < 1e-14 && (ls[1] - 2.0).abs() < 1e-14);
        // errors
        assert!(matches!(
            a23().solve(&[1.0, 2.0]),
            Err(PetirError::Unimplemented)
        ));
        assert!(a.solve(&[1.0]).is_err());
        let sing = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 2.0, 4.0]).unwrap();
        assert!(matches!(
            sing.solve(&[1.0, 1.0]),
            Err(PetirError::Singular { .. })
        ));
        assert!(a23().det().is_err());
        assert!(a.mul(&a23().transpose()).is_err());
        // the dgemm zero skip: 0 * inf does not reach C
        let z = Matrix::from_row_major(1, 2, vec![0.0, 1.0]).unwrap();
        let b = Matrix::from_row_major(2, 1, vec![f64::INFINITY, 2.0]).unwrap();
        assert_eq!(z.mul(&b).unwrap().get(0, 0), 2.0);
        // round trip through SquareMatrix
        assert_eq!(Matrix::from_square(&a.to_square().unwrap()).unwrap(), a);
        assert!(Matrix::from_square(&SquareMatrix::new(0)).is_err());
    }
}
