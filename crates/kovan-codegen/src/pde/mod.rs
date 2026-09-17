//! PDE-infrastructure code generation.
//!
//! Emits discretisation scaffolding for the spatial operators in the KOVAN
//! catalogue (docs/kovan.md § "Numerical Methods → PDE Infrastructure":
//! finite-volume methods, finite-difference methods, discretisation helpers,
//! boundary-condition scaffolds).
//!
//! Fully generated: a 1-D finite-difference Poisson solver (central second-order
//! stencil + Thomas tridiagonal solve) as the reference discretisation. Its
//! template file is both emitted verbatim by [`generate`] and compiled into
//! [`mod@reference`], so the test below exercises the exact emitted source.
//!
//! Catalogued but not yet generated: multi-dimensional finite differences,
//! finite-volume flux assembly, and general boundary-condition scaffolds — these
//! are large and are deferred (see `DECISIONS.md`).

use crate::CodegenError;

/// PDE discretisation schemes KOVAN codegen can emit. A closed, enum-dispatched
/// set (no trait objects), mirroring the numerical-method catalogue enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdeScheme {
    /// 1-D Poisson `-u'' = s` via second-order central finite differences and a
    /// Thomas tridiagonal solve, with Dirichlet boundaries. Fully generated.
    Poisson1dFiniteDifference,
    /// 1-D steady diffusion by the finite-volume method. Catalogued, not yet
    /// generated.
    Diffusion1dFiniteVolume,
    /// General Dirichlet/Neumann/Robin boundary-condition scaffold. Catalogued,
    /// not yet generated.
    BoundaryConditionScaffold,
}

/// The exact algorithm sources that [`generate`] emits, compiled for testing.
pub mod reference {
    /// 1-D finite-difference Poisson solver — see `templates/poisson_1d_fd.rs`.
    pub mod poisson_1d_fd {
        include!("templates/poisson_1d_fd.rs");
    }
}

/// Generate Rust source for the requested [`PdeScheme`].
///
/// # Errors
/// [`CodegenError::Unimplemented`] for the finite-volume diffusion scheme and
/// the boundary-condition scaffold, which are catalogued but not yet generated.
pub fn generate(scheme: PdeScheme) -> Result<String, CodegenError> {
    let src = match scheme {
        PdeScheme::Poisson1dFiniteDifference => include_str!("templates/poisson_1d_fd.rs"),
        PdeScheme::Diffusion1dFiniteVolume => {
            return Err(CodegenError::Unimplemented(
                "PDE scheme: Diffusion1dFiniteVolume",
            ))
        }
        PdeScheme::BoundaryConditionScaffold => {
            return Err(CodegenError::Unimplemented(
                "PDE scheme: BoundaryConditionScaffold",
            ))
        }
    };
    Ok(src.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_1d_recovers_sine_solution() {
        // -u'' = pi^2 sin(pi x) on [0, 1], u(0) = u(1) = 0 has the exact
        // solution u(x) = sin(pi x). Central FD is O(dx^2), so a fine grid
        // matches to a small tolerance.
        use std::f64::consts::PI;
        let (xs, us) = reference::poisson_1d_fd::solve_poisson_1d(
            0.0,
            1.0,
            200,
            |x| PI * PI * (PI * x).sin(),
            0.0,
            0.0,
        )
        .unwrap();
        let mut max_err = 0.0_f64;
        for (x, u) in xs.iter().zip(us.iter()) {
            let exact = (PI * x).sin();
            max_err = max_err.max((u - exact).abs());
        }
        assert!(max_err < 1e-4, "max error {max_err}");
    }

    #[test]
    fn generate_is_deterministic() {
        assert_eq!(
            generate(PdeScheme::Poisson1dFiniteDifference).unwrap(),
            generate(PdeScheme::Poisson1dFiniteDifference).unwrap()
        );
    }

    #[test]
    fn unimplemented_variants_error() {
        assert!(generate(PdeScheme::Diffusion1dFiniteVolume).is_err());
        assert!(generate(PdeScheme::BoundaryConditionScaffold).is_err());
    }
}
