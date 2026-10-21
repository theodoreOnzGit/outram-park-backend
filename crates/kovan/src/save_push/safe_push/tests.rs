//! GH issue #502: a bare remote and two clones, "ours" (Kovan's) and
//! "theirs" (another writer), reproducing a push rejected because the
//! remote moved on. Skipped when no system `git` is available, as every
//! git-using test in this crate is.

use super::*;
use std::process::Command;

/// `git` in `dir` with a test identity; panics with Git's stderr on failure.
fn g(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct Pair {
    _tmp: tempfile::TempDir,
    bare: std::path::PathBuf,
    ours: std::path::PathBuf,
    theirs: std::path::PathBuf,
}

/// A bare remote whose `branch` holds `shared.md`, cloned twice. Both
/// clones get a local identity so the merge in `ours` can commit.
fn pair(branch: &str) -> Option<Pair> {
    if !crate::advanced_git::system_git_available() {
        eprintln!("system git not available; skipping");
        return None;
    }
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let seed = t.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    std::fs::write(seed.join("shared.md"), "seed\n").unwrap();
    g(&seed, &["init", "-q", "-b", branch]);
    g(&seed, &["add", "."]);
    g(&seed, &["commit", "-q", "-m", "seed"]);
    let bare = t.join("remote.git");
    g(
        t,
        &[
            "clone",
            "-q",
            "--bare",
            &seed.to_string_lossy(),
            &bare.to_string_lossy(),
        ],
    );
    let (ours, theirs) = (t.join("ours"), t.join("theirs"));
    for c in [&ours, &theirs] {
        g(
            t,
            &["clone", "-q", &bare.to_string_lossy(), &c.to_string_lossy()],
        );
        g(c, &["config", "user.name", "t"]);
        g(c, &["config", "user.email", "t@t"]);
    }
    Some(Pair {
        _tmp: tmp,
        bare,
        ours,
        theirs,
    })
}

fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> String {
    std::fs::write(dir.join(file), text).unwrap();
    g(dir, &["add", "."]);
    g(dir, &["commit", "-q", "-m", msg]);
    g(dir, &["rev-parse", "HEAD"])
}

fn tip(bare: &Path, branch: &str) -> String {
    g(bare, &["rev-parse", &format!("refs/heads/{branch}")])
}

fn is_ancestor(dir: &Path, a: &str, b: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["merge-base", "--is-ancestor", a, b])
        .status()
        .unwrap()
        .success()
}

/// No reset that moves `HEAD` appears in the clone's reflog. `git merge
/// --abort` is `reset --merge` internally and logs `reset: moving to HEAD`,
/// which does not move `HEAD`; any other target (#502's was `FETCH_HEAD`)
/// fails.
fn assert_never_reset(dir: &Path) {
    let reflog = g(dir, &["reflog", "--format=%gs"]);
    assert!(
        !reflog
            .lines()
            .any(|l| l.starts_with("reset:") && l != "reset: moving to HEAD"),
        "the clone was reset:\n{reflog}"
    );
}

/// (1) The #502 case with different files: the remote moved on, the
/// rejected push is followed by a merge and a second push, the remote ends
/// up with both commits, and the local save is an ancestor of what was
/// pushed (so a gitlink to it stays fetchable).
#[test]
fn a_moved_remote_is_merged_and_pushed_keeping_the_local_commit() {
    let Some(p) = pair("main") else { return };
    let theirs = commit(&p.theirs, "theirs.pdf", "theirs\n", "theirs");
    g(&p.theirs, &["push", "-q", "origin", "main"]);
    let mine = commit(&p.ours, "mine.pdf", "mine\n", "mine");

    let ok = push_keeping_local(&p.ours, "origin", "main", BranchRule::AnyBranch).unwrap();
    assert_eq!(
        ok,
        SafePushOk::Pushed {
            merged: Some(theirs.clone())
        }
    );
    let pushed = tip(&p.bare, "main");
    assert_eq!(pushed, g(&p.ours, &["rev-parse", "HEAD"]));
    assert!(is_ancestor(&p.bare, &mine, &pushed), "local save dropped");
    assert!(is_ancestor(&p.bare, &theirs, &pushed), "remote overwritten");
    assert!(p.ours.join("mine.pdf").exists() && p.ours.join("theirs.pdf").exists());
    assert_never_reset(&p.ours);

    // Nothing left to push: up to date.
    assert_eq!(
        push_keeping_local(&p.ours, "origin", "main", BranchRule::AnyBranch).unwrap(),
        SafePushOk::UpToDate
    );
}

/// (2) The remote moved on with a conflicting edit to the same file: a
/// typed [`SafePushError::Diverged`], the local commit still `HEAD`, a
/// clean working tree with no merge in progress, no reset, and the remote
/// untouched.
#[test]
fn a_conflicting_remote_is_a_typed_error_and_the_local_commit_stays() {
    let Some(p) = pair("main") else { return };
    let theirs = commit(&p.theirs, "shared.md", "theirs\n", "theirs");
    g(&p.theirs, &["push", "-q", "origin", "main"]);
    let mine = commit(&p.ours, "shared.md", "mine\n", "mine");

    let err = push_keeping_local(&p.ours, "origin", "main", BranchRule::AnyBranch).unwrap_err();
    match &err {
        SafePushError::Diverged {
            branch,
            local,
            remote,
            ..
        } => {
            assert_eq!(branch, "main");
            assert_eq!(local, &mine);
            assert_eq!(remote, &theirs);
        }
        other => panic!("expected Diverged, got {other:?}"),
    }
    let shown = err.to_string();
    assert!(shown.contains("committed locally"), "{shown}");
    assert!(shown.contains("lazygit"), "{shown}");

    assert_eq!(g(&p.ours, &["rev-parse", "HEAD"]), mine);
    assert_eq!(g(&p.ours, &["status", "--porcelain"]), "");
    let merge_in_progress = Command::new("git")
        .arg("-C")
        .arg(&p.ours)
        .args(["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .status()
        .unwrap()
        .success();
    assert!(!merge_in_progress, "the conflicted merge was left in place");
    assert_eq!(
        std::fs::read_to_string(p.ours.join("shared.md")).unwrap(),
        "mine\n"
    );
    assert_eq!(tip(&p.bare, "main"), theirs);
    assert_never_reset(&p.ours);
}

/// Merge is never attempted over unsaved files: a typed error, nothing
/// changed.
#[test]
fn a_moved_remote_with_unsaved_files_is_not_merged() {
    let Some(p) = pair("main") else { return };
    commit(&p.theirs, "theirs.pdf", "theirs\n", "theirs");
    g(&p.theirs, &["push", "-q", "origin", "main"]);
    let mine = commit(&p.ours, "mine.pdf", "mine\n", "mine");
    std::fs::write(p.ours.join("draft.md"), "unsaved\n").unwrap();

    let err = push_keeping_local(&p.ours, "origin", "main", BranchRule::AnyBranch).unwrap_err();
    assert_eq!(
        err,
        SafePushError::UncommittedChanges {
            branch: "main".into(),
            local: mine.clone()
        }
    );
    assert!(err.to_string().contains("committed locally"));
    assert_eq!(g(&p.ours, &["rev-parse", "HEAD"]), mine);
    assert!(p.ours.join("draft.md").exists());
    assert_never_reset(&p.ours);
}

/// (3) [`BranchRule::NeverMain`] refuses `main` before anything runs, and
/// still pushes `develop`.
#[test]
fn never_main_refuses_main_and_pushes_develop() {
    let Some(p) = pair("main") else { return };
    let before = tip(&p.bare, "main");
    commit(&p.ours, "review.md", "reviewed\n", "review");

    let err = push_keeping_local(&p.ours, "origin", "main", BranchRule::NeverMain).unwrap_err();
    assert_eq!(
        err,
        SafePushError::MainRefused {
            branch: "main".into()
        }
    );
    assert!(err.to_string().contains("never `main`"));
    assert_eq!(tip(&p.bare, "main"), before, "main was pushed");

    g(&p.ours, &["checkout", "-q", "-b", "develop"]);
    assert_eq!(
        push_keeping_local(&p.ours, "origin", "develop", BranchRule::NeverMain).unwrap(),
        SafePushOk::Pushed { merged: None }
    );
    assert_eq!(tip(&p.bare, "develop"), g(&p.ours, &["rev-parse", "HEAD"]));
}

/// Pushing a branch other than the checked-out one is refused rather than
/// merging into the wrong branch.
#[test]
fn a_branch_that_is_not_checked_out_is_refused() {
    let Some(p) = pair("main") else { return };
    let err = push_keeping_local(&p.ours, "origin", "develop", BranchRule::NeverMain).unwrap_err();
    assert_eq!(
        err,
        SafePushError::NotOnBranch {
            branch: "develop".into()
        }
    );
    assert!(err.to_string().contains("not on `develop`"));
}

/// A failure that is not "the remote moved on" (here, a remote that does
/// not exist) is [`SafePushError::Git`], with no fetch or merge attempted.
#[test]
fn any_other_push_failure_is_a_git_error() {
    let Some(p) = pair("main") else { return };
    g(
        &p.ours,
        &["remote", "set-url", "origin", "/nonexistent/remote.git"],
    );
    let err = push_keeping_local(&p.ours, "origin", "main", BranchRule::AnyBranch).unwrap_err();
    assert!(matches!(err, SafePushError::Git { .. }), "{err:?}");
    assert!(err.to_string().starts_with("Git said:"));
}
