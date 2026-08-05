//! Substitution seam for cross-section data and its feedback update.
//!
//! Reference origin: `sigmavalupd3d.m`, plus the two-group data each benchmark
//! case carries. Proposed substitute: `njoy-outram-park-fork`.
//!
//! This is explicitly a **later** step. The benchmarks supply their own
//! two-group sets, and those sets are part of the benchmark specification: a
//! solve using cross sections generated from evaluated nuclear data is no
//! longer solving the same problem, so it cannot be compared to the reference
//! as a parity check. What `njoy-outram-park-fork` substitutes for is the
//! *feedback interpolation* — how cross sections vary with fuel temperature,
//! moderator density and boron — not the benchmark data itself.
//!
//! **No implementation here yet.**

use super::{Component, Implementation};

/// Where cross sections and their feedback derivatives come from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrossSectionSource {
    /// The stage-1 faithful translation: benchmark-supplied two-group data
    /// with the MATLAB's feedback update.
    #[default]
    Reference,
    /// `njoy-outram-park-fork` standing in for it.
    ///
    /// **Not implemented.** See the module note on why this cannot be a
    /// straight parity comparison against a benchmark-data solve.
    Njoy,
}

impl CrossSectionSource {
    /// The substitution-map entry this source belongs to.
    pub const COMPONENT: Component = Component::CrossSections;

    /// Which of the two paths a call on this source would take.
    #[must_use]
    pub const fn implementation(&self) -> Implementation {
        match self {
            Self::Reference => Implementation::Reference,
            Self::Njoy => Implementation::Substituted,
        }
    }

    /// Whether this source may be used in a solve. See
    /// [`super::channel_flow::ChannelFlowKernel::is_accepted`].
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        match self {
            Self::Reference => true,
            Self::Njoy => Self::COMPONENT.parity_status().is_accepted(),
        }
    }
}
