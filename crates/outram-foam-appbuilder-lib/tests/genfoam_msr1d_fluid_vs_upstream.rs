// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The shipped `1D_MSR_pointKinetics` `fluidRegion` (cyclic `onePhase` loop) against the real binary (gh:#1905, README round 80)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
#![cfg(feature = "std")]
use std::path::{Path, PathBuf};

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, OnePhaseCcCase};

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copy_dir(&s, &d);
        } else {
            std::fs::copy(&s, &d).expect("copy");
        }
    }
}

fn stage(tag: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1905_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases/msr1d_root"), &root);
    std::fs::copy(root.join("constant/multiRegionCouplingDict_TH"), root.join("constant/multiRegionCouplingDict")).expect("coupling dict");
    let poly = root.join("constant/fluidRegion/polyMesh");
    (root, poly)
}

#[test]
fn probe_first_refusal() {
    let (root, poly) = stage("probe");
    match OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()) {
        Ok(c) => println!("READS: {} cells", c.t.len()),
        Err(e) => println!("FIRST REFUSAL: {e}"),
    }
}
