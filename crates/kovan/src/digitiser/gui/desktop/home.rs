//! The startup screen (op-9vo6.3, GitHub issue #35 §2): "KOVAN / Recent
//! Kovan folders / [ Open Kovan Folder… ] / [ + Create Kovan Folder… ]" —
//! replacing the previous default of launching straight into the PDF
//! reader.
//!
//! This module owns no [`egui_file_dialog::FileDialog`] of its own —
//! [`super::DigitiseApp`] already has one shared instance (see
//! `FileDialogTarget`) — so [`HomeState::ui`] returns a [`HomeAction`] when
//! a directory picker needs to be opened, and the caller feeds a picked
//! directory back in through [`HomeState::open_dir`] /
//! [`HomeState::begin_create`].
//!
//! # Recent-roots persistence
//!
//! §2/§5's own instruction: "Recent-roots list is local state -> .kovan/ or
//! the XDG config dir, never tracked." There is by definition no open root
//! at this screen, so `.kovan/` isn't an option yet — this uses the OS
//! config directory instead, the same `directories::ProjectDirs` pattern
//! `njoy-outram-park-fork::acquire` already uses for its own cache dir.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};

use crate::root::{KovanRoot, RootConfig};

/// How many recent roots to remember. A plain list, not a priority queue —
/// small enough that trimming on every push is cheap.
const MAX_RECENT: usize = 10;

fn recent_roots_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan").map(|d| d.config_dir().join("recent_roots.toml"))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentRootsFile {
    #[serde(default)]
    roots: Vec<PathBuf>,
}

fn load_recent_roots() -> Vec<PathBuf> {
    let Some(path) = recent_roots_path() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    toml::from_str::<RecentRootsFile>(&text).map(|f| f.roots).unwrap_or_default()
}

fn save_recent_roots(roots: &[PathBuf]) {
    let Some(path) = recent_roots_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = RecentRootsFile { roots: roots.to_vec() };
    if let Ok(text) = toml::to_string_pretty(&file) {
        let _ = std::fs::write(path, text);
    }
}

fn push_recent(roots: &mut Vec<PathBuf>, path: PathBuf) {
    roots.retain(|p| p != &path);
    roots.insert(0, path);
    roots.truncate(MAX_RECENT);
    save_recent_roots(roots);
}

/// A directory the "Open"/"Create" pickers just returned — the caller
/// (`DigitiseApp`) should call [`HomeState::open_dir`] or
/// [`HomeState::begin_create`] with it, matching which dialog was open.
pub enum HomeAction {
    RequestOpenDialog,
    RequestCreateDialog,
}

/// State for the startup screen. Owns the recent-roots list and, once one
/// is open, the [`KovanRoot`] itself — `DigitiseApp` reads
/// [`HomeState::root`] to decide whether to show this screen or the Wiki
/// (§8: "after opening a root, land in the Wiki").
pub struct HomeState {
    recent_roots: Vec<PathBuf>,
    root: Option<KovanRoot>,
    /// Directory picked for "+ Create Kovan Folder…", awaiting the small
    /// id/name prompt (§5's `[library] id/name`) before
    /// [`KovanRoot::create`] actually runs.
    pending_create_dir: Option<PathBuf>,
    new_library_id: String,
    new_library_name: String,
    message: String,
    message_is_error: bool,
}

impl Default for HomeState {
    fn default() -> Self {
        Self {
            recent_roots: load_recent_roots(),
            root: None,
            pending_create_dir: None,
            new_library_id: String::new(),
            new_library_name: String::new(),
            message: String::new(),
            message_is_error: false,
        }
    }
}

impl HomeState {
    pub fn root(&self) -> Option<&KovanRoot> {
        self.root.as_ref()
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = true;
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = false;
    }

    /// A directory was picked for "Open Kovan Folder…", or a recent-roots
    /// entry was clicked. [`KovanRoot::discover`] rather than
    /// [`KovanRoot::open`] — the picked directory may be anywhere inside
    /// the library, not necessarily its exact root.
    pub fn open_dir(&mut self, dir: &Path) {
        match KovanRoot::discover(dir) {
            Ok(root) => {
                push_recent(&mut self.recent_roots, root.path().to_path_buf());
                self.set_status(format!("opened {}", root.path().display()));
                self.root = Some(root);
            }
            Err(e) => self.set_error(format!("not a Kovan folder: {e}")),
        }
    }

    /// A directory was picked for "+ Create Kovan Folder…" — stash it and
    /// show the id/name prompt rather than creating immediately.
    pub fn begin_create(&mut self, dir: &Path) {
        let default_id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "library".to_string());
        self.new_library_id = default_id.clone();
        self.new_library_name = default_id;
        self.pending_create_dir = Some(dir.to_path_buf());
    }

    fn finish_create(&mut self) {
        let Some(dir) = self.pending_create_dir.take() else { return };
        if self.new_library_id.trim().is_empty() {
            self.set_error("library id must not be empty");
            self.pending_create_dir = Some(dir);
            return;
        }
        let config = RootConfig::new(self.new_library_id.trim(), self.new_library_name.trim());
        match KovanRoot::create(&dir, config, true) {
            Ok(root) => {
                push_recent(&mut self.recent_roots, root.path().to_path_buf());
                self.set_status(format!("created {}", root.path().display()));
                self.root = Some(root);
            }
            Err(e) => {
                self.set_error(format!("could not create Kovan folder: {e}"));
                self.pending_create_dir = Some(dir);
            }
        }
    }

    /// Draw the startup screen. Returns `Some` when a directory picker
    /// needs to be opened by the caller.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<HomeAction> {
        let mut action = None;

        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading("KOVAN");
            // op-ngvq (GH issue #35's 2026-09-01 checkpoint §1): the
            // canonical full expansion belongs on the startup screen;
            // ordinary workspace chrome elsewhere keeps the plain "KOVAN".
            ui.weak("Knowledge-Oriented V&V for Analysis of Nuclear Reactors");
            ui.add_space(20.0);

            if !self.message.is_empty() {
                let color = if self.message_is_error { Color32::from_rgb(220, 90, 90) } else { ui.visuals().weak_text_color() };
                ui.colored_label(color, &self.message);
                ui.add_space(10.0);
            }

            if let Some(dir) = self.pending_create_dir.clone() {
                ui.group(|ui| {
                    ui.label(format!("Create Kovan folder in: {}", dir.display()));
                    ui.horizontal(|ui| {
                        ui.label("Library id:");
                        ui.text_edit_singleline(&mut self.new_library_id);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Library name:");
                        ui.text_edit_singleline(&mut self.new_library_name);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            self.finish_create();
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_create_dir = None;
                        }
                    });
                });
                ui.add_space(20.0);
            }

            ui.label("Recent Kovan folders");
            ui.add_space(6.0);
            if self.recent_roots.is_empty() {
                ui.weak("(none yet)");
            } else {
                let mut to_open = None;
                for path in &self.recent_roots {
                    if ui.button(path.display().to_string()).clicked() {
                        to_open = Some(path.clone());
                    }
                }
                if let Some(path) = to_open {
                    self.open_dir(&path);
                }
            }

            ui.add_space(24.0);
            ui.horizontal(|ui| {
                if ui.button("Open Kovan Folder…").clicked() {
                    action = Some(HomeAction::RequestOpenDialog);
                }
                if ui.button("+ Create Kovan Folder…").clicked() {
                    action = Some(HomeAction::RequestCreateDialog);
                }
            });
        });

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_dir_discovers_a_root_from_a_nested_path() {
        let dir = tempfile::tempdir().unwrap();
        KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        std::fs::create_dir_all(dir.path().join("papers").join("x")).unwrap();

        let mut home = HomeState::default();
        home.open_dir(&dir.path().join("papers").join("x"));

        assert!(!home.message_is_error, "{}", home.message);
        assert_eq!(home.root().unwrap().path(), dir.path());
    }

    #[test]
    fn open_dir_on_a_non_root_sets_an_error_and_no_root() {
        let dir = tempfile::tempdir().unwrap();
        let mut home = HomeState::default();
        home.open_dir(dir.path());
        assert!(home.message_is_error);
        assert!(home.root().is_none());
    }

    #[test]
    fn begin_create_then_finish_create_opens_a_new_root() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("my-lib");
        let mut home = HomeState::default();
        home.begin_create(&target);
        assert_eq!(home.new_library_id, "my-lib");
        home.finish_create();
        assert!(!home.message_is_error, "{}", home.message);
        assert_eq!(home.root().unwrap().path(), target);
    }
}
