// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Grep gate: no platform-libm transcendental on the HEM solver's property
//! path** (stage S3 of gh:#903, epic gh:#896; plan in
//! `crates/tampines/docs/hem-knp-stabilisation/02-petir-coverage-audit.md` §6).
//! The twin of `crates/tampines-steam-tables/tests/no_platform_libm_on_hot_path.rs`.
//!
//! # Why
//!
//! In a `std` crate, `x.exp()`, `x.powf(y)`, `x.powi(n)`, `x.sin()` and
//! friends resolve to the inherent `f64` methods, whose last bit belongs to
//! the target's C library (glibc, MSVC, wasi-libc, Apple libm) or code
//! generator. S3 moved every such call in the files below onto
//! `petir::mathf::RealMath` (`r_exp`, `r_ln`, `r_powf`, `r_powi`, `r_sin`,
//! `r_atan`), one fixed implementation on every target. A missed or newly
//! added raw call compiles and runs silently on the platform route, because
//! the `r_` methods do not shadow `std`. This test catches it.
//!
//! # What it covers
//!
//! Non-test, non-comment lines of [`HOT_PATH_FILES`]: the Helmholtz EOS, the
//! transport models, the `(p,h)`/`(p,T)` flashes, VLE, the ancillaries and the
//! derived-property layer that `rhoPimpleFoam` calls through `state_ph`,
//! `state_pt`, `drho_dp_t`, `state_trho`, `viscosity` and `conductivity`.
//! Everything after an inline `#[cfg(test)] mod … {` is skipped.
//!
//! # What it does NOT cover
//!
//! - `sqrt` and the exact functions (`abs`, `floor`, ...): already identical
//!   on every target.
//! - `transport_corresponding_states.rs`, `fluid.rs`, `single_cv.rs`, the
//!   vendored `openfoam_algorithms` copy, `humid_air`, `incompressibles`,
//!   `mixtures`: outside S3's scope (stages S4 and S7 of the plan).
//!
//! **Since 2026-10-20** the workspace-wide gate
//! `crates/petir/tests/no_platform_libm.rs` covers every crate's `src/`
//! (gh:#985); this narrower gate stays so a failure shows up in this crate.

use std::fs;
use std::path::PathBuf;

/// Files under `src/` on the HEM/KNP property path (S3 scope).
const HOT_PATH_FILES: &[&str] =
    &["eos.rs", "transport.rs", "flash.rs", "vle.rs", "ancillaries.rs", "props.rs"];

/// Raw `f64` methods (and their UFCS spellings) that reach the platform libm
/// or a backend-dependent lowering.
const RAW: &[&str] = &[
    "powi", "powf", "exp", "exp2", "exp_m1", "ln", "ln_1p", "log", "log10", "log2", "cbrt",
    "hypot", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh",
    "asinh", "acosh", "atanh",
];

/// Non-test, non-comment code lines, with any trailing `//` comment removed.
fn code_lines(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let t = raw.trim();
        if t.starts_with("#[cfg(test)]") {
            let mut j = i + 1;
            while j < lines.len() && (lines[j].trim().is_empty() || lines[j].trim().starts_with("#[")) {
                j += 1;
            }
            if j < lines.len() && lines[j].contains("mod ") && lines[j].trim_end().ends_with('{') {
                break;
            }
        }
        if !t.starts_with("//") {
            out.push((i + 1, raw.split("//").next().unwrap_or("").to_string()));
        }
    }
    out
}

fn offences_in(text: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    for (n, code) in code_lines(text) {
        if RAW
            .iter()
            .any(|name| code.contains(&format!(".{name}(")) || code.contains(&format!("f64::{name}(")))
        {
            v.push((n, code.trim().to_string()));
        }
    }
    v
}

#[test]
fn no_platform_libm_on_hot_path() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offences = Vec::new();
    for f in HOT_PATH_FILES {
        let path = src.join(f);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("hot-path file {path:?} unreadable ({e}): update HOT_PATH_FILES"));
        for (n, code) in offences_in(&text) {
            offences.push(format!("{f}:{n}  {code}"));
        }
    }
    assert!(
        offences.is_empty(),
        "raw f64 transcendental(s) on the HEM hot path reach the platform libm. \
         Use petir::mathf::RealMath (`.r_exp(`, `.r_powf(`, `.r_powi(`, ...):\n  {}",
        offences.join("\n  ")
    );
}

/// The gate can fail: it flags a raw call and its UFCS spelling, and ignores
/// comments, `r_` methods and inline test modules.
#[test]
fn the_gate_flags_raw_calls_and_only_raw_calls() {
    let sample = "\
use petir::mathf::RealMath;
fn a(x: f64) -> f64 { x.powf(1.5) }
fn b(x: f64) -> f64 { f64::ln(x) }
fn c(x: f64) -> f64 { x.r_powf(1.5) + x.r_exp() + x.sqrt() } // x.exp() in a comment
// x.sin() in a comment line
#[cfg(test)]
mod tests {
    fn d(x: f64) -> f64 { x.exp() }
}
";
    let hits: Vec<usize> = offences_in(sample).into_iter().map(|(n, _)| n).collect();
    assert_eq!(hits, vec![2, 3]);
}
