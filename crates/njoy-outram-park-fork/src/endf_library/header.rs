// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **What an ENDF-6 tape says it is**, read from the first records of its
//! MF=1/MT=451 section (ENDF-102, the ENDF-6 Formats Manual, CSEWG document
//! BNL-203218-2018-INRE, section 1.1 and Tables 1-3).
//!
//! | record | fields |
//! |---|---|
//! | HEAD | ZA, AWR, LRP, LFI, **NLIB**, NMOD |
//! | CONT | ELIS, STA, LIS, **LISO**, 0, **NFOR** |
//! | CONT | AWI, EMAX, **LREL**, 0, **NSUB**, **NVER** |
//! | CONT | TEMP, 0, LDRV, 0, NWD, NXC |
//! | TEXT ×3 | ZSYMAM ALAB EDATE AUTH; REF DDATE …; **HSUB** `----ENDF/B-VIII.0  MATERIAL 9228` |
//!
//! The third text line, `HSUB`, names the library and its release in words
//! (`ENDF/B-VIII.0`, `ENDF/B-VII.1`, `JENDL-3.3`, `TENDL-2023`). It is the
//! most specific identity a tape carries: on the workspace's reference tapes
//! the `LREL` field does **not** separate releases (an ENDF/B-VII.0 B-10 tape
//! carries `LREL = 1`, the same as a VII.1 Cl-35 tape, and an ENDF/B-VIII.1
//! Sr-88 tape carries `LREL = 0`, like VIII.0), while `HSUB` does.

use crate::endf::parse::parse_endf_float;

/// The header of an ENDF-6 tape: identity, library and sub-library.
#[derive(Debug, Clone, PartialEq)]
pub struct EndfHeader {
    /// `MAT` (columns 67-70 of the HEAD record); `None` when the line is too
    /// short to carry it.
    pub mat: Option<i32>,
    /// `ZA = 1000 Z + A` (natural elements have `A = 0`).
    pub za: u32,
    /// Atomic weight ratio to the neutron.
    pub awr: f64,
    /// Library number `NLIB` (ENDF-102 Table 2: 0 ENDF/B, 2 JEFF, 6 JENDL,
    /// 17 TENDL, ...).
    pub nlib: i64,
    /// Isomeric state of the target, `LISO` (0 ground).
    pub liso: u8,
    /// Format number `NFOR` (6 for ENDF-6); `None` when the record is absent.
    pub nfor: Option<i64>,
    /// Release number `LREL`; `None` when the record is absent.
    pub lrel: Option<i64>,
    /// Sub-library `NSUB` (10 incident neutrons, 12 thermal scattering, ...);
    /// `None` when the record is absent.
    pub nsub: Option<i64>,
    /// Library version `NVER`; `None` when the record is absent.
    pub nver: Option<i64>,
    /// `ZSYMAM` (`" 92-U -235 "`), trimmed; empty when absent.
    pub symbol: String,
    /// The library named on the `HSUB` line, trimmed (`"ENDF/B-VIII.0"`);
    /// `None` when the tape has no such line.
    pub library_text: Option<String>,
}

/// One ENDF line split into its 6 data fields and its MAT/MF/MT columns.
struct Line<'a> {
    text: &'a str,
    mf_mt: Option<(i64, i64)>,
}

fn columns(l: &str, a: usize, b: usize) -> &str {
    l.get(a..b.min(l.len())).unwrap_or("").trim()
}

fn field(l: &str, i: usize) -> &str {
    columns(l, 11 * i, 11 * i + 11)
}

fn int(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok()
}

impl EndfHeader {
    /// **Read the header from the start of a tape** (`text` need hold only
    /// the first dozen lines; two kilobytes is plenty).
    ///
    /// The MF=1/MT=451 records are found by their MF and MT columns (71-75),
    /// so a tape with or without a TPID line reads the same. A text with no
    /// such columns (a hand-made fixture) is read by position, TPID first.
    ///
    /// `None` when the HEAD record is unreadable: not an ENDF tape. A tape
    /// in an older format (`NFOR` other than 6) reads, with its `nfor`, so
    /// the caller can say why it is not used.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let lines: Vec<Line> = text
            .lines()
            .take(40)
            .map(|t| Line {
                text: t,
                mf_mt: match (int(columns(t, 70, 72)), int(columns(t, 72, 75))) {
                    (Some(mf), Some(mt)) => Some((mf, mt)),
                    _ => None,
                },
            })
            .collect();
        let mut records: Vec<&str> = lines
            .iter()
            .filter(|l| l.mf_mt == Some((1, 451)))
            .map(|l| l.text)
            .take(7)
            .collect();
        if records.is_empty() {
            records = lines.iter().skip(1).take(7).map(|l| l.text).collect();
        }
        let head = records.first()?;
        let za = parse_endf_float(field(head, 0)).ok()?;
        let awr = parse_endf_float(field(head, 1)).ok()?;
        if !(za.is_finite() && za >= 0.0 && awr.is_finite() && awr > 0.0) {
            return None;
        }
        let za = u32::try_from(za.round() as i64).ok()?;
        let nlib = int(field(head, 4)).unwrap_or(0);
        let mat = int(columns(head, 66, 70)).and_then(|m| i32::try_from(m).ok());
        let r2 = records.get(1);
        let r3 = records.get(2);
        let liso = r2.and_then(|l| int(field(l, 3))).unwrap_or(0);
        let text_lines: &[&str] = records.get(4..).unwrap_or(&[]);
        let library_text = text_lines
            .iter()
            .find_map(|l| l.strip_prefix("----"))
            .map(|rest| {
                let rest = rest.get(..66.min(rest.len())).unwrap_or(rest);
                let upper = rest.to_ascii_uppercase();
                let end = upper.find("MATERIAL").unwrap_or(rest.len());
                rest[..end].trim().to_string()
            })
            .filter(|s| !s.is_empty());
        Some(Self {
            mat,
            za,
            awr,
            nlib,
            liso: u8::try_from(liso).ok()?,
            nfor: r2.and_then(|l| int(field(l, 5))),
            lrel: r3.and_then(|l| int(field(l, 2))),
            nsub: r3.and_then(|l| int(field(l, 4))),
            nver: r3.and_then(|l| int(field(l, 5))),
            symbol: text_lines
                .first()
                .map(|l| field(l, 0).to_string())
                .unwrap_or_default(),
            library_text,
        })
    }

    /// Atomic number.
    #[must_use]
    pub fn z(&self) -> u32 {
        self.za / 1000
    }

    /// Mass number (0 for a natural element).
    #[must_use]
    pub fn a(&self) -> u32 {
        self.za % 1000
    }

    /// What kind of data this tape holds, from `NSUB`.
    #[must_use]
    pub fn sub_library(&self) -> SubLibrary {
        match self.nsub {
            Some(10) => SubLibrary::IncidentNeutron,
            Some(12) => SubLibrary::ThermalScattering,
            Some(n) => SubLibrary::Other(n),
            None => SubLibrary::Unknown,
        }
    }

    /// Whether this is an ENDF-6 tape (`NFOR = 6`), the format this crate
    /// processes. A fixture without the second record is taken as ENDF-6.
    #[must_use]
    pub fn is_endf6(&self) -> bool {
        self.nfor.is_none_or(|f| f == 6)
    }

    /// **The library this tape belongs to, as one string**: the `HSUB` name
    /// when the tape has one (`"ENDF/B-VIII.0"`), else the family from
    /// `NLIB` (ENDF-102 Table 2) with `NVER` (`"JEFF (NLIB 2) version 3"`).
    /// Two tapes of one library give the same string.
    #[must_use]
    pub fn library(&self) -> String {
        if let Some(t) = &self.library_text {
            return t.clone();
        }
        let family = library_family(self.nlib);
        match self.nver {
            Some(v) => format!("{family} (NLIB {}) version {v}", self.nlib),
            None => format!("{family} (NLIB {})", self.nlib),
        }
    }
}

/// What a tape holds, from its sub-library number (ENDF-102 Table 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubLibrary {
    /// `NSUB = 10`: incident-neutron data, what transport is processed from.
    IncidentNeutron,
    /// `NSUB = 12`: thermal neutron scattering, `S(α,β)`.
    ThermalScattering,
    /// Any other sub-library (decay, photo-atomic, fission yields, ...).
    Other(i64),
    /// The record holding `NSUB` is absent.
    Unknown,
}

/// The evaluation group for an `NLIB` number, as ENDF-102 (2018) Table 2
/// lists them.
#[must_use]
pub fn library_family(nlib: i64) -> &'static str {
    match nlib {
        0 => "ENDF/B",
        1 => "ENDF/A",
        2 => "JEFF",
        3 => "EFF",
        4 => "ENDF/B high energy",
        5 => "CENDL",
        6 => "JENDL",
        17 => "TENDL",
        18 => "ROSFOND",
        21 => "SG-23",
        31 => "INDL/V",
        32 => "INDL/A",
        33 => "FENDL",
        34 => "IRDF",
        35 => "BROND (IAEA version)",
        36 => "INGDB-90",
        37 => "FENDL/A",
        41 => "BROND",
        _ => "unlisted library",
    }
}
