// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Enforces that PETIR's **library code contains no explicit panicking
//! construct**.
//!
//! # Why this matters more here than in most crates
//!
//! PETIR is built to run on bare metal (`thumbv7em-none-eabihf`) and in wasm.
//! On a microcontroller a panic is not a stack trace and a non-zero exit -- it
//! is the end of the program, in a device that may be controlling something.
//! "Return an error the caller can act on" is not a style preference there, it
//! is the difference between a degraded reading and a dead node.
//!
//! So every fallible operation in this crate returns [`petir::Result`], and the
//! error is an enum the caller can match on. This test is what stops that
//! eroding: an `unwrap()` added in a hurry inside a solver would compile, pass
//! every numerical test, and only fail in the field.
//!
//! # What this gate DOES cover
//!
//! Explicit panicking constructs in non-test library code:
//! `panic!`, `unwrap()`, `expect(...)`, `unreachable!`, `todo!`,
//! `unimplemented!`, `assert!`, `assert_eq!`, `assert_ne!`.
//!
//! # What it does NOT cover, stated plainly
//!
//! 1. **Slice and array indexing.** `c[i]` panics when `i` is out of range.
//!    As of 2026-09-14 clippy's `indexing_slicing` lint counts ~164 such sites
//!    in this crate. Most are provably in range by construction, but "provably"
//!    here means by reading, not by the compiler. Tracked as a bead.
//! 2. **Allocation failure.** `Vec` aborts rather than returning an error when
//!    the allocator fails. `alloc` offers no stable fallible `Vec` API, so this
//!    is not fixable from inside PETIR today.
//! 3. **Arithmetic overflow in debug builds.** Integer overflow panics under
//!    `debug_assertions`. PETIR's arithmetic is almost entirely `f64` (which
//!    saturates to infinity rather than panicking), but index arithmetic is
//!    `usize`.
//!
//! A crate that claimed "no panics" while those three held would be
//! overclaiming. What this gate establishes is narrower and still worth having:
//! **no one has written a deliberate panic into a numerical routine.**

use std::fs;
use std::path::{Path, PathBuf};

/// Modules lifted VERBATIM from elsewhere in this workspace.
///
/// These are exempt only from being *rewritten*, not from the check: the
/// check passes on them today and must keep passing. If a lift ever did
/// introduce an explicit panic, the fix would be upstream and then a re-lift,
/// not an edit here -- see `tests/verbatim_provenance.rs`.
const LIFTED: &[&str] = &[
    "linalg/square_matrix.rs",
    "poly/roots.rs",
    "poly/linear_eqn.rs",
    "poly/quadratic_eqn.rs",
    "poly/cubic_eqn.rs",
    "poly/polynomial.rs",
    "specfunc/erf_inv.rs",
    "specfunc/inc_gamma.rs",
    "specfunc/inv_inc_gamma.rs",
    "scalar.rs",
];

/// Patterns that panic at runtime.
const PANICKING: &[&str] = &[
    "panic!",
    ".unwrap()",
    ".expect(",
    "unreachable!",
    "todo!",
    "unimplemented!",
    "assert!",
    "assert_eq!",
    "assert_ne!",
];

/// Collect every `.rs` file under `src/`.
fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Strip the parts of a file that are allowed to panic: `#[cfg(test)]` modules
/// (test code SHOULD assert) and doc comments (an example calling `.unwrap()`
/// is idiomatic and does not ship).
fn library_lines(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        // A #[cfg(test)] module runs to the end of the file in this crate's
        // layout; everything after it is test code.
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        // Doc comments and ordinary comments are not code.
        if line.starts_with("///") || line.starts_with("//!") || line.starts_with("//") {
            continue;
        }
        out.push((i + 1, raw.to_string()));
    }
    out
}

#[test]
fn library_code_contains_no_explicit_panic() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&src, &mut files);
    assert!(!files.is_empty(), "found no source files under {src:?}");

    let mut offences: Vec<String> = Vec::new();

    for file in &files {
        let rel = file
            .strip_prefix(&src)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");

        // verification_tests.rs is a test module included by its parent under
        // #[cfg(test)]; it is test code by role even though it is its own file.
        if rel.ends_with("verification_tests.rs") || rel.ends_with("recurrence_tests.rs") {
            continue;
        }

        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };

        for (lineno, line) in library_lines(&text) {
            for pat in PANICKING {
                if line.contains(pat) {
                    let lifted = LIFTED.iter().any(|l| rel.ends_with(l));
                    offences.push(format!(
                        "{rel}:{lineno}{}  {}",
                        if lifted { " (LIFTED)" } else { "" },
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "PETIR library code must not panic -- return a petir::Result instead, so a \
         caller on bare metal can handle the failure rather than die of it.\n\
         If the offending file is marked (LIFTED), do NOT edit it here: fix it \
         upstream and re-lift, or `tests/verbatim_provenance.rs` will fail.\n\n{}",
        offences.join("\n")
    );
}

/// Every public fallible entry point should return [`petir::Result`] rather
/// than signalling failure some other way.
///
/// Checked by sampling the API rather than by reflection, which Rust does not
/// offer: these are the constructors and drivers a caller actually reaches for,
/// and all of them are total functions returning a matchable error.
#[test]
fn the_public_fallible_api_returns_result() {
    use petir::linalg::SquareMatrix;
    use petir::min::{MinMethod, Minimizer};
    use petir::roots::{BracketingMethod, BracketingSolver};
    use petir::{ChebSeries, PetirError};

    // A singular system reports, rather than returning garbage or panicking.
    let mut a = SquareMatrix::new(2);
    a.set(0, 0, 1.0);
    a.set(0, 1, 2.0);
    a.set(1, 0, 2.0);
    a.set(1, 1, 4.0);
    assert!(petir::linalg::det(&a).is_err());

    // A null interval reports.
    assert!(matches!(
        ChebSeries::new(8, 1.0, 1.0, |x| x),
        Err(PetirError::Domain)
    ));

    // A bracket that does not straddle zero reports.
    let bad = BracketingSolver::new(BracketingMethod::Brent, |x: f64| x * x + 1.0, 0.0, 1.0);
    assert!(matches!(bad, Err(PetirError::Invalid)));

    // A triple that does not enclose a minimum reports.
    let bad_min = Minimizer::new(MinMethod::Brent, |x: f64| x, 1.0, 0.0, 3.0);
    assert!(matches!(bad_min, Err(PetirError::Invalid)));

    // And the error is an ENUM the caller can branch on, with a stable GSL
    // symbol behind it -- not a string to be parsed.
    assert_eq!(PetirError::Domain.gsl_code(), "GSL_EDOM");
}
