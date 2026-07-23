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
//! > **⚠️ LICENSING FLAG (see `NOTICE`).** LIGGGHTS/LAMMPS are **GPL-2.0-only**,
//! > incompatible with this workspace's **GPL-3.0-only**. This crate is an
//! > **independent implementation** informed by public DEM literature and by
//! > naming the upstream algorithms — it does **not** copy or translate GPL-2.0
//! > source. Porting actual LIGGGHTS/LAMMPS source is **blocked pending a
//! > maintainer licensing decision**. Phase 1 (below) is generic textbook DEM
//! > and is unaffected.
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
//! - Phase 2 — Contact mechanics (Hooke, Hertz) — planned (`op-t3l.2`).
//! - Phase 3 — Boundaries (Plane, Wall, Box, Cylinder) — planned (`op-t3l.3`).
//! - Phase 4 — Thermal DEM (particle/particle + particle/wall heat transfer) —
//!   planned (`op-t3l.4`).
//! - Phase 5 — Future CFD-DEM coupling (reserve architecture only) — planned
//!   (`op-t3l.5`).
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Enum dispatch (no `Box<dyn>`), no lifetime parameters (own by value / index
//! ids), `uom`-typed API boundaries, Android-buildable (pure-Rust, no BLAS/GUI).

pub mod particle;

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
