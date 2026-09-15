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
//! is the object OpenMC assembles in `openmc/deplete/` before handing it to a
//! matrix-exponential solver; see the `cram` module for the solver that
//! consumes it.
//!
//! # Sign / index convention (read before using `set`/`add`)
//!
//! `A` is indexed `A[row][col]`, and the ODE is `dN[row]/dt = sum_col A[row][col] * N[col]`:
//!
//! * **Off-diagonal** `A[i][j]` (`i != j`) is the **production** rate coefficient
//!   of nuclide `i` from nuclide `j` — units `1/s`, always `>= 0`. For a decay
//!   `j -> i` with decay constant `lambda_j` and branching `b`, this is
//!   `+b * lambda_j`. For a neutron reaction `j -> i` with one-group microscopic
//!   reaction rate `r_j` (`1/s`), this is `+r_j`.
//! * **Diagonal** `A[j][j]` is the **total removal** rate coefficient of nuclide
//!   `j` — units `1/s`, always `<= 0`. It is the negative sum of every decay
//!   constant and every neutron-absorption rate that destroys `j`.
//!
//! Assembling `A` so that each column `j`'s entries sum to zero (for a pure
//! rearrangement with no net loss to species outside the tracked set) makes
//! total-atom conservation exact; the `cram` tests exploit this.
//!
//! # Units
//!
//! Every stored coefficient is `1/s`. Number densities passed to [`DepletionMatrix::mat_vec`]
//! may be in any consistent unit (atoms, atoms/(barn·cm), …) — the matrix is
//! linear, so the unit of `N` simply carries through to `dN/dt`.

/// A dense `n x n` depletion (burnup) matrix `A` in units of inverse seconds,
/// defining the linear system `dN/dt = A N`.
///
/// Stored row-major (`A[row][col] == data[row * n + col]`). The matrix is
/// typically sparse in practice (each nuclide feeds only a handful of
/// daughters), but depletion chains handled by this crate are small (tens of
/// nuclides), so a dense store is simplest and keeps the CRAM linear solves
/// straightforward. See the module docs for the sign/index convention.
#[derive(Debug, Clone, PartialEq)]
pub struct DepletionMatrix {
    n: usize,
    /// Row-major coefficients, length `n * n`, units `1/s`.
    data: Vec<f64>,
}

impl DepletionMatrix {
    /// A zero `order x order` matrix (no decay, no transmutation).
    ///
    /// `order` is the number of nuclides tracked in the depletion chain.
    pub fn zeros(order: usize) -> Self {
        Self {
            n: order,
            data: vec![0.0; order * order],
        }
    }

    /// The matrix order `n` — the number of tracked nuclides.
    pub fn order(&self) -> usize {
        self.n
    }

    /// The coefficient `A[row][col]` in `1/s`.
    ///
    /// Panics if `row` or `col` is out of range.
    pub fn get(&self, row: usize, col: usize) -> f64 {
        assert!(
            row < self.n && col < self.n,
            "index ({row},{col}) out of range for order {}",
            self.n
        );
        self.data[row * self.n + col]
    }

    /// Overwrite the coefficient `A[row][col]` (`1/s`).
    ///
    /// Panics if `row` or `col` is out of range.
    pub fn set(&mut self, row: usize, col: usize, val: f64) {
        assert!(
            row < self.n && col < self.n,
            "index ({row},{col}) out of range for order {}",
            self.n
        );
        self.data[row * self.n + col] = val;
    }

    /// Accumulate `val` (`1/s`) into `A[row][col]`.
    ///
    /// This is the natural primitive when building a chain: each decay or
    /// reaction channel adds its production term to an off-diagonal and its
    /// removal term to the diagonal. Panics if indices are out of range.
    pub fn add(&mut self, row: usize, col: usize, val: f64) {
        assert!(
            row < self.n && col < self.n,
            "index ({row},{col}) out of range for order {}",
            self.n
        );
        self.data[row * self.n + col] += val;
    }

    /// The right-hand side `dN/dt = A N` for a number-density vector `n_vec`.
    ///
    /// `n_vec.len()` must equal [`order`](Self::order). Returns a fresh vector
    /// of the same length. Used by explicit ODE checks and by callers that want
    /// the instantaneous production/removal rates.
    pub fn mat_vec(&self, n_vec: &[f64]) -> Vec<f64> {
        assert_eq!(
            n_vec.len(),
            self.n,
            "vector length {} != matrix order {}",
            n_vec.len(),
            self.n
        );
        let mut out = vec![0.0; self.n];
        for row in 0..self.n {
            let base = row * self.n;
            let mut acc = 0.0;
            for col in 0..self.n {
                acc += self.data[base + col] * n_vec[col];
            }
            out[row] = acc;
        }
        out
    }

    /// Read-only view of the row-major coefficient buffer (`1/s`), length `n*n`.
    ///
    /// Intended for solvers (e.g. CRAM) that copy `A` into a working complex
    /// matrix. Prefer [`get`](Self::get) for individual lookups.
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}
