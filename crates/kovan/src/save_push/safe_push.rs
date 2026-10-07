//! A push that **never discards the local commit** when the remote has moved
//! on (GH issue #502), reusable by every Kovan path that commits and pushes:
//! push-after-save today, the review.md commit-and-push of GH issue #771
//! next.
//!
//! # What went wrong (#502, 2026-10-02)
//!
//! A Save committed a PDF into the open corpus, another writer had pushed
//! seconds earlier, and the push was rejected as non-fast-forward. Kovan
//! reported "pull first"; Pull then found the corpus "has saves the remote
//! does not have" and offered the forced pull, whose
//! `git reset --hard FETCH_HEAD` dropped the save from the branch (reflog:
//! `reset: moving to FETCH_HEAD`, then `branch: Reset to HEAD` from the
//! `checkout -B`). The PDF was recovered by hand with a cherry-pick.
//!
//! # What happens now
//!
//! [`push_keeping_local`] pushes (fast-forward only, never forced). If the
//! remote rejects it as non-fast-forward, it fetches the branch and
//! **merges** the remote's tip into the local branch, then pushes once
//! more. Nothing in this module ever runs `reset`, `checkout -B`, `clean`
//! or a forced push.
//!
//! - **Merge, not rebase.** A Save records each corpus's commit as a
//!   gitlink in the Kovan repository's own commit, and push-after-save
//!   pushes the corpora first precisely so that gitlink is fetchable
//!   ([`super::push_after_save`]). A rebase would rewrite the saved commit's
//!   id, leaving the Kovan repository pointing at a commit no remote has; a
//!   merge keeps the saved commit as an ancestor of what is pushed. It is
//!   also what the GUI's ordinary Pull already does (`git pull`, merge by
//!   default, [`crate::advanced_git::pull_in`]).
//! - **A merge that does not apply cleanly is aborted** (`git merge
//!   --abort`, which only undoes the merge in progress) and reported as
//!   [`SafePushError::Diverged`]. The local commit is still `HEAD`, the
//!   working tree is clean, and nothing was sent.
//! - **A working tree with uncommitted or untracked files is never merged
//!   into**: [`SafePushError::UncommittedChanges`]. Aborting a merge over
//!   local edits is the one case where `merge --abort` can lose them.
//! - **[`BranchRule::NeverMain`] refuses `main`** before anything runs, for
//!   paths that must only push a work branch or `develop` (#771).

use std::fmt;
use std::path::Path;

use super::{git, git_ok};

/// Which branches a caller may push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchRule {
    /// Any branch. Push-after-save uses this: the corpus repositories'
    /// only branch is `main`, and pushing it is what the maintainer asked
    /// Save to do (2026-09-28).
    AnyBranch,
    /// Every branch except `main` (GH issue #771: review.md goes to the
    /// current branch or `develop`, never `main`).
    NeverMain,
}

/// A push that went through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafePushOk {
    /// The branch was pushed. `merged` is the remote's tip that was merged
    /// in first because the remote had moved on, or `None` when the first
    /// push was a fast-forward.
    Pushed { merged: Option<String> },
    /// The remote already had the branch's commit.
    UpToDate,
}

/// Why nothing (more) was pushed. In every case the local commit is kept
/// and nothing was reset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafePushError {
    /// [`BranchRule::NeverMain`] and the branch is `main`. Nothing ran.
    MainRefused { branch: String },
    /// The repository is not on `branch`, so merging into it would merge
    /// into some other branch. Nothing ran.
    NotOnBranch { branch: String },
    /// The remote has moved on and the working tree has uncommitted or
    /// untracked files, so no merge was attempted.
    UncommittedChanges { branch: String, local: String },
    /// The remote has moved on and its commits do not merge cleanly with
    /// the local ones. The merge was aborted; `local` is still `HEAD`.
    Diverged {
        branch: String,
        local: String,
        remote: String,
        git_says: String,
    },
    /// Any other `git` failure (network, credentials, the remote moving
    /// again during the retry), with Git's words.
    Git { message: String },
}

impl fmt::Display for SafePushError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let short = |s: &str| s.chars().take(7).collect::<String>();
        match self {
            Self::MainRefused { branch } => write!(
                f,
                "refusing to push `{branch}`: this path pushes a work branch or `develop`, \
                 never `main`; nothing was sent"
            ),
            Self::NotOnBranch { branch } => write!(
                f,
                "the folder is not on `{branch}`, so nothing was merged or pushed"
            ),
            Self::UncommittedChanges { branch, local } => write!(
                f,
                "your save is committed locally ({}); the remote's `{branch}` has changed, and \
                 the folder also has unsaved files, so the two were not combined. Nothing was \
                 reset or discarded — save or remove the files, then Save again, or resolve \
                 with lazygit/git",
                short(local)
            ),
            Self::Diverged {
                branch,
                local,
                remote,
                git_says,
            } => write!(
                f,
                "your save is committed locally ({}); the remote's `{branch}` has changed ({}) \
                 and the two edit the same files, so they could not be combined automatically. \
                 Nothing was reset or discarded — resolve with lazygit/git (`git pull`, fix the \
                 conflict, commit), then Save again. Git said: {git_says}",
                short(local),
                short(remote)
            ),
            Self::Git { message } => write!(f, "Git said: {message}"),
        }
    }
}

impl std::error::Error for SafePushError {}

/// How one `git push --porcelain` attempt ended.
enum Attempt {
    Pushed,
    UpToDate,
    /// Rejected because the remote has commits the local branch lacks.
    Behind(String),
    Failed(String),
}

/// Push `branch` of the repository at `dir` to `remote`, fast-forward only;
/// if the remote has moved on, merge its tip in and push once more. The
/// local commit is never reset, rebased or discarded (module doc).
///
/// `dir` must be on `branch` ([`SafePushError::NotOnBranch`] otherwise).
pub fn push_keeping_local(
    dir: &Path,
    remote: &str,
    branch: &str,
    rule: BranchRule,
) -> Result<SafePushOk, SafePushError> {
    if rule == BranchRule::NeverMain && branch == "main" {
        return Err(SafePushError::MainRefused {
            branch: branch.into(),
        });
    }
    let on = git_ok(dir, &["symbolic-ref", "-q", "--short", "HEAD"]).unwrap_or_default();
    if on.trim() != branch {
        return Err(SafePushError::NotOnBranch {
            branch: branch.into(),
        });
    }
    let said = match attempt(dir, remote, branch) {
        Attempt::Pushed => return Ok(SafePushOk::Pushed { merged: None }),
        Attempt::UpToDate => return Ok(SafePushOk::UpToDate),
        Attempt::Failed(message) => return Err(SafePushError::Git { message }),
        Attempt::Behind(said) => said,
    };

    let git_err = |message: String| SafePushError::Git { message };
    git_ok(dir, &["fetch", "-q", remote, branch]).map_err(git_err)?;
    let fetched = git_ok(dir, &["rev-parse", "FETCH_HEAD"])
        .map_err(git_err)?
        .trim()
        .to_string();
    let local = git_ok(dir, &["rev-parse", "HEAD"])
        .map_err(git_err)?
        .trim()
        .to_string();
    let dirty = !git_ok(dir, &["status", "--porcelain"])
        .map_err(git_err)?
        .trim()
        .is_empty();
    if dirty {
        return Err(SafePushError::UncommittedChanges {
            branch: branch.into(),
            local,
        });
    }
    merge_or_abort(dir, remote, branch, &local, &fetched)?;

    match attempt(dir, remote, branch) {
        Attempt::Pushed | Attempt::UpToDate => Ok(SafePushOk::Pushed {
            merged: Some(fetched),
        }),
        Attempt::Behind(again) => Err(SafePushError::Git {
            message: format!(
                "the remote moved again while the save was being combined with it; your save \
                 and the merge are kept locally, Save again to retry. First refusal: {said}; \
                 second: {again}"
            ),
        }),
        Attempt::Failed(message) => Err(SafePushError::Git { message }),
    }
}

/// Merge `fetched` into the checked-out `branch`. On failure abort the
/// merge (never a reset) and report [`SafePushError::Diverged`].
fn merge_or_abort(
    dir: &Path,
    remote: &str,
    branch: &str,
    local: &str,
    fetched: &str,
) -> Result<(), SafePushError> {
    // The same fallback identity a Save's own commits use when none is
    // configured (`crate::repository`), so a merge commit can be written.
    let has_identity = git_ok(dir, &["config", "user.email"]).is_ok();
    let message = format!("Kovan: merge {remote}/{branch} before pushing (GH #502)");
    let mut args: Vec<&str> = Vec::new();
    if !has_identity {
        args.extend(["-c", "user.name=Kovan", "-c", "user.email=kovan@localhost"]);
    }
    args.extend(["merge", "-q", "--no-edit", "-m", &message, fetched]);
    let out = git(dir, &args).map_err(|message| SafePushError::Git { message })?;
    if out.status.success() {
        return Ok(());
    }
    let git_says = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
    .trim()
    .to_string();
    let in_progress = git_ok(dir, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok();
    let abort_note = if in_progress {
        match git_ok(dir, &["merge", "--abort"]) {
            Ok(_) => String::new(),
            Err(e) => format!(" (undoing the half-done merge also failed: {e})"),
        }
    } else {
        String::new()
    };
    Err(SafePushError::Diverged {
        branch: branch.into(),
        local: local.into(),
        remote: fetched.into(),
        git_says: format!("{git_says}{abort_note}"),
    })
}

/// One fast-forward-only push of `refs/heads/<branch>`.
fn attempt(dir: &Path, remote: &str, branch: &str) -> Attempt {
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    let out = match git(dir, &["push", "--porcelain", remote, &refspec]) {
        Ok(o) => o,
        Err(message) => return Attempt::Failed(message),
    };
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let flag = stdout
        .lines()
        .find(|l| l.contains(&refspec))
        .and_then(|l| l.chars().next());
    let said = format!("{}\n{stderr}", stdout.trim()).trim().to_string();
    match (out.status.success(), flag) {
        (true, Some('=')) => Attempt::UpToDate,
        (true, _) => Attempt::Pushed,
        (false, _) => {
            let lowered = said.to_ascii_lowercase();
            if flag == Some('!')
                && (lowered.contains("non-fast-forward") || lowered.contains("fetch first"))
            {
                Attempt::Behind(said)
            } else {
                Attempt::Failed(said)
            }
        }
    }
}

#[cfg(test)]
mod tests;
