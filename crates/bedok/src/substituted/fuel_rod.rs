//! Substitution seam for one-dimensional cylindrical fuel-rod conduction.
//!
//! Reference origin: `fuelrodheat_1dcylnd.m` / `fuelrodheat_1dcylndtime.m` —
//! steady and transient radial conduction in a fuel rod. Two candidate
//! substitutes: `outram-park-fork-offbeat`, which is much the richer model
//! (eigenstrain, gap conductance, fission-gas release), and TUAS's
//! `one_d_solid_structure`, which is closer in scope to the original.
//!
//! Richer is not automatically better for a parity gate: OFFBEAT models effects
//! the reference does not, so agreement is only expected where those effects
//! are switched off. That has to be arranged deliberately rather than hoped
//! for.
//!
//! **No implementation here yet.**

use super::{Component, Implementation};

/// Which implementation solves radial conduction in the fuel rod.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FuelRodKernel {
    /// The stage-1 faithful translation in `reference::th`.
    #[default]
    Reference,
    /// `outram-park-fork-offbeat` standing in for it.
    ///
    /// **Not implemented.** The richer model; see the module note on what that
    /// costs a parity comparison.
    Offbeat,
    /// TUAS `one_d_solid_structure` standing in for it.
    ///
    /// **Not implemented.** Closer in scope to the original.
    TuasSolidStructure,
}

impl FuelRodKernel {
    /// The substitution-map entry this kernel belongs to.
    pub const COMPONENT: Component = Component::FuelRod;

    /// Which of the two paths a call on this kernel would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::Offbeat | Self::TuasSolidStructure => Implementation::Substituted,
        }
    }

    /// Whether this kernel may be used in a solve. See
    /// [`super::channel_flow::ChannelFlowKernel::is_accepted`].
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::Offbeat | Self::TuasSolidStructure => {
                Self::COMPONENT.parity_status().is_accepted()
            }
        }
    }
}
