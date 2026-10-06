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
//! 2. For the lib target and every example target, the module tree is read
//!    from the root file's `mod` declarations ([`crate::call_graph::modules`]),
//!    so a multi-file example such as `examples/htgr_sim_v1/` is a tree of
//!    modules too. Bins, tests and benches are not included.
//! 3. Every `fn` in those files is found by `code-walk`'s source scanner
//!    (`code_walk::source::FileIndex`), which supplies the qualified name,
//!    signature, doc sentence and body range.
//! 4. Each function's body is expanded by `code-walk`'s builder
//!    (`code_walk::builder::Workspace::expand`): every call-shaped token is
//!    sent to rust-analyzer (through the keep-warm `lsp_daemon`, one batched
//!    request per function) and becomes a resolved call, a call into std or
//!    a dependency (counted, dropped), or an `UNRESOLVED(<kind>)` gap. No
//!    call is guessed. Every call site is kept, not only the first per pair.
//!
//! Known limits are `code-walk`'s: trait-method calls stop at
//! `UNRESOLVED(trait)`; a call to a derived method (`Default::default()` on
//! a `#[derive(Default)]` type) is `UNRESOLVED(other)`; macro bodies are
//! not entered; a `fn` nested in a body is listed on its own and its calls
//! are also counted in the outer body; a function value written as a path
//! (`.map(other_crate::leaf)`) is not seen at all, only a bare name
//! (`.map(leaf)`) is (found 2026-10-06 while writing
//! `tests/call_graph_rust_analyzer.rs`).
//!
//! # Cost
//!
//! One rust-analyzer definition query per call-shaped token. Measured
//! 2026-10-06 (16-core desktop, rust-analyzer 1.98.0) on
//! `outram-park-digital-twin-engine` (lib and its 4 examples) plus
//! `boon-lay` (lib and 5 examples): 3632 functions, 37641 queries, 127 s
//! with a cold rust-analyzer (about 60 s of it indexing the workspace) and
//! 50 s warm; the two runs' JSON (10.5 MB) was byte-identical.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;

use super::code_walk::builder::Workspace;
use super::code_walk::graph::EdgeKind;
use super::code_walk::source::{Container, FileIndex};
use crate::call_graph::modules;
use crate::call_graph::{
    function_ids, CallGraphDoc, CallKind, CrateGraph, FnKind, Function, Module, OutsideFn, RawCall,
    Target, TargetKind, Unresolved,
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

/// Builds the call graph of `scope` (every member when `None`).
pub fn build(root: &Path, scope: Option<&[String]>) -> Result<CallGraphDoc, String> {
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

    let mut ws = Workspace::open(&root)?;
    let result = build_with(&mut ws, &all, &selected, map.as_ref(), started);
    ws.close();
    result
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
        for (kind, name, src) in &m.targets {
            let mut mods: Vec<Module> = Vec::new();
            // (file, module path, parent, test, mod-rs)
            let mut queue = vec![(src.clone(), String::new(), None::<String>, false, true)];
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
                let functions = functions_of(&file, &idx, &found.test_ranges, test);
                for f in &functions {
                    in_scope.insert((file.clone(), f.line), f.id.clone());
                }
                let maturity = match kind {
                    TargetKind::Lib => module_maturity(map, &m.name, &path),
                    TargetKind::Example => map.and_then(|mp| mp.get(&m.name)).map(|c| c.maturity),
                };
                mods.push(Module {
                    file,
                    path,
                    parent,
                    test,
                    maturity,
                    functions,
                });
            }
            targets.push(Target {
                kind: *kind,
                name: name.clone(),
                root: src.clone(),
                modules: mods,
            });
        }
        crates.push(CrateGraph {
            name: m.name.clone(),
            dir: m.dir.clone(),
            maturity: map.and_then(|mp| mp.get(&m.name)).map(|c| c.maturity),
            targets,
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
                _ => CallKind::Call,
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
    for c in &mut crates {
        for t in &mut c.targets {
            for m in &mut t.modules {
                for f in &mut m.functions {
                    if let Some(v) = gaps.remove(&f.id) {
                        f.unresolved = v;
                    }
                }
            }
        }
    }
    let doc = CallGraphDoc::assemble(crates, raw, outside, walk.external_calls);
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
        let test = module_test
            || modules::is_test_attr(&attrs)
            || test_ranges.iter().any(|&(a, b)| a <= d.line && d.line <= b);
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
        });
    }
    out
}

/// `kovan-cli call-graph`: build and write to `out`, or stdout.
pub fn run(root: &Path, crates: Option<Vec<String>>, out: Option<PathBuf>) -> Result<(), String> {
    let doc = build(root, crates.as_deref())?;
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
pub fn run_split(root: &Path, crates: Option<Vec<String>>, dir: &Path) -> Result<(), String> {
    let doc = build(root, crates.as_deref())?;
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
        if d.schema != crate::call_graph::SCHEMA_VERSION {
            return Err(format!("{}: schema {}, expected {}", f.display(), d.schema, crate::call_graph::SCHEMA_VERSION));
        }
        docs.push(d);
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

fn write_split(root: &Path, doc: &crate::call_graph::CallGraphDoc, dir: &Path) -> Result<(), String> {
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
    let ra = std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "rust-analyzer missing".into());
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
