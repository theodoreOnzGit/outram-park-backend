use super::*;
use crate::code_index::folders::tests::{input, C1, C2, LIB, UTIL};
use crate::code_index::folders::build;

fn files(lib: &str, util: &str) -> BTreeMap<String, String> {
    [("lib.rs", lib), ("util.rs", util)]
        .iter()
        .map(|(f, t)| (f.to_string(), t.to_string()))
        .collect()
}

/// Methodology: the rust-analyzer-free refresh against a full build of the
/// fixture (`folders` tests). (1) Unchanged source: the refresh reproduces
/// the full build's `kovan.toml` byte for byte, nothing out of date. (2)
/// `twice` edited, a new function added, a line inserted above `leaf`:
/// `leaf` keeps its id, callees and reaching tests with its new lines and
/// is not flagged; `twice` keeps its id and committed callees but has a
/// new hash and is flagged; the new function is flagged with no callees.
/// (3) `util.rs` no longer parses: its previous entries are kept, flagged,
/// and the file reported. A refresh of a refreshed index keeps the flags.
///
/// Result (2026-10-07): passes.
#[test]
fn refresh_recomputes_hashes_and_flags_what_it_cannot_recompute() {
    let full = build(&input(C1));
    let prev = &full.folders["crates/kern/src"];
    let (same, r) = refresh_folder("kern", "crates/kern/src", Some(prev), &files(LIB, UTIL), None, &[], C2);
    assert_eq!(same.to_toml().unwrap(), prev.to_toml().unwrap());
    assert!(r.out_of_date.is_empty() && r.unparsed.is_empty());

    let edited = LIB
        .replace("pub mod util;\n", "pub mod util;\n// a note\n")
        .replace("leaf(x) + inner(x)", "leaf(x) - inner(x)")
        + "\npub fn fresh() {}\n";
    let (ix, r) = refresh_folder("kern", "crates/kern/src", Some(prev), &files(&edited, UTIL), None, &[], C2);
    let get = |i: &FolderIndex, n: &str| i.modules["lib.rs"].functions.iter().find(|f| f.name == n).cloned().unwrap();
    let (old_leaf, leaf) = (get(prev, "leaf"), get(&ix, "leaf"));
    assert_eq!((leaf.id.clone(), leaf.callees.clone(), leaf.reached_by.clone()), (old_leaf.id, old_leaf.callees, old_leaf.reached_by));
    assert_eq!(leaf.lines, [4, 7]);
    assert!(!leaf.index_out_of_date);
    let (old_twice, twice) = (get(prev, "twice"), get(&ix, "twice"));
    assert_eq!(twice.id, old_twice.id);
    assert_eq!(twice.callees, old_twice.callees);
    assert_ne!(twice.hash, old_twice.hash);
    assert!(twice.index_out_of_date);
    let fresh = get(&ix, "fresh");
    assert!(fresh.index_out_of_date && fresh.callees.is_empty());
    assert_eq!(
        r.out_of_date,
        vec!["crates/kern/src/lib.rs::fresh", "crates/kern/src/lib.rs::twice"]
    );
    // Round trip through text, and a second refresh keeps the flags.
    let reread = FolderIndex::parse(&ix.to_toml().unwrap()).unwrap();
    let (twice_again, _) = refresh_folder("kern", "crates/kern/src", Some(&reread), &files(&edited, UTIL), None, &[], C2);
    assert!(get(&twice_again, "twice").index_out_of_date);

    let (broken, r) = refresh_folder("kern", "crates/kern/src", Some(prev), &files(LIB, "fn ("), None, &[], C2);
    assert_eq!(r.unparsed, vec!["crates/kern/src/util.rs"]);
    let helper = &broken.modules["util.rs"].functions[0];
    assert_eq!(helper.id, prev.modules["util.rs"].functions[0].id);
    assert!(helper.index_out_of_date);

    // No previous index (a new folder): everything is flagged.
    let (new, r) = refresh_folder("kern", "crates/kern/src", None, &files(LIB, UTIL), None, &[], C2);
    assert_eq!(new.modules["lib.rs"].path, "?");
    assert_eq!(r.out_of_date.len(), new.functions().count());
}

/// Methodology (#843): the refresh on a folder with items. (1) Unchanged
/// source reproduces the full build byte for byte. (2) A method of
/// `Fractions` is edited: the struct keeps its id, has a new hash and is
/// flagged, as the method is; the enum, trait, alias and constant are
/// untouched. (3) A previous index written before items existed (functions
/// only): the refresh adds every item, flagged, and keeps every function
/// entry as it was.
///
/// Result (2026-10-13): passes.
#[test]
fn refresh_handles_items_like_functions() {
    use crate::code_index::folders::tests::{input_with_items, TYPES};
    let full = build(&input_with_items(C1));
    let prev = &full.folders["crates/kern/src"];
    let mut fs = files(LIB, UTIL);
    fs.insert("types.rs".into(), TYPES.into());
    let (same, r) = refresh_folder("kern", "crates/kern/src", Some(prev), &fs, None, &[], C2);
    assert_eq!(same.to_toml().unwrap(), prev.to_toml().unwrap());
    assert!(r.out_of_date.is_empty());

    fs.insert("types.rs".into(), TYPES.replace("        self.sic\n", "        self.sic * 1.0\n"));
    let (ix, r) = refresh_folder("kern", "crates/kern/src", Some(prev), &fs, None, &[], C2);
    let get = |i: &FolderIndex, q: &str| i.modules["types.rs"].functions.iter().find(|f| f.qual == q).cloned().unwrap();
    let (old, new) = (get(prev, "Fractions"), get(&ix, "Fractions"));
    assert_eq!(new.id, old.id);
    assert_ne!(new.hash, old.hash);
    assert!(new.index_out_of_date);
    for q in ["Regime", "Fluid", "Fraction", "SIGMA", "TOL"] {
        assert_eq!(get(&ix, q), get(prev, q), "{q}");
    }
    assert_eq!(
        r.out_of_date,
        vec!["crates/kern/src/types.rs::Fractions", "crates/kern/src/types.rs::Fractions::total"]
    );

    let mut before_items = prev.clone();
    for m in before_items.modules.values_mut() {
        m.functions.retain(|f| f.item.is_fn());
    }
    fs.insert("types.rs".into(), TYPES.into());
    let (ix, _) = refresh_folder("kern", "crates/kern/src", Some(&before_items), &fs, None, &[], C2);
    for (name, m) in &before_items.modules {
        let fns: Vec<&FunctionIndex> = ix.modules[name].functions.iter().filter(|f| f.item.is_fn()).collect();
        assert_eq!(fns, m.functions.iter().collect::<Vec<_>>(), "{name}");
    }
    let added = get(&ix, "SIGMA");
    assert!(added.index_out_of_date && added.shape.is_some());
}
