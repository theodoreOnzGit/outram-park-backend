// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Re-derives every Bessel coefficient in `src/specfunc/bessel.rs` from the
//! vendored GSL C sources and compares them **bit for bit**.
//!
//! # Why a table audit and not just the numerical tests
//!
//! `src/specfunc/bessel.rs` carries 22 coefficient tables, 358 literals in
//! all. The module's own tests check the *functions* against Wronskians and
//! integral representations, which is the stronger check in principle — but a
//! single mistyped digit in a high-order Chebyshev coefficient perturbs the
//! result by far less than those tolerances, and would pass silently while
//! leaving the port no longer a port.
//!
//! This test closes that: it parses both files as text, converts every
//! literal on both sides to `f64`, and requires exact equality. It needs no
//! GSL build and no linker — just the vendored tree.
//!
//! # It skips when the vendored tree is absent, deliberately
//!
//! `upstream_source/GSL` is gitignored (see `upstream_source/README.md`), so a
//! fresh container will not have it and this test would otherwise fail for a
//! reason that has nothing to do with the code. It prints a skip line instead,
//! matching `tests/gsl_cheb_mode_code_to_code.rs`. The functional tests in
//! `src/specfunc/bessel.rs` run unconditionally and are what a clone without
//! the upstream tree relies on.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// Rust `const` name -> (upstream file, upstream array name).
///
/// The full inventory. If a table is added to the module without a row here,
/// `every_rust_table_is_audited` fails.
const TABLES: &[(&str, &str, &str)] = &[
    ("BI0", "bessel_I0.c", "bi0_data"),
    ("AI0", "bessel_I0.c", "ai0_data"),
    ("AI02", "bessel_I0.c", "ai02_data"),
    ("BI1", "bessel_I1.c", "bi1_data"),
    ("AI1", "bessel_I1.c", "ai1_data"),
    ("AI12", "bessel_I1.c", "ai12_data"),
    ("BJ0", "bessel_J0.c", "bj0_data"),
    ("BJ1", "bessel_J1.c", "bj1_data"),
    ("BY0", "bessel_Y0.c", "by0_data"),
    ("BY1", "bessel_Y1.c", "by1_data"),
    ("K0_POLY", "bessel_K0.c", "k0_poly"),
    ("I0_POLY", "bessel_K0.c", "i0_poly"),
    ("AK0", "bessel_K0.c", "ak0_data"),
    ("AK02", "bessel_K0.c", "ak02_data"),
    ("K1_POLY", "bessel_K1.c", "k1_poly"),
    ("I1_POLY", "bessel_K1.c", "i1_poly"),
    ("AK1", "bessel_K1.c", "ak1_data"),
    ("AK12", "bessel_K1.c", "ak12_data"),
    ("BM0", "bessel_amp_phase.c", "bm0_data"),
    ("BTH0", "bessel_amp_phase.c", "bth0_data"),
    ("BM1", "bessel_amp_phase.c", "bm1_data"),
    ("BTH1", "bessel_amp_phase.c", "bth1_data"),
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn upstream_dir() -> PathBuf {
    crate_root().join("upstream_source/GSL/specfunc")
}

/// Strip `/* ... */` comments, which GSL's tables carry inline.
fn strip_block_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Every float literal in `body`, as `f64`. Fortran-style `d` exponents are
/// normalised to `e`; GSL's tables do not use them today but SLATEC-derived
/// files elsewhere in the tree do.
fn parse_floats(body: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let starts = c.is_ascii_digit()
            || ((c == '.' || c == '-' || c == '+')
                && bytes
                    .get(i + 1)
                    .is_some_and(|n| n.is_ascii_digit() || *n == '.'));
        if !starts {
            i += 1;
            continue;
        }
        let start = i;
        if bytes[i] == '-' || bytes[i] == '+' {
            i += 1;
        }
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == '.') {
            i += 1;
        }
        if i < bytes.len() && matches!(bytes[i], 'e' | 'E' | 'd' | 'D') {
            let save = i;
            i += 1;
            if i < bytes.len() && (bytes[i] == '-' || bytes[i] == '+') {
                i += 1;
            }
            if i < bytes.len() && bytes[i].is_ascii_digit() {
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            } else {
                i = save;
            }
        }
        let lit: String = bytes[start..i]
            .iter()
            .collect::<String>()
            .replace(['d', 'D'], "e");
        // A bare "12" from an array dimension is a float too; the caller only
        // ever hands us an initialiser body, so that cannot occur.
        out.push(lit.parse::<f64>().expect("literal in a numeric table"));
    }
    out
}

/// Extract the body delimited by `open`/`close` that follows `decl_needle`.
///
/// C initialisers are braced (`= { ... };`) and Rust array literals are
/// bracketed (`= [ ... ];`), so the delimiters are a parameter. Neither side
/// nests, so a first-`open`-to-first-`close` scan is enough — and a wrong
/// delimiter does not silently half-work, because the length check in
/// [`every_bessel_coefficient_is_bit_identical_to_the_vendored_gsl`] rejects
/// whatever it picks up instead.
fn extract_delimited(text: &str, decl_needle: &str, open: char, close: char) -> Option<String> {
    let at = text.find(decl_needle)?;
    let rest = &text[at..];
    let o = rest.find(open)?;
    let c = rest[o..].find(close)?;
    Some(rest[o + 1..o + c].to_string())
}

fn upstream_table(file: &str, array: &str) -> Option<Vec<f64>> {
    let text = fs::read_to_string(upstream_dir().join(file)).ok()?;
    let text = strip_block_comments(&text);
    // `static double ak0_data[24] = {` / `double bm0_data[21] = {`
    let needle = format!("double {array}[");
    let body = extract_delimited(&text, &needle, '{', '}')?;
    Some(parse_floats(&body))
}

/// The module source with every `//` comment removed.
///
/// Stripping up front rather than per-table matters: one generated table
/// carries an inline note mentioning `[7]`, and a bracket inside a comment
/// terminates the array scan early. The file has no string literal containing
/// `//`, so a line-wise strip is exact here.
fn rust_source() -> String {
    let raw = fs::read_to_string(crate_root().join("src/specfunc/bessel.rs"))
        .expect("src/specfunc/bessel.rs is part of this crate");
    raw.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_table(src: &str, name: &str) -> Option<Vec<f64>> {
    // The declaration itself contains brackets (`const BI0: [f64; 12] = [`),
    // so the scan starts after the `= `, not after the name.
    let at = src.find(&format!("const {name}: [f64; "))?;
    let eq = src[at..].find("] = [")? + at + 4;
    let body = extract_delimited(&src[eq..], "", '[', ']')?;
    Some(parse_floats(&body))
}

/// Every coefficient must be bit-identical to the vendored C.
///
/// Bit-identical, not "close": both sides are decimal literals read by the
/// same `f64` parser, so anything other than equality means a digit differs.
#[test]
fn every_bessel_coefficient_is_bit_identical_to_the_vendored_gsl() {
    let dir = upstream_dir();
    if !dir.is_dir() {
        eprintln!("skipping: vendored GSL not present at {dir:?}");
        return;
    }
    let src = rust_source();
    let mut checked = 0usize;
    for (rust_name, file, array) in TABLES {
        let ours = rust_table(&src, rust_name)
            .unwrap_or_else(|| panic!("no `const {rust_name}` in src/specfunc/bessel.rs"));
        let theirs = upstream_table(file, array)
            .unwrap_or_else(|| panic!("no `{array}` in specfunc/{file}"));
        // `i1_poly` is declared `[7]` upstream with six initialisers and used
        // with n=6, so the Rust side legitimately holds six. Any other length
        // mismatch is a transcription error.
        assert_eq!(
            ours.len(),
            theirs.len(),
            "{rust_name} has {} values, {array} has {}",
            ours.len(),
            theirs.len()
        );
        for (k, (a, b)) in ours.iter().zip(theirs.iter()).enumerate() {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "{rust_name}[{k}] = {a:e} but {file}/{array}[{k}] = {b:e}"
            );
        }
        checked += ours.len();
    }
    assert_eq!(
        checked,
        358,
        "expected 358 coefficients across {} tables, audited {checked}",
        TABLES.len()
    );
}

/// No table may exist in the module without a row in [`TABLES`]. Without this
/// the audit would silently shrink as tables were added.
#[test]
fn every_rust_table_is_audited() {
    let src = rust_source();
    let declared: Vec<String> = src
        .lines()
        .filter_map(|l| l.strip_prefix("const "))
        .filter(|l| l.contains(": [f64; "))
        .filter_map(|l| l.split(':').next())
        .map(str::to_string)
        .collect();
    let audited: BTreeMap<&str, ()> = TABLES.iter().map(|(n, _, _)| (*n, ())).collect();
    for name in &declared {
        assert!(
            audited.contains_key(name.as_str()),
            "`const {name}` is not in TABLES -- add its upstream file and array \
             name so the audit covers it"
        );
    }
    assert_eq!(
        declared.len(),
        TABLES.len(),
        "TABLES lists {} tables, the module declares {}",
        TABLES.len(),
        declared.len()
    );
}

/// Each `cheb_series` in GSL names an `order`, and `cheb_eval_e` reads
/// `c[0 ..= order]`. The port evaluates whole slices instead, which is only
/// correct because every array's length is exactly `order + 1`. If upstream
/// ever shortened a series without shrinking its array, the port would
/// silently include a coefficient GSL ignores.
#[test]
fn every_series_uses_its_whole_array() {
    let dir = upstream_dir();
    if !dir.is_dir() {
        eprintln!("skipping: vendored GSL not present at {dir:?}");
        return;
    }
    // (file, cheb_series name, data array)
    let series: &[(&str, &str, &str)] = &[
        ("bessel_I0.c", "bi0_cs", "bi0_data"),
        ("bessel_I0.c", "ai0_cs", "ai0_data"),
        ("bessel_I0.c", "ai02_cs", "ai02_data"),
        ("bessel_I1.c", "bi1_cs", "bi1_data"),
        ("bessel_I1.c", "ai1_cs", "ai1_data"),
        ("bessel_I1.c", "ai12_cs", "ai12_data"),
        ("bessel_J0.c", "bj0_cs", "bj0_data"),
        ("bessel_J1.c", "bj1_cs", "bj1_data"),
        ("bessel_Y0.c", "by0_cs", "by0_data"),
        ("bessel_Y1.c", "by1_cs", "by1_data"),
        ("bessel_K0.c", "ak0_cs", "ak0_data"),
        ("bessel_K0.c", "ak02_cs", "ak02_data"),
        ("bessel_K1.c", "ak1_cs", "ak1_data"),
        ("bessel_K1.c", "ak12_cs", "ak12_data"),
        (
            "bessel_amp_phase.c",
            "_gsl_sf_bessel_amp_phase_bm0_cs",
            "bm0_data",
        ),
        (
            "bessel_amp_phase.c",
            "_gsl_sf_bessel_amp_phase_bth0_cs",
            "bth0_data",
        ),
        (
            "bessel_amp_phase.c",
            "_gsl_sf_bessel_amp_phase_bm1_cs",
            "bm1_data",
        ),
        (
            "bessel_amp_phase.c",
            "_gsl_sf_bessel_amp_phase_bth1_cs",
            "bth1_data",
        ),
    ];
    for (file, cs, array) in series {
        let text = fs::read_to_string(upstream_dir().join(file)).expect("vendored source");
        let text = strip_block_comments(&text);
        let body = extract_delimited(&text, &format!("cheb_series {cs} ="), '{', '}')
            .unwrap_or_else(|| panic!("no `cheb_series {cs}` in specfunc/{file}"));
        // { data, order, a, b, order_sp }
        let fields: Vec<&str> = body.split(',').map(str::trim).collect();
        let order: usize = fields
            .get(1)
            .and_then(|f| f.parse().ok())
            .unwrap_or_else(|| panic!("cannot read order from `{cs}`: {body:?}"));
        let n = upstream_table(file, array)
            .unwrap_or_else(|| panic!("no `{array}`"))
            .len();
        assert_eq!(
            n,
            order + 1,
            "{cs} has order {order} but {array} holds {n} values -- the port \
             evaluates the whole slice and would include a coefficient GSL \
             does not"
        );
    }
}
