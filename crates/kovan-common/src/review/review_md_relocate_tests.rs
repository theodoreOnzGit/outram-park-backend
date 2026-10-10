//! Tests of moving and retiring a function's `review.md` entries
//! (GitHub #771): entries follow a cross-folder move with a `moved`
//! record and their signature bytes unchanged; unreadable entries and
//! other functions' entries stay byte for byte; a deletion becomes a
//! history row.

use std::collections::BTreeMap;

use super::*;
use crate::review::draft::{draft_needs_fix, draft_review, DraftInput};
use crate::review::id::mint_fn_id;
use crate::review::index::{FunctionIndex, ItemKind};
use crate::review::review_md::ReviewEntry;
use crate::review::review_md_write::{upsert_needs_fix, upsert_review};
use crate::review::signing::signed_bytes;
use crate::review::wizard::TestAuthorship;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const ACK: &str = "89abcdef0123456789abcdef0123456789abcdef";
const FILE: &str = "crates/x/src/lib.rs";
const NOW: &str = "2026-10-10T10:00:00+08:00";

/// A malformed review entry (no `[review]` table): unreadable.
const BROKEN: &str = "# Review: broken (github:someone)\n\n```toml\n[kovan]\nid = \"review-broken\"\nkind = \"review\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\nKeep   these   bytes.\n";

fn func(qual: &str) -> FunctionIndex {
    FunctionIndex {
        id: mint_fn_id(&format!("{FILE}::{qual}"), "t", "t"),
        name: qual.into(),
        qual: qual.into(),
        item: ItemKind::Fn,
        lines: [1, 3],
        hash: format!("sha256:{}", "b".repeat(64)),
        doc_hash: format!("sha256:{}", "d".repeat(64)),
        callees: Vec::new(),
        reached_by: Vec::new(),
        test: false,
        index_out_of_date: false,
        physical_interface: false,
    }
}

fn draft(f: &FunctionIndex, by: &str) -> ReviewEntry {
    draft_review(&DraftInput {
        function: f.clone(),
        file: FILE.into(),
        callee_hashes: BTreeMap::new(),
        by: by.into(),
        date: "2026-10-10".into(),
        now: NOW.into(),
        commit: SHA.into(),
        cargo_lock: None,
        checklist: crate::review::draft::tests::clean(),
        tests: TestAuthorship::Unknown,
        is_port: false,
        authorship: None,
        no_concept: None,
        relations: Vec::new(),
        previous: None,
    })
    .unwrap()
}

/// `BROKEN`, two reviewers' stamps of `twice` with comments, a needs-fix on
/// `twice` and a stamp of `other`.
fn fixture() -> (String, FunctionIndex) {
    let twice = func("twice");
    let other = func("other");
    let mut md = BROKEN.to_string();
    md = upsert_review(&md, &draft(&twice, "github:a"), "A's careful comments.")
        .unwrap()
        .text;
    md = upsert_review(&md, &draft(&twice, "github:b"), "")
        .unwrap()
        .text;
    md = upsert_review(&md, &draft(&other, "github:a"), "Other is fine.")
        .unwrap()
        .text;
    let n = draft_needs_fix(
        &twice,
        FILE,
        "github:a",
        "2026-10-10",
        NOW,
        SHA,
        "magic number",
    )
    .unwrap();
    md = upsert_needs_fix(&md, &n, "").unwrap().text;
    (md, twice)
}

/// Methodology: acknowledge a move of `twice` from `crates/x/src/lib.rs`
/// to `crates/x/src/sub/a.rs` (another folder) whose `review.md` already
/// holds an unrelated unreadable entry. Pass: the old file keeps `BROKEN`
/// byte for byte and `other`'s stamp, and names `twice` nowhere; the new
/// file has both reviews and the needs-fix at the new path, each review
/// with one `moved` record (from, to, the acknowledge commit), the
/// comments kept, the signed bytes unchanged (path is not signed), and its
/// own unreadable entry kept.
///
/// Result (2026-10-10): passes.
#[test]
fn a_cross_folder_move_carries_every_entry_with_a_moved_record() {
    let (md, twice) = fixture();
    let to = "crates/x/src/sub/a.rs::twice";
    let before: Vec<Vec<u8>> = parse_review_md(&md)
        .reviews()
        .filter(|r| r.function_id() == twice.id)
        .map(signed_bytes)
        .collect();
    let r = acknowledge_move(&md, Some(BROKEN), &twice.id, to, ACK).unwrap();
    assert_eq!(r.entries, 3);
    assert!(r.from_text.starts_with(BROKEN), "{}", r.from_text);
    let old = parse_review_md(&r.from_text);
    assert_eq!(old.unreadable.len(), 1);
    assert_eq!(old.reviews().count(), 1, "only other's stamp is left");
    assert!(old.reviews().all(|r| r.function_id() != twice.id));
    assert_eq!(old.needs_fixes().count(), 0);

    let new_text = r.to_text.unwrap();
    assert!(new_text.starts_with(BROKEN));
    let new = parse_review_md(&new_text);
    assert_eq!(new.unreadable.len(), 1);
    let moved: Vec<&ReviewEntry> = new.reviews().collect();
    assert_eq!(moved.len(), 2);
    for m in &moved {
        assert_eq!(m.path().as_deref(), Some(to));
        assert_eq!(
            m.review.moved,
            vec![MoveRecord {
                from: format!("{FILE}::twice"),
                to: Some(to.into()),
                commit: ACK.into()
            }]
        );
    }
    let after: Vec<Vec<u8>> = moved.iter().map(|r| signed_bytes(r)).collect();
    assert_eq!(
        before, after,
        "the signed bytes do not change with the path"
    );
    assert!(new_text.contains("A's careful comments."));
    assert!(new_text.contains("# Review: twice (github:a)"));
    assert_eq!(
        new.needs_fixes().next().unwrap().path().as_deref(),
        Some(to)
    );
}

/// Methodology: a move inside the folder (a rename `twice` -> `double` in
/// the same file). Pass: entries are rewritten in place (headings follow
/// the new name), `BROKEN` stays first byte for byte, and `other` is
/// untouched. Moving a function with no entry is refused.
///
/// Result (2026-10-10): passes.
#[test]
fn a_move_in_the_folder_rewrites_in_place() {
    let (md, twice) = fixture();
    let to = format!("{FILE}::double");
    let r = acknowledge_move(&md, None, &twice.id, &to, ACK).unwrap();
    assert!(r.to_text.is_none());
    assert!(r.from_text.starts_with(BROKEN));
    assert!(r.from_text.contains("# Review: double (github:b)"));
    assert!(r.from_text.contains("Other is fine."));
    let doc = parse_review_md(&r.from_text);
    assert_eq!(doc.reviews().count(), 3);
    assert!(doc
        .reviews()
        .filter(|r| r.function_id() == twice.id)
        .all(|r| r.path().as_deref() == Some(to.as_str()) && r.review.moved.len() == 1));
    assert_eq!(
        acknowledge_move(&md, None, "fn:nothing", &to, ACK),
        Err(RelocateError::NotFound("fn:nothing".into()))
    );
}

/// Methodology: record the deletion of `twice` twice over. Pass: every
/// entry of `twice` is gone, `other` and `BROKEN` stay, a
/// `deleted_functions` table holds one row (the second call does not add a
/// duplicate).
///
/// Result (2026-10-10): passes.
#[test]
fn a_deletion_becomes_one_history_row() {
    let (md, twice) = fixture();
    let row = DeletedFunction {
        function: twice.id.clone(),
        path: format!("{FILE}::twice"),
        deleted_commit: Some(ACK.into()),
        branch: Some("develop".into()),
        last_review_commit: SHA.into(),
        reviewers: vec!["github:a".into(), "github:b".into()],
    };
    let text = record_deletion(&md, &twice.id, &row, NOW).unwrap();
    assert!(text.starts_with(BROKEN));
    let doc = parse_review_md(&text);
    assert!(doc.reviews().all(|r| r.function_id() != twice.id));
    assert_eq!(doc.reviews().count(), 1);
    assert_eq!(doc.needs_fixes().count(), 0);
    let rows = |d: &crate::review::review_md::ReviewDocument| -> Vec<DeletedFunction> {
        d.entries
            .iter()
            .filter_map(|e| match &e.entry {
                Entry::DeletedFunctions(t) => Some(t.deleted.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    };
    assert_eq!(rows(&doc), vec![row.clone()]);
    // Nothing left to remove: refused, the table is not touched.
    assert!(matches!(
        record_deletion(&text, &twice.id, &row, NOW),
        Err(RelocateError::NotFound(_))
    ));
    // Another function deleted later joins the same table.
    let other = func("other");
    let row2 = DeletedFunction {
        function: other.id.clone(),
        path: format!("{FILE}::other"),
        ..row.clone()
    };
    let text2 = record_deletion(&text, &other.id, &row2, NOW).unwrap();
    assert_eq!(rows(&parse_review_md(&text2)), vec![row, row2]);
    assert_eq!(parse_review_md(&text2).reviews().count(), 0);
    assert!(text2.starts_with(BROKEN));
}
