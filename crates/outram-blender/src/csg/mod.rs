// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.

//! **Constructive solid geometry (CSG) description and its pure navigation
//! kernel** — the geometry a Monte Carlo code tracks through.
//!
//! Moved here from `outram-mc-libs` (`src/geometry/`) on 2026-10-02, GitHub
//! issue #486, by maintainer decision: outram-blender owns geometry
//! **description** and the **pure** queries on it (surface evaluate, sense,
//! distance and normal; cell membership and boundary distance; universe
//! search; geometry location and boundary distance; lattice indices, local
//! positions and distances), so the geometry plotter and the transport code
//! share one locator and "draw what the solver sees" holds by construction.
//! `outram-mc-libs` keeps the **transport-state** work (surface crossing,
//! nudging, corner and diffuse reflection, cross-section lookups, distribcell,
//! virtual lattices, volume calculation, GPU encoders, tally scoring), reaches
//! it on these types through `*Ext` traits, and re-exports everything here
//! under its old `outram_mc_libs::geometry::*` paths.
//!
//! The code is a port of OpenMC's geometry (MIT licence; see `NOTICE` and
//! `LICENSE.openmc`). Units are raw `f64` **centimetres**, as in OpenMC and
//! outram-mc-libs: this is the inner loop of particle tracking, so the
//! workspace `uom` rule is deliberately not applied here (documented in
//! `crates/outram-mc-libs/CLAUDE.md`, "Units: raw `f64`, not `uom`").
//!
//! This module is in the crate's **core**: it needs no cargo feature and pulls
//! no dependency, so `default-features = false` builds it for Android and
//! `wasm32-unknown-unknown`. The one exception is [`gpu`]'s `render`
//! submodule (gh:#587, 2026-10-05), the GPU ray tracer, which needs the
//! default-on `gpu` feature and is never built on Android or wasm32;
//! [`gpu::flat`], its geometry encoding, is core like the rest.

pub mod cell;
pub mod geometry;
pub mod gpu;
pub mod lattice;
pub mod plot;
pub mod position;
pub mod surface;
pub mod triso_particle;
pub mod universe;
