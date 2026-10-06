//! The concept tree, levels 1–3, as typed data (GitHub #724, #727).
//!
//! What belongs here: reading the two hand-maintained TOML files beside this
//! module into one [`ConceptTree`] that anything else (Kovan's mind map, the
//! Code Review tab, the hosted page) can walk without parsing TOML itself:
//!
//! - `concept_skeleton.toml`: the `[[document]]` list, and the `[[node]]`s of
//!   level 1 (the 19 IAEA Milestones issues, `NN-name`, `NN` = NG-G-3.1
//!   Rev. 1 §3.NN) and level 2 (regulatory review categories, unnumbered);
//! - `concept_proposals.toml`: the level-3 `[[concept]]`s. Only `approved`
//!   and `deferred` concepts are part of the standard tree; a `deferred` one
//!   is kept (and flagged, so a map can grey it) because the maintainer
//!   approved its place and is waiting only on its sources. `proposed`
//!   concepts are not in the tree. The `[[implementation]]` (level 4) seeds in
//!   the same file are **not** read here: level 4 is generated from code tags
//!   (#729) and is not part of the standard tree.
//!
//! Both files are compiled in with `include_str!` and parsed once, on first
//! use ([`concept_tree`]), so the tree needs no file system, no corpus
//! checkout and no network; this module is wasm-clean
//! (`cargo check --release -p kovan-literature --target wasm32-unknown-unknown`).
//!
//! What does not belong here: drawing, the literature filed under a node
//! (Kovan's `corpus.rs` decides that), and validation beyond what parsing
//! needs. The structural rules (19 L1 issues in order, parents exist,
//! sources name declared documents, cross-links resolve) are enforced by
//! `tests/concept_skeleton.rs` and `tests/concept_proposals.rs`, which read
//! the data through this module.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// The skeleton file (levels 1–2 and the documents), compiled in.
pub const SKELETON_TOML: &str = include_str!("concept_skeleton.toml");
/// The concepts file (level 3, plus the level-4 seeds this module ignores),
/// compiled in.
pub const PROPOSALS_TOML: &str = include_str!("concept_proposals.toml");

/// Where a document's file is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentTier {
    /// In the public standard corpus (`reactor-literature/` repository,
    /// folder `kovan-standard-open-corpus/`); `file` is relative to that
    /// repository's root.
    Standard,
    /// In the maintainer's private corpus (may not be redistributed, e.g. the
    /// IAEA documents). Cited only; no tool may require the file.
    Private,
}

/// One `[[document]]` of the skeleton: a source the tree's nodes cite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptDocument {
    /// Unique id, e.g. `nureg-0800-toc-rev6`. Sources refer to it.
    pub id: String,
    pub title: String,
    pub publisher: String,
    /// As the document prints it (`"March 2007"`, `"2015"`).
    pub date: String,
    pub tier: DocumentTier,
    /// The file, relative to its tier's repository root.
    pub file: String,
    /// The licence basis, as recorded in the skeleton.
    pub licence: String,
}

/// A node's citation of a document, at section (and sometimes page) level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptSource {
    /// A [`ConceptDocument::id`].
    pub document: String,
    /// The document's own section numbering, verbatim.
    pub section: String,
    /// The printed page, where the document gives one.
    pub page: Option<String>,
}

/// Where a level-3 concept comes from (`origin` in the proposals file).
/// Levels 1 and 2 carry no origin field: their sources say where they come
/// from, and their [`ConceptNode::origin`] is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptOrigin {
    /// A subsection an NRC (or NRC-family) document names itself.
    Nrc,
    /// A subsection of the IAEA Milestones text.
    Iaea,
    /// A concept outram-park's code needs that the cited text only implies;
    /// [`ConceptNode::why`] says why.
    OutramPark,
}

/// Whether a node is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptStatus {
    /// Approved by the maintainer (every level-1 and level-2 node is).
    Approved,
    /// Placed by the maintainer, awaiting sources. Shown, but greyed.
    Deferred,
}

/// One node of the tree, at any level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptNode {
    /// Slash-separated, parent first, no root prefix:
    /// `02-nuclear-safety/nuclear-design/neutron-transport`.
    pub path: String,
    pub title: String,
    /// 1 (IAEA issue), 2 (review category), 3 or deeper (concept).
    pub level: usize,
    pub sources: Vec<ConceptSource>,
    /// Other nodes this one also belongs to (paths; the node's home is its
    /// own path). Shown dotted on a map.
    pub cross_links: Vec<String>,
    /// `None` for levels 1–2.
    pub origin: Option<ConceptOrigin>,
    pub status: ConceptStatus,
    /// Why an `outram-park` concept exists.
    pub why: Option<String>,
    /// A judgement call recorded for the maintainer.
    pub note: Option<String>,
}

impl ConceptNode {
    /// The parent's path, or `None` for a level-1 issue.
    pub fn parent_path(&self) -> Option<&str> {
        self.path.rsplit_once('/').map(|(p, _)| p)
    }

    /// The last path segment.
    pub fn slug(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// Levels 1–3 and the documents they cite. Obtain it with [`concept_tree`].
#[derive(Debug)]
pub struct ConceptTree {
    documents: Vec<ConceptDocument>,
    /// Depth-first tree order (see [`ConceptTree::nodes`]).
    nodes: Vec<ConceptNode>,
    by_path: HashMap<String, usize>,
    /// Parent path (`""` for level 1) to child indices, in file order.
    children: HashMap<String, Vec<usize>>,
    /// Cross-link target path to the indices of the nodes linking to it.
    linked_from: HashMap<String, Vec<usize>>,
}

/// Why the TOML could not be read into a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptTreeError(pub String);

impl std::fmt::Display for ConceptTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "concept tree: {}", self.0)
    }
}

impl std::error::Error for ConceptTreeError {}

#[derive(Deserialize)]
struct RawSkeleton {
    document: Vec<RawDocument>,
    node: Vec<RawNode>,
}

#[derive(Deserialize)]
struct RawDocument {
    id: String,
    title: String,
    publisher: String,
    date: String,
    tier: String,
    file: String,
    licence: String,
}

#[derive(Deserialize)]
struct RawSource {
    document: String,
    section: String,
    #[serde(default)]
    page: Option<String>,
}

#[derive(Deserialize)]
struct RawNode {
    path: String,
    title: String,
    sources: Vec<RawSource>,
    #[serde(default)]
    cross_links: Vec<String>,
}

#[derive(Deserialize)]
struct RawProposals {
    #[serde(default)]
    concept: Vec<RawConcept>,
}

#[derive(Deserialize)]
struct RawConcept {
    path: String,
    title: String,
    origin: String,
    sources: Vec<RawSource>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    cross_links: Vec<String>,
    status: String,
}

fn sources(raw: Vec<RawSource>) -> Vec<ConceptSource> {
    raw.into_iter()
        .map(|s| ConceptSource {
            document: s.document,
            section: s.section,
            page: s.page,
        })
        .collect()
}

fn level_of(path: &str) -> usize {
    path.split('/').count()
}

impl ConceptTree {
    /// Build a tree from the two files' text. [`concept_tree`] does this
    /// once for the compiled-in files; this is public so a test or a tool can
    /// build one from edited text.
    pub fn parse(skeleton: &str, proposals: &str) -> Result<Self, ConceptTreeError> {
        let s: RawSkeleton =
            toml::from_str(skeleton).map_err(|e| ConceptTreeError(format!("skeleton: {e}")))?;
        let p: RawProposals =
            toml::from_str(proposals).map_err(|e| ConceptTreeError(format!("proposals: {e}")))?;

        let mut documents = Vec::with_capacity(s.document.len());
        for d in s.document {
            let tier = match d.tier.as_str() {
                "standard" => DocumentTier::Standard,
                "private" => DocumentTier::Private,
                other => return Err(ConceptTreeError(format!("{}: tier {other:?}", d.id))),
            };
            documents.push(ConceptDocument {
                id: d.id,
                title: d.title,
                publisher: d.publisher,
                date: d.date,
                tier,
                file: d.file,
                licence: d.licence,
            });
        }

        let mut nodes = Vec::new();
        for n in s.node {
            nodes.push(ConceptNode {
                level: level_of(&n.path),
                path: n.path,
                title: n.title,
                sources: sources(n.sources),
                cross_links: n.cross_links,
                origin: None,
                status: ConceptStatus::Approved,
                why: None,
                note: None,
            });
        }
        for c in p.concept {
            let status = match c.status.as_str() {
                "approved" => ConceptStatus::Approved,
                "deferred" => ConceptStatus::Deferred,
                "proposed" => continue,
                other => return Err(ConceptTreeError(format!("{}: status {other:?}", c.path))),
            };
            let origin = match c.origin.as_str() {
                "nrc" => ConceptOrigin::Nrc,
                "iaea" => ConceptOrigin::Iaea,
                "outram-park" => ConceptOrigin::OutramPark,
                other => return Err(ConceptTreeError(format!("{}: origin {other:?}", c.path))),
            };
            nodes.push(ConceptNode {
                level: level_of(&c.path),
                path: c.path,
                title: c.title,
                sources: sources(c.sources),
                cross_links: c.cross_links,
                origin: Some(origin),
                status,
                why: c.why,
                note: c.note,
            });
        }

        // Validate on the file order, then put the nodes in depth-first tree
        // order (roots in IAEA order, each node's children in file order), so
        // a parent always precedes its children even where the concepts
        // file lists a child before its parent.
        let mut file_index: HashMap<String, usize> = HashMap::with_capacity(nodes.len());
        for (i, n) in nodes.iter().enumerate() {
            if file_index.insert(n.path.clone(), i).is_some() {
                return Err(ConceptTreeError(format!("duplicate path {}", n.path)));
            }
        }
        let mut file_children: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            let parent = n.parent_path().unwrap_or("");
            if !parent.is_empty() && !file_index.contains_key(parent) {
                return Err(ConceptTreeError(format!(
                    "{}: parent {parent} missing",
                    n.path
                )));
            }
            file_children.entry(parent.to_string()).or_default().push(i);
        }
        let mut order = Vec::with_capacity(nodes.len());
        let mut stack: Vec<usize> = file_children
            .get("")
            .map(|v| v.iter().rev().copied().collect())
            .unwrap_or_default();
        while let Some(i) = stack.pop() {
            order.push(i);
            if let Some(kids) = file_children.get(&nodes[i].path) {
                stack.extend(kids.iter().rev().copied());
            }
        }
        let mut slots: Vec<Option<ConceptNode>> = nodes.into_iter().map(Some).collect();
        let nodes: Vec<ConceptNode> = order.into_iter().filter_map(|i| slots[i].take()).collect();

        let mut by_path = HashMap::with_capacity(nodes.len());
        let mut children: HashMap<String, Vec<usize>> = HashMap::new();
        let mut linked_from: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            by_path.insert(n.path.clone(), i);
            children
                .entry(n.parent_path().unwrap_or("").to_string())
                .or_default()
                .push(i);
            for x in &n.cross_links {
                linked_from.entry(x.clone()).or_default().push(i);
            }
        }
        Ok(Self {
            documents,
            nodes,
            by_path,
            children,
            linked_from,
        })
    }

    /// Every node, levels 1–3, in depth-first tree order: each level-1
    /// issue in IAEA order, followed by everything below it, children in
    /// file order. A parent always comes before its children.
    pub fn nodes(&self) -> &[ConceptNode] {
        &self.nodes
    }

    /// Every declared document, in file order.
    pub fn documents(&self) -> &[ConceptDocument] {
        &self.documents
    }

    /// The document with id `id`.
    pub fn document(&self, id: &str) -> Option<&ConceptDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    /// The node at `path`.
    pub fn node(&self, path: &str) -> Option<&ConceptNode> {
        self.by_path.get(path).map(|&i| &self.nodes[i])
    }

    /// The level-1 issues, in IAEA order.
    pub fn roots(&self) -> impl Iterator<Item = &ConceptNode> {
        self.children("")
    }

    /// The direct children of the node at `path` (`""` gives the roots), in
    /// file order.
    pub fn children(&self, path: &str) -> impl Iterator<Item = &ConceptNode> {
        self.children
            .get(path)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(|&i| &self.nodes[i])
    }

    /// The parent of the node at `path` (`None` for a level-1 issue or an
    /// unknown path).
    pub fn parent(&self, path: &str) -> Option<&ConceptNode> {
        self.node(path)?.parent_path().and_then(|p| self.node(p))
    }

    /// The nodes the node at `path` cross-links to. A link naming no node in
    /// the tree (one to a concept still `proposed`) is skipped.
    pub fn cross_links(&self, path: &str) -> impl Iterator<Item = &ConceptNode> {
        self.node(path)
            .map(|n| n.cross_links.as_slice())
            .unwrap_or(&[])
            .iter()
            .filter_map(|x| self.node(x))
    }

    /// The nodes that cross-link *to* `path` (the reverse direction of
    /// [`Self::cross_links`]).
    pub fn cross_linked_from(&self, path: &str) -> impl Iterator<Item = &ConceptNode> {
        self.linked_from
            .get(path)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(|&i| &self.nodes[i])
    }

    /// The documents the node at `path` cites, each once, in the order its
    /// sources first name them. An undeclared document id is skipped.
    pub fn documents_cited_by(&self, path: &str) -> Vec<&ConceptDocument> {
        let mut out: Vec<&ConceptDocument> = Vec::new();
        for s in self.node(path).map(|n| n.sources.as_slice()).unwrap_or(&[]) {
            if let Some(d) = self.document(&s.document) {
                if !out.iter().any(|o| o.id == d.id) {
                    out.push(d);
                }
            }
        }
        out
    }

    /// The nodes that cite document `id` in their sources, in tree order.
    pub fn nodes_citing(&self, id: &str) -> impl Iterator<Item = &ConceptNode> {
        let id = id.to_string();
        self.nodes
            .iter()
            .filter(move |n| n.sources.iter().any(|s| s.document == id))
    }

    /// Whether `path` is `ancestor` or lies below it.
    pub fn is_within(path: &str, ancestor: &str) -> bool {
        path == ancestor
            || path
                .strip_prefix(ancestor)
                .is_some_and(|rest| rest.starts_with('/'))
    }
}

/// The compiled-in tree, parsed on first use.
///
/// # Panics
///
/// If the compiled-in TOML does not parse, which `tests/concept_skeleton.rs`
/// rules out before anything ships.
pub fn concept_tree() -> &'static ConceptTree {
    static TREE: OnceLock<ConceptTree> = OnceLock::new();
    TREE.get_or_init(|| {
        ConceptTree::parse(SKELETON_TOML, PROPOSALS_TOML)
            .unwrap_or_else(|e| panic!("compiled-in concept tree: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_in_tree_has_nineteen_issues_in_iaea_order() {
        let t = concept_tree();
        let roots: Vec<&str> = t.roots().map(|n| n.path.as_str()).collect();
        assert_eq!(roots.len(), 19);
        assert_eq!(roots[0], "01-national-position");
        assert_eq!(roots[1], "02-nuclear-safety");
        assert_eq!(roots[18], "19-procurement");
        assert!(t.roots().all(|n| n.level == 1 && n.origin.is_none()));
    }

    #[test]
    fn a_deep_concept_resolves_with_its_parent_and_sources() {
        let t = concept_tree();
        let p = "02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant";
        let n = t.node(p).expect("majorant");
        assert_eq!(n.level, 5);
        assert_eq!(n.slug(), "majorant");
        assert!(!n.sources.is_empty());
        assert!(!t.documents_cited_by(p).is_empty());
        assert_eq!(
            t.parent(p).unwrap().path,
            "02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking"
        );
        let up = t.parent("02-nuclear-safety/nuclear-design").unwrap();
        assert_eq!(up.path, "02-nuclear-safety");
        assert!(t.parent("02-nuclear-safety").is_none());
        assert!(t
            .children("02-nuclear-safety/nuclear-design")
            .any(|c| c.path == "02-nuclear-safety/nuclear-design/neutron-transport"));
    }

    #[test]
    fn proposed_concepts_are_left_out_and_deferred_ones_kept() {
        let skeleton = r#"
[[document]]
id = "d"
title = "D"
publisher = "P"
date = "2000"
tier = "standard"
file = "f.pdf"
licence = "L"

[[node]]
path = "01-a"
title = "A"
sources = [{ document = "d", section = "1" }]

[[node]]
path = "01-a/b"
title = "B"
sources = [{ document = "d", section = "1", page = "3" }]
cross_links = ["01-a/b/d"]
"#;
        let proposals = r#"
[[concept]]
path = "01-a/b/c"
title = "C"
origin = "nrc"
sources = [{ document = "d", section = "1.1" }]
status = "approved"

[[concept]]
path = "01-a/b/d"
title = "D"
origin = "outram-park"
why = "because"
sources = [{ document = "d", section = "1.2" }]
status = "deferred"

[[concept]]
path = "01-a/b/e"
title = "E"
origin = "nrc"
sources = [{ document = "d", section = "1.3" }]
status = "proposed"
"#;
        let t = ConceptTree::parse(skeleton, proposals).unwrap();
        assert_eq!(t.nodes().len(), 4);
        assert!(t.node("01-a/b/e").is_none());
        assert_eq!(t.node("01-a/b/d").unwrap().status, ConceptStatus::Deferred);
        assert_eq!(
            t.node("01-a/b/d").unwrap().origin,
            Some(ConceptOrigin::OutramPark)
        );
        let kids: Vec<&str> = t.children("01-a/b").map(|n| n.slug()).collect();
        assert_eq!(kids, ["c", "d"]);
        assert_eq!(t.cross_links("01-a/b").next().unwrap().path, "01-a/b/d");
        assert_eq!(
            t.cross_linked_from("01-a/b/d").next().unwrap().path,
            "01-a/b"
        );
        assert_eq!(
            t.node("01-a/b").unwrap().sources[0].page.as_deref(),
            Some("3")
        );
        assert_eq!(t.nodes_citing("d").count(), 4);
        assert_eq!(t.document("d").unwrap().tier, DocumentTier::Standard);
        assert!(ConceptTree::is_within("01-a/b/c", "01-a"));
        assert!(!ConceptTree::is_within("01-ab", "01-a"));
    }

    /// The concepts file lists some children before their parents
    /// (`.../pebble-bed/pebble-bed-packing` before `.../pebble-bed`); the
    /// tree's order puts every parent first anyway.
    #[test]
    fn nodes_are_in_depth_first_order_with_parents_first() {
        let t = concept_tree();
        let mut seen = std::collections::HashSet::new();
        for n in t.nodes() {
            if let Some(p) = n.parent_path() {
                assert!(seen.contains(p), "{} before its parent", n.path);
            }
            seen.insert(n.path.as_str());
        }
        assert_eq!(t.nodes()[0].path, "01-national-position");
    }

    #[test]
    fn a_missing_parent_is_an_error() {
        let skeleton = "document = []\n[[node]]\npath = \"01-a/b\"\ntitle = \"B\"\nsources = []\n";
        assert!(ConceptTree::parse(skeleton, "").is_err());
    }
}
