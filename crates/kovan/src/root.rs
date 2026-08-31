//! The **Kovan root** — a Git-backed literature library on disk.
//!
//! A Kovan root is the directory a user points Kovan at. It is identified by a
//! [`ROOT_MARKER`] (`kovan_root.toml`) file at its top level, normally
//! alongside a `.git/` directory. Everything else in the library — papers,
//! topics, projects, the bibliography, the stored source PDFs — is addressed
//! relative to that directory.
//!
//! This module implements §2 and §5 of the Kovan redesign
//! ([GitHub issue #35](https://github.com/theodoreOnzGit/outram-park-backend/issues/35));
//! the keep/adapt/replace analysis behind it is in
//! `crates/kovan/docs/kovan-redesign-migration-map.md`.
//!
//! # What belongs here, and what does not
//!
//! This module owns **root identity and layout**: finding a root, reading and
//! validating its `kovan_root.toml`, and answering "where does X live in this
//! library?" as an absolute path. It deliberately does **not** create roots,
//! initialise Git, scan for papers, or read any file other than the marker —
//! those are separate steps of the redesign (§3, §4, §7 respectively), so that
//! merely *opening* a library stays cheap and cannot fail for reasons
//! unrelated to the library's identity.
//!
//! # Relationship to [`crate::project`]
//!
//! [`crate::project`] implements an **older, different** project format (a
//! `pdf/` + `markdown/` folder indexed by a generated `kovan.toml` with
//! line-range section pointers). The two are not versions of one another —
//! that format addresses content by regenerated line ranges, this one by
//! stable ids — and they coexist until a migration path exists. See the
//! migration map, §3.
//!
//! # `kovan_root.toml`
//!
//! Kept deliberately small: it *identifies and configures* the library, it is
//! not a database. Everything with a sensible convention has a default, so the
//! minimum viable file is just a schema version and a library name:
//!
//! ```toml
//! schema_version = 1
//!
//! [library]
//! id = "reactor-literature"
//! name = "Reactor Literature"
//! ```
//!
//! A fully-specified file overrides the conventional layout:
//!
//! ```toml
//! schema_version = 1
//!
//! [library]
//! id = "reactor-literature"
//! name = "Reactor Literature"
//!
//! [paths]
//! bibliography = "bibliography.bib"
//! papers = "papers"
//! topics = "topics"
//! projects = "projects"
//! open_sources = "literature/open"
//! restricted_sources = "literature/proprietary"
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Filename that marks a directory as a Kovan root (§2).
///
/// Distinct from `kovan.toml`, which marks an *entity* inside a library (a
/// paper, a topic, a project — §6/§7). One `kovan_root.toml` per library;
/// many `kovan.toml` beneath it.
pub const ROOT_MARKER: &str = "kovan_root.toml";

/// Directory holding Kovan's derived, disposable local state (§1, §3).
///
/// Everything under it is rebuildable from tracked files, so `rm -rf` on it is
/// always safe. It is never the source of truth for anything, and must be
/// gitignored by any root Kovan creates (§4).
pub const STATE_DIR: &str = ".kovan";

/// The `schema_version` this build reads and writes.
///
/// A root declaring a *newer* version is refused rather than guessed at — see
/// [`RootError::UnsupportedSchema`]. An older version would be migrated on
/// open, but version 1 is the first, so there is nothing to migrate from yet.
pub const SCHEMA_VERSION: u32 = 1;

/// Errors from locating, reading, or validating a Kovan root.
#[derive(Debug)]
pub enum RootError {
    /// No [`ROOT_MARKER`] was found at `start` or in any ancestor directory.
    NotAKovanRoot { start: PathBuf },
    /// An I/O failure, carrying the path it happened on.
    Io { path: PathBuf, source: std::io::Error },
    /// `kovan_root.toml` is not valid TOML, or does not match the schema.
    Toml { path: PathBuf, message: String },
    /// The root declares a `schema_version` this build does not understand.
    UnsupportedSchema { path: PathBuf, found: u32, supported: u32 },
}

impl std::fmt::Display for RootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAKovanRoot { start } => write!(
                f,
                "{}: not inside a Kovan library (no {ROOT_MARKER} here or in any parent directory)",
                start.display()
            ),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Toml { path, message } => write!(f, "{}: {message}", path.display()),
            Self::UnsupportedSchema { path, found, supported } => write!(
                f,
                "{}: schema_version {found} is newer than this build understands \
                 (supports {supported}) — upgrade Kovan to open this library",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RootError {}

/// Who the library is, for display and for stable reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryMeta {
    /// Stable machine identifier, e.g. `"reactor-literature"`. Conventionally
    /// lowercase kebab-case; not enforced, since it is never a path component.
    pub id: String,
    /// Human-readable name shown in the UI, e.g. `"Reactor Literature"`.
    pub name: String,
}

/// Where each part of the library lives, **relative to the root directory**.
///
/// Every field has a conventional default (§5: "prefer conventions/defaults
/// when values are discoverable"), so a `kovan_root.toml` need not mention
/// `[paths]` at all. Use the accessors on [`KovanRoot`] to get absolute paths;
/// these relative values are the on-disk representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RootPaths {
    /// The single authoritative BibTeX file. Under the citekey-as-id
    /// amendment (§7) this is the library's identity namespace: a paper's id
    /// *is* its BibTeX cite key.
    pub bibliography: PathBuf,
    /// Directory of paper entities, one subdirectory per citekey.
    pub papers: PathBuf,
    /// Root of the topic collection tree (§6). Arbitrarily nestable.
    pub topics: PathBuf,
    /// Root of the project collection tree (§6). Shares the topic tree's
    /// machinery; `kind` in each `kovan.toml` distinguishes the semantics.
    pub projects: PathBuf,
    /// Storage for open / redistributable source documents. Committable.
    pub open_sources: PathBuf,
    /// Storage for restricted / proprietary source documents. Gitignored, and
    /// must never reach a commit — see §4 and `DATA_POLICY.md`.
    pub restricted_sources: PathBuf,
}

impl Default for RootPaths {
    fn default() -> Self {
        Self {
            bibliography: PathBuf::from("bibliography.bib"),
            papers: PathBuf::from("papers"),
            topics: PathBuf::from("topics"),
            projects: PathBuf::from("projects"),
            open_sources: PathBuf::from("literature/open"),
            restricted_sources: PathBuf::from("literature/proprietary"),
        }
    }
}

/// The parsed contents of `kovan_root.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootConfig {
    /// On-disk format version — see [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Library identity (§5).
    pub library: LibraryMeta,
    /// Layout overrides; omitted entirely in a conventional library.
    #[serde(default)]
    pub paths: RootPaths,
}

impl RootConfig {
    /// A configuration for a new library with the conventional layout.
    ///
    /// `id` is the stable machine identifier and `name` the human-readable
    /// title; neither is validated, since neither becomes a path component.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            library: LibraryMeta { id: id.into(), name: name.into() },
            paths: RootPaths::default(),
        }
    }

    /// Render as the TOML text of a `kovan_root.toml`.
    ///
    /// Unlike the generated `kovan.toml` of [`crate::project`], this file is
    /// **hand-editable** — it is a user's own configuration, not a derived
    /// index — so it carries no "do not edit" header.
    ///
    /// # Errors
    ///
    /// Only if TOML serialisation fails, which cannot happen for this type's
    /// field types; the `Result` exists so callers need not `unwrap`.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

/// An opened Kovan library: its root directory plus its validated config.
///
/// Construct with [`KovanRoot::open`] (an exact directory) or
/// [`KovanRoot::discover`] (search upward from anywhere inside the library).
/// Both only read `kovan_root.toml`; nothing else on disk is touched, so
/// opening a library is cheap and cannot fail for reasons unrelated to its
/// identity.
///
/// Owned entirely by value — no lifetimes, no borrows of the caller's paths —
/// per the workspace Rust design rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KovanRoot {
    root: PathBuf,
    config: RootConfig,
}

impl KovanRoot {
    /// Whether `dir` is a Kovan root, i.e. directly contains a
    /// `kovan_root.toml`.
    ///
    /// A cheap existence check only — it does not parse the file, so a root
    /// with a malformed marker still answers `true`. Use it to decide whether
    /// to *attempt* [`KovanRoot::open`], not to conclude the library is valid.
    pub fn is_root(dir: &Path) -> bool {
        dir.join(ROOT_MARKER).is_file()
    }

    /// Open the library rooted exactly at `dir`.
    ///
    /// # Errors
    ///
    /// [`RootError::NotAKovanRoot`] if `dir` has no marker file,
    /// [`RootError::Io`] if it cannot be read, [`RootError::Toml`] if it is
    /// malformed, and [`RootError::UnsupportedSchema`] if it declares a newer
    /// `schema_version` than this build supports.
    pub fn open(dir: &Path) -> Result<Self, RootError> {
        let marker = dir.join(ROOT_MARKER);
        if !marker.is_file() {
            return Err(RootError::NotAKovanRoot { start: dir.to_path_buf() });
        }
        let text = std::fs::read_to_string(&marker)
            .map_err(|source| RootError::Io { path: marker.clone(), source })?;
        let config: RootConfig = toml::from_str(&text)
            .map_err(|e| RootError::Toml { path: marker.clone(), message: e.to_string() })?;
        if config.schema_version > SCHEMA_VERSION {
            return Err(RootError::UnsupportedSchema {
                path: marker,
                found: config.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(Self { root: dir.to_path_buf(), config })
    }

    /// Find the enclosing library by walking upward from `start`.
    ///
    /// `start` may be the root itself, or any path inside it (including a
    /// file). The **nearest** enclosing root wins, so a nested library shadows
    /// an outer one rather than being absorbed by it.
    ///
    /// # Errors
    ///
    /// [`RootError::NotAKovanRoot`] if no ancestor carries a marker; otherwise
    /// the same errors as [`KovanRoot::open`] for the root that was found.
    pub fn discover(start: &Path) -> Result<Self, RootError> {
        let mut cursor: Option<&Path> = if start.is_file() { start.parent() } else { Some(start) };
        while let Some(dir) = cursor {
            if Self::is_root(dir) {
                return Self::open(dir);
            }
            cursor = dir.parent();
        }
        Err(RootError::NotAKovanRoot { start: start.to_path_buf() })
    }

    /// The library's root directory, as given to [`open`](Self::open) or found
    /// by [`discover`](Self::discover).
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// The validated `kovan_root.toml` contents.
    pub fn config(&self) -> &RootConfig {
        &self.config
    }

    /// Absolute path of this root's `kovan_root.toml`.
    pub fn marker_path(&self) -> PathBuf {
        self.root.join(ROOT_MARKER)
    }

    /// Whether the library is under Git.
    ///
    /// §2 identifies a root by `kovan_root.toml` *and* `.git/`, but this is
    /// reported rather than required: a library whose `.git` is missing (never
    /// initialised, or deleted) must still open, so the UI can offer to
    /// initialise it instead of refusing to show the user their own files.
    ///
    /// Accepts both a normal repository (`.git/` directory) and a worktree or
    /// submodule checkout (`.git` file).
    pub fn has_git(&self) -> bool {
        self.root.join(".git").exists()
    }

    /// Absolute path of the library's BibTeX file (§7's identity namespace).
    ///
    /// Existence is **not** checked — a freshly created library has no
    /// bibliography until its first paper is ingested.
    pub fn bibliography_path(&self) -> PathBuf {
        self.root.join(&self.config.paths.bibliography)
    }

    /// Absolute path of the `papers/` directory (§7).
    pub fn papers_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.papers)
    }

    /// Absolute path of the `topics/` collection tree (§6).
    pub fn topics_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.topics)
    }

    /// Absolute path of the `projects/` collection tree (§6).
    pub fn projects_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.projects)
    }

    /// Absolute path of open / redistributable source storage.
    pub fn open_sources_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.open_sources)
    }

    /// Absolute path of restricted / proprietary source storage.
    ///
    /// Gitignored by construction in any root Kovan creates (§4). Nothing
    /// under it may ever be staged or committed.
    pub fn restricted_sources_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.restricted_sources)
    }

    /// Absolute path of the derived-state directory (`.kovan/`).
    ///
    /// Disposable: everything under it is rebuildable from tracked files, so
    /// deleting it is always safe (§1).
    pub fn state_dir(&self) -> PathBuf {
        self.root.join(STATE_DIR)
    }

    /// Absolute path of one paper's directory, `papers/<citekey>/`.
    ///
    /// `citekey` is the paper's id under the §7 amendment — its BibTeX cite
    /// key, as produced by `kovan_literature::parse_bib_entries`. The caller
    /// is responsible for having validated that it is filesystem-safe; this
    /// method only joins paths and does not check existence.
    pub fn paper_dir(&self, citekey: &str) -> PathBuf {
        self.papers_dir().join(citekey)
    }

    /// Absolute path of one paper's canonical research Markdown,
    /// `papers/<citekey>/<citekey>.md` (§12).
    ///
    /// The directory name, the filename, the wiki-link target and the citation
    /// key are all the same string — that is the point of the §7 amendment.
    pub fn paper_markdown(&self, citekey: &str) -> PathBuf {
        self.paper_dir(citekey).join(format!("{citekey}.md"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a minimal valid root marker into `dir` and return `dir`.
    fn seed_root(dir: &Path, body: &str) {
        std::fs::write(dir.join(ROOT_MARKER), body).unwrap();
    }

    const MINIMAL: &str = r#"
schema_version = 1

[library]
id = "reactor-literature"
name = "Reactor Literature"
"#;

    #[test]
    fn minimal_marker_parses_and_takes_conventional_paths() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);

        let root = KovanRoot::open(tmp.path()).unwrap();
        assert_eq!(root.config().library.id, "reactor-literature");
        assert_eq!(root.config().library.name, "Reactor Literature");
        // §5: a file that omits [paths] entirely still gets the conventions.
        assert_eq!(root.config().paths, RootPaths::default());
        assert_eq!(root.bibliography_path(), tmp.path().join("bibliography.bib"));
        assert_eq!(root.papers_dir(), tmp.path().join("papers"));
        assert_eq!(
            root.restricted_sources_dir(),
            tmp.path().join("literature").join("proprietary")
        );
        assert_eq!(root.state_dir(), tmp.path().join(".kovan"));
    }

    #[test]
    fn explicit_paths_override_the_conventions() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(
            tmp.path(),
            r#"
schema_version = 1

[library]
id = "lib"
name = "Lib"

[paths]
bibliography = "refs/main.bib"
papers = "lit/papers"
"#,
        );

        let root = KovanRoot::open(tmp.path()).unwrap();
        assert_eq!(root.bibliography_path(), tmp.path().join("refs/main.bib"));
        assert_eq!(root.papers_dir(), tmp.path().join("lit/papers"));
        // Unmentioned fields keep their conventional defaults.
        assert_eq!(root.topics_dir(), tmp.path().join("topics"));
    }

    #[test]
    fn open_without_a_marker_is_not_a_kovan_root() {
        let tmp = tempfile::tempdir().unwrap();
        let err = KovanRoot::open(tmp.path()).unwrap_err();
        assert!(matches!(err, RootError::NotAKovanRoot { .. }), "{err}");
    }

    #[test]
    fn malformed_marker_reports_toml_not_io() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), "schema_version = = 1");
        let err = KovanRoot::open(tmp.path()).unwrap_err();
        assert!(matches!(err, RootError::Toml { .. }), "{err}");
    }

    #[test]
    fn missing_library_table_is_a_schema_error() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), "schema_version = 1\n");
        let err = KovanRoot::open(tmp.path()).unwrap_err();
        assert!(matches!(err, RootError::Toml { .. }), "{err}");
    }

    #[test]
    fn newer_schema_version_is_refused_rather_than_guessed_at() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(
            tmp.path(),
            r#"
schema_version = 99

[library]
id = "lib"
name = "Lib"
"#,
        );
        let err = KovanRoot::open(tmp.path()).unwrap_err();
        match err {
            RootError::UnsupportedSchema { found, supported, .. } => {
                assert_eq!(found, 99);
                assert_eq!(supported, SCHEMA_VERSION);
            }
            other => panic!("expected UnsupportedSchema, got {other}"),
        }
    }

    #[test]
    fn discover_walks_up_from_a_nested_directory() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);
        let nested = tmp.path().join("papers").join("wang2018multiphysics");
        std::fs::create_dir_all(&nested).unwrap();

        let root = KovanRoot::discover(&nested).unwrap();
        assert_eq!(root.path(), tmp.path());
    }

    #[test]
    fn discover_accepts_a_file_path_and_starts_from_its_parent() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);
        let nested = tmp.path().join("papers").join("wang2018multiphysics");
        std::fs::create_dir_all(&nested).unwrap();
        let md = nested.join("wang2018multiphysics.md");
        std::fs::write(&md, "# notes\n").unwrap();

        let root = KovanRoot::discover(&md).unwrap();
        assert_eq!(root.path(), tmp.path());
    }

    #[test]
    fn discover_picks_the_nearest_root_so_a_nested_library_shadows_an_outer_one() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);
        let inner = tmp.path().join("sub").join("inner-library");
        std::fs::create_dir_all(&inner).unwrap();
        seed_root(
            &inner,
            r#"
schema_version = 1

[library]
id = "inner"
name = "Inner"
"#,
        );

        let root = KovanRoot::discover(&inner).unwrap();
        assert_eq!(root.path(), inner);
        assert_eq!(root.config().library.id, "inner");
    }

    #[test]
    fn discover_outside_any_library_reports_not_a_kovan_root() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        let err = KovanRoot::discover(&nested).unwrap_err();
        assert!(matches!(err, RootError::NotAKovanRoot { .. }), "{err}");
    }

    #[test]
    fn is_root_is_a_cheap_check_that_does_not_parse() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!KovanRoot::is_root(tmp.path()));
        seed_root(tmp.path(), "this is not valid toml = = =");
        // Present but malformed: `is_root` still says yes, `open` says no.
        assert!(KovanRoot::is_root(tmp.path()));
        assert!(KovanRoot::open(tmp.path()).is_err());
    }

    #[test]
    fn has_git_reports_rather_than_requires() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);
        let root = KovanRoot::open(tmp.path()).unwrap();
        // A library with no .git still opens — the UI offers to init it.
        assert!(!root.has_git());

        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        assert!(KovanRoot::open(tmp.path()).unwrap().has_git());
    }

    #[test]
    fn has_git_accepts_a_git_file_as_used_by_worktrees_and_submodules() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);
        std::fs::write(tmp.path().join(".git"), "gitdir: ../.git/modules/lit\n").unwrap();
        assert!(KovanRoot::open(tmp.path()).unwrap().has_git());
    }

    #[test]
    fn paper_paths_use_the_citekey_as_directory_and_filename() {
        let tmp = tempfile::tempdir().unwrap();
        seed_root(tmp.path(), MINIMAL);
        let root = KovanRoot::open(tmp.path()).unwrap();

        // §7 amendment: directory name == markdown filename == citekey.
        assert_eq!(
            root.paper_dir("wang2018multiphysics"),
            tmp.path().join("papers").join("wang2018multiphysics")
        );
        assert_eq!(
            root.paper_markdown("wang2018multiphysics"),
            tmp.path()
                .join("papers")
                .join("wang2018multiphysics")
                .join("wang2018multiphysics.md")
        );
    }

    #[test]
    fn config_round_trips_through_toml() {
        let config = RootConfig::new("reactor-literature", "Reactor Literature");
        let text = config.to_toml().unwrap();
        let back: RootConfig = toml::from_str(&text).unwrap();
        assert_eq!(back, config);
        assert_eq!(back.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn a_written_config_is_readable_as_a_root() {
        let tmp = tempfile::tempdir().unwrap();
        let config = RootConfig::new("lib", "Lib");
        std::fs::write(tmp.path().join(ROOT_MARKER), config.to_toml().unwrap()).unwrap();

        let root = KovanRoot::open(tmp.path()).unwrap();
        assert_eq!(root.config(), &config);
    }
}
