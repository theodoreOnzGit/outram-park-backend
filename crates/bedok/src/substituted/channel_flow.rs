//! Substitution seam for single-phase channel flow with evaporation.
//!
//! Reference origin: `singleflow1devap.m` / `singleflow1devaptime.m` — a 1-D
//! single-phase channel with evaporation, steady and transient. Proposed
//! substitute: `tuas_boussinesq_solver` (fluid-array machinery) composed
//! through `tampines`.
//!
//! **No implementation here yet.** [`ChannelFlowKernel::Tuas`] names the
//! substitution and carries its parity state; the physics arrives with the
//! gate, not before.

use super::{Component, Implementation};

/// Which implementation performs the single-phase channel-flow solve.
///
/// Enum dispatch, not a trait object: the set of channel-flow implementations
/// is closed, so adding one is a compile error at every `match` that has not
/// been updated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChannelFlowKernel {
    /// The stage-1 faithful translation in `reference::th`.
    ///
    /// The default, and the oracle every substitution is measured against.
    #[default]
    Reference,
    /// `tuas_boussinesq_solver` / `tampines` standing in for it.
    ///
    /// **Not implemented.** Selecting it today only records an intent; there is
    /// no code behind it and it has not passed a parity gate. The open physics
    /// question is whether TUAS's fluid-array formulation reproduces the
    /// MATLAB's evaporation treatment, which is where the two models most
    /// plainly differ.
    Tuas,
}

impl ChannelFlowKernel {
    /// The substitution-map entry this kernel belongs to.
    pub const COMPONENT: Component = Component::ChannelFlow;

    /// Which of the two paths a call on this kernel would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::Tuas => Implementation::Substituted,
        }
    }

    /// Whether this kernel may be used in a solve.
    ///
    /// The reference is accepted by definition — it defines parity. A
    /// substitute is accepted only once [`Component::parity_status`] records a
    /// measured pass.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::Tuas => Self::COMPONENT.parity_status().is_accepted(),
        }
    }
}
