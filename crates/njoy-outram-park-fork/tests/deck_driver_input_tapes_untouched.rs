// SPDX-License-Identifier: AGPL-3.0-only
//! **The deck driver never writes to an input tape** (gh:#1508).
//!
//! # Methodology
//!
//! Upstream NJOY2016 opens a module's output units with `openz(lun,1)`
//! (`status='new'`) and its inputs with `openz(lun,0)` (`status='old'`,
//! `util.f90:199-212`), and never writes to a unit it only reads. Before
//! gh:#1508, `TapeUnits::write_coded_files` wrote back every coded unit at the
//! end of a deck, the input evaluation `tape20` included (deck 24: Pu-239 grew
//! from 23.8 MB to 25.3 MB).
//!
//! A small deck runs through [`run_deck`] in a temporary work directory on
//! `reference-data/endf/n-001_H_002-ENDF8.0.endf` (MAT 128) as `tape20`:
//!
//! ```text
//! reconr 20 21     (coded input, coded output PENDF)
//! moder  21 -22    (coded to blocked binary, kept in memory)
//! moder -22 23     (binary back to coded)
//! ```
//!
//! **Pass criterion:** `tape20`'s bytes after the run are identical to its
//! bytes before (the whole file is compared, which is stronger than a hash);
//! the output units `tape21` and `tape23` are written and non-empty, and
//! `tape22` (binary, no blocked-binary writer) is not. The test also checks that
//! it can fail: re-writing the evaluation through `Tape::write`, as the old
//! driver did, must change its bytes.
//!
//! # Results (2026-11-12)
//!
//! Passes. `tape20` is byte-identical after the run. `tape21` and `tape23` are
//! written. The evaluation's round trip through `Tape::write` changes its bytes,
//! so the pre-fix driver would have failed this test.

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

const DECK: &str = "reconr
20 21/
'h-2 pendf for gh:1508'/
128 0/
.001/
0/
moder
21 -22/
moder
-22 23/
stop
";

#[test]
fn input_tape_bytes_are_unchanged_and_outputs_are_written() {
    let Some(src) = reference_endf_or_skip("n-001_H_002-ENDF8.0.endf", "deck-input-untouched") else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("njoy-deck-1508-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(&src, dir.join("tape20")).unwrap();
    let before = std::fs::read(dir.join("tape20")).unwrap();

    // The test can fail: the old write-back changes this evaluation's bytes.
    let mut rewritten = Vec::new();
    Tape::read_file(&dir.join("tape20")).unwrap().write(&mut rewritten).unwrap();
    assert_ne!(rewritten, before, "a rewrite through Tape::write leaves this tape unchanged, so the test could not fail");

    let run = run_deck(DECK, &DeckOptions { workdir: Some(dir.clone()), parse_only: false, run_date: None }).unwrap();
    assert!(run.blocked.is_none(), "deck blocked: {:?}", run.blocked);

    let after = std::fs::read(dir.join("tape20")).unwrap();
    assert!(after == before, "input tape20 was modified ({} -> {} bytes)", before.len(), after.len());
    assert!(!run.units.is_output(20), "tape20 is an input");
    for u in [21, 23] {
        assert!(run.units.is_output(u), "tape{u} is an output");
        let len = std::fs::metadata(dir.join(format!("tape{u}"))).map(|m| m.len()).unwrap_or(0);
        assert!(len > 0, "output tape{u} not written");
    }
    assert!(!dir.join("tape22").exists(), "binary unit 22 has no blocked-binary writer");
    std::fs::remove_dir_all(&dir).unwrap();
}
