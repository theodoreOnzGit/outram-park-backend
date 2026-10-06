//! V&V of the Zotero port in `kovan_common::zotero` (GitHub #748).
//!
//! Every expected value below comes from upstream **data**, never from
//! running upstream code (not yet authorised; code-to-code is #752):
//!
//! * `tests/data/zotero/citeProcJSExport.json` and `journalArticle.json` are
//!   copies of zotero/utilities `test/data/` (commit 4051881d59c6);
//!   `itemJSON.json` is zotero `test/tests/data/itemJSON.js` (commit
//!   9cbba8c4d281, plain JSON despite the extension). All AGPL-3.0, (c)
//!   Corporation for Digital Scholarship; see `NOTICE`.
//! * Schema counts were taken from zotero-schema `schema.json` (commit
//!   b86c79b56479) with an independent Python `json` read on 2026-10-07, not
//!   through the generator under test.
//!
//! Results recorded 2026-10-07 (all with `cargo test --release`): see each
//! test's doc comment.

use kovan_common::zotero::csl::{item_from_csl_json_with, item_to_csl_json_with};
use kovan_common::zotero::date::DateOptions;
use kovan_common::zotero::schema::{field_from_type_and_base, is_valid_for_type, primary_creator_type};
use kovan_common::zotero::schema_generated::{
    CSL_DATE_FIELDS, CSL_NAMES, CSL_TEXT_FIELDS, CSL_TYPES, EN_US_CREATOR_TYPE_LABELS,
    EN_US_FIELD_LABELS, EN_US_ITEM_TYPE_LABELS, ITEM_TYPE_SCHEMAS, META_FIELD_TYPES,
};
use kovan_common::zotero::{CreatorType, Field, ItemType, ZoteroItem, SCHEMA_VERSION};
use kovan_common::{Author, DocumentType, KovanDocument, Visibility};
use serde_json::{Map, Value};

const CITEPROC_EXPORT: &str = include_str!("data/zotero/citeProcJSExport.json");
const ITEM_JSON: &str = include_str!("data/zotero/itemJSON.json");
const JOURNAL_ARTICLE: &str = include_str!("data/zotero/journalArticle.json");

fn opts() -> DateOptions {
    DateOptions {
        current_year: 2026,
        ..DateOptions::default()
    }
}

fn fixture(text: &str) -> Map<String, Value> {
    serde_json::from_str(text).expect("fixture is JSON")
}

/// **Methodology.** Count and spot-check the generated tables against
/// `schema.json` v45, read independently (Python `json`): item types, the
/// field registry (each type's `field` then `baseField`, first sighting
/// wins), creator types, item-type/field pairs, base mappings, CSL tables,
/// en-US labels; plus the field order of `journalArticle`, `book`, `film`,
/// and primary creator types.
///
/// **Pass:** every count and spot value equal.
///
/// **Result (2026-10-07):** pass. 40 item types; 123 fields (121 used as
/// `field` + `authority`, `medium` used only as `baseField`); 37 creator
/// types; 770 type-field pairs of which 72 carry a base field; 37 item
/// types with a primary creator type (all but note, attachment,
/// annotation); CSL 32 types / 47 text / 4 date / 24 names; en-US labels
/// 40 / 126 / 37.
#[test]
fn schema_tables_match_schema_json() {
    assert_eq!(SCHEMA_VERSION, 45);
    assert_eq!(ItemType::ALL.len(), 40);
    assert_eq!(Field::ALL.len(), 123);
    assert_eq!(CreatorType::ALL.len(), 37);
    let pairs: usize = ITEM_TYPE_SCHEMAS.iter().map(|s| s.fields.len()).sum();
    let mapped: usize = ITEM_TYPE_SCHEMAS
        .iter()
        .map(|s| s.fields.iter().filter(|f| f.base_field.is_some()).count())
        .sum();
    assert_eq!((pairs, mapped), (770, 72));
    assert_eq!(
        ITEM_TYPE_SCHEMAS
            .iter()
            .filter(|s| s.primary_creator_type.is_some())
            .count(),
        37
    );
    assert_eq!(
        (
            CSL_TYPES.len(),
            CSL_TEXT_FIELDS.len(),
            CSL_DATE_FIELDS.len(),
            CSL_NAMES.len()
        ),
        (32, 47, 4, 24)
    );
    assert_eq!(
        (
            EN_US_ITEM_TYPE_LABELS.len(),
            EN_US_FIELD_LABELS.len(),
            EN_US_CREATOR_TYPE_LABELS.len()
        ),
        (40, 126, 37)
    );
    assert_eq!(META_FIELD_TYPES.len(), 3);

    let ja: Vec<&str> = ItemType::JournalArticle
        .fields()
        .map(Field::as_str)
        .collect();
    assert_eq!(ja.len(), 31);
    assert_eq!(
        &ja[..6],
        [
            "title",
            "abstractNote",
            "publicationTitle",
            "publisher",
            "place",
            "date"
        ]
    );
    let book: Vec<&str> = ItemType::Book.fields().map(Field::as_str).collect();
    assert_eq!(book.len(), 29);
    assert_eq!(
        &book[..6],
        [
            "title",
            "abstractNote",
            "series",
            "seriesNumber",
            "volume",
            "numberOfVolumes"
        ]
    );
    let film: Vec<&str> = ItemType::Film.fields().map(Field::as_str).collect();
    assert_eq!(
        (film.len(), &film[..3]),
        (20, &["title", "abstractNote", "distributor"][..])
    );
    assert_eq!(ItemType::Note.fields().count(), 0);
    assert_eq!(
        primary_creator_type(ItemType::Film),
        Some(CreatorType::Director)
    );
    assert_eq!(
        primary_creator_type(ItemType::Book),
        Some(CreatorType::Author)
    );
    assert_eq!(primary_creator_type(ItemType::Note), None);
    // webpage: websiteTitle -> publicationTitle, websiteType -> type.
    assert_eq!(
        field_from_type_and_base(ItemType::Webpage, Field::PublicationTitle),
        Some(Field::WebsiteTitle)
    );
    assert_eq!(
        field_from_type_and_base(ItemType::Webpage, Field::Type),
        Some(Field::WebsiteType)
    );
    assert!(
        Field::Authority.is_base_field()
            && !ItemType::ALL
                .iter()
                .any(|t| is_valid_for_type(Field::Authority, *t))
    );
    // en-US labels.
    assert_eq!(Field::PublicationTitle.label(), "Publication");
    assert_eq!(ItemType::JournalArticle.label(), "Journal Article");
    assert_eq!(CreatorType::BookAuthor.label(), "Book Author");
    // CSL.
    assert_eq!(ItemType::Film.csl_type(), Some("motion_picture"));
    assert_eq!(ItemType::Note.csl_type(), Some("document"));
    assert_eq!(ItemType::Annotation.csl_type(), None);
    assert_eq!(
        CreatorType::SeriesEditor.csl_name(),
        Some("collection-editor")
    );
}

/// **Methodology.** Every item of zotero's `itemJSON.js` (one per regular
/// item type, every field filled, generated by Zotero's own `toJSON`; see
/// `supportTest.js:94`) is parsed into [`ZoteroItem`], checked against the
/// schema, and written back.
///
/// **Pass:** written JSON equals the fixture value-for-value
/// (`serde_json::Value` equality), and `validate()` reports nothing.
///
/// **Result (2026-10-07):** pass for all 37 items.
#[test]
fn item_json_round_trips_and_validates() {
    let items = fixture(ITEM_JSON);
    assert_eq!(items.len(), 37);
    for (name, json) in &items {
        let item = ZoteroItem::from_json_value(json).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(item.item_type.as_str(), name.as_str());
        assert_eq!(&item.to_json_value(), json, "{name}: JSON round trip");
        assert!(item.validate().is_empty(), "{name}: {:?}", item.validate());
        // And through serde.
        let s = serde_json::to_string(&item).unwrap();
        let back: ZoteroItem = serde_json::from_str(&s).unwrap();
        assert_eq!(back, item, "{name}: serde round trip");
    }
}

/// **Methodology.** utilities `utilities_itemTest.js:43-65`, "should stably
/// perform itemToCSLJSON -> itemFromCSLJSON -> itemToCSLJSON": each CSL item
/// of `citeProcJSExport.json` is imported and exported again, `id` removed on
/// both sides, and `collection-title` removed from `podcast` beforehand
/// (upstream's own TEMP exclusion, zotero issue #1667).
///
/// **Pass:** exported CSL equals the fixture (deep JSON equality).
///
/// **Result (2026-10-07):** pass for all 37 CSL items.
#[test]
fn csl_export_import_export_is_stable() {
    let data = fixture(CITEPROC_EXPORT);
    assert_eq!(data.len(), 37);
    for (name, json) in &data {
        let mut json = json.as_object().unwrap().clone();
        if name == "podcast" {
            json.remove("collection-title");
        }
        let item =
            item_from_csl_json_with(&json, &opts()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut out = item_to_csl_json_with(&item, &opts()).unwrap();
        out.remove("id");
        json.remove("id");
        assert_eq!(
            Value::Object(out),
            Value::Object(json),
            "{name}: export -> import -> export"
        );
    }
}

/// **Methodology.** utilities_itemTest.js:66-76: the legacy `shortTitle`
/// key imports, and the export writes only the canonical keys.
///
/// **Result (2026-10-07):** pass.
#[test]
fn csl_legacy_short_title_key() {
    let data = fixture(CITEPROC_EXPORT);
    let mut json = data["artwork"].as_object().unwrap().clone();
    let mut canonical: Vec<String> = json.keys().cloned().collect();
    let short = json.remove("title-short").unwrap();
    json.insert("shortTitle".into(), short);
    let item = item_from_csl_json_with(&json, &opts()).unwrap();
    let out = item_to_csl_json_with(&item, &opts()).unwrap();
    let mut keys: Vec<String> = out.keys().cloned().collect();
    // The fixture's `id` is not reproduced (no item URI).
    canonical.retain(|k| k != "id");
    canonical.sort();
    keys.sort();
    assert_eq!(keys, canonical);
}

/// **Methodology.** The Zotero item JSON of each regular type
/// (`itemJSON.js`) exported to CSL and compared with the CSL fixture of the
/// same type (`citeProcJSExport.json`). Upstream generated both from the same
/// sample data, but the check that tied them together is commented out
/// upstream (`supportTest.js:121-147`), so this is a consistency measurement,
/// not an upstream-asserted equality. `id` is ignored (no URI in item JSON).
///
/// **Prediction written before the first run:** at most 2 of 37 types
/// differ. **Refuted (2026-10-07): all 37 differ, in `issued` (and the
/// patent also in `submitted`, its `filingDate`) only.** The
/// item JSON's date is `1999-12-31`; today's `itemToCSLJSON` tries
/// `parseEDTF` first (utilities_item.js:321), which accepts that string and
/// gives integer date parts `[1999, 12, 31]`, while the fixture holds
/// `["1999", 12, 31]`, the year-as-string that the older `strToDate`-only
/// path produced (still produced today for non-EDTF strings such as
/// "December 31, 1999", which is why the export -> import -> export test
/// passes). The fixture predates the EDTF change; the upstream test that
/// would have caught it is commented out. To be confirmed by running
/// upstream (#752).
///
/// **Pass (revised to the explained difference, not loosened):** for every
/// type, the only differing key is `issued`, and it is equal once the
/// fixture's string year is read as an integer.
///
/// **Result (2026-10-07):** pass, 37/37.
#[test]
fn item_json_to_csl_matches_csl_fixture() {
    let items = fixture(ITEM_JSON);
    let csl = fixture(CITEPROC_EXPORT);
    let mut issued_only = 0;
    for (name, json) in &items {
        let item = ZoteroItem::from_json_value(json).unwrap();
        let mut out = item_to_csl_json_with(&item, &opts()).unwrap();
        out.remove("id");
        let mut want = csl[name].as_object().unwrap().clone();
        want.remove("id");
        let mut diff: Vec<String> = Vec::new();
        for k in want.keys().chain(out.keys()) {
            if want.get(k) != out.get(k) && !diff.contains(k) {
                diff.push(k.clone());
            }
        }
        for k in &diff {
            // Only EDTF-parsed date variables may differ (issued; submitted
            // for the patent's filingDate).
            assert!(
                ["issued", "submitted"].contains(&k.as_str()),
                "{name}: {diff:?}"
            );
            let mut fixed = want[k].clone();
            let year = &mut fixed["date-parts"][0][0];
            *year = serde_json::json!(year.as_str().unwrap().parse::<i64>().unwrap());
            assert_eq!(out[k], fixed, "{name}: {k}");
        }
        if diff.iter().any(|k| k == "issued") {
            issued_only += 1;
        }
        if diff.iter().any(|k| k == "submitted") {
            assert_eq!(name, "patent");
        }
    }
    assert_eq!(issued_only, 37);
}

/// **Methodology.** `journalArticle.json` (utilities test data) has numeric
/// `issue` and `volume`; `fromJSON` stores numbers as strings
/// (item.js:779). Its CSL export is compared with the hand-derivable
/// values.
///
/// **Result (2026-10-07):** pass.
#[test]
fn numeric_fields_become_strings() {
    let data = fixture(JOURNAL_ARTICLE);
    let item = ZoteroItem::from_json_value(&data["journalArticle"]).unwrap();
    assert_eq!(item.field(Field::Issue), Some("5"));
    assert_eq!(item.field(Field::Volume), Some("6"));
    let csl = item_to_csl_json_with(&item, &opts()).unwrap();
    assert_eq!(csl["issue"], "5");
    assert_eq!(csl["volume"], "6");
    assert_eq!(csl["type"], "article-journal");
    assert_eq!(csl["container-title"], "Publication title");
    // "1999-12-31" parses as EDTF: integer parts (see the test above).
    assert_eq!(
        csl["issued"],
        serde_json::json!({"date-parts": [[1999, 12, 31]]})
    );
    assert_eq!(
        csl["accessed"],
        serde_json::json!({"date-parts": [["1997", 6, 13]]})
    );
}

/// Child items in the shapes upstream's own tests use: an annotation
/// (itemTest.js:3420-3440, 2629-2660), a linked-file attachment
/// (itemTest.js:3388-3398), a collection (collection.js:826).
///
/// **Result (2026-10-07):** pass.
#[test]
fn child_items_and_collections() {
    let ann = serde_json::json!({
        "itemType": "annotation",
        "parentItem": "ABCD2345",
        "annotationType": "highlight",
        "annotationText": "This is highlighted text.",
        "annotationComment": "This is a comment with <i>rich-text</i>\nAnd a new line",
        "annotationColor": "#ffec00",
        "annotationPageLabel": "15",
        "annotationSortIndex": "00015|002431|00000",
        "annotationPosition": "{\"pageIndex\":123,\"rects\":[[314.4,412.8,556.2,609.6]]}",
        "tags": [{"tag": "tagA"}],
        "relations": {"mendeleyDB:annotationUUID": ["13e4ec18-f49a-47fb-93f6-fda915d3a1c2"]}
    });
    let item = ZoteroItem::from_json_value(&ann).unwrap();
    let a = item.annotation.as_ref().unwrap();
    assert_eq!(a.annotation_type.map(|t| t.as_str()), Some("highlight"));
    assert_eq!(a.sort_index.as_deref(), Some("00015|002431|00000"));
    assert!(item.validate().is_empty());
    assert_eq!(item.to_json_value(), ann);

    let att = serde_json::json!({
        "itemType": "attachment", "linkMode": "linked_file", "contentType": "text/plain",
        "charset": "utf-8", "path": "attachments:test.txt", "title": "test.txt",
        "parentItem": "ABCD2345", "tags": [], "relations": {}
    });
    let item = ZoteroItem::from_json_value(&att).unwrap();
    assert_eq!(
        item.attachment.as_ref().unwrap().path.as_deref(),
        Some("attachments:test.txt")
    );
    assert_eq!(item.to_json_value(), att);
    assert!(ZoteroItem::from_json_value(
        &serde_json::json!({"itemType": "attachment", "linkMode": "nope"})
    )
    .is_err());
    assert!(ZoteroItem::from_json_value(&serde_json::json!({"itemType": "foo"})).is_err());

    let col = serde_json::json!({"key": "COLL1234", "version": 3, "name": "Reactors", "parentCollection": false, "relations": {}});
    let c = kovan_common::zotero::ZoteroCollection::from_json_value(&col).unwrap();
    assert_eq!(c.parent_collection, None);
    assert_eq!(c.to_json_value(), col);
}

fn kovan_doc() -> KovanDocument {
    KovanDocument::builder(
        "kovan-7",
        "smith2021",
        Visibility::Open,
        DocumentType::Paper,
        "Decay heat in pebble beds",
    )
    .author(Author {
        family: "Smith".into(),
        given: "Jane".into(),
        affiliation: Some("NUS".into()),
    })
    .author(Author {
        family: "OECD/NEA".into(),
        given: "".into(),
        affiliation: None,
    })
    .abstract_text("An abstract.")
    .year(2021)
    .doi("10.1016/j.nucengdes.2021.111111")
    .journal("Nuclear Engineering and Design")
    .publisher("Elsevier")
    .volume("380")
    .pages("111111")
    .number("3")
    .keywords(vec!["decay heat".into()])
    .tags(vec!["htgr".into()])
    .source_url("https://example.org/paper")
    .source_path("open/papers/smith2021.pdf")
    .source_sha256("00")
    .page_count(12)
    .assets(vec!["img.png".into()])
    .related_symbols(vec!["sym".into()])
    .markdown_body("# body")
    .build()
}

/// **Methodology.** kovan -> Zotero -> kovan on a fully-populated `Paper`,
/// comparing every field; the fields documented as lossy in
/// `zotero::kovan` are checked to be exactly the ones that differ.
///
/// **Result (2026-10-07):** pass. Kept: slug (via `citationKey`), title,
/// authors (names), abstract, year, DOI, journal, publisher, volume, pages,
/// number (issue), keywords (automatic tags), tags, source URL, source path
/// (linked-file child). Lost, as documented: id, visibility (back as
/// Proprietary), affiliation, page_count (journalArticle has no numPages),
/// source_sha256, assets, related_*, markdown_body.
#[test]
fn kovan_to_zotero_to_kovan() {
    let doc = kovan_doc();
    let item = ZoteroItem::from_kovan_document(&doc);
    assert_eq!(item.item_type, ItemType::JournalArticle);
    assert!(item.validate().is_empty(), "{:?}", item.validate());
    let back = item.to_kovan_document();
    assert_eq!(back.slug, doc.slug);
    assert_eq!(back.title, doc.title);
    assert_eq!(back.document_type, doc.document_type);
    let names = |d: &KovanDocument| {
        d.authors
            .iter()
            .map(|a| (a.family.clone(), a.given.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&back), names(&doc));
    assert_eq!(back.abstract_text, doc.abstract_text);
    assert_eq!(back.year, doc.year);
    assert_eq!(back.doi, doc.doi);
    assert_eq!(back.journal, doc.journal);
    assert_eq!(back.publisher, doc.publisher);
    assert_eq!(back.volume, doc.volume);
    assert_eq!(back.pages, doc.pages);
    assert_eq!(back.number, doc.number);
    assert_eq!(back.keywords, doc.keywords);
    assert_eq!(back.tags, doc.tags);
    assert_eq!(back.source_url, doc.source_url);
    assert_eq!(back.source_path, doc.source_path);
    // Documented losses.
    assert_eq!(back.id, "zotero:");
    assert_eq!(back.visibility, Visibility::Proprietary);
    assert_eq!(back.authors[0].affiliation, None);
    assert_eq!(back.page_count, None);
    assert_eq!(back.source_sha256, None);
    assert!(
        back.assets.is_empty() && back.related_symbols.is_empty() && back.markdown_body.is_empty()
    );
}

/// **Methodology.** Zotero -> kovan -> Zotero on all 37 `itemJSON.js`
/// items: the document keeps the item in `zotero_item`, and the export
/// writes kovan's values over it.
///
/// **Pass:** the exported item equals the original exactly.
///
/// **Result (2026-10-07):** pass for all 37 (lossless).
#[test]
fn zotero_to_kovan_to_zotero_is_lossless() {
    for (name, json) in &fixture(ITEM_JSON) {
        let item = ZoteroItem::from_json_value(json).unwrap();
        let doc = item.to_kovan_document();
        let back = ZoteroItem::from_kovan_document(&doc);
        assert_eq!(back, item, "{name}");
        // A kovan edit is carried over.
        let mut edited = doc.clone();
        edited.title = "Changed".into();
        let out = ZoteroItem::from_kovan_document(&edited);
        assert_eq!(out.field_via_base(Field::Title), Some("Changed"), "{name}");
    }
}

/// **Methodology.** Spot values of the Zotero -> kovan map on the
/// `journalArticle`, `report` and `thesis` fixtures.
///
/// **Result (2026-10-07):** pass.
#[test]
fn zotero_to_kovan_field_map() {
    let items = fixture(ITEM_JSON);
    let ja = ZoteroItem::from_json_value(&items["journalArticle"])
        .unwrap()
        .to_kovan_document();
    assert_eq!(ja.document_type, DocumentType::Paper);
    assert_eq!(ja.journal.as_deref(), Some("Publication title"));
    assert_eq!(ja.year, Some(1999));
    assert_eq!(ja.authors[0].family, "authorLast");
    assert!(ja.authors.iter().all(|a| !a.family.starts_with("editor")));
    assert_eq!(ja.slug, "Citation key");
    let rep = ZoteroItem::from_json_value(&items["report"])
        .unwrap()
        .to_kovan_document();
    assert_eq!(rep.document_type, DocumentType::Report);
    assert!(rep.institution.is_some() && rep.publisher.is_none());
    let th = ZoteroItem::from_json_value(&items["thesis"])
        .unwrap()
        .to_kovan_document();
    assert_eq!(th.document_type, DocumentType::Thesis);
    assert!(
        th.page_count.is_some()
            || th
                .zotero_item
                .as_ref()
                .unwrap()
                .field(Field::NumPages)
                .is_some()
    );
}
