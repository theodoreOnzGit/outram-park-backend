//! Packaging guard: **no raw ENDF tape may live inside any crate directory.**
//!
//! # Methodology
//!
//! Walks the whole `crates/` tree of the workspace (skipping `target/` and the
//! gitignored `upstream_source/` and `.claude/` scratch) and fails if any file
//! with an `.endf` extension is found. Reference tapes belong in the repo-root
//! `reference-data/endf/`, reached through
//! [`njoy_outram_park_fork::reference_data`].
//!
//! # Why it is a test and not a convention
//!
//! `cargo package` builds the tarball by walking the crate root, so a tape
//! placed under a crate is a candidate for publication. crates.io rejects a
//! package over 10 MB; this workspace's eleven reference tapes total ~89 MB, of
//! which U-235 alone is 36.9 MB. Until 2026-08-17 they sat in
//! `crates/njoy-outram-park-fork/tests/resources/` and were kept out of the
//! tarball only by the `include` allowlist in `Cargo.toml` — correct, but one
//! careless `"tests/**"` entry away from a 89 MB publish attempt. Moving them
//! out and asserting it here makes the layout, not a rule, the thing that holds.
//!
//! # Results
//!
//! Measured 2026-08-17, after the move: 0 `.endf` files under `crates/`,
//! 11 under `reference-data/endf/` (~89 MB). `cargo package --list -p
//! njoy-outram-park-fork` reports 245 files, none of them a tape.
//!
//! Skips (passes with a note) when the workspace root cannot be located, so a
//! crates.io consumer running the suite outside a git clone is not failed by it.

use njoy_outram_park_fork::reference_data::is_endf_tape;
use std::path::{Path, PathBuf};

/// The workspace `crates/` directory, or `None` when this is not a git clone of
/// the workspace (e.g. a vendored copy from crates.io).
fn crates_dir() -> Option<PathBuf> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // <workspace>/crates/njoy-outram-park-fork -> <workspace>/crates
    p.pop();
    p.is_dir().then_some(p)
}

/// Every `.endf` file under `dir`, skipping build output and vendored trees.
fn find_endf_tapes(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            // `target/` is build output; `upstream_source/` vendors the NJOY2016
            // Fortran (gitignored); `.claude/` is agent scratch space. None of
            // the three is packaged, and all three can be large.
            if matches!(name.as_ref(), "target" | "upstream_source") || name.starts_with('.') {
                continue;
            }
            find_endf_tapes(&path, found);
        } else if is_endf_tape(&path) {
            found.push(path);
        }
    }
}

/// A raw ENDF tape under `crates/` would be swept into a `cargo package`
/// tarball. Keep them in `reference-data/endf/` at the repository root.
#[test]
fn no_endf_tape_lives_under_crates() {
    let Some(dir) = crates_dir() else {
        println!("[packaging guard] SKIP: not a workspace clone, no crates/ directory");
        return;
    };

    let mut found = Vec::new();
    find_endf_tapes(&dir, &mut found);

    assert!(
        found.is_empty(),
        "{} raw ENDF tape(s) found inside crates/ — these would be swept into a \
         `cargo package` tarball and blow the 10 MB crates.io limit. Move them to \
         the repo-root `reference-data/endf/` and read them through \
         `njoy_outram_park_fork::reference_data::reference_endf`:\n{}",
        found.len(),
        found
            .iter()
            .map(|p| format!("  {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
