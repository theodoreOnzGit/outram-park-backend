// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// test/tests/searchTest.js, ported case by case; fixtures from
// test/content/support.js (createDataObject, importFileAttachment,
// importPDFAttachment, createAnnotation) and test/tests/data/search/.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Zotero's own search tests (searchTest.js), ported.
//!
//! **Methodology.** Each test cites the upstream `it(...)` by line. Upstream
//! creates rows in a fresh SQLite database (`createDataObject`,
//! `importPDFAttachment`, ...); here each test builds the same objects into
//! a [`ZoteroLibrary`] fixture ([`Fx`]) and runs the ported [`Search`] over
//! it. The inputs (conditions, operators, values) and the expected results
//! (`sameMembers`, `include`, `notInclude`, `lengthOf`) are upstream's,
//! unchanged. Upstream's random strings (`Zotero.Utilities.randomString`)
//! are replaced by a deterministic generator of 8-character strings, so the
//! tests are reproducible; where upstream relies on randomness for
//! uniqueness, the generator gives unique strings too.
//!
//! The shared `before()` fixture (searchTest.js:235-269) is [`base`]: three
//! imported files (`search/foo.html`, `search/foobar.html`,
//! `search/baz.pdf`), an imported-URL snapshot, a linked file and a linked
//! URL. Their full text, which Zotero's indexer would extract, is supplied
//! explicitly: the HTML files' text content (`hello` + body), and the PDFs'
//! text as `pdftotext` (poppler) prints it (`baz`; test.pdf's Zotero
//! blurb). The group-library copies are left out: a `ZoteroLibrary` is one
//! library, and every assertion upstream restricts to the user library.
//!
//! The clock is fixed at 2026-10-07T12:00:00Z and every item's `dateAdded`
//! is that instant (upstream items are created "now").
//!
//! **Reference.** Upstream Zotero desktop cannot run here, and the
//! translation server at 127.0.0.1:1969 exposes no search, so these
//! upstream tests are the reference. Upstream code was read, not run.
//!
//! **Results (2026-10-07):** see the module docs of
//! `kovan_discovery::zotero` for the count; every ported assertion passes
//! with upstream's expected values.

// `foo`, `baz`: upstream's fixture names (fooItem, bazItem).
#![allow(clippy::disallowed_names)]

use kovan_common::zotero::{
    AnnotationData, AnnotationType, AttachmentData, Creator, CreatorType, Field, ItemType,
    LinkMode, Tag, ZoteroCollection, ZoteroItem, ZoteroLibrary,
};
use kovan_discovery::zotero::{
    quick_search, QuickSearchMode, QuickSearchScope, Search, SearchClock, SearchError,
    SearchLibrary,
};

const NOW: i64 = 1_791_374_400; // 2026-10-07T12:00:00Z
const NOW_ISO: &str = "2026-10-07T12:00:00Z";
const TEST_PDF_TEXT: &str = "Zotero [zoh-TAIR-oh] is a free, easy-to-use tool to help you \
collect, organize, cite, and share\nyour research sources.";

struct Fx {
    items: Vec<ZoteroItem>,
    collections: Vec<ZoteroCollection>,
    text: Vec<(String, String)>,
    saved: Vec<Search>,
    seed: u64,
}

struct Base {
    foo: String,
    foobar: String,
    baz: String,
    imported_url: String,
    linked_file: String,
    linked_url: String,
}

impl Fx {
    fn new() -> Self {
        Fx {
            items: Vec::new(),
            collections: Vec::new(),
            text: Vec::new(),
            saved: Vec::new(),
            seed: 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        self.seed ^= self.seed >> 12;
        self.seed ^= self.seed << 25;
        self.seed ^= self.seed >> 27;
        self.seed.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn pick_from(&mut self, alphabet: &str, n: usize) -> String {
        let a: Vec<char> = alphabet.chars().collect();
        (0..n)
            .map(|_| a[(self.next() % a.len() as u64) as usize])
            .collect()
    }

    /// `Zotero.Utilities.randomString()` (8 chars of [a-zA-Z0-9]).
    fn rand(&mut self) -> String {
        self.pick_from(
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            8,
        )
    }

    /// `Zotero.DataObjectUtilities.generateKey()`.
    fn key(&mut self) -> String {
        self.pick_from("23456789ABCDEFGHIJKLMNPQRSTUVWXYZ", 8)
    }

    fn push(&mut self, mut it: ZoteroItem) -> String {
        let k = self.key();
        it.key = Some(k.clone());
        it.date_added = Some(NOW_ISO.into());
        it.date_modified = Some(NOW_ISO.into());
        self.items.push(it);
        k
    }

    /// `createDataObject('item', {...})`: a `book` by default.
    fn item(&mut self, f: impl FnOnce(&mut ZoteroItem)) -> String {
        self.typed(ItemType::Book, f)
    }

    fn typed(&mut self, t: ItemType, f: impl FnOnce(&mut ZoteroItem)) -> String {
        let mut it = ZoteroItem::new(t);
        f(&mut it);
        self.push(it)
    }

    fn titled(&mut self, title: &str) -> String {
        let t = title.to_owned();
        self.item(move |i| i.set_field(Field::Title, t))
    }

    fn attachment(
        &mut self,
        parent: Option<&str>,
        title: &str,
        mode: LinkMode,
        content_type: &str,
        text: Option<&str>,
    ) -> String {
        let mut it = ZoteroItem::new(ItemType::Attachment);
        it.set_field(Field::Title, title);
        it.parent_item = parent.map(str::to_owned);
        it.attachment = Some(AttachmentData {
            link_mode: Some(mode),
            content_type: Some(content_type.to_owned()),
            ..AttachmentData::default()
        });
        let k = self.push(it);
        if let Some(t) = text {
            self.text.push((k.clone(), t.to_owned()));
        }
        k
    }

    /// `importPDFAttachment(parent)` (support.js:1197): test.pdf.
    fn pdf(&mut self, parent: Option<&str>) -> String {
        self.attachment(
            parent,
            "test.pdf",
            LinkMode::ImportedFile,
            "application/pdf",
            Some(TEST_PDF_TEXT),
        )
    }

    /// `importFileAttachment("search/foobar.html", { parentID })`.
    fn foobar_html(&mut self, parent: Option<&str>) -> String {
        self.attachment(
            parent,
            "foobar.html",
            LinkMode::ImportedFile,
            "text/html",
            Some("hello\nfoo bar"),
        )
    }

    /// `createAnnotation(type, parent, {comment})` (support.js:1211): a
    /// highlight gets random text; the comment is random unless given.
    fn annotation(&mut self, t: AnnotationType, parent: &str, comment: Option<&str>) -> String {
        let text = (t == AnnotationType::Highlight).then(|| self.rand());
        let comment = match comment {
            Some(c) => c.to_owned(),
            None => self.rand(),
        };
        let mut it = ZoteroItem::new(ItemType::Annotation);
        it.parent_item = Some(parent.to_owned());
        it.annotation = Some(AnnotationData {
            annotation_type: Some(t),
            text,
            comment: Some(comment),
            color: Some("#ffd400".into()),
            ..AnnotationData::default()
        });
        self.push(it)
    }

    fn note(&mut self, parent: Option<&str>, html: &str) -> String {
        let mut it = ZoteroItem::new(ItemType::Note);
        it.parent_item = parent.map(str::to_owned);
        it.note = Some(html.to_owned());
        self.push(it)
    }

    fn collection(&mut self, parent: Option<&str>) -> String {
        let k = self.key();
        let mut c = ZoteroCollection::new(self.rand());
        c.key = Some(k.clone());
        c.parent_collection = parent.map(str::to_owned);
        self.collections.push(c);
        k
    }

    fn get(&mut self, key: &str) -> &mut ZoteroItem {
        self.items
            .iter_mut()
            .find(|i| i.key.as_deref() == Some(key))
            .expect("item")
    }

    fn ann(&mut self, key: &str) -> &mut AnnotationData {
        self.get(key).annotation.as_mut().expect("annotation")
    }

    /// `createDataObject('search')` (support.js:612-621): title contains
    /// R1, title isNot R2. Returns (key, R1).
    fn saved_search(&mut self) -> (String, String) {
        let mut s = Search::new();
        s.key = Some(self.key());
        let name = self.rand();
        s.set_name(&name).unwrap();
        let r1 = self.rand();
        let r2 = self.rand();
        s.add_condition("title", "contains", &r1).unwrap();
        s.add_condition("title", "isNot", &r2).unwrap();
        let k = s.key.clone().unwrap();
        self.saved.push(s);
        (k, r1)
    }

    fn lib(&self) -> SearchLibrary {
        let zl = ZoteroLibrary {
            collections: self.collections.clone(),
            items: self.items.clone(),
        };
        let mut l = SearchLibrary::new(&zl, SearchClock::utc(NOW));
        for (k, t) in &self.text {
            l.set_full_text(k.clone(), t.clone());
        }
        for s in &self.saved {
            l.add_saved_search(s.clone());
        }
        l
    }
}

/// The shared fixture of `describe("#search()")` (searchTest.js:235-269).
fn base() -> (Fx, Base) {
    let mut fx = Fx::new();
    let foo = fx.attachment(
        None,
        "foo.html",
        LinkMode::ImportedFile,
        "text/html",
        Some("hello\nfoo"),
    );
    let foobar = fx.foobar_html(None);
    let baz = fx.attachment(
        None,
        "baz.pdf",
        LinkMode::ImportedFile,
        "application/pdf",
        Some("baz"),
    );
    let imported_url = fx.attachment(
        None,
        "imported-url-pdf",
        LinkMode::ImportedUrl,
        "application/pdf",
        Some(TEST_PDF_TEXT),
    );
    fx.get(&imported_url)
        .set_field(Field::Url, "http://example.com/imported-url.pdf");
    let linked_file = fx.attachment(
        None,
        "linked-file-pdf",
        LinkMode::LinkedFile,
        "application/pdf",
        Some(TEST_PDF_TEXT),
    );
    let linked_url = fx.attachment(
        None,
        "linked-url",
        LinkMode::LinkedUrl,
        "application/pdf",
        None,
    );
    fx.get(&linked_url)
        .set_field(Field::Url, "http://example.com/linked-url.pdf");
    (
        fx,
        Base {
            foo,
            foobar,
            baz,
            imported_url,
            linked_file,
            linked_url,
        },
    )
}

fn search(conds: &[(&str, &str, &str)]) -> Search {
    let mut s = Search::new();
    for (c, o, v) in conds {
        s.add_condition(c, o, v).unwrap();
    }
    s
}

fn run(fx: &Fx, s: &Search) -> Vec<String> {
    s.run(&fx.lib()).unwrap()
}

fn same(mut got: Vec<String>, want: &[&String]) {
    let mut w: Vec<String> = want.iter().map(|s| (*s).clone()).collect();
    w.sort();
    got.sort();
    assert_eq!(got, w);
}

fn author(last: &str) -> Creator {
    Creator::person(CreatorType::Author, "", last)
}

// ---------------------------------------------------------------- basics

/// searchTest.js:3 "should fail if empty"
#[test]
fn name_should_fail_if_empty() {
    assert_eq!(Search::new().set_name(""), Err(SearchError::EmptyName));
}

/// searchTest.js:10 "should convert old-style 'collection' condition value"
#[test]
fn add_condition_converts_old_style_collection_value() {
    let mut fx = Fx::new();
    let col = fx.collection(None);
    let c2 = col.clone();
    let item = fx.item(move |i| i.collections = vec![c2]);
    let s = search(&[("collection", "is", &format!("0_{col}"))]);
    same(run(&fx, &s), &[&item]);
}

// ------------------------------------------------- combineConditions / mapPredicate
// searchTest.js:160-221 assert the generated SQL text. Here a predicate is a
// row set, so the same cases are asserted on sets: A AND B is the
// intersection, A OR B the union, a nested group combines first.

mod sets {
    use super::*;
    use kovan_discovery::zotero::levels::{combine_conditions, map_predicate, Built, Predicate};
    use kovan_discovery::zotero::{Level, Operator};
    use std::collections::BTreeSet;

    fn lib() -> SearchLibrary {
        let mut fx = Fx::new();
        for _ in 0..4 {
            fx.item(|_| {});
        }
        fx.lib()
    }

    fn p(rows: &[usize]) -> Built {
        Built::Pred(Predicate {
            rows: rows.iter().copied().collect(),
            levels: vec![Level::Item],
            negate: false,
        })
    }

    fn set(rows: &[usize]) -> BTreeSet<usize> {
        rows.iter().copied().collect()
    }

    /// searchTest.js:165 "should AND predicates in 'all' mode"
    #[test]
    fn and_in_all_mode() {
        let l = lib();
        let r = combine_conditions(&l, &[p(&[0, 1]), p(&[1, 2])], Level::Any, true);
        assert_eq!(r, Some(set(&[1])));
    }

    /// searchTest.js:170 "should OR and parenthesize predicates in 'any' mode"
    #[test]
    fn or_in_any_mode() {
        let l = lib();
        let r = combine_conditions(
            &l,
            &[Built::JoinMode(Operator::Any), p(&[0]), p(&[1])],
            Level::Any,
            true,
        );
        assert_eq!(r, Some(set(&[0, 1])));
    }

    /// searchTest.js:177 "should combine a nested group" (A AND (B OR C))
    #[test]
    fn nested_group() {
        let l = lib();
        let r = combine_conditions(
            &l,
            &[
                p(&[0, 1, 2]),
                Built::GroupStart,
                Built::JoinMode(Operator::Any),
                p(&[1]),
                p(&[2, 3]),
                Built::GroupEnd,
            ],
            Level::Any,
            true,
        );
        assert_eq!(r, Some(set(&[1, 2])));
    }

    /// searchTest.js:200, 205, 209, 213 (mapPredicate)
    #[test]
    fn map_predicate_cases() {
        let l = lib();
        let x = set(&[0, 2]);
        // 'any' result level: unchanged
        assert_eq!(
            map_predicate(&l, &x, &[Level::Item], Level::Any, false, false),
            x
        );
        assert_eq!(
            map_predicate(&l, &x, &[Level::Any], Level::Any, false, false),
            x
        );
        // a negated level-agnostic condition is not rolled up
        assert_eq!(
            map_predicate(&l, &x, &[Level::Any], Level::Item, true, false),
            x
        );
        // note vs annotation: nothing
        assert!(map_predicate(&l, &x, &[Level::Note], Level::Annotation, false, false).is_empty());
        // a multi-level field at a level it matches at: unchanged
        let ia = [Level::Item, Level::Attachment];
        assert_eq!(map_predicate(&l, &x, &ia, Level::Item, false, false), x);
        assert_eq!(
            map_predicate(&l, &x, &ia, Level::Attachment, false, false),
            x
        );
        assert_eq!(map_predicate(&l, &x, &ia, Level::Any, false, false), x);
    }
}

// ---------------------------------------------------------------- nested groups

/// searchTest.js:285 "should match A AND (B OR C)"
#[test]
fn groups_a_and_b_or_c() {
    let (mut fx, _) = base();
    let ab = fx.item(|i| {
        i.set_field(Field::Title, "zgrpA");
        i.tags = vec![Tag {
            tag: "zgrpB".into(),
            tag_type: None,
        }];
    });
    let ac = fx.item(|i| {
        i.set_field(Field::Title, "zgrpA");
        i.tags = vec![Tag {
            tag: "zgrpC".into(),
            tag_type: None,
        }];
    });
    fx.titled("zgrpA");
    fx.item(|i| {
        i.tags = vec![Tag {
            tag: "zgrpB".into(),
            tag_type: None,
        }]
    });
    let s = search(&[
        ("joinMode", "all", ""),
        ("title", "contains", "zgrpA"),
        ("groupStart", "true", ""),
        ("joinMode", "any", ""),
        ("tag", "is", "zgrpB"),
        ("tag", "is", "zgrpC"),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &s), &[&ab, &ac]);
}

/// searchTest.js:303 "should match A OR (B AND C)"
#[test]
fn groups_a_or_b_and_c() {
    let (mut fx, _) = base();
    let a = fx.titled("zg2A");
    let bc = fx.item(|i| {
        i.set_field(Field::Title, "zg2B");
        i.tags = vec![Tag {
            tag: "zg2C".into(),
            tag_type: None,
        }];
    });
    fx.titled("zg2B");
    let s = search(&[
        ("joinMode", "any", ""),
        ("title", "contains", "zg2A"),
        ("groupStart", "true", ""),
        ("joinMode", "all", ""),
        ("title", "contains", "zg2B"),
        ("tag", "is", "zg2C"),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &s), &[&a, &bc]);
}

/// searchTest.js:320 "should match nested groups A AND (B OR (C AND D))"
#[test]
fn groups_nested() {
    let (mut fx, _) = base();
    let ab = fx.item(|i| {
        i.set_field(Field::Title, "zg3A");
        i.tags = vec![Tag {
            tag: "zg3B".into(),
            tag_type: None,
        }];
    });
    let acd = fx.item(|i| {
        i.set_field(Field::Title, "zg3A zg3C");
        i.tags = vec![Tag {
            tag: "zg3D".into(),
            tag_type: None,
        }];
    });
    fx.titled("zg3A zg3C");
    let s = search(&[
        ("joinMode", "all", ""),
        ("title", "contains", "zg3A"),
        ("groupStart", "true", ""),
        ("joinMode", "any", ""),
        ("tag", "is", "zg3B"),
        ("groupStart", "true", ""),
        ("joinMode", "all", ""),
        ("title", "contains", "zg3C"),
        ("tag", "is", "zg3D"),
        ("groupEnd", "true", ""),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &s), &[&ab, &acd]);
}

// ---------------------------------------------------------------- cross-level

/// searchTest.js:344 "should match a top-level item by a condition on a descendant annotation"
#[test]
fn cross_item_by_descendant_annotation() {
    let (mut fx, _) = base();
    let text = format!("zscopematch{}", fx.rand());
    let item = fx.item(|i| i.creators = vec![author("Zscopesmith")]);
    let att = fx.pdf(Some(&item));
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&an).text = Some(text.clone());
    fx.item(|i| i.creators = vec![author("Zscopesmith")]);
    let other = fx.item(|i| i.creators = vec![author("Zscopejones")]);
    let oatt = fx.pdf(Some(&other));
    let oan = fx.annotation(AnnotationType::Highlight, &oatt, None);
    fx.ann(&oan).text = Some(text.clone());
    let s = search(&[
        ("creator", "contains", "Zscopesmith"),
        ("groupStart", "true", ""),
        ("resultLevel", "annotation", ""),
        ("annotationText", "contains", &text),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:385 "should match an annotation by its top-level item's title"
#[test]
fn cross_annotation_by_item_title() {
    let (mut fx, _) = base();
    let title = format!("zanntitle{}", fx.rand());
    let item = fx.titled(&title);
    let att = fx.pdf(Some(&item));
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let s = search(&[
        ("resultLevel", "annotation", ""),
        ("title", "contains", &title),
    ]);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:402 "should match an annotation by its parent attachment's title"
#[test]
fn cross_annotation_by_attachment_title() {
    let (mut fx, _) = base();
    let title = format!("zatttitle{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Title, title.clone());
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let s = search(&[
        ("resultLevel", "annotation", ""),
        ("title", "contains", &title),
    ]);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:420 "should match annotations by type and color"
#[test]
fn cross_annotation_type_and_color() {
    let (mut fx, _) = base();
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    let hl = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&hl).color = Some("#ff6666".into());
    let note = fx.annotation(AnnotationType::Note, &att, None);
    fx.ann(&note).color = Some("#5fb236".into());
    // ANNOTATION_TYPE_HIGHLIGHT = 1, ANNOTATION_TYPE_NOTE = 2
    same(run(&fx, &search(&[("annotationType", "is", "1")])), &[&hl]);
    same(
        run(&fx, &search(&[("annotationColor", "is", "#5fb236")])),
        &[&note],
    );
    let m = run(&fx, &search(&[("annotationType", "isNot", "2")]));
    assert!(m.contains(&hl));
    assert!(!m.contains(&note));
    assert!(!m.contains(&item));
    assert!(!m.contains(&att));
}

/// searchTest.js:460 "should return descendant annotations as results with a top-level condition"
#[test]
fn cross_descendant_annotations_as_results() {
    let (mut fx, _) = base();
    let text = format!("zresult{}", fx.rand());
    let item = fx.item(|i| i.creators = vec![author("Zresultsmith")]);
    let att = fx.pdf(Some(&item));
    let m1 = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&m1).text = Some(text.clone());
    let m2 = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&m2).text = Some(text.clone());
    fx.annotation(AnnotationType::Highlight, &att, None);
    let other = fx.item(|i| i.creators = vec![author("Zresultjones")]);
    let oatt = fx.pdf(Some(&other));
    let oan = fx.annotation(AnnotationType::Highlight, &oatt, None);
    fx.ann(&oan).text = Some(text.clone());
    let s = search(&[
        ("resultLevel", "annotation", ""),
        ("creator", "contains", "Zresultsmith"),
        ("annotationText", "contains", &text),
    ]);
    same(run(&fx, &s), &[&m1, &m2]);
}

/// searchTest.js:497 "should match annotations for a negated annotation condition at the annotation result level"
#[test]
fn cross_negated_annotation_condition() {
    let (mut fx, _) = base();
    let text = format!("zneg{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    let with = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&with).text = Some(text.clone());
    let without = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&without).text = Some("zsomethingelse".into());
    let s = search(&[
        ("resultLevel", "annotation", ""),
        ("annotationText", "doesNotContain", &text),
    ]);
    same(run(&fx, &s), &[&without]);
}

/// searchTest.js:519 "should match an item in a collection by a child attachment's content"
#[test]
fn cross_collection_and_child_content() {
    let (mut fx, _) = base();
    let col = fx.collection(None);
    let c = col.clone();
    let m = fx.item(move |i| i.collections = vec![c]);
    fx.foobar_html(Some(&m));
    let c = col.clone();
    fx.item(move |i| i.collections = vec![c]);
    let nic = fx.item(|_| {});
    fx.foobar_html(Some(&nic));
    let s = search(&[
        ("resultLevel", "item", ""),
        ("collection", "is", &col),
        ("fulltextContent", "contains", "foo bar"),
    ]);
    same(run(&fx, &s), &[&m]);
}

/// searchTest.js:544 "should return a collection's matching descendants at a descendant result level"
#[test]
fn cross_collection_descendants() {
    let (mut fx, _) = base();
    let text = format!("zcoll{}", fx.rand());
    let col = fx.collection(None);
    let c = col.clone();
    let item = fx.item(move |i| i.collections = vec![c]);
    let att = fx.pdf(Some(&item));
    let m = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&m).text = Some(text.clone());
    let other = fx.item(|_| {});
    let oatt = fx.pdf(Some(&other));
    let oan = fx.annotation(AnnotationType::Highlight, &oatt, None);
    fx.ann(&oan).text = Some(text.clone());
    let s = search(&[
        ("resultLevel", "annotation", ""),
        ("collection", "is", &col),
        ("annotationText", "contains", &text),
    ]);
    same(run(&fx, &s), &[&m]);
}

/// searchTest.js:573 "should match an itemData field at the item or attachment level it lives at"
#[test]
fn cross_item_data_level() {
    let (mut fx, _) = base();
    let it_title = format!("zti{}", fx.rand());
    let at_title = format!("zta{}", fx.rand());
    let item = fx.titled(&it_title);
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Title, at_title.clone());
    let s = |level: &str, v: &str| search(&[("resultLevel", level, ""), ("title", "contains", v)]);
    same(run(&fx, &s("attachment", &at_title)), &[&att]);
    same(run(&fx, &s("attachment", &it_title)), &[]);
    same(run(&fx, &s("item", &it_title)), &[&item]);
    same(run(&fx, &s("item", &at_title)), &[]);
}

/// searchTest.js:603 "should match Any Field against an attachment's own fields at the attachment result level"
#[test]
fn cross_any_field_attachment_level() {
    let (mut fx, _) = base();
    let item_url = format!("https://example.com/zaf{}", fx.rand());
    let att_url = format!("https://example.com/zaf{}", fx.rand());
    let iu = item_url.clone();
    let item = fx.item(move |i| i.set_field(Field::Url, iu));
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Url, att_url.clone());
    let s = |v: &str| {
        search(&[
            ("resultLevel", "attachment", ""),
            ("anyField", "contains", v),
        ])
    };
    same(run(&fx, &s(&att_url)), &[&att]);
    same(run(&fx, &s(&item_url)), &[]);
}

/// searchTest.js:631 "should match an item by Any Field in a group at the attachment level"
#[test]
fn cross_any_field_group_attachment_level() {
    let (mut fx, _) = base();
    let url = format!("https://example.com/zafg{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Url, url.clone());
    let other = fx.item(|_| {});
    fx.pdf(Some(&other));
    let s = search(&[
        ("resultLevel", "item", ""),
        ("groupStart", "true", ""),
        ("resultLevel", "attachment", ""),
        ("anyField", "contains", &url),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:654 "should map a bare descendant condition to the result level (no group)"
#[test]
fn cross_bare_descendant_condition() {
    let (mut fx, _) = base();
    let text = format!("zbarecorr{}", fx.rand());
    let item = fx.titled("zbarecorritem");
    let att = fx.pdf(Some(&item));
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    fx.ann(&an).text = Some(text.clone());
    let s = search(&[
        ("resultLevel", "item", ""),
        ("annotationText", "contains", &text),
    ]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:672 "should match an item by its tag and a descendant annotation's comment"
#[test]
fn cross_tag_and_annotation_comment() {
    let (mut fx, _) = base();
    let tag = format!("ztag{}", fx.rand());
    let word = format!("zword{}", fx.rand());
    let t = tag.clone();
    let item = fx.item(move |i| {
        i.tags = vec![Tag {
            tag: t,
            tag_type: None,
        }]
    });
    let att = fx.pdf(Some(&item));
    fx.annotation(
        AnnotationType::Highlight,
        &att,
        Some(&format!("foo {word} bar")),
    );
    let t = tag.clone();
    let tag_only = fx.item(move |i| {
        i.tags = vec![Tag {
            tag: t,
            tag_type: None,
        }]
    });
    fx.pdf(Some(&tag_only));
    let ann_only = fx.item(|_| {});
    let aatt = fx.pdf(Some(&ann_only));
    fx.annotation(
        AnnotationType::Highlight,
        &aatt,
        Some(&format!("foo {word} bar")),
    );
    let s = search(&[
        ("resultLevel", "item", ""),
        ("tag", "is", &tag),
        ("annotationComment", "contains", &word),
    ]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:706 "should map a bare full-text condition to the result level (no group)"
#[test]
fn cross_bare_fulltext() {
    let (mut fx, b) = base();
    let item = fx.titled("zbareft");
    fx.foobar_html(Some(&item));
    let s = search(&[
        ("resultLevel", "item", ""),
        ("fulltextContent", "contains", "foo bar"),
    ]);
    same(run(&fx, &s), &[&item, &b.foobar]);
}

/// searchTest.js:723 "should return a standalone attachment whose annotation matches, at the item result level"
#[test]
fn cross_standalone_attachment_annotation() {
    let (mut fx, _) = base();
    let word = format!("zsa{}", fx.rand());
    let st = fx.pdf(None);
    fx.annotation(AnnotationType::Highlight, &st, Some(&format!("x {word} y")));
    let s = search(&[
        ("resultLevel", "item", ""),
        ("annotationComment", "contains", &word),
    ]);
    same(run(&fx, &s), &[&st]);
}

/// searchTest.js:739 "should return a standalone attachment whose annotation has a tag, in a search for top-level items"
#[test]
fn cross_standalone_attachment_annotation_tag() {
    let (mut fx, _) = base();
    let tag = format!("zsat{}", fx.rand());
    let st = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Highlight, &st, None);
    fx.get(&an).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    let s = search(&[("resultLevel", "item", ""), ("tag", "is", &tag)]);
    same(run(&fx, &s), &[&st]);
}

/// searchTest.js:758 "shouldn't match an item by a tag on a child attachment in the trash"
#[test]
fn cross_tag_on_trashed_child() {
    let (mut fx, _) = base();
    let tag = format!("ztrash{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.get(&att).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    fx.get(&att).deleted = Some(true);
    let s = search(&[("resultLevel", "item", ""), ("tag", "is", &tag)]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:776 "shouldn't match an item by an annotation on a child attachment in the trash"
#[test]
fn cross_annotation_on_trashed_child() {
    let (mut fx, _) = base();
    let word = format!("ztrashann{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.annotation(
        AnnotationType::Highlight,
        &att,
        Some(&format!("x {word} y")),
    );
    fx.get(&att).deleted = Some(true);
    let s = search(&[
        ("resultLevel", "item", ""),
        ("annotationComment", "contains", &word),
    ]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:795 "should match a trashed item by a tag on its child attachment when searching the trash"
#[test]
fn cross_trash_tag_on_child() {
    let (mut fx, _) = base();
    let tag = format!("ztrash{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.get(&att).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    fx.get(&item).deleted = Some(true);
    let s = search(&[
        ("deleted", "true", ""),
        ("resultLevel", "item", ""),
        ("tag", "is", &tag),
    ]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:814 "should bind a same-entity group below a non-item result level"
#[test]
fn cross_same_entity_group() {
    let (mut fx, _) = base();
    let text = format!("zsame{}", fx.rand());
    let comment = format!("zsamec{}", fx.rand());
    let item = fx.titled("zsameitem");
    let both_att = fx.pdf(Some(&item));
    let both = fx.annotation(AnnotationType::Highlight, &both_att, Some(&comment));
    fx.ann(&both).text = Some(text.clone());
    let split_att = fx.pdf(Some(&item));
    let has_text = fx.annotation(AnnotationType::Highlight, &split_att, None);
    fx.ann(&has_text).text = Some(text.clone());
    fx.annotation(AnnotationType::Highlight, &split_att, Some(&comment));
    let s = search(&[
        ("resultLevel", "attachment", ""),
        ("groupStart", "true", ""),
        ("resultLevel", "annotation", ""),
        ("annotationText", "contains", &text),
        ("annotationComment", "contains", &comment),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &s), &[&both_att]);
}

/// searchTest.js:848 "should roll a tag on a descendant up to the result item"
#[test]
fn cross_roll_tag_up() {
    let (mut fx, _) = base();
    let tag = format!("zroll{}", fx.rand());
    let via_child = fx.titled("zrollchild");
    let att = fx.pdf(Some(&via_child));
    fx.get(&att).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    let t = tag.clone();
    let via_self = fx.item(move |i| {
        i.set_field(Field::Title, "zrollself");
        i.tags = vec![Tag {
            tag: t,
            tag_type: None,
        }];
    });
    fx.titled("zrollmiss");
    let s = search(&[("resultLevel", "item", ""), ("tag", "is", &tag)]);
    same(run(&fx, &s), &[&via_child, &via_self]);
}

/// searchTest.js:872 "should not propagate a tag down to descendant result items"
#[test]
fn cross_tag_not_down() {
    let (mut fx, _) = base();
    let tag = format!("zdown{}", fx.rand());
    let t = tag.clone();
    let item = fx.item(move |i| {
        i.set_field(Field::Title, "zdownitem");
        i.tags = vec![Tag {
            tag: t,
            tag_type: None,
        }];
    });
    fx.pdf(Some(&item));
    let s = search(&[("resultLevel", "attachment", ""), ("tag", "is", &tag)]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:888 "should not error when a condition can't reach the result level"
#[test]
fn cross_unreachable_level() {
    let (fx, _) = base();
    let s = search(&[
        ("resultLevel", "attachment", ""),
        ("joinMode", "any", ""),
        ("note", "contains", "zsqltest"),
    ]);
    assert!(run(&fx, &s).is_empty());
    let s2 = search(&[
        ("resultLevel", "attachment", ""),
        ("note", "contains", "zsqltest"),
    ]);
    assert!(run(&fx, &s2).is_empty());
}

/// searchTest.js:907 "should project to all descendant annotations when there's no annotation condition"
#[test]
fn cross_project_to_annotations() {
    let (mut fx, _) = base();
    let item = fx.item(|i| i.creators = vec![author("Zprojsmith")]);
    let att = fx.pdf(Some(&item));
    let a1 = fx.annotation(AnnotationType::Highlight, &att, None);
    let a2 = fx.annotation(AnnotationType::Highlight, &att, None);
    let s = search(&[
        ("resultLevel", "annotation", ""),
        ("creator", "contains", "Zprojsmith"),
    ]);
    same(run(&fx, &s), &[&a1, &a2]);
}

// ---------------------------------------------------------------- collection

/// searchTest.js:928 "should find item in collection"
#[test]
fn collection_find_item() {
    let mut fx = Fx::new();
    let col = fx.collection(None);
    let c = col.clone();
    let item = fx.item(move |i| i.collections = vec![c]);
    same(run(&fx, &search(&[("collection", "is", &col)])), &[&item]);
}

/// searchTest.js:939 "should find items not in collection"
#[test]
fn collection_items_not_in() {
    let mut fx = Fx::new();
    let col = fx.collection(None);
    let c = col.clone();
    let item = fx.item(move |i| i.collections = vec![c]);
    assert!(!run(&fx, &search(&[("collection", "isNot", &col)])).contains(&item));
}

/// searchTest.js:950 "shouldn't find item in collection with no items"
#[test]
fn collection_empty() {
    let mut fx = Fx::new();
    let col = fx.collection(None);
    fx.item(|_| {});
    assert!(run(&fx, &search(&[("collection", "is", &col)])).is_empty());
}

/// searchTest.js:961 "should find item in subcollection in recursive mode"
#[test]
fn collection_recursive() {
    let mut fx = Fx::new();
    let c1 = fx.collection(None);
    let c2 = fx.collection(Some(&c1));
    let c = c2.clone();
    let item = fx.item(move |i| i.collections = vec![c]);
    let s = search(&[("collection", "is", &c1), ("recursive", "true", "")]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:974 "should return no results for a collection that doesn't exist in recursive mode"
#[test]
fn collection_missing_recursive() {
    let mut fx = Fx::new();
    fx.item(|_| {});
    let k = fx.key();
    let s = search(&[("collection", "is", &k), ("recursive", "true", "")]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:986 "should have same result after the same search conditions is removed and added"
#[test]
fn collection_remove_and_add_condition() {
    let mut fx = Fx::new();
    let t1 = format!("zremadd{}", fx.rand());
    let t2 = format!("zremadd{}", fx.rand());
    let one = fx.titled(&t1);
    let two = fx.titled(&t2);
    let mut s = search(&[
        ("joinMode", "any", ""),
        ("title", "contains", &t1),
        ("title", "contains", &t2),
    ]);
    same(run(&fx, &s), &[&one, &two]);
    s.remove_condition(1).unwrap();
    s.add_condition("title", "contains", &t1).unwrap();
    same(run(&fx, &s), &[&one, &two]);
}

// ---------------------------------------------------------------- tags and counts

/// searchTest.js:1014 "should match annotation with tag"
#[test]
fn tag_matches_annotation() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let tag = fx.rand();
    fx.get(&an).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    same(run(&fx, &search(&[("tag", "is", &tag)])), &[&an]);
}

/// searchTest.js:1030 "should match by tag count"
#[test]
fn num_tags() {
    let (mut fx, _) = base();
    let i1 = fx.item(|_| {});
    let (a, b) = (fx.rand(), fx.rand());
    let i2 = fx.item(move |i| {
        i.tags = vec![
            Tag {
                tag: a,
                tag_type: None,
            },
            Tag {
                tag: b,
                tag_type: None,
            },
        ]
    });
    let m = run(&fx, &search(&[("numTags", "is", "0")]));
    assert!(m.contains(&i1) && !m.contains(&i2));
    let m = run(&fx, &search(&[("numTags", "is", "2")]));
    assert!(m.contains(&i2) && !m.contains(&i1));
    let m = run(&fx, &search(&[("numTags", "isGreaterThan", "1")]));
    assert!(m.contains(&i2) && !m.contains(&i1));
    let m = run(&fx, &search(&[("numTags", "isLessThan", "1")]));
    assert!(m.contains(&i1) && !m.contains(&i2));
}

/// searchTest.js:1068 "should match top-level items by child note and attachment counts"
#[test]
fn num_notes_and_attachments() {
    let (mut fx, _) = base();
    let i1 = fx.item(|_| {});
    let i2 = fx.item(|_| {});
    let note = fx.note(Some(&i2), "foo");
    let att = fx.attachment(
        Some(&i2),
        "test.png",
        LinkMode::ImportedFile,
        "image/png",
        None,
    );
    let m = run(&fx, &search(&[("numNotes", "is", "0")]));
    assert!(m.contains(&i1));
    assert!(!m.contains(&i2));
    assert!(!m.contains(&note));
    assert!(!m.contains(&att));
    let m = run(&fx, &search(&[("numNotes", "is", "1")]));
    assert!(m.contains(&i2) && !m.contains(&i1));
    let m = run(&fx, &search(&[("numAttachments", "is", "1")]));
    assert!(m.contains(&i2) && !m.contains(&i1) && !m.contains(&att));
    fx.get(&note).deleted = Some(true);
    let m = run(&fx, &search(&[("numNotes", "is", "0")]));
    assert!(m.contains(&i2));
}

/// searchTest.js:1112 "should count annotations on an attachment and under a top-level item"
#[test]
fn num_annotations() {
    let (mut fx, _) = base();
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let plain = fx.item(|_| {});
    let m = run(&fx, &search(&[("numAnnotations", "is", "1")]));
    assert!(m.contains(&att) && m.contains(&item));
    assert!(!m.contains(&plain) && !m.contains(&an));
    let m = run(&fx, &search(&[("numAnnotations", "is", "0")]));
    assert!(m.contains(&plain));
    assert!(!m.contains(&item) && !m.contains(&att) && !m.contains(&an));
}

// ---------------------------------------------------------------- is / empty / dates

/// searchTest.js:1140 "should match a value in a different case"
#[test]
fn is_different_case() {
    let (mut fx, _) = base();
    let item = fx.typed(ItemType::JournalArticle, |i| {
        i.set_field(Field::PublicationTitle, "Review of Finance")
    });
    let s = search(&[("publicationTitle", "is", "review of finance")]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:1151 "should match a value with different accents"
#[test]
fn is_different_accents() {
    let (mut fx, _) = base();
    let item = fx.typed(ItemType::JournalArticle, |i| {
        i.set_field(Field::PublicationTitle, "Revue de Séance")
    });
    let s = search(&[("publicationTitle", "is", "revue de seance")]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:1164 "should match items with an empty or non-empty text field"
#[test]
fn empty_text_field() {
    let (mut fx, _) = base();
    let i1 = fx.item(|_| {});
    let i2 = fx.item(|i| i.set_field(Field::Place, "Berlin"));
    let m = run(&fx, &search(&[("place", "isEmpty", "")]));
    assert!(m.contains(&i1) && !m.contains(&i2));
    let m = run(&fx, &search(&[("place", "isNotEmpty", "")]));
    assert!(m.contains(&i2) && !m.contains(&i1));
}

/// searchTest.js:1185 "should match items with an empty date field"
#[test]
fn empty_date_field() {
    let (mut fx, _) = base();
    let i1 = fx.item(|_| {});
    let i2 = fx.item(|i| i.set_field(Field::Date, "2020-01-01"));
    let m = run(&fx, &search(&[("date", "isEmpty", "")]));
    assert!(m.contains(&i1) && !m.contains(&i2));
}

/// searchTest.js:1201 "should handle 'today'"
#[test]
fn date_added_today() {
    let (mut fx, _) = base();
    let item = fx.item(|_| {});
    assert!(run(&fx, &search(&[("dateAdded", "is", "today")])).contains(&item));
    assert!(run(&fx, &search(&[("dateAdded", "is", "yesterday")])).is_empty());
}

/// searchTest.js:1222 "should compare dates and still accept text operators"
#[test]
fn date_type_fields() {
    let (mut fx, _) = base();
    let filing = Field::from_name("filingDate").unwrap();
    let i1 = fx.typed(ItemType::Patent, |i| i.set_field(filing, "2019-06-08"));
    let i2 = fx.typed(ItemType::Patent, |i| i.set_field(filing, "2021-01-15"));
    let i3 = fx.typed(ItemType::Patent, |i| i.set_field(filing, "Foo"));
    let m = run(&fx, &search(&[("filingDate", "isBefore", "2020")]));
    assert!(m.contains(&i1) && !m.contains(&i2) && !m.contains(&i3));
    let m = run(&fx, &search(&[("filingDate", "contains", "2021")]));
    assert!(m.contains(&i2) && !m.contains(&i1));
}

/// searchTest.js:1254 "should search by attachment file type"
#[test]
fn file_type_id() {
    let (fx, b) = base();
    // Zotero.FileTypes.getID('webpage') = 1 (resource/schema/system.sql:113)
    same(
        run(&fx, &search(&[("fileTypeID", "is", "1")])),
        &[&b.foo, &b.foobar],
    );
}

/// searchTest.js:1264 "should find stored files"
#[test]
fn storage_stored_files() {
    let (fx, b) = base();
    let m = run(
        &fx,
        &search(&[("attachmentStorageType", "is", "storedFile")]),
    );
    for k in [&b.foo, &b.foobar, &b.baz, &b.imported_url] {
        assert!(m.contains(k));
    }
    assert!(!m.contains(&b.linked_file) && !m.contains(&b.linked_url));
}

/// searchTest.js:1279 "should find linked files"
#[test]
fn storage_linked_files() {
    let (fx, b) = base();
    let m = run(
        &fx,
        &search(&[("attachmentStorageType", "is", "linkedFile")]),
    );
    assert!(m.contains(&b.linked_file) && !m.contains(&b.baz) && !m.contains(&b.linked_url));
}

/// searchTest.js:1289 "should find web links"
#[test]
fn storage_web_links() {
    let (fx, b) = base();
    let m = run(&fx, &search(&[("attachmentStorageType", "is", "webLink")]));
    assert!(m.contains(&b.linked_url) && !m.contains(&b.baz) && !m.contains(&b.linked_file));
}

/// searchTest.js:1299 "should match a top-level item via a child attachment"
#[test]
fn storage_top_level_via_child() {
    let (mut fx, _) = base();
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    let m = run(
        &fx,
        &search(&[
            ("resultLevel", "item", ""),
            ("attachmentStorageType", "is", "storedFile"),
        ]),
    );
    assert!(m.contains(&item) && !m.contains(&att));
}

/// searchTest.js:1316 "should roll a child attachment's last-read date up to its top-level item"
#[test]
fn last_read_rolls_up() {
    let (mut fx, _) = base();
    let item = fx.titled("zlastread");
    let att = fx.pdf(Some(&item));
    fx.get(&att).attachment.as_mut().unwrap().last_read = Some(NOW);
    let s = search(&[
        ("resultLevel", "item", ""),
        ("lastRead", "isInTheLast", "1 days"),
    ]);
    same(run(&fx, &s), &[&item]);
}

// ---------------------------------------------------------------- fulltextContent

/// searchTest.js:1333 "should find text in HTML files"
#[test]
fn fulltext_html() {
    let (fx, b) = base();
    let s = search(&[("fulltextContent", "contains", "foo bar")]);
    same(run(&fx, &s), &[&b.foobar]);
}

/// searchTest.js:1341 "should work in subsearch"
#[test]
fn fulltext_subsearch() {
    let (fx, b) = base();
    let s = search(&[("fulltextContent", "contains", "foo bar")]);
    let mut s2 = search(&[("title", "contains", "foobar")]);
    s2.set_scope(s, false);
    same(run(&fx, &s2), &[&b.foobar]);
}

/// searchTest.js:1353 "should find matching items with joinMode=ANY with no other conditions"
#[test]
fn fulltext_any_no_other() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent", "contains", "foo"),
        ("fulltextContent", "contains", "bar"),
    ]);
    same(run(&fx, &s), &[&b.foo, &b.foobar]);
}

/// searchTest.js:1363 "should find matching items with joinMode=ANY and non-matching other condition"
#[test]
fn fulltext_any_nonmatching_other() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent", "contains", "foo"),
        ("fulltextContent", "contains", "bar"),
        ("title", "contains", "nomatch"),
    ]);
    same(run(&fx, &s), &[&b.foo, &b.foobar]);
}

/// searchTest.js:1374 "should find matching items in regexp mode with joinMode=ANY with matching other condition"
#[test]
fn fulltext_regexp_any_matching_other() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent/regexp", "contains", "foo.+bar"),
        ("title", "is", "foo.html"),
    ]);
    same(run(&fx, &s), &[&b.foo, &b.foobar]);
}

/// searchTest.js:1384 "should find matching item in regexp mode with joinMode=ANY and non-matching other condition"
#[test]
fn fulltext_regexp_any_nonmatching_other() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent/regexp", "contains", "foo.+bar"),
        ("title", "contains", "nomatch"),
    ]);
    same(run(&fx, &s), &[&b.foobar]);
}

/// searchTest.js:1394 "should find item matching other condition in regexp mode when joinMode=ANY"
#[test]
fn fulltext_regexp_any_other_matches() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent/regexp", "contains", "nomatch"),
        ("title", "is", "foobar.html"),
    ]);
    same(run(&fx, &s), &[&b.foobar]);
}

/// searchTest.js:1404 "should find matching item in regexp mode with joinMode=ANY and recursive mode flag"
#[test]
fn fulltext_regexp_any_recursive() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent/regexp", "contains", "foo.+bar"),
        ("recursive", "true", ""),
    ]);
    same(run(&fx, &s), &[&b.foobar]);
}

/// searchTest.js:1414 "should find items that don't contain a single word with joinMode=ANY"
#[test]
fn fulltext_not_word() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent", "doesNotContain", "foo"),
    ]);
    let m = run(&fx, &s);
    assert!(!m.contains(&b.foo) && !m.contains(&b.foobar));
}

/// searchTest.js:1423 "should find items that don't contain a phrase with joinMode=ANY"
#[test]
fn fulltext_not_phrase() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent", "doesNotContain", "foo bar"),
    ]);
    assert!(!run(&fx, &s).contains(&b.foobar));
}

/// searchTest.js:1432 "should find items that don't contain a regexp pattern with joinMode=ANY"
#[test]
fn fulltext_not_regexp() {
    let (fx, b) = base();
    let s = search(&[
        ("joinMode", "any", ""),
        ("fulltextContent/regexp", "doesNotContain", "foo.+bar"),
    ]);
    let m = run(&fx, &s);
    assert!(!m.contains(&b.foobar));
    assert!(m.contains(&b.foo) && m.contains(&b.baz));
}

/// searchTest.js:1445 "should obey the group join mode for a grouped fulltextContent"
#[test]
fn fulltext_group_join_mode() {
    let (fx, b) = base();
    let or = search(&[
        ("groupStart", "true", ""),
        ("joinMode", "any", ""),
        ("title", "is", "foo.html"),
        ("fulltextContent", "contains", "foo bar"),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &or), &[&b.foo, &b.foobar]);
    let and = search(&[
        ("joinMode", "any", ""),
        ("title", "is", "foo.html"),
        ("groupStart", "true", ""),
        ("joinMode", "all", ""),
        ("title", "is", "foobar.html"),
        ("fulltextContent", "contains", "nomatchphrase"),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &and), &[&b.foo]);
}

/// searchTest.js:1470 "should compose a grouped fulltextContent in doesNotContain and regexp modes"
#[test]
fn fulltext_group_negation_and_regexp() {
    let (fx, b) = base();
    let neg = search(&[
        ("groupStart", "true", ""),
        ("joinMode", "all", ""),
        ("fulltextContent", "doesNotContain", "foo"),
        ("title", "is", "baz.pdf"),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &neg), &[&b.baz]);
    let re = search(&[
        ("groupStart", "true", ""),
        ("joinMode", "any", ""),
        ("title", "is", "foo.html"),
        ("fulltextContent/regexp", "contains", "foo.+bar"),
        ("groupEnd", "true", ""),
    ]);
    same(run(&fx, &re), &[&b.foo, &b.foobar]);
}

// ---------------------------------------------------------------- annotations

/// searchTest.js:1494 "should return matches for annotation text"
#[test]
fn annotation_text() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let s7: String = fx.ann(&an).text.clone().unwrap().chars().take(7).collect();
    let s = search(&[("joinMode", "any", ""), ("annotationText", "contains", &s7)]);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:1509 "should return matches for annotation comment"
#[test]
fn annotation_comment() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Note, &att, None);
    let s7: String = fx
        .ann(&an)
        .comment
        .clone()
        .unwrap()
        .chars()
        .take(7)
        .collect();
    let s = search(&[
        ("joinMode", "any", ""),
        ("annotationComment", "contains", &s7),
    ]);
    same(run(&fx, &s), &[&an]);
}

// ---------------------------------------------------------------- parents and children

/// searchTest.js:1524 "should handle ANY search with no-op condition"
#[test]
fn ipc_any_noop() {
    let (mut fx, _) = base();
    let r = fx.rand();
    let s = search(&[
        ("joinMode", "any", ""),
        ("savedSearch", "is", &r),
        ("includeParentsAndChildren", "true", ""),
    ]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:1535 "should handle ANY search with two no-op conditions"
#[test]
fn ipc_any_two_noops() {
    let (mut fx, _) = base();
    let (r1, r2) = (fx.rand(), fx.rand());
    let s = search(&[
        ("joinMode", "any", ""),
        ("savedSearch", "is", &r1),
        ("savedSearch", "is", &r2),
        ("includeParentsAndChildren", "true", ""),
    ]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:1547 "should include a match's parents and children"
#[test]
fn ipc_parents_and_children() {
    let (mut fx, _) = base();
    let it_title = format!("zincp{}", fx.rand());
    let at_title = format!("zinca{}", fx.rand());
    let item = fx.titled(&it_title);
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Title, at_title.clone());
    let s = |v: &str| {
        search(&[
            ("title", "is", v),
            ("includeParentsAndChildren", "true", ""),
        ])
    };
    same(run(&fx, &s(&it_title)), &[&item, &att]);
    same(run(&fx, &s(&at_title)), &[&item, &att]);
}

/// searchTest.js:1569 "shouldn't include the parent of a match in the trash"
#[test]
fn ipc_trashed_child() {
    let (mut fx, _) = base();
    let at_title = format!("zincd{}", fx.rand());
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Title, at_title.clone());
    fx.get(&att).deleted = Some(true);
    let s = search(&[
        ("title", "is", &at_title),
        ("includeParentsAndChildren", "true", ""),
    ]);
    assert!(run(&fx, &s).is_empty());
}

/// searchTest.js:1588 "should keep only top-level items"
#[test]
fn no_children() {
    let (mut fx, _) = base();
    let title = format!("znochild{}", fx.rand());
    let item = fx.titled(&title);
    let att = fx.pdf(Some(&item));
    fx.get(&att).set_field(Field::Title, title.clone());
    let s = search(&[("title", "contains", &title), ("noChildren", "true", "")]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:1607 "should allow more than max bound parameters"
/// (Zotero.DB.MAX_BOUND_PARAMETERS is 999, so 1099 conditions; upstream
/// only asserts that the search runs.)
#[test]
fn key_many_conditions() {
    let (mut fx, _) = base();
    let mut s = Search::new();
    for _ in 0..1099 {
        let k = fx.key();
        s.add_condition("key", "is", &k).unwrap();
    }
    assert!(s.run(&fx.lib()).is_ok());
}

// ---------------------------------------------------------------- anyField / titleCreatorYear

/// searchTest.js:1618 "should expand an 'any field' within its own group"
#[test]
fn any_field_own_group() {
    let (mut fx, _) = base();
    let item = fx.titled("znestfoo");
    let s = search(&[
        ("groupStart", "true", ""),
        ("joinMode", "any", ""),
        ("anyField", "contains", "znestfoo"),
        ("title", "contains", "zzznomatch"),
        ("groupEnd", "true", ""),
    ]);
    assert!(run(&fx, &s).contains(&item));
}

/// searchTest.js:1637 "should return matches for multiple 'any field' conditions with joinMode=any"
#[test]
fn any_field_multiple_any() {
    let (mut fx, _) = base();
    let t1 = format!("zanyj{}", fx.rand());
    let t2 = format!("zanyk{}", fx.rand());
    let one = fx.titled(&t1);
    let two = fx.titled(&t2);
    let s = search(&[
        ("joinMode", "any", ""),
        ("anyField", "contains", &t1),
        ("anyField", "contains", &t2),
    ]);
    same(run(&fx, &s), &[&one, &two]);
}

/// searchTest.js:1651 "should return matches for 'any field' and title condition with joinMode=any"
#[test]
fn any_field_and_title_any() {
    let (mut fx, _) = base();
    let t1 = format!("zanyt{}", fx.rand());
    let t2 = format!("zanyu{}", fx.rand());
    let one = fx.titled(&t1);
    let two = fx.titled(&t2);
    let s = search(&[
        ("joinMode", "any", ""),
        ("anyField", "contains", &t1),
        ("title", "contains", &t2),
    ]);
    same(run(&fx, &s), &[&one, &two]);
}

/// searchTest.js:1665 "should return matches for a single 'any field' condition"
#[test]
fn any_field_single() {
    let (mut fx, _) = base();
    let t = format!("zanys{}", fx.rand());
    let one = fx.titled(&t);
    fx.item(|_| {});
    same(run(&fx, &search(&[("anyField", "contains", &t)])), &[&one]);
}

/// searchTest.js:1675 "should return matches for two 'any field' condition with joinMode=all"
#[test]
fn any_field_two_all() {
    let (mut fx, _) = base();
    let w1 = format!("zanyv{}", fx.rand());
    let w2 = format!("zanyw{}", fx.rand());
    let one = fx.titled(&format!("{w1}-{w2}"));
    let two = fx.titled(&format!("{w2}-{w1}"));
    let s = search(&[
        ("anyField", "contains", &w1),
        ("anyField", "contains", &w2),
        ("joinMode", "all", ""),
    ]);
    same(run(&fx, &s), &[&one, &two]);
}

/// searchTest.js:1689 "should return matches for annotation text" (anyField)
#[test]
fn any_field_annotation_text() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let s7: String = fx.ann(&an).text.clone().unwrap().chars().take(7).collect();
    assert!(run(&fx, &search(&[("anyField", "contains", &s7)])).contains(&an));
}

/// searchTest.js:1700 "should return matches for annotation comment" (anyField)
#[test]
fn any_field_annotation_comment() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Note, &att, None);
    let s7: String = fx
        .ann(&an)
        .comment
        .clone()
        .unwrap()
        .chars()
        .take(7)
        .collect();
    assert!(run(&fx, &search(&[("anyField", "contains", &s7)])).contains(&an));
}

/// searchTest.js:1713 "should match title, creator, and year but not other fields"
#[test]
fn title_creator_year_fields() {
    let (mut fx, _) = base();
    let word = format!("ztcy{}", fx.rand());
    let by_title = fx.titled(&format!("a {word} b"));
    let w = word.clone();
    let by_creator =
        fx.item(move |i| i.creators = vec![Creator::person(CreatorType::Author, "X", w)]);
    let w = word.clone();
    fx.item(move |i| i.set_field(Field::AbstractNote, format!("a {w} b")));
    let s = search(&[("titleCreatorYear", "contains", &word)]);
    same(run(&fx, &s), &[&by_title, &by_creator]);
}

/// searchTest.js:1733 "should exclude items whose title contains the value for doesNotContain"
#[test]
fn title_creator_year_does_not_contain() {
    let (mut fx, _) = base();
    let word = format!("ztcy{}", fx.rand());
    let with = fx.titled(&format!("a {word} b"));
    let without = fx.titled("nothing here");
    let m = run(
        &fx,
        &search(&[("titleCreatorYear", "doesNotContain", &word)]),
    );
    assert!(m.contains(&without) && !m.contains(&with));
}

// ---------------------------------------------------------------- savedSearch

/// searchTest.js:1753 "should return items in the saved search"
#[test]
fn saved_search_is() {
    let mut fx = Fx::new();
    let (k, title) = fx.saved_search();
    let item = fx.titled(&title);
    assert_eq!(run(&fx, &search(&[("savedSearch", "is", &k)])), vec![item]);
}

/// searchTest.js:1765 "should return items not in the saved search for isNot operator"
#[test]
fn saved_search_is_not() {
    let mut fx = Fx::new();
    let (k, title) = fx.saved_search();
    let item = fx.titled(&title);
    assert!(!run(&fx, &search(&[("savedSearch", "isNot", &k)])).contains(&item));
}

/// searchTest.js:1777 "should return no results for a search that doesn't exist"
#[test]
fn saved_search_missing() {
    let mut fx = Fx::new();
    fx.item(|_| {});
    let k = fx.key();
    assert!(run(&fx, &search(&[("savedSearch", "is", &k)])).is_empty());
}

// ---------------------------------------------------------------- unfiled

/// searchTest.js:1790 "shouldn't include items in My Publications"
#[test]
fn unfiled_not_publications() {
    let mut fx = Fx::new();
    let i1 = fx.item(|_| {});
    let i2 = fx.item(|i| i.in_publications = Some(true));
    let m = run(&fx, &search(&[("unfiled", "true", "")]));
    assert!(m.contains(&i1) && !m.contains(&i2));
}

/// searchTest.js:1801 "should include items belonging only to trashed collections"
#[test]
fn unfiled_trashed_collection() {
    let mut fx = Fx::new();
    let col = fx.collection(None);
    let c = col.clone();
    let item = fx.item(move |i| i.collections = vec![c]);
    let s = search(&[("unfiled", "true", "")]);
    assert!(!run(&fx, &s).contains(&item));
    fx.collections[0].deleted = Some(true);
    assert!(run(&fx, &s).contains(&item));
}

// ---------------------------------------------------------------- accent-insensitive

fn title_matches(fx: &Fx, term: &str) -> Vec<String> {
    run(fx, &search(&[("title", "contains", term)]))
}

/// searchTest.js:1823 "should match accented and unaccented text interchangeably"
#[test]
fn accents_interchangeable() {
    let mut fx = Fx::new();
    let item = fx.titled("zdiaséance");
    same(title_matches(&fx, "zdiaseance"), &[&item]);
    let item2 = fx.titled("zdiaresume");
    same(title_matches(&fx, "zdiarésumé"), &[&item2]);
}

/// searchTest.js:1839 "should match an accented creator from an unaccented term"
#[test]
fn accents_creator() {
    let mut fx = Fx::new();
    let item = fx.item(|i| {
        i.creators = vec![Creator::person(
            CreatorType::Author,
            "Étiennezdia",
            "Müllerzdia",
        )]
    });
    let s = search(&[("creator", "contains", "etiennezdia mullerzdia")]);
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:1850 "should match an accented tag from an unaccented term"
#[test]
fn accents_tag() {
    let mut fx = Fx::new();
    let item = fx.item(|i| {
        i.tags = vec![Tag {
            tag: "résumézdia".into(),
            tag_type: None,
        }]
    });
    same(
        run(&fx, &search(&[("tag", "contains", "resumezdia")])),
        &[&item],
    );
}

/// searchTest.js:1861 "should match non-ASCII text case-insensitively"
#[test]
fn accents_non_ascii_case() {
    let mut fx = Fx::new();
    let greek = fx.titled("zdiaΘΕΜΑ");
    let cyr = fx.titled("zdiaПРИВЕТ");
    same(title_matches(&fx, "zdiaθεμα"), &[&greek]);
    same(title_matches(&fx, "zdiaпривет"), &[&cyr]);
}

/// searchTest.js:1876 "should match ligatures, superscripts, fractions, and non-decomposing letters"
#[test]
fn accents_ligatures() {
    let mut fx = Fx::new();
    let item = fx.titled("zdiasøren œuvre ﬁle x² ½ straße");
    same(
        title_matches(&fx, "zdiasoren oeuvre file x2 1/2 strasse"),
        &[&item],
    );
}

/// searchTest.js:1884 "should ignore rich-text formatting tags in item fields"
#[test]
fn accents_rich_text() {
    let mut fx = Fx::new();
    let item =
        fx.titled("The <span style=\"font-variant:small-caps;\">zdiadrosophila</span> genome");
    same(title_matches(&fx, "zdiadrosophila genome"), &[&item]);
    assert!(title_matches(&fx, "small-caps").is_empty());
}

/// searchTest.js:1903 "should fold curly quotes and dashes to their ASCII forms"
#[test]
fn accents_quotes_dashes() {
    let mut fx = Fx::new();
    let quoted = fx.titled("zdia\u{201c}pp. 10\u{2013}12\u{201d}");
    let em = fx.titled("zdiamind\u{2014}body problem");
    same(title_matches(&fx, "zdia\"pp. 10-12\""), &[&quoted]);
    same(title_matches(&fx, "zdiamind-body"), &[&em]);
}

/// searchTest.js:1926 "should distinguish voiced kana and composed Hangul"
#[test]
fn accents_kana_hangul() {
    let mut fx = Fx::new();
    let ja = fx.titled("zdiaがん");
    let ko = fx.titled("zdia한국");
    same(title_matches(&fx, "zdiaがん"), &[&ja]);
    same(title_matches(&fx, "zdia한국"), &[&ko]);
    assert!(title_matches(&fx, "zdiaかん").is_empty());
    assert!(title_matches(&fx, "zdia하").is_empty());
}

// ---------------------------------------------------------------- quick search

/// searchTest.js:1968 "should match annotation for tag search"
#[test]
fn quick_fields_annotation_tag() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let tag = fx.rand();
    fx.get(&an).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    let s = search(&[("quicksearch-fields", "contains", &tag)]);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:1982 "should match annotation of top-level attachment within scope"
#[test]
fn quick_fields_scope_top_level_attachment() {
    let (mut fx, _) = base();
    let col = fx.collection(None);
    let att = fx.pdf(None);
    fx.get(&att).collections = vec![col.clone()];
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let tag = fx.rand();
    fx.get(&an).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    let scope = search(&[("noChildren", "true", ""), ("collectionID", "is", &col)]);
    let mut s = search(&[("quicksearch-fields", "contains", &tag)]);
    s.set_scope(scope, true);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:2011 "should match annotation of a child attachment of an item within scope"
#[test]
fn quick_fields_scope_child_attachment() {
    let (mut fx, _) = base();
    let col = fx.collection(None);
    let c = col.clone();
    let item = fx.item(move |i| i.collections = vec![c]);
    let att = fx.pdf(Some(&item));
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let tag = fx.rand();
    fx.get(&an).tags = vec![Tag {
        tag: tag.clone(),
        tag_type: None,
    }];
    let scope = search(&[("noChildren", "true", ""), ("collectionID", "is", &col)]);
    let mut s = search(&[("quicksearch-fields", "contains", &tag)]);
    s.set_scope(scope, true);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:2043 "should match parent attachment for annotation comment"
#[test]
fn quick_everything_annotation_comment() {
    let (mut fx, _) = base();
    let att = fx.pdf(None);
    let an = fx.annotation(AnnotationType::Highlight, &att, None);
    let comment = fx.ann(&an).comment.clone().unwrap();
    let s = search(&[("quicksearch-everything", "contains", &comment)]);
    same(run(&fx, &s), &[&an]);
}

/// searchTest.js:2055 "should not include items outside of scope during phrase search"
#[test]
fn quick_everything_phrase_in_scope() {
    let (mut fx, b) = base();
    let col = fx.collection(None);
    fx.get(&b.foo).collections = vec![col.clone()];
    let scope = search(&[("noChildren", "true", ""), ("collectionID", "is", &col)]);
    let mut s = search(&[("quicksearch-everything", "contains", "\"foo\"")]);
    s.set_scope(scope, true);
    assert_eq!(run(&fx, &s), vec![b.foo.clone()]);
}

/// `importTextContent` (searchTest.js:2077): a stored text/plain file
/// titled "xxxx".
fn text_attachment(fx: &mut Fx, content: &str) -> String {
    fx.attachment(
        None,
        "xxxx",
        LinkMode::ImportedFile,
        "text/plain",
        Some(content),
    )
}

fn everything(fx: &Fx, term: &str) -> Vec<String> {
    run(fx, &search(&[("quicksearch-everything", "contains", term)]))
}

/// searchTest.js:2085 "should match full-text content by word prefix"
#[test]
fn quick_everything_word_prefix() {
    let (mut fx, _) = base();
    let item = text_attachment(&mut fx, "zqs electrophoresis protocol");
    assert!(everything(&fx, "electro").contains(&item));
}

/// searchTest.js:2093 "should match unquoted words as prefixes anywhere in the content"
#[test]
fn quick_everything_prefixes_anywhere() {
    let (mut fx, _) = base();
    let item = text_attachment(&mut fx, "the climate of the region has seen many changes");
    assert!(everything(&fx, "clim chang").contains(&item));
}

/// searchTest.js:2101 "should require every word of a multi-word search"
#[test]
fn quick_everything_every_word() {
    let (mut fx, _) = base();
    let both = text_attachment(&mut fx, "zqsalpha zqsbeta zqsgamma");
    let one = text_attachment(&mut fx, "zqsalpha zqsdelta");
    let m = everything(&fx, "zqsalpha zqsgamma");
    assert!(m.contains(&both) && !m.contains(&one));
}

/// searchTest.js:2112 "should skip content matching for a term too short for the index"
#[test]
fn quick_everything_short_term() {
    let (mut fx, _) = base();
    let item = text_attachment(&mut fx, "elephant");
    assert!(!everything(&fx, "el").contains(&item));
}

/// searchTest.js:2122 "should skip note matching for a short term but honor a quoted one"
#[test]
fn quick_everything_note_short_term() {
    let (mut fx, _) = base();
    let note = fx.note(None, "<p>zqsnote re content</p>");
    assert!(everything(&fx, "zqsnote").contains(&note));
    assert!(!everything(&fx, "re").contains(&note));
    assert!(everything(&fx, "\"re\"").contains(&note));
}

// ---------------------------------------------------------------- deleted

/// searchTest.js:2147 "should not match regular items in trash with annotated child attachments"
#[test]
fn deleted_parent_in_trash() {
    let (mut fx, _) = base();
    let item = fx.item(|i| i.deleted = Some(true));
    let att = fx.pdf(Some(&item));
    fx.annotation(AnnotationType::Highlight, &att, None);
    assert!(!run(&fx, &Search::new()).contains(&att));
}

/// searchTest.js:2160 "should not match regular items with annotated child attachments in trash"
#[test]
fn deleted_child_in_trash() {
    let (mut fx, _) = base();
    let item = fx.item(|_| {});
    let att = fx.pdf(Some(&item));
    fx.get(&att).deleted = Some(true);
    fx.annotation(AnnotationType::Highlight, &att, None);
    assert!(!run(&fx, &Search::new()).contains(&att));
}

// ---------------------------------------------------------------- JSON

/// searchTest.js:2198 "should output all data"
#[test]
fn to_json_all_data() {
    let mut s = Search::new();
    s.set_name("Test").unwrap();
    s.add_condition("joinMode", "any", "").unwrap();
    s.add_condition("fulltextContent/regexp", "contains", "s.+")
        .unwrap();
    let j = s.to_json();
    assert_eq!(j["name"], "Test");
    let c = j["conditions"].as_array().unwrap();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0]["condition"], "joinMode");
    assert_eq!(c[0]["operator"], "any");
    assert_eq!(c[0]["value"], "");
    for k in ["id", "required", "mode"] {
        assert!(c[0].get(k).is_none());
        assert!(c[1].get(k).is_none());
    }
    assert_eq!(c[1]["condition"], "fulltextContent/regexp");
    assert_eq!(c[1]["operator"], "contains");
    assert_eq!(c[1]["value"], "s.+");
}

/// searchTest.js:2224 "should migrate a `childNote` condition to `note` at the item level"
/// (also covers "Loading ... should migrate a stored `childNote`",
/// searchTest.js:43, which reaches the same migration through the database)
#[test]
fn from_json_child_note() {
    let mut fx = Fx::new();
    let text = format!("zjsoncn{}", fx.rand());
    let item = fx.titled("zjsoncnitem");
    fx.note(Some(&item), &format!("<p>{text}</p>"));
    let mut s = Search::new();
    s.from_json(
        &serde_json::json!({
            "name": "Test",
            "conditions": [{ "condition": "childNote", "operator": "contains", "value": text }]
        }),
        false,
    )
    .unwrap();
    let names: Vec<&str> = s
        .conditions()
        .iter()
        .map(|c| c.condition.as_str())
        .collect();
    assert!(names.contains(&"note") && !names.contains(&"childNote"));
    let rl = s
        .conditions()
        .iter()
        .find(|c| c.condition == "resultLevel")
        .unwrap();
    assert_eq!(rl.operator.as_str(), "item");
    same(run(&fx, &s), &[&item]);
}

/// searchTest.js:2251 "should update all data"
#[test]
fn from_json_update() {
    let mut s = Search::new();
    s.set_name("Test").unwrap();
    s.add_condition("joinMode", "any", "").unwrap();
    s.add_condition("title", "isNot", "foo").unwrap();
    let mut j = s.to_json();
    j["name"] = "Test 2".into();
    j["conditions"] = serde_json::json!([
        { "condition": "title", "operator": "contains", "value": "foo" },
        { "condition": "year", "operator": "is", "value": "2016" }
    ]);
    s.from_json(&j, false).unwrap();
    assert_eq!(s.name(), Some("Test 2"));
    let c = s.conditions();
    assert_eq!(c.len(), 2);
    assert_eq!(
        (
            c[0].condition.as_str(),
            c[0].operator.as_str(),
            c[0].value.as_str()
        ),
        ("title", "contains", "foo")
    );
    assert_eq!(
        (
            c[1].condition.as_str(),
            c[1].operator.as_str(),
            c[1].value.as_str()
        ),
        ("year", "is", "2016")
    );
}

fn unknown_prop_json() -> serde_json::Value {
    serde_json::json!({
        "name": "Search",
        "conditions": [{ "condition": "title", "operator": "contains", "value": "foo" }],
        "foo": "Bar"
    })
}

/// searchTest.js:2282 "should ignore unknown property in non-strict mode"
#[test]
fn from_json_non_strict() {
    assert!(Search::new().from_json(&unknown_prop_json(), false).is_ok());
}

/// searchTest.js:2298 "should throw on unknown property in strict mode"
#[test]
fn from_json_strict() {
    match Search::new().from_json(&unknown_prop_json(), true) {
        Err(SearchError::Json(m)) => assert!(m.starts_with("Unknown search property")),
        other => panic!("expected an error, got {other:?}"),
    }
}

// ---------------------------------------------------------------- the items pane (not an upstream test)

/// `CollectionTreeRow.getSearchObject` (collectionTreeRow.js:406) in a
/// collection: kovan's own check that [`quick_search`] reproduces the
/// scope used by searchTest.js:1982-2039.
#[test]
fn items_pane_quick_search_collection() {
    let (mut fx, _) = base();
    let col = fx.collection(None);
    let c = col.clone();
    let item = fx.item(move |i| {
        i.set_field(Field::Title, "zpane alpha");
        i.collections = vec![c];
    });
    fx.titled("zpane alpha outside");
    let s = quick_search(
        "zpane",
        QuickSearchMode::TitleCreatorYear,
        &QuickSearchScope::Collection {
            key: col,
            recursive: false,
        },
        &[],
    )
    .unwrap();
    same(run(&fx, &s), &[&item]);
}
