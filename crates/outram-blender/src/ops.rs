//! Mesh **operators** — the editing verbs (Blender's `bmesh/operators`, `bmo_*`).
//!
//! > **Scaffold: every operator here is an honest `TODO` stub.** The enum,
//! > variants, and dispatch are real and compile; the geometry is not
//! > implemented yet and each [`MeshOp::apply`] arm returns
//! > [`MeshOpError::NotImplemented`]. This module fixes the *shape* of the
//! > operator API so the rest of the crate (and its beads) can be built around
//! > it; the algorithms are separate workstreams tracked under epic `op-hzs`.
//!
//! ## Why an enum, not trait objects
//!
//! The operator set is closed and known at compile time, so per the workspace
//! design rules it is a single [`MeshOp`] enum dispatched by `match` — not
//! `Box<dyn Trait>`. Adding a new operator forces every `match` site to handle
//! it (exhaustiveness), and rust-analyzer's go-to-definition works on each
//! variant.
//!
//! ## The operators (Blender analogue in parentheses)
//!
//! - [`MeshOp::Extrude`] — push faces/edges/verts out along a direction
//!   (`bmo_extrude`, the Extrude Region tool).
//! - [`MeshOp::Subdivide`] — Catmull-Clark subdivision surface refinement
//!   (`bmo_subdivide` / the OpenSubdiv path; see [`crate::modifiers`] for the
//!   non-destructive Subsurf modifier).
//! - [`MeshOp::Bevel`] — round or chamfer edges/vertices (`bmo_bevel`).
//! - [`MeshOp::Boolean`] — CSG union/difference/intersection of two meshes
//!   (`bmo_boolean`, backed upstream by the Manifold library). This is the
//!   operator most relevant to feeding `outram-mc-libs` CSG geometry.

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Errors returned by [`MeshOp::apply`].
#[derive(Debug, thiserror::Error)]
pub enum MeshOpError {
    /// The operator is scaffolded but its algorithm is not implemented yet.
    #[error("mesh operator not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// Boolean CSG mode for [`MeshOp::Boolean`] (mirrors Blender's Boolean modifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanMode {
    /// Keep the volume in either mesh (A ∪ B).
    Union,
    /// Keep the volume of A outside B (A \ B).
    Difference,
    /// Keep the volume common to both (A ∩ B).
    Intersect,
}

/// A closed set of mesh-editing operators.
///
/// Construct a variant with its parameters, then call [`MeshOp::apply`] on a
/// mesh. Every variant currently returns [`MeshOpError::NotImplemented`] — the
/// parameters are captured by value (no borrows) so the API is stable for when
/// the algorithms land.
#[derive(Debug, Clone)]
pub enum MeshOp {
    /// Extrude the whole mesh's faces along `offset` (direction and distance
    /// combined). Real extrude will select a face region; the scaffold captures
    /// only the offset.
    Extrude {
        /// Model-space translation applied to the newly created geometry.
        offset: Vec3,
    },
    /// Apply `iterations` rounds of Catmull-Clark subdivision.
    Subdivide {
        /// Number of refinement passes (each roughly quadruples face count).
        iterations: u32,
    },
    /// Bevel every edge by `width` model-space units into `segments` pieces.
    Bevel {
        /// Bevel width in model-space units.
        width: f64,
        /// Number of segments across the bevel (1 = a single chamfer).
        segments: u32,
    },
    /// Combine `self`'s target mesh with `other` under a [`BooleanMode`].
    Boolean {
        /// The second operand mesh (owned by value — no lifetimes).
        other: Mesh,
        /// Union / difference / intersection.
        mode: BooleanMode,
    },
}

impl MeshOp {
    /// Apply this operator to `mesh`, returning the edited mesh.
    ///
    /// **Not yet implemented.** Every variant returns
    /// [`MeshOpError::NotImplemented`] naming the operator; the `match` is
    /// exhaustive so a future implementer sees exactly which arms remain. The
    /// `mesh` argument is consumed and (once implemented) the result returned,
    /// matching Blender's destructive-operator model.
    pub fn apply(&self, _mesh: Mesh) -> Result<Mesh, MeshOpError> {
        match self {
            MeshOp::Extrude { .. } => Err(MeshOpError::NotImplemented(
                "extrude (bmo_extrude): create side faces + offset the region",
            )),
            MeshOp::Subdivide { .. } => Err(MeshOpError::NotImplemented(
                "Catmull-Clark subdivide: face points, edge points, vertex smoothing",
            )),
            MeshOp::Bevel { .. } => Err(MeshOpError::NotImplemented(
                "bevel (bmo_bevel): inset edges and build chamfer segments",
            )),
            MeshOp::Boolean { .. } => Err(MeshOpError::NotImplemented(
                "boolean (bmo_boolean / Manifold): CSG union/difference/intersect",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// The scaffold contract: operators dispatch and report NotImplemented
    /// rather than silently returning a wrong mesh (no fake-green).
    #[test]
    fn operators_report_not_implemented() {
        let m = primitives::cube(1.0);
        let op = MeshOp::Subdivide { iterations: 1 };
        assert!(matches!(op.apply(m), Err(MeshOpError::NotImplemented(_))));
    }
}
