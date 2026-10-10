//! Tests of [`super::draft_review`] and [`super::draft_needs_fix`]: a draft
//! signed by a founder key and judged by the staleness engine with
//! signatures enforced must be **valid** at rung 3; every refusal is typed.
//! Pure: no disk, no git (git facts are built by hand, as the engine's own
//! tests do).

use std::collections::BTreeMap;

use super::*;
use crate::review::engine::{
    evaluate, FolderReviews, GitFacts, ReviewKey, SignaturePolicy, StampCommit, StampFacts,
    StampState,
};
use crate::review::id::mint_fn_id;
use crate::review::index::{FolderIndex, ItemKind, ModuleIndex};
use crate::review::review_md::{Entry, ParsedEntry, ReviewDocument};
use crate::review::root::{CodeReviewSettings, ReviewRoot};
use crate::review::root_append::founding_reviewer;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const BY: &str = "github:tester";
const FILE: &str = "crates/x/src/lib.rs";

pub(crate) fn h(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn func(qual: &str, hash: char, callees: &[&str]) -> FunctionIndex {
    FunctionIndex {
        id: mint_fn_id(&format!("{FILE}::{qual}"), "t", "t"),
        name: qual.into(),
        qual: qual.into(),
        item: ItemKind::Fn,
        lines: [1, 3],
        hash: h(hash),
        doc_hash: h('d'),
        callees: callees
            .iter()
            .map(|q| mint_fn_id(&format!("{FILE}::{q}"), "t", "t"))
            .collect(),
        reached_by: Vec::new(),
        test: false,
        index_out_of_date: false,
        physical_interface: false,
    }
}

/// A complete answer set that stamps at rung 3 (as the engine's tests use).
pub(crate) fn clean() -> BTreeMap<String, String> {
    [
        ("doc_matches_behaviour", "yes"),
        ("limits_and_guards", "guarded_returns_result"),
        ("error_handling", "returns_result"),
        ("numerical_hazards", "none_found"),
        ("test_reach", "reached_and_checked"),
        ("vv_evidence", "unit_tests_only"),
        ("vv_case_author", "agent_wrote_or_cowrote"),
        ("maintainability", "yes"),
        ("independence", "someone_else"),
        ("unintended_function", "no"),
        ("coding_standards", "yes"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn input(f: &FunctionIndex, leaf: &FunctionIndex) -> DraftInput {
    DraftInput {
        function: f.clone(),
        file: FILE.into(),
        callee_hashes: [(leaf.id.clone(), leaf.hash.clone())].into_iter().collect(),
        by: BY.into(),
        date: "2026-10-10".into(),
        now: "2026-10-10T10:00:00+08:00".into(),
        commit: SHA.into(),
        cargo_lock: Some(h('1')),
        checklist: clean(),
        tests: TestAuthorship::Unknown,
        is_port: false,
        authorship: None,
        no_concept: None,
        relations: Vec::new(),
        previous: None,
    }
}

fn index(fns: &[FunctionIndex]) -> FolderIndex {
    let mut idx = FolderIndex::new("x", "crates/x/src");
    idx.modules.insert(
        "lib.rs".into(),
        ModuleIndex {
            path: "crate".into(),
            functions: fns.to_vec(),
        },
    );
    idx
}

fn doc(entries: Vec<Entry>) -> ReviewDocument {
    ReviewDocument {
        entries: entries
            .into_iter()
            .map(|entry| ParsedEntry {
                heading: "h".into(),
                line: 1,
                entry,
                body: String::new(),
            })
            .collect(),
        unreadable: Vec::new(),
        migrated: Vec::new(),
    }
}

/// Git facts that make the review authentic (committed after `SHA`, the
/// hash at `SHA` is the recorded one, no agent trailer).
fn git_for(r: &ReviewEntry) -> GitFacts {
    let mut g = GitFacts {
        head: "f".repeat(40),
        cargo_lock: h('1'),
        ..GitFacts::default()
    };
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
                committer_time: None,
            }),
            reviewed_commit_time: None,
            tests_at_review: None,
        },
    );
    g
}

/// Methodology: draft a review of `twice` (calling `leaf`) from index
/// facts, sign it with a freshly generated founder key (test passphrase,
/// in memory), and run `engine::evaluate` with `SignaturePolicy::Enforce`
/// against a root naming that key's reviewer as founder. Pass: the
/// function is `Valid` at rung 3 and the draft's fields come from the
/// index (target, hash, doc_hash, callee hashes) and the gate (rung).
///
/// Result (2026-10-10): passes.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn signed_draft_evaluates_valid_at_rung_3() {
    use crate::review::signing::keystore::generate;
    let leaf = func("leaf", 'a', &[]);
    let twice = func("twice", 'b', &["leaf"]);
    let mut r = draft_review(&input(&twice, &leaf)).unwrap();
    assert_eq!(r.kovan.target.as_deref(), Some(twice.id.as_str()));
    assert_eq!(r.review.path.as_deref(), Some("crates/x/src/lib.rs::twice"));
    assert_eq!(
        (r.review.rung, r.review.hash.as_str()),
        (3, twice.hash.as_str())
    );
    assert_eq!(r.review.callees.get(&leaf.id), Some(&leaf.hash));
    assert_eq!(r.kovan.origin.as_deref(), Some("human"));
    assert!(r.review.signature.is_none(), "a draft is unsigned");

    let (kf, key) = generate(BY, "k1", "2026-10-10", "test passphrase").unwrap();
    key.sign_review_at(&mut r, "2026-10-10T10:00:00+08:00")
        .unwrap();
    let root = ReviewRoot {
        code_review: Some(CodeReviewSettings {
            founder: Some(BY.into()),
            ..Default::default()
        }),
        reviewers: vec![founding_reviewer(BY, None, kf.reviewer_key(), "2026-10-10")],
        deleted_crates: Vec::new(),
    };
    let reviews = [FolderReviews {
        krate: "x".into(),
        dir: "crates/x/src".into(),
        doc: doc(vec![Entry::Review(r.clone())]),
    }];
    let ev = evaluate(
        &reviews,
        &[index(&[leaf, twice.clone()])],
        &root,
        &git_for(&r),
        &BTreeMap::new(),
        SignaturePolicy::Enforce,
    );
    let f = &ev.functions[&twice.id];
    assert_eq!(f.state, StampState::Valid, "{:?}", f.reviews);
    assert_eq!(f.rung, Some(3));
}

/// Methodology: each refusal the module doc lists, one input at a time:
/// an out-of-date index entry, a callee without a hash, no wizard answers
/// (gate blocks), a malformed commit, a first-version id; and a re-stamp
/// keeps the previous entry's id and `created`.
///
/// Result (2026-10-10): passes.
#[test]
fn refusals_are_typed_and_restamp_keeps_identity() {
    let leaf = func("leaf", 'a', &[]);
    let twice = func("twice", 'b', &["leaf"]);
    let mut i = input(&twice, &leaf);
    i.function.index_out_of_date = true;
    assert!(matches!(
        draft_review(&i),
        Err(DraftError::IndexOutOfDate { .. })
    ));
    let mut i = input(&twice, &leaf);
    i.callee_hashes.clear();
    assert_eq!(
        draft_review(&i),
        Err(DraftError::MissingCalleeHash(leaf.id.clone()))
    );
    let mut i = input(&twice, &leaf);
    i.checklist.clear();
    assert!(matches!(draft_review(&i), Err(DraftError::Gate(r)) if !r.is_empty()));
    let mut i = input(&twice, &leaf);
    i.commit = "HEAD".into();
    assert!(matches!(draft_review(&i), Err(DraftError::Field(_))));
    let mut i = input(&twice, &leaf);
    i.function.id = "crates/x/src/lib.rs::twice".into();
    assert!(matches!(
        draft_review(&i),
        Err(DraftError::NotAFunctionId(_))
    ));
    assert!(DraftError::Gate(vec![]).to_string().contains("wizard"));

    let first = draft_review(&input(&twice, &leaf)).unwrap();
    assert!(
        first.kovan.id.starts_with("review-twice-tester-"),
        "{}",
        first.kovan.id
    );
    let mut i = input(&twice, &leaf);
    i.now = "2026-10-11T09:00:00+08:00".into();
    i.previous = Some(first.kovan.clone());
    let again = draft_review(&i).unwrap();
    assert_eq!(
        (again.kovan.id.as_str(), again.kovan.created.as_str()),
        (first.kovan.id.as_str(), first.kovan.created.as_str())
    );
    assert_eq!(again.kovan.modified, "2026-10-11T09:00:00+08:00");
}

/// Methodology: a drafted needs-fix on unchanged code makes the engine show
/// "needs fix" whatever the reviews say; a too-short note is refused.
///
/// Result (2026-10-10): passes.
#[test]
fn needs_fix_draft_blocks_the_function() {
    let twice = func("twice", 'b', &[]);
    let n = draft_needs_fix(
        &twice,
        FILE,
        BY,
        "2026-10-10",
        "2026-10-10T10:00:00+08:00",
        SHA,
        "off by one",
    )
    .unwrap();
    assert!(
        n.kovan.id.starts_with("needs-fix-twice-tester-"),
        "{}",
        n.kovan.id
    );
    let reviews = [FolderReviews {
        krate: "x".into(),
        dir: "crates/x/src".into(),
        doc: doc(vec![Entry::NeedsFix(n)]),
    }];
    let ev = evaluate(
        &reviews,
        &[index(&[twice.clone()])],
        &ReviewRoot::default(),
        &GitFacts::default(),
        &BTreeMap::new(),
        SignaturePolicy::Enforce,
    );
    assert!(matches!(
        ev.functions[&twice.id].state,
        StampState::NeedsFixOpen { .. }
    ));
    assert!(matches!(
        draft_needs_fix(&twice, FILE, BY, "2026-10-10", "now", SHA, "x"),
        Err(DraftError::Field(_))
    ));
}
