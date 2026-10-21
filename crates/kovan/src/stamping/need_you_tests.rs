//! End-to-end tests of the need-you queue, move acknowledge, deletion
//! history, the stamp's change authorship and commit-and-push (GitHub
//! #771), on throwaway git repositories in temporary folders with a local
//! bare remote. Nothing touches the real keystore, `~/.config/kovan`, the
//! real workspace or any real remote; keys use the test passphrase in a
//! temporary keystore.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use kovan_common::code_index::refresh::refresh_folder;
use kovan_common::review::review_md::{parse_review_md, Entry};
use kovan_common::review::state::StateKind;
use kovan_common::review::types::AuthorshipKind;

use super::commit_push::{commit_and_push, CommitPushError, CommitPushJob};
use super::queue::{build, mark_seen, seen_path, Filter, QueueOptions, RowKind};
use super::recent::RecentWhat;
use super::relocate::{acknowledge_moves, record_deletions};
use super::tests::{founder_key, Repo, BY, DIR, LIB};
use super::*;

const AGENT: &str = "\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_test";
const SUB: &str = "crates/demo/src/sub";

/// Commit everything with the agent attribution trailer.
fn agent_commit(r: &Repo, msg: &str) {
    r.git(&["add", "-A"]);
    r.git(&["commit", "-q", "-m", &format!("{msg}{AGENT}")]);
}

/// Local identity, so kovan's own `git commit` (no `-c` flags) works.
fn identity(dir: &Path) {
    for (k, v) in [
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
    ] {
        let ok = Command::new("git")
            .current_dir(dir)
            .args(["config", k, v])
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }
}

/// Regenerate the fixture folder's `kovan.toml` after an edit, as an
/// index run would (the refresh, then `twice -> leaf` kept by hand: the
/// call graph needs rust-analyzer).
fn reindex(r: &Repo) {
    let path = format!("{DIR}/kovan.toml");
    let prev = kovan_common::review::index::FolderIndex::parse(&r.read(&path)).unwrap();
    let files = std::collections::BTreeMap::from([("lib.rs".to_string(), r.read(LIB))]);
    let head = head_of(r);
    let (mut idx, _) = refresh_folder("demo", DIR, Some(&prev), &files, None, &[], &head);
    let m = idx.modules.get_mut("lib.rs").unwrap();
    let leaf = m
        .functions
        .iter()
        .find(|f| f.qual == "leaf")
        .unwrap()
        .id
        .clone();
    for f in &mut m.functions {
        f.index_out_of_date = false;
        if f.qual == "twice" {
            f.callees = vec![leaf.clone()];
        }
    }
    r.write(&path, &idx.to_toml().unwrap());
}

fn opts() -> QueueOptions {
    QueueOptions::default()
}

/// Methodology: on the fixture, stamp `twice` (its lines were written in
/// human commits), then an agent commit edits `twice` and the reviewer
/// re-stamps. Pass: the first stamp records `authorship = human`; the
/// re-stamp records `agent` with the session link (only the commits since
/// the previous review count).
///
/// Result (2026-10-10): passes.
#[test]
fn the_stamp_records_the_authorship_of_the_reviewed_change() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "twice", "");
    let md = r.read(&format!("{DIR}/{REVIEW_MD}"));
    let a = parse_review_md(&md)
        .reviews()
        .next()
        .unwrap()
        .review
        .authorship
        .clone()
        .unwrap();
    assert_eq!(a.kind, AuthorshipKind::Human);

    r.edit("leaf(x) + 1.0", "leaf(x) + 2.0");
    reindex(&r);
    agent_commit(&r, "agent edits twice");
    r.stamp(&key, "twice", "Re-reviewed.");
    let md = r.read(&format!("{DIR}/{REVIEW_MD}"));
    let a = parse_review_md(&md)
        .reviews()
        .next()
        .unwrap()
        .review
        .authorship
        .clone()
        .unwrap();
    assert_eq!(a.kind, AuthorshipKind::Agent);
    assert_eq!(
        a.sessions,
        vec!["https://claude.ai/code/session_test".to_string()]
    );
    assert_eq!(r.state("twice").unwrap().state, Some(StateKind::Valid));
}

/// Methodology: stamp `leaf` and `twice`; an agent commit then edits
/// `leaf` and adds two new functions (one reached by nothing); a human
/// commit adds a third. Build the queue. Then open the agent commit's
/// folded row (mark it seen) and build again.
///
/// Pass: `leaf` is a directly stale row on top, agent-authored; `twice`
/// is a re-confirm row (its callee changed), agent-authored through the
/// callee's lines; the new functions fold into one row per commit
/// ("＋ 2 new functions · <4 hex> · 2 untested" for the agent commit, the
/// human one separately), the agent row carrying the session; the filter
/// keeps the agent rows under "agent only" and the human commit's row
/// under "human only"; the recently-reviewed list holds both stamps, and
/// the crate's maturity reads in plain English from `code_map.json`; after
/// opening, the agent commit's folded row is gone.
///
/// Result (2026-10-10): passes.
#[test]
fn the_queue_puts_stale_on_top_and_folds_new_functions_per_commit() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "leaf", "");
    r.stamp(&key, "twice", "");
    r.edit("x * 2.0", "x * 3.0");
    let lib = r.read(LIB);
    r.write(
        LIB,
        &format!("{lib}\n/// New one.\npub fn fresh_a() -> u8 {{\n    1\n}}\n\n/// New two.\npub fn fresh_b() -> u8 {{\n    2\n}}\n"),
    );
    agent_commit(&r, "agent: edit leaf, add two");
    let lib = r.read(LIB);
    r.write(
        LIB,
        &format!("{lib}\n/// By hand.\npub fn by_hand() -> u8 {{\n    3\n}}\n"),
    );
    r.commit("human adds one");
    // The Code Review data's code map, as "Index fresh" leaves it.
    let map = kovan_common::code_map::CodeMap {
        root: "r".into(),
        crates: vec![kovan_common::code_map::CrateNode {
            name: "demo".into(),
            description: None,
            backronym: None,
            row: 1,
            topic: kovan_common::code_map::Topic::Risk,
            fidelity: None,
            maturity: 3,
            maturity_modules: vec![],
            lib_dir: None,
            dir: Some("crates/demo".into()),
        }],
        edges: vec![],
    };
    let data = crate::code_review_data::data_dir(r.path());
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(
        data.join("code_map.json"),
        serde_json::to_string(&map).unwrap(),
    )
    .unwrap();

    let q = build(r.path(), &opts()).unwrap();
    let kinds: Vec<&RowKind> = q.rows.iter().map(|r| &r.kind).collect();
    assert_eq!(kinds[0], &RowKind::Stale, "{:#?}", q.rows);
    assert!(q.rows[0].title.starts_with("leaf"));
    assert_eq!(
        q.rows[0].open_target(),
        Some(format!("{LIB}::leaf").as_str())
    );
    assert_eq!(
        q.rows[0].authorship.as_ref().unwrap().kind,
        AuthorshipKind::Agent
    );
    let twice = q
        .rows
        .iter()
        .find(|r| r.title.starts_with("twice"))
        .unwrap();
    assert!(matches!(twice.kind, RowKind::ReConfirm { .. }), "{twice:?}");
    assert_eq!(
        twice.authorship.as_ref().unwrap().kind,
        AuthorshipKind::Agent
    );
    assert!(
        twice.detail.iter().any(|d| d.contains("session_test")),
        "{:?}",
        twice.detail
    );

    let new: Vec<_> = q
        .rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::NewCode { .. }))
        .collect();
    // The human commit, the agent commit, and the fixture's first commit
    // (which introduced `other`, never reviewed).
    assert_eq!(new.len(), 3, "{new:#?}");
    assert!(
        new[2].title.starts_with("\u{ff0b} 1 new function"),
        "{}",
        new[2].title
    );
    // Newest commit first: the human one, then the agent one.
    assert!(
        new[0].title.starts_with("\u{ff0b} 1 new function \u{b7} "),
        "{}",
        new[0].title
    );
    assert!(
        new[1].title.starts_with("\u{ff0b} 2 new functions \u{b7} "),
        "{}",
        new[1].title
    );
    assert!(
        new[1].title.ends_with(" \u{b7} 2 untested"),
        "{}",
        new[1].title
    );
    assert_eq!(new[1].functions.len(), 2);
    assert!(new[1]
        .functions
        .iter()
        .all(|f| f.call_graph_id.starts_with(LIB)));
    let agent_commit_id = match &new[1].kind {
        RowKind::NewCode { commit, .. } => commit.clone(),
        _ => unreachable!(),
    };
    assert_eq!(
        new[1].authorship.as_ref().unwrap().kind,
        AuthorshipKind::Agent
    );
    assert!(new[1].authorship_line().unwrap().contains("session_test"));
    assert!(
        q.rows.iter().position(|r| r.kind == RowKind::Stale)
            < q.rows
                .iter()
                .position(|r| matches!(r.kind, RowKind::NewCode { .. }))
    );

    let agent_only = q.filtered(Filter::AgentOnly);
    assert!(agent_only.iter().any(|r| r.kind == RowKind::Stale));
    assert!(agent_only.iter().all(|r| r.key != new[0].key));
    let human_only = q.filtered(Filter::HumanOnly);
    assert_eq!(human_only.len(), 2);
    assert_eq!(
        (human_only[0].key.as_str(), human_only[1].key.as_str()),
        (new[0].key.as_str(), new[2].key.as_str())
    );
    assert_eq!(q.filtered(Filter::All).len(), q.rows.len());

    assert_eq!(q.recent.len(), 2);
    assert!(q
        .recent
        .iter()
        .all(|e| e.what == RecentWhat::Stamped && e.by == BY));
    assert!(
        q.recent[0].line().starts_with("stamped "),
        "{}",
        q.recent[0].line()
    );
    assert!(!q.recent[0].call_graph_id.is_empty());
    let m = q.maturity_of("demo").unwrap();
    assert_eq!(m.stale, 2);
    assert!(
        m.words
            .starts_with("Maturity 2: AI V&V (was 3: human reviewed). 2 reviewed functions"),
        "{}",
        m.words
    );

    mark_seen(&seen_path(r.path()), &agent_commit_id).unwrap();
    mark_seen(&seen_path(r.path()), &agent_commit_id).unwrap();
    let q = build(r.path(), &opts()).unwrap();
    let new: Vec<_> = q
        .rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::NewCode { .. }))
        .collect();
    assert_eq!(new.len(), 2, "the opened row is cleared");
    assert!(new.iter().all(|r| !r.key.contains(&agent_commit_id)));
}

/// The fixture's `other` (doc comment and body), byte for byte.
const OTHER: &str = "/// One.\npub fn other() -> u32 {\n    1\n}\n";

/// Methodology: stamp `other`, then move it, unchanged, to
/// `crates/demo/src/sub/x.rs` (another folder, with its own `kovan.toml`
/// as an index run writes it) and commit. Build the queue; batch
/// acknowledge the move row's ready functions; commit; judge again.
///
/// Pass: one moved row `crates/demo/src → crates/demo/src/sub` with
/// `other` ready (no reaching test fails); the acknowledge writes both
/// `review.md` files (the entry leaves the old folder's and lands in the
/// new folder's with a `moved` record at HEAD); after committing, `other`
/// is valid at its new place (the signature still verifies, the hash at
/// the certified commit is read at the old path); acknowledging again
/// skips it.
///
/// Result (2026-10-10): passes.
#[test]
fn acknowledging_a_cross_folder_move_moves_the_review_entry() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "other", "Other checked.");
    let lib = r.read(LIB);
    r.write(LIB, &lib.replace(OTHER, ""));
    r.write(&format!("{SUB}/x.rs"), OTHER);
    r.commit("move other into sub");
    let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
    let files = std::collections::BTreeMap::from([("x.rs".to_string(), OTHER.to_string())]);
    let (mut idx, _) = refresh_folder("demo", SUB, None, &files, None, &[], &head);
    for m in idx.modules.values_mut() {
        for f in &mut m.functions {
            f.index_out_of_date = false;
        }
    }
    r.write(&format!("{SUB}/kovan.toml"), &idx.to_toml().unwrap());
    r.commit("index sub");

    let q = build(r.path(), &opts()).unwrap();
    let mv = q
        .rows
        .iter()
        .find(|r| matches!(r.kind, RowKind::Moved { .. }))
        .unwrap_or_else(|| panic!("{:#?}", q.rows));
    let RowKind::Moved {
        from_dir,
        to_dir,
        ready,
    } = &mv.kind
    else {
        unreachable!()
    };
    assert_eq!((from_dir.as_str(), to_dir.as_str()), (DIR, SUB));
    assert_eq!(ready.len(), 1, "{:?}", mv.detail);
    assert_eq!(
        mv.open_target(),
        Some(format!("{SUB}/x.rs::other").as_str())
    );

    let rep = acknowledge_moves(r.path(), ready).unwrap();
    assert_eq!(rep.done, ready.clone(), "{:?}", rep.skipped);
    assert_eq!(
        rep.written,
        BTreeSet::from([format!("{DIR}/{REVIEW_MD}"), format!("{SUB}/{REVIEW_MD}")])
    );
    let old = parse_review_md(&r.read(&format!("{DIR}/{REVIEW_MD}")));
    assert_eq!(old.reviews().count(), 0);
    let new_md = r.read(&format!("{SUB}/{REVIEW_MD}"));
    let new = parse_review_md(&new_md);
    let e = new.reviews().next().unwrap();
    assert_eq!(
        e.path().as_deref(),
        Some(format!("{SUB}/x.rs::other").as_str())
    );
    assert_eq!(e.review.moved.len(), 1);
    assert_eq!(e.review.moved[0].commit, head_of(&r));
    assert!(new_md.contains("Other checked."));

    r.commit("acknowledge the move");
    let s = stamp_states(r.path())
        .unwrap()
        .into_iter()
        .find(|s| s.function == format!("{SUB}/x.rs::other"))
        .expect("other has a state at its new place");
    assert_eq!(s.state, Some(StateKind::Valid), "{}", s.reason);
    let again = acknowledge_moves(r.path(), ready).unwrap();
    assert!(again.done.is_empty() && again.skipped.len() == 1);
}

fn head_of(r: &Repo) -> String {
    r.git(&["rev-parse", "HEAD"]).trim().to_string()
}

/// Methodology: stamp `other`, delete it from the source and commit.
/// Pass: a deleted row; recording the deletion removes its entry from
/// `review.md` and adds a `deleted_functions` row naming the last review
/// commit and the reviewer; the row is gone from the next queue.
///
/// Result (2026-10-10): passes.
#[test]
fn a_deleted_function_goes_to_the_history_table() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "other", "");
    let stamped_at = parse_review_md(&r.read(&format!("{DIR}/{REVIEW_MD}")))
        .reviews()
        .next()
        .unwrap()
        .review
        .commit
        .clone();
    let lib = r.read(LIB);
    r.write(LIB, &lib.replace(OTHER, ""));
    r.commit("delete other");
    let q = build(r.path(), &opts()).unwrap();
    let row = q
        .rows
        .iter()
        .find(|r| r.kind == RowKind::Deleted)
        .unwrap_or_else(|| panic!("{:#?}", q.rows));
    assert!(row.title.contains("other"), "{}", row.title);
    let ids: Vec<String> = row.functions.iter().map(|f| f.id.clone()).collect();
    let rep = record_deletions(r.path(), &ids).unwrap();
    assert_eq!(rep.done, ids, "{:?}", rep.skipped);
    let doc = parse_review_md(&r.read(&format!("{DIR}/{REVIEW_MD}")));
    assert_eq!(doc.reviews().count(), 0);
    let rows: Vec<_> = doc
        .entries
        .iter()
        .filter_map(|e| match &e.entry {
            Entry::DeletedFunctions(d) => Some(d.deleted.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].last_review_commit, stamped_at);
    assert_eq!(rows[0].reviewers, vec![BY.to_string()]);
    r.commit("record deletion");
    let q = build(r.path(), &opts()).unwrap();
    assert!(q.rows.iter().all(|r| r.kind != RowKind::Deleted));
}

/// A bare repository in a temporary folder, and a second clone of it.
struct Remote {
    bare: tempfile::TempDir,
}

impl Remote {
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("--git-dir")
            .arg(self.bare.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

/// Methodology: the fixture with a local bare remote `origin`. On `main`,
/// stamp `twice` and press Commit and push: refused, nothing committed.
/// Switch to `develop` and press again: committed (only `review.md` and
/// `kovan_root.toml`, an unrelated edit left uncommitted) and pushed to
/// the remote's `develop`; the message has no agent trailer and the stamp
/// turns valid. Then another clone pushes a commit to `develop` (the
/// remote moves on), the reviewer stamps `leaf` and presses again (through
/// the worker job): the remote's commit is merged, the push goes through,
/// and the local stamp commit is on the remote (never reset).
///
/// Result (2026-10-10): passes.
#[test]
fn commit_and_push_lands_on_develop_refuses_main_and_merges_a_moved_remote() {
    let r = Repo::new();
    identity(r.path());
    let remote = Remote {
        bare: tempfile::tempdir().unwrap(),
    };
    let out = Command::new("git")
        .args(["init", "-q", "--bare", "-b", "develop"])
        .arg(remote.bare.path())
        .status()
        .unwrap();
    assert!(out.success());
    r.git(&[
        "remote",
        "add",
        "origin",
        &remote.bare.path().to_string_lossy(),
    ]);
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.git(&["reset", "-q", "--soft", "HEAD~1"]); // the key registration, left for kovan to commit
    r.git(&["reset", "-q"]);

    // Stamp without committing: the dialog's write.
    let req = StampRequest {
        function: format!("{LIB}::twice"),
        by: BY.into(),
        checklist: super::tests::clean(),
        ..StampRequest::default()
    };
    // The root is uncommitted, which draft_stamp does not mind (it checks
    // only the function's file).
    let mut d = draft_stamp(r.path(), &req).unwrap();
    key.sign_review(&mut d.entry).unwrap();
    write_review(r.path(), &d.entry, "").unwrap();
    r.write("NOTES.txt", "an unrelated edit\n");
    let files = BTreeSet::from([format!("{DIR}/{REVIEW_MD}"), ROOT_FILE.to_string()]);

    let head = head_of(&r);
    let e = commit_and_push(r.path(), &files).unwrap_err();
    assert_eq!(
        e,
        CommitPushError::MainRefused {
            branch: "main".into()
        }
    );
    assert!(e.to_string().contains("never `main`"));
    assert_eq!(head_of(&r), head, "nothing committed on main");
    assert!(r.git(&["status", "--porcelain"]).contains("review.md"));
    // A file kovan does not write is refused.
    assert!(matches!(
        commit_and_push(r.path(), &BTreeSet::from([LIB.to_string()])),
        Err(CommitPushError::MainRefused { .. })
    ));

    r.git(&["switch", "-q", "-c", "develop"]);
    assert!(matches!(
        commit_and_push(r.path(), &BTreeSet::from([LIB.to_string()])),
        Err(CommitPushError::NotKovanFile(_))
    ));
    let p = commit_and_push(r.path(), &files).unwrap();
    assert_eq!(
        (p.branch.as_str(), p.remote.as_str(), p.merged.clone()),
        ("develop", "origin", None)
    );
    assert_eq!(remote.git(&["rev-parse", "develop"]), p.commit);
    let msg = r.git(&["log", "-1", "--format=%B"]);
    assert!(msg.starts_with("kovan review: 1 stamp (#771)"), "{msg}");
    assert!(!msg.contains("Claude"), "{msg}");
    let shown = r.git(&["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(
        shown.lines().collect::<BTreeSet<_>>(),
        BTreeSet::from([ROOT_FILE, "crates/demo/src/review.md"])
    );
    assert!(
        r.git(&["status", "--porcelain"]).contains("NOTES.txt"),
        "the unrelated edit stays"
    );
    assert_eq!(r.state("twice").unwrap().state, Some(StateKind::Valid));
    assert_eq!(
        commit_and_push(r.path(), &files),
        Err(CommitPushError::NothingToCommit)
    );

    // The remote moves on.
    let other = tempfile::tempdir().unwrap();
    let ok = Command::new("git")
        .args(["clone", "-q", "-b", "develop"])
        .arg(remote.bare.path())
        .arg(other.path())
        .status()
        .unwrap();
    assert!(ok.success());
    identity(other.path());
    std::fs::write(other.path().join("README.md"), "from elsewhere\n").unwrap();
    for args in [
        vec!["add", "README.md"],
        vec!["commit", "-q", "-m", "elsewhere"],
        vec!["push", "-q", "origin", "develop"],
    ] {
        assert!(Command::new("git")
            .current_dir(other.path())
            .args(&args)
            .status()
            .unwrap()
            .success());
    }
    let theirs = remote.git(&["rev-parse", "develop"]);

    let mut d = draft_stamp(
        r.path(),
        &StampRequest {
            function: format!("{LIB}::leaf"),
            ..req
        },
    )
    .unwrap();
    key.sign_review(&mut d.entry).unwrap();
    write_review(r.path(), &d.entry, "").unwrap();
    std::fs::remove_file(r.path().join("NOTES.txt")).unwrap();
    let mut job = CommitPushJob::default();
    assert!(job.start(r.path(), BTreeSet::from([format!("{DIR}/{REVIEW_MD}")])));
    assert!(!job.start(r.path(), BTreeSet::new()), "one run at a time");
    job.wait();
    let status = job.status().unwrap().unwrap();
    assert!(status.contains("after merging"), "{status}");
    let Some(Ok(p)) = job.last.clone() else {
        panic!()
    };
    assert_eq!(p.merged.as_deref(), Some(theirs.as_str()));
    let tip = remote.git(&["rev-parse", "develop"]);
    assert_eq!(tip, head_of(&r), "the merge was pushed");
    let ancestor = Command::new("git")
        .arg("--git-dir")
        .arg(remote.bare.path())
        .args(["merge-base", "--is-ancestor", &p.commit, "develop"])
        .status()
        .unwrap();
    assert!(
        ancestor.success(),
        "the local stamp commit is on the remote"
    );
    assert!(r.path().join("README.md").exists());
}
