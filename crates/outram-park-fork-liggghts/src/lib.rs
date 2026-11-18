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
//! > LIGGGHTS-PUBLIC source may be ported into this ~~GPL-3.0-only~~ **AGPL-3.0-only** (relicensed
//! > 2026-10-20, gh:#973) crate.
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
//! ## LIGGGHTS-faithful path (translated from upstream, cross-code verified)
//!
//! Added 2026-09-15. These modules are a **direct translation of
//! LIGGGHTS-PUBLIC** (commit `3d5c00f2`), each carrying its upstream provenance
//! header, and are verified against an upstream run committed under
//! `reference-data/liggghts/` — see `docs/verification-and-validation.md`.
//!
//! - [`granular`] — the contact chain (`surface_model_default` →
//!   `normal_model_hertz`/`hooke` → `tangential_model_history`/`no_history`),
//!   **including the per-contact tangential shear history** that [`contact`]
//!   omits.
//! - [`granular_system`] — the history-aware driver, in LIGGGHTS' step order.
//! - [`integrator`] — `fix nve/sphere` kick–drift–kick velocity-Verlet.
//! - [`timestep`] — `fix check/timestep/gran` Rayleigh and Hertz criteria.
//!
//! **Use these for any packed-bed or settling problem.** [`contact`] hard-codes
//! the tangential displacement to zero, so it has no shear *spring*: a static
//! assembly built on it cannot carry shear, and a heap has zero angle of
//! repose. [`particle::Particle::integrate`] is likewise not symplectic (its
//! own doc comment explains the measured consequences) and should not drive
//! contact dynamics.
//!
//! ## Extensions (clean-room, unit-tested)
//!
//! - [`simulation`] — multi-particle DEM engine: linked-cell neighbor search +
//!   velocity-Verlet ensemble stepping composing [`contact`] + [`boundary`].
//! - [`rolling`] — rolling resistance (Ai et al.) + cohesion (JKR / linear).
//! - [`mesh_wall`] — triangulated (STL-style) walls + moving/rotating boundaries.
//! - [`thermal_radiation`] — grey-body radiation + near-field gas-gap conduction.
//! - [`bonded`] — linear parallel-bond model (Potyondy & Cundall 2004) for
//!   cemented granular material, agglomerates, and TRISO/pebble-matrix bonds.
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Enum dispatch (no `Box<dyn>`), no lifetime parameters (own by value / index
//! ids), `uom`-typed API boundaries, Android-buildable (pure-Rust, no BLAS/GUI).

// `no_std` + `alloc` without the default `std` feature (gh:#1602, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports, unused_variables, unused_mut))]

extern crate alloc;
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
/// `HashMap` / `HashSet`: `std`'s in a `std` build, `hashbrown`'s without
/// (workspace rule, `docs/claude-md/rust-design-rules.md`).
#[allow(unused_imports)]
pub(crate) mod hash {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::collections::{HashMap, HashSet};
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) use hashbrown::{HashMap, HashSet};
}

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use nostd_prelude::*;

pub mod bonded;
pub mod boundary;
pub mod capillary;
pub mod compute;
pub mod contact;
pub mod coupling;
pub mod diagnostics;
#[cfg(feature = "gnn")]
pub mod gnn_bridge;
#[cfg(all(feature = "std", not(target_os = "android")))]
pub mod gpu;
pub mod force_fixes;
pub mod granular;
pub mod granular_system;
pub mod heat_gran;
// A fresh HTR-10 pour from empty, steppable by a UI (gh:#561, dhoby-ghaut Step 1).
pub mod htr10_fill;
pub mod insert;
pub mod insert_stream;
pub mod integrator;
pub mod massflow;
pub mod mesh_dump;
pub mod multicontact;
pub mod multisphere;
pub mod mesh_stress;
pub mod mesh_wall;
pub mod neighbor_list;
pub mod particle;
pub mod prelude;
pub mod rdf;
pub mod rolling;
pub mod simulation;
pub mod superquadric;
pub mod thermal;
pub mod thermal_radiation;
pub mod timestep;
pub mod tracer;
pub mod tri_mesh;
pub mod variable;
/// Serial stand-ins for the `rayon` surface this crate uses, on `wasm32` where
/// `rayon` does not build. Numerically exact here — see the module docs.
#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
pub(crate) mod wasm_par;

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
