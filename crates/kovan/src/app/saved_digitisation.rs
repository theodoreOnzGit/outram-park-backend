//! # Re-opening a saved digitised graph: prefill the wizard, restore the curves
//!
//! Maintainer, 2026-09-28: "next time we have an edit digitisation, please
//! pre-fill the values in the wizard with existing values." Before this, the
//! reader's "Edit digitisation" re-cropped the saved region and opened the
//! three-stage [`super::plot_setup::PlotSetup`] **empty** — ranges, log flags
//! and labels all retyped — and the digitiser started with no points, so an
//! edit meant re-tracing every curve.
//!
//! This module is the exact inverse of the save path, and nothing more:
//!
//! | Saved by | As | Read back by |
//! |---|---|---|
//! | [`crate::digitiser::dataset::DigitisedDataset::extraction`] | `x_axis`/`y_axis`: `"{scale} scale, px {p1} = {v1} , px {p2} = {v2}"` (rectangle) or `"{scale} scale, left = {l}, right = {r}"` / `"top = {t}, bottom = {b}"` (parallelogram) | [`parse_axis_record`] |
//! | the same | `figure`, `x_label`, `y_label`, `digitised_by` | [`SavedDigitisation::from_artifact`] |
//! | `DigitiseApp::save_into_project` | one ```csv fence (one curve), or `### Series:` blocks ([`crate::artifact::render_multi_series_body`]) | [`saved_series`] |
//! | the artifact's `[source]` | page, region | the re-crop itself (`PdfReaderState::recrop_artifact`) |
//!
//! ## What cannot be restored, and why nothing is guessed
//!
//! - **Notes, rotation and deskew are not in the artifact.** The wizard's
//!   quarter-turn and skew are recorded only in the dataset's `notes`, which
//!   the `[extraction]` table does not carry. They start at zero; an operator
//!   who turned the figure the first time must turn it again.
//! - **A rectangle calibration's pixels ARE recorded** (`px 107.2 = …`), in the
//!   space of the raster as it was digitised — i.e. *after* any turn/deskew.
//!   They are restored verbatim, so they are right exactly when the same turn
//!   and skew are applied again, and the digitiser warns when they fall
//!   outside the crop.
//! - **A parallelogram calibration's corners are NOT recorded** — only its
//!   edge values. Its values, scales and labels are prefilled, but the saved
//!   points are held back until the operator has dragged the corners back
//!   onto the axes, and then placed through *that* calibration. Pixel
//!   positions are never invented.
//! - The CSV carries data values only, so per-point origin and reading
//!   uncertainty are not recorded; restored points are marked hand-placed by
//!   the saved `digitised_by`, and their uncertainty is re-derived from the
//!   calibration they are placed through (±0.5 px, as for any placed point).
//!
//! Everything here is egui-free, so it is tested headless.

use crate::artifact::{parse_series_blocks, Artifact, Extraction};
use crate::digitiser::calibration::{AxisCalibration, AxisRef, AxisScale, PlotCalibration};

use super::plot_setup::PlotSetup;

/// One axis's saved calibration, as recovered from its `[extraction]` string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisRecord {
    /// Whether the axis was logarithmic.
    pub log: bool,
    /// Value at the first reference: the left edge (x) or the bottom (y) —
    /// the digitiser's `X1`/`Y1`, the wizard's `min`.
    pub v1: f64,
    /// Value at the second reference: right (x) or top (y) — `X2`/`Y2`.
    pub v2: f64,
    /// The two reference pixels `(p1, p2)`, when the save recorded them (a
    /// rectangle calibration does; a parallelogram's string does not).
    pub pixels: Option<(f64, f64)>,
}

impl AxisRecord {
    /// The axis as an [`AxisCalibration`], when its pixels were recorded.
    pub fn calibration(&self) -> Option<AxisCalibration> {
        let (p1, p2) = self.pixels?;
        let scale = if self.log {
            AxisScale::Logarithmic
        } else {
            AxisScale::Linear
        };
        AxisCalibration::new(
            scale,
            AxisRef {
                pixel: p1,
                value: self.v1,
            },
            AxisRef {
                pixel: p2,
                value: self.v2,
            },
        )
        .ok()
    }
}

/// Parse one `x_axis`/`y_axis` string written by
/// [`crate::digitiser::dataset::DigitisedDataset::extraction`] back into an
/// [`AxisRecord`]. `None` for anything that is not exactly one of the shapes
/// the save path writes — a malformed string is left for the operator to
/// fill, never half-guessed.
///
/// Accepted:
/// - `"linear scale, px 107.2 = 0 , px 358.7 = 100"` (rectangle; pixels kept)
/// - `"log scale, left = 1, right = 1000"` (parallelogram x)
/// - `"log scale, top = 1000, bottom = 1"` (parallelogram y; `bottom` is `v1`)
pub fn parse_axis_record(s: &str) -> Option<AxisRecord> {
    let (scale, rest) = s.trim().split_once(',')?;
    let log = match scale.trim() {
        "linear scale" => false,
        "log scale" => true,
        _ => return None,
    };
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return None;
    }
    let num = |t: &str| -> Option<f64> { t.trim().parse::<f64>().ok().filter(|v| v.is_finite()) };
    let pair = |t: &str| -> Option<(String, f64)> {
        let (k, v) = t.split_once('=')?;
        Some((k.trim().to_string(), num(v)?))
    };
    let (k1, a) = pair(parts[0])?;
    let (k2, b) = pair(parts[1])?;
    let rec = if let (Some(p1), Some(p2)) = (k1.strip_prefix("px "), k2.strip_prefix("px ")) {
        AxisRecord {
            log,
            v1: a,
            v2: b,
            pixels: Some((num(p1)?, num(p2)?)),
        }
    } else {
        match (k1.as_str(), k2.as_str()) {
            ("left", "right") => AxisRecord {
                log,
                v1: a,
                v2: b,
                pixels: None,
            },
            ("top", "bottom") => AxisRecord {
                log,
                v1: b,
                v2: a,
                pixels: None,
            },
            _ => return None,
        }
    };
    // The same guard the wizard's own `range_error` applies: a record the
    // wizard would refuse is not a record worth prefilling.
    if rec.v1 == rec.v2 || (rec.log && (rec.v1 <= 0.0 || rec.v2 <= 0.0)) {
        return None;
    }
    Some(rec)
}

/// One curve read back out of a saved artifact's body.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSeries {
    /// The `### Series:` name, or `None` for a single-curve body.
    pub name: Option<String>,
    /// The CSV's header row, `(x label, y label)`.
    pub header: Option<(String, String)>,
    /// `(x, y)` data values, in saved order.
    pub points: Vec<(f64, f64)>,
    /// Data rows that did not parse as two numbers, skipped.
    pub skipped_rows: usize,
}

/// Parse one curve's CSV (`xlabel,ylabel` header then `x,y` rows, RFC 4180
/// quoting on the header, `#` comment lines from the pre-2026-09-08 format
/// ignored).
fn parse_series_csv(name: Option<String>, csv_text: &str) -> SavedSeries {
    let data: String = csv_text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut header = None;
    let mut points = Vec::new();
    let mut skipped_rows = 0;
    for (i, rec) in reader.records().enumerate() {
        let Ok(rec) = rec else {
            skipped_rows += 1;
            continue;
        };
        let x = rec.get(0).and_then(|s| s.trim().parse::<f64>().ok());
        let y = rec.get(1).and_then(|s| s.trim().parse::<f64>().ok());
        match (x, y, rec.len()) {
            (Some(x), Some(y), 2) => points.push((x, y)),
            _ if i == 0 && rec.len() == 2 => {
                header = Some((rec[0].to_string(), rec[1].to_string()));
            }
            _ => skipped_rows += 1,
        }
    }
    SavedSeries {
        name,
        header,
        points,
        skipped_rows,
    }
}

/// Every curve in a saved `digitised_graph` artifact: the `### Series:`
/// blocks when the body has them, otherwise its one ```csv fence. Empty when
/// the body has no CSV at all.
pub fn saved_series(artifact: &Artifact) -> Vec<SavedSeries> {
    let blocks = parse_series_blocks(&artifact.body);
    if !blocks.is_empty() {
        return blocks
            .into_iter()
            .map(|b| parse_series_csv(Some(b.name), &b.csv))
            .collect();
    }
    artifact
        .csv_block()
        .map(|csv| vec![parse_series_csv(None, csv)])
        .unwrap_or_default()
}

/// Everything recoverable from a saved digitised graph, ready for the app:
/// the prefilled wizard, the calibration it was digitised against, and the
/// curves.
#[derive(Debug, Clone)]
pub struct SavedDigitisation {
    /// The artifact this was read from (the re-save replaces it).
    pub artifact_id: String,
    /// The wizard, every recoverable field filled; `prefill_note` says what
    /// was not.
    pub setup: PlotSetup,
    /// Saved x-axis calibration, if its string parsed.
    pub x: Option<AxisRecord>,
    /// Saved y-axis calibration, if its string parsed.
    pub y: Option<AxisRecord>,
    /// The saved curves.
    pub series: Vec<SavedSeries>,
    /// Who digitised it, from `[extraction].digitised_by`.
    pub digitised_by: Option<String>,
}

impl SavedDigitisation {
    /// Read `artifact` back into a prefilled wizard plus restorable curves.
    ///
    /// `page` and `document_title` are what the crop and the active paper
    /// already resolved (the artifact's `[source]` page, the paper's
    /// bibliography title) — the same two [`PlotSetup::begin`] takes.
    pub fn from_artifact(
        artifact: &Artifact,
        page: Option<u32>,
        document_title: Option<String>,
    ) -> Self {
        let ex: Option<&Extraction> = artifact.toml.extraction.as_ref();
        let series = saved_series(artifact);
        let mut blank: Vec<&str> = Vec::new();

        // The heading is what the save path passed as the title (the
        // figure designation, `#` intact); the extraction's `figure` had
        // `#` rewritten to "No.", so it is only the fallback.
        let figure = Some(artifact.heading.trim().to_string())
            .filter(|f| !f.is_empty())
            .or_else(|| ex.and_then(|e| e.figure.clone()));
        let mut setup = PlotSetup::begin(figure, page, document_title);

        let parse = |s: Option<&String>| s.and_then(|s| parse_axis_record(s));
        let x = parse(ex.and_then(|e| e.x_axis.as_ref()));
        let y = parse(ex.and_then(|e| e.y_axis.as_ref()));
        match x {
            Some(r) => {
                setup.x_min = r.v1.to_string();
                setup.x_max = r.v2.to_string();
                setup.x_log = r.log;
            }
            None => blank.push("x range"),
        }
        match y {
            Some(r) => {
                setup.y_min = r.v1.to_string();
                setup.y_max = r.v2.to_string();
                setup.y_log = r.log;
            }
            None => blank.push("y range"),
        }

        // Labels: the extraction first (what the save wrote), the CSV header
        // second — but never the header's own `x`/`y` fallback, which is what
        // the save writes for a BLANK label and so carries no information.
        let header = series.first().and_then(|s| s.header.clone());
        let label = |from_ex: Option<&String>, from_csv: Option<String>, fallback: &str| {
            from_ex
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| from_csv.filter(|s| !s.trim().is_empty() && s.trim() != fallback))
        };
        match label(
            ex.and_then(|e| e.x_label.as_ref()),
            header.as_ref().map(|h| h.0.clone()),
            "x",
        ) {
            Some(l) => setup.x_label = l,
            None => blank.push("x label"),
        }
        match label(
            ex.and_then(|e| e.y_label.as_ref()),
            header.as_ref().map(|h| h.1.clone()),
            "y",
        ) {
            Some(l) => setup.y_label = l,
            None => blank.push("y label"),
        }

        let mut note = format!(
            "Prefilled from saved digitisation {} ({} curve{}, {} point{}). \
             Not recorded in the artifact, so not restored: notes, rotation, deskew",
            artifact.id(),
            series.len(),
            if series.len() == 1 { "" } else { "s" },
            series.iter().map(|s| s.points.len()).sum::<usize>(),
            if series.iter().map(|s| s.points.len()).sum::<usize>() == 1 {
                ""
            } else {
                "s"
            },
        );
        note.push_str(" — apply the same turn/skew as the first time.");
        if !blank.is_empty() {
            note.push_str(&format!(
                " Could not read the saved {}; left blank.",
                blank.join(", ")
            ));
        }
        if x.is_some_and(|r| r.pixels.is_none()) || y.is_some_and(|r| r.pixels.is_none()) {
            note.push_str(
                " Parallelogram calibration: its corners were not saved, so the \
                 points are restored only after you drag the corners back onto the axes.",
            );
        }
        // The values are the data: a range changed in the wizard re-reads
        // every point from its pixel, so the wizard must know what "as saved"
        // was in order to warn before doing it (maintainer, 2026-09-28).
        // Only a rectangle record is re-read; a parallelogram's points are
        // placed through the operator's new corners with their values kept.
        let rectangle = |r: Option<AxisRecord>| r.filter(|r| r.pixels.is_some());
        if let (Some(xr), Some(yr)) = (rectangle(x), rectangle(y)) {
            setup.saved_ranges = Some(super::plot_setup::SavedRanges {
                text: [
                    setup.x_min.clone(),
                    setup.x_max.clone(),
                    setup.y_min.clone(),
                    setup.y_max.clone(),
                ],
                x_log: xr.log,
                y_log: yr.log,
                points: series.iter().map(|s| s.points.len()).sum(),
            });
        }
        note.push_str(
            " The saved values are kept exactly while the ranges are left as saved; \
             their markers are drawn where the saved header puts them, so markers \
             off the curves mean the header does not describe these values.",
        );
        setup.prefill_note = Some(note);

        Self {
            artifact_id: artifact.id().to_string(),
            setup,
            x,
            y,
            series,
            digitised_by: ex.and_then(|e| e.digitised_by.clone()),
        }
    }

    /// The exact calibration the saved points were digitised against, when
    /// both axes recorded their pixels (a rectangle calibration). `None` for a
    /// parallelogram or an unreadable record.
    pub fn saved_calibration(&self) -> Option<PlotCalibration> {
        Some(PlotCalibration::AxisAligned {
            x: self.x?.calibration()?,
            y: self.y?.calibration()?,
        })
    }

    /// The saved reference pixels in the digitiser's `ref_px` order
    /// `[X1, X2, Y1, Y2]`, when recorded.
    pub fn reference_pixels(&self) -> Option<[f64; 4]> {
        let (x1, x2) = self.x?.pixels?;
        let (y1, y2) = self.y?.pixels?;
        Some([x1, x2, y1, y2])
    }

    /// Whether the saved record is a parallelogram (values without pixels).
    pub fn is_parallelogram(&self) -> bool {
        self.x.is_some_and(|r| r.pixels.is_none()) || self.y.is_some_and(|r| r.pixels.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digitiser::calibration::ParallelogramCalibration;
    use crate::digitiser::calibration::PixelPoint;
    use crate::digitiser::dataset::{
        DigitisedDataset, FigureSource, ReviewStatus, DATASET_SCHEMA_VERSION,
    };

    #[test]
    fn parses_a_linear_rectangle_axis_with_its_pixels() {
        let r = parse_axis_record("linear scale, px 107.2 = 0 , px 358.7 = 100").unwrap();
        assert_eq!(
            r,
            AxisRecord {
                log: false,
                v1: 0.0,
                v2: 100.0,
                pixels: Some((107.2, 358.7))
            }
        );
    }

    #[test]
    fn parses_a_log_rectangle_axis() {
        let r = parse_axis_record("log scale, px 107.2 = 0.001 , px 358.7 = 1000").unwrap();
        assert!(r.log);
        assert_eq!((r.v1, r.v2), (0.001, 1000.0));
        assert_eq!(r.pixels, Some((107.2, 358.7)));
    }

    #[test]
    fn parses_both_parallelogram_axes_with_bottom_as_the_minimum() {
        let x = parse_axis_record("linear scale, left = -5, right = 5").unwrap();
        assert_eq!((x.v1, x.v2, x.pixels), (-5.0, 5.0, None));
        let y = parse_axis_record("log scale, top = 1000, bottom = 1").unwrap();
        assert!(y.log);
        assert_eq!((y.v1, y.v2, y.pixels), (1.0, 1000.0, None), "bottom is Y1");
    }

    #[test]
    fn malformed_axis_strings_are_none_not_guesses() {
        for bad in [
            "",
            "linear scale",
            "cubic scale, px 1 = 0 , px 2 = 1",
            "linear scale, px 1 = 0",
            "linear scale, px 1 = zero , px 2 = 1",
            "linear scale, px 1 = 0 , px 2 = 1 , px 3 = 2",
            "linear scale, left = 0, top = 1",
            "linear scale, px 1 = 5 , px 2 = 5",   // zero extent
            "log scale, px 1 = 0 , px 2 = 10",     // log of zero
            "linear scale, px one = 0 , px 2 = 1", // pixel not a number
        ] {
            assert!(parse_axis_record(bad).is_none(), "{bad:?} must not parse");
        }
    }

    /// Build a dataset and its [`Extraction`] exactly the way the save path
    /// does, then render the artifact the way `save_digitised_csv` would.
    fn saved_artifact(
        cal: PlotCalibration,
        heading: &str,
        body: String,
        x_label: &str,
        y_label: &str,
    ) -> Artifact {
        let d = DigitisedDataset {
            schema_version: DATASET_SCHEMA_VERSION,
            source: FigureSource::new(heading).unwrap(),
            calibration: cal,
            x_label: x_label.into(),
            y_label: y_label.into(),
            digitised_by: "tester via kovan (gui, hand-placed)".into(),
            digitised_at: "2026-09-28T00:00:00Z".into(),
            trace: None,
            review: ReviewStatus::Unreviewed,
            series: None,
            points: Vec::new(),
        };
        let toml = crate::artifact::ArtifactToml {
            kovan: crate::artifact::ArtifactMeta {
                id: "graph-7".into(),
                kind: crate::artifact::ArtifactKind::DigitisedGraph,
                created: "2026-09-28T00:00:00Z".into(),
                modified: "2026-09-28T00:00:00Z".into(),
                reviewed: None,
            },
            source: Some(crate::artifact::SourceAnchor {
                page: Some(12),
                pages: None,
                region: None,
            }),
            classification: Default::default(),
            extraction: Some(d.extraction("manual_digitisation", None)),
            relation: None,
            connections: Vec::new(),
        };
        let md = crate::artifact::render_artifact_block(
            crate::artifact::ARTIFACT_LEVEL,
            heading,
            &toml,
            &body,
        )
        .unwrap();
        let parsed = crate::artifact::parse_document(&md);
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        parsed.artifacts.into_iter().next().unwrap()
    }

    fn rect_cal() -> PlotCalibration {
        PlotCalibration::AxisAligned {
            x: AxisCalibration::new(
                AxisScale::Linear,
                AxisRef {
                    pixel: 40.5,
                    value: 0.0,
                },
                AxisRef {
                    pixel: 460.25,
                    value: 1200.0,
                },
            )
            .unwrap(),
            y: AxisCalibration::new(
                AxisScale::Logarithmic,
                AxisRef {
                    pixel: 380.0,
                    value: 0.001,
                },
                AxisRef {
                    pixel: 20.0,
                    value: 1000.0,
                },
            )
            .unwrap(),
        }
    }

    #[test]
    fn a_rectangle_save_prefills_every_wizard_field_and_its_exact_calibration() {
        let body = crate::artifact::render_csv_body("Time (h),\"Release, Bq\"\n1,0.5\n2,5\n");
        let art = saved_artifact(rect_cal(), "Fig. 7", body, "Time (h)", "Release, Bq");
        let saved = SavedDigitisation::from_artifact(&art, Some(12), Some("Verfondern".into()));
        let s = &saved.setup;
        assert_eq!(s.figure, "Fig. 7");
        assert_eq!(s.page, "12");
        assert_eq!(s.document_title, "Verfondern");
        assert_eq!((s.x_min.as_str(), s.x_max.as_str()), ("0", "1200"));
        assert_eq!((s.y_min.as_str(), s.y_max.as_str()), ("0.001", "1000"));
        assert!(!s.x_log && s.y_log);
        assert_eq!(s.x_label, "Time (h)");
        assert_eq!(s.y_label, "Release, Bq");
        assert!(s.range_error().is_none() && s.label_error().is_none());
        assert_eq!(saved.reference_pixels(), Some([40.5, 460.25, 380.0, 20.0]));
        assert_eq!(
            saved.saved_calibration(),
            Some(rect_cal()),
            "the calibration round-trips exactly"
        );
        assert!(!saved.is_parallelogram());
        assert_eq!(saved.series.len(), 1);
        assert_eq!(saved.series[0].points, vec![(1.0, 0.5), (2.0, 5.0)]);
        assert_eq!(saved.series[0].skipped_rows, 0);
        let note = s.prefill_note.as_deref().unwrap();
        assert!(!note.contains("Could not read"), "{note}");
        assert_eq!(
            saved.digitised_by.as_deref(),
            Some("tester via kovan (gui, hand-placed)")
        );
    }

    #[test]
    fn a_parallelogram_save_prefills_values_and_says_the_corners_are_missing() {
        let cal = PlotCalibration::Parallelogram(
            ParallelogramCalibration::new(
                [
                    PixelPoint { x: 10.0, y: 10.0 },
                    PixelPoint { x: 110.0, y: 12.0 },
                    PixelPoint { x: 108.0, y: 112.0 },
                    PixelPoint { x: 8.0, y: 110.0 },
                ],
                AxisScale::Linear,
                -5.0,
                5.0,
                AxisScale::Logarithmic,
                1000.0,
                1.0,
            )
            .unwrap(),
        );
        let art = saved_artifact(
            cal,
            "Fig. 2",
            crate::artifact::render_csv_body("a,b\n0,10\n"),
            "a",
            "b",
        );
        let saved = SavedDigitisation::from_artifact(&art, None, None);
        assert_eq!(
            (saved.setup.x_min.as_str(), saved.setup.x_max.as_str()),
            ("-5", "5")
        );
        assert_eq!(
            (saved.setup.y_min.as_str(), saved.setup.y_max.as_str()),
            ("1", "1000")
        );
        assert!(saved.setup.y_log && !saved.setup.x_log);
        assert!(saved.is_parallelogram());
        assert!(
            saved.saved_calibration().is_none(),
            "no corners, no calibration"
        );
        assert!(saved.reference_pixels().is_none());
        assert!(saved
            .setup
            .prefill_note
            .as_deref()
            .unwrap()
            .contains("corners were not saved"));
    }

    #[test]
    fn a_multi_series_body_restores_every_named_curve() {
        let blocks = [
            crate::artifact::SeriesBlock {
                name: "1600 degC".into(),
                csv: "t,f\n1,0.1\n2,0.2\n".into(),
            },
            crate::artifact::SeriesBlock {
                name: "1700 degC".into(),
                csv: "t,f\n1,0.3\n".into(),
            },
        ];
        let body = crate::artifact::render_multi_series_body(&blocks);
        let art = saved_artifact(rect_cal(), "Fig. 9", body, "t", "f");
        let saved = SavedDigitisation::from_artifact(&art, None, None);
        let names: Vec<_> = saved.series.iter().map(|s| s.name.clone()).collect();
        assert_eq!(
            names,
            vec![Some("1600 degC".into()), Some("1700 degC".into())]
        );
        assert_eq!(saved.series[0].points, vec![(1.0, 0.1), (2.0, 0.2)]);
        assert_eq!(saved.series[1].points, vec![(1.0, 0.3)]);
    }

    #[test]
    fn an_unreadable_calibration_leaves_the_range_blank_and_says_so() {
        let mut art = saved_artifact(
            rect_cal(),
            "Fig. 3",
            crate::artifact::render_csv_body("x,y\n1,1\n"),
            "",
            "",
        );
        if let Some(ex) = art.toml.extraction.as_mut() {
            ex.x_axis = Some("garbled".into());
        }
        let saved = SavedDigitisation::from_artifact(&art, None, None);
        assert!(saved.setup.x_min.is_empty() && saved.setup.x_max.is_empty());
        assert_eq!(saved.setup.y_min, "0.001", "the readable axis still fills");
        assert!(
            saved.setup.x_label.is_empty(),
            "the CSV's `x` fallback header is not a label"
        );
        let note = saved.setup.prefill_note.clone().unwrap();
        assert!(
            note.contains("x range") && note.contains("x label"),
            "{note}"
        );
        assert!(saved.saved_calibration().is_none());
    }
}
