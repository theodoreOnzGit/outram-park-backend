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
                shape: None,
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
        items: Vec::new(),
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
        uses: Vec::new(),
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
        targets: Vec::new(),
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

/// A file holding only items, as a `types.rs` would.
pub(crate) const TYPES: &str = "/// Failure fraction of each coating layer.
pub struct Fractions {
    pub sic: f64,
}

impl Fractions {
    pub fn total(&self) -> f64 {
        self.sic
    }
}

/// Flow regime.
pub enum Regime {
    Laminar,
    Turbulent,
}

pub trait Fluid {
    fn density(&self) -> f64;
}

pub type Fraction = f64;

/// Stefan-Boltzmann constant, W/(m2 K4), CODATA 2018.
pub const SIGMA: f64 = 5.670374e-8;

#[cfg(test)]
mod tests {
    const TOL: f64 = 1e-9;
}
";

pub(crate) const TYPES_RS: &str = "crates/kern/src/types.rs";

/// [`input`] with a third library file, `types.rs`, and every file's items.
pub(crate) fn input_with_items(commit: &str) -> BuildInput {
    let mut i = input(commit);
    i.doc.crates[0].targets[0].modules.push(module_of(TYPES_RS, "types", TYPES, &[]));
    i.hashed.insert(TYPES_RS.into(), hash_functions(TYPES).unwrap());
    for (p, t) in [(LIB_RS, LIB), (UTIL_RS, UTIL), (IT_RS, IT), (TYPES_RS, TYPES)] {
        i.items.insert(p.into(), crate::review::hash::hash_items(t).unwrap());
    }
    i
}

/// Methodology (#843): build the fixture with a `types.rs` holding a
/// struct with a method, an enum, a trait, a type alias, a constant and a
/// constant under `#[cfg(test)]`. Pass: each is written to the folder's
/// `kovan.toml` beside the functions with its `item`, `shape`, hash and
/// line range; the test constant is marked test code; ids are minted
/// `fn:` ids and stay the same on a second build at another commit; the
/// functions of the fixture are exactly as they were without items; a
/// first-version reader's view survives (the text parses back to the same
/// index); and an input without items writes no item.
///
/// Result (2026-10-13): passes.
#[test]
fn items_are_indexed_beside_functions() {
    let b = build(&input_with_items(C1));
    assert!(b.report.skipped_items.is_empty());
    let fi = &b.folders["crates/kern/src"];
    let got: Vec<(String, ItemKind, Option<ItemShape>, bool)> = fi.modules["types.rs"]
        .functions
        .iter()
        .map(|f| (f.qual.clone(), f.item, f.shape, f.test))
        .collect();
    let want = [
        ("Fractions", ItemKind::Type, Some(ItemShape::Struct), false),
        ("Fractions::total", ItemKind::Fn, None, false),
        ("Regime", ItemKind::Type, Some(ItemShape::Enum), false),
        ("Fluid", ItemKind::Type, Some(ItemShape::Trait), false),
        ("Fluid::density", ItemKind::Fn, None, false),
        ("Fraction", ItemKind::Type, Some(ItemShape::Alias), false),
        ("SIGMA", ItemKind::Const, Some(ItemShape::Const), false),
        ("TOL", ItemKind::Const, Some(ItemShape::Const), true),
    ];
    let mut got_sorted = got.clone();
    got_sorted.sort();
    let mut want_sorted: Vec<_> = want.iter().map(|(q, i, s, t)| (q.to_string(), *i, *s, *t)).collect();
    want_sorted.sort();
    assert_eq!(got_sorted, want_sorted);
    let fr = find(&b, "crates/kern/src", "types.rs", "Fractions");
    assert_eq!(fr.lines, [1, 4]);
    assert!(fr.id.starts_with("fn:") && fr.hash.starts_with("sha256:"));
    assert!(fr.callees.is_empty() && fr.reached_by.is_empty());
    // Ids are stable across builds, with the first build as the priors.
    let mut again = input_with_items(C2);
    again.previous = b.folders.clone();
    let b2 = build(&again);
    assert_eq!(b2.folders, b.folders);
    // The functions are untouched by indexing items.
    let plain = build(&input(C1));
    for (name, m) in &plain.folders["crates/kern/src"].modules {
        let fns: Vec<&FunctionIndex> = fi.modules[name].functions.iter().filter(|f| f.item.is_fn()).collect();
        assert_eq!(fns, m.functions.iter().collect::<Vec<_>>(), "{name}");
    }
    assert!(plain.folders.values().all(|f| f.functions().all(|(_, f)| f.shape.is_none())));
    let text = fi.to_toml().unwrap();
    assert!(text.contains("item = \"type\"") && text.contains("shape = \"struct\""), "{text}");
    assert_eq!(&FolderIndex::parse(&text).unwrap(), fi);
}

/// Methodology (#843): the items fixture with four uses in the call graph:
/// `leaf` (reached by the tests `t_twice` and `it_leaf`) names `Fractions`
/// and `SIGMA`; the struct `Fractions` names the alias `Fraction`; the test
/// function `it_leaf` names `Regime` directly; and one use of something
/// that is not an indexed item. Pass: `leaf` lists the two ids under
/// `uses` and its `callees` are as before; `Fractions` lists `Fraction`;
/// `Fractions` and `SIGMA` are reached by both tests, `Fraction` too
/// (through the struct that holds it), `Regime` by `it_leaf` alone, the
/// trait `Fluid` by none; the unknown use is dropped; and a refresh of the
/// built folder reproduces it byte for byte.
///
/// Result (2026-10-13): passes.
#[test]
fn uses_link_functions_to_items_and_items_to_tests() {
    let mut i = input_with_items(C1);
    let l = |n: &str| format!("{LIB_RS}::{n}");
    let t = |n: &str| format!("{TYPES_RS}::{n}");
    let raw = |from: String, to: String| crate::call_graph::RawCall { from, to, kind: CallKind::Call, line: 3 };
    i.doc.set_uses(vec![
        raw(l("leaf"), t("Fractions")),
        raw(l("leaf"), t("SIGMA")),
        raw(l("leaf"), t("SIGMA")),
        raw(t("Fractions"), t("Fraction")),
        raw(format!("{IT_RS}::it_leaf"), t("Regime")),
        raw(l("leaf"), "crates/other/src/lib.rs::Elsewhere".into()),
        raw(l("leaf"), l("leaf")),
    ]);
    assert_eq!(i.doc.uses.len(), 5, "aggregated, self use dropped");
    let b = build(&i);
    let id = |file: &str, name: &str| find(&b, "crates/kern/src", file, name).id.clone();
    let leaf = find(&b, "crates/kern/src", "lib.rs", "leaf");
    let mut want = vec![id("types.rs", "Fractions"), id("types.rs", "SIGMA")];
    want.sort();
    assert_eq!(leaf.uses, want);
    assert_eq!(leaf.callees, find(&build(&input(C1)), "crates/kern/src", "lib.rs", "leaf").callees);
    let fr = find(&b, "crates/kern/src", "types.rs", "Fractions");
    assert_eq!(fr.uses, vec![id("types.rs", "Fraction")]);
    let both = vec![l("t_twice"), format!("{IT_RS}::it_leaf")];
    assert_eq!(fr.reached_by, both);
    assert_eq!(find(&b, "crates/kern/src", "types.rs", "SIGMA").reached_by, both);
    assert_eq!(find(&b, "crates/kern/src", "types.rs", "Fraction").reached_by, both);
    assert_eq!(find(&b, "crates/kern/src", "types.rs", "Regime").reached_by, vec![format!("{IT_RS}::it_leaf")]);
    assert!(find(&b, "crates/kern/src", "types.rs", "Fluid").reached_by.is_empty());
    let prev = &b.folders["crates/kern/src"];
    let files: BTreeMap<String, String> =
        [("lib.rs", LIB), ("util.rs", UTIL), ("types.rs", TYPES)].iter().map(|(f, t)| (f.to_string(), t.to_string())).collect();
    let (same, _) = crate::code_index::refresh::refresh_folder("kern", "crates/kern/src", Some(prev), &files, None, &[], C2);
    assert_eq!(same.to_toml().unwrap(), prev.to_toml().unwrap());
}
