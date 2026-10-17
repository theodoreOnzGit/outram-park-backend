// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! # A whole ENDF library from a folder, a zip, or uploaded files
//!
//! **Epic gh #925, child #935.** The maintainer's design (2026-10-17): take in
//! a whole library at once, as the zip archive or the extracted folder, for
//! every ENDF library; a parser reads the names and matches them to an enum
//! that gives the folder layout; the indexing is shared code; and in a
//! browser the user uploads the zip or folder and **the same code** runs.
//!
//! ```text
//!  folder (native)  ─┐
//!  zip as bytes      ├─► EndfLibraryIndex ─► main_library() ─► incident_neutron(lib, Z, A, LISO) ─► Tape ─► process
//!  files as bytes   ─┘      │  names ─► LibraryLayout (which files to open) + EndfLibrary named in paths
//!                           └  each file's first 4 KB ─► EndfHeader (identity: ZA, LISO, NSUB, MAT, library)
//! ```
//!
//! - **Bytes in, values out.** [`EndfLibraryIndex::from_zip_bytes`] and
//!   [`EndfLibraryIndex::from_files`] touch no file and run in `wasm32`;
//!   [`EndfLibraryIndex::from_folder`] is the thin native wrapper. All three
//!   build the index with one function.
//! - **Names give the layout, headers give the identity.**
//!   [`LibraryLayout::detect`] reads the paths (a `neutrons/` folder, an
//!   NNDC `ENDF-B-VIII.0_neutrons/` folder, the IAEA `n_9228_92-U-235.zip`
//!   tree, or flat), which decides **which files are opened**: decay, yields
//!   and photo-atomic data are never inflated. Each opened file is identified
//!   by its MF=1/MT=451 header ([`EndfHeader`]), never by its name, and
//!   [`library_named_in`] reports which [`crate::acquire::EndfLibrary`] a
//!   folder name points at.
//! - **Every ENDF-6 library.** Identity uses only fields ENDF-102 defines for
//!   every library; the library is the tape's own `HSUB` line, or its `NLIB`
//!   family ([`library_family`], ENDF-102 Table 2) and `NVER`. A library
//!   with no enum variant indexes exactly like one with a variant. Tapes in
//!   an older format (`NFOR` other than 6) are listed as skipped, with the
//!   reason, because this crate processes ENDF-6.
//! - **One library at a time.** [`EndfLibraryIndex::main_library`] is the
//!   library most of the incident-neutron tapes belong to; a lookup uses its
//!   tapes only and names any other library's tape for the same nuclide in
//!   the error rather than mixing evaluations silently.
//! - **Malformed input is an error, never a panic** (a corrupt zip, a file
//!   that is not a tape: skipped with a reason).
//!
//! [`generate`] is #936's browser generation path on top of this, and
//! [`fuzzy_score`] the picker's matcher.
//!
//! # Relation to [`crate::endf_folder`]
//!
//! `endf_folder` (gh:#581) lists a folder and finds one tape in it by MAT and
//! NSUB, for the Dhoby Ghaut workbench's Step 0. It is left as it is; this
//! module is the general index (zips, uploads, layouts, library choice) that
//! nuclear-data loading uses.

mod generate;
mod header;
mod index;
mod layout;

pub use generate::{
    generate_ace, generate_thermal_ace, suggest_thermal_partner, temperature_folder, AceRequest,
    GeneratedAce,
};
pub use header::{library_family, EndfHeader, SubLibrary};
pub use index::{EndfLibraryIndex, IndexedTape, LibrarySource, LookupError, SkippedFile};
pub use layout::{library_named_in, LibraryLayout, PathRole};

/// **Score how well `query` matches `candidate`**, or `None` if it does not.
/// Both are lowercased, so the match is case-insensitive.
///
/// - **Substring** matches score highest (1000+), with a bonus for a prefix
///   match and for a tighter candidate.
/// - **Subsequence** matches (every query character in order, fzf-style)
///   score below any substring, penalised by how spread out they are.
///
/// Moved here on 2026-10-17 (gh #936) from the njoy TUI's nuclide finder
/// (`src/bin/njoy-tui/nuclides.rs`, which now calls this one) so the
/// browser's ACE picker and the TUI share it; `kovan_common::fuzzy` is a
/// port of the same algorithm, kept in step by hand (a dependency between the
/// two crates would run the wrong way).
#[must_use]
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let query = query.trim().to_lowercase();
    let candidate = candidate.to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    const SUBSTRING_BASE: i32 = 1_000;
    const SUBSEQUENCE_BASE: i32 = 0;
    if let Some(pos) = candidate.find(&query) {
        let prefix_bonus = if pos == 0 { 50 } else { 0 };
        let tightness = 100 - (candidate.len() as i32 - query.len() as i32).min(100);
        return Some(SUBSTRING_BASE + prefix_bonus + tightness);
    }
    let cand: Vec<char> = candidate.chars().collect();
    let mut ci = 0usize;
    let mut first_match: Option<usize> = None;
    let mut last_match = 0usize;
    for qc in query.chars() {
        let mut found = false;
        while ci < cand.len() {
            if cand[ci] == qc {
                first_match.get_or_insert(ci);
                last_match = ci;
                ci += 1;
                found = true;
                break;
            }
            ci += 1;
        }
        if !found {
            return None;
        }
    }
    let span = last_match - first_match.unwrap_or(0) + 1;
    let spread_penalty = span as i32 - query.chars().count() as i32;
    Some(SUBSEQUENCE_BASE - spread_penalty)
}

#[cfg(test)]
mod tests;
