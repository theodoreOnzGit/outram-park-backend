//! Digitised datasets — the output type, with mandatory provenance.
//!
//! Belongs here: [`FigureSource`], [`PointOrigin`], [`ReviewStatus`],
//! [`DigitisedPoint`], [`TraceRecord`], [`DigitisedDataset`], and their
//! JSON/CSV export. The design rule (from `DATA_POLICY.md`: digitisation is
//! a processing step and must be documented as one) is that **a dataset
//! cannot exist, be serialised, or be exported without its calibration and
//! source record** — [`DigitisedDataset`]'s calibration and source are plain
//! required fields, there is no points-only constructor, and both exporters
//! read them from the struct itself.
//!
//! Does not belong here: pixel scanning ([`super::trace`]), calibration math
//! ([`super::calibration`]), or interactive editing (the TUI/GUI binaries own
//! that, and record their edits *into* these types).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::calibration::PlotCalibration;
use super::detect::PixelRect;
use super::trace::{PixelTracePoint, TraceConfig};
use super::DigitiserError;

/// Version stamp written into every serialised dataset so future readers can
/// tell what they are looking at. Bump on breaking schema changes.
pub const DATASET_SCHEMA_VERSION: u32 = 1;

/// Where the digitised figure came from — the document-level half of the
/// provenance record.
///
/// `document_id`/`document_title` should reference the figure's
/// [`crate::KovanDocument`] (its `id` and `title`) when the source has been
/// catalogued into the KOVAN literature archive; they stay `None` for a
/// not-yet-catalogued source, in which case `image_path` at least pins the
/// file that was digitised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureSource {
    /// [`crate::KovanDocument::id`] of the catalogued source document, if
    /// catalogued.
    pub document_id: Option<String>,
    /// [`crate::KovanDocument::title`] (or a free-text citation) of the
    /// source document.
    pub document_title: Option<String>,
    /// Figure designation as printed, e.g. `"Fig. 7"` or `"Figure 3(b)"`.
    /// Required — a digitisation that cannot say which figure it read is not
    /// usable as evidence.
    pub figure: String,
    /// Page number the figure appears on, if known.
    pub page: Option<u32>,
    /// Path of the image file that was digitised (as given by the caller).
    pub image_path: Option<String>,
    /// Lowercase-hex SHA-256 of the image file's bytes, so the exact raster
    /// this dataset was read from can be re-identified. Filled automatically
    /// when the raster was loaded from a file.
    pub image_sha256: Option<String>,
    /// Free-text notes (e.g. "curve labelled '235U thermal'", crop applied,
    /// known scan skew).
    pub notes: Option<String>,
}

impl FigureSource {
    /// Minimal source record: just the figure designation. Fill the optional
    /// fields directly afterwards.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Io`] if `figure` is empty — the figure designation
    /// is the one non-negotiable part of the provenance record.
    pub fn new(figure: impl Into<String>) -> Result<Self, DigitiserError> {
        let figure = figure.into();
        if figure.trim().is_empty() {
            return Err(DigitiserError::Io(
                "figure designation must not be empty (provenance requirement)".to_string(),
            ));
        }
        Ok(Self {
            document_id: None,
            document_title: None,
            figure,
            page: None,
            image_path: None,
            image_sha256: None,
            notes: None,
        })
    }
}

/// How a single point came to be — automatic, hand-placed, or hand-corrected.
/// Closed set, enum-dispatched; recorded per point so a reviewer can see
/// exactly which values a human touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointOrigin {
    /// Emitted by the automatic tracer, untouched by a human.
    AutoTraced,
    /// Placed by a human (TUI/GUI editing), never produced by the tracer.
    HandPlaced {
        /// Who placed it (operator name as given to the front end).
        by: String,
    },
    /// Auto-traced, then moved by a human.
    HandCorrected {
        /// Who corrected it.
        by: String,
    },
}

/// Which front end a human review happened in. Closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewInterface {
    /// `kovan-tui`'s Digitiser tab (ratatui).
    Tui,
    /// `kovan`, the GUI (egui).
    Gui,
    /// Reviewed outside the shipped front ends (e.g. plotted and inspected by
    /// hand); the reviewer takes responsibility for the method.
    External,
}

/// Whether a human has verified this dataset. The automatic CLI always emits
/// [`ReviewStatus::Unreviewed`]; only the hybrid front ends (or an external
/// reviewer) may record a review, and the record says who, when, and where —
/// **confirmation is recorded, never assumed**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    /// No human has checked the points against the figure.
    Unreviewed,
    /// A human inspected the points overlaid on the figure and accepted them.
    Reviewed {
        /// Reviewer name.
        by: String,
        /// UTC timestamp, ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`).
        at: String,
        /// Front end the review happened in.
        interface: ReviewInterface,
    },
}

/// One digitised data point, in the figure's own units, with its reading
/// uncertainty and per-point origin.
///
/// Uncertainties are stored as separate `minus`/`plus` magnitudes (both
/// `>= 0`) because on a logarithmic axis the pixel reading error maps to an
/// **asymmetric, value-dependent** interval — collapsing it to one symmetric
/// number would misstate exactly the case (log-log decay-heat curves) this
/// tool exists for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DigitisedPoint {
    /// Data x value, in the figure's x-axis units.
    pub x: f64,
    /// Data y value, in the figure's y-axis units.
    pub y: f64,
    /// Magnitude of the downward x reading uncertainty: the value could be as
    /// low as `x - x_minus`.
    pub x_minus: f64,
    /// Magnitude of the upward x reading uncertainty.
    pub x_plus: f64,
    /// Magnitude of the downward y reading uncertainty.
    pub y_minus: f64,
    /// Magnitude of the upward y reading uncertainty.
    pub y_plus: f64,
    /// Pixel column this point sits at (kept so the TUI/GUI can re-overlay
    /// the point on the image; `None` only for hand-placed points created in
    /// data space).
    pub x_px: Option<f64>,
    /// Pixel row this point sits at.
    pub y_px: Option<f64>,
    /// How the point came to be.
    pub origin: PointOrigin,
}

/// Record of the automatic pass that produced the auto-traced points: the
/// exact configuration, so the run can be reproduced bit-for-bit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceRecord {
    /// Engine identifier and version, e.g.
    /// `"kovan graph digitiser 0.0.0"`.
    pub engine: String,
    /// The full trace configuration used.
    pub config: TraceConfig,
    /// The pixel frame the trace ran inside.
    pub frame: PixelRect,
    /// `true` when the frame came from automatic detection,
    /// `false` when the caller supplied it.
    pub frame_auto_detected: bool,
}

/// A complete digitised dataset: points **plus** the calibration, source,
/// operator, and review records that make them usable as validation evidence.
///
/// There is deliberately no way to build or export one without calibration
/// and source — they are required fields of the only constructors
/// ([`DigitisedDataset::from_pixel_trace`] and deserialisation of a
/// previously exported record), and both exporters embed them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DigitisedDataset {
    /// Schema version of this record ([`DATASET_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Which document and figure the points were read from.
    pub source: FigureSource,
    /// The axis calibration every point was computed with (reference points,
    /// linear/log per axis).
    pub calibration: PlotCalibration,
    /// x-axis label as printed on the figure, units included, e.g.
    /// `"Time after fission burst (s)"`.
    pub x_label: String,
    /// y-axis label as printed on the figure, units included.
    pub y_label: String,
    /// Who ran the digitisation (a person, or e.g.
    /// `"kovan-cli digitise (automatic)"` for the unattended CLI).
    pub digitised_by: String,
    /// UTC timestamp of the digitisation, ISO 8601.
    pub digitised_at: String,
    /// The automatic pass that produced the auto-traced points; `None` for a
    /// dataset built entirely by hand in a front end.
    pub trace: Option<TraceRecord>,
    /// Human verification state. Starts [`ReviewStatus::Unreviewed`].
    pub review: ReviewStatus,
    /// The points, in increasing-x order as traced.
    pub points: Vec<DigitisedPoint>,
}

impl DigitisedDataset {
    /// Convert a pixel-space trace into a data-space dataset.
    ///
    /// Every trace point is mapped through `calibration`, and its reading
    /// uncertainty is computed **in data space** by mapping the pixel
    /// half-thickness through the same calibration (see
    /// [`uncertainty_interval`]), so log axes get the correct asymmetric,
    /// value-dependent intervals.
    ///
    /// All provenance fields are required arguments — that is the point.
    #[allow(clippy::too_many_arguments)] // each argument IS a provenance requirement
    pub fn from_pixel_trace(
        source: FigureSource,
        calibration: PlotCalibration,
        x_label: impl Into<String>,
        y_label: impl Into<String>,
        digitised_by: impl Into<String>,
        digitised_at: impl Into<String>,
        trace_record: TraceRecord,
        trace_points: &[PixelTracePoint],
    ) -> Self {
        let column_half = 0.5 * trace_record.config.column_step.max(1) as f64;
        let points = trace_points
            .iter()
            .map(|p| {
                let (x, y) = calibration.point_at(p.x_px, p.y_px);
                let half_thickness = (p.thickness_px / 2.0).max(0.5);
                let ((x_minus, x_plus), (y_minus, y_plus)) = xy_uncertainty_interval(
                    &calibration,
                    p.x_px,
                    p.y_px,
                    column_half,
                    half_thickness,
                );
                DigitisedPoint {
                    x,
                    y,
                    x_minus,
                    x_plus,
                    y_minus,
                    y_plus,
                    x_px: Some(p.x_px),
                    y_px: Some(p.y_px),
                    origin: PointOrigin::AutoTraced,
                }
            })
            .collect();
        Self {
            schema_version: DATASET_SCHEMA_VERSION,
            source,
            calibration,
            x_label: x_label.into(),
            y_label: y_label.into(),
            digitised_by: digitised_by.into(),
            digitised_at: digitised_at.into(),
            trace: Some(trace_record),
            review: ReviewStatus::Unreviewed,
            points,
        }
    }

    /// Serialise to pretty-printed JSON — the canonical on-disk form; feed it
    /// back to [`DigitisedDataset::from_json_str`] (which the TUI/GUI use to
    /// load a dataset for review).
    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("DigitisedDataset always serialises")
    }

    /// Parse a dataset previously written by [`DigitisedDataset::to_json_string`].
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Io`] on malformed JSON or an unknown
    /// `schema_version`.
    pub fn from_json_str(json: &str) -> Result<Self, DigitiserError> {
        let d: Self = serde_json::from_str(json)
            .map_err(|e| DigitiserError::Io(format!("cannot parse dataset json: {e}")))?;
        if d.schema_version != DATASET_SCHEMA_VERSION {
            return Err(DigitiserError::Io(format!(
                "unsupported dataset schema_version {} (this build reads {})",
                d.schema_version, DATASET_SCHEMA_VERSION
            )));
        }
        Ok(d)
    }

    /// Write the JSON form to `path`.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Io`] on filesystem failure.
    pub fn write_json(&self, path: &Path) -> Result<(), DigitiserError> {
        std::fs::write(path, self.to_json_string())
            .map_err(|e| DigitiserError::Io(format!("cannot write {}: {e}", path.display())))
    }

    /// Read a JSON dataset from `path`.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Io`] on filesystem failure or malformed content.
    pub fn read_json(path: &Path) -> Result<Self, DigitiserError> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| DigitiserError::Io(format!("cannot read {}: {e}", path.display())))?;
        Self::from_json_str(&s)
    }

    /// Serialise to CSV with the **full provenance record embedded** as `#`
    /// comment header lines — so even the "just give me columns" export can
    /// never be separated from its calibration. Data rows are deliberately
    /// lean: `x, y` only. Per-point uncertainty
    /// (`x_minus`/`x_plus`/`y_minus`/`y_plus`) and each point's `origin`
    /// (auto-traced/hand-placed/hand-corrected) are real data, not noise,
    /// but they belong in the full structured record
    /// ([`DigitisedDataset::to_json_string`]) rather than cluttering the
    /// row a plotting script or spreadsheet reads — maintainer dogfooding
    /// feedback, 2026-09-02.
    pub fn to_csv_string(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "# kovan digitiser dataset (schema v{})",
            self.schema_version
        );
        let _ = writeln!(s, "# figure: {}", self.source.figure);
        if let Some(t) = &self.source.document_title {
            let _ = writeln!(s, "# document_title: {t}");
        }
        if let Some(i) = &self.source.document_id {
            let _ = writeln!(s, "# document_id: {i}");
        }
        if let Some(p) = self.source.page {
            let _ = writeln!(s, "# page: {p}");
        }
        if let Some(p) = &self.source.image_path {
            let _ = writeln!(s, "# image_path: {p}");
        }
        if let Some(h) = &self.source.image_sha256 {
            let _ = writeln!(s, "# image_sha256: {h}");
        }
        if let Some(n) = &self.source.notes {
            let _ = writeln!(s, "# notes: {n}");
        }
        match &self.calibration {
            PlotCalibration::AxisAligned { x: cx, y: cy } => {
                let _ = writeln!(
                    s,
                    "# x_axis: {} scale, px {} = {} , px {} = {}",
                    cx.scale, cx.r1.pixel, cx.r1.value, cx.r2.pixel, cx.r2.value
                );
                let _ = writeln!(
                    s,
                    "# y_axis: {} scale, px {} = {} , px {} = {}",
                    cy.scale, cy.r1.pixel, cy.r1.value, cy.r2.pixel, cy.r2.value
                );
            }
            PlotCalibration::Parallelogram(p) => {
                let _ = writeln!(
                    s,
                    "# calibration: parallelogram, pixel corners {:?}",
                    p.pixel_corners
                );
                let _ = writeln!(
                    s,
                    "# x_axis: {} scale, left = {}, right = {}",
                    p.x_scale, p.x_value_at_left, p.x_value_at_right
                );
                let _ = writeln!(
                    s,
                    "# y_axis: {} scale, top = {}, bottom = {}",
                    p.y_scale, p.y_value_at_top, p.y_value_at_bottom
                );
            }
        }
        let _ = writeln!(s, "# x_label: {}", self.x_label);
        let _ = writeln!(s, "# y_label: {}", self.y_label);
        let _ = writeln!(s, "# digitised_by: {}", self.digitised_by);
        let _ = writeln!(s, "# digitised_at: {}", self.digitised_at);
        if let Some(t) = &self.trace {
            let _ = writeln!(s, "# engine: {}", t.engine);
        }
        match &self.review {
            ReviewStatus::Unreviewed => {
                let _ = writeln!(s, "# review: UNREVIEWED — points not yet human-verified");
            }
            ReviewStatus::Reviewed { by, at, interface } => {
                let _ = writeln!(s, "# review: reviewed by {by} at {at} via {interface:?}");
            }
        }
        let _ = writeln!(s, "# per-point uncertainty and origin (auto-traced/hand-placed/hand-corrected) are in the JSON export, not this CSV");
        let _ = writeln!(s, "x,y");
        for p in &self.points {
            let _ = writeln!(s, "{},{}", p.x, p.y);
        }
        s
    }

    /// Write the CSV form to `path`.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Io`] on filesystem failure.
    pub fn write_csv(&self, path: &Path) -> Result<(), DigitiserError> {
        std::fs::write(path, self.to_csv_string())
            .map_err(|e| DigitiserError::Io(format!("cannot write {}: {e}", path.display())))
    }

    /// Record a human review — called by the hybrid front ends after the
    /// operator has inspected the overlay and accepted the points.
    pub fn record_review(
        &mut self,
        by: impl Into<String>,
        at: impl Into<String>,
        interface: ReviewInterface,
    ) {
        self.review = ReviewStatus::Reviewed {
            by: by.into(),
            at: at.into(),
            interface,
        };
    }
}

/// Map a `± half_pixels` pixel reading error at `pixel` through an axis
/// calibration, returning `(minus, plus)` magnitudes in data units (both
/// `>= 0`).
///
/// On a linear axis the two magnitudes are equal; on a logarithmic axis they
/// are asymmetric and grow with the value — which is why they are computed by
/// evaluating the calibration at `pixel ± half_pixels` rather than by a
/// constant scale factor.
pub fn uncertainty_interval(
    axis: &super::calibration::AxisCalibration,
    pixel: f64,
    half_pixels: f64,
) -> (f64, f64) {
    let v = axis.value_at(pixel);
    let a = axis.value_at(pixel - half_pixels);
    let b = axis.value_at(pixel + half_pixels);
    let lo = a.min(b);
    let hi = a.max(b);
    (v - lo, hi - v)
}

/// The [`PlotCalibration`]-level generalisation of [`uncertainty_interval`]
/// — a pixel-space perturbation of `half_px_x`/`half_px_y` around
/// `(x_px, y_px)`, read off as data-value spread in each axis (op-vyb9).
///
/// [`PlotCalibration::AxisAligned`] delegates straight to
/// [`uncertainty_interval`] for each axis — **byte-identical to the
/// pre-parallelogram behaviour**, since x only ever depended on column and y
/// only ever depended on row there. [`PlotCalibration::Parallelogram`]
/// perturbs the pixel column (for x) or row (for y) by the same half-pixel
/// amount and reads the resulting spread off [`PlotCalibration::point_at`] —
/// pixel space is still a plain image grid even though the pixel→data map is
/// skewed, so "perturb the column, see how x moves" still makes sense; it
/// just no longer decomposes into two independent 1-D calibrations the way
/// the axis-aligned case does, and a small amount of the other axis's
/// movement is folded in along with it (an accepted simplification, same
/// spirit as `uncertainty_interval`'s own "treat x and y as independent"
/// assumption).
pub fn xy_uncertainty_interval(
    cal: &PlotCalibration,
    x_px: f64,
    y_px: f64,
    half_px_x: f64,
    half_px_y: f64,
) -> ((f64, f64), (f64, f64)) {
    match cal {
        PlotCalibration::AxisAligned { x, y } => (
            uncertainty_interval(x, x_px, half_px_x),
            uncertainty_interval(y, y_px, half_px_y),
        ),
        PlotCalibration::Parallelogram(_) => {
            let (vx, vy) = cal.point_at(x_px, y_px);
            let (ax, _) = cal.point_at(x_px - half_px_x, y_px);
            let (bx, _) = cal.point_at(x_px + half_px_x, y_px);
            let (_, cy) = cal.point_at(x_px, y_px - half_px_y);
            let (_, dy) = cal.point_at(x_px, y_px + half_px_y);
            let x_lo = ax.min(bx);
            let x_hi = ax.max(bx);
            let y_lo = cy.min(dy);
            let y_hi = cy.max(dy);
            ((vx - x_lo, x_hi - vx), (vy - y_lo, y_hi - vy))
        }
    }
}

/// Current UTC time as an ISO 8601 string (`YYYY-MM-DDTHH:MM:SSZ`), from the
/// system clock and pure `std` (no chrono dependency). Used by the binaries
/// to stamp `digitised_at` / review times; pass an explicit string instead
/// when reproducible output is needed (the CLI's `--timestamp` flag).
pub fn utc_now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Gregorian calendar date from days since 1970-01-01 (Howard Hinnant's
/// `civil_from_days` algorithm, exact over the full i64 range used here).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // year of era
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::super::calibration::{AxisCalibration, AxisRef, AxisScale, PlotCalibration};
    use super::super::detect::PixelRect;
    use super::super::trace::{PixelTracePoint, TraceConfig};
    use super::*;

    fn cal() -> PlotCalibration {
        PlotCalibration::AxisAligned {
            x: AxisCalibration::new(
                AxisScale::Logarithmic,
                AxisRef {
                    pixel: 0.0,
                    value: 1.0,
                },
                AxisRef {
                    pixel: 100.0,
                    value: 1000.0,
                },
            )
            .unwrap(),
            y: AxisCalibration::new(
                AxisScale::Linear,
                AxisRef {
                    pixel: 100.0,
                    value: 0.0,
                },
                AxisRef {
                    pixel: 0.0,
                    value: 50.0,
                },
            )
            .unwrap(),
        }
    }

    fn dataset() -> DigitisedDataset {
        let trace_record = TraceRecord {
            engine: "test".to_string(),
            config: TraceConfig::default(),
            frame: PixelRect {
                left: 0,
                right: 100,
                top: 0,
                bottom: 100,
            },
            frame_auto_detected: true,
        };
        DigitisedDataset::from_pixel_trace(
            FigureSource::new("Fig. 1").unwrap(),
            cal(),
            "time (s)",
            "power (%)",
            "unit test",
            "2026-08-11T00:00:00Z",
            trace_record,
            &[PixelTracePoint {
                x_px: 50.0,
                y_px: 40.0,
                thickness_px: 3.0,
            }],
        )
    }

    #[test]
    fn empty_figure_designation_is_rejected() {
        assert!(FigureSource::new("  ").is_err());
    }

    #[test]
    fn log_axis_uncertainty_is_asymmetric() {
        let d = dataset();
        let p = &d.points[0];
        // x is on a log axis: upward error must exceed downward error.
        assert!(
            p.x_plus > p.x_minus,
            "plus {} minus {}",
            p.x_plus,
            p.x_minus
        );
        // y is linear: symmetric to rounding.
        assert!((p.y_plus - p.y_minus).abs() < 1e-9);
        assert!(p.x_minus > 0.0 && p.y_minus > 0.0);
    }

    #[test]
    fn json_round_trip_preserves_everything() {
        let d = dataset();
        let back = DigitisedDataset::from_json_str(&d.to_json_string()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let mut d = dataset();
        d.schema_version = 999;
        let json = serde_json::to_string(&d).unwrap();
        assert!(DigitisedDataset::from_json_str(&json).is_err());
    }

    #[test]
    fn csv_embeds_the_provenance_record() {
        let mut d = dataset();
        d.record_review("reviewer", "2026-08-11T01:00:00Z", ReviewInterface::Tui);
        let csv = d.to_csv_string();
        for needle in [
            "# figure: Fig. 1",
            "# x_axis: log scale",
            "# y_axis: linear scale",
            "# digitised_by: unit test",
            "reviewed by reviewer",
            "x,y",
        ] {
            assert!(csv.contains(needle), "csv missing {needle:?}:\n{csv}");
        }
    }

    #[test]
    fn unreviewed_status_is_stated_in_csv() {
        let csv = dataset().to_csv_string();
        assert!(csv.contains("UNREVIEWED"));
    }

    /// Maintainer dogfooding feedback, 2026-09-02: per-point uncertainty
    /// and origin columns clutter the exported CSV and don't belong in
    /// it — they stay in the JSON export instead.
    #[test]
    fn csv_rows_do_not_contain_per_point_uncertainty_or_origin_columns() {
        let csv = dataset().to_csv_string();
        // Explanatory `#` header comments are allowed to mention these
        // words (they say where the data actually lives); only the
        // non-comment lines (the header row + data rows) must not.
        let data_section: String = csv
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        for needle in [
            "x_minus",
            "x_plus",
            "y_minus",
            "y_plus",
            "auto",
            "hand-placed",
            "hand-corrected",
        ] {
            assert!(
                !data_section.contains(needle),
                "{needle:?} leaked into the data section:\n{data_section}"
            );
        }
        // ... but the JSON export still carries it all in full.
        let json = dataset().to_json_string();
        assert!(
            json.contains("x_minus") && json.contains("y_plus"),
            "{json}"
        );
    }

    #[test]
    fn csv_rows_contain_only_x_and_y() {
        let trace_record = TraceRecord {
            engine: "test".to_string(),
            config: TraceConfig::default(),
            frame: PixelRect {
                left: 0,
                right: 100,
                top: 0,
                bottom: 100,
            },
            frame_auto_detected: true,
        };
        let d = DigitisedDataset::from_pixel_trace(
            FigureSource::new("Fig. 1").unwrap(),
            cal(),
            "time (s)",
            "power (%)",
            "unit test",
            "2026-08-11T00:00:00Z",
            trace_record,
            &[
                PixelTracePoint {
                    x_px: 0.0,
                    y_px: 100.0,
                    thickness_px: 3.0,
                },
                PixelTracePoint {
                    x_px: 50.0,
                    y_px: 50.0,
                    thickness_px: 3.0,
                },
                PixelTracePoint {
                    x_px: 100.0,
                    y_px: 0.0,
                    thickness_px: 3.0,
                },
            ],
        );
        let csv = d.to_csv_string();
        let data_lines: Vec<&str> = csv
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        // Header + exactly 3 data rows, each with exactly 2 comma-separated
        // fields (x, y) -- no calibration/reference values, uncertainty, or
        // origin leaking into the rows themselves.
        assert_eq!(data_lines.len(), 4, "{data_lines:?}");
        assert_eq!(data_lines[0], "x,y");
        for row in &data_lines[1..] {
            assert_eq!(
                row.split(',').count(),
                2,
                "row {row:?} should have exactly x,y"
            );
        }
    }

    #[test]
    fn timestamp_format_is_iso8601_utc() {
        let t = utc_now_iso8601();
        // e.g. 2026-08-11T02:03:04Z
        assert_eq!(t.len(), 20, "got {t}");
        assert!(t.ends_with('Z') && t.chars().nth(10) == Some('T'));
        // Sanity: the algorithm agrees with a known date (2026-08-11 is
        // 20676 days after the epoch at 00:00:00 UTC).
        assert_eq!(civil_from_days(20_676), (2026, 8, 11));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
}
