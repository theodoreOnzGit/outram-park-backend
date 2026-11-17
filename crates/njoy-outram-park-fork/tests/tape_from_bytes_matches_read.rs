//! `Tape::from_bytes` / `Tape::to_text` (the `no_std` tape entry points,
//! gh:#1592) give exactly what `Tape::read` / `Tape::write` give.
//!
//! # Methodology
//!
//! `from_bytes` splits lines itself where `read` uses `BufRead::split(b'\n')`,
//! and `to_text` writes through `core::fmt` into a `String` where `write` goes
//! through `io::Write`. The claim is that neither change alters a tape, so the
//! check is identity, not a tolerance:
//!
//! - **Every ENDF tape in `reference-data/endf/`** (84 ENDF/B-VIII.0 tapes on
//!   2026-11-16) is parsed both ways. The two tapes must have the same TPID,
//!   the same sections in the same order with the same keys, and every row
//!   value must be **bit-identical** (`f64::to_bits`). Then `to_text` of the
//!   one must equal, byte for byte, what `write` writes for the other.
//!   Reference-data tier, so behind `long-tests` like the crate's other tests
//!   on `reference-data/`.
//! - **The line-splitting edge cases** `BufRead::split` defines and
//!   `from_bytes` has to reproduce: empty input, no final newline, a final
//!   newline, a blank last line. Always on, no data needed.
//!
//! # Results
//!
//! 2026-11-16: all 84 tapes identical both ways (sections, bit-identical rows,
//! written text); every edge case agrees. See the issue (gh:#1592) for the run.

#![cfg(feature = "std")]

use njoy_outram_park_fork::endf::tape::Tape;

/// Parse both ways and assert the two tapes, and their written text, agree.
fn assert_same(bytes: &[u8], what: &str) {
    let a = Tape::read(bytes).unwrap_or_else(|e| panic!("{what}: read: {e}"));
    let b = Tape::from_bytes(bytes).unwrap_or_else(|e| panic!("{what}: from_bytes: {e}"));
    assert_eq!(a.sections().len(), b.sections().len(), "{what}: section count");
    for (sa, sb) in a.sections().iter().zip(b.sections()) {
        assert_eq!(sa.key, sb.key, "{what}: section key");
        assert_eq!(sa.rows.len(), sb.rows.len(), "{what}: rows of {:?}", sa.key);
        for (ra, rb) in sa.rows.iter().zip(&sb.rows) {
            let ba: Vec<u64> = ra.iter().map(|x| x.to_bits()).collect();
            let bb: Vec<u64> = rb.iter().map(|x| x.to_bits()).collect();
            assert_eq!(ba, bb, "{what}: a row of {:?}", sa.key);
        }
    }
    let mut written = Vec::new();
    a.write(&mut written).unwrap();
    let text = b.to_text().unwrap();
    assert!(written == text.as_bytes(), "{what}: to_text differs from write");
}

#[test]
fn line_splitting_matches_bufread_split() {
    let line = |mat: &str| format!("{:66}{mat}", "");
    let tpid = " tape id";
    let body = format!("{tpid}\n{}\n{}", line("9237 3  1    1"), line("9237 3  0    2"));
    for (what, text) in [
        ("empty", String::new()),
        ("tpid only, no newline", tpid.to_string()),
        ("tpid only, newline", format!("{tpid}\n")),
        ("no final newline", body.clone()),
        ("final newline", format!("{body}\n")),
        ("blank last line", format!("{body}\n\n")),
    ] {
        // Both may reject a malformed line; then they must reject alike.
        let a = Tape::read(text.as_bytes()).map(|t| t.to_text().unwrap());
        let b = Tape::from_bytes(text.as_bytes()).map(|t| t.to_text().unwrap());
        match (a, b) {
            (Ok(x), Ok(y)) => assert_eq!(x, y, "{what}"),
            (Err(x), Err(y)) => assert_eq!(x.to_string(), y.to_string(), "{what}"),
            (x, y) => panic!("{what}: read {x:?} but from_bytes {y:?}"),
        }
    }
}

#[cfg(feature = "long-tests")]
#[test]
fn every_reference_tape_parses_and_writes_identically() {
    let dir = njoy_outram_park_fork::reference_data::reference_endf_dir();
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "endf"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no ENDF tapes in {}", dir.display());
    for p in &paths {
        let bytes = std::fs::read(p).unwrap();
        assert_same(&bytes, &p.display().to_string());
    }
    println!("{} tapes identical both ways", paths.len());
}
