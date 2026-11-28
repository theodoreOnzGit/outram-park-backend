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

// ---------------------------------------------------------------------------------------------------------------------------------------------
// Round 84 (gh:#1938 item 3, gh:#1954): renumbering and unused points against the real binary; zones at later write times; sets/duplicateFaces
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// A `Result` the round 84 tests need: the value, or a panic naming `what` (an explicit `match`, the workspace rule for new code).
fn r84_ok<T, E: core::fmt::Display>(r: Result<T, E>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{what}: {e}"),
    }
}

/// A packed reference (`scripts/pack_start_dir_r66.py`) as `(path, text)`.
fn packed(name: &str) -> Vec<(String, String)> {
    let bytes = gunzip(&vv_dir().join("refs").join(name));
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let eol = at + bytes[at..].iter().position(|&b| b == b'\n').expect("header line");
        let parts: Vec<&str> = r84_ok(std::str::from_utf8(&bytes[at..eol]), "utf8").split('\t').collect();
        let len: usize = r84_ok(parts[2].parse::<usize>(), "length");
        out.push((parts[1].to_string(), r84_ok(String::from_utf8(bytes[eol + 1..eol + 1 + len].to_vec()), "utf8 body")));
        at = eol + 1 + len + 1;
    }
    out
}

fn set_entry(file: &Path, key: &str, value: &str) {
    let text = r84_ok(std::fs::read_to_string(file), "read a case file");
    let mut done = false;
    let out: Vec<String> = text
        .lines()
        .map(|l| {
            if !done && l.starts_with(&format!("{key} ")) {
                done = true;
                format!("{key} {value};")
            } else {
                l.to_string()
            }
        })
        .collect();
    assert!(done, "no entry {key} in {}", file.display());
    r84_ok(std::fs::write(file, out.join("\n") + "\n"), "write a case file");
}

/// Port files `(path, text)` against the packed real ones whose path starts with `prefix`: `(compared, byte-identical, the differing paths)`.
fn byte_compare(port: &[(String, String)], real: &[(String, String)], prefix: &str) -> (usize, usize, Vec<String>) {
    let (mut n, mut same, mut bad) = (0usize, 0usize, Vec::new());
    for (p, text) in real.iter().filter(|(p, _)| p.starts_with(prefix)) {
        n += 1;
        match port.iter().find(|(q, _)| q == p) {
            Some((_, mine)) if mine == text => same += 1,
            Some((_, mine)) => {
                let at = mine.bytes().zip(text.bytes()).position(|(a, b)| a != b).unwrap_or(mine.len().min(text.len()));
                println!("  {p}: first difference at byte {at} (port {} bytes, real {})", mine.len(), text.len());
                bad.push(p.clone());
            }
            None => bad.push(format!("{p} (not written)")),
        }
    }
    for (p, _) in port.iter().filter(|(p, _)| p.starts_with(prefix)) {
        if !real.iter().any(|(q, _)| q == p) {
            bad.push(format!("{p} (extra)"));
        }
    }
    (n, same, bad)
}

/// README round 84, criterion M1 (Y8): the 2D_fullCoupling steady stage for one step on the shipped fluid mesh and on the two edits of
/// `scripts/make_fc_mesh_variant_r84.py` (`swap`: two internal faces out of upper-triangular order; `unused`: an unused point). The port's
/// start-directory files (`baffle_less_start_files`: 8 `polyMesh` files + `sets/duplicateFaces`) and the `1/fluidRegion.baffleLess` of
/// `written_files` after one step, byte for byte against the real ones (`refs/fc_r84_mesh_<v>`).
#[test]
fn r84_fullcoupling_baffle_less_copies_of_a_renumbered_and_an_unused_point_mesh_match_the_real_binary() {
    let mut bad = Vec::new();
    for v in ["base", "swap", "unused"] {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("r84_mesh_{v}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(&vv_dir().join("cases/fullcoupling_three"), &root);
        set_entry(&root.join("system/controlDict"), "endTime", "1");
        if v != "base" {
            let script = vv_dir().join("scripts/make_fc_mesh_variant_r84.py");
            let status = r84_ok(std::process::Command::new("python3").arg("-I").arg(&script).arg(&root).arg(v).status(), "python3");
            assert!(status.success(), "make_fc_mesh_variant_r84.py {v}");
        }
        let real = packed(&format!("fc_r84_mesh_{v}.txt.gz"));
        let mut run = match RegionSolvers::read(&root, CouplingSwitches::default()) {
            Ok(r) => r,
            Err(e) => {
                bad.push(format!("{v}: read refused: {e}"));
                continue;
            }
        };
        let start = r84_ok(run.baffle_less_start_files(&WriteOptions::precision_17()), "start files");
        let (n0, s0, b0) = byte_compare(&start.files, &real, "0/");
        r84_ok(run.step(), "one step");
        assert!(run.is_write_time(), "{v}: step 1 is a write time");
        let later = r84_ok(run.written_files(&WriteOptions::precision_17()), "written files");
        let (n1, s1, b1) = byte_compare(&later.files, &real, "1/fluidRegion.baffleLess/");
        println!("{v}: start directory {s0} of {n0} byte-identical; t = 1 baffle-less {s1} of {n1}; not written {:?}", start.not_written);
        if n0 != 9 || n1 != 8 || !b0.is_empty() || !b1.is_empty() {
            bad.push(format!("{v}: {s0}/{n0} and {s1}/{n1}: {b0:?} {b1:?}"));
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    assert!(bad.is_empty(), "criterion M1: {bad:?}");
}

/// README round 84, criterion M2 (Y9, Y10): the ESFR baffle-less mesh the port BUILDS, WITH the 7 cell zones and 2 face zones `addZones` copies,
/// written at `210` and compared with the real `210/fluidRegion.baffleLess/polyMesh` (eight files); and the start directory's
/// `sets/duplicateFaces` (the `labelHashSet` order) against the real `200/.../sets/duplicateFaces`. `refs/esfr_r84_baffleless_later`. Driven by
/// `ESFR_T_CASE` as round 78 (not run without it). The zone layout is topology: the `210` files are written from the mesh as read at `200`
/// (the fluid mesh does not move), not after 73 steps.
#[test]
fn r84_esfr_baffle_less_zones_at_a_later_write_and_the_duplicate_face_set_match_the_real_binary() {
    let Some(dir) = std::env::var("ESFR_T_CASE").ok().map(PathBuf::from) else {
        println!("ESFR_T_CASE not set: skipped");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("r84_baffleless_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    for d in ["constant", "system", "0", "200"] {
        copy_dir(&dir.join(d), &tmp.join(d));
    }
    let _ = std::fs::remove_dir_all(tmp.join("200/fluidRegion.baffleLess"));
    let run = r84_ok(RegionSolvers::read(&tmp, CouplingSwitches::default()), "read");
    let real = packed("esfr_r84_baffleless_later.txt.gz");
    let start = r84_ok(run.baffle_less_start_files(&WriteOptions::precision_17()), "start files");
    let (n0, s0, b0) = byte_compare(&start.files, &real, "200/fluidRegion.baffleLess/polyMesh/sets/");
    let region = run.region("fluidRegion").expect("fluidRegion");
    let mesh = r84_ok(region.baffle_less.as_ref().expect("removeBaffles region").as_ref(), "build refused");
    println!("zones: {} cell zones, {} face zones ({:?} faces), duplicate set {} faces", mesh.cell_zones.len(), mesh.face_zones.len(), mesh.face_zones.iter().map(|z| z.labels.len()).collect::<Vec<_>>(), mesh.duplicate_face_set.as_ref().map_or(0, Vec::len));
    let files: Vec<(String, String)> = r84_ok(write_poly_mesh(mesh, "210/fluidRegion.baffleLess/polyMesh", &WriteOptions::precision_17()), "write")
        .into_iter()
        .map(|(f, t)| (format!("210/fluidRegion.baffleLess/polyMesh/{f}"), t))
        .collect();
    let (n1, s1, b1) = byte_compare(&files, &real, "210/");
    println!("sets/duplicateFaces: {s0} of {n0} byte-identical {b0:?}; 210: {s1} of {n1} byte-identical {b1:?}");
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(n0 == 1 && b0.is_empty(), "criterion M2: duplicateFaces");
    assert!(n1 == 8 && b1.is_empty(), "criterion M2: the 210 files");
}
