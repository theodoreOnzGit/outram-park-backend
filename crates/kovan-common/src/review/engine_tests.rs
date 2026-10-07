//! One test per #739 D6 scenario and per #765 rule. Every test builds the
//! three inputs (review.md entries, current kovan.toml indexes, git facts)
//! by hand and checks the engine's state; nothing touches disk or git.
//!
//! Result (2026-10-07): all pass.

use super::*;
use crate::review::id::mint_fn_id;
use crate::review::index::{FunctionIndex, ItemKind, ModuleIndex};
use crate::review::review_md::{
    Entry, EntryMeta, NeedsFixBody, NeedsFixEntry, ParsedEntry, ReviewBody, Unreadable,
};
use crate::review::root::{
    Qualification, QualificationBasis, QualificationRecord, Reviewer,
};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const M: &str = "github:m";
const R: &str = "github:r";

/// The stable id of the test function first seen at `path`.
fn fid(path: &str) -> String {
    mint_fn_id(path, "t", "t")
}

fn h(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn fun(id: &str, hash: char, callees: &[&str], tests: &[&str]) -> FunctionIndex {
    let qual = id.rsplit_once(".rs::").map(|(_, q)| q.to_string()).unwrap_or_default();
    FunctionIndex {
        id: fid(id),
        name: qual.rsplit("::").next().unwrap_or("").into(),
        qual,
        item: ItemKind::Fn,
        lines: [1, 5],
        hash: h(hash),
        doc_hash: h('d'),
        callees: callees.iter().map(|s| fid(s)).collect(),
        reached_by: tests.iter().map(|s| s.to_string()).collect(),
        test: false,
    }
}

/// An index of `dir` holding `file` with `fns` (ids may name another file:
/// the indexer keeps a moved function's id).
fn folder(krate: &str, dir: &str, files: &[(&str, Vec<FunctionIndex>)]) -> FolderIndex {
    let mut idx = FolderIndex::new(krate, dir);
    idx.crate_root = dir.ends_with("/src");
    for (file, fns) in files {
        idx.modules.insert(
            file.to_string(),
            ModuleIndex {
                path: format!("crate::{}", file.trim_end_matches(".rs")),
                functions: fns.clone(),
            },
        );
    }
    idx.test_run = Some(TestRun {
        commit: SHA.into(),
        cargo_lock: h('1'),
        suite: Suite::Full,
        passed: vec!["t::ok".into(), "t::ok2".into()],
        failed: vec!["t::bad".into()],
        edited: vec!["t::edited".into()],
    });
    idx
}

/// A review of `id` (target = where it was), hash `hash`.
fn review(id: &str, by: &str, hash: char, callees: &[(&str, char)]) -> ReviewEntry {
    let qual = id.split_once(".rs::").map(|(_, q)| q).unwrap();
    ReviewEntry {
        kovan: EntryMeta {
            id: format!("review-{qual}-{by}"),
            kind: "review".into(),
            origin: Some("human".into()),
            created: "c".into(),
            modified: "m".into(),
            target: Some(fid(id)),
        },
        review: ReviewBody {
            function: None,
            path: Some(id.into()),
            by: by.into(),
            rung: 3,
            date: "2026-10-07".into(),
            commit: SHA.into(),
            hash: h(hash),
            doc_hash: h('d'),
            cargo_lock: Some(h('1')),
            callees: callees.iter().map(|(c, x)| (fid(c), h(*x))).collect(),
            checklist: BTreeMap::new(),
            no_concept: None,
            authorship: None,
            moved: vec![],
            signature: None,
        },
        relations: vec![],
    }
}

fn entries(e: Vec<Entry>) -> ReviewDocument {
    ReviewDocument {
        entries: e
            .into_iter()
            .map(|entry| ParsedEntry {
                heading: "h".into(),
                line: 1,
                entry,
                body: String::new(),
            })
            .collect(),
        unreadable: vec![],
        migrated: vec![],
    }
}

fn reviews_in(krate: &str, dir: &str, rs: Vec<ReviewEntry>) -> FolderReviews {
    FolderReviews {
        krate: krate.into(),
        dir: dir.into(),
        doc: entries(rs.into_iter().map(Entry::Review).collect()),
    }
}

fn root() -> ReviewRoot {
    let rv = |id: &str, role: Role, scope: &[&str]| Reviewer {
        id: id.into(),
        name: None,
        role,
        scope: scope.iter().map(|s| s.to_string()).collect(),
        qualification: vec![],
        admitted: None,
        admitted_by: None,
        keys: vec![],
        revoked: None,
    };
    ReviewRoot {
        code_review: None,
        reviewers: vec![
            rv(M, Role::Maintainer, &[]),
            rv(R, Role::Reviewer, &["crates/x/**"]),
            rv("github:q", Role::Reviewer, &["crates/x/**"]),
        ],
        deleted_crates: vec![],
    }
}

/// Git facts that make every given review authentic.
fn git_for(rs: &[&FolderReviews]) -> GitFacts {
    let mut g = GitFacts {
        head: "f".repeat(40),
        cargo_lock: h('1'),
        ..GitFacts::default()
    };
    for fr in rs {
        for r in fr.doc.reviews() {
            g.stamps.insert(
                ReviewKey {
                    function: r.function_id(),
                    by: r.review.by.clone(),
                },
                StampFacts {
                    hash_at_commit: Some(r.review.hash.clone()),
                    added_in: Some(StampCommit {
                        commit: "e".repeat(40),
                        after_certified: true,
                        agent_trailer: false,
                    }),
                },
            );
        }
    }
    g
}

/// [`evaluate`] with the run's hashes filled in for every function the
/// test did not set: by default nothing changed since the recorded run.
fn eval(
    revs: &[FolderReviews],
    idx: &[FolderIndex],
    rt: &ReviewRoot,
    g: &GitFacts,
    concepts: &ConceptAreas,
    policy: SignaturePolicy,
) -> Evaluation {
    let mut g = g.clone();
    for i in idx {
        for (_, f) in i.functions() {
            g.hashes_at_test_run.entry(f.id.clone()).or_insert_with(|| f.hash.clone());
        }
    }
    evaluate(revs, idx, rt, &g, concepts, policy)
}

fn run(revs: &[FolderReviews], idx: &[FolderIndex]) -> Evaluation {
    let refs: Vec<&FolderReviews> = revs.iter().collect();
    eval(revs, idx, &root(), &git_for(&refs), &ConceptAreas::new(), SignaturePolicy::NotChecked)
}

fn kind(ev: &Evaluation, path: &str) -> StateKind {
    ev.functions[&fid(path)].state.kind()
}

const D: &str = "crates/x/src";
const F: &str = "crates/x/src/a.rs::f";

/// Baseline: an unchanged reviewed function is valid at rung 3; an
/// unreviewed one is new; a changed one is directly stale; a doc-only edit
/// is doc changed.
#[test]
fn baseline_valid_new_stale_doc() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &["t::ok"]), fun("crates/x/src/a.rs::g", 'b', &[], &[])])])]);
    assert_eq!(kind(&ev, F), StateKind::Valid);
    assert_eq!(ev.functions[&fid(F)].rung, Some(3));
    assert_eq!(kind(&ev, "crates/x/src/a.rs::g"), StateKind::New);
    assert!(ev.functions[&fid("crates/x/src/a.rs::g")].untested);

    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![fun(F, 'z', &[], &[])])])]);
    assert_eq!(kind(&ev, F), StateKind::DirectlyStale);

    let mut f = fun(F, 'a', &[], &[]);
    f.doc_hash = h('e');
    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![f])])]);
    assert_eq!(kind(&ev, F), StateKind::DocChanged);
}

/// D6 rename only: the function is found by hash under a new name; the
/// review follows it (moved, awaiting acknowledge) and the id match is
/// reported. Also when the indexer kept the id and only `qual` changed.
#[test]
fn d6_rename_only_review_follows() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let g = "crates/x/src/a.rs::g";
    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![fun(g, 'a', &[], &["t::ok"])])])]);
    assert_eq!(ev.id_matches, vec![IdMatch { review_function: fid(F), current_id: fid(g) }]);
    match &ev.functions[&fid(g)].state {
        StampState::Moved { from, to, tests, .. } => {
            assert_eq!(from.as_ref().unwrap().qual, "f");
            assert_eq!(to.qual, "g");
            assert_eq!(*tests, TestVerdict::Reach(ReachVerdict::Passed { passed: vec!["t::ok".into()], not_run: vec![] }));
        }
        s => panic!("{s:?}"),
    }
    assert!(ev.deleted.is_empty());

    let mut kept = fun(F, 'a', &[], &[]);
    kept.qual = "g".into(); // the indexer kept the id; only the name changed
    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![kept])])]);
    assert_eq!(kind(&ev, F), StateKind::Moved);
}

/// D6 rename and edit in one commit: no carry-over. The new function is
/// new; the old review is deleted and its history row goes to the folder's
/// review.md.
#[test]
fn d6_rename_and_edit_is_new() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let g = "crates/x/src/a.rs::g";
    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![fun(g, 'b', &[], &[])])])]);
    assert_eq!(kind(&ev, g), StateKind::New);
    assert_eq!(ev.deleted.len(), 1);
    assert_eq!(ev.deleted[0].id, fid(F));
    assert_eq!(ev.history[0].placement, HistoryPlacement::FolderReviewMd { dir: D.into() });
    assert_eq!(ev.history[0].row.last_review_commit, SHA);
}

/// D6 file rename: the review entry stays; the function is moved to the new
/// file and awaits acknowledge.
#[test]
fn d6_file_rename_is_a_move() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let ev = run(&revs, &[folder("x", D, &[("b.rs", vec![fun(F, 'a', &[], &[])])])]);
    match &ev.functions[&fid(F)].state {
        StampState::Moved { to, .. } => assert_eq!(to.file, "crates/x/src/b.rs"),
        s => panic!("{s:?}"),
    }
}

/// D6 file split in two: the functions that stay are valid; the one that
/// lands in another folder is moved, and the batch names both folders.
#[test]
fn d6_file_split_moves_only_what_left() {
    let f2 = "crates/x/src/a.rs::f2";
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[]), review(f2, M, 'b', &[])])];
    let ev = run(
        &revs,
        &[
            folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])]),
            folder("x", "crates/x/src/sub", &[("c.rs", vec![fun(f2, 'b', &[], &[])])]),
        ],
    );
    assert_eq!(kind(&ev, F), StateKind::Valid);
    assert_eq!(kind(&ev, f2), StateKind::Moved);
    let batches = ev.move_batches();
    assert_eq!(batches[&(D.to_string(), "crates/x/src/sub".to_string())], vec![fid(f2)]);
}

/// D6 whole folder moved: every function is moved, in one batch.
#[test]
fn d6_folder_move_is_one_batch() {
    let old = "crates/x/src/old";
    let (a, b) = ("crates/x/src/old/m.rs::a", "crates/x/src/old/m.rs::b");
    let revs = [reviews_in("x", old, vec![review(a, M, 'a', &[]), review(b, M, 'b', &[])])];
    let ev = run(&revs, &[folder("x", "crates/x/src/new", &[("m.rs", vec![fun(a, 'a', &[], &[]), fun(b, 'b', &[], &[])])])]);
    let batches = ev.move_batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[&(old.to_string(), "crates/x/src/new".to_string())], {
        let mut v = vec![fid(a), fid(b)];
        v.sort();
        v
    });
}

/// D6 function deleted: removed, with a history row in its folder's
/// review.md carrying the deleting commit and its reviewers.
#[test]
fn d6_function_delete_history_in_folder() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[]), review(F, R, 'a', &[])])];
    let refs: Vec<&FolderReviews> = revs.iter().collect();
    let mut g = git_for(&refs);
    g.deleted_in.insert(fid(F), "9".repeat(40));
    let idx = [folder("x", D, &[("a.rs", vec![fun("crates/x/src/a.rs::other", 'q', &[], &[])])])];
    let ev = eval(&revs, &idx, &root(), &g, &ConceptAreas::new(), SignaturePolicy::NotChecked);
    assert_eq!(ev.deleted[0].reviews.len(), 2);
    let h = &ev.history[0];
    assert_eq!(h.placement, HistoryPlacement::FolderReviewMd { dir: D.into() });
    assert_eq!(h.row.deleted_commit.as_deref(), Some("9".repeat(40).as_str()));
    assert_eq!(h.row.reviewers, vec![M.to_string(), R.to_string()]);
}

/// D6 whole folder deleted: its history goes to the crate root kovan.toml.
#[test]
fn d6_folder_delete_history_in_crate_index() {
    let old = "crates/x/src/old";
    let revs = [reviews_in("x", old, vec![review("crates/x/src/old/m.rs::a", M, 'a', &[])])];
    let ev = run(&revs, &[folder("x", D, &[("lib.rs", vec![])])]);
    assert_eq!(
        ev.history[0].placement,
        HistoryPlacement::CrateIndex { krate: "x".into(), dir: D.into(), deleted_folder: old.into() }
    );
}

/// D6 crate deleted: its history goes to kovan_root.toml.
#[test]
fn d6_crate_delete_history_in_root() {
    let revs = [reviews_in("gone", "crates/gone/src", vec![review("crates/gone/src/lib.rs::a", M, 'a', &[])])];
    let ev = run(&revs, &[folder("x", D, &[("lib.rs", vec![])])]);
    assert_eq!(
        ev.history[0].placement,
        HistoryPlacement::WorkspaceRoot { krate: "gone".into(), dir: "crates/gone/src".into() }
    );
}

/// D6 callee changed: the caller always needs a re-confirm, even with every
/// reaching test passing (necessary, not sufficient); a failing reaching
/// test blocks the re-confirm.
#[test]
fn d6_callee_change_needs_reconfirm_and_failing_test_blocks() {
    let g = "crates/x/src/a.rs::g";
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[(g, 'b')]), review(g, M, 'b', &[])])];
    let idx = |tests: &[&str]| [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[g], tests), fun(g, 'c', &[], &[])])])];
    let ev = run(&revs, &idx(&["t::ok"]));
    assert_eq!(kind(&ev, g), StateKind::DirectlyStale);
    match &ev.functions[&fid(F)].state {
        StampState::InheritedStale { cause, blocked, tests } => {
            assert_eq!(*cause, InheritedCause::Callees(vec![fid(g)]));
            assert!(!blocked);
            assert_eq!(*tests, TestVerdict::Reach(ReachVerdict::Passed { passed: vec!["t::ok".into()], not_run: vec![] }));
        }
        s => panic!("{s:?}"),
    }
    assert_eq!(ev.functions[&fid(F)].blocked_by, vec![fid(g)]);
    let ev = run(&revs, &idx(&["t::ok", "t::bad"]));
    match &ev.functions[&fid(F)].state {
        StampState::InheritedStale { blocked, tests, .. } => {
            assert!(*blocked);
            assert_eq!(*tests, TestVerdict::Reach(ReachVerdict::Failed { failed: vec!["t::bad".into()] }));
        }
        s => panic!("{s:?}"),
    }
    // A test edited in the same change does not count.
    let ev = run(&revs, &idx(&["t::edited"]));
    assert!(matches!(
        &ev.functions[&fid(F)].state,
        StampState::InheritedStale { tests: TestVerdict::Reach(ReachVerdict::NoneRan { .. }), blocked: true, .. }
    ));
}

/// D6 / external dependency update: a Cargo.lock change leaves the stamp
/// pending a workspace test; a full pass at the new lock clears it with no
/// re-confirm; a reaching test failing at the new lock makes it inherited
/// stale.
#[test]
fn d6_cargo_lock_change_pending_until_full_pass() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let refs: Vec<&FolderReviews> = revs.iter().collect();
    let mut g = git_for(&refs);
    g.cargo_lock = h('2');
    let ev_at = |run_lock: char, tests: &[&str], g: &GitFacts| {
        let mut idx = folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], tests)])]);
        idx.test_run.as_mut().unwrap().cargo_lock = h(run_lock);
        eval(&revs, &[idx], &root(), g, &ConceptAreas::new(), SignaturePolicy::NotChecked)
    };
    assert_eq!(kind(&ev_at('1', &["t::ok"], &g), F), StateKind::PendingWorkspaceTest);
    assert_eq!(kind(&ev_at('2', &["t::ok"], &g), F), StateKind::Valid);
    assert_eq!(kind(&ev_at('2', &[], &g), F), StateKind::Valid, "a full pass clears an untested one too");
    let failing = ev_at('2', &["t::bad"], &g);
    assert!(matches!(
        &failing.functions[&fid(F)].state,
        StampState::InheritedStale { cause: InheritedCause::LockTestFailed, blocked: true, .. }
    ));
    // The function changed since the recorded run: the run does not speak
    // for it, so the stamp stays pending.
    let mut changed = g.clone();
    changed.hashes_at_test_run.insert(fid(F), h('z'));
    assert_eq!(kind(&ev_at('2', &["t::ok"], &changed), F), StateKind::PendingWorkspaceTest);
}

/// D6 time-bound stamps: the hash recomputed at the certified commit must
/// match, and the stamp must arrive after it; an agent-trailer commit never
/// counts.
#[test]
fn d6_time_bound_stamps() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])])];
    let refs: Vec<&FolderReviews> = revs.iter().collect();
    let key = ReviewKey { function: fid(F), by: M.into() };
    let state_with = |facts: StampFacts| {
        let mut g = git_for(&refs);
        g.stamps.insert(key.clone(), facts);
        eval(&revs, &idx, &root(), &g, &ConceptAreas::new(), SignaturePolicy::NotChecked).functions[&fid(F)]
            .state
            .clone()
    };
    let good = git_for(&refs).stamps[&key].clone();
    let mut f = good.clone();
    f.hash_at_commit = Some(h('z'));
    assert_eq!(
        state_with(f),
        StampState::Unverified(UnverifiedReason::HashMismatchAtCommit { at_commit: Some(h('z')) })
    );
    let mut f = good.clone();
    f.added_in.as_mut().unwrap().after_certified = false;
    assert_eq!(state_with(f), StampState::Unverified(UnverifiedReason::NotAfterCertifiedCommit));
    let mut f = good.clone();
    f.added_in.as_mut().unwrap().agent_trailer = true;
    assert_eq!(state_with(f), StampState::Unverified(UnverifiedReason::AgentTrailer));
    let mut f = good;
    f.added_in = None;
    assert_eq!(state_with(f), StampState::Unverified(UnverifiedReason::NotCommitted));
    let ev = eval(&revs, &idx, &root(), &GitFacts::default(), &ConceptAreas::new(), SignaturePolicy::NotChecked);
    assert_eq!(ev.functions[&fid(F)].state, StampState::Unverified(UnverifiedReason::NoGitFacts));
}

/// Concern beats approval: an open needs-fix blocks a validly reviewed
/// function; once the code changes it shows as fixed, awaiting re-review.
#[test]
fn concern_beats_approval() {
    let nf = |hash: char| NeedsFixEntry {
        kovan: EntryMeta {
            id: "fix-1".into(),
            kind: "needs_fix".into(),
            origin: None,
            created: "c".into(),
            modified: "m".into(),
            target: Some(fid(F)),
        },
        needs_fix: NeedsFixBody {
            function: None,
            path: Some(F.into()),
            by: R.into(),
            date: "2026-10-07".into(),
            commit: SHA.into(),
            hash: h(hash),
            note: "guard missing".into(),
            status: FixStatus::Open,
            resolved_by: None,
            highlights: vec![],
        },
    };
    let mut fr = reviews_in("x", D, vec![review(F, M, 'a', &[])]);
    fr.doc.entries.push(ParsedEntry { heading: "n".into(), line: 2, entry: Entry::NeedsFix(nf('a')), body: String::new() });
    let ev = run(std::slice::from_ref(&fr), &[folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])])]);
    assert_eq!(kind(&ev, F), StateKind::NeedsFixOpen);
    assert_eq!(ev.functions[&fid(F)].rung, None);
    let ev = run(std::slice::from_ref(&fr), &[folder("x", D, &[("a.rs", vec![fun(F, 'b', &[], &[])])])]);
    assert_eq!(kind(&ev, F), StateKind::Fixed);
}

/// The rung is derived, never chosen (maintainer, #769, 2026-10-07). A
/// recorded rung 4 counts only when the V&V answers qualify, the V&V case
/// was written and verified by hand, and git shows no agent trailer on the
/// commits that added the reaching tests; anything else derives rung 3, so
/// a recorded 4 is invalid (shown, never counted; Leak Before Break). A
/// recorded 3 where 4 is derived is invalid too: the rung is not a choice.
#[test]
fn rung_is_derived_and_a_mismatch_is_invalid() {
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &["t::ok"])])])];
    let human = "add flash analytical test".to_string();
    let agent = "add test\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>".to_string();
    let eval = |r: &ReviewEntry, msgs: Option<&String>| {
        let revs = [reviews_in("x", D, vec![r.clone()])];
        let mut g = git_for(&[&revs[0]]);
        if let Some(m) = msgs {
            g.test_commit_messages.insert("t::ok".into(), vec![m.clone()]);
        }
        eval(&revs, &idx, &root(), &g, &ConceptAreas::new(), SignaturePolicy::NotChecked)
            .functions[&fid(F)]
            .clone()
    };
    let mut r4 = review(F, M, 'a', &[]);
    r4.review.rung = 4;
    r4.review.checklist.insert("vv_evidence".into(), "analytical_case".into());
    r4.review.checklist.insert("vv_case_author".into(), "human_wrote_and_verified".into());
    let f = eval(&r4, Some(&human));
    assert_eq!(f.state, StampState::Valid);
    assert_eq!(f.rung, Some(4));
    // Git contradicts the human answer: invalid, not counted.
    let f = eval(&r4, Some(&agent));
    assert_eq!(
        f.state,
        StampState::Invalid(InvalidReason::RungMismatch { recorded: 4, derived: 3, tests: TestAuthorship::Agent })
    );
    assert_eq!(f.rung, None);
    // No commit facts for the reaching test: git cannot agree, so 3.
    assert_eq!(eval(&r4, None).state.kind(), StateKind::Invalid);
    // A half-done V&V case derives 3.
    let mut half = r4.clone();
    half.review.checklist.insert("vv_case_author".into(), "agent_wrote_or_cowrote".into());
    assert_eq!(eval(&half, Some(&human)).state.kind(), StateKind::Invalid);
    half.review.rung = 3;
    assert_eq!(eval(&half, Some(&human)).rung, Some(3));
    // #764's placeholder q8 opens nothing.
    let mut legacy = review(F, M, 'a', &[]);
    legacy.review.rung = 4;
    legacy.review.checklist.insert("q8".into(), "analytical_case".into());
    assert_eq!(eval(&legacy, Some(&human)).state.kind(), StateKind::Invalid);
    // Recording 3 where 4 is derived is also a mismatch.
    let mut low = r4.clone();
    low.review.rung = 3;
    assert!(matches!(
        eval(&low, Some(&human)).state,
        StampState::Invalid(InvalidReason::RungMismatch { recorded: 3, derived: 4, .. })
    ));
}

/// The key history is append-only (#762 follow-up): against the previous
/// committed `kovan_root.toml`, a removed last entry, a changed entry, a
/// removed key and a removed reviewer each give a loud warning; appending
/// gives none.
#[test]
fn key_history_is_append_only_against_git() {
    use crate::review::root::{KeyEvent, KeyEventKind, ReviewerKey};
    let key = |events: Vec<KeyEvent>| ReviewerKey {
        id: "k1".into(),
        alg: "ed25519".into(),
        public: "AAAA".into(),
        created: "2026-10-07".into(),
        endorsed_by: None,
        reset: false,
        retired: false,
        retired_on: None,
        unretired: None,
        history: events,
    };
    let created = KeyEvent::unsigned(KeyEventKind::Created, "2026-10-07");
    let retired = KeyEvent::unsigned(KeyEventKind::Retired, "2026-11-01");
    let with = |events: Vec<KeyEvent>| {
        let mut r = root();
        r.reviewers[0].keys = vec![key(events)];
        r
    };
    let before = with(vec![created.clone(), retired.clone()]);
    assert!(history_append_only(&before, &before).is_empty());
    let mut appended = before.clone();
    appended.reviewers[0].keys[0]
        .history
        .push(KeyEvent::unsigned(KeyEventKind::Unretired, "2026-12-01"));
    assert!(history_append_only(&before, &appended).is_empty());
    let last_deleted = with(vec![created.clone()]);
    assert_eq!(
        history_append_only(&before, &last_deleted),
        vec![HistoryWarning::EntryRemoved { reviewer: M.into(), key: "k1".into(), index: 1 }]
    );
    let changed = with(vec![created.clone(), KeyEvent::unsigned(KeyEventKind::Retired, "2026-11-02")]);
    assert_eq!(
        history_append_only(&before, &changed),
        vec![HistoryWarning::EntryChanged { reviewer: M.into(), key: "k1".into(), index: 1 }]
    );
    let no_key = with(vec![]);
    let mut no_key = no_key;
    no_key.reviewers[0].keys.clear();
    assert_eq!(
        history_append_only(&before, &no_key),
        vec![HistoryWarning::KeyRemoved { reviewer: M.into(), key: "k1".into() }]
    );
    let mut gone = before.clone();
    gone.reviewers.remove(0);
    assert_eq!(history_append_only(&before, &gone), vec![HistoryWarning::ReviewerRemoved { reviewer: M.into() }]);
    // Wired into evaluate through the git facts.
    let revs = [reviews_in("x", D, vec![])];
    let idx = [folder("x", D, &[("a.rs", vec![])])];
    let mut g = git_for(&[&revs[0]]);
    g.previous_root = Some(before);
    let ev = eval(&revs, &idx, &last_deleted, &g, &ConceptAreas::new(), SignaturePolicy::NotChecked);
    assert_eq!(ev.history_warnings.len(), 1);
}

/// Rung 5: a second valid review by someone who is neither the first
/// reviewer nor a code author, who answered `independence = someone_else`,
/// and who holds a qualification covering the function's concept area. Without the area, the qualification or the
/// independence it stays at the reviews' own rung. Qualification is shown
/// but not enforced below rung 5.
#[test]
fn rung5_needs_independent_qualified_second_reviewer() {
    let mut second = review(F, R, 'a', &[]);
    second.review.checklist.insert("independence".into(), "someone_else".into());
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[]), second.clone()])];
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])])];
    let refs: Vec<&FolderReviews> = revs.iter().collect();
    let g = git_for(&refs);
    let mut rt = root();
    rt.reviewers[1].qualification = vec![Qualification::Record(QualificationRecord {
        area: "concept:thermal-hydraulics".into(),
        basis: QualificationBasis::SelfStudy,
        evidence: vec!["https://github.com/r/notes".into()],
        endorsed_by: None,
        self_declared: true,
    })];
    let areas: ConceptAreas = [(fid(F), ["concept:thermal-hydraulics/natural-circulation".to_string()].into())].into();
    let rung = |rt: &ReviewRoot, g: &GitFacts, a: &ConceptAreas| {
        eval(&revs, &idx, rt, g, a, SignaturePolicy::NotChecked).functions[&fid(F)].rung
    };
    assert_eq!(rung(&rt, &g, &areas), Some(5));
    let ev = eval(&revs, &idx, &rt, &g, &areas, SignaturePolicy::NotChecked);
    let shown = &ev.functions[&fid(F)].reviews.iter().find(|r| r.by == R).unwrap().qualifications;
    assert_eq!(shown, &vec!["thermal-hydraulics (self-study; self-declared)".to_string()]);
    assert_eq!(rung(&rt, &g, &ConceptAreas::new()), Some(3), "no known area");
    assert_eq!(rung(&root(), &g, &areas), Some(3), "not qualified");
    let mut authored = g.clone();
    authored.code_authors.insert(fid(F), [R.to_string()].into());
    assert_eq!(rung(&rt, &authored, &areas), Some(3), "the second reviewer wrote the code");
    // A self-check (or an unstated independence answer) is never the
    // independent review, though it still counts at its own rung.
    for who in [Some("self_check"), None] {
        let mut sc = second.clone();
        match who {
            Some(w) => {
                sc.review.checklist.insert("independence".into(), w.into());
            }
            None => {
                sc.review.checklist.remove("independence");
            }
        }
        let revs2 = [reviews_in("x", D, vec![review(F, M, 'a', &[]), sc])];
        let ev = eval(&revs2, &idx, &rt, &git_for(&[&revs2[0]]), &areas, SignaturePolicy::NotChecked);
        assert_eq!(ev.functions[&fid(F)].rung, Some(3), "{who:?}");
        assert_eq!(ev.functions[&fid(F)].state, StampState::Valid);
    }
    let single = [reviews_in("x", D, vec![review(F, R, 'a', &[])])];
    let ev = eval(&single, &idx, &rt, &git_for(&[&single[0]]), &areas, SignaturePolicy::NotChecked);
    assert_eq!(ev.functions[&fid(F)].rung, Some(3), "one reviewer is never rung 5");
}

/// A malformed entry is no review: the function shows "review unreadable"
/// and does not count; beside a valid review it changes nothing.
#[test]
fn malformed_entry_is_no_review() {
    let u = Unreadable {
        heading: "Broken".into(),
        line: 3,
        message: "bad hash".into(),
        kind: Some("review".into()),
        function: Some(fid(F)),
        by: Some(R.into()),
    };
    let mut fr = reviews_in("x", D, vec![]);
    fr.doc.unreadable.push(u.clone());
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])])];
    let ev = run(std::slice::from_ref(&fr), &idx);
    assert_eq!(kind(&ev, F), StateKind::Unreadable);
    assert!(!kind(&ev, F).counts());
    let mut fr2 = reviews_in("x", D, vec![review(F, M, 'a', &[])]);
    fr2.doc.unreadable.push(u);
    let ev = run(std::slice::from_ref(&fr2), &idx);
    assert_eq!(kind(&ev, F), StateKind::Valid);
    assert_eq!(ev.functions[&fid(F)].reviews.len(), 2);
}

/// Scope, registry and signature: a reviewer outside their scope, an
/// unregistered reviewer, a revoked one, and signature enforcement (which
/// cannot verify until #762) each stop a stamp counting.
#[test]
fn scope_registry_and_signature() {
    let other = "crates/y/src/a.rs::f";
    let revs = [reviews_in("y", "crates/y/src", vec![review(other, R, 'a', &[])])];
    let idx = [folder("y", "crates/y/src", &[("a.rs", vec![fun(other, 'a', &[], &[])])])];
    assert_eq!(kind(&run(&revs, &idx), other), StateKind::OutsideScope);

    let revs = [reviews_in("x", D, vec![review(F, "github:nobody", 'a', &[])])];
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])])];
    assert_eq!(
        run(&revs, &idx).functions[&fid(F)].state,
        StampState::Unverified(UnverifiedReason::UnknownReviewer)
    );

    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let g = git_for(&[&revs[0]]);
    let mut rt = root();
    rt.reviewers[0].revoked = Some(crate::review::root::Revocation {
        date: "2026-10-01".into(),
        compromised_from: None,
        by: M.into(),
        signature: None,
    });
    let ev = eval(&revs, &idx, &rt, &g, &ConceptAreas::new(), SignaturePolicy::NotChecked);
    assert_eq!(ev.functions[&fid(F)].state, StampState::Unverified(UnverifiedReason::Revoked));

    let ev = eval(&revs, &idx, &root(), &g, &ConceptAreas::new(), SignaturePolicy::Enforce);
    assert_eq!(
        ev.functions[&fid(F)].state,
        StampState::Unverified(UnverifiedReason::Signature(SigReason::NoSignature))
    );
}

/// The same text found twice (an identical copy): the review is moved with
/// both candidates, for the maintainer to say which is the original.
#[test]
fn identical_copies_ask_which_is_original() {
    let revs = [reviews_in("x", D, vec![review(F, M, 'a', &[])])];
    let (g1, g2) = ("crates/x/src/a.rs::g1", "crates/x/src/a.rs::g2");
    let ev = run(&revs, &[folder("x", D, &[("a.rs", vec![fun(g1, 'a', &[], &[]), fun(g2, 'a', &[], &[])])])]);
    // The review is shown on the first candidate by id (ids are opaque, so
    // either copy), listing both.
    let mut want = vec![fid(g1), fid(g2)];
    want.sort();
    match &ev.functions[&want[0]].state {
        StampState::Moved { candidates, .. } => assert_eq!(candidates, &want),
        s => panic!("{s:?}"),
    }
    assert_eq!(ev.functions[&want[1]].state.kind(), StateKind::New);
    assert!(ev.id_matches.is_empty());
}

/// Determinism: the same inputs in a different order give the same result.
#[test]
fn evaluation_is_order_independent() {
    let g = "crates/x/src/a.rs::g";
    let a = reviews_in("x", D, vec![review(F, M, 'a', &[(g, 'b')]), review(g, R, 'b', &[])]);
    let b = reviews_in("x", D, vec![review(g, R, 'b', &[]), review(F, M, 'a', &[(g, 'b')])]);
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[g], &[]), fun(g, 'b', &[], &[])])])];
    assert_eq!(run(&[a], &idx), run(&[b], &idx));
}

/// An upstream tag label is checked against what the tag resolves to now:
/// matching, moved, or unchecked when it could not be looked up (offline).
/// It never changes a function's state; the commit pin is the key.
#[test]
fn upstream_tag_moved_is_reported() {
    use crate::review::review_md::{UpstreamEntry, UpstreamTable};
    let repo = "https://github.com/CoolProp/CoolProp".to_string();
    let up = UpstreamEntry {
        kovan: EntryMeta {
            id: "upstream".into(),
            kind: "upstream".into(),
            origin: None,
            created: "c".into(),
            modified: "m".into(),
            target: None,
        },
        upstream: UpstreamTable {
            is_port: true,
            repository: Some(repo.clone()),
            commit: Some(SHA.into()),
            tag: Some("v6.4.1".into()),
            files: BTreeMap::new(),
            routines: BTreeMap::new(),
            confirmed_by: M.into(),
            date: "2026-10-07".into(),
        },
    };
    let mut fr = reviews_in("x", D, vec![review(F, M, 'a', &[])]);
    fr.doc.entries.push(ParsedEntry { heading: "u".into(), line: 9, entry: Entry::Upstream(up), body: String::new() });
    let idx = [folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])])];
    let mut g = git_for(&[&fr]);
    let tags = |g: &GitFacts| {
        eval(std::slice::from_ref(&fr), &idx, &root(), g, &ConceptAreas::new(), SignaturePolicy::NotChecked)
            .upstream_tags
    };
    assert_eq!(tags(&g)[0].check, TagCheck::Unchecked);
    g.tag_commits.insert((repo.clone(), "v6.4.1".into()), SHA.into());
    assert_eq!(tags(&g)[0].check, TagCheck::Matches);
    g.tag_commits.insert((repo, "v6.4.1".into()), "fedcba9".into());
    let t = tags(&g);
    assert_eq!(t[0].check, TagCheck::Moved { now: "fedcba9".into() });
    let ev = eval(std::slice::from_ref(&fr), &idx, &root(), &g, &ConceptAreas::new(), SignaturePolicy::NotChecked);
    assert_eq!(kind(&ev, F), StateKind::Valid, "a moved tag does not touch stamps");
}

/// With signatures enforced (#762), a review signed by the registered
/// founder key counts; an unsigned one is unverified; a scoped reviewer's
/// signed review of a file outside its scope is outside scope. Real
/// generated keys, no mocks.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn enforced_signatures_map_onto_states() {
    use crate::review::signing::keystore;
    let pass = "correct horse battery staple";
    let (mf, mk) = keystore::generate(M, "k1", "2026-10-07", pass).unwrap();
    let (rf, rk) = keystore::generate(R, "r1", "2026-10-07", pass).unwrap();
    let mut rt = root();
    rt.reviewers.retain(|r| r.id == M || r.id == R);
    rt.code_review = Some(crate::review::root::CodeReviewSettings { rust_analyzer: None, founder: Some(M.into()) });
    rt.reviewers[0].keys = vec![mf.reviewer_key()];
    rt.reviewers[1].keys = vec![rf.reviewer_key()];
    rt.reviewers[1].admitted = Some("2026-10-07".into());
    mk.admit(&mut rt.reviewers[1]).unwrap();

    let mut signed = review(F, M, 'a', &[]);
    mk.sign_review(&mut signed).unwrap();
    let unsigned = review(F, R, 'a', &[]);
    let other = "crates/y/src/a.rs::f";
    let mut outside = review(other, R, 'a', &[]);
    rk.sign_review(&mut outside).unwrap();
    let revs = [
        reviews_in("x", D, vec![signed, unsigned]),
        reviews_in("y", "crates/y/src", vec![outside]),
    ];
    let idx = [
        folder("x", D, &[("a.rs", vec![fun(F, 'a', &[], &[])])]),
        folder("y", "crates/y/src", &[("a.rs", vec![fun(other, 'a', &[], &[])])]),
    ];
    let g = git_for(&[&revs[0], &revs[1]]);
    let ev = eval(&revs, &idx, &rt, &g, &ConceptAreas::new(), SignaturePolicy::Enforce);
    let f = &ev.functions[&fid(F)];
    assert_eq!(f.state, StampState::Valid);
    let by_r = f.reviews.iter().find(|r| r.by == R).unwrap();
    assert_eq!(by_r.state, StampState::Unverified(UnverifiedReason::Signature(SigReason::NoSignature)));
    assert_eq!(ev.functions[&fid(other)].state, StampState::OutsideScope);
}
