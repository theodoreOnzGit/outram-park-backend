//! The Wiki/collection home (op-9vo6.8, GitHub issue #35 §8): what a user
//! lands on after opening a root, in place of the PDF reader.
//!
//! Deliberately a plain hierarchical browser, not the interactive mindmap —
//! §45 lists "fancy mindmap physics before the underlying model works" as
//! an explicit non-goal, and `op-9vo6.21` builds the mindmap on top of this
//! same [`KnowledgeIndex`] later. This screen only ever shows one
//! collection's direct children plus its directly classified papers
//! ([`KnowledgeIndex::children_of`]/[`papers_in`](KnowledgeIndex::papers_in)
//! are one level deep by construction), which is what keeps §8's own
//! constraint — "must not render thousands of paper nodes at root level" —
//! true regardless of library size.
//!
//! Also hosts the "+ Ingest Literature…" flow (op-9vo6.9, §22-23): the
//! automatic-detection/classification-prompt UI over [`crate::ingest`]'s
//! `preview`/`ingest` functions. It lives here rather than a separate view
//! because §22 frames ingestion as something launched *from* the Wiki, not
//! a standalone tab (§25: "Digitisers are contextual tools launched from
//! Research" — same idea, applied to ingestion).
//!
//! # No private copy of the knowledge state (op-dkll)
//!
//! This module used to own its own [`KnowledgeIndex`], reloaded on
//! construction and refreshed after every ingest/classify — a second,
//! independent database from Mindmap's own per-frame reload, per GH issue
//! #35's 2026-09-01 checkpoint (§8: "Wiki and Mindmap should not behave as
//! independent databases... avoid separately loading stale caches in
//! different views when a single application-level state can provide
//! consistency"). [`WikiState`] now takes the index as a `&KnowledgeIndex`
//! parameter on every call instead of owning one — [`WikiAction::OpenPaper`]
//! (a successful ingest) and [`WikiAction::KnowledgeChanged`] (a successful
//! reclassify) are how it tells `DigitiseApp` a shared rebuild is due;
//! `DigitiseApp` owns the single rebuild, not this module.

use eframe::egui::{self, Color32};

use crate::entity::{Access, Classification, EntityConfig};
use crate::index::KnowledgeIndex;
use crate::ingest::{self, IngestChoice, IngestPreview};
use crate::root::KovanRoot;

/// A pending "+ Ingest Literature…" flow: a PDF has been picked and
/// previewed; the user is filling in SOURCE/TOPICS/PROJECTS before
/// confirming.
struct IngestFlow {
    preview: IngestPreview,
    citekey: String,
    access: Access,
    /// Comma-separated, matching §16's slash-path syntax per entry — full
    /// fuzzy `+ New Topic` completion is `op-9vo6.16`'s job; a text field is
    /// enough for this pass to actually classify something.
    topics_text: String,
    projects_text: String,
    message: String,
}

impl IngestFlow {
    fn new(preview: IngestPreview) -> Self {
        let citekey = preview.suggested_citekey.clone();
        let message = if preview.already_exists {
            format!("a paper with citekey {citekey:?} already exists — choose a different citekey")
        } else {
            String::new()
        };
        Self {
            preview,
            citekey,
            access: Access::Restricted,
            topics_text: String::new(),
            projects_text: String::new(),
            message,
        }
    }
}

fn split_paths(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// A pending "sort this paper" flow (op-j3ib, GH issue #35's 2026-09-01
/// 05:33 "if i right click the literature, i want to be able to sort it") —
/// ~~opened by right-clicking a paper link~~ opened from "Reclassify…" on a
/// citation in a concept's right-click menu (papers stopped being rows on
/// 2026-09-22, #245), prefilled from its current classification.
struct ClassifyFlow {
    citekey: String,
    topics_text: String,
    projects_text: String,
    message: String,
}

impl ClassifyFlow {
    fn new(citekey: String, index: &KnowledgeIndex) -> Self {
        let (topics_text, projects_text) = index
            .papers
            .iter()
            .find(|p| p.citekey == citekey)
            .map(|p| (p.topics.join(", "), p.projects.join(", ")))
            .unwrap_or_default();
        Self {
            citekey,
            topics_text,
            projects_text,
            message: String::new(),
        }
    }
}

/// What the caller (`DigitiseApp`) should do after [`WikiState::ui`] returns.
pub enum WikiAction {
    /// A file picker for a PDF to ingest should open.
    RequestIngestDialog,
    /// A paper was clicked (or "Ingest & Open" just finished) — the caller
    /// should activate it (op-sr4n, GitHub issue #35's 2026-09-01 "unify
    /// root and active-paper context" comment). Ingesting also changes the
    /// shared knowledge state, same as [`Self::KnowledgeChanged`] — the
    /// caller should refresh it here too, not only navigate.
    OpenPaper(String),
    /// A reclassify (op-j3ib) succeeded — the caller should rebuild the
    /// shared `KnowledgeIndex`/`KnowledgeGraph` (op-dkll) before the next
    /// frame renders Wiki/Mindmap/Bibliography against it.
    KnowledgeChanged,
}

pub struct WikiState {
    /// The concept being shown, from the built-in corpus or the user's
    /// library (#249); `None` is the top, where the corpus root and the
    /// user's own top-level concepts are listed together.
    current: Option<crate::node_id::NodeId>,
    ingest_flow: Option<IngestFlow>,
    classify_flow: Option<ClassifyFlow>,
    /// Parsed bibliography for the citation lists (#245), re-read only when
    /// the file changes.
    bib: crate::mindmap::BibCache,
}

impl Default for WikiState {
    fn default() -> Self {
        Self {
            current: None,
            ingest_flow: None,
            classify_flow: None,
            bib: crate::mindmap::BibCache::default(),
        }
    }
}

impl WikiState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The concept being shown (`None` is the top). Read by the app's
    /// back/forward history (#242).
    pub(crate) fn current(&self) -> Option<&crate::node_id::NodeId> {
        self.current.as_ref()
    }

    /// Show `concept`: how back/forward, and the Mindmap sharing its
    /// location with this view, move the Wiki (#242).
    pub(crate) fn set_current(&mut self, concept: Option<crate::node_id::NodeId>) {
        self.current = concept;
    }

    /// A PDF was picked (from the "+ Ingest Literature…" button's dialog) —
    /// run §22's automatic-detection preview and open the classification
    /// form. On failure (an unreadable PDF), returns the error message for
    /// the caller's own status line — there is no form to attach it to yet.
    pub fn begin_ingest(
        &mut self,
        root: &KovanRoot,
        pdf_path: &std::path::Path,
    ) -> Result<(), String> {
        let preview = ingest::preview(root, pdf_path).map_err(|e| e.to_string())?;
        self.ingest_flow = Some(IngestFlow::new(preview));
        Ok(())
    }

    /// Draw the ingest form, if one is open. Returns the citekey the paper
    /// was ingested under, the frame ingestion succeeds — the caller
    /// activates it immediately (op-sr4n.2: "Ingest & Open" must actually
    /// open, not just refresh the index) and refreshes the shared knowledge
    /// state (op-dkll).
    pub(super) fn ingest_form(&mut self, ui: &mut egui::Ui, root: &KovanRoot) -> Option<String> {
        let Some(flow) = &mut self.ingest_flow else {
            return None;
        };
        let mut close = false;
        let mut confirmed = None;

        egui::Window::new("Ingest Literature")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Title: {}", flow.preview.title));
                if !flow.preview.authors.is_empty() {
                    ui.label(format!("Authors: {}", flow.preview.authors));
                }
                if let Some(year) = flow.preview.year {
                    ui.label(format!("Year: {year}"));
                }
                if let Some(doi) = &flow.preview.doi {
                    ui.label(format!("DOI: {doi}"));
                }
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Citekey:");
                    ui.text_edit_singleline(&mut flow.citekey);
                });

                ui.label("SOURCE");
                ui.radio_value(
                    &mut flow.access,
                    Access::Restricted,
                    "Restricted / proprietary",
                );
                ui.radio_value(&mut flow.access, Access::Open, "Open / redistributable");

                ui.horizontal(|ui| {
                    ui.label("Topics (comma-separated, e.g. htgrs/materials):");
                    ui.text_edit_singleline(&mut flow.topics_text);
                });
                ui.horizontal(|ui| {
                    ui.label("Projects:");
                    ui.text_edit_singleline(&mut flow.projects_text);
                });

                if !flow.message.is_empty() {
                    ui.colored_label(Color32::from_rgb(220, 90, 90), &flow.message);
                }

                ui.horizontal(|ui| {
                    if ui.button("Ingest & Open").clicked() {
                        confirmed = Some(());
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });

        let mut opened = None;
        if let Some(()) = confirmed {
            let citekey = flow.citekey.clone();
            let choice = IngestChoice {
                citekey: citekey.clone(),
                access: flow.access,
                topics: split_paths(&flow.topics_text),
                projects: split_paths(&flow.projects_text),
            };
            match ingest::ingest(root, &flow.preview, choice) {
                Ok(()) => {
                    opened = Some(citekey);
                    close = true;
                }
                Err(e) => flow.message = e.to_string(),
            }
        }
        if close {
            self.ingest_flow = None;
        }
        opened
    }

    /// Draw the "sort this paper" form, if one is open (op-j3ib). Persists
    /// straight to the paper's own `kovan.toml` via
    /// [`EntityConfig::load`]/[`EntityConfig::save`] — no new API, the same
    /// pair `ingest.rs`'s own paper-creation path uses. Leaving both fields
    /// empty puts the paper back in the Unsorted inbox (§7's
    /// `EntityConfig::validate` rejects an empty classification outright,
    /// so this is the friendly equivalent rather than surfacing that as an
    /// error) — same fallback ingestion itself already applies. Returns
    /// `true` the frame a reclassify actually succeeds, so the caller knows
    /// to refresh the shared knowledge state (op-dkll).
    fn classify_form(&mut self, ui: &mut egui::Ui, root: &KovanRoot) -> bool {
        let Some(flow) = &mut self.classify_flow else {
            return false;
        };
        let mut close = false;
        let mut save_clicked = false;

        egui::Window::new(format!("Sort {}", flow.citekey))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Topics (comma-separated, e.g. htgrs/materials):");
                    ui.text_edit_singleline(&mut flow.topics_text);
                });
                ui.horizontal(|ui| {
                    ui.label("Projects:");
                    ui.text_edit_singleline(&mut flow.projects_text);
                });
                ui.small("Leave both empty to put it back in Unsorted.");

                if !flow.message.is_empty() {
                    ui.colored_label(Color32::from_rgb(220, 90, 90), &flow.message);
                }

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save_clicked = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });

        let mut changed = false;
        if save_clicked {
            let dir = root.paper_dir(&flow.citekey);
            let topics = split_paths(&flow.topics_text);
            let projects = split_paths(&flow.projects_text);
            // op-8aq6: create whatever topic/project entities don't exist
            // yet before writing a classification that names them — same
            // fix as ingestion's own, since this form writes the identical
            // kind of dangling-path classification if skipped.
            match crate::entity::ensure_classification_paths(root, &topics, &projects) {
                Ok(()) => {
                    let classification = if topics.is_empty() && projects.is_empty() {
                        Classification::unsorted()
                    } else {
                        Classification { topics, projects }
                    };
                    let result = EntityConfig::load(&dir).map(|mut config| {
                        config.classification = classification;
                        config
                    });
                    match result.and_then(|config| config.save(&dir)) {
                        Ok(()) => {
                            changed = true;
                            close = true;
                        }
                        Err(e) => flow.message = e.to_string(),
                    }
                }
                Err(e) => flow.message = e.to_string(),
            }
        }
        if close {
            self.classify_flow = None;
        }
        changed
    }

    /// Draw the Wiki browser against `index` (the shared `KnowledgeIndex`,
    /// op-dkll — this module no longer keeps its own copy). Returns `Some`
    /// when the caller should act: open a file picker, activate a paper, or
    /// refresh the shared knowledge state.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        root: &KovanRoot,
        index: &KnowledgeIndex,
    ) -> Option<WikiAction> {
        let mut action = None;

        // The ingest form is drawn by the app, over every tab
        // (`DigitiseApp::ingest_form_ui`): started from the PDF reader's
        // ingest prompt it must show there, not only in the Wiki (2026-09-22).
        if self.classify_form(ui, root) {
            action = Some(WikiAction::KnowledgeChanged);
        }

        ui.horizontal(|ui| {
            ui.heading(&root.config().library.name);
            if ui.button("+ Ingest Literature…").clicked() {
                action = Some(WikiAction::RequestIngestDialog);
            }
        });
        ui.separator();

        // Breadcrumb, through the runtime graph (corpus and library, #249).
        let index_opt = Some(index);
        ui.horizontal_wrapped(|ui| {
            super::navigation_style(ui);
            let mut go_to = None;
            if ui.link("Top").clicked() {
                go_to = Some(None);
            }
            if let Some(cur) = &self.current {
                for (id, title) in crate::runtime_graph::breadcrumb(index_opt, cur) {
                    ui.label(">");
                    if ui.link(title).clicked() {
                        go_to = Some(Some(id));
                    }
                }
            }
            if let Some(to) = go_to {
                self.current = to;
            }
        });
        ui.add_space(8.0);

        // Concepts only, as on the Mindmap (maintainer direction, 2026-09-22,
        // #245): papers are not rows. A concept's citations drop down on
        // hover and are actionable from its right-click menu. The concept
        // list comes from the runtime graph, which also supplies the
        // synthetic "Unsorted" concept at the top so an unclassified paper
        // never disappears (op-sr4n.4).
        let mut open_paper = None;
        let mut classify_target = None;
        let entries = self.bib.entries(root).clone();
        let mut drill_into = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let children = crate::runtime_graph::children(index_opt, self.current.as_ref());
            let here = self
                .current
                .as_ref()
                .map(|id| crate::runtime_graph::citations(index_opt, &entries, id))
                .unwrap_or_default();

            if self.current.is_none() && children.len() <= 1 {
                ui.weak("(your library has no topics or projects yet — use + Ingest Literature to get started)");
            }

            // The concept you are on: its own citations, as one hoverable,
            // right-clickable line.
            if !here.is_empty() {
                let title = self
                    .current
                    .as_ref()
                    .and_then(|id| crate::runtime_graph::concept(index_opt, id))
                    .map(|c| c.title)
                    .unwrap_or_default();
                let resp = ui
                    .add(
                        egui::Label::new(format!("\u{1F4C4} {} citation(s) here", here.len()))
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_ui(|ui| crate::mindmap::citations_hover(ui, &title, &here));
                resp.context_menu(|ui| {
                    pick_citation(ui, &here, &mut open_paper, &mut classify_target);
                });
                ui.add_space(6.0);
            }

            for c in &children {
                use crate::runtime_graph::ConceptKind;
                let cites = crate::runtime_graph::citations(index_opt, &entries, &c.id);
                let icon = match c.kind {
                    ConceptKind::CorpusTopic => "\u{1F4DA}",
                    ConceptKind::Topic => "\u{1F4C1}",
                    ConceptKind::Project => "\u{1F4E6}",
                    ConceptKind::Unsorted => "\u{1F4E5}",
                };
                let label = format!("{icon} {}", c.title);
                let mut text = format!("{label}   \u{1F4C4} {}", cites.len());
                if c.sub_concepts > 0 {
                    text.push_str(&format!("   \u{2937} {}", c.sub_concepts));
                }
                let resp = ui
                    .link(text)
                    .on_hover_ui(|ui| crate::mindmap::citations_hover(ui, &label, &cites));
                if resp.clicked() {
                    drill_into = Some(c.id.clone());
                }
                resp.context_menu(|ui| {
                    ui.strong(&label);
                    if c.kind == ConceptKind::CorpusTopic {
                        ui.weak("built-in corpus (read-only)");
                    }
                    ui.separator();
                    pick_citation(ui, &cites, &mut open_paper, &mut classify_target);
                    ui.separator();
                    if ui.button("Go here").clicked() {
                        drill_into = Some(c.id.clone());
                        ui.close();
                    }
                });
            }
        });
        if let Some(id) = drill_into {
            self.current = Some(id);
        }

        if let Some(citekey) = classify_target {
            self.classify_flow = Some(ClassifyFlow::new(citekey, index));
        }
        if let Some(citekey) = open_paper {
            action = Some(WikiAction::OpenPaper(citekey));
        }
        action
    }
}

/// A concept's citation menu on the Wiki: each citation opens the paper or
/// reclassifies it (the right-click-a-paper action from when papers were
/// rows, op-j3ib), writing the choice into `open` or `classify`.
fn pick_citation(
    ui: &mut egui::Ui,
    citations: &[crate::mindmap::Citation],
    open: &mut Option<String>,
    classify: &mut Option<String>,
) {
    match crate::mindmap::citations_menu(ui, citations, "Reclassify…") {
        Some(crate::mindmap::CitationPick::Open(k)) => *open = Some(k),
        Some(crate::mindmap::CitationPick::Secondary(k)) => *classify = Some(k),
        None => {}
    }
}
