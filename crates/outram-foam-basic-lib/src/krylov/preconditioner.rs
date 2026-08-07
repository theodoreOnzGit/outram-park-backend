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

//! Preconditioners for the asymmetric Krylov solvers.
//!
//! A preconditioner supplies an approximate inverse `M^{-1} ≈ A^{-1}` that is
//! cheap to apply. Applying it to a residual `r` produces a "preconditioned
//! residual" `z = M^{-1} r`; a good `M` clusters the eigenvalues of `A M^{-1}`
//! (right) or `M^{-1} A` (left) and so cuts the Krylov iteration count.
//!
//! Dispatch is by the [`Preconditioner`] enum (defined in the parent module),
//! never by trait objects, per the workspace design rules. This file provides
//! the two concrete data-carrying variants:
//!
//! - [`JacobiPreconditioner`] — diagonal (reciprocal-diagonal) scaling. Rock-solid
//!   and always applicable; the guaranteed fallback.
//! - [`Ilu0Preconditioner`] — incomplete LU with zero fill-in, ILU(0). A genuine
//!   incomplete factorisation on a CSR view of `A` built internally from the LDU
//!   coefficients, with a small-pivot guard so it never divides by (near-)zero.
//!
//! All quantities are dimensionless `f64`: the preconditioner acts on the raw
//! algebraic residual, not on a `uom`-typed physical field.

use crate::ldu_matrix::LduMatrix;

/// Smallest pivot magnitude the ILU(0) factorisation will divide by.
///
/// If a computed pivot falls below this in magnitude it is replaced (preserving
/// sign) so that the triangular solves stay finite. This trades a small amount of
/// factorisation accuracy for guaranteed robustness — the solver still converges,
/// just with a slightly weaker preconditioner on that row.
const ILU_PIVOT_FLOOR: f64 = 1.0e-300;

/// Diagonal (Jacobi) preconditioner: `M = diag(A)`, so `M^{-1} r = r / diag`.
///
/// The cheapest possible preconditioner. It is always applicable and cannot break
/// down: any exactly-zero diagonal entry is treated as `1.0` (identity on that
/// row) so the reciprocal is finite. Stores the precomputed reciprocal diagonal,
/// length `n_cells`, each entry dimensionless.
pub struct JacobiPreconditioner {
    /// `inv_diag[i] = 1 / diag[i]`, with a zero diagonal mapped to `1.0`.
    inv_diag: Vec<f64>,
}

impl JacobiPreconditioner {
    /// Build the reciprocal diagonal from `a`.
    ///
    /// A diagonal entry that is exactly `0.0` (or non-finite) is replaced by a
    /// unit reciprocal so `apply` never produces `inf`/`NaN`.
    pub fn new(a: &LduMatrix) -> Self {
        let inv_diag = a
            .diag
            .iter()
            .map(|&d| {
                if d != 0.0 && d.is_finite() {
                    1.0 / d
                } else {
                    1.0
                }
            })
            .collect();
        Self { inv_diag }
    }

    /// Apply `z = M^{-1} r` — elementwise `z_i = r_i / diag_i`.
    ///
    /// `r` and `z` must both have length `n_cells`.
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        debug_assert_eq!(r.len(), self.inv_diag.len());
        debug_assert_eq!(z.len(), self.inv_diag.len());
        for i in 0..z.len() {
            z[i] = r[i] * self.inv_diag[i];
        }
    }
}

/// ILU(0) preconditioner: incomplete LU factorisation with **no fill-in**.
///
/// The factors `L` (unit lower-triangular) and `U` (upper-triangular) are computed
/// with the standard IKJ Gaussian-elimination algorithm but every update that
/// would land outside the sparsity pattern of `A` is discarded — so `L\U` occupy
/// exactly the same nonzero positions as `A`. Applying `M^{-1} = (LU)^{-1}` is one
/// forward substitution (`L y = r`) followed by one back substitution (`U z = y`).
///
/// # This is a genuine ILU(0), not a Jacobi fallback
///
/// The factorisation is real: for a symmetric problem it reduces to an incomplete
/// Cholesky-like factor and typically beats Jacobi substantially; for a
/// nonsymmetric problem it captures both off-diagonal directions. The only
/// safeguard is a small-pivot floor (`ILU_PIVOT_FLOOR`): a pivot smaller than
/// that in magnitude is bumped up (keeping its sign) so the triangular solves stay
/// finite. In the degenerate limit where *every* off-diagonal contribution
/// vanishes, ILU(0) coincides exactly with Jacobi — that is a property of the
/// method, not a substitution of one for the other.
///
/// # Storage (CSR, built internally from the LDU coefficients)
///
/// The LDU (face-addressed) matrix is expanded once, at construction, into a
/// compressed-sparse-row (CSR) view: for each row `i` the diagonal `diag[i]`, plus
/// `upper[f]` at column `neighbour[f]` for every face owned by `i`, plus
/// `lower[f]` at column `owner[f]` for every face neighboured by `i`. Columns
/// within a row are stored in ascending order. The factorisation overwrites those
/// values with the combined `L\U` (unit `L` diagonal is implicit and not stored).
pub struct Ilu0Preconditioner {
    /// Number of rows / unknowns.
    n: usize,
    /// CSR row offsets, length `n + 1`.
    row_ptr: Vec<usize>,
    /// CSR column indices, ascending within each row.
    col_idx: Vec<usize>,
    /// CSR values, overwritten with the `L\U` factors.
    lu: Vec<f64>,
    /// For each row, the index into `col_idx`/`lu` of its diagonal entry.
    diag_ptr: Vec<usize>,
}

impl Ilu0Preconditioner {
    /// Build the ILU(0) factorisation of `a`.
    ///
    /// Constructs the CSR view then runs the in-place IKJ incomplete-LU
    /// elimination restricted to existing nonzeros. Runs in roughly
    /// `O(nnz · avg_row)` time. Pivots below `ILU_PIVOT_FLOOR` are floored to
    /// keep the factor invertible.
    pub fn new(a: &LduMatrix) -> Self {
        let n = a.n_cells;

        // --- Count nonzeros per row: 1 diagonal + off-diagonals from faces. ---
        let mut row_count = vec![1usize; n]; // diagonal always present
        for f in 0..a.n_internal_faces {
            let o = a.owner[f];
            let nb = a.neighbour[f];
            // upper[f] contributes to row o (column nb); lower[f] to row nb (column o).
            row_count[o] += 1;
            row_count[nb] += 1;
        }

        // --- Row offsets (prefix sum). ---
        let mut row_ptr = vec![0usize; n + 1];
        for i in 0..n {
            row_ptr[i + 1] = row_ptr[i] + row_count[i];
        }
        let nnz = row_ptr[n];

        // --- Scatter (column, value) pairs into each row, then sort per row. ---
        // Temporarily collect per-row entries, then flatten in ascending column
        // order so binary search on a row is valid.
        let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for i in 0..n {
            rows[i].push((i, a.diag[i])); // diagonal
        }
        for f in 0..a.n_internal_faces {
            let o = a.owner[f];
            let nb = a.neighbour[f];
            rows[o].push((nb, a.upper[f]));
            rows[nb].push((o, a.lower[f]));
        }

        let mut col_idx = Vec::with_capacity(nnz);
        let mut lu = Vec::with_capacity(nnz);
        let mut diag_ptr = vec![0usize; n];
        for i in 0..n {
            rows[i].sort_by_key(|&(c, _)| c);
            for &(c, v) in &rows[i] {
                if c == i {
                    diag_ptr[i] = col_idx.len();
                }
                col_idx.push(c);
                lu.push(v);
            }
        }

        let mut me = Self {
            n,
            row_ptr,
            col_idx,
            lu,
            diag_ptr,
        };
        me.factor();
        me
    }

    /// Locate the value at `(row, col)` within the CSR structure, if present.
    ///
    /// Binary search over the ascending column indices of `row`. Returns the flat
    /// index into `lu`/`col_idx`, or `None` if `col` is not a stored nonzero.
    fn find(&self, row: usize, col: usize) -> Option<usize> {
        let lo = self.row_ptr[row];
        let hi = self.row_ptr[row + 1];
        let slice = &self.col_idx[lo..hi];
        match slice.binary_search(&col) {
            Ok(rel) => Some(lo + rel),
            Err(_) => None,
        }
    }

    /// In-place IKJ incomplete-LU elimination restricted to the sparsity pattern.
    ///
    /// After this call `lu` holds the combined factors: strictly-lower entries are
    /// the multipliers of unit-lower `L`, the diagonal and strictly-upper entries
    /// are `U`.
    fn factor(&mut self) {
        for i in 0..self.n {
            let ri = self.row_ptr[i];
            let ri_end = self.row_ptr[i + 1];
            // Walk the entries of row i in ascending column order.
            for p in ri..ri_end {
                let k = self.col_idx[p];
                if k >= i {
                    break; // reached the diagonal / upper part: L updates done
                }
                // a_ik := a_ik / a_kk   (multiplier for row k)
                let mut akk = self.lu[self.diag_ptr[k]];
                if akk.abs() < ILU_PIVOT_FLOOR {
                    akk = if akk < 0.0 {
                        -ILU_PIVOT_FLOOR
                    } else {
                        ILU_PIVOT_FLOOR
                    };
                }
                let aik = self.lu[p] / akk;
                self.lu[p] = aik;
                // For every j > k in row i, subtract a_ik * a_kj (if a_kj exists).
                for q in (p + 1)..ri_end {
                    let j = self.col_idx[q];
                    if let Some(kj) = self.find(k, j) {
                        let akj = self.lu[kj];
                        self.lu[q] -= aik * akj;
                    }
                }
            }
            // Floor the diagonal pivot for the eventual back-substitution divide.
            let dptr = self.diag_ptr[i];
            if self.lu[dptr].abs() < ILU_PIVOT_FLOOR {
                self.lu[dptr] = if self.lu[dptr] < 0.0 {
                    -ILU_PIVOT_FLOOR
                } else {
                    ILU_PIVOT_FLOOR
                };
            }
        }
    }

    /// Apply `z = M^{-1} r = U^{-1} L^{-1} r` via forward then back substitution.
    ///
    /// `L` is unit lower-triangular (implicit unit diagonal), `U` upper-triangular
    /// with the stored diagonal. `r` and `z` must both have length `n`.
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        debug_assert_eq!(r.len(), self.n);
        debug_assert_eq!(z.len(), self.n);

        // Forward solve L y = r (store y in z). Unit diagonal on L.
        for i in 0..self.n {
            let mut sum = r[i];
            let ri = self.row_ptr[i];
            let ri_end = self.row_ptr[i + 1];
            for p in ri..ri_end {
                let j = self.col_idx[p];
                if j >= i {
                    break; // L part only (strictly lower)
                }
                sum -= self.lu[p] * z[j];
            }
            z[i] = sum;
        }

        // Back solve U z = y (in place on z). Divide by stored diagonal.
        for i in (0..self.n).rev() {
            let mut sum = z[i];
            let ri_end = self.row_ptr[i + 1];
            let dptr = self.diag_ptr[i];
            for p in (dptr + 1)..ri_end {
                let j = self.col_idx[p];
                sum -= self.lu[p] * z[j];
            }
            z[i] = sum / self.lu[dptr];
        }
    }
}

// --- Dispatch surface -------------------------------------------------------
//
// The `Preconditioner` enum itself lives in the parent module (`mod.rs`) so it
// sits alongside `KrylovSettings`/`KrylovResult`, but its `apply` delegates to
// the free helpers below to keep those variant-specific routines next to the
// data they act on.

/// Apply a [`JacobiPreconditioner`]: `z = r / diag`.
pub(crate) fn jacobi_apply(p: &JacobiPreconditioner, r: &[f64], z: &mut [f64]) {
    p.apply(r, z);
}

/// Apply an [`Ilu0Preconditioner`]: `z = (LU)^{-1} r`.
pub(crate) fn ilu0_apply(p: &Ilu0Preconditioner, r: &[f64], z: &mut [f64]) {
    p.apply(r, z);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldu_matrix::LduMatrix;

    /// Build a symmetric tridiagonal LduMatrix with diag `d` and off-diag `e`.
    fn tridiag(n: usize, d: f64, e: f64) -> LduMatrix {
        let mut owner = Vec::new();
        let mut neigh = Vec::new();
        for i in 0..n - 1 {
            owner.push(i);
            neigh.push(i + 1);
        }
        let mut a = LduMatrix::new(n, owner, neigh);
        for i in 0..n {
            a.diag[i] = d;
        }
        for f in 0..a.n_internal_faces {
            a.lower[f] = e;
            a.upper[f] = e;
        }
        a
    }

    #[test]
    fn jacobi_reciprocal_diag() {
        let a = tridiag(4, 2.0, -1.0);
        let p = JacobiPreconditioner::new(&a);
        let r = [2.0, 4.0, 6.0, 8.0];
        let mut z = [0.0; 4];
        p.apply(&r, &mut z);
        // z = r / 2
        assert_eq!(z, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn jacobi_zero_diag_guard() {
        let mut a = tridiag(2, 0.0, -1.0);
        a.diag[0] = 0.0;
        let p = JacobiPreconditioner::new(&a);
        let r = [5.0, 3.0];
        let mut z = [0.0; 2];
        p.apply(&r, &mut z);
        assert_eq!(z[0], 5.0); // zero diag -> identity on that row
    }

    #[test]
    fn ilu0_exact_for_tridiagonal() {
        // ILU(0) is an EXACT LU factorisation for a tridiagonal matrix (no fill
        // is ever discarded), so M^{-1} r must equal A^{-1} r to round-off.
        let n = 5;
        let a = tridiag(n, 4.0, -1.0);
        let ilu = Ilu0Preconditioner::new(&a);

        // Pick a known x, form b = A x, then check ILU-apply(b) == x.
        let x_true: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0) * 0.5).collect();
        let b = a.multiply(&x_true);
        let mut z = vec![0.0; n];
        ilu.apply(&b, &mut z);
        for i in 0..n {
            assert!(
                (z[i] - x_true[i]).abs() < 1e-10,
                "row {i}: {} vs {}",
                z[i],
                x_true[i]
            );
        }
    }

    #[test]
    fn ilu0_nonsymmetric_finite() {
        // Nonsymmetric: lower != upper. Just require a finite, sane apply.
        let mut a = tridiag(4, 5.0, 0.0);
        for f in 0..a.n_internal_faces {
            a.lower[f] = -1.0;
            a.upper[f] = -2.0;
        }
        let ilu = Ilu0Preconditioner::new(&a);
        let r = [1.0, 2.0, 3.0, 4.0];
        let mut z = [0.0; 4];
        ilu.apply(&r, &mut z);
        assert!(z.iter().all(|v| v.is_finite()));
    }
}
