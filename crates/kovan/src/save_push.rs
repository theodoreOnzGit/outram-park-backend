//! Push after Save Repository (maintainer, 2026-09-28: "kovan should be able
//! to push pdfs to the proprietary repos by default", then "and open
//! source").
//!
//! A Save ([`crate::repository::save_repository_with_message`]) commits up to
//! three repositories and, before this module, never pushed any of them, so
//! an ingested proprietary PDF stayed on one machine and the Kovan
//! repository's gitlink pointed at a commit no other clone could fetch.
//! [`push_after_save`] now pushes them, **on by default**; the opt-out is
//! `[save] push_after_save = false` in `kovan_root.toml`
//! ([`crate::root::SaveConfig`]), which the "Push after save" checkbox on the
//! Save Repository tab writes.
//!
//! # Order
//!
//! 1. the **proprietary corpus** ([`KovanRoot::restricted_sources_dir`]),
//! 2. the **open corpus** ([`KovanRoot::open_corpus_dir`]),
//! 3. the **Kovan repository** itself — only if neither corpus push failed
//!    or was refused, so the parent never publishes a gitlink to a corpus
//!    commit that is not on its remote. A corpus that is merely *skipped*
//!    (not downloaded here, or no remote configured for it) does not hold
//!    the parent back: that corpus was never going to be pushed from here.
//!
//! The **standard corpus is never pushed**: it is read-only to everyone but
//! its maintainer, and Save never commits into it.
//!
//! # Safety rules (non-negotiable, each pinned by a test)
//!
//! - **Never forced.** The refspec is `refs/heads/B:refs/heads/B` with no
//!   `+` and no `--force`, so Git itself refuses anything but a
//!   fast-forward. A remote that has moved on is reported as "pull first";
//!   the local commit is kept.
//! - **Never from a detached `HEAD`.** `git submodule update` leaves a
//!   submodule detached, and a Save then commits onto no branch. Before
//!   pushing, the commit is put on the submodule's tracked branch
//!   (`.gitmodules` `branch =`, else the remote's default branch), but only
//!   when that branch's tip — local and remote-tracking — is an ancestor of
//!   the commit, i.e. the branch fast-forwards. Otherwise nothing is moved
//!   and the push is refused.
//! - **Each corpus goes only to its own configured remote.** Every push URL
//!   of the proprietary corpus's remote must be the private remote in
//!   `kovan_root.toml` (`[private_submodule] remote` and/or
//!   `[corpora] proprietary_remote`, which must agree), and must not be the
//!   open-corpus or standard-corpus remote; the open corpus's must be
//!   `[corpora] open_remote` and must not be a proprietary one. A mismatch is
//!   refused, so a proprietary PDF cannot reach a public repository through
//!   a mis-set `origin`.
//! - **System `git` for the network**, as every remote operation in Kovan
//!   ([`crate::advanced_git`]), so the user's credential helpers apply.
//!   `GIT_TERMINAL_PROMPT=0` makes a missing credential fail fast with Git's
//!   own message instead of hanging on a prompt no GUI window can answer.
//!
//! # Pull: the corpora follow the Kovan folder (GH issue #422)
//!
//! The reverse direction. After the Kovan folder is pulled, [`pull_corpora`]
//! brings each downloaded corpus to its remote's branch tip, so a corpus
//! another clone saved into does not fall behind and get refused by the next
//! push. It only ever fast-forwards on its own; a corpus whose local work
//! would be destroyed is reported, not overridden, and the GUI asks.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::root::KovanRoot;

/// One of the repositories a Save pushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushRepo {
    /// The private literature submodule.
    ProprietaryCorpus,
    /// The user's open corpus.
    OpenCorpus,
    /// The Kovan folder's own repository.
    KovanRepository,
}

impl PushRepo {
    /// The label the UI shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::ProprietaryCorpus => "Proprietary corpus",
            Self::OpenCorpus => "Open corpus",
            Self::KovanRepository => "Kovan repository",
        }
    }
}

/// What happened to one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOutcome {
    /// New commits were pushed to `remote_url`'s `branch`.
    /// `attached` is set when the commit was on a detached `HEAD` and was
    /// first put on `branch` (a fast-forward of that branch).
    Pushed {
        remote_url: String,
        branch: String,
        attached: bool,
    },
    /// The remote already had everything: nothing to push.
    UpToDate { branch: String },
    /// Not attempted, by design (not downloaded here, no remote configured,
    /// no system `git`, or a corpus before it did not make it).
    Skipped { reason: String },
    /// Refused by a safety rule before anything was sent.
    Refused { reason: String },
    /// `git push` ran and failed: a remote that has moved on ("pull first"),
    /// an authentication failure, the network — with Git's own words.
    Failed { message: String },
}

impl PushOutcome {
    /// Whether this blocks the Kovan repository's push (and should be shown
    /// as a problem).
    pub fn is_problem(&self) -> bool {
        matches!(self, Self::Refused { .. } | Self::Failed { .. })
    }
}

/// One repository's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPush {
    pub repo: PushRepo,
    pub dir: PathBuf,
    pub outcome: PushOutcome,
}

impl RepoPush {
    /// One human-readable line, e.g. `Proprietary corpus: pushed main to …`.
    pub fn line(&self) -> String {
        let what = match &self.outcome {
            PushOutcome::Pushed {
                remote_url,
                branch,
                attached,
            } => {
                let attached = if *attached {
                    " (the save was on a detached HEAD; it was put on this branch first)"
                } else {
                    ""
                };
                format!("pushed {branch} to {remote_url}{attached}")
            }
            PushOutcome::UpToDate { branch } => format!("nothing to push ({branch} is up to date)"),
            PushOutcome::Skipped { reason } => format!("not pushed — {reason}"),
            PushOutcome::Refused { reason } => format!("REFUSED — {reason}"),
            PushOutcome::Failed { message } => format!("push FAILED — {message}"),
        };
        format!("{}: {what}", self.repo.label())
    }
}

/// Every repository's result, in push order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushReport {
    pub repos: Vec<RepoPush>,
}

impl PushReport {
    /// Whether any repository failed or was refused.
    pub fn has_problem(&self) -> bool {
        self.repos.iter().any(|r| r.outcome.is_problem())
    }

    /// The result of `repo`, if it was considered.
    pub fn get(&self, repo: PushRepo) -> Option<&PushOutcome> {
        self.repos
            .iter()
            .find(|r| r.repo == repo)
            .map(|r| &r.outcome)
    }

    /// One line per repository ([`RepoPush::line`]).
    pub fn lines(&self) -> Vec<String> {
        self.repos.iter().map(RepoPush::line).collect()
    }
}

/// Push the proprietary corpus, the open corpus and then the Kovan
/// repository, under the module's safety rules. Never panics; every
/// problem is a [`PushOutcome`] in the report.
///
/// Called after a successful save whether or not that save committed
/// anything, so commits from earlier saves that were never pushed (every
/// save before 2026-09-28) go up too.
pub fn push_after_save(root: &KovanRoot) -> PushReport {
    let mut report = PushReport::default();
    if !crate::advanced_git::system_git_available() {
        for (repo, dir) in [
            (PushRepo::ProprietaryCorpus, root.restricted_sources_dir()),
            (PushRepo::OpenCorpus, root.open_corpus_dir()),
            (PushRepo::KovanRepository, root.path().to_path_buf()),
        ] {
            report.repos.push(RepoPush {
                repo,
                dir,
                outcome: PushOutcome::Skipped {
                    reason: "no usable system `git`, which pushing needs".into(),
                },
            });
        }
        return report;
    }

    for repo in [PushRepo::ProprietaryCorpus, PushRepo::OpenCorpus] {
        let dir = match repo {
            PushRepo::ProprietaryCorpus => root.restricted_sources_dir(),
            _ => root.open_corpus_dir(),
        };
        let outcome = push_corpus(root, repo, &dir);
        report.repos.push(RepoPush { repo, dir, outcome });
    }

    let dir = root.path().to_path_buf();
    let outcome = if report.has_problem() {
        PushOutcome::Skipped {
            reason: "a corpus above was not pushed, and the Kovan repository must not \
                     publish a link to a corpus commit other clones cannot fetch"
                .into(),
        }
    } else {
        push_repo(&dir, None, None)
    };
    report.repos.push(RepoPush {
        repo: PushRepo::KovanRepository,
        dir,
        outcome,
    });
    report
}

/// Push one corpus repository after checking which remote it may go to.
fn push_corpus(root: &KovanRoot, repo: PushRepo, dir: &Path) -> PushOutcome {
    if !dir.join(".git").exists() {
        return PushOutcome::Skipped {
            reason: "not downloaded in this Kovan folder".into(),
        };
    }
    if same_dir(dir, &root.standard_corpus_dir()) {
        return PushOutcome::Refused {
            reason: "this folder is the standard corpus, which Kovan never pushes".into(),
        };
    }
    let cfg = root.config();
    let proprietary: Vec<String> = cfg
        .private_submodule
        .as_ref()
        .map(|p| p.remote.clone())
        .into_iter()
        .chain(cfg.corpora.proprietary_remote.clone())
        .collect();
    let open: Vec<String> = cfg.corpora.open_remote.clone().into_iter().collect();

    let (allowed, forbidden, what) = match repo {
        PushRepo::ProprietaryCorpus => {
            let mut forbidden = open.clone();
            forbidden.push(crate::corpus::CORPUS_REPOSITORY_URL.to_string());
            (proprietary.clone(), forbidden, "proprietary")
        }
        _ => (open.clone(), proprietary.clone(), "open"),
    };
    let Some(first) = allowed.first() else {
        return PushOutcome::Skipped {
            reason: format!("no {what} remote is configured in kovan_root.toml"),
        };
    };
    if allowed
        .iter()
        .any(|u| normalize_url(u) != normalize_url(first))
    {
        return PushOutcome::Refused {
            reason: format!(
                "kovan_root.toml names two different {what} remotes ({}); make them agree",
                allowed.join(" and ")
            ),
        };
    }
    if let Some(bad) = forbidden
        .iter()
        .find(|f| normalize_url(f) == normalize_url(first))
    {
        return PushOutcome::Refused {
            reason: format!(
                "the configured {what} remote {first} is also configured as another corpus's \
                 remote ({bad}); refusing so {what} documents cannot go to the wrong repository"
            ),
        };
    }
    let branch = gitmodules_branch(root, dir);
    push_repo(dir, Some((first.as_str(), what)), branch)
}

/// Push the repository at `dir` to its remote's branch, fast-forward only.
///
/// `expected`, when given, is `(url, kind)`: every push URL of the remote
/// must be `url`, else the push is refused. `tracked_branch` is the branch a
/// detached `HEAD` belongs on (`.gitmodules` `branch =`), if known.
fn push_repo(
    dir: &Path,
    expected: Option<(&str, &str)>,
    tracked_branch: Option<String>,
) -> PushOutcome {
    if !dir.join(".git").exists() {
        return PushOutcome::Skipped {
            reason: "not a Git repository".into(),
        };
    }
    let Some(remote) = pick_remote(dir) else {
        return PushOutcome::Skipped {
            reason: "this repository has no remote (or several and none is `origin`)".into(),
        };
    };
    let urls = match git_ok(dir, &["remote", "get-url", "--push", "--all", &remote]) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect::<Vec<_>>(),
        Err(message) => return PushOutcome::Failed { message },
    };
    if urls.is_empty() {
        return PushOutcome::Skipped {
            reason: format!("remote `{remote}` has no URL"),
        };
    }
    if let Some((want, kind)) = expected {
        if let Some(bad) = urls
            .iter()
            .find(|u| normalize_url(u) != normalize_url(want))
        {
            return PushOutcome::Refused {
                reason: format!(
                    "`{remote}` pushes to {bad}, which is not the configured {kind} remote \
                     {want}; nothing was sent"
                ),
            };
        }
    }
    if urls.iter().any(|u| u.contains("outram-park-backend")) {
        return PushOutcome::Refused {
            reason: "Kovan never pushes to outram-park-backend".into(),
        };
    }
    if git_ok(dir, &["rev-parse", "--verify", "-q", "HEAD"]).is_err() {
        return PushOutcome::Skipped {
            reason: "nothing committed yet".into(),
        };
    }

    let (branch, attached) = match git_ok(dir, &["symbolic-ref", "-q", "--short", "HEAD"]) {
        Ok(b) => (b.trim().to_string(), false),
        Err(_) => match attach_detached_head(dir, &remote, tracked_branch) {
            Ok(b) => (b, true),
            Err(reason) => return PushOutcome::Refused { reason },
        },
    };

    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    let out = match git(dir, &["push", "--porcelain", &remote, &refspec]) {
        Ok(o) => o,
        Err(message) => return PushOutcome::Failed { message },
    };
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let flag = stdout
        .lines()
        .find(|l| l.contains(&refspec))
        .and_then(|l| l.chars().next());
    let remote_url = urls[0].clone();
    match (out.status.success(), flag) {
        (true, Some('=')) => PushOutcome::UpToDate { branch },
        (true, _) => PushOutcome::Pushed {
            remote_url,
            branch,
            attached,
        },
        (false, _) => {
            let said = format!("{}\n{stderr}", stdout.trim()).trim().to_string();
            let lowered = said.to_ascii_lowercase();
            let message = if flag == Some('!')
                || lowered.contains("non-fast-forward")
                || lowered.contains("fetch first")
            {
                format!(
                    "the remote has commits this folder does not have — pull first. Nothing \
                     was overwritten and your saved commit is kept. Git said: {said}"
                )
            } else {
                format!("Git said: {said}")
            };
            PushOutcome::Failed { message }
        }
    }
}

/// Put a detached `HEAD`'s commit on its branch by fast-forwarding that
/// branch to it, then check the branch out (the same commit, so the files
/// are not touched). Returns the branch, or why it was left alone.
fn attach_detached_head(
    dir: &Path,
    remote: &str,
    tracked_branch: Option<String>,
) -> Result<String, String> {
    let branch = tracked_branch
        .or_else(|| remote_default_branch(dir, remote))
        .ok_or_else(|| {
            "the save is on a detached HEAD and neither .gitmodules nor the remote names the \
             branch it belongs on; nothing was pushed"
                .to_string()
        })?;
    if git_ok(dir, &["check-ref-format", "--branch", &branch]).is_err() {
        return Err(format!("`{branch}` is not a valid branch name"));
    }
    for base in [
        format!("refs/heads/{branch}"),
        format!("refs/remotes/{remote}/{branch}"),
    ] {
        if git_ok(dir, &["rev-parse", "--verify", "-q", &base]).is_err() {
            continue;
        }
        if git_ok(dir, &["merge-base", "--is-ancestor", &base, "HEAD"]).is_err() {
            return Err(format!(
                "the save is on a detached HEAD that does not contain `{base}`, so `{branch}` \
                 cannot simply fast-forward to it; nothing was moved or pushed — merge it onto \
                 `{branch}` by hand"
            ));
        }
    }
    git_ok(dir, &["checkout", "-q", "-B", &branch])
        .map_err(|e| format!("could not put the save on `{branch}`: {e}"))?;
    if git_ok(
        dir,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("refs/remotes/{remote}/{branch}"),
        ],
    )
    .is_ok()
    {
        let _ = git_ok(
            dir,
            &[
                "branch",
                "-q",
                &format!("--set-upstream-to={remote}/{branch}"),
                &branch,
            ],
        );
    }
    Ok(branch)
}

/// The remote's default branch: the local `refs/remotes/<remote>/HEAD`, else
/// asked of the remote (`git ls-remote --symref`).
fn remote_default_branch(dir: &Path, remote: &str) -> Option<String> {
    if let Ok(r) = git_ok(
        dir,
        &[
            "symbolic-ref",
            "-q",
            "--short",
            &format!("refs/remotes/{remote}/HEAD"),
        ],
    ) {
        let r = r.trim();
        if let Some(b) = r.strip_prefix(&format!("{remote}/")) {
            return Some(b.to_string());
        }
    }
    let text = git_ok(dir, &["ls-remote", "--symref", remote, "HEAD"]).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("ref: refs/heads/"))
        .and_then(|rest| rest.split_whitespace().next())
        .map(String::from)
}

/// The `branch =` that the Kovan repository's `.gitmodules` gives for the
/// submodule mounted at `dir`, if any.
fn gitmodules_branch(root: &KovanRoot, dir: &Path) -> Option<String> {
    let rel = dir.strip_prefix(root.path()).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    let file = root.path().join(".gitmodules");
    if !file.exists() {
        return None;
    }
    let file = file.to_string_lossy().to_string();
    let paths = git_ok(
        root.path(),
        &[
            "config",
            "-f",
            &file,
            "--get-regexp",
            r"^submodule\..*\.path$",
        ],
    )
    .ok()?;
    let key = paths.lines().find_map(|l| {
        let (k, v) = l.split_once(' ')?;
        (v.trim() == rel).then(|| k.trim_end_matches(".path").to_string())
    })?;
    let branch = git_ok(
        root.path(),
        &["config", "-f", &file, "--get", &format!("{key}.branch")],
    )
    .ok()?
    .trim()
    .to_string();
    // `.` means "the superproject's branch name" — not a branch to attach to.
    (!branch.is_empty() && branch != ".").then_some(branch)
}

/// `origin` if present, otherwise the only remote.
fn pick_remote(dir: &Path) -> Option<String> {
    let text = git_ok(dir, &["remote"]).ok()?;
    let remotes: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if remotes.contains(&"origin") {
        Some("origin".into())
    } else if remotes.len() == 1 {
        Some(remotes[0].to_string())
    } else {
        None
    }
}

/// A corpus [`pull_corpora`] considers, in the order it considers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusKind {
    /// The private literature submodule.
    Proprietary,
    /// The user's open corpus.
    Open,
    /// The read-only standard corpus.
    Standard,
}

impl CorpusKind {
    /// The label the UI shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Proprietary => "Proprietary corpus",
            Self::Open => "Open corpus",
            Self::Standard => "Standard corpus",
        }
    }
}

/// What [`pull_corpora`] did to one corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusPullOutcome {
    /// Moved from `from` to the remote's tip `to`, and left on `branch`.
    Updated {
        branch: String,
        from: String,
        to: String,
    },
    /// Already at the remote's tip; now on `branch` (it may have been
    /// detached at that same commit before).
    UpToDate { branch: String },
    /// Not attempted: not downloaded here, no remote, no system `git`.
    Skipped { reason: String },
    /// Following the remote would destroy local work (`reason` says which),
    /// so nothing was touched. The caller asks the user, and on "yes"
    /// overrides with [`crate::advanced_git::force_pull_in`] against
    /// `remote`/`branch`.
    NeedsConfirmation {
        remote: String,
        branch: String,
        reason: String,
    },
    /// A `git` step failed, with Git's words. Nothing was overridden.
    Failed { message: String },
}

/// One corpus's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusPull {
    pub corpus: CorpusKind,
    pub dir: PathBuf,
    pub outcome: CorpusPullOutcome,
}

impl CorpusPull {
    /// One human-readable line, e.g. `Open corpus: updated main 1a2b3c4..5d6e7f8`.
    pub fn line(&self) -> String {
        let short = |s: &str| s.chars().take(7).collect::<String>();
        let what = match &self.outcome {
            CorpusPullOutcome::Updated { branch, from, to } => {
                format!("updated {branch} {}..{}", short(from), short(to))
            }
            CorpusPullOutcome::UpToDate { branch } => format!("up to date ({branch})"),
            CorpusPullOutcome::Skipped { reason } => format!("not pulled — {reason}"),
            CorpusPullOutcome::NeedsConfirmation { reason, .. } => {
                format!("NOT pulled yet — {reason}; asking before overriding")
            }
            CorpusPullOutcome::Failed { message } => format!("pull FAILED — {message}"),
        };
        format!("{}: {what}", self.corpus.label())
    }
}

/// Bring every downloaded corpus to its remote's branch tip — run after the
/// Kovan folder itself was pulled (GH issue #422; maintainer, 2026-09-29:
/// *"when pulling from kovan corpus, i want the submodules to pull in and
/// override the local one as well"*).
///
/// Without this, a corpus that another clone saved into falls behind its
/// remote, and the next [`push_after_save`] refuses it (its detached save
/// does not contain the remote branch, so it cannot fast-forward).
///
/// For each of the proprietary, open and standard corpus, when downloaded:
/// fetch the tracked branch (`.gitmodules` `branch =`, else the remote's
/// default — the same rule the push uses), then
///
/// - **nothing local would be lost** (a clean tree, `HEAD` an ancestor of
///   the fetched tip): `git checkout -B <branch> FETCH_HEAD`. The corpus is
///   left *on the branch*, not detached, so the next save pushes cleanly.
/// - **something would be lost** (uncommitted or untracked files, or commits
///   the remote does not have): nothing is touched, and the outcome is
///   [`CorpusPullOutcome::NeedsConfirmation`]. Overriding destroys work, so
///   it is the caller's to ask about (the #279 prompt), never done here.
///
/// The gitlinks in the Kovan folder are not committed here; the next Save
/// records the corpora where they now are.
pub fn pull_corpora(root: &KovanRoot) -> Vec<CorpusPull> {
    let corpora = [
        (CorpusKind::Proprietary, root.restricted_sources_dir()),
        (CorpusKind::Open, root.open_corpus_dir()),
        (CorpusKind::Standard, root.standard_corpus_dir()),
    ];
    let git_ok_here = crate::advanced_git::system_git_available();
    let mut out: Vec<CorpusPull> = Vec::new();
    for (corpus, dir) in corpora {
        // Two corpora configured on one folder: pull it once.
        if out.iter().any(|p| same_dir(&p.dir, &dir)) {
            continue;
        }
        let outcome = if git_ok_here {
            pull_one_corpus(root, &dir)
        } else {
            CorpusPullOutcome::Skipped {
                reason: "no usable system `git`, which pulling needs".into(),
            }
        };
        out.push(CorpusPull {
            corpus,
            dir,
            outcome,
        });
    }
    out
}

/// [`pull_corpora`] for the corpus at `dir`.
fn pull_one_corpus(root: &KovanRoot, dir: &Path) -> CorpusPullOutcome {
    if !dir.join(".git").exists() {
        return CorpusPullOutcome::Skipped {
            reason: "not downloaded in this Kovan folder".into(),
        };
    }
    let Some(remote) = pick_remote(dir) else {
        return CorpusPullOutcome::Skipped {
            reason: "no remote (or several and none is `origin`)".into(),
        };
    };
    let Some(branch) =
        gitmodules_branch(root, dir).or_else(|| remote_default_branch(dir, &remote))
    else {
        return CorpusPullOutcome::Skipped {
            reason: "neither .gitmodules nor the remote names a branch to follow".into(),
        };
    };
    if let Err(message) = git_ok(dir, &["fetch", "-q", &remote, &branch]) {
        return CorpusPullOutcome::Failed { message };
    }
    let fetched = match git_ok(dir, &["rev-parse", "FETCH_HEAD"]) {
        Ok(s) => s.trim().to_string(),
        Err(message) => return CorpusPullOutcome::Failed { message },
    };
    let head = git_ok(dir, &["rev-parse", "--verify", "-q", "HEAD"])
        .map(|s| s.trim().to_string())
        .ok();

    let dirty = match git_ok(dir, &["status", "--porcelain"]) {
        Ok(s) => !s.trim().is_empty(),
        Err(message) => return CorpusPullOutcome::Failed { message },
    };
    let behind_only = head.as_deref().is_none_or(|h| {
        git_ok(dir, &["merge-base", "--is-ancestor", h, &fetched]).is_ok()
    });
    if dirty || !behind_only {
        let reason = match (dirty, behind_only) {
            (true, false) => "it has unsaved files and saves the remote does not have",
            (true, true) => "it has unsaved (uncommitted or untracked) files",
            _ => "it has saves the remote does not have",
        };
        return CorpusPullOutcome::NeedsConfirmation {
            remote,
            branch,
            reason: reason.into(),
        };
    }

    if let Err(message) = git_ok(dir, &["checkout", "-q", "-B", &branch, &fetched]) {
        return CorpusPullOutcome::Failed { message };
    }
    let tracking = format!("refs/remotes/{remote}/{branch}");
    if git_ok(dir, &["rev-parse", "--verify", "-q", &tracking]).is_ok() {
        let _ = git_ok(
            dir,
            &[
                "branch",
                "-q",
                &format!("--set-upstream-to={remote}/{branch}"),
                &branch,
            ],
        );
    }
    match head {
        Some(from) if from == fetched => CorpusPullOutcome::UpToDate { branch },
        from => CorpusPullOutcome::Updated {
            branch,
            from: from.unwrap_or_default(),
            to: fetched,
        },
    }
}

/// A remote URL reduced to what identifies the repository, so the
/// `https://`, `ssh://` and `git@host:` spellings of one GitHub repository
/// compare equal: scheme, user and a trailing `.git` or `/` are dropped and
/// the host is lower-cased. A local path is compared as written, less a
/// trailing `/` or `.git`.
pub fn normalize_url(url: &str) -> String {
    let mut u = url.trim().to_string();
    let had_scheme = if let Some(pos) = u.find("://") {
        u = u[pos + 3..].to_string();
        true
    } else {
        false
    };
    if let Some(at) = u.find('@') {
        if !u[..at].contains('/') {
            u = u[at + 1..].to_string();
        }
    }
    if !had_scheme {
        // scp-like `host:owner/repo`, but not a Windows drive or a local path.
        if let Some(colon) = u.find(':') {
            let host = &u[..colon];
            if colon > 1 && !host.contains('/') {
                u = format!("{host}/{}", &u[colon + 1..]);
            }
        }
    }
    while u.ends_with('/') {
        u.pop();
    }
    if let Some(stripped) = u.strip_suffix(".git") {
        u = stripped.to_string();
    }
    match u.split_once('/') {
        Some((host, rest)) if !host.is_empty() => format!("{}/{rest}", host.to_ascii_lowercase()),
        _ => u,
    }
}

/// Whether two paths name the same directory.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Run the system `git` in `dir`, never prompting on a terminal.
fn git(dir: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("could not run git: {e}"))
}

/// [`git`], failing on a non-zero exit with Git's stderr.
fn git_ok(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git(dir, args)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests;
