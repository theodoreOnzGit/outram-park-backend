//! The derived knowledge graph (§33, §30, `op-9vo6.15`).
//!
//! Combines four of §33's five sources into one edge list — the fifth,
//! "filesystem hierarchy: parent -> child collection", is deliberately
//! **not** duplicated here: `KnowledgeIndex::children_of` (`op-9vo6.7`)
//! already answers that directly from the collection tree, and repeating
//! it as graph edges would be a second, driftable copy of the same fact.
//!
//! - Paper `kovan.toml` -> Topic/Project (from [`KnowledgeIndex::papers`]).
//! - Artifact TOML -> Topic/Project (from each paper's parsed artifacts).
//! - Explicit `[[target]]` / `[[target#artifact]]` wiki links.
//! - Explicit `[@citekey]` citations.
//!
//! Like [`crate::index::KnowledgeIndex`], [`KnowledgeGraph::rebuild`] is
//! the only source of truth; `.kovan/graph/` is a disposable cache
//! (§1, §46's "Rebuild" scenario).
//!
//! # `wang2018multiphysics#table-4-4` is the literal node identity
//!
//! §33: this string is the same global artifact identity `[[...]]` (wiki
//! navigation) and `[@...]` (citation) both read. [`artifact_node`]
//! constructs exactly that string, so a backlink query and a rendered
//! wiki-link never disagree on what an artifact is called.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::artifact::parse_document;
use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;

pub const GRAPH_SCHEMA_VERSION: u32 = 1;

const CACHE_HEADER: &str = "\
# GENERATED FILE — do not edit by hand.
# The derived knowledge graph: wiki links, citations, and paper/artifact
# classification, combined from the tracked kovan.toml/Markdown files.
# Deleting this file (or the whole .kovan/ directory) is always safe — it
# is regenerated from those tracked files, never the other way around.
# See crates/kovan/src/graph.rs.
";

#[derive(Debug)]
pub enum GraphError {
    Io { path: PathBuf, source: std::io::Error },
    Toml { path: PathBuf, message: String },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Toml { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for GraphError {}

/// What kind of relationship an [`Edge`] records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Paper/artifact -> Topic/Project, from `kovan.toml`/artifact TOML.
    Classification,
    /// An explicit `[[target]]` or `[[target#artifact]]` link.
    WikiLink,
    /// An explicit `[@citekey]` citation.
    Cites,
}

/// A paper node identity: `paper:<citekey>`.
pub fn paper_node(citekey: &str) -> String {
    format!("paper:{citekey}")
}

/// A collection (topic or project) node identity: `collection:<path>`,
/// using the same slash-path syntax as §16's classification lists.
pub fn collection_node(path: &str) -> String {
    format!("collection:{path}")
}

/// An artifact node identity: `artifact:<citekey>#<id>` — the literal §33
/// global identity, e.g. `artifact:wang2018multiphysics#table-4-4`.
pub fn artifact_node(citekey: &str, id: &str) -> String {
    format!("artifact:{citekey}#{id}")
}

/// One directed edge in the graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// The derived graph: every edge, from every paper currently in the
/// library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

impl KnowledgeGraph {
    /// Rebuild the whole graph from `root`'s tracked files, given an
    /// already-scanned `index` (avoids a second directory walk — callers
    /// normally already have one from `KnowledgeIndex::load_or_rebuild`).
    pub fn rebuild(root: &KovanRoot, index: &KnowledgeIndex) -> Self {
        let mut edges = Vec::new();

        for paper in &index.papers {
            let from = paper_node(&paper.citekey);
            for t in &paper.topics {
                edges.push(Edge { from: from.clone(), to: collection_node(t), kind: EdgeKind::Classification });
            }
            for p in &paper.projects {
                edges.push(Edge { from: from.clone(), to: collection_node(p), kind: EdgeKind::Classification });
            }

            let md_path = root.paper_markdown(&paper.citekey);
            let Ok(text) = std::fs::read_to_string(&md_path) else { continue };

            let parsed = parse_document(&text);
            for artifact in &parsed.artifacts {
                let art_node = artifact_node(&paper.citekey, artifact.id());
                for t in &artifact.toml.classification.topics {
                    edges.push(Edge { from: art_node.clone(), to: collection_node(t), kind: EdgeKind::Classification });
                }
                for p in &artifact.toml.classification.projects {
                    edges.push(Edge { from: art_node.clone(), to: collection_node(p), kind: EdgeKind::Classification });
                }
            }

            for link in extract_wiki_links(&text) {
                let to = match link.artifact {
                    Some(id) => artifact_node(&link.citekey, &id),
                    None => paper_node(&link.citekey),
                };
                edges.push(Edge { from: from.clone(), to, kind: EdgeKind::WikiLink });
            }
            for cited in extract_citations(&text) {
                edges.push(Edge { from: from.clone(), to: paper_node(&cited), kind: EdgeKind::Cites });
            }
        }

        edges.sort();
        edges.dedup();
        Self { schema_version: GRAPH_SCHEMA_VERSION, edges }
    }

    /// Persist to `.kovan/graph/graph.toml`, atomically — same convention
    /// as [`KnowledgeIndex::save_cache`].
    pub fn save_cache(&self, root: &KovanRoot) -> Result<(), GraphError> {
        let dir = root.state_dir().join("graph");
        std::fs::create_dir_all(&dir).map_err(|source| GraphError::Io { path: dir.clone(), source })?;
        let final_path = dir.join("graph.toml");
        let tmp_path = dir.join("graph.toml.tmp");
        let body =
            toml::to_string_pretty(self).map_err(|e| GraphError::Toml { path: final_path.clone(), message: e.to_string() })?;
        std::fs::write(&tmp_path, format!("{CACHE_HEADER}\n{body}"))
            .map_err(|source| GraphError::Io { path: tmp_path.clone(), source })?;
        std::fs::rename(&tmp_path, &final_path).map_err(|source| GraphError::Io { path: final_path, source })
    }

    /// Read a previously saved cache — `None` on any failure, exactly like
    /// [`KnowledgeIndex::load_cache`]: a caller's only correct response is
    /// to [`rebuild`](Self::rebuild) instead.
    pub fn load_cache(root: &KovanRoot) -> Option<Self> {
        let path = root.state_dir().join("graph").join("graph.toml");
        let text = std::fs::read_to_string(path).ok()?;
        let graph: Self = toml::from_str(&text).ok()?;
        (graph.schema_version == GRAPH_SCHEMA_VERSION).then_some(graph)
    }

    /// The normal call site: prefer a valid cache, falling back to (and
    /// re-persisting) a full rebuild.
    pub fn load_or_rebuild(root: &KovanRoot, index: &KnowledgeIndex) -> Self {
        if let Some(cached) = Self::load_cache(root) {
            return cached;
        }
        let fresh = Self::rebuild(root, index);
        let _ = fresh.save_cache(root);
        fresh
    }

    /// Every edge pointing **at** `target` — backlinks, always derived,
    /// never authored (§33).
    pub fn backlinks(&self, target: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.to == target).collect()
    }

    /// Every edge originating **from** `source`.
    pub fn outlinks(&self, source: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.from == source).collect()
    }
}

/// A parsed `[[target]]` / `[[target#artifact]]` wiki link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLinkRef {
    pub citekey: String,
    pub artifact: Option<String>,
}

/// Scan for `[[target]]` / `[[target#artifact]]` — a small hand-rolled
/// scan rather than a new `regex` dependency (this crate does not
/// currently depend on `regex`; `kovan-semantics` does, for an unrelated
/// purpose, and pulling it in here for one pattern is not worth a new
/// dependency edge).
pub fn extract_wiki_links(markdown: &str) -> Vec<WikiLinkRef> {
    let mut out = Vec::new();
    for (start, _) in markdown.match_indices("[[") {
        let after = &markdown[start + 2..];
        let Some(end) = after.find("]]") else { continue };
        let inner = &after[..end];
        if inner.is_empty() || inner.contains('[') || inner.contains(']') {
            continue;
        }
        let (citekey, artifact) = match inner.split_once('#') {
            Some((c, a)) => (c.to_string(), Some(a.to_string())),
            None => (inner.to_string(), None),
        };
        if !citekey.is_empty() {
            out.push(WikiLinkRef { citekey, artifact });
        }
    }
    out
}

/// Scan for `[@citekey]` citation markers.
pub fn extract_citations(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (start, _) in markdown.match_indices("[@") {
        let after = &markdown[start + 2..];
        let Some(end) = after.find(']') else { continue };
        let inner = &after[..end];
        if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':')) {
            out.push(inner.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    #[test]
    fn extract_wiki_links_finds_plain_and_artifact_targets() {
        let md = "See [[wang2018multiphysics]] and [[wang2018multiphysics#table-4-4]] for detail.";
        let links = extract_wiki_links(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], WikiLinkRef { citekey: "wang2018multiphysics".to_string(), artifact: None });
        assert_eq!(
            links[1],
            WikiLinkRef { citekey: "wang2018multiphysics".to_string(), artifact: Some("table-4-4".to_string()) }
        );
    }

    #[test]
    fn extract_citations_finds_a_bracketed_citekey() {
        let md = "Wang's methodology [@wang2018multiphysics] uses graphite data.";
        assert_eq!(extract_citations(md), vec!["wang2018multiphysics".to_string()]);
    }

    #[test]
    fn extract_functions_ignore_ordinary_markdown_links_and_footnotes() {
        assert!(extract_wiki_links("[an ordinary link](https://example.com)").is_empty());
        assert!(extract_citations("water is H[2]O, not a citation").is_empty());
    }

    #[test]
    fn rebuild_combines_classification_wiki_links_and_citations() {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        EntityConfig::topic("htgrs", "HTGRs").save(&root.topics_dir().join("htgrs")).unwrap();

        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
        EntityConfig::paper(CiteKey::parse("lee2020corrosion").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("lee2020corrosion"))
            .unwrap();

        std::fs::write(
            root.paper_markdown("wang2018multiphysics"),
            "# wang2018multiphysics\n\n## Summary\n\nCites [@lee2020corrosion] and links [[lee2020corrosion#a-note]].\n\n\
             ## A table\n\n```toml\n[kovan]\nid = \"table-1\"\nkind = \"digitised_table\"\ncreated = \"c\"\nmodified = \"m\"\n\n\
             [source]\npage = 3\n\n[classification]\ntopics = [\"htgrs\"]\n```\n\n```csv\na,b\n1,2\n```\n",
        )
        .unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);

        let wang = paper_node("wang2018multiphysics");
        let lee = paper_node("lee2020corrosion");
        let htgrs = collection_node("htgrs");
        let table = artifact_node("wang2018multiphysics", "table-1");

        assert!(graph.edges.contains(&Edge { from: wang.clone(), to: htgrs.clone(), kind: EdgeKind::Classification }));
        assert!(graph.edges.contains(&Edge { from: lee.clone(), to: htgrs.clone(), kind: EdgeKind::Classification }));
        assert!(graph.edges.contains(&Edge { from: table.clone(), to: htgrs.clone(), kind: EdgeKind::Classification }));
        assert!(graph.edges.contains(&Edge { from: wang.clone(), to: lee.clone(), kind: EdgeKind::Cites }));
        assert!(graph
            .edges
            .contains(&Edge { from: wang.clone(), to: artifact_node("lee2020corrosion", "a-note"), kind: EdgeKind::WikiLink }));

        // Backlinks are derived, not authored: htgrs has three inbound
        // classification edges without anyone writing them by hand.
        assert_eq!(graph.backlinks(&htgrs).len(), 3);
        assert_eq!(graph.backlinks(&lee).len(), 1);
    }

    #[test]
    fn deleting_the_cache_and_rebuilding_reproduces_the_same_graph() {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        let before = KnowledgeGraph::rebuild(&root, &index);
        before.save_cache(&root).unwrap();
        assert!(KnowledgeGraph::load_cache(&root).is_some());

        std::fs::remove_dir_all(root.state_dir()).unwrap();
        assert!(KnowledgeGraph::load_cache(&root).is_none());

        let after = KnowledgeGraph::rebuild(&root, &index);
        assert_eq!(before, after);
    }
}
