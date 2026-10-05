//! # A folder of ENDF tapes: what is in it, and finding a tape in it
//!
//! A user can hand a program the ENDF data two ways: a **flat** folder of tapes
//! (`reference-data/endf/`, or a library's `neutrons/` folder picked on its
//! own), or a freshly **extracted library** such as the official
//! ENDF/B-VIII.0 archive, whose tapes sit in sub-library folders
//! (`neutrons/`, `thermal_scatt/`, `decay/`, ...). This module lists either
//! ([`scan_endf_folder`]) and finds a wanted tape in the listing by its
//! identity ([`TapeIdentity::find`]), so that a loader can read the same
//! tapes from the folder the user chose rather than from a fixed directory.
//!
//! Moved here on 2026-10-05 (gh:#581) from the Dhoby Ghaut workbench's Step 0
//! scan, so the scan the user sees and the nuclear-data load that follows use
//! one implementation (nuclear-data code belongs in this crate).
//!
//! # Identity, not file name
//!
//! Libraries name the same evaluation differently: this repository's
//! `n-092_U_235-ENDF8.0.endf` is `neutrons/n-092_U_235.endf` in the extracted
//! ENDF/B-VIII.0 archive. A tape is identified by the **MAT number** and the
//! **sub-library number `NSUB`** of its MF1/MT451 header (ENDF-102 § 1.1:
//! MAT on every record in columns 67-70; NSUB in the third record of
//! MF1/MT451, which is the fourth line of the tape counting the TPID line).
//! The file name is used only to break ties (two evaluations of one MAT in
//! one folder, such as this repository's ENDF/B-VII.0 and VIII.0 U-235) and
//! as the fallback when a header cannot be read.
//!
//! # What this does not check
//!
//! Only the header is read. A tape with the right MAT and NSUB from another
//! library or version (a JEFF U-235 in the folder) is found as U-235; the
//! library is whatever the user put in the folder.

use std::path::{Path, PathBuf};

/// The sub-library folders transport reads, in an extracted ENDF library
/// (incident neutrons and thermal scattering).
pub const LIBRARY_SUBDIRS: [&str; 2] = ["neutrons", "thermal_scatt"];

/// One tape found in a folder, with what its header says.
#[derive(Clone, Debug, PartialEq)]
pub struct TapeHeader {
    /// Path relative to the scanned folder, `/`-separated
    /// (`neutrons/n-092_U_235.endf`, or just the file name in a flat folder).
    pub file: String,
    /// MAT number (second line, columns 67-70); `None` if unreadable.
    pub mat: Option<i32>,
    /// `ZSYMAM` (MF1/MT451 sixth line, columns 1-11), e.g. ` 92-U -235 `.
    pub symbol: String,
    /// Sub-library number `NSUB` (fourth line, columns 45-55): 10 incident
    /// neutrons, 12 thermal scattering, ...; `None` if unreadable.
    pub nsub: Option<i64>,
}

impl TapeHeader {
    /// Read the header of the tape at `path`, recording it under the name
    /// `file`. Never fails: an unreadable field is `None` (or empty).
    #[must_use]
    pub fn read(path: &Path, file: String) -> Self {
        use std::io::{BufRead, BufReader};
        let mut info = Self {
            file,
            mat: None,
            symbol: String::new(),
            nsub: None,
        };
        let Ok(f) = std::fs::File::open(path) else {
            return info;
        };
        let lines: Vec<String> = BufReader::new(f)
            .lines()
            .take(6)
            .map_while(Result::ok)
            .collect();
        let field = |l: &str, a: usize, b: usize| {
            l.get(a..b.min(l.len()))
                .map(str::trim)
                .unwrap_or("")
                .to_string()
        };
        if let Some(l) = lines.get(1) {
            info.mat = field(l, 66, 70).parse().ok();
        }
        if let Some(l) = lines.get(3) {
            info.nsub = field(l, 44, 55).parse().ok();
        }
        if let Some(l) = lines.get(5) {
            info.symbol = field(l, 0, 11);
        }
        info
    }

    /// What the sub-library number means, in words.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self.nsub {
            Some(10) => "neutron",
            Some(12) => "thermal scattering",
            Some(20_040) => "alpha",
            Some(0) => "photo-nuclear",
            Some(_) => "other",
            None => "unreadable header",
        }
    }

    /// The file name without its folder.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(&self.file)
    }
}

/// How a scanned folder is laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndfFolderLayout {
    /// Tapes directly in the folder.
    Flat,
    /// An extracted library with sub-library folders. `scanned` are the ones
    /// read ([`LIBRARY_SUBDIRS`] that exist); `others` are listed only.
    Library {
        /// Sub-library folders whose tapes were read.
        scanned: Vec<String>,
        /// Other sub-folders, not read.
        others: Vec<String>,
    },
}

/// Every `*.endf` (and `*.dat`) tape `dir` holds, with its header, sorted by
/// name. A folder with a `neutrons/` or `thermal_scatt/` sub-folder is read as
/// an extracted library (those two sub-folders, plus any tapes at the top
/// level); any other folder as flat. An unreadable folder gives no tapes.
#[must_use]
pub fn scan_endf_folder(dir: &Path) -> (EndfFolderLayout, Vec<TapeHeader>) {
    let subdirs: Vec<String> = std::fs::read_dir(dir)
        .map(|es| {
            es.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let is_library = LIBRARY_SUBDIRS.iter().any(|s| subdirs.iter().any(|d| d == s));
    if !is_library {
        return (EndfFolderLayout::Flat, scan_flat(dir, ""));
    }
    let mut scanned = Vec::new();
    let mut tapes = Vec::new();
    for s in LIBRARY_SUBDIRS {
        if subdirs.iter().any(|d| d == s) {
            scanned.push(s.to_string());
            tapes.extend(scan_flat(&dir.join(s), &format!("{s}/")));
        }
    }
    let mut others: Vec<String> = subdirs
        .into_iter()
        .filter(|d| !LIBRARY_SUBDIRS.contains(&d.as_str()))
        .collect();
    others.sort();
    tapes.extend(scan_flat(dir, ""));
    tapes.sort_by(|a, b| a.file.cmp(&b.file));
    (EndfFolderLayout::Library { scanned, others }, tapes)
}

fn scan_flat(dir: &Path, prefix: &str) -> Vec<TapeHeader> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        let is_tape = p.is_file()
            && p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "endf" || x == "dat");
        if !is_tape {
            continue;
        }
        let file = format!(
            "{prefix}{}",
            p.file_name().and_then(|f| f.to_str()).unwrap_or_default()
        );
        out.push(TapeHeader::read(&p, file));
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

/// What a wanted tape is: its file name in the program's own copy, and the
/// MAT and NSUB read from that copy's header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapeIdentity {
    /// File name (no folder).
    pub file: String,
    /// MAT number, when the reference copy's header was readable.
    pub mat: Option<i32>,
    /// Sub-library number, likewise.
    pub nsub: Option<i64>,
}

/// Why [`TapeIdentity::find`] found no single tape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindTapeError {
    /// Nothing in the folder is this tape.
    Missing,
    /// Several tapes have this MAT and NSUB and none has the wanted file
    /// name; their names, so the user can remove all but one.
    Ambiguous(Vec<String>),
}

impl TapeIdentity {
    /// The identity of the tape at `path` (its file name and its header's
    /// MAT and NSUB). A missing or unreadable file gives the name alone.
    #[must_use]
    pub fn of_tape(path: &Path) -> Self {
        let file = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or_default()
            .to_string();
        let h = TapeHeader::read(path, file.clone());
        Self {
            file,
            mat: h.mat,
            nsub: h.nsub,
        }
    }

    /// Whether scanned tape `t` is this tape: same MAT and NSUB when both
    /// headers were read, else the same file name.
    #[must_use]
    pub fn matches(&self, t: &TapeHeader) -> bool {
        match (self.mat, self.nsub, t.mat, t.nsub) {
            (Some(m), Some(s), Some(tm), Some(ts)) => m == tm && s == ts,
            _ => t.file_name() == self.file,
        }
    }

    /// The one tape in `tapes` that is this tape. Among several matches, the
    /// one with the wanted file name wins.
    ///
    /// # Errors
    /// [`FindTapeError::Missing`] when none matches,
    /// [`FindTapeError::Ambiguous`] when several match and none has the name.
    pub fn find<'a>(&self, tapes: &'a [TapeHeader]) -> Result<&'a TapeHeader, FindTapeError> {
        let hits: Vec<&TapeHeader> = tapes.iter().filter(|t| self.matches(t)).collect();
        match hits.as_slice() {
            [] => Err(FindTapeError::Missing),
            [one] => Ok(one),
            many => many
                .iter()
                .find(|t| t.file_name() == self.file)
                .copied()
                .ok_or_else(|| FindTapeError::Ambiguous(many.iter().map(|t| t.file.clone()).collect())),
        }
    }
}

/// A scanned folder, kept so many tapes can be looked up in it.
#[derive(Clone, Debug, PartialEq)]
pub struct EndfFolder {
    /// The folder.
    pub dir: PathBuf,
    /// How it is laid out.
    pub layout: EndfFolderLayout,
    /// Its tapes.
    pub tapes: Vec<TapeHeader>,
}

impl EndfFolder {
    /// Scan `dir` ([`scan_endf_folder`]).
    #[must_use]
    pub fn scan(dir: &Path) -> Self {
        let (layout, tapes) = scan_endf_folder(dir);
        Self {
            dir: dir.to_path_buf(),
            layout,
            tapes,
        }
    }

    /// Where the tape identified as `want` is in this folder.
    ///
    /// # Errors
    /// As [`TapeIdentity::find`].
    pub fn locate(&self, want: &TapeIdentity) -> Result<PathBuf, FindTapeError> {
        want.find(&self.tapes).map(|t| self.dir.join(&t.file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_endf() -> PathBuf {
        crate::reference_data::reference_endf_dir()
    }

    /// The scan reads the reference library's own headers: U-235 is MAT 9228,
    /// an incident-neutron tape.
    #[test]
    fn the_scan_reads_mat_and_sublibrary_from_the_reference_tapes() {
        let (layout, tapes) = scan_endf_folder(&reference_endf());
        assert_eq!(layout, EndfFolderLayout::Flat);
        let u235 = tapes
            .iter()
            .find(|t| t.file == "n-092_U_235-ENDF8.0.endf")
            .expect("U-235 tape");
        assert_eq!(u235.mat, Some(9228));
        assert_eq!(u235.nsub, Some(10));
        assert_eq!(u235.kind(), "neutron");
        assert!(u235.symbol.contains("235"), "{:?}", u235.symbol);
    }

    /// In the reference folder two U-235 evaluations share MAT 9228; the
    /// wanted file name breaks the tie. A tape whose name is absent and whose
    /// MAT is shared is reported as ambiguous, never picked at random.
    #[test]
    fn a_shared_mat_is_resolved_by_name_or_refused() {
        let dir = reference_endf();
        let folder = EndfFolder::scan(&dir);
        let want = TapeIdentity::of_tape(&dir.join("n-092_U_235-ENDF8.0.endf"));
        assert_eq!(want.mat, Some(9228));
        assert_eq!(folder.locate(&want), Ok(dir.join("n-092_U_235-ENDF8.0.endf")));
        let renamed = TapeIdentity {
            file: "n-092_U_235.endf".into(),
            ..want
        };
        assert!(matches!(folder.locate(&renamed), Err(FindTapeError::Ambiguous(v)) if v.len() >= 2));
    }

    /// An extracted library (sub-library folders, the library's own file
    /// names) is recognised, and a tape is found in it by MAT and NSUB.
    /// Built from copies of reference tapes renamed the way the official
    /// ENDF/B-VIII.0 archive names them.
    #[test]
    fn an_extracted_library_is_scanned_and_matched_by_identity() {
        let src = reference_endf();
        let root = std::env::temp_dir().join(format!("njoy_endf_folder_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("neutrons")).expect("dir");
        std::fs::create_dir_all(root.join("thermal_scatt")).expect("dir");
        std::fs::create_dir_all(root.join("decay")).expect("dir");
        for (from, to) in [
            ("n-092_U_235-ENDF8.0.endf", "neutrons/n-092_U_235.endf"),
            ("n-008_O_016-ENDF8.0.endf", "neutrons/n-008_O_016.endf"),
            ("tsl-crystalline-graphite.endf", "thermal_scatt/tsl-crystalline-graphite.endf"),
        ] {
            std::fs::copy(src.join(from), root.join(to)).expect("copy");
        }
        let folder = EndfFolder::scan(&root);
        assert_eq!(
            folder.layout,
            EndfFolderLayout::Library {
                scanned: vec!["neutrons".into(), "thermal_scatt".into()],
                others: vec!["decay".into()]
            }
        );
        let u235 = TapeIdentity::of_tape(&src.join("n-092_U_235-ENDF8.0.endf"));
        assert_eq!(folder.locate(&u235), Ok(root.join("neutrons/n-092_U_235.endf")));
        let gr = TapeIdentity::of_tape(&src.join("tsl-crystalline-graphite.endf"));
        assert_eq!(gr.nsub, Some(12));
        assert_eq!(folder.locate(&gr), Ok(root.join("thermal_scatt/tsl-crystalline-graphite.endf")));
        let u238 = TapeIdentity::of_tape(&src.join("n-092_U_238.endf"));
        assert_eq!(folder.locate(&u238), Err(FindTapeError::Missing));
        let _ = std::fs::remove_dir_all(&root);
    }
}
