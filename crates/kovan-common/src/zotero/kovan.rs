// Part of the kovan Zotero port (GitHub #747, #748).
//
// No upstream logic is ported in this file: it maps the Zotero item model
// (ported from Zotero, AGPL-3.0, Corporation for Digital Scholarship) onto
// kovan's own `KovanDocument`. It follows Zotero's conventions where it has
// to choose: a value whose field the item type lacks goes into Extra as a
// `Title Case Name: value` line (item.js:5859, `combineExtraFields`).

//! [`ZoteroItem`] <-> [`KovanDocument`].
//!
//! ## Zotero -> kovan ([`ZoteroItem::to_kovan_document`])
//!
//! | `KovanDocument` | from the Zotero item |
//! |---|---|
//! | `id` | `"zotero:" + key` (`"zotero:"` when the item has no key) |
//! | `slug` | `citationKey`, else `<first author family><year>`, lower-case ASCII |
//! | `visibility` | always `Proprietary` (Zotero records no redistribution right; `DATA_POLICY.md` makes "may not be committed" the safe default) |
//! | `document_type` | journal/magazine/newspaper article, conference paper, preprint -> `Paper`; `report` -> `Report`; `standard` -> `Standard`; `thesis` -> `Thesis`; every other type -> `Other` |
//! | `title` | `title` through its base field (`caseName`, `subject`, ...) |
//! | `authors` | creators of the type's **primary** creator type, in order; a single-field creator becomes `family` = name, `given` = "" (kovan's organisation convention) |
//! | `abstract_text` | `abstractNote` |
//! | `year` | the year of `date` (through its base field), when non-negative |
//! | `doi` | `DOI`, else a `DOI: ...` line of Extra |
//! | `journal` | `publicationTitle` through its base field (so `bookTitle`, `websiteTitle`, `proceedingsTitle`, ...) |
//! | `institution` | for `report`/`thesis`: the `publisher`-mapped field (`institution`, `university`) |
//! | `publisher` | for every other type: `publisher` through its base field |
//! | `volume`, `pages` | through their base fields |
//! | `number` | `issue` when the type has one, else `number` through its base field |
//! | `keywords` | automatic tags (`type: 1`) |
//! | `tags` | manual tags |
//! | `source_url` | `url` |
//! | `source_path` | `path` of the first `linked_file` child in `attachments` (export format) |
//! | `page_count` | `numPages`, when it is a plain integer |
//! | `zotero_item` | the whole item, verbatim |
//!
//! **Lost going Zotero -> kovan, except through `zotero_item`:** every
//! other field (`extra`, `language`, `series`, `ISSN`, `accessDate`, ...); the
//! full date (only the year is kept); non-primary creators (editors,
//! translators, ...); `key`/`version` beyond the id; collections, relations,
//! notes, attachments other than one path, annotations, `dateAdded`/
//! `dateModified`; and whether a `Paper` was a conference paper, a preprint
//! or a magazine article.
//!
//! Because the item is kept in `zotero_item`, Zotero -> kovan -> Zotero is
//! lossless: [`ZoteroItem::from_kovan_document`] starts from the stored item
//! and only overwrites what kovan holds (tested).
//!
//! ## kovan -> Zotero ([`ZoteroItem::from_kovan_document`])
//!
//! The reverse of the table, starting from `zotero_item` when present (its
//! item type is kept). Kovan values whose field the item type lacks go into
//! Extra (`Publication Title: ...`); a `DOI:` Extra line is read back.
//!
//! **Lost going kovan -> Zotero -> kovan:** `id` unless it is `zotero:<key>`;
//! `visibility` (comes back `Proprietary`); `Author::affiliation`;
//! `source_sha256`, `assets`, `related_symbols`, `related_repositories`,
//! `related_benchmarks`, `markdown_body`; `page_count` for types without
//! `numPages`; the `document_type` distinctions Zotero lacks (`Benchmark`
//! becomes `report` -> `Report`, `Manual` becomes `document` -> `Other`);
//! `journal` on a type with no publication-title field (it is written to
//! Extra, not read back); and `publisher` on `report`/`thesis`, whose one
//! publisher-mapped field holds `institution` (publisher goes to Extra).

use super::date::{parse_edtf, str_to_date, DateOptions};
use super::item::{AttachmentData, Creator, CreatorName, LinkMode, Tag, ZoteroItem};
use super::schema::{field_from_type_and_base, primary_creator_type};
use super::schema_generated::{CreatorType, Field, ItemType};
use super::validate::combine_extra_fields;
use crate::document::{Author, DocumentType, KovanDocument, Visibility};

/// The id prefix of a document imported from Zotero.
pub const ZOTERO_ID_PREFIX: &str = "zotero:";

/// The kovan document type of a Zotero item type.
pub fn document_type_for(item_type: ItemType) -> DocumentType {
    match item_type {
        ItemType::JournalArticle
        | ItemType::ConferencePaper
        | ItemType::Preprint
        | ItemType::MagazineArticle
        | ItemType::NewspaperArticle => DocumentType::Paper,
        ItemType::Report => DocumentType::Report,
        ItemType::Standard => DocumentType::Standard,
        ItemType::Thesis => DocumentType::Thesis,
        _ => DocumentType::Other,
    }
}

/// The Zotero item type for a kovan document type (a new item's type).
pub fn item_type_for(document_type: DocumentType) -> ItemType {
    match document_type {
        DocumentType::Paper => ItemType::JournalArticle,
        DocumentType::Report | DocumentType::Benchmark => ItemType::Report,
        DocumentType::Standard => ItemType::Standard,
        DocumentType::Thesis => ItemType::Thesis,
        DocumentType::Manual | DocumentType::Other => ItemType::Document,
    }
}

/// Whether the type's publisher-mapped field is an institution.
fn publisher_is_institution(t: ItemType) -> bool {
    matches!(t, ItemType::Report | ItemType::Thesis)
}

/// The field kovan's `number` maps to for this type.
fn number_field(t: ItemType) -> Option<Field> {
    field_from_type_and_base(t, Field::Issue).or_else(|| field_from_type_and_base(t, Field::Number))
}

/// The value of a `Key: value` line of Extra.
fn extra_line(extra: &str, key: &str) -> Option<String> {
    extra.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case(key)
            .then(|| v.trim().to_owned())
    })
}

/// Write `value` to `base`'s field for item type `t`, or queue it for Extra
/// when the type has no such field.
fn put_field(
    item: &mut ZoteroItem,
    t: ItemType,
    to_extra: &mut Vec<(String, String)>,
    base: Field,
    value: Option<&str>,
) {
    match field_from_type_and_base(t, base) {
        Some(f) => {
            if item.field(f) != value {
                item.set_field(f, value.unwrap_or(""));
            }
        }
        None => {
            if let Some(v) = value.filter(|v| !v.is_empty()) {
                to_extra.push((base.as_str().to_owned(), v.to_owned()));
            }
        }
    }
}

fn year_of(date: &str) -> Option<u32> {
    if let Some(e) = parse_edtf(date) {
        return u32::try_from(e.begin.year).ok();
    }
    str_to_date(date, &DateOptions::default())
        .year
        .and_then(|y| y.parse().ok())
}

fn slug_from(item: &ZoteroItem, authors: &[Author], year: Option<u32>) -> String {
    if let Some(k) = item.field(Field::CitationKey) {
        return k.to_owned();
    }
    let mut s: String = authors
        .first()
        .map(|a| a.family.as_str())
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if let Some(y) = year {
        s.push_str(&y.to_string());
    }
    s
}

impl ZoteroItem {
    /// This item as a [`KovanDocument`] (see the module docs for the field
    /// map and what is lost). The item itself is kept in
    /// `KovanDocument::zotero_item`.
    pub fn to_kovan_document(&self) -> KovanDocument {
        let t = self.item_type;
        let via = |f: Field| {
            self.field_via_base(f)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let primary = primary_creator_type(t);
        let authors: Vec<Author> = self
            .creators
            .iter()
            .filter(|c| Some(c.creator_type) == primary)
            .map(|c| match &c.name {
                CreatorName::TwoField {
                    first_name,
                    last_name,
                } => Author {
                    family: last_name.clone(),
                    given: first_name.clone(),
                    affiliation: None,
                },
                CreatorName::SingleField { name } => Author {
                    family: name.clone(),
                    given: String::new(),
                    affiliation: None,
                },
            })
            .collect();
        let year = via(Field::Date).and_then(|d| year_of(&d));
        let mut doc = KovanDocument::new(
            format!("{ZOTERO_ID_PREFIX}{}", self.key.clone().unwrap_or_default()),
            slug_from(self, &authors, year),
            Visibility::Proprietary,
            document_type_for(t),
            via(Field::Title).unwrap_or_default(),
        );
        doc.authors = authors;
        doc.abstract_text = via(Field::AbstractNote).unwrap_or_default();
        doc.year = year;
        doc.doi =
            via(Field::Doi).or_else(|| self.field(Field::Extra).and_then(|e| extra_line(e, "DOI")));
        doc.journal = via(Field::PublicationTitle);
        if publisher_is_institution(t) {
            doc.institution = via(Field::Publisher);
        } else {
            doc.publisher = via(Field::Publisher);
        }
        doc.volume = via(Field::Volume);
        doc.pages = via(Field::Pages);
        doc.number = number_field(t)
            .and_then(|f| self.field(f))
            .map(str::to_owned);
        doc.keywords = self
            .tags
            .iter()
            .filter(|g| g.is_automatic())
            .map(|g| g.tag.clone())
            .collect();
        doc.tags = self
            .tags
            .iter()
            .filter(|g| !g.is_automatic())
            .map(|g| g.tag.clone())
            .collect();
        doc.source_url = via(Field::Url);
        doc.source_path = self
            .attachments
            .iter()
            .filter_map(|a| a.attachment.as_ref())
            .find(|a| a.link_mode == Some(LinkMode::LinkedFile))
            .and_then(|a| a.path.clone());
        doc.page_count = via(Field::NumPages).and_then(|n| n.trim().parse().ok());
        doc.zotero_item = Some(self.clone());
        doc
    }

    /// A Zotero item for a [`KovanDocument`]: its stored `zotero_item` with
    /// kovan's values written over it, or a new item of the mapped type (see
    /// the module docs).
    pub fn from_kovan_document(doc: &KovanDocument) -> ZoteroItem {
        let mut item = match &doc.zotero_item {
            Some(z) => z.clone(),
            None => ZoteroItem::new(item_type_for(doc.document_type)),
        };
        let t = item.item_type;
        if let Some(key) = doc
            .id
            .strip_prefix(ZOTERO_ID_PREFIX)
            .filter(|k| !k.is_empty())
        {
            item.key = Some(key.to_owned());
        }
        let mut to_extra: Vec<(String, String)> = Vec::new();
        let put = |item: &mut ZoteroItem,
                   x: &mut Vec<(String, String)>,
                   base: Field,
                   value: Option<&str>| { put_field(item, t, x, base, value) };
        put(
            &mut item,
            &mut to_extra,
            Field::Title,
            Some(&doc.title)
                .filter(|s| !s.is_empty())
                .map(String::as_str),
        );
        put(
            &mut item,
            &mut to_extra,
            Field::AbstractNote,
            Some(&doc.abstract_text)
                .filter(|s| !s.is_empty())
                .map(String::as_str),
        );
        put(
            &mut item,
            &mut to_extra,
            Field::CitationKey,
            Some(&doc.slug)
                .filter(|s| !s.is_empty())
                .map(String::as_str),
        );
        // The DOI may already live in Extra; leave it there if it matches.
        let doi_in_extra = item.field(Field::Extra).and_then(|e| extra_line(e, "DOI"));
        if field_from_type_and_base(t, Field::Doi).is_some()
            || doi_in_extra.as_deref() != doc.doi.as_deref()
        {
            put(&mut item, &mut to_extra, Field::Doi, doc.doi.as_deref());
        }
        put(
            &mut item,
            &mut to_extra,
            Field::PublicationTitle,
            doc.journal.as_deref(),
        );
        if publisher_is_institution(t) {
            put(
                &mut item,
                &mut to_extra,
                Field::Publisher,
                doc.institution.as_deref(),
            );
            if let Some(p) = doc.publisher.as_deref().filter(|p| !p.is_empty()) {
                to_extra.push(("publisher".into(), p.to_owned()));
            }
        } else {
            put(
                &mut item,
                &mut to_extra,
                Field::Publisher,
                doc.publisher.as_deref(),
            );
            if let Some(i) = doc.institution.as_deref().filter(|p| !p.is_empty()) {
                to_extra.push(("institution".into(), i.to_owned()));
            }
        }
        put(
            &mut item,
            &mut to_extra,
            Field::Volume,
            doc.volume.as_deref(),
        );
        put(&mut item, &mut to_extra, Field::Pages, doc.pages.as_deref());
        match number_field(t) {
            Some(f) => {
                if item.field(f) != doc.number.as_deref() {
                    item.set_field(f, doc.number.clone().unwrap_or_default());
                }
            }
            None => {
                if let Some(n) = doc.number.as_deref().filter(|n| !n.is_empty()) {
                    to_extra.push(("number".into(), n.to_owned()));
                }
            }
        }
        put(
            &mut item,
            &mut to_extra,
            Field::Url,
            doc.source_url.as_deref(),
        );
        if let Some(pc) = doc.page_count {
            let cur = item
                .field_via_base(Field::NumPages)
                .and_then(|n| n.trim().parse::<u32>().ok());
            if cur != Some(pc) {
                put(
                    &mut item,
                    &mut to_extra,
                    Field::NumPages,
                    Some(&pc.to_string()),
                );
            }
        }
        // Keep a stored date whose year matches; otherwise write the year.
        let cur_year = item.field_via_base(Field::Date).and_then(year_of);
        if cur_year != doc.year {
            put(
                &mut item,
                &mut to_extra,
                Field::Date,
                doc.year.map(|y| y.to_string()).as_deref(),
            );
        }

        // Creators: primary-type creators come from `authors`; keep the rest.
        let primary = primary_creator_type(t).unwrap_or(CreatorType::Author);
        if t.is_regular() {
            let from_doc: Vec<Creator> = doc
                .authors
                .iter()
                .map(|a| {
                    if a.given.is_empty() {
                        Creator::single(primary, a.family.clone())
                    } else {
                        Creator::person(primary, a.given.clone(), a.family.clone())
                    }
                })
                .collect();
            let current: Vec<Creator> = item
                .creators
                .iter()
                .filter(|c| c.creator_type == primary)
                .cloned()
                .collect();
            let same = current.len() == from_doc.len()
                && current.iter().zip(&from_doc).all(|(c, d)| {
                    let as_author = |c: &Creator| match &c.name {
                        CreatorName::TwoField {
                            first_name,
                            last_name,
                        } => (last_name.clone(), first_name.clone()),
                        CreatorName::SingleField { name } => (name.clone(), String::new()),
                    };
                    as_author(c) == as_author(d)
                });
            if !same {
                let others: Vec<Creator> = item
                    .creators
                    .iter()
                    .filter(|c| c.creator_type != primary)
                    .cloned()
                    .collect();
                item.creators = from_doc.into_iter().chain(others).collect();
            }
        }

        // Tags: manual from `tags`, automatic (type 1) from `keywords`.
        let wanted: Vec<Tag> = doc
            .tags
            .iter()
            .map(|g| Tag {
                tag: g.clone(),
                tag_type: None,
            })
            .chain(doc.keywords.iter().map(|g| Tag {
                tag: g.clone(),
                tag_type: Some(1),
            }))
            .collect();
        let key = |g: &Tag| (g.tag.clone(), g.is_automatic());
        let mut have: Vec<(String, bool)> = item.tags.iter().map(key).collect();
        let mut want: Vec<(String, bool)> = wanted.iter().map(key).collect();
        have.sort();
        want.sort();
        if have != want {
            item.tags = wanted;
        }

        if let Some(path) = doc.source_path.as_deref() {
            let present = item
                .attachments
                .iter()
                .any(|a| a.attachment.as_ref().and_then(|d| d.path.as_deref()) == Some(path));
            if !present {
                let mut a = ZoteroItem::new(ItemType::Attachment);
                let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
                a.set_field(Field::Title, name);
                a.attachment = Some(AttachmentData {
                    link_mode: Some(LinkMode::LinkedFile),
                    content_type: path
                        .to_ascii_lowercase()
                        .ends_with(".pdf")
                        .then(|| "application/pdf".to_owned()),
                    path: Some(path.to_owned()),
                    ..AttachmentData::default()
                });
                item.attachments.push(a);
            }
        }

        if !to_extra.is_empty() {
            let extra = item.field(Field::Extra).unwrap_or("").to_owned();
            item.set_field(Field::Extra, combine_extra_fields(&extra, to_extra));
        }
        item
    }
}

impl KovanDocument {
    /// Shorthand for [`ZoteroItem::to_kovan_document`].
    pub fn from_zotero_item(item: &ZoteroItem) -> KovanDocument {
        item.to_kovan_document()
    }

    /// Shorthand for [`ZoteroItem::from_kovan_document`].
    pub fn to_zotero_item(&self) -> ZoteroItem {
        ZoteroItem::from_kovan_document(self)
    }
}
