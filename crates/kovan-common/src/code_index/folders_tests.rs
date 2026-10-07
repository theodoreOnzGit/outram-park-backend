use super::*;
use crate::call_graph::{
    Call, CallKind, CrateGraph, FnKind, Function, Module, OutsideFn, Target, Totals,
};
use crate::review::hash::hash_functions;
use crate::review::review_md::parse_review_md;

pub(crate) const LIB: &str = "pub mod util;

/// Doubles.
pub fn leaf(x: f64) -> f64 {
    x * 2.0
}

pub fn twice(x: f64) -> f64 {
    fn inner(y: f64) -> f64 {
        util::helper(y)
    }
    leaf(x) + inner(x)
}

#[cfg(test)]
mod tests {
    #[test]
    fn t_twice() {
        assert!(super::twice(1.0) > 0.0);
    }
}
";

pub(crate) const UTIL: &str = "pub fn helper(x: f64) -> f64 {
    x + 1.0
}
";

pub(crate) const IT: &str = "#[test]
fn it_leaf() {
    assert_eq!(kern::leaf(1.0), 2.0);
}
";

pub(crate) const C1: &str = "1111111111111111111111111111111111111111";
pub(crate) const C2: &str = "2222222222222222222222222222222222222222";

pub(crate) const LIB_RS: &str = "crates/kern/src/lib.rs";
pub(crate) const UTIL_RS: &str = "crates/kern/src/util.rs";
pub(crate) const IT_RS: &str = "crates/kern/tests/it.rs";

/// The call graph of a module's functions, as the builder would give it:
/// every function `syn` finds, plus `extra` (a nested `fn`).
fn module_of(file: &str, path: &str, text: &str, extra: &[(&str, u32, u32)]) -> Module {
    let mut functions: Vec<Function> = hash_functions(text)
        .unwrap()
        .into_iter()
        .map(|h| {
            let test_fn = h.entry.code.starts_with("# [ test ]");
            Function {
                id: format!("{file}::{}", h.entry.qualname()),
                ambiguous: false,
                name: h.entry.name.clone(),
                kind: FnKind::Free,
                owner: None,
                trait_name: None,
                test: h.entry.is_test || test_fn,
                test_fn,
                start_line: h.entry.lines[0],
                line: h.entry.lines[0],
                end_line: h.entry.lines[1],
                signature: String::new(),
                doc: String::new(),
                source: String::new(),
                unresolved: Vec::new(),
                reached_by: None,
                cited_by: Vec::new(),
            }
        })
        .collect();
    for (name, a, b) in extra {
        let mut f = functions[0].clone();
        f.id = format!("{file}::{name}");
        f.name = name.to_string();
        f.test = false;
        f.test_fn = false;
        f.start_line = *a;
        f.line = *a;
        f.end_line = *b;
        functions.push(f);
    }
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

pub(crate) fn graph() -> CallGraphDoc {
    let lib = Target {
        kind: TargetKind::Lib,
        name: "kern".into(),
        root: LIB_RS.into(),
        modules: vec![
            module_of(LIB_RS, "", LIB, &[("inner", 9, 11)]),
            module_of(UTIL_RS, "util", UTIL, &[]),
        ],
    };
    let it = Target {
        kind: TargetKind::Test,
        name: "it".into(),
        root: IT_RS.into(),
        modules: vec![module_of(IT_RS, "", IT, &[])],
    };
    let call = |from: &str, to: &str| Call {
        from: from.into(),
        to: to.into(),
        kind: CallKind::Call,
        lines: vec![1],
    };
    let l = |n: &str| format!("{LIB_RS}::{n}");
    CallGraphDoc {
        schema: crate::call_graph::SCHEMA_VERSION,
        scope: vec!["kern".into()],
        crates: vec![CrateGraph {
            name: "kern".into(),
            dir: "crates/kern".into(),
            maturity: None,
            targets: vec![lib],
            tests: vec![it],
        }],
        calls: vec![
            call(&l("twice"), &l("leaf")),
            call(&l("twice"), &l("inner")),
            call(&l("inner"), &format!("{UTIL_RS}::helper")),
            call(&l("t_twice"), &l("twice")),
            call(&format!("{IT_RS}::it_leaf"), &l("leaf")),
            call(&l("leaf"), "crates/other/src/lib.rs::far"),
        ],
        module_calls: Vec::new(),
        crate_calls: Vec::new(),
        outside: vec![OutsideFn {
            id: "crates/other/src/lib.rs::far".into(),
            krate: Some("other".into()),
            file: "crates/other/src/lib.rs".into(),
            line: 1,
        }],
        totals: Totals::default(),
        commit: None,
        site_base: None,
        generator: None,
    }
}

pub(crate) fn hashed() -> BTreeMap<String, Vec<HashedFn>> {
    [(LIB_RS, LIB), (UTIL_RS, UTIL), (IT_RS, IT)]
        .iter()
        .map(|(p, t)| (p.to_string(), hash_functions(t).unwrap()))
        .collect()
}

pub(crate) fn input(commit: &str) -> BuildInput {
    BuildInput {
        doc: graph(),
        hashed: hashed(),
        commit: commit.into(),
        ..BuildInput::default()
    }
}

fn find<'a>(b: &'a Built, dir: &str, file: &str, name: &str) -> &'a FunctionIndex {
    b.folders[dir].modules[file]
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap()
}

pub(crate) fn review_md(id: &str, path: &str, hash: &str, callee: (&str, &str)) -> String {
    format!(
        "# Review\n\n```toml\n[kovan]\nid = \"review-leaf-alice\"\nkind = \"review\"\ncreated = \"2026-10-07\"\nmodified = \"2026-10-07\"\ntarget = \"{id}\"\n\n[review]\npath = \"{path}\"\nby = \"github:alice\"\nrung = 3\ndate = \"2026-10-07\"\ncommit = \"{C1}\"\nhash = \"{hash}\"\ndoc_hash = \"sha256:{}\"\n\n[review.callees]\n\"{}\" = \"{}\"\n```\n",
        "b".repeat(64),
        callee.0,
        callee.1
    )
}

/// Methodology: a hand-built call graph of a one-crate fixture (lib with a
/// submodule, a nested `fn inner` the hasher does not list, a
/// `#[cfg(test)]` test and an integration test; one call leaves the
/// scope) and the real hashes of its source. Checked: one index per folder
/// (`src/`, the crate root, and `tests/`), module keys, ids are `fn:` ids,
/// `callees` look **through** the nested `inner` to `helper`, `reached_by`
/// lists every reaching test by test id (two for `leaf`, one for `helper`
/// through `inner`), `inner` is reported unhashed and the out-of-scope
/// callee with no known id is reported.
///
/// Result (2026-10-07): passes.
#[test]
fn folders_are_built_from_the_call_graph_and_hashes() {
    let b = build(&input(C1));
    assert_eq!(
        b.folders.keys().collect::<Vec<_>>(),
        vec!["crates/kern/src", "crates/kern/tests"]
    );
    let src = &b.folders["crates/kern/src"];
    assert!(src.crate_root && !b.folders["crates/kern/tests"].crate_root);
    assert_eq!(src.modules["lib.rs"].path, "crate");
    assert_eq!(src.modules["util.rs"].path, "crate::util");
    assert_eq!(b.folders["crates/kern/tests"].modules["it.rs"].path, "test:it");
    let leaf = find(&b, "crates/kern/src", "lib.rs", "leaf");
    let helper = find(&b, "crates/kern/src", "util.rs", "helper");
    let twice = find(&b, "crates/kern/src", "lib.rs", "twice");
    assert!(crate::review::id::is_fn_id(&leaf.id));
    assert_eq!(leaf.lines, [3, 6]);
    let mut want = vec![leaf.id.clone(), helper.id.clone()];
    want.sort();
    assert_eq!(twice.callees, want);
    assert_eq!(
        leaf.reached_by,
        vec![format!("{LIB_RS}::t_twice"), format!("{IT_RS}::it_leaf")]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(helper.reached_by, vec![format!("{LIB_RS}::t_twice")]);
    let t = find(&b, "crates/kern/src", "lib.rs", "t_twice");
    assert!(t.test && t.reached_by.is_empty());
    assert_eq!(b.report.unhashed, vec![format!("{LIB_RS}::inner")]);
    assert_eq!(b.report.outside_without_id, vec!["crates/other/src/lib.rs::far"]);
    for fi in b.folders.values() {
        let text = fi.to_toml().unwrap();
        assert_eq!(FolderIndex::parse(&text).unwrap().to_toml().unwrap(), text);
    }
}

/// Methodology: determinism and id continuity. The same inputs give
/// byte-identical `kovan.toml` text; with the previous output as
/// `previous`, a run at another commit gives the same bytes (ids kept from
/// the cache, not re-minted); without it, unreviewed ids are minted anew
/// (the documented consequence of losing the cache), but a function a
/// `review.md` names keeps its id, and a review's callee keeps its id by
/// hash, so the review's join keys survive losing every `kovan.toml`.
/// The `review.md` is cached as `[[review]]` and `[test_run]` is carried.
///
/// Result (2026-10-07): passes.
#[test]
fn output_is_deterministic_and_reviewed_ids_survive_losing_the_cache() {
    let texts = |b: &Built| -> Vec<String> { b.folders.values().map(|f| f.to_toml().unwrap()).collect() };
    let first = build(&input(C1));
    assert_eq!(texts(&build(&input(C1))), texts(&first));
    let mut again = input(C2);
    again.previous = first.folders.clone();
    assert_eq!(texts(&build(&again)), texts(&first));
    let lost = build(&input(C2));
    assert_ne!(texts(&lost), texts(&first));

    // A review of `leaf` naming `helper` as a callee by its id and hash.
    let leaf = find(&first, "crates/kern/src", "lib.rs", "leaf").clone();
    let helper = find(&first, "crates/kern/src", "util.rs", "helper").clone();
    let mut with_review = input(C1);
    with_review.reviews.insert(
        "crates/kern/src/review.md".into(),
        parse_review_md(&review_md(
            "fn:00000000000000aa",
            &format!("{LIB_RS}::leaf"),
            &leaf.hash,
            (&helper.id, &helper.hash),
        )),
    );
    let run = TestRun {
        commit: C1.into(),
        cargo_lock: format!("sha256:{}", "c".repeat(64)),
        suite: crate::review::index::Suite::Full,
        passed: vec![format!("{IT_RS}::it_leaf")],
        failed: Vec::new(),
        edited: Vec::new(),
    };
    with_review.test_runs.insert("crates/kern/tests".into(), run.clone());
    let reviewed = build(&with_review);
    assert_eq!(find(&reviewed, "crates/kern/src", "lib.rs", "leaf").id, "fn:00000000000000aa");
    let src = &reviewed.folders["crates/kern/src"];
    assert_eq!(src.reviews.len(), 1);
    assert_eq!(src.reviews[0].function, "fn:00000000000000aa");
    assert_eq!(reviewed.folders["crates/kern/tests"].test_run.as_ref(), Some(&run));
    // Every kovan.toml lost, another commit: the review's ids come back.
    with_review.commit = C2.into();
    let rebuilt = build(&with_review);
    assert_eq!(find(&rebuilt, "crates/kern/src", "lib.rs", "leaf").id, "fn:00000000000000aa");
    assert_eq!(find(&rebuilt, "crates/kern/src", "util.rs", "helper").id, helper.id);
}
