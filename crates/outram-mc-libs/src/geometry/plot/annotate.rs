// SPDX-License-Identifier: GPL-3.0

//! **Legend, title and dimensioned axes for a plot image** — NEW WORK, no
//! OpenMC counterpart.
//!
//! OpenMC's images are bare rasters: no legend, no axes. The crate's
//! geometry-drawing HARD RULE (`crates/outram-mc-libs/CLAUDE.md`) asks for
//! images a human can check *without* the input deck beside them — "colour by
//! material, with a legend and the key dimensions marked". This module frames
//! an already-rendered image with:
//!
//! - a title line;
//! - tick marks and coordinate labels in cm along the bottom and left edges of
//!   a slice ([`annotate_slice`]), derived from the slice's own origin, width
//!   and basis — so the numbers are the geometry's, not a caption typed by hand;
//! - a legend panel mapping each colour to a label.
//!
//! The raster inside the frame is copied unchanged, so an annotated image
//! still carries the exact OpenMC-parity pixels; parity tests compare the
//! *unannotated* image.
//!
//! Text is drawn with a built-in 5x7 bitmap font (upper-case letters, digits
//! and common punctuation; lower case is drawn as upper case) so no font file
//! or text-rendering dependency is needed.

use super::colour::{Rgb, BLACK, WHITE};
use super::image::ImageData;
use super::slice::SlicePlot;

/// One legend row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendEntry {
    /// Swatch colour.
    pub colour: Rgb,
    /// Label text.
    pub label: String,
}

impl LegendEntry {
    /// A legend row.
    #[must_use]
    pub fn new(colour: Rgb, label: impl Into<String>) -> Self {
        Self {
            colour,
            label: label.into(),
        }
    }
}

/// Glyph rows, top to bottom, 5 bits each (bit 4 = leftmost column).
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        ' ' => [0; 7],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x11, 0x1F, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        ',' => [0, 0, 0, 0, 0x0C, 0x04, 0x08],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        '+' => [0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0],
        ':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '/' => [0, 0x01, 0x02, 0x04, 0x08, 0x10, 0],
        '=' => [0, 0, 0x1F, 0, 0x1F, 0, 0],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '_' => [0, 0, 0, 0, 0, 0, 0x1F],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '\'' => [0x0C, 0x04, 0x08, 0, 0, 0, 0],
        '*' => [0, 0x04, 0x15, 0x0E, 0x15, 0x04, 0],
        _ => [0x0E, 0x11, 0x01, 0x02, 0x04, 0, 0x04], // '?'
    }
}

/// Pixel scale of the font: each glyph cell is `5*S` wide, `7*S` tall.
const S: usize = 2;
/// Horizontal advance per character.
const ADVANCE: usize = 6 * S;
/// Line height.
const LINE: usize = 7 * S + 6;

/// Width in pixels of `text` drawn with [`draw_text`].
#[must_use]
pub fn text_width(text: &str) -> usize {
    text.chars().count() * ADVANCE
}

/// Draw `text` with its top-left corner at `(x, y)`; pixels off the image are
/// dropped.
pub fn draw_text(img: &mut ImageData, x: i64, y: i64, text: &str, colour: Rgb) {
    for (k, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        let x0 = x + (k * ADVANCE) as i64;
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) == 0 {
                    continue;
                }
                for dy in 0..S {
                    for dx in 0..S {
                        let px = x0 + (col * S + dx) as i64;
                        let py = y + (row * S + dy) as i64;
                        if px >= 0
                            && py >= 0
                            && (px as usize) < img.width
                            && (py as usize) < img.height
                        {
                            img.set(px as usize, py as usize, colour);
                        }
                    }
                }
            }
        }
    }
}

fn fill_rect(img: &mut ImageData, x0: usize, y0: usize, w: usize, h: usize, c: Rgb) {
    for y in y0..(y0 + h).min(img.height) {
        for x in x0..(x0 + w).min(img.width) {
            img.set(x, y, c);
        }
    }
}

/// A "nice" tick step (1, 2 or 5 times a power of ten) giving at most
/// `max_ticks` intervals over `span`.
fn nice_step(span: f64, max_ticks: usize) -> f64 {
    let raw = span / max_ticks.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if m * mag >= raw {
            return m * mag;
        }
    }
    10.0 * mag
}

fn fmt_tick(v: f64, step: f64) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    };
    let v = if v.abs() < step * 1e-9 { 0.0 } else { v };
    format!("{v:.decimals$}")
}

fn legend_size(legend: &[LegendEntry]) -> (usize, usize) {
    if legend.is_empty() {
        return (0, 0);
    }
    let w = legend
        .iter()
        .map(|e| text_width(&e.label))
        .max()
        .unwrap_or(0)
        + 7 * S
        + 30;
    (w, legend.len() * LINE + 10)
}

fn draw_legend(img: &mut ImageData, x0: usize, y0: usize, legend: &[LegendEntry]) {
    for (i, e) in legend.iter().enumerate() {
        let y = y0 + i * LINE;
        fill_rect(img, x0, y, 7 * S + 2, 7 * S + 2, BLACK);
        fill_rect(img, x0 + 1, y + 1, 7 * S, 7 * S, e.colour);
        draw_text(
            img,
            (x0 + 7 * S + 10) as i64,
            (y + 1) as i64,
            &e.label,
            BLACK,
        );
    }
}

/// Frame `image` with a title and a legend (no axes) — for ray-traced views.
#[must_use]
pub fn annotate_image(image: &ImageData, title: &str, legend: &[LegendEntry]) -> ImageData {
    let pad = 10;
    let top = LINE + pad;
    let (lw, lh) = legend_size(legend);
    let w = pad + image.width + if lw > 0 { pad + lw } else { 0 } + pad;
    let w = w.max(text_width(title) + 2 * pad);
    let h = top + image.height.max(lh) + pad;
    let mut out = ImageData::filled(w, h, WHITE);
    draw_text(&mut out, pad as i64, (pad / 2) as i64, title, BLACK);
    blit(&mut out, image, pad, top);
    draw_legend(&mut out, pad + image.width + pad, top, legend);
    out
}

fn blit(out: &mut ImageData, image: &ImageData, x0: usize, y0: usize) {
    for y in 0..image.height {
        for x in 0..image.width {
            out.set(x0 + x, y0 + y, image.get(x, y));
        }
    }
}

/// Frame a slice image with a title, a legend, and tick marks labelled in cm
/// along the bottom (horizontal axis) and left (vertical axis) edges.
///
/// The tick values come from `plot`'s origin, width and basis, using the same
/// pixel-to-coordinate map as [`SlicePlot::pixel_centre`], so a tick marks the
/// pixel whose centre is nearest that coordinate. Axis names follow the basis
/// (`X`, `Y` or `Z`). `image` must be `plot`'s own image (same pixel size).
#[must_use]
pub fn annotate_slice(
    image: &ImageData,
    plot: &SlicePlot,
    title: &str,
    legend: &[LegendEntry],
) -> ImageData {
    let (ax1, ax2) = plot.basis.axes();
    let axis_name = |a: usize| ["X", "Y", "Z"][a];
    let o = [plot.origin.x, plot.origin.y, plot.origin.z];
    let (lo1, hi1) = (o[ax1] - plot.width[0] / 2.0, o[ax1] + plot.width[0] / 2.0);
    let (lo2, hi2) = (o[ax2] - plot.width[1] / 2.0, o[ax2] + plot.width[1] / 2.0);
    let step1 = nice_step(hi1 - lo1, (image.width / 70).clamp(2, 10));
    let step2 = nice_step(hi2 - lo2, (image.height / 50).clamp(2, 12));
    let ticks = |lo: f64, hi: f64, step: f64| -> Vec<f64> {
        let mut v = Vec::new();
        let mut t = (lo / step).ceil() * step;
        while t <= hi + step * 1e-9 {
            v.push(t);
            t += step;
        }
        v
    };
    let t1 = ticks(lo1, hi1, step1);
    let t2 = ticks(lo2, hi2, step2);
    let ylabel_w = t2
        .iter()
        .map(|&v| text_width(&fmt_tick(v, step2)))
        .max()
        .unwrap_or(0);

    let pad = 10;
    let tick = 6;
    let left = pad + ylabel_w + 4 + tick;
    let top = LINE + pad;
    let bottom = tick + 4 + LINE + LINE;
    let (lw, lh) = legend_size(legend);
    let w = left + image.width + pad + if lw > 0 { lw + pad } else { 0 };
    let w = w.max(text_width(title) + 2 * pad);
    let h = top + image.height.max(lh) + bottom;
    let mut out = ImageData::filled(w, h, WHITE);
    draw_text(&mut out, pad as i64, (pad / 2) as i64, title, BLACK);
    blit(&mut out, image, left, top);
    // Frame.
    let (fx0, fy0, fx1, fy1) = (left - 1, top - 1, left + image.width, top + image.height);
    for x in fx0..=fx1 {
        out.set(x, fy0, BLACK);
        out.set(x, fy1, BLACK);
    }
    for y in fy0..=fy1 {
        out.set(fx0, y, BLACK);
        out.set(fx1, y, BLACK);
    }
    // Horizontal-axis ticks: pixel x whose centre is at coordinate v.
    for &v in &t1 {
        let fx = (v - lo1) / (hi1 - lo1) * image.width as f64 - 0.5;
        let x = left + (fx.round().clamp(0.0, (image.width - 1) as f64)) as usize;
        for d in 1..=tick {
            out.set(x, fy1 + d, BLACK);
        }
        let s = fmt_tick(v, step1);
        draw_text(
            &mut out,
            x as i64 - (text_width(&s) / 2) as i64,
            (fy1 + tick + 4) as i64,
            &s,
            BLACK,
        );
    }
    // Vertical-axis ticks: row 0 is the top (highest coordinate).
    for &v in &t2 {
        let fy = (hi2 - v) / (hi2 - lo2) * image.height as f64 - 0.5;
        let y = top + (fy.round().clamp(0.0, (image.height - 1) as f64)) as usize;
        for d in 1..=tick {
            out.set(fx0 - d, y, BLACK);
        }
        let s = fmt_tick(v, step2);
        draw_text(
            &mut out,
            (fx0 - tick - 3 - text_width(&s)) as i64,
            y as i64 - (7 * S / 2) as i64,
            &s,
            BLACK,
        );
    }
    let caption = format!(
        "{} [CM] HORIZONTAL, {} [CM] VERTICAL, {} = {:.4} CM",
        axis_name(ax1),
        axis_name(ax2),
        axis_name(3 - ax1 - ax2),
        o[3 - ax1 - ax2]
    );
    draw_text(
        &mut out,
        pad as i64,
        (fy1 + tick + 4 + LINE) as i64,
        &caption,
        BLACK,
    );
    draw_legend(&mut out, left + image.width + pad, top, legend);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(380.0, 8), 50.0);
        assert_eq!(nice_step(6.4, 5), 2.0);
        assert!((nice_step(0.9, 5) - 0.2).abs() < 1e-12);
        assert_eq!(fmt_tick(-2.0, 2.0), "-2");
        assert_eq!(fmt_tick(0.4, 0.2), "0.4");
    }

    /// The annotated image carries the raster unchanged.
    #[test]
    fn annotation_preserves_the_raster() {
        use crate::geometry::plot::slice::PlotBasis;
        use crate::geometry::position::Position;
        let mut img = ImageData::filled(50, 40, Rgb::new(1, 2, 3));
        img.set(7, 9, Rgb::new(9, 9, 9));
        let plot = SlicePlot::new(PlotBasis::Xz, Position::ZERO, [5.0, 4.0], [50, 40]);
        let out = annotate_slice(
            &img,
            &plot,
            "TEST",
            &[LegendEntry::new(Rgb::new(9, 9, 9), "NINE")],
        );
        let found = (0..out.height).any(|y0| {
            (0..out.width).any(|x0| {
                x0 + 50 <= out.width
                    && y0 + 40 <= out.height
                    && (0..40).all(|y| (0..50).all(|x| out.get(x0 + x, y0 + y) == img.get(x, y)))
            })
        });
        assert!(found, "raster not copied verbatim into the annotated frame");
    }
}
