//! **Sync down** in the Code Review tab (maintainer, 2026-10-12): pull the
//! code repository's checked-out branch, so stamps and fixes pushed from
//! another machine appear without a terminal. Before this, the maintainer
//! ran `git pull` by hand and then pressed "Rebuild data" to see a stamp.
//!
//! The Git is [`crate::advanced_git`]'s, the same calls the Save Repository
//! tab makes: [`advanced_git::pull_target_in`] picks the branch and remote,
//! [`advanced_git::pull_in`] pulls, and a pull Git refuses because the
//! folder and the remote disagree raises the same "Pull anyway?" prompt
//! ([`super::advanced_git_view::force_pull_modal`]). "yes, can" runs
//! [`advanced_git::force_pull_in`] (unpushed commits kept on a
//! `kovan-kept/…` branch), "no, i manage myself" runs
//! [`advanced_git::abort_in_progress_in`]. After a pull that changed the
//! folder, the host rebuilds its data from the saved SCIP index.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The pull and the forced pull
//! run on a thread; a frame only checks whether it finished.

use std::path::PathBuf;
use std::thread::JoinHandle;

use crate::advanced_git::{self, PullTarget, RemoteError};

/// What a pull on the thread came back with.
#[derive(Debug)]
pub(crate) enum PullDone {
    /// The pull ran; Git's words.
    Pulled(String),
    /// Git refused or stopped because the folder and the remote disagree:
    /// ask before forcing.
    Conflict {
        target: PullTarget,
        git_says: String,
    },
    /// Anything else (no remote, offline, refused credentials).
    Failed(String),
    /// The forced pull ran (`Ok`) or failed.
    Forced(Result<String, String>),
}

/// Find the target and pull it. Runs on the sync thread.
pub(crate) fn pull(dir: &std::path::Path) -> PullDone {
    let target = match advanced_git::pull_target_in(dir) {
        Ok(t) => t,
        Err(e) => return PullDone::Failed(e),
    };
    match advanced_git::pull_in(&target.dir, &target.remote, &target.branch) {
        Ok(out) => PullDone::Pulled(out),
        Err(RemoteError::Conflict { output, .. }) => PullDone::Conflict {
            target,
            git_says: output,
        },
        Err(e) => PullDone::Failed(e.to_string()),
    }
}

/// Sync down's state: at most one Git job, or one unanswered prompt.
#[derive(Default)]
pub(crate) struct CodeSync {
    job: Option<(&'static str, JoinHandle<PullDone>)>,
    prompt: Option<(PullTarget, String)>,
    /// The last result, shown beside the button.
    pub(crate) message: Option<Result<String, String>>,
}

impl CodeSync {
    /// A pull or forced pull is running, or the prompt is waiting.
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some() || self.prompt.is_some()
    }

    /// What the running job is doing, for the spinner.
    pub(crate) fn working(&self) -> Option<&'static str> {
        self.job.as_ref().map(|(w, _)| *w)
    }

    /// Start pulling `dir` on a thread (ignored while busy).
    pub(crate) fn start(&mut self, dir: PathBuf) {
        if self.busy() {
            return;
        }
        self.message = None;
        self.job = Some(("pulling\u{2026}", std::thread::spawn(move || pull(&dir))));
    }

    /// Take a finished job. Returns whether the folder changed, so the host
    /// reloads its data.
    pub(crate) fn poll(&mut self) -> bool {
        if !self.job.as_ref().is_some_and(|(_, h)| h.is_finished()) {
            return false;
        }
        let Some((_, handle)) = self.job.take() else {
            return false;
        };
        let done = handle
            .join()
            .unwrap_or_else(|_| PullDone::Failed("the pull failed unexpectedly".into()));
        self.record(done)
    }

    /// File a finished job's result; returns whether the folder changed.
    pub(crate) fn record(&mut self, done: PullDone) -> bool {
        match done {
            PullDone::Pulled(out) => {
                let changed = !out.contains("Already up to date");
                self.message = Some(Ok(if changed {
                    "Synced: pulled the latest; rebuilding the view".into()
                } else {
                    "Already up to date".into()
                }));
                changed
            }
            PullDone::Conflict { target, git_says } => {
                self.prompt = Some((target, git_says));
                false
            }
            PullDone::Failed(e) => {
                self.message = Some(Err(format!("Sync down failed: {e}")));
                false
            }
            PullDone::Forced(Ok(out)) => {
                self.message = Some(Ok(format!("Synced by force: {out}")));
                true
            }
            PullDone::Forced(Err(e)) => {
                self.message = Some(Err(format!("Forced pull failed: {e}")));
                false
            }
        }
    }

    /// Answer the prompt: force (on a thread) or leave the folder as it was.
    pub(crate) fn answer(&mut self, force: bool) {
        let Some((target, _)) = self.prompt.take() else {
            return;
        };
        if force {
            self.job = Some((
                "pulling by force\u{2026}",
                std::thread::spawn(move || {
                    PullDone::Forced(
                        advanced_git::force_pull_in(&target.dir, &target.remote, &target.branch)
                            .map_err(|e| e.to_string()),
                    )
                }),
            ));
        } else {
            self.message = Some(match advanced_git::abort_in_progress_in(&target.dir) {
                Ok(_) => Ok("Sync cancelled: nothing in the folder was changed".into()),
                Err(e) => Err(format!(
                    "Sync cancelled, but undoing the half-done merge failed: {e}"
                )),
            });
        }
    }

    /// Draw the prompt while one waits. `unsaved` is the number of review
    /// files this session wrote and has not committed: a forced pull would
    /// delete them. GUI drawing code (exempt from the test rule; the answers
    /// are [`Self::answer`], tested).
    pub(crate) fn prompt_ui(&mut self, ctx: &egui::Context, unsaved: usize) {
        let Some((target, git_says)) = &self.prompt else {
            return;
        };
        let extra = (unsaved > 0).then(|| {
            format!(
                "\u{26a0} {unsaved} review file(s) written this session are not committed \
                 yet: commit them under \u{2691} need you first, or a forced pull deletes them."
            )
        });
        let answer = super::advanced_git_view::force_pull_modal(
            ctx,
            "code-review-force-pull",
            "Code repository",
            &target.dir,
            &target.remote,
            &target.branch,
            git_says,
            extra.as_deref(),
        );
        if let Some(force) = answer {
            self.answer(force);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> PullTarget {
        PullTarget {
            dir: PathBuf::from("/nonexistent/kovan-sync-test"),
            remote: "origin".into(),
            branch: "main".into(),
        }
    }

    /// Git's "Already up to date" changes nothing, so the view is not
    /// rebuilt; any other successful pull rebuilds it.
    #[test]
    fn only_a_pull_that_changed_the_folder_asks_for_a_rebuild() {
        let mut s = CodeSync::default();
        assert!(!s.record(PullDone::Pulled("Already up to date.\n".into())));
        assert_eq!(s.message, Some(Ok("Already up to date".into())));
        assert!(s.record(PullDone::Pulled("Fast-forward\n a | 1 +\n".into())));
        assert!(s.record(PullDone::Forced(Ok("now matches".into()))));
        assert!(!s.record(PullDone::Failed("offline".into())));
        assert!(matches!(s.message, Some(Err(_))));
    }

    /// A conflict waits for an answer, holds the button off meanwhile, and
    /// "no, i manage myself" forces nothing.
    #[test]
    fn a_conflict_waits_for_an_answer_and_no_forces_nothing() {
        let mut s = CodeSync::default();
        assert!(!s.record(PullDone::Conflict {
            target: target(),
            git_says: "CONFLICT (content)".into(),
        }));
        assert!(s.busy());
        s.start(PathBuf::from("/elsewhere"));
        assert!(s.working().is_none(), "a pull started over the prompt");
        s.answer(false);
        assert!(!s.busy());
        assert!(s.working().is_none(), "declining started a job");
    }

    /// The pull itself, on a folder with no repository: refused in words,
    /// off the GUI thread, and recorded once it finishes.
    #[test]
    fn a_pull_outside_a_repository_is_reported_not_forced() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = CodeSync::default();
        s.start(dir.path().to_path_buf());
        for _ in 0..2000 {
            if s.job.is_none() || s.poll() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(s.job.is_none(), "the pull never finished");
        assert!(s.prompt.is_none());
        assert!(
            matches!(&s.message, Some(Err(e)) if e.contains("Git repository")),
            "{:?}",
            s.message
        );
    }
}
