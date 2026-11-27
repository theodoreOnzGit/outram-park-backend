// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # Upstream's `GAMGSolver` is the DEFAULT wherever a case selects `GAMG` (gh:#1947, README round 76)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Pattern of `outram-mc-libs/tests/correct_physics_is_default.rs`: construct through the ordinary path (`Default`) and assert
//! the physics is present, so a default that drifts back to a substitute fails here. Pass criterion: exact equality of the enum.
//! The data-driven half (the shipped 2D_MSFR rootCase read by default holds a native GAMG) is
//! `genfoam_msfr_allrun_vs_upstream::gamg_is_the_default_and_the_allrun_protocol_is_named`.
//!
//! ## Results
//!
//! 2026-11-27, release: see README "Round 76 results". `AllrunEdits::default()` is deliberately NOT `Native`: it is the PROTOCOL of
//! the committed MSFR references (`GAMG` replaced by `PCG`/`DIC` in the real run too); `AllrunEdits::as_shipped_gamg()` is `Native`.

#![cfg(feature = "std")]

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, GamgPolicy};
use outram_foam_appbuilder_lib::genfoam::workflow::allrun::AllrunEdits;

#[test]
fn gamg_policy_defaults_to_upstreams_own_solver() {
    assert_eq!(GamgPolicy::default(), GamgPolicy::Native);
    assert_eq!(CaseReadOptions::default().gamg, GamgPolicy::Native);
}

#[test]
fn allrun_edits_default_is_the_reference_protocol_and_shipped_variant_is_native() {
    assert_eq!(AllrunEdits::default().gamg, GamgPolicy::SubstitutePcgDic);
    assert_eq!(AllrunEdits::as_shipped_gamg().gamg, GamgPolicy::Native);
}
