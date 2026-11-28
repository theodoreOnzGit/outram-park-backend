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
//! * `2D_fullCoupling`: ~~the first refusal is `simulationType other than RAS is not ported` (the case is `laminar`)~~ **CHANGED
//!   2026-11-27 (gh:#1913, README round 55)**: rows 1-5 are ported (laminar, polynomial thermo, Nusselt `htc`, passive structure, the
//!   `h` solvers) and the shipped `fluidRegion` is READ AS SHIPPED with the one named deviation `GAMG` -> `PCG`/`DIC`
//!   ([`GamgPolicy::SubstitutePcgDic`]); with `GamgPolicy::Refuse` the first refusal is the `GAMG` one (row 8).
//! * `2D_FFTF`: ~~the first refusal is `fvSolution: no solver entry matches `h`` (the reader's `h` lookup finds no entry; the case's `h`
//!   solver is `PBiCG`/GAMG, not the ported PBiCGStab/diagonal)~~ **CHANGED 2026-11-27 (gh:#1870, README round 62)**: the regex keys
//!   `"(h|k|epsilon)"`, `PBiCG`/`DILU`, the pump `momentumSourceTimeProfile`, `cellCenteredFaceReconstruction`, `Churchill`/`Rehme` and
//!   per-axis drag and `noSlip` are ported; with the named `GAMG` substitution the first refusal is
//!   ~~`structureProperties.IHXPrimary:IHXSecondary.localTortuosity is not ported` (the tensor-diffusivity Laplacian of `UEqn_1p.H`/`EEqn_1p.H`)~~
//!   **CHANGED 2026-11-27 (gh:#1229, README round 64)**: `localTortuosity` is ported (`two_phase::tensor_laplacian`, differenced on `w7_tort`);
//!   the first refusal was `nut.vesselWall: `zeroGradient` is not ported` (the turbulence reader's wall-function-only `nut` patches;
//!   `porousKEpsilon` with `zeroGradient` wall entries). ~~that refusal~~ **CHANGED 2026-11-27 (gh:#1929, README round 65)**: the
//!   shipped fluid region is READ with no refusal and RUN by `genfoam_fftf_pieces_vs_upstream::fftf_steady_state_*`; `GamgPolicy::Refuse`
//!   still refuses `p_rgh` GAMG by name. **gh:#1947 (round 76):** every read here names its `GamgPolicy`; the DEFAULT is now `Native`
//!   (~~`Refuse`~~), so the refusal is an explicit ablation, as the `gamg: GamgPolicy::Refuse` fields say.
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
fn full_coupling_is_read_as_shipped_with_the_gamg_substitution() {
    let Some(up) = upstream() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let case = up.join("Tutorials/featureCases/2D_fullCoupling/rootCase");
    // row 8: with GAMG refused the first refusal is the GAMG one, no longer the `laminar` one
    let msg = first_refusal(&case).expect("GAMG is refused by name");
    eprintln!("2D_fullCoupling first refusal with GamgPolicy::Refuse: {msg}");
    assert!(msg.contains("GAMG"), "{msg}");
    let opts = CaseReadOptions { check_region_solvers: false, gamg: GamgPolicy::SubstitutePcgDic, ..CaseReadOptions::default() };
    let c = OnePhaseCcCase::read_with_options(&case, &case.join("constant/fluidRegion/polyMesh"), &opts).expect("the shipped fluidRegion is readable");
    assert!(c.laminar && c.passive.is_some());
    assert!(c.flow.as_ref().is_some_and(|f| f.p_rgh_gamg_substituted));
}

#[test]
fn fftf_is_read_and_gamg_is_refused_by_name() {
    let Some(up) = upstream() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let case = up.join("Tutorials/reactorCases/2D_FFTF/rootCase");
    // the GAMG substitution is the named deviation of the whole workspace (rounds 17, 21, 44)
    let opts = CaseReadOptions { check_region_solvers: false, gamg: GamgPolicy::SubstitutePcgDic, ..CaseReadOptions::default() };
    // ~~first refusal `nut.vesselWall: `zeroGradient` is not ported`~~ **CHANGED 2026-11-27 (gh:#1929, README round 65)**: the shipped
    // fluid region is READ with no refusal (`porousKEpsilon` with `zeroGradient`/`calculated` patches, `mappedWall` as plain walls).
    let case_read = OnePhaseCcCase::read_with_options(&case, &case.join("constant/fluidRegion/polyMesh"), &opts).expect("the shipped FFTF fluidRegion is readable");
    eprintln!("2D_FFTF is read: {} cells", case_read.alphat.len());
    assert_eq!(case_read.alphat.len(), 18_538);
    // as shipped (GAMG): refused by name, GAMG is not ported
    let refused = CaseReadOptions { check_region_solvers: false, gamg: GamgPolicy::Refuse, ..CaseReadOptions::default() };
    let msg = OnePhaseCcCase::read_with_options(&case, &case.join("constant/fluidRegion/polyMesh"), &refused).err().map(|e| format!("{e}")).expect("GAMG is refused");
    assert!(msg.contains("GAMG"), "{msg}");
}
