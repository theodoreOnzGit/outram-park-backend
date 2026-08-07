// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! **Volume-meshing bridge** (feature `foam-mesh`) — blender surface `Mesh` →
//! `outram-park-fork-cfmesh` tet→dual→boundary-layers pipeline → OpenFOAM
//! `polyMesh`.
//!
//! This module is the seam between the two crates. `outram-blender` authors a
//! *surface* (a closed, watertight, outward-wound [`crate::mesh::Mesh`]);
//! `outram-park-fork-cfmesh` turns that surface into a solvable *volume* mesh —
//! a hex-carve that is body-fit-snapped, tetrahedralized, dualised into
//! polyhedral cells, and grown near-wall prism boundary layers — then writes it
//! out as an OpenFOAM `polyMesh`. Nothing here re-implements meshing; it only
//! converts data between the two representations and calls the backend.
//!
//! ```text
//!   blender Mesh   triangulate + convert    cfmesh pipeline        foam::write_polymesh
//!   (surface)  ──► (points + tri index) ──► (tet → dual → layers) ──► polyMesh on disk
//! ```
//!
//! # What belongs here / what does not
//!
//! **Belongs:** the `Mesh → (points, triangles)` conversion (via
//! [`crate::triangulate`], so an arbitrary-polygon surface is accepted, not just
//! triangles), the coordinate-type bridge (blender [`crate::math::Vec3`] →
//! cfmesh [`Vec3`]), and thin wrappers over the backend's pipeline entry points
//! ([`mesh_to_tet_dual`], the primitive passthroughs, [`export_polymesh`]).
//!
//! **Does not belong:** any geometry algorithm. Carving, snapping,
//! tetrahedralization, the polyhedral dual, boundary-layer insertion, quality
//! checks and the polyMesh writer all live in `outram-park-fork-cfmesh`. If you
//! reach for a new mesh operation here, it belongs in that crate instead.
//!
//! # Units and conventions
//!
//! All lengths are **metres** — surface coordinates, [`TetDualOptions::cell_size`],
//! and [`TetDualOptions::first_layer_thickness`]. Angles in [`TetDualReport`] are
//! **degrees**; volume is **cubic metres**; [`TetDualOptions::expansion`] is
//! dimensionless (`>= 1`). Blender's [`crate::math::Vec3`] is nominally
//! dimensionless model space (see [`crate::math`]); this bridge treats one model
//! unit as one metre, matching the convention the cfmesh pipeline and its
//! built-in primitives use.
//!
//! # Surface requirements — checked, not assumed
//!
//! The input surface must be **closed, watertight, and consistently outward-wound**
//! (outward normals), exactly as the [`crate::primitives`] generators produce.
//! Fan-triangulation ([`crate::triangulate::triangulate`]) preserves winding, so
//! an outward-wound polygon mesh stays outward-wound.
//!
//! **This requirement is now enforced at the seam.** The backend's carve stage
//! decides which background cells are interior with a **ray-parity** test
//! (`outram_park_fork_cfmesh::carve::inside` — cast a ray, count triangle
//! crossings, odd ⇒ inside). Ray parity is only meaningful on a closed surface:
//! a single missing or duplicated triangle flips the parity of an entire ray
//! corridor, so a leaky surface yields a *silently* corrupt volume mesh that
//! still passes the backend's own `validate()` (which checks cell closure, not
//! whether the cells are the right ones). To make that failure loud instead of
//! silent, [`mesh_to_surface`] returns a [`SurfaceError`] — and
//! [`mesh_to_tet_dual`] propagates it — when the triangulated surface is not a
//! closed, consistently-wound 2-manifold. See [`check_closed_manifold`].
//!
//! **Measured evidence for the gate (2026-08-07).** A flat 2x2
//! [`crate::primitives::grid`] — an *open* patch enclosing exactly zero volume —
//! was handed straight to the backend, as this bridge used to do. It returned
//! **`Ok`**: `report.valid == true`, `cell_count == 618`,
//! `n_negative_volume_cells == 0`, `mesh.validate().is_ok()`, and
//! `total_volume == 0.864 m³`. Nothing flagged the input. With the gate,
//! [`mesh_to_tet_dual`] now returns `Err` naming the unpaired edge before any
//! meshing work starts.
//!
//! # Scope and trust
//!
//! **Untrusted AI-assisted draft pending human V&V.** The verification here is of
//! mesh *topology* (closed cells, no inverted cells) and volume conservation
//! against the analytic volume of the built-in primitives — **not** validation
//! against a CFD/TH solve. Offline demonstration only, per the workspace
//! `RESPONSIBLE_USE.md`: not for reactor operation, licensing, or safety-critical
//! decisions.

use std::collections::HashMap;
use std::path::Path;

use crate::mesh::Mesh;

// Re-export the backend types callers need, so a user of this bridge does not
// have to name the `outram-park-fork-cfmesh` crate directly for the common path.
pub use outram_park_fork_cfmesh::math::Vec3;
pub use outram_park_fork_cfmesh::pipeline::{TetDualOptions, TetDualReport};
pub use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;

/// Why an authored surface cannot be handed to the volume mesher.
///
/// Every variant names the offending geometry — the source polygon face, the
/// triangle it fan-triangulated to, and the vertex indices involved — so a
/// failure points at a specific face of the authored mesh rather than at "the
/// mesh". These are all conditions under which the backend's **ray-parity**
/// inside test is ill-defined (see the module docs), so continuing would produce
/// a silently wrong volume mesh.
///
/// Indices are `usize` positions: `face` indexes [`crate::mesh::FaceId`]s of the
/// input mesh, `tri` indexes the emitted triangle list, and `a`/`b`/`c` index the
/// emitted point list. No units — these are pure topology.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SurfaceError {
    /// The mesh produced no triangles at all (no faces, or every face was
    /// sub-triangular). There is nothing to carve.
    #[error(
        "surface has no triangles to mesh: the input mesh has {face_count} face(s) and \
         produced 0 triangles — author a closed surface before volume meshing"
    )]
    Empty {
        /// Number of faces the input mesh had.
        face_count: usize,
    },

    /// A polygon face has fewer than three corners, so it cannot be
    /// triangulated. Previously such a face was **silently dropped**, which
    /// punched a hole in the exported surface.
    ///
    /// A [`Mesh`] built through [`Mesh::add_face`] cannot hold such a face (that
    /// constructor asserts `>= 3` corners), so this is a defence against a
    /// future constructor that relaxes the invariant — not a condition reachable
    /// through today's public API.
    #[error(
        "face {face} has only {corners} corner(s) and cannot be triangulated; \
         dropping it would leave a hole in the exported surface and make the \
         backend's ray-parity inside test meaningless"
    )]
    DegenerateFace {
        /// [`crate::mesh::FaceId`] index of the offending face.
        face: usize,
        /// How many corners that face actually had (`< 3`).
        corners: usize,
    },

    /// A fan-triangulated triangle repeats a corner vertex (e.g. a polygon whose
    /// vertex ring contains the same [`crate::mesh::VertexId`] twice). Such a
    /// triangle has no well-defined normal and contributes a self-edge that can
    /// never be paired, so the surface cannot be closed.
    #[error(
        "face {face} triangulated to triangle {tri} = [{a}, {b}, {c}], which repeats a \
         corner vertex; the surface cannot be a closed 2-manifold"
    )]
    RepeatedCorner {
        /// [`crate::mesh::FaceId`] index of the source face.
        face: usize,
        /// Index into the emitted triangle list.
        tri: usize,
        /// First corner vertex index.
        a: usize,
        /// Second corner vertex index.
        b: usize,
        /// Third corner vertex index.
        c: usize,
    },

    /// A fan-triangulated triangle has (numerically) zero area — three distinct
    /// but collinear corners. Its normal is undefined, so surface snapping and
    /// ray/triangle intersection both degenerate on it.
    ///
    /// The threshold is relative: `area <= 1e-12 * extent²`, where `extent` is
    /// the surface bounding-box diagonal (metres). That is an "exactly zero to
    /// `f64`" bar, not a sliver-quality bar — a thin-but-real triangle passes.
    #[error(
        "face {face} triangulated to triangle {tri} = [{a}, {b}, {c}] with area {area:.3e} m², \
         which is degenerate (collinear corners); its normal is undefined"
    )]
    ZeroAreaTriangle {
        /// [`crate::mesh::FaceId`] index of the source face.
        face: usize,
        /// Index into the emitted triangle list.
        tri: usize,
        /// First corner vertex index.
        a: usize,
        /// Second corner vertex index.
        b: usize,
        /// Third corner vertex index.
        c: usize,
        /// Measured triangle area, in square metres.
        area: f64,
    },

    /// A triangle corner indexes past the end of the point list.
    #[error(
        "triangle {tri} (from face {face}) references point index {index}, but the surface \
         has only {n_points} point(s)"
    )]
    IndexOutOfRange {
        /// [`crate::mesh::FaceId`] index of the source face.
        face: usize,
        /// Index into the emitted triangle list.
        tri: usize,
        /// The out-of-range corner index.
        index: usize,
        /// Number of points available.
        n_points: usize,
    },

    /// The surface is **not closed**: the directed half-edge `a -> b` exists but
    /// its opposite `b -> a` does not, so the undirected edge `{a, b}` is used by
    /// exactly one triangle instead of two. The surface has a boundary (a hole),
    /// and a ray crossing that hole gets the wrong parity.
    #[error(
        "surface is not watertight: edge ({a} -> {b}) of triangle {tri} (from face {face}) \
         has no opposing triangle, so that edge is shared by 1 triangle, not 2 — the \
         backend's ray-parity inside test would silently mis-classify cells"
    )]
    NotClosed {
        /// [`crate::mesh::FaceId`] index of the source face.
        face: usize,
        /// Index into the emitted triangle list.
        tri: usize,
        /// Start vertex of the unpaired directed edge.
        a: usize,
        /// End vertex of the unpaired directed edge.
        b: usize,
    },

    /// Two triangles traverse the same edge in the **same** direction. Either the
    /// surface is non-manifold (three or more triangles on one edge), the two
    /// triangles are wound inconsistently, or a triangle is duplicated. In every
    /// case the edge is not shared by exactly two oppositely-wound triangles.
    #[error(
        "surface winding is inconsistent or non-manifold: directed edge ({a} -> {b}) is \
         traversed by both triangle {tri} (from face {face}) and triangle {other_tri} \
         (from face {other_face}); an orientable closed surface traverses each directed \
         edge exactly once"
    )]
    InconsistentWinding {
        /// [`crate::mesh::FaceId`] index of the source face of `tri`.
        face: usize,
        /// Index into the emitted triangle list.
        tri: usize,
        /// [`crate::mesh::FaceId`] index of the source face of `other_tri`.
        other_face: usize,
        /// The earlier triangle that already traversed this directed edge.
        other_tri: usize,
        /// Start vertex of the doubly-traversed directed edge.
        a: usize,
        /// End vertex of the doubly-traversed directed edge.
        b: usize,
    },
}

/// Relative area floor below which a triangle counts as degenerate.
///
/// Multiplied by the squared bounding-box diagonal of the surface, so it scales
/// with the model and stays an "exactly zero to `f64`" test rather than a
/// sliver-quality gate.
const DEGENERATE_AREA_REL: f64 = 1e-12;

/// Convert a blender surface [`Mesh`] into the cfmesh surface representation:
/// a flat point list plus triangle vertex-index triples — **validating** that the
/// result is a closed, consistently-wound 2-manifold.
///
/// Every polygon face is fan-triangulated from its first corner (an `n`-gon
/// becomes `n - 2` triangles), so quads and `n`-gons are accepted, not only
/// triangle meshes; winding is preserved. Each blender [`crate::math::Vec3`] is
/// copied component-wise into a cfmesh [`Vec3`] (one model unit → one metre).
/// The returned data is exactly the `(points, tris)` pair
/// [`surface_to_tet_dual_mesh`](outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh)
/// expects. `points` has one entry per mesh vertex in [`crate::mesh::VertexId`]
/// order, including any vertex no face uses.
///
/// This is equivalent to [`crate::triangulate::triangulate`] followed by an
/// index extraction, done inline so each emitted triangle keeps the
/// [`crate::mesh::FaceId`] it came from and a diagnostic can name it.
///
/// # Errors
///
/// Returns a [`SurfaceError`] naming the offending face when the surface is
/// empty, has a sub-triangular or degenerate face, indexes out of range, is not
/// watertight, or is inconsistently wound / non-manifold. **Nothing is silently
/// dropped** — an earlier version filtered non-triangles out of the output, which
/// could hand the backend a leaky surface whose ray-parity inside test is
/// meaningless (see the module docs).
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, foam_mesh};
///
/// // A closed primitive converts cleanly: 8 corners, 6 quads → 12 triangles.
/// let (points, tris) = foam_mesh::mesh_to_surface(&primitives::cube(2.0)).unwrap();
/// assert_eq!(points.len(), 8);
/// assert_eq!(tris.len(), 12);
///
/// // An open surface is rejected rather than silently exported leaky.
/// let grid = primitives::grid(2, 2, 1.0);
/// assert!(foam_mesh::mesh_to_surface(&grid).is_err());
/// ```
pub fn mesh_to_surface(mesh: &Mesh) -> Result<(Vec<Vec3>, Vec<[usize; 3]>), SurfaceError> {
    let points: Vec<Vec3> =
        mesh.positions().iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect();

    let mut tris: Vec<[usize; 3]> = Vec::new();
    // Parallel to `tris`: the FaceId each triangle was fanned from, so any
    // diagnostic below can name the authored face rather than a triangle index.
    let mut tri_face: Vec<usize> = Vec::new();

    for (fi, poly) in mesh.polygons().iter().enumerate() {
        let k = poly.len();
        if k < 3 {
            return Err(SurfaceError::DegenerateFace { face: fi, corners: k });
        }
        // Fan from the first corner: (v0, v1, v2), (v0, v2, v3), …
        for i in 1..k - 1 {
            tris.push([poly[0].0, poly[i].0, poly[i + 1].0]);
            tri_face.push(fi);
        }
    }

    if tris.is_empty() {
        return Err(SurfaceError::Empty { face_count: mesh.face_count() });
    }

    check_surface(&points, &tris, &tri_face)?;
    Ok((points, tris))
}

/// Verify that a triangle soup is a **closed, consistently-wound 2-manifold**:
/// every undirected edge is shared by exactly two triangles, traversed once in
/// each direction.
///
/// This is the precondition the backend's ray-parity inside test needs (see the
/// module docs). It also rejects out-of-range indices, repeated corners, and
/// zero-area (collinear) triangles, whose normals are undefined.
///
/// `points` are in metres; `tris` are corner-index triples into `points`. The
/// check is `O(3 * n_tris)` time and memory in a hash map of directed edges, and
/// is deterministic: it reports the first failure in triangle order.
///
/// # Errors
///
/// Returns the [`SurfaceError`] describing the first violation found. When called
/// on a bare triangle soup like this, the `face` field of the error equals the
/// triangle index (there is no polygon provenance to report); when the check runs
/// inside [`mesh_to_surface`], `face` is the real [`crate::mesh::FaceId`].
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, foam_mesh};
///
/// let (points, tris) = foam_mesh::mesh_to_surface(&primitives::cube(2.0)).unwrap();
/// assert!(foam_mesh::check_closed_manifold(&points, &tris).is_ok());
///
/// // Delete one triangle and the surface is no longer closed.
/// let mut leaky = tris.clone();
/// leaky.pop();
/// assert!(foam_mesh::check_closed_manifold(&points, &leaky).is_err());
/// ```
pub fn check_closed_manifold(points: &[Vec3], tris: &[[usize; 3]]) -> Result<(), SurfaceError> {
    // No polygon provenance available: each triangle stands for itself.
    let identity: Vec<usize> = (0..tris.len()).collect();
    check_surface(points, tris, &identity)
}

/// Shared implementation of [`check_closed_manifold`], carrying the
/// triangle → source-face map so errors can name the authored face.
///
/// `tri_face[i]` is the face index triangle `i` came from; it must have the same
/// length as `tris`.
fn check_surface(
    points: &[Vec3],
    tris: &[[usize; 3]],
    tri_face: &[usize],
) -> Result<(), SurfaceError> {
    debug_assert_eq!(tris.len(), tri_face.len(), "triangle/face maps must align");
    let face_of = |ti: usize| tri_face.get(ti).copied().unwrap_or(ti);

    // --- Bounding-box diagonal, for the scale-relative degeneracy threshold. --
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in points {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let diag = hi.sub(lo);
    let extent2 = diag.dot(diag);
    let area_floor = DEGENERATE_AREA_REL * extent2;

    // --- Per-triangle validity: in range, distinct corners, non-zero area. ----
    for (ti, t) in tris.iter().enumerate() {
        for &idx in t {
            if idx >= points.len() {
                return Err(SurfaceError::IndexOutOfRange {
                    face: face_of(ti),
                    tri: ti,
                    index: idx,
                    n_points: points.len(),
                });
            }
        }
        if t[0] == t[1] || t[1] == t[2] || t[2] == t[0] {
            return Err(SurfaceError::RepeatedCorner {
                face: face_of(ti),
                tri: ti,
                a: t[0],
                b: t[1],
                c: t[2],
            });
        }
        let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
        let area = 0.5 * b.sub(a).cross(c.sub(a)).length();
        if area <= area_floor {
            return Err(SurfaceError::ZeroAreaTriangle {
                face: face_of(ti),
                tri: ti,
                a: t[0],
                b: t[1],
                c: t[2],
                area,
            });
        }
    }

    // --- Orientability / manifoldness: each directed edge traversed once. -----
    let mut half: HashMap<(usize, usize), usize> = HashMap::with_capacity(3 * tris.len());
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            if let Some(&other) = half.get(&(a, b)) {
                return Err(SurfaceError::InconsistentWinding {
                    face: face_of(ti),
                    tri: ti,
                    other_face: face_of(other),
                    other_tri: other,
                    a,
                    b,
                });
            }
            half.insert((a, b), ti);
        }
    }

    // --- Closure: every directed edge must have its opposite present. ---------
    // Iterating `tris` (not the map) keeps the reported failure deterministic.
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            if !half.contains_key(&(b, a)) {
                return Err(SurfaceError::NotClosed { face: face_of(ti), tri: ti, a, b });
            }
        }
    }

    Ok(())
}

/// Volume-mesh a blender surface [`Mesh`] via the cfmesh tet→dual→layers pipeline.
///
/// Triangulates the surface, converts it to cfmesh coordinates, and calls
/// [`surface_to_tet_dual_mesh`](outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh).
/// The returned [`VolumeMesh`] is always **valid** (closed cells, in-range
/// addressing) and free of negative-volume cells — the pipeline gates every
/// optional stage and errors out rather than returning a broken mesh. The
/// [`TetDualReport`] carries the cell count, total volume (m³), max
/// non-orthogonality (deg) and skewness, negative-cell count, and one
/// `stage_notes` line per stage that was skipped to keep the mesh valid.
///
/// # Parameters
///
/// - `mesh` — a closed, watertight, outward-wound surface (metres).
/// - `opts` — [`TetDualOptions`]: background `cell_size` (m), which stages to run
///   (`snap` / `delaunay` / `dual`), boundary-layer spec (`n_layers`,
///   `first_layer_thickness` in m, `expansion`), and the `wall_patch` name.
///
/// # Errors
///
/// Returns `Err(msg)` in two families, both stringified for uniform handling in
/// the GUI:
///
/// - **Rejected input** — the [`SurfaceError`] from [`mesh_to_surface`], raised
///   *before* any meshing work, when the authored surface is empty, degenerate,
///   not watertight, or inconsistently wound. The message names the offending
///   [`crate::mesh::FaceId`]. This is a guard, not a nicety: the backend carve
///   classifies cells by ray parity, which silently mis-classifies on a leaky
///   surface (see the module docs).
/// - **Meshing failure** — `cell_size <= 0`, the carve produced zero cells (cell
///   size too large for the geometry), or — should not happen — the final mesh is
///   not closed.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, foam_mesh};
/// use outram_blender::foam_mesh::TetDualOptions;
///
/// let cube = primitives::cube(2.0); // a 2 m cube, volume 8 m³
/// let opts = TetDualOptions { cell_size: 0.5, first_layer_thickness: 0.02, ..Default::default() };
/// let (vol_mesh, report) = foam_mesh::mesh_to_tet_dual(&cube, &opts).unwrap();
///
/// assert!(report.valid);
/// assert_eq!(report.n_negative_volume_cells, 0);
/// // A box survives every stage exactly: volume conserved to 8 m³.
/// assert!((report.total_volume - 8.0).abs() < 1e-6);
/// ```
pub fn mesh_to_tet_dual(
    mesh: &Mesh,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    let (points, tris) = mesh_to_surface(mesh).map_err(|e| e.to_string())?;
    outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh(&points, &tris, opts)
}

/// Write a generated [`VolumeMesh`] to an OpenFOAM `polyMesh` directory.
///
/// Thin wrapper over
/// [`foam::write_polymesh`](outram_park_fork_cfmesh::foam::write_polymesh) that
/// maps its I/O error to a `String` for uniform handling in the GUI. `dir` is the
/// target `polyMesh` directory (e.g. `<case>/constant/polyMesh`); it is created
/// if absent, and the standard `points` / `faces` / `owner` / `neighbour` /
/// `boundary` files are written there.
///
/// # Errors
///
/// Returns `Err(msg)` if the directory cannot be created or a file cannot be
/// written.
pub fn export_polymesh(mesh: &VolumeMesh, dir: &Path) -> Result<(), String> {
    outram_park_fork_cfmesh::foam::write_polymesh(mesh, dir).map_err(|e| e.to_string())
}

/// Convenience passthrough: tet-dual mesh of an axis-aligned **box** `[min, max]`
/// (metres). See [`box_tet_dual`](outram_park_fork_cfmesh::pipeline::box_tet_dual).
pub fn box_tet_dual(
    min: Vec3,
    max: Vec3,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    outram_park_fork_cfmesh::pipeline::box_tet_dual(min, max, opts)
}

/// Convenience passthrough: tet-dual mesh of a **sphere** of `radius` (metres)
/// about `centre`, triangulated with `n_lat` × `n_lon` bands. See
/// [`sphere_tet_dual`](outram_park_fork_cfmesh::pipeline::sphere_tet_dual).
pub fn sphere_tet_dual(
    centre: Vec3,
    radius: f64,
    n_lat: usize,
    n_lon: usize,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    outram_park_fork_cfmesh::pipeline::sphere_tet_dual(centre, radius, n_lat, n_lon, opts)
}

/// Convenience passthrough: tet-dual mesh of a **cylinder** of `radius`×`height`
/// (metres) from `base`, `n_seg` circumferential segments. See
/// [`cylinder_tet_dual`](outram_park_fork_cfmesh::pipeline::cylinder_tet_dual).
pub fn cylinder_tet_dual(
    base: Vec3,
    radius: f64,
    height: f64,
    n_seg: usize,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    outram_park_fork_cfmesh::pipeline::cylinder_tet_dual(base, radius, height, n_seg, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// V&V — headline (verification only; measured 2026-08-03 on cfmesh 0.0.0).
    ///
    /// **Methodology:** author a 2 m cube in outram-blender
    /// ([`primitives::cube`]), bridge it through [`mesh_to_tet_dual`] with
    /// `cell_size = 0.5 m` (so the interior carves ~4³ background cells before
    /// tet+dual), default stages (snap + Delaunay + face-minimal dual + 3 prism
    /// layers). **Pass criterion:** the pipeline returns a valid mesh
    /// (`report.valid`), zero negative-volume cells, volume conserved to the
    /// analytic 8 m³ (a flat-walled box survives every stage exactly), and a
    /// cell count in the hundreds–thousands order of magnitude (tet+dual+layers
    /// multiply the coarse carve).
    ///
    /// **Result:** valid == true, 0 negative cells, |V − 8| < 1e-6 m³,
    /// cell_count in [50, 100_000]. Interpretation: the blender→cfmesh surface
    /// conversion and the pipeline call are wired correctly and preserve the
    /// authored geometry's volume. This is verification of topology + volume, not
    /// validation against a CFD solve.
    #[test]
    fn cube_bridges_to_valid_tet_dual_mesh() {
        let cube = primitives::cube(2.0);
        let opts = TetDualOptions {
            cell_size: 0.5,
            first_layer_thickness: 0.02,
            ..Default::default()
        };
        let (mesh, report) = mesh_to_tet_dual(&cube, &opts).expect("cube meshes");

        assert!(mesh.validate().is_ok(), "final mesh must be closed");
        assert!(report.valid, "report must flag the mesh valid");
        assert_eq!(report.n_negative_volume_cells, 0, "no inverted cells");
        assert!(
            (report.total_volume - 8.0).abs() < 1e-6,
            "box volume conserved to 8 m³, got {}",
            report.total_volume
        );
        // Order-of-magnitude sanity: a ~4³ carve, tetrahedralised and dualised
        // with 3 boundary layers, lands well inside this wide band.
        assert!(
            (50..=100_000).contains(&report.cell_count),
            "cell count out of expected order of magnitude: {}",
            report.cell_count
        );
    }

    /// V&V — a UV-sphere (curved wall) authored in outram-blender still yields a
    /// valid, inverted-cell-free mesh through the bridge, exercising the
    /// graceful-degradation stages (a coarse snap / dual / layer that would
    /// tangle a curved wall is skipped, not returned broken).
    ///
    /// **Methodology:** [`primitives::uv_sphere(24, 12, 3.0)`] → [`mesh_to_tet_dual`]
    /// with default options (`cell_size = 0.6 m`). **Pass criterion:** valid,
    /// zero negative cells. Volume is *not* asserted exactly — a staircase carve
    /// of a curved surface conserves volume only to the discretisation error, so
    /// only validity + inversion-freedom are gated. **Result (2026-08-03):**
    /// valid == true, 0 negative cells.
    #[test]
    fn sphere_bridges_to_valid_mesh() {
        let sphere = primitives::uv_sphere(24, 12, 3.0);
        let (mesh, report) =
            mesh_to_tet_dual(&sphere, &TetDualOptions::default()).expect("sphere meshes");
        assert!(mesh.validate().is_ok());
        assert!(report.valid);
        assert_eq!(report.n_negative_volume_cells, 0);
    }

    /// V&V — `mesh_to_surface` triangulates and converts a cube's 6 quads into
    /// 12 triangles over 8 shared points, preserving coordinates.
    #[test]
    fn mesh_to_surface_triangulates_cube() {
        let cube = primitives::cube(2.0);
        let (points, tris) = mesh_to_surface(&cube).expect("a cube is a closed manifold");
        assert_eq!(points.len(), 8, "cube has 8 corners");
        assert_eq!(tris.len(), 12, "6 quads fan-triangulate to 12 triangles");
        // Every index is in range.
        assert!(tris.iter().flatten().all(|&i| i < points.len()));
    }

    // =======================================================================
    // Watertightness gate at the meshing seam.
    //
    // The backend carve classifies interior cells by RAY PARITY
    // (`outram_park_fork_cfmesh::carve::inside`: cast a ray, count triangle
    // crossings, odd => inside). Parity is only meaningful on a closed surface,
    // and a corrupt result still passes the backend's own `validate()` (which
    // checks that cells are closed, not that they are the RIGHT cells). So the
    // seam must reject a non-manifold surface up front rather than pass it on.
    // =======================================================================

    /// V&V — good path: every closed [`primitives`] generator passes the
    /// manifold gate.
    ///
    /// **Methodology:** convert `cube(2.0)`, `uv_sphere(16, 8, 1.0)` and
    /// `cylinder(12, 1.0, 2.0)` through [`mesh_to_surface`], then re-run
    /// [`check_closed_manifold`] on the emitted soup. **Pass criterion:** all
    /// three convert without error, every emitted triangle count equals the
    /// fan-triangulation count `sum(n_i - 2)` over faces, and the standalone
    /// check agrees.
    ///
    /// **Result (measured 2026-08-07):** `cube(2.0)` → 8 points / 12 triangles;
    /// `uv_sphere(16, 8, 1.0)` → 114 points / 224 triangles; `cylinder(12, 1.0,
    /// 2.0)` → 24 points / 44 triangles. Each equals `sum(n_i - 2)` over that
    /// mesh's faces, and each is accepted by [`check_closed_manifold`].
    /// Interpretation: the gate does not reject legitimate authored geometry,
    /// and in particular the uv-sphere's 32 narrow pole triangles all clear the
    /// zero-area threshold.
    #[test]
    fn closed_primitives_pass_the_manifold_gate() {
        let cases = [
            ("cube", primitives::cube(2.0)),
            ("uv_sphere", primitives::uv_sphere(16, 8, 1.0)),
            ("cylinder", primitives::cylinder(12, 1.0, 2.0)),
        ];
        for (name, mesh) in cases {
            let expected: usize = mesh.polygons().iter().map(|p| p.len() - 2).sum();
            let (points, tris) = mesh_to_surface(&mesh)
                .unwrap_or_else(|e| panic!("{name} must be a closed manifold, got: {e}"));
            assert_eq!(tris.len(), expected, "{name}: no triangle may be dropped");
            check_closed_manifold(&points, &tris)
                .unwrap_or_else(|e| panic!("{name} soup must re-check clean, got: {e}"));
        }
    }

    /// V&V — an **open** surface is rejected, not silently exported leaky.
    ///
    /// **Methodology:** `primitives::grid(2, 2, 1.0)` is a flat 2x2 patch — a
    /// disc (χ = 1), not a closed solid, so its perimeter edges each touch one
    /// triangle. Feed it to [`mesh_to_surface`] and to [`mesh_to_tet_dual`].
    /// **Pass criterion:** both return `Err`; the error is
    /// [`SurfaceError::NotClosed`] and its message names a face and the unpaired
    /// edge.
    ///
    /// **Result (measured 2026-08-07):** `Err(NotClosed { face: 0, tri: 0,
    /// a: 0, b: 3 })`.
    ///
    /// **Why this matters — the old behaviour, measured.** The same 9-point /
    /// 8-triangle open soup was passed directly to
    /// `outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh` (which is
    /// what this bridge used to do) on 2026-08-07. It returned **`Ok`**, with
    /// `report.valid == true`, `cell_count == 618`,
    /// `n_negative_volume_cells == 0`, `mesh.validate().is_ok() == true`, and
    /// `total_volume == 0.864 m³` — a fabricated volume, since a flat patch
    /// encloses **zero** volume. Nothing in the returned mesh or report flagged
    /// the input as unusable: the backend's `validate()` checks that cells are
    /// closed, not that they are the *right* cells. That is precisely the
    /// silent corruption this gate exists to stop.
    #[test]
    fn open_grid_is_rejected_not_silently_meshed() {
        let grid = primitives::grid(2, 2, 1.0);
        let err = mesh_to_surface(&grid).expect_err("an open patch is not watertight");
        assert!(
            matches!(err, SurfaceError::NotClosed { .. }),
            "expected NotClosed, got {err:?}"
        );
        assert!(err.to_string().contains("not watertight"), "message: {err}");

        let piped = mesh_to_tet_dual(&grid, &TetDualOptions::default())
            .expect_err("the bridge must refuse an open surface");
        assert!(piped.contains("not watertight"), "message: {piped}");
    }

    /// V&V — a **hole punched in an otherwise closed** soup is caught.
    ///
    /// **Methodology:** take the cube's 12 valid triangles and drop the last
    /// one, leaving the three edges of that triangle each shared by only one
    /// triangle. **Pass criterion:** [`check_closed_manifold`] returns
    /// [`SurfaceError::NotClosed`]. **Result (2026-08-07):** as expected — this
    /// is the exact failure mode the old `.filter(|poly| poly.len() == 3)`
    /// silent drop could produce.
    #[test]
    fn deleted_triangle_breaks_watertightness() {
        let (points, mut tris) = mesh_to_surface(&primitives::cube(2.0)).unwrap();
        tris.pop();
        let err = check_closed_manifold(&points, &tris).expect_err("11/12 tris is leaky");
        assert!(matches!(err, SurfaceError::NotClosed { .. }), "got {err:?}");
    }

    /// V&V — a **duplicated** triangle (non-manifold edge) is caught.
    ///
    /// **Methodology:** append a copy of the cube's first triangle, so its three
    /// directed edges are each traversed twice in the same direction. **Pass
    /// criterion:** [`SurfaceError::InconsistentWinding`], naming both triangles.
    /// **Result (2026-08-07):** as expected.
    #[test]
    fn duplicated_triangle_is_non_manifold() {
        let (points, mut tris) = mesh_to_surface(&primitives::cube(2.0)).unwrap();
        tris.push(tris[0]);
        let err = check_closed_manifold(&points, &tris).expect_err("duplicate is non-manifold");
        match err {
            SurfaceError::InconsistentWinding { tri, other_tri, .. } => {
                assert_eq!(other_tri, 0, "first traversal was triangle 0");
                assert_eq!(tri, 12, "the appended duplicate is triangle 12");
            }
            other => panic!("expected InconsistentWinding, got {other:?}"),
        }
    }

    /// V&V — a **flipped** triangle (locally inconsistent winding) is caught.
    ///
    /// **Methodology:** reverse the corner order of one cube triangle. Its edges
    /// now run the same direction as their neighbours' rather than opposite, so
    /// the surface is no longer consistently oriented. **Pass criterion:** an
    /// `Err` — either `InconsistentWinding` (a directed edge traversed twice) or
    /// `NotClosed` (an edge left unpaired); both are correct diagnoses of a
    /// flipped face. **Result (2026-08-07):** `Err`, as expected.
    #[test]
    fn flipped_triangle_breaks_orientation() {
        let (points, mut tris) = mesh_to_surface(&primitives::cube(2.0)).unwrap();
        tris[0].swap(1, 2);
        let err = check_closed_manifold(&points, &tris).expect_err("flipped winding");
        assert!(
            matches!(
                err,
                SurfaceError::InconsistentWinding { .. } | SurfaceError::NotClosed { .. }
            ),
            "got {err:?}"
        );
    }

    /// V&V — a face whose vertex ring **repeats a corner** is a hard error, and
    /// the message names the offending face.
    ///
    /// **Methodology:** build a "quad" `[0, 1, 1, 2]` by hand through
    /// [`Mesh::from_polygons`] plus a second face, so face 0 fan-triangulates to
    /// `[0, 1, 1]` — a triangle with a repeated corner and a `1 -> 1` self-edge
    /// that can never be paired. **Pass criterion:**
    /// [`SurfaceError::RepeatedCorner`] with `face == 0`. **Result
    /// (2026-08-07):** as expected; previously this triangle passed the
    /// `poly.len() == 3` filter untouched and went straight to the backend.
    #[test]
    fn repeated_corner_names_the_offending_face() {
        use crate::math::Vec3 as BVec3;
        let positions = vec![
            BVec3::new(0.0, 0.0, 0.0),
            BVec3::new(1.0, 0.0, 0.0),
            BVec3::new(0.0, 1.0, 0.0),
            BVec3::new(0.0, 0.0, 1.0),
        ];
        // Face 0 repeats vertex 1; face 1 is an ordinary triangle.
        let faces = vec![vec![0, 1, 1, 2], vec![0, 2, 3]];
        let mesh = Mesh::from_polygons(&positions, &faces);

        let err = mesh_to_surface(&mesh).expect_err("a repeated corner is degenerate");
        match err {
            SurfaceError::RepeatedCorner { face, a, b, c, .. } => {
                assert_eq!(face, 0, "the diagnostic must name face 0");
                assert_eq!([a, b, c], [0, 1, 1]);
            }
            other => panic!("expected RepeatedCorner, got {other:?}"),
        }
    }

    /// V&V — a **collinear (zero-area)** triangle is a hard error naming its
    /// face.
    ///
    /// **Methodology:** a "quad" `[0, 1, 2, 3]` whose first three vertices are
    /// collinear along +X fan-triangulates to `[0, 1, 2]` with exactly zero
    /// area. **Pass criterion:** [`SurfaceError::ZeroAreaTriangle`] with
    /// `face == 0` and `area == 0.0`. **Result (2026-08-07):** as expected. The
    /// threshold is `1e-12 * extent²` (an "exactly zero to `f64`" bar), so a
    /// thin-but-real triangle is not rejected — see
    /// [`closed_primitives_pass_the_manifold_gate`], where a 16x8 uv-sphere's
    /// narrow pole triangles all pass.
    #[test]
    fn collinear_triangle_is_rejected() {
        use crate::math::Vec3 as BVec3;
        let positions = vec![
            BVec3::new(0.0, 0.0, 0.0),
            BVec3::new(1.0, 0.0, 0.0),
            BVec3::new(2.0, 0.0, 0.0), // collinear with the previous two
            BVec3::new(0.0, 1.0, 0.0),
        ];
        let mesh = Mesh::from_polygons(&positions, &[vec![0, 1, 2, 3]]);
        let err = mesh_to_surface(&mesh).expect_err("collinear corners are degenerate");
        match err {
            SurfaceError::ZeroAreaTriangle { face, area, .. } => {
                assert_eq!(face, 0);
                assert_eq!(area, 0.0, "exactly collinear");
            }
            other => panic!("expected ZeroAreaTriangle, got {other:?}"),
        }
    }

    /// V&V — an empty mesh reports [`SurfaceError::Empty`] rather than handing
    /// the backend an empty soup.
    #[test]
    fn empty_mesh_is_rejected() {
        let err = mesh_to_surface(&Mesh::new()).expect_err("no faces, nothing to mesh");
        assert_eq!(err, SurfaceError::Empty { face_count: 0 });
    }

    /// V&V — an out-of-range corner index is reported, not indexed into.
    ///
    /// [`mesh_to_surface`] cannot produce one ([`Mesh`] owns its indices), but
    /// [`check_closed_manifold`] is public and takes a bare soup, so it must not
    /// panic on a hand-built one.
    #[test]
    fn out_of_range_index_is_reported_not_panicked() {
        let (points, mut tris) = mesh_to_surface(&primitives::cube(2.0)).unwrap();
        tris[0][2] = 999;
        let err = check_closed_manifold(&points, &tris).expect_err("index 999 is out of range");
        assert!(
            matches!(err, SurfaceError::IndexOutOfRange { index: 999, n_points: 8, .. }),
            "got {err:?}"
        );
    }

    /// V&V — export writes the standard OpenFOAM polyMesh files to disk.
    ///
    /// **Methodology:** mesh a cube, [`export_polymesh`] to a temp dir, assert the
    /// five polyMesh files exist and are non-empty. **Result (2026-08-03):**
    /// `points`, `faces`, `owner`, `neighbour`, `boundary` all written.
    #[test]
    fn export_writes_polymesh_files() {
        let cube = primitives::cube(2.0);
        let opts = TetDualOptions { cell_size: 0.5, ..Default::default() };
        let (mesh, _report) = mesh_to_tet_dual(&cube, &opts).expect("cube meshes");

        let dir = std::env::temp_dir().join(format!("outram_blender_meshtest_{}", std::process::id()));
        export_polymesh(&mesh, &dir).expect("export succeeds");
        for f in ["points", "faces", "owner", "neighbour", "boundary"] {
            let p = dir.join(f);
            let meta = std::fs::metadata(&p).unwrap_or_else(|_| panic!("{f} written"));
            assert!(meta.len() > 0, "{f} is non-empty");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
