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

//! # `closures::interfacial` — two-phase geometry & regime closures
//!
//! Rust port of GeN-Foam's `physicsModels/{interfacialAreaModels,
//! fluidDiameterModels, virtualMassModels, dispersionModels,
//! contactPartitionModels, regimeMapModels}`. These are the **geometric and
//! topological** closures of the two-fluid model — how much interface area
//! exists, how big is a bubble/droplet/film, how strongly does an accelerating
//! phase drag its added mass along, how does turbulence spread a dispersed
//! phase, what fraction of a wall does each phase touch, and which named flow
//! regime applies at a given local state. Belongs here: two-phase interface
//! *geometry* and regime *selection*. Does **not** belong here: the drag,
//! heat-transfer, or phase-change *values* themselves (those are `ff_drag`,
//! `fs_drag` (sibling module), `heat_transfer`, `phase_change`) — this module
//! supplies the multipliers and switches those other closures consume.
//!
//! ## Sub-modules (each a closed enum/struct, `match`-dispatched, no `dyn`)
//!
//! | Module | Port of | Public type(s) |
//! |---|---|---|
//! | [`area`] | `interfacialAreaModels/{spherical,annular,NoKazimi,Schor}` | [`area::InterfacialArea`] |
//! | [`diameter`] | `fluidDiameterModels/{isomolarBubble,isothermalBubble,pipeFilm,WallisFilm}` | [`diameter::BubbleDiameter`], [`diameter::FilmDiameter`] |
//! | [`virtual_mass`] | `virtualMassModels/virtualMassCoefficientModels` (`constant` only — see module docs) | [`virtual_mass::VirtualMassCoefficient`] |
//! | [`dispersion`] | `dispersionModels/constant` | [`dispersion::TurbulentDispersion`] |
//! | [`contact_partition`] | `contactPartitionModels/{complementary,linear}` | [`contact_partition::ContactPartition`] |
//! | [`regime_map`] | `regimeMapModels/oneParameter` (`twoParameters` deferred — see module docs) | [`regime_map::RegimeMap1D`] |
//! | [`units`] | (no upstream equivalent) | Local `uom` aliases: `units::InterfacialAreaConcentration`, `units::FluidDiameter` |
//!
//! Each sub-module's own doc comment carries the full methodology, the
//! upstream `.H`/`.C` provenance, and (where a simplification from upstream's
//! OpenFOAM mesh/registry machinery to a pure closure was necessary — e.g.
//! `contactPartitionModels::complementary`'s registry lookup, or
//! `regimeMapModels::twoParameters`'s point-in-polygon engine) an explicit note
//! of what changed and why. See `tests.rs` for the V&V methodology and results
//! (measured 2026-07-15).
//!
//! Tracked by bead op-p6p.7.8; see `docs/genfoam-port-plan.md`.

pub mod area;
pub mod contact_partition;
pub mod diameter;
pub mod dispersion;
pub mod regime_map;
pub mod units;
pub mod virtual_mass;

#[cfg(test)]
mod tests;
