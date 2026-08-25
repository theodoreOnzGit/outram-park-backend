//! Read a PDF's own embedded annotations (`/Annots`, ISO 32000-1 §12.5) and
//! turn them into an overlay [`super::pdf_reader`] paints on top of the
//! rasterized page — closing the "Okular writes highlights/notes into the
//! PDF, kovan never showed them" gap (GitHub issue #30).
//!
//! ## Scope: geometry + colour, not appearance-stream compositing
//!
//! A conforming PDF annotation normally carries its own `/AP` `/N` (normal)
//! appearance stream — the *exact* content a viewer should paint, generated
//! by whatever tool created it (Okular included). Executing that stream
//! needs `kopitiam_pdf`'s content-stream interpreter
//! (`kopitiam_pdf::mupdf::interpret::Processor::new`/`run_stream`), which is
//! `pub(crate)` inside that crate — not reachable from here. That gap is
//! filed upstream in `docs/kopitiam-issues/` (see that directory's README)
//! asking for a public entry point so a follow-up can render `/AP`
//! faithfully; `kovan` must not modify `kopitiam-pdf` itself to work around
//! it (workspace hard rule).
//!
//! Until then, this module reads each annotation's own declared geometry
//! (`/Rect`, `/QuadPoints`, `/L`, `/InkList`) and colour (`/C`, `/IC`,
//! `/CA`) directly — plain PDF dictionary data, defined by the ISO 32000
//! specification itself, read through `kopitiam_pdf`'s already-public
//! [`PdfDocument`]/[`Object`] accessors — and draws a faithful
//! *approximation*: filled/stroked primitives instead of the real
//! appearance pixels. This makes every annotation visible, in roughly the
//! right place and colour; it will not reproduce a custom stamp image or a
//! `/DA`-styled `FreeText` box pixel-for-pixel.
//!
//! ## Coordinate mapping
//!
//! [`pdf_point_to_pixel`] independently re-derives the PDF user-space ->
//! `RENDER_DPI` texture-pixel-space transform that `rasterize_page` itself
//! applies (`kopitiam_pdf`'s own page-to-device matrix, `page_run.rs`'s
//! `page_ctm`, is `pub(crate)` too, so it cannot be called directly) —
//! flip y, apply `/Rotate`, shift the `/MediaBox` origin to `(0, 0)`, then
//! scale by `dpi / 72`. It is **not** trusted on inspection alone: this
//! module's tests build a real minimal PDF with a filled rectangle at a
//! known `/Rect`, rasterize it with the real [`rasterize_page`], and check
//! the pixels this function predicts are the filled ones — for `/Rotate 0`
//! and `/Rotate 90` (the swap direction is the main way a hand-derived
//! rotation transform gets it backwards).

use eframe::egui::{Color32, Pos2};
use kopitiam_pdf::mupdf::{cmyk_to_rgb, gray_to_rgb, Object, PdfDocument};

/// Which PDF annotation subtype this is — the subtypes Okular's own
/// annotation toolbar actually produces, plus [`AnnotationKind::Other`] for
/// everything else present in a file but out of scope to draw (`Link`,
/// `Widget`, `Sound`, `Movie`, `3D`, an undecodable `Stamp`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationKind {
    Highlight,
    Underline,
    StrikeOut,
    Squiggly,
    /// A note (`/Subtype /Text`) or its detached comment window (`/Popup`)
    /// — both drawn as a small icon with `/Contents` shown on hover.
    Note,
    FreeText,
    Square,
    Circle,
    Line,
    Ink,
    Other(String),
}

/// One annotation read off a page, already in this panel's texture-pixel
/// space (scaled/flipped/rotated by [`pdf_point_to_pixel`]) — a caller only
/// needs its own texture-pixel -> screen transform on top, the same as it
/// already applies to kovan's native [`super::pdf_reader`] annotations.
#[derive(Debug, Clone)]
pub struct PdfAnnotation {
    pub kind: AnnotationKind,
    /// Bounding rect (`/Rect`), texture-pixel space, corners normalised so
    /// `min <= max` on both axes.
    pub rect: (Pos2, Pos2),
    /// Markup quads (`/QuadPoints`), one 4-corner polygon per quad, corners
    /// reordered into a simple (non-self-intersecting) draw order — see
    /// [`read_quad_points`]. `Highlight`/`Underline`/`StrikeOut`/`Squiggly`
    /// only; empty for every other kind.
    pub quads: Vec<[Pos2; 4]>,
    /// Freehand ink strokes (`/InkList`) or the `Line` subtype's single
    /// 2-point stroke (`/L`) — one polyline per stroke; empty for every
    /// other kind.
    pub strokes: Vec<Vec<Pos2>>,
    /// `/C` — the annotation's main colour (border/stroke, or the markup
    /// colour for text-markup kinds). `None` for an explicitly empty `/C`
    /// (transparent, per spec) or when the key is absent.
    pub color: Option<Color32>,
    /// `/IC` — the interior (fill) colour, `Square`/`Circle` only.
    pub interior_color: Option<Color32>,
    /// `/CA` — the annotation's opacity, `1.0` when absent (spec default).
    pub opacity: f32,
    /// `/Contents` — the note/comment text, decoded per the PDF "text
    /// string" convention (UTF-16BE with a BOM, or PDFDocEncoding).
    pub contents: String,
    /// `/T` — the annotation's title (conventionally the author), decoded
    /// the same way as `contents`.
    pub author: String,
}

/// Structural, content-free dump of what this module sees on `page_index`,
/// for the `KOVAN_ANNOT_DEBUG=1` diagnostic (see [`super::pdf_reader`]'s
/// module doc). Reports which of [`read_page_annotations`]'s early-outs
/// fires and how many raw `/Annots` entries survive each filtering step —
/// the seam between "this page genuinely has no annotations" and "it has
/// them but something here dropped them". Prints no annotation *content*.
pub fn debug_describe_page(doc: &PdfDocument, page_index: usize) -> String {
    let Ok(page) = doc.page(page_index).cloned() else {
        return format!("page {page_index}: doc.page() FAILED");
    };
    let Some(annots_raw) = page.dict_gets("Annots") else {
        let keys: Vec<String> = (0..page.dict_len())
            .filter_map(|i| page.dict_get_key(i))
            .map(|k| String::from_utf8_lossy(k).into_owned())
            .collect();
        return format!("page {page_index}: page dict has NO /Annots key; keys={keys:?}");
    };
    let is_ref = annots_raw.is_indirect();
    let Ok(annots) = doc.resolve(annots_raw) else {
        return format!("page {page_index}: /Annots present (indirect={is_ref}) but resolve() FAILED");
    };
    let n = annots.array_len();
    let mut subtypes: Vec<String> = Vec::new();
    let mut unresolvable = 0;
    let mut non_dict = 0;
    let mut flag_skipped = 0;
    for i in 0..n {
        let Some(entry) = annots.array_get(i) else { continue };
        let Ok(annot) = doc.resolve(entry) else {
            unresolvable += 1;
            continue;
        };
        if !annot.is_dict() {
            non_dict += 1;
            continue;
        }
        let flags = doc.resolve_get(&annot, "F").map(|o| o.to_int()).unwrap_or(0);
        if flags & (1 << 1) != 0 || flags & (1 << 5) != 0 {
            flag_skipped += 1;
        }
        subtypes.push(
            annot
                .dict_gets("Subtype")
                .map(|o| String::from_utf8_lossy(o.to_name()).into_owned())
                .unwrap_or_else(|| "<none>".to_string()),
        );
    }
    format!(
        "page {page_index}: /Annots indirect={is_ref} array_len={n} unresolvable={unresolvable} \
non_dict={non_dict} hidden_or_noview={flag_skipped} subtypes={subtypes:?}"
    )
}

/// Read every visible annotation on page `page_index` (0-based), already
/// mapped into `dpi`-scaled texture-pixel space. Skips an annotation whose
/// `/F` flags mark it `Hidden` (bit 2) or `NoView` (bit 6) — the two flags
/// ISO 32000-1 Table 165 defines specifically to mean "a viewer must not
/// render this". Malformed/unresolvable entries are skipped individually
/// rather than failing the whole page, matching `kopitiam_pdf`'s own
/// "safe, silent failure on a type mismatch" accessor convention.
pub fn read_page_annotations(doc: &PdfDocument, page_index: usize, dpi: f32) -> Vec<PdfAnnotation> {
    let Ok(page) = doc.page(page_index).cloned() else {
        return Vec::new();
    };
    let mediabox = mediabox_of(doc, &page);
    let rotate = rotate_of(doc, &page);
    let to_px = |x: f32, y: f32| pdf_point_to_pixel(mediabox, rotate, dpi, x, y);

    let Some(annots) = page.dict_gets("Annots") else {
        return Vec::new();
    };
    let Ok(annots) = doc.resolve(annots) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for i in 0..annots.array_len() {
        let Some(entry) = annots.array_get(i) else { continue };
        let Ok(annot) = doc.resolve(entry) else { continue };
        if !annot.is_dict() {
            continue;
        }

        let flags = doc.resolve_get(&annot, "F").map(|o| o.to_int()).unwrap_or(0);
        const HIDDEN: i64 = 1 << 1;
        const NO_VIEW: i64 = 1 << 5;
        if flags & HIDDEN != 0 || flags & NO_VIEW != 0 {
            continue;
        }

        let subtype = annot
            .dict_gets("Subtype")
            .map(|o| String::from_utf8_lossy(o.to_name()).into_owned())
            .unwrap_or_default();
        let kind = match subtype.as_str() {
            "Highlight" => AnnotationKind::Highlight,
            "Underline" => AnnotationKind::Underline,
            "StrikeOut" => AnnotationKind::StrikeOut,
            "Squiggly" => AnnotationKind::Squiggly,
            "Text" | "Popup" => AnnotationKind::Note,
            "FreeText" => AnnotationKind::FreeText,
            "Square" => AnnotationKind::Square,
            "Circle" => AnnotationKind::Circle,
            "Line" => AnnotationKind::Line,
            "Ink" => AnnotationKind::Ink,
            other => AnnotationKind::Other(other.to_string()),
        };
        if matches!(kind, AnnotationKind::Other(_)) {
            continue;
        }

        let Some(rect) = read_rect(doc, &annot) else { continue };
        let (rmin, rmax) = (to_px(rect.0, rect.1), to_px(rect.2, rect.3));
        let rect_px = (
            Pos2::new(rmin.x.min(rmax.x), rmin.y.min(rmax.y)),
            Pos2::new(rmin.x.max(rmax.x), rmin.y.max(rmax.y)),
        );

        let quads = if matches!(
            kind,
            AnnotationKind::Highlight | AnnotationKind::Underline | AnnotationKind::StrikeOut | AnnotationKind::Squiggly
        ) {
            read_quad_points(doc, &annot, to_px)
        } else {
            Vec::new()
        };

        let strokes = match kind {
            AnnotationKind::Ink => read_ink_list(doc, &annot, to_px),
            AnnotationKind::Line => read_line(doc, &annot, to_px).into_iter().collect(),
            _ => Vec::new(),
        };

        out.push(PdfAnnotation {
            kind,
            rect: rect_px,
            quads,
            strokes,
            color: read_color(doc, &annot, "C"),
            interior_color: read_color(doc, &annot, "IC"),
            opacity: doc
                .resolve_get(&annot, "CA")
                .ok()
                .map(|o| o.to_real() as f32)
                .filter(|v| v.is_finite())
                .unwrap_or(1.0)
                .clamp(0.0, 1.0),
            contents: read_text_string(doc, &annot, "Contents"),
            author: read_text_string(doc, &annot, "T"),
        });
    }
    out
}

/// `/Rect`, in raw PDF user-space points: `(x0, y0, x1, y1)`.
fn read_rect(doc: &PdfDocument, annot: &Object) -> Option<(f32, f32, f32, f32)> {
    let arr = doc.resolve_get(annot, "Rect").ok()?;
    if arr.array_len() < 4 {
        return None;
    }
    let v = |i: usize| -> f32 {
        arr.array_get(i)
            .and_then(|o| doc.resolve(o).ok())
            .map(|o| o.to_real() as f32)
            .unwrap_or(0.0)
    };
    Some((v(0), v(1), v(2), v(3)))
}

/// `/QuadPoints`: groups of 8 numbers, `x1 y1 x2 y2 x3 y3 x4 y4` per group
/// (ISO 32000-1 Table 179). The de-facto producer order is top-left,
/// top-right, bottom-left, bottom-right — connecting the four points in
/// that literal 1-2-3-4 order draws a self-intersecting "bowtie", not the
/// intended quadrilateral, so corners 3 and 4 are swapped into a proper
/// simple-polygon draw order (1, 2, 4, 3) before returning.
fn read_quad_points(
    doc: &PdfDocument,
    annot: &Object,
    to_px: impl Fn(f32, f32) -> Pos2,
) -> Vec<[Pos2; 4]> {
    let Ok(arr) = doc.resolve_get(annot, "QuadPoints") else { return Vec::new() };
    let n = arr.array_len();
    let v = |i: usize| -> f32 {
        arr.array_get(i)
            .and_then(|o| doc.resolve(o).ok())
            .map(|o| o.to_real() as f32)
            .unwrap_or(0.0)
    };
    let mut quads = Vec::with_capacity(n / 8);
    let mut i = 0;
    while i + 8 <= n {
        let p = |k: usize| to_px(v(i + 2 * k), v(i + 2 * k + 1));
        quads.push([p(0), p(1), p(3), p(2)]);
        i += 8;
    }
    quads
}

/// `/InkList`: an array of stroke arrays, each a flat `x1 y1 x2 y2 ...`
/// point list (ISO 32000-1 Table 175).
fn read_ink_list(doc: &PdfDocument, annot: &Object, to_px: impl Fn(f32, f32) -> Pos2) -> Vec<Vec<Pos2>> {
    let Ok(list) = doc.resolve_get(annot, "InkList") else { return Vec::new() };
    let mut strokes = Vec::with_capacity(list.array_len());
    for si in 0..list.array_len() {
        let Some(stroke) = list.array_get(si) else { continue };
        let Ok(stroke) = doc.resolve(stroke) else { continue };
        let n = stroke.array_len();
        let v = |i: usize| -> f32 {
            stroke
                .array_get(i)
                .and_then(|o| doc.resolve(o).ok())
                .map(|o| o.to_real() as f32)
                .unwrap_or(0.0)
        };
        let mut pts = Vec::with_capacity(n / 2);
        let mut i = 0;
        while i + 2 <= n {
            pts.push(to_px(v(i), v(i + 1)));
            i += 2;
        }
        if pts.len() >= 2 {
            strokes.push(pts);
        }
    }
    strokes
}

/// `/L`: `[x1 y1 x2 y2]`, the `Line` subtype's single straight segment
/// (ISO 32000-1 Table 176).
fn read_line(doc: &PdfDocument, annot: &Object, to_px: impl Fn(f32, f32) -> Pos2) -> Option<Vec<Pos2>> {
    let arr = doc.resolve_get(annot, "L").ok()?;
    if arr.array_len() < 4 {
        return None;
    }
    let v = |i: usize| -> f32 {
        arr.array_get(i)
            .and_then(|o| doc.resolve(o).ok())
            .map(|o| o.to_real() as f32)
            .unwrap_or(0.0)
    };
    Some(vec![to_px(v(0), v(1)), to_px(v(2), v(3))])
}

/// A `/C`-or-`/IC`-shaped colour array: 0 components means "transparent"
/// (`None`), 1 is `DeviceGray`, 3 is `DeviceRGB`, 4 is `DeviceCMYK` (ISO
/// 32000-1 §8.6.3) — reusing `kopitiam_pdf`'s own gray/CMYK -> RGB
/// conversion so a colour here matches how the same components would
/// render inside a page's own content stream.
fn read_color(doc: &PdfDocument, annot: &Object, key: &str) -> Option<Color32> {
    let arr = doc.resolve_get(annot, key).ok()?;
    let n = arr.array_len();
    if n == 0 {
        return None;
    }
    let v = |i: usize| -> f32 {
        arr.array_get(i)
            .and_then(|o| doc.resolve(o).ok())
            .map(|o| o.to_real() as f32)
            .unwrap_or(0.0)
    };
    let to_u8 = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgb = match n {
        1 => gray_to_rgb(v(0)),
        3 => [v(0), v(1), v(2)],
        4 => cmyk_to_rgb(v(0), v(1), v(2), v(3)),
        _ => return None,
    };
    Some(Color32::from_rgb(to_u8(rgb[0]), to_u8(rgb[1]), to_u8(rgb[2])))
}

/// Decode a PDF "text string" (ISO 32000-1 §7.9.2.2): a `FE FF` big-endian
/// UTF-16 BOM prefix means UTF-16BE, otherwise the bytes are PDFDocEncoding
/// — approximated here as Latin-1/ASCII (every character this module
/// actually needs to display — plain author names and note text — round
/// -trips correctly; the handful of PDFDocEncoding code points that differ
/// from Latin-1 are typographic punctuation this module doesn't need to get
/// byte-exact).
fn read_text_string(doc: &PdfDocument, annot: &Object, key: &str) -> String {
    let Ok(obj) = doc.resolve_get(annot, key) else { return String::new() };
    let bytes = obj.to_string_bytes();
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// `/MediaBox`, normalised so `(x0, y0) <= (x1, y1)`; falls back to US
/// Letter (612x792pt) when absent or degenerate, matching
/// `rasterize_page`'s own fallback.
fn mediabox_of(doc: &PdfDocument, page: &Object) -> (f32, f32, f32, f32) {
    if let Ok(arr) = doc.resolve_get(page, "MediaBox") {
        if arr.array_len() >= 4 {
            let v = |i: usize| -> f32 {
                arr.array_get(i)
                    .and_then(|o| doc.resolve(o).ok())
                    .map(|o| o.to_real() as f32)
                    .unwrap_or(0.0)
            };
            let (x0, y0, x1, y1) = (v(0), v(1), v(2), v(3));
            if (x1 - x0).abs() >= 1.0 && (y1 - y0).abs() >= 1.0 {
                return (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
            }
        }
    }
    (0.0, 0.0, 612.0, 792.0)
}

/// `/Rotate`, snapped to `{0, 90, 180, 270}` the same way `rasterize_page`
/// snaps it (round to the nearest multiple of 90, then reduce mod 360).
fn rotate_of(doc: &PdfDocument, page: &Object) -> i32 {
    let raw = doc.resolve_get(page, "Rotate").map(|o| o.to_int()).unwrap_or(0);
    let mut r = ((raw % 360) + 360) % 360;
    r = 90 * ((r + 45) / 90);
    (r % 360) as i32
}

/// Map a PDF user-space point `(x, y)` to this panel's `dpi`-scaled
/// texture-pixel space, given the page's `mediabox` (`x0, y0, x1, y1`,
/// already normalised) and its snapped `/Rotate`. See the module doc for
/// why this exists as an independent re-derivation rather than a call into
/// `kopitiam_pdf` (whose equivalent, `page_ctm`, is private) and how it is
/// checked against a real render instead of trusted on inspection.
pub fn pdf_point_to_pixel(mediabox: (f32, f32, f32, f32), rotate: i32, dpi: f32, x: f32, y: f32) -> Pos2 {
    let (x0, y0, x1, y1) = mediabox;
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    // Box-relative, y-flipped: PDF space has y increasing upward from the
    // box's bottom edge; pixel space has y increasing downward from the top.
    let bx = x - x0;
    let by = h - (y - y0);
    let (px, py) = match rotate {
        90 => (h - by, bx),
        180 => (w - bx, h - by),
        270 => (by, w - bx),
        _ => (bx, by),
    };
    let scale = dpi / 72.0;
    Pos2::new(px * scale, py * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kopitiam_pdf::mupdf::{rasterize_page_native, PdfDocument};

    /// Build a one-page PDF from `bodies` (objects 1..) with a classic
    /// xref, the same minimal-fixture shape `kopitiam_pdf`'s own tests use
    /// (a generic PDF-testing technique, not anything copied from that
    /// crate).
    fn build_pdf(bodies: &[&[u8]]) -> Vec<u8> {
        let mut pdf: Vec<u8> = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.5\n");
        let mut offsets = vec![0usize; bodies.len() + 1];
        for (idx, body) in bodies.iter().enumerate() {
            let num = idx + 1;
            offsets[num] = pdf.len();
            pdf.extend_from_slice(format!("{num} 0 obj\n").as_bytes());
            pdf.extend_from_slice(body);
            pdf.extend_from_slice(b"\nendobj\n");
        }
        let xref_ofs = pdf.len();
        let size = bodies.len() + 1;
        pdf.extend_from_slice(format!("xref\n0 {size}\n").as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets.iter().skip(1) {
            pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref_ofs}\n%%EOF\n").as_bytes(),
        );
        pdf
    }

    /// A **non-square** 300x200pt page (deliberately not square — a square
    /// mediabox can't catch a width/height mix-up in the 90/270 branches,
    /// since both dimensions carry the same magnitude) with a solid black
    /// rectangle filled at PDF-space `(50, 120)..(90, 160)` — in the upper
    /// portion when unrotated (PDF y increases upward). `rotate` becomes
    /// that page's `/Rotate`.
    fn rect_page_doc(rotate: i32) -> PdfDocument {
        let content = b"<< /Length 40 >>\nstream\n0 0 0 rg 50 120 40 40 re f\nendstream";
        let page = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Rotate {rotate} /Contents 4 0 R >>"
        );
        let bodies: [&[u8]; 4] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page.as_bytes(),
            content,
        ];
        PdfDocument::open(build_pdf(&bodies)).unwrap()
    }

    /// A pixel is "filled" if it's much darker than white — matches how
    /// `kopitiam_pdf`'s own rasterizer tests define "dark".
    fn is_dark(doc_pix: &kopitiam_pdf::mupdf::Pixmap, x: i32, y: i32) -> bool {
        doc_pix.luma(x, y).map(|l| l < 60).unwrap_or(false)
    }

    /// Checked at every one of the four snapped `/Rotate` values on a
    /// deliberately non-square page — a square mediabox can't catch a
    /// width/height mix-up in the 90/270 branches, since both dimensions
    /// carry the same magnitude.
    #[test]
    fn rotated_pixel_matches_a_real_rasterized_page() {
        for rotate in [0, 90, 180, 270] {
            let doc = rect_page_doc(rotate);
            let pix = rasterize_page_native(&doc, 0, 72.0).unwrap();
            let mediabox = mediabox_of(&doc, &doc.page(0).unwrap().clone());
            assert_eq!(rotate_of(&doc, &doc.page(0).unwrap().clone()), rotate);

            let p = pdf_point_to_pixel(mediabox, rotate, 72.0, 70.0, 140.0);
            assert!(
                is_dark(&pix, p.x as i32, p.y as i32),
                "predicted centre pixel ({p:?}) is not dark under /Rotate {rotate} ({}x{} pixmap)",
                pix.w,
                pix.h
            );
            for (x, y) in [(45.0, 140.0), (95.0, 140.0), (70.0, 115.0), (70.0, 165.0)] {
                let p = pdf_point_to_pixel(mediabox, rotate, 72.0, x, y);
                assert!(
                    !is_dark(&pix, p.x as i32, p.y as i32),
                    "predicted margin pixel ({p:?}) is unexpectedly dark under /Rotate {rotate}"
                );
            }
        }
    }

    fn highlight_page_doc() -> PdfDocument {
        let annot = b"<< /Type /Annot /Subtype /Highlight /Rect [50 120 90 160] \
/QuadPoints [50 160 90 160 50 120 90 120] /C [1 1 0] /CA 0.4 /Contents (hello) /T (tester) >>";
        let page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>";
        let bodies: [&[u8]; 4] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
            annot,
        ];
        PdfDocument::open(build_pdf(&bodies)).unwrap()
    }

    #[test]
    fn highlight_annotation_is_read_with_its_quad_colour_and_text() {
        let doc = highlight_page_doc();
        let anns = read_page_annotations(&doc, 0, 72.0);
        assert_eq!(anns.len(), 1);
        let a = &anns[0];
        assert_eq!(a.kind, AnnotationKind::Highlight);
        assert_eq!(a.quads.len(), 1);
        assert_eq!(a.color, Some(Color32::from_rgb(255, 255, 0)));
        assert!((a.opacity - 0.4).abs() < 1e-6);
        assert_eq!(a.contents, "hello");
        assert_eq!(a.author, "tester");
    }

    fn ink_page_doc() -> PdfDocument {
        let annot = b"<< /Type /Annot /Subtype /Ink /Rect [40 40 160 160] \
/InkList [[40 40 100 100 160 160] [50 150 150 50]] /C [1 0 0] /CA 1 /Border [0 0 2] >>";
        let page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>";
        let bodies: [&[u8]; 4] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
            annot,
        ];
        PdfDocument::open(build_pdf(&bodies)).unwrap()
    }

    #[test]
    fn ink_annotation_is_read_with_both_strokes() {
        let doc = ink_page_doc();
        let anns = read_page_annotations(&doc, 0, 72.0);
        assert_eq!(anns.len(), 1, "expected exactly one annotation to be read");
        let a = &anns[0];
        assert_eq!(a.kind, AnnotationKind::Ink);
        assert_eq!(a.strokes.len(), 2, "expected both InkList strokes");
        assert_eq!(a.strokes[0].len(), 3);
        assert_eq!(a.strokes[1].len(), 2);
        assert_eq!(a.color, Some(Color32::from_rgb(255, 0, 0)));
    }

    /// A `/Annots` entry that is itself an indirect reference to an array
    /// (rather than the array sitting inline in the page dict) — some
    /// producers write it this way. `doc.resolve(annots)` in
    /// `read_page_annotations` must follow that indirection.
    #[test]
    fn indirect_annots_array_is_resolved() {
        let annot = b"<< /Type /Annot /Subtype /Ink /Rect [40 40 160 160] \
/InkList [[40 40 100 100 160 160]] /C [1 0 0] >>";
        let annots_arr = b"[5 0 R]";
        let page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots 4 0 R >>";
        let bodies: [&[u8]; 5] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
            annots_arr,
            annot,
        ];
        let doc = PdfDocument::open(build_pdf(&bodies)).unwrap();
        let anns = read_page_annotations(&doc, 0, 72.0);
        assert_eq!(anns.len(), 1, "expected the indirect /Annots array to be resolved");
    }

    #[test]
    fn hidden_annotation_is_skipped() {
        let annot = b"<< /Type /Annot /Subtype /Highlight /Rect [0 0 10 10] /F 2 >>";
        let page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>";
        let bodies: [&[u8]; 4] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
            annot,
        ];
        let doc = PdfDocument::open(build_pdf(&bodies)).unwrap();
        assert!(read_page_annotations(&doc, 0, 72.0).is_empty());
    }

    #[test]
    fn link_annotation_is_ignored() {
        let annot = b"<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] >>";
        let page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>";
        let bodies: [&[u8]; 4] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
            annot,
        ];
        let doc = PdfDocument::open(build_pdf(&bodies)).unwrap();
        assert!(read_page_annotations(&doc, 0, 72.0).is_empty());
    }

    #[test]
    fn page_with_no_annots_key_returns_empty() {
        let page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>";
        let bodies: [&[u8]; 3] = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
        ];
        let doc = PdfDocument::open(build_pdf(&bodies)).unwrap();
        assert!(read_page_annotations(&doc, 0, 72.0).is_empty());
    }
}
