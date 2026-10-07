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
//!
//! # Push after save (2026-09-28)
//!
//! A "Push after save" checkbox, **ticked by default**, sits under the note
//! box. Ticked, Save Repository also pushes the proprietary corpus, the open
//! corpus and the Kovan repository ([`crate::save_push`] has the order and
//! the safety rules), and each repository's result — pushed, nothing to
//! push, not pushed and why, refused, failed with Git's words — is listed
//! under the button. Unticking it is persisted as
//! `[save] push_after_save = false` in the library's `kovan_root.toml`
//! (so it travels with the library), via
//! [`crate::root::KovanRoot::set_push_after_save`].
//!
//! # Pulling the Kovan folder pulls its corpora (GH issue #422)
//!
//! A successful Pull of the Kovan folder (ordinary, or forced through the
//! prompt below) runs [`crate::save_push::pull_corpora`]: each corpus that
//! is merely behind is fast-forwarded onto its branch, one line per corpus
//! is listed under the pull, and each corpus whose local work would be
//! destroyed gets the same "sure anot?" prompt, queued one at a time.
//!
//! # Git runs off the GUI thread (maintainer, 2026-09-29)
//!
//! Save Repository (with its push), Fetch, Pull, Push and the forced pull
//! run on a background thread ([`AdvancedGitState::spawn`]); the tab shows
//! a spinner and disables those buttons until [`AdvancedGitState::poll_job`]
//! sees it finish and records the result, so a slow network never freezes
//! the window. One job at a time, and no "sure anot?" prompt is shown while
//! one runs.
//!
//! # One exception to "presentation only": the conflicted-pull prompt
//!
//! Since GH issue #279 this file also owns a decision, not just a
//! rendering: a pull that comes back [`advanced_git::RemoteError::Conflict`]
//! raises [`ForcePullPrompt`] instead of a red label, and the answer runs
//! either `advanced_git::force_pull_in` (**destroys every uncommitted and
//! untracked file in the folder**) or `advanced_git::abort_in_progress_in`.
//! The git itself still lives in `advanced_git`; what lives here is the
//! guarantee that neither runs without the user having been shown, in
//! words, what "yes, can" throws away.

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
    label: String,
    dir: PathBuf,
    remotes: Vec<RemoteInfo>,
    /// The branch checked out, if any.
    branch: Option<String>,
}

impl RepoPanel {
    fn load(label: String, dir: PathBuf) -> Self {
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
    /// The user's own commit note, typed into the "what did you do?" box
    /// (maintainer request, 2026-09-28). Appended to the body of every
    /// commit the next Save Repository makes, under the unchanged generated
    /// subject ([`crate::repository::compose_commit_message`]). Cleared only
    /// after a save that actually committed; kept on failure and on a
    /// nothing-to-save, so what the user typed is never lost.
    commit_note: String,
    /// The "Push after save" checkbox: loaded from the library's
    /// `kovan_root.toml` on every refresh (default ON) and written back when
    /// the user changes it. See the module doc.
    push_after_save: bool,
    /// A pull Git stopped on a conflict, waiting for the user's answer to
    /// the "sure anot?" prompt (GH issue #279). `None` the rest of the time.
    pending_force_pull: Option<ForcePullPrompt>,
    /// Further prompts waiting their turn behind [`Self::pending_force_pull`]
    /// — raised when pulling the Kovan folder finds more than one corpus
    /// whose local work a pull would destroy (#422). Asked one at a time.
    queued_force_pulls: std::collections::VecDeque<ForcePullPrompt>,
    /// The save, fetch, pull or push running on a background thread, if
    /// any — network Git can take seconds to minutes, and the window must
    /// not freeze meanwhile (maintainer, 2026-09-29). One at a time: the
    /// buttons are disabled until it lands in [`Self::poll_job`].
    job: Option<GitJob>,
}

/// A Git operation running off the GUI thread, and what to say meanwhile.
struct GitJob {
    /// e.g. "pulling Kovan folder…".
    what: String,
    handle: std::thread::JoinHandle<GitJobDone>,
}

/// What a finished [`GitJob`] hands back to the GUI thread to record.
enum GitJobDone {
    /// A panel's fetch/pull/push; `corpora` is set when the Kovan folder
    /// was pulled and its corpora followed it (#422).
    Remote {
        label: String,
        dir: PathBuf,
        remote: String,
        branch: String,
        op: RemoteOp,
        result: Result<String, advanced_git::RemoteError>,
        kovan_folder: bool,
        corpora: Option<Vec<crate::save_push::CorpusPull>>,
    },
    /// Save Repository, with push-after-save when it is on.
    Save {
        result: Result<Option<SaveSummary>, crate::repository::RepositoryError>,
        pushed: Option<crate::save_push::PushReport>,
    },
    /// The "yes, can" forced pull.
    ForcePull {
        prompt: ForcePullPrompt,
        result: Result<String, advanced_git::RemoteError>,
        corpora: Option<Vec<crate::save_push::CorpusPull>>,
    },
}

/// The pull that just hit a conflict, held while [`AdvancedGitState::force_pull_prompt_ui`]
/// asks whether to force it through (GH issue #279).
///
/// The repository is carried here by value rather than as an index into
/// [`AdvancedGitState::repos`], because a refresh between raising the prompt
/// and answering it would renumber that list — and answering "yes, can" is
/// destructive enough that it must not be able to land on a different
/// repository than the one the user was shown.
struct ForcePullPrompt {
    label: String,
    dir: PathBuf,
    remote: String,
    branch: String,
    /// Git's own account of the conflict, shown on request rather than by
    /// default — it is the evidence, not the question.
    git_says: String,
    /// Whether a forced pull here is of the Kovan folder itself, whose
    /// corpora then follow it ([`crate::save_push::pull_corpora`], #422).
    follow_corpora: bool,
}

impl AdvancedGitState {
    fn refresh(&mut self, root: &KovanRoot) {
        self.loaded_once = true;
        self.stale = false;
        self.push_after_save = advanced_git::push_after_save_setting(root);
        match advanced_git::status(root) {
            Ok(s) => self.status = Some(s),
            Err(e) => self.set_error(e.to_string()),
        }
        self.branches = advanced_git::local_branches(root).unwrap_or_default();
        self.history = advanced_git::history(root, 20).unwrap_or_default();
        // Every open and proprietary repository (#458); a tier with one
        // repository keeps its plain label.
        let corpus = root.corpus_repos();
        let mut panels: Vec<(String, PathBuf)> =
            vec![("Kovan folder".to_string(), root.path().to_path_buf())];
        for tier in [
            crate::corpus_tiers::Tier::Open,
            crate::corpus_tiers::Tier::Proprietary,
        ] {
            let in_tier: Vec<_> = corpus.iter().filter(|r| r.tier == tier).collect();
            for r in &in_tier {
                let label = if in_tier.len() == 1 {
                    tier.label().to_string()
                } else {
                    r.label()
                };
                panels.push((label, r.dir.clone()));
            }
        }
        self.repos = panels
            .into_iter()
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
        self.poll_job(root);
        let busy = self.job.is_some();
        if busy {
            // Nothing else repaints an idle window; keep checking the job.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
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
                // Maintainer, 2026-09-30: on a fresh Kovan folder the list
                // "floods" the tab and pushes the Save Repository button off
                // the bottom ("I cannot even find the save repository
                // button"). The list therefore gets its OWN scroll area,
                // capped in height, so the note box and the button below it
                // stay on screen however many files changed. A one-line count
                // comes first, so the size is visible without scrolling.
                ui.weak(format!(
                    "{} added, {} changed, {} removed",
                    s.added.len(),
                    s.changed.len(),
                    s.removed.len()
                ));
                egui::ScrollArea::vertical()
                    .id_salt("save_repository_changes")
                    .max_height(changes_list_max_height(ui.available_height()))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for a in &s.added {
                            ui.label(format!("+ {a}"));
                        }
                        for c in &s.changed {
                            ui.label(format!("~ {c}"));
                        }
                        for r in &s.removed {
                            ui.label(format!("- {r}"));
                        }
                    });
            }
            None => {
                ui.weak("(loading…)");
            }
        }
        ui.add_space(4.0);

        ui.add(
            egui::TextEdit::multiline(&mut self.commit_note)
                .hint_text("what did you do? (optional)")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.small("Added to the commit message under \"Save Kovan repository\".");
        ui.add_space(4.0);

        if ui
            .checkbox(&mut self.push_after_save, "Push after save")
            .on_hover_text(
                "Also push the proprietary corpus, the open corpus and this Kovan folder. \
                 Each corpus goes only to its own remote in kovan_root.toml; never forced.",
            )
            .changed()
        {
            self.persist_push_setting(root);
        }
        ui.add_space(4.0);

        let mut save_result = None;
        ui.horizontal(|ui| {
            // op-nswf, GH issue #35 2026-09-01 05:42: "Under the git tab, i
            // expect to see save to repository. I don't see any button" —
            // the backend (`crate::repository::save_repository`) already
            // existed and was tested; it just had no button wired to it.
            if ui
                .add_enabled(!busy, egui::Button::new("Save Repository"))
                .clicked()
            {
                save_result = Some(());
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh"))
                .clicked()
            {
                self.refresh(root);
            }
            if let Some(job) = &self.job {
                ui.spinner();
                ui.weak(&job.what);
            }
        });
        if save_result.is_some() {
            let (root, note, push) = (root.clone(), self.commit_note.clone(), self.push_after_save);
            self.spawn(
                if push {
                    "saving and pushing…"
                } else {
                    "saving…"
                }
                .into(),
                move || {
                    let (result, pushed) = advanced_git::save_and_push(&root, &note, push);
                    GitJobDone::Save { result, pushed }
                },
            );
        }
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
                    ui.label(egui::RichText::new(&repo.label).strong());
                    ui.weak(repo.dir.display().to_string());
                    match (repo.remote(), &repo.branch) {
                        (Some(remote), Some(branch)) => {
                            ui.label(format!("{} ({}), branch {branch}", remote.url, remote.name));
                            ui.horizontal(|ui| {
                                for op in [RemoteOp::Fetch, RemoteOp::Pull, RemoteOp::Push] {
                                    if ui.add_enabled(!busy, egui::Button::new(op.button())).clicked() {
                                        action = Some((i, op));
                                    }
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
                    self.run(root, i, op);
                }
            });
        });

        // Modal, so it is drawn on the context rather than inside the
        // collapsed "Advanced…" section that raised it — a prompt that can
        // destroy the folder must not be something the user can scroll away
        // from without answering (#279).
        let ctx = ui.ctx().clone();
        self.force_pull_prompt_ui(&ctx, root);
    }

    /// Write the checkbox to `kovan_root.toml`; on failure, say so and put
    /// the checkbox back to what the file still says.
    fn persist_push_setting(&mut self, root: &KovanRoot) {
        let wanted = self.push_after_save;
        let written = KovanRoot::open(root.path())
            .map_err(|e| e.to_string())
            .and_then(|mut fresh| fresh.set_push_after_save(wanted));
        match written {
            Ok(()) => self.set_status(if wanted {
                "push after save: on"
            } else {
                "push after save: off — saves stay on this computer until you push"
            }),
            Err(e) => {
                self.push_after_save = advanced_git::push_after_save_setting(root);
                self.set_error(format!("could not save the push setting: {e}"));
            }
        }
    }

    /// File what a Save Repository returned, and decide the commit note's
    /// fate: cleared only when something was committed, kept otherwise (a
    /// failure, or nothing to save) so the user never loses what they typed.
    /// Returns whether a commit was made (the caller then re-scans). Split
    /// out from [`Self::ui`] so it is testable without a repository.
    ///
    /// `pushed` is the push-after-save report, if a push ran: one line per
    /// repository is appended to the message, which turns red when any push
    /// failed or was refused. The note's fate does not depend on the push —
    /// the commit it went into exists either way.
    fn record_save(
        &mut self,
        result: Result<Option<crate::repository::SaveSummary>, crate::repository::RepositoryError>,
        pushed: Option<crate::save_push::PushReport>,
    ) -> bool {
        let committed = self.record_commit(result);
        if let Some(report) = pushed {
            for line in report.lines() {
                self.message.push('\n');
                self.message.push_str(&line);
            }
            if report.has_problem() {
                self.message_is_error = true;
            }
        }
        committed
    }

    /// The commit half of [`Self::record_save`].
    fn record_commit(
        &mut self,
        result: Result<Option<crate::repository::SaveSummary>, crate::repository::RepositoryError>,
    ) -> bool {
        match result {
            Ok(Some(summary)) => {
                self.set_status(format!(
                    "saved: {} added, {} changed, {} removed",
                    summary.added.len(),
                    summary.changed.len(),
                    summary.removed.len()
                ));
                self.commit_note.clear();
                true
            }
            Ok(None) => {
                self.set_status("nothing to save — already up to date");
                false
            }
            Err(e) => {
                self.set_error(e.to_string());
                false
            }
        }
    }

    /// Fetch, pull or push repository `i` against its own remote and branch.
    fn run(&mut self, root: &KovanRoot, i: usize, op: RemoteOp) {
        let Some(repo) = self.repos.get(i) else {
            return;
        };
        let (Some(remote), Some(branch)) = (repo.remote(), repo.branch.as_deref()) else {
            return;
        };
        let (remote, branch, label, dir) = (
            remote.name.clone(),
            branch.to_string(),
            repo.label.clone(),
            repo.dir.clone(),
        );
        let kovan_folder = dir == root.path();
        let root = root.clone();
        self.spawn(format!("{}ing {label}…", op.verb()), move || {
            let result = match op {
                RemoteOp::Fetch => advanced_git::fetch_in(&dir, &remote),
                RemoteOp::Pull => advanced_git::pull_in(&dir, &remote, &branch),
                RemoteOp::Push => advanced_git::push_in(&dir, &remote, &branch),
            };
            // #422: pulling the Kovan folder pulls its corpora too.
            let corpora = (kovan_folder && matches!(op, RemoteOp::Pull) && result.is_ok())
                .then(|| crate::save_push::pull_corpora(&root));
            GitJobDone::Remote {
                label,
                dir,
                remote,
                branch,
                op,
                result,
                kovan_folder,
                corpora,
            }
        });
    }

    /// Run `work` on a background thread; [`Self::poll_job`] records it.
    fn spawn(&mut self, what: String, work: impl FnOnce() -> GitJobDone + Send + 'static) {
        self.job = Some(GitJob {
            what,
            handle: std::thread::spawn(work),
        });
    }

    /// Record the background job once it has finished; a no-op while it
    /// runs. Also raises the next queued corpus prompt (#422), which waits
    /// for the job so two Git operations never run at once.
    fn poll_job(&mut self, root: &KovanRoot) {
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            if let Some(job) = self.job.take() {
                match job.handle.join() {
                    Ok(done) => self.record_job(done, root),
                    Err(_) => self.set_error(format!("{} failed unexpectedly", job.what)),
                }
            }
        }
        if self.job.is_none() && self.pending_force_pull.is_none() {
            self.pending_force_pull = self.queued_force_pulls.pop_front();
        }
    }

    /// Record what a finished background job returned.
    fn record_job(&mut self, done: GitJobDone, root: &KovanRoot) {
        match done {
            GitJobDone::Remote {
                label,
                dir,
                remote,
                branch,
                op,
                result,
                kovan_folder,
                corpora,
            } => {
                self.record(label, dir, remote, branch, op, result);
                if let Some(prompt) = self.pending_force_pull.as_mut() {
                    prompt.follow_corpora = kovan_folder;
                }
                if let Some(corpora) = corpora {
                    self.follow_corpora(corpora);
                }
                if matches!(op, RemoteOp::Pull) {
                    self.refresh(root);
                }
            }
            GitJobDone::Save { result, pushed } => {
                if self.record_save(result, pushed) {
                    self.refresh(root);
                }
            }
            GitJobDone::ForcePull {
                prompt,
                result,
                corpora,
            } => {
                match result {
                    // The files on disk were replaced underneath the rest
                    // of the app, whose index and any open paper were read
                    // before the pull. Refreshing this tab does not reload
                    // those, so say so rather than letting a stale Wiki look
                    // like the pull did not work. (True of an ordinary
                    // successful pull too — this is the loudest case, not a
                    // new one.)
                    // `done` names any `kovan-kept/…` branch the unpushed
                    // saves were moved onto (GH #502), so it is shown.
                    Ok(done) => self.set_status(format!(
                        "{}: pulled by force — {done}. Reopen the folder from Home so the rest \
                         of Kovan reads the new files.",
                        prompt.label
                    )),
                    Err(e) => {
                        self.set_error(format!("{}: could not force the pull: {e}", prompt.label))
                    }
                }
                // #422: the Kovan folder was replaced; its corpora follow.
                if let Some(corpora) = corpora {
                    self.follow_corpora(corpora);
                }
                // The working tree and the history both moved; everything on
                // this tab is now stale.
                self.refresh(root);
            }
        }
    }

    /// File what [`crate::save_push::pull_corpora`] did after the Kovan
    /// folder was pulled (#422): one line per corpus under the pull's
    /// message, and — for each corpus whose local work a pull would destroy
    /// — the "sure anot?" prompt, queued, so overriding it is still the
    /// user's answer and never automatic. Split out so it is testable
    /// without a repository.
    fn follow_corpora(&mut self, pulled: Vec<crate::save_push::CorpusPull>) {
        use crate::save_push::CorpusPullOutcome;
        for p in pulled {
            self.message.push('\n');
            self.message.push_str(&p.line());
            match p.outcome {
                CorpusPullOutcome::Failed { .. } => self.message_is_error = true,
                CorpusPullOutcome::NeedsConfirmation {
                    remote,
                    branch,
                    reason,
                } => {
                    let label = if p.name.is_empty() {
                        p.corpus.label().to_string()
                    } else {
                        format!("{} ({})", p.corpus.label(), p.name)
                    };
                    let prompt = ForcePullPrompt {
                        label,
                        dir: p.dir,
                        remote,
                        branch,
                        git_says: format!("Not pulled, because {reason}."),
                        follow_corpora: false,
                    };
                    if self.pending_force_pull.is_none() {
                        self.pending_force_pull = Some(prompt);
                    } else {
                        self.queued_force_pulls.push_back(prompt);
                    }
                }
                _ => {}
            }
        }
    }

    /// File what a remote operation returned: a message, or — for the one
    /// failure the user can answer (a conflicted pull, #279) — the prompt.
    ///
    /// Split out from [`Self::run`] so the branch that decides *between*
    /// those two is testable without a repository or a running `git`.
    fn record(
        &mut self,
        label: String,
        dir: PathBuf,
        remote: String,
        branch: String,
        op: RemoteOp,
        result: Result<String, advanced_git::RemoteError>,
    ) {
        match result {
            Ok(_) => self.set_status(format!("{label}: {} done", op.verb())),
            // Only `pull_in` ever produces this, and it is not an error the
            // user can do nothing about — ask, rather than printing Git's
            // complaint in red and leaving them stuck.
            Err(advanced_git::RemoteError::Conflict { output, .. }) => {
                self.message.clear();
                self.pending_force_pull = Some(ForcePullPrompt {
                    label,
                    dir,
                    remote,
                    branch,
                    git_says: output,
                    follow_corpora: false,
                });
            }
            Err(e) => self.set_error(format!("{label}: {e}")),
        }
    }

    /// The conflicted-pull prompt (GH issue #279), in the maintainer's own
    /// words: *"you may have unsaved changes, u sure u want to pull anot?"*,
    /// answered with **yes, can** or **no, i manage myself**.
    ///
    /// - **yes, can** — [`advanced_git::force_pull_in`]: the folder becomes
    ///   an exact copy of the remote, and everything not saved is destroyed
    ///   (~~not saved *and* pushed~~ — since GH #502 unpushed saves are kept
    ///   on a `kovan-kept/…` branch). The dialog says so before the button is reachable;
    ///   this is the one place in the tab that can lose work.
    /// - **no, i manage myself** — [`advanced_git::abort_in_progress_in`]:
    ///   the folder goes back to how it was before Pull was pressed, so
    ///   declining means the pull simply did not happen. Without this a
    ///   conflicted merge would be left half-applied in a folder whose
    ///   owner was never meant to need Git vocabulary (op-wqaw).
    fn force_pull_prompt_ui(&mut self, ctx: &egui::Context, root: &KovanRoot) {
        if self.job.is_some() {
            return;
        }
        let Some(prompt) = self.pending_force_pull.take() else {
            return;
        };
        let mut force = None;
        // A real `egui::Modal`, not a `Window`: its backdrop blocks input to
        // everything behind it, and its `should_close()` (escape, or a click
        // on the backdrop) is deliberately ignored — a prompt that can
        // destroy the folder is answered with one of the two buttons or not
        // at all.
        egui::Modal::new(egui::Id::new("advanced-git-force-pull"))
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading("Pull anyway?");
                ui.label(format!("{} — {}", prompt.label, prompt.dir.display()));
                ui.add_space(4.0);
                ui.strong("You may have unsaved changes, u sure u want to pull anot?");
                ui.add_space(4.0);
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!(
                        "\u{26a0} \"yes, can\" replaces this folder with {}/{}. Anything not \
                         saved is gone for good — edits in progress and files you added but \
                         never saved. Saves that were never pushed are moved aside onto a \
                         kovan-kept/… branch (named once the pull is done), not deleted.",
                        prompt.remote, prompt.branch
                    ),
                );
                ui.weak("\"no, i manage myself\" leaves the folder exactly as it was before you pressed Pull.");
                egui::CollapsingHeader::new("What Git said")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.weak(&prompt.git_says);
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("yes, can").clicked() {
                        force = Some(true);
                    }
                    if ui.button("no, i manage myself").clicked() {
                        force = Some(false);
                    }
                });
            });

        match force {
            Some(true) => {
                let root = root.clone();
                self.spawn(format!("pulling {} by force…", prompt.label), move || {
                    let result =
                        advanced_git::force_pull_in(&prompt.dir, &prompt.remote, &prompt.branch);
                    let corpora = (prompt.follow_corpora && result.is_ok())
                        .then(|| crate::save_push::pull_corpora(&root));
                    GitJobDone::ForcePull {
                        prompt,
                        result,
                        corpora,
                    }
                });
            }
            Some(false) => match advanced_git::abort_in_progress_in(&prompt.dir) {
                Ok(_) => self.set_status(format!(
                    "{}: pull cancelled — nothing in the folder was changed",
                    prompt.label
                )),
                Err(e) => self.set_error(format!(
                    "{}: pull cancelled, but undoing the half-done merge failed: {e}",
                    prompt.label
                )),
            },
            // Neither button pressed yet — keep asking.
            None => self.pending_force_pull = Some(prompt),
        }
        // Answered: the next queued corpus prompt (#422), if any, is raised
        // by `poll_job` once no job is running.
    }
}

/// Height cap for the "Changes since last save" list: a third of the room left
/// below the heading, clamped to `[120, 360]` px. That leaves the note box,
/// the push checkbox and the Save Repository button visible below the list
/// even on a small window. The list scrolls inside its cap.
fn changes_list_max_height(available: f32) -> f32 {
    (available / 3.0).clamp(120.0, 360.0)
}

/// A remote operation a repository panel can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteOp {
    Fetch,
    Pull,
    Push,
}

impl RemoteOp {
    /// The button's caption.
    fn button(self) -> &'static str {
        match self {
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
        }
    }

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
            label: "test".into(),
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

    fn conflict() -> advanced_git::RemoteError {
        advanced_git::RemoteError::Conflict {
            command: "git pull origin main".into(),
            output: "CONFLICT (content): Merge conflict in kovan.toml".into(),
        }
    }

    fn state_after(result: Result<String, advanced_git::RemoteError>) -> AdvancedGitState {
        let mut state = AdvancedGitState::default();
        state.record(
            "Kovan folder".into(),
            PathBuf::from("/tmp/local-kovan-repo"),
            "origin".into(),
            "main".into(),
            RemoteOp::Pull,
            result,
        );
        state
    }

    /// A conflicted pull raises the prompt instead of a red error message —
    /// the whole point of GH issue #279: the user is asked, not just told.
    #[test]
    fn a_conflicted_pull_raises_the_prompt_and_says_nothing_in_red() {
        let state = state_after(Err(conflict()));
        let prompt = state.pending_force_pull.expect("no prompt raised");
        assert_eq!(prompt.dir, PathBuf::from("/tmp/local-kovan-repo"));
        assert_eq!(
            (prompt.remote.as_str(), prompt.branch.as_str()),
            ("origin", "main")
        );
        assert!(prompt.git_says.contains("Merge conflict"));
        assert!(
            state.message.is_empty(),
            "left a stale message under the prompt"
        );
    }

    /// Every other failure keeps the old behaviour. A forced pull must not
    /// be offered as the answer to a typo'd URL or a refused password —
    /// those are not resolved by destroying the folder.
    #[test]
    fn an_ordinary_failure_still_reports_an_error_and_offers_no_forced_pull() {
        let state = state_after(Err(advanced_git::RemoteError::Failed {
            command: "git pull origin main".into(),
            stderr: "fatal: Authentication failed".into(),
        }));
        assert!(state.pending_force_pull.is_none());
        assert!(state.message_is_error);
        assert!(state.message.contains("Authentication failed"));

        let ok = state_after(Ok(String::new()));
        assert!(ok.pending_force_pull.is_none());
        assert!(!ok.message_is_error);
    }

    /// Drawing the prompt without pressing either button changes nothing
    /// and keeps asking — a dialog that can destroy the folder must not be
    /// dismissible by ignoring it. Run headless through `egui::Context`,
    /// with no window and no GPU.
    #[test]
    fn drawing_the_prompt_without_answering_it_neither_pulls_nor_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), crate::root::RootConfig::new("lib", "Lib"), true)
            .unwrap();
        let mut state = state_after(Err(conflict()));
        // A directory that is not a repository at all: if either branch ran
        // despite no button being pressed, the git call would fail and land
        // an error message here.
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ctx| {
                state.force_pull_prompt_ui(ctx, &root);
            });
        }
        assert!(
            state.pending_force_pull.is_some(),
            "the prompt vanished unanswered"
        );
        assert!(state.message.is_empty(), "something ran: {}", state.message);
    }

    /// Pulling the Kovan folder lists each corpus's result, and every corpus
    /// whose local work a pull would destroy gets its own prompt, asked one
    /// at a time — overriding a corpus is never automatic (#422).
    #[test]
    fn corpora_that_need_overriding_are_asked_about_one_at_a_time() {
        use crate::save_push::{CorpusKind, CorpusPull, CorpusPullOutcome};
        let confirm = |corpus, dir: &str| CorpusPull {
            corpus,
            name: String::new(),
            dir: PathBuf::from(dir),
            outcome: CorpusPullOutcome::NeedsConfirmation {
                remote: "origin".into(),
                branch: "main".into(),
                reason: "it has saves the remote does not have".into(),
            },
        };
        let mut state = state_after(Ok(String::new()));
        state.follow_corpora(vec![
            confirm(CorpusKind::Proprietary, "/k/prop"),
            confirm(CorpusKind::Open, "/k/open"),
            CorpusPull {
                corpus: CorpusKind::Standard,
                name: String::new(),
                dir: PathBuf::from("/k/std"),
                outcome: CorpusPullOutcome::UpToDate {
                    branch: "main".into(),
                },
            },
        ]);
        assert_eq!(state.message.lines().count(), 4, "{}", state.message);
        assert!(state.message.contains("Standard corpus: up to date"));
        let first = state.pending_force_pull.as_ref().expect("no prompt");
        assert_eq!(first.dir, PathBuf::from("/k/prop"));
        assert!(!first.follow_corpora);
        assert_eq!(state.queued_force_pulls.len(), 1);
        assert_eq!(state.queued_force_pulls[0].dir, PathBuf::from("/k/open"));
    }

    /// A job runs off the GUI thread and is recorded only once it has
    /// finished; while it runs, nothing is recorded and no prompt is shown.
    #[test]
    fn a_background_job_is_recorded_when_it_finishes_and_not_before() {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), crate::root::RootConfig::new("lib", "Lib"), true)
            .unwrap();
        let gate = std::sync::Arc::new(std::sync::RwLock::new(false));
        let mut state = AdvancedGitState::default();
        state.queued_force_pulls.push_back(ForcePullPrompt {
            label: "Open corpus".into(),
            dir: PathBuf::from("/k/open"),
            remote: "origin".into(),
            branch: "main".into(),
            git_says: String::new(),
            follow_corpora: false,
        });
        let release = gate.clone();
        state.spawn("pushing Kovan folder…".into(), move || {
            while !*release.read().unwrap() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            GitJobDone::Remote {
                label: "Kovan folder".into(),
                dir: PathBuf::from("/k"),
                remote: "origin".into(),
                branch: "main".into(),
                op: RemoteOp::Push,
                result: Ok(String::new()),
                kovan_folder: true,
                corpora: None,
            }
        });
        state.poll_job(&root);
        assert!(state.job.is_some(), "recorded before it finished");
        assert!(state.message.is_empty());
        assert!(state.pending_force_pull.is_none(), "prompt raised mid-job");

        *gate.write().unwrap() = true;
        for _ in 0..400 {
            state.poll_job(&root);
            if state.job.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(state.job.is_none(), "the job never finished");
        assert!(
            state.message.contains("Kovan folder: push done"),
            "{}",
            state.message
        );
        assert_eq!(
            state.pending_force_pull.map(|p| p.dir),
            Some(PathBuf::from("/k/open"))
        );
    }

    /// GH #502: a forced pull that moved unpushed saves onto a
    /// `kovan-kept/…` branch says so in the status line, rather than a
    /// fixed "now matches" that hides where the saves went.
    #[test]
    fn a_forced_pull_names_the_branch_the_unpushed_saves_were_kept_on() {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), crate::root::RootConfig::new("lib", "Lib"), true)
            .unwrap();
        let mut state = AdvancedGitState::default();
        state.record_job(
            GitJobDone::ForcePull {
                prompt: ForcePullPrompt {
                    label: "Open corpus".into(),
                    dir: PathBuf::from("/k/open"),
                    remote: "origin".into(),
                    branch: "main".into(),
                    git_says: String::new(),
                    follow_corpora: false,
                },
                result: Ok("/k/open now matches origin/main exactly; saves that were never \
                            pushed are kept on kovan-kept/f7352f5abcde"
                    .into()),
                corpora: None,
            },
            &root,
        );
        assert!(!state.message_is_error, "{}", state.message);
        assert!(
            state.message.contains("kovan-kept/f7352f5abcde"),
            "{}",
            state.message
        );
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

    /// The commit note is cleared only by a save that committed; a failure
    /// or a nothing-to-save keeps what the user typed (2026-09-28).
    #[test]
    fn the_commit_note_survives_everything_but_a_successful_save() {
        let typed = "read chapter 3";
        let mut state = AdvancedGitState {
            commit_note: typed.into(),
            ..Default::default()
        };
        let err = crate::repository::RepositoryError::Git("boom".into());
        assert!(!state.record_save(Err(err), None));
        assert_eq!(state.commit_note, typed);
        assert!(state.message_is_error);

        assert!(!state.record_save(Ok(None), None));
        assert_eq!(state.commit_note, typed);

        let summary = crate::repository::SaveSummary {
            added: vec!["notes.md".into()],
            ..Default::default()
        };
        assert!(state.record_save(Ok(Some(summary)), None));
        assert!(state.commit_note.is_empty());
        assert!(!state.message_is_error);
    }

    /// Each repository's push result is shown under the save line; a
    /// refused or failed push turns the message red but still clears the
    /// note, because the commit it went into was made (2026-09-28).
    #[test]
    fn push_results_are_listed_per_repository_and_a_problem_is_red() {
        use crate::save_push::{PushOutcome, PushRepo, PushReport, RepoPush};
        let summary = crate::repository::SaveSummary {
            added: vec!["a.pdf".into()],
            ..Default::default()
        };
        let report = PushReport {
            repos: vec![
                RepoPush {
                    repo: PushRepo::ProprietaryCorpus,
                    name: String::new(),
                    dir: PathBuf::new(),
                    outcome: PushOutcome::Pushed {
                        remote_url: "https://example.com/private.git".into(),
                        branch: "main".into(),
                        attached: true,
                        merged: None,
                    },
                },
                RepoPush {
                    repo: PushRepo::OpenCorpus,
                    name: String::new(),
                    dir: PathBuf::new(),
                    outcome: PushOutcome::Failed {
                        message: "the remote has commits this folder does not have — pull first"
                            .into(),
                    },
                },
                RepoPush {
                    repo: PushRepo::KovanRepository,
                    name: String::new(),
                    dir: PathBuf::new(),
                    outcome: PushOutcome::Skipped {
                        reason: "a corpus above was not pushed".into(),
                    },
                },
            ],
            warnings: Vec::new(),
        };
        let mut state = AdvancedGitState {
            commit_note: "note".into(),
            ..Default::default()
        };
        assert!(state.record_save(Ok(Some(summary.clone())), Some(report)));
        assert!(state.commit_note.is_empty());
        assert!(state.message_is_error);
        let lines: Vec<&str> = state.message.lines().collect();
        assert!(lines[0].starts_with("saved: 1 added"), "{lines:?}");
        assert!(lines[1]
            .starts_with("Proprietary corpus: pushed main to https://example.com/private.git"));
        assert!(
            lines[2].starts_with("Open corpus: push FAILED") && lines[2].contains("pull first")
        );
        assert!(lines[3].starts_with("Kovan repository: not pushed"));

        let ok = PushReport {
            repos: vec![RepoPush {
                repo: PushRepo::KovanRepository,
                name: String::new(),
                dir: PathBuf::new(),
                outcome: PushOutcome::UpToDate {
                    branch: "main".into(),
                },
            }],
            warnings: Vec::new(),
        };
        let mut state = AdvancedGitState::default();
        state.record_save(Ok(Some(summary)), Some(ok));
        assert!(!state.message_is_error);
        assert!(state.message.contains("nothing to push"));
    }

    /// Drawn headless, the checkbox shows the library's persisted setting:
    /// ticked by default, unticked once the library opted out.
    #[test]
    fn the_push_checkbox_reflects_the_library_setting() {
        let dir = tempfile::tempdir().unwrap();
        let mut root =
            KovanRoot::create(dir.path(), crate::root::RootConfig::new("lib", "Lib"), true)
                .unwrap();
        let ctx = egui::Context::default();
        let mut state = AdvancedGitState::default();
        let _ = ctx.run_ui(Default::default(), |ui| state.ui(ui, &root));
        assert!(state.push_after_save, "default must be ticked");

        root.set_push_after_save(false).unwrap();
        let mut state = AdvancedGitState::default();
        let _ = ctx.run_ui(Default::default(), |ui| state.ui(ui, &root));
        assert!(!state.push_after_save);

        // Changing it writes the file.
        state.push_after_save = true;
        state.persist_push_setting(&root);
        assert!(crate::advanced_git::push_after_save_setting(&root));
        assert!(!state.message_is_error, "{}", state.message);
    }

    /// A fresh Kovan folder with thousands of new files must not push the
    /// Save Repository button off the window (maintainer, 2026-09-30: "my
    /// save repository page is flooded with changes. I cannot even find the
    /// save repository button").
    ///
    /// Drawn headless in a 1000 × 700 window with 5 000 added files, the whole
    /// tab (list, note box, checkbox, buttons) must fit within the window
    /// height. Before the change, every file was a bare label in the page, so
    /// the page grew with the file count and the button sat below all of them.
    /// Control run, 2026-09-30: with the cap removed (the list in a scroll
    /// area with no height limit), this test FAILS, with an 849 px page in a
    /// 700 px window.
    #[test]
    fn thousands_of_changes_do_not_push_the_save_button_off_screen() {
        let dir = tempfile::tempdir().unwrap();
        let root =
            KovanRoot::create(dir.path(), crate::root::RootConfig::new("lib", "Lib"), true)
                .unwrap();
        let mut state = AdvancedGitState {
            loaded_once: true, // keep `ui` from re-scanning and replacing the status
            status: Some(crate::repository::SaveSummary {
                added: (0..5000).map(|i| format!("corpus/file_{i:05}.md")).collect(),
                changed: Vec::new(),
                removed: Vec::new(),
            }),
            ..Default::default()
        };
        let window = egui::vec2(1000.0, 700.0);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, window)),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut page_height = f32::NAN;
        let _ = ctx.run_ui(input, |ui| {
            state.ui(ui, &root);
            page_height = ui.min_rect().height();
        });
        assert!(
            page_height < window.y,
            "the Save Repository tab is {page_height:.0} px tall in a {:.0} px window; \
             the button is off screen",
            window.y
        );
    }

    #[test]
    fn the_changes_list_cap_leaves_room_below_it() {
        assert_eq!(changes_list_max_height(300.0), 120.0);
        assert_eq!(changes_list_max_height(600.0), 200.0);
        assert_eq!(changes_list_max_height(3000.0), 360.0);
    }
}
