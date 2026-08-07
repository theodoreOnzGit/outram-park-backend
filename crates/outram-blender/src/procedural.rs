// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// A directed-acyclic-graph geometry evaluator. Follows the published architecture
// of Blender's Geometry Nodes (nodes/geometry, GPL-2.0-or-later) — concepts only,
// no upstream source was copied. Node bodies delegate to this crate's own
// operators, each cited in its own module.
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

//! Procedural geometry generation — a **Geometry-Nodes-style** node graph
//! (Blender's `nodes/geometry`, the Geometry Nodes system).
//!
//! ## The concept
//!
//! Blender's Geometry Nodes builds geometry by evaluating a directed acyclic
//! graph of nodes: *input* nodes create primitives, *processing* nodes
//! transform or combine geometry (many wrap the same verbs as [`crate::ops`]),
//! and an *output* node yields the final mesh. The graph is data, not code, so
//! a design can be authored, stored, and replayed — which is exactly what an
//! OUTRAM PARK reactor-geometry generator wants (parametric fuel pins, lattices,
//! coolant channels driven by a few numeric inputs).
//!
//! ## What is implemented
//!
//! [`GeometryGraph::evaluate`] is a **real** evaluator. It locates the single
//! [`GeometryNode::OutputMesh`] node and walks the graph upstream by
//! [`NodeId`], building a [`Mesh`] bottom-up:
//!
//! - [`GeometryNode::Primitive`] emits a fresh primitive from [`crate::primitives`];
//! - [`GeometryNode::Transform`] applies a translation via [`crate::transform::Affine3`];
//! - [`GeometryNode::Join`] concatenates two meshes (offsetting the second
//!   operand's face indices by the first's vertex count);
//! - [`GeometryNode::Subdivide`] delegates to [`crate::subdivision::catmull_clark`];
//! - [`GeometryNode::Boolean`] delegates to [`crate::boolean::boolean`].
//!
//! The walk is defensive: an out-of-range [`NodeId`] returns
//! [`ProceduralError::BadNode`] and a cyclic reference returns
//! [`ProceduralError::Cycle`] — a malformed graph never panics.
//!
//! ## Why an enum of nodes
//!
//! The node kinds are a closed set, so [`GeometryNode`] is an enum (no trait
//! objects), and edges between nodes are **indices** ([`NodeId`]) into the
//! graph's node `Vec` — the same no-pointers/no-lifetimes discipline as
//! [`crate::mesh`].

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::transform::Affine3;

/// Errors from evaluating a [`GeometryGraph`].
#[derive(Debug, thiserror::Error)]
pub enum ProceduralError {
    /// The graph has no [`GeometryNode::OutputMesh`] node to read a result from.
    #[error("geometry graph has no output node")]
    NoOutput,
    /// A node referenced a [`NodeId`] that is out of range for the graph — a
    /// malformed edge. Carries the offending id.
    #[error("geometry graph references out-of-range node {0:?}")]
    BadNode(NodeId),
    /// A cycle was detected while walking upstream from the output — the node
    /// carried is one that was re-entered while already on the current
    /// depth-first path. A valid geometry graph must be acyclic.
    #[error("geometry graph contains a cycle at node {0:?}")]
    Cycle(NodeId),
    /// A [`GeometryNode::Boolean`] node's underlying CSG operation failed; the
    /// [`crate::boolean::BooleanError`] is propagated unchanged.
    #[error("boolean node failed: {0}")]
    Boolean(#[from] crate::boolean::BooleanError),
}

/// Index of a [`GeometryNode`] within a [`GeometryGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Which primitive an input node emits.
///
/// Each variant maps one-to-one onto a generator in [`crate::primitives`]; the
/// parameters are the same dimensionless model-space lengths those generators
/// take (see [`crate::math`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveKind {
    /// A cube of the given side length — see [`crate::primitives::cube`].
    Cube {
        /// Full edge length.
        size: f64,
    },
    /// A UV sphere — see [`crate::primitives::uv_sphere`].
    UvSphere {
        /// Subdivisions around the polar axis (must be `>= 3`).
        segments: usize,
        /// Subdivisions pole to pole (must be `>= 2`).
        rings: usize,
        /// Sphere radius.
        radius: f64,
    },
    /// A closed cylinder with axis along Z — see [`crate::primitives::cylinder`].
    Cylinder {
        /// Number of sides around the axis (must be `>= 3`).
        segments: usize,
        /// Cylinder radius.
        radius: f64,
        /// Full height along Z (the cylinder spans `-height/2 .. height/2`).
        height: f64,
    },
}

/// A node in a procedural geometry graph.
///
/// Each node references its inputs by [`NodeId`]. Evaluation performs a
/// depth-first walk from [`GeometryNode::OutputMesh`] back to the input nodes
/// (see [`GeometryGraph::evaluate`]).
#[derive(Debug, Clone)]
pub enum GeometryNode {
    /// Source node: emit a primitive mesh.
    Primitive(PrimitiveKind),
    /// Transform the geometry coming from `input` by a uniform translation.
    Transform {
        /// Upstream node whose geometry is transformed.
        input: NodeId,
        /// Model-space translation `[dx, dy, dz]` applied to every vertex.
        translate: [f64; 3],
    },
    /// Join two geometry streams into one mesh (concatenate elements). The two
    /// operands keep their own vertices — no welding/deduplication is performed,
    /// so joining two disjoint solids yields a mesh with both, each still a
    /// separate closed surface.
    Join {
        /// First upstream node.
        a: NodeId,
        /// Second upstream node.
        b: NodeId,
    },
    /// Refine the geometry from `input` with `levels` rounds of Catmull-Clark
    /// subdivision — see [`crate::subdivision::catmull_clark`].
    Subdivide {
        /// Upstream node whose geometry is subdivided.
        input: NodeId,
        /// Number of Catmull-Clark refinement passes.
        levels: u32,
    },
    /// Combine the geometry from `a` and `b` under a CSG [`crate::ops::BooleanMode`]
    /// — see [`crate::boolean::boolean`]. A failure of the underlying operation
    /// surfaces as [`ProceduralError::Boolean`].
    Boolean {
        /// First operand node (the `A` mesh).
        a: NodeId,
        /// Second operand node (the `B` mesh).
        b: NodeId,
        /// Union / difference / intersection.
        mode: crate::ops::BooleanMode,
    },
    /// Terminal node: the graph's result is the geometry from `input`.
    OutputMesh {
        /// Upstream node providing the final mesh.
        input: NodeId,
    },
}

/// A directed graph of [`GeometryNode`]s that evaluates to a single [`Mesh`].
///
/// Nodes are stored in a `Vec` and referenced by [`NodeId`]; add nodes with
/// [`GeometryGraph::add`], which returns the new node's id for wiring later
/// nodes to it.
#[derive(Debug, Clone, Default)]
pub struct GeometryGraph {
    nodes: Vec<GeometryNode>,
}

impl GeometryGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        GeometryGraph::default()
    }

    /// Add a node and return its [`NodeId`] for wiring downstream nodes.
    pub fn add(&mut self, node: GeometryNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// Read-only access to a node by id.
    pub fn node(&self, id: NodeId) -> Option<&GeometryNode> {
        self.nodes.get(id.0)
    }

    /// Number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Evaluate the graph to a final mesh.
    ///
    /// Finds the single [`GeometryNode::OutputMesh`] node and evaluates the
    /// geometry feeding into it by a depth-first walk over [`NodeId`] edges.
    ///
    /// # Errors
    ///
    /// - [`ProceduralError::NoOutput`] if the graph has no `OutputMesh` node.
    /// - [`ProceduralError::BadNode`] if any edge references a [`NodeId`] that
    ///   is out of range for this graph.
    /// - [`ProceduralError::Cycle`] if the upstream references form a cycle.
    /// - [`ProceduralError::Boolean`] if a [`GeometryNode::Boolean`] node's CSG
    ///   operation fails.
    ///
    /// A malformed graph is reported through one of these variants; it never
    /// panics. If several `OutputMesh` nodes exist, the first (lowest
    /// [`NodeId`]) is used.
    pub fn evaluate(&self) -> Result<Mesh, ProceduralError> {
        let output = self
            .nodes
            .iter()
            .position(|n| matches!(n, GeometryNode::OutputMesh { .. }))
            .map(NodeId);
        let Some(output_id) = output else {
            return Err(ProceduralError::NoOutput);
        };
        // `on_path[i]` marks node i as currently on the active DFS path, so a
        // re-entry (a cycle) is detected without forbidding a DAG diamond
        // (a node legitimately reached by two distinct paths).
        let mut on_path = vec![false; self.nodes.len()];
        self.eval_node(output_id, &mut on_path)
    }

    /// Recursively evaluate the geometry produced by node `id`.
    ///
    /// `on_path` tracks the nodes on the current depth-first path for cycle
    /// detection: it is set on entry and cleared on exit, so a shared upstream
    /// node reached by two sibling branches is re-evaluated (allowed) while a
    /// back-edge onto the current path is rejected as a [`ProceduralError::Cycle`].
    fn eval_node(&self, id: NodeId, on_path: &mut Vec<bool>) -> Result<Mesh, ProceduralError> {
        let idx = id.0;
        let node = self.nodes.get(idx).ok_or(ProceduralError::BadNode(id))?;
        if on_path[idx] {
            return Err(ProceduralError::Cycle(id));
        }
        on_path[idx] = true;

        let result = match node {
            GeometryNode::Primitive(kind) => Ok(build_primitive(*kind)),
            GeometryNode::Transform { input, translate } => {
                let mesh = self.eval_node(*input, on_path)?;
                Ok(apply_translation(&mesh, *translate))
            }
            GeometryNode::Join { a, b } => {
                let ma = self.eval_node(*a, on_path)?;
                let mb = self.eval_node(*b, on_path)?;
                Ok(join_meshes(&ma, &mb))
            }
            GeometryNode::Subdivide { input, levels } => {
                let mesh = self.eval_node(*input, on_path)?;
                Ok(crate::subdivision::catmull_clark(&mesh, *levels))
            }
            GeometryNode::Boolean { a, b, mode } => {
                let ma = self.eval_node(*a, on_path)?;
                let mb = self.eval_node(*b, on_path)?;
                // `?` on the boolean result converts BooleanError into
                // ProceduralError::Boolean via the `#[from]` impl.
                Ok(crate::boolean::boolean(&ma, &mb, *mode)?)
            }
            GeometryNode::OutputMesh { input } => self.eval_node(*input, on_path),
        };

        on_path[idx] = false;
        result
    }
}

/// Build a fresh primitive mesh for a [`PrimitiveKind`], dispatching to the
/// matching [`crate::primitives`] generator.
fn build_primitive(kind: PrimitiveKind) -> Mesh {
    match kind {
        PrimitiveKind::Cube { size } => crate::primitives::cube(size),
        PrimitiveKind::UvSphere { segments, rings, radius } => {
            crate::primitives::uv_sphere(segments, rings, radius)
        }
        PrimitiveKind::Cylinder { segments, radius, height } => {
            crate::primitives::cylinder(segments, radius, height)
        }
    }
}

/// Apply a model-space translation `[dx, dy, dz]` to every vertex of `mesh`,
/// preserving its face connectivity.
///
/// The positions are pushed through [`Affine3::translation`] (the trusted CPU
/// transform path) and the mesh is rebuilt from the moved positions and the
/// original polygon list.
fn apply_translation(mesh: &Mesh, translate: [f64; 3]) -> Mesh {
    let xf = Affine3::translation(Vec3::new(translate[0], translate[1], translate[2]));
    let positions = xf.transform_points(&mesh.positions());
    let faces = polygons_as_usize(mesh);
    Mesh::from_polygons(&positions, &faces)
}

/// Concatenate two meshes into one.
///
/// The first operand's vertices come first, then the second's; the second
/// operand's face indices are offset by the first's vertex count so they point
/// at the appended positions. No vertex welding is performed, so two disjoint
/// solids stay two separate closed surfaces in the result.
fn join_meshes(a: &Mesh, b: &Mesh) -> Mesh {
    let mut positions = a.positions();
    let offset = positions.len();
    positions.extend(b.positions());

    let mut faces = polygons_as_usize(a);
    for face in b.polygons() {
        faces.push(face.iter().map(|v| v.0 + offset).collect());
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Convert a mesh's polygon soup ([`Mesh::polygons`]) into the plain
/// `Vec<Vec<usize>>` face-index form [`Mesh::from_polygons`] consumes.
fn polygons_as_usize(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons()
        .iter()
        .map(|face| face.iter().map(|v| v.0).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph with no output node is rejected before any evaluation.
    ///
    /// Methodology: build a one-node graph (a lone `Primitive`) with no
    /// `OutputMesh` and evaluate. Result: `evaluate` returns
    /// [`ProceduralError::NoOutput`].
    #[test]
    fn graph_without_output_is_detected() {
        let mut g = GeometryGraph::new();
        g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        assert!(matches!(g.evaluate(), Err(ProceduralError::NoOutput)));
    }

    /// A minimal `Primitive(Cube) -> OutputMesh` graph evaluates to a cube.
    ///
    /// Methodology: wire a cube primitive (`size = 1.0`) straight into an
    /// output node and evaluate. Judged against the known cube topology from
    /// [`crate::primitives::cube`] (8 vertices, 6 quad faces, `chi = 2`).
    ///
    /// Results (exact topological identities, no tolerance): the evaluated mesh
    /// has V = 8, F = 6, and `chi = V - E + F = 2`, i.e. the evaluator returns
    /// the primitive unchanged through the output node.
    #[test]
    fn cube_through_output_evaluates_to_a_cube() {
        let mut g = GeometryGraph::new();
        let prim = g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        g.add(GeometryNode::OutputMesh { input: prim });

        let mesh = g.evaluate().expect("cube graph must evaluate");
        assert_eq!(mesh.vertex_count(), 8, "cube has 8 vertices");
        assert_eq!(mesh.face_count(), 6, "cube has 6 quad faces");
        assert_eq!(mesh.euler_characteristic(), 2, "closed cube has chi = 2");
    }

    /// Joining two cubes concatenates them into one mesh of both.
    ///
    /// Methodology: build cube A at the origin (`size = 1.0`, spans
    /// `-0.5..0.5` on each axis) and cube B translated by `[3, 0, 0]` (spans
    /// `2.5..3.5` on X), then `Join` them and output. Each cube contributes 8
    /// vertices and 6 faces, and `Join` welds nothing, so the result must carry
    /// both.
    ///
    /// Results: V = 16 and F = 12 (8 + 8, 6 + 6). Disjointness is checked from
    /// the position set — the combined X extent is `[-0.5, 3.5]` and no vertex
    /// lies in the open gap `(0.5, 2.5)`, so the two solids do not overlap. (A
    /// single cube would only span `[-0.5, 0.5]`, so seeing X = 3.5 confirms the
    /// translated cube B is present.) Note `chi = 4` here — two separate closed
    /// surfaces, `chi = 2` each — so it is intentionally not asserted.
    #[test]
    fn join_two_disjoint_cubes() {
        let mut g = GeometryGraph::new();
        let a = g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        let b_src = g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        let b = g.add(GeometryNode::Transform {
            input: b_src,
            translate: [3.0, 0.0, 0.0],
        });
        let joined = g.add(GeometryNode::Join { a, b });
        g.add(GeometryNode::OutputMesh { input: joined });

        let mesh = g.evaluate().expect("join graph must evaluate");
        assert_eq!(mesh.vertex_count(), 16, "two cubes => 16 vertices");
        assert_eq!(mesh.face_count(), 12, "two cubes => 12 faces");

        let xs: Vec<f64> = mesh.positions().iter().map(|p| p.x).collect();
        let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!((min_x - (-0.5)).abs() < 1e-12, "min X should be -0.5, got {min_x}");
        assert!((max_x - 3.5).abs() < 1e-12, "max X should be 3.5, got {max_x}");
        // Disjoint: no vertex sits in the gap between the two cubes.
        assert!(
            xs.iter().all(|&x| x <= 0.5 + 1e-12 || x >= 2.5 - 1e-12),
            "the two cubes must not overlap (gap 0.5..2.5 is empty)"
        );
    }

    /// One Catmull-Clark level on a cube produces the standard refined mesh.
    ///
    /// Methodology: `Primitive(Cube) -> Subdivide { levels: 1 } -> OutputMesh`,
    /// delegating to [`crate::subdivision::catmull_clark`]. Judged against the
    /// closed-form count for level-1 Catmull-Clark of a cube (6 quad faces, 8
    /// vertices, 12 edges):
    ///
    /// - new vertices = original 8 + one face point per face (6) + one edge
    ///   point per edge (12) = **26**;
    /// - new faces = every quad splits into 4 => `6 * 4` = **24**;
    /// - `chi = V - E + F` stays **2** (subdivision preserves topology), which
    ///   pins the edge count at `E = V + F - 2 = 48`.
    ///
    /// Results: V = 26, F = 24, `chi = 2`.
    #[test]
    fn subdivide_cube_one_level() {
        let mut g = GeometryGraph::new();
        let prim = g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        let sub = g.add(GeometryNode::Subdivide { input: prim, levels: 1 });
        g.add(GeometryNode::OutputMesh { input: sub });

        let mesh = g.evaluate().expect("subdivide graph must evaluate");
        assert_eq!(mesh.vertex_count(), 26, "Catmull-Clark L1 cube: 8 + 6 + 12 = 26 verts");
        assert_eq!(mesh.face_count(), 24, "Catmull-Clark L1 cube: 6 * 4 = 24 faces");
        assert_eq!(mesh.euler_characteristic(), 2, "subdivision preserves chi = 2");
    }

    /// An out-of-range [`NodeId`] edge is reported, not panicked on.
    ///
    /// Methodology: an `OutputMesh` whose `input` is `NodeId(99)` in a two-node
    /// graph. Result: `evaluate` returns [`ProceduralError::BadNode`] rather
    /// than indexing out of bounds.
    #[test]
    fn out_of_range_node_is_reported() {
        let mut g = GeometryGraph::new();
        g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        g.add(GeometryNode::OutputMesh { input: NodeId(99) });
        assert!(matches!(g.evaluate(), Err(ProceduralError::BadNode(_))));
    }

    /// A self-referential node is detected as a cycle, not an infinite loop.
    ///
    /// Methodology: node `NodeId(0)` is a `Transform` whose `input` is itself
    /// (`NodeId(0)`); an `OutputMesh` feeds from it. Walking upstream re-enters
    /// `NodeId(0)` while it is still on the DFS path. Result: `evaluate` returns
    /// [`ProceduralError::Cycle`] and terminates.
    #[test]
    fn self_cycle_is_detected() {
        let mut g = GeometryGraph::new();
        // First node (NodeId(0)) references itself.
        let t = g.add(GeometryNode::Transform {
            input: NodeId(0),
            translate: [1.0, 0.0, 0.0],
        });
        g.add(GeometryNode::OutputMesh { input: t });
        assert!(matches!(g.evaluate(), Err(ProceduralError::Cycle(_))));
    }
}
