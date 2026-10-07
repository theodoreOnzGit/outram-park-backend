//! **Full-suite test evidence** (GitHub #766; decided on #739, D4 and
//! "external dependency updates", 2026-10-07).
//!
//! `kovan-cli test` runs `cargo test --workspace --lib --tests --release`,
//! streams its output through [`output::OutputParser`] and writes one
//! [`TestEvidence`] file. This module is the pure half of that: no process,
//! no filesystem, no git, no clock, so it builds for wasm and CI and the
//! desktop compute the same answers from the same file.
//!
//! # Where the evidence lives
//!
//! ```text
//! <workspace>/kovan_test_evidence.toml       counted evidence: full suite,
//!                                            clean tree, every binary finished
//! <workspace>/target/kovan/test_evidence_last.toml
//!                                            anything else (a `-p` run, a
//!                                            dirty tree, a crashed binary):
//!                                            kept for reading, never counted
//! ```
//!
//! The evidence file is **not** `kovan.toml`. `kovan.toml` is a disposable
//! cache (maintainer, 2026-10-07) and its `[test_run]` table
//! ([`super::index::TestRun`]) is a **projection** of this file onto test
//! ids ([`map::to_test_run`]), so regenerating `kovan.toml` loses nothing:
//! the indexer re-projects from the evidence file. The evidence is the one
//! costly input (about 2.5 h for the whole workspace), which is why it sits
//! in its own file rather than inside the cache.
//!
//! # The file
//!
//! ```toml
//! schema_version = 1
//! kind = "test_evidence"
//! commit = "<HEAD sha>"
//! dirty = false                 # tracked/untracked .rs, Cargo.toml, Cargo.lock
//! cargo_lock = "sha256:…"       # Cargo.lock at the run
//! date = "2026-10-07T12:00:00Z"
//! rustc = "rustc 1.98.0 (88d9e12ae 2026-08-18)"
//! cargo = "cargo 1.98.0 (797e8a9bc 2026-08-05)"
//! command = ["cargo", "test", "--lib", …]
//! scope = "full"                # or "partial", with partial_reasons
//! exit_code = 0
//! complete = true               # every test binary printed its summary
//! [totals]
//! passed = 8123
//! failed = 0
//! ignored = 41
//!
//! [[binary]]
//! package = "kovan-common"
//! kind = "lib"                  # lib | bin | test
//! target = "kovan_common"
//! src = "crates/kovan-common/src/lib.rs"
//! expected = 173                # "running 173 tests"
//! complete = true
//! passed = ["review::index::tests::index_round_trips…", …]   # libtest names
//! failed = []
//! ignored = []
//! [binary.summary]              # libtest's own "test result:" line
//! passed = 173
//! …
//! ```
//!
//! Test names are stored **as libtest printed them**, per binary. Mapping
//! them to the call graph's test ids ([`crate::call_graph::reach`]) is a
//! separate, re-runnable step ([`map`]), because the observation should not
//! depend on the indexer's naming.
//!
//! # What counts
//!
//! [`TestEvidence::counted`]: the full suite (no package selection, filter or
//! feature change; [`args`]), a clean tree, and every binary finished with
//! its summary matching the lines parsed. A full run **with failures** counts:
//! a failing test at the current `Cargo.lock` is exactly the evidence that
//! makes the functions it reaches inherited-stale.
//!
//! # Not here
//!
//! Whether a function's hash is unchanged since `commit` (needs git) is the
//! staleness engine's job (#765); [`verdict::reach_verdict`] answers only
//! what the recorded run says about the tests that reach it.

pub mod args;
pub mod map;
pub mod output;
pub mod verdict;

use serde::{Deserialize, Serialize};

use super::types::{check_commit, check_hash, FieldError};

/// The counted evidence file, at the workspace root (next to `Cargo.lock`).
pub const EVIDENCE_FILE: &str = "kovan_test_evidence.toml";

/// Where a run that does not count is written, workspace-relative (under
/// the gitignored `target/`).
pub const UNCOUNTED_FILE: &str = "target/kovan/test_evidence_last.toml";

/// `kind = "test_evidence"`: a code-folder `kovan.toml` reader refuses it.
pub const EVIDENCE_KIND: &str = "test_evidence";

/// The `schema_version` written; additive changes never bump it.
pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// Was the run the whole suite?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// `cargo test --workspace --lib --tests --release`, with only
    /// result-neutral extras (`-j N`, `--test-threads N`, …; [`args`]).
    Full,
    /// Anything narrower or different; never counted.
    Partial,
}

/// The kind of a test binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryKind {
    /// A library's unit tests.
    Lib,
    /// A binary target's unit tests.
    Bin,
    /// An integration-test target (`tests/*.rs`).
    Test,
    /// Anything else cargo ran.
    Other,
}

/// libtest's `test result:` line for one binary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub ok: bool,
    pub passed: u32,
    pub failed: u32,
    pub ignored: u32,
    pub measured: u32,
    pub filtered_out: u32,
}

/// The results of one test binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryResults {
    /// The Cargo package name; empty when the build messages did not say.
    pub package: String,
    pub kind: BinaryKind,
    /// The Cargo target name.
    pub target: String,
    /// The target's root file, workspace-relative, `/`-separated.
    pub src: String,
    /// From `running N tests`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
    /// The summary was printed and matches the result lines parsed.
    pub complete: bool,
    #[serde(default)]
    pub passed: Vec<String>,
    #[serde(default)]
    pub failed: Vec<String>,
    #[serde(default)]
    pub ignored: Vec<String>,
}

impl BinaryResults {
    /// Whether the summary line agrees with the result lines parsed.
    pub fn summary_matches(&self) -> bool {
        self.summary.is_some_and(|s| {
            s.passed as usize == self.passed.len()
                && s.failed as usize == self.failed.len()
                && s.ignored as usize == self.ignored.len()
        })
    }
}

/// Counts over every binary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
}

/// One `kovan-cli test` run (module doc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestEvidence {
    pub schema_version: u32,
    pub kind: String,
    /// `git rev-parse HEAD` at the run.
    pub commit: String,
    /// Uncommitted changes to `.rs`, `Cargo.toml`, `Cargo.lock` or cargo
    /// config, before or after the run: the results then do not describe
    /// `commit`, and the run is never counted.
    pub dirty: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirty_paths: Vec<String>,
    /// `sha256:` of `Cargo.lock`'s bytes at the run.
    pub cargo_lock: String,
    /// UTC, ISO 8601.
    pub date: String,
    pub rustc: String,
    pub cargo: String,
    /// The exact command run, `cargo` first.
    pub command: Vec<String>,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partial_reasons: Vec<String>,
    /// cargo's exit code; `None` when it was killed by a signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// The build succeeded and every binary is complete.
    pub complete: bool,
    pub totals: Totals,
    #[serde(default, rename = "binary", skip_serializing_if = "Vec::is_empty")]
    pub binaries: Vec<BinaryResults>,
}

/// Why a run does not count as full-suite evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotCounted {
    /// Not the whole suite; the reasons.
    Partial(Vec<String>),
    /// Uncommitted changes; the paths.
    Dirty(Vec<String>),
    /// The build failed, or these binaries did not finish (crashed, killed)
    /// or printed a summary that disagrees with their result lines.
    Incomplete(Vec<String>),
}

impl std::fmt::Display for NotCounted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Partial(r) => write!(f, "partial run ({})", r.join(", ")),
            Self::Dirty(p) => write!(f, "uncommitted changes ({})", p.join(", ")),
            Self::Incomplete(b) => write!(f, "incomplete ({})", b.join(", ")),
        }
    }
}

/// Why an evidence file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    Toml(String),
    NotEvidence { kind: String },
    NewerSchema(u32),
    Field(FieldError),
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(e) => write!(f, "{EVIDENCE_FILE}: {e}"),
            Self::NotEvidence { kind } => {
                write!(f, "{EVIDENCE_FILE}: kind {kind:?}, not {EVIDENCE_KIND:?}")
            }
            Self::NewerSchema(v) => write!(
                f,
                "{EVIDENCE_FILE}: schema_version {v} is newer than {EVIDENCE_SCHEMA_VERSION}"
            ),
            Self::Field(e) => write!(f, "{EVIDENCE_FILE}: {e}"),
        }
    }
}

impl std::error::Error for EvidenceError {}

impl TestEvidence {
    /// Assemble a run's evidence. `complete` and `totals` are derived from
    /// `binaries` and `build_ok`; binaries and their name lists are sorted.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        commit: String,
        dirty_paths: Vec<String>,
        cargo_lock: String,
        date: String,
        rustc: String,
        cargo: String,
        command: Vec<String>,
        plan_reasons: Vec<String>,
        exit_code: Option<i32>,
        build_ok: bool,
        mut binaries: Vec<BinaryResults>,
    ) -> TestEvidence {
        let mut partial_reasons = plan_reasons;
        let mut totals = Totals::default();
        for b in &mut binaries {
            for v in [&mut b.passed, &mut b.failed, &mut b.ignored] {
                v.sort();
                v.dedup();
            }
            b.complete = b.summary_matches();
            totals.passed += b.passed.len();
            totals.failed += b.failed.len();
            totals.ignored += b.ignored.len();
            if let Some(s) = b.summary {
                if s.filtered_out > 0 {
                    partial_reasons.push(format!(
                        "{} tests filtered out of {}",
                        s.filtered_out, b.src
                    ));
                }
            }
        }
        binaries.sort_by(|a, b| (&a.src, &a.target, a.kind).cmp(&(&b.src, &b.target, b.kind)));
        let complete = build_ok && binaries.iter().all(|b| b.complete);
        let mut dirty_paths = dirty_paths;
        dirty_paths.sort();
        dirty_paths.dedup();
        TestEvidence {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            kind: EVIDENCE_KIND.to_string(),
            commit,
            dirty: !dirty_paths.is_empty(),
            dirty_paths,
            cargo_lock,
            date,
            rustc,
            cargo,
            command,
            scope: if partial_reasons.is_empty() {
                Scope::Full
            } else {
                Scope::Partial
            },
            partial_reasons,
            exit_code,
            complete,
            totals,
            binaries,
        }
    }

    /// `Ok` when the run is full-suite evidence (module doc, What counts);
    /// otherwise every reason it is not.
    pub fn counted(&self) -> Result<(), Vec<NotCounted>> {
        let mut why = Vec::new();
        if self.scope != Scope::Full || !self.partial_reasons.is_empty() {
            why.push(NotCounted::Partial(self.partial_reasons.clone()));
        }
        if self.dirty {
            why.push(NotCounted::Dirty(self.dirty_paths.clone()));
        }
        if !self.complete {
            let mut bad: Vec<String> = self
                .binaries
                .iter()
                .filter(|b| !b.complete)
                .map(|b| b.src.clone())
                .collect();
            if bad.is_empty() {
                bad.push("the build or cargo failed before every binary ran".into());
            }
            why.push(NotCounted::Incomplete(bad));
        }
        if why.is_empty() {
            Ok(())
        } else {
            Err(why)
        }
    }

    /// The workspace-relative path this run is written to: [`EVIDENCE_FILE`]
    /// when it counts, else [`UNCOUNTED_FILE`], so a partial or broken run
    /// never replaces counted evidence.
    pub fn destination(&self) -> &'static str {
        if self.counted().is_ok() {
            EVIDENCE_FILE
        } else {
            UNCOUNTED_FILE
        }
    }

    /// Read an evidence file.
    pub fn parse(text: &str) -> Result<TestEvidence, EvidenceError> {
        let v: toml::Value =
            toml::from_str(text).map_err(|e| EvidenceError::Toml(e.to_string()))?;
        let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind != EVIDENCE_KIND {
            return Err(EvidenceError::NotEvidence {
                kind: kind.to_string(),
            });
        }
        let e: TestEvidence =
            toml::from_str(text).map_err(|e| EvidenceError::Toml(e.to_string()))?;
        if e.schema_version > EVIDENCE_SCHEMA_VERSION {
            return Err(EvidenceError::NewerSchema(e.schema_version));
        }
        check_commit("commit", &e.commit).map_err(EvidenceError::Field)?;
        check_hash("cargo_lock", &e.cargo_lock).map_err(EvidenceError::Field)?;
        Ok(e)
    }

    /// The file's text (deterministic for the same data).
    pub fn to_toml(&self) -> Result<String, EvidenceError> {
        toml::to_string_pretty(self).map_err(|e| EvidenceError::Toml(e.to_string()))
    }
}

/// A file whose uncommitted change can change what `cargo test` builds:
/// Rust source, a manifest, the lock file, cargo config, the toolchain pin.
pub fn is_build_input(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.ends_with(".rs")
        || matches!(
            name,
            "Cargo.toml" | "Cargo.lock" | "rust-toolchain" | "rust-toolchain.toml"
        )
        || path.contains(".cargo/config")
}

/// The build inputs among `git status --porcelain --untracked-files=all`
/// lines (tracked changes and untracked files alike: an untracked
/// `tests/*.rs` is a new test target). Both sides of a rename are listed.
pub fn dirty_build_inputs(porcelain: &str) -> Vec<String> {
    let mut out: Vec<String> = porcelain
        .lines()
        .filter(|l| l.len() > 3)
        .flat_map(|l| {
            l[3..]
                .split(" -> ")
                .map(|p| p.trim_matches('"').to_string())
                .collect::<Vec<_>>()
        })
        .filter(|p| is_build_input(p))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests;
