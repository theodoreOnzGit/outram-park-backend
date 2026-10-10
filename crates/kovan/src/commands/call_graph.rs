//! `kovan-cli call-graph`: the workspace call graph, crate → module/example
//! → function, with each function's source, as deterministic JSON for the
//! code-review UI (GitHub #737, part of #735). The data model and the
//! sorting live in [`crate::call_graph`]; this module finds the code and
//! resolves the calls.
//!
//! # How it is built
//!
//! 1. `cargo metadata --no-deps` ([`crate::code_map::run_cargo_metadata`])
//!    lists each crate's targets. The scope is every member, or `--crates`.
//!    Maturity comes from the same `[package.metadata.kovan]` tags the code
//!    map reads ([`crate::code_map::CodeMap::from_cargo_metadata`]).
//! 2. For the lib target, every example target and (schema 2, #746) every
//!    integration-test target (`tests/*.rs`, kept in `CrateGraph::tests`),
//!    the module tree is read from the root file's `mod` declarations
//!    ([`crate::call_graph::modules`]), so a multi-file example such as
//!    `examples/htgr_sim_v1/` is a tree of modules too. Bins and benches are
//!    not included. ~~Tests are not included.~~ **CORRECTED 2026-10-06**:
//!    integration tests are, so the walk from a test can start there. Each
//!    file's attribution header is read into `Module::upstream`
//!    ([`crate::call_graph::upstream`]).
//! 3. Every `fn` in those files is found by `code-walk`'s source scanner
//!    (`code_walk::source::FileIndex`), which supplies the qualified name,
//!    signature, doc sentence and body range.
//! 4. Each function's body is expanded by `code-walk`'s builder
//!    (`code_walk::builder::Workspace::expand`): every call-shaped token is
//!    sent to rust-analyzer (through the keep-warm `lsp_daemon`, one batched
//!    request per function) and becomes a resolved call, a call into std or
//!    a dependency (counted, dropped), or an `UNRESOLVED(<kind>)` gap. No
//!    call is guessed. Every call site is kept, not only the first per pair.
//! 5. Schema 2 (#746): the code-walk blocks of every Markdown page under
//!    `docs/` and `crates/*/docs/` become `Function::cited_by`
//!    ([`crate::call_graph::citations`]); one `git log` over the scope's
//!    crate folders gives each file's `history`
//!    ([`crate::call_graph::history`]); `assemble` then walks the graph
//!    from every test and example into `Function::reached_by`
//!    ([`crate::call_graph::reach`]).
//!
//! # Two backends (GitHub #757)
//!
//! Step 4's definitions come from one of two places, chosen with
//! `--backend` ([`CallBackend`]):
//!
//! - **`lsp`** (the default): rust-analyzer's LSP through the keep-warm
//!   daemon, one batched request per function. Measured 941 s for the whole
//!   workspace from cold (#757); fine for a few crates.
//! - **`scip`**: one `rust-analyzer scip` run over the whole workspace
//!   (`--scip <file>` reads one written earlier), decoded in memory
//!   ([`crate::scip`]) and asked the **same positions** the LSP would be,
//!   so every classification (trait gap, closure, constructor, external) is
//!   the same code. It also adds what the text scanner cannot see: calls
//!   through **operators** (`CallKind::Operator`, a workspace `impl Add`
//!   reached by `a + b`) and functions **named as paths**
//!   (`.map(T::f)`, `CallKind::FnValue`). The function list, ids,
//!   signatures, docs, upstream, history and citations still come from the
//!   source scanner: definitions are matched to it by **location**, never by
//!   SCIP symbol string, because symbols collide across Cargo targets.
//!
//! The document records which backend and which rust-analyzer resolved it
//! (`generator`, schema 3): SCIP is an unstable rust-analyzer subcommand.
//! Agreement between the two and the timings are in
//! `crates/kovan/docs/call-graph-scip-vs-lsp.md`.
//!
//! Known limits are `code-walk`'s: trait-method calls stop at
//! `UNRESOLVED(trait)`; a call to a derived method (`Default::default()` on
//! a `#[derive(Default)]` type) is `UNRESOLVED(other)`; macro bodies are
//! not entered; a `fn` nested in a body is listed on its own and its calls
//! are also counted in the outer body; a function value written as a path
//! (`.map(other_crate::leaf)`) is not seen at all, only a bare name
//! (`.map(leaf)`) is (found 2026-10-06 while writing
//! `tests/call_graph_rust_analyzer.rs`). **CORRECTED 2026-10-07 (#757):**
//! with `--backend scip` a path is seen (`fn_value`), and operator calls
//! are too (`operator`); the LSP backend still misses both. With either
//! backend: a call through a function-typed parameter is
//! `UNRESOLVED(closure)` (it was a false recursive edge to the enclosing
//! function until #757), and the gaps of functions in integration tests are
//! kept (they were collected and dropped until #757). Where the backends
//! differ by design: rust-analyzer's LSP jumps from `x.to_string()` to the
//! type's `Display::fmt` (an edge, or `UNRESOLVED(other)` for a derived
//! `Display`), SCIP records the reference to std's `ToString::to_string`
//! (external, dropped).
//!
//! # Cost
//!
//! LSP backend: one rust-analyzer definition query per call-shaped token. Measured
//! 2026-10-06 (16-core desktop, rust-analyzer 1.98.0) on
//! `outram-park-digital-twin-engine` (lib and its 4 examples) plus
//! `boon-lay` (lib and 5 examples): 3632 functions, 37641 queries, 127 s
//! with a cold rust-analyzer (about 60 s of it indexing the workspace) and
//! 50 s warm; the two runs' JSON (10.5 MB) was byte-identical.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;

use super::code_walk::builder::Workspace;
use super::code_walk::graph::EdgeKind;
use super::code_walk::source::{Container, FileIndex};
use crate::call_graph::citations::{self, Citation, CitationKind, FnRef};
use crate::call_graph::history::{self, CommitRef};
use crate::call_graph::{modules, upstream};
use crate::call_graph::{
    function_ids, Backend, CallGraphDoc, CallKind, CrateGraph, FnKind, Function, Generator, Module,
    OutsideFn, RawCall, Target, TargetKind, Unresolved,
};
use crate::code_map::CodeMap;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: String,
    #[serde(default)]
    targets: Vec<MetaTarget>,
}

#[derive(Deserialize)]
struct MetaTarget {
    name: String,
    kind: Vec<String>,
    src_path: String,
}

/// A workspace member, paths relative to the workspace root.
struct Member {
    name: String,
    dir: String,
    targets: Vec<(TargetKind, String, String)>,
}

fn relative(root: &Path, p: &Path) -> Option<String> {
    let canon = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let rel = canon.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn members(root: &Path, json: &str) -> Result<Vec<Member>, String> {
    let meta: Metadata = serde_json::from_str(json)
        .map_err(|e| format!("cargo metadata is not the expected JSON: {e}"))?;
    let mut out = Vec::new();
    for p in meta.packages {
        let manifest = PathBuf::from(&p.manifest_path);
        let Some(dir) = manifest.parent().and_then(|d| relative(root, d)) else {
            continue;
        };
        let mut targets = Vec::new();
        for t in &p.targets {
            let kind = if t.kind.iter().any(|k| {
                matches!(
                    k.as_str(),
                    "lib" | "rlib" | "cdylib" | "staticlib" | "dylib" | "proc-macro"
                )
            }) {
                TargetKind::Lib
            } else if t.kind.iter().any(|k| k == "example") {
                TargetKind::Example
            } else if t.kind.iter().any(|k| k == "test") {
                TargetKind::Test
            } else {
                continue;
            };
            if let Some(src) = relative(root, Path::new(&t.src_path)) {
                targets.push((kind, t.name.clone(), src));
            }
        }
        out.push(Member {
            name: p.name,
            dir,
            targets,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The 0-based first line of the doc comment and attributes directly above
/// 0-based `line` (the same lines `source::first_doc_line` reads).
fn start_of_item(lines: &[String], line: u32) -> u32 {
    let mut k = line as usize;
    while k > 0 {
        let t = lines[k - 1].trim();
        if t.starts_with("///") || t.starts_with("#[") || t.ends_with(")]") || t.ends_with("\"]") {
            k -= 1;
        } else {
            break;
        }
    }
    k as u32
}

fn attributes_text(lines: &[String], from: u32, to: u32) -> String {
    (from..to)
        .filter_map(|k| lines.get(k as usize))
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The maturity of library module `path`: the deepest tagged module that is
/// it or an ancestor, else the crate's.
fn module_maturity(map: Option<&CodeMap>, krate: &str, path: &str) -> Option<u8> {
    let c = map?.get(krate)?;
    let mut best: Option<(usize, u8)> = None;
    for m in &c.maturity_modules {
        let hit = path == m.module || path.starts_with(&format!("{}::", m.module));
        if hit && best.is_none_or(|(len, _)| m.module.len() > len) {
            best = Some((m.module.len(), m.level));
        }
    }
    Some(best.map_or(c.maturity, |b| b.1))
}

/// Where `call-graph` gets its definitions (see the module doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallBackend {
    /// rust-analyzer's LSP, through the keep-warm daemon.
    Lsp,
    /// A SCIP index: this file, or (`None`) one generated now with
    /// `rust-analyzer scip` into `target/kovan-scip/index.scip`.
    Scip(Option<PathBuf>),
}

/// `rust-analyzer --version`, or `rust-analyzer missing`.
pub fn rust_analyzer_version() -> String {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "rust-analyzer missing".into())
}

/// Runs `rust-analyzer scip` over the workspace into
/// `<root>/target/kovan-scip/index.scip` (never committed: `target/` is
/// ignored), its log beside it, and returns the index path. About 3 min and
/// several GB of memory for this workspace (#757).
pub fn generate_scip(root: &Path) -> Result<PathBuf, String> {
    generate_scip_controlled(root, &super::index_control::RunControl::default())
}

/// [`generate_scip`] with progress, cancellation and priority (GitHub #780):
/// the child is run by [`super::index_control::run_child`], under `nice`
/// when `ctl.nice`, and a cancel stops it by its own process id.
pub fn generate_scip_controlled(root: &Path, ctl: &super::index_control::RunControl) -> Result<PathBuf, String> {
    let dir = root.join("target").join("kovan-scip");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let out = dir.join("index.scip");
    let log_path = dir.join("rust-analyzer-scip.log");
    ctl.say(format!(
        "call-graph: running rust-analyzer scip over {} (about 3 min for outram-park; log in {})",
        root.display(),
        log_path.display()
    ));
    let started = Instant::now();
    let mut cmd = super::index_control::command(
        "rust-analyzer",
        &["scip".as_ref(), root.as_os_str(), "--output".as_ref(), out.as_os_str()],
        ctl.nice,
    );
    cmd.current_dir(root);
    let status = super::index_control::run_child(cmd, &log_path, "rust-analyzer scip", ctl)?;
    if !status.success() {
        return Err(format!(
            "rust-analyzer scip failed ({status}); see {}",
            log_path.display()
        ));
    }
    ctl.say(format!(
        "call-graph: rust-analyzer scip wrote {} in {:.1} s",
        out.display(),
        started.elapsed().as_secs_f64()
    ));
    Ok(out)
}

/// Builds the call graph of `scope` (every member when `None`) with the
/// LSP backend.
pub fn build(root: &Path, scope: Option<&[String]>) -> Result<CallGraphDoc, String> {
    build_using(root, scope, &CallBackend::Lsp)
}

/// Builds the call graph of `scope` (every member when `None`) with the
/// given backend.
pub fn build_using(
    root: &Path,
    scope: Option<&[String]>,
    backend: &CallBackend,
) -> Result<CallGraphDoc, String> {
    let started = Instant::now();
    let root =
        std::fs::canonicalize(root).map_err(|e| format!("resolving {}: {e}", root.display()))?;
    let json = crate::code_map::run_cargo_metadata(&root, false)?;
    let all = members(&root, &json)?;
    let map = match CodeMap::from_cargo_metadata(&json) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!(
                "call-graph: maturity tags unreadable, maturity omitted: {}",
                e.join("; ")
            );
            None
        }
    };
    let selected: Vec<&Member> = match scope {
        None => all.iter().collect(),
        Some(names) => {
            let mut v = Vec::new();
            for n in names {
                let m = all.iter().find(|m| &m.name == n).ok_or_else(|| {
                    format!(
                        "`{n}` is not a member of the workspace at {}",
                        root.display()
                    )
                })?;
                v.push(m);
            }
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v.dedup_by(|a, b| a.name == b.name);
            v
        }
    };

    let (mut ws, generator) = match backend {
        CallBackend::Lsp => (
            Workspace::open(&root)?,
            Generator {
                backend: Backend::Lsp,
                rust_analyzer: rust_analyzer_version(),
            },
        ),
        CallBackend::Scip(file) => {
            let path = match file {
                Some(p) => p.clone(),
                None => generate_scip(&root)?,
            };
            let ix = read_scip(&path)?;
            let installed = rust_analyzer_version();
            if ix.tool_version.is_empty() || !installed.contains(&ix.tool_version) {
                eprintln!(
                    "call-graph: WARNING: the index was written by {} {}, the installed one is {installed}",
                    ix.tool_name, ix.tool_version
                );
            }
            scip_workspace(&root, ix)?
        }
    };
    let result = build_with(&mut ws, &all, &selected, map.as_ref(), started);
    ws.close();
    let mut doc = result?;
    doc.generator = Some(generator);
    Ok(doc)
}

/// Reads a SCIP index and says what it holds.
pub fn read_scip(path: &Path) -> Result<crate::scip::ScipIndex, String> {
    let t = Instant::now();
    let ix = crate::scip::ScipIndex::read(path)?;
    eprintln!(
        "call-graph: read {} in {:.1} s: {} documents, {} occurrences, {} symbols, written by {} {}",
        path.display(),
        t.elapsed().as_secs_f64(),
        ix.documents.len(),
        ix.occurrence_count(),
        ix.symbol_count(),
        ix.tool_name,
        ix.tool_version
    );
    Ok(ix)
}

fn scip_workspace(root: &Path, ix: crate::scip::ScipIndex) -> Result<(Workspace, Generator), String> {
    let generator = Generator {
        backend: Backend::Scip,
        rust_analyzer: format!("{} {}", ix.tool_name, ix.tool_version)
            .trim()
            .to_string(),
    };
    Ok((Workspace::open_scip(root, ix)?, generator))
}

/// Builds the call graph of `scope` (every member when `None`) from an
/// already decoded SCIP index (`kovan-cli index`, #767, which also reads
/// the index for the link files and so decodes it once).
pub fn build_from_scip(
    root: &Path,
    scope: Option<&[String]>,
    ix: crate::scip::ScipIndex,
) -> Result<CallGraphDoc, String> {
    let started = Instant::now();
    let root =
        std::fs::canonicalize(root).map_err(|e| format!("resolving {}: {e}", root.display()))?;
    let json = crate::code_map::run_cargo_metadata(&root, false)?;
    let all = members(&root, &json)?;
    let map = CodeMap::from_cargo_metadata(&json).ok();
    let selected: Vec<&Member> = match scope {
        None => all.iter().collect(),
        Some(names) => {
            let mut v = Vec::new();
            for n in names {
                v.push(all.iter().find(|m| &m.name == n).ok_or_else(|| {
                    format!("`{n}` is not a member of the workspace at {}", root.display())
                })?);
            }
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v.dedup_by(|a, b| a.name == b.name);
            v
        }
    };
    let (mut ws, generator) = scip_workspace(&root, ix)?;
    let result = build_with(&mut ws, &all, &selected, map.as_ref(), started);
    ws.close();
    let mut doc = result?;
    doc.generator = Some(generator);
    Ok(doc)
}

/// Every workspace member's (name, folder), folders workspace-relative,
/// sorted by name (`cargo metadata --no-deps`).
pub fn member_dirs(root: &Path) -> Result<Vec<(String, String)>, String> {
    let root =
        std::fs::canonicalize(root).map_err(|e| format!("resolving {}: {e}", root.display()))?;
    let json = crate::code_map::run_cargo_metadata(&root, false)?;
    Ok(members(&root, &json)?
        .into_iter()
        .map(|m| (m.name, m.dir))
        .collect())
}

fn build_with(
    ws: &mut Workspace,
    all: &[Member],
    selected: &[&Member],
    map: Option<&CodeMap>,
    started: Instant,
) -> Result<CallGraphDoc, String> {
    let root = ws.root.clone();
    let exists = |rel: &str| root.join(rel).is_file();

    // 1-2. Module trees and the functions in them.
    let mut crates: Vec<CrateGraph> = Vec::new();
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    // (file, 1-based line) -> function id, for every in-scope function.
    let mut in_scope: HashMap<(String, u32), String> = HashMap::new();
    for m in selected {
        let mut targets = Vec::new();
        let mut tests = Vec::new();
        for (kind, name, src) in &m.targets {
            let mut mods: Vec<Module> = Vec::new();
            // (file, module path, parent, test, mod-rs); an integration-test
            // target is test code throughout.
            let root_test = *kind == TargetKind::Test;
            let mut queue = vec![(src.clone(), String::new(), None::<String>, root_test, true)];
            while let Some((file, path, parent, test, mod_rs)) = queue.pop() {
                if !claimed.insert(file.clone()) {
                    eprintln!("call-graph: {file} is reached twice; kept under its first module");
                    continue;
                }
                let idx = ws.index(&file)?.clone();
                let found = modules::scan(&idx.lines, &idx.code);
                for d in &found.decls {
                    let Some(child) = modules::candidate_files(&file, mod_rs, d)
                        .into_iter()
                        .find(|c| exists(c))
                    else {
                        eprintln!(
                            "call-graph: {file}:{}: no file for `mod {};`",
                            d.line + 1,
                            d.name
                        );
                        continue;
                    };
                    let mut segs: Vec<&str> = if path.is_empty() {
                        Vec::new()
                    } else {
                        vec![path.as_str()]
                    };
                    segs.extend(d.inline.iter().map(String::as_str));
                    segs.push(&d.name);
                    let child_path = segs.join("::");
                    // A file loaded through `#[path]` owns its directory like a mod.rs.
                    let child_mod_rs = modules::is_mod_rs(&child) || d.path_attr.is_some();
                    queue.push((
                        child,
                        child_path,
                        Some(file.clone()),
                        test || d.test,
                        child_mod_rs,
                    ));
                }
                let mut functions = functions_of(&file, &idx, &found.test_ranges, test);
                let (upstream, upstream_unparsed) = match upstream::scan(&idx.lines) {
                    upstream::Scan::None => (None, None),
                    upstream::Scan::Parsed(u) => (Some(u), None),
                    upstream::Scan::Unparsed(why) => (None, Some(why)),
                };
                let mut concepts = Vec::new();
                for (line, kind, path) in citations::concept_tags(&idx.lines) {
                    if kind == '!' {
                        concepts.push(path);
                        continue;
                    }
                    // A `///` tag belongs to the function whose doc block holds it.
                    if let Some(f) = functions
                        .iter_mut()
                        .find(|f| f.start_line <= line + 1 && line + 1 < f.line)
                    {
                        f.cited_by.push(Citation {
                            kind: CitationKind::ConceptTag,
                            page: path,
                            line: line + 1,
                            anchor: String::new(),
                            site: None,
                        });
                    }
                }
                for f in &functions {
                    in_scope.insert((file.clone(), f.line), f.id.clone());
                }
                let maturity = match kind {
                    TargetKind::Lib => module_maturity(map, &m.name, &path),
                    TargetKind::Example | TargetKind::Test => {
                        map.and_then(|mp| mp.get(&m.name)).map(|c| c.maturity)
                    }
                };
                mods.push(Module {
                    file,
                    path,
                    parent,
                    test,
                    maturity,
                    functions,
                    upstream,
                    upstream_unparsed,
                    history: Vec::new(),
                    concepts,
                });
            }
            let t = Target {
                kind: *kind,
                name: name.clone(),
                root: src.clone(),
                modules: mods,
            };
            match kind {
                TargetKind::Test => tests.push(t),
                _ => targets.push(t),
            }
        }
        crates.push(CrateGraph {
            name: m.name.clone(),
            dir: m.dir.clone(),
            maturity: map.and_then(|mp| mp.get(&m.name)).map(|c| c.maturity),
            targets,
            tests,
        });
    }

    // 3-4. Expand every function through rust-analyzer.
    let mut order: Vec<(String, u32)> = in_scope.keys().cloned().collect();
    order.sort();
    let total = order.len();
    eprintln!(
        "call-graph: {} crate(s), {total} functions to expand",
        crates.len()
    );
    let mut walk = ws.start(&[])?;
    for (k, (file, line)) in order.iter().enumerate() {
        let decl = ws
            .index(file)?
            .fn_at_line(line - 1)
            .cloned()
            .ok_or_else(|| format!("{file}:{line}: fn vanished while building"))?;
        let id = walk.graph.intern(Workspace::node(file, &decl));
        ws.expand(&mut walk, id)?;
        if (k + 1) % 250 == 0 || k + 1 == total {
            eprintln!(
                "call-graph: {}/{total} functions, {} queries, {:.1} s",
                k + 1,
                ws.queries,
                started.elapsed().as_secs_f64()
            );
        }
    }

    // Ids for every node the walk met, in scope or not.
    let mut file_ids: HashMap<String, HashMap<u32, String>> = HashMap::new();
    let mut id_of = |ws: &mut Workspace, file: &str, line: u32| -> Result<String, String> {
        if let Some(id) = in_scope.get(&(file.to_string(), line)) {
            return Ok(id.clone());
        }
        if !file_ids.contains_key(file) {
            let idx = ws.index(file)?;
            let quals: Vec<String> = idx.fns.iter().map(|d| d.qualname()).collect();
            let ids = function_ids(file, &quals);
            let m = idx
                .fns
                .iter()
                .zip(ids)
                .map(|(d, (id, _))| (d.line + 1, id))
                .collect();
            file_ids.insert(file.to_string(), m);
        }
        Ok(file_ids[file]
            .get(&line)
            .cloned()
            .unwrap_or_else(|| format!("{file}::?L{line}")))
    };
    let in_scope_ids: BTreeSet<String> = in_scope.values().cloned().collect();
    let mut raw = Vec::new();
    let mut outside = Vec::new();
    for e in &walk.sites {
        let (from_n, to_n) = (
            walk.graph.node(e.from).clone(),
            walk.graph.node(e.to).clone(),
        );
        let from = id_of(ws, &from_n.file, from_n.line)?;
        let to = id_of(ws, &to_n.file, to_n.line)?;
        if !in_scope_ids.contains(&to) {
            outside.push(OutsideFn {
                id: to.clone(),
                krate: owning_crate(all, &to_n.file),
                file: to_n.file.clone(),
                line: to_n.line,
            });
        }
        raw.push(RawCall {
            from,
            to,
            kind: match e.kind {
                EdgeKind::FnValue => CallKind::FnValue,
                EdgeKind::Operator => CallKind::Operator,
                EdgeKind::Call | EdgeKind::Hand { .. } => CallKind::Call,
            },
            line: e.call_line,
        });
    }
    let mut gaps: HashMap<String, Vec<Unresolved>> = HashMap::new();
    for g in &walk.graph.gaps {
        let n = walk.graph.node(g.from).clone();
        let id = id_of(ws, &n.file, n.line)?;
        gaps.entry(id).or_default().push(Unresolved {
            line: g.call_line,
            kind: g.kind.tag().to_string(),
            callee: g.callee.clone(),
            detail: g.detail.clone(),
        });
    }
    // Every target, integration tests included: ~~`c.targets` only~~
    // CORRECTED 2026-10-07 (#757): the gaps of functions in `tests/*.rs`
    // were collected and then dropped here, so test functions never listed
    // their unresolved calls.
    for c in &mut crates {
        for t in c.targets.iter_mut().chain(c.tests.iter_mut()) {
            for m in &mut t.modules {
                for f in &mut m.functions {
                    if let Some(v) = gaps.remove(&f.id) {
                        f.unresolved = v;
                    }
                }
            }
        }
    }
    // 5. Pages that cite each function, and each file's recent history.
    attach_citations(&root, &mut crates, &in_scope);
    let commit = attach_history(&root, &mut crates);
    let mut doc = CallGraphDoc::assemble(crates, raw, outside, walk.external_calls);
    doc.commit = commit;
    doc.site_base = Some(citations::SITE_BASE.to_string());
    eprintln!(
        "call-graph: done in {:.1} s: {} functions, {} calls ({} sites), {} unresolved, {} outside, {} queries",
        started.elapsed().as_secs_f64(),
        doc.totals.functions,
        doc.totals.calls,
        doc.totals.call_sites,
        doc.totals.unresolved,
        doc.totals.outside,
        ws.queries
    );
    Ok(doc)
}

/// Markdown pages under `docs/` and every `crates/*/docs/`, workspace-
/// relative and sorted (`target`, `book` and dot folders skipped).
fn doc_pages(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "target" && name != "book" && !name.starts_with('.') {
                    walk(root, &path, out);
                }
            } else if name.ends_with(".md") {
                if let Some(r) = relative(root, &path) {
                    out.push(r);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &root.join("docs"), &mut out);
    if let Ok(rd) = std::fs::read_dir(root.join("crates")) {
        for e in rd.flatten() {
            walk(root, &e.path().join("docs"), &mut out);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `(name, owner, id)` of every in-scope function, by file.
type ByFile<'a> = HashMap<&'a str, Vec<(&'a str, Option<&'a str>, &'a str)>>;

/// The file part of a code-walk path (`a/b.rs::T::f` -> `a/b.rs`).
fn path_file(p: &str) -> Option<&str> {
    let (f, _) = p.split_once(".rs::")?;
    Some(&p[..f.len() + 3])
}

/// Resolves a page's function reference to an in-scope id: by declaration
/// line first (`snippet-check`), else by file and name (and owner type when
/// the path names one), and only when exactly one function matches.
fn resolve_ref(
    r: &FnRef,
    in_scope: &HashMap<(String, u32), String>,
    by_file: &ByFile,
) -> Option<String> {
    let (file, name, owner) = match r {
        FnRef::At { file, line, name } => {
            if let Some(id) = in_scope.get(&(file.clone(), *line)) {
                if id.ends_with(&format!("::{name}")) || id.contains(&format!("::{name}#")) {
                    return Some(id.clone());
                }
            }
            (file.as_str(), name.as_str(), None)
        }
        FnRef::Path(p) => {
            let file = path_file(p)?;
            let qual = &p[file.len() + 2..];
            match qual.split_once("::") {
                Some((ty, name)) => (file, name, Some(ty)),
                None => (file, qual, None),
            }
        }
    };
    let hits: Vec<&str> = by_file
        .get(file)?
        .iter()
        .filter(|(n, o, _)| *n == name && (owner.is_none() || *o == owner))
        .map(|(_, _, id)| *id)
        .collect();
    match hits.as_slice() {
        [one] => Some(one.to_string()),
        _ => None,
    }
}

/// Fills `cited_by` from the code-walk blocks of every doc page.
fn attach_citations(
    root: &Path,
    crates: &mut [CrateGraph],
    in_scope: &HashMap<(String, u32), String>,
) {
    let mut books = Vec::new();
    for (list, prefix) in [
        ("deep-dives.txt", "deep-dives"),
        ("tutorials.txt", "tutorials"),
    ] {
        if let Ok(t) = std::fs::read_to_string(root.join("docs/site").join(list)) {
            books.extend(citations::parse_books(&t, prefix));
        }
    }
    let mut found: HashMap<String, Vec<Citation>> = HashMap::new();
    let (mut blocks, mut unmatched) = (0usize, 0usize);
    {
        let mut by_file: ByFile = HashMap::new();
        for c in crates.iter() {
            for t in c.targets.iter().chain(c.tests.iter()) {
                for m in &t.modules {
                    for f in &m.functions {
                        by_file.entry(m.file.as_str()).or_default().push((
                            f.name.as_str(),
                            f.owner.as_deref(),
                            f.id.as_str(),
                        ));
                    }
                }
            }
        }
        for page in doc_pages(root) {
            let Ok(text) = std::fs::read_to_string(root.join(&page)) else {
                continue;
            };
            for b in citations::scan_page(&text) {
                blocks += 1;
                let mut ids = BTreeSet::new();
                for r in &b.refs {
                    let file = match r {
                        FnRef::At { file, .. } => Some(file.as_str()),
                        FnRef::Path(p) => path_file(p),
                    };
                    match resolve_ref(r, in_scope, &by_file) {
                        Some(id) => {
                            ids.insert(id);
                        }
                        None if file.is_some_and(|f| by_file.contains_key(f)) => {
                            eprintln!(
                                "call-graph: {page}:{}: no unique function for {r:?}",
                                b.line
                            );
                            unmatched += 1;
                        }
                        None => {}
                    }
                }
                for id in ids {
                    found.entry(id).or_default().push(Citation {
                        kind: CitationKind::CodeWalk,
                        page: page.clone(),
                        line: b.line,
                        anchor: b.anchor.clone(),
                        site: citations::site_path(&books, &page, &b.anchor),
                    });
                }
            }
        }
    }
    for c in crates.iter_mut() {
        for t in c.targets.iter_mut().chain(c.tests.iter_mut()) {
            for m in &mut t.modules {
                for f in &mut m.functions {
                    if let Some(v) = found.remove(&f.id) {
                        f.cited_by.extend(v);
                    }
                }
            }
        }
    }
    eprintln!(
        "call-graph: {blocks} code-walk block(s) read; {unmatched} in-scope reference(s) matched no unique function"
    );
}

/// Fills each module's `history` from one `git log` over the scope's crate
/// folders and returns HEAD. Outside a git checkout both stay empty.
fn attach_history(root: &Path, crates: &mut [CrateGraph]) -> Option<String> {
    let run = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let head = run(&["rev-parse", "HEAD"]).map(|s| s.trim().to_string())?;
    let mut args: Vec<&str> = history::GIT_LOG_ARGS.to_vec();
    let dirs: Vec<String> = crates.iter().map(|c| c.dir.clone()).collect();
    args.extend(dirs.iter().map(String::as_str));
    let Some(log) = run(&args) else {
        eprintln!("call-graph: git log failed; history omitted");
        return Some(head);
    };
    let mut map: BTreeMap<String, Vec<CommitRef>> = history::parse(&log);
    for c in crates.iter_mut() {
        for t in c.targets.iter_mut().chain(c.tests.iter_mut()) {
            for m in &mut t.modules {
                if let Some(v) = map.remove(&m.file) {
                    m.history = v;
                }
            }
        }
    }
    Some(head)
}

fn owning_crate(all: &[Member], file: &str) -> Option<String> {
    all.iter()
        .filter(|m| file.starts_with(&format!("{}/", m.dir)))
        .max_by_key(|m| m.dir.len())
        .map(|m| m.name.clone())
}

/// The functions declared in one module file.
fn functions_of(
    file: &str,
    idx: &FileIndex,
    test_ranges: &[(u32, u32)],
    module_test: bool,
) -> Vec<Function> {
    let quals: Vec<String> = idx.fns.iter().map(|d| d.qualname()).collect();
    let ids = function_ids(file, &quals);
    let mut seen_lines = BTreeSet::new();
    let mut out = Vec::new();
    for (d, (id, ambiguous)) in idx.fns.iter().zip(ids) {
        if !seen_lines.insert(d.line) {
            continue; // two `fn`s on one line: the walk can only name the first
        }
        let start = start_of_item(&idx.lines, d.line);
        let end = d.body.map_or(d.line, |b| b.1);
        let attrs = attributes_text(&idx.lines, start, d.line);
        let test_fn = modules::is_test_attr(&attrs);
        let test =
            module_test || test_fn || test_ranges.iter().any(|&(a, b)| a <= d.line && d.line <= b);
        let (kind, owner, trait_name) = match &d.container {
            Container::Free => (FnKind::Free, None, None),
            Container::Impl {
                self_ty,
                trait_name: None,
            } => (FnKind::Method, Some(self_ty.clone()), None),
            Container::Impl {
                self_ty,
                trait_name: Some(t),
            } => (FnKind::TraitImpl, Some(self_ty.clone()), Some(t.clone())),
            Container::Trait { name } => (FnKind::TraitDecl, Some(name.clone()), None),
        };
        let source = idx.lines[start as usize..=(end as usize).min(idx.lines.len() - 1)].join("\n");
        out.push(Function {
            id,
            ambiguous,
            name: d.name.clone(),
            kind,
            owner,
            trait_name,
            test,
            start_line: start + 1,
            line: d.line + 1,
            end_line: end + 1,
            signature: d.signature.clone(),
            doc: d.doc.clone(),
            source,
            unresolved: Vec::new(),
            test_fn,
            reached_by: None,
            cited_by: Vec::new(),
        });
    }
    out
}

/// `kovan-cli call-graph`: build and write to `out`, or stdout.
pub fn run(
    root: &Path,
    crates: Option<Vec<String>>,
    out: Option<PathBuf>,
    backend: &CallBackend,
) -> Result<(), String> {
    let doc = build_using(root, crates.as_deref(), backend)?;
    let text = doc.to_json();
    match out {
        Some(path) => {
            std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
            eprintln!("call-graph: wrote {}", path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}

/// `kovan-cli call-graph --split-dir <dir>`: build, then write one
/// `<crate>.json` per crate (no source text) and an `index.json`
/// ([`crate::call_graph::split`], for web-kovan, GitHub #736). The index
/// carries every review stamp's state, judged on the working tree by
/// [`crate::review_stamps::check`]; with no `review/stamps.toml` it is
/// empty. Files already in `dir` that the split does not name are left
/// alone.
pub fn run_split(
    root: &Path,
    crates: Option<Vec<String>>,
    dir: &Path,
    backend: &CallBackend,
) -> Result<(), String> {
    let doc = build_using(root, crates.as_deref(), backend)?;
    write_split(root, &doc, dir)
}

/// `kovan-cli call-graph --merge <files> [-o | --split-dir]`: merge
/// documents written by separate runs (one per crate, the incremental site
/// build, #745) with [`crate::call_graph::CallGraphDoc::merge`], then write
/// them as one document or split.
pub fn run_merge(root: &Path, files: &[PathBuf], out: Option<PathBuf>, split_dir: Option<PathBuf>) -> Result<(), String> {
    let mut docs = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(f).map_err(|e| format!("reading {}: {e}", f.display()))?;
        let d: crate::call_graph::CallGraphDoc =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?;
        let (lo, hi) = (crate::call_graph::OLDEST_READABLE_SCHEMA, crate::call_graph::SCHEMA_VERSION);
        if d.schema < lo || d.schema > hi {
            return Err(format!("{}: schema {}, expected {lo} to {hi}", f.display(), d.schema));
        }
        docs.push(d);
    }
    if crate::call_graph::CallGraphDoc::mixed_generators(&docs) {
        eprintln!("call-graph: WARNING: the merged documents were resolved by different backends or rust-analyzer versions; the first one's is recorded");
    }
    let doc = crate::call_graph::CallGraphDoc::merge(docs);
    match split_dir {
        Some(dir) => write_split(root, &doc, &dir),
        None => {
            let text = doc.to_json();
            match out {
                Some(path) => std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display())),
                None => {
                    print!("{text}");
                    Ok(())
                }
            }
        }
    }
}

/// Write `doc` split into `dir` (see [`run_split`]).
pub fn write_split(root: &Path, doc: &crate::call_graph::CallGraphDoc, dir: &Path) -> Result<(), String> {
    let stamps = stamp_states(root)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let files = crate::call_graph::split::split_files(doc, stamps);
    let total: usize = files.iter().map(|(_, t)| t.len()).sum();
    for (name, text) in &files {
        let path = dir.join(name);
        std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    eprintln!("call-graph: wrote {} files ({total} bytes) to {}", files.len(), dir.display());
    Ok(())
}

/// Every stamp in `review/stamps.toml`, judged on the working tree, in the
/// form the web reads. Artifact stamps and unchecked ones are left out
/// (the web shows functions only).
pub fn stamp_states(root: &Path) -> Result<Vec<crate::call_graph::split::StampState>, String> {
    use crate::call_graph::split::{StampState, StampVerdict};
    use crate::review_stamps::{check, Scope, Verdict, DEFAULT_REPO_URL};
    let report = check(root, &Scope::All)?;
    Ok(report
        .checked
        .iter()
        .filter_map(|c| {
            let function = c.stamp.function.clone()?;
            let (verdict, reason) = match &c.verdict {
                Verdict::Valid => (StampVerdict::Valid, String::new()),
                Verdict::Void(r) => (StampVerdict::Stale, r.to_string()),
                Verdict::Unchecked(_) => return None,
            };
            Some(StampState {
                function,
                verdict,
                reason,
                rung: c.stamp.rung,
                reviewer: c.stamp.reviewer.clone(),
                date: c.stamp.date.to_string(),
                note: c.stamp.note.clone(),
                permalink: c.stamp.permalink(DEFAULT_REPO_URL),
                state: None,
            })
        })
        .collect())
}

/// `kovan-cli call-graph-keys`: one line `<crate> <key>` per workspace
/// member (or `crates`), the cache key of its call-graph data in the
/// incremental site build (#745, maintainer 2026-10-06). The key is the
/// SHA-256 of:
///
/// - every file `cargo package --list` puts in the crate that exists on disk
///   (path and contents; files cargo generates, such as `Cargo.toml.orig`
///   and `.cargo_vcs_info.json`, are not on disk and do not count), so
///   whatever `exclude` leaves out cannot invalidate it;
/// - the keys of its workspace dependencies (normal and build, optional
///   included), so it is transitive: a change anywhere below a crate
///   re-indexes it, since its calls resolve into that code;
/// - `rust-analyzer --version` and the call graph's schema version.
///
/// Deterministic: same tree, same keys.
pub fn run_keys(root: &Path, crates: Option<Vec<String>>) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    #[derive(Deserialize)]
    struct Meta {
        packages: Vec<Pkg>,
    }
    #[derive(Deserialize)]
    struct Pkg {
        name: String,
        manifest_path: String,
        #[serde(default)]
        dependencies: Vec<Dep>,
    }
    #[derive(Deserialize)]
    struct Dep {
        name: String,
        #[serde(default)]
        kind: Option<String>,
    }
    let json = crate::code_map::run_cargo_metadata(root, true)?;
    let meta: Meta = serde_json::from_str(&json).map_err(|e| format!("cargo metadata: {e}"))?;
    let names: BTreeSet<String> = meta.packages.iter().map(|p| p.name.clone()).collect();
    let ra = rust_analyzer_version();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut own: BTreeMap<String, String> = BTreeMap::new();
    let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in &meta.packages {
        let dir = Path::new(&p.manifest_path).parent().unwrap_or(root).to_path_buf();
        let out = std::process::Command::new(&cargo)
            .current_dir(root)
            .args(["package", "--list", "--allow-dirty", "--offline", "-p", &p.name])
            .output()
            .map_err(|e| format!("cargo package --list -p {}: {e}", p.name))?;
        if !out.status.success() {
            return Err(format!("cargo package --list -p {}: {}", p.name, String::from_utf8_lossy(&out.stderr).trim()));
        }
        let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
        files.sort();
        let mut h = Sha256::new();
        for f in files {
            if let Ok(bytes) = std::fs::read(dir.join(&f)) {
                h.update(f.as_bytes());
                h.update([0]);
                h.update(Sha256::digest(&bytes));
            }
        }
        own.insert(p.name.clone(), format!("{:x}", h.finalize()));
        let mut d: Vec<String> = p
            .dependencies
            .iter()
            .filter(|d| d.kind.as_deref() != Some("dev") && names.contains(&d.name) && d.name != p.name)
            .map(|d| d.name.clone())
            .collect();
        d.sort();
        d.dedup();
        deps.insert(p.name.clone(), d);
    }
    fn key(
        name: &str,
        own: &BTreeMap<String, String>,
        deps: &BTreeMap<String, Vec<String>>,
        tail: &str,
        memo: &mut BTreeMap<String, String>,
    ) -> String {
        if let Some(k) = memo.get(name) {
            return k.clone();
        }
        let mut h = Sha256::new();
        h.update(own[name].as_bytes());
        for d in &deps[name] {
            h.update(key(d, own, deps, tail, memo).as_bytes());
        }
        h.update(tail.as_bytes());
        let k = format!("{:x}", h.finalize());
        memo.insert(name.to_string(), k.clone());
        k
    }
    let tail = format!("{ra}\nschema {}\nsplit {}", crate::call_graph::SCHEMA_VERSION, crate::call_graph::split::SPLIT_SCHEMA);
    let mut memo = BTreeMap::new();
    let wanted: Vec<String> = crates.unwrap_or_else(|| names.iter().cloned().collect());
    for c in wanted {
        if !names.contains(&c) {
            return Err(format!("{c}: not a workspace member"));
        }
        println!("{c} {}", key(&c, &own, &deps, &tail, &mut memo));
    }
    Ok(())
}

/// `--backend` on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BackendArg {
    Lsp,
    Scip,
}

/// `kovan-cli call-graph-diff <a> <b>`: prints the edge-by-edge comparison
/// ([`crate::call_graph::compare`]) as Markdown, listing up to `list`
/// differing edges of each kind.
pub fn run_diff(a: &Path, b: &Path, list: usize) -> Result<(), String> {
    use crate::call_graph::compare::{compare, Comparison};
    let load = |p: &Path| -> Result<CallGraphDoc, String> {
        let t = std::fs::read_to_string(p).map_err(|e| format!("reading {}: {e}", p.display()))?;
        serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
    };
    let (da, db) = (load(a)?, load(b)?);
    let c = compare(&da, &db);
    let gen = |d: &CallGraphDoc| match &d.generator {
        Some(g) => format!("{:?} ({})", g.backend, g.rust_analyzer),
        None => "not recorded (schema < 3)".into(),
    };
    println!("# Call-graph comparison\n");
    println!("- A: `{}`, {}", a.display(), gen(&da));
    println!("- B: `{}`, {}", b.display(), gen(&db));
    println!(
        "- functions: A {}, B {}, in both {}\n",
        c.functions_a, c.functions_b, c.functions_both
    );
    println!("| caller crate | both | same lines | kind differs | A only | B only | recall of A |");
    println!("|---|---|---|---|---|---|---|");
    let row = |name: &str, e: &crate::call_graph::compare::EdgeCounts| {
        println!(
            "| {name} | {} | {} | {} | {} | {} | {:.2} % |",
            e.both,
            e.same_lines,
            e.kind_differs,
            e.only_a,
            e.only_b,
            100.0 * e.recall_of_a()
        );
    };
    for (k, e) in &c.per_crate {
        row(k, e);
    }
    row("**total**", &c.total);
    println!(
        "\nJaccard (both / union): {:.2} %",
        100.0 * c.total.jaccard()
    );
    for (label, edges) in [("A only", &c.only_a), ("B only", &c.only_b)] {
        println!("\n## {label}: {} edges\n", edges.len());
        for (kind, n) in Comparison::only_by_kind(edges) {
            let selfs = edges.iter().filter(|e| e.kind == kind && e.is_self_edge()).count();
            println!("- {kind:?}: {n} ({selfs} self-edges)");
        }
        for e in edges.iter().take(list) {
            println!("  - `{}` -> `{}` ({:?}, lines {:?})", e.from, e.to, e.kind, e.lines);
        }
    }
    println!("\n## Unresolved calls (matched by function, line and kind)\n");
    println!("| kind | A | B | both |");
    println!("|---|---|---|---|");
    for (k, u) in &c.unresolved {
        println!("| {k} | {} | {} | {} |", u.a, u.b, u.both);
    }
    Ok(())
}

/// Splits `a,b , c` into crate names.
pub fn parse_crates(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the item start walks up over `///` and attribute lines
    /// and stops at code or a blank line.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn item_start_includes_docs_and_attributes() {
        let lines: Vec<String> = ["}", "", "/// Doc.", "#[inline]", "pub fn f() {", "}"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(start_of_item(&lines, 4), 2);
        assert_eq!(start_of_item(&lines, 0), 0);
    }

    /// Methodology: the crate list splits on commas and drops blanks.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn crate_list_parses() {
        assert_eq!(
            parse_crates("boon-lay, outram-park-digital-twin-engine,"),
            vec!["boon-lay", "outram-park-digital-twin-engine"]
        );
    }

    /// Methodology: functions of a small file get ids, kinds, test flags,
    /// lines and source text as the model documents.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn functions_of_a_file() {
        let text = "/// Free.\npub fn a() {\n    b();\n}\nfn b() {}\nstruct T;\nimpl std::fmt::Display for T {\n    fn fmt(&self) {}\n}\nimpl std::fmt::Debug for T {\n    fn fmt(&self) {}\n}\n#[test]\nfn t() {}\n";
        let idx = FileIndex::parse(text);
        let f = functions_of("x.rs", &idx, &[], false);
        let ids: Vec<&str> = f.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "x.rs::a",
                "x.rs::b",
                "x.rs::T::fmt#1",
                "x.rs::T::fmt#2",
                "x.rs::t"
            ]
        );
        assert_eq!((f[0].start_line, f[0].line, f[0].end_line), (1, 2, 4));
        assert_eq!(f[0].source, "/// Free.\npub fn a() {\n    b();\n}");
        assert_eq!(f[0].doc, "Free.");
        assert_eq!(f[2].kind, FnKind::TraitImpl);
        assert_eq!(f[2].trait_name.as_deref(), Some("Display"));
        assert!(f[4].test && !f[0].test);
    }
}
