//! PDF backend — true vector output, written directly.
//!
//! # Why there is no PDF library here
//!
//! The workspace reuse rule says look for an existing answer before writing
//! one, so this was checked first. The workspace already carries `lopdf` (a
//! low-level PDF object model, used by `kovan-literature`) and no plotting or
//! PDF-*writing* crate at all; `printpdf`, `plotters` and an SVG-to-PDF
//! converter would all have been new dependencies.
//!
//! `lopdf` would work, but it earns its keep on documents with fonts, images,
//! annotations, transparency groups or an existing file to parse — and this
//! figure has **none of those**. Because [`super::font`] has already reduced
//! every glyph to a polyline, a page here is one content stream of `m`/`l`/`S`
//! path operators, four small dictionaries and a cross-reference table. Writing
//! those directly is about eighty lines, adds **no dependency to this crate at
//! all**, and makes the output byte-for-byte reproducible, which is what issue
//! #26 asks for. If this file ever needs an embedded font, a raster image or
//! multiple pages, switch to `lopdf` rather than growing this.
//!
//! # Conformance
//!
//! PDF 1.4, one page, one uncompressed content stream, no `/Info` dictionary
//! (a creation date would destroy reproducibility), no external resources. The
//! `/MediaBox` is in PostScript points, so the figure prints at its declared
//! physical size.
//!
//! # Coordinates
//!
//! PDF's origin is the **bottom left** with y upward; the layout pass works
//! top-left with y downward. This is the only backend that flips.

use super::layout::{DrawOp, PageSize};
use super::Scene;

/// Renders a scene to a complete PDF file.
pub fn render(scene: &Scene, page: PageSize) -> Vec<u8> {
    let ops = super::layout::to_draw_ops(scene, page);
    render_ops(&ops, page)
}

/// Serialises an already-laid-out draw list to a complete PDF file.
pub fn render_ops(ops: &[DrawOp], page: PageSize) -> Vec<u8> {
    let content = content_stream(ops, page);
    assemble(content.as_bytes(), page)
}

/// Builds the page content stream: colour, width and dash state changes
/// interleaved with path construction and painting operators.
fn content_stream(ops: &[DrawOp], page: PageSize) -> String {
    let mut out = String::with_capacity(64 * 1024);
    // Round caps and joins, matching the SVG and PNG backends so a stroked
    // glyph looks the same in all three.
    out.push_str("1 J 1 j\n");
    let flip = |p: &[f64; 2]| [p[0], page.height_pt - p[1]];

    for op in ops {
        match op {
            DrawOp::Polygon { points, colour } => {
                if points.len() < 3 {
                    continue;
                }
                let [r, g, b] = colour.as_unit_floats();
                out.push_str(&format!("{} {} {} rg\n", num(r), num(g), num(b)));
                let first = flip(&points[0]);
                out.push_str(&format!("{} {} m\n", num(first[0]), num(first[1])));
                for p in &points[1..] {
                    let p = flip(p);
                    out.push_str(&format!("{} {} l\n", num(p[0]), num(p[1])));
                }
                // `h` closes the subpath, `f` fills it with the non-zero rule.
                out.push_str("h f\n");
            }
            DrawOp::Polyline {
                points,
                width,
                colour,
                dash,
            } => {
                if points.len() < 2 {
                    continue;
                }
                let [r, g, b] = colour.as_unit_floats();
                out.push_str(&format!("{} {} {} RG\n", num(r), num(g), num(b)));
                out.push_str(&format!("{} w\n", num(*width)));
                match dash {
                    Some((on, off)) => out.push_str(&format!("[{} {}] 0 d\n", num(*on), num(*off))),
                    None => out.push_str("[] 0 d\n"),
                }
                let first = flip(&points[0]);
                out.push_str(&format!("{} {} m\n", num(first[0]), num(first[1])));
                for p in &points[1..] {
                    let p = flip(p);
                    out.push_str(&format!("{} {} l\n", num(p[0]), num(p[1])));
                }
                out.push_str("S\n");
            }
        }
    }
    out
}

/// Wraps a content stream in the four objects a one-page PDF needs, and writes
/// a correct cross-reference table.
fn assemble(content: &[u8], page: PageSize) -> Vec<u8> {
    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R \
             /Resources << /ProcSet [/PDF] >> >>",
            num(page.width_pt),
            num(page.height_pt)
        )
        .into_bytes(),
        {
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend_from_slice(content);
            stream.extend_from_slice(b"\nendstream");
            stream
        },
    ];

    let mut out: Vec<u8> = Vec::with_capacity(content.len() + 2048);
    out.extend_from_slice(b"%PDF-1.4\n");
    // A binary comment line marks the file as binary for transfer tools.
    out.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");

    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    out
}

/// Formats a number for a PDF content stream: fixed point, trailing zeros
/// stripped, no exponent (PDF has no exponential number syntax).
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s
    }
}

/// Checks that the emitted PDF is structurally valid and reproducible.
///
/// # Methodology
///
/// Renders a scene twice and asserts byte equality. Then parses the produced
/// bytes far enough to prove the cross-reference table is self-consistent: the
/// `startxref` value must point at the literal `xref`, and every offset listed
/// in the table must land exactly on its own `N 0 obj` header. A wrong offset
/// is the classic hand-written-PDF bug and viewers reject the file outright, so
/// this is the gate that matters.
///
/// # Result
///
/// Passes as of 2026-08-20: header `%PDF-1.4`, 4 objects, all 4 offsets land on
/// their object headers, trailer `/Size 5 /Root 1 0 R`, file ends `%%EOF`.
#[cfg(test)]
#[test]
fn pdf_xref_offsets_are_correct_and_output_is_reproducible() {
    use super::{Series, SeriesStyle, INK};
    let mut scene = Scene::new("PDF gate", "x", "y");
    scene.series.push(Series {
        name: "s".into(),
        style: SeriesStyle::Line {
            width: 1.0,
            dash: None,
        },
        colour: INK,
        points: vec![[0.1, 0.1], [0.5, 0.8], [0.9, 0.2]],
        show_in_legend: true,
    });
    let a = render(&scene, PageSize::DEFAULT);
    let b = render(&scene, PageSize::DEFAULT);
    assert_eq!(a, b, "PDF export must be byte-reproducible");
    assert!(a.starts_with(b"%PDF-1.4"));
    assert!(a.ends_with(b"%%EOF\n"));

    // Byte offsets, not character offsets. The file carries the conventional
    // non-UTF-8 binary marker comment on line 2, so decoding it with
    // `from_utf8_lossy` first would substitute U+FFFD and shift every offset —
    // which would make this test fail on a perfectly valid file.
    let marker = b"startxref";
    let marker_at = (0..a.len())
        .rev()
        .find(|i| a[*i..].starts_with(marker))
        .expect("startxref present");
    let tail = String::from_utf8_lossy(&a[marker_at + marker.len()..]).to_string();
    let startxref = tail
        .lines()
        .find(|l| !l.trim().is_empty())
        .expect("startxref value")
        .trim()
        .parse::<usize>()
        .expect("startxref parses");
    assert!(
        a[startxref..].starts_with(b"xref"),
        "startxref must point at the xref table"
    );

    let text = String::from_utf8_lossy(&a[startxref..]).to_string();
    // Skip three lines, not two: `xref`, the `0 N` subsection header, and the
    // mandatory free-list entry for object 0, which is an `f` record.
    let mut object_number = 1usize;
    for line in text.lines().skip(3) {
        if !line.ends_with(" n ") {
            break;
        }
        let offset: usize = line[..10].parse().expect("xref offset parses");
        let header = format!("{object_number} 0 obj");
        assert!(
            a[offset..].starts_with(header.as_bytes()),
            "object {object_number} offset {offset} does not land on {header:?}"
        );
        object_number += 1;
    }
    assert_eq!(object_number, 5, "all four objects must be listed");
}
