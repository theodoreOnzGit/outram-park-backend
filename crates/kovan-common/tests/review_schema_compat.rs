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
//! Result (2026-10-07): v1 (the first version: call-graph-key ids, string
//! qualifications) loads, its ids migrated in memory to `fn:` ids; v2 (the
//! hybrid `fn:` id with `path`, per-area qualification records, upstream
//! tag labels) loads unchanged. Each has 1 root, 1 index and a `review.md`
//! of 6 entries (one of each kind but `other`).

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
        match dir.file_name().and_then(|n| n.to_str()) {
            Some("v1") => assert_eq!(doc.migrated.len(), 2, "two call-graph keys migrate"),
            _ => assert!(doc.migrated.is_empty(), "{}: {:?}", dir.display(), doc.migrated),
        }
        assert!(doc.reviews().all(|r| kovan_common::review::id::is_fn_id(&r.function_id())));
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
