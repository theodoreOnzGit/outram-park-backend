//! Automatic curve tracing — extracting curve pixel positions by column scan.
//!
//! Belongs here: [`CurveSelector`] (which pixels count as curve ink),
//! [`TraceStrategy`] (which vertical run to keep when a column has several),
//! [`TraceConfig`], [`PixelTracePoint`], and [`trace_curve`]. All strategy
//! dispatch is by enum `match` — no trait objects, per the workspace Rust
//! design rules. The trace is deterministic: the same raster and config
//! always produce the same points.
//!
//! Does not belong here: converting pixels to data values (that is
//! [`super::calibration`], applied in [`super::dataset`]) and axis-box
//! finding ([`super::detect`]).

use serde::{Deserialize, Serialize};

use super::detect::PixelRect;
use super::raster::PlotRaster;
use super::DigitiserError;

/// Which pixels count as "curve ink". Closed set, enum-dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveSelector {
    /// Any pixel with Rec. 709 luminance strictly below `max_luminance` is
    /// curve ink. The right default for black-on-white published figures.
    DarkestBand {
        /// Luminance cut, 0–255. 128 tolerates anti-aliasing and scan grey.
        max_luminance: u8,
    },
    /// Pixels within `tolerance` of a target colour (Euclidean RGB distance,
    /// 0–441). Use for a coloured curve that must be separated from black
    /// gridlines or from other curves.
    Rgb {
        /// Target curve colour as `[r, g, b]`.
        rgb: [u8; 3],
        /// Maximum Euclidean RGB distance from `rgb` that still counts.
        tolerance: u16,
    },
}

impl CurveSelector {
    /// Does the pixel at `(x, y)` count as curve ink under this selector?
    pub fn matches(&self, raster: &PlotRaster, x: u32, y: u32) -> bool {
        match *self {
            CurveSelector::DarkestBand { max_luminance } => raster.luminance(x, y) < max_luminance,
            CurveSelector::Rgb { rgb, tolerance } => {
                let [r, g, b] = raster.rgb(x, y);
                let dr = r as i32 - rgb[0] as i32;
                let dg = g as i32 - rgb[1] as i32;
                let db = b as i32 - rgb[2] as i32;
                let d2 = (dr * dr + dg * dg + db * db) as f64;
                d2.sqrt() <= tolerance as f64
            }
        }
    }
}

/// When a scanned column holds several disjoint vertical runs of curve ink
/// (curve + gridline, or two curves), which one is the curve? Closed set,
/// enum-dispatched. Ties always resolve to the topmost run (deterministic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceStrategy {
    /// Centroid of *all* matching pixels in the column. Cheapest; correct
    /// only when the column contains nothing but the one curve.
    ColumnCentroid,
    /// Centroid of the longest contiguous run. Robust against thin
    /// horizontal gridlines crossing the column.
    LargestRun,
    /// Centroid of the run nearest (vertically) to the previous column's
    /// accepted point; the first accepted column uses the longest run. Tracks
    /// one curve through crossings with other curves or gridlines. The
    /// default.
    ContinuityNearest,
}

/// Tuning for [`trace_curve`]. [`TraceConfig::default`] suits a clean
/// black-on-white single-curve figure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TraceConfig {
    /// What counts as curve ink. Default: luminance < 128.
    pub selector: CurveSelector,
    /// Run-choice strategy. Default: [`TraceStrategy::ContinuityNearest`].
    pub strategy: TraceStrategy,
    /// Sample every `column_step`-th pixel column (≥ 1). Default 1.
    pub column_step: u32,
    /// Pixels to shrink the frame inward on every side before scanning, so
    /// the frame lines and their anti-aliasing halo are not traced as curve.
    /// Default 3.
    pub inset: u32,
    /// Skip a column when the matched fraction of its scanned height exceeds
    /// this (it is a vertical gridline or axis, not curve). Default 0.6.
    pub max_column_fill: f64,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            selector: CurveSelector::DarkestBand { max_luminance: 128 },
            strategy: TraceStrategy::ContinuityNearest,
            column_step: 1,
            inset: 3,
            max_column_fill: 0.6,
        }
    }
}

/// One traced curve sample, still in pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PixelTracePoint {
    /// Column index of the sample (whole pixel, stored as `f64` so hand
    /// corrections can be sub-pixel).
    pub x_px: f64,
    /// Centroid row of the accepted ink run in this column.
    pub y_px: f64,
    /// Vertical extent (pixel count) of the accepted run — the local curve
    /// line thickness, which [`super::dataset`] turns into the per-point
    /// reading uncertainty.
    pub thickness_px: f64,
}

/// Trace the curve inside `frame`, one sample per scanned column.
///
/// **Method (deterministic).** For each sampled column inside the frame
/// (shrunk by [`TraceConfig::inset`]), the contiguous vertical runs of pixels
/// matching [`TraceConfig::selector`] are collected. Columns whose matched
/// fraction exceeds [`TraceConfig::max_column_fill`] are skipped as vertical
/// gridlines. One run is accepted per remaining column according to
/// [`TraceConfig::strategy`], and its centroid row becomes the sample.
/// Columns with no matching pixels yield no sample (gaps are permitted —
/// dashed curves still trace).
///
/// Returns the samples in strictly increasing `x_px` order; possibly empty
/// (e.g. an empty plot region) — emptiness is the *caller's* signal to warn,
/// not an error, because a legitimately empty sub-range can occur when
/// tracing a figure region-by-region.
///
/// # Errors
///
/// [`DigitiserError::Trace`] if `frame` (after inset) leaves no columns or
/// rows to scan, or `column_step == 0`.
pub fn trace_curve(
    raster: &PlotRaster,
    frame: &PixelRect,
    config: &TraceConfig,
) -> Result<Vec<PixelTracePoint>, DigitiserError> {
    if config.column_step == 0 {
        return Err(DigitiserError::Trace(
            "column_step must be >= 1".to_string(),
        ));
    }
    let left = frame.left + config.inset;
    let right = frame.right.saturating_sub(config.inset);
    let top = frame.top + config.inset;
    let bottom = frame.bottom.saturating_sub(config.inset);
    if left >= right || top >= bottom {
        return Err(DigitiserError::Trace(format!(
            "frame too small after inset {}: columns {left}..{right}, rows {top}..{bottom}",
            config.inset
        )));
    }
    let span = (bottom - top + 1) as f64;

    let mut points = Vec::new();
    let mut prev_y: Option<f64> = None;

    let mut x = left;
    while x <= right {
        let runs = ink_runs(raster, x, top, bottom, &config.selector);
        let matched: u32 = runs.iter().map(|r| r.len).sum();
        if !runs.is_empty() && (matched as f64) / span <= config.max_column_fill {
            let chosen = match config.strategy {
                TraceStrategy::ColumnCentroid => {
                    // Weighted centroid over all runs; thickness = total ink.
                    let total: f64 = runs.iter().map(|r| r.len as f64).sum();
                    let centroid: f64 = runs
                        .iter()
                        .map(|r| r.centroid() * r.len as f64)
                        .sum::<f64>()
                        / total;
                    (centroid, total)
                }
                TraceStrategy::LargestRun => {
                    let r = runs
                        .iter()
                        .max_by(|a, b| {
                            // Longest run; ties -> topmost (smaller start).
                            a.len.cmp(&b.len).then(b.start.cmp(&a.start))
                        })
                        .expect("non-empty");
                    (r.centroid(), r.len as f64)
                }
                TraceStrategy::ContinuityNearest => {
                    let r = match prev_y {
                        None => runs
                            .iter()
                            .max_by(|a, b| a.len.cmp(&b.len).then(b.start.cmp(&a.start)))
                            .expect("non-empty"),
                        Some(py) => runs
                            .iter()
                            .min_by(|a, b| {
                                let da = (a.centroid() - py).abs();
                                let db = (b.centroid() - py).abs();
                                da.partial_cmp(&db)
                                    .expect("finite centroids")
                                    .then(a.start.cmp(&b.start))
                            })
                            .expect("non-empty"),
                    };
                    (r.centroid(), r.len as f64)
                }
            };
            prev_y = Some(chosen.0);
            points.push(PixelTracePoint {
                x_px: x as f64,
                y_px: chosen.0,
                thickness_px: chosen.1,
            });
        }
        x += config.column_step;
    }
    Ok(points)
}

/// A contiguous vertical run of curve ink in one column.
#[derive(Debug, Clone, Copy)]
struct InkRun {
    /// First (topmost) row of the run.
    start: u32,
    /// Number of rows in the run.
    len: u32,
}

impl InkRun {
    /// Centre row of the run.
    fn centroid(&self) -> f64 {
        self.start as f64 + (self.len as f64 - 1.0) / 2.0
    }
}

/// All contiguous ink runs in column `x` between rows `top..=bottom`,
/// top-to-bottom order.
fn ink_runs(
    raster: &PlotRaster,
    x: u32,
    top: u32,
    bottom: u32,
    selector: &CurveSelector,
) -> Vec<InkRun> {
    let mut runs = Vec::new();
    let mut cur: Option<InkRun> = None;
    for y in top..=bottom {
        if selector.matches(raster, x, y) {
            match cur.as_mut() {
                Some(r) => r.len += 1,
                None => cur = Some(InkRun { start: y, len: 1 }),
            }
        } else if let Some(r) = cur.take() {
            runs.push(r);
        }
    }
    if let Some(r) = cur {
        runs.push(r);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: PixelRect = PixelRect {
        left: 0,
        right: 99,
        top: 0,
        bottom: 99,
    };

    /// 100x100 white image with a 3-px-thick horizontal black band centred
    /// on row 50, plus a 1-px horizontal gridline at row 20.
    fn banded() -> PlotRaster {
        PlotRaster::from_rgb_fn(100, 100, |_, y| {
            if (49..=51).contains(&y) || y == 20 {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        })
    }

    #[test]
    fn largest_run_ignores_thin_gridline() {
        let cfg = TraceConfig {
            strategy: TraceStrategy::LargestRun,
            ..TraceConfig::default()
        };
        let pts = trace_curve(&banded(), &FRAME, &cfg).unwrap();
        assert!(!pts.is_empty());
        for p in &pts {
            assert!((p.y_px - 50.0).abs() < 1e-9, "got {}", p.y_px);
            assert!((p.thickness_px - 3.0).abs() < 1e-9);
        }
    }

    #[test]
    fn column_centroid_is_pulled_by_the_gridline() {
        // Documents WHY ColumnCentroid is not the default: the gridline at
        // row 20 drags the centroid off the curve at row 50.
        let cfg = TraceConfig {
            strategy: TraceStrategy::ColumnCentroid,
            ..TraceConfig::default()
        };
        let pts = trace_curve(&banded(), &FRAME, &cfg).unwrap();
        assert!((pts[0].y_px - 42.5).abs() < 1e-9, "got {}", pts[0].y_px);
    }

    #[test]
    fn continuity_tracks_through_a_crossing() {
        // Flat curve at row 60 crossed by a diagonal; continuity should hold
        // row 60 rather than jumping to the diagonal.
        let img = PlotRaster::from_rgb_fn(100, 100, |x, y| {
            let flat = (59..=61).contains(&y);
            let diag = y == x; // crosses the flat band near x = 60
            if flat || diag {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        });
        let cfg = TraceConfig {
            strategy: TraceStrategy::ContinuityNearest,
            ..TraceConfig::default()
        };
        let pts = trace_curve(&img, &FRAME, &cfg).unwrap();
        // Away from the crossing the accepted run must be the flat band.
        for p in pts.iter().filter(|p| p.x_px < 40.0 || p.x_px > 80.0) {
            assert!((p.y_px - 60.0).abs() <= 1.0, "x {} y {}", p.x_px, p.y_px);
        }
    }

    #[test]
    fn vertical_gridline_columns_are_skipped() {
        let img = PlotRaster::from_rgb_fn(100, 100, |x, y| {
            if x == 30 || (49..=51).contains(&y) {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        });
        let pts = trace_curve(&img, &FRAME, &TraceConfig::default()).unwrap();
        assert!(pts.iter().all(|p| p.x_px != 30.0));
        assert!(pts.iter().any(|p| p.x_px == 29.0));
    }

    #[test]
    fn trace_is_deterministic() {
        let a = trace_curve(&banded(), &FRAME, &TraceConfig::default()).unwrap();
        let b = trace_curve(&banded(), &FRAME, &TraceConfig::default()).unwrap();
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(&b) {
            assert_eq!(p.x_px, q.x_px);
            assert_eq!(p.y_px, q.y_px);
        }
    }
}
