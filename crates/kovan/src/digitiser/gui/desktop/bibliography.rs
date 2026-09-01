//! Bibliography browser + `.bib` entry editor (op-9vml — "Kovan should have
//! a bibliography window which manages bib files like jabref. However,
//! there should be more than jabref ... an entry ... should be able to jump
//! to its PDF, its markdown sections, and its digitised CSVs").
//!
//! Two halves:
//!
//! - **Cross-referencing** (the "more than JabRef" part): list the open
//!   [`KovanRoot`]'s papers and jump straight to one (op-9r26, GH issue #35
//!   2026-09-01: "Bibliography should automatically use the bibliography
//!   belonging to the open root ... Do not ask the user to select a
//!   project folder ... Do not open a folder picker"). One click routes
//!   through the same [`super::DigitiseApp::activate_paper`] every other
//!   paper-opening view uses (Wiki, Mindmap) — no separate "open PDF" vs.
//!   "edit markdown" actions to pick between, since activating a paper
//!   already does both.
//! - **`.bib` entry editing** (op-xj8t, the plain JabRef half — list, add,
//!   edit, delete, save): reads the root's one `.bib` file
//!   ([`KovanRoot::bibliography_path`]) via
//!   [`kovan_literature::parse_bib_entries`] (op-vi1n) into
//!   [`kovan_literature::BibEntry`] and writes it back with
//!   [`kovan_literature::render_entries`]. **Deliberately not a
//!   byte-for-byte round trip** — see those functions' own docs: original
//!   field order and any comments/whitespace outside entries are not
//!   preserved. Saving fully rewrites the file from the in-memory entry
//!   list.
//!
//! # Migrated off the older `crate::project` format (op-9r26)
//!
//! This module used to browse a `pdf/` + `markdown/` "kovan folder" project
//! (`crate::project::regenerate`, addressed by a hand-typed root path and a
//! generated `kovan.toml` with regenerated line-range sections) — a
//! genuinely different, older model from [`KovanRoot`]'s citekey-addressed
//! papers (see `root.rs`'s own module doc, "Relationship to
//! `crate::project`"). It now speaks only the new model; the old one is
//! still used elsewhere (the Digitiser tabs' own manual project-root save
//! path, and a PDF opened outside any paper) but no longer has a GUI
//! surface of its own — see `crates/kovan/docs/kovan-redesign-migration-map.md`.

use std::collections::BTreeMap;

use eframe::egui;
use kovan_literature::BibEntry;

use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;

/// In-progress edit of one `.bib` entry (op-xj8t) — a `Vec` of `(name,
/// value)` pairs rather than the target `BTreeMap` directly, so the editor
/// can show fields in a stable, user-controlled order and support adding a
/// new blank field row without fighting `BTreeMap`'s key-sorted iteration.
struct EntryEditor {
    /// `Some(i)` = editing `entries[i]` in place; `None` = a brand-new entry.
    editing_index: Option<usize>,
    entry_type: String,
    cite_key: String,
    fields: Vec<(String, String)>,
}

impl EntryEditor {
    fn new_blank() -> Self {
        Self {
            editing_index: None,
            entry_type: "article".to_string(),
            cite_key: String::new(),
            fields: vec![("title".to_string(), String::new())],
        }
    }

    fn from_entry(i: usize, entry: &BibEntry) -> Self {
        Self {
            editing_index: Some(i),
            entry_type: entry.entry_type.clone(),
            cite_key: entry.cite_key.clone(),
            fields: entry.fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    fn to_entry(&self) -> BibEntry {
        let fields: BTreeMap<String, String> = self
            .fields
            .iter()
            .filter(|(k, _)| !k.trim().is_empty())
            .map(|(k, v)| (k.trim().to_lowercase(), v.clone()))
            .collect();
        BibEntry {
            entry_type: self.entry_type.trim().to_lowercase(),
            cite_key: self.cite_key.trim().to_string(),
            fields,
        }
    }
}

/// State for the bibliography tab.
#[derive(Default)]
pub struct BibliographyState {
    filter: String,
    /// The root's `.bib` file, parsed (op-xj8t) — `None` until [`Self::ui`]
    /// has loaded it at least once for the currently open root; `Some(vec![])`
    /// for a valid, empty (or not-yet-existing) bibliography, which is not
    /// an error — a freshly created library has no entries yet.
    entries: Option<Vec<BibEntry>>,
    editor: Option<EntryEditor>,
    message: String,
}

/// What the operator asked to do with one listed paper — handed back to
/// [`super::DigitiseApp`], which owns [`super::DigitiseApp::activate_paper`].
pub enum BibliographyAction {
    OpenPaper(String),
}

impl BibliographyState {
    /// Load (or reload) the bibliography from `root` — the "belongs to the
    /// open root" rule, so there is no separate project/folder concept for
    /// this panel to track. A missing `.bib` file is the ordinary "nothing
    /// ingested yet" empty state, not an error.
    fn load(&mut self, root: &KovanRoot) {
        self.editor = None;
        match std::fs::read_to_string(root.bibliography_path()) {
            Ok(text) => match kovan_literature::parse_bib_entries(&text) {
                Ok(entries) => self.entries = Some(entries),
                Err(e) => {
                    self.message = format!("{}: {e}", root.bibliography_path().display());
                    self.entries = Some(Vec::new());
                }
            },
            Err(_) => self.entries = Some(Vec::new()),
        }
    }

    /// Write `self.entries` back to `root`'s `.bib` file (op-xj8t) — fully
    /// rewrites the file from the in-memory list (see the module doc's "not
    /// a byte-for-byte round trip" note), then reloads so an edited/added/
    /// deleted entry is reflected immediately.
    fn save_bib_entries(&mut self, root: &KovanRoot) {
        let Some(entries) = &self.entries else { return };
        let path = root.bibliography_path();
        let text = kovan_literature::render_entries(entries);
        if let Err(e) = std::fs::write(&path, text) {
            self.message = format!("cannot write {}: {e}", path.display());
            return;
        }
        self.message = format!("saved {}", path.display());
        self.load(root);
    }

    fn start_new_entry(&mut self) {
        self.editor = Some(EntryEditor::new_blank());
    }

    fn start_edit(&mut self, i: usize) {
        if let Some(entries) = &self.entries {
            if let Some(entry) = entries.get(i) {
                self.editor = Some(EntryEditor::from_entry(i, entry));
            }
        }
    }

    fn delete_entry(&mut self, i: usize, root: &KovanRoot) {
        if let Some(entries) = &mut self.entries {
            if i < entries.len() {
                entries.remove(i);
                self.save_bib_entries(root);
            }
        }
    }

    fn commit_editor(&mut self, root: &KovanRoot) {
        let Some(editor) = self.editor.take() else { return };
        let entry = editor.to_entry();
        if entry.cite_key.is_empty() {
            self.message = "cite key cannot be empty".to_string();
            self.editor = Some(editor);
            return;
        }
        let entries = self.entries.get_or_insert_with(Vec::new);
        match editor.editing_index {
            Some(i) if i < entries.len() => entries[i] = entry,
            _ => entries.push(entry),
        }
        self.save_bib_entries(root);
    }

    /// The `.bib` entry list + add/edit form (op-xj8t) — drawn above the
    /// paper cross-reference list.
    fn bib_editor_ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot) {
        ui.horizontal(|ui| {
            ui.heading("Bibliography entries");
            if ui.button("+ New entry").clicked() {
                self.start_new_entry();
            }
            if ui.button("Refresh").clicked() {
                self.load(root);
            }
        });

        if let Some(editor) = &mut self.editor {
            let mut commit = false;
            let mut cancel = false;
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("type");
                    ui.text_edit_singleline(&mut editor.entry_type);
                    ui.label("cite key*");
                    ui.text_edit_singleline(&mut editor.cite_key);
                });
                let mut remove_field: Option<usize> = None;
                for (i, (name, value)) in editor.fields.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(name).desired_width(100.0));
                        ui.text_edit_multiline(value);
                        if ui.small_button("✕").clicked() {
                            remove_field = Some(i);
                        }
                    });
                }
                if let Some(i) = remove_field {
                    editor.fields.remove(i);
                }
                if ui.button("+ field").clicked() {
                    editor.fields.push((String::new(), String::new()));
                }
                ui.horizontal(|ui| {
                    if ui.button("Save entry").clicked() {
                        commit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
            if commit {
                self.commit_editor(root);
            } else if cancel {
                self.editor = None;
            }
        }

        let Some(entries) = &self.entries else {
            ui.small("no .bib entries loaded");
            return;
        };
        if entries.is_empty() {
            ui.small("no bibliography entries yet — ingest a paper from the Wiki to get started");
        }
        let mut edit_i = None;
        let mut delete_i = None;
        egui::ScrollArea::vertical().id_salt("bib_entries_scroll").max_height(220.0).show(ui, |ui| {
            for (i, entry) in entries.iter().enumerate() {
                ui.horizontal(|ui| {
                    let title = entry.fields.get("title").map(String::as_str).unwrap_or("");
                    ui.label(format!("{} [{}] {}", entry.cite_key, entry.entry_type, title));
                    if ui.small_button("Edit").clicked() {
                        edit_i = Some(i);
                    }
                    if ui.small_button("Delete").clicked() {
                        delete_i = Some(i);
                    }
                });
            }
        });
        if let Some(i) = edit_i {
            self.start_edit(i);
        }
        if let Some(i) = delete_i {
            self.delete_entry(i, root);
        }
        ui.separator();
    }

    /// Draw the panel; returns `Some` the frame the operator clicks "Open"
    /// on a paper. `index` is the open root's already-loaded
    /// [`KnowledgeIndex`] (the same one Wiki/Mindmap use) — this panel
    /// lists papers from it, not a second document scan.
    pub fn ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot, index: &KnowledgeIndex) -> Option<BibliographyAction> {
        if self.entries.is_none() {
            self.load(root);
        }

        self.bib_editor_ui(ui, root);
        if !self.message.is_empty() {
            ui.label(&self.message);
        }

        ui.horizontal(|ui| {
            ui.label("filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.small("Papers — click Open to activate one (its PDF and research Markdown together).");
        ui.separator();

        let needle = self.filter.trim().to_ascii_lowercase();
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for paper in &index.papers {
                if !needle.is_empty() && !paper.citekey.to_ascii_lowercase().contains(&needle) {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(&paper.citekey);
                    if ui.button("Open").clicked() {
                        action = Some(BibliographyAction::OpenPaper(paper.citekey.clone()));
                    }
                });
            }
        });
        action
    }
}
