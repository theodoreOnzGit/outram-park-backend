//! Tests of [`super`]: the tracked / derived split (GitHub #1777).

use super::*;
use crate::review::index::{CachedReview, ModuleIndex};

fn h(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn func(id: &str, qual: &str, lines: [u32; 2], hash: char) -> FunctionIndex {
    FunctionIndex {
        id: id.into(),
        name: qual.into(),
        qual: qual.into(),
        item: Default::default(),
        shape: None,
        lines,
        hash: h(hash),
        doc_hash: h('d'),
        callees: vec!["fn:00000000000000c1".into()],
        uses: vec!["fn:00000000000000u1".into()],
        reached_by: vec![format!("crates/k/tests/t.rs::reaches_{qual}")],
        test: false,
        index_out_of_date: false,
        physical_interface: false,
    }
}

/// Two functions in source order (`zeta` above `alpha`), `zeta` reviewed.
fn sample() -> FolderIndex {
    let mut idx = FolderIndex::new("k", "crates/k/src");
    idx.modules.insert(
        "a.rs".into(),
        ModuleIndex {
            path: "crate::a".into(),
            functions: vec![
                func("fn:000000000000000a", "zeta", [3, 9], 'a'),
                func("fn:000000000000000b", "alpha", [11, 20], 'b'),
            ],
        },
    );
    idx.reviews.push(CachedReview {
        function: "fn:000000000000000a".into(),
        by: "github:someone".into(),
        artifact: "review-1".into(),
    });
    idx
}

/// Methodology: split a whole index, check what each file holds, and merge
/// the two back. Pass: the tracked text has no `lines`, `callees` or `uses`
/// and `reached_by` only for the reviewed function; the merge gives back
/// the original exactly.
///
/// Result (2026-11-26): passes.
#[test]
fn split_then_merge_gives_back_the_index() {
    let idx = sample();
    let t = tracked_text(&idx).unwrap();
    for field in ["lines", "callees", "uses", "index_out_of_date"] {
        assert!(!t.contains(&format!("{field} =")), "{field} in tracked:\n{t}");
    }
    assert!(t.contains("reaches_zeta"), "the reviewed function keeps reached_by");
    assert!(!t.contains("reaches_alpha"), "an unreviewed one does not");
    // (qual, id) order in the tracked file: alpha before zeta.
    assert!(t.find("qual = \"alpha\"").unwrap() < t.find("qual = \"zeta\"").unwrap());
    let back = merge(
        FolderIndex::parse(&t).unwrap(),
        Some(FolderIndex::parse(&derived_text(&idx).unwrap()).unwrap()),
    );
    let mut want = idx.clone();
    want.normalise();
    assert_eq!(back, want);
}

/// Methodology: shift every function down 5 lines (an edit above them),
/// then edit one function's body (its hash). Pass: the shift changes the
/// cache and not the tracked text; the body edit changes the tracked text.
///
/// Result (2026-11-26): passes.
#[test]
fn moving_code_changes_the_cache_and_editing_it_changes_git() {
    let idx = sample();
    let mut shifted = idx.clone();
    for m in shifted.modules.values_mut() {
        for f in &mut m.functions {
            f.lines = [f.lines[0] + 5, f.lines[1] + 5];
            f.callees.push("fn:00000000000000c2".into());
        }
    }
    assert_eq!(tracked_text(&shifted).unwrap(), tracked_text(&idx).unwrap());
    assert_ne!(derived_text(&shifted).unwrap(), derived_text(&idx).unwrap());
    let mut edited = idx.clone();
    edited.modules.get_mut("a.rs").unwrap().functions[1].hash = h('e');
    assert_ne!(tracked_text(&edited).unwrap(), tracked_text(&idx).unwrap());
}

/// Methodology: merge a tracked file with no cache, and with a cache whose
/// entry has another hash. Pass: both mark the function out of date (not
/// silently empty), lines unknown.
///
/// Result (2026-11-26): passes.
#[test]
fn no_matching_cache_marks_the_function_out_of_date() {
    let idx = sample();
    let t = FolderIndex::parse(&tracked_text(&idx).unwrap()).unwrap();
    let alone = merge(t.clone(), None);
    assert!(alone.functions().all(|(_, f)| f.index_out_of_date && f.lines == [0, 0]));
    let mut stale = idx.clone();
    stale.modules.get_mut("a.rs").unwrap().functions[1].hash = h('x');
    let half = merge(t, Some(stale));
    let by_qual: BTreeMap<&str, &FunctionIndex> = half.functions().map(|(_, f)| (f.qual.as_str(), f)).collect();
    assert!(!by_qual["zeta"].index_out_of_date && by_qual["zeta"].lines == [3, 9]);
    assert!(by_qual["alpha"].index_out_of_date, "a cache from older code is not trusted");
}

/// Methodology: a `kovan.toml` written before the split (derived fields
/// inline), read with no cache. Pass: it loads with every field as written,
/// nothing marked out of date (schemas never break).
///
/// Result (2026-11-26): passes.
#[test]
fn a_file_from_before_the_split_still_loads() {
    let idx = sample();
    let old = idx.to_toml().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let loaded = load(dir.path(), &old).unwrap();
    let mut want = idx.clone();
    want.normalise();
    assert_eq!(loaded, want);
}

/// Methodology: [`write`] into a temporary workspace twice, then once more
/// after shifting lines. Pass: the first write creates both files; the
/// second writes neither; the shift rewrites the cache only; [`load`]
/// reads back the whole index each time.
///
/// Result (2026-11-26): passes.
#[test]
fn writing_touches_the_tracked_file_only_when_it_changes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("crates/k/src")).unwrap();
    let rel = "crates/k/src/kovan.toml";
    let idx = sample();
    assert!(write(root, rel, &idx).unwrap());
    assert!(root.join(derived_rel("crates/k/src")).is_file());
    let ignore = std::fs::read_to_string(root.join(DERIVED_DIR).join(".gitignore")).unwrap();
    assert_eq!(ignore, "*\n", "the cache ignores itself in any repository");
    assert!(!write(root, rel, &idx).unwrap());
    assert!(!write_cache(root, &idx).unwrap());
    let mut shifted = idx.clone();
    for m in shifted.modules.values_mut() {
        for f in &mut m.functions {
            f.lines = [f.lines[0] + 1, f.lines[1] + 1];
        }
    }
    let before = std::fs::read_to_string(root.join(rel)).unwrap();
    assert!(!write(root, rel, &shifted).unwrap(), "a shift leaves git alone");
    assert_eq!(std::fs::read_to_string(root.join(rel)).unwrap(), before);
    let loaded = load(root, &before).unwrap();
    let mut want = shifted;
    want.normalise();
    assert_eq!(loaded, want);
    assert_eq!(derived_rel(""), "target/kovan/derived/kovan.derived.toml");
}

/// Methodology: a folder whose tracked `kovan.toml` lists no review, then a
/// `review.md` naming `alpha` is written and [`sync_reviews`] run. Pass:
/// the tracked file now carries alpha's `reached_by` (from the cache) and
/// its `[[review]]`; a folder without a code index is left alone.
///
/// Result (2026-11-26): passes.
#[test]
fn a_new_review_puts_its_reaching_tests_into_git() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("crates/k/src")).unwrap();
    let rel = "crates/k/src/kovan.toml";
    let mut idx = sample();
    idx.reviews.clear();
    write(root, rel, &idx).unwrap();
    assert!(!std::fs::read_to_string(root.join(rel)).unwrap().contains("reaches_alpha"));
    let md = review_md_naming("fn:000000000000000b");
    let md = md.as_str();
    let doc = crate::review::review_md::parse_review_md(md);
    assert_eq!(doc.reviews().count(), 1, "fixture parses as one review: {:?}", doc.unreadable);
    assert!(sync_reviews(root, "crates/k/src", md).unwrap());
    let t = std::fs::read_to_string(root.join(rel)).unwrap();
    assert!(t.contains("reaches_alpha") && t.contains("review-2"), "{t}");
    assert!(!sync_reviews(root, "crates/none", md).unwrap());
}

/// A `review.md` holding one review of `target`, written by the real writer.
fn review_md_naming(target: &str) -> String {
    use crate::review::review_md::{EntryMeta, ReviewBody, ReviewEntry};
    let entry = ReviewEntry {
        kovan: EntryMeta {
            id: "review-2".into(),
            kind: "review".into(),
            origin: Some("human".into()),
            created: "2026-11-26T10:00:00+08:00".into(),
            modified: "2026-11-26T10:00:00+08:00".into(),
            target: Some(target.into()),
        },
        review: ReviewBody {
            path: Some("crates/k/src/a.rs::alpha".into()),
            by: "github:someone".into(),
            rung: 3,
            date: "2026-11-26".into(),
            signed_at: None,
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            hash: h('b'),
            doc_hash: h('d'),
            cargo_lock: Some(h('c')),
            callees: BTreeMap::new(),
            checklist: BTreeMap::new(),
            no_concept: None,
            authorship: None,
            moved: vec![],
            separation_attestation: None,
            signature: None,
        },
        relations: vec![],
    };
    crate::review::review_md_write::upsert_review("", &entry, "Looks right.").unwrap().text
}
