// Part of the kovan Zotero port (GitHub #747, #752). No upstream logic is
// ported here (see `super`'s header).

//! Import a Zotero data folder, or a file in any importable Zotero format,
//! into a Kovan folder: [`read_source`] → [`plan`] → [`write`] (the module
//! docs of [`super`] give the layout and the schema-safety argument).
//!
//! Reuse, not reimplementation: the data folder is read by
//! `kovan_literature::zotero::local_library::read_data_folder`; a file is
//! read by the ported Zotero translator ([`Translator::import`], detected
//! with [`detect_import`]); both become [`KovanDocument`]s through
//! `local_library::import::import` (children grouped in Zotero's export
//! shape, `zotero_item` kept), and the loss accounting is kovan-metrics'
//! `import_library` report.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use kovan_common::zotero::{ItemType, LinkMode, ZoteroLibrary};
use kovan_common::KovanDocument;
use kovan_literature::zotero::framework::api_json::ALLOWED_KEY_CHARS;
use kovan_literature::zotero::local_library::import::{import, ImportOptions, Losses};
use kovan_literature::zotero::local_library::{
    read_data_folder, AttachmentFile, DbSource, LibraryKind, LocalLibrary, ReadOptions, ReadReport,
    SchemaVersions, ZoteroDataFolder, BUNDLED_USERDATA_VERSION,
};
use kovan_literature::zotero::translators::{detect_import, Translator};
use kovan_literature::{parse_bib_entries, render_entries, to_bibtex, BibEntry};

use super::{document_path, REPORT_DIR};
use crate::entity::{Access, CiteKey, EntityConfig, ENTITY_MARKER};
use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;

/// What is being imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    /// A Zotero data folder (holding `zotero.sqlite`).
    DataFolder(PathBuf),
    /// A file, read with this import translator.
    File {
        /// The file.
        path: PathBuf,
        /// The translator.
        translator: Translator,
    },
}

/// Work out what `source` is: a Zotero data folder (a directory holding
/// `zotero.sqlite` or `zotero.sqlite.bak`, or the `zotero.sqlite` file
/// itself), else a file whose format is `format` (a translator's format
/// name, [`Translator::format_name`]) or, without one, the first import
/// translator whose `detectImport` accepts it, in the translation-server's
/// order ([`detect_import`]).
pub fn detect_source(source: &Path, format: Option<&str>) -> Result<ImportSource, String> {
    if source.is_dir() {
        if source.join("zotero.sqlite").is_file() || source.join("zotero.sqlite.bak").is_file() {
            return Ok(ImportSource::DataFolder(source.to_path_buf()));
        }
        return Err(format!(
            "{} is a folder without zotero.sqlite: not a Zotero data folder \
             (Zotero: Settings > Advanced > Files and Folders > Data Directory Location)",
            source.display()
        ));
    }
    if source.file_name().is_some_and(|n| n == "zotero.sqlite") {
        let dir = source.parent().unwrap_or(Path::new(".")).to_path_buf();
        return Ok(ImportSource::DataFolder(dir));
    }
    let translator = match format {
        Some(name) => {
            let t = Translator::from_format_name(name).ok_or_else(|| {
                format!("unknown format {name:?}; `kovan-cli zotero formats` lists them")
            })?;
            if !t.metadata().can_import() {
                return Err(format!("{} ({name}) is export-only", t.metadata().label));
            }
            t
        }
        None => {
            let text = read_text(source)?;
            *detect_import(&text).first().ok_or_else(|| {
                format!(
                    "{}: no import translator recognises this file; name one with --format",
                    source.display()
                )
            })?
        }
    };
    Ok(ImportSource::File {
        path: source.to_path_buf(),
        translator,
    })
}

fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// How to read the source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceOptions {
    /// Zotero's `baseAttachmentPath` preference, to resolve linked files
    /// stored relative to it (`attachments:` paths). Data folders only.
    pub base_attachment_path: Option<PathBuf>,
    /// The first generated item key for a file import (see
    /// [`next_generated_key_index`]); data-folder items keep their keys.
    pub first_key_index: u64,
}

/// Read `source` into the shape the local-library import takes: a data
/// folder as Zotero stores it, or a file's items as one user library (keys
/// generated from [`SourceOptions::first_key_index`]).
pub fn read_source(
    source: &ImportSource,
    opts: &SourceOptions,
) -> Result<ZoteroDataFolder, String> {
    match source {
        ImportSource::DataFolder(dir) => {
            let read = ReadOptions {
                base_attachment_path: opts.base_attachment_path.clone(),
                ..ReadOptions::default()
            };
            read_data_folder(dir, &read).map_err(|e| format!("{}: {e}", dir.display()))
        }
        ImportSource::File { path, translator } => {
            let text = read_text(path)?;
            let mut options = translator.default_options();
            options.first_key_index = opts.first_key_index;
            let result = translator
                .import(&text, &options)
                .map_err(|e| format!("{}: {e:?}", path.display()))?;
            let items = result
                .zotero_items()
                .map_err(|e| format!("{}: {e:?}", path.display()))?;
            // A linked file named by the import (RIS `L1`, BibTeX `file`) is
            // referenced where it is; anything else has no file.
            let mut files = BTreeMap::new();
            for it in items.iter().filter(|i| i.item_type == ItemType::Attachment) {
                let (Some(key), Some(att)) = (it.key.clone(), it.attachment.as_ref()) else {
                    continue;
                };
                let file = match att.path.as_deref() {
                    Some(p) if Path::new(p).is_absolute() => {
                        AttachmentFile::Linked(PathBuf::from(p))
                    }
                    Some(p) => AttachmentFile::Unresolvable(format!("relative path {p:?}")),
                    None if att.link_mode == Some(LinkMode::LinkedUrl) => AttachmentFile::NoFile,
                    None => AttachmentFile::Unresolvable("no path".into()),
                };
                files.insert(key, file);
            }
            let mut report = ReadReport::default();
            report.notes.push(format!(
                "read {} with the {} import translator",
                path.display(),
                translator.metadata().label
            ));
            Ok(ZoteroDataFolder {
                data_dir: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
                source: DbSource::LiveCopy,
                schema: SchemaVersions {
                    userdata: BUNDLED_USERDATA_VERSION,
                    system: None,
                    triggers: None,
                    compatibility: None,
                },
                libraries: vec![LocalLibrary {
                    library_id: 1,
                    kind: LibraryKind::User,
                    editable: true,
                    files_editable: true,
                    version: 0,
                    uri: "http://zotero.org/users/local/kovan".into(),
                    contents: ZoteroLibrary {
                        // The translation-server drops collections too.
                        collections: Vec::new(),
                        items,
                        searches: Vec::new(),
                    },
                    files,
                }],
                report,
            })
        }
    }
}

/// The number of a generated key (`KVN` + five base-33 digits,
/// `KeyGenerator::key`), or `None` for any other key.
fn generated_key_index(key: &str) -> Option<u64> {
    let digits = key.strip_prefix("KVN")?;
    if digits.chars().count() != 5 {
        return None;
    }
    digits.chars().try_fold(0u64, |n, c| {
        let d = ALLOWED_KEY_CHARS.chars().position(|a| a == c)?;
        Some(n * 33 + d as u64)
    })
}

/// The first generated-key index not used by any imported paper of `root`,
/// so a second file import into the same folder does not reuse the keys
/// (and so the ids, `zotero:KVN…`) of the first.
pub fn next_generated_key_index(root: &KovanRoot) -> u64 {
    existing_documents(root)
        .keys()
        .filter_map(|id| id.strip_prefix("zotero:"))
        .filter_map(generated_key_index)
        .map(|n| n + 1)
        .max()
        .unwrap_or(0)
}

/// Every imported paper of `root`: document id → citekey.
pub fn existing_documents(root: &KovanRoot) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for dir in root.paper_dirs() {
        let Ok(text) = std::fs::read_to_string(document_path(&dir)) else {
            continue;
        };
        if let Ok(doc) = serde_json::from_str::<KovanDocument>(&text) {
            let citekey = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.insert(doc.id, citekey);
        }
    }
    out
}

/// What to import, and how.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanOptions {
    /// Import Zotero's trashed items too.
    pub include_trashed: bool,
    /// Topic paths to file every paper under (default: `unsorted`).
    pub topics: Vec<String>,
}

/// One paper to write.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedPaper {
    /// Its citekey (directory and file name).
    pub citekey: String,
    /// The Zotero library and key it came from.
    pub library_id: i64,
    /// The Zotero item key.
    pub key: String,
    /// The document (written as `<citekey>.kovan-document.json`).
    pub document: KovanDocument,
    /// The bibliography entry.
    pub bib: BibEntry,
    /// The paper directory, `papers/<year|undated>/<citekey>/`.
    pub dir: PathBuf,
    /// The attachment file to reference (or copy), when one is on disk.
    pub attachment: Option<PathBuf>,
}

/// What an import would do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportPlan {
    /// Papers to write.
    pub papers: Vec<PlannedPaper>,
    /// Documents already in the folder (same id): `(id, existing citekey)`.
    /// Not written again.
    pub already_present: Vec<(String, String)>,
    /// Citekeys taken by another paper (or earlier in this import), so
    /// given a suffix: `(wanted, used)`. Nothing existing is touched.
    pub renamed: Vec<(String, String)>,
    /// Ids of documents whose recorded attachment path is not a file on disk.
    pub attachment_missing: Vec<String>,
    /// What the local-library import did not carry.
    pub losses: Losses,
    /// The libraries, for the report.
    pub libraries: Vec<(String, ZoteroLibrary)>,
    /// The reader's notes and skipped/dropped counts.
    pub read_notes: Vec<String>,
}

/// The citekeys already used in `root`: paper directories and bibliography
/// entries.
fn taken_citekeys(root: &KovanRoot) -> Result<BTreeSet<String>, String> {
    let mut taken: BTreeSet<String> = root
        .paper_dirs()
        .iter()
        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    let bib = root.bibliography_path();
    if bib.is_file() {
        let text = std::fs::read_to_string(&bib).map_err(|e| format!("{}: {e}", bib.display()))?;
        let entries = parse_bib_entries(&text).map_err(|e| {
            format!(
                "{}: not valid BibTeX ({e:?}); nothing imported",
                bib.display()
            )
        })?;
        taken.extend(entries.into_iter().map(|e| e.cite_key));
    }
    Ok(taken)
}

/// A citekey for `doc` that is safe as a directory name and not in `taken`:
/// its slug (`citationKey`, else `<first author><year>`), sanitised, with
/// `a`…`z` then `-2`, `-3`… appended when taken.
fn choose_citekey(doc: &KovanDocument, key: &str, taken: &BTreeSet<String>) -> String {
    let base = CiteKey::sanitise(&doc.slug)
        .or_else(|_| CiteKey::sanitise(&format!("zotero-{key}")))
        .map(|k| k.as_str().to_owned())
        .unwrap_or_else(|_| "zotero-item".to_owned());
    let free = |k: &str| !taken.contains(k) && CiteKey::parse(k).is_ok();
    if free(&base) {
        return base;
    }
    for c in 'a'..='z' {
        let k = format!("{base}{c}");
        if free(&k) {
            return k;
        }
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|k| free(k))
        .expect("an unused suffix exists")
}

/// The bibliography entry of `doc` under `citekey`, from
/// `kovan_literature::to_bibtex` (the derivation ingestion uses).
fn bib_entry(doc: &KovanDocument, citekey: &str) -> Result<BibEntry, String> {
    let mut d = doc.clone();
    d.slug = citekey.to_owned();
    let mut entry = parse_bib_entries(&to_bibtex(&d))
        .ok()
        .and_then(|mut v| v.pop())
        .ok_or_else(|| format!("{}: no BibTeX entry could be derived", doc.id))?;
    entry.cite_key = citekey.to_owned();
    Ok(entry)
}

/// Plan the import of `folder` into `root`; reads `root`, writes nothing.
pub fn plan(
    root: &KovanRoot,
    folder: &ZoteroDataFolder,
    opts: &PlanOptions,
) -> Result<ImportPlan, String> {
    let imported = import(
        folder,
        &ImportOptions {
            include_trashed: opts.include_trashed,
        },
    );
    let existing = existing_documents(root);
    let mut taken = taken_citekeys(root)?;
    let mut out = ImportPlan {
        losses: imported.losses.clone(),
        ..ImportPlan::default()
    };
    let mut seen_ids = BTreeSet::new();
    for d in imported.documents {
        let doc = d.document;
        if let Some(citekey) = existing.get(&doc.id) {
            out.already_present.push((doc.id.clone(), citekey.clone()));
            continue;
        }
        if !seen_ids.insert(doc.id.clone()) {
            // The same id from a second library (listed in the losses).
            continue;
        }
        let citekey = choose_citekey(&doc, &d.key, &taken);
        let wanted = CiteKey::sanitise(&doc.slug)
            .map(|k| k.as_str().to_owned())
            .unwrap_or_default();
        if !wanted.is_empty() && wanted != citekey {
            out.renamed.push((wanted, citekey.clone()));
        }
        taken.insert(citekey.clone());
        let bib = bib_entry(&doc, &citekey)?;
        let dir = root.new_paper_dir(&citekey, bib.fields.get("year").map(String::as_str));
        let attachment = match doc.source_path.as_deref() {
            Some(p) if Path::new(p).is_file() => Some(PathBuf::from(p)),
            Some(_) => {
                out.attachment_missing.push(doc.id.clone());
                None
            }
            None => None,
        };
        out.papers.push(PlannedPaper {
            citekey,
            library_id: d.library_id,
            key: d.key,
            document: doc,
            bib,
            dir,
            attachment,
        });
    }
    for lib in &folder.libraries {
        let name = match &lib.kind {
            LibraryKind::User => "My Library".to_owned(),
            LibraryKind::Group { name, group_id, .. } => format!("group {group_id} ({name})"),
        };
        let mut contents = lib.contents.clone();
        if opts.include_trashed {
            // Imported, so not reported as skipped.
            for it in &mut contents.items {
                it.deleted = None;
            }
        }
        out.libraries.push((name, contents));
    }
    out.read_notes = folder.report.notes.clone();
    if !folder.report.skipped.is_empty() {
        out.read_notes.push(format!(
            "{} objects skipped by the reader (unknown types)",
            folder.report.skipped.len()
        ));
    }
    if !folder.report.dropped.is_empty() {
        out.read_notes.push(format!(
            "{} values dropped by the reader (invalid for their item type, as Zotero ignores them)",
            folder.report.dropped.len()
        ));
    }
    Ok(out)
}

/// How to write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// Copy each attachment into the folder's proprietary repository
    /// (default: reference it where it is).
    pub copy_attachments: bool,
    /// Topic paths to file every paper under.
    pub topics: Vec<String>,
    /// What was imported, for the report.
    pub source_description: String,
}

/// What [`write`] wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    /// The paper directories written.
    pub papers: Vec<PathBuf>,
    /// Attachments copied into the folder.
    pub copied: Vec<PathBuf>,
    /// Attachments referenced in place.
    pub referenced: usize,
    /// Attachments not copied because the destination file existed
    /// (referenced in place instead).
    pub copy_collisions: Vec<PathBuf>,
    /// The report file.
    pub report: Option<PathBuf>,
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> String + '_ {
    move |e| format!("{}: {e}", path.display())
}

/// Write `bytes` to a file that must not exist yet.
fn create_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io(path))?;
    f.write_all(bytes).map_err(io(path))
}

/// Append `entries` to the bibliography, keeping every existing byte (the
/// file is re-written atomically as its old text plus the new entries).
fn append_bibliography(root: &KovanRoot, entries: &[BibEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let bib = root.bibliography_path();
    let mut text = if bib.is_file() {
        std::fs::read_to_string(&bib).map_err(io(&bib))?
    } else {
        String::new()
    };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&render_entries(entries));
    let tmp = PathBuf::from(format!("{}.tmp", bib.display()));
    std::fs::write(&tmp, text).map_err(io(&tmp))?;
    std::fs::rename(&tmp, &bib).map_err(io(&bib))
}

/// Write `plan` into `root`: the bibliography entries, then each paper's
/// `kovan.toml`, Markdown stub and document file, then the report. Nothing
/// existing is overwritten; a paper directory that appeared since planning
/// is an error before anything is written.
pub fn write(
    root: &KovanRoot,
    plan: &ImportPlan,
    opts: &WriteOptions,
) -> Result<ImportOutcome, String> {
    for p in &plan.papers {
        if p.dir.exists() || root.paper_dir(&p.citekey).join(ENTITY_MARKER).is_file() {
            return Err(format!(
                "{} exists (written since planning?); nothing imported",
                p.dir.display()
            ));
        }
    }
    let store = if opts.copy_attachments {
        Some(
            crate::ingest::resolve_target(root, Access::Restricted, None)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    if !opts.topics.is_empty() {
        crate::entity::ensure_classification_paths(root, &opts.topics, &[])
            .map_err(|e| e.to_string())?;
    }
    let bib: Vec<BibEntry> = plan.papers.iter().map(|p| p.bib.clone()).collect();
    append_bibliography(root, &bib)?;

    let mut out = ImportOutcome::default();
    for p in &plan.papers {
        let citekey = CiteKey::parse(&p.citekey).map_err(|e| e.to_string())?;
        let mut config = EntityConfig::paper(citekey, Access::Restricted);
        if !opts.topics.is_empty() {
            config = config.with_topics(opts.topics.clone());
        }
        if let Some(src) = &p.attachment {
            let mut recorded = src.clone();
            if let Some(repo) = &store {
                let ext = src
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                let dest = repo.dir.join(format!("{}{ext}", p.citekey));
                std::fs::create_dir_all(&repo.dir).map_err(io(&repo.dir))?;
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&dest)
                {
                    Ok(mut f) => {
                        let mut r = std::fs::File::open(src).map_err(io(src))?;
                        std::io::copy(&mut r, &mut f).map_err(io(&dest))?;
                        recorded = crate::ingest::relative_to(&p.dir, &dest);
                        config = config.with_pdf(&recorded).with_repo(repo.name.clone());
                        out.copied.push(dest);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        out.copy_collisions.push(dest);
                        out.referenced += 1;
                        config = config.with_pdf(&recorded);
                    }
                    Err(e) => return Err(format!("{}: {e}", dest.display())),
                }
            } else {
                out.referenced += 1;
                config = config.with_pdf(&recorded);
            }
        }
        config.save_paper(&p.dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&p.document).map_err(|e| e.to_string())?;
        create_new(&document_path(&p.dir), format!("{json}\n").as_bytes())?;
        out.papers.push(p.dir.clone());
    }

    let report = report_markdown(plan, &out, opts);
    let dir = root.path().join(REPORT_DIR);
    std::fs::create_dir_all(&dir).map_err(io(&dir))?;
    let stamp = crate::digitiser::dataset::utc_now_iso8601().replace(':', "-");
    let path = (1..)
        .map(|n| match n {
            1 => dir.join(format!("{stamp}.md")),
            n => dir.join(format!("{stamp}-{n}.md")),
        })
        .find(|p| !p.exists())
        .expect("a free name");
    create_new(&path, report.as_bytes())?;
    out.report = Some(path);

    // Best effort, as in ingestion: the next load rebuilds a stale cache.
    let _ = KnowledgeIndex::rebuild(root).save_cache(root);
    Ok(out)
}

/// The import report: what was written, then kovan-metrics' per-library
/// loss accounting and counts.
pub fn report_markdown(plan: &ImportPlan, out: &ImportOutcome, opts: &WriteOptions) -> String {
    let mut s = String::new();
    s.push_str("# Zotero import into kovan\n\n");
    s.push_str(&format!("Source: {}\n\n", opts.source_description));
    s.push_str(&format!(
        "Written: {} papers; {} attachments copied, {} referenced in place; \
         {} already in the folder (not written again); {} citekeys given a suffix.\n\n",
        out.papers.len(),
        out.copied.len(),
        out.referenced,
        plan.already_present.len(),
        plan.renamed.len()
    ));
    let mut lines: Vec<String> = Vec::new();
    lines.extend(plan.losses.summary());
    lines.extend(plan.read_notes.iter().cloned());
    if !plan.attachment_missing.is_empty() {
        lines.push(format!(
            "{} documents whose attachment path is not a file on disk",
            plan.attachment_missing.len()
        ));
    }
    for p in &out.copy_collisions {
        lines.push(format!(
            "not copied (file exists, referenced in place): {}",
            p.display()
        ));
    }
    if !lines.is_empty() {
        s.push_str("## Not carried, and notes\n\n");
        for l in lines {
            s.push_str(&format!("- {l}\n"));
        }
        s.push('\n');
    }
    if !plan.renamed.is_empty() {
        s.push_str("## Citekeys given a suffix\n\n| Wanted | Used |\n|---|---|\n");
        for (a, b) in &plan.renamed {
            s.push_str(&format!("| {a} | {b} |\n"));
        }
        s.push('\n');
    }
    if !plan.already_present.is_empty() {
        s.push_str("## Already in the folder\n\n| Document | Citekey |\n|---|---|\n");
        for (id, c) in &plan.already_present {
            s.push_str(&format!("| {id} | {c} |\n"));
        }
        s.push('\n');
    }
    s.push_str(
        "Standalone attachments (a PDF with no parent item) are imported as papers; \
         the loss accounting below lists them as skipped because it counts only \
         bibliographic items.\n\n",
    );
    for (name, lib) in &plan.libraries {
        s.push_str(&format!("---\n\n# Library: {name}\n\n"));
        let (_, report) = kovan_metrics::zotero::import_library(lib);
        // Demote the report's headings one level under the library heading.
        for line in report.to_markdown().lines() {
            if line.starts_with('#') {
                s.push('#');
            }
            s.push_str(line);
            s.push('\n');
        }
        s.push('\n');
        for line in kovan_metrics::zotero::LibraryCounts::of(lib)
            .to_markdown()
            .lines()
        {
            if line.starts_with('#') {
                s.push('#');
            }
            s.push_str(line);
            s.push('\n');
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_literature::zotero::framework::api_json::KeyGenerator;

    #[test]
    fn generated_keys_decode_to_their_index() {
        for n in [0u64, 1, 32, 33, 1000, 33u64.pow(5) - 1] {
            assert_eq!(generated_key_index(&KeyGenerator::key(n)), Some(n));
        }
        assert_eq!(generated_key_index("ABCD2345"), None);
    }

    #[test]
    fn a_taken_citekey_gets_a_letter_suffix() {
        let doc = KovanDocument::new(
            "zotero:K",
            "smith2010",
            kovan_common::Visibility::Proprietary,
            kovan_common::DocumentType::Paper,
            "T",
        );
        let mut taken = BTreeSet::new();
        assert_eq!(choose_citekey(&doc, "K", &taken), "smith2010");
        taken.insert("smith2010".to_owned());
        taken.insert("smith2010a".to_owned());
        assert_eq!(choose_citekey(&doc, "K", &taken), "smith2010b");
        let mut odd = doc.clone();
        odd.slug = "Citation key: with/slash".into();
        assert_eq!(choose_citekey(&odd, "K", &taken), "Citation-key-with-slash");
        odd.slug = "::".into();
        assert_eq!(choose_citekey(&odd, "ABCD", &taken), "zotero-ABCD");
    }
}
