//! The runtime concept graph: the built-in corpus plus the user's library,
//! as one set of concepts (GitHub issue #249, epic #247).
//!
//! What belongs here: answering, for any concept, "what is it called, what
//! are its sub-concepts, what does it cite", whether it comes from
//! [`crate::corpus`] or from the open Kovan folder's
//! [`KnowledgeIndex`]. The Mindmap and the Wiki both read concepts only
//! through these functions, so the two views cannot disagree, and neither
//! cares which namespace a concept is in except where editing is concerned.
//!
//! The user's folder is optional everywhere (`Option<&KnowledgeIndex>`):
//! with none open, the graph is the corpus alone, which is what makes "Kovan
//! always has a nuclear-engineering mind map" hold (maintainer brief,
//! 2026-09-22).
//!
//! What does not belong here: drawing, and connections between concepts
//! (#252). No lifetimes and no stored borrow: every function takes the
//! index it reads, per the workspace Rust rules.

use crate::corpus;
use crate::entity::EntityKind;
use crate::index::KnowledgeIndex;
use crate::mindmap::{concept_citations, Citation};
use crate::node_id::{EntryKind, Namespace, NodeId};
use std::collections::HashMap;

/// Path of the synthetic library collection unclassified papers are filed
/// under (`entity.rs`'s `UNSORTED` topic, which has no directory on disk).
pub const UNSORTED_PATH: &str = "unsorted";

/// What kind of concept a node is, which decides its colour and what may be
/// done to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptKind {
    /// A built-in corpus topic. Read-only.
    CorpusTopic,
    /// A user topic.
    Topic,
    /// A user project.
    Project,
    /// The synthetic "Unsorted" collection: has no directory, takes no
    /// subtopics.
    Unsorted,
}

impl ConceptKind {
    /// Whether the user may add a subtopic under a concept of this kind.
    /// Corpus topics are read-only; attaching user knowledge to them is a
    /// connection (#252), not a subtopic.
    /// Whether a **user** subtopic may be added under a concept of this kind.
    ///
    /// `CorpusTopic` says yes (maintainer, 2026-09-22, #274) — and this is
    /// not a hole in the corpus being read-only. The corpus node itself is
    /// never edited; what is created is a node in the **user's** library,
    /// parented to the corpus one. That is the overlay epic #247 describes:
    /// *"opening a personal folder extends the map and never replaces it"*.
    /// Refusing it left the built-in nuclear-engineering branches — the only
    /// thing on screen before a folder is opened — with no way to add
    /// anything, which read as the gesture being broken.
    ///
    /// `Unsorted` says no: it is synthetic, has no directory, and exists
    /// only while unclassified papers do.
    pub fn accepts_subtopics(self) -> bool {
        matches!(self, Self::Topic | Self::Project | Self::CorpusTopic)
    }
}

/// One concept as the views show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConcept {
    pub id: NodeId,
    pub title: String,
    pub kind: ConceptKind,
    /// Number of direct sub-concepts.
    pub sub_concepts: usize,
}

fn corpus_concept(t: &corpus::CorpusTopic) -> RuntimeConcept {
    RuntimeConcept {
        id: t.id(),
        title: t.title.to_string(),
        kind: ConceptKind::CorpusTopic,
        sub_concepts: corpus::children_of(t.path).count(),
    }
}

fn library_concept(
    index: &KnowledgeIndex,
    path: &str,
    kind: EntityKind,
    name: &str,
) -> RuntimeConcept {
    RuntimeConcept {
        id: NodeId::concept(Namespace::Library, path),
        title: name.to_string(),
        kind: match kind {
            EntityKind::Project => ConceptKind::Project,
            _ => ConceptKind::Topic,
        },
        sub_concepts: index.children_of(path).len(),
    }
}

fn unsorted_concept() -> RuntimeConcept {
    RuntimeConcept {
        id: NodeId::concept(Namespace::Library, UNSORTED_PATH),
        title: "Unsorted".to_string(),
        kind: ConceptKind::Unsorted,
        sub_concepts: 0,
    }
}

/// Whether `index` has unclassified papers but no real `unsorted` topic, so
/// the synthetic "Unsorted" concept is needed to keep them reachable
/// (op-sr4n.4: a paper must never disappear for want of a classification).
fn needs_unsorted(index: &KnowledgeIndex) -> bool {
    !index.papers_in(UNSORTED_PATH).is_empty()
        && !index.collections.iter().any(|c| c.path == UNSORTED_PATH)
}

/// The concept `id` names, if it exists.
pub fn concept(index: Option<&KnowledgeIndex>, id: &NodeId) -> Option<RuntimeConcept> {
    if id.kind != EntryKind::Concept {
        return None;
    }
    match id.namespace {
        Namespace::Corpus => corpus::topic_at(&id.path).map(corpus_concept),
        Namespace::Library => {
            let index = index?;
            if let Some(c) = index.collections.iter().find(|c| c.path == id.path) {
                return Some(library_concept(index, &c.path, c.kind, &c.name));
            }
            (id.path == UNSORTED_PATH && needs_unsorted(index)).then(unsorted_concept)
        }
    }
}

/// The concepts at the top of the wiki: the corpus root, then the user's own
/// top-level topics and projects (and "Unsorted" when needed).
pub fn top_level(index: Option<&KnowledgeIndex>) -> Vec<RuntimeConcept> {
    let mut out: Vec<RuntimeConcept> = corpus::topic_at(corpus::ROOT_TOPIC)
        .map(corpus_concept)
        .into_iter()
        .collect();
    if let Some(index) = index {
        out.extend(
            index
                .children_of("")
                .into_iter()
                .filter(|c| !is_corpus_mirror(&c.path))
                .map(|c| library_concept(index, &c.path, c.kind, &c.name)),
        );
        if needs_unsorted(index) {
            out.push(unsorted_concept());
        }
    }
    out
}

/// Whether a **library** collection path is really just a mirror of a corpus
/// topic — scaffolding, not a concept of its own.
///
/// Nesting a user subtopic under a corpus branch needs that branch to exist
/// as directories on disk, because `index::scan_collections` will not walk
/// past a directory with no `kovan.toml`. Those mirrored ancestors are an
/// implementation detail of the overlay; the **corpus** node already
/// represents that concept.
///
/// Drawing them produced exactly the duplicates the maintainer reported on
/// 2026-09-22 — "there is a TRISO (corpus) and triso (topic)", "Fuel &
/// Materials now has a fuse-and-materials" — one dark-green card and one
/// light-green card for the same idea, differing only in whether the title
/// had been slugified. A user concept *inside* a mirrored path is not
/// affected: its own path is not a corpus path, so it still draws.
pub fn is_corpus_mirror(path: &str) -> bool {
    corpus::topic_at(path).is_some()
}

/// The direct sub-concepts of `parent`, or [`top_level`] for `None`.
pub fn children(index: Option<&KnowledgeIndex>, parent: Option<&NodeId>) -> Vec<RuntimeConcept> {
    let Some(parent) = parent else {
        return top_level(index);
    };
    if parent.kind != EntryKind::Concept {
        return Vec::new();
    }
    match parent.namespace {
        Namespace::Corpus => {
            // Corpus children first (dark green, immutable), then the user's
            // own subtopics written under the same path (light green,
            // editable) — the overlay #247 describes and #274 asked for.
            //
            // Without this second half, adding a subtopic under a corpus
            // topic wrote the entity to disk and drew nothing: the corpus
            // arm returned only `corpus::children_of`, so the new node had
            // nowhere to appear (maintainer, 2026-09-22: "i should see a
            // light green node popping out and linked. I don't see
            // anything").
            let mut out: Vec<RuntimeConcept> =
                corpus::children_of(&parent.path).map(corpus_concept).collect();
            if let Some(index) = index {
                out.extend(
                    index
                        .children_of(&parent.path)
                        .into_iter()
                        // Never the mirrored scaffolding: the corpus card
                        // beside it already is that concept.
                        .filter(|c| !is_corpus_mirror(&c.path))
                        .map(|c| library_concept(index, &c.path, c.kind, &c.name)),
                );
            }
            out
        }
        Namespace::Library => match index {
            Some(index) => index
                .children_of(&parent.path)
                .into_iter()
                .filter(|c| !is_corpus_mirror(&c.path))
                .map(|c| library_concept(index, &c.path, c.kind, &c.name))
                .collect(),
            None => Vec::new(),
        },
    }
}

/// The citations of concept `id`: corpus literature filed under a corpus
/// topic, or the papers classified directly under a library concept.
/// `entries` is the user's parsed bibliography ([`crate::mindmap::BibCache`]);
/// it is ignored for corpus concepts, whose metadata is compiled in.
pub fn citations(
    index: Option<&KnowledgeIndex>,
    entries: &HashMap<String, (String, String)>,
    id: &NodeId,
) -> Vec<Citation> {
    if id.kind != EntryKind::Concept {
        return Vec::new();
    }
    match id.namespace {
        Namespace::Corpus => {
            let mut out: Vec<Citation> = corpus::literature_in(&id.path)
                .map(|l| {
                    let family = l
                        .authors
                        .first()
                        .map(|a| a.split(',').next().unwrap_or(a).trim().to_string())
                        .unwrap_or_default();
                    let author_year = [family, l.year.map(|y| y.to_string()).unwrap_or_default()]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    Citation {
                        citekey: l.id.to_string(),
                        title: l.title.to_string(),
                        author_year,
                        namespace: Namespace::Corpus,
                    }
                })
                .collect();
            out.sort_by(|a, b| (&a.author_year, &a.citekey).cmp(&(&b.author_year, &b.citekey)));
            out
        }
        Namespace::Library => match index {
            Some(index) => concept_citations(index, entries, &id.path),
            None => Vec::new(),
        },
    }
}

/// Where "up one level" goes from `current` (`None` is the top): the parent
/// concept, or the top from a top-level concept. `None` when already at the
/// top, where there is nowhere to go.
pub fn up_one_level(current: Option<&NodeId>) -> Option<Option<NodeId>> {
    current.map(NodeId::parent_concept)
}

/// The breadcrumb from the top to `id`: each ancestor concept and `id`
/// itself, with its title.
pub fn breadcrumb(index: Option<&KnowledgeIndex>, id: &NodeId) -> Vec<(NodeId, String)> {
    let mut chain = Vec::new();
    let mut at = Some(id.clone());
    while let Some(node) = at {
        let title = concept(index, &node).map(|c| c.title).unwrap_or_else(|| {
            node.path
                .rsplit('/')
                .next()
                .unwrap_or(&node.path)
                .to_string()
        });
        at = node.parent_concept();
        chain.push((node, title));
    }
    chain.reverse();
    chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::{KovanRoot, RootConfig};

    fn library() -> (tempfile::TempDir, KovanRoot, KnowledgeIndex) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();
        let index = KnowledgeIndex::rebuild(&root);
        (dir, root, index)
    }

    /// With no folder open the graph is the corpus: the top is Nuclear
    /// Engineering, whose children are the nine branches.
    #[test]
    fn with_no_folder_the_graph_is_the_corpus() {
        let top = top_level(None);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].title, "Nuclear Engineering");
        assert_eq!(top[0].kind, ConceptKind::CorpusTopic);
        assert_eq!(children(None, Some(&top[0].id)).len(), 9);
        assert_eq!(top[0].sub_concepts, 9);
    }

    /// Opening a folder adds the user's concepts beside the corpus; it does
    /// not replace it.
    #[test]
    fn a_folder_extends_the_corpus_and_does_not_replace_it() {
        let (_d, _r, index) = library();
        let top = top_level(Some(&index));
        let titles: Vec<&str> = top.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["Nuclear Engineering", "HTGRs"]);
        assert_eq!(top[1].id, NodeId::concept(Namespace::Library, "htgrs"));
        let cites = citations(Some(&index), &HashMap::new(), &top[1].id);
        assert_eq!(cites[0].citekey, "wang2018multiphysics");
        assert_eq!(cites[0].namespace, Namespace::Library);
    }

    /// A corpus concept resolves with or without a folder; a library concept
    /// only with one; a missing path resolves to nothing.
    #[test]
    fn concepts_resolve_in_their_own_namespace() {
        let (_d, _r, index) = library();
        let th = NodeId::concept(Namespace::Corpus, "nuclear-engineering/thermal-hydraulics");
        assert_eq!(concept(None, &th).unwrap().title, "Thermal Hydraulics");
        assert_eq!(
            concept(Some(&index), &th).unwrap().kind,
            ConceptKind::CorpusTopic
        );
        let mine = NodeId::concept(Namespace::Library, "htgrs");
        assert!(concept(None, &mine).is_none());
        assert_eq!(
            concept(Some(&index), &mine).unwrap().kind,
            ConceptKind::Topic
        );
        assert!(concept(None, &NodeId::concept(Namespace::Corpus, "nope")).is_none());
        // A corpus topic accepts a **user** subtopic (#274, maintainer
        // 2026-09-22). This assertion was `!…` until then: the corpus node
        // itself stays immutable, but refusing the gesture left the built-in
        // branches — all you see before a folder is open — with no way to add
        // anything. `Unsorted` still refuses: it is synthetic and has no
        // directory to create anything in.
        assert!(ConceptKind::CorpusTopic.accepts_subtopics());
        assert!(ConceptKind::Topic.accepts_subtopics());
        assert!(ConceptKind::Project.accepts_subtopics());
        assert!(!ConceptKind::Unsorted.accepts_subtopics());
    }

    /// Corpus literature appears as citations of the topics it is filed
    /// under, labelled from its compiled metadata, with no folder open.
    #[test]
    fn corpus_literature_is_cited_by_its_topics() {
        let pra = NodeId::concept(Namespace::Corpus, "nuclear-engineering/pra");
        let cites = citations(None, &HashMap::new(), &pra);
        let keys: Vec<&str> = cites.iter().map(|c| c.citekey.as_str()).collect();
        assert_eq!(
            keys,
            ["nureg-2201", "wash-1400"],
            "sorted by author and year"
        );
        assert_eq!(cites[0].author_year, "Siu 2016");
        assert_eq!(
            cites[1].author_year,
            "U.S. Nuclear Regulatory Commission 1975"
        );
        assert!(cites.iter().all(|c| c.namespace == Namespace::Corpus));

        let msr = NodeId::concept(Namespace::Corpus, "nuclear-engineering/reactor-systems/msr");
        let keys: Vec<String> = citations(None, &HashMap::new(), &msr)
            .into_iter()
            .map(|c| c.citekey)
            .collect();
        assert_eq!(
            keys,
            ["nureg-cr-7289", "krpan2026physor"],
            "sorted by author and year"
        );
    }

    /// Up goes to the parent, from a top-level concept to the top, and
    /// nowhere from the top.
    #[test]
    fn up_one_level_climbs_to_the_top_and_stops() {
        let chf = NodeId::concept(
            Namespace::Corpus,
            "nuclear-engineering/thermal-hydraulics/two-phase-flow/critical-heat-flux",
        );
        let tpf = up_one_level(Some(&chf)).unwrap().unwrap();
        assert_eq!(
            tpf.path,
            "nuclear-engineering/thermal-hydraulics/two-phase-flow"
        );
        let th = up_one_level(Some(&tpf)).unwrap().unwrap();
        assert_eq!(th.path, "nuclear-engineering/thermal-hydraulics");
        let ne = up_one_level(Some(&th)).unwrap().unwrap();
        assert_eq!(ne.path, "nuclear-engineering");
        assert_eq!(
            up_one_level(Some(&ne)),
            Some(None),
            "the root goes to the top"
        );
        assert_eq!(up_one_level(None), None, "nowhere above the top");
        let mine = NodeId::concept(Namespace::Library, "htgrs");
        assert_eq!(up_one_level(Some(&mine)), Some(None));
    }

    #[test]
    fn the_breadcrumb_names_every_ancestor() {
        let chf = NodeId::concept(
            Namespace::Corpus,
            "nuclear-engineering/thermal-hydraulics/two-phase-flow/critical-heat-flux",
        );
        let titles: Vec<String> = breadcrumb(None, &chf).into_iter().map(|(_, t)| t).collect();
        assert_eq!(
            titles,
            [
                "Nuclear Engineering",
                "Thermal Hydraulics",
                "Two-Phase Flow",
                "Critical Heat Flux"
            ]
        );
    }
}
