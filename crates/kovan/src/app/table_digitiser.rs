//! Table digitiser GUI tab: a LibreOffice-Calc-style grid on the left, the
//! PDF reader on the right (GitHub #353/#356, maintainer direction
//! 2026-09-28: "libreoffice calc cells on the left panel, pdf viewer of
//! table on right hand side, and i slowly use the select text to copy/paste
//! into the libreoffice calc cells").
//!
//! **No OCR.** ~~The region was recognised by `kopitiam-ocr`
//! (op-hnhp, #287).~~ **CORRECTED 2026-09-28**: the OCR engine never loaded a
//! model (#288), so this tab never produced a cell. The text now comes from
//! the PDF's own text layer: in this view the reader selects **characters**
//! and copies them as soon as a drag ends
//! (`PdfReaderState::enter_table_mode`), so the loop is drag over the PDF,
//! then Ctrl+V in the grid. A scanned page has no text layer; its values are
//! typed into the grid while reading the page on the right.
//!
//! The grid itself — cells, cursor, paste, CSV, undo — is
//! [`crate::digitiser::table_grid`], which has no `egui` in it. This file
//! only draws it and turns keys into [`GridCommand`]s, and [`apply`] (the
//! part that decides what a key *does*) is a plain method tested without a
//! window.
//!
//! [`apply`]: TableDigitiserState::apply
//!
//! ## Keys (LibreOffice Calc's)
//!
//! | Key | Not editing | Editing a cell |
//! |---|---|---|
//! | arrows | move (Shift extends the selection) | move the text cursor |
//! | Enter / Shift+Enter | move down / up | commit, then move down / up |
//! | Tab / Shift+Tab | move right / left | commit, then move right / left |
//! | typing | start editing, replacing the cell | type |
//! | F2, double-click | edit, keeping the text | — |
//! | Esc | — | cancel the edit |
//! | Delete, Backspace | clear the selection | edit the text |
//! | Ctrl+V / Ctrl+C | paste at / copy the selection (tab-separated) | into / from the cell text |
//! | Ctrl+Z / Ctrl+Y | undo / redo | — |
//!
//! The grid only takes keys after it was last clicked, so the reader's own
//! arrow-key scrolling keeps working when the PDF was clicked last.
//! When a key (or a paste that selects a block) moves the active cell past
//! the edge of the grid's viewport, the grid scrolls the minimum distance to
//! bring it back into view; it scrolls only on a move, so the mouse wheel is
//! never fought.
//!
//! ## Buttons
//!
//! **Save artifact** (Ctrl+S) saves and returns to the PDF reader.
//! **Cancel digitisation**, beside it, returns to the PDF reader without
//! saving; if the grid holds anything unsaved it first asks "Discard this
//! table?" ([`TableDigitiserState::request_cancel`]).
//! **Format to standard form (E)**, in the toolbar after Undo/Redo, rewrites
//! whole-cell standard form (`2.1×10^6`) in the highlighted cells as E
//! notation (`2.1e6`), one undo step, after an "are you sure?" box that
//! shows each cell before (superscripts raised) and after. Cells draw the
//! reader's `^` marks as real superscripts; the stored text keeps them.
//!
//! ## Column widths and row heights (Calc's)
//!
//! Drag the right edge of a column letter to change that column's width,
//! or the bottom edge of a row number to change that row's height; the
//! pointer turns into a resize cursor over the edge. Double-click a column
//! edge to fit the column to its widest cell (Calc's optimal width), or a
//! row edge to fit the row to its tallest cell. Sizes are presentation
//! only: they are not saved into the CSV or the artifact, and reset when a
//! new table region is loaded. The sizing arithmetic is the egui-free
//! functions [`clamp_size`], [`fit_size`], [`fit_len`], [`sync_sizes`] and
//! [`edge_hit`].

use eframe::egui::{self, Color32, Key, Modifiers, Sense, Stroke, Vec2};

use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::PlotRaster;
use crate::digitiser::table_grid::{
    superscript_segments, CellChange, CellPos, CellRange, Direction, TableGrid,
};
use crate::session::PaperSession;

use super::pdf_reader::CropProvenance;

/// Default width of one grid column, points.
const CELL_WIDTH: f32 = 96.0;
/// Width of the row-number gutter, points.
const GUTTER_WIDTH: f32 = 36.0;
/// Narrowest a column may be dragged, points.
const MIN_CELL_WIDTH: f32 = 24.0;
/// Widest a column may be dragged or fitted, points.
const MAX_CELL_WIDTH: f32 = 1200.0;
/// Tallest a row may be dragged or fitted, points.
const MAX_ROW_HEIGHT: f32 = 400.0;
/// Half-width of the grab zone around a column/row edge, points.
const EDGE_GRAB: f32 = 4.0;
/// Space either side of a cell's text: left inset plus right slack.
const CELL_TEXT_PAD: f32 = 10.0;
/// Font size of the grid's cell text.
const CELL_FONT: f32 = 13.0;

/// Clamp a dragged or fitted size to `[min, max]`.
pub fn clamp_size(size: f32, min: f32, max: f32) -> f32 {
    if size.is_nan() {
        return min;
    }
    size.clamp(min, max.max(min))
}

/// Calc's optimal width/height: the largest measured text extent plus
/// `pad`, clamped to `[min, max]`. An empty line (no text anywhere) gets
/// `default`, as Calc leaves an empty column at the standard width.
pub fn fit_size(
    extents: impl IntoIterator<Item = f32>,
    pad: f32,
    min: f32,
    max: f32,
    default: f32,
) -> f32 {
    let widest = extents
        .into_iter()
        .filter(|e| *e > 0.0)
        .fold(None, |m: Option<f32>, e| Some(m.map_or(e, |m| m.max(e))));
    match widest {
        Some(w) => clamp_size(w + pad, min, max),
        None => clamp_size(default, min, max),
    }
}

/// Make `sizes` exactly `n` long: new trailing entries get `default`, extra
/// ones are dropped. Keeps the sizes in step with a grid that grew on paste
/// or changed shape on undo.
pub fn fit_len(sizes: &mut Vec<f32>, n: usize, default: f32) {
    sizes.resize(n, default);
}

/// Follow a grid insert/delete at index `at` that took a line count from
/// `before` to `after`: one more inserts a `default` entry at `at`, one
/// fewer removes entry `at` (the other sizes keep their lines), anything
/// else just fits the length. `TableGrid::delete_row` on the last row
/// empties it instead of removing it, so `before == after` leaves the
/// sizes alone.
pub fn sync_sizes(sizes: &mut Vec<f32>, before: usize, after: usize, at: usize, default: f32) {
    fit_len(sizes, before, default);
    if after == before + 1 {
        sizes.insert(at.min(sizes.len()), default);
    } else if after + 1 == before && at < sizes.len() {
        sizes.remove(at);
    } else {
        fit_len(sizes, after, default);
    }
}

/// Which edge the pointer is on: the index of the line whose far edge
/// (right edge of a column, bottom edge of a row) lies within `grab` of
/// `pos`, where line 0 starts at `start` and each line is `sizes[i]` long.
/// The nearest edge wins when zones overlap. `None` when on no edge.
pub fn edge_hit(pos: f32, start: f32, sizes: &[f32], grab: f32) -> Option<usize> {
    let mut edge = start;
    let mut best: Option<(usize, f32)> = None;
    for (i, size) in sizes.iter().enumerate() {
        edge += size;
        let d = (pos - edge).abs();
        if d <= grab && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Which way a resize drag runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Col,
    Row,
}

/// A column/row edge being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResizeDrag {
    axis: Axis,
    index: usize,
    /// Pointer coordinate (x for a column, y for a row) where the drag began.
    grab: f32,
    /// The line's size when the drag began.
    start: f32,
}

/// What a key press or click asks the grid to do. Separate from `egui` so
/// [`TableDigitiserState::apply`] can be tested with no window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridCommand {
    /// Move the cursor; `extend` stretches the selection (Shift).
    Move {
        dir: Direction,
        extend: bool,
    },
    /// Commit any edit in progress, then move without extending.
    CommitAndMove(Direction),
    /// Start editing the cursor cell. `Some(text)` replaces its contents
    /// (typing over a cell); `None` keeps them (F2, double-click).
    StartEdit(Option<String>),
    /// Drop the edit in progress and keep the old text.
    CancelEdit,
    /// Empty every selected cell.
    Clear,
    /// Paste tab-separated text at the selection.
    Paste(String),
    Undo,
    Redo,
}

/// An edit in progress: which cell, and its text so far.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CellEdit {
    pos: CellPos,
    text: String,
    /// Focus the text box on the next frame (the frame it appears).
    focus: bool,
}

/// State for the table digitiser tab.
pub struct TableDigitiserState {
    grid: TableGrid,
    edit: Option<CellEdit>,
    /// Whether the grid was clicked more recently than anything outside it,
    /// so keys belong to the grid and not to the PDF reader.
    grid_focused: bool,
    operator: String,
    /// The table's name in the source, e.g. `"Table 5"`: the saved
    /// artifact's heading.
    table_name: String,
    /// Where the table sits in the PDF, from the reader's "Read table" box.
    crop_provenance: Option<CropProvenance>,
    /// A saved table to load back into the grid, by artifact id, resolved on
    /// the next frame that has the paper open (re-opening a saved table
    /// keeps its corrections instead of starting from nothing).
    pending_reload: Option<String>,
    /// The "which table is this?" box shown when a new table region arrives,
    /// before the grid (the table's version of the graph digitiser's setup
    /// wizard). `None` once answered, and never for a re-opened saved table,
    /// which already has a name.
    setup: Option<TableSetup>,
    /// The grid's CSV as last saved or loaded (empty for a new table), so
    /// Cancel knows whether leaving would lose anything.
    last_saved_csv: String,
    /// The "Discard this table?" box is open, after Cancel digitisation was
    /// pressed with unsaved cells.
    confirm_discard: bool,
    /// The "Reformat N cell(s) to E notation?" box is open, previewing these
    /// changes to the highlighted cells.
    pending_reformat: Option<Vec<CellChange>>,
    /// Per-column widths, points; kept as long as the grid is wide.
    /// Presentation only, never saved.
    col_widths: Vec<f32>,
    /// Per-row heights, points; kept as long as the grid is tall.
    /// Presentation only, never saved.
    row_heights: Vec<f32>,
    /// A column/row edge being dragged.
    resize_drag: Option<ResizeDrag>,
    /// The active cell and selection as of the last painted frame, so the
    /// grid scrolls only when they **changed** (see [`follow_moved`]).
    /// Following every frame would pin the view to the cursor and make
    /// wheel-scrolling the grid snap straight back.
    followed: Option<(CellPos, CellRange)>,
    /// The part of the grid's content visible as last painted, in content
    /// coordinates (scroll offset + viewport size). Presentation only; read
    /// by the follow-the-cursor tests.
    grid_view: egui::Rect,
    message: String,
    message_is_error: bool,
}

impl Default for TableDigitiserState {
    fn default() -> Self {
        Self {
            grid: TableGrid::default(),
            edit: None,
            grid_focused: false,
            // op-n0kz: pre-fill from the OS login name where available,
            // same as the graph digitiser's own "your name" field.
            operator: super::default_operator_name(),
            table_name: String::new(),
            crop_provenance: None,
            pending_reload: None,
            setup: None,
            last_saved_csv: String::new(),
            confirm_discard: false,
            pending_reformat: None,
            col_widths: Vec::new(),
            row_heights: Vec::new(),
            resize_drag: None,
            followed: None,
            grid_view: egui::Rect::NOTHING,
            message: String::new(),
            message_is_error: false,
        }
    }
}

/// The answers in the "which table is this?" box.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TableSetup {
    /// The table's designation as printed, e.g. `"Table 5"`. Required.
    name: String,
    /// Why the last Start was refused, if it was.
    error: Option<String>,
    /// Put the cursor in the name box on the frame the box first appears.
    focus: bool,
}

/// What happened in the table view this frame, for the caller to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableOutcome {
    /// Still digitising.
    Continue,
    /// The artifact was saved: close the view and return to the PDF.
    Saved,
    /// The setup box was cancelled, or Cancel digitisation was pressed (and
    /// any unsaved table discarded): drop the region and return to the PDF.
    Cancelled,
}

/// Calc's column name for zero-based column `col`: A..Z, AA..AZ, BA...
pub fn column_name(col: usize) -> String {
    let mut n = col + 1;
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII letters")
}

/// Lay out `text` with the reader's `^` marks drawn as real superscripts
/// ([`superscript_segments`]): `2.1×10^6` shows the `6` raised, in a smaller
/// font. Used for the grid's cells and the reformat box's "Before" column.
fn superscript_job(text: &str, size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (piece, sup) in superscript_segments(text) {
        let format = if sup {
            egui::TextFormat {
                font_id: egui::FontId::proportional(size * 0.7),
                color,
                valign: egui::Align::TOP,
                ..Default::default()
            }
        } else {
            egui::TextFormat::simple(egui::FontId::proportional(size), color)
        };
        job.append(&piece, 0.0, format);
    }
    job
}

impl TableDigitiserState {
    fn set_status(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = false;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = true;
    }

    /// Receive a table region from the PDF reader's "Read table" box. The
    /// raster is not needed any more (there is no OCR), only where the table
    /// is. A **new** table starts from an empty grid; a re-opened saved one
    /// (`source_artifact_id` set) is loaded back from its CSV on the next
    /// frame.
    pub fn load_crop(&mut self, _raster: PlotRaster, provenance: Option<CropProvenance>) {
        self.edit = None;
        self.pending_reload = provenance
            .as_ref()
            .and_then(|p| p.source_artifact_id.clone());
        self.table_name = provenance
            .as_ref()
            .map(|p| p.figure.clone())
            .unwrap_or_default();
        self.crop_provenance = provenance;
        self.grid = TableGrid::default();
        self.reset_sizes();
        self.last_saved_csv.clear();
        self.confirm_discard = false;
        self.grid_focused = true;
        // A new region is asked for its name first, like the graph
        // digitiser's setup wizard; a re-opened saved table already has one.
        self.setup = self.pending_reload.is_none().then(|| TableSetup {
            name: self.table_name.clone(),
            error: None,
            focus: true,
        });
        self.set_status(if self.pending_reload.is_some() {
            "loading the saved table\u{2026}"
        } else {
            "drag over the table text on the right, then Ctrl+V into the grid"
        });
    }

    /// Accept the setup box's answers and go on to the grid. Refused, with
    /// the reason kept for the box to show, while the name is empty: an
    /// artifact heading of "Digitised table" says nothing about which table
    /// the numbers came from.
    fn setup_start(&mut self) -> bool {
        let Some(setup) = self.setup.as_mut() else {
            return true;
        };
        let name = setup.name.trim();
        if name.is_empty() {
            setup.error = Some("say which table this is, as printed (e.g. Table 5)".into());
            return false;
        }
        self.table_name = name.to_string();
        self.setup = None;
        self.grid_focused = true;
        self.set_status("drag over the table text on the right, then Ctrl+V into the grid");
        true
    }

    /// Whether leaving now would lose cells: the grid (with any edit in
    /// progress written in) differs from what was last saved or loaded. An
    /// empty grid has nothing to lose.
    fn has_unsaved_changes(&self) -> bool {
        let csv = match &self.edit {
            Some(edit) => {
                let mut grid = self.grid.clone();
                grid.set(edit.pos.row, edit.pos.col, &edit.text);
                grid.to_csv()
            }
            None => self.grid.to_csv(),
        };
        !csv.is_empty() && csv != self.last_saved_csv
    }

    /// Cancel digitisation was pressed (maintainer, 2026-09-28: "table
    /// digitiser should also have a cancel digitisation option, which brings
    /// us back to pdf reader").
    ///
    /// Returns `true` when the view should close now: the grid is empty or
    /// unchanged since the last save/load, and the per-table state has been
    /// reset. Returns `false` when there are unsaved cells, and opens the
    /// "Discard this table?" box instead, so work is never dropped silently.
    fn request_cancel(&mut self) -> bool {
        if self.has_unsaved_changes() {
            self.confirm_discard = true;
            false
        } else {
            self.reset_table();
            true
        }
    }

    /// Forget the table in hand, so re-entering the tab later does not show
    /// a stale half-done grid. The operator's name is kept.
    fn reset_table(&mut self) {
        self.grid = TableGrid::default();
        self.reset_sizes();
        self.edit = None;
        self.crop_provenance = None;
        self.pending_reload = None;
        self.setup = None;
        self.table_name.clear();
        self.last_saved_csv.clear();
        self.confirm_discard = false;
        self.set_status("");
    }

    /// Back to default column widths and row heights (a new table).
    fn reset_sizes(&mut self) {
        self.col_widths.clear();
        self.row_heights.clear();
        self.resize_drag = None;
    }

    /// Insert or delete a grid row/column through `op`, keeping the matching
    /// size vector in step.
    fn change_shape(
        &mut self,
        axis: Axis,
        at: usize,
        default: f32,
        op: impl FnOnce(&mut TableGrid),
    ) {
        let count = |g: &TableGrid| match axis {
            Axis::Col => g.cols(),
            Axis::Row => g.rows(),
        };
        let before = count(&self.grid);
        op(&mut self.grid);
        let after = count(&self.grid);
        let sizes = match axis {
            Axis::Col => &mut self.col_widths,
            Axis::Row => &mut self.row_heights,
        };
        sync_sizes(sizes, before, after, at, default);
    }

    fn operator_name(&self) -> String {
        let t = self.operator.trim();
        if t.is_empty() {
            "unnamed operator".to_string()
        } else {
            t.to_string()
        }
    }

    /// Carry out one command.
    ///
    /// There is no "mark reviewed" step (maintainer, 2026-09-28: "the
    /// workflow is fully human"). Every value is typed or pasted by the
    /// operator from the PDF, so nothing machine-produced waits on a review
    /// gate, unlike the graph digitiser's traced points.
    pub fn apply(&mut self, command: GridCommand) {
        match command {
            GridCommand::Move { dir, extend } => {
                if self.edit.is_none() {
                    self.grid.move_cursor(dir, extend);
                }
            }
            GridCommand::CommitAndMove(dir) => {
                self.commit_edit();
                self.grid.move_cursor(dir, false);
            }
            GridCommand::StartEdit(replace) => {
                let pos = self.grid.cursor();
                let text = replace.unwrap_or_else(|| self.grid.get(pos.row, pos.col).to_owned());
                self.edit = Some(CellEdit {
                    pos,
                    text,
                    focus: true,
                });
            }
            GridCommand::CancelEdit => self.edit = None,
            GridCommand::Clear => {
                if self.edit.is_none() {
                    self.grid.clear_selection();
                }
            }
            GridCommand::Paste(text) => {
                self.commit_edit();
                self.grid.paste(&text);
            }
            GridCommand::Undo => {
                self.edit = None;
                self.grid.undo();
            }
            GridCommand::Redo => {
                self.edit = None;
                self.grid.redo();
            }
        }
    }

    /// The toolbar's **Format to standard form (E)** (maintainer, 2026-09-28:
    /// "User selects cells, and clicks a format to standard form button,
    /// which then puts in e notation for highlighted cells"). Commits any
    /// edit, then previews the change as a dry run over the **selection**
    /// ([`TableGrid::standard_form_changes`]) and, if anything would change,
    /// opens the "are you sure?" box showing each cell's text before (with
    /// real superscripts) and its E form after (maintainer, same day: "The
    /// wizard then asks in a popup box, are you sure? then displays the
    /// superscripted text before, and e form text after"). With nothing
    /// convertible it only says so, and no box opens.
    fn request_reformat(&mut self) {
        self.commit_edit();
        let changes = self.grid.standard_form_changes();
        if changes.is_empty() {
            self.set_status("no standard-form values in the highlighted cells");
        } else {
            self.pending_reformat = Some(changes);
        }
    }

    /// "Reformat" in the box: rewrite the previewed cells as E notation via
    /// [`TableGrid::reformat_standard_form`] (one undo step; cells with
    /// units are left alone). Returns how many cells changed.
    fn confirm_reformat(&mut self) -> usize {
        if self.pending_reformat.take().is_none() {
            return 0;
        }
        let n = self.grid.reformat_standard_form();
        self.set_status(format!(
            "reformatted {n} cell(s) to E notation \u{2014} Ctrl+Z to undo"
        ));
        n
    }

    /// "Cancel", Esc or clicking outside the box: nothing changes.
    fn cancel_reformat(&mut self) {
        self.pending_reformat = None;
    }

    /// Write the edit in progress into its cell.
    fn commit_edit(&mut self) {
        if let Some(edit) = self.edit.take() {
            self.grid.set(edit.pos.row, edit.pos.col, &edit.text);
        }
    }

    /// Save the grid into the active paper's notes as a `digitised_table`
    /// artifact (GH issue #35), the same way a digitised graph is saved. A
    /// re-opened table replaces its own block rather than adding a second.
    ///
    /// `[extraction] method = "pdf_native"`: the values were lifted from the
    /// PDF's text layer (or typed by a human reading it), and nothing was
    /// recognised. Provenance goes in `[extraction]`, never as `#` lines
    /// inside the CSV fence.
    ///
    /// The body uses the digitised graph's series schema
    /// (`artifact::render_multi_series_body`): the CSV sits between
    /// `### start of data series` / `### Series: <table name>` and
    /// `### end of series` (maintainer, 2026-09-28: "digitised table should
    /// follow the same schema as digitised csv, with ### rather than #"). The
    /// `###` lines are depth 3, inside the `#` artifact block, so they never
    /// split it, and `Artifact::csv_block` still finds the one CSV fence.
    ///
    /// Returns whether the artifact was written.
    fn save_into_project(&mut self, active_paper: Option<&mut PaperSession>) -> bool {
        self.commit_edit();
        let csv = self.grid.to_csv();
        if csv.is_empty() {
            self.set_error("nothing to save \u{2014} the grid is empty");
            return false;
        }
        let Some(session) = active_paper else {
            self.set_error("no paper open: ingest this PDF (or open its paper from the Wiki, Bibliography or Mindmap) so this saves into its notes");
            return false;
        };
        let prov = self.crop_provenance.clone();
        let heading = match self.table_name.trim() {
            "" => "Digitised table".to_string(),
            name => name.to_string(),
        };
        let csv_body = crate::artifact::render_multi_series_body(&[crate::artifact::SeriesBlock {
            name: heading.clone(),
            csv,
        }]);
        let anchor = prov.as_ref().map(|p| crate::artifact::SourceAnchor {
            page: Some((p.page_index + 1) as u32),
            pages: None,
            region: p.region(),
        });
        let replace_id = prov.as_ref().and_then(|p| p.source_artifact_id.clone());
        let mut extraction = crate::artifact::Extraction::new("pdf_native", None);
        extraction.figure = (!self.table_name.trim().is_empty()).then(|| heading.clone());
        extraction.digitised_by = Some(format!(
            "{} via kovan (gui, table grid, every value entered by hand)",
            self.operator_name()
        ));
        extraction.digitised_at = Some(utc_now_iso8601());
        let citekey = session.citekey().to_string();
        let result: Result<String, String> = crate::classify::save_digitised_csv(
            session,
            crate::artifact::ArtifactKind::DigitisedTable,
            &heading,
            anchor,
            Some(extraction),
            replace_id.as_deref(),
            &csv_body,
        )
        .map_err(|e| e.to_string())
        .and_then(|_| {
            session
                .save_document()
                .map(|()| format!("saved into {citekey}'s notes"))
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(m) => {
                self.set_status(m);
                self.last_saved_csv = self.grid.to_csv();
                true
            }
            Err(e) => {
                self.set_error(e);
                false
            }
        }
    }

    /// Load a saved table's CSV back into the grid, once a paper is open.
    fn resolve_reload(&mut self, active_paper: Option<&PaperSession>) {
        let (Some(id), Some(session)) = (self.pending_reload.clone(), active_paper) else {
            return;
        };
        self.pending_reload = None;
        let index = crate::research_record::ResearchRecordIndex::from_session(session);
        let Some(csv) = index.get(&id).and_then(|a| a.csv_block()) else {
            self.set_error(format!(
                "saved table {id} has no CSV to load \u{2014} starting empty"
            ));
            return;
        };
        match TableGrid::from_csv(csv) {
            Ok(grid) => {
                self.grid = grid;
                self.last_saved_csv = self.grid.to_csv();
                self.set_status(format!("loaded saved table {id}"));
            }
            Err(e) => self.set_error(format!("saved table {id}: {e}")),
        }
    }

    /// Turn this frame's input into grid commands. Only called while the grid
    /// has the keyboard.
    fn commands_from_input(&self, ctx: &egui::Context) -> Vec<GridCommand> {
        let editing = self.edit.is_some();
        let mut out = Vec::new();
        ctx.input_mut(|i| {
            let shift = i.modifiers.shift;
            // Tab and Enter first, before the text box or egui's focus
            // traversal can take them.
            for (key, dir, rev) in [
                (Key::Enter, Direction::Down, Direction::Up),
                (Key::Tab, Direction::Right, Direction::Left),
            ] {
                // Shift first: egui matches keys "logically", so a pattern
                // without Shift also matches the key WITH Shift, and checking
                // the plain key first swallowed Shift+Enter / Shift+Tab.
                if i.consume_key(Modifiers::SHIFT, key) {
                    out.push(GridCommand::CommitAndMove(rev));
                }
                if i.consume_key(Modifiers::NONE, key) {
                    out.push(GridCommand::CommitAndMove(dir));
                }
            }
            if editing {
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    out.push(GridCommand::CancelEdit);
                }
                return;
            }
            // Arrows are CONSUMED, not just read: the PDF reader beside the
            // grid nudges its view on bare arrow keys, and it is drawn after
            // this panel, so a consumed arrow never reaches it (maintainer,
            // 2026-09-28: "when i am moving my arrow keys in the excel, the
            // pdf shouldnt move"). Only the arrows: mouse panning and the
            // reader's other keys are untouched.
            for (key, dir) in [
                (Key::ArrowUp, Direction::Up),
                (Key::ArrowDown, Direction::Down),
                (Key::ArrowLeft, Direction::Left),
                (Key::ArrowRight, Direction::Right),
            ] {
                // Shift first, for the reason given at Enter/Tab above.
                if i.consume_key(Modifiers::SHIFT, key) {
                    out.push(GridCommand::Move { dir, extend: true });
                }
                if i.consume_key(Modifiers::NONE, key) {
                    out.push(GridCommand::Move { dir, extend: false });
                }
            }
            if i.key_pressed(Key::F2) {
                out.push(GridCommand::StartEdit(None));
            }
            if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                out.push(GridCommand::Clear);
            }
            if i.modifiers.command && i.key_pressed(Key::Z) {
                out.push(if shift {
                    GridCommand::Redo
                } else {
                    GridCommand::Undo
                });
            }
            if i.modifiers.command && i.key_pressed(Key::Y) {
                out.push(GridCommand::Redo);
            }
            for event in &i.events {
                match event {
                    egui::Event::Paste(text) => out.push(GridCommand::Paste(text.clone())),
                    egui::Event::Text(text) if !i.modifiers.command => {
                        out.push(GridCommand::StartEdit(Some(text.clone())));
                    }
                    _ => {}
                }
            }
        });
        out
    }

    /// Draw the grid panel (the left side of the split).
    ///
    /// The table is saved as an **artifact** in the paper's notes, never
    /// exported to a loose `.csv` file (maintainer, 2026-09-28: "i don't want
    /// to export to csv, i want to save artifact"). Save sits at the top and
    /// Ctrl+S does the same.
    ///
    /// Returns [`TableOutcome::Saved`] the frame a save succeeds, so the
    /// caller can close this view and go back to the PDF (maintainer,
    /// 2026-09-28: "save artifact for table should close the table digitiser
    /// and return to pdf viewer"), and [`TableOutcome::Cancelled`] when the
    /// setup box is backed out of or **Cancel digitisation** is pressed
    /// (after the "Discard this table?" box, if anything is unsaved; see
    /// [`Self::request_cancel`]).
    ///
    /// A new region first shows a "which table is this?" box (maintainer,
    /// 2026-09-28: "a similar wizard as the digitised graph ... the first
    /// popup box asks you what table is this. Then it brings you straight to
    /// the digitiser"). The graph wizard's other stages (axis ranges and
    /// labels) have no table equivalent, so it is one question.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        active_paper: Option<&mut PaperSession>,
    ) -> TableOutcome {
        let mut saved = false;
        let mut cancelled = false;
        self.resolve_reload(active_paper.as_deref());
        if self.setup.is_some() {
            let citekey = active_paper.as_ref().map(|s| s.citekey().to_string());
            return self.setup_ui(ui.ctx(), citekey.as_deref());
        }

        // Keys belong to the grid only while it was the last thing clicked.
        let panel = ui.max_rect();
        if let Some(press) = ui.ctx().input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.press_origin())
                .flatten()
        }) {
            self.grid_focused = panel.contains(press);
        }
        if self.grid_focused && !self.confirm_discard && self.pending_reformat.is_none() {
            for command in self.commands_from_input(ui.ctx()) {
                self.apply(command);
            }
            if self.edit.is_none() && ui.ctx().input(|i| i.events.contains(&egui::Event::Copy)) {
                ui.ctx().copy_text(self.grid.copy_selection());
            }
        }

        let save_key = ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S));
        ui.horizontal(|ui| {
            ui.heading("Table digitiser");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let citekey = active_paper.as_ref().map(|s| s.citekey().to_string());
                let button =
                    egui::Button::new(egui::RichText::new("\u{1F4BE} Save artifact").strong())
                        .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.6));
                let hover = match &citekey {
                    Some(c) => format!(
                        "save into {c}'s notes as a digitised_table artifact \
                         (method = pdf_native). Ctrl+S"
                    ),
                    None => "no paper open: ingest this PDF, or open its paper from the \
                             Wiki, Bibliography or Mindmap"
                        .to_string(),
                };
                let clicked = ui
                    .add_enabled(citekey.is_some(), button)
                    .on_hover_text(&hover)
                    .on_disabled_hover_text(&hover)
                    .clicked();
                if clicked || save_key {
                    saved = self.save_into_project(active_paper);
                }
                // Secondary to Save: a plain button, to its left.
                if ui
                    .button("\u{2716} Cancel digitisation")
                    .on_hover_text(
                        "go back to the PDF reader without saving \
                         (asks first if the grid has unsaved cells)",
                    )
                    .clicked()
                    && !saved
                {
                    cancelled = self.request_cancel();
                }
            });
        });
        if self.confirm_discard {
            cancelled = self.confirm_discard_ui(ui.ctx());
        }
        if cancelled {
            return TableOutcome::Cancelled;
        }
        ui.small(
            "Drag over the table text in the PDF on the right (it is copied at once), \
             click a cell here, Ctrl+V. Enter/Tab move like LibreOffice Calc. No OCR: \
             a scanned page's values are typed in.",
        );
        if self.message_is_error {
            egui::Frame::new()
                .fill(Color32::from_rgb(120, 30, 30))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.colored_label(Color32::WHITE, format!("\u{26A0} {}", self.message));
                });
        } else if !self.message.is_empty() {
            ui.label(&self.message);
        }

        ui.horizontal(|ui| {
            ui.label("table name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.table_name)
                    .hint_text("e.g. Table 5")
                    .desired_width(120.0),
            );
            ui.label("your name*:");
            ui.add(egui::TextEdit::singleline(&mut self.operator).desired_width(120.0));
        });
        // Default row height: one line of body text plus a little air;
        // the minimum is the text itself.
        let text_h = ui.text_style_height(&egui::TextStyle::Body);
        let default_row_h = text_h + 6.0;
        let min_row_h = text_h + 2.0;
        ui.horizontal(|ui| {
            let at = self.grid.cursor();
            if ui
                .button("+ row")
                .on_hover_text("insert a row above the cursor")
                .clicked()
            {
                self.change_shape(Axis::Row, at.row, default_row_h, |g| g.insert_row(at.row));
            }
            if ui
                .button("+ col")
                .on_hover_text("insert a column left of the cursor")
                .clicked()
            {
                self.change_shape(Axis::Col, at.col, CELL_WIDTH, |g| g.insert_col(at.col));
            }
            if ui.button("\u{2212} row").clicked() {
                self.change_shape(Axis::Row, at.row, default_row_h, |g| g.delete_row(at.row));
            }
            if ui.button("\u{2212} col").clicked() {
                self.change_shape(Axis::Col, at.col, CELL_WIDTH, |g| g.delete_col(at.col));
            }
            if ui.button("Undo").clicked() {
                self.apply(GridCommand::Undo);
            }
            if ui.button("Redo").clicked() {
                self.apply(GridCommand::Redo);
            }
            if ui
                .button("Format to standard form (E)")
                .on_hover_text(
                    "select cells first \u{2014} turns whole-cell standard form \
                     (2.1\u{00D7}10^6) in the highlighted cells into E notation (2.1e6); \
                     cells with units such as 1.0\u{00D7}10^5 m^2 are left alone. \
                     Ctrl+Z undoes",
                )
                .clicked()
            {
                self.request_reformat();
            }
            ui.label(format!("{}{}", column_name(at.col), at.row + 1));
        });
        if self.pending_reformat.is_some() {
            self.reformat_ui(ui.ctx());
        }
        ui.separator();

        // The cell editor/viewer, with room left below it for the actions.
        let actions_height = 110.0;
        let grid_height = (ui.available_height() - actions_height).max(120.0);
        let mut clicked: Option<(CellPos, bool, bool)> = None; // (pos, shift, double)

        // **Follow the cursor** (maintainer, 2026-09-28: "when the cursor
        // moves beyond the scroll area ... the scrollbar shld follow"). An
        // arrow, Enter or Tab past the edge of the viewport -- or a paste
        // that selects a block -- used to leave the active cell off-screen,
        // because these cells are painted rectangles and the `ScrollArea`
        // has no idea which one is active. Only a CHANGE scrolls.
        let follow = follow_moved(
            &mut self.followed,
            (self.grid.cursor(), self.grid.selection()),
        );
        let grid_out = egui::ScrollArea::both()
            .id_salt("table_grid_scroll")
            .max_height(grid_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Sizes follow the grid's shape (it grows on paste and
                // changes on undo); insert/delete keep them aligned.
                fit_len(&mut self.col_widths, self.grid.cols(), CELL_WIDTH);
                fit_len(&mut self.row_heights, self.grid.rows(), default_row_h);
                let widths = self.col_widths.clone();
                let heights = self.row_heights.clone();
                let header_h = default_row_h;
                let origin = ui.cursor().min;
                let sel = self.grid.selection();
                let cursor = self.grid.cursor();
                let visuals = ui.visuals().clone();
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                // Selection colours: the theme's selection colour, strong
                // enough to stand out from the unselected cells in both
                // light and dark themes (maintainer, 2026-09-28: "the table
                // i select should have contrasting highlight with the
                // surrounding cells").
                let sel_rgb = visuals.selection.bg_fill;
                let cell_on =
                    Color32::from_rgba_unmultiplied(sel_rgb.r(), sel_rgb.g(), sel_rgb.b(), 150);
                let header_on =
                    Color32::from_rgba_unmultiplied(sel_rgb.r(), sel_rgb.g(), sel_rgb.b(), 110);
                let strong = visuals.strong_text_color();
                let edge = Stroke::new(2.0, visuals.selection.stroke.color);
                // Header row: corner, then column letters.
                ui.horizontal(|ui| {
                    ui.allocate_exact_size(Vec2::new(GUTTER_WIDTH, header_h), Sense::hover());
                    for (c, &w) in widths.iter().enumerate() {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(w, header_h), Sense::hover());
                        // Calc lights up the headers of the selected
                        // columns and rows, so the block is findable at a
                        // glance even when scrolled.
                        let on = (sel.left..=sel.right).contains(&c);
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            if on {
                                header_on
                            } else {
                                visuals.faint_bg_color
                            },
                        );
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(0.5, visuals.weak_text_color()),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            column_name(c),
                            egui::FontId::proportional(12.0),
                            if on { strong } else { visuals.text_color() },
                        );
                    }
                });
                for (r, &row_h) in heights.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(GUTTER_WIDTH, row_h), Sense::hover());
                        let on = (sel.top..=sel.bottom).contains(&r);
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            if on {
                                header_on
                            } else {
                                visuals.faint_bg_color
                            },
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            (r + 1).to_string(),
                            egui::FontId::proportional(12.0),
                            if on { strong } else { visuals.text_color() },
                        );
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(0.5, visuals.weak_text_color()),
                            egui::StrokeKind::Inside,
                        );
                        for (c, &w) in widths.iter().enumerate() {
                            let pos = CellPos::new(r, c);
                            let size = Vec2::new(w, row_h);
                            if let Some(edit) = self.edit.as_mut().filter(|e| e.pos == pos) {
                                let response = ui.add_sized(
                                    size,
                                    egui::TextEdit::singleline(&mut edit.text)
                                        .margin(Vec2::new(3.0, 2.0)),
                                );
                                if edit.focus {
                                    response.request_focus();
                                    edit.focus = false;
                                }
                                if follow && pos == cursor {
                                    ui.scroll_to_rect(response.rect, None);
                                }
                                continue;
                            }
                            let (rect, response) =
                                ui.allocate_exact_size(size, Sense::click_and_drag());
                            if follow && pos == cursor {
                                // `Align::None`: the minimum distance, so a
                                // cell already on screen moves nothing.
                                ui.scroll_to_rect(rect, None);
                            }
                            let selected = sel.contains(pos);
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                if selected {
                                    cell_on
                                } else {
                                    visuals.extreme_bg_color
                                },
                            );
                            ui.painter().rect_stroke(
                                rect,
                                0.0,
                                Stroke::new(0.5, visuals.weak_text_color()),
                                egui::StrokeKind::Inside,
                            );
                            // Outline the selected block as one shape: an
                            // edge only where a selected cell meets an
                            // unselected one.
                            if selected {
                                let p = ui.painter();
                                let (tl, tr) = (rect.left_top(), rect.right_top());
                                let (bl, br) = (rect.left_bottom(), rect.right_bottom());
                                if r == sel.top {
                                    p.line_segment([tl, tr], edge);
                                }
                                if r == sel.bottom {
                                    p.line_segment([bl, br], edge);
                                }
                                if c == sel.left {
                                    p.line_segment([tl, bl], edge);
                                }
                                if c == sel.right {
                                    p.line_segment([tr, br], edge);
                                }
                            }
                            if pos == cursor {
                                // The active cell: a heavy border in the
                                // strongest text colour, so it reads even
                                // inside a large selected block.
                                ui.painter().rect_stroke(
                                    rect.shrink(1.0),
                                    1.0,
                                    Stroke::new(2.5, strong),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let text = self.grid.get(r, c);
                            if !text.is_empty() {
                                // `^` marks from the reader show as real
                                // superscripts; the stored text keeps them.
                                let color = if selected {
                                    strong
                                } else {
                                    visuals.text_color()
                                };
                                let galley = ui
                                    .painter()
                                    .layout_job(superscript_job(text, CELL_FONT, color));
                                // Vertically centred in a tall row; wide
                                // text clips at the cell edge.
                                let at =
                                    rect.left_center() + Vec2::new(4.0, -galley.size().y / 2.0);
                                ui.painter()
                                    .with_clip_rect(rect.shrink(2.0))
                                    .galley(at, galley, color);
                            }
                            if response.double_clicked() {
                                clicked = Some((pos, false, true));
                            } else if response.clicked() || response.drag_started() {
                                let shift = ui.input(|i| i.modifiers.shift);
                                clicked = Some((pos, shift, false));
                            } else if ui.input(|i| i.pointer.primary_down())
                                && self.resize_drag.is_none()
                                && self.grid_focused
                                && ui.rect_contains_pointer(rect)
                                && ui.input(|i| i.pointer.is_decidedly_dragging())
                            {
                                // Drag across cells to select a block, as in
                                // Calc.
                                clicked = Some((pos, true, false));
                            }
                        }
                    });
                }
                // The column/row edges go on top of the headers they sit on.
                self.resize_ui(ui, origin, header_h, default_row_h, min_row_h);
            });
        self.grid_view =
            egui::Rect::from_min_size(grid_out.state.offset.to_pos2(), grid_out.inner_rect.size());
        if let Some((pos, shift, double)) = clicked {
            // Clicking elsewhere commits the edit first, as in Calc.
            self.commit_edit();
            self.grid.set_cursor(pos, shift);
            self.grid_focused = true;
            if double {
                self.apply(GridCommand::StartEdit(None));
            }
        }

        ui.separator();
        if let Some(prov) = &self.crop_provenance {
            ui.small(format!(
                "from page {}, {}",
                prov.page_index + 1,
                prov.author
            ));
        }
        if saved {
            TableOutcome::Saved
        } else {
            TableOutcome::Continue
        }
    }

    /// Drag/double-click handling for the column-header and row-number
    /// edges, Calc's way: drag the right edge of a column letter or the
    /// bottom edge of a row number to resize; double-click it to fit the
    /// contents. `origin` is the grid's top-left corner (the gutter's
    /// corner cell).
    fn resize_ui(
        &mut self,
        ui: &mut egui::Ui,
        origin: egui::Pos2,
        header_h: f32,
        default_row_h: f32,
        min_row_h: f32,
    ) {
        let x0 = origin.x + GUTTER_WIDTH;
        let y0 = origin.y + header_h;
        let total_w: f32 = self.col_widths.iter().sum();
        let total_h: f32 = self.row_heights.iter().sum();
        let hover = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| ui.clip_rect().contains(*p));
        let press = ui.input(|i| i.pointer.press_origin());
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        for axis in [Axis::Col, Axis::Row] {
            let (band, start, name) = match axis {
                Axis::Col => (
                    egui::Rect::from_min_max(
                        egui::pos2(x0, origin.y),
                        egui::pos2(x0 + total_w + EDGE_GRAB, y0),
                    ),
                    x0,
                    "table_col_edges",
                ),
                Axis::Row => (
                    egui::Rect::from_min_max(
                        egui::pos2(origin.x, y0),
                        egui::pos2(x0, y0 + total_h + EDGE_GRAB),
                    ),
                    y0,
                    "table_row_edges",
                ),
            };
            let coord = |p: egui::Pos2| match axis {
                Axis::Col => p.x,
                Axis::Row => p.y,
            };
            let response = ui.interact(band, ui.id().with(name), Sense::click_and_drag());
            let sizes = match axis {
                Axis::Col => &self.col_widths,
                Axis::Row => &self.row_heights,
            };
            let hit_at = |p: egui::Pos2| {
                band.contains(p)
                    .then(|| edge_hit(coord(p), start, sizes, EDGE_GRAB))
                    .flatten()
            };
            let hovering = hover.and_then(hit_at);
            let pressed = press.and_then(hit_at);
            let dragging = self.resize_drag.filter(|d| d.axis == axis);
            if hovering.is_some() || dragging.is_some() {
                ui.ctx().set_cursor_icon(match axis {
                    Axis::Col => egui::CursorIcon::ResizeHorizontal,
                    Axis::Row => egui::CursorIcon::ResizeVertical,
                });
            }
            if response.double_clicked() {
                if let Some(index) = pressed {
                    self.resize_drag = None;
                    self.auto_fit(ui, axis, index, default_row_h, min_row_h);
                }
            } else if response.drag_started() {
                if let (Some(index), Some(p)) = (pressed, press) {
                    self.resize_drag = Some(ResizeDrag {
                        axis,
                        index,
                        grab: coord(p),
                        start: sizes[index],
                    });
                }
            }
            if let Some(drag) = self.resize_drag.filter(|d| d.axis == axis) {
                if let (true, Some(p)) = (response.dragged(), pointer) {
                    let (sizes, min, max) = match axis {
                        Axis::Col => (&mut self.col_widths, MIN_CELL_WIDTH, MAX_CELL_WIDTH),
                        Axis::Row => (&mut self.row_heights, min_row_h, MAX_ROW_HEIGHT),
                    };
                    if let Some(size) = sizes.get_mut(drag.index) {
                        *size = clamp_size(drag.start + coord(p) - drag.grab, min, max);
                    }
                }
                if response.drag_stopped() || !primary_down {
                    self.resize_drag = None;
                }
            }
        }
    }

    /// Calc's optimal width (column) or height (row): fit line `index` to
    /// its widest/tallest cell text as drawn, superscripts included.
    fn auto_fit(
        &mut self,
        ui: &egui::Ui,
        axis: Axis,
        index: usize,
        default_row_h: f32,
        min_row_h: f32,
    ) {
        let color = ui.visuals().text_color();
        let measure = |text: &str| {
            let size = ui
                .painter()
                .layout_job(superscript_job(text, CELL_FONT, color))
                .size();
            match axis {
                Axis::Col => size.x,
                Axis::Row => size.y,
            }
        };
        match axis {
            Axis::Col => {
                let extents: Vec<f32> = (0..self.grid.rows())
                    .map(|r| self.grid.get(r, index))
                    .filter(|t| !t.is_empty())
                    .map(measure)
                    .collect();
                if let Some(w) = self.col_widths.get_mut(index) {
                    *w = fit_size(
                        extents,
                        CELL_TEXT_PAD,
                        MIN_CELL_WIDTH,
                        MAX_CELL_WIDTH,
                        CELL_WIDTH,
                    );
                }
            }
            Axis::Row => {
                let extents: Vec<f32> = (0..self.grid.cols())
                    .map(|c| self.grid.get(index, c))
                    .filter(|t| !t.is_empty())
                    .map(measure)
                    .collect();
                if let Some(h) = self.row_heights.get_mut(index) {
                    *h = fit_size(extents, 6.0, min_row_h, MAX_ROW_HEIGHT, default_row_h);
                }
            }
        }
    }

    /// The "Reformat N cell(s) to E notation?" box: one row per previewed
    /// cell, its name, the text before (superscripts drawn as such) and the
    /// E form after.
    fn reformat_ui(&mut self, ctx: &egui::Context) {
        let Some(changes) = self.pending_reformat.as_ref() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("table-digitiser-reformat")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            let text_color = ui.visuals().text_color();
            ui.heading(format!("Reformat {} cell(s) to E notation?", changes.len()));
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    egui::Grid::new("table-digitiser-reformat-grid")
                        .striped(true)
                        .spacing(Vec2::new(16.0, 4.0))
                        .show(ui, |ui| {
                            ui.label("");
                            ui.strong("Before");
                            ui.strong("After");
                            ui.end_row();
                            for ch in changes {
                                ui.weak(format!("{}{}", column_name(ch.pos.col), ch.pos.row + 1));
                                ui.label(superscript_job(&ch.before, 14.0, text_color));
                                ui.monospace(&ch.after);
                                ui.end_row();
                            }
                        });
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Reformat").clicked() {
                    confirm = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if confirm {
            self.confirm_reformat();
        } else if cancel || modal.should_close() {
            self.cancel_reformat();
        }
    }

    /// The "Discard this table?" box. Returns `true` when the table was
    /// discarded (the view should close); Esc, clicking outside or "Keep
    /// editing" close the box and leave the grid as it was.
    fn confirm_discard_ui(&mut self, ctx: &egui::Context) -> bool {
        let mut discard = false;
        let mut keep = false;
        let modal = egui::Modal::new(egui::Id::new("table-digitiser-discard")).show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.heading("Discard this table?");
            ui.add_space(4.0);
            ui.label("The grid has cells that have not been saved as an artifact.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Discard and go back").clicked() {
                    discard = true;
                }
                if ui.button("Keep editing").clicked() {
                    keep = true;
                }
            });
        });
        if discard || keep || modal.should_close() {
            return self.answer_discard(discard);
        }
        false
    }

    /// The answer to "Discard this table?": `true` discards (resets the table
    /// and returns `true`, leave now); `false` is "Keep editing" (closes the
    /// box, grid untouched, returns `false`).
    fn answer_discard(&mut self, discard: bool) -> bool {
        if discard {
            self.reset_table();
        } else {
            self.confirm_discard = false;
        }
        discard
    }

    /// The "which table is this?" box, drawn over the view until answered.
    fn setup_ui(&mut self, ctx: &egui::Context, citekey: Option<&str>) -> TableOutcome {
        let page = self.crop_provenance.as_ref().map(|p| p.page_index + 1);
        let mut outcome = TableOutcome::Continue;
        let mut start = false;
        let modal = egui::Modal::new(egui::Id::new("table-digitiser-setup")).show(ctx, |ui| {
            let Some(setup) = self.setup.as_mut() else {
                return;
            };
            ui.set_max_width(420.0);
            ui.heading("Which table is this?");
            ui.add_space(4.0);
            let mut source = String::new();
            if let Some(p) = page {
                source.push_str(&format!("page {p}"));
            }
            if let Some(c) = citekey {
                if !source.is_empty() {
                    source.push_str(" \u{00B7} ");
                }
                source.push_str(c);
            }
            if !source.is_empty() {
                ui.weak(source);
            }
            ui.add_space(6.0);
            ui.label("Table, as printed*:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut setup.name)
                    .hint_text("e.g. Table 5")
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut setup.focus) {
                response.request_focus();
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                start = true;
            }
            if let Some(e) = &setup.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Start digitising").clicked() {
                    start = true;
                }
                if ui.button("Cancel").clicked() {
                    outcome = TableOutcome::Cancelled;
                }
            });
        });
        if modal.should_close() {
            outcome = TableOutcome::Cancelled;
        }
        if start && self.setup_start() {
            return TableOutcome::Continue;
        }
        if outcome == TableOutcome::Cancelled {
            self.reset_table();
        }
        outcome
    }
}

/// Whether the grid's active cell or selection changed since the frame that
/// last recorded `last`, recording `now` either way. The first frame counts
/// as a change. This is the whole of the "only scroll when the cursor
/// moved" rule, kept out of the painter so it can be tested plainly.
fn follow_moved(last: &mut Option<(CellPos, CellRange)>, now: (CellPos, CellRange)) -> bool {
    let moved = *last != Some(now);
    *last = Some(now);
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, crate::root::KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = crate::root::KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false)
            .unwrap();
        (dir, root)
    }

    fn paper(root: &crate::root::KovanRoot) -> PaperSession {
        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();
        PaperSession::open(root, "wang2018multiphysics").unwrap()
    }

    #[test]
    fn a_new_region_asks_which_table_first_and_needs_an_answer() {
        let mut s = TableDigitiserState::default();
        let raster = PlotRaster::from_rgb_fn(4, 4, |_, _| [255, 255, 255]);
        s.load_crop(raster, None);
        assert!(s.setup.is_some(), "a new region opens the setup box");
        assert!(!s.setup_start(), "an empty name is refused");
        assert!(s.setup.as_ref().unwrap().error.is_some());
        s.setup.as_mut().unwrap().name = "  Table 5 ".into();
        assert!(s.setup_start());
        assert!(s.setup.is_none());
        assert_eq!(s.table_name, "Table 5");
    }

    #[test]
    fn a_reopened_saved_table_skips_the_setup_box() {
        let mut s = TableDigitiserState::default();
        let raster = PlotRaster::from_rgb_fn(4, 4, |_, _| [255, 255, 255]);
        let prov = CropProvenance {
            page_index: 2,
            min: egui::Pos2::ZERO,
            max: egui::Pos2::new(10.0, 10.0),
            page_px: [100.0, 100.0],
            created_at: String::new(),
            author: String::new(),
            figure: "Table 3".into(),
            source_artifact_id: Some("table-3".into()),
        };
        s.load_crop(raster, Some(prov));
        assert!(s.setup.is_none());
        assert_eq!(s.table_name, "Table 3");
    }

    /// Escape (or clicking outside) cancels the box through egui's own
    /// modal, headless.
    #[test]
    fn escape_on_the_setup_box_cancels_back_to_the_pdf() {
        let ctx = egui::Context::default();
        let mut s = TableDigitiserState::default();
        s.load_crop(PlotRaster::from_rgb_fn(4, 4, |_, _| [255, 255, 255]), None);
        let mut outcome = TableOutcome::Continue;
        for press_escape in [false, true] {
            let mut input = egui::RawInput::default();
            if press_escape {
                input.events.push(egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
            let _ = ctx.run_ui(input, |ui| {
                outcome = s.ui(ui, None);
            });
        }
        assert_eq!(outcome, TableOutcome::Cancelled);
        assert!(s.setup.is_none());
    }

    #[test]
    fn sizes_clamp_to_their_bounds() {
        assert_eq!(
            clamp_size(10.0, MIN_CELL_WIDTH, MAX_CELL_WIDTH),
            MIN_CELL_WIDTH
        );
        assert_eq!(clamp_size(150.0, MIN_CELL_WIDTH, MAX_CELL_WIDTH), 150.0);
        assert_eq!(
            clamp_size(1e6, MIN_CELL_WIDTH, MAX_CELL_WIDTH),
            MAX_CELL_WIDTH
        );
        assert_eq!(clamp_size(f32::NAN, 24.0, 100.0), 24.0);
        // A max below the min cannot make the size smaller than the min.
        assert_eq!(clamp_size(5.0, 24.0, 10.0), 24.0);
    }

    #[test]
    fn auto_fit_takes_the_widest_text_plus_padding() {
        assert_eq!(
            fit_size([30.0, 120.0, 60.0], 10.0, 24.0, 1200.0, 96.0),
            130.0
        );
        // Narrow text still gets at least the minimum.
        assert_eq!(fit_size([4.0], 10.0, 24.0, 1200.0, 96.0), 24.0);
        // An empty column goes back to the default width.
        assert_eq!(fit_size([], 10.0, 24.0, 1200.0, 96.0), 96.0);
        assert_eq!(fit_size([0.0], 10.0, 24.0, 1200.0, 96.0), 96.0);
        // Very wide text is capped.
        assert_eq!(fit_size([5000.0], 10.0, 24.0, 1200.0, 96.0), 1200.0);
    }

    #[test]
    fn sizes_follow_inserts_and_deletes_at_the_right_index() {
        let mut v = vec![10.0, 20.0, 30.0];
        sync_sizes(&mut v, 3, 4, 1, 96.0);
        assert_eq!(v, [10.0, 96.0, 20.0, 30.0]);
        sync_sizes(&mut v, 4, 3, 2, 96.0);
        assert_eq!(v, [10.0, 96.0, 30.0]);
        // Inserting past the end appends.
        sync_sizes(&mut v, 3, 4, 9, 96.0);
        assert_eq!(v, [10.0, 96.0, 30.0, 96.0]);
        // Deleting the last remaining line empties it: no size change.
        let mut one = vec![50.0];
        sync_sizes(&mut one, 1, 1, 0, 96.0);
        assert_eq!(one, [50.0]);
        // Growth by paste pads with defaults; shrinking truncates.
        let mut g = vec![50.0];
        fit_len(&mut g, 3, 96.0);
        assert_eq!(g, [50.0, 96.0, 96.0]);
        fit_len(&mut g, 2, 96.0);
        assert_eq!(g, [50.0, 96.0]);
    }

    #[test]
    fn edge_hit_finds_the_far_edge_within_the_grab_zone() {
        // Columns from x = 100: edges at 150, 250, 280.
        let sizes = [50.0, 100.0, 30.0];
        assert_eq!(edge_hit(150.0, 100.0, &sizes, 4.0), Some(0));
        assert_eq!(edge_hit(153.5, 100.0, &sizes, 4.0), Some(0));
        assert_eq!(edge_hit(146.5, 100.0, &sizes, 4.0), Some(0));
        assert_eq!(edge_hit(200.0, 100.0, &sizes, 4.0), None);
        assert_eq!(edge_hit(252.0, 100.0, &sizes, 4.0), Some(1));
        assert_eq!(edge_hit(283.0, 100.0, &sizes, 4.0), Some(2));
        // The line's own start edge is not its resize edge.
        assert_eq!(edge_hit(100.0, 100.0, &sizes, 4.0), None);
        // Overlapping zones (a 6-px column): the nearer edge wins.
        let thin = [50.0, 6.0];
        assert_eq!(edge_hit(151.0, 100.0, &thin, 4.0), Some(0));
        assert_eq!(edge_hit(155.0, 100.0, &thin, 4.0), Some(1));
    }

    #[test]
    fn toolbar_insert_and_delete_keep_sizes_aligned_and_a_new_table_resets_them() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("a\tb\tc\nd\te\tf".into()));
        fit_len(&mut s.col_widths, s.grid.cols(), CELL_WIDTH);
        fit_len(&mut s.row_heights, s.grid.rows(), 20.0);
        s.col_widths[2] = 200.0;
        s.row_heights[1] = 60.0;
        s.change_shape(Axis::Col, 1, CELL_WIDTH, |g| g.insert_col(1));
        assert_eq!(s.col_widths.len(), s.grid.cols());
        assert_eq!(
            s.col_widths[3], 200.0,
            "the wide column moved right with its cells"
        );
        s.change_shape(Axis::Row, 0, 20.0, |g| g.delete_row(0));
        assert_eq!(s.row_heights.len(), s.grid.rows());
        assert_eq!(
            s.row_heights[0], 60.0,
            "the tall row moved up with its cells"
        );
        s.reset_table();
        assert!(s.col_widths.is_empty() && s.row_heights.is_empty());
        s.col_widths.push(300.0);
        s.load_crop(PlotRaster::from_rgb_fn(4, 4, |_, _| [255, 255, 255]), None);
        assert!(
            s.col_widths.is_empty(),
            "a new region starts at default sizes"
        );
    }

    #[test]
    fn column_names_follow_calc() {
        assert_eq!(column_name(0), "A");
        assert_eq!(column_name(25), "Z");
        assert_eq!(column_name(26), "AA");
        assert_eq!(column_name(27), "AB");
        assert_eq!(column_name(701), "ZZ");
        assert_eq!(column_name(702), "AAA");
    }

    #[test]
    fn typing_replaces_the_cell_and_enter_commits_and_moves_down() {
        let mut s = TableDigitiserState::default();
        s.grid.set(0, 0, "old");
        s.apply(GridCommand::StartEdit(Some("n".into())));
        s.edit.as_mut().unwrap().text.push_str("ew");
        s.apply(GridCommand::CommitAndMove(Direction::Down));
        assert_eq!(s.grid.get(0, 0), "new");
        assert_eq!(s.grid.cursor(), CellPos::new(1, 0));
        assert!(s.edit.is_none());
    }

    #[test]
    fn f2_keeps_the_text_and_escape_cancels() {
        let mut s = TableDigitiserState::default();
        s.grid.set(0, 0, "keep");
        s.apply(GridCommand::StartEdit(None));
        assert_eq!(s.edit.as_ref().unwrap().text, "keep");
        s.edit.as_mut().unwrap().text = "changed".into();
        s.apply(GridCommand::CancelEdit);
        assert_eq!(s.grid.get(0, 0), "keep");
    }

    #[test]
    fn tab_commits_and_moves_right_and_arrows_do_nothing_while_editing() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::StartEdit(Some("1".into())));
        s.apply(GridCommand::Move {
            dir: Direction::Down,
            extend: false,
        });
        assert_eq!(
            s.grid.cursor(),
            CellPos::new(0, 0),
            "arrows edit text, not move"
        );
        s.apply(GridCommand::CommitAndMove(Direction::Right));
        assert_eq!(s.grid.get(0, 0), "1");
        assert_eq!(s.grid.cursor(), CellPos::new(0, 1));
    }

    #[test]
    fn a_pasted_pdf_row_fills_a_row_of_cells() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("I-131\t8.02 d\t0.61".into()));
        assert_eq!(s.grid.to_csv(), "I-131,8.02 d,0.61\n");
    }

    #[test]
    fn save_writes_a_pdf_native_table_artifact_with_a_clean_csv_fence() {
        let (_dir, root) = make_root();
        let mut session = paper(&root);
        let mut s = TableDigitiserState {
            table_name: "Table 5".into(),
            ..Default::default()
        };
        s.apply(GridCommand::Paste(
            "nuclide\trelease (Bq)\nI-131\t1.2e9".into(),
        ));
        assert!(s.save_into_project(Some(&mut session)), "{}", s.message);
        assert!(!s.message_is_error, "{}", s.message);

        let reopened = PaperSession::open(&root, "wang2018multiphysics").unwrap();
        let md = reopened.markdown();
        assert!(md.contains("kind = \"digitised_table\""), "{md}");
        assert!(md.contains("method = \"pdf_native\""), "{md}");
        assert!(md.contains("entered by hand"), "{md}");
        // The digitised graph's series schema, with `###` markers.
        assert!(
            md.contains("### start of data series\n\n### Series: Table 5\n\n```csv\n"),
            "{md}"
        );
        assert!(md.contains("### end of series"), "{md}");
        assert!(!md.contains("kopitiam-ocr"), "{md}");
        let idx = crate::research_record::ResearchRecordIndex::from_session(&reopened);
        let table = idx
            .artifacts()
            .iter()
            .find(|a| a.kind() == crate::artifact::ArtifactKind::DigitisedTable)
            .expect("the digitised table");
        assert_eq!(
            table.csv_block(),
            Some("nuclide,release (Bq)\nI-131,1.2e9\n")
        );
    }

    #[test]
    fn a_reopened_saved_table_loads_its_csv_back_into_the_grid() {
        let (_dir, root) = make_root();
        let mut session = paper(&root);
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("x\ty\n1\t2".into()));
        s.save_into_project(Some(&mut session));
        let id = crate::research_record::ResearchRecordIndex::from_session(&session)
            .artifacts()
            .iter()
            .find(|a| a.kind() == crate::artifact::ArtifactKind::DigitisedTable)
            .map(|a| a.id().to_string())
            .unwrap();

        let mut fresh = TableDigitiserState {
            pending_reload: Some(id),
            ..Default::default()
        };
        fresh.resolve_reload(Some(&session));
        assert!(!fresh.message_is_error, "{}", fresh.message);
        assert_eq!(fresh.grid.to_csv(), "x,y\n1,2\n");
    }

    #[test]
    fn saving_an_empty_grid_or_without_a_paper_is_refused() {
        let mut s = TableDigitiserState::default();
        assert!(!s.save_into_project(None));
        assert!(s.message_is_error);
        assert!(s.message.contains("empty"), "{}", s.message);
        s.apply(GridCommand::Paste("1".into()));
        assert!(!s.save_into_project(None));
        assert!(s.message.contains("ingest this PDF"), "{}", s.message);
    }

    #[test]
    fn cancel_on_an_empty_grid_leaves_at_once() {
        let mut s = TableDigitiserState {
            table_name: "Table 5".into(),
            ..Default::default()
        };
        assert!(!s.has_unsaved_changes());
        assert!(s.request_cancel(), "nothing to lose, so no prompt");
        assert!(!s.confirm_discard);
        assert!(s.table_name.is_empty(), "per-table state is reset");
    }

    #[test]
    fn cancel_with_unsaved_cells_asks_first_and_keep_editing_keeps_them() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("x\ty\n1\t2".into()));
        assert!(s.has_unsaved_changes());
        assert!(!s.request_cancel(), "unsaved cells must not be dropped");
        assert!(s.confirm_discard, "the discard box opens");
        // "Keep editing" (or Esc) only closes the box.
        assert!(!s.answer_discard(false));
        assert!(!s.confirm_discard);
        assert_eq!(s.grid.to_csv(), "x,y\n1,2\n");
    }

    #[test]
    fn an_edit_in_progress_counts_as_unsaved() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::StartEdit(Some("7".into())));
        assert!(s.has_unsaved_changes());
    }

    #[test]
    fn confirming_the_discard_resets_the_table() {
        let mut s = TableDigitiserState {
            table_name: "Table 5".into(),
            crop_provenance: Some(CropProvenance {
                page_index: 0,
                min: egui::Pos2::ZERO,
                max: egui::Pos2::new(1.0, 1.0),
                page_px: [10.0, 10.0],
                created_at: String::new(),
                author: String::new(),
                figure: "Table 5".into(),
                source_artifact_id: None,
            }),
            ..Default::default()
        };
        s.apply(GridCommand::Paste("1\t2".into()));
        s.apply(GridCommand::StartEdit(Some("3".into())));
        assert!(!s.request_cancel());
        assert!(s.answer_discard(true), "Discard and go back leaves");
        assert!(s.grid.to_csv().is_empty());
        assert!(s.edit.is_none());
        assert!(s.crop_provenance.is_none());
        assert!(s.pending_reload.is_none());
        assert!(s.table_name.is_empty());
        assert!(s.message.is_empty());
        assert!(!s.confirm_discard);
    }

    #[test]
    fn cancel_after_a_successful_save_leaves_without_asking() {
        let (_dir, root) = make_root();
        let mut session = paper(&root);
        let mut s = TableDigitiserState {
            table_name: "Table 5".into(),
            ..Default::default()
        };
        s.apply(GridCommand::Paste("a\tb".into()));
        assert!(s.save_into_project(Some(&mut session)), "{}", s.message);
        assert!(!s.has_unsaved_changes());
        assert!(s.request_cancel());
        assert!(!s.confirm_discard);
        // A change after the save is unsaved again.
        s.apply(GridCommand::Paste("c".into()));
        assert!(s.has_unsaved_changes());
    }

    #[test]
    fn a_reloaded_saved_table_is_not_unsaved() {
        let (_dir, root) = make_root();
        let mut session = paper(&root);
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("x\ty".into()));
        s.save_into_project(Some(&mut session));
        let id = crate::research_record::ResearchRecordIndex::from_session(&session)
            .artifacts()
            .iter()
            .find(|a| a.kind() == crate::artifact::ArtifactKind::DigitisedTable)
            .map(|a| a.id().to_string())
            .unwrap();
        let mut fresh = TableDigitiserState {
            pending_reload: Some(id),
            ..Default::default()
        };
        fresh.resolve_reload(Some(&session));
        assert!(!fresh.has_unsaved_changes());
    }

    /// Escape on the "Discard this table?" box keeps editing, through egui's
    /// own modal, headless.
    #[test]
    fn escape_on_the_discard_box_keeps_the_table() {
        let ctx = egui::Context::default();
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("1\t2".into()));
        assert!(!s.request_cancel());
        let mut outcome = TableOutcome::Continue;
        for press_escape in [false, true] {
            let mut input = egui::RawInput::default();
            if press_escape {
                input.events.push(egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
            let _ = ctx.run_ui(input, |ui| {
                outcome = s.ui(ui, None);
            });
        }
        assert_eq!(outcome, TableOutcome::Continue);
        assert!(!s.confirm_discard, "Esc closed the box");
        assert_eq!(s.grid.to_csv(), "1,2\n", "and kept the cells");
    }

    #[test]
    fn format_to_standard_form_with_nothing_convertible_opens_no_box() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste("1.0\u{00D7}10^5 m^2\tI-131".into()));
        s.request_reformat();
        assert!(s.pending_reformat.is_none());
        assert!(
            s.message
                .contains("no standard-form values in the highlighted cells"),
            "{}",
            s.message
        );
        assert_eq!(s.grid.to_csv(), "1.0\u{00D7}10^5 m^2,I-131\n");
    }

    #[test]
    fn format_to_standard_form_cancelled_changes_nothing() {
        let mut s = TableDigitiserState::default();
        s.apply(GridCommand::Paste(
            "2.1\u{00D7}10^6\t8.2\u{00D7}10^-7".into(),
        ));
        s.request_reformat();
        let preview = s.pending_reformat.clone().expect("the box opens");
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].before, "2.1\u{00D7}10^6");
        assert_eq!(preview[0].after, "2.1e6");
        s.cancel_reformat();
        assert!(s.pending_reformat.is_none());
        assert_eq!(s.grid.to_csv(), "2.1\u{00D7}10^6,8.2\u{00D7}10^-7\n");
    }

    #[test]
    fn format_to_standard_form_confirmed_converts_exactly_the_previewed_cells() {
        let mut s = TableDigitiserState::default();
        // Paste leaves the pasted block highlighted.
        s.apply(GridCommand::Paste(
            "2.1\u{00D7}10^6\t8.2\u{00D7}10^-7".into(),
        ));
        s.request_reformat();
        let preview = s.pending_reformat.clone().expect("the box opens");
        assert_eq!(s.confirm_reformat(), preview.len());
        for ch in &preview {
            assert_eq!(s.grid.get(ch.pos.row, ch.pos.col), ch.after);
        }
        assert_eq!(s.grid.to_csv(), "2.1e6,8.2e-7\n");
        assert!(s.pending_reformat.is_none());
        assert!(
            s.message.contains("reformatted 2 cell(s) to E notation"),
            "{}",
            s.message
        );
        s.apply(GridCommand::Undo);
        assert_eq!(
            s.grid.to_csv(),
            "2.1\u{00D7}10^6,8.2\u{00D7}10^-7\n",
            "one undo step"
        );
    }

    #[test]
    fn superscript_job_raises_the_exponent() {
        let job = superscript_job("2.1\u{00D7}10^6", 13.0, Color32::WHITE);
        assert_eq!(job.text, "2.1\u{00D7}106");
        assert_eq!(job.sections.len(), 2);
        assert_eq!(job.sections[1].format.valign, egui::Align::TOP);
        assert!(job.sections[1].format.font_id.size < 13.0);
    }

    /// Arrow keys that move the grid are used up, so the PDF reader drawn
    /// after the grid in the same frame never sees them and does not scroll.
    #[test]
    fn grid_arrows_are_consumed_so_the_pdf_does_not_see_them() {
        let ctx = egui::Context::default();
        let mut s = TableDigitiserState::default();
        s.grid_focused = true;
        // One press per frame, as a keyboard delivers them: Down, then
        // Shift+Down.
        for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            });
            let mut reader_saw_arrow = true;
            let _ = ctx.run_ui(input, |ui| {
                for c in s.commands_from_input(ui.ctx()) {
                    s.apply(c);
                }
                // What the reader would check, later in the same frame.
                reader_saw_arrow = ui.input(|i| i.key_pressed(Key::ArrowDown));
            });
            assert!(
                !reader_saw_arrow,
                "the reader must not see a grid arrow ({modifiers:?})"
            );
        }
        assert_eq!(
            s.grid.cursor(),
            CellPos::new(2, 0),
            "both presses moved the grid"
        );
        let sel = s.grid.selection();
        assert_eq!(
            (sel.top, sel.bottom),
            (1, 2),
            "Shift+Down extended the selection"
        );
    }

    /// Shift+Enter moves up and Shift+Tab moves left, as in Calc; neither is
    /// swallowed by the plain-key check.
    #[test]
    fn shift_enter_and_shift_tab_move_back() {
        let ctx = egui::Context::default();
        let mut s = TableDigitiserState::default();
        s.grid_focused = true;
        s.grid.set_cursor(CellPos::new(3, 3), false);
        for key in [Key::Enter, Key::Tab] {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            });
            let _ = ctx.run_ui(input, |ui| {
                for c in s.commands_from_input(ui.ctx()) {
                    s.apply(c);
                }
            });
        }
        assert_eq!(s.grid.cursor(), CellPos::new(2, 2));
    }

    /// The grid takes Enter and typed text from real egui input, headless.
    #[test]
    fn keys_reach_the_grid_through_egui_input() {
        let ctx = egui::Context::default();
        let mut s = TableDigitiserState::default();
        s.grid_focused = true;
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Paste("a\tb".into()));
        let _ = ctx.run_ui(input, |ui| {
            for c in s.commands_from_input(ui.ctx()) {
                s.apply(c);
            }
        });
        assert_eq!(s.grid.to_csv(), "a,b\n");
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        let _ = ctx.run_ui(input, |ui| {
            for c in s.commands_from_input(ui.ctx()) {
                s.apply(c);
            }
        });
        assert_eq!(s.grid.cursor(), CellPos::new(1, 0));
    }

    // ------------------------------------------------------------------
    // Follow the cursor (maintainer, 2026-09-28).
    // ------------------------------------------------------------------

    /// Only a change of cursor or selection asks to scroll; an idle frame
    /// does not, or wheel-scrolling the grid would snap straight back.
    #[test]
    fn follow_fires_on_a_move_and_not_on_an_idle_frame() {
        let a = CellPos::new(0, 0);
        let b = CellPos::new(1, 0);
        let one = |p: CellPos| CellRange {
            top: p.row,
            left: p.col,
            bottom: p.row,
            right: p.col,
        };
        let mut last = None;
        assert!(follow_moved(&mut last, (a, one(a))), "first frame");
        assert!(!follow_moved(&mut last, (a, one(a))), "idle frame");
        assert!(follow_moved(&mut last, (b, one(b))), "cursor moved");
        let block = CellRange {
            top: 1,
            left: 0,
            bottom: 5,
            right: 2,
        };
        assert!(follow_moved(&mut last, (b, block)), "selection grew");
        assert!(!follow_moved(&mut last, (b, block)));
    }

    /// Run the whole tab headlessly in a small window, one frame per event
    /// list, half a second apart so egui's scroll animation completes.
    fn drive_grid(s: &mut TableDigitiserState, frames: Vec<Vec<egui::Event>>) {
        let ctx = egui::Context::default();
        for (n, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                time: Some(n as f64 * 0.5),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 500.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ui| {
                let _ = s.ui(ui, None);
            });
        }
    }

    fn arrow_down() -> egui::Event {
        egui::Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    /// Pressing Down past the bottom of the viewport scrolls the grid so
    /// the active cell stays on screen. Control: the same tall grid with the
    /// cursor left at the top does not scroll at all, so the movement is the
    /// follow and not something else.
    #[test]
    fn arrowing_down_past_the_viewport_scrolls_the_grid() {
        let tall: String = (1..=80).map(|n| format!("{n}\n")).collect();
        let mut s = TableDigitiserState::default();
        s.grid_focused = true;
        s.apply(GridCommand::Paste(tall));
        drive_grid(&mut s, vec![Vec::new(); 4]);
        assert_eq!(s.grid_view.min.y, 0.0, "cursor at the top: no scroll");

        let mut frames: Vec<Vec<egui::Event>> = (0..60).map(|_| vec![arrow_down()]).collect();
        frames.extend(vec![Vec::new(); 4]);
        drive_grid(&mut s, frames);
        assert_eq!(s.grid.cursor(), CellPos::new(60, 0));
        let row_h = s.row_heights[0];
        let header_h = row_h;
        let cursor_top = header_h + 60.0 * row_h;
        let view = s.grid_view;
        assert!(view.min.y > 0.0, "the grid scrolled (view {view:?})");
        assert!(
            view.min.y <= cursor_top && cursor_top + row_h <= view.max.y + 0.5,
            "the whole active cell is in view (view {view:?}, cell {cursor_top}..{})",
            cursor_top + row_h
        );
        // `Align::None` scrolls the minimum: the cell sits at the bottom
        // edge, not recentred.
        assert!(
            (view.max.y - (cursor_top + row_h)).abs() < row_h,
            "minimum scroll: the cell is at the bottom edge (view {view:?})"
        );
    }
}
