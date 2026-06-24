/// Sparse LDU (lower-diagonal-upper) matrix for FV implicit operators.
///
/// Mirrors `Foam::lduMatrix` from
/// `src/OpenFOAM/matrices/lduMatrix/lduMatrix/lduMatrix.H`.
///
/// Storage follows OpenFOAM's face-addressing convention:
/// ```text
/// A·x[c] = diag[c]·x[c]
///          + Σ_{f: owner[f]=c} upper[f]·x[neighbour[f]]
///          + Σ_{f: neighbour[f]=c} lower[f]·x[owner[f]]
/// ```
/// For a symmetric matrix (e.g. Laplacian), `lower[f] == upper[f]`.
#[derive(Debug, Clone)]
pub struct LduMatrix {
    pub n_cells: usize,
    pub n_internal_faces: usize,

    /// Diagonal coefficients, length `n_cells`.
    pub diag: Vec<f64>,
    /// Lower off-diagonal (neighbour → owner contribution), length `n_internal_faces`.
    pub lower: Vec<f64>,
    /// Upper off-diagonal (owner → neighbour contribution), length `n_internal_faces`.
    pub upper: Vec<f64>,

    /// Owner cell index per internal face (shared with `FvMesh`).
    pub owner: Vec<usize>,
    /// Neighbour cell index per internal face (shared with `FvMesh`).
    pub neighbour: Vec<usize>,
}

impl LduMatrix {
    pub fn new(
        n_cells: usize,
        owner: Vec<usize>,
        neighbour: Vec<usize>,
    ) -> Self {
        let n_int = owner.len();
        debug_assert_eq!(neighbour.len(), n_int);
        Self {
            n_cells,
            n_internal_faces: n_int,
            diag: vec![0.0; n_cells],
            lower: vec![0.0; n_int],
            upper: vec![0.0; n_int],
            owner,
            neighbour,
        }
    }

    /// Matrix–vector product `y = A·x` (used for residual calculation).
    pub fn multiply(&self, x: &[f64]) -> Vec<f64> {
        debug_assert_eq!(x.len(), self.n_cells);
        let mut y = vec![0.0_f64; self.n_cells];

        // Diagonal
        for c in 0..self.n_cells {
            y[c] += self.diag[c] * x[c];
        }

        // Off-diagonal (both lower and upper contributions per face)
        for f in 0..self.n_internal_faces {
            let o = self.owner[f];
            let n = self.neighbour[f];
            y[o] += self.upper[f] * x[n];
            y[n] += self.lower[f] * x[o];
        }

        y
    }

    /// Residual `r = b - A·x`.
    pub fn residual(&self, x: &[f64], b: &[f64]) -> Vec<f64> {
        let ax = self.multiply(x);
        b.iter().zip(ax.iter()).map(|(bi, ai)| bi - ai).collect()
    }

    /// L1-scaled norm of residual: `||r||₁ / (||A·x||₁ + ε)`.
    pub fn normalised_residual(&self, x: &[f64], b: &[f64]) -> f64 {
        let ax = self.multiply(x);
        let r_norm: f64 = b.iter().zip(ax.iter()).map(|(bi, ai)| (bi - ai).abs()).sum();
        let ax_norm: f64 = ax.iter().map(|a| a.abs()).sum::<f64>()
            + b.iter().map(|bi| bi.abs()).sum::<f64>();
        let denom = ax_norm * 0.5;
        if denom < f64::EPSILON { r_norm } else { r_norm / denom }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// 2×2 identity:  diag = [1, 1], lower = upper = [] (no off-diag)
    fn identity_2x2() -> LduMatrix {
        let mut m = LduMatrix::new(2, vec![], vec![]);
        m.diag[0] = 1.0;
        m.diag[1] = 1.0;
        m
    }

    /// Symmetric tridiagonal for 3 cells connected by 2 internal faces:
    /// ```
    ///  A = [ 2 -1  0 ]
    ///      [-1  2 -1 ]
    ///      [ 0 -1  2 ]
    /// ```
    fn tridiag_3x3() -> LduMatrix {
        // faces: f0 = (0,1), f1 = (1,2)
        let mut m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
        m.diag  = vec![ 2.0,  2.0,  2.0];
        m.upper = vec![-1.0, -1.0];
        m.lower = vec![-1.0, -1.0];
        m
    }

    #[test]
    fn identity_multiply() {
        let m = identity_2x2();
        let x = vec![3.0, 7.0];
        let y = m.multiply(&x);
        assert_relative_eq!(y[0], 3.0);
        assert_relative_eq!(y[1], 7.0);
    }

    #[test]
    fn tridiag_multiply() {
        let m = tridiag_3x3();
        // A · [1, 1, 1] = [1, 0, 1]  (boundary rows get diag contribution minus off-diag)
        let x = vec![1.0, 1.0, 1.0];
        let y = m.multiply(&x);
        assert_relative_eq!(y[0], 2.0 * 1.0 + (-1.0) * 1.0, epsilon = 1e-12);
        assert_relative_eq!(y[1], (-1.0)*1.0 + 2.0*1.0 + (-1.0)*1.0, epsilon = 1e-12);
        assert_relative_eq!(y[2], (-1.0)*1.0 + 2.0*1.0, epsilon = 1e-12);
    }

    #[test]
    fn residual_exact_solution() {
        let m = identity_2x2();
        let b = vec![3.0, 7.0];
        let x = vec![3.0, 7.0]; // exact solution
        let r = m.residual(&x, &b);
        assert_relative_eq!(r[0], 0.0, epsilon = 1e-15);
        assert_relative_eq!(r[1], 0.0, epsilon = 1e-15);
    }
}
