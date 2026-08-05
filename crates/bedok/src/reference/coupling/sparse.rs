//! Sparse linear algebra used by the coupled drivers.
//!
//! # Provenance
//!
//! Support code for the translation of Than Yan Ren's (SNRSI) BEDOK MATLAB
//! snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`). It has no MATLAB
//! counterpart of its own: it supplies the operations MATLAB provides as
//! built-in syntax — sparse `+`/`-`, `A*x`, `spdiags`, `\` and
//! `decomposition()` — so that the ported drivers read the same way the
//! original does.
//!
//! # What the MATLAB operators mean here
//!
//! | MATLAB | Rust |
//! |---|---|
//! | `A*x` (sparse × dense vector) | [`spmv`] |
//! | `A+B`, `A-B`, `c*A` | [`linear_combination`] |
//! | `A + spdiags(d,0,n,n)` | [`add_diagonal`] |
//! | `A*spdiags(d,0,n,n)` | [`scale_columns`] |
//! | `decomposition(A)` then `A\b` | [`SparseLu`] |
//! | `A\b` (one-shot) | [`SparseLu::factorise`] then [`SparseLu::solve`] |
//!
//! **`\` is a direct sparse LU here, deliberately.** MATLAB's backslash on a
//! sparse unsymmetric matrix runs UMFPACK — a direct factorisation, not an
//! iterative solve — and `decomposition(A)` caches that factorisation for
//! reuse across right-hand sides. Substituting a Krylov method would change
//! the answers by its own tolerance and is out of scope for the reference
//! translation (`docs/bedok-port-scoping.md` §1, stage 1).

use faer::prelude::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::Mat;

use super::error::{CouplingError, Result};

/// A sparse operator over the state vector, in compressed-sparse-column form.
///
/// All BEDOK operators (`gradD`, the nodal correction, `sigma.tot`, `sigma.s`,
/// `sigma.f`, `sigma.fp`) are square and of side `philenf` — the full state
/// length including any extra components, in units of inverse centimetres
/// times centimetres cubed as the discretised balance equation leaves them.
pub type SparseMatrix = SparseColMat<usize, f64>;

/// Matrix-vector product `a * x`, the MATLAB `A*x`.
///
/// # Panics
///
/// If `x.len()` differs from the column count of `a`. A length mismatch means
/// the state-vector convention has been broken somewhere upstream, which is
/// exactly the failure mode `grid.rs` exists to prevent, so it is loud.
#[must_use]
pub fn spmv(a: &SparseMatrix, x: &[f64]) -> Vec<f64> {
    assert_eq!(
        a.ncols(),
        x.len(),
        "matrix has {} columns but the vector has {} entries",
        a.ncols(),
        x.len()
    );
    let mut y = vec![0.0_f64; a.nrows()];
    let col_ptr = a.col_ptr();
    let row_idx = a.row_idx();
    let val = a.val();
    for j in 0..a.ncols() {
        let xj = x[j];
        if xj == 0.0 {
            continue;
        }
        for k in col_ptr[j]..col_ptr[j + 1] {
            y[row_idx[k]] += val[k] * xj;
        }
    }
    y
}

/// Linear combination `sum_i coefficient_i * matrix_i`, the MATLAB `A+B-C`.
///
/// Structurally-zero entries stay absent; duplicate entries are summed. Every
/// term must have the same shape.
///
/// # Errors
///
/// [`CouplingError::SparseAssembly`] if the term list is empty, if the terms
/// disagree in shape, or if assembly fails.
pub fn linear_combination(terms: &[(f64, &SparseMatrix)]) -> Result<SparseMatrix> {
    let Some((_, first)) = terms.first() else {
        return Err(CouplingError::SparseAssembly {
            reason: "linear_combination called with no terms".to_string(),
        });
    };
    let (nrows, ncols) = (first.nrows(), first.ncols());
    let mut triplets: Vec<Triplet<usize, usize, f64>> = Vec::new();
    for (coefficient, matrix) in terms {
        if matrix.nrows() != nrows || matrix.ncols() != ncols {
            return Err(CouplingError::SparseAssembly {
                reason: format!(
                    "shape mismatch: {}x{} vs {}x{}",
                    matrix.nrows(),
                    matrix.ncols(),
                    nrows,
                    ncols
                ),
            });
        }
        let col_ptr = matrix.col_ptr();
        let row_idx = matrix.row_idx();
        let val = matrix.val();
        for j in 0..ncols {
            for k in col_ptr[j]..col_ptr[j + 1] {
                triplets.push(Triplet::new(row_idx[k], j, coefficient * val[k]));
            }
        }
    }
    from_triplets(nrows, ncols, &triplets)
}

/// `a + spdiags(d, 0, n, n)` — add a diagonal to a square operator.
///
/// Used for the time-derivative term `spdiags(invv*(omega+1/dt),0,…) + M` of
/// the transient flux solve. `d` carries units of inverse velocity per second,
/// i.e. inverse centimetres, matching the removal terms already in `a`.
///
/// # Errors
///
/// [`CouplingError::SparseAssembly`] if `a` is not square or `d` has the wrong
/// length.
pub fn add_diagonal(a: &SparseMatrix, d: &[f64]) -> Result<SparseMatrix> {
    if a.nrows() != a.ncols() || d.len() != a.nrows() {
        return Err(CouplingError::SparseAssembly {
            reason: format!(
                "add_diagonal: matrix {}x{}, diagonal length {}",
                a.nrows(),
                a.ncols(),
                d.len()
            ),
        });
    }
    let diagonal = diagonal_matrix(d)?;
    linear_combination(&[(1.0, a), (1.0, &diagonal)])
}

/// `a * spdiags(d, 0, n, n)` — scale column `j` of `a` by `d[j]`.
///
/// This is the "delayed production of the new flux moves into the system
/// matrix" step of the exponential-transform scheme: right-multiplying the
/// fission operator by a diagonal scales each column, i.e. each source node's
/// contribution, by its own precursor weight.
///
/// # Errors
///
/// [`CouplingError::SparseAssembly`] if `d.len()` differs from the column count.
pub fn scale_columns(a: &SparseMatrix, d: &[f64]) -> Result<SparseMatrix> {
    if d.len() != a.ncols() {
        return Err(CouplingError::SparseAssembly {
            reason: format!(
                "scale_columns: matrix has {} columns, scale vector {}",
                a.ncols(),
                d.len()
            ),
        });
    }
    let col_ptr = a.col_ptr();
    let row_idx = a.row_idx();
    let val = a.val();
    let mut triplets: Vec<Triplet<usize, usize, f64>> = Vec::with_capacity(val.len());
    for j in 0..a.ncols() {
        for k in col_ptr[j]..col_ptr[j + 1] {
            triplets.push(Triplet::new(row_idx[k], j, val[k] * d[j]));
        }
    }
    from_triplets(a.nrows(), a.ncols(), &triplets)
}

/// A square diagonal operator, the MATLAB `spdiags(d, 0, n, n)`.
///
/// # Errors
///
/// [`CouplingError::SparseAssembly`] if assembly fails.
pub fn diagonal_matrix(d: &[f64]) -> Result<SparseMatrix> {
    let triplets: Vec<Triplet<usize, usize, f64>> = d
        .iter()
        .enumerate()
        .map(|(i, &v)| Triplet::new(i, i, v))
        .collect();
    from_triplets(d.len(), d.len(), &triplets)
}

/// Assemble a sparse matrix from triplets, summing duplicates.
///
/// # Errors
///
/// [`CouplingError::SparseAssembly`] if `faer` rejects the triplet list (index
/// overflow or allocation failure).
pub fn from_triplets(
    nrows: usize,
    ncols: usize,
    triplets: &[Triplet<usize, usize, f64>],
) -> Result<SparseMatrix> {
    SparseColMat::try_new_from_triplets(nrows, ncols, triplets).map_err(|e| {
        CouplingError::SparseAssembly {
            reason: format!("{e:?}"),
        }
    })
}

/// A cached sparse LU factorisation — the MATLAB `decomposition(A)`.
///
/// Holding the factorisation and calling [`solve`](Self::solve) repeatedly
/// reproduces MATLAB's `dM = decomposition(M); x = dM\b;` exactly: one
/// factorisation, many triangular solves. Results are identical to a fresh
/// `M\b` per right-hand side, so the reuse is purely a cost saving.
///
/// # Note on pivoting
///
/// `faer`'s sparse LU uses partial (row) pivoting, as UMFPACK does. Fill-in
/// ordering differs between the two libraries, so the rounding of the solve is
/// not bit-identical to MATLAB's — expected, and the reason parity tolerances
/// are set physically rather than at machine epsilon
/// (`docs/bedok-port-scoping.md` §5).
#[derive(Debug)]
pub struct SparseLu {
    factorisation: faer::sparse::linalg::solvers::Lu<usize, f64>,
    order: usize,
}

impl SparseLu {
    /// Factorise `a`, which must be square.
    ///
    /// # Errors
    ///
    /// [`CouplingError::SparseAssembly`] if `a` is not square, or
    /// [`CouplingError::Singular`] if the factorisation fails (a structurally or
    /// numerically singular operator — in BEDOK this means the diffusion
    /// operator has lost a row, usually an all-void plane).
    pub fn factorise(a: &SparseMatrix) -> Result<Self> {
        if a.nrows() != a.ncols() {
            return Err(CouplingError::SparseAssembly {
                reason: format!("LU needs a square matrix, got {}x{}", a.nrows(), a.ncols()),
            });
        }
        let factorisation = a.sp_lu().map_err(|e| CouplingError::Singular {
            reason: format!("{e:?}"),
        })?;
        Ok(Self {
            factorisation,
            order: a.nrows(),
        })
    }

    /// Solve `A x = b` with the cached factorisation — the MATLAB `dM\b`.
    ///
    /// # Panics
    ///
    /// If `b.len()` differs from the order of the factorised matrix.
    #[must_use]
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        assert_eq!(
            self.order,
            b.len(),
            "right-hand side has {} entries, factorised matrix is {}x{}",
            b.len(),
            self.order,
            self.order
        );
        let mut rhs = Mat::<f64>::zeros(self.order, 1);
        for (i, &v) in b.iter().enumerate() {
            rhs[(i, 0)] = v;
        }
        self.factorisation.solve_in_place(rhs.as_mut());
        (0..self.order).map(|i| rhs[(i, 0)]).collect()
    }

    /// Order of the factorised matrix.
    #[must_use]
    pub const fn order(&self) -> usize {
        self.order
    }
}

/// Replace non-finite entries with zero — MATLAB `fixinfnan.m` (default mode).
///
/// Yan Ren applies this to every flux solve so that a blown-up node does not
/// poison the whole vector through the subsequent norms. The "special mode"
/// (`fixinfnan(v, anything)`, which substitutes `min(abs(v))` instead) is not
/// used anywhere the coupling calls it, so only the default is translated.
///
/// # Note
///
/// This silently converts divergence into a zero flux. It is translated as-is;
/// it is a symptom-suppressor, not a fix, and any solve that needs it has
/// already failed.
pub fn fix_inf_nan(v: &mut [f64]) {
    for x in v.iter_mut() {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
}

/// Sum of a vector, MATLAB `sum(v)`.
#[must_use]
pub fn sum(v: &[f64]) -> f64 {
    v.iter().sum()
}

/// Euclidean norm, MATLAB `norm(v)` / `norm(v,2)`.
#[must_use]
pub fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Sum of absolute values, MATLAB `norm(v,1)`.
#[must_use]
pub fn norm1(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).sum()
}

/// Largest absolute difference between two vectors, MATLAB
/// `max(abs(a-b))` — the fuel-temperature convergence measure, in kelvin.
///
/// # Panics
///
/// If the two vectors have different lengths.
#[must_use]
pub fn max_abs_difference(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "max_abs_difference on unequal lengths");
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f64, f64::max)
}

/// Picard under-relaxation `(1-w)*old + w*new`, applied in place to `new`.
///
/// `w` is the relaxation factor: `w = 1` is no damping (take the new value),
/// `w -> 0` freezes the field. Yan Ren defaults it to 0.5 for the
/// neutronics/T-H feedback fields.
///
/// # Panics
///
/// If the two vectors have different lengths.
pub fn under_relax(new: &mut [f64], old: &[f64], w: f64) {
    assert_eq!(new.len(), old.len(), "under_relax on unequal lengths");
    for (n, &o) in new.iter_mut().zip(old.iter()) {
        *n = (1.0 - w) * o + w * *n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tridiagonal(n: usize) -> SparseMatrix {
        let mut t = Vec::new();
        for i in 0..n {
            t.push(Triplet::new(i, i, 2.0));
            if i + 1 < n {
                t.push(Triplet::new(i, i + 1, -1.0));
                t.push(Triplet::new(i + 1, i, -1.0));
            }
        }
        from_triplets(n, n, &t).expect("assembles")
    }

    #[test]
    fn matvec_matches_a_dense_reference() {
        let a = tridiagonal(4);
        let x = [1.0, 2.0, 3.0, 4.0];
        // 2*1-2 = 0 ; -1+4-3 = 0 ; -2+6-4 = 0 ; -3+8 = 5
        assert_eq!(spmv(&a, &x), vec![0.0, 0.0, 0.0, 5.0]);
    }

    #[test]
    fn lu_round_trips_a_solve() {
        let a = tridiagonal(6);
        let x_exact = [1.0, -2.0, 3.5, 0.25, -1.0, 4.0];
        let b = spmv(&a, &x_exact);
        let lu = SparseLu::factorise(&a).expect("nonsingular");
        let x = lu.solve(&b);
        for (got, want) in x.iter().zip(x_exact.iter()) {
            assert!((got - want).abs() < 1e-12, "got {got}, want {want}");
        }
    }

    #[test]
    fn linear_combination_sums_duplicate_entries() {
        let a = tridiagonal(3);
        let combined = linear_combination(&[(1.0, &a), (-1.0, &a)]).expect("assembles");
        for &v in combined.val() {
            assert_eq!(v, 0.0);
        }
    }

    #[test]
    fn column_scaling_multiplies_on_the_right() {
        let a = tridiagonal(3);
        let d = [1.0, 2.0, 3.0];
        let scaled = scale_columns(&a, &d).expect("assembles");
        let x = [1.0, 1.0, 1.0];
        // A*diag(d)*x == A*d
        assert_eq!(spmv(&scaled, &x), spmv(&a, &d));
    }

    #[test]
    fn diagonal_addition_shifts_the_diagonal() {
        let a = tridiagonal(3);
        let shifted = add_diagonal(&a, &[1.0, 1.0, 1.0]).expect("assembles");
        let x = [1.0, 0.0, 0.0];
        let base = spmv(&a, &x);
        let got = spmv(&shifted, &x);
        assert_eq!(got[0], base[0] + 1.0);
        assert_eq!(got[1], base[1]);
    }

    #[test]
    fn non_finite_entries_become_zero() {
        let mut v = [1.0, f64::NAN, f64::INFINITY, -2.0];
        fix_inf_nan(&mut v);
        assert_eq!(v, [1.0, 0.0, 0.0, -2.0]);
    }

    #[test]
    fn under_relaxation_interpolates() {
        let mut new = [10.0, 0.0];
        under_relax(&mut new, &[0.0, 10.0], 0.5);
        assert_eq!(new, [5.0, 5.0]);
    }
}
