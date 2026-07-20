// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::phase` — fluid-phase field state and phase pairs
//!
//! The **fluid-side field-state foundation** of GeN-Foam's porous-medium
//! two-fluid thermal-hydraulics. A porous cell carries, simultaneously, one or
//! more `fluid` phases (each a volume fraction `alpha`, a velocity, an enthalpy,
//! …) and an unresolved solid `structure` occupying the complementary volume.
//! This module owns the **fluid** side of that state; the solid `structure`
//! lives in [`super::structure`] and the correlation leaves in
//! [`super::closures`].
//!
//! Ports upstream
//! `src/classes/thermalHydraulics/src/phaseModels/{phaseBase, fluid}` plus the
//! kinematic core of `physicsModels/phasePairs/{FSPair, FFPair}`.
//!
//! ## Module map
//!
//! | Sub-module | Provides | Upstream |
//! |---|---|---|
//! | [`phase_base`] | [`PhaseBase`] — the shared `alpha` field + name + residual fraction every phase embeds; the [`VolumeFraction`] alias | `phaseBase.{C,H}` |
//! | [`fluid`] | [`Fluid`] — the per-cell fluid field-state bag; [`StateOfMatter`] dispatch | `fluid/fluid.{C,H}` |
//! | [`phase_pair`] | Pairwise [`fs_reynolds`] / [`ff_reynolds`] / [`ff_relative_velocity_magnitude`] — the dimensionless numbers the closures read | `phasePairs/{FSPair,FFPair}.C` |
//!
//! ## What lives here vs. elsewhere
//!
//! This module is deliberately **state, not physics**: [`Fluid`] holds fields
//! and exposes them; the thermo package ([`super::thermophysical`]), turbulence
//! ([`super::closures::turbulence`]), drag/heat-transfer closures, and the
//! porous solver ([`super::solver`]) update those fields by mutable reference.
//! The [`phase_pair`] helpers port only the *kinematic* dimensionless numbers
//! (Reynolds, relative velocity); the drag tensor `Kd` and heat-transfer
//! coefficient assembly that the full upstream `FSPair`/`FFPair` also perform
//! are closure/solver responsibilities and are **not** here.
//!
//! ## Dispatch: no trait objects
//!
//! Per the workspace no-`dyn` rule, phase classification is a closed enum
//! ([`StateOfMatter`]) dispatched by value, not runtime polymorphism. The
//! fluid-vs-structure distinction that upstream expresses through the
//! `phaseBase` base class is handled by the solver holding a [`Fluid`] and a
//! structure directly (both embed a [`PhaseBase`]); a unifying `Phase` enum, if
//! one is wanted, belongs at the solver level once [`super::structure`] lands,
//! since it must name both concrete types.

pub mod fluid;
pub mod phase_base;
pub mod phase_pair;

pub use fluid::{Fluid, StateOfMatter};
pub use phase_base::{volume_fraction, PhaseBase, VolumeFraction};
pub use phase_pair::{ff_relative_velocity_magnitude, ff_reynolds, fs_reynolds};
