// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT — the OpenFOAM Fuel BEhaviour Analysis Tool
//   Upstream project: foam-for-nuclear (EPFL LRS, PSI LRT, TAMU MEDAL)
//   Source:           https://gitlab.com/foam-for-nuclear/offbeat
//   Upstream commit:  80e84450a115b0c411e1bfa5d166379f6bf6c084 (2026-01-05)
//   Copyright (C) 2018-2026 OFFBEAT contributors
//   Licence:          GPL-3.0
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

//! # OUTRAM PARK fork of OFFBEAT — nuclear fuel-performance
//!
//! A pure-Rust translation of [OFFBEAT](https://gitlab.com/foam-for-nuclear/offbeat),
//! the multi-dimensional finite-volume fuel-behaviour solver, onto the
//! OUTRAM-FOAM finite-volume substrate in `outram-foam-basic-lib`.
//!
//! ## What a fuel-performance code computes
//!
//! Given a fuel rod (or a TRISO particle) and its irradiation history — linear
//! power against time, coolant conditions, fast-neutron flux — it predicts the
//! **thermal and mechanical state of the fuel through life**: temperature
//! distribution, fuel and cladding deformation, the closure of the fuel/cladding
//! gap, the stress in the cladding, the pressure of released fission gas, and
//! ultimately whether the cladding (or a TRISO coating layer) fails.
//!
//! The physics is a strongly coupled loop. Power deposits heat; heat sets the
//! temperature field; temperature drives thermal expansion, creep and swelling;
//! those deformations close the gap; gap closure changes the gap conductance,
//! which changes the temperature field again.
//!
//! ## What belongs in this crate, and what does not
//!
//! **Belongs here:** the fuel-performance physics — solid mechanics
//! ([`mechanics`]), constitutive laws ([`rheology`]), material property
//! correlations ([`materials`]), gap and contact ([`gap`]), burnup accumulation
//! ([`burnup`]), fission-gas release ([`fgr`]) and cladding corrosion
//! ([`corrosion`]).
//!
//! **Does not belong here:** the finite-volume machinery itself. Meshes, fields,
//! `fvc`/`fvm` operators, the LDU matrix and the Krylov solvers all come from
//! `outram-foam-basic-lib` and are used, never re-implemented. Neutron transport
//! and cross-section data live in `outram-mc-libs` and `njoy-outram-park-fork`.
//!
//! ## Units
//!
//! Public constructors and results are typed with `uom` where a caller supplies
//! or consumes a physical quantity. The inner numerical loops carry **raw `f64`
//! in strict SI** — temperature in kelvin, stress in pascal, length in metre,
//! burnup in MWd/kgHM unless a specific item documents otherwise — because the
//! correlations are dense and per-cell, and `uom` round-trips inside a cell loop
//! cost more than they buy. Every such raw-`f64` boundary says so in its doc
//! comment.
//!
//! ## Status
//!
//! **Scaffold / early.** This crate has had no human verification or validation.
//! Per `RESPONSIBLE_USE.md`, AI-assisted output is untrusted draft material until
//! a human reviews it. Nothing here may be described as validated.
//!
//! ## Provenance
//!
//! OFFBEAT is GPL-3.0, the same licence as this workspace. Each ported module
//! keeps an attribution header naming the upstream file it derives from. The
//! upstream tree is **not** vendored into this repository — porting is done from
//! a read-only clone kept outside the working tree.

#![forbid(unsafe_code)]

pub mod error;
pub mod materials;
pub mod mechanics;

pub mod burnup;
pub mod corrosion;
pub mod fgr;
pub mod gap;
pub mod rheology;

pub mod prelude;

pub use error::OffbeatError;
