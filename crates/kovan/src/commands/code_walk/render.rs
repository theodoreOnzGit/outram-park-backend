//! Markdown, Mermaid and JSON renderings of a walk.
//!
//! Every format marks an unresolved call explicitly and in a fixed,
//! greppable form — `UNRESOLVED(<kind>)` in Markdown and Mermaid, a
//! `"gaps"` array with a `"kind"` per entry in JSON — so a lesson-writing
//! agent can find each one and either fill it by hand (a `hand:` line, see
//! `super::lesson`) or explain it in prose. Hand-filled hops are labelled
//! `filled by hand` in every format. Output is deterministic: no dates, no
//! timings, no query counts, so the check mode can compare it byte for byte.

use serde::Serialize;

use super::graph::{CallGraph, Edge, EdgeKind, FnNode, Gap, NodeId, TreeLine};

/// Default repository the permalinks point into. `@@COMMIT@@` is replaced by
/// the Pages build (`scripts/build-pages.sh`) with the commit it built.
pub const DEFAULT_REPO_URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend";

/// What was walked, for headers and JSON.
#[derive(Debug, Clone)]
pub(crate) struct Header {
    pub(crate) from: String,
    pub(crate) to: Option<String>,
    pub(crate) depth: usize,
    pub(crate) repo_url: String,
}

fn permalink(repo: &str, file: &str, line: u32) -> String {
    format!("{repo}/blob/@@COMMIT@@/{file}#L{line}")
}

fn node_link(h: &Header, n: &FnNode) -> String {
    format!("[`{}`]({})", n.label(), permalink(&h.repo_url, &n.file, n.line))
}

fn via_text(h: &Header, g: &CallGraph, e: &Edge) -> String {
    let caller = g.node(e.from);
    let at = format!(
        "[L{}]({})",
        e.call_line,
        permalink(&h.repo_url, &caller.file, e.call_line)
    );
    match &e.kind {
        EdgeKind::Call => format!(" · called at {at}"),
        EdgeKind::FnValue => format!(" · passed as a function value at {at}"),
        EdgeKind::Hand { note } if note.is_empty() => " · **filled by hand**".to_string(),
        EdgeKind::Hand { note } => format!(" · **filled by hand**: {note}"),
    }
}

fn gap_line(h: &Header, g: &CallGraph, gap: &Gap) -> String {
    let caller = g.node(gap.from);
    let target = match &gap.target {
        Some((f, l)) => format!(" (→ [`{f}:{l}`]({}))", permalink(&h.repo_url, f, *l)),
        None => String::new(),
    };
    format!(
        "UNRESOLVED({}): `{}` at [L{}]({}){target} — {}",
        gap.kind.tag(),
        gap.callee,
        gap.call_line,
        permalink(&h.repo_url, &caller.file, gap.call_line),
        gap.detail
    )
}

/// Renders tree lines as a nested Markdown list.
pub(crate) fn markdown_lines(h: &Header, g: &CallGraph, lines: &[TreeLine]) -> String {
    let mut out = String::new();
    for l in lines {
        match l {
            TreeLine::Node {
                depth,
                node,
                via,
                truncated,
            } => {
                let n = g.node(*node);
                out.push_str(&"  ".repeat(*depth));
                out.push_str("- ");
                out.push_str(&node_link(h, n));
                if !n.signature.is_empty() {
                    out.push_str(&format!(" `{}`", n.signature.replace('`', "'")));
                }
                if !n.doc.is_empty() {
                    out.push_str(&format!(" — {}", n.doc));
                }
                if let Some(e) = via {
                    out.push_str(&via_text(h, g, e));
                }
                if *truncated {
                    out.push_str(" · *(calls below the depth limit not shown)*");
                }
                out.push('\n');
            }
            TreeLine::BackRef { depth, node, via } => {
                out.push_str(&"  ".repeat(*depth));
                out.push_str(&format!(
                    "- `{}` *(expanded elsewhere in this walk)*{}\n",
                    g.node(*node).label(),
                    via_text(h, g, via)
                ));
            }
            TreeLine::Gap { depth, gap } => {
                out.push_str(&"  ".repeat(*depth));
                out.push_str(&format!("- {}\n", gap_line(h, g, gap)));
            }
        }
    }
    out
}

/// The concept-path rendering: header, the chains as a prefix tree, then
/// the unresolved calls inside the functions on the chains (where a longer
/// or alternative route could hide).
pub(crate) fn markdown_path(h: &Header, g: &CallGraph, chains: &[Vec<NodeId>]) -> String {
    let to = h.to.as_deref().unwrap_or("");
    let mut out = String::new();
    if chains.is_empty() {
        out.push_str(&format!(
            "**No call chain found** from `{}` to `{to}` within {} hops \
             ({} functions explored). The chain may pass through one of the \
             unresolved calls below; fill it with a `hand:` line once you have \
             read the code.\n\n",
            h.from,
            h.depth,
            g.nodes.len()
        ));
        out.push_str(&gap_list(h, g, &(0..g.nodes.len()).map(NodeId).collect::<Vec<_>>()));
        return out;
    }
    let hops = chains[0].len() - 1;
    out.push_str(&format!(
        "Call chain from `{}` to `{}`: {hops} hop{}, {} shortest chain{}.\n\n",
        g.node(chains[0][0]).label(),
        g.node(*chains[0].last().unwrap_or(&chains[0][0])).label(),
        if hops == 1 { "" } else { "s" },
        chains.len(),
        if chains.len() == 1 { "" } else { "s" },
    ));
    out.push_str(&markdown_lines(h, g, &super::graph::chains_tree(g, chains)));
    let mut on_chain: Vec<NodeId> = chains
        .iter()
        .flat_map(|c| c[..c.len() - 1].iter().copied())
        .collect();
    on_chain.sort();
    on_chain.dedup();
    let gaps = gap_list(h, g, &on_chain);
    if !gaps.is_empty() {
        out.push_str("\nUnresolved calls inside the functions on this chain:\n\n");
        out.push_str(&gaps);
    }
    out
}

fn gap_list(h: &Header, g: &CallGraph, nodes: &[NodeId]) -> String {
    let mut out = String::new();
    for &n in nodes {
        let gaps = g.gaps_of(n);
        if gaps.is_empty() {
            continue;
        }
        out.push_str(&format!("- in {}:\n", node_link(h, g.node(n))));
        for gap in gaps {
            out.push_str(&format!("  - {}\n", gap_line(h, g, gap)));
        }
    }
    out
}

/// Tree-mode header plus the tree.
pub(crate) fn markdown_tree(h: &Header, g: &CallGraph, lines: &[TreeLine], truncated: bool) -> String {
    let shown = lines
        .iter()
        .filter(|l| matches!(l, TreeLine::Node { .. }))
        .count();
    let gaps = lines
        .iter()
        .filter(|l| matches!(l, TreeLine::Gap { .. }))
        .count();
    let s = |n: usize| if n == 1 { "" } else { "s" };
    let mut out = format!(
        "Everything `{}` reaches in the workspace, to {} hop{}: {shown} function{}, \
         {gaps} unresolved call{}. A function is expanded once; later calls to it \
         say *(expanded elsewhere in this walk)*.\n\n",
        h.from,
        h.depth,
        s(h.depth),
        s(shown),
        s(gaps)
    );
    if truncated {
        out.push_str("**Truncated**: the function cap was reached before the depth limit.\n\n");
    }
    out.push_str(&markdown_lines(h, g, lines));
    out
}

fn mermaid_escape(s: &str) -> String {
    s.replace('"', "#quot;")
}

/// A Mermaid `flowchart` of the given lines (back-references become edges to
/// the existing node; gaps become dashed `UNRESOLVED(...)` nodes).
pub(crate) fn mermaid(g: &CallGraph, lines: &[TreeLine]) -> String {
    let mut out = String::from("```mermaid\nflowchart TD\n");
    let mut declared: Vec<NodeId> = Vec::new();
    let mut edges: Vec<String> = Vec::new();
    let mut gap_n = 0usize;
    let declare = |out: &mut String, declared: &mut Vec<NodeId>, n: NodeId| {
        if !declared.contains(&n) {
            declared.push(n);
            out.push_str(&format!(
                "  n{}[\"{}\"]\n",
                n.0,
                mermaid_escape(&g.node(n).label())
            ));
        }
    };
    let arrow = |e: &Edge| match &e.kind {
        EdgeKind::Hand { .. } => format!("  n{} -. filled by hand .-> n{}", e.from.0, e.to.0),
        EdgeKind::FnValue => format!("  n{} -- fn value --> n{}", e.from.0, e.to.0),
        EdgeKind::Call => format!("  n{} --> n{}", e.from.0, e.to.0),
    };
    for l in lines {
        match l {
            TreeLine::Node { node, via, .. } => {
                declare(&mut out, &mut declared, *node);
                if let Some(e) = via {
                    edges.push(arrow(e));
                }
            }
            TreeLine::BackRef { node, via, .. } => {
                declare(&mut out, &mut declared, *node);
                edges.push(arrow(via));
            }
            TreeLine::Gap { gap, .. } => {
                out.push_str(&format!(
                    "  g{gap_n}[\"UNRESOLVED({}): {}\"]:::gap\n",
                    gap.kind.tag(),
                    mermaid_escape(&gap.callee)
                ));
                edges.push(format!("  n{} -.-> g{gap_n}", gap.from.0));
                gap_n += 1;
            }
        }
    }
    edges.dedup();
    for e in edges {
        out.push_str(&e);
        out.push('\n');
    }
    out.push_str("  classDef gap stroke-dasharray: 5 5\n```\n");
    out
}

#[derive(Serialize)]
struct JsonNode {
    id: usize,
    key: String,
    file: String,
    qualname: String,
    line: u32,
    signature: String,
    doc: String,
    permalink: String,
}

#[derive(Serialize)]
struct JsonWalk<'a> {
    tool: &'static str,
    mode: &'static str,
    from: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
    depth: usize,
    truncated: bool,
    external_calls: usize,
    nodes: Vec<JsonNode>,
    edges: &'a [Edge],
    /// Every unresolved call; `kind` is one of trait, closure, macro,
    /// no-definition, other.
    gaps: &'a [Gap],
    /// Path mode: each shortest chain as node ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    chains: Option<&'a [Vec<NodeId>]>,
}

/// The whole explored graph as JSON.
pub(crate) fn json(
    h: &Header,
    g: &CallGraph,
    chains: Option<&[Vec<NodeId>]>,
    truncated: bool,
    external_calls: usize,
) -> String {
    let nodes = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| JsonNode {
            id: i,
            key: n.key(),
            file: n.file.clone(),
            qualname: n.qualname.clone(),
            line: n.line,
            signature: n.signature.clone(),
            doc: n.doc.clone(),
            permalink: permalink(&h.repo_url, &n.file, n.line),
        })
        .collect();
    let w = JsonWalk {
        tool: "kovan-cli code-walk",
        mode: if h.to.is_some() { "path" } else { "tree" },
        from: &h.from,
        to: h.to.as_deref(),
        depth: h.depth,
        truncated,
        external_calls,
        nodes,
        edges: &g.edges,
        gaps: &g.gaps,
        chains,
    };
    serde_json::to_string_pretty(&w).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::super::graph::{GapKind, TreeLine};
    use super::*;

    fn sample() -> (CallGraph, Header) {
        let mut g = CallGraph::default();
        let a = g.intern(FnNode {
            file: "crates/x/examples/demo.rs".into(),
            qualname: "main".into(),
            line: 3,
            signature: "fn main()".into(),
            doc: String::new(),
        });
        let b = g.intern(FnNode {
            file: "crates/x/src/lib.rs".into(),
            qualname: "Sphere::distance".into(),
            line: 40,
            signature: "pub fn distance(&self) -> f64".into(),
            doc: "Distance to the surface.".into(),
        });
        g.add_edge(Edge {
            from: a,
            to: b,
            call_line: 3,
            kind: EdgeKind::Hand {
                note: "trait dispatch; Godiva is a Sphere".into(),
            },
        });
        g.gaps.push(Gap {
            from: a,
            call_line: 5,
            kind: GapKind::Trait,
            callee: "volume".into(),
            target: Some(("crates/x/src/lib.rs".into(), 9)),
            detail: "trait method".into(),
        });
        g.expanded = vec![true, true];
        let h = Header {
            from: "crates/x/examples/demo.rs::main".into(),
            to: Some("crates/x/src/lib.rs::Sphere::distance".into()),
            depth: 12,
            repo_url: DEFAULT_REPO_URL.into(),
        };
        (g, h)
    }

    /// Methodology: a two-node chain whose only hop is hand-filled, plus a
    /// trait gap in the entry point, rendered in all three formats.
    ///
    /// Result (2026-10-04): the hand hop is labelled `filled by hand` and
    /// the gap `UNRESOLVED(trait)` in Markdown and Mermaid, and JSON carries
    /// `"kind": "hand"` and `"kind": "trait"`; permalinks use `@@COMMIT@@`.
    #[test]
    fn every_format_marks_gaps_and_hand_hops() {
        let (g, h) = sample();
        let chains = vec![vec![NodeId(0), NodeId(1)]];
        let md = markdown_path(&h, &g, &chains);
        assert!(md.contains("1 hop, 1 shortest chain."));
        assert!(md.contains("**filled by hand**: trait dispatch; Godiva is a Sphere"));
        assert!(md.contains("UNRESOLVED(trait): `volume` at [L5]"));
        assert!(md.contains("/blob/@@COMMIT@@/crates/x/src/lib.rs#L40"));
        assert!(md.contains("`pub fn distance(&self) -> f64` — Distance to the surface."));

        let lines = vec![
            TreeLine::Node { depth: 0, node: NodeId(0), via: None, truncated: false },
            TreeLine::Node { depth: 1, node: NodeId(1), via: Some(g.edges[0].clone()), truncated: false },
            TreeLine::Gap { depth: 1, gap: g.gaps[0].clone() },
        ];
        let mm = mermaid(&g, &lines);
        assert!(mm.contains("n0 -. filled by hand .-> n1"));
        assert!(mm.contains("UNRESOLVED(trait): volume"));

        let js = json(&h, &g, Some(&chains), false, 0);
        let v: serde_json::Value = serde_json::from_str(&js).unwrap();
        assert_eq!(v["edges"][0]["kind"], "hand");
        assert_eq!(v["gaps"][0]["kind"], "trait");
        assert_eq!(v["chains"][0][1], 1);
    }

    /// Methodology: an empty chain list renders the "no chain" notice and
    /// still lists every gap in the explored graph.
    ///
    /// Result (2026-10-04): passes.
    #[test]
    fn missing_chain_lists_gaps() {
        let (g, h) = sample();
        let md = markdown_path(&h, &g, &[]);
        assert!(md.starts_with("**No call chain found**"));
        assert!(md.contains("UNRESOLVED(trait)"));
    }
}
