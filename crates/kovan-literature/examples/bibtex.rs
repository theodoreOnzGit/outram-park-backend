//! Render a `KovanDocument` to a placeholder BibTeX entry.
//!
//! Run with: `cargo run -p kovan-literature --example bibtex`

use kovan_literature::{to_bibtex, Author, DocumentType, KovanDocument, Visibility};

fn main() {
    let mut doc = KovanDocument::new(
        "doc-42",
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

    print!("{}", to_bibtex(&doc));
}
