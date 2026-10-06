//! `KovanDocument` files written before the Zotero port still load, and
//! re-serialise byte for byte (maintainer rule 2026-10-07: kovan's existing
//! schemas must not break; the port may only add optional fields).
//!
//! **Methodology.** The fixtures in `tests/data/kovan_document_b142a065/`
//! were written by the `KovanDocument` of commit b142a065d2 (the parent of
//! the port): its `document.rs` was copied verbatim into a scratch crate
//! with the same serde, serde_json and toml versions, and a fully-populated
//! and a minimal document were serialised to JSON (`to_string_pretty`) and
//! TOML (`toml::to_string`). This test reads each with today's code,
//! compares it with the same document built today, and re-serialises it.
//!
//! **Pass:** each fixture deserialises to the expected document with
//! `zotero_item == None`, and today's serialisation is byte-identical to the
//! fixture text.
//!
//! **Result (2026-10-07):** pass, all four fixtures.

use kovan_common::{Author, DocumentType, KovanDocument, Visibility};

fn full() -> KovanDocument {
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
    .author(Author {
        family: "Smith".into(),
        given: "Jane".into(),
        affiliation: None,
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

fn minimal() -> KovanDocument {
    KovanDocument::new(
        "doc-1",
        "smith2020",
        Visibility::Proprietary,
        DocumentType::Paper,
        "A Title",
    )
}

#[test]
fn pre_port_json_loads_and_reserialises_identically() {
    for (text, want) in [
        (
            include_str!("data/kovan_document_b142a065/full.json"),
            full(),
        ),
        (
            include_str!("data/kovan_document_b142a065/minimal.json"),
            minimal(),
        ),
    ] {
        let doc: KovanDocument = serde_json::from_str(text).expect("old JSON loads");
        assert_eq!(doc, want);
        assert!(doc.zotero_item.is_none());
        assert_eq!(serde_json::to_string_pretty(&doc).unwrap(), text);
    }
}

#[test]
fn pre_port_toml_loads_and_reserialises_identically() {
    for (text, want) in [
        (
            include_str!("data/kovan_document_b142a065/full.toml"),
            full(),
        ),
        (
            include_str!("data/kovan_document_b142a065/minimal.toml"),
            minimal(),
        ),
    ] {
        let doc: KovanDocument = toml::from_str(text).expect("old TOML loads");
        assert_eq!(doc, want);
        assert!(doc.zotero_item.is_none());
        assert_eq!(toml::to_string(&doc).unwrap(), text);
    }
}

/// A document carrying a Zotero item round-trips through JSON and TOML.
#[test]
fn document_with_zotero_item_round_trips() {
    let item: kovan_common::zotero::ZoteroItem = serde_json::from_value(serde_json::json!({
        "key": "ABCD2345", "version": 4, "itemType": "journalArticle", "title": "T",
        "creators": [{"creatorType": "author", "firstName": "A", "lastName": "B"},
                     {"creatorType": "editor", "name": "Org"}],
        "tags": [{"tag": "x"}, {"tag": "auto", "type": 1}], "collections": ["COLL1234"],
        "relations": {"dc:relation": ["http://zotero.org/users/1/items/XYZ"]},
        "dateAdded": "2015-04-12T09:00:22Z"
    }))
    .unwrap();
    let doc = item.to_kovan_document();
    let json = serde_json::to_string(&doc).unwrap();
    assert_eq!(serde_json::from_str::<KovanDocument>(&json).unwrap(), doc);
    let toml_text = toml::to_string(&doc).unwrap();
    assert_eq!(toml::from_str::<KovanDocument>(&toml_text).unwrap(), doc);
}
