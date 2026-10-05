// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.

//! **GPU ray tracing of the assembled CSG geometry** (gh:#587): the solid,
//! x-ray and slice pictures of [`crate::csg::plot`], traced on the GPU
//! through the same geometry, so a viewer stays interactive on a geometry as
//! large as HTR-10's explicit pebble bed with TRISO lattices.
//!
//! - [`flat`] — the geometry as one `u32` buffer. Pure Rust, no dependency:
//!   built on every target (Android and wasm included) and unit-tested there.
//! - [`index`] — the grid index `flat` writes for a universe with many
//!   region tokens: per voxel, the cells that reach it, each cut down to the
//!   surfaces passing through the voxel. Pure Rust, every target.
//! - `render` — the `wgpu` compute pipelines and the WGSL tracer. Present
//!   only with the default-on `gpu` feature, off Android (no `wgpu` there)
//!   and off wasm32 (the browser demos do not need it), like [`crate::gpu`].
//!
//! The CPU plotter is the reference and the fallback; the GPU path is `f32`.

pub mod flat;
pub mod index;

#[cfg(all(
    feature = "gpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
pub mod render;
