//! Build a canonical `KovanDocument` and print it.
//!
//! Run with: `cargo run -p kovan-common --example build_document`

use kovan_common::{Author, DocumentType, KovanDocument, Visibility};

fn main() {
    let mut doc = KovanDocument::new(
        "icsbep-heu-met-fast-001",
        "godiva-heu-met-fast-001",
        Visibility::Open,
        DocumentType::Benchmark,
        "HEU-MET-FAST-001 (Godiva) bare critical sphere",
    );
    doc.authors.push(Author {
        family: "ICSBEP".into(),
        given: "".into(),
        affiliation: Some("OECD/NEA".into()),
    });
    doc.year = Some(2016);
    doc.keywords.push("criticality".into());

    println!("{doc:#?}");
}
