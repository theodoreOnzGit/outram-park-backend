//! Tests of the `review.md` splice writers: unreadable entries and other
//! reviewers' entries survive byte for byte; a re-stamp by the same
//! reviewer replaces; a write that would not read back is refused.

use std::collections::BTreeMap;

use super::*;
use crate::review::draft::{draft_needs_fix, draft_review, DraftInput};
use crate::review::id::mint_fn_id;
use crate::review::index::{FunctionIndex, ItemKind};
use crate::review::review_md::FixStatus;
use crate::review::wizard::TestAuthorship;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const FILE: &str = "crates/x/src/lib.rs";

/// A malformed review entry (no `[review]` table): unreadable.
const BROKEN: &str = "# Review: broken (github:someone)\n\n```toml\n[kovan]\nid = \"review-broken\"\nkind = \"review\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\nKeep   these   bytes.\n";

/// An entry of another kind, with odd spacing, kept verbatim.
const NOTE: &str =
    "# A note\n\n```toml\n[kovan]\nid = \"note-1\"\nkind = \"note\"\n```\n\nfree   text\n";

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

fn draft(f: &FunctionIndex, by: &str, now: &str) -> ReviewEntry {
    draft_review(&DraftInput {
        function: f.clone(),
        file: FILE.into(),
        callee_hashes: BTreeMap::new(),
        by: by.into(),
        date: "2026-10-10".into(),
        now: now.into(),
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

/// Methodology: a `review.md` holding an unreadable entry and a `note`;
/// write reviewer A's stamp (appended), reviewer B's stamp of the same
/// function (appended), then A's re-stamp (replaces A's only). After every
/// write the unreadable entry and the note are present byte for byte, the
/// file reads with one review per reviewer, and the replaced entry's
/// comments are the new ones.
///
/// Result (2026-10-10): passes.
#[test]
fn restamp_replaces_and_unreadable_survives_byte_for_byte() {
    let f = func("twice");
    let md = format!("{BROKEN}\n{NOTE}");
    let a1 = draft(&f, "github:a", "2026-10-10T10:00:00+08:00");
    let w = upsert_review(&md, &a1, "First look.").unwrap();
    assert!(!w.replaced);
    assert!(
        w.text.starts_with(&md),
        "appending keeps every earlier byte"
    );
    let b = draft(&f, "github:b", "2026-10-10T10:05:00+08:00");
    let w = upsert_review(&w.text, &b, "").unwrap();
    assert!(!w.replaced);
    let mut a2 = draft(&f, "github:a", "2026-10-10T11:00:00+08:00");
    a2.review
        .checklist
        .insert("maintainability".into(), "yes".into());
    a2.kovan.id = a1.kovan.id.clone();
    let w = upsert_review(&w.text, &a2, "Second look.").unwrap();
    assert!(w.replaced);
    assert!(w.text.contains(BROKEN) && w.text.contains(NOTE));
    assert!(w.text.contains("Second look.") && !w.text.contains("First look."));
    let doc = parse_review_md(&w.text);
    assert_eq!(doc.unreadable.len(), 1);
    let reviews: Vec<&ReviewEntry> = doc.reviews().collect();
    assert_eq!(reviews.len(), 2);
    assert!(reviews.contains(&&a2) && reviews.contains(&&b));
    assert!(w.text.contains("## Sign-off"));
}

/// Methodology: the refusals: a comment line that is a `#` heading, and a
/// file already holding two standing reviews by the reviewer (both
/// unreadable; a third would be demoted with them, so the read-back check
/// refuses rather than writing).
///
/// Result (2026-10-10): passes.
#[test]
fn refuses_heading_comments_and_duplicate_standing_reviews() {
    let f = func("twice");
    let a = draft(&f, "github:a", "2026-10-10T10:00:00+08:00");
    assert!(matches!(
        upsert_review("", &a, "# oops"),
        Err(WriteError::HeadingInComments(_))
    ));
    let one = upsert_review("", &a, "").unwrap().text;
    let mut other = a.clone();
    other.kovan.id = "review-dup".into();
    let block =
        crate::artifact::render_block(1, "dup", &Entry::Review(other).to_toml().unwrap(), "");
    let two = format!("{one}\n{block}");
    assert_eq!(parse_review_md(&two).unreadable.len(), 2);
    let e = upsert_review(&two, &a, "").unwrap_err();
    assert!(matches!(e, WriteError::NotReadBack(_)), "{e}");
}

/// Methodology: a needs-fix is appended beside a review (the review kept
/// byte for byte), then resolved by writing the same id with status
/// `resolved`, which replaces it.
///
/// Result (2026-10-10): passes.
#[test]
fn needs_fix_appends_then_resolves_in_place() {
    let f = func("twice");
    let a = draft(&f, "github:a", "2026-10-10T10:00:00+08:00");
    let md = upsert_review(BROKEN, &a, "").unwrap().text;
    let mut n = draft_needs_fix(
        &f,
        FILE,
        "github:b",
        "2026-10-10",
        "2026-10-10T12:00:00+08:00",
        SHA,
        "unit missing",
    )
    .unwrap();
    let w = upsert_needs_fix(&md, &n, "See line 3.").unwrap();
    assert!(!w.replaced && w.text.starts_with(&md));
    n.needs_fix.status = FixStatus::Resolved;
    n.needs_fix.resolved_by = Some(a.kovan.id.clone());
    let w2 = upsert_needs_fix(&w.text, &n, "").unwrap();
    assert!(w2.replaced && w2.text.starts_with(&md));
    let doc = parse_review_md(&w2.text);
    assert_eq!(
        doc.needs_fixes().next().map(|x| x.needs_fix.status),
        Some(FixStatus::Resolved)
    );
    assert_eq!(doc.needs_fixes().count(), 1);
    assert!(WriteError::NotReadBack("x".into())
        .to_string()
        .contains("nothing written"));
}
