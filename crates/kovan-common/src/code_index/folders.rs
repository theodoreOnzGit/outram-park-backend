//! Build every folder's `kovan.toml` ([`FolderIndex`]) from one call graph
//! (SCIP backend, #757), the per-file function hashes, the folders'
//! `review.md`, the previous `kovan.toml` files and the counted test
//! evidence (#766). Pure: the caller reads the files and runs the tools.
//!
//! # Which functions
//!
//! Every function of the call graph (lib modules, examples, integration
//! tests: the files Cargo compiles for those targets) that the `syn`
//! hasher ([`crate::review::hash`]) also finds, joined by name and closing
//! line (and first line when that is not unique). A function the hasher
//! does not list (a `fn` nested in another's body, one generated inside a
//! `macro_rules!`) has no hash yet (#764 deferred nested items) and is left
//! out; it is counted in [`BuildReport::unhashed`]. Its calls are not
//! lost: `callees` looks **through** it (below).
//!
//! # Fields
//!
//! - `id`: [`super::ids`].
//! - `qual`: the call graph's id without the `file::` prefix (`#k` kept).
//! - `lines`: the hasher's line range (first outer attribute or doc line to
//!   the closing brace), so the rust-analyzer-free refresh
//!   ([`super::refresh`]) writes the same numbers.
//! - `callees`: the resolved workspace callees by id, looking through
//!   functions that are not indexed (a nested helper's callees are its
//!   outer function's). A callee outside a scoped run's crates is listed
//!   only when a claim or the previous index names its path
//!   ([`super::ids::id_outside`]); otherwise it is counted in
//!   [`BuildReport::outside_without_id`]. A whole-workspace run has none.
//! - `reached_by`: **every** test that reaches the function
//!   ([`crate::call_graph::reach::tests_reaching`]), by **test id**, the
//!   call graph's path id of the test function, as `kovan-cli test`'s
//!   evidence names tests (#766); not the opaque id. Test functions list
//!   none.
//! - `[test_run]`: **carried over** from the folder's previous
//!   `kovan.toml` (maintainer, #766, 2026-10-07: test evidence lives in
//!   the committed `kovan.toml`, written there by `kovan-cli test`). The
//!   caller recovers it from the last committed version when the file on
//!   disk is malformed, and passes none (pending, never passed) when that
//!   fails too.
//! - `[upstream]` and `[[review]]`: cached from the folder's `review.md`.
//! - `[[deleted_folder]]`: the crate root folder only, from git
//!   ([`super::deleted`]).
//! - `commit` is left out: writing the commit an index was built at into
//!   every `kovan.toml` would change every file on every commit (and
//!   committing them would change it again), so diffs would carry no
//!   information. The commit is used only to mint new ids.
//!
//! The module key of a file is `crate::<path>` for the library,
//! `example:<name>::<path>` and `test:<name>::<path>` for example and
//! integration-test targets (`crate`, `example:<name>`, `test:<name>` for a
//! target's root file).

use std::collections::{BTreeMap, BTreeSet};

use crate::call_graph::reach::tests_reaching;
use crate::call_graph::{CallGraphDoc, TargetKind};
use crate::review::hash::HashedFn;
use crate::review::index::{
    CachedReview, DeletedFolder, FolderIndex, FunctionIndex, ItemKind, ModuleIndex, TestRun,
};
use crate::review::review_md::ReviewDocument;

use super::ids::{assign_ids, claims_from_review_md, id_outside, priors_of, Located, Moved, Unmatched};

/// Everything [`build`] reads, owned (no borrowed struct: workspace Rust
/// rules).
#[derive(Debug, Clone, Default)]
pub struct BuildInput {
    pub doc: CallGraphDoc,
    /// Workspace-relative file -> its hashed functions. A file missing here
    /// (it did not parse) contributes no functions; the caller reports it.
    pub hashed: BTreeMap<String, Vec<HashedFn>>,
    /// Workspace-relative `review.md` path -> parsed.
    pub reviews: BTreeMap<String, ReviewDocument>,
    /// Folder -> its previous, readable `kovan.toml`.
    pub previous: BTreeMap<String, FolderIndex>,
    /// Folder -> its carried-over `[test_run]` (module doc).
    pub test_runs: BTreeMap<String, TestRun>,
    /// Crate name -> its deleted folders' history.
    pub deleted: BTreeMap<String, Vec<DeletedFolder>>,
    /// The commit the index is built at (minting only).
    pub commit: String,
}

/// What [`build`] could not do cleanly, for the caller to print.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildReport {
    /// Call-graph functions with no `syn` hash (nested or macro-made).
    pub unhashed: Vec<String>,
    pub moved: Vec<Moved>,
    pub unmatched: Vec<Unmatched>,
    /// Out-of-scope callees with no known id (scoped runs).
    pub outside_without_id: Vec<String>,
}

/// The folders' indexes and the report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Built {
    /// Folder -> its `kovan.toml`.
    pub folders: BTreeMap<String, FolderIndex>,
    pub report: BuildReport,
}

/// The folder part of a workspace-relative path.
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, f)| f)
}

fn module_key(kind: TargetKind, target: &str, path: &str) -> String {
    let root = match kind {
        TargetKind::Lib => "crate".to_string(),
        TargetKind::Example => format!("example:{target}"),
        TargetKind::Test => format!("test:{target}"),
    };
    if path.is_empty() {
        root
    } else {
        format!("{root}::{path}")
    }
}

/// The hashed function the call graph's `f` is.
fn join<'h>(hs: &'h [HashedFn], name: &str, start: u32, end: u32) -> Option<&'h HashedFn> {
    let c: Vec<&HashedFn> = hs
        .iter()
        .filter(|h| h.entry.name == name && h.entry.lines[1] == end)
        .collect();
    match c.as_slice() {
        [one] => Some(one),
        [] => None,
        many => {
            let s: Vec<&&HashedFn> = many.iter().filter(|h| h.entry.lines[0] == start).collect();
            match s.as_slice() {
                [one] => Some(one),
                _ => None,
            }
        }
    }
}

/// Build every folder's index (module doc).
pub fn build(input: &BuildInput) -> Built {
    let doc = &input.doc;
    let mut report = BuildReport::default();
    // 1. Join hashes; collect the located functions (path id -> row).
    struct Row {
        dir: String,
        file: String,
        name: String,
        test: bool,
        hash: String,
        doc_hash: String,
        lines: [u32; 2],
    }
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let mut folders: BTreeMap<String, FolderIndex> = BTreeMap::new();
    for c in &doc.crates {
        let root_dir = c
            .targets
            .iter()
            .find(|t| t.kind == TargetKind::Lib)
            .or(c.targets.first())
            .map(|t| parent(&t.root).to_string());
        for t in c.targets.iter().chain(c.tests.iter()) {
            for m in &t.modules {
                let dir = parent(&m.file).to_string();
                let fi = folders
                    .entry(dir.clone())
                    .or_insert_with(|| FolderIndex::new(&c.name, &dir));
                if root_dir.as_deref() == Some(dir.as_str()) {
                    fi.crate_root = true;
                }
                fi.modules.insert(
                    file_name(&m.file).to_string(),
                    ModuleIndex {
                        path: module_key(t.kind, &t.name, &m.path),
                        functions: Vec::new(),
                    },
                );
                let hs = input.hashed.get(&m.file).map(Vec::as_slice).unwrap_or(&[]);
                for f in &m.functions {
                    match join(hs, &f.name, f.start_line, f.end_line) {
                        Some(h) => {
                            rows.insert(
                                f.id.clone(),
                                Row {
                                    dir: dir.clone(),
                                    file: file_name(&m.file).to_string(),
                                    name: f.name.clone(),
                                    test: f.test,
                                    hash: h.hashes.hash.clone(),
                                    doc_hash: h.hashes.doc_hash.clone(),
                                    lines: h.entry.lines,
                                },
                            );
                        }
                        None => report.unhashed.push(f.id.clone()),
                    }
                }
            }
        }
    }
    // 2. Ids.
    let mut claims = Vec::new();
    for (path, rd) in &input.reviews {
        claims.extend(claims_from_review_md(path, rd));
    }
    let priors: Vec<_> = input.previous.values().flat_map(priors_of).collect();
    let located: Vec<Located> = rows
        .iter()
        .map(|(pid, r)| Located {
            path_id: pid.clone(),
            lines: r.lines,
            hash: r.hash.clone(),
        })
        .collect();
    let a = assign_ids(&located, &claims, &priors, &input.commit);
    report.moved = a.moved;
    report.unmatched = a.unmatched;
    let mut ids: BTreeMap<String, String> = a.ids;
    let mut outside_missing = BTreeSet::new();
    for o in &doc.outside {
        match id_outside(&o.id, &claims, &priors) {
            Some(id) => {
                ids.insert(o.id.clone(), id);
            }
            None => {
                outside_missing.insert(o.id.clone());
            }
        }
    }
    // 3. Callees, looking through functions that are not indexed.
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for c in &doc.calls {
        if c.from != c.to {
            adj.entry(c.from.as_str()).or_default().insert(c.to.as_str());
        }
    }
    let callees_of = |start: &str, missing: &mut BTreeSet<String>| -> Vec<String> {
        let mut out = BTreeSet::new();
        let mut seen: BTreeSet<&str> = BTreeSet::from([start]);
        let mut stack: Vec<&str> = adj.get(start).map(|s| s.iter().copied().collect()).unwrap_or_default();
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if let Some(id) = ids.get(n) {
                out.insert(id.clone());
            } else if outside_missing.contains(n) {
                missing.insert(n.to_string());
            } else if let Some(next) = adj.get(n) {
                stack.extend(next.iter().copied());
            }
        }
        out.into_iter().collect()
    };
    let reach = tests_reaching(doc);
    // 4. Fill the folders.
    let mut missing = BTreeSet::new();
    for (pid, r) in &rows {
        let reached_by: Vec<String> = if r.test {
            Vec::new()
        } else {
            reach.get(pid).map(|s| s.iter().cloned().collect()).unwrap_or_default()
        };
        let entry = FunctionIndex {
            id: ids[pid].clone(),
            name: r.name.clone(),
            qual: pid
                .strip_prefix(&format!("{}/{}::", r.dir, r.file))
                .unwrap_or(pid)
                .to_string(),
            item: ItemKind::Fn,
            lines: r.lines,
            hash: r.hash.clone(),
            doc_hash: r.doc_hash.clone(),
            callees: callees_of(pid, &mut missing),
            reached_by,
            test: r.test,
            index_out_of_date: false,
        };
        if let Some(m) = folders
            .get_mut(&r.dir)
            .and_then(|fi| fi.modules.get_mut(&r.file))
        {
            m.functions.push(entry);
        }
    }
    report.outside_without_id = missing.into_iter().collect();
    report.unhashed.sort();
    for (dir, fi) in &mut folders {
        fi.test_run = input.test_runs.get(dir).cloned();
        let md = if dir.is_empty() {
            "review.md".to_string()
        } else {
            format!("{dir}/review.md")
        };
        if let Some(rd) = input.reviews.get(&md) {
            fi.upstream = rd.upstream().cloned();
            fi.reviews = rd
                .reviews()
                .map(|r| CachedReview {
                    function: r.function_id(),
                    by: r.review.by.clone(),
                    artifact: r.kovan.id.clone(),
                })
                .collect();
        }
        if fi.crate_root {
            fi.deleted_folders = input.deleted.get(&fi.krate).cloned().unwrap_or_default();
        }
        fi.normalise();
    }
    Built { folders, report }
}

#[cfg(test)]
#[path = "folders_tests.rs"]
pub(crate) mod tests;
