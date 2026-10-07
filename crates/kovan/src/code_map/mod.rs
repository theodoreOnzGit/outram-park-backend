//! The **code map** of a Cargo workspace (GitHub #734).
//!
//! The model, layout and SVG moved on 2026-10-06 to the wasm-clean
//! [`kovan_common::code_map`], so web-kovan (`kovan-web`, GitHub #736) can
//! draw the same map in a browser. Everything is re-exported here under its
//! old path (`kovan::code_map::{CodeMap, layout, svg, ...}`). What stays is
//! the one part that does I/O: running `cargo metadata`.

use std::path::Path;

pub use kovan_common::code_map::*;

/// Run `cargo metadata --format-version 1 --no-deps` in `workspace` and
/// return its JSON. Uses `$CARGO` when set (inside `cargo test`), else
/// `cargo` from `PATH`. The one function of the code map that does I/O.
pub fn run_cargo_metadata(workspace: &Path, offline: bool) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(workspace)
        .args(["metadata", "--format-version", "1", "--no-deps"]);
    if offline {
        cmd.arg("--offline");
    }
    let out = cmd.output().map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed in {}: {}",
            workspace.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("cargo metadata output is not UTF-8: {e}"))
}

/// Build the map of the workspace at `workspace` (cargo metadata, then
/// [`CodeMap::from_cargo_metadata`]), errors joined one per line.
pub fn load_workspace(workspace: &Path) -> Result<CodeMap, String> {
    let json = run_cargo_metadata(workspace, false)?;
    CodeMap::from_cargo_metadata(&json).map_err(|e| e.join("\n"))
}

/// The map of ANY workspace or crate (GitHub #780): crates with no tag are
/// untagged placeholders, named in the second value
/// ([`CodeMap::from_cargo_metadata_allowing_untagged`]).
pub fn load_any_workspace(workspace: &Path) -> Result<(CodeMap, Vec<String>), String> {
    let json = run_cargo_metadata(workspace, false)?;
    CodeMap::from_cargo_metadata_allowing_untagged(&json).map_err(|e| e.join("\n"))
}
