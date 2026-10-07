// Part of the kovan Zotero port (GitHub #747, #750).
//
// No upstream logic is ported in this file: it groups the items the reader
// returned into Zotero's export shape (`attachments`/`notes` under their
// parent, as `Zotero.Utilities.Internal.itemToExportFormat` does,
// utilities_internal.js:1016, Zotero commit 9cbba8c4d281, AGPL-3.0,
// (c) Corporation for Digital Scholarship) and hands each item to
// `kovan_common::zotero`'s KovanDocument conversion.

//! Import a read Zotero data folder into kovan: one [`KovanDocument`] per
//! literature item, with a report of what the import does not carry.
//!
//! **What becomes a document.** Every top-level regular item (a book, an
//! article, ...) and every top-level ("standalone") attachment, in each user
//! and group library. Its child attachments and notes go with it in Zotero's
//! export shape (`attachments`, `notes`), so they are kept in
//! `KovanDocument::zotero_item` and survive kovan -> Zotero. The conversion
//! is `ZoteroItem::to_kovan_document` (kovan-common, #748), unchanged; this
//! module only adds `source_path` when that conversion leaves it empty: the
//! resolved file of the first PDF attachment (else the first attachment with
//! a file), since a stored file's location is known only to the reader.
//!
//! **What the import does not carry, reported in [`Losses`]:** trashed items
//! (skipped unless [`ImportOptions::include_trashed`]); standalone notes (not
//! literature); annotations (a Zotero item kind the export shape has no slot
//! for; they stay in the read [`ZoteroLibrary`](kovan_common::zotero::ZoteroLibrary));
//! collection names and the collection tree (each document keeps only its
//! collection *keys*, in `zotero_item`); saved searches; which library a
//! document came from (the id is `zotero:<KEY>`, so two libraries can give
//! the same id: listed in [`Losses::duplicate_ids`]); the attachment files
//! themselves (referenced by path, never copied); and attachments whose file
//! is missing. Every document is `Visibility::Proprietary` (the conversion's
//! rule: Zotero records no redistribution right).
//!
//! Nothing is written unless [`write_documents`] is called with a folder.
//! **Data policy (#747, workspace `DATA_POLICY.md`):** a Zotero library is the
//! user's own, usually proprietary, data; write it to the local corpus, never
//! into `reactor-literature` or this repository.

use super::{AttachmentFile, LibraryKind, LocalLibrary, ZoteroDataFolder};
use kovan_common::zotero::{ItemType, ZoteroItem};
use kovan_common::KovanDocument;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

/// What to import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportOptions {
    /// Import items in Zotero's trash too (default false).
    pub include_trashed: bool,
}

/// One imported document.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedDocument {
    /// The Zotero library it came from.
    pub library_id: i64,
    /// The Zotero item key.
    pub key: String,
    /// The document.
    pub document: KovanDocument,
}

/// What the import did not carry into kovan (see the module docs).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Losses {
    /// Trashed top-level items not imported.
    pub trashed_items_skipped: usize,
    /// Standalone (top-level) notes not imported.
    pub standalone_notes: usize,
    /// Annotations, kept only in the read library.
    pub annotations: usize,
    /// Collections whose names and nesting are not in any document.
    pub collections: usize,
    /// Saved searches not imported.
    pub saved_searches: usize,
    /// Document ids produced by more than one library.
    pub duplicate_ids: Vec<String>,
    /// Attachment keys whose file is not on disk (or unresolvable).
    pub missing_files: Vec<String>,
}

impl Losses {
    /// One line per non-empty loss, for a report.
    pub fn summary(&self) -> Vec<String> {
        let mut v = Vec::new();
        let mut n = |count: usize, what: &str| {
            if count > 0 {
                v.push(format!("{count} {what}"));
            }
        };
        n(self.trashed_items_skipped, "trashed items skipped");
        n(self.standalone_notes, "standalone notes not imported");
        n(
            self.annotations,
            "annotations kept only in the Zotero library",
        );
        n(self.collections, "collections (names, tree) not carried");
        n(self.saved_searches, "saved searches not carried");
        n(
            self.duplicate_ids.len(),
            "document ids duplicated across libraries",
        );
        n(
            self.missing_files.len(),
            "attachments without a file on disk",
        );
        v
    }
}

/// The result of [`import`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ZoteroImport {
    /// The documents, library by library, in item order.
    pub documents: Vec<ImportedDocument>,
    /// What was not carried.
    pub losses: Losses,
}

fn file_of(lib: &LocalLibrary, att: &ZoteroItem) -> Option<String> {
    let key = att.key.as_deref()?;
    lib.files
        .get(key)
        .and_then(AttachmentFile::path)
        .map(|p| p.to_string_lossy().into_owned())
}

/// Turn every library of `folder` into kovan documents (no disk writes).
pub fn import(folder: &ZoteroDataFolder, opts: &ImportOptions) -> ZoteroImport {
    let mut out = ZoteroImport::default();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for lib in &folder.libraries {
        let c = &lib.contents;
        out.losses.collections += c.collections.len();
        out.losses.saved_searches += c.searches.len();
        let mut children: BTreeMap<&str, Vec<&ZoteroItem>> = BTreeMap::new();
        for it in &c.items {
            if it.item_type == ItemType::Annotation {
                out.losses.annotations += 1;
            }
            if let Some(p) = it.parent_item.as_deref() {
                children.entry(p).or_default().push(it);
            }
            if it.item_type == ItemType::Attachment {
                if let Some(k) = it.key.as_deref() {
                    let present = lib
                        .files
                        .get(k)
                        .is_some_and(|f| matches!(f, AttachmentFile::NoFile) || f.exists());
                    if !present {
                        out.losses.missing_files.push(k.to_owned());
                    }
                }
            }
        }
        for it in c.items.iter().filter(|i| i.parent_item.is_none()) {
            let regular = it.item_type.is_regular();
            if !regular && it.item_type != ItemType::Attachment {
                if it.item_type == ItemType::Note {
                    out.losses.standalone_notes += 1;
                }
                continue;
            }
            if it.deleted == Some(true) && !opts.include_trashed {
                out.losses.trashed_items_skipped += 1;
                continue;
            }
            let key = it.key.clone().unwrap_or_default();
            let mut export = it.clone();
            let kids = children.get(key.as_str()).cloned().unwrap_or_default();
            export.attachments = kids
                .iter()
                .filter(|k| k.item_type == ItemType::Attachment)
                .map(|k| (*k).clone())
                .collect();
            export.notes = kids
                .iter()
                .filter(|k| k.item_type == ItemType::Note)
                .map(|k| (*k).clone())
                .collect();
            let mut doc = export.to_kovan_document();
            if doc.source_path.is_none() {
                let candidates: Vec<&ZoteroItem> = if regular {
                    export.attachments.iter().collect()
                } else {
                    vec![it]
                };
                let is_pdf = |a: &&&ZoteroItem| {
                    a.attachment
                        .as_ref()
                        .and_then(|d| d.content_type.as_deref())
                        == Some("application/pdf")
                };
                doc.source_path = candidates
                    .iter()
                    .filter(is_pdf)
                    .find_map(|a| file_of(lib, a))
                    .or_else(|| candidates.iter().find_map(|a| file_of(lib, a)));
            }
            let n = seen.entry(doc.id.clone()).or_insert(0);
            *n += 1;
            if *n == 2 {
                out.losses.duplicate_ids.push(doc.id.clone());
            }
            out.documents.push(ImportedDocument {
                library_id: lib.library_id,
                key,
                document: doc,
            });
        }
    }
    out
}

fn library_dir(folder: &ZoteroDataFolder, library_id: i64) -> String {
    match folder
        .libraries
        .iter()
        .find(|l| l.library_id == library_id)
        .map(|l| &l.kind)
    {
        Some(LibraryKind::Group { group_id, .. }) => format!("group-{group_id}"),
        _ => "user".to_owned(),
    }
}

/// Write each document as pretty JSON to
/// `<target>/<user|group-ID>/<KEY>.json`. Never overwrites: an existing file
/// is an error (`AlreadyExists`) and nothing after it is written.
pub fn write_documents(
    folder: &ZoteroDataFolder,
    import: &ZoteroImport,
    target: &Path,
) -> std::io::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for d in &import.documents {
        let dir = target.join(library_dir(folder, d.library_id));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", d.key));
        let json = serde_json::to_string_pretty(&d.document)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        f.write_all(json.as_bytes())?;
        f.write_all(b"\n")?;
        written.push(path);
    }
    Ok(written)
}
