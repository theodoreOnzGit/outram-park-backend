// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// MOVED 2026-10-17 from `outram-mc-libs/src/depletion/matrix.rs` (gh:#950,
// maintainer direction "All math moved to petir"). The storage, `zeros`,
// `order`, `get`, `mat_vec` and `as_slice` arithmetic is unchanged; the
// panicking index checks became `Result`s (PETIR's no-panic rule), which is
// recorded per method below. `lincomb` is new here: it is the dense form of
// the scipy-sparse arithmetic in OpenMC `openmc/deplete/_matrix_funcs.py`
// (MIT, see `super`'s provenance block).

//! Dense depletion (burnup) matrix `A` with units of inverse seconds.
//!
//! # What this represents
//!
//! The nuclide-inventory evolution under combined radioactive decay and
//! neutron-induced transmutation is the linear ODE system
//!
//! ```text
//!     dN/dt = A N,          N(t) = exp(A t) N(0)
//! ```
//!
//! where `N` is the vector of nuclide number densities and `A` is the
//! **burnup matrix** (also called the transmutation or Bateman matrix). This
//! is the object OpenMC's `Chain.form_matrix` assembles
//! (`openmc/deplete/chain.py`) before handing it to a matrix-exponential
//! solver; see [`super::cram`] for the solver that consumes it.
//!
//! # Sign / index convention (read before using `set`/`add`)
//!
//! `A` is indexed `A[row][col]`, and the ODE is
//! `dN[row]/dt = sum_col A[row][col] * N[col]` — upstream's "`A[j, i]`
//! describing rates at which isotope `i` transmutes to isotope `j`"
//! (`openmc/deplete/cram.py:70-72`):
//!
//! * **Off-diagonal** `A[i][j]` (`i != j`) is the **production** rate
//!   coefficient of nuclide `i` from nuclide `j`, `1/s`, `>= 0`.
//! * **Diagonal** `A[j][j]` is the **total removal** rate coefficient of
//!   nuclide `j`, `1/s`, `<= 0`.
//!
//! # Dense, not sparse — **DEVIATION**
//!
//! **DEVIATION:** upstream stores `A` as a `scipy.sparse.csc_array`. This is a
//! dense row-major store, as it was in `outram-mc-libs` before the move. The
//! values of every entry are the same; what differs is only that a
//! structurally absent entry is an explicit `0.0` here. Arithmetic on a
//! stored zero adds or multiplies an exact zero, which changes no non-zero
//! result (the sign of an exact zero can differ, and nothing downstream reads
//! it). The reason is `no_std` and size: the chains this has served are tens
//! of nuclides, where a dense store is simplest. A full ENDF chain (~3800
//! nuclides) would want the sparse form; that is not ported.

use alloc::vec;
use alloc::vec::Vec;

use crate::{PetirError, Result};

/// A dense `n x n` depletion (burnup) matrix `A` in units of inverse seconds,
/// defining the linear system `dN/dt = A N`.
///
/// Stored row-major (`A[row][col] == data[row * n + col]`). See the module
/// docs for the sign/index convention and for why it is dense.
#[derive(Debug, Clone, PartialEq)]
pub struct DepletionMatrix {
    n: usize,
    /// Row-major coefficients, length `n * n`, units `1/s`.
    data: Vec<f64>,
}

impl DepletionMatrix {
    /// A zero `order x order` matrix (no decay, no transmutation).
    pub fn zeros(order: usize) -> Self {
        Self {
            n: order,
            data: vec![0.0; order.saturating_mul(order)],
        }
    }

    /// The matrix order `n` — the number of tracked nuclides.
    pub fn order(&self) -> usize {
        self.n
    }

    /// Row-major offset of `(row, col)`, or `None` out of range.
    #[inline]
    fn offset(&self, row: usize, col: usize) -> Option<usize> {
        if row < self.n && col < self.n {
            row.checked_mul(self.n)?.checked_add(col)
        } else {
            None
        }
    }

    /// The coefficient `A[row][col]` in `1/s`.
    ///
    /// **Out of range returns `NaN`**, never panics — the convention of
    /// [`crate::linalg::SquareMatrix::get`]. Before the move this panicked;
    /// `NaN` poisons every later operation and compares unequal to
    /// everything, so the defect still surfaces. Use [`Self::try_get`] for a
    /// matchable error.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.try_get(row, col).unwrap_or(f64::NAN)
    }

    /// The coefficient `A[row][col]`, or [`PetirError::Domain`] out of range.
    #[inline]
    pub fn try_get(&self, row: usize, col: usize) -> Result<f64> {
        self.offset(row, col)
            .and_then(|i| self.data.get(i).copied())
            .ok_or(PetirError::Domain)
    }

    /// Overwrite the coefficient `A[row][col]` (`1/s`).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `row` or `col` is out of range; the matrix
    /// is left unchanged. (Before the move this panicked.)
    #[inline]
    pub fn set(&mut self, row: usize, col: usize, val: f64) -> Result<()> {
        let i = self.offset(row, col).ok_or(PetirError::Domain)?;
        let slot = self.data.get_mut(i).ok_or(PetirError::Domain)?;
        *slot = val;
        Ok(())
    }

    /// Accumulate `val` (`1/s`) into `A[row][col]`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `row` or `col` is out of range; the matrix
    /// is left unchanged. (Before the move this panicked.)
    #[inline]
    pub fn add(&mut self, row: usize, col: usize, val: f64) -> Result<()> {
        let i = self.offset(row, col).ok_or(PetirError::Domain)?;
        let slot = self.data.get_mut(i).ok_or(PetirError::Domain)?;
        *slot += val;
        Ok(())
    }

    /// The right-hand side `dN/dt = A N` for a number-density vector `n_vec`.
    ///
    /// Accumulates each row left to right from `0.0`, exactly as before the
    /// move.
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `n_vec.len() != order()` (before the
    /// move this panicked).
    pub fn mat_vec(&self, n_vec: &[f64]) -> Result<Vec<f64>> {
        if n_vec.len() != self.n {
            return Err(PetirError::LengthMismatch {
                expected: self.n,
                found: n_vec.len(),
            });
        }
        let mut out = Vec::with_capacity(self.n);
        if self.n == 0 {
            return Ok(out);
        }
        for row in self.data.chunks_exact(self.n) {
            let mut acc = 0.0;
            for (a, x) in row.iter().zip(n_vec) {
                acc += a * x;
            }
            out.push(acc);
        }
        Ok(out)
    }

    /// Read-only view of the row-major coefficient buffer (`1/s`), length `n*n`.
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Mutable view of the row-major coefficient buffer, length `n*n`.
    ///
    /// For a caller assembling the matrix in bulk (an operator's
    /// `form_matrix`); the convention in the module docs still applies.
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// The scaled matrix `c * A`, entry by entry — `c * M` on a
    /// `scipy.sparse.csc_array` (it scales `M.data`).
    pub fn scaled(&self, c: f64) -> DepletionMatrix {
        DepletionMatrix {
            n: self.n,
            data: self.data.iter().map(|v| c * v).collect(),
        }
    }
}

/// One term of a linear combination of burnup matrices, and whether it is
/// added or subtracted — the binary operator that precedes it in upstream's
/// expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// `... + c * M`
    Plus,
    /// `... - c * M`
    Minus,
}

/// `c0*M0 (+|-) c1*M1 (+|-) ...`, evaluated **left to right, entry by entry**,
/// exactly as Python evaluates the expressions in OpenMC
/// `openmc/deplete/_matrix_funcs.py:4-80` on `scipy.sparse` operands.
///
/// For each entry the first term is `c0 * m0`; every later term is then
/// added or subtracted in the order written: `((c0*m0 + c1*m1) + c2*m2) -
/// c3*m3`. Keeping the subtraction as a subtraction (rather than adding a
/// negated coefficient) is not needed for the bits — `a - c*m` and
/// `a + (-c)*m` round identically — but it keeps each call site readable
/// against the line it ports.
///
/// # Errors
///
/// [`PetirError::Invalid`] for an empty list, and
/// [`PetirError::LengthMismatch`] if the matrices differ in order.
pub fn lincomb(first: (f64, &DepletionMatrix), rest: &[(Sign, f64, &DepletionMatrix)]) -> Result<DepletionMatrix> {
    let (c0, m0) = first;
    let mut out = m0.scaled(c0);
    for &(sign, c, m) in rest {
        if m.n != out.n {
            return Err(PetirError::LengthMismatch {
                expected: out.n,
                found: m.n,
            });
        }
        for (o, v) in out.data.iter_mut().zip(&m.data) {
            match sign {
                Sign::Plus => *o = *o + c * v,
                Sign::Minus => *o = *o - c * v,
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_range_is_reported_not_panicked() {
        let mut m = DepletionMatrix::zeros(2);
        assert_eq!(m.set(2, 0, 1.0), Err(PetirError::Domain));
        assert_eq!(m.add(0, 5, 1.0), Err(PetirError::Domain));
        assert!(m.get(3, 3).is_nan());
        assert_eq!(m.try_get(3, 0), Err(PetirError::Domain));
        assert_eq!(
            m.mat_vec(&[1.0]),
            Err(PetirError::LengthMismatch { expected: 2, found: 1 })
        );
        m.set(1, 0, 2.0).unwrap();
        m.add(1, 0, 0.5).unwrap();
        assert_eq!(m.get(1, 0), 2.5);
        assert_eq!(m.mat_vec(&[2.0, 7.0]).unwrap(), vec![0.0, 5.0]);
        assert_eq!(DepletionMatrix::zeros(0).mat_vec(&[]).unwrap(), Vec::<f64>::new());
        m.as_mut_slice()[0] = -1.0;
        assert_eq!(m.as_slice()[0], -1.0);
    }

    /// `lincomb` is left-to-right and refuses mismatched orders.
    #[test]
    fn lincomb_is_left_to_right() {
        let mut a = DepletionMatrix::zeros(1);
        a.set(0, 0, 3.0).unwrap();
        let mut b = DepletionMatrix::zeros(1);
        b.set(0, 0, 0.1).unwrap();
        let m = lincomb((1.0 / 4.0, &a), &[(Sign::Plus, 1.0 / 6.0, &b), (Sign::Minus, 1.0 / 12.0, &a)]).unwrap();
        let want: f64 = ((1.0 / 4.0) * 3.0 + (1.0 / 6.0) * 0.1) - (1.0 / 12.0) * 3.0;
        assert_eq!(m.get(0, 0).to_bits(), want.to_bits());
        let c = DepletionMatrix::zeros(2);
        assert!(lincomb((1.0, &a), &[(Sign::Plus, 1.0, &c)]).is_err());
    }
}
