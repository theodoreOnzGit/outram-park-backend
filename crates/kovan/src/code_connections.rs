//! A source code repository's connections from its reviewed functions to
//! standard corpus concepts: `<repository>/docs/mindmap/connections.toml`
//! (GitHub #839; maintainer, 2026-10-12).
//!
//! # What the maintainer decided
//!
//! - "for rust source code repositories, the mindmap/connections.toml should
//!   be made under docs/mindmap/connections.toml."
//! - "It should be created fresh if not already created. If corrupted, then
//!   create fresh. Don't attempt repair."
//! - "the connections.toml in source code repositories can only connect to
//!   the standard corpus concepts."
//! - A function can be given a connection "once the code is reviewed".
//!
//! # Beside [`crate::connections`], not inside it
//!
//! That module is a **Kovan folder's** `mindmap/connections.toml`: links
//! between two mind-map nodes. This file lives in a **code repository** and
//! its source is a function, which is not a mind-map node, so it has its own
//! shape:
//!
//! ```toml
//! [[connection]]
//! source = "fn:dd41ff2953124783"
//! path = "crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction"
//! target = "corpus:concept/02-nuclear-safety/fuel-system-design"
//! ```
//!
//! `source` is the function's `fn:` id from its folder's `kovan.toml` (kept
//! when the function moves); `path` is where it was when the link was made,
//! for a reader; `target` is a standard corpus concept's node id.
//!
//! # Not part of the stamp
//!
//! The concept a function *implements* is recorded in its signed review.
//! These connections are unsigned links added afterwards: adding one never
//! changes or voids a stamp.
//!
//! # Fresh, never repaired
//!
//! [`read`] says whether the file is absent, readable or corrupted. A file
//! is corrupted when it is not TOML of this shape, or holds an entry whose
//! `source` is not an `fn:` id or whose `target` is not a standard corpus
//! concept id. A corrupted file reads as no connections, and the next
//! [`add`] replaces it with a fresh file holding only the new connection
//! (the old bytes stay in git). The file is created by the first [`add`];
//! reading never writes.
//!
//! Plain functions, no egui. [`add_for_reviewed`] loads the workspace and
//! runs the staleness engine: call it off the UI thread.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::node_id::{EntryKind, Namespace, NodeId};

/// The file, relative to the repository.
pub const REL_PATH: &str = "docs/mindmap/connections.toml";

/// One link from a function to a standard corpus concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeConnection {
    /// The function's `fn:` id.
    pub source: String,
    /// `file.rs::qual` when the link was made.
    pub path: String,
    /// A standard corpus concept.
    pub target: NodeId,
}

/// What [`read`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// No file yet: no connections.
    Absent,
    Readable(Vec<CodeConnection>),
    /// Not this module's shape (module doc); the reason is for the user.
    Corrupted(String),
}

impl FileState {
    /// The connections: none when absent or corrupted.
    pub fn connections(self) -> Vec<CodeConnection> {
        match self {
            Self::Readable(c) => c,
            Self::Absent | Self::Corrupted(_) => Vec::new(),
        }
    }
}

/// Why a connection was not added or removed.
#[derive(Debug)]
pub enum CodeConnectionError {
    /// The target is not a concept of the standard corpus.
    NotStandardConcept(String),
    /// The source is not an `fn:` id.
    NotAFunctionId(String),
    /// Already recorded.
    Duplicate { path: String, target: String },
    /// The function has no valid review (the reason follows).
    NotReviewed(String),
    /// The workspace could not be read.
    Workspace(String),
    Io(std::io::Error),
    /// The file could not be rendered as TOML (not reachable through this
    /// module's own types; kept so a future field cannot panic here).
    Render(String),
}

impl std::fmt::Display for CodeConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStandardConcept(t) => write!(
                f,
                "{t} is not a concept of the standard corpus: a code repository's connections \
                 can only point at standard corpus concepts"
            ),
            Self::NotAFunctionId(s) => write!(f, "{s} is not an fn: id"),
            Self::Duplicate { path, target } => write!(f, "{path} is already connected to {target}"),
            Self::NotReviewed(why) => write!(
                f,
                "a connection can be added once the function is reviewed: {why}"
            ),
            Self::Workspace(e) | Self::Render(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CodeConnectionError {}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default, rename = "connection")]
    connections: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    source: String,
    path: String,
    target: String,
}

/// `<repo>/docs/mindmap/connections.toml`.
pub fn file_path(repo: &Path) -> PathBuf {
    repo.join(REL_PATH)
}

/// Whether `id` names a concept in the standard corpus namespace.
fn is_standard_concept_id(id: &NodeId) -> bool {
    id.namespace == Namespace::Corpus && id.kind == EntryKind::Concept && id.artifact.is_none()
}

/// The standard corpus concept `id` names, if the built-in corpus has it.
fn standard_concept(id: &NodeId) -> bool {
    is_standard_concept_id(id) && crate::corpus::topic_at(&id.path).is_some()
}

/// Parse the file's text (module doc, "Fresh, never repaired").
fn parse(text: &str) -> Result<Vec<CodeConnection>, String> {
    let file: File = toml::from_str(text).map_err(|e| format!("not readable TOML: {e}"))?;
    file.connections
        .into_iter()
        .map(|e| {
            if !e.source.starts_with("fn:") {
                return Err(format!("source {:?} is not an fn: id", e.source));
            }
            let target = NodeId::parse(&e.target)
                .ok()
                .filter(is_standard_concept_id)
                .ok_or_else(|| format!("target {:?} is not a standard corpus concept", e.target))?;
            Ok(CodeConnection {
                source: e.source,
                path: e.path,
                target,
            })
        })
        .collect()
}

/// Read `repo`'s connections file. Never writes.
pub fn read(repo: &Path) -> FileState {
    match std::fs::read_to_string(file_path(repo)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Absent,
        Err(e) => FileState::Corrupted(format!("cannot be read: {e}")),
        Ok(text) => match parse(&text) {
            Ok(c) => FileState::Readable(c),
            Err(why) => FileState::Corrupted(why),
        },
    }
}

/// The connections of function `source` (an `fn:` id) in `repo`.
pub fn for_function(repo: &Path, source: &str) -> Vec<CodeConnection> {
    read(repo)
        .connections()
        .into_iter()
        .filter(|c| c.source == source)
        .collect()
}

fn write(repo: &Path, connections: &[CodeConnection]) -> Result<(), CodeConnectionError> {
    let file = File {
        connections: connections
            .iter()
            .map(|c| Entry {
                source: c.source.clone(),
                path: c.path.clone(),
                target: c.target.to_string(),
            })
            .collect(),
    };
    let body =
        toml::to_string_pretty(&file).map_err(|e| CodeConnectionError::Render(e.to_string()))?;
    let path = file_path(repo);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(CodeConnectionError::Io)?;
    }
    let header = "# Kovan code connections.\n\
                  #\n\
                  # One entry per link from a reviewed function of this repository to a\n\
                  # concept of Kovan's standard corpus. Written by desktop kovan (Code\n\
                  # Review, right-click a reviewed function). Unsigned: not part of any\n\
                  # review stamp.\n\n";
    std::fs::write(path, format!("{header}{body}")).map_err(CodeConnectionError::Io)
}

/// What [`add`] did to the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// The file did not exist and was created.
    pub created: bool,
    /// The file was corrupted (the reason) and was replaced by a fresh one
    /// holding only this connection.
    pub replaced_corrupted: Option<String>,
}

/// Record a connection from function `source` (an `fn:` id, at `path`) to
/// the standard corpus concept `target` in `repo` (module doc). Does not
/// check the function's review: [`add_for_reviewed`] does.
pub fn add(
    repo: &Path,
    source: &str,
    path: &str,
    target: &NodeId,
) -> Result<Added, CodeConnectionError> {
    if !source.starts_with("fn:") {
        return Err(CodeConnectionError::NotAFunctionId(source.to_string()));
    }
    if !standard_concept(target) {
        return Err(CodeConnectionError::NotStandardConcept(target.to_string()));
    }
    let (mut all, created, replaced_corrupted) = match read(repo) {
        FileState::Absent => (Vec::new(), true, None),
        FileState::Readable(c) => (c, false, None),
        FileState::Corrupted(why) => (Vec::new(), false, Some(why)),
    };
    if all.iter().any(|c| c.source == source && &c.target == target) {
        return Err(CodeConnectionError::Duplicate {
            path: path.to_string(),
            target: target.to_string(),
        });
    }
    all.push(CodeConnection {
        source: source.to_string(),
        path: path.to_string(),
        target: target.clone(),
    });
    write(repo, &all)?;
    Ok(Added {
        created,
        replaced_corrupted,
    })
}

/// Remove the connection from `source` to `target`; false when there was
/// none (a corrupted file holds none, and is left as it is).
pub fn remove(repo: &Path, source: &str, target: &NodeId) -> Result<bool, CodeConnectionError> {
    let FileState::Readable(mut all) = read(repo) else {
        return Ok(false);
    };
    let before = all.len();
    all.retain(|c| !(c.source == source && &c.target == target));
    if all.len() == before {
        return Ok(false);
    }
    write(repo, &all)?;
    Ok(true)
}

/// A connection added to a reviewed function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedForFunction {
    /// The function's `fn:` id.
    pub source: String,
    pub added: Added,
}

/// [`add`] for the function `function` names (an `fn:` id or a call-graph
/// id) in the workspace at `repo`, **only when it has a valid review**: the
/// staleness engine's state of the function is valid, or pending only a
/// workspace test (`StateKind::review_holds`); stale, unverified and
/// unreviewed functions are refused with the state named.
pub fn add_for_reviewed(
    repo: &Path,
    function: &str,
    target: &NodeId,
) -> Result<AddedForFunction, CodeConnectionError> {
    use kovan_common::review::state::StateKind;
    let ws = crate::stamping::load_workspace(repo).map_err(CodeConnectionError::Workspace)?;
    let (idx, file, f) = ws.find(function).ok_or_else(|| {
        CodeConnectionError::Workspace(format!(
            "{function} is not in any kovan.toml (run Index fresh)"
        ))
    })?;
    let (source, path) = (f.id.clone(), format!("{}::{}", idx.file_path(file), f.qual));
    let we = crate::stamping::evaluate_loaded(repo, &ws);
    let kind = we
        .evaluation
        .functions
        .get(&source)
        .map_or(StateKind::New, |r| r.state.kind());
    // "Pending workspace test" is a review that stands (the function is
    // unchanged; `Cargo.lock` moved): CORRECTED 2026-10-13, it was refused,
    // so a function reviewed before an unrelated dependency change could
    // not be connected.
    if !kind.review_holds() {
        return Err(CodeConnectionError::NotReviewed(format!(
            "{path} is {}",
            kind.label()
        )));
    }
    let added = add(repo, &source, &path, target)?;
    Ok(AddedForFunction { source, added })
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: &str = "fn:00000000000000aa";
    const P: &str = "crates/demo/src/lib.rs::leaf";

    /// Two concepts of the built-in standard corpus (any two).
    fn two_concepts() -> (NodeId, NodeId) {
        let mut t = crate::corpus::topics()
            .iter()
            .filter(|t| t.concept.is_some())
            .map(|t| t.id());
        (t.next().unwrap(), t.next().unwrap())
    }

    /// Methodology: in a temporary folder standing for a code repository,
    /// add connections from a function to standard corpus concepts and read
    /// them back; try a library concept, a literature node, a concept the
    /// corpus does not have, a bad source and a duplicate; remove one.
    ///
    /// Pass: the first add creates `docs/mindmap/connections.toml`; both
    /// connections read back in order and by function; every non-standard
    /// target and the bad source are refused and write nothing; a duplicate
    /// is refused; remove takes one out and reports a second remove as
    /// nothing to do. Reading an absent file writes nothing.
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn connections_point_only_at_standard_corpus_concepts() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path();
        let (a, b) = two_concepts();
        assert_eq!(read(repo), FileState::Absent);
        assert!(!file_path(repo).exists(), "reading never writes");

        for bad in [
            NodeId::concept(Namespace::Library, &a.path),
            NodeId::literature(Namespace::Corpus, "some-paper"),
            NodeId::concept(Namespace::Corpus, "no-such-concept-in-the-corpus"),
        ] {
            let e = add(repo, F, P, &bad).unwrap_err();
            assert!(matches!(e, CodeConnectionError::NotStandardConcept(_)), "{e}");
        }
        let e = add(repo, "crates/demo/src/lib.rs::leaf", P, &a).unwrap_err();
        assert!(matches!(e, CodeConnectionError::NotAFunctionId(_)), "{e}");
        assert!(!file_path(repo).exists(), "a refusal writes nothing");

        let first = add(repo, F, P, &a).unwrap();
        assert_eq!(first, Added { created: true, replaced_corrupted: None });
        assert!(repo.join("docs/mindmap/connections.toml").is_file());
        let second = add(repo, F, P, &b).unwrap();
        assert_eq!(second, Added { created: false, replaced_corrupted: None });
        let e = add(repo, F, P, &a).unwrap_err();
        assert!(matches!(e, CodeConnectionError::Duplicate { .. }), "{e}");

        let all = read(repo).connections();
        assert_eq!(all.iter().map(|c| &c.target).collect::<Vec<_>>(), [&a, &b]);
        assert_eq!(for_function(repo, F).len(), 2);
        assert!(for_function(repo, "fn:00000000000000bb").is_empty());

        assert!(remove(repo, F, &a).unwrap());
        assert!(!remove(repo, F, &a).unwrap());
        assert_eq!(read(repo).connections().len(), 1);
    }

    /// Methodology: write each kind of corrupted file (not TOML, an unknown
    /// field, a source that is not an `fn:` id, a target that is not a
    /// standard corpus concept), read it, then add a connection.
    ///
    /// Pass: each reads as corrupted with no connections, and is left
    /// untouched by reading and by remove; the add replaces it with a fresh
    /// file holding only the new connection and reports the reason. Nothing
    /// of the old file is carried over (no repair).
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn a_corrupted_file_is_replaced_fresh_never_repaired() {
        let (a, b) = two_concepts();
        let good = format!("[[connection]]\nsource = \"{F}\"\npath = \"{P}\"\ntarget = \"{b}\"\n");
        let corrupted = [
            "this is { not toml".to_string(),
            format!("{good}\n[[connection]]\nsource = \"{F}\"\npath = \"{P}\"\ntarget = \"{a}\"\nextra = 1\n"),
            format!("{good}\n[[connection]]\nsource = \"leaf\"\npath = \"{P}\"\ntarget = \"{a}\"\n"),
            format!("{good}\n[[connection]]\nsource = \"{F}\"\npath = \"{P}\"\ntarget = \"library:concept/x\"\n"),
        ];
        for text in corrupted {
            let d = tempfile::tempdir().unwrap();
            let repo = d.path();
            std::fs::create_dir_all(file_path(repo).parent().unwrap()).unwrap();
            std::fs::write(file_path(repo), &text).unwrap();
            assert!(matches!(read(repo), FileState::Corrupted(_)), "{text}");
            assert!(for_function(repo, F).is_empty());
            assert!(!remove(repo, F, &b).unwrap());
            assert_eq!(std::fs::read_to_string(file_path(repo)).unwrap(), text, "left as it is");

            let added = add(repo, F, P, &a).unwrap();
            assert!(!added.created && added.replaced_corrupted.is_some(), "{added:?}");
            let all = read(repo).connections();
            assert_eq!(all.len(), 1, "fresh: only the new connection, {text}");
            assert_eq!(all[0].target, a);
        }
    }

    /// Methodology: on the stamping fixture (a throwaway git repository),
    /// try to connect `leaf` before any review, stamp it and connect it,
    /// then edit it (so its review is stale) and try again.
    ///
    /// Pass: refused while unreviewed and while stale, naming the state;
    /// accepted while the stamp is valid, recording the function's `fn:` id
    /// and path; an unknown function is refused. Commit and push plans the
    /// connections file as a file kovan wrote.
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn only_a_function_with_a_valid_review_gets_a_connection() {
        use crate::stamping::tests::{founder_key, Repo, LIB};
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        let (a, b) = two_concepts();
        let leaf = format!("{LIB}::leaf");

        let e = add_for_reviewed(r.path(), &leaf, &a).unwrap_err();
        assert!(matches!(e, CodeConnectionError::NotReviewed(_)), "{e}");
        let e = add_for_reviewed(r.path(), "crates/demo/src/lib.rs::absent", &a).unwrap_err();
        assert!(matches!(e, CodeConnectionError::Workspace(_)), "{e}");

        r.stamp(&key, "leaf", "");
        let ok = add_for_reviewed(r.path(), &leaf, &a).unwrap();
        assert!(ok.source.starts_with("fn:") && ok.added.created);
        let all = for_function(r.path(), &ok.source);
        assert_eq!((all.len(), all[0].path.as_str()), (1, leaf.as_str()));

        // Commit and push takes the file (it is one of the kinds kovan
        // writes), and its message names it. Not on `main`: that is refused.
        r.git(&["checkout", "-q", "-b", "develop"]);
        let session = std::collections::BTreeSet::from([REL_PATH.to_string()]);
        let plan = crate::stamping::commit_push::plan(r.path(), &session).unwrap();
        assert_eq!(plan.files, [REL_PATH]);
        assert!(plan.message.contains("function connections"), "{}", plan.message);

        r.edit("x * 2.0", "x * 3.0");
        r.commit("edit leaf");
        r.reindex();
        let e = add_for_reviewed(r.path(), &leaf, &b).unwrap_err();
        assert!(matches!(e, CodeConnectionError::NotReviewed(_)), "{e}");
        assert_eq!(for_function(r.path(), &ok.source).len(), 1);
    }
}
