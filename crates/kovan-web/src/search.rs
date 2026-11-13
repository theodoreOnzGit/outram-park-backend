//! The search bar's matching: crates (from the code map), modules and
//! functions (from `search.json`), by name and path fragments.
//!
//! Scored with kovan's own fuzzy scorer, `kovan_common::fuzzy::fuzzy_score`
//! (the literature finder's, moved to kovan-common on 2026-10-06), once per
//! whitespace-separated term: every term must match the candidate's
//! `crate path name` text, and the scores add. So "live step" finds
//! `live_pools.rs::step`: "live" matches the module path, "step" the name.
//! A term that matches the name itself scores a bonus, so a function called
//! `step` ranks above one merely in a module mentioning it.
//!
//! A crate is also found by its backronym (GitHub #815), but only when every
//! term is a plain substring of it: "melt behaviour" finds sembawang. The
//! backronym is kept out of the fuzzy text, because a long phrase contains
//! almost any letter sequence and would match nearly every query.
//!
//! Test code is found too (GitHub #1164, 2026-11-03): a test module or
//! function is labelled `test` and ranks below an ordinary result with the
//! same score.

use kovan_common::call_graph::split::SearchIndex;
use kovan_common::code_map::CodeMap;
use kovan_common::fuzzy::fuzzy_score;

/// What a result points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hit {
    Crate(String),
    Module { krate: String, file: String },
    Function { krate: String, id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub hit: Hit,
    pub score: i32,
    /// `crate`, `module`, `fn` or `test`.
    pub kind: &'static str,
    /// Test code (#1164): choosing it shows the map's tests.
    pub test: bool,
    /// The name shown first.
    pub label: String,
    /// Crate and path.
    pub detail: String,
}

fn score(terms: &[String], haystack: &str, name: &str) -> Option<i32> {
    let mut total = 0;
    for t in terms {
        let s = fuzzy_score(t, haystack)?;
        total += s;
        if name.to_lowercase().contains(t.as_str()) {
            total += 200;
        }
    }
    Some(total)
}

/// The best `limit` matches for `query`, best first (ties: ordinary code
/// before tests, then by label).
pub fn search(query: &str, map: &CodeMap, index: Option<&SearchIndex>, limit: usize) -> Vec<SearchResult> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for c in &map.crates {
        let by_backronym = c
            .backronym
            .as_deref()
            .map(str::to_lowercase)
            .filter(|b| terms.iter().all(|t| b.contains(t.as_str())))
            .map(|_| 0);
        if let Some(s) = score(&terms, &c.name, &c.name).or(by_backronym) {
            out.push(SearchResult {
                hit: Hit::Crate(c.name.clone()),
                score: s + 100,
                kind: "crate",
                test: false,
                label: c.name.clone(),
                detail: c.backronym.clone().or_else(|| c.description.clone()).unwrap_or_default(),
            });
        }
    }
    if let Some(ix) = index {
        let test_modules: std::collections::BTreeSet<usize> = ix.test_modules.iter().copied().collect();
        let test_functions: std::collections::BTreeSet<usize> = ix.test_functions.iter().copied().collect();
        for (mi, m) in ix.modules.iter().enumerate() {
            let krate = &ix.crates[m.0];
            let name = if m.2.is_empty() { m.1.rsplit('/').next().unwrap_or(&m.1) } else { m.2.rsplit("::").next().unwrap_or(&m.2) };
            let hay = format!("{krate} {} {}", m.2, m.1);
            if let Some(s) = score(&terms, &hay, name) {
                out.push(SearchResult {
                    hit: Hit::Module { krate: krate.clone(), file: m.1.clone() },
                    score: s + 50,
                    kind: if test_modules.contains(&mi) { "test" } else { "module" },
                    test: test_modules.contains(&mi),
                    label: if m.2.is_empty() { m.1.clone() } else { m.2.clone() },
                    detail: format!("{krate} · {}", m.1),
                });
            }
        }
        for (fi, f) in ix.functions.iter().enumerate() {
            let m = &ix.modules[f.0];
            let krate = &ix.crates[m.0];
            let name = f.1.split('#').next().unwrap_or(&f.1);
            let hay = format!("{krate} {} {}", m.2, name);
            if let Some(s) = score(&terms, &hay, name) {
                out.push(SearchResult {
                    hit: Hit::Function { krate: krate.clone(), id: format!("{}::{}", m.1, f.1) },
                    score: s,
                    kind: if test_functions.contains(&fi) { "test" } else { "fn" },
                    test: test_functions.contains(&fi),
                    label: name.to_string(),
                    detail: format!("{krate} · {}:{}", m.1, f.2),
                });
            }
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.test.cmp(&b.test)).then_with(|| a.label.cmp(&b.label)));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::call_graph::split::{SearchFn, SearchModule};

    /// "live step" finds `live_pools.rs::step` first.
    #[test]
    fn path_fragments_and_name_find_the_function() {
        let map = CodeMap { root: "r".into(), crates: vec![], edges: vec![] };
        let ix = SearchIndex {
            schema: 1,
            crates: vec!["boon-lay".into()],
            modules: vec![
                SearchModule(0, "crates/boon-lay/src/a/live_pools.rs".into(), "a::live_pools".into()),
                SearchModule(0, "crates/boon-lay/src/b.rs".into(), "b".into()),
            ],
            functions: vec![
                SearchFn(0, "step".into(), 10),
                SearchFn(0, "Pool::new".into(), 20),
                SearchFn(1, "step".into(), 3),
            ],
            test_modules: vec![],
            test_functions: vec![],
        };
        let r = search("live step", &map, Some(&ix), 5);
        assert_eq!(r[0].hit, Hit::Function { krate: "boon-lay".into(), id: "crates/boon-lay/src/a/live_pools.rs::step".into() });
        // `b.rs::step` has no "live" in its crate, path or name.
        assert!(r.iter().all(|x| x.detail.contains("live_pools")));
        assert!(search("", &map, Some(&ix), 5).is_empty());
        assert!(search("zzzz", &map, Some(&ix), 5).is_empty());
    }

    /// "melt behaviour" finds sembawang by its backronym (#815), and a word
    /// that is not in the backronym does not.
    #[test]
    fn a_crate_is_found_by_its_backronym() {
        let node = |name: &str, backronym: Option<&str>| kovan_common::code_map::CrateNode {
            name: name.into(),
            description: Some(format!("the {name} crate")),
            backronym: backronym.map(Into::into),
            row: 3,
            topic: kovan_common::code_map::Topic::Risk,
            fidelity: None,
            maturity: 1,
            maturity_modules: vec![],
            lib_dir: None,
            dir: None,
        };
        let map = CodeMap {
            root: "r".into(),
            crates: vec![
                node("sembawang", Some("Severe-accident Evolution and Melt Behaviour Analysis Workbench for Advanced Nuclear Geometries")),
                node("changi", None),
            ],
            edges: vec![],
        };
        let r = search("melt behaviour", &map, None, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].hit, Hit::Crate("sembawang".into()));
        assert!(r[0].detail.starts_with("Severe-accident"), "the backronym is the detail line");
        assert!(search("melt dispersion", &map, None, 5).is_empty());
        // The name still matches as before, with the backronym as detail.
        assert_eq!(search("sembawang", &map, None, 5)[0].hit, Hit::Crate("sembawang".into()));
    }

    /// Methodology (#1164): the same name in a library module and in an
    /// integration-test module with the same path, so both score alike.
    /// Pass: both are found; the test is labelled `test`, marked, and
    /// ranked after the ordinary function; the test module is found and
    /// marked too.
    #[test]
    fn test_code_is_found_labelled_and_ranked_after_ordinary_code() {
        let map = CodeMap { root: "r".into(), crates: vec![], edges: vec![] };
        let ix = SearchIndex {
            schema: 1,
            crates: vec!["boon-lay".into()],
            modules: vec![
                SearchModule(0, "crates/boon-lay/tests/mhtgr.rs".into(), "mhtgr".into()),
                SearchModule(0, "crates/boon-lay/src/mhtgr.rs".into(), "mhtgr".into()),
            ],
            functions: vec![SearchFn(0, "paper_releases".into(), 9), SearchFn(1, "paper_releases".into(), 3)],
            test_modules: vec![0],
            test_functions: vec![0],
        };
        let r = search("paper releases", &map, Some(&ix), 5);
        let fns: Vec<&SearchResult> = r.iter().filter(|x| matches!(x.hit, Hit::Function { .. })).collect();
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].score, fns[1].score);
        assert_eq!((fns[0].kind, fns[0].test), ("fn", false));
        assert_eq!((fns[1].kind, fns[1].test), ("test", true));
        assert!(fns[1].detail.contains("tests/mhtgr.rs"));
        let m = search("mhtgr", &map, Some(&ix), 5);
        assert!(m.iter().any(|x| x.test && x.kind == "test" && matches!(&x.hit, Hit::Module { file, .. } if file.contains("tests/"))));
    }
}
