//! ENDF tape model — the data substrate every NJOY module operates on.
//!
//! ## ENDF ASCII line format
//!
//! Every line is exactly 80 characters:
//!
//! ```text
//! CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC MMMM FF MMM NNNNN
//! |<---------- 6 × 11-char float fields (cols 1–66) --------->| MAT  MF  MT  seq
//! ```
//!
//! The float format is ENDF's own: `+1.234567+6` encodes 1.234567 × 10⁶.
//! There is no `E`; the exponent sign immediately follows the mantissa digits.
//! Blank fields read as 0.0 (Fortran `e11.0` semantics).
//!
//! ## Section model
//!
//! The tape is a sequence of **sections** identified by `(MAT, MF, MT)`.
//! Sentinel records mark structure boundaries:
//!
//! | Record | MAT | MF | MT | Meaning |
//! |--------|-----|----|----|---------|
//! | SEND   | mat |  f |  0 | end of section (MT) |
//! | FEND   | mat |  0 |  0 | end of file (MF) |
//! | MEND   |  0  |  0 |  0 | end of material (MAT) |
//! | TEND   | −1  |  0 |  0 | end of tape |
//!
//! Ported from NJOY2016 `src/endf.f90`.

pub mod interp;
pub mod parse;
pub mod records;
pub mod tape;

pub use parse::parse_endf_float;
pub use records::{Cont, List, Tab1, Tab2};
pub use tape::{Section, Tape};

/// Identifies a single ENDF section by material, file, and section number.
///
/// - `mat` — material number (e.g. 228 for He-4)
/// - `mf`  — file number (1 = general info, 3 = cross sections, 6 = products, …)
/// - `mt`  — reaction/section number (e.g. 2 = elastic, 102 = capture, …)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EndfKey {
    /// Material number (`MAT`).
    pub mat: i32,
    /// File number (`MF`).
    pub mf: i32,
    /// Section number (`MT`).
    pub mt: i32,
}
