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
//! # Duplicate detection
//!
//! §23 step 2 asks for "duplicate check". Two checks run, both at preview
//! time and again at [`ingest`] time (the second is what protects against a
//! race):
//!
//! - **The citekey** [`IngestPreview::already_exists`] would collide with.
//! - **The document itself** ([`find_existing`], since 2026-09-30): the
//!   incoming PDF is compared, by path and then by SHA-256 content hash
//!   ([`crate::fingerprint`], cached in `.kovan/`), with every downloaded
//!   standard-corpus file ([`crate::standard_corpus`]), every paper's PDF
//!   and, since GitHub issue #458, **every PDF in every corpus repository of
//!   every tier** ([`crate::corpus_tiers`]), so a document already in any
//!   standard, open or proprietary repository is caught even when no paper
//!   records it. A match is [`IngestPreview::duplicate`] and [`IngestError::Duplicate`]:
//!   nothing is written, and the caller opens the existing entry instead.
//!   Only files of the same byte length are hashed, so the check reads
//!   almost nothing when there is no duplicate.
//!
//!   ~~A full content-fingerprint check is real future work, not done
//!   here.~~ **CORRECTED 2026-09-30**: its absence let WASH-1400 be ingested
//!   a second time as `2008muffletwond`, byte-identical to its
//!   standard-corpus file, because a standard-corpus document was never
//!   "ingested" to begin with. Both halves are fixed; see
//!   [`crate::standard_corpus`].
//!
//! # Which repository (GitHub issue #458)
//!
//! A tier may hold several repositories. [`IngestChoice::target`] names the
//! tier and the repository; `None` means the tier's default
//! ([`crate::root::KovanRoot::default_repo`]) of the tier the access implies
//! (open for [`Access::Open`], proprietary otherwise). A restricted document
//! may only go to a proprietary repository, and a standard repository only
//! when it is configured `writable` ([`resolve_target`]). The repository is
//! recorded in the paper's `[source] repo`.
//!
//! A file whose **name** matches a corpus document's or a paper's PDF, but
//! whose content differs, is only a warning ([`IngestPreview::name_clash`]):
//! it may be a different revision, and a name is weak evidence.
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
use crate::fingerprint::HashCache;
use crate::index::KnowledgeIndex;
use crate::root::KovanRoot;
use crate::standard_corpus::StandardCorpus;

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
    /// The PDF is already in the library ([`find_existing`]); nothing was
    /// written. Open the existing entry instead.
    Duplicate(ExistingEntry),
    /// The chosen tier/repository cannot take this document
    /// ([`resolve_target`]); nothing was written.
    Target { reason: String },
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
            Self::Duplicate(existing) => write!(f, "not ingested: {existing}"),
            Self::Target { reason } => write!(f, "not ingested: {reason}"),
        }
    }
}

impl std::error::Error for IngestError {}

/// How an incoming PDF matched something already in the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchKind {
    /// It is the same file.
    SamePath,
    /// Byte-identical content, with this SHA-256.
    SameContent { sha256: String },
    /// Same file name, different content: a warning, not a duplicate.
    SameFileName,
}

/// An entry already in the library that an incoming PDF matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExistingEntry {
    /// A standard-corpus document ([`crate::corpus::LITERATURE`]). `pdf` is
    /// its corpus file, or `None` when it is not downloaded here.
    StandardCorpus {
        id: &'static str,
        title: &'static str,
        pdf: Option<PathBuf>,
        matched: MatchKind,
    },
    /// A paper in this folder, with the PDF it records.
    Paper {
        citekey: String,
        pdf: PathBuf,
        matched: MatchKind,
    },
    /// A PDF in one of the folder's corpus repositories that no paper
    /// records (GitHub issue #458).
    RepoFile {
        tier: crate::corpus_tiers::Tier,
        repo: String,
        pdf: PathBuf,
        matched: MatchKind,
    },
}

impl ExistingEntry {
    /// How it matched.
    pub fn matched(&self) -> &MatchKind {
        match self {
            Self::StandardCorpus { matched, .. }
            | Self::Paper { matched, .. }
            | Self::RepoFile { matched, .. } => matched,
        }
    }
}

impl std::fmt::Display for ExistingEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (what, pdf, matched) = match self {
            Self::StandardCorpus {
                id,
                title,
                pdf,
                matched,
            } => (
                format!("standard-corpus document {id} ({title:?})"),
                pdf.clone(),
                matched,
            ),
            Self::Paper {
                citekey,
                pdf,
                matched,
            } => (format!("paper {citekey}"), Some(pdf.clone()), matched),
            Self::RepoFile {
                tier,
                repo,
                pdf,
                matched,
            } => (
                format!("a file of the {} repository {repo:?}", tier.key()),
                Some(pdf.clone()),
                matched,
            ),
        };
        let at = pdf
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not downloaded here)".to_string());
        match matched {
            MatchKind::SamePath => write!(f, "this PDF is already in Kovan as {what}: {at}"),
            MatchKind::SameContent { sha256 } => write!(
                f,
                "this PDF is already in Kovan as {what}: identical content (SHA-256 {}...) to {at}",
                &sha256[..sha256.len().min(16)]
            ),
            MatchKind::SameFileName => write!(
                f,
                "this PDF has the same file name as {what} ({at}) but different content; \
                 check it is not the same document before ingesting"
            ),
        }
    }
}

/// Every paper's PDF that exists here: its recorded `[source].pdf`, or its
/// standard-corpus file ([`crate::standard_corpus::paper_pdf`]).
fn paper_pdfs(root: &KovanRoot, corpus: &StandardCorpus) -> Vec<(String, PathBuf)> {
    root.paper_dirs()
        .into_iter()
        .filter_map(|dir| {
            let id = EntityConfig::load(&dir).ok()?.id;
            let pdf = crate::standard_corpus::paper_pdf(root, corpus, &id)?;
            Some((id, pdf))
        })
        .collect()
}

/// The library entry `pdf` duplicates, if any: the same file, or a
/// byte-identical one, among the downloaded standard-corpus documents
/// (checked first) and the papers' PDFs. Hashes only files of `pdf`'s
/// length, through `cache`. See the module doc.
pub fn find_existing(
    root: &KovanRoot,
    corpus: &StandardCorpus,
    pdf: &Path,
    cache: &mut HashCache,
) -> Option<ExistingEntry> {
    let target = pdf.canonicalize().ok()?;
    if let Some(lit) = corpus.entry_for_path(&target) {
        return Some(ExistingEntry::StandardCorpus {
            id: lit.id,
            title: lit.title,
            pdf: Some(target),
            matched: MatchKind::SamePath,
        });
    }
    let papers = paper_pdfs(root, corpus);
    let same = |p: &Path| p.canonicalize().ok().as_ref() == Some(&target);
    if let Some((citekey, p)) = papers.iter().find(|(_, p)| same(p)) {
        return Some(ExistingEntry::Paper {
            citekey: citekey.clone(),
            pdf: p.clone(),
            matched: MatchKind::SamePath,
        });
    }
    let len = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    let size = len(&target)?;
    let standard: Vec<_> = corpus
        .downloaded()
        .into_iter()
        .filter(|(_, p)| len(p) == Some(size))
        .collect();
    let papers: Vec<_> = papers
        .into_iter()
        .filter(|(_, p)| len(p) == Some(size))
        .collect();
    let repo_files: Vec<_> = repo_pdfs(root)
        .into_iter()
        .filter(|(_, p)| len(p) == Some(size))
        .collect();
    // The incoming file itself being in a repository is not a duplicate:
    // that is an ingest in place (`corpus_resident`).
    let repo_files: Vec<_> = repo_files.into_iter().filter(|(_, p)| !same(p)).collect();
    if standard.is_empty() && papers.is_empty() && repo_files.is_empty() {
        return None;
    }
    let sha256 = cache.sha256(&target)?;
    let content = MatchKind::SameContent {
        sha256: sha256.clone(),
    };
    let mut found = None;
    for (lit, p) in standard {
        if cache.sha256(&p).as_ref() == Some(&sha256) {
            found = Some(ExistingEntry::StandardCorpus {
                id: lit.id,
                title: lit.title,
                pdf: Some(p),
                matched: content.clone(),
            });
            break;
        }
    }
    if found.is_none() {
        for (citekey, p) in papers {
            if cache.sha256(&p).as_ref() == Some(&sha256) {
                found = Some(ExistingEntry::Paper {
                    citekey,
                    pdf: p,
                    matched: content.clone(),
                });
                break;
            }
        }
    }
    if found.is_none() {
        for (repo, p) in repo_files {
            if cache.sha256(&p).as_ref() == Some(&sha256) {
                found = Some(ExistingEntry::RepoFile {
                    tier: repo.tier,
                    repo: repo.name.clone(),
                    pdf: p,
                    matched: content.clone(),
                });
                break;
            }
        }
    }
    cache.save();
    found
}

/// Every PDF in every corpus repository of `root` (all tiers, in order),
/// with the repository it is in; Git's own folder skipped. A repository
/// mounted in two tiers is walked once.
fn repo_pdfs(root: &KovanRoot) -> Vec<(crate::corpus_tiers::CorpusRepo, PathBuf)> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out = Vec::new();
    for repo in root.corpus_repos() {
        let key = repo.dir.canonicalize().unwrap_or_else(|_| repo.dir.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let files = crate::corpus_tiers::pdfs_in(&repo.dir);
        out.extend(files.into_iter().map(|f| (repo.clone(), f)));
    }
    out
}

/// The repositories an ingest form may offer for a document with `access`
/// (#458), in tier order: for a restricted document only the proprietary
/// repositories; for an open one only the open tiers (open, and `writable`
/// standard). ~~For an open one every writable repository, including
/// proprietary, "where keeping an open document private is always safe".~~
/// **CHANGED 2026-10-06 (maintainer):** open and proprietary literature are
/// kept strictly apart, so the dropdown for open literature never lists a
/// proprietary repository, and vice versa. Each is accepted by
/// [`resolve_target`].
pub fn target_choices(root: &KovanRoot, access: Access) -> Vec<crate::corpus_tiers::CorpusRepo> {
    use crate::corpus_tiers::Tier;
    let all = root.corpus_repos();
    let order: &[Tier] = if access.is_committable() {
        &[Tier::Open, Tier::Standard]
    } else {
        &[Tier::Proprietary]
    };
    order
        .iter()
        .flat_map(|tier| {
            all.iter()
                .filter(move |r| r.tier == *tier && r.writable)
                .cloned()
        })
        .collect()
}

/// The target an ingest form starts on: the default repository of the tier
/// `access` implies ([`resolve_target`] with no target), as a
/// [`crate::corpus_tiers::RepoRef`]. `None` when there is none.
pub fn default_target(root: &KovanRoot, access: Access) -> Option<crate::corpus_tiers::RepoRef> {
    resolve_target(root, access, None)
        .ok()
        .map(|r| crate::corpus_tiers::RepoRef {
            tier: r.tier,
            name: r.name,
        })
}

/// The corpus repository an ingest of a document with `access` goes to:
/// `target` when given, else the default repository of the tier `access`
/// implies (open for a committable access, proprietary otherwise).
///
/// Refused ([`IngestError::Target`]) when `target` names no repository of
/// that tier, when a restricted document would go anywhere but a
/// proprietary repository (it would be published), or when a standard
/// repository is not configured `writable` (the corpus maintainer maintains
/// those).
pub fn resolve_target(
    root: &KovanRoot,
    access: Access,
    target: Option<&crate::corpus_tiers::RepoRef>,
) -> Result<crate::corpus_tiers::CorpusRepo, IngestError> {
    use crate::corpus_tiers::Tier;
    let tier = match target {
        Some(t) => t.tier,
        None if access.is_committable() => Tier::Open,
        None => Tier::Proprietary,
    };
    if access.is_committable() && tier == Tier::Proprietary {
        return Err(IngestError::Target {
            reason: "an open document goes in an open repository; proprietary repositories hold \
                     restricted literature only (open and proprietary are kept apart)"
                .to_string(),
        });
    }
    if !access.is_committable() && tier != Tier::Proprietary {
        return Err(IngestError::Target {
            reason: format!(
                "a restricted document may only be stored in a proprietary repository, not the \
                 {} tier, which is published",
                tier.key()
            ),
        });
    }
    let repo = match target {
        Some(t) => root
            .tier_repos(tier)
            .into_iter()
            .find(|r| r.name == t.name)
            .ok_or_else(|| IngestError::Target {
                reason: format!("there is no {} repository named {:?}", tier.key(), t.name),
            })?,
        None => root.default_repo(tier).ok_or_else(|| IngestError::Target {
            reason: format!("this folder has no {} repository", tier.key()),
        })?,
    };
    if !repo.writable {
        return Err(IngestError::Target {
            reason: format!(
                "the {} repository {:?} is read-only (standard repositories are maintained by \
                 the corpus maintainer; set `writable = true` on its [[repos.standard]] entry to \
                 file documents there)",
                tier.key(),
                repo.name
            ),
        });
    }
    Ok(repo)
}

/// A standard-corpus document (downloaded or not) or paper whose PDF has
/// `pdf`'s file name, ignoring case: a possible duplicate to warn about.
/// Call after [`find_existing`] found nothing.
pub fn same_file_name(
    root: &KovanRoot,
    corpus: &StandardCorpus,
    pdf: &Path,
) -> Option<ExistingEntry> {
    let name = pdf.file_name()?.to_string_lossy().to_lowercase();
    let named = |p: &Path| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().to_lowercase() == name)
    };
    if let Some(lit) = crate::corpus::LITERATURE
        .iter()
        .find(|l| l.corpus_file.is_some_and(|f| named(Path::new(f))))
    {
        return Some(ExistingEntry::StandardCorpus {
            id: lit.id,
            title: lit.title,
            pdf: corpus.locate(lit),
            matched: MatchKind::SameFileName,
        });
    }
    paper_pdfs(root, corpus)
        .into_iter()
        .find(|(_, p)| named(p))
        .map(|(citekey, pdf)| ExistingEntry::Paper {
            citekey,
            pdf,
            matched: MatchKind::SameFileName,
        })
}

/// The content-hash cache for `root` ([`crate::fingerprint`]).
fn hash_cache(root: &KovanRoot) -> HashCache {
    HashCache::load(&root.state_dir())
}

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
    /// The library entry this PDF already is ([`find_existing`]). When set,
    /// [`ingest`] refuses; open this entry instead.
    pub duplicate: Option<ExistingEntry>,
    /// A corpus document or paper with the same file name but different
    /// content ([`same_file_name`]): shown as a warning, does not block.
    pub name_clash: Option<ExistingEntry>,
}

/// Run the automatic-detection half of §22 over `pdf_path`. Writes nothing
/// but the disposable hash cache. The standard corpus is searched wherever
/// Kovan pulls it ([`StandardCorpus::for_root`]).
pub fn preview(root: &KovanRoot, pdf_path: &Path) -> Result<IngestPreview, IngestError> {
    preview_with(root, &StandardCorpus::for_root(Some(root)), pdf_path)
}

/// [`preview`], against the standard-corpus checkouts in `corpus`.
pub fn preview_with(
    root: &KovanRoot,
    corpus: &StandardCorpus,
    pdf_path: &Path,
) -> Result<IngestPreview, IngestError> {
    let duplicate = find_existing(root, corpus, pdf_path, &mut hash_cache(root));
    let name_clash = match duplicate {
        Some(_) => None,
        None => same_file_name(root, corpus, pdf_path),
    };
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
        duplicate,
        name_clash,
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
    /// Which tier and repository to store the PDF in (GitHub issue #458);
    /// `None` is the default repository of the tier `access` implies. See
    /// [`resolve_target`].
    pub target: Option<crate::corpus_tiers::RepoRef>,
}

/// Run §23's write transaction: store the PDF, create/update the
/// bibliography, create the paper directory and its `kovan.toml` +
/// canonical Markdown stub, and refresh the derived index cache.
///
/// §23 step 10 ("open Research workspace") is deliberately not this
/// function's job — it is GUI navigation, not a filesystem write, and the
/// Research workspace itself is `op-9vo6.25`'s later step. A caller opens
/// it itself once this returns `Ok`.
///
/// Refuses with [`IngestError::Duplicate`], writing nothing, when the PDF is
/// already in the library ([`find_existing`], re-checked here).
pub fn ingest(
    root: &KovanRoot,
    preview: &IngestPreview,
    choice: IngestChoice,
) -> Result<(), IngestError> {
    ingest_with(root, &StandardCorpus::for_root(Some(root)), preview, choice)
}

/// [`ingest`], against the standard-corpus checkouts in `corpus`.
pub fn ingest_with(
    root: &KovanRoot,
    corpus: &StandardCorpus,
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

    if let Some(existing) = find_existing(root, corpus, &preview.source_pdf, &mut hash_cache(root))
    {
        return Err(IngestError::Duplicate(existing));
    }

    // §23 step 3: store the PDF in the chosen repository (#458). In an
    // open (or writable standard) one, into the user's own folder there,
    // never the standard corpus's (#255).
    let target = resolve_target(root, choice.access, choice.target.as_ref())?;
    let store_dir = match target.tier {
        crate::corpus_tiers::Tier::Proprietary => target.dir.clone(),
        _ => crate::corpus_repos::open_corpus_ingest_dir(&target.dir),
    };
    // A PDF already in one of the folder's corpora is used where it is: a
    // copy would duplicate a corpus document, and copying a proprietary one
    // into the open corpus would publish it.
    let (dest_pdf, repo_name) = match corpus_resident(root, &preview.source_pdf) {
        Some((in_place, repo)) => (in_place, repo),
        None => {
            std::fs::create_dir_all(&store_dir).map_err(|source| IngestError::Io {
                path: store_dir.clone(),
                source,
            })?;
            let dest_pdf = store_dir.join(format!("{}.pdf", citekey.as_str()));
            std::fs::copy(&preview.source_pdf, &dest_pdf).map_err(|source| IngestError::Io {
                path: dest_pdf.clone(),
                source,
            })?;
            (dest_pdf, Some(target.name.clone()))
        }
    };

    // §23 step 4: create/update the bibliography.
    let mut entry = preview.bib_entry.clone();
    entry.cite_key = citekey.as_str().to_string();
    append_bib_entry(root, entry)?;

    // §23 steps 5-7: paper directory, kovan.toml, canonical Markdown stub,
    // filed by year (maintainer direction, 2026-09-22).
    let paper_dir = root.new_paper_dir(
        citekey.as_str(),
        preview.bib_entry.fields.get("year").map(String::as_str),
    );
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
    let mut config = config.with_pdf(relative_to(&paper_dir, &dest_pdf));
    if let Some(repo) = repo_name {
        config = config.with_repo(repo);
    }
    config.save_paper(&paper_dir).map_err(IngestError::Entity)?;

    // §23 step 9: update the derived index/graph. Best-effort — a failure
    // here does not undo the ingestion; the next `load_or_rebuild` self-heals.
    let _ = KnowledgeIndex::rebuild(root).save_cache(root);

    Ok(())
}

/// `pdf`, as a path inside `root`, when it is already in one of the
/// folder's corpus repositories (any repository of any tier, #458), with
/// that repository's name (`None` for the conventional standard-corpus
/// mount when `[repos]` dropped it); `None` otherwise.
fn corpus_resident(root: &KovanRoot, pdf: &Path) -> Option<(PathBuf, Option<String>)> {
    let pdf = pdf.canonicalize().ok()?;
    let root_dir = root.path().canonicalize().ok()?;
    let inside = |d: &Path| d.canonicalize().is_ok_and(|d| pdf.starts_with(d));
    let repo = match root.corpus_repos().into_iter().find(|r| inside(&r.dir)) {
        Some(r) => Some(r.name),
        None if inside(&root.standard_corpus_dir()) => None,
        None => return None,
    };
    Some((
        root.path()
            .join(pdf.strip_prefix(&root_dir).unwrap_or(&pdf)),
        repo,
    ))
}

/// Append `entry` to `root`'s bibliography, creating the file if absent,
/// atomically (temp file + rename). Rejects a citekey already present, same
/// as the entity-directory check in [`ingest`] — this one is what actually
/// guards against a concurrent-write race, since it re-reads the file
/// immediately before writing rather than trusting an earlier check.
pub(crate) fn append_bib_entry(root: &KovanRoot, entry: BibEntry) -> Result<(), IngestError> {
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
pub(crate) fn relative_to(base: &Path, target: &Path) -> PathBuf {
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
        let target = PathBuf::from("/lib/literature/open-corpus/wang2018multiphysics.pdf");
        let rel = relative_to(&base, &target);
        assert_eq!(
            rel,
            PathBuf::from("../../literature/open-corpus/wang2018multiphysics.pdf")
        );
    }

    /// A PDF already in a corpus is ingested where it is: no copy, and the
    /// paper records the corpus path.
    #[test]
    fn a_corpus_pdf_is_ingested_in_place() {
        let (_dir, root) = make_root();
        let folder = root.open_corpus_dir().join("me-open-corpus");
        std::fs::create_dir_all(&folder).unwrap();
        let pdf = folder.join("original-name.pdf");
        write_test_pdf(&pdf, "In Place Study");
        let p = preview(&root, &pdf).unwrap();
        let citekey = p.suggested_citekey.clone();
        let choice = IngestChoice {
            citekey: citekey.clone(),
            access: Access::Open,
            topics: vec![],
            projects: vec![],
            target: None,
        };
        ingest(&root, &p, choice).unwrap();
        let recorded = EntityConfig::load(&root.paper_dir(&citekey))
            .unwrap()
            .source
            .unwrap()
            .pdf
            .unwrap();
        assert_eq!(
            recorded,
            PathBuf::from("../../../literature/open-corpus/me-open-corpus/original-name.pdf")
        );
        // The test PDF carries no year, so the paper is filed as undated.
        assert!(root
            .papers_dir()
            .join("undated")
            .join(&citekey)
            .join("kovan.toml")
            .is_file());
        let pdfs: Vec<_> = walk_pdfs(root.path());
        assert_eq!(pdfs.len(), 1, "no copy expected: {pdfs:?}");
    }

    /// Papers are filed by the year of their bibliography entry
    /// (maintainer direction, 2026-09-22), and found again wherever filed.
    #[test]
    fn a_paper_is_filed_under_its_year() {
        let (dir, root) = make_root();
        let pdf = dir.path().join("in.pdf");
        write_test_pdf(&pdf, "A Dated Study");
        let mut p = preview(&root, &pdf).unwrap();
        p.bib_entry.fields.insert("year".into(), "2021".into());
        let citekey = p.suggested_citekey.clone();
        let choice = IngestChoice {
            citekey: citekey.clone(),
            access: Access::Open,
            topics: vec![],
            projects: vec![],
            target: None,
        };
        ingest(&root, &p, choice).unwrap();
        let filed = root.papers_dir().join("2021").join(&citekey);
        assert!(filed.join("kovan.toml").is_file());
        assert_eq!(root.paper_dir(&citekey), filed);
        assert_eq!(root.paper_dirs(), vec![filed.clone()]);
        assert!(root.paper_markdown(&citekey).starts_with(&filed));
        assert!(KnowledgeIndex::rebuild(&root).has_paper(&citekey));
    }

    fn walk_pdfs(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk_pdfs(&p));
            } else if p.extension().is_some_and(|x| x == "pdf") {
                out.push(p);
            }
        }
        out
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
            target: None,
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
            target: None,
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
            target: None,
        };
        ingest(&root, &p, choice).unwrap();

        let config = EntityConfig::load(&root.paper_dir(&p.suggested_citekey)).unwrap();
        assert_eq!(config.classification, Classification::unsorted());
    }

    // -------------------------------------------------------------------
    // The duplicate guard (2026-09-30): a standard-corpus document, or a
    // paper's PDF, is not ingested a second time. All PDFs are synthetic.
    // -------------------------------------------------------------------

    /// The WASH-1400 corpus entry, whose `corpus_file` the synthetic
    /// stand-in is written to.
    fn wash() -> &'static crate::corpus::CorpusLiterature {
        crate::standard_corpus::entry("wash-1400").unwrap()
    }

    /// A folder whose standard-corpus checkout holds a synthetic PDF at
    /// WASH-1400's corpus path, and the corpus searched only in the folder
    /// (never the real application-data clone).
    fn root_with_corpus_file() -> (tempfile::TempDir, KovanRoot, StandardCorpus, PathBuf) {
        let (dir, root) = make_root();
        let pdf = root.standard_corpus_dir().join(wash().corpus_file.unwrap());
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        write_test_pdf(&pdf, "Synthetic Stand-in For A Corpus Report");
        let corpus = StandardCorpus::with_checkouts(vec![
            root.standard_corpus_dir(),
            root.open_corpus_dir(),
        ]);
        (dir, root, corpus, pdf)
    }

    fn open_choice(p: &IngestPreview) -> IngestChoice {
        IngestChoice {
            citekey: p.suggested_citekey.clone(),
            access: Access::Open,
            topics: vec![],
            projects: vec![],
            target: None,
        }
    }

    /// The bug: a byte-identical copy of a standard-corpus file, from
    /// anywhere, is recognised by content and refused, writing nothing.
    #[test]
    fn a_copy_of_a_standard_corpus_file_is_refused_by_content() {
        let (dir, root, corpus, corpus_pdf) = root_with_corpus_file();
        let copy = dir.path().join("downloads").join("muffletwond.pdf");
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        std::fs::copy(&corpus_pdf, &copy).unwrap();

        let p = preview_with(&root, &corpus, &copy).unwrap();
        let dup = p
            .duplicate
            .clone()
            .expect("recognised as the corpus document");
        let expected = crate::fingerprint::sha256_file(&corpus_pdf).unwrap();
        match &dup {
            ExistingEntry::StandardCorpus { id, matched, .. } => {
                assert_eq!(*id, "wash-1400");
                assert_eq!(matched, &MatchKind::SameContent { sha256: expected });
            }
            other => panic!("expected the corpus entry, got {other:?}"),
        }
        assert!(dup.to_string().contains("wash-1400"), "{dup}");

        let err = ingest_with(&root, &corpus, &p, open_choice(&p)).unwrap_err();
        assert!(matches!(err, IngestError::Duplicate(_)), "{err}");
        assert!(root.paper_dirs().is_empty(), "no paper written");
        assert!(!root.bibliography_path().is_file(), "no bib entry written");
        assert!(
            root.state_dir()
                .join(crate::fingerprint::CACHE_FILE)
                .is_file(),
            "hashes are cached"
        );
    }

    /// The corpus file itself (same path) is the corpus document.
    #[test]
    fn the_standard_corpus_file_itself_is_refused_by_path() {
        let (_dir, root, corpus, corpus_pdf) = root_with_corpus_file();
        let p = preview_with(&root, &corpus, &corpus_pdf).unwrap();
        assert!(matches!(
            p.duplicate,
            Some(ExistingEntry::StandardCorpus {
                matched: MatchKind::SamePath,
                ..
            })
        ));
        assert!(ingest_with(&root, &corpus, &p, open_choice(&p)).is_err());
    }

    /// A copy of an already-ingested paper's PDF points at that paper.
    #[test]
    fn a_copy_of_a_papers_pdf_is_refused_and_names_the_paper() {
        let (dir, root, corpus, _) = root_with_corpus_file();
        let first = dir.path().join("first.pdf");
        write_test_pdf(&first, "An Ordinary Paper");
        let p = preview_with(&root, &corpus, &first).unwrap();
        assert!(p.duplicate.is_none(), "{:?}", p.duplicate);
        ingest_with(&root, &corpus, &p, open_choice(&p)).unwrap();

        let copy = dir.path().join("renamed-copy.pdf");
        std::fs::copy(&first, &copy).unwrap();
        let again = preview_with(&root, &corpus, &copy).unwrap();
        match again.duplicate {
            Some(ExistingEntry::Paper {
                citekey,
                matched: MatchKind::SameContent { .. },
                ..
            }) => assert_eq!(citekey, p.suggested_citekey),
            other => panic!("expected the paper, got {other:?}"),
        }
    }

    /// Ordinary new PDFs still ingest when a corpus is present, including
    /// one that shares a corpus file's name but not its content (a warning
    /// only).
    #[test]
    fn new_pdfs_still_ingest_and_a_name_match_only_warns() {
        let (dir, root, corpus, _) = root_with_corpus_file();
        let fresh = dir.path().join("fresh.pdf");
        write_test_pdf(&fresh, "A Genuinely New Study");
        let p = preview_with(&root, &corpus, &fresh).unwrap();
        assert!(p.duplicate.is_none() && p.name_clash.is_none());
        ingest_with(&root, &corpus, &p, open_choice(&p)).unwrap();

        let same_name = dir.path().join("dl").join("ML15334A199.pdf");
        std::fs::create_dir_all(same_name.parent().unwrap()).unwrap();
        write_test_pdf(&same_name, "Different Bytes Same Name");
        let q = preview_with(&root, &corpus, &same_name).unwrap();
        assert!(q.duplicate.is_none());
        assert!(matches!(
            q.name_clash,
            Some(ExistingEntry::StandardCorpus {
                id: "wash-1400",
                matched: MatchKind::SameFileName,
                ..
            })
        ));
        ingest_with(&root, &corpus, &q, open_choice(&q)).unwrap();
        assert_eq!(root.paper_dirs().len(), 2);
    }
}
