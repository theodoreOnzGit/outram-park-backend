//! A small single-stroke vector font, used by every export backend.
//!
//! # Why a stroke font instead of a real one
//!
//! The three export backends (SVG, PDF, PNG) have to produce *the same figure*.
//! SVG and PDF can both reference a system or base-14 font; a raster PNG cannot
//! — it needs glyph outlines rasterised, which means either a font rasteriser
//! dependency or a font file to embed. Rather than add either, and rather than
//! let the PNG drift typographically from the vector formats, all three
//! backends draw text as **polylines** from the table below.
//!
//! Two things fall out of that choice, both of them useful:
//!
//! * the PDF writer needs no font dictionary, no encoding table and no embedded
//!   font program — it is a pure path stream, which is why
//!   [`super::pdf`] is as short as it is;
//! * the figures are geometrically identical across formats, so a PNG preview is
//!   an exact preview of the PDF that goes into a paper.
//!
//! The look is the classic plotter/CAD single-stroke lettering that Hershey-font
//! plotting libraries produce. It is deliberately not a typeface with filled
//! outlines.
//!
//! # Glyph coordinate system
//!
//! Each glyph is a list of polylines on an integer grid:
//!
//! * `y = 0` is the baseline,
//! * `y = CAP_HEIGHT` (14) is the cap height,
//! * `y = X_HEIGHT` (10) is the x-height,
//! * `y = DESCENDER` (-4) is the descender depth,
//! * `x` runs from 0 to at most 8, and every glyph advances by [`ADVANCE`] (10).
//!
//! The font is monospaced. For axis labels, legends and annotations — all this
//! tool draws — that is adequate, and it makes text width exactly computable
//! without any metric table.
//!
//! # Coverage
//!
//! Printable ASCII (0x20–0x7E) plus the degree sign `°`, the middle dot `·` and
//! the multiplication sign `×`, which show up in unit labels. Any character
//! outside that set is drawn as a hollow box so that a missing glyph is visible
//! rather than silently dropped.

/// Baseline-to-cap-height distance, in glyph grid units.
pub const CAP_HEIGHT: f64 = 14.0;
/// Baseline-to-x-height distance, in glyph grid units.
///
/// Not used by the layout pass — text is positioned by cap height and baseline
/// — but it is the metric the lowercase glyphs above are drawn against, so it
/// belongs with them rather than only in a comment.
#[allow(dead_code)]
pub const X_HEIGHT: f64 = 10.0;
/// Depth of descenders below the baseline, in glyph grid units.
///
/// Like [`X_HEIGHT`], this is a metric the glyph table is drawn against rather
/// than one the layout pass reads; it is the bound
/// `every_glyph_stays_inside_its_box` checks against.
#[allow(dead_code)]
pub const DESCENDER: f64 = -4.0;
/// Horizontal advance per character, in glyph grid units (the font is
/// monospaced).
pub const ADVANCE: f64 = 10.0;

/// One glyph: a set of polylines in glyph grid units.
type Glyph = &'static [&'static [(i8, i8)]];

/// Fallback glyph for anything not in the table: a hollow box, so a missing
/// character is visible in the output instead of silently vanishing.
const TOFU: Glyph = &[&[(1, 0), (7, 0), (7, 14), (1, 14), (1, 0)]];

/// Returns the polylines for `ch`, or the missing-glyph box.
pub fn glyph(ch: char) -> Glyph {
    match ch {
        ' ' => &[],
        '!' => &[&[(4, 14), (4, 4)], &[(4, 1), (4, 0)]],
        '"' => &[&[(3, 14), (3, 10)], &[(5, 14), (5, 10)]],
        '#' => &[
            &[(2, 0), (3, 14)],
            &[(5, 0), (6, 14)],
            &[(1, 5), (7, 5)],
            &[(1, 9), (7, 9)],
        ],
        '$' => &[
            &[
                (7, 12),
                (5, 14),
                (3, 14),
                (1, 12),
                (1, 10),
                (7, 6),
                (7, 3),
                (5, 1),
                (3, 1),
                (1, 3),
            ],
            &[(4, 15), (4, 0)],
        ],
        // The diagonal runs bottom-left to top-right. Glyph y is measured
        // upward, so `(1, 0) -> (7, 14)` is the "/" stroke a per-cent sign
        // needs; the mirror image reads as a backslash on the page.
        '%' => &[
            &[(1, 0), (7, 14)],
            &[(1, 14), (3, 14), (3, 11), (1, 11), (1, 14)],
            &[(5, 3), (7, 3), (7, 0), (5, 0), (5, 3)],
        ],
        '&' => &[&[
            (7, 0),
            (2, 9),
            (2, 12),
            (4, 14),
            (6, 12),
            (6, 10),
            (1, 5),
            (1, 2),
            (3, 0),
            (5, 0),
            (7, 3),
        ]],
        '\'' => &[&[(4, 14), (4, 10)]],
        '(' => &[&[(6, 15), (3, 11), (3, 3), (6, -1)]],
        ')' => &[&[(2, 15), (5, 11), (5, 3), (2, -1)]],
        '*' => &[&[(4, 12), (4, 6)], &[(1, 11), (7, 7)], &[(7, 11), (1, 7)]],
        '+' => &[&[(4, 11), (4, 3)], &[(1, 7), (7, 7)]],
        ',' => &[&[(5, 1), (4, 0), (3, -2)]],
        '-' => &[&[(1, 7), (7, 7)]],
        '.' => &[&[(4, 1), (4, 0)]],
        '/' => &[&[(1, -1), (7, 15)]],
        '0' => &[
            &[
                (1, 4),
                (1, 10),
                (2, 13),
                (4, 14),
                (6, 13),
                (7, 10),
                (7, 4),
                (6, 1),
                (4, 0),
                (2, 1),
                (1, 4),
            ],
            &[(2, 3), (6, 11)],
        ],
        '1' => &[&[(2, 12), (4, 14), (4, 0)], &[(2, 0), (6, 0)]],
        '2' => &[&[
            (1, 11),
            (2, 13),
            (4, 14),
            (6, 13),
            (7, 11),
            (7, 9),
            (1, 0),
            (7, 0),
        ]],
        '3' => &[&[
            (1, 14),
            (7, 14),
            (4, 8),
            (6, 8),
            (7, 6),
            (7, 3),
            (5, 0),
            (2, 0),
            (1, 2),
        ]],
        '4' => &[&[(5, 0), (5, 14), (1, 4), (7, 4)]],
        '5' => &[&[
            (7, 14),
            (2, 14),
            (1, 8),
            (4, 9),
            (6, 8),
            (7, 5),
            (6, 1),
            (3, 0),
            (1, 2),
        ]],
        '6' => &[&[
            (7, 13),
            (4, 14),
            (2, 12),
            (1, 8),
            (1, 3),
            (3, 0),
            (5, 0),
            (7, 3),
            (7, 5),
            (5, 8),
            (3, 8),
            (1, 6),
        ]],
        '7' => &[&[(1, 14), (7, 14), (3, 0)]],
        '8' => &[
            &[
                (3, 8),
                (1, 10),
                (1, 12),
                (3, 14),
                (5, 14),
                (7, 12),
                (7, 10),
                (5, 8),
                (3, 8),
            ],
            &[
                (5, 8),
                (7, 6),
                (7, 2),
                (5, 0),
                (3, 0),
                (1, 2),
                (1, 6),
                (3, 8),
            ],
        ],
        '9' => &[&[
            (1, 1),
            (4, 0),
            (6, 2),
            (7, 6),
            (7, 11),
            (5, 14),
            (3, 14),
            (1, 11),
            (1, 9),
            (3, 6),
            (5, 6),
            (7, 8),
        ]],
        ':' => &[&[(4, 9), (4, 8)], &[(4, 1), (4, 0)]],
        ';' => &[&[(4, 9), (4, 8)], &[(5, 1), (4, 0), (3, -2)]],
        '<' => &[&[(7, 12), (1, 7), (7, 2)]],
        '=' => &[&[(1, 9), (7, 9)], &[(1, 5), (7, 5)]],
        '>' => &[&[(1, 12), (7, 7), (1, 2)]],
        '?' => &[
            &[
                (1, 11),
                (2, 13),
                (4, 14),
                (6, 13),
                (7, 11),
                (7, 9),
                (4, 6),
                (4, 4),
            ],
            &[(4, 1), (4, 0)],
        ],
        '@' => &[&[
            (6, 5),
            (4, 4),
            (3, 6),
            (4, 8),
            (6, 8),
            (6, 4),
            (7, 3),
            (5, 1),
            (2, 2),
            (1, 5),
            (2, 11),
            (5, 14),
            (7, 12),
        ]],
        'A' => &[&[(1, 0), (4, 14), (7, 0)], &[(2, 5), (6, 5)]],
        'B' => &[
            &[(1, 0), (1, 14), (5, 14), (7, 12), (7, 9), (5, 7), (1, 7)],
            &[(5, 7), (7, 5), (7, 2), (5, 0), (1, 0)],
        ],
        'C' => &[&[
            (7, 12),
            (5, 14),
            (3, 14),
            (1, 11),
            (1, 3),
            (3, 0),
            (5, 0),
            (7, 2),
        ]],
        'D' => &[&[(1, 0), (1, 14), (4, 14), (7, 11), (7, 3), (4, 0), (1, 0)]],
        'E' => &[&[(7, 14), (1, 14), (1, 0), (7, 0)], &[(1, 7), (5, 7)]],
        'F' => &[&[(7, 14), (1, 14), (1, 0)], &[(1, 7), (5, 7)]],
        'G' => &[&[
            (7, 12),
            (5, 14),
            (3, 14),
            (1, 11),
            (1, 3),
            (3, 0),
            (5, 0),
            (7, 2),
            (7, 6),
            (4, 6),
        ]],
        'H' => &[&[(1, 0), (1, 14)], &[(7, 0), (7, 14)], &[(1, 7), (7, 7)]],
        'I' => &[&[(2, 14), (6, 14)], &[(4, 14), (4, 0)], &[(2, 0), (6, 0)]],
        'J' => &[&[(6, 14), (6, 3), (4, 0), (2, 0), (1, 3)]],
        'K' => &[&[(1, 0), (1, 14)], &[(7, 14), (1, 6)], &[(3, 8), (7, 0)]],
        'L' => &[&[(1, 14), (1, 0), (7, 0)]],
        'M' => &[&[(1, 0), (1, 14), (4, 7), (7, 14), (7, 0)]],
        'N' => &[&[(1, 0), (1, 14), (7, 0), (7, 14)]],
        'O' => &[&[
            (1, 4),
            (1, 10),
            (3, 14),
            (5, 14),
            (7, 10),
            (7, 4),
            (5, 0),
            (3, 0),
            (1, 4),
        ]],
        'P' => &[&[(1, 0), (1, 14), (5, 14), (7, 12), (7, 9), (5, 7), (1, 7)]],
        'Q' => &[
            &[
                (1, 4),
                (1, 10),
                (3, 14),
                (5, 14),
                (7, 10),
                (7, 4),
                (5, 0),
                (3, 0),
                (1, 4),
            ],
            &[(5, 3), (7, -1)],
        ],
        'R' => &[
            &[(1, 0), (1, 14), (5, 14), (7, 12), (7, 9), (5, 7), (1, 7)],
            &[(4, 7), (7, 0)],
        ],
        'S' => &[&[
            (7, 12),
            (5, 14),
            (3, 14),
            (1, 12),
            (1, 9),
            (7, 5),
            (7, 2),
            (5, 0),
            (3, 0),
            (1, 2),
        ]],
        'T' => &[&[(1, 14), (7, 14)], &[(4, 14), (4, 0)]],
        'U' => &[&[(1, 14), (1, 3), (3, 0), (5, 0), (7, 3), (7, 14)]],
        'V' => &[&[(1, 14), (4, 0), (7, 14)]],
        'W' => &[&[(1, 14), (2, 0), (4, 8), (6, 0), (7, 14)]],
        'X' => &[&[(1, 14), (7, 0)], &[(7, 14), (1, 0)]],
        'Y' => &[&[(1, 14), (4, 7), (7, 14)], &[(4, 7), (4, 0)]],
        'Z' => &[&[(1, 14), (7, 14), (1, 0), (7, 0)]],
        '[' => &[&[(6, 15), (3, 15), (3, -1), (6, -1)]],
        '\\' => &[&[(1, 15), (7, -1)]],
        ']' => &[&[(2, 15), (5, 15), (5, -1), (2, -1)]],
        '^' => &[&[(2, 11), (4, 14), (6, 11)]],
        '_' => &[&[(1, -2), (7, -2)]],
        '`' => &[&[(3, 14), (5, 12)]],
        'a' => &[
            &[(1, 9), (3, 10), (5, 10), (7, 8), (7, 0)],
            &[(7, 6), (3, 5), (1, 3), (2, 1), (5, 0), (7, 2)],
        ],
        'b' => &[
            &[(1, 14), (1, 0)],
            &[
                (1, 8),
                (3, 10),
                (5, 10),
                (7, 8),
                (7, 2),
                (5, 0),
                (3, 0),
                (1, 2),
            ],
        ],
        'c' => &[&[
            (7, 8),
            (5, 10),
            (3, 10),
            (1, 8),
            (1, 2),
            (3, 0),
            (5, 0),
            (7, 2),
        ]],
        'd' => &[
            &[(7, 14), (7, 0)],
            &[
                (7, 8),
                (5, 10),
                (3, 10),
                (1, 8),
                (1, 2),
                (3, 0),
                (5, 0),
                (7, 2),
            ],
        ],
        'e' => &[&[
            (1, 5),
            (7, 5),
            (7, 8),
            (5, 10),
            (3, 10),
            (1, 8),
            (1, 2),
            (3, 0),
            (6, 0),
            (7, 1),
        ]],
        'f' => &[&[(6, 14), (4, 14), (3, 12), (3, 0)], &[(1, 9), (6, 9)]],
        'g' => &[
            &[(7, 10), (7, -2), (5, -4), (2, -4), (1, -2)],
            &[
                (7, 8),
                (5, 10),
                (3, 10),
                (1, 8),
                (1, 4),
                (3, 2),
                (5, 2),
                (7, 4),
            ],
        ],
        'h' => &[
            &[(1, 14), (1, 0)],
            &[(1, 8), (3, 10), (5, 10), (7, 8), (7, 0)],
        ],
        'i' => &[&[(4, 13), (4, 12)], &[(4, 10), (4, 0)]],
        'j' => &[&[(5, 13), (5, 12)], &[(5, 10), (5, -2), (3, -4), (2, -3)]],
        'k' => &[&[(1, 14), (1, 0)], &[(7, 10), (1, 4)], &[(3, 6), (7, 0)]],
        'l' => &[&[(3, 14), (3, 2), (5, 0)]],
        'm' => &[
            &[(1, 10), (1, 0)],
            &[(1, 8), (2, 10), (3, 10), (4, 8), (4, 0)],
            &[(4, 8), (5, 10), (6, 10), (7, 8), (7, 0)],
        ],
        'n' => &[
            &[(1, 10), (1, 0)],
            &[(1, 8), (3, 10), (5, 10), (7, 8), (7, 0)],
        ],
        'o' => &[&[
            (1, 3),
            (1, 7),
            (3, 10),
            (5, 10),
            (7, 7),
            (7, 3),
            (5, 0),
            (3, 0),
            (1, 3),
        ]],
        'p' => &[
            &[(1, 10), (1, -4)],
            &[
                (1, 8),
                (3, 10),
                (5, 10),
                (7, 8),
                (7, 2),
                (5, 0),
                (3, 0),
                (1, 2),
            ],
        ],
        'q' => &[
            &[(7, 10), (7, -4)],
            &[
                (7, 8),
                (5, 10),
                (3, 10),
                (1, 8),
                (1, 2),
                (3, 0),
                (5, 0),
                (7, 2),
            ],
        ],
        'r' => &[&[(2, 10), (2, 0)], &[(2, 7), (4, 10), (6, 10), (7, 9)]],
        's' => &[&[
            (7, 9),
            (5, 10),
            (2, 10),
            (1, 8),
            (7, 4),
            (6, 1),
            (3, 0),
            (1, 1),
        ]],
        't' => &[&[(3, 14), (3, 2), (5, 0), (6, 1)], &[(1, 10), (6, 10)]],
        'u' => &[
            &[(1, 10), (1, 2), (3, 0), (5, 0), (7, 2)],
            &[(7, 10), (7, 0)],
        ],
        'v' => &[&[(1, 10), (4, 0), (7, 10)]],
        'w' => &[&[(1, 10), (2, 0), (4, 6), (6, 0), (7, 10)]],
        'x' => &[&[(1, 10), (7, 0)], &[(7, 10), (1, 0)]],
        'y' => &[&[(1, 10), (4, 1)], &[(7, 10), (3, -4), (1, -4)]],
        'z' => &[&[(1, 10), (7, 10), (1, 0), (7, 0)]],
        '{' => &[&[(6, 15), (4, 13), (4, 9), (2, 7), (4, 5), (4, 1), (6, -1)]],
        '|' => &[&[(4, 15), (4, -1)]],
        '}' => &[&[(2, 15), (4, 13), (4, 9), (6, 7), (4, 5), (4, 1), (2, -1)]],
        '~' => &[&[(1, 7), (3, 9), (5, 5), (7, 7)]],
        '\u{00B0}' => &[&[
            (3, 11),
            (2, 12),
            (3, 14),
            (5, 14),
            (6, 12),
            (5, 11),
            (3, 11),
        ]],
        '\u{00B7}' => &[&[(4, 6), (4, 5)]],
        '\u{00D7}' => &[&[(2, 10), (6, 4)], &[(6, 10), (2, 4)]],
        _ => TOFU,
    }
}

/// Width, in glyph grid units, of `text` rendered in this monospaced font.
pub fn text_width_units(text: &str) -> f64 {
    text.chars().count() as f64 * ADVANCE
}

/// Converts `text` into polylines in a local coordinate system whose origin is
/// the text's baseline start, in units where 1.0 equals one glyph grid unit.
///
/// The caller scales and translates the result; `super::layout` does that at a
/// scale of `font_size / CAP_HEIGHT`, so a "12 pt" label has 12 pt cap height.
pub fn text_polylines(text: &str) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    let mut pen_x = 0.0_f64;
    for ch in text.chars() {
        for stroke in glyph(ch) {
            out.push(
                stroke
                    .iter()
                    .map(|(x, y)| [pen_x + f64::from(*x), f64::from(*y)])
                    .collect(),
            );
        }
        pen_x += ADVANCE;
    }
    out
}

/// Checks that the font table has no glyph escaping the declared bounding box.
///
/// # Methodology
///
/// Walks every printable ASCII character plus the three non-ASCII additions,
/// fetches its polylines, and asserts every vertex satisfies
/// `0 <= x <= 8` and `DESCENDER - 1 <= y <= CAP_HEIGHT + 2` (the slack on `y`
/// covers ascenders on parentheses and braces, which deliberately overshoot the
/// cap height, and the underscore, which sits just below the descender line).
/// A glyph outside the box would collide with its neighbour or clip against a
/// plot frame.
///
/// # Result
///
/// Passes as of 2026-08-20 for all 95 printable ASCII characters and for `°`,
/// `·` and `×`.
#[cfg(test)]
#[test]
fn every_glyph_stays_inside_its_box() {
    let mut chars: Vec<char> = (0x20u8..=0x7Eu8).map(char::from).collect();
    chars.extend(['\u{00B0}', '\u{00B7}', '\u{00D7}']);
    for ch in chars {
        for stroke in glyph(ch) {
            for (x, y) in *stroke {
                let (x, y) = (f64::from(*x), f64::from(*y));
                assert!((0.0..=8.0).contains(&x), "glyph {ch:?} x = {x} out of box");
                assert!(
                    y >= DESCENDER - 1.0 && y <= CAP_HEIGHT + 2.0,
                    "glyph {ch:?} y = {y} out of box"
                );
            }
        }
    }
}

/// Checks that no printable ASCII character falls through to the missing-glyph
/// box.
///
/// # Methodology
///
/// Compares each character's polylines against [`TOFU`]'s. The space character
/// is exempt (it is legitimately empty).
///
/// # Result
///
/// Passes as of 2026-08-20: full printable-ASCII coverage, no tofu.
#[cfg(test)]
#[test]
fn printable_ascii_is_fully_covered() {
    for ch in (0x21u8..=0x7Eu8).map(char::from) {
        let g = glyph(ch);
        assert!(
            g != TOFU,
            "no glyph for printable ASCII {ch:?} — it would render as a box"
        );
    }
}
