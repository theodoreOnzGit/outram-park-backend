//! # kovan-literature
//!
//! The nuclear-engineering knowledge archive. It turns source PDFs into the
//! canonical [`KovanDocument`] and generates derived artifacts (Markdown,
//! BibTeX, extracted assets).
//!
//! ## Canonical workflow
//!
//! ```text
//! PDF → Markdown → KovanDocument → BibTeX → generated knowledge artifacts
//! ```
//!
//! The [`KovanDocument`] struct is authoritative; BibTeX and generated Markdown
//! are always derived from it, never the other way round.
//!
//! ## Storage layout
//!
//! Content lives on disk next to this crate:
//!
//! - `open/{papers,reports,standards,benchmarks}/` — redistributable content,
//!   may be committed.
//! - `proprietary/{…}/` — user-owned content; **gitignored**, never committed.
//! - `generated/{markdown,bibtex,assets}/` — reproducible outputs.
//!
//! Placeholder stage: the pipeline functions below are stubs marked
//! `// TODO(kovan)` and return [`LiteratureError::Unimplemented`] (or a minimal
//! placeholder) so the crate compiles and its tests pass without doing real work.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

pub use kovan_common::{
    Author, DocumentType, KovanBenchmark, KovanDocument, Visibility,
};

/// Errors produced by the literature pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteratureError {
    /// The requested operation is not implemented yet (placeholder stage).
    Unimplemented(&'static str),
    /// A source file could not be read or was malformed.
    Io(String),
}

impl std::fmt::Display for LiteratureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LiteratureError::Unimplemented(what) => {
                write!(f, "not implemented yet: {what}")
            }
            LiteratureError::Io(msg) => write!(f, "io error: {msg}"),
        }
    }
}

impl std::error::Error for LiteratureError {}

/// Target maximum number of pages per generated Markdown document. Larger
/// documents should be split. See the project instructions.
pub const MAX_MARKDOWN_PAGES: u32 = 30;

/// Roots of the on-disk storage tree, relative to the crate directory.
pub mod storage {
    use super::{Path, PathBuf, Visibility};

    /// Directory for redistributable content that may be committed.
    pub const OPEN_ROOT: &str = "open";
    /// Directory for user-owned content that must never be committed.
    pub const PROPRIETARY_ROOT: &str = "proprietary";
    /// Directory for reproducible generated artifacts.
    pub const GENERATED_ROOT: &str = "generated";

    /// Return the storage root for a given [`Visibility`], joined onto `base`
    /// (usually the `kovan-literature` crate directory).
    pub fn root_for(base: &Path, visibility: Visibility) -> PathBuf {
        match visibility {
            Visibility::Open => base.join(OPEN_ROOT),
            Visibility::Proprietary => base.join(PROPRIETARY_ROOT),
        }
    }
}

/// Import a PDF and produce Markdown body text.
///
/// TODO(kovan): implement PDF import + Markdown generation, splitting output so
/// each document stays within [`MAX_MARKDOWN_PAGES`].
pub fn pdf_to_markdown(_pdf: &Path) -> Result<String, LiteratureError> {
    Err(LiteratureError::Unimplemented("pdf_to_markdown"))
}

/// Extract document metadata (title, authors, DOI, …) from a source PDF into a
/// [`KovanDocument`] skeleton.
///
/// TODO(kovan): implement metadata extraction.
pub fn extract_metadata(_pdf: &Path) -> Result<KovanDocument, LiteratureError> {
    Err(LiteratureError::Unimplemented("extract_metadata"))
}

/// Extract embedded assets (figures, tables) from a source PDF into
/// `generated/assets/`, returning their written paths.
///
/// TODO(kovan): implement asset extraction.
pub fn extract_assets(_pdf: &Path) -> Result<Vec<PathBuf>, LiteratureError> {
    Err(LiteratureError::Unimplemented("extract_assets"))
}

/// One heading in a Markdown document: its level (1–6) and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Heading level, 1 (`#`) through 6 (`######`).
    pub level: u8,
    /// Plain-text heading content.
    pub text: String,
}

/// Parse a Markdown body and return its heading outline, using `pulldown-cmark`
/// (the one Markdown parser KOVAN starts with). This is a real, deterministic
/// helper — useful for splitting long documents and building a table of
/// contents.
pub fn markdown_outline(markdown: &str) -> Vec<Heading> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let mut outline = Vec::new();
    let mut current: Option<u8> = None;
    let mut buf = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(heading_level_to_u8(level));
                buf.clear();
            }
            Event::Text(t) | Event::Code(t) if current.is_some() => buf.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = current.take() {
                    outline.push(Heading {
                        level,
                        text: buf.trim().to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    // silence unused import warnings if the enum shape changes upstream
    let _ = HeadingLevel::H1;
    outline
}

fn heading_level_to_u8(level: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        H1 => 1,
        H2 => 2,
        H3 => 3,
        H4 => 4,
        H5 => 5,
        H6 => 6,
    }
}

/// Render a [`KovanDocument`] to a BibTeX entry.
///
/// This is a minimal placeholder that emits a syntactically-valid entry from
/// the fields that are always present. TODO(kovan): full field coverage,
/// proper entry-type selection, and key generation.
pub fn to_bibtex(doc: &KovanDocument) -> String {
    let entry_type = match doc.document_type {
        DocumentType::Paper => "article",
        DocumentType::Report | DocumentType::Manual => "techreport",
        DocumentType::Standard => "misc",
        DocumentType::Benchmark => "misc",
        DocumentType::Other => "misc",
    };
    let authors = doc
        .authors
        .iter()
        .map(|a| format!("{}, {}", a.family, a.given))
        .collect::<Vec<_>>()
        .join(" and ");
    let mut out = format!("@{entry_type}{{{key},\n", key = doc.slug);
    out.push_str(&format!("  title = {{{}}},\n", doc.title));
    if !authors.is_empty() {
        out.push_str(&format!("  author = {{{authors}}},\n"));
    }
    if let Some(year) = doc.year {
        out.push_str(&format!("  year = {{{year}}},\n"));
    }
    if let Some(doi) = &doc.doi {
        out.push_str(&format!("  doi = {{{doi}}},\n"));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn pipeline_stubs_report_unimplemented() {
        assert!(matches!(
            pdf_to_markdown(Path::new("x.pdf")),
            Err(LiteratureError::Unimplemented(_))
        ));
    }

    #[test]
    fn proprietary_and_open_roots_differ() {
        let base = Path::new("/tmp/lit");
        assert_ne!(
            storage::root_for(base, Visibility::Open),
            storage::root_for(base, Visibility::Proprietary),
        );
    }

    #[test]
    fn bibtex_placeholder_emits_key_and_title() {
        let doc = KovanDocument::new(
            "doc-1",
            "smith2020",
            Visibility::Open,
            DocumentType::Paper,
            "A Title",
        );
        let bib = to_bibtex(&doc);
        assert!(bib.starts_with("@article{smith2020,"));
        assert!(bib.contains("title = {A Title}"));
    }

    #[test]
    fn markdown_outline_extracts_headings() {
        let md = "# Title\n\nsome text\n\n## Section A\n\n### Sub\n";
        let outline = markdown_outline(md);
        assert_eq!(outline.len(), 3);
        assert_eq!(outline[0], Heading { level: 1, text: "Title".into() });
        assert_eq!(outline[1].level, 2);
        assert_eq!(outline[2], Heading { level: 3, text: "Sub".into() });
    }
}
