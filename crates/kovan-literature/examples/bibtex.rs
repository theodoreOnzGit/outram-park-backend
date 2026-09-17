//! Render a `KovanDocument` to a BibTeX entry.
//!
//! Shows the canonical direction of the KOVAN pipeline: the Rust
//! `KovanDocument` is authoritative and BibTeX is *generated* from it
//! (`docs/kovan.md`, "Canonical Representation"). Deterministic and offline.
//!
//! Run with: `cargo run -p kovan-literature --example bibtex`

use kovan_literature::{to_bibtex, Author, DocumentType, KovanDocument, Visibility};

fn main() {
    let mut doc = KovanDocument::new(
        "kovan-doc-42",
        "zweibaum2015ciet",
        Visibility::Open,
        DocumentType::Report,
        "Compact Integral Effects Test (CIET) facility characterisation",
    );
    doc.authors.push(Author {
        family: "Zweibaum".into(),
        given: "Nicolas".into(),
        affiliation: None,
    });
    doc.year = Some(2015);
    doc.institution = Some("University of California, Berkeley".into());
    doc.keywords = vec!["FHR".into(), "thermal hydraulics".into()];

    print!("{}", to_bibtex(&doc));
}
