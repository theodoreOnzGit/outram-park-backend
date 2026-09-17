// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/contactPartitionModels/
//             {complementary/complementaryContactPartition.{H,C},
//             linear/linearContactPartition.{H,C}}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::contact_partition` — fluid-structure wall-contact partition
//!
//! Port of GeN-Foam's `contactPartitionModel` family: the fraction of a
//! structure's wetted wall area attributed to a given fluid phase (e.g. what
//! fraction of a rod's surface is "wetted by liquid" vs. "in contact with
//! vapour film"), used to split fluid-structure drag/heat-transfer between the
//! phases of a boiling or film-flow scenario.
//!
//! ## Model set (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Contact fraction |
//! |---|---|---|
//! | [`ContactPartition::Linear`] | `linear` | Equal to the phase's own (pair-)normalized void fraction |
//! | [`ContactPartition::Complementary`] | `complementary` | `1 -` the other phase's contact fraction |
//!
//! ## The `complementary` registry lookup, faithfully simplified
//!
//! Upstream's `complementary::value()` does not compute anything itself — on
//! first call it searches the mesh's `objectRegistry` for *the other*
//! `contactPartitionModel` instance registered for the same structure (there
//! are always exactly two fluids in an `FSPair`, so exactly one is `linear` and
//! the other `complementary`) and returns `1 - thatModel->value(celli)`. That
//! registry lookup is OpenFOAM mesh-object-registry plumbing, not algebra, and
//! is out of scope for a pure closure (the solver bead that owns the fluid
//! registry is the natural place to wire "find my complementary pair's
//! partition model"). This port keeps the physics — `1 - other` — and takes the
//! other model's already-evaluated fraction as an argument
//! ([`ContactPartition::Complementary`]'s `value` takes `other_fraction`
//! instead of re-deriving it), which is exact for any registry wiring a caller
//! chooses.

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Fluid-structure wall-contact partition fraction — dimensionless, `0..=1`.
///
/// Closed enum port of GeN-Foam's `contactPartitionModel` family. Evaluate
/// with [`ContactPartition::value`]. See the module docs for the
/// `complementary` registry-lookup simplification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactPartition {
    /// Contact fraction equal to the phase's own (pair-)normalized void
    /// fraction `alphaN` (i.e. the structure's wall is assumed wetted in exact
    /// proportion to how much of the fluid volume this phase occupies).
    Linear,
    /// Contact fraction `1 -` the complementary phase's fraction (see the
    /// module docs for why this takes the other fraction as an input rather
    /// than a mesh-registry lookup).
    Complementary,
}

impl ContactPartition {
    /// Evaluate the contact partition fraction.
    ///
    /// For [`ContactPartition::Linear`], `input` is this phase's own
    /// (pair-)normalized void fraction `alphaN`. For
    /// [`ContactPartition::Complementary`], `input` is the complementary
    /// phase's own already-evaluated contact fraction.
    #[must_use]
    pub fn value(&self, input: Ratio) -> Ratio {
        Ratio::new::<ratio>(self.value_bare(input.get::<ratio>()))
    }

    fn value_bare(&self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Complementary => 1.0 - x,
        }
    }
}
