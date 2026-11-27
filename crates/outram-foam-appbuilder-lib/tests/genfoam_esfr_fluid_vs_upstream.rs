// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # 3D_SmallESFR fluid region vs the real binary (gh:#1911, README round 71)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
#![cfg(feature = "std")]
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_fc::OnePhaseFcCase;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
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

/// A scratch copy of `cases/<name>` with its `polyMesh/*.gz` inflated in place.
fn stage(name: &str, tag: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1911_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases").join(name), &root);
    let pm = root.join("constant/fluidRegion/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        std::fs::write(pm.join(f), gunzip(&pm.join(format!("{f}.gz")))).expect("polyMesh");
    }
    root
}

#[test]
fn probe_shipped_first_refusal() {
    let e = OnePhaseFcCase::read(&stage("esfr_fluid", "probe")).expect_err("refused");
    println!("FIRST REFUSAL: {e}");
}
