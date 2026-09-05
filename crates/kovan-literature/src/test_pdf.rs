//! Test-only helpers that synthesise a small, fully open/synthetic PDF with
//! `lopdf`, so the ingestion pipeline can be exercised end-to-end and offline
//! without shipping any real (possibly proprietary) PDF fixture.
//!
//! Not part of the public API — compiled only under `#[cfg(test)]`.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};

/// A unique temp path for a test artifact (process-scoped, no fixture files).
pub fn tmp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{}", std::process::id(), name))
}

/// Write a minimal one-page PDF carrying:
/// - a line of body text ("Kovan Steam Tables Report"), and
/// - an Info dictionary (`Title`, `Author`, `Keywords`, `CreationDate`).
///
/// All content is synthetic and open — safe to generate in tests.
pub fn build_text_pdf(path: &Path) {
    let mut doc = Document::with_version("1.5");

    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 18.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new(
                "Tj",
                vec![Object::string_literal("Kovan Steam Tables Report")],
            ),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("A Test Steam-Table Report"),
        "Author" => Object::string_literal("Doe, Jane and Smith, John"),
        "Keywords" => Object::string_literal("steam, IAPWS, benchmark"),
        "CreationDate" => Object::string_literal("D:20210304120000Z"),
    });
    doc.trailer.set("Info", info_id);

    doc.save(path).expect("write synthetic test PDF");
}

/// Write a minimal synthetic PDF with `page_count` pages, each carrying one line
/// of body text, and **no** Info dictionary.
///
/// Used to check that the page count is recovered from the page tree (`/Pages`
/// `/Count`) rather than from form-feed breaks in extracted text — the two
/// disagree for real-world producers whose output `pdf-extract` returns without
/// page separators. `page_count` is clamped to at least 1.
///
/// All content is synthetic and open — safe to generate in tests.
pub fn build_multipage_pdf(path: &Path, page_count: usize) {
    let n = page_count.max(1);
    let mut doc = Document::with_version("1.5");

    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut kids: Vec<Object> = Vec::with_capacity(n);
    for i in 0..n {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 18.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(format!("Kovan page {}", i + 1))],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        kids.push(page_id.into());
    }

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => n as i64,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    doc.save(path).expect("write synthetic multipage test PDF");
}
