//! V&V of `kovan-cli zotero` (GitHub #752, epic #747): import a Zotero
//! library or file into a Kovan folder, export to every Zotero format.
//!
//! **Methodology.** Black-box: the compiled `kovan-cli` binary runs against
//! throwaway folders in a temp dir. The Zotero data folder is built the way
//! kovan-literature's reader tests build them, from upstream Zotero's own
//! `system.sql`/`userdata.sql`/`triggers.sql` and `Item#_saveData` SQL
//! (`crates/kovan-literature/tests/zotero_db/mod.rs`, shared by path; Zotero
//! commit 9cbba8c4d281, AGPL-3.0), filled with upstream's `itemJSON.js` (37
//! regular items, one per type) plus a hand-built article with a child note
//! and a stored PDF, a trashed book and a standalone note. File imports use
//! the translator fixtures under `crates/kovan-literature/tests/data/zotero/`.
//! No real Zotero library is read (the opt-in test aside, which reads only
//! the folder the user names in `KOVAN_ZOTERO_DATA_DIR`).
//!
//! **Predictions, written before the first run (2026-10-07).**
//! 1. A data-folder import writes 38 papers (37 + the article; the trashed
//!    book and the standalone note are not imported), each with
//!    `kovan.toml`, the Markdown stub and the document file, 38
//!    bibliography entries, and a report; the PDF is referenced in place.
//! 2. Loading the folder back gives, for every paper, exactly the Zotero
//!    item (children nested) the import started from: lossless.
//! 3. Every export translator writes a non-empty file from the folder; the
//!    BibTeX, RIS and CSL JSON exports re-import into a fresh folder as 38
//!    papers with the same titles.
//! 4. Importing the same data folder again writes no paper and changes no
//!    existing file (all 38 "already present"); importing the same BibTeX
//!    file twice writes it twice under new ids and suffixed citekeys.
//! 5. A target inside this repository or a `reactor-literature` folder is
//!    refused and nothing is created; an existing export file is not
//!    overwritten without `--force`.
//! 6. A folder written before this change (an ingested PDF paper, a topic,
//!    a hand-formatted bibliography) keeps every file byte-identical, the
//!    bibliography's old text as an exact prefix, and the same index entry
//!    for the old paper, after a Zotero import into it.
//!
//! **Results (2026-10-07, `cargo test --release -p kovan --test
//! zotero_cli`):** predictions 1 and 3-6 held on the first run. **Prediction
//! 2 was refuted:** the article (no `citationKey`, a stored PDF) came back
//! with its derived slug as a new `citationKey` and its stored file as an
//! extra linked-file attachment. The defect was in kovan-common's
//! `ZoteroItem::from_kovan_document`, whose "lossless" claim had been tested
//! only on fixtures that all carry a `citationKey` and no file; it was fixed
//! there (and a unit test added), and prediction 2 then held, unchanged.
//! **The tests can fail:** with the import made to ignore documents already
//! in the folder, and the bibliography re-rendered instead of appended to,
//! the round-trip test and the schema-compatibility test failed
//! (2026-10-07); both mutations were reverted.

#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../../kovan-literature/tests/zotero_db/mod.rs"]
mod zotero_db;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use kovan::entity::{Access, EntityConfig};
use kovan::index::KnowledgeIndex;
use kovan::root::{KovanRoot, RootConfig};
use kovan::zotero::load::load_items;
use kovan_common::zotero::{Field, ZoteroItem};
use kovan_literature::zotero::local_library::import::{import, ImportOptions};
use kovan_literature::zotero::local_library::{read_data_folder, ReadOptions};
use kovan_literature::zotero::translators::Translator;
use serde_json::{json, Value};
use zotero_db::{IdOrder, ZoteroDb};

const ITEM_JSON: &str = include_str!("../../kovan-literature/tests/data/zotero/itemJSON.json");
const BIB_FIXTURE: &str =
    "../kovan-literature/tests/data/zotero/fixtures/import/dates_and_markup.bib";

fn kovan(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kovan-cli"))
        .args(args)
        .output()
        .expect("spawn kovan-cli")
}

fn ok(args: &[&str]) -> String {
    let out = kovan(args);
    assert!(
        out.status.success(),
        "kovan-cli {args:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// `name: value` from line-oriented output.
fn count(out: &str, name: &str) -> usize {
    out.lines()
        .find_map(|l| l.strip_prefix(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("no {name} in\n{out}"))
        .trim()
        .parse()
        .unwrap()
}

fn item(v: Value) -> ZoteroItem {
    ZoteroItem::from_json_value(&v).unwrap()
}

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// The test Zotero data folder (see the module docs). Returns its path.
fn build_zotero(dir: &Path) -> PathBuf {
    let zdir = dir.join("Zotero");
    let db = ZoteroDb::create(&zdir, IdOrder::Forward);
    let items: serde_json::Map<String, Value> = serde_json::from_str(ITEM_JSON).unwrap();
    for v in items.values() {
        db.save_item(1, &item(v.clone()));
    }
    let stamp = json!("2015-04-12T09:00:22Z");
    db.save_item(
        1,
        &item(json!({
            "key": "ARTICLE1", "version": 3, "itemType": "journalArticle",
            "title": "Pebble bed heat transfer", "date": "March 2010",
            "publicationTitle": "Nuclear Engineering and Design", "volume": "240",
            "DOI": "10.1016/j.nucengdes.2009.01.001",
            "creators": [{"creatorType": "author", "firstName": "Ada", "lastName": "Lovelace"}],
            "tags": [{"tag": "pebble bed"}], "collections": [], "relations": {},
            "dateAdded": stamp, "dateModified": stamp})),
    );
    db.save_item(
        1,
        &item(json!({
            "key": "NOTE0001", "version": 1, "itemType": "note", "parentItem": "ARTICLE1",
            "note": "<p>Read section 3</p>", "tags": [], "relations": {},
            "dateAdded": stamp, "dateModified": stamp})),
    );
    db.save_item(
        1,
        &item(json!({
            "key": "PDFSTOR1", "version": 1, "itemType": "attachment", "parentItem": "ARTICLE1",
            "title": "Full Text PDF", "linkMode": "imported_file",
            "contentType": "application/pdf", "charset": "", "filename": "paper.pdf",
            "tags": [], "relations": {}, "dateAdded": stamp, "dateModified": stamp})),
    );
    db.save_item(
        1,
        &item(json!({
            "key": "TRASHED1", "version": 1, "itemType": "book", "title": "Discarded",
            "creators": [], "tags": [], "collections": [], "relations": {}, "deleted": true,
            "dateAdded": stamp, "dateModified": stamp})),
    );
    db.save_item(
        1,
        &item(json!({
            "key": "NOTE0002", "version": 1, "itemType": "note", "note": "<p>loose</p>",
            "tags": [], "collections": [], "relations": {},
            "dateAdded": stamp, "dateModified": stamp})),
    );
    drop(db);
    let sdir = zdir.join("storage").join("PDFSTOR1");
    std::fs::create_dir_all(&sdir).unwrap();
    std::fs::write(sdir.join("paper.pdf"), b"%PDF-1.4 synthetic stand-in\n").unwrap();
    zdir
}

/// Every file under `dir` except the disposable `.kovan/` state, by
/// relative path.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_path_buf();
            if rel.starts_with(".kovan") || rel.starts_with(".git") {
                continue;
            }
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn titles(items: &[kovan::zotero::load::CorpusItem]) -> Vec<String> {
    let mut v: Vec<String> = items
        .iter()
        .map(|c| c.item.field_via_base(Field::Title).unwrap_or("").to_owned())
        .collect();
    v.sort();
    v
}

/// **Result (2026-10-07):** pass; every translator of `Translator::ALL`
/// once, directions matching its metadata.
#[test]
fn formats_lists_every_translator_once_with_its_directions() {
    let out = ok(&["zotero", "formats"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), Translator::ALL.len());
    for t in Translator::ALL {
        let line = lines
            .iter()
            .find(|l| l.split('\t').next() == Some(t.format_name()))
            .unwrap_or_else(|| panic!("{} missing", t.format_name()));
        let dirs = line.split('\t').nth(1).unwrap();
        assert_eq!(dirs.contains("import"), t.metadata().can_import(), "{line}");
        assert_eq!(dirs.contains("export"), t.metadata().can_export(), "{line}");
    }
}

/// Predictions 1-4 (data folder). **Result (2026-10-07):** pass after the
/// kovan-common fix (see the module docs): 38 papers written, 38
/// bibliography entries, one report, the PDF referenced in place and found
/// by kovan's PDF lookup; all 38 load back equal to the imported items;
/// every export translator wrote a non-empty file; BibTeX, RIS and CSL JSON
/// re-imported as 38 papers with the same 38 titles; the second import
/// wrote 0 papers (38 already present) and changed no file.
#[test]
fn data_folder_import_export_reimport_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let zdir = build_zotero(tmp.path());
    let lib = tmp.path().join("lib");

    // Dry run: nothing written, not even the folder.
    let dry = ok(&[
        "zotero",
        "import",
        s(&zdir),
        "--to",
        s(&lib),
        "--init",
        "--dry-run",
    ]);
    assert!(!dry.is_empty());
    assert!(!lib.exists());

    let out = ok(&["zotero", "import", s(&zdir), "--to", s(&lib), "--init"]);
    assert_eq!(count(&out, "written"), 38, "{out}");
    assert_eq!(count(&out, "attachments_referenced"), 1, "{out}");
    let root = KovanRoot::open(&lib).unwrap();
    let dirs = root.paper_dirs();
    assert_eq!(dirs.len(), 38);
    for d in &dirs {
        let c = d.file_name().unwrap().to_str().unwrap();
        assert!(d.join("kovan.toml").is_file(), "{c}");
        assert!(d.join(format!("{c}.md")).is_file(), "{c}");
        assert!(d.join(format!("{c}.kovan-document.json")).is_file(), "{c}");
        let cfg = EntityConfig::load(d).unwrap();
        assert_eq!(cfg.source.as_ref().unwrap().access, Access::Restricted);
    }
    let bib = std::fs::read_to_string(root.bibliography_path()).unwrap();
    assert_eq!(kovan_literature::parse_bib_entries(&bib).unwrap().len(), 38);
    assert_eq!(KnowledgeIndex::rebuild(&root).papers.len(), 38);
    let reports: Vec<_> = std::fs::read_dir(lib.join("zotero-imports"))
        .unwrap()
        .collect();
    assert_eq!(reports.len(), 1);
    // The PDF is referenced where Zotero stores it, and kovan finds it.
    let pdf = zdir.join("storage/PDFSTOR1/paper.pdf");
    let article = dirs
        .iter()
        .find(|d| {
            d.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("lovelace2010")
        })
        .expect("the article's paper");
    let found = kovan::standard_corpus::paper_pdf(
        &root,
        &kovan::standard_corpus::StandardCorpus::with_checkouts(vec![]),
        article.file_name().unwrap().to_str().unwrap(),
    );
    assert_eq!(
        found.unwrap().canonicalize().unwrap(),
        pdf.canonicalize().unwrap()
    );

    // Lossless: every paper loads back as the item the import started from.
    let folder = read_data_folder(&zdir, &ReadOptions::default()).unwrap();
    let originals = import(&folder, &ImportOptions::default());
    let (loaded, warnings) = load_items(&[lib.clone()]).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.len(), 38);
    for d in &originals.documents {
        let want = d.document.zotero_item.as_ref().unwrap();
        let got = loaded
            .iter()
            .find(|c| c.item.key.as_deref() == Some(d.key.as_str()))
            .unwrap_or_else(|| panic!("{} not loaded", d.key));
        assert!(got.lossless);
        assert_eq!(&got.item, want, "{}", d.key);
    }

    // Every export translator writes something.
    for t in Translator::ALL
        .into_iter()
        .filter(|t| t.metadata().can_export())
    {
        let o = tmp.path().join(format!("export.{}", t.format_name()));
        ok(&[
            "zotero",
            "export",
            "--format",
            t.format_name(),
            "--from",
            s(&lib),
            "-o",
            s(&o),
        ]);
        let text = std::fs::read_to_string(&o).unwrap();
        // Note HTML and Note Markdown export only top-level notes; this
        // library imports none (a standalone note is reported, not imported),
        // so upstream Zotero writes an empty note container: nothing for Note
        // Markdown, an empty HTML shell for Note HTML (both read from
        // kovan-literature's reference/export/note_*/ files, made by running
        // upstream). Added 2026-10-07 when those translators landed after
        // this test was written.
        match t.format_name() {
            "note_markdown" => assert_eq!(text, "", "note_markdown"),
            "note_html" => assert_eq!(
                text,
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body><div class=\"zotero-notes\"></div></body></html>",
                "note_html"
            ),
            name => assert!(!text.trim().is_empty(), "{name} export is empty"),
        }
    }
    // ... and the round-trip formats come back with the same titles.
    let want_titles = titles(&loaded);
    for name in ["bibtex", "ris", "csljson"] {
        let o = tmp.path().join(format!("export.{name}"));
        let lib2 = tmp.path().join(format!("lib-{name}"));
        let out = ok(&[
            "zotero",
            "import",
            s(&o),
            "--to",
            s(&lib2),
            "--init",
            "--format",
            name,
        ]);
        assert_eq!(count(&out, "written"), 38, "{name}: {out}");
        let (back, _) = load_items(&[lib2]).unwrap();
        assert_eq!(titles(&back), want_titles, "{name}");
    }

    // Prediction 4: a second import of the same folder writes no paper and
    // changes no existing file.
    let before = snapshot(&lib);
    let again = ok(&["zotero", "import", s(&zdir), "--to", s(&lib)]);
    assert_eq!(count(&again, "written"), 0, "{again}");
    assert_eq!(count(&again, "already_present"), 38, "{again}");
    let after = snapshot(&lib);
    for (p, bytes) in &before {
        assert_eq!(after.get(p), Some(bytes), "{} changed", p.display());
    }
    let new: Vec<_> = after.keys().filter(|p| !before.contains_key(*p)).collect();
    assert_eq!(new.len(), 1, "only the new report: {new:?}");
    assert!(new[0].starts_with("zotero-imports"));
}

/// Prediction 4 (files) and the search/duplicates commands. **Result
/// (2026-10-07):** pass: the second import wrote every entry again under
/// new `zotero:KVN…` ids and suffixed citekeys, `duplicates` reported at
/// least one set, and the quick search found the twice-imported book twice
/// and a phrase in no item zero times.
#[test]
fn a_file_imported_twice_gets_new_ids_and_shows_as_duplicates() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let bib = fixture(BIB_FIXTURE);
    let first = ok(&["zotero", "import", s(&bib), "--to", s(&lib), "--init"]);
    let n = count(&first, "written");
    assert!(n >= 2, "{first}");
    let second = ok(&["zotero", "import", s(&bib), "--to", s(&lib)]);
    assert_eq!(count(&second, "written"), n, "{second}");
    assert_eq!(count(&second, "renamed"), n, "{second}");
    let root = KovanRoot::open(&lib).unwrap();
    let ids = kovan::zotero::import::existing_documents(&root);
    assert_eq!(ids.len(), 2 * n, "distinct ids: {ids:?}");

    let dups = ok(&["zotero", "duplicates", s(&lib)]);
    assert!(count(&dups, "duplicate_sets") >= 1, "{dups}");
    let hits = ok(&["zotero", "search", s(&lib), "Second Record"]);
    assert_eq!(count(&hits, "matches"), 2, "{hits}");
    let none = ok(&["zotero", "search", s(&lib), "no such words anywhere"]);
    assert_eq!(count(&none, "matches"), 0, "{none}");
}

/// `--copy-attachments` copies the PDF into the proprietary repository and
/// records it relative to the paper. **Result (2026-10-07):** pass.
#[test]
fn copy_attachments_copies_the_pdf_into_the_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let zdir = build_zotero(tmp.path());
    let lib = tmp.path().join("lib");
    let out = ok(&[
        "zotero",
        "import",
        s(&zdir),
        "--to",
        s(&lib),
        "--init",
        "--copy-attachments",
    ]);
    assert_eq!(count(&out, "attachments_copied"), 1, "{out}");
    let root = KovanRoot::open(&lib).unwrap();
    let copied: Vec<_> = std::fs::read_dir(root.restricted_sources_dir())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pdf"))
        .collect();
    assert_eq!(copied.len(), 1);
    let citekey = copied[0].file_stem().unwrap().to_str().unwrap();
    let cfg = EntityConfig::load(&root.paper_dir(citekey)).unwrap();
    assert!(cfg.source.unwrap().pdf.unwrap().is_relative());
}

/// Prediction 5. **Result (2026-10-07):** pass: refused inside this crate's
/// folder and inside a `reactor-literature` folder, nothing created; an
/// existing export file kept without `--force`, replaced with it.
#[test]
fn targets_inside_the_repository_are_refused_and_exports_never_overwrite() {
    let bib = fixture(BIB_FIXTURE);
    let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("zotero-import-refusal-test");
    let out = kovan(&["zotero", "import", s(&bib), "--to", s(&inside), "--init"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("refusing"));
    assert!(!inside.exists(), "nothing created inside the repository");

    let tmp = tempfile::tempdir().unwrap();
    let rl = tmp.path().join("reactor-literature").join("lib");
    let out = kovan(&["zotero", "import", s(&bib), "--to", s(&rl), "--init"]);
    assert!(!out.status.success());
    assert!(!rl.exists());

    let lib = tmp.path().join("lib");
    ok(&["zotero", "import", s(&bib), "--to", s(&lib), "--init"]);
    let o = inside.with_extension("ris");
    let out = kovan(&[
        "zotero",
        "export",
        "--format",
        "ris",
        "--from",
        s(&lib),
        "-o",
        s(&o),
    ]);
    assert!(!out.status.success());
    assert!(!o.exists());

    let o = tmp.path().join("mine.ris");
    std::fs::write(&o, "keep me").unwrap();
    let out = kovan(&[
        "zotero",
        "export",
        "--format",
        "ris",
        "--from",
        s(&lib),
        "-o",
        s(&o),
    ]);
    assert!(!out.status.success());
    assert_eq!(std::fs::read_to_string(&o).unwrap(), "keep me");
    ok(&[
        "zotero",
        "export",
        "--format",
        "ris",
        "--from",
        s(&lib),
        "-o",
        s(&o),
        "--force",
    ]);
    assert!(std::fs::read_to_string(&o).unwrap().contains("TY  -"));
}

/// A tiny one-page PDF with an `/Info` `/Title` (as `src/ingest.rs` builds).
fn write_test_pdf(path: &Path, title: &str) {
    use lopdf::{dictionary, Document, Object};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id });
    let pages =
        dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let info_id = doc.add_object(dictionary! { "Title" => Object::string_literal(title) });
    doc.trailer.set("Info", info_id);
    doc.save(path).unwrap();
}

/// Prediction 6, the schema-compatibility proof. **Result (2026-10-07):**
/// pass: every pre-existing file (paper `kovan.toml` and Markdown, topic
/// entities, `kovan_root.toml`, `.gitignore`, the stored PDF) byte-identical;
/// the old bibliography, including a hand-formatted entry and a comment, an
/// exact prefix of the new one with its entries parsing identically; the
/// old paper's `kovan.toml` and index entry unchanged; 38 papers added.
#[test]
fn a_folder_written_before_this_change_is_unchanged_by_an_import() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let root = KovanRoot::create(&lib, RootConfig::new("lib", "Lib"), false).unwrap();
    // An ingested PDF paper, through the existing ingestion path.
    let pdf = tmp.path().join("in.pdf");
    write_test_pdf(&pdf, "An Older Study");
    let p = kovan::ingest::preview(&root, &pdf).unwrap();
    let old_key = p.suggested_citekey.clone();
    kovan::ingest::ingest(
        &root,
        &p,
        kovan::ingest::IngestChoice {
            citekey: old_key.clone(),
            access: Access::Restricted,
            topics: vec!["htgrs/materials".into()],
            projects: vec![],
            target: None,
        },
    )
    .unwrap();
    // A hand-formatted entry the renderer would not reproduce byte for byte.
    let bib_path = root.bibliography_path();
    let mut old_bib = std::fs::read_to_string(&bib_path).unwrap();
    old_bib.push_str("\n% my own comment\n@book{handwritten1999,\n    title={Kept   As Typed},\n    year=1999}\n");
    std::fs::write(&bib_path, &old_bib).unwrap();
    let old_entries = kovan_literature::parse_bib_entries(&old_bib).unwrap();
    let old_index = KnowledgeIndex::rebuild(&root);
    let old_cfg = EntityConfig::load(&root.paper_dir(&old_key)).unwrap();
    let before = snapshot(&lib);

    let zdir = build_zotero(tmp.path());
    ok(&["zotero", "import", s(&zdir), "--to", s(&lib)]);

    let after = snapshot(&lib);
    for (p, bytes) in &before {
        if p == Path::new("bibliography.bib") {
            continue;
        }
        assert_eq!(after.get(p), Some(bytes), "{} changed", p.display());
    }
    let new_bib = std::fs::read_to_string(&bib_path).unwrap();
    assert!(
        new_bib.starts_with(&old_bib),
        "old bibliography kept as a prefix"
    );
    let new_entries = kovan_literature::parse_bib_entries(&new_bib).unwrap();
    assert_eq!(&new_entries[..old_entries.len()], &old_entries[..]);
    assert_eq!(new_entries.len(), old_entries.len() + 38);
    let root = KovanRoot::open(&lib).unwrap();
    assert_eq!(
        EntityConfig::load(&root.paper_dir(&old_key)).unwrap(),
        old_cfg
    );
    let new_index = KnowledgeIndex::rebuild(&root);
    for old in &old_index.papers {
        assert!(new_index.papers.contains(old), "{old:?}");
    }
    assert_eq!(new_index.papers.len(), old_index.papers.len() + 38);
    // The old paper still exports, through its bibliography entry.
    let (items, warnings) = load_items(&[lib.clone()]).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let old = items.iter().find(|c| c.label == old_key).unwrap();
    assert!(!old.lossless);
    assert_eq!(
        old.item.field_via_base(Field::Title),
        Some("An Older Study")
    );
}

/// Opt-in: import the real Zotero data folder named by
/// `KOVAN_ZOTERO_DATA_DIR` into a temporary Kovan folder (deleted
/// afterwards) and print counts only. Skipped when the variable is unset.
/// The data folder is only read (through an in-memory copy); nothing from
/// it is written anywhere but the temp dir, and nothing enters the
/// repository.
#[test]
fn opt_in_real_library_import_counts() {
    let Some(dir) = std::env::var_os("KOVAN_ZOTERO_DATA_DIR") else {
        eprintln!("KOVAN_ZOTERO_DATA_DIR not set: skipping the real-library import");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let out = ok(&[
        "zotero",
        "import",
        dir.to_str().unwrap(),
        "--to",
        s(&lib),
        "--init",
    ]);
    // Counts only (`name: number`); per-item lines name the user's papers.
    for l in out.lines() {
        if l.split_once(": ")
            .is_some_and(|(_, v)| v.trim().parse::<u64>().is_ok())
        {
            eprintln!("{l}");
        }
    }
    let ris = tmp.path().join("all.ris");
    ok(&[
        "zotero",
        "export",
        "--format",
        "ris",
        "--from",
        s(&lib),
        "-o",
        s(&ris),
    ]);
    eprintln!(
        "ris export: {} bytes",
        std::fs::metadata(&ris).unwrap().len()
    );
}
