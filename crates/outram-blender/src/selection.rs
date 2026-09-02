// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Topological element selection for the mesh editor. Follows the published
// architecture of Blender's edit-mesh selection layer (source/blender/editors/
// mesh/editmesh_select.cc and the BM_select_* API in source/blender/bmesh,
// github.com/blender/blender, GPL-2.0-or-later): a per-element selection flag,
// three select "modes" (vertex / edge / face) with flushing between them, and
// the region / linked / mirror select operators built on top. Concepts only —
// no upstream source was copied; this is an index-set reimplementation.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Topological **element selection** — the state every Edit-Mode operator reads.
//!
//! Blender analogue: `editors/mesh/editmesh_select.cc` plus the `BM_select_*` /
//! `BM_elem_flag` API. In Blender selection is a per-element flag on the mesh;
//! here it is a separate [`Selection`] value holding three sorted index sets
//! (selected vertices, edges, faces) and the current [`SelectMode`]. Keeping it
//! out of [`crate::mesh::Mesh`] means a mesh stays a pure geometry container and
//! an operator takes `(&Mesh, &Selection)` explicitly.
//!
//! # What this module provides (GH issue #37 §A — `op-hzs.54.1`)
//!
//! - **Select modes** — [`SelectMode::Vertex`] / [`SelectMode::Edge`] /
//!   [`SelectMode::Face`], with [`Selection::set_mode`] doing Blender's
//!   *selection flush*: switching to a coarser domain keeps only fully-selected
//!   elements, switching to a finer domain selects every sub-element.
//! - **Whole-mesh ops** — [`Selection::select_all`], [`Selection::deselect_all`],
//!   [`Selection::invert`].
//! - **Single element** — [`Selection::select`], [`Selection::deselect`],
//!   [`Selection::toggle`], [`Selection::is_selected`] over an [`Element`].
//! - **Region select** — [`Selection::select_in_box`],
//!   [`Selection::select_in_sphere`] (headless, model-space) and
//!   [`Selection::select_in_screen_polygon`] (the box / circle / lasso tools —
//!   the caller supplies the projection, so any orthographic or perspective
//!   camera works). Each takes a [`RegionMode`] deciding whether an edge/face
//!   needs *all* or just *any* of its vertices inside.
//! - **Select linked** — [`Selection::select_linked`] (grow to whole connected
//!   components) and [`Selection::select_linked_from`] (pick one component from
//!   a seed, Blender's `L`). Delimiters (seam / sharp / material) arrive with
//!   the per-edge attribute layers in `op-hzs.54.28`.
//! - **Select mirror** — [`Selection::select_mirror`]: for each selected element
//!   also select its mirror image across an [`Axis`] plane through the origin,
//!   matched by position within a tolerance.
//! - **Set algebra** — [`Selection::union`], [`Selection::subtract`],
//!   [`Selection::intersect`], [`Selection::retain`] compose the primitives
//!   above into Blender's extend / subtract / intersect box-select modes
//!   without widening the per-operator API.
//! - **Loop / ring** (`op-hzs.54.2`) — [`Selection::select_edge_loop`],
//!   [`Selection::select_edge_ring`], [`Selection::select_face_loop`],
//!   [`Selection::select_boundary_loop`], [`Selection::select_shortest_path`],
//!   over [`crate::topology`]'s adjacency walkers.
//! - **Grow / shrink / similar / nth** (`op-hzs.54.3`) —
//!   [`Selection::select_more`], [`Selection::select_less`],
//!   [`Selection::select_similar`] ([`SimilarTrait`]),
//!   [`Selection::checker_deselect`].
//! - **By trait** (`op-hzs.54.4`) — [`Selection::select_non_manifold`]
//!   ([`NonManifoldKinds`]), [`Selection::select_loose`],
//!   [`Selection::select_interior_faces`],
//!   [`Selection::select_faces_by_sides`] ([`NumberCompare`]). "Ungrouped
//!   vertices" waits on vertex groups in `op-hzs.54.28`.

use std::collections::{BTreeSet, HashMap};

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};
use crate::topology::{self, MeshTopology};

/// Which element domain selection operations act on (Blender's Edit-Mode
/// vertex / edge / face select-mode buttons).
///
/// The active mode decides what [`Selection::select_all`],
/// [`Selection::invert`], and the region-select operators add, and what a bare
/// [`Element`] is expected to be. [`Selection::set_mode`] converts an existing
/// selection when the mode changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectMode {
    /// Individual vertices.
    Vertex,
    /// Whole edges (both endpoints).
    Edge,
    /// Whole faces (every corner).
    Face,
}

/// One addressable mesh element, tagged by domain — the unit
/// [`Selection::select`] / [`Selection::deselect`] / [`Selection::toggle`] /
/// [`Selection::is_selected`] operate on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    /// A vertex by id.
    Vertex(VertexId),
    /// An edge by id.
    Edge(EdgeId),
    /// A face by id.
    Face(FaceId),
}

/// A coordinate axis — names the reflection plane for [`Selection::select_mirror`]
/// (the plane through the origin **orthogonal** to this axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// The Y-Z plane (mirror negates X).
    X,
    /// The X-Z plane (mirror negates Y).
    Y,
    /// The X-Y plane (mirror negates Z).
    Z,
}

impl Axis {
    /// Reflect `p` across the origin plane orthogonal to this axis.
    fn reflect(self, p: Vec3) -> Vec3 {
        match self {
            Axis::X => Vec3::new(-p.x, p.y, p.z),
            Axis::Y => Vec3::new(p.x, -p.y, p.z),
            Axis::Z => Vec3::new(p.x, p.y, -p.z),
        }
    }
}

/// Whether a multi-vertex element (edge or face) is caught by a region when
/// only *some* of its vertices lie inside.
///
/// Matches the two useful Blender box-select behaviours. Ignored when the
/// active [`SelectMode`] is [`SelectMode::Vertex`] (a vertex is simply in or
/// out).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionMode {
    /// Select the element if **any** vertex is inside the region (Blender's
    /// default — you can lasso part of a face).
    Touching,
    /// Select the element only if **all** its vertices are inside the region
    /// (a strict "fully enclosed" box select).
    Enclosed,
}

/// A trait that [`Selection::select_similar`] matches on (Blender's
/// `Select ▸ Select Similar`, `Shift+G`). Each variant is meaningful in one
/// [`SelectMode`]; calling it in the wrong mode is a no-op.
///
/// Attribute-backed traits (material, crease, bevel weight, seam, sharp, vertex
/// groups) arrive with the per-element attribute layers in `op-hzs.54.28`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimilarTrait {
    /// **Vertex** — number of connecting edges (valence). Exact match;
    /// threshold ignored.
    VertexValence,
    /// **Edge** — length. `threshold` is a relative tolerance
    /// (`|a - ref| <= threshold * max(|ref|, eps)`).
    EdgeLength,
    /// **Edge** — direction (undirected). `threshold` is the angle tolerance
    /// in radians.
    EdgeDirection,
    /// **Edge** — number of incident faces (1 = boundary, 2 = manifold).
    /// Exact match; threshold ignored.
    EdgeFaceCount,
    /// **Face** — area. `threshold` is a relative tolerance.
    FaceArea,
    /// **Face** — number of sides. Exact match; threshold ignored.
    FaceSides,
    /// **Face** — perimeter. `threshold` is a relative tolerance.
    FacePerimeter,
    /// **Face** — normal direction. `threshold` is the angle tolerance in
    /// radians.
    FaceNormal,
    /// **Face** — coplanar with a selected face: normals parallel within
    /// ~0.5° **and** plane offset equal within `threshold` (absolute, model
    /// units).
    FaceCoplanar,
}

/// Which non-manifold conditions [`Selection::select_non_manifold`] catches
/// (Blender's `Select ▸ All by Trait ▸ Non Manifold` checkboxes). All default
/// on via [`NonManifoldKinds::all`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NonManifoldKinds {
    /// Wire edges — edges with **no** incident face.
    pub wire: bool,
    /// Boundary edges — edges with **one** incident face (an open border).
    pub boundary: bool,
    /// Edges shared by **three or more** faces.
    pub multiple_faces: bool,
}

impl NonManifoldKinds {
    /// Every condition enabled.
    pub fn all() -> Self {
        NonManifoldKinds { wire: true, boundary: true, multiple_faces: true }
    }
}

impl Default for NonManifoldKinds {
    fn default() -> Self {
        Self::all()
    }
}

/// How [`Selection::select_faces_by_sides`] compares a face's side count to the
/// target (Blender's `Select ▸ All by Trait ▸ Faces by Sides` "Type" dropdown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberCompare {
    /// Side count `== n`.
    Equal,
    /// Side count `!= n`.
    NotEqual,
    /// Side count `< n`.
    Less,
    /// Side count `> n`.
    Greater,
}

impl NumberCompare {
    fn test(self, value: usize, target: usize) -> bool {
        match self {
            NumberCompare::Equal => value == target,
            NumberCompare::NotEqual => value != target,
            NumberCompare::Less => value < target,
            NumberCompare::Greater => value > target,
        }
    }
}

/// The set of currently-selected mesh elements plus the active [`SelectMode`].
///
/// Three sorted sets (vertices, edges, faces) are kept at all times; the active
/// mode is a *view* onto them, not a restriction — the region operators write
/// into the set matching the current mode, and [`Selection::set_mode`] rewrites
/// the sets so they stay mutually consistent (Blender's selection flush).
///
/// A `Selection` holds only indices, so it is `Clone` and carries no borrow of
/// the mesh it describes; pair it with the `&Mesh` explicitly at each call.
/// Indices are **not** validated against a mesh on construction — an operator
/// that mutates topology invalidates a `Selection` exactly as it would a raw
/// index, and should rebuild or remap it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    mode: SelectMode,
    verts: BTreeSet<VertexId>,
    edges: BTreeSet<EdgeId>,
    faces: BTreeSet<FaceId>,
}

impl Selection {
    /// An empty selection in `mode`.
    pub fn new(mode: SelectMode) -> Self {
        Selection {
            mode,
            verts: BTreeSet::new(),
            edges: BTreeSet::new(),
            faces: BTreeSet::new(),
        }
    }

    /// Everything in `mesh` selected, in `mode` (and every domain flushed to
    /// match — a fully-selected mesh is fully selected in all three sets).
    pub fn all(mesh: &Mesh, mode: SelectMode) -> Self {
        let mut s = Selection::new(mode);
        s.select_all(mesh);
        s
    }

    /// The active select mode.
    pub fn mode(&self) -> SelectMode {
        self.mode
    }

    /// Currently-selected vertices, ascending by id.
    pub fn selected_vertices(&self) -> impl Iterator<Item = VertexId> + '_ {
        self.verts.iter().copied()
    }

    /// Currently-selected edges, ascending by id.
    pub fn selected_edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.edges.iter().copied()
    }

    /// Currently-selected faces, ascending by id.
    pub fn selected_faces(&self) -> impl Iterator<Item = FaceId> + '_ {
        self.faces.iter().copied()
    }

    /// Number of selected vertices.
    pub fn vertex_count(&self) -> usize {
        self.verts.len()
    }

    /// Number of selected edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Number of selected faces.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// `true` when no vertex, edge, or face is selected.
    pub fn is_empty(&self) -> bool {
        self.verts.is_empty() && self.edges.is_empty() && self.faces.is_empty()
    }

    /// Whether a specific element is selected (checks the set for its domain,
    /// independent of the active mode).
    pub fn is_selected(&self, e: Element) -> bool {
        match e {
            Element::Vertex(v) => self.verts.contains(&v),
            Element::Edge(ed) => self.edges.contains(&ed),
            Element::Face(f) => self.faces.contains(&f),
        }
    }

    /// Select one element into the active mode's domain.
    ///
    /// The element's domain is normally the same as [`Selection::mode`]. A
    /// **coarser** element still works — selecting a face in vertex mode selects
    /// its corners, selecting a face in edge mode selects its boundary edges. A
    /// **finer** element than the mode (a vertex in face mode) is ignored,
    /// matching Blender, where that pick is not possible. The two derived
    /// domains are refreshed afterwards (Blender's *selection flush*).
    pub fn select(&mut self, mesh: &Mesh, e: Element) {
        self.set_primary(mesh, e, true);
    }

    /// Deselect one element from the active mode's domain. Element-domain
    /// mismatch is handled exactly as in [`Selection::select`].
    pub fn deselect(&mut self, mesh: &Mesh, e: Element) {
        self.set_primary(mesh, e, false);
    }

    /// Flip the selected state of one element.
    pub fn toggle(&mut self, mesh: &Mesh, e: Element) {
        if self.is_selected(e) {
            self.deselect(mesh, e);
        } else {
            self.select(mesh, e);
        }
    }

    /// Add (`on`) or remove (`!on`) an element from the primary domain, mapping
    /// a coarser element down to the primary domain, then re-sync.
    fn set_primary(&mut self, mesh: &Mesh, e: Element, on: bool) {
        match self.mode {
            SelectMode::Vertex => {
                let vs: Vec<VertexId> = match e {
                    Element::Vertex(v) => vec![v],
                    Element::Edge(ed) => mesh.edge(ed).map(|x| x.verts.to_vec()).unwrap_or_default(),
                    Element::Face(f) => mesh.face_vertices(f),
                };
                for v in vs {
                    if on {
                        self.verts.insert(v);
                    } else {
                        self.verts.remove(&v);
                    }
                }
            }
            SelectMode::Edge => {
                let es: Vec<EdgeId> = match e {
                    Element::Edge(ed) => vec![ed],
                    Element::Face(f) => face_edges(mesh, f),
                    Element::Vertex(_) => Vec::new(),
                };
                for ed in es {
                    if on {
                        self.edges.insert(ed);
                    } else {
                        self.edges.remove(&ed);
                    }
                }
            }
            SelectMode::Face => {
                if let Element::Face(f) = e {
                    if on {
                        self.faces.insert(f);
                    } else {
                        self.faces.remove(&f);
                    }
                }
            }
        }
        self.sync(mesh);
    }

    /// Select every element of `mesh` in the active mode (the other two domains
    /// follow by flush).
    pub fn select_all(&mut self, mesh: &Mesh) {
        match self.mode {
            SelectMode::Vertex => self.verts = (0..mesh.vertex_count()).map(VertexId).collect(),
            SelectMode::Edge => self.edges = (0..mesh.edge_count()).map(EdgeId).collect(),
            SelectMode::Face => self.faces = (0..mesh.face_count()).map(FaceId).collect(),
        }
        self.sync(mesh);
    }

    /// Deselect everything (all three domains).
    pub fn deselect_all(&mut self) {
        self.verts.clear();
        self.edges.clear();
        self.faces.clear();
    }

    /// Invert the selection **in the active mode**: every element of that
    /// domain flips selected ↔ unselected, then the other two domains are
    /// re-derived. Matches Blender's mode-sensitive `Select ▸ Invert`.
    pub fn invert(&mut self, mesh: &Mesh) {
        match self.mode {
            SelectMode::Vertex => {
                let all: BTreeSet<VertexId> = (0..mesh.vertex_count()).map(VertexId).collect();
                self.verts = all.difference(&self.verts).copied().collect();
            }
            SelectMode::Edge => {
                let all: BTreeSet<EdgeId> = (0..mesh.edge_count()).map(EdgeId).collect();
                self.edges = all.difference(&self.edges).copied().collect();
            }
            SelectMode::Face => {
                let all: BTreeSet<FaceId> = (0..mesh.face_count()).map(FaceId).collect();
                self.faces = all.difference(&self.faces).copied().collect();
            }
        }
        self.sync(mesh);
    }

    /// Change the active mode, rewriting the selection so the domains stay
    /// consistent — Blender's *selection flush*:
    ///
    /// - **→ [`SelectMode::Vertex`]**: keep every vertex the current selection
    ///   implies.
    /// - **→ [`SelectMode::Edge`]**: an edge is selected iff *both* endpoints
    ///   are currently selected.
    /// - **→ [`SelectMode::Face`]**: a face is selected iff *all* its corners
    ///   are currently selected.
    ///
    /// Switching to a finer mode never loses elements; switching to a coarser
    /// mode keeps only the fully-selected ones — exactly Blender's behaviour
    /// when you box-select some verts then press `3`.
    pub fn set_mode(&mut self, mesh: &Mesh, mode: SelectMode) {
        if self.mode == mode {
            return;
        }
        // `sync` keeps `self.verts` as the effective vertex set in every mode,
        // so it is the right thing to re-derive the new primary domain from.
        let verts = self.verts.clone();
        self.mode = mode;
        match mode {
            SelectMode::Vertex => self.verts = verts,
            SelectMode::Edge => {
                self.edges = (0..mesh.edge_count())
                    .map(EdgeId)
                    .filter(|&e| {
                        mesh.edge(e).is_some_and(|ed| {
                            verts.contains(&ed.verts[0]) && verts.contains(&ed.verts[1])
                        })
                    })
                    .collect();
            }
            SelectMode::Face => {
                self.faces = (0..mesh.face_count())
                    .map(FaceId)
                    .filter(|&f| {
                        let vs = mesh.face_vertices(f);
                        !vs.is_empty() && vs.iter().all(|v| verts.contains(v))
                    })
                    .collect();
            }
        }
        self.sync(mesh);
    }

    /// Union another selection's active-domain set into this one, then re-sync.
    /// Backs Blender's *extend* box-select. `other`'s active mode is ignored;
    /// its set matching **this** selection's mode is the one merged.
    pub fn union(&mut self, mesh: &Mesh, other: &Selection) {
        match self.mode {
            SelectMode::Vertex => self.verts.extend(other.verts.iter().copied()),
            SelectMode::Edge => self.edges.extend(other.edges.iter().copied()),
            SelectMode::Face => self.faces.extend(other.faces.iter().copied()),
        }
        self.sync(mesh);
    }

    /// Remove `other`'s active-domain elements from this selection, then
    /// re-sync. Backs Blender's *subtract* box-select.
    pub fn subtract(&mut self, mesh: &Mesh, other: &Selection) {
        match self.mode {
            SelectMode::Vertex => self.verts.retain(|v| !other.verts.contains(v)),
            SelectMode::Edge => self.edges.retain(|e| !other.edges.contains(e)),
            SelectMode::Face => self.faces.retain(|f| !other.faces.contains(f)),
        }
        self.sync(mesh);
    }

    /// Keep only elements also present in `other`'s active-domain set, then
    /// re-sync. Backs Blender's *intersect* box-select.
    pub fn intersect(&mut self, mesh: &Mesh, other: &Selection) {
        match self.mode {
            SelectMode::Vertex => self.verts.retain(|v| other.verts.contains(v)),
            SelectMode::Edge => self.edges.retain(|e| other.edges.contains(e)),
            SelectMode::Face => self.faces.retain(|f| other.faces.contains(f)),
        }
        self.sync(mesh);
    }

    /// Keep only active-mode elements for which `keep` returns `true`, then
    /// re-sync the derived domains. The predicate is called once per selected
    /// element in the active domain.
    pub fn retain(&mut self, mesh: &Mesh, mut keep: impl FnMut(Element) -> bool) {
        match self.mode {
            SelectMode::Vertex => self.verts.retain(|&v| keep(Element::Vertex(v))),
            SelectMode::Edge => self.edges.retain(|&e| keep(Element::Edge(e))),
            SelectMode::Face => self.faces.retain(|&f| keep(Element::Face(f))),
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Region select
    // ---------------------------------------------------------------------

    /// Add to the selection every element of `mesh` inside the axis-aligned box
    /// `[min, max]` (model space). `region` decides whether an edge/face needs
    /// all or any vertex inside (ignored in vertex mode).
    ///
    /// Extends the current selection; call [`Selection::deselect_all`] first
    /// for a replace, or select into a fresh [`Selection`] and use
    /// [`Selection::subtract`] / [`Selection::union`] for the other modes.
    pub fn select_in_box(&mut self, mesh: &Mesh, min: Vec3, max: Vec3, region: RegionMode) {
        let (lo, hi) = (
            Vec3::new(min.x.min(max.x), min.y.min(max.y), min.z.min(max.z)),
            Vec3::new(min.x.max(max.x), min.y.max(max.y), min.z.max(max.z)),
        );
        self.select_by_vertex_predicate(mesh, region, |p| {
            p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && p.z >= lo.z && p.z <= hi.z
        });
    }

    /// Add to the selection every element of `mesh` within `radius` of
    /// `center` (model space). `region` as for [`Selection::select_in_box`].
    pub fn select_in_sphere(&mut self, mesh: &Mesh, center: Vec3, radius: f64, region: RegionMode) {
        let r2 = radius * radius;
        self.select_by_vertex_predicate(mesh, region, |p| {
            let d = p.sub(center);
            d.dot(d) <= r2
        });
    }

    /// Add to the selection every element of `mesh` whose projected position
    /// falls inside the screen-space polygon `polygon` (a closed ring of
    /// `[x, y]` points; the last point is joined back to the first).
    ///
    /// This is the headless form of Blender's **box**, **circle** and **lasso**
    /// select — box is a 4-point rectangle, circle a regular polygon, lasso the
    /// freehand ring. `project` maps a model-space point to the same 2-D space
    /// as `polygon` (e.g. multiply by a view-projection matrix and take
    /// `x, y`); supplying it here keeps this module free of any camera type.
    /// Points that project behind the camera should be handled by the caller's
    /// `project` (e.g. by returning a coordinate far outside the polygon).
    pub fn select_in_screen_polygon(
        &mut self,
        mesh: &Mesh,
        project: impl Fn(Vec3) -> [f64; 2],
        polygon: &[[f64; 2]],
        region: RegionMode,
    ) {
        if polygon.len() < 3 {
            return;
        }
        self.select_by_vertex_predicate(mesh, region, |p| {
            point_in_polygon_2d(project(p), polygon)
        });
    }

    /// Shared core of the region operators: mark the vertices for which
    /// `inside` holds, add the active-mode elements they imply (per `region`
    /// for edges/faces), then re-sync the derived domains.
    fn select_by_vertex_predicate(
        &mut self,
        mesh: &Mesh,
        region: RegionMode,
        inside: impl Fn(Vec3) -> bool,
    ) {
        let vin: Vec<bool> = (0..mesh.vertex_count())
            .map(|i| mesh.vertex(VertexId(i)).map(|v| inside(v.position)).unwrap_or(false))
            .collect();
        let vhit = |v: VertexId| vin.get(v.0).copied().unwrap_or(false);

        match self.mode {
            SelectMode::Vertex => {
                for (i, &hit) in vin.iter().enumerate() {
                    if hit {
                        self.verts.insert(VertexId(i));
                    }
                }
            }
            SelectMode::Edge => {
                for e in 0..mesh.edge_count() {
                    let Some(edge) = mesh.edge(EdgeId(e)) else { continue };
                    let hit = match region {
                        RegionMode::Touching => vhit(edge.verts[0]) || vhit(edge.verts[1]),
                        RegionMode::Enclosed => vhit(edge.verts[0]) && vhit(edge.verts[1]),
                    };
                    if hit {
                        self.edges.insert(EdgeId(e));
                    }
                }
            }
            SelectMode::Face => {
                for f in 0..mesh.face_count() {
                    let vs = mesh.face_vertices(FaceId(f));
                    if vs.is_empty() {
                        continue;
                    }
                    let hit = match region {
                        RegionMode::Touching => vs.iter().any(|&v| vhit(v)),
                        RegionMode::Enclosed => vs.iter().all(|&v| vhit(v)),
                    };
                    if hit {
                        self.faces.insert(FaceId(f));
                    }
                }
            }
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Select linked
    // ---------------------------------------------------------------------

    /// Grow the selection so that every connected component containing at least
    /// one already-selected element is fully selected in the active mode
    /// (Blender's `Select ▸ Select Linked ▸ Linked`, `Ctrl+L`).
    ///
    /// Connectivity is by shared edge, unbounded. To stop at seams / sharp
    /// edges / material boundaries use [`Selection::select_linked_delimited`]
    /// with an edge set from
    /// [`crate::attributes::MeshAttributes::linked_delimiters`].
    pub fn select_linked(&mut self, mesh: &Mesh) {
        let component = flood_components(mesh, self.verts.iter().copied(), None);
        self.set_component(mesh, &component, true);
    }

    /// Like [`Selection::select_linked`] but the flood is over **face**
    /// adjacency and does not cross any edge in `delimiters` (Blender's `Ctrl+L`
    /// with a seam / sharp / material delimiter set). The seeded faces are
    /// whichever faces the current selection touches; the result is every face
    /// reachable from them without crossing a delimiter, flushed to the active
    /// mode.
    pub fn select_linked_delimited(
        &mut self,
        mesh: &Mesh,
        delimiters: &BTreeSet<EdgeId>,
    ) {
        let topo = MeshTopology::new(mesh);
        let blocked: BTreeSet<(usize, usize)> = delimiters
            .iter()
            .filter_map(|&e| mesh.edge(e))
            .map(|ed| (ed.verts[0].0.min(ed.verts[1].0), ed.verts[0].0.max(ed.verts[1].0)))
            .collect();

        // Seed faces: any face all of whose verts are currently selected, or
        // any face touching a selected vertex if none are fully selected.
        let sel_v = self.verts.clone();
        let mut seeds: Vec<usize> = (0..mesh.face_count())
            .filter(|&f| {
                let vs = mesh.face_vertices(FaceId(f));
                !vs.is_empty() && vs.iter().all(|v| sel_v.contains(v))
            })
            .collect();
        if seeds.is_empty() {
            seeds = (0..mesh.face_count())
                .filter(|&f| mesh.face_vertices(FaceId(f)).iter().any(|v| sel_v.contains(v)))
                .collect();
        }

        let mut seen: BTreeSet<usize> = seeds.iter().copied().collect();
        let mut stack = seeds;
        while let Some(f) = stack.pop() {
            for e in topo.face_edges(mesh, FaceId(f)) {
                let ed = mesh.edge(e).unwrap();
                if blocked.contains(&(ed.verts[0].0.min(ed.verts[1].0), ed.verts[0].0.max(ed.verts[1].0))) {
                    continue;
                }
                for &g in topo.edge_faces(e) {
                    if seen.insert(g.0) {
                        stack.push(g.0);
                    }
                }
            }
        }

        let component: BTreeSet<VertexId> =
            seen.iter().flat_map(|&f| mesh.face_vertices(FaceId(f))).collect();
        self.set_component(mesh, &component, true);
    }

    /// Select exactly the one connected component that contains `seed`
    /// (Blender's hover-`L`), adding it to any prior selection.
    pub fn select_linked_from(&mut self, mesh: &Mesh, seed: Element) {
        let seed_verts: Vec<VertexId> = match seed {
            Element::Vertex(v) => vec![v],
            Element::Edge(e) => mesh.edge(e).map(|ed| ed.verts.to_vec()).unwrap_or_default(),
            Element::Face(f) => mesh.face_vertices(f),
        };
        let component = flood_components(mesh, seed_verts.into_iter(), None);
        self.set_component(mesh, &component, true);
    }

    /// Set the active-mode domain to every element fully inside `component`
    /// (a vertex set); `extend` keeps whatever was already selected.
    fn set_component(&mut self, mesh: &Mesh, component: &BTreeSet<VertexId>, extend: bool) {
        match self.mode {
            SelectMode::Vertex => {
                if !extend {
                    self.verts.clear();
                }
                self.verts.extend(component.iter().copied());
            }
            SelectMode::Edge => {
                if !extend {
                    self.edges.clear();
                }
                for e in 0..mesh.edge_count() {
                    if let Some(ed) = mesh.edge(EdgeId(e)) {
                        if component.contains(&ed.verts[0]) && component.contains(&ed.verts[1]) {
                            self.edges.insert(EdgeId(e));
                        }
                    }
                }
            }
            SelectMode::Face => {
                if !extend {
                    self.faces.clear();
                }
                for f in 0..mesh.face_count() {
                    let vs = mesh.face_vertices(FaceId(f));
                    if !vs.is_empty() && vs.iter().all(|v| component.contains(v)) {
                        self.faces.insert(FaceId(f));
                    }
                }
            }
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Loop / ring selection (op-hzs.54.2 — GH issue #37 §A)
    // ---------------------------------------------------------------------

    /// Select the **edge loop** through `seed` — Blender's `Alt`-click. In edge
    /// mode the loop edges are added; in vertex mode their vertices; in face
    /// mode the [`Selection::select_face_loop`] strip (the natural analogue).
    pub fn select_edge_loop(&mut self, mesh: &Mesh, seed: EdgeId) {
        let topo = MeshTopology::new(mesh);
        if self.mode == SelectMode::Face {
            let faces = topology::face_loop(&topo, mesh, seed);
            self.add_faces(mesh, faces);
        } else {
            let edges = topology::edge_loop(&topo, mesh, seed);
            self.add_edges(mesh, edges);
        }
    }

    /// Select the **edge ring** through `seed` — Blender's `Ctrl+Alt`-click.
    /// Mode handling as for [`Selection::select_edge_loop`].
    pub fn select_edge_ring(&mut self, mesh: &Mesh, seed: EdgeId) {
        let topo = MeshTopology::new(mesh);
        if self.mode == SelectMode::Face {
            let faces = topology::face_loop(&topo, mesh, seed);
            self.add_faces(mesh, faces);
        } else {
            let edges = topology::edge_ring(&topo, mesh, seed);
            self.add_edges(mesh, edges);
        }
    }

    /// Select the **face loop** perpendicular to `seed` — the strip of quads
    /// the ring walk crosses (Blender's `Alt`-click in face mode). In vertex /
    /// edge mode the strip's vertices / boundary edges are added instead.
    pub fn select_face_loop(&mut self, mesh: &Mesh, seed: EdgeId) {
        let topo = MeshTopology::new(mesh);
        let faces = topology::face_loop(&topo, mesh, seed);
        self.add_faces(mesh, faces);
    }

    /// Select the **boundary loop** that contains `seed` — the ring of open
    /// (one-face) edges around a hole or the outer border. A no-op if `seed`
    /// is not a boundary edge.
    pub fn select_boundary_loop(&mut self, mesh: &Mesh, seed: EdgeId) {
        let topo = MeshTopology::new(mesh);
        let edges = topology::boundary_loop(&topo, mesh, seed);
        self.add_edges(mesh, edges);
    }

    /// Select the **shortest path** between two elements (Blender's
    /// `Ctrl`-click). In vertex mode the path is the geometry-shortest vertex
    /// chain (Dijkstra on edge lengths); in edge / face mode it is the
    /// fewest-hops chain over edge / face adjacency. `from` and `to` should
    /// match the active mode; a mismatch maps each to a representative vertex
    /// and falls back to the vertex path.
    pub fn select_shortest_path(&mut self, mesh: &Mesh, from: Element, to: Element) {
        let topo = MeshTopology::new(mesh);
        match (self.mode, from, to) {
            (SelectMode::Edge, Element::Edge(a), Element::Edge(b)) => {
                let path = topology::shortest_hop_path(a, b, |e| {
                    edge_neighbours(&topo, mesh, e)
                });
                self.add_edges(mesh, path);
            }
            (SelectMode::Face, Element::Face(a), Element::Face(b)) => {
                let path = topology::shortest_hop_path(a, b, |f| {
                    face_neighbours(&topo, mesh, f)
                });
                self.add_faces(mesh, path);
            }
            _ => {
                let va = element_vertex(mesh, from);
                let vb = element_vertex(mesh, to);
                if let (Some(a), Some(b)) = (va, vb) {
                    let path = topology::shortest_vertex_path(&topo, mesh, a, b);
                    for v in path {
                        self.verts.insert(v);
                    }
                    if self.mode != SelectMode::Vertex {
                        // Nothing coarser to infer from a 1-wide path; leave the
                        // verts and let `sync` derive whatever it implies.
                    }
                    self.sync(mesh);
                }
            }
        }
    }

    /// Add a set of edges in the active mode: edge mode inserts them; vertex
    /// mode inserts their endpoints; face mode inserts faces all of whose
    /// edges are in the set (the quad strip of a ring).
    fn add_edges(&mut self, mesh: &Mesh, edges: impl IntoIterator<Item = EdgeId>) {
        let set: BTreeSet<EdgeId> = edges.into_iter().collect();
        match self.mode {
            SelectMode::Edge => self.edges.extend(set.iter().copied()),
            SelectMode::Vertex => {
                for &e in &set {
                    if let Some(ed) = mesh.edge(e) {
                        self.verts.insert(ed.verts[0]);
                        self.verts.insert(ed.verts[1]);
                    }
                }
            }
            SelectMode::Face => {
                let topo = MeshTopology::new(mesh);
                for f in 0..mesh.face_count() {
                    let fe = topo.face_edges(mesh, FaceId(f));
                    if !fe.is_empty() && fe.iter().all(|e| set.contains(e)) {
                        self.faces.insert(FaceId(f));
                    }
                }
            }
        }
        self.sync(mesh);
    }

    /// Add a set of faces in the active mode: face mode inserts them; vertex
    /// mode inserts their corners; edge mode inserts every edge both of whose
    /// endpoints lie on a face in the set.
    fn add_faces(&mut self, mesh: &Mesh, faces: impl IntoIterator<Item = FaceId>) {
        let set: BTreeSet<FaceId> = faces.into_iter().collect();
        match self.mode {
            SelectMode::Face => self.faces.extend(set.iter().copied()),
            SelectMode::Vertex => {
                for &f in &set {
                    for v in mesh.face_vertices(f) {
                        self.verts.insert(v);
                    }
                }
            }
            SelectMode::Edge => {
                let verts: BTreeSet<VertexId> =
                    set.iter().flat_map(|&f| mesh.face_vertices(f)).collect();
                for e in 0..mesh.edge_count() {
                    if let Some(ed) = mesh.edge(EdgeId(e)) {
                        if verts.contains(&ed.verts[0]) && verts.contains(&ed.verts[1]) {
                            self.edges.insert(EdgeId(e));
                        }
                    }
                }
            }
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Grow / shrink / similar / checker (op-hzs.54.3 — GH issue #37 §A)
    // ---------------------------------------------------------------------

    /// Grow the selection by one ring — add every element of the active domain
    /// adjacent to an already-selected one (Blender's `Select ▸ More`,
    /// `Ctrl` `NumpadPlus`). Adjacency: vertices by shared edge, edges by
    /// shared vertex, faces by shared edge.
    pub fn select_more(&mut self, mesh: &Mesh) {
        let topo = MeshTopology::new(mesh);
        match self.mode {
            SelectMode::Vertex => {
                let mut add: Vec<VertexId> = Vec::new();
                for &v in &self.verts {
                    for &e in topo.vertex_edges(v) {
                        if let Some(n) = topo.other_end(mesh, e, v) {
                            add.push(n);
                        }
                    }
                }
                self.verts.extend(add);
            }
            SelectMode::Edge => {
                let mut add: Vec<EdgeId> = Vec::new();
                for &e in &self.edges {
                    let Some(ed) = mesh.edge(e) else { continue };
                    for &v in &ed.verts {
                        add.extend(topo.vertex_edges(v).iter().copied());
                    }
                }
                self.edges.extend(add);
            }
            SelectMode::Face => {
                let mut add: Vec<FaceId> = Vec::new();
                for &f in &self.faces {
                    for e in topo.face_edges(mesh, f) {
                        add.extend(topo.edge_faces(e).iter().copied());
                    }
                }
                self.faces.extend(add);
            }
        }
        self.sync(mesh);
    }

    /// Shrink the selection by one ring — remove every selected element of the
    /// active domain that touches an unselected one (Blender's `Select ▸ Less`,
    /// `Ctrl` `NumpadMinus`). The inverse boundary of [`Selection::select_more`].
    pub fn select_less(&mut self, mesh: &Mesh) {
        let topo = MeshTopology::new(mesh);
        match self.mode {
            SelectMode::Vertex => {
                let sel = self.verts.clone();
                self.verts.retain(|&v| {
                    topo.vertex_edges(v)
                        .iter()
                        .filter_map(|&e| topo.other_end(mesh, e, v))
                        .all(|n| sel.contains(&n))
                });
            }
            SelectMode::Edge => {
                let sel = self.edges.clone();
                self.edges.retain(|&e| {
                    let Some(ed) = mesh.edge(e) else { return false };
                    ed.verts
                        .iter()
                        .all(|&v| topo.vertex_edges(v).iter().all(|n| sel.contains(n)))
                });
            }
            SelectMode::Face => {
                let sel = self.faces.clone();
                self.faces.retain(|&f| {
                    topo.face_edges(mesh, f)
                        .iter()
                        .all(|&e| topo.edge_faces(e).iter().all(|n| sel.contains(n)))
                });
            }
        }
        self.sync(mesh);
    }

    /// Select every element of the active domain whose `trait_` value matches
    /// that of **some** currently-selected element, within `threshold`
    /// (Blender's `Select ▸ Select Similar`, `Shift+G`). See [`SimilarTrait`]
    /// for what `threshold` means per trait. A no-op if `trait_`'s domain does
    /// not match the active mode, or nothing is selected.
    pub fn select_similar(&mut self, mesh: &Mesh, trait_: SimilarTrait, threshold: f64) {
        let topo = MeshTopology::new(mesh);
        match trait_ {
            SimilarTrait::VertexValence => {
                if self.mode != SelectMode::Vertex {
                    return;
                }
                let refs: BTreeSet<usize> =
                    self.verts.iter().map(|&v| topo.vertex_edges(v).len()).collect();
                for v in 0..mesh.vertex_count() {
                    if refs.contains(&topo.vertex_edges(VertexId(v)).len()) {
                        self.verts.insert(VertexId(v));
                    }
                }
            }
            SimilarTrait::EdgeLength
            | SimilarTrait::EdgeDirection
            | SimilarTrait::EdgeFaceCount => {
                if self.mode != SelectMode::Edge {
                    return;
                }
                let refs: Vec<EdgeTrait> =
                    self.edges.iter().map(|&e| edge_trait(mesh, &topo, e, trait_)).collect();
                for e in 0..mesh.edge_count() {
                    let cand = edge_trait(mesh, &topo, EdgeId(e), trait_);
                    if refs.iter().any(|r| cand.matches(r, threshold)) {
                        self.edges.insert(EdgeId(e));
                    }
                }
            }
            SimilarTrait::FaceArea
            | SimilarTrait::FaceSides
            | SimilarTrait::FacePerimeter
            | SimilarTrait::FaceNormal
            | SimilarTrait::FaceCoplanar => {
                if self.mode != SelectMode::Face {
                    return;
                }
                let refs: Vec<FaceTrait> =
                    self.faces.iter().map(|&f| face_trait(mesh, f, trait_)).collect();
                for f in 0..mesh.face_count() {
                    let cand = face_trait(mesh, FaceId(f), trait_);
                    if refs.iter().any(|r| cand.matches(r, threshold)) {
                        self.faces.insert(FaceId(f));
                    }
                }
            }
        }
        self.sync(mesh);
    }

    /// Thin out the selection to a regular pattern: over the selected elements
    /// of the active domain (ascending by id), keep `selected` in a row, then
    /// deselect `deselected` in a row, repeating; `offset` shifts where the
    /// pattern starts (Blender's `Select ▸ Checker Deselect` / Select Nth).
    ///
    /// `selected` and `deselected` are clamped to at least 1 and 0
    /// respectively; with `deselected == 0` this is a no-op.
    pub fn checker_deselect(
        &mut self,
        mesh: &Mesh,
        selected: usize,
        deselected: usize,
        offset: usize,
    ) {
        let selected = selected.max(1);
        if deselected == 0 {
            return;
        }
        let period = selected + deselected;
        let ids: Vec<usize> = match self.mode {
            SelectMode::Vertex => self.verts.iter().map(|v| v.0).collect(),
            SelectMode::Edge => self.edges.iter().map(|e| e.0).collect(),
            SelectMode::Face => self.faces.iter().map(|f| f.0).collect(),
        };
        for (i, id) in ids.into_iter().enumerate() {
            let phase = (i + offset) % period;
            if phase >= selected {
                match self.mode {
                    SelectMode::Vertex => {
                        self.verts.remove(&VertexId(id));
                    }
                    SelectMode::Edge => {
                        self.edges.remove(&EdgeId(id));
                    }
                    SelectMode::Face => {
                        self.faces.remove(&FaceId(id));
                    }
                }
            }
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Select by trait (op-hzs.54.4 — GH issue #37 §A)
    // ---------------------------------------------------------------------

    /// Select the **non-manifold** geometry of `mesh` per `kinds` (Blender's
    /// `Select ▸ All by Trait ▸ Non Manifold`).
    ///
    /// In edge mode the offending edges are selected. In vertex mode a vertex
    /// is selected if it touches an offending edge, **or** has an odd number of
    /// incident boundary edges (a fan pinch / bow-tie). Face mode is a no-op.
    ///
    /// The "non-contiguous" case (neighbouring faces wound inconsistently
    /// across a shared edge) needs the winding analysis in
    /// [`crate::recalc_normals`] and is not covered here.
    pub fn select_non_manifold(&mut self, mesh: &Mesh, kinds: NonManifoldKinds) {
        let topo = MeshTopology::new(mesh);
        let is_bad = |e: EdgeId| {
            let n = topo.edge_faces(e).len();
            (kinds.wire && n == 0) || (kinds.boundary && n == 1) || (kinds.multiple_faces && n >= 3)
        };
        match self.mode {
            SelectMode::Edge => {
                for e in 0..mesh.edge_count() {
                    if is_bad(EdgeId(e)) {
                        self.edges.insert(EdgeId(e));
                    }
                }
            }
            SelectMode::Vertex => {
                for v in 0..mesh.vertex_count() {
                    let at = topo.vertex_edges(VertexId(v));
                    let touches_bad = at.iter().any(|&e| is_bad(e));
                    let boundary_count =
                        at.iter().filter(|&&e| topo.edge_faces(e).len() == 1).count();
                    if touches_bad || boundary_count % 2 == 1 {
                        self.verts.insert(VertexId(v));
                    }
                }
            }
            SelectMode::Face => {}
        }
        self.sync(mesh);
    }

    /// Select **loose geometry** — elements not connected to any face (Blender's
    /// `Select ▸ All by Trait ▸ Loose Geometry`). Vertex mode: vertices with no
    /// incident face. Edge mode: edges with no incident face (including wire
    /// edges). Face mode is a no-op.
    pub fn select_loose(&mut self, mesh: &Mesh) {
        let topo = MeshTopology::new(mesh);
        match self.mode {
            SelectMode::Vertex => {
                for v in 0..mesh.vertex_count() {
                    if topo.vertex_faces(VertexId(v)).is_empty() {
                        self.verts.insert(VertexId(v));
                    }
                }
            }
            SelectMode::Edge => {
                for e in 0..mesh.edge_count() {
                    if topo.edge_faces(EdgeId(e)).is_empty() {
                        self.edges.insert(EdgeId(e));
                    }
                }
            }
            SelectMode::Face => {}
        }
        self.sync(mesh);
    }

    /// Select **interior faces** — faces every edge of which is shared by three
    /// or more faces (Blender's `Select ▸ All by Trait ▸ Interior Faces`, the
    /// buried faces inside a non-manifold solid). Face mode only.
    pub fn select_interior_faces(&mut self, mesh: &Mesh) {
        if self.mode != SelectMode::Face {
            return;
        }
        let topo = MeshTopology::new(mesh);
        for f in 0..mesh.face_count() {
            let fe = topo.face_edges(mesh, FaceId(f));
            if !fe.is_empty() && fe.iter().all(|&e| topo.edge_faces(e).len() > 2) {
                self.faces.insert(FaceId(f));
            }
        }
        self.sync(mesh);
    }

    /// Select faces whose side count compares to `sides` as `cmp` says
    /// (Blender's `Select ▸ All by Trait ▸ Faces by Sides` — e.g.
    /// `NumberCompare::Greater` with `sides == 4` selects every n-gon). Face
    /// mode only.
    pub fn select_faces_by_sides(&mut self, mesh: &Mesh, sides: usize, cmp: NumberCompare) {
        if self.mode != SelectMode::Face {
            return;
        }
        for f in 0..mesh.face_count() {
            if cmp.test(mesh.face_vertices(FaceId(f)).len(), sides) {
                self.faces.insert(FaceId(f));
            }
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Select mirror
    // ---------------------------------------------------------------------

    /// For each currently-selected element, also select the element that is its
    /// mirror image across the `axis` plane through the origin, matched by
    /// position within `merge_dist` (Blender's `Select ▸ Select Mirror`).
    ///
    /// With `extend == false` the result is *only* the mirror images (the
    /// original selection is replaced); with `extend == true` the mirror images
    /// are added to it. An element with no mirror partner within tolerance is
    /// simply skipped.
    pub fn select_mirror(&mut self, mesh: &Mesh, axis: Axis, merge_dist: f64, extend: bool) {
        let lookup = PositionLookup::new(mesh, merge_dist.max(1e-9));
        let mirror_vert = |v: VertexId| {
            mesh.vertex(v).and_then(|vx| lookup.find(axis.reflect(vx.position)))
        };

        let mut m_verts: BTreeSet<VertexId> = BTreeSet::new();
        let mut m_edges: BTreeSet<EdgeId> = BTreeSet::new();
        let mut m_faces: BTreeSet<FaceId> = BTreeSet::new();

        match self.mode {
            SelectMode::Vertex => {
                for &v in &self.verts {
                    if let Some(mv) = mirror_vert(v) {
                        m_verts.insert(mv);
                    }
                }
            }
            SelectMode::Edge => {
                for &e in &self.edges {
                    let Some(edge) = mesh.edge(e) else { continue };
                    if let (Some(a), Some(b)) =
                        (mirror_vert(edge.verts[0]), mirror_vert(edge.verts[1]))
                    {
                        if let Some(me) = find_edge(mesh, a, b) {
                            m_edges.insert(me);
                        }
                    }
                }
            }
            SelectMode::Face => {
                for &f in &self.faces {
                    let want: Option<BTreeSet<VertexId>> =
                        mesh.face_vertices(f).iter().map(|&v| mirror_vert(v)).collect();
                    let Some(want) = want else { continue };
                    for cand in 0..mesh.face_count() {
                        let cvs: BTreeSet<VertexId> =
                            mesh.face_vertices(FaceId(cand)).into_iter().collect();
                        if cvs == want {
                            m_faces.insert(FaceId(cand));
                            break;
                        }
                    }
                }
            }
        }

        match self.mode {
            SelectMode::Vertex => {
                if !extend {
                    self.verts.clear();
                }
                self.verts.extend(m_verts);
            }
            SelectMode::Edge => {
                if !extend {
                    self.edges.clear();
                }
                self.edges.extend(m_edges);
            }
            SelectMode::Face => {
                if !extend {
                    self.faces.clear();
                }
                self.faces.extend(m_faces);
            }
        }
        self.sync(mesh);
    }

    // ---------------------------------------------------------------------
    // Selection flush
    // ---------------------------------------------------------------------

    /// Re-derive the two non-active domains from the active one so the three
    /// sets stay mutually consistent (Blender's *selection flush*). Called
    /// after every mutation.
    ///
    /// - **Vertex active** — an edge is selected iff both endpoints are; a face
    ///   iff all its corners are.
    /// - **Edge active** — a vertex is selected iff it is an endpoint of a
    ///   selected edge; a face iff all its boundary edges are selected.
    /// - **Face active** — a vertex is selected iff it belongs to a selected
    ///   face; an edge iff both its endpoints do.
    fn sync(&mut self, mesh: &Mesh) {
        match self.mode {
            SelectMode::Vertex => {
                self.edges = (0..mesh.edge_count())
                    .map(EdgeId)
                    .filter(|&e| {
                        mesh.edge(e).is_some_and(|ed| {
                            self.verts.contains(&ed.verts[0]) && self.verts.contains(&ed.verts[1])
                        })
                    })
                    .collect();
                self.faces = (0..mesh.face_count())
                    .map(FaceId)
                    .filter(|&f| {
                        let vs = mesh.face_vertices(f);
                        !vs.is_empty() && vs.iter().all(|v| self.verts.contains(v))
                    })
                    .collect();
            }
            SelectMode::Edge => {
                self.verts = self
                    .edges
                    .iter()
                    .filter_map(|&e| mesh.edge(e))
                    .flat_map(|ed| ed.verts)
                    .collect();
                self.faces = (0..mesh.face_count())
                    .map(FaceId)
                    .filter(|&f| {
                        let fe = face_edges(mesh, f);
                        !fe.is_empty() && fe.iter().all(|e| self.edges.contains(e))
                    })
                    .collect();
            }
            SelectMode::Face => {
                self.verts = self.faces.iter().flat_map(|&f| mesh.face_vertices(f)).collect();
                self.edges = (0..mesh.edge_count())
                    .map(EdgeId)
                    .filter(|&e| {
                        mesh.edge(e).is_some_and(|ed| {
                            self.verts.contains(&ed.verts[0]) && self.verts.contains(&ed.verts[1])
                        })
                    })
                    .collect();
            }
        }
    }
}

/// A position → [`VertexId`] lookup with a distance tolerance, backed by a
/// rounded-coordinate grid hash. Used by [`Selection::select_mirror`] to find
/// the vertex at a mirrored position.
struct PositionLookup {
    cell: f64,
    grid: HashMap<[i64; 3], Vec<(Vec3, VertexId)>>,
}

impl PositionLookup {
    fn new(mesh: &Mesh, tol: f64) -> Self {
        let cell = tol.max(1e-9);
        let mut grid: HashMap<[i64; 3], Vec<(Vec3, VertexId)>> = HashMap::new();
        for i in 0..mesh.vertex_count() {
            if let Some(v) = mesh.vertex(VertexId(i)) {
                grid.entry(Self::key(v.position, cell)).or_default().push((v.position, VertexId(i)));
            }
        }
        PositionLookup { cell, grid }
    }

    fn key(p: Vec3, cell: f64) -> [i64; 3] {
        [
            (p.x / cell).round() as i64,
            (p.y / cell).round() as i64,
            (p.z / cell).round() as i64,
        ]
    }

    /// The id of a vertex within `tol` of `target`, or `None`. Searches the 27
    /// grid cells around `target`.
    fn find(&self, target: Vec3) -> Option<VertexId> {
        let k = Self::key(target, self.cell);
        let tol2 = self.cell * self.cell;
        let mut best: Option<(f64, VertexId)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cell = [k[0] + dx, k[1] + dy, k[2] + dz];
                    for &(p, id) in self.grid.get(&cell).map(|v| v.as_slice()).unwrap_or(&[]) {
                        let d = p.sub(target);
                        let d2 = d.dot(d);
                        if d2 <= tol2 && best.is_none_or(|(bd, _)| d2 < bd) {
                            best = Some((d2, id));
                        }
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }
}

// `vertex_adjacency` / `find_edge` / `face_edges` below are lightweight,
// un-cached counterparts of the queries on [`crate::topology::MeshTopology`],
// kept local for the per-`sync()` hot path (which runs after every mutation and
// would otherwise rebuild a whole `MeshTopology` each time). The loop/ring
// selection above, being an explicit user action, does build a `MeshTopology`
// and uses the shared implementation there.

/// The id of the undirected edge between `a` and `b`, or `None`.
fn find_edge(mesh: &Mesh, a: VertexId, b: VertexId) -> Option<EdgeId> {
    (0..mesh.edge_count()).map(EdgeId).find(|&e| {
        mesh.edge(e).is_some_and(|edge| {
            (edge.verts[0] == a && edge.verts[1] == b) || (edge.verts[0] == b && edge.verts[1] == a)
        })
    })
}

/// The edges of a face, in boundary order — one per consecutive vertex pair
/// around [`Mesh::face_vertices`]. An empty `Vec` if the face is out of range
/// or an expected edge is somehow missing.
fn face_edges(mesh: &Mesh, f: FaceId) -> Vec<EdgeId> {
    let vs = mesh.face_vertices(f);
    let n = vs.len();
    if n < 3 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        match find_edge(mesh, vs[i], vs[(i + 1) % n]) {
            Some(e) => out.push(e),
            None => return Vec::new(),
        }
    }
    out
}

/// Edges sharing a vertex with `e` — the adjacency for an edge-mode shortest
/// path.
fn edge_neighbours(topo: &MeshTopology, mesh: &Mesh, e: EdgeId) -> Vec<EdgeId> {
    let Some(edge) = mesh.edge(e) else { return Vec::new() };
    let mut out: Vec<EdgeId> = Vec::new();
    for &v in &edge.verts {
        for &n in topo.vertex_edges(v) {
            if n != e && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// Faces sharing an edge with `f` — the adjacency for a face-mode shortest
/// path.
fn face_neighbours(topo: &MeshTopology, mesh: &Mesh, f: FaceId) -> Vec<FaceId> {
    let mut out: Vec<FaceId> = Vec::new();
    for e in topo.face_edges(mesh, f) {
        for &n in topo.edge_faces(e) {
            if n != f && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// A representative vertex for any [`Element`] — the vertex itself, an edge's
/// first endpoint, or a face's first corner.
fn element_vertex(mesh: &Mesh, e: Element) -> Option<VertexId> {
    match e {
        Element::Vertex(v) => Some(v),
        Element::Edge(ed) => mesh.edge(ed).map(|x| x.verts[0]),
        Element::Face(f) => mesh.face_vertices(f).first().copied(),
    }
}

/// `true` when `a` and `b` agree to a relative tolerance `t`
/// (`|a - b| <= t * max(|a|, |b|, eps)`).
fn rel_eq(a: f64, b: f64, t: f64) -> bool {
    (a - b).abs() <= t * a.abs().max(b.abs()).max(1e-9)
}

/// `true` when `a` and `b` are parallel **or anti-parallel** to within
/// `angle_tol` radians (an undirected direction comparison). Zero-length inputs
/// never match.
fn dir_parallel(a: Vec3, b: Vec3, angle_tol: f64) -> bool {
    let (la, lb) = (a.length(), b.length());
    if la < 1e-12 || lb < 1e-12 {
        return false;
    }
    (a.dot(b).abs() / (la * lb)).min(1.0) >= angle_tol.cos()
}

/// One selected-element trait value for [`Selection::select_similar`] in edge
/// mode.
#[derive(Debug, Clone, Copy)]
enum EdgeTrait {
    Length(f64),
    Direction(Vec3),
    FaceCount(usize),
}

impl EdgeTrait {
    fn matches(&self, reference: &EdgeTrait, threshold: f64) -> bool {
        match (self, reference) {
            (EdgeTrait::Length(a), EdgeTrait::Length(b)) => rel_eq(*a, *b, threshold),
            (EdgeTrait::Direction(a), EdgeTrait::Direction(b)) => dir_parallel(*a, *b, threshold),
            (EdgeTrait::FaceCount(a), EdgeTrait::FaceCount(b)) => a == b,
            _ => false,
        }
    }
}

fn edge_trait(mesh: &Mesh, topo: &MeshTopology, e: EdgeId, t: SimilarTrait) -> EdgeTrait {
    let Some(edge) = mesh.edge(e) else { return EdgeTrait::FaceCount(usize::MAX) };
    let (a, b) = (
        mesh.vertex(edge.verts[0]).map(|v| v.position).unwrap_or(Vec3::ZERO),
        mesh.vertex(edge.verts[1]).map(|v| v.position).unwrap_or(Vec3::ZERO),
    );
    match t {
        SimilarTrait::EdgeLength => EdgeTrait::Length(b.sub(a).length()),
        SimilarTrait::EdgeDirection => EdgeTrait::Direction(b.sub(a)),
        SimilarTrait::EdgeFaceCount => EdgeTrait::FaceCount(topo.edge_faces(e).len()),
        _ => EdgeTrait::FaceCount(usize::MAX),
    }
}

/// One selected-element trait value for [`Selection::select_similar`] in face
/// mode.
#[derive(Debug, Clone, Copy)]
enum FaceTrait {
    Area(f64),
    Sides(usize),
    Perimeter(f64),
    Normal(Vec3),
    /// Unit normal + the centroid it passes through (for a coplanar test).
    Plane(Vec3, Vec3),
}

impl FaceTrait {
    fn matches(&self, reference: &FaceTrait, threshold: f64) -> bool {
        match (self, reference) {
            (FaceTrait::Area(a), FaceTrait::Area(b)) => rel_eq(*a, *b, threshold),
            (FaceTrait::Sides(a), FaceTrait::Sides(b)) => a == b,
            (FaceTrait::Perimeter(a), FaceTrait::Perimeter(b)) => rel_eq(*a, *b, threshold),
            (FaceTrait::Normal(a), FaceTrait::Normal(b)) => dir_parallel(*a, *b, threshold),
            (FaceTrait::Plane(na, ca), FaceTrait::Plane(nb, cb)) => {
                dir_parallel(*na, *nb, 0.0087) && (nb.dot(ca.sub(*cb))).abs() <= threshold
            }
            _ => false,
        }
    }
}

fn face_trait(mesh: &Mesh, f: FaceId, t: SimilarTrait) -> FaceTrait {
    match t {
        SimilarTrait::FaceArea => FaceTrait::Area(face_area(mesh, f)),
        SimilarTrait::FaceSides => FaceTrait::Sides(mesh.face_vertices(f).len()),
        SimilarTrait::FacePerimeter => FaceTrait::Perimeter(face_perimeter(mesh, f)),
        SimilarTrait::FaceNormal => FaceTrait::Normal(mesh.face_normal(f)),
        SimilarTrait::FaceCoplanar => FaceTrait::Plane(mesh.face_normal(f), mesh.face_centroid(f)),
        _ => FaceTrait::Sides(usize::MAX),
    }
}

/// Face area by Newell's method (half the magnitude of the Newell vector) —
/// robust for non-planar / concave polygons.
fn face_area(mesh: &Mesh, f: FaceId) -> f64 {
    let vs = mesh.face_vertices(f);
    if vs.len() < 3 {
        return 0.0;
    }
    let mut n = Vec3::ZERO;
    for i in 0..vs.len() {
        let cur = mesh.vertex(vs[i]).map(|v| v.position).unwrap_or(Vec3::ZERO);
        let nxt = mesh.vertex(vs[(i + 1) % vs.len()]).map(|v| v.position).unwrap_or(Vec3::ZERO);
        n = n.add(Vec3::new(
            (cur.y - nxt.y) * (cur.z + nxt.z),
            (cur.z - nxt.z) * (cur.x + nxt.x),
            (cur.x - nxt.x) * (cur.y + nxt.y),
        ));
    }
    n.length() * 0.5
}

/// Face perimeter — the sum of its boundary edge lengths.
fn face_perimeter(mesh: &Mesh, f: FaceId) -> f64 {
    let vs = mesh.face_vertices(f);
    let n = vs.len();
    (0..n)
        .map(|i| {
            let a = mesh.vertex(vs[i]).map(|v| v.position).unwrap_or(Vec3::ZERO);
            let b = mesh.vertex(vs[(i + 1) % n]).map(|v| v.position).unwrap_or(Vec3::ZERO);
            b.sub(a).length()
        })
        .sum()
}

/// Flood-fill the connected components (by shared edge) that contain any of
/// `seeds`, returning the full vertex set of those components.
fn flood_components(
    mesh: &Mesh,
    seeds: impl Iterator<Item = VertexId>,
    delimiters: Option<&BTreeSet<EdgeId>>,
) -> BTreeSet<VertexId> {
    // Adjacency, optionally omitting delimiter edges.
    let blocked: BTreeSet<(usize, usize)> = delimiters
        .map(|d| {
            d.iter()
                .filter_map(|&e| mesh.edge(e))
                .map(|ed| (ed.verts[0].0.min(ed.verts[1].0), ed.verts[0].0.max(ed.verts[1].0)))
                .collect()
        })
        .unwrap_or_default();
    let mut adj: HashMap<VertexId, Vec<VertexId>> = HashMap::new();
    for e in 0..mesh.edge_count() {
        if let Some(ed) = mesh.edge(EdgeId(e)) {
            let (a, b) = (ed.verts[0], ed.verts[1]);
            if blocked.contains(&(a.0.min(b.0), a.0.max(b.0))) {
                continue;
            }
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
    }
    let mut seen: BTreeSet<VertexId> = seeds.collect();
    let mut stack: Vec<VertexId> = seen.iter().copied().collect();
    while let Some(v) = stack.pop() {
        for &n in adj.get(&v).map(|s| s.as_slice()).unwrap_or(&[]) {
            if seen.insert(n) {
                stack.push(n);
            }
        }
    }
    seen
}

/// Even-odd (ray-casting) point-in-polygon test for a closed 2-D ring.
///
/// `polygon` is a list of `[x, y]` vertices; the edge from the last back to the
/// first is implied. A point exactly on an edge is reported inconsistently (as
/// with any even-odd test) — acceptable for interactive selection.
fn point_in_polygon_2d(p: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let n = polygon.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (polygon[i][0], polygon[i][1]);
        let (xj, yj) = (polygon[j][0], polygon[j][1]);
        let intersects = ((yi > p[1]) != (yj > p[1]))
            && (p[0] < (xj - xi) * (p[1] - yi) / (yj - yi) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    fn cube() -> Mesh {
        primitives::cube(2.0) // corners at ±1
    }

    #[test]
    fn select_all_none_invert_vertex_mode() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Vertex);
        assert!(s.is_empty());

        s.select_all(&m);
        assert_eq!(s.vertex_count(), 8);
        assert_eq!(s.edge_count(), 12);
        assert_eq!(s.face_count(), 6);

        s.invert(&m);
        assert!(s.is_empty(), "invert of all is none");

        s.select(&m, Element::Vertex(VertexId(0)));
        s.invert(&m);
        assert_eq!(s.vertex_count(), 7);
        assert!(!s.is_selected(Element::Vertex(VertexId(0))));
    }

    #[test]
    fn edge_select_flushes_down_to_vertices() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Edge);
        s.select(&m, Element::Edge(EdgeId(0)));
        let e = m.edge(EdgeId(0)).unwrap();
        assert!(s.is_selected(Element::Vertex(e.verts[0])));
        assert!(s.is_selected(Element::Vertex(e.verts[1])));
        assert_eq!(s.vertex_count(), 2);
    }

    #[test]
    fn set_mode_face_to_vertex_and_back_round_trips_a_full_face() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Face);
        s.select(&m, Element::Face(FaceId(0)));
        let want = m.face_vertices(FaceId(0)).len();
        assert_eq!(s.vertex_count(), want);

        s.set_mode(&m, SelectMode::Vertex);
        assert_eq!(s.vertex_count(), want);

        s.set_mode(&m, SelectMode::Face);
        assert_eq!(s.face_count(), 1, "the fully-selected face comes back");
    }

    #[test]
    fn set_mode_vertex_to_face_drops_partial_faces() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Vertex);
        // Two of a face's corners only.
        let vs = m.face_vertices(FaceId(0));
        s.select(&m, Element::Vertex(vs[0]));
        s.select(&m, Element::Vertex(vs[1]));
        s.set_mode(&m, SelectMode::Face);
        assert_eq!(s.face_count(), 0, "a half-selected face is not selected in face mode");
    }

    #[test]
    fn box_select_enclosed_vs_touching() {
        let m = cube(); // corners at ±1
        // A box covering only x < 0.
        let (min, max) = (Vec3::new(-2.0, -2.0, -2.0), Vec3::new(0.0, 2.0, 2.0));

        let mut touch = Selection::new(SelectMode::Face);
        touch.select_in_box(&m, min, max, RegionMode::Touching);

        let mut enclosed = Selection::new(SelectMode::Face);
        enclosed.select_in_box(&m, min, max, RegionMode::Enclosed);

        // Every face of a cube has some x<0 corner except the +X face, so
        // "touching" catches 5 of 6; "enclosed" catches only the -X face.
        assert_eq!(touch.face_count(), 5);
        assert_eq!(enclosed.face_count(), 1);
    }

    #[test]
    fn sphere_select_vertex_mode() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Vertex);
        // Radius 1.8 < sqrt(3)*1 ≈ 1.732? no: sqrt(3) ≈ 1.732, so r=1.8 catches all 8.
        s.select_in_sphere(&m, Vec3::ZERO, 1.8, RegionMode::Touching);
        assert_eq!(s.vertex_count(), 8);

        let mut none = Selection::new(SelectMode::Vertex);
        none.select_in_sphere(&m, Vec3::ZERO, 1.0, RegionMode::Touching);
        assert_eq!(none.vertex_count(), 0, "cube corners are at distance sqrt(3) > 1");
    }

    #[test]
    fn screen_polygon_is_an_orthographic_box() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Vertex);
        // Project by dropping Z; select the square x,y in [0,2] (the +X+Y quadrant).
        let rect = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        s.select_in_screen_polygon(&m, |p| [p.x, p.y], &rect, RegionMode::Touching);
        // Cube corners at x=+1,y=+1 (two of them, z=±1) fall inside.
        assert_eq!(s.vertex_count(), 2);
    }

    #[test]
    fn linked_grows_to_whole_component() {
        // Two disjoint cubes in one mesh.
        let a = primitives::cube(2.0);
        let mut positions = a.positions();
        let mut faces: Vec<Vec<usize>> =
            a.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let off = positions.len();
        for p in a.positions() {
            positions.push(p.add(Vec3::new(10.0, 0.0, 0.0)));
        }
        for f in a.polygons() {
            faces.push(f.iter().map(|v| v.0 + off).collect());
        }
        let m = Mesh::from_polygons(&positions, &faces);

        let mut s = Selection::new(SelectMode::Vertex);
        s.select(&m, Element::Vertex(VertexId(0)));
        s.select_linked(&m);
        assert_eq!(s.vertex_count(), 8, "only the first cube's component");

        s.select_linked_from(&m, Element::Vertex(VertexId(off)));
        assert_eq!(s.vertex_count(), 16, "now both components");
    }

    #[test]
    fn select_mirror_across_x_finds_the_opposite_face() {
        let m = cube();
        // Find the -X face and the +X face by centroid.
        let neg_x = (0..m.face_count() as usize)
            .map(FaceId)
            .find(|&f| m.face_centroid(f).x < -0.5)
            .unwrap();
        let pos_x = (0..m.face_count() as usize)
            .map(FaceId)
            .find(|&f| m.face_centroid(f).x > 0.5)
            .unwrap();

        let mut s = Selection::new(SelectMode::Face);
        s.select(&m, Element::Face(neg_x));
        s.select_mirror(&m, Axis::X, 1e-6, true);
        assert!(s.is_selected(Element::Face(pos_x)));
        assert!(s.is_selected(Element::Face(neg_x)), "extend keeps the original");

        let mut replaced = Selection::new(SelectMode::Face);
        replaced.select(&m, Element::Face(neg_x));
        replaced.select_mirror(&m, Axis::X, 1e-6, false);
        assert!(replaced.is_selected(Element::Face(pos_x)));
        assert!(!replaced.is_selected(Element::Face(neg_x)), "no extend replaces");
    }

    #[test]
    fn set_algebra_composes_extend_and_subtract() {
        let m = cube();
        let mut base = Selection::new(SelectMode::Vertex);
        base.select_in_box(&m, Vec3::new(-2.0, -2.0, -2.0), Vec3::new(0.0, 2.0, 2.0), RegionMode::Touching);
        let n0 = base.vertex_count();

        let mut rhs = Selection::new(SelectMode::Vertex);
        rhs.select_in_box(&m, Vec3::new(-2.0, -2.0, -2.0), Vec3::new(2.0, 2.0, 0.0), RegionMode::Touching);

        let mut ext = base.clone();
        ext.union(&m, &rhs);
        assert!(ext.vertex_count() >= n0);

        let mut sub = base.clone();
        sub.subtract(&m, &rhs);
        assert!(sub.vertex_count() <= n0);
    }

    #[test]
    fn edge_loop_selection_picks_a_grid_row_in_edge_mode() {
        let m = primitives::grid(5, 5, 5.0);
        let topo = crate::topology::MeshTopology::new(&m);
        // An interior horizontal edge.
        let e = (0..m.edge_count()).map(EdgeId).find(|&e| {
            let ed = m.edge(e).unwrap();
            let (a, b) = (m.vertex(ed.verts[0]).unwrap().position, m.vertex(ed.verts[1]).unwrap().position);
            topo.is_manifold_edge(e) && (a.y - b.y).abs() < 1e-9
        }).unwrap();

        let mut s = Selection::new(SelectMode::Edge);
        s.select_edge_loop(&m, e);
        assert_eq!(s.edge_count(), 5);

        // In vertex mode the same loop selects its 6 vertices.
        let mut sv = Selection::new(SelectMode::Vertex);
        sv.select_edge_loop(&m, e);
        assert_eq!(sv.vertex_count(), 6);
    }

    #[test]
    fn face_loop_selection_is_a_strip_of_faces() {
        let m = primitives::grid(6, 6, 6.0);
        let topo = crate::topology::MeshTopology::new(&m);
        let e = topo.face_edges(&m, FaceId(0))[0];
        let mut s = Selection::new(SelectMode::Face);
        s.select_face_loop(&m, e);
        assert_eq!(s.face_count(), 6);
    }

    #[test]
    fn boundary_loop_selection_rings_a_grid_border() {
        let m = primitives::grid(4, 4, 4.0);
        let topo = crate::topology::MeshTopology::new(&m);
        let b = (0..m.edge_count()).map(EdgeId).find(|&e| topo.is_boundary_edge(e)).unwrap();
        let mut s = Selection::new(SelectMode::Edge);
        s.select_boundary_loop(&m, b);
        assert_eq!(s.edge_count(), 16, "the whole 16-edge border");
    }

    #[test]
    fn shortest_path_selection_vertex_mode() {
        let m = primitives::grid(4, 4, 4.0);
        let corner_a = (0..m.vertex_count()).map(VertexId).min_by(|&a, &b| {
            let (pa, pb) = (m.vertex(a).unwrap().position, m.vertex(b).unwrap().position);
            (pa.x + pa.y).partial_cmp(&(pb.x + pb.y)).unwrap()
        }).unwrap();
        let corner_b = (0..m.vertex_count()).map(VertexId).max_by(|&a, &b| {
            let (pa, pb) = (m.vertex(a).unwrap().position, m.vertex(b).unwrap().position);
            (pa.x + pa.y).partial_cmp(&(pb.x + pb.y)).unwrap()
        }).unwrap();

        let mut s = Selection::new(SelectMode::Vertex);
        s.select_shortest_path(&m, Element::Vertex(corner_a), Element::Vertex(corner_b));
        assert_eq!(s.vertex_count(), 9, "8 grid steps → 9 vertices");
    }

    #[test]
    fn shortest_path_selection_face_mode() {
        let m = primitives::grid(4, 4, 4.0);
        let mut s = Selection::new(SelectMode::Face);
        s.select_shortest_path(&m, Element::Face(FaceId(0)), Element::Face(FaceId(m.face_count() - 1)));
        assert!(s.face_count() >= 2 && s.face_count() <= m.face_count());
        assert!(s.is_selected(Element::Face(FaceId(0))));
        assert!(s.is_selected(Element::Face(FaceId(m.face_count() - 1))));
    }

    #[test]
    fn select_more_then_less_returns_to_the_seed_interior() {
        let m = primitives::grid(6, 6, 6.0);
        let mut s = Selection::new(SelectMode::Face);
        s.select(&m, Element::Face(FaceId(14))); // an interior face
        let n1 = s.face_count();
        s.select_more(&m);
        assert!(s.face_count() > n1, "grew");
        s.select_less(&m);
        assert_eq!(s.face_count(), n1, "shrank back to the single interior face");
    }

    #[test]
    fn select_similar_face_sides_picks_all_quads() {
        let m = primitives::grid(3, 3, 3.0); // all quads
        let mut s = Selection::new(SelectMode::Face);
        s.select(&m, Element::Face(FaceId(0)));
        s.select_similar(&m, SimilarTrait::FaceSides, 0.0);
        assert_eq!(s.face_count(), m.face_count(), "every grid face is a quad");
    }

    #[test]
    fn select_similar_face_normal_picks_one_cube_side() {
        let m = cube();
        let mut s = Selection::new(SelectMode::Face);
        s.select(&m, Element::Face(FaceId(0)));
        s.select_similar(&m, SimilarTrait::FaceNormal, 0.01);
        // A cube has no two faces with parallel normals except opposite pairs,
        // which `dir_parallel` treats as matching → 2 faces.
        assert_eq!(s.face_count(), 2);
    }

    #[test]
    fn select_similar_edge_length_on_a_cube() {
        let m = cube(); // all 12 edges equal length
        let mut s = Selection::new(SelectMode::Edge);
        s.select(&m, Element::Edge(EdgeId(0)));
        s.select_similar(&m, SimilarTrait::EdgeLength, 0.001);
        assert_eq!(s.edge_count(), 12);
    }

    #[test]
    fn checker_deselect_keeps_every_other() {
        let m = primitives::grid(4, 1, 4.0); // 4 faces in a row: ids 0,1,2,3
        let mut s = Selection::all(&m, SelectMode::Face);
        assert_eq!(s.face_count(), 4);
        s.checker_deselect(&m, 1, 1, 0);
        assert_eq!(s.face_count(), 2);
        assert!(s.is_selected(Element::Face(FaceId(0))));
        assert!(!s.is_selected(Element::Face(FaceId(1))));
        assert!(s.is_selected(Element::Face(FaceId(2))));
    }

    #[test]
    fn non_manifold_finds_grid_border_in_edge_mode() {
        let m = primitives::grid(4, 4, 4.0);
        let mut s = Selection::new(SelectMode::Edge);
        s.select_non_manifold(&m, NonManifoldKinds { wire: false, boundary: true, multiple_faces: false });
        assert_eq!(s.edge_count(), 16, "the 16 boundary edges");

        // A closed cube has no non-manifold edges.
        let c = cube();
        let mut sc = Selection::new(SelectMode::Edge);
        sc.select_non_manifold(&c, NonManifoldKinds::all());
        assert_eq!(sc.edge_count(), 0);
    }

    #[test]
    fn loose_geometry_catches_a_stray_vertex() {
        let mut positions = cube().positions();
        let faces: Vec<Vec<usize>> =
            cube().polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        positions.push(Vec3::new(10.0, 10.0, 10.0)); // unused vertex
        let m = Mesh::from_polygons(&positions, &faces);

        let mut s = Selection::new(SelectMode::Vertex);
        s.select_loose(&m);
        assert_eq!(s.vertex_count(), 1);
        assert!(s.is_selected(Element::Vertex(VertexId(positions.len() - 1))));
    }

    #[test]
    fn faces_by_sides_selects_the_ngon() {
        // A grid of quads plus one triangle appended.
        let g = primitives::grid(2, 2, 2.0);
        let mut positions = g.positions();
        let mut faces: Vec<Vec<usize>> =
            g.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let a = positions.len();
        positions.push(Vec3::new(5.0, 0.0, 0.0));
        positions.push(Vec3::new(6.0, 0.0, 0.0));
        positions.push(Vec3::new(5.5, 1.0, 0.0));
        faces.push(vec![a, a + 1, a + 2]); // a triangle
        let m = Mesh::from_polygons(&positions, &faces);

        let mut s = Selection::new(SelectMode::Face);
        s.select_faces_by_sides(&m, 4, NumberCompare::Less);
        assert_eq!(s.face_count(), 1, "only the triangle has < 4 sides");
    }

    #[test]
    fn interior_faces_selects_a_cube_diaphragm() {
        // A cube split in half at z = 0 by an internal quad. Each diaphragm
        // edge is shared by the diaphragm + an upper + a lower side face = 3
        // users, so the diaphragm is an interior face and nothing else is.
        #[rustfmt::skip]
        let p = [
            Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0,  0.0), Vec3::new(1.0, -1.0,  0.0), Vec3::new(1.0, 1.0,  0.0), Vec3::new(-1.0, 1.0,  0.0),
            Vec3::new(-1.0, -1.0,  1.0), Vec3::new(1.0, -1.0,  1.0), Vec3::new(1.0, 1.0,  1.0), Vec3::new(-1.0, 1.0,  1.0),
        ];
        let faces = vec![
            vec![0, 3, 2, 1],          // bottom cap
            vec![8, 9, 10, 11],        // top cap
            vec![0, 1, 5, 4], vec![1, 2, 6, 5], vec![2, 3, 7, 6], vec![3, 0, 4, 7], // lower sides
            vec![4, 5, 9, 8], vec![5, 6, 10, 9], vec![6, 7, 11, 10], vec![7, 4, 8, 11], // upper sides
            vec![4, 5, 6, 7],          // diaphragm (id 10)
        ];
        let m = Mesh::from_polygons(&p, &faces);
        let mut s = Selection::new(SelectMode::Face);
        s.select_interior_faces(&m);
        assert_eq!(s.face_count(), 1);
        assert!(s.is_selected(Element::Face(FaceId(10))));
    }
}
