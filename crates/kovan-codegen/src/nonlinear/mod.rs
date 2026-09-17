//! Nonlinear-solver code generation.
//!
//! Emits self-contained, documented Rust source for the nonlinear solvers in
//! the KOVAN catalogue (docs/kovan.md § "Numerical Methods → Nonlinear
//! Solvers"). Each template file is both emitted verbatim by [`generate`] and
//! compiled into [`mod@reference`] so the tests exercise the exact emitted source.
//!
//! Fully generated: scalar fixed-point iteration, Newton's method for systems.
//! Catalogued but not yet generated: quasi-Newton (BFGS-style), Broyden, trust
//! region.

use crate::{CodegenError, NonlinearSolver};

/// The exact algorithm sources that [`generate`] emits, compiled for testing.
pub mod reference {
    /// Scalar fixed-point iteration — see `templates/fixed_point.rs`.
    pub mod fixed_point {
        include!("templates/fixed_point.rs");
    }
    /// Newton's method for systems — see `templates/newton_system.rs`.
    pub mod newton_system {
        include!("templates/newton_system.rs");
    }
}

/// Generate Rust source for the requested [`NonlinearSolver`].
///
/// The [`NonlinearSolver::Newton`] variant emits the vector (systems) Newton
/// method; scalar fixed-point iteration is emitted alongside it as the
/// [`NonlinearSolver::QuasiNewton`] catalogue entry is not yet generated.
///
/// # Errors
/// [`CodegenError::Unimplemented`] for quasi-Newton, Broyden, and trust-region,
/// which are catalogued but not yet generated. Fixed-point iteration is always
/// available via [`generate_fixed_point`].
pub fn generate(method: NonlinearSolver) -> Result<String, CodegenError> {
    let src = match method {
        NonlinearSolver::Newton => include_str!("templates/newton_system.rs"),
        NonlinearSolver::QuasiNewton => {
            return Err(CodegenError::Unimplemented("nonlinear solver: QuasiNewton"))
        }
        NonlinearSolver::Broyden => {
            return Err(CodegenError::Unimplemented("nonlinear solver: Broyden"))
        }
        NonlinearSolver::TrustRegion => {
            return Err(CodegenError::Unimplemented("nonlinear solver: TrustRegion"))
        }
    };
    Ok(src.to_string())
}

/// Generate the scalar fixed-point iteration template.
///
/// Fixed-point iteration solves `x = g(x)` and is not a variant of the
/// [`NonlinearSolver`] enum (which enumerates Newton-family root solvers), so it
/// is exposed here directly. Deterministic; always available.
pub fn generate_fixed_point() -> String {
    include_str!("templates/fixed_point.rs").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_finds_dottie_number() {
        // x = cos(x) converges to the Dottie number, 0.7390851332151607.
        let r = reference::fixed_point::fixed_point(|x| x.cos(), 1.0, 1e-12, 500).unwrap();
        assert!((r - 0.739_085_133_215_160_6).abs() < 1e-10, "got {r}");
    }

    #[test]
    fn newton_system_solves_2d() {
        // F1 = x^2 + y^2 - 4 = 0 (circle radius 2)
        // F2 = x - y         = 0 (line y = x)
        // Root in the first quadrant: x = y = sqrt(2).
        let f = |v: &[f64]| vec![v[0] * v[0] + v[1] * v[1] - 4.0, v[0] - v[1]];
        let jac = |v: &[f64]| vec![vec![2.0 * v[0], 2.0 * v[1]], vec![1.0, -1.0]];
        let sol =
            reference::newton_system::newton_system(f, jac, vec![1.0, 1.5], 1e-12, 100).unwrap();
        let s = std::f64::consts::SQRT_2;
        assert!(
            (sol[0] - s).abs() < 1e-10 && (sol[1] - s).abs() < 1e-10,
            "{sol:?}"
        );
    }

    #[test]
    fn generate_is_deterministic() {
        assert_eq!(
            generate(NonlinearSolver::Newton).unwrap(),
            generate(NonlinearSolver::Newton).unwrap()
        );
        assert_eq!(generate_fixed_point(), generate_fixed_point());
    }

    #[test]
    fn unimplemented_variants_error() {
        assert!(generate(NonlinearSolver::Broyden).is_err());
        assert!(generate(NonlinearSolver::TrustRegion).is_err());
    }
}
