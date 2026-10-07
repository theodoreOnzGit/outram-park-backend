// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281),
// `chrome/content/zotero/mergeItems.mjs` (`mergeItems`, which
// `Zotero.Items.merge` in `xpcom/data/items.js:1037` now forwards to),
// `xpcom/data/items.js` (`moveChildItems`, 1018), `xpcom/data/notes.js`
// (`replaceAllItemKeys`, 301), `xpcom/data/item.js` (`addTag` 4805,
// `multiDiff` 5291), `xpcom/data/dataObjectUtilities.js` (`diff`, 305),
// `elements/duplicatesMergePane.js` (`setItems`, `merge`).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Merging duplicate items, as a pure function over a [`ZoteroLibrary`]:
//! [`merge_items`] is `mergeItems(item, otherItems)` (mergeItems.mjs:3).
//!
//! ## Which values win
//!
//! - **Fields: the master's, all of them.** `mergeItems` copies no field
//!   from the other items. The duplicates pane (duplicatesMergePane.js)
//!   picks the master (the oldest by Date Added, [`merge_pane_order`]),
//!   offers the others' differing values as alternatives
//!   ([`field_alternatives`], `Item#multiDiff`), writes the ones the user
//!   picks onto the master, and only then calls `mergeItems`. A caller here
//!   does the same: edit the master's fields, then merge.
//! - **Date Added: the earliest** of all the items.
//! - **Collections: the union** (the master's first, then each other's new
//!   ones).
//! - **Tags: the union**, a tag already on the master keeping the master's
//!   type when that is manual; an automatic master tag becomes manual when
//!   another item has it as manual (`addTag` updates the type in place).
//! - **Relations: moved to the master** (`moveRelations`): every relation
//!   of another item is added to the master (except one pointing at the
//!   master), the other item's `dc:replaces` relations are removed, every
//!   item in the library pointing at the other item is re-pointed at the
//!   master (except `dc:replaces` relations and the master itself), and the
//!   master gains `dc:replaces` -> the other item's URI.
//! - **Notes: moved** under the master; links in their HTML to the other
//!   item, and to merged attachments, are rewritten to the master and the
//!   attachments kept (`Zotero.Notes.replaceItemKey`/`replaceAllItemKeys`).
//! - **Attachments:** PDFs that match a master PDF by file hash, or by the
//!   50 most common words of their text, are merged (one-to-one; the copy
//!   with embedded annotations is kept); web attachments (snapshots, linked
//!   URLs) with the same title and URL (snapshots: the same title) are
//!   merged; everything else is moved under the master. A trashed
//!   attachment is moved, never merged. Merging moves the attachment's
//!   annotations (except external ones), its embedded note and its
//!   relations to the kept attachment, and trashes the other.
//! - **The other items are trashed** (`deleted = true`).
//!
//! ## What a pure function cannot know, and the evidence that replaces it
//!
//! Upstream reads attachment files: their MD5 (`attachmentHash`), whether
//! they exist, their full text (`attachmentText`) and whether a PDF has
//! embedded annotations (`PDFWorker.hasAnnotations`). Here those come from
//! [`AttachmentEvidence`], one per attachment key in
//! [`MergeOptions::evidence`]. An attachment without an entry uses
//! [`AttachmentEvidence::from_item`]: the `md5` its JSON carries (the
//! stored file's hash, written by sync), the file taken to exist exactly
//! when that hash is known, no text, and embedded annotations **unknown,
//! treated as present** (upstream's `logAndBeSafe` answer when it cannot
//! tell), which keeps both copies rather than discarding one.
//!
//! ## Not equivalent
//!
//! - Children are taken in library order; upstream sorts attachments and
//!   notes by title (locale collation) unless the user prefers
//!   chronological order. This only decides which of two equally good
//!   matches is merged ("Doesn't matter which got merged", upstream test).
//! - The text hash is the sorted word list itself, not its MD5 (equality is
//!   all that is used), and ties in word frequency at the 50th word are
//!   broken by diacritic-folded UTF-16 order, not ICU collation. The 500 MB
//!   size limit is not applied (no sizes here).
//! - A note created when both attachments have an embedded note gets a
//!   deterministic key (from the two attachment keys) instead of a random
//!   one.
//! - `dateModified` is not touched (upstream's `save()` stamps it), and
//!   items in other libraries are never subjects here (one library).
//! - `toAttachment.getRelationsByPredicate(...)` is always truthy (an
//!   array), so upstream's "add `dc:replaces` if absent" in `doMerge` never
//!   runs; it is not ported (moveRelations adds the relation anyway).
//!
//! ## An upstream behaviour ported as is, and flagged
//!
//! `replaceAllItemKeys` (notes.js:301) builds `%2Fitems%2F(<keys>)` from
//! the remap; with **no** merged attachments the alternation is empty,
//! `()` matches before every key, and each `%2Fitems%2F<KEY>` link in a
//! moved note becomes `%2Fitems%2FundefinedKEY`. `mergeItems` calls it
//! for every moved note, so on reading this corrupts note links whenever a
//! merge merges no PDF. It is reproduced here (the port is the spec) and
//! pinned by a test; it has not been observed in a running Zotero (#752).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use kovan_common::zotero::{Field, ItemType, LinkMode, Tag, ZoteroItem, ZoteroLibrary};
use serde_json::Value;

use super::relations::{
    add_relation, relations_by_predicate, remove_relation, subjects_by_object, LibraryUri,
    ObjectType, REPLACED_ITEM_PREDICATE,
};
use super::text::{cmp_utf16, is_js_whitespace, remove_diacritics, utf16_len};

/// What upstream would read from an attachment's file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttachmentEvidence {
    /// Whether the file exists (`fileExists()`; a linked URL needs none).
    pub file_exists: bool,
    /// The file's MD5 (`attachmentHash`).
    pub md5: Option<String>,
    /// The file's full text (`attachmentText`).
    pub text: Option<String>,
    /// Whether a PDF has embedded (unimported) annotations; `None` is
    /// unknown, treated as `true` as upstream treats an error.
    pub has_embedded_annotations: Option<bool>,
}

impl AttachmentEvidence {
    /// What the item's own JSON says: its `md5`, the file existing iff the
    /// hash is known, no text, embedded annotations unknown.
    pub fn from_item(item: &ZoteroItem) -> Self {
        let md5 = item.attachment.as_ref().and_then(|a| a.md5.clone());
        AttachmentEvidence {
            file_exists: md5.is_some(),
            md5,
            text: None,
            has_embedded_annotations: None,
        }
    }
}

/// Inputs to [`merge_items`] besides the items.
#[derive(Debug, Clone)]
pub struct MergeOptions {
    /// The library the items are in (for item URIs).
    pub library: LibraryUri,
    /// Per attachment key, what its file would tell upstream.
    pub evidence: BTreeMap<String, AttachmentEvidence>,
}

impl MergeOptions {
    /// Options with no file evidence.
    pub fn new(library: LibraryUri) -> Self {
        MergeOptions {
            library,
            evidence: BTreeMap::new(),
        }
    }
}

/// The result of [`merge_items`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOutcome {
    /// The library after the merge.
    pub library: ZoteroLibrary,
    /// Keys of every pre-existing item the merge changed, sorted.
    pub changed: Vec<String>,
    /// Keys of items the merge created (notes split off attachments).
    pub created: Vec<String>,
    /// Merged attachment key -> kept attachment key, in merge order
    /// (`remapAttachmentKeys`).
    pub remapped_attachment_keys: Vec<(String, String)>,
}

/// Why a merge cannot run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// No item has this key.
    UnknownItem(String),
    /// The master is also among the other items.
    MasterAmongOthers,
    /// Not a top-level regular item ("pane.item.duplicates.onlyTopLevel").
    NotTopLevel(String),
    /// Item types differ ("pane.item.duplicates.onlySameItemType").
    DifferentItemTypes,
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownItem(k) => write!(f, "no item with key {k}"),
            Self::MasterAmongOthers => {
                write!(f, "the master item is among the items merged into it")
            }
            Self::NotTopLevel(k) => write!(f, "item {k} is not a top-level regular item"),
            Self::DifferentItemTypes => write!(f, "only items of the same type can be merged"),
        }
    }
}

impl std::error::Error for MergeError {}

/// The master and the other items as the duplicates pane orders them
/// (`setItems` + `setMaster(0)`, duplicatesMergePane.js:78-198): every item
/// must be a top-level regular item of one type; they are sorted by Date
/// Added (stable), the first is the master.
pub fn merge_pane_order(
    library: &ZoteroLibrary,
    keys: &[String],
) -> Result<(String, Vec<String>), MergeError> {
    let mut items: Vec<&ZoteroItem> = Vec::new();
    for k in keys {
        let it = library
            .item(k)
            .ok_or_else(|| MergeError::UnknownItem(k.clone()))?;
        if !it.item_type.is_regular() || it.parent_item.is_some() {
            return Err(MergeError::NotTopLevel(k.clone()));
        }
        if items.first().is_some_and(|f| f.item_type != it.item_type) {
            return Err(MergeError::DifferentItemTypes);
        }
        items.push(it);
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        let (x, y) = (&items[a].date_added, &items[b].date_added);
        match (x, y) {
            (Some(x), Some(y)) => cmp_utf16(x, y),
            _ => std::cmp::Ordering::Equal,
        }
    });
    let mut sorted = order.into_iter().map(|i| keys[i].clone());
    let master = sorted.next().ok_or(MergeError::MasterAmongOthers)?;
    Ok((master, sorted.collect()))
}

fn js_has_value(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        // `val === 0` counts; other falsy numbers (NaN) cannot occur in JSON.
        Some(_) => true,
    }
}

/// `val1 !== val2` in JavaScript: arrays and objects are distinct objects.
fn js_strict_ne(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (Some(Value::Array(_) | Value::Object(_)), _)
        | (_, Some(Value::Array(_) | Value::Object(_))) => true,
        (a, b) => a != b,
    }
}

/// `Zotero.Item#multiDiff(otherItems, ignoreFields)` (item.js:5291) over
/// the items' JSON: for each field, the values the other items have that
/// differ from the master's, in item order without repeats; `None` when
/// nothing differs. The pane calls it with `["dateAdded", "dateModified",
/// "accessDate"]`.
///
/// Ported: `DataObjectUtilities.diff`'s scalar branch (add/modify; deletes
/// are skipped by `multiDiff`) and `_creatorsDiff` (the whole creator list
/// as one value). Not ported: the member diffs of `collections`, `tags`,
/// `relations` and the HTML diff of `note`, whose alternatives the merge
/// pane does not offer (it keeps the master's collections, tags and
/// relations and lets `mergeItems` union them).
pub fn field_alternatives(
    master: &ZoteroItem,
    others: &[&ZoteroItem],
    ignore: &[&str],
) -> Option<BTreeMap<String, Vec<Value>>> {
    let this = master.to_json_value();
    let this = this.as_object().cloned().unwrap_or_default();
    let mut alternatives: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut has_diffs = false;
    let skip = |f: &str| f == "key" || f == "version" || ignore.contains(&f);
    let unported = ["collections", "tags", "relations", "note", "conditions"];
    for other in others {
        let that = other.to_json_value();
        let that = that.as_object().cloned().unwrap_or_default();
        let mut changes: Vec<(String, Value)> = Vec::new();
        for (field, v1) in &this {
            if skip(field) {
                continue;
            }
            let v2 = that.get(field);
            if !js_has_value(Some(v1)) && !js_has_value(v2) {
                continue;
            }
            if unported.contains(&field.as_str()) {
                continue;
            }
            if field == "creators" {
                let empty2 = v2.and_then(Value::as_array).is_none_or(|a| a.is_empty());
                if !empty2 && Some(v1) != v2 {
                    changes.push((field.clone(), v2.cloned().unwrap_or(Value::Null)));
                }
                continue;
            }
            if js_strict_ne(Some(v1), v2) && js_has_value(v2) {
                changes.push((field.clone(), v2.cloned().unwrap_or(Value::Null)));
            }
        }
        for (field, v2) in &that {
            if skip(field) || this.contains_key(field) || unported.contains(&field.as_str()) {
                continue;
            }
            let empty = match v2 {
                Value::Bool(false) | Value::Null => true,
                Value::String(s) => s.is_empty(),
                Value::Object(o) => o.is_empty(),
                Value::Array(a) => a.is_empty(),
                _ => false,
            };
            if !empty {
                changes.push((field.clone(), v2.clone()));
            }
        }
        for (field, value) in changes {
            let list = alternatives.entry(field).or_default();
            let identity = matches!(value, Value::Array(_) | Value::Object(_));
            if list.is_empty() || identity || !list.contains(&value) {
                has_diffs = true;
                list.push(value);
            }
        }
    }
    has_diffs.then_some(alternatives)
}

/// `getMostCommonWords(s, n)` (mergeItems.mjs:425): words are runs of
/// letters (`\p{Letter}`, lower-cased) between whitespace, other
/// characters dropped, kept when longer than 3 UTF-16 units; the `n` most
/// frequent.
pub fn most_common_words(s: &str, n: usize) -> Vec<String> {
    static LETTER: OnceLock<regex::Regex> = OnceLock::new();
    let letter = LETTER.get_or_init(|| regex::Regex::new(r"^\p{L}$").expect("valid regex"));
    let mut freqs: BTreeMap<String, usize> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut flush = |w: &mut String| {
        if utf16_len(w) > 3 {
            let e = freqs.entry(w.clone()).or_insert(0);
            if *e == 0 {
                order.push(w.clone());
            }
            *e += 1;
        }
        w.clear();
    };
    let mut buf = [0u8; 4];
    for c in s.chars() {
        if is_js_whitespace(c) {
            flush(&mut current);
            continue;
        }
        if letter.is_match(c.encode_utf8(&mut buf)) {
            current.extend(c.to_lowercase());
        }
    }
    flush(&mut current);
    let fold = |w: &str| remove_diacritics(w).to_lowercase();
    order.sort_by(|a, b| {
        freqs[b]
            .cmp(&freqs[a])
            .then_with(|| cmp_utf16(&fold(a), &fold(b)))
            .then_with(|| cmp_utf16(a, b))
    });
    order.truncate(n);
    order
}

/// `Zotero.Notes.replaceAllItemKeys(item, itemKeyMap)` (notes.js:301) on a
/// note's HTML: every `%2Fitems%2F<key>` (global) and the **first**
/// `data-attachment-key="<key>"` (the second regex has no `g` flag). With
/// an empty map upstream's alternation is `()`, which this reproduces.
pub fn replace_all_item_keys(note: &str, map: &[(String, String)]) -> String {
    let keys: Vec<&str> = if map.is_empty() {
        vec![""]
    } else {
        map.iter().map(|(k, _)| k.as_str()).collect()
    };
    let lookup = |k: &str| -> String {
        map.iter()
            .find(|(f, _)| f == k)
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| "undefined".to_owned())
    };
    // Pass 1: global.
    let pat = "%2Fitems%2F";
    let mut out = String::with_capacity(note.len());
    let mut rest = note;
    while let Some(pos) = rest.find(pat) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + pat.len()..];
        match keys.iter().find(|k| after.starts_with(**k)) {
            Some(k) => {
                out.push_str(pat);
                out.push_str(&lookup(k));
                rest = &after[k.len()..];
            }
            None => {
                out.push_str(pat);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    // Pass 2: first match only.
    let pat = "data-attachment-key=\"";
    let mut search_from = 0;
    while let Some(pos) = out[search_from..].find(pat).map(|p| p + search_from) {
        let after = pos + pat.len();
        let hit = keys
            .iter()
            .find(|k| out[after..].starts_with(**k) && out[after + k.len()..].starts_with('"'));
        if let Some(k) = hit {
            let replacement = format!("{pat}{}\"", lookup(k));
            out.replace_range(pos..after + k.len() + 1, &replacement);
            break;
        }
        search_from = after;
    }
    out
}

/// `Zotero.Notes.replaceItemKey` (notes.js:318).
pub fn replace_item_key(note: &str, from: &str, to: &str) -> String {
    replace_all_item_keys(note, &[(from.to_owned(), to.to_owned())])
}

const KEY_CHARS: &[u8] = b"23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

fn derived_key(seed: &str, taken: &BTreeSet<String>) -> String {
    let mut n: u32 = 0;
    loop {
        // FNV-1a over seed + counter.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in seed.bytes().chain(n.to_le_bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        let key: String = (0..8)
            .map(|i| KEY_CHARS[((h >> (i * 7)) % KEY_CHARS.len() as u64) as usize] as char)
            .collect();
        if !taken.contains(&key) {
            return key;
        }
        n += 1;
    }
}

struct Merger<'o> {
    lib: ZoteroLibrary,
    opts: &'o MergeOptions,
    changed: BTreeSet<String>,
    created: Vec<String>,
    remap: Vec<(String, String)>,
}

impl Merger<'_> {
    fn idx(&self, key: &str) -> Result<usize, MergeError> {
        self.lib
            .items
            .iter()
            .position(|i| i.key.as_deref() == Some(key))
            .ok_or_else(|| MergeError::UnknownItem(key.to_owned()))
    }

    fn key(&self, i: usize) -> String {
        self.lib.items[i].key.clone().unwrap_or_default()
    }

    fn uri(&self, i: usize) -> String {
        self.opts.library.item_uri(&self.key(i))
    }

    fn touch(&mut self, i: usize) {
        let k = self.key(i);
        if !self.created.contains(&k) {
            self.changed.insert(k);
        }
    }

    fn children(&self, parent: usize, t: ItemType, include_trashed: bool) -> Vec<usize> {
        let pk = self.key(parent);
        self.lib
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                it.item_type == t
                    && it.parent_item.as_deref() == Some(pk.as_str())
                    && (include_trashed || it.deleted != Some(true))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn deleted(&self, i: usize) -> bool {
        self.lib.items[i].deleted == Some(true)
    }

    fn link_mode(&self, i: usize) -> Option<LinkMode> {
        self.lib.items[i]
            .attachment
            .as_ref()
            .and_then(|a| a.link_mode)
    }

    fn content_type(&self, i: usize) -> Option<String> {
        self.lib.items[i]
            .attachment
            .as_ref()
            .and_then(|a| a.content_type.clone())
    }

    fn is_file(&self, i: usize) -> bool {
        self.lib.items[i].item_type == ItemType::Attachment
            && self.link_mode(i) != Some(LinkMode::LinkedUrl)
    }

    fn is_pdf(&self, i: usize) -> bool {
        self.is_file(i) && self.content_type(i).as_deref() == Some("application/pdf")
    }

    fn is_web(&self, i: usize) -> bool {
        self.lib.items[i].item_type == ItemType::Attachment
            && !matches!(
                self.link_mode(i),
                Some(LinkMode::ImportedFile | LinkMode::LinkedFile)
            )
    }

    fn is_imported(&self, i: usize) -> bool {
        matches!(
            self.link_mode(i),
            Some(LinkMode::ImportedFile | LinkMode::ImportedUrl)
        )
    }

    fn evidence(&self, i: usize) -> AttachmentEvidence {
        self.opts
            .evidence
            .get(&self.key(i))
            .cloned()
            .unwrap_or_else(|| AttachmentEvidence::from_item(&self.lib.items[i]))
    }

    fn bytes_hash(&self, i: usize) -> Option<String> {
        let e = self.evidence(i);
        e.file_exists.then_some(e.md5).flatten()
    }

    /// `hashAttachmentText` (mergeItems.mjs:388).
    fn text_hash(&self, i: usize) -> Option<String> {
        let e = self.evidence(i);
        if !e.file_exists {
            return None;
        }
        let text = e.text.filter(|t| !t.is_empty())?;
        let mut words = most_common_words(&text, 50);
        if words.len() < 10 {
            return None;
        }
        words.sort_by(|a, b| cmp_utf16(a, b));
        Some(format!("text:{}", words.join(" ")))
    }

    fn has_embedded_annotations(&self, i: usize) -> bool {
        if !self.is_pdf(i) {
            return false;
        }
        self.evidence(i).has_embedded_annotations.unwrap_or(true)
    }

    /// `hashItem` (mergeItems.mjs:362).
    fn hash_item(&self, item: usize, text: bool) -> Vec<(String, usize)> {
        self.children(item, ItemType::Attachment, false)
            .into_iter()
            .filter(|&a| self.is_file(a))
            .filter_map(|a| {
                let h = if text {
                    self.text_hash(a)
                } else {
                    self.bytes_hash(a)
                };
                h.map(|h| (h, a))
            })
            .collect()
    }

    fn set_parent(&mut self, i: usize, parent: usize) {
        let pk = self.key(parent);
        if self.lib.items[i].parent_item.as_deref() != Some(pk.as_str()) {
            self.lib.items[i].parent_item = Some(pk);
            self.touch(i);
        }
    }

    fn trash(&mut self, i: usize) {
        if self.lib.items[i].deleted != Some(true) {
            self.lib.items[i].deleted = Some(true);
            self.touch(i);
        }
    }

    /// `moveRelations(fromItem, toItem)` (mergeItems.mjs:466).
    fn move_relations(&mut self, from: usize, to: usize) {
        let from_uri = self.uri(from);
        let to_uri = self.uri(to);
        let old = self.lib.items[from].relations.clone();
        for (p, objs) in &old {
            for o in objs {
                if *o != to_uri
                    && add_relation(&mut self.lib.items[to].relations, p, o).unwrap_or(false)
                {
                    self.touch(to);
                }
            }
        }
        let repl: Vec<String> =
            relations_by_predicate(&self.lib.items[from].relations, REPLACED_ITEM_PREDICATE)
                .to_vec();
        for r in repl {
            remove_relation(
                &mut self.lib.items[from].relations,
                REPLACED_ITEM_PREDICATE,
                &r,
            );
            self.touch(from);
        }
        for s in subjects_by_object(&self.lib, ObjectType::Item, &from_uri) {
            if s.predicate == REPLACED_ITEM_PREDICATE || s.index == to {
                continue;
            }
            let rels = &mut self.lib.items[s.index].relations;
            remove_relation(rels, &s.predicate, &from_uri);
            let _ = add_relation(rels, &s.predicate, &to_uri);
            self.touch(s.index);
        }
        if add_relation(
            &mut self.lib.items[to].relations,
            REPLACED_ITEM_PREDICATE,
            &from_uri,
        )
        .unwrap_or(false)
        {
            self.touch(to);
        }
    }

    /// `Zotero.Items.moveChildItems` (items.js:1018): annotations of a file
    /// attachment, except external ones, move to `to`.
    fn move_child_items(&mut self, from: usize, to: usize) {
        if !self.is_file(from) {
            return;
        }
        for a in self.children(from, ItemType::Annotation, true) {
            let external =
                self.lib.items[a].other.get("annotationIsExternal") == Some(&Value::Bool(true));
            if external {
                continue;
            }
            self.set_parent(a, to);
        }
    }

    /// `moveEmbeddedNote` (mergeItems.mjs:450).
    fn move_embedded_note(&mut self, from: usize, to: usize) {
        let note = self.lib.items[from].note.clone().unwrap_or_default();
        if note.is_empty() {
            return;
        }
        let (from_key, to_key) = (self.key(from), self.key(to));
        let target = if self.lib.items[to]
            .note
            .as_deref()
            .is_some_and(|n| !n.is_empty())
        {
            let taken: BTreeSet<String> = self
                .lib
                .items
                .iter()
                .filter_map(|i| i.key.clone())
                .collect();
            let key = derived_key(&format!("note:{from_key}:{to_key}"), &taken);
            let mut n = ZoteroItem::new(ItemType::Note);
            n.key = Some(key.clone());
            n.parent_item = self.lib.items[to].parent_item.clone();
            self.lib.items.push(n);
            self.created.push(key);
            self.lib.items.len() - 1
        } else {
            to
        };
        self.lib.items[target].note = Some(replace_item_key(&note, &from_key, &to_key));
        self.lib.items[from].note = Some(String::new());
        self.touch(from);
        self.touch(target);
    }

    fn do_merge(&mut self, from: usize, to: usize, merged: &mut BTreeSet<usize>) {
        merged.insert(to);
        self.move_child_items(from, to);
        self.move_embedded_note(from, to);
        self.move_relations(from, to);
        self.trash(from);
        let (fk, tk) = (self.key(from), self.key(to));
        match self.remap.iter_mut().find(|(f, _)| *f == fk) {
            Some(e) => e.1 = tk,
            None => self.remap.push((fk, tk)),
        }
    }

    /// `mergePDFAttachments` (mergeItems.mjs:94).
    fn merge_pdfs(&mut self, master: usize, others: &[usize]) {
        let mut hashes: BTreeMap<String, usize> =
            self.hash_item(master, false).into_iter().collect();
        let mut with_text = false;
        for &other in others {
            let mut merged: BTreeSet<usize> = BTreeSet::new();
            for a in self.children(other, ItemType::Attachment, true) {
                if !self.is_pdf(a) {
                    continue;
                }
                if self.deleted(a) {
                    self.set_parent(a, master);
                    continue;
                }
                let mut m = self.bytes_hash(a).and_then(|h| hashes.get(&h).copied());
                if m.is_none()
                    && !self
                        .children(master, ItemType::Attachment, false)
                        .is_empty()
                {
                    if !with_text {
                        hashes.extend(self.hash_item(master, true));
                        with_text = true;
                    }
                    m = self.text_hash(a).and_then(|h| hashes.get(&h).copied());
                }
                let m = match m {
                    Some(m) if !merged.contains(&m) => m,
                    _ => {
                        self.set_parent(a, master);
                        continue;
                    }
                };
                if self.content_type(m) != self.content_type(a) {
                    self.set_parent(a, master);
                    continue;
                }
                let same_class = (self.is_imported(m) && self.is_imported(a))
                    || (self.link_mode(m) == Some(LinkMode::LinkedFile)
                        && self.link_mode(a) == Some(LinkMode::LinkedFile));
                if !same_class {
                    self.set_parent(a, master);
                    continue;
                }
                if self.has_embedded_annotations(a) {
                    if !self.has_embedded_annotations(m) {
                        self.do_merge(m, a, &mut merged);
                    }
                    self.set_parent(a, master);
                    continue;
                }
                self.do_merge(a, m, &mut merged);
            }
        }
    }

    /// `mergeWebAttachments` (mergeItems.mjs:225).
    fn merge_web(&mut self, master: usize, others: &[usize]) {
        let mut candidates: Vec<usize> = self
            .children(master, ItemType::Attachment, false)
            .into_iter()
            .filter(|&a| self.is_web(a))
            .filter(|&a| {
                self.link_mode(a) == Some(LinkMode::LinkedUrl) || self.evidence(a).file_exists
            })
            .collect();
        let title =
            |s: &Self, i: usize| s.lib.items[i].field(Field::Title).unwrap_or("").to_owned();
        let url = |s: &Self, i: usize| s.lib.items[i].field(Field::Url).unwrap_or("").to_owned();
        for &other in others {
            for a in self.children(other, ItemType::Attachment, true) {
                if !self.is_web(a) {
                    continue;
                }
                if self.deleted(a) {
                    self.set_parent(a, master);
                    continue;
                }
                let lm = self.link_mode(a);
                let found = candidates
                    .iter()
                    .copied()
                    .find(|&m| {
                        title(self, m) == title(self, a)
                            && url(self, m) == url(self, a)
                            && self.link_mode(m) == lm
                    })
                    .or_else(|| {
                        (lm != Some(LinkMode::LinkedUrl))
                            .then(|| {
                                candidates.iter().copied().find(|&m| {
                                    title(self, m) == title(self, a) && self.link_mode(m) == lm
                                })
                            })
                            .flatten()
                    });
                let Some(m) = found else {
                    self.set_parent(a, master);
                    continue;
                };
                self.trash(a);
                self.move_relations(a, m);
                let a_uri = self.uri(a);
                if add_relation(
                    &mut self.lib.items[m].relations,
                    REPLACED_ITEM_PREDICATE,
                    &a_uri,
                )
                .unwrap_or(false)
                {
                    self.touch(m);
                }
                candidates.retain(|&c| c != m);
            }
        }
    }

    /// `mergeOtherAttachments` (mergeItems.mjs:286).
    fn merge_other_attachments(&mut self, master: usize, others: &[usize]) {
        for &other in others {
            for a in self.children(other, ItemType::Attachment, true) {
                if self.is_pdf(a) || self.is_web(a) {
                    continue;
                }
                self.set_parent(a, master);
            }
        }
    }

    /// `Zotero.Item#addTag(name, type)` (item.js:4805).
    fn add_tag(&mut self, item: usize, name: &str, tag_type: u8) {
        let stored = (tag_type != 0).then_some(tag_type);
        let tags = &mut self.lib.items[item].tags;
        if let Some(t) = tags.iter_mut().find(|t| t.tag == name) {
            if t.tag_type.unwrap_or(0) == tag_type {
                return;
            }
            t.tag_type = stored;
        } else {
            tags.push(Tag {
                tag: name.to_owned(),
                tag_type: stored,
            });
        }
        self.touch(item);
    }
}

/// `mergeItems(item, otherItems)` (mergeItems.mjs:3): merge the items with
/// keys `others` into `master`, returning the new library (see the module
/// docs for every rule). The input library is not modified.
pub fn merge_items(
    library: &ZoteroLibrary,
    master: &str,
    others: &[String],
    options: &MergeOptions,
) -> Result<MergeOutcome, MergeError> {
    if others.iter().any(|o| o == master) {
        return Err(MergeError::MasterAmongOthers);
    }
    let mut m = Merger {
        lib: library.clone(),
        opts: options,
        changed: BTreeSet::new(),
        created: Vec::new(),
        remap: Vec::new(),
    };
    let mi = m.idx(master)?;
    let ois: Vec<usize> = others.iter().map(|k| m.idx(k)).collect::<Result<_, _>>()?;

    let mut earliest = m.lib.items[mi].date_added.clone();
    m.merge_pdfs(mi, &ois);
    m.merge_web(mi, &ois);
    m.merge_other_attachments(mi, &ois);

    for &oi in &ois {
        if let (Some(o), Some(e)) = (&m.lib.items[oi].date_added, &earliest) {
            if cmp_utf16(o, e).is_lt() {
                earliest = Some(o.clone());
            }
        }
        let (ok, mk) = (m.key(oi), m.key(mi));
        for n in m.children(oi, ItemType::Note, true) {
            m.set_parent(n, mi);
            let html = m.lib.items[n].note.clone().unwrap_or_default();
            let html = replace_item_key(&html, &ok, &mk);
            let html = replace_all_item_keys(&html, &m.remap);
            if m.lib.items[n].note.as_deref() != Some(html.as_str()) {
                m.lib.items[n].note = Some(html);
                m.touch(n);
            }
        }
        m.move_relations(oi, mi);
        for c in m.lib.items[oi].collections.clone() {
            if !m.lib.items[mi].collections.contains(&c) {
                m.lib.items[mi].collections.push(c);
                m.touch(mi);
            }
        }
        for t in m.lib.items[oi].tags.clone() {
            let master_type = m.lib.items[mi]
                .tags
                .iter()
                .find(|x| x.tag == t.tag)
                .map(|x| x.tag_type.unwrap_or(0));
            if master_type == Some(0) {
                continue;
            }
            m.add_tag(mi, &t.tag, t.tag_type.unwrap_or(0));
        }
        m.trash(oi);
    }
    if m.lib.items[mi].date_added != earliest {
        m.lib.items[mi].date_added = earliest;
        m.touch(mi);
    }

    Ok(MergeOutcome {
        changed: m.changed.into_iter().collect(),
        created: m.created,
        remapped_attachment_keys: m.remap,
        library: m.lib,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_all_item_keys_global_and_first_only() {
        let note = "a%2Fitems%2FAAAA2222 b%2Fitems%2FAAAA2222 \
                    data-attachment-key=\"AAAA2222\" data-attachment-key=\"AAAA2222\"";
        let out = replace_all_item_keys(note, &[("AAAA2222".into(), "BBBB3333".into())]);
        assert_eq!(
            out,
            "a%2Fitems%2FBBBB3333 b%2Fitems%2FBBBB3333 \
             data-attachment-key=\"BBBB3333\" data-attachment-key=\"AAAA2222\""
        );
    }

    #[test]
    fn replace_all_item_keys_with_an_empty_map_reproduces_upstream() {
        let out = replace_all_item_keys("x%2Fitems%2FKEY23456", &[]);
        assert_eq!(out, "x%2Fitems%2FundefinedKEY23456");
    }

    #[test]
    fn most_common_words_rules() {
        let w = most_common_words("The quick, quick brown fox; QUICK foxes bröwn 123 abc", 2);
        assert_eq!(w, vec!["quick".to_string(), "brown".to_string()]);
    }

    #[test]
    fn derived_keys_use_zotero_key_chars() {
        let k = derived_key("x", &BTreeSet::new());
        assert_eq!(k.len(), 8);
        assert!(k.bytes().all(|b| KEY_CHARS.contains(&b)));
    }
}
