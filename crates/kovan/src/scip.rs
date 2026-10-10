//! A reader for the SCIP index `rust-analyzer scip` writes (GitHub #757).
//!
//! SCIP ("SCIP Code Intelligence Protocol", <https://github.com/sourcegraph/scip>,
//! Apache-2.0) is a protobuf file holding, for every source file of the
//! workspace, every *occurrence* of a symbol: its range, the symbol string and
//! whether the occurrence defines or references it. One `rust-analyzer scip`
//! run over the whole workspace (about 3 min, 343 MB on 2026-10-07) therefore
//! answers every "where is this defined?" question the call graph used to ask
//! rust-analyzer's LSP one token at a time.
//!
//! # Why a hand-written decoder
//!
//! The index is read with a ~100-line protobuf wire-format decoder over the
//! handful of fields the call graph needs, instead of the `scip` crate: that
//! crate pulls in `protobuf` and its code generator, none of which is in the
//! workspace, and the fields used here are a stable subset of
//! `scip.proto` (checked against the proto at the version rust-analyzer 1.98.0
//! writes):
//!
//! ```text
//! Index              1 metadata, 2 documents (repeated), 3 external_symbols
//! Metadata           2 tool_info, 3 project_root
//! ToolInfo           1 name, 2 version
//! Document           1 relative_path, 2 occurrences (repeated), 6 position_encoding
//! Occurrence         1 range (packed int32: [line, start, end] or
//!                    [line, start, end_line, end]), 2 symbol, 3 symbol_roles
//! ```
//!
//! Unknown fields are skipped, so a newer index with more fields still reads.
//! The format is unstable on rust-analyzer's side (`scip` is listed as an
//! unstable subcommand), which is why the version that wrote the index is
//! kept ([`ScipIndex::tool_version`]) and recorded in the call graph.
//!
//! # Symbols are matched by location, never by string alone
//!
//! rust-analyzer's symbol strings name the package, not the Cargo target, so
//! a library module and an example module with the same path share a symbol
//! (1,011 symbols had more than one definition in the #757 measurement). A
//! reference is resolved to the definition **nearest to it** in the file tree
//! ([`ScipIndex::nearest_definition`]): the same file first, then the longest
//! shared folder prefix. One exception: when the reference is written through
//! the package's own library crate name (`my_crate::prelude::run` from that
//! package's example, which has a `run` of its own), only the library's
//! definitions are candidates. Within one file, helpers of one name
//! nested in different functions also share a symbol; there the caller's
//! scope decides (see [`ScipIndex::nearest_definition`]). Both cases were
//! found by the #757 comparison (`crates/kovan/docs/call-graph-scip-vs-lsp.md`).
//!
//! Plain `std`; no I/O except [`ScipIndex::read`].

use std::collections::HashMap;
use std::path::Path;

/// How a document's columns are counted (`Document.position_encoding`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionEncoding {
    /// Not stated; read as UTF-8 bytes, which is what rust-analyzer writes.
    Unspecified,
    Utf8,
    Utf16,
    Utf32,
}

impl PositionEncoding {
    fn from_proto(v: u64) -> PositionEncoding {
        match v {
            1 => PositionEncoding::Utf8,
            2 => PositionEncoding::Utf16,
            3 => PositionEncoding::Utf32,
            _ => PositionEncoding::Unspecified,
        }
    }

    fn units(self, c: char) -> u32 {
        match self {
            PositionEncoding::Unspecified | PositionEncoding::Utf8 => c.len_utf8() as u32,
            PositionEncoding::Utf16 => c.len_utf16() as u32,
            PositionEncoding::Utf32 => 1,
        }
    }

    /// The column in this encoding of the `col`-th char of `line`.
    pub fn units_of_char_col(self, line: &str, col: u32) -> u32 {
        line.chars().take(col as usize).map(|c| self.units(c)).sum()
    }

    /// The char column of encoding column `units` of `line` (the char that
    /// starts at or covers it; past the end, counted on as if ASCII).
    pub fn char_col_of_units(self, line: &str, units: u32) -> u32 {
        let mut u = 0u32;
        let mut n = 0u32;
        for c in line.chars() {
            if u >= units {
                return n;
            }
            u += self.units(c);
            n += 1;
        }
        n + units.saturating_sub(u)
    }
}

/// A symbol: a workspace-wide one (interned), or a document-local one
/// (`local <n>`, meaningful only inside its own document).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sym {
    Global(u32),
    Local(u32),
}

/// One occurrence of a symbol in a document. Lines and columns are 0-based,
/// columns in the document's [`PositionEncoding`], end exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occurrence {
    pub line: u32,
    pub start: u32,
    pub end_line: u32,
    pub end: u32,
    pub symbol: Sym,
    /// `SymbolRole` bits; [`Occurrence::is_definition`] reads bit 0.
    pub roles: u32,
}

impl Occurrence {
    pub fn is_definition(&self) -> bool {
        self.roles & 1 != 0
    }
}

/// One source file of the index.
#[derive(Debug, Clone)]
pub struct Document {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    pub encoding: PositionEncoding,
    /// Sorted by `(line, start, end_line, end)`.
    pub occurrences: Vec<Occurrence>,
}

impl Document {
    /// The occurrences on `line` (those that start there).
    pub fn on_line(&self, line: u32) -> &[Occurrence] {
        let a = self.occurrences.partition_point(|o| o.line < line);
        let b = self.occurrences.partition_point(|o| o.line <= line);
        &self.occurrences[a..b]
    }

    /// The occurrences that start exactly at `(line, start)`.
    pub fn at(&self, line: u32, start: u32) -> impl Iterator<Item = &Occurrence> {
        self.on_line(line).iter().filter(move |o| o.start == start)
    }

    /// Where local symbol `n` is defined in this document.
    pub fn local_definition(&self, n: u32) -> Option<&Occurrence> {
        self.occurrences
            .iter()
            .find(|o| o.symbol == Sym::Local(n) && o.is_definition())
    }
}

/// Where a global symbol is defined: a document (index into
/// [`ScipIndex::documents`]) and the 0-based start of the defining range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DefSite {
    pub doc: u32,
    pub line: u32,
    pub start: u32,
}

/// A decoded index: every document's occurrences, and every global symbol's
/// definition sites.
#[derive(Debug, Clone, Default)]
pub struct ScipIndex {
    /// `ToolInfo.name` and `.version`, as the indexer wrote them.
    pub tool_name: String,
    pub tool_version: String,
    /// `Metadata.project_root` (a `file://` URI).
    pub project_root: String,
    /// Sorted by path.
    pub documents: Vec<Document>,
    by_path: HashMap<String, u32>,
    /// Interned global symbol strings.
    symbol_ids: HashMap<String, u32>,
    /// Indexed by global symbol id; sorted, de-duplicated.
    defs: Vec<Vec<DefSite>>,
    /// Global symbols of an indexed package that have no definition in any
    /// document: items a derive or another macro generated
    /// (`#[derive(Default)]`'s `default`).
    generated: std::collections::HashSet<u32>,
    /// The package of every symbol with more than one definition.
    collided: HashMap<u32, String>,
}

/// A path under a Cargo target folder other than the library's: an example,
/// test or bench, or a binary (`src/bin/`, `src/main.rs`).
fn is_non_lib_path(path: &str) -> bool {
    path.split('/')
        .any(|s| matches!(s, "examples" | "tests" | "benches"))
        || path.contains("/src/bin/")
        || path.starts_with("src/bin/")
        || path.ends_with("src/main.rs")
}

/// The package of a symbol string (`rust-analyzer cargo <package>
/// <version> <descriptors>`), if it has that shape.
fn package_of(symbol: &str) -> Option<&str> {
    let mut parts = symbol.splitn(5, ' ');
    let (_scheme, _manager, package) = (parts.next()?, parts.next()?, parts.next()?);
    parts.next()?;
    Some(package)
}

impl ScipIndex {
    /// Reads and decodes an index file.
    pub fn read(path: &Path) -> Result<ScipIndex, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        ScipIndex::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Decodes an index from its bytes.
    pub fn decode(bytes: &[u8]) -> Result<ScipIndex, String> {
        let mut ix = ScipIndex::default();
        let mut docs: Vec<Document> = Vec::new();
        for_each_field(bytes, |num, w| {
            match (num, w) {
                (1, Wire::Len(a, b)) => ix.read_metadata(&bytes[a..b])?,
                (2, Wire::Len(a, b)) => docs.push(ix.read_document(&bytes[a..b])?),
                _ => {}
            }
            Ok(())
        })?;
        docs.sort_by(|a, b| a.path.cmp(&b.path));
        // A file indexed twice (a `#[path]` module of a test that is also a
        // binary's root, a module shared by two examples) is one document
        // with the global occurrences of both copies: each copy defines its
        // own symbols, and dropping one lost its definitions (#757). Locals
        // are kept from the first copy only, since each copy numbers its
        // own.
        let mut merged: Vec<Document> = Vec::with_capacity(docs.len());
        for d in docs {
            match merged.last_mut() {
                Some(last) if last.path == d.path => {
                    last.occurrences.extend(
                        d.occurrences
                            .into_iter()
                            .filter(|o| matches!(o.symbol, Sym::Global(_))),
                    );
                    last.occurrences
                        .sort_by_key(|o| (o.line, o.start, o.end_line, o.end, o.symbol, o.roles));
                    last.occurrences.dedup();
                }
                _ => merged.push(d),
            }
        }
        let docs = merged;
        ix.defs = vec![Vec::new(); ix.symbol_ids.len()];
        for (k, d) in docs.iter().enumerate() {
            ix.by_path.insert(d.path.clone(), k as u32);
            for o in d.occurrences.iter().filter(|o| o.is_definition()) {
                if let Sym::Global(g) = o.symbol {
                    ix.defs[g as usize].push(DefSite {
                        doc: k as u32,
                        line: o.line,
                        start: o.start,
                    });
                }
            }
        }
        for v in &mut ix.defs {
            v.sort();
            v.dedup();
        }
        let indexed: std::collections::HashSet<&str> = ix
            .symbol_ids
            .iter()
            .filter(|(_, &g)| !ix.defs[g as usize].is_empty())
            .filter_map(|(s, _)| package_of(s))
            .collect();
        let generated = ix
            .symbol_ids
            .iter()
            .filter(|(s, &g)| {
                ix.defs[g as usize].is_empty() && package_of(s).is_some_and(|p| indexed.contains(p))
            })
            .map(|(_, &g)| g)
            .collect();
        ix.generated = generated;
        let collided = ix
            .symbol_ids
            .iter()
            .filter(|(_, &g)| ix.defs[g as usize].len() > 1)
            .filter_map(|(s, &g)| package_of(s).map(|p| (g, p.to_string())))
            .collect();
        ix.collided = collided;
        ix.documents = docs;
        Ok(ix)
    }

    fn read_metadata(&mut self, m: &[u8]) -> Result<(), String> {
        let (mut name, mut version, mut root) = (String::new(), String::new(), String::new());
        for_each_field(m, |num, w| {
            match (num, w) {
                (2, Wire::Len(a, b)) => {
                    let t = &m[a..b];
                    for_each_field(t, |n, w| {
                        match (n, w) {
                            (1, Wire::Len(a, b)) => name = utf8(&t[a..b])?,
                            (2, Wire::Len(a, b)) => version = utf8(&t[a..b])?,
                            _ => {}
                        }
                        Ok(())
                    })?
                }
                (3, Wire::Len(a, b)) => root = utf8(&m[a..b])?,
                _ => {}
            }
            Ok(())
        })?;
        self.tool_name = name;
        self.tool_version = version;
        self.project_root = root;
        Ok(())
    }

    fn read_document(&mut self, d: &[u8]) -> Result<Document, String> {
        let mut path = String::new();
        let mut encoding = PositionEncoding::Unspecified;
        let mut occurrences = Vec::new();
        for_each_field(d, |num, w| {
            match (num, w) {
                // rust-analyzer on Windows writes `crates\x\src\lib.rs`
                // (GitHub #820). Every lookup here is by a `/`-separated
                // workspace-relative path, so the separator is normalised on
                // read, on every platform: an index written on Windows then
                // reads the same on Linux.
                (1, Wire::Len(a, b)) => path = utf8(&d[a..b])?.replace('\\', "/"),
                (2, Wire::Len(a, b)) => {
                    if let Some(o) = self.read_occurrence(&d[a..b])? {
                        occurrences.push(o);
                    }
                }
                (6, Wire::Varint(v)) => encoding = PositionEncoding::from_proto(v),
                _ => {}
            }
            Ok(())
        })?;
        occurrences
            .sort_by_key(|o: &Occurrence| (o.line, o.start, o.end_line, o.end, o.symbol, o.roles));
        occurrences.dedup();
        Ok(Document {
            path,
            encoding,
            occurrences,
        })
    }

    fn read_occurrence(&mut self, o: &[u8]) -> Result<Option<Occurrence>, String> {
        let mut range: Vec<u32> = Vec::new();
        let mut symbol = (0usize, 0usize);
        let mut roles = 0u32;
        for_each_field(o, |num, w| {
            match (num, w) {
                (1, Wire::Len(a, b)) => {
                    let mut i = a;
                    while i < b {
                        let (v, n) = varint(&o[..b], i)?;
                        range.push(v as u32);
                        i = n;
                    }
                }
                (1, Wire::Varint(v)) => range.push(v as u32),
                (2, Wire::Len(a, b)) => symbol = (a, b),
                (3, Wire::Varint(v)) => roles = v as u32,
                _ => {}
            }
            Ok(())
        })?;
        let (line, start, end_line, end) = match range.as_slice() {
            [l, s, e] => (*l, *s, *l, *e),
            [l, s, el, e] => (*l, *s, *el, *e),
            _ => return Ok(None),
        };
        let symbol = &o[symbol.0..symbol.1];
        if symbol.is_empty() {
            return Ok(None);
        }
        let local = symbol
            .strip_prefix(b"local ")
            .and_then(|n| std::str::from_utf8(n).ok())
            .and_then(|n| n.parse().ok());
        let symbol = match local {
            Some(n) => Sym::Local(n),
            None => self.intern(symbol)?,
        };
        Ok(Some(Occurrence {
            line,
            start,
            end_line,
            end,
            symbol,
            roles,
        }))
    }

    fn intern(&mut self, s: &[u8]) -> Result<Sym, String> {
        if let Some(&g) = std::str::from_utf8(s)
            .ok()
            .and_then(|k| self.symbol_ids.get(k))
        {
            return Ok(Sym::Global(g));
        }
        let s = utf8(s)?;
        let next = self.symbol_ids.len() as u32;
        self.symbol_ids.insert(s, next);
        Ok(Sym::Global(next))
    }

    /// The document for a workspace-relative path.
    pub fn document(&self, path: &str) -> Option<&Document> {
        self.by_path.get(path).map(|&k| &self.documents[k as usize])
    }

    /// Every definition site of a global symbol (empty for a symbol defined
    /// outside the indexed documents: std or a dependency).
    pub fn definitions(&self, sym: Sym) -> &[DefSite] {
        match sym {
            Sym::Global(g) => self.defs.get(g as usize).map_or(&[], Vec::as_slice),
            Sym::Local(_) => &[],
        }
    }

    /// A symbol of an indexed (workspace) package with no definition in the
    /// index: generated by a derive or another macro. Its source is the
    /// attribute or macro call, not a function body.
    pub fn is_generated(&self, sym: Sym) -> bool {
        matches!(sym, Sym::Global(g) if self.generated.contains(&g))
    }

    /// The definition of `sym` nearest to a reference at 0-based line
    /// `from_line` of file `from`: the same file first (there: not inside
    /// one of the `hidden` line ranges, the bodies of functions other than
    /// the caller, whose nested items the reference cannot see; then inside
    /// `within`, the caller's own body, whose nested item shadows an outer
    /// one; then the nearest line), then the most shared leading folders,
    /// then the first by `(path, line)`. Two helpers nested in different
    /// functions of one file share a symbol, which is why the scope rules
    /// are needed. `lead` is the first segment of the path the
    /// reference is written with (`my_crate` in `my_crate::a::f`), if any:
    /// when it is the symbol's own package's library name, only library
    /// definitions are candidates. `None` when it has none in the index.
    pub fn nearest_definition(
        &self,
        sym: Sym,
        from: &str,
        from_line: u32,
        within: Option<(u32, u32)>,
        hidden: &[(u32, u32)],
        lead: Option<&str>,
    ) -> Option<DefSite> {
        let all = self.definitions(sym);
        if all.len() <= 1 {
            return all.first().copied();
        }
        let lib_only: Vec<DefSite> = match (lead, sym) {
            (Some(lead), Sym::Global(g)) => match self.collided.get(&g) {
                Some(pkg) if lead == pkg.replace('-', "_") => all
                    .iter()
                    .filter(|d| !is_non_lib_path(&self.documents[d.doc as usize].path))
                    .copied()
                    .collect(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        let defs: &[DefSite] = if lib_only.is_empty() { all } else { &lib_only };
        let score = |d: &DefSite| {
            let p = self.documents[d.doc as usize].path.as_str();
            let shared = p
                .split('/')
                .zip(from.split('/'))
                .take_while(|(a, b)| a == b)
                .count();
            let same = p == from;
            let inside = same && within.is_some_and(|(a, b)| a <= d.line && d.line <= b);
            let visible = !same || !hidden.iter().any(|&(a, b)| a <= d.line && d.line <= b);
            let closeness = if same {
                u32::MAX - d.line.abs_diff(from_line)
            } else {
                0
            };
            (same, visible, inside, shared, closeness)
        };
        // max_by_key keeps the LAST maximum; iterate reversed so the first
        // (smallest path, line) wins a tie.
        defs.iter().rev().max_by_key(|d| score(d)).copied()
    }

    /// The number of distinct global symbols.
    pub fn symbol_count(&self) -> usize {
        self.symbol_ids.len()
    }

    /// Total occurrences over every document.
    pub fn occurrence_count(&self) -> usize {
        self.documents.iter().map(|d| d.occurrences.len()).sum()
    }
}

fn utf8(s: &[u8]) -> Result<String, String> {
    String::from_utf8(s.to_vec()).map_err(|e| format!("a string field is not UTF-8: {e}"))
}

/// A protobuf field value: wire type 0 (varint) and 2 (length-delimited, as
/// the byte range `a..b` of the message); fixed-width values are skipped.
#[derive(Debug, Clone, Copy)]
enum Wire {
    Varint(u64),
    Len(usize, usize),
    Fixed,
}

fn varint(b: &[u8], mut i: usize) -> Result<(u64, usize), String> {
    let mut x = 0u64;
    let mut shift = 0u32;
    loop {
        let c = *b.get(i).ok_or("truncated varint")?;
        i += 1;
        if shift < 64 {
            x |= u64::from(c & 0x7f) << shift;
        }
        if c < 0x80 {
            return Ok((x, i));
        }
        shift += 7;
        if shift > 70 {
            return Err("varint longer than 10 bytes".into());
        }
    }
}

/// Calls `f(field number, value)` for every field of one message, in order.
fn for_each_field(
    b: &[u8],
    mut f: impl FnMut(u64, Wire) -> Result<(), String>,
) -> Result<(), String> {
    let mut i = 0;
    while i < b.len() {
        let (key, j) = varint(b, i)?;
        let (num, wire) = (key >> 3, key & 7);
        let (w, next) = match wire {
            0 => {
                let (v, j) = varint(b, j)?;
                (Wire::Varint(v), j)
            }
            2 => {
                let (n, j) = varint(b, j)?;
                let end = j
                    .checked_add(n as usize)
                    .filter(|&e| e <= b.len())
                    .ok_or("length-delimited field runs past the end")?;
                (Wire::Len(j, end), end)
            }
            1 => (Wire::Fixed, j + 8),
            5 => (Wire::Fixed, j + 4),
            w => return Err(format!("unsupported protobuf wire type {w}")),
        };
        if next > b.len() {
            return Err("fixed-width field runs past the end".into());
        }
        f(num, w)?;
        i = next;
    }
    Ok(())
}

/// Protobuf encoding helpers for tests (here and in the call-graph tests):
/// a hand-built index in the same wire format rust-analyzer writes.
#[cfg(test)]
pub(crate) mod encode {
    pub(crate) fn varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }

    pub(crate) fn bytes(num: u64, b: &[u8], out: &mut Vec<u8>) {
        varint(num << 3 | 2, out);
        varint(b.len() as u64, out);
        out.extend_from_slice(b);
    }

    pub(crate) fn uint(num: u64, v: u64, out: &mut Vec<u8>) {
        varint(num << 3, out);
        varint(v, out);
    }

    /// `(range, symbol, roles)`.
    pub(crate) type Occ<'a> = (&'a [u32], &'a str, u64);

    pub(crate) fn index(tool_version: &str, docs: &[(&str, &[Occ])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut tool = Vec::new();
        bytes(1, b"rust-analyzer", &mut tool);
        bytes(2, tool_version.as_bytes(), &mut tool);
        let mut meta = Vec::new();
        bytes(2, &tool, &mut meta);
        bytes(3, b"file:///ws", &mut meta);
        bytes(1, &meta, &mut out);
        for (path, occs) in docs {
            let mut d = Vec::new();
            bytes(1, path.as_bytes(), &mut d);
            for (range, sym, roles) in occs.iter() {
                let mut o = Vec::new();
                let mut packed = Vec::new();
                for &r in range.iter() {
                    varint(u64::from(r), &mut packed);
                }
                bytes(1, &packed, &mut o);
                bytes(2, sym.as_bytes(), &mut o);
                if *roles != 0 {
                    uint(3, *roles, &mut o);
                }
                bytes(2, &o, &mut d);
            }
            uint(6, 1, &mut d);
            bytes(2, &d, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a hand-encoded index whose document paths use `\`, as
    /// rust-analyzer 1.97.1 writes them on Windows (seen 2026-10-10 in a
    /// whole-workspace index of outram-park-backend: `crates\tuas_…\main.rs`).
    /// The document must be found by its `/`-separated workspace-relative
    /// path, and an example must still be told from the library.
    ///
    /// Before the fix every lookup missed, so that run wrote 48 empty
    /// `kovan_links.json` files and no `callees` for 45,397 functions
    /// (GitHub #820).
    ///
    /// Result (2026-10-10, Windows 11): passes.
    #[test]
    fn windows_separators_in_document_paths_are_normalised() {
        let f = "rust-analyzer cargo app 0.1.0 m/f().";
        let lib = [(&[1u32, 7, 8][..], f, 1u64)];
        let ex = [(&[3u32, 4, 5][..], f, 1u64), (&[9, 8, 9], f, 0)];
        let bytes = encode::index(
            "1.97.1",
            &[
                ("crates\\app\\src\\m.rs", &lib),
                ("crates\\app\\examples\\x\\m.rs", &ex),
            ],
        );
        let ix = ScipIndex::decode(&bytes).unwrap();
        assert!(ix.document("crates/app/src/m.rs").is_some());
        assert!(ix.document("crates\\app\\src\\m.rs").is_none());
        let d = ix.document("crates/app/examples/x/m.rs").unwrap();
        assert_eq!(d.occurrences.len(), 2);
        assert!(is_non_lib_path(&d.path));
        assert!(!is_non_lib_path("crates/app/src/m.rs"));
    }

    /// Methodology: a hand-encoded index (the wire format of `scip.proto`)
    /// with two documents, a symbol defined in both (the cross-target
    /// collision #757 found), a local, and a symbol with no definition (std).
    /// Decoding must return the tool version, both documents sorted, the
    /// occurrences by position, and resolve each reference to the nearest
    /// definition.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn decodes_and_resolves_by_location() {
        let shared = "rust-analyzer cargo app 0.1.0 m/f().";
        let ex = [
            (&[3u32, 4, 5][..], shared, 1u64),
            (&[9, 8, 9], shared, 0),
            (&[9, 1, 2], "local 7", 0),
            (&[2, 4, 5], "local 7", 1),
        ];
        let lib = [
            (&[1u32, 7, 8][..], shared, 1u64),
            (&[5, 0, 6, 3], "rust-analyzer cargo std 1 Vec#new().", 0),
        ];
        let bytes = encode::index(
            "1.98.0",
            &[
                ("crates/app/src/m.rs", &lib),
                ("crates/app/examples/x/m.rs", &ex),
            ],
        );
        let ix = ScipIndex::decode(&bytes).unwrap();
        assert_eq!(ix.tool_name, "rust-analyzer");
        assert_eq!(ix.tool_version, "1.98.0");
        assert_eq!(ix.documents[0].path, "crates/app/examples/x/m.rs");
        assert_eq!(ix.documents[0].encoding, PositionEncoding::Utf8);
        let d = ix.document("crates/app/examples/x/m.rs").unwrap();
        assert_eq!(
            d.occurrences.iter().map(|o| o.line).collect::<Vec<_>>(),
            vec![2, 3, 9, 9]
        );
        let r = d.at(9, 8).next().unwrap();
        assert_eq!(ix.definitions(r.symbol).len(), 2);
        // From the example, the example's own definition; from elsewhere in
        // src, the library's.
        let near = ix
            .nearest_definition(
                r.symbol,
                "crates/app/examples/x/main.rs",
                0,
                None,
                &[],
                None,
            )
            .unwrap();
        assert_eq!(
            (ix.documents[near.doc as usize].path.as_str(), near.line),
            ("crates/app/examples/x/m.rs", 3)
        );
        let near = ix
            .nearest_definition(r.symbol, "crates/app/src/lib.rs", 0, None, &[], None)
            .unwrap();
        assert_eq!(
            (ix.documents[near.doc as usize].path.as_str(), near.line),
            ("crates/app/src/m.rs", 1)
        );
        // Written through the library's crate name from the example: the
        // library's, although the example's own copy is nearer.
        let near = ix
            .nearest_definition(
                r.symbol,
                "crates/app/examples/x/main.rs",
                0,
                None,
                &[],
                Some("app"),
            )
            .unwrap();
        assert_eq!(ix.documents[near.doc as usize].path, "crates/app/src/m.rs");
        let near = ix
            .nearest_definition(
                r.symbol,
                "crates/app/examples/x/main.rs",
                0,
                None,
                &[],
                Some("crate"),
            )
            .unwrap();
        assert_eq!(
            ix.documents[near.doc as usize].path,
            "crates/app/examples/x/m.rs"
        );
        let local = d.at(9, 1).next().unwrap();
        assert_eq!(d.local_definition(7).map(|o| o.line), Some(2));
        assert_eq!(local.symbol, Sym::Local(7));
        let lib_doc = ix.document("crates/app/src/m.rs").unwrap();
        let std_ref = lib_doc.at(5, 0).next().unwrap();
        assert_eq!((std_ref.end_line, std_ref.end), (6, 3));
        assert!(ix.definitions(std_ref.symbol).is_empty());
        assert!(
            !ix.is_generated(std_ref.symbol),
            "std is not an indexed package"
        );
    }

    /// Methodology: a workspace package's symbol with no definition
    /// occurrence (what `#[derive(Default)]` leaves: references to
    /// `impl#[T][Default]default()` and no definition) is `generated`; a std
    /// symbol is not.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn derived_items_are_generated() {
        let derived = "rust-analyzer cargo app 0.1.0 m/impl#[T][Default]default().";
        let occs = [
            (
                &[0u32, 0, 1][..],
                "rust-analyzer cargo app 0.1.0 m/T#",
                1u64,
            ),
            (&[2, 4, 11], derived, 0),
            (
                &[3, 4, 7],
                "rust-analyzer cargo core 1 default/Default#default().",
                0,
            ),
        ];
        let ix = ScipIndex::decode(&encode::index("1", &[("m.rs", &occs)])).unwrap();
        let d = ix.document("m.rs").unwrap();
        assert!(ix.is_generated(d.at(2, 4).next().unwrap().symbol));
        assert!(!ix.is_generated(d.at(3, 4).next().unwrap().symbol));
        assert_eq!(package_of(derived), Some("app"));
    }

    /// Methodology: a file indexed twice (two documents, one path) keeps
    /// both copies' definitions; two definitions of one symbol in one file
    /// resolve to the one nearer the reference.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn duplicate_documents_merge_and_same_file_picks_nearest() {
        let a = "rust-analyzer cargo app 0.1.0 bin/f().";
        let b = "rust-analyzer cargo app 0.1.0 t/f().";
        let h = "rust-analyzer cargo app 0.1.0 m/helper().";
        let first = [(&[1u32, 3, 4][..], a, 1u64), (&[2, 0, 1], "local 1", 1)];
        let second = [(&[1u32, 3, 4][..], b, 1u64), (&[2, 0, 1], "local 1", 1)];
        let nested = [
            (&[3u32, 7, 13][..], h, 1u64),
            (&[40, 7, 13], h, 1),
            (&[44, 4, 10], h, 0),
        ];
        let ix = ScipIndex::decode(&encode::index(
            "1",
            &[("x.rs", &first), ("x.rs", &second), ("n.rs", &nested)],
        ))
        .unwrap();
        assert_eq!(ix.documents.len(), 2);
        let x = ix.document("x.rs").unwrap();
        assert_eq!(x.occurrences.len(), 3, "both globals, one local");
        for o in x
            .occurrences
            .iter()
            .filter(|o| matches!(o.symbol, Sym::Global(_)))
        {
            assert_eq!(ix.definitions(o.symbol).len(), 1);
        }
        let n = ix.document("n.rs").unwrap();
        let r = n.at(44, 4).next().unwrap();
        assert_eq!(
            ix.nearest_definition(r.symbol, "n.rs", 44, None, &[], None)
                .unwrap()
                .line,
            40
        );
        // Inside the calling function's body beats nearer elsewhere.
        let inner = ix.nearest_definition(r.symbol, "n.rs", 44, Some((0, 10)), &[], None);
        assert_eq!(inner.unwrap().line, 3);
        // A helper nested in ANOTHER function is out of scope, however near.
        let outer = ix.nearest_definition(
            r.symbol,
            "n.rs",
            44,
            Some((43, 50)),
            &[(0, 10), (39, 42)],
            None,
        );
        assert_eq!(
            outer.unwrap().line,
            40,
            "both hidden: the nearest line decides"
        );
        let top = ix.nearest_definition(r.symbol, "n.rs", 44, Some((43, 50)), &[(39, 42)], None);
        assert_eq!(top.unwrap().line, 3, "line 3 is visible, line 40 hidden");
        assert_eq!(
            ix.nearest_definition(r.symbol, "n.rs", 5, None, &[], None)
                .unwrap()
                .line,
            3
        );
    }

    /// Methodology: char columns convert to and from UTF-8 and UTF-16 units
    /// on a line with a two-byte (`µ`) and a four-byte (`𝜇`) character.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn columns_convert_between_encodings() {
        let line = "a µ 𝜇 f(x)";
        let f = line.chars().position(|c| c == 'f').unwrap() as u32;
        let u8c = PositionEncoding::Utf8.units_of_char_col(line, f);
        assert_eq!(u8c, line.find('f').unwrap() as u32);
        assert_eq!(PositionEncoding::Utf8.char_col_of_units(line, u8c), f);
        let u16c = PositionEncoding::Utf16.units_of_char_col(line, f);
        assert_eq!(u16c, f + 1);
        assert_eq!(PositionEncoding::Utf16.char_col_of_units(line, u16c), f);
        assert_eq!(PositionEncoding::Utf32.units_of_char_col(line, f), f);
    }

    /// Methodology: truncated input is an error, not a panic.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn truncated_input_is_an_error() {
        let bytes = encode::index("1", &[("a.rs", &[(&[0u32, 0, 1][..], "s", 1)])]);
        assert!(ScipIndex::decode(&bytes[..bytes.len() - 3]).is_err());
    }
}
