//! `kovan api-docs` — regenerate a crate's `docs/api.md`, the committed
//! markdown mirror of its public API.
//!
//! A Rust port of `scripts/gen_api_docs.py`, which it **replaces** (retired
//! 2026-08-14). The pipeline is unchanged:
//!
//! ```text
//! cargo +nightly doc --output-format json  ->  target/doc/<crate>.json
//!                        rustdoc-md        ->  crates/<crate>/docs/api.md
//! ```
//!
//! # Why the Python went
//!
//! The chain used to be `kovan` (Rust) → `python3` → `cargo` + `rustdoc-md`: a
//! Rust binary spawning an interpreter in order to spawn Rust tooling. That
//! inverts the direction epic `op-yz7b` already set when it deleted
//! `docs/historian/*.py` and `token_usage.py` in favour of `kovan-metrics`, so
//! the toolchain would need no Python interpreter. The reason recorded in
//! `.githooks/kovan-bin.sh` is concrete rather than aesthetic: on Windows,
//! `python3` routinely resolves to a Microsoft Store alias stub that prints an
//! advert and exits, which silently turned the token hooks into no-ops.
//!
//! # Why nightly, and why that is not alarming
//!
//! **`rustdoc-md` is an ordinary stable binary** that reads a JSON file. The
//! nightly requirement belongs one step upstream, to rustdoc's
//! `--output-format json`, which is still gated behind `-Z unstable-options`.
//! Verified 2026-08-14: `cargo +stable doc --output-format json` fails with
//! *"unexpected argument `--output-format` found"*.
//!
//! It is **build tooling only**. Nothing shipped needs nightly; the workspace
//! builds, tests and publishes on stable, and nightly is touched only when a
//! mirror is regenerated.
//!
//! The alternative was tried and rejected before this workspace's first commit:
//! scraping rustdoc's HTML with pandoc produced a *truncated* enum-variant list,
//! because rustdoc hides long variant lists behind a JavaScript "Show N
//! variants" widget meant for browsers. The JSON is the same structured AST
//! rustdoc renders from, so item lists come out complete and correctly typed.
//!
//! # Why this lives in `kovan-cli` and not `kovan-semantics`
//!
//! It spawns `cargo`, so it is desktop-scope and neither offline nor
//! deterministic. `kovan-semantics` must stay Android-clean and offline by
//! charter. This is the same split [`super::setup`] already uses for its
//! `cargo install` path.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Regenerate `crates/<crate_dir>/docs/api.md`.
///
/// `workspace_root` is the directory containing `crates/`. `crate_dir` is the
/// **directory name** under `crates/` (e.g. `outram-foam-basic-lib`), which may
/// differ from the `[package] name` in its manifest. `private` adds
/// `--document-private-items`, for auditing internals rather than the published
/// surface.
///
/// Returns the path written. Every failure names the missing prerequisite and
/// the command that fixes it, rather than reporting a bare non-zero exit: a
/// missing toolchain is a task, not a diagnosis (see the "API-doc toolchain"
/// hard rule in the workspace `CLAUDE.md`).
pub fn generate(workspace_root: &Path, crate_dir: &str, private: bool) -> io::Result<PathBuf> {
    let crate_path = workspace_root.join("crates").join(crate_dir);
    if !crate_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} does not exist", crate_path.display()),
        ));
    }

    let manifest = std::fs::read_to_string(crate_path.join("Cargo.toml"))?;
    let package = package_name(&manifest).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "could not find a [package] name in {}",
                crate_path.join("Cargo.toml").display()
            ),
        )
    })?;
    // rustdoc names its JSON after the crate, which uses underscores where the
    // package name may use hyphens.
    let snake = package.replace('-', "_");

    ensure_nightly()?;
    ensure_rustdoc_md()?;

    let mut doc = Command::new("cargo");
    doc.args([
        "+nightly",
        "doc",
        "-p",
        &package,
        "--no-deps",
        "-Z",
        "unstable-options",
        "--output-format",
        "json",
    ]);
    if private {
        doc.arg("--document-private-items");
    }
    let status = doc
        .current_dir(workspace_root)
        .status()
        .map_err(|error| io::Error::new(error.kind(), format!("could not run cargo: {error}")))?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "cargo +nightly doc failed for {package} (exit {status})"
        )));
    }

    let json = workspace_root
        .join("target")
        .join("doc")
        .join(format!("{snake}.json"));
    if !json.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "expected rustdoc JSON at {}, but it was not produced",
                json.display()
            ),
        ));
    }

    let out_dir = crate_path.join("docs");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("api.md");

    let status = Command::new("rustdoc-md")
        .arg("--path")
        .arg(&json)
        .arg("--output")
        .arg(&out_path)
        .status()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "could not run rustdoc-md: {error} -- install it with \
                     `cargo install rustdoc-md --locked`"
                ),
            )
        })?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "rustdoc-md failed for {package} (exit {status})"
        )));
    }

    // The intermediate JSON is a build artifact of this command alone, and it is
    // large; the Python did the same.
    std::fs::remove_file(&json)?;

    // Work around a rustdoc-md output bug: it doubles the tick on an elided
    // lifetime reference, emitting `&''` where `&'` is meant. Carried over from
    // the Python verbatim -- when rustdoc-md fixes it this becomes a no-op
    // rather than a corruption, since the pattern cannot occur in valid output.
    let text = std::fs::read_to_string(&out_path)?;
    let fixed = text.replace("&''", "&'");
    if fixed != text {
        std::fs::write(&out_path, fixed)?;
    }

    Ok(out_path)
}

/// Read `[package] name` from a manifest without a TOML parser.
///
/// Mirrors `kovan_semantics::agent_docs`'s reader: takes the first `name = ` key
/// inside the `[package]` section and stops at the next section header. Enough
/// for this workspace's manifests, and avoids a TOML dependency for one field.
fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = line.strip_prefix("name") {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Fail with an actionable message if no nightly toolchain is installed.
fn ensure_nightly() -> io::Result<()> {
    let output = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not run rustup: {error} -- rustup is required to select nightly"),
            )
        })?;
    if String::from_utf8_lossy(&output.stdout).contains("nightly") {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no nightly toolchain installed -- rustdoc's JSON output is nightly-only. \
         Run: rustup toolchain install nightly",
    ))
}

/// Fail with an actionable message if `rustdoc-md` is not on `PATH`.
///
/// Unlike the Python it replaces, this does **not** silently `cargo install` the
/// tool. Installing software is a decision for the person running the command,
/// and a surprise network fetch inside a documentation regeneration is exactly
/// the kind of implicit side effect this workspace's tooling avoids. The error
/// gives the exact command.
fn ensure_rustdoc_md() -> io::Result<()> {
    if which::which("rustdoc-md").is_ok() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "rustdoc-md not found on PATH. Run: cargo install rustdoc-md --locked",
    ))
}

/// Run `kovan api-docs <crate>`.
pub fn run(workspace_root: &Path, crate_dir: &str, private: bool) -> io::Result<()> {
    let path = generate(workspace_root, crate_dir, private)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_name_is_read_from_the_package_section_only() {
        assert_eq!(
            package_name("[package]\nname = \"kovan-cli\"\n").as_deref(),
            Some("kovan-cli")
        );
        // A `[[bin]]` name must not be mistaken for the package name -- getting
        // this wrong would look for the wrong rustdoc JSON file.
        assert_eq!(
            package_name("[package]\nversion = \"1\"\n\n[[bin]]\nname = \"kovan\"\n").as_deref(),
            None
        );
    }

    #[test]
    fn a_missing_crate_directory_is_reported_before_anything_is_spawned() {
        let tmp = tempfile::tempdir().unwrap();
        let error = generate(tmp.path(), "no-such-crate", false).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("no-such-crate"));
    }

    /// The rustdoc-md `&''` output bug is worked around, and the fix cannot
    /// corrupt correct output because `&''` is not valid Rust.
    #[test]
    fn the_elided_lifetime_workaround_is_idempotent() {
        assert_eq!("fn f(x: &'' str)".replace("&''", "&'"), "fn f(x: &' str)");
        // Already-correct text is untouched, so re-running is safe.
        let correct = "fn f(x: &'a str)";
        assert_eq!(correct.replace("&''", "&'"), correct);
    }
}
