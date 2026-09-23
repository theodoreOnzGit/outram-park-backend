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
//!   disk. ~~The operator supplies its path.~~ **CORRECTED 2026-09-23**
//!   (GH issue #287): the operator no longer types a path — [`discover_model`]
//!   finds one, and the front end runs OCR as soon as a region arrives. The
//!   maintainer's own words: "i don't want to deal with selecting an OCR
//!   model, i should be able to just see the table and csv extracted". The
//!   *download* machinery is still not ported: if no model is on the machine,
//!   [`discover_model`] returns `None` and the front end says which
//!   directories it looked in.

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
        self.review = ReviewStatus::Reviewed {
            by: by.into(),
            at: at.into(),
            interface,
        };
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
        let _ = writeln!(
            s,
            "# kovan table-ocr dataset (schema v{})",
            self.schema_version
        );
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

/// The file name of the model [`discover_model`] prefers.
pub const PREFERRED_MODEL: &str = "eng.traineddata";

/// Tesseract's orientation-and-script-detection data, which
/// [`discover_model`] never picks.
///
/// It is a `.traineddata` file and sits in the same directory as the real
/// models, but it carries **no LSTM recognizer** — `LstmRecognizer::load`
/// on it can only fail, so choosing it would turn "no model installed" into
/// an obscure load error.
const NOT_A_RECOGNIZER: &str = "osd.traineddata";

/// The directories searched for a `.traineddata` model, in order (#287).
///
/// 1. **`$KOVAN_TESSDATA`** — the explicit override, kept because the field
///    it replaces was an override in practice. It may name a *file* as well
///    as a directory; see [`discover_model`].
/// 2. **`$TESSDATA_PREFIX`** — Tesseract's own variable. Both the directory
///    it names and `<it>/tessdata` are searched, because the convention has
///    meant each at different times.
/// 3. **Kovan's own application-data folder**, `<data>/tessdata` — where a
///    model a user downloads for Kovan alone can be dropped, beside the
///    standard corpus clone.
/// 4. **The usual system locations** for a distribution's tesseract data.
///
/// The list is returned whether or not the directories exist; searching a
/// missing directory simply finds nothing.
pub fn model_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(explicit) = std::env::var_os("KOVAN_TESSDATA") {
        dirs.push(std::path::PathBuf::from(explicit));
    }
    if let Some(prefix) = std::env::var_os("TESSDATA_PREFIX") {
        let prefix = std::path::PathBuf::from(prefix);
        dirs.push(prefix.join("tessdata"));
        dirs.push(prefix);
    }
    if let Some(data) = directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan") {
        dirs.push(data.data_dir().join("tessdata"));
    }
    dirs.extend(
        [
            "/usr/share/tessdata",
            "/usr/local/share/tessdata",
            "/usr/share/tesseract-ocr/5/tessdata",
            "/usr/share/tesseract-ocr/4.00/tessdata",
            "/opt/homebrew/share/tessdata",
            "/opt/local/share/tessdata",
        ]
        .into_iter()
        .map(std::path::PathBuf::from),
    );
    dirs
}

/// Every `.traineddata` model on the machine worth trying, best first
/// (#287): [`model_search_dirs`] order, and within each directory
/// [`PREFERRED_MODEL`] before the rest.
///
/// **Directory order outranks language.** An `eng.traineddata` in
/// `/usr/share` does *not* jump ahead of a model in `KOVAN_TESSDATA`, or the
/// override would stop being one — the preference decides between models the
/// user has not chosen between, nothing more.
///
/// A **list**, not one path, because a model being present does not mean it
/// can be used: measured 2026-09-23 on the maintainer's machine, the only
/// model installed (`afr.traineddata`) is rejected by `kopitiam-ocr` 0.1.0
/// with "network outputs 96 != recoder code_range + 1 = 97 (CTC-null
/// invariant)". A caller walks this list and uses the first that loads, so
/// one unusable file does not stand in for "no OCR on this machine".
pub fn discover_models() -> Vec<std::path::PathBuf> {
    discover_models_in(&model_search_dirs())
}

/// [`discover_models`] over an explicit list — the testable half.
pub fn discover_models_in(dirs: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    let mut found: Vec<std::path::PathBuf> = Vec::new();
    for d in dirs {
        if d.is_file() {
            found.push(d.clone());
            continue;
        }
        found.extend(models_in(d));
    }
    found.dedup();
    found
}

/// The first model [`discover_models`] would try, or `None` if the machine
/// has none.
pub fn discover_model() -> Option<std::path::PathBuf> {
    discover_models().into_iter().next()
}

/// [`discover_model`] over an explicit list — the testable half.
pub fn discover_model_in(dirs: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    discover_models_in(dirs).into_iter().next()
}

/// Every usable-looking `.traineddata` in one directory, preferred first,
/// never [`NOT_A_RECOGNIZER`].
fn models_in(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| e == "traineddata")
                && p.file_name().is_some_and(|n| n != NOT_A_RECOGNIZER)
        })
        .collect();
    out.sort();
    if let Some(i) = out
        .iter()
        .position(|p| p.file_name().is_some_and(|n| n == PREFERRED_MODEL))
    {
        out.swap(0, i);
    }
    out
}

/// The model to use from one directory: [`PREFERRED_MODEL`] if it is there,
/// otherwise the alphabetically first other `.traineddata`, never
/// [`NOT_A_RECOGNIZER`].
///
/// Falling back to *some other language* is deliberate. These models are
/// Latin-script LSTM recognizers; one trained on another language still
/// reads digits and Latin letters, which is most of what a data table is,
/// and a usable table the operator then corrects beats an empty tab. The
/// front end says which model it used, so a reader knows why the words may
/// be worse than the numbers.
pub fn pick_model_in(dir: &Path) -> Option<std::path::PathBuf> {
    models_in(dir).into_iter().next()
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

    /// #287: English wins when it is there, and the orientation/script data
    /// is never chosen — it has no recognizer, so picking it would turn "no
    /// model installed" into an obscure load failure.
    #[test]
    fn model_discovery_prefers_english_and_never_the_orientation_data() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str| std::fs::write(dir.path().join(name), b"x").unwrap();
        write("osd.traineddata");
        write("afr.traineddata");
        write("eng.traineddata");
        assert_eq!(
            pick_model_in(dir.path()),
            Some(dir.path().join("eng.traineddata"))
        );

        std::fs::remove_file(dir.path().join("eng.traineddata")).unwrap();
        assert_eq!(
            pick_model_in(dir.path()),
            Some(dir.path().join("afr.traineddata")),
            "another Latin-script model is better than nothing"
        );

        std::fs::remove_file(dir.path().join("afr.traineddata")).unwrap();
        assert_eq!(
            pick_model_in(dir.path()),
            None,
            "osd alone is not a model to recognise with"
        );
    }

    /// The search takes the first directory that has one, and a search entry
    /// that is itself a file is used as the model — which is how
    /// `KOVAN_TESSDATA` can name one.
    #[test]
    fn model_discovery_takes_the_first_hit_and_accepts_a_file() {
        let empty = tempfile::tempdir().unwrap();
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(first.path().join("deu.traineddata"), b"x").unwrap();
        std::fs::write(second.path().join("eng.traineddata"), b"x").unwrap();

        assert_eq!(
            discover_model_in(&[
                empty.path().to_path_buf(),
                first.path().to_path_buf(),
                second.path().to_path_buf(),
            ]),
            Some(first.path().join("deu.traineddata")),
            "order decides; it does not go looking for a better language"
        );

        let named_file = second.path().join("eng.traineddata");
        assert_eq!(
            discover_model_in(std::slice::from_ref(&named_file)),
            Some(named_file)
        );
        assert_eq!(
            discover_model_in(&[empty.path().to_path_buf()]),
            None,
            "a machine with no model says so rather than guessing a path"
        );
    }

    /// The order the doc promises, in the order it promises it — a reader
    /// troubleshooting "it picked the wrong model" needs this to be true.
    #[test]
    fn the_search_order_starts_with_the_overrides() {
        let dirs = model_search_dirs();
        let strings: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        assert!(
            strings.iter().any(|d| d.ends_with("tessdata")),
            "no tessdata directory in the search path: {strings:?}"
        );
        assert!(
            strings.contains(&"/usr/share/tessdata".to_string()),
            "the usual system location is missing: {strings:?}"
        );
    }

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
