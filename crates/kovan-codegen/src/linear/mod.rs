//! Linear-solver code generation.
//!
//! Emits self-contained, documented Rust source for the dense direct solvers in
//! the KOVAN catalogue (docs/kovan.md § "Numerical Methods → Linear Solvers").
//! As with the other families, each template file is both emitted verbatim by
//! [`generate`] and compiled into [`mod@reference`] so the tests exercise the exact
//! emitted source.
//!
//! Fully generated: LU (Gaussian elimination with partial pivoting).
//! Catalogued but not yet generated: Jacobi, Gauss–Seidel, SOR, conjugate
//! gradient, BiCGSTAB, GMRES, QR, Cholesky.

use crate::{CodegenError, LinearSolver};

/// The exact algorithm sources that [`generate`] emits, compiled for testing.
pub mod reference {
    /// Dense LU solver — see `templates/lu.rs`.
    pub mod lu {
        include!("templates/lu.rs");
    }
}

/// Generate Rust source for the requested [`LinearSolver`].
///
/// # Errors
/// [`CodegenError::Unimplemented`] for the iterative Krylov solvers and the
/// QR / Cholesky factorisations, which are catalogued but not yet generated.
pub fn generate(method: LinearSolver) -> Result<String, CodegenError> {
    let src = match method {
        LinearSolver::Lu => include_str!("templates/lu.rs"),
        LinearSolver::Jacobi => return Err(CodegenError::Unimplemented("linear solver: Jacobi")),
        LinearSolver::GaussSeidel => {
            return Err(CodegenError::Unimplemented("linear solver: Gauss-Seidel"))
        }
        LinearSolver::Sor => return Err(CodegenError::Unimplemented("linear solver: SOR")),
        LinearSolver::ConjugateGradient => {
            return Err(CodegenError::Unimplemented(
                "linear solver: ConjugateGradient",
            ))
        }
        LinearSolver::BiCgStab => {
            return Err(CodegenError::Unimplemented("linear solver: BiCGSTAB"))
        }
        LinearSolver::Gmres => return Err(CodegenError::Unimplemented("linear solver: GMRES")),
        LinearSolver::Qr => return Err(CodegenError::Unimplemented("linear solver: QR")),
        LinearSolver::Cholesky => {
            return Err(CodegenError::Unimplemented("linear solver: Cholesky"))
        }
    };
    Ok(src.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lu_solves_known_3x3_system() {
        // A x = b with the exact solution x = [20, -11, 30]:
        //   2x + y -  z =  -1
        //  -3x - y + 2z =  11
        //  -2x + y + 2z =   9
        let a = vec![
            vec![2.0, 1.0, -1.0],
            vec![-3.0, -1.0, 2.0],
            vec![-2.0, 1.0, 2.0],
        ];
        let b = vec![-1.0, 11.0, 9.0];
        let x = reference::lu::lu_solve(&a, &b).unwrap();
        let expected = [20.0, -11.0, 30.0];
        for (got, want) in x.iter().zip(expected) {
            assert!((got - want).abs() < 1e-12, "got {got}, want {want}");
        }
    }

    #[test]
    fn lu_requires_pivoting_when_leading_pivot_is_zero() {
        // Leading 0 on the diagonal forces a row swap; solution is [2, 3].
        let a = vec![vec![0.0, 1.0], vec![1.0, 1.0]];
        let b = vec![3.0, 5.0];
        let x = reference::lu::lu_solve(&a, &b).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-12 && (x[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn lu_detects_singular_matrix() {
        let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]]; // rank 1
        let b = vec![1.0, 2.0];
        assert!(reference::lu::lu_solve(&a, &b).is_none());
    }

    #[test]
    fn generate_is_deterministic() {
        assert_eq!(
            generate(LinearSolver::Lu).unwrap(),
            generate(LinearSolver::Lu).unwrap()
        );
    }

    #[test]
    fn unimplemented_variants_error() {
        assert!(generate(LinearSolver::Gmres).is_err());
        assert!(generate(LinearSolver::Cholesky).is_err());
    }
}
