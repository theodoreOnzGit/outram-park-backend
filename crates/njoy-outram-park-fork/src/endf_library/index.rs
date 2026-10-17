// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! The common index: one implementation for a folder, a zip, or files handed
//! over as bytes. See the module docs of [`super`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::header::{EndfHeader, SubLibrary};
use super::layout::{library_named_in, LibraryLayout, PathRole};
use crate::acquire::{symbol_for_z, EndfLibrary};
use crate::archive::ZipArchive;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// Bytes read from the start of each file to identify it: enough for the
/// TPID line and the seven MF=1/MT=451 records the header needs, with room
/// for a long TPID or CRLF line ends.
const PEEK_BYTES: usize = 4096;

/// Folder depth followed by [`EndfLibraryIndex::from_folder`]: a library
/// root, its sub-library folders, and one more level for a library folder
/// inside the chosen one.
const MAX_FOLDER_DEPTH: usize = 4;

/// Where an indexed file's bytes are.
#[derive(Debug, Clone)]
enum Item {
    File(PathBuf),
    Bytes(Arc<Vec<u8>>),
    Member(usize),
    Nested { archive: usize, member: usize },
}

/// What the library was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibrarySource {
    /// A folder on disk (native only).
    Folder(PathBuf),
    /// A zip archive given as bytes.
    Zip {
        /// Its size in bytes.
        bytes: usize,
    },
    /// Files given as bytes (a browser folder or multi-file upload).
    Files {
        /// How many.
        count: usize,
    },
}

/// One ENDF-6 tape in the index.
#[derive(Debug, Clone)]
pub struct IndexedTape {
    /// Its path in the folder or archive, `/`-separated (a member of a
    /// nested zip is `outer.zip/inner`).
    pub path: String,
    /// What its header says it is.
    pub header: EndfHeader,
    item: Item,
}

impl IndexedTape {
    /// The file name without its folder.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// A short name for a picker: `U-235`, `Am-242m1`, or the thermal law's
    /// `ZSYMAM` text (`H in H2O`).
    #[must_use]
    pub fn display_name(&self) -> String {
        let h = &self.header;
        match h.sub_library() {
            SubLibrary::IncidentNeutron => {
                let sym = symbol_for_z(h.z()).unwrap_or("?");
                let m = if h.liso > 0 {
                    format!("m{}", h.liso)
                } else {
                    String::new()
                };
                if h.a() == 0 {
                    format!("{sym}-nat")
                } else {
                    format!("{sym}-{}{m}", h.a())
                }
            }
            _ => {
                if h.symbol.is_empty() {
                    self.file_name().to_string()
                } else {
                    h.symbol.clone()
                }
            }
        }
    }
}

/// A file the index did not keep, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    /// Its path.
    pub path: String,
    /// Why.
    pub reason: String,
}

/// Why a lookup found no single tape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// No tape of the library is this nuclide or law. `elsewhere` lists
    /// tapes of it from **other** libraries in the same folder, which are not
    /// used without being asked for.
    Missing {
        /// `path (library)` of each tape of another library.
        elsewhere: Vec<String>,
    },
    /// Several tapes of the library are this nuclide or law.
    Ambiguous(Vec<String>),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { elsewhere } if elsewhere.is_empty() => write!(f, "no tape for it"),
            Self::Missing { elsewhere } => write!(
                f,
                "no tape of this library; other libraries have: {elsewhere:?}"
            ),
            Self::Ambiguous(v) => write!(f, "several tapes: {v:?}; remove all but one"),
        }
    }
}

/// **An ENDF library, indexed**: every ENDF-6 tape in a folder, a zip or a set
/// of uploaded files, identified by its own header. See the module docs of
/// [`super`].
#[derive(Debug, Clone)]
pub struct EndfLibraryIndex {
    source: LibrarySource,
    layout: LibraryLayout,
    named: Option<EndfLibrary>,
    archive: Option<ZipArchive>,
    nested: Vec<ZipArchive>,
    tapes: Vec<IndexedTape>,
    skipped: Vec<SkippedFile>,
}

fn gunzip_if(bytes: Vec<u8>, path: &str) -> Result<Vec<u8>, NjoyError> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        crate::acer::read::inflate_gzip(&bytes, Path::new(path))
    } else {
        Ok(bytes)
    }
}

impl EndfLibraryIndex {
    /// **A zip archive of a library** (or of any folder of tapes), given as
    /// bytes. Members that are themselves zips (the IAEA per-nuclide tree)
    /// are opened one level down.
    ///
    /// # Errors
    /// The bytes are not a readable zip.
    pub fn from_zip_bytes(bytes: Vec<u8>) -> Result<Self, NjoyError> {
        let n = bytes.len();
        let archive = ZipArchive::parse(Arc::new(bytes))?;
        let items = archive
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_dir())
            .map(|(i, e)| (e.name.clone(), Item::Member(i)))
            .collect();
        Self::build(LibrarySource::Zip { bytes: n }, Some(archive), items)
    }

    /// **Files given as bytes**, `(path, contents)`: a browser folder upload
    /// (paths relative to the chosen folder) or several dropped files. A file
    /// that is a zip is opened, so one dropped library zip works too.
    ///
    /// # Errors
    /// None today beyond those of [`Self::from_zip_bytes`] for a nested zip
    /// whose central directory is unreadable, which is skipped with a reason
    /// instead; kept as a `Result` so a future check can refuse.
    pub fn from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, NjoyError> {
        let count = files.len();
        let items = files
            .into_iter()
            .map(|(p, b)| (p.replace('\\', "/"), Item::Bytes(Arc::new(b))))
            .collect();
        Self::build(LibrarySource::Files { count }, None, items)
    }

    /// **A folder on disk** (native only: `std::fs` compiles for `wasm32`
    /// and fails at run time), followed down [`MAX_FOLDER_DEPTH`] levels.
    ///
    /// # Errors
    /// The folder cannot be read.
    pub fn from_folder(dir: &Path) -> Result<Self, NjoyError> {
        let mut items = Vec::new();
        walk(dir, "", 0, &mut items)?;
        items.sort_by(|a, b| a.0.cmp(&b.0));
        Self::build(LibrarySource::Folder(dir.to_path_buf()), None, items)
    }

    fn build(
        source: LibrarySource,
        archive: Option<ZipArchive>,
        items: Vec<(String, Item)>,
    ) -> Result<Self, NjoyError> {
        let mut me = Self {
            source,
            layout: LibraryLayout::Flat,
            named: None,
            archive,
            nested: Vec::new(),
            tapes: Vec::new(),
            skipped: Vec::new(),
        };
        // Open nested zips one level down.
        let mut flat: Vec<(String, Item)> = Vec::with_capacity(items.len());
        for (path, item) in items {
            if !path.to_ascii_lowercase().ends_with(".zip") {
                flat.push((path, item));
                continue;
            }
            let opened = me.item_arc(&item).and_then(ZipArchive::parse);
            match opened {
                Ok(z) => {
                    let k = me.nested.len();
                    for (j, e) in z.entries().iter().enumerate().filter(|(_, e)| !e.is_dir()) {
                        flat.push((
                            format!("{path}/{}", e.name),
                            Item::Nested {
                                archive: k,
                                member: j,
                            },
                        ));
                    }
                    me.nested.push(z);
                }
                Err(e) => me.skipped.push(SkippedFile {
                    path,
                    reason: format!("unreadable zip: {e}"),
                }),
            }
        }
        me.layout = LibraryLayout::detect(flat.iter().map(|(p, _)| p.as_str()));
        me.named = library_named_in(flat.iter().map(|(p, _)| p.as_str()));
        for (path, item) in flat {
            if me.layout.role(&path) == PathRole::Skip {
                me.skipped.push(SkippedFile {
                    path,
                    reason: "not a sub-library transport reads, or not a data file".into(),
                });
                continue;
            }
            let head = match me.prefix(&item, &path) {
                Ok(h) => h,
                Err(e) => {
                    me.skipped.push(SkippedFile {
                        path,
                        reason: format!("unreadable: {e}"),
                    });
                    continue;
                }
            };
            let text = String::from_utf8_lossy(&head);
            match EndfHeader::parse(&text) {
                None => me.skipped.push(SkippedFile {
                    path,
                    reason: "not an ENDF tape (no readable MF=1/MT=451 header)".into(),
                }),
                Some(h) if !h.is_endf6() => me.skipped.push(SkippedFile {
                    path,
                    reason: format!(
                        "NFOR = {:?}: not ENDF-6, which is the format this crate processes",
                        h.nfor
                    ),
                }),
                Some(header) => me.tapes.push(IndexedTape { path, header, item }),
            }
        }
        Ok(me)
    }

    fn item_arc(&self, item: &Item) -> Result<Arc<Vec<u8>>, NjoyError> {
        match item {
            Item::Bytes(b) => Ok(b.clone()),
            other => self.read_item(other).map(Arc::new),
        }
    }

    fn read_item(&self, item: &Item) -> Result<Vec<u8>, NjoyError> {
        match item {
            Item::File(p) => std::fs::read(p).map_err(NjoyError::Io),
            Item::Bytes(b) => Ok(b.as_ref().clone()),
            Item::Member(i) => self
                .archive
                .as_ref()
                .ok_or_else(|| NjoyError::InvalidInput("no archive for a member".into()))?
                .read(*i),
            Item::Nested { archive, member } => self
                .nested
                .get(*archive)
                .ok_or_else(|| NjoyError::InvalidInput("no nested archive".into()))?
                .read(*member),
        }
    }

    fn prefix(&self, item: &Item, path: &str) -> Result<Vec<u8>, NjoyError> {
        let raw = match item {
            Item::File(p) => {
                use std::io::Read;
                let mut buf = Vec::with_capacity(PEEK_BYTES);
                std::fs::File::open(p)
                    .and_then(|f| f.take(PEEK_BYTES as u64).read_to_end(&mut buf))
                    .map_err(NjoyError::Io)?;
                buf
            }
            Item::Bytes(b) => b[..b.len().min(PEEK_BYTES)].to_vec(),
            Item::Member(i) => self
                .archive
                .as_ref()
                .map_or(Ok(Vec::new()), |a| a.read_prefix(*i, PEEK_BYTES))?,
            Item::Nested { archive, member } => self
                .nested
                .get(*archive)
                .map_or(Ok(Vec::new()), |a| a.read_prefix(*member, PEEK_BYTES))?,
        };
        if raw.starts_with(&[0x1f, 0x8b]) {
            // A gzipped tape: its header needs the stream from the start.
            let mut all = gunzip_if(self.read_item(item)?, path)?;
            all.truncate(PEEK_BYTES);
            return Ok(all);
        }
        Ok(raw)
    }

    /// What the library was read from.
    #[must_use]
    pub fn source(&self) -> &LibrarySource {
        &self.source
    }

    /// The layout read from the names.
    #[must_use]
    pub fn layout(&self) -> LibraryLayout {
        self.layout
    }

    /// The library a folder or archive name points at (`ENDF-B-VIII.0_neutrons`),
    /// if any. The tapes' headers decide what is used ([`Self::main_library`]).
    #[must_use]
    pub fn named_library(&self) -> Option<EndfLibrary> {
        self.named
    }

    /// Every indexed ENDF-6 tape, in path order.
    #[must_use]
    pub fn tapes(&self) -> &[IndexedTape] {
        &self.tapes
    }

    /// Every file not kept, with the reason.
    #[must_use]
    pub fn skipped(&self) -> &[SkippedFile] {
        &self.skipped
    }

    /// **The library most incident-neutron tapes belong to**, by their
    /// headers ([`EndfHeader::library`]); ties go to the name that sorts
    /// first, so the choice is the same on every run. `None` when there is
    /// no incident-neutron tape. In a folder holding one library this is that
    /// library; in a mixed folder (the workspace's `reference-data/endf`) it
    /// is the one the folder is mostly made of, and tapes of the others are
    /// reported, not used ([`LookupError::Missing`]).
    #[must_use]
    pub fn main_library(&self) -> Option<String> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for t in self
            .tapes
            .iter()
            .filter(|t| t.header.sub_library() == SubLibrary::IncidentNeutron)
        {
            let l = t.header.library();
            match counts.iter_mut().find(|(n, _)| *n == l) {
                Some((_, c)) => *c += 1,
                None => counts.push((l, 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        counts.into_iter().next().map(|(l, _)| l)
    }

    /// The tapes of one library and sub-library.
    pub fn of(&self, library: &str, sub: SubLibrary) -> impl Iterator<Item = &IndexedTape> {
        let library = library.to_string();
        self.tapes
            .iter()
            .filter(move |t| t.header.sub_library() == sub && t.header.library() == library)
    }

    fn pick<'s>(
        &'s self,
        library: &str,
        sub: SubLibrary,
        is_it: impl Fn(&EndfHeader) -> bool,
    ) -> Result<&'s IndexedTape, LookupError> {
        let all: Vec<&IndexedTape> = self
            .tapes
            .iter()
            .filter(|t| t.header.sub_library() == sub && is_it(&t.header))
            .collect();
        let (mine, others): (Vec<&IndexedTape>, Vec<&IndexedTape>) =
            all.into_iter().partition(|t| t.header.library() == library);
        match mine.as_slice() {
            [one] => Ok(one),
            [] => Err(LookupError::Missing {
                elsewhere: others
                    .iter()
                    .map(|t| format!("{} ({})", t.path, t.header.library()))
                    .collect(),
            }),
            many => Err(LookupError::Ambiguous(
                many.iter().map(|t| t.path.clone()).collect(),
            )),
        }
    }

    /// **The incident-neutron tape of nuclide `(z, a, liso)` in `library`**
    /// (a [`Self::main_library`] string), by header identity.
    ///
    /// # Errors
    /// [`LookupError`].
    pub fn incident_neutron(
        &self,
        library: &str,
        z: u32,
        a: u32,
        liso: u8,
    ) -> Result<&IndexedTape, LookupError> {
        self.pick(library, SubLibrary::IncidentNeutron, |h| {
            h.za == 1000 * z + a && h.liso == liso
        })
    }

    /// **The thermal-scattering tape with MAT `mat` in `library`.**
    ///
    /// # Errors
    /// [`LookupError`].
    pub fn thermal(&self, library: &str, mat: i32) -> Result<&IndexedTape, LookupError> {
        self.pick(library, SubLibrary::ThermalScattering, |h| {
            h.mat == Some(mat)
        })
    }

    /// **A tape's bytes** (gzip unwrapped), as the folder, archive or upload
    /// holds them.
    ///
    /// # Errors
    /// The file cannot be read, or the zip member fails its checks.
    pub fn read_bytes(&self, tape: &IndexedTape) -> Result<Vec<u8>, NjoyError> {
        gunzip_if(self.read_item(&tape.item)?, &tape.path)
    }

    /// **A tape, parsed.**
    ///
    /// # Errors
    /// As [`Self::read_bytes`], and a tape that does not parse.
    pub fn read_tape(&self, tape: &IndexedTape) -> Result<Tape, NjoyError> {
        Tape::read(std::io::Cursor::new(self.read_bytes(tape)?))
    }

    /// **Fuzzy-find tapes for a picker**: incident-neutron and thermal tapes
    /// of `library`, best match first ([`super::fuzzy_score`] over the display
    /// name, `U235`-style and `92235`-style keys, and the file name). An
    /// empty query lists them all in (Z, A, LISO) order, laws after nuclides.
    #[must_use]
    pub fn search(&self, library: &str, query: &str) -> Vec<&IndexedTape> {
        let mut pool: Vec<&IndexedTape> = self
            .tapes
            .iter()
            .filter(|t| {
                t.header.library() == library
                    && matches!(
                        t.header.sub_library(),
                        SubLibrary::IncidentNeutron | SubLibrary::ThermalScattering
                    )
            })
            .collect();
        pool.sort_by_key(|t| {
            (
                t.header.sub_library() != SubLibrary::IncidentNeutron,
                t.header.za,
                t.header.liso,
                t.path.clone(),
            )
        });
        if query.trim().is_empty() {
            return pool;
        }
        let mut scored: Vec<(i32, usize, &IndexedTape)> = pool
            .into_iter()
            .enumerate()
            .filter_map(|(order, t)| {
                let name = t.display_name();
                let keys = [
                    name.replace('-', ""),
                    name.clone(),
                    t.header.za.to_string(),
                    t.file_name().to_string(),
                ];
                keys.iter()
                    .filter_map(|k| super::fuzzy_score(query, k))
                    .max()
                    .map(|s| (s, order, t))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, t)| t).collect()
    }

    /// **A one-paragraph account of what was read**, for a run's provenance:
    /// the source, layout, named and main library, and counts.
    #[must_use]
    pub fn summary(&self) -> String {
        let src = match &self.source {
            LibrarySource::Folder(p) => format!("folder {}", p.display()),
            LibrarySource::Zip { bytes } => format!("zip of {bytes} bytes"),
            LibrarySource::Files { count } => format!("{count} uploaded files"),
        };
        let main = self.main_library().unwrap_or_else(|| "none".into());
        let n_main = self.of(&main, SubLibrary::IncidentNeutron).count();
        let n_neutron = self
            .tapes
            .iter()
            .filter(|t| t.header.sub_library() == SubLibrary::IncidentNeutron)
            .count();
        let n_thermal = self.of(&main, SubLibrary::ThermalScattering).count();
        format!(
            "{main} from {src}; layout {:?}; named in paths: {}; {n_main} incident-neutron tapes of {main} \
             ({} of other libraries, not used), {n_thermal} thermal; {} files skipped",
            self.layout,
            self.named.map_or("none", |l| l.label()),
            n_neutron - n_main,
            self.skipped.len()
        )
    }
}

fn walk(
    dir: &Path,
    rel: &str,
    depth: usize,
    out: &mut Vec<(String, Item)>,
) -> Result<(), NjoyError> {
    let rd = std::fs::read_dir(dir).map_err(NjoyError::Io)?;
    for e in rd.flatten() {
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
            if depth + 1 < MAX_FOLDER_DEPTH {
                walk(&p, &r, depth + 1, out)?;
            }
        } else if p.is_file() {
            out.push((r, Item::File(p)));
        }
    }
    Ok(())
}
