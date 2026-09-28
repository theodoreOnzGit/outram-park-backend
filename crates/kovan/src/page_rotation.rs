//! Turn one page of a PDF by a multiple of 90 degrees and save it
//! (maintainer, 2026-09-28: "there should be a way to rotate and save
//! individual pages of pdf in case they are in the 90 degree orientation").
//!
//! A table printed landscape on a portrait page is unreadable in the table
//! digitiser until it is turned. Rotating the **page** (its `/Rotate` entry,
//! PDF 32000-1:2008 §7.7.3.3) rather than the rendered picture keeps
//! everything consistent: `kopitiam_pdf` applies `/Rotate` in its page
//! transform for both rendering and structured text, so after a turn the
//! text selection still lands on the glyphs it highlights.
//!
//! The change is written as an **incremental update**
//! (`kopitiam_pdf::mupdf::incremental_update`): the original bytes are kept
//! untouched and a new version of the page object is appended. Truncating the
//! file back to its old length restores the original exactly.
//!
//! No `egui` here, so this is unit-tested without a window; the buttons are in
//! `crate::app::pdf_reader`.

use kopitiam_pdf::mupdf::page_edit::locate_page_slot;
use kopitiam_pdf::mupdf::{incremental_update, NewObject, Object, PdfDocument};

/// `/Rotate` is inheritable (§7.7.3.4): a page without its own entry takes
/// its nearest `/Pages` ancestor's. Walks up `/Parent` at most this far, so a
/// malformed cyclic tree cannot loop forever.
const MAX_TREE_DEPTH: usize = 64;

/// The rotation page `page_index` is displayed with, in degrees clockwise,
/// normalised to 0, 90, 180 or 270 the way the renderer snaps it.
pub fn effective_rotation(doc: &PdfDocument, page_index: usize) -> Result<i64, String> {
    let slot = locate_page_slot(doc, page_index).map_err(|e| e.to_string())?;
    let mut node = doc
        .resolve(&Object::new_indirect(slot.page_num as i64, slot.page_gen))
        .map_err(|e| e.to_string())?;
    for _ in 0..MAX_TREE_DEPTH {
        if let Some(r) = node.dict_gets("Rotate") {
            let r = doc.resolve(r).map_err(|e| e.to_string())?;
            return Ok(snap(r.to_int()));
        }
        match node.dict_gets("Parent") {
            Some(p) => node = doc.resolve(p).map_err(|e| e.to_string())?,
            None => break,
        }
    }
    Ok(0)
}

/// Snap any integer rotation to the nearest multiple of 90 in `[0, 360)`,
/// matching `kopitiam_pdf`'s page transform (MuPDF's rule).
fn snap(degrees: i64) -> i64 {
    let r = degrees.rem_euclid(360);
    (90 * ((r + 45) / 90)) % 360
}

/// Turn page `page_index` of `doc` by `quarter_turns` x 90 degrees
/// (positive = clockwise) and return the whole new file: the original bytes
/// plus an appended update carrying the page with its new `/Rotate`. The
/// page's own entry is set explicitly, so an inherited rotation is honoured
/// as the starting point and other pages are untouched.
pub fn rotate_page(
    doc: &PdfDocument,
    page_index: usize,
    quarter_turns: i32,
) -> Result<Vec<u8>, String> {
    let current = effective_rotation(doc, page_index)?;
    let target = snap(current + 90 * quarter_turns as i64);
    let slot = locate_page_slot(doc, page_index).map_err(|e| e.to_string())?;
    let mut page = doc
        .resolve(&Object::new_indirect(slot.page_num as i64, slot.page_gen))
        .map_err(|e| e.to_string())?;
    if !page.is_dict() {
        return Err(format!("page {} is not a dictionary", page_index + 1));
    }
    page.dict_put("Rotate", Object::Int(target));
    incremental_update(doc, &[(slot.page_num, NewObject::Plain(page))]).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A real two-page PDF built with `lopdf`, optionally with `/Rotate` on
    /// the page tree root so the second page inherits it.
    pub(crate) fn two_page_pdf(inherited_rotate: Option<i64>) -> Vec<u8> {
        use lopdf::{dictionary, Document, Object as L};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for _ in 0..2 {
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            kids.push(L::Reference(page));
        }
        let mut pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => 2,
        };
        if let Some(r) = inherited_rotate {
            pages.set("Rotate", r);
        }
        doc.objects.insert(pages_id, L::Dictionary(pages));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// A one-page portrait PDF with "Hi" drawn in Helvetica near the top
    /// left, so text positions can be checked.
    fn text_pdf() -> Vec<u8> {
        use lopdf::{content::Content, content::Operation, dictionary, Document, Stream};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![lopdf::Object::string_literal("Hi")]),
                Operation::new("ET", vec![]),
            ],
        };
        let contents = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => contents,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        });
        doc.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// The assumption this module rests on: `kopitiam_pdf` applies `/Rotate`
    /// to BOTH the rendered page and the structured text, so a selection
    /// drawn on the turned picture still finds the glyphs under it. If only
    /// the picture turned, the table digitiser would copy the wrong text.
    #[test]
    fn the_renderer_and_the_text_layer_both_follow_the_turn() {
        use kopitiam_pdf::mupdf::{page_to_stext, rasterize_page, StextBlock, StextOptions};
        let first_char = |doc: &PdfDocument| {
            let page = page_to_stext(doc, 0, StextOptions::default()).unwrap();
            page.blocks
                .iter()
                .find_map(|b| match b {
                    StextBlock::Text(t) => t.lines.first().and_then(|l| l.chars.first()).copied(),
                    _ => None,
                })
                .expect("the page has text")
        };
        let doc = PdfDocument::open(text_pdf()).unwrap();
        let pix = rasterize_page(&doc, 0, 72.0).unwrap();
        assert!(pix.h > pix.w, "portrait before: {}x{}", pix.w, pix.h);
        let h = first_char(&doc);
        // Unrotated, top-left origin: x = 100, y = 792 - 700 = 92.
        assert!(
            (h.origin.x - 100.0).abs() < 1.0 && (h.origin.y - 92.0).abs() < 1.0,
            "{:?}",
            h.origin
        );

        let turned = PdfDocument::open(rotate_page(&doc, 0, 1).unwrap()).unwrap();
        let pix = rasterize_page(&turned, 0, 72.0).unwrap();
        assert!(
            pix.w > pix.h,
            "landscape after a quarter turn: {}x{}",
            pix.w,
            pix.h
        );
        let h = first_char(&turned);
        // 90 degrees clockwise: the old top-left corner goes to the top
        // right. The glyph moves to x = 792 - 92 = 700, y = 100.
        assert!(
            (h.origin.x - 700.0).abs() < 1.0 && (h.origin.y - 100.0).abs() < 1.0,
            "{:?}",
            h.origin
        );
    }

    #[test]
    fn a_page_turns_clockwise_and_back_and_other_pages_stay_put() {
        let doc = PdfDocument::open(two_page_pdf(None)).unwrap();
        assert_eq!(effective_rotation(&doc, 0).unwrap(), 0);
        let turned = PdfDocument::open(rotate_page(&doc, 0, 1).unwrap()).unwrap();
        assert_eq!(effective_rotation(&turned, 0).unwrap(), 90);
        assert_eq!(
            effective_rotation(&turned, 1).unwrap(),
            0,
            "only the asked page turns"
        );
        let back = PdfDocument::open(rotate_page(&turned, 0, -1).unwrap()).unwrap();
        assert_eq!(effective_rotation(&back, 0).unwrap(), 0);
    }

    #[test]
    fn four_turns_come_full_circle_and_negative_turns_wrap() {
        let mut doc = PdfDocument::open(two_page_pdf(None)).unwrap();
        for _ in 0..4 {
            doc = PdfDocument::open(rotate_page(&doc, 1, 1).unwrap()).unwrap();
        }
        assert_eq!(effective_rotation(&doc, 1).unwrap(), 0);
        let ccw = PdfDocument::open(rotate_page(&doc, 1, -1).unwrap()).unwrap();
        assert_eq!(effective_rotation(&ccw, 1).unwrap(), 270);
    }

    #[test]
    fn an_inherited_rotation_is_the_starting_point() {
        let doc = PdfDocument::open(two_page_pdf(Some(90))).unwrap();
        assert_eq!(effective_rotation(&doc, 1).unwrap(), 90);
        let turned = PdfDocument::open(rotate_page(&doc, 1, 1).unwrap()).unwrap();
        assert_eq!(effective_rotation(&turned, 1).unwrap(), 180);
        assert_eq!(
            effective_rotation(&turned, 0).unwrap(),
            90,
            "page 1 still inherits"
        );
    }

    #[test]
    fn the_original_bytes_survive_as_a_prefix() {
        let original = two_page_pdf(None);
        let doc = PdfDocument::open(original.clone()).unwrap();
        let turned = rotate_page(&doc, 0, 1).unwrap();
        assert!(turned.len() > original.len());
        assert_eq!(&turned[..original.len()], &original[..]);
    }

    #[test]
    fn rotation_snaps_like_the_renderer() {
        assert_eq!(snap(0), 0);
        assert_eq!(snap(-90), 270);
        assert_eq!(snap(450), 90);
        assert_eq!(snap(100), 90);
        assert_eq!(snap(360), 0);
    }

    #[test]
    fn a_page_out_of_range_is_an_error_not_a_panic() {
        let doc = PdfDocument::open(two_page_pdf(None)).unwrap();
        assert!(rotate_page(&doc, 5, 1).is_err());
    }
}
