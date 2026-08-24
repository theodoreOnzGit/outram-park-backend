//! Project browser + `.bib` entry editor (op-9vml — "Kovan should have a
//! bibliography window which manages bib files like jabref. However, there
//! should be more than jabref ... an entry ... should be able to jump to its
//! PDF, its markdown sections, and its digitised CSVs").
//!
//! Two halves, both now implemented:
//!
//! - **Cross-referencing** (the "more than JabRef" part): list the
//!   project's documents from `kovan.toml` (via [`crate::project::regenerate`]),
//!   search/filter by id, jump to a document's PDF (PDF Reader tab) or its
//!   markdown sections (Markdown Editor tab, op-wr08).
//! - **`.bib` entry editing** (op-xj8t, the plain JabRef half — list,
//!   add, edit, delete, save): reads the project's one `.bib` file via
//!   [`kovan_literature::parse_bib_entries`] (op-vi1n) into
//!   [`kovan_literature::BibEntry`] and writes it back with
//!   [`kovan_literature::render_entries`]. **Deliberately not a
//!   byte-for-byte round trip** — see those functions' own docs: original
//!   field order and any comments/whitespace outside entries are not
//!   preserved. Saving fully rewrites the file from the in-memory entry
//!   list, the same "regenerate wins over incremental patch" posture
//!   `crate::project::write_index` already takes for `kovan.toml`.

use std::collections::BTreeMap;

use eframe::egui;
use kovan_literature::BibEntry;

use crate::project::{self, ProjectIndex};

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

/// State for the bibliography/project-browser tab.
#[derive(Default)]
pub struct BibliographyState {
    root: String,
    index: Option<ProjectIndex>,
    filter: String,
    /// The project's `.bib` file, parsed (op-xj8t) — `None` until a project
    /// with a readable `.bib` file has been opened.
    entries: Option<Vec<BibEntry>>,
    editor: Option<EntryEditor>,
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
                self.entries = None;
                self.editor = None;
                let bib_file = index.bib_file.clone();
                self.index = Some(index);
                self.message = format!("opened {root} — {n} document(s)");
                self.load_bib_entries(&bib_file);
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    fn bib_path(&self, bib_file: &str) -> std::path::PathBuf {
        std::path::Path::new(&self.root).join(bib_file)
    }

    fn load_bib_entries(&mut self, bib_file: &str) {
        let path = self.bib_path(bib_file);
        match std::fs::read_to_string(&path) {
            Ok(text) => match kovan_literature::parse_bib_entries(&text) {
                Ok(entries) => self.entries = Some(entries),
                Err(e) => self.message = format!("{}: {e}", path.display()),
            },
            Err(e) => self.message = format!("{}: {e}", path.display()),
        }
    }

    /// Write `self.entries` back to the project's `.bib` file (op-xj8t) —
    /// fully rewrites the file from the in-memory list (see the module
    /// doc's "not a byte-for-byte round trip" note), then re-scans the
    /// project so `kovan.toml`'s cite-key join picks up any cite-key change
    /// immediately (an edited/added/deleted entry can change which
    /// documents the join finds).
    fn save_bib_entries(&mut self) {
        let Some(index) = &self.index else { return };
        let Some(entries) = &self.entries else { return };
        let path = self.bib_path(&index.bib_file);
        let text = kovan_literature::render_entries(entries);
        if let Err(e) = std::fs::write(&path, text) {
            self.message = format!("cannot write {}: {e}", path.display());
            return;
        }
        let root = self.root.clone();
        self.message = format!("saved {}", path.display());
        self.open_project(&root);
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

    fn delete_entry(&mut self, i: usize) {
        if let Some(entries) = &mut self.entries {
            if i < entries.len() {
                entries.remove(i);
                self.save_bib_entries();
            }
        }
    }

    fn commit_editor(&mut self) {
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
        self.save_bib_entries();
    }

    /// The `.bib` entry list + add/edit form (op-xj8t) — drawn above the
    /// existing document cross-reference list.
    fn bib_editor_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Bibliography entries");
            if ui.button("+ New entry").clicked() {
                self.start_new_entry();
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
                self.commit_editor();
            } else if cancel {
                self.editor = None;
            }
        }

        let Some(entries) = &self.entries else {
            ui.small("no .bib entries loaded");
            return;
        };
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
            self.delete_entry(i);
        }
        ui.separator();
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

        if self.index.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("open a kovan-folder project to browse its documents");
            });
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            return None;
        }

        self.bib_editor_ui(ui);

        let Some(index) = &self.index else {
            return None;
        };

        ui.horizontal(|ui| {
            ui.label("filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.label(format!("bib file: {}", index.bib_file));
        ui.small("Document cross-references — jump to a document's PDF or markdown sections.");
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
