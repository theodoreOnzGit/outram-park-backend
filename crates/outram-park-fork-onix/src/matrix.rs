//! The burnup / depletion matrix `A` (units `1/s`).
//!
//! The Bateman depletion equation is `dn/dt = A·n`, where `n` is the vector of
//! nuclide number densities (atoms, or atoms·cm⁻³) and `A` is the burnup matrix
//! with units `1/s`. For nuclide `i`:
//!
//! ```text
//!   A[i][i] = -(λ_i + Σ_c r_{i,c})              (loss: decay + all reactions)
//!   A[i][j] =  (partial decay j→i) + (reaction rate j→i)   (gain from parent j)
//! ```
//!
//! This is exactly ONIX's `A = B·1e-24·φ + C` (`onix/salameche/burn.py:187`),
//! where `B` is the cross-section matrix (`get_xs_mat`) and `C` the decay matrix
//! (`get_decay_mat`). Here the reaction rates are supplied already in `1/s`.
//!
//! The matrix is stored **dense, row-major**. Depletion matrices are sparse in
//! reality, but for the modest nuclide counts of a stand-alone chain (tens to a
//! few hundred) dense storage plus a dense complex solve in [`crate::cram`] is
//! simple, allocation-light, and pure Rust (no BLAS).
//!
//! ## Provenance (GPLv3 relicensing of MIT upstream)
//!
//! Assembly logic ported from ONIX (open-source, MIT; commit `7328dc6`):
//! `onix/salameche/mat_builder.py` (`get_xs_mat` lines 5–127, `get_decay_mat`
//! lines 134–193) and `onix/salameche/burn.py:187` (the `B·1e-24·φ + C` sum).
//! Independent Rust re-implementation; OUTRAM PARK fork relicenses under
//! **GPL-3.0-only** (MIT is GPL-3.0-compatible).

/// A dense, row-major depletion matrix `A` with units `1/s`.
///
/// Index convention: `A[i][j]` is the rate at which parent `j` feeds nuclide
/// `i` (off-diagonal, `>= 0`); the diagonal `A[i][i]` is the total loss rate of
/// nuclide `i` (`<= 0`). Multiply by a timestep Δt (seconds) to get the
/// dimensionless matrix whose exponential action gives the depleted inventory.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnupMatrix {
    n: usize,
    /// Row-major `n*n` entries, units `1/s`.
    data: Vec<f64>,
}

impl BurnupMatrix {
    /// An `n`×`n` zero matrix.
    pub fn zeros(n: usize) -> Self {
        Self {
            n,
            data: vec![0.0; n * n],
        }
    }

    /// The dimension `n` (number of tracked nuclides).
    pub fn dim(&self) -> usize {
        self.n
    }

    /// Read entry `A[i][j]` (units `1/s`). Panics if out of bounds.
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.n + j]
    }

    /// Set entry `A[i][j]` (units `1/s`). Panics if out of bounds.
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        self.data[i * self.n + j] = v;
    }

    /// Accumulate `v` into `A[i][j]` (units `1/s`). Panics if out of bounds.
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        self.data[i * self.n + j] += v;
    }

    /// Borrow the raw row-major entries (units `1/s`).
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Matrix–vector product `A·x` (units of the result: `1/s` × units of `x`).
    ///
    /// Used by the analytic-comparison tests and by callers who want the
    /// instantaneous rate of change `dn/dt = A·n`. `x` must have length
    /// [`BurnupMatrix::dim`].
    pub fn mul_vec(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(x.len(), self.n, "vector length must equal matrix dimension");
        let mut out = vec![0.0; self.n];
        for (i, out_i) in out.iter_mut().enumerate() {
            let row = &self.data[i * self.n..(i + 1) * self.n];
            *out_i = row.iter().zip(x).map(|(a, b)| a * b).sum();
        }
        out
    }

    /// Column sums of `A` (units `1/s`), one per parent nuclide `j`.
    ///
    /// For a matrix that only *rearranges* atoms among tracked nuclides (no net
    /// creation: every daughter of every channel is tracked, no fission), each
    /// column sums to `0` — loss on the diagonal exactly balances the gains it
    /// feeds. A nonzero column sum means nuclide `j` loses atoms to species not
    /// tracked in the system (leakage out of the chain), or, if negative-below
    /// zero is impossible, indicates the fission/absorption bookkeeping. This is
    /// the basis of the total-atom-conservation V&V check.
    pub fn column_sums(&self) -> Vec<f64> {
        let mut sums = vec![0.0; self.n];
        for i in 0..self.n {
            let row = &self.data[i * self.n..(i + 1) * self.n];
            for (s, &v) in sums.iter_mut().zip(row) {
                *s += v;
            }
        }
        sums
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_vec_matches_hand_calc() {
        // A 2x2: [[-1, 0], [1, 0]] represents A -> B with lambda_A = 1.
        let mut a = BurnupMatrix::zeros(2);
        a.set(0, 0, -1.0);
        a.set(1, 0, 1.0);
        let dn = a.mul_vec(&[10.0, 0.0]);
        assert_eq!(dn, vec![-10.0, 10.0]);
    }

    #[test]
    fn column_sums_zero_for_pure_transfer() {
        // Column 0: -1 (loss of A) + 1 (gain of B) = 0 -> atoms conserved.
        let mut a = BurnupMatrix::zeros(2);
        a.set(0, 0, -1.0);
        a.set(1, 0, 1.0);
        let cs = a.column_sums();
        assert!(cs[0].abs() < 1e-15);
    }
}
