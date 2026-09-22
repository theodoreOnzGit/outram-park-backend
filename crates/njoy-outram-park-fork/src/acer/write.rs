//! Serialise an [`AceTable`] by handing it to the **one** ACE serialiser.
//!
//! ## What this used to be, and why it changed (2026-09-22)
//!
//! This file was a second, independent implementation of the Type-1 writer:
//! its own header layout, its own IZ/AW and NXS/JXS loops, its own XSS loop,
//! and its own copies of `fortran_e`, `fortran_f`, `fortran_f0` and the text
//! padding. [`RawAceTable::to_type1_string`] was a third. Two writers of one
//! format is one writer and one latent bug, and that is exactly what
//! happened: the copies had drifted, and the read-side one wrote `1pE11.4` of
//! zero as `0.0000`, dropped `f11.0`'s decimal point and used Rust's `{:E}`
//! exponent. Every byte after those header fields shifted, and nothing caught
//! it because the only test of that path compared *values*.
//!
//! So [`AceTable`] now converts to a [`RawAceTable`] — the type the ported
//! reader produces — and the serialisation happens once, in
//! [`RawAceTable::to_type1_string`] and [`RawAceTable::to_type2_bytes`], on
//! the shared edit descriptors in [`crate::acer::fortran_fmt`]. The output is
//! unchanged; what changed is that there is now nothing to drift.
//!
//! The on-disk layout, for reference:
//!
//! ```text
//! line 1:  ZAID(a10) AWR(f12.6) ' ' kT(1pe11.4) ' ' date(a10)
//! line 2:  comment(a70) mat-id(a10)
//! 4 lines: 16 × (IZ(i7) AW(f11.0))           — 4 pairs per line
//! 6 lines: NXS(1..16) then JXS(1..32)        — 8 integers (i9) per line
//! N lines: XSS data                          — 4 values per line, 20-char fields
//! ```
//!
//! Each XSS word is written as an integer (`i20`) or a real (`1pE20.11`) per
//! its [`AceTable::xss_is_int`] flag, four to a line — the line breaks are
//! positional (every fourth value), exactly as NJOY's `typen` buffers them.
//! That flag is carried into the raw table rather than re-derived, so a
//! built table never falls back to the reader's heuristic.

use std::io::{self, Write};
use std::path::Path;

use super::fortran_fmt::fixed;
use super::read::{AceClass, AceFileType, AceHeader, RawAceTable};
use super::AceTable;
use crate::NjoyError;

impl AceTable {
    /// This table as the container-agnostic [`RawAceTable`] the ported reader
    /// and writers share.
    ///
    /// The class is taken from the ZAID's last letter, the same rule
    /// [`crate::acer::read::read_type1`] applies, so a table round-trips
    /// through a file as the class it was built as. An unrecognised letter is
    /// an error rather than a silent default — a table whose class nothing can
    /// name cannot be written correctly by any of the class writers.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when the ZAID does not end in one of
    /// upstream's class letters.
    pub fn to_raw(&self, file_type: AceFileType) -> Result<RawAceTable, NjoyError> {
        let letter = self.zaid.trim().chars().last().ok_or_else(|| {
            NjoyError::EndfParse("AceTable: empty ZAID, so no class can be determined".into())
        })?;
        let class = AceClass::from_letter(letter).ok_or_else(|| {
            NjoyError::EndfParse(format!(
                "AceTable: ZAID {:?} ends in {letter:?}, which is not one of upstream's \
                 class letters c/h/o/r/s/a/t/p/u/y (acer.f90:513-533)",
                self.zaid
            ))
        })?;
        let zaid_num = if class == AceClass::Thermal {
            None
        } else {
            self.zaid.trim()[..self.zaid.trim().len() - letter.len_utf8()]
                .trim()
                .parse::<f64>()
                .ok()
        };
        Ok(RawAceTable {
            file_type,
            header: AceHeader {
                raw_text: [
                    fixed(&self.zaid, 10).into_bytes(),
                    fixed(&self.date, 10).into_bytes(),
                    fixed(&self.comment, 70).into_bytes(),
                    fixed(&self.mat_id, 10).into_bytes(),
                ],
                zaid: self.zaid.clone(),
                zaid_num,
                class,
                awr: self.awr,
                kt_mev: self.kt_mev,
                date: self.date.clone(),
                comment: self.comment.clone(),
                mat_id: self.mat_id.clone(),
                // A table this crate builds carries no IZ/AW pairs; upstream
                // fills them only from ACER's `nxtra` cards.
                iz: [0; 16],
                aw: [0.0; 16],
            },
            nxs: self.nxs,
            jxs: self.jxs,
            xss_is_int: Some(self.xss_is_int.clone()),
            xss: self.xss.clone(),
        })
    }

    /// Write this table to `path` as a Type-1 ASCII ACE file.
    ///
    /// # Errors
    /// Propagates any [`std::io::Error`] from creating or writing the file.
    /// A ZAID whose class letter is unrecognised is reported as an
    /// [`io::ErrorKind::InvalidData`] error.
    pub fn write_type1<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut f = io::BufWriter::new(std::fs::File::create(path)?);
        self.write_to(&mut f)
    }

    /// Write this table in Type-1 ASCII to any [`Write`] sink.
    ///
    /// Exposed (rather than only [`write_type1`][Self::write_type1]) so callers
    /// can serialise to an in-memory buffer — used by the round-trip tests.
    ///
    /// # Errors
    /// As [`write_type1`][Self::write_type1].
    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let raw = self
            .to_raw(AceFileType::Type1Ascii)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        w.write_all(raw.to_type1_string().as_bytes())
    }

    /// Write this table as a **Type 2** (Fortran unformatted sequential) ACE
    /// file — the container `acer`'s `itype = 2` produces.
    ///
    /// Available only since the two writers were merged: the old
    /// `AceTable`-specific serialiser could emit Type 1 alone, so a
    /// continuous-energy or thermal table built here had no binary form.
    ///
    /// # Errors
    /// As [`write_type1`][Self::write_type1], plus any write failure.
    pub fn write_type2<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        self.to_raw(AceFileType::Type2Binary)?.write_type2(path)
    }
}
