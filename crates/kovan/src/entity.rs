//! **Entities** — the papers, topics and projects a Kovan library is made of.
//!
//! Every entity is a directory containing a `kovan.toml`. Which kind it is is
//! declared by that file's `kind` field, and dispatched by [`EntityKind`] — an
//! enum, exhaustively matched, never a trait object, per the workspace Rust
//! design rules.
//!
//! Implements §6 (collections) and §7 (papers) of the Kovan redesign
//! ([GitHub issue #35](https://github.com/theodoreOnzGit/outram-park-backend/issues/35)).
//!
//! # `kovan.toml` is not `kovan_root.toml`
//!
//! [`crate::root`]'s `kovan_root.toml` marks the *library*; there is exactly
//! one. This module's `kovan.toml` marks an *entity inside* the library; there
//! are many. Keeping the two filenames distinct is what lets root discovery
//! walk upward without ever mistaking a paper for a library.
//!
//! # Topics and projects share one implementation
//!
//! §6: "Topics and Projects should share collection/tree machinery where
//! practical; `kind` distinguishes semantics." They do — one
//! [`EntityConfig`], one loader, one writer. There is no separate topic type
//! and project type to drift apart.
//!
//! # A paper's id is its BibTeX cite key
//!
//! Under the amendment agreed in issue #35, a paper has no identity field
//! separate from its citation key. The paper directory name, its Markdown
//! filename, its `[[wiki-link]]` target and its `[@citation]` key are all
//! literally the same string. [`CiteKey`] is that string, validated as safe to
//! use as a directory name — see its docs for why validation is not optional.
//!
//! ```
//! use kovan::entity::{Access, Classification, EntityConfig, EntityKind, CiteKey};
//!
//! let key = CiteKey::parse("wang2018multiphysics").unwrap();
//! let paper = EntityConfig::paper(key.clone(), Access::Restricted)
//!     .with_topics(["htgrs/thermal-hydraulics", "htgrs/neutronics"])
//!     .with_projects(["outram-park"]);
//!
//! assert_eq!(paper.id, "wang2018multiphysics");
//! assert_eq!(paper.kind, EntityKind::Paper);
//! assert!(paper.validate().is_ok());
//! # let _ = Classification::default();
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::root::KovanRoot;

/// Filename marking a directory as a Kovan entity (§6, §7).
pub const ENTITY_MARKER: &str = "kovan.toml";

/// The classification every paper lands in when the user has not chosen one.
///
/// §7 requires a paper to belong to at least one topic or project, and also
/// requires "an `Unsorted` inbox/classification for rapid ingestion" — this is
/// that inbox. Ingestion should file into it rather than leaving a paper
/// unclassified, so nothing is ever lost by being unfiled.
pub const UNSORTED: &str = "unsorted";

/// The `schema_version` this build reads and writes for entities.
pub const SCHEMA_VERSION: u32 = 1;

/// Errors from reading, writing or validating an entity.
#[derive(Debug)]
pub enum EntityError {
    /// The directory has no `kovan.toml`.
    NotAnEntity { path: PathBuf },
    /// An I/O failure, carrying the path it happened on.
    Io { path: PathBuf, source: std::io::Error },
    /// `kovan.toml` is not valid TOML, or does not match the schema.
    Toml { path: PathBuf, message: String },
    /// The entity declares a `schema_version` this build does not understand.
    UnsupportedSchema { path: PathBuf, found: u32, supported: u32 },
    /// A cite key cannot be used as a directory name — see [`CiteKey`].
    UnsafeCiteKey { raw: String, reason: String },
    /// A paper declares no topic and no project. §7 requires at least one;
    /// file it under [`UNSORTED`] instead of leaving it unclassified.
    Unclassified { id: String },
    /// `kind` and the payload disagree — e.g. a topic carrying `[source]`,
    /// which only a paper may have.
    KindMismatch { id: String, message: String },
}

impl std::fmt::Display for EntityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnEntity { path } => {
                write!(f, "{}: not a Kovan entity (no {ENTITY_MARKER})", path.display())
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Toml { path, message } => write!(f, "{}: {message}", path.display()),
            Self::UnsupportedSchema { path, found, supported } => write!(
                f,
                "{}: schema_version {found} is newer than this build understands (supports {supported})",
                path.display()
            ),
            Self::UnsafeCiteKey { raw, reason } => {
                write!(f, "cite key {raw:?} cannot be a directory name: {reason}")
            }
            Self::Unclassified { id } => write!(
                f,
                "{id}: a paper must belong to at least one topic or project \
                 — file it under {UNSORTED:?} if it is not sorted yet"
            ),
            Self::KindMismatch { id, message } => write!(f, "{id}: {message}"),
        }
    }
}

impl std::error::Error for EntityError {}

/// What an entity is. The closed set of things a `kovan.toml` can mark.
///
/// Adding a variant forces every `match` to handle it, which is the point —
/// see the workspace rule preferring enums over trait objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    /// One piece of literature, identified by its BibTeX cite key (§7).
    Paper,
    /// A subject-matter collection, arbitrarily nestable (§6).
    Topic,
    /// A piece of work literature is gathered for; shares the topic tree's
    /// machinery, differing only in semantics (§6).
    Project,
}

impl EntityKind {
    /// Whether this kind is a collection (topic or project) rather than a paper.
    ///
    /// Collections share one tree implementation, so most traversal code
    /// branches here rather than on the specific variant.
    pub fn is_collection(self) -> bool {
        matches!(self, Self::Topic | Self::Project)
    }

    /// The directory, relative to the library root, whose tree this kind lives
    /// in — `papers`, `topics` or `projects` in a conventional layout.
    ///
    /// Returns the *conventional* name; a library that overrides `[paths]`
    /// should resolve through [`crate::root::KovanRoot`]'s accessors instead.
    pub fn conventional_dir(self) -> &'static str {
        match self {
            Self::Paper => "papers",
            Self::Topic => "topics",
            Self::Project => "projects",
        }
    }
}

/// Whether a source document may be redistributed.
///
/// The default is [`Access::Restricted`], deliberately. §41 and this project's
/// `DATA_POLICY.md` agree: a free download is not a redistribution licence,
/// and an unknown provenance must be treated as closed. Guessing wrong in this
/// direction costs a re-download; guessing wrong in the other direction
/// publishes someone else's copyrighted PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// Openly licensed or otherwise redistributable. Committable.
    Open,
    /// Restricted, proprietary, or of unknown licence. Gitignored; never
    /// committed.
    #[default]
    Restricted,
}

impl Access {
    /// Whether documents at this access level may be committed to the
    /// repository.
    pub fn is_committable(self) -> bool {
        matches!(self, Self::Open)
    }
}

/// Where a paper's source document lives, and on what terms (§7).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SourceRef {
    /// Redistribution status. Defaults to [`Access::Restricted`].
    #[serde(default)]
    pub access: Access,
    /// Path to the source PDF, relative to the entity's own directory (so a
    /// library stays relocatable). `None` for a paper catalogued from
    /// metadata alone, with no document held locally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf: Option<PathBuf>,
}

/// Which topics and projects an entity belongs to (§7, §16).
///
/// Values are slash-separated paths within the respective tree, e.g.
/// `"htgrs/materials/graphite-properties"` — matching §16's fine-grained
/// classification syntax exactly, so paper-level and artifact-level
/// classification are written the same way.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Classification {
    /// Topic paths, e.g. `"htgrs/thermal-hydraulics"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<String>,
    /// Project paths, e.g. `"outram-park"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<String>,
}

impl Classification {
    /// The inbox classification for a paper that has not been sorted yet — a
    /// single topic, [`UNSORTED`].
    pub fn unsorted() -> Self {
        Self { topics: vec![UNSORTED.to_string()], projects: Vec::new() }
    }

    /// Whether this entity belongs to nothing at all.
    ///
    /// §7 forbids this for papers; [`EntityConfig::validate`] enforces it.
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty() && self.projects.is_empty()
    }

    /// Total number of classifications, across both trees.
    pub fn len(&self) -> usize {
        self.topics.len() + self.projects.len()
    }
}

/// A BibTeX cite key that is safe to use as a directory and file name.
///
/// # Why this is a type and not a `String`
///
/// Under §7's amendment the cite key *is* the paper's identity, and it becomes
/// a path component: `papers/<citekey>/<citekey>.md`. Auto-generated keys like
/// `wang2018multiphysics` are already safe, but hand-typed BibTeX keys are not
/// constrained by anything — they routinely carry punctuation, and a key
/// containing `/`, `..`, or a Windows-reserved name would escape the papers
/// directory or fail to create at all. §7 therefore requires validation before
/// a cite key becomes a directory name. Making it a type means that check
/// cannot be forgotten at a call site.
///
/// Use [`CiteKey::parse`] to accept a key as-is (rejecting unsafe ones), or
/// [`CiteKey::sanitise`] to derive a safe key from an unsafe one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CiteKey(String);

/// Windows reserves these device names; a file or directory so named cannot be
/// created there, with or without an extension.
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

impl CiteKey {
    /// Accept `raw` as a cite key, or explain why it cannot be one.
    ///
    /// Rejected: an empty key; `.` and `..`; anything containing a path
    /// separator (`/` or `\`), a control character, or one of the characters
    /// Windows forbids in a filename (`< > : " | ? *`); a key with leading or
    /// trailing whitespace or a trailing dot (silently stripped by Windows); a
    /// Windows reserved device name; and a key longer than 255 bytes, the
    /// common filesystem limit for one path component.
    ///
    /// # Errors
    ///
    /// [`EntityError::UnsafeCiteKey`], naming the specific reason so the
    /// message can be shown to whoever typed the key.
    pub fn parse(raw: &str) -> Result<Self, EntityError> {
        let reject = |reason: &str| {
            Err(EntityError::UnsafeCiteKey { raw: raw.to_string(), reason: reason.to_string() })
        };
        if raw.is_empty() {
            return reject("it is empty");
        }
        if raw == "." || raw == ".." {
            return reject("it names a directory-traversal entry");
        }
        if raw.len() > 255 {
            return reject("it is longer than 255 bytes, the usual limit for one path component");
        }
        if let Some(bad) = raw.chars().find(|c| {
            matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*') || c.is_control()
        }) {
            return reject(&format!("it contains {bad:?}, which is not allowed in a filename"));
        }
        if raw.trim() != raw {
            return reject("it has leading or trailing whitespace");
        }
        if raw.ends_with('.') {
            return reject("it ends with a dot, which Windows silently strips");
        }
        let stem = raw.split('.').next().unwrap_or(raw).to_ascii_uppercase();
        if WINDOWS_RESERVED.contains(&stem.as_str()) {
            return reject(&format!("{stem} is a reserved device name on Windows"));
        }
        Ok(Self(raw.to_string()))
    }

    /// Derive a safe cite key from an arbitrary string.
    ///
    /// Every character that is not ASCII-alphanumeric, `-`, `_` or `+` is
    /// replaced with `-`, runs of `-` are collapsed, and leading/trailing `-`
    /// are trimmed. The result is then put through [`CiteKey::parse`], so a
    /// sanitised key is safe by the same standard as an accepted one.
    ///
    /// **This changes the paper's identity**, so it is not applied silently:
    /// call it only where the caller can tell the user their key was rewritten
    /// and, if the key is already in a `.bib` file, update that file too.
    ///
    /// # Errors
    ///
    /// [`EntityError::UnsafeCiteKey`] if nothing usable survives — e.g. an
    /// input made entirely of punctuation.
    pub fn sanitise(raw: &str) -> Result<Self, EntityError> {
        let mut out = String::with_capacity(raw.len());
        for ch in raw.chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+') {
                out.push(ch);
            } else if !out.ends_with('-') {
                out.push('-');
            }
        }
        let trimmed = out.trim_matches('-');
        if trimmed.is_empty() {
            return Err(EntityError::UnsafeCiteKey {
                raw: raw.to_string(),
                reason: "nothing usable remains after removing unsafe characters".to_string(),
            });
        }
        Self::parse(trimmed)
    }

    /// The key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CiteKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CiteKey {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|e| e.to_string())
    }
}

impl From<CiteKey> for String {
    fn from(value: CiteKey) -> Self {
        value.0
    }
}

/// The parsed contents of an entity's `kovan.toml` (§6, §7).
///
/// One type serves all three kinds. `kind` selects the semantics, and the
/// optional sections carry what only some kinds have: `[source]` is a paper's,
/// and `name` is how a collection is displayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityConfig {
    /// On-disk format version — see [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The entity's identity, and its directory name. For a paper this is its
    /// BibTeX cite key (§7's amendment); for a collection it is its slug.
    pub id: String,
    /// What this entity is.
    pub kind: EntityKind,
    /// Human-readable display name. Absent for a paper, whose card label is
    /// formatted from its BibTeX entry instead (§9) — deliberately not stored
    /// here, so the bibliography stays the one source of bibliographic truth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where the source document is and on what terms. Papers only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    /// Which topics and projects this entity belongs to.
    #[serde(default, skip_serializing_if = "classification_is_empty")]
    pub classification: Classification,
}

fn classification_is_empty(c: &Classification) -> bool {
    c.is_empty()
}

impl EntityConfig {
    /// A paper entity, identified by its cite key, filed under [`UNSORTED`]
    /// until classified.
    ///
    /// Starting unsorted rather than unclassified is what keeps
    /// [`validate`](Self::validate) satisfiable for a paper straight out of
    /// ingestion (§7's "inbox for rapid ingestion").
    pub fn paper(id: CiteKey, access: Access) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: id.into(),
            kind: EntityKind::Paper,
            name: None,
            source: Some(SourceRef { access, pdf: None }),
            classification: Classification::unsorted(),
        }
    }

    /// A topic collection with the given slug and display name.
    pub fn topic(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::collection(EntityKind::Topic, id, name)
    }

    /// A project collection with the given slug and display name.
    pub fn project(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::collection(EntityKind::Project, id, name)
    }

    fn collection(kind: EntityKind, id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: id.into(),
            kind,
            name: Some(name.into()),
            source: None,
            classification: Classification::default(),
        }
    }

    /// Replace the topic classifications, returning `self` for chaining.
    pub fn with_topics<I, S>(mut self, topics: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.classification.topics = topics.into_iter().map(Into::into).collect();
        self
    }

    /// Replace the project classifications, returning `self` for chaining.
    pub fn with_projects<I, S>(mut self, projects: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.classification.projects = projects.into_iter().map(Into::into).collect();
        self
    }

    /// Attach a source PDF path, relative to the entity's own directory.
    pub fn with_pdf(mut self, pdf: impl Into<PathBuf>) -> Self {
        let access = self.source.as_ref().map(|s| s.access).unwrap_or_default();
        self.source = Some(SourceRef { access, pdf: Some(pdf.into()) });
        self
    }

    /// Check the invariants §6 and §7 impose beyond what the type system does.
    ///
    /// - A paper must belong to at least one topic or project.
    /// - Only a paper may carry `[source]`.
    ///
    /// # Errors
    ///
    /// [`EntityError::Unclassified`] or [`EntityError::KindMismatch`].
    pub fn validate(&self) -> Result<(), EntityError> {
        match self.kind {
            EntityKind::Paper => {
                if self.classification.is_empty() {
                    return Err(EntityError::Unclassified { id: self.id.clone() });
                }
            }
            EntityKind::Topic | EntityKind::Project => {
                if self.source.is_some() {
                    return Err(EntityError::KindMismatch {
                        id: self.id.clone(),
                        message: format!(
                            "a {:?} must not carry a [source] section — only a paper has one",
                            self.kind
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    /// Render as the TOML text of a `kovan.toml`.
    ///
    /// Hand-editable, like `kovan_root.toml` and unlike [`crate::project`]'s
    /// generated index — so it carries no "do not edit" header.
    ///
    /// # Errors
    ///
    /// Only if TOML serialisation fails, which cannot happen for these field
    /// types; the `Result` spares callers an `unwrap`.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Whether `dir` is an entity directory, i.e. directly contains a
    /// `kovan.toml`.
    ///
    /// A cheap existence check that does not parse — the same split as
    /// [`crate::root::KovanRoot::is_root`].
    pub fn is_entity(dir: &Path) -> bool {
        dir.join(ENTITY_MARKER).is_file()
    }

    /// Read the entity in `dir`.
    ///
    /// Validates the schema version but **not** the §6/§7 invariants — call
    /// [`validate`](Self::validate) for those. Loading is kept total so a
    /// library with one malformed entity can still be browsed.
    ///
    /// # Errors
    ///
    /// [`EntityError::NotAnEntity`], [`EntityError::Io`],
    /// [`EntityError::Toml`], or [`EntityError::UnsupportedSchema`].
    pub fn load(dir: &Path) -> Result<Self, EntityError> {
        let marker = dir.join(ENTITY_MARKER);
        if !marker.is_file() {
            return Err(EntityError::NotAnEntity { path: dir.to_path_buf() });
        }
        let text = std::fs::read_to_string(&marker)
            .map_err(|source| EntityError::Io { path: marker.clone(), source })?;
        let config: Self = toml::from_str(&text)
            .map_err(|e| EntityError::Toml { path: marker.clone(), message: e.to_string() })?;
        if config.schema_version > SCHEMA_VERSION {
            return Err(EntityError::UnsupportedSchema {
                path: marker,
                found: config.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(config)
    }

    /// Create a paper's directory: its `kovan.toml` **and** its canonical
    /// research Markdown (§12).
    ///
    /// `dir` is the paper's own directory, normally
    /// [`crate::root::KovanRoot::paper_dir`]. The Markdown is created at
    /// `<id>.md` inside it — same string as the directory name, per §7's
    /// amendment — seeded with a `## Summary` heading and nothing else.
    ///
    /// **An existing Markdown file is never overwritten.** The canonical
    /// Markdown is the researcher's own knowledge (§12); re-running ingestion
    /// on a paper that already exists must not destroy it.
    ///
    /// The stub deliberately contains no abstract. §9: publisher abstract
    /// prose may be copyright-protected, so the portable wiki prefers a
    /// researcher-written summary. The empty `## Summary` is that invitation.
    ///
    /// # Errors
    ///
    /// [`EntityError::KindMismatch`] if this is not a paper, plus whatever
    /// [`save`](Self::save) reports.
    pub fn save_paper(&self, dir: &Path) -> Result<(), EntityError> {
        if self.kind != EntityKind::Paper {
            return Err(EntityError::KindMismatch {
                id: self.id.clone(),
                message: format!("save_paper called on a {:?}", self.kind),
            });
        }
        self.save(dir)?;
        let markdown = dir.join(format!("{}.md", self.id));
        if !markdown.exists() {
            let stub = format!("# {}\n\n## Summary\n\n", self.id);
            std::fs::write(&markdown, stub)
                .map_err(|source| EntityError::Io { path: markdown, source })?;
        }
        Ok(())
    }

    /// Write this entity's `kovan.toml` into `dir`, creating `dir` if needed.
    ///
    /// Validates before writing, so an invalid entity never reaches disk.
    ///
    /// # Errors
    ///
    /// Whatever [`validate`](Self::validate) reports, [`EntityError::Toml`] if
    /// serialisation fails, or [`EntityError::Io`].
    pub fn save(&self, dir: &Path) -> Result<(), EntityError> {
        self.validate()?;
        std::fs::create_dir_all(dir)
            .map_err(|source| EntityError::Io { path: dir.to_path_buf(), source })?;
        let marker = dir.join(ENTITY_MARKER);
        let text = self
            .to_toml()
            .map_err(|message| EntityError::Toml { path: marker.clone(), message })?;
        std::fs::write(&marker, text)
            .map_err(|source| EntityError::Io { path: marker.clone(), source })
    }
}

/// Ensure every collection along a slash-separated `path` exists as a real
/// entity directory under `root`'s `topics/` or `projects/` tree, creating
/// whichever segments are missing (parents first).
///
/// Fixes op-8aq6 (GH issue #35's 2026-09-01 checkpoint, §6-7): classifying a
/// paper into e.g. `"htgrs/neutronics"` used to only ever write that string
/// into the paper's own `kovan.toml` — nothing created the corresponding
/// `topics/htgrs/neutronics/kovan.toml` collection entity. Since
/// [`crate::index::KnowledgeIndex::children_of`] only lists collections that
/// exist as real directories, a classification with no backing entity has no
/// link anywhere in the Wiki tree that reaches it — the paper becomes
/// permanently unreachable by drill-down, silently, even though it is still
/// on disk and in the index. Both [`crate::ingest::ingest`] and the Wiki's
/// own reclassify flow must call this before writing a classification that
/// names a path, so "classification changes where a paper appears; it must
/// never determine whether a paper exists or is discoverable" (the
/// checkpoint's own invariant) actually holds.
///
/// A segment's display name defaults to the segment itself — the same
/// "slug doubles as name until renamed" convention [`EntityConfig::save`]'s
/// own callers already use elsewhere. Already-existing segments are left
/// untouched (never overwritten), so this is safe to call unconditionally
/// before every classification write, not just the first one down a path.
pub fn ensure_collection_path(root: &KovanRoot, kind: EntityKind, path: &str) -> Result<(), EntityError> {
    let mut dir = match kind {
        EntityKind::Topic => root.topics_dir(),
        EntityKind::Project => root.projects_dir(),
        EntityKind::Paper => panic!("ensure_collection_path is for topics/projects, not papers"),
    };
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        dir = dir.join(segment);
        if !EntityConfig::is_entity(&dir) {
            let config = match kind {
                EntityKind::Topic => EntityConfig::topic(segment, segment),
                EntityKind::Project => EntityConfig::project(segment, segment),
                EntityKind::Paper => unreachable!("checked above"),
            };
            config.save(&dir)?;
        }
    }
    Ok(())
}

/// [`ensure_collection_path`] for every path in `topics` (as
/// [`EntityKind::Topic`]) and `projects` (as [`EntityKind::Project`]) —
/// the shape both [`crate::ingest::ingest`] and a reclassify action need to
/// call with one line.
pub fn ensure_classification_paths(root: &KovanRoot, topics: &[String], projects: &[String]) -> Result<(), EntityError> {
    for path in topics {
        ensure_collection_path(root, EntityKind::Topic, path)?;
    }
    for path in projects {
        ensure_collection_path(root, EntityKind::Project, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> CiteKey {
        CiteKey::parse(s).unwrap()
    }

    // -------------------------------------------------------------------
    // CiteKey — §7's "validate/sanitize a citekey before it becomes a
    // directory name"
    // -------------------------------------------------------------------

    #[test]
    fn auto_generated_style_keys_are_accepted_as_is() {
        for k in ["wang2018multiphysics", "ornl-4344", "smith_2020a", "a+b", "IAEA1694"] {
            assert!(CiteKey::parse(k).is_ok(), "{k} should be accepted");
        }
    }

    #[test]
    fn path_separators_are_rejected_so_a_key_cannot_escape_the_papers_directory() {
        for k in ["../etc/passwd", "a/b", "a\\b", ".."] {
            let err = CiteKey::parse(k).unwrap_err();
            assert!(matches!(err, EntityError::UnsafeCiteKey { .. }), "{k}: {err}");
        }
    }

    #[test]
    fn windows_hostile_keys_are_rejected() {
        for k in ["a:b", "a?b", "a*b", "a|b", "a<b", "a>b", "a\"b", "trailing.", " lead", "trail "]
        {
            let err = CiteKey::parse(k).unwrap_err();
            assert!(matches!(err, EntityError::UnsafeCiteKey { .. }), "{k}: {err}");
        }
    }

    #[test]
    fn windows_reserved_device_names_are_rejected_with_or_without_extension() {
        for k in ["CON", "con", "NUL", "COM1", "lpt9", "AUX.bib"] {
            let err = CiteKey::parse(k).unwrap_err();
            assert!(matches!(err, EntityError::UnsafeCiteKey { .. }), "{k}: {err}");
        }
    }

    #[test]
    fn empty_control_and_overlong_keys_are_rejected() {
        assert!(CiteKey::parse("").is_err());
        assert!(CiteKey::parse("a\u{0}b").is_err());
        assert!(CiteKey::parse("a\nb").is_err());
        assert!(CiteKey::parse(&"x".repeat(256)).is_err());
        // 255 is the boundary and is allowed.
        assert!(CiteKey::parse(&"x".repeat(255)).is_ok());
    }

    #[test]
    fn sanitise_derives_a_safe_key_from_a_hand_typed_one() {
        assert_eq!(CiteKey::sanitise("Wang, J. (2018)").unwrap().as_str(), "Wang-J-2018");
        assert_eq!(CiteKey::sanitise("a//b").unwrap().as_str(), "a-b");
        assert_eq!(CiteKey::sanitise("  spaced  key  ").unwrap().as_str(), "spaced-key");
        // Already safe keys pass through untouched.
        assert_eq!(CiteKey::sanitise("wang2018multiphysics").unwrap().as_str(), "wang2018multiphysics");
    }

    #[test]
    fn sanitise_fails_rather_than_inventing_an_identity() {
        // Nothing usable survives — better to refuse than to name a paper "-".
        let err = CiteKey::sanitise("///").unwrap_err();
        assert!(matches!(err, EntityError::UnsafeCiteKey { .. }), "{err}");
    }

    #[test]
    fn sanitise_output_is_always_acceptable_to_parse() {
        for raw in ["Wang, J. (2018)", "a//b", "!!!x!!!", "CON!"] {
            if let Ok(k) = CiteKey::sanitise(raw) {
                assert!(CiteKey::parse(k.as_str()).is_ok(), "{raw} -> {k} should re-parse");
            }
        }
    }

    // -------------------------------------------------------------------
    // Access — §41's safe default
    // -------------------------------------------------------------------

    #[test]
    fn access_defaults_to_restricted_because_free_download_is_not_redistribution() {
        assert_eq!(Access::default(), Access::Restricted);
        assert!(!Access::Restricted.is_committable());
        assert!(Access::Open.is_committable());
        // A [source] with no `access` key inherits the safe default.
        let s: SourceRef = toml::from_str("").unwrap();
        assert_eq!(s.access, Access::Restricted);
    }

    // -------------------------------------------------------------------
    // Kinds and validation — §6 / §7
    // -------------------------------------------------------------------

    #[test]
    fn topics_and_projects_are_collections_and_papers_are_not() {
        assert!(EntityKind::Topic.is_collection());
        assert!(EntityKind::Project.is_collection());
        assert!(!EntityKind::Paper.is_collection());
        assert_eq!(EntityKind::Paper.conventional_dir(), "papers");
        assert_eq!(EntityKind::Topic.conventional_dir(), "topics");
        assert_eq!(EntityKind::Project.conventional_dir(), "projects");
    }

    #[test]
    fn a_new_paper_starts_in_the_unsorted_inbox_so_it_is_valid_immediately() {
        let p = EntityConfig::paper(key("wang2018multiphysics"), Access::Restricted);
        assert_eq!(p.classification.topics, vec![UNSORTED.to_string()]);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn a_paper_belonging_to_nothing_is_rejected() {
        let mut p = EntityConfig::paper(key("x2020"), Access::Open);
        p.classification = Classification::default();
        let err = p.validate().unwrap_err();
        assert!(matches!(err, EntityError::Unclassified { .. }), "{err}");
    }

    #[test]
    fn a_paper_may_belong_to_many_classifications_at_once() {
        // §10: one canonical record appearing under several branches.
        let p = EntityConfig::paper(key("wang2018multiphysics"), Access::Restricted)
            .with_topics(["htgrs/neutronics", "htgrs/thermal-hydraulics"])
            .with_projects(["outram-park"]);
        assert_eq!(p.classification.len(), 3);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn a_collection_carrying_a_source_section_is_rejected() {
        let mut t = EntityConfig::topic("htgrs", "HTGRs");
        t.source = Some(SourceRef::default());
        let err = t.validate().unwrap_err();
        assert!(matches!(err, EntityError::KindMismatch { .. }), "{err}");
    }

    #[test]
    fn a_collection_needs_no_classification() {
        // Only papers must belong to something; a top-level topic belongs to
        // nothing by construction.
        assert!(EntityConfig::topic("htgrs", "HTGRs").validate().is_ok());
        assert!(EntityConfig::project("outram-park", "Outram Park").validate().is_ok());
    }

    // -------------------------------------------------------------------
    // On-disk round trips
    // -------------------------------------------------------------------

    #[test]
    fn paper_toml_matches_the_shape_the_issue_specifies() {
        let p = EntityConfig::paper(key("wang2018multiphysics"), Access::Restricted)
            .with_pdf("../../literature/proprietary/pdf/wang2018multiphysics.pdf")
            .with_topics(["htgrs/thermal-hydraulics", "htgrs/neutronics"])
            .with_projects(["outram-park"]);
        let text = p.to_toml().unwrap();

        assert!(text.contains("schema_version = 1"), "{text}");
        assert!(text.contains(r#"id = "wang2018multiphysics""#), "{text}");
        assert!(text.contains(r#"kind = "paper""#), "{text}");
        assert!(text.contains(r#"access = "restricted""#), "{text}");

        let back: EntityConfig = toml::from_str(&text).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn collection_toml_matches_the_shape_the_issue_specifies() {
        let t = EntityConfig::topic("htgrs", "HTGRs");
        let text = t.to_toml().unwrap();
        assert!(text.contains(r#"id = "htgrs""#), "{text}");
        assert!(text.contains(r#"name = "HTGRs""#), "{text}");
        assert!(text.contains(r#"kind = "topic""#), "{text}");
        // No [source] and no [classification] noise on a bare collection.
        assert!(!text.contains("[source]"), "{text}");
        assert!(!text.contains("[classification]"), "{text}");
        assert_eq!(toml::from_str::<EntityConfig>(&text).unwrap(), t);
    }

    #[test]
    fn save_then_load_round_trips_through_a_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("htgrs");
        let t = EntityConfig::topic("htgrs", "HTGRs");
        t.save(&dir).unwrap();

        assert!(EntityConfig::is_entity(&dir));
        assert_eq!(EntityConfig::load(&dir).unwrap(), t);
    }

    #[test]
    fn save_refuses_to_write_an_invalid_entity() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bad");
        let mut p = EntityConfig::paper(key("x2020"), Access::Open);
        p.classification = Classification::default();

        assert!(p.save(&dir).is_err());
        // Nothing reached disk.
        assert!(!dir.join(ENTITY_MARKER).exists());
    }

    #[test]
    fn load_on_a_plain_directory_reports_not_an_entity() {
        let tmp = tempfile::tempdir().unwrap();
        let err = EntityConfig::load(tmp.path()).unwrap_err();
        assert!(matches!(err, EntityError::NotAnEntity { .. }), "{err}");
    }

    #[test]
    fn newer_entity_schema_version_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(ENTITY_MARKER),
            "schema_version = 99\nid = \"x\"\nkind = \"topic\"\n",
        )
        .unwrap();
        let err = EntityConfig::load(tmp.path()).unwrap_err();
        assert!(matches!(err, EntityError::UnsupportedSchema { .. }), "{err}");
    }

    #[test]
    fn an_unsafe_citekey_in_a_file_is_rejected_at_deserialisation() {
        // The CiteKey newtype guards construction, so a key that would escape
        // the papers directory cannot be smuggled in through a file either.
        // Wrapped in a table because TOML has no bare-scalar document form.
        #[derive(Deserialize)]
        struct Holder {
            key: CiteKey,
        }
        assert!(toml::from_str::<Holder>(r#"key = "a/b""#).is_err());
        assert!(toml::from_str::<Holder>(r#"key = "..""#).is_err());
        let ok: Holder = toml::from_str(r#"key = "wang2018multiphysics""#).unwrap();
        assert_eq!(ok.key.as_str(), "wang2018multiphysics");
    }

    // -------------------------------------------------------------------
    // save_paper — §12's canonical Markdown
    // -------------------------------------------------------------------

    #[test]
    fn save_paper_writes_both_the_toml_and_the_canonical_markdown() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("wang2018multiphysics");
        let p = EntityConfig::paper(key("wang2018multiphysics"), Access::Restricted);
        p.save_paper(&dir).unwrap();

        assert!(dir.join(ENTITY_MARKER).is_file());
        // §7 amendment: markdown filename == directory name == citekey.
        let md = dir.join("wang2018multiphysics.md");
        assert!(md.is_file());
        let body = std::fs::read_to_string(&md).unwrap();
        assert!(body.contains("## Summary"), "{body}");
    }

    #[test]
    fn save_paper_never_overwrites_existing_research_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("wang2018multiphysics");
        let p = EntityConfig::paper(key("wang2018multiphysics"), Access::Restricted);
        p.save_paper(&dir).unwrap();

        let md = dir.join("wang2018multiphysics.md");
        std::fs::write(&md, "# mine\n\nyears of notes\n").unwrap();

        // Re-ingesting the same paper must not destroy the researcher's work.
        p.save_paper(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&md).unwrap(), "# mine\n\nyears of notes\n");
    }

    #[test]
    fn save_paper_on_a_collection_is_a_kind_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let err = EntityConfig::topic("htgrs", "HTGRs").save_paper(tmp.path()).unwrap_err();
        assert!(matches!(err, EntityError::KindMismatch { .. }), "{err}");
    }

    // -------------------------------------------------------------------
    // ensure_collection_path / ensure_classification_paths — op-8aq6
    // -------------------------------------------------------------------

    fn test_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), crate::root::RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    #[test]
    fn ensure_collection_path_creates_every_missing_segment_parents_first() {
        let (_dir, root) = test_root();
        ensure_collection_path(&root, EntityKind::Topic, "htgrs/neutronics").unwrap();

        assert!(EntityConfig::is_entity(&root.topics_dir().join("htgrs")));
        assert!(EntityConfig::is_entity(&root.topics_dir().join("htgrs/neutronics")));
        let leaf = EntityConfig::load(&root.topics_dir().join("htgrs/neutronics")).unwrap();
        assert_eq!(leaf.kind, EntityKind::Topic);
    }

    #[test]
    fn ensure_collection_path_leaves_an_existing_segment_untouched() {
        let (_dir, root) = test_root();
        let htgrs_dir = root.topics_dir().join("htgrs");
        EntityConfig::topic("htgrs", "My Custom HTGR Name").save(&htgrs_dir).unwrap();

        ensure_collection_path(&root, EntityKind::Topic, "htgrs/neutronics").unwrap();

        let htgrs = EntityConfig::load(&htgrs_dir).unwrap();
        assert_eq!(htgrs.name.as_deref(), Some("My Custom HTGR Name"), "an existing entity must not be overwritten");
    }

    #[test]
    fn ensure_collection_path_is_idempotent() {
        let (_dir, root) = test_root();
        ensure_collection_path(&root, EntityKind::Project, "outram-park").unwrap();
        ensure_collection_path(&root, EntityKind::Project, "outram-park").unwrap();
        assert!(EntityConfig::is_entity(&root.projects_dir().join("outram-park")));
    }

    #[test]
    fn ensure_classification_paths_covers_both_trees() {
        let (_dir, root) = test_root();
        ensure_classification_paths(&root, &["htgrs/materials".to_string()], &["outram-park".to_string()]).unwrap();
        assert!(EntityConfig::is_entity(&root.topics_dir().join("htgrs/materials")));
        assert!(EntityConfig::is_entity(&root.projects_dir().join("outram-park")));
    }

    /// The actual bug report (GH issue #35, op-8aq6): a paper classified
    /// into a not-yet-existing topic path must remain reachable by Wiki
    /// drill-down, i.e. `KnowledgeIndex::children_of` must now find a real
    /// link all the way down to it.
    #[test]
    fn a_paper_classified_into_a_new_topic_path_stays_reachable_by_drill_down() {
        let (_dir, root) = test_root();
        ensure_classification_paths(&root, &["htgrs/neutronics".to_string()], &[]).unwrap();
        EntityConfig::paper(key("wang2018multiphysics"), Access::Open)
            .with_topics(["htgrs/neutronics"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();

        let index = crate::index::KnowledgeIndex::rebuild(&root);
        let root_children: Vec<&str> = index.children_of("").iter().map(|c| c.path.as_str()).collect();
        assert!(root_children.contains(&"htgrs"), "{root_children:?}");
        let htgrs_children: Vec<&str> = index.children_of("htgrs").iter().map(|c| c.path.as_str()).collect();
        assert!(htgrs_children.contains(&"htgrs/neutronics"), "{htgrs_children:?}");
        let papers = index.papers_in("htgrs/neutronics");
        assert!(papers.iter().any(|p| p.citekey == "wang2018multiphysics"));
    }
}
