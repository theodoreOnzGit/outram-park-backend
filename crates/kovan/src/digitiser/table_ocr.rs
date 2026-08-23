//! Table digitiser — OCR text recognition over a cropped table region,
//! human-reviewed before export (op-hnhp — GitHub issue #30: "draw box,
//! right click, digitise with OCR, check values then export csv or
//! copy/paste").
//!
//! ## Engine decision (op-9bvi)
//!
//! [`kopitiam_ocr`] — a pure-Rust translation of Tesseract's LSTM
//! recognizer (see this crate's `NOTICE` for the full provenance/licensing
//! record; AGPL-3.0-only, same crate-local dependency shape as
//! `kopitiam-pdf`). This is deliberately the **one place** in this crate's
//! digitiser that reaches for anything ML-shaped — the plot digitiser's own
//! "no tick-label OCR" rule is unchanged and still applies to axis values,
//! which a human still supplies. Table *cell text* is different ground,
//! opened explicitly by this decision, and gated the same way the plot
//! digitiser already gates automatic output: [`RecognizedTable`] always
//! starts [`ReviewStatus::Unreviewed`][crate::digitiser::dataset::ReviewStatus],
//! and nothing in this module marks it reviewed — only a human front end
//! calling [`RecognizedTable::record_review`] can.
//!
//! ## What this module does *not* do
//!
//! - **Table structure / column detection.** [`recognize_table`] finds
//!   *text lines* ([`kopitiam_ocr::find_text_lines`]) and splits each line
//!   into cells by a simple heuristic — a run of two or more spaces is a
//!   column boundary (see `split_into_cells`, private below). This is deterministic and
//!   ML-free, matching the workspace's offline-first posture, but it is
//!   **not** real table/border/column detection: a table whose columns
//!   aren't whitespace-separated in the OCR'd text will not split cleanly,
//!   and the operator is expected to catch and fix that during the
//!   mandatory review step, same as the plot digitiser's auto-trace errors
//!   are expected to be caught and hand-corrected.
//! - **Model download.** The `.traineddata` model file must already be on
//!   disk; the operator supplies its path. `kopitiam`'s own OCR pipeline
//!   downloads models on demand into a cache — that download machinery is
//!   not ported here (out of scope for this pass; a natural follow-up if a
//!   model-path text field turns out to be too much friction in practice).

use std::path::Path;

use kopitiam_ocr::{find_text_lines, otsu_binarize, to_gray, LstmRecognizer, RgbImage, TessdataManager};
use serde::{Deserialize, Serialize};

use super::dataset::{utc_now_iso8601, ReviewInterface, ReviewStatus};
use super::DigitiserError;

/// Current `RecognizedTable` schema version.
pub const TABLE_SCHEMA_VERSION: u32 = 1;

/// A recognized table: OCR'd rows of cell text, with the same
/// provenance-and-review discipline the plot digitiser's
/// [`super::dataset::DigitisedDataset`] enforces (`DATA_POLICY.md`:
/// digitisation is a processing step and must be documented as one).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecognizedTable {
    pub schema_version: u32,
    /// SHA-256 of the source crop's pixel data, if known — the same
    /// provenance convention [`super::raster::PlotRaster::source_sha256`]
    /// uses for the plot digitiser.
    pub source_image_sha256: Option<String>,
    /// Free-text note on where the crop came from (e.g. a PDF path and page
    /// number) — filled in by the caller, not derived here.
    pub source_note: Option<String>,
    /// Engine + model identification, e.g. `"kopitiam-ocr 0.1.0 (model:
    /// /path/to/eng.traineddata)"` — recorded so a reviewer can tell which
    /// model produced a given recognition.
    pub engine: String,
    /// Who/when ran the automatic pass (distinct from `review`, which
    /// records who *checked* the result).
    pub recognized_by: String,
    pub recognized_at: String,
    pub review: ReviewStatus,
    /// One row per recognized text line, one cell per whitespace-split
    /// segment (see the module doc's "table structure" limitation).
    pub rows: Vec<Vec<String>>,
}

impl RecognizedTable {
    /// Mark this table reviewed — the plot digitiser's `record_review`
    /// pattern, reused verbatim rather than duplicated via a shared trait
    /// (the workspace's no-trait-objects rule makes a two-line duplication
    /// the simpler, correct choice over an abstraction for one method).
    pub fn record_review(
        &mut self,
        by: impl Into<String>,
        at: impl Into<String>,
        interface: ReviewInterface,
    ) {
        self.review = ReviewStatus::Reviewed { by: by.into(), at: at.into(), interface };
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("RecognizedTable always serialises")
    }

    pub fn write_json(&self, path: &Path) -> Result<(), DigitiserError> {
        std::fs::write(path, self.to_json_string())
            .map_err(|e| DigitiserError::Io(format!("cannot write {}: {e}", path.display())))
    }

    /// Serialise to CSV with the provenance record embedded as `#` comment
    /// header lines — the plot digitiser's
    /// [`super::dataset::DigitisedDataset::to_csv_string`] convention,
    /// reused here so a table export is never separated from where it came
    /// from and whether it has been reviewed.
    pub fn to_csv_string(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "# kovan table-ocr dataset (schema v{})", self.schema_version);
        let _ = writeln!(s, "# engine: {}", self.engine);
        if let Some(note) = &self.source_note {
            let _ = writeln!(s, "# source: {note}");
        }
        let review = match &self.review {
            ReviewStatus::Unreviewed => "UNREVIEWED".to_string(),
            ReviewStatus::Reviewed { by, at, .. } => format!("reviewed by {by} at {at}"),
        };
        let _ = writeln!(s, "# review: {review}");
        for row in &self.rows {
            let cells: Vec<String> = row.iter().map(|c| csv_escape(c)).collect();
            let _ = writeln!(s, "{}", cells.join(","));
        }
        s
    }

    pub fn write_csv(&self, path: &Path) -> Result<(), DigitiserError> {
        std::fs::write(path, self.to_csv_string())
            .map_err(|e| DigitiserError::Io(format!("cannot write {}: {e}", path.display())))
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Run the automatic OCR pass over `image` using the `.traineddata` model
/// at `model_path`: grayscale → Otsu binarize → find text lines → recognize
/// each line → split into cells (module doc's whitespace-run heuristic).
/// Always returns rows marked [`ReviewStatus::Unreviewed`] — nothing in
/// this function, or its callers in this crate, may mark a table reviewed.
///
/// # Errors
///
/// [`DigitiserError::Ocr`] if the model file can't be read/parsed, or a
/// line fails to recognize.
pub fn recognize_table(
    model_path: &Path,
    image: &RgbImage,
    operator: impl Into<String>,
) -> Result<RecognizedTable, DigitiserError> {
    let model_bytes = std::fs::read(model_path)
        .map_err(|e| DigitiserError::Ocr(format!("cannot read {}: {e}", model_path.display())))?;
    let manager = TessdataManager::from_bytes(&model_bytes)
        .map_err(|e| DigitiserError::Ocr(format!("{}: {e}", model_path.display())))?;
    let recognizer = LstmRecognizer::load(&manager)
        .map_err(|e| DigitiserError::Ocr(format!("{}: {e}", model_path.display())))?;

    let gray = to_gray(image);
    let binary = otsu_binarize(&gray);
    let lines = find_text_lines(&binary, &gray);

    let mut rows = Vec::with_capacity(lines.len());
    for line in &lines {
        let text = recognizer
            .recognize_line(line)
            .map_err(|e| DigitiserError::Ocr(format!("line recognition failed: {e}")))?;
        rows.push(split_into_cells(&text));
    }

    Ok(RecognizedTable {
        schema_version: TABLE_SCHEMA_VERSION,
        source_image_sha256: None,
        source_note: None,
        engine: format!("kopitiam-ocr (model: {})", model_path.display()),
        recognized_by: operator.into(),
        recognized_at: utc_now_iso8601(),
        review: ReviewStatus::Unreviewed,
        rows,
    })
}

/// Split one recognized line into cells on runs of 2+ spaces — see the
/// module doc's "table structure" limitation. A single-space gap (an
/// ordinary word boundary) stays inside one cell.
fn split_into_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut space_run = 0usize;
    for c in line.chars() {
        if c == ' ' {
            space_run += 1;
            if space_run == 1 {
                current.push(c);
            }
        } else {
            if space_run >= 2 {
                let trimmed = current.trim_end().to_string();
                if !trimmed.is_empty() {
                    cells.push(trimmed);
                }
                current.clear();
            }
            space_run = 0;
            current.push(c);
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        cells.push(trimmed);
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_two_or_more_spaces_not_single() {
        assert_eq!(
            split_into_cells("Isotope    Half-life   Yield"),
            vec!["Isotope", "Half-life", "Yield"]
        );
        assert_eq!(split_into_cells("Sr-90 decays"), vec!["Sr-90 decays"]);
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        assert_eq!(split_into_cells("  a   b  "), vec!["a", "b"]);
    }

    #[test]
    fn empty_line_produces_no_cells() {
        assert!(split_into_cells("   ").is_empty());
        assert!(split_into_cells("").is_empty());
    }

    #[test]
    fn csv_export_embeds_provenance_and_escapes_commas() {
        let table = RecognizedTable {
            schema_version: TABLE_SCHEMA_VERSION,
            source_image_sha256: None,
            source_note: Some("fig7.pdf page 3".to_string()),
            engine: "kopitiam-ocr (model: eng.traineddata)".to_string(),
            recognized_by: "kovan (gui)".to_string(),
            recognized_at: "2026-08-23T00:00:00Z".to_string(),
            review: ReviewStatus::Unreviewed,
            rows: vec![
                vec!["a, b".to_string(), "c".to_string()],
                vec!["1".to_string(), "2".to_string()],
            ],
        };
        let csv = table.to_csv_string();
        assert!(csv.contains("# review: UNREVIEWED"));
        assert!(csv.contains("# source: fig7.pdf page 3"));
        assert!(csv.contains("\"a, b\",c"));
        assert!(csv.contains("1,2"));
    }

    #[test]
    fn json_round_trips() {
        let table = RecognizedTable {
            schema_version: TABLE_SCHEMA_VERSION,
            source_image_sha256: Some("abc123".to_string()),
            source_note: None,
            engine: "kopitiam-ocr".to_string(),
            recognized_by: "x".to_string(),
            recognized_at: "2026-08-23T00:00:00Z".to_string(),
            review: ReviewStatus::Unreviewed,
            rows: vec![vec!["1".to_string()]],
        };
        let json = table.to_json_string();
        let back: RecognizedTable = serde_json::from_str(&json).unwrap();
        assert_eq!(back, table);
    }
}
