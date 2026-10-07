//! Fixture tests for [`super`]. The fixtures under
//! `tests/fixtures/test_evidence/` are **captured** `cargo test` output
//! (cargo 1.98.0, 2026-10-07), with the absolute paths replaced by `/WS`:
//!
//! - `evfix_full.txt`: a throwaway crate `evfix` (lib with a passing, an
//!   ignored, a `should_panic`, and a failing test, a file module
//!   `geom::tests` with a nested inline module; `tests/integration.rs`; and
//!   `tests/crash.rs`, whose second test aborted the process, so that binary
//!   printed one result and no summary). Command:
//!   `cargo test --release -j 1 --lib --tests --no-fail-fast --message-format=json-render-diagnostics`.
//! - `evfix_filtered.txt`: the same with `-- area` (a filter).
//! - `bishan_partial.txt`: `cargo test --release -j 1 -p bishan --lib --tests
//!   --no-fail-fast --message-format=json-render-diagnostics` in this
//!   workspace (registry build messages dropped).

use super::args::plan;
use super::map::{restrict, to_test_run, Mapped, TestIdMap};
use super::output::{LineKind, OutputParser};
use super::verdict::{reach_verdict, ReachVerdict};
use super::*;
use crate::call_graph::{CallGraphDoc, CrateGraph, FnKind, Function, Module, Target, TargetKind};
use crate::review::index::FolderIndex;

const FULL: &str = include_str!("../../../tests/fixtures/test_evidence/evfix_full.txt");
const FILTERED: &str = include_str!("../../../tests/fixtures/test_evidence/evfix_filtered.txt");
const BISHAN: &str = include_str!("../../../tests/fixtures/test_evidence/bishan_partial.txt");

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn lock() -> String {
    format!("sha256:{}", "c".repeat(64))
}

fn parse(text: &str) -> (output::ParsedRun, usize) {
    let mut p = OutputParser::new("/WS");
    let json = text.lines().filter(|l| p.feed(l) == LineKind::Json).count();
    (p.finish(), json)
}

fn evidence(text: &str, extra: &[&str]) -> TestEvidence {
    let extra: Vec<String> = extra.iter().map(|s| s.to_string()).collect();
    let pl = plan(&extra).unwrap();
    let (run, _) = parse(text);
    let mut command = vec!["cargo".to_string()];
    command.extend(pl.args);
    TestEvidence::new(
        COMMIT.into(),
        Vec::new(),
        lock(),
        "2026-10-07T00:00:00Z".into(),
        "rustc 1.98.0 (88d9e12ae 2026-08-18)".into(),
        "cargo 1.98.0 (797e8a9bc 2026-08-05)".into(),
        command,
        pl.partial_reasons,
        Some(101),
        run.build_ok,
        run.binaries,
    )
}

/// Methodology: the captured full output parses into its three binaries
/// with the package, kind, target and workspace-relative source from cargo's
/// artifact messages; every outcome lands in the right list (`should
/// panic` suffix stripped, `ignored, reason` read as ignored); the result
/// lines in the `failures:` section are not double-counted; the aborted
/// binary is incomplete. The run is full in scope but does **not** count
/// (incomplete) and goes to the uncounted file.
///
/// Result (2026-10-07): passes; lib 4 passed / 1 failed / 1 ignored,
/// crash 1 passed and no summary, integration 2 passed.
#[test]
fn captured_full_output_parses_and_a_crashed_binary_is_incomplete() {
    let (run, json) = parse(FULL);
    assert!(json >= 4 && run.build_ok);
    let e = evidence(FULL, &["-j", "1"]);
    let srcs: Vec<(&str, BinaryKind, &str, &str)> = e
        .binaries
        .iter()
        .map(|b| {
            (
                b.package.as_str(),
                b.kind,
                b.target.as_str(),
                b.src.as_str(),
            )
        })
        .collect();
    assert_eq!(
        srcs,
        vec![
            ("evfix", BinaryKind::Lib, "evfix", "crates/evfix/src/lib.rs"),
            (
                "evfix",
                BinaryKind::Test,
                "crash",
                "crates/evfix/tests/crash.rs"
            ),
            (
                "evfix",
                BinaryKind::Test,
                "integration",
                "crates/evfix/tests/integration.rs"
            ),
        ]
    );
    let lib = &e.binaries[0];
    assert_eq!(
        lib.passed,
        vec![
            "geom::tests::area_is_positive",
            "geom::tests::inner::nested_ok",
            "tests::adds",
            "tests::panics"
        ]
    );
    assert_eq!(lib.failed, vec!["tests::fails"]);
    assert_eq!(lib.ignored, vec!["tests::slow"]);
    assert!(lib.complete && lib.expected == Some(6));
    let crash = &e.binaries[1];
    assert_eq!(
        (crash.passed.len(), crash.summary, crash.complete),
        (1, None, false)
    );
    assert!(e.binaries[2].complete);
    assert_eq!(
        e.totals,
        Totals {
            passed: 7,
            failed: 1,
            ignored: 1
        }
    );
    assert_eq!(e.scope, Scope::Full);
    assert_eq!(
        e.counted(),
        Err(vec![NotCounted::Incomplete(vec![
            "crates/evfix/tests/crash.rs".into()
        ])])
    );
    assert_eq!(e.destination(), UNCOUNTED_FILE);
}

/// Methodology: a run without the crashed binary (and with failures)
/// counts, is written to the counted file, and round-trips through TOML; a
/// code-folder `kovan.toml` reader refuses it and it refuses a `kovan.toml`.
///
/// Result (2026-10-07): passes.
#[test]
fn a_complete_full_run_with_failures_counts_and_round_trips() {
    let mut e = evidence(FULL, &[]);
    e.binaries.retain(|b| b.target != "crash");
    let e = TestEvidence::new(
        e.commit,
        e.dirty_paths,
        e.cargo_lock,
        e.date,
        e.rustc,
        e.cargo,
        e.command,
        e.partial_reasons,
        e.exit_code,
        true,
        e.binaries,
    );
    assert_eq!(e.counted(), Ok(()));
    assert_eq!(e.destination(), UNCOUNTED_FILE);
    let text = e.to_toml().unwrap();
    assert_eq!(TestEvidence::parse(&text).unwrap(), e);
    assert!(text.contains("kind = \"test_evidence\"") && text.contains("[[binary]]"));
    assert!(FolderIndex::parse(&text).is_err());
    let folder = "schema_version = 1\nkind = \"code_folder\"\ncrate = \"x\"\ndir = \"d\"\n";
    assert!(matches!(
        TestEvidence::parse(folder),
        Err(EvidenceError::NotEvidence { .. })
    ));
    let mut dirty = e.clone();
    dirty.dirty = true;
    dirty.dirty_paths = vec!["crates/evfix/src/lib.rs".into()];
    assert!(
        matches!(dirty.counted(), Err(v) if v == vec![NotCounted::Dirty(vec!["crates/evfix/src/lib.rs".into()])])
    );
}

/// Methodology: a filter passed after `--` is caught twice: by the argument
/// plan and by libtest's own `filtered out` counts; a `-p` run of this
/// workspace's `bishan` (captured) is partial. Neither counts.
///
/// Result (2026-10-07): passes.
#[test]
fn filtered_and_package_runs_are_partial() {
    let e = evidence(FILTERED, &["--", "area"]);
    assert_eq!(e.scope, Scope::Partial);
    assert_eq!(e.partial_reasons[0], "-- area");
    assert!(e
        .partial_reasons
        .iter()
        .any(|r| r.contains("5 tests filtered out")));
    assert!(e.complete);
    // Even without the argument, libtest's counts expose the filter.
    let mut p = OutputParser::new("/WS");
    FILTERED.lines().for_each(|l| {
        p.feed(l);
    });
    let run = p.finish();
    let e2 = TestEvidence::new(
        COMMIT.into(),
        vec![],
        lock(),
        String::new(),
        String::new(),
        String::new(),
        vec![],
        vec![],
        Some(0),
        run.build_ok,
        run.binaries,
    );
    assert_eq!(e2.scope, Scope::Partial);

    let b = evidence(BISHAN, &["-p", "bishan", "-j", "1"]);
    assert_eq!(b.scope, Scope::Partial);
    assert_eq!(b.partial_reasons, vec!["-p bishan"]);
    assert_eq!(b.binaries.len(), 1);
    assert_eq!(b.binaries[0].src, "crates/bishan/src/lib.rs");
    assert_eq!(b.binaries[0].passed.len(), 3);
    assert!(matches!(b.counted(), Err(v) if matches!(v[0], NotCounted::Partial(_))));
}

fn func(file: &str, name: &str, id_suffix: &str) -> Function {
    Function {
        id: format!("{file}::{name}{id_suffix}"),
        ambiguous: !id_suffix.is_empty(),
        name: name.into(),
        kind: FnKind::Free,
        owner: None,
        trait_name: None,
        test: true,
        test_fn: true,
        start_line: 1,
        line: 1,
        end_line: 1,
        signature: String::new(),
        doc: String::new(),
        source: String::new(),
        unresolved: Vec::new(),
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
        maturity: None,
        functions,
        upstream: None,
        upstream_unparsed: None,
        history: Vec::new(),
        concepts: Vec::new(),
    }
}

/// A hand-built call graph of `evfix`. To exercise the ambiguous rule, the
/// lib root lists `fails` twice (`#1`, `#2`) and `geom/tests.rs` lists
/// `nested_ok` twice, as the id scheme does for one name in two inline
/// modules of a file.
fn evfix_graph() -> CallGraphDoc {
    let lib = "crates/evfix/src/lib.rs";
    let gt = "crates/evfix/src/geom/tests.rs";
    let it = "crates/evfix/tests/integration.rs";
    let c = CrateGraph {
        name: "evfix".into(),
        dir: "crates/evfix".into(),
        maturity: None,
        targets: vec![Target {
            kind: TargetKind::Lib,
            name: "evfix".into(),
            root: lib.into(),
            modules: vec![
                module(
                    lib,
                    "",
                    vec![
                        func(lib, "adds", ""),
                        func(lib, "slow", ""),
                        func(lib, "panics", ""),
                        func(lib, "fails", "#1"),
                        func(lib, "fails", "#2"),
                    ],
                ),
                module("crates/evfix/src/geom/mod.rs", "geom", vec![]),
                module(
                    gt,
                    "geom::tests",
                    vec![
                        func(gt, "area_is_positive", ""),
                        func(gt, "nested_ok", "#1"),
                        func(gt, "nested_ok", "#2"),
                    ],
                ),
            ],
        }],
        tests: vec![Target {
            kind: TargetKind::Test,
            name: "integration".into(),
            root: it.into(),
            modules: vec![module(
                it,
                "",
                vec![
                    func(it, "integration_adds", ""),
                    func(it, "helper_test", ""),
                ],
            )],
        }],
    };
    CallGraphDoc::assemble(vec![c], vec![], vec![], 0)
}

/// Methodology: libtest names map to the call graph's test ids by the rule
/// in [`map`] (longest file-module prefix, inline modules flattened); the
/// projection to `[test_run]` keeps passes and failures, marks every
/// candidate of an ambiguous failure failed, drops an ambiguous pass and
/// reports it; [`restrict`] keeps one folder's tests; the verdict for a
/// function reached by a failing test is `Failed`.
///
/// Result (2026-10-07): passes.
#[test]
fn libtest_names_map_to_call_graph_test_ids() {
    let m = TestIdMap::from_graph(&evfix_graph());
    let lib = "crates/evfix/src/lib.rs";
    assert_eq!(
        m.map(lib, "geom::tests::area_is_positive"),
        Mapped::Id("crates/evfix/src/geom/tests.rs::area_is_positive".into())
    );
    assert_eq!(
        m.map(lib, "tests::adds"),
        Mapped::Id(format!("{lib}::adds"))
    );
    assert!(
        matches!(m.map(lib, "geom::tests::inner::nested_ok"), Mapped::Ambiguous(v) if v.len() == 2)
    );
    assert_eq!(m.map(lib, "tests::nope"), Mapped::NoFunction);
    assert_eq!(
        m.map("crates/evfix/tests/crash.rs", "a_first"),
        Mapped::NoTarget
    );
    assert_eq!(
        m.map("crates/evfix/tests/integration.rs", "helpers::helper_test"),
        Mapped::Id("crates/evfix/tests/integration.rs::helper_test".into())
    );

    let mut e = evidence(FULL, &[]);
    e.binaries.retain(|b| b.target != "crash");
    let e = TestEvidence::new(
        e.commit,
        e.dirty_paths,
        e.cargo_lock,
        e.date,
        e.rustc,
        e.cargo,
        e.command,
        e.partial_reasons,
        e.exit_code,
        true,
        e.binaries,
    );
    let (run, unmapped) = to_test_run(&e, &m).unwrap();
    assert_eq!(
        run.failed,
        vec![format!("{lib}::fails#1"), format!("{lib}::fails#2")]
    );
    assert_eq!(
        run.passed,
        vec![
            "crates/evfix/src/geom/tests.rs::area_is_positive".to_string(),
            format!("{lib}::adds"),
            format!("{lib}::panics"),
            "crates/evfix/tests/integration.rs::helper_test".into(),
            "crates/evfix/tests/integration.rs::integration_adds".into(),
        ]
    );
    let names: Vec<&str> = unmapped.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(names, vec!["geom::tests::inner::nested_ok", "tests::fails"]);

    let wanted = [format!("{lib}::adds"), format!("{lib}::fails#1")];
    let only_lib: std::collections::BTreeSet<&str> = wanted.iter().map(String::as_str).collect();
    let r = restrict(&run, &only_lib);
    assert_eq!((r.passed.len(), r.failed.len()), (1, 1));

    assert_eq!(
        reach_verdict(
            &[format!("{lib}::adds"), format!("{lib}::fails#2")],
            Some(&run),
            &lock()
        ),
        ReachVerdict::Failed {
            failed: vec![format!("{lib}::fails#2")]
        }
    );

    // Partial evidence never projects.
    assert!(to_test_run(&evidence(BISHAN, &["-p", "bishan"]), &m).is_err());
}

/// Methodology: of a `git status --porcelain` listing, only build inputs
/// make a run dirty: modified and untracked `.rs`, manifests, the lock,
/// cargo config and both sides of a renamed source file; docs and the
/// evidence file itself do not.
///
/// Result (2026-10-07): passes.
#[test]
fn only_build_inputs_make_a_run_dirty() {
    let porcelain = [
        " M crates/a/src/lib.rs",
        "?? crates/a/tests/new.rs",
        "M  Cargo.lock",
        "?? .cargo/config.toml",
        "R  crates/a/src/old.rs -> crates/a/src/new.rs",
        " M crates/a/README.md",
        "?? kovan_test_evidence.toml",
        " M crates/a/Cargo.toml",
    ]
    .join("\n");
    assert_eq!(
        dirty_build_inputs(&porcelain),
        vec![
            ".cargo/config.toml",
            "Cargo.lock",
            "crates/a/Cargo.toml",
            "crates/a/src/lib.rs",
            "crates/a/src/new.rs",
            "crates/a/src/old.rs",
            "crates/a/tests/new.rs",
        ]
    );
    assert!(dirty_build_inputs("").is_empty());
}
