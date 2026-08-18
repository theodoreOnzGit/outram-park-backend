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
}
