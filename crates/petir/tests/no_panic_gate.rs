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
//! # What a SECOND gate covers: indexing that can fail
//!
//! `c[i]` panics when `i` is out of range, and a subscript is not an explicit
//! panicking construct, so the first gate never saw one.
//! `no_subscript_in_the_crate_can_fail_at_runtime` closes that, **across the
//! whole crate including the verbatim lifts**: the only subscript allowed is a
//! literal index into a `const` array whose length is a literal, and the test
//! parses those declarations itself and checks the index against the length.
//!
//! Why that carve-out and no other: rustc's deny-by-default
//! `unconditional_panic` lint rejects `A[5]` on a `const A: [f64; 3]`
//! outright, so `AH[0]` and `EC[6]` in the Runge-Kutta tableau are checked at
//! compile time and cannot fail. Keeping them is what lets those stage loops
//! still diff line-for-line against `rkf45.c`. Everything else goes through
//! `get`/`first`/`last`/`split_first`/`split_last`, a slice pattern, or a
//! `zip` -- which is why `crate::zip` exists.
//!
//! **An earlier version of this gate allowed any integer literal, and exempted
//! the lifts.** Both were wrong, and writing the stricter check is what showed
//! it: `den_m[0]`, `num_m[2]`, `roots[1]` and `p[0]` are literal indices into
//! `Vec`s, which are bounds-checked at run time like any other, and they sat
//! inside lifted files where the exemption hid them. They were fixed upstream
//! in `chem-eng-real-time-process-control-simulator` and re-lifted, which is
//! why the lifts need no exemption now.
//!
//! # What neither gate covers, stated plainly
//!
//! 1. **Allocation failure.** `Vec` aborts rather than returning an error when
//!    the allocator fails. `alloc` offers no stable fallible `Vec` API, so
//!    this is not fixable from inside PETIR today.
//! 2. **Arithmetic overflow in debug builds.** Integer overflow panics under
//!    `debug_assertions`. PETIR's arithmetic is almost entirely `f64` (which
//!    saturates to infinity rather than panicking), but index arithmetic is
//!    `usize`.
//!
//! A crate that claimed "no panics" while those two held would be
//! overclaiming. What the two gates together establish is narrower and worth
//! having: **no deliberate panic has been written into a numerical routine,
//! and no subscript anywhere in the crate can fail at run time.**

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Modules lifted VERBATIM from elsewhere in this workspace.
///
/// For the **explicit-panic** gate these are exempt only from being
/// *rewritten*, not from the check: it passes on them today and must keep
/// passing. If a lift ever did introduce an explicit panic, the fix would be
/// upstream and then a re-lift, not an edit here.
///
/// The **indexing** gate no longer exempts them: as of 2026-09-15 the lifts
/// contain no failing subscript either, because the ones they had were fixed
/// in `outram-foam-basic-lib` and
/// `chem-eng-real-time-process-control-simulator` and then re-lifted. That is
/// the route any future one must take too -- editing a lift here would break
/// the byte-identical property `tests/verbatim_provenance.rs` enforces.
///
/// Kept in step with that test's own `LIFTS` table by
/// `the_lifted_list_matches_the_provenance_tests`.
///
/// This list is therefore used only to label a first-gate offence as
/// `(LIFTED)`, so the failure message can say where the fix belongs.
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
/// it. It passes the indexing gate regardless: the file is constants and type
/// aliases, with no subscript in it.
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
fn index_expressions(line: &str) -> Vec<(String, String)> {
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
        // Walk back over the indexed expression: `self.c`, `TAB`, `a.values`.
        let mut start = i;
        while start > 0 {
            let c = bytes[start - 1];
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                start -= 1;
            } else {
                break;
            }
        }
        let target: String = bytes[start..i].iter().collect();
        let index: String = bytes[i + 1..i + 1 + close].iter().collect();
        out.push((target, index));
    }
    out
}

/// Every `const NAME: [T; N]` in the crate, as `NAME -> N`, plus any
/// `use ... as ALIAS` rename of one.
///
/// This is what makes the literal-index carve-out a *check* rather than an
/// assumption: `AH[0]` is allowed because `AH` is declared `[f64; 5]` right
/// here in the crate and `0 < 5`, not because `0` looks harmless. A literal
/// index into a `Vec` gets no such licence -- and that distinction is exactly
/// what the first version of this test missed.
fn fixed_size_consts(files: &[PathBuf]) -> HashMap<String, usize> {
    let mut lengths: HashMap<String, usize> = HashMap::new();
    let mut aliases: Vec<(String, String)> = Vec::new();

    for file in files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim();

            // `const NAME: [.. ; 123] = ..`
            if let Some(rest) = t.strip_prefix("const ").or_else(|| t.strip_prefix("pub const ")) {
                if let Some((name, ty)) = rest.split_once(':') {
                    if let Some(len) = outer_array_len(ty) {
                        lengths.insert(name.trim().to_string(), len);
                    }
                }
            }

            // `use path::{ NAME as ALIAS, .. }` -- fast_pow imports fast_exp's
            // tables under new names, and they are still the same constants.
            for part in t.split(|c| c == '{' || c == ',' || c == '}') {
                let seg = part.trim().trim_end_matches(';');
                if let Some((from, to)) = seg.split_once(" as ") {
                    let from = from.trim().rsplit("::").next().unwrap_or("").trim();
                    let to = to.trim();
                    if !from.is_empty() && !to.is_empty() {
                        aliases.push((to.to_string(), from.to_string()));
                    }
                }
            }
        }
    }

    for (alias, original) in aliases {
        if let Some(&len) = lengths.get(&original) {
            lengths.insert(alias, len);
        }
    }
    lengths
}

/// The declared length of an array type such as `[f64; 8]` or `[[f64; 2]; 3]`,
/// or `None` if the type is not a fixed-size array.
///
/// Only the OUTER length matters, because `NAME[k]` selects a row.
fn outer_array_len(ty: &str) -> Option<usize> {
    let t = ty.trim().strip_prefix('[')?;
    // Everything up to the matching close bracket.
    let mut depth = 1usize;
    let mut end = None;
    for (i, c) in t.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let inner = t.get(..end?)?;
    // The outer length follows the last top-level `;`.
    let mut depth = 0usize;
    let mut semi = None;
    for (i, c) in inner.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => semi = Some(i),
            _ => {}
        }
    }
    inner
        .get(semi? + 1..)?
        .trim()
        .replace('_', "")
        .parse::<usize>()
        .ok()
}

/// An index rustc checks at compile time: a literal into a `const` array whose
/// declared length is also a literal.
///
/// A literal index into a `Vec` or a slice is NOT this -- it is bounds-checked
/// at run time like any other, and `den_m[0]` panicking on an empty `den_m` is
/// exactly the bug the first version of this gate waved through.
fn is_compile_time_checked(target: &str, index: &str, consts: &HashMap<String, usize>) -> bool {
    let Ok(k) = index.trim().replace('_', "").parse::<usize>() else {
        return false; // not a literal at all
    };
    let name = target.rsplit('.').next().unwrap_or(target).trim();
    consts.get(name).is_some_and(|&len| k < len)
}

/// No subscript anywhere in PETIR's library code can fail at run time.
///
/// The one permitted form is a literal index into a `const` array of literal
/// length, which rustc checks at compile time; see this file's module
/// documentation for why that one is safe and nothing else is.
///
/// Unlike the explicit-panic gate, this one covers the **verbatim lifts too**.
/// They pass because the subscripts they had were fixed in
/// `outram-foam-basic-lib` and `chem-eng-real-time-process-control-simulator`
/// and re-lifted -- which is the route a future one has to take as well.
#[test]
fn no_subscript_in_the_crate_can_fail_at_runtime() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&src, &mut files);
    assert!(!files.is_empty(), "found no source files under {src:?}");

    let consts = fixed_size_consts(&files);
    assert!(
        consts.get("AH") == Some(&5) && consts.get("EC") == Some(&7),
        "the const-array scan found nothing recognisable -- AH and EC from the \
         RKF45 tableau should be [f64; 5] and [f64; 7]. Has the declaration \
         style changed? Without them the allowances below cannot be trusted. \
         Found {} entries.",
        consts.len()
    );

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

        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };

        for (lineno, line) in library_lines(&text) {
            // A trailing `//` comment on a code line is still commentary.
            let code = line.split("//").next().unwrap_or(&line);
            for (target, index) in index_expressions(code) {
                if is_compile_time_checked(&target, &index, &consts) {
                    continue;
                }
                let lifted = LIFTED.iter().any(|l| rel.ends_with(l));
                offences.push(format!(
                    "{rel}:{lineno}{}  {target}[{index}]   {}",
                    if lifted { " (LIFTED)" } else { "" },
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "PETIR must contain no subscript that can fail at run time -- reach for \
         get/get_mut, first/last, split_first/split_last, a slice pattern, or a \
         `zip_flat!` (crate::zip) instead, and return a petir::Result where the \
         case is genuinely reachable.\n\
         The one exception is a literal index into a `const` array of literal \
         length, which rustc checks at compile time; a literal index into a Vec \
         or a slice is NOT that.\n\
         If the offending file is marked (LIFTED), do NOT edit it here: fix it \
         upstream and re-lift, or tests/verbatim_provenance.rs will fail.\n\n{}",
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
