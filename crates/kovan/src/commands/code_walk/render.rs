//! Markdown, Mermaid and JSON renderings of a walk.
//!
//! Every format marks an unresolved call explicitly and in a fixed,
//! greppable form — `UNRESOLVED(<kind>)` in Markdown and Mermaid, a
//! `"gaps"` array with a `"kind"` per entry in JSON — so a lesson-writing
//! agent can find each one and either fill it by hand (a `hand:` line, see
//! `super::lesson`) or explain it in prose. Hand-filled hops are labelled
//! `filled by hand` in every format. Output is deterministic: no dates, no
//! timings, no query counts, so the check mode can compare it byte for byte.
//!
//! **Inline code (lesson blocks, 2026-10-05).** A concept path rendered for a
//! lesson shows each hop's code on the page, not only a link to it: every
//! function on the chain is followed by an mdBook `{{#include}}` of its lines
//! (the whole function for the last hop, the signature and the lines around
//! each call for the others), so a reader sees the code without clicking. The
//! line ranges are fixed when the walk is generated and re-checked by
//! `code-walk-check`; each snippet also carries a `<!-- snippet-check: -->`
//! comment (file, line, the text that line must hold) that
//! `scripts/build-pages.sh` verifies without rust-analyzer, so a range that
//! drifted fails the site build instead of showing the wrong lines.
//!
//! **Trees too (2026-10-06).** A tree block in a lesson (no `to=`) shows each
//! expanded function's code the same way, folded behind a `<details>` toggle
//! when the tree has more than [`TREE_OPEN_MAX_NODES`] functions; see
//! `markdown_lines_inline`.

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
    /// Set for a concept path rendered into a lesson: show each hop's code.
    pub(crate) inline: Option<Inline>,
}

/// Where a function's code is, 1-based inclusive lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    /// The line holding `fn <name>`.
    pub(crate) decl: u32,
    /// The line of the body's opening `{` (`decl` for a bodiless `fn f();`).
    pub(crate) open: u32,
    /// The body's closing `}` (`open` when bodiless).
    pub(crate) end: u32,
}

/// What the inline rendering needs: the path from the lesson's directory to
/// the workspace root (mdBook resolves `{{#include}}` relative to the page),
/// and each node's span (`None` when its file could not be indexed).
#[derive(Debug, Clone, Default)]
pub(crate) struct Inline {
    pub(crate) to_root: String,
    pub(crate) spans: Vec<Option<Span>>,
}

/// The last hop is shown whole up to this many lines.
const LEAF_MAX_LINES: u32 = 40;
/// A caller is shown from its signature to its last call when that fits in
/// this many lines; otherwise the signature and a window around each call.
const CALLER_MAX_LINES: u32 = 30;
/// Signature lines shown at most, when the body is cut.
const SIGNATURE_MAX_LINES: u32 = 8;

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
        EdgeKind::Operator => format!(" · called through an operator at {at}"),
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
    if let (Some(inline), false) = (&h.inline, chains.is_empty()) {
        return markdown_path_inline(h, inline, g, chains);
    }
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

/// The bare function name of a node (`Type::name` → `name`).
fn bare_name(n: &FnNode) -> &str {
    n.qualname.rsplit("::").next().unwrap_or(&n.qualname)
}

/// The line ranges to show for one function: the whole of it (capped) when
/// it calls nothing on the chain, else the signature and the lines around
/// each call site. Returns the ranges and whether the function continues
/// past the last one.
pub(crate) fn snippet_ranges(span: Span, calls: &[u32]) -> (Vec<(u32, u32)>, bool) {
    let Span { decl, open, end } = span;
    let mut calls: Vec<u32> = calls.iter().copied().filter(|&c| c >= decl && c <= end).collect();
    calls.sort_unstable();
    calls.dedup();
    if calls.is_empty() {
        let last = end.min(decl + LEAF_MAX_LINES - 1);
        return (vec![(decl, last)], last < end);
    }
    let last_call = *calls.last().unwrap_or(&decl);
    let through = (last_call + 1).min(end);
    if through + 1 - decl <= CALLER_MAX_LINES {
        return (vec![(decl, through)], through < end);
    }
    let mut ranges: Vec<(u32, u32)> = vec![(decl, open.min(decl + SIGNATURE_MAX_LINES - 1))];
    for c in calls {
        let (a, b) = (c.saturating_sub(2).max(decl), (c + 1).min(end));
        match ranges.last_mut() {
            // Overlapping or adjacent: one range.
            Some(r) if a <= r.1 + 1 => r.1 = r.1.max(b),
            _ => ranges.push((a, b)),
        }
    }
    let shown_to = ranges.last().map(|r| r.1).unwrap_or(decl);
    (ranges, shown_to < end)
}

/// One hop's code as a fenced block of `{{#include}}`s, `// …` between the
/// pieces, preceded by the checks the Pages build runs on it.
fn snippet(inline: &Inline, n: &FnNode, span: Span, calls: &[(u32, &str)]) -> String {
    let lines: Vec<u32> = calls.iter().map(|c| c.0).collect();
    let (ranges, more) = snippet_ranges(span, &lines);
    let mut out = format!("<!-- snippet-check: {}:{} fn {} -->\n", n.file, span.decl, bare_name(n));
    for (line, callee) in calls {
        if *line >= span.decl && *line <= span.end {
            out.push_str(&format!("<!-- snippet-check: {}:{line} {callee} -->\n", n.file));
        }
    }
    out.push_str("\n```rust,ignore\n");
    for (i, (a, b)) in ranges.iter().enumerate() {
        if i > 0 {
            out.push_str("    // …\n");
        }
        out.push_str(&format!("{{{{#include {}{}:{a}:{b}}}}}\n", inline.to_root, n.file));
    }
    if more {
        out.push_str("    // … (the rest of the function: follow the link above)\n");
    }
    out.push_str("```\n");
    out
}

/// The concept path for a lesson: one numbered step per function, each with
/// its code inline (see the module doc). Branches of a multi-chain walk say
/// which step they continue from.
fn markdown_path_inline(h: &Header, inline: &Inline, g: &CallGraph, chains: &[Vec<NodeId>]) -> String {
    let hops = chains[0].len() - 1;
    let mut out = format!(
        "Call chain from `{}` to `{}`: {hops} hop{}, {} shortest chain{}. Each step shows \
         its code; the name links to it on GitHub.\n\n",
        g.node(chains[0][0]).label(),
        g.node(*chains[0].last().unwrap_or(&chains[0][0])).label(),
        if hops == 1 { "" } else { "s" },
        chains.len(),
        if chains.len() == 1 { "" } else { "s" },
    );
    let lines = super::graph::chains_tree(g, chains);
    // Step numbers in display order, and the call sites each node makes into
    // its children on the chains (hand-filled hops have no call site).
    let mut step_of: Vec<Option<usize>> = vec![None; g.nodes.len()];
    let mut calls_of: Vec<Vec<(u32, String)>> = vec![Vec::new(); g.nodes.len()];
    let mut step = 0usize;
    for l in &lines {
        if let TreeLine::Node { node, via, .. } = l {
            step += 1;
            step_of[node.0] = Some(step);
            if let Some(e) = via {
                if matches!(e.kind, EdgeKind::Call | EdgeKind::FnValue | EdgeKind::Operator) {
                    let callee = bare_name(g.node(e.to)).to_string();
                    if !calls_of[e.from.0].iter().any(|c| c.0 == e.call_line) {
                        calls_of[e.from.0].push((e.call_line, callee));
                    }
                }
            }
        }
    }
    let mut prev: Option<NodeId> = None;
    for l in &lines {
        let TreeLine::Node { node, via, .. } = l else { continue };
        let n = g.node(*node);
        out.push_str(&format!("**{}.** ", step_of[node.0].unwrap_or(0)));
        if let Some(e) = via {
            if prev != Some(e.from) {
                out.push_str(&format!("(from step {}) ", step_of[e.from.0].unwrap_or(0)));
            }
            out.push_str("→ ");
        }
        out.push_str(&node_link(h, n));
        if let Some(e) = via {
            out.push_str(&via_text(h, g, e));
        }
        if !n.doc.is_empty() {
            out.push_str(&format!(" — {}", n.doc));
        }
        out.push_str("\n\n");
        match inline.spans.get(node.0).copied().flatten() {
            Some(span) => {
                let calls: Vec<(u32, &str)> =
                    calls_of[node.0].iter().map(|(l, c)| (*l, c.as_str())).collect();
                out.push_str(&snippet(inline, n, span, &calls));
            }
            None if !n.signature.is_empty() => {
                out.push_str(&format!("`{}`\n", n.signature.replace('`', "'")));
            }
            None => {}
        }
        out.push('\n');
        prev = Some(*node);
    }
    let mut on_chain: Vec<NodeId> = chains
        .iter()
        .flat_map(|c| c[..c.len() - 1].iter().copied())
        .collect();
    on_chain.sort();
    on_chain.dedup();
    let gaps = gap_list(h, g, &on_chain);
    if !gaps.is_empty() {
        out.push_str("Unresolved calls inside the functions on this chain:\n\n");
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
    match &h.inline {
        Some(inline) => out.push_str(&markdown_lines_inline(h, inline, g, lines, shown)),
        None => out.push_str(&markdown_lines(h, g, lines)),
    }
    out
}

/// A tree with this many functions or fewer opens every function's code;
/// a larger one folds each behind a "code" toggle, so the page stays
/// readable on a phone while every function's code is still on it.
const TREE_OPEN_MAX_NODES: usize = 15;
/// Indentation per depth level, and the deepest level indented further
/// (deep trees must still fit a phone screen).
const TREE_INDENT_EM: f64 = 0.9;
const TREE_INDENT_MAX_DEPTH: usize = 6;

/// The tree for a lesson (maintainer request, 2026-10-06: every walk shows
/// its code inline as well as the links). Each line is a block indented by
/// its depth; each expanded function carries its code as a fenced block of
/// `{{#include}}`s (the same ranges and `snippet-check` comments as a
/// concept path: the signature and the lines around each call it makes in
/// the tree, or the whole function, capped, when it calls nothing shown).
/// It is not a Markdown list because an included line starts at column 0
/// and would end a list item.
fn markdown_lines_inline(
    h: &Header,
    inline: &Inline,
    g: &CallGraph,
    lines: &[TreeLine],
    shown: usize,
) -> String {
    // The call sites each function makes in this tree.
    let mut calls_of: Vec<Vec<(u32, String)>> = vec![Vec::new(); g.nodes.len()];
    let mut add = |from: NodeId, line: u32, callee: String| {
        if !calls_of[from.0].iter().any(|c| c.0 == line) {
            calls_of[from.0].push((line, callee));
        }
    };
    for l in lines {
        match l {
            // A self-edge is skipped: rust-analyzer resolves a call through an
            // `impl Fn` parameter (`cell_r(r)` in `draw`) to the enclosing
            // function, so the callee's name is not on that line.
            TreeLine::Node { via: Some(e), .. } | TreeLine::BackRef { via: e, .. }
                if matches!(e.kind, EdgeKind::Call | EdgeKind::FnValue | EdgeKind::Operator) && e.from != e.to =>
            {
                add(e.from, e.call_line, bare_name(g.node(e.to)).to_string())
            }
            TreeLine::Gap { gap, .. } => {
                // Only a plain identifier is a checkable token on that line
                // (a gap's callee can be `(expression)`).
                let callee = gap.callee.trim_end_matches('!');
                let callee = callee.rsplit("::").next().unwrap_or(callee).to_string();
                if !callee.is_empty() && callee.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    add(gap.from, gap.call_line, callee)
                }
            }
            _ => {}
        }
    }
    let open = if shown <= TREE_OPEN_MAX_NODES { " open" } else { "" };
    let indent = |d: usize| d.min(TREE_INDENT_MAX_DEPTH) as f64 * TREE_INDENT_EM;
    let mut out = String::new();
    for l in lines {
        match l {
            TreeLine::Node { depth, node, via, truncated } => {
                let n = g.node(*node);
                let mut text = node_link(h, n);
                if !n.signature.is_empty() {
                    text.push_str(&format!(" `{}`", n.signature.replace('`', "'")));
                }
                if !n.doc.is_empty() {
                    text.push_str(&format!(" — {}", n.doc));
                }
                if let Some(e) = via {
                    text.push_str(&via_text(h, g, e));
                }
                if *truncated {
                    text.push_str(" · *(calls below the depth limit not shown)*");
                }
                out.push_str(&format!(
                    "<div class=\"cw-node\" style=\"margin-left:{:.1}em\">\n\n{text}\n\n",
                    indent(*depth)
                ));
                if let Some(span) = inline.spans.get(node.0).copied().flatten() {
                    let calls: Vec<(u32, &str)> =
                        calls_of[node.0].iter().map(|(l, c)| (*l, c.as_str())).collect();
                    out.push_str(&format!("<details{open}><summary>code</summary>\n\n"));
                    out.push_str(&snippet(inline, n, span, &calls));
                    out.push_str("\n</details>\n");
                }
                out.push_str("</div>\n\n");
            }
            TreeLine::BackRef { depth, node, via } => {
                out.push_str(&format!(
                    "<div class=\"cw-node\" style=\"margin-left:{:.1}em\">\n\n`{}` *(expanded elsewhere in this walk)*{}\n\n</div>\n\n",
                    indent(*depth),
                    g.node(*node).label(),
                    via_text(h, g, via)
                ));
            }
            TreeLine::Gap { depth, gap } => {
                out.push_str(&format!(
                    "<div class=\"cw-node\" style=\"margin-left:{:.1}em\">\n\n{}\n\n</div>\n\n",
                    indent(*depth),
                    gap_line(h, g, gap)
                ));
            }
        }
    }
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
        EdgeKind::Operator => format!("  n{} -- operator --> n{}", e.from.0, e.to.0),
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
            inline: None,
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
    /// Methodology: the line ranges for a short and a long last hop, and for
    /// a caller whose calls fit in one range and one whose do not (windows
    /// around each call, merged when they touch).
    ///
    /// Result (2026-10-05): a 10-line leaf is shown whole; a 100-line leaf
    /// is cut at 40 lines and says so; a caller calling at +5 is shown to
    /// +6; a caller calling at +50 and +52 shows its signature to the `{`
    /// and one merged window 48..53.
    #[test]
    fn snippet_ranges_show_the_signature_and_each_call() {
        let leaf = Span { decl: 10, open: 10, end: 19 };
        assert_eq!(snippet_ranges(leaf, &[]), (vec![(10, 19)], false));
        let long = Span { decl: 10, open: 12, end: 109 };
        assert_eq!(snippet_ranges(long, &[]), (vec![(10, 49)], true));
        assert_eq!(snippet_ranges(long, &[15]), (vec![(10, 16)], true));
        assert_eq!(
            snippet_ranges(long, &[60, 62]),
            (vec![(10, 12), (58, 63)], true)
        );
        // A call line outside the function (a hand hop) is ignored.
        assert_eq!(snippet_ranges(leaf, &[200]), (vec![(10, 19)], false));
    }

    /// Methodology: the sample graph rendered as a lesson TREE (no `to=`),
    /// as a tree block in a lesson is.
    ///
    /// Result (2026-10-06): both functions carry their code as includes with
    /// `snippet-check` comments, inside an open `<details>` (small tree);
    /// the links and the gap are kept; the output is indented blocks, not a
    /// Markdown list.
    #[test]
    fn lesson_trees_show_each_function_inline() {
        let (g, mut h) = sample();
        h.to = None;
        h.inline = Some(Inline {
            to_root: "../../".into(),
            spans: vec![
                Some(Span { decl: 3, open: 3, end: 8 }),
                Some(Span { decl: 40, open: 40, end: 45 }),
            ],
        });
        let lines = vec![
            TreeLine::Node { depth: 0, node: NodeId(0), via: None, truncated: false },
            TreeLine::Node { depth: 1, node: NodeId(1), via: Some(g.edges[0].clone()), truncated: false },
            TreeLine::Gap { depth: 1, gap: g.gaps[0].clone() },
        ];
        let md = markdown_tree(&h, &g, &lines, false);
        // Code inline for both functions, links kept, open (small tree). The
        // entry point is shown to the line after its one call in the tree
        // (the gap at L5), as a concept path would show it.
        assert!(md.contains("{{#include ../../crates/x/examples/demo.rs:3:6}}"));
        assert!(md.contains("<!-- snippet-check: crates/x/examples/demo.rs:5 volume -->"));
        assert!(md.contains("{{#include ../../crates/x/src/lib.rs:40:45}}"));
        assert!(md.contains("<details open><summary>code</summary>"));
        assert!(md.contains("[`lib.rs::Sphere::distance`]"));
        assert!(md.contains("<!-- snippet-check: crates/x/src/lib.rs:40 fn distance -->"));
        assert!(md.contains("UNRESOLVED(trait)"));
        // Not a Markdown list: an included line at column 0 would end it.
        assert!(!md.contains("\n- ["));
        assert!(md.contains("margin-left:0.9em"));
    }

    /// Methodology: the sample chain rendered with inline code, as a lesson
    /// block is.
    ///
    /// Result (2026-10-05): each step carries an mdBook include relative to
    /// the page and a `snippet-check` comment naming the `fn` line; the hand
    /// hop is still labelled `filled by hand`.
    #[test]
    fn lesson_paths_show_each_hop_inline() {
        let (g, mut h) = sample();
        h.inline = Some(Inline {
            to_root: "../../".into(),
            spans: vec![
                Some(Span { decl: 3, open: 3, end: 8 }),
                Some(Span { decl: 40, open: 40, end: 45 }),
            ],
        });
        let md = markdown_path(&h, &g, &[vec![NodeId(0), NodeId(1)]]);
        assert!(md.contains("{{#include ../../crates/x/examples/demo.rs:3:8}}"));
        assert!(md.contains("{{#include ../../crates/x/src/lib.rs:40:45}}"));
        assert!(md.contains("<!-- snippet-check: crates/x/src/lib.rs:40 fn distance -->"));
        assert!(md.contains("**2.** → [`lib.rs::Sphere::distance`]"));
        assert!(md.contains("**filled by hand**"));
        assert!(md.contains("UNRESOLVED(trait)"));
    }
}
