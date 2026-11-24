//! **Fault-tree diagrams** (gh:#1720): a plain node/edge type, a
//! deterministic tidy-tree layout, and (feature `egui`) the drawing.
//!
//! Nothing in the workspace drew a fault tree before this (searched
//! 2026-11-23: `raffles::scram` builds and quantifies trees but is GUI-free,
//! and no `bugis`, engine or `dhoby-ghaut` module lays one out). The type
//! here knows nothing about `raffles`: a caller converts its own tree into a
//! [`Diagram`] (the HTR-10 simulator's adapter from
//! `raffles::scram::FaultTreeModel` lives in that simulator), so `raffles`
//! stays GUI-free and `bugis` stays standalone (`crates/bugis/AGENTS.md`).
//!
//! # The layout, and why it is this one
//!
//! A fault tree from a real model is a **directed acyclic graph**, not a
//! tree: a basic event or a whole gate can feed several parents. The drawing
//! convention of fault-tree practice is to draw it as a tree anyway, expand a
//! shared gate **once**, and show every later occurrence as a **transfer**
//! symbol (a triangle) that refers back to it. [`layout`] does exactly that:
//!
//! 1. **Unfold** the graph from the root in depth-first pre-order, children
//!    in the order the caller lists them. The first occurrence of a gate is
//!    expanded; a later occurrence is a leaf with [`Placed::transfer`] set.
//!    Basic and house events are leaves wherever they occur. Because a gate
//!    is expanded only once, a cycle (which a fault tree must not have, but a
//!    malformed input might) also ends in a transfer instead of looping.
//! 2. **Place**: leaves take consecutive slots `0, 1, 2, ...` in pre-order;
//!    a parent sits at the midpoint of its first and last child; depth is the
//!    level. This is the simple layered layout (Wetherell and Shannon's
//!    "minimum" layout, before Reingold-Tilford's compaction): it can be
//!    wider than a compacted tree, but it never overlaps, it keeps every
//!    subtree's leaves contiguous so a highlighted cut set reads left to
//!    right, and it is a pure function of the input, so positions can be
//!    pinned by a test.
//!
//! World coordinates: x in slots ([`SLOT_W`] apart), y = `-depth *`
//! [`LEVEL_H`] (y up, as `web_demo::view::View` expects), root at the top.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[cfg(feature = "egui")]
pub mod draw;

/// Horizontal distance between neighbouring leaf slots, world units.
pub const SLOT_W: f64 = 1.0;
/// Vertical distance between levels, world units.
pub const LEVEL_H: f64 = 1.0;

/// What a node is drawn as: the gate's logic, or an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// All inputs.
    And,
    /// Any input.
    Or,
    /// At least `k` of the inputs (the MEF `atleast`).
    AtLeast(usize),
    /// Exclusive or of two inputs (the MEF `xor`).
    Xor,
    /// Negation of the single input.
    Not,
    /// Negated AND.
    Nand,
    /// Negated OR.
    Nor,
    /// Pass-through of a single input.
    Null,
    /// A basic event: a leaf with a probability.
    BasicEvent,
    /// A house event, set true or false: a leaf switch.
    HouseEvent(bool),
}

impl Symbol {
    /// Whether this symbol is a gate (has inputs) rather than an event.
    pub fn is_gate(self) -> bool {
        !matches!(self, Symbol::BasicEvent | Symbol::HouseEvent(_))
    }

    /// The short text drawn inside or beside a gate symbol.
    pub fn short_text(self) -> String {
        match self {
            Symbol::And => String::from("AND"),
            Symbol::Or => String::from("OR"),
            Symbol::AtLeast(k) => alloc::format!("{k}/n"),
            Symbol::Xor => String::from("XOR"),
            Symbol::Not => String::from("NOT"),
            Symbol::Nand => String::from("NAND"),
            Symbol::Nor => String::from("NOR"),
            Symbol::Null => String::from("="),
            Symbol::BasicEvent => String::new(),
            Symbol::HouseEvent(true) => String::from("T"),
            Symbol::HouseEvent(false) => String::from("F"),
        }
    }
}

/// How a node is emphasised when drawn: a highlighted cut set, the
/// selected event, or dimmed because it is outside what is highlighted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mark {
    /// Drawn plainly.
    #[default]
    Normal,
    /// Part of what is highlighted (a cut set's members and the gates above
    /// them).
    Highlight,
    /// The node the reader selected.
    Selected,
    /// Outside the highlight: drawn faintly.
    Dim,
}

/// One gate or event.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The name drawn in the node's label box.
    pub label: String,
    /// A second line under the name (a probability, a tag); may be empty.
    pub detail: String,
    /// What the node is.
    pub symbol: Symbol,
    /// Inputs, as indices into [`Diagram::nodes`], in drawing order. Empty
    /// for an event.
    pub children: Vec<usize>,
}

/// A fault tree as plain nodes and edges.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Diagram {
    /// Every gate and event, each once.
    pub nodes: Vec<Node>,
    /// The top event's index.
    pub root: usize,
}

/// One drawn occurrence of a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// Which [`Diagram::nodes`] entry this occurrence draws.
    pub node: usize,
    /// Horizontal position, world units.
    pub x: f64,
    /// Level below the root (the root is 0).
    pub depth: usize,
    /// The occurrence this one hangs from, as an index into
    /// [`Layout::placed`]; `None` for the root.
    pub parent: Option<usize>,
    /// A repeated gate drawn as a transfer symbol, not expanded again.
    pub transfer: bool,
}

/// Where [`layout`] put every occurrence.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Layout {
    /// Occurrences in depth-first pre-order; the root is first.
    pub placed: Vec<Placed>,
    /// Number of leaf slots used, so the width is `leaf_count * SLOT_W`.
    pub leaf_count: usize,
    /// Deepest level reached.
    pub max_depth: usize,
}

impl Layout {
    /// World position of occurrence `i`: `(x, -depth * LEVEL_H)`.
    pub fn world(&self, i: usize) -> [f64; 2] {
        let p = self.placed[i];
        [p.x, -(p.depth as f64) * LEVEL_H]
    }

    /// Centre of the drawing's bounding box, world units (Reset centres
    /// here).
    pub fn centre(&self) -> [f64; 2] {
        let width = self.leaf_count.saturating_sub(1) as f64 * SLOT_W;
        [0.5 * width, -0.5 * self.max_depth as f64 * LEVEL_H]
    }

    /// Half the larger side of the bounding box plus one slot of margin,
    /// world units: what Reset fits to the screen.
    pub fn half_extent(&self) -> f64 {
        let width = self.leaf_count.saturating_sub(1) as f64 * SLOT_W;
        let height = self.max_depth as f64 * LEVEL_H;
        0.5 * width.max(height) + SLOT_W
    }

    /// Every occurrence that draws node `node`, in pre-order.
    pub fn occurrences_of(&self, node: usize) -> Vec<usize> {
        self.placed
            .iter()
            .enumerate()
            .filter(|(_, p)| p.node == node)
            .map(|(i, _)| i)
            .collect()
    }
}

/// Per-occurrence marks that highlight `nodes` (for example a minimal cut
/// set's basic events) and every gate above each of their occurrences, and
/// dim the rest. An empty `nodes` marks everything [`Mark::Normal`]. The
/// result is indexed like [`Layout::placed`].
pub fn highlight(layout: &Layout, nodes: &[usize]) -> Vec<Mark> {
    if nodes.is_empty() {
        return vec![Mark::Normal; layout.placed.len()];
    }
    let mut marks = vec![Mark::Dim; layout.placed.len()];
    for (i, p) in layout.placed.iter().enumerate() {
        if nodes.contains(&p.node) {
            let mut at = Some(i);
            while let Some(j) = at {
                marks[j] = Mark::Highlight;
                at = layout.placed[j].parent;
            }
        }
    }
    marks
}

/// Lay `diagram` out as a tidy tree (see the module docs for the method).
///
/// An out-of-range root or child index is skipped rather than panicking, so
/// a malformed diagram draws what it can.
pub fn layout(diagram: &Diagram) -> Layout {
    let n = diagram.nodes.len();
    if diagram.root >= n {
        return Layout::default();
    }
    // 1. Unfold in pre-order. The stack holds (node, depth, parent occurrence).
    let mut placed: Vec<Placed> = Vec::new();
    let mut kids: Vec<Vec<usize>> = Vec::new();
    let mut expanded = vec![false; n];
    let mut stack: Vec<(usize, usize, Option<usize>)> = vec![(diagram.root, 0, None)];
    while let Some((node, depth, parent)) = stack.pop() {
        let entry = &diagram.nodes[node];
        let gate = entry.symbol.is_gate() && !entry.children.is_empty();
        let transfer = gate && expanded[node];
        let here = placed.len();
        placed.push(Placed { node, x: 0.0, depth, parent, transfer });
        kids.push(Vec::new());
        if let Some(p) = parent {
            kids[p].push(here);
        }
        if gate && !transfer {
            expanded[node] = true;
            for &c in entry.children.iter().rev() {
                if c < n {
                    stack.push((c, depth + 1, Some(here)));
                }
            }
        }
    }
    // 2. Place: leaves in pre-order take slots; then parents, children first
    // (reverse pre-order visits every child before its parent).
    let mut slot = 0usize;
    for (i, p) in placed.iter_mut().enumerate() {
        if kids[i].is_empty() {
            p.x = slot as f64 * SLOT_W;
            slot += 1;
        }
    }
    for i in (0..placed.len()).rev() {
        if let (Some(&first), Some(&last)) = (kids[i].first(), kids[i].last()) {
            placed[i].x = 0.5 * (placed[first].x + placed[last].x);
        }
    }
    let max_depth = placed.iter().map(|p| p.depth).max().unwrap_or(0);
    Layout { placed, leaf_count: slot, max_depth }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(label: &str, symbol: Symbol, children: &[usize]) -> Node {
        Node { label: String::from(label), detail: String::new(), symbol, children: children.to_vec() }
    }

    /// SCRAM's `Theatre` model (upstream `input/Theatre/theatre.xml`):
    /// `Theatre = AND(Mains_Fail, Generator)`, `Generator = OR(Gen_Fail,
    /// Relay_Fail)`. Positions pinned: three leaves in slots 0, 1, 2; the OR
    /// over its two leaves at 1.5; the AND over slot 0 and the OR at 0.75.
    #[test]
    fn a_small_tree_lays_out_at_pinned_positions() {
        let d = Diagram {
            nodes: vec![
                node("Theatre", Symbol::And, &[1, 2]),
                node("Mains_Fail", Symbol::BasicEvent, &[]),
                node("Generator", Symbol::Or, &[3, 4]),
                node("Gen_Fail", Symbol::BasicEvent, &[]),
                node("Relay_Fail", Symbol::BasicEvent, &[]),
            ],
            root: 0,
        };
        let l = layout(&d);
        let got: Vec<(usize, f64, usize, Option<usize>)> =
            l.placed.iter().map(|p| (p.node, p.x, p.depth, p.parent)).collect();
        assert_eq!(
            got,
            vec![
                (0, 0.75, 0, None),
                (1, 0.0, 1, Some(0)),
                (2, 1.5, 1, Some(0)),
                (3, 1.0, 2, Some(2)),
                (4, 2.0, 2, Some(2)),
            ]
        );
        assert_eq!((l.leaf_count, l.max_depth), (3, 2));
        assert_eq!(l.world(3), [1.0, -2.0]);
        assert_eq!(l.centre(), [1.0, -1.0]);
        assert_eq!(l.half_extent(), 2.0);
        assert!(l.placed.iter().all(|p| !p.transfer));
        // Same input, same layout: it is a pure function.
        assert_eq!(layout(&d), l);
    }

    /// A gate shared by two parents is expanded once and drawn as a transfer
    /// the second time; a shared basic event is a leaf both times. A cycle
    /// (malformed) ends in a transfer instead of looping, and bad indices are
    /// skipped.
    #[test]
    fn shared_gates_become_transfers_and_malformed_input_terminates() {
        let d = Diagram {
            nodes: vec![
                node("Top", Symbol::Or, &[1, 2]),
                node("Left", Symbol::And, &[3, 4]),
                node("Right", Symbol::And, &[3, 1]),
                node("Shared", Symbol::Or, &[4, 5]),
                node("e", Symbol::BasicEvent, &[]),
                node("h", Symbol::HouseEvent(true), &[]),
            ],
            root: 0,
        };
        let l = layout(&d);
        let shared = l.occurrences_of(3);
        assert_eq!(shared.len(), 2);
        assert!(!l.placed[shared[0]].transfer && l.placed[shared[1]].transfer);
        let left = l.occurrences_of(1);
        assert!(!l.placed[left[0]].transfer && l.placed[left[1]].transfer);
        assert_eq!(l.occurrences_of(4).len(), 2, "the event is drawn wherever it occurs");
        // No two leaves share a slot.
        let mut xs: Vec<f64> = l
            .placed
            .iter()
            .enumerate()
            .filter(|(i, _)| !l.placed.iter().any(|q| q.parent == Some(*i)))
            .map(|(_, p)| p.x)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        assert_eq!(xs.len(), l.leaf_count);

        let cyclic = Diagram {
            nodes: vec![node("A", Symbol::Or, &[1, 9]), node("B", Symbol::And, &[0])],
            root: 0,
        };
        let c = layout(&cyclic);
        assert_eq!(c.placed.len(), 3, "A, B, then A again as a transfer");
        assert!(c.placed[2].transfer);
        assert_eq!(layout(&Diagram { nodes: vec![], root: 0 }), Layout::default());
    }

    /// Highlighting a basic event lights it and the gates above it, in
    /// every place it occurs, and dims the rest; nothing listed, nothing
    /// dimmed.
    #[test]
    fn highlight_lights_members_and_their_ancestors() {
        let d = Diagram {
            nodes: vec![
                node("Theatre", Symbol::And, &[1, 2]),
                node("Mains_Fail", Symbol::BasicEvent, &[]),
                node("Generator", Symbol::Or, &[3, 4]),
                node("Gen_Fail", Symbol::BasicEvent, &[]),
                node("Relay_Fail", Symbol::BasicEvent, &[]),
            ],
            root: 0,
        };
        let l = layout(&d);
        assert_eq!(
            highlight(&l, &[3]),
            vec![Mark::Highlight, Mark::Dim, Mark::Highlight, Mark::Highlight, Mark::Dim]
        );
        assert_eq!(
            highlight(&l, &[1, 4]),
            vec![Mark::Highlight, Mark::Highlight, Mark::Highlight, Mark::Dim, Mark::Highlight]
        );
        assert!(highlight(&l, &[]).iter().all(|m| *m == Mark::Normal));
    }

    /// Symbols: gates versus events, and the text each draws.
    #[test]
    fn symbols_classify_and_label() {
        assert!(Symbol::AtLeast(2).is_gate() && !Symbol::BasicEvent.is_gate());
        assert!(!Symbol::HouseEvent(false).is_gate());
        assert_eq!(Symbol::AtLeast(2).short_text(), "2/n");
        assert_eq!(Symbol::HouseEvent(true).short_text(), "T");
        assert_eq!(Symbol::BasicEvent.short_text(), "");
        for s in [Symbol::And, Symbol::Or, Symbol::Xor, Symbol::Not, Symbol::Nand, Symbol::Nor, Symbol::Null] {
            assert!(!s.short_text().is_empty());
        }
    }
}
