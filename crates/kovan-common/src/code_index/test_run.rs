//! Write a counted `kovan-cli test` run into every folder's `kovan.toml`
//! `[test_run]` (maintainer, #766, 2026-10-07: "test evidence lives IN
//! kovan.toml, and the kovan.toml files ARE committed"; the separate
//! `kovan_test_evidence.toml` is dropped).
//!
//! libtest names are mapped to test ids with #766's rule
//! ([`crate::review::evidence::map`]) over a call graph rebuilt **from the
//! `kovan.toml` files themselves** ([`graph_from_folders`]): each module's
//! key gives its target (`crate` / `test:<name>`) and module path, and each
//! function marked `test` is a candidate. So `kovan-cli test` needs no
//! rust-analyzer. (A `#[cfg(test)]` helper is marked `test` too, so a
//! helper sharing a test's name in the same module makes the mapping
//! ambiguous; ambiguous names are reported, a failure among them marks
//! every candidate failed, as #766 decided.)
//!
//! Each folder gets the run [`restrict`]ed to the tests it uses: its
//! functions' `reached_by` and its own tests. A run that does not count
//! (partial, dirty, incomplete) is never written here.

use std::collections::{BTreeMap, BTreeSet};

use crate::call_graph::{CallGraphDoc, CrateGraph, FnKind, Function, Module, Target, TargetKind};
use crate::review::evidence::map::{restrict, to_test_run, TestIdMap, Unmapped};
use crate::review::evidence::{NotCounted, TestEvidence};
use crate::review::index::FolderIndex;

fn test_fn(id: String, name: &str) -> Function {
    Function {
        id,
        ambiguous: false,
        name: name.to_string(),
        kind: FnKind::Free,
        owner: None,
        trait_name: None,
        test: true,
        test_fn: true,
        start_line: 0,
        line: 0,
        end_line: 0,
        signature: String::new(),
        doc: String::new(),
        source: String::new(),
        unresolved: Vec::new(),
        reached_by: None,
        cited_by: Vec::new(),
    }
}

/// The part of a call graph the libtest-name mapping reads (targets, their
/// root files, module paths and test functions), rebuilt from `kovan.toml`
/// files. Example targets are skipped (their tests are not in the suite).
pub fn graph_from_folders(folders: &[FolderIndex]) -> CallGraphDoc {
    // (crate, kind, target name) -> modules; root file per target.
    let mut targets: BTreeMap<(String, TargetKind, String), (Option<String>, Vec<Module>)> = BTreeMap::new();
    for fi in folders {
        for (file, m) in &fi.modules {
            let (kind, name, path) = if let Some(rest) = m.path.strip_prefix("test:") {
                let (n, p) = rest.split_once("::").unwrap_or((rest, ""));
                (TargetKind::Test, n.to_string(), p.to_string())
            } else if m.path == "crate" || m.path.starts_with("crate::") {
                let p = m.path.strip_prefix("crate").unwrap_or("").trim_start_matches("::");
                (TargetKind::Lib, fi.krate.clone(), p.to_string())
            } else {
                continue;
            };
            let path_ws = fi.file_path(file);
            let functions = m
                .functions
                .iter()
                .filter(|f| f.test)
                .map(|f| test_fn(format!("{path_ws}::{}", f.qual), &f.name))
                .collect();
            let e = targets.entry((fi.krate.clone(), kind, name)).or_default();
            if path.is_empty() {
                e.0 = Some(path_ws.clone());
            }
            e.1.push(Module {
                file: path_ws,
                path,
                parent: None,
                test: false,
                maturity: None,
                functions,
                upstream: None,
                upstream_unparsed: None,
                history: Vec::new(),
                concepts: Vec::new(),
            });
        }
    }
    let mut crates: BTreeMap<String, CrateGraph> = BTreeMap::new();
    for ((krate, kind, name), (root, modules)) in targets {
        let Some(root) = root else { continue };
        let c = crates.entry(krate.clone()).or_insert_with(|| CrateGraph {
            name: krate.clone(),
            dir: String::new(),
            maturity: None,
            targets: Vec::new(),
            tests: Vec::new(),
        });
        let t = Target { kind, name, root, modules };
        match kind {
            TargetKind::Test => c.tests.push(t),
            _ => c.targets.push(t),
        }
    }
    CallGraphDoc {
        crates: crates.into_values().collect(),
        ..CallGraphDoc::default()
    }
}

/// Write counted `evidence` into every folder's `[test_run]`, restricted to
/// the tests it uses. Returns the libtest names that did not map to one
/// test id; refuses (with the reasons) a run that does not count.
pub fn write_counted(evidence: &TestEvidence, folders: &mut [FolderIndex]) -> Result<Vec<Unmapped>, Vec<NotCounted>> {
    let map = TestIdMap::from_graph(&graph_from_folders(folders));
    let (run, unmapped) = to_test_run(evidence, &map)?;
    for fi in folders.iter_mut() {
        let mut wanted: BTreeSet<String> = BTreeSet::new();
        for (file, f) in fi.functions() {
            wanted.extend(f.reached_by.iter().cloned());
            if f.test {
                wanted.insert(format!("{}::{}", fi.file_path(file), f.qual));
            }
        }
        let w: BTreeSet<&str> = wanted.iter().map(String::as_str).collect();
        fi.test_run = Some(restrict(&run, &w));
        fi.normalise();
    }
    Ok(unmapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_index::folders::build;
    use crate::code_index::folders::tests::{input, C1, IT_RS, LIB_RS};
    use crate::review::evidence::{BinaryKind, BinaryResults, Scope, Summary};

    /// Methodology: the fixture's full build (`folders` tests) gives two
    /// folders; a counted run with the lib's `tests::t_twice` passing and the
    /// integration test `it_leaf` failing is written into them. `src/` (whose
    /// `leaf` is reached by both, and which holds `t_twice`) gets both,
    /// `t_twice` passed and `it_leaf` failed; `tests/` gets `it_leaf`. An
    /// unknown name is reported, and a dirty run is refused and writes
    /// nothing.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn a_counted_run_lands_in_every_folder_restricted_to_its_tests() {
        let mut folders: Vec<FolderIndex> = build(&input(C1)).folders.into_values().collect();
        let bin = |src: &str, kind: BinaryKind, passed: &[&str], failed: &[&str]| BinaryResults {
            package: "kern".into(),
            kind,
            target: "kern".into(),
            src: src.into(),
            expected: Some((passed.len() + failed.len()) as u32),
            complete: true,
            passed: passed.iter().map(|s| s.to_string()).collect(),
            failed: failed.iter().map(|s| s.to_string()).collect(),
            ignored: Vec::new(),
            summary: Some(Summary {
                ok: failed.is_empty(),
                passed: passed.len() as u32,
                failed: failed.len() as u32,
                ..Summary::default()
            }),
        };
        let ev = TestEvidence::new(
            C1.into(),
            Vec::new(),
            format!("sha256:{}", "c".repeat(64)),
            "2026-10-07T00:00:00Z".into(),
            "rustc".into(),
            "cargo".into(),
            vec!["cargo".into(), "test".into()],
            Vec::new(),
            Some(101),
            true,
            vec![
                bin(LIB_RS, BinaryKind::Lib, &["tests::t_twice", "tests::nope"], &[]),
                bin(IT_RS, BinaryKind::Test, &[], &["it_leaf"]),
            ],
        );
        assert_eq!(ev.scope, Scope::Full);
        let unmapped = write_counted(&ev, &mut folders).unwrap();
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].name, "tests::nope");
        let src = folders.iter().find(|f| f.dir == "crates/kern/src").unwrap();
        let run = src.test_run.as_ref().unwrap();
        assert_eq!(run.passed, vec![format!("{LIB_RS}::t_twice")]);
        assert_eq!(run.failed, vec![format!("{IT_RS}::it_leaf")]);
        let tests = folders.iter().find(|f| f.dir == "crates/kern/tests").unwrap();
        assert_eq!(tests.test_run.as_ref().unwrap().failed, vec![format!("{IT_RS}::it_leaf")]);
        assert!(tests.test_run.as_ref().unwrap().passed.is_empty());

        let mut dirty = ev.clone();
        dirty.dirty = true;
        let mut fresh: Vec<FolderIndex> = build(&input(C1)).folders.into_values().collect();
        assert!(write_counted(&dirty, &mut fresh).is_err());
        assert!(fresh.iter().all(|f| f.test_run.is_none()));
    }
}
