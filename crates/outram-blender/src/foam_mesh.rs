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
//! # Surface requirements
//!
//! The input surface must be **closed, watertight, and consistently outward-wound**
//! (outward normals), exactly as the [`crate::primitives`] generators produce. A
//! non-watertight or inward-wound surface gives an ill-defined inside test and is
//! not supported by the backend carve. Fan-triangulation
//! ([`crate::triangulate::triangulate`]) preserves winding, so an outward-wound
//! polygon mesh stays outward-wound.
//!
//! # Scope and trust
//!
//! **Untrusted AI-assisted draft pending human V&V.** The verification here is of
//! mesh *topology* (closed cells, no inverted cells) and volume conservation
//! against the analytic volume of the built-in primitives — **not** validation
//! against a CFD/TH solve. Offline demonstration only, per the workspace
//! `RESPONSIBLE_USE.md`: not for reactor operation, licensing, or safety-critical
//! decisions.

use std::path::Path;

use crate::mesh::Mesh;
use crate::triangulate::triangulate;

// Re-export the backend types callers need, so a user of this bridge does not
// have to name the `outram-park-fork-cfmesh` crate directly for the common path.
pub use outram_park_fork_cfmesh::math::Vec3;
pub use outram_park_fork_cfmesh::pipeline::{TetDualOptions, TetDualReport};
pub use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;

/// Convert a blender surface [`Mesh`] into the cfmesh surface representation:
/// a flat point list plus triangle vertex-index triples.
///
/// The mesh is fan-triangulated first ([`crate::triangulate::triangulate`]), so
/// quads and `n`-gons are accepted, not only triangle meshes; winding is
/// preserved. Each blender [`crate::math::Vec3`] is copied component-wise into a
/// cfmesh [`Vec3`] (one model unit → one metre). The returned data is exactly the
/// `(points, tris)` pair [`surface_to_tet_dual_mesh`](outram_park_fork_cfmesh::pipeline::surface_to_tet_dual_mesh)
/// expects.
///
/// The surface should be closed and outward-wound (see the module docs).
pub fn mesh_to_surface(mesh: &Mesh) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let tri_mesh = triangulate(mesh);
    let points: Vec<Vec3> =
        tri_mesh.positions().iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect();
    let tris: Vec<[usize; 3]> = tri_mesh
        .polygons()
        .iter()
        .filter(|poly| poly.len() == 3)
        .map(|poly| [poly[0].0, poly[1].0, poly[2].0])
        .collect();
    (points, tris)
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
/// Returns `Err(msg)` only when meshing cannot start or finish: `cell_size <= 0`,
/// the carve produced zero cells (surface not closed / cell size too large), the
/// surface produced no triangles, or — should not happen — the final mesh is not
/// closed.
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
    let (points, tris) = mesh_to_surface(mesh);
    if tris.is_empty() {
        return Err("surface has no triangular faces to mesh".into());
    }
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
        let (points, tris) = mesh_to_surface(&cube);
        assert_eq!(points.len(), 8, "cube has 8 corners");
        assert_eq!(tris.len(), 12, "6 quads fan-triangulate to 12 triangles");
        // Every index is in range.
        assert!(tris.iter().flatten().all(|&i| i < points.len()));
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
