//! Typed, namespaced node identities (GitHub issue #248, epic #247).
//!
//! What belongs here: [`NodeId`], the identity of anything the mind map,
//! search or a connection can point at, and its one string form. The runtime
//! graph merges several sources (the built-in corpus, the user's library, and
//! later other mounted roots), so a bare `terry2005#figure-2` is not globally
//! unique. A [`NodeId`] always says which [`Namespace`] it belongs to.
//!
//! **Strings only at the boundary.** Code passes [`NodeId`] values around;
//! only serialization (TOML, the history, egui ids) turns one into a string
//! with [`NodeId::to_string`], and only [`NodeId::parse`] turns a string back,
//! validating it. The string form is
//!
//! ```text
//! <namespace>:<kind>/<path>[#<artifact>]
//! corpus:concept/nuclear-engineering/thermal-hydraulics
//! library:concept/htgrs/fuel
//! library:literature/wang2018multiphysics#fig-2
//! ```
//!
//! What does not belong here: the older untyped ids in [`crate::graph`]
//! (`paper:…`, `collection:…`, `artifact:…#…`), which stay as they are for the
//! code that already uses them. [`NodeId::from_graph_id`] reads them into the
//! library namespace, so the two never have to be compared as strings.

use std::fmt;

/// Which source a node comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Namespace {
    /// The built-in nuclear-engineering corpus compiled into Kovan
    /// ([`crate::corpus`]). Read-only.
    Corpus,
    /// The Kovan folder the user has open.
    Library,
}

impl Namespace {
    fn as_str(self) -> &'static str {
        match self {
            Self::Corpus => "corpus",
            Self::Library => "library",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "corpus" => Some(Self::Corpus),
            "library" => Some(Self::Library),
            _ => None,
        }
    }
}

/// What kind of thing a node is, within its namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntryKind {
    /// A concept: a corpus topic, or a user topic/project. Its path is
    /// slash-separated, parent first.
    Concept,
    /// A literature entry (paper, report, book, ...), by its citekey or
    /// corpus id.
    Literature,
}

impl EntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Concept => "concept",
            Self::Literature => "literature",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "concept" => Some(Self::Concept),
            "literature" => Some(Self::Literature),
            _ => None,
        }
    }
}

/// The identity of one node in the runtime graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    pub namespace: Namespace,
    pub kind: EntryKind,
    /// The concept path or literature id. Never empty.
    pub path: String,
    /// An artifact inside a literature entry (a figure, a note), if the node
    /// is one.
    pub artifact: Option<String>,
}

/// Why a string is not a valid [`NodeId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeIdError {
    /// No `namespace:` prefix, or an unknown namespace.
    Namespace(String),
    /// No `kind/` segment, or an unknown kind.
    Kind(String),
    /// An empty path, an empty path segment, or characters a path may not
    /// hold (`:`, `#`, whitespace).
    Path(String),
    /// An artifact on a concept, or an empty artifact id.
    Artifact(String),
}

impl fmt::Display for NodeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Namespace(s) => write!(f, "{s:?}: expected corpus: or library: first"),
            Self::Kind(s) => write!(
                f,
                "{s:?}: expected concept/ or literature/ after the namespace"
            ),
            Self::Path(s) => write!(f, "{s:?}: the path is empty or has an invalid character"),
            Self::Artifact(s) => write!(
                f,
                "{s:?}: an artifact needs a literature entry and a non-empty id"
            ),
        }
    }
}

impl std::error::Error for NodeIdError {}

/// Whether `path` is non-empty, has no empty segment and holds no `:`, `#`
/// or whitespace (which would make the string form ambiguous).
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|seg| !seg.is_empty())
        && !path
            .chars()
            .any(|c| c == ':' || c == '#' || c.is_whitespace())
}

impl NodeId {
    /// A concept in `namespace` at the slash-separated `path`.
    ///
    /// # Panics
    ///
    /// If `path` is not valid (see [`NodeIdError::Path`]); use
    /// [`NodeId::parse`] for untrusted input.
    pub fn concept(namespace: Namespace, path: &str) -> Self {
        assert!(valid_path(path), "invalid concept path {path:?}");
        Self {
            namespace,
            kind: EntryKind::Concept,
            path: path.to_string(),
            artifact: None,
        }
    }

    /// A literature entry in `namespace` with id `id`.
    ///
    /// # Panics
    ///
    /// If `id` is not a valid path segment; use [`NodeId::parse`] for
    /// untrusted input.
    pub fn literature(namespace: Namespace, id: &str) -> Self {
        assert!(valid_path(id), "invalid literature id {id:?}");
        Self {
            namespace,
            kind: EntryKind::Literature,
            path: id.to_string(),
            artifact: None,
        }
    }

    /// Parse and validate the string form (see the module doc).
    pub fn parse(s: &str) -> Result<Self, NodeIdError> {
        let (ns, rest) = s
            .split_once(':')
            .ok_or_else(|| NodeIdError::Namespace(s.to_string()))?;
        let namespace =
            Namespace::parse(ns).ok_or_else(|| NodeIdError::Namespace(s.to_string()))?;
        let (kind, rest) = rest
            .split_once('/')
            .ok_or_else(|| NodeIdError::Kind(s.to_string()))?;
        let kind = EntryKind::parse(kind).ok_or_else(|| NodeIdError::Kind(s.to_string()))?;
        let (path, artifact) = match rest.split_once('#') {
            Some((p, a)) => (p, Some(a)),
            None => (rest, None),
        };
        if !valid_path(path) {
            return Err(NodeIdError::Path(s.to_string()));
        }
        if let Some(a) = artifact {
            if kind != EntryKind::Literature || !valid_path(a) || a.contains('/') {
                return Err(NodeIdError::Artifact(s.to_string()));
            }
        }
        Ok(Self {
            namespace,
            kind,
            path: path.to_string(),
            artifact: artifact.map(str::to_string),
        })
    }

    /// Read one of [`crate::graph`]'s older untyped ids (`paper:…`,
    /// `collection:…`, `artifact:<citekey>#<id>`) into the library
    /// namespace. `None` for anything else.
    pub fn from_graph_id(id: &str) -> Option<Self> {
        let (kind, rest) = id.split_once(':')?;
        let parsed = match kind {
            "collection" => Self {
                namespace: Namespace::Library,
                kind: EntryKind::Concept,
                path: rest.to_string(),
                artifact: None,
            },
            "paper" => Self {
                namespace: Namespace::Library,
                kind: EntryKind::Literature,
                path: rest.to_string(),
                artifact: None,
            },
            "artifact" => {
                let (citekey, art) = rest.split_once('#')?;
                Self {
                    namespace: Namespace::Library,
                    kind: EntryKind::Literature,
                    path: citekey.to_string(),
                    artifact: Some(art.to_string()),
                }
            }
            _ => return None,
        };
        (valid_path(&parsed.path)
            && parsed
                .artifact
                .as_deref()
                .is_none_or(|a| valid_path(a) && !a.contains('/')))
        .then_some(parsed)
    }

    /// The parent concept of a concept (`None` for a top-level concept or a
    /// literature node).
    pub fn parent_concept(&self) -> Option<Self> {
        if self.kind != EntryKind::Concept {
            return None;
        }
        let (parent, _) = self.path.rsplit_once('/')?;
        Some(Self::concept(self.namespace, parent))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}/{}",
            self.namespace.as_str(),
            self.kind.as_str(),
            self.path
        )?;
        if let Some(a) = &self.artifact {
            write!(f, "#{a}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_form_round_trips() {
        for s in [
            "corpus:concept/nuclear-engineering/thermal-hydraulics",
            "library:concept/htgrs",
            "library:literature/wang2018multiphysics",
            "library:literature/wang2018multiphysics#fig-2",
            "corpus:literature/nureg-0800",
        ] {
            let id = NodeId::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"));
            assert_eq!(id.to_string(), s);
        }
    }

    #[test]
    fn malformed_strings_are_rejected_with_the_reason() {
        use NodeIdError::*;
        let cases: [(&str, fn(&NodeIdError) -> bool); 9] = [
            ("htgrs", |e| matches!(e, Namespace(_))),
            ("mounted:concept/x", |e| matches!(e, Namespace(_))),
            ("corpus:htgrs", |e| matches!(e, Kind(_))),
            ("corpus:figure/x", |e| matches!(e, Kind(_))),
            ("corpus:concept/", |e| matches!(e, Path(_))),
            ("corpus:concept/a//b", |e| matches!(e, Path(_))),
            ("library:concept/a b", |e| matches!(e, Path(_))),
            ("library:concept/htgrs#fig", |e| matches!(e, Artifact(_))),
            ("library:literature/x#", |e| matches!(e, Artifact(_))),
        ];
        for (s, expected) in cases {
            let err = NodeId::parse(s).expect_err(s);
            assert!(expected(&err), "{s}: wrong error {err:?}");
        }
    }

    /// The same path in two namespaces is two different nodes.
    #[test]
    fn namespaces_keep_identical_paths_apart() {
        let a = NodeId::concept(Namespace::Corpus, "htgr");
        let b = NodeId::concept(Namespace::Library, "htgr");
        assert_ne!(a, b);
        assert_ne!(a.to_string(), b.to_string());
    }

    #[test]
    fn graph_ids_read_into_the_library_namespace() {
        assert_eq!(
            NodeId::from_graph_id("collection:htgrs/fuel"),
            Some(NodeId::concept(Namespace::Library, "htgrs/fuel"))
        );
        assert_eq!(
            NodeId::from_graph_id("paper:wang2018"),
            Some(NodeId::literature(Namespace::Library, "wang2018"))
        );
        let art = NodeId::from_graph_id("artifact:wang2018#fig-2").unwrap();
        assert_eq!(art.to_string(), "library:literature/wang2018#fig-2");
        assert_eq!(NodeId::from_graph_id("nonsense"), None);
    }

    #[test]
    fn a_concept_knows_its_parent() {
        let c = NodeId::concept(Namespace::Corpus, "nuclear-engineering/safety/source-term");
        assert_eq!(
            c.parent_concept(),
            Some(NodeId::concept(
                Namespace::Corpus,
                "nuclear-engineering/safety"
            ))
        );
        assert_eq!(
            NodeId::concept(Namespace::Corpus, "nuclear-engineering").parent_concept(),
            None
        );
    }
}
