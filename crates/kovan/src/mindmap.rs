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
//! until "Unpin" or "Reset nodes" (#246; the button was "Unpin all" until
//! the maintainer renamed it, 2026-09-22). It sits on a two-axis `ScrollArea`
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
    /// "Sort into…" was chosen on a citation — the caller should open the
    /// shared sort-a-paper flow for this citekey (`op-j3ib`). The Mindmap
    /// does not own that dialog: the Wiki, the Mindmap and the PDF reader
    /// all reach the same one, so it lives with the app.
    SortPaper(String),
    /// A subtopic was created — the caller should rebuild the shared
    /// `KnowledgeIndex` before the next frame, or the new node is on disk
    /// and absent from every view until the folder is reopened.
    KnowledgeChanged,
    /// Something needing a Kovan folder was asked for while none is open —
    /// the caller should open the setup dialog (maintainer, 2026-09-22).
    ///
    /// The map draws the built-in corpus with no folder at all (#247), so
    /// "no folder open" is the **ordinary** first-run state here, not an
    /// error. An action that needs somewhere to write therefore has to say
    /// so and offer the way out, rather than vanishing from the menu.
    OpenSetup,
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
            let (title, mut author_year) = entries
                .get(&p.citekey)
                .cloned()
                .unwrap_or_else(|| (p.citekey.clone(), String::new()));
            // Say *why* a paper is here when it is not filed here itself
            // (#276). Without this a paper reached through one of its
            // artifacts is indistinguishable from one the user filed under
            // this concept, and the two are different statements.
            if p.via_artifacts.iter().any(|t| t == path) {
                let note = "via an artifact";
                author_year = if author_year.is_empty() {
                    note.to_string()
                } else {
                    format!("{author_year} \u{2014} {note}")
                };
            }
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
    _index: &KnowledgeIndex,
    parent_path: &str,
    name: &str,
    kind: EntityKind,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name must not be empty".to_string());
    }
    let slug = crate::classify::slugify(name);
    if slug.is_empty() {
        return Err("name has no usable characters for an id".to_string());
    }
    let parent_kind = kind;
    let tree_root = match parent_kind {
        EntityKind::Project => root.projects_dir(),
        _ => root.topics_dir(),
    };
    let dir = if parent_path.is_empty() {
        tree_root.join(&slug)
    } else {
        tree_root.join(parent_path).join(&slug)
    };
    // Every ancestor segment must exist as an entity or the new subtopic is
    // invisible: `index::scan_collections` skips a directory without a
    // `kovan.toml` and does **not** recurse into it, so a subtopic under a
    // **corpus** parent — whose path has no user directories behind it —
    // would be written to disk and then never indexed, never drawn
    // (maintainer, 2026-09-22). `ensure_classification_paths` is the same
    // routine ingestion and the sort dialog already use for exactly this.
    if !parent_path.is_empty() {
        let ancestors = vec![parent_path.to_string()];
        let (topics, projects) = match kind {
            EntityKind::Project => (Vec::new(), ancestors),
            _ => (ancestors, Vec::new()),
        };
        crate::entity::ensure_classification_paths(root, &topics, &projects)
            .map_err(|e| e.to_string())?;
    }
    let config = match parent_kind {
        EntityKind::Project => EntityConfig::project(slug, name),
        _ => EntityConfig::topic(slug, name),
    };
    config.save(&dir).map_err(|e| e.to_string())
}

/// The stand-in for "Add subtopic here…" when **no Kovan folder is open**:
/// a button that says why the action is unavailable and opens the setup
/// dialog. Returns `true` on the frame it is clicked.
///
/// Shown rather than hidden (maintainer, 2026-09-22, whose own words are the
/// label). Kovan draws its built-in nuclear-engineering map with no folder
/// at all (#247), so a first-time user right-clicks a corpus topic before
/// they have a library — and an entry that is simply absent is
/// indistinguishable from the feature not existing. It reads as a bug, and
/// the maintainer reported it as one ("i can't even see add subtopic").
/// Saying what is missing, and being the way to fix it, turns a dead end
/// into the next step.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn setup_prompt_item(ui: &mut egui::Ui) -> bool {
    let clicked = ui
        .button("please setup your kovan repo before adding subtopic")
        .on_hover_text("opens the Kovan setup dialog")
        .clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// The "Add subtopic" entry of a right-click menu: a button that arms
/// `draft` for a subtopic under `parent` (shown as `parent_label`) and
/// closes the menu. `parent` is a library concept path (`""` for the top of
/// the user's library). The half-typed name lives in `draft`, which the page
/// keeps between frames.
///
/// **The name is NOT typed here (fixed 2026-09-22).** This used to host the
/// text field and a Create button inside the context menu itself, and the
/// maintainer reported that "the box refuses to let me add topic": a
/// `TextEdit` inside an egui context menu does not reliably keep focus —
/// interacting with it can dismiss the menu that owns it, so the field
/// cannot be typed into. The entry now only *arms* the draft;
/// [`MindmapState::subtopic_dialog_ui`] draws a real window where the name
/// can actually be entered, which is also how every other text entry in this
/// app already works (the sort dialog, the connection dialogs).
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn subtopic_menu_item(
    ui: &mut egui::Ui,
    parent: &str,
    parent_label: &str,
    draft: &mut Option<(String, String, EntityKind)>,
) {
    for (label, kind, what) in [
        ("Add subtopic here\u{2026}", EntityKind::Topic, "subtopic"),
        ("Add project here\u{2026}", EntityKind::Project, "project"),
    ] {
        if ui
            .button(label)
            .on_hover_text(format!("a new {what} under {parent_label}"))
            .clicked()
        {
            *draft = Some((parent.to_string(), String::new(), kind));
            // Close the menu: the name is typed in the dialog
            // ([`MindmapState::subtopic_dialog_ui`]), not here.
            ui.close();
        }
    }
}

/// The user-concept CRUD entries — Rename, Move and Delete — in the same
/// right-click menu as the add actions.
///
/// All three are **full** operations as of 2026-09-23: they rewrite every
/// reference to the paths involved, through
/// [`crate::concept_ops`]. ~~Rename changed the display name only, and
/// Delete was offered for an empty concept alone~~ — epic #241 had deferred
/// the transactional rewrite those need, and this now exists, so the limits
/// are gone. A populated concept can be renamed, moved or deleted, and the
/// papers and artifacts that named it follow.
///
/// Delete confirms first, showing the plan's own count of what it will
/// touch: it is the one irreversible action here, and "2 concepts, 4 files"
/// is what tells a user whether they meant it.
///
/// Corpus concepts get none of these: they are immutable and are not ours to
/// edit.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn concept_crud_items(
    ui: &mut egui::Ui,
    path: &str,
    title: &str,
    kind: EntityKind,
    rename: &mut Option<(String, EntityKind, String)>,
    move_to: &mut Option<(String, EntityKind, String)>,
    delete: &mut Option<(String, EntityKind)>,
) {
    if ui
        .button("Rename\u{2026}")
        .on_hover_text("renames it everywhere it is referenced")
        .clicked()
    {
        *rename = Some((path.to_string(), kind, title.to_string()));
        ui.close();
    }
    if ui
        .button("Move\u{2026}")
        .on_hover_text("re-parents it, with everything under it")
        .clicked()
    {
        *move_to = Some((path.to_string(), kind, String::new()));
        ui.close();
    }
    if ui
        .button("Delete\u{2026}")
        .on_hover_text("deletes it and everything under it")
        .clicked()
    {
        *delete = Some((path.to_string(), kind));
        ui.close();
    }
}

/// The "Add literature here…" entry: arms `draft` with the concept path
/// being filed under, and closes the menu. The paper is chosen in
/// [`MindmapState::literature_dialog_ui`] — a fuzzy search, for the same
/// focus reason the subtopic name is not typed in the menu either.
///
/// This is the inverse of the sort dialog: that one starts from a paper and
/// picks concepts, this one starts from a concept and picks a paper. Both
/// write the same thing — a path in the paper's classification — so a
/// library sorted either way is indistinguishable afterwards.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn literature_menu_item(
    ui: &mut egui::Ui,
    parent: &str,
    parent_label: &str,
    draft: &mut Option<(String, String)>,
) {
    if ui
        .button("Add literature here\u{2026}")
        .on_hover_text(format!("file a paper under {parent_label}"))
        .clicked()
    {
        *draft = Some((parent.to_string(), String::new()));
        ui.close();
    }
}

/// "Add hyperlink…" on a concept's right-click menu (#285), beside "Add
/// subtopic…" where the maintainer asked for it.
///
/// Only arms the draft; the picking is done in
/// [`MindmapState::hyperlink_dialog_ui`], the same split the subtopic and
/// literature entries already use.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn hyperlink_menu_item(
    ui: &mut egui::Ui,
    source: &crate::node_id::NodeId,
    label: &str,
    draft: &mut Option<(crate::node_id::NodeId, String)>,
) {
    if ui
        .button("Add hyperlink\u{2026}")
        .on_hover_text(format!("link {label} to another concept on the map"))
        .clicked()
    {
        *draft = Some((source.clone(), String::new()));
        ui.close();
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

/// A light-blue link card in the star: something the concept you are on is
/// **linked to**, as opposed to one of its sub-concepts (#285, #286).
///
/// Two very different stores feed the same card, because to a reader they
/// are the same thing — "this points somewhere else":
///
/// - a concept hyperlink the user made here, from
///   [`crate::connections`] (`mindmap/connections.toml`);
/// - a relation artifact whose other end is this concept, from
///   [`crate::relation`] (`mindmap.md`) — typically an annotation in a paper
///   connected to this topic in the PDF reader.
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Debug, Clone)]
struct LinkCard {
    /// What the card says.
    label: String,
    /// The small line under it: which kind of link this is.
    detail: String,
    /// Where a double-click goes.
    target: LinkTarget,
    /// The other end, for the tooltip and for "Remove link".
    node: crate::node_id::NodeId,
    /// Whether this link is the user's own hyperlink, and so can be removed
    /// from here. A relation artifact belongs to the paper that owns it and
    /// is edited in the PDF reader, not on the map.
    removable: bool,
}

/// What double-clicking a [`LinkCard`] does.
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Debug, Clone)]
enum LinkTarget {
    /// Travel to another concept.
    Concept(crate::node_id::NodeId),
    /// Open a paper (an artifact link opens the paper that owns it).
    Paper(String),
}

/// The user's links, cached like [`BibCache`]: both files are re-read only
/// when their modification time or length changes, so drawing the map does
/// not parse `mindmap.md` sixty times a second.
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Default)]
pub struct LinkCache {
    connections_stamp: Option<(std::time::SystemTime, u64)>,
    relations_stamp: Option<(std::time::SystemTime, u64)>,
    connections: Vec<crate::connections::Connection>,
    relations: Vec<crate::relation::UserRelation>,
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
fn file_stamp(path: &std::path::Path) -> Option<(std::time::SystemTime, u64)> {
    std::fs::metadata(path)
        .and_then(|m| Ok((m.modified()?, m.len())))
        .ok()
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
impl LinkCache {
    /// The link cards for the concept at `current`, refreshing from disk
    /// first if either file changed.
    fn cards(
        &mut self,
        root: &KovanRoot,
        index: Option<&KnowledgeIndex>,
        current: &crate::node_id::NodeId,
    ) -> Vec<LinkCard> {
        let stamp = file_stamp(&root.mindmap_connections());
        if stamp.is_none() || stamp != self.connections_stamp {
            self.connections_stamp = stamp;
            self.connections = crate::connections::load(root);
        }
        let stamp = file_stamp(&root.mindmap_markdown());
        if stamp.is_none() || stamp != self.relations_stamp {
            self.relations_stamp = stamp;
            self.relations = crate::relation::connections_all(root);
        }

        let title_of = |node: &crate::node_id::NodeId| -> String {
            if let Some(artifact) = &node.artifact {
                // An artifact is named by the paper it is in plus its own id
                // — the identity the reader shows, not a guessed heading.
                return format!("{}#{artifact}", node.path);
            }
            crate::runtime_graph::concept(index, node)
                .map(|c| c.title)
                .unwrap_or_else(|| node.path.clone())
        };

        let mut out: Vec<LinkCard> = crate::connections::for_node(&self.connections, current)
            .into_iter()
            .map(|other| LinkCard {
                label: title_of(other),
                detail: "hyperlink".to_string(),
                target: link_target(other),
                node: other.clone(),
                removable: true,
            })
            .collect();

        // `relation`'s endpoints are the older untyped `graph::NodeId`
        // strings (`artifact:<citekey>#<id>`, `collection:<path>`), so they
        // are read into typed ids before being compared — and **canonicalised**
        // on both sides, for two different reasons. A connection made in the
        // PDF reader names the *library* path of a topic: canonicalising the
        // relation's end is what puts the card on the **corpus** node the map
        // shows for a mirrored path, and canonicalising `current` is what
        // puts it there when the user is standing on the mirror itself. The
        // same trap `runtime_graph::canonical_concept` was written for after
        // the Up button landed on the light-green mirror of a corpus topic.
        let here = crate::runtime_graph::canonical_concept(current);
        for rel in &self.relations {
            let ends = [&rel.source, &rel.target].map(|e| {
                candidate_node_id(e).map(|id| (crate::runtime_graph::canonical_concept(&id), id))
            });
            let other = match &ends {
                [Some((canon, _)), Some((_, raw))] if *canon == here => raw,
                [Some((_, raw)), Some((canon, _))] if *canon == here => raw,
                _ => continue,
            };
            out.push(LinkCard {
                label: title_of(other),
                detail: rel.kind.label().to_string(),
                target: link_target(other),
                node: other.clone(),
                removable: false,
            });
        }
        out
    }
}

/// Where a link to `node` goes: another concept travels, a literature node
/// (a paper, or one of its artifacts) opens that paper.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn link_target(node: &crate::node_id::NodeId) -> LinkTarget {
    match node.kind {
        crate::node_id::EntryKind::Concept => LinkTarget::Concept(node.clone()),
        crate::node_id::EntryKind::Literature => LinkTarget::Paper(node.path.clone()),
    }
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

/// An action a view offers on a citation, beyond "Open".
///
/// Views differ in what they can do with a paper, so each passes the set it
/// supports to [`citations_menu`] rather than the menu hard-coding one
/// "secondary" slot. The Mindmap offers both; the Wiki offers Reclassify.
#[cfg(all(feature = "gui", not(target_os = "android")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CitationAction {
    /// Show the paper's literature card (§9).
    LiteratureCard,
    /// Sort the paper into topics/projects (`op-j3ib`).
    Reclassify,
}

#[cfg(all(feature = "gui", not(target_os = "android")))]
impl CitationAction {
    /// The menu label for this action.
    pub(crate) fn label(self) -> &'static str {
        match self {
            CitationAction::LiteratureCard => "Literature card",
            CitationAction::Reclassify => "Sort into…",
        }
    }
}

/// What a click in a citation list asked for.
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub(crate) enum CitationPick {
    /// Open the paper (a back/forward step).
    Open(String),
    /// One of the view's extra actions, on this citekey.
    Action(CitationAction, String),
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
/// citation as a sub-menu with "Open" and each of `actions`. Corpus
/// citations are listed but not yet actionable: they open through the source
/// resolver (#253).
#[cfg(all(feature = "gui", not(target_os = "android")))]
pub(crate) fn citations_menu(
    ui: &mut egui::Ui,
    citations: &[Citation],
    actions: &[CitationAction],
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
                    for action in actions {
                        if ui
                            .add_enabled(actionable, egui::Button::new(action.label()))
                            .on_disabled_hover_text(not_yet)
                            .clicked()
                        {
                            pick = Some(CitationPick::Action(*action, c.citekey.clone()));
                            ui.close();
                        }
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

/// A link card's fill: "light blue boxes with dark blue underlined text,
/// just like hyperlinks in markdown or wikipedia" (maintainer, 2026-09-23,
/// #285). Solid rather than the 25 % wash the concept cards use, because
/// what makes a hyperlink recognisable is the block of pale colour behind
/// dark text, not a tint of the border.
#[cfg(all(feature = "gui", not(target_os = "android")))]
const LINK_FILL: egui::Color32 = egui::Color32::from_rgb(198, 224, 250);

/// A link card's text, border and underline — the dark blue every wiki
/// trains the eye to read as "this goes somewhere". Dark enough to carry
/// the contrast against [`LINK_FILL`] in either theme, since the fill is a
/// fixed light colour rather than a themed one.
#[cfg(all(feature = "gui", not(target_os = "android")))]
const LINK_TEXT: egui::Color32 = egui::Color32::from_rgb(20, 60, 160);

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
    subtopic_draft: Option<(String, String, EntityKind)>,
    /// A pending "Add literature here…": the concept path being filed
    /// under, and what is typed in its fuzzy search.
    literature_draft: Option<(String, String)>,
    /// A pending "Rename…": the concept path, its kind, and the new name
    /// being typed.
    rename_draft: Option<(String, EntityKind, String)>,
    /// A pending "Move…": the concept path, its kind, and the destination
    /// parent being searched for.
    move_draft: Option<(String, EntityKind, String)>,
    /// A pending "Delete…", awaiting confirmation.
    delete_draft: Option<(String, EntityKind)>,
    /// A pending "Add hyperlink…" (#285): the concept the link starts from,
    /// and what is typed in its fuzzy search.
    hyperlink_draft: Option<(crate::node_id::NodeId, String)>,
    /// The user's links into the current star (#285, #286), re-read only
    /// when their files change.
    links: LinkCache,
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
            literature_draft: None,
            rename_draft: None,
            move_draft: None,
            delete_draft: None,
            hyperlink_draft: None,
            links: LinkCache::default(),
            bib: BibCache::default(),
        }
    }
}

/// The [`crate::node_id::NodeId`] behind a finder candidate's node string
/// (#285).
///
/// `library_candidates` mixes two identity syntaxes: a library collection
/// arrives as `collection:<path>` (the older untyped [`crate::graph`] form)
/// and a corpus topic as its `NodeId`'s own string. Both are read here so
/// nothing downstream has to know which it got.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn candidate_node_id(node: &str) -> Option<crate::node_id::NodeId> {
    crate::node_id::NodeId::parse(node)
        .ok()
        .or_else(|| crate::node_id::NodeId::from_graph_id(node))
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
            self.literature_draft = None;
            self.rename_draft = None;
            self.move_draft = None;
            self.delete_draft = None;
            self.hyperlink_draft = None;
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

    /// Card colour by kind.
    ///
    /// **The two greens carry meaning** (maintainer, 2026-09-22, #274):
    /// **dark green is the built-in corpus and is immutable; light green is
    /// the user's own topics and is editable.** Since a corpus topic now
    /// accepts a user subtopic, the two sit side by side in the same map and
    /// the colour is how you tell which is which at a glance.
    ///
    /// Colour is never the *only* signal: the card's right-click menu still
    /// prints "built-in corpus (read-only)", for anyone who cannot separate
    /// the greens. Both are mid-luminance so they read against the light and
    /// the dark theme alike.
    ///
    /// **Projects are light lilac** (maintainer, 2026-09-22) — off the
    /// green axis entirely, because a project is a different *kind* of thing
    /// from a topic rather than a different owner of one. Unsorted stays
    /// grey: it is an inbox, not a concept.
    fn color_for(kind: crate::runtime_graph::ConceptKind) -> egui::Color32 {
        use crate::runtime_graph::ConceptKind;
        match kind {
            ConceptKind::CorpusTopic => egui::Color32::from_rgb(56, 124, 68),
            ConceptKind::Topic => egui::Color32::from_rgb(150, 210, 140),
            ConceptKind::Project => egui::Color32::from_rgb(198, 176, 232),
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
        use crate::mindmap_view::{
            fit_zoom, parent_position, star_bounds, star_layout_with_parent, up_button_centre,
            CanvasLayout, CARD_SIZE, UP_BUTTON_SIZE,
        };
        use crate::node_id::{Namespace, NodeId};

        let mut action = None;

        // ── Breadcrumb ──────────────────────────────────────────────────
        let mut crumb_to: Option<Option<NodeId>> = None;
        let up = crate::runtime_graph::up_one_level(self.current.as_ref());
        // Up one level (maintainer direction, 2026-09-22): to the parent
        // concept, or to the top from a top-level one. Named once here, for
        // both the breadcrumb button and the parent card in the star (#283).
        let up_label = match &up {
            Some(Some(parent)) => crate::runtime_graph::concept(index, parent)
                .map(|c| c.title)
                .unwrap_or_else(|| parent.path.clone()),
            Some(None) => "the top".to_string(),
            None => String::new(),
        };
        ui.horizontal(|ui| {
            crate::app::navigation_style(ui);
            let up_label = &up_label;
            if ui
                .add_enabled(up.is_some(), egui::Button::new("\u{2B06} Up"))
                .on_hover_text(format!("Up one level, to {up_label}"))
                .on_disabled_hover_text("Already at the top")
                .clicked()
            {
                crumb_to = up.clone();
            }
            ui.separator();
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
        // #285/#286: the user's links sit on the ring beside the
        // sub-concepts, so the layout spaces them like any other card.
        let links: Vec<LinkCard> = match (root, self.current.clone()) {
            (Some(r), Some(current)) => self.links.cards(r, index, &current),
            _ => Vec::new(),
        };
        let mut fan_sizes: Vec<usize> = fans.iter().map(Vec::len).collect();
        fan_sizes.extend(std::iter::repeat_n(0, links.len()));
        // #283: the ring turns half a step when the parent card is shown, so
        // nothing sits in the corridor the dotted line and the Up button use.
        let auto = star_layout_with_parent(&fan_sizes, up.is_some());
        let parent_at = up.is_some().then(|| parent_position(&auto));
        let up_button_at = up.is_some().then(|| up_button_centre(&auto));
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
        // The link cards take the ring slots after the sub-concepts. They are
        // pinnable like any other card, keyed by their target's node id.
        let link_cards: Vec<(&LinkCard, Point)> = links
            .iter()
            .enumerate()
            .map(|(k, l)| {
                let auto = auto.ring[ring.len() + k];
                let pinned = self
                    .pinned
                    .get(&(here_key.clone(), l.node.to_string()))
                    .copied();
                (l, pinned.unwrap_or(auto))
            })
            .collect();
        let ring_points: Vec<Point> = cards
            .iter()
            .filter(|(_, _, r)| *r != CardRole::Centre)
            .map(|(_, p, _)| *p)
            .chain(link_cards.iter().map(|(_, p)| *p))
            // The parent card is part of the map: Fit and the pan limits have
            // to know about it, or it sits outside where the view can go.
            .chain(parent_at)
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
            crate::app::navigation_style(ui);
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
                .add_enabled(any_pinned_here, egui::Button::new("Reset nodes"))
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
            ui.label(
                egui::RichText::new(
                    "Drag the background or scroll to pan; drag a card to pin it; Ctrl + scroll to zoom",
                )
                .small()
                .weak(),
            );
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
        // Set by the Up button or a double-click on the parent card (#283).
        let mut go_up = false;
        // A hyperlink the user asked to remove (#285).
        let mut remove_link: Option<crate::node_id::NodeId> = None;
        let mut opened_paper = None;
        let mut literature_card_for = None;
        let mut reclassify_for = None;
        let mut newly_selected = None;
        let mut toggled: Option<NodeId> = None;
        let mut pin_moves: Vec<((String, String), Point)> = Vec::new();
        let mut unpin = None;
        let mut draft = self.subtopic_draft.take();
        let mut open_setup = false;
        let mut literature_draft = self.literature_draft.take();
        let mut rename_draft = self.rename_draft.take();
        let mut move_draft = self.move_draft.take();
        let mut delete_draft = self.delete_draft.take();
        let mut hyperlink_draft = self.hyperlink_draft.take();
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
                    subtopic_menu_item(ui, parent, label, &mut draft);
                });
            }

            let painter = ui.painter_at(rect);
            let at = |p: Point| {
                let (x, y) = canvas.to_canvas(p);
                rect.min + egui::vec2(x as f32, y as f32)
            };
            let z = zoom as f32;
            let card_size = egui::vec2(CARD_SIZE.0 as f32, CARD_SIZE.1 as f32) * z;

            // Connectors under the cards, centre to ring and ring to its fan:
            // smooth curves from edge to edge, the edges chosen by where the
            // two cards sit ([`crate::mindmap_view::connector`]).
            let stroke = egui::Stroke::new((1.5 * z).max(0.5), egui::Color32::from_gray(120));
            let ring_at: Vec<Point> = cards
                .iter()
                .filter_map(|(_, p, r)| matches!(r, CardRole::Ring(_)).then_some(*p))
                .collect();
            for (_, p, role) in &cards {
                let from = match role {
                    CardRole::Ring(_) if centre.is_some() => Point::new(0.0, 0.0),
                    CardRole::Fan(i, _) => ring_at[*i],
                    _ => continue,
                };
                if let Some(curve) = crate::mindmap_view::connector(from, *p) {
                    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                        curve.map(at),
                        false,
                        egui::Color32::TRANSPARENT,
                        stroke,
                    ));
                }
            }
            // #285/#286: a link's connector is the same curve in the link
            // blue, so it reads as attached to this concept but not as one
            // of its sub-concepts.
            let link_stroke = egui::Stroke::new((1.5 * z).max(0.5), LINK_TEXT);
            for (_, p) in &link_cards {
                if let Some(curve) = crate::mindmap_view::connector(Point::new(0.0, 0.0), *p) {
                    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                        curve.map(at),
                        false,
                        egui::Color32::TRANSPARENT,
                        link_stroke,
                    ));
                }
            }

            // ── "Up one level": the parent card (#283) ──────────────
            // Drawn before the concept cards so the dotted line passes under
            // anything it meets. Maintainer, 2026-09-23: "a purple node with
            // an up button that allows user to go up one level, this will be
            // connected to central node in dotted line with an up button
            // within a box on the dotted line. double clicking brings us to
            // that level."
            if let (Some(parent), Some(button)) = (parent_at, up_button_at) {
                // Purple, and deliberately not one of `color_for`'s kinds:
                // this card is not a concept, it is where you came from. The
                // nearest neighbour on the palette is the projects' light
                // lilac, so this one is darker and more saturated.
                let purple = egui::Color32::from_rgb(150, 110, 210);
                let rounding = 6.0 * z;

                let line = [
                    at(Point::new(0.0, -0.5 * CARD_SIZE.1)),
                    at(Point::new(parent.x, parent.y + 0.5 * CARD_SIZE.1)),
                ];
                painter.extend(egui::Shape::dashed_line(
                    &line,
                    egui::Stroke::new((1.5 * z).max(0.5), purple),
                    (10.0 * z).max(2.0),
                    (7.0 * z).max(2.0),
                ));

                // The Up button, in its own box on that line.
                let b = egui::Rect::from_center_size(
                    at(button),
                    egui::vec2(UP_BUTTON_SIZE.0 as f32, UP_BUTTON_SIZE.1 as f32) * z,
                );
                let br = ui
                    .interact(b, ui.id().with("mindmap-up-button"), egui::Sense::click())
                    .on_hover_text(format!("Up one level, to {up_label}"));
                painter.rect_filled(b, rounding, ui.visuals().extreme_bg_color);
                painter.rect_stroke(
                    b,
                    rounding,
                    egui::Stroke::new((1.5 * z).max(0.5), purple),
                    egui::StrokeKind::Middle,
                );
                painter.text(
                    b.center(),
                    egui::Align2::CENTER_CENTER,
                    "\u{2B06}",
                    egui::FontId::proportional((CARD_FONT_SIZE * zoom) as f32),
                    purple,
                );
                if br.clicked() {
                    go_up = true;
                }

                // The card itself, painted like a concept card so it reads as
                // part of the same map, and double-clicked like one to travel.
                let r = egui::Rect::from_center_size(at(parent), card_size);
                let resp = ui
                    .interact(r, ui.id().with("mindmap-up-card"), egui::Sense::click())
                    .on_hover_text(format!("Double-click to go up to {up_label}"));
                painter.rect_filled(r, rounding, purple.gamma_multiply(0.25));
                painter.rect_filled(
                    egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 6.0 * z, r.max.y)),
                    egui::CornerRadius {
                        nw: rounding as u8,
                        sw: rounding as u8,
                        ne: 0,
                        se: 0,
                    },
                    purple,
                );
                painter.rect_stroke(
                    r,
                    rounding,
                    egui::Stroke::new((1.2 * z).max(0.5), purple),
                    egui::StrokeKind::Middle,
                );
                let text = painter.with_clip_rect(r.shrink(3.0 * z));
                let left = r.min.x + 12.0 * z;
                text.text(
                    egui::pos2(left, r.center().y - 7.0 * z),
                    egui::Align2::LEFT_CENTER,
                    &up_label,
                    egui::FontId::proportional((CARD_FONT_SIZE * zoom) as f32),
                    ui.visuals().strong_text_color(),
                );
                text.text(
                    egui::pos2(left, r.center().y + 10.0 * z),
                    egui::Align2::LEFT_CENTER,
                    "\u{2B06} up one level",
                    egui::FontId::proportional((0.78 * CARD_FONT_SIZE * zoom) as f32),
                    ui.visuals().weak_text_color(),
                );
                if resp.double_clicked() {
                    go_up = true;
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
                    match citations_menu(
                        ui,
                        &c.citations,
                        &[CitationAction::LiteratureCard, CitationAction::Reclassify],
                    ) {
                        Some(CitationPick::Open(k)) => opened_paper = Some(k),
                        Some(CitationPick::Action(CitationAction::LiteratureCard, k)) => {
                            literature_card_for = Some(k)
                        }
                        Some(CitationPick::Action(CitationAction::Reclassify, k)) => {
                            reclassify_for = Some(k)
                        }
                        None => {}
                    }
                    ui.separator();
                    if ui.button("Go here").clicked() {
                        drilled = Some(concept.id.clone());
                        ui.close();
                    }
                    // #285. Offered on every concept, corpus included: the
                    // link is recorded in the *user's* own
                    // `mindmap/connections.toml`, so linking from a built-in
                    // topic changes nothing that is read-only.
                    if root.is_some() {
                        hyperlink_menu_item(ui, &concept.id, &concept.title, &mut hyperlink_draft);
                    }
                    if concept.kind.accepts_subtopics() {
                        if root.is_some() {
                            subtopic_menu_item(ui, &concept.id.path, &concept.title, &mut draft);
                            literature_menu_item(
                                ui,
                                &concept.id.path,
                                &concept.title,
                                &mut literature_draft,
                            );
                            // CRUD, for the user's own concepts only — the
                            // corpus is immutable.
                            if let (Some(index), Some(entity_kind)) = (
                                index,
                                match concept.kind {
                                    crate::runtime_graph::ConceptKind::Topic => {
                                        Some(EntityKind::Topic)
                                    }
                                    crate::runtime_graph::ConceptKind::Project => {
                                        Some(EntityKind::Project)
                                    }
                                    _ => None,
                                },
                            ) {
                                let _ = index;
                                ui.separator();
                                concept_crud_items(
                                    ui,
                                    &concept.id.path,
                                    &concept.title,
                                    entity_kind,
                                    &mut rename_draft,
                                    &mut move_draft,
                                    &mut delete_draft,
                                );
                            }
                        } else if setup_prompt_item(ui) {
                            open_setup = true;
                        }
                    }
                    if is_pinned && ui.button("Unpin").clicked() {
                        unpin = Some((here_key.clone(), id.clone()));
                        ui.close();
                    }
                });
            }

            // ── Link cards (#285, #286) ─────────────────────────────
            // "light blue boxes with dark blue underlined text, just like
            // hyperlinks in markdown or wikipedia" (maintainer, 2026-09-23).
            for (link, p) in &link_cards {
                let key = link.node.to_string();
                let r = egui::Rect::from_center_size(at(*p), card_size);
                let resp = ui.interact(
                    r,
                    ui.id().with(("mindmap-link", &key)),
                    egui::Sense::click_and_drag(),
                );
                let rounding = 6.0 * z;
                painter.rect_filled(r, rounding, LINK_FILL);
                painter.rect_stroke(
                    r,
                    rounding,
                    egui::Stroke::new((1.2 * z).max(0.5), LINK_TEXT),
                    egui::StrokeKind::Middle,
                );
                let text = painter.with_clip_rect(r.shrink(3.0 * z));
                let left = r.min.x + 12.0 * z;
                let drawn = text.text(
                    egui::pos2(left, r.center().y - 7.0 * z),
                    egui::Align2::LEFT_CENTER,
                    &link.label,
                    egui::FontId::proportional((CARD_FONT_SIZE * zoom) as f32),
                    LINK_TEXT,
                );
                // The underline is what makes it read as a link rather than
                // as a blue card; drawn from the text's own box so it is
                // exactly as wide as the text at any zoom.
                text.line_segment(
                    [
                        egui::pos2(drawn.min.x, drawn.max.y),
                        egui::pos2(drawn.max.x, drawn.max.y),
                    ],
                    egui::Stroke::new((1.0 * z).max(0.5), LINK_TEXT),
                );
                text.text(
                    egui::pos2(left, r.center().y + 10.0 * z),
                    egui::Align2::LEFT_CENTER,
                    format!("\u{1F517} {}", link.detail),
                    egui::FontId::proportional((0.78 * CARD_FONT_SIZE * zoom) as f32),
                    LINK_TEXT.gamma_multiply(0.75),
                );

                let resp = resp.on_hover_text(format!("{}\ndouble-click to follow", link.node));
                if resp.double_clicked() {
                    match &link.target {
                        LinkTarget::Concept(id) => drilled = Some(id.clone()),
                        LinkTarget::Paper(citekey) => opened_paper = Some(citekey.clone()),
                    }
                }
                if resp.dragged() {
                    let d = resp.drag_delta() / z;
                    pin_moves.push((
                        (here_key.clone(), key.clone()),
                        Point::new(p.x + d.x as f64, p.y + d.y as f64),
                    ));
                }
                resp.context_menu(|ui| {
                    ui.strong(&link.label);
                    ui.weak(link.node.to_string());
                    ui.separator();
                    if ui.button("Follow").clicked() {
                        match &link.target {
                            LinkTarget::Concept(id) => drilled = Some(id.clone()),
                            LinkTarget::Paper(citekey) => opened_paper = Some(citekey.clone()),
                        }
                        ui.close();
                    }
                    if link.removable {
                        if ui.button("Remove hyperlink").clicked() {
                            remove_link = Some(link.node.clone());
                            ui.close();
                        }
                    } else {
                        // A relation artifact belongs to the paper that owns
                        // it; the PDF reader's Connections window edits it.
                        ui.weak("a connection from a paper \u{2014} edit it in the reader");
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
        self.literature_draft = literature_draft;
        self.rename_draft = rename_draft;
        self.move_draft = move_draft;
        self.delete_draft = delete_draft;
        self.hyperlink_draft = hyperlink_draft;
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
        // The dialog is the only source of a create request now: the menu
        // entry only arms the draft (see `subtopic_menu_item`).
        if self.literature_dialog_ui(ui, root, index) {
            action = Some(MindmapAction::KnowledgeChanged);
        }
        if let (Some(root), Some(index)) = (root, index) {
            if self.concept_ops_ui(ui, root, index) {
                action = Some(MindmapAction::KnowledgeChanged);
            }
        }
        if self.hyperlink_dialog_ui(ui, root, index) {
            action = Some(MindmapAction::KnowledgeChanged);
        }
        let create_subtopic_req = self.subtopic_dialog_ui(ui);
        if let (Some((parent, name, kind)), Some(root), Some(index)) =
            (create_subtopic_req, root, index)
        {
            match create_subtopic(root, index, &parent, &name, kind) {
                Ok(()) => {
                    self.message = format!("added {name:?}");
                    // Without this the entity is written and nothing draws
                    // it: every view reads the shared index, which still
                    // predates the new node.
                    action = Some(MindmapAction::KnowledgeChanged);
                }
                Err(e) => self.message = format!("could not add subtopic: {e}"),
            }
        }
        if let (Some(index), true) = (index, self.showing_unsorted_inbox()) {
            if let Some(citekey) = self.unsorted_inbox_ui(ui, root, index) {
                action = Some(MindmapAction::SortPaper(citekey));
            }
        }
        if let Some(citekey) = literature_card_for {
            self.selected = Some(graph::paper_node(&citekey));
        }
        // #283: the Up button on the dotted line, or a double-click on the
        // purple card. `up` is `Some(None)` when the level above is the top.
        if go_up {
            if let Some(target) = up.clone() {
                self.set_current(target);
            }
        }
        if let Some(target) = remove_link {
            match (root, self.current.clone()) {
                (Some(root), Some(current)) => {
                    match crate::connections::remove(root, &current, &target) {
                        Ok(true) => {
                            self.message = "link removed".to_string();
                            action = Some(MindmapAction::KnowledgeChanged);
                        }
                        Ok(false) => self.message = "that link is already gone".to_string(),
                        Err(e) => self.message = format!("could not remove the link: {e}"),
                    }
                }
                _ => self.message = "no library open".to_string(),
            }
        }
        if let Some(id) = drilled {
            self.set_current(Some(id));
        }
        if let Some(citekey) = opened_paper {
            action = Some(MindmapAction::OpenPaper(citekey));
        }
        if let Some(citekey) = reclassify_for {
            action = Some(MindmapAction::SortPaper(citekey));
        }
        if open_setup {
            action = Some(MindmapAction::OpenSetup);
        }

        if let (Some(root), Some(index), Some(graph)) = (root, index, graph) {
            self.literature_card_ui(ui, root, index, graph);
        }

        action
    }

    /// The "new subtopic" dialog, shown while a subtopic draft is armed.
    ///
    /// A real window rather than an entry inside the right-click menu, for
    /// the focus reason recorded on [`subtopic_menu_item`]. Returns
    /// `Some((parent_path, name))` on the frame Create is pressed.
    /// The "Add hyperlink…" picker (#285).
    ///
    /// **The same fuzzy finder as everywhere else** (maintainer, 2026-09-23:
    /// "hyperlink addition ui should be the same fuzzyfinder"), over
    /// [`crate::autocomplete::library_candidates`] restricted to topics and
    /// projects — not [`crate::collection_picker::rank`], which sees only
    /// the user's own library. The maintainer's own example links a user
    /// topic to "the NJOY entry within the code corpus", and a corpus node
    /// is exactly what `rank` cannot offer.
    ///
    /// Returns `true` on the frame a link is recorded, so the caller
    /// rebuilds the shared knowledge and the new card is drawn at once
    /// rather than after a restart (#286).
    fn hyperlink_dialog_ui(
        &mut self,
        ui: &mut egui::Ui,
        root: Option<&KovanRoot>,
        index: Option<&KnowledgeIndex>,
    ) -> bool {
        use crate::autocomplete::{library_candidates, CandidateKind};
        let Some((source, query)) = self.hyperlink_draft.clone() else {
            return false;
        };
        let (Some(root), Some(index)) = (root, index) else {
            // No library open: nowhere to record it. Drop the draft rather
            // than leaving a dialog that cannot succeed.
            self.hyperlink_draft = None;
            return false;
        };

        let mut text = query;
        let mut close = false;
        let mut added = false;
        let source_label = crate::runtime_graph::concept(Some(index), &source)
            .map(|c| c.title)
            .unwrap_or_else(|| source.path.clone());

        egui::Window::new("Add hyperlink")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.label(format!("Link {source_label} to:"));
                let field = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .hint_text("search topics and projects\u{2026}")
                        .desired_width(360.0),
                );
                if !field.has_focus() && text.is_empty() {
                    field.request_focus();
                }
                ui.separator();
                let candidates = library_candidates(
                    root,
                    index,
                    &text,
                    &[CandidateKind::Topic, CandidateKind::Project],
                );
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        if candidates.is_empty() {
                            ui.weak("no matches");
                        }
                        for c in &candidates {
                            let Some(target) = candidate_node_id(&c.node) else {
                                continue;
                            };
                            if target == source {
                                continue; // a node cannot link to itself
                            }
                            if ui
                                .small_button(format!(
                                    "\u{1F517} {}  \u{2014}  {}",
                                    c.candidate.label, c.candidate.detail
                                ))
                                .clicked()
                            {
                                match crate::connections::add(root, &source, &target) {
                                    Ok(()) => {
                                        self.message = format!(
                                            "linked {source_label} to {}",
                                            c.candidate.label
                                        );
                                        added = true;
                                    }
                                    Err(e) => self.message = format!("could not link: {e}"),
                                }
                                close = true;
                            }
                        }
                    });
                ui.separator();
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });

        if close {
            self.hyperlink_draft = None;
        } else {
            self.hyperlink_draft = Some((source, text));
        }
        added
    }

    fn subtopic_dialog_ui(&mut self, ui: &mut egui::Ui) -> Option<(String, String, EntityKind)> {
        let Some((parent, text, kind)) = &mut self.subtopic_draft else {
            return None;
        };
        let parent = parent.clone();
        let kind = *kind;
        let mut request = None;
        let mut cancel = false;
        let under = if parent.is_empty() {
            "the top of your library".to_string()
        } else {
            parent.clone()
        };
        let what = match kind {
            EntityKind::Project => "project",
            _ => "subtopic",
        };
        egui::Window::new(format!("New {what}"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.label(format!("Under {under}:"));
                let field = ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text("name")
                        .desired_width(280.0),
                );
                // Focus it on the frame the dialog appears, so the name can
                // be typed without a click first.
                if !field.has_focus() && text.is_empty() {
                    field.request_focus();
                }
                let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    let named = !text.trim().is_empty();
                    if (ui.add_enabled(named, egui::Button::new("Create")).clicked()
                        || (submitted && named))
                        && named
                    {
                        request = Some((parent.clone(), text.clone(), kind));
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if request.is_some() || cancel {
            self.subtopic_draft = None;
        }
        request
    }

    /// The Rename / Move / Delete dialogs, and the transactional rewrite
    /// behind them ([`crate::concept_ops`]).
    ///
    /// Returns `true` on the frame an operation is applied, so the caller
    /// rebuilds the shared index.
    ///
    /// Each one **plans first**. A plan reads the whole library and writes
    /// nothing, so an operation that cannot be computed — a name that
    /// slugifies to nothing, a destination already taken, a move into a
    /// concept's own subtree — reports that with the library untouched. It
    /// also yields the counts the delete confirmation shows, which is the
    /// difference between "delete this" and "delete this, 3 sub-concepts and
    /// 12 references".
    fn concept_ops_ui(
        &mut self,
        ui: &mut egui::Ui,
        root: &KovanRoot,
        index: &KnowledgeIndex,
    ) -> bool {
        use crate::concept_ops::{apply, plan, ConceptOp};
        let mut done = false;
        let mut run = |me: &mut Self, path: &str, kind: EntityKind, op: ConceptOp| {
            match plan(root, index, path, kind, &op).and_then(|p| apply(&p).map(|()| p)) {
                Ok(p) => {
                    me.message = p.summary();
                    // Standing on a concept that has moved or gone would
                    // leave an empty star with no way back.
                    if me.current.as_ref().is_some_and(|c| c.path == path) {
                        me.set_current(None);
                    }
                    done = true;
                }
                Err(e) => me.message = format!("could not do that: {e}"),
            }
        };

        // ── Rename ──────────────────────────────────────────────────────
        if let Some((path, kind, name)) = self.rename_draft.clone() {
            let mut close = false;
            let mut submit = false;
            let mut text = name;
            egui::Window::new("Rename concept")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label(format!("New name for {path}:"));
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text("name")
                            .desired_width(280.0),
                    );
                    if !field.has_focus() && text.is_empty() {
                        field.request_focus();
                    }
                    let entered =
                        field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.weak(
                        "Everything filed under it, and every artifact that names it, is updated.",
                    );
                    ui.horizontal(|ui| {
                        let named = !text.trim().is_empty();
                        if (ui.add_enabled(named, egui::Button::new("Rename")).clicked()
                            || (entered && named))
                            && named
                        {
                            submit = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if submit {
                run(
                    self,
                    &path,
                    kind,
                    ConceptOp::Rename {
                        new_name: text.clone(),
                    },
                );
                self.rename_draft = None;
            } else if close {
                self.rename_draft = None;
            } else {
                self.rename_draft = Some((path, kind, text));
            }
        }

        // ── Move ────────────────────────────────────────────────────────
        if let Some((path, kind, query)) = self.move_draft.clone() {
            let mut close = false;
            let mut chosen: Option<String> = None;
            let mut text = query;
            egui::Window::new("Move concept")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label(format!("New parent for {path}:"));
                    ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text("search concepts\u{2026}")
                            .desired_width(340.0),
                    );
                    if ui.button("\u{2191} the top level").clicked() {
                        chosen = Some(String::new());
                    }
                    ui.separator();
                    let entity_kind = kind;
                    for c in crate::collection_picker::rank(index, entity_kind, &text, &[]) {
                        // Its own subtree is not a destination; `plan`
                        // refuses it anyway, but offering it would invite
                        // the error rather than prevent it.
                        if c.path == path || c.path.starts_with(&format!("{path}/")) {
                            continue;
                        }
                        if ui.small_button(format!("\u{1F4C1} {}", c.path)).clicked() {
                            chosen = Some(c.path.clone());
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            if let Some(new_parent) = chosen {
                run(self, &path, kind, ConceptOp::Move { new_parent });
                self.move_draft = None;
            } else if close {
                self.move_draft = None;
            } else {
                self.move_draft = Some((path, kind, text));
            }
        }

        // ── Delete, with the plan's own counts in the confirmation ──────
        if let Some((path, kind)) = self.delete_draft.clone() {
            let preview = plan(root, index, &path, kind, &ConceptOp::Delete);
            let mut confirm = false;
            let mut close = false;
            egui::Window::new("Delete concept")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete {path}?"));
                    match &preview {
                        Ok(p) => {
                            ui.weak(p.summary());
                            ui.weak("Papers left with no classification go back to Unsorted.");
                        }
                        Err(e) => {
                            ui.colored_label(ui.visuals().warn_fg_color, e.to_string());
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(preview.is_ok(), egui::Button::new("Delete"))
                            .clicked()
                        {
                            confirm = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if confirm {
                run(self, &path, kind, ConceptOp::Delete);
                self.delete_draft = None;
            } else if close {
                self.delete_draft = None;
            }
        }
        done
    }

    /// The "Add literature here" dialog: a fuzzy search over the library's
    /// papers, filing the chosen one under the armed concept.
    ///
    /// Returns `true` on the frame a paper is filed, so the caller can
    /// rebuild the shared index.
    ///
    /// Only the **user's own** papers are offered. Corpus literature is
    /// already filed by the corpus itself (`CorpusLiterature::topics`) and
    /// has no user-side `kovan.toml` to write a classification into — so
    /// offering it would be a control that cannot do anything.
    fn literature_dialog_ui(
        &mut self,
        ui: &mut egui::Ui,
        root: Option<&KovanRoot>,
        index: Option<&KnowledgeIndex>,
    ) -> bool {
        let (Some(root), Some(index)) = (root, index) else {
            return false;
        };
        let Some((concept, query)) = &mut self.literature_draft else {
            return false;
        };
        let concept = concept.clone();
        let mut chosen: Option<String> = None;
        let mut cancel = false;
        egui::Window::new("Add literature")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.label(format!("File a paper under {concept}:"));
                ui.add(
                    egui::TextEdit::singleline(query)
                        .hint_text("search your library\u{2026}")
                        .desired_width(360.0),
                );
                ui.separator();
                let hits = crate::autocomplete::library_candidates(
                    root,
                    index,
                    query,
                    &[crate::autocomplete::CandidateKind::Paper],
                );
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        let mut any = false;
                        for hit in &hits {
                            // Corpus entries come back from the same search; they
                            // are not ours to reclassify.
                            if hit.node.starts_with("corpus:") {
                                continue;
                            }
                            any = true;
                            if ui
                                .button(format!(
                                    "{} \u{2014} {}",
                                    hit.candidate.label, hit.candidate.detail
                                ))
                                .clicked()
                            {
                                chosen = Some(hit.candidate.insert_text.clone());
                            }
                        }
                        if !any {
                            ui.weak("no matching paper in your library");
                        }
                    });
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });

        let mut filed = false;
        if let Some(citekey) = chosen {
            // A project path files under `projects`, a topic path under
            // `topics`; a corpus path is a topic, which is also the fallback
            // for a path the index does not know.
            let as_project = index
                .collections
                .iter()
                .find(|c| c.path == concept)
                .is_some_and(|c| c.kind == EntityKind::Project);
            let dir = root.paper_dir(&citekey);
            match EntityConfig::load(&dir) {
                Ok(mut config) => {
                    let list = if as_project {
                        &mut config.classification.projects
                    } else {
                        &mut config.classification.topics
                    };
                    if !list.iter().any(|p| p == &concept) {
                        list.push(concept.clone());
                    }
                    // Filing it anywhere real takes it out of the inbox.
                    config
                        .classification
                        .topics
                        .retain(|t| t != crate::entity::UNSORTED);
                    match config.save(&dir) {
                        Ok(()) => {
                            self.message = format!("filed {citekey:?} under {concept:?}");
                            filed = true;
                        }
                        Err(e) => self.message = format!("could not file it: {e}"),
                    }
                }
                Err(e) => self.message = format!("could not read {citekey:?}: {e}"),
            }
            self.literature_draft = None;
        }
        if cancel {
            self.literature_draft = None;
        }
        filed
    }

    /// Whether the map has been drilled into the synthetic **Unsorted**
    /// concept, so the inbox is what the user is asking to see.
    fn showing_unsorted_inbox(&self) -> bool {
        self.current.as_ref().is_some_and(|id| {
            id.path == crate::runtime_graph::UNSORTED_PATH
                && id.namespace == crate::node_id::Namespace::Library
        })
    }

    /// The Unsorted inbox: **one box per unclassified paper**, each with its
    /// own right-click menu (#273).
    ///
    /// Returns the citekey whose "Sort into…" was chosen, if any.
    ///
    /// # Why papers appear here as boxes at all
    ///
    /// Epic #241 is explicit that the map shows *"concepts and projects
    /// only"*, and #245 removed papers as rows on that basis. **Unsorted is
    /// the deliberate exception**, and the exception is narrow: papers are
    /// boxes *inside this view*, never nodes in the concept graph.
    ///
    /// The reason is structural rather than cosmetic. Everywhere else a
    /// paper is reached as a *citation of the concept it is filed under* —
    /// but an unsorted paper is by definition filed under nothing, so that
    /// mechanism cannot reach it. Sorting a paper requires seeing the paper.
    /// Drilling into Unsorted and finding an empty ring, which is what
    /// happened before this, made the inbox look empty when it was not.
    ///
    /// The view empties itself: a sorted paper leaves on the next rebuild,
    /// and `runtime_graph::needs_unsorted` drops the whole card once the
    /// last one is gone.
    fn unsorted_inbox_ui(
        &self,
        ui: &mut egui::Ui,
        root: Option<&KovanRoot>,
        index: &KnowledgeIndex,
    ) -> Option<String> {
        let papers = index.papers_in(crate::runtime_graph::UNSORTED_PATH);
        let mut sort_me = None;
        egui::Window::new("Unsorted")
            .default_width(420.0)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .show(ui.ctx(), |ui| {
                if papers.is_empty() {
                    ui.weak("nothing unsorted \u{2014} everything is filed.");
                    return;
                }
                ui.weak(format!(
                    "{} paper(s) with no classification. Right-click one to file it.",
                    papers.len()
                ));
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for p in &papers {
                            // Its own box, so each paper is a distinct
                            // right-click target rather than a row in a list.
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let (title, author_year) = match root {
                                    Some(root) => bib_display(root, &p.citekey),
                                    None => (p.citekey.clone(), String::new()),
                                };
                                ui.strong(&title);
                                if !author_year.is_empty() {
                                    ui.weak(&author_year);
                                }
                                ui.weak(&p.citekey);
                                let resp = ui.interact(
                                    ui.min_rect(),
                                    ui.id().with(("unsorted-box", &p.citekey)),
                                    egui::Sense::click(),
                                );
                                resp.context_menu(|ui| {
                                    if ui.button("Sort into\u{2026}").clicked() {
                                        sort_me = Some(p.citekey.clone());
                                        ui.close();
                                    }
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });
            });
        sort_me
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

    /// #285: a hyperlink the user made shows as a removable link card, on
    /// both ends of the link, and points at the other concept.
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn a_concept_hyperlink_shows_as_a_removable_link_card_from_both_ends() {
        use crate::node_id::{Namespace, NodeId};
        let (_dir, root) = make_root();
        EntityConfig::topic("njoy", "NJOY")
            .save(&root.topics_dir().join("njoy"))
            .unwrap();
        let index = crate::index::KnowledgeIndex::rebuild(&root);
        let mine = NodeId::concept(Namespace::Library, "njoy");
        let corpus = NodeId::concept(Namespace::Corpus, crate::corpus::ROOT_TOPIC);
        crate::connections::add(&root, &mine, &corpus).unwrap();

        let mut cache = LinkCache::default();
        let here = cache.cards(&root, Some(&index), &mine);
        assert_eq!(here.len(), 1);
        assert_eq!(here[0].detail, "hyperlink");
        assert!(here[0].removable, "the user's own link can be removed here");
        assert!(matches!(&here[0].target, LinkTarget::Concept(id) if *id == corpus));

        // Persisted once, seen from both ends.
        let there = cache.cards(&root, Some(&index), &corpus);
        assert_eq!(there.len(), 1);
        assert!(matches!(&there[0].target, LinkTarget::Concept(id) if *id == mine));

        // And a concept with no links has none.
        assert!(cache
            .cards(
                &root,
                Some(&index),
                &NodeId::concept(Namespace::Library, "elsewhere")
            )
            .is_empty());
    }

    /// #286: an artifact connected to a topic in the PDF reader shows on
    /// that topic's star, and opening it opens the paper.
    ///
    /// The relation names the topic by its **library** path, which is what
    /// the reader writes; the star is standing on the node the map shows.
    /// Both go through `canonical_concept`, so a mirrored library path and
    /// the corpus node it stands for are the same place — without that the
    /// card silently fails to appear.
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn a_relation_from_a_paper_shows_on_the_concept_it_points_at() {
        use crate::node_id::{Namespace, NodeId};
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
        let mut paper = crate::session::PaperSession::open(&root, "wang2018multiphysics").unwrap();
        paper.append_block(
            "# Conduction Coefficient\n\n```toml\n[kovan]\nid = \"conduction-coeff\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n",
        );
        paper.save_document().unwrap();
        let index = crate::index::KnowledgeIndex::rebuild(&root);

        crate::relation::add_connection(
            &root,
            &crate::graph::artifact_node("wang2018multiphysics", "conduction-coeff"),
            &crate::graph::collection_node("htgrs"),
            crate::relation::RelationKind::RelatedTo,
        )
        .unwrap();

        let mut cache = LinkCache::default();
        let cards = cache.cards(
            &root,
            Some(&index),
            &NodeId::concept(Namespace::Library, "htgrs"),
        );
        assert_eq!(cards.len(), 1, "the linked artifact is missing: {cards:?}");
        assert!(
            cards[0].label.contains("conduction-coeff"),
            "{}",
            cards[0].label
        );
        assert!(
            !cards[0].removable,
            "a relation belongs to its paper, not to the map"
        );
        assert!(matches!(
            &cards[0].target,
            LinkTarget::Paper(k) if k == "wang2018multiphysics"
        ));
    }

    /// The mirror case the canonicalisation is actually for: the reader
    /// records the connection against the **library** path of a topic that
    /// mirrors a corpus one, while the map has you standing on the **corpus**
    /// node. They are the same place, and the card has to appear there.
    ///
    /// Both directions are asserted because each is carried by a *different*
    /// half of the canonicalisation, which a first version of this test got
    /// wrong: canonicalising the **relation's end** is what makes the card
    /// appear on the corpus node, and canonicalising **`current`** is what
    /// makes it appear when you are standing on the library mirror instead.
    /// Each half was checked capable of failing by removing it. The failure
    /// is silent on screen either way — the card simply is not drawn.
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn a_relation_to_a_mirrored_library_topic_shows_on_its_corpus_node() {
        use crate::node_id::{Namespace, NodeId};
        let (_dir, root) = make_root();
        // A corpus topic path, mirrored into the user's library — the shape
        // `runtime_graph::is_corpus_mirror` is about.
        let mirrored = crate::corpus::TOPICS[1].path;
        assert!(crate::runtime_graph::is_corpus_mirror(mirrored));
        let mut dir = root.topics_dir();
        for segment in mirrored.split('/') {
            dir = dir.join(segment);
            EntityConfig::topic(segment, segment).save(&dir).unwrap();
        }
        EntityConfig::paper(CiteKey::parse("lee2020corrosion").unwrap(), Access::Open)
            .save_paper(&root.paper_dir("lee2020corrosion"))
            .unwrap();
        let mut paper = crate::session::PaperSession::open(&root, "lee2020corrosion").unwrap();
        paper.append_block(
            "# A Note\n\n```toml\n[kovan]\nid = \"a-note\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n",
        );
        paper.save_document().unwrap();
        let index = crate::index::KnowledgeIndex::rebuild(&root);

        crate::relation::add_connection(
            &root,
            &crate::graph::artifact_node("lee2020corrosion", "a-note"),
            &crate::graph::collection_node(mirrored),
            crate::relation::RelationKind::RelatedTo,
        )
        .unwrap();

        let mut cache = LinkCache::default();
        let on_corpus_node = cache.cards(
            &root,
            Some(&index),
            &NodeId::concept(Namespace::Corpus, mirrored),
        );
        assert_eq!(
            on_corpus_node.len(),
            1,
            "the link made against the library mirror is invisible on the corpus node"
        );
        assert!(on_corpus_node[0].label.contains("a-note"));

        // And the other way round: standing on the library mirror itself.
        let on_library_mirror = cache.cards(
            &root,
            Some(&index),
            &NodeId::concept(Namespace::Library, mirrored),
        );
        assert_eq!(
            on_library_mirror.len(),
            1,
            "the same link is invisible from the mirror node it was made against"
        );
    }

    /// The finder hands back two identity syntaxes; both have to be readable
    /// or "Add hyperlink…" silently skips half its own candidates (#285).
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn a_candidates_node_id_is_read_in_either_syntax() {
        use crate::node_id::{Namespace, NodeId};
        assert_eq!(
            candidate_node_id("collection:htgrs/fuel"),
            Some(NodeId::concept(Namespace::Library, "htgrs/fuel"))
        );
        assert_eq!(
            candidate_node_id("corpus:concept/nuclear-data"),
            Some(NodeId::concept(Namespace::Corpus, "nuclear-data"))
        );
        assert_eq!(candidate_node_id("nonsense"), None);
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

    /// ~~`delete_concept_removes_an_empty_one_and_refuses_a_populated_one`~~
    /// and ~~`renaming_a_concept_keeps_its_path`~~ **REMOVED 2026-09-23.**
    /// They pinned the deliberately limited CRUD this module used to
    /// implement itself — a rename that changed only the display name, and a
    /// delete that refused a populated concept. Both limits are gone now
    /// that `crate::concept_ops` does the transactional rewrite, and that
    /// module's own tests cover rename, move and delete against a library
    /// with papers and artifacts referring to the paths involved.
    /// A subtopic added under a **corpus** topic appears as a light-green
    /// user concept under it, and does not also appear as a second top-level
    /// branch (#274; maintainer 2026-09-22: "i should see a light green node
    /// popping out and linked").
    ///
    /// Three things have to hold together for that, and each failed on its
    /// own at some point: the ancestor path must be materialised as entities
    /// or `scan_collections` never reaches the new node; `children` must ask
    /// the index as well as the corpus; and `top_level` must not list the
    /// mirrored ancestors as concepts of their own.
    #[test]
    #[cfg(all(feature = "gui", not(target_os = "android")))]
    fn a_subtopic_under_a_corpus_topic_shows_beneath_it() {
        use crate::node_id::{Namespace, NodeId};
        use crate::runtime_graph::{children, top_level, ConceptKind};

        let (_dir, root) = make_root();
        let parent = "nuclear-engineering/fuel-and-materials/triso";
        assert!(
            crate::corpus::topic_at(parent).is_some(),
            "fixture assumes this corpus topic exists"
        );

        let index = KnowledgeIndex::rebuild(&root);
        create_subtopic(&root, &index, parent, "My TRISO Notes", EntityKind::Topic).unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        let under = children(
            Some(&index),
            Some(&NodeId::concept(Namespace::Corpus, parent)),
        );
        let mine = under
            .iter()
            .find(|c| c.title == "My TRISO Notes")
            .expect("the new subtopic is drawn under its corpus parent");
        assert_eq!(
            mine.kind,
            ConceptKind::Topic,
            "a user topic (light green), not a corpus one"
        );
        assert_eq!(mine.id.namespace, Namespace::Library);

        // The mirrored ancestors exist on disk so the tree is walkable, but
        // they are scaffolding — the corpus node already represents them, at
        // *every* level. The maintainer saw both halves of this fail:
        // "there is a TRISO (corpus) and triso (topic)" and "Fuel & Materials
        // now has a fuel-and-materials".
        let tops: Vec<String> = top_level(Some(&index))
            .into_iter()
            .map(|c| c.title)
            .collect();
        assert!(
            !tops.iter().any(|t| t == "nuclear-engineering"),
            "the mirrored corpus path must not become its own branch: {tops:?}"
        );

        for (parent, twin) in [
            ("nuclear-engineering", "fuel-and-materials"),
            ("nuclear-engineering/fuel-and-materials", "triso"),
        ] {
            let kids: Vec<String> = children(
                Some(&index),
                Some(&NodeId::concept(Namespace::Corpus, parent)),
            )
            .into_iter()
            .map(|c| c.title)
            .collect();
            assert!(
                !kids.iter().any(|t| t == twin),
                "{parent} shows a slugified twin {twin:?} beside its corpus card: {kids:?}"
            );
        }
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

    /// The **caller** chooses what is created, and it lands in that kind's
    /// own tree.
    ///
    /// ~~`create_subtopic_matches_the_parents_kind`~~ **CHANGED 2026-09-22**:
    /// the kind used to be inferred from the parent, so a project could only
    /// ever be born under another project. The menu now offers "Add project
    /// here…" beside "Add subtopic here…" (maintainer), which means a project
    /// can be started under a topic — including under a **corpus** topic,
    /// whose kind is not a user kind at all and which the old inference
    /// silently read as `Topic`.
    #[test]
    fn create_subtopic_puts_each_kind_in_its_own_tree() {
        let (_dir, root) = make_root();
        EntityConfig::project("outram-park", "Outram Park")
            .save(&root.projects_dir().join("outram-park"))
            .unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        create_subtopic(
            &root,
            &index,
            "outram-park",
            "Sub Effort",
            EntityKind::Project,
        )
        .unwrap();
        assert!(EntityConfig::is_entity(
            &root.projects_dir().join("outram-park").join("sub-effort")
        ));

        create_subtopic(&root, &index, "", "New Topic", EntityKind::Topic).unwrap();
        assert!(EntityConfig::is_entity(
            &root.topics_dir().join("new-topic")
        ));

        // A project started under a topic: the new entity goes in the
        // projects tree at the mirrored path, not beside the topic.
        create_subtopic(
            &root,
            &index,
            "new-topic",
            "Side Quest",
            EntityKind::Project,
        )
        .unwrap();
        assert!(EntityConfig::is_entity(
            &root.projects_dir().join("new-topic").join("side-quest")
        ));
    }

    #[test]
    fn create_subtopic_rejects_an_empty_name() {
        let (_dir, root) = make_root();
        let index = KnowledgeIndex::rebuild(&root);
        assert!(create_subtopic(&root, &index, "", "   ", EntityKind::Topic).is_err());
    }
}
