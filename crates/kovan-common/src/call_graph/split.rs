//! The call graph cut into **one file per crate**, for the web (web-kovan,
//! GitHub #736 / #738).
//!
//! The whole-workspace [`CallGraphDoc`] is too large to download at once
//! (10.5 MB for two crates, #737), so `kovan-cli call-graph --split-dir`
//! writes:
//!
//! ```text
//! index.json      SplitIndex: every crate's counts and file size, and every
//!                 review stamp's state (small)
//! <crate>.json    CrateSlice: the crate's targets, module tree and functions
//!                 WITHOUT source text, the calls into and out of it, the
//!                 module and crate aggregates touching it, and a per-file
//!                 `links` slot (empty for now)
//! ```
//!
//! and `search.json` ([`SearchIndex`]): every crate, module and function by
//! name and path, compact, for the page's search bar (loaded up front).
//!
//! The web page fetches `index.json` and `search.json` at start and a `<crate>.json` when a
//! crate is opened or expanded.
//!
//! # No source text (maintainer, 2026-10-06)
//!
//! Source is **not** shipped: each function keeps its file, its line range,
//! its signature and its first doc sentence, and the page fetches the file
//! itself on demand from the repository at the commit the site was built
//! from (`raw.githubusercontent.com/<repo>/<commit>/<file>`), then slices the
//! function's lines. Measured 2026-10-06 on boon-lay (914 functions): the
//! slice is 2.27 MB pretty with source, 1.22 MB pretty without, and smaller
//! again compact (the form written here; the report of #736 has the number).
//!
//! # Links in the source (reserved, empty)
//!
//! [`SourceFile::links`] is where definition and reference links for the
//! identifiers in a file will go (token range -> target id), to be filled
//! from rust-analyzer's whole-workspace index (`rust-analyzer scip`, #745).
//! Nothing fills it yet; the slot exists so that the UI and the file layout
//! do not change when it is filled.
//!
//! # Determinism
//!
//! Pure functions of the document: same document, byte-identical files
//! (`BTreeMap`s and the document's own sorted order only).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{AggregateCall, Call, CallGraphDoc, CrateGraph, OutsideFn, Totals};

/// Version of the split layout; bumped on any breaking change.
pub const SPLIT_SCHEMA: u32 = 1;

/// `index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitIndex {
    pub schema: u32,
    /// The [`super::SCHEMA_VERSION`] of the document that was split.
    pub graph_schema: u32,
    pub crates: Vec<IndexEntry>,
    /// The whole document's totals.
    pub totals: Totals,
    /// Every review stamp's state (`review/stamps.toml`, #739), judged when
    /// the files were written. Empty when there are no stamps.
    #[serde(default)]
    pub stamps: Vec<StampState>,
}

/// One crate in [`SplitIndex`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub name: String,
    pub dir: String,
    /// `<name>.json`, relative to the index.
    pub file: String,
    /// Bytes of `file`, so the page can say what it loads.
    pub bytes: u64,
    pub modules: usize,
    pub functions: usize,
}

/// `<crate>.json`: one crate with every call that starts or ends in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateSlice {
    pub schema: u32,
    /// The crate, every function's `source` emptied (module doc).
    #[serde(rename = "crate")]
    pub krate: CrateGraph,
    /// Calls whose caller or callee is in this crate.
    pub calls: Vec<Call>,
    /// Module aggregates with either end in this crate.
    pub module_calls: Vec<AggregateCall>,
    /// Crate aggregates with either end this crate.
    pub crate_calls: Vec<AggregateCall>,
    /// The out-of-scope workspace functions this crate's calls reach.
    pub outside: Vec<OutsideFn>,
    /// Schema 2: the commit the graph was built at, and the site base the
    /// `cited_by[].site` paths are relative to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_base: Option<String>,
    /// One per module file, sorted by file.
    pub files: Vec<SourceFile>,
}

/// Per-file data of a [`CrateSlice`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    pub file: String,
    /// Identifier links in this file. **Reserved, always empty today** (see
    /// the module doc).
    #[serde(default)]
    pub links: Vec<SourceLink>,
}

/// One identifier in a source file and where it leads (reserved).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLink {
    /// 1-based line.
    pub line: u32,
    /// 0-based character columns, end exclusive.
    pub col_start: u32,
    pub col_end: u32,
    /// A call-graph function id, or another symbol id.
    pub target: String,
    pub kind: LinkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Definition,
    Reference,
}

/// A stamp's state as the web shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StampVerdict {
    /// The function still hashes to the stamped hash.
    Valid,
    /// Void: the function changed (or is gone) since it was stamped.
    Stale,
}

/// One stamp, judged: from the first-version `review/stamps.toml` checker
/// (`kovan::review_stamps::check`, what `kovan-cli call-graph --split-dir`
/// writes today), or since 2026-10-10 from `review.md` through the
/// staleness engine (`kovan::stamping::stamp_states`, which fills `state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StampState {
    /// The stamped function's id (code-walk path form).
    pub function: String,
    pub verdict: StampVerdict,
    /// Why it is stale; empty when valid.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    pub rung: u8,
    pub reviewer: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The code as it was stamped.
    pub permalink: String,
    /// The full state from the staleness engine (#765,
    /// [`crate::review::state::StateKind`]); absent in data built from the
    /// first-version `review/stamps.toml` checker, which only knows
    /// valid/stale ([`StampState::kind`] maps those). Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<crate::review::state::StateKind>,
    /// Rung 5, IV&V (GitHub #810): passed or not, every review's reasons
    /// in words, the audit record (shown, never verified), the "independent
    /// V&V not counted" flag and the registry warnings that concern it
    /// ([`crate::review::ivv_view::summarise`]). Absent in older data and
    /// from the `stamps.toml` checker. Additive.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::review::ivv_view::serde_opt_arc"
    )]
    pub ivv: Option<std::sync::Arc<crate::review::ivv_view::IvvSummary>>,
}

impl StampState {
    /// The state kind: `state` when present, else `verdict` read as valid
    /// or directly stale.
    pub fn kind(&self) -> crate::review::state::StateKind {
        use crate::review::state::StateKind;
        self.state.unwrap_or(match self.verdict {
            StampVerdict::Valid => StateKind::Valid,
            StampVerdict::Stale => StateKind::DirectlyStale,
        })
    }
}

/// `search.json`: names and paths of everything, for the search bar.
/// Tuples serialise as JSON arrays, which keeps the file small.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndex {
    pub schema: u32,
    /// Crate names; a [`SearchModule`]'s first field indexes this.
    pub crates: Vec<String>,
    pub modules: Vec<SearchModule>,
    pub functions: Vec<SearchFn>,
}

/// `(crate index, file, module path)`; the path is `""` for a target root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchModule(pub usize, pub String, pub String);

/// `(module index, id after "<file>::", line)`: the function id is
/// `"{file}::{suffix}"`. Test functions are left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFn(pub usize, pub String, pub u32);

impl SearchIndex {
    /// Build from the whole document (test modules and functions left out).
    pub fn build(doc: &CallGraphDoc) -> SearchIndex {
        let mut ix = SearchIndex { schema: SPLIT_SCHEMA, crates: Vec::new(), modules: Vec::new(), functions: Vec::new() };
        for (ci, c) in doc.crates.iter().enumerate() {
            ix.crates.push(c.name.clone());
            for t in &c.targets {
                for m in t.modules.iter().filter(|m| !m.test) {
                    let mi = ix.modules.len();
                    ix.modules.push(SearchModule(ci, m.file.clone(), m.path.clone()));
                    for f in m.functions.iter().filter(|f| !f.test) {
                        let suffix = f.id.strip_prefix(&format!("{}::", m.file)).unwrap_or(&f.id).to_string();
                        ix.functions.push(SearchFn(mi, suffix, f.line));
                    }
                }
            }
        }
        ix
    }
}

/// Cut `doc` into one [`CrateSlice`] per crate, in the document's order.
pub fn split(doc: &CallGraphDoc) -> Vec<CrateSlice> {
    // Which crate each in-scope function and module file belongs to.
    let mut fn_crate: BTreeMap<&str, &str> = BTreeMap::new();
    let mut file_crate: BTreeMap<&str, &str> = BTreeMap::new();
    for c in &doc.crates {
        for t in c.targets.iter().chain(c.tests.iter()) {
            for m in &t.modules {
                file_crate.insert(m.file.as_str(), c.name.as_str());
                for f in &m.functions {
                    fn_crate.insert(f.id.as_str(), c.name.as_str());
                }
            }
        }
    }
    doc.crates
        .iter()
        .map(|c| {
            let name = c.name.as_str();
            let touches_fn = |id: &str| fn_crate.get(id) == Some(&name);
            let calls: Vec<Call> =
                doc.calls.iter().filter(|k| touches_fn(&k.from) || touches_fn(&k.to)).cloned().collect();
            let reached: BTreeSet<&str> = calls.iter().map(|k| k.to.as_str()).collect();
            let outside = doc.outside.iter().filter(|o| reached.contains(o.id.as_str())).cloned().collect();
            let in_file = |f: &str| file_crate.get(f) == Some(&name);
            let module_calls =
                doc.module_calls.iter().filter(|a| in_file(&a.from) || in_file(&a.to)).cloned().collect();
            let crate_calls = doc.crate_calls.iter().filter(|a| a.from == name || a.to == name).cloned().collect();
            let mut krate = c.clone();
            let mut files = Vec::new();
            for t in krate.targets.iter_mut().chain(krate.tests.iter_mut()) {
                for m in &mut t.modules {
                    files.push(SourceFile { file: m.file.clone(), links: Vec::new() });
                    for f in &mut m.functions {
                        f.source.clear();
                    }
                }
            }
            files.sort_by(|a, b| a.file.cmp(&b.file));
            files.dedup_by(|a, b| a.file == b.file);
            CrateSlice {
                schema: SPLIT_SCHEMA,
                krate,
                calls,
                module_calls,
                crate_calls,
                outside,
                commit: doc.commit.clone(),
                site_base: doc.site_base.clone(),
                files,
            }
        })
        .collect()
}

/// Compact JSON with a trailing newline, the form every split file is
/// written in. Compact, not pretty: these files are downloaded, never read
/// or committed.
pub fn to_json<T: Serialize>(v: &T) -> String {
    let mut s = serde_json::to_string(v).expect("split file serialises");
    s.push('\n');
    s
}

/// The serialised files of a split: `(file name, contents)`, the index last.
/// `stamps` is stored in the index as given.
pub fn split_files(doc: &CallGraphDoc, stamps: Vec<StampState>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut crates = Vec::new();
    for slice in split(doc) {
        let name = slice.krate.name.clone();
        let text = to_json(&slice);
        let modules = slice.krate.targets.iter().map(|t| t.modules.len()).sum();
        let functions = slice.krate.targets.iter().flat_map(|t| &t.modules).map(|m| m.functions.len()).sum();
        crates.push(IndexEntry {
            dir: slice.krate.dir.clone(),
            file: format!("{name}.json"),
            bytes: text.len() as u64,
            modules,
            functions,
            name: name.clone(),
        });
        out.push((format!("{name}.json"), text));
    }
    let index = SplitIndex { schema: SPLIT_SCHEMA, graph_schema: doc.schema, crates, totals: doc.totals.clone(), stamps };
    out.push(("search.json".to_string(), to_json(&SearchIndex::build(doc))));
    out.push(("index.json".to_string(), to_json(&index)));
    out
}

#[cfg(test)]
mod tests {
    use super::super::tests::fixture;
    use super::*;

    /// Methodology: split the two-crate fixture of `call_graph::tests`
    /// (`app` calls `core`, one call leaves the scope) and check that each
    /// crate's slice holds exactly the calls touching it, no source text,
    /// an empty `links` slot per file, and that splitting twice gives the
    /// same bytes.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn each_crate_gets_its_own_calls_and_no_source() {
        let (crates, calls, outside) = fixture();
        let doc = CallGraphDoc::assemble(crates, calls, outside, 0);
        let parts = split(&doc);
        assert_eq!(parts.len(), 2);
        let app = &parts[0];
        assert_eq!(app.krate.name, "app");
        let fns: Vec<_> = app.krate.targets.iter().flat_map(|t| &t.modules).flat_map(|m| &m.functions).collect();
        assert!(fns.iter().all(|f| f.source.is_empty() && f.end_line >= f.start_line));
        assert_eq!(app.files.len(), 3);
        assert!(app.files.iter().all(|f| f.links.is_empty()));
        // app: main->go, go->leaf, step->help, step->go. core: only go->leaf.
        assert_eq!(app.calls.len(), 4);
        let core = &parts[1];
        assert_eq!(core.calls.len(), 1);
        assert_eq!(core.calls[0].from, "crates/app/src/run.rs::go");
        assert_eq!(app.outside.len(), 1);
        assert!(core.outside.is_empty());
        let a = split_files(&doc, Vec::new());
        let b = split_files(&doc, Vec::new());
        assert_eq!(a, b);
        assert_eq!(a.last().unwrap().0, "index.json");
        let index: SplitIndex = serde_json::from_str(&a.last().unwrap().1).unwrap();
        let search: SearchIndex = serde_json::from_str(&a[a.len() - 2].1).unwrap();
        assert_eq!(search.crates, vec!["app", "core"]);
        let f = &search.functions[0];
        let m = &search.modules[f.0];
        assert!(format!("{}::{}", m.1, f.1).starts_with("crates/app/"));
        assert_eq!(index.crates[0].functions, 3);
        assert_eq!(index.crates[0].bytes as usize, a[0].1.len());
        let back: CrateSlice = serde_json::from_str(&a[0].1).unwrap();
        assert_eq!(&back, app);
    }
}
