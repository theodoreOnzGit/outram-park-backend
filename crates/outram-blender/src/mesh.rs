//! BMesh-inspired **index-based half-edge** mesh topology.
//!
//! This is the core data structure every other module operates on. It mirrors
//! the four-element model of Blender's **BMesh** (`source/blender/bmesh`):
//!
//! | This crate | Blender BMesh | Role |
//! |---|---|---|
//! | [`Vertex`] | `BMVert` | a point in space |
//! | [`Edge`] | `BMEdge` | an undirected connection between two vertices |
//! | [`Loop`] | `BMLoop` | one corner of one face — a *directed* half-edge |
//! | [`Face`] | `BMFace` | a polygon, defined by its ring of loops |
//!
//! The key idea borrowed from BMesh is the **loop**: a face is not stored as a
//! list of vertex indices but as a cycle of [`Loop`] records, each of which
//! knows its vertex, its edge, its face, and its `next`/`prev` neighbours
//! around the face. Two loops that share an edge but belong to different faces
//! are the two "half-edges" of that edge. This is what makes adjacency queries
//! (walk a face, walk around a vertex) O(1) instead of a search.
//!
//! ## No pointers, no lifetimes — indices only
//!
//! Blender's C uses raw pointers between `BMVert`/`BMEdge`/`BMLoop`/`BMFace`.
//! The workspace design rules forbid `&'a`-linked graph nodes, so every link
//! here is a **newtype index** ([`VertexId`], [`EdgeId`], [`LoopId`],
//! [`FaceId`]) into one of the `Vec`s inside [`Mesh`]. This is the
//! `CellId(usize)`-into-a-`Vec` pattern the workspace `CLAUDE.md` prescribes.
//!
//! ## What is and is not implemented at scaffold stage
//!
//! Implemented: construction ([`Mesh::add_vertex`], [`Mesh::add_face`] with
//! automatic edge deduplication) and read-only queries (element counts,
//! [`Mesh::face_vertices`], [`Mesh::euler_characteristic`]). This is enough for
//! the [`crate::primitives`] generators to build valid closed meshes and for
//! their tests to check `V - E + F`.
//!
//! Not yet implemented: the full radial-cycle links around an edge (BMesh's
//! `radial_next`/`radial_prev`, needed to enumerate *all* faces on an edge for
//! non-manifold meshes), Euler operators (split/join), and per-element custom
//! data layers. Those are tracked in [`crate::ops`] and the crate's beads.

use crate::math::Vec3;

/// Index of a [`Vertex`] within a [`Mesh`]'s vertex array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VertexId(pub usize);

/// Index of an [`Edge`] within a [`Mesh`]'s edge array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId(pub usize);

/// Index of a [`Loop`] within a [`Mesh`]'s loop array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LoopId(pub usize);

/// Index of a [`Face`] within a [`Mesh`]'s face array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub usize);

/// A mesh vertex: a point in model space plus one incident loop (BMesh `BMVert`).
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    /// Position in dimensionless model space (see [`crate::math`]).
    pub position: Vec3,
    /// One [`Loop`] that starts at this vertex, or `None` for an isolated
    /// vertex not yet used by any face. A full BMesh stores the disk cycle of
    /// all incident edges; this scaffold keeps just a single representative.
    pub loop_id: Option<LoopId>,
}

/// An undirected edge between two vertices (BMesh `BMEdge`).
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    /// The two endpoint vertices. Order is the order the edge was first
    /// created; [`Mesh::add_face`] treats `[a, b]` and `[b, a]` as the same
    /// edge when deduplicating.
    pub verts: [VertexId; 2],
}

/// One corner of one face — a directed half-edge (BMesh `BMLoop`).
///
/// A [`Face`] owns a cyclic list of loops. Following [`Loop::next`] repeatedly
/// walks the face's boundary counter-clockwise (as wound at construction) and
/// returns to the start after [`Face::len`] steps.
#[derive(Debug, Clone, Copy)]
pub struct Loop {
    /// The vertex this loop-corner is anchored at (the *from* vertex of the
    /// directed half-edge).
    pub vert: VertexId,
    /// The undirected [`Edge`] this loop runs along (from [`Loop::vert`] to the
    /// next loop's vertex).
    pub edge: EdgeId,
    /// The [`Face`] this loop belongs to.
    pub face: FaceId,
    /// The next loop counter-clockwise around [`Loop::face`].
    pub next: LoopId,
    /// The previous loop counter-clockwise around [`Loop::face`].
    pub prev: LoopId,
}

/// A polygon face, stored as an entry point into its ring of loops (BMesh `BMFace`).
#[derive(Debug, Clone, Copy)]
pub struct Face {
    /// Any one loop on this face's boundary; walk [`Loop::next`] from here to
    /// enumerate the whole face.
    pub loop_start: LoopId,
    /// Number of sides (vertices == edges == loops) on this face.
    pub len: usize,
}

/// An index-based half-edge mesh: the container all elements live in.
///
/// Build one with [`Mesh::new`], add geometry with [`Mesh::add_vertex`] and
/// [`Mesh::add_face`], then query it. All connectivity is by index into the
/// four `Vec`s below, so a `Mesh` is `Clone` and contains no borrows.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
    loops: Vec<Loop>,
    faces: Vec<Face>,
}

impl Mesh {
    /// Create an empty mesh with no vertices, edges, loops, or faces.
    pub fn new() -> Self {
        Mesh::default()
    }

    /// Add an isolated vertex at `position` and return its [`VertexId`].
    ///
    /// The vertex has no incident loop until it is used by an [`Mesh::add_face`]
    /// call.
    pub fn add_vertex(&mut self, position: Vec3) -> VertexId {
        let id = VertexId(self.vertices.len());
        self.vertices.push(Vertex { position, loop_id: None });
        id
    }

    /// Add a polygon face through the given vertices, in boundary order.
    ///
    /// Winding: `verts` should be listed counter-clockwise as seen from the
    /// side the outward normal points toward. Edges are **deduplicated** — if
    /// an edge between two consecutive vertices already exists (in either
    /// direction) it is reused, so a closed surface ends up with the correct
    /// edge count (e.g. a cube built from 6 quads has 12 edges, not 24). This
    /// is what makes [`Mesh::euler_characteristic`] come out to 2 for a closed
    /// genus-0 mesh.
    ///
    /// # Panics
    ///
    /// Panics if `verts.len() < 3` (a face needs at least three corners) or if
    /// any [`VertexId`] is out of range for this mesh — both indicate a caller
    /// bug in a generator, caught loudly rather than producing broken topology.
    pub fn add_face(&mut self, verts: &[VertexId]) -> FaceId {
        let n = verts.len();
        assert!(n >= 3, "a face needs at least 3 vertices, got {n}");
        for v in verts {
            assert!(
                v.0 < self.vertices.len(),
                "vertex {} out of range (mesh has {} vertices)",
                v.0,
                self.vertices.len()
            );
        }

        let face_id = FaceId(self.faces.len());
        let loop_base = self.loops.len();

        // First pass: resolve/create the edge for each side and build loops.
        for (i, &v) in verts.iter().enumerate() {
            let next_v = verts[(i + 1) % n];
            let edge = self.find_or_add_edge(v, next_v);

            let this_loop = LoopId(loop_base + i);
            let next_loop = LoopId(loop_base + (i + 1) % n);
            let prev_loop = LoopId(loop_base + (i + n - 1) % n);

            self.loops.push(Loop {
                vert: v,
                edge,
                face: face_id,
                next: next_loop,
                prev: prev_loop,
            });

            // Give the vertex a representative incident loop if it lacks one.
            if self.vertices[v.0].loop_id.is_none() {
                self.vertices[v.0].loop_id = Some(this_loop);
            }
        }

        self.faces.push(Face { loop_start: LoopId(loop_base), len: n });
        face_id
    }

    /// Find an existing undirected edge between `a` and `b`, or create one.
    fn find_or_add_edge(&mut self, a: VertexId, b: VertexId) -> EdgeId {
        for (i, e) in self.edges.iter().enumerate() {
            if (e.verts[0] == a && e.verts[1] == b) || (e.verts[0] == b && e.verts[1] == a) {
                return EdgeId(i);
            }
        }
        let id = EdgeId(self.edges.len());
        self.edges.push(Edge { verts: [a, b] });
        id
    }

    /// Number of vertices (`V`).
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Number of unique edges (`E`).
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Number of loops (directed half-edge corners) — equals the sum of all
    /// face side counts.
    pub fn loop_count(&self) -> usize {
        self.loops.len()
    }

    /// Number of faces (`F`).
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Read-only access to a vertex by id, or `None` if out of range.
    pub fn vertex(&self, id: VertexId) -> Option<&Vertex> {
        self.vertices.get(id.0)
    }

    /// Read-only access to an edge by id, or `None` if out of range.
    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(id.0)
    }

    /// Read-only access to a loop by id, or `None` if out of range.
    pub fn loop_at(&self, id: LoopId) -> Option<&Loop> {
        self.loops.get(id.0)
    }

    /// Read-only access to a face by id, or `None` if out of range.
    pub fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.get(id.0)
    }

    /// The vertices of a face, in boundary (loop) order.
    ///
    /// Walks the face's loop cycle via [`Loop::next`] and collects each loop's
    /// vertex. Returns an empty `Vec` if `face` is out of range.
    pub fn face_vertices(&self, face: FaceId) -> Vec<VertexId> {
        let Some(f) = self.faces.get(face.0) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(f.len);
        let mut cur = f.loop_start;
        for _ in 0..f.len {
            let l = self.loops[cur.0];
            out.push(l.vert);
            cur = l.next;
        }
        out
    }

    /// The Euler characteristic `chi = V - E + F`.
    ///
    /// For a closed, orientable, genus-0 surface (topologically a sphere) this
    /// equals **2**; the [`crate::primitives`] closed generators are tested
    /// against this. A single flat grid/plane patch is a disc, for which
    /// `chi = 1`. In general `chi = 2 - 2g` for a closed surface of genus `g`,
    /// so this is a quick topological sanity check on generated meshes.
    ///
    /// Returned as `i64` because `V - E + F` can be negative for high-genus
    /// meshes.
    pub fn euler_characteristic(&self) -> i64 {
        self.vertices.len() as i64 - self.edges.len() as i64 + self.faces.len() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single triangle: 3 vertices, 3 edges, 1 face; it is a disc, chi = 1.
    #[test]
    fn single_triangle_topology() {
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        let f = m.add_face(&[a, b, c]);

        assert_eq!(m.vertex_count(), 3);
        assert_eq!(m.edge_count(), 3);
        assert_eq!(m.face_count(), 1);
        assert_eq!(m.loop_count(), 3);
        assert_eq!(m.euler_characteristic(), 1);
        assert_eq!(m.face_vertices(f), vec![a, b, c]);
    }

    /// Two triangles sharing an edge: the shared edge must be deduplicated, so
    /// E = 5 (not 6). V=4, E=5, F=2 -> chi = 1 (a quad disc).
    #[test]
    fn shared_edge_is_deduplicated() {
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(1.0, 1.0, 0.0));
        let d = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c]);
        m.add_face(&[a, c, d]);

        assert_eq!(m.edge_count(), 5, "shared edge a-c must be reused");
        assert_eq!(m.face_count(), 2);
        assert_eq!(m.euler_characteristic(), 1);
    }
}
