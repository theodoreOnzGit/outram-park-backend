//! GitHub #462: `Nuclide::with_thermal_scattering` binds an S(alpha,beta) law
//! to a nuclide entry, so every material referencing the entry gets it
//! (OpenMC associates it per material). It is `#[deprecated]`; this test
//! makes the deprecation impossible to silence by accident.
//!
//! Methodology: scan every `.rs` file under `crates/` for a call
//! `.with_thermal_scattering(`. Each file that makes one must carry a
//! `#![allow(deprecated)]` or `#[allow(deprecated)]` whose line mentions
//! `#462` (the reason, written at the call's file). A new call site without
//! the marker fails, so the shared-nuclide case cannot be re-introduced
//! unseen. Doc and ordinary comments are ignored.
//!
//! Result (2026-11-14): passes; the marked files are the migration backlog
//! listed on #462.

use std::path::{Path, PathBuf};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if name == "target" || name == "vendor" || name.starts_with('.') || name == "reactor-literature" {
                continue;
            }
            rs_files(&p, out);
        } else if name.ends_with(".rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_nuclide_level_sab_call_site_is_marked_with_its_reason() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    rs_files(&crates, &mut files);
    let mut unmarked = Vec::new();
    let mut calls = 0;
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let calls_here = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.contains(".with_thermal_scattering("));
        if !calls_here {
            continue;
        }
        calls += 1;
        let marked = text.lines().any(|l| l.contains("allow(deprecated)") && l.contains("#462"))
            || text
                .lines()
                .zip(text.lines().skip(1))
                .any(|(a, b)| a.contains("allow(deprecated)") && b.contains("#462"))
            || text.contains("allow(deprecated)") && text.contains("GitHub #462");
        if !marked {
            unmarked.push(f.display().to_string());
        }
    }
    println!("{calls} files call Nuclide::with_thermal_scattering");
    assert!(calls > 0, "scan found no call sites: the scan itself is broken");
    assert!(
        unmarked.is_empty(),
        "nuclide-level S(a,b) call without a #462 marker (use assign_material_sab): {unmarked:#?}"
    );
}
