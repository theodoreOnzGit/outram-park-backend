//! Substitution seam for six-equation drift-flux two-phase flow.
//!
//! Reference origin: `driftflux6_solverstatic3d.m`. Proposed substitute:
//! `outram-foam-multiphase::drift_flux`, which exists but whose fidelity match
//! to Yan Ren's formulation is unverified — the two may not close the same set
//! of six equations, which is the first thing a parity attempt must establish.
//!
//! **No implementation here yet.**

use super::{Component, Implementation};

/// Which implementation performs the drift-flux two-phase solve.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DriftFluxKernel {
    /// The stage-1 faithful translation in `reference::th`.
    #[default]
    Reference,
    /// `outram-foam-multiphase::drift_flux` standing in for it.
    ///
    /// **Not implemented.** Selecting it today only records an intent.
    OutramFoamMultiphase,
}

impl DriftFluxKernel {
    /// The substitution-map entry this kernel belongs to.
    pub const COMPONENT: Component = Component::DriftFlux;

    /// Which of the two paths a call on this kernel would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::OutramFoamMultiphase => Implementation::Substituted,
        }
    }

    /// Whether this kernel may be used in a solve. See
    /// [`super::channel_flow::ChannelFlowKernel::is_accepted`].
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::OutramFoamMultiphase => Self::COMPONENT.parity_status().is_accepted(),
        }
    }
}
