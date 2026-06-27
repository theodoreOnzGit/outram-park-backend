//! ENDF record types parsed from raw line data.
//!
//! ENDF structures are built from a small vocabulary of record types. Each type
//! corresponds to one or more 80-character lines. Ported from the `contio`,
//! `listio`, `tab1io`, `tab2io` subroutines in NJOY2016 `endf.f90`.
//!
//! All parsers operate on a slice of `[f64; 6]` rows (one per ENDF data line,
//! already parsed by [`super::parse`]). The slice is advanced by a cursor so
//! multi-record sections can be read sequentially.

use crate::NjoyError;

/// The six-word header present at the start of every ENDF record.
///
/// Corresponds to the CONT record (`contio`) in NJOY.
///
/// - `c1`, `c2` — two floating-point values (ZA, AWR, or other physic quantities
///   depending on the section)
/// - `l1`, `l2` — integer flags (stored as f64 in ENDF, converted by `nint`)
/// - `n1`, `n2` — integer counts
#[derive(Debug, Clone, PartialEq)]
pub struct Cont {
    /// First floating-point field.
    pub c1: f64,
    /// Second floating-point field.
    pub c2: f64,
    /// First integer flag.
    pub l1: i32,
    /// Second integer flag.
    pub l2: i32,
    /// First integer count (often `NR` — number of interpolation regions, or `NL`, …).
    pub n1: i32,
    /// Second integer count (often `NP` — number of data points, or `NZ`, …).
    pub n2: i32,
}

impl Cont {
    /// Parse a CONT record from the six fields of one ENDF line.
    pub fn from_fields(f: &[f64; 6]) -> Self {
        Cont {
            c1: f[0],
            c2: f[1],
            l1: f[2].round() as i32,
            l2: f[3].round() as i32,
            n1: f[4].round() as i32,
            n2: f[5].round() as i32,
        }
    }
}

/// A LIST record: a CONT header followed by `n1` floating-point values.
///
/// Corresponds to `listio` in NJOY. Used for resonance parameters, covariance
/// matrices, and many other array-valued sections.
#[derive(Debug, Clone)]
pub struct List {
    /// Section header (C1, C2, L1, L2, N1=NPL, N2).
    pub head: Cont,
    /// The `N1` data values that follow the header.
    pub data: Vec<f64>,
}

/// A TAB1 record: a CONT header + `n1` interpolation regions + `n2` (x, y) pairs.
///
/// Corresponds to `tab1io` in NJOY. The most common cross-section record format:
/// - `head.n1` (`NR`) — number of interpolation regions
/// - `head.n2` (`NP`) — number of (energy, cross-section) pairs
/// - `interp` — region table: pairs `(NBT_r, INT_r)` where `NBT_r` is the last
///   energy point in region r and `INT_r` is the interpolation law (1=const,
///   2=lin-lin, 3=lin-log, 4=log-lin, 5=log-log)
/// - `pairs` — the `NP` (x, y) data pairs in reading order
#[derive(Debug, Clone)]
pub struct Tab1 {
    /// Section header (NR = n1, NP = n2).
    pub head: Cont,
    /// Interpolation region table: `(nbt, int_law)` pairs, length = `head.n1`.
    pub interp: Vec<(u32, u32)>,
    /// The `NP` (x, y) data pairs. Length = `head.n2`.
    pub pairs: Vec<(f64, f64)>,
}

/// A TAB2 record: a CONT header + `n1` interpolation regions (no data pairs).
///
/// Corresponds to `tab2io` in NJOY. Used as the outer axis of 2-D tabulated data
/// (e.g. the energy grid in MF=6); the actual data rows are attached separately
/// as LIST or TAB1 records indexed by the outer-axis values.
#[derive(Debug, Clone)]
pub struct Tab2 {
    /// Section header (NR = n1, NZ = n2).
    pub head: Cont,
    /// Interpolation region table: `(nbt, int_law)` pairs, length = `head.n1`.
    pub interp: Vec<(u32, u32)>,
}

/// Cursor into a section's data rows, used to read records sequentially.
///
/// A section is stored as a `Vec<[f64; 6]>` — one entry per non-sentinel
/// ENDF data line. The cursor advances by the number of rows consumed.
pub struct SectionCursor<'a> {
    rows: &'a [[f64; 6]],
    pos: usize,
}

impl<'a> SectionCursor<'a> {
    /// Create a new cursor starting at row 0.
    pub fn new(rows: &'a [[f64; 6]]) -> Self {
        SectionCursor { rows, pos: 0 }
    }

    /// How many rows remain unread.
    pub fn remaining(&self) -> usize {
        self.rows.len().saturating_sub(self.pos)
    }

    fn next_row(&mut self) -> Result<&[f64; 6], NjoyError> {
        if self.pos >= self.rows.len() {
            return Err(NjoyError::EndfParse("unexpected end of section data".into()));
        }
        let row = &self.rows[self.pos];
        self.pos += 1;
        Ok(row)
    }

    /// Read one CONT record (one row).
    pub fn read_cont(&mut self) -> Result<Cont, NjoyError> {
        let row = self.next_row()?;
        Ok(Cont::from_fields(row))
    }

    /// Read one LIST record (CONT header + `n1` data values, possibly spanning
    /// multiple 6-per-row lines).
    pub fn read_list(&mut self) -> Result<List, NjoyError> {
        let head = self.read_cont()?;
        let n = head.n1 as usize;
        let mut data = Vec::with_capacity(n);
        let mut remaining = n;
        while remaining > 0 {
            let row = self.next_row()?;
            let take = remaining.min(6);
            data.extend_from_slice(&row[..take]);
            remaining -= take;
        }
        Ok(List { head, data })
    }

    /// Read one TAB1 record (CONT + NR interp pairs + NP (x,y) pairs).
    pub fn read_tab1(&mut self) -> Result<Tab1, NjoyError> {
        let head = self.read_cont()?;
        let nr = head.n1 as usize;
        let np = head.n2 as usize;
        // Interpolation table: 2*NR values packed 6 per row
        let interp = read_pairs(self, 2 * nr)?
            .chunks(2)
            .map(|c| (c[0] as u32, c[1] as u32))
            .collect();
        // (x, y) pairs: 2*NP values packed 6 per row
        let xy_flat = read_pairs(self, 2 * np)?;
        let pairs = xy_flat
            .chunks(2)
            .map(|c| (c[0], c[1]))
            .collect();
        Ok(Tab1 { head, interp, pairs })
    }

    /// Read one TAB2 record (CONT + NR interp pairs only).
    pub fn read_tab2(&mut self) -> Result<Tab2, NjoyError> {
        let head = self.read_cont()?;
        let nr = head.n1 as usize;
        let interp = read_pairs(self, 2 * nr)?
            .chunks(2)
            .map(|c| (c[0] as u32, c[1] as u32))
            .collect();
        Ok(Tab2 { head, interp })
    }
}

/// Read `n` values from the cursor (packed 6 per row) into a flat `Vec<f64>`.
fn read_pairs(cur: &mut SectionCursor<'_>, n: usize) -> Result<Vec<f64>, NjoyError> {
    let mut out = Vec::with_capacity(n);
    let mut remaining = n;
    while remaining > 0 {
        let row = cur.next_row()?;
        let take = remaining.min(6);
        out.extend_from_slice(&row[..take]);
        remaining -= take;
    }
    Ok(out)
}
