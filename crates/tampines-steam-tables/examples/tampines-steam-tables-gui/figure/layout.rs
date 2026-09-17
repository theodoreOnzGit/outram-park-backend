//! Turns a [`Scene`] into page-space geometry.
//!
//! # Coordinate system
//!
//! Page space has its origin at the **top left**, x to the right and y
//! **downward**, in PostScript points (1/72 inch) — the same convention SVG and
//! raster images use. The PDF backend is the only one that flips, because PDF
//! puts its origin at the bottom left.
//!
//! # What comes out
//!
//! Only two primitives: a stroked [`DrawOp::Polyline`] and a filled
//! [`DrawOp::Polygon`]. Axis frames, tick marks, every piece of text, every
//! marker and the legend are all reduced to those two here, so a backend is
//! nothing more than a serialiser. Text becomes polylines via
//! [`super::font`].
//!
//! # Clipping
//!
//! Data is clipped against the plot rectangle with the Liang–Barsky algorithm,
//! segment by segment, so a curve that leaves the axes is cut at the frame
//! rather than drawn across the margins. `NaN` coordinates break the polyline
//! (the pen-up convention documented on [`Series::points`]), which is how a
//! genuinely discontinuous curve is drawn without inventing a joining segment.

use super::font;
use super::{AxisScale, FigurePalette, MarkerShape, Rgb, Scene, Series, SeriesStyle};

/// Page dimensions in PostScript points (1/72 inch).
#[derive(Clone, Copy, Debug)]
pub struct PageSize {
    /// Page width in points.
    pub width_pt: f64,
    /// Page height in points.
    pub height_pt: f64,
}

impl PageSize {
    /// The default figure size: 10 in x 7.5 in, a 4:3 landscape figure that
    /// drops into a two-column paper at half width without resampling.
    pub const DEFAULT: Self = Self {
        width_pt: 720.0,
        height_pt: 540.0,
    };
}

/// A page-space rectangle, top-left origin.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    /// Left edge.
    pub x0: f64,
    /// Top edge.
    pub y0: f64,
    /// Right edge.
    pub x1: f64,
    /// Bottom edge.
    pub y1: f64,
}

impl Rect {
    /// Width in points.
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }
    /// Height in points.
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
    /// Whether a page-space point is inside (inclusive).
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.x0 && p[0] <= self.x1 && p[1] >= self.y0 && p[1] <= self.y1
    }
}

/// Horizontal text anchoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAnchor {
    /// Anchor at the left edge of the text.
    Start,
    /// Anchor at the horizontal centre.
    Middle,
    /// Anchor at the right edge.
    End,
}

/// Vertical text anchoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAnchor {
    /// Anchor on the text baseline. Nothing in the current layout uses it —
    /// every label is centred or top-anchored — but it is the identity case the
    /// other two are defined against, and omitting it would make the offset
    /// arithmetic in `push_text` harder to check.
    #[allow(dead_code)]
    Baseline,
    /// Anchor at half cap height.
    Middle,
    /// Anchor at the cap height (the visual top).
    Top,
}

/// The only two things a backend has to know how to draw.
#[derive(Clone, Debug)]
pub enum DrawOp {
    /// An open, stroked polyline.
    Polyline {
        /// Vertices in page space.
        points: Vec<[f64; 2]>,
        /// Stroke width in points.
        width: f64,
        /// Stroke colour.
        colour: Rgb,
        /// Dash pattern `(on, off)` in points, `None` for solid.
        dash: Option<(f64, f64)>,
    },
    /// A closed, filled polygon.
    Polygon {
        /// Vertices in page space.
        points: Vec<[f64; 2]>,
        /// Fill colour.
        colour: Rgb,
    },
}

/// Font size, in points, of the figure title.
const TITLE_SIZE: f64 = 15.0;
/// Font size, in points, of axis titles.
const AXIS_LABEL_SIZE: f64 = 12.0;
/// Font size, in points, of tick labels.
const TICK_LABEL_SIZE: f64 = 10.0;
/// Font size, in points, of legend entries and footnotes.
const SMALL_SIZE: f64 = 9.0;
/// Stroke width, in points, of text.
const TEXT_STROKE: f64 = 0.7;
/// Length, in points, of a major tick mark.
const TICK_LEN: f64 = 5.0;

/// Renders `scene` onto a page of the given size, in `palette`'s colours.
///
/// Every plotted series keeps its own colour regardless of `palette` — only
/// the page background, axes, frame, text and grid follow it. Pass
/// [`FigurePalette::LIGHT_PUBLICATION`] for this tool's original, unchanged
/// figure output.
pub fn to_draw_ops(scene: &Scene, page: PageSize, palette: FigurePalette) -> Vec<DrawOp> {
    let mut ops = Vec::new();

    // Page background. Every backend gets an explicit ground rather than
    // relying on a viewer's default, so a figure dropped on a mismatched
    // background (a dark slide behind a light-publication PNG, or vice versa)
    // is still legible.
    ops.push(DrawOp::Polygon {
        points: vec![
            [0.0, 0.0],
            [page.width_pt, 0.0],
            [page.width_pt, page.height_pt],
            [0.0, page.height_pt],
        ],
        colour: palette.background,
    });

    // The bottom of the page is shared, in this order, by the x-axis label, the
    // legend and the footnotes. All three are measured before the plot
    // rectangle is fixed, so nothing is ever clipped off the page — the plot
    // gives up the space, not the text.
    let content_width = page.width_pt - 82.0 - 26.0;
    let note_lines = wrap_notes(&scene.notes, content_width, SMALL_SIZE);
    let note_height = if note_lines.is_empty() {
        0.0
    } else {
        note_lines.len() as f64 * (SMALL_SIZE * 1.45) + 6.0
    };
    let legend = LegendLayout::measure(scene, content_width);
    let plot = Rect {
        x0: 82.0,
        y0: 46.0,
        x1: page.width_pt - 26.0,
        y1: page.height_pt - 40.0 - legend.height - note_height,
    };

    let x_ticks = ticks_for(scene.x_range, scene.x_scale, 9);
    let y_ticks = ticks_for(scene.y_range, scene.y_scale, 8);

    // Grid, under everything.
    for tick in &x_ticks {
        if let Some(px) = map_x(scene, &plot, tick.value) {
            ops.push(DrawOp::Polyline {
                points: vec![[px, plot.y0], [px, plot.y1]],
                width: if tick.major { 0.5 } else { 0.3 },
                colour: palette.grid,
                dash: None,
            });
        }
    }
    for tick in &y_ticks {
        if let Some(py) = map_y(scene, &plot, tick.value) {
            ops.push(DrawOp::Polyline {
                points: vec![[plot.x0, py], [plot.x1, py]],
                width: if tick.major { 0.5 } else { 0.3 },
                colour: palette.grid,
                dash: None,
            });
        }
    }

    // Data.
    for series in &scene.series {
        draw_series(&mut ops, scene, &plot, series);
    }

    // Frame on top of the data, so a curve running along an axis stays readable.
    ops.push(DrawOp::Polyline {
        points: vec![
            [plot.x0, plot.y0],
            [plot.x1, plot.y0],
            [plot.x1, plot.y1],
            [plot.x0, plot.y1],
            [plot.x0, plot.y0],
        ],
        width: 1.0,
        colour: palette.ink,
        dash: None,
    });

    // Ticks and their labels.
    for tick in &x_ticks {
        let Some(px) = map_x(scene, &plot, tick.value) else {
            continue;
        };
        let len = if tick.major { TICK_LEN } else { TICK_LEN * 0.5 };
        ops.push(DrawOp::Polyline {
            points: vec![[px, plot.y1], [px, plot.y1 - len]],
            width: 0.8,
            colour: palette.ink,
            dash: None,
        });
        if tick.major {
            push_text(
                &mut ops,
                &tick.label,
                [px, plot.y1 + 4.0],
                TICK_LABEL_SIZE,
                HAnchor::Middle,
                VAnchor::Top,
                0.0,
                palette.ink,
            );
        }
    }
    for tick in &y_ticks {
        let Some(py) = map_y(scene, &plot, tick.value) else {
            continue;
        };
        let len = if tick.major { TICK_LEN } else { TICK_LEN * 0.5 };
        ops.push(DrawOp::Polyline {
            points: vec![[plot.x0, py], [plot.x0 + len, py]],
            width: 0.8,
            colour: palette.ink,
            dash: None,
        });
        if tick.major {
            push_text(
                &mut ops,
                &tick.label,
                [plot.x0 - 5.0, py],
                TICK_LABEL_SIZE,
                HAnchor::End,
                VAnchor::Middle,
                0.0,
                palette.ink,
            );
        }
    }

    // Titles. The figure titles name the diagram, the formulation and the
    // crate, so they are long; the title shrinks to fit rather than running off
    // the page. It never grows past `TITLE_SIZE`.
    push_text(
        &mut ops,
        &scene.title,
        // Centred on the page, not on the plot rectangle: the plot is offset
        // right by its y-axis label gutter, and centring on it pushes a
        // full-width title off the right edge.
        [page.width_pt * 0.5, 14.0],
        fitted_size(&scene.title, page.width_pt - 24.0, TITLE_SIZE),
        HAnchor::Middle,
        VAnchor::Top,
        0.0,
        palette.ink,
    );
    push_text(
        &mut ops,
        &scene.x_label,
        [(plot.x0 + plot.x1) * 0.5, plot.y1 + 20.0],
        fitted_size(&scene.x_label, plot.width(), AXIS_LABEL_SIZE),
        HAnchor::Middle,
        VAnchor::Top,
        0.0,
        palette.ink,
    );
    // The y-axis title runs bottom-to-top, so the space it has to fit in is the
    // plot's *height*, not its width — sizing it against the width lets it
    // overrun the frame vertically and collide with the figure title.
    push_text(
        &mut ops,
        &scene.y_label,
        [22.0, (plot.y0 + plot.y1) * 0.5],
        fitted_size(&scene.y_label, plot.height(), AXIS_LABEL_SIZE),
        HAnchor::Middle,
        VAnchor::Middle,
        -90.0,
        palette.ink,
    );

    legend.draw(&mut ops, scene, plot.x0, plot.y1 + 34.0, palette.ink);

    // Footnotes, under the legend.
    let mut note_y = plot.y1 + 34.0 + legend.height + 4.0;
    for line in &note_lines {
        push_text(
            &mut ops,
            line,
            [plot.x0, note_y],
            SMALL_SIZE,
            HAnchor::Start,
            VAnchor::Top,
            0.0,
            palette.ink,
        );
        note_y += SMALL_SIZE * 1.45;
    }

    ops
}

/// The largest font size, no bigger than `preferred`, at which `text` fits in
/// `width_pt`.
///
/// The font is monospaced, so this is exact arithmetic rather than a guess.
fn fitted_size(text: &str, width_pt: f64, preferred: f64) -> f64 {
    let units = font::text_width_units(text);
    if units <= 0.0 {
        return preferred;
    }
    preferred.min(width_pt * font::CAP_HEIGHT / units)
}

/// Greedy word wrap for the footnotes.
///
/// The font is monospaced, so the number of characters that fit is exact
/// rather than estimated. A word longer than the line is left to overhang
/// rather than being broken mid-token, since the only long tokens here are
/// formulae and file paths, which are worse to read broken.
fn wrap_notes(notes: &[String], width_pt: f64, size: f64) -> Vec<String> {
    let per_char = font::ADVANCE * (size / font::CAP_HEIGHT);
    let columns = (width_pt / per_char).floor().max(20.0) as usize;
    let mut lines = Vec::new();
    for note in notes {
        let mut current = String::new();
        for word in note.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word.chars().count() <= columns {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}

/// Where the legend goes and how big it is.
///
/// The legend sits **below** the axes, in as many columns as fit. A figure here
/// routinely carries a dozen or more entries — four structural curves, the
/// markers, and up to seven reference datasets — and an in-plot legend box that
/// size covers the data it is describing.
#[derive(Clone, Copy, Debug)]
struct LegendLayout {
    /// Number of entries.
    entries: usize,
    /// Columns across the page.
    columns: usize,
    /// Width of one column, in points.
    column_width: f64,
    /// Height of one row, in points.
    row_height: f64,
    /// Total height reserved, in points.
    height: f64,
}

/// Length of the style swatch drawn beside each legend entry, in points.
const LEGEND_SWATCH: f64 = 18.0;

impl LegendLayout {
    /// Measures the legend for `scene` given the available width.
    fn measure(scene: &Scene, width_pt: f64) -> Self {
        let entries = scene.series.iter().filter(|s| s.show_in_legend).count();
        if entries == 0 {
            return Self {
                entries: 0,
                columns: 1,
                column_width: width_pt,
                row_height: 0.0,
                height: 0.0,
            };
        }
        // Columns are chosen from the entry *count*, not from the widest label:
        // a figure here can carry fourteen entries, one of which is long, and
        // sizing the column to that one label forces a single fourteen-row
        // column that eats half the page. Long labels are truncated to the
        // column instead — the full name is always in the CSV.
        const TARGET_ROWS: usize = 7;
        let columns = entries.div_ceil(TARGET_ROWS).clamp(1, 3);
        let column_width = width_pt / columns as f64;
        let rows = entries.div_ceil(columns);
        let row_height = SMALL_SIZE * 1.55;
        Self {
            entries,
            columns,
            column_width,
            row_height,
            height: rows as f64 * row_height + 6.0,
        }
    }

    /// Draws the legend with its top-left corner at `(x, y)`.
    ///
    /// Only series with `show_in_legend` appear, which is how a fan of 21
    /// Zaloudek quality curves contributes one entry rather than twenty-one.
    fn draw(&self, ops: &mut Vec<DrawOp>, scene: &Scene, x: f64, y: f64, ink: Rgb) {
        if self.entries == 0 {
            return;
        }
        let shown: Vec<&Series> = scene.series.iter().filter(|s| s.show_in_legend).collect();
        let rows = self.entries.div_ceil(self.columns);
        for (index, series) in shown.iter().enumerate() {
            let column = index / rows;
            let row = index % rows;
            let cell_x = x + column as f64 * self.column_width;
            let cell_y = y + (row as f64 + 0.5) * self.row_height;
            match series.style {
                SeriesStyle::Line { width, dash } => ops.push(DrawOp::Polyline {
                    points: vec![[cell_x, cell_y], [cell_x + LEGEND_SWATCH, cell_y]],
                    width,
                    colour: series.colour,
                    dash,
                }),
                SeriesStyle::Markers { shape, size } => push_marker(
                    ops,
                    [cell_x + LEGEND_SWATCH * 0.5, cell_y],
                    shape,
                    size,
                    series.colour,
                ),
            }
            let text_width = self.column_width - LEGEND_SWATCH - 12.0;
            let per_char = font::ADVANCE * (SMALL_SIZE / font::CAP_HEIGHT);
            let budget = (text_width / per_char).floor().max(4.0) as usize;
            let label: String = if series.name.chars().count() > budget {
                series.name.chars().take(budget).collect()
            } else {
                series.name.clone()
            };
            push_text(
                ops,
                &label,
                [cell_x + LEGEND_SWATCH + 6.0, cell_y],
                SMALL_SIZE,
                HAnchor::Start,
                VAnchor::Middle,
                0.0,
                ink,
            );
        }
    }
}

/// Maps one series into page space, clipping to the plot rectangle.
fn draw_series(ops: &mut Vec<DrawOp>, scene: &Scene, plot: &Rect, series: &Series) {
    match series.style {
        SeriesStyle::Line { width, dash } => {
            for run in mapped_runs(scene, plot, series) {
                for piece in clip_polyline(&run, plot) {
                    if piece.len() >= 2 {
                        ops.push(DrawOp::Polyline {
                            points: piece,
                            width,
                            colour: series.colour,
                            dash,
                        });
                    }
                }
            }
        }
        SeriesStyle::Markers { shape, size } => {
            for run in mapped_runs(scene, plot, series) {
                for point in run {
                    if plot.contains(point) {
                        push_marker(ops, point, shape, size, series.colour);
                    }
                }
            }
        }
    }
}

/// Splits a series into runs of consecutive representable points, mapped into
/// page space. A `NaN` (pen-up) or a value the axis scale cannot represent ends
/// the current run.
fn mapped_runs(scene: &Scene, plot: &Rect, series: &Series) -> Vec<Vec<[f64; 2]>> {
    let mut runs = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    for point in &series.points {
        match (map_x(scene, plot, point[0]), map_y(scene, plot, point[1])) {
            (Some(px), Some(py)) => current.push([px, py]),
            _ => {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// Data x to page x, or `None` if the value has no place on this axis.
fn map_x(scene: &Scene, plot: &Rect, value: f64) -> Option<f64> {
    let v = scene.x_scale.forward(value)?;
    let lo = scene.x_scale.forward(scene.x_range.0)?;
    let hi = scene.x_scale.forward(scene.x_range.1)?;
    if (hi - lo).abs() < f64::EPSILON {
        return None;
    }
    Some(plot.x0 + (v - lo) / (hi - lo) * plot.width())
}

/// Data y to page y (note the flip: page y grows downward).
fn map_y(scene: &Scene, plot: &Rect, value: f64) -> Option<f64> {
    let v = scene.y_scale.forward(value)?;
    let lo = scene.y_scale.forward(scene.y_range.0)?;
    let hi = scene.y_scale.forward(scene.y_range.1)?;
    if (hi - lo).abs() < f64::EPSILON {
        return None;
    }
    Some(plot.y1 - (v - lo) / (hi - lo) * plot.height())
}

/// Liang–Barsky clip of a polyline against `rect`, returning the pieces that
/// survive.
fn clip_polyline(points: &[[f64; 2]], rect: &Rect) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    for window in points.windows(2) {
        let (a, b) = (window[0], window[1]);
        match clip_segment(a, b, rect) {
            None => {
                if current.len() >= 2 {
                    out.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            Some((ca, cb)) => {
                if current.is_empty() {
                    current.push(ca);
                    current.push(cb);
                } else if approx_same(*current.last().expect("non-empty"), ca) {
                    current.push(cb);
                } else {
                    if current.len() >= 2 {
                        out.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                    current.push(ca);
                    current.push(cb);
                }
            }
        }
    }
    if current.len() >= 2 {
        out.push(current);
    }
    out
}

/// Whether two page-space points are the same to within a quarter of a point —
/// far below any visible difference, and enough to decide whether two clipped
/// segments join.
fn approx_same(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.25 && (a[1] - b[1]).abs() < 0.25
}

/// Liang–Barsky clip of a single segment. Returns `None` if it misses `rect`
/// entirely.
fn clip_segment(a: [f64; 2], b: [f64; 2], rect: &Rect) -> Option<([f64; 2], [f64; 2])> {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    let checks = [
        (-dx, a[0] - rect.x0),
        (dx, rect.x1 - a[0]),
        (-dy, a[1] - rect.y0),
        (dy, rect.y1 - a[1]),
    ];
    for (p, q) in checks {
        if p.abs() < f64::EPSILON {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return None;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }
    Some((
        [a[0] + t0 * dx, a[1] + t0 * dy],
        [a[0] + t1 * dx, a[1] + t1 * dy],
    ))
}

/// Emits the geometry for one marker.
fn push_marker(ops: &mut Vec<DrawOp>, at: [f64; 2], shape: MarkerShape, size: f64, colour: Rgb) {
    let r = size * 0.5;
    let [cx, cy] = at;
    let circle = |segments: usize| -> Vec<[f64; 2]> {
        (0..segments)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / segments as f64;
                [cx + r * a.cos(), cy + r * a.sin()]
            })
            .collect()
    };
    match shape {
        MarkerShape::Circle => ops.push(DrawOp::Polygon {
            points: circle(14),
            colour,
        }),
        MarkerShape::OpenCircle => {
            let mut pts = circle(14);
            pts.push(pts[0]);
            ops.push(DrawOp::Polyline {
                points: pts,
                width: 0.9,
                colour,
                dash: None,
            });
        }
        MarkerShape::Square => ops.push(DrawOp::Polygon {
            points: vec![
                [cx - r, cy - r],
                [cx + r, cy - r],
                [cx + r, cy + r],
                [cx - r, cy + r],
            ],
            colour,
        }),
        MarkerShape::Triangle => ops.push(DrawOp::Polygon {
            points: vec![
                [cx, cy - r * 1.15],
                [cx + r, cy + r * 0.75],
                [cx - r, cy + r * 0.75],
            ],
            colour,
        }),
        MarkerShape::Diamond => ops.push(DrawOp::Polygon {
            points: vec![
                [cx, cy - r * 1.25],
                [cx + r * 1.05, cy],
                [cx, cy + r * 1.25],
                [cx - r * 1.05, cy],
            ],
            colour,
        }),
        MarkerShape::Cross => {
            ops.push(DrawOp::Polyline {
                points: vec![[cx - r, cy - r], [cx + r, cy + r]],
                width: 1.0,
                colour,
                dash: None,
            });
            ops.push(DrawOp::Polyline {
                points: vec![[cx + r, cy - r], [cx - r, cy + r]],
                width: 1.0,
                colour,
                dash: None,
            });
        }
        MarkerShape::Plus => {
            ops.push(DrawOp::Polyline {
                points: vec![[cx - r, cy], [cx + r, cy]],
                width: 1.0,
                colour,
                dash: None,
            });
            ops.push(DrawOp::Polyline {
                points: vec![[cx, cy - r], [cx, cy + r]],
                width: 1.0,
                colour,
                dash: None,
            });
        }
    }
}

/// Emits `text` as stroked polylines, anchored and optionally rotated.
///
/// `rotate_deg` turns the text about its anchor, positive counter-clockwise on
/// the page, so `-90.0` gives the bottom-to-top y-axis title.
#[allow(clippy::too_many_arguments)]
pub fn push_text(
    ops: &mut Vec<DrawOp>,
    text: &str,
    anchor_at: [f64; 2],
    size: f64,
    h: HAnchor,
    v: VAnchor,
    rotate_deg: f64,
    colour: Rgb,
) {
    if text.is_empty() {
        return;
    }
    let k = size / font::CAP_HEIGHT;
    let width = font::text_width_units(text) * k;
    let dx = match h {
        HAnchor::Start => 0.0,
        HAnchor::Middle => -width * 0.5,
        HAnchor::End => -width,
    };
    // Page y grows downward while glyph y grows upward, so pushing the baseline
    // *down* the page (which is what `Top` and `Middle` anchoring need) is a
    // POSITIVE `dy` here. Getting this sign wrong puts a `Top`-anchored title
    // one cap height above the page edge, where it is silently clipped.
    let dy = match v {
        VAnchor::Baseline => 0.0,
        VAnchor::Middle => size * 0.5,
        VAnchor::Top => size,
    };
    let (sin, cos) = (rotate_deg.to_radians().sin(), rotate_deg.to_radians().cos());
    for stroke in font::text_polylines(text) {
        let points: Vec<[f64; 2]> = stroke
            .iter()
            .map(|[gx, gy]| {
                // glyph units -> points, y still upward
                let lx = gx * k + dx;
                let ly = gy * k - dy;
                // rotate in a y-up frame, then flip to page space
                let rx = lx * cos - ly * sin;
                let ry = lx * sin + ly * cos;
                [anchor_at[0] + rx, anchor_at[1] - ry]
            })
            .collect();
        ops.push(DrawOp::Polyline {
            points,
            width: TEXT_STROKE,
            colour,
            dash: None,
        });
    }
}

/// One axis tick.
#[derive(Clone, Debug)]
pub struct Tick {
    /// Position in data units.
    pub value: f64,
    /// Rendered label (empty for minor ticks).
    pub label: String,
    /// Whether it is a labelled major tick.
    pub major: bool,
}

/// Chooses ticks for an axis.
///
/// Linear axes get a "nice" step from the 1 / 2 / 2.5 / 5 x 10^k family, aiming
/// for about `target` intervals. Log axes get one major tick per decade, plus
/// unlabelled minor ticks at 2..9 within each decade when the span is short
/// enough for them to be distinguishable.
pub fn ticks_for(range: (f64, f64), scale: AxisScale, target: usize) -> Vec<Tick> {
    let (lo, hi) = (range.0.min(range.1), range.0.max(range.1));
    match scale {
        AxisScale::Linear => {
            let step = nice_step((hi - lo) / target.max(1) as f64);
            if !step.is_finite() || step <= 0.0 {
                return Vec::new();
            }
            let first = (lo / step).ceil() * step;
            let mut ticks = Vec::new();
            let mut k = 0;
            loop {
                let value = first + step * f64::from(k);
                if value > hi + step * 1e-9 || k > 1000 {
                    break;
                }
                ticks.push(Tick {
                    label: format_linear(value, step),
                    value,
                    major: true,
                });
                k += 1;
            }
            ticks
        }
        AxisScale::Log10 => {
            if lo <= 0.0 {
                return Vec::new();
            }
            let d_lo = lo.log10().floor() as i32;
            let d_hi = hi.log10().ceil() as i32;
            let decades = d_hi - d_lo;
            let minors = decades <= 6;
            let mut ticks = Vec::new();
            for d in d_lo..=d_hi {
                let base = 10.0_f64.powi(d);
                if base >= lo && base <= hi {
                    ticks.push(Tick {
                        value: base,
                        label: format_log_decade(d),
                        major: true,
                    });
                }
                if minors {
                    for m in 2..10 {
                        let value = base * f64::from(m);
                        if value >= lo && value <= hi {
                            ticks.push(Tick {
                                value,
                                label: String::new(),
                                major: false,
                            });
                        }
                    }
                }
            }
            ticks
        }
    }
}

/// Rounds a raw step up to the nearest 1 / 2 / 2.5 / 5 x 10^k.
fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return f64::NAN;
    }
    let exponent = raw.log10().floor();
    let magnitude = 10.0_f64.powf(exponent);
    let mantissa = raw / magnitude;
    let nice = if mantissa <= 1.0 {
        1.0
    } else if mantissa <= 2.0 {
        2.0
    } else if mantissa <= 2.5 {
        2.5
    } else if mantissa <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

/// Formats a linear tick label with just enough decimals for the step size,
/// falling back to scientific notation when the magnitudes get extreme.
fn format_linear(value: f64, step: f64) -> String {
    let snapped = if value.abs() < step * 1e-9 {
        0.0
    } else {
        value
    };
    if snapped != 0.0 && (snapped.abs() >= 1e5 || snapped.abs() < 1e-3) {
        return format!("{snapped:.1e}");
    }
    let decimals = (-step.log10().floor() as i32).clamp(0, 6) as usize;
    format!("{snapped:.decimals$}")
}

/// Formats a log-axis decade label: plain digits for readable magnitudes,
/// `10^n` otherwise.
fn format_log_decade(decade: i32) -> String {
    if (-3..=5).contains(&decade) {
        let value = 10.0_f64.powi(decade);
        if decade >= 0 {
            format!("{value:.0}")
        } else {
            format!("{value:.precision$}", precision = (-decade) as usize)
        }
    } else {
        format!("10^{decade}")
    }
}

/// Checks that clipping keeps what is inside and discards what is outside.
///
/// # Methodology
///
/// Clips three segments against a 100 x 100 rectangle: one wholly inside, one
/// wholly outside, and one crossing the left edge. Asserts the inside segment
/// is returned unchanged, the outside one is rejected, and the crossing one is
/// cut exactly at the boundary.
///
/// # Result
///
/// Passes as of 2026-08-20; the crossing segment is cut at x = 0 to within
/// 1e-9 points.
#[cfg(test)]
#[test]
fn liang_barsky_clips_at_the_frame() {
    let rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 100.0,
        y1: 100.0,
    };
    let inside = clip_segment([10.0, 10.0], [90.0, 90.0], &rect).expect("inside survives");
    assert_eq!(inside.0, [10.0, 10.0]);
    assert_eq!(inside.1, [90.0, 90.0]);

    assert!(clip_segment([-50.0, 10.0], [-10.0, 90.0], &rect).is_none());

    let crossing = clip_segment([-50.0, 50.0], [50.0, 50.0], &rect).expect("crossing survives");
    assert!((crossing.0[0] - 0.0).abs() < 1e-9, "cut at the left edge");
    assert_eq!(crossing.1, [50.0, 50.0]);
}

/// Checks that a pen-up `NaN` breaks a polyline rather than joining across it.
///
/// # Methodology
///
/// Builds a scene with one line series whose points contain a `NaN` break, runs
/// the layout, and counts how many polyline ops carry the series colour.
///
/// # Result
///
/// Passes as of 2026-08-20: two separate polylines, never one joined across the
/// discontinuity.
#[cfg(test)]
#[test]
fn a_nan_breaks_the_curve_instead_of_joining_it() {
    let mut scene = Scene::new("t", "x", "y");
    scene.x_range = (0.0, 10.0);
    scene.y_range = (0.0, 10.0);
    let colour = Rgb::new(1, 2, 3);
    scene.series.push(Series {
        name: "broken".into(),
        style: SeriesStyle::Line {
            width: 1.0,
            dash: None,
        },
        colour,
        points: vec![
            [1.0, 1.0],
            [2.0, 2.0],
            [f64::NAN, f64::NAN],
            [8.0, 8.0],
            [9.0, 9.0],
        ],
        show_in_legend: false,
    });
    let ops = to_draw_ops(&scene, PageSize::DEFAULT, FigurePalette::LIGHT_PUBLICATION);
    let runs = ops
        .iter()
        .filter(|op| matches!(op, DrawOp::Polyline { colour: c, .. } if *c == colour))
        .count();
    assert_eq!(runs, 2, "the NaN must split the curve into two polylines");
}

/// Checks the tick generators produce sane, in-range, labelled ticks.
///
/// # Methodology
///
/// Generates linear ticks over `[0, 1]` and log ticks over
/// `[611.657, 1e8]` — the pressure span from the triple point to 100 MPa, which
/// is the p-h diagram's actual axis — and asserts every tick lies inside its
/// range, that at least three major ticks exist, and that major ticks carry a
/// non-empty label while minor ticks do not.
///
/// # Result
///
/// Passes as of 2026-08-20: the log axis yields 6 labelled decades (1e3 to 1e8)
/// with minor ticks between them.
#[cfg(test)]
#[test]
fn tick_generation_stays_in_range_and_labels_only_majors() {
    for (range, scale) in [
        ((0.0, 1.0), AxisScale::Linear),
        ((-40.0, 380.0), AxisScale::Linear),
        ((611.657, 1.0e8), AxisScale::Log10),
    ] {
        let ticks = ticks_for(range, scale, 8);
        assert!(
            ticks.iter().filter(|t| t.major).count() >= 3,
            "too few major ticks for {range:?} {scale:?}"
        );
        for tick in &ticks {
            assert!(
                tick.value >= range.0 - 1e-9 && tick.value <= range.1 + 1e-9,
                "tick {} escaped {range:?}",
                tick.value
            );
            assert_eq!(
                tick.major,
                !tick.label.is_empty(),
                "label presence must track major-ness"
            );
        }
    }
}
