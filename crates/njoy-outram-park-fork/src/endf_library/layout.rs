// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **How a library's files are laid out**, read from their names. The
//! layout tells the loader which files to open: in an extracted library the
//! decay, fission-yield and photo-atomic sub-libraries hold thousands of
//! tapes transport never reads, and a zip member is only inflated when it is
//! worth reading. A file's *identity* still comes from its own header
//! ([`super::EndfHeader`]), never from its name.

use crate::acquire::EndfLibrary;

/// The folder structures this loader knows. Detected from the names alone
/// ([`LibraryLayout::detect`]); every layout reads the same tapes the same
/// way, so an unrecognised layout costs time (every file is opened), never a
/// wrong answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryLayout {
    /// Tapes directly in the folder, or any arrangement not listed below:
    /// every file is opened and kept if its header is an ENDF-6 tape
    /// (`reference-data/endf/`, a hand-made folder).
    Flat,
    /// One folder per sub-library named for it: `neutrons/`,
    /// `thermal_scatt/`, `decay/`, ... (the extracted-library layout
    /// `endf_folder` has scanned since gh:#581). Only `neutrons/` and
    /// `thermal_scatt/` (and loose top-level files) are opened.
    SubLibraryFolders,
    /// The **NNDC per-sub-library archives**: the folder inside
    /// `ENDF-B-VIII.0_neutrons.zip` is `ENDF-B-VIII.0_neutrons/`, holding
    /// `n-001_H_001.endf` …; the thermal one is
    /// `ENDF-B-VIII.0_thermal_scatt/`. Checked on 2026-10-17 against the
    /// maintainer's local copies of both ENDF/B-VIII.0 archives (561 and 91
    /// members). Only `*_neutrons/` and `*_thermal_scatt/` are opened.
    NndcSubLibraryArchives,
    /// The **IAEA NDS per-nuclide tree** this crate downloads from
    /// ([`crate::acquire`]): one zip per tape, `n/n_9228_92-U-235.zip`,
    /// `tsl/tsl-HinH2O.zip`. Each inner zip is opened and its tape read.
    IaeaPerNuclide,
}

/// Which part of a library a file sits in, by its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRole {
    /// An incident-neutron folder: open it.
    Neutrons,
    /// A thermal-scattering folder: open it.
    Thermal,
    /// Not in a named folder: open it and let the header decide.
    Unknown,
    /// A sub-library transport does not read (decay, photo-atomic, ...), or
    /// a file that is not data (a PDF, a changelog): not opened.
    Skip,
}

const SKIP_EXTENSIONS: [&str; 10] = [
    "pdf", "txt", "md", "html", "htm", "list", "py", "json", "png", "csv",
];

impl LibraryLayout {
    /// **The layout from the files' paths** (`/`-separated, relative to the
    /// folder or archive root).
    #[must_use]
    pub fn detect<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let mut sub = false;
        let mut nndc = false;
        let mut iaea = false;
        for p in paths {
            let parts: Vec<&str> = p.split('/').collect();
            let dirs = &parts[..parts.len().saturating_sub(1)];
            if dirs
                .iter()
                .any(|d| *d == "neutrons" || *d == "thermal_scatt")
            {
                sub = true;
            }
            if dirs
                .iter()
                .any(|d| d.ends_with("_neutrons") || d.ends_with("_thermal_scatt"))
            {
                nndc = true;
            }
            // A per-nuclide zip, as a file or (once opened) as a folder.
            if parts.iter().any(|c| is_iaea_neutron_zip(c)) {
                iaea = true;
            }
        }
        if nndc {
            Self::NndcSubLibraryArchives
        } else if sub {
            Self::SubLibraryFolders
        } else if iaea {
            Self::IaeaPerNuclide
        } else {
            Self::Flat
        }
    }

    /// Which part of the library the file at `path` is in, under this layout.
    #[must_use]
    pub fn role(self, path: &str) -> PathRole {
        let parts: Vec<&str> = path.split('/').collect();
        let file = parts.last().copied().unwrap_or("");
        if file.is_empty() || file.starts_with('.') {
            return PathRole::Skip;
        }
        let ext = file
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if SKIP_EXTENSIONS.contains(&ext.as_str()) || file.eq_ignore_ascii_case("CHANGELOG") {
            return PathRole::Skip;
        }
        let dirs = &parts[..parts.len() - 1];
        match self {
            Self::Flat => PathRole::Unknown,
            Self::SubLibraryFolders | Self::NndcSubLibraryArchives => {
                let named = |suffix: &str| {
                    dirs.iter()
                        .any(|d| *d == suffix || d.ends_with(&format!("_{suffix}")))
                };
                if named("neutrons") {
                    PathRole::Neutrons
                } else if named("thermal_scatt") {
                    PathRole::Thermal
                } else if dirs.is_empty() {
                    PathRole::Unknown
                } else {
                    PathRole::Skip
                }
            }
            Self::IaeaPerNuclide => {
                if dirs.iter().any(|d| *d == "n" || is_iaea_neutron_zip(d))
                    || is_iaea_neutron_zip(file)
                {
                    PathRole::Neutrons
                } else if dirs.contains(&"tsl") {
                    PathRole::Thermal
                } else {
                    PathRole::Unknown
                }
            }
        }
    }
}

/// `n_<MAT>_<Z>-<Sym>-<A>.zip` ([`crate::acquire::zip_filename`]'s form).
fn is_iaea_neutron_zip(file: &str) -> bool {
    let Some(stem) = file.strip_prefix("n_").and_then(|s| s.strip_suffix(".zip")) else {
        return false;
    };
    let mut it = stem.splitn(2, '_');
    let mat_ok = it
        .next()
        .is_some_and(|m| m.len() == 4 && m.bytes().all(|b| b.is_ascii_digit()));
    mat_ok && it.next().is_some_and(|rest| rest.split('-').count() == 3)
}

/// **The library the names point at**, when a folder or archive name carries
/// one (`ENDF-B-VIII.0_neutrons/`, `ENDF-B-VII.1/`): the [`EndfLibrary`]
/// whose IAEA directory name ([`EndfLibrary::dir`]) appears in a folder
/// name. `None` when no folder names one, or two disagree. This is what the
/// names say; the tapes' own headers say what they are.
#[must_use]
pub fn library_named_in<'a>(paths: impl IntoIterator<Item = &'a str>) -> Option<EndfLibrary> {
    let mut found: Option<EndfLibrary> = None;
    for p in paths {
        let parts: Vec<&str> = p.split('/').collect();
        for d in &parts[..parts.len().saturating_sub(1)] {
            // Longest name first, so `ENDF-B-VII.1` is not read as `ENDF-B-VII`.
            let mut libs = EndfLibrary::all().to_vec();
            libs.sort_by_key(|l| std::cmp::Reverse(l.dir().len()));
            if let Some(l) = libs.into_iter().find(|l| d.contains(l.dir())) {
                match found {
                    None => found = Some(l),
                    Some(f) if f == l => {}
                    Some(_) => return None,
                }
            }
        }
    }
    found
}
