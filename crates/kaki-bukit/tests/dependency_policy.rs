// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Mechanically enforces this crate's two structural promises.
//!
//! Both are stated in `README.md` and `CLAUDE.md`, and a promise that lives
//! only in prose is one an ordinary refactor can break without anyone
//! noticing. This test makes each one fail loudly at the moment it stops being
//! true.
//!
//! # Promise 1: PETIR is the only dependency
//!
//! Every numerical kernel this crate needs comes from `petir`. That is not
//! tidiness — it is what makes the crate `no_std`, what keeps it free of C
//! libraries, and what gives it PETIR's bit-identical-across-platforms
//! transcendentals. One innocuous-looking `rand` or `num-traits` added for
//! convenience would cost all three, and would do it quietly.
//!
//! # Promise 2: PETIR is taken with `default-features = false`
//!
//! PETIR's default feature set includes `transfer-fn`, which pulls in `uom`.
//! Cyclus's kernel is dimensionless bookkeeping — upstream's own
//! `Material::units()` returns the literal string `"kg"` — so there is nothing
//! here for `uom` to type, and carrying it would add a large dependency tree
//! to a crate whose selling point is having almost none.
//!
//! Note this test runs with `std` available (it is an integration test, not
//! part of the `no_std` library) and reads the manifest as text rather than
//! through `cargo metadata`, so it needs no JSON parser and no network.

use std::fs;
use std::path::PathBuf;

fn manifest() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The lines of the `[dependencies]` table, comments and blanks stripped.
fn dependency_lines(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_deps = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]";
            continue;
        }
        if !in_deps || t.is_empty() || t.starts_with('#') {
            continue;
        }
        out.push(t.to_string());
    }
    out
}

#[test]
fn petir_is_the_only_dependency() {
    let deps = dependency_lines(&manifest());
    assert_eq!(
        deps.len(),
        1,
        "this crate must depend on petir and nothing else, but [dependencies] holds: {deps:?}\n\
         If a new dependency is genuinely needed, that is a design decision to raise with the \
         maintainer — see this file's module docs for what it costs."
    );
    assert!(
        deps[0].starts_with("petir"),
        "the single dependency must be petir, found: {}",
        deps[0]
    );
}

#[test]
fn petir_is_taken_without_default_features() {
    let deps = dependency_lines(&manifest());
    let petir = &deps[0];
    assert!(
        petir.contains("default-features = false"),
        "petir must be taken with default-features = false so `uom` stays out of the \
         dependency graph; found: {petir}"
    );
}

#[test]
fn petir_is_a_path_dependency_on_the_in_workspace_crate() {
    let deps = dependency_lines(&manifest());
    let petir = &deps[0];
    assert!(
        petir.contains("path = \"../petir\""),
        "petir must be the in-workspace crate, not a crates.io release, so this port \
         and its numerics move together; found: {petir}"
    );
}

#[test]
fn the_library_declares_no_std() {
    let lib = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let src = fs::read_to_string(&lib).expect("src/lib.rs must be readable");
    assert!(
        src.contains("#![no_std]"),
        "src/lib.rs must carry #![no_std]. This crate's portability claim — embedded, \
         wasm, Android — rests on it, and there is deliberately no `std` feature that \
         turns it off."
    );
    assert!(
        src.contains("#![forbid(unsafe_code)]"),
        "src/lib.rs must carry #![forbid(unsafe_code)]"
    );
}

#[test]
fn the_std_feature_is_additive_only() {
    // The `std` feature exists solely to add `impl std::error::Error`. It must
    // not enable anything in petir, or the no_std build stops being the one
    // that is actually exercised.
    let src = manifest();
    let features: Vec<&str> = src
        .lines()
        .skip_while(|l| l.trim() != "[features]")
        .map(str::trim)
        .filter(|l| l.starts_with("std"))
        .collect();
    assert_eq!(
        features,
        vec!["std = []"],
        "the `std` feature must stay empty; enabling anything through it would make the \
         no_std build a different build from the tested one"
    );
}

#[test]
fn every_source_file_carries_a_provenance_or_licence_header() {
    // The workspace provenance rule: a ported file must name its upstream, and
    // a file that is not a port must still carry the crate's own copyright.
    // This catches a new file added without either.
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut missing = Vec::new();
    let mut stack = vec![src_dir];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src must be readable") {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = fs::read_to_string(&path).expect("source file must be readable");
            let head: String = text.lines().take(15).collect::<Vec<_>>().join("\n");
            let ok = head.contains("PROVENANCE")
                || head.contains("GPL-3.0-only")
                || head.contains("SPDX-License-Identifier");
            if !ok {
                missing.push(path.display().to_string());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these source files carry neither a PROVENANCE block nor a licence header, \
         which the workspace provenance rule requires: {missing:#?}"
    );
}
