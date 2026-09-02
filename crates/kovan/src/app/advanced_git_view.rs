//! GUI for the "Save Repository" tab (§38, `op-9vo6.20`; reframed by
//! op-wqaw, GH issue #35's 2026-09-01 checkpoint §21) — a thin view over
//! [`crate::advanced_git`]'s library functions. All the actual git work
//! (local via `gix`, remote via the system `git` binary) lives there; this
//! module only renders results and forwards button clicks, on demand
//! rather than every frame, so opening this tab doesn't spawn a `git`
//! subprocess on every repaint.
//!
//! # "Save Repository" is the primary frame, not "Git" (op-wqaw)
//!
//! Most users shouldn't need Git vocabulary to save their work — the
//! checkpoint's own words: "Most users should not need Git vocabulary."
//! [`AdvancedGitState::ui`] therefore leads with "Changes since last save"
//! and a single prominent Save Repository button; branches/history/remotes/
//! fetch-pull-push (real Git concepts, §38) sit inside a collapsed
//! "Advanced…" section underneath, not as the tab's headline. Nothing about
//! `crate::advanced_git`/`crate::repository`'s own behaviour changed here —
//! this file is presentation only.

use eframe::egui::{self, Color32};

use crate::advanced_git::{self, BranchInfo, RemoteInfo};
use crate::repository::SaveSummary;
use crate::root::KovanRoot;
use kovan_discovery::git::CommitInfo;

#[derive(Default)]
pub struct AdvancedGitState {
    /// Whether [`Self::refresh`] has been called at least once for the
    /// currently open root — gates the auto-load in [`Self::ui`] so a repo
    /// that keeps failing to load (not a git repository, e.g.) is retried
    /// only on an explicit Refresh click, not every single frame.
    loaded_once: bool,
    /// Set by [`Self::mark_stale`] when the app has written a tracked repo
    /// file since the last refresh (a Save Document, an annotation/CSV save,
    /// a `.bib` edit — GH issue #35 2026-09-02: "on every save, git status
    /// should be auto-run for the Save Repository tab"). Consumed by the
    /// next [`Self::ui`], which re-scans and clears it.
    stale: bool,
    status: Option<SaveSummary>,
    branches: Vec<BranchInfo>,
    history: Vec<CommitInfo>,
    remotes: Vec<RemoteInfo>,
    remote_input: String,
    branch_input: String,
    message: String,
    message_is_error: bool,
}

impl AdvancedGitState {
    fn refresh(&mut self, root: &KovanRoot) {
        self.loaded_once = true;
        self.stale = false;
        match advanced_git::status(root) {
            Ok(s) => self.status = Some(s),
            Err(e) => self.set_error(e.to_string()),
        }
        self.branches = advanced_git::local_branches(root).unwrap_or_default();
        self.history = advanced_git::history(root, 20).unwrap_or_default();
        self.remotes = advanced_git::list_remotes(root).unwrap_or_default();
    }

    /// Mark the git status stale so the next [`Self::ui`] re-scans — call
    /// this from the app whenever a save has just written a tracked repo
    /// file. Cheap (a bool); the actual `git status` only runs when this
    /// tab is next drawn.
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    fn set_status(&mut self, m: impl Into<String>) {
        self.message = m.into();
        self.message_is_error = false;
    }

    fn set_error(&mut self, m: impl Into<String>) {
        self.message = m.into();
        self.message_is_error = true;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, root: &KovanRoot) {
        // op-wqaw: load the status the first time this tab is shown, so
        // "Changes since last save" has something real on it immediately —
        // the checkpoint's own point that most users shouldn't need to know
        // "Refresh" is a Git-status re-scan before they can even see it.
        if !self.loaded_once || self.stale {
            self.stale = false;
            self.refresh(root);
        }

        ui.heading("Save Repository");
        ui.small("Version history is kept using a Git backend.");
        ui.add_space(4.0);

        ui.strong("Changes since last save");
        match &self.status {
            Some(s) if s.is_empty() => {
                ui.weak("clean — nothing to save");
            }
            Some(s) => {
                for a in &s.added {
                    ui.label(format!("+ {a}"));
                }
                for c in &s.changed {
                    ui.label(format!("~ {c}"));
                }
                for r in &s.removed {
                    ui.label(format!("- {r}"));
                }
            }
            None => {
                ui.weak("(loading…)");
            }
        }
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            // op-nswf, GH issue #35 2026-09-01 05:42: "Under the git tab, i
            // expect to see save to repository. I don't see any button" —
            // the backend (`crate::repository::save_repository`) already
            // existed and was tested; it just had no button wired to it.
            if ui.button("Save Repository").clicked() {
                match advanced_git::save(root) {
                    Ok(Some(summary)) => {
                        self.set_status(format!(
                            "saved: {} added, {} changed, {} removed",
                            summary.added.len(),
                            summary.changed.len(),
                            summary.removed.len()
                        ));
                        self.refresh(root);
                    }
                    Ok(None) => self.set_status("nothing to save — already up to date"),
                    Err(e) => self.set_error(e.to_string()),
                }
            }
            if ui.button("Refresh").clicked() {
                self.refresh(root);
            }
        });
        if !self.message.is_empty() {
            let color = if self.message_is_error { Color32::from_rgb(220, 90, 90) } else { ui.visuals().weak_text_color() };
            ui.colored_label(color, &self.message);
        }
        ui.separator();

        egui::CollapsingHeader::new("Advanced…").default_open(false).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.strong("Branches");
                for b in &self.branches {
                    ui.label(if b.is_current { format!("* {}", b.name) } else { format!("  {}", b.name) });
                }

                ui.add_space(8.0);
                ui.strong("History");
                for c in &self.history {
                    ui.label(format!("{} {}", c.short_id, c.summary));
                }

                ui.add_space(8.0);
                ui.strong("Remotes (system git)");
                if !advanced_git::system_git_available() {
                    ui.weak("system git not found — Kovan still works; remote operations are unavailable");
                }
                for r in &self.remotes {
                    ui.label(format!("{}: {}", r.name, r.url));
                }
                ui.horizontal(|ui| {
                    ui.label("remote:");
                    ui.text_edit_singleline(&mut self.remote_input);
                    ui.label("branch:");
                    ui.text_edit_singleline(&mut self.branch_input);
                });
                ui.horizontal(|ui| {
                    if ui.button("Fetch").clicked() {
                        self.run(root, |r, remote, _| advanced_git::fetch(r, remote));
                    }
                    if ui.button("Pull").clicked() {
                        self.run(root, advanced_git::pull);
                    }
                    if ui.button("Push").clicked() {
                        self.run(root, advanced_git::push);
                    }
                });
            });
        });
    }

    fn run(&mut self, root: &KovanRoot, op: impl Fn(&KovanRoot, &str, &str) -> Result<String, advanced_git::RemoteError>) {
        match op(root, &self.remote_input, &self.branch_input) {
            Ok(_) => self.set_status("done"),
            Err(e) => self.set_error(e.to_string()),
        }
    }
}
