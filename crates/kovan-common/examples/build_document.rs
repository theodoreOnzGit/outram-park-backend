//! Build a canonical `KovanDocument` with the builder and print it.
//!
//! Run with: `cargo run -p kovan-common --example build_document`

use kovan_common::{Author, DocumentType, KovanDocument, Visibility};

fn main() {
    // The struct is large, so build it with `KovanDocument::builder(...)` and
    // chain only the fields you have. Every field has an empty/`None` default,
    // so `.build()` is infallible.
    let doc = KovanDocument::builder(
        "icsbep-heu-met-fast-001",
        "godiva-heu-met-fast-001",
        Visibility::Open,
        DocumentType::Benchmark,
        "HEU-MET-FAST-001 (Godiva) bare critical sphere",
    )
    // Organisational author: name in `family`, empty `given`.
    .author(Author {
        family: "ICSBEP".into(),
        given: "".into(),
        affiliation: Some("OECD/NEA".into()),
    })
    .year(2016)
    .keywords(vec!["criticality".into()])
    .source_path("open/benchmarks/heu-met-fast-001.pdf")
    .page_count(120)
    .build();

    println!("{doc:#?}");
}
