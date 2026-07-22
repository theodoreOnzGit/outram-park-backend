//! Error type for `outram-park-fork-pflotran`.
//!
//! A single [`PflotranError`] enum covers the crate's failure modes. During the
//! scaffold phase the most common variant is [`PflotranError::NotImplemented`]:
//! entry points whose physics is not yet translated return it, so a caller can
//! never mistake an unfinished stub for a real result.

use thiserror::Error;

/// Errors returned by `outram-park-fork-pflotran`.
///
/// This enum is expected to grow as flow modes, the solver, and I/O land
/// (beads op-v6s.4..op-v6s.8). New variants are added rather than overloading a
/// generic string, so callers can `match` on the specific failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PflotranError {
    /// The requested functionality is a documented scaffold and has no real
    /// implementation yet. Carries a short label naming the missing piece
    /// (e.g. `"RICHARDS flow-mode solve"`).
    ///
    /// Returning this — instead of `todo!()`/`unimplemented!()` panics or a
    /// fabricated value — is how the scaffold stays honest per the workspace
    /// `RESPONSIBLE_USE.md` rule.
    #[error("not implemented yet: {0} (this crate is a scaffold — see bead op-v6s)")]
    NotImplemented(&'static str),

    /// A supplied physical input was outside its valid range or otherwise
    /// inconsistent (e.g. a negative porosity, a saturation above 1).
    #[error("invalid physical input: {0}")]
    InvalidInput(String),

    /// A nonlinear (Newton) or linear (Krylov) solve failed to converge within
    /// its iteration/tolerance budget.
    #[error("solver failed to converge: {0}")]
    Convergence(String),
}
