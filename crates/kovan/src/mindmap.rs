//! The interactive mindmap (§8, §9, `op-9vo6.21`) — the primary home view,
//! built on the working collection model (`op-9vo6.7`/`.8`) rather than
//! before it, per §45's explicit non-goal: "fancy mindmap physics before
//! the underlying model works."
//!
//! # Rendering: `egui_graphs`, not a hand-rolled painter (op-jvjc)
//!
//! §45's non-goal blocked physics only until the underlying model — paper/
//! collection classification, [`KnowledgeIndex`]/[`KnowledgeGraph`] — was
//! working and tested. It now is, so GH issue #35 checkpoint §9 (`op-jvjc`)
//! authorized moving off the original static-radial-fan painter (fixed
//! ring radius, `painter.circle_filled`/`line_segment`, per-node
//! `ui.interact(..., Sense::click())`) onto the `egui_graphs` crate — pan,
//! zoom, node-dragging, selection, hit-testing and layout are now that
//! crate's job; **KOVAN stays the sole authoritative data model.**
//! `NodeKind` is passed to `egui_graphs::Graph` as its node payload
//! unchanged — the graph widget's internal `petgraph`-backed representation
//! is never persisted and never becomes anything other than a disposable,
//! per-frame rendering aid rebuilt from [`KnowledgeIndex`] (see
//! [`MindmapState::ensure_view`]).
//!
//! Layout is [`egui_graphs::FruchtermanReingold`] (force-directed), applied
//! to a **star topology**: when [`MindmapState::current`] is a collection,
//! one node represents it, with an edge to each direct child collection and
//! each directly-classified paper — the closest real-graph equivalent of
//! the old radial fan's spokes-from-a-center-point, but now genuinely
//! physics-driven rather than a fixed angle-per-node placement. At the
//! shared root (`current == ""`) there is no such anchor node, matching the
//! old top-level behaviour where `center` was just a screen point, not a
//! modelled entity.
//!
//! # Scope: what this step implements, and what it defers
//!
//! §8's right-click menu lists six actions: Open, Add subtopic, Rename,
//! Add literature, Move, Merge, Delete/Reclassify. This pass implements
//! **Open** (drill in — now a node double-click, handled through
//! [`egui_graphs::GraphChange::NodeDoubleClicked`]) and **Add subtopic** (a
//! plain, additive [`EntityConfig::topic`]/[`EntityConfig::project`] write,
//! already exhaustively tested by `op-9vo6.6`). **Rename**, **Move**,
//! **Merge** and **Delete/Reclassify** are not implemented and do not
//! appear in the menu — §40 requires those to be transactional and to
//! "never silently orphan or delete knowledge," which for Move/Merge/
//! Delete means rewriting every affected paper's classification and every
//! `[[...]]` reference atomically. That is real, separate work, not
//! something to rush through here; a menu item that does nothing (or
//! something unsafe) is worse than a menu item that doesn't exist yet.
//! "Add literature" is `op-9vo6.9`'s ingestion flow, already reachable
//! from the Wiki view — this menu does not duplicate it.
//!
//! **Add-subtopic trigger changed with the `egui_graphs` swap.**
//! [`egui_graphs::GraphChange`] (checked directly against its 0.32.0
//! source, not assumed) has no secondary-click/right-click variant at all —
//! only click, double-click, selection, drag and hover. The old per-node
//! "right-click a node to add a subtopic under it" gesture has no
//! equivalent through that event stream. The fallback used here is a
//! **whole-canvas** right-click, read off the raw
//! [`egui_graphs::GraphViewResponse::response`] (the ordinary
//! `egui::Response` for the widget's whole area, still available
//! underneath), which always targets [`MindmapState::current`] — the
//! collection currently drilled into — rather than whichever node happens
//! to be under the pointer. This is a disclosed, deliberate scope
//! reduction: the feature (add a subtopic under the current collection)
//! is preserved; the trigger widens from "any node" to "the canvas," and
//! the old "Open" menu item (which only ever duplicated a double-click) is
//! dropped since it is exactly what the fallback's `target` already made a
//! no-op.
//!
//! GUI state (selection, the context menu) lives only in [`MindmapState`],
//! in memory — never in artifact TOML (§21). It is not yet persisted to
//! `.kovan/` across sessions; if that is wanted later, `.kovan/` is the
//! sanctioned location per §21, never artifact TOML.
//!
//! # Android/Termux portability
//!
//! Everything in this file that touches `eframe`/`egui`/`egui_graphs` is
//! gated behind `#[cfg(feature = "gui")]`, matching the pattern
//! `crate::digitiser::mod::gui` already uses. [`LiteratureCard`],
//! [`literature_card`], `bib_display`, `extract_summary` and
//! `create_subtopic` have no GUI dependency and stay unconditional, so a
//! headless (Android/Termux, `kovan-cli`/`kovan-tui`) build of this crate
//! can still use them.

use crate::artifact::ArtifactKind;
use crate::entity::{EntityConfig, EntityKind};
use crate::graph::{self, EdgeKind, KnowledgeGraph};
use crate::index::KnowledgeIndex;
use crate::research_record::ResearchRecordIndex;
use crate::root::KovanRoot;
use crate::session::PaperSession;

#[cfg(feature = "gui")]
use eframe::egui;

/// What the mindmap wants the caller to do next.
#[cfg(feature = "gui")]
pub enum MindmapAction {
    /// A paper node was double-clicked — the caller should open its
    /// Research workspace (`op-9vo6.10`'s `PaperSession`), once that
    /// navigation exists.
    OpenPaper(String),
}

#[cfg(feature = "gui")]
#[derive(Debug, Clone, PartialEq)]
struct ContextMenu {
    pos: egui::Pos2,
    /// `None` until "Add subtopic…" is clicked; then the text field's
    /// current contents. Always applies to [`MindmapState::current`] — see
    /// the module doc's "Add-subtopic trigger changed" section for why
    /// there is no longer a separate per-node target.
    new_subtopic: Option<String>,
}

#[cfg(feature = "gui")]
#[derive(Debug, Clone)]
enum NodeKind {
    /// `name` is not kept here — it only ever fed the node's display label,
    /// which `egui_graphs::Graph::add_node_with_label` already owns once
    /// the node is built (see [`Node::label`](egui_graphs::Node::label)).
    Collection { path: String, kind: EntityKind },
    Paper { citekey: String },
}

#[cfg(feature = "gui")]
fn node_id(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Collection { path, .. } => graph::collection_node(path),
        NodeKind::Paper { citekey } => graph::paper_node(citekey),
    }
}

/// One paper's mindmap/literature card (§9). Author/year is a **display
/// label formatted from the BibTeX entry** — never the paper's `id`, which
/// stays the citekey (§7's amendment; §9 restates this explicitly).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiteratureCard {
    pub title_or_citekey: String,
    pub author_year: String,
    pub topics: Vec<String>,
    pub projects: Vec<String>,
    pub note_count: usize,
    pub formula_count: usize,
    pub table_count: usize,
    pub graph_count: usize,
    pub citation_count: usize,
    pub backlink_count: usize,
    /// The researcher-written `## Summary` section, if any. Never a
    /// publisher abstract — §9: abstract prose may be copyright-protected,
    /// so this reads only what a human wrote, never anything auto-committed
    /// from the source document.
    pub summary: String,
}

/// Build a paper's literature card from its index entry, bibliography
/// record, and its own Markdown (for artifact counts and its `## Summary`).
pub fn literature_card(root: &KovanRoot, index: &KnowledgeIndex, graph: &KnowledgeGraph, citekey: &str) -> LiteratureCard {
    let mut card = LiteratureCard::default();
    if let Some(paper) = index.papers.iter().find(|p| p.citekey == citekey) {
        card.topics = paper.topics.clone();
        card.projects = paper.projects.clone();
    }

    let (title, author_year) = bib_display(root, citekey);
    card.title_or_citekey = title;
    card.author_year = author_year;

    if let Ok(session) = PaperSession::open(root, citekey) {
        let research = ResearchRecordIndex::from_session(&session);
        for a in research.artifacts() {
            match a.kind() {
                ArtifactKind::Note | ArtifactKind::Annotation | ArtifactKind::SourceReference => card.note_count += 1,
                ArtifactKind::Formula => card.formula_count += 1,
                ArtifactKind::DigitisedTable => card.table_count += 1,
                ArtifactKind::DigitisedGraph => card.graph_count += 1,
            }
        }
        card.summary = extract_summary(session.markdown());
    }

    let node = graph::paper_node(citekey);
    card.citation_count = graph.outlinks(&node).iter().filter(|e| e.kind == EdgeKind::Cites).count();
    card.backlink_count = graph.backlinks(&node).len();
    card
}

/// `(title, "Family Year")`, both derived from the BibTeX entry — falls
/// back to the bare citekey when there is no bibliography entry yet (a
/// paper catalogued from metadata alone).
fn bib_display(root: &KovanRoot, citekey: &str) -> (String, String) {
    let fallback = (citekey.to_string(), String::new());
    let Ok(text) = std::fs::read_to_string(root.bibliography_path()) else { return fallback };
    let Ok(entries) = kovan_literature::parse_bib_entries(&text) else { return fallback };
    let Some(entry) = entries.into_iter().find(|e| e.cite_key == citekey) else { return fallback };

    let title = entry.fields.get("title").cloned().unwrap_or_else(|| citekey.to_string());
    let author = entry.fields.get("author").cloned().unwrap_or_default();
    let year = entry.fields.get("year").cloned().unwrap_or_default();
    // BibTeX name order is "Family, Given and Family, Given ..."; the
    // display label wants only the first author's family name.
    let family = author.split(" and ").next().unwrap_or("").split(',').next().unwrap_or("").trim().to_string();
    let author_year = [family, year].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    (title, author_year)
}

/// The prose under a `## Summary` heading, up to the next heading (or end
/// of document). Empty if there is no such heading.
fn extract_summary(markdown: &str) -> String {
    let Some(start) = markdown.find("## Summary") else { return String::new() };
    let after = &markdown[start + "## Summary".len()..];
    let end = after.find("\n#").unwrap_or(after.len());
    after[..end].trim().to_string()
}

/// Add a subtopic/subproject under `parent_path` (or a top-level topic when
/// `parent_path` is `""`) — matches the parent's own kind (a project's
/// child is a project, a topic's a topic), defaulting to a topic at the
/// shared root, where kind is not yet established.
///
/// Unconditional/GUI-free by design (see the module doc's "Android/Termux
/// portability" section) even though only [`MindmapState`]'s `gui`-gated
/// context menu calls it today — a future headless consumer (e.g. a
/// `kovan-cli` mindmap subcommand) can reuse it without pulling in `eframe`.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
fn create_subtopic(root: &KovanRoot, index: &KnowledgeIndex, parent_path: &str, name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name must not be empty".to_string());
    }
    let slug = crate::classify::slugify(name);
    if slug.is_empty() {
        return Err("name has no usable characters for an id".to_string());
    }
    let parent_kind = if parent_path.is_empty() {
        EntityKind::Topic
    } else {
        index.collections.iter().find(|c| c.path == parent_path).map(|c| c.kind).unwrap_or(EntityKind::Topic)
    };
    let tree_root = match parent_kind {
        EntityKind::Project => root.projects_dir(),
        _ => root.topics_dir(),
    };
    let dir = if parent_path.is_empty() { tree_root.join(&slug) } else { tree_root.join(parent_path).join(&slug) };
    let config = match parent_kind {
        EntityKind::Project => EntityConfig::project(slug, name),
        _ => EntityConfig::topic(slug, name),
    };
    config.save(&dir).map_err(|e| e.to_string())
}

#[cfg(feature = "gui")]
type MindmapGraph = egui_graphs::Graph<NodeKind>;
#[cfg(feature = "gui")]
type MindmapLayoutState = egui_graphs::FruchtermanReingoldState;
// `FruchtermanReingold` itself only implements `ForceAlgorithm` (the force
// computation); `egui_graphs::Layout` is implemented for the generic
// `LayoutForceDirected<A: ForceAlgorithm>` wrapper around it, not for
// `FruchtermanReingold` directly (confirmed against 0.32.0's
// `layouts/force_directed/{algorithm,layout}.rs`).
#[cfg(feature = "gui")]
type MindmapLayout = egui_graphs::LayoutForceDirected<egui_graphs::FruchtermanReingold>;

#[cfg(feature = "gui")]
pub struct MindmapState {
    current: String,
    selected: Option<String>,
    context_menu: Option<ContextMenu>,
    message: String,
    /// Cache of `(scope, index snapshot used to build it, the built
    /// egui_graphs graph)`, rebuilt only in [`Self::ensure_view`] when
    /// `current` or `index` has actually changed. Rebuilding fresh every
    /// frame would defeat `FruchtermanReingold`'s force-directed
    /// convergence and discard any node the user just dragged.
    view: Option<(String, KnowledgeIndex, MindmapGraph)>,
}

#[cfg(feature = "gui")]
impl Default for MindmapState {
    fn default() -> Self {
        Self { current: String::new(), selected: None, context_menu: None, message: String::new(), view: None }
    }
}

#[cfg(feature = "gui")]
impl MindmapState {
    /// Build the star-topology graph for `current`'s scope: one anchor node
    /// for `current` itself (when non-empty), with an edge to each direct
    /// child collection and each directly-classified paper. At the shared
    /// root (`current == ""`) there is no anchor node — direct children and
    /// papers stand alone, same as the old top-level behaviour.
    fn build_graph(index: &KnowledgeIndex, current: &str) -> MindmapGraph {
        let mut g: MindmapGraph = egui_graphs::Graph::new();

        let push = |g: &mut MindmapGraph, node: NodeKind, label: String| {
            let color = Self::color_for(&node);
            let idx = g.add_node_with_label(node, label);
            if let Some(n) = g.node_mut(idx) {
                n.set_color(color);
            }
            idx
        };

        let anchor = if current.is_empty() {
            None
        } else {
            let (kind, name) = index
                .collections
                .iter()
                .find(|c| c.path == current)
                .map(|c| (c.kind, c.name.clone()))
                .unwrap_or((EntityKind::Topic, current.to_string()));
            let node = NodeKind::Collection { path: current.to_string(), kind };
            Some(push(&mut g, node, name))
        };

        for child in index.children_of(current) {
            let node = NodeKind::Collection { path: child.path.clone(), kind: child.kind };
            let idx = push(&mut g, node, child.name.clone());
            if let Some(a) = anchor {
                g.add_edge(a, idx, ());
            }
        }
        for paper in index.papers_in(current) {
            let node = NodeKind::Paper { citekey: paper.citekey.clone() };
            let idx = push(&mut g, node, paper.citekey.clone());
            if let Some(a) = anchor {
                g.add_edge(a, idx, ());
            }
        }

        g
    }

    /// Node colour by kind — Project (orange), Collection/Topic (blue),
    /// Paper (green) — the same palette the old hand-painted renderer used.
    fn color_for(kind: &NodeKind) -> egui::Color32 {
        match kind {
            NodeKind::Collection { kind: EntityKind::Project, .. } => egui::Color32::from_rgb(220, 150, 60),
            NodeKind::Collection { .. } => egui::Color32::from_rgb(90, 140, 220),
            NodeKind::Paper { .. } => egui::Color32::from_rgb(120, 190, 120),
        }
    }

    /// Rebuild [`Self::view`] iff `current` or `index` no longer match what
    /// it was last built from.
    fn ensure_view(&mut self, index: &KnowledgeIndex) {
        let stale = match &self.view {
            Some((current, cached_index, _)) => current != &self.current || cached_index != index,
            None => true,
        };
        if stale {
            self.view = Some((self.current.clone(), index.clone(), Self::build_graph(index, &self.current)));
        }
    }

    /// Draw the mindmap and process this frame's interaction. Returns
    /// `Some` when the caller should navigate to a paper's Research
    /// workspace.
    pub fn ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot, index: &KnowledgeIndex, graph: &KnowledgeGraph) -> Option<MindmapAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            if ui.link("Wiki").clicked() {
                self.current.clear();
            }
            let mut acc = String::new();
            for part in self.current.clone().split('/').filter(|s| !s.is_empty()) {
                ui.label(">");
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(part);
                if ui.link(part).clicked() {
                    self.current = acc.clone();
                }
            }
            if !self.message.is_empty() {
                ui.weak(&self.message);
            }
        });

        self.ensure_view(index);

        let mut drilled = None;
        let mut opened_paper = None;
        {
            let (_, _, egui_graph) = self.view.as_mut().expect("ensure_view just populated this");

            let settings_interaction =
                egui_graphs::SettingsInteraction::new().with_dragging_enabled(true).with_node_selection_enabled(true);
            let settings_navigation =
                egui_graphs::SettingsNavigation::new().with_fit_to_screen_enabled(false).with_zoom_and_pan_enabled(true);
            let settings_style = egui_graphs::SettingsStyle::new().with_labels_always(true);

            let response = egui_graphs::GraphView::<MindmapLayoutState, MindmapLayout>::new()
                .with_interactions(&settings_interaction)
                .with_navigations(&settings_navigation)
                .with_styles(&settings_style)
                .with_id(Some(format!("mindmap::{}", self.current)))
                .show(ui, egui_graph);

            for change in &response.changes {
                match change {
                    egui_graphs::GraphChange::NodeSelected { node } => {
                        if let Some(payload) = egui_graph.node(*node).map(|n| n.payload().clone()) {
                            self.selected = Some(node_id(&payload));
                        }
                    }
                    egui_graphs::GraphChange::NodeDeselected { node } => {
                        if let Some(payload) = egui_graph.node(*node).map(|n| n.payload().clone()) {
                            if self.selected.as_deref() == Some(node_id(&payload).as_str()) {
                                self.selected = None;
                            }
                        }
                    }
                    egui_graphs::GraphChange::NodeDoubleClicked { node } => {
                        if let Some(payload) = egui_graph.node(*node).map(|n| n.payload().clone()) {
                            match payload {
                                NodeKind::Collection { path, .. } => drilled = Some(path),
                                NodeKind::Paper { citekey } => opened_paper = Some(citekey),
                            }
                        }
                    }
                    _ => {}
                }
            }

            if response.response.secondary_clicked() {
                if let Some(pos) = response.response.interact_pointer_pos() {
                    self.context_menu = Some(ContextMenu { pos, new_subtopic: None });
                }
            }
        }

        if let Some(path) = drilled {
            self.current = path;
        }
        if let Some(citekey) = opened_paper {
            action = Some(MindmapAction::OpenPaper(citekey));
        }

        self.context_menu_ui(ui, root, index);
        self.literature_card_ui(ui, root, index, graph);

        action
    }

    fn context_menu_ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot, index: &KnowledgeIndex) {
        let Some(menu) = self.context_menu.clone() else { return };
        let mut close = false;
        egui::Area::new(ui.id().with("mindmap-context-menu")).fixed_pos(menu.pos).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                if let Some(mut text) = menu.new_subtopic.clone() {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut text);
                        if ui.button("Create").clicked() {
                            match create_subtopic(root, index, &self.current, &text) {
                                Ok(()) => self.message = format!("added {text:?}"),
                                Err(e) => self.message = format!("could not add subtopic: {e}"),
                            }
                            close = true;
                        }
                    });
                    if !close {
                        if let Some(m) = self.context_menu.as_mut() {
                            m.new_subtopic = Some(text);
                        }
                    }
                } else if ui.button("Add subtopic…").clicked() {
                    if let Some(m) = self.context_menu.as_mut() {
                        m.new_subtopic = Some(String::new());
                    }
                }
                ui.separator();
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if close {
            self.context_menu = None;
        }
    }

    fn literature_card_ui(&self, ui: &mut egui::Ui, root: &KovanRoot, index: &KnowledgeIndex, graph: &KnowledgeGraph) {
        let Some(selected) = &self.selected else { return };
        let Some(citekey) = selected.strip_prefix("paper:") else { return };

        egui::Window::new("Literature card").id(ui.id().with("literature-card")).collapsible(false).show(ui.ctx(), |ui| {
            let card = literature_card(root, index, graph, citekey);
            ui.strong(&card.title_or_citekey);
            if !card.author_year.is_empty() {
                ui.label(&card.author_year);
            }
            if !card.topics.is_empty() || !card.projects.is_empty() {
                ui.label(format!("Topics: {}  Projects: {}", card.topics.join(", "), card.projects.join(", ")));
            }
            ui.label(format!(
                "{} notes, {} formulas, {} tables, {} graphs",
                card.note_count, card.formula_count, card.table_count, card.graph_count
            ));
            ui.label(format!("{} citations, {} backlinks", card.citation_count, card.backlink_count));
            if !card.summary.is_empty() {
                ui.separator();
                ui.label(&card.summary);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey};
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    #[test]
    #[cfg(feature = "gui")]
    fn build_graph_places_children_and_papers_around_the_anchor() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs").save(&root.topics_dir().join("htgrs")).unwrap();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
        // Papers filed under "htgrs" don't show at the shared root; a
        // top-level topic does. At the shared root there is no anchor node.
        let index = KnowledgeIndex::rebuild(&root);
        let top = MindmapState::build_graph(&index, "");
        assert_eq!(top.node_count(), 1);
        assert_eq!(top.edge_count(), 0);
        assert!(matches!(&top.node(top.nodes_iter().next().unwrap().0).unwrap().payload(), NodeKind::Collection { path, .. } if path == "htgrs"));

        // Drilled into "htgrs": one anchor node plus the paper, joined by
        // an edge.
        let drilled = MindmapState::build_graph(&index, "htgrs");
        assert_eq!(drilled.node_count(), 2);
        assert_eq!(drilled.edge_count(), 1);
    }

    #[test]
    fn extract_summary_reads_up_to_the_next_heading() {
        let md = "# Title\n\n## Summary\n\nThis is the summary.\nStill the summary.\n\n## Notes\n\nNot the summary.\n";
        assert_eq!(extract_summary(md), "This is the summary.\nStill the summary.");
        assert_eq!(extract_summary("# no summary heading here\n"), "");
    }

    #[test]
    fn bib_display_formats_family_name_and_year() {
        let (_dir, root) = make_root();
        std::fs::write(
            root.bibliography_path(),
            "@article{wang2018multiphysics,\n  author = {Wang, Yan and Lee, Kim},\n  title = {A Study},\n  year = {2018},\n}\n",
        )
        .unwrap();
        let (title, author_year) = bib_display(&root, "wang2018multiphysics");
        assert_eq!(title, "A Study");
        assert_eq!(author_year, "Wang 2018");
    }

    #[test]
    fn bib_display_falls_back_to_the_citekey_with_no_bibliography_entry() {
        let (_dir, root) = make_root();
        let (title, author_year) = bib_display(&root, "unknownkey");
        assert_eq!(title, "unknownkey");
        assert_eq!(author_year, "");
    }

    #[test]
    fn literature_card_counts_artifacts_and_backlinks() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs").save(&root.topics_dir().join("htgrs")).unwrap();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
        let mut session = PaperSession::open(&root, "wang2018multiphysics").unwrap();
        session.append_block(
            "## A table\n\n```toml\n[kovan]\nid = \"t1\"\nkind = \"digitised_table\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 1\n```\n",
        );
        session.append_block("## Summary\n\nA hand-written summary.\n");
        session.save_document().unwrap();

        // A second paper that cites the first — the actual source of a
        // backlink (a paper's own classification is an OUTLINK from it,
        // never a backlink to itself).
        EntityConfig::paper(CiteKey::parse("lee2020corrosion").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("lee2020corrosion"))
            .unwrap();
        let mut citer = PaperSession::open(&root, "lee2020corrosion").unwrap();
        citer.append_block("## Notes\n\nBuilds on [@wang2018multiphysics].\n");
        citer.save_document().unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let card = literature_card(&root, &index, &graph, "wang2018multiphysics");

        assert_eq!(card.table_count, 1);
        assert_eq!(card.topics, vec!["htgrs".to_string()]);
        assert_eq!(card.backlink_count, 1, "lee2020corrosion's [@wang2018multiphysics] citation is a backlink to this paper");
    }

    #[test]
    fn create_subtopic_matches_the_parents_kind() {
        let (_dir, root) = make_root();
        EntityConfig::project("outram-park", "Outram Park").save(&root.projects_dir().join("outram-park")).unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        create_subtopic(&root, &index, "outram-park", "Sub Effort").unwrap();
        assert!(EntityConfig::is_entity(&root.projects_dir().join("outram-park").join("sub-effort")));

        create_subtopic(&root, &index, "", "New Topic").unwrap();
        assert!(EntityConfig::is_entity(&root.topics_dir().join("new-topic")));
    }

    #[test]
    fn create_subtopic_rejects_an_empty_name() {
        let (_dir, root) = make_root();
        let index = KnowledgeIndex::rebuild(&root);
        assert!(create_subtopic(&root, &index, "", "   ").is_err());
    }
}
