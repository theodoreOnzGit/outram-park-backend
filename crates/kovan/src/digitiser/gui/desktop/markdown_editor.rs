//! Structured markdown section editor (op-wr08 — "Kovan should be able to
//! read and write the individual markdowns via gui, add or remove
//! information, but should not be able to change the structure").
//!
//! A thin GUI shell over [`crate::project`]: this file owns no parsing or
//! line-range logic of its own — it opens a "kovan folder" project (design
//! doc `docs/kovan-folder-format.md`, op-63u0), lists its documents and
//! their five standard sections (from `kovan.toml`, via
//! [`crate::project::regenerate`]), and for the selected section shows
//! [`crate::project::read_section`]'s split: the marker + heading line as
//! **read-only** text (never an editable field, so there is nothing here a
//! user could type into to change the structure), and the body as a plain
//! multi-line [`egui::TextEdit`]. Saving calls
//! [`crate::project::write_section`], which itself re-checks the section's
//! range against a fresh disk scan and refuses a stale write rather than
//! silently overwriting — this panel surfaces that as a message, it does
//! not re-implement the check.

use eframe::egui;

use crate::project::{self, ProjectError, ProjectIndex, SectionContent};

/// State for the markdown editor tab.
#[derive(Default)]
pub struct MarkdownEditorState {
    root: String,
    index: Option<ProjectIndex>,
    selected_doc: usize,
    selected_section: usize,
    /// The section as last read from disk — `loaded_range` is what
    /// [`project::write_section`] checks freshness against.
    loaded: Option<SectionContent>,
    loaded_range: Option<[usize; 2]>,
    /// The editable copy of `loaded.body` — diverges from it as the user
    /// types; `loaded` stays the on-open snapshot until the next open/save.
    body_buffer: String,
    dirty: bool,
    message: String,
}

/// Human-readable label for one of [`project::SECTION_ORDER`]'s keys.
fn section_label(key: &str) -> &'static str {
    match key {
        "ai_summary" => "AI Summary",
        "author_summary" => "Author Summary",
        "full_text" => "Full Text",
        "table_csvs" => "Table CSVs",
        "graph_csvs" => "Graph CSVs",
        _ => "?",
    }
}

impl MarkdownEditorState {
    /// Open `root` as a "kovan folder" project — scans it (read-only; does
    /// **not** write `kovan.toml`, so simply browsing to a folder never
    /// mutates it) and lists its documents.
    pub fn open_project(&mut self, root: &str) {
        match project::regenerate(std::path::Path::new(root)) {
            Ok(index) => {
                self.root = root.to_string();
                let n = index.documents.len();
                self.index = Some(index);
                self.selected_doc = 0;
                self.selected_section = 0;
                self.loaded = None;
                self.loaded_range = None;
                self.dirty = false;
                self.message = format!("opened {root} — {n} document(s)");
                self.load_selected_section();
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    /// Open `root` (if not already open — re-scanning on every call would
    /// undo an in-progress edit whenever the bibliography window's "Edit
    /// Markdown" is clicked again for a different document in the same
    /// project) and select `doc_id`'s first present section, or its first
    /// section regardless if none has content yet. The bibliography
    /// window's (op-9vml) cross-reference hand-off.
    pub fn open_document(&mut self, root: &str, doc_id: &str) {
        if self.root != root || self.index.is_none() {
            self.open_project(root);
        }
        let Some(index) = &self.index else { return };
        let Some(pos) = index.documents.iter().position(|d| d.id == doc_id) else {
            self.message = format!("{doc_id}: not found in {root}'s kovan.toml");
            return;
        };
        self.selected_doc = pos;
        self.selected_section = project::SECTION_ORDER
            .iter()
            .position(|name| index.documents[pos].sections.get(name).is_some())
            .unwrap_or(0);
        self.load_selected_section();
    }

    fn selected_markdown_rel(&self) -> Option<String> {
        self.index
            .as_ref()?
            .documents
            .get(self.selected_doc)
            .map(|d| d.markdown.clone())
    }

    /// Load the currently selected document/section's content from disk,
    /// if that section actually has a range recorded in `kovan.toml` (a
    /// document need not have every section yet — see the design doc's
    /// "don't fabricate a zero" rule).
    fn load_selected_section(&mut self) {
        self.loaded = None;
        self.loaded_range = None;
        self.body_buffer.clear();
        self.dirty = false;
        let Some(index) = &self.index else { return };
        let Some(doc) = index.documents.get(self.selected_doc) else {
            return;
        };
        let name = project::SECTION_ORDER[self.selected_section];
        let Some(range) = doc.sections.get(name) else {
            self.message = format!("{} has no {} section yet", doc.id, section_label(name));
            return;
        };
        let markdown_path = std::path::Path::new(&self.root).join(&doc.markdown);
        match project::read_section(&markdown_path, range) {
            Ok(content) => {
                self.body_buffer = content.body.clone();
                self.loaded = Some(content);
                self.loaded_range = Some(range);
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    fn save(&mut self) {
        let (Some(range), Some(markdown_rel)) = (self.loaded_range, self.selected_markdown_rel())
        else {
            self.message = "nothing loaded to save".to_string();
            return;
        };
        let name = project::SECTION_ORDER[self.selected_section];
        match project::write_section(
            std::path::Path::new(&self.root),
            &markdown_rel,
            name,
            range,
            &self.body_buffer,
        ) {
            Ok(index) => {
                self.index = Some(index);
                self.message = format!("saved {} / {}", markdown_rel, section_label(name));
                self.dirty = false;
                // The range likely shifted (body length changed) — reload
                // so the next save checks against the new range, not the
                // one that just became stale by the very save that
                // succeeded.
                self.load_selected_section();
            }
            Err(ProjectError::StaleSectionRange { .. }) => {
                self.message =
                    "save rejected: this section changed on disk since it was opened — \
                     reopen the document and re-apply your edit"
                        .to_string();
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mut on_browse: impl FnMut()) {
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
                ui.label("open a kovan-folder project to edit its markdown");
            });
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return;
        };

        if index.documents.is_empty() {
            ui.label("no documents indexed — no pdf/<stem> with a matching markdown/<stem> yet");
            return;
        }

        ui.separator();
        let mut doc_changed = false;
        egui::ComboBox::from_label("document")
            .selected_text(index.documents[self.selected_doc].id.clone())
            .show_ui(ui, |ui| {
                for (i, doc) in index.documents.iter().enumerate() {
                    if ui
                        .selectable_value(&mut self.selected_doc, i, &doc.id)
                        .clicked()
                    {
                        doc_changed = true;
                    }
                }
            });

        let mut section_changed = false;
        ui.horizontal(|ui| {
            for (i, name) in project::SECTION_ORDER.iter().enumerate() {
                let has = index.documents[self.selected_doc].sections.get(name).is_some();
                let label = if has {
                    section_label(name).to_string()
                } else {
                    format!("{} (empty)", section_label(name))
                };
                if ui
                    .selectable_value(&mut self.selected_section, i, label)
                    .clicked()
                {
                    section_changed = true;
                }
            }
        });

        if doc_changed || section_changed {
            self.load_selected_section();
        }

        ui.separator();
        let Some(loaded) = &self.loaded else {
            ui.label(&self.message);
            return;
        };
        // Structure — marker + heading — shown but never editable: this is
        // the whole point of op-wr08's "content-only" contract.
        ui.label("structure (read-only):");
        ui.add_enabled(
            false,
            egui::TextEdit::singleline(&mut loaded.marker_line.clone()),
        );
        ui.add_enabled(
            false,
            egui::TextEdit::singleline(&mut loaded.heading_line.clone()),
        );
        ui.separator();
        ui.label("content:");
        let response = ui.add(
            egui::TextEdit::multiline(&mut self.body_buffer)
                .desired_width(f32::INFINITY)
                .desired_rows(20),
        );
        if response.changed() {
            self.dirty = true;
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.dirty, egui::Button::new("Save"))
                .clicked()
            {
                self.save();
            }
            if self.dirty {
                ui.colored_label(egui::Color32::from_rgb(230, 160, 60), "unsaved changes");
            }
        });
        ui.label(&self.message);
    }
}
