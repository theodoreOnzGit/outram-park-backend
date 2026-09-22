//! The first-run setup dialog (GitHub issue #255).
//!
//! What belongs here: the window that asks, on first launch, for the Kovan
//! folder and then, optionally, the user's **open corpus** and **proprietary
//! corpus** GitHub repositories; and the marker that remembers first run is
//! over. It is **skippable** (maintainer decision, 2026-09-22): the built-in
//! nuclear-engineering map shows with or without a folder (epic #247), and the
//! top bar's "⚙ Setup" reopens the dialog at any time.
//!
//! What does not belong here: creating the folder or the repositories. The
//! dialog only returns a [`SetupRequest`]; the app acts on it with
//! [`crate::root::KovanRoot`] and [`crate::corpus_repos`], so none of the Git
//! or filesystem logic lives in GUI code.

use crate::root::KovanRoot;
use eframe::egui;
use std::path::PathBuf;

/// What the user asked the dialog to do.
pub(super) enum SetupRequest {
    /// Open the folder picker for the Kovan folder field.
    Browse,
    /// Set up with these answers. `folder` is required; either remote may be
    /// absent, in which case that corpus is initialised as a local repository.
    Finish {
        folder: PathBuf,
        open_remote: Option<String>,
        proprietary_remote: Option<String>,
    },
    /// Close without setting anything up; first run counts as done.
    Skip,
}

/// The dialog's state, kept between frames.
#[derive(Default)]
pub(super) struct SetupDialog {
    pub(super) open: bool,
    /// The Kovan folder path, typed or picked.
    pub(super) folder: String,
    open_remote: String,
    proprietary_remote: String,
    message: String,
}

/// A trimmed remote URL, or `None` for an empty field.
fn remote(field: &str) -> Option<String> {
    let t = field.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl SetupDialog {
    /// Open the dialog, prefilled from `root` when a Kovan folder is already
    /// open (so reopening it shows the current remotes).
    pub(super) fn show_for(&mut self, root: Option<&KovanRoot>) {
        self.open = true;
        self.message.clear();
        if let Some(root) = root {
            self.folder = root.path().display().to_string();
            let c = &root.config().corpora;
            self.open_remote = c.open_remote.clone().unwrap_or_default();
            self.proprietary_remote = c.proprietary_remote.clone().unwrap_or_default();
        }
    }

    /// Show a message (a validation problem, or the outcome) in the dialog.
    pub(super) fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Draw the dialog, if open. Returns what the user asked for this frame.
    pub(super) fn ui(&mut self, ctx: &egui::Context) -> Option<SetupRequest> {
        if !self.open {
            return None;
        }
        let mut request = None;
        egui::Window::new("Set up Kovan")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    "Kovan always shows its built-in nuclear-engineering map. Set up \
                     your own Kovan folder to add your knowledge on top of it.",
                );
                ui.add_space(8.0);

                ui.strong("1. Kovan folder (required)");
                ui.weak("An existing Kovan folder is opened; any other folder becomes a new one.");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.folder).desired_width(360.0));
                    if ui.button("Browse…").clicked() {
                        request = Some(SetupRequest::Browse);
                    }
                });
                ui.add_space(6.0);

                ui.strong("2. Open corpus (optional)");
                ui.weak(
                    "A GitHub repository of redistributable literature: PDFs at its root, \
                     or in top-level folders whose name contains \"open-corpus\". Left \
                     empty, a local repository is created for you to push later.",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.open_remote)
                        .hint_text("https://github.com/you/your-open-corpus.git")
                        .desired_width(420.0),
                );
                ui.add_space(6.0);

                ui.strong("3. Proprietary corpus (optional, must be a PRIVATE repository)");
                ui.weak(
                    "Literature you may not redistribute. Kovan never commits it to your \
                     Kovan folder. Left empty, a local repository is created instead.",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.proprietary_remote)
                        .hint_text("https://github.com/you/your-private-corpus.git")
                        .desired_width(420.0),
                );
                ui.weak("URLs only: never put a password or token here.");

                if !self.message.is_empty() {
                    ui.add_space(6.0);
                    ui.colored_label(ui.visuals().warn_fg_color, &self.message);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Set up").clicked() {
                        let folder = self.folder.trim();
                        if folder.is_empty() {
                            self.message = "Choose a Kovan folder first.".into();
                        } else if [&self.open_remote, &self.proprietary_remote]
                            .iter()
                            .any(|r| r.trim().contains(char::is_whitespace))
                        {
                            self.message = "A repository URL cannot contain spaces.".into();
                        } else {
                            request = Some(SetupRequest::Finish {
                                folder: PathBuf::from(folder),
                                open_remote: remote(&self.open_remote),
                                proprietary_remote: remote(&self.proprietary_remote),
                            });
                        }
                    }
                    if ui.button("Skip for now").clicked() {
                        request = Some(SetupRequest::Skip);
                    }
                });
            });
        request
    }
}

/// The file whose presence records that first run is over, in Kovan's
/// platform config folder (beside `recent_roots.toml`).
fn first_run_marker() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")
        .map(|d| d.config_dir().join("setup_done"))
}

/// Whether this is Kovan's first run (the setup dialog has never been
/// finished or skipped). `false` when there is no config folder, so a
/// platform without one is not nagged every launch.
pub(super) fn is_first_run() -> bool {
    first_run_marker().is_some_and(|p| !p.exists())
}

/// Record that first run is over. Best effort: failing to write the marker
/// only means the dialog shows again next launch.
pub(super) fn mark_first_run_done() {
    if let Some(p) = first_run_marker() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, "Kovan setup was finished or skipped.\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_remote_fields_mean_no_remote() {
        assert_eq!(remote("   "), None);
        assert_eq!(
            remote("  https://github.com/a/b.git "),
            Some("https://github.com/a/b.git".to_string())
        );
    }

    /// Reopening the dialog on an open folder shows its current remotes.
    #[test]
    fn reopening_prefills_from_the_open_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let mut root = KovanRoot::create(
            tmp.path(),
            crate::root::RootConfig::new("lib", "Lib"),
            false,
        )
        .unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: Some("https://example.com/o.git".into()),
            proprietary_remote: None,
        })
        .unwrap();
        let mut d = SetupDialog::default();
        d.show_for(Some(&root));
        assert!(d.open);
        assert_eq!(d.open_remote, "https://example.com/o.git");
        assert_eq!(d.proprietary_remote, "");
        assert_eq!(d.folder, tmp.path().display().to_string());
    }
}
