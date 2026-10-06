//! **Tests and examples that reach a function** (GitHub #746, part 1): the
//! call graph walked forwards from every test and every example, recorded
//! on each function they reach.
//!
//! # Entry points
//!
//! - A **test** is a function carrying a test attribute (`#[test]`,
//!   `#[tokio::test]`, `#[rstest]`: [`super::Function::test_fn`]), in the
//!   lib's `#[cfg(test)]` code, in an example, or in an integration-test
//!   target under `tests/` (schema 2 builds those; see
//!   [`super::CrateGraph::tests`]). Helpers in test code are walked through
//!   but are not entry points.
//! - An **example** is a whole example target: every non-test function in it
//!   is a starting point at hop 0, because an egui example's code is mostly
//!   entered through trait callbacks (`eframe::App::update`) that the graph
//!   does not resolve, so a walk from `main` alone would miss most of it.
//!   `hops` is the distance from the nearest function of the example, and
//!   `via` names that function (the smallest id at that distance).
//!
//! # What is recorded
//!
//! On every non-test function reached by at least one entry: the number of
//! tests and of examples that reach it, and the nearest [`REACH_CAP`] of
//! each, sorted by `(hops, id)`. A direct call from a test is 1 hop. An
//! example's own functions are not listed as reached by that example.
//!
//! # A lower bound
//!
//! The walk follows only **resolved** calls (`calls`, both direct calls and
//! function values). Calls the tool marked `UNRESOLVED` (closures, trait
//! methods, derived methods, macro bodies) are not followed, and calls into
//! functions outside the scope stop there. So a function with no entry
//! here may still be exercised by a test; the lists are a lower bound.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::{CallGraphDoc, TargetKind};

/// How many of the nearest tests (and examples) each function lists.
pub const REACH_CAP: usize = 10;

/// The entries that reach one function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reach {
    /// All tests that reach the function.
    pub tests_total: usize,
    /// The nearest [`REACH_CAP`], by `(hops, id)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<TestReach>,
    /// All example targets that reach the function.
    pub examples_total: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleReach>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TestReach {
    pub hops: u32,
    /// The test function's id.
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExampleReach {
    pub hops: u32,
    #[serde(rename = "crate")]
    pub krate: String,
    /// The example target's name.
    pub example: String,
    /// The example's function nearest to this one.
    pub via: String,
}

/// Breadth-first distances from `starts` (all at hop 0).
fn bfs<'a>(
    adj: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    starts: &[&'a str],
) -> BTreeMap<&'a str, (u32, &'a str)> {
    let mut seen: BTreeMap<&str, (u32, &str)> = BTreeMap::new();
    let mut q = VecDeque::new();
    let mut sorted = starts.to_vec();
    sorted.sort();
    for s in sorted {
        if !seen.contains_key(s) {
            seen.insert(s, (0, s));
            q.push_back(s);
        }
    }
    while let Some(n) = q.pop_front() {
        let (d, origin) = seen[n];
        if let Some(next) = adj.get(n) {
            for &m in next {
                // Keep the smallest origin among the shortest paths: every
                // node of layer d is final before layer d is expanded (FIFO),
                // so by induction `origin` is the minimum for `n`, and a
                // layer-(d+1) node takes the minimum over its parents.
                match seen.get(m) {
                    None => {
                        seen.insert(m, (d + 1, origin));
                        q.push_back(m);
                    }
                    Some(&(dm, om)) if dm == d + 1 && origin < om => {
                        seen.insert(m, (dm, origin));
                    }
                    _ => {}
                }
            }
        }
    }
    seen
}

/// Fills `reached_by` on every non-test function of `doc`.
pub fn fill(doc: &mut CallGraphDoc) {
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for c in &doc.calls {
        if c.from != c.to {
            adj.entry(c.from.as_str())
                .or_default()
                .insert(c.to.as_str());
        }
    }
    let mut tests: Vec<&str> = Vec::new();
    // (crate, example) -> its non-test functions.
    let mut examples: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for c in &doc.crates {
        for t in c.targets.iter().chain(c.tests.iter()) {
            for m in &t.modules {
                for f in &m.functions {
                    if f.test_fn {
                        tests.push(f.id.as_str());
                    } else if t.kind == TargetKind::Example && !f.test {
                        examples
                            .entry((c.name.as_str(), t.name.as_str()))
                            .or_default()
                            .push(f.id.as_str());
                    }
                }
            }
        }
    }
    tests.sort();
    tests.dedup();
    let mut by_fn: BTreeMap<String, (Vec<TestReach>, Vec<ExampleReach>)> = BTreeMap::new();
    for t in &tests {
        for (n, (d, _)) in bfs(&adj, &[t]) {
            if d > 0 {
                by_fn.entry(n.to_string()).or_default().0.push(TestReach {
                    hops: d,
                    id: t.to_string(),
                });
            }
        }
    }
    for ((krate, ex), fns) in &examples {
        let own: BTreeSet<&str> = fns.iter().copied().collect();
        for (n, (d, via)) in bfs(&adj, fns) {
            if !own.contains(n) {
                by_fn
                    .entry(n.to_string())
                    .or_default()
                    .1
                    .push(ExampleReach {
                        hops: d,
                        krate: krate.to_string(),
                        example: ex.to_string(),
                        via: via.to_string(),
                    });
            }
        }
    }
    for c in &mut doc.crates {
        for t in c.targets.iter_mut().chain(c.tests.iter_mut()) {
            for m in &mut t.modules {
                for f in &mut m.functions {
                    f.reached_by = None;
                    if f.test {
                        continue;
                    }
                    let Some((mut ts, mut es)) = by_fn.remove(&f.id) else {
                        continue;
                    };
                    ts.sort();
                    es.sort();
                    let (tt, et) = (ts.len(), es.len());
                    ts.truncate(REACH_CAP);
                    es.truncate(REACH_CAP);
                    f.reached_by = Some(Reach {
                        tests_total: tt,
                        tests: ts,
                        examples_total: et,
                        examples: es,
                    });
                }
            }
        }
    }
}
