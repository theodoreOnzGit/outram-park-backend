//! Deterministic mindmap layout, camera and headless ASCII renderer
//! (op-30um.8).
//!
//! What belongs here: turning a [`crate::mindmap_model::MindmapModel`]'s
//! *currently visible* nodes into world-space positions, a camera that maps
//! world space to a viewport (Fit All / Centre Selection / zoom-at-cursor),
//! manual node pinning, and an [`ascii_render`] projection for headless
//! dogfooding — behaviour ported (not code-ported — see `bn show
//! op-30um.8`) from the maintainer's Python prototype
//! `collaboration/kovan-issue-35-prototypes/layer4-mindmap-ascii/mindmap_layout.py`.
//!
//! What does **not** belong here: which nodes are visible in the first
//! place (that is [`crate::mindmap_model::MindmapModel::visible_nodes`] —
//! this module only ever lays out what that function already decided is
//! visible); egui widgets/painting (that is [`crate::mindmap`]/`crate::app`,
//! out of scope for this pass); and any force/physics simulation — see the
//! design rule below, which is non-negotiable.
//!
//! # DESIGN RULE: hierarchy determines positions; relation edges are never
//! layout forces
//!
//! Stated last in the prototype, and repeated here because it is the one
//! rule a future "make it prettier" change is most likely to violate: the
//! [`layout`] function only ever reads [`MindmapModel::children_of`] and
//! parent/child structure. It never looks at
//! [`MindmapModel::visible_edges`] to attract or repel anything. This is
//! the concrete form of `op-9vo6`'s §45 non-goal, "no fancy mindmap physics
//! before the model works" — do not add a force-directed pass here. (The
//! existing hand-rolled/`egui_graphs` mindmap in [`crate::mindmap`] is a
//! separate, GUI-bound renderer and is untouched by this module.)
//!
//! # Determinism
//!
//! No wall clock, no RNG, no I/O anywhere in this module. [`layout`] is a
//! pure function of `(model, previous_state)`; [`ascii_render`] is a pure
//! function of `(model, layout_state, width)`. Calling either twice with
//! the same inputs must produce byte-identical output — see this module's
//! `ascii_render_is_deterministic` test.

use std::collections::HashMap;

use crate::mindmap_model::{MapNode, MindmapModel};

/// Camera zoom is always clamped to this range, matching
/// [`crate::mindmap_model::MindmapModel::zoom_by`]'s clamp on the
/// unrelated *semantic*-zoom factor. The two zooms are independent
/// concepts (this one is world-to-viewport scale; that one is
/// presentation detail) that simply happen to share a clamp range in the
/// prototype.
const ZOOM_MIN: f64 = 0.25;
const ZOOM_MAX: f64 = 3.0;

/// A point in world space (the same space [`layout`] places nodes in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned bounding box in world space, as produced by
/// [`bounds_for`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    /// Width, floored at `1.0` so a single-point or degenerate bounds
    /// never divides by zero in [`Camera::fit`].
    pub fn width(&self) -> f64 {
        (self.max_x - self.min_x).max(1.0)
    }

    /// Height, floored at `1.0` — see [`width`](Self::width).
    pub fn height(&self) -> f64 {
        (self.max_y - self.min_y).max(1.0)
    }

    /// The bounds' centre point.
    pub fn centre(&self) -> Point {
        Point::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }
}

/// The mindmap camera: a viewport size, a zoom factor, and a world-space
/// centre point. GUI-independent — `crate::app`/`crate::mindmap` reads
/// this to decide where to draw, but nothing here touches `egui`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub viewport_w: f64,
    pub viewport_h: f64,
    pub zoom: f64,
    pub centre: Point,
}

impl Camera {
    /// A camera over a `viewport_w x viewport_h` viewport, zoom `1.0`,
    /// centred on the world origin.
    pub fn new(viewport_w: f64, viewport_h: f64) -> Self {
        Self {
            viewport_w,
            viewport_h,
            zoom: 1.0,
            centre: Point::new(0.0, 0.0),
        }
    }

    /// "Fit All": centre on `bounds` and choose the largest zoom (clamped
    /// to `[0.25, 3.0]`) that still fits `bounds` inside the viewport after
    /// subtracting `padding` on every side.
    pub fn fit(&mut self, bounds: Bounds, padding: f64) {
        let usable_w = (self.viewport_w - 2.0 * padding).max(1.0);
        let usable_h = (self.viewport_h - 2.0 * padding).max(1.0);
        let fit_zoom = (usable_w / bounds.width()).min(usable_h / bounds.height());
        self.zoom = fit_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        self.centre = bounds.centre();
    }

    /// "Centre Selection": recentre on `p`. Zoom is deliberately
    /// unchanged — this is not [`fit`](Self::fit).
    pub fn centre_on(&mut self, p: Point) {
        self.centre = p;
    }

    /// Zoom by `factor` while keeping the world point `cursor_world`
    /// visually fixed under the cursor — i.e. after this call, the same
    /// world point still projects to the same viewport pixel it did
    /// before. A no-op if clamping leaves the zoom unchanged (e.g.
    /// already at the `3.0` ceiling and zooming in further).
    pub fn zoom_at(&mut self, factor: f64, cursor_world: Point) {
        let old = self.zoom;
        let new = (old * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        if new == old {
            return;
        }
        let ratio = old / new;
        self.centre = Point::new(
            cursor_world.x + (self.centre.x - cursor_world.x) * ratio,
            cursor_world.y + (self.centre.y - cursor_world.y) * ratio,
        );
        self.zoom = new;
    }
}

/// World-space positions for every currently laid-out node, plus which of
/// them are manually pinned.
#[derive(Debug, Clone, Default)]
pub struct LayoutState {
    positions: HashMap<String, Point>,
    pinned: HashMap<String, Point>,
}

impl LayoutState {
    /// An empty layout: no positions, nothing pinned.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current position of `node_id`, if [`layout`] has placed it.
    pub fn position(&self, node_id: &str) -> Option<Point> {
        self.positions.get(node_id).copied()
    }

    /// Whether `node_id` is currently manually pinned.
    pub fn is_pinned(&self, node_id: &str) -> bool {
        self.pinned.contains_key(node_id)
    }

    /// Manually pin `node_id` at `p` (e.g. after a user drag). A pinned
    /// node keeps this exact position across every subsequent
    /// [`layout`] call, until [`unpin`](Self::unpin)ned.
    pub fn pin(&mut self, node_id: &str, p: Point) {
        self.pinned.insert(node_id.to_string(), p);
        self.positions.insert(node_id.to_string(), p);
    }

    /// Release `node_id`'s manual pin — the next [`layout`] call is free
    /// to place it automatically again.
    pub fn unpin(&mut self, node_id: &str) {
        self.pinned.remove(node_id);
    }
}

/// Sort key used for both root ordering and sibling ordering: rank
/// collections before papers, then alphabetically by title, then by id as
/// a final tiebreak so the order is fully deterministic even when two
/// nodes share a title.
fn sort_key(node: &MapNode) -> (u8, String, String) {
    let rank = match node.kind {
        crate::mindmap_model::MapNodeKind::Collection(_) => 0,
        crate::mindmap_model::MapNodeKind::Paper => 1,
        crate::mindmap_model::MapNodeKind::Artifact(_) => 2,
    };
    (rank, node.title.to_lowercase(), node.id.clone())
}

/// Lay out `model`'s *currently visible* nodes (per
/// [`MindmapModel::visible_nodes`]) as a stable left-to-right hierarchy
/// diagram: each root starts a horizontal lane at `x = 0`, each child is
/// one `x_gap` further right than its parent, and a parent's `y` is the
/// average of its children's `y` (a leaf just takes the next free `y` in
/// its lane, `y_gap` apart). Independent roots are separated vertically by
/// `root_gap` so unrelated papers/collections don't run into each other.
///
/// **Only hierarchy drives this — see the module's DESIGN RULE.** Cross-
/// links ([`MindmapModel::visible_edges`]) are never consulted here.
///
/// `previous` carries forward pinned positions (see [`LayoutState::pin`])
/// so a relayout after a topology change (an expand/collapse, a focus)
/// never moves a node the user has manually placed, and — because the
/// automatic placement algorithm itself is a pure function of the visible
/// hierarchy — re-running `layout` on an unchanged model always places
/// every *unpinned* node at exactly the position it had before, too.
pub fn layout(
    model: &MindmapModel,
    previous: Option<&LayoutState>,
    x_gap: f64,
    y_gap: f64,
    root_gap: f64,
) -> LayoutState {
    let empty = LayoutState::new();
    let previous = previous.unwrap_or(&empty);
    let mut state = LayoutState {
        positions: HashMap::new(),
        pinned: previous.pinned.clone(),
    };

    let visible: HashMap<&str, &MapNode> =
        model.visible_nodes().into_iter().map(|n| (n.id.as_str(), n)).collect();

    // children[parent] = sorted child ids, restricted to visible nodes.
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for node in visible.values() {
        if let Some(parent) = &node.parent {
            if visible.contains_key(parent.as_str()) {
                children.entry(parent.clone()).or_default().push(node.id.clone());
            }
        }
    }
    for kids in children.values_mut() {
        kids.sort_by_key(|id| sort_key(visible[id.as_str()]));
    }

    let mut roots: Vec<&MapNode> = visible
        .values()
        .filter(|n| n.parent.as_deref().is_none_or(|p| !visible.contains_key(p)))
        .copied()
        .collect();
    roots.sort_by_key(|n| sort_key(n));

    let mut cursor_y = 0.0_f64;

    // Recursive placement, matching the prototype's `place`: a parent's y
    // is the mean of its children's y; a leaf claims the next free lane
    // slot. Written as an explicit stack-based walk rather than a
    // recursive closure, since Rust closures cannot straightforwardly
    // recurse into themselves without extra indirection.
    fn place(
        node_id: &str,
        depth: u32,
        x_gap: f64,
        y_gap: f64,
        cursor_y: &mut f64,
        children: &HashMap<String, Vec<String>>,
        pinned: &HashMap<String, Point>,
        positions: &mut HashMap<String, Point>,
    ) -> f64 {
        let kids = children.get(node_id);
        let y = match kids {
            Some(kids) if !kids.is_empty() => {
                let ys: Vec<f64> = kids
                    .iter()
                    .map(|k| place(k, depth + 1, x_gap, y_gap, cursor_y, children, pinned, positions))
                    .collect();
                ys.iter().sum::<f64>() / ys.len() as f64
            }
            _ => {
                let y = *cursor_y;
                *cursor_y += y_gap;
                y
            }
        };
        let auto = Point::new(depth as f64 * x_gap, y);
        let placed = pinned.get(node_id).copied().unwrap_or(auto);
        positions.insert(node_id.to_string(), placed);
        placed.y
    }

    for root in &roots {
        let start = cursor_y;
        place(
            &root.id,
            0,
            x_gap,
            y_gap,
            &mut cursor_y,
            &children,
            &state.pinned,
            &mut state.positions,
        );
        cursor_y = cursor_y.max(start + y_gap) + root_gap;
    }

    state
}

/// Lay out `model` with the prototype's default spacing
/// (`x_gap = 280`, `y_gap = 90`, `root_gap = 150`).
pub fn layout_default(model: &MindmapModel, previous: Option<&LayoutState>) -> LayoutState {
    layout(model, previous, 280.0, 90.0, 150.0)
}

/// The bounding box of every id in `visible_ids` that `state` has a
/// position for. Ids with no recorded position are silently skipped —
/// this is not an error (e.g. a stale id from before the last topology
/// change). Falls back to a unit box at the origin when nothing matches,
/// so [`Camera::fit`] always has *something* finite to fit.
pub fn bounds_for(state: &LayoutState, visible_ids: &[&str]) -> Bounds {
    let mut points = visible_ids.iter().filter_map(|id| state.position(id));
    let Some(first) = points.next() else {
        return Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1.0,
            max_y: 1.0,
        };
    };
    let mut bounds = Bounds {
        min_x: first.x,
        min_y: first.y,
        max_x: first.x,
        max_y: first.y,
    };
    for p in points {
        bounds.min_x = bounds.min_x.min(p.x);
        bounds.min_y = bounds.min_y.min(p.y);
        bounds.max_x = bounds.max_x.max(p.x);
        bounds.max_y = bounds.max_y.max(p.y);
    }
    bounds
}

/// A one-letter/short tag per [`crate::mindmap_model::MapNodeKind`], used
/// by [`ascii_render`]. Display only — never round-tripped back into a
/// kind.
fn icon(node: &MapNode) -> &'static str {
    use crate::artifact::ArtifactKind;
    use crate::entity::EntityKind;
    use crate::mindmap_model::MapNodeKind;
    match node.kind {
        MapNodeKind::Collection(EntityKind::Project) => "P",
        MapNodeKind::Collection(_) => "T",
        MapNodeKind::Paper => "PAPER",
        // A paper's own header artifact is drawn as the paper node itself,
        // never as a child of it.
        MapNodeKind::Artifact(ArtifactKind::Paper) => "PAPER",
        MapNodeKind::Artifact(ArtifactKind::Relation) => "REL",
        MapNodeKind::Artifact(ArtifactKind::Mindmap) => "MAP",
        MapNodeKind::Artifact(ArtifactKind::Note) => "NOTE",
        MapNodeKind::Artifact(ArtifactKind::Annotation) => "NOTE",
        MapNodeKind::Artifact(ArtifactKind::DigitisedGraph) => "GRAPH",
        MapNodeKind::Artifact(ArtifactKind::DigitisedTable) => "TABLE",
        MapNodeKind::Artifact(ArtifactKind::Formula) => "FORMULA",
        MapNodeKind::Artifact(ArtifactKind::SourceReference) => "REF",
    }
}

/// Render `model`'s currently visible hierarchy (per [`layout`]'s same
/// visibility rules — this function does not itself decide what is
/// visible, it only draws what [`MindmapModel::visible_nodes`] already
/// said) as a fixed-width ASCII tree, followed by a `RELATIONS` section
/// listing every currently visible edge.
///
/// This is a **harness check, not physics/UI validation** — it exists so
/// the mindmap's hierarchy and expansion behaviour can be asserted in a
/// plain `#[test]` with no window, exactly like this workspace's headless-
/// mode rule for egui simulators. It says nothing about whether the actual
/// egui rendering (colour, pixel layout, hit-testing) is correct.
///
/// `width` is accepted for API stability with the prototype (a future
/// wrapping/truncation pass may use it) but the current renderer does not
/// wrap long lines — every line's `[KIND] Title` may exceed `width` for a
/// long title, which is preferable to silently truncating a title a test
/// fixture then can't distinguish from another.
pub fn ascii_render(model: &MindmapModel, state: &LayoutState, _width: usize) -> String {
    let visible: HashMap<&str, &MapNode> =
        model.visible_nodes().into_iter().map(|n| (n.id.as_str(), n)).collect();

    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for node in visible.values() {
        if let Some(parent) = &node.parent {
            if visible.contains_key(parent.as_str()) {
                children.entry(parent.clone()).or_default().push(node.id.clone());
            }
        }
    }
    for kids in children.values_mut() {
        kids.sort_by_key(|id| sort_key(visible[id.as_str()]));
    }

    let mut roots: Vec<&MapNode> = visible
        .values()
        .filter(|n| n.parent.as_deref().is_none_or(|p| !visible.contains_key(p)))
        .copied()
        .collect();
    roots.sort_by_key(|n| sort_key(n));

    let mut lines: Vec<String> = Vec::new();

    fn walk(
        node_id: &str,
        prefix: &str,
        last: bool,
        model: &MindmapModel,
        visible: &HashMap<&str, &MapNode>,
        children: &HashMap<String, Vec<String>>,
        lines: &mut Vec<String>,
    ) {
        let node = visible[node_id];
        let kids = children.get(node_id);
        let selected = if model.selected() == Some(node_id) {
            "*"
        } else {
            " "
        };
        let expandable = kids.is_some_and(|k| !k.is_empty()) || model.children_of(node_id).len() > 0;
        let chevron = if model.is_expanded(node_id) && expandable {
            "v"
        } else if expandable {
            ">"
        } else {
            "-"
        };
        let branch = if last { "`-" } else { "|-" };
        lines.push(format!(
            "{prefix}{branch}{selected}{chevron} [{}] {}",
            icon(node),
            node.title
        ));
        let next_prefix = format!("{prefix}{}", if last { "  " } else { "| " });
        if let Some(kids) = kids {
            let n = kids.len();
            for (i, k) in kids.iter().enumerate() {
                walk(k, &next_prefix, i + 1 == n, model, visible, children, lines);
            }
        }
    }

    let n = roots.len();
    for (i, root) in roots.iter().enumerate() {
        walk(
            &root.id,
            "",
            i + 1 == n,
            model,
            &visible,
            &children,
            &mut lines,
        );
    }

    let edges = model.visible_edges();
    if !edges.is_empty() {
        lines.push(String::new());
        lines.push("RELATIONS".to_string());
        for e in edges {
            let source_title = visible
                .get(e.source.as_str())
                .map_or(e.source.as_str(), |n| n.title.as_str());
            let target_title = visible
                .get(e.target.as_str())
                .map_or(e.target.as_str(), |n| n.title.as_str());
            lines.push(format!(
                "  {source_title} --{}--> {target_title}",
                e.label
            ));
        }
    }

    let _ = state; // positions are not drawn by this projection (hierarchy/text only).
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::graph::KnowledgeGraph;
    use crate::index::KnowledgeIndex;
    use crate::mindmap_model::{build_model, TypedEdge};
    use crate::root::{KovanRoot, RootConfig};
    use crate::session::PaperSession;

    /// Same small library as `mindmap_model`'s tests: `terry2005` (an
    /// `Annotation`) and `iaea1694` (a `DigitisedGraph`), both under the
    /// `htgrs` topic.
    fn small_library() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();

        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();

        EntityConfig::paper(CiteKey::parse("terry2005").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("terry2005"))
            .unwrap();
        let mut terry = PaperSession::open(&root, "terry2005").unwrap();
        terry.append_block(
            "# Pebble packing note\n\n```toml\n[kovan]\nid = \"pebble-packing-note\"\nkind = \"annotation\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 4\n```\n",
        );
        terry.save_document().unwrap();

        EntityConfig::paper(CiteKey::parse("iaea1694").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("iaea1694"))
            .unwrap();
        let mut iaea = PaperSession::open(&root, "iaea1694").unwrap();
        iaea.append_block(
            "# Digitised curve\n\n```toml\n[kovan]\nid = \"digitised-curve\"\nkind = \"digitised_graph\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 9\n```\n",
        );
        iaea.save_document().unwrap();

        (dir, root)
    }

    #[test]
    fn camera_fit_all_clamps_zoom_and_centres_on_bounds() {
        let mut cam = Camera::new(1200.0, 800.0);
        let bounds = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 560.0,
            max_y: 180.0,
        };
        cam.fit(bounds, 80.0);
        assert_eq!(cam.centre, Point::new(280.0, 90.0));
        assert!(cam.zoom > 0.0 && cam.zoom <= 3.0);
    }

    #[test]
    fn camera_fit_all_never_exceeds_the_zoom_ceiling_on_a_tiny_bounds() {
        let mut cam = Camera::new(1200.0, 800.0);
        let bounds = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1.0,
            max_y: 1.0,
        };
        cam.fit(bounds, 80.0);
        assert_eq!(cam.zoom, 3.0);
    }

    #[test]
    fn camera_centre_on_changes_only_the_centre() {
        let mut cam = Camera::new(1200.0, 800.0);
        cam.zoom = 1.7;
        cam.centre_on(Point::new(42.0, -13.0));
        assert_eq!(cam.centre, Point::new(42.0, -13.0));
        assert_eq!(cam.zoom, 1.7);
    }

    #[test]
    fn camera_zoom_at_cursor_keeps_the_world_point_visually_fixed() {
        let mut cam = Camera::new(1200.0, 800.0);
        cam.centre = Point::new(100.0, 50.0);
        cam.zoom = 1.0;
        let cursor = Point::new(200.0, 150.0);

        // The screen-space offset of `cursor` from `centre`, scaled by
        // zoom, must be identical before and after.
        let before = ((cursor.x - cam.centre.x) * cam.zoom, (cursor.y - cam.centre.y) * cam.zoom);
        cam.zoom_at(1.25, cursor);
        let after = ((cursor.x - cam.centre.x) * cam.zoom, (cursor.y - cam.centre.y) * cam.zoom);
        assert!((before.0 - after.0).abs() < 1e-9);
        assert!((before.1 - after.1).abs() < 1e-9);
        assert!((cam.zoom - 1.25).abs() < 1e-9);
    }

    #[test]
    fn camera_zoom_at_cursor_respects_the_ceiling_and_is_a_no_op_past_it() {
        let mut cam = Camera::new(1200.0, 800.0);
        cam.zoom = 3.0;
        let before_centre = cam.centre;
        cam.zoom_at(2.0, Point::new(500.0, 500.0));
        assert_eq!(cam.zoom, 3.0);
        assert_eq!(cam.centre, before_centre);
    }

    #[test]
    fn relayout_after_expand_does_not_move_already_placed_nodes() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);

        let s1 = layout_default(&model, None);
        let terry_before = s1.position("paper:terry2005").unwrap();
        let iaea_before = s1.position("paper:iaea1694").unwrap();
        let collection_before = s1.position("collection:htgrs").unwrap();

        model.toggle("paper:terry2005");
        let s2 = layout_default(&model, Some(&s1));

        assert_eq!(s2.position("paper:terry2005").unwrap(), terry_before);
        assert_eq!(s2.position("paper:iaea1694").unwrap(), iaea_before);
        assert_eq!(s2.position("collection:htgrs").unwrap(), collection_before);
        assert!(s2.position("artifact:terry2005#pebble-packing-note").is_some());
    }

    #[test]
    fn pinned_node_survives_relayout_after_a_topology_change() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);
        model.toggle("paper:terry2005");

        let s1 = layout_default(&model, None);
        let mut s1 = s1;
        let pinned_at = Point::new(9999.0, -1234.0);
        s1.pin("artifact:terry2005#pebble-packing-note", pinned_at);
        assert_eq!(
            s1.position("artifact:terry2005#pebble-packing-note"),
            Some(pinned_at)
        );

        // A further topology change (expanding the other paper too) must
        // not move the pinned node.
        model.toggle("paper:iaea1694");
        let s2 = layout_default(&model, Some(&s1));
        assert_eq!(
            s2.position("artifact:terry2005#pebble-packing-note"),
            Some(pinned_at)
        );
        assert!(s2.is_pinned("artifact:terry2005#pebble-packing-note"));
    }

    #[test]
    fn layout_is_deterministic_across_repeated_calls() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);
        model.toggle("paper:terry2005");
        model.toggle("paper:iaea1694");

        let a = layout_default(&model, None);
        let b = layout_default(&model, None);

        let ids: Vec<&str> = model.visible_nodes().iter().map(|n| n.id.as_str()).collect();
        for id in ids {
            assert_eq!(a.position(id), b.position(id), "mismatch at {id}");
        }
    }

    #[test]
    fn ascii_render_is_deterministic() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let mut model = build_model(&root, &index, &graph, &[]);
        model.toggle("paper:terry2005");
        model.focus("artifact:terry2005#pebble-packing-note").unwrap();

        let state = layout_default(&model, None);
        let first = ascii_render(&model, &state, 112);
        let second = ascii_render(&model, &state, 112);
        assert_eq!(first, second, "ascii_render must be byte-identical across repeated calls");

        // Basic content sanity, so this isn't just testing "equals itself".
        assert!(first.contains("terry2005"));
        assert!(first.contains("Pebble packing note"));
        assert!(first.contains("*")); // the focused/selected marker
    }

    #[test]
    fn ascii_render_reflects_expansion_and_relations() {
        let (_dir, root) = small_library();
        let index = KnowledgeIndex::rebuild(&root);
        let graph = KnowledgeGraph::rebuild(&root, &index);
        let user_edges = [TypedEdge {
            source: "artifact:terry2005#pebble-packing-note".to_string(),
            target: "artifact:iaea1694#digitised-curve".to_string(),
            label: "uses_data_from".to_string(),
        }];
        let mut model = build_model(&root, &index, &graph, &user_edges);

        let collapsed_state = layout_default(&model, None);
        let collapsed = ascii_render(&model, &collapsed_state, 112);
        assert!(!collapsed.contains("Pebble packing note"));

        model.toggle("paper:terry2005");
        model.toggle("paper:iaea1694");
        let expanded_state = layout_default(&model, Some(&collapsed_state));
        let expanded = ascii_render(&model, &expanded_state, 112);
        assert!(expanded.contains("Pebble packing note"));
        assert!(expanded.contains("Digitised curve"));
        assert!(expanded.contains("uses_data_from"));
    }
}
