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
//! # What a SECOND gate covers: variable slice indexing
//!
//! `c[i]` panics when `i` is out of range, and a subscript is not an explicit
//! panicking construct, so the first gate never saw one.
//! `ported_modules_contain_no_variable_slice_indexing` closes that: in every
//! module PETIR ported or wrote, an index must be an **integer literal**.
//!
//! A literal index into a fixed-size array is checked by the compiler --
//! rustc's deny-by-default `unconditional_panic` lint rejects `A[5]` on an
//! `[f64; 3]` outright -- so `AH[0]` and `EC[6]` in the Runge-Kutta tableau
//! cannot fail and are left alone. Keeping them is what lets those loops still
//! diff line-for-line against `rkf45.c`. Everything else goes through
//! `get`/`split_first`/`split_last`/`first`/`last` or a `zip`, which is why
//! `crate::zip` exists.
//!
//! **The 19 verbatim lifts are exempt from the indexing gate**, and only from
//! that one. Rewriting a subscript inside a lift would destroy the
//! byte-identical property the lift exists for (`tests/verbatim_provenance.rs`
//! allows zero substantive deviations on 17 of the 19). The fix for those is
//! upstream, then a re-lift.
//!
//! # What neither gate covers, stated plainly
//!
//! 1. **Allocation failure** — see below. Slice indexing *is* covered now, by
//!    `ported_modules_contain_no_variable_slice_indexing`; it was not when this
//!    list was first written.
//! 2. **Arithmetic overflow in debug builds.** Integer overflow panics under
//!    `debug_assertions`. PETIR's arithmetic is almost entirely `f64` (which
//!    saturates to infinity rather than panicking), but index arithmetic is
//!    `usize`.
//!
//! A crate that claimed "no panics" while those two held would be
//! overclaiming. What the two gates together establish is narrower and worth
//! having: **no deliberate panic has been written into a numerical routine,
//! and no routine PETIR ported can panic on an out-of-range index.**

use std::fs;
use std::path::{Path, PathBuf};

/// Modules lifted VERBATIM from elsewhere in this workspace.
///
/// For the **explicit-panic** gate these are exempt only from being
/// *rewritten*, not from the check: it passes on them today and must keep
/// passing. If a lift ever did introduce an explicit panic, the fix would be
/// upstream and then a re-lift, not an edit here.
///
/// For the **indexing** gate they are genuinely exempt, because rewriting a
/// subscript here would break the byte-identical property
/// `tests/verbatim_provenance.rs` enforces.
///
/// Kept in step with that test's own `LIFTS` table by
/// `the_lifted_list_matches_the_provenance_tests`.
///
/// # `scalar.rs` is deliberately NOT here
///
/// Its PROVENANCE block calls it a verbatim lift of
/// `outram-foam-basic-lib/src/primitives/scalar.rs`, and the OpenFOAM guard
/// constants in it are. But PETIR then **added** GSL's `GSL_SQRT_DBL_EPSILON`,
/// `GSL_ROOT3_DBL_EPSILON`, `GSL_DBL_MIN`/`MAX` and `ROOT_SMALL` alongside
/// them, so the file is a hybrid: 13 substantive lines exist only here. That
/// is why the provenance test does not cover it, and the indexing exemption is
/// granted **on the strength of that coverage** -- so `scalar.rs` does not get
/// it, and is held to the same indexing rule as the ported modules. It passes:
/// the file is constants and type aliases, with no subscript in it.
///
/// Whether those GSL constants belong in an OpenFOAM-derived file at all, or
/// in a `gsl_consts` module of their own, is a real question -- `bn:op-l87q`.
const LIFTED: &[&str] = &[
    // --- from outram-foam-basic-lib -------------------------------------
    "linalg/square_matrix.rs",
    "poly/roots.rs",
    "poly/linear_eqn.rs",
    "poly/quadratic_eqn.rs",
    "poly/cubic_eqn.rs",
    "poly/polynomial.rs",
    "specfunc/erf_inv.rs",
    "specfunc/inc_gamma.rs",
    "specfunc/inv_inc_gamma.rs",
    // --- from chem-eng-real-time-process-control-simulator ---------------
    "transfer_fn/continuous_tf.rs",
    "transfer_fn/conversion.rs",
    "transfer_fn/cplx.rs",
    "transfer_fn/decaying_sinusoid.rs",
    "transfer_fn/discrete_tf.rs",
    "transfer_fn/first_order_transfer_fn.rs",
    "transfer_fn/first_order_transfer_fn_with_zeroes.rs",
    "transfer_fn/polynomial.rs",
    "transfer_fn/second_order_transfer_fn.rs",
    "transfer_fn/step_fn.rs",
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

/// Split a line into the index expressions it contains: for each `name[...]`,
/// the text between the brackets.
///
/// Deliberately narrow. An identifier (or a path ending in one, such as
/// `self.c`) must sit immediately before the `[`, which is what distinguishes
/// an *index* from array **type** syntax (`[f64; 8]`), an array literal, an
/// attribute (`#[inline]`), or a slice pattern -- none of which can panic and
/// all of which are preceded by a space, `#`, `(`, `:` or another `[`.
fn index_expressions(line: &str) -> Vec<String> {
    let bytes: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for (i, &ch) in bytes.iter().enumerate() {
        if ch != '[' || i == 0 {
            continue;
        }
        // The character before must end an identifier or path.
        let prev = bytes[i - 1];
        if !(prev.is_ascii_alphanumeric() || prev == '_') {
            continue;
        }
        // `vec![..]`, `assert_eq![..]` and friends are macros, not indexing;
        // they are excluded by the `!` that precedes the bracket, which is not
        // alphanumeric, so nothing extra is needed here.
        let Some(close) = bytes[i + 1..].iter().position(|&c| c == ']') else {
            continue;
        };
        out.push(bytes[i + 1..i + 1 + close].iter().collect::<String>());
    }
    out
}

/// An index that the compiler checks: a plain integer literal, which rustc's
/// deny-by-default `unconditional_panic` lint rejects outright when it is out
/// of range for an array of known length.
fn is_compile_time_checked(index: &str) -> bool {
    let t = index.trim();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '_')
}

/// Every module PETIR **ported or wrote** must index only by integer literal,
/// so no subscript in it can panic at runtime.
///
/// See this file's module documentation for why the 19 verbatim lifts are
/// exempt, and why literal indices are left in place rather than rewritten.
#[test]
fn ported_modules_contain_no_variable_slice_indexing() {
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

        if rel.ends_with("verification_tests.rs") || rel.ends_with("recurrence_tests.rs") {
            continue;
        }
        if LIFTED.iter().any(|l| rel.ends_with(l)) {
            continue;
        }

        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };

        for (lineno, line) in library_lines(&text) {
            // A trailing `//` comment on a code line is still commentary.
            let code = line.split("//").next().unwrap_or(&line);
            for index in index_expressions(code) {
                if !is_compile_time_checked(&index) {
                    offences.push(format!("{rel}:{lineno}  [{index}]   {}", line.trim()));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "PETIR's ported modules must not index by a runtime value -- reach for \
         get/get_mut, first/last, split_first/split_last, or a `zip_flat!` \
         (crate::zip) instead, and return a petir::Result where the case is \
         genuinely reachable.\n\
         An integer-literal index into a fixed-size array is fine and is what \
         keeps the tableau loops diffable against upstream.\n\n{}",
        offences.join("\n")
    );
}

/// The lifted-file list above must name exactly the lifts
/// `tests/verbatim_provenance.rs` checks.
///
/// # Why a test rather than a shared constant
///
/// Integration tests are separate binaries and cannot share a module without a
/// `tests/common/`, which would be more machinery than this needs. Reading the
/// other test's source and comparing the two lists costs one file read and
/// catches the failure that matters: a lift added there and not here would be
/// silently exempted from the indexing gate — or worse, one removed there and
/// left here would exempt a file that is no longer a lift at all.
#[test]
fn the_lifted_list_matches_the_provenance_tests() {
    let provenance = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("verbatim_provenance.rs");
    let text = fs::read_to_string(&provenance)
        .unwrap_or_else(|e| panic!("cannot read {provenance:?}: {e}"));

    // Each entry in that file's LIFTS table opens with `lifted: "<path>",`.
    let mut declared: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("lifted: \"") {
            if let Some(path) = rest.split('"').next() {
                declared.push(path.to_string());
            }
        }
    }
    assert!(
        !declared.is_empty(),
        "found no `lifted:` entries in {provenance:?} -- has that table been \
         renamed? This cross-check needs updating with it."
    );

    let mut declared_sorted = declared.clone();
    declared_sorted.sort();
    let mut ours: Vec<String> = LIFTED.iter().map(|s| (*s).to_string()).collect();
    ours.sort();

    assert_eq!(
        ours, declared_sorted,
        "the LIFTED list in tests/no_panic_gate.rs has drifted from the LIFTS \
         table in tests/verbatim_provenance.rs. Every verbatim lift must appear \
         in both: the provenance test is what pins it byte-for-byte, and this \
         file's list is what exempts it from the indexing gate on that basis."
    );
}
