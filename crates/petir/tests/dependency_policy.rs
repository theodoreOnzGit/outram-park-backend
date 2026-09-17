// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Enforces the one place PETIR cannot follow the workspace's
//! single-source-of-truth dependency rule mechanically.
//!
//! # The gap this closes
//!
//! The workspace rule is that every third-party version lives in the root
//! `[workspace.dependencies]` and members inherit it with `<dep>.workspace =
//! true`, so versions cannot drift. PETIR obeys that for `libm`.
//!
//! It cannot for `uom`. The root entry is `uom = "0.38.0"` with default
//! features, and those include `std`. Cargo's workspace inheritance can only
//! ADD features to an inherited dependency, never remove them — a member
//! writing `default-features = false` has it silently ignored (Cargo warns, and
//! plans to make it a hard error). There is therefore no way to obtain a
//! `no_std` `uom` through inheritance, so PETIR declares it directly with a
//! duplicated version string.
//!
//! A duplicated version string is exactly the drift the workspace rule exists
//! to prevent, so this test re-imposes it: the two must stay byte-identical,
//! and a bump to one that is not matched in the other fails here rather than
//! six months later as a mystery duplicate in `Cargo.lock`.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("petir must live at <workspace>/crates/petir")
        .to_path_buf()
}

/// Pull the version string out of a `uom = ...` line, whatever its form.
fn uom_version(line: &str) -> Option<String> {
    let after_eq = line.split_once('=')?.1.trim();
    // Either `"0.38.0"` or `{ version = "0.38.0", ... }`.
    let quoted = if after_eq.starts_with('{') {
        after_eq.split_once("version")?.1.split_once('=')?.1.trim()
    } else {
        after_eq
    };
    let inner = quoted.trim_start_matches('"');
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

#[test]
fn uom_version_matches_workspace() {
    let root = workspace_root();

    let root_manifest = fs::read_to_string(root.join("Cargo.toml"))
        .expect("workspace Cargo.toml must be readable");
    let petir_manifest = fs::read_to_string(root.join("crates/petir/Cargo.toml"))
        .expect("petir Cargo.toml must be readable");

    let root_uom = root_manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("uom") && l.contains('='))
        .and_then(uom_version)
        .expect("root [workspace.dependencies] must declare uom");

    let petir_uom = petir_manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("uom") && l.contains('='))
        .and_then(uom_version)
        .expect("crates/petir/Cargo.toml must declare uom");

    assert_eq!(
        root_uom, petir_uom,
        "uom version drifted between the workspace root ({root_uom}) and PETIR \
         ({petir_uom}).\n\
         PETIR declares uom crate-locally because a no_std uom cannot be obtained \
         through workspace inheritance (feature narrowing is impossible), so the \
         version string is duplicated on purpose. Bump BOTH in the same change."
    );
}

/// PETIR's `uom` must stay `no_std`.
///
/// The whole crate's portability claim rests on it: a `uom` with `std` on would
/// still compile on the host and quietly fail the bare-metal and wasm builds,
/// and the failure would surface as a linker error far from its cause.
#[test]
fn petir_uom_is_no_std() {
    let root = workspace_root();
    let petir_manifest = fs::read_to_string(root.join("crates/petir/Cargo.toml"))
        .expect("petir Cargo.toml must be readable");

    let uom_line = petir_manifest
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("uom") && l.contains('='))
        .expect("crates/petir/Cargo.toml must declare uom");

    assert!(
        uom_line.contains("default-features = false"),
        "PETIR's uom must set default-features = false -- uom's default features \
         include `std`. Line was: {uom_line}"
    );
    assert!(
        !uom_line.contains("\"std\""),
        "PETIR's uom must not enable the `std` feature. Line was: {uom_line}"
    );
}

/// `libm` must be inherited, not pinned locally.
///
/// It has no feature-narrowing problem, so there is no excuse for it to dodge
/// the single-source-of-truth rule the way `uom` must.
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
