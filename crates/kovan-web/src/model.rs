//! The Code Review UI's pure model: no egui, no I/O, so it is tested
//! headlessly and builds on every target (Android included).
//!
//! - [`ModTree`]: a crate's **module tree** from the call graph's
//!   `Module::parent` links (#737 schema 1). One source file is one module:
//!   inline `mod x { … }` blocks are not separate modules in that data, so
//!   their functions count as their file's.
//! - [`tree_layout`]: where each module card goes in the crate view, a
//!   two-sided mind map around the crate card.
//! - [`DeepLink`]: the page's `#…` hash, parsed and written.
//! - [`js_map_url`], [`home_url`]: the side panel's way back to the site's
//!   JavaScript code map and home page.
//! - [`Facts`]: per-function lookups (stamp state, maturity, callees, the
//!   "blocked by" list of the review bar).
//! - [`rustdoc_module_page`], [`rustdoc_fn_pages`]: rustdoc's URL scheme.

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::call_graph::split::{CrateSlice, StampState, StampVerdict};
use kovan_common::call_graph::{CrateGraph, FnKind, Function, Module, TargetKind};
use kovan_common::code_map::CodeMap;
use kovan_common::geometry::Point;

/// One module (source file) of a crate.
#[derive(Debug, Clone, PartialEq)]
pub struct ModNode {
    pub file: String,
    /// Path from the target root, `a::b`; empty for a target root.
    pub path: String,
    /// The card's title: the last path segment, or the target's name for a
    /// target root.
    pub name: String,
    pub kind: TargetKind,
    /// The Cargo target's name.
    pub target: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub functions: usize,
    pub maturity: Option<u8>,
    pub test: bool,
}

/// The module tree of one crate: the library's, then each example's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModTree {
    pub nodes: Vec<ModNode>,
    /// The library root (`lib.rs`), if the crate has a library.
    pub lib_root: Option<usize>,
    /// Each example's root file, by example name.
    pub example_roots: Vec<usize>,
}

impl ModTree {
    /// Build the tree of `krate`. Modules under `#[cfg(test)]` are left out
    /// unless `tests`.
    pub fn build(krate: &CrateGraph, tests: bool) -> ModTree {
        let mut tree = ModTree::default();
        for t in &krate.targets {
            let mods: Vec<&Module> = t.modules.iter().filter(|m| tests || !m.test).collect();
            let base = tree.nodes.len();
            let index: BTreeMap<&str, usize> =
                mods.iter().enumerate().map(|(i, m)| (m.file.as_str(), base + i)).collect();
            for m in &mods {
                let name = if m.path.is_empty() {
                    t.name.clone()
                } else {
                    m.path.rsplit("::").next().unwrap_or(&m.path).to_string()
                };
                tree.nodes.push(ModNode {
                    file: m.file.clone(),
                    path: m.path.clone(),
                    name,
                    kind: t.kind,
                    target: t.name.clone(),
                    parent: m.parent.as_deref().and_then(|p| index.get(p).copied()),
                    children: Vec::new(),
                    functions: m.functions.iter().filter(|f| tests || !f.test).count(),
                    maturity: m.maturity,
                    test: m.test,
                });
            }
            for i in base..tree.nodes.len() {
                if let Some(p) = tree.nodes[i].parent {
                    tree.nodes[p].children.push(i);
                }
            }
            for i in base..tree.nodes.len() {
                let mut ch = std::mem::take(&mut tree.nodes[i].children);
                ch.sort_by(|a, b| tree.nodes[*a].path.cmp(&tree.nodes[*b].path));
                tree.nodes[i].children = ch;
            }
            let root = (base..tree.nodes.len()).find(|&i| tree.nodes[i].path.is_empty() && tree.nodes[i].parent.is_none());
            match (t.kind, root) {
                (TargetKind::Lib, Some(r)) => tree.lib_root = Some(r),
                (TargetKind::Example, Some(r)) => tree.example_roots.push(r),
                _ => {}
            }
        }
        tree
    }

    /// The node of `file`.
    pub fn find(&self, file: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.file == file)
    }

    /// The top-level modules of the library (children of `lib.rs`).
    pub fn top_level(&self) -> Vec<usize> {
        self.lib_root.map(|r| self.nodes[r].children.clone()).unwrap_or_default()
    }

    /// `node` and its ancestors, root first.
    pub fn ancestry(&self, node: usize) -> Vec<usize> {
        let mut v = vec![node];
        while let Some(p) = self.nodes[*v.last().unwrap()].parent {
            v.push(p);
        }
        v.reverse();
        v
    }

    /// Every node in display order (depth first, children by path), with
    /// its depth: the library first, then each example.
    pub fn depth_first(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(usize, usize)> =
            self.example_roots.iter().rev().map(|&r| (r, 0)).chain(self.lib_root.map(|r| (r, 0))).collect();
        while let Some((n, d)) = stack.pop() {
            out.push((n, d));
            for &c in self.nodes[n].children.iter().rev() {
                stack.push((c, d + 1));
            }
        }
        out
    }
}

/// What a card in the crate view stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TreeItem {
    /// The crate itself (its `lib.rs`), at the origin.
    Crate,
    /// The "examples" group card.
    Examples,
    Module(usize),
}

/// The crate view's cards: centre and parent of each.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeLayout {
    pub cards: Vec<(TreeItem, Point, Option<TreeItem>)>,
}

impl TreeLayout {
    pub fn position(&self, item: TreeItem) -> Option<Point> {
        self.cards.iter().find(|c| c.0 == item).map(|c| c.1)
    }
}

/// Module card size in the crate view, world units. A drawing choice.
pub const MOD_CARD: (f64, f64) = (230.0, 64.0);
/// Horizontal distance between tree levels (card width plus room for the
/// connector), world units.
pub const LEVEL_DX: f64 = 290.0;
/// Vertical distance between neighbouring leaves.
pub const LEAF_DY: f64 = 78.0;

/// Lay out `tree` as a two-sided mind map: the crate card at the origin,
/// the library's top-level modules (and an "examples" group card holding
/// each example's root) split between the right and the left so the two
/// sides hold about as many leaves, each subtree's children stacked in path
/// order with the parent level with the middle of its children. A
/// module in `collapsed` shows no children. Deterministic: same tree, same
/// layout.
pub fn tree_layout(tree: &ModTree, collapsed: &BTreeSet<String>) -> TreeLayout {
    fn kids(tree: &ModTree, collapsed: &BTreeSet<String>, item: TreeItem) -> Vec<TreeItem> {
        match item {
            TreeItem::Crate => {
                let mut v: Vec<TreeItem> = tree.top_level().into_iter().map(TreeItem::Module).collect();
                if !tree.example_roots.is_empty() {
                    v.push(TreeItem::Examples);
                }
                v
            }
            TreeItem::Examples => tree.example_roots.iter().map(|&r| TreeItem::Module(r)).collect(),
            TreeItem::Module(i) => {
                if collapsed.contains(&tree.nodes[i].file) {
                    Vec::new()
                } else {
                    tree.nodes[i].children.iter().map(|&c| TreeItem::Module(c)).collect()
                }
            }
        }
    }
    fn leaves(tree: &ModTree, collapsed: &BTreeSet<String>, item: TreeItem) -> usize {
        let k = kids(tree, collapsed, item);
        if k.is_empty() {
            1
        } else {
            k.into_iter().map(|c| leaves(tree, collapsed, c)).sum()
        }
    }
    /// Place `item` at depth `depth` on side `side` (+1 right, -1 left),
    /// its leaves from `*next_y` down; returns its y.
    #[allow(clippy::too_many_arguments)]
    fn place(
        tree: &ModTree,
        collapsed: &BTreeSet<String>,
        item: TreeItem,
        parent: TreeItem,
        depth: usize,
        side: f64,
        next_y: &mut f64,
        out: &mut Vec<(TreeItem, Point, Option<TreeItem>)>,
    ) -> f64 {
        let k = kids(tree, collapsed, item);
        let y = if k.is_empty() {
            let y = *next_y;
            *next_y += LEAF_DY;
            y
        } else {
            let ys: Vec<f64> =
                k.iter().map(|&c| place(tree, collapsed, c, item, depth + 1, side, next_y, out)).collect();
            0.5 * (ys[0] + ys[ys.len() - 1])
        };
        out.push((item, Point::new(side * LEVEL_DX * depth as f64, y), Some(parent)));
        y
    }
    let top = kids(tree, collapsed, TreeItem::Crate);
    let weights: Vec<usize> = top.iter().map(|&c| leaves(tree, collapsed, c)).collect();
    let total: usize = weights.iter().sum();
    // The first children go right until the right holds half the leaves.
    let mut right = Vec::new();
    let mut left = Vec::new();
    let mut acc = 0;
    for (c, w) in top.iter().zip(&weights) {
        if acc * 2 < total || right.is_empty() {
            right.push(*c);
            acc += w;
        } else {
            left.push(*c);
        }
    }
    let mut cards = Vec::new();
    for (side, items) in [(1.0, &right), (-1.0, &left)] {
        let mut next_y = 0.0;
        let start = cards.len();
        for &c in items.iter() {
            place(tree, collapsed, c, TreeItem::Crate, 1, side, &mut next_y, &mut cards);
        }
        // Centre this side's column on the crate card.
        let shift = 0.5 * (next_y - LEAF_DY);
        for card in &mut cards[start..] {
            card.1.y -= shift;
        }
    }
    cards.push((TreeItem::Crate, Point::new(0.0, 0.0), None));
    cards.sort_by(|a, b| a.0.cmp(&b.0));
    TreeLayout { cards }
}

/// What the page's `#…` hash points at (deep links, #738).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLink {
    /// No hash: the code map.
    Map,
    /// `#crate=boon-lay`.
    Crate(String),
    /// `#module=crates/boon-lay/src/a/b.rs`.
    Module(String),
    /// `#crates/boon-lay/src/a.rs::Type::f` (the function id itself), with
    /// `source` set by a `src=` prefix: `#src=crates/…::f` opens the source
    /// panel too (the pop-out tab's link).
    Function { id: String, source: bool },
}

/// `%XX` decoding of a URL fragment.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl DeepLink {
    /// Parse a hash, with or without its leading `#`.
    pub fn parse(hash: &str) -> DeepLink {
        let h = percent_decode(hash.strip_prefix('#').unwrap_or(hash));
        let h = h.trim();
        if let Some(c) = h.strip_prefix("crate=") {
            DeepLink::Crate(c.to_string())
        } else if let Some(m) = h.strip_prefix("module=") {
            DeepLink::Module(m.to_string())
        } else if let Some(f) = h.strip_prefix("src=") {
            DeepLink::Function { id: f.to_string(), source: true }
        } else if let Some(f) = h.strip_prefix("fn=") {
            DeepLink::Function { id: f.to_string(), source: false }
        } else if h.contains("::") {
            DeepLink::Function { id: h.to_string(), source: false }
        } else {
            DeepLink::Map
        }
    }

    /// The hash, `#` included (`""` for the map).
    pub fn to_hash(&self) -> String {
        match self {
            DeepLink::Map => String::new(),
            DeepLink::Crate(c) => format!("#crate={c}"),
            DeepLink::Module(m) => format!("#module={m}"),
            DeepLink::Function { id, source: false } => format!("#{id}"),
            DeepLink::Function { id, source: true } => format!("#src={id}"),
        }
    }
}

/// The site's JavaScript code map (`code-map/`), opened on `krate` when one
/// is given: `code-map/#<crate>`, which that page selects and centres on
/// load (`docs/site/code-map/index.html`). `site_root` is
/// [`crate::data::Store::site_root`] (`"../"` on the page, the published
/// site natively). The side panel's "Go back to JavaScript map" button
/// (maintainer, 2026-10-07).
pub fn js_map_url(site_root: &str, krate: Option<&str>) -> String {
    match krate {
        Some(c) if !c.is_empty() => format!("{site_root}code-map/#{}", percent_encode(c)),
        _ => format!("{site_root}code-map/"),
    }
}

/// The site's home page, for the side panel's "Go to homepage" button.
pub fn home_url(site_root: &str) -> String {
    format!("{site_root}index.html")
}

/// `%XX` encoding of everything but `A-Z a-z 0-9 - _ . ~`, the inverse of
/// [`percent_decode`] (and of JavaScript's `decodeURIComponent`).
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The workspace crate whose folder holds `path` (a file or a function id),
/// by the longest matching `dir/` prefix in the code map.
pub fn crate_of<'a>(map: &'a CodeMap, path: &str) -> Option<&'a str> {
    map.crates
        .iter()
        .filter_map(|c| c.dir.as_deref().map(|d| (c.name.as_str(), d)))
        .filter(|(_, d)| path.starts_with(&format!("{d}/")))
        .max_by_key(|(_, d)| d.len())
        .map(|(n, _)| n)
}

/// The file part of a function id (`file.rs::Type::f` -> `file.rs`).
pub fn file_of(id: &str) -> &str {
    match id.find(".rs::") {
        Some(i) => &id[..i + 3],
        None => id,
    }
}

/// The name part of a function id (`file.rs::Type::f#2` -> `Type::f`).
pub fn short_name(id: &str) -> &str {
    let rest = match id.find(".rs::") {
        Some(i) => &id[i + 5..],
        None => id,
    };
    rest.split('#').next().unwrap_or(rest)
}

/// A function's review state as the bar shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Review {
    Unreviewed,
    Valid(StampState),
    Stale(StampState),
}

impl Review {
    pub fn label(&self) -> &'static str {
        match self {
            Review::Unreviewed => "unreviewed",
            Review::Valid(_) => "valid",
            Review::Stale(_) => "stale",
        }
    }
    pub fn is_valid(&self) -> bool {
        matches!(self, Review::Valid(_))
    }
}

/// Lookups over everything loaded so far.
#[derive(Debug, Default, Clone)]
pub struct Facts {
    stamps: BTreeMap<String, StampState>,
}

impl Facts {
    pub fn new(stamps: &[StampState]) -> Facts {
        Facts { stamps: stamps.iter().map(|s| (s.function.clone(), s.clone())).collect() }
    }

    /// The review state of function `id`.
    pub fn review(&self, id: &str) -> Review {
        match self.stamps.get(id) {
            None => Review::Unreviewed,
            Some(s) if s.verdict == StampVerdict::Valid => Review::Valid(s.clone()),
            Some(s) => Review::Stale(s.clone()),
        }
    }

    /// The distinct workspace functions `id` calls (itself excluded), in
    /// `(crate, id)` order: every resolved call in the data is to a
    /// workspace function (std and dependencies are dropped at build).
    pub fn callees(&self, map: &CodeMap, slice: &CrateSlice, id: &str) -> Vec<String> {
        let mut v: Vec<(String, String)> = slice
            .calls
            .iter()
            .filter(|c| c.from == id && c.to != id)
            .map(|c| (crate_of(map, &c.to).unwrap_or("").to_string(), c.to.clone()))
            .collect();
        v.sort();
        v.dedup();
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// The distinct functions that call `id` (itself excluded), sorted, from
    /// the slice of `id`'s crate (which holds every call into it from the
    /// crates in the build's scope).
    pub fn callers(&self, slice: &CrateSlice, id: &str) -> Vec<String> {
        let mut v: Vec<String> = slice.calls.iter().filter(|c| c.to == id && c.from != id).map(|c| c.from.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// The functions `id` calls that lack a valid stamp: what blocks its own
    /// stamp under the bottom-up rule (#740). UNRESOLVED calls are not
    /// counted (they are not in `calls`).
    pub fn blocked_by(&self, map: &CodeMap, slice: &CrateSlice, id: &str) -> Vec<String> {
        self.callees(map, slice, id).into_iter().filter(|c| !self.review(c).is_valid()).collect()
    }
}

/// A function and the module it is in, by id (integration-test targets
/// included, schema 2).
pub fn find_function<'a>(krate: &'a CrateGraph, id: &str) -> Option<(&'a Module, &'a Function, TargetKind)> {
    krate.targets.iter().chain(krate.tests.iter()).find_map(|t| {
        t.modules.iter().find_map(|m| m.functions.iter().find(|f| f.id == id).map(|f| (m, f, t.kind)))
    })
}

/// The module with this file (integration-test targets included).
pub fn find_module<'a>(krate: &'a CrateGraph, file: &str) -> Option<(&'a Module, TargetKind, &'a str)> {
    krate
        .targets
        .iter()
        .chain(krate.tests.iter())
        .find_map(|t| t.modules.iter().find(|m| m.file == file).map(|m| (m, t.kind, t.name.as_str())))
}

/// A crate name as rustdoc spells its folder (`boon-lay` -> `boon_lay`).
pub fn rustdoc_crate(name: &str) -> String {
    name.replace('-', "_")
}

/// The rustdoc page of a library module, relative to the `api/` folder:
/// `boon_lay/a/b/index.html` (`boon_lay/index.html` for the root).
pub fn rustdoc_module_page(krate: &str, module_path: &str) -> String {
    let mut s = rustdoc_crate(krate);
    for seg in module_path.split("::").filter(|s| !s.is_empty()) {
        s.push('/');
        s.push_str(seg);
    }
    s.push_str("/index.html");
    s
}

/// The rustdoc pages a library function may be documented on, relative to
/// `api/`, each `(page, fragment)`: `fn.f.html` for a free function, the
/// owner's `struct.`/`enum.`/`union.`/`trait.` page with `#method.f` (or
/// `#tymethod.f` for a trait declaration) for a method. The caller keeps the
/// first page that exists; a private item has none.
pub fn rustdoc_fn_pages(krate: &str, module_path: &str, f: &Function) -> Vec<(String, String)> {
    let dir = rustdoc_module_page(krate, module_path).trim_end_matches("index.html").to_string();
    let owner = f.owner.as_deref().map(|o| o.split('<').next().unwrap_or(o).trim().to_string());
    match (f.kind, owner) {
        (FnKind::Free, _) => vec![(format!("{dir}fn.{}.html", f.name), String::new())],
        (FnKind::TraitDecl, Some(o)) => {
            vec![(format!("{dir}trait.{o}.html"), format!("#tymethod.{}", f.name))]
        }
        (_, Some(o)) => ["struct", "enum", "union", "trait"]
            .iter()
            .map(|k| (format!("{dir}{k}.{o}.html"), format!("#method.{}", f.name)))
            .collect(),
        _ => Vec::new(),
    }
}

/// The maturity shown for a function: its module's (library modules carry
/// one), else the crate's tag.
pub fn function_maturity(module: &Module, map: &CodeMap, krate: &str) -> Option<u8> {
    module.maturity.or_else(|| map.get(krate).map(|c| c.maturity))
}

/// Which way a function card unfolds in the module view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    /// ▸, to the right: the functions it calls.
    Callees,
    /// ◂, to the left: the functions that call it.
    Callers,
}

/// The module view: the module's functions on a ring around the module
/// card, and every unfolded call level as a column, callees to the right
/// and callers to the left.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HierLayout {
    /// Ring cards, in ring order.
    pub ring: Vec<(String, Point)>,
    /// Unfolded cards: id, centre, column (`+d` callees, `-d` callers).
    pub unfolded: Vec<(String, Point, i32)>,
    /// `(caller, callee)` between two cards on screen.
    pub edges: Vec<(String, String)>,
    /// Ids whose callees or callers are not loaded yet (their crate's data).
    pub waiting: Vec<String>,
}

impl HierLayout {
    pub fn position(&self, id: &str) -> Option<Point> {
        self.ring.iter().find(|r| r.0 == id).map(|r| r.1).or_else(|| self.unfolded.iter().find(|u| u.0 == id).map(|u| u.1))
    }
    /// Every card centre.
    pub fn points(&self) -> impl Iterator<Item = Point> + '_ {
        self.ring.iter().map(|r| r.1).chain(self.unfolded.iter().map(|u| u.1))
    }
}

/// Distance between unfolded columns, world units.
pub const CALL_COLUMN_DX: f64 = 240.0;
/// Distance between cards in a column.
pub const CALL_ROW_DY: f64 = 62.0;

/// Lay out the module view. `ring_ids` are the module's functions (ring
/// order); `expanded` the unfolded `(id, direction)` pairs; `callees` and
/// `callers` answer for a function id, or `None` while its crate's data is
/// not loaded.
///
/// The ring is [`kovan_common::mindmap_view::star_positions`], the same ring
/// kovan's mind map draws (its cards never overlap). Unfolding is breadth
/// first: column 1 holds the callees of the unfolded ring cards, column 2
/// those of the unfolded column-1 cards, and so on; callers mirror to the
/// left. **A function already on screen is never drawn twice**: a further
/// call to it is an edge to the card that is there, so recursion and
/// diamonds stay finite. Ring cards unfold both ways; a callee-side card
/// only further right, a caller-side card only further left. Each column
/// is centred on the ring's middle, in the order its cards were reached.
pub fn hierarchy_layout(
    ring_ids: &[String],
    expanded: &BTreeSet<(String, Dir)>,
    callees: impl Fn(&str) -> Option<Vec<String>>,
    callers: impl Fn(&str) -> Option<Vec<String>>,
) -> HierLayout {
    use kovan_common::mindmap_view::{star_positions, CARD_SIZE};
    let ring_pts = star_positions(ring_ids.len());
    let ring: Vec<(String, Point)> = ring_ids.iter().cloned().zip(ring_pts).collect();
    let reach = ring.iter().map(|r| r.1.x.abs()).fold(0.0_f64, f64::max) + 0.5 * CARD_SIZE.0;
    let mut placed: BTreeSet<String> = ring_ids.iter().cloned().collect();
    let mut columns: BTreeMap<i32, Vec<String>> = BTreeMap::new();
    let mut edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut waiting = Vec::new();
    for (dir, sign) in [(Dir::Callees, 1), (Dir::Callers, -1)] {
        let mut frontier: Vec<String> = ring_ids.to_vec();
        let mut depth = 0;
        while !frontier.is_empty() && depth < 64 {
            depth += 1;
            let mut next = Vec::new();
            for id in &frontier {
                if !expanded.contains(&(id.clone(), dir)) {
                    continue;
                }
                let found = match dir {
                    Dir::Callees => callees(id),
                    Dir::Callers => callers(id),
                };
                let Some(list) = found else {
                    waiting.push(id.clone());
                    continue;
                };
                for other in list {
                    if &other == id {
                        continue;
                    }
                    edges.insert(match dir {
                        Dir::Callees => (id.clone(), other.clone()),
                        Dir::Callers => (other.clone(), id.clone()),
                    });
                    if placed.insert(other.clone()) {
                        columns.entry(sign * depth).or_default().push(other.clone());
                        next.push(other);
                    }
                }
            }
            frontier = next;
        }
    }
    // Every call between two cards on screen is drawn, unfolded or not, so
    // a cycle or a call back into the ring shows as an edge.
    for id in &placed {
        if let Some(list) = callees(id) {
            for other in list {
                if &other != id && placed.contains(&other) {
                    edges.insert((id.clone(), other));
                }
            }
        }
    }
    let mut unfolded = Vec::new();
    for (col, ids) in &columns {
        let x = col.signum() as f64 * (reach + 40.0 + 0.5 * CARD_SIZE.0 + CALL_COLUMN_DX * (col.abs() - 1) as f64);
        let top = -0.5 * CALL_ROW_DY * (ids.len() as f64 - 1.0);
        for (k, id) in ids.iter().enumerate() {
            unfolded.push((id.clone(), Point::new(x, top + CALL_ROW_DY * k as f64), *col));
        }
    }
    waiting.sort();
    waiting.dedup();
    HierLayout { ring, unfolded, edges: edges.into_iter().collect(), waiting }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::call_graph::{Call, CallKind, Target};

    fn func(file: &str, name: &str, line: u32) -> Function {
        Function {
            id: format!("{file}::{name}"),
            ambiguous: false,
            name: name.into(),
            kind: FnKind::Free,
            owner: None,
            trait_name: None,
            test: false,
            start_line: line,
            line,
            end_line: line + 2,
            signature: format!("fn {name}()"),
            doc: String::new(),
            source: String::new(),
            unresolved: Vec::new(),
            test_fn: false,
            reached_by: None,
            cited_by: Vec::new(),
        }
    }

    fn module(file: &str, path: &str, parent: Option<&str>, fns: Vec<Function>) -> Module {
        Module {
            file: file.into(),
            path: path.into(),
            parent: parent.map(Into::into),
            test: false,
            maturity: Some(1),
            functions: fns,
            upstream: None,
            upstream_unparsed: None,
            history: Vec::new(),
            concepts: Vec::new(),
        }
    }

    /// A crate shaped like boon-lay's `triso_atops_fork::activities::live_pools`.
    fn krate() -> CrateGraph {
        let d = "crates/bl/src";
        CrateGraph {
            name: "bl".into(),
            dir: "crates/bl".into(),
            maturity: Some(1),
            tests: Vec::new(),
            targets: vec![
                Target {
                    kind: TargetKind::Lib,
                    name: "bl".into(),
                    root: format!("{d}/lib.rs"),
                    modules: vec![
                        module(&format!("{d}/lib.rs"), "", None, vec![]),
                        module(&format!("{d}/fork/mod.rs"), "fork", Some(&format!("{d}/lib.rs")), vec![]),
                        module(&format!("{d}/fork/act/mod.rs"), "fork::act", Some(&format!("{d}/fork/mod.rs")), vec![]),
                        module(
                            &format!("{d}/fork/act/live_pools.rs"),
                            "fork::act::live_pools",
                            Some(&format!("{d}/fork/act/mod.rs")),
                            vec![func(&format!("{d}/fork/act/live_pools.rs"), "step", 10)],
                        ),
                        module(&format!("{d}/chem.rs"), "chem", Some(&format!("{d}/lib.rs")), vec![func(&format!("{d}/chem.rs"), "rate", 3)]),
                        Module { test: true, ..module(&format!("{d}/tests.rs"), "tests", Some(&format!("{d}/lib.rs")), vec![]) },
                    ],
                },
                Target {
                    kind: TargetKind::Example,
                    name: "demo".into(),
                    root: "crates/bl/examples/demo.rs".into(),
                    modules: vec![module("crates/bl/examples/demo.rs", "", None, vec![func("crates/bl/examples/demo.rs", "main", 1)])],
                },
            ],
        }
    }

    /// The tree follows the `parent` links, sorted by path, test modules
    /// out by default, examples as their own roots.
    #[test]
    fn the_tree_follows_the_mod_structure() {
        let t = ModTree::build(&krate(), false);
        let names: Vec<&str> = t.top_level().iter().map(|&i| t.nodes[i].name.as_str()).collect();
        assert_eq!(names, vec!["chem", "fork"]);
        let live = t.find("crates/bl/src/fork/act/live_pools.rs").unwrap();
        let chain: Vec<&str> = t.ancestry(live).iter().map(|&i| t.nodes[i].name.as_str()).collect();
        assert_eq!(chain, vec!["bl", "fork", "act", "live_pools"]);
        assert_eq!(t.nodes[live].functions, 1);
        assert_eq!(t.example_roots.len(), 1);
        assert!(t.find("crates/bl/src/tests.rs").is_none());
        assert!(ModTree::build(&krate(), true).find("crates/bl/src/tests.rs").is_some());
        let order: Vec<&str> = t.depth_first().iter().map(|&(i, _)| t.nodes[i].name.as_str()).collect();
        assert_eq!(order, vec!["bl", "chem", "fork", "act", "live_pools", "demo"]);
    }

    /// Every module (and the examples group) gets one card, no two cards
    /// overlap, children sit one level further out than their parent, and
    /// collapsing hides a subtree.
    #[test]
    fn the_crate_view_layout_places_every_module_without_overlap() {
        let t = ModTree::build(&krate(), false);
        let l = tree_layout(&t, &BTreeSet::new());
        // crate + examples group + 5 non-root lib/example modules (chem, fork,
        // act, live_pools, demo).
        assert_eq!(l.cards.len(), 7);
        for (i, a) in l.cards.iter().enumerate() {
            for b in &l.cards[i + 1..] {
                let apart = (a.1.x - b.1.x).abs() >= MOD_CARD.0 || (a.1.y - b.1.y).abs() >= MOD_CARD.1;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
            if let Some(p) = a.2 {
                let pp = l.position(p).unwrap();
                assert!((a.1.x.abs() - pp.x.abs() - LEVEL_DX).abs() < 1e-9, "{a:?} one level out");
            }
        }
        let live = t.find("crates/bl/src/fork/act/live_pools.rs").unwrap();
        let fork = t.find("crates/bl/src/fork/mod.rs").unwrap();
        let collapsed: BTreeSet<String> = [t.nodes[fork].file.clone()].into();
        let l2 = tree_layout(&t, &collapsed);
        assert!(l2.position(TreeItem::Module(live)).is_none());
        assert_eq!(tree_layout(&t, &BTreeSet::new()), l, "deterministic");
    }

    #[test]
    fn deep_links_round_trip() {
        for link in [
            DeepLink::Map,
            DeepLink::Crate("boon-lay".into()),
            DeepLink::Module("crates/boon-lay/src/a.rs".into()),
            DeepLink::Function { id: "crates/boon-lay/src/a.rs::T::f#2".into(), source: false },
            DeepLink::Function { id: "crates/boon-lay/src/a.rs::f".into(), source: true },
        ] {
            assert_eq!(DeepLink::parse(&link.to_hash()), link);
        }
        assert_eq!(
            DeepLink::parse("#crates/x/src/a.rs::T%3A%3Af"),
            DeepLink::Function { id: "crates/x/src/a.rs::T::f".into(), source: false }
        );
        assert_eq!(file_of("crates/x/src/a.rs::T::f"), "crates/x/src/a.rs");
        assert_eq!(short_name("crates/x/src/a.rs::T::f#2"), "T::f");
    }

    /// The side panel's links back to the site (maintainer, 2026-10-07).
    #[test]
    fn links_back_to_the_site() {
        assert_eq!(js_map_url("../", Some("outram-mc-libs")), "../code-map/#outram-mc-libs");
        assert_eq!(js_map_url("../", None), "../code-map/");
        assert_eq!(js_map_url("../", Some("")), "../code-map/");
        assert_eq!(
            js_map_url(crate::data::SITE_URL, Some("boon-lay")),
            "https://theodoreonzgit.github.io/outram-park-backend/code-map/#boon-lay"
        );
        assert_eq!(home_url("../"), "../index.html");
        assert_eq!(percent_encode("a b/é"), "a%20b%2F%C3%A9");
        assert_eq!(percent_decode(&percent_encode("a b/é#x")), "a b/é#x");
    }

    /// rustdoc's URL scheme: free functions, methods, module pages.
    #[test]
    fn rustdoc_urls_follow_rustdocs_scheme() {
        assert_eq!(rustdoc_module_page("boon-lay", "a::live_pools"), "boon_lay/a/live_pools/index.html");
        assert_eq!(rustdoc_module_page("boon-lay", ""), "boon_lay/index.html");
        let f = func("x.rs", "step", 1);
        assert_eq!(rustdoc_fn_pages("boon-lay", "a::b", &f), vec![("boon_lay/a/b/fn.step.html".to_string(), String::new())]);
        let m = Function { kind: FnKind::Method, owner: Some("Pool<T>".into()), ..func("x.rs", "y", 1) };
        let pages = rustdoc_fn_pages("boon-lay", "a", &m);
        assert_eq!(pages[0], ("boon_lay/a/struct.Pool.html".to_string(), "#method.y".to_string()));
    }

    /// Methodology: a ring of two functions where `a` calls `b` and `c`,
    /// `c` calls `a` (recursion back into the ring) and `d`, and `d` calls
    /// `c` (a cycle). Unfold `a` and then `c`, and `b`'s callers.
    ///
    /// Result (2026-10-06): every function is drawn once, the cycle and the
    /// call back into the ring are edges, columns sit outside the ring, and
    /// a function whose data is missing is reported as waiting.
    #[test]
    fn unfolding_draws_each_function_once() {
        let calls: BTreeMap<&str, Vec<&str>> =
            [("a", vec!["b", "c"]), ("c", vec!["a", "d"]), ("d", vec!["c"]), ("b", vec![])].into();
        let callees = |id: &str| calls.get(id).map(|v| v.iter().map(|s| s.to_string()).collect());
        let callers = |id: &str| {
            if id == "zz" {
                return None;
            }
            Some(calls.iter().filter(|(_, v)| v.contains(&id)).map(|(k, _)| k.to_string()).collect())
        };
        let ring = vec!["a".to_string(), "b".to_string()];
        let mut ex: BTreeSet<(String, Dir)> = BTreeSet::new();
        ex.insert(("a".into(), Dir::Callees));
        ex.insert(("c".into(), Dir::Callees));
        ex.insert(("b".into(), Dir::Callers));
        let l = hierarchy_layout(&ring, &ex, callees, callers);
        let ids: Vec<&str> = l.unfolded.iter().map(|u| u.0.as_str()).collect();
        assert_eq!(ids, vec!["c", "d"], "b and a are on the ring; c then d to the right");
        assert_eq!(l.unfolded[0].2, 1);
        assert_eq!(l.unfolded[1].2, 2);
        assert!(l.edges.contains(&("c".into(), "a".into())));
        assert!(l.edges.contains(&("d".into(), "c".into())));
        let ring_reach = l.ring.iter().map(|r| r.1.x.abs()).fold(0.0, f64::max);
        assert!(l.unfolded.iter().all(|u| u.1.x > ring_reach));
        assert!(l.waiting.is_empty());
        ex.insert(("zz".into(), Dir::Callers));
        let ring2 = vec!["zz".to_string()];
        let l2 = hierarchy_layout(&ring2, &ex, |_: &str| Some(vec![]), callers);
        assert_eq!(l2.waiting, vec!["zz".to_string()]);
    }

    /// With no stamps every callee blocks; a valid stamp unblocks it, a
    /// stale one does not; self-calls never count.
    #[test]
    fn blocked_by_lists_callees_without_a_valid_stamp() {
        let map = CodeMap {
            root: "r".into(),
            crates: vec![kovan_common::code_map::CrateNode {
                name: "bl".into(),
                description: None,
                row: 2,
                topic: kovan_common::code_map::Topic::Risk,
                fidelity: None,
                maturity: 1,
                maturity_modules: vec![],
                lib_dir: None,
                dir: Some("crates/bl".into()),
            }],
            edges: vec![],
        };
        let k = krate();
        let a = "crates/bl/src/chem.rs::rate".to_string();
        let b = "crates/bl/src/fork/act/live_pools.rs::step".to_string();
        let call = |from: &str, to: &str| Call { from: from.into(), to: to.into(), kind: CallKind::Call, lines: vec![1] };
        let slice = CrateSlice {
            schema: 1,
            krate: k,
            calls: vec![call(&b, &a), call(&b, &b), call(&b, "crates/other/src/lib.rs::z")],
            module_calls: vec![],
            crate_calls: vec![],
            outside: vec![],
            commit: None,
            site_base: None,
            files: vec![],
        };
        let none = Facts::new(&[]);
        assert_eq!(none.blocked_by(&map, &slice, &b).len(), 2);
        assert_eq!(none.review(&a), Review::Unreviewed);
        let stamp = |f: &str, v| StampState {
            function: f.into(),
            verdict: v,
            reason: String::new(),
            rung: 3,
            reviewer: "R".into(),
            date: "2026-10-06".into(),
            note: String::new(),
            permalink: String::new(),
        };
        let facts = Facts::new(&[stamp(&a, StampVerdict::Valid), stamp("crates/other/src/lib.rs::z", StampVerdict::Stale)]);
        assert_eq!(facts.blocked_by(&map, &slice, &b), vec!["crates/other/src/lib.rs::z".to_string()]);
        assert_eq!(crate_of(&map, &a), Some("bl"));
    }
}
