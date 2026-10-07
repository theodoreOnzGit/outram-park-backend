//! Desktop kovan reads code-review `review.md` entries (GitHub #764,
//! maintainer 2026-10-07: "add them additively to kovan's closed
//! ArtifactKind list").
//!
//! # Methodology
//!
//! - **New kinds read.** kovan-common's committed v2 `review.md` fixture
//!   (one entry of every code-review kind) is parsed with
//!   [`kovan::artifact::parse_document`]: no problems, and each entry has
//!   the expected [`ArtifactKind`].
//! - **Nothing is lost on a rewrite.** Every entry is re-rendered with
//!   [`kovan::artifact::render_artifact_block`] (what any literature-side
//!   edit does) and the result is read with
//!   `kovan_common::review::review_md::parse_review_md`: the entries must
//!   equal those read from the original file. Before [`ExtraKeys`], kovan
//!   tolerated `[review]` on read and dropped it on write.
//! - **Old data unchanged.** The pre-#764 fixtures are covered by
//!   `tests/schema_743.rs` (byte-for-byte re-render), which must still pass;
//!   here every old kind's wire name is checked to read as before.
//!
//! # Results
//!
//! 2026-10-07: all pass; `schema_743` (8 tests) and `local_library_compat`
//! pass unchanged.
//!
//! [`ExtraKeys`]: kovan::artifact::ExtraKeys

use kovan::artifact::{parse_document, render_artifact_block, ArtifactKind};
use kovan_common::review::review_md::parse_review_md;

fn fixture() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kovan-common/tests/fixtures/review/v2/review.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn review_entries_parse_as_artifacts() {
    let md = fixture();
    let doc = parse_document(&md);
    assert!(doc.problems.is_empty(), "{:?}", doc.problems);
    let kinds: Vec<ArtifactKind> = doc.artifacts.iter().map(|a| a.kind()).collect();
    assert_eq!(
        kinds,
        [
            ArtifactKind::Upstream,
            ArtifactKind::Review,
            ArtifactKind::NeedsFix,
            ArtifactKind::Annotation,
            ArtifactKind::Architecture,
            ArtifactKind::DeletedFunctions,
        ]
    );
    assert!(kinds.iter().filter(|k| k.is_review_kind()).count() == 5);
}

#[test]
fn rewriting_through_kovan_keeps_every_review_field() {
    let md = fixture();
    let before = parse_review_md(&md);
    assert!(before.unreadable.is_empty());
    let doc = parse_document(&md);
    let rewritten: Vec<String> = doc
        .artifacts
        .iter()
        .map(|a| render_artifact_block(a.level, &a.heading, &a.toml, &a.body).unwrap())
        .collect();
    let after = parse_review_md(&rewritten.join("\n"));
    assert!(after.unreadable.is_empty(), "{:?}", after.unreadable);
    let entries = |d: &kovan_common::review::review_md::ReviewDocument| {
        d.entries
            .iter()
            .map(|e| (e.heading.clone(), e.entry.clone(), e.body.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(entries(&after), entries(&before));
}

#[test]
fn old_kind_names_read_as_before() {
    for (i, k) in ArtifactKind::ALL.iter().enumerate().take(13) {
        let md = format!(
            "# A\n\n```toml\n[kovan]\nid = \"a{i}\"\nkind = \"{}\"\ncreated = \"c\"\nmodified = \"m\"\n```\n",
            k.as_str()
        );
        let doc = parse_document(&md);
        assert!(doc.problems.is_empty(), "{}: {:?}", k.as_str(), doc.problems);
        assert_eq!(doc.artifacts[0].kind(), *k);
        assert!(!k.is_review_kind());
        let a = &doc.artifacts[0];
        let again = render_artifact_block(a.level, &a.heading, &a.toml, &a.body).unwrap();
        assert_eq!(again, md, "{} re-renders byte for byte", k.as_str());
    }
}
