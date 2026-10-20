// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **Workspace gate: no platform-libm transcendental in any converted crate's
//! `src/`** (gh:#985, epic gh:#990).
//!
//! # Why
//!
//! In a `std` crate, `x.exp()`, `x.powi(n)`, `x.atan2(y)` and friends resolve
//! to the inherent `f64`/`f32` methods, whose last bit is set by the target's
//! C library or code generator (glibc, MSVC, wasi-libc, Apple libm). gh:#985
//! moved every such call in the workspace onto [`petir::mathf::RealMath`]
//! (`.r_exp()`, `.r_powi()`, ...), one fixed implementation on every target.
//! The `r_` methods do not shadow `std`, so a missed or newly added raw call
//! **compiles and runs silently on the platform route**; nothing numerical
//! catches it on a single host. This test does.
//!
//! It generalises the two hot-path gates that came first,
//! `crates/tampines-steam-tables/tests/no_platform_libm_on_hot_path.rs` and
//! `crates/outram-park-fork-coolprop/tests/no_platform_libm_on_hot_path.rs`,
//! from a list of solver directories to every member crate. Those two stay:
//! they predate this gate, their docs and V&V records cite them, and they
//! fail inside the crate a contributor is actually working on.
//!
//! # What it covers
//!
//! Non-test, non-comment lines of every `.rs` file under `crates/*/src/`
//! (binaries in `src/bin/` included), for every crate directory that has a
//! `Cargo.toml`. Files under a `tests/` directory, files named `tests.rs`, and
//! everything after an inline `#[cfg(test)] mod … {` are test code and are
//! skipped, exactly as in the two hot-path gates. A test module in a file of
//! another name (`#[cfg(test)] mod upstream_tests;`) IS scanned; that is
//! stricter than needed, and such files are kept on PETIR too. A raw call is the method
//! spelling `.name(` or the path spellings `f64::name` / `f32::name` (a UFCS
//! call or a function pointer) for every name in [`RAW`]. Calls through `petir::real::Real` by trait path
//! (`Real::asinh(x)`, the route for the inverse hyperbolics, which
//! `RealMath` does not carry) are not matched.
//!
//! # What it does NOT cover
//!
//! - `sqrt`, `abs`, `floor`, `ceil`, `round`, `mul_add`: IEEE-754 correctly
//!   rounded or exact, so identical everywhere. Not flagged.
//! - `examples/`, `tests/` and `benches/`. Those are not library code; the
//!   gh:#985 sweep converted them anyway, but nothing holds them there.
//! - A call through a helper whose name is not in [`RAW`]. This is a text
//!   match, not a call-graph analysis.
//! - Calls inside a dependency. Only the call site in this workspace is
//!   visible. (Since `cf41d10121`, `uom` is built without `std`, so its
//!   `Quantity::exp`/`powi`/... no longer exist to be called.)
//!
//! # Exclusions
//!
//! Every exclusion is listed in [`EXCLUDED_CRATES`], [`EXCLUDED_DIRS`] or
//! [`EXCLUDED_LINES`] with its reason. A new one needs a reason of the same
//! kind; "it was failing" is not one.

use std::fs;
use std::path::{Path, PathBuf};

/// Raw `f64`/`f32` methods (and their UFCS spellings) that reach the platform
/// libm or a backend-dependent lowering.
const RAW: &[&str] = &[
    "powi", "powf", "exp", "exp2", "exp_m1", "ln", "ln_1p", "log", "log10", "log2", "cbrt",
    "hypot", "sin", "cos", "tan", "sin_cos", "asin", "acos", "atan", "atan2", "sinh", "cosh",
    "tanh", "asinh", "acosh", "atanh",
];

/// Whole crates the gate does not scan, with the reason.
const EXCLUDED_CRATES: &[(&str, &str)] = &[(
    "petir",
    "the implementation itself: its ports, its `libm` delegation and its \
     platform canaries (`tests/libm_vs_platform.rs`) are the point",
)];

/// Directories (relative to `crates/`) the gate does not scan, with the reason.
const EXCLUDED_DIRS: &[(&str, &str)] = &[(
    "boon-lay/src/rustbca",
    "the RustBCA port carries its own `Real` trait and its own routing \
     discipline, gated by `local_change_tests.rs` (gh:#995 owns it)",
)];

/// Individual lines the gate does not flag: (path relative to `crates/`, a
/// substring of the code line, reason). The substring must match exactly one
/// line of that file, so a stale entry fails rather than lingering.
const EXCLUDED_LINES: &[(&str, &str, &str)] = &[
    // chem-eng's own complex type: `Cplx::exp` / `Cplx::ln` are its methods,
    // and their bodies (`z_domain/cplx.rs`) are on `r_exp`/`r_cos`/`r_sin`/
    // `r_ln`/`r_atan2`. A text match cannot see the receiver type.
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "((x).exp() - Cplx::real(1.0)) / lambda", "Cplx::exp, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "let e1 = (l1 * t_s).exp();", "Cplx::exp, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "let e2 = (l2 * t_s).exp();", "Cplx::exp, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "p_c.iter().map(|&p| (p * t_s).exp())", "Cplx::exp, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "z_c.iter().map(|&z| (z * t_s).exp())", "Cplx::exp, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "let w_d = Cplx::new(0.0, w_c * t_s).exp();", "Cplx::exp, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "p_d.iter().map(|&p| p.ln()", "Cplx::ln, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/conversion.rs", "z_d.iter().map(|&z| z.ln()", "Cplx::ln, own type, PETIR inside"),
    ("chem-eng-real-time-process-control-simulator/src/lib/beta_testing/z_domain/verification_tests.rs", "Cplx::new(0.0, -w * t).exp()", "Cplx::exp, own type, PETIR inside"),
];

/// Lines that are still on the platform route and are someone else's to
/// convert right now. These are FAILURES the gate tolerates only while the
/// named issue is open; each must be removed when that work lands.
const PENDING_LINES: &[(&str, &str, &str)] = &[
    // Empty. tuas' six uom `Quantity::powi(P2)` sites, which this list held
    // while the gate was written, were rerouted by the uom-without-std work
    // (`cf41d10121`, gh:#985) before it landed.
];

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("petir must live at <workspace>/crates/petir")
        .to_path_buf()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs")
            && !p.file_name().is_some_and(|n| n == "tests.rs")
        {
            out.push(p);
        }
    }
}

/// Non-test, non-comment code lines, with any trailing `//` comment removed.
/// The same rule as the two hot-path gates.
fn code_lines(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.starts_with("#[cfg(test)]") {
            // An inline test module runs to the end of the file; a bare
            // `mod tests;` declaration does not, and its file is skipped above.
            let mut j = i + 1;
            while j < lines.len()
                && (lines[j].trim().is_empty() || lines[j].trim().starts_with("#["))
            {
                j += 1;
            }
            if j < lines.len() && lines[j].contains("mod ") && lines[j].trim_end().ends_with('{')
            {
                break;
            }
        }
        if !t.starts_with("//") {
            let code = lines[i].split("//").next().unwrap_or("");
            out.push((i + 1, code.to_string()));
        }
        i += 1;
    }
    out
}

/// `path` occurs in `code` not followed by an identifier character, so both
/// the call `f64::exp(x)` and the function pointer `.map(f64::exp)` match,
/// and `f64::exp_m1` does not match `f64::exp`.
fn has_path(code: &str, path: &str) -> bool {
    code.match_indices(path).any(|(i, _)| {
        code[i + path.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

fn offences_in(text: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    for (n, code) in code_lines(text) {
        for name in RAW {
            let method = format!(".{name}(");
            if code.contains(&method)
                || has_path(&code, &format!("f64::{name}"))
                || has_path(&code, &format!("f32::{name}"))
            {
                v.push((n, code.trim().to_string()));
                break;
            }
        }
    }
    v
}

/// Every `.rs` file the gate scans, as (path relative to `crates/`, path).
fn scanned_files() -> Vec<(String, PathBuf)> {
    let crates = crates_dir();
    let mut dirs: Vec<PathBuf> = fs::read_dir(&crates)
        .expect("crates/ is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .collect();
    dirs.sort();
    let mut out = Vec::new();
    for d in dirs {
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        if EXCLUDED_CRATES.iter().any(|(c, _)| *c == name) {
            continue;
        }
        let mut files = Vec::new();
        rs_files(&d.join("src"), &mut files);
        for f in files {
            let rel = f.strip_prefix(&crates).unwrap().to_string_lossy().replace('\\', "/");
            if EXCLUDED_DIRS.iter().any(|(dir, _)| rel.starts_with(&format!("{dir}/"))) {
                continue;
            }
            out.push((rel, f));
        }
    }
    out
}

fn allowed(rel: &str, code: &str) -> bool {
    EXCLUDED_LINES
        .iter()
        .chain(PENDING_LINES)
        .any(|(f, sub, _)| *f == rel && code.contains(sub))
}

#[test]
fn no_platform_libm_in_any_converted_crate() {
    let files = scanned_files();
    assert!(files.len() > 1000, "found only {} files; the walk is broken", files.len());

    let mut offences = Vec::new();
    for (rel, f) in &files {
        let text = fs::read_to_string(f).expect("readable source file");
        for (n, code) in offences_in(&text) {
            if !allowed(rel, &code) {
                offences.push(format!("crates/{rel}:{n}  {code}"));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "raw f64/f32 transcendental(s) reach the platform libm. Use \
         petir::mathf::RealMath (`.r_exp(`, `.r_powi(`, `.r_atan2(`, ...), or \
         petir::real::Real by trait path for asinh/acosh/atanh:\n  {}",
        offences.join("\n  ")
    );
}

/// Every listed exclusion still matches exactly one line of a file that
/// exists, so a fixed or moved site cannot leave a stale entry behind, and
/// every excluded directory and crate still exists.
#[test]
fn every_exclusion_is_live() {
    let crates = crates_dir();
    for (c, _) in EXCLUDED_CRATES {
        assert!(crates.join(c).join("Cargo.toml").is_file(), "excluded crate {c} is gone");
    }
    for (d, _) in EXCLUDED_DIRS {
        assert!(crates.join(d).is_dir(), "excluded directory {d} is gone");
    }
    for (f, sub, _) in EXCLUDED_LINES.iter().chain(PENDING_LINES) {
        let text = fs::read_to_string(crates.join(f))
            .unwrap_or_else(|_| panic!("excluded file {f} is gone"));
        let hits = offences_in(&text).into_iter().filter(|(_, code)| code.contains(sub)).count();
        assert_eq!(hits, 1, "exclusion `{sub}` in {f} matches {hits} raw-call lines, not 1");
    }
}

/// The gate can fail: it flags a raw method call and both UFCS spellings in
/// non-test code, and ignores comments, `r_` methods, `Real::` trait-path
/// calls, `sqrt`, and inline test modules.
#[test]
fn the_gate_flags_raw_calls_and_only_raw_calls() {
    let sample = "\
use petir::mathf::RealMath;
fn a(x: f64) -> f64 { x.powi(2) }
fn b(x: f64) -> f64 { f64::exp(x) }
fn c(x: f32) -> f32 { f32::sin(x) }
fn d(x: f64) -> f64 { x.r_powi(2) + x.r_atan() + x.sqrt() } // x.atan() in a comment
fn e(x: f64) -> f64 { petir::real::Real::asinh(x) }
fn f(x: f64) -> f64 { x.atanh() }
fn h(v: &[f64]) -> Vec<f64> { v.iter().copied().map(f64::exp).collect() }
fn i(x: f64) -> f64 { x.r_exp_m1() + f64::EPSILON }
// x.ln() in a comment line
/// x.cbrt() in a doc comment
#[cfg(test)]
mod tests {
    fn g(x: f64) -> f64 { x.cbrt() }
}
";
    let hits: Vec<usize> = offences_in(sample).into_iter().map(|(n, _)| n).collect();
    assert_eq!(hits, vec![2, 3, 4, 7, 8]);
}

/// The walk reaches crates, skips the exclusions, and the line allow-list
/// is keyed to the file: the same text in another file is still an offence.
#[test]
fn the_walk_honours_exclusions_and_only_them() {
    let files = scanned_files();
    assert!(files.iter().all(|(r, _)| !r.starts_with("petir/")));
    assert!(files.iter().all(|(r, _)| !r.starts_with("boon-lay/src/rustbca/")));
    assert!(files.iter().any(|(r, _)| r.starts_with("boon-lay/src/")));
    assert!(files.iter().any(|(r, _)| r.starts_with("outram-mc-libs/src/")));
    assert!(files.iter().any(|(r, _)| r.starts_with("dhoby-ghaut/src/bin/")));
    let (f, sub, _) = EXCLUDED_LINES[0];
    assert!(allowed(f, sub));
    assert!(!allowed("outram-mc-libs/src/lib.rs", sub));
}
