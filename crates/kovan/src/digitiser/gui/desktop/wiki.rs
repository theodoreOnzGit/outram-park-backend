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

use crate::entity::{Access, Classification, EntityConfig, EntityKind};
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
        Self { preview, citekey, access: Access::Restricted, topics_text: String::new(), projects_text: String::new(), message }
    }
}

fn split_paths(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

/// A pending "sort this paper" flow (op-j3ib, GH issue #35's 2026-09-01
/// 05:33 "if i right click the literature, i want to be able to sort it") —
/// opened by right-clicking a paper link, prefilled from its current
/// classification.
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
        Self { citekey, topics_text, projects_text, message: String::new() }
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
    /// Slash-separated path of the collection currently drilled into; `""`
    /// is both tree roots shown together.
    current: String,
    ingest_flow: Option<IngestFlow>,
    classify_flow: Option<ClassifyFlow>,
}

impl Default for WikiState {
    fn default() -> Self {
        Self { current: String::new(), ingest_flow: None, classify_flow: None }
    }
}

impl WikiState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A PDF was picked (from the "+ Ingest Literature…" button's dialog) —
    /// run §22's automatic-detection preview and open the classification
    /// form. On failure (an unreadable PDF), returns the error message for
    /// the caller's own status line — there is no form to attach it to yet.
    pub fn begin_ingest(&mut self, root: &KovanRoot, pdf_path: &std::path::Path) -> Result<(), String> {
        let preview = ingest::preview(root, pdf_path).map_err(|e| e.to_string())?;
        self.ingest_flow = Some(IngestFlow::new(preview));
        Ok(())
    }

    /// Draw the ingest form, if one is open. Returns the citekey the paper
    /// was ingested under, the frame ingestion succeeds — the caller
    /// activates it immediately (op-sr4n.2: "Ingest & Open" must actually
    /// open, not just refresh the index) and refreshes the shared knowledge
    /// state (op-dkll).
    fn ingest_form(&mut self, ui: &mut egui::Ui, root: &KovanRoot) -> Option<String> {
        let Some(flow) = &mut self.ingest_flow else { return None };
        let mut close = false;
        let mut confirmed = None;

        egui::Window::new("Ingest Literature").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
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
            ui.radio_value(&mut flow.access, Access::Restricted, "Restricted / proprietary");
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
        let Some(flow) = &mut self.classify_flow else { return false };
        let mut close = false;
        let mut save_clicked = false;

        egui::Window::new(format!("Sort {}", flow.citekey)).collapsible(false).resizable(false).show(ui.ctx(), |ui| {
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
    pub fn ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot, index: &KnowledgeIndex) -> Option<WikiAction> {
        let mut action = None;

        if let Some(citekey) = self.ingest_form(ui, root) {
            action = Some(WikiAction::OpenPaper(citekey));
        }
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

        // Breadcrumb.
        ui.horizontal_wrapped(|ui| {
            let mut go_to = None;
            if ui.link("Wiki").clicked() {
                go_to = Some(String::new());
            }
            let mut acc = String::new();
            for part in self.current.split('/').filter(|s| !s.is_empty()) {
                ui.label(">");
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(part);
                if ui.link(part).clicked() {
                    go_to = Some(acc.clone());
                }
            }
            if let Some(path) = go_to {
                self.current = path;
            }
        });
        ui.add_space(8.0);

        // op-sr4n.4: a freshly ingested paper defaults to
        // `Classification::unsorted()` (a single topic path literally named
        // "unsorted", entity.rs's `UNSORTED` constant) but no
        // `topics/unsorted/kovan.toml` directory is ever created for it — so
        // without this, `children_of("")` never surfaces it and the paper is
        // invisible in the Wiki, violating "a paper must never disappear
        // merely because it has not been classified yet". A synthetic entry
        // at the tree root, shown only while there is something to show,
        // makes the existing `current = path` / `papers_in(path)` drill-down
        // machinery reach it with no new collection type or on-disk directory.
        let unsorted_count = if self.current.is_empty() { index.papers_in("unsorted").len() } else { 0 };
        let mut open_paper = None;
        let mut classify_target = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let children: Vec<(String, EntityKind, String)> =
                index.children_of(&self.current).into_iter().map(|c| (c.path.clone(), c.kind, c.name.clone())).collect();
            let papers: Vec<String> = index.papers_in(&self.current).into_iter().map(|p| p.citekey.clone()).collect();

            if children.is_empty() && papers.is_empty() && unsorted_count == 0 && self.current.is_empty() {
                ui.weak("(no topics or projects yet — use + Ingest Literature to get started)");
            }

            let mut drill_into = None;
            if unsorted_count > 0 {
                if ui.link(format!("\u{1F4E5} Unsorted ({unsorted_count})")).clicked() {
                    drill_into = Some("unsorted".to_string());
                }
            }
            for (path, kind, name) in &children {
                let icon = match kind {
                    EntityKind::Topic => "\u{1F4C1}",
                    EntityKind::Project => "\u{1F4E6}",
                    EntityKind::Paper => "",
                };
                if ui.link(format!("{icon} {name}")).clicked() {
                    drill_into = Some(path.clone());
                }
            }
            if let Some(path) = drill_into {
                self.current = path;
            }

            if !papers.is_empty() {
                ui.add_space(8.0);
                ui.label("Papers");
                for citekey in &papers {
                    // op-sr4n.2: clicking a paper activates it (GH issue #35's
                    // "unify root and active-paper context" comment).
                    // op-j3ib: right-clicking it opens the "sort" form.
                    let resp = ui.link(format!("\u{1F4C4} {citekey}"));
                    if resp.clicked() {
                        open_paper = Some(citekey.clone());
                    }
                    if resp.secondary_clicked() {
                        classify_target = Some(citekey.clone());
                    }
                }
            }
        });

        if let Some(citekey) = classify_target {
            self.classify_flow = Some(ClassifyFlow::new(citekey, index));
        }
        if let Some(citekey) = open_paper {
            action = Some(WikiAction::OpenPaper(citekey));
        }
        action
    }
}
