//! Bringing a PDF into a Kovan root — GitHub issue #35 §22-23
//! (`op-9vo6.9`, "PDF ingestion into the root").
//!
//! Two-step API, matching §22's "attempt automatically, then ask only
//! meaningful classification": [`preview`] runs the automatic half
//! (metadata extraction, BibTeX-entry generation, a citekey collision
//! check) without writing anything; [`ingest`] runs §23's write
//! transaction once the caller (a GUI form, a CLI prompt) has the user's
//! SOURCE/TOPICS/PROJECTS choice.
//!
//! # Duplicate detection, scoped
//!
//! §23 step 2 asks for "duplicate check". This pass implements the case
//! that actually matters before anything is written — the *citekey*
//! [`IngestPreview::already_exists`] would collide with — rather than a
//! full content-fingerprint database (hashing every already-ingested PDF
//! against the incoming one to catch the same paper re-added under a
//! different generated key). That fuller check is real future work, not
//! done here; a citekey collision is caught both at preview time and again
//! at [`ingest`] time (the second check is what actually protects against
//! a race, not the first).
//!
//! # Reuse, not a second metadata pipeline
//!
//! Metadata extraction, BibTeX-entry generation and BibTeX parsing all
//! come from `kovan_literature` (`extract_metadata`, `to_bibtex`,
//! `parse_bib_entries`, `render_entries`) — see the workspace's "search
//! before building" rule. What this module adds is the transaction that
//! turns that metadata into a paper entity under a [`KovanRoot`], which
//! `kovan_literature` has no concept of (it predates the root/entity
//! model).
//!
//! One known, deliberate divergence from `kovan_literature`'s own
//! ingestion path (`pdf_import.rs`): [`KovanDocument::visibility`], as
//! `extract_metadata` sets it, is inferred from the *source file's
//! existing path* — meaningless for a freshly picked PDF that is not yet
//! stored anywhere, and the documented cause of bead `op-nv6g` (wrongly
//! defaulting to Open for staging imports). This module never reads that
//! field; [`IngestChoice::access`] is instead always an explicit choice
//! from the caller, defaulting to [`Access::Restricted`] per §41 and
//! `DATA_POLICY.md`.

use std::path::{Path, PathBuf};

use kovan_literature::{parse_bib_entries, render_entries, to_bibtex, BibEntry};

use crate::entity::{Access, CiteKey, EntityConfig, EntityError, ENTITY_MARKER};
use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;

/// Errors from previewing or running an ingestion.
#[derive(Debug)]
pub enum IngestError {
    /// The PDF could not be read at all (see `kovan_literature::extract_metadata`).
    Metadata { path: PathBuf, message: String },
    /// A paper with this citekey is already in the library.
    CiteKeyTaken { citekey: String },
    /// The chosen citekey is not safe as a directory name — see [`CiteKey`].
    Entity(EntityError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The bibliography file exists but is not valid BibTeX.
    Bib { path: PathBuf, message: String },
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Metadata { path, message } => {
                write!(
                    f,
                    "{}: could not extract metadata: {message}",
                    path.display()
                )
            }
            Self::CiteKeyTaken { citekey } => {
                write!(f, "a paper with citekey {citekey:?} already exists")
            }
            Self::Entity(e) => write!(f, "{e}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Bib { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for IngestError {}

/// What was recovered automatically from a PDF, before the user is asked
/// anything (§22's "before asking questions, attempt: fingerprint/duplicate
/// detection, title/authors/year, DOI, embedded metadata, existing BibTeX
/// match, native-text availability").
#[derive(Debug, Clone)]
pub struct IngestPreview {
    pub source_pdf: PathBuf,
    /// The citekey `kovan_literature::to_bibtex` derived from the extracted
    /// metadata. Editable by the caller before [`ingest`] — this is a
    /// suggestion, not a commitment.
    pub suggested_citekey: String,
    pub title: String,
    /// "Family, Given and Family, Given …", BibTeX name order — display
    /// only; the structured author list lives in the generated BibTeX entry.
    pub authors: String,
    pub year: Option<u32>,
    pub doi: Option<String>,
    /// The generated BibTeX entry, keyed by `suggested_citekey`. [`ingest`]
    /// rewrites its `cite_key` if the caller edited the suggestion.
    pub bib_entry: BibEntry,
    /// Whether `suggested_citekey` already names a paper in this library.
    /// Does not by itself block ingestion — the caller may pick a different
    /// citekey — but a caller that ingests anyway without changing it will
    /// hit [`IngestError::CiteKeyTaken`] from [`ingest`].
    pub already_exists: bool,
}

/// Run the automatic-detection half of §22 over `pdf_path`. Writes nothing.
pub fn preview(root: &KovanRoot, pdf_path: &Path) -> Result<IngestPreview, IngestError> {
    let doc = kovan_literature::extract_metadata(pdf_path).map_err(|e| IngestError::Metadata {
        path: pdf_path.to_path_buf(),
        message: e.to_string(),
    })?;

    let bibtex_text = to_bibtex(&doc);
    let bib_entry = parse_bib_entries(&bibtex_text)
        .ok()
        .and_then(|mut v| v.pop())
        .ok_or_else(|| IngestError::Metadata {
            path: pdf_path.to_path_buf(),
            message: "could not derive a BibTeX entry from the extracted metadata".to_string(),
        })?;

    let authors = doc
        .authors
        .iter()
        .map(|a| {
            if a.given.is_empty() {
                a.family.clone()
            } else {
                format!("{}, {}", a.family, a.given)
            }
        })
        .collect::<Vec<_>>()
        .join(" and ");

    let already_exists = root
        .paper_dir(&bib_entry.cite_key)
        .join(ENTITY_MARKER)
        .is_file();

    Ok(IngestPreview {
        source_pdf: pdf_path.to_path_buf(),
        suggested_citekey: bib_entry.cite_key.clone(),
        title: doc.title,
        authors,
        year: doc.year,
        doi: doc.doi,
        bib_entry,
        already_exists,
    })
}

/// What the user picked in §22's classification prompt.
#[derive(Debug, Clone)]
pub struct IngestChoice {
    /// The citekey to actually use — normally `preview.suggested_citekey`,
    /// unedited.
    pub citekey: String,
    /// Defaults to [`Access::Restricted`] at the call site that builds this
    /// (the GUI form), never here — §41: an unknown-provenance PDF must not
    /// silently become Open.
    pub access: Access,
    pub topics: Vec<String>,
    pub projects: Vec<String>,
}

/// Run §23's write transaction: store the PDF, create/update the
/// bibliography, create the paper directory and its `kovan.toml` +
/// canonical Markdown stub, and refresh the derived index cache.
///
/// §23 step 10 ("open Research workspace") is deliberately not this
/// function's job — it is GUI navigation, not a filesystem write, and the
/// Research workspace itself is `op-9vo6.25`'s later step. A caller opens
/// it itself once this returns `Ok`.
pub fn ingest(
    root: &KovanRoot,
    preview: &IngestPreview,
    choice: IngestChoice,
) -> Result<(), IngestError> {
    let citekey = CiteKey::parse(&choice.citekey).map_err(IngestError::Entity)?;

    if root
        .paper_dir(citekey.as_str())
        .join(ENTITY_MARKER)
        .is_file()
    {
        return Err(IngestError::CiteKeyTaken {
            citekey: citekey.as_str().to_string(),
        });
    }

    // §23 step 3: store the PDF under open/restricted source storage.
    let store_dir = if choice.access.is_committable() {
        root.open_sources_dir()
    } else {
        root.restricted_sources_dir()
    };
    std::fs::create_dir_all(&store_dir).map_err(|source| IngestError::Io {
        path: store_dir.clone(),
        source,
    })?;
    let dest_pdf = store_dir.join(format!("{}.pdf", citekey.as_str()));
    std::fs::copy(&preview.source_pdf, &dest_pdf).map_err(|source| IngestError::Io {
        path: dest_pdf.clone(),
        source,
    })?;

    // §23 step 4: create/update the bibliography.
    let mut entry = preview.bib_entry.clone();
    entry.cite_key = citekey.as_str().to_string();
    append_bib_entry(root, entry)?;

    // §23 steps 5-7: paper directory, kovan.toml, canonical Markdown stub.
    let paper_dir = root.paper_dir(citekey.as_str());
    let mut config = EntityConfig::paper(citekey.clone(), choice.access);
    if !choice.topics.is_empty() || !choice.projects.is_empty() {
        // op-8aq6: a classification naming a topic/project path that has no
        // backing collection entity yet made the paper permanently
        // unreachable by Wiki drill-down — create whatever's missing first.
        crate::entity::ensure_classification_paths(root, &choice.topics, &choice.projects)
            .map_err(IngestError::Entity)?;
        config = config
            .with_topics(choice.topics)
            .with_projects(choice.projects);
    }
    // else: leave EntityConfig::paper's default Classification::unsorted()
    // in place — §7's inbox for rapid ingestion, and what keeps `validate`
    // satisfiable without forcing the user to classify before ingesting.
    let config = config.with_pdf(relative_to(&paper_dir, &dest_pdf));
    config.save_paper(&paper_dir).map_err(IngestError::Entity)?;

    // §23 step 9: update the derived index/graph. Best-effort — a failure
    // here does not undo the ingestion; the next `load_or_rebuild` self-heals.
    let _ = KnowledgeIndex::rebuild(root).save_cache(root);

    Ok(())
}

/// Append `entry` to `root`'s bibliography, creating the file if absent,
/// atomically (temp file + rename). Rejects a citekey already present, same
/// as the entity-directory check in [`ingest`] — this one is what actually
/// guards against a concurrent-write race, since it re-reads the file
/// immediately before writing rather than trusting an earlier check.
fn append_bib_entry(root: &KovanRoot, entry: BibEntry) -> Result<(), IngestError> {
    let bib_path = root.bibliography_path();
    let mut entries = if bib_path.is_file() {
        let text = std::fs::read_to_string(&bib_path).map_err(|source| IngestError::Io {
            path: bib_path.clone(),
            source,
        })?;
        parse_bib_entries(&text).map_err(|e| IngestError::Bib {
            path: bib_path.clone(),
            message: format!("{e:?}"),
        })?
    } else {
        Vec::new()
    };
    if entries.iter().any(|e| e.cite_key == entry.cite_key) {
        return Err(IngestError::CiteKeyTaken {
            citekey: entry.cite_key,
        });
    }
    entries.push(entry);
    entries.sort_by(|a, b| a.cite_key.cmp(&b.cite_key));

    let text = render_entries(&entries);
    let tmp_path = PathBuf::from(format!("{}.tmp", bib_path.display()));
    std::fs::write(&tmp_path, text).map_err(|source| IngestError::Io {
        path: tmp_path.clone(),
        source,
    })?;
    std::fs::rename(&tmp_path, &bib_path).map_err(|source| IngestError::Io {
        path: bib_path,
        source,
    })
}

/// Express `target` relative to `base` (both absolute paths under the same
/// root). A small hand-rolled component diff rather than a new dependency —
/// this is the only place in the crate that needs it. Assumes neither path
/// contains `..` or a symlink hop, true of every path this module builds
/// from [`KovanRoot`]'s own accessors.
fn relative_to(base: &Path, target: &Path) -> PathBuf {
    let base_comps: Vec<_> = base.components().collect();
    let target_comps: Vec<_> = target.components().collect();
    let common = base_comps
        .iter()
        .zip(target_comps.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut out = PathBuf::new();
    for _ in common..base_comps.len() {
        out.push("..");
    }
    for comp in &target_comps[common..] {
        out.push(comp.as_os_str());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Classification;
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    /// A tiny, structurally valid one-page PDF with an `/Info` `/Title` —
    /// enough for `extract_metadata`'s Info-dictionary path, no real text
    /// content needed.
    fn write_test_pdf(path: &Path, title: &str) {
        use lopdf::{dictionary, Document, Object};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id });
        let pages = dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let info_id = doc.add_object(dictionary! { "Title" => Object::string_literal(title) });
        doc.trailer.set("Info", info_id);
        doc.save(path).unwrap();
    }

    #[test]
    fn relative_to_computes_a_sibling_subtree_path() {
        let base = PathBuf::from("/lib/papers/wang2018multiphysics");
        let target = PathBuf::from("/lib/literature/open/wang2018multiphysics.pdf");
        let rel = relative_to(&base, &target);
        assert_eq!(
            rel,
            PathBuf::from("../../literature/open/wang2018multiphysics.pdf")
        );
    }

    #[test]
    fn preview_then_ingest_creates_a_classified_paper() {
        let (dir, root) = make_root();
        let pdf_path = dir.path().join("incoming.pdf");
        write_test_pdf(&pdf_path, "A Multiphysics Study");

        let p = preview(&root, &pdf_path).unwrap();
        assert!(!p.already_exists);
        assert_eq!(p.title, "A Multiphysics Study");

        let choice = IngestChoice {
            citekey: p.suggested_citekey.clone(),
            access: Access::Open,
            topics: vec!["htgrs".to_string()],
            projects: vec![],
        };
        ingest(&root, &p, choice).unwrap();

        let paper_dir = root.paper_dir(&p.suggested_citekey);
        assert!(paper_dir.join("kovan.toml").is_file());
        assert!(paper_dir
            .join(format!("{}.md", p.suggested_citekey))
            .is_file());
        let stored_pdf = root
            .open_sources_dir()
            .join(format!("{}.pdf", p.suggested_citekey));
        assert!(stored_pdf.is_file());

        let bib_text = std::fs::read_to_string(root.bibliography_path()).unwrap();
        assert!(bib_text.contains(&p.suggested_citekey));

        let index = KnowledgeIndex::rebuild(&root);
        assert!(index.has_paper(&p.suggested_citekey));
    }

    #[test]
    fn ingesting_an_already_taken_citekey_is_rejected() {
        let (dir, root) = make_root();
        let pdf_path = dir.path().join("incoming.pdf");
        write_test_pdf(&pdf_path, "Same Title Twice");

        let p = preview(&root, &pdf_path).unwrap();
        let choice = |topics: Vec<&str>| IngestChoice {
            citekey: p.suggested_citekey.clone(),
            access: Access::Restricted,
            topics: topics.into_iter().map(String::from).collect(),
            projects: vec![],
        };
        ingest(&root, &p, choice(vec!["htgrs"])).unwrap();

        let err = ingest(&root, &p, choice(vec!["htgrs"])).unwrap_err();
        assert!(matches!(err, IngestError::CiteKeyTaken { .. }));
    }

    #[test]
    fn no_topics_or_projects_chosen_falls_back_to_unsorted() {
        let (dir, root) = make_root();
        let pdf_path = dir.path().join("incoming.pdf");
        write_test_pdf(&pdf_path, "Unfiled Paper");
        let p = preview(&root, &pdf_path).unwrap();

        let choice = IngestChoice {
            citekey: p.suggested_citekey.clone(),
            access: Access::Restricted,
            topics: vec![],
            projects: vec![],
        };
        ingest(&root, &p, choice).unwrap();

        let config = EntityConfig::load(&root.paper_dir(&p.suggested_citekey)).unwrap();
        assert_eq!(config.classification, Classification::unsorted());
    }
}
