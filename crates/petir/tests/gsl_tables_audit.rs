// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Re-derives every GSL-ported coefficient table in `src/specfunc/` from the
//! vendored GSL C sources and compares them **bit for bit**.
//!
//! Covers `bessel.rs`, `psi.rs`, `zeta.rs`, `debye.rs`, `airy.rs`,
//! `lambert.rs`, `clausen.rs`, `transport.rs`, `atanint.rs`,
//! `synchrotron.rs` and `fermi_dirac.rs` — 88 file-scope tables plus two
//! local ones, **2080 literals**.
//!
//! # Why a table audit and not just the numerical tests
//!
//! Each module's own tests check the *functions* against identities that
//! share no table with them — Wronskians, recurrences, reflection formulas,
//! integral representations. That is the stronger check in principle, but a
//! single mistyped digit in a high-order Chebyshev coefficient perturbs the
//! result by far less than those tolerances, and would pass silently while
//! leaving the port no longer a port.
//!
//! This test closes that: it parses both files as text, converts every
//! literal on both sides to `f64`, and requires exact equality. It needs no
//! GSL build and no linker — just the vendored tree.
//!
//! # Three of GSL's table entries are macro expressions
//!
//! `psi_table[1]` is `-M_EULER`, `psi_1_table[1]` is `M_PI*M_PI/6.0`, and
//! `eta_pos_int_table[1]` is `M_LN2`. The Rust side carries the `f64` a C
//! compiler produces for each; [`MACROS`] performs the same substitution on
//! the C text before parsing, so the comparison stays bit-for-bit rather than
//! being exempted.
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

/// GSL macro expressions that appear inside the tables, with the exact `f64`
/// a C compiler produces for each. Applied to the C text before parsing.
///
/// `M_PI*M_PI/6.0` must come first: the substitution is textual, so replacing
/// `M_PI` alone would leave `3.14159...*3.14159.../6.0`, which parses as
/// three literals rather than one.
const MACROS: &[(&str, &str)] = &[
    ("M_PI*M_PI/6.0", "1.6449340668482264"),
    (
        "M_EULER",
        "0.57721566490153286060651209008240243104215933593992",
    ),
    (
        "M_LN2",
        "0.69314718055994530941723212145817656807550013436026",
    ),
    (
        "M_PI",
        "3.14159265358979323846264338327950288419716939937511",
    ),
];

/// (Rust source file, Rust `const` name, upstream C file, upstream array).
///
/// The full inventory. If a table is added to one of these modules without a
/// row here, `every_rust_table_is_audited` fails.
const TABLES: &[(&str, &str, &str, &str)] = &[
    ("bessel.rs", "BI0", "bessel_I0.c", "bi0_data"),
    ("bessel.rs", "AI0", "bessel_I0.c", "ai0_data"),
    ("bessel.rs", "AI02", "bessel_I0.c", "ai02_data"),
    ("bessel.rs", "BI1", "bessel_I1.c", "bi1_data"),
    ("bessel.rs", "AI1", "bessel_I1.c", "ai1_data"),
    ("bessel.rs", "AI12", "bessel_I1.c", "ai12_data"),
    ("bessel.rs", "BJ0", "bessel_J0.c", "bj0_data"),
    ("bessel.rs", "BJ1", "bessel_J1.c", "bj1_data"),
    ("bessel.rs", "BY0", "bessel_Y0.c", "by0_data"),
    ("bessel.rs", "BY1", "bessel_Y1.c", "by1_data"),
    ("bessel.rs", "K0_POLY", "bessel_K0.c", "k0_poly"),
    ("bessel.rs", "I0_POLY", "bessel_K0.c", "i0_poly"),
    ("bessel.rs", "AK0", "bessel_K0.c", "ak0_data"),
    ("bessel.rs", "AK02", "bessel_K0.c", "ak02_data"),
    ("bessel.rs", "K1_POLY", "bessel_K1.c", "k1_poly"),
    ("bessel.rs", "I1_POLY", "bessel_K1.c", "i1_poly"),
    ("bessel.rs", "AK1", "bessel_K1.c", "ak1_data"),
    ("bessel.rs", "AK12", "bessel_K1.c", "ak12_data"),
    ("bessel.rs", "BM0", "bessel_amp_phase.c", "bm0_data"),
    ("bessel.rs", "BTH0", "bessel_amp_phase.c", "bth0_data"),
    ("bessel.rs", "BM1", "bessel_amp_phase.c", "bm1_data"),
    ("bessel.rs", "BTH1", "bessel_amp_phase.c", "bth1_data"),
    ("psi.rs", "R1PY", "psi.c", "r1py_data"),
    ("psi.rs", "PSI_CS", "psi.c", "psics_data"),
    ("psi.rs", "APSI_CS", "psi.c", "apsics_data"),
    ("psi.rs", "PSI_TABLE", "psi.c", "psi_table"),
    ("psi.rs", "PSI_1_TABLE", "psi.c", "psi_1_table"),
    ("zeta.rs", "ZETA_XLT1", "zeta.c", "zeta_xlt1_data"),
    ("zeta.rs", "ZETA_XGT1", "zeta.c", "zeta_xgt1_data"),
    ("zeta.rs", "ZETAM1_INTER", "zeta.c", "zetam1_inter_data"),
    ("zeta.rs", "HZETA_C", "zeta.c", "hzeta_c"),
    ("zeta.rs", "ZETA_NEG_INT", "zeta.c", "zeta_neg_int_table"),
    (
        "zeta.rs",
        "ZETAM1_POS_INT",
        "zeta.c",
        "zetam1_pos_int_table",
    ),
    ("zeta.rs", "ETA_POS_INT", "zeta.c", "eta_pos_int_table"),
    ("zeta.rs", "ETA_NEG_INT", "zeta.c", "eta_neg_int_table"),
    ("debye.rs", "ADEB1", "debye.c", "adeb1_data"),
    ("debye.rs", "ADEB2", "debye.c", "adeb2_data"),
    ("debye.rs", "ADEB3", "debye.c", "adeb3_data"),
    ("debye.rs", "ADEB4", "debye.c", "adeb4_data"),
    ("debye.rs", "ADEB5", "debye.c", "adeb5_data"),
    ("debye.rs", "ADEB6", "debye.c", "adeb6_data"),
    ("airy.rs", "AM21", "airy.c", "am21_data"),
    ("airy.rs", "ATH1", "airy.c", "ath1_data"),
    ("airy.rs", "AM22", "airy.c", "am22_data"),
    ("airy.rs", "ATH2", "airy.c", "ath2_data"),
    ("airy.rs", "AIF", "airy.c", "ai_data_f"),
    ("airy.rs", "AIG", "airy.c", "ai_data_g"),
    ("airy.rs", "BIF", "airy.c", "data_bif"),
    ("airy.rs", "BIG", "airy.c", "data_big"),
    ("airy.rs", "BIF2", "airy.c", "data_bif2"),
    ("airy.rs", "BIG2", "airy.c", "data_big2"),
    ("airy.rs", "AIP", "airy.c", "data_aip"),
    ("airy.rs", "BIP", "airy.c", "data_bip"),
    ("airy.rs", "BIP2", "airy.c", "data_bip2"),
    ("clausen.rs", "ACLAUS", "clausen.c", "aclaus_data"),
    ("transport.rs", "TRANSPORT2", "transport.c", "transport2_data"),
    ("transport.rs", "TRANSPORT3", "transport.c", "transport3_data"),
    ("transport.rs", "TRANSPORT4", "transport.c", "transport4_data"),
    ("transport.rs", "TRANSPORT5", "transport.c", "transport5_data"),
    ("atanint.rs", "ATANINT", "atanint.c", "atanint_data"),
    ("synchrotron.rs", "SYNCH1", "synchrotron.c", "synchrotron1_data"),
    ("synchrotron.rs", "SYNCH2", "synchrotron.c", "synchrotron2_data"),
    ("synchrotron.rs", "SYNCH1A", "synchrotron.c", "synchrotron1a_data"),
    ("synchrotron.rs", "SYNCH21", "synchrotron.c", "synchrotron21_data"),
    ("synchrotron.rs", "SYNCH22", "synchrotron.c", "synchrotron22_data"),
    ("synchrotron.rs", "SYNCH2A", "synchrotron.c", "synchrotron2a_data"),
    ("fermi_dirac.rs", "FD_1_A", "fermi_dirac.c", "fd_1_a_data"),
    ("fermi_dirac.rs", "FD_1_B", "fermi_dirac.c", "fd_1_b_data"),
    ("fermi_dirac.rs", "FD_1_C", "fermi_dirac.c", "fd_1_c_data"),
    ("fermi_dirac.rs", "FD_1_D", "fermi_dirac.c", "fd_1_d_data"),
    ("fermi_dirac.rs", "FD_1_E", "fermi_dirac.c", "fd_1_e_data"),
    ("fermi_dirac.rs", "FD_2_A", "fermi_dirac.c", "fd_2_a_data"),
    ("fermi_dirac.rs", "FD_2_B", "fermi_dirac.c", "fd_2_b_data"),
    ("fermi_dirac.rs", "FD_2_C", "fermi_dirac.c", "fd_2_c_data"),
    ("fermi_dirac.rs", "FD_2_D", "fermi_dirac.c", "fd_2_d_data"),
    ("fermi_dirac.rs", "FD_2_E", "fermi_dirac.c", "fd_2_e_data"),
    ("fermi_dirac.rs", "FD_MHALF_A", "fermi_dirac.c", "fd_mhalf_a_data"),
    ("fermi_dirac.rs", "FD_MHALF_B", "fermi_dirac.c", "fd_mhalf_b_data"),
    ("fermi_dirac.rs", "FD_MHALF_C", "fermi_dirac.c", "fd_mhalf_c_data"),
    ("fermi_dirac.rs", "FD_MHALF_D", "fermi_dirac.c", "fd_mhalf_d_data"),
    ("fermi_dirac.rs", "FD_HALF_A", "fermi_dirac.c", "fd_half_a_data"),
    ("fermi_dirac.rs", "FD_HALF_B", "fermi_dirac.c", "fd_half_b_data"),
    ("fermi_dirac.rs", "FD_HALF_C", "fermi_dirac.c", "fd_half_c_data"),
    ("fermi_dirac.rs", "FD_HALF_D", "fermi_dirac.c", "fd_half_d_data"),
    ("fermi_dirac.rs", "FD_3HALF_A", "fermi_dirac.c", "fd_3half_a_data"),
    ("fermi_dirac.rs", "FD_3HALF_B", "fermi_dirac.c", "fd_3half_b_data"),
    ("fermi_dirac.rs", "FD_3HALF_C", "fermi_dirac.c", "fd_3half_c_data"),
    ("fermi_dirac.rs", "FD_3HALF_D", "fermi_dirac.c", "fd_3half_d_data"),
];

/// `TWOPI_POW` in `zeta.rs` is a LOCAL array inside `gsl_sf_zeta_e`'s
/// reflection branch rather than a file-scope declaration, so the parser
/// above cannot find it by name. It is audited by
/// [`the_local_twopi_table_matches_upstream`] instead, which searches the
/// function body. Listed here so `every_rust_table_is_audited` knows it is
/// not an omission.
///
/// **That test did not exist until 2026-09-19.** The comment above claimed it
/// did, `AUDITED_ELSEWHERE` made `every_rust_table_is_audited` accept the
/// table as covered on the strength of the claim, and nothing checked the 18
/// coefficients at all. This is exactly the failure mode an "audited
/// elsewhere" list invites: it is an assertion that something else is doing
/// the work. The test below now does it, and the entry is only honest
/// because of that.
const AUDITED_ELSEWHERE: &[(&str, &str)] = &[
    ("zeta.rs", "TWOPI_POW"),
    ("lambert.rs", "SERIES_C"),
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
    // `static double ak0_data[24] = {` / `double bm0_data[21] = {`.
    // psi.c writes `r1py_data[]` with no dimension, so the bracket contents
    // are skipped rather than matched.
    let at = text.find(&format!("double {array}["))?;
    let body = extract_delimited(&text[at..], "", '{', '}')?;
    Some(parse_floats(&expand_macros(&body)))
}

/// Replace every GSL macro in [`MACROS`] with its `f64` spelling.
fn expand_macros(body: &str) -> String {
    let mut out = body.to_string();
    for (macro_name, value) in MACROS {
        out = out.replace(macro_name, value);
    }
    assert!(
        !out.contains("M_"),
        "an unhandled GSL macro remains in a table: {:?}",
        out.split_whitespace()
            .find(|t| t.contains("M_"))
            .unwrap_or("?")
    );
    out
}

/// The module source with every `//` comment removed.
///
/// Stripping up front rather than per-table matters: one generated table
/// carries an inline note mentioning `[7]`, and a bracket inside a comment
/// terminates the array scan early. The file has no string literal containing
/// `//`, so a line-wise strip is exact here.
fn rust_source(module: &str) -> String {
    let raw = fs::read_to_string(crate_root().join("src/specfunc").join(module))
        .unwrap_or_else(|e| panic!("src/specfunc/{module} is part of this crate: {e}"));
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
///
/// # What this deliberately does NOT catch
///
/// It compares the parsed `f64`, not the text. Several GSL tables carry ~30
/// decimal digits where `f64` holds about 17, and a change past the 17th is
/// invisible here — correctly so, because it does not change the compiled
/// constant either. Verified by injection: altering the 30th digit of
/// `zetam1_pos_int_table[2]` passes, altering the 16th fails with
/// `PSI_TABLE[2] = 4.2278433509846725e-1 but psi.c/psi_table[2] =
/// 4.2278433509846713e-1`.
///
/// So this is a guarantee about the *values* the port compiles to, which is
/// the thing that matters. If the trailing digits themselves ever need to
/// match — for a higher-precision port — that is a different test.
#[test]
fn every_coefficient_is_bit_identical_to_the_vendored_gsl() {
    let dir = upstream_dir();
    if !dir.is_dir() {
        eprintln!("skipping: vendored GSL not present at {dir:?}");
        return;
    }
    let mut checked = 0usize;
    for (module, rust_name, file, array) in TABLES {
        let src = rust_source(module);
        let ours = rust_table(&src, rust_name)
            .unwrap_or_else(|| panic!("no `const {rust_name}` in src/specfunc/{module}"));
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
        2080,
        "expected 2080 coefficients across {} tables, audited {checked}",
        TABLES.len()
    );
}

/// No table may exist in the module without a row in [`TABLES`]. Without this
/// the audit would silently shrink as tables were added.
#[test]
fn every_rust_table_is_audited() {
    let mut total_declared = 0usize;
    for module in [
        "bessel.rs",
        "psi.rs",
        "zeta.rs",
        "debye.rs",
        "airy.rs",
        "lambert.rs",
        "clausen.rs",
        "transport.rs",
        "atanint.rs",
        "synchrotron.rs",
        "fermi_dirac.rs",
    ] {
        let src = rust_source(module);
        let declared: Vec<String> = src
            .lines()
            .filter_map(|l| l.strip_prefix("const "))
            .filter(|l| l.contains(": [f64; "))
            .filter_map(|l| l.split(':').next())
            .map(str::to_string)
            .collect();
        let audited: BTreeMap<(&str, &str), ()> = TABLES
            .iter()
            .map(|(m, n, _, _)| ((*m, *n), ()))
            .chain(AUDITED_ELSEWHERE.iter().map(|(m, n)| ((*m, *n), ())))
            .collect();
        for name in &declared {
            assert!(
                audited.contains_key(&(module, name.as_str())),
                "`const {name}` in src/specfunc/{module} is not in TABLES -- add \
                 its upstream file and array name so the audit covers it, or \
                 list it in AUDITED_ELSEWHERE with the test that does"
            );
        }
        total_declared += declared.len();
    }
    assert_eq!(
        total_declared,
        TABLES.len() + AUDITED_ELSEWHERE.len(),
        "TABLES + AUDITED_ELSEWHERE list {} tables, the modules declare {total_declared}",
        TABLES.len() + AUDITED_ELSEWHERE.len()
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
    //
    // `zeta.c`'s `zetam1_inter_cs` is DELIBERATELY absent: it is the one
    // series in these modules whose order is smaller than its array, and
    // `zeta::tests::the_intermediate_series_stops_at_the_order_gsl_declares`
    // is what covers it. `the_one_series_shorter_than_its_array` below pins
    // that it really is the only one.
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
        ("psi.c", "r1py_cs", "r1py_data"),
        ("psi.c", "psi_cs", "psics_data"),
        ("psi.c", "apsi_cs", "apsics_data"),
        ("zeta.c", "zeta_xlt1_cs", "zeta_xlt1_data"),
        ("zeta.c", "zeta_xgt1_cs", "zeta_xgt1_data"),
        ("debye.c", "adeb1_cs", "adeb1_data"),
        ("debye.c", "adeb2_cs", "adeb2_data"),
        ("debye.c", "adeb3_cs", "adeb3_data"),
        ("debye.c", "adeb4_cs", "adeb4_data"),
        ("debye.c", "adeb5_cs", "adeb5_data"),
        ("debye.c", "adeb6_cs", "adeb6_data"),
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

/// `zetam1_inter_cs` is the **only** series in these modules whose declared
/// order is smaller than its data array.
///
/// `every_series_uses_its_whole_array` deliberately omits it, and the port
/// slices it to 23 of 24 by hand. This checks that the omission is a single
/// known exception rather than a growing list — if upstream shortened another
/// series, the corresponding port would silently include a coefficient GSL
/// ignores and nothing else would notice.
#[test]
fn the_one_series_shorter_than_its_array() {
    let dir = upstream_dir();
    if !dir.is_dir() {
        eprintln!("skipping: vendored GSL not present at {dir:?}");
        return;
    }
    let text = strip_block_comments(
        &fs::read_to_string(dir.join("zeta.c")).expect("vendored specfunc/zeta.c"),
    );
    let body = extract_delimited(&text, "cheb_series zetam1_inter_cs =", '{', '}')
        .expect("no `cheb_series zetam1_inter_cs` in specfunc/zeta.c");
    let fields: Vec<&str> = body.split(',').map(str::trim).collect();
    let order: usize = fields
        .get(1)
        .and_then(|f| f.parse().ok())
        .expect("cannot read zetam1_inter_cs's order");
    let n = upstream_table("zeta.c", "zetam1_inter_data")
        .expect("no `zetam1_inter_data`")
        .len();
    assert_eq!(order, 22, "zetam1_inter_cs's order moved to {order}");
    assert_eq!(n, 24, "zetam1_inter_data now holds {n} values");
    assert!(
        order + 1 < n,
        "zetam1_inter_cs no longer has a shorter order than its array; if \
         upstream fixed this, `zeta::zetam1_intermediate` should stop slicing"
    );
}

/// **`TWOPI_POW` against upstream's local `twopi_pow[18]`.**
///
/// The array lives inside `gsl_sf_zeta_e`'s reflection branch in `zeta.c`
/// rather than at file scope, so `every_coefficient_is_bit_identical_to_the_
/// vendored_gsl`'s parser cannot reach it by name. This one searches the
/// function body for the declaration instead and compares bit-for-bit, as the
/// file-scope audit does.
///
/// It is the check `AUDITED_ELSEWHERE` has always claimed existed and, until
/// 2026-09-19, did not — see the comment on that constant.
#[test]
fn the_local_twopi_table_matches_upstream() {
    let c = fs::read_to_string(upstream_dir().join("zeta.c")).expect("vendored specfunc/zeta.c");
    let at = c
        .find("const double twopi_pow[18] = {")
        .expect("upstream's local twopi_pow[18] declaration moved or was renamed");
    let rest = &c[at..];
    let open = rest.find('{').expect("no { after the declaration");
    let close = rest[open..].find('}').expect("no } closing the initialiser");
    let upstream: Vec<f64> = rest[open + 1..open + close]
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.parse::<f64>()
                .unwrap_or_else(|_| panic!("not a literal in twopi_pow: {t:?}"))
        })
        .collect();

    let r = rust_source("zeta.rs");
    let ours = rust_table(&r, "TWOPI_POW")
        .expect("TWOPI_POW is no longer declared in src/specfunc/zeta.rs");

    assert_eq!(
        ours.len(),
        upstream.len(),
        "TWOPI_POW has {} coefficients, upstream's twopi_pow has {}",
        ours.len(),
        upstream.len()
    );
    assert_eq!(upstream.len(), 18, "upstream twopi_pow is declared [18]");
    for (i, (a, b)) in ours.iter().zip(upstream.iter()).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "TWOPI_POW[{i}]: ours {a:e}, upstream {b:e}"
        );
    }
}

/// **`SERIES_C` against upstream's local `c[12]` in `series_eval`.**
///
/// Like `TWOPI_POW`, the array is declared inside a function in `lambert.c`
/// rather than at file scope, so the name-based parser cannot reach it.
///
/// These twelve coefficients carry ~30 decimal digits where `f64` holds about
/// 17, so the comparison is on the parsed value, as everywhere else in this
/// file — see `every_coefficient_is_bit_identical_to_the_vendored_gsl`'s note
/// on what that deliberately does not catch.
#[test]
fn the_local_lambert_series_matches_upstream() {
    let c = fs::read_to_string(upstream_dir().join("lambert.c"))
        .expect("vendored specfunc/lambert.c");
    let at = c
        .find("static const double c[12] = {")
        .expect("upstream's local c[12] in series_eval moved or was renamed");
    let rest = &c[at..];
    let open = rest.find('{').expect("no { after the declaration");
    let close = rest[open..].find('}').expect("no } closing the initialiser");
    let upstream = parse_floats(&strip_block_comments(&rest[open + 1..open + close]));

    let r = rust_source("lambert.rs");
    let ours =
        rust_table(&r, "SERIES_C").expect("SERIES_C is no longer declared in specfunc/lambert.rs");

    assert_eq!(ours.len(), 12, "SERIES_C has {} coefficients", ours.len());
    assert_eq!(
        upstream.len(),
        12,
        "upstream c[12] parsed to {} values",
        upstream.len()
    );
    for (i, (a, b)) in ours.iter().zip(upstream.iter()).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "SERIES_C[{i}]: ours {a:e}, upstream {b:e}"
        );
    }
}
