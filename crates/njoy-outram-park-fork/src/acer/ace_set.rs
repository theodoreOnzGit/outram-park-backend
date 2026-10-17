// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! # A set of ACE files, indexed by what their headers say (gh #936)
//!
//! The ACE way in: a user drops several ACE files, a folder of them, or a zip
//! of them, and transport builds its nuclides from them **as they are**: the
//! temperature, the nuclide and the thermal law were fixed when the files were
//! made, and nothing is processed. This is the ACE counterpart of
//! [`crate::endf_library`] (#936 point 4): a table is identified by its own
//! header, the **ZAID** (`92235.00c`), its **class** (the ZAID's letter) and
//! its **kT**, never by its file name. Files that are not ACE are listed as
//! skipped, with the reason.
//!
//! Each file is read with [`super::read::read_library_from_bytes`] (Type 1,
//! Type 2 sequential or direct, gzipped or not, one or several tables); a file
//! that is a zip is opened first ([`crate::archive`]). Bytes in, so it runs in
//! a browser; [`AceSet::from_folder`] is the native wrapper.
//!
//! # Temperature matching
//!
//! A table is "at" a requested temperature `T` when its header kT is within a
//! relative **1e-4** of `k_B T` (`k_B = 8.617333262e-11 MeV/K`, the value this
//! crate's ACER writes with, `acer/mod.rs`). The reason for 1e-4: a Type-1
//! header prints kT as `E12.4`, four decimals of mantissa, so a written table
//! carries a relative rounding of up to 5e-5/1.0 (NJOY's U-234 table at
//! 293.6 K reads `2.5300E-08` against `2.53005e-08` computed, 2e-5). 1e-4 is
//! twice the worst case, and is 0.03 K at 293.6 K, far below any temperature
//! a model would mean to distinguish. A **0 K** table is one with
//! `kT <= 1e-9 eV` (the threshold outram-mc's `Nuclide::from_ace` uses).

use std::path::Path;
use std::sync::Arc;

use super::read::{read_library_from_bytes, AceClass, RawAceTable};
use crate::archive::{looks_like_zip, ZipArchive};
use crate::NjoyError;

/// Boltzmann's constant \[MeV/K\], as this crate's ACER writes kT.
pub const K_BOLTZMANN_MEV_PER_K: f64 = 8.617_333_262e-11;

/// Relative kT tolerance for "this table is at that temperature"; see the
/// module docs for the reason.
pub const KT_RELATIVE_TOLERANCE: f64 = 1.0e-4;

/// kT \[MeV\] at or below which a table is a 0 K table (1e-9 eV).
pub const ZERO_KELVIN_KT_MEV: f64 = 1.0e-15;

/// One table of an [`AceSet`], with the file it came from.
#[derive(Debug, Clone)]
pub struct AceSetTable {
    /// The file (`/`-separated path within the upload, folder or zip).
    pub file: String,
    /// The table as read.
    pub table: Arc<RawAceTable>,
}

impl AceSetTable {
    /// The ZAID as written, trimmed (`92235.00c`, `hh2o.00t`).
    #[must_use]
    pub fn zaid(&self) -> &str {
        self.table.header.zaid.trim()
    }

    /// The class (continuous neutron, thermal, ...).
    #[must_use]
    pub fn class(&self) -> AceClass {
        self.table.header.class
    }

    /// `1000 Z + A` for a numeric ZAID; `None` for a thermal table's name.
    #[must_use]
    pub fn za(&self) -> Option<u32> {
        self.table
            .header
            .zaid_num
            .filter(|z| z.is_finite() && *z >= 0.0)
            .map(|z| z.trunc() as u32)
    }

    /// The ZAID's name part, before the `.` (`92235`, `hh2o`, `tsl1`).
    #[must_use]
    pub fn stem(&self) -> &str {
        self.zaid().split('.').next().unwrap_or("")
    }

    /// kT \[MeV\] from the header.
    #[must_use]
    pub fn kt_mev(&self) -> f64 {
        self.table.header.kt_mev
    }

    /// The header temperature \[K\].
    #[must_use]
    pub fn temperature_k(&self) -> f64 {
        self.kt_mev() / K_BOLTZMANN_MEV_PER_K
    }

    /// A 0 K table.
    #[must_use]
    pub fn is_zero_kelvin(&self) -> bool {
        self.kt_mev() <= ZERO_KELVIN_KT_MEV
    }

    /// Whether this table is at `temperature_k` ([`KT_RELATIVE_TOLERANCE`]).
    #[must_use]
    pub fn is_at(&self, temperature_k: f64) -> bool {
        let want = temperature_k * K_BOLTZMANN_MEV_PER_K;
        if want <= ZERO_KELVIN_KT_MEV {
            return self.is_zero_kelvin();
        }
        ((self.kt_mev() - want) / want).abs() <= KT_RELATIVE_TOLERANCE
    }

    /// `92235.00c (U235.ace.gz, 293.6 K)`: for provenance.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{} ({}, {:.1} K)",
            self.zaid(),
            self.file,
            self.temperature_k()
        )
    }
}

/// Why a lookup in an [`AceSet`] found no single table.
#[derive(Debug, Clone, PartialEq)]
pub enum AceLookupError {
    /// No table of this identity at all.
    Missing,
    /// Tables of this identity exist, at other temperatures \[K\]: a
    /// mismatch is refused, never accepted (#936 point 3).
    WrongTemperature(Vec<f64>),
    /// Several tables match; their files.
    Ambiguous(Vec<String>),
}

impl std::fmt::Display for AceLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "no ACE table for it"),
            Self::WrongTemperature(t) => write!(f, "ACE tables for it exist only at {t:?} K"),
            Self::Ambiguous(v) => write!(f, "several ACE tables match: {v:?}"),
        }
    }
}

/// **Uploaded or folder ACE files, indexed by header.** See the module docs.
#[derive(Debug, Clone, Default)]
pub struct AceSet {
    tables: Vec<AceSetTable>,
    skipped: Vec<(String, String)>,
}

impl AceSet {
    /// **Files as bytes** (`(path, contents)`): several dropped files, a
    /// folder upload, or zips of either. A file that is not ACE is skipped
    /// with the reason; nothing here panics on a malformed file.
    #[must_use]
    pub fn from_files(files: Vec<(String, Vec<u8>)>) -> Self {
        let mut me = Self::default();
        for (path, bytes) in files {
            me.add(path.replace('\\', "/"), bytes);
        }
        me
    }

    fn add(&mut self, path: String, bytes: Vec<u8>) {
        if looks_like_zip(&bytes) {
            match ZipArchive::parse(Arc::new(bytes)) {
                Ok(z) => {
                    for (i, e) in z.entries().iter().enumerate().filter(|(_, e)| !e.is_dir()) {
                        let inner = format!("{path}/{}", e.name);
                        match z.read(i) {
                            Ok(b) if !looks_like_zip(&b) => self.add(inner, b),
                            Ok(_) => self
                                .skipped
                                .push((inner, "a zip inside a zip is not opened".into())),
                            Err(err) => self.skipped.push((inner, err.to_string())),
                        }
                    }
                }
                Err(err) => self.skipped.push((path, err.to_string())),
            }
            return;
        }
        match read_library_from_bytes(bytes, &path) {
            Ok(tables) if !tables.is_empty() => {
                self.tables.extend(tables.into_iter().map(|t| AceSetTable {
                    file: path.clone(),
                    table: Arc::new(t),
                }))
            }
            Ok(_) => self.skipped.push((path, "no ACE table in it".into())),
            Err(err) => self.skipped.push((path, format!("not read as ACE: {err}"))),
        }
    }

    /// **Every file under `dir`** (native only), with paths relative to it.
    ///
    /// # Errors
    /// The folder cannot be read.
    pub fn from_folder(dir: &Path) -> Result<Self, NjoyError> {
        let mut files = Vec::new();
        collect(dir, "", &mut files)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Self::from_files(files))
    }

    /// Every table read, in file order.
    #[must_use]
    pub fn tables(&self) -> &[AceSetTable] {
        &self.tables
    }

    /// Files not read as ACE, with the reason.
    #[must_use]
    pub fn skipped(&self) -> &[(String, String)] {
        &self.skipped
    }

    fn one<'s>(
        &'s self,
        is_it: impl Fn(&AceSetTable) -> bool,
        temperature_k: f64,
    ) -> Result<&'s AceSetTable, AceLookupError> {
        let all: Vec<&AceSetTable> = self.tables.iter().filter(|t| is_it(t)).collect();
        if all.is_empty() {
            return Err(AceLookupError::Missing);
        }
        let at: Vec<&AceSetTable> = all
            .iter()
            .copied()
            .filter(|t| t.is_at(temperature_k))
            .collect();
        match at.as_slice() {
            [one] => Ok(one),
            [] => {
                let mut ts: Vec<f64> = all.iter().map(|t| t.temperature_k()).collect();
                ts.sort_by(f64::total_cmp);
                ts.dedup();
                Err(AceLookupError::WrongTemperature(ts))
            }
            many => Err(AceLookupError::Ambiguous(
                many.iter().map(|t| t.describe()).collect(),
            )),
        }
    }

    /// **The continuous-energy neutron table of `za` at `temperature_k`.**
    ///
    /// # Errors
    /// [`AceLookupError`]: none, only at other temperatures, or several.
    pub fn neutron(&self, za: u32, temperature_k: f64) -> Result<&AceSetTable, AceLookupError> {
        self.one(
            |t| t.class() == AceClass::ContinuousNeutron && t.za() == Some(za),
            temperature_k,
        )
    }

    /// **The 0 K table of `za`**, the companion DBRC takes its elastic from.
    ///
    /// # Errors
    /// [`AceLookupError::Missing`] when there is none (DBRC then stays off,
    /// and the nuclide says so); `Ambiguous` for several.
    pub fn zero_kelvin(&self, za: u32) -> Result<&AceSetTable, AceLookupError> {
        self.neutron(za, 0.0)
    }

    /// **The thermal (`t`) table whose ZAID name is `stem`** (`tsl1`,
    /// `hh2o`; case-insensitive) at `temperature_k`.
    ///
    /// # Errors
    /// [`AceLookupError`].
    pub fn thermal(&self, stem: &str, temperature_k: f64) -> Result<&AceSetTable, AceLookupError> {
        self.one(
            |t| t.class() == AceClass::Thermal && t.stem().eq_ignore_ascii_case(stem),
            temperature_k,
        )
    }
}

fn collect(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), NjoyError> {
    for e in std::fs::read_dir(dir).map_err(NjoyError::Io)?.flatten() {
        let p = e.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let r = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if p.is_dir() {
            collect(&p, &r, out)?;
        } else if p.is_file() {
            out.push((r, std::fs::read(&p).map_err(NjoyError::Io)?));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type1(zaid: &str, kt: &str) -> Vec<u8> {
        // A minimal Type-1 table: header, IZ/AW, NXS (length 1), JXS, XSS.
        let mut s = format!("{zaid:>10}{:>12}{kt:>12} 10/17/26\n", "1.000000");
        s.push_str(&format!("{:<70}{:>10}\n", "fixture", "mat 9999"));
        for _ in 0..4 {
            s.push_str(
                "      0         0.      0         0.      0         0.      0         0.\n",
            );
        }
        s.push_str("       1       0       0       0       0       0       0       0\n");
        s.push_str("       0       0       0       0       0       0       0       0\n");
        for _ in 0..4 {
            s.push_str("       0       0       0       0       0       0       0       0\n");
        }
        s.push_str(" 1.000000000000E+00\n");
        s.into_bytes()
    }

    /// Tables are found by header (ZAID, class, kT), never by file name; a
    /// temperature mismatch is refused with the temperatures that exist; the
    /// 0 K table is the companion; zips are opened; junk is skipped, never a
    /// panic.
    #[test]
    fn tables_are_found_by_header_and_temperature() {
        let zip =
            crate::archive::write_zip(&[("0K/u.ace".into(), type1("92235.00c", "0.0000E+00"))])
                .unwrap();
        let set = AceSet::from_files(vec![
            ("misnamed.bin".into(), type1("92235.00c", "2.5300E-08")),
            ("hot.ace".into(), type1("92235.01c", "5.1704E-08")),
            ("lib.zip".into(), zip),
            ("water.ace".into(), type1("tsl1.00t", "2.5300E-08")),
            ("notes.txt".into(), b"just some notes".to_vec()),
            ("broken.zip".into(), b"PK\x03\x04 truncated".to_vec()),
        ]);
        assert_eq!(set.tables().len(), 4, "skipped: {:?}", set.skipped());
        assert_eq!(set.skipped().len(), 2);
        let t = set.neutron(92235, 293.6).unwrap();
        assert_eq!(
            (t.file.as_str(), t.za(), t.stem()),
            ("misnamed.bin", Some(92235), "92235")
        );
        assert!((t.temperature_k() - 293.6).abs() < 0.05);
        assert!(t.describe().contains("misnamed.bin"));
        assert_eq!(set.neutron(92235, 600.0).unwrap().zaid(), "92235.01c");
        match set.neutron(92235, 900.0) {
            Err(AceLookupError::WrongTemperature(ts)) => assert_eq!(ts.len(), 3),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            set.neutron(92238, 293.6).err(),
            Some(AceLookupError::Missing)
        );
        let cold = set.zero_kelvin(92235).unwrap();
        assert!(cold.is_zero_kelvin() && cold.file == "lib.zip/0K/u.ace");
        assert_eq!(set.thermal("TSL1", 293.6).unwrap().za(), None);
        assert!(set.thermal("tsl1", 600.0).is_err());
        assert!(AceSet::from_folder(Path::new("/nonexistent-ace-dir")).is_err());
        assert!(AceLookupError::Missing.to_string().contains("no ACE"));
    }
}
