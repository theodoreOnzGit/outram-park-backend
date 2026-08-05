//! Assembly-composition and control-rod-bank maps, and MATLAB `readmatrix`
//! semantics.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source | the ten `*.csv` inputs of the BEDOK MATLAB snapshot, read by `readmatrix` in `iaea3ds.m`, `neacrpa2.m` and `neacrpd1.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # What these files are
//!
//! All ten CSVs are **inputs**, not reference outputs. Nine of them are
//! 17 × 17 maps over the modelled core quadrant/octant; each entry is a
//! *material index* into the case's cross-section tables (or `0` for "outside
//! the core", which the solver excludes from the unknowns). The tenth,
//! `NEACRPD1_COL.csv`, is a 14 × 10 table mapping *(axial level, radial column
//! type)* to a material index — the BWR case composes its 3-D material map from
//! the radial map plus this axial column table.
//!
//! `NEACRPA2_CRODBANKS.csv` is different in kind: its entries are **control-rod
//! bank numbers** (`0` = no rod at that radial position), indexing
//! `geometry.crod`.
//!
//! # Two MATLAB behaviours that must be reproduced exactly
//!
//! 1. **`readmatrix('IAEA3DS_1')` appends `.csv`** to a name with no extension.
//!    Reproduced here by naming the files through [`CompositionMap`] rather
//!    than by string.
//! 2. **Every file carries a UTF-8 byte-order mark** (`EF BB BF`). MATLAB
//!    tolerates it; a naive reader parses the first field as `NaN`. [`parse`]
//!    strips it.
//!
//! The files are embedded with `include_str!`, so nothing is read from disk at
//! run time and the maps travel with the compiled library.

use crate::error::{BedokError, Result};

const IAEA3DS_1: &str = include_str!("data/IAEA3DS_1.csv");
const IAEA3DS_2: &str = include_str!("data/IAEA3DS_2.csv");
const IAEA3DS_3: &str = include_str!("data/IAEA3DS_3.csv");
const IAEA3DS_4: &str = include_str!("data/IAEA3DS_4.csv");
const NEACRPA2_1: &str = include_str!("data/NEACRPA2_1.csv");
const NEACRPA2_2: &str = include_str!("data/NEACRPA2_2.csv");
const NEACRPA2_3: &str = include_str!("data/NEACRPA2_3.csv");
const NEACRPA2_CRODBANKS: &str = include_str!("data/NEACRPA2_CRODBANKS.csv");
const NEACRPD1_1: &str = include_str!("data/NEACRPD1_1.csv");
const NEACRPD1_COL: &str = include_str!("data/NEACRPD1_COL.csv");

/// One of the ten embedded input maps.
///
/// The variant *is* the filename in the MATLAB, so a case constructor never
/// spells a path and the `.csv`-appending behaviour of `readmatrix` cannot be
/// got wrong. Enum dispatch, per the workspace Rust rules — there is no
/// mechanism here for loading an arbitrary file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionMap {
    /// IAEA-3D axial level 1: the bottom axial reflector. `readmatrix('IAEA3DS_1')`.
    Iaea3dsBottomReflector,
    /// IAEA-3D axial levels 2–14: the lower fuelled region. `readmatrix('IAEA3DS_2')`.
    Iaea3dsLowerFuel,
    /// IAEA-3D axial levels 15–18: the upper fuelled region, where the partly
    /// inserted rods sit. `readmatrix('IAEA3DS_3')`.
    Iaea3dsUpperFuel,
    /// IAEA-3D axial level 19: the top axial reflector. `readmatrix('IAEA3DS_4')`.
    Iaea3dsTopReflector,
    /// NEACRP PWR axial reflector plane (levels 1 and 18). `readmatrix('NEACRPA2_1')`.
    NeacrpA2AxialReflector,
    /// NEACRP PWR axial level 2 (the bottom fuelled plane). `readmatrix('NEACRPA2_2')`.
    NeacrpA2LowerFuel,
    /// NEACRP PWR axial levels 3–17 (the bulk of the fuel). `readmatrix('NEACRPA2_3')`.
    NeacrpA2MainFuel,
    /// NEACRP PWR control-rod bank numbers per radial position.
    /// `readmatrix('NEACRPA2_CRODBANKS')`.
    NeacrpA2ControlRodBanks,
    /// NEACRP BWR radial map: per radial position, which *column type*
    /// (1–10, `0` = outside the core). `readmatrix('NEACRPD1_1')`.
    NeacrpD1RadialColumns,
    /// NEACRP BWR axial column table: material index by (axial level 1–14,
    /// column type 1–10). `readmatrix('NEACRPD1_COL')`.
    NeacrpD1ColumnTable,
}

impl CompositionMap {
    /// The MATLAB name this map is loaded under, without the `.csv` extension
    /// MATLAB appends — i.e. exactly the string passed to `readmatrix`.
    #[must_use]
    pub const fn matlab_name(self) -> &'static str {
        match self {
            Self::Iaea3dsBottomReflector => "IAEA3DS_1",
            Self::Iaea3dsLowerFuel => "IAEA3DS_2",
            Self::Iaea3dsUpperFuel => "IAEA3DS_3",
            Self::Iaea3dsTopReflector => "IAEA3DS_4",
            Self::NeacrpA2AxialReflector => "NEACRPA2_1",
            Self::NeacrpA2LowerFuel => "NEACRPA2_2",
            Self::NeacrpA2MainFuel => "NEACRPA2_3",
            Self::NeacrpA2ControlRodBanks => "NEACRPA2_CRODBANKS",
            Self::NeacrpD1RadialColumns => "NEACRPD1_1",
            Self::NeacrpD1ColumnTable => "NEACRPD1_COL",
        }
    }

    /// The embedded file contents, byte-order mark included.
    #[must_use]
    const fn raw(self) -> &'static str {
        match self {
            Self::Iaea3dsBottomReflector => IAEA3DS_1,
            Self::Iaea3dsLowerFuel => IAEA3DS_2,
            Self::Iaea3dsUpperFuel => IAEA3DS_3,
            Self::Iaea3dsTopReflector => IAEA3DS_4,
            Self::NeacrpA2AxialReflector => NEACRPA2_1,
            Self::NeacrpA2LowerFuel => NEACRPA2_2,
            Self::NeacrpA2MainFuel => NEACRPA2_3,
            Self::NeacrpA2ControlRodBanks => NEACRPA2_CRODBANKS,
            Self::NeacrpD1RadialColumns => NEACRPD1_1,
            Self::NeacrpD1ColumnTable => NEACRPD1_COL,
        }
    }

    /// Parse this map into a dense numeric matrix.
    ///
    /// Equivalent to MATLAB `readmatrix('<name>')`.
    ///
    /// # Errors
    ///
    /// [`BedokError::Fixture`] if the embedded file is not rectangular or holds
    /// a field that is not a number.
    pub fn load(self) -> Result<NumericMatrix> {
        parse(self.matlab_name(), self.raw())
    }
}

/// A dense matrix of `f64`, the shape MATLAB's `readmatrix` returns.
///
/// Entries are dimensionless: material indices, column-type indices or
/// control-rod bank numbers depending on the file. Stored row-major.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericMatrix {
    rows: usize,
    cols: usize,
    values: Vec<f64>,
}

impl NumericMatrix {
    /// Number of rows. MATLAB `size(M,1)`.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns. MATLAB `size(M,2)`.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// Entry at **0-based** `(row, col)`.
    ///
    /// # Panics
    ///
    /// If either index is out of range — an out-of-range map lookup means the
    /// grid and the map disagree, which must not be papered over.
    #[must_use]
    pub fn at(&self, row: usize, col: usize) -> f64 {
        assert!(row < self.rows, "row {row} >= {}", self.rows);
        assert!(col < self.cols, "col {col} >= {}", self.cols);
        self.values[row * self.cols + col]
    }

    /// Entry at **1-based** `(row, col)`, i.e. MATLAB `M(row, col)`.
    ///
    /// The case constructors index these maps with expressions such as
    /// `ceil(ix/maxix*17)`, which are naturally 1-based; this accessor is the
    /// single place the conversion happens.
    ///
    /// # Panics
    ///
    /// If either index is zero or out of range.
    #[must_use]
    pub fn at_matlab(&self, row: usize, col: usize) -> f64 {
        assert!(row >= 1 && col >= 1, "MATLAB indices start at 1");
        self.at(row - 1, col - 1)
    }

    /// Entry at 1-based `(row, col)` as a material / bank index.
    ///
    /// # Errors
    ///
    /// [`BedokError::Fixture`] if the entry is negative or not an integer —
    /// every entry of every embedded map is a small non-negative integer, so
    /// anything else means the file was corrupted.
    pub fn index_at_matlab(&self, row: usize, col: usize) -> Result<usize> {
        let v = self.at_matlab(row, col);
        if v < 0.0 || v.fract() != 0.0 {
            return Err(BedokError::Fixture {
                path: "composition map".to_string(),
                reason: format!("entry ({row},{col}) = {v} is not a non-negative integer"),
            });
        }
        Ok(v as usize)
    }
}

/// Parse CSV text the way MATLAB's `readmatrix` does for these files.
///
/// Strips a leading UTF-8 byte-order mark, accepts CRLF line endings, and
/// requires the result to be rectangular.
///
/// `name` is used only in error messages.
///
/// # Errors
///
/// [`BedokError::Fixture`] if a field does not parse as a number, if rows have
/// differing lengths, or if the text holds no rows.
pub fn parse(name: &str, text: &str) -> Result<NumericMatrix> {
    // MATLAB tolerates the BOM; a plain reader would turn the first field into
    // NaN. See the module docs.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut values: Vec<f64> = Vec::new();
    let mut rows = 0usize;
    let mut cols = 0usize;

    for record in reader.records() {
        let record = record.map_err(|e| BedokError::Fixture {
            path: name.to_string(),
            reason: format!("csv parse failed: {e}"),
        })?;
        // MATLAB's readmatrix silently drops a trailing empty field produced by
        // a line that ends in a delimiter; these files do not have one, but a
        // wholly empty record must not become a zero-width row.
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let mut row_len = 0usize;
        for field in record.iter() {
            let field = field.trim();
            let v: f64 = field.parse().map_err(|_| BedokError::Fixture {
                path: name.to_string(),
                reason: format!(
                    "row {} field {row_len}: {field:?} is not a number",
                    rows + 1
                ),
            })?;
            values.push(v);
            row_len += 1;
        }
        if rows == 0 {
            cols = row_len;
        } else if row_len != cols {
            return Err(BedokError::Fixture {
                path: name.to_string(),
                reason: format!("row {} has {row_len} fields, expected {cols}", rows + 1),
            });
        }
        rows += 1;
    }

    if rows == 0 || cols == 0 {
        return Err(BedokError::Fixture {
            path: name.to_string(),
            reason: "no data rows".to_string(),
        });
    }

    Ok(NumericMatrix { rows, cols, values })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every embedded map parses, and the nine radial maps are 17 × 17 — the
    /// modelled quadrant of a 17-assembly-wide core.
    #[test]
    fn radial_maps_are_seventeen_by_seventeen() {
        for map in [
            CompositionMap::Iaea3dsBottomReflector,
            CompositionMap::Iaea3dsLowerFuel,
            CompositionMap::Iaea3dsUpperFuel,
            CompositionMap::Iaea3dsTopReflector,
            CompositionMap::NeacrpA2AxialReflector,
            CompositionMap::NeacrpA2LowerFuel,
            CompositionMap::NeacrpA2MainFuel,
            CompositionMap::NeacrpA2ControlRodBanks,
            CompositionMap::NeacrpD1RadialColumns,
        ] {
            let m = map
                .load()
                .unwrap_or_else(|e| panic!("{}: {e}", map.matlab_name()));
            assert_eq!(m.rows(), 17, "{} rows", map.matlab_name());
            assert_eq!(m.cols(), 17, "{} cols", map.matlab_name());
        }
    }

    /// The BWR axial column table is 14 axial levels by 10 column types, which
    /// is why `neacrpd1.m` forces `maxiz = 14`.
    #[test]
    fn bwr_column_table_is_fourteen_by_ten() {
        let m = CompositionMap::NeacrpD1ColumnTable.load().expect("parses");
        assert_eq!(m.rows(), 14);
        assert_eq!(m.cols(), 10);
    }

    /// Spot values, read directly off the source CSVs.
    #[test]
    fn spot_values_match_the_source_files() {
        // IAEA3DS_1 is the bottom reflector: material 4 (reflector) everywhere
        // except the rodded positions, and its first row starts 4,4,4,...
        let m = CompositionMap::Iaea3dsBottomReflector
            .load()
            .expect("parses");
        assert_eq!(m.at_matlab(1, 1), 4.0);
        assert_eq!(m.at_matlab(1, 17), 4.0);

        // IAEA3DS_2 first row: 3,2,2,2,2,2,2,3,3,2,2,2,2,1,1,4,4
        let m = CompositionMap::Iaea3dsLowerFuel.load().expect("parses");
        assert_eq!(m.at_matlab(1, 1), 3.0, "central rodded inner fuel");
        assert_eq!(m.at_matlab(1, 2), 2.0, "inner fuel");
        assert_eq!(m.at_matlab(1, 14), 1.0, "outer fuel");
        assert_eq!(m.at_matlab(1, 16), 4.0, "radial reflector");

        // IAEA3DS_4 is the top reflector, with rodded reflector (5) where the
        // rods are: first row 5,4,4,4,4,4,4,5,5,...
        let m = CompositionMap::Iaea3dsTopReflector.load().expect("parses");
        assert_eq!(m.at_matlab(1, 1), 5.0);
        assert_eq!(m.at_matlab(1, 2), 4.0);
        assert_eq!(m.at_matlab(1, 8), 5.0);

        // Control-rod banks: first row 1,0,0,2,2,0,0,0,0,0,0,3,3,0,0,0,0
        let m = CompositionMap::NeacrpA2ControlRodBanks
            .load()
            .expect("parses");
        assert_eq!(m.at_matlab(1, 1), 1.0, "central CA is bank 1");
        assert_eq!(m.at_matlab(1, 2), 0.0, "no rod");
        assert_eq!(m.at_matlab(1, 4), 2.0);
        assert_eq!(m.at_matlab(1, 12), 3.0);
        assert_eq!(m.at_matlab(2, 2), 4.0, "second row starts 0,4,4,0,0,...");

        // BWR radial map first row: 8,6,6,9,9,6,6,7,7,6,6,4,4,2,2,10,10
        let m = CompositionMap::NeacrpD1RadialColumns
            .load()
            .expect("parses");
        assert_eq!(m.at_matlab(1, 1), 8.0);
        assert_eq!(m.at_matlab(1, 16), 10.0);
        assert_eq!(m.at_matlab(17, 17), 0.0, "outside the core");

        // BWR column table: bottom level is all material 1 except column 10,
        // and the top level (row 14) is all material 4 except column 10.
        let m = CompositionMap::NeacrpD1ColumnTable.load().expect("parses");
        assert_eq!(m.at_matlab(1, 1), 1.0);
        assert_eq!(
            m.at_matlab(1, 10),
            19.0,
            "column 10 is the radial reflector"
        );
        assert_eq!(m.at_matlab(14, 1), 4.0);
        assert_eq!(m.at_matlab(2, 2), 5.0);
    }

    /// The byte-order mark is stripped rather than poisoning the first field.
    #[test]
    fn byte_order_mark_is_stripped() {
        let m = parse("bom", "\u{feff}1,2\n3,4\n").expect("parses");
        assert_eq!(m.at(0, 0), 1.0);
        assert_eq!(m.at(1, 1), 4.0);
    }

    /// A ragged file is an error, not a silently zero-padded matrix.
    #[test]
    fn ragged_rows_are_rejected() {
        assert!(parse("ragged", "1,2,3\n4,5\n").is_err());
    }

    #[test]
    fn material_indices_come_back_as_integers() {
        let m = CompositionMap::Iaea3dsLowerFuel.load().expect("parses");
        assert_eq!(m.index_at_matlab(1, 1).expect("integer"), 3);
    }
}
