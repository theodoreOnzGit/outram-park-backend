// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation of granular-DEM physics. NOT a translation of
// GPL-2.0 LIGGGHTS/LAMMPS source — see the crate NOTICE for the GPL-2 vs GPL-3
// licensing flag (maintainer decision, bead op-t3l).
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # outram-park-fork-liggghts
//!
//! Independent, pure-Rust **granular discrete-element-method (DEM)** library for
//! OUTRAM PARK (bead epic `op-t3l`): particles, contact mechanics, thermal DEM,
//! and pebble-/packed-bed physics — the DEM/granular pillar of the Phase II
//! architecture (kept separate from the thermophysical-property pillar
//! [`tampines`] and the CFD/multiphase pillar [`outram-foam-multiphase`], with
//! CFD-DEM coupling deferred to a future explicit seam).
//!
//! > **Licensing (see `NOTICE`).** LIGGGHTS-PUBLIC's source headers declare
//! > **"GNU Public License, version 2 or later"**, which **is compatible with
//! > GPL-3.0** (the "or later" option permits use under GPLv3) — so
//! > LIGGGHTS-PUBLIC source may be ported into this GPL-3.0-only crate.
//! > (Correcting an earlier note that wrongly said "GPL-2.0-only / blocked".)
//! > When porting, confirm the specific file's "or later" header and keep its
//! > attribution + provenance. LAMMPS-proper headers are version-unspecified
//! > (murkier) — treat those with care. Phase 1 below is clean-room from public
//! > DEM literature (no upstream-derived code) regardless.
//!
//! > **⚠️ Unverified until validated — scaffold.** No human V&V yet. Not for
//! > nuclear facility operation, reactor control, safety-critical, or licensing
//! > decisions.
//!
//! ## Roadmap (each physics bead's DoD: theory docs + verification tests +
//! ## reference-benchmark comparison + unit-safe `uom`)
//!
//! - **Phase 1 — Particle framework** ([`particle`]) — `Particle { position,
//!   velocity, angular_velocity, mass, radius, temperature }` + explicit time
//!   integration. **In progress** (bead `op-t3l.1`).
//! - **Phase 2 — Contact mechanics** ([`contact`]) — Hooke + Hertz-Mindlin
//!   normal/tangential contact (enum dispatch). Foundation done (`op-t3l.2`).
//! - **Phase 3 — Boundaries** ([`boundary`]) — Plane / Wall / Box / Cylinder
//!   signed-distance + particle overlap. Foundation done (`op-t3l.3`).
//! - **Phase 4 — Thermal DEM** ([`thermal`]) — particle/particle + particle/wall
//!   contact conduction + temperature integration. Foundation done (`op-t3l.4`).
//! - **Phase 5 — CFD-DEM coupling** ([`coupling`]) — reserved architecture only
//!   (interfaces defined, no physics). Done as reserved (`op-t3l.5`).
//!
//! Phases 2-4 are **clean-room, unit-tested foundations, not benchmark-validated**
//! (that is a later human step) — see each module's "Honest scope".
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Enum dispatch (no `Box<dyn>`), no lifetime parameters (own by value / index
//! ids), `uom`-typed API boundaries, Android-buildable (pure-Rust, no BLAS/GUI).

pub mod boundary;
pub mod contact;
pub mod coupling;
pub mod particle;
pub mod thermal;

/// Errors produced by the DEM library in this crate.
#[derive(Debug, thiserror::Error)]
pub enum DemError {
    /// A model input was outside its valid physical range (e.g. non-positive mass/radius).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// A requested feature is scaffolded but not yet implemented.
    #[error("not yet implemented: {0}")]
    NotImplemented(String),
}
