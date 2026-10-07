// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281),
// `test/tests/duplicatesTest.js`, `relationsTest.js`, `mergeItemsTest.js`,
// `dataObjectTest.js`, `itemTest.js`; Zotero utilities,
// https://github.com/zotero/utilities (commit 4051881d59c6),
// `test/tests/utilitiesTest.js`.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Zotero's own tests for duplicates, relations and merging, ported with
//! the same inputs and expected results (GitHub #751).
//!
//! **Methodology.** Each test names the upstream file, `describe`/`it` and
//! line. Upstream sets state up through the database (`createDataObject`,
//! `importPDFAttachment`, `createGroup`) or the UI (the duplicates pane);
//! here that state is written as a `ZoteroLibrary` fixture, and what
//! upstream reads from files (MD5, full text, embedded annotations, file
//! existence) is written as `AttachmentEvidence`, chosen to satisfy the
//! upstream test's **own assertions about those files** (e.g. "texts
//! equal, hashes differ"). Where upstream uses a test PDF whose text the
//! test does not state, the text here is synthetic and says so. Expected
//! results are upstream's assertions, unchanged; nothing was loosened.
//!
//! Upstream Zotero cannot run here and the local translation server does
//! not expose these functions, so these ported tests are the reference
//! (code-to-code verification is #752).
//!
//! **Results (2026-10-07):** see `docs/zotero-port.md` (every case passes;
//! the list of what could not be ported is there).

use std::collections::BTreeMap;

use kovan_common::zotero::{
    AnnotationData, AnnotationType, AttachmentData, Field, ItemType, LinkMode, Tag,
    ZoteroCollection, ZoteroItem, ZoteroLibrary,
};
use kovan_semantics::zotero::merge::replace_all_item_keys;
use kovan_semantics::zotero::relations::{
    add_linked_item, add_related_item, add_relation, has_relation, linked_item, related_item_keys,
    relations_by_predicate, remove_relation, remove_relations_to_erased_item, set_relations,
    subjects_by_predicate_and_object, update_user, LibraryRef, ObjectType, RelationError,
    Relations,
};
use kovan_semantics::zotero::text::{clean_isbn, to_isbn13};
use kovan_semantics::zotero::{
    find_duplicates, merge_items, merge_pane_order, AttachmentEvidence, DuplicateOptions,
    LibraryUri, MergeOptions, MergeOutcome, LINKED_OBJECT_PREDICATE, RELATED_ITEM_PREDICATE,
    REPLACED_ITEM_PREDICATE,
};
use serde_json::Value;

// ---------------------------------------------------------------- fixtures

const LOCAL: &str = "LOCALKEY";

fn user() -> LibraryUri {
    LibraryUri::LocalUser(LOCAL.into())
}

/// `createDataObject('item')`: a `book` (the test helper's default type).
fn book(key: &str) -> ZoteroItem {
    let mut i = ZoteroItem::new(ItemType::Book);
    i.key = Some(key.into());
    i
}

/// `createDataObject('item', { setTitle: true })`.
fn titled(key: &str, title: &str) -> ZoteroItem {
    let mut i = book(key);
    i.set_field(Field::Title, title);
    i
}

/// `item.clone()` then save: same type and fields, a new key, no
/// collections (`includeCollections` is off) and no relations here.
fn clone_as(item: &ZoteroItem, key: &str) -> ZoteroItem {
    let mut c = ZoteroItem::new(item.item_type);
    c.key = Some(key.into());
    c.fields = item.fields.clone();
    c.creators = item.creators.clone();
    c.tags = item.tags.clone();
    c
}

fn attachment(key: &str, parent: &str, lm: LinkMode, content_type: &str) -> ZoteroItem {
    let mut a = ZoteroItem::new(ItemType::Attachment);
    a.key = Some(key.into());
    a.parent_item = Some(parent.into());
    a.attachment = Some(AttachmentData {
        link_mode: Some(lm),
        content_type: Some(content_type.into()),
        ..AttachmentData::default()
    });
    a
}

/// `importPDFAttachment(item)` / `importFileAttachment(*.pdf)`.
fn pdf(key: &str, parent: &str) -> ZoteroItem {
    attachment(key, parent, LinkMode::ImportedFile, "application/pdf")
}

/// `importSnapshotAttachment` / `importFromSnapshotContent`.
fn snapshot(key: &str, parent: &str, title: &str, url: &str) -> ZoteroItem {
    let mut a = attachment(key, parent, LinkMode::ImportedUrl, "text/html");
    a.set_field(Field::Title, title);
    a.set_field(Field::Url, url);
    a
}

/// `Zotero.Attachments.linkFromURL`.
fn linked_url(key: &str, parent: &str, title: &str, url: &str) -> ZoteroItem {
    let mut a = attachment(key, parent, LinkMode::LinkedUrl, "");
    a.attachment.as_mut().unwrap().content_type = None;
    a.set_field(Field::Title, title);
    a.set_field(Field::Url, url);
    a
}

/// `createAnnotation(type, attachment)`.
fn annotation(key: &str, parent: &str, t: AnnotationType) -> ZoteroItem {
    let mut a = ZoteroItem::new(ItemType::Annotation);
    a.key = Some(key.into());
    a.parent_item = Some(parent.into());
    a.annotation = Some(AnnotationData {
        annotation_type: Some(t),
        ..AnnotationData::default()
    });
    a
}

/// An annotation imported from the PDF file non-destructively
/// (`PDFWorker.import(id, true)`): `annotationIsExternal`.
fn external_annotation(key: &str, parent: &str) -> ZoteroItem {
    let mut a = annotation(key, parent, AnnotationType::Highlight);
    a.other
        .insert("annotationIsExternal".into(), Value::Bool(true));
    a
}

/// A note shaped like `EditorInstance.createNoteFromAnnotations` output:
/// the annotation's attachment and the parent item as encoded URIs
/// (synthetic HTML; only the `%2Fitems%2F<KEY>` parts matter).
fn annotation_note(key: &str, parent: &str, attachment_key: &str) -> ZoteroItem {
    let mut n = ZoteroItem::new(ItemType::Note);
    n.key = Some(key.into());
    n.parent_item = Some(parent.into());
    let pre = format!("http%3A%2F%2Fzotero.org%2Fusers%2Flocal%2F{LOCAL}");
    n.note = Some(format!(
        "<div data-schema-version=\"9\"><p><span class=\"highlight\" data-annotation=\"\
         %7B%22attachmentURI%22%3A%22{pre}%2Fitems%2F{attachment_key}%22%7D\">x</span> \
         <span class=\"citation\" data-citation=\"%7B%22citationItems%22%3A%5B%7B%22uris%22\
         %3A%5B%22{pre}%2Fitems%2F{parent}%22%5D%7D%5D%7D\">(cite)</span></p></div>"
    ));
    n
}

fn lib(items: Vec<ZoteroItem>) -> ZoteroLibrary {
    ZoteroLibrary {
        collections: vec![],
        items,
    }
}

fn get<'a>(lib: &'a ZoteroLibrary, key: &str) -> &'a ZoteroItem {
    lib.item(key).unwrap_or_else(|| panic!("no item {key}"))
}

fn deleted(lib: &ZoteroLibrary, key: &str) -> bool {
    get(lib, key).deleted == Some(true)
}

/// `item.numAttachments(includeTrashed)`.
fn num_attachments(lib: &ZoteroLibrary, parent: &str, include_trashed: bool) -> usize {
    lib.children(parent)
        .filter(|i| i.item_type == ItemType::Attachment)
        .filter(|i| include_trashed || i.deleted != Some(true))
        .count()
}

fn parent_of(lib: &ZoteroLibrary, key: &str) -> String {
    get(lib, key).parent_item.clone().unwrap_or_default()
}

fn rels(lib: &ZoteroLibrary, key: &str, p: &str) -> Vec<String> {
    relations_by_predicate(&get(lib, key).relations, p).to_vec()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// File facts: the file exists, with this MD5 and text, and these
/// embedded annotations.
fn ev(md5: &str, text: Option<&str>, embedded: bool) -> AttachmentEvidence {
    AttachmentEvidence {
        file_exists: true,
        md5: Some(md5.into()),
        text: text.map(str::to_owned),
        has_embedded_annotations: Some(embedded),
    }
}

fn merge_with(
    library: &ZoteroLibrary,
    master: &str,
    others: &[&str],
    evidence: Vec<(&str, AttachmentEvidence)>,
) -> MergeOutcome {
    merge_in(user(), library, master, others, evidence)
}

fn merge_in(
    uri: LibraryUri,
    library: &ZoteroLibrary,
    master: &str,
    others: &[&str],
    evidence: Vec<(&str, AttachmentEvidence)>,
) -> MergeOutcome {
    let mut opts = MergeOptions::new(uri);
    opts.evidence = evidence
        .into_iter()
        .map(|(k, e)| (k.to_owned(), e))
        .collect::<BTreeMap<_, _>>();
    let others: Vec<String> = others.iter().map(|s| s.to_string()).collect();
    merge_items(library, master, &others, &opts).expect("merge")
}

fn uri(key: &str) -> String {
    user().item_uri(key)
}

/// Synthetic document text: `n` distinct letter-only words of 6 letters,
/// each repeated `rep` times, from a family named by `seed`.
fn words(seed: char, n: usize, rep: usize) -> String {
    let mut out = Vec::new();
    for i in 0..n {
        let a = (b'a' + (i / 26) as u8) as char;
        let b = (b'a' + (i % 26) as u8) as char;
        let w = format!("{seed}{seed}word{a}{b}");
        for _ in 0..rep {
            out.push(w.clone());
        }
    }
    out.join(" ")
}

// ------------------------------------------------- utilities: cleanISBN

/// utilitiesTest.js `#cleanISBN()` "should return false for non-ISBN string".
#[test]
fn utilities_clean_isbn_false_for_non_isbn() {
    assert_eq!(clean_isbn("", false), None);
    assert_eq!(clean_isbn("Random String 123", false), None);
    assert_eq!(clean_isbn("1234X67890", false), None);
    assert_eq!(clean_isbn("987123456789X", false), None);
}

/// utilitiesTest.js `#cleanISBN()` "should return false for invalid ISBN string".
#[test]
fn utilities_clean_isbn_false_for_invalid() {
    assert_eq!(clean_isbn("1234567890", false), None);
    assert_eq!(clean_isbn("9871234567890", false), None);
}

/// utilitiesTest.js `#cleanISBN()` "should return valid ISBN string given clean, valid ISBN string".
#[test]
fn utilities_clean_isbn_valid() {
    assert_eq!(
        clean_isbn("123456789X", false).as_deref(),
        Some("123456789X")
    );
    assert_eq!(
        clean_isbn("123456789x", false).as_deref(),
        Some("123456789X")
    );
    assert_eq!(
        clean_isbn("9781234567897", false).as_deref(),
        Some("9781234567897")
    );
    assert_eq!(
        clean_isbn("9791843123391", false).as_deref(),
        Some("9791843123391")
    );
}

/// utilitiesTest.js `#cleanISBN()` "should strip off internal characters in ISBN string".
#[test]
fn utilities_clean_isbn_internal_characters() {
    let ignored = "\u{2D}\u{AD}\u{2010}\u{2011}\u{2012}\u{2013}\u{2014}\u{2015}\u{2043}\u{2212}\
                   \u{20}\u{A0}\r\n\t\u{B}\u{C}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\
                   \u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}";
    for c in ignored.chars() {
        assert_eq!(
            clean_isbn(&format!("9781{c}234567897"), false).as_deref(),
            Some("9781234567897"),
            "stripped off U+{:04X}",
            c as u32
        );
    }
    assert_eq!(
        clean_isbn(&format!("9781{ignored}234567897"), false).as_deref(),
        Some("9781234567897")
    );
    let isbn_chars: String = format!("{ignored}1234567890");
    for i in 1u32..1327 {
        let Some(c) = char::from_u32(i) else { continue };
        if isbn_chars.contains(c) {
            continue;
        }
        assert_eq!(
            clean_isbn(&format!("9781{c}234567897"), false),
            None,
            "did not ignore internal character U+{i:04X}"
        );
    }
}

/// utilitiesTest.js `#cleanISBN()` "should strip off surrounding non-ISBN string".
#[test]
fn utilities_clean_isbn_surrounding() {
    for s in [
        "ISBN 9781234567897",
        "ISBN:9781234567897",
        "9781234567897 ISBN13",
        "9781234567897(ISBN13)",
        "ISBN13:9781234567897 (print)",
        "978 9781234567 897",
    ] {
        assert_eq!(
            clean_isbn(s, false).as_deref(),
            Some("9781234567897"),
            "{s}"
        );
    }
}

/// utilitiesTest.js `#cleanISBN()` "should return the first valid ISBN from a string with multiple ISBNs".
#[test]
fn utilities_clean_isbn_first_valid() {
    assert_eq!(
        clean_isbn("9781234567897, 9791843123391", false).as_deref(),
        Some("9781234567897")
    );
    assert_eq!(
        clean_isbn("123456789X, 0199535922", false).as_deref(),
        Some("123456789X")
    );
    assert_eq!(
        clean_isbn("123456789X 9781234567897", false).as_deref(),
        Some("123456789X")
    );
    assert_eq!(
        clean_isbn("9781234567897 123456789X", false).as_deref(),
        Some("9781234567897")
    );
    assert_eq!(
        clean_isbn("1234567890 9781234567897", false).as_deref(),
        Some("9781234567897")
    );
}

/// utilitiesTest.js `#cleanISBN()` "should not return an ISBN from a middle of a longer number string".
#[test]
fn utilities_clean_isbn_not_inside_numbers() {
    assert_eq!(clean_isbn("1239781234567897", false), None);
    assert_eq!(clean_isbn("9781234567897123", false), None);
    assert_eq!(clean_isbn("1239781234567897123", false), None);
}

/// utilitiesTest.js `#cleanISBN()` "should return valid ISBN from a dirty string".
#[test]
fn utilities_clean_isbn_dirty() {
    assert_eq!(
        clean_isbn(
            "<b>ISBN</b>:978-1 234\u{A0}56789 - 7(print)\n<b>ISBN-10</b>:123\u{2D}456789X (print)",
            false
        )
        .as_deref(),
        Some("9781234567897")
    );
}

/// utilitiesTest.js `#cleanISBN()` "should not validate check digit when dontValidate is set".
#[test]
fn utilities_clean_isbn_dont_validate() {
    assert_eq!(
        clean_isbn("9781234567890", true).as_deref(),
        Some("9781234567890")
    );
    assert_eq!(
        clean_isbn("1234567890", true).as_deref(),
        Some("1234567890")
    );
    assert_eq!(
        clean_isbn("1234567890 9781234567897", true).as_deref(),
        Some("1234567890")
    );
    assert_eq!(
        clean_isbn("9781234567890 123456789X", true).as_deref(),
        Some("9781234567890")
    );
}

/// utilitiesTest.js `#cleanISBN()` "should not pass non-ISBN strings if dontValidate is set".
#[test]
fn utilities_clean_isbn_dont_validate_non_isbn() {
    for s in [
        "",
        "Random String 123",
        "1234X67890",
        "123456789Y",
        "987123456789X",
        "1239781234567897",
        "9781234567897123",
        "1239781234567897123",
    ] {
        assert_eq!(clean_isbn(s, true), None, "{s}");
    }
}

/// utilitiesTest.js `toISBN13` "should throw on invalid ISBN" (`None` here).
#[test]
fn utilities_to_isbn13_invalid() {
    for s in ["", "random string", "1234567890123"] {
        assert_eq!(to_isbn13(s), None, "{s}");
    }
}

/// utilitiesTest.js `toISBN13` "should convert to ISBN13".
#[test]
fn utilities_to_isbn13_converts() {
    assert_eq!(to_isbn13("123456789X").as_deref(), Some("9781234567897"));
    assert_eq!(to_isbn13("9781234567897").as_deref(), Some("9781234567897"));
    assert_eq!(to_isbn13("9791843123391").as_deref(), Some("9791843123391"));
    assert_eq!(
        to_isbn13("978-1234567897").as_deref(),
        Some("9781234567897")
    );
}

/// utilitiesTest.js `toISBN13` "should ignore invalid check digit".
#[test]
fn utilities_to_isbn13_ignores_check_digit() {
    assert_eq!(to_isbn13("1234567890").as_deref(), Some("9781234567897"));
    assert_eq!(to_isbn13("9781234567890").as_deref(), Some("9781234567897"));
}

// ---------------------------------------------------- duplicatesTest.js

/// duplicatesTest.js:26 "ISBN matching" / "should match books with
/// equivalent ISBN-10 and ISBN-13".
#[test]
fn duplicates_isbn10_and_isbn13_match() {
    let mut a = titled("ITEMAAAA", "Effective Java");
    a.set_field(Field::Isbn, "0134685997");
    let mut b = titled("ITEMBBBB", "Effective Java, 3rd Edition");
    b.set_field(Field::Isbn, "9780134685991");
    let l = lib(vec![a, b]);
    let d = find_duplicates(&l, &DuplicateOptions::default());
    assert_eq!(d.set_of(0), vec![0, 1]);
}

/// The duplicates pane's merge: select the item in the duplicates view
/// (its whole set), order the set as the pane does, merge.
fn merge_in_duplicates_view(l: &ZoteroLibrary, key: &str) -> MergeOutcome {
    let d = find_duplicates(l, &DuplicateOptions::default());
    let idx = l
        .items
        .iter()
        .position(|i| i.key.as_deref() == Some(key))
        .unwrap();
    let keys: Vec<String> = d
        .set_of(idx)
        .into_iter()
        .map(|i| l.items[i].key.clone().unwrap())
        .collect();
    assert!(keys.len() > 1, "item is in a duplicate set");
    let (master, others) = merge_pane_order(l, &keys).unwrap();
    let others: Vec<&str> = others.iter().map(String::as_str).collect();
    merge_with(l, &master, &others, vec![])
}

/// duplicatesTest.js:64 "Merging" / "should merge two items in duplicates view".
#[test]
fn duplicates_merge_two_items_in_view() {
    let a = titled("ITEMAAAA", "A random title");
    let b = clone_as(&a, "ITEMBBBB");
    let l = lib(vec![a, b]);
    let out = merge_in_duplicates_view(&l, "ITEMAAAA");
    // "Items should be gone" from the duplicates view.
    let after = find_duplicates(&out.library, &DuplicateOptions::default());
    assert!(after.sets.is_empty());
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert_eq!(
        rels(&out.library, "ITEMAAAA", REPLACED_ITEM_PREDICATE),
        vec![uri("ITEMBBBB")]
    );
}

/// duplicatesTest.js:83 "should combine collections from all items".
#[test]
fn duplicates_merge_combines_collections() {
    let mut a = titled("ITEMAAAA", "A random title");
    a.collections = vec!["COLLAAAA".into()];
    let mut b = clone_as(&a, "ITEMBBBB");
    b.collections = vec!["COLLBBBB".into()];
    let mut l = lib(vec![a, b]);
    for k in ["COLLAAAA", "COLLBBBB"] {
        let mut c = ZoteroCollection::new(k);
        c.key = Some(k.into());
        l.collections.push(c);
    }
    let out = merge_in_duplicates_view(&l, "ITEMAAAA");
    assert!(deleted(&out.library, "ITEMBBBB"));
    let in_coll = |c: &str| {
        out.library
            .items_in_collection(c)
            .any(|i| i.key.as_deref() == Some("ITEMAAAA"))
    };
    assert!(in_coll("COLLAAAA"));
    assert!(in_coll("COLLBBBB"));
}

/// duplicatesTest.js:103 "should not create a relation to self if related
/// items are merged".
#[test]
fn duplicates_merge_no_relation_to_self() {
    let u = user();
    let mut a = titled("ITEMAAAA", "A random title");
    let mut b = clone_as(&a, "ITEMBBBB");
    let mut c = clone_as(&a, "ITEMCCCC");
    add_related_item(&u, &mut a, &u, &b.clone()).unwrap();
    add_related_item(&u, &mut b, &u, &a.clone()).unwrap();
    add_related_item(&u, &mut b, &u, &c.clone()).unwrap();
    add_related_item(&u, &mut c, &u, &b.clone()).unwrap();
    let l = lib(vec![a, b, c]);
    let out = merge_in_duplicates_view(&l, "ITEMAAAA");
    let rk = |k: &str| sorted(related_item_keys(get(&out.library, k)));
    assert_eq!(
        rk("ITEMAAAA"),
        vec!["ITEMBBBB".to_string(), "ITEMCCCC".to_string()]
    );
    assert_eq!(rk("ITEMBBBB"), vec!["ITEMAAAA".to_string()]);
    assert_eq!(rk("ITEMCCCC"), vec!["ITEMAAAA".to_string()]);
}

// ----------------------------------------------------- relationsTest.js

/// relationsTest.js:5 `#getByPredicateAndObject()` "should return items
/// matching predicate and object".
#[test]
fn relations_get_by_predicate_and_object() {
    let mut item = book("ITEMAAAA");
    let mut r = Relations::new();
    r.insert(
        "dc:relation".into(),
        vec!["http://zotero.org/users/1/items/SHREREMS".into()],
    );
    r.insert(
        "owl:sameAs".into(),
        vec![
            "http://zotero.org/groups/1/items/SRRMGSRM".into(),
            "http://zotero.org/groups/1/items/GSMRRSSM".into(),
        ],
    );
    set_relations(&mut item.relations, r).unwrap();
    let l = lib(vec![book("OTHERAAA"), item]);
    let found = subjects_by_predicate_and_object(
        &l,
        ObjectType::Item,
        "owl:sameAs",
        "http://zotero.org/groups/1/items/SRRMGSRM",
    )
    .unwrap();
    assert_eq!(found, vec![1]);
}

/// relationsTest.js:31 `#updateUser` "should update relations using local
/// user key to use userID".
#[test]
fn relations_update_user_from_local_key() {
    let u = user();
    let item1 = book("ITEMAAAA");
    let mut item2 = book("ITEMBBBB");
    add_related_item(&u, &mut item2, &u, &item1).unwrap();
    assert!(rels(
        &lib(vec![item2.clone()]),
        "ITEMBBBB",
        RELATED_ITEM_PREDICATE
    )[0]
    .contains("/users/local"));
    let mut l = lib(vec![item1, item2]);
    update_user(&mut l, &format!("local/{LOCAL}"), "1");
    assert!(rels(&l, "ITEMBBBB", RELATED_ITEM_PREDICATE)[0].contains("/users/1"));
}

/// relationsTest.js:48 `#updateUser` "should update relations from one
/// userID to another".
#[test]
fn relations_update_user_between_ids() {
    let u = LibraryUri::User(1);
    let item1 = book("ITEMAAAA");
    let mut item2 = book("ITEMBBBB");
    add_related_item(&u, &mut item2, &u, &item1).unwrap();
    let mut l = lib(vec![item1, item2]);
    assert!(rels(&l, "ITEMBBBB", RELATED_ITEM_PREDICATE)[0].contains("/users/1"));
    update_user(&mut l, "1", "2");
    assert!(rels(&l, "ITEMBBBB", RELATED_ITEM_PREDICATE)[0].contains("/users/2"));
}

// ---------------------------------------------- dataObjectTest.js (relations)

fn object_uri(kind: &str) -> String {
    format!("http://zotero.org/groups/1/{kind}/ABCD2345")
}

/// dataObjectTest.js:581 `#addRelation()` "should add a relation to an
/// object" (types `collection` and `item`).
#[test]
fn data_object_add_relation() {
    let mut item = book("ITEMAAAA");
    let mut coll = ZoteroCollection::new("C");
    for (r, kind) in [
        (&mut item.relations, "items"),
        (&mut coll.relations, "collections"),
    ] {
        let o = object_uri(kind);
        add_relation(r, "owl:sameAs", &o).unwrap();
        assert!(r["owl:sameAs"].contains(&o));
    }
}

/// dataObjectTest.js:596 `#removeRelation()` "should remove a relation from an object".
#[test]
fn data_object_remove_relation() {
    let mut item = book("ITEMAAAA");
    let mut coll = ZoteroCollection::new("C");
    for (r, kind) in [
        (&mut item.relations, "items"),
        (&mut coll.relations, "collections"),
    ] {
        let o = object_uri(kind);
        add_relation(r, "owl:sameAs", &o).unwrap();
        remove_relation(r, "owl:sameAs", &o);
        assert_eq!(r.len(), 0);
    }
}

/// dataObjectTest.js:613 `#hasRelation()` "should return true if an object has a given relation".
#[test]
fn data_object_has_relation() {
    let mut item = book("ITEMAAAA");
    let mut coll = ZoteroCollection::new("C");
    for (r, kind) in [
        (&mut item.relations, "items"),
        (&mut coll.relations, "collections"),
    ] {
        let o = object_uri(kind);
        add_relation(r, "owl:sameAs", &o).unwrap();
        assert!(has_relation(r, "owl:sameAs", &o));
    }
}

/// dataObjectTest.js:626 `#setRelations()` "shouldn't allow invalid 'relations' predicates".
#[test]
fn data_object_set_relations_rejects_invalid_predicate() {
    let mut item = book("ITEMAAAA");
    let mut r = Relations::new();
    r.insert("0".into(), vec!["http://example.com/foo".into()]);
    assert_eq!(
        set_relations(&mut item.relations, r),
        Err(RelationError::InvalidPredicate("0".into()))
    );
}

/// Two libraries: item1 in the user library, item2 in a group, and
/// `item2.addLinkedItem(item1)` (dataObjectTest.js:637-684 setup).
fn linked_pair(trash_item2: bool) -> (LibraryRef, LibraryRef) {
    let (ul, gl) = (user(), LibraryUri::Group(1));
    let mut item1 = book("ITEMAAAA");
    let mut item2 = book("ITEMBBBB");
    assert_eq!(add_linked_item(&gl, &mut item2, &ul, &mut item1), Ok(true));
    if trash_item2 {
        item2.deleted = Some(true);
    }
    (
        LibraryRef {
            uri: ul,
            library: lib(vec![item1]),
        },
        LibraryRef {
            uri: gl,
            library: lib(vec![item2]),
        },
    )
}

/// dataObjectTest.js:637 `#_getLinkedObject()` "should return a linked object in another library".
#[test]
fn data_object_linked_object_in_other_library() {
    let (u, g) = linked_pair(false);
    assert_eq!(linked_item(&u, &u.library.items[0], &g, false), Ok(Some(0)));
}

/// dataObjectTest.js:648 "shouldn't return a linked item in the trash in another library".
#[test]
fn data_object_linked_object_not_in_trash() {
    let (u, g) = linked_pair(true);
    assert_eq!(linked_item(&u, &u.library.items[0], &g, false), Ok(None));
}

/// dataObjectTest.js:661 "shouldn't return reverse linked objects by default".
#[test]
fn data_object_no_reverse_link_by_default() {
    let (u, g) = linked_pair(false);
    assert_eq!(linked_item(&g, &g.library.items[0], &u, false), Ok(None));
}

/// dataObjectTest.js:672 "should return reverse linked objects with bidirectional flag".
#[test]
fn data_object_reverse_link_with_bidirectional() {
    let (u, g) = linked_pair(false);
    assert_eq!(linked_item(&g, &g.library.items[0], &u, true), Ok(Some(0)));
}

/// dataObjectTest.js:686 `#_addLinkedObject()` "should add an owl:sameAs
/// relation" (and Date Modified is unchanged).
#[test]
fn data_object_add_linked_object() {
    let (u, g) = linked_pair(false);
    let item1 = &u.library.items[0];
    let preds = relations_by_predicate(&item1.relations, LINKED_OBJECT_PREDICATE);
    assert!(preds.contains(&g.uri.item_uri("ITEMBBBB")));
    assert_eq!(item1.date_modified, None);
}

// --------------------------------------------------- itemTest.js (relations)

/// itemTest.js:2089 `#addRelatedItem` "should add a dc:relation relation to an item".
#[test]
fn item_add_related_item() {
    let u = user();
    let mut item1 = book("ITEMAAAA");
    let item2 = book("ITEMBBBB");
    add_related_item(&u, &mut item1, &u, &item2).unwrap();
    let r = relations_by_predicate(&item1.relations, RELATED_ITEM_PREDICATE);
    assert_eq!(r, [uri("ITEMBBBB")]);
}

/// itemTest.js:2100 "should allow an unsaved item to be related to an item in the user library".
#[test]
fn item_add_related_item_unsaved() {
    let u = user();
    let item1 = book("ITEMAAAA");
    let mut item2 = book("ITEMBBBB");
    add_related_item(&u, &mut item2, &u, &item1).unwrap();
    let r = relations_by_predicate(&item2.relations, RELATED_ITEM_PREDICATE);
    assert_eq!(r, [uri("ITEMAAAA")]);
}

/// itemTest.js:2111 "should throw an error for a relation in a different library".
#[test]
fn item_add_related_item_other_library() {
    let mut item1 = book("ITEMAAAA");
    let item2 = book("ITEMBBBB");
    let e = add_related_item(&user(), &mut item1, &LibraryUri::Group(1), &item2).unwrap_err();
    assert_eq!(
        e.to_string(),
        "Cannot relate item to an item in a different library"
    );
}

/// itemTest.js:2309 `#_eraseData()` "should remove relations pointing to this item".
#[test]
fn item_erase_removes_relations_to_it() {
    let u = user();
    let mut item1 = book("ITEMAAAA");
    let mut item2 = book("ITEMBBBB");
    add_related_item(&u, &mut item1, &u, &item2.clone()).unwrap();
    add_related_item(&u, &mut item2, &u, &item1.clone()).unwrap();
    let mut l = lib(vec![item1, item2]);
    remove_relations_to_erased_item(&mut l, &u, "ITEMAAAA");
    l.items.remove(0);
    assert!(related_item_keys(get(&l, "ITEMBBBB")).is_empty());
    assert!(get(&l, "ITEMBBBB").relations.is_empty());
}

// ----------------------------------------------------- mergeItemsTest.js

/// mergeItemsTest.js:8 "should merge two items".
#[test]
fn merge_two_items() {
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB")]);
    let out = merge_with(&l, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    assert!(!deleted(&out.library, "ITEMAAAA"));
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert_eq!(
        rels(&out.library, "ITEMAAAA", REPLACED_ITEM_PREDICATE),
        vec![uri("ITEMBBBB")]
    );
}

/// mergeItemsTest.js:25 "should merge three items".
#[test]
fn merge_three_items() {
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB"), book("ITEMCCCC")]);
    let out = merge_with(&l, "ITEMAAAA", &["ITEMBBBB", "ITEMCCCC"], vec![]);
    assert!(!deleted(&out.library, "ITEMAAAA"));
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert!(deleted(&out.library, "ITEMCCCC"));
    assert_eq!(
        sorted(rels(&out.library, "ITEMAAAA", REPLACED_ITEM_PREDICATE)),
        sorted(vec![uri("ITEMBBBB"), uri("ITEMCCCC")])
    );
}

/// mergeItemsTest.js:45 "should use the earliest Date Added".
#[test]
fn merge_earliest_date_added() {
    let mut a = book("ITEMAAAA");
    a.date_added = Some("2019-01-02 00:00:00".into());
    let mut b = book("ITEMBBBB");
    b.date_added = Some("2019-01-01 00:00:00".into());
    let mut c = book("ITEMCCCC");
    c.date_added = Some("2019-01-03 00:00:00".into());
    let out = merge_with(
        &lib(vec![a, b, c]),
        "ITEMAAAA",
        &["ITEMBBBB", "ITEMCCCC"],
        vec![],
    );
    assert_eq!(
        get(&out.library, "ITEMAAAA").date_added.as_deref(),
        Some("2019-01-01 00:00:00")
    );
}

fn tag(name: &str, t: Option<u8>) -> Tag {
    Tag {
        tag: name.into(),
        tag_type: t,
    }
}

/// mergeItemsTest.js:54 "should keep automatic tag on non-master item as automatic".
#[test]
fn merge_keeps_automatic_tag_from_other() {
    let mut a = book("ITEMAAAA");
    a.tags = vec![tag("A", None)];
    let mut b = book("ITEMBBBB");
    b.tags = vec![tag("B", Some(1))];
    let out = merge_with(&lib(vec![a, b]), "ITEMAAAA", &["ITEMBBBB"], vec![]);
    let t = get(&out.library, "ITEMAAAA")
        .tags
        .iter()
        .find(|t| t.tag == "B")
        .cloned()
        .unwrap();
    assert_eq!(t.tag_type, Some(1));
}

/// mergeItemsTest.js:63 "should skip automatic tag on non-master item that
/// exists as manual tag on master".
#[test]
fn merge_skips_automatic_tag_that_is_manual_on_master() {
    let mut a = book("ITEMAAAA");
    a.tags = vec![tag("A", None), tag("B", None)];
    let mut b = book("ITEMBBBB");
    b.tags = vec![tag("B", Some(1))];
    let out = merge_with(&lib(vec![a, b]), "ITEMAAAA", &["ITEMBBBB"], vec![]);
    let t = get(&out.library, "ITEMAAAA")
        .tags
        .iter()
        .find(|t| t.tag == "B")
        .cloned()
        .unwrap();
    assert_eq!(t.tag_type, None); // assert.notProperty(tag, 'type')
}

/// mergeItemsTest.js:72 "should keep automatic tag on master if it also
/// exists on non-master item".
#[test]
fn merge_keeps_automatic_master_tag() {
    let mut a = book("ITEMAAAA");
    a.tags = vec![tag("B", Some(1))];
    let mut b = book("ITEMBBBB");
    b.tags = vec![tag("B", Some(1))];
    let out = merge_with(&lib(vec![a, b]), "ITEMAAAA", &["ITEMBBBB"], vec![]);
    assert_eq!(get(&out.library, "ITEMAAAA").tags[0].tag_type, Some(1));
}

/// mergeItemsTest.js:80 "should merge two items when servant is linked to
/// an item absent from cache". The object cache does not exist here; the
/// fixture is the two group libraries, itemX (group two) linked to item2.
#[test]
fn merge_with_servant_linked_from_other_library() {
    let (g1, g2) = (LibraryUri::Group(25026), LibraryUri::Group(11592));
    let item1 = book("ITEMAAAA");
    let mut item2 = book("ITEMBBBB");
    let mut item_x = book("ITEMXXXX");
    add_linked_item(&g2, &mut item_x, &g1, &mut item2).unwrap();
    let x_rels = relations_by_predicate(&item_x.relations, LINKED_OBJECT_PREDICATE).to_vec();
    assert_eq!(x_rels, vec![g1.item_uri("ITEMBBBB")]);
    let l = lib(vec![item1, item2]);
    let out = merge_in(g1.clone(), &l, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    assert!(!deleted(&out.library, "ITEMAAAA"));
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert_eq!(
        rels(&out.library, "ITEMAAAA", REPLACED_ITEM_PREDICATE),
        vec![g1.item_uri("ITEMBBBB")]
    );
}

/// mergeItemsTest.js:129 "should move merge-tracking relation from replaced item to master".
#[test]
fn merge_moves_merge_tracking_relation() {
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB"), book("ITEMCCCC")]);
    let out = merge_with(&l, "ITEMBBBB", &["ITEMCCCC"], vec![]);
    let out = merge_with(&out.library, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    assert_eq!(
        sorted(rels(&out.library, "ITEMAAAA", REPLACED_ITEM_PREDICATE)),
        sorted(vec![uri("ITEMBBBB"), uri("ITEMCCCC")])
    );
}

/// mergeItemsTest.js:146 "should transfer merge-tracking relations when
/// merging two pairs into one item".
#[test]
fn merge_transfers_merge_tracking_of_two_pairs() {
    let l = lib(vec![
        titled("ITEMAAAA", "A"),
        titled("ITEMBBBB", "B"),
        titled("ITEMCCCC", "C"),
        titled("ITEMDDDD", "D"),
    ]);
    let out = merge_with(&l, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    let out = merge_with(&out.library, "ITEMCCCC", &["ITEMDDDD"], vec![]);
    let out = merge_with(&out.library, "ITEMAAAA", &["ITEMCCCC"], vec![]);
    assert_eq!(
        sorted(rels(&out.library, "ITEMAAAA", REPLACED_ITEM_PREDICATE)),
        sorted(vec![uri("ITEMBBBB"), uri("ITEMCCCC"), uri("ITEMDDDD")])
    );
}

/// mergeItemsTest.js:166 "should update relations pointing to replaced item to point to master".
#[test]
fn merge_repoints_relations_to_master() {
    let mut item3 = book("ITEMCCCC");
    add_relation(
        &mut item3.relations,
        RELATED_ITEM_PREDICATE,
        &uri("ITEMBBBB"),
    )
    .unwrap();
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB"), item3]);
    let out = merge_with(&l, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    assert_eq!(
        rels(&out.library, "ITEMCCCC", RELATED_ITEM_PREDICATE),
        vec![uri("ITEMAAAA")]
    );
}

/// mergeItemsTest.js:183 "should not update relations pointing to replaced
/// item in other libraries". item3 lives in another library, which a
/// one-library merge cannot see: it is passed through unchanged.
#[test]
fn merge_leaves_other_libraries_alone() {
    let (g1, g2) = (LibraryUri::Group(1), LibraryUri::Group(2));
    let mut item3 = book("ITEMCCCC");
    add_relation(
        &mut item3.relations,
        LINKED_OBJECT_PREDICATE,
        &g1.item_uri("ITEMBBBB"),
    )
    .unwrap();
    let other_lib = LibraryRef {
        uri: g2,
        library: lib(vec![item3]),
    };
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB")]);
    let _ = merge_in(g1.clone(), &l, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    assert_eq!(
        rels(&other_lib.library, "ITEMCCCC", LINKED_OBJECT_PREDICATE),
        vec![g1.item_uri("ITEMBBBB")]
    );
}

/// `importPDFAttachment` imports the same test.pdf every time: one MD5,
/// no embedded annotations.
const TEST_PDF_MD5: &str = "test-pdf-md5";

/// mergeItemsTest.js:203 "should merge identical attachments based on file hash".
#[test]
fn merge_attachments_by_file_hash() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        b,
        pdf("ATTBBBBB", "ITEMBBBB"),
    ]);
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![
            ("ATTAAAAA", ev(TEST_PDF_MD5, None, false)),
            ("ATTBBBBB", ev(TEST_PDF_MD5, None, false)),
        ],
    );
    assert!(!deleted(&out.library, "ITEMAAAA"));
    assert!(!deleted(&out.library, "ATTAAAAA"));
    assert_eq!(num_attachments(&out.library, "ITEMAAAA", true), 1);
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert!(deleted(&out.library, "ATTBBBBB"));
}

/// mergeItemsTest.js:220 "should merge one attachment per item into the master attachment".
#[test]
fn merge_one_attachment_per_item_into_master() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let c = clone_as(&a, "ITEMCCCC");
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        b,
        pdf("ATTBBBBB", "ITEMBBBB"),
        c,
        pdf("ATTCCCCC", "ITEMCCCC"),
    ]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB", "ITEMCCCC"],
        vec![e("ATTAAAAA"), e("ATTBBBBB"), e("ATTCCCCC")],
    );
    assert!(!deleted(&out.library, "ATTAAAAA"));
    assert_eq!(num_attachments(&out.library, "ITEMAAAA", true), 1);
    for k in ["ITEMBBBB", "ATTBBBBB", "ITEMCCCC", "ATTCCCCC"] {
        assert!(deleted(&out.library, k), "{k}");
    }
}

/// The two-attachment content-hash cases: master att A, other att B.
fn two_pdf_merge(ev_a: AttachmentEvidence, ev_b: AttachmentEvidence) -> MergeOutcome {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        b,
        pdf("ATTBBBBB", "ITEMBBBB"),
    ]);
    merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![("ATTAAAAA", ev_a), ("ATTBBBBB", ev_b)],
    )
}

fn assert_merged_into_master(out: &MergeOutcome) {
    assert!(!deleted(&out.library, "ITEMAAAA"));
    assert!(!deleted(&out.library, "ATTAAAAA"));
    assert_eq!(num_attachments(&out.library, "ITEMAAAA", true), 1);
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert!(deleted(&out.library, "ATTBBBBB"));
}

fn assert_both_kept(out: &MergeOutcome) {
    assert!(!deleted(&out.library, "ITEMAAAA"));
    assert!(!deleted(&out.library, "ATTAAAAA"));
    assert_eq!(num_attachments(&out.library, "ITEMAAAA", true), 2);
    assert!(deleted(&out.library, "ITEMBBBB"));
    assert!(!deleted(&out.library, "ATTBBBBB"));
}

/// mergeItemsTest.js:243 "should merge identical attachments based on
/// content hash": equal text (asserted upstream), different MD5. Text
/// synthetic (60 words).
#[test]
fn merge_attachments_by_content_hash() {
    let t = words('a', 60, 3);
    let out = two_pdf_merge(ev("md5-1", Some(&t), false), ev("md5-2", Some(&t), false));
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:263 "should merge identical attachments based on
/// content hash when unindexed": there is no full-text index here, so this
/// is the same computation as the previous case.
#[test]
fn merge_attachments_by_content_hash_unindexed() {
    let t = words('a', 60, 3);
    let out = two_pdf_merge(ev("md5-1", Some(&t), false), ev("md5-2", Some(&t), false));
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:285 "shouldn't merge attachments based on content
/// hash when files are empty": empty text (asserted upstream), MD5s differ.
#[test]
fn merge_not_by_empty_content() {
    let out = two_pdf_merge(ev("md5-1", Some(""), false), ev("md5-2", Some(""), false));
    assert_both_kept(&out);
}

/// mergeItemsTest.js:306 "should ignore PDF attachment with missing file".
#[test]
fn merge_ignores_missing_file() {
    let mut missing = ev(TEST_PDF_MD5, None, false);
    missing.file_exists = false;
    let out = two_pdf_merge(ev(TEST_PDF_MD5, None, false), missing);
    assert_both_kept(&out);
}

/// mergeItemsTest.js:325 "should allow small differences when hashing
/// content" (JSTOR_1/2.pdf: texts differ, MD5s differ, the 50 most common
/// words agree). Synthetic texts: one body plus a different one-off
/// cover-page line each.
#[test]
fn merge_allows_small_text_differences() {
    let body = words('a', 60, 3);
    let t1 = format!("{body} downloaded from jstor terms conditions");
    let t2 = format!("{body} accessed through jstor stable link");
    assert_ne!(t1, t2);
    let out = two_pdf_merge(ev("md5-1", Some(&t1), false), ev("md5-2", Some(&t2), false));
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:351 "should keep similar but not identical attachments
/// separate" (wonderland_short vs wonderland_long). Synthetic: the long
/// text's added chapters dominate its 50 most common words.
#[test]
fn merge_keeps_similar_attachments_separate() {
    let short = words('a', 30, 3);
    let long = format!("{short} {}", words('b', 60, 5));
    let out = two_pdf_merge(
        ev("md5-1", Some(&short), false),
        ev("md5-2", Some(&long), false),
    );
    assert_both_kept(&out);
}

/// mergeItemsTest.js:371 "should only match attachments one-to-one":
/// watermarked_1 on the master, watermarked_2 twice on the other item.
/// Synthetic texts: one body, different watermark words.
#[test]
fn merge_matches_one_to_one() {
    let body = words('a', 60, 3);
    let w1 = format!("{body} watermark first");
    let w2 = format!("{body} watermark second");
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        b,
        pdf("ATTBBBBB", "ITEMBBBB"),
        pdf("ATTCCCCC", "ITEMBBBB"),
    ]);
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![
            ("ATTAAAAA", ev("md5-1", Some(&w1), false)),
            ("ATTBBBBB", ev("md5-2", Some(&w2), false)),
            ("ATTCCCCC", ev("md5-2", Some(&w2), false)),
        ],
    );
    assert!(!deleted(&out.library, "ATTAAAAA"));
    assert_eq!(num_attachments(&out.library, "ITEMAAAA", true), 2);
    assert!(deleted(&out.library, "ITEMBBBB"));
    let (d2, d3) = (
        deleted(&out.library, "ATTBBBBB"),
        deleted(&out.library, "ATTCCCCC"),
    );
    assert!((d2 || d3) && !(d2 && d3));
}

/// mergeItemsTest.js:390 "should copy annotations when merging".
#[test]
fn merge_copies_annotations() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let note = annotation_note("NOTEBBBB", "ITEMBBBB", "ATTBBBBB");
    assert!(note.note.as_ref().unwrap().contains("ATTBBBBB"));
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        annotation("ANNAAAAA", "ATTAAAAA", AnnotationType::Note),
        b,
        pdf("ATTBBBBB", "ITEMBBBB"),
        annotation("ANNBBBBB", "ATTBBBBB", AnnotationType::Highlight),
        note,
    ]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA") && !deleted(lb, "ATTAAAAA") && !deleted(lb, "ANNAAAAA"));
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 1);
    assert!(deleted(lb, "ITEMBBBB") && deleted(lb, "ATTBBBBB"));
    assert!(!deleted(lb, "ANNBBBBB"));
    assert_eq!(parent_of(lb, "ANNAAAAA"), "ATTAAAAA");
    assert_eq!(parent_of(lb, "ANNBBBBB"), "ATTAAAAA");
    let html = get(lb, "NOTEBBBB").note.clone().unwrap();
    assert!(!html.contains("ITEMBBBB") && html.contains("ITEMAAAA"));
    assert!(!html.contains("ATTBBBBB") && html.contains("ATTAAAAA"));
}

/// mergeItemsTest.js:420 "should merge attachments in group library with
/// annotation created by another user".
#[test]
fn merge_group_annotation_by_other_user() {
    let g = LibraryUri::Group(7);
    let a = book("ITEMAAAA");
    let b = clone_as(&a, "ITEMBBBB");
    let mut ann2 = annotation("ANNBBBBB", "ATTBBBBB", AnnotationType::Highlight);
    ann2.other
        .insert("createdByUserID".into(), Value::from(92624235u64));
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        annotation("ANNAAAAA", "ATTAAAAA", AnnotationType::Note),
        b,
        pdf("ATTBBBBB", "ITEMBBBB"),
        ann2,
    ]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_in(
        g,
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    assert_eq!(parent_of(&out.library, "ANNBBBBB"), "ATTAAAAA");
    assert_eq!(
        get(&out.library, "ANNBBBBB").other["createdByUserID"],
        Value::from(92624235u64)
    );
}

/// mergeItemsTest.js:440 "should update all item keys when moving notes"
/// (three PDFs imported on both items: three MD5s).
#[test]
fn merge_updates_all_item_keys_in_moved_notes() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let mut items = vec![a];
    let mut evidence = Vec::new();
    for (i, md5) in ["arxiv", "doi", "title"].iter().enumerate() {
        let k = format!("ATTA{i}AAA");
        items.push(pdf(&k, "ITEMAAAA"));
        evidence.push((k, ev(md5, None, false)));
    }
    items.push(b);
    for (i, md5) in ["arxiv", "doi", "title"].iter().enumerate() {
        let (k, ann, note) = (
            format!("ATTB{i}BBB"),
            format!("ANNB{i}BBB"),
            format!("NOTB{i}BBB"),
        );
        items.push(pdf(&k, "ITEMBBBB"));
        items.push(annotation(&ann, &k, AnnotationType::Highlight));
        let n = annotation_note(&note, "ITEMBBBB", &k);
        let h = n.note.clone().unwrap();
        assert!(h.contains("ITEMBBBB") && h.contains(&k));
        items.push(n);
        evidence.push((k, ev(md5, None, false)));
    }
    let l = lib(items);
    let ev_ref: Vec<(&str, AttachmentEvidence)> = evidence
        .iter()
        .map(|(k, e)| (k.as_str(), e.clone()))
        .collect();
    let out = merge_with(&l, "ITEMAAAA", &["ITEMBBBB"], ev_ref);
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA"));
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 3);
    assert!(deleted(lb, "ITEMBBBB"));
    for i in 0..3 {
        let (a1, a2, n) = (
            format!("ATTA{i}AAA"),
            format!("ATTB{i}BBB"),
            format!("NOTB{i}BBB"),
        );
        assert_eq!(parent_of(lb, &n), "ITEMAAAA");
        let h = get(lb, &n).note.clone().unwrap();
        assert!(h.contains("ITEMAAAA") && !h.contains("ITEMBBBB"));
        assert!(h.contains(&a1) && !h.contains(&a2));
    }
}

/// The web-attachment cases: master item A and other item B, attachments
/// listed per item; snapshots' files exist unless said otherwise.
fn web_merge(
    master_atts: Vec<ZoteroItem>,
    other_atts: Vec<ZoteroItem>,
    missing: &[&str],
) -> MergeOutcome {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let mut evidence = Vec::new();
    for at in master_atts.iter().chain(other_atts.iter()) {
        let k = at.key.clone().unwrap();
        let mut e = ev("snapshot-md5", None, false);
        e.file_exists = !missing.contains(&k.as_str());
        evidence.push((k, e));
    }
    let mut items = vec![a];
    items.extend(master_atts);
    items.push(b);
    items.extend(other_atts);
    let l = lib(items);
    let ev_ref: Vec<(&str, AttachmentEvidence)> = evidence
        .iter()
        .map(|(k, e)| (k.as_str(), e.clone()))
        .collect();
    merge_with(&l, "ITEMAAAA", &["ITEMBBBB"], ev_ref)
}

/// mergeItemsTest.js:490 "should merge snapshots with the same title, even if URL differs".
#[test]
fn merge_snapshots_same_title_different_url() {
    let out = web_merge(
        vec![snapshot(
            "ATTAAAAA",
            "ITEMAAAA",
            "Snapshot",
            "https://example.com/test.html",
        )],
        vec![snapshot(
            "ATTBBBBB",
            "ITEMBBBB",
            "Snapshot",
            "https://otherdomain.example.com/test.html",
        )],
        &[],
    );
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:523 "should keep a non-master snapshot that matches a trashed master snapshot".
#[test]
fn merge_keeps_snapshot_matching_trashed_master() {
    let mut s1 = snapshot("ATTAAAAA", "ITEMAAAA", "Snapshot", "http://example.com/");
    s1.deleted = Some(true);
    let out = web_merge(
        vec![s1],
        vec![snapshot(
            "ATTBBBBB",
            "ITEMBBBB",
            "Snapshot",
            "http://example.com/",
        )],
        &[],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA"));
    assert!(deleted(lb, "ATTAAAAA"));
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB"));
    assert!(!deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
}

/// mergeItemsTest.js:543 "should merge linked URLs".
#[test]
fn merge_linked_urls() {
    let out = web_merge(
        vec![linked_url(
            "ATTAAAAA",
            "ITEMAAAA",
            "Catalog Entry",
            "https://example.com/",
        )],
        vec![
            linked_url(
                "ATTBBBBB",
                "ITEMBBBB",
                "Catalog Entry",
                "https://example.com/",
            ),
            linked_url(
                "ATTCCCCC",
                "ITEMBBBB",
                "Catalog Entry",
                "https://example.com/",
            ),
        ],
        &[],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA") && !deleted(lb, "ATTAAAAA"));
    assert_eq!(
        get(lb, "ATTAAAAA").field(Field::Url),
        Some("https://example.com/")
    );
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB") && deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTCCCCC"), "ITEMAAAA");
    assert!(!deleted(lb, "ATTCCCCC"));
}

/// mergeItemsTest.js:576 "should keep linked URL with same title but different URL".
#[test]
fn merge_keeps_linked_url_with_different_url() {
    let out = web_merge(
        vec![linked_url(
            "ATTAAAAA",
            "ITEMAAAA",
            "Catalog Entry",
            "https://example.com/",
        )],
        vec![linked_url(
            "ATTBBBBB",
            "ITEMBBBB",
            "Catalog Entry",
            "https://otherdomain.example.com/",
        )],
        &[],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ATTAAAAA"));
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
    assert!(!deleted(lb, "ATTBBBBB"));
    assert_eq!(
        get(lb, "ATTBBBBB").field(Field::Url),
        Some("https://otherdomain.example.com/")
    );
}

/// mergeItemsTest.js:603 "should keep web attachment with same URL but different title".
#[test]
fn merge_keeps_web_attachment_with_different_title() {
    let out = web_merge(
        vec![linked_url(
            "ATTAAAAA",
            "ITEMAAAA",
            "Catalog Entry",
            "https://example.com/",
        )],
        vec![
            linked_url(
                "ATTBBBBB",
                "ITEMBBBB",
                "Official Website",
                "https://example.com/",
            ),
            linked_url(
                "ATTCCCCC",
                "ITEMBBBB",
                "Catalog Entry",
                "https://example.com/",
            ),
        ],
        &[],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ATTAAAAA"));
    assert_eq!(
        get(lb, "ATTAAAAA").field(Field::Url),
        Some("https://example.com/")
    );
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
    assert!(!deleted(lb, "ATTBBBBB"));
    assert!(deleted(lb, "ATTCCCCC"));
}

/// mergeItemsTest.js:636 "should keep only snapshot that exists when
/// merging non-master snapshot (missing) with equivalent master snapshot
/// (exists)". (Upstream also checks the master's file still exists; there
/// are no files here.)
#[test]
fn merge_snapshot_missing_on_other() {
    let out = web_merge(
        vec![snapshot(
            "ATTAAAAA",
            "ITEMAAAA",
            "Snapshot",
            "http://example.com/",
        )],
        vec![snapshot(
            "ATTBBBBB",
            "ITEMBBBB",
            "Snapshot",
            "http://example.com/",
        )],
        &["ATTBBBBB"],
    );
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:658 "should both snapshots when merging non-master
/// snapshot (exists) with equivalent master snapshot (missing)".
#[test]
fn merge_snapshot_missing_on_master() {
    let out = web_merge(
        vec![snapshot(
            "ATTAAAAA",
            "ITEMAAAA",
            "Snapshot",
            "http://example.com/",
        )],
        vec![snapshot(
            "ATTBBBBB",
            "ITEMBBBB",
            "Snapshot",
            "http://example.com/",
        )],
        &["ATTAAAAA"],
    );
    assert_both_kept(&out);
}

/// mergeItemsTest.js:680 "should move related items of merged attachments".
#[test]
fn merge_moves_related_items_of_merged_attachments() {
    let u = user();
    let related = book("RELATEDA");
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let mut att2 = pdf("ATTBBBBB", "ITEMBBBB");
    add_related_item(&u, &mut att2, &u, &related).unwrap();
    let l = lib(vec![related, a, pdf("ATTAAAAA", "ITEMAAAA"), b, att2]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    assert_merged_into_master(&out);
    assert_eq!(
        related_item_keys(get(&out.library, "ATTAAAAA")),
        vec!["RELATEDA".to_string()]
    );
}

/// mergeItemsTest.js:702 "should move merge-tracking relation from replaced
/// attachment to master attachment" (three items, one test.pdf each).
#[test]
fn merge_moves_attachment_merge_tracking() {
    let l = lib(vec![
        book("ITEMAAAA"),
        pdf("ATTAAAAA", "ITEMAAAA"),
        book("ITEMBBBB"),
        pdf("ATTBBBBB", "ITEMBBBB"),
        book("ITEMCCCC"),
        pdf("ATTCCCCC", "ITEMCCCC"),
    ]);
    let e = || {
        ["ATTAAAAA", "ATTBBBBB", "ATTCCCCC"]
            .into_iter()
            .map(|k| (k, ev(TEST_PDF_MD5, None, false)))
            .collect::<Vec<_>>()
    };
    let out = merge_with(&l, "ITEMBBBB", &["ITEMCCCC"], e());
    let out = merge_with(&out.library, "ITEMAAAA", &["ITEMBBBB"], e());
    assert_eq!(
        sorted(rels(&out.library, "ATTAAAAA", REPLACED_ITEM_PREDICATE)),
        sorted(vec![uri("ATTBBBBB"), uri("ATTCCCCC")])
    );
}

/// mergeItemsTest.js:722 "should not merge attachments with different content types".
#[test]
fn merge_not_with_different_content_types() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let att2 = attachment("ATTBBBBB", "ITEMBBBB", LinkMode::ImportedFile, "text/plain");
    let l = lib(vec![a, pdf("ATTAAAAA", "ITEMAAAA"), b, att2]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    assert_both_kept(&out);
    assert_eq!(parent_of(&out.library, "ATTBBBBB"), "ITEMAAAA");
}

/// mergeItemsTest.js:742 "should merge two stored-file attachments with different link modes".
#[test]
fn merge_stored_files_with_different_link_modes() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let att1 = attachment(
        "ATTAAAAA",
        "ITEMAAAA",
        LinkMode::ImportedUrl,
        "application/pdf",
    );
    let l = lib(vec![a, att1, b, pdf("ATTBBBBB", "ITEMBBBB")]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:764 "should not merge attachments with different link mode types".
#[test]
fn merge_not_linked_with_stored() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let att1 = attachment(
        "ATTAAAAA",
        "ITEMAAAA",
        LinkMode::LinkedFile,
        "application/pdf",
    );
    let l = lib(vec![a, att1, b, pdf("ATTBBBBB", "ITEMBBBB")]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    assert_both_kept(&out);
    assert_eq!(parent_of(&out.library, "ATTBBBBB"), "ITEMAAAA");
}

/// mergeItemsTest.js:789 "should not merge an attachment with a deleted master attachment".
#[test]
fn merge_not_with_trashed_master_attachment() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let mut att1 = pdf("ATTAAAAA", "ITEMAAAA");
    att1.deleted = Some(true);
    let l = lib(vec![a, att1, b, pdf("ATTBBBBB", "ITEMBBBB")]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA") && deleted(lb, "ATTAAAAA"));
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB") && !deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
}

/// mergeItemsTest.js:809 "should move but not merge a trashed non-master PDF attachment".
#[test]
fn merge_moves_trashed_other_pdf() {
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let mut att2 = pdf("ATTBBBBB", "ITEMBBBB");
    att2.deleted = Some(true);
    let l = lib(vec![
        a,
        pdf("ATTAAAAA", "ITEMAAAA"),
        b,
        att2,
        annotation("ANNBBBBB", "ATTBBBBB", AnnotationType::Highlight),
    ]);
    let e = |k| (k, ev(TEST_PDF_MD5, None, false));
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![e("ATTAAAAA"), e("ATTBBBBB")],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA") && !deleted(lb, "ATTAAAAA"));
    assert!(deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB"));
    assert_eq!(parent_of(lb, "ANNBBBBB"), "ATTBBBBB");
}

/// mergeItemsTest.js:832 "should move but not merge a trashed non-master snapshot".
#[test]
fn merge_moves_trashed_other_snapshot() {
    let mut s2 = snapshot("ATTBBBBB", "ITEMBBBB", "Snapshot", "http://example.com/");
    s2.deleted = Some(true);
    let out = web_merge(
        vec![snapshot(
            "ATTAAAAA",
            "ITEMAAAA",
            "Snapshot",
            "http://example.com/",
        )],
        vec![s2],
        &[],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA") && !deleted(lb, "ATTAAAAA"));
    assert!(deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 2);
    assert!(deleted(lb, "ITEMBBBB"));
    assert!(rels(lb, "ATTAAAAA", REPLACED_ITEM_PREDICATE).is_empty());
}

/// mergeItemsTest.js:855 "should move a trashed non-master non-PDF/non-web attachment".
#[test]
fn merge_moves_trashed_other_plain_attachment() {
    let mut att = attachment("ATTBBBBB", "ITEMBBBB", LinkMode::ImportedFile, "text/plain");
    att.deleted = Some(true);
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB"), att]);
    let out = merge_with(
        &l,
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![("ATTBBBBB", ev(TEST_PDF_MD5, None, false))],
    );
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA"));
    assert!(deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
    assert_eq!(num_attachments(lb, "ITEMAAAA", true), 1);
    assert!(deleted(lb, "ITEMBBBB"));
}

/// The embedded-annotation cases (annotated_1/annotated_2/notAnnotated
/// PDFs): the pair matches (same text, different MD5); each file's
/// embedded-annotation state is what upstream's test asserts or implies.
fn annotated_merge(
    master_embedded: bool,
    other_embedded: bool,
    master_children: Vec<ZoteroItem>,
    other_children: Vec<ZoteroItem>,
) -> MergeOutcome {
    let t = words('a', 60, 3);
    let a = titled("ITEMAAAA", "T");
    let b = clone_as(&a, "ITEMBBBB");
    let mut items = vec![a, pdf("ATTAAAAA", "ITEMAAAA")];
    items.extend(master_children);
    items.push(b);
    items.push(pdf("ATTBBBBB", "ITEMBBBB"));
    items.extend(other_children);
    merge_with(
        &lib(items),
        "ITEMAAAA",
        &["ITEMBBBB"],
        vec![
            ("ATTAAAAA", ev("md5-1", Some(&t), master_embedded)),
            ("ATTBBBBB", ev("md5-2", Some(&t), other_embedded)),
        ],
    )
}

fn annotations_of(lib: &ZoteroLibrary, att: &str) -> Vec<ZoteroItem> {
    lib.children(att)
        .filter(|i| i.item_type == ItemType::Annotation)
        .cloned()
        .collect()
}

fn is_external(i: &ZoteroItem) -> bool {
    i.other.get("annotationIsExternal") == Some(&Value::Bool(true))
}

/// mergeItemsTest.js:873 "should not merge two matching PDF attachments
/// with embedded annotations" (both files' annotations imported
/// non-destructively, so both stay embedded).
#[test]
fn merge_not_two_pdfs_with_embedded_annotations() {
    let out = annotated_merge(
        true,
        true,
        vec![external_annotation("ANNAAAAA", "ATTAAAAA")],
        vec![external_annotation("ANNBBBBB", "ATTBBBBB")],
    );
    assert_both_kept(&out);
    assert_eq!(parent_of(&out.library, "ATTBBBBB"), "ITEMAAAA");
    for att in ["ATTAAAAA", "ATTBBBBB"] {
        let anns = annotations_of(&out.library, att);
        assert_eq!(anns.len(), 1);
        assert!(is_external(&anns[0]));
    }
}

/// mergeItemsTest.js:906 "should merge imported annotations into PDF with
/// remaining unimported annotations": after the destructive import,
/// annotated_1 has no embedded annotation left and annotated_2 keeps an
/// unsupported one (asserted upstream).
#[test]
fn merge_imported_annotations_into_pdf_with_unimported_ones() {
    let out = annotated_merge(
        false,
        true,
        vec![annotation(
            "ANNAAAAA",
            "ATTAAAAA",
            AnnotationType::Highlight,
        )],
        vec![annotation(
            "ANNBBBBB",
            "ATTBBBBB",
            AnnotationType::Highlight,
        )],
    );
    let lb = &out.library;
    assert!(deleted(lb, "ATTAAAAA"));
    assert!(!deleted(lb, "ATTBBBBB"));
    assert_eq!(annotations_of(lb, "ATTAAAAA").len(), 0);
    let anns = annotations_of(lb, "ATTBBBBB");
    assert_eq!(anns.len(), 2);
    assert!(anns.iter().all(|a| !is_external(a)));
}

/// mergeItemsTest.js:942 "should merge a non-master PDF without embedded
/// annotations into a master PDF with embedded annotations".
#[test]
fn merge_plain_other_into_annotated_master() {
    let out = annotated_merge(true, false, vec![], vec![]);
    assert_merged_into_master(&out);
}

/// mergeItemsTest.js:959 "should merge a master PDF without embedded
/// annotations into a non-master PDF with embedded annotations".
#[test]
fn merge_plain_master_into_annotated_other() {
    let out = annotated_merge(false, true, vec![], vec![]);
    let lb = &out.library;
    assert!(!deleted(lb, "ITEMAAAA"));
    assert!(deleted(lb, "ATTAAAAA"));
    assert_eq!(num_attachments(lb, "ITEMAAAA", false), 1);
    assert!(deleted(lb, "ITEMBBBB"));
    assert!(!deleted(lb, "ATTBBBBB"));
    assert_eq!(parent_of(lb, "ATTBBBBB"), "ITEMAAAA");
}

// ------------------------------------------------ a flagged upstream behaviour

/// Not an upstream test: pins the `replaceAllItemKeys` empty-map behaviour
/// the port reproduces (see `merge` module docs). A merge with no merged
/// PDF moves a note whose `%2Fitems%2F` links gain `undefined`.
#[test]
fn flagged_note_links_when_no_attachment_was_merged() {
    let note = annotation_note("NOTEBBBB", "ITEMBBBB", "ATTXXXXX");
    let l = lib(vec![book("ITEMAAAA"), book("ITEMBBBB"), note]);
    let out = merge_with(&l, "ITEMAAAA", &["ITEMBBBB"], vec![]);
    let h = get(&out.library, "NOTEBBBB").note.clone().unwrap();
    assert!(h.contains("%2Fitems%2FundefinedITEMAAAA"));
    assert_eq!(
        replace_all_item_keys("%2Fitems%2FK", &[]),
        "%2Fitems%2FundefinedK"
    );
}
