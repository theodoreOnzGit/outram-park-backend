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
//! Since GitHub issue #458 a tier may hold several repositories
//! ([`crate::corpus_tiers`]); each is pushed **independently**, one result
//! per repository:
//!
//! 1. every **proprietary** repository (the first is
//!    [`KovanRoot::restricted_sources_dir`]),
//! 2. every **open** repository (the first is [`KovanRoot::open_corpus_dir`]),
//! 3. every **standard** repository configured `writable = true` (none by
//!    default),
//! 4. the **Kovan repository** itself — only if no corpus push failed
//!    or was refused, so the parent never publishes a gitlink to a corpus
//!    commit that is not on its remote. A corpus that is merely *skipped*
//!    (not downloaded here, or no remote configured for it) does not hold
//!    the parent back: that corpus was never going to be pushed from here.
//!
//! A **standard repository is never pushed** unless configured `writable`:
//! it is read-only to everyone but its maintainer, and Save never commits
//! into it. A checkout mounted in two tiers is pushed once.
//!
//! The report also carries a **size warning** for every checkout near
//! GitHub's recommended 1 GB ([`crate::corpus_tiers::size_warning`]),
//! suggesting another repository in the same tier.
//!
//! # Safety rules (non-negotiable, each pinned by a test)
//!
//! - **Never forced.** The refspec is `refs/heads/B:refs/heads/B` with no
//!   `+` and no `--force`, so Git itself refuses anything but a
//!   fast-forward.
//! - **A remote that has moved on is merged, never reset to** (GH issue
//!   #502). ~~A remote that has moved on is reported as "pull first"; the
//!   local commit is kept.~~ **CHANGED 2026-10-07:** "pull first" sent the
//!   user to Pull, whose forced pull then `reset --hard` the save away. Now
//!   [`safe_push::push_keeping_local`] fetches, merges the remote's tip into
//!   the save and pushes again; if the two do not merge cleanly the merge
//!   is aborted, the save stays committed locally, and the outcome is
//!   [`PushOutcome::KeptLocally`] with a typed
//!   [`safe_push::SafePushError`]. Merge rather than rebase so the commit a
//!   Save's gitlink records stays on the pushed history.
//! - **Never from a detached `HEAD`.** `git submodule update` leaves a
//!   submodule detached, and a Save then commits onto no branch. Before
//!   pushing, the commit is put on the submodule's tracked branch
//!   (`.gitmodules` `branch =`, else the remote's default branch), but only
//!   when that branch's tip — local and remote-tracking — is an ancestor of
//!   the commit, i.e. the branch fast-forwards. Otherwise nothing is moved
//!   and the push is refused.
//! - **Each corpus goes only to its own configured remote.** Every push URL
//!   of a proprietary repository's remote must be that repository's
//!   configured remote (for the first one, `[private_submodule] remote`
//!   and/or `[corpora] proprietary_remote`, which must agree), and an open
//!   repository's must be its own remote and must not be any proprietary
//!   one. A mismatch is refused, so a proprietary PDF cannot reach a public
//!   repository through a mis-set `origin`.
//! - **Proprietary never goes to a public remote** (#458). A proprietary
//!   repository's remote is refused when it is any standard or open
//!   repository's remote, Kovan's built-in standard corpus, a
//!   `[repos] known_public` entry or an `outram-park-backend` URL
//!   ([`crate::corpus_tiers::public_remote_reason`]), or when it is an
//!   HTTPS (or GitHub `git@`) remote that `git ls-remote` can read with
//!   no credential of the user's in reach
//!   ([`crate::corpus_tiers::anonymously_readable`]).
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

use crate::corpus_tiers::{CorpusRepo, RepoOrigin, Tier};
use crate::root::KovanRoot;

pub mod safe_push;
use safe_push::{BranchRule, SafePushError, SafePushOk};

/// One of the repositories a Save pushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushRepo {
    /// A proprietary repository (the private literature submodule).
    ProprietaryCorpus,
    /// An open repository.
    OpenCorpus,
    /// A standard repository configured `writable` (#458).
    StandardCorpus,
    /// The Kovan folder's own repository.
    KovanRepository,
}

impl PushRepo {
    /// The label the UI shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::ProprietaryCorpus => "Proprietary corpus",
            Self::OpenCorpus => "Open corpus",
            Self::StandardCorpus => "Standard corpus",
            Self::KovanRepository => "Kovan repository",
        }
    }

    /// The kind for a corpus repository of `tier`.
    pub fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Proprietary => Self::ProprietaryCorpus,
            Tier::Open => Self::OpenCorpus,
            Tier::Standard => Self::StandardCorpus,
        }
    }
}

/// What happened to one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOutcome {
    /// New commits were pushed to `remote_url`'s `branch`.
    /// `attached` is set when the commit was on a detached `HEAD` and was
    /// first put on `branch` (a fast-forward of that branch). `merged` is
    /// the remote's tip merged into the save first because the remote had
    /// moved on (GH issue #502), or `None` for a plain fast-forward.
    Pushed {
        remote_url: String,
        branch: String,
        attached: bool,
        merged: Option<String>,
    },
    /// The remote already had everything: nothing to push.
    UpToDate { branch: String },
    /// Not attempted, by design (not downloaded here, no remote configured,
    /// no system `git`, or a corpus before it did not make it).
    Skipped { reason: String },
    /// Refused by a safety rule before anything was sent.
    Refused { reason: String },
    /// `git push` ran and failed: an authentication failure, the network —
    /// with Git's own words.
    Failed { message: String },
    /// The remote has moved on and could not be combined with the save
    /// automatically (GH issue #502): the save is **committed locally**,
    /// nothing was reset or discarded, and `error` says what to resolve.
    KeptLocally { error: SafePushError },
}

impl PushOutcome {
    /// Whether this blocks the Kovan repository's push (and should be shown
    /// as a problem).
    pub fn is_problem(&self) -> bool {
        matches!(
            self,
            Self::Refused { .. } | Self::Failed { .. } | Self::KeptLocally { .. }
        )
    }
}

/// One repository's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPush {
    pub repo: PushRepo,
    /// The corpus repository's name ([`crate::corpus_tiers::CorpusRepo::name`]);
    /// empty for the Kovan repository.
    pub name: String,
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
                merged,
            } => {
                let attached = if *attached {
                    " (the save was on a detached HEAD; it was put on this branch first)"
                } else {
                    ""
                };
                let merged = match merged {
                    Some(tip) => format!(
                        " (the remote had moved on; its {} was merged in first, nothing was \
                         discarded)",
                        tip.chars().take(7).collect::<String>()
                    ),
                    None => String::new(),
                };
                format!("pushed {branch} to {remote_url}{attached}{merged}")
            }
            PushOutcome::UpToDate { branch } => format!("nothing to push ({branch} is up to date)"),
            PushOutcome::Skipped { reason } => format!("not pushed — {reason}"),
            PushOutcome::Refused { reason } => format!("REFUSED — {reason}"),
            PushOutcome::Failed { message } => format!("push FAILED — {message}"),
            PushOutcome::KeptLocally { error } => format!("NOT pushed — {error}"),
        };
        if self.name.is_empty() {
            format!("{}: {what}", self.repo.label())
        } else {
            format!("{} ({}): {what}", self.repo.label(), self.name)
        }
    }
}

/// Every repository's result, in push order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushReport {
    pub repos: Vec<RepoPush>,
    /// Checkouts near GitHub's recommended 1 GB
    /// ([`crate::corpus_tiers::size_warning`]); advice, not a problem.
    pub warnings: Vec<String>,
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

    /// One line per repository ([`RepoPush::line`]), then one per warning.
    pub fn lines(&self) -> Vec<String> {
        self.repos
            .iter()
            .map(RepoPush::line)
            .chain(self.warnings.iter().map(|w| format!("Warning: {w}")))
            .collect()
    }

    /// Every result for repositories of kind `repo`, in push order (a tier
    /// may hold several, #458).
    pub fn all(&self, repo: PushRepo) -> Vec<&RepoPush> {
        self.repos.iter().filter(|r| r.repo == repo).collect()
    }

    /// The result of the corpus repository named `name`, if it was
    /// considered.
    pub fn named(&self, name: &str) -> Option<&PushOutcome> {
        self.repos
            .iter()
            .find(|r| r.name == name)
            .map(|r| &r.outcome)
    }
}

/// The corpus repositories a push considers, in push order: every
/// proprietary one, every open one, then every `writable` standard one; a
/// checkout mounted twice is listed once.
fn push_targets(all: &[CorpusRepo]) -> Vec<CorpusRepo> {
    let mut out: Vec<CorpusRepo> = Vec::new();
    for tier in [Tier::Proprietary, Tier::Open, Tier::Standard] {
        for r in all.iter().filter(|r| r.tier == tier && r.writable) {
            if !out.iter().any(|t| same_dir(&t.dir, &r.dir)) {
                out.push(r.clone());
            }
        }
    }
    out
}

/// Push every corpus repository and then the Kovan repository, under the
/// module's safety rules. Never panics; every problem is a [`PushOutcome`]
/// in the report.
///
/// Called after a successful save whether or not that save committed
/// anything, so commits from earlier saves that were never pushed (every
/// save before 2026-09-28) go up too.
pub fn push_after_save(root: &KovanRoot) -> PushReport {
    push_after_save_warning_at(root, crate::corpus_tiers::SIZE_WARN_BYTES)
}

/// [`push_after_save`], warning about checkouts of `size_warn_bytes` or
/// more instead of [`crate::corpus_tiers::SIZE_WARN_BYTES`] (for tests).
pub fn push_after_save_warning_at(root: &KovanRoot, size_warn_bytes: u64) -> PushReport {
    let all = root.corpus_repos();
    let targets = push_targets(&all);
    let mut report = PushReport::default();
    let mut seen: Vec<PathBuf> = Vec::new();
    for r in &all {
        if r.is_downloaded() && !seen.iter().any(|d| same_dir(d, &r.dir)) {
            seen.push(r.dir.clone());
            report
                .warnings
                .extend(crate::corpus_tiers::size_warning(r, size_warn_bytes));
        }
    }
    if !crate::advanced_git::system_git_available() {
        let skipped = || PushOutcome::Skipped {
            reason: "no usable system `git`, which pushing needs".into(),
        };
        for r in &targets {
            report.repos.push(RepoPush {
                repo: PushRepo::for_tier(r.tier),
                name: r.name.clone(),
                dir: r.dir.clone(),
                outcome: skipped(),
            });
        }
        report.repos.push(RepoPush {
            repo: PushRepo::KovanRepository,
            name: String::new(),
            dir: root.path().to_path_buf(),
            outcome: skipped(),
        });
        return report;
    }

    for r in &targets {
        let outcome = push_corpus(root, &all, r);
        report.repos.push(RepoPush {
            repo: PushRepo::for_tier(r.tier),
            name: r.name.clone(),
            dir: r.dir.clone(),
            outcome,
        });
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
        name: String::new(),
        dir,
        outcome,
    });
    report
}

/// Push one corpus repository after checking which remote it may go to
/// (`all` is every repository of the folder, for the cross-tier checks).
fn push_corpus(root: &KovanRoot, all: &[CorpusRepo], repo: &CorpusRepo) -> PushOutcome {
    let dir = repo.dir.as_path();
    if !repo.is_downloaded() {
        return PushOutcome::Skipped {
            reason: "not downloaded in this Kovan folder".into(),
        };
    }
    if !repo.writable {
        return PushOutcome::Refused {
            reason: "this standard repository is read-only; Kovan never pushes it".into(),
        };
    }
    if repo.tier != Tier::Standard && same_dir(dir, &root.standard_corpus_dir()) {
        return PushOutcome::Refused {
            reason: "this folder is the standard corpus, which Kovan never pushes".into(),
        };
    }
    let cfg = root.config();
    let mut proprietary: Vec<String> = all
        .iter()
        .filter(|r| r.tier == Tier::Proprietary)
        .filter_map(|r| r.remote.clone())
        .collect();
    proprietary.extend(cfg.private_submodule.as_ref().map(|p| p.remote.clone()));
    proprietary.extend(cfg.corpora.proprietary_remote.clone());

    let what = repo.tier.key();
    let mut allowed: Vec<String> = repo.remote.clone().into_iter().collect();
    if repo.tier == Tier::Proprietary && repo.origin == RepoOrigin::Legacy {
        allowed.extend(cfg.private_submodule.as_ref().map(|p| p.remote.clone()));
        allowed.extend(cfg.corpora.proprietary_remote.clone());
    }
    let Some(first) = allowed.first().cloned() else {
        return PushOutcome::Skipped {
            reason: format!("no {what} remote is configured in kovan_root.toml"),
        };
    };
    if allowed
        .iter()
        .any(|u| normalize_url(u) != normalize_url(&first))
    {
        return PushOutcome::Refused {
            reason: format!(
                "kovan_root.toml names two different remotes for the {what} repository {:?} \
                 ({}); make them agree",
                repo.name,
                allowed.join(" and ")
            ),
        };
    }
    if repo.tier == Tier::Proprietary {
        if let Some(why) = crate::corpus_tiers::public_remote_reason(cfg, all, &first) {
            return PushOutcome::Refused {
                reason: format!(
                    "the proprietary repository {:?} would push to {first}, but {why}; refusing \
                     so proprietary documents cannot reach a public repository",
                    repo.name
                ),
            };
        }
        if crate::corpus_tiers::anonymously_readable(&first) == Some(true) {
            return PushOutcome::Refused {
                reason: format!(
                    "the proprietary repository {:?} would push to {first}, which can be read \
                     without any credentials, so it is public; make it private first",
                    repo.name
                ),
            };
        }
    } else if let Some(bad) = proprietary
        .iter()
        .find(|p| normalize_url(p) == normalize_url(&first))
    {
        return PushOutcome::Refused {
            reason: format!(
                "the configured {what} remote {first} is also configured as a proprietary \
                 remote ({bad}); refusing so documents cannot go to the wrong repository"
            ),
        };
    }
    let branch = repo.branch.clone().or_else(|| gitmodules_branch(root, dir));
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

    // GH issue #502: a remote that moved on is merged into the save and
    // pushed again, never reset to; see `safe_push`.
    let remote_url = urls[0].clone();
    match safe_push::push_keeping_local(dir, &remote, &branch, BranchRule::AnyBranch) {
        Ok(SafePushOk::UpToDate) => PushOutcome::UpToDate { branch },
        Ok(SafePushOk::Pushed { merged }) => PushOutcome::Pushed {
            remote_url,
            branch,
            attached,
            merged,
        },
        Err(e @ (SafePushError::MainRefused { .. } | SafePushError::NotOnBranch { .. })) => {
            PushOutcome::Refused {
                reason: e.to_string(),
            }
        }
        Err(SafePushError::Git { message }) => PushOutcome::Failed {
            message: format!("Git said: {message}"),
        },
        Err(error) => PushOutcome::KeptLocally { error },
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

/// The tier of a corpus [`pull_corpora`] considers (one per repository
/// since #458; a tier may appear several times).
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
    /// Was not downloaded in this Kovan folder, and has now been fetched from
    /// its configured remote (a registered submodule initialised, or a new
    /// one added) and left on `branch` at `head`. A plain clone of someone's
    /// Kovan folder, or a folder whose corpora were configured but never
    /// fetched, lands here on its first pull.
    Downloaded { branch: String, head: String },
    /// Not attempted: not downloaded here and no remote configured to
    /// download it from, or no system `git`.
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

impl CorpusKind {
    /// The kind for a repository of `tier`.
    pub fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Proprietary => Self::Proprietary,
            Tier::Open => Self::Open,
            Tier::Standard => Self::Standard,
        }
    }
}

/// One corpus repository's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusPull {
    pub corpus: CorpusKind,
    /// The repository's name ([`crate::corpus_tiers::CorpusRepo::name`]).
    pub name: String,
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
            CorpusPullOutcome::Downloaded { branch, head } => {
                format!("downloaded (was missing), on {branch} at {}", short(head))
            }
            CorpusPullOutcome::Skipped { reason } => format!("not pulled — {reason}"),
            CorpusPullOutcome::NeedsConfirmation { reason, .. } => {
                format!("NOT pulled yet — {reason}; asking before overriding")
            }
            CorpusPullOutcome::Failed { message } => format!("pull FAILED — {message}"),
        };
        if self.name.is_empty() {
            format!("{}: {what}", self.corpus.label())
        } else {
            format!("{} ({}): {what}", self.corpus.label(), self.name)
        }
    }
}

/// Bring every corpus to its remote's branch tip, downloading any not yet here — run after the
/// Kovan folder itself was pulled (GH issue #422; maintainer, 2026-09-29:
/// *"when pulling from kovan corpus, i want the submodules to pull in and
/// override the local one as well"*).
///
/// Without this, a corpus that another clone saved into falls behind its
/// remote, and the next [`push_after_save`] refuses it (its detached save
/// does not contain the remote branch, so it cannot fast-forward).
///
/// For each corpus repository — every proprietary, every open and every
/// standard one (#458) — when downloaded:
/// fetch the tracked branch (`.gitmodules` `branch =`, else the remote's
/// default — the same rule the push uses), then
///
/// - **nothing local would be lost** (a clean tree, `HEAD` and the local
///   branch both ancestors of the fetched tip): `git checkout -B <branch>
///   FETCH_HEAD`. The corpus is
///   left *on the branch*, not detached, so the next save pushes cleanly.
/// - **something would be lost** (uncommitted or untracked files, or commits
///   the remote does not have): nothing is touched, and the outcome is
///   [`CorpusPullOutcome::NeedsConfirmation`]. Overriding destroys work, so
///   it is the caller's to ask about (the #279 prompt), never done here.
///
/// The gitlinks in the Kovan folder are not committed here; the next Save
/// records the corpora where they now are.
///
/// **A corpus that is configured but not downloaded is downloaded** (the
/// open and proprietary corpora from `[corpora]`, the standard corpus from
/// [`crate::corpus::CORPUS_REPOSITORY_URL`]), through
/// [`crate::corpus_repos::ensure_corpus`], then followed as above
/// ([`CorpusPullOutcome::Downloaded`]). Before this, Pull reported such a
/// corpus as "not downloaded" and left it empty, so a fresh clone of a Kovan
/// folder never got its literature (maintainer, 2026-09-30: *"make sure the
/// pull button from kovan gui also pulls in both corpuses"*).
pub fn pull_corpora(root: &KovanRoot) -> Vec<CorpusPull> {
    pull_corpora_with(
        root,
        crate::corpus::CORPUS_REPOSITORY_URL,
        crate::corpus::CORPUS_REPOSITORY_BRANCH,
    )
}

/// [`pull_corpora`] with the **built-in** standard corpus's remote and
/// branch given, so tests can use a local repository instead of the network
/// (as [`crate::corpus_repos::ensure_library_corpora_with`] does). Other
/// repositories use their own configured remotes and branches.
pub fn pull_corpora_with(
    root: &KovanRoot,
    standard_remote: &str,
    standard_branch: &str,
) -> Vec<CorpusPull> {
    let all = root.corpus_repos();
    let git_ok_here = crate::advanced_git::system_git_available();
    let mut out: Vec<CorpusPull> = Vec::new();
    let mut builtin_seen = false;
    let ordered = [Tier::Proprietary, Tier::Open, Tier::Standard]
        .into_iter()
        .flat_map(|tier| all.iter().filter(move |r| r.tier == tier));
    for r in ordered {
        let (remote, branch) = if r.origin == RepoOrigin::Builtin && !builtin_seen {
            builtin_seen = true;
            (
                Some(standard_remote.to_string()),
                Some(standard_branch.to_string()),
            )
        } else {
            (
                crate::corpus_tiers::setup_remote(root.config(), r),
                r.branch.clone(),
            )
        };
        let (corpus, name, dir) = (CorpusKind::for_tier(r.tier), r.name.clone(), r.dir.clone());
        // Two corpora configured on one folder: pull it once.
        if out.iter().any(|p| same_dir(&p.dir, &dir)) {
            continue;
        }
        let outcome = if !git_ok_here {
            CorpusPullOutcome::Skipped {
                reason: "no usable system `git`, which pulling needs".into(),
            }
        } else if dir.join(".git").exists() {
            pull_one_corpus(root, &dir, branch.as_deref())
        } else {
            download_corpus(root, &dir, remote.as_deref(), branch.as_deref())
        };
        out.push(CorpusPull {
            corpus,
            name,
            dir,
            outcome,
        });
    }
    out
}

/// [`pull_corpora_with`] for a corpus with no checkout at `dir`: fetch it
/// from `remote` with [`crate::corpus_repos::ensure_corpus`] (which never
/// overwrites files already there), then follow its branch as
/// [`pull_one_corpus`] does.
fn download_corpus(
    root: &KovanRoot,
    dir: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
) -> CorpusPullOutcome {
    let Some(url) = remote else {
        return CorpusPullOutcome::Skipped {
            reason: "not downloaded in this Kovan folder, and no remote is configured \
                     to download it from"
                .into(),
        };
    };
    if let Err(e) = crate::corpus_repos::ensure_corpus(root, dir, Some(url), branch) {
        return CorpusPullOutcome::Failed {
            message: format!("downloading it failed: {e}"),
        };
    }
    match pull_one_corpus(root, dir, branch) {
        CorpusPullOutcome::Updated { branch, to, .. } => CorpusPullOutcome::Downloaded {
            branch,
            head: to,
        },
        CorpusPullOutcome::UpToDate { branch } => {
            let head = git_ok(dir, &["rev-parse", "HEAD"])
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            CorpusPullOutcome::Downloaded { branch, head }
        }
        other => other,
    }
}

/// [`pull_corpora`] for the corpus at `dir`, following `branch_hint` (its
/// configured branch) when given, else the `.gitmodules` branch, else the
/// remote's default.
fn pull_one_corpus(root: &KovanRoot, dir: &Path, branch_hint: Option<&str>) -> CorpusPullOutcome {
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
    let Some(branch) = gitmodules_branch(root, dir)
        .or_else(|| branch_hint.map(String::from))
        .or_else(|| remote_default_branch(dir, &remote))
    else {
        return CorpusPullOutcome::Skipped {
            reason: "neither .gitmodules nor the remote names a branch to follow".into(),
        };
    };
    follow_branch(dir, remote, branch)
}

/// Fetch `branch` from `remote` into the repository at `dir` and move it
/// there, **only when nothing local would be lost**: a clean tree, and a
/// `HEAD` and local `branch` that are both ancestors of the fetched tip
/// (~~`HEAD` only~~ **CORRECTED 2026-10-07, GH #502**: the local branch was
/// not checked, so a detached `HEAD` behind an unpushed branch let
/// `checkout -B` drop that branch's commits). Otherwise nothing is
/// touched and the outcome is [`CorpusPullOutcome::NeedsConfirmation`].
/// [`pull_one_corpus`]'s second half, shared with
/// [`crate::corpus_repos::update_standard_corpus`].
pub(crate) fn follow_branch(dir: &Path, remote: String, branch: String) -> CorpusPullOutcome {
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
    // Both `HEAD` and the local `branch` must be contained in the fetched
    // tip: `checkout -B` below moves `branch`, so a branch with commits the
    // remote lacks — while `HEAD` is detached somewhere older — would
    // otherwise lose them from the branch silently (found for GH #502).
    let local_branch = format!("refs/heads/{branch}");
    let branch_tip = git_ok(dir, &["rev-parse", "--verify", "-q", &local_branch])
        .map(|s| s.trim().to_string())
        .ok();
    let behind_only = [head.as_deref(), branch_tip.as_deref()]
        .into_iter()
        .flatten()
        .all(|h| git_ok(dir, &["merge-base", "--is-ancestor", h, &fetched]).is_ok());
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
