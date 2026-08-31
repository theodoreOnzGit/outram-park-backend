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

use eframe::egui::{self, Color32};

use crate::entity::{Access, EntityKind};
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

/// What the caller (`DigitiseApp`) should do after [`WikiState::ui`] returns.
pub enum WikiAction {
    /// A file picker for a PDF to ingest should open.
    RequestIngestDialog,
}

pub struct WikiState {
    index: KnowledgeIndex,
    /// Slash-separated path of the collection currently drilled into; `""`
    /// is both tree roots shown together.
    current: String,
    ingest_flow: Option<IngestFlow>,
}

impl WikiState {
    pub fn new(root: &KovanRoot) -> Self {
        Self { index: KnowledgeIndex::load_or_rebuild(root), current: String::new(), ingest_flow: None }
    }

    fn refresh(&mut self, root: &KovanRoot) {
        self.index = KnowledgeIndex::rebuild(root);
        let _ = self.index.save_cache(root);
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

    fn ingest_form(&mut self, ui: &mut egui::Ui, root: &KovanRoot) {
        let Some(flow) = &mut self.ingest_flow else { return };
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

        if let Some(()) = confirmed {
            let choice = IngestChoice {
                citekey: flow.citekey.clone(),
                access: flow.access,
                topics: split_paths(&flow.topics_text),
                projects: split_paths(&flow.projects_text),
            };
            match ingest::ingest(root, &flow.preview, choice) {
                Ok(()) => {
                    self.refresh(root);
                    close = true;
                }
                Err(e) => flow.message = e.to_string(),
            }
        }
        if close {
            self.ingest_flow = None;
        }
    }

    /// Draw the Wiki browser. Returns `Some` when the "+ Ingest
    /// Literature…" button was clicked and a file picker should open.
    pub fn ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot) -> Option<WikiAction> {
        let mut action = None;

        self.ingest_form(ui, root);

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

        egui::ScrollArea::vertical().show(ui, |ui| {
            let children: Vec<(String, EntityKind, String)> =
                self.index.children_of(&self.current).into_iter().map(|c| (c.path.clone(), c.kind, c.name.clone())).collect();
            let papers: Vec<String> = self.index.papers_in(&self.current).into_iter().map(|p| p.citekey.clone()).collect();

            if children.is_empty() && papers.is_empty() && self.current.is_empty() {
                ui.weak("(no topics or projects yet — use + Ingest Literature to get started)");
            }

            let mut drill_into = None;
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
                    // op-9vo6.10 (PaperSession) wires this link to actually
                    // open the Research workspace; for now it is a listing.
                    ui.label(format!("\u{1F4C4} {citekey}"));
                }
            }
        });

        action
    }
}
