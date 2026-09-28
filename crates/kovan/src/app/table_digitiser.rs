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

use eframe::egui::{self, Color32, Key, Modifiers, Sense, Stroke, Vec2};

use crate::digitiser::dataset::utc_now_iso8601;
use crate::digitiser::raster::PlotRaster;
use crate::digitiser::table_grid::{CellPos, Direction, TableGrid};
use crate::session::PaperSession;

use super::pdf_reader::CropProvenance;

/// Width of one grid column, points.
const CELL_WIDTH: f32 = 96.0;
/// Width of the row-number gutter, points.
const GUTTER_WIDTH: f32 = 36.0;

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
    /// The setup box was cancelled: drop the region and return to the PDF.
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
                if i.consume_key(Modifiers::NONE, key) {
                    out.push(GridCommand::CommitAndMove(dir));
                }
                if i.consume_key(Modifiers::SHIFT, key) {
                    out.push(GridCommand::CommitAndMove(rev));
                }
            }
            if editing {
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    out.push(GridCommand::CancelEdit);
                }
                return;
            }
            for (key, dir) in [
                (Key::ArrowUp, Direction::Up),
                (Key::ArrowDown, Direction::Down),
                (Key::ArrowLeft, Direction::Left),
                (Key::ArrowRight, Direction::Right),
            ] {
                if i.key_pressed(key) {
                    out.push(GridCommand::Move { dir, extend: shift });
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
    /// setup box is backed out of.
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
        if self.grid_focused {
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
            });
        });
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
        ui.horizontal(|ui| {
            let at = self.grid.cursor();
            if ui
                .button("+ row")
                .on_hover_text("insert a row above the cursor")
                .clicked()
            {
                self.grid.insert_row(at.row);
            }
            if ui
                .button("+ col")
                .on_hover_text("insert a column left of the cursor")
                .clicked()
            {
                self.grid.insert_col(at.col);
            }
            if ui.button("\u{2212} row").clicked() {
                self.grid.delete_row(at.row);
            }
            if ui.button("\u{2212} col").clicked() {
                self.grid.delete_col(at.col);
            }
            if ui.button("Undo").clicked() {
                self.apply(GridCommand::Undo);
            }
            if ui.button("Redo").clicked() {
                self.apply(GridCommand::Redo);
            }
            ui.label(format!("{}{}", column_name(at.col), at.row + 1));
        });
        ui.separator();

        // The cell editor/viewer, with room left below it for the actions.
        let actions_height = 110.0;
        let grid_height = (ui.available_height() - actions_height).max(120.0);
        let mut clicked: Option<(CellPos, bool, bool)> = None; // (pos, shift, double)
        egui::ScrollArea::both()
            .id_salt("table_grid_scroll")
            .max_height(grid_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let row_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
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
                    ui.allocate_exact_size(Vec2::new(GUTTER_WIDTH, row_h), Sense::hover());
                    for c in 0..self.grid.cols() {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(CELL_WIDTH, row_h), Sense::hover());
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
                for r in 0..self.grid.rows() {
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
                        for c in 0..self.grid.cols() {
                            let pos = CellPos::new(r, c);
                            let size = Vec2::new(CELL_WIDTH, row_h);
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
                                continue;
                            }
                            let (rect, response) =
                                ui.allocate_exact_size(size, Sense::click_and_drag());
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
                                ui.painter().with_clip_rect(rect.shrink(2.0)).text(
                                    rect.left_center() + Vec2::new(4.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    egui::FontId::proportional(13.0),
                                    if selected {
                                        strong
                                    } else {
                                        visuals.text_color()
                                    },
                                );
                            }
                            if response.double_clicked() {
                                clicked = Some((pos, false, true));
                            } else if response.clicked() || response.drag_started() {
                                let shift = ui.input(|i| i.modifiers.shift);
                                clicked = Some((pos, shift, false));
                            } else if ui.input(|i| i.pointer.primary_down())
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
            });
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
            self.setup = None;
            self.crop_provenance = None;
            self.set_status("");
        }
        outcome
    }
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
}
