//! The interactive mindmap (§8, §9, `op-9vo6.21`) — the primary home view,
//! built on the working collection model (`op-9vo6.7`/`.8`) rather than
//! before it, per §45's explicit non-goal: "fancy mindmap physics before
//! the underlying model works."
//!
//! # Rendering: our own scroll-area canvas (GitHub issue #243)
//!
//! ~~Rendering: `egui_graphs`, not a hand-rolled painter (op-jvjc).~~
//! **CHANGED 2026-09-22 (maintainer direction, epic #241).** From op-jvjc
//! until then, pan, zoom, dragging, selection and a force-directed
//! (`FruchtermanReingold`) layout were the `egui_graphs` crate's job. It was
//! removed because every node is to become a widget card (#244) with its own
//! menus, which a graph widget that owns the drawing and reports no
//! right-clicks cannot host, and because the maintainer asked for the same
//! pan/zoom controls as `htgr_sim_v1`'s plant view.
//!
//! The page now draws the drill-in star itself: the current concept at the
//! centre and its sub-concepts on a ring around it, placed by
//! [`crate::mindmap_view::star_layout`] so no two cards overlap and nothing
//! drifts. ~~Directly classified papers were ring cards too~~ — since #244
//! papers are not cards: each concept card shows a citation count, its
//! citations drop down on hover, and its right-click menu opens them (#245).
//! A ▸ on a ring card fans its own sub-concepts outward without moving the
//! centre, and dragging any card but the centre pins it there for that star,
//! until "Unpin" or "Unpin all" (#246). It sits on a two-axis `ScrollArea`
//! (drag or scroll to pan) whose canvas stops 25 % of the map's own size past
//! each edge, plus half a viewport width sideways
//! ([`crate::mindmap_view::CanvasLayout`]); −, +, Fit, 100 % and
//! Re-centre buttons and Ctrl + scroll set the zoom, which redraws the cards
//! at scale (text included) and keeps the middle of the view fixed. Moving to
//! another concept re-centres. **KOVAN stays the sole data model:** the star
//! is rebuilt from [`KnowledgeIndex`] every frame and never persisted.
//!
//! Dragging a card went with `egui_graphs` and came back as drag-to-pin in
//! #246, the same day.
//!
//! # Scope: what this step implements, and what it defers
//!
//! §8's right-click menu lists six actions: Open, Add subtopic, Rename,
//! Add literature, Move, Merge, Delete/Reclassify. This pass implements
//! **Open** (drill in — now a node double-click, handled through
//! `egui_graphs::GraphChange::NodeDoubleClicked`) and **Add subtopic** (a
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
//! **Add-subtopic trigger.** Since #245 every card has its own right-click
//! menu, whose "Add subtopic here…" goes under **that** card's concept; a
//! right-click on the empty canvas adds under [`MindmapState::current`]. The
//! history below is kept as written:
//!
//! ~~**Add-subtopic trigger changed with the `egui_graphs` swap.**~~
//! `egui_graphs::GraphChange` (checked directly against its 0.32.0
//! source, not assumed) has no secondary-click/right-click variant at all —
//! only click, double-click, selection, drag and hover. The old per-node
//! "right-click a node to add a subtopic under it" gesture has no
//! equivalent through that event stream. The fallback used here is a
//! **whole-canvas** right-click, read off the raw
//! `egui_graphs::GraphViewResponse::response` (the ordinary
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
//! Everything in this file that touches `eframe`/`egui` is
//! gated behind `#[cfg(all(feature = "gui", not(target_os = "android")))]`, matching the pattern
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

#[cfg(all(feature = "gui", not(target_os = "android")))]
use eframe::egui;

/// What the mindmap wants the caller to do next.
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub enum MindmapAction {
    /// A paper node was double-clicked — the caller should open its
    /// Research workspace (`op-9vo6.10`'s `PaperSession`), once that
    /// navigation exists.
    OpenPaper(String),
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
pub fn literature_card(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    graph: &KnowledgeGraph,
    citekey: &str,
) -> LiteratureCard {
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
                // The paper header is the card itself, not one of its
                // artifacts — counting it would double-count the paper.
                // A connector is an edge, and a saved mindmap is a view of
                // the graph — neither is a note/table/figure of the paper.
                ArtifactKind::Paper | ArtifactKind::Relation | ArtifactKind::Mindmap => {}
                ArtifactKind::Note | ArtifactKind::Annotation | ArtifactKind::SourceReference => {
                    card.note_count += 1
                }
                ArtifactKind::Formula => card.formula_count += 1,
                ArtifactKind::DigitisedTable => card.table_count += 1,
                ArtifactKind::DigitisedGraph => card.graph_count += 1,
            }
        }
        card.summary = extract_summary(session.markdown());
    }

    let node = graph::paper_node(citekey);
    card.citation_count = graph
        .outlinks(&node)
        .iter()
        .filter(|e| e.kind == EdgeKind::Cites)
        .count();
    card.backlink_count = graph.backlinks(&node).len();
    card
}

/// `(title, "Family Year")`, both derived from the BibTeX entry — falls
/// back to the bare citekey when there is no bibliography entry yet (a
/// paper catalogued from metadata alone).
///
/// `pub(crate)` rather than private: reused by [`crate::app`]'s digitiser
/// panels to auto-fill a crop's document title from the active paper
/// (`op-u1m9`) instead of asking the user to retype what the library
/// already knows.
pub(crate) fn bib_display(root: &KovanRoot, citekey: &str) -> (String, String) {
    let fallback = (citekey.to_string(), String::new());
    let Ok(text) = std::fs::read_to_string(root.bibliography_path()) else {
        return fallback;
    };
    let Ok(entries) = kovan_literature::parse_bib_entries(&text) else {
        return fallback;
    };
    let Some(entry) = entries.into_iter().find(|e| e.cite_key == citekey) else {
        return fallback;
    };
    bib_display_from_fields(citekey, &entry.fields)
}

/// [`bib_display`]'s formatting of one entry's fields, shared with
/// [`BibCache`] so both produce the same labels.
fn bib_display_from_fields(
    citekey: &str,
    fields: &std::collections::BTreeMap<String, String>,
) -> (String, String) {
    let title = fields
        .get("title")
        .cloned()
        .unwrap_or_else(|| citekey.to_string());
    let author = fields.get("author").cloned().unwrap_or_default();
    let year = fields.get("year").cloned().unwrap_or_default();
    // BibTeX name order is "Family, Given and Family, Given ..."; the
    // display label wants only the first author's family name.
    let family = author
        .split(" and ")
        .next()
        .unwrap_or("")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let author_year = [family, year]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (title, author_year)
}

/// One citation under a concept, as the hover list and right-click menu show
/// it (GitHub issue #245): `"Family Year"` and the title from the BibTeX
/// entry, falling back to the citekey, exactly as [`bib_display`] does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The citekey (library) or corpus id (corpus).
    pub citekey: String,
    /// The title, or the citekey when there is no BibTeX entry.
    pub title: String,
    /// `"Family Year"` of the first author, or empty.
    pub author_year: String,
    /// Where the entry lives: the user's library (openable today) or the
    /// built-in corpus (opened through the source resolver, #253).
    pub namespace: crate::node_id::Namespace,
}

impl Citation {
    /// `"Family Year — Title"`, or the title alone when there is no author
    /// or year.
    pub fn label(&self) -> String {
        if self.author_year.is_empty() {
            self.title.clone()
        } else {
            format!("{} — {}", self.author_year, self.title)
        }
    }
}

/// The library's BibTeX entries as `citekey -> (title, "Family Year")`,
/// re-read only when `references.bib` changes on disk.
///
/// [`bib_display`] re-reads and re-parses the whole file per paper, which is
/// fine for one label but not for a hover list redrawn every frame. This
/// caches the parse against the file's modification time and length; the formatting is
/// [`bib_display`]'s, so the two cannot disagree.
#[derive(Debug, Default)]
pub struct BibCache {
    /// Modification time and length of the file last parsed. Both, so an
    /// edit is seen even on a filesystem with coarse timestamps.
    stamp: Option<(std::time::SystemTime, u64)>,
    entries: std::collections::HashMap<String, (String, String)>,
}

impl BibCache {
    /// Refresh from `root`'s bibliography if it changed, then return the map.
    pub fn entries(
        &mut self,
        root: &KovanRoot,
    ) -> &std::collections::HashMap<String, (String, String)> {
        let path = root.bibliography_path();
        let stamp = std::fs::metadata(&path)
            .and_then(|m| Ok((m.modified()?, m.len())))
            .ok();
        if stamp.is_none() || stamp != self.stamp {
            self.stamp = stamp;
            self.entries.clear();
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(parsed) = kovan_literature::parse_bib_entries(&text) {
                    for e in parsed {
                        let (title, author_year) = bib_display_from_fields(&e.cite_key, &e.fields);
                        self.entries
                            .insert(e.cite_key.clone(), (title, author_year));
                    }
                }
            }
        }
        &self.entries
    }
}

/// The papers classified directly under the concept at `path`, as
/// citations, sorted by author/year then citekey. `entries` comes from
/// [`BibCache::entries`].
pub fn concept_citations(
    index: &KnowledgeIndex,
    entries: &std::collections::HashMap<String, (String, String)>,
    path: &str,
) -> Vec<Citation> {
    let mut out: Vec<Citation> = index
        .papers_in(path)
        .into_iter()
        .map(|p| {
            let (title, author_year) = entries
                .get(&p.citekey)
                .cloned()
                .unwrap_or_else(|| (p.citekey.clone(), String::new()));
            Citation {
                citekey: p.citekey.clone(),
                title,
                author_year,
                namespace: crate::node_id::Namespace::Library,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.author_year, &a.citekey).cmp(&(&b.author_year, &b.citekey)));
    out
}

/// The prose under a `## Summary` heading, up to the next heading (or end
/// of document). Empty if there is no such heading.
fn extract_summary(markdown: &str) -> String {
    let Some(start) = markdown.find("## Summary") else {
        return String::new();
    };
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
#[cfg_attr(
    not(all(feature = "gui", not(target_os = "android"))),
    allow(dead_code)
)]
fn create_subtopic(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    parent_path: &str,
    name: &str,
) -> Result<(), String> {
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
        index
            .collections
            .iter()
            .find(|c| c.path == parent_path)
            .map(|c| c.kind)
            .unwrap_or(EntityKind::Topic)
    };
    let tree_root = match parent_kind {
        EntityKind::Project => root.projects_dir(),
        _ => root.topics_dir(),
    };
    let dir = if parent_path.is_empty() {
        tree_root.join(&slug)
    } else {
        tree_root.join(parent_path).join(&slug)
    };
    let config = match parent_kind {
        EntityKind::Project => EntityConfig::project(slug, name),
        _ => EntityConfig::topic(slug, name),
    };
    config.save(&dir).map_err(|e| e.to_string())
}

/// The "Add subtopic" entry of a right-click menu: a button that opens a
/// name field for a subtopic under `parent` (shown as `parent_label`), and a
/// Create button that requests it. `parent` is a library concept path (`""`
/// for the top of the user's library). The half-typed name lives in
/// `draft`, which the page keeps between frames.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn subtopic_menu_item(
    ui: &mut egui::Ui,
    parent: &str,
    parent_label: &str,
    draft: &mut Option<(String, String)>,
    request: &mut Option<(String, String)>,
) {
    match draft {
        Some((target, text)) if target == parent => {
            ui.label(format!("New subtopic under {parent_label}:"));
            ui.horizontal(|ui| {
                ui.text_edit_singleline(text);
                if ui.button("Create").clicked() {
                    *request = Some((parent.to_string(), text.clone()));
                    ui.close();
                }
            });
            if request.is_some() {
                *draft = None;
            }
        }
        _ => {
            if ui.button("Add subtopic here…").clicked() {
                *draft = Some((parent.to_string(), String::new()));
            }
        }
    }
}

/// One concept card: the concept from the runtime graph (corpus or library,
/// #249) and the papers it cites (GitHub issues #244, #245). Papers are not
/// cards; they are a concept's citations.
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Debug, Clone)]
struct StarCard {
    concept: crate::runtime_graph::RuntimeConcept,
    citations: Vec<Citation>,
}

/// Where a card sits in the star, for drawing and for which controls it has.
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardRole {
    /// The concept you are on, at the centre. Not draggable.
    Centre,
    /// Ring card `i`, a direct sub-concept; can be expanded in place.
    Ring(usize),
    /// Sub-concept `k` of ring card `i`, shown because `i` is expanded.
    Fan(usize, usize),
}

/// What a click in a citation list asked for.
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub(crate) enum CitationPick {
    /// Open the paper (a back/forward step).
    Open(String),
    /// The view's second action on a paper: the literature card on the
    /// Mindmap, reclassify on the Wiki.
    Secondary(String),
}

/// The read-only citation list shown when hovering a concept: at most
/// [`HOVER_CITATION_LIMIT`] entries, then a count of the rest (the
/// right-click menu lists them all).
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub(crate) fn citations_hover(ui: &mut egui::Ui, title: &str, citations: &[Citation]) {
    ui.strong(title);
    if citations.is_empty() {
        ui.weak("no citations");
        return;
    }
    ui.weak(format!(
        "{} citation(s) — right-click to open",
        citations.len()
    ));
    for c in citations.iter().take(HOVER_CITATION_LIMIT) {
        ui.label(format!("\u{1F4C4} {}", c.label()));
    }
    if citations.len() > HOVER_CITATION_LIMIT {
        ui.weak(format!(
            "… and {} more",
            citations.len() - HOVER_CITATION_LIMIT
        ));
    }
}

/// Most citations the hover list shows before summarising the rest. A UX
/// choice: a tooltip taller than this stops being glanceable.
#[cfg(all(feature = "gui", not(target_os = "android")))]
const HOVER_CITATION_LIMIT: usize = 12;

/// The actionable citation list in a concept's right-click menu: every
/// citation as a sub-menu with "Open" and `secondary`. Corpus citations are
/// listed but not yet actionable: they open through the source resolver
/// (#253).
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub(crate) fn citations_menu(
    ui: &mut egui::Ui,
    citations: &[Citation],
    secondary: &str,
) -> Option<CitationPick> {
    let mut pick = None;
    if citations.is_empty() {
        ui.weak("no citations");
        return None;
    }
    ui.weak(format!("Citations ({})", citations.len()));
    egui::ScrollArea::vertical()
        .id_salt("citations-menu")
        .max_height(320.0)
        .show(ui, |ui| {
            for c in citations {
                let actionable = c.namespace == crate::node_id::Namespace::Library;
                ui.menu_button(format!("\u{1F4C4} {}", c.label()), |ui| {
                    let not_yet = "Built-in corpus sources open with the source resolver (#253)";
                    if ui
                        .add_enabled(actionable, egui::Button::new("Open"))
                        .on_disabled_hover_text(not_yet)
                        .clicked()
                    {
                        pick = Some(CitationPick::Open(c.citekey.clone()));
                        ui.close();
                    }
                    if ui
                        .add_enabled(actionable, egui::Button::new(secondary))
                        .on_disabled_hover_text(not_yet)
                        .clicked()
                    {
                        pick = Some(CitationPick::Secondary(c.citekey.clone()));
                        ui.close();
                    }
                });
            }
        });
    pick
}

/// Smallest and largest zoom the page allows. Drawing limits: below the
/// minimum the card text is unreadable, above the maximum one card fills the
/// screen.
#[cfg(all(feature = "gui", not(target_os = "android")))]
const ZOOM_LIMITS: (f64, f64) = (0.15, 4.0);

/// Factor one press of the zoom buttons changes the zoom by.
#[cfg(all(feature = "gui", not(target_os = "android")))]
const ZOOM_STEP: f64 = 1.25;

/// Card title size at zoom 1, points; it scales with the zoom.
#[cfg(all(feature = "gui", not(target_os = "android")))]
const CARD_FONT_SIZE: f64 = 14.0;

/// The page's pan/zoom state, kept between frames (GitHub issue #243).
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Debug, Clone, Copy, PartialEq)]
struct MapViewport {
    /// Zoom chosen by the user; `None` fits the whole star to the viewport
    /// (the starting view, and what Fit returns to).
    zoom: Option<f64>,
    /// Re-centre on the next frame: set on opening, on Fit and Re-centre, on
    /// moving to another concept, and when a fitted view is resized.
    recentre: bool,
    /// The last frame drawn, so a zoom keeps the middle of the view where it
    /// was, and a change in the map's extent (a drag, an expand) does not
    /// shift what is on screen.
    last_zoom: f64,
    last_offset: (f64, f64),
    last_viewport: (f64, f64),
    last_origin: Option<(f64, f64)>,
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
impl Default for MapViewport {
    fn default() -> Self {
        Self {
            zoom: None,
            recentre: true,
            last_zoom: 0.0,
            last_offset: (0.0, 0.0),
            last_viewport: (0.0, 0.0),
            last_origin: None,
        }
    }
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
pub struct MindmapState {
    /// The concept at the centre; `None` is the top, where the corpus root
    /// and the user's own top-level concepts sit side by side.
    current: Option<crate::node_id::NodeId>,
    /// The selected card's node id, or `"paper:<citekey>"` while a
    /// literature card is shown.
    selected: Option<String>,
    message: String,
    viewport: MapViewport,
    /// The concept the view was last centred on; moving to another one
    /// re-centres.
    centred_on: Option<Option<crate::node_id::NodeId>>,
    /// Concepts expanded in place (#246). A concept is only ever a ring card
    /// in its parent's star, so one set serves every star.
    expanded: std::collections::HashSet<crate::node_id::NodeId>,
    /// Cards dragged to a position of the user's choosing (#246), keyed by
    /// `(centre concept, card)` as strings, in world units relative to that
    /// star's centre: a concept pinned in one star is not pinned in another.
    pinned: std::collections::HashMap<(String, String), crate::mindmap_layout::Point>,
    /// The subtopic name being typed in a right-click menu, and the library
    /// concept path it will go under.
    subtopic_draft: Option<(String, String)>,
    bib: BibCache,
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
impl Default for MindmapState {
    /// Opens on the corpus root, so a fresh Kovan shows Nuclear Engineering
    /// and its branches (maintainer brief, 2026-09-22).
    fn default() -> Self {
        Self {
            current: Some(crate::node_id::NodeId::concept(
                crate::node_id::Namespace::Corpus,
                crate::corpus::ROOT_TOPIC,
            )),
            selected: None,
            message: String::new(),
            viewport: MapViewport::default(),
            centred_on: None,
            expanded: Default::default(),
            pinned: Default::default(),
            subtopic_draft: None,
            bib: BibCache::default(),
        }
    }
}

/// A string key for "the star centred on `current`", for pins.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn star_key(current: &Option<crate::node_id::NodeId>) -> String {
    current
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default()
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
impl MindmapState {
    /// The concept at the centre (`None` is the top). Read by the app's
    /// back/forward history (#242).
    pub(crate) fn current(&self) -> Option<&crate::node_id::NodeId> {
        self.current.as_ref()
    }

    /// Centre the map on `concept`: how back/forward, and the Wiki sharing
    /// its location with this view, move the map (#242). A selection or
    /// half-typed subtopic belongs to the old centre, so both are cleared.
    pub(crate) fn set_current(&mut self, concept: Option<crate::node_id::NodeId>) {
        if self.current != concept {
            self.current = concept;
            self.selected = None;
            self.subtopic_draft = None;
        }
    }

    fn card(
        index: Option<&KnowledgeIndex>,
        entries: &std::collections::HashMap<String, (String, String)>,
        concept: crate::runtime_graph::RuntimeConcept,
    ) -> StarCard {
        let citations = crate::runtime_graph::citations(index, entries, &concept.id);
        StarCard { concept, citations }
    }

    /// The star for `current`: the centre card (the concept itself, or
    /// `None` at the top, which is not a modelled entity) and the ring of its
    /// direct sub-concepts, from the runtime graph (corpus and library).
    fn star_cards(
        index: Option<&KnowledgeIndex>,
        entries: &std::collections::HashMap<String, (String, String)>,
        current: Option<&crate::node_id::NodeId>,
    ) -> (Option<StarCard>, Vec<StarCard>) {
        let centre = current
            .and_then(|id| crate::runtime_graph::concept(index, id))
            .map(|c| Self::card(index, entries, c));
        let ring = crate::runtime_graph::children(index, current)
            .into_iter()
            .map(|c| Self::card(index, entries, c))
            .collect();
        (centre, ring)
    }

    /// Card colour by kind: corpus topics Gruvbox aqua, projects orange,
    /// topics blue (the palette the old renderer used), Unsorted grey.
    fn color_for(kind: crate::runtime_graph::ConceptKind) -> egui::Color32 {
        use crate::runtime_graph::ConceptKind;
        match kind {
            ConceptKind::CorpusTopic => egui::Color32::from_rgb(142, 192, 124),
            ConceptKind::Project => egui::Color32::from_rgb(220, 150, 60),
            ConceptKind::Topic => egui::Color32::from_rgb(90, 140, 220),
            ConceptKind::Unsorted => egui::Color32::from_gray(150),
        }
    }

    /// Draw the mindmap and process this frame's interaction. Returns
    /// `Some` when the caller should navigate to a paper's Research
    /// workspace. Every library argument is optional: with no Kovan folder
    /// open the map shows the built-in corpus alone.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        root: Option<&KovanRoot>,
        index: Option<&KnowledgeIndex>,
        graph: Option<&KnowledgeGraph>,
    ) -> Option<MindmapAction> {
        use crate::mindmap_layout::Point;
        use crate::mindmap_view::{fit_zoom, star_bounds, star_layout, CanvasLayout, CARD_SIZE};
        use crate::node_id::{Namespace, NodeId};

        let mut action = None;

        // ── Breadcrumb ──────────────────────────────────────────────────
        let mut crumb_to: Option<Option<NodeId>> = None;
        ui.horizontal(|ui| {
            if ui.link("Top").clicked() {
                crumb_to = Some(None);
            }
            if let Some(cur) = &self.current {
                for (id, title) in crate::runtime_graph::breadcrumb(index, cur) {
                    ui.label(">");
                    if ui.link(title).clicked() {
                        crumb_to = Some(Some(id));
                    }
                }
            }
            if root.is_none() {
                ui.weak("  (built-in corpus — open a Kovan folder on Home to add your own)");
            }
            if !self.message.is_empty() {
                ui.weak(&self.message);
            }
        });
        if let Some(to) = crumb_to {
            self.set_current(to);
        }

        // ── The star: cards, which ring cards are expanded, where all go ─
        let empty = std::collections::HashMap::new();
        let (centre, ring, fans) = {
            let entries = match root {
                Some(r) => self.bib.entries(r),
                None => &empty,
            };
            let (centre, ring) = Self::star_cards(index, entries, self.current.as_ref());
            let fans: Vec<Vec<StarCard>> = ring
                .iter()
                .map(|c| {
                    if !self.expanded.contains(&c.concept.id) {
                        return Vec::new();
                    }
                    crate::runtime_graph::children(index, Some(&c.concept.id))
                        .into_iter()
                        .map(|k| Self::card(index, entries, k))
                        .collect()
                })
                .collect();
            (centre, ring, fans)
        };
        let fan_sizes: Vec<usize> = fans.iter().map(Vec::len).collect();
        let auto = star_layout(&fan_sizes);
        let here_key = star_key(&self.current);
        let placed = |card: &StarCard, auto: Point| {
            self.pinned
                .get(&(here_key.clone(), card.concept.id.to_string()))
                .copied()
                .unwrap_or(auto)
        };
        let mut cards: Vec<(&StarCard, Point, CardRole)> = Vec::new();
        if let Some(c) = &centre {
            cards.push((c, Point::new(0.0, 0.0), CardRole::Centre));
        }
        for (i, c) in ring.iter().enumerate() {
            cards.push((c, placed(c, auto.ring[i]), CardRole::Ring(i)));
        }
        for (i, fan) in fans.iter().enumerate() {
            for (k, c) in fan.iter().enumerate() {
                cards.push((c, placed(c, auto.fans[i][k]), CardRole::Fan(i, k)));
            }
        }
        let ring_points: Vec<Point> = cards
            .iter()
            .filter(|(_, _, r)| *r != CardRole::Centre)
            .map(|(_, p, _)| *p)
            .collect();
        let bounds = star_bounds(centre.is_some(), &ring_points);
        let any_pinned_here = self.pinned.keys().any(|(c, _)| c == &here_key);

        // Where "Add subtopic" on the empty canvas goes: the top of the
        // user's library, or the library concept you are on. Not a corpus
        // topic (read-only; linking to one is a connection, #252).
        let canvas_subtopic_parent: Option<(String, String)> = match (&self.current, root) {
            (_, None) => None,
            (None, Some(_)) => Some((String::new(), "the top of your library".to_string())),
            (Some(id), Some(_)) => centre
                .as_ref()
                .filter(|c| {
                    c.concept.kind.accepts_subtopics() && id.namespace == Namespace::Library
                })
                .map(|c| (id.path.clone(), c.concept.title.clone())),
        };

        // ── Zoom controls ───────────────────────────────────────────────
        let vp = &mut self.viewport;
        let shown_zoom = vp.zoom.unwrap_or(vp.last_zoom.max(ZOOM_LIMITS.0));
        let mut unpin_all = false;
        let mut collapse_all = false;
        ui.horizontal(|ui| {
            ui.label("Zoom:");
            if ui.button(" − ").clicked() {
                vp.zoom = Some(shown_zoom / ZOOM_STEP);
            }
            if ui.button(" + ").clicked() {
                vp.zoom = Some(shown_zoom * ZOOM_STEP);
            }
            if ui.button("Fit").on_hover_text("Whole map in view").clicked() {
                vp.zoom = None;
                vp.recentre = true;
            }
            if ui.button("100 %").clicked() {
                vp.zoom = Some(1.0);
            }
            if ui
                .button("Re-centre")
                .on_hover_text("Put the current concept back in the middle")
                .clicked()
            {
                vp.recentre = true;
            }
            ui.label(format!("{:.0} %", 100.0 * shown_zoom));
            ui.separator();
            if ui
                .add_enabled(any_pinned_here, egui::Button::new("Unpin all"))
                .on_hover_text("Put every card you dragged here back in its place")
                .clicked()
            {
                unpin_all = true;
            }
            if ui
                .add_enabled(fan_sizes.iter().any(|&m| m > 0), egui::Button::new("Collapse all"))
                .clicked()
            {
                collapse_all = true;
            }
            ui.separator();
            ui.weak("Drag the background or scroll to pan; drag a card to pin it; Ctrl + scroll to zoom");
        });

        // ── Viewport: which zoom, and whether to move the scroll offset ─
        let avail = ui.available_size();
        let viewport = (avail.x as f64, avail.y as f64);
        if self.centred_on.as_ref() != Some(&self.current) {
            self.centred_on = Some(self.current.clone());
            vp.recentre = true;
            vp.last_origin = None;
        }
        if vp.zoom.is_none() && viewport != vp.last_viewport {
            vp.recentre = true;
        }
        let zoom = vp
            .zoom
            .unwrap_or_else(|| fit_zoom(bounds, viewport))
            .clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1);
        if let Some(z) = vp.zoom.as_mut() {
            *z = z.clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1);
        }
        let canvas = CanvasLayout::new(bounds, zoom, viewport);
        // The centre of the star is the world origin, with or without a
        // centre card.
        let offset = if vp.recentre {
            vp.recentre = false;
            Some(canvas.offset_centring(Point::new(0.0, 0.0), viewport))
        } else if vp.last_zoom > 0.0 && zoom != vp.last_zoom {
            // Keep the world point at the middle of the view where it is.
            let before = CanvasLayout::new(bounds, vp.last_zoom, vp.last_viewport);
            let middle = before.to_world((
                vp.last_offset.0 + 0.5 * vp.last_viewport.0,
                vp.last_offset.1 + 0.5 * vp.last_viewport.1,
            ));
            Some(canvas.offset_centring(middle, viewport))
        } else {
            // The map's extent changed (a card dragged past the edge, a
            // branch expanded): shift the scroll by as much as the world
            // origin moved on the canvas, so nothing on screen jumps.
            vp.last_origin.filter(|o| *o != canvas.origin).map(|o| {
                (
                    (vp.last_offset.0 + canvas.origin.0 - o.0).max(0.0),
                    (vp.last_offset.1 + canvas.origin.1 - o.1).max(0.0),
                )
            })
        };

        // ── The canvas ──────────────────────────────────────────────────
        let mut area = egui::ScrollArea::both()
            .id_salt("mindmap-viewport")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .scroll_source(egui::scroll_area::ScrollSource::ALL);
        if let Some((x, y)) = offset {
            area = area.scroll_offset(egui::vec2(x as f32, y as f32));
        }
        let mut drilled: Option<NodeId> = None;
        let mut opened_paper = None;
        let mut literature_card_for = None;
        let mut newly_selected = None;
        let mut toggled: Option<NodeId> = None;
        let mut pin_moves: Vec<((String, String), Point)> = Vec::new();
        let mut unpin = None;
        let mut create_subtopic_req = None;
        let mut draft = self.subtopic_draft.take();
        let selected = self.selected.clone();
        let pinned = &self.pinned;
        let expanded = &self.expanded;
        let output = area.show(ui, |ui| {
            let (rect, background) = ui.allocate_exact_size(
                egui::vec2(canvas.size.0 as f32, canvas.size.1 as f32),
                egui::Sense::click(),
            );
            if background.clicked() {
                newly_selected = Some(None);
            }
            if let Some((parent, label)) = &canvas_subtopic_parent {
                background.context_menu(|ui| {
                    subtopic_menu_item(ui, parent, label, &mut draft, &mut create_subtopic_req);
                });
            }

            let painter = ui.painter_at(rect);
            let at = |p: Point| {
                let (x, y) = canvas.to_canvas(p);
                rect.min + egui::vec2(x as f32, y as f32)
            };
            let z = zoom as f32;
            let card_size = egui::vec2(CARD_SIZE.0 as f32, CARD_SIZE.1 as f32) * z;

            // Spokes under the cards: centre to ring, ring to its fan.
            let stroke = egui::Stroke::new((1.5 * z).max(0.5), egui::Color32::from_gray(120));
            let ring_at: Vec<Point> = cards
                .iter()
                .filter_map(|(_, p, r)| matches!(r, CardRole::Ring(_)).then_some(*p))
                .collect();
            for (_, p, role) in &cards {
                match role {
                    CardRole::Ring(_) if centre.is_some() => {
                        painter.line_segment([at(Point::new(0.0, 0.0)), at(*p)], stroke);
                    }
                    CardRole::Fan(i, _) => {
                        painter.line_segment([at(ring_at[*i]), at(*p)], stroke);
                    }
                    _ => {}
                }
            }

            for (c, p, role) in &cards {
                let concept = &c.concept;
                let id = concept.id.to_string();
                let r = egui::Rect::from_center_size(at(*p), card_size);
                let sense = if *role == CardRole::Centre {
                    egui::Sense::click()
                } else {
                    egui::Sense::click_and_drag()
                };
                let resp = ui.interact(r, ui.id().with(("mindmap-card", &id)), sense);
                let colour = Self::color_for(concept.kind);
                let is_selected = selected.as_deref() == Some(id.as_str());
                let is_pinned = pinned.contains_key(&(here_key.clone(), id.clone()));
                let rounding = 6.0 * z;

                // The card: fill, colour strip, border, title, counts.
                painter.rect_filled(r, rounding, colour.gamma_multiply(0.25));
                painter.rect_filled(
                    egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 6.0 * z, r.max.y)),
                    egui::CornerRadius {
                        nw: rounding as u8,
                        sw: rounding as u8,
                        ne: 0,
                        se: 0,
                    },
                    colour,
                );
                let edge = if is_selected {
                    egui::Stroke::new((2.5 * z).max(1.0), egui::Color32::WHITE)
                } else {
                    let w = if *role == CardRole::Centre { 2.5 } else { 1.2 };
                    egui::Stroke::new((w * z).max(0.5), colour)
                };
                painter.rect_stroke(r, rounding, edge, egui::StrokeKind::Middle);
                let text = painter.with_clip_rect(r.shrink(3.0 * z));
                let left = r.min.x + 12.0 * z;
                text.text(
                    egui::pos2(left, r.center().y - 7.0 * z),
                    egui::Align2::LEFT_CENTER,
                    &concept.title,
                    egui::FontId::proportional((CARD_FONT_SIZE * zoom) as f32),
                    ui.visuals().strong_text_color(),
                );
                let mut counts = format!("\u{1F4C4} {}", c.citations.len());
                if concept.sub_concepts > 0 {
                    counts.push_str(&format!("   \u{2937} {}", concept.sub_concepts));
                }
                if is_pinned {
                    counts.push_str("   \u{1F4CC}");
                }
                text.text(
                    egui::pos2(left, r.center().y + 10.0 * z),
                    egui::Align2::LEFT_CENTER,
                    counts,
                    egui::FontId::proportional((0.78 * CARD_FONT_SIZE * zoom) as f32),
                    ui.visuals().weak_text_color(),
                );

                // Expand/collapse toggle on ring cards with sub-concepts.
                if let CardRole::Ring(_) = role {
                    if concept.sub_concepts > 0 {
                        let open = expanded.contains(&concept.id);
                        let t = egui::Rect::from_center_size(
                            egui::pos2(r.max.x - 12.0 * z, r.center().y),
                            egui::vec2(18.0, 18.0) * z,
                        );
                        let tr = ui
                            .interact(
                                t,
                                ui.id().with(("mindmap-toggle", &id)),
                                egui::Sense::click(),
                            )
                            .on_hover_text(if open {
                                "Collapse"
                            } else {
                                "Show sub-concepts here"
                            });
                        text.text(
                            t.center(),
                            egui::Align2::CENTER_CENTER,
                            if open { "\u{25BE}" } else { "\u{25B8}" },
                            egui::FontId::proportional((1.1 * CARD_FONT_SIZE * zoom) as f32),
                            ui.visuals().strong_text_color(),
                        );
                        if tr.clicked() {
                            toggled = Some(concept.id.clone());
                        }
                    }
                }

                let resp = resp.on_hover_ui(|ui| citations_hover(ui, &concept.title, &c.citations));
                if resp.clicked() {
                    newly_selected = Some(Some(id.clone()));
                }
                if resp.double_clicked() {
                    drilled = Some(concept.id.clone());
                }
                if resp.dragged() {
                    let d = resp.drag_delta() / z;
                    pin_moves.push((
                        (here_key.clone(), id.clone()),
                        Point::new(p.x + d.x as f64, p.y + d.y as f64),
                    ));
                }
                resp.context_menu(|ui| {
                    ui.strong(&concept.title);
                    if concept.kind == crate::runtime_graph::ConceptKind::CorpusTopic {
                        ui.weak("built-in corpus (read-only)");
                    }
                    ui.separator();
                    match citations_menu(ui, &c.citations, "Literature card") {
                        Some(CitationPick::Open(k)) => opened_paper = Some(k),
                        Some(CitationPick::Secondary(k)) => literature_card_for = Some(k),
                        None => {}
                    }
                    ui.separator();
                    if ui.button("Go here").clicked() {
                        drilled = Some(concept.id.clone());
                        ui.close();
                    }
                    if concept.kind.accepts_subtopics() && root.is_some() {
                        subtopic_menu_item(
                            ui,
                            &concept.id.path,
                            &concept.title,
                            &mut draft,
                            &mut create_subtopic_req,
                        );
                    }
                    if is_pinned && ui.button("Unpin").clicked() {
                        unpin = Some((here_key.clone(), id.clone()));
                        ui.close();
                    }
                });
            }
        });
        let vp = &mut self.viewport;
        vp.last_zoom = zoom;
        vp.last_offset = (output.state.offset.x as f64, output.state.offset.y as f64);
        vp.last_viewport = viewport;
        vp.last_origin = Some(canvas.origin);

        // Ctrl + scroll over the map zooms it (egui reports a zoom delta, not
        // a scroll, so it does not also pan).
        if ui.rect_contains_pointer(output.inner_rect) {
            let factor = ui.input(|i| i.zoom_delta()) as f64;
            if factor != 1.0 {
                vp.zoom = Some((zoom * factor).clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1));
            }
        }

        // ── Apply this frame's interaction ──────────────────────────────
        self.subtopic_draft = draft;
        if let Some(sel) = newly_selected {
            self.selected = sel;
        }
        for (key, p) in pin_moves {
            self.pinned.insert(key, p);
        }
        if let Some(key) = unpin {
            self.pinned.remove(&key);
        }
        if unpin_all {
            self.pinned.retain(|(c, _), _| c != &here_key);
        }
        if collapse_all {
            self.expanded.clear();
        }
        if let Some(id) = toggled {
            if !self.expanded.remove(&id) {
                self.expanded.insert(id);
            }
        }
        if let (Some((parent, name)), Some(root), Some(index)) = (create_subtopic_req, root, index)
        {
            match create_subtopic(root, index, &parent, &name) {
                Ok(()) => self.message = format!("added {name:?}"),
                Err(e) => self.message = format!("could not add subtopic: {e}"),
            }
        }
        if let Some(citekey) = literature_card_for {
            self.selected = Some(graph::paper_node(&citekey));
        }
        if let Some(id) = drilled {
            self.set_current(Some(id));
        }
        if let Some(citekey) = opened_paper {
            action = Some(MindmapAction::OpenPaper(citekey));
        }

        if let (Some(root), Some(index), Some(graph)) = (root, index, graph) {
            self.literature_card_ui(ui, root, index, graph);
        }

        action
    }

    fn literature_card_ui(
        &self,
        ui: &mut egui::Ui,
        root: &KovanRoot,
        index: &KnowledgeIndex,
        graph: &KnowledgeGraph,
    ) {
        let Some(selected) = &self.selected else {
            return;
        };
        let Some(citekey) = selected.strip_prefix("paper:") else {
            return;
        };

        egui::Window::new("Literature card")
            .id(ui.id().with("literature-card"))
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                let card = literature_card(root, index, graph, citekey);
                ui.strong(&card.title_or_citekey);
                if !card.author_year.is_empty() {
                    ui.label(&card.author_year);
                }
                if !card.topics.is_empty() || !card.projects.is_empty() {
                    ui.label(format!(
                        "Topics: {}  Projects: {}",
                        card.topics.join(", "),
                        card.projects.join(", ")
                    ));
                }
                ui.label(format!(
                    "{} notes, {} formulas, {} tables, {} graphs",
                    card.note_count, card.formula_count, card.table_count, card.graph_count
                ));
                ui.label(format!(
                    "{} citations, {} backlinks",
                    card.citation_count, card.backlink_count
                ));
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
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn star_cards_show_concepts_with_papers_as_citations() {
        use crate::node_id::{Namespace, NodeId};
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        EntityConfig::topic("fuel", "Fuel")
            .save(&root.topics_dir().join("htgrs").join("fuel"))
            .unwrap();
        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();
        let index = KnowledgeIndex::rebuild(&root);
        let entries = std::collections::HashMap::new();

        // The top: the corpus root beside the user's topic, no centre card,
        // no paper cards; the user topic's badge counts its paper and its
        // sub-concept.
        let (centre, ring) = MindmapState::star_cards(Some(&index), &entries, None);
        assert!(centre.is_none());
        let titles: Vec<&str> = ring.iter().map(|c| c.concept.title.as_str()).collect();
        assert_eq!(titles, ["Nuclear Engineering", "HTGRs"]);
        assert_eq!(ring[1].citations.len(), 1);
        assert_eq!(ring[1].concept.sub_concepts, 1);

        // On "htgrs": the concept at the centre carrying the paper as a
        // citation, and only its sub-concept on the ring.
        let htgrs = NodeId::concept(Namespace::Library, "htgrs");
        let (centre, ring) = MindmapState::star_cards(Some(&index), &entries, Some(&htgrs));
        let centre = centre.expect("a centre card");
        assert_eq!(centre.citations[0].citekey, "wang2018multiphysics");
        assert_eq!(ring.len(), 1);
        assert_eq!(
            ring[0].concept.id,
            NodeId::concept(Namespace::Library, "htgrs/fuel")
        );
    }

    /// Papers not yet classified are never lost: at the top they are the
    /// citations of a synthetic "Unsorted" card.
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn unsorted_papers_appear_under_a_synthetic_card() {
        let (_dir, root) = make_root();
        EntityConfig::paper(CiteKey::parse("orphan2020paper").unwrap(), Access::Open)
            .with_topics(["unsorted"])
            .save_paper(&root.paper_dir("orphan2020paper"))
            .unwrap();
        let index = KnowledgeIndex::rebuild(&root);
        let (_, ring) =
            MindmapState::star_cards(Some(&index), &std::collections::HashMap::new(), None);
        let unsorted = ring
            .iter()
            .find(|c| c.concept.kind == crate::runtime_graph::ConceptKind::Unsorted)
            .expect("an Unsorted card");
        assert_eq!(unsorted.citations[0].citekey, "orphan2020paper");
    }

    /// A fresh map with no folder open is centred on the corpus root and
    /// shows its nine branches (the brief's acceptance test, headless).
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn a_fresh_map_shows_the_corpus_with_no_folder() {
        let state = MindmapState::default();
        let (centre, ring) =
            MindmapState::star_cards(None, &std::collections::HashMap::new(), state.current());
        assert_eq!(centre.unwrap().concept.title, "Nuclear Engineering");
        assert_eq!(ring.len(), 9);
    }

    /// The citation cache formats labels as `bib_display` does, sorts a
    /// concept's citations by author/year, falls back to the citekey for a
    /// paper with no BibTeX entry, and re-reads the file when it changes.
    #[test]
    fn citations_come_from_the_cached_bibliography() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        for key in ["zhang2020later", "abe2010earlier", "nobib2015paper"] {
            EntityConfig::paper(CiteKey::parse(key).unwrap(), Access::Open)
                .with_topics(["htgrs"])
                .save_paper(&root.paper_dir(key))
                .unwrap();
        }
        std::fs::write(
            root.bibliography_path(),
            "@article{zhang2020later, title={Later}, author={Zhang, Wei}, year={2020}}\n\
             @article{abe2010earlier, title={Earlier}, author={Abe, Ken and Oka, Y}, year={2010}}\n",
        )
        .unwrap();
        let index = KnowledgeIndex::rebuild(&root);
        let mut cache = BibCache::default();
        let cites = concept_citations(&index, cache.entries(&root), "htgrs");
        let labels: Vec<String> = cites.iter().map(Citation::label).collect();
        assert_eq!(
            labels,
            ["nobib2015paper", "Abe 2010 — Earlier", "Zhang 2020 — Later"],
            "no-entry paper first (empty author/year), then by author/year"
        );
        assert_eq!(
            bib_display(&root, "abe2010earlier"),
            ("Earlier".into(), "Abe 2010".into())
        );

        // A changed file is picked up (the modification time moves on).
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            root.bibliography_path(),
            "@article{nobib2015paper, title={Now Listed}, author={Ng, A}, year={2015}}\n",
        )
        .unwrap();
        let cites = concept_citations(&index, cache.entries(&root), "htgrs");
        assert!(cites.iter().any(|c| c.label() == "Ng 2015 — Now Listed"));
    }

    #[test]
    fn extract_summary_reads_up_to_the_next_heading() {
        let md = "# Title\n\n## Summary\n\nThis is the summary.\nStill the summary.\n\n## Notes\n\nNot the summary.\n";
        assert_eq!(
            extract_summary(md),
            "This is the summary.\nStill the summary."
        );
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
        let mut session = PaperSession::open(&root, "wang2018multiphysics").unwrap();
        session.append_block(
            "# A table\n\n```toml\n[kovan]\nid = \"t1\"\nkind = \"digitised_table\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 1\n```\n",
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
        assert_eq!(
            card.backlink_count, 1,
            "lee2020corrosion's [@wang2018multiphysics] citation is a backlink to this paper"
        );
    }

    #[test]
    fn create_subtopic_matches_the_parents_kind() {
        let (_dir, root) = make_root();
        EntityConfig::project("outram-park", "Outram Park")
            .save(&root.projects_dir().join("outram-park"))
            .unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        create_subtopic(&root, &index, "outram-park", "Sub Effort").unwrap();
        assert!(EntityConfig::is_entity(
            &root.projects_dir().join("outram-park").join("sub-effort")
        ));

        create_subtopic(&root, &index, "", "New Topic").unwrap();
        assert!(EntityConfig::is_entity(
            &root.topics_dir().join("new-topic")
        ));
    }

    #[test]
    fn create_subtopic_rejects_an_empty_name() {
        let (_dir, root) = make_root();
        let index = KnowledgeIndex::rebuild(&root);
        assert!(create_subtopic(&root, &index, "", "   ").is_err());
    }
}
