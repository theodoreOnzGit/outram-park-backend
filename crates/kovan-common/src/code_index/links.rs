//! The per-crate **compact link index**, `<crate>/kovan_links.json` (#745,
//! #767): every identifier occurrence in the crate's source that links to a
//! definition, so web-kovan and desktop kovan can offer go-to-definition
//! and find-references **without rust-analyzer** (maintainer, #767). It is
//! small enough to ship in the crates.io package (#739 D4).
//!
//! # Format (schema 1)
//!
//! ```json
//! {
//!   "schema": 1,
//!   "kind": "kovan_links",
//!   "crate": "tampines",
//!   "dir": "crates/tampines",
//!   "generator": "rust-analyzer 1.98.0",
//!   "encoding": "utf8",
//!   "files": [ { "path": "src/lib.rs", "hash": "sha256:…", "occ": [0, 4, 5, 2,  1, 8, 3, 0] } ],
//!   "ext":   [ "crates/other/src/x.rs" ],
//!   "defs":  [ 0, 3, 7,   1, 10, 4 ]
//! }
//! ```
//!
//! - `files`: the crate's own source files, crate-relative, sorted. `hash`
//!   is `sha256:` of the file text the occurrences were taken from: a
//!   reader compares it with the file it has and shows "links out of date"
//!   for an edited file (the rust-analyzer-free refresh never rewrites
//!   this index, [`super::refresh`]).
//! - `occ`: the file's linking occurrences as a flat list of quadruples
//!   `[line delta, column, length, definition]`, sorted by position: the
//!   line is 0-based and delta-coded from the previous occurrence's (the
//!   first from 0), the column and length are in `encoding` units (SCIP's:
//!   UTF-8 bytes for rust-analyzer), and `definition` indexes `defs`.
//!   Occurrences that **are** definitions are not listed (the definition
//!   site is in `defs`), nor references to std or dependencies (no site in
//!   the workspace). Local variables are included.
//! - `ext`: workspace-relative files outside the crate that hold a
//!   definition referenced here, sorted.
//! - `defs`: the definition sites, a flat list of triples `[file, line,
//!   column]`, sorted; `file < files.len()` is `files[file]`, otherwise
//!   `ext[file - files.len()]`.
//!
//! **References** to a definition are the occurrences whose `definition`
//! points at it: all of them within this crate; from other crates, in their
//! own link index (a reader loads those it needs).
//!
//! A symbol with several definitions (rust-analyzer gives a library item and
//! an example's same-path copy one symbol) links to the one nearest the
//! reference (`kovan::scip::ScipIndex::nearest_definition`). JSON was chosen
//! over a binary layout so web-kovan reads it with `serde_json` and a diff
//! stays readable; integers only, so it compresses well (the crates.io
//! package is gzipped).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::review::hash::sha256_tagged;

/// The file name, at the crate's root folder.
pub const LINKS_FILE: &str = "kovan_links.json";
/// The schema written.
pub const LINKS_SCHEMA: u32 = 1;
/// `kind`.
pub const LINKS_KIND: &str = "kovan_links";

/// One own file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkFile {
    /// Crate-relative.
    pub path: String,
    /// `sha256:` of the text.
    pub hash: String,
    /// Flat `[line delta, column, length, definition]` quadruples.
    pub occ: Vec<u32>,
}

/// One crate's link index (module doc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkIndex {
    pub schema: u32,
    pub kind: String,
    #[serde(rename = "crate")]
    pub krate: String,
    pub dir: String,
    pub generator: String,
    pub encoding: String,
    pub files: Vec<LinkFile>,
    #[serde(default)]
    pub ext: Vec<String>,
    pub defs: Vec<u32>,
}

/// A position: workspace-relative file, 0-based line and column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Site {
    pub path: String,
    pub line: u32,
    pub col: u32,
}

/// One occurrence handed to [`build`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Occ {
    pub line: u32,
    pub col: u32,
    pub len: u32,
    pub def: Site,
}

/// One own file handed to [`build`]: workspace-relative path, its text, its
/// occurrences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOccs {
    pub path: String,
    pub text: String,
    pub occs: Vec<Occ>,
}

/// Why a link index could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinksError {
    Json(String),
    NotLinks(String),
    NewerSchema(u32),
    /// An index points outside its tables.
    OutOfRange(&'static str),
}

impl std::fmt::Display for LinksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "{LINKS_FILE}: {e}"),
            Self::NotLinks(k) => write!(f, "{LINKS_FILE}: kind {k:?}, not {LINKS_KIND:?}"),
            Self::NewerSchema(v) => write!(f, "{LINKS_FILE}: schema {v} is newer than {LINKS_SCHEMA}"),
            Self::OutOfRange(what) => write!(f, "{LINKS_FILE}: {what} index out of range"),
        }
    }
}

impl std::error::Error for LinksError {}

fn rel<'p>(dir: &str, path: &'p str) -> &'p str {
    if dir.is_empty() {
        path
    } else {
        path.strip_prefix(dir).and_then(|p| p.strip_prefix('/')).unwrap_or(path)
    }
}

/// Build crate `krate`'s index (folder `dir`, workspace-relative) from its
/// files' occurrences. Deterministic: everything is sorted, in any input
/// order.
pub fn build(krate: &str, dir: &str, generator: &str, encoding: &str, files: &[FileOccs]) -> LinkIndex {
    let mut own: Vec<&FileOccs> = files.iter().collect();
    own.sort_by(|a, b| a.path.cmp(&b.path));
    let own_paths: BTreeMap<&str, u32> = own
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.as_str(), i as u32))
        .collect();
    let ext: Vec<String> = own
        .iter()
        .flat_map(|f| f.occs.iter().map(|o| o.def.path.as_str()))
        .filter(|p| !own_paths.contains_key(p))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect();
    let file_no = |p: &str| -> u32 {
        own_paths.get(p).copied().unwrap_or_else(|| {
            own.len() as u32 + ext.binary_search_by(|e| e.as_str().cmp(p)).unwrap_or(0) as u32
        })
    };
    let sites: BTreeSet<(u32, u32, u32)> = own
        .iter()
        .flat_map(|f| f.occs.iter().map(|o| (file_no(&o.def.path), o.def.line, o.def.col)))
        .collect();
    let def_no: BTreeMap<(u32, u32, u32), u32> =
        sites.iter().enumerate().map(|(i, s)| (*s, i as u32)).collect();
    let mut out_files = Vec::new();
    for f in &own {
        let mut occs = f.occs.clone();
        occs.sort();
        occs.dedup_by(|a, b| a.line == b.line && a.col == b.col);
        let mut flat = Vec::with_capacity(occs.len() * 4);
        let mut prev = 0u32;
        for o in &occs {
            flat.extend([
                o.line - prev,
                o.col,
                o.len,
                def_no[&(file_no(&o.def.path), o.def.line, o.def.col)],
            ]);
            prev = o.line;
        }
        out_files.push(LinkFile {
            path: rel(dir, &f.path).to_string(),
            hash: text_hash(&f.text),
            occ: flat,
        });
    }
    LinkIndex {
        schema: LINKS_SCHEMA,
        kind: LINKS_KIND.to_string(),
        krate: krate.to_string(),
        dir: dir.to_string(),
        generator: generator.to_string(),
        encoding: encoding.to_string(),
        files: out_files,
        ext,
        defs: sites.into_iter().flat_map(|(f, l, c)| [f, l, c]).collect(),
    }
}

impl LinkIndex {
    /// The JSON text (compact, one trailing newline).
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap_or_default();
        s.push('\n');
        s
    }

    /// Read and check an index.
    pub fn parse(text: &str) -> Result<LinkIndex, LinksError> {
        let ix: LinkIndex = serde_json::from_str(text).map_err(|e| LinksError::Json(e.to_string()))?;
        if ix.kind != LINKS_KIND {
            return Err(LinksError::NotLinks(ix.kind));
        }
        if ix.schema > LINKS_SCHEMA {
            return Err(LinksError::NewerSchema(ix.schema));
        }
        let nfiles = (ix.files.len() + ix.ext.len()) as u32;
        if !ix.defs.len().is_multiple_of(3) || ix.defs.chunks(3).any(|d| d[0] >= nfiles) {
            return Err(LinksError::OutOfRange("defs file"));
        }
        let ndefs = (ix.defs.len() / 3) as u32;
        for f in &ix.files {
            if !f.occ.len().is_multiple_of(4) || f.occ.chunks(4).any(|o| o[3] >= ndefs) {
                return Err(LinksError::OutOfRange("occ definition"));
            }
        }
        Ok(ix)
    }

    fn path_of(&self, file: u32) -> String {
        let n = self.files.len() as u32;
        if file < n {
            let p = &self.files[file as usize].path;
            if self.dir.is_empty() {
                p.clone()
            } else {
                format!("{}/{p}", self.dir)
            }
        } else {
            self.ext[(file - n) as usize].clone()
        }
    }

    fn def(&self, k: u32) -> Site {
        let d = &self.defs[(k * 3) as usize..(k * 3 + 3) as usize];
        Site {
            path: self.path_of(d[0]),
            line: d[1],
            col: d[2],
        }
    }

    /// The occurrences of own file `path` (crate-relative), decoded:
    /// (line, column, length, definition index).
    pub fn occurrences(&self, path: &str) -> Vec<(u32, u32, u32, u32)> {
        let Some(f) = self.files.iter().find(|f| f.path == path) else {
            return Vec::new();
        };
        let mut line = 0;
        f.occ
            .chunks(4)
            .map(|o| {
                line += o[0];
                (line, o[1], o[2], o[3])
            })
            .collect()
    }

    /// Go to definition: the definition of the identifier covering
    /// (`line`, `col`) in own file `path` (crate-relative).
    pub fn definition_at(&self, path: &str, line: u32, col: u32) -> Option<Site> {
        self.occurrences(path)
            .into_iter()
            .find(|&(l, c, n, _)| l == line && c <= col && col < c + n.max(1))
            .map(|(_, _, _, d)| self.def(d))
    }

    /// Find references: every occurrence in this crate linking to `site`.
    pub fn references(&self, site: &Site) -> Vec<Site> {
        let Some(k) = (0..(self.defs.len() / 3) as u32).find(|&k| &self.def(k) == site) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for f in &self.files {
            for (l, c, _, d) in self.occurrences(&f.path) {
                if d == k {
                    out.push(Site {
                        path: self.path_of(self.files.iter().position(|g| g.path == f.path).unwrap_or(0) as u32),
                        line: l,
                        col: c,
                    });
                }
            }
        }
        out
    }

    /// Whether own file `path`'s links were taken from `text` (else they
    /// are out of date: shown, never silently trusted).
    pub fn is_current(&self, path: &str, text: &str) -> bool {
        self.files
            .iter()
            .any(|f| f.path == path && f.hash == text_hash(text))
    }
}

/// The `hash` of a source file's text: `sha256:` of it with every
/// carriage-return-line-feed pair read as a line feed.
///
/// A Windows checkout with `core.autocrlf = true` holds the same committed
/// file with CRLF line endings (GitHub #820, seen 2026-10-10). Hashing the
/// bytes as they are would make every file "links out of date" on the other
/// system. Positions are unaffected: lines and columns do not count the
/// carriage return. A file with no CRLF hashes as before, so indexes
/// written on Linux stay valid.
pub fn text_hash(text: &str) -> String {
    if text.contains("\r\n") {
        sha256_tagged(text.replace("\r\n", "\n").as_bytes())
    } else {
        sha256_tagged(text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the same file text with LF and with CRLF line endings
    /// (a Linux and a Windows `autocrlf` checkout of one commit). Both must
    /// hash alike, the LF form must hash exactly as the plain SHA-256 of its
    /// bytes (so existing indexes stay valid), and an index built from one
    /// form must call the other current.
    ///
    /// Result (2026-10-10, Windows 11): passes.
    #[test]
    fn a_file_hashes_alike_with_unix_and_windows_line_endings() {
        let unix = "fn a() {\n    b();\n}\n";
        let windows = unix.replace('\n', "\r\n");
        assert_eq!(text_hash(unix), text_hash(&windows));
        assert_eq!(text_hash(unix), sha256_tagged(unix.as_bytes()));
        assert_ne!(text_hash(unix), text_hash("fn a() {}\n"));
        let empty = r#"{"schema":1,"kind":"kovan_links","crate":"x","dir":"crates/x","generator":"t","encoding":"utf8","files":[],"ext":[],"defs":[]}"#;
        let mut ix = LinkIndex::parse(empty).unwrap();
        ix.files.push(LinkFile { path: "src/a.rs".into(), hash: text_hash(&windows), occ: Vec::new() });
        assert!(ix.is_current("src/a.rs", unix) && ix.is_current("src/a.rs", &windows));
        assert!(!ix.is_current("src/a.rs", "fn a() {}\n"));
    }

    fn site(p: &str, l: u32, c: u32) -> Site {
        Site { path: p.into(), line: l, col: c }
    }

    /// Methodology: two own files and one other crate's file. `a.rs` calls
    /// `b.rs`'s `leaf` twice and a function in `crates/y`; `b.rs` uses a
    /// local. The index is built from the occurrences in two input orders
    /// (byte-identical JSON), round-trips through [`LinkIndex::parse`],
    /// answers go-to-definition and find-references, notices an edited file,
    /// and refuses out-of-range and foreign documents.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn links_round_trip_and_answer_definition_and_references() {
        let leaf = site("crates/x/src/b.rs", 0, 7);
        let other = site("crates/y/src/lib.rs", 3, 7);
        let a = FileOccs {
            path: "crates/x/src/a.rs".into(),
            text: "fn f() { leaf(); leaf(); y::g(); }\n".into(),
            occs: vec![
                Occ { line: 0, col: 9, len: 4, def: leaf.clone() },
                Occ { line: 2, col: 4, len: 4, def: leaf.clone() },
                Occ { line: 2, col: 15, len: 1, def: other.clone() },
            ],
        };
        let b = FileOccs {
            path: "crates/x/src/b.rs".into(),
            text: "pub fn leaf() { let v = 1; v; }\n".into(),
            occs: vec![Occ { line: 0, col: 27, len: 1, def: site("crates/x/src/b.rs", 0, 20) }],
        };
        let ix = build("x", "crates/x", "rust-analyzer 1.98.0", "utf8", &[a.clone(), b.clone()]);
        let again = build("x", "crates/x", "rust-analyzer 1.98.0", "utf8", &[b, a.clone()]);
        assert_eq!(ix.to_json(), again.to_json());
        let back = LinkIndex::parse(&ix.to_json()).unwrap();
        assert_eq!(back, ix);
        assert_eq!(ix.ext, vec!["crates/y/src/lib.rs"]);
        assert_eq!(ix.files[0].occ, vec![0, 9, 4, 0, 2, 4, 4, 0, 0, 15, 1, 2]);
        assert_eq!(ix.definition_at("src/a.rs", 2, 6), Some(leaf.clone()));
        assert_eq!(ix.definition_at("src/a.rs", 2, 15), Some(other));
        assert_eq!(ix.definition_at("src/a.rs", 1, 0), None);
        assert_eq!(
            ix.references(&leaf),
            vec![site("crates/x/src/a.rs", 0, 9), site("crates/x/src/a.rs", 2, 4)]
        );
        assert!(ix.is_current("src/a.rs", &a.text));
        assert!(!ix.is_current("src/a.rs", "fn f() {}\n"));
        let mut bad = ix.clone();
        bad.files[0].occ[3] = 99;
        assert_eq!(LinkIndex::parse(&bad.to_json()), Err(LinksError::OutOfRange("occ definition")));
        let mut foreign = ix.clone();
        foreign.kind = "paper".into();
        assert!(matches!(LinkIndex::parse(&foreign.to_json()), Err(LinksError::NotLinks(_))));
    }
}
