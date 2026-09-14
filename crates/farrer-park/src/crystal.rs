// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! **Crystal plasticity** — plastic flow as crystallographic slip on discrete
//! systems, rather than as an isotropic scalar yield surface.
//!
//! # What this buys over J2
//!
//! [`crate::material::Material::J2`] knows only one internal variable and one
//! yield stress, so every direction of loading is equivalent and every point
//! of a body behaves alike. A crystal has twelve slip systems with a fixed
//! geometry and its own orientation, so:
//!
//! - the yield stress depends on orientation through the Schmid factor, and a
//!   polycrystal is stronger than any of its grains for that reason alone;
//! - neighbouring grains of different orientation deform by different amounts
//!   under the same far-field stress, which is where the intergranular
//!   stresses that drive fatigue crack initiation come from;
//! - slip is resolved *per system*, which is what a fatigue indicator
//!   parameter needs (see [`crate::fatigue`]).
//!
//! # Reading order
//!
//! - [`slip`] — the crystal-frame geometry: plane normals, slip directions,
//!   Schmid tensors, Schmid factors, the latent-hardening `q`-matrix.
//! - [`orient`] — [`Orientation`], the crystal-to-sample rotation, from a
//!   matrix, a Rodrigues vector or Bunge Euler angles.
//! - [`elastic`] — [`CrystalElasticity`], isotropic or cubic, and the
//!   fourth-order rotation into sample axes.
//! - [`flow`] — the rate-dependent power-law flow rule and the saturating
//!   self-and-latent hardening law.
//! - [`update`] — [`CrystalPlasticity`], the material itself, and
//!   [`CrystalState`], the per-quadrature-point history.
//!
//! # What is NOT modelled
//!
//! Stated once here and repeated on [`CrystalPlasticity`], because a reader
//! who misses it will over-trust the output:
//!
//! - **Small strain only.** No elastic/plastic multiplicative split, no
//!   lattice reorientation, no texture evolution. PRISMS-Plasticity, which
//!   this is ported from, is a finite-deformation code; the reduction is
//!   deliberate and is the crate's scope (see the crate `CLAUDE.md`).
//! - **No backstress**, so no Bauschinger effect and no true cyclic
//!   saturation.
//! - **No twinning**, no non-Schmid effects, no BCC `{112}` or `{123}`
//!   families, one phase only.
//!
//! Nothing in this module is validated. It is verified against analytical
//! Schmid factors, geometric invariants, frame indifference, and numerical
//! differentiation of its own tangent — see `docs/verification.md`, cases 12
//! to 17.
//!
//! # Units
//!
//! Stresses and slip resistances in pascals, strains and slips dimensionless,
//! the reference slip rate per second, the time increment in seconds.
//! Orientations and Schmid tensors are dimensionless.

pub mod elastic;
pub mod flow;
pub mod orient;
pub mod slip;
pub mod update;

pub use elastic::CrystalElasticity;
pub use flow::{PowerLawFlow, SaturatingHardening};
pub use orient::Orientation;
pub use slip::{SlipFamily, SlipSystem, MAX_SLIP_SYSTEMS};
pub use update::{CrystalPlasticity, CrystalState};
