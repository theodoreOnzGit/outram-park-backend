// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The baffle-less mesh the port BUILDS (`removeBaffles`) against the one the real binary wrote (gh:#1236, README round 78)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, criteria and predictions: README "Round 78", committed before each run. `ESFR_T_CASE` is an INFLATED copy of the
//! `3D_SmallESFR_old` transient case (`constant`, `system`, `0`, the written `200`; `scripts/make_esfr_old_case.sh`) from which the real
//! `200/fluidRegion.baffleLess` is REMOVED by this test (a temporary copy is read), so the port cannot read it. The reference is the real
//! binary's own `<start>/fluidRegion.baffleLess/polyMesh` (eight files, `refs/esfr_r78_baffleless_polymesh.ref.txt.gz`, `=== name` markers).
//! Criterion M1: every one of the eight files written by `io::foam_ascii_mesh::write_poly_mesh` is byte-identical. Not run without
//! `ESFR_T_CASE`; the default-run pins of the merge are the unit tests of `genfoam::multi_region::baffle_merge`.
#![cfg(feature = "std")]
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::{CouplingSwitches, RegionSolvers};
use outram_foam_appbuilder_lib::io::foam_ascii::WriteOptions;
use outram_foam_appbuilder_lib::io::foam_ascii_mesh::write_poly_mesh;

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
    decompress_to_vec(&bytes[at..bytes.len() - 8]).expect("inflate")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// M1: the port's built `fluidRegion.baffleLess` mesh against the real binary's, file by file, byte by byte. Prints every number.
#[test]
fn esfr_baffle_less_mesh_is_byte_identical_to_the_real_one() {
    let Some(dir) = std::env::var("ESFR_T_CASE").ok().map(PathBuf::from) else {
        println!("ESFR_T_CASE not set: skipped");
        return;
    };
    // a private copy of the case without the real baffle-less mesh (the port must not be able to read it)
    let tmp = std::env::temp_dir().join(format!("r78_baffleless_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    for d in ["constant", "system", "0", "200"] {
        copy_dir(&dir.join(d), &tmp.join(d));
    }
    let _ = std::fs::remove_dir_all(tmp.join("200/fluidRegion.baffleLess"));
    assert!(!tmp.join("200/fluidRegion.baffleLess").exists());
    let run = RegionSolvers::read(&tmp, CouplingSwitches::default()).unwrap_or_else(|e| panic!("read: {e}"));
    let region = run.region("fluidRegion").expect("fluidRegion");
    let mesh = region.baffle_less.as_ref().expect("fluidRegion is a removeBaffles region").as_ref().unwrap_or_else(|e| panic!("build refused: {e}"));
    println!("BUILT: {} points, {} faces ({} internal), {} cells", mesh.points.len(), mesh.faces.len(), mesh.neighbour.len(), mesh.n_cells);
    let files = write_poly_mesh(&mesh.at_start(), "200/fluidRegion.baffleLess/polyMesh", &WriteOptions::precision_17()).unwrap_or_else(|e| panic!("write: {e}"));
    let reference = String::from_utf8(gunzip(&vv_dir().join("refs/esfr_r78_baffleless_polymesh.ref.txt.gz"))).expect("utf8");
    let mut parts: Vec<(String, String)> = Vec::new();
    for chunk in format!("\n{reference}").split("\n=== ").skip(1) {
        let (name, body) = chunk.split_once('\n').expect("name line");
        parts.push((name.to_string(), body.to_string()));
    }
    // the `\n=== ` separator ate the newline that ended every file but the last: put it back (the last, `points`, keeps its own)
    let last = parts.len() - 1;
    for (i, (_, body)) in parts.iter_mut().enumerate() {
        if i != last {
            body.push('\n');
        }
    }
    assert_eq!(parts.len(), 8, "the reference holds eight files");
    let mut all_equal = true;
    for (name, text) in &files {
        let up = &parts.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no reference for {name}")).1;
        let exact = text == up;
        let same = exact;
        println!("FILE {name}: port {} bytes, real {} bytes, byte-identical {exact}", text.len(), up.len());
        if !same {
            let at = text.bytes().zip(up.bytes()).position(|(a, b)| a != b).unwrap_or(0);
            println!("  first difference at byte {at}: port {:?} real {:?}", &text[at.saturating_sub(20)..(at + 40).min(text.len())], &up[at.saturating_sub(20)..(at + 40).min(up.len())]);
        }
        all_equal &= same;
    }
    assert!(all_equal, "the built mesh differs from the real one (see the FILE lines)");
    let _ = std::fs::remove_dir_all(&tmp);
}
