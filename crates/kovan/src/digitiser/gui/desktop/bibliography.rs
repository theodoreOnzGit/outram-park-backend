//! Project browser / cross-reference window (op-9vml — "Kovan should have a
//! bibliography window which manages bib files like jabref. However, there
//! should be more than jabref ... an entry ... should be able to jump to its
//! PDF, its markdown sections, and its digitised CSVs").
//!
//! **Scoped down from the full bead, deliberately — read this before
//! extending.** The bead's core ask is a JabRef-like `.bib` entry
//! list/search/**edit** surface. This workspace has no BibTeX *parser*
//! today (`kovan_literature::bibtex::to_bibtex` only *renders* a
//! `KovanDocument` into BibTeX text, one-way — see
//! `crates/kovan/src/project.rs`'s own doc comment and kopi-beans `op-vi1n`,
//! filed for exactly this gap), so there is no way to read an existing
//! `.bib` file's entries back into anything a GUI could list, search, or
//! edit. **This panel implements the cross-referencing half only** — the
//! part GitHub issue #30 called "more than JabRef" and the part that is
//! actually buildable today, from `kovan.toml`'s own document list (no bib
//! parsing needed): list the project's documents, search/filter by id, jump
//! to a document's PDF (in the PDF Reader tab) or its markdown sections (in
//! the Markdown Editor tab, op-wr08). It does **not** show or edit `.bib`
//! entry fields (title/authors/year/…) — that half of the bead stays open,
//! blocked on `op-vi1n`.

use eframe::egui;

use crate::project::{self, ProjectIndex};

/// State for the bibliography/project-browser tab.
#[derive(Default)]
pub struct BibliographyState {
    root: String,
    index: Option<ProjectIndex>,
    filter: String,
    message: String,
}

/// What the operator asked to do with one listed document — handed back to
/// [`super::DigitiseApp`], which owns the PDF reader and markdown editor
/// this panel jumps into.
pub enum BibliographyAction {
    OpenPdf { path: String },
    EditMarkdown { root: String, doc_id: String },
}

impl BibliographyState {
    pub fn open_project(&mut self, root: &str) {
        match project::regenerate(std::path::Path::new(root)) {
            Ok(index) => {
                let n = index.documents.len();
                self.root = root.to_string();
                self.index = Some(index);
                self.message = format!("opened {root} — {n} document(s)");
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    /// Draw the panel; returns `Some` the frame the operator clicks a jump
    /// action on one row.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        mut on_browse: impl FnMut(),
    ) -> Option<BibliographyAction> {
        ui.horizontal(|ui| {
            ui.label("project folder:");
            ui.text_edit_singleline(&mut self.root);
            if ui.button("Open").clicked() {
                let root = self.root.clone();
                self.open_project(&root);
            }
            if ui.button("Browse…").clicked() {
                on_browse();
            }
        });

        let Some(index) = &self.index else {
            ui.centered_and_justified(|ui| {
                ui.label("open a kovan-folder project to browse its documents");
            });
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return None;
        };

        ui.horizontal(|ui| {
            ui.label("filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.label(format!("bib file: {}", index.bib_file));
        ui.small(
            "Shows kovan.toml's cross-references (PDF / markdown sections) only — \
             .bib entry text is not shown here; see this panel's module docs for why.",
        );
        ui.separator();

        let needle = self.filter.trim().to_ascii_lowercase();
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for doc in &index.documents {
                if !needle.is_empty() && !doc.id.to_ascii_lowercase().contains(&needle) {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(&doc.id);
                    if ui.button("Open PDF").clicked() {
                        action = Some(BibliographyAction::OpenPdf {
                            path: std::path::Path::new(&self.root)
                                .join(&doc.pdf)
                                .to_string_lossy()
                                .into_owned(),
                        });
                    }
                    if ui.button("Edit Markdown").clicked() {
                        action = Some(BibliographyAction::EditMarkdown {
                            root: self.root.clone(),
                            doc_id: doc.id.clone(),
                        });
                    }
                    let sections: Vec<&str> = project::SECTION_ORDER
                        .iter()
                        .filter(|name| doc.sections.get(name).is_some())
                        .copied()
                        .collect();
                    ui.label(format!("[{}]", sections.join(", ")));
                });
            }
        });
        action
    }
}
