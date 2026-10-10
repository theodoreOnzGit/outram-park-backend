//! **Commit and push `review.md`** from desktop kovan (GitHub #771; #739
//! "Protecting review.md", maintainer 2026-10-07: "kovan gets a
//! commit-and-push action for `review.md`, like the literature corpus
//! push, to the current branch or `develop`. It is never `main`. … It must
//! not repeat #502").
//!
//! ```text
//!  files kovan wrote this session (review.md, kovan_root.toml)
//!     │ plan(): on a branch? not main? each file a review.md or
//!     │ kovan_root.toml under the root? changed in git?
//!     v
//!  git add -- <files>; git commit -m <generated> -- <files>
//!     │   (only these paths: anything else staged or edited stays as it was)
//!     v
//!  save_push::safe_push::push_keeping_local(root, remote, branch, NeverMain)
//!         fast-forward push; if the remote moved: fetch, MERGE, push again;
//!         never reset, rebase or force (#502)
//! ```
//!
//! # What is committed
//!
//! **Only the files kovan wrote in this session**, as the caller lists them
//! (the stamp dialog's writes, acknowledged moves and deletions), and only
//! the two kinds kovan writes: a folder's `review.md` and the workspace's
//! `kovan_root.toml` (a key registration in the dialog writes it, and a
//! stamp by an unregistered key does not count, so the two travel
//! together). #739 names `review.md` only; including `kovan_root.toml` when
//! kovan itself changed it this session is this module's reading, recorded
//! for the maintainer on #771. A `kovan.toml` or source file is never
//! committed here.
//!
//! # Which branch
//!
//! **The current branch.** `main` is refused before anything is staged
//! ([`CommitPushError::MainRefused`]), and so is a detached `HEAD`; kovan
//! never switches branches for you (switching moves the working tree under
//! an open editor). The remote is the branch's upstream remote, else
//! `origin`, else the only remote.
//!
//! # The commit carries no agent trailer
//!
//! The message is generated from what changed and names the person's
//! action; it never carries `Co-Authored-By: Claude` or `Claude-Session:`,
//! which would mark every stamp in it unverified (#739). The commit is
//! made with the repository's own `user.name`/`user.email`.

use std::collections::BTreeSet;
use std::path::Path;

use kovan_common::review::review_md::{parse_review_md, Entry, ReviewDocument};

use super::flow::Worker;
use super::{REVIEW_MD, ROOT_FILE};
use crate::review_stamps::git;
use crate::save_push::safe_push::{push_keeping_local, BranchRule, SafePushError, SafePushOk};

/// What would be committed and where it would go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPlan {
    pub branch: String,
    pub remote: Option<String>,
    /// Workspace-relative, sorted, each changed in git.
    pub files: Vec<String>,
    pub message: String,
}

/// A commit that was pushed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pushed {
    pub commit: String,
    pub branch: String,
    pub remote: String,
    /// The remote's tip merged in first because it had moved on.
    pub merged: Option<String>,
    pub files: Vec<String>,
}

/// Why nothing (more) happened. After [`CommitPushError::NoRemote`] and
/// [`CommitPushError::PushFailed`] the commit exists locally and is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitPushError {
    /// The branch is `main`. Nothing was staged or committed.
    MainRefused { branch: String },
    /// `HEAD` is detached. Nothing was staged or committed.
    Detached,
    /// None of the files has a change to commit.
    NothingToCommit,
    /// A file kovan does not write (not a `review.md` or the root's
    /// `kovan_root.toml`), or outside the workspace. Nothing was staged.
    NotKovanFile(String),
    /// `git add` or `git commit` failed (Git's words).
    Git(String),
    /// Committed, but the repository has no remote to push to.
    NoRemote { commit: String, branch: String },
    /// Committed, but the push did not go through; the commit is kept.
    PushFailed {
        commit: String,
        branch: String,
        error: SafePushError,
    },
}

impl std::fmt::Display for CommitPushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let short = |s: &str| s.chars().take(7).collect::<String>();
        match self {
            Self::MainRefused { branch } => write!(
                f,
                "You are on `{branch}`. kovan commits review.md to a work branch or `develop`, \
                 never `main`. Nothing was committed: switch branch (`git switch develop`) and \
                 press Commit and push again."
            ),
            Self::Detached => write!(
                f,
                "HEAD is detached (no branch is checked out). Nothing was committed: check out \
                 a work branch or `develop` first."
            ),
            Self::NothingToCommit => write!(f, "Nothing to commit: the files kovan wrote have no changes."),
            Self::NotKovanFile(p) => write!(
                f,
                "{p} is not a file kovan writes (review.md or kovan_root.toml); nothing was committed"
            ),
            Self::Git(e) => write!(f, "Git said: {e}"),
            Self::NoRemote { commit, branch } => write!(
                f,
                "Committed {} on `{branch}`, but this repository has no remote to push to. \
                 The commit is kept.",
                short(commit)
            ),
            Self::PushFailed {
                commit,
                branch,
                error,
            } => write!(
                f,
                "Committed {} on `{branch}`; the push did not go through and nothing was reset. \
                 {error}",
                short(commit)
            ),
        }
    }
}

impl std::error::Error for CommitPushError {}

/// Whether `rel` is a file kind kovan writes (module doc).
fn is_kovan_file(rel: &str) -> bool {
    let safe = !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.split('/').any(|c| c == ".." || c.is_empty());
    safe && (rel == ROOT_FILE || rel == REVIEW_MD || rel.ends_with(&format!("/{REVIEW_MD}")))
}

/// The checked-out branch; `None` when `HEAD` is detached.
pub fn current_branch(root: &Path) -> Option<String> {
    git::git(root, &["symbolic-ref", "-q", "--short", "HEAD"])
        .ok()
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
}

/// The remote to push `branch` to (module doc).
fn remote_for(root: &Path, branch: &str) -> Option<String> {
    if let Ok(r) = git::git(
        root,
        &["config", "--get", &format!("branch.{branch}.remote")],
    ) {
        let r = r.trim();
        if !r.is_empty() && r != "." {
            return Some(r.to_string());
        }
    }
    let remotes: Vec<String> = git::git(root, &["remote"])
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if remotes.iter().any(|r| r == "origin") {
        return Some("origin".into());
    }
    match remotes.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

/// The review entries in `now` that are new or differ from `before`, and
/// how many of `before`'s entries are gone (moved out or retired).
fn changes(before: &ReviewDocument, now: &ReviewDocument) -> (Vec<String>, usize) {
    let mut changed = Vec::new();
    for e in &now.entries {
        if !before.entries.iter().any(|b| b.entry == e.entry) {
            changed.push(e.heading.clone());
        }
    }
    let gone = before
        .entries
        .iter()
        .filter(|b| !now.entries.iter().any(|e| e.entry == b.entry))
        .count();
    (changed, gone)
}

/// The generated commit message for `files` (module doc): a summary line
/// counting stamps, needs-fix notes and other entries changed, then each
/// file with the headings of its new or changed entries.
pub fn commit_message(root: &Path, files: &[String]) -> String {
    let (mut stamps, mut fixes, mut other, mut gone) = (0usize, 0usize, 0usize, 0usize);
    let mut body = Vec::new();
    for f in files {
        if f == ROOT_FILE {
            body.push(format!("- {f}: reviewer keys registered"));
            continue;
        }
        let before = parse_review_md(&git::show(root, "HEAD", f).unwrap_or_default());
        let now = parse_review_md(&std::fs::read_to_string(root.join(f)).unwrap_or_default());
        let (changed, g) = changes(&before, &now);
        gone += g;
        for h in &changed {
            match now
                .entries
                .iter()
                .find(|e| &e.heading == h)
                .map(|e| &e.entry)
            {
                Some(Entry::Review(_)) => stamps += 1,
                Some(Entry::NeedsFix(_)) => fixes += 1,
                _ => other += 1,
            }
        }
        let mut line = format!("- {f}:");
        if changed.is_empty() && g > 0 {
            line.push_str(&format!(
                " {g} entr{} moved out or retired",
                if g == 1 { "y" } else { "ies" }
            ));
        }
        for h in &changed {
            line.push_str(&format!("\n    {h}"));
        }
        body.push(line);
    }
    let mut parts = Vec::new();
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    if stamps > 0 {
        parts.push(plural(stamps, "stamp", "stamps"));
    }
    if fixes > 0 {
        parts.push(plural(fixes, "needs-fix note", "needs-fix notes"));
    }
    if other > 0 {
        parts.push(plural(other, "other entry", "other entries"));
    }
    if gone > 0 {
        parts.push(plural(
            gone,
            "entry moved or retired",
            "entries moved or retired",
        ));
    }
    if parts.is_empty() {
        parts.push("review records".into());
    }
    format!(
        "kovan review: {} (#771)\n\nWritten by desktop kovan's Commit and push: only the files \
         kovan wrote this session.\n\n{}\n",
        parts.join(", "),
        body.join("\n")
    )
}

/// Check what would be committed (module doc), without changing anything.
pub fn plan(root: &Path, files: &BTreeSet<String>) -> Result<CommitPlan, CommitPushError> {
    let branch = current_branch(root).ok_or(CommitPushError::Detached)?;
    if branch == "main" {
        return Err(CommitPushError::MainRefused { branch });
    }
    if let Some(bad) = files.iter().find(|f| !is_kovan_file(f)) {
        return Err(CommitPushError::NotKovanFile(bad.clone()));
    }
    let mut changed = Vec::new();
    for f in files {
        let out =
            git::git(root, &["status", "--porcelain", "--", f]).map_err(CommitPushError::Git)?;
        if !out.trim().is_empty() {
            changed.push(f.clone());
        }
    }
    if changed.is_empty() {
        return Err(CommitPushError::NothingToCommit);
    }
    Ok(CommitPlan {
        remote: remote_for(root, &branch),
        message: commit_message(root, &changed),
        branch,
        files: changed,
    })
}

/// Commit the session's kovan files and push them (module doc).
pub fn commit_and_push(root: &Path, files: &BTreeSet<String>) -> Result<Pushed, CommitPushError> {
    let p = plan(root, files)?;
    let mut add = vec!["add", "--"];
    add.extend(p.files.iter().map(String::as_str));
    git::git(root, &add).map_err(CommitPushError::Git)?;
    let mut commit = vec!["commit", "-q", "-m", p.message.as_str(), "--"];
    commit.extend(p.files.iter().map(String::as_str));
    git::git(root, &commit).map_err(CommitPushError::Git)?;
    let head = git::rev_parse(root, "HEAD").map_err(CommitPushError::Git)?;
    let Some(remote) = p.remote.clone() else {
        return Err(CommitPushError::NoRemote {
            commit: head,
            branch: p.branch,
        });
    };
    match push_keeping_local(root, &remote, &p.branch, BranchRule::NeverMain) {
        Ok(ok) => Ok(Pushed {
            commit: head,
            branch: p.branch,
            remote,
            merged: match ok {
                SafePushOk::Pushed { merged } => merged,
                SafePushOk::UpToDate => None,
            },
            files: p.files,
        }),
        Err(error) => Err(CommitPushError::PushFailed {
            commit: head,
            branch: p.branch,
            error,
        }),
    }
}

/// A commit-and-push running on a worker (the UI thread only polls).
#[derive(Default)]
pub struct CommitPushJob {
    worker: Option<Worker<Result<Pushed, CommitPushError>>>,
    /// The last finished run.
    pub last: Option<Result<Pushed, CommitPushError>>,
}

impl CommitPushJob {
    /// Start a run over `files` unless one is running; returns whether it
    /// started.
    pub fn start(&mut self, root: &Path, files: BTreeSet<String>) -> bool {
        if self.worker.is_some() {
            return false;
        }
        let root = root.to_path_buf();
        self.last = None;
        self.worker = Some(Worker::spawn(move || commit_and_push(&root, &files)));
        true
    }

    /// Take a finished run's result; never blocks. Returns whether one
    /// finished just now.
    pub fn poll(&mut self) -> bool {
        let Some(r) = self.worker.as_ref().and_then(Worker::try_take) else {
            return false;
        };
        self.worker = None;
        self.last = Some(r);
        true
    }

    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }

    /// The last run in plain words.
    pub fn status(&self) -> Option<Result<String, String>> {
        self.last.as_ref().map(|r| match r {
            Ok(p) => Ok(format!(
                "Committed {} and pushed `{}` to {}{} ({}).",
                p.commit.chars().take(7).collect::<String>(),
                p.branch,
                p.remote,
                if p.merged.is_some() {
                    ", after merging the remote's newer commits (nothing was reset)"
                } else {
                    ""
                },
                p.files.join(", ")
            )),
            Err(e) => Err(e.to_string()),
        })
    }

    /// Poll until the run finishes (tests only; the UI never waits).
    #[cfg(test)]
    pub fn wait(&mut self) {
        let start = std::time::Instant::now();
        while self.busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(120));
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll();
        }
    }
}
