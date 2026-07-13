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
}
