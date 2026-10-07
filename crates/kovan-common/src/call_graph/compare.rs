//! Edge-by-edge comparison of two call-graph documents (GitHub #757): the
//! instrument that judges the SCIP backend against the LSP one, and that a
//! CI check can use to compare a regenerated graph with a committed one.
//!
//! Edges are matched by `(from, to)` function id. Both backends take their
//! ids from the same source scanner, so an id names the same function in
//! both documents; the call kind is compared separately (an edge one
//! backend calls `call` and the other `fn_value` is still the same edge).
//! Unresolved calls are matched by `(function, line, kind)`.
//!
//! Pure data, no I/O.

use std::collections::{BTreeMap, BTreeSet};

use super::{CallGraphDoc, CallKind};

/// Edge counts for one caller crate (or the total).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdgeCounts {
    /// Edges in both documents.
    pub both: usize,
    /// Of those, the ones whose call-site lines are identical.
    pub same_lines: usize,
    /// Of those, the ones whose call kind differs.
    pub kind_differs: usize,
    pub only_a: usize,
    pub only_b: usize,
}

impl EdgeCounts {
    /// `both / (both + only_a)`: the share of A's edges that B has.
    pub fn recall_of_a(&self) -> f64 {
        ratio(self.both, self.both + self.only_a)
    }

    /// `both / (both + only_a + only_b)`.
    pub fn jaccard(&self) -> f64 {
        ratio(self.both, self.both + self.only_a + self.only_b)
    }
}

fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 {
        1.0
    } else {
        n as f64 / d as f64
    }
}

/// An edge found in one document only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeOnly {
    pub from: String,
    pub to: String,
    pub kind: CallKind,
    pub lines: Vec<u32>,
}

impl EdgeOnly {
    /// A function calling itself.
    pub fn is_self_edge(&self) -> bool {
        self.from == self.to
    }
}

/// Unresolved-call counts for one kind (`trait`, `closure`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnresolvedCounts {
    pub a: usize,
    pub b: usize,
    /// Same function, line and kind in both.
    pub both: usize,
}

/// The comparison of document A (the reference) with document B.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comparison {
    pub functions_a: usize,
    pub functions_b: usize,
    /// Function ids in both.
    pub functions_both: usize,
    pub total: EdgeCounts,
    /// By the caller's crate.
    pub per_crate: BTreeMap<String, EdgeCounts>,
    /// Sorted.
    pub only_a: Vec<EdgeOnly>,
    pub only_b: Vec<EdgeOnly>,
    pub unresolved: BTreeMap<String, UnresolvedCounts>,
}

impl Comparison {
    /// Counts of `only_a` (or `only_b`) by call kind.
    pub fn only_by_kind(edges: &[EdgeOnly]) -> BTreeMap<CallKind, usize> {
        let mut m = BTreeMap::new();
        for e in edges {
            *m.entry(e.kind).or_default() += 1;
        }
        m
    }
}

/// Compares `a` (the reference) with `b`.
pub fn compare(a: &CallGraphDoc, b: &CallGraphDoc) -> Comparison {
    let mut crate_of: BTreeMap<&str, &str> = BTreeMap::new();
    let ids = |d: &CallGraphDoc| -> BTreeSet<String> {
        d.functions().map(|(_, _, f)| f.id.clone()).collect()
    };
    for d in [a, b] {
        for (c, _, f) in d.functions() {
            crate_of.entry(f.id.as_str()).or_insert(c.name.as_str());
        }
    }
    let (ia, ib) = (ids(a), ids(b));
    let edges = |d: &CallGraphDoc| -> BTreeMap<(String, String), (CallKind, Vec<u32>)> {
        let mut m: BTreeMap<(String, String), (CallKind, Vec<u32>)> = BTreeMap::new();
        for c in &d.calls {
            let e = m
                .entry((c.from.clone(), c.to.clone()))
                .or_insert((c.kind, Vec::new()));
            // Two kinds on one pair (a call and a fn value): merge the
            // lines, keep the first kind.
            e.1.extend(c.lines.iter().copied());
            e.1.sort_unstable();
            e.1.dedup();
        }
        m
    };
    let (ea, eb) = (edges(a), edges(b));
    let mut cmp = Comparison {
        functions_a: ia.len(),
        functions_b: ib.len(),
        functions_both: ia.intersection(&ib).count(),
        ..Comparison::default()
    };
    let crate_name = |from: &str| crate_of.get(from).copied().unwrap_or("?").to_string();
    for (k, (ka, la)) in &ea {
        let pc = cmp.per_crate.entry(crate_name(&k.0)).or_default();
        match eb.get(k) {
            Some((kb, lb)) => {
                pc.both += 1;
                pc.same_lines += usize::from(la == lb);
                pc.kind_differs += usize::from(ka != kb);
            }
            None => {
                pc.only_a += 1;
                cmp.only_a.push(EdgeOnly {
                    from: k.0.clone(),
                    to: k.1.clone(),
                    kind: *ka,
                    lines: la.clone(),
                });
            }
        }
    }
    for (k, (kb, lb)) in &eb {
        if !ea.contains_key(k) {
            cmp.per_crate.entry(crate_name(&k.0)).or_default().only_b += 1;
            cmp.only_b.push(EdgeOnly {
                from: k.0.clone(),
                to: k.1.clone(),
                kind: *kb,
                lines: lb.clone(),
            });
        }
    }
    for c in cmp.per_crate.values() {
        cmp.total.both += c.both;
        cmp.total.same_lines += c.same_lines;
        cmp.total.kind_differs += c.kind_differs;
        cmp.total.only_a += c.only_a;
        cmp.total.only_b += c.only_b;
    }
    let gaps = |d: &CallGraphDoc| -> BTreeSet<(String, u32, String)> {
        d.functions()
            .flat_map(|(_, _, f)| {
                f.unresolved
                    .iter()
                    .map(move |u| (f.id.clone(), u.line, u.kind.clone()))
            })
            .collect()
    };
    let (ga, gb) = (gaps(a), gaps(b));
    for g in &ga {
        let u = cmp.unresolved.entry(g.2.clone()).or_default();
        u.a += 1;
        u.both += usize::from(gb.contains(g));
    }
    for g in &gb {
        cmp.unresolved.entry(g.2.clone()).or_default().b += 1;
    }
    cmp
}

#[cfg(test)]
mod tests {
    use super::super::tests::fixture;
    use super::super::{Backend, CallGraphDoc, RawCall, Unresolved};
    use super::*;

    /// Methodology: the shared two-crate fixture as A; B drops one edge,
    /// moves a call site, adds an operator edge and a closure gap, and
    /// changes one edge's kind. Every difference must be counted once, in
    /// its caller's crate.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn counts_every_difference_once() {
        let (crates, calls, outside) = fixture();
        let a = CallGraphDoc::assemble(crates.clone(), calls.clone(), outside.clone(), 0);
        let mut calls_b: Vec<RawCall> = calls
            .into_iter()
            .filter(|c| !c.from.ends_with("demo.rs::main"))
            .map(|mut c| {
                if c.to.ends_with("run.rs::go") {
                    c.line = 12;
                }
                if c.to.ends_with("lib.rs::help") {
                    c.kind = CallKind::FnValue;
                }
                c
            })
            .collect();
        calls_b.push(RawCall {
            from: "crates/app/src/run.rs::step".into(),
            to: "crates/core/src/lib.rs::leaf".into(),
            kind: CallKind::Operator,
            line: 10,
        });
        let mut crates_b = crates;
        crates_b[0].targets[1].modules[0].functions[1]
            .unresolved
            .push(Unresolved {
                line: 3,
                kind: "closure".into(),
                callee: "f".into(),
                detail: String::new(),
            });
        let mut b = CallGraphDoc::assemble(crates_b, calls_b, outside, 0);
        b.generator = Some(super::super::Generator {
            backend: Backend::Scip,
            rust_analyzer: String::new(),
        });
        let c = compare(&a, &b);
        assert_eq!((c.functions_a, c.functions_b, c.functions_both), (4, 4, 4));
        assert_eq!(c.total.both, 3);
        assert_eq!(c.total.only_a, 1);
        assert_eq!(c.total.only_b, 1);
        assert_eq!(c.total.same_lines, 2, "step -> go moved from line 11 to 12");
        assert_eq!(c.total.kind_differs, 1);
        assert_eq!(c.only_a[0].from, "crates/app/examples/demo.rs::main");
        assert_eq!(Comparison::only_by_kind(&c.only_b)[&CallKind::Operator], 1);
        assert_eq!(c.per_crate["app"].only_a, 1);
        assert_eq!(
            c.unresolved["trait"],
            UnresolvedCounts {
                a: 1,
                b: 1,
                both: 1
            }
        );
        assert_eq!(
            c.unresolved["closure"],
            UnresolvedCounts {
                a: 0,
                b: 1,
                both: 0
            }
        );
        assert!((c.total.recall_of_a() - 0.75).abs() < 1e-12);
        let same = compare(&a, &a);
        assert_eq!(
            (same.total.only_a, same.total.only_b, same.total.jaccard()),
            (0, 0, 1.0)
        );
    }
}
