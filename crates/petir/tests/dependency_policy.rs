// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Enforces how PETIR obtains a `no_std` `uom`.
//!
//! ~~# The gap this closes~~
//!
//! ~~It cannot for `uom`. The root entry is `uom = "0.38.0"` with default
//! features, and those include `std`. [...] PETIR declares it directly with a
//! duplicated version string, [and this test kept the two byte-identical].~~
//!
//! **CORRECTED 2026-10-20 (gh:#985).** The workspace root's `uom` is now itself
//! `no_std` (`default-features = false`, no `std`), so PETIR inherits it with
//! `workspace = true` like every other member and there is no duplicated
//! version string left to drift. Inheritance can still only ADD features, so
//! the invariant has moved to the ROOT: if the root entry ever lists `std` (or
//! drops `default-features = false`), PETIR's `no_std` build links `std`. These
//! tests pin that, plus the inheritance itself.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("petir must live at <workspace>/crates/petir")
        .to_path_buf()
}

fn uom_line(manifest: &str, what: &str) -> String {
    manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("uom") && l.contains('='))
        .unwrap_or_else(|| panic!("{what} must declare uom"))
        .to_string()
}

/// PETIR inherits `uom` from the workspace instead of pinning it locally.
#[test]
fn uom_is_inherited_from_the_workspace() {
    let root = workspace_root();
    let petir_manifest = fs::read_to_string(root.join("crates/petir/Cargo.toml"))
        .expect("petir Cargo.toml must be readable");
    let line = uom_line(&petir_manifest, "crates/petir/Cargo.toml");
    assert!(
        line.contains("workspace = true"),
        "PETIR's uom must be inherited (`workspace = true`); the root entry is \
         no_std since gh:#985. Line was: {line}"
    );
}

/// The `uom` PETIR inherits must stay `no_std`.
///
/// The whole crate's portability claim rests on it: a `uom` with `std` on would
/// still compile on the host and quietly fail the bare-metal and wasm builds,
/// and the failure would surface as a linker error far from its cause. Since
/// PETIR inherits (gh:#985), the line that must stay `no_std` is the ROOT's.
#[test]
fn petir_uom_is_no_std() {
    let root = workspace_root();
    let root_manifest = fs::read_to_string(root.join("Cargo.toml"))
        .expect("workspace Cargo.toml must be readable");
    let uom_line = uom_line(&root_manifest, "root [workspace.dependencies]");
    let uom_line = uom_line.as_str();

    assert!(
        uom_line.contains("default-features = false"),
        "The workspace uom must set default-features = false -- uom's default features \
         include `std`. Line was: {uom_line}"
    );
    assert!(
        !uom_line.contains("\"std\""),
        "The workspace uom must not enable the `std` feature. Line was: {uom_line}"
    );
}

/// `libm` must be inherited, not pinned locally.
///
/// It has no feature-narrowing problem, so there is no excuse for it to dodge
/// the single-source-of-truth rule ~~the way `uom` must~~ (since gh:#985 `uom`
/// is inherited too).
#[test]
fn libm_is_inherited_from_the_workspace() {
    let root = workspace_root();
    let petir_manifest = fs::read_to_string(root.join("crates/petir/Cargo.toml"))
        .expect("petir Cargo.toml must be readable");

    let libm_line = petir_manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("libm"))
        .expect("crates/petir/Cargo.toml must declare libm");

    assert!(
        libm_line.contains("workspace = true"),
        "libm must be inherited with `libm.workspace = true`, not pinned here. \
         Line was: {libm_line}"
    );
}
