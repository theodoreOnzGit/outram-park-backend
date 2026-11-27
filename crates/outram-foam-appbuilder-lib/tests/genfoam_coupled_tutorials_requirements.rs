// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The coupled tutorials' unported requirements, pinned (gh:#1879 "U go couple lah", gh:#1361 round 54)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! The shipped `Tutorials/featureCases/2D_fullCoupling` and `Tutorials/reactorCases/2D_FFTF` are read EXACTLY AS SHIPPED
//! (`GENFOAM_UPSTREAM`, default `/home/user/upstream-build/GeN-Foam-src`) by the port's `onePhase` `cellCentered` reader
//! (`OnePhaseCcCase::read_with_options`, `check_region_solvers` off because a `rootCase` has no stage). The reader refuses by name
//! the first selection it does not port; this test pins the FIRST refusal of each case, so the requirements table of README
//! "Round 54" is checkable and cannot go stale silently: when a piece is ported the pinned text changes, the test fails, and the
//! table row is struck through in the same change.
//!
//! ## Results (measured 2026-11-27, release)
//!
//! * `2D_fullCoupling`: the first refusal is `simulationType other than RAS is not ported` (the case is `laminar`).
//! * `2D_FFTF`: the first refusal is `fvSolution: no solver entry matches `h`` (the reader's `h` lookup finds no entry; the case's `h` solver is `PBiCG`/GAMG, not the ported PBiCGStab/diagonal).
//!
//! Pass criterion: the pinned substring is in the refusal. Controls: none (the refusal text is the measurement).

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, GamgPolicy, OnePhaseCcCase};

fn upstream() -> Option<PathBuf> {
    let root = std::env::var("GENFOAM_UPSTREAM").unwrap_or_else(|_| "/home/user/upstream-build/GeN-Foam-src".to_string());
    let p = PathBuf::from(root);
    p.join("Tutorials").is_dir().then_some(p)
}

fn first_refusal(case: &Path) -> Option<String> {
    let opts = CaseReadOptions { check_region_solvers: false, gamg: GamgPolicy::Refuse, ..CaseReadOptions::default() };
    let poly = case.join("constant/fluidRegion/polyMesh");
    OnePhaseCcCase::read_with_options(case, &poly, &opts).err().map(|e| format!("{e}"))
}

#[test]
fn full_coupling_first_refusal_is_pinned() {
    let Some(up) = upstream() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let msg = first_refusal(&up.join("Tutorials/featureCases/2D_fullCoupling/rootCase")).expect("the shipped case is not yet readable");
    eprintln!("2D_fullCoupling first refusal: {msg}");
    assert!(msg.contains("simulationType other than RAS is not ported"), "{msg}");
}

#[test]
fn fftf_first_refusal_is_pinned() {
    let Some(up) = upstream() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let msg = first_refusal(&up.join("Tutorials/reactorCases/2D_FFTF/rootCase")).expect("the shipped case is not yet readable");
    eprintln!("2D_FFTF first refusal: {msg}");
    assert!(msg.contains("no solver entry matches `h`"), "{msg}");
}
