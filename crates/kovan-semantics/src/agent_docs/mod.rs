//! Bundle this workspace's public-API documentation into a flat set of files
//! small enough to hand to an **external chat agent** with a fixed context
//! budget.
//!
//! # The problem this solves
//!
//! A coding agent running *inside* the repository can open any file it likes.
//! An agent reached through a web chat window cannot: it sees only what was
//! uploaded, and its context is finite. Two constraints follow, and both are
//! measured facts about this workspace rather than preferences:
//!
//! 1. **The upload dialog takes files, not folders.** So the output is *flat* —
//!    one file per crate, no subdirectories. [`write_bundle`] will not create
//!    one.
//! 2. **The corpus is far larger than the budget.** The thirteen
//!    `crates/<crate>/docs/api.md` mirrors totalled 5,154,447 bytes when this
//!    module was written — roughly 1.29 M estimated tokens against a typical
//!    200 k window, and the largest single crate exceeds that window on its
//!    own. Copying everything is not a design that can work.
//!
//! # The shape of the answer
//!
//! Every bundle carries two things unconditionally:
//!
//! - **`AGENTS.md`** — the workspace's coding rules, written for a remote agent
//!   (see [`agents_md`]). Hardcoded, not derived from the repository's own
//!   `CLAUDE.md`, which is mostly harness policy irrelevant to a chat agent.
//! - **`_INDEX.md`** — a condensed signature index covering **every** crate that
//!   has a mirror, so the agent has a map of the whole workspace even when it
//!   has been given the full text of only a few crates (see [`condense`]).
//!
//! and then the **verbatim** `api.md` of each crate the caller selected. The
//! index is what stops the agent inventing APIs for crates it was not given;
//! the verbatim files are what let it write correct code for the ones it was.
//!
//! # Determinism
//!
//! Re-running over unchanged inputs produces byte-identical output. Crates are
//! ordered by directory name, counts are accumulated in [`BTreeMap`]s, and
//! nothing here writes a timestamp, a hostname, or an absolute path into a
//! generated file. `agent_docs::tests::the_bundle_is_byte_identical_on_a_rerun`
//! is the gate on that, because a generator that quietly stops being
//! reproducible still looks like it works.
//!
//! [`BTreeMap`]: std::collections::BTreeMap

pub mod agents_md;
pub mod condense;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub use agents_md::agents_markdown;
pub use condense::{condensed_index_markdown, crate_index_markdown};

/// Bytes of source text assumed to correspond to one model token.
///
/// **This is a convention, not a measurement.** KOVAN has no tokenizer and must
/// not gain one — it is offline and deterministic by charter, and every real
/// tokenizer is a model-specific data file. Four bytes per token is the usual
/// rule of thumb for English prose and code.
///
/// It is **optimistic for this corpus.** Generated API markdown is dense in
/// punctuation, `snake_case` identifiers and fully-qualified paths, all of which
/// tokenize worse than prose, so a real count will typically come out *above*
/// this estimate. Treat any budget computed from it as soft, and prefer leaving
/// headroom over filling it exactly.
pub const BYTES_PER_ESTIMATED_TOKEN: u64 = 4;

/// Estimated model tokens for `bytes` of text, rounding up.
///
/// See [`BYTES_PER_ESTIMATED_TOKEN`] for what this is and is not. Every caller
/// that surfaces the result must label it an *estimate*; describing it as a
/// token count would be a claim this cannot support.
pub fn estimated_tokens(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_ESTIMATED_TOKEN)
}

/// One workspace member and the documentation files found for it.
///
/// Produced by [`inventory`]. The paths are **relative to the workspace root**,
/// never absolute, so that a bundle generated on one machine is byte-identical
/// to one generated on another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateEntry {
    /// Directory name under `crates/`, e.g. `outram-foam-basic-lib`. This is
    /// the identifier `kovan api-docs` takes, and the one used to name
    /// the crate's file in the bundle.
    pub directory: String,
    /// The `[package] name` from the crate's `Cargo.toml`, e.g.
    /// `outram_foam_basic_lib` may differ from the directory name.
    pub package: String,
    /// Path to `docs/api.md`, relative to the workspace root, if it exists.
    pub api_md: Option<PathBuf>,
    /// Size of `docs/api.md` in bytes, or `0` when absent.
    pub api_bytes: u64,
}

impl CrateEntry {
    /// Whether this crate has a rustdoc mirror to contribute.
    pub fn has_api_docs(&self) -> bool {
        self.api_md.is_some()
    }

    /// The flat filename this crate's **full** documentation takes in the
    /// bundle, e.g. `outram-foam-basic-lib.api.md`.
    ///
    /// Flat by construction — it contains no path separator — because the
    /// upload dialog this bundle exists for accepts files but not folders.
    pub fn bundle_filename(&self) -> String {
        format!("{}.api.md", self.directory)
    }

    /// The flat filename this crate's **condensed index** takes, e.g.
    /// `outram-foam-basic-lib.index.md`. Flat for the same reason.
    pub fn index_filename(&self) -> String {
        format!("{}.index.md", self.directory)
    }
}

/// Walk `crates/` and record every member with its documentation files.
///
/// Returns entries **sorted by directory name**, which is what makes every
/// downstream artifact reproducible. Directories without a `Cargo.toml` are
/// skipped silently: they are not crates.
///
/// `workspace_root` is the directory containing `crates/`. Errors from reading
/// individual `Cargo.toml` files are propagated rather than swallowed — a crate
/// that cannot be read is a fact the caller needs, not one to paper over.
pub fn inventory(workspace_root: &Path) -> io::Result<Vec<CrateEntry>> {
    let crates_dir = workspace_root.join("crates");
    let mut entries: Vec<CrateEntry> = Vec::new();

    // `read_dir` order is filesystem-dependent, so collect then sort. This is
    // the single most likely place for non-determinism to enter.
    for item in fs::read_dir(&crates_dir)? {
        let path = item?.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let Some(directory) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let package =
            package_name(&fs::read_to_string(&manifest)?).unwrap_or_else(|| directory.to_string());

        let api_path = path.join("docs").join("api.md");
        let (api_md, api_bytes) = if api_path.is_file() {
            let bytes = fs::metadata(&api_path)?.len();
            (
                Some(
                    PathBuf::from("crates")
                        .join(directory)
                        .join("docs")
                        .join("api.md"),
                ),
                bytes,
            )
        } else {
            (None, 0)
        };

        entries.push(CrateEntry {
            directory: directory.to_string(),
            package,
            api_md,
            api_bytes,
        });
    }

    entries.sort_by(|a, b| a.directory.cmp(&b.directory));
    Ok(entries)
}

/// Read `[package] name` out of a `Cargo.toml` without a TOML parser.
///
/// Deliberately minimal: it takes the first `name = "..."` that appears at the
/// start of a line after a `[package]` header, and stops at the next section.
/// That is enough for this workspace's manifests and avoids adding a TOML
/// dependency for one field. Returns `None` if no such key is found, and the
/// caller falls back to the directory name.
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
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// What [`write_bundle`] produced, for the caller to report to the user.
///
/// Carries the sizes so a CLI can print a per-file table and a running total
/// against the budget. Every token figure here is an **estimate** — see
/// [`estimated_tokens`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BundleReport {
    /// Bundle filename → size in bytes, ordered by filename.
    pub files: BTreeMap<String, u64>,
    /// Crates whose full `api.md` was copied, by directory name.
    pub included: Vec<String>,
    /// Crates that got a condensed `<crate>.index.md`, by directory name.
    pub indexed: Vec<String>,
    /// Crates that have **no** `docs/api.md` and so appear nowhere in the
    /// bundle, by directory name. Named in `AGENTS.md` so the agent is told
    /// what it has not been shown.
    pub missing_api_docs: Vec<String>,
}

impl BundleReport {
    /// Total bytes across every file written to disk.
    ///
    /// **Not the upload size.** The bundle is a menu: the optional per-crate
    /// index files are written so they are *available*, not because they are all
    /// meant to be uploaded together. Budget against [`core_bytes`](Self::core_bytes).
    pub fn total_bytes(&self) -> u64 {
        self.files.values().sum()
    }

    /// Bytes of the **core upload set** — `AGENTS.md`, `_INDEX.md`, and the full
    /// `api.md` of every selected crate.
    ///
    /// This is what the budget is checked against, because it is what a reader
    /// uploads every time. The optional `<crate>.index.md` files are then added
    /// one at a time out of whatever headroom is left; see
    /// [`optional_files`](Self::optional_files).
    pub fn core_bytes(&self) -> u64 {
        self.files
            .iter()
            .filter(|(name, _)| !name.ends_with(".index.md"))
            .map(|(_, bytes)| bytes)
            .sum()
    }

    /// Estimated tokens for the core upload set.
    pub fn core_estimated_tokens(&self) -> u64 {
        estimated_tokens(self.core_bytes())
    }

    /// The optional per-crate index files and their sizes, smallest first, so a
    /// caller can report how many fit in the remaining headroom.
    pub fn optional_files(&self) -> Vec<(String, u64)> {
        let mut files: Vec<(String, u64)> = self
            .files
            .iter()
            .filter(|(name, _)| name.ends_with(".index.md"))
            .map(|(name, bytes)| (name.clone(), *bytes))
            .collect();
        files.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        files
    }

    /// Total **estimated** tokens across every file written.
    pub fn total_estimated_tokens(&self) -> u64 {
        estimated_tokens(self.total_bytes())
    }

    /// Whether the **core upload set** exceeds `budget_tokens`.
    ///
    /// A `true` here is a warning, not a wall: the estimate is optimistic (see
    /// [`BYTES_PER_ESTIMATED_TOKEN`]), so a set that fits on paper may still not
    /// fit in practice.
    pub fn exceeds_budget(&self, budget_tokens: u64) -> bool {
        self.core_estimated_tokens() > budget_tokens
    }
}

/// Write the flat bundle into `out_dir`, replacing anything already there.
///
/// `selected` names the crate directories whose `api.md` is copied verbatim;
/// crates outside it still appear in `_INDEX.md`. A name in `selected` that
/// matches no crate, or matches one with no mirror, is simply not copied — the
/// caller is expected to have validated the selection and to report on it.
///
/// # Why the directory is cleared first
///
/// The bundle is *uploaded*, so a stale file left behind from a previous run is
/// not merely untidy — it is a crate the agent will be told about that the
/// maintainer thought they had dropped. Clearing makes the directory's contents
/// exactly the current selection, always.
///
/// Only the bundle's own file types are removed (`*.md`), so pointing this at a
/// directory holding something else cannot destroy it wholesale.
pub fn write_bundle(
    workspace_root: &Path,
    out_dir: &Path,
    entries: &[CrateEntry],
    selected: &[String],
) -> io::Result<BundleReport> {
    fs::create_dir_all(out_dir)?;
    for item in fs::read_dir(out_dir)? {
        let path = item?.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            fs::remove_file(path)?;
        }
    }

    let mut report = BundleReport::default();
    for entry in entries {
        if !entry.has_api_docs() {
            report.missing_api_docs.push(entry.directory.clone());
        }
    }

    // The roster first, so a partial failure still leaves the map behind.
    let roster = condensed_index_markdown(entries);
    write_and_record(out_dir, "_INDEX.md", &roster, &mut report)?;

    // The middle rung: one condensed index per documented crate. Always written
    // for every crate that has a mirror -- they are separate files, so the
    // reader chooses at upload time which to spend budget on.
    for entry in entries {
        // A crate whose FULL api.md is in the bundle does not also need a
        // condensed index of itself -- that would be paying twice for the same
        // crate out of one budget.
        if selected.iter().any(|s| s == &entry.directory) {
            continue;
        }
        if let Some(body) = crate_index_markdown(workspace_root, entry)? {
            write_and_record(out_dir, &entry.index_filename(), &body, &mut report)?;
            report.indexed.push(entry.directory.clone());
        }
    }

    for name in selected {
        let Some(entry) = entries.iter().find(|e| &e.directory == name) else {
            continue;
        };
        let Some(api_relative) = &entry.api_md else {
            continue;
        };
        let body = fs::read_to_string(workspace_root.join(api_relative))?;
        write_and_record(out_dir, &entry.bundle_filename(), &body, &mut report)?;
        report.included.push(entry.directory.clone());
    }

    // AGENTS.md last: it reports on the bundle, so it needs the finished tally.
    let agents = agents_markdown(&report);
    write_and_record(out_dir, "AGENTS.md", &agents, &mut report)?;

    Ok(report)
}

/// Write one bundle file and record its size in `report`.
fn write_and_record(
    out_dir: &Path,
    filename: &str,
    body: &str,
    report: &mut BundleReport,
) -> io::Result<()> {
    fs::write(out_dir.join(filename), body)?;
    report.files.insert(filename.to_string(), body.len() as u64);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic two-crate workspace, so the tests do not depend on the real
    /// `crates/` tree (which changes, and whose api.md files are megabytes).
    fn fixture(root: &Path) {
        let with_docs = root.join("crates").join("zed-crate").join("docs");
        fs::create_dir_all(&with_docs).unwrap();
        fs::write(
            root.join("crates").join("zed-crate").join("Cargo.toml"),
            "[package]\nname = \"zed_crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            with_docs.join("api.md"),
            "# Module `alpha`\n\nDoes the alpha thing.\n\n```rust\npub fn alpha() -> f64 { /* ... */ }\n```\n",
        )
        .unwrap();

        let no_docs = root.join("crates").join("alpha-crate");
        fs::create_dir_all(&no_docs).unwrap();
        fs::write(
            no_docs.join("Cargo.toml"),
            "[package]\nname = \"alpha_crate\"\n",
        )
        .unwrap();
    }

    #[test]
    fn inventory_is_sorted_and_finds_api_docs() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());

        let entries = inventory(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        // Sorted by directory name, not filesystem order -- this is what makes
        // every downstream artifact reproducible.
        assert_eq!(entries[0].directory, "alpha-crate");
        assert_eq!(entries[1].directory, "zed-crate");

        assert!(!entries[0].has_api_docs());
        assert!(entries[1].has_api_docs());
        assert_eq!(entries[1].package, "zed_crate");
        assert!(entries[1].api_bytes > 0);
    }

    #[test]
    fn a_bundle_filename_is_flat() {
        let entry = CrateEntry {
            directory: "outram-foam-basic-lib".to_string(),
            package: "outram_foam_basic_lib".to_string(),
            api_md: None,
            api_bytes: 0,
        };
        let name = entry.bundle_filename();
        assert_eq!(name, "outram-foam-basic-lib.api.md");
        assert!(
            !name.contains('/') && !name.contains('\\'),
            "the upload dialog takes files but not folders, so a bundle name \
             must never contain a path separator"
        );
    }

    #[test]
    fn the_token_figure_is_an_estimate_that_rounds_up() {
        assert_eq!(estimated_tokens(0), 0);
        assert_eq!(estimated_tokens(1), 1, "a partial token still costs one");
        assert_eq!(estimated_tokens(4), 1);
        assert_eq!(estimated_tokens(5), 2);
        assert_eq!(estimated_tokens(5_154_447), 1_288_612);
    }

    /// V&V: **the generator is reproducible.**
    ///
    /// # Why this exists
    ///
    /// "Deterministic" is a claim that decays silently. A timestamp, an
    /// absolute path, or a `HashMap` iteration order leaking into an output
    /// does not fail a build or a type check — the generator keeps producing
    /// plausible files, and only a byte comparison notices. KOVAN's whole
    /// charter is offline determinism, so this is the gate on it.
    ///
    /// # Methodology
    ///
    /// Build a synthetic workspace, run [`write_bundle`] twice into two
    /// separate directories, and compare every file byte-for-byte. Also assert
    /// the output is flat, since the folder-free layout is the reason the
    /// bundle exists at all.
    ///
    /// # Results (2026-08-14)
    ///
    /// Both runs produced the same three files (`AGENTS.md`, `_INDEX.md`,
    /// `zed-crate.api.md`) with identical bytes, and no subdirectory was
    /// created. Interpretation: nothing machine- or clock-dependent reaches the
    /// output.
    #[test]
    fn the_bundle_is_byte_identical_on_a_rerun() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let entries = inventory(tmp.path()).unwrap();
        let selected = vec!["zed-crate".to_string()];

        let first = tmp.path().join("out-1");
        let second = tmp.path().join("out-2");
        let report = write_bundle(tmp.path(), &first, &entries, &selected).unwrap();
        write_bundle(tmp.path(), &second, &entries, &selected).unwrap();

        assert_eq!(report.included, vec!["zed-crate".to_string()]);
        assert_eq!(report.missing_api_docs, vec!["alpha-crate".to_string()]);

        for filename in report.files.keys() {
            let a = fs::read(first.join(filename)).unwrap();
            let b = fs::read(second.join(filename)).unwrap();
            assert_eq!(a, b, "{filename} differed between two runs");
        }

        // Flat: files only, no directories.
        for item in fs::read_dir(&first).unwrap() {
            let path = item.unwrap().path();
            assert!(
                path.is_file(),
                "the bundle must be flat, found a directory: {}",
                path.display()
            );
        }
    }

    /// Dropping a crate from the selection must drop its file, not leave it to
    /// be uploaded by accident.
    #[test]
    fn a_rerun_does_not_leave_a_deselected_crate_behind() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let entries = inventory(tmp.path()).unwrap();
        let out = tmp.path().join("out");

        write_bundle(tmp.path(), &out, &entries, &vec!["zed-crate".to_string()]).unwrap();
        assert!(out.join("zed-crate.api.md").is_file());

        write_bundle(tmp.path(), &out, &entries, &[]).unwrap();
        assert!(
            !out.join("zed-crate.api.md").exists(),
            "a deselected crate's file must not survive a regeneration -- it \
             would be uploaded as though it were still wanted"
        );
    }

    #[test]
    fn package_name_is_read_from_the_package_section_only() {
        assert_eq!(
            package_name("[package]\nname = \"kovan_semantics\"\n").as_deref(),
            Some("kovan_semantics")
        );
        // A `name` under a later section must not be mistaken for the package.
        assert_eq!(
            package_name("[package]\nversion = \"1\"\n\n[[bin]]\nname = \"kovan\"\n").as_deref(),
            None
        );
        assert_eq!(package_name("").as_deref(), None);
    }
}
