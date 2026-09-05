//! PNG backend — an anti-aliased software rasteriser over the same draw list.
//!
//! # Why rasterise here instead of screenshotting the GUI
//!
//! A framebuffer grab of `egui_plot` would depend on the window size, the
//! display scale factor and the GPU backend, and cannot run at all without a
//! display. This path is a pure function of the [`Scene`], runs headless, and
//! produces exactly the figure the PDF and SVG backends produce — so a PNG
//! preview is a faithful preview of the PDF that goes into a paper.
//!
//! # Anti-aliasing
//!
//! Strokes use analytic coverage: a pixel's coverage is
//! `clamp(half_width + 0.5 - distance_to_segment, 0, 1)`, which gives round
//! caps and round joins for free (consecutive segments simply overlap) and
//! matches the round line caps the SVG and PDF backends ask for. Fills use 4x4
//! supersampling inside the polygon's bounding box, which is cheap because the
//! only filled shapes are markers, the legend panel and the page ground.
//!
//! Compositing is `dst = src * coverage + dst * (1 - coverage)` in 8-bit sRGB.
//! That is not gamma-correct blending; it is what essentially every 2-D
//! rasteriser does, and at these stroke widths the difference is invisible.

use super::layout::{DrawOp, PageSize};
use super::{FigurePalette, Rgb, Scene};

/// Default output resolution, in pixels per PostScript point. 2.0 gives 144 dpi
/// — a 720 x 540 pt figure becomes 1440 x 1080 px, which is a sensible default
/// for a slide or a screen preview. Raise it for a print figure.
pub const DEFAULT_PIXELS_PER_POINT: f64 = 2.0;

/// Renders a scene to PNG bytes, in `palette`'s colours.
///
/// `pixels_per_point` scales the page: 1.0 is 72 dpi, 2.0 is 144 dpi, and so
/// on. It is clamped to a sane band so a stray value cannot ask for a
/// multi-gigabyte allocation.
pub fn render(
    scene: &Scene,
    page: PageSize,
    pixels_per_point: f64,
    palette: FigurePalette,
) -> Result<Vec<u8>, String> {
    let ops = super::layout::to_draw_ops(scene, page, palette);
    render_ops(&ops, page, pixels_per_point, palette.background)
}

/// Rasterises an already-laid-out draw list. `background` seeds the canvas
/// before any op is drawn, so a partially-covered pixel at an anti-aliased
/// page edge blends toward the right colour instead of toward white.
pub fn render_ops(
    ops: &[DrawOp],
    page: PageSize,
    pixels_per_point: f64,
    background: Rgb,
) -> Result<Vec<u8>, String> {
    let scale = pixels_per_point.clamp(0.5, 8.0);
    let width = (page.width_pt * scale).round().max(1.0) as usize;
    let height = (page.height_pt * scale).round().max(1.0) as usize;

    let mut canvas = Canvas::new(width, height, background);
    for op in ops {
        match op {
            DrawOp::Polygon { points, colour } => {
                let scaled: Vec<[f64; 2]> = points
                    .iter()
                    .map(|p| [p[0] * scale, p[1] * scale])
                    .collect();
                canvas.fill_polygon(&scaled, *colour);
            }
            DrawOp::Polyline {
                points,
                width: stroke,
                colour,
                dash,
            } => {
                let scaled: Vec<[f64; 2]> = points
                    .iter()
                    .map(|p| [p[0] * scale, p[1] * scale])
                    .collect();
                let w = (stroke * scale).max(0.6);
                match dash {
                    None => canvas.stroke_polyline(&scaled, w, *colour),
                    Some((on, off)) => {
                        for piece in dash_polyline(&scaled, on * scale, off * scale) {
                            canvas.stroke_polyline(&piece, w, *colour);
                        }
                    }
                }
            }
        }
    }
    canvas.encode_png()
}

/// An 8-bit sRGB canvas.
struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: usize, height: usize, ground: Rgb) -> Self {
        let mut pixels = Vec::with_capacity(width * height * 3);
        for _ in 0..width * height {
            pixels.extend_from_slice(&[ground.r, ground.g, ground.b]);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Blends `colour` into pixel `(x, y)` with the given coverage.
    fn blend(&mut self, x: usize, y: usize, colour: Rgb, coverage: f64) {
        if coverage <= 0.0 || x >= self.width || y >= self.height {
            return;
        }
        let a = coverage.min(1.0);
        let i = (y * self.width + x) * 3;
        for (channel, src) in [colour.r, colour.g, colour.b].into_iter().enumerate() {
            let dst = f64::from(self.pixels[i + channel]);
            let blended = f64::from(src) * a + dst * (1.0 - a);
            self.pixels[i + channel] = blended.round().clamp(0.0, 255.0) as u8;
        }
    }

    /// Strokes a polyline with analytic coverage anti-aliasing.
    fn stroke_polyline(&mut self, points: &[[f64; 2]], width: f64, colour: Rgb) {
        if points.len() < 2 {
            return;
        }
        let half = width * 0.5;
        for segment in points.windows(2) {
            let (a, b) = (segment[0], segment[1]);
            let pad = half + 1.5;
            let x0 = (a[0].min(b[0]) - pad).floor().max(0.0) as usize;
            let x1 = (a[0].max(b[0]) + pad).ceil().min(self.width as f64) as usize;
            let y0 = (a[1].min(b[1]) - pad).floor().max(0.0) as usize;
            let y1 = (a[1].max(b[1]) + pad).ceil().min(self.height as f64) as usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    let px = [x as f64 + 0.5, y as f64 + 0.5];
                    let d = distance_to_segment(px, a, b);
                    let coverage = (half + 0.5 - d).clamp(0.0, 1.0);
                    self.blend(x, y, colour, coverage);
                }
            }
        }
    }

    /// Fills a polygon with 4x4 supersampled coverage, even-odd rule.
    fn fill_polygon(&mut self, points: &[[f64; 2]], colour: Rgb) {
        if points.len() < 3 {
            return;
        }
        let min_x = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let max_y = points
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        if !(min_x.is_finite() && max_x.is_finite() && min_y.is_finite() && max_y.is_finite()) {
            return;
        }
        let x0 = min_x.floor().max(0.0) as usize;
        let x1 = (max_x.ceil() + 1.0).min(self.width as f64).max(0.0) as usize;
        let y0 = min_y.floor().max(0.0) as usize;
        let y1 = (max_y.ceil() + 1.0).min(self.height as f64).max(0.0) as usize;
        const SUB: usize = 4;
        for y in y0..y1 {
            for x in x0..x1 {
                let mut hits = 0;
                for sy in 0..SUB {
                    for sx in 0..SUB {
                        let p = [
                            x as f64 + (sx as f64 + 0.5) / SUB as f64,
                            y as f64 + (sy as f64 + 0.5) / SUB as f64,
                        ];
                        if point_in_polygon(p, points) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    self.blend(x, y, colour, hits as f64 / (SUB * SUB) as f64);
                }
            }
        }
    }

    /// Encodes the canvas as a PNG.
    fn encode_png(&self) -> Result<Vec<u8>, String> {
        let image =
            image::RgbImage::from_raw(self.width as u32, self.height as u32, self.pixels.clone())
                .ok_or_else(|| {
                "raster buffer size does not match its declared dimensions".to_string()
            })?;
        let mut out = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        Ok(out)
    }
}

/// Perpendicular distance from `p` to the segment `a`–`b`, with the endpoints
/// treated as caps (which is what makes the stroke ends round).
fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let (wx, wy) = (p[0] - a[0], p[1] - a[1]);
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= f64::EPSILON {
        0.0
    } else {
        ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0)
    };
    let (dx, dy) = (wx - t * vx, wy - t * vy);
    (dx * dx + dy * dy).sqrt()
}

/// Even-odd point-in-polygon test.
fn point_in_polygon(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (yi, yj) = (poly[i][1], poly[j][1]);
        if (yi > p[1]) != (yj > p[1]) {
            let x_cross = poly[i][0] + (p[1] - yi) / (yj - yi) * (poly[j][0] - poly[i][0]);
            if p[0] < x_cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Splits a polyline into the "on" pieces of a dash pattern, walking arclength
/// continuously across vertices so the pattern does not restart at every joint.
fn dash_polyline(points: &[[f64; 2]], on: f64, off: f64) -> Vec<Vec<[f64; 2]>> {
    let period = on + off;
    if points.len() < 2 || !(period > 0.0) {
        return vec![points.to_vec()];
    }
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    let mut travelled = 0.0_f64;
    for segment in points.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if length <= f64::EPSILON {
            continue;
        }
        let steps = (length / (period * 0.25)).ceil().max(1.0) as usize;
        let mut previous_on = (travelled % period) < on;
        if previous_on && current.is_empty() {
            current.push(a);
        }
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            let here = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let now_on = ((travelled + length * t) % period) < on;
            if now_on {
                if !previous_on {
                    current.clear();
                }
                current.push(here);
            } else if previous_on {
                current.push(here);
                if current.len() >= 2 {
                    out.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            previous_on = now_on;
        }
        travelled += length;
    }
    if current.len() >= 2 {
        out.push(current);
    }
    out
}

/// Checks that the rasteriser actually draws, and that its output decodes.
///
/// # Methodology
///
/// Renders a scene containing a single thick black diagonal on white paper at
/// 1 px/pt, decodes the PNG back with the `image` crate, and asserts: the
/// decoded dimensions equal the requested page size; at least one pixel is
/// substantially darker than paper (so the draw list really reached the
/// canvas); and the corner pixel farthest from any drawing is still paper
/// white (so the rasteriser is not smearing).
///
/// # Result
///
/// Passes as of 2026-08-20: 720 x 540 px decoded, dark pixels present, top-left
/// corner exactly `(255, 255, 255)`.
#[cfg(test)]
#[test]
fn raster_output_decodes_and_contains_ink() {
    use super::{Series, SeriesStyle, INK};
    let mut scene = Scene::new("Raster gate", "x", "y");
    scene.series.push(Series {
        name: "diag".into(),
        style: SeriesStyle::Line {
            width: 3.0,
            dash: None,
        },
        colour: INK,
        points: vec![[0.05, 0.05], [0.95, 0.95]],
        show_in_legend: false,
    });
    let bytes = render(
        &scene,
        PageSize::DEFAULT,
        1.0,
        FigurePalette::LIGHT_PUBLICATION,
    )
    .expect("render succeeds");
    let decoded = image::load_from_memory(&bytes)
        .expect("PNG decodes")
        .to_rgb8();
    assert_eq!(decoded.width(), 720);
    assert_eq!(decoded.height(), 540);
    let dark = decoded.pixels().filter(|p| p.0[0] < 128).count();
    assert!(
        dark > 100,
        "expected ink on the canvas, found {dark} dark pixels"
    );
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 255, 255]);
}

/// Checks that dashing splits a line without losing or duplicating it.
///
/// # Methodology
///
/// Dashes a straight 100 pt line with a 10 pt on / 10 pt off pattern and
/// asserts that between 4 and 6 pieces come out (the exact count depends on
/// where the final partial period lands) and that every piece lies on the
/// original line.
///
/// # Result
///
/// Passes as of 2026-08-20: 5 pieces, all collinear with the source.
#[cfg(test)]
#[test]
fn dashing_splits_a_line_into_on_pieces() {
    let pieces = dash_polyline(&[[0.0, 0.0], [100.0, 0.0]], 10.0, 10.0);
    assert!(
        (4..=6).contains(&pieces.len()),
        "expected about five dashes, got {}",
        pieces.len()
    );
    for piece in &pieces {
        for point in piece {
            assert!(point[1].abs() < 1e-9, "dash left the source line");
            assert!((0.0..=100.0).contains(&point[0]));
        }
    }
}
