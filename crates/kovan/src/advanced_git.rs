//! Advanced Git (§38, `op-9vo6.20`): real Git concepts, for the separate
//! "Advanced Git" tab/area §38 asks for — status/diff, branches, history,
//! remotes, fetch/pull/push.
//!
//! # Local vs. remote, strictly split
//!
//! §38 draws a hard line: local operations use `gix` "for guaranteed local
//! operations"; remote/network operations use the **system `git` binary**,
//! explicitly **not** gitoxide remotes. This module keeps that split at
//! the function level, not just in prose:
//!
//! - [`status`], [`local_branches`], [`history`] — local, `gix`-backed
//!   (reusing [`crate::repository::status`] and
//!   [`kovan_discovery::git::GitProvider`] rather than a second
//!   implementation of either).
//! - [`list_remotes`], [`fetch`], [`pull`], [`push`], [`force_pull_in`],
//!   [`abort_in_progress_in`] — shell out to the
//!   system `git` binary via [`std::process::Command`]. `kovan-discovery`'s
//!   `GixCliBackend` is **not** reused here even though its name suggests
//!   it might fit: it wraps the `gix` *CLI* (gitoxide's own binary, a
//!   different tool), and every one of its `GitBackend` methods is
//!   presently a stub that declines with "unsupported" — using it for
//!   remote operations would be silently wrong, not just redundant.
//!
//! # Kovan works without system Git
//!
//! §38: "Kovan remains fully functional without system Git; only remote
//! operations are unavailable." [`system_git_available`] is the one check
//! a caller needs — [`fetch`]/[`pull`]/[`push`]/[`list_remotes`] all
//! return [`RemoteError::GitUnavailable`] cleanly rather than panicking or
//! hanging when it is `false`.
//!
//! # A conflicted pull is a question, not just an error (GH issue #279)
//!
//! [`pull_in`] separates the one remote failure the user can answer —
//! the folder and the remote disagree — into [`RemoteError::Conflict`], so
//! the GUI can ask *"you may have unsaved changes, u sure u want to pull
//! anot?"* (the maintainer's own words, 2026-09-23) and act on the answer:
//! [`force_pull_in`] for "yes, can", [`abort_in_progress_in`] for "no, i
//! manage myself". [`force_pull_in`] **destroys uncommitted and untracked
//! work by design** — read its doc before calling it from anywhere else.

use std::process::Command;

use kovan_discovery::git::{CommitInfo, GitProvider};

use crate::repository::{self, RepositoryError, SaveSummary};
use crate::root::KovanRoot;

/// One local branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
}

/// Local working-tree status (§37/§38) — what [`crate::repository::save_repository`]
/// would commit, without committing it.
pub fn status(root: &KovanRoot) -> Result<SaveSummary, RepositoryError> {
    repository::status(root)
}

/// §37's "Save Repository" — `git add .` + `git commit`, deterministic, no
/// AI (`crate::repository::save_repository`'s own doc). `Ok(None)` means
/// there was nothing to commit. Exposed here, alongside [`status`], so the
/// Advanced Git view only ever imports from this module (op-nswf, GH issue
/// #35 2026-09-01 05:42: "Under the git tab, i expect to see save to
/// repository. I don't see any button" — the backend already existed and
/// was tested; it just had no button wired to it).
pub fn save(root: &KovanRoot) -> Result<Option<SaveSummary>, RepositoryError> {
    repository::save_repository(root)
}

/// Up to `max` commits of history, newest first — reuses
/// `kovan_discovery::git::GitProvider`, already this workspace's tested
/// git-history reader, rather than a second implementation.
pub fn history(root: &KovanRoot, max: usize) -> Result<Vec<CommitInfo>, RepositoryError> {
    let provider =
        GitProvider::open(root.path()).map_err(|e| RepositoryError::Git(e.to_string()))?;
    provider
        .history(max)
        .map_err(|e| RepositoryError::Git(e.to_string()))
}

/// Local branches (`refs/heads/*`), marking which one `HEAD` points at.
pub fn local_branches(root: &KovanRoot) -> Result<Vec<BranchInfo>, RepositoryError> {
    let repo = gix::open(root.path()).map_err(|e| RepositoryError::Git(e.to_string()))?;
    let current = repo
        .head_name()
        .ok()
        .flatten()
        .map(|n| n.shorten().to_string());

    let platform = repo
        .references()
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    let iter = platform
        .local_branches()
        .map_err(|e| RepositoryError::Git(e.to_string()))?;

    let mut out = Vec::new();
    for reference in iter {
        let reference = reference.map_err(|e| RepositoryError::Git(e.to_string()))?;
        let name = reference.name().shorten().to_string();
        let is_current = current.as_deref() == Some(name.as_str());
        out.push(BranchInfo { name, is_current });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

// ---------------------------------------------------------------------------
// Remote operations — system `git` only, per §38.
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum RemoteError {
    /// No usable system `git` binary — §38's "remains fully functional
    /// without system Git; only remote operations are unavailable".
    GitUnavailable,
    /// A [`pull_in`] Git refused, or could not finish, because the folder
    /// and the remote disagree — the one failure the user can be offered a
    /// way out of, rather than only shown (GH issue #279). Kept separate
    /// from [`Self::Failed`] so the caller can put up the "sure anot?"
    /// prompt for exactly this case and nothing else: a bad URL, a missing
    /// branch or a refused credential must still surface as a plain error.
    ///
    /// `output` is Git's own stdout **and** stderr, in that order — see
    /// [`is_conflict`] for why both are needed.
    Conflict {
        command: String,
        output: String,
    },
    /// `git` ran and exited non-zero.
    Failed {
        command: String,
        stderr: String,
    },
    Io(std::io::Error),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitUnavailable => write!(
                f,
                "no usable system `git` binary — remote operations are unavailable"
            ),
            Self::Conflict { command, output } => {
                write!(f, "`{command}` stopped on a conflict: {output}")
            }
            Self::Failed { command, stderr } => write!(f, "`{command}` failed: {stderr}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RemoteError {}

/// Whether the system `git` binary can be run at all.
pub fn system_git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run the system `git` in the repository at `dir` (any repository, not only
/// a Kovan folder: the corpus repositories too, #255), handing back the raw
/// [`std::process::Output`] — exit status, stdout **and** stderr — whatever
/// the exit status was.
///
/// [`run_git_in`] is the usual wrapper; this one exists because `git pull`
/// reports a merge conflict on **stdout** (`CONFLICT (content): Merge
/// conflict in …`, `Automatic merge failed …`), which a stderr-only error
/// path throws away (GH issue #279).
fn git_output_in(
    dir: &std::path::Path,
    args: &[&str],
) -> Result<std::process::Output, RemoteError> {
    if !system_git_available() {
        return Err(RemoteError::GitUnavailable);
    }
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(RemoteError::Io)
}

/// [`git_output_in`], failing on a non-zero exit with its stderr.
fn run_git_in(dir: &std::path::Path, args: &[&str]) -> Result<String, RemoteError> {
    let output = git_output_in(dir, args)?;
    if !output.status.success() {
        return Err(RemoteError::Failed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// One configured remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

/// The library's configured remotes (`git remote -v`, fetch URLs, deduped
/// by name).
pub fn list_remotes(root: &KovanRoot) -> Result<Vec<RemoteInfo>, RemoteError> {
    list_remotes_in(root.path())
}

/// The remotes of the repository at `dir` (see [`list_remotes`]).
pub fn list_remotes_in(dir: &std::path::Path) -> Result<Vec<RemoteInfo>, RemoteError> {
    let text = run_git_in(dir, &["remote", "-v"])?;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(url)) = (parts.next(), parts.next()) else {
            continue;
        };
        if seen.insert(name.to_string()) {
            out.push(RemoteInfo {
                name: name.to_string(),
                url: url.to_string(),
            });
        }
    }
    Ok(out)
}

/// `git fetch <remote>` — network I/O via the system binary, never gitoxide.
pub fn fetch(root: &KovanRoot, remote: &str) -> Result<String, RemoteError> {
    fetch_in(root.path(), remote)
}

/// `git pull <remote> <branch>`.
pub fn pull(root: &KovanRoot, remote: &str, branch: &str) -> Result<String, RemoteError> {
    pull_in(root.path(), remote, branch)
}

/// `git push <remote> <branch>`.
pub fn push(root: &KovanRoot, remote: &str, branch: &str) -> Result<String, RemoteError> {
    push_in(root.path(), remote, branch)
}

/// [`fetch`] in the repository at `dir`.
pub fn fetch_in(dir: &std::path::Path, remote: &str) -> Result<String, RemoteError> {
    run_git_in(dir, &["fetch", remote])
}

/// [`pull`] in the repository at `dir`.
///
/// A pull Git refuses, or leaves half-done, because the folder and the
/// remote disagree comes back as [`RemoteError::Conflict`] rather than
/// [`RemoteError::Failed`] (GH issue #279), so the GUI can offer
/// [`force_pull_in`] instead of only printing Git's complaint. Everything
/// else — unreachable remote, unknown branch, refused credentials — stays a
/// plain `Failed`.
pub fn pull_in(dir: &std::path::Path, remote: &str, branch: &str) -> Result<String, RemoteError> {
    let args = ["pull", remote, branch];
    let output = git_output_in(dir, &args)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() {
        return Ok(stdout.to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let command = format!("git {}", args.join(" "));
    // stdout first: that is where the conflict itself is reported, and a
    // reader of the message wants it before the "From <url>" fetch noise.
    let combined = format!("{}\n{}", stdout.trim(), stderr.trim());
    if is_conflict(&combined) {
        return Err(RemoteError::Conflict {
            command,
            output: combined.trim().to_string(),
        });
    }
    Err(RemoteError::Failed {
        command,
        stderr: stderr.trim().to_string(),
    })
}

/// Whether a failed `git pull`'s combined stdout+stderr says the folder and
/// the remote disagree — i.e. whether a forced pull is the way out.
///
/// The needles are Git's own wording, captured from a real `git` (2.x,
/// 2026-09-23) driving a local bare remote through each case, not guessed:
///
/// | case | Git says | stream |
/// |---|---|---|
/// | uncommitted edit to a file the merge touches | `error: Your local changes to the following files would be overwritten by merge:` … `Please commit your changes or stash them before you merge.` | stderr |
/// | untracked file the merge would write over | `error: The following untracked working tree files would be overwritten by merge:` | stderr |
/// | both sides committed, same lines | `CONFLICT (content): Merge conflict in <file>` / `Automatic merge failed; fix conflicts and then commit the result.` | **stdout** |
/// | pulled again, merge still unresolved | `error: Pulling is not possible because you have unmerged files.` | stderr |
/// | both sides committed, no `pull.rebase` set | `fatal: Need to specify how to reconcile divergent branches.` | stderr |
/// | `pull.rebase=true`, same lines | `error: could not apply <sha>…` / `Resolve all conflicts manually` | stdout |
///
/// The last one is included deliberately even though Git frames it as a
/// missing configuration: the folder *has* diverged, and the forced pull is
/// exactly what resolves it. Matching is case-insensitive; the needles are
/// substrings, so the file list and SHA that follow them do not matter.
fn is_conflict(output: &str) -> bool {
    const NEEDLES: [&str; 8] = [
        "would be overwritten by merge",
        "please commit your changes or stash them",
        "conflict (",
        "automatic merge failed",
        "you have unmerged files",
        "unresolved conflict",
        "need to specify how to reconcile divergent branches",
        "resolve all conflicts manually",
    ];
    let lowered = output.to_ascii_lowercase();
    NEEDLES.iter().any(|n| lowered.contains(n))
}

/// The forced pull behind the GUI's "yes, can" (GH issue #279): make `dir`
/// match `remote`/`branch` exactly, **destroying every local change**.
///
/// Concretely — abort whatever merge or rebase the failed pull left behind
/// ([`abort_in_progress_in`]), `git fetch <remote> <branch>`,
/// `git reset --hard FETCH_HEAD`, then `git clean -fd`.
///
/// # This throws work away
///
/// Everything not committed **and** pushed is gone afterwards, with no undo:
/// uncommitted edits, and — the maintainer's explicit choice, 2026-09-23 —
/// untracked files too, so a PDF or a note dropped into the folder and never
/// saved does not survive. Only ignored files (`git clean` without `-x`) and
/// submodule contents (without `-ff`) are left alone. Local *commits* that
/// were never pushed are discarded as well: `reset --hard` moves the branch
/// to the fetched tip, it does not merge onto it.
///
/// Never call this without the user having answered the prompt; the caller
/// that does is `crate::app::advanced_git_view`.
pub fn force_pull_in(
    dir: &std::path::Path,
    remote: &str,
    branch: &str,
) -> Result<String, RemoteError> {
    abort_in_progress_in(dir)?;
    run_git_in(dir, &["fetch", remote, branch])?;
    // FETCH_HEAD, not `<remote>/<branch>`: it is what the fetch just wrote,
    // so this works even where no remote-tracking ref exists (a folder set
    // up by `corpus_repos::clone`'s detached checkout, e.g.).
    run_git_in(dir, &["reset", "--hard", "FETCH_HEAD"])?;
    run_git_in(dir, &["clean", "-fd"])?;
    Ok(format!(
        "{dir} now matches {remote}/{branch} exactly",
        dir = dir.display()
    ))
}

/// Abort a merge or rebase a failed [`pull_in`] left in progress, putting
/// the folder back as it was before the pull — the GUI's "no, i manage
/// myself" (GH issue #279), and the first step of [`force_pull_in`].
///
/// `Ok(false)` means there was nothing in progress, which is the ordinary
/// case for a pull Git refused outright (it aborts by itself, leaving the
/// working tree untouched) and is not an error. `git merge --abort` with no
/// merge in flight exits 128 with "There is no merge to abort", so the
/// markers are checked first rather than running it and swallowing failures.
pub fn abort_in_progress_in(dir: &std::path::Path) -> Result<bool, RemoteError> {
    if git_path_exists(dir, "MERGE_HEAD") {
        run_git_in(dir, &["merge", "--abort"])?;
        return Ok(true);
    }
    if git_path_exists(dir, "rebase-merge") || git_path_exists(dir, "rebase-apply") {
        run_git_in(dir, &["rebase", "--abort"])?;
        return Ok(true);
    }
    Ok(false)
}

/// Whether `name` exists inside `dir`'s git directory, asked of Git itself
/// (`git rev-parse --git-path`) rather than assuming `dir/.git/<name>` —
/// which is wrong for a worktree or a submodule, where `.git` is a file
/// pointing elsewhere.
fn git_path_exists(dir: &std::path::Path, name: &str) -> bool {
    let Ok(path) = run_git_in(dir, &["rev-parse", "--git-path", name]) else {
        return false;
    };
    // `-C dir` makes the answer relative to `dir`, when it is relative.
    dir.join(path.trim()).exists()
}

/// [`push`] in the repository at `dir`.
pub fn push_in(dir: &std::path::Path, remote: &str, branch: &str) -> Result<String, RemoteError> {
    run_git_in(dir, &["push", remote, branch])
}

/// `git remote add <name> <url>` in the repository at `dir`.
pub fn add_remote_in(dir: &std::path::Path, name: &str, url: &str) -> Result<String, RemoteError> {
    run_git_in(dir, &["remote", "add", name, url])
}

/// The branch checked out in the repository at `dir`, if any (read with
/// `gix`, no subprocess). `None` for a detached head or no repository.
pub fn current_branch_in(dir: &std::path::Path) -> Option<String> {
    let repo = gix::open(dir).ok()?;
    let name = repo.head_name().ok().flatten()?;
    Some(name.shorten().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootConfig;
    use std::process::Command as StdCommand;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), true).unwrap();
        (dir, root)
    }

    #[test]
    fn local_branches_reports_the_current_branch() {
        let (_dir, root) = make_root();
        repository::save_repository(&root).unwrap();
        let branches = local_branches(&root).unwrap();
        assert_eq!(branches.len(), 1);
        assert!(branches[0].is_current);
    }

    #[test]
    fn history_reflects_a_real_commit() {
        let (_dir, root) = make_root();
        repository::save_repository(&root).unwrap();
        let commits = history(&root, 10).unwrap();
        assert_eq!(commits.len(), 1);
    }

    #[test]
    fn status_matches_repository_status() {
        let (_dir, root) = make_root();
        let a = status(&root).unwrap();
        let b = repository::status(&root).unwrap();
        assert_eq!(a, b);
    }

    /// End-to-end against a real *local* bare repo standing in for a
    /// remote — no network access, but real `git fetch`/`pull`/`push`
    /// subprocess invocations, exercising the actual command construction
    /// and error handling this module is responsible for.
    #[test]
    fn fetch_pull_push_work_against_a_local_bare_remote() {
        if !system_git_available() {
            eprintln!("system git not available; skipping");
            return;
        }
        let remote_dir = tempfile::tempdir().unwrap();
        let status = StdCommand::new("git")
            .args(["init", "--bare", "-b", "main"])
            .arg(remote_dir.path())
            .status()
            .unwrap();
        assert!(status.success());

        let (_dir, root) = make_root();
        // KovanRoot::create's gix::init doesn't name a branch; give the
        // local repo one so push has something to name.
        StdCommand::new("git")
            .args(["-C"])
            .arg(root.path())
            .args(["checkout", "-B", "main"])
            .status()
            .unwrap();
        repository::save_repository(&root).unwrap();

        StdCommand::new("git")
            .args(["-C"])
            .arg(root.path())
            .args(["remote", "add", "origin"])
            .arg(remote_dir.path())
            .status()
            .unwrap();

        let remotes = list_remotes(&root).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");

        push(&root, "origin", "main").unwrap();
        fetch(&root, "origin").unwrap();
    }

    // -----------------------------------------------------------------
    // GH issue #279 — a conflicted pull, and the forced pull out of it.
    // -----------------------------------------------------------------

    /// Real `git` in `dir`, asserting success; identity is passed per
    /// invocation so the test does not depend on a global `user.email`.
    fn git(dir: &std::path::Path, args: &[&str]) -> String {
        let out = StdCommand::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn write(dir: &std::path::Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).unwrap();
    }

    /// A bare "remote" whose `main` holds `f.txt = theirs`, plus a clone
    /// left one commit behind on `f.txt = base`.
    fn diverging_pair() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote.git");
        let theirs = dir.path().join("theirs");
        let ours = dir.path().join("ours");

        assert!(StdCommand::new("git")
            .args(["init", "-q", "--bare", "-b", "main"])
            .arg(&remote)
            .status()
            .unwrap()
            .success());
        assert!(StdCommand::new("git")
            .args(["clone", "-q"])
            .arg(&remote)
            .arg(&theirs)
            .status()
            .unwrap()
            .success());
        write(&theirs, "f.txt", "base\n");
        git(&theirs, &["add", "."]);
        git(&theirs, &["commit", "-qm", "base"]);
        git(&theirs, &["push", "-q", "origin", "main"]);

        assert!(StdCommand::new("git")
            .args(["clone", "-q"])
            .arg(&remote)
            .arg(&ours)
            .status()
            .unwrap()
            .success());

        write(&theirs, "f.txt", "theirs\n");
        git(&theirs, &["commit", "-qam", "theirs"]);
        git(&theirs, &["push", "-q", "origin", "main"]);

        (dir, remote, ours)
    }

    /// The needles in [`is_conflict`] against Git's real wording (captured
    /// 2026-09-23), and against failures that must *not* offer a forced
    /// pull — a wrong URL or a missing branch is not something the user can
    /// answer "yes, can" to.
    #[test]
    fn a_conflict_is_told_apart_from_an_ordinary_pull_failure() {
        for conflicted in [
            "error: Your local changes to the following files would be overwritten by merge:\n\tf.txt\nPlease commit your changes or stash them before you merge.\nAborting",
            "error: The following untracked working tree files would be overwritten by merge:\n\tf.txt",
            "Auto-merging f.txt\nCONFLICT (content): Merge conflict in f.txt\nAutomatic merge failed; fix conflicts and then commit the result.",
            "error: Pulling is not possible because you have unmerged files.\nfatal: Exiting because of an unresolved conflict.",
            "hint: You have divergent branches and need to specify how to reconcile them.\nfatal: Need to specify how to reconcile divergent branches.",
            "error: could not apply 1467fce... mine\nResolve all conflicts manually, mark them as resolved with git add",
        ] {
            assert!(is_conflict(conflicted), "should be a conflict: {conflicted}");
        }
        for ordinary in [
            "fatal: repository 'https://example.com/nope.git' not found",
            "fatal: couldn't find remote ref no-such-branch",
            "fatal: Authentication failed for 'https://example.com/'",
            "Already up to date.",
            "",
        ] {
            assert!(
                !is_conflict(ordinary),
                "should not be a conflict: {ordinary}"
            );
        }
    }

    /// An uncommitted edit Git refuses to overwrite comes back as
    /// [`RemoteError::Conflict`], carrying Git's own words — the case the
    /// prompt's "you may have unsaved changes" is about.
    #[test]
    fn an_uncommitted_edit_makes_pull_report_a_conflict() {
        if !system_git_available() {
            eprintln!("system git not available; skipping");
            return;
        }
        let (_dir, _remote, ours) = diverging_pair();
        write(&ours, "f.txt", "mine\n");

        match pull_in(&ours, "origin", "main") {
            Err(RemoteError::Conflict { output, .. }) => {
                assert!(
                    output.contains("would be overwritten by merge"),
                    "conflict message lost Git's reason: {output}"
                );
            }
            other => panic!("expected a conflict, got {other:?}"),
        }
        // Git aborted by itself: nothing to abort, nothing changed.
        assert!(!abort_in_progress_in(&ours).unwrap());
        assert_eq!(
            std::fs::read_to_string(ours.join("f.txt")).unwrap(),
            "mine\n"
        );
    }

    /// Two sides that both committed the same line: the pull leaves a merge
    /// in progress (or is refused outright, depending on the machine's
    /// `pull.rebase`), and either way it is a conflict. "no, i manage
    /// myself" then puts the folder back exactly as it was before Pull.
    #[test]
    fn declining_the_prompt_restores_the_folder_to_its_pre_pull_state() {
        if !system_git_available() {
            eprintln!("system git not available; skipping");
            return;
        }
        let (_dir, _remote, ours) = diverging_pair();
        write(&ours, "f.txt", "mine\n");
        git(&ours, &["commit", "-qam", "mine"]);
        let before = git(&ours, &["rev-parse", "HEAD"]);

        // Force the merge strategy so the in-progress state is reached on
        // any machine, whatever `pull.rebase` is set to there.
        let merged = StdCommand::new("git")
            .arg("-C")
            .arg(&ours)
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "-c",
                "pull.rebase=false",
                "pull",
                "origin",
                "main",
            ])
            .output()
            .unwrap();
        assert!(!merged.status.success());
        assert!(is_conflict(&format!(
            "{}{}",
            String::from_utf8_lossy(&merged.stdout),
            String::from_utf8_lossy(&merged.stderr)
        )));
        assert!(ours.join(".git/MERGE_HEAD").exists());

        assert!(abort_in_progress_in(&ours).unwrap());
        assert_eq!(git(&ours, &["rev-parse", "HEAD"]), before);
        assert_eq!(
            std::fs::read_to_string(ours.join("f.txt")).unwrap(),
            "mine\n"
        );
        assert!(git(&ours, &["status", "--porcelain"]).trim().is_empty());
        assert!(!abort_in_progress_in(&ours).unwrap());
    }

    /// "yes, can": the folder ends up an exact mirror of the remote — the
    /// local commit is gone, the uncommitted edit is gone, and (the
    /// maintainer's choice) the untracked file is gone too.
    #[test]
    fn the_forced_pull_makes_the_folder_match_the_remote_exactly() {
        if !system_git_available() {
            eprintln!("system git not available; skipping");
            return;
        }
        let (_dir, _remote, ours) = diverging_pair();
        write(&ours, "f.txt", "mine\n");
        git(&ours, &["commit", "-qam", "mine"]);
        write(&ours, "f.txt", "mine, edited again\n");
        write(&ours, "scratch.md", "never saved\n");
        std::fs::create_dir(ours.join("notes")).unwrap();
        write(&ours, "notes/a.md", "also never saved\n");

        assert!(matches!(
            pull_in(&ours, "origin", "main"),
            Err(RemoteError::Conflict { .. })
        ));

        force_pull_in(&ours, "origin", "main").unwrap();

        assert_eq!(
            std::fs::read_to_string(ours.join("f.txt")).unwrap(),
            "theirs\n"
        );
        assert!(!ours.join("scratch.md").exists());
        assert!(!ours.join("notes").exists());
        assert!(git(&ours, &["status", "--porcelain"]).trim().is_empty());
        // And it is the remote's commit, not a merge of the two.
        let log = git(&ours, &["log", "--oneline"]);
        assert!(log.contains("theirs"), "{log}");
        assert!(
            !log.contains("mine"),
            "the discarded commit survived: {log}"
        );

        // Idempotent: a second forced pull on an already-matching folder is
        // a no-op, not an error.
        force_pull_in(&ours, "origin", "main").unwrap();
    }

    #[test]
    fn remote_operations_report_git_unavailable_cleanly_when_configured_to_look_for_a_missing_binary(
    ) {
        // `run_git` itself always probes the real `git`; this test instead
        // confirms `system_git_available` is a plain, panic-free bool
        // check callers can gate on (§38's "remains fully functional
        // without system Git").
        let _ = system_git_available();
    }
}
