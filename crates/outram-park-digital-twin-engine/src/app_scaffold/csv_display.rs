//! Read-only CSV display with a one-click "Copy CSV" button.
//!
//! Replaces the copy-paste-from-screen pattern `ciet_educational_simulator_v2`
//! and `fhr_sim_v2` hand-roll today (see [`super::csv_logging`]'s module doc
//! comment for the survey): each renders its "CSV" as a separate
//! `ui.label(...)` per row inside a `ScrollArea`
//! (`ciet_simulator_v2/app/panels_and_pages/heater_page.rs`'s
//! `ciet_sim_heater_page_csv` is the pattern), so getting the data out means
//! dragging a selection across however many rows happen to be on screen --
//! there is no way to select "all of it" in one gesture, and no copy button
//! at all.
//!
//! This module renders the whole thing as **one** read-only text box (native
//! drag-select and Ctrl+C still work on it, unlike a `ScrollArea` of separate
//! labels) plus a button that copies the entire string to the clipboard in
//! one click. It is generic over what a simulator's CSV columns are --
//! exactly like [`super::csv_logging::CsvLogger`], this module does not know
//! what a "reactor power" or a "fuel temperature" is, only how to join and
//! display strings that are already columns.
//!
//! # Relationship to `csv_logging`
//!
//! [`super::csv_logging::CsvLogger`] writes a real file to disk;
//! [`draw_csv_panel`] shows text on screen. A simulator can use either, both,
//! or neither -- they share no state and do not have to agree on a row
//! format, though [`rows_to_csv_string`] and [`CsvLogger::maybe_write_row`]
//! both take `&[String]`-shaped rows, so the same row-building code can feed
//! both if a caller wants a file **and** an on-screen copyable view.
//!
//! [`CsvLogger::maybe_write_row`]: super::csv_logging::CsvLogger::maybe_write_row
//!
//! # Snapshot-on-click, row cap, and time-interval filtering
//!
//! [`CsvSnapshotPanel`] ports three more behaviours from
//! `ciet_educational_simulator_v2`'s hand-rolled pages -- their "Update CSV
//! Data" button (`ciet_sim_heater_page_graph`) and the
//! `csv_display_interval_seconds`/`graph_data_record_interval_seconds` ratio
//! filter (`ciet_sim_heater_page_csv`) -- as reusable `app_scaffold`
//! infrastructure, generic over any simulator's row-shaped time series, and
//! **enabled by default** (a caller does not opt out of any of the three):
//!
//! 1. **Snapshot on click, not every frame.** [`CsvSnapshotPanel::draw`]
//!    only calls its `fetch_rows` closure -- and only rebuilds the displayed
//!    text -- when the operator clicks "Update CSV Data". Between clicks the
//!    box is frozen: a reader mid-copy-paste does not have the text shift
//!    under them, and the row-filter/join is not repeated every repaint.
//! 2. **Time-interval subsampling**, via
//!    [`CsvSnapshotPanel::display_interval_seconds`]: an operator-adjustable
//!    "CSV Display Interval (Seconds)" slider keeps only rows whose time
//!    (column 0 of each row, assumed seconds -- see
//!    [`filter_rows_by_time_interval`]) has advanced by at least that many
//!    seconds since the last kept row, independent of whatever rate the
//!    source data is actually recorded at. This is CIET's filter; ported
//!    unchanged in spirit (CIET additionally exposes the *recording*
//!    interval as a second slider that mutates the shared sampler directly --
//!    that half is caller-specific plumbing this generic module cannot own,
//!    so it stays the caller's choice whether to expose one).
//! 3. **[`MAX_CSV_ROWS`]**, a hard cap (most-recent rows kept) applied after
//!    the interval filter. Belt-and-suspenders beyond what CIET itself
//!    does -- CIET relies on its 4000-sample ring buffer plus the interval
//!    filter alone -- added here so a very small display interval cannot
//!    still produce an unbounded string.

use egui::Ui;

/// Join `header` and `rows` into CSV text via the `csv` crate, so a value
/// containing a comma or a quote round-trips correctly instead of silently
/// splitting a column -- the same reasoning [`super::csv_logging`]'s module
/// doc comment gives for not hand comma-joining with `+ "," +`.
///
/// # Panics
///
/// Never in practice: writing to an in-memory `Vec<u8>` cannot fail the way
/// writing to a file can, and the `csv` crate only emits valid UTF-8 when fed
/// valid UTF-8 fields (which `String` always is).
pub fn rows_to_csv_string(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record(header)
        .expect("writing a record to an in-memory buffer cannot fail");
    for row in rows {
        writer
            .write_record(row)
            .expect("writing a record to an in-memory buffer cannot fail");
    }
    let bytes = writer
        .into_inner()
        .expect("flushing an in-memory buffer cannot fail");
    String::from_utf8(bytes).expect("the csv crate only emits valid UTF-8 for valid UTF-8 input")
}

/// Draw `title` as a heading with a "Copy CSV" button beside it, then
/// `csv_text` in a scrollable, monospace text box below.
///
/// `id_salt` must be unique among every [`draw_csv_panel`] call active in the
/// same frame (egui's own requirement for any widget holding interaction
/// state) -- pass something like `"heater_csv"` if a page has only one panel,
/// or a value that varies per tab if a page can show several.
///
/// **The box is read-only in practice, not enforced.** It is an
/// [`egui::TextEdit::multiline`] over a throwaway per-frame copy of
/// `csv_text`, which is what lets native drag-select and Ctrl+C work on the
/// *whole* body as one contiguous selection -- a plain [`egui::Label`] is
/// selectable too, but only one label's worth at a time, which is exactly
/// the CIET/FHR pattern this module replaces. Any edit the user makes to the
/// box is discarded the next frame, since `csv_text` (the real data this was
/// built from, typically via [`rows_to_csv_string`]) is never written back --
/// there is nothing for a stray keystroke to corrupt.
pub fn draw_csv_panel(ui: &mut Ui, id_salt: &str, title: &str, csv_text: &str) {
    ui.horizontal(|ui| {
        ui.heading(title);
        if ui.button("\u{1F4CB} Copy CSV").clicked() {
            ui.ctx().copy_text(csv_text.to_string());
        }
    });
    egui::ScrollArea::both().id_salt(id_salt).show(ui, |ui| {
        let mut scratch = csv_text.to_string();
        ui.add(
            egui::TextEdit::multiline(&mut scratch)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(20),
        );
    });
}

/// Hard cap on the number of rows [`CsvSnapshotPanel`] will ever freeze into
/// a CSV, applied after [`filter_rows_by_time_interval`] -- see the module
/// doc comment's "row cap" point. The most **recent** rows are kept, since a
/// time series is normally read for its latest behaviour.
pub const MAX_CSV_ROWS: usize = 1000;

/// [`CsvSnapshotPanel`]'s starting "CSV Display Interval (Seconds)", before
/// an operator has touched the slider -- matches
/// `ciet_educational_simulator_v2`'s own default for the same field
/// (`csv_data.rs`'s `csv_display_interval_seconds: 0.1`).
pub const DEFAULT_CSV_DISPLAY_INTERVAL_SECONDS: f64 = 0.1;

/// Keep only rows whose time -- **column 0 of each row, parsed as seconds**
/// -- has advanced by at least `interval_seconds` since the last kept row.
/// The first row is always kept. Rows whose column 0 does not parse as an
/// `f64` are dropped rather than guessed at.
///
/// This is `ciet_educational_simulator_v2`'s `csv_data_display_interval`
/// ratio filter (`heater_page.rs`'s `ciet_sim_heater_page_csv`), generalised
/// from "keep every Nth *sample*" (which needs to know the recording rate)
/// to "keep every row at least `interval_seconds` newer than the last kept
/// one" (which does not) -- algebraically the same result when the source is
/// sampled at a uniform rate, and better-behaved when it is not.
///
/// `interval_seconds <= 0.0` disables filtering (every row is kept) rather
/// than treating it as "keep nothing" or dividing by zero.
pub fn filter_rows_by_time_interval(
    rows: &[Vec<String>],
    interval_seconds: f64,
) -> Vec<Vec<String>> {
    if interval_seconds <= 0.0 {
        return rows.to_vec();
    }
    let mut kept = Vec::new();
    let mut last_kept_time: Option<f64> = None;
    for row in rows {
        let Some(t) = row.first().and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        let keep = match last_kept_time {
            None => true,
            Some(last) => t - last >= interval_seconds,
        };
        if keep {
            kept.push(row.clone());
            last_kept_time = Some(t);
        }
    }
    kept
}

/// Keep at most `max_rows` rows, dropping from the **front** (oldest) if
/// `rows` is longer -- see [`MAX_CSV_ROWS`].
pub fn cap_row_count(rows: &[Vec<String>], max_rows: usize) -> Vec<Vec<String>> {
    if rows.len() <= max_rows {
        rows.to_vec()
    } else {
        rows[rows.len() - max_rows..].to_vec()
    }
}

/// A CSV panel that snapshots on an "Update CSV Data" click, subsamples by a
/// time interval, and caps its row count -- see the module doc comment's
/// "Snapshot-on-click, row cap, and time-interval filtering" section for the
/// full rationale and the `ciet_educational_simulator_v2` precedent each
/// piece ports.
///
/// Owns its state (the frozen text, the interval setting), so a caller keeps
/// one `CsvSnapshotPanel` per CSV view as a field on its own `eframe::App`
/// struct (or wherever it holds cross-frame GUI state) and calls
/// [`Self::draw`] on it every repaint, the same way it would hold a
/// [`super::plot_history::PlotHistory`] or a [`super::csv_logging::CsvLogger`].
pub struct CsvSnapshotPanel {
    frozen_csv_text: String,
    display_interval_seconds: f64,
}

impl Default for CsvSnapshotPanel {
    fn default() -> Self {
        Self {
            frozen_csv_text: String::new(),
            display_interval_seconds: DEFAULT_CSV_DISPLAY_INTERVAL_SECONDS,
        }
    }
}

impl CsvSnapshotPanel {
    /// A panel with no snapshot yet -- draws as just the header line (via
    /// [`rows_to_csv_string`] on an empty row set) until the first "Update
    /// CSV Data" click.
    pub fn new() -> Self {
        Self::default()
    }

    /// The operator-adjustable subsampling interval currently in effect --
    /// see [`filter_rows_by_time_interval`].
    pub fn display_interval_seconds(&self) -> f64 {
        self.display_interval_seconds
    }

    /// Rebuild the frozen CSV text from `header`/`rows` right now, applying
    /// [`filter_rows_by_time_interval`] then [`cap_row_count`]. Exposed
    /// separately from [`Self::draw`] for callers that want to force a
    /// refresh outside of the button click (e.g. a test, or an initial
    /// snapshot on open) -- normal operation is the button.
    pub fn refresh(&mut self, header: &[&str], rows: &[Vec<String>]) {
        let filtered = filter_rows_by_time_interval(rows, self.display_interval_seconds);
        let capped = cap_row_count(&filtered, MAX_CSV_ROWS);
        self.frozen_csv_text = rows_to_csv_string(header, &capped);
    }

    /// Draw the "Update CSV Data" button, the "CSV Display Interval
    /// (Seconds)" slider, and the frozen CSV text (via [`draw_csv_panel`]).
    ///
    /// `fetch_rows` is only invoked when the button is clicked this frame --
    /// see the module doc comment's "snapshot on click" point. `header` is
    /// read every frame (it is just column names, not the data itself, and
    /// [`draw_csv_panel`] needs a title regardless of whether a refresh
    /// happened).
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        id_salt: &str,
        title: &str,
        header: &[&str],
        fetch_rows: impl FnOnce() -> Vec<Vec<String>>,
    ) {
        ui.horizontal(|ui| {
            if ui.button("Update CSV Data").clicked() {
                let rows = fetch_rows();
                self.refresh(header, &rows);
            }
            ui.add(
                egui::Slider::new(&mut self.display_interval_seconds, 0.05..=1000.0)
                    .logarithmic(true)
                    .text("CSV Display Interval (Seconds)")
                    .drag_value_speed(0.001),
            );
        });
        if self.frozen_csv_text.is_empty() {
            self.frozen_csv_text = rows_to_csv_string(header, &[]);
        }
        draw_csv_panel(ui, id_salt, title, &self.frozen_csv_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: build CSV text from a header and two rows, one of which
    /// carries a value containing a comma, and parse it back with
    /// [`csv::Reader`] -- an independent reader from the writer this module
    /// uses, so a bug in one would not be masked by the same bug in the
    /// other. Checked field-by-field, not just that the reader accepted it.
    ///
    /// Result (2026-08-18): all three rows (header + 2 data rows) round-trip
    /// exactly, including the comma-containing field staying one field.
    #[test]
    fn rows_to_csv_string_round_trips_through_an_independent_reader() {
        let header = ["time_s", "power_mw", "note"];
        let rows = vec![
            vec!["0.0".to_string(), "10.0".to_string(), "steady".to_string()],
            vec![
                "0.1".to_string(),
                "10.5".to_string(),
                "reading, unstable".to_string(),
            ],
        ];
        let csv_text = rows_to_csv_string(&header, &rows);

        let mut reader = csv::ReaderBuilder::new().from_reader(csv_text.as_bytes());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            header.to_vec()
        );
        let records: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 2);
        assert_eq!(&records[0][0], "0.0");
        assert_eq!(&records[0][1], "10.0");
        assert_eq!(&records[0][2], "steady");
        assert_eq!(&records[1][2], "reading, unstable");
    }

    /// Methodology: a header with zero rows must still produce a single
    /// header line, not an empty string or a panic -- the "recording just
    /// started" state every simulator opens in.
    ///
    /// Result (2026-08-18): passes.
    #[test]
    fn header_with_no_rows_produces_just_the_header_line() {
        let csv_text = rows_to_csv_string(&["a", "b"], &[]);
        assert_eq!(csv_text.trim(), "a,b");
    }

    /// Methodology: run [`draw_csv_panel`] inside a real (headless) `egui`
    /// pass -- the same `Context::default()` + `run_ui` harness
    /// `app_scaffold::crash`'s own tests use -- over CSV text built from
    /// [`rows_to_csv_string`], and confirm it does not panic. This is a smoke
    /// test for the widget wiring (unique IDs, valid `TextEdit` state), not a
    /// check on rendered pixels.
    ///
    /// Result (2026-08-18): three passes (zero rows, one row, a row with an
    /// embedded comma) all complete without panicking.
    #[test]
    fn draw_csv_panel_does_not_panic_across_a_headless_egui_pass() {
        for rows in [
            vec![],
            vec![vec!["1.0".to_string(), "2.0".to_string()]],
            vec![vec!["1.0".to_string(), "a, b".to_string()]],
        ] {
            let csv_text = rows_to_csv_string(&["x", "y"], &rows);
            let ctx = egui::Context::default();
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                draw_csv_panel(ui, "test_csv_panel", "Test CSV", &csv_text);
            });
        }
    }

    /// Methodology: five rows at t = 0.0, 0.05, 0.10, 0.15, 0.20 s, filtered
    /// at a 0.1 s interval -- must keep exactly the rows whose time has
    /// advanced at least 0.1 s since the last **kept** row (not the last row
    /// seen), so 0.05 is dropped (only 0.05 s since 0.0) but 0.20 is kept
    /// (0.10 s since the kept 0.10).
    ///
    /// Result (2026-08-18): kept times are exactly [0.0, 0.10, 0.20].
    #[test]
    fn filter_rows_by_time_interval_keeps_rows_advanced_by_at_least_the_interval() {
        let rows: Vec<Vec<String>> = [0.0, 0.05, 0.10, 0.15, 0.20]
            .iter()
            .map(|t| vec![t.to_string(), "x".to_string()])
            .collect();
        let kept = filter_rows_by_time_interval(&rows, 0.1);
        let kept_times: Vec<f64> = kept.iter().map(|r| r[0].parse().unwrap()).collect();
        assert_eq!(kept_times, vec![0.0, 0.10, 0.20]);
    }

    /// Methodology: an interval of `0.0` (and, separately, negative) must
    /// disable filtering entirely -- every row comes back, in order -- per
    /// the function's documented "<= 0.0 disables filtering" contract, not
    /// silently divide-by-zero or drop everything.
    ///
    /// Result (2026-08-18): both cases return all 3 input rows unchanged.
    #[test]
    fn filter_rows_by_time_interval_at_zero_or_negative_keeps_everything() {
        let rows: Vec<Vec<String>> = [0.0, 1.0, 2.0]
            .iter()
            .map(|t| vec![t.to_string()])
            .collect();
        assert_eq!(filter_rows_by_time_interval(&rows, 0.0).len(), 3);
        assert_eq!(filter_rows_by_time_interval(&rows, -5.0).len(), 3);
    }

    /// Methodology: 1500 rows capped at [`MAX_CSV_ROWS`] (1000) must keep
    /// exactly the most **recent** 1000 -- checked by verifying the first
    /// kept row is index 500 (the 1500th row minus 1000), not just that the
    /// length is right.
    ///
    /// Result (2026-08-18): 1000 rows kept, first one is row 500.
    #[test]
    fn cap_row_count_keeps_the_most_recent_rows() {
        let rows: Vec<Vec<String>> = (0..1500).map(|i| vec![i.to_string()]).collect();
        let capped = cap_row_count(&rows, MAX_CSV_ROWS);
        assert_eq!(capped.len(), MAX_CSV_ROWS);
        assert_eq!(capped[0][0], "500");
        assert_eq!(capped[999][0], "1499");
    }

    /// Methodology: fewer rows than the cap must pass through unchanged --
    /// the common case (a simulator that has not run long enough to hit
    /// [`MAX_CSV_ROWS`] yet).
    ///
    /// Result (2026-08-18): all 3 rows kept, unchanged.
    #[test]
    fn cap_row_count_is_a_no_op_under_the_cap() {
        let rows: Vec<Vec<String>> = (0..3).map(|i| vec![i.to_string()]).collect();
        let capped = cap_row_count(&rows, MAX_CSV_ROWS);
        assert_eq!(capped.len(), 3);
    }

    /// Methodology: **the header must always be present** -- before any
    /// "Update CSV Data" click (a fresh [`CsvSnapshotPanel`]) and after a
    /// [`CsvSnapshotPanel::refresh`] with real rows -- checked directly on
    /// the frozen text a caller would see, not inferred from
    /// `rows_to_csv_string`'s own guarantee.
    ///
    /// Result (2026-08-18): both states start with the header line.
    #[test]
    fn csv_snapshot_panel_always_carries_the_header() {
        let header = ["t", "value"];
        let mut panel = CsvSnapshotPanel::new();

        // Before any click: draw() must have populated the header-only text.
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            panel.draw(ui, "test_panel_header", "Test", &header, Vec::new);
        });
        assert!(panel.frozen_csv_text.starts_with("t,value"));

        // After a refresh with real rows: header must still lead the text.
        panel.refresh(&header, &[vec!["1.0".to_string(), "9.9".to_string()]]);
        assert!(panel.frozen_csv_text.starts_with("t,value"));
    }

    /// Methodology: [`CsvSnapshotPanel::refresh`] must apply both the
    /// interval filter and the row cap, in that order -- built from 5 rows
    /// at a 0.1 s interval (keeping 3, per
    /// `filter_rows_by_time_interval_keeps_rows_advanced_by_at_least_the_interval`)
    /// and parsed back with an independent [`csv::Reader`].
    ///
    /// Result (2026-08-18): exactly 3 data rows survive, at the expected
    /// times.
    #[test]
    fn refresh_applies_the_interval_filter() {
        let header = ["t", "value"];
        let rows: Vec<Vec<String>> = [0.0, 0.05, 0.10, 0.15, 0.20]
            .iter()
            .map(|t| vec![t.to_string(), "x".to_string()])
            .collect();

        let mut panel = CsvSnapshotPanel::new();
        panel.refresh(&header, &rows);

        let mut reader = csv::ReaderBuilder::new().from_reader(panel.frozen_csv_text.as_bytes());
        let records: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 3);
        assert_eq!(&records[0][0], "0");
        assert_eq!(&records[1][0], "0.1");
        assert_eq!(&records[2][0], "0.2");
    }
}
