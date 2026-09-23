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

/// How a hand-drawn stroke is snapped onto the curve (GH issue #290).
///
/// The manual counterpart of [`TraceConfig`]: the operator says *where* the
/// curve is by drawing along it, and this says how finely to sample that
/// stroke and how far to look for the ink underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnapConfig {
    /// Which pixels count as curve ink — same meaning as
    /// [`TraceConfig::selector`].
    pub selector: CurveSelector,
    /// Spacing between output points, in pixels, measured **along the
    /// stroke** rather than along x (maintainer, 2026-09-23: "the points will
    /// be placed 2 pixels apart"). On a steep segment this is finer in x than
    /// a column scan can be, which is most of why drawing beats scanning
    /// there.
    pub spacing_px: f64,
    /// How far above and below a drawn sample to look for ink. Large enough
    /// to forgive an unsteady hand, small enough not to jump to the curve
    /// next door.
    pub search_radius_px: u32,
    /// Ink runs taller than this are not the curve — an axis, a bar, or a
    /// vertical gridline the stroke happened to cross — and the sample is
    /// dropped rather than read off them.
    ///
    /// This is the manual counterpart of [`TraceConfig::max_column_fill`],
    /// expressed in pixels rather than as a fraction of the frame because a
    /// drawn stroke has no frame: it is judged against the line the operator
    /// is following, whose thickness is a property of the figure.
    pub max_thickness_px: u32,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            selector: CurveSelector::DarkestBand { max_luminance: 128 },
            spacing_px: 2.0,
            search_radius_px: 12,
            max_thickness_px: 24,
        }
    }
}

/// Resample a polyline at `spacing` intervals of arc length, keeping the
/// first point and every `spacing` of drawn path after it.
///
/// The last drawn point is **not** forced into the output: it is wherever the
/// operator happened to release the button, so keeping it would place one
/// point at an arbitrary distance from its neighbour.
fn resample_by_arc_length(stroke: &[(f64, f64)], spacing: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let Some(&first) = stroke.first() else {
        return out;
    };
    out.push(first);
    let mut carried = 0.0;
    for pair in stroke.windows(2) {
        let (ax, ay) = pair[0];
        let (bx, by) = pair[1];
        let seg = (bx - ax).hypot(by - ay);
        if seg <= f64::EPSILON {
            continue;
        }
        let mut walked = spacing - carried;
        while walked <= seg {
            let t = walked / seg;
            out.push((ax + t * (bx - ax), ay + t * (by - ay)));
            walked += spacing;
        }
        carried = seg - (walked - spacing);
    }
    out
}

/// Snap a hand-drawn stroke onto the curve (GH issue #290).
///
/// **Method (deterministic).** The stroke is resampled by arc length at
/// [`SnapConfig::spacing_px`], and each sample is pulled **vertically** onto
/// the nearest run of curve ink within [`SnapConfig::search_radius_px`], in
/// the sample's own column. The run's centroid becomes `y_px` and its length
/// becomes `thickness_px`, exactly as in [`trace_curve`], so the per-point
/// uncertainty [`super::dataset`] derives from line thickness is unchanged.
///
/// Samples with no ink in their window are **dropped** — that is the stroke
/// having strayed off the curve (or crossed a gap in a dashed one), and
/// inventing a point there would be a reading nobody took. Consecutive
/// duplicates are dropped too: a steep stroke puts several samples in one
/// column, and after a vertical snap they are the same reading.
///
/// **Limit, by construction:** a vertical snap cannot follow a curve that
/// doubles back in x, such as a hysteresis loop. Those points are dropped
/// rather than guessed at.
///
/// `stroke` is in raster pixel coordinates, in the order it was drawn.
///
/// # Errors
///
/// [`DigitiserError::Trace`] if `spacing_px` is not positive, or the stroke
/// is empty.
pub fn snap_stroke(
    raster: &PlotRaster,
    stroke: &[(f64, f64)],
    config: &SnapConfig,
) -> Result<Vec<PixelTracePoint>, DigitiserError> {
    // `is_finite` first so a NaN spacing is refused rather than silently
    // resampling nothing — `<= 0.0` alone is false for NaN.
    if !config.spacing_px.is_finite() || config.spacing_px <= 0.0 {
        return Err(DigitiserError::Trace(
            "spacing_px must be greater than zero".to_string(),
        ));
    }
    if stroke.is_empty() {
        return Err(DigitiserError::Trace(
            "nothing was drawn — hold the button and draw along the curve".to_string(),
        ));
    }
    let (w, h) = (raster.width(), raster.height());
    if w == 0 || h == 0 {
        return Err(DigitiserError::Trace("the image is empty".to_string()));
    }

    let mut out: Vec<PixelTracePoint> = Vec::new();
    for (sx, sy) in resample_by_arc_length(stroke, config.spacing_px) {
        if sx < 0.0 || sy < 0.0 {
            continue;
        }
        let x = sx.round() as u32;
        if x >= w {
            continue;
        }
        let centre = sy.round() as i64;
        let radius = config.search_radius_px as i64;
        let top = centre.saturating_sub(radius).clamp(0, h as i64 - 1) as u32;
        let bottom = (centre + radius).clamp(0, h as i64 - 1) as u32;

        let runs = ink_runs(raster, x, top, bottom, &config.selector);
        let Some(run) = runs.into_iter().min_by(|a, b| {
            let da = (a.centroid() - sy).abs();
            let db = (b.centroid() - sy).abs();
            da.total_cmp(&db)
        }) else {
            continue;
        };
        // The window clips the run wherever the drawn point sat near the
        // edge of it, and a clipped run has the wrong centroid *and* the
        // wrong thickness — which would feed a wrong uncertainty into
        // `dataset`. Grow it back to the ink's real extent before reading
        // either off it. (Caught by
        // `samples_with_no_ink_under_them_are_dropped_not_invented`, which
        // first failed with a centroid of 49 on a band centred at 50.)
        let mut start = run.start;
        while start > 0 && config.selector.matches(raster, x, start - 1) {
            start -= 1;
        }
        let mut end = run.start + run.len - 1;
        while end + 1 < h && config.selector.matches(raster, x, end + 1) {
            end += 1;
        }
        let run = InkRun {
            start,
            len: end - start + 1,
        };
        if run.len > config.max_thickness_px {
            continue;
        }
        let point = PixelTracePoint {
            x_px: x as f64,
            y_px: run.centroid(),
            thickness_px: run.len as f64,
        };
        if out
            .last()
            .is_some_and(|p| p.x_px == point.x_px && p.y_px == point.y_px)
        {
            continue;
        }
        out.push(point);
    }
    Ok(out)
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

    // ------------------------------------------------------------------
    // GH issue #290 — the hand-drawn stroke, snapped.
    // ------------------------------------------------------------------

    /// A 100x100 white image with a 3-px-thick **sloping** black curve,
    /// `y = 10 + x/2`, and a horizontal gridline at row 90 for the snap to
    /// ignore.
    fn sloping() -> PlotRaster {
        PlotRaster::from_rgb_fn(100, 100, |x, y| {
            let curve = 10.0 + x as f64 / 2.0;
            if (y as f64 - curve).abs() <= 1.0 || y == 90 {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        })
    }

    /// A stroke drawn a few pixels off the curve is pulled onto it, and the
    /// run's thickness comes through for the uncertainty the dataset derives.
    #[test]
    fn a_stroke_drawn_near_the_curve_snaps_onto_it() {
        let raster = sloping();
        // Drawn 4 px above the curve, deliberately sloppily.
        let stroke: Vec<(f64, f64)> = (0..=90)
            .step_by(10)
            .map(|x| (x as f64, 10.0 + x as f64 / 2.0 - 4.0))
            .collect();

        let pts = snap_stroke(&raster, &stroke, &SnapConfig::default()).unwrap();

        assert!(pts.len() > 20, "only {} points", pts.len());
        for p in &pts {
            let expected = 10.0 + p.x_px / 2.0;
            assert!(
                (p.y_px - expected).abs() <= 1.0,
                "({}, {}) is not on the curve (expected y {expected})",
                p.x_px,
                p.y_px
            );
            assert!(
                p.thickness_px >= 2.0 && p.thickness_px <= 4.0,
                "thickness {} is not the drawn line",
                p.thickness_px
            );
            assert!(p.y_px < 80.0, "snapped to the gridline at row 90");
        }
    }

    /// "the points will be placed 2 pixels apart" — along the stroke, which
    /// is the default. Measured on a flat curve, where the arc length is the
    /// x distance, so the spacing is directly checkable.
    #[test]
    fn points_land_two_pixels_apart_along_the_stroke() {
        let raster = banded();
        let stroke = vec![(10.0, 48.0), (90.0, 48.0)];
        let pts = snap_stroke(&raster, &stroke, &SnapConfig::default()).unwrap();

        assert!(pts.len() >= 40, "only {} points", pts.len());
        for pair in pts.windows(2) {
            assert!(
                (pair[1].x_px - pair[0].x_px - 2.0).abs() < 1e-9,
                "{} to {} is not a 2 px step",
                pair[0].x_px,
                pair[1].x_px
            );
        }

        // And the spacing is a setting, not a constant.
        let coarse = snap_stroke(
            &raster,
            &stroke,
            &SnapConfig {
                spacing_px: 10.0,
                ..SnapConfig::default()
            },
        )
        .unwrap();
        assert!(coarse.len() < pts.len() / 4);
    }

    /// [`banded`] without its gridline — for the cases that need "there is
    /// nothing else up there to snap to".
    fn band_only() -> PlotRaster {
        PlotRaster::from_rgb_fn(100, 100, |_, y| {
            if (49..=51).contains(&y) {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        })
    }

    /// A stroke that wanders off the ink drops those samples rather than
    /// inventing a reading, and keeps the ones that did find the curve.
    #[test]
    fn samples_with_no_ink_under_them_are_dropped_not_invented() {
        let raster = band_only();
        // Starts on the band, then climbs far above it.
        let stroke = vec![(10.0, 50.0), (40.0, 50.0), (60.0, 5.0)];
        let pts = snap_stroke(&raster, &stroke, &SnapConfig::default()).unwrap();

        assert!(!pts.is_empty());
        for p in &pts {
            assert!(
                (p.y_px - 50.0).abs() < 1e-9,
                "invented a point at y {}",
                p.y_px
            );
        }
        // Nothing survives from the part of the stroke that left the band.
        assert!(pts.iter().all(|p| p.x_px <= 46.0), "{pts:?}");
    }

    /// A steep stroke puts several samples in one column; after a vertical
    /// snap they are the same reading, and one is kept.
    #[test]
    fn a_steep_stroke_does_not_emit_the_same_reading_twice() {
        let raster = band_only();
        let stroke = vec![(50.0, 30.0), (50.0, 60.0)];
        let pts = snap_stroke(&raster, &stroke, &SnapConfig::default()).unwrap();
        assert_eq!(pts.len(), 1, "{pts:?}");
        assert!((pts[0].y_px - 50.0).abs() < 1e-9);
    }

    /// Drawing near a gridline snaps to the **gridline** — there is ink
    /// there, and the operator put the stroke on it. Pinned so it is a
    /// documented property of drawing by hand rather than a surprise: the
    /// answer is to draw along the curve, or to raise the selector's
    /// threshold, not for the snap to second-guess where the pointer went.
    #[test]
    fn a_stroke_drawn_along_a_gridline_snaps_to_the_gridline() {
        let raster = banded(); // band at 49..=51, 1-px gridline at row 20
        let stroke = vec![(20.0, 21.0), (80.0, 21.0)];
        let pts = snap_stroke(&raster, &stroke, &SnapConfig::default()).unwrap();
        assert!(!pts.is_empty());
        for p in &pts {
            assert!((p.y_px - 20.0).abs() < 1e-9, "{p:?}");
            assert!((p.thickness_px - 1.0).abs() < 1e-9);
        }
    }

    /// An axis or a bar the stroke crosses is too thick to be the curve, so
    /// the sample is dropped instead of being read off it with a nonsense
    /// thickness (which `dataset` would turn into a nonsense uncertainty).
    #[test]
    fn ink_far_thicker_than_a_curve_is_not_read_as_one() {
        // A 40-px-tall black block: thicker than `max_thickness_px`.
        let raster = PlotRaster::from_rgb_fn(100, 100, |_, y| {
            if (30..=69).contains(&y) {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        });
        let stroke = vec![(10.0, 50.0), (90.0, 50.0)];
        assert!(snap_stroke(&raster, &stroke, &SnapConfig::default())
            .unwrap()
            .is_empty());

        // Raising the cap reads it, so the cap is the reason and not a bug.
        let pts = snap_stroke(
            &raster,
            &stroke,
            &SnapConfig {
                max_thickness_px: 64,
                ..SnapConfig::default()
            },
        )
        .unwrap();
        assert!(!pts.is_empty());
        assert!((pts[0].thickness_px - 40.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_or_badly_spaced_stroke_is_an_error_not_an_empty_dataset() {
        let raster = banded();
        assert!(snap_stroke(&raster, &[], &SnapConfig::default()).is_err());
        assert!(snap_stroke(
            &raster,
            &[(10.0, 50.0)],
            &SnapConfig {
                spacing_px: 0.0,
                ..SnapConfig::default()
            }
        )
        .is_err());
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
