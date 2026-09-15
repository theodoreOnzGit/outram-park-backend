//! Error type for the translation.
//!
//! # Why this module exists
//!
//! No `.m` counterpart. The reference signals failure with `error(...)`, which
//! aborts the whole run; the closest faithful Rust is a `Result` carrying the
//! same condition and message. Where the reference *prints* rather than errors
//! (`convertsparsekey3d.m`'s debug block, for instance), the translation logs
//! and continues — the behaviour, not the mechanism, is what is preserved.

use thiserror::Error;

/// Failure conditions raised by the translated reference.
///
/// Each variant names the `.m` file whose `error(...)` call it stands in for,
/// so a failure can be traced back to the MATLAB line that produced it.
#[derive(Debug, Error)]
pub enum BedokError {
    /// `pauseonnan.m` — `error('NaN occured')`.
    ///
    /// The reference's spelling of the message is preserved deliberately; it is
    /// what a user grepping the MATLAB will search for.
    #[error("NaN occured")]
    NanEncountered,

    /// `pauseonnan.m` — `error('Unexpected  complex number')`.
    ///
    /// The doubled space is in the reference and is kept. This variant is
    /// currently unreachable: the translation works in `f64` throughout, so
    /// there is no complex value for `~isreal` to detect. It exists to record
    /// that the reference performs the check.
    #[error("Unexpected  complex number")]
    UnexpectedComplex,

    /// A coordinate branch in `handle2dcoords.m` / `handle3dcoords.m` matched
    /// no populated field set, leaving the reference's outputs unassigned.
    #[error("no coordinate branch matched: none of (maxir, maxitheta, maxiz), (maxix, maxiy, maxiz) or (maxi1, maxi2, maxi3) is fully populated")]
    NoCoordinateBranch,

    /// The flux solvers' preconditioned-GMRES branch, which is **not
    /// translated**.
    ///
    /// This is the one place the translation declines to reproduce the
    /// reference rather than reproducing it faithfully, so it is worth being
    /// precise about the scope.
    ///
    /// `diffusion_solverxyz.m` and `sanodaldiffusion_solverxyz.m` both switch
    /// from a direct factorisation to `gmres(LHS, RHS, 100, tol, 20, L, U, x0)`
    /// with an `ilu` preconditioner once `philenf >= 50_000_000`. Reaching that
    /// needs 50 million unknowns — for two energy groups, a mesh of 25 million
    /// nodes — whose sparse operators alone would not fit in the memory of any
    /// machine this code runs on. The branch is unreachable for every case in
    /// the snapshot and for anything a user could plausibly build.
    ///
    /// Translating it would mean writing an ILU factorisation and a restarted
    /// GMRES that could never be exercised, and therefore never verified,
    /// against the reference. An explicit error is the honest alternative: it
    /// is visible, it names the threshold, and it cannot be mistaken for the
    /// direct path having run.
    #[error(
        "the gmres/ilu branch of the flux solvers is not translated; \
         philenf = {philenf} is at or above the reference's sizethresh of {threshold}"
    )]
    IterativeSolveNotTranslated {
        /// The problem size that selected the branch.
        philenf: usize,
        /// The reference's `sizethresh`.
        threshold: usize,
    },
}

/// Result alias for the translated reference.
pub type Result<T> = std::result::Result<T, BedokError>;
