//! The call graph `code-walk` builds, and the two searches over it: the
//! shortest chain(s) between two functions, and the de-duplicated tree of
//! everything an entry point reaches.
//!
//! Pure data and pure algorithms — no rust-analyzer, no file I/O — so the
//! searches are unit-tested on synthetic graphs (`tests` below).

use std::collections::VecDeque;

use serde::Serialize;

/// Index of a function node in [`CallGraph::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct NodeId(pub(crate) usize);

/// A workspace function the walk reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FnNode {
    /// Workspace-relative path, `/`-separated.
    pub(crate) file: String,
    /// `Type::method` or `function`.
    pub(crate) qualname: String,
    /// 1-based line of the declaration's identifier.
    pub(crate) line: u32,
    pub(crate) signature: String,
    pub(crate) doc: String,
}

impl FnNode {
    /// `file::qualname`, the form `--from`/`--to` and hand-filled hops use.
    pub(crate) fn key(&self) -> String {
        format!("{}::{}", self.file, self.qualname)
    }

    /// `keff.rs::transport_history` — the short display label.
    pub(crate) fn label(&self) -> String {
        let base = self.file.rsplit('/').next().unwrap_or(&self.file);
        format!("{base}::{}", self.qualname)
    }
}

/// How a call edge was established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum EdgeKind {
    /// A call expression rust-analyzer resolved.
    Call,
    /// A function passed by value (`.map(f)`), resolved by rust-analyzer.
    FnValue,
    /// A hop a person (or a lesson-writing agent) filled in by hand, with
    /// their note; never produced by the tool itself.
    Hand { note: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Edge {
    pub(crate) from: NodeId,
    pub(crate) to: NodeId,
    /// 1-based line of the call site in `from`'s file (the declaration line
    /// of `from` for a hand-filled hop, which has no call site).
    pub(crate) call_line: u32,
    #[serde(flatten)]
    pub(crate) kind: EdgeKind,
}

/// Why a call could not be followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GapKind {
    /// Resolves to a trait method declaration; the implementor is chosen at
    /// run time or by a generic bound.
    Trait,
    /// A call through a local closure, a closure/`fn` parameter, or a
    /// parenthesised expression (`(self.f)(x)`).
    Closure,
    /// A workspace macro; its expansion is not followed.
    Macro,
    /// rust-analyzer returned no definition for a call-shaped token.
    NoDefinition,
    /// Resolved to something in the workspace that is not a function body
    /// the walk can enter (a `static` fn pointer, a field, ...).
    Other,
}

impl GapKind {
    pub(crate) fn tag(self) -> &'static str {
        match self {
            GapKind::Trait => "trait",
            GapKind::Closure => "closure",
            GapKind::Macro => "macro",
            GapKind::NoDefinition => "no-definition",
            GapKind::Other => "other",
        }
    }
}

/// A call the walk saw but could not follow. Always reported, never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Gap {
    pub(crate) from: NodeId,
    /// 1-based line of the call site in `from`'s file.
    pub(crate) call_line: u32,
    pub(crate) kind: GapKind,
    /// The token at the call site (`volume`, `my_macro!`, `(expression)`).
    pub(crate) callee: String,
    /// Where rust-analyzer pointed, if anywhere: workspace path and 1-based
    /// line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target: Option<(String, u32)>,
    pub(crate) detail: String,
}

/// A partially-expanded call graph. A node is *expanded* once its body has
/// been scanned and all its edges and gaps recorded.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct CallGraph {
    pub(crate) nodes: Vec<FnNode>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) gaps: Vec<Gap>,
    #[serde(skip)]
    pub(crate) expanded: Vec<bool>,
}

impl CallGraph {
    /// Adds `node` (or returns the existing id for the same file and line).
    pub(crate) fn intern(&mut self, node: FnNode) -> NodeId {
        if let Some(i) = self
            .nodes
            .iter()
            .position(|n| n.file == node.file && n.line == node.line)
        {
            return NodeId(i);
        }
        self.nodes.push(node);
        self.expanded.push(false);
        NodeId(self.nodes.len() - 1)
    }

    /// Adds an edge unless one between the same pair already exists (the
    /// first call site wins).
    pub(crate) fn add_edge(&mut self, edge: Edge) {
        if !self
            .edges
            .iter()
            .any(|e| e.from == edge.from && e.to == edge.to)
        {
            self.edges.push(edge);
        }
    }

    pub(crate) fn node(&self, id: NodeId) -> &FnNode {
        &self.nodes[id.0]
    }

    /// Outgoing edges of `id`, in call-site order.
    pub(crate) fn out_edges(&self, id: NodeId) -> Vec<&Edge> {
        let mut v: Vec<&Edge> = self.edges.iter().filter(|e| e.from == id).collect();
        v.sort_by_key(|e| (e.call_line, e.to));
        v
    }

    /// Gaps recorded in `id`'s body, in call-site order.
    pub(crate) fn gaps_of(&self, id: NodeId) -> Vec<&Gap> {
        let mut v: Vec<&Gap> = self.gaps.iter().filter(|g| g.from == id).collect();
        v.sort_by_key(|g| g.call_line);
        v
    }

    /// The edge `from -> to` (first call site).
    pub(crate) fn edge(&self, from: NodeId, to: NodeId) -> Option<&Edge> {
        self.edges.iter().find(|e| e.from == from && e.to == to)
    }
}

/// Every shortest chain `from -> ... -> to` over the graph's edges, at most
/// `max_paths` of them, in deterministic (call-site) order. Empty if `to` is
/// unreachable in the graph as built.
///
/// Breadth-first from `from` recording *all* predecessors at the minimal
/// distance, then a depth-first enumeration of those predecessor lists. The
/// graph needs to be expanded only up to the target's layer: every edge out
/// of a node nearer than the target is present, which is all a shortest path
/// can use.
pub(crate) fn shortest_paths(
    g: &CallGraph,
    from: NodeId,
    to: NodeId,
    max_paths: usize,
) -> Vec<Vec<NodeId>> {
    let n = g.nodes.len();
    let mut dist = vec![usize::MAX; n];
    let mut preds: Vec<Vec<NodeId>> = vec![Vec::new(); n];
    let mut queue = VecDeque::new();
    dist[from.0] = 0;
    queue.push_back(from);
    while let Some(u) = queue.pop_front() {
        if dist[u.0] >= dist[to.0] {
            continue;
        }
        for e in g.out_edges(u) {
            let v = e.to;
            if dist[v.0] == usize::MAX {
                dist[v.0] = dist[u.0] + 1;
                preds[v.0].push(u);
                queue.push_back(v);
            } else if dist[v.0] == dist[u.0] + 1 && !preds[v.0].contains(&u) {
                preds[v.0].push(u);
            }
        }
    }
    if dist[to.0] == usize::MAX {
        return Vec::new();
    }
    if from == to {
        return vec![vec![from]];
    }
    // Mark the shortest-path DAG: nodes from which `to` is reached along
    // predecessor links.
    let mut on_dag = vec![false; n];
    let mut stack = vec![to];
    on_dag[to.0] = true;
    while let Some(v) = stack.pop() {
        for &u in &preds[v.0] {
            if !on_dag[u.0] {
                on_dag[u.0] = true;
                stack.push(u);
            }
        }
    }
    // Enumerate forwards in call-site order, so the first `max_paths`
    // chains are the ones that read in source order.
    let mut out: Vec<Vec<NodeId>> = Vec::new();
    let mut frames: Vec<Vec<NodeId>> = vec![vec![from]];
    while let Some(path) = frames.pop() {
        if out.len() >= max_paths {
            break;
        }
        let u = *path.last().unwrap_or(&from);
        if u == to {
            out.push(path);
            continue;
        }
        let next: Vec<NodeId> = g
            .out_edges(u)
            .iter()
            .map(|e| e.to)
            .filter(|&v| on_dag[v.0] && dist[v.0] == dist[u.0] + 1 && preds[v.0].contains(&u))
            .collect();
        for &v in next.iter().rev() {
            let mut p = path.clone();
            p.push(v);
            frames.push(p);
        }
    }
    out
}

/// One line of a rendered walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeLine {
    /// A function shown in full. `via` is the edge that reached it (`None`
    /// for the root); `truncated` is set when it has calls the depth limit
    /// kept from being shown.
    Node {
        depth: usize,
        node: NodeId,
        via: Option<Edge>,
        truncated: bool,
    },
    /// A function already shown elsewhere in this walk.
    BackRef {
        depth: usize,
        node: NodeId,
        via: Edge,
    },
    /// An unresolved call, as a leaf.
    Gap { depth: usize, gap: Gap },
}

/// The exhaustive tree from `root` to `max_depth` hops, de-duplicated: each
/// function is expanded once, under the caller that reaches it in the fewest
/// hops (ties broken by call-site order), and every other call to it is a
/// [`TreeLine::BackRef`]. Gaps are listed under the function they occur in.
///
/// Expanding each function under its breadth-first parent, rather than
/// wherever a depth-first walk first meets it, guarantees no function's
/// subtree is hidden behind a back-reference at a depth where the limit cut
/// it off.
pub(crate) fn tree(g: &CallGraph, root: NodeId, max_depth: usize) -> Vec<TreeLine> {
    let n = g.nodes.len();
    let mut depth = vec![usize::MAX; n];
    let mut parent: Vec<Option<NodeId>> = vec![None; n];
    let mut queue = VecDeque::new();
    depth[root.0] = 0;
    queue.push_back(root);
    while let Some(u) = queue.pop_front() {
        if depth[u.0] >= max_depth {
            continue;
        }
        for e in g.out_edges(u) {
            if depth[e.to.0] == usize::MAX {
                depth[e.to.0] = depth[u.0] + 1;
                parent[e.to.0] = Some(u);
                queue.push_back(e.to);
            }
        }
    }
    let mut out = Vec::new();
    emit(g, root, None, 0, max_depth, &parent, &mut out);
    out
}

fn emit(
    g: &CallGraph,
    u: NodeId,
    via: Option<Edge>,
    d: usize,
    max_depth: usize,
    parent: &[Option<NodeId>],
    out: &mut Vec<TreeLine>,
) {
    let has_calls = !g.out_edges(u).is_empty() || !g.gaps_of(u).is_empty();
    let unexpanded = !g.expanded.get(u.0).copied().unwrap_or(false);
    out.push(TreeLine::Node {
        depth: d,
        node: u,
        via,
        truncated: d >= max_depth && (has_calls || unexpanded),
    });
    if d >= max_depth {
        return;
    }
    // Interleave edges and gaps in call-site order.
    let edges = g.out_edges(u);
    let gaps = g.gaps_of(u);
    let (mut ei, mut gi) = (0, 0);
    while ei < edges.len() || gi < gaps.len() {
        let take_edge = match (edges.get(ei), gaps.get(gi)) {
            (Some(e), Some(gp)) => e.call_line <= gp.call_line,
            (Some(_), None) => true,
            _ => false,
        };
        if take_edge {
            let e = edges[ei];
            ei += 1;
            if parent[e.to.0] == Some(u) && e.to != u {
                emit(g, e.to, Some(e.clone()), d + 1, max_depth, parent, out);
            } else {
                out.push(TreeLine::BackRef {
                    depth: d + 1,
                    node: e.to,
                    via: e.clone(),
                });
            }
        } else {
            out.push(TreeLine::Gap {
                depth: d + 1,
                gap: gaps[gi].clone(),
            });
            gi += 1;
        }
    }
}

/// The chains as one prefix-shared tree (several shortest chains usually
/// share their first hops), each hop carrying the edge that reached it.
pub(crate) fn chains_tree(g: &CallGraph, chains: &[Vec<NodeId>]) -> Vec<TreeLine> {
    let mut out = Vec::new();
    let mut prev: Vec<NodeId> = Vec::new();
    for chain in chains {
        let common = prev
            .iter()
            .zip(chain.iter())
            .take_while(|(a, b)| a == b)
            .count();
        for (d, &node) in chain.iter().enumerate().skip(common) {
            let via = (d > 0).then(|| g.edge(chain[d - 1], node).cloned()).flatten();
            out.push(TreeLine::Node {
                depth: d,
                node,
                via,
                truncated: false,
            });
        }
        prev = chain.clone();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, line: u32) -> FnNode {
        FnNode {
            file: "crates/x/src/lib.rs".to_string(),
            qualname: name.to_string(),
            line,
            signature: format!("fn {name}()"),
            doc: String::new(),
        }
    }

    /// Synthetic graph:
    ///
    /// ```text
    /// main -> run -> history -> scatter -> cm_to_lab
    ///   \             ^  \-> sample ----------^
    ///    \-> setup ---/   \-> prn
    ///         history -> history (recursion)
    /// ```
    fn graph() -> (CallGraph, Vec<NodeId>) {
        let mut g = CallGraph::default();
        let names = ["main", "run", "setup", "history", "scatter", "sample", "cm_to_lab", "prn"];
        let ids: Vec<NodeId> = names
            .iter()
            .enumerate()
            .map(|(i, n)| g.intern(node(n, 10 * (i as u32 + 1))))
            .collect();
        let e = |g: &mut CallGraph, a: usize, b: usize, line: u32| {
            g.add_edge(Edge {
                from: ids[a],
                to: ids[b],
                call_line: line,
                kind: EdgeKind::Call,
            })
        };
        e(&mut g, 0, 1, 11); // main -> run
        e(&mut g, 0, 2, 12); // main -> setup
        e(&mut g, 1, 3, 21); // run -> history
        e(&mut g, 2, 3, 31); // setup -> history
        e(&mut g, 3, 4, 41); // history -> scatter
        e(&mut g, 3, 5, 42); // history -> sample
        e(&mut g, 3, 7, 43); // history -> prn
        e(&mut g, 3, 3, 44); // history -> history
        e(&mut g, 4, 6, 51); // scatter -> cm_to_lab
        e(&mut g, 5, 6, 61); // sample -> cm_to_lab
        for x in g.expanded.iter_mut() {
            *x = true;
        }
        g.gaps.push(Gap {
            from: ids[4],
            call_line: 52,
            kind: GapKind::Trait,
            callee: "volume".to_string(),
            target: None,
            detail: "trait method".to_string(),
        });
        (g, ids)
    }

    /// Methodology: two diamonds (main->{run,setup}->history and
    /// history->{scatter,sample}->cm_to_lab) give exactly four shortest
    /// chains main -> cm_to_lab of 4 hops; a recursion edge must not loop.
    ///
    /// Result (2026-10-04): four chains, all of length 5 nodes, ordered by
    /// call-site lines (run before setup, scatter before sample).
    #[test]
    fn shortest_paths_finds_every_minimal_chain_in_call_order() {
        let (g, id) = graph();
        let p = shortest_paths(&g, id[0], id[6], 10);
        assert_eq!(p.len(), 4);
        assert!(p.iter().all(|c| c.len() == 5));
        assert_eq!(p[0], vec![id[0], id[1], id[3], id[4], id[6]]);
        assert_eq!(p[1], vec![id[0], id[1], id[3], id[5], id[6]]);
        assert_eq!(p[2], vec![id[0], id[2], id[3], id[4], id[6]]);
        // Cap respected.
        assert_eq!(shortest_paths(&g, id[0], id[6], 2).len(), 2);
        // Unreachable: cm_to_lab calls nothing.
        assert!(shortest_paths(&g, id[6], id[0], 10).is_empty());
        // Trivial.
        assert_eq!(shortest_paths(&g, id[3], id[3], 10), vec![vec![id[3]]]);
    }

    /// Methodology: the tree from `main` to depth 4 must expand `history`
    /// once (under `run`, its first breadth-first parent), show the call
    /// from `setup` and the recursive call as back-references, show
    /// `cm_to_lab` once with the second call a back-reference, and list the
    /// gap under `scatter` in call-site order.
    ///
    /// Result (2026-10-04): passes; every function appears as a full node
    /// exactly once.
    #[test]
    fn tree_dedupes_with_back_references_and_lists_gaps() {
        let (g, id) = graph();
        let t = tree(&g, id[0], 4);
        let full: Vec<NodeId> = t
            .iter()
            .filter_map(|l| match l {
                TreeLine::Node { node, .. } => Some(*node),
                _ => None,
            })
            .collect();
        let mut sorted = full.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), full.len(), "a function expanded twice");
        assert_eq!(full.len(), 8);
        let backrefs: Vec<NodeId> = t
            .iter()
            .filter_map(|l| match l {
                TreeLine::BackRef { node, .. } => Some(*node),
                _ => None,
            })
            .collect();
        // sample -> cm_to_lab, history -> history, setup -> history.
        assert_eq!(backrefs, vec![id[6], id[3], id[3]]);
        let gap_pos = t.iter().position(|l| matches!(l, TreeLine::Gap { .. })).unwrap();
        // The gap (line 52) follows cm_to_lab (line 51) under scatter.
        assert!(matches!(t[gap_pos - 1], TreeLine::Node { node, .. } if node == id[6]));
        assert!(matches!(t[gap_pos], TreeLine::Gap { depth: 4, .. }));
    }

    /// Methodology: a depth limit of 2 must stop at `history`, marking it
    /// truncated, and never show deeper functions.
    ///
    /// Result (2026-10-04): passes.
    #[test]
    fn tree_depth_limit_marks_truncation() {
        let (g, id) = graph();
        let t = tree(&g, id[0], 2);
        assert!(t.iter().any(|l| matches!(l,
            TreeLine::Node { node, truncated: true, depth: 2, .. } if *node == id[3])));
        assert!(!t.iter().any(|l| matches!(l, TreeLine::Node { node, .. } if *node == id[6])));
    }

    /// Methodology: two chains sharing three hops render as one prefix tree:
    /// the shared hops once, then the two diverging tails.
    ///
    /// Result (2026-10-04): 7 lines; the shared prefix main/run/history
    /// appears once, and the two chains kept under the cap are the first two
    /// in call-site order.
    #[test]
    fn chains_tree_shares_prefixes() {
        let (g, id) = graph();
        let chains = shortest_paths(&g, id[0], id[6], 2);
        let t = chains_tree(&g, &chains);
        // main, run, history, scatter, cm_to_lab, then sample, cm_to_lab.
        assert_eq!(t.len(), 7);
        assert!(matches!(&t[5], TreeLine::Node { node, depth: 3, via: Some(e), .. }
            if *node == id[5] && e.call_line == 42));
    }
}
