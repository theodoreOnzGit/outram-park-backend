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
//! of 6 entries (one of each kind but `other`). v3 (GitHub #783,
//! 2026-10-07) is v2 plus `signed_at` on the review and the architecture
//! node; v1 and v2 load with `signed_at` absent and still re-emit without
//! it (`signing/signed_at_tests.rs` pins a signed v1 `review.md` byte for
//! byte, and its signatures). v4 (GitHub #809, 2026-10-08) is v3 plus the
//! rung-5 IV&V records: `[[code_review.developing_organisation]]` (one
//! workspace entry, one per-crate override), `[[reviewer.organisation]]`,
//! `[[reviewer.separation]]`, and `separation_attestation` on a review;
//! v1 to v3 load with all of them absent, re-emit without them, and their
//! `review.md` renders byte for byte as before
//! (`old_files_write_back_unchanged`).

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
        // `signed_at` (#783) exists from v3 on, and never appears from nowhere.
        let has_signed_at = doc.reviews().any(|r| r.review.signed_at.is_some())
            && doc.architectures().any(|a| a.architecture.signed_at.is_some());
        let v3_or_later = !matches!(dir.file_name().and_then(|n| n.to_str()), Some("v1" | "v2"));
        assert_eq!(has_signed_at, v3_or_later, "{}", dir.display());
        if !v3_or_later {
            assert!(!render_review_md(&doc.entries).unwrap().contains("signed_at"));
        }
        // The #809 records exist from v4 on, and never appear from nowhere.
        let v4_or_later = !matches!(dir.file_name().and_then(|n| n.to_str()), Some("v1" | "v2" | "v3"));
        let has_ivv = doc.reviews().any(|r| r.review.separation_attestation.is_some())
            && r.code_review.as_ref().is_some_and(|c| c.developing_organisation.len() == 2)
            && r.reviewers.iter().any(|x| !x.organisations.is_empty())
            && r.reviewers.iter().any(|x| !x.separations.is_empty());
        assert_eq!(has_ivv, v4_or_later, "{}", dir.display());
        if !v4_or_later {
            let md_out = render_review_md(&doc.entries).unwrap();
            assert!(!md_out.contains("separation_attestation"));
            let root_out = r.write_into(&root).unwrap();
            for key in ["developing_organisation", "reviewer.organisation", "reviewer.separation"] {
                assert!(!root_out.contains(key), "{}: {key}", dir.display());
            }
        }
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

/// **Old files write back unchanged** (GitHub #809: schemas never break).
///
/// Methodology: the fixtures were written by hand (an inline array where
/// the renderer writes one per line), so the byte check is on files the
/// renderer wrote: for v1, v2 and v3, `review.md` is rendered once, and that
/// rendering must read and render again to exactly the same bytes, with no
/// `separation_attestation` anywhere (the field is absent, so serde skips
/// it). A `review.md` written by the pre-#809 renderer and signed then is
/// pinned byte for byte, with its signatures, by
/// `signing/signed_at_tests.rs` (`old_review_md_loads_unchanged`,
/// `v1_fixture_bytes_identical_and_verified`). And a reader shaped like the
/// pre-#809 schema (no organisation, attestation or
/// `separation_attestation` fields; serde ignores unknown keys) reads the
/// v4 root and review, so an older kovan can still open a newer workspace.
///
/// Result (2026-10-08): passes for v1, v2 and v3; v4 reads in the old
/// shape and is itself a fixed point of the renderer.
#[test]
fn old_files_write_back_unchanged() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review");
    for v in ["v1", "v2", "v3"] {
        let md = std::fs::read_to_string(base.join(v).join("review.md")).unwrap();
        let once = render_review_md(&parse_review_md(&md).entries).unwrap();
        let twice = render_review_md(&parse_review_md(&once).entries).unwrap();
        assert_eq!(twice, once, "{v}: a written file writes back byte for byte");
        assert!(!once.contains("separation_attestation"), "{v}");
    }
    #[derive(serde::Deserialize)]
    struct OldReviewer {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct OldCodeReview {
        rust_analyzer: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct OldRoot {
        code_review: Option<OldCodeReview>,
        reviewer: Vec<OldReviewer>,
    }
    let v4 = std::fs::read_to_string(base.join("v4/kovan_root.toml")).unwrap();
    let old: OldRoot = toml::from_str(&v4).unwrap();
    assert_eq!(old.code_review.unwrap().rust_analyzer.as_deref(), Some("0.3.2645"));
    assert_eq!(old.reviewer.len(), 2);
    assert_eq!(old.reviewer[1].id, "orcid:0000-0002-1825-0097");
    let md = std::fs::read_to_string(base.join("v4/review.md")).unwrap();
    let doc = parse_review_md(&md);
    assert!(doc.unreadable.is_empty());
    let r = doc.reviews().next().unwrap();
    assert_eq!(r.review.separation_attestation.as_deref(), Some("sep-2026-10-08"));
    let once = render_review_md(&doc.entries).unwrap();
    assert!(once.contains("separation_attestation = \"sep-2026-10-08\""));
    assert_eq!(render_review_md(&parse_review_md(&once).entries).unwrap(), once);
}
