// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Grep gate: no platform-libm transcendental on the HEM solver's property
//! path** (stage S2 of gh:#903, epic gh:#896; plan in
//! `crates/tampines/docs/hem-knp-stabilisation/02-petir-coverage-audit.md` §6).
//!
//! # Why
//!
//! In a `std` crate, `x.powi(n)`, `x.exp()`, `x.atan()` and friends resolve to
//! the inherent `f64` methods, whose last bit is set by the target's C library
//! or code generator (glibc, MSVC, wasi-libc, Apple libm). The S0 negative
//! control (`tests/cross_platform_determinism.rs`) measured that difference
//! between x86_64-linux and wasm32-wasip1 in `lambda_ph` (std `atan`) and
//! `p_rho_h` (std `ln`/`exp`). S2 moved every such call on the solver path
//! onto `petir::mathf::RealMath` (`r_powi`, `r_atan`, `r_exp`, `r_ln`, ...),
//! which is one fixed implementation on every target.
//!
//! A missed or newly added raw call **compiles and runs silently on the
//! platform route**, because the `r_` methods do not shadow `std`. Nothing
//! numerical would catch it on a single target. This test does.
//!
//! # What it covers
//!
//! Non-test, non-comment lines of every `.rs` file under the directories in
//! [`HOT_PATH_DIRS`]: the IF97 regions 1-5, the backward `(p,h)`/`(p,s)`/`(h,s)`
//! equations, viscosity and thermal conductivity, the functional flash
//! interfaces the solver calls (`ph_flash_eqm`, `pt_flash_eqm`,
//! `rho_h_flash_eqm`, ...), and the choked-flow nozzle code used by the
//! Edwards break boundary condition. Files under a `tests/` directory, files
//! named `tests.rs`, and everything after an inline `#[cfg(test)] mod … {` are
//! test code and are skipped.
//!
//! # What it does NOT cover
//!
//! - `sqrt`, `abs`, `floor`, `ceil`, `round`, `mul_add`: IEEE-754 correctly
//!   rounded or exact, so already identical everywhere. Not flagged.
//! - Off-path modules (`steam_turbine_equations` outside the nozzles,
//!   `backward_eqn_chebyshev_experimental`, `dielectric_constant`,
//!   `surface_tension`, `openfoam_algorithms`). Stage S7 of the plan.
//! - A call through a helper whose name is not in [`RAW`]. The gate is a text
//!   match, not a call-graph analysis.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories under `src/` on the HEM/KNP property path.
const HOT_PATH_DIRS: &[&str] = &[
    "region_1_subcooled_liquid",
    "region_2_vapour",
    "region_3_single_phase_plus_supercritical_steam",
    "region_4_vap_liq_equilibrium",
    "region_5_steam_at_800_plus_degc",
    "backward_eqn_ph_region_1_to_4",
    "backward_eqn_ps_region_1_to_4",
    "backward_eqn_hs_region_1_to_4",
    "dynamic_viscosity",
    "thermal_conductivity",
    "interfaces/functional_programming",
    "steam_turbine_equations/converging_diverging_nozzles",
];

/// Raw `f64` methods (and their UFCS spellings) that reach the platform libm
/// or a backend-dependent lowering.
const RAW: &[&str] = &[
    "powi", "powf", "exp", "exp2", "exp_m1", "ln", "ln_1p", "log", "log10", "log2", "cbrt",
    "hypot", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh",
    "asinh", "acosh", "atanh",
];

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
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
            while j < lines.len() && (lines[j].trim().is_empty() || lines[j].trim().starts_with("#[")) {
                j += 1;
            }
            if j < lines.len() && lines[j].contains("mod ") && lines[j].trim_end().ends_with('{') {
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

fn offences_in(text: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    for (n, code) in code_lines(text) {
        for name in RAW {
            let method = format!(".{name}(");
            let ufcs = format!("f64::{name}(");
            if code.contains(&method) || code.contains(&ufcs) {
                v.push((n, code.trim().to_string()));
                break;
            }
        }
    }
    v
}

#[test]
fn no_platform_libm_on_hot_path() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for d in HOT_PATH_DIRS {
        let dir = src.join(d);
        assert!(dir.is_dir(), "hot-path directory {dir:?} is missing: update HOT_PATH_DIRS");
        rs_files(&dir, &mut files);
    }
    assert!(files.len() > 50, "found only {} files; the walk is broken", files.len());

    let mut offences = Vec::new();
    for f in &files {
        let text = fs::read_to_string(f).expect("readable source file");
        let rel = f.strip_prefix(&src).unwrap_or(f).display().to_string();
        for (n, code) in offences_in(&text) {
            offences.push(format!("{rel}:{n}  {code}"));
        }
    }
    assert!(
        offences.is_empty(),
        "raw f64 transcendental(s) on the HEM hot path reach the platform libm. \
         Use petir::mathf::RealMath (`.r_powi(`, `.r_exp(`, `.r_atan(`, ...):\n  {}",
        offences.join("\n  ")
    );
}

/// The gate can fail: it flags a raw call, its UFCS spelling and a raw call in
/// non-test code, and ignores comments, `r_` methods and inline test modules.
#[test]
fn the_gate_flags_raw_calls_and_only_raw_calls() {
    let sample = "\
use petir::mathf::RealMath;
fn a(x: f64) -> f64 { x.powi(2) }
fn b(x: f64) -> f64 { f64::exp(x) }
fn c(x: f64) -> f64 { x.r_powi(2) + x.r_atan() + x.sqrt() } // x.atan() in a comment
// x.ln() in a comment line
#[cfg(test)]
mod tests {
    fn d(x: f64) -> f64 { x.cbrt() }
}
";
    let hits: Vec<usize> = offences_in(sample).into_iter().map(|(n, _)| n).collect();
    assert_eq!(hits, vec![2, 3]);
}
