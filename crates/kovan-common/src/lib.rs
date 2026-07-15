//! # kovan-common
//!
//! Shared canonical types for the KOVAN knowledge layer. Every other KOVAN
//! crate depends on this one and speaks in these types; cross-crate links
//! (a symbol referencing a document, a benchmark referencing a validation
//! case) are expressed as the string IDs defined here rather than as direct
//! crate-to-crate dependencies.
//!
//! **Source-of-truth rule:** these Rust structs are authoritative. BibTeX,
//! TOML, and Markdown metadata are *generated* from them and must never be
//! treated as the canonical record.
//!
//! ## What belongs here
//!
//! Types that more than one KOVAN crate needs: documents, symbols,
//! repositories, correlations, benchmarks, validation cases, and the small
//! enums/records they contain. Do **not** put pipeline logic (PDF parsing,
//! semantic extraction, code generation) here — that lives in the respective
//! feature crate.
//!
//! Placeholder stage: types carry their intended fields with doc comments, but
//! most helper logic is a `// TODO(kovan)` stub.

#![forbid(unsafe_code)]

/// Whether a piece of content may be redistributed (committed) or must stay
/// local to the user's machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Visibility {
    /// Redistributable content — NRC reports, arXiv papers, open-access
    /// journals, public theses. May be committed to version control.
    Open,
    /// User-owned content — textbooks, paywalled or proprietary reports.
    /// Must remain local and must never be committed.
    Proprietary,
}

/// The kind of literature a [`KovanDocument`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Anything else; refine into a dedicated variant when a real need appears.
    Other,
}

/// A single author of a document.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Author {
    /// Family name (surname).
    pub family: String,
    /// Given name(s).
    pub given: String,
    /// Optional affiliation/institution, free text for now.
    pub affiliation: Option<String>,
}

/// The canonical KOVAN document — the single source of truth for one piece of
/// literature. Everything else (BibTeX, generated Markdown, indices) is derived
/// from this struct.
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
    /// Ordered list of authors.
    pub authors: Vec<Author>,

    /// Abstract text (plain text).
    pub abstract_text: String,

    /// Publication year, if known.
    pub year: Option<u32>,
    /// Digital Object Identifier, if any.
    pub doi: Option<String>,

    /// Journal name, if a journal paper.
    pub journal: Option<String>,
    /// Institution, if a report/thesis.
    pub institution: Option<String>,
    /// Publisher, if applicable.
    pub publisher: Option<String>,

    /// Free-form keywords.
    pub keywords: Vec<String>,
    /// KOVAN-internal tags.
    pub tags: Vec<String>,

    /// Where the source PDF/record came from, if recorded.
    pub source_url: Option<String>,

    /// IDs of related [`KovanSymbol`]s.
    pub related_symbols: Vec<String>,
    /// IDs of related [`KovanRepository`]s.
    pub related_repositories: Vec<String>,
    /// IDs of related [`KovanBenchmark`]s.
    pub related_benchmarks: Vec<String>,

    /// The document body as Markdown (generated from the source PDF).
    pub markdown_body: String,
}

impl KovanDocument {
    /// Create an otherwise-empty document with the required identity and
    /// classification fields set. All optional/collection fields start empty.
    ///
    /// This is a convenience placeholder; richer builders belong in
    /// `kovan-literature` once the ingestion pipeline exists.
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
            keywords: Vec::new(),
            tags: Vec::new(),
            source_url: None,
            related_symbols: Vec::new(),
            related_repositories: Vec::new(),
            related_benchmarks: Vec::new(),
            markdown_body: String::new(),
        }
    }
}

/// A semantic symbol extracted from a source repository (a function, type,
/// module, …). Normalised across languages by `kovan-semantics`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanSymbol {
    /// Stable identifier for the symbol.
    pub id: String,
    /// Fully-qualified name/path as reported by the language tooling.
    pub qualified_name: String,
    /// Symbol kind, free text for now (e.g. "fn", "struct", "class").
    pub kind: String,
    /// ID of the repository this symbol belongs to.
    pub repository_id: String,
}

/// A source-code repository KOVAN understands (e.g. TUAS, OpenFOAM, NJOY).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanRepository {
    /// Stable identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Primary language, free text for now (e.g. "Rust", "C++", "Fortran").
    pub language: String,
}

/// An engineering correlation (e.g. a Nusselt-number correlation) linking a
/// literature source to an implementation and validation evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanCorrelation {
    /// Stable identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// ID of the [`KovanDocument`] this correlation is sourced from.
    pub source_document_id: Option<String>,
}

/// A benchmark specification (e.g. an ICSBEP critical-experiment evaluation).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanBenchmark {
    /// Stable identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// ID of the [`KovanDocument`] describing the benchmark, if any.
    pub source_document_id: Option<String>,
}

/// A validation case tying an implementation to a benchmark/correlation and its
/// measured result — the provenance record KOVAN ultimately aims to produce.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanValidationCase {
    /// Stable identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// ID of the benchmark this case is validated against, if any.
    pub benchmark_id: Option<String>,
    /// ID of the repository/symbol implementing it, if any.
    pub implementation_symbol_id: Option<String>,
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
}
