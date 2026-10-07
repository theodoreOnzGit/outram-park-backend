// Part of the kovan Zotero port (GitHub #747, #752). No upstream logic is
// ported here: this module wires the ported pieces (kovan-literature's
// translators and local-library reader, kovan-common's item model,
// kovan-metrics' import report, kovan-semantics' duplicate detection and
// kovan-discovery's search) into a Kovan folder.

//! Zotero in a Kovan folder: import a Zotero library (or a file in any
//! importable Zotero format) as papers, and load a folder's papers back as
//! Zotero items for export, duplicate detection and search. The CLI front
//! end is `kovan-cli zotero` ([`crate::commands::zotero`]).
//!
//! ## Where an imported item lands
//!
//! In the **same layout as a PDF ingested through [`crate::ingest`]**, so the
//! rest of kovan (index, Wiki, mind map, PDF reader) sees it with no change:
//!
//! ```text
//! <kovan folder>/
//! ├── bibliography.bib                      one entry per paper, APPENDED (existing bytes kept)
//! ├── papers/<year|undated>/<citekey>/
//! │   ├── kovan.toml                         paper entity, access = "restricted", topic "unsorted"
//! │   ├── <citekey>.md                       the canonical Markdown stub (`## Summary`)
//! │   └── <citekey>.kovan-document.json      NEW, additive: the full KovanDocument, Zotero item included
//! ├── literature/proprietary/<citekey>.pdf   only with --copy-attachments
//! └── zotero-imports/<UTC time>.md           the import report (kovan-metrics' Markdown + what was written)
//! ```
//!
//! The citekey and bibliography entry come from `kovan_literature::to_bibtex`
//! on the imported [`KovanDocument`], exactly as ingestion derives them from
//! a PDF's metadata. A PDF attachment is **referenced in place** (an absolute
//! `[source] pdf` path into the Zotero `storage/` folder) unless the caller
//! asks for it to be copied into the folder's proprietary repository.
//!
//! ## Schema safety (maintainer rule, #747: "kovan schemas never break")
//!
//! Nothing existing is re-typed, renamed or rewritten. `kovan.toml`,
//! `kovan_root.toml` and the Markdown stub are written by the existing code
//! ([`crate::entity::EntityConfig::save_paper`]) in the existing format; the
//! one new file per paper (`<citekey>.kovan-document.json`) is kovan-common's
//! existing canonical serde form of [`KovanDocument`] (what `kovan-cli lit
//! import --json-out` writes), which nothing else reads, so a folder without
//! it is simply a folder with no Zotero papers. The bibliography is
//! **appended to**, not re-rendered, so every existing entry stays
//! byte-identical. Every new file is created with `create_new`: nothing
//! existing is overwritten. Proved by `tests/zotero_cli.rs`.
//!
//! ## Data policy
//!
//! A Zotero library is the user's own, usually proprietary, data. Every
//! imported paper is `access = "restricted"` (the Zotero conversion records
//! no redistribution right; `DATA_POLICY.md`). [`protected_location`] refuses
//! a target inside the outram-park-backend repository or the
//! `reactor-literature` open-literature submodule; the CLI only overrides it
//! with an explicit flag.

pub mod import;
pub mod load;

use std::path::{Path, PathBuf};

pub use kovan_common::KovanDocument;

/// The suffix of the per-paper file holding the full [`KovanDocument`] of an
/// imported item: `papers/<year>/<citekey>/<citekey>.kovan-document.json`.
pub const DOCUMENT_SUFFIX: &str = ".kovan-document.json";

/// The folder (under the Kovan folder) the import reports are written to.
pub const REPORT_DIR: &str = "zotero-imports";

/// The document file of the paper in `paper_dir` (whose directory name is
/// its citekey).
pub fn document_path(paper_dir: &Path) -> PathBuf {
    let citekey = paper_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    paper_dir.join(format!("{citekey}{DOCUMENT_SUFFIX}"))
}

/// `path` made absolute and canonical as far as it exists (the part that
/// does not exist yet is appended unchanged), so a target that is about to
/// be created can still be checked against the protected locations.
fn resolve(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|d| d.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut existing = abs.as_path();
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if let Ok(c) = existing.canonicalize() {
            let mut out = c;
            for t in tail.iter().rev() {
                out.push(t);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                existing = parent;
            }
            _ => return abs,
        }
    }
}

/// Whether `dir` is a checkout of this repository (outram-park-backend):
/// it holds the workspace `Cargo.toml` and this crate.
fn is_this_repository(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file() && dir.join("crates/kovan/Cargo.toml").is_file()
}

/// The protected location `target` is inside, with what it is, or `None`.
///
/// Protected (data policy, #747): any checkout of this repository
/// (outram-park-backend, found by its layout or, for the build this binary
/// came from, by its source path) and any folder named `reactor-literature`
/// (the open-literature submodule, which is published). A Zotero library is
/// the user's own data and goes to their own Kovan folder.
pub fn protected_location(target: &Path) -> Option<(PathBuf, &'static str)> {
    let target = resolve(target);
    let source_checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .ok()
        .filter(|d| is_this_repository(d));
    for dir in target.ancestors() {
        if dir.file_name().is_some_and(|n| n == "reactor-literature") {
            return Some((
                dir.to_path_buf(),
                "the reactor-literature open-literature submodule",
            ));
        }
        if is_this_repository(dir) || source_checkout.as_deref() == Some(dir) {
            return Some((dir.to_path_buf(), "the outram-park-backend repository"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_crates_own_folder_is_protected() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("not-yet/created");
        let (dir, what) = protected_location(&inside).expect("protected");
        assert!(what.contains("outram-park-backend"));
        assert!(inside.canonicalize().is_err(), "nothing created");
        assert!(dir.join("crates/kovan/Cargo.toml").is_file());
    }

    #[test]
    fn a_reactor_literature_folder_is_protected_and_a_temp_folder_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(protected_location(tmp.path()).is_none());
        let rl = tmp.path().join("reactor-literature").join("x");
        assert!(
            protected_location(&rl).is_some_and(|(_, what)| what.contains("reactor-literature"))
        );
    }

    #[test]
    fn document_path_is_named_after_the_citekey() {
        assert_eq!(
            document_path(Path::new("/lib/papers/2010/smith2010")),
            PathBuf::from("/lib/papers/2010/smith2010/smith2010.kovan-document.json")
        );
    }
}
