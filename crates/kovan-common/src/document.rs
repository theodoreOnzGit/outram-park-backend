//! The canonical literature document type and its ergonomic builder.
//!
//! [`KovanDocument`] is the single source of truth for one piece of literature
//! (KOVAN's "Canonical Representation" rule — BibTeX/TOML/Markdown are generated
//! *from* it, never authoritative). Because the struct is large, construct it
//! with [`KovanDocumentBuilder`] (via [`KovanDocument::builder`]) rather than by
//! field-by-field mutation.

/// Whether a piece of content may be redistributed (committed) or must stay
/// local to the user's machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Visibility {
    /// Redistributable content — NRC reports, arXiv papers, open-access
    /// journals, public theses. May be committed to version control.
    Open,
    /// User-owned content — textbooks, paywalled or proprietary reports.
    /// Must remain local and must never be committed.
    Proprietary,
}

/// The kind of literature a [`KovanDocument`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DocumentType {
    /// Journal or conference paper, preprint.
    Paper,
    /// Technical report (e.g. an NRC/NUREG report).
    Report,
    /// A standard or code (e.g. ASME, IEEE, ISO).
    Standard,
    /// A benchmark specification (e.g. an ICSBEP evaluation).
    Benchmark,
    /// A user manual or software manual.
    Manual,
    /// A doctoral or master's thesis / dissertation (e.g. a UC Berkeley
    /// eScholarship deposit). Distinct from [`DocumentType::Report`] because the
    /// citation form differs: a thesis cites its awarding institution, not an
    /// issuing organisation and report number.
    Thesis,
    /// Anything else; refine into a dedicated variant when a real need appears.
    Other,
}

/// A single author of a document.
///
/// Also used for organisational "authors" (e.g. a standards body or a
/// benchmark evaluation group) — by convention, set `family` to the
/// organisation's name and leave `given` as an empty string (see
/// `examples/build_document.rs`, which does this for an ICSBEP evaluation).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Author {
    /// Family name (surname), or the full name for an organisational author.
    pub family: String,
    /// Given name(s). Empty for an organisational author.
    pub given: String,
    /// Affiliation/institution, free text for now. `None` when unknown or
    /// not applicable (e.g. for an organisational author, where the
    /// organisation is already named in `family`).
    pub affiliation: Option<String>,
}

/// The canonical KOVAN document — the single source of truth for one piece of
/// literature. Everything else (BibTeX, generated Markdown, indices) is derived
/// from this struct.
///
/// The struct is intentionally wide (identity, classification, bibliographic
/// metadata, journal locators, source provenance, generated assets, and cross
/// links). Prefer [`KovanDocument::builder`] over field-by-field mutation for
/// readable construction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanDocument {
    /// Stable unique identifier (e.g. a content hash or an assigned key).
    pub id: String,
    /// Human-friendly URL/file-safe slug.
    pub slug: String,

    /// Open vs proprietary; governs whether it may be committed.
    pub visibility: Visibility,
    /// The kind of document.
    pub document_type: DocumentType,

    /// Full title.
    pub title: String,
    /// Ordered list of authors. Empty if unknown or not yet catalogued.
    #[serde(default)]
    pub authors: Vec<Author>,

    /// Abstract text (plain text). Empty string if unknown or not yet
    /// extracted from the source (not `Option` — an absent abstract is
    /// indistinguishable from an unextracted one at this stage, and callers
    /// should treat both the same way).
    pub abstract_text: String,

    /// Publication year, if known.
    pub year: Option<u32>,
    /// Digital Object Identifier, if any.
    pub doi: Option<String>,

    /// Journal name, if a journal paper. `None` for report/standard/manual
    /// document types, where it does not apply.
    pub journal: Option<String>,
    /// Institution, if a report/thesis. `None` for paper/standard document
    /// types, where it does not apply.
    pub institution: Option<String>,
    /// Publisher, if applicable. `None` if unknown or not applicable.
    pub publisher: Option<String>,

    /// Journal volume for a journal article (a `String` because volumes are
    /// not always numeric, e.g. `"12A"`). `None` when unknown or not a journal
    /// article.
    pub volume: Option<String>,
    /// Page range or article number within the volume (e.g. `"110439"` or
    /// `"245-260"`). `String` because it may be a hyphenated range or an
    /// electronic article id. `None` when unknown.
    pub pages: Option<String>,
    /// Journal issue number (e.g. `"3"`). `String` for the same reason as
    /// [`KovanDocument::volume`]. `None` when unknown or not applicable.
    pub number: Option<String>,

    /// Free-form keywords (author- or extraction-supplied). Empty if none
    /// were recorded.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// KOVAN-internal tags (curated by the local user/tooling, distinct from
    /// `keywords`). Empty if none have been applied.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Where the source PDF/record came from, if recorded (e.g. a download
    /// URL). `None` for locally authored or unattributed documents.
    pub source_url: Option<String>,

    /// Path to the source file this document was ingested from (the on-disk
    /// PDF, relative or absolute per the caller). `None` before ingestion or
    /// for locally authored documents.
    pub source_path: Option<String>,
    /// Lowercase-hex SHA-256 of the source file's bytes, for provenance /
    /// change detection. `None` when the hash has not been computed (the
    /// `kovan-literature` pipeline currently leaves this `None` — see its
    /// `DECISIONS.md`).
    pub source_sha256: Option<String>,

    /// Number of pages in the source document, if known (e.g. counted from the
    /// PDF). `None` before ingestion or when the count is unavailable.
    pub page_count: Option<u32>,
    /// Paths (relative to the generated-assets root) of files extracted from
    /// the source, such as figures. Empty if none were extracted or the
    /// asset pass has not run.
    #[serde(default)]
    pub assets: Vec<String>,

    /// IDs of related [`crate::KovanSymbol`]s (e.g. a code symbol implementing
    /// a correlation this document derives). Empty if none are linked yet.
    #[serde(default)]
    pub related_symbols: Vec<String>,
    /// IDs of related [`crate::KovanRepository`]s. Empty if none are linked yet.
    #[serde(default)]
    pub related_repositories: Vec<String>,
    /// IDs of related [`crate::KovanBenchmark`]s. Empty if none are linked yet.
    #[serde(default)]
    pub related_benchmarks: Vec<String>,

    /// The document body as Markdown (generated from the source PDF). Empty
    /// string before the PDF-import pipeline has run (see `kovan-literature`).
    pub markdown_body: String,
}

impl KovanDocument {
    /// Create an otherwise-empty document with the required identity and
    /// classification fields set. All optional/collection fields start empty.
    ///
    /// For anything beyond the bare identity, prefer [`KovanDocument::builder`],
    /// which reads far more clearly than a sequence of field assignments.
    pub fn new(
        id: impl Into<String>,
        slug: impl Into<String>,
        visibility: Visibility,
        document_type: DocumentType,
        title: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            slug: slug.into(),
            visibility,
            document_type,
            title: title.into(),
            authors: Vec::new(),
            abstract_text: String::new(),
            year: None,
            doi: None,
            journal: None,
            institution: None,
            publisher: None,
            volume: None,
            pages: None,
            number: None,
            keywords: Vec::new(),
            tags: Vec::new(),
            source_url: None,
            source_path: None,
            source_sha256: None,
            page_count: None,
            assets: Vec::new(),
            related_symbols: Vec::new(),
            related_repositories: Vec::new(),
            related_benchmarks: Vec::new(),
            markdown_body: String::new(),
        }
    }

    /// Start building a document from its required identity fields. Chain the
    /// setter methods on the returned [`KovanDocumentBuilder`] and finish with
    /// [`KovanDocumentBuilder::build`].
    ///
    /// # Example
    /// ```
    /// use kovan_common::{DocumentType, KovanDocument, Visibility};
    /// let doc = KovanDocument::builder(
    ///     "kovan-1", "doe2021", Visibility::Open, DocumentType::Paper, "A Title",
    /// )
    /// .year(2021)
    /// .journal("Nuclear Engineering and Design")
    /// .volume("410")
    /// .pages("112345")
    /// .page_count(12)
    /// .build();
    /// assert_eq!(doc.year, Some(2021));
    /// assert_eq!(doc.volume.as_deref(), Some("410"));
    /// ```
    pub fn builder(
        id: impl Into<String>,
        slug: impl Into<String>,
        visibility: Visibility,
        document_type: DocumentType,
        title: impl Into<String>,
    ) -> KovanDocumentBuilder {
        KovanDocumentBuilder {
            doc: KovanDocument::new(id, slug, visibility, document_type, title),
        }
    }
}

/// Ergonomic builder for [`KovanDocument`].
///
/// Owns the document it is assembling by value (no lifetimes, no trait objects,
/// per the workspace rules); each setter takes and returns `self`. Every field
/// has a sensible empty default, so [`KovanDocumentBuilder::build`] is
/// infallible. Obtain one from [`KovanDocument::builder`].
#[derive(Debug, Clone)]
pub struct KovanDocumentBuilder {
    doc: KovanDocument,
}

impl KovanDocumentBuilder {
    /// Append a single author (call repeatedly to add several, in order).
    pub fn author(mut self, author: Author) -> Self {
        self.doc.authors.push(author);
        self
    }

    /// Replace the entire author list.
    pub fn authors(mut self, authors: Vec<Author>) -> Self {
        self.doc.authors = authors;
        self
    }

    /// Set the plain-text abstract.
    pub fn abstract_text(mut self, text: impl Into<String>) -> Self {
        self.doc.abstract_text = text.into();
        self
    }

    /// Set the publication year.
    pub fn year(mut self, year: u32) -> Self {
        self.doc.year = Some(year);
        self
    }

    /// Set the DOI.
    pub fn doi(mut self, doi: impl Into<String>) -> Self {
        self.doc.doi = Some(doi.into());
        self
    }

    /// Set the journal name.
    pub fn journal(mut self, journal: impl Into<String>) -> Self {
        self.doc.journal = Some(journal.into());
        self
    }

    /// Set the institution.
    pub fn institution(mut self, institution: impl Into<String>) -> Self {
        self.doc.institution = Some(institution.into());
        self
    }

    /// Set the publisher.
    pub fn publisher(mut self, publisher: impl Into<String>) -> Self {
        self.doc.publisher = Some(publisher.into());
        self
    }

    /// Set the journal volume.
    pub fn volume(mut self, volume: impl Into<String>) -> Self {
        self.doc.volume = Some(volume.into());
        self
    }

    /// Set the page range / article number.
    pub fn pages(mut self, pages: impl Into<String>) -> Self {
        self.doc.pages = Some(pages.into());
        self
    }

    /// Set the journal issue number.
    pub fn number(mut self, number: impl Into<String>) -> Self {
        self.doc.number = Some(number.into());
        self
    }

    /// Replace the keyword list.
    pub fn keywords(mut self, keywords: Vec<String>) -> Self {
        self.doc.keywords = keywords;
        self
    }

    /// Replace the tag list.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.doc.tags = tags;
        self
    }

    /// Set the source URL the document was fetched from.
    pub fn source_url(mut self, url: impl Into<String>) -> Self {
        self.doc.source_url = Some(url.into());
        self
    }

    /// Set the on-disk path of the source file this document was ingested from.
    pub fn source_path(mut self, path: impl Into<String>) -> Self {
        self.doc.source_path = Some(path.into());
        self
    }

    /// Set the lowercase-hex SHA-256 of the source file's bytes.
    pub fn source_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.doc.source_sha256 = Some(sha256.into());
        self
    }

    /// Set the source page count.
    pub fn page_count(mut self, pages: u32) -> Self {
        self.doc.page_count = Some(pages);
        self
    }

    /// Replace the generated-asset path list.
    pub fn assets(mut self, assets: Vec<String>) -> Self {
        self.doc.assets = assets;
        self
    }

    /// Replace the related-symbol ID list.
    pub fn related_symbols(mut self, ids: Vec<String>) -> Self {
        self.doc.related_symbols = ids;
        self
    }

    /// Replace the related-repository ID list.
    pub fn related_repositories(mut self, ids: Vec<String>) -> Self {
        self.doc.related_repositories = ids;
        self
    }

    /// Replace the related-benchmark ID list.
    pub fn related_benchmarks(mut self, ids: Vec<String>) -> Self {
        self.doc.related_benchmarks = ids;
        self
    }

    /// Set the generated Markdown body.
    pub fn markdown_body(mut self, body: impl Into<String>) -> Self {
        self.doc.markdown_body = body.into();
        self
    }

    /// Finish building and return the assembled [`KovanDocument`]. Infallible —
    /// every field has an empty/`None` default from [`KovanDocument::new`].
    pub fn build(self) -> KovanDocument {
        self.doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_constructor_sets_identity_and_empties_the_rest() {
        let doc = KovanDocument::new(
            "doc-1",
            "smith-2020-decay-heat",
            Visibility::Open,
            DocumentType::Paper,
            "Decay heat in pebble-bed reactors",
        );
        assert_eq!(doc.id, "doc-1");
        assert_eq!(doc.visibility, Visibility::Open);
        assert_eq!(doc.document_type, DocumentType::Paper);
        assert!(doc.authors.is_empty());
        assert!(doc.markdown_body.is_empty());
        assert!(doc.assets.is_empty());
        assert_eq!(doc.page_count, None);
        assert_eq!(doc.volume, None);
        assert_eq!(doc.source_path, None);
    }

    #[test]
    fn document_json_round_trips() {
        let doc = KovanDocument::new(
            "doc-1",
            "smith2020",
            Visibility::Open,
            DocumentType::Paper,
            "A Title",
        );
        let json = serde_json::to_string(&doc).expect("serialise");
        let back: KovanDocument = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(doc, back);
    }

    /// A fully-populated document (every `Option` set, every `Vec`
    /// non-empty) — exercises fields the "empty" fixtures don't touch,
    /// including the v2 journal-locator, source-pointer, and asset fields.
    fn sample_document() -> KovanDocument {
        KovanDocument::builder(
            "icsbep-heu-met-fast-001",
            "godiva-heu-met-fast-001",
            Visibility::Open,
            DocumentType::Benchmark,
            "HEU-MET-FAST-001 (Godiva) bare critical sphere",
        )
        .author(Author {
            family: "ICSBEP".into(),
            given: "".into(),
            affiliation: Some("OECD/NEA".into()),
        })
        .abstract_text("Bare, highly-enriched uranium metal critical sphere.")
        .year(2016)
        .doi("10.1234/icsbep")
        .institution("OECD Nuclear Energy Agency")
        .publisher("OECD/NEA")
        .journal("International Handbook of Evaluated Criticality Safety Benchmarks")
        .volume("I")
        .pages("1-120")
        .number("HEU-MET-FAST-001")
        .keywords(vec!["criticality".into()])
        .tags(vec!["bare-sphere".into()])
        .source_url("https://www.oecd-nea.org/")
        .source_path("open/benchmarks/heu-met-fast-001.pdf")
        .source_sha256("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        .page_count(120)
        .assets(vec!["heu-met-fast-001-img000.jpg".into()])
        .related_symbols(vec!["sym-godiva-model".into()])
        .related_repositories(vec!["repo-outram-mc-libs".into()])
        .related_benchmarks(vec!["bench-godiva".into()])
        .markdown_body("# Godiva\n\nBare critical sphere benchmark.")
        .build()
    }

    #[test]
    fn builder_sets_every_field() {
        let doc = sample_document();
        assert_eq!(doc.authors.len(), 1);
        assert_eq!(doc.year, Some(2016));
        assert_eq!(doc.volume.as_deref(), Some("I"));
        assert_eq!(doc.pages.as_deref(), Some("1-120"));
        assert_eq!(doc.number.as_deref(), Some("HEU-MET-FAST-001"));
        assert_eq!(doc.page_count, Some(120));
        assert_eq!(doc.assets, vec!["heu-met-fast-001-img000.jpg".to_string()]);
        assert_eq!(
            doc.source_path.as_deref(),
            Some("open/benchmarks/heu-met-fast-001.pdf")
        );
        assert!(doc.source_sha256.is_some());
    }

    #[test]
    fn fully_populated_document_json_round_trips() {
        let doc = sample_document();
        let json = serde_json::to_string_pretty(&doc).expect("serialise");
        let back: KovanDocument = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(doc, back);
    }

    #[test]
    fn fully_populated_document_toml_round_trips() {
        let doc = sample_document();
        let toml_str = toml::to_string(&doc).expect("toml serialise");
        let back: KovanDocument = toml::from_str(&toml_str).expect("toml deserialise");
        assert_eq!(doc, back);
    }

    /// Documents already on disk may predate a field added to
    /// [`KovanDocument`] later (schema evolution). The `Vec` fields carry
    /// `#[serde(default)]` precisely so such a document still deserialises
    /// (as an empty collection) instead of hard-erroring; `Option` fields get
    /// the same treatment for free from serde's built-in "missing optional
    /// field is `None`" rule. This test locks that contract in — the JSON
    /// below deliberately omits *every* field added in the v2 pass (journal
    /// locators, source pointer, page count, assets) as well as the original
    /// collections, and must still deserialise with those defaulted.
    #[test]
    fn document_json_missing_newer_fields_still_deserialises() {
        let json = r#"{
            "id": "doc-1",
            "slug": "slug",
            "visibility": "Open",
            "document_type": "Paper",
            "title": "A Title",
            "abstract_text": "",
            "year": null,
            "doi": null,
            "journal": null,
            "institution": null,
            "publisher": null,
            "source_url": null,
            "markdown_body": ""
        }"#;
        let doc: KovanDocument = serde_json::from_str(json).expect("deserialise");
        // Original collections default empty.
        assert!(doc.authors.is_empty());
        assert!(doc.keywords.is_empty());
        assert!(doc.tags.is_empty());
        assert!(doc.related_symbols.is_empty());
        assert!(doc.related_repositories.is_empty());
        assert!(doc.related_benchmarks.is_empty());
        // v2 additions default too.
        assert!(doc.assets.is_empty());
        assert_eq!(doc.page_count, None);
        assert_eq!(doc.volume, None);
        assert_eq!(doc.pages, None);
        assert_eq!(doc.number, None);
        assert_eq!(doc.source_path, None);
        assert_eq!(doc.source_sha256, None);
    }

    #[test]
    fn visibility_json_round_trips() {
        for v in [Visibility::Open, Visibility::Proprietary] {
            let json = serde_json::to_string(&v).expect("serialise");
            let back: Visibility = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(v, back);
        }
    }

    #[test]
    fn document_type_json_round_trips() {
        for dt in [
            DocumentType::Paper,
            DocumentType::Report,
            DocumentType::Standard,
            DocumentType::Benchmark,
            DocumentType::Manual,
            DocumentType::Thesis,
            DocumentType::Other,
        ] {
            let json = serde_json::to_string(&dt).expect("serialise");
            let back: DocumentType = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(dt, back);
        }
    }

    #[test]
    fn author_json_and_toml_round_trip() {
        let author = Author {
            family: "Smith".into(),
            given: "Jane".into(),
            affiliation: Some("NUS".into()),
        };
        let json = serde_json::to_string(&author).expect("serialise");
        assert_eq!(author, serde_json::from_str(&json).expect("deserialise"));

        // Author is not itself a valid TOML *document* root when every field
        // could be absent, but with all fields present it round-trips like
        // any other struct.
        let toml_str = toml::to_string(&author).expect("toml serialise");
        assert_eq!(author, toml::from_str(&toml_str).expect("toml deserialise"));
    }

    #[test]
    fn author_with_no_affiliation_round_trips() {
        // Organisational-author convention: empty `given`, `affiliation` left
        // `None` because the organisation is already named in `family`.
        let author = Author {
            family: "ICSBEP".into(),
            given: "".into(),
            affiliation: None,
        };
        let json = serde_json::to_string(&author).expect("serialise");
        assert_eq!(author, serde_json::from_str(&json).expect("deserialise"));
    }
}
