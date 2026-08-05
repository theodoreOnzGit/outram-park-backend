//! Substitution seam for the sparse linear solve behind the diffusion
//! equation.
//!
//! Reference origin: MATLAB's `\`, `decomposition()` and `gmres`. The stage-1
//! translation uses a **direct** sparse LU (`faer`) because that is what those
//! do, and stage 1 must match them.
//!
//! Proposed substitute: `outram-foam-basic-lib`'s `ldu_matrix` and `krylov`,
//! which offer **iterative** solvers only — conjugate gradient, GAMG,
//! Gauss-Seidel.
//!
//! # Why this row is the awkward one
//!
//! Two things make it so, and both are properties of the problem rather than
//! of anyone's code:
//!
//! - **A direct factorisation and an iterative solve do not agree bit for
//!   bit.** The iterative answer is only as converged as its own tolerance, so
//!   the parity tolerance for this component has to be set from the physics —
//!   how much `k_eff` movement is acceptable — not from machine epsilon. This
//!   is the substitution most likely to move results while being entirely
//!   correct.
//! - **The diffusion left-hand side is non-symmetric.** `gradD + nodal +
//!   sigma.tot - sigma.s` picks up asymmetry from down-scattering, so
//!   conjugate gradient does not apply to it unmodified. A substitution must
//!   choose a method that tolerates that, and say which.
//!
//! **No implementation here yet.**

use super::{Component, Implementation};

/// Which linear solver factorises or iterates the diffusion system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LinearSolverKernel {
    /// The stage-1 direct sparse LU (`faer`), matching MATLAB's `\`.
    #[default]
    Reference,
    /// `outram-foam-basic-lib` `ldu_matrix` / `krylov` standing in for it.
    ///
    /// **Not implemented.** Iterative; see the module note on the tolerance and
    /// symmetry consequences.
    OutramFoamKrylov,
}

impl LinearSolverKernel {
    /// The substitution-map entry this kernel belongs to.
    pub const COMPONENT: Component = Component::LinearSolver;

    /// Which of the two paths a call on this kernel would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::OutramFoamKrylov => Implementation::Substituted,
        }
    }

    /// Whether this kernel may be used in a solve. See
    /// [`super::channel_flow::ChannelFlowKernel::is_accepted`].
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::OutramFoamKrylov => Self::COMPONENT.parity_status().is_accepted(),
        }
    }
}
