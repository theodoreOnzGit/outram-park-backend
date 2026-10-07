// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// the tables chrome/content/zotero/xpcom/data/search.js queries
// (resource/schema/userdata.sql: items, itemData, itemNotes,
// itemAttachments, itemAnnotations, itemTags, itemCreators, collectionItems,
// deletedItems, deletedCollections, publicationsItems), the fileTypes table
// of resource/schema/system.sql, and what data/item.js stores in them
// (setField: date fields as multipart dates, accessDate as UTC SQL).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! [`SearchLibrary`]: an in-memory stand-in for the rows of Zotero's SQLite
//! database that the search reads, built from a
//! [`kovan_common::zotero::ZoteroLibrary`].
//!
//! | Zotero table | Built from |
//! |---|---|
//! | `items` | every item of the library, including child items and the translator-format `attachments`/`notes` nested in an item (whose parent becomes that item) |
//! | `itemData` | `fields`, keyed by type-specific name; date fields (`Field::is_date`) stored as multipart dates (`strToMultipart`, item.js:852), `accessDate` as a UTC SQL date-time (item.js:855-867); names that are not Zotero fields are skipped (upstream would have moved them to Extra) |
//! | `itemCreators`/`creators` | `creators`; a single-field creator is `lastName` with an empty `firstName`, as Zotero stores `fieldMode: 1` |
//! | `itemTags`/`tags` | `tags` |
//! | `itemNotes` | note items (with their parent), and attachments with a non-empty `note` (parent NULL, item.js:2129) |
//! | `itemAttachments` | attachment items (`linkMode`, `contentType`, `lastRead`) |
//! | `itemAnnotations` | annotation items (type number, text, comment, colour) |
//! | `collectionItems` | `collections` of each item |
//! | `deletedItems` | items with `deleted: true` |
//! | `deletedCollections` | collections with `deleted: true` |
//! | `publicationsItems` | items with `inPublications: true` |
//! | `retractedItems`, `feedItems`, `groupItems` | **no data in a `ZoteroLibrary`**: empty |
//! | full-text index (`fulltextContent`) | the attachment text the caller supplies with [`SearchLibrary::set_full_text`] |
//! | saved searches | the searches the caller supplies with [`SearchLibrary::add_saved_search`] |
//!
//! An item without a key gets the synthetic key `~<n>` (its position in the
//! flattened list); real Zotero keys are eight characters of
//! `23456789ABCDEFGHIJKLMNPQRSTUVWXYZ`, so the two cannot collide.

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::zotero::date::{
    iso_to_sql, is_iso_date, is_sql_date, is_sql_date_time, str_to_multipart, DateOptions,
};
use kovan_common::zotero::{
    AnnotationType, CreatorName, Field, ItemType, LinkMode, ZoteroCollection, ZoteroItem,
    ZoteroLibrary,
};

use super::normalize::note_plain_text;
use super::search::Search;

/// A set of rows (indices into the flattened item list).
pub type RowSet = BTreeSet<usize>;

/// The clock and time zone the date conditions read (`'NOW'`,
/// `'localtime'`, `today`/`yesterday`). Upstream reads the system clock;
/// here it is an input, so a search is deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchClock {
    /// Seconds since the Unix epoch (UTC).
    pub now_unix: i64,
    /// Minutes east of UTC of "local time".
    pub utc_offset_minutes: i32,
}

impl SearchClock {
    /// A UTC clock at `now_unix`.
    pub fn utc(now_unix: i64) -> Self {
        SearchClock {
            now_unix,
            utc_offset_minutes: 0,
        }
    }
}

/// One row of `items` with the per-item data of the other tables.
#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub key: String,
    pub item: ZoteroItem,
    pub parent: Option<usize>,
    pub deleted: bool,
    /// `itemData`: (field, stored value).
    pub item_data: Vec<(Field, String)>,
    /// `itemCreators`: (creator type name, firstName, lastName).
    pub creators: Vec<(String, String, String)>,
    /// `dateAdded` / `dateModified` as UTC SQL date-times.
    pub date_added: Option<String>,
    pub date_modified: Option<String>,
    /// Normalised plain text of the note, when the row is in `itemNotes`.
    pub note_text: Option<String>,
    /// Whether the row is in `itemNotes`, and that row's `parentItemID`.
    pub in_item_notes: bool,
    pub note_parent: Option<usize>,
}

impl Row {
    pub fn is_attachment(&self) -> bool {
        self.item.item_type == ItemType::Attachment
    }
    pub fn is_annotation(&self) -> bool {
        self.item.item_type == ItemType::Annotation
    }
}

/// The `fileTypes`/`fileTypeMimeTypes` rows of resource/schema/system.sql
/// (lines 113-162): id, name, MIME prefixes.
pub const FILE_TYPES: [(u32, &str, &[&str]); 8] = [
    (1, "webpage", &["text/html"]),
    (
        2,
        "image",
        &[
            "image/",
            "application/vnd.oasis.opendocument.graphics",
            "application/vnd.oasis.opendocument.image",
        ],
    ),
    (3, "pdf", &["application/pdf"]),
    (
        4,
        "audio",
        &[
            "audio/",
            "x-pn-realaudio",
            "application/ogg",
            "application/x-killustrator",
        ],
    ),
    (5, "video", &["video/", "application/x-shockwave-flash"]),
    (
        6,
        "document",
        &[
            "text/plain",
            "application/rtf",
            "application/msword",
            "text/xml",
            "application/postscript",
            "application/wordperfect5.1",
            "application/x-latex",
            "application/x-tex",
            "application/x-kword",
            "application/x-kspread",
            "application/x-kchart",
            "application/vnd.oasis.opendocument.chart",
            "application/vnd.oasis.opendocument.database",
            "application/vnd.oasis.opendocument.formula",
            "application/vnd.oasis.opendocument.spreadsheet",
            "application/vnd.oasis.opendocument.text",
        ],
    ),
    (
        7,
        "presentation",
        &[
            "application/powerpoint",
            "application/vnd.oasis.opendocument.presentation",
            "application/x-kpresenter",
            "application/vnd.ms-powerpoint",
        ],
    ),
    (8, "ebook", &["application/epub+zip", "application/epub"]),
];

/// `Zotero.Attachments.LINK_MODE_*` (attachments.js:35-39).
pub fn link_mode_number(m: LinkMode) -> u8 {
    match m {
        LinkMode::ImportedFile => 0,
        LinkMode::ImportedUrl => 1,
        LinkMode::LinkedFile => 2,
        LinkMode::LinkedUrl => 3,
        LinkMode::EmbeddedImage => 4,
    }
}

/// `Zotero.Annotations.ANNOTATION_TYPE_*` (annotations.js:31-36), the
/// number `itemAnnotations.type` stores.
pub fn annotation_type_number(t: AnnotationType) -> u8 {
    match t {
        AnnotationType::Highlight => 1,
        AnnotationType::Note => 2,
        AnnotationType::Image => 3,
        AnnotationType::Ink => 4,
        AnnotationType::Underline => 5,
        AnnotationType::Text => 6,
    }
}

/// The library a [`Search`] runs against: the flattened rows plus the
/// collections, saved searches, full-text content and clock.
#[derive(Debug, Clone)]
pub struct SearchLibrary {
    pub(crate) rows: Vec<Row>,
    pub(crate) by_key: BTreeMap<String, usize>,
    pub(crate) collections: Vec<ZoteroCollection>,
    pub(crate) saved_searches: Vec<Search>,
    pub(crate) full_text: BTreeMap<String, String>,
    pub(crate) clock: SearchClock,
}

/// The date options of the client in en-US at the clock's year and zone.
pub(crate) fn date_options(clock: &SearchClock) -> DateOptions {
    let year = (clock.now_unix + clock.utc_offset_minutes as i64 * 60).div_euclid(86_400);
    DateOptions {
        current_year: super::dates::civil_from_days(year).0,
        utc_offset_minutes: clock.utc_offset_minutes,
        ..DateOptions::default()
    }
}

/// What `setField` stores for `field` (item.js:847-868).
fn stored_value(field: Field, value: &str, opts: &DateOptions) -> String {
    if field == Field::AccessDate {
        if is_iso_date(value) && !is_sql_date(value) {
            if let Some(sql) = iso_to_sql(value) {
                return sql;
            }
        }
        return value.to_owned();
    }
    if field.is_date() && !is_multipart(value) {
        return str_to_multipart(value, opts);
    }
    value.to_owned()
}

/// `Zotero.Date.isMultipart` (utilities date.js): `YYYY-MM-DD ` followed by
/// the original text.
pub fn is_multipart(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 11 || b[4] != b'-' || b[7] != b'-' || b[10] != b' ' {
        return false;
    }
    if !b[..4].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let mm = &s[5..7];
    let dd = &s[8..10];
    let month_ok = matches!(mm.as_bytes(), [b'0', b'0'..=b'9']) || matches!(mm, "10" | "11" | "12");
    let day_ok = matches!(
        dd.as_bytes(),
        [b'0', b'0'..=b'9'] | [b'1'..=b'2', b'0'..=b'9']
    ) || matches!(dd, "30" | "31");
    month_ok && day_ok
}

fn to_sql_datetime(iso: Option<&str>) -> Option<String> {
    let v = iso?;
    if is_sql_date_time(v) {
        return Some(v.to_owned());
    }
    iso_to_sql(v)
}

impl SearchLibrary {
    /// Build the search tables of `library` (see the module docs), with the
    /// given clock and no saved searches or full-text content.
    pub fn new(library: &ZoteroLibrary, clock: SearchClock) -> Self {
        let opts = date_options(&clock);
        // Flatten: (item, parent key from nesting).
        let mut flat: Vec<(ZoteroItem, Option<String>)> = Vec::new();
        fn push(
            flat: &mut Vec<(ZoteroItem, Option<String>)>,
            it: &ZoteroItem,
            nest: Option<String>,
        ) {
            let mut own = it.clone();
            let children: Vec<ZoteroItem> = own
                .attachments
                .drain(..)
                .chain(own.notes.drain(..))
                .collect();
            let idx = flat.len();
            flat.push((own, nest));
            let key = flat[idx].0.key.clone().unwrap_or_else(|| format!("~{idx}"));
            if flat[idx].0.key.is_none() {
                flat[idx].0.key = Some(key.clone());
            }
            for c in &children {
                push(flat, c, Some(key.clone()));
            }
        }
        for it in &library.items {
            push(&mut flat, it, None);
        }
        let mut by_key = BTreeMap::new();
        for (i, (it, _)) in flat.iter().enumerate() {
            by_key
                .entry(it.key.clone().expect("key assigned"))
                .or_insert(i);
        }
        let mut rows = Vec::with_capacity(flat.len());
        for (it, nest) in flat {
            let parent_key = it.parent_item.clone().or(nest);
            let parent = parent_key.and_then(|k| by_key.get(&k).copied());
            let mut item_data = Vec::new();
            for (name, value) in &it.fields {
                if value.is_empty() {
                    continue;
                }
                if let Some(f) = Field::from_name(name) {
                    item_data.push((f, stored_value(f, value, &opts)));
                }
            }
            let creators = it
                .creators
                .iter()
                .map(|c| {
                    let (first, last) = match &c.name {
                        CreatorName::TwoField {
                            first_name,
                            last_name,
                        } => (first_name.clone(), last_name.clone()),
                        CreatorName::SingleField { name } => (String::new(), name.clone()),
                    };
                    (c.creator_type.as_str().to_owned(), first, last)
                })
                .collect();
            let is_note = it.item_type == ItemType::Note;
            let att_note = it.item_type == ItemType::Attachment
                && it.note.as_deref().is_some_and(|n| !n.is_empty());
            let in_item_notes = is_note || att_note;
            let note_text =
                in_item_notes.then(|| note_plain_text(it.note.as_deref().unwrap_or("")));
            rows.push(Row {
                key: it.key.clone().expect("key assigned"),
                parent,
                deleted: it.deleted == Some(true),
                item_data,
                creators,
                date_added: to_sql_datetime(it.date_added.as_deref()),
                date_modified: to_sql_datetime(it.date_modified.as_deref()),
                note_text,
                in_item_notes,
                note_parent: if is_note { parent } else { None },
                item: it,
            });
        }
        SearchLibrary {
            rows,
            by_key,
            collections: library.collections.clone(),
            saved_searches: Vec::new(),
            full_text: BTreeMap::new(),
            clock,
        }
    }

    /// Add a saved search (looked up by its `key` from `savedSearch`
    /// conditions). A search without a key cannot be referenced.
    pub fn add_saved_search(&mut self, search: Search) {
        self.saved_searches.push(search);
    }

    /// Set the extracted full text of an attachment (the full-text index's
    /// content for that item). Only attachment items are searched, as
    /// upstream indexes and scans attachments only (fulltext.js:2693).
    pub fn set_full_text(&mut self, item_key: impl Into<String>, text: impl Into<String>) {
        self.full_text.insert(item_key.into(), text.into());
    }

    /// The clock date conditions read.
    pub fn clock(&self) -> SearchClock {
        self.clock
    }

    /// The number of rows (items, including child items).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the library has no items.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The key of row `i`.
    pub fn key_of(&self, i: usize) -> &str {
        &self.rows[i].key
    }

    /// The row of an item key.
    pub fn row_of(&self, key: &str) -> Option<usize> {
        self.by_key.get(key).copied()
    }

    pub(crate) fn all(&self) -> RowSet {
        (0..self.rows.len()).collect()
    }

    /// `deletedItems`.
    pub(crate) fn deleted_direct(&self) -> RowSet {
        (0..self.rows.len())
            .filter(|&i| self.rows[i].deleted)
            .collect()
    }

    /// `Zotero.Search._deletedItemsSQL` (search.js:2215): trashed items, the
    /// child notes and attachments of trashed items, annotations of trashed
    /// attachments, and annotations of attachments of trashed items.
    pub(crate) fn deleted_items(&self) -> RowSet {
        let d = self.deleted_direct();
        let mut out = d.clone();
        for (i, r) in self.rows.iter().enumerate() {
            if r.in_item_notes && r.note_parent.is_some_and(|p| d.contains(&p)) {
                out.insert(i);
            }
            if r.is_attachment() && r.parent.is_some_and(|p| d.contains(&p)) {
                out.insert(i);
            }
            if r.is_annotation() {
                if let Some(p) = r.parent {
                    if d.contains(&p) {
                        out.insert(i);
                    }
                    let pr = &self.rows[p];
                    if pr.is_attachment() && pr.parent.is_some_and(|g| d.contains(&g)) {
                        out.insert(i);
                    }
                }
            }
        }
        out
    }

    /// Rows of `itemAttachments`.
    pub(crate) fn attachments(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.rows.len()).filter(|&i| self.rows[i].is_attachment())
    }

    /// Rows of `itemNotes`.
    pub(crate) fn item_notes(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.rows.len()).filter(|&i| self.rows[i].in_item_notes)
    }

    /// Rows of `itemAnnotations`.
    pub(crate) fn annotations(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.rows.len()).filter(|&i| self.rows[i].is_annotation())
    }

    /// The collection with this key.
    pub(crate) fn collection(&self, key: &str) -> Option<&ZoteroCollection> {
        self.collections
            .iter()
            .find(|c| c.key.as_deref() == Some(key))
    }

    /// `getDescendents(false, 'collection')` (collection.js:852): the keys of
    /// non-trashed subcollections, recursively.
    pub(crate) fn descendant_collections(&self, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![key.to_owned()];
        while let Some(k) = stack.pop() {
            for c in &self.collections {
                if c.parent_collection.as_deref() == Some(k.as_str()) && c.deleted != Some(true) {
                    if let Some(ck) = &c.key {
                        if !out.contains(ck) && ck != key {
                            out.push(ck.clone());
                            stack.push(ck.clone());
                        }
                    }
                }
            }
        }
        out
    }

    /// The saved search with this key.
    pub(crate) fn saved_search(&self, key: &str) -> Option<&Search> {
        self.saved_searches
            .iter()
            .find(|s| s.key.as_deref() == Some(key))
    }
}
