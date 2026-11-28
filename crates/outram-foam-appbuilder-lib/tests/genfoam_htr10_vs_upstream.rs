// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `reactorCases/3D_HTR-10` against the real GeN-Foam binary (gh:#1876, README round 87)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
#![cfg(feature = "std")]
use std::path::PathBuf;

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, OnePhaseCcCase};

/// The case directory to probe: `GENFOAM_HTR10_CASE` (a copy of the shipped `rootCase`), or `None`.
fn probe_case() -> Option<PathBuf> {
    match std::env::var("GENFOAM_HTR10_CASE") {
        Ok(s) => Some(PathBuf::from(s)),
        Err(_) => None,
    }
}

/// Round 87 inventory probe: the first refusal of the `onePhase` reader on the shipped case (prints; runs only with the env var).
#[test]
fn probe_shipped_first_refusal() {
    let root = match probe_case() {
        Some(r) => r,
        None => return,
    };
    let poly = root.join("constant/fluidRegion/polyMesh");
    match OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()) {
        Ok(c) => println!("READS: {} cells", c.t.len()),
        Err(e) => println!("FIRST REFUSAL: {e}"),
    }
}
