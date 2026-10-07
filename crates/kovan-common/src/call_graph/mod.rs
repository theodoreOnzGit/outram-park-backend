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
//! # Schema 2 (2026-10-06, GitHub #746): additions only
//!
//! Every schema-1 field keeps its name, type and meaning; a reader that
//! ignores unknown fields reads schema 2 unchanged. Added (all omitted from
//! the JSON when empty):
//!
//! ```text
//! CallGraphDoc.commit            HEAD the data was built at
//! CallGraphDoc.site_base         base URL of Citation.site paths
//! CrateGraph.tests[]             integration-test targets (tests/*.rs),
//!                                Target with kind "test"; same shape
//! Module.upstream                {style, line, project, repository, version,
//!                                 commit, source, files[], licence, url}
//!                                from the attribution header; url only when
//!                                repository (github/gitlab) and commit are
//!                                both recorded: see [`upstream`]
//! Module.upstream_unparsed       a provenance marker that could not be read
//! Module.history[]               {sha, date, author, subject}, newest first,
//!                                at most 10: see [`history`]
//! Module.concepts[]              `//! kovan-concept:` tags
//! Function.test_fn               carries #[test] (an entry point)
//! Function.reached_by            {tests_total, tests[{hops, id}],
//!                                 examples_total,
//!                                 examples[{hops, crate, example, via}]},
//!                                nearest 10 of each; resolved calls only,
//!                                so a LOWER BOUND: see [`reach`]
//! Function.cited_by[]            {kind: code_walk|concept_tag|relation,
//!                                 page, line, anchor, site}: see [`citations`]
//! Totals.{test_targets, test_fns, non_test_functions, reached_by_tests,
//!   reached_by_examples, upstream_files, upstream_links, upstream_unparsed,
//!   cited_functions, citations, history_files}
//! ```
//!
//! # Schema 3 (2026-10-07, GitHub #757): additions only
//!
//! ```text
//! Call.kind "operator"           a call through an operator (`a + b`,
//!                                `v[i]`, `-x`) to a workspace impl of the
//!                                operator trait; only the SCIP backend
//!                                finds these
//! CallGraphDoc.generator         {backend: lsp|scip, rust_analyzer}: how the
//!                                calls were resolved, and the rust-analyzer
//!                                that resolved them (SCIP's output is not
//!                                stable across versions)
//! ```
//!
//! A schema-2 document reads unchanged (test
//! `schema_2_documents_still_load`, on a document written by the schema-2
//! code); a schema-2 *reader* meets an unknown `"operator"` kind, which is
//! why the version moved.
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

pub mod citations;
pub mod compare;
pub mod history;
pub mod modules;
pub mod reach;
pub mod split;
pub mod upstream;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Version of the JSON layout; bumped on any breaking change.
/// Schema 2 (2026-10-06, #746) only adds fields; a schema-1 reader that
/// ignores unknown fields reads it unchanged. Schema 3 (2026-10-07, #757)
/// adds the `operator` call kind and `generator`; schema-2 documents still
/// load ([`OLDEST_READABLE_SCHEMA`]).
pub const SCHEMA_VERSION: u32 = 3;

/// The oldest schema this model still reads: every version since only added
/// fields and values.
pub const OLDEST_READABLE_SCHEMA: u32 = 1;

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
    /// Schema 2: the commit (`git rev-parse HEAD`) the data was built at;
    /// absent outside a git checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Schema 2: the base URL that `Citation::site` paths are relative to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_base: Option<String>,
    /// Schema 3: how the calls were resolved, and by which rust-analyzer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<Generator>,
}

/// Schema 3 (#757): what resolved the calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generator {
    pub backend: Backend,
    /// The rust-analyzer that resolved them: `rust-analyzer --version` for
    /// the LSP backend, the index's own tool name and version for SCIP.
    pub rust_analyzer: String,
}

/// Where call targets come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// rust-analyzer's LSP, one definition query per call-shaped token.
    Lsp,
    /// One `rust-analyzer scip` index of the workspace (#757).
    Scip,
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
    /// Schema 2: the integration-test targets (`tests/*.rs`, kind `test`),
    /// kept apart from `targets` so a schema-1 reader is unaffected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<Target>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Lib,
    Example,
    /// Schema 2: an integration-test target; only in `CrateGraph::tests`.
    Test,
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
    /// Schema 2: the upstream counterpart from the file's attribution
    /// header ([`upstream`]); absent when the file has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<upstream::Upstream>,
    /// Schema 2: a provenance marker was seen but could not be parsed (the
    /// reason and the line); no `upstream` is guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_unparsed: Option<String>,
    /// Schema 2: the newest commits that touched the file ([`history`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<history::CommitRef>,
    /// Schema 2: `//! kovan-concept:` tags in the file ([`citations`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<String>,
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
    /// Schema 2: carries a test attribute (`#[test]`, `#[tokio::test]`,
    /// `#[rstest]`): an entry point for [`reach`]. `test` is also set.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test_fn: bool,
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
    /// Schema 2: the tests and examples that reach this (non-test) function
    /// through resolved calls, nearest first; a lower bound ([`reach`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reached_by: Option<reach::Reach>,
    /// Schema 2: the pages (and concept tags) that cite this function
    /// ([`citations`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited_by: Vec<citations::Citation>,
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
    /// Schema 3 (#757): an operator (`a + b`, `v[i]`, `-x`) that resolves
    /// to a workspace `impl` of the operator trait (SCIP backend only).
    Operator,
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
    /// Schema 2: integration-test targets.
    #[serde(default)]
    pub test_targets: usize,
    /// Schema 2: functions carrying a test attribute.
    #[serde(default)]
    pub test_fns: usize,
    /// Schema 2: non-test functions reached by at least one test.
    #[serde(default)]
    pub reached_by_tests: usize,
    /// Schema 2: non-test functions reached by at least one example.
    #[serde(default)]
    pub reached_by_examples: usize,
    /// Schema 2: non-test functions.
    #[serde(default)]
    pub non_test_functions: usize,
    /// Schema 2: modules with a parsed `upstream`.
    #[serde(default)]
    pub upstream_files: usize,
    /// Schema 2: of those, the ones with a `url`.
    #[serde(default)]
    pub upstream_links: usize,
    /// Schema 2: modules with `upstream_unparsed`.
    #[serde(default)]
    pub upstream_unparsed: usize,
    /// Schema 2: functions with at least one citation, and all citations.
    #[serde(default)]
    pub cited_functions: usize,
    #[serde(default)]
    pub citations: usize,
    /// Schema 2: modules with history.
    #[serde(default)]
    pub history_files: usize,
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
            c.tests.sort_by(|a, b| a.name.cmp(&b.name));
            for t in c.targets.iter_mut().chain(c.tests.iter_mut()) {
                t.modules.sort_by(|a, b| a.file.cmp(&b.file));
                for m in &mut t.modules {
                    m.functions
                        .sort_by(|a, b| (a.line, &a.id).cmp(&(b.line, &b.id)));
                    for f in &mut m.functions {
                        f.unresolved.sort();
                        f.unresolved.dedup();
                        f.cited_by.sort();
                        f.cited_by.dedup();
                    }
                    m.concepts.sort();
                    m.concepts.dedup();
                }
            }
        }
        // Where each in-scope function lives.
        let mut home: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
        for c in &crates {
            for t in c.targets.iter().chain(c.tests.iter()) {
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
            totals.test_targets += c.tests.len();
            for t in c.targets.iter().chain(c.tests.iter()) {
                totals.modules += t.modules.len();
                for m in &t.modules {
                    totals.functions += m.functions.len();
                    if let Some(u) = &m.upstream {
                        totals.upstream_files += 1;
                        totals.upstream_links += usize::from(u.url.is_some());
                    }
                    totals.upstream_unparsed += usize::from(m.upstream_unparsed.is_some());
                    totals.history_files += usize::from(!m.history.is_empty());
                    for f in &m.functions {
                        totals.test_fns += usize::from(f.test_fn);
                        totals.non_test_functions += usize::from(!f.test);
                        totals.cited_functions += usize::from(!f.cited_by.is_empty());
                        totals.citations += f.cited_by.len();
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
        let mut doc = CallGraphDoc {
            schema: SCHEMA_VERSION,
            scope: crates.iter().map(|c| c.name.clone()).collect(),
            crates,
            calls,
            module_calls: agg(by_module),
            crate_calls: agg(by_crate),
            outside,
            totals,
            commit: None,
            site_base: None,
            generator: None,
        };
        reach::fill(&mut doc);
        let (mut by_tests, mut by_examples) = (0, 0);
        for (_, _, f) in doc.functions() {
            if let Some(r) = &f.reached_by {
                by_tests += usize::from(r.tests_total > 0);
                by_examples += usize::from(r.examples_total > 0);
            }
        }
        doc.totals.reached_by_tests = by_tests;
        doc.totals.reached_by_examples = by_examples;
        doc
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
        // Schema 2: every run records the same HEAD and site base.
        let commit = docs.iter().find_map(|d| d.commit.clone());
        let site_base = docs.iter().find_map(|d| d.site_base.clone());
        // Schema 3: the first generator; [`CallGraphDoc::mixed_generators`]
        // tells a caller when the documents disagree.
        let generator = docs.iter().find_map(|d| d.generator.clone());
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
            .flat_map(|c| c.targets.iter().chain(c.tests.iter()))
            .flat_map(|t| t.modules.iter())
            .flat_map(|m| m.functions.iter().map(|f| f.id.clone()))
            .collect();
        outside.retain(|o| !in_scope.contains(&o.id));
        // `assemble` recomputes `reached_by` over the merged calls, so a test
        // reaching into another crate counts as in one run.
        let mut doc = CallGraphDoc::assemble(crates, raw, outside, external);
        doc.commit = commit;
        doc.site_base = site_base;
        doc.generator = generator;
        doc
    }

    /// True when `docs` were resolved by different backends or
    /// rust-analyzer versions, so a [`CallGraphDoc::merge`] of them would
    /// record only the first.
    pub fn mixed_generators(docs: &[CallGraphDoc]) -> bool {
        let mut g = docs.iter().map(|d| &d.generator);
        match g.next() {
            Some(first) => g.any(|x| x != first),
            None => false,
        }
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
            c.targets.iter().chain(c.tests.iter()).flat_map(move |t| {
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
            test_fn: false,
            reached_by: None,
            cited_by: Vec::new(),
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
            upstream: None,
            upstream_unparsed: None,
            history: Vec::new(),
            concepts: Vec::new(),
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
            tests: Vec::new(),
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
            tests: Vec::new(),
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

    /// Methodology: the fixture plus an integration-test target whose
    /// `#[test]` `t_step` calls `run.rs::step` (which calls `go`, which calls
    /// `core::leaf`), and 12 more tests calling `go` directly. `leaf` must be
    /// reached by `t_step` at 3 hops and by the `demo` example at 2 hops via
    /// its `main`; `go` must list 10 of its 13 tests, nearest first, with
    /// the total; test functions themselves get no `reached_by`.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn tests_and_examples_reach_functions() {
        let (mut crates, mut calls, outside) = fixture();
        let file = "crates/app/tests/it.rs";
        let mut fns = vec![func(file, "t_step", 1, vec![])];
        for k in 0..12 {
            fns.push(func(file, &format!("t{k:02}"), 10 + k, vec![]));
        }
        for f in &mut fns {
            f.test = true;
            f.test_fn = true;
        }
        crates[0].tests.push(Target {
            kind: TargetKind::Test,
            name: "it".into(),
            root: file.into(),
            modules: vec![module(file, "", fns)],
        });
        let c = |from: String, to: &str| RawCall {
            from,
            to: to.into(),
            kind: CallKind::Call,
            line: 1,
        };
        calls.push(c(format!("{file}::t_step"), "crates/app/src/run.rs::step"));
        for k in 0..12 {
            calls.push(c(format!("{file}::t{k:02}"), "crates/app/src/run.rs::go"));
        }
        let doc = CallGraphDoc::assemble(crates, calls, outside, 0);
        let leaf = doc.function("crates/core/src/lib.rs::leaf").unwrap();
        let r = leaf.reached_by.as_ref().unwrap();
        assert_eq!(r.tests_total, 13);
        assert_eq!(r.tests[0].hops, 2);
        let t_step = r.tests.iter().find(|t| t.id.ends_with("::t_step"));
        assert!(t_step.is_none(), "t_step is 3 hops, beyond the nearest 10");
        assert_eq!(r.examples_total, 1);
        assert_eq!(
            (
                r.examples[0].hops,
                r.examples[0].example.as_str(),
                r.examples[0].via.as_str()
            ),
            (2, "demo", "crates/app/examples/demo.rs::main")
        );
        let go = doc.function("crates/app/src/run.rs::go").unwrap();
        let r = go.reached_by.as_ref().unwrap();
        assert_eq!((r.tests_total, r.tests.len()), (13, 10));
        assert_eq!(
            r.tests[0],
            reach::TestReach {
                hops: 1,
                id: format!("{file}::t00")
            }
        );
        let step = doc.function("crates/app/src/run.rs::step").unwrap();
        let r = step.reached_by.as_ref().unwrap();
        assert_eq!(
            r.tests,
            vec![reach::TestReach {
                hops: 1,
                id: format!("{file}::t_step")
            }]
        );
        assert_eq!(r.examples_total, 0);
        assert!(doc
            .function(&format!("{file}::t00"))
            .unwrap()
            .reached_by
            .is_none());
        assert_eq!(doc.totals.test_targets, 1);
        assert_eq!(doc.totals.test_fns, 13);
        assert_eq!(doc.totals.reached_by_tests, 3);
        // Determinism with the new data.
        assert_eq!(
            doc.to_json(),
            serde_json::from_str::<CallGraphDoc>(&doc.to_json())
                .unwrap()
                .to_json()
        );
    }

    /// Methodology: `tests/data/call_graph_schema2_bishan.json` is a real
    /// document written by the schema-2 code (`kovan-cli call-graph --crates
    /// bishan` at commit c73539fa75, 2026-10-07, before #757). It must load
    /// into the schema-3 model with every field intact (re-serialising it
    /// gives the same JSON except `schema`, which the file states as 2), and
    /// merging it alone must give the same calls.
    ///
    /// Result (2026-10-07): passes; 6 functions, 6 calls, 2 unresolved.
    #[test]
    fn schema_2_documents_still_load() {
        let text = include_str!("../../tests/data/call_graph_schema2_bishan.json");
        let doc: CallGraphDoc = serde_json::from_str(text).expect("schema-2 document loads");
        assert_eq!(doc.schema, 2);
        assert!(doc.schema >= OLDEST_READABLE_SCHEMA && doc.schema < SCHEMA_VERSION);
        assert!(doc.generator.is_none());
        assert_eq!((doc.totals.functions, doc.totals.calls, doc.totals.unresolved), (6, 6, 2));
        assert_eq!(doc.to_json(), text, "nothing lost or added on a round trip");
        let merged = CallGraphDoc::merge(vec![doc.clone()]);
        assert_eq!(merged.calls, doc.calls);
        assert_eq!(merged.schema, SCHEMA_VERSION);
    }

    /// Methodology: an `operator` call and a `generator` survive a JSON
    /// round trip, and documents from two backends are flagged as mixed.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn operator_calls_and_generator_round_trip() {
        let (crates, mut calls, outside) = fixture();
        calls.push(RawCall {
            from: "crates/app/src/run.rs::go".into(),
            to: "crates/core/src/lib.rs::leaf".into(),
            kind: CallKind::Operator,
            line: 4,
        });
        let mut doc = CallGraphDoc::assemble(crates, calls, outside, 0);
        doc.generator = Some(Generator {
            backend: Backend::Scip,
            rust_analyzer: "rust-analyzer 1.98.0".into(),
        });
        let json = doc.to_json();
        assert!(json.contains("\"kind\": \"operator\""));
        assert!(json.contains("\"backend\": \"scip\""));
        let back: CallGraphDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(back, doc);
        let mut lsp = doc.clone();
        lsp.generator = Some(Generator {
            backend: Backend::Lsp,
            rust_analyzer: "rust-analyzer 1.98.0".into(),
        });
        assert!(CallGraphDoc::mixed_generators(&[doc.clone(), lsp]));
        assert!(!CallGraphDoc::mixed_generators(&[doc.clone(), doc]));
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
