//! Substitution seam for delayed-neutron kinetics in the transient path.
//!
//! Reference origin: the delayed-neutron precursor treatment inside
//! `thdiffusion_solvertimexyz.m`. Proposed substitute: `teh-o-prke`.
//!
//! The substitution is not a like-for-like swap and should not be described as
//! one. The reference carries **spatially resolved** precursor concentrations
//! alongside the flux; `teh-o-prke` solves **point** reactor kinetics. They
//! coincide only under a shape assumption, so this gate is as much about
//! stating that assumption as about measuring a difference.
//!
//! **No implementation here yet.**

use super::{Component, Implementation};

/// Which implementation advances the delayed-neutron precursors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KineticsKernel {
    /// The stage-1 faithful translation in `reference::coupling`.
    ///
    /// Spatially resolved precursors, as in the MATLAB.
    #[default]
    Reference,
    /// `teh-o-prke` point kinetics standing in for it.
    ///
    /// **Not implemented.** See the module note: this is a fidelity change, not
    /// only an implementation change.
    TehOPrke,
}

impl KineticsKernel {
    /// The substitution-map entry this kernel belongs to.
    pub const COMPONENT: Component = Component::Kinetics;

    /// Which of the two paths a call on this kernel would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::TehOPrke => Implementation::Substituted,
        }
    }

    /// Whether this kernel may be used in a solve. See
    /// [`super::channel_flow::ChannelFlowKernel::is_accepted`].
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::TehOPrke => Self::COMPONENT.parity_status().is_accepted(),
        }
    }
}
