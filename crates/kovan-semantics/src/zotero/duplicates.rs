// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281),
// `chrome/content/zotero/xpcom/duplicates.js` (`Zotero.Duplicates`,
// `_findDuplicates`, `Zotero.DisjointSetForest`).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University; Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Duplicate detection over a [`ZoteroLibrary`] in memory: a port of
//! `Zotero.Duplicates.prototype._findDuplicates` (duplicates.js:104-416).
//!
//! Upstream builds the candidate rows with SQL and unions matching rows in
//! a disjoint-set forest. This port builds the same rows from the items:
//!
//! | Pass | Upstream SQL (duplicates.js) | Here |
//! |---|---|---|
//! | ISBN | `itemTypeID = book AND fieldID = ISBN`, not in `deletedItems` (213) | `book` items, field `ISBN`, `deleted != true`; `cleanISBN` (validated), then `toISBN13`; exact-match runs over the values sorted |
//! | DOI | `fieldID = DOI AND value LIKE '10.%'` (243) | field `DOI` starting `10.` (SQLite `LIKE` is case-insensitive but `10.` has no letters); value trimmed and upper-cased; exact-match runs |
//! | year | `SUBSTR(value, 1, 4)` of `date` and every field mapped to it, `!= '0000'` (274) | the same four characters of the stored **multipart** form, made with `kovan_common::zotero::date::str_to_multipart` (Zotero stores dates as `YYYY-MM-DD original`) |
//! | title | `title` and every field mapped to it, item type not attachment/note (291) | the same fields and filter; `normalizeString`; sorted |
//! | creators | `lastName`, `firstName`, `fieldMode`, by `orderIndex` (313) | the item's creators in order; a single-field creator is `lastName` = name, `fieldMode` 1 |
//!
//! The title pass compares every pair in a run of equal normalised titles
//! and rejects a pair when both have DOIs that differ, both have ISBNs that
//! differ, both have years more than one apart, or when they share no
//! creator by normalised last name + first initial (no creators on either
//! side is a match; creators on only one side is not).
//!
//! **Equivalence.** The partition into sets is independent of row order
//! (every pair in an equal-value run is unioned), so it equals upstream's
//! whatever order SQLite returns rows in. Values sort in UTF-16 order as
//! JavaScript's `<` does. Sets are reported sorted (items by library index,
//! sets by their first item), which upstream does not promise.
//!
//! **Not equivalent.** Upstream reads one library of the database; this
//! reads one [`ZoteroLibrary`]'s top-level `items` list (an export's nested
//! `attachments`/`notes` are not candidates, as children never are for
//! titles). A two-digit year in a date is resolved against
//! [`DuplicateOptions::date`]'s `current_year`, as upstream resolves it
//! against the year the item was saved.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use kovan_common::zotero::date::{str_to_multipart, DateOptions};
use kovan_common::zotero::schema::type_fields_from_base;
use kovan_common::zotero::{CreatorName, Field, ItemType, ZoteroItem, ZoteroLibrary};

use super::text::{clean_isbn, cmp_utf16, js_trim, normalize_string, to_isbn13};

/// Options for [`find_duplicates`].
#[derive(Debug, Clone, Default)]
pub struct DuplicateOptions {
    /// How a date field is turned into its stored multipart form (only the
    /// current year matters, for two-digit years).
    pub date: DateOptions,
}

/// The duplicate sets of a library: what `Zotero.Duplicates` holds after
/// `getSearchObject()`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DuplicateSets {
    /// Each set's items as indices into `ZoteroLibrary::items`, ascending;
    /// sets ordered by their first index. Every set has two or more items.
    pub sets: Vec<Vec<usize>>,
}

impl DuplicateSets {
    /// Every item in some set (`DisjointSetForest.findAll(true)`), ascending.
    pub fn all_items(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.sets.iter().flatten().copied().collect();
        v.sort_unstable();
        v
    }

    /// The set containing item `index` (`getSetItemsByItemID`): just
    /// `[index]` when it is in none, as upstream's `find` makes a new set.
    pub fn set_of(&self, index: usize) -> Vec<usize> {
        self.sets
            .iter()
            .find(|s| s.contains(&index))
            .cloned()
            .unwrap_or_else(|| vec![index])
    }

    /// The sets as item keys (an item without a key is left out).
    pub fn keys(&self, library: &ZoteroLibrary) -> Vec<Vec<String>> {
        self.sets
            .iter()
            .map(|s| {
                s.iter()
                    .filter_map(|&i| library.items.get(i).and_then(|it| it.key.clone()))
                    .collect()
            })
            .collect()
    }
}

/// `Zotero.DisjointSetForest` (duplicates.js:439). Only set membership is
/// observable upstream, so union by rank with path compression here gives
/// the same partition.
#[derive(Debug, Clone, Default)]
struct Forest {
    parent: BTreeMap<usize, usize>,
    rank: BTreeMap<usize, u32>,
}

impl Forest {
    fn find(&mut self, x: usize) -> usize {
        let p = *self.parent.entry(x).or_insert(x);
        self.rank.entry(x).or_insert(0);
        if p == x {
            return x;
        }
        let r = self.find(p);
        self.parent.insert(x, r);
        r
    }

    fn union(&mut self, x: usize, y: usize) {
        let (xr, yr) = (self.find(x), self.find(y));
        if xr == yr {
            return;
        }
        let (rx, ry) = (self.rank[&xr], self.rank[&yr]);
        match rx.cmp(&ry) {
            Ordering::Less => {
                self.parent.insert(xr, yr);
            }
            Ordering::Greater => {
                self.parent.insert(yr, xr);
            }
            Ordering::Equal => {
                self.parent.insert(yr, xr);
                self.rank.insert(xr, rx + 1);
            }
        }
    }

    fn sets(mut self) -> Vec<Vec<usize>> {
        let ids: Vec<usize> = self.parent.keys().copied().collect();
        let mut by_root: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for id in ids {
            let r = self.find(id);
            by_root.entry(r).or_default().push(id);
        }
        let mut sets: Vec<Vec<usize>> = by_root.into_values().collect();
        for s in &mut sets {
            s.sort_unstable();
        }
        sets.sort();
        sets
    }
}

#[derive(Debug, Clone)]
struct Row {
    item: usize,
    value: String,
}

/// `sortByValue` (duplicates.js:126) for non-null strings: JS `<`.
fn sort_rows(rows: &mut [Row]) {
    rows.sort_by(|a, b| cmp_utf16(&a.value, &b.value));
}

/// `processRows` without a comparison function (exact match, no
/// reprocessing; duplicates.js:154-199).
fn process_exact(rows: &[Row], forest: &mut Forest) {
    let len = rows.len();
    let mut i = 0;
    while i < len {
        let mut j = i + 1;
        let mut last_match: Option<usize> = None;
        while j < len {
            if rows[i].value.is_empty()
                || rows[j].value.is_empty()
                || rows[i].value != rows[j].value
            {
                break;
            }
            forest.union(rows[i].item, rows[j].item);
            last_match = Some(j);
            j += 1;
        }
        if let Some(m) = last_match {
            i = m;
        }
        i += 1;
    }
}

/// A creator row as the title comparison sees it (duplicates.js:331-334).
#[derive(Debug, Clone, PartialEq, Eq)]
struct CreatorKey {
    last_name: String,
    /// `normalizeString(firstName).charAt(0)` as a UTF-16 code unit; `None`
    /// for "" and for single-field creators (upstream's `false || ""`).
    first_initial: Option<u16>,
}

fn is_candidate(item: &ZoteroItem) -> bool {
    item.deleted != Some(true)
}

fn not_attachment_or_note(item: &ZoteroItem) -> bool {
    !matches!(item.item_type, ItemType::Attachment | ItemType::Note)
}

/// Find the duplicate sets of `library` (`Zotero.Duplicates._findDuplicates`).
pub fn find_duplicates(library: &ZoteroLibrary, options: &DuplicateOptions) -> DuplicateSets {
    let mut forest = Forest::default();
    let items = &library.items;

    // ISBN: books only (duplicates.js:213-238).
    let mut isbn_cache: BTreeMap<usize, String> = BTreeMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if it.item_type != ItemType::Book || !is_candidate(it) {
            continue;
        }
        let Some(v) = it.field(Field::Isbn) else {
            continue;
        };
        let Some(clean) = clean_isbn(v, false) else {
            continue;
        };
        let Some(v13) = to_isbn13(&clean) else {
            continue;
        };
        isbn_cache.insert(i, v13.clone());
        rows.push(Row {
            item: i,
            value: v13,
        });
    }
    sort_rows(&mut rows);
    process_exact(&rows, &mut forest);

    // DOI (duplicates.js:240-266).
    let mut doi_cache: BTreeMap<usize, String> = BTreeMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if !is_candidate(it) {
            continue;
        }
        let Some(v) = it.field(Field::Doi) else {
            continue;
        };
        if !v.starts_with("10.") {
            continue;
        }
        let nv = js_trim(v).to_uppercase();
        doi_cache.insert(i, nv.clone());
        rows.push(Row { item: i, value: nv });
    }
    sort_rows(&mut rows);
    process_exact(&rows, &mut forest);

    // Years (duplicates.js:268-284): `date` and every field mapped to it.
    let mut date_fields: Vec<&'static str> = vec![Field::Date.as_str()];
    date_fields.extend(
        type_fields_from_base(Field::Date)
            .iter()
            .map(|f| f.as_str()),
    );
    let mut year_cache: BTreeMap<usize, i64> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if !is_candidate(it) {
            continue;
        }
        // ORDER BY value: the last row of an item wins.
        let mut values: Vec<String> = it
            .fields
            .iter()
            .filter(|(k, v)| date_fields.contains(&k.as_str()) && !v.is_empty())
            .map(|(_, v)| str_to_multipart(v, &options.date))
            .collect();
        values.sort_by(|a, b| cmp_utf16(a, b));
        for m in values {
            let year: String = m.chars().take(4).collect();
            if year == "0000" {
                continue;
            }
            if let Ok(y) = year.parse::<i64>() {
                year_cache.insert(i, y);
            }
        }
    }

    // Titles (duplicates.js:286-411).
    let mut title_fields: Vec<&'static str> = type_fields_from_base(Field::Title)
        .iter()
        .map(|f| f.as_str())
        .collect();
    title_fields.push(Field::Title.as_str());
    let mut rows: Vec<Row> = Vec::new();
    let mut creators: BTreeMap<usize, Vec<CreatorKey>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if !is_candidate(it) || !not_attachment_or_note(it) {
            continue;
        }
        for (k, v) in &it.fields {
            if title_fields.contains(&k.as_str()) {
                rows.push(Row {
                    item: i,
                    value: normalize_string(v),
                });
            }
        }
        let cs: Vec<CreatorKey> = it
            .creators
            .iter()
            .map(|c| match &c.name {
                CreatorName::TwoField {
                    first_name,
                    last_name,
                } => CreatorKey {
                    last_name: normalize_string(last_name),
                    first_initial: normalize_string(first_name).encode_utf16().next(),
                },
                CreatorName::SingleField { name } => CreatorKey {
                    last_name: normalize_string(name),
                    first_initial: None,
                },
            })
            .collect();
        if !cs.is_empty() {
            creators.insert(i, cs);
        }
    }
    sort_rows(&mut rows);
    let compare = |a: &Row, b: &Row| -> i8 {
        if a.value.is_empty() || b.value.is_empty() || a.value != b.value {
            return -1;
        }
        let differ = |c: &BTreeMap<usize, String>| match (c.get(&a.item), c.get(&b.item)) {
            (Some(x), Some(y)) => x != y,
            _ => false,
        };
        if differ(&doi_cache) || differ(&isbn_cache) {
            return 0;
        }
        if let (Some(x), Some(y)) = (year_cache.get(&a.item), year_cache.get(&b.item)) {
            if (x - y).abs() > 1 {
                return 0;
            }
        }
        match (creators.get(&a.item), creators.get(&b.item)) {
            (None, None) => 1,
            (Some(ac), Some(bc)) if ac.iter().any(|x| bc.iter().any(|y| x == y)) => 1,
            _ => 0,
        }
    };
    // processRows(rows, compare, reprocessMatches = true).
    for i in 0..rows.len() {
        let mut j = i + 1;
        while j < rows.len() {
            match compare(&rows[i], &rows[j]) {
                -1 => break,
                0 => {
                    j += 1;
                    continue;
                }
                _ => {}
            }
            forest.union(rows[i].item, rows[j].item);
            j += 1;
        }
    }

    DuplicateSets {
        sets: forest.sets(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::zotero::{Creator, CreatorType};

    fn item(t: ItemType, title: &str) -> ZoteroItem {
        let mut i = ZoteroItem::new(t);
        i.set_field(Field::Title, title);
        i
    }

    fn sets(items: Vec<ZoteroItem>) -> Vec<Vec<usize>> {
        let lib = ZoteroLibrary {
            collections: vec![],
            items,
        };
        find_duplicates(&lib, &DuplicateOptions::default()).sets
    }

    #[test]
    fn same_title_no_creators_match() {
        let s = sets(vec![
            item(ItemType::JournalArticle, "A Title"),
            item(ItemType::JournalArticle, "a title!"),
            item(ItemType::JournalArticle, "Other"),
        ]);
        assert_eq!(s, vec![vec![0, 1]]);
    }

    #[test]
    fn creators_on_one_side_only_do_not_match() {
        let mut a = item(ItemType::Book, "T");
        a.creators
            .push(Creator::person(CreatorType::Author, "Jane", "Doe"));
        let b = item(ItemType::Book, "T");
        assert!(sets(vec![a, b]).is_empty());
    }

    #[test]
    fn creator_initial_and_last_name_must_match() {
        let mut a = item(ItemType::Book, "T");
        a.creators
            .push(Creator::person(CreatorType::Author, "Jane", "Doé"));
        let mut b = item(ItemType::Book, "T");
        b.creators
            .push(Creator::person(CreatorType::Editor, "J.", "Doe"));
        let mut c = item(ItemType::Book, "T");
        c.creators
            .push(Creator::person(CreatorType::Author, "Kim", "Doe"));
        assert_eq!(sets(vec![a, b, c]), vec![vec![0, 1]]);
    }

    #[test]
    fn years_more_than_one_apart_and_different_dois_block() {
        let mut a = item(ItemType::JournalArticle, "T");
        a.set_field(Field::Date, "2001");
        let mut b = item(ItemType::JournalArticle, "T");
        b.set_field(Field::Date, "2002-05-01");
        let mut c = item(ItemType::JournalArticle, "T");
        c.set_field(Field::Date, "2004");
        let mut d = item(ItemType::JournalArticle, "T");
        d.set_field(Field::Doi, "10.1/x");
        let mut e = item(ItemType::JournalArticle, "T");
        e.set_field(Field::Doi, "10.1/y");
        // a~b (1 year), c is 2+ from a and b but matches d and e (no year);
        // d and e differ on DOI but are joined through c (union-find).
        assert_eq!(sets(vec![a, b, c, d, e]), vec![vec![0, 1, 2, 3, 4]]);
    }

    #[test]
    fn doi_matches_case_insensitively_and_trashed_items_are_ignored() {
        let mut a = item(ItemType::JournalArticle, "X");
        a.set_field(Field::Doi, "10.1000/ABC");
        let mut b = item(ItemType::Preprint, "Y");
        b.set_field(Field::Doi, " 10.1000/abc ".trim_start());
        let mut c = item(ItemType::JournalArticle, "Z");
        c.set_field(Field::Doi, "10.1000/abc");
        c.deleted = Some(true);
        assert_eq!(sets(vec![a, b, c]), vec![vec![0, 1]]);
    }

    #[test]
    fn notes_and_attachments_never_match_on_title() {
        let mut n1 = ZoteroItem::new(ItemType::Attachment);
        n1.set_field(Field::Title, "PDF");
        let mut n2 = ZoteroItem::new(ItemType::Attachment);
        n2.set_field(Field::Title, "PDF");
        assert!(sets(vec![n1, n2]).is_empty());
    }
}
