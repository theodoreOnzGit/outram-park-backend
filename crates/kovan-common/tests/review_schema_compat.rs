//! **Backwards compatibility of the code-review files** (GitHub #764; #739
//! decision 11: additive only, "a test loads every committed `kovan.toml`
//! written by earlier versions").
//!
//! Methodology: every schema version's fixtures live under
//! `tests/fixtures/review/v<N>/` and are never edited once committed; a new
//! version adds a new folder. Each `kovan_root.toml`, `kovan.toml` and
//! `review.md` there must load with this version's reader with no
//! unreadable entries, and re-emitting and re-reading the loaded data must
//! give the same data back.
//!
//! Result (2026-10-07): v1 (the first version) loads; 1 root, 1 index and a
//! `review.md` of 6 entries (one of each kind but `other`).

use std::path::Path;

use kovan_common::review::index::FolderIndex;
use kovan_common::review::review_md::{parse_review_md, render_review_md};
use kovan_common::review::root::ReviewRoot;

#[test]
fn every_committed_review_fixture_loads() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review");
    let mut versions = 0;
    for v in std::fs::read_dir(&base).unwrap() {
        let dir = v.unwrap().path();
        versions += 1;
        let root = std::fs::read_to_string(dir.join("kovan_root.toml")).unwrap();
        let r = ReviewRoot::parse(&root).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert!(!r.reviewers.is_empty());
        assert_eq!(ReviewRoot::parse(&r.write_into(&root).unwrap()).unwrap(), r);

        let idx_text = std::fs::read_to_string(dir.join("kovan.toml")).unwrap();
        let idx = FolderIndex::parse(&idx_text).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert!(idx.functions().count() > 0);
        assert_eq!(FolderIndex::parse(&idx.to_toml().unwrap()).unwrap(), {
            let mut n = idx.clone();
            n.normalise();
            n
        });

        let md = std::fs::read_to_string(dir.join("review.md")).unwrap();
        let doc = parse_review_md(&md);
        assert!(doc.unreadable.is_empty(), "{}: {:?}", dir.display(), doc.unreadable);
        assert!(doc.entries.len() >= 6);
        let again = parse_review_md(&render_review_md(&doc.entries).unwrap());
        let strip = |d: &kovan_common::review::review_md::ReviewDocument| {
            d.entries
                .iter()
                .map(|e| (e.heading.clone(), e.entry.clone(), e.body.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(strip(&again), strip(&doc));
    }
    assert!(versions >= 1);
}

/// **The #764 placeholder checklist keys in the v1 fixture** (GitHub #769).
///
/// Methodology: the v1 `review.md` was written with #764's placeholder keys
/// `q1` and `q8`. The review must stay readable (additive schema), and the
/// wizard's stamp gate must refuse those keys with
/// `LegacyPlaceholderKey` naming the replacing key, never map them, and
/// never let the placeholder `q8 = "reference_code_to_code"` open rung 4.
///
/// Result (2026-10-07): passes; `q1` -> `doc_matches_behaviour`, `q8` ->
/// `vv_evidence`, rung 4 closed.
#[test]
fn v1_placeholder_checklist_keys_load_but_the_wizard_refuses_them() {
    use kovan_common::review::wizard::{stamp_gate, AnswerError, Applicability, GateReason};
    let md = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review/v1/review.md"),
    )
    .unwrap();
    let doc = parse_review_md(&md);
    let r = doc.reviews().next().expect("the v1 fixture has a review");
    assert_eq!(r.review.checklist.get("q8").map(String::as_str), Some("reference_code_to_code"));
    let g = stamp_gate(&r.review.checklist, Applicability::default());
    assert!(!g.stampable() && !g.rung4_allowed);
    for (old, new) in [("q1", "doc_matches_behaviour"), ("q8", "vv_evidence")] {
        assert!(g.blocked_by.contains(&GateReason::Invalid(AnswerError::LegacyPlaceholderKey {
            key: old.into(),
            use_instead: new.into(),
        })));
    }
}
