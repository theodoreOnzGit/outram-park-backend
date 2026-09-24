//! The derived knowledge index — a scan of a [`KovanRoot`]'s papers and
//! collections, kept under `.kovan/index/` purely as a fast-reload cache.
//!
//! Implements GitHub issue #35 §7's `op-9vo6.7`: "repository scan + index
//! reconstruction ... this is the load-bearing guarantee of the whole
//! design — `.kovan/` holds only disposable derived/local state." The rule
//! that guarantee rests on is simple and must never be broken by a future
//! change to this module: **[`KnowledgeIndex::rebuild`] is the only source
//! of truth.** [`KnowledgeIndex::save_cache`] and [`KnowledgeIndex::load_cache`]
//! exist only to avoid re-walking the filesystem on every frame; nothing may
//! ever trust the cache over a fresh [`rebuild`](KnowledgeIndex::rebuild) when
//! the two disagree, and deleting the cache must always be harmless — see the
//! `rm -rf .kovan then rebuild reproduces the same index` test below, which is
//! the §46 "Rebuild" acceptance scenario exercised directly.
//!
//! Scope, deliberately narrow for this pass: paper and collection membership
//! only (what a plain hierarchical Wiki browser, `op-9vo6.8`, needs). The
//! full wiki-link/backlink graph is `op-9vo6.15`'s job, and full-text search
//! is later still — both build on this module rather than duplicating its
//! scan.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::entity::{Access, EntityConfig, EntityKind};
use crate::root::KovanRoot;

/// The `schema_version` this build reads and writes for the cache file.
pub const INDEX_SCHEMA_VERSION: u32 = 1;

/// Header written above the cached TOML — same "generated, do not edit"
/// convention as [`crate::project`]'s `kovan.toml` index, since this file is
/// exactly that kind of artifact: regenerable, never hand-authored.
const CACHE_HEADER: &str = "\
# GENERATED FILE — do not edit by hand.
# A derived cache of the library's papers/topics/projects, rebuilt from the
# tracked kovan.toml files under papers/, topics/ and projects/. Deleting
# this file (or the whole .kovan/ directory) is always safe — it is
# regenerated from those tracked files, never the other way around.
# See crates/kovan/src/index.rs.
";

/// Errors from persisting or reading the derived cache. Never from
/// [`KnowledgeIndex::rebuild`] itself, which is total — see its own docs.
#[derive(Debug)]
pub enum IndexError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Toml {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Toml { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for IndexError {}

/// One paper, as recorded in the index.
///
/// A thin projection of [`EntityConfig`] — just what the Wiki browser and
/// ingestion's duplicate check need. The paper's own `kovan.toml` remains the
/// authoritative record; this is a read-optimised copy of a few of its
/// fields, never written back to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperEntry {
    /// The paper's citekey (§7's amendment: this *is* its identity).
    pub citekey: String,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    /// Collection paths this paper is reachable through because one of its
    /// **artifacts** is connected to them (#276), rather than because the
    /// paper itself is filed there.
    ///
    /// A paper's own classification says where the *paper* belongs; an
    /// artifact's connection says where that *finding* belongs. A
    /// fuel-performance paper whose Figure 3 is a thermal-conductivity
    /// correlation is legitimately reachable from both topics, and this is
    /// what carries the second one. Kept **separate** from `topics` and
    /// `projects` so the distinction survives: filing a figure must never
    /// read as having refiled the paper, and a view can say *why* a paper
    /// appears under a topic.
    #[serde(default)]
    pub via_artifacts: Vec<String>,
}

/// Fill each paper's [`PaperEntry::via_artifacts`] from the user's
/// connections (#276).
///
/// A connection whose **source** is an artifact of paper `P` and whose
/// **target** is a collection means: that finding belongs to that
/// collection, so `P` is reachable from it. Connections pointing at papers
/// or at other artifacts are not classifications and are skipped here —
/// they are relationships between findings, which the graph shows
/// separately.
///
/// Best effort: a library with no connections file simply yields none, which
/// is the ordinary case and not an error.
fn attach_artifact_connections(root: &KovanRoot, papers: &mut [PaperEntry]) {
    use crate::node_id::EntryKind;
    let relations = crate::relation::connections_all(root);
    if relations.is_empty() {
        return;
    }
    for paper in papers.iter_mut() {
        let mut found: Vec<String> = relations
            .iter()
            .filter_map(|r| {
                // Both endpoints are stored as identity *strings*, so they
                // are parsed rather than matched on shape — a substring test
                // would confuse `artifact:a2020x#fig3` with a paper whose
                // citekey merely starts the same way.
                //
                // **They are `graph`-style ids** (`paper:…`, `collection:…`,
                // `artifact:<citekey>#<id>`), which `NodeId::from_graph_id`
                // reads and `NodeId::parse` does **not**: `parse` expects the
                // typed `<namespace>:<kind>/<path>` form, so it rejects
                // `artifact:…` at the namespace and returns `Err`. Using it
                // here dropped every relation silently and left
                // `via_artifacts` permanently empty — a connected artifact
                // changed nothing anywhere (maintainer, 2026-09-22).
                //
                // `parse` is still tried second: a target may be a **corpus**
                // node (`corpus:concept/…`), which only `parse` understands,
                // now that the connection finder offers the corpus.
                let read = |id: &str| {
                    crate::node_id::NodeId::from_graph_id(id)
                        .or_else(|| crate::node_id::NodeId::parse(id).ok())
                };
                let source = read(&r.source)?;
                let target = read(&r.target)?;
                let from_this_paper = source.kind == EntryKind::Literature
                    && source.artifact.is_some()
                    && source.path == paper.citekey;
                (from_this_paper && target.kind == EntryKind::Concept).then_some(target.path)
            })
            .collect();
        found.sort();
        found.dedup();
        // A path the paper is already filed under directly adds nothing.
        found.retain(|p| !paper.topics.contains(p) && !paper.projects.contains(p));
        paper.via_artifacts = found;
    }
}

/// One collection node (a topic or project), as recorded in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionEntry {
    /// Slash-separated path within its own tree, e.g. `"htgrs/materials"` —
    /// matches §16's fine-grained classification syntax exactly.
    pub path: String,
    pub kind: EntityKind,
    pub name: String,
}

/// The scanned state of a library: every paper and every collection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeIndex {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub papers: Vec<PaperEntry>,
    #[serde(default)]
    pub collections: Vec<CollectionEntry>,
}

impl KnowledgeIndex {
    /// Rebuild the index by walking `root`'s `papers/`, `topics/` and
    /// `projects/` trees and reading each `kovan.toml` found. **This is the
    /// only source of truth** — everything else in this module is a
    /// courtesy cache on top of it.
    ///
    /// Total: an entity directory whose `kovan.toml` fails to parse is
    /// silently skipped, the same "kept total so a library with one
    /// malformed entity can still be browsed" rule [`EntityConfig::load`]
    /// itself documents. A missing directory (e.g. a freshly created
    /// library with no papers yet) is likewise not an error — it just
    /// contributes nothing.
    pub fn rebuild(root: &KovanRoot) -> Self {
        let mut collections = Vec::new();
        scan_collections(
            &root.topics_dir(),
            EntityKind::Topic,
            String::new(),
            &mut collections,
        );
        scan_collections(
            &root.projects_dir(),
            EntityKind::Project,
            String::new(),
            &mut collections,
        );
        collections.sort_by(|a, b| a.path.cmp(&b.path));

        let mut papers = Vec::new();
        scan_papers(&root.paper_dirs(), &mut papers);
        papers.sort_by(|a, b| a.citekey.cmp(&b.citekey));
        attach_artifact_connections(root, &mut papers);

        Self {
            schema_version: INDEX_SCHEMA_VERSION,
            papers,
            collections,
        }
    }

    /// Persist this index to `.kovan/index/index.toml`, atomically (temp
    /// file + rename, same pattern as [`crate::project::write_index`]).
    pub fn save_cache(&self, root: &KovanRoot) -> Result<(), IndexError> {
        let dir = root.state_dir().join("index");
        std::fs::create_dir_all(&dir).map_err(|source| IndexError::Io {
            path: dir.clone(),
            source,
        })?;
        let final_path = dir.join("index.toml");
        let tmp_path = dir.join("index.toml.tmp");
        let body = toml::to_string_pretty(self).map_err(|e| IndexError::Toml {
            path: final_path.clone(),
            message: e.to_string(),
        })?;
        std::fs::write(&tmp_path, format!("{CACHE_HEADER}\n{body}")).map_err(|source| {
            IndexError::Io {
                path: tmp_path.clone(),
                source,
            }
        })?;
        std::fs::rename(&tmp_path, &final_path).map_err(|source| IndexError::Io {
            path: final_path,
            source,
        })
    }

    /// Read a previously saved cache. Returns `None` on any failure —
    /// missing file, malformed TOML, or a `schema_version` this build
    /// doesn't recognise — so a caller's only correct response to `None` is
    /// to call [`rebuild`](Self::rebuild) instead, never to treat it as
    /// fatal (§1: `rm -rf .kovan` must always be safe).
    pub fn load_cache(root: &KovanRoot) -> Option<Self> {
        let path = root.state_dir().join("index").join("index.toml");
        let text = std::fs::read_to_string(path).ok()?;
        let index: Self = toml::from_str(&text).ok()?;
        (index.schema_version == INDEX_SCHEMA_VERSION).then_some(index)
    }

    /// The normal path a GUI/CLI should call on opening a library: prefer a
    /// valid cache, falling back to (and re-persisting) a full rebuild.
    pub fn load_or_rebuild(root: &KovanRoot) -> Self {
        if let Some(cached) = Self::load_cache(root) {
            return cached;
        }
        let fresh = Self::rebuild(root);
        let _ = fresh.save_cache(root);
        fresh
    }

    /// Collections whose path is a **direct** child of `parent_path` (`""`
    /// for both tree roots together). Never returns a deeper descendant —
    /// this is what lets a browser drill down one level at a time instead
    /// of ever materialising the whole tree, which is how §8's "must not
    /// render thousands of paper nodes at root level" stays true regardless
    /// of library size.
    pub fn children_of(&self, parent_path: &str) -> Vec<&CollectionEntry> {
        self.collections
            .iter()
            .filter(|c| is_direct_child(parent_path, &c.path))
            .collect()
    }

    /// Papers classified **directly** under exactly `path` (not its
    /// descendants — a paper filed under a subtopic does not also appear at
    /// the parent topic).
    pub fn papers_in(&self, path: &str) -> Vec<&PaperEntry> {
        self.papers
            .iter()
            .filter(|p| {
                p.topics.iter().any(|t| t == path)
                    || p.projects.iter().any(|pr| pr == path)
                    // Reached through one of its artifacts (#276) — the
                    // paper appears under this topic without being filed
                    // there itself.
                    || p.via_artifacts.iter().any(|t| t == path)
            })
            .collect()
    }

    /// Whether a paper with this citekey is already in the index — the
    /// duplicate check ingestion (`op-9vo6.9`) runs before writing anything.
    pub fn has_paper(&self, citekey: &str) -> bool {
        self.papers.iter().any(|p| p.citekey == citekey)
    }
}

fn is_direct_child(parent: &str, path: &str) -> bool {
    if parent.is_empty() {
        !path.contains('/')
    } else {
        match path.strip_prefix(parent) {
            Some(rest) => rest.starts_with('/') && !rest[1..].contains('/'),
            None => false,
        }
    }
}

fn scan_collections(dir: &Path, kind: EntityKind, prefix: String, out: &mut Vec<CollectionEntry>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() || !EntityConfig::is_entity(&path) {
            continue;
        }
        let Ok(config) = EntityConfig::load(&path) else {
            continue;
        };
        if config.kind != kind {
            continue;
        }
        let slug = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let full_path = if prefix.is_empty() {
            slug.clone()
        } else {
            format!("{prefix}/{slug}")
        };
        out.push(CollectionEntry {
            path: full_path.clone(),
            kind,
            name: config.name.unwrap_or(slug),
        });
        scan_collections(&path, kind, full_path, out);
    }
}

fn scan_papers(dirs: &[std::path::PathBuf], out: &mut Vec<PaperEntry>) {
    for path in dirs {
        let Ok(config) = EntityConfig::load(path) else {
            continue;
        };
        if config.kind != EntityKind::Paper {
            continue;
        }
        out.push(PaperEntry {
            citekey: config.id,
            access: config.source.map(|s| s.access).unwrap_or_default(),
            topics: config.classification.topics,
            projects: config.classification.projects,
            // Filled by `attach_artifact_connections` once every paper is
            // scanned: it reads the library's connections, not this paper's
            // own `kovan.toml`.
            via_artifacts: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A paper is reachable from a topic one of its **artifacts** is
    /// connected to, without being filed there itself (#276) — the
    /// "this can add the paper into multiple categories" effect.
    ///
    /// The two routes stay distinguishable: `topics` is where the *paper*
    /// is filed, `via_artifacts` where a *finding* in it points.
    #[test]
    fn a_paper_is_reachable_through_its_artifacts_connections() {
        let index = KnowledgeIndex {
            papers: vec![PaperEntry {
                citekey: "ong2026fuel".to_string(),
                access: Default::default(),
                topics: vec!["htgrs/fuel".to_string()],
                projects: Vec::new(),
                via_artifacts: vec!["materials/thermal-conductivity".to_string()],
            }],
            ..Default::default()
        };

        assert_eq!(index.papers_in("htgrs/fuel").len(), 1, "filed directly");
        assert_eq!(
            index.papers_in("materials/thermal-conductivity").len(),
            1,
            "reached through an artifact's connection"
        );
        assert!(index.papers_in("lwrs").is_empty());

        // Filing a figure must not read as having refiled the paper.
        let paper = &index.papers[0];
        assert_eq!(paper.topics, vec!["htgrs/fuel".to_string()]);
        assert!(
            !paper.topics.contains(&"materials/thermal-conductivity".to_string()),
            "an artifact connection is not the paper's own classification"
        );
    }
    use crate::entity::{CiteKey, EntityConfig};
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    fn key(s: &str) -> CiteKey {
        CiteKey::parse(s).unwrap()
    }

    #[test]
    fn rebuild_on_a_fresh_library_is_empty() {
        let (_dir, root) = make_root();
        let index = KnowledgeIndex::rebuild(&root);
        assert!(index.papers.is_empty());
        assert!(index.collections.is_empty());
    }

    #[test]
    fn rebuild_finds_nested_topics_and_a_classified_paper() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        EntityConfig::topic("materials", "Materials")
            .save(&root.topics_dir().join("htgrs").join("materials"))
            .unwrap();
        EntityConfig::paper(key("wang2018multiphysics"), Access::Restricted)
            .with_topics(["htgrs/materials"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        assert_eq!(index.papers.len(), 1);
        assert_eq!(index.papers[0].citekey, "wang2018multiphysics");
        assert_eq!(index.collections.len(), 2);

        let root_children = index.children_of("");
        assert_eq!(root_children.len(), 1);
        assert_eq!(root_children[0].path, "htgrs");

        let htgrs_children = index.children_of("htgrs");
        assert_eq!(htgrs_children.len(), 1);
        assert_eq!(htgrs_children[0].path, "htgrs/materials");

        assert_eq!(index.papers_in("htgrs/materials").len(), 1);
        assert!(index.papers_in("htgrs").is_empty());
        assert!(index.has_paper("wang2018multiphysics"));
    }

    #[test]
    fn deleting_the_cache_and_rebuilding_reproduces_the_same_index() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        EntityConfig::paper(key("wang2018multiphysics"), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();

        let before = KnowledgeIndex::rebuild(&root);
        before.save_cache(&root).unwrap();
        assert!(KnowledgeIndex::load_cache(&root).is_some());

        // §46 "Rebuild": delete .kovan/ entirely and reconstruct.
        std::fs::remove_dir_all(root.state_dir()).unwrap();
        assert!(KnowledgeIndex::load_cache(&root).is_none());

        let after = KnowledgeIndex::rebuild(&root);
        assert_eq!(before, after);
    }

    #[test]
    fn load_or_rebuild_persists_a_cache_the_first_time() {
        let (_dir, root) = make_root();
        assert!(KnowledgeIndex::load_cache(&root).is_none());
        let index = KnowledgeIndex::load_or_rebuild(&root);
        assert_eq!(index, KnowledgeIndex::load_cache(&root).unwrap());
    }

    #[test]
    fn a_paper_with_malformed_toml_is_skipped_not_fatal() {
        let (_dir, root) = make_root();
        let bad_dir = root.papers_dir().join("broken");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("kovan.toml"), "not valid toml {{{").unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        assert!(index.papers.is_empty());
    }
}
