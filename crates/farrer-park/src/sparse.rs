// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! The finite-element sparse matrix: compressed sparse row, with element
//! scatter and gather.
//!
//! # What belongs in this module
//!
//! [`CsrMatrix`] — values on a fixed [`crate::dof::SparsityPattern`] — the
//! scatter operation assembly uses, the sparse matrix-vector product, and the
//! incomplete-LU preconditioner built on the same pattern.
//!
//! # What does NOT belong here
//!
//! Krylov solvers. Those are in `outram-foam-basic-lib`, shared with the
//! finite-volume side; this module supplies the
//! [`outram_foam_basic_lib::linear_operator::LinearOperator`] implementation
//! that lets them act on a finite-element matrix. See [`crate::operator`].
//!
//! # Why CSR and not `LduMatrix`
//!
//! `LduMatrix` stores one coefficient per mesh *face*, which is exactly right
//! when the discretisation is finite volume and every off-diagonal coupling is
//! a face flux. A finite-element stiffness matrix couples every pair of degrees
//! of freedom sharing an element — 8 by 8 for a Quad4 in plane strain, 24 by 24
//! for a Hex8 — and those couplings do not correspond to faces at all. Forcing
//! them into face addressing would mean inventing faces; CSR simply stores
//! them.
//!
//! # Units
//!
//! Stiffness entries are newton per metre; more generally, the units of the
//! assembled operator. Treated as bare `f64` here.

use crate::dof::SparsityPattern;
use crate::error::{FemError, Result};

/// A sparse matrix in compressed sparse row form, on a fixed sparsity pattern.
///
/// The pattern is set at construction and never changes: assembly writes into
/// existing slots and [`zero`](Self::zero) resets the values between Newton
/// iterations. That is what makes repeated assembly allocation-free.
///
/// # Units
///
/// Entries carry whatever units the assembling layer gave them (newton per
/// metre for a stiffness matrix).
#[derive(Debug, Clone, PartialEq)]
pub struct CsrMatrix {
    n_rows: usize,
    n_cols: usize,
    row_ptr: Vec<usize>,
    col_idx: Vec<usize>,
    values: Vec<f64>,
}

impl CsrMatrix {
    /// Allocate a zero-filled matrix on `pattern`.
    ///
    /// Square by construction: a finite-element stiffness matrix is.
    #[must_use]
    pub fn from_pattern(pattern: &SparsityPattern) -> Self {
        let n = pattern.n_rows();
        Self {
            n_rows: n,
            n_cols: n,
            row_ptr: pattern.row_ptr.clone(),
            col_idx: pattern.col_idx.clone(),
            values: vec![0.0; pattern.nnz()],
        }
    }

    /// Number of rows (dimensionless count).
    #[must_use]
    pub fn n_rows(&self) -> usize {
        self.n_rows
    }

    /// Number of columns (dimensionless count).
    #[must_use]
    pub fn n_cols(&self) -> usize {
        self.n_cols
    }

    /// Number of stored entries (dimensionless count).
    #[must_use]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Row offsets, length `n_rows + 1`.
    #[must_use]
    pub fn row_ptr(&self) -> &[usize] {
        &self.row_ptr
    }

    /// Column indices, sorted ascending within each row.
    #[must_use]
    pub fn col_idx(&self) -> &[usize] {
        &self.col_idx
    }

    /// The stored values, in the same order as [`col_idx`](Self::col_idx).
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Reset every stored value to zero, keeping the pattern and the
    /// allocation.
    pub fn zero(&mut self) {
        self.values.iter_mut().for_each(|v| *v = 0.0);
    }

    /// The storage slot of entry `(i, j)`, or `None` if it is not in the
    /// pattern.
    ///
    /// `O(log nnz_row)` by binary search, which is why
    /// [`SparsityPattern`] guarantees sorted rows.
    #[must_use]
    pub fn slot(&self, i: usize, j: usize) -> Option<usize> {
        let (lo, hi) = (self.row_ptr[i], self.row_ptr[i + 1]);
        self.col_idx[lo..hi].binary_search(&j).ok().map(|k| lo + k)
    }

    /// Read entry `(i, j)`; zero if it is not in the pattern.
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.slot(i, j).map_or(0.0, |s| self.values[s])
    }

    /// Accumulate `v` into entry `(i, j)`.
    ///
    /// # Errors
    ///
    /// [`FemError::BoundaryCondition`] if `(i, j)` is not in the pattern —
    /// which means the pattern and the element loop disagree, a structural bug
    /// rather than a numerical one.
    pub fn add(&mut self, i: usize, j: usize, v: f64) -> Result<()> {
        match self.slot(i, j) {
            Some(s) => {
                self.values[s] += v;
                Ok(())
            }
            None => Err(FemError::BoundaryCondition(format!(
                "entry ({i}, {j}) is outside the sparsity pattern"
            ))),
        }
    }

    /// Overwrite entry `(i, j)` with `v`, if it is in the pattern.
    ///
    /// Returns `true` if the entry existed and was written. Used by strong
    /// Dirichlet elimination, which needs to *replace* a row rather than
    /// accumulate into it.
    pub fn set(&mut self, i: usize, j: usize, v: f64) -> bool {
        match self.slot(i, j) {
            Some(s) => {
                self.values[s] = v;
                true
            }
            None => false,
        }
    }

    /// Scatter a dense element matrix into the global matrix.
    ///
    /// `ke` is row-major of side `dofs.len()`, in the element's own
    /// degree-of-freedom ordering, and `dofs` maps each local index to a global
    /// one (produced by [`crate::dof::DofMap::element_dofs`]).
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] if `ke` is not `dofs.len()` squared, and
    /// whatever [`add`](Self::add) returns for an entry outside the pattern.
    pub fn scatter(&mut self, dofs: &[usize], ke: &[f64]) -> Result<()> {
        let n = dofs.len();
        if ke.len() != n * n {
            return Err(FemError::LengthMismatch {
                context: "CsrMatrix::scatter: element matrix",
                expected: n * n,
                actual: ke.len(),
            });
        }
        for (a, &i) in dofs.iter().enumerate() {
            for (b, &j) in dofs.iter().enumerate() {
                let v = ke[a * n + b];
                if v != 0.0 {
                    self.add(i, j, v)?;
                }
            }
        }
        Ok(())
    }

    /// Gather the entries of one element's block out of the global matrix into
    /// `ke` (row-major, side `dofs.len()`).
    ///
    /// The inverse of [`scatter`](Self::scatter) as a *read*: it does not undo
    /// an assembly, since several elements contribute to the same entries. Used
    /// by tests that check a scatter landed where it should.
    pub fn gather(&self, dofs: &[usize], ke: &mut [f64]) {
        let n = dofs.len();
        for (a, &i) in dofs.iter().enumerate() {
            for (b, &j) in dofs.iter().enumerate() {
                ke[a * n + b] = self.get(i, j);
            }
        }
    }

    /// Sparse matrix-vector product `y = A x`.
    ///
    /// `x` has `n_cols` entries, `y` has `n_rows` and is fully overwritten.
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) {
        for i in 0..self.n_rows {
            let mut s = 0.0;
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                s += self.values[k] * x[self.col_idx[k]];
            }
            y[i] = s;
        }
    }

    /// The main diagonal, written into `d` (length `n_rows`).
    pub fn diagonal_into(&self, d: &mut [f64]) {
        for i in 0..self.n_rows {
            d[i] = self.get(i, i);
        }
    }

    /// Largest absolute entry on the diagonal — the scale a penalty stiffness
    /// is measured against.
    #[must_use]
    pub fn max_abs_diagonal(&self) -> f64 {
        (0..self.n_rows).fold(0.0_f64, |m, i| m.max(self.get(i, i).abs()))
    }

    /// Largest absolute deviation from symmetry, `max |A_ij - A_ji|`.
    ///
    /// A linear-elastic or consistently-linearised elasto-plastic stiffness
    /// matrix is symmetric; a non-trivial value here means the tangent is
    /// wrong, and it is checked by the assembly tests.
    #[must_use]
    pub fn max_asymmetry(&self) -> f64 {
        let mut m = 0.0_f64;
        for i in 0..self.n_rows {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                let j = self.col_idx[k];
                m = m.max((self.values[k] - self.get(j, i)).abs());
            }
        }
        m
    }
}

/// A preconditioner for [`CsrMatrix`], dispatched by enum (never a trait
/// object).
///
/// # Choosing one
///
/// | Variant | Setup cost | Strength | When |
/// |---|---|---|---|
/// | [`Identity`](CsrPreconditioner::Identity) | none | none | a baseline, or a very well conditioned system |
/// | [`Jacobi`](CsrPreconditioner::Jacobi) | `O(n)` | weak but unconditionally safe | a first cut, and the fallback when ILU(0) breaks down |
/// | [`Ilu0`](CsrPreconditioner::Ilu0) | `O(nnz * row)` | strong on an elasticity stiffness matrix | the default for the verification suite |
///
/// # Units
///
/// Entries carry the inverse units of the matrix; treated as bare `f64`.
#[derive(Debug, Clone)]
pub enum CsrPreconditioner {
    /// `M = I`.
    Identity,
    /// Reciprocal diagonal, `z = r / diag(A)`.
    Jacobi(Vec<f64>),
    /// Incomplete LU with zero fill-in: `L` and `U` share `A`'s pattern.
    ///
    /// Holds the combined factors in the matrix's own storage order, plus the
    /// slot index of each row's diagonal so the triangular solves need no
    /// search.
    Ilu0(Ilu0Csr),
}

/// The stored ILU(0) factorisation of a [`CsrMatrix`].
///
/// Both triangular factors live in one value array on the original sparsity
/// pattern: entries left of the diagonal are `L` (unit diagonal implied),
/// entries from the diagonal rightwards are `U`.
#[derive(Debug, Clone)]
pub struct Ilu0Csr {
    row_ptr: Vec<usize>,
    col_idx: Vec<usize>,
    lu: Vec<f64>,
    diag_slot: Vec<usize>,
}

impl CsrPreconditioner {
    /// Reciprocal-diagonal (Jacobi) preconditioner.
    ///
    /// A diagonal entry with magnitude below `1e-300` is treated as `1.0`, so
    /// this can never break down.
    #[must_use]
    pub fn jacobi(a: &CsrMatrix) -> Self {
        let mut d = vec![0.0; a.n_rows()];
        a.diagonal_into(&mut d);
        for v in d.iter_mut() {
            *v = if v.abs() > 1.0e-300 { 1.0 / *v } else { 1.0 };
        }
        CsrPreconditioner::Jacobi(d)
    }

    /// ILU(0) factorisation on `a`'s own sparsity pattern.
    ///
    /// # Algorithm
    ///
    /// The standard IKJ-ordered incomplete LU: for each row `i`, for each
    /// already-factorised column `k < i` present in the row, scale by the
    /// pivot and eliminate along row `k`, **dropping any fill outside the
    /// pattern**. A pivot whose magnitude falls below `1e-12` times the
    /// largest diagonal is floored at that value, which keeps the factor
    /// usable rather than producing infinities; the preconditioner is then
    /// weaker, never wrong, because the Krylov solver judges convergence on
    /// the true residual regardless.
    ///
    /// # Cost
    ///
    /// `O(sum over rows of nnz_row^2)` and one extra value array.
    #[must_use]
    pub fn ilu0(a: &CsrMatrix) -> Self {
        let n = a.n_rows();
        let row_ptr = a.row_ptr().to_vec();
        let col_idx = a.col_idx().to_vec();
        let mut lu = a.values().to_vec();

        // Slot of the diagonal in each row (the pattern always stores it).
        let mut diag_slot = vec![0usize; n];
        for i in 0..n {
            diag_slot[i] = (row_ptr[i]..row_ptr[i + 1])
                .find(|&k| col_idx[k] == i)
                .unwrap_or(row_ptr[i]);
        }
        let floor = 1.0e-12 * a.max_abs_diagonal().max(1.0e-300);

        // Scratch: column -> slot in the current row.
        let mut where_in_row = vec![usize::MAX; n];
        for i in 0..n {
            for k in row_ptr[i]..row_ptr[i + 1] {
                where_in_row[col_idx[k]] = k;
            }
            for k in row_ptr[i]..row_ptr[i + 1] {
                let col = col_idx[k];
                if col >= i {
                    break;
                }
                let pivot = lu[diag_slot[col]];
                let p = if pivot.abs() < floor {
                    floor.copysign(if pivot == 0.0 { 1.0 } else { pivot })
                } else {
                    pivot
                };
                let mult = lu[k] / p;
                lu[k] = mult;
                if mult == 0.0 {
                    continue;
                }
                for kk in row_ptr[col]..row_ptr[col + 1] {
                    let c2 = col_idx[kk];
                    if c2 <= col {
                        continue;
                    }
                    let slot = where_in_row[c2];
                    if slot != usize::MAX {
                        lu[slot] -= mult * lu[kk];
                    }
                }
            }
            for k in row_ptr[i]..row_ptr[i + 1] {
                where_in_row[col_idx[k]] = usize::MAX;
            }
            let d = lu[diag_slot[i]];
            if d.abs() < floor {
                lu[diag_slot[i]] = floor.copysign(if d == 0.0 { 1.0 } else { d });
            }
        }

        CsrPreconditioner::Ilu0(Ilu0Csr {
            row_ptr,
            col_idx,
            lu,
            diag_slot,
        })
    }

    /// Apply the preconditioner: write `z = M^{-1} r`.
    ///
    /// `r` and `z` have the system length; `z` is fully overwritten.
    pub fn apply_to(&self, r: &[f64], z: &mut [f64]) {
        match self {
            CsrPreconditioner::Identity => z.copy_from_slice(r),
            CsrPreconditioner::Jacobi(inv) => {
                for i in 0..z.len() {
                    z[i] = r[i] * inv[i];
                }
            }
            CsrPreconditioner::Ilu0(f) => {
                let n = z.len();
                // Forward solve L y = r (unit diagonal).
                for i in 0..n {
                    let mut s = r[i];
                    for k in f.row_ptr[i]..f.row_ptr[i + 1] {
                        let c = f.col_idx[k];
                        if c >= i {
                            break;
                        }
                        s -= f.lu[k] * z[c];
                    }
                    z[i] = s;
                }
                // Backward solve U z = y.
                for i in (0..n).rev() {
                    let mut s = z[i];
                    for k in f.row_ptr[i]..f.row_ptr[i + 1] {
                        let c = f.col_idx[k];
                        if c > i {
                            s -= f.lu[k] * z[c];
                        }
                    }
                    z[i] = s / f.lu[f.diag_slot[i]];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dof::{DofMap, SparsityPattern};
    use crate::mesh::unit_square_quad4;

    fn small_pattern(n: usize) -> SparsityPattern {
        // Dense n x n pattern.
        let mut row_ptr = vec![0];
        let mut col_idx = Vec::new();
        for _ in 0..n {
            for j in 0..n {
                col_idx.push(j);
            }
            row_ptr.push(col_idx.len());
        }
        SparsityPattern { row_ptr, col_idx }
    }

    /// Scatter then gather must round-trip on a single element.
    #[test]
    fn scatter_gather_round_trip() {
        let p = small_pattern(6);
        let mut a = CsrMatrix::from_pattern(&p);
        let dofs = [0usize, 2, 5];
        let ke = [1.0, 2.0, 3.0, 2.0, 4.0, 5.0, 3.0, 5.0, 6.0];
        a.scatter(&dofs, &ke).unwrap();
        let mut out = [0.0; 9];
        a.gather(&dofs, &mut out);
        assert_eq!(out.to_vec(), ke.to_vec());
        // Untouched entries stay zero.
        assert_eq!(a.get(1, 1), 0.0);
    }

    /// The matrix-vector product must agree with a dense reference.
    #[test]
    fn spmv_matches_dense() {
        let n = 5;
        let p = small_pattern(n);
        let mut a = CsrMatrix::from_pattern(&p);
        let mut dense = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let v = ((i * 7 + j * 3) % 11) as f64 - 4.0;
                dense[i * n + j] = v;
                a.add(i, j, v).unwrap();
            }
        }
        let x: Vec<f64> = (0..n).map(|i| 0.5 * i as f64 - 1.0).collect();
        let mut y = vec![0.0; n];
        a.spmv(&x, &mut y);
        for i in 0..n {
            let want: f64 = (0..n).map(|j| dense[i * n + j] * x[j]).sum();
            assert!((y[i] - want).abs() < 1e-13);
        }
    }

    /// On a dense pattern ILU(0) is a complete LU, so applying it must solve
    /// the system exactly. That is the strongest available check that the
    /// factorisation and the triangular solves agree.
    #[test]
    fn ilu0_on_a_dense_pattern_is_an_exact_solve() {
        let n = 6;
        let p = small_pattern(n);
        let mut a = CsrMatrix::from_pattern(&p);
        for i in 0..n {
            for j in 0..n {
                let v = if i == j {
                    10.0 + i as f64
                } else {
                    1.0 / (1.0 + (i as f64 - j as f64).abs())
                };
                a.add(i, j, v).unwrap();
            }
        }
        let x_true: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0).sqrt()).collect();
        let mut b = vec![0.0; n];
        a.spmv(&x_true, &mut b);

        let m = CsrPreconditioner::ilu0(&a);
        let mut x = vec![0.0; n];
        m.apply_to(&b, &mut x);
        for i in 0..n {
            assert!(
                (x[i] - x_true[i]).abs() < 1e-10,
                "ILU(0) exact solve: {} vs {}",
                x[i],
                x_true[i]
            );
        }
    }

    /// Jacobi must reproduce a diagonal inverse exactly.
    #[test]
    fn jacobi_inverts_the_diagonal() {
        let p = small_pattern(4);
        let mut a = CsrMatrix::from_pattern(&p);
        for i in 0..4 {
            a.add(i, i, 2.0 + i as f64).unwrap();
        }
        let m = CsrPreconditioner::jacobi(&a);
        let r = vec![1.0, 1.0, 1.0, 1.0];
        let mut z = vec![0.0; 4];
        m.apply_to(&r, &mut z);
        for i in 0..4 {
            assert!((z[i] - 1.0 / (2.0 + i as f64)).abs() < 1e-14);
        }
    }

    /// A mesh-derived pattern must accept every entry the element loop writes.
    #[test]
    fn mesh_pattern_accepts_every_element_entry() {
        let mesh = unit_square_quad4(3).unwrap();
        let dofs = DofMap::displacement(&mesh);
        let p = SparsityPattern::from_mesh(&mesh, &dofs);
        let mut a = CsrMatrix::from_pattern(&p);
        let ned = dofs.element_dof_count(mesh.element_type());
        let ke = vec![1.0; ned * ned];
        let mut ed = vec![0usize; ned];
        for e in 0..mesh.n_elements() {
            dofs.element_dofs(&mesh, crate::mesh::ElemId(e), &mut ed);
            a.scatter(&ed, &ke).expect("pattern must cover the element block");
        }
        assert!(a.max_abs_diagonal() > 0.0);
    }
}
