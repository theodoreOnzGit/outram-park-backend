// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! # HTR-10 primary loop thermal-hydraulics (gh:#1151)
//!
//! The 0-D/1-D primary circuit of the HTR-10 and the core models it carries,
//! **thermal-hydraulics only**. ~~(maintainer decision D2, 2026-11-03:
//! `tampines` owns the loop, the bypass branches and the coupled transient),
//! with point kinetics from `teh-o-prke`~~ **CORRECTED again 2026-11-03:
//! maintainer moved coupled kinetics to nee_soon** ("any neutronics and TH
//! shld live in nee soon"; "Tampines will have fixed or user determined heat
//! sources"). The heat source is an **input** here: every core model takes
//! the power from its caller (`CoreModel::advance(dt, power, ..)`); nothing
//! in this module computes power from reactivity. The point kinetics, the
//! coupled transient and the LOFC / CRW cases are
//! `nee_soon::htr10_th_transient`.
//!
//! | Module | What |
//! |---|---|
//! | [`primary_loop`] | circulator + coastdown, baffle, hot gas duct, SG, loop momentum with buoyancy |
//! | [`core_model`] | the core plug-in interface ([`CoreModel`], [`CoreBoundary`], [`CoreResponse`], [`FeedbackTemperatures`]) |
//! | [`lumped_core`] | the stand-in: 5 axial bed zones + cone, reflector, barrel, RPV, RCCS; bypass paths |
//! | [`rz_core`] | the R-Z porous core (`outram-foam-appbuilder-lib::htr10_rz::Htr10RzTransientCore`) behind the loop interface (gh:#1151 part B) |
//! | [`data`] | embedded copies of the cited / digitised tables the loop and the stand-in read |
//!
//! Loop-only V&V note: `crates/tampines/verification_and_validation/htr10_primary_loop/`.
//! The coupled record (part A) is
//! `crates/nee_soon/verification_and_validation/htr10_loop_lofc/README.md`.
//!
//! Inputs: `docs/reactor-scoping/htr10-th-benchmark.md` (the cited input
//! sheet) and `docs/reactor-scoping/htr10-th-data/`. Source keys ([T1],
//! [J09], [C09], [H06], [G02], [S5]) are the sheet's.
//!
//! > Research, education and V&V only. Not for licensing, operation or
//! > safety decisions. AI draft, unreviewed.

pub mod core_model;
pub mod data;
pub mod lumped_core;
pub mod primary_loop;
pub mod rz_core;

pub use core_model::{CoreBoundary, CoreModel, CoreResponse, FeedbackTemperatures};
pub use lumped_core::{BypassSplit, LumpedPebbleBedCore};
pub use primary_loop::{CoastdownModel, FormLoss, PrimaryLoop, PrimaryLoopConfig, SteamGeneratorModel};
pub use rz_core::{RzCoreTimeStep, RzPorousCore};

#[cfg(test)]
mod tests;
