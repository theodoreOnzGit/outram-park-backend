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
//! and a single prominent Save Repository button; branches/history and the
//! per-repository fetch-pull-push panels (real Git concepts, §38; since #255
//! one panel each for the Kovan folder and its two corpus repositories, with
//! the remote and branch taken from Git, never typed) sit inside a collapsed
//! "Advanced…" section underneath, not as the tab's headline. Nothing about
//! `crate::advanced_git`/`crate::repository`'s own behaviour changed here —
//! this file is presentation only.

use eframe::egui::{self, Color32};

use crate::advanced_git::{self, BranchInfo, RemoteInfo};
use std::path::PathBuf;

/// One repository the tab can fetch, pull and push: the Kovan folder itself,
/// or one of its corpus repositories (#255).
///
/// The remote and branch are **never typed by the user** (maintainer
/// direction, 2026-09-22): the repositories were decided in the setup dialog,
/// so each panel uses the repository's own `origin` and the branch it has
/// checked out, read from Git on refresh.
struct RepoPanel {
    label: &'static str,
    dir: PathBuf,
    remotes: Vec<RemoteInfo>,
    /// The branch checked out, if any.
    branch: Option<String>,
}

impl RepoPanel {
    fn load(label: &'static str, dir: PathBuf) -> Self {
        Self {
            label,
            remotes: advanced_git::list_remotes_in(&dir).unwrap_or_default(),
            branch: advanced_git::current_branch_in(&dir),
            dir,
        }
    }

    /// The remote to use: `origin` if present, otherwise the only remote.
    fn remote(&self) -> Option<&RemoteInfo> {
        self.remotes
            .iter()
            .find(|r| r.name == "origin")
            .or_else(|| (self.remotes.len() == 1).then(|| &self.remotes[0]))
    }
}
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
    /// The Kovan folder, then each corpus repository that exists.
    repos: Vec<RepoPanel>,
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
        self.repos = std::iter::once(("Kovan folder", root.path().to_path_buf()))
            .chain([
                ("Open corpus", root.open_corpus_dir()),
                ("Proprietary corpus", root.restricted_sources_dir()),
            ])
            .filter(|(_, dir)| crate::corpus_repos::is_git_repo(dir))
            .map(|(label, dir)| RepoPanel::load(label, dir))
            .collect();
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
            let color = if self.message_is_error {
                Color32::from_rgb(220, 90, 90)
            } else {
                ui.visuals().weak_text_color()
            };
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
                ui.strong("Repositories (system git)");
                if !advanced_git::system_git_available() {
                    ui.weak("system git not found — Kovan still works; remote operations are unavailable");
                }
                let mut action: Option<(usize, RemoteOp)> = None;
                for (i, repo) in self.repos.iter().enumerate() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(repo.label).strong());
                    ui.weak(repo.dir.display().to_string());
                    match (repo.remote(), &repo.branch) {
                        (Some(remote), Some(branch)) => {
                            ui.label(format!("{} ({}), branch {branch}", remote.url, remote.name));
                            ui.horizontal(|ui| {
                                if ui.button("Fetch").clicked() {
                                    action = Some((i, RemoteOp::Fetch));
                                }
                                if ui.button("Pull").clicked() {
                                    action = Some((i, RemoteOp::Pull));
                                }
                                if ui.button("Push").clicked() {
                                    action = Some((i, RemoteOp::Push));
                                }
                            });
                        }
                        (None, _) => {
                            ui.weak("no GitHub repository connected — add its URL in \u{2699} Setup");
                        }
                        (Some(remote), None) => {
                            ui.label(format!("{} ({})", remote.url, remote.name));
                            ui.weak("no branch checked out yet — save something first");
                        }
                    }
                }
                if let Some((i, op)) = action {
                    self.run(i, op);
                }
            });
        });
    }

    /// Fetch, pull or push repository `i` against its own remote and branch.
    fn run(&mut self, i: usize, op: RemoteOp) {
        let Some(repo) = self.repos.get(i) else {
            return;
        };
        let (Some(remote), Some(branch)) = (repo.remote(), repo.branch.as_deref()) else {
            return;
        };
        let (remote, label) = (remote.name.clone(), repo.label);
        let result = match op {
            RemoteOp::Fetch => advanced_git::fetch_in(&repo.dir, &remote),
            RemoteOp::Pull => advanced_git::pull_in(&repo.dir, &remote, branch),
            RemoteOp::Push => advanced_git::push_in(&repo.dir, &remote, branch),
        };
        match result {
            Ok(_) => self.set_status(format!("{label}: {} done", op.verb())),
            Err(e) => self.set_error(format!("{label}: {e}")),
        }
    }
}

/// A remote operation a repository panel can run.
#[derive(Debug, Clone, Copy)]
enum RemoteOp {
    Fetch,
    Pull,
    Push,
}

impl RemoteOp {
    fn verb(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Pull => "pull",
            Self::Push => "push",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(names: &[&str]) -> RepoPanel {
        RepoPanel {
            label: "test",
            dir: PathBuf::new(),
            remotes: names
                .iter()
                .map(|n| RemoteInfo {
                    name: n.to_string(),
                    url: format!("https://example.com/{n}.git"),
                })
                .collect(),
            branch: Some("main".into()),
        }
    }

    /// The panel pushes to `origin`, or to the only remote there is; with
    /// several and no `origin` it does not guess.
    #[test]
    fn the_remote_is_origin_or_the_only_one_never_a_guess() {
        assert_eq!(
            panel(&["upstream", "origin"]).remote().unwrap().name,
            "origin"
        );
        assert_eq!(panel(&["github"]).remote().unwrap().name, "github");
        assert!(panel(&["a", "b"]).remote().is_none());
        assert!(panel(&[]).remote().is_none());
    }
}
