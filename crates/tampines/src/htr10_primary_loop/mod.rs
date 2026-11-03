// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! # HTR-10 primary loop and coupled transient (gh:#1151 part A)
//!
//! The 0-D/1-D primary circuit of the HTR-10 (maintainer decision D2,
//! 2026-11-03: `tampines` owns the loop, the bypass branches and the coupled
//! transient), with point kinetics from `teh-o-prke`, and a LOFC ATWS case
//! run against a **lumped core stand-in**.
//!
//! | Module | What |
//! |---|---|
//! | [`primary_loop`] | circulator + coastdown, baffle, hot gas duct, SG, loop momentum with buoyancy |
//! | [`core_model`] | the core plug-in interface ([`CoreModel`], [`CoreBoundary`], [`CoreResponse`]) |
//! | [`lumped_core`] | the stand-in: 5 axial bed zones + cone, reflector, barrel, RPV, RCCS; bypass paths |
//! | [`kinetics`] | PRKE + ANS decay heat + Xe-135 + fuel/moderator/reflector feedback |
//! | [`transient`] | the coupled step, the LOFC case and its named sensitivities, metrics |
//! | [`data`] | embedded copies of the cited / digitised tables |
//!
//! **Part B** (after gh:#1150) adds an R-Z core variant to [`CoreModel`];
//! nothing else changes. The V&V record is
//! `crates/tampines/verification_and_validation/htr10_loop_lofc/README.md`.
//!
//! Inputs: `docs/reactor-scoping/htr10-th-benchmark.md` (the cited input
//! sheet) and `docs/reactor-scoping/htr10-th-data/`. Source keys ([T1],
//! [J09], [C09], [H06], [G02], [S5]) are the sheet's.
//!
//! > Research, education and V&V only. Not for licensing, operation or
//! > safety decisions. AI draft, unreviewed.

pub mod core_model;
pub mod data;
pub mod kinetics;
pub mod lumped_core;
pub mod primary_loop;
pub mod transient;

pub use core_model::{CoreBoundary, CoreModel, CoreResponse};
pub use kinetics::{
    DelayedNeutronSet, FeedbackCoefficients, FeedbackTemperatures, Htr10Kinetics, KineticsParameters,
    ReactivityBreakdown, XenonModel,
};
pub use lumped_core::{BypassSplit, LumpedPebbleBedCore};
pub use primary_loop::{CoastdownModel, FormLoss, PrimaryLoop, PrimaryLoopConfig, SteamGeneratorModel};
pub use transient::{lofc_metrics, run_lofc, CoupledTransient, InitialState, LofcCase, LofcMetrics, TransientSample};

#[cfg(test)]
mod tests;
