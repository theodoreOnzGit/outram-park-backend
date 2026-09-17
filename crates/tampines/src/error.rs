//! Crate-wide error type for TAMPINES.

use thiserror::Error;

/// Errors returned by TAMPINES's public API.
///
/// The framework is built out incrementally (see the crate-level docs for the
/// current module surface); a public item that exists as a documented stub
/// but has no working implementation yet returns
/// [`TampinesError::NotYetImplemented`] rather than panicking or silently
/// returning a placeholder value.
#[derive(Debug, Error)]
pub enum TampinesError {
    /// The called component's physics is not implemented yet.
    ///
    /// `component` names the module or method (e.g. `"hem::future_multiphase::drift_flux"`)
    /// so a caller hitting this can find the relevant stub and its tracking
    /// bead.
    #[error("TAMPINES component not yet implemented: {component}")]
    NotYetImplemented {
        /// Path-like name of the unimplemented component.
        component: &'static str,
    },

    /// A caller-supplied input is outside the range the model accepts.
    ///
    /// Distinct from [`Unphysical`](Self::Unphysical): this is a *usage*
    /// error — a mismatched slice length, a non-positive cell count, a
    /// negative timestep — caught before any physics runs.
    #[error("TAMPINES invalid input: {0}")]
    InvalidInput(String),

    /// A quantity left the range physics allows, during a solve.
    ///
    /// Reported rather than clamped. A void fraction of `1.4` or a negative
    /// absolute pressure means the discretisation has failed, and continuing
    /// from a clamped value produces a plausible-looking answer that is
    /// wrong — the failure mode this crate's V&V rules exist to prevent.
    #[error("TAMPINES unphysical state: {0}")]
    Unphysical(String),

    /// A linear solve or an iteration failed.
    ///
    /// Carries what failed and where, so a caller can tell a singular matrix
    /// from a non-converged fixed point.
    #[error("TAMPINES numerical failure: {0}")]
    Numerical(String),

    /// A closure borrowed from the OUTRAM-FOAM 3-D multiphase reference
    /// rejected its inputs.
    ///
    /// The 1-D solvers in [`crate::multiphase_1d`] evaluate
    /// [`outram_foam_multiphase`]'s slip and drag correlations directly, so
    /// that crate's own error surfaces here rather than being flattened into a
    /// string — the provenance of a failure is part of the diagnosis.
    #[error("TAMPINES multiphase closure failed: {0}")]
    Closure(#[from] outram_foam_multiphase::MultiphaseError),
}
