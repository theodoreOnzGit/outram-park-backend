//! Root-finding code generation.
//!
//! Emits self-contained, documented Rust source for the scalar root finders in
//! the KOVAN catalogue (docs/kovan.md § "Numerical Methods → Root Finding").
//! Each method's source lives in a template file under `templates/`. That same
//! file is both:
//!
//! * returned verbatim (via `include_str!`) by [`generate`], and
//! * compiled into the [`mod@reference`] module (via `include!`),
//!
//! so the numerical-correctness tests at the bottom of this file exercise the
//! *exact bytes* that [`generate`] emits. Emission and verification cannot
//! drift.
//!
//! Fully generated: bisection, regula falsi, secant, Newton–Raphson, Brent.
//! Catalogued but not yet generated: Illinois, Pegasus (regula-falsi variants).

use crate::{CodegenError, RootFinder};

/// The exact algorithm sources that [`generate`] emits, compiled so the tests
/// can execute them. Each submodule is one template file.
pub mod reference {
    /// Bisection root finder — see `templates/bisection.rs`.
    pub mod bisection {
        include!("templates/bisection.rs");
    }
    /// Regula-falsi root finder — see `templates/regula_falsi.rs`.
    pub mod regula_falsi {
        include!("templates/regula_falsi.rs");
    }
    /// Secant root finder — see `templates/secant.rs`.
    pub mod secant {
        include!("templates/secant.rs");
    }
    /// Newton–Raphson root finder — see `templates/newton_raphson.rs`.
    pub mod newton_raphson {
        include!("templates/newton_raphson.rs");
    }
    /// Brent root finder — see `templates/brent.rs`.
    pub mod brent {
        include!("templates/brent.rs");
    }
}

/// Generate Rust source for the requested [`RootFinder`].
///
/// Returns compilable, `///`-documented source implementing the method with
/// plain `f64` and a generic function argument (no external dependencies, no
/// `unsafe`, no trait-object dispatch). The returned string is deterministic:
/// the same input always yields byte-identical output.
///
/// # Errors
/// [`CodegenError::Unimplemented`] for catalogued-but-not-yet-generated
/// variants (Illinois, Pegasus).
pub fn generate(method: RootFinder) -> Result<String, CodegenError> {
    let src = match method {
        RootFinder::Bisection => include_str!("templates/bisection.rs"),
        RootFinder::RegulaFalsi => include_str!("templates/regula_falsi.rs"),
        RootFinder::Secant => include_str!("templates/secant.rs"),
        RootFinder::NewtonRaphson => include_str!("templates/newton_raphson.rs"),
        RootFinder::Brent => include_str!("templates/brent.rs"),
        RootFinder::Illinois => return Err(CodegenError::Unimplemented("root finder: Illinois")),
        RootFinder::Pegasus => return Err(CodegenError::Unimplemented("root finder: Pegasus")),
    };
    Ok(src.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // sqrt(2) is the positive root of f(x) = x^2 - 2. Analytical value:
    // 1.4142135623730951. Every generated finder must recover it.
    const SQRT2: f64 = std::f64::consts::SQRT_2;
    fn f(x: f64) -> f64 {
        x * x - 2.0
    }
    fn df(x: f64) -> f64 {
        2.0 * x
    }

    #[test]
    fn bisection_finds_sqrt2() {
        let r = reference::bisection::bisection(f, 0.0, 2.0, 1e-12, 200).unwrap();
        assert!((r - SQRT2).abs() < 1e-11, "got {r}");
    }

    #[test]
    fn regula_falsi_finds_sqrt2() {
        let r = reference::regula_falsi::regula_falsi(f, 0.0, 2.0, 1e-12, 200).unwrap();
        assert!((r - SQRT2).abs() < 1e-10, "got {r}");
    }

    #[test]
    fn secant_finds_sqrt2() {
        let r = reference::secant::secant(f, 1.0, 2.0, 1e-13, 100).unwrap();
        assert!((r - SQRT2).abs() < 1e-11, "got {r}");
    }

    #[test]
    fn newton_finds_sqrt2() {
        let r = reference::newton_raphson::newton_raphson(f, df, 1.0, 1e-13, 100).unwrap();
        assert!((r - SQRT2).abs() < 1e-12, "got {r}");
    }

    #[test]
    fn brent_finds_sqrt2() {
        let r = reference::brent::brent(f, 0.0, 2.0, 1e-13, 100).unwrap();
        assert!((r - SQRT2).abs() < 1e-11, "got {r}");
    }

    #[test]
    fn bad_bracket_returns_none() {
        assert!(reference::bisection::bisection(f, 3.0, 4.0, 1e-12, 50).is_none());
        assert!(reference::brent::brent(f, 3.0, 4.0, 1e-12, 50).is_none());
    }

    #[test]
    fn generate_is_deterministic() {
        for m in [
            RootFinder::Bisection,
            RootFinder::Secant,
            RootFinder::NewtonRaphson,
            RootFinder::Brent,
        ] {
            assert_eq!(generate(m).unwrap(), generate(m).unwrap());
        }
    }

    #[test]
    fn unimplemented_variants_error() {
        assert!(generate(RootFinder::Illinois).is_err());
        assert!(generate(RootFinder::Pegasus).is_err());
    }
}
