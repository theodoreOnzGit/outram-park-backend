// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! # HTR-10 coupled thermal-hydraulic transients (gh:#1151)
//!
//! Point kinetics (`teh-o-prke`) coupled to the `tampines` primary loop and
//! its core models: the LOFC ATWS case with its named sensitivity cases.
//! **Moved here from `tampines::htr10_primary_loop` on 2026-11-03**
//! (maintainer: "any neutronics and TH shld live in nee soon"; "Tampines
//! will have fixed or user determined heat sources"): `tampines` keeps the
//! loop and core thermal-hydraulics with the power as an input; everything
//! that computes power from reactivity lives here.
//!
//! | Module | What |
//! |---|---|
//! | [`kinetics`] | PRKE + ANS decay heat + Xe-135 + fuel/moderator/reflector feedback + the start-up neutron source + external reactivity |
//! | [`transient`] | the coupled step, the LOFC and CRW cases (lumped or R-Z core) and their named sensitivities, metrics, thermocouple probes |
//! | [`data`] | embedded copies of the measured traces and the Tab 2.60 reactivity |
//!
//! V&V records: `crates/nee_soon/verification_and_validation/htr10_loop_lofc/README.md`
//! (part A, lumped stand-in) and `.../htr10_rz_transients/README.md`
//! (part B, R-Z core).
//!
//! Inputs: `docs/reactor-scoping/htr10-th-benchmark.md` and
//! `docs/reactor-scoping/htr10-th-data/`. Source keys are the sheet's.
//!
//! > Research, education and V&V only. Not for licensing, operation or
//! > safety decisions. AI draft, unreviewed.

pub mod data;
pub mod kinetics;
pub mod transient;

pub use kinetics::{
    DelayedNeutronSet, FeedbackCoefficients, FeedbackTemperatures, Htr10Kinetics, KineticsParameters,
    NeutronSource, ReactivityBreakdown, XenonModel,
};
pub use transient::{
    c09_thermocouple_positions, crw_metrics, crw_reactivity, lofc_metrics, run_case, run_lofc, CoreChoice,
    CoupledTransient, CrwMetrics, InitialState, LofcCase, LofcMetrics, ProbeRow, TransientSample,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_rz;
