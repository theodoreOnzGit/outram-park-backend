//! Standard-corpus documents as ingested literature.
//!
//! The standard corpus is two things kept apart on purpose: the metadata
//! compiled into Kovan ([`crate::corpus::LITERATURE`]), and the PDFs, which
//! are **not** shipped in the crate but pulled from a separate Git repository
//! ([`crate::corpus::CORPUS_REPOSITORY_URL`], folder
//! [`crate::corpus::STANDARD_CORPUS_FOLDER`]). This module joins the two, so
//! a standard-corpus document is treated as already in the library rather
//! than as a stray PDF to ingest again.
//!
//! # Why this exists (the bug)
//!
//! Until 2026-09-30 Kovan's only notion of "ingested" was "some paper's
//! `kovan.toml` records this PDF" (`paper_owning_pdf`, the literature list's
//! `owners` map). Nothing ever wrote a paper for a standard-corpus document,
//! so every one of them was listed as "not ingested yet", opening one offered
//! the Ingest prompt, and ingesting created a second paper under a
//! generated citekey: the maintainer ingested WASH-1400 as
//! `papers/2008/2008muffletwond`, byte-identical to the corpus file.
//!
//! # What "ingested" means for a corpus document
//!
//! - **Listed in [`crate::corpus::LITERATURE`] and its
//!   [`corpus_file`](crate::corpus::CorpusLiterature::corpus_file) present
//!   in a checkout** ([`StandardCorpus::locate`]): ingested and available.
//!   It is listed and searchable with its corpus metadata, and opens from
//!   the corpus file.
//! - **Listed but the file is absent** (not pulled yet, or a citation-only
//!   entry with no `corpus_file`): known, not downloaded
//!   ([`Availability::NotDownloaded`]); shown with its source URL, never
//!   hidden and never treated as missing data.
//!
//! # Where the checkouts are
//!
//! [`StandardCorpus::for_root`] searches, in order, every place Kovan's
//! default pull can put a corpus repository: **every standard repository**
//! of the folder ([`crate::corpus_tiers`], GitHub issue #458: the built-in
//! `literature/standard-corpus/` submodule, [`KovanRoot::standard_corpus_dir`],
//! then each `[[repos.standard]]`), **every open repository** (an open
//! corpus may be a checkout of the same repository, the maintainer's
//! layout), and the shared application-data clone
//! ([`crate::corpus_repos::standard_corpus_dir`]). `corpus_file` is a path
//! relative to a repository root, so a document is found in whichever
//! checkout holds it: when the standard corpus is split into topic
//! repositories, an entry lives in whichever one holds its file.
//!
//! # Where the user's notes go (decision, 2026-09-30)
//!
//! Annotations, digitisations and research notes need a paper, because every
//! paper-aware view works on a [`crate::session::PaperSession`]. So the first
//! time a standard-corpus document is **opened** in a Kovan folder,
//! [`ensure_paper`] files an ordinary paper for it, keyed by the corpus id:
//!
//! ```text
//! papers/<year>/<corpus-id>/kovan.toml     [source] corpus = "<corpus-id>", access = "open"
//! papers/<year>/<corpus-id>/<corpus-id>.md annotations, digitisations, notes
//! bibliography.bib                         @techreport{<corpus-id>, ...} from the corpus metadata
//! ```
//!
//! The PDF is never copied: `[source].pdf` points into the corpus checkout
//! when it is inside the folder, and `[source].corpus` finds it wherever it
//! is otherwise. Listing and searching need no files at all; only opening
//! writes, since only then is there something of the user's to keep. A paper
//! that already records the corpus PDF (an earlier ingest in place) is
//! reused as the notes instead of a second one being made.

use crate::corpus::{CorpusLiterature, LiteratureKind, SourceStatus, LITERATURE};
use crate::entity::{Access, CiteKey, EntityConfig, EntityError};
use crate::root::KovanRoot;
use kovan_literature::BibEntry;
use std::path::{Path, PathBuf};

/// Whether a corpus document's PDF is on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// In a checkout, at this path.
    Downloaded(PathBuf),
    /// Known from the compiled metadata; the PDF is not here (corpus not
    /// pulled, or a citation-only entry).
    NotDownloaded,
}

/// The corpus checkouts on this machine, searched in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StandardCorpus {
    checkouts: Vec<PathBuf>,
}

impl StandardCorpus {
    /// Every place Kovan's default pull puts a corpus repository, for `root`
    /// (or only the shared clone when no folder is open). See the module
    /// doc.
    pub fn for_root(root: Option<&KovanRoot>) -> Self {
        use crate::corpus_tiers::Tier;
        let mut dirs = Vec::new();
        if let Some(root) = root {
            let repos = root.corpus_repos();
            for tier in [Tier::Standard, Tier::Open] {
                dirs.extend(
                    repos
                        .iter()
                        .filter(|r| r.tier == tier)
                        .map(|r| r.dir.clone()),
                );
            }
            // A folder whose `[repos]` dropped the built-ins still finds a
            // corpus checked out at the conventional path.
            dirs.push(root.standard_corpus_dir());
        }
        dirs.extend(crate::corpus_repos::standard_corpus_dir());
        Self::with_checkouts(dirs)
    }

    /// A corpus searched in exactly `checkouts` (for tests, or a caller that
    /// knows where its repositories are). Duplicates are dropped.
    pub fn with_checkouts(checkouts: Vec<PathBuf>) -> Self {
        let mut out: Vec<PathBuf> = Vec::new();
        for dir in checkouts {
            let same = |d: &PathBuf| {
                d == &dir
                    || matches!((d.canonicalize(), dir.canonicalize()), (Ok(a), Ok(b)) if a == b)
            };
            if !out.iter().any(same) {
                out.push(dir);
            }
        }
        Self { checkouts: out }
    }

    /// The checkouts searched, in order.
    pub fn checkouts(&self) -> &[PathBuf] {
        &self.checkouts
    }

    /// Where `lit`'s PDF is, in the first checkout that holds it.
    pub fn locate(&self, lit: &CorpusLiterature) -> Option<PathBuf> {
        let file = lit.corpus_file?;
        self.checkouts
            .iter()
            .map(|d| d.join(file))
            .find(|p| p.is_file())
    }

    /// [`Self::locate`], as an [`Availability`].
    pub fn availability(&self, lit: &CorpusLiterature) -> Availability {
        match self.locate(lit) {
            Some(p) => Availability::Downloaded(p),
            None => Availability::NotDownloaded,
        }
    }

    /// Every corpus document whose PDF is here, with its path.
    pub fn downloaded(&self) -> Vec<(&'static CorpusLiterature, PathBuf)> {
        LITERATURE
            .iter()
            .filter_map(|l| self.locate(l).map(|p| (l, p)))
            .collect()
    }

    /// The corpus document whose PDF `path` is (the same file, however it
    /// is reached), if any.
    pub fn entry_for_path(&self, path: &Path) -> Option<&'static CorpusLiterature> {
        let target = path.canonicalize().ok()?;
        LITERATURE.iter().find(|l| {
            l.corpus_file.is_some_and(|file| {
                self.checkouts
                    .iter()
                    .any(|d| d.join(file).canonicalize().ok().as_ref() == Some(&target))
            })
        })
    }
}

/// The compiled entry with this id.
pub fn entry(id: &str) -> Option<&'static CorpusLiterature> {
    LITERATURE.iter().find(|l| l.id == id)
}

/// A short licence-status label for display.
pub fn status_label(status: SourceStatus) -> &'static str {
    match status {
        SourceStatus::VerifiedPublicDomain => "public domain (verified)",
        SourceStatus::VerifiedOpenLicence => "open licence (verified)",
        SourceStatus::PubliclyAccessibleUnverified => {
            "publicly accessible, redistribution unverified"
        }
        SourceStatus::Restricted => "restricted",
    }
}

/// A multi-line description of `lit` for a tooltip: title, authors, year,
/// topics, licence status, and where the PDF is or can be obtained.
pub fn describe(lit: &CorpusLiterature, availability: &Availability) -> String {
    let mut out = format!("{}\n{}", lit.title, lit.authors.join("; "));
    if let Some(y) = lit.year {
        out.push_str(&format!(" ({y})"));
    }
    out.push_str(&format!("\nstandard corpus: {}", lit.id));
    if !lit.topics.is_empty() {
        out.push_str(&format!("\ntopics: {}", lit.topics.join(", ")));
    }
    out.push_str(&format!("\nlicence: {}", status_label(lit.status)));
    match availability {
        Availability::Downloaded(p) => out.push_str(&format!("\n{}", p.display())),
        Availability::NotDownloaded => {
            out.push_str("\nnot downloaded");
            if let Some(url) = lit.source_url {
                out.push_str(&format!(": {url}"));
            }
        }
    }
    out
}

/// The BibTeX entry for `lit`, from its compiled metadata only. An author
/// without a comma is an organisation and is braced, so BibTeX does not
/// split it into given and family names.
pub fn bib_entry(lit: &CorpusLiterature) -> BibEntry {
    let entry_type = match lit.kind {
        LiteratureKind::Paper => "article",
        LiteratureKind::Report => "techreport",
        LiteratureKind::Book => "book",
        LiteratureKind::Thesis => "phdthesis",
        LiteratureKind::Webpage | LiteratureKind::Standard | LiteratureKind::Other => "misc",
    };
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("title".to_string(), lit.title.to_string());
    let authors = lit
        .authors
        .iter()
        .map(|a| {
            if a.contains(',') {
                a.to_string()
            } else {
                format!("{{{a}}}")
            }
        })
        .collect::<Vec<_>>()
        .join(" and ");
    if !authors.is_empty() {
        fields.insert("author".to_string(), authors);
    }
    if let Some(y) = lit.year {
        fields.insert("year".to_string(), y.to_string());
    }
    if let Some(url) = lit.source_url {
        fields.insert("url".to_string(), url.to_string());
    }
    fields.insert(
        "note".to_string(),
        format!("Kovan standard corpus: {}", lit.id),
    );
    BibEntry {
        entry_type: entry_type.to_string(),
        cite_key: lit.id.to_string(),
        fields,
    }
}

/// The paper already holding `lit`'s notes: one marked
/// `[source] corpus = "<id>"`, or one whose recorded PDF is `lit`'s corpus
/// file (ingested in place before this module existed).
pub fn paper_for(
    root: &KovanRoot,
    corpus: &StandardCorpus,
    lit: &CorpusLiterature,
) -> Option<String> {
    let located = corpus.locate(lit).and_then(|p| p.canonicalize().ok());
    root.paper_dirs().into_iter().find_map(|dir| {
        let config = EntityConfig::load(&dir).ok()?;
        let source = config.source.as_ref()?;
        if source.corpus.as_deref() == Some(lit.id) {
            return Some(config.id);
        }
        let pdf = dir.join(source.pdf.as_ref()?).canonicalize().ok()?;
        (Some(&pdf) == located.as_ref()).then_some(config.id)
    })
}

/// Why [`ensure_paper`] could not file a paper.
#[derive(Debug)]
pub enum StandardPaperError {
    /// A different paper already uses the corpus id as its citekey.
    CiteKeyTaken {
        citekey: String,
    },
    Entity(EntityError),
    Ingest(crate::ingest::IngestError),
}

impl std::fmt::Display for StandardPaperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CiteKeyTaken { citekey } => write!(
                f,
                "a paper named {citekey:?} already exists and is not the standard-corpus \
                 document {citekey}; rename it to open the corpus document's notes"
            ),
            Self::Entity(e) => write!(f, "{e}"),
            Self::Ingest(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StandardPaperError {}

/// The citekey of the paper holding `lit`'s notes in `root`, filing one
/// first if there is none (see the module doc for the layout). Never copies
/// the PDF, never touches the corpus checkout, and leaves an existing
/// bibliography entry with the same key as it is.
pub fn ensure_paper(
    root: &KovanRoot,
    corpus: &StandardCorpus,
    lit: &CorpusLiterature,
) -> Result<String, StandardPaperError> {
    if let Some(existing) = paper_for(root, corpus, lit) {
        return Ok(existing);
    }
    let citekey = CiteKey::parse(lit.id).map_err(StandardPaperError::Entity)?;
    if root
        .paper_dir(lit.id)
        .join(crate::entity::ENTITY_MARKER)
        .is_file()
    {
        return Err(StandardPaperError::CiteKeyTaken {
            citekey: lit.id.to_string(),
        });
    }
    let year = lit.year.map(|y| y.to_string());
    let paper_dir = root.new_paper_dir(lit.id, year.as_deref());
    let access = if lit.status.redistributable() {
        Access::Open
    } else {
        Access::Restricted
    };
    let topics: Vec<String> = lit.topics.iter().map(|t| t.to_string()).collect();
    crate::entity::ensure_classification_paths(root, &topics, &[])
        .map_err(StandardPaperError::Entity)?;
    let mut config = EntityConfig::paper(citekey, access).with_corpus(lit.id);
    if !topics.is_empty() {
        config = config.with_topics(topics);
    }
    // Record the PDF relative to the paper only when it is inside the
    // folder; a path into the shared application-data clone would not
    // survive a move to another machine, and `corpus` finds it anyway.
    let inside = match (corpus.locate(lit), root.path().canonicalize()) {
        (Some(pdf), Ok(root_dir)) => pdf
            .canonicalize()
            .ok()
            .filter(|c| c.starts_with(&root_dir))
            .map(|c| root.path().join(c.strip_prefix(&root_dir).unwrap_or(&c))),
        _ => None,
    };
    if let Some(pdf) = inside {
        config = config.with_pdf(crate::ingest::relative_to(&paper_dir, &pdf));
    }
    config
        .save_paper(&paper_dir)
        .map_err(StandardPaperError::Entity)?;
    match crate::ingest::append_bib_entry(root, bib_entry(lit)) {
        Ok(()) | Err(crate::ingest::IngestError::CiteKeyTaken { .. }) => {}
        Err(e) => return Err(StandardPaperError::Ingest(e)),
    }
    let _ = crate::index::KnowledgeIndex::rebuild(root).save_cache(root);
    Ok(lit.id.to_string())
}

/// The PDF of paper `citekey`: its recorded `[source].pdf` when that file
/// exists, else its standard-corpus document's file, if it is one and is
/// downloaded. `None` otherwise.
pub fn paper_pdf(root: &KovanRoot, corpus: &StandardCorpus, citekey: &str) -> Option<PathBuf> {
    let dir = root.paper_dir(citekey);
    let source = EntityConfig::load(&dir).ok()?.source?;
    if let Some(p) = source.pdf.as_ref().map(|rel| dir.join(rel)) {
        if p.is_file() {
            return Some(p);
        }
    }
    corpus.locate(entry(source.corpus.as_deref()?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootConfig;

    fn wash() -> &'static CorpusLiterature {
        entry("wash-1400").unwrap()
    }

    /// A folder whose `literature/standard-corpus/` holds a synthetic
    /// stand-in for WASH-1400's corpus file (never the real document), and
    /// the corpus searched only there.
    fn folder_with_wash(bytes: &[u8]) -> (tempfile::TempDir, KovanRoot, StandardCorpus) {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(tmp.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let pdf = root.standard_corpus_dir().join(wash().corpus_file.unwrap());
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        std::fs::write(&pdf, bytes).unwrap();
        let corpus = StandardCorpus::with_checkouts(vec![
            root.standard_corpus_dir(),
            root.open_corpus_dir(),
        ]);
        (tmp, root, corpus)
    }

    /// A pulled corpus file is available at its checkout path, found from
    /// any path to it; an absent one is known but not downloaded, as is a
    /// document found only in a second checkout until that one is searched.
    #[test]
    fn availability_follows_the_checkouts() {
        let (tmp, root, corpus) = folder_with_wash(b"%PDF-1.4 synthetic wash");
        let located = corpus.locate(wash()).unwrap();
        assert!(located.starts_with(root.standard_corpus_dir()));
        assert_eq!(
            corpus.entry_for_path(&located).map(|l| l.id),
            Some("wash-1400")
        );
        let other = entry("nureg-2201").unwrap();
        assert_eq!(corpus.availability(other), Availability::NotDownloaded);

        // The same repository mounted as the open corpus (the maintainer's
        // layout) is a second checkout.
        let second = tmp.path().join("elsewhere");
        let f = second.join(other.corpus_file.unwrap());
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, b"x").unwrap();
        let both = StandardCorpus::with_checkouts(vec![root.standard_corpus_dir(), second]);
        assert_eq!(both.availability(other), Availability::Downloaded(f));
        assert_eq!(both.downloaded().len(), 2);
        let text = describe(other, &corpus.availability(other));
        assert!(text.contains("not downloaded: https://"), "{text}");
        assert!(text.contains("licence: public domain"), "{text}");
    }

    /// Opening a corpus document files one paper keyed by its corpus id,
    /// with corpus metadata, no copied PDF, and is idempotent.
    #[test]
    fn ensure_paper_files_one_note_keyed_by_the_corpus_id() {
        let (_tmp, root, corpus) = folder_with_wash(b"%PDF-1.4 synthetic wash");
        let before = std::fs::read_dir(
            root.standard_corpus_dir()
                .join("kovan-standard-open-corpus/nrc"),
        )
        .unwrap()
        .count();
        let key = ensure_paper(&root, &corpus, wash()).unwrap();
        assert_eq!(key, "wash-1400");
        let dir = root.paper_dir("wash-1400");
        assert_eq!(dir, root.papers_dir().join("1975").join("wash-1400"));
        let config = EntityConfig::load(&dir).unwrap();
        let source = config.source.clone().unwrap();
        assert_eq!(source.corpus.as_deref(), Some("wash-1400"));
        assert_eq!(source.access, Access::Open);
        assert!(config
            .classification
            .topics
            .contains(&"nuclear-engineering/pra".to_string()));
        assert_eq!(
            paper_pdf(&root, &corpus, "wash-1400")
                .unwrap()
                .canonicalize()
                .unwrap(),
            corpus.locate(wash()).unwrap().canonicalize().unwrap()
        );
        let bib = std::fs::read_to_string(root.bibliography_path()).unwrap();
        assert!(bib.contains("@techreport{wash-1400,"), "{bib}");
        assert!(
            bib.contains("{U.S. Nuclear Regulatory Commission}"),
            "{bib}"
        );
        assert_eq!(ensure_paper(&root, &corpus, wash()).unwrap(), "wash-1400");
        assert_eq!(root.paper_dirs().len(), 1);
        let after = std::fs::read_dir(
            root.standard_corpus_dir()
                .join("kovan-standard-open-corpus/nrc"),
        )
        .unwrap()
        .count();
        assert_eq!(before, after, "the corpus checkout is never written");
    }

    /// A corpus document found only outside the folder (the shared clone)
    /// still opens: the paper records the corpus id, not an absolute path.
    #[test]
    fn a_corpus_pdf_outside_the_folder_is_found_by_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(
            &tmp.path().join("lib"),
            RootConfig::new("lib", "Lib"),
            false,
        )
        .unwrap();
        let clone = tmp.path().join("app-data-clone");
        let f = clone.join(wash().corpus_file.unwrap());
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, b"x").unwrap();
        let corpus = StandardCorpus::with_checkouts(vec![root.standard_corpus_dir(), clone]);
        ensure_paper(&root, &corpus, wash()).unwrap();
        let source = EntityConfig::load(&root.paper_dir("wash-1400"))
            .unwrap()
            .source
            .unwrap();
        assert_eq!(source.pdf, None);
        assert_eq!(paper_pdf(&root, &corpus, "wash-1400"), Some(f));
    }

    /// A paper that already records the corpus PDF (ingested in place
    /// earlier) is the document's notes; no second paper is filed.
    #[test]
    fn an_earlier_in_place_ingest_is_reused() {
        let (_tmp, root, corpus) = folder_with_wash(b"%PDF-1.4 synthetic wash");
        let dir = root.new_paper_dir("nrc1975reactor", Some("1975"));
        let pdf = corpus.locate(wash()).unwrap();
        EntityConfig::paper(CiteKey::parse("nrc1975reactor").unwrap(), Access::Open)
            .with_pdf(crate::ingest::relative_to(&dir, &pdf))
            .save_paper(&dir)
            .unwrap();
        assert_eq!(
            ensure_paper(&root, &corpus, wash()).unwrap(),
            "nrc1975reactor"
        );
        assert_eq!(root.paper_dirs().len(), 1);
    }

    /// A different paper squatting on the corpus id is refused, not merged.
    #[test]
    fn a_different_paper_with_the_corpus_id_is_refused() {
        let (_tmp, root, corpus) = folder_with_wash(b"%PDF-1.4 synthetic wash");
        let dir = root.new_paper_dir("wash-1400", Some("2001"));
        EntityConfig::paper(CiteKey::parse("wash-1400").unwrap(), Access::Restricted)
            .save_paper(&dir)
            .unwrap();
        assert!(matches!(
            ensure_paper(&root, &corpus, wash()),
            Err(StandardPaperError::CiteKeyTaken { .. })
        ));
    }
}
