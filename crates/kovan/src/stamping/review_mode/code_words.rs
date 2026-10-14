//! **Completion words for a quick fix, from the cached index** (GitHub
//! #857). No language server: the maintainer does not want a second
//! rust-analyzer running ("too memory intensive", 2026-10-13).
//!
//! ```text
//!  every folder `kovan.toml` (already loaded and cached by
//!  `stamping::load_workspace`) ── functions, types, constants
//!       │ from_workspace(ws, file being fixed)
//!       v
//!  CodeWords: names sorted for a prefix lookup, each ranked
//!       0 this file   1 this crate   2 the rest of the workspace
//!       │ complete("isoth", names already in the buffer)
//!       v
//!  nearest first, shortest first: `isothermal_thinning_factor`
//! ```
//!
//! **What this cannot do, by design.** It knows names, not types: after
//! `layer.` it offers every known name with the typed prefix, not only the
//! methods of `layer`'s type, and it knows nothing outside this workspace
//! (no `std`, no dependencies from crates.io). Names already in the buffer
//! (locals, arguments) are offered first.

use std::collections::BTreeSet;

use kovan_common::review::index::FunctionIndex;

use crate::stamping::Workspace;

/// One name that can be completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeWord {
    pub name: String,
    /// Shown beside it: what it is and where it lives.
    pub detail: String,
    /// 0 the file being fixed, 1 its crate, 2 elsewhere.
    pub rank: u8,
}

/// The names of a workspace, sorted for prefix lookup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeWords {
    /// `(lower-case name, word)`, sorted by the lower-case name.
    sorted: Vec<(String, CodeWord)>,
}

/// The file part of an index path id (`crates/a/src/x.rs::Type::name`).
fn what(f: &FunctionIndex) -> &'static str {
    f.shape.map_or("function", |s| s.plain_name())
}

impl CodeWords {
    /// Every non-test function, type and constant of `ws`, ranked by how
    /// near it is to `file` (workspace-relative).
    pub fn from_workspace(ws: &Workspace, file: &str) -> Self {
        // A folder index names its files by file name, under its `dir`.
        let rows: Vec<(String, &str, &FunctionIndex)> = ws
            .indexes
            .iter()
            .flat_map(|ix| {
                ix.modules.iter().flat_map(move |(name, m)| {
                    let path = format!("{}/{name}", ix.dir);
                    m.functions.iter().map(move |f| (path.clone(), ix.krate.as_str(), f))
                })
            })
            .collect();
        let krate = rows.iter().find(|(p, _, _)| p == file).map(|(_, k, _)| *k);
        Self::from_entries(rows.iter().map(|(p, k, f)| (p.as_str(), Some(*k) == krate, *f)), file)
    }

    /// As [`Self::from_workspace`], from `(file, same crate, entry)` rows.
    pub fn from_entries<'a>(
        entries: impl Iterator<Item = (&'a str, bool, &'a FunctionIndex)>,
        file: &str,
    ) -> Self {
        let mut seen = BTreeSet::new();
        let mut sorted = Vec::new();
        for (path, same_crate, f) in entries {
            if f.test || f.name.is_empty() {
                continue;
            }
            let rank = if path == file { 0 } else if same_crate { 1 } else { 2 };
            let detail = format!("{} {} in {path}", what(f), f.qual);
            if seen.insert((f.name.clone(), detail.clone())) {
                sorted.push((f.name.to_lowercase(), CodeWord { name: f.name.clone(), detail, rank }));
            }
        }
        sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.rank.cmp(&b.1.rank)).then(a.1.detail.cmp(&b.1.detail)));
        Self { sorted }
    }

    pub fn len(&self) -> usize {
        self.sorted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    /// Up to `limit` words starting with `prefix` (any case), nearest first
    /// and then shortest first. `in_buffer` names (locals and arguments of
    /// the function being fixed) come before everything else. Nothing for an
    /// empty prefix, and never the prefix itself. ~~Two characters were
    /// needed~~ **CHANGED 2026-10-13**: the list now opens only when asked
    /// for (Ctrl+N / Ctrl+P), so one character is enough.
    pub fn complete(&self, prefix: &str, in_buffer: &[String], limit: usize) -> Vec<CodeWord> {
        if prefix.is_empty() {
            return Vec::new();
        }
        let p = prefix.to_lowercase();
        let mut out: Vec<CodeWord> = in_buffer
            .iter()
            .filter(|w| w.to_lowercase().starts_with(&p) && w.as_str() != prefix)
            .map(|w| CodeWord { name: w.clone(), detail: "in this function".into(), rank: 0 })
            .collect();
        let local: BTreeSet<String> = out.iter().map(|w| w.name.clone()).collect();
        let from = self.sorted.partition_point(|(k, _)| k.as_str() < p.as_str());
        let mut indexed: Vec<&CodeWord> = self.sorted[from..]
            .iter()
            .take_while(|(k, _)| k.starts_with(&p))
            .map(|(_, w)| w)
            .filter(|w| w.name != prefix)
            .collect();
        indexed.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.name.len().cmp(&b.name.len())).then(a.name.cmp(&b.name)));
        out.sort_by(|a, b| a.name.len().cmp(&b.name.len()).then(a.name.cmp(&b.name)));
        out.dedup();
        // A name the buffer already holds is listed once, with where it is defined.
        for w in indexed {
            match out.iter_mut().find(|o| o.name == w.name && local.contains(&o.name) && o.detail == "in this function") {
                Some(o) => o.detail = w.detail.clone(),
                None => out.push(w.clone()),
            }
        }
        out.truncate(limit);
        out
    }
}

/// The identifiers of `text` (at least two characters, not starting with a
/// digit), each once, in order of first appearance.
pub fn buffer_words(text: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    text.split(|c: char| !(c == '_' || c.is_alphanumeric()))
        .filter(|w| w.chars().count() >= 2 && !w.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .filter(|w| seen.insert(w.to_string()))
        .map(str::to_string)
        .collect()
}

/// The identifier being typed: the run of name characters that ends
/// `before` (the text of the line up to the cursor), with the number of
/// characters it holds. Empty when the cursor does not follow a name.
pub fn word_before(before: &str) -> &str {
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| *c == '_' || c.is_alphanumeric())
        .last()
        .map_or(before.len(), |(i, _)| i);
    let w = &before[start..];
    if w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        ""
    } else {
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::review::index::{ItemKind, ItemShape};

    fn entry(name: &str, qual: &str, shape: Option<ItemShape>, test: bool) -> FunctionIndex {
        FunctionIndex {
            id: format!("fn:{qual}"),
            name: name.into(),
            qual: qual.into(),
            item: if shape.is_some() { ItemKind::Type } else { ItemKind::Fn },
            shape,
            lines: [1, 2],
            hash: String::new(),
            doc_hash: String::new(),
            callees: Vec::new(),
            uses: Vec::new(),
            reached_by: Vec::new(),
            test,
            index_out_of_date: false,
            physical_interface: false,
        }
    }

    /// **Methodology.** An index of six entries over three files (the file
    /// being fixed, another file of its crate, another crate) is asked for
    /// `"isoth"`, `"sic"`, a one-letter prefix, nothing and an exact name. Pass: the
    /// order is this file, this crate, elsewhere; a test is never offered;
    /// matching ignores case; nothing typed gives nothing and the exact
    /// name is never offered back.
    ///
    /// **Result (2026-10-13).** As stated.
    #[test]
    fn words_come_nearest_first_and_tests_are_left_out() {
        let e = [
            ("a/src/geometry.rs", true, entry("isothermal_thinning_factor", "SicLayer::isothermal_thinning_factor", None, false)),
            ("a/src/stress.rs", true, entry("isothermal_hold", "isothermal_hold", None, false)),
            ("b/src/lib.rs", false, entry("isotherm", "isotherm", None, false)),
            ("a/src/geometry.rs", true, entry("isothermal_is_tested", "isothermal_is_tested", None, true)),
            ("a/src/geometry.rs", true, entry("SicLayer", "SicLayer", Some(ItemShape::Struct), false)),
            ("b/src/lib.rs", false, entry("SicLayerError", "SicLayerError", Some(ItemShape::Enum), false)),
        ];
        let w = CodeWords::from_entries(e.iter().map(|(p, s, f)| (*p, *s, f)), "a/src/geometry.rs");
        assert_eq!(w.len(), 5, "the test is left out");
        let names = |v: Vec<CodeWord>| v.into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names(w.complete("isoth", &[], 10)), ["isothermal_thinning_factor", "isothermal_hold", "isotherm"]);
        let sic = w.complete("sic", &[], 10);
        assert_eq!(names(sic.clone()), ["SicLayer", "SicLayerError"]);
        assert_eq!(sic[0].detail, "record SicLayer in a/src/geometry.rs");
        assert_eq!(w.complete("i", &[], 10).len(), 3, "one letter is enough when asked");
        assert!(w.complete("", &[], 10).is_empty(), "nothing typed, nothing offered");
        assert_eq!(names(w.complete("SicLayer", &[], 10)), ["SicLayerError"], "never the word already typed");
        assert_eq!(w.complete("isoth", &[], 2).len(), 2, "the limit holds");
        assert!(w.complete("zzz", &[], 10).is_empty());
    }

    /// Names already in the buffer come first, and one that the index also
    /// knows is listed once, with where it is defined.
    #[test]
    fn names_in_the_buffer_come_first() {
        let e = [("a/src/x.rs", true, entry("thickness_now", "thickness_now", None, false))];
        let w = CodeWords::from_entries(e.iter().map(|(p, s, f)| (*p, *s, f)), "a/src/y.rs");
        let buf = buffer_words("let thick = 2.0 * thin; thickness_now(thick) // 35 um");
        assert_eq!(buf, ["let", "thick", "thin", "thickness_now", "um"]);
        let got = w.complete("thi", &buf, 10);
        assert_eq!(got.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["thin", "thick", "thickness_now"]);
        assert_eq!(got[0].detail, "in this function");
        assert_eq!(got[2].detail, "function thickness_now in a/src/x.rs");
    }

    /// **Methodology.** The words are built from this repository's own
    /// committed index, as a quick fix on boon-lay's `stress.rs` would, and
    /// `"cbrt"` is asked for. Pass: `cbrt_mean_radius` (same crate, defined
    /// in `geometry.rs`) is offered, and `induced_stress` (same file) ranks
    /// as this file's. The `cbrt_mean_radius` half is skipped on an index
    /// that predates that rename (2026-10-13).
    ///
    /// **Result (2026-10-13).** Both hold on the freshly indexed tree:
    /// more than 1000 names, `induced_stress` at rank 0.
    #[test]
    fn the_real_index_ranks_this_file_first() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ws = crate::stamping::load_workspace_uncached(&dunce::canonicalize(root).unwrap()).expect("this repository loads");
        let file = "crates/boon-lay/src/fuel_failure/stress.rs";
        let w = CodeWords::from_workspace(&ws, file);
        assert!(w.len() > 1000, "a workspace of names: {}", w.len());
        let stress = w.complete("induced_str", &[], 12);
        let first = stress.first().expect("induced_stress is indexed");
        assert_eq!((first.name.as_str(), first.rank), ("induced_stress", 0), "{first:?}");
        assert!(first.detail.ends_with(file), "{}", first.detail);
        if let Some(c) = w.complete("cbrt_mean", &[], 12).first() {
            assert_eq!((c.name.as_str(), c.rank), ("cbrt_mean_radius", 1), "{c:?}");
        }
    }

    #[test]
    fn the_word_being_typed_ends_at_the_cursor() {
        assert_eq!(word_before("    layer.isoth"), "isoth");
        assert_eq!(word_before("let σ_t = cbrt_me"), "cbrt_me");
        assert_eq!(word_before("x + "), "");
        assert_eq!(word_before("x = 12"), "", "a number is not a name");
        assert_eq!(word_before(""), "");
    }
}
