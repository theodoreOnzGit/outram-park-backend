// Part of the kovan Zotero port (GitHub #747, #752). No upstream logic is
// ported here (see `super`'s header).

//! Load kovan documents as Zotero items, for export, duplicate detection
//! and search.
//!
//! | Source | Each item comes from |
//! |---|---|
//! | a Kovan folder, paper with a `<citekey>.kovan-document.json` | that [`KovanDocument`] through `ZoteroItem::from_kovan_document`; **lossless** when it carries `zotero_item` (an imported Zotero item) |
//! | a Kovan folder, paper without one (ingested from a PDF) | its bibliography entry, read by the ported Zotero **BibTeX import translator** (the paper has no `KovanDocument`; the bibliography is its bibliographic truth, §9) |
//! | a `.json` file | a [`KovanDocument`] (`kovan-cli lit import --json-out`) or a JSON array of them |
//! | any other folder | every `.json` [`KovanDocument`] under it (e.g. `local_library::import::write_documents` output) |
//!
//! Items are returned in a fixed order (papers by directory, files by path),
//! each with a unique key: an item without one gets a generated key
//! (`KVN…`, `KeyGenerator`) not used by any other item.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kovan_common::zotero::{ZoteroItem, ZoteroLibrary};
use kovan_common::KovanDocument;
use kovan_literature::zotero::framework::api_json::KeyGenerator;
use kovan_literature::zotero::translators::Translator;
use kovan_literature::{parse_bib_entries, render_entry, BibEntry};

use super::document_path;
use crate::root::KovanRoot;

/// One loaded item.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusItem {
    /// Where it came from: the citekey, or the file.
    pub label: String,
    /// The item, children (attachments, notes) nested in Zotero's export
    /// shape.
    pub item: ZoteroItem,
    /// Whether it came from a stored Zotero item (`zotero_item`), so exports
    /// losslessly.
    pub lossless: bool,
}

/// The items of `sources`, plus warnings for what could not be read.
pub fn load_items(sources: &[PathBuf]) -> Result<(Vec<CorpusItem>, Vec<String>), String> {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    let mut pending_bib: Vec<(String, BibEntry, usize)> = Vec::new();
    for src in sources {
        if src.is_dir() && KovanRoot::is_root(src) {
            let root = KovanRoot::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
            load_root(&root, &mut items, &mut pending_bib, &mut warnings)?;
        } else if src.is_dir() {
            let mut files = Vec::new();
            walk_json(src, &mut files);
            files.sort();
            for f in files {
                match read_documents(&f) {
                    Ok(docs) => push_documents(&f, docs, &mut items),
                    Err(e) => warnings.push(format!("skipped {e}")),
                }
            }
        } else if src.is_file() {
            let docs = read_documents(src)?;
            push_documents(src, docs, &mut items);
        } else {
            return Err(format!("{}: no such file or folder", src.display()));
        }
    }
    // Papers without a document: their bibliography entries through Zotero's
    // BibTeX import, one entry at a time so each maps back to its paper.
    let mut used: BTreeSet<String> = items.iter().filter_map(|c| c.item.key.clone()).collect();
    let mut next = 0u64;
    let mut from_bib: Vec<(usize, CorpusItem)> = Vec::new();
    for (citekey, entry, slot) in pending_bib {
        match bib_item(&entry, &mut next, &mut used) {
            Some(item) => from_bib.push((
                slot,
                CorpusItem {
                    label: citekey,
                    item,
                    lossless: false,
                },
            )),
            None => warnings.push(format!(
                "{citekey}: the BibTeX import gave no item for its bibliography entry"
            )),
        }
    }
    // Put them back in paper order.
    for (offset, (slot, item)) in from_bib.into_iter().enumerate() {
        items.insert(slot + offset, item);
    }
    assign_missing_keys(&mut items, &mut next, &mut used);
    Ok((items, warnings))
}

fn load_root(
    root: &KovanRoot,
    items: &mut Vec<CorpusItem>,
    pending_bib: &mut Vec<(String, BibEntry, usize)>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let bib_path = root.bibliography_path();
    let bib: BTreeMap<String, BibEntry> = if bib_path.is_file() {
        let text = std::fs::read_to_string(&bib_path)
            .map_err(|e| format!("{}: {e}", bib_path.display()))?;
        parse_bib_entries(&text)
            .map_err(|e| format!("{}: {e:?}", bib_path.display()))?
            .into_iter()
            .map(|e| (e.cite_key.clone(), e))
            .collect()
    } else {
        BTreeMap::new()
    };
    for dir in root.paper_dirs() {
        let citekey = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let doc_file = document_path(&dir);
        if doc_file.is_file() {
            let text = std::fs::read_to_string(&doc_file)
                .map_err(|e| format!("{}: {e}", doc_file.display()))?;
            match serde_json::from_str::<KovanDocument>(&text) {
                Ok(doc) => {
                    items.push(CorpusItem {
                        label: citekey,
                        lossless: doc.zotero_item.is_some(),
                        item: ZoteroItem::from_kovan_document(&doc),
                    });
                    continue;
                }
                Err(e) => warnings.push(format!("{}: {e}", doc_file.display())),
            }
        }
        match bib.get(&citekey) {
            // Remember where it goes; the BibTeX import runs once keys are known.
            Some(entry) => pending_bib.push((citekey, entry.clone(), items.len())),
            None => warnings.push(format!("{citekey}: no bibliography entry, not exported")),
        }
    }
    Ok(())
}

/// The bibliography entry as a Zotero item (BibTeX import translator),
/// children nested, with keys not in `used`.
fn bib_item(entry: &BibEntry, next: &mut u64, used: &mut BTreeSet<String>) -> Option<ZoteroItem> {
    let t = Translator::BibTeX;
    let mut options = t.default_options();
    while used.contains(&KeyGenerator::key(*next)) {
        *next += 1;
    }
    options.first_key_index = *next;
    let found = t
        .import(&render_entry(entry), &options)
        .ok()?
        .zotero_items()
        .ok()?;
    *next += found.len() as u64;
    let mut parent: Option<ZoteroItem> = None;
    let mut children = Vec::new();
    for it in found {
        if it.parent_item.is_none() && parent.is_none() {
            parent = Some(it);
        } else if it.parent_item.is_some() {
            children.push(it);
        }
    }
    let mut parent = parent?;
    for c in children {
        if let Some(k) = &c.key {
            used.insert(k.clone());
        }
        if c.item_type == kovan_common::zotero::ItemType::Attachment {
            parent.attachments.push(c);
        } else {
            parent.notes.push(c);
        }
    }
    if let Some(k) = &parent.key {
        used.insert(k.clone());
    }
    Some(parent)
}

fn assign_missing_keys(items: &mut [CorpusItem], next: &mut u64, used: &mut BTreeSet<String>) {
    for c in items.iter_mut() {
        if c.item.key.as_deref().is_none_or(str::is_empty) {
            while used.contains(&KeyGenerator::key(*next)) {
                *next += 1;
            }
            let k = KeyGenerator::key(*next);
            used.insert(k.clone());
            c.item.key = Some(k);
        }
    }
}

fn walk_json(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_json(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// A `.json` file holding one [`KovanDocument`] or an array of them.
fn read_documents(path: &Path) -> Result<Vec<KovanDocument>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Ok(doc) = serde_json::from_str::<KovanDocument>(&text) {
        return Ok(vec![doc]);
    }
    serde_json::from_str::<Vec<KovanDocument>>(&text)
        .map_err(|e| format!("{}: not a kovan document ({e})", path.display()))
}

fn push_documents(path: &Path, docs: Vec<KovanDocument>, items: &mut Vec<CorpusItem>) {
    for doc in docs {
        items.push(CorpusItem {
            label: path.display().to_string(),
            lossless: doc.zotero_item.is_some(),
            item: ZoteroItem::from_kovan_document(&doc),
        });
    }
}

/// The items as one library (duplicate detection and search run on a
/// library), item order kept.
pub fn as_library(items: &[CorpusItem]) -> ZoteroLibrary {
    ZoteroLibrary {
        collections: Vec::new(),
        items: items.iter().map(|c| c.item.clone()).collect(),
        searches: Vec::new(),
    }
}
