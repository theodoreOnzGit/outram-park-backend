//! One-shot automatic digitisation — the pipeline every front end shares.
//!
//! Belongs here: [`AxisValueSpec`], [`AutoDigitiseConfig`], [`AxisPixelRefs`]
//! and [`auto_digitise`], which chain frame detection → calibration → trace →
//! dataset in one deterministic call. The CLI runs exactly this and nothing
//! more; the TUI/GUI run it as their "automatic pass first" and then let a
//! human correct the result.
//!
//! Does not belong here: the individual algorithms (see [`super::detect`],
//! [`super::calibration`], [`super::trace`]) or any interactivity.

use serde::{Deserialize, Serialize};

use super::calibration::{AxisCalibration, AxisRef, AxisScale, PlotCalibration};
use super::dataset::{DigitisedDataset, FigureSource, TraceRecord};
use super::detect::{detect_plot_frame, DetectConfig, PixelRect};
use super::raster::PlotRaster;
use super::trace::{trace_curve, TraceConfig};
use super::DigitiserError;

/// How the numeric axis values are anchored to pixels for one axis. Closed
/// set, enum-dispatched.
///
/// Tick-label OCR is deliberately out of scope (see the [`super`] module
/// doc), so the *values* always come from the caller; what varies is whether
/// the *pixels* they attach to come from automatic frame detection or are
/// given explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AxisPixelRefs {
    /// Anchor the values to the detected frame edges: `min_value` at the
    /// frame's left (x axis) / bottom (y axis), `max_value` at its right /
    /// top. The fully automatic path — correct whenever the figure's axis
    /// extremes are labelled, which is the common case.
    FrameEdges {
        /// Data value at the left/bottom frame edge.
        min_value: f64,
        /// Data value at the right/top frame edge.
        max_value: f64,
    },
    /// Two explicit pixel↔value pairs, e.g. read off gridline intersections.
    /// Use when the curve is cropped oddly or the frame edges are unlabelled.
    Explicit {
        /// First reference (pixel coordinate along this axis + its value).
        r1: AxisRef,
        /// Second reference.
        r2: AxisRef,
    },
}

/// Full specification of one axis: scale plus pixel anchoring.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisValueSpec {
    /// Linear or logarithmic.
    pub scale: AxisScale,
    /// Where the values sit in pixel space.
    pub refs: AxisPixelRefs,
}

/// Everything the automatic pipeline needs besides the image and the
/// provenance strings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutoDigitiseConfig {
    /// x-axis specification.
    pub x: AxisValueSpec,
    /// y-axis specification.
    pub y: AxisValueSpec,
    /// Frame-detection tuning.
    pub detect: DetectConfig,
    /// Curve-trace tuning.
    pub trace: TraceConfig,
}

/// Run the full automatic pipeline: detect (or derive) the frame, build the
/// calibration, trace the curve, and package a [`DigitisedDataset`] with the
/// complete provenance record. Deterministic: same raster + config +
/// provenance strings → identical dataset.
///
/// Frame detection is skipped only when **both** axes use
/// [`AxisPixelRefs::Explicit`] *and* automatic detection fails — in that case
/// the trace region falls back to the rectangle spanned by the explicit
/// reference pixels. When either axis anchors to
/// [`AxisPixelRefs::FrameEdges`], detection must succeed.
///
/// `digitised_by`/`digitised_at` are recorded verbatim; pass
/// [`super::dataset::utc_now_iso8601`] for `digitised_at` unless a
/// reproducible stamp is required. The returned dataset is always
/// [`super::dataset::ReviewStatus::Unreviewed`].
///
/// # Errors
///
/// Any [`DigitiserError`] from detection, calibration, or tracing.
pub fn auto_digitise(
    raster: &PlotRaster,
    config: &AutoDigitiseConfig,
    source: FigureSource,
    x_label: impl Into<String>,
    y_label: impl Into<String>,
    digitised_by: impl Into<String>,
    digitised_at: impl Into<String>,
) -> Result<DigitisedDataset, DigitiserError> {
    let detected = detect_plot_frame(raster, &config.detect);
    let both_explicit = matches!(config.x.refs, AxisPixelRefs::Explicit { .. })
        && matches!(config.y.refs, AxisPixelRefs::Explicit { .. });

    let (frame, frame_auto_detected) = match detected {
        Ok(f) => (f, true),
        Err(_) if both_explicit => (explicit_ref_rect(config)?, false),
        Err(e) => return Err(e),
    };

    let x_cal = axis_calibration(&config.x, frame.left as f64, frame.right as f64)?;
    // Rows grow downward: min_value anchors to `bottom` (the larger row).
    let y_cal = axis_calibration(&config.y, frame.bottom as f64, frame.top as f64)?;
    let calibration = PlotCalibration { x: x_cal, y: y_cal };

    let trace_points = trace_curve(raster, &frame, &config.trace)?;

    let mut source = source;
    if source.image_sha256.is_none() {
        source.image_sha256 = raster.source_sha256().map(str::to_string);
    }

    let record = TraceRecord {
        engine: format!("kovan graph digitiser {}", env!("CARGO_PKG_VERSION")),
        config: config.trace,
        frame,
        frame_auto_detected,
    };

    Ok(DigitisedDataset::from_pixel_trace(
        source,
        calibration,
        x_label,
        y_label,
        digitised_by,
        digitised_at,
        record,
        &trace_points,
    ))
}

/// Build one axis's calibration from its spec, anchoring
/// [`AxisPixelRefs::FrameEdges`] values to the given frame-edge pixels.
fn axis_calibration(
    spec: &AxisValueSpec,
    min_edge_pixel: f64,
    max_edge_pixel: f64,
) -> Result<AxisCalibration, DigitiserError> {
    match spec.refs {
        AxisPixelRefs::FrameEdges {
            min_value,
            max_value,
        } => AxisCalibration::new(
            spec.scale,
            AxisRef {
                pixel: min_edge_pixel,
                value: min_value,
            },
            AxisRef {
                pixel: max_edge_pixel,
                value: max_value,
            },
        ),
        AxisPixelRefs::Explicit { r1, r2 } => AxisCalibration::new(spec.scale, r1, r2),
    }
}

/// Trace-region fallback when both axes have explicit pixel refs and frame
/// detection failed: the rectangle spanned by the reference pixels.
fn explicit_ref_rect(config: &AutoDigitiseConfig) -> Result<PixelRect, DigitiserError> {
    let (AxisPixelRefs::Explicit { r1: x1, r2: x2 }, AxisPixelRefs::Explicit { r1: y1, r2: y2 }) =
        (config.x.refs, config.y.refs)
    else {
        unreachable!("caller checked both_explicit");
    };
    let left = x1.pixel.min(x2.pixel).floor();
    let right = x1.pixel.max(x2.pixel).ceil();
    let top = y1.pixel.min(y2.pixel).floor();
    let bottom = y1.pixel.max(y2.pixel).ceil();
    if left < 0.0 || top < 0.0 || right <= left + 10.0 || bottom <= top + 10.0 {
        return Err(DigitiserError::Detection(format!(
            "explicit reference pixels span a degenerate trace region \
             ({left}..{right}, {top}..{bottom})"
        )));
    }
    Ok(PixelRect {
        left: left as u32,
        right: right as u32,
        top: top as u32,
        bottom: bottom as u32,
    })
}
