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

    /// A `.m` file the snapshot references but does not contain.
    ///
    /// The handover is incomplete — `docs/bedok-reference-defects.md` lists
    /// five referenced-but-absent files. Where a translated module's control
    /// flow reaches one of them, this reports which file and from where, rather
    /// than silently producing a default.
    ///
    /// Note the reference itself often *catches* the resulting MATLAB
    /// "undefined function" error and continues on a fallback path; where it
    /// does, the translation reproduces that fallback and surfaces this as a
    /// per-item outcome rather than failing the whole call. See
    /// [`crate::driftflux6_solverstatic3d`].
    #[error("{file} is referenced from {referenced_from} but is absent from the snapshot")]
    ReferenceFileMissing {
        /// The absent `.m` file.
        file: &'static str,
        /// The file that calls it.
        referenced_from: &'static str,
    },

    /// `sigmavalupd3d_handler.m` — defect C1, on a lattice position with no
    /// previous value to inherit.
    ///
    /// The rod-level search leaves `rodlvl` unassigned when a bank's tip sits
    /// at or above the top of its column. Later positions silently reuse the
    /// previous one's value; the **first** has none, and MATLAB raises
    /// `Undefined function or variable 'rodlvl'`. There is no defensible
    /// substitute, so this reports the position rather than inventing one.
    ///
    /// See [`crate::sigmavalupd3d_handler`] for why this case is not exotic:
    /// it is a fully withdrawn bank.
    #[error(
        "control-rod level is undefined at lattice position ({ix}, {iy}) (bank {bank}):          the bank tip is at or above the top of its column and no previous column          has set a level (sigmavalupd3d_handler.m, defect C1)"
    )]
    UninitialisedRodLevel {
        /// The 0-based `x` index of the lattice position.
        ix: usize,
        /// The 0-based `y` index.
        iy: usize,
        /// The control-rod bank number.
        bank: usize,
    },

    /// The critical-boron search produced an eigenvalue outside a sane range.
    ///
    /// `criticalboron_xyz.m` raises `criticalboron_xyz:badeig` whenever a
    /// search eigensolve returns a `k_eff` outside `[0.8, 1.2]`, and
    /// `criticalboron_xyz:badboot` for `[0.5, 1.5]` during the Phase-0
    /// bootstrap. Both abort rather than feeding a garbage value into the
    /// secant — the reference's comment records boron diverging past 1e5 ppm
    /// when an earlier version did not check.
    #[error("critical-boron {phase} returned k_eff = {k_eff} at {boron} ppm, outside the sane range")]
    BoronSearchDiverged {
        /// The offending eigenvalue.
        k_eff: f64,
        /// The boron concentration it was computed at, ppm.
        boron: f64,
        /// Which phase raised it: `"eigensolve"` or `"bootstrap"`.
        phase: &'static str,
    },

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
