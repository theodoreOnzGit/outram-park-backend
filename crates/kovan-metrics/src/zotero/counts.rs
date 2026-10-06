// Part of the kovan Zotero port (GitHub #747, #751). Kovan's own accounting
// over kovan-common's port of the Zotero data model; no upstream code.

//! [`LibraryCounts`]: what a [`ZoteroLibrary`] holds.

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::zotero::{ZoteroItem, ZoteroLibrary};

use super::md_cell;

/// The key used in the count maps for an attachment without a `linkMode`
/// or an annotation without an `annotationType`.
pub const MISSING: &str = "(missing)";

/// How often a tag occurs, split by tag type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TagCount {
    /// Items carrying it as a manual tag (`type` 0 or absent).
    pub manual: usize,
    /// Items carrying it as an automatic tag (`type: 1`).
    pub automatic: usize,
}

/// The items of one collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionCount {
    /// The collection key (empty when the collection has none).
    pub key: String,
    /// The collection name.
    pub name: String,
    /// The names from the top-level ancestor down, joined with ` / `.
    pub path: String,
    /// Items listing this collection in `collections`.
    pub direct: usize,
    /// Distinct items in this collection or any subcollection.
    pub recursive: usize,
    /// `parentCollection` names a key no collection of the library has.
    pub parent_missing: bool,
}

/// Counts over a [`ZoteroLibrary`].
///
/// "Items" are every [`ZoteroItem`] of the library: those in
/// `ZoteroLibrary::items` and, recursively, the children nested in their
/// `attachments` and `notes` (the translator export format).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryCounts {
    /// Every item.
    pub items: usize,
    /// Items with no `parentItem` that are not nested in another item.
    pub top_level: usize,
    /// Items with a `parentItem`, or nested in another item.
    pub children: usize,
    /// Items with `deleted: true`.
    pub trashed: usize,
    /// Item type name -> items.
    pub by_item_type: BTreeMap<String, usize>,
    /// One entry per collection, sorted by path, then key.
    pub collections: Vec<CollectionCount>,
    /// Top-level regular items (not notes, attachments or annotations) in
    /// no collection. Kovan's definition; the same set as Zotero's
    /// "Unfiled Items" view only when no item is in the trash.
    pub unfiled: usize,
    /// Tag -> occurrences.
    pub tags: BTreeMap<String, TagCount>,
    /// Attachment link mode (`imported_file`, ...) -> attachments;
    /// [`MISSING`] for an attachment without one.
    pub link_modes: BTreeMap<String, usize>,
    /// Annotation type (`highlight`, ...) -> annotations; [`MISSING`] for an
    /// annotation without one.
    pub annotation_types: BTreeMap<String, usize>,
    /// Relation predicate (`dc:relation`, `owl:sameAs`, ...) -> objects.
    pub relation_predicates: BTreeMap<String, usize>,
}

/// Every item of the library, with whether it was nested in another item.
fn all_items(lib: &ZoteroLibrary) -> Vec<(&ZoteroItem, bool)> {
    fn walk<'a>(item: &'a ZoteroItem, nested: bool, out: &mut Vec<(&'a ZoteroItem, bool)>) {
        out.push((item, nested));
        for c in item.attachments.iter().chain(item.notes.iter()) {
            walk(c, true, out);
        }
    }
    let mut out = Vec::new();
    for i in &lib.items {
        walk(i, false, &mut out);
    }
    out
}

impl LibraryCounts {
    /// Count `lib`.
    pub fn of(lib: &ZoteroLibrary) -> LibraryCounts {
        let mut c = LibraryCounts::default();
        let items = all_items(lib);
        for (item, nested) in &items {
            c.items += 1;
            if *nested || item.parent_item.is_some() {
                c.children += 1;
            } else {
                c.top_level += 1;
                if item.item_type.is_regular() && item.collections.is_empty() {
                    c.unfiled += 1;
                }
            }
            if item.deleted == Some(true) {
                c.trashed += 1;
            }
            *c.by_item_type
                .entry(item.item_type.as_str().to_owned())
                .or_default() += 1;
            // A tag listed twice on one item counts once per tag type.
            let mut seen: BTreeSet<(&str, bool)> = BTreeSet::new();
            for t in &item.tags {
                if seen.insert((t.tag.as_str(), t.is_automatic())) {
                    let e = c.tags.entry(t.tag.clone()).or_default();
                    if t.is_automatic() {
                        e.automatic += 1;
                    } else {
                        e.manual += 1;
                    }
                }
            }
            if let Some(a) = &item.attachment {
                let k = a.link_mode.map(|m| m.as_str()).unwrap_or(MISSING);
                *c.link_modes.entry(k.to_owned()).or_default() += 1;
            }
            if let Some(a) = &item.annotation {
                let k = a.annotation_type.map(|m| m.as_str()).unwrap_or(MISSING);
                *c.annotation_types.entry(k.to_owned()).or_default() += 1;
            }
            for (p, objs) in &item.relations {
                *c.relation_predicates.entry(p.clone()).or_default() += objs.len();
            }
        }
        c.collections = collection_counts(lib, &items);
        c
    }

    /// The counts as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# Zotero library counts\n\n");
        s.push_str("| Items | Top-level | Children | In the trash | Unfiled (top-level regular) | Collections | Tags |\n");
        s.push_str("|---:|---:|---:|---:|---:|---:|---:|\n");
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n\n",
            self.items,
            self.top_level,
            self.children,
            self.trashed,
            self.unfiled,
            self.collections.len(),
            self.tags.len()
        ));
        table(&mut s, "Item types", "Item type", &self.by_item_type);
        s.push_str("## Collections\n\n");
        if self.collections.is_empty() {
            s.push_str("None.\n\n");
        } else {
            s.push_str("| Collection | Key | Items | Items incl. subcollections |\n");
            s.push_str("|---|---|---:|---:|\n");
            for col in &self.collections {
                let mark = if col.parent_missing {
                    " (parent missing)"
                } else {
                    ""
                };
                s.push_str(&format!(
                    "| {}{} | {} | {} | {} |\n",
                    md_cell(&col.path, 120),
                    mark,
                    md_cell(&col.key, 20),
                    col.direct,
                    col.recursive
                ));
            }
            s.push('\n');
        }
        s.push_str("## Tags\n\n");
        if self.tags.is_empty() {
            s.push_str("None.\n\n");
        } else {
            s.push_str("| Tag | Manual | Automatic |\n|---|---:|---:|\n");
            for (t, n) in &self.tags {
                s.push_str(&format!(
                    "| {} | {} | {} |\n",
                    md_cell(t, 80),
                    n.manual,
                    n.automatic
                ));
            }
            s.push('\n');
        }
        table(
            &mut s,
            "Attachment link modes",
            "Link mode",
            &self.link_modes,
        );
        table(
            &mut s,
            "Annotation types",
            "Annotation type",
            &self.annotation_types,
        );
        table(
            &mut s,
            "Relation predicates (objects)",
            "Predicate",
            &self.relation_predicates,
        );
        s
    }
}

fn table(s: &mut String, title: &str, col: &str, m: &BTreeMap<String, usize>) {
    s.push_str(&format!("## {title}\n\n"));
    if m.is_empty() {
        s.push_str("None.\n\n");
        return;
    }
    s.push_str(&format!("| {col} | Count |\n|---|---:|\n"));
    for (k, n) in m {
        s.push_str(&format!("| {} | {n} |\n", md_cell(k, 80)));
    }
    s.push('\n');
}

fn collection_counts(lib: &ZoteroLibrary, items: &[(&ZoteroItem, bool)]) -> Vec<CollectionCount> {
    let keys: BTreeSet<&str> = lib
        .collections
        .iter()
        .filter_map(|c| c.key.as_deref())
        .collect();
    // Item indices directly in each collection key.
    let mut direct: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for (i, (item, _)) in items.iter().enumerate() {
        for k in &item.collections {
            direct.entry(k.as_str()).or_default().insert(i);
        }
    }
    // Children of each collection key.
    let mut kids: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for col in &lib.collections {
        if let (Some(k), Some(p)) = (col.key.as_deref(), col.parent_collection.as_deref()) {
            kids.entry(p).or_default().push(k);
        }
    }
    let name_of = |k: &str| lib.collections.iter().find(|c| c.key.as_deref() == Some(k));
    let mut out: Vec<CollectionCount> = lib
        .collections
        .iter()
        .map(|col| {
            let key = col.key.clone().unwrap_or_default();
            // Path, guarding against a parent cycle.
            let mut names = vec![col.name.clone()];
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            seen.insert(key.as_str());
            let mut parent = col.parent_collection.as_deref();
            let mut parent_missing = false;
            while let Some(p) = parent {
                if !seen.insert(p) {
                    break;
                }
                match name_of(p) {
                    Some(pc) => {
                        names.push(pc.name.clone());
                        parent = pc.parent_collection.as_deref();
                    }
                    None => {
                        parent_missing = !keys.contains(p);
                        break;
                    }
                }
            }
            names.reverse();
            // Recursive membership, guarding against a cycle.
            let mut all: BTreeSet<usize> = BTreeSet::new();
            let mut stack: Vec<&str> = vec![key.as_str()];
            let mut visited: BTreeSet<&str> = BTreeSet::new();
            while let Some(k) = stack.pop() {
                if !visited.insert(k) {
                    continue;
                }
                if let Some(d) = direct.get(k) {
                    all.extend(d.iter().copied());
                }
                if let Some(ks) = kids.get(k) {
                    stack.extend(ks.iter().copied());
                }
            }
            CollectionCount {
                direct: direct.get(key.as_str()).map_or(0, BTreeSet::len),
                recursive: all.len(),
                key,
                name: col.name.clone(),
                path: names.join(" / "),
                parent_missing,
            }
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.key.cmp(&b.key)));
    out
}
