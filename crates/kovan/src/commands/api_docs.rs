//! `kovan-cli api-docs` — regenerate a crate's `docs/<crate>-api.md`, the committed
//! markdown mirror of its public API.
//!
//! A Rust port of `scripts/gen_api_docs.py`, which it **replaces** (retired
//! 2026-08-14). The pipeline is:
//!
//! ```text
//! cargo +nightly doc --release --lib …  ->  Cargo-reported library JSON path
//!                        rustdoc-md    ->  crates/<crate>/docs/<crate>-api.md
//! ```
//!
//! Cargo's artifact messages supply the JSON path, including the library name,
//! configured target directory and any build-target subdirectory (gh:#779).
//! `KOVAN_RUSTDOC_TOOLCHAIN` selects the nightly toolchain (default: `nightly`).
//! CI pins it alongside rustdoc-md to keep the JSON producer and reader paired.
//!
//! # Credit
//!
//! The Cargo-artifact JSON resolution, the `KOVAN_RUSTDOC_TOOLCHAIN` pin and
//! the end-to-end pipeline tests (gh:#779) were contributed by Curtis
//! (KnightNiwrem on GitHub) in PR #1303, 2026-11-05.
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
use std::process::{Command, Stdio};

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

    let manifest = dunce::canonicalize(crate_path.join("Cargo.toml"))?;
    let toolchain = std::env::var("KOVAN_RUSTDOC_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
    ensure_nightly(&toolchain)?;
    ensure_rustdoc_md()?;

    let mut doc = Command::new("cargo");
    doc.arg(format!("+{toolchain}"))
        .args([
            "doc",
            "--release",
            "--lib",
            "--message-format=json-render-diagnostics",
            "--no-deps",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .arg("--manifest-path")
        .arg(&manifest);
    if private {
        doc.arg("--document-private-items");
    }
    let output = doc
        .current_dir(workspace_root)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| io::Error::new(error.kind(), format!("could not run cargo: {error}")))?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo +{toolchain} doc failed for {crate_dir} (exit {})",
            output.status
        )));
    }

    let json = rustdoc_json_path(&output.stdout, &manifest)?;
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
            "rustdoc-md failed for {crate_dir} (exit {status})"
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

/// Cargo's structured output identifies artifacts without guessing their paths.
#[derive(serde::Deserialize)]
#[serde(tag = "reason")]
enum DocMessage {
    #[serde(rename = "compiler-artifact")]
    Artifact {
        manifest_path: PathBuf,
        target: DocTarget,
        filenames: Vec<PathBuf>,
    },
    #[serde(other)]
    Other,
}

/// A target's kinds distinguish libraries from binaries and build scripts.
#[derive(serde::Deserialize)]
struct DocTarget {
    kind: Vec<String>,
}

/// Select the library JSON Cargo actually produced for the requested manifest.
/// Library kinds match the existing call-graph reader in `call_graph::members`.
fn rustdoc_json_path(output: &[u8], manifest: &Path) -> io::Result<PathBuf> {
    let mut paths = Vec::new();
    for line in output.split(|byte| *byte == b'\n') {
        // Other tools (for example procedural macros) may print ordinary text.
        if !line.starts_with(b"{") {
            continue;
        }
        let message: DocMessage = serde_json::from_slice(line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid cargo artifact message: {error}"),
            )
        })?;
        let DocMessage::Artifact {
            manifest_path,
            target,
            filenames,
        } = message
        else {
            continue;
        };
        if manifest_path == manifest
            && target.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "lib" | "rlib" | "cdylib" | "staticlib" | "dylib" | "proc-macro"
                )
            })
        {
            paths.extend(filenames.into_iter().filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            }));
        }
    }
    paths.sort();
    paths.dedup();
    match paths.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "cargo did not report library JSON for {}",
                manifest.display()
            ),
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cargo produced library JSON for multiple build targets; configure a single build.target for one API mirror",
        )),
    }
}

/// Fail with an actionable message if the selected nightly is unavailable.
fn ensure_nightly(toolchain: &str) -> io::Result<()> {
    let output = Command::new("rustup")
        .args(["run", toolchain, "rustdoc", "--version"])
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not run rustup: {error} -- rustup is required to select nightly"),
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "documentation toolchain {toolchain} is unavailable -- rustdoc's JSON output is nightly-only. \
             Run: rustup toolchain install {toolchain}"
        ),
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
/// Each crate is a separate nightly `cargo doc` invocation, and one that fails
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

    /// Serialises the tests that run the real `cargo doc` pipeline. With
    /// `CARGO_TARGET_DIR` set in the environment every fixture shares one
    /// target directory, so parallel runs overwrite each other's
    /// `doc/example_package.json` and rustdoc-md reads a half-written file
    /// (seen 2026-11-07 when reviewing PR #1303).
    static DOC_PIPELINE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_doc_pipeline() -> std::sync::MutexGuard<'static, ()> {
        DOC_PIPELINE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

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
            "/// The fixture's public answer.\npub fn answer() -> u8 { 42 }\n\
             #[cfg(debug_assertions)]\npub fn debug_only() {}\n\
             #[cfg(not(debug_assertions))]\npub fn release_only() {}\n",
        )
        .unwrap();
        tmp
    }

    /// Run the real rustdoc -> rustdoc-md pipeline on a renamed library. The
    /// old package-derived JSON path fails after rustdoc creates example_core.json.
    #[test]
    fn generates_docs_for_a_library_named_differently_from_its_package() {
        let _pipeline = lock_doc_pipeline();
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
        // Measured 2026-11-05: --release leaves rustdoc's default cfgs intact;
        // it does not remove cfg(debug_assertions) items from this mirror.
        assert!(markdown.contains("debug_only"), "{markdown}");
        assert!(!markdown.contains("release_only"), "{markdown}");
    }

    /// The same pipeline must still work when Cargo infers the library name
    /// from a hyphenated package, with no explicit [lib] section.
    #[test]
    fn generates_docs_for_a_default_library_name() {
        let _pipeline = lock_doc_pipeline();
        let tmp = doc_workspace("");
        // Exercise --all as well: Cargo selects the package from its manifest,
        // with no per-crate metadata query or hand-written TOML parser.
        run(tmp.path(), None, true, true, false).unwrap();
        let path = tmp
            .path()
            .join("crates/example-directory/docs/example-directory-api.md");
        let markdown = std::fs::read_to_string(path).unwrap();
        assert!(markdown.contains("answer"), "{markdown}");
        assert!(
            markdown.contains("The fixture's public answer."),
            "{markdown}"
        );
    }

    #[test]
    fn generates_docs_for_quoted_and_commented_package_names() {
        let _pipeline = lock_doc_pipeline();
        for name in [
            "name = 'example-package'",
            "name = \"example-package\" # a trailing comment",
        ] {
            let tmp = doc_workspace("");
            let manifest = tmp.path().join("crates/example-directory/Cargo.toml");
            let text = std::fs::read_to_string(&manifest).unwrap();
            std::fs::write(&manifest, text.replace("name = \"example-package\"", name)).unwrap();
            let path = generate(tmp.path(), "example-directory", false).unwrap();
            let markdown = std::fs::read_to_string(path).unwrap();
            assert!(markdown.contains("answer"), "{markdown}");
        }
    }

    #[test]
    fn generates_docs_with_a_configured_build_target_and_output_directory() {
        let _pipeline = lock_doc_pipeline();
        let tmp = doc_workspace("\n[lib]\nname = 'example_core'\n");
        let version = Command::new("rustc").arg("-vV").output().unwrap();
        assert!(version.status.success());
        let version = String::from_utf8(version.stdout).unwrap();
        let host = version
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .unwrap();
        std::fs::create_dir(tmp.path().join(".cargo")).unwrap();
        std::fs::write(
            tmp.path().join(".cargo/config.toml"),
            format!("[build]\ntarget = '{host}'\ntarget-dir = 'custom-build-output'\n"),
        )
        .unwrap();
        // A stale host artifact must not be selected for this target build.
        let stale_doc = tmp.path().join("custom-build-output/doc");
        std::fs::create_dir_all(&stale_doc).unwrap();
        std::fs::write(stale_doc.join("example_core.json"), "not rustdoc JSON").unwrap();
        let path = generate(tmp.path(), "example-directory", false).unwrap();
        let markdown = std::fs::read_to_string(path).unwrap();
        assert!(markdown.contains("answer"), "{markdown}");
        assert!(stale_doc.join("example_core.json").is_file());
    }

    #[test]
    fn json_path_uses_the_reported_artifact_for_the_requested_manifest() {
        let manifest = Path::new("workspace/crates/example/Cargo.toml");
        let json = Path::new("custom-build-output/aarch64-linux-android/doc/example_core.json");
        let messages = [
            artifact_message(
                Path::new("other/Cargo.toml"),
                "lib",
                Path::new("other.json"),
            ),
            artifact_message(manifest, "bin", Path::new("binary.json")),
            artifact_message(manifest, "lib", Path::new("libexample_core.rmeta")),
            artifact_message(manifest, "lib", json),
            "ordinary output from another tool".into(),
            serde_json::json!({"reason": "build-finished", "success": true}).to_string(),
        ]
        .join("\n");
        assert_eq!(
            rustdoc_json_path(messages.as_bytes(), manifest).unwrap(),
            json
        );
    }

    /// A Cargo artifact message, containing only the fields this reader needs.
    fn artifact_message(manifest: &Path, kind: &str, json: &Path) -> String {
        serde_json::json!({
            "reason": "compiler-artifact",
            "manifest_path": manifest,
            "target": {"kind": [kind]},
            "filenames": [json]
        })
        .to_string()
    }

    #[test]
    fn json_path_accepts_every_library_kind() {
        let manifest = Path::new("Cargo.toml");
        let json = Path::new("target/doc/example_package.json");
        for kind in ["lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"] {
            let message = artifact_message(manifest, kind, json);
            assert_eq!(
                rustdoc_json_path(message.as_bytes(), manifest).unwrap(),
                json,
                "{kind}"
            );
        }
    }

    #[test]
    fn json_path_reports_missing_library_artifacts() {
        let manifest = Path::new("Cargo.toml");
        let message = artifact_message(manifest, "bin", Path::new("binary.json"));
        let error = rustdoc_json_path(message.as_bytes(), manifest).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("Cargo.toml"));
    }

    #[test]
    fn json_path_reports_malformed_artifact_messages() {
        let error = rustdoc_json_path(b"{not JSON}", Path::new("Cargo.toml")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn json_path_rejects_multiple_build_targets() {
        let manifest = Path::new("Cargo.toml");
        let messages = [
            artifact_message(manifest, "lib", Path::new("target/host/doc/example.json")),
            artifact_message(manifest, "lib", Path::new("target/other/doc/example.json")),
        ]
        .join("\n");
        let error = rustdoc_json_path(messages.as_bytes(), manifest).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("single build.target"));
    }

    #[test]
    fn repeated_artifact_messages_identify_the_same_file() {
        let manifest = Path::new("Cargo.toml");
        let json = Path::new("target/doc/example.json");
        let message = artifact_message(manifest, "lib", json);
        let messages = format!("{message}\n{message}\n");
        assert_eq!(
            rustdoc_json_path(messages.as_bytes(), manifest).unwrap(),
            json
        );
    }

    #[test]
    fn a_missing_selected_toolchain_reports_its_install_command() {
        let toolchain = "nightly-2000-01-01";
        let error = ensure_nightly(toolchain).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            error
                .to_string()
                .contains("rustup toolchain install nightly-2000-01-01")
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
