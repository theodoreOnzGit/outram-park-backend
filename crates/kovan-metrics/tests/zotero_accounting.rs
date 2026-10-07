//! V&V for `kovan_metrics::zotero` (GitHub #751).
//!
//! **Methodology.** Not a port, so there is no upstream test to reproduce.
//! Two fixtures:
//!
//! 1. kovan-common's copy of Zotero's own `test/tests/data/itemJSON.js`
//!    (`crates/kovan-common/tests/data/zotero/itemJSON.json`: one item of
//!    each of the 37 regular item types with every field filled), imported
//!    whole. The expected losses of the `journalArticle` were written down
//!    from kovan-common's documented field map (`zotero::kovan` module docs)
//!    **before the first run**, and are asserted exactly.
//! 2. A hand-built library exercising every count (collections with a
//!    subcollection, manual/automatic tags, child attachments and
//!    annotations, a nested export-format attachment, the trash, relations),
//!    with every expected count written down before the first run.
//!
//! **Results (2026-10-07, `cargo test --release -p kovan-metrics`).** All
//! seven tests passed on the first run; every prediction held as written and
//! nothing was adjusted afterwards. On the itemJSON fixture the report lists
//! 37 imported, 0 skipped and 839 loss entries over 105 properties. The
//! largest: `accessDate`, `dateAdded`, `dateModified`, `rights`,
//! `shortTitle`, `version` dropped on all 37 items; `language` on 36;
//! `date` reduced to its year on 34 (the other three types have no `date`
//! but `dateDecided`/`dateEnacted`/`issueDate`, likewise reduced);
//! `itemType` changed on 32 (only the five types kovan maps one-to-one
//! survive: journal article, report, standard, thesis, and `document` for
//! `Other`); creators dropped on 36 items and retyped on 14 (non-primary
//! creators are dropped, primary ones of a type that does not come back are
//! retyped to the new type's primary creator).

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::zotero::{
    AnnotationData, AnnotationType, AttachmentData, Creator, CreatorType, Field, ItemType,
    LinkMode, Tag, ZoteroCollection, ZoteroItem, ZoteroLibrary,
};
use kovan_common::DocumentType;
use kovan_metrics::zotero::{
    import_library, CollectionCount, LibraryCounts, LossKind, SkipReason, TagCount,
};

fn item_json_library() -> ZoteroLibrary {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../kovan-common/tests/data/zotero/itemJSON.json"
    );
    let text = std::fs::read_to_string(p).expect("itemJSON.json");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let items = v
        .as_object()
        .unwrap()
        .values()
        .map(|i| ZoteroItem::from_json_value(i).unwrap())
        .collect();
    ZoteroLibrary {
        collections: Vec::new(),
        items,
        ..Default::default()
    }
}

/// Every one of the 37 regular items is imported, nothing is skipped, and
/// each item type is counted once.
#[test]
fn item_json_imports_every_regular_type() {
    let lib = item_json_library();
    let c = LibraryCounts::of(&lib);
    assert_eq!(c.items, 37);
    assert_eq!(c.top_level, 37);
    assert_eq!(c.by_item_type.len(), 37);
    assert!(c.by_item_type.values().all(|&n| n == 1));
    let (docs, r) = import_library(&lib);
    assert_eq!(r.considered, 37);
    assert_eq!(r.imported.len(), 37);
    assert_eq!(docs.len(), 37);
    assert!(r.skipped.is_empty());
    let keys: Vec<&str> = r.imported.iter().map(|i| i.key.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "imported is sorted by key");
    assert!(docs.iter().zip(&r.imported).all(|(d, i)| d.id == i.id));
}

/// The journal article's losses, exactly as predicted from kovan-common's
/// field map: 23 properties dropped, `date` reduced to its year, and the
/// four non-primary creators dropped. Title, DOI, abstract, publication
/// title, publisher, volume, pages, issue, URL, citation key, the item type
/// and both primary creators survive.
#[test]
fn journal_article_losses_match_the_documented_field_map() {
    let (_, r) = import_library(&item_json_library());
    let ja: Vec<_> = r.losses.iter().filter(|l| l.key == "UXSSZ972").collect();
    let dropped: BTreeSet<&str> = ja
        .iter()
        .filter(|l| l.kind == LossKind::Dropped && l.property != "creators")
        .map(|l| l.property.as_str())
        .collect();
    let expected: BTreeSet<&str> = [
        "ISSN",
        "PMCID",
        "PMID",
        "accessDate",
        "archive",
        "archiveLocation",
        "callNumber",
        "extra",
        "journalAbbreviation",
        "language",
        "libraryCatalog",
        "partNumber",
        "partTitle",
        "place",
        "rights",
        "section",
        "series",
        "seriesText",
        "seriesTitle",
        "shortTitle",
        "dateAdded",
        "dateModified",
        "version",
    ]
    .into_iter()
    .collect();
    assert_eq!(dropped, expected);
    let altered: Vec<(&str, &LossKind)> = ja
        .iter()
        .filter(|l| matches!(l.kind, LossKind::Altered { .. }))
        .map(|l| (l.property.as_str(), &l.kind))
        .collect();
    assert_eq!(
        altered,
        vec![(
            "date",
            &LossKind::Altered {
                kovan: "1999".into()
            }
        )]
    );
    let creators: BTreeSet<&str> = ja
        .iter()
        .filter(|l| l.property == "creators")
        .map(|l| {
            assert_eq!(l.kind, LossKind::Dropped);
            l.original.split(':').next().unwrap()
        })
        .collect();
    assert_eq!(
        creators,
        ["contributor", "editor", "reviewedAuthor", "translator"]
            .into_iter()
            .collect()
    );
    assert_eq!(ja.len(), 23 + 1 + 4);
    assert!(!ja.iter().any(|l| l.kind == LossKind::MovedToExtra));
}

/// A `webpage` maps to kovan's `Other`, which comes back as a `document`:
/// the item type is reported altered.
#[test]
fn webpage_item_type_is_altered() {
    let (_, r) = import_library(&item_json_library());
    let wp = r
        .imported
        .iter()
        .find(|i| i.item_type == "webpage")
        .unwrap();
    assert_eq!(wp.document_type, DocumentType::Other);
    let l = r
        .losses
        .iter()
        .find(|l| l.key == wp.key && l.property == "itemType")
        .unwrap();
    assert_eq!(l.original, "webpage");
    assert_eq!(
        l.kind,
        LossKind::Altered {
            kovan: "document".into()
        }
    );
}

fn item(key: &str, t: ItemType) -> ZoteroItem {
    let mut i = ZoteroItem::new(t);
    i.key = Some(key.into());
    i
}

fn hand_built() -> ZoteroLibrary {
    let mut parent = ZoteroCollection::new("Parent");
    parent.key = Some("COLLPARE".into());
    let mut child = ZoteroCollection::new("Child");
    child.key = Some("COLLCHIL".into());
    child.parent_collection = Some("COLLPARE".into());

    let mut a = item("AAAAAAAA", ItemType::JournalArticle);
    a.set_field(Field::Title, "A | piped title");
    a.creators
        .push(Creator::person(CreatorType::Author, "Ada", "Lovelace"));
    a.collections.push("COLLCHIL".into());
    a.tags.push(Tag {
        tag: "x".into(),
        tag_type: None,
    });
    a.tags.push(Tag {
        tag: "y".into(),
        tag_type: Some(1),
    });
    a.relations.insert(
        "dc:relation".into(),
        vec![
            "http://zotero.org/users/1/items/BBBBBBBB".into(),
            "http://zotero.org/users/1/items/FFFFFFFF".into(),
        ],
    );

    let mut b = item("BBBBBBBB", ItemType::Book);
    b.set_field(Field::Title, "B");
    b.tags.push(Tag {
        tag: "x".into(),
        tag_type: Some(0),
    });
    let mut nested = ZoteroItem::new(ItemType::Attachment);
    nested.attachment = Some(AttachmentData {
        link_mode: Some(LinkMode::ImportedUrl),
        ..AttachmentData::default()
    });
    b.attachments.push(nested);

    let mut c = item("CCCCCCCC", ItemType::Attachment);
    c.parent_item = Some("AAAAAAAA".into());
    c.attachment = Some(AttachmentData {
        link_mode: Some(LinkMode::LinkedFile),
        path: Some("/tmp/a.pdf".into()),
        ..AttachmentData::default()
    });
    let mut d = item("DDDDDDDD", ItemType::Annotation);
    d.parent_item = Some("CCCCCCCC".into());
    d.annotation = Some(AnnotationData {
        annotation_type: Some(AnnotationType::Highlight),
        ..AnnotationData::default()
    });
    let mut e = item("EEEEEEEE", ItemType::Note);
    e.note = Some("<p>standalone</p>".into());
    let mut f = item("FFFFFFFF", ItemType::Book);
    f.deleted = Some(true);
    f.collections.push("COLLPARE".into());

    ZoteroLibrary {
        collections: vec![child, parent],
        items: vec![a, b, c, d, e, f],
        ..Default::default()
    }
}

/// Every count of the hand-built library, as predicted before the run.
#[test]
fn hand_built_library_counts() {
    let c = LibraryCounts::of(&hand_built());
    assert_eq!(c.items, 7);
    assert_eq!(c.top_level, 4);
    assert_eq!(c.children, 3);
    assert_eq!(c.trashed, 1);
    assert_eq!(c.unfiled, 1);
    let types: BTreeMap<&str, usize> = c
        .by_item_type
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    assert_eq!(
        types,
        [
            ("annotation", 1),
            ("attachment", 2),
            ("book", 2),
            ("journalArticle", 1),
            ("note", 1)
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        c.tags.get("x"),
        Some(&TagCount {
            manual: 2,
            automatic: 0
        })
    );
    assert_eq!(
        c.tags.get("y"),
        Some(&TagCount {
            manual: 0,
            automatic: 1
        })
    );
    assert_eq!(c.link_modes.get("linked_file"), Some(&1));
    assert_eq!(c.link_modes.get("imported_url"), Some(&1));
    assert_eq!(c.annotation_types.get("highlight"), Some(&1));
    assert_eq!(c.relation_predicates.get("dc:relation"), Some(&2));
    assert_eq!(c.saved_searches, 0);
    assert_eq!(
        c.collections,
        vec![
            CollectionCount {
                key: "COLLPARE".into(),
                name: "Parent".into(),
                path: "Parent".into(),
                direct: 1,
                recursive: 2,
                parent_missing: false,
            },
            CollectionCount {
                key: "COLLCHIL".into(),
                name: "Child".into(),
                path: "Parent / Child".into(),
                direct: 1,
                recursive: 1,
                parent_missing: false,
            },
        ]
    );
}

/// What is imported and skipped from the hand-built library, and the
/// losses that only exist outside kovan's fields (collections, relations,
/// a nested attachment).
#[test]
fn hand_built_library_import() {
    let (docs, r) = import_library(&hand_built());
    assert_eq!(r.considered, 6);
    let imported: Vec<&str> = r.imported.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(imported, vec!["AAAAAAAA", "BBBBBBBB"]);
    assert_eq!(docs.len(), 2);
    let skipped: Vec<(&str, &SkipReason)> = r
        .skipped
        .iter()
        .map(|s| (s.key.as_deref().unwrap(), &s.reason))
        .collect();
    assert_eq!(
        skipped,
        vec![
            (
                "CCCCCCCC",
                &SkipReason::ChildItem {
                    parent: "AAAAAAAA".into()
                }
            ),
            (
                "DDDDDDDD",
                &SkipReason::ChildItem {
                    parent: "CCCCCCCC".into()
                }
            ),
            ("EEEEEEEE", &SkipReason::NotRegular),
            ("FFFFFFFF", &SkipReason::Trashed),
        ]
    );
    let has = |k: &str, p: &str| {
        r.losses
            .iter()
            .any(|l| l.key == k && l.property == p && l.kind == LossKind::Dropped)
    };
    assert!(has("AAAAAAAA", "collections"));
    assert!(has("AAAAAAAA", "relations"));
    assert!(has("BBBBBBBB", "attachments"));
    // Tags survive (manual -> tags, automatic -> keywords).
    assert!(!r.losses.iter().any(|l| l.property == "tags"));
    // Title and the primary creator survive.
    assert!(!r
        .losses
        .iter()
        .any(|l| l.property == "title" || l.property == "creators"));
    let by = r.losses_by_property();
    assert_eq!(by.get("collections"), Some(&[1, 0, 0]));
}

/// Duplicate and missing keys are skipped, the first occurrence wins.
#[test]
fn duplicate_and_missing_keys_are_skipped() {
    let mut first = item("KKKKKKKK", ItemType::Book);
    first.set_field(Field::Title, "first");
    let mut second = item("KKKKKKKK", ItemType::Book);
    second.set_field(Field::Title, "second");
    let keyless = ZoteroItem::new(ItemType::Book);
    let lib = ZoteroLibrary {
        collections: Vec::new(),
        items: vec![first, keyless, second],
        ..Default::default()
    };
    let (docs, r) = import_library(&lib);
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].title, "first");
    let reasons: Vec<&SkipReason> = r.skipped.iter().map(|s| &s.reason).collect();
    assert_eq!(
        reasons,
        vec![&SkipReason::DuplicateKey, &SkipReason::MissingKey]
    );
}

/// The Markdown is deterministic (same library, items in reverse order,
/// same text) and escapes pipes.
#[test]
fn markdown_is_deterministic_and_escaped() {
    let lib = hand_built();
    let mut rev = lib.clone();
    rev.items.reverse();
    rev.collections.reverse();
    let (_, r1) = import_library(&lib);
    let (_, r2) = import_library(&rev);
    assert_eq!(r1.to_markdown(), r2.to_markdown());
    assert_eq!(
        LibraryCounts::of(&lib).to_markdown(),
        LibraryCounts::of(&rev).to_markdown()
    );
    let md = r1.to_markdown();
    assert!(md.starts_with("# Zotero import report\n"));
    assert!(md.contains("A \\| piped title"));
    assert!(md.contains("## Lossy, by property (items)"));
    let counts = LibraryCounts::of(&lib).to_markdown();
    assert!(counts.contains("| Parent / Child | COLLCHIL | 1 | 1 |"));
}
