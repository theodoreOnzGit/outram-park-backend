//! The **other reviewable items** of a source file for the call-graph
//! document (schema 4, GitHub #843): structs, enums, unions, traits, type
//! aliases, constants and statics, so the Code Review map can show how a
//! type is defined beside the functions that use it.
//!
//! The items are the ones the review hasher lists
//! ([`crate::review::rust_items`], "Which other items exist"), so a card on
//! the map and an entry in `kovan.toml` are always the same item with the
//! same id: `file::Name`, by [`super::function_ids`] over the file's items
//! in source order, with one whose id a function of the file already holds
//! left out ([`crate::code_index::folders`] applies the same rule).
//!
//! Each is written as a [`Function`] with `shape` set: `kind` is `free`,
//! `start_line..=end_line` is the definition with its doc comment, `line`
//! is the line the name is on, `signature` is that line trimmed, `doc` the
//! first sentence of the doc comment and `source` the lines as written.

use std::collections::BTreeSet;

use super::{function_ids, FnKind, Function};
use crate::review::rust_items::parse_file;

/// The first sentence of a normalised doc text (its first paragraph up to
/// the first `. `, or the whole paragraph).
fn first_sentence(doc: &str) -> String {
    let para = doc.split("\n\n").next().unwrap_or("");
    match para.find(". ") {
        Some(i) => para[..=i].to_string(),
        None => para.to_string(),
    }
}

/// The items of `file` (module doc). `fn_ids` are the ids of the file's
/// functions; `module_test` marks every item as test code. A file that does
/// not parse has none.
pub fn items_of(file: &str, text: &str, fn_ids: &BTreeSet<String>, module_test: bool) -> Vec<Function> {
    let Ok(parsed) = parse_file(text) else {
        return Vec::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let quals: Vec<String> = parsed.items.iter().map(|i| i.name.clone()).collect();
    let mut out = Vec::new();
    for (it, (id, ambiguous)) in parsed.items.iter().zip(function_ids(file, &quals)) {
        if fn_ids.contains(&id) {
            continue;
        }
        let [start, end] = it.lines;
        let slice = |a: u32, b: u32| -> &[&str] {
            let a = (a.max(1) as usize - 1).min(lines.len());
            let b = (b as usize).min(lines.len()).max(a);
            &lines[a..b]
        };
        let name_line = it.name_line.clamp(start, end);
        out.push(Function {
            id,
            ambiguous,
            name: it.name.clone(),
            kind: FnKind::Free,
            owner: None,
            trait_name: None,
            test: module_test || it.is_test,
            test_fn: false,
            start_line: start,
            line: name_line,
            end_line: end,
            signature: slice(name_line, name_line).first().map(|l| l.trim().to_string()).unwrap_or_default(),
            doc: first_sentence(&it.doc),
            source: slice(start, end).join("\n"),
            unresolved: Vec::new(),
            reached_by: None,
            cited_by: Vec::new(),
            shape: it.shape,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::index::ItemShape;

    /// Methodology: a file with a documented struct, a constant whose name
    /// a function id already holds, an enum under `#[cfg(test)]` and one
    /// function. Pass: the struct is listed with its id, its line range
    /// from the doc comment to the closing brace, the declaration line as
    /// signature, the first doc sentence and its source as written; the
    /// constant is left out; the enum is test code; the function is not an
    /// item; a file that does not parse gives none.
    ///
    /// Result (2026-10-13): passes.
    #[test]
    fn items_carry_their_definition() {
        let src = "/// Failure fractions. One per layer.\n#[derive(Debug)]\npub struct Fractions {\n    pub sic: f64,\n}\n\nconst taken: u8 = 1;\n\n#[cfg(test)]\nenum E { A }\n\nfn f() {}\n";
        let fn_ids: BTreeSet<String> = ["a/b.rs::f".to_string(), "a/b.rs::taken".to_string()].into();
        let got = items_of("a/b.rs", src, &fn_ids, false);
        assert_eq!(got.len(), 2);
        let s = &got[0];
        assert_eq!(s.id, "a/b.rs::Fractions");
        assert_eq!((s.start_line, s.line, s.end_line), (1, 3, 5));
        assert_eq!(s.signature, "pub struct Fractions {");
        assert_eq!(s.doc, "Failure fractions.");
        assert_eq!(s.source, "/// Failure fractions. One per layer.\n#[derive(Debug)]\npub struct Fractions {\n    pub sic: f64,\n}");
        assert_eq!(s.shape, Some(ItemShape::Struct));
        assert!(!s.test && got[1].test && got[1].name == "E");
        assert!(items_of("a/b.rs", src, &fn_ids, true).iter().all(|i| i.test));
        assert!(items_of("a/b.rs", "fn (", &fn_ids, false).is_empty());
    }
}
