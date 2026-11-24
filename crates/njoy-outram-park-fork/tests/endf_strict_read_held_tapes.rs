// Reads reference files: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **Every held ENDF tape reads under the strict reader** (gh:#660).
//!
//! # Methodology
//!
//! Since gh:#660 [`Tape::read_file`] refuses a card NJOY2016 would stop on: a non-numeric
//! `e11.0` field outside MF=1/MT=451's text lines and MF=32's INTG cards, a malformed INTG
//! integer, or a non-integer MAT/MF/MT/sequence column. A refusal of a tape NJOY reads would
//! be a regression, so this reads every file in `reference-data/endf/` and in NJOY2016's
//! `tests/resources/` (located by `NJOY2016_ROOT`, else `upstream_source/NJOY2016`; skipped
//! with a message when absent). Files that are not ENDF tapes (`.md`, `.py`, `.readme`,
//! `.ace`) are expected to be refused and are skipped.
//!
//! Pass criterion: every other file reads.
//!
//! # Result (2026-11-24)
//!
//! 152 tapes read, 0 refused: 84 in `reference-data/endf/` (its 6 non-ENDF files excluded)
//! and 68 in `tests/resources/` (its `thermalscattering.ace` excluded). Read without the
//! filter, the 7 excluded files are exactly the ones refused. About 6.5 s in release.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_dir;

fn njoy_resources() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
        .join("tests/resources")
}

fn is_tape(p: &Path) -> bool {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    !matches!(ext, "md" | "py" | "readme" | "ace")
}

#[test]
fn every_held_endf_tape_reads_strictly() {
    let mut read = 0usize;
    let mut refused = Vec::new();
    for dir in [reference_endf_dir(), njoy_resources()] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("skip: {} not present", dir.display());
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths.iter().filter(|p| p.is_file() && is_tape(p)) {
            match Tape::read_file(p) {
                Ok(_) => read += 1,
                Err(e) => refused.push(format!("{}: {e}", p.display())),
            }
        }
    }
    println!("{read} tapes read strictly; {} refused", refused.len());
    assert!(refused.is_empty(), "held tapes refused by the strict reader:\n{}", refused.join("\n"));
}
