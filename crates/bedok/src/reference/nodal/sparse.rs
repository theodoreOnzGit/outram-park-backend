//! Sparse and small-dense linear algebra for the reference nodal solvers.
//!
//! # Provenance
//!
//! Original author of the BEDOK MATLAB implementation this crate translates:
//! **Than Yan Ren**, Singapore Nuclear Research and Safety Institute (SNRSI).
//! Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`. Translated under the
//! permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! This particular file is **not** a translation of one `.m` file. It supplies
//! the MATLAB built-ins the ported nodal files lean on, in the order they are
//! used across `makegradDxyz.m`, `makesigmadfxyz.m`, `calc_bucklingxyz.m`,
//! `calc_transleakagexyz.m`, `calc_a1_expansionxyz.m`,
//! `calc_a1234_expansionxyz.m`, `calc_sanodalxyz.m`,
//! `sanodaldiffusion_solverxyz.m` and `diffusion_solverxyz.m`:
//!
//! | MATLAB | Here |
//! |---|---|
//! | `sparse(i,j,v,m,n)` | [`SparseMatrix::from_triplets`] |
//! | `A*x` | [`SparseMatrix::mul_vec`] |
//! | `A+B`, `A-B` | [`SparseMatrix::add`], [`SparseMatrix::sub`] |
//! | `spdiags(d,0,n,n)*A` | [`SparseMatrix::scale_rows`] |
//! | `speye(n)` | [`SparseMatrix::identity`] |
//! | `A(i,j)` on a sparse `A` | [`SparseMatrix::get`] |
//! | `decomposition(A)` then `dA\b` | [`SparseLu`] (faer's direct sparse LU) |
//! | `A\b` on a small dense `A`, `pagemldivide` | [`solve_dense_in_place`] |
//!
//! # Deviations from MATLAB, recorded rather than hidden
//!
//! - **Explicit zeros are kept.** MATLAB's `sparse()` discards numerically zero
//!   entries; [`SparseMatrix::from_triplets`] stores them. No computed value
//!   changes, but the stored sparsity pattern is a superset of MATLAB's, which
//!   can shift the pivots the LU picks and hence the last few digits of a
//!   solve. Recorded, not repaired: dropping zeros would make some assembled
//!   operators structurally singular that MATLAB happens to survive.
//! - **The direct solver is faer's sparse LU with partial pivoting**, not
//!   UMFPACK. Same algorithm class, different pivot order, so results agree to
//!   solver tolerance rather than bit for bit.
//!
//! Nothing here is verified against Yan Ren's implementation — see the crate
//! docs on verification status.

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};

/// A real sparse matrix in compressed-sparse-column form.
///
/// Holds no physical quantity of its own — it is whatever operator the caller
/// assembled (a leakage operator in cm⁻¹, a cross-section operator in cm⁻¹, a
/// dimensionless buckling operator). Sizes are node/group state-vector lengths,
/// so a square operator is `state_len` × `state_len` in the sense of
/// [`Grid::state_len`](crate::reference::grid::Grid::state_len).
///
/// Row indices within a column are stored ascending, and duplicate
/// `(row, col)` triplets are summed at construction, matching MATLAB's
/// `sparse(i,j,v,m,n)`.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseMatrix {
    nrows: usize,
    ncols: usize,
    /// `col_ptr[j] .. col_ptr[j+1]` is the entry range of column `j`.
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    values: Vec<f64>,
}

impl SparseMatrix {
    /// Assembles an `nrows` × `ncols` matrix from `(row, col, value)` triplets,
    /// summing duplicates.
    ///
    /// This is MATLAB's `sparse(i,j,v,m,n)` with 0-based indices. Entries whose
    /// value is exactly zero are **kept** (see the module-level note).
    ///
    /// # Panics
    ///
    /// If any triplet index is outside the declared shape. MATLAB would raise
    /// `Index exceeds matrix dimensions`; an out-of-range triplet always means
    /// the index arithmetic upstream is wrong, which is precisely the failure
    /// this port most needs to be loud about.
    #[must_use]
    pub fn from_triplets(nrows: usize, ncols: usize, triplets: &[(usize, usize, f64)]) -> Self {
        for &(r, c, _) in triplets {
            assert!(
                r < nrows && c < ncols,
                "triplet ({r},{c}) outside {nrows}x{ncols}"
            );
        }

        // A *stable* sort, so duplicates are summed in the order the caller
        // supplied them. Floating-point addition is not associative, and
        // reproducibility across runs matters more here than sort speed.
        let mut order: Vec<usize> = (0..triplets.len()).collect();
        order.sort_by_key(|&k| (triplets[k].1, triplets[k].0));

        let mut col_ptr = vec![0usize; ncols + 1];
        let mut row_idx: Vec<usize> = Vec::with_capacity(triplets.len());
        let mut values: Vec<f64> = Vec::with_capacity(triplets.len());

        let mut k = 0usize;
        let mut col = 0usize;
        while k < order.len() {
            let (r, c, v) = triplets[order[k]];
            let mut acc = v;
            let mut k2 = k + 1;
            while k2 < order.len() {
                let (r2, c2, v2) = triplets[order[k2]];
                if r2 == r && c2 == c {
                    acc += v2;
                    k2 += 1;
                } else {
                    break;
                }
            }
            while col <= c {
                col_ptr[col] = row_idx.len();
                col += 1;
            }
            row_idx.push(r);
            values.push(acc);
            k = k2;
        }
        while col <= ncols {
            col_ptr[col] = row_idx.len();
            col += 1;
        }

        Self {
            nrows,
            ncols,
            col_ptr,
            row_idx,
            values,
        }
    }

    /// The `n` × `n` identity scaled by `factor` — MATLAB `factor*speye(n)`.
    #[must_use]
    pub fn identity(n: usize, factor: f64) -> Self {
        let triplets: Vec<(usize, usize, f64)> = (0..n).map(|i| (i, i, factor)).collect();
        Self::from_triplets(n, n, &triplets)
    }

    /// Number of rows.
    #[must_use]
    pub const fn nrows(&self) -> usize {
        self.nrows
    }

    /// Number of columns.
    #[must_use]
    pub const fn ncols(&self) -> usize {
        self.ncols
    }

    /// Number of stored entries, including any explicit zeros.
    #[must_use]
    pub fn stored_entries(&self) -> usize {
        self.values.len()
    }

    /// Entry `(row, col)`, or `0.0` if not stored — MATLAB `A(row,col)`.
    ///
    /// Binary search within the column, so `O(log nnz_col)`.
    ///
    /// # Panics
    ///
    /// If `row` or `col` is outside the matrix.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        assert!(row < self.nrows && col < self.ncols, "({row},{col}) OOB");
        let lo = self.col_ptr[col];
        let hi = self.col_ptr[col + 1];
        match self.row_idx[lo..hi].binary_search(&row) {
            Ok(k) => self.values[lo + k],
            Err(_) => 0.0,
        }
    }

    /// The main diagonal as a dense vector — MATLAB `full(diag(A))`.
    ///
    /// Length `min(nrows, ncols)`.
    #[must_use]
    pub fn diagonal(&self) -> Vec<f64> {
        (0..self.nrows.min(self.ncols))
            .map(|i| self.get(i, i))
            .collect()
    }

    /// `A * x` — MATLAB `A*x`.
    ///
    /// # Panics
    ///
    /// If `x.len() != ncols`. MATLAB raises a dimension-mismatch error here,
    /// and there is at least one call site in the reference where that can
    /// genuinely happen (see [`crate::reference::nodal`] on `Nc > 0`).
    #[must_use]
    // Column-major CSC traversal: the loop variable is the column index into
    // both `col_ptr` and `x`, so an iterator form would need a zip of unequal
    // things.
    #[allow(clippy::needless_range_loop)]
    pub fn mul_vec(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(
            x.len(),
            self.ncols,
            "matrix-vector dimension mismatch: {} columns, vector of {}",
            self.ncols,
            x.len()
        );
        let mut y = vec![0.0; self.nrows];
        for col in 0..self.ncols {
            let xc = x[col];
            for k in self.col_ptr[col]..self.col_ptr[col + 1] {
                y[self.row_idx[k]] += self.values[k] * xc;
            }
        }
        y
    }

    /// `A + B` — MATLAB `A+B`.
    ///
    /// # Panics
    ///
    /// If the shapes differ, mirroring MATLAB's error.
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        self.combine(other, 1.0)
    }

    /// `A - B` — MATLAB `A-B`.
    ///
    /// # Panics
    ///
    /// If the shapes differ, mirroring MATLAB's error.
    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        self.combine(other, -1.0)
    }

    fn combine(&self, other: &Self, sign: f64) -> Self {
        assert!(
            self.nrows == other.nrows && self.ncols == other.ncols,
            "sparse shape mismatch: {}x{} vs {}x{}",
            self.nrows,
            self.ncols,
            other.nrows,
            other.ncols
        );
        let mut col_ptr = vec![0usize; self.ncols + 1];
        let mut row_idx = Vec::with_capacity(self.values.len() + other.values.len());
        let mut values = Vec::with_capacity(self.values.len() + other.values.len());
        for col in 0..self.ncols {
            let (mut a, ae) = (self.col_ptr[col], self.col_ptr[col + 1]);
            let (mut b, be) = (other.col_ptr[col], other.col_ptr[col + 1]);
            while a < ae || b < be {
                let take_a = b >= be || (a < ae && self.row_idx[a] <= other.row_idx[b]);
                let take_b = a >= ae || (b < be && other.row_idx[b] <= self.row_idx[a]);
                let row = if take_a {
                    self.row_idx[a]
                } else {
                    other.row_idx[b]
                };
                let mut v = 0.0;
                if take_a {
                    v += self.values[a];
                    a += 1;
                }
                if take_b {
                    v += sign * other.values[b];
                    b += 1;
                }
                row_idx.push(row);
                values.push(v);
            }
            col_ptr[col + 1] = row_idx.len();
        }
        Self {
            nrows: self.nrows,
            ncols: self.ncols,
            col_ptr,
            row_idx,
            values,
        }
    }

    /// Scales row `i` by `d[i]` — MATLAB `spdiags(d,0,n,n)*A`.
    ///
    /// # Panics
    ///
    /// If `d.len() != nrows`.
    #[must_use]
    pub fn scale_rows(&self, d: &[f64]) -> Self {
        assert_eq!(d.len(), self.nrows, "row-scaling length mismatch");
        let mut out = self.clone();
        for (k, v) in out.values.iter_mut().enumerate() {
            *v *= d[self.row_idx[k]];
        }
        out
    }

    /// Converts to faer's CSC type, for factorisation.
    fn to_faer(&self) -> SparseColMat<usize, f64> {
        let mut triplets = Vec::with_capacity(self.values.len());
        for col in 0..self.ncols {
            for k in self.col_ptr[col]..self.col_ptr[col + 1] {
                triplets.push(Triplet::new(self.row_idx[k], col, self.values[k]));
            }
        }
        SparseColMat::try_new_from_triplets(self.nrows, self.ncols, &triplets)
            .expect("valid CSC triplets")
    }

    /// Factorises with a **direct** sparse LU — MATLAB `decomposition(A)`.
    ///
    /// The factorisation is computed once and reused for every subsequent
    /// right-hand side, exactly as MATLAB's `decomposition` object is. This is
    /// deliberately *not* an iterative solve: the reference uses `\`, and
    /// substituting a Krylov method here would be a stage-2 change (see
    /// `docs/bedok-port-scoping.md` §5).
    ///
    /// # Errors
    ///
    /// Returns `None` if the matrix is structurally singular or factorisation
    /// runs out of memory. MATLAB warns and returns `Inf`/`NaN` instead; the
    /// caller decides what to do.
    #[must_use]
    pub fn lu(&self) -> Option<SparseLu> {
        self.to_faer().sp_lu().ok().map(|inner| SparseLu {
            inner,
            n: self.ncols,
        })
    }
}

/// A cached direct sparse LU factorisation — MATLAB's `decomposition` object.
///
/// Holds no physical quantity; it solves `A x = b` for whatever operator `A`
/// was factorised.
pub struct SparseLu {
    inner: faer::sparse::linalg::solvers::Lu<usize, f64>,
    n: usize,
}

impl core::fmt::Debug for SparseLu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SparseLu").field("n", &self.n).finish()
    }
}

impl SparseLu {
    /// Solves `A x = b` — MATLAB `dA\b`.
    ///
    /// # Panics
    ///
    /// If `rhs.len()` does not match the factorised matrix order.
    #[must_use]
    pub fn solve(&self, rhs: &[f64]) -> Vec<f64> {
        assert_eq!(rhs.len(), self.n, "rhs length mismatch");
        let mut x = faer::Mat::<f64>::from_fn(self.n, 1, |i, _| rhs[i]);
        self.inner.solve_in_place(x.as_mut());
        (0..self.n).map(|i| x[(i, 0)]).collect()
    }
}

/// Solves the small dense system `a * x = b` in place — MATLAB `A\b` for a
/// square dense `A`, and one page of `pagemldivide`.
///
/// `a` is `n` × `n` in **row-major** order; `b` is the length-`n` right-hand
/// side and receives the solution.
///
/// Gaussian elimination with partial pivoting, the same algorithm MATLAB's
/// `mldivide` selects for a general square dense matrix.
///
/// # Singular systems
///
/// A zero pivot is **not** treated as an error: the elimination proceeds and
/// produces `Inf`/`NaN`, which is what MATLAB does (with a warning) and what
/// the reference relies on propagating. `calc_a1_expansionxyz.m` reaches this
/// case for a reflective outer face over a node with zero diffusion
/// coefficient — see [`super::first_moment`].
///
/// # Panics
///
/// If `a.len() != n*n` or `b.len() != n`.
pub fn solve_dense_in_place(a: &mut [f64], n: usize, b: &mut [f64]) {
    assert_eq!(a.len(), n * n, "dense matrix must be n*n");
    assert_eq!(b.len(), n, "rhs must be length n");
    for k in 0..n {
        // partial pivot
        let mut piv = k;
        let mut best = a[k * n + k].abs();
        for i in (k + 1)..n {
            let v = a[i * n + k].abs();
            if v > best {
                best = v;
                piv = i;
            }
        }
        if piv != k {
            for j in 0..n {
                a.swap(k * n + j, piv * n + j);
            }
            b.swap(k, piv);
        }
        let pivot = a[k * n + k];
        for i in (k + 1)..n {
            let f = a[i * n + k] / pivot;
            if f == 0.0 {
                a[i * n + k] = 0.0;
                continue;
            }
            for j in k..n {
                a[i * n + j] -= f * a[k * n + j];
            }
            b[i] -= f * b[k];
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in (k + 1)..n {
            s -= a[k * n + j] * b[j];
        }
        b[k] = s / a[k * n + k];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triplets_sum_duplicates_like_matlab_sparse() {
        let a = SparseMatrix::from_triplets(2, 2, &[(0, 0, 1.0), (0, 0, 2.5), (1, 1, -1.0)]);
        assert_eq!(a.get(0, 0), 3.5);
        assert_eq!(a.get(1, 1), -1.0);
        assert_eq!(a.get(0, 1), 0.0);
        assert_eq!(a.stored_entries(), 2);
    }

    #[test]
    fn explicit_zeros_are_retained_unlike_matlab() {
        let a = SparseMatrix::from_triplets(2, 2, &[(0, 0, 0.0)]);
        assert_eq!(a.stored_entries(), 1);
        assert_eq!(a.get(0, 0), 0.0);
    }

    #[test]
    fn matvec_matches_hand_computation() {
        // [[1 2],[0 3]] * [4,5] = [14,15]
        let a = SparseMatrix::from_triplets(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 1, 3.0)]);
        assert_eq!(a.mul_vec(&[4.0, 5.0]), vec![14.0, 15.0]);
    }

    #[test]
    fn add_and_sub_merge_disjoint_and_overlapping_patterns() {
        let a = SparseMatrix::from_triplets(2, 2, &[(0, 0, 1.0), (1, 1, 2.0)]);
        let b = SparseMatrix::from_triplets(2, 2, &[(0, 1, 3.0), (1, 1, 4.0)]);
        let s = a.add(&b);
        assert_eq!(s.get(0, 0), 1.0);
        assert_eq!(s.get(0, 1), 3.0);
        assert_eq!(s.get(1, 1), 6.0);
        let d = a.sub(&b);
        assert_eq!(d.get(0, 1), -3.0);
        assert_eq!(d.get(1, 1), -2.0);
    }

    #[test]
    fn scale_rows_is_left_multiplication_by_a_diagonal() {
        let a = SparseMatrix::from_triplets(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 1, 3.0)]);
        let s = a.scale_rows(&[10.0, 100.0]);
        assert_eq!(s.get(0, 0), 10.0);
        assert_eq!(s.get(0, 1), 20.0);
        assert_eq!(s.get(1, 1), 300.0);
    }

    #[test]
    fn diagonal_reads_the_main_diagonal() {
        let a = SparseMatrix::from_triplets(3, 3, &[(0, 0, 7.0), (2, 2, -1.0), (1, 0, 5.0)]);
        assert_eq!(a.diagonal(), vec![7.0, 0.0, -1.0]);
    }

    #[test]
    fn sparse_lu_solves_a_small_system() {
        // [[4 1],[1 3]] x = [1, 2] -> x = [1/11, 7/11]
        let a = SparseMatrix::from_triplets(
            2,
            2,
            &[(0, 0, 4.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)],
        );
        let lu = a.lu().expect("nonsingular");
        let x = lu.solve(&[1.0, 2.0]);
        assert!((x[0] - 1.0 / 11.0).abs() < 1e-12);
        assert!((x[1] - 7.0 / 11.0).abs() < 1e-12);
    }

    #[test]
    fn dense_solve_handles_pivoting() {
        // [[0 1],[1 0]] x = [2,3] -> x = [3,2]; needs a row swap.
        let mut a = vec![0.0, 1.0, 1.0, 0.0];
        let mut b = vec![2.0, 3.0];
        solve_dense_in_place(&mut a, 2, &mut b);
        assert_eq!(b, vec![3.0, 2.0]);
    }

    #[test]
    fn dense_solve_of_a_singular_system_yields_non_finite_like_matlab() {
        let mut a = vec![0.0, 0.0, 0.0, 0.0];
        let mut b = vec![1.0, 1.0];
        solve_dense_in_place(&mut a, 2, &mut b);
        assert!(b.iter().all(|v| !v.is_finite()));
    }
}
