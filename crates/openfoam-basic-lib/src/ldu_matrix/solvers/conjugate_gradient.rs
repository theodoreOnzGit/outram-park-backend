use crate::ldu_matrix::ldu_matrix::LduMatrix;
use crate::ldu_matrix::fv_matrix::{SolverPerformance, SolverSettings};

/// Preconditioned Conjugate Gradient solver for **symmetric** LDU matrices.
///
/// Uses a diagonal (Jacobi) preconditioner — `M = diag(A)` — which is
/// OpenFOAM's default `DIC` (Diagonal Incomplete Cholesky without fill) for
/// the symmetric case.  The full DIC preconditioner requires a factorisation
/// pass; the diagonal-only version is exact when the matrix is already
/// diagonally dominant, and is a useful first step before adding DIC.
///
/// ## When to use vs Gauss-Seidel
///
/// | Solver | Good for |
/// |---|---|
/// | Gauss-Seidel | Convection-dominated (asymmetric upper ≠ lower) |
/// | PCG (this) | Symmetric SPD systems — pressure Poisson (`fvm::laplacian`) |
///
/// The pressure equation assembled by `fvm::laplacian` is symmetric
/// (`upper[f] == lower[f]`), so PCG converges in O(√κ) iterations vs
/// O(κ) for Gauss-Seidel, where κ is the condition number.
pub fn conjugate_gradient(
    ldu: &LduMatrix,
    b: &[f64],
    settings: &SolverSettings,
) -> (Vec<f64>, SolverPerformance) {
    let n = ldu.n_cells;
    debug_assert_eq!(b.len(), n);

    // Diagonal preconditioner: M^{-1}[c] = 1 / diag[c]
    let m_inv: Vec<f64> = ldu.diag.iter().map(|&d| {
        if d.abs() < 1e-300 { 1.0 } else { 1.0 / d }
    }).collect();

    let mut x = vec![0.0_f64; n];
    let mut r = ldu.residual(&x, b);          // r = b - A·x  (= b since x=0)

    // z = M^{-1} · r
    let mut z: Vec<f64> = r.iter().zip(m_inv.iter()).map(|(ri, mi)| ri * mi).collect();
    let mut p = z.clone();

    let mut rz = dot(&r, &z);                 // r^T M^{-1} r
    let b_norm: f64 = b.iter().map(|bi| bi * bi).sum::<f64>().sqrt().max(1e-300);

    let mut n_iter = 0;
    let mut final_residual = rz.sqrt() / b_norm;

    for iter in 0..settings.max_iter {
        let ap = ldu.multiply(&p);            // A·p
        let pap = dot(&p, &ap);               // p^T A p

        if pap.abs() < 1e-300 {
            break;
        }

        let alpha = rz / pap;

        // x = x + alpha * p
        for c in 0..n { x[c] += alpha * p[c]; }
        // r = r - alpha * A·p
        for c in 0..n { r[c] -= alpha * ap[c]; }

        // z = M^{-1} · r
        for c in 0..n { z[c] = r[c] * m_inv[c]; }

        let rz_new = dot(&r, &z);
        final_residual = rz_new.sqrt() / b_norm;

        n_iter = iter + 1;
        if final_residual < settings.tolerance {
            break;
        }

        let beta = rz_new / rz;
        // p = z + beta * p
        for c in 0..n { p[c] = z[c] + beta * p[c]; }

        rz = rz_new;
    }

    (
        x,
        SolverPerformance {
            n_iterations: n_iter,
            final_residual,
            converged: final_residual < settings.tolerance,
        },
    )
}

#[inline]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(ai, bi)| ai * bi).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn tridiag(n: usize) -> (LduMatrix, Vec<f64>) {
        // −∇²φ = 1 on [0,1], φ(0)=φ(1)=0 → φ_exact = x(1-x)/2
        // 1D finite-difference: diag = 2/h², off-diag = -1/h², source = 1
        let h = 1.0 / (n + 1) as f64;
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut m = LduMatrix::new(n, owner, neighbour);
        let coeff = 1.0 / (h * h);
        m.diag  = vec![2.0 * coeff; n];
        m.upper = vec![-coeff; n - 1];
        m.lower = vec![-coeff; n - 1];
        let b = vec![1.0; n];
        (m, b)
    }

    #[test]
    fn cg_identity() {
        let mut m = LduMatrix::new(3, vec![], vec![]);
        m.diag = vec![2.0, 3.0, 4.0];
        let b = vec![4.0, 9.0, 8.0];
        let settings = SolverSettings { tolerance: 1e-10, max_iter: 100 };
        let (x, perf) = conjugate_gradient(&m, &b, &settings);
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert_relative_eq!(x[0], 2.0, epsilon = 1e-8);
        assert_relative_eq!(x[1], 3.0, epsilon = 1e-8);
        assert_relative_eq!(x[2], 2.0, epsilon = 1e-8);
    }

    #[test]
    fn cg_solves_1d_poisson() {
        let n = 50;
        let h = 1.0 / (n + 1) as f64;
        let (m, b) = tridiag(n);
        let settings = SolverSettings { tolerance: 1e-10, max_iter: 1000 };
        let (x, perf) = conjugate_gradient(&m, &b, &settings);
        assert!(perf.converged, "did not converge: residual = {}", perf.final_residual);
        // Check against exact solution φ = x*(1-x)/2 at interior points
        for i in 0..n {
            let xi = (i + 1) as f64 * h;
            let exact = xi * (1.0 - xi) / 2.0;
            assert_relative_eq!(x[i], exact, epsilon = h * h);
        }
    }

    #[test]
    fn cg_faster_than_gs_on_symmetric() {
        // CG should converge in at most n iterations on an n×n SPD system
        let n = 20;
        let (m, b) = tridiag(n);
        let settings = SolverSettings { tolerance: 1e-10, max_iter: 500 };
        let (_, perf) = conjugate_gradient(&m, &b, &settings);
        assert!(perf.converged);
        // CG on a tridiagonal SPD should need << n iterations with preconditioning
        assert!(perf.n_iterations <= n, "expected <= {} iters, got {}", n, perf.n_iterations);
    }
}
