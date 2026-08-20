//! Backend-independent figure description, and the three exporters that render
//! it.
//!
//! # Shape of this module
//!
//! ```text
//!   Scene  ──layout::to_draw_ops──▶  Vec<DrawOp>  ──▶  svg::render  ──▶ String
//!                                                 ├──▶  pdf::render  ──▶ Vec<u8>
//!                                                 └──▶  png::render  ──▶ Vec<u8>
//! ```
//!
//! A [`Scene`] is what a diagram tab *means*: named series of data-space points
//! with a style, plus axis ranges, labels and provenance notes. It knows nothing
//! about pages, pixels or fonts.
//!
//! [`layout::to_draw_ops`] turns a `Scene` into page-space geometry: axes,
//! ticks, tick labels, legend, clipped data polylines and markers. Text is
//! converted to polylines there, using [`font`], so the three backends only ever
//! have to handle two primitives — a stroked polyline and a filled polygon.
//! That is why all three produce the same figure, and why the PDF writer needs
//! no font machinery.
//!
//! # Why not export egui's own canvas
//!
//! `egui_plot` draws to a GPU texture; getting a file out of it means a
//! framebuffer screenshot, which is raster-only, depends on the window size and
//! the display scale factor, and cannot run without a display at all. This
//! path is deterministic, resolution-independent, produces true vector output
//! for PDF and SVG, and — decisively for a headless CI box or a remote agent
//! session — runs with no display present. `--export-all` on the command line
//! uses exactly the same code the GUI's export buttons do.

pub mod csv_export;
pub mod font;
pub mod layout;
pub mod pdf;
pub mod png;
pub mod svg;

/// A colour, as 8-bit sRGB components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    /// Red component, 0–255.
    pub r: u8,
    /// Green component, 0–255.
    pub g: u8,
    /// Blue component, 0–255.
    pub b: u8,
}

impl Rgb {
    /// Builds a colour from 8-bit sRGB components.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The three components as 0.0–1.0 floats, which is what PDF's `RG`/`rg`
    /// colour operators take.
    pub fn as_unit_floats(self) -> [f64; 3] {
        [
            f64::from(self.r) / 255.0,
            f64::from(self.g) / 255.0,
            f64::from(self.b) / 255.0,
        ]
    }

    /// The `#rrggbb` form SVG uses.
    pub fn as_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// Black, used for axes, frames and text.
pub const INK: Rgb = Rgb::new(20, 20, 20);
/// The mid grey used for grid lines.
pub const GRID: Rgb = Rgb::new(200, 200, 200);
/// Paper white.
pub const PAPER: Rgb = Rgb::new(255, 255, 255);

/// Whether an axis is linear or base-10 logarithmic.
///
/// Issue #26 asks for a log pressure axis on the p-h diagram, which spans the
/// triple-point pressure (611.657 Pa) to 100 MPa — five and a half decades, so
/// a linear axis would compress everything below about 10 MPa into the baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisScale {
    /// Values are plotted as they are.
    Linear,
    /// Values are plotted as `log10(value)`. Non-positive values are dropped
    /// (they have no logarithm), which the layout pass does by breaking the
    /// polyline rather than by clamping.
    Log10,
}

impl AxisScale {
    /// Maps a data value into the space the axis is linear in. Returns `None`
    /// for a value with no representation (a non-positive value on a log axis,
    /// or any non-finite value).
    pub fn forward(self, value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        match self {
            Self::Linear => Some(value),
            Self::Log10 => {
                if value > 0.0 {
                    Some(value.log10())
                } else {
                    None
                }
            }
        }
    }

    /// Inverse of [`AxisScale::forward`], used to place tick labels.
    pub fn inverse(self, mapped: f64) -> f64 {
        match self {
            Self::Linear => mapped,
            Self::Log10 => 10.0_f64.powf(mapped),
        }
    }
}

/// The shape drawn at a scattered data point.
///
/// Distinct shapes matter here because a figure routinely carries four or five
/// overlay layers at once, and colour alone does not survive a greyscale print
/// or a colour-blind reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerShape {
    /// Filled circle.
    Circle,
    /// Filled square.
    Square,
    /// Filled upward triangle.
    Triangle,
    /// Filled diamond.
    Diamond,
    /// Open (unfilled) circle.
    OpenCircle,
    /// Diagonal cross, stroked.
    Cross,
    /// Upright plus, stroked.
    Plus,
}

/// How a series is drawn.
///
/// An enum rather than a trait object, per the workspace Rust design rules: the
/// set of styles is closed and known at compile time, so a new variant should
/// force every `match` to be updated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeriesStyle {
    /// A stroked polyline of the given width in points.
    Line {
        /// Stroke width in points.
        width: f64,
        /// Dash pattern in points, `None` for a solid line.
        dash: Option<(f64, f64)>,
    },
    /// Scattered markers of the given shape and size (diameter in points).
    Markers {
        /// Marker shape.
        shape: MarkerShape,
        /// Marker size (diameter or width) in points.
        size: f64,
    },
}

/// One named, styled set of points in data space.
#[derive(Clone, Debug)]
pub struct Series {
    /// Legend name. Also the name used in the CSV export's `series` column.
    pub name: String,
    /// How it is drawn.
    pub style: SeriesStyle,
    /// Colour.
    pub colour: Rgb,
    /// Points in data space, in the axis units named by the scene's labels.
    /// A `NaN` in either coordinate is a deliberate **pen-up**: it breaks the
    /// polyline, which is how a discontinuous curve (an isobar crossing the
    /// dome, an isotherm at its saturation jump) is represented without
    /// inventing a segment across the gap.
    pub points: Vec<[f64; 2]>,
    /// Whether the series gets a legend entry. Long fans of curves (21
    /// Zaloudek quality curves, say) share one entry rather than 21.
    ///
    /// Note that a `Series` deliberately carries **no** provenance string: the
    /// citation lives on [`crate::data::PlotLayer`], which is what the CSV
    /// export writes from. Keeping one copy means the figure and the data file
    /// cannot disagree about where a point came from.
    pub show_in_legend: bool,
}

/// Everything needed to draw one diagram, in data space.
#[derive(Clone, Debug)]
pub struct Scene {
    /// Figure title.
    pub title: String,
    /// x-axis label, including units.
    pub x_label: String,
    /// y-axis label, including units.
    pub y_label: String,
    /// x-axis scale.
    pub x_scale: AxisScale,
    /// y-axis scale.
    pub y_scale: AxisScale,
    /// Inclusive x-axis range in data units.
    pub x_range: (f64, f64),
    /// Inclusive y-axis range in data units.
    pub y_range: (f64, f64),
    /// The series, drawn in order, so later ones sit on top.
    pub series: Vec<Series>,
    /// Footnotes printed under the axes — used for the "quality is a derived
    /// lever-rule quantity, not an independently validated property" caveat
    /// that issue #26 requires be stated, and for dataset caveats such as
    /// Marviken test 24 not being validated.
    pub notes: Vec<String>,
}

impl Scene {
    /// An empty scene with sane defaults, ready to have series pushed onto it.
    pub fn new(
        title: impl Into<String>,
        x_label: impl Into<String>,
        y_label: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            x_label: x_label.into(),
            y_label: y_label.into(),
            x_scale: AxisScale::Linear,
            y_scale: AxisScale::Linear,
            x_range: (0.0, 1.0),
            y_range: (0.0, 1.0),
            series: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Recomputes [`Scene::x_range`] and [`Scene::y_range`] from the data, with
    /// a small margin, ignoring pen-up breaks and values the axis scale cannot
    /// represent.
    ///
    /// Returns `false` and leaves the ranges alone if there is nothing
    /// representable to fit — a scene with every layer switched off, for
    /// instance.
    pub fn autoscale(&mut self) -> bool {
        let mut x_lo = f64::INFINITY;
        let mut x_hi = f64::NEG_INFINITY;
        let mut y_lo = f64::INFINITY;
        let mut y_hi = f64::NEG_INFINITY;
        for series in &self.series {
            for point in &series.points {
                let (Some(_), Some(_)) = (
                    self.x_scale.forward(point[0]),
                    self.y_scale.forward(point[1]),
                ) else {
                    continue;
                };
                x_lo = x_lo.min(point[0]);
                x_hi = x_hi.max(point[0]);
                y_lo = y_lo.min(point[1]);
                y_hi = y_hi.max(point[1]);
            }
        }
        if !(x_lo.is_finite() && x_hi.is_finite() && y_lo.is_finite() && y_hi.is_finite()) {
            return false;
        }
        self.x_range = pad_range(x_lo, x_hi, self.x_scale);
        self.y_range = pad_range(y_lo, y_hi, self.y_scale);
        true
    }
}

/// Widens `[lo, hi]` by 3 % of its span (in the axis's own linear space) so
/// points do not sit exactly on the frame. Degenerate spans get an absolute
/// widening instead, since 3 % of zero is zero.
fn pad_range(lo: f64, hi: f64, scale: AxisScale) -> (f64, f64) {
    let (Some(a), Some(b)) = (scale.forward(lo), scale.forward(hi)) else {
        return (lo, hi);
    };
    let span = b - a;
    let pad = if span.abs() < f64::EPSILON {
        if a.abs() < f64::EPSILON {
            1.0
        } else {
            a.abs() * 0.05
        }
    } else {
        span * 0.03
    };
    (scale.inverse(a - pad), scale.inverse(b + pad))
}

/// A qualitative palette for overlay layers.
///
/// Chosen for separability in greyscale as well as in colour: the entries
/// alternate dark and mid tones rather than running through a hue wheel at
/// constant lightness.
pub const PALETTE: [Rgb; 10] = [
    Rgb::new(0x1f, 0x4e, 0x9c), // deep blue
    Rgb::new(0xc0, 0x39, 0x2b), // brick red
    Rgb::new(0x1e, 0x8a, 0x4c), // green
    Rgb::new(0x8e, 0x44, 0xad), // purple
    Rgb::new(0xd6, 0x8a, 0x10), // amber
    Rgb::new(0x16, 0xa0, 0xa8), // teal
    Rgb::new(0x7f, 0x4b, 0x28), // brown
    Rgb::new(0xc2, 0x18, 0x7a), // magenta
    Rgb::new(0x4a, 0x4a, 0x4a), // dark grey
    Rgb::new(0x5b, 0x8c, 0x00), // olive
];

/// Checks that the axis transforms round-trip and reject what they should.
///
/// # Methodology
///
/// For a spread of positive values, asserts
/// `inverse(forward(v)) == v` to within 1e-12 relative on both scales; asserts
/// that a log axis returns `None` for zero, for a negative value and for
/// `NaN`; asserts a linear axis also returns `None` for `NaN` (the pen-up
/// convention depends on that).
///
/// # Result
///
/// Passes as of 2026-08-20.
#[cfg(test)]
#[test]
fn axis_scales_round_trip_and_reject_unrepresentable_values() {
    for value in [1e-3, 0.5, 1.0, 22.064, 1000.0, 3.5e6] {
        for scale in [AxisScale::Linear, AxisScale::Log10] {
            let mapped = scale.forward(value).expect("positive finite value");
            let back = scale.inverse(mapped);
            assert!(
                (back - value).abs() <= value.abs() * 1e-12,
                "{scale:?} did not round-trip {value}"
            );
        }
    }
    assert!(AxisScale::Log10.forward(0.0).is_none());
    assert!(AxisScale::Log10.forward(-1.0).is_none());
    assert!(AxisScale::Log10.forward(f64::NAN).is_none());
    assert!(AxisScale::Linear.forward(f64::NAN).is_none());
    assert!(AxisScale::Linear.forward(f64::INFINITY).is_none());
}

/// Checks that autoscaling brackets the data and copes with an empty scene.
///
/// # Methodology
///
/// Builds a scene with one series containing a pen-up `NaN` break and a value a
/// log axis cannot represent, autoscales, and asserts the resulting range
/// strictly contains every representable point. Then autoscales a scene with no
/// representable point at all and asserts it reports failure instead of
/// producing an infinite range.
///
/// # Result
///
/// Passes as of 2026-08-20.
#[cfg(test)]
#[test]
fn autoscale_brackets_the_data_and_reports_an_empty_scene() {
    let mut scene = Scene::new("t", "x", "y");
    scene.y_scale = AxisScale::Log10;
    scene.series.push(Series {
        name: "s".into(),
        style: SeriesStyle::Line {
            width: 1.0,
            dash: None,
        },
        colour: INK,
        points: vec![
            [1.0, 10.0],
            [f64::NAN, f64::NAN],
            [3.0, 1000.0],
            [4.0, -5.0],
        ],
        show_in_legend: true,
    });
    assert!(scene.autoscale());
    assert!(scene.x_range.0 < 1.0 && scene.x_range.1 > 3.0);
    assert!(scene.y_range.0 < 10.0 && scene.y_range.1 > 1000.0);
    assert!(
        scene.y_range.0 > 0.0,
        "a log axis must not autoscale to <= 0"
    );

    let mut empty = Scene::new("t", "x", "y");
    assert!(!empty.autoscale());
}
