//! Engineering Pattern Library.
//!
//! A growing, deterministic library of small, reusable scientific-computing
//! building blocks that recur across solvers (docs/kovan.md § "Engineering
//! Pattern Library"). These are the connective tissue *around* the numerical
//! kernels — convergence measures, relaxation updates, stopping criteria —
//! rather than the kernels themselves.
//!
//! Each pattern's source lives in a template file under `templates/`, is emitted
//! verbatim by [`generate`], and is compiled into [`mod@reference`] so the tests
//! exercise the exact emitted source. New patterns are added as new
//! [`EngineeringPattern`] variants plus a template file.
//!
//! Priority order (per the KOVAN spec): correctness > traceability >
//! maintainability > performance > convenience.

/// Reusable scientific-computing patterns KOVAN codegen can emit. Closed,
/// enum-dispatched set (no trait objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineeringPattern {
    /// RMS norm of a residual vector — a grid-independent convergence measure.
    RmsResidualNorm,
    /// Under-/over-relaxation update `x_old + alpha (x_star - x_old)`.
    UnderRelaxationUpdate,
    /// Relative-change stopping criterion `|new - old| / max(|new|, floor)`.
    RelativeChange,
}

/// The exact pattern sources that [`generate`] emits, compiled for testing.
pub mod reference {
    /// RMS residual norm — see `templates/rms_residual_norm.rs`.
    pub mod rms_residual_norm {
        include!("templates/rms_residual_norm.rs");
    }
    /// Under-relaxation update — see `templates/under_relaxation.rs`.
    pub mod under_relaxation {
        include!("templates/under_relaxation.rs");
    }
    /// Relative-change criterion — see `templates/relative_change.rs`.
    pub mod relative_change {
        include!("templates/relative_change.rs");
    }
}

/// Generate Rust source for the requested [`EngineeringPattern`].
///
/// Every current pattern is fully generated, so this is infallible for now; it
/// returns `String` (not `Result`) to keep the pattern API distinct from the
/// numerical-method [`crate::generate`] surface, which can report unimplemented
/// catalogue entries. The output is deterministic.
pub fn generate(pattern: EngineeringPattern) -> String {
    let src = match pattern {
        EngineeringPattern::RmsResidualNorm => include_str!("templates/rms_residual_norm.rs"),
        EngineeringPattern::UnderRelaxationUpdate => {
            include_str!("templates/under_relaxation.rs")
        }
        EngineeringPattern::RelativeChange => include_str!("templates/relative_change.rs"),
    };
    src.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_norm_of_known_vector() {
        // sqrt((3^2 + 4^2)/2) = sqrt(25/2) = 3.5355339...
        let got = reference::rms_residual_norm::rms_residual_norm(&[3.0, 4.0]);
        assert!((got - (12.5_f64).sqrt()).abs() < 1e-12, "got {got}");
        assert_eq!(reference::rms_residual_norm::rms_residual_norm(&[]), 0.0);
    }

    #[test]
    fn under_relaxation_blends_correctly() {
        // Half-way blend of 0 and 10 is 5.
        let got = reference::under_relaxation::under_relaxation_update(0.0, 10.0, 0.5);
        assert!((got - 5.0).abs() < 1e-15);
        // alpha = 1 gives the raw update.
        let raw = reference::under_relaxation::under_relaxation_update(2.0, 7.0, 1.0);
        assert!((raw - 7.0).abs() < 1e-15);
    }

    #[test]
    fn relative_change_is_scale_free_and_zero_safe() {
        let rc = reference::relative_change::relative_change(1.01, 1.0, 1e-30);
        assert!((rc - 0.01 / 1.01).abs() < 1e-15);
        // Passing through zero must not divide by zero.
        let safe = reference::relative_change::relative_change(0.0, 1e-40, 1e-30);
        assert!(safe.is_finite());
    }

    #[test]
    fn generate_is_deterministic() {
        for p in [
            EngineeringPattern::RmsResidualNorm,
            EngineeringPattern::UnderRelaxationUpdate,
            EngineeringPattern::RelativeChange,
        ] {
            assert_eq!(generate(p), generate(p));
        }
    }
}
