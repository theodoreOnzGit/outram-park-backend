//! Substitution seam for critical heat flux.
//!
//! Reference origin: `w3chf.m` / `w3chfhottest.m` — the W-3 correlation, and a
//! hot-channel variant of it. Two candidate substitutes exist in the
//! workspace, `outram-foam-multiphase::chf` and `outram-foam-appbuilder-lib`'s
//! `closures::heat_transfer::chf`; **which of them actually implements W-3**
//! is the open question, and a substitution that quietly swaps in a different
//! correlation is not a parity failure but a physics change.
//!
//! **No implementation here yet.**

use super::{Component, Implementation};

/// Which implementation evaluates the critical heat flux.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChfKernel {
    /// The stage-1 faithful translation of W-3 in `reference::th`.
    #[default]
    Reference,
    /// `outram-foam-multiphase::chf` standing in for it.
    ///
    /// **Not implemented.** Confirm it is W-3 before gating it.
    OutramFoamMultiphase,
    /// `outram-foam-appbuilder-lib` `closures::heat_transfer::chf` standing in
    /// for it.
    ///
    /// **Not implemented.** Confirm it is W-3 before gating it.
    AppbuilderClosure,
}

impl ChfKernel {
    /// The substitution-map entry this kernel belongs to.
    pub const COMPONENT: Component = Component::CriticalHeatFlux;

    /// Which of the two paths a call on this kernel would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::OutramFoamMultiphase | Self::AppbuilderClosure => Implementation::Substituted,
        }
    }

    /// Whether this kernel may be used in a solve. See
    /// [`super::channel_flow::ChannelFlowKernel::is_accepted`].
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::OutramFoamMultiphase | Self::AppbuilderClosure => {
                Self::COMPONENT.parity_status().is_accepted()
            }
        }
    }
}
