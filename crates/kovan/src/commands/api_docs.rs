//! `kovan-cli api-docs` — regenerate a crate's `docs/<crate>-api.md`, the committed
//! markdown mirror of its public API.
//!
//! A Rust port of `scripts/gen_api_docs.py`, which it **replaces** (retired
//! 2026-08-14). The pipeline is unchanged:
//!
//! ```text
//! cargo +nightly doc --output-format json  ->  target/doc/<crate>.json
//!                        rustdoc-md        ->  crates/<crate>/docs/<crate>-api.md
//! ```
//!
//! # Filename: `<crate>-api.md`, not `api.md`
//!
//! Named after its own crate directory (2026-08-17 onward) rather than the bare
//! `api.md` every crate used to write. Two problems that name had: a reader with
//! several of these files open in an editor or a bundle sees N identically-named
//! tabs, and nothing stopped a crate from acquiring a *second*, differently-named
//! mirror by accident — `njoy-outram-park-fork` had carried both `docs/api.md`
//! and `docs/njoy-api.md`, byte-identical, since 2026-08-14, doubling that
//! crate's published package for no reason anyone meant. `<crate>-api.md` is
//! self-describing out of context and only one name is ever right for a given
//! crate directory, so a second copy cannot arise without the mismatch being
//! visible in the filename itself.
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
//! # Why this lives in `kovan` (the CLI) and not `kovan-semantics`
//!
//! It spawns `cargo`, so it is desktop-scope and neither offline nor
//! deterministic. `kovan-semantics` must stay Android-clean and offline by
//! charter. This is the same split [`super::setup`] already uses for its
//! `cargo install` path.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Regenerate `crates/<crate_dir>/docs/<crate_dir>-api.md`.
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
    ensure_nightly()?;
    ensure_rustdoc_md()?;

    // Cargo supplies the library target's name: a package can override it with
    // [lib] name, as kovan does (gh:#779). It also supplies the target directory.
    let metadata =
        crate::code_map::run_cargo_metadata(workspace_root, false).map_err(io::Error::other)?;
    let json = rustdoc_json_path(&metadata, &package)?;

    let mut doc = Command::new("cargo");
    doc.args([
        "+nightly",
        "doc",
        "--release",
        "--lib",
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
    let out_path = out_dir.join(format!("{crate_dir}-api.md"));

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

/// The Cargo metadata fields needed to locate a library's rustdoc JSON.
#[derive(serde::Deserialize)]
struct DocMetadata {
    packages: Vec<DocPackage>,
    target_directory: PathBuf,
}

/// A package and the targets Cargo discovered for it.
#[derive(serde::Deserialize)]
struct DocPackage {
    name: String,
    targets: Vec<DocTarget>,
}

/// A target's actual crate name and kinds, independent of its package name.
#[derive(serde::Deserialize)]
struct DocTarget {
    name: String,
    kind: Vec<String>,
}

/// Resolve the library JSON from Cargo metadata, never from a package-name guess.
/// Library kinds match the existing call-graph reader in `call_graph::members`.
fn rustdoc_json_path(metadata: &str, package: &str) -> io::Result<PathBuf> {
    let metadata: DocMetadata = serde_json::from_str(metadata).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid cargo metadata: {e}"),
        )
    })?;
    let member = metadata
        .packages
        .iter()
        .find(|p| p.name == package)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("package {package} not found in cargo metadata"),
            )
        })?;
    let library = member
        .targets
        .iter()
        .find(|t| {
            t.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "lib" | "rlib" | "cdylib" | "staticlib" | "dylib" | "proc-macro"
                )
            })
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("package {package} has no library target to document"),
            )
        })?;
    Ok(metadata
        .target_directory
        .join("doc")
        .join(format!("{}.json", library.name)))
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

/// Which crates a run covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Refresh only crates that already have a `docs/<crate>-api.md`.
    ///
    /// The default for `--all`, and what "regenerate the suite" normally means:
    /// bring the committed mirrors back in step with the code, without
    /// deciding on anyone's behalf that 23 more crates should acquire one.
    Existing,
    /// Every crate under `crates/`, creating mirrors that do not yet exist.
    All,
}

/// Crate directory names under `crates/`, sorted, optionally filtered to those
/// that already carry a mirror.
fn crates_in_scope(workspace_root: &Path, scope: Scope) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(workspace_root.join("crates"))? {
        let path = entry?.path();
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if scope == Scope::Existing && !path.join("docs").join(format!("{name}-api.md")).is_file() {
            continue;
        }
        names.push(name.to_string());
    }
    names.sort();
    Ok(names)
}

/// Regenerate every mirror in `scope`.
///
/// # Why one crate's failure does not stop the rest
///
/// Each crate is a separate `cargo +nightly doc` invocation, and one that fails
/// to compile says nothing about the other thirty-six. Aborting on the first
/// error would leave the suite half-regenerated with no summary of what
/// happened. Failures are collected, reported by name at the end, and turned
/// into a non-zero exit so a script still notices.
fn run_all(workspace_root: &Path, scope: Scope, private: bool) -> io::Result<()> {
    let names = crates_in_scope(workspace_root, scope)?;
    if names.is_empty() {
        println!("no crates in scope");
        return Ok(());
    }

    println!(
        "regenerating {} crate{} ({})",
        names.len(),
        if names.len() == 1 { "" } else { "s" },
        match scope {
            Scope::Existing => "refreshing existing mirrors",
            Scope::All => "every crate, creating missing mirrors",
        }
    );

    let mut failed: Vec<(String, String)> = Vec::new();
    for (index, name) in names.iter().enumerate() {
        println!("[{}/{}] {name}", index + 1, names.len());
        match generate(workspace_root, name, private) {
            Ok(path) => println!("        wrote {}", path.display()),
            Err(error) => {
                eprintln!("        FAILED: {error}");
                failed.push((name.clone(), error.to_string()));
            }
        }
    }

    println!();
    println!(
        "{} of {} regenerated",
        names.len() - failed.len(),
        names.len()
    );
    if failed.is_empty() {
        return Ok(());
    }
    println!("{} failed:", failed.len());
    for (name, error) in &failed {
        println!("  {name}: {error}");
    }
    Err(io::Error::other(format!(
        "{} of {} crates failed to regenerate",
        failed.len(),
        names.len()
    )))
}

/// Run `kovan-cli api-docs`.
///
/// `crate_dir` names a single crate; `all` regenerates the whole suite instead,
/// and `include_missing` widens that to crates with no mirror yet. Exactly one
/// of `crate_dir` and `all` is expected — the CLI enforces that.
pub fn run(
    workspace_root: &Path,
    crate_dir: Option<&str>,
    all: bool,
    include_missing: bool,
    private: bool,
) -> io::Result<()> {
    if all {
        let scope = if include_missing {
            Scope::All
        } else {
            Scope::Existing
        };
        return run_all(workspace_root, scope, private);
    }
    let Some(crate_dir) = crate_dir else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "name a crate, or pass --all to regenerate the whole suite",
        ));
    };
    let path = generate(workspace_root, crate_dir, private)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dependency-free workspace whose directory, package and library names
    /// can differ, as they do for kovan (gh:#779).
    fn doc_workspace(lib_manifest: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/example-directory\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        let crate_path = tmp.path().join("crates/example-directory");
        std::fs::create_dir_all(crate_path.join("src")).unwrap();
        std::fs::write(
            crate_path.join("Cargo.toml"),
            format!(
                "[package]\nname = \"example-package\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{lib_manifest}"
            ),
        )
        .unwrap();
        std::fs::write(
            crate_path.join("src/lib.rs"),
            "/// The fixture's public answer.\npub fn answer() -> u8 { 42 }\n",
        )
        .unwrap();
        tmp
    }

    /// Run the real rustdoc -> rustdoc-md pipeline on a renamed library. The
    /// old package-derived JSON path fails after rustdoc creates example_core.json.
    #[test]
    #[ignore = "requires nightly Rust and rustdoc-md; run explicitly with --ignored"]
    fn generates_docs_for_a_library_named_differently_from_its_package() {
        let tmp = doc_workspace("\n[lib]\nname = \"example_core\"\n");
        let path = generate(tmp.path(), "example-directory", false).unwrap();
        assert_eq!(
            path,
            tmp.path()
                .join("crates/example-directory/docs/example-directory-api.md")
        );
        let markdown = std::fs::read_to_string(path).unwrap();
        assert!(markdown.contains("answer"), "{markdown}");
        assert!(
            markdown.contains("The fixture's public answer."),
            "{markdown}"
        );
    }

    /// The same pipeline must still work when Cargo infers the library name
    /// from a hyphenated package, with no explicit [lib] section.
    #[test]
    #[ignore = "requires nightly Rust and rustdoc-md; run explicitly with --ignored"]
    fn generates_docs_for_a_default_library_name() {
        let tmp = doc_workspace("");
        let path = generate(tmp.path(), "example-directory", false).unwrap();
        let markdown = std::fs::read_to_string(path).unwrap();
        assert!(markdown.contains("answer"), "{markdown}");
        assert!(
            markdown.contains("The fixture's public answer."),
            "{markdown}"
        );
    }

    #[test]
    fn real_cargo_metadata_resolves_custom_and_default_library_names() {
        for (manifest, filename) in [
            ("\n[lib]\nname = \"example_core\"\n", "example_core.json"),
            ("", "example_package.json"),
        ] {
            let tmp = doc_workspace(manifest);
            let metadata = crate::code_map::run_cargo_metadata(tmp.path(), true).unwrap();
            let path = rustdoc_json_path(&metadata, "example-package").unwrap();
            assert_eq!(path.file_name().unwrap(), filename);
            assert_eq!(path.parent().unwrap().file_name().unwrap(), "doc");
        }
    }

    #[test]
    fn json_path_uses_the_library_target_and_cargos_target_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("custom-build-output");
        let metadata = serde_json::json!({
            "target_directory": target,
            "packages": [{
                "name": "another-package",
                "targets": [{"name": "another_core", "kind": ["lib"]}]
            }, {
                "name": "example-package",
                "targets": [
                    {"name": "example-package", "kind": ["bin"]},
                    {"name": "example_core", "kind": ["lib"]}
                ]
            }]
        });
        assert_eq!(
            rustdoc_json_path(&metadata.to_string(), "example-package").unwrap(),
            target.join("doc/example_core.json")
        );
    }

    #[test]
    fn json_path_accepts_the_default_name_and_every_library_kind() {
        for kind in ["lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"] {
            let metadata = serde_json::json!({
                "target_directory": "target",
                "packages": [{
                    "name": "example-package",
                    "targets": [{"name": "example_package", "kind": [kind]}]
                }]
            });
            assert_eq!(
                rustdoc_json_path(&metadata.to_string(), "example-package").unwrap(),
                Path::new("target").join("doc/example_package.json"),
                "{kind}"
            );
        }
    }

    #[test]
    fn json_path_reports_missing_packages_and_binary_only_packages() {
        let metadata = serde_json::json!({
            "target_directory": "target",
            "packages": [{
                "name": "example-package",
                "targets": [{"name": "example-package", "kind": ["bin"]}]
            }]
        });
        let json = metadata.to_string();
        let missing = rustdoc_json_path(&json, "missing-package").unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        assert!(missing.to_string().contains("missing-package"));
        let binary = rustdoc_json_path(&json, "example-package").unwrap_err();
        assert_eq!(binary.kind(), io::ErrorKind::InvalidInput);
        assert!(binary.to_string().contains("no library target"));
    }

    #[test]
    fn json_path_reports_malformed_metadata() {
        let error = rustdoc_json_path("not JSON", "example-package").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

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

    /// `crates_in_scope(Existing)` must look for `<name>-api.md`, not the old
    /// bare `api.md` -- a crate carrying only the pre-2026-08-17 filename should
    /// no longer register as "already documented" (it needs a fresh
    /// regeneration to pick up the new name once).
    #[test]
    fn scope_existing_looks_for_the_crate_named_mirror() {
        let tmp = tempfile::tempdir().unwrap();
        let crates_dir = tmp.path().join("crates");

        // `zed-crate` has the new-convention mirror.
        let zed_docs = crates_dir.join("zed-crate").join("docs");
        std::fs::create_dir_all(&zed_docs).unwrap();
        std::fs::write(
            crates_dir.join("zed-crate").join("Cargo.toml"),
            "[package]\nname = \"zed-crate\"\n",
        )
        .unwrap();
        std::fs::write(zed_docs.join("zed-crate-api.md"), "# stub\n").unwrap();

        // `alpha-crate` has only the old bare filename -- must not count.
        let alpha_docs = crates_dir.join("alpha-crate").join("docs");
        std::fs::create_dir_all(&alpha_docs).unwrap();
        std::fs::write(
            crates_dir.join("alpha-crate").join("Cargo.toml"),
            "[package]\nname = \"alpha-crate\"\n",
        )
        .unwrap();
        std::fs::write(alpha_docs.join("api.md"), "# stub\n").unwrap();

        let existing = crates_in_scope(tmp.path(), Scope::Existing).unwrap();
        assert_eq!(existing, vec!["zed-crate".to_string()]);

        let all = crates_in_scope(tmp.path(), Scope::All).unwrap();
        assert_eq!(
            all,
            vec!["alpha-crate".to_string(), "zed-crate".to_string()]
        );
    }
}
