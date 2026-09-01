//! The interactive mindmap (§8, §9, `op-9vo6.21`) — the primary home view,
//! built on the working collection model (`op-9vo6.7`/`.8`) rather than
//! before it, per §45's explicit non-goal: "fancy mindmap physics before
//! the underlying model works."
//!
//! # Deliberately not physics
//!
//! Layout is a static radial fan — the current collection's children and
//! papers placed evenly around it at a fixed radius — recomputed each
//! frame from [`KnowledgeIndex`], not a force-directed simulation. That is
//! squarely what §45 asks for at this stage: an interactive, drillable
//! map, not a physics engine.
//!
//! # Scope: what this step implements, and what it defers
//!
//! §8's right-click menu lists six actions: Open, Add subtopic, Rename,
//! Add literature, Move, Merge, Delete/Reclassify. This pass implements
//! **Open** (drill in — same as a click) and **Add subtopic** (a plain,
//! additive [`EntityConfig::topic`]/[`EntityConfig::project`] write,
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
//! GUI state (pan, zoom, selection) lives only in [`MindmapState`], in
//! memory — never in artifact TOML (§21). It is not yet persisted to
//! `.kovan/` across sessions; if that is wanted later, `.kovan/` is the
//! sanctioned location per §21, never artifact TOML.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::artifact::ArtifactKind;
use crate::entity::{EntityConfig, EntityKind};
use crate::graph::{self, EdgeKind, KnowledgeGraph};
use crate::index::KnowledgeIndex;
use crate::research_record::ResearchRecordIndex;
use crate::root::KovanRoot;
use crate::session::PaperSession;

const NODE_RADIUS: f32 = 26.0;
const RING_RADIUS: f32 = 160.0;

/// What the mindmap wants the caller to do next.
pub enum MindmapAction {
    /// A paper node was double-clicked — the caller should open its
    /// Research workspace (`op-9vo6.10`'s `PaperSession`), once that
    /// navigation exists.
    OpenPaper(String),
}

#[derive(Debug, Clone, PartialEq)]
struct ContextMenu {
    pos: Pos2,
    /// Collection path (possibly `""`, the shared root) the menu applies to.
    target: String,
    new_subtopic: Option<String>,
}

#[derive(Debug, Clone)]
enum NodeKind {
    Collection { path: String, kind: EntityKind, name: String },
    Paper { citekey: String },
}

struct LaidOutNode {
    kind: NodeKind,
    pos: Pos2,
}

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

pub struct MindmapState {
    current: String,
    pan: Vec2,
    zoom: f32,
    selected: Option<String>,
    context_menu: Option<ContextMenu>,
    message: String,
}

impl Default for MindmapState {
    fn default() -> Self {
        Self { current: String::new(), pan: Vec2::ZERO, zoom: 1.0, selected: None, context_menu: None, message: String::new() }
    }
}

impl MindmapState {
    fn layout(&self, index: &KnowledgeIndex, center: Pos2) -> Vec<LaidOutNode> {
        let children = index.children_of(&self.current);
        let papers = index.papers_in(&self.current);
        let total = (children.len() + papers.len()).max(1);
        let radius = RING_RADIUS * self.zoom;

        children
            .into_iter()
            .map(|c| NodeKind::Collection { path: c.path.clone(), kind: c.kind, name: c.name.clone() })
            .chain(papers.into_iter().map(|p| NodeKind::Paper { citekey: p.citekey.clone() }))
            .enumerate()
            .map(|(i, kind)| {
                let angle = (i as f32 / total as f32) * std::f32::consts::TAU;
                let pos = center + Vec2::new(angle.cos(), angle.sin()) * radius;
                LaidOutNode { kind, pos }
            })
            .collect()
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

        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let center = rect.center() + self.pan;

        if response.dragged() {
            self.pan += response.drag_delta();
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.zoom = (self.zoom * (1.0 + scroll * 0.001)).clamp(0.3, 3.0);
            }
        }

        let nodes = self.layout(index, center);
        let painter = ui.painter_at(rect);

        for node in &nodes {
            painter.line_segment([center, node.pos], Stroke::new(1.0, ui.visuals().weak_text_color()));
        }

        let mut opened_paper = None;
        for node in &nodes {
            let node_rect = Rect::from_center_size(node.pos, Vec2::splat(NODE_RADIUS * 2.0));
            let id = ui.id().with(node_id(&node.kind));
            let node_response = ui.interact(node_rect, id, Sense::click());

            let color = match &node.kind {
                NodeKind::Collection { kind: EntityKind::Project, .. } => Color32::from_rgb(220, 150, 60),
                NodeKind::Collection { .. } => Color32::from_rgb(90, 140, 220),
                NodeKind::Paper { .. } => Color32::from_rgb(120, 190, 120),
            };
            painter.circle_filled(node.pos, NODE_RADIUS, color);
            let label = match &node.kind {
                NodeKind::Collection { name, .. } => name.clone(),
                NodeKind::Paper { citekey } => citekey.clone(),
            };
            painter.text(node.pos, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), Color32::BLACK);

            if node_response.clicked() {
                self.selected = Some(node_id(&node.kind));
            }
            if node_response.double_clicked() {
                match &node.kind {
                    NodeKind::Collection { path, .. } => self.current = path.clone(),
                    NodeKind::Paper { citekey } => opened_paper = Some(citekey.clone()),
                }
            }
            if node_response.secondary_clicked() {
                if let Some(pos) = node_response.interact_pointer_pos() {
                    let target = match &node.kind {
                        NodeKind::Collection { path, .. } => path.clone(),
                        NodeKind::Paper { .. } => self.current.clone(),
                    };
                    self.context_menu = Some(ContextMenu { pos, target, new_subtopic: None });
                }
            }
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
                if ui.button("Open").clicked() {
                    self.current = menu.target.clone();
                    close = true;
                }
                ui.separator();
                if let Some(mut text) = menu.new_subtopic.clone() {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut text);
                        if ui.button("Create").clicked() {
                            match create_subtopic(root, index, &menu.target, &text) {
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
    fn layout_places_children_and_papers_around_the_center() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs").save(&root.topics_dir().join("htgrs")).unwrap();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
        // Papers filed under "htgrs" don't show at the shared root; a
        // top-level topic does.
        let index = KnowledgeIndex::rebuild(&root);
        let state = MindmapState::default();
        let nodes = state.layout(&index, Pos2::ZERO);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0].kind, NodeKind::Collection { path, .. } if path == "htgrs"));
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
