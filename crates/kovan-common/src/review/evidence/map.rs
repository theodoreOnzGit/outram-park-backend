//! Map libtest names to the call graph's **test ids**, and project counted
//! evidence onto `kovan.toml`'s `[test_run]` ([`TestRun`]).
//!
//! A test id is the call graph's function id ([`crate::call_graph`] module
//! doc, [`crate::call_graph::reach`]): `crates/x/src/steam.rs::flash_works`,
//! with **inline modules flattened** and `#k` appended when the name is not
//! unique in its file. libtest prints the full module path from the target
//! root: `steam::tests::flash_works`.
//!
//! # The rule
//!
//! 1. The binary's `src` (workspace-relative root file) selects the call
//!    graph target whose `root` is the same file (lib targets and
//!    integration-test targets; bin targets are not in the call graph).
//! 2. The **longest file-module path** of that target that prefixes the
//!    libtest name (`steam`, or `steam::tests` when `tests` is its own file)
//!    selects the file; the root module has the empty path.
//! 3. The last segment is the function name. Exactly one test function
//!    (`test_fn`) with that name in that file → its id.
//!
//! Zero candidates is [`Mapped::NoFunction`]; several (the same name in two
//! inline modules of one file, ids ending `#k`) is [`Mapped::Ambiguous`]. In
//! [`to_test_run`] an ambiguous **failure** marks every candidate failed
//! (a failure is never dropped), an ambiguous pass marks none passed.

use std::collections::{BTreeMap, BTreeSet};

use crate::call_graph::{CallGraphDoc, TargetKind};

use super::super::index::{Suite, TestRun};
use super::{NotCounted, TestEvidence};

/// How one libtest name maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mapped {
    Id(String),
    /// No call-graph target has this root file.
    NoTarget,
    /// The target has no test function of that name where the path says.
    NoFunction,
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, Default)]
struct TargetMap {
    /// module path → (fn name → ids of test functions)
    modules: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// The lookup built from one call graph.
#[derive(Debug, Clone, Default)]
pub struct TestIdMap {
    targets: BTreeMap<String, TargetMap>,
}

impl TestIdMap {
    pub fn from_graph(doc: &CallGraphDoc) -> TestIdMap {
        let mut targets: BTreeMap<String, TargetMap> = BTreeMap::new();
        for c in &doc.crates {
            for t in c.targets.iter().chain(c.tests.iter()) {
                if t.kind == TargetKind::Example {
                    continue;
                }
                let tm = targets.entry(t.root.clone()).or_default();
                for m in &t.modules {
                    let fns = tm.modules.entry(m.path.clone()).or_default();
                    for f in m.functions.iter().filter(|f| f.test_fn) {
                        fns.entry(f.name.clone()).or_default().push(f.id.clone());
                    }
                }
            }
        }
        TestIdMap { targets }
    }

    /// Map the libtest name `name` of the binary rooted at `src`.
    pub fn map(&self, src: &str, name: &str) -> Mapped {
        let Some(tm) = self.targets.get(src) else {
            return Mapped::NoTarget;
        };
        let best = tm
            .modules
            .keys()
            .filter(|p| p.is_empty() || name.starts_with(&format!("{p}::")))
            .max_by_key(|p| p.len());
        let Some(path) = best else {
            return Mapped::NoFunction;
        };
        let fn_name = name.rsplit("::").next().unwrap_or(name);
        match tm.modules[path].get(fn_name).map(Vec::as_slice) {
            Some([id]) => Mapped::Id(id.clone()),
            Some(ids) if ids.len() > 1 => Mapped::Ambiguous(ids.to_vec()),
            _ => Mapped::NoFunction,
        }
    }
}

/// A libtest name that did not map to one test id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    pub src: String,
    pub name: String,
    pub why: Mapped,
}

/// The `[test_run]` table for `kovan.toml`, from **counted** evidence only
/// (a partial, dirty or incomplete run gives its reasons instead, and the
/// engine shows "test evidence pending"). Also returns the names that did
/// not map to exactly one id.
pub fn to_test_run(
    evidence: &TestEvidence,
    map: &TestIdMap,
) -> Result<(TestRun, Vec<Unmapped>), Vec<NotCounted>> {
    evidence.counted()?;
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unmapped = Vec::new();
    for b in &evidence.binaries {
        for (names, is_fail) in [(&b.passed, false), (&b.failed, true)] {
            for n in names {
                match map.map(&b.src, n) {
                    Mapped::Id(id) => {
                        if is_fail {
                            failed.insert(id);
                        } else {
                            passed.insert(id);
                        }
                    }
                    other => {
                        if let (true, Mapped::Ambiguous(ids)) = (is_fail, &other) {
                            failed.extend(ids.iter().cloned());
                        }
                        unmapped.push(Unmapped {
                            src: b.src.clone(),
                            name: n.clone(),
                            why: other,
                        });
                    }
                }
            }
        }
    }
    // A failure wins over a pass of an id an ambiguous failure also marked.
    let passed: Vec<String> = passed.difference(&failed).cloned().collect();
    Ok((
        TestRun {
            commit: evidence.commit.clone(),
            cargo_lock: evidence.cargo_lock.clone(),
            suite: Suite::Full,
            passed,
            failed: failed.into_iter().collect(),
            edited: Vec::new(),
        },
        unmapped,
    ))
}

/// `run` restricted to the tests in `wanted` (one folder's `reached_by`),
/// so a folder's `kovan.toml` carries only the evidence it uses.
pub fn restrict(run: &TestRun, wanted: &BTreeSet<&str>) -> TestRun {
    let keep = |v: &Vec<String>| -> Vec<String> {
        v.iter()
            .filter(|t| wanted.contains(t.as_str()))
            .cloned()
            .collect()
    };
    TestRun {
        commit: run.commit.clone(),
        cargo_lock: run.cargo_lock.clone(),
        suite: run.suite,
        passed: keep(&run.passed),
        failed: keep(&run.failed),
        edited: keep(&run.edited),
    }
}
