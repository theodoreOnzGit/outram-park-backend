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
//! Implements the pipeline described in `docs/kovan.md` sections
//! "Literature Workflow", "Canonical Representation" and "PDF Processing".
//! The [`KovanDocument`] struct is authoritative; BibTeX and generated Markdown
//! are always derived from it, never the other way round.
//!
//! ## Determinism & offline guarantees
//!
//! Every function here is **deterministic** (same input bytes → same output
//! bytes) and runs **fully offline** — no network, no cloud, no OCR service.
//! PDF text extraction uses the pure-Rust [`pdf_extract`] crate; the low-level
//! object model (metadata, assets) uses pure-Rust [`lopdf`]. Both build for
//! Android (`aarch64-linux-android`), matching KOVAN's Android-first mandate
//! (`docs/kovan.md`, "Android First").
//!
//! ## Storage layout
//!
//! Content lives on disk next to this crate (`docs/kovan.md`, "Storage Layout"):
//!
//! - `open/{papers,reports,standards,benchmarks,theses}/` — redistributable
//!   content, may be committed.
//! - `proprietary/{…}/` — user-owned content; **gitignored**, never committed.
//! - `generated/{markdown,bibtex,assets}/{open,proprietary}/` — reproducible
//!   outputs, split by [`Visibility`] so the proprietary half can be kept out of
//!   both git and the published crate. See [`storage::generated_dir_for`].
//!
//! Three distribution tiers follow from that split: generated **open BibTeX** is
//! committed *and* published to crates.io; open PDFs and generated open Markdown
//! are committed but **not** published (licence scope and size); everything
//! proprietary is neither.
//!
//! ## What is real vs. best-effort
//!
//! - [`pdf_to_markdown`], [`markdown_outline`], [`to_bibtex`] — fully
//!   implemented and tested.
//! - [`extract_metadata`] — best-effort heuristics (PDF Info dictionary first,
//!   then conservative text scanning). Unknown fields are left `None`/empty
//!   rather than guessed.
//! - [`extract_assets`] — extracts embedded raster images whose codec is already
//!   a standalone file format (JPEG via `DCTDecode`, JPEG-2000 via `JPXDecode`).
//!   Images stored under other filters are reported-skipped, not re-encoded.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

pub use kovan_common::{Author, DocumentType, KovanBenchmark, KovanDocument, Visibility};

mod bibtex;
mod markdown;
mod metadata;
mod pdf_import;

#[cfg(test)]
mod test_pdf;

pub use bibtex::to_bibtex;
pub use markdown::{
    markdown_outline, split_markdown_by_page_limit, text_to_markdown, Heading, PAGE_SEPARATOR,
};
pub use metadata::extract_metadata;
pub use pdf_import::{extract_assets, extract_pdf_text, pdf_to_markdown};

/// Errors produced by the literature pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteratureError {
    /// The requested operation is not implemented yet (placeholder stage).
    Unimplemented(&'static str),
    /// A source file could not be read, parsed, or was malformed.
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
/// documents should be split with [`split_markdown_by_page_limit`]. See
/// `docs/kovan.md`, "PDF Processing" (`≤ 30 pages` per Markdown document).
pub const MAX_MARKDOWN_PAGES: u32 = 30;

/// Roots of the on-disk storage tree, relative to the crate directory.
///
/// Implements `docs/kovan.md`, "Storage Layout".
pub mod storage {
    use super::{Path, PathBuf, Visibility};

    /// Directory for redistributable content that may be committed.
    pub const OPEN_ROOT: &str = "open";
    /// Directory for user-owned content that must never be committed.
    pub const PROPRIETARY_ROOT: &str = "proprietary";
    /// Directory for reproducible generated artifacts.
    pub const GENERATED_ROOT: &str = "generated";

    /// Sub-directory of [`GENERATED_ROOT`] holding generated BibTeX entries.
    pub const BIBTEX_DIR: &str = "bibtex";
    /// Sub-directory of [`GENERATED_ROOT`] holding generated Markdown bodies.
    pub const MARKDOWN_DIR: &str = "markdown";
    /// Sub-directory of [`GENERATED_ROOT`] holding extracted image assets.
    pub const ASSETS_DIR: &str = "assets";

    /// Directory a generated artifact of `kind` and `visibility` belongs in,
    /// joined onto `base` (usually the `kovan-literature` crate directory).
    ///
    /// The generated tree is split by [`Visibility`] one level below each artifact
    /// kind — `generated/bibtex/open/`, `generated/bibtex/proprietary/`, and so on
    /// — because the two halves have different distribution rules:
    ///
    /// - **open** — committed to the repository, and for BibTeX also *published*
    ///   in the packaged crate (citation entries are small bibliographic facts).
    /// - **proprietary** — never committed and never published; an artifact
    ///   derived from user-owned content is equally user-owned.
    ///
    /// Both rules are enforced outside this function (the root `.gitignore` and
    /// the `exclude` list in this crate's `Cargo.toml`); this is the single place
    /// that decides *which* directory a writer should target, so the two
    /// mechanisms and the code cannot drift apart.
    ///
    /// `kind` should be one of [`BIBTEX_DIR`], [`MARKDOWN_DIR`], [`ASSETS_DIR`].
    pub fn generated_dir_for(base: &Path, kind: &str, visibility: Visibility) -> PathBuf {
        let leaf = match visibility {
            Visibility::Open => OPEN_ROOT,
            Visibility::Proprietary => PROPRIETARY_ROOT,
        };
        base.join(GENERATED_ROOT).join(kind).join(leaf)
    }

    /// Return the storage root for a given [`Visibility`], joined onto `base`
    /// (usually the `kovan-literature` crate directory).
    pub fn root_for(base: &Path, visibility: Visibility) -> PathBuf {
        match visibility {
            Visibility::Open => base.join(OPEN_ROOT),
            Visibility::Proprietary => base.join(PROPRIETARY_ROOT),
        }
    }

    /// Infer a document's [`Visibility`] from where its source file lives.
    ///
    /// Any path that contains a `proprietary/` component is treated as
    /// [`Visibility::Proprietary`]; everything else defaults to
    /// [`Visibility::Open`]. This is the deterministic rule the ingestion
    /// pipeline uses so proprietary material never gets an "open" label by
    /// accident (`docs/kovan.md`, "Open vs Proprietary").
    pub fn visibility_from_path(path: &Path) -> Visibility {
        let proprietary = path
            .components()
            .any(|c| c.as_os_str().eq_ignore_ascii_case(PROPRIETARY_ROOT));
        if proprietary {
            Visibility::Proprietary
        } else {
            Visibility::Open
        }
    }

    /// Infer a [`super::DocumentType`] from a storage sub-directory name in the
    /// source path (`papers/`, `reports/`, `standards/`, `benchmarks/`,
    /// `manuals/`, `theses/` or `dissertations/`), falling back to
    /// [`super::DocumentType::Other`] when none is present.
    pub fn document_type_from_path(path: &Path) -> super::DocumentType {
        use super::DocumentType::*;
        for c in path.components() {
            match c
                .as_os_str()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str()
            {
                "papers" => return Paper,
                "reports" => return Report,
                "standards" => return Standard,
                "benchmarks" => return Benchmark,
                "manuals" => return Manual,
                "theses" | "dissertations" => return Thesis,
                _ => {}
            }
        }
        Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn proprietary_and_open_roots_differ() {
        let base = Path::new("/tmp/lit");
        assert_ne!(
            storage::root_for(base, Visibility::Open),
            storage::root_for(base, Visibility::Proprietary),
        );
    }

    #[test]
    fn visibility_inferred_from_path() {
        assert_eq!(
            storage::visibility_from_path(Path::new("open/papers/x.pdf")),
            Visibility::Open
        );
        assert_eq!(
            storage::visibility_from_path(Path::new("proprietary/reports/x.pdf")),
            Visibility::Proprietary
        );
    }

    #[test]
    fn generated_dirs_are_split_by_visibility() {
        let base = Path::new("/tmp/lit");
        let open = storage::generated_dir_for(base, storage::BIBTEX_DIR, Visibility::Open);
        let prop = storage::generated_dir_for(base, storage::BIBTEX_DIR, Visibility::Proprietary);
        assert!(open.ends_with("generated/bibtex/open"), "got {open:?}");
        assert!(
            prop.ends_with("generated/bibtex/proprietary"),
            "got {prop:?}"
        );
        assert_ne!(open, prop);
    }

    #[test]
    fn theses_directory_infers_the_thesis_type() {
        assert_eq!(
            storage::document_type_from_path(Path::new("open/theses/wang2018.pdf")),
            DocumentType::Thesis
        );
    }

    #[test]
    fn document_type_inferred_from_path() {
        assert_eq!(
            storage::document_type_from_path(Path::new("open/reports/x.pdf")),
            DocumentType::Report
        );
        assert_eq!(
            storage::document_type_from_path(Path::new("x.pdf")),
            DocumentType::Other
        );
    }
}
