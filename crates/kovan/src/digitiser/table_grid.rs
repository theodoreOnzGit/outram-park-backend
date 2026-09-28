//! A spreadsheet-style grid for digitising tables by hand, with no OCR
//! (GitHub #353/#354, maintainer direction 2026-09-28: "i want libreoffice
//! like interface ... libreoffice calc cells on the left panel, pdf viewer of
//! table on right hand side").
//!
//! This is the **model** only: cells, a cursor, a selection, paste, CSV and
//! undo. It has no `egui` in it, so it compiles on Android/Termux and every
//! behaviour here is unit-tested without a window. The GUI that draws it is
//! `crate::app::table_digitiser`.
//!
//! ## Behaviour borrowed from LibreOffice Calc
//!
//! LibreOffice is **not** vendored (it is millions of lines of C++, which
//! would break this crate's pure-Rust and Android build). Only its user-facing
//! conventions are copied, so a Calc user's hands already know the grid:
//!
//! - **Paste** fills cells from the top-left of the selection: rows split on
//!   newlines, columns on tabs, the same way Calc pastes tab-separated text
//!   copied from another spreadsheet. A single trailing newline is ignored,
//!   because copying a whole row usually ends with one.
//! - **Copy** writes the selection back out as tab-separated text, so a block
//!   copied here pastes into Calc cell for cell.
//! - **CSV** export uses a comma separator and `"` quoting, doubling a `"`
//!   inside a quoted field (RFC 4180), which is Calc's default "Text CSV"
//!   export. It is written by the `csv` crate rather than by hand.
//!
//! Calc's sheet is fixed at about a million rows. This grid instead **grows**
//! when the cursor or a paste runs past its edge, and export drops the empty
//! trailing rows and columns, so what is saved is only what was typed.

use std::fmt;

/// Rows a new grid starts with. Enough for most published tables at a
/// glance; the grid grows past it on demand.
pub const DEFAULT_ROWS: usize = 20;
/// Columns a new grid starts with. See [`DEFAULT_ROWS`].
pub const DEFAULT_COLS: usize = 8;
/// Undo steps kept. Each is a full snapshot, and a digitised table is at
/// most a few thousand short strings, so this bounds memory at a few MB.
pub const UNDO_DEPTH: usize = 100;

/// A cell's position, zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellPos {
    pub row: usize,
    pub col: usize,
}

impl CellPos {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// A rectangular block of cells, both corners **inclusive**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRange {
    pub top: usize,
    pub left: usize,
    pub bottom: usize,
    pub right: usize,
}

impl CellRange {
    /// Whether `pos` lies inside the range.
    pub fn contains(&self, pos: CellPos) -> bool {
        (self.top..=self.bottom).contains(&pos.row) && (self.left..=self.right).contains(&pos.col)
    }
}

/// Which way an arrow key moves the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Why a CSV could not be loaded into a grid.
#[derive(Debug)]
pub struct CsvLoadError(csv::Error);

impl fmt::Display for CsvLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not read CSV: {}", self.0)
    }
}

impl std::error::Error for CsvLoadError {}

/// The grid: always rectangular, never smaller than 1 x 1.
#[derive(Debug, Clone)]
pub struct TableGrid {
    cells: Vec<Vec<String>>,
    cursor: CellPos,
    /// The other corner of a Shift-extended selection. `None` means the
    /// selection is just the cursor cell.
    anchor: Option<CellPos>,
    undo: Vec<Vec<Vec<String>>>,
    redo: Vec<Vec<Vec<String>>>,
}

impl Default for TableGrid {
    fn default() -> Self {
        Self::new(DEFAULT_ROWS, DEFAULT_COLS)
    }
}

impl TableGrid {
    /// An empty grid of `rows` x `cols`, each at least 1.
    pub fn new(rows: usize, cols: usize) -> Self {
        let (rows, cols) = (rows.max(1), cols.max(1));
        Self {
            cells: vec![vec![String::new(); cols]; rows],
            cursor: CellPos::default(),
            anchor: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn rows(&self) -> usize {
        self.cells.len()
    }

    pub fn cols(&self) -> usize {
        self.cells[0].len()
    }

    /// The text in a cell, or `""` outside the grid.
    pub fn get(&self, row: usize, col: usize) -> &str {
        self.cells
            .get(row)
            .and_then(|r| r.get(col))
            .map_or("", String::as_str)
    }

    pub fn cursor(&self) -> CellPos {
        self.cursor
    }

    /// The selected block: from the anchor to the cursor, or the cursor cell
    /// alone when nothing is extended.
    pub fn selection(&self) -> CellRange {
        let a = self.anchor.unwrap_or(self.cursor);
        let c = self.cursor;
        CellRange {
            top: a.row.min(c.row),
            left: a.col.min(c.col),
            bottom: a.row.max(c.row),
            right: a.col.max(c.col),
        }
    }

    /// Put the cursor on `pos`, growing the grid to reach it. With `extend`
    /// the selection stretches from its anchor (Shift+click in Calc);
    /// without it the selection collapses to the new cell.
    pub fn set_cursor(&mut self, pos: CellPos, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.ensure_size(pos.row + 1, pos.col + 1);
        self.cursor = pos;
    }

    /// Move the cursor one cell. It stops at the top and left edges and grows
    /// the grid at the bottom and right. With `extend` it stretches the
    /// selection instead (Shift+arrow).
    pub fn move_cursor(&mut self, dir: Direction, extend: bool) {
        let CellPos { row, col } = self.cursor;
        let next = match dir {
            Direction::Up => CellPos::new(row.saturating_sub(1), col),
            Direction::Down => CellPos::new(row + 1, col),
            Direction::Left => CellPos::new(row, col.saturating_sub(1)),
            Direction::Right => CellPos::new(row, col + 1),
        };
        self.set_cursor(next, extend);
    }

    /// Replace one cell's text, growing the grid to reach it. Undoable.
    pub fn set(&mut self, row: usize, col: usize, text: &str) {
        if self.get(row, col) == text {
            return;
        }
        self.checkpoint();
        self.ensure_size(row + 1, col + 1);
        self.cells[row][col] = text.to_owned();
    }

    /// Empty every cell in the selection (Delete in Calc). Undoable.
    pub fn clear_selection(&mut self) {
        let s = self.selection();
        let any =
            (s.top..=s.bottom).any(|r| (s.left..=s.right).any(|c| !self.get(r, c).is_empty()));
        if !any {
            return;
        }
        self.checkpoint();
        for r in s.top..=s.bottom {
            for c in s.left..=s.right {
                self.cells[r][c].clear();
            }
        }
    }

    /// Paste tab-separated text at the top-left of the selection: rows on
    /// newlines, columns on tabs, growing the grid as needed. `\r\n` is
    /// treated as `\n`, and one trailing newline is ignored. Afterwards the
    /// pasted block is selected, as in Calc. Undoable.
    pub fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n");
        let text = text.strip_suffix('\n').unwrap_or(&text);
        if text.is_empty() {
            return;
        }
        let block: Vec<Vec<&str>> = text.split('\n').map(|l| l.split('\t').collect()).collect();
        let height = block.len();
        let width = block.iter().map(Vec::len).max().unwrap_or(1);
        let origin = {
            let s = self.selection();
            CellPos::new(s.top, s.left)
        };
        self.checkpoint();
        self.ensure_size(origin.row + height, origin.col + width);
        for (dr, line) in block.iter().enumerate() {
            for (dc, field) in line.iter().enumerate() {
                self.cells[origin.row + dr][origin.col + dc] = (*field).to_owned();
            }
        }
        self.cursor = origin;
        self.anchor = None;
        if height > 1 || width > 1 {
            self.anchor = Some(CellPos::new(
                origin.row + height - 1,
                origin.col + width - 1,
            ));
        }
    }

    /// The selection as tab-separated text, rows joined by `\n`: what Calc
    /// puts on the clipboard for a block.
    pub fn copy_selection(&self) -> String {
        let s = self.selection();
        (s.top..=s.bottom)
            .map(|r| {
                (s.left..=s.right)
                    .map(|c| self.get(r, c))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Insert an empty row above `at` (clamped to the end). Undoable.
    pub fn insert_row(&mut self, at: usize) {
        self.checkpoint();
        let at = at.min(self.rows());
        let cols = self.cols();
        self.cells.insert(at, vec![String::new(); cols]);
    }

    /// Insert an empty column left of `at` (clamped to the end). Undoable.
    pub fn insert_col(&mut self, at: usize) {
        self.checkpoint();
        let at = at.min(self.cols());
        for row in &mut self.cells {
            row.insert(at, String::new());
        }
    }

    /// Delete row `at`. The last remaining row is emptied instead, so the
    /// grid never reaches zero size. Undoable.
    pub fn delete_row(&mut self, at: usize) {
        if at >= self.rows() {
            return;
        }
        self.checkpoint();
        if self.rows() == 1 {
            self.cells[0].iter_mut().for_each(String::clear);
        } else {
            self.cells.remove(at);
        }
        self.clamp_cursor();
    }

    /// Delete column `at`. The last remaining column is emptied instead.
    /// Undoable.
    pub fn delete_col(&mut self, at: usize) {
        if at >= self.cols() {
            return;
        }
        self.checkpoint();
        if self.cols() == 1 {
            self.cells.iter_mut().for_each(|r| r[0].clear());
        } else {
            for row in &mut self.cells {
                row.remove(at);
            }
        }
        self.clamp_cursor();
    }

    /// Step back one edit. Returns whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.cells, prev));
        self.clamp_cursor();
        true
    }

    /// Re-apply the last undone edit. Returns whether there was one.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.cells, next));
        self.clamp_cursor();
        true
    }

    /// The cells with empty trailing rows and columns dropped: exactly the
    /// block that was filled in. Empty cells *inside* it are kept, since a
    /// blank in a published table is data. An empty grid gives no rows.
    pub fn trimmed(&self) -> Vec<Vec<String>> {
        let last_row = self
            .cells
            .iter()
            .rposition(|r| r.iter().any(|c| !c.is_empty()));
        let Some(last_row) = last_row else {
            return Vec::new();
        };
        let last_col = self
            .cells
            .iter()
            .filter_map(|r| r.iter().rposition(|c| !c.is_empty()))
            .max()
            .unwrap_or(0);
        self.cells[..=last_row]
            .iter()
            .map(|r| r[..=last_col].to_vec())
            .collect()
    }

    /// The filled block as CSV: comma-separated, `"`-quoted where needed,
    /// `\n` line endings, and no comment lines, because provenance belongs in
    /// the artifact's `[extraction]` table, not inside the data. An empty grid
    /// gives `""`.
    pub fn to_csv(&self) -> String {
        let mut w = csv::WriterBuilder::new()
            .terminator(csv::Terminator::Any(b'\n'))
            .from_writer(Vec::new());
        for row in self.trimmed() {
            // Writing to a Vec cannot fail, and every row has the same width.
            w.write_record(&row).expect("in-memory CSV write");
        }
        let bytes = w.into_inner().expect("in-memory CSV flush");
        String::from_utf8(bytes).expect("CSV written from Strings is UTF-8")
    }

    /// Load a CSV (for example a saved table's payload) into a fresh grid.
    /// Ragged rows are padded, and the grid is padded out to at least the
    /// default size so there is room to keep editing.
    pub fn from_csv(text: &str) -> Result<Self, CsvLoadError> {
        let mut r = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut rows: Vec<Vec<String>> = Vec::new();
        for record in r.records() {
            let record = record.map_err(CsvLoadError)?;
            rows.push(record.iter().map(str::to_owned).collect());
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut grid = Self::new(rows.len().max(DEFAULT_ROWS), width.max(DEFAULT_COLS));
        for (r, row) in rows.into_iter().enumerate() {
            for (c, field) in row.into_iter().enumerate() {
                grid.cells[r][c] = field;
            }
        }
        Ok(grid)
    }

    /// Save the current cells for undo and forget anything redoable.
    fn checkpoint(&mut self) {
        if self.undo.len() == UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.undo.push(self.cells.clone());
        self.redo.clear();
    }

    /// Grow (never shrink) to at least `rows` x `cols`.
    fn ensure_size(&mut self, rows: usize, cols: usize) {
        let cols = cols.max(self.cols());
        for row in &mut self.cells {
            row.resize(cols, String::new());
        }
        while self.cells.len() < rows {
            self.cells.push(vec![String::new(); cols]);
        }
    }

    /// Keep the cursor and anchor inside the grid after it shrank.
    fn clamp_cursor(&mut self) {
        let (rmax, cmax) = (self.rows() - 1, self.cols() - 1);
        let clamp = |p: CellPos| CellPos::new(p.row.min(rmax), p.col.min(cmax));
        self.cursor = clamp(self.cursor);
        self.anchor = self.anchor.map(clamp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_grid_is_the_default_size_and_empty() {
        let g = TableGrid::default();
        assert_eq!((g.rows(), g.cols()), (DEFAULT_ROWS, DEFAULT_COLS));
        assert_eq!(g.to_csv(), "");
        assert!(g.trimmed().is_empty());
    }

    #[test]
    fn the_cursor_stops_at_the_top_left_and_grows_the_grid_at_the_bottom_right() {
        let mut g = TableGrid::new(2, 2);
        g.move_cursor(Direction::Up, false);
        g.move_cursor(Direction::Left, false);
        assert_eq!(g.cursor(), CellPos::new(0, 0));
        g.move_cursor(Direction::Down, false);
        g.move_cursor(Direction::Down, false);
        g.move_cursor(Direction::Right, false);
        g.move_cursor(Direction::Right, false);
        assert_eq!(g.cursor(), CellPos::new(2, 2));
        assert_eq!((g.rows(), g.cols()), (3, 3));
    }

    #[test]
    fn shift_arrows_extend_the_selection_and_a_plain_move_collapses_it() {
        let mut g = TableGrid::default();
        g.move_cursor(Direction::Right, true);
        g.move_cursor(Direction::Down, true);
        let s = g.selection();
        assert_eq!((s.top, s.left, s.bottom, s.right), (0, 0, 1, 1));
        g.move_cursor(Direction::Down, false);
        let s = g.selection();
        assert_eq!((s.top, s.left, s.bottom, s.right), (2, 1, 2, 1));
    }

    #[test]
    fn pasting_tab_separated_rows_fills_a_block_from_the_cursor_and_selects_it() {
        let mut g = TableGrid::new(2, 2);
        g.set_cursor(CellPos::new(1, 1), false);
        g.paste("Kr-85\t1.2e-4\t3\r\nI-131\t5.0e-5\t4\n");
        assert_eq!((g.rows(), g.cols()), (3, 4));
        assert_eq!(g.get(1, 1), "Kr-85");
        assert_eq!(g.get(2, 3), "4");
        let s = g.selection();
        assert_eq!((s.top, s.left, s.bottom, s.right), (1, 1, 2, 3));
    }

    #[test]
    fn a_single_value_paste_goes_into_the_cursor_cell_only() {
        let mut g = TableGrid::default();
        g.set_cursor(CellPos::new(3, 2), false);
        g.paste("0.61");
        assert_eq!(g.get(3, 2), "0.61");
        assert_eq!(g.selection().bottom, 3);
    }

    #[test]
    fn copy_gives_tab_separated_text_that_pastes_back_identically() {
        let mut g = TableGrid::default();
        g.paste("a\tb\nc\td");
        let copied = g.copy_selection();
        assert_eq!(copied, "a\tb\nc\td");
        let mut h = TableGrid::default();
        h.paste(&copied);
        assert_eq!(h.trimmed(), g.trimmed());
    }

    #[test]
    fn delete_clears_the_selection_only() {
        let mut g = TableGrid::default();
        g.paste("1\t2\n3\t4");
        g.set_cursor(CellPos::new(0, 1), false);
        g.move_cursor(Direction::Down, true);
        g.clear_selection();
        assert_eq!(
            g.trimmed(),
            vec![vec!["1".to_owned()], vec!["3".to_owned()]]
        );
    }

    #[test]
    fn export_trims_trailing_empties_but_keeps_interior_blanks() {
        let mut g = TableGrid::default();
        g.set(0, 0, "x");
        g.set(2, 2, "y");
        assert_eq!(g.to_csv(), "x,,\n,,\n,,y\n");
    }

    #[test]
    fn csv_quotes_commas_quotes_and_newlines_and_round_trips() {
        let mut g = TableGrid::default();
        g.set(0, 0, "T (K), peak");
        g.set(0, 1, "the \"hot\" leg");
        g.set(1, 0, "two\nlines");
        g.set(1, 1, "1.5");
        let csv = g.to_csv();
        assert_eq!(
            csv,
            "\"T (K), peak\",\"the \"\"hot\"\" leg\"\n\"two\nlines\",1.5\n"
        );
        let back = TableGrid::from_csv(&csv).unwrap();
        assert_eq!(back.trimmed(), g.trimmed());
    }

    #[test]
    fn a_ragged_csv_loads_padded_and_at_least_default_size() {
        let g = TableGrid::from_csv("a,b,c\nd\n").unwrap();
        assert_eq!(g.get(1, 0), "d");
        assert_eq!(g.get(1, 2), "");
        assert!(g.rows() >= DEFAULT_ROWS && g.cols() >= DEFAULT_COLS);
    }

    #[test]
    fn undo_and_redo_step_through_edits_and_a_new_edit_drops_redo() {
        let mut g = TableGrid::default();
        g.set(0, 0, "1");
        g.set(0, 0, "2");
        assert!(g.undo());
        assert_eq!(g.get(0, 0), "1");
        assert!(g.redo());
        assert_eq!(g.get(0, 0), "2");
        g.undo();
        g.set(0, 0, "3");
        assert!(!g.redo(), "a fresh edit forgets the undone branch");
        assert!(g.undo() && g.undo());
        assert_eq!(g.get(0, 0), "");
        assert!(!g.undo());
    }

    #[test]
    fn setting_a_cell_to_its_own_value_is_not_an_undo_step() {
        let mut g = TableGrid::default();
        g.set(0, 0, "");
        assert!(!g.undo());
    }

    #[test]
    fn the_undo_stack_is_bounded() {
        let mut g = TableGrid::default();
        for i in 0..(UNDO_DEPTH + 10) {
            g.set(0, 0, &i.to_string());
        }
        let mut steps = 0;
        while g.undo() {
            steps += 1;
        }
        assert_eq!(steps, UNDO_DEPTH);
    }

    #[test]
    fn rows_and_columns_insert_and_delete_and_never_reach_zero() {
        let mut g = TableGrid::new(1, 1);
        g.set(0, 0, "keep");
        g.insert_row(0);
        g.insert_col(0);
        assert_eq!(g.get(1, 1), "keep");
        g.delete_row(0);
        g.delete_col(0);
        assert_eq!((g.rows(), g.cols()), (1, 1));
        assert_eq!(g.get(0, 0), "keep");
        g.delete_row(0);
        assert_eq!((g.rows(), g.cols()), (1, 1));
        assert_eq!(g.get(0, 0), "", "the last row is emptied, not removed");
    }

    #[test]
    fn deleting_rows_keeps_the_cursor_inside_the_grid() {
        let mut g = TableGrid::new(3, 3);
        g.set_cursor(CellPos::new(2, 2), false);
        g.delete_row(2);
        g.delete_col(2);
        assert_eq!(g.cursor(), CellPos::new(1, 1));
    }
}
