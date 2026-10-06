//! The workspace **call graph** at three levels, crate → module (or example)
//! → function, with each function's source text, as deterministic data for
//! the code-review UI (GitHub #737, part of #735).
//!
//! `kovan-cli call-graph` builds it (`commands::call_graph`, which resolves
//! calls through rust-analyzer with `code-walk`'s machinery) and writes it as
//! JSON. web-kovan reads that JSON as a static file; desktop kovan can build
//! it live. This module is the data model and the pure assembly step: plain
//! `serde` + `std`, no GUI, no I/O, no rust-analyzer, so it can move to a
//! wasm-clean crate unchanged. ~~can move~~ **Moved 2026-10-06** to
//! `kovan_common::call_graph` (web-kovan, #736); `kovan::call_graph`
//! re-exports it. [`split`] cuts a document into one file per crate for the
//! web.
//!
//! # Shape
//!
//! ```text
//! CallGraphDoc
//!   crates[]         name, dir, maturity (the Cargo.toml tag)
//!     targets[]      the lib, then each example (kind, name, root file)
//!       modules[]    one per source file in the target's module tree
//!         functions[]  id, lines, signature, doc, source, unresolved calls
//!   calls[]          function -> function, every call-site line
//!   module_calls[]   module (file) -> module, call sites and function pairs
//!   crate_calls[]    crate -> crate, the same counts
//!   outside[]        call targets in workspace code outside the scope
//!   totals           counts of all of the above
//! ```
//!
//! # Function ids
//!
//! A function's `id` is `code-walk`'s form, `path/to/file.rs::name` for a
//! free function and `path/to/file.rs::Type::name` for a method (trait-impl
//! methods use the self type, trait declarations the trait name), so review
//! stamps (#739) and `code-walk --from` use the same key. When that form is
//! not unique in its file (two `impl`s of different traits both defining
//! `fmt` on one type, or a nested helper sharing a name), every one of them
//! gets `#k` appended, `k` counting from 1 in source order, and
//! `ambiguous: true`; [`function_ids`] is the rule.
//!
//! # Determinism
//!
//! [`CallGraphDoc::assemble`] sorts everything (crates by name, targets lib
//! first then examples by name, modules by file, functions by line, calls by
//! `(from, to, kind)` with their lines ascending, the aggregates by key) and
//! uses only `BTreeMap`s, so the same inputs in any order give
//! byte-identical JSON (test `assembly_is_order_independent`). Nothing
//! machine-specific (absolute paths, timings) is stored.

pub mod modules;
pub mod split;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Version of the JSON layout; bumped on any breaking change.
pub const SCHEMA_VERSION: u32 = 1;

/// The whole graph for one scope of crates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallGraphDoc {
    pub schema: u32,
    /// The crates analysed, sorted. Calls into other workspace crates are
    /// kept, their targets listed in `outside`.
    pub scope: Vec<String>,
    pub crates: Vec<CrateGraph>,
    pub calls: Vec<Call>,
    pub module_calls: Vec<AggregateCall>,
    pub crate_calls: Vec<AggregateCall>,
    pub outside: Vec<OutsideFn>,
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateGraph {
    pub name: String,
    /// The crate's folder relative to the workspace root, `/`-separated.
    pub dir: String,
    /// The crate's `[package.metadata.kovan]` maturity (0..=4), when tagged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maturity: Option<u8>,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Lib,
    Example,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub kind: TargetKind,
    /// The Cargo target name (`boon_lay`, `htgr_sim_v1`).
    pub name: String,
    /// The target's root file, workspace-relative.
    pub root: String,
    pub modules: Vec<Module>,
}

/// One source file of a target's module tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// Workspace-relative path; the module's id in `module_calls`.
    pub file: String,
    /// Path from the target root, `physics::fission_product_release`; empty
    /// for the root itself.
    pub path: String,
    /// The declaring module's file; `None` for the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Declared under `#[cfg(test)]` (itself or an ancestor).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
    /// From the crate tag: the deepest `maturity_modules` entry that is this
    /// module or an ancestor, else the crate's level. Library modules only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maturity: Option<u8>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FnKind {
    /// A free function (also a function nested in another's body).
    Free,
    /// In an inherent `impl Type`.
    Method,
    /// In `impl Trait for Type`.
    TraitImpl,
    /// A trait's own declaration (with or without a default body).
    TraitDecl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    /// `file.rs::name` or `file.rs::Type::name` (see the module doc), unique.
    pub id: String,
    /// Set when `id` needed a `#k` suffix to be unique.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ambiguous: bool,
    pub name: String,
    pub kind: FnKind,
    /// The self type (impls) or the trait (trait declarations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The trait of a trait impl.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "trait")]
    pub trait_name: Option<String>,
    /// A `#[test]` function, or inside `#[cfg(test)]` code.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
    /// 1-based first line: the start of the `///` doc comment and attributes
    /// directly above the declaration, else the declaration.
    pub start_line: u32,
    /// 1-based line of the `fn` identifier.
    pub line: u32,
    /// 1-based line of the closing brace (the declaration line when there is
    /// no body).
    pub end_line: u32,
    pub signature: String,
    /// The first sentence of the doc comment; empty when undocumented.
    pub doc: String,
    /// Lines `start_line..=end_line` as written, joined with `\n`.
    pub source: String,
    /// Every call in the body the tool could not follow, by line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved: Vec<Unresolved>,
}

/// A call `code-walk` marks `UNRESOLVED(<kind>)`: never guessed, never
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Unresolved {
    /// 1-based call-site line.
    pub line: u32,
    /// `trait`, `closure`, `macro`, `no-definition` or `other`.
    pub kind: String,
    /// The token at the call site.
    pub callee: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    /// A call expression rust-analyzer resolved.
    Call,
    /// A function passed by value (`.map(f)`), resolved by rust-analyzer.
    FnValue,
}

/// `from` calls `to` at each of `lines` (1-based, in `from`'s file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub from: String,
    pub to: String,
    pub kind: CallKind,
    pub lines: Vec<u32>,
}

/// Calls between two modules (by file) or two crates (by name).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateCall {
    pub from: String,
    pub to: String,
    /// Call sites.
    pub sites: usize,
    /// Distinct `(caller, callee)` function pairs.
    pub pairs: usize,
}

/// A workspace function outside the scope that a function in scope calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutsideFn {
    pub id: String,
    /// The workspace crate whose folder holds `file`, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub krate: Option<String>,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub crates: usize,
    pub targets: usize,
    pub modules: usize,
    pub functions: usize,
    /// Distinct caller-callee edges.
    pub calls: usize,
    pub call_sites: usize,
    pub unresolved: usize,
    pub unresolved_by_kind: BTreeMap<String, usize>,
    pub outside: usize,
    /// Calls into std or third-party dependencies, filtered out.
    pub external_calls: usize,
}

/// Call sites and distinct `(caller, callee)` pairs per `(from, to)` key.
type Tally = BTreeMap<(String, String), (usize, BTreeSet<(String, String)>)>;

/// One resolved call site, as the builder finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCall {
    pub from: String,
    pub to: String,
    pub kind: CallKind,
    pub line: u32,
}

/// Unique ids for the functions of one file, given each one's
/// `code-walk` qualified name (`name` or `Type::name`) in source order.
pub fn function_ids(file: &str, quals: &[String]) -> Vec<(String, bool)> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for q in quals {
        *count.entry(q.as_str()).or_default() += 1;
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    quals
        .iter()
        .map(|q| {
            if count[q.as_str()] == 1 {
                (format!("{file}::{q}"), false)
            } else {
                let k = seen.entry(q.as_str()).or_default();
                *k += 1;
                (format!("{file}::{q}#{k}"), true)
            }
        })
        .collect()
}

impl CallGraphDoc {
    /// Sorts and aggregates the builder's output into the document.
    /// `external_calls` is the count of calls into std and dependencies.
    pub fn assemble(
        mut crates: Vec<CrateGraph>,
        raw_calls: Vec<RawCall>,
        outside: Vec<OutsideFn>,
        external_calls: usize,
    ) -> CallGraphDoc {
        crates.sort_by(|a, b| a.name.cmp(&b.name));
        for c in &mut crates {
            c.targets
                .sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
            for t in &mut c.targets {
                t.modules.sort_by(|a, b| a.file.cmp(&b.file));
                for m in &mut t.modules {
                    m.functions
                        .sort_by(|a, b| (a.line, &a.id).cmp(&(b.line, &b.id)));
                    for f in &mut m.functions {
                        f.unresolved.sort();
                        f.unresolved.dedup();
                    }
                }
            }
        }
        // Where each in-scope function lives.
        let mut home: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
        for c in &crates {
            for t in &c.targets {
                for m in &t.modules {
                    for f in &m.functions {
                        home.insert(f.id.as_str(), (m.file.as_str(), c.name.as_str()));
                    }
                }
            }
        }
        let mut outside_map: BTreeMap<String, OutsideFn> = BTreeMap::new();
        for o in outside {
            outside_map.entry(o.id.clone()).or_insert(o);
        }

        let mut grouped: BTreeMap<(String, String, CallKind), BTreeSet<u32>> = BTreeMap::new();
        for r in raw_calls {
            grouped
                .entry((r.from, r.to, r.kind))
                .or_default()
                .insert(r.line);
        }
        let calls: Vec<Call> = grouped
            .into_iter()
            .map(|((from, to, kind), lines)| Call {
                from,
                to,
                kind,
                lines: lines.into_iter().collect(),
            })
            .collect();

        let mut by_module: Tally = BTreeMap::new();
        let mut by_crate: Tally = BTreeMap::new();
        for c in &calls {
            let Some(&(fm, fc)) = home.get(c.from.as_str()) else {
                continue;
            };
            let (tm, tc) = match home.get(c.to.as_str()) {
                Some(&(m, k)) => (Some(m.to_string()), Some(k.to_string())),
                None => (None, outside_map.get(&c.to).and_then(|o| o.krate.clone())),
            };
            let pair = (c.from.clone(), c.to.clone());
            if let Some(tm) = tm {
                let e = by_module.entry((fm.to_string(), tm)).or_default();
                e.0 += c.lines.len();
                e.1.insert(pair.clone());
            }
            if let Some(tc) = tc {
                let e = by_crate.entry((fc.to_string(), tc)).or_default();
                e.0 += c.lines.len();
                e.1.insert(pair);
            }
        }
        let agg = |m: Tally| {
            m.into_iter()
                .map(|((from, to), (sites, pairs))| AggregateCall {
                    from,
                    to,
                    sites,
                    pairs: pairs.len(),
                })
                .collect::<Vec<_>>()
        };

        let mut totals = Totals {
            crates: crates.len(),
            external_calls,
            ..Totals::default()
        };
        for c in &crates {
            totals.targets += c.targets.len();
            for t in &c.targets {
                totals.modules += t.modules.len();
                for m in &t.modules {
                    totals.functions += m.functions.len();
                    for f in &m.functions {
                        totals.unresolved += f.unresolved.len();
                        for u in &f.unresolved {
                            *totals.unresolved_by_kind.entry(u.kind.clone()).or_default() += 1;
                        }
                    }
                }
            }
        }
        totals.calls = calls.len();
        totals.call_sites = calls.iter().map(|c| c.lines.len()).sum();
        let outside: Vec<OutsideFn> = outside_map.into_values().collect();
        totals.outside = outside.len();
        CallGraphDoc {
            schema: SCHEMA_VERSION,
            scope: crates.iter().map(|c| c.name.clone()).collect(),
            crates,
            calls,
            module_calls: agg(by_module),
            crate_calls: agg(by_crate),
            outside,
            totals,
        }
    }

    /// One document from several built separately (one per crate, for the
    /// incremental site build, #745): every crate once (the first copy
    /// wins), every call site, the outside functions that are still outside
    /// the merged scope, the external-call counts added, assembled again.
    ///
    /// Each function's calls are found from its own body alone, so a crate
    /// built on its own gives the same calls as in a whole-workspace run;
    /// what changes with the scope is only which callees count as
    /// `outside`, and that is recomputed here. So merging single-crate
    /// documents gives the bytes a run over all of them gives (test
    /// `merging_single_crate_documents_equals_one_run`, and the script check
    /// in `crates/kovan-web/web/data.sh --check`).
    pub fn merge(docs: Vec<CallGraphDoc>) -> CallGraphDoc {
        let mut crates: Vec<CrateGraph> = Vec::new();
        let mut raw = Vec::new();
        let mut outside = Vec::new();
        let mut external = 0;
        for d in docs {
            external += d.totals.external_calls;
            for c in d.crates {
                if !crates.iter().any(|k| k.name == c.name) {
                    crates.push(c);
                }
            }
            for c in d.calls {
                for line in c.lines {
                    raw.push(RawCall { from: c.from.clone(), to: c.to.clone(), kind: c.kind, line });
                }
            }
            outside.extend(d.outside);
        }
        let in_scope: BTreeSet<String> = crates
            .iter()
            .flat_map(|c| c.targets.iter())
            .flat_map(|t| t.modules.iter())
            .flat_map(|m| m.functions.iter().map(|f| f.id.clone()))
            .collect();
        outside.retain(|o| !in_scope.contains(&o.id));
        CallGraphDoc::assemble(crates, raw, outside, external)
    }

    /// Pretty JSON with a trailing newline: the file `kovan-cli call-graph`
    /// writes.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("call graph serialises");
        s.push('\n');
        s
    }

    /// Every function, with the crate and module it is in.
    pub fn functions(&self) -> impl Iterator<Item = (&CrateGraph, &Module, &Function)> {
        self.crates.iter().flat_map(|c| {
            c.targets.iter().flat_map(move |t| {
                t.modules
                    .iter()
                    .flat_map(move |m| m.functions.iter().map(move |f| (c, m, f)))
            })
        })
    }

    /// The function with this id, if it is in scope.
    pub fn function(&self, id: &str) -> Option<&Function> {
        self.functions().map(|(_, _, f)| f).find(|f| f.id == id)
    }

    /// The calls out of `id`, in `(to, kind)` order.
    pub fn calls_from<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Call> + 'a {
        self.calls.iter().filter(move |c| c.from == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn func(file: &str, name: &str, line: u32, unresolved: Vec<Unresolved>) -> Function {
        Function {
            id: format!("{file}::{name}"),
            ambiguous: false,
            name: name.into(),
            kind: FnKind::Free,
            owner: None,
            trait_name: None,
            test: false,
            start_line: line,
            line,
            end_line: line + 2,
            signature: format!("fn {name}()"),
            doc: String::new(),
            source: format!("fn {name}() {{\n}}"),
            unresolved,
        }
    }

    fn module(file: &str, path: &str, functions: Vec<Function>) -> Module {
        Module {
            file: file.into(),
            path: path.into(),
            parent: None,
            test: false,
            maturity: Some(1),
            functions,
        }
    }

    /// A two-crate fixture: `app` (lib with two modules, one example) calls
    /// into `core`; one call leaves the scope.
    pub(crate) fn fixture() -> (Vec<CrateGraph>, Vec<RawCall>, Vec<OutsideFn>) {
        let gap = Unresolved {
            line: 4,
            kind: "trait".into(),
            callee: "measure".into(),
            detail: "trait method".into(),
        };
        let app = CrateGraph {
            name: "app".into(),
            dir: "crates/app".into(),
            maturity: Some(1),
            targets: vec![
                Target {
                    kind: TargetKind::Example,
                    name: "demo".into(),
                    root: "crates/app/examples/demo.rs".into(),
                    modules: vec![module(
                        "crates/app/examples/demo.rs",
                        "",
                        vec![func("crates/app/examples/demo.rs", "main", 1, vec![])],
                    )],
                },
                Target {
                    kind: TargetKind::Lib,
                    name: "app".into(),
                    root: "crates/app/src/lib.rs".into(),
                    modules: vec![
                        module(
                            "crates/app/src/run.rs",
                            "run",
                            vec![
                                func("crates/app/src/run.rs", "step", 9, vec![gap.clone(), gap]),
                                func("crates/app/src/run.rs", "go", 2, vec![]),
                            ],
                        ),
                        module("crates/app/src/lib.rs", "", vec![]),
                    ],
                },
            ],
        };
        let core = CrateGraph {
            name: "core".into(),
            dir: "crates/core".into(),
            maturity: None,
            targets: vec![Target {
                kind: TargetKind::Lib,
                name: "core".into(),
                root: "crates/core/src/lib.rs".into(),
                modules: vec![module(
                    "crates/core/src/lib.rs",
                    "",
                    vec![func("crates/core/src/lib.rs", "leaf", 3, vec![])],
                )],
            }],
        };
        let c = |from: &str, to: &str, line: u32| RawCall {
            from: from.into(),
            to: to.into(),
            kind: CallKind::Call,
            line,
        };
        let calls = vec![
            c(
                "crates/app/src/run.rs::go",
                "crates/core/src/lib.rs::leaf",
                5,
            ),
            c(
                "crates/app/examples/demo.rs::main",
                "crates/app/src/run.rs::go",
                2,
            ),
            c(
                "crates/app/src/run.rs::go",
                "crates/core/src/lib.rs::leaf",
                3,
            ),
            c(
                "crates/app/src/run.rs::go",
                "crates/core/src/lib.rs::leaf",
                5,
            ),
            c(
                "crates/app/src/run.rs::step",
                "crates/util/src/lib.rs::help",
                10,
            ),
            c(
                "crates/app/src/run.rs::step",
                "crates/app/src/run.rs::go",
                11,
            ),
        ];
        let outside = vec![OutsideFn {
            id: "crates/util/src/lib.rs::help".into(),
            krate: Some("util".into()),
            file: "crates/util/src/lib.rs".into(),
            line: 1,
        }];
        (vec![app, core], calls, outside)
    }

    /// Methodology: assemble the fixture as given and again with every input
    /// list reversed (crates, targets, modules, functions, calls). Both must
    /// serialise to byte-identical JSON, and the aggregates must count call
    /// sites (duplicated sites merged) and distinct function pairs.
    ///
    /// Result (2026-10-06): passes; `go -> leaf` has lines [3, 5], the
    /// module edge `run.rs -> core lib.rs` 2 sites / 1 pair, the crate edge
    /// `app -> util` 1 site, and the duplicate trait gap is merged.
    #[test]
    fn assembly_is_order_independent() {
        let (crates, calls, outside) = fixture();
        let a = CallGraphDoc::assemble(crates.clone(), calls.clone(), outside.clone(), 7).to_json();
        let mut crates_r = crates;
        crates_r.reverse();
        for c in &mut crates_r {
            c.targets.reverse();
            for t in &mut c.targets {
                t.modules.reverse();
                for m in &mut t.modules {
                    m.functions.reverse();
                }
            }
        }
        let mut calls_r = calls;
        calls_r.reverse();
        let b = CallGraphDoc::assemble(crates_r, calls_r, outside, 7).to_json();
        assert_eq!(a, b);

        let doc: CallGraphDoc = serde_json::from_str(&a).unwrap();
        assert_eq!(doc.scope, vec!["app", "core"]);
        assert_eq!(doc.crates[0].targets[0].kind, TargetKind::Lib);
        let go = doc
            .calls_from("crates/app/src/run.rs::go")
            .collect::<Vec<_>>();
        assert_eq!(go.len(), 1);
        assert_eq!(go[0].lines, vec![3, 5]);
        let m = doc
            .module_calls
            .iter()
            .find(|m| m.from == "crates/app/src/run.rs" && m.to == "crates/core/src/lib.rs")
            .unwrap();
        assert_eq!((m.sites, m.pairs), (2, 1));
        let util = doc.crate_calls.iter().find(|c| c.to == "util").unwrap();
        assert_eq!((util.from.as_str(), util.sites), ("app", 1));
        assert_eq!(doc.totals.unresolved, 1);
        assert_eq!(doc.totals.functions, 4);
        assert_eq!(doc.totals.call_sites, 5);
        assert_eq!(doc.totals.external_calls, 7);
        let fns: Vec<&str> = doc.crates[0].targets[0].modules[1]
            .functions
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(fns, vec!["go", "step"]);
    }

    /// Methodology: assemble the two-crate fixture in one run, then as two
    /// single-crate runs would see it (each crate's own calls; calls into
    /// the other crate listed as `outside`), and merge those.
    ///
    /// Result (2026-10-06): byte-identical JSON.
    #[test]
    fn merging_single_crate_documents_equals_one_run() {
        let (crates, calls, outside) = fixture();
        let full = CallGraphDoc::assemble(crates.clone(), calls.clone(), outside.clone(), 7).to_json();
        let single = |name: &str, external: usize| {
            let c: Vec<CrateGraph> = crates.iter().filter(|c| c.name == name).cloned().collect();
            let prefix = format!("crates/{name}/");
            let mine: Vec<RawCall> = calls.iter().filter(|r| r.from.starts_with(&prefix)).cloned().collect();
            let mut out: Vec<OutsideFn> = outside.clone();
            for r in &mine {
                if !r.to.starts_with(&prefix) && !out.iter().any(|o| o.id == r.to) {
                    out.push(OutsideFn { id: r.to.clone(), krate: None, file: r.to.split("::").next().unwrap().into(), line: 1 });
                }
            }
            CallGraphDoc::assemble(c, mine, out, external)
        };
        let merged = CallGraphDoc::merge(vec![single("core", 0), single("app", 7)]).to_json();
        assert_eq!(merged, full);
    }

    /// Methodology: a file with a unique free function and two methods that
    /// share the code-walk form `Type::fmt`.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn ambiguous_ids_get_a_counter() {
        let ids = function_ids("a.rs", &["new".into(), "T::fmt".into(), "T::fmt".into()]);
        assert_eq!(
            ids,
            vec![
                ("a.rs::new".to_string(), false),
                ("a.rs::T::fmt#1".to_string(), true),
                ("a.rs::T::fmt#2".to_string(), true),
            ]
        );
    }
}
