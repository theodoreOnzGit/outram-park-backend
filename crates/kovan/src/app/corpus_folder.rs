//! The "where should the standard corpus go?" window (maintainer direction,
//! 2026-10-06: *"when kovan opens, the user also needs to specify an empty
//! folder (or existing one) for the public corpus, so that kovan knows where
//! to dump the pdfs"*).
//!
//! Shown at start until a folder is chosen. The folder may be new, empty, or
//! an existing clone of the standard corpus
//! ([`crate::corpus_repos::check_standard_corpus_folder`]); once chosen it is
//! remembered, and Kovan refreshes the corpus there at every start
//! ([`crate::corpus_repos::update_standard_corpus`]). The built-in map does
//! not depend on it, so "Not now" closes the window for this session only.
//!
//! What does not belong here: checking, recording or cloning the folder. The
//! window only returns a [`CorpusFolderRequest`]; the app acts on it through
//! [`crate::corpus_repos`].

use eframe::egui;
use std::path::PathBuf;

/// What the user asked the window to do.
pub(super) enum CorpusFolderRequest {
    /// Open the folder picker for the folder field.
    Browse,
    /// Use this folder for the standard corpus.
    Use(PathBuf),
    /// Close for this session; ask again next start.
    NotNow,
}

/// The window's state, kept between frames.
#[derive(Default)]
pub(super) struct CorpusFolderDialog {
    pub(super) open: bool,
    /// The folder path, typed or picked.
    pub(super) folder: String,
    message: String,
}

impl CorpusFolderDialog {
    /// Open the window, suggesting `suggested` when the field is empty.
    pub(super) fn show(&mut self, suggested: Option<PathBuf>) {
        self.open = true;
        self.message.clear();
        if self.folder.is_empty() {
            if let Some(dir) = suggested {
                self.folder = dir.display().to_string();
            }
        }
    }

    /// Show a problem with the chosen folder in the window.
    pub(super) fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Draw the window, if open. Returns what the user asked for this frame.
    pub(super) fn ui(&mut self, ctx: &egui::Context) -> Option<CorpusFolderRequest> {
        if !self.open {
            return None;
        }
        let mut request = None;
        egui::Window::new("Standard corpus folder")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    "Kovan's standard corpus is the open literature behind the built-in \
                     map (NRC, DOE, EPA, EC and other openly licensed documents). Choose \
                     where Kovan should keep its PDFs: an empty folder, or one that \
                     already holds the standard corpus. Kovan refreshes it every time \
                     it opens.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.folder).desired_width(360.0));
                    if ui.button("Browse…").clicked() {
                        request = Some(CorpusFolderRequest::Browse);
                    }
                });
                ui.weak("It is downloaded from GitHub: a few hundred MB on the first run.");
                if !self.message.is_empty() {
                    ui.add_space(6.0);
                    ui.colored_label(ui.visuals().warn_fg_color, &self.message);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Use this folder").clicked() {
                        let folder = self.folder.trim();
                        if folder.is_empty() {
                            self.message = "Choose a folder first.".into();
                        } else {
                            request = Some(CorpusFolderRequest::Use(PathBuf::from(folder)));
                        }
                    }
                    if ui.button("Not now").clicked() {
                        request = Some(CorpusFolderRequest::NotNow);
                    }
                });
            });
        request
    }
}
