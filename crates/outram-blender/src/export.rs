//! Export bridges from an authored [`Mesh`] to the OUTRAM PARK solvers.
//!
//! > **Scaffold: honest `TODO` stubs.** These functions document the *target
//! > interfaces* of the two consumer crates and return
//! > [`ExportError::NotImplemented`]; no conversion is performed yet. The crate
//! > deliberately does **not** hard-depend on `outram-foam-mesh` or
//! > `outram-mc-libs` at scaffold stage — those crates are under active
//! > development by other fleets, and adding a path dependency now would create
//! > churn. The dependency is described by concept and wired later (tracked
//! > under epic `op-hzs`).
//!
//! ## Two very different targets
//!
//! An authored mesh here is a **boundary surface** (a shell of vertices, edges,
//! and polygon faces). The two solver targets consume geometry in fundamentally
//! different forms, so each bridge has real work beyond a format copy:
//!
//! ### 1. `outram-foam-mesh` polyMesh (CFD)
//!
//! OpenFOAM's `polyMesh` — mirrored by `outram_foam_basic_lib::mesh::FvMesh` and
//! the `io::poly_mesh` reader/writer — is a **finite-volume VOLUME mesh**:
//!
//! - `points` — vertex coordinates (metres);
//! - `faces` — each an ordered list of point indices;
//! - `owner[f]` / `neighbour[f]` — the two cells straddling each internal face
//!   (boundary faces have an owner only);
//! - boundary **patches** grouping the exterior faces by name.
//!
//! The gap: our surface mesh has faces but **no cells**. Turning a closed
//! surface into a volume mesh is the job of a mesher (blockMesh / snappyHexMesh
//! in `outram-foam-mesh`). The realistic bridge is therefore
//! [`to_polymesh_surface`], which emits our surface as a **boundary patch**
//! (points + faces + owner, no neighbour) that a volume mesher then fills — not
//! a ready-to-solve volume mesh. This distinction is documented, not hidden.
//!
//! ### 2. `outram-mc-libs` CSG (Monte Carlo transport)
//!
//! `outram_mc_libs`'s geometry is **constructive solid geometry**: analytic
//! `SurfaceKind` primitives (`XPlane`/`YPlane`/`ZPlane`, `Sphere`, `ZCylinder`,
//! …) combined by an RPN `region: Vec<RegionToken>` of signed half-spaces into
//! `Cell`s. A triangulated boundary mesh does **not** map onto analytic
//! surfaces directly. Two honest routes (both future work):
//!
//! - **Primitive fitting** — recognise that a mesh *came from* a
//!   [`crate::primitives`] generator (a `cube`, `cylinder`, `uv_sphere`) and
//!   emit the matching analytic CSG surfaces + region. This is exact and is the
//!   near-term plan for reactor geometry (pins, cans, reflectors are
//!   primitives). See [`to_csg_primitive`].
//! - **Faceted surfaces** — treat each triangle as a bounded plane half-space
//!   (a DAGMC-style mesh-in-CSG). Heavier; deferred.
//!
//! ## No dependency yet — plain intermediate data
//!
//! Until the bridges are wired, [`triangulate`] provides a dependency-free,
//! indexed triangle soup ([`IndexedTriangles`]) — the common denominator both
//! targets (and any OBJ/STL exporter) build on. It is fully implemented and
//! tested, so downstream code has something real to consume today.

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Errors from an export bridge.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The bridge to a solver mesh format is scaffolded but not implemented.
    #[error("export bridge not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// A dependency-free, flattened triangle mesh: the common export denominator.
///
/// Every polygon face of a [`Mesh`] is fan-triangulated into this indexed form
/// (`positions` + triangle `indices` triplets). This is what an OBJ/STL writer,
/// a polyMesh boundary patch, or a faceted-CSG surface would each build from —
/// so it is implemented and tested even while the solver bridges are stubs.
#[derive(Debug, Clone, Default)]
pub struct IndexedTriangles {
    /// Vertex positions, indexed by the entries of [`IndexedTriangles::indices`].
    pub positions: Vec<Vec3>,
    /// Flat list of triangle corner indices, three consecutive entries per
    /// triangle, each indexing into [`IndexedTriangles::positions`].
    pub indices: Vec<u32>,
}

impl IndexedTriangles {
    /// Number of triangles (== `indices.len() / 3`).
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Fan-triangulate every face of `mesh` into an [`IndexedTriangles`] soup.
///
/// Each `n`-gon face contributes `n - 2` triangles by a simple fan from its
/// first vertex — valid for the convex faces the [`crate::primitives`]
/// generators produce. Positions are copied 1:1 from the mesh (indices are
/// preserved), so `positions.len()` equals `mesh.vertex_count()`.
///
/// This is real, tested code — the dependency-free foundation the solver
/// bridges below will build on.
pub fn triangulate(mesh: &Mesh) -> IndexedTriangles {
    let mut positions = Vec::with_capacity(mesh.vertex_count());
    for i in 0..mesh.vertex_count() {
        // vertex(i) is Some for every i < vertex_count.
        positions.push(mesh.vertex(crate::mesh::VertexId(i)).unwrap().position);
    }

    let mut indices = Vec::new();
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(crate::mesh::FaceId(f));
        if vs.len() < 3 {
            continue;
        }
        // Fan from the first corner: (v0, v1, v2), (v0, v2, v3), …
        for k in 1..(vs.len() - 1) {
            indices.push(vs[0].0 as u32);
            indices.push(vs[k].0 as u32);
            indices.push(vs[k + 1].0 as u32);
        }
    }
    IndexedTriangles { positions, indices }
}

/// **Stub.** Emit `mesh` as an `outram-foam-mesh` polyMesh **boundary patch**.
///
/// Target: `outram_foam_basic_lib::mesh::FvMesh` / `io::poly_mesh` — points,
/// faces, `owner`, and a boundary patch (no `neighbour`, since a surface has no
/// cells; a volume mesher fills the interior). Returns
/// [`ExportError::NotImplemented`]; the return type is `()` as a placeholder so
/// this crate need not yet depend on `outram-foam-mesh`. See the module docs
/// for why a surface is not a ready-to-solve volume mesh.
pub fn to_polymesh_surface(_mesh: &Mesh) -> Result<(), ExportError> {
    Err(ExportError::NotImplemented(
        "outram-foam-mesh polyMesh boundary patch (points/faces/owner + patch)",
    ))
}

/// **Stub.** Emit `mesh` as `outram-mc-libs` CSG by primitive fitting.
///
/// Target: `outram_mc_libs`'s `SurfaceKind` analytic surfaces + a `Cell` whose
/// `region: Vec<RegionToken>` is the RPN combination of signed half-spaces.
/// Intended near-term route: recognise a [`crate::primitives`]-generated shape
/// and emit its exact analytic CSG (a `cube` → six planes intersected, a
/// `cylinder` → a `ZCylinder` capped by two `ZPlane`s, a `uv_sphere` → a
/// `Sphere`). Returns [`ExportError::NotImplemented`]; `()` placeholder return
/// avoids a dependency on `outram-mc-libs` at scaffold stage.
pub fn to_csg_primitive(_mesh: &Mesh) -> Result<(), ExportError> {
    Err(ExportError::NotImplemented(
        "outram-mc-libs CSG by primitive fitting (SurfaceKind + RegionToken RPN)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn triangulate_cube_gives_12_triangles() {
        // A cube has 6 quads; fan-triangulating each quad yields 2 triangles.
        let tris = triangulate(&primitives::cube(1.0));
        assert_eq!(tris.positions.len(), 8);
        assert_eq!(tris.triangle_count(), 12);
        // Every index is in range.
        assert!(tris.indices.iter().all(|&i| (i as usize) < tris.positions.len()));
    }

    #[test]
    fn solver_bridges_report_not_implemented() {
        let m = primitives::cube(1.0);
        assert!(matches!(
            to_polymesh_surface(&m),
            Err(ExportError::NotImplemented(_))
        ));
        assert!(matches!(
            to_csg_primitive(&m),
            Err(ExportError::NotImplemented(_))
        ));
    }
}
