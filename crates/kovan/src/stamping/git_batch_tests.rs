//! Tests of [`super::GitBatch`]: every batched answer against the `git`
//! command it replaces, on a throwaway repository with a branch, a merge and
//! a commit outside `HEAD`'s history.

use super::*;
use crate::stamping::states::stamp_commit;

/// A repository: `main` adds `review.md` (`A`), adds `X`, branches `side`
/// (edits `lib.rs`, adds `Y` above `A`), removes `X` on `main`, merges `side` (the
/// merge's `review.md` differs from both parents), adds `X` again; `other`
/// holds one commit outside `HEAD`'s history.
fn repo() -> (tempfile::TempDir, Vec<String>) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().to_path_buf();
    let run = |a: &[&str]| git::git(&p, a).unwrap();
    let write = |f: &str, t: &str| std::fs::write(p.join(f), t).unwrap();
    let commit = |m: &str| {
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", m]);
        run(&["rev-parse", "HEAD"]).trim().to_string()
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@example.org"]);
    run(&["config", "user.name", "T"]);
    run(&["config", "commit.gpgsign", "false"]);
    std::fs::create_dir(p.join("sub")).unwrap();
    write("sub/k.toml", "k = 1\n");
    write("review.md", "A\n");
    write("lib.rs", "fn a() {}\nfn b() {}\nfn c() {}\n");
    let c1 = commit("one");
    write("review.md", "A\nX\n");
    let c2 = commit("add X\n\nCo-Authored-By: Agent <noreply@anthropic.com>");
    run(&["checkout", "-q", "-b", "side"]);
    write("lib.rs", "fn a() {}\nfn b() { 1; }\nfn c() {}\n");
    let c3 = commit("edit b");
    write("review.md", "Y\nA\nX\n");
    let c4 = commit("add Y");
    run(&["checkout", "-q", "main"]);
    write("review.md", "A\n");
    let c5 = commit("drop X");
    run(&["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    let m = run(&["rev-parse", "HEAD"]).trim().to_string();
    write("review.md", "Y\nA\nX\n");
    let c6 = commit("X again");
    run(&["checkout", "-q", "-b", "other", &c1]);
    write("lib.rs", "fn other() {}\n");
    let o = commit("other");
    run(&["checkout", "-q", "main"]);
    (d, vec![c1, c2, c3, c4, c5, m, c6, o])
}

/// The stamp commit as the refresh found it before #1023: `git log -1
/// -S<value>` and three more commands (the oracle).
fn pickaxe(root: &Path, review_md: &str, needle: &str, certified: &str) -> Option<StampCommit> {
    if !git::show(root, "HEAD", review_md)?.contains(needle) {
        return None;
    }
    let out = git::git(root, &["log", "-1", "--format=%H%x1f%ct%x1f%B", &format!("-S{needle}"), "--", review_md]).ok()?;
    let mut parts = out.splitn(3, '\x1f');
    let commit = parts.next()?.trim().to_string();
    let time = parts.next().and_then(|t| t.trim().parse().ok());
    let body = parts.next().unwrap_or("");
    if commit.is_empty() {
        return None;
    }
    let descendant = git::git(root, &["merge-base", "--is-ancestor", certified, &commit]).is_ok();
    let same = git::rev_parse(root, certified).is_ok_and(|c| c == commit);
    let in_history = git::git(root, &["merge-base", "--is-ancestor", certified, "HEAD"]).is_ok();
    Some(StampCommit {
        commit,
        after_certified: descendant && !same,
        certified_in_history: in_history,
        agent_trailer: kovan_common::review::types::agent_trailer(body).0,
        committer_time: time,
    })
}

/// Methodology (GitHub #1023, "the results must not change"): on the
/// repository above, every question the batch answers is also put to the
/// `git` command it replaces, and the answers must be equal: `show` for
/// each commit and a blob, a missing file and a directory (which the batch
/// hands back to `git show`); `rev-parse` and `cat-file -e` for each commit,
/// `HEAD~1`, a branch name, names that are no commit (one empty, one with a
/// space, a full id not in the clone), and the `cat-file` still running
/// after all of them; `merge-base
/// --is-ancestor` for every ordered pair of those (in and out of `HEAD`'s
/// history); `show -s --format=%ct`; the last commit to change a file; the
/// stamp commit (`log -1 -S`) for values added, removed, re-added across a
/// merge, present only in a merge's result, and absent, each against every
/// certified commit; and `git blame` of several ranges at once (one past
/// the end of the file, an uncommitted edit) against one `blame` each.
///
/// Result (2026-10-22): passes; every answer equal.
#[test]
fn batched_answers_equal_the_commands_they_replace() {
    let (d, commits) = repo();
    let p = d.path();
    let mut b = GitBatch::new(p);
    let mut revs: Vec<String> = commits.clone();
    revs.extend(["HEAD".into(), "HEAD~1".into(), "side".into(), "nope".into(), String::new(), "a b".into(), "0123456789abcdef0123456789abcdef01234567".into()]);
    for rev in &revs {
        for file in ["review.md", "lib.rs", "missing.rs", "sub", "sub/k.toml"] {
            assert_eq!(b.show(rev, file), git::show(p, rev, file), "show {rev}:{file}");
        }
        assert_eq!(b.resolve(rev), git::rev_parse(p, rev).ok(), "rev-parse {rev}");
        let exists = git::git(p, &["cat-file", "-e", &format!("{rev}^{{commit}}")]).is_ok();
        assert_eq!(b.in_clone(rev), exists, "cat-file -e {rev}");
        let ct = git::git(p, &["show", "-s", "--format=%ct", rev]).ok().and_then(|s| s.trim().parse::<i64>().ok());
        assert_eq!(b.committer_time(rev), ct, "%ct {rev}");
        for other in &revs {
            let want = git::git(p, &["merge-base", "--is-ancestor", rev, other]).is_ok();
            assert_eq!(b.is_ancestor(rev, other), want, "is-ancestor {rev} {other}");
        }
    }
    assert!(b.cat_running(), "no answer broke the cat-file stream");
    for f in ["review.md", "lib.rs", "nothing.rs"] {
        let want = git::git(p, &["log", "-1", "--format=%H", "--", f]).ok().map(|s| s.trim().to_string());
        assert_eq!(b.last_change(f), want, "last change {f}");
    }
    let mut found = 0;
    for needle in ["X", "Y", "A", "Y\nA", "A\nX", "zzz"] {
        for certified in &revs {
            let want = pickaxe(p, "review.md", needle, certified);
            found += usize::from(want.is_some());
            assert_eq!(stamp_commit(&mut b, "review.md", needle, certified), want, "-S{needle:?} certified {certified}");
        }
    }
    assert!(found > 20, "the values are found: {found}");
    assert!(pickaxe(p, "review.md", "X", &commits[0]).is_some_and(|s| s.commit == commits[6] && !s.agent_trailer));

    std::fs::write(p.join("lib.rs"), "fn a() {}\nfn b() { 1; }\nfn c() { 2; }\n").unwrap();
    for ranges in [vec![[1, 1], [2, 3]], vec![[1, 3], [3, 3]], vec![[2, 2], [1, 99]]] {
        let set: BTreeSet<[u32; 2]> = ranges.iter().copied().collect();
        let got = b.blame("lib.rs", &set);
        for [x, y] in ranges {
            assert_eq!(got[&[x, y]], blame_range(p, "lib.rs", x, y), "blame {x},{y}");
        }
    }
    assert_eq!(b.blame("lib.rs", &BTreeSet::from([[1, 3]]))[&[1, 3]], vec!["t@example.org".to_string()]);
}

/// Methodology (GitHub #1023, "per-HEAD caching where safe"): a batch
/// dropped keeps its stamp commits, history and blame for the next batch
/// at the same `HEAD`; after a new commit nothing carries over; a commit
/// outside `HEAD`'s history is never kept (it can be collected without
/// `HEAD` moving): after `other` is deleted and collected, a new batch at
/// the same `HEAD` says it is gone, as `git` does.
///
/// Result (2026-10-22): passes.
#[test]
fn answers_are_kept_per_head_only() {
    let (d, commits) = repo();
    let p = d.path();
    let key = ("review.md".to_string(), "X".to_string(), commits[0].clone());
    {
        let mut b = GitBatch::new(p);
        assert!(b.in_clone(&commits[7]));
        assert!(b.committer_time(&commits[7]).is_some());
        let found = stamp_commit(&mut b, "review.md", "X", &commits[0]);
        b.keep_added(key.clone(), found);
        b.blame("lib.rs", &BTreeSet::from([[1, 2]]));
    }
    {
        let mut b = GitBatch::new(p);
        assert!(b.kept_added(&key).is_some(), "kept for this HEAD");
        assert!(b.kept.history.is_some());
        assert_eq!(b.kept.blame.len(), 1);
        assert!(b.kept.times.is_empty(), "a commit outside HEAD's history is not kept");
    }
    git::git(p, &["branch", "-q", "-D", "other"]).unwrap();
    git::git(p, &["reflog", "expire", "--expire=now", "--all"]).unwrap();
    git::git(p, &["gc", "-q", "--prune=now"]).unwrap();
    {
        let mut b = GitBatch::new(p);
        assert!(!b.in_clone(&commits[7]));
        assert_eq!(b.committer_time(&commits[7]), None);
    }
    std::fs::write(p.join("review.md"), "B\n").unwrap();
    git::git(p, &["commit", "-q", "-am", "new head"]).unwrap();
    let mut b = GitBatch::new(p);
    assert_eq!(b.kept_added(&key), None, "a new HEAD starts afresh");
    assert!(b.kept.blame.is_empty());
}

/// Methodology: the parsers on hand-written input. A commit header gives
/// its committer seconds, and a header this does not read gives `None`
/// (the caller then asks git); `rev-list --parents` gives each commit's
/// parents and the walk reaches ancestors only; porcelain blame maps final
/// lines to e-mails; a log entry keeps its body with the newline git ends
/// it with.
///
/// Result (2026-10-22): passes.
#[test]
fn the_parsers_read_what_git_writes() {
    let c = b"tree 1\nparent 2\nauthor A <a@x> 10 +0000\ncommitter C <c@x> 1700000000 +0800\n\nmsg\n";
    assert_eq!(committer_seconds(c), Some(Some(1_700_000_000)));
    assert_eq!(committer_seconds(b"tree 1\ncommitter C <c@x> soon +0800\n\nm"), None);
    assert_eq!(committer_seconds(b"tree 1\n\ncommitter C <c@x> 1 +0000\n"), None, "a message line is not the header");
    let h = History::parse("c b\nb a x\na\nx\n");
    assert!(h.reaches("c", "a") && h.reaches("c", "c") && h.reaches("b", "x"));
    assert!(!h.reaches("b", "c"));
    let sha = "a".repeat(40);
    let por = format!("{sha} 1 7 2\nauthor-mail <m@x>\n\tline\n{sha} 2 8\nauthor-mail <n@x>\n\tauthor-mail <no@x>\n");
    assert_eq!(porcelain_mails(&por), BTreeMap::from([(7, "<m@x>".to_string()), (8, "<n@x>".to_string())]));
    let log = LogCommit::parse_all("\0a\x1fb c\x1f12\x1fsubject\n\nbody\n\n\0d\x1f\x1f\x1fx\n");
    assert_eq!(log.len(), 2);
    assert_eq!((log[0].parents.len(), log[0].time, log[0].body.as_str()), (2, Some(12), "subject\n\nbody\n\n"));
    assert_eq!((log[1].parents.len(), log[1].time), (0, None));
}
