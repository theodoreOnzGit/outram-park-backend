//! Mindmap view-model: expansion state, semantic zoom, focus (op-30um.7).
//!
//! What belongs here: a GUI-independent model of the mindmap's *logical*
//! graph — which nodes exist, which are currently visible, and the three
//! behavioural rules the maintainer's headless Python dogfood
//! (`collaboration/kovan-issue-35-prototypes/layer3-mindmap-ui/mindmap_model.py`,
//! a behaviour spec, not code to port literally — see `bn show op-30um.7`)
//! pins down:
//!
//! 1. **Expansion/collapse is explicit user state and is the only thing
//!    that controls visible topology.** Nothing else — not zoom, not a
//!    relayout, not opening a different paper — may add or remove a node
//!    from [`MindmapModel::visible_nodes`].
//! 2. **Semantic zoom changes node presentation only.** [`DetailLevel`] is
//!    derived from the zoom factor and never touches `expanded`/`selected`.
//! 3. **[`MindmapModel::focus`] expands only the target's ancestor path** —
//!    the headless equivalent of the egui "Show in Mindmap"/"Centre
//!    Selection" action. It never expands siblings or unrelated relations.
//!
//! What does **not** belong here: pixel/world-space positions, cameras and
//! the ASCII renderer (that is [`crate::mindmap_layout`]); egui widgets and
//! rendering, colour resolution, and wiring into the existing hand-rolled
//! painter (that is [`crate::mindmap`] and `crate::app`, out of scope for
//! this pass per `bn show op-30um.7`); and the canonical user-authored
//! relation model (`RelationKind`/`UserRelation`, `op-30um.1`, landing in
//! [`crate::relation`]) — this module only consumes a plain [`TypedEdge`]
//! input, documented below as the seam that later work plugs into.
//!
//! # Node identity
//!
//! Every node id is built with [`crate::graph`]'s `paper_node` /
//! `collection_node` / `artifact_node` helpers — never formatted by hand —
//! so a mindmap node id is always the same string a wiki-link, a citation,
//! or a [`crate::graph::KnowledgeGraph`] edge would use for the same
//! entity.
//!
//! # Colour is not this module's job
//!
//! [`MapNodeKind::Artifact`] carries the artifact's real
//! [`crate::artifact::ArtifactKind`] rather than a pre-resolved colour.
//! Another agent is adding the central Gruvbox accent-by-kind mapping to
//! `app/theme.rs` in this same work wave; this module deliberately does not
//! import from `crate::app` or duplicate that mapping — a caller with GUI
//! access resolves the accent from the kind exposed here.

use std::collections::{HashMap, HashSet};

use crate::artifact::ArtifactKind;
use crate::entity::EntityKind;
use crate::graph::{self, KnowledgeGraph};
use crate::index::KnowledgeIndex;
use crate::research_record::ResearchRecordIndex;
use crate::root::KovanRoot;
use crate::session::PaperSession;

/// What kind of thing a [`MapNode`] represents.
///
/// A thin classification over the node id's own namespace
/// (`collection:`/`paper:`/`artifact:`, per [`crate::graph`]) — kept as an
/// enum, not re-derived from the id string at every use site, so a `match`
/// on it is exhaustive and a new collection/artifact kind is a compile
/// error everywhere it matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapNodeKind {
    /// A topic or project collection (`collection:<path>`). `EntityKind`
    /// is reused as-is from [`crate::index::CollectionEntry::kind`] rather
    /// than a second topic/project enum — in practice this is always
    /// [`EntityKind::Topic`] or [`EntityKind::Project`], never
    /// [`EntityKind::Paper`], since only [`KnowledgeIndex::collections`]
    /// feeds this variant.
    Collection(EntityKind),
    /// A paper (`paper:<citekey>`).
    Paper,
    /// A research artifact (`artifact:<citekey>#<id>`), carrying its real
    /// [`ArtifactKind`] so a GUI caller can resolve an accent colour from
    /// it (see the module doc's "Colour is not this module's job").
    Artifact(ArtifactKind),
}

/// One node in the mindmap's logical graph.
///
/// `parent` is the single hierarchy edge [`MindmapModel::visible_nodes`]
/// walks — deliberately not a `Vec` of parents, matching the prototype's
/// choice to keep classification (many-to-many, already a
/// [`crate::graph::EdgeKind::Classification`] edge) separate from the tree
/// a mindmap actually draws chevrons over. A paper today has `parent:
/// None`; its topic/project memberships surface as ordinary edges, not as
/// a second tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapNode {
    /// The node's identity — always built by [`graph::paper_node`],
    /// [`graph::collection_node`] or [`graph::artifact_node`].
    pub id: String,
    pub kind: MapNodeKind,
    /// Display title. For a paper this is its citekey today — a nicer
    /// bibliography-derived title (see [`crate::mindmap::bib_display`],
    /// which this module deliberately does not depend on to stay
    /// GUI-layer-free) is a follow-on for whichever pass wires this model
    /// into the GUI.
    pub title: String,
    /// The single node this one hangs off in the *hierarchy* tree, or
    /// `None` for a root (a top-level collection, or every paper today).
    pub parent: Option<String>,
    /// A short secondary line — currently only populated for artifacts
    /// (their `ArtifactKind`, `{:?}`-formatted).
    pub subtitle: String,
}

/// One edge in the mindmap's logical graph — either a canonical
/// [`KnowledgeGraph`] edge (classification/wiki-link/citation) or a
/// user-authored relation coming in through the [`TypedEdge`] seam.
/// `label` is a short, human-readable tag (`"Classification"`,
/// `"WikiLink"`, `"Cites"`, or whatever [`TypedEdge::label`] said) —
/// display only, never re-parsed to recover the original
/// [`crate::graph::EdgeKind`]/`RelationKind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEdge {
    pub source: String,
    pub target: String,
    pub label: String,
}

/// The seam this module exposes for user-authored typed relations, until
/// `op-30um.1` lands the canonical `RelationKind`/`UserRelation` model in
/// [`crate::relation`].
///
/// This is deliberately **not** the canonical relation type — it is a
/// plain, engine-agnostic `(source, target, label)` triple. Once
/// `op-30um.1` exists, its expected shape is a small adapter at the
/// `build_model` call site that turns each `UserRelation` into one
/// `TypedEdge` (`label` from `RelationKind`'s `Display`/`Debug`, most
/// likely) — this module should not need to change for that to happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedEdge {
    pub source: String,
    pub target: String,
    pub label: String,
}

/// Semantic zoom's three presentation levels (prototype thresholds:
/// `< 0.55` compact, `< 1.15` normal, otherwise detailed). Presentation
/// only — see the module doc's rule 2. A GUI caller uses this to decide
/// how much detail to draw on a node, never whether the node is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Compact,
    Normal,
    Detailed,
}

/// An operation on a [`MindmapModel`] was given a node id the model has
/// never seen (via [`MindmapModel::add_node`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownNodeError(pub String);

impl std::fmt::Display for UnknownNodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no such mindmap node: {}", self.0)
    }
}

impl std::error::Error for UnknownNodeError {}

/// The mindmap view-model: every known node/edge, plus the user-controlled
/// expansion/selection/zoom state layered over them.
///
/// Construct with [`build_model`] from a live library, or assemble by hand
/// with [`MindmapModel::add_node`]/[`add_edge`](Self::add_edge) for tests
/// (see this module's own test suite for the latter).
#[derive(Debug, Clone)]
pub struct MindmapModel {
    nodes: HashMap<String, MapNode>,
    /// `parent id -> child ids`, in the order children were added. Only
    /// [`MindmapModel::visible_nodes`]' hierarchy walk reads this
    /// directly; [`crate::mindmap_layout`] re-derives its own sorted
    /// order from [`MindmapModel::visible_nodes`] rather than trusting
    /// this map's iteration order, so insertion order here is not itself
    /// a determinism guarantee.
    hierarchy: HashMap<String, Vec<String>>,
    edges: Vec<MapEdge>,
    expanded: HashSet<String>,
    selected: Option<String>,
    zoom: f64,
}

impl Default for MindmapModel {
    fn default() -> Self {
        Self::new()
    }
}

impl MindmapModel {
    /// An empty model with no nodes/edges, nothing expanded, nothing
    /// selected, and zoom at `1.0` (matches the prototype's default).
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            hierarchy: HashMap::new(),
            edges: Vec::new(),
            expanded: HashSet::new(),
            selected: None,
            zoom: 1.0,
        }
    }

    /// Add (or replace) one node. Registers it under its parent's child
    /// list when `node.parent` is `Some`.
    pub fn add_node(&mut self, node: MapNode) {
        if let Some(parent) = &node.parent {
            self.hierarchy
                .entry(parent.clone())
                .or_default()
                .push(node.id.clone());
        }
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add one edge — a canonical [`KnowledgeGraph`] edge or a
    /// user-relation [`TypedEdge`], already converted to a [`MapEdge`] by
    /// the caller (see [`build_model`]).
    pub fn add_edge(&mut self, edge: MapEdge) {
        self.edges.push(edge);
    }

    /// Look a node up by id.
    pub fn node(&self, id: &str) -> Option<&MapNode> {
        self.nodes.get(id)
    }

    /// The ids of `id`'s direct children in the hierarchy tree, in the
    /// order they were added. Empty (not an error) for a node with no
    /// children, or for an id the model does not know.
    pub fn children_of(&self, id: &str) -> &[String] {
        self.hierarchy.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every node this model knows about, in no particular order — use
    /// [`visible_nodes`](Self::visible_nodes) for anything display-facing.
    pub fn all_nodes(&self) -> impl Iterator<Item = &MapNode> {
        self.nodes.values()
    }

    /// Flip `node_id` between expanded and collapsed.
    pub fn toggle(&mut self, node_id: &str) {
        if !self.expanded.remove(node_id) {
            self.expanded.insert(node_id.to_string());
        }
    }

    /// Explicitly expand `node_id` (a no-op if already expanded, or if
    /// `node_id` is not a known node — expansion state for an id the
    /// model has never seen is harmless and simply has no visible
    /// effect).
    pub fn expand(&mut self, node_id: &str) {
        self.expanded.insert(node_id.to_string());
    }

    /// Explicitly collapse `node_id`.
    pub fn collapse(&mut self, node_id: &str) {
        self.expanded.remove(node_id);
    }

    /// Whether `node_id` is currently expanded.
    pub fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded.contains(node_id)
    }

    /// The currently selected node, if any.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select `node_id` and expand only its ancestor path — the headless
    /// equivalent of egui's "Show in Mindmap" / "Centre Selection". Never
    /// touches any node outside that path: siblings, other relations and
    /// unrelated branches stay exactly as expanded/collapsed as they were.
    ///
    /// # Errors
    ///
    /// [`UnknownNodeError`] if `node_id` has never been
    /// [`add_node`](Self::add_node)-ed.
    pub fn focus(&mut self, node_id: &str) -> Result<(), UnknownNodeError> {
        if !self.nodes.contains_key(node_id) {
            return Err(UnknownNodeError(node_id.to_string()));
        }
        self.selected = Some(node_id.to_string());
        let mut cursor = self.nodes.get(node_id).and_then(|n| n.parent.clone());
        while let Some(parent) = cursor {
            self.expanded.insert(parent.clone());
            cursor = self.nodes.get(&parent).and_then(|n| n.parent.clone());
        }
        Ok(())
    }

    /// Every node currently visible: every root (`parent.is_none()`), plus
    /// the children of any expanded node, transitively, plus the selected
    /// node (kept visible even if its ancestors happen to collapse again —
    /// matching the prototype's step 8, "collapse Terry; target paper
    /// remains independent"). Sorted by id for a deterministic, testable
    /// order — the underlying storage is a `HashMap` with no ordering
    /// guarantee of its own.
    pub fn visible_nodes(&self) -> Vec<&MapNode> {
        let mut visible: HashSet<String> = self
            .nodes
            .values()
            .filter(|n| n.parent.is_none())
            .map(|n| n.id.clone())
            .collect();

        let mut changed = true;
        while changed {
            changed = false;
            let frontier: Vec<String> = visible.iter().cloned().collect();
            for parent in frontier {
                if !self.expanded.contains(&parent) {
                    continue;
                }
                if let Some(children) = self.hierarchy.get(&parent) {
                    for child in children {
                        if visible.insert(child.clone()) {
                            changed = true;
                        }
                    }
                }
            }
        }

        if let Some(selected) = &self.selected {
            visible.insert(selected.clone());
        }

        let mut out: Vec<&MapNode> = self
            .nodes
            .values()
            .filter(|n| visible.contains(&n.id))
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Every edge whose `source` and `target` are both currently visible
    /// (per [`visible_nodes`](Self::visible_nodes)).
    pub fn visible_edges(&self) -> Vec<&MapEdge> {
        let ids: HashSet<&str> = self
            .visible_nodes()
            .into_iter()
            .map(|n| n.id.as_str())
            .collect();
        self.edges
            .iter()
            .filter(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str()))
            .collect()
    }

    /// The current zoom factor.
    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// Multiply the zoom factor by `factor`, clamped to `[0.25, 3.0]`
    /// (matching [`crate::mindmap_layout::Camera`]'s clamp). Never touches
    /// `expanded`/`selected` — see the module doc's rule 2.
    pub fn zoom_by(&mut self, factor: f64) {
        self.zoom = (self.zoom * factor).clamp(0.25, 3.0);
    }

    /// The presentation level implied by the current zoom.
    pub fn detail_level(&self) -> DetailLevel {
        if self.zoom < 0.55 {
            DetailLevel::Compact
        } else if self.zoom < 1.15 {
            DetailLevel::Normal
        } else {
            DetailLevel::Detailed
        }
    }
}

/// Map one [`crate::artifact::Artifact`]'s kind onto the [`MapNodeKind`]
/// variant its mindmap node carries. A thin, total pass-through today —
/// kept as a named function (rather than inlined at the one call site in
/// [`build_model`]) because the prototype's `_artifact_kind` had a
/// fallback branch for artifact kinds the model doesn't know about; the
/// real [`ArtifactKind`] enum is closed and exhaustively matched here, so
/// there is no fallback to reproduce, but a future new variant will make
/// this `match` (not a silent default) the compile error that points here.
fn map_node_kind_for_artifact(kind: ArtifactKind) -> MapNodeKind {
    match kind {
        ArtifactKind::Note
        | ArtifactKind::Annotation
        | ArtifactKind::SourceReference
        | ArtifactKind::Formula
        | ArtifactKind::DigitisedTable
        | ArtifactKind::DigitisedGraph => MapNodeKind::Artifact(kind),
    }
}

/// Build a fresh [`MindmapModel`] from a live library: every collection and
/// paper from `index`, every artifact of every paper (opened fresh via
/// [`PaperSession::open`] — a paper whose session fails to open
/// contributes just its paper node, matching [`crate::mindmap::literature_card`]'s
/// "no session, no artifact counts" tolerance rather than failing the whole
/// build), every canonical edge from `graph`, and every `user_edges` entry
/// as an additional [`MapEdge`] (see [`TypedEdge`]'s seam documentation).
///
/// Expansion/selection/zoom all start at the [`MindmapModel::new`] default
/// — this function only populates topology, never opens anything.
pub fn build_model(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    graph: &KnowledgeGraph,
    user_edges: &[TypedEdge],
) -> MindmapModel {
    let mut model = MindmapModel::new();

    for c in &index.collections {
        let id = graph::collection_node(&c.path);
        let parent = c
            .path
            .rsplit_once('/')
            .map(|(parent_path, _)| graph::collection_node(parent_path));
        model.add_node(MapNode {
            id,
            kind: MapNodeKind::Collection(c.kind),
            title: c.name.clone(),
            parent,
            subtitle: String::new(),
        });
    }

    for p in &index.papers {
        let paper_id = graph::paper_node(&p.citekey);
        model.add_node(MapNode {
            id: paper_id.clone(),
            kind: MapNodeKind::Paper,
            title: p.citekey.clone(),
            parent: None,
            subtitle: String::new(),
        });

        if let Ok(session) = PaperSession::open(root, &p.citekey) {
            let research = ResearchRecordIndex::from_session(&session);
            for artifact in research.artifacts() {
                let artifact_id = graph::artifact_node(&p.citekey, artifact.id());
                model.add_node(MapNode {
                    id: artifact_id,
                    kind: map_node_kind_for_artifact(artifact.kind()),
                    title: artifact.heading.clone(),
                    parent: Some(paper_id.clone()),
                    subtitle: format!("{:?}", artifact.kind()),
                });
            }
        }
    }

    for e in &graph.edges {
        model.add_edge(MapEdge {
            source: e.from.clone(),
            target: e.to.clone(),
            label: format!("{:?}", e.kind),
        });
    }

    for e in user_edges {
        model.add_edge(MapEdge {
            source: e.source.clone(),
            target: e.target.clone(),
            label: e.label.clone(),
        });
    }

    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    /// Builds a small library: two topics/papers, `terry2005` with an
    /// `Annotation` artifact and `iaea1694` with a `DigitisedGraph`
    /// artifact — enough to reproduce the prototype's own dogfood result
    /// (README: "expanding `terry2005` reveals its annotation; focusing
    /// that annotation preserves its ancestor path; ... expanding the
    /// target paper exposes the prototype `uses_data_from` edge").
    fn small_library() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();

        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();

        EntityConfig::paper(CiteKey::parse("terry2005").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("terry2005"))
            .unwrap();
        let mut terry = PaperSession::open(&root, "terry2005").unwrap();
        terry.append_block(
            "## Pebble packing note\n\n```toml\n[kovan]\nid = \"pebble-packing-note\"\nkind = \"annotation\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 4\n```\n",
        );
        terry.save_document().unwrap();

        EntityConfig::paper(CiteKey::parse("iaea1694").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("iaea1694"))
            .unwrap();
        let mut iaea = PaperSession::open(&root, "iaea1694").unwrap();
        iaea.append_block(
            "## Digitised curve\n\n```toml\n[kovan]\nid = \"digitised-curve\"\nkind = \"digitised_graph\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 9\n```\n",
        );
        iaea.save_document().unwrap();

        (dir, root)
    }

    #[test]
    fn a_fresh_model_shows_only_roots() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let model = build_model(&root, &index, &graph, &[]);

        let visible: Vec<&str> = model.visible_nodes().iter().map(|n| n.id.as_str()).collect();
        assert!(visible.contains(&"paper:terry2005"));
        assert!(visible.contains(&"paper:iaea1694"));
        assert!(visible.contains(&"collection:htgrs"));
        assert!(
            !visible.contains(&"artifact:terry2005#pebble-packing-note"),
            "artifacts must stay hidden until their paper is expanded"
        );
    }

    #[test]
    fn expanding_a_paper_reveals_its_artifact() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);

        model.toggle("paper:terry2005");
        let visible: Vec<&str> = model.visible_nodes().iter().map(|n| n.id.as_str()).collect();
        assert!(visible.contains(&"artifact:terry2005#pebble-packing-note"));

        let artifact = model
            .node("artifact:terry2005#pebble-packing-note")
            .unwrap();
        assert_eq!(
            artifact.kind,
            MapNodeKind::Artifact(ArtifactKind::Annotation)
        );
    }

    #[test]
    fn collapsing_again_hides_the_artifact_but_a_focused_selection_stays_visible() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);

        model.toggle("paper:terry2005");
        model
            .focus("artifact:terry2005#pebble-packing-note")
            .unwrap();
        model.toggle("paper:terry2005"); // collapse again

        let visible: Vec<&str> = model.visible_nodes().iter().map(|n| n.id.as_str()).collect();
        assert!(
            visible.contains(&"artifact:terry2005#pebble-packing-note"),
            "a selected node stays visible even if its ancestor collapses again"
        );
    }

    #[test]
    fn focus_expands_only_the_ancestor_path_and_selects_the_target() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);

        model
            .focus("artifact:terry2005#pebble-packing-note")
            .unwrap();

        assert_eq!(
            model.selected(),
            Some("artifact:terry2005#pebble-packing-note")
        );
        assert!(model.is_expanded("paper:terry2005"));
        // Nothing outside the ancestor path was touched.
        assert!(!model.is_expanded("paper:iaea1694"));
    }

    #[test]
    fn focus_on_an_unknown_node_is_an_error() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);

        let err = model.focus("paper:does-not-exist").unwrap_err();
        assert_eq!(err.0, "paper:does-not-exist");
    }

    #[test]
    fn semantic_zoom_changes_detail_level_but_never_visible_topology() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);
        model.toggle("paper:terry2005");

        let before: HashSet<String> = model
            .visible_nodes()
            .iter()
            .map(|n| n.id.clone())
            .collect();
        assert_eq!(model.detail_level(), DetailLevel::Normal);

        model.zoom_by(0.4); // 1.0 -> 0.4
        assert_eq!(model.detail_level(), DetailLevel::Compact);
        let after_shrink: HashSet<String> = model
            .visible_nodes()
            .iter()
            .map(|n| n.id.clone())
            .collect();
        assert_eq!(before, after_shrink);

        model.zoom_by(5.0); // 0.4 -> clamped to 2.0... actually 0.4*5=2.0
        assert_eq!(model.detail_level(), DetailLevel::Detailed);
        let after_grow: HashSet<String> = model
            .visible_nodes()
            .iter()
            .map(|n| n.id.clone())
            .collect();
        assert_eq!(before, after_grow);
    }

    #[test]
    fn zoom_is_clamped_to_a_quarter_and_three() {
        let mut model = MindmapModel::new();
        model.zoom_by(0.01);
        assert_eq!(model.zoom(), 0.25);
        model.zoom_by(1000.0);
        assert_eq!(model.zoom(), 3.0);
    }

    #[test]
    fn expanding_a_second_paper_exposes_a_user_relation_edge() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let user_edges = [TypedEdge {
            source: "artifact:terry2005#pebble-packing-note".to_string(),
            target: "artifact:iaea1694#digitised-curve".to_string(),
            label: "uses_data_from".to_string(),
        }];
        let mut model = build_model(&root, &index, &graph, &user_edges);

        model.toggle("paper:terry2005");
        // The edge is not yet visible: its target artifact's paper isn't
        // expanded yet. (Canonical Classification edges between the
        // already-visible paper/collection roots are unaffected and may
        // still be present, so this checks for the specific relation
        // rather than asserting the whole edge list is empty.)
        assert!(!model
            .visible_edges()
            .iter()
            .any(|e| e.label == "uses_data_from"));

        model.toggle("paper:iaea1694");
        let edges: Vec<&MapEdge> = model
            .visible_edges()
            .into_iter()
            .filter(|e| e.label == "uses_data_from")
            .collect();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn build_model_is_deterministic() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);

        let a = build_model(&root, &index, &graph, &[]);
        let b = build_model(&root, &index, &graph, &[]);

        let ids_a: Vec<String> = a.visible_nodes().iter().map(|n| n.id.clone()).collect();
        let ids_b: Vec<String> = b.visible_nodes().iter().map(|n| n.id.clone()).collect();
        assert_eq!(ids_a, ids_b);
    }
}
