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

/// One cell's text before and after a proposed change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellChange {
    pub pos: CellPos,
    pub before: String,
    pub after: String,
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

    /// Rewrite the **selected** cells that are written in standard form
    /// (`2.1×10^6`, `8.2 x 10^-7`) as E notation (`2.1e6`, `8.2e-7`), so a
    /// spreadsheet or `f64::parse` reads them as numbers (maintainer,
    /// 2026-09-28: "user selects cells, and clicks a format to standard form
    /// button, which then puts in e notation for highlighted cells"). See
    /// [`standard_form_to_e`] for exactly what counts; other cells, selected
    /// or not, are left alone. One undo step; returns how many cells changed.
    pub fn reformat_standard_form(&mut self) -> usize {
        let changes = self.standard_form_changes();
        if changes.is_empty() {
            return 0;
        }
        self.checkpoint();
        for change in &changes {
            self.cells[change.pos.row][change.pos.col] = change.after.clone();
        }
        changes.len()
    }

    /// What [`Self::reformat_standard_form`] would do, without doing it: each
    /// selected cell it would change, with its text before and after, in
    /// reading order. For the "are you sure?" preview the maintainer asked
    /// for ("displays the superscripted text before, and e form text
    /// after").
    pub fn standard_form_changes(&self) -> Vec<CellChange> {
        let sel = self.selection();
        (sel.top..=sel.bottom)
            .flat_map(|r| (sel.left..=sel.right).map(move |c| CellPos::new(r, c)))
            .filter_map(|pos| {
                let before = self.get(pos.row, pos.col);
                standard_form_to_e(before).map(|after| CellChange {
                    pos,
                    before: before.to_owned(),
                    after,
                })
            })
            .collect()
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

/// The E-notation form of `cell` when the **whole cell** is one number in
/// standard form, else `None`.
///
/// Accepted: a mantissa (optional sign, digits, optional decimal part), a
/// multiplication sign (`×`, `x`, `X`, `·`, `⋅`, `*`), `10`, and an exponent,
/// with spaces allowed around the sign. The exponent is written either
///
/// - after a caret, `10^6`, which is how the reader's character selection
///   marks a superscript (`app::pdf_reader::select_chars_in_rect`), or
/// - straight after the `10`, `106`, which is what a superscript turns into
///   when the text was copied without superscript detection. `3×100` is
///   therefore read as 3e0, not 300: in a table this button is pressed on,
///   that is the far likelier meaning, and the change is one undo away.
///
/// A bare `10^6` gives `1e6`. The minus sign may be `-` or `−` (U+2212).
/// Anything else in the cell (units, footnote marks, words) means it is not
/// converted, so prose and labels are never touched: `1.0X10^5 m^2` keeps
/// its superscripts as they are (maintainer, 2026-09-28).
pub fn standard_form_to_e(cell: &str) -> Option<String> {
    if let Some(e) = tidy_e_notation(cell) {
        return Some(e);
    }
    let s: String = cell.trim().replace('\u{2212}', "-");
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let take_digits = |i: &mut usize| {
        let start = *i;
        while *i < chars.len() && chars[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > start
    };
    let skip_spaces = |i: &mut usize| {
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
    };
    // Mantissa, or a bare "10^...".
    let mut mantissa = String::new();
    if chars.get(i).is_some_and(|c| *c == '+' || *c == '-') {
        mantissa.push(chars[i]);
        i += 1;
    }
    let int_start = i;
    let has_mantissa = take_digits(&mut i);
    let bare_power = has_mantissa
        && chars[int_start..i].iter().collect::<String>() == "10"
        && chars.get(i) == Some(&'^');
    if bare_power {
        i = int_start;
        mantissa.push('1');
    } else {
        if !has_mantissa {
            return None;
        }
        if chars.get(i) == Some(&'.') {
            i += 1;
            // Some PDFs' text layer puts a space after the decimal point
            // (HTR-10 Table 3's Xe-131m reads "9. 3×10⁶"); it is dropped,
            // but only when digits follow, so "9. ×" is still refused.
            let mut j = i;
            skip_spaces(&mut j);
            if chars.get(j).is_some_and(|c| c.is_ascii_digit()) {
                i = j;
            }
            take_digits(&mut i);
        }
        mantissa.extend(chars[int_start..i].iter().filter(|c| !c.is_whitespace()));
        skip_spaces(&mut i);
        if !chars
            .get(i)
            .is_some_and(|c| matches!(c, '×' | 'x' | 'X' | '·' | '⋅' | '*'))
        {
            return None;
        }
        i += 1;
        skip_spaces(&mut i);
    }
    // "10", then the exponent.
    if chars.get(i) != Some(&'1') || chars.get(i + 1) != Some(&'0') {
        return None;
    }
    i += 2;
    if chars.get(i) == Some(&'^') {
        i += 1;
    }
    let mut exponent = String::new();
    if chars.get(i).is_some_and(|c| *c == '+' || *c == '-') {
        if chars[i] == '-' {
            exponent.push('-');
        }
        i += 1;
    }
    let exp_start = i;
    if !take_digits(&mut i) || i != chars.len() {
        return None;
    }
    let digits: String = chars[exp_start..i].iter().collect();
    let digits = digits.trim_start_matches('0');
    exponent.push_str(if digits.is_empty() { "0" } else { digits });
    Some(format!("{mantissa}e{exponent}"))
}

/// Split `text` into plain and superscript pieces for display, reading the
/// `^` marks the reader's character selection writes: `2.1×10^6` gives
/// `[("2.1×10", false), ("6", true)]`. A superscript run is an optional
/// leading sign (`-`, `−`, `+`) then letters and digits; it ends at anything
/// else (a space, a bracket, punctuation). A `^` with nothing superscriptable
/// after it is kept as plain text.
pub fn superscript_segments(text: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let push = |out: &mut Vec<(String, bool)>, piece: &str, sup: bool| {
        if piece.is_empty() {
            return;
        }
        match out.last_mut() {
            Some((last, s)) if *s == sup => last.push_str(piece),
            _ => out.push((piece.to_owned(), sup)),
        }
    };
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut plain = String::new();
    while i < chars.len() {
        if chars[i] == '^' {
            let mut j = i + 1;
            if chars
                .get(j)
                .is_some_and(|c| matches!(c, '-' | '\u{2212}' | '+'))
            {
                j += 1;
            }
            let body_start = j;
            while chars.get(j).is_some_and(|c| c.is_alphanumeric()) {
                j += 1;
            }
            if j > body_start {
                push(&mut out, &plain, false);
                plain.clear();
                let sup: String = chars[i + 1..j].iter().collect();
                push(&mut out, &sup, true);
                i = j;
                continue;
            }
        }
        plain.push(chars[i]);
        i += 1;
    }
    push(&mut out, &plain, false);
    out
}

/// A cell already in E notation but not in a form a parser reads, rewritten
/// canonically: `1.1E−4` (a Unicode minus, as the HTR-10 paper's Table 7
/// prints it) becomes `1.1e-4`. `None` when the cell is not E notation or is
/// already canonical (`2.1e6`), so a canonical cell is never "changed".
fn tidy_e_notation(cell: &str) -> Option<String> {
    let s = cell.trim();
    let (mantissa, exponent) = s.split_once(['e', 'E'])?;
    let mantissa_ok = {
        let m = mantissa
            .strip_prefix(['+', '-', '\u{2212}'])
            .unwrap_or(mantissa);
        let (int, frac) = m.split_once('.').unwrap_or((m, ""));
        !int.is_empty()
            && int.chars().all(|c| c.is_ascii_digit())
            && frac.chars().all(|c| c.is_ascii_digit())
    };
    let (sign, digits) = match exponent.chars().next()? {
        '-' | '\u{2212}' => ("-", &exponent[exponent.chars().next()?.len_utf8()..]),
        '+' => ("", &exponent[1..]),
        _ => ("", exponent),
    };
    if !mantissa_ok || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    let tidy = format!(
        "{}e{sign}{}",
        mantissa.replace('\u{2212}', "-"),
        if digits.is_empty() { "0" } else { digits }
    );
    (tidy != cell).then_some(tidy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_form_with_a_caret_becomes_e_notation() {
        assert_eq!(standard_form_to_e("2.1×10^6").as_deref(), Some("2.1e6"));
        assert_eq!(
            standard_form_to_e(" 8.2 x 10^-7 ").as_deref(),
            Some("8.2e-7")
        );
        assert_eq!(
            standard_form_to_e("-3.5·10^\u{2212}8").as_deref(),
            Some("-3.5e-8")
        );
        assert_eq!(standard_form_to_e("1.6*10^+07").as_deref(), Some("1.6e7"));
        assert_eq!(standard_form_to_e("10^6").as_deref(), Some("1e6"));
    }

    #[test]
    fn e_notation_with_a_unicode_minus_is_tidied_and_canonical_e_is_left_alone() {
        // HTR-10 Table 7 prints its doses as "1.1E−4" (U+2212).
        assert_eq!(
            standard_form_to_e("1.1E\u{2212}4").as_deref(),
            Some("1.1e-4")
        );
        assert_eq!(
            standard_form_to_e("9.6E\u{2212}6").as_deref(),
            Some("9.6e-6")
        );
        assert_eq!(
            standard_form_to_e("\u{2212}2.0E+03").as_deref(),
            Some("-2.0e3")
        );
        assert_eq!(standard_form_to_e("2.1e6"), None);
        assert_eq!(standard_form_to_e("8.2e-7"), None);
        assert_eq!(standard_form_to_e("E-4"), None);
        assert_eq!(standard_form_to_e("1.1E"), None);
    }

    #[test]
    fn a_stray_space_after_the_decimal_point_is_dropped() {
        // HTR-10 Table 3, Xe-131m, as the PDF text layer gives it.
        assert_eq!(standard_form_to_e("9. 3×10^6").as_deref(), Some("9.3e6"));
    }

    #[test]
    fn a_lost_superscript_is_read_as_the_exponent() {
        // What the maintainer's copy produced before superscripts were
        // detected: 2.1×10⁶ arriving as "2.1×106".
        assert_eq!(standard_form_to_e("2.1×106").as_deref(), Some("2.1e6"));
        assert_eq!(standard_form_to_e("3.5×108").as_deref(), Some("3.5e8"));
    }

    #[test]
    fn anything_that_is_not_one_standard_form_number_is_left_alone() {
        for cell in [
            "",
            "I-131",
            "2.1e6",
            "2.1",
            "×10^6",
            "2.1×10^6 Bq",
            // A value with a unit keeps its superscripts and is not
            // reformatted (maintainer, 2026-09-28: "tables are 1.0X10^5 m^2,
            // i think for those, keep superscripts and do not reformat").
            "1.0X10^5 m^2",
            "m^2",
            "2.1×10^",
            "5 × 3",
        ] {
            assert_eq!(standard_form_to_e(cell), None, "{cell:?}");
        }
    }

    #[test]
    fn reformatting_converts_only_standard_form_cells_in_one_undo_step() {
        let mut g = TableGrid::default();
        g.paste("nuclide\tactivity (Bq)\nKr-85\t2.1×10^6\nI-131\t8.2×107");
        // The paste leaves the whole block selected.
        assert_eq!(g.reformat_standard_form(), 2);
        assert_eq!(
            g.to_csv(),
            "nuclide,activity (Bq)\nKr-85,2.1e6\nI-131,8.2e7\n"
        );
        assert!(g.undo());
        assert_eq!(g.get(1, 1), "2.1×10^6");
        g.set_cursor(CellPos::new(0, 0), false);
        g.set_cursor(CellPos::new(2, 1), true);
        assert_eq!(g.reformat_standard_form(), 2);
        assert_eq!(g.reformat_standard_form(), 0, "nothing left to convert");
    }

    #[test]
    fn the_preview_lists_before_and_after_without_changing_anything() {
        let mut g = TableGrid::default();
        g.paste("I-131\t8.2×10^−7\t1.0×10^5 m^2");
        let changes = g.standard_form_changes();
        assert_eq!(
            changes,
            vec![CellChange {
                pos: CellPos::new(0, 1),
                before: "8.2×10^−7".into(),
                after: "8.2e-7".into(),
            }]
        );
        assert_eq!(g.get(0, 1), "8.2×10^−7", "a preview changes nothing");
        assert!(g.undo(), "the paste's own undo step");
        assert!(!g.undo(), "and nothing more: a preview adds no undo step");
    }

    #[test]
    fn superscript_segments_read_the_caret_marks() {
        let seg = |s: &str| superscript_segments(s);
        assert_eq!(
            seg("2.1×10^6"),
            vec![("2.1×10".into(), false), ("6".into(), true)]
        );
        assert_eq!(
            seg("1.0×10^5 m^2"),
            vec![
                ("1.0×10".into(), false),
                ("5".into(), true),
                (" m".into(), false),
                ("2".into(), true),
            ]
        );
        assert_eq!(
            seg("10^\u{2212}7"),
            vec![("10".into(), false), ("\u{2212}7".into(), true)]
        );
        assert_eq!(seg("a ^ b"), vec![("a ^ b".into(), false)]);
        assert_eq!(seg("2.1e6"), vec![("2.1e6".into(), false)]);
    }

    #[test]
    fn only_the_highlighted_cells_are_reformatted() {
        let mut g = TableGrid::default();
        g.paste("2.1×10^6\t8.2×10^7\n1.6×10^7\t3.5×10^8");
        // Select the right-hand column only.
        g.set_cursor(CellPos::new(0, 1), false);
        g.set_cursor(CellPos::new(1, 1), true);
        assert_eq!(g.reformat_standard_form(), 2);
        assert_eq!(g.to_csv(), "2.1×10^6,8.2e7\n1.6×10^7,3.5e8\n");
    }

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
