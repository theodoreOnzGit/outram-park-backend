//! Push-after-save tests: real repositories in a temp directory, with local
//! **bare** repositories as the remotes (a push into a non-bare checkout's
//! current branch is refused by Git, so the stand-ins must be bare).

use super::*;
use crate::repository::save_repository_with_message;
use crate::root::{CorporaConfig, RootConfig};

/// `git` in `dir` with a test identity; panics with Git's stderr on failure.
fn g(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "protocol.file.allow=always"])
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

/// A bare repository at `tmp/<name>.git` whose `main` holds `file`.
fn bare_remote(tmp: &Path, name: &str, file: &str) -> String {
    let work = tmp.join(format!("{name}-seed"));
    let f = work.join(file);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, b"seed").unwrap();
    g(&work, &["init", "-q", "-b", "main"]);
    g(&work, &["add", "."]);
    g(&work, &["commit", "-q", "-m", "seed"]);
    let bare = tmp.join(format!("{name}.git"));
    g(
        tmp,
        &[
            "clone",
            "-q",
            "--bare",
            &work.to_string_lossy(),
            &bare.to_string_lossy(),
        ],
    );
    bare.to_string_lossy().to_string()
}

/// The commit `branch` points at in the (bare) repository at `repo`.
fn tip(repo: &str, branch: &str) -> String {
    g(
        Path::new(repo),
        &["rev-parse", &format!("refs/heads/{branch}")],
    )
}

fn head(dir: &Path) -> String {
    g(dir, &["rev-parse", "HEAD"])
}

struct Fixture {
    _tmp: tempfile::TempDir,
    root: KovanRoot,
    proprietary: String,
    open: String,
    standard: String,
    parent: String,
}

/// A Kovan folder whose three corpora are submodules of bare remotes (as
/// setup makes it), with its own bare `origin`, after one save and push.
fn fixture() -> Option<Fixture> {
    if !crate::advanced_git::system_git_available() {
        return None;
    }
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let standard = bare_remote(t, "standard", "kovan-standard-open-corpus/a.pdf");
    let open = bare_remote(t, "open", "me-open-corpus/b.pdf");
    let proprietary = bare_remote(t, "proprietary", "papers/c.pdf");
    let parent = t.join("parent.git").to_string_lossy().to_string();
    g(t, &["init", "-q", "--bare", "-b", "main", &parent]);

    let config = RootConfig::new("lib", "Lib").with_private_submodule(proprietary.clone());
    let mut root = KovanRoot::create(&t.join("lib"), config, true).unwrap();
    root.set_corpora(CorporaConfig {
        open_remote: Some(open.clone()),
        proprietary_remote: Some(proprietary.clone()),
    })
    .unwrap();
    let setup = crate::corpus_repos::ensure_library_corpora_with(&root, &standard, "main");
    assert!(setup.standard.is_ok() && setup.open.is_ok() && setup.proprietary.is_ok());
    g(root.path(), &["remote", "add", "origin", &parent]);
    save_repository_with_message(&root, "").unwrap();
    let first = push_after_save(&root);
    assert!(!first.has_problem(), "{:?}", first.lines());
    Some(Fixture {
        _tmp: tmp,
        root,
        proprietary,
        open,
        standard,
        parent,
    })
}

fn parent_branch(root: &KovanRoot) -> String {
    g(root.path(), &["symbolic-ref", "--short", "HEAD"])
}

/// The gitlink the Kovan repository's `rev` records at `rel`.
fn gitlink(repo: &Path, rev: &str, rel: &str) -> String {
    let line = g(repo, &["ls-tree", rev, "--", rel]);
    line.split_whitespace()
        .nth(2)
        .unwrap_or_default()
        .to_string()
}

/// A save then a push lands every new commit on its own remote, corpora
/// first and the Kovan repository last, and the pushed parent's gitlinks
/// point at commits its corpus remotes have.
#[test]
fn save_then_push_lands_on_every_remote_corpora_first() {
    let Some(f) = fixture() else { return };
    let prop_dir = f.root.restricted_sources_dir();
    let open_dir = f.root.open_corpus_dir();
    std::fs::write(prop_dir.join("papers/new.pdf"), b"restricted").unwrap();
    std::fs::write(open_dir.join("me-open-corpus/new.pdf"), b"open").unwrap();
    save_repository_with_message(&f.root, "two papers")
        .unwrap()
        .unwrap();

    let report = push_after_save(&f.root);
    let order: Vec<PushRepo> = report.repos.iter().map(|r| r.repo).collect();
    assert_eq!(
        order,
        [
            PushRepo::ProprietaryCorpus,
            PushRepo::OpenCorpus,
            PushRepo::KovanRepository
        ]
    );
    for r in &report.repos {
        assert!(
            matches!(
                r.outcome,
                PushOutcome::Pushed {
                    attached: false,
                    ..
                }
            ),
            "{}",
            r.line()
        );
    }
    assert_eq!(tip(&f.proprietary, "main"), head(&prop_dir));
    assert_eq!(tip(&f.open, "main"), head(&open_dir));
    let branch = parent_branch(&f.root);
    assert_eq!(tip(&f.parent, &branch), head(f.root.path()));

    // The published parent records exactly the published corpus commits.
    let parent = Path::new(&f.parent);
    let rev = format!("refs/heads/{branch}");
    assert_eq!(
        gitlink(parent, &rev, "literature/proprietary"),
        tip(&f.proprietary, "main")
    );
    assert_eq!(
        gitlink(parent, &rev, "literature/open-corpus"),
        tip(&f.open, "main")
    );

    // The proprietary PDF went to the proprietary remote and nowhere else.
    let listed = |repo: &str| g(Path::new(repo), &["ls-tree", "-r", "--name-only", "main"]);
    assert!(listed(&f.proprietary).contains("papers/new.pdf"));
    assert!(!listed(&f.open).contains("papers/new.pdf"));
    assert!(!g(parent, &["ls-tree", "-r", "--name-only", &rev]).contains("new.pdf"));

    // Pushing again finds nothing to push.
    let again = push_after_save(&f.root);
    for r in &again.repos {
        assert!(
            matches!(r.outcome, PushOutcome::UpToDate { .. }),
            "{}",
            r.line()
        );
    }
}

/// With push-after-save turned off, a save commits and pushes nothing.
#[test]
fn opting_out_pushes_nothing() {
    let Some(mut f) = fixture() else { return };
    f.root.set_push_after_save(false).unwrap();
    let before = (tip(&f.proprietary, "main"), tip(&f.open, "main"));
    let branch = parent_branch(&f.root);
    let parent_before = tip(&f.parent, &branch);

    let prop_dir = f.root.restricted_sources_dir();
    std::fs::write(prop_dir.join("papers/new.pdf"), b"restricted").unwrap();
    let (saved, pushed) = crate::advanced_git::save_and_push(
        &f.root,
        "",
        crate::advanced_git::push_after_save_setting(&f.root),
    );
    assert!(saved.unwrap().is_some());
    assert!(pushed.is_none());
    assert_eq!((tip(&f.proprietary, "main"), tip(&f.open, "main")), before);
    assert_eq!(tip(&f.parent, &branch), parent_before);
    assert_ne!(head(&prop_dir), before.0, "the save itself still committed");
}

/// A remote that has moved on is never overwritten: the push fails with
/// "pull first", the remote keeps its commit, the local save is kept, and
/// the Kovan repository is not pushed.
#[test]
fn a_diverged_remote_is_not_forced_and_says_pull_first() {
    let Some(f) = fixture() else { return };
    let t = f._tmp.path();
    let other = t.join("other-clone");
    g(
        t,
        &["clone", "-q", &f.proprietary, &other.to_string_lossy()],
    );
    std::fs::write(other.join("papers/theirs.pdf"), b"theirs").unwrap();
    g(&other, &["add", "."]);
    g(&other, &["commit", "-q", "-m", "theirs"]);
    g(&other, &["push", "-q", "origin", "main"]);
    let theirs = head(&other);
    let branch = parent_branch(&f.root);
    let parent_before = tip(&f.parent, &branch);

    let prop_dir = f.root.restricted_sources_dir();
    std::fs::write(prop_dir.join("papers/mine.pdf"), b"mine").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    let mine = head(&prop_dir);

    let report = push_after_save(&f.root);
    match report.get(PushRepo::ProprietaryCorpus).unwrap() {
        PushOutcome::Failed { message } => assert!(message.contains("pull first"), "{message}"),
        other => panic!("expected a failed push, got {other:?}"),
    }
    assert_eq!(
        tip(&f.proprietary, "main"),
        theirs,
        "the remote was overwritten"
    );
    assert_eq!(head(&prop_dir), mine, "the local save was lost");
    assert!(matches!(
        report.get(PushRepo::KovanRepository).unwrap(),
        PushOutcome::Skipped { .. }
    ));
    assert_eq!(tip(&f.parent, &branch), parent_before);
}

/// The maintainer's real case: a fresh clone of the Kovan repository whose
/// submodules `git submodule update` left on a detached HEAD. A save commits
/// onto it; the push puts that commit on `main` (a fast-forward) and pushes
/// it, and the submodule is on `main` afterwards.
#[test]
fn a_detached_submodule_save_is_put_on_its_branch_and_pushed() {
    let Some(f) = fixture() else { return };
    let t = f._tmp.path();
    let clone = t.join("lib-clone");
    let branch = parent_branch(&f.root);
    g(
        t,
        &[
            "clone",
            "-q",
            "-b",
            &branch,
            &f.parent,
            &clone.to_string_lossy(),
        ],
    );
    g(
        &clone,
        &[
            "submodule",
            "update",
            "--init",
            "--",
            "literature/proprietary",
        ],
    );
    let prop_dir = clone.join("literature/proprietary");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&prop_dir)
            .args(["symbolic-ref", "-q", "HEAD"])
            .status()
            .map(|s| !s.success())
            .unwrap(),
        "fixture: the submodule should start detached"
    );

    let root = KovanRoot::open(&clone).unwrap();
    std::fs::write(prop_dir.join("papers/on-detached.pdf"), b"d").unwrap();
    save_repository_with_message(&root, "").unwrap().unwrap();
    let saved = head(&prop_dir);

    let report = push_after_save(&root);
    match report.get(PushRepo::ProprietaryCorpus).unwrap() {
        PushOutcome::Pushed {
            attached: true,
            branch,
            ..
        } => assert_eq!(branch, "main"),
        other => panic!("expected an attached push, got {other:?}"),
    }
    assert_eq!(tip(&f.proprietary, "main"), saved);
    assert_eq!(g(&prop_dir, &["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(head(&prop_dir), saved, "attaching must not move HEAD");
}

/// A detached save whose branch has moved on locally is not a
/// fast-forward: refused, with nothing moved and nothing pushed.
#[test]
fn a_detached_head_that_would_not_fast_forward_is_refused() {
    let Some(f) = fixture() else { return };
    let prop_dir = f.root.restricted_sources_dir();
    // `main` gets a commit the detached HEAD does not have.
    g(&prop_dir, &["checkout", "-q", "--detach"]);
    let detached_at = head(&prop_dir);
    g(&prop_dir, &["checkout", "-q", "main"]);
    std::fs::write(prop_dir.join("papers/on-main.pdf"), b"m").unwrap();
    g(&prop_dir, &["add", "."]);
    g(&prop_dir, &["commit", "-q", "-m", "on main"]);
    let main_tip = head(&prop_dir);
    g(&prop_dir, &["checkout", "-q", &detached_at]);
    std::fs::remove_file(prop_dir.join("papers/on-main.pdf")).ok();
    std::fs::write(prop_dir.join("papers/on-detached.pdf"), b"d").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    let remote_before = tip(&f.proprietary, "main");

    let report = push_after_save(&f.root);
    assert!(
        matches!(
            report.get(PushRepo::ProprietaryCorpus).unwrap(),
            PushOutcome::Refused { .. }
        ),
        "{:?}",
        report.lines()
    );
    assert_eq!(g(&prop_dir, &["rev-parse", "refs/heads/main"]), main_tip);
    assert_eq!(tip(&f.proprietary, "main"), remote_before);
}

/// The proprietary corpus's `origin` pointed at the open corpus's remote:
/// refused, and the open remote does not receive the proprietary commit.
#[test]
fn a_proprietary_origin_that_is_not_the_private_remote_is_refused() {
    let Some(f) = fixture() else { return };
    let prop_dir = f.root.restricted_sources_dir();
    g(&prop_dir, &["remote", "set-url", "origin", &f.open]);
    std::fs::write(prop_dir.join("papers/secret.pdf"), b"secret").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    let open_before = tip(&f.open, "main");

    let report = push_after_save(&f.root);
    match report.get(PushRepo::ProprietaryCorpus).unwrap() {
        PushOutcome::Refused { reason } => assert!(reason.contains("proprietary"), "{reason}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(tip(&f.open, "main"), open_before);
    let listed = g(
        Path::new(&f.open),
        &["log", "--all", "--name-only", "--format="],
    );
    assert!(!listed.contains("secret.pdf"));
    assert!(matches!(
        report.get(PushRepo::KovanRepository).unwrap(),
        PushOutcome::Skipped { .. }
    ));

    // A separate `pushurl` is checked too, not only the fetch URL.
    g(&prop_dir, &["remote", "set-url", "origin", &f.proprietary]);
    g(&prop_dir, &["config", "remote.origin.pushurl", &f.open]);
    let report = push_after_save(&f.root);
    assert!(matches!(
        report.get(PushRepo::ProprietaryCorpus).unwrap(),
        PushOutcome::Refused { .. }
    ));
}

/// The strict check works the other way too: the open corpus may only push
/// to the open remote.
#[test]
fn an_open_corpus_origin_that_is_not_the_open_remote_is_refused() {
    let Some(f) = fixture() else { return };
    let open_dir = f.root.open_corpus_dir();
    g(&open_dir, &["remote", "set-url", "origin", &f.proprietary]);
    std::fs::write(open_dir.join("me-open-corpus/x.pdf"), b"x").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    let report = push_after_save(&f.root);
    assert!(matches!(
        report.get(PushRepo::OpenCorpus).unwrap(),
        PushOutcome::Refused { .. }
    ));
}

/// A `kovan_root.toml` that names the same repository as both the open and
/// the proprietary remote is refused for both corpora.
#[test]
fn the_same_remote_for_open_and_proprietary_is_refused() {
    let Some(mut f) = fixture() else { return };
    f.root
        .set_corpora(CorporaConfig {
            open_remote: Some(f.proprietary.clone()),
            proprietary_remote: Some(f.proprietary.clone()),
        })
        .unwrap();
    let report = push_after_save(&f.root);
    assert!(matches!(
        report.get(PushRepo::ProprietaryCorpus).unwrap(),
        PushOutcome::Refused { .. }
    ));
    assert!(matches!(
        report.get(PushRepo::OpenCorpus).unwrap(),
        PushOutcome::Refused { .. }
    ));
}

/// An unreachable remote (as good as a refused credential for this code
/// path) is reported plainly with Git's words, never as success.
#[test]
fn an_unreachable_remote_is_reported_plainly() {
    let Some(f) = fixture() else { return };
    let prop_dir = f.root.restricted_sources_dir();
    std::fs::write(prop_dir.join("papers/new.pdf"), b"n").unwrap();
    save_repository_with_message(&f.root, "").unwrap().unwrap();
    std::fs::remove_dir_all(&f.proprietary).unwrap();
    let report = push_after_save(&f.root);
    match report.get(PushRepo::ProprietaryCorpus).unwrap() {
        PushOutcome::Failed { message } => assert!(message.contains("Git said"), "{message}"),
        other => panic!("expected a failure, got {other:?}"),
    }
}

/// The standard corpus is never pushed, even though it is a submodule.
#[test]
fn the_standard_corpus_is_never_pushed() {
    let Some(f) = fixture() else { return };
    let report = push_after_save(&f.root);
    assert_eq!(report.repos.len(), 3);
    assert!(report
        .repos
        .iter()
        .all(|r| !same_dir(&r.dir, &f.root.standard_corpus_dir())));
}

#[test]
fn url_spellings_of_one_repository_compare_equal() {
    let a = normalize_url("https://github.com/Org/Repo.git");
    assert_eq!(a, normalize_url("git@github.com:Org/Repo.git"));
    assert_eq!(a, normalize_url("ssh://git@GitHub.com/Org/Repo"));
    assert_eq!(a, normalize_url("https://github.com/Org/Repo/"));
    assert_ne!(a, normalize_url("https://github.com/Org/Other.git"));
    assert_ne!(
        a,
        normalize_url("https://github.com/org/repo.git"),
        "paths stay case-sensitive"
    );
    assert_eq!(normalize_url("/tmp/x/p.git"), normalize_url("/tmp/x/p"));
}

/// Another clone pushes a commit to `remote`'s `main` (as a second machine
/// saving into the corpus would). Returns the new tip.
fn advance_remote(tmp: &Path, remote: &str, file: &str) -> String {
    let other = tmp.join(format!("other-{}", file.replace('/', "_")));
    g(tmp, &["clone", "-q", remote, &other.to_string_lossy()]);
    let f = other.join(file);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, b"from another clone").unwrap();
    g(&other, &["add", "."]);
    g(&other, &["commit", "-q", "-m", "saved elsewhere"]);
    g(&other, &["push", "-q", "origin", "main"]);
    tip(remote, "main")
}

/// GH issue #422: a corpus another clone saved into is behind its remote;
/// pulling the corpora fast-forwards it, leaves it on `main` (not detached),
/// and the next save's push is no longer refused. Before #422 this exact
/// state made push-after-save refuse the corpus and skip the Kovan
/// repository (seen 2026-09-29: open corpus 7 behind, proprietary 16).
#[test]
fn pulling_the_corpora_fast_forwards_a_corpus_that_fell_behind() {
    let Some(f) = fixture() else { return };
    let t = f._tmp.path();
    let open_dir = f.root.open_corpus_dir();
    let prop_dir = f.root.restricted_sources_dir();
    // Leave both detached, as `git submodule update` would.
    g(&open_dir, &["checkout", "-q", "--detach"]);
    g(&prop_dir, &["checkout", "-q", "--detach"]);
    let open_before = head(&open_dir);
    let open_tip = advance_remote(t, &f.open, "me-open-corpus/elsewhere.pdf");
    let prop_tip = advance_remote(t, &f.proprietary, "papers/elsewhere.pdf");

    let pulled = pull_corpora(&f.root);
    let lines: Vec<String> = pulled.iter().map(CorpusPull::line).collect();
    let get = |k: CorpusKind| &pulled.iter().find(|p| p.corpus == k).unwrap().outcome;
    assert_eq!(
        get(CorpusKind::Open),
        &CorpusPullOutcome::Updated {
            branch: "main".into(),
            from: open_before,
            to: open_tip.clone(),
        },
        "{lines:?}"
    );
    assert!(
        matches!(
            get(CorpusKind::Proprietary),
            CorpusPullOutcome::Updated { .. }
        ),
        "{lines:?}"
    );
    assert!(
        matches!(
            get(CorpusKind::Standard),
            CorpusPullOutcome::UpToDate { .. }
        ),
        "{lines:?}"
    );
    assert_eq!(head(&open_dir), open_tip);
    assert_eq!(head(&prop_dir), prop_tip);
    for dir in [&open_dir, &prop_dir] {
        assert_eq!(g(dir, &["symbolic-ref", "--short", "HEAD"]), "main");
    }

    // A new save now pushes everywhere instead of being refused.
    std::fs::write(prop_dir.join("papers/after.pdf"), b"new").unwrap();
    save_repository_with_message(&f.root, "after pull")
        .unwrap()
        .unwrap();
    let report = push_after_save(&f.root);
    assert!(!report.has_problem(), "{:?}", report.lines());
    assert_eq!(tip(&f.proprietary, "main"), head(&prop_dir));

    // Pulling again finds everything up to date.
    for p in pull_corpora(&f.root) {
        assert!(
            matches!(p.outcome, CorpusPullOutcome::UpToDate { .. }),
            "{}",
            p.line()
        );
    }
}

/// Following the remote would destroy local work: a corpus with an unpushed
/// save, or with an unsaved file, is reported as needing confirmation and
/// left exactly as it was — overriding is the user's call, never automatic.
#[test]
fn a_corpus_with_local_work_is_not_overridden_without_asking() {
    let Some(f) = fixture() else { return };
    let t = f._tmp.path();
    let prop_dir = f.root.restricted_sources_dir();
    let open_dir = f.root.open_corpus_dir();
    advance_remote(t, &f.proprietary, "papers/elsewhere.pdf");
    advance_remote(t, &f.open, "me-open-corpus/elsewhere.pdf");

    // Proprietary: a local save the remote does not have.
    std::fs::write(prop_dir.join("papers/local.pdf"), b"only here").unwrap();
    g(&prop_dir, &["add", "."]);
    g(&prop_dir, &["commit", "-q", "-m", "local only"]);
    let prop_head = head(&prop_dir);
    // Open: an unsaved file.
    std::fs::write(open_dir.join("me-open-corpus/unsaved.pdf"), b"draft").unwrap();
    let open_head = head(&open_dir);

    let pulled = pull_corpora(&f.root);
    for (kind, dir) in [
        (CorpusKind::Proprietary, &prop_dir),
        (CorpusKind::Open, &open_dir),
    ] {
        let p = pulled.iter().find(|p| p.corpus == kind).unwrap();
        match &p.outcome {
            CorpusPullOutcome::NeedsConfirmation { remote, branch, .. } => {
                assert_eq!((remote.as_str(), branch.as_str()), ("origin", "main"));
            }
            other => panic!("{kind:?}: expected a confirmation, got {other:?}"),
        }
        assert_eq!(p.dir, *dir);
    }
    assert_eq!(head(&prop_dir), prop_head);
    assert!(prop_dir.join("papers/local.pdf").exists());
    assert_eq!(head(&open_dir), open_head);
    assert!(open_dir.join("me-open-corpus/unsaved.pdf").exists());

    // "yes, can": the forced pull makes the corpus match its remote and
    // leaves it on `main`.
    crate::advanced_git::force_pull_in(&prop_dir, "origin", "main").unwrap();
    assert_eq!(head(&prop_dir), tip(&f.proprietary, "main"));
    assert!(!prop_dir.join("papers/local.pdf").exists());
    assert_eq!(g(&prop_dir, &["symbolic-ref", "--short", "HEAD"]), "main");
}

/// Maintainer, 2026-09-30: *"make sure the pull button from kovan gui also
/// pulls in both corpuses"*. A Kovan folder whose corpora are registered but
/// were never fetched (a plain clone, or `git submodule deinit`) used to get
/// "not pulled — not downloaded" from Pull and stay empty. Now Pull fetches
/// both the open and the proprietary corpus, and leaves each on `main` at
/// its remote's tip.
#[test]
fn pulling_downloads_corpora_that_are_configured_but_not_fetched() {
    let Some(f) = fixture() else { return };
    let open_dir = f.root.open_corpus_dir();
    let prop_dir = f.root.restricted_sources_dir();
    for dir in [&open_dir, &prop_dir] {
        let rel = dir.strip_prefix(f.root.path()).unwrap().to_string_lossy().to_string();
        g(f.root.path(), &["submodule", "deinit", "-q", "-f", "--", &rel]);
        assert!(!dir.join(".git").exists(), "{rel} still checked out");
    }

    let pulled = pull_corpora_with(&f.root, &f.standard, "main");
    let lines: Vec<String> = pulled.iter().map(CorpusPull::line).collect();
    for (kind, dir, remote) in [
        (CorpusKind::Open, &open_dir, &f.open),
        (CorpusKind::Proprietary, &prop_dir, &f.proprietary),
    ] {
        let p = pulled.iter().find(|p| p.corpus == kind).unwrap();
        assert_eq!(
            p.outcome,
            CorpusPullOutcome::Downloaded {
                branch: "main".into(),
                head: tip(remote, "main"),
            },
            "{lines:?}"
        );
        assert_eq!(head(dir), tip(remote, "main"));
        assert_eq!(g(dir, &["symbolic-ref", "--short", "HEAD"]), "main");
    }
    assert!(open_dir.join("me-open-corpus/b.pdf").exists());
    assert!(prop_dir.join("papers/c.pdf").exists());

    // The next pull finds them present and up to date.
    for p in pull_corpora_with(&f.root, &f.standard, "main") {
        assert!(
            matches!(p.outcome, CorpusPullOutcome::UpToDate { .. }),
            "{}",
            p.line()
        );
    }
}
