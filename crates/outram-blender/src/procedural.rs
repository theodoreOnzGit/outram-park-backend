//! Procedural geometry generation — a **Geometry-Nodes-style** node graph
//! (Blender's `nodes/geometry`, the Geometry Nodes system).
//!
//! > **Scaffold: a sketch, not a working evaluator.** The node and graph types
//! > compile and model the *concept*; [`GeometryGraph::evaluate`] is a `TODO`
//! > stub that returns [`ProceduralError::NotImplemented`]. This exists to fix
//! > the vocabulary (nodes, sockets, a directed evaluation graph) so the real
//! > evaluator can be designed against it under epic `op-hzs`.
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
//! ## Why an enum of nodes
//!
//! The node kinds are a closed set, so [`GeometryNode`] is an enum (no trait
//! objects), and edges between nodes are **indices** ([`NodeId`]) into the
//! graph's node `Vec` — the same no-pointers/no-lifetimes discipline as
//! [`crate::mesh`].

use crate::mesh::Mesh;

/// Errors from evaluating a [`GeometryGraph`].
#[derive(Debug, thiserror::Error)]
pub enum ProceduralError {
    /// The node-graph evaluator is scaffolded but not implemented yet.
    #[error("procedural geometry evaluation not yet implemented: {0}")]
    NotImplemented(&'static str),
    /// The graph has no [`GeometryNode::OutputMesh`] node to read a result from.
    #[error("geometry graph has no output node")]
    NoOutput,
}

/// Index of a [`GeometryNode`] within a [`GeometryGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Which primitive an input node emits (parameters kept minimal for the sketch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveKind {
    /// A cube of the given side length — see [`crate::primitives::cube`].
    Cube {
        /// Full edge length.
        size: f64,
    },
    /// A UV sphere — see [`crate::primitives::uv_sphere`].
    UvSphere {
        /// Subdivisions around the polar axis.
        segments: usize,
        /// Subdivisions pole to pole.
        rings: usize,
        /// Sphere radius.
        radius: f64,
    },
}

/// A node in a procedural geometry graph.
///
/// Each node references its inputs by [`NodeId`]. Real evaluation would perform
/// a topological walk from [`GeometryNode::OutputMesh`] back to the inputs; the
/// scaffold only models the node shapes.
#[derive(Debug, Clone)]
pub enum GeometryNode {
    /// Source node: emit a primitive mesh.
    Primitive(PrimitiveKind),
    /// Transform the geometry coming from `input` by a uniform translation.
    Transform {
        /// Upstream node whose geometry is transformed.
        input: NodeId,
        /// Model-space translation applied to every vertex.
        translate: [f64; 3],
    },
    /// Join two geometry streams into one mesh (concatenate elements).
    Join {
        /// First upstream node.
        a: NodeId,
        /// Second upstream node.
        b: NodeId,
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
    /// **Not yet implemented.** Returns [`ProceduralError::NoOutput`] if there
    /// is no [`GeometryNode::OutputMesh`], otherwise
    /// [`ProceduralError::NotImplemented`] — the topological walk / per-node
    /// geometry evaluation is a future workstream (no fake-green).
    pub fn evaluate(&self) -> Result<Mesh, ProceduralError> {
        let has_output = self
            .nodes
            .iter()
            .any(|n| matches!(n, GeometryNode::OutputMesh { .. }));
        if !has_output {
            return Err(ProceduralError::NoOutput);
        }
        Err(ProceduralError::NotImplemented(
            "topological node-graph evaluation (Primitive/Transform/Join/Output)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_without_output_is_detected() {
        let mut g = GeometryGraph::new();
        g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        assert!(matches!(g.evaluate(), Err(ProceduralError::NoOutput)));
    }

    #[test]
    fn wired_graph_reports_not_implemented() {
        let mut g = GeometryGraph::new();
        let prim = g.add(GeometryNode::Primitive(PrimitiveKind::Cube { size: 1.0 }));
        g.add(GeometryNode::OutputMesh { input: prim });
        assert!(matches!(
            g.evaluate(),
            Err(ProceduralError::NotImplemented(_))
        ));
    }
}
