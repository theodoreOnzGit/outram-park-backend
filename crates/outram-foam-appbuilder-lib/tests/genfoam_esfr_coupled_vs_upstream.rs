// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # 3D_SmallESFR coupled (fluid + diffusion + legacy thermo-mechanics) vs the real binary (gh:#1944, README round 74)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, criteria, predictions: README "Round 74 (gh:#1944, 3D_SmallESFR coupled)", committed before each run. The case is staged by
//! `scripts/make_esfr_coupled_case.sh` (the shipped `legacySolver` plus the protocol edits named there); the directory is given by the
//! environment variable `ESFR_COUPLED_CASE` (an INFLATED copy: `INFLATE=1`), because the three 3-D meshes are too large to copy per test.
#![cfg(feature = "std")]
use std::path::PathBuf;

use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::{CouplingSwitches, RegionSolvers};

fn case_dir() -> Option<PathBuf> {
    std::env::var("ESFR_COUPLED_CASE").ok().map(PathBuf::from)
}

/// Round 74 probe: the first refusal (if any) of [`RegionSolvers::read`] on the shipped `legacySolver`. A read, not a run.
#[test]
fn probe_esfr_coupled_first_refusal() {
    let Some(dir) = case_dir() else {
        println!("ESFR_COUPLED_CASE not set: probe skipped");
        return;
    };
    match RegionSolvers::read(&dir, CouplingSwitches::default()) {
        Ok(s) => println!("READS: {} regions, end time {}", ["fluidRegion", "neutroRegion", "thermoMechanicalRegion"].iter().filter(|r| s.region(r).is_some()).count(), s.end_time()),
        Err(e) => println!("FIRST REFUSAL: {e}"),
    }
}
