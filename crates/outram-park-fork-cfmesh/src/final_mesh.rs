// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f):
//   meshLibrary/tetMesh/tetMeshGenerator/tetMeshGenerator.C
//     (optimiseFinalMesh: the order of the calls)
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/
//     meshOptimizerOptimizeSurface.C (meshOptimizer::optimizeSurface)
//     optimizeMeshFV.C (optimizeMeshFV, untangleMeshFV, optimizeLowQualityFaces)
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later, conveyed here
// under GPL-3.0-only.
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

//! cfMesh's **final mesh optimisation** on the bare dual, and the
//! **projection of boundary moves** back onto the surface (gh:#1283).
//!
//! **Upstream's order** (`tetMeshGenerator::optimiseFinalMesh`):
//! `meshOptimizer::optimizeSurface(octree)` (`meshSurfaceOptimizer::
//! optimizeSurface`, then `mapVerticesOntoSurfacePatches`),
//! `optimizeMeshFV` (`laplacianPC` × 5, then `untangleMeshFV(10, 50, 2)`),
//! `optimizeLowQualityFaces(10)`, `optimizeBoundaryLayer(false)` (layers
//! only), `untangleMeshFV(10, 50, 2)`. [`optimise_final_mesh`] runs it on
//! a mesh with no layer; with layers,
//! [`crate::boundary_layer_optimisation::add_boundary_layers_cfmesh_on_surface`]
//! runs it around the layer.
//!
//! **What upstream does to keep boundary points on the surface during and
//! after `untangleMeshFV`** (read 2026-11-05 from `optimizeMeshFV.C` and
//! `tetMeshOptimisation.C`): nothing that consults the surface. Its
//! boundary loop calls `optimiseBoundaryVolumeOptimizer(true)` while
//! `nGlobalIter < 2`, which projects each move onto the tangent plane (a
//! surface point), the tangent line (an edge point) or drops it (a
//! corner), from the eigen-split of the simplex's boundary-triangle normal
//! tensor; then `optimiseBoundarySurfaceLaplace` while `< 5` and
//! `optimiseBoundaryVolumeOptimizer(false)` after that, **both
//! unconstrained**. No `meshSurfaceMapper` call follows, in
//! `untangleMeshFV` or in `optimiseFinalMesh` (its `optimizeSurface` comes
//! first). The one place upstream maps a finished mesh back onto the
//! surface is `projectSurfaceAfterBackScaling` (`mapVerticesOntoSurface`,
//! then `optimiseFinalMesh` again), which runs only with
//! `anisotropicSources`.
//!
//! So [`BoundaryProjection`] is **not upstream**. It puts the points a
//! boundary pass moved back onto their patches with this crate's port of
//! `mapVerticesOntoSurfacePatches`
//! ([`crate::surface_mapper::map_points_onto_surface_patches_given`]), in
//! the spirit of `projectSurfaceAfterBackScaling` (map, then let the
//! optimiser repair the cells). It is off by default.

use crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls;
use crate::math::Vec3;
use crate::mesh_optimizer::{find_bad_faces, optimize_low_quality_faces, untangle_mesh_fv_projected, LaplaceKind, MeshOptimizer, OptimiserReport, UntangleControls};
use crate::patches::SurfaceRegions;
use crate::volume_mesh::VolumeMesh;

/// Whether, and when, the boundary points `untangleMeshFV`'s boundary loop
/// moves are put back onto the surface (gh:#1283). Not upstream (module
/// docs). Only used with `UntangleControls::boundary_moves` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryProjection {
    /// No projection: upstream's behaviour, and the default.
    #[default]
    Off,
    /// After every pass of the boundary loop, map the points it moved back
    /// onto their patches; the next global iteration's interior passes see
    /// the projected points.
    EachPass,
    /// Once, when the untangle has finished: every point any boundary pass
    /// moved, mapped back onto its patch, with no untangling afterwards.
    /// The isolation case: the cells the moves changed, minus the moves'
    /// departure from the surface.
    AfterUntangle,
}

/// The surface a mesh's boundary belongs to, with the patch of every
/// boundary face fixed at construction (the nearest surface triangle of
/// its centroid, [`crate::surface_mapper::boundary_face_regions`]). The
/// topology of the mesh it was built on must not change while it is used.
#[derive(Debug, Clone)]
pub struct BoundaryProjector {
    points: Vec<Vec3>,
    tris: Vec<[usize; 3]>,
    regions: SurfaceRegions,
    /// The patch of every face of the mesh (`None`: internal).
    pub face_region: Vec<Option<usize>>,
}

impl BoundaryProjector {
    /// The projector of `mesh` onto the surface `(points, tris)` with
    /// `regions`; the face patches are read from `mesh` as it is now.
    pub fn new(mesh: &VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions) -> Self {
        let face_region = crate::surface_mapper::boundary_face_regions(mesh, points, tris, regions);
        BoundaryProjector { points: points.to_vec(), tris: tris.to_vec(), regions: regions.clone(), face_region }
    }

    /// Map the boundary points with `only[v]` set onto their patches (patch
    /// nodes to the patch, edge nodes to the feature edge, corners to the
    /// corner). Returns the largest move.
    pub fn project(&self, mesh: &mut VolumeMesh, only: &[bool]) -> f64 {
        if !only.iter().any(|&b| b) {
            return 0.0;
        }
        let mapped = crate::surface_mapper::map_points_onto_surface_patches_given(mesh, &self.points, &self.tris, &self.regions, &self.face_region, only);
        let mut max = 0.0f64;
        for (p, q) in mesh.points.iter_mut().zip(&mapped.points) {
            max = max.max(q.sub(*p).length());
            *p = *q;
        }
        max
    }

    /// `meshOptimizer::optimizeSurface(octree)` on `mesh` with this
    /// projector's patches ([`crate::mesh_optimizer::optimize_surface`]).
    pub fn optimize_surface(&self, mesh: &mut VolumeMesh) -> crate::surface_optimizer::UntangleReport {
        crate::mesh_optimizer::optimize_surface(mesh, &self.points, &self.tris, &self.regions, &self.face_region)
    }
}

/// What the projection did in one untangle.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProjectionReport {
    /// Points any boundary pass moved.
    pub points_moved: usize,
    /// Projections made (passes with `EachPass`, 0 or 1 with
    /// `AfterUntangle`).
    pub projections: usize,
    /// The largest single projection move, metres.
    pub max_move: f64,
}

/// `untangleMeshFV` with `controls`, and the boundary moves projected as
/// `projection` says when a projector is given (with no projector, or
/// `Off`, this is [`crate::mesh_optimizer::untangle_mesh_fv_with`] bit for
/// bit).
pub fn untangle_on_surface(
    mesh: &mut VolumeMesh,
    locked: &[bool],
    controls: &UntangleControls,
    projection: BoundaryProjection,
    projector: Option<&BoundaryProjector>,
) -> (OptimiserReport, ProjectionReport) {
    let mut pr = ProjectionReport::default();
    let each = projection == BoundaryProjection::EachPass;
    let (rep, moved) = untangle_mesh_fv_projected(mesh, locked, controls, &mut |m: &mut VolumeMesh, pass: &[bool]| {
        if let (true, Some(p)) = (each, projector) {
            pr.max_move = pr.max_move.max(p.project(m, pass));
            pr.projections += 1;
        }
    });
    pr.points_moved = moved.iter().filter(|&&b| b).count();
    if let (BoundaryProjection::AfterUntangle, Some(p)) = (projection, projector) {
        pr.max_move = pr.max_move.max(p.project(mesh, &moved));
        pr.projections += 1;
    }
    (rep, pr)
}

/// One stage note for a projection.
pub(crate) fn projection_note(projection: BoundaryProjection, pr: &ProjectionReport) -> String {
    match projection {
        BoundaryProjection::Off => String::new(),
        _ => format!("; projection {projection:?}: {} points moved by boundary passes, {} projections, largest {:.2e} m", pr.points_moved, pr.projections, pr.max_move),
    }
}

/// `tetMeshGenerator::optimiseFinalMesh` on a mesh with **no layer**
/// (module docs), its boundary on the surface `(points, tris)` with
/// `regions`: `optimizeSurface` (surface smoothing, then the patch
/// mapping), `laplacianPC` × 5 when
/// [`BoundaryLayerOptimisationControls::optimize_mesh_fv_laplace`] is on,
/// `untangleMeshFV(10, 50, 2)`, `optimizeLowQualityFaces(10)`,
/// `untangleMeshFV(10, 50, 2)`. The untangles take the boundary moves, the
/// LP untangler, the stall bound and the projection from `controls`.
/// Returns the stage notes. The caller decides whether to keep the result.
pub fn optimise_final_mesh(mesh: &mut VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions, controls: &BoundaryLayerOptimisationControls) -> Vec<String> {
    let mut notes = Vec::new();
    let projector = BoundaryProjector::new(mesh, points, tris, regions);
    let r = projector.optimize_surface(mesh);
    notes.push(format!("optimiseFinalMesh: optimizeSurface: inverted points {} -> {}", r.n_inverted_first, r.n_inverted_last));
    if controls.optimize_mesh_fv_laplace {
        MeshOptimizer::new(mesh).optimize_laplacian(mesh, LaplaceKind::PointCells, 5);
        notes.push("optimiseFinalMesh: optimizeMeshFV laplacianPC x5".into());
    }
    let none = vec![false; mesh.points.len()];
    let c = UntangleControls {
        max_global_iterations: 10,
        max_iterations: 50,
        max_surface_iterations: 2,
        relaxed: false,
        stop_on_stall: controls.stop_untangling_on_stall,
        boundary_moves: controls.untangle_boundary_moves,
        mesh_untangler: controls.untangle_mesh_untangler,
        // the tet-dual pipeline's opt-in keeps the pre-round-5 calls: Foam
        // Studio's pins (not run in this gate) depend on them
        boundary_calls: crate::mesh_optimizer::BoundaryMoveCalls::AsDocumented,
    };
    let (r, pr) = untangle_on_surface(mesh, &none, &c, controls.boundary_projection, Some(&projector));
    notes.push(format!(
        "optimiseFinalMesh: optimizeMeshFV untangleMeshFV: bad faces {} -> {} ({} global, {} boundary passes){}",
        r.flagged_before,
        r.flagged_after,
        r.global_iterations,
        r.surface_iterations,
        projection_note(controls.boundary_projection, &pr)
    ));
    let r = optimize_low_quality_faces(mesh, &none, 10);
    notes.push(format!("optimiseFinalMesh: optimizeLowQualityFaces: {} -> {} ({} passes)", r.flagged_before, r.flagged_after, r.iterations));
    let (r, pr) = untangle_on_surface(mesh, &none, &c, controls.boundary_projection, Some(&projector));
    notes.push(format!(
        "optimiseFinalMesh: untangleMeshFV: bad faces {} -> {} ({} global, {} boundary passes){}",
        r.flagged_before,
        r.flagged_after,
        r.global_iterations,
        r.surface_iterations,
        projection_note(controls.boundary_projection, &pr)
    ));
    let n_bad = find_bad_faces(mesh).iter().filter(|&&b| b).count();
    notes.push(format!("optimiseFinalMesh: {n_bad} bad faces at the end (findBadFaces)"));
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls;
    use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};

    /// The largest |z| in `m` of the points of the `inlet` faces (as the
    /// projector reads the patches) that lie on the plane z = 0 in `m0`.
    /// The default dual's capture leaves some rim points off the surface
    /// (reverted, gh:#1241); those are not the projection's business.
    fn inlet_off_plane(m: &VolumeMesh, m0: &VolumeMesh, pr: &BoundaryProjector, regions: &SurfaceRegions) -> f64 {
        let inlet = regions.names.iter().position(|n| n == "inlet").unwrap();
        (0..m.face_count())
            .filter(|&f| pr.face_region[f] == Some(inlet))
            .flat_map(|f| m.faces[f].iter().filter(|&&p| m0.points[p].z == 0.0).map(|&p| m.points[p].z.abs()))
            .fold(0.0, f64::max)
    }

    /// A short pipe (R 10 mm, L 40 mm, 24 sides) at h 5 mm, bare, the
    /// default dual: small enough for the default test run. In the
    /// pipeline's face order before gh:#1285 (`upper_triangular: false`),
    /// the order these tests' cases were recorded on (the shake below picks
    /// its points in face order).
    fn small_pipe() -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions, VolumeMesh) {
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.04, 24);
        let o = TetDualOptions { cell_size: 0.005, n_layers: 0, ..Default::default() };
        let (m, _) = crate::pipeline::run_pipeline(&p, &t, &o, Some(&regions), false).unwrap();
        (p, t, regions, m)
    }

    /// The projector moves only the points it is asked to, and puts an
    /// inlet point pushed off the plane back onto it (z = 0 to round-off),
    /// and is idempotent: a second projection of every boundary point
    /// moves none.
    #[test]
    fn projector_restores_moved_points_and_leaves_the_rest() {
        let (p, t, regions, m0) = small_pipe();
        let pr = BoundaryProjector::new(&m0, &p, &t, &regions);
        let inlet = regions.names.iter().position(|n| n == "inlet").unwrap();
        let f = (0..m0.face_count()).find(|&f| pr.face_region[f] == Some(inlet)).unwrap();
        let (a, b) = (m0.faces[f][0], m0.faces[f][1]);
        let mut m = m0.clone();
        m.points[a].z += 2e-4;
        m.points[b].z += 2e-4;
        let mut only = vec![false; m.points.len()];
        only[a] = true;
        let moved = pr.project(&mut m, &only);
        assert!(moved > 1e-4, "the pushed point came back ({moved})");
        assert!(m.points[a].z.abs() < 1e-12, "on the inlet plane: {}", m.points[a].z);
        assert_eq!(m.points[b].z, m0.points[b].z + 2e-4, "a point not asked for is not moved");
        // Every boundary point of the bare mesh: a no-op to round-off.
        let mut all = vec![false; m0.points.len()];
        for f in 0..m0.face_count() {
            if m0.neighbour[f].is_none() {
                for &q in &m0.faces[f] {
                    all[q] = true;
                }
            }
        }
        let mut m2 = m0.clone();
        pr.project(&mut m2, &all);
        let mv = pr.project(&mut m2, &all);
        assert!(mv < 1e-12, "a projected boundary projects onto itself ({mv:e})");
    }

    /// With no projector, or `Off`, the untangle is bit for bit
    /// `untangle_mesh_fv_with`; `EachPass` with boundary moves leaves the
    /// inlet planar to 1e-9 m (the gh:#1283 gate), on a mesh whose interior
    /// points were shaken so that the untangle has work to do.
    ///
    /// **gh:#1285 (2026-11-05).** The shake picks every third interior
    /// point **in face order**, so it is a different case on the
    /// upper-triangular mesh. The recorded case (`small_pipe`'s mesh in the
    /// pipeline's pre-gh:#1285 face order, `upper_triangular: false`) is
    /// kept and asserted as before. On the same mesh in the gh:#1285 order
    /// the shake picks other points: `EachPass` still keeps the inlet
    /// planar (asserted), but **`AfterUntangle` does not**: a rim point
    /// (r 9.967 mm) that the boundary passes slid 1.9 mm up the wall is
    /// mapped back only to z = 1.78e-4 m by the one projection at the end.
    /// That is a measured limit of `AfterUntangle`, recorded in
    /// `verification_and_validation/upper_triangular_gh1285/README.md`, and
    /// not asserted away.
    ///
    /// **MOVED 2026-11-11 (gh:#1385 round 3):** ~~`AfterUntangle` leaves the
    /// rim point 1.78e-4 m off the inlet plane in the gh:#1285 order~~ it now
    /// keeps the inlet planar there too (0e0). Cause: the part tet mesh now
    /// takes cfMesh's face and cell centres and cell-list order, and the
    /// volume optimiser, Knupp metric and tet-mesh updates follow upstream's
    /// arithmetic (V&V README, round 3), so the interior smoothing, and with
    /// it the boundary passes, take other paths on this shaken mesh. The
    /// limit of `AfterUntangle` is not removed in general (the projection is
    /// still only at the end); this case no longer shows it.
    #[test]
    fn projected_untangle_keeps_the_inlet_planar() {
        let (p, t, regions, recorded) = small_pipe();
        let reordered = crate::renumber_mesh::upper_triangular(&recorded);
        for (case, m0) in [("recorded", recorded), ("gh1285 order", reordered)] {
            let pr = BoundaryProjector::new(&m0, &p, &t, &regions);
            let shaken = shake_interior(&m0);
            let none = vec![false; m0.points.len()];
            // One interior pass per global iteration, so bad faces are left
            // for the boundary loop.
            let c = UntangleControls { boundary_moves: true, max_global_iterations: 4, max_iterations: 1, ..UntangleControls::default() };
            let mut a = shaken.clone();
            let ra = crate::mesh_optimizer::untangle_mesh_fv_with(&mut a, &none, &c);
            let mut b = shaken.clone();
            let (rb, _) = untangle_on_surface(&mut b, &none, &c, BoundaryProjection::Off, Some(&pr));
            assert_eq!(a.points, b.points, "Off is the plain untangle");
            assert_eq!(ra.flagged_after, rb.flagged_after);
            let mut e = shaken.clone();
            let (re, pe) = untangle_on_surface(&mut e, &none, &c, BoundaryProjection::EachPass, Some(&pr));
            let mut g = shaken.clone();
            let (_, pg) = untangle_on_surface(&mut g, &none, &c, BoundaryProjection::AfterUntangle, Some(&pr));
            let plain_off = inlet_off_plane(&a, &m0, &pr, &regions);
            let after_off = inlet_off_plane(&g, &m0, &pr, &regions);
            println!("{case}: plain (inlet off plane {plain_off:e}): {} -> {}, {} boundary passes; EachPass: {} -> {}, {pe:?}; AfterUntangle {pg:?}, off plane {after_off:e}", ra.flagged_before, ra.flagged_after, ra.surface_iterations, re.flagged_before, re.flagged_after);
            assert!(ra.surface_iterations > 0, "the case exercises the boundary loop");
            assert!(inlet_off_plane(&e, &m0, &pr, &regions) < 1e-9, "{case}: EachPass: inlet off plane by {:e}", inlet_off_plane(&e, &m0, &pr, &regions));
            // ~~the "gh1285 order" case missed by 1.78e-4 m (assert > 1e-5)~~
            // now planar too, gh:#1385 round 3 (doc comment)
            assert!(after_off < 1e-9, "{case}: AfterUntangle: inlet off plane by {after_off:e}");
            assert_eq!(pe.points_moved > 0, ra.surface_iterations > 0, "boundary points move exactly when a boundary pass runs");
        }
    }

    /// The test shake: every third interior point, in face order, pushed
    /// 1.5 times its distance from the face centre away from it.
    fn shake_interior(m0: &VolumeMesh) -> VolumeMesh {
        let mut shaken = m0.clone();
        let mut on_bnd = vec![false; m0.points.len()];
        for f in 0..m0.face_count() {
            if m0.neighbour[f].is_none() {
                for &q in &m0.faces[f] {
                    on_bnd[q] = true;
                }
            }
        }
        let mut k = 0usize;
        for f in 0..m0.face_count() {
            if m0.neighbour[f].is_none() {
                continue;
            }
            for &q in &m0.faces[f] {
                if !on_bnd[q] {
                    k += 1;
                    if k % 3 == 0 {
                        let c = m0.face_centroid(f);
                        shaken.points[q] = shaken.points[q].add(c.sub(shaken.points[q]).scale(-1.5));
                    }
                }
            }
        }
        shaken
    }

    /// `optimise_final_mesh` on the bare small pipe returns a closed mesh
    /// and its notes name every upstream call in order.
    #[test]
    fn optimise_final_mesh_runs_upstreams_calls_in_order() {
        let (p, t, regions, m0) = small_pipe();
        let mut m = m0.clone();
        let c = BoundaryLayerOptimisationControls { untangle_boundary_moves: true, untangle_mesh_untangler: true, boundary_projection: BoundaryProjection::EachPass, ..Default::default() };
        let notes = optimise_final_mesh(&mut m, &p, &t, &regions, &c);
        assert!(m.validate().is_ok());
        let order = ["optimizeSurface", "optimizeMeshFV untangleMeshFV", "optimizeLowQualityFaces", "optimiseFinalMesh: untangleMeshFV", "at the end"];
        for (n, o) in notes.iter().zip(order) {
            assert!(n.contains(o), "{n} should be {o}");
        }
    }

    /// ~~The Foam Studio unstructured default's~~ The pre-gh:#1298 Foam
    /// Studio unstructured default's (gh:#1298 moved Foam Studio to the
    /// cell-centre dual with upstream's sequence; this crate's defaults did
    /// not change) cfMesh mesh (per-corner
    /// median dual, h 4 mm, no layer, L 200 mm `pipe_surface`) keeps the
    /// fingerprint recorded in gh:#1240 / gh:#1241,
    /// ~~18084991279620725399~~ (**re-pinned 2026-11-05, gh:#1285:**
    /// 13281953444677737411, the same mesh in the upper-triangular face
    /// order), through the gh:#1283 code paths (every new control off).
    #[test]
    fn foam_studio_default_mesh_keeps_its_fingerprint() {
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.2, 48);
        let o = TetDualOptions { cell_size: 0.004, n_layers: 0, layer_patches: vec!["walls".into()], ..Default::default() };
        let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        assert_eq!(m.cell_count(), 6353);
        assert_eq!(crate::pipeline::tests::mesh_fingerprint(&m), 13281953444677737411);
    }
}

/// The gh:#1283 differential (`--ignored`, feature `cell-centre-dual`): the
/// Foam Studio pipe (48 sides, R 10 mm, L 200 mm), `CellCentre` +
/// `Corners`, h 4 / 5 / 6 mm, 0 and 3 wall layers (first 0.3 mm, × 1.2,
/// `CfMeshOptimise`), under the routes of [`diagnose_gh1283::variant`].
/// Prints per mesh: cells, time, max / p99 non-orthogonality, cells with a
/// non-positive face pyramid, the projected inlet / πR², the largest |z|
/// of an inlet point, the largest distance of any boundary point from the
/// surface, `findBadFaces` and OpenFOAM's `checkMesh` failures, the
/// layers' mean total thickness, V / πR²L. Filters: `CFMESH_HS`
/// (`0.004,0.005`), `CFMESH_LAYERS` (`0,3`), `CFMESH_VARIANTS` (names);
/// `CFMESH_NOTES=1` prints the stage notes. Predictions and results: the
/// gh:#1283 section of
/// `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
#[cfg(all(test, feature = "cell-centre-dual"))]
pub(crate) mod diagnose_gh1283 {
    use super::*;
    use crate::dual::{DualKind, FeaturePointRule};
    use crate::layers::LayerQualityStrategy;
    use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};

    /// The routes, by name:
    /// - `base`: today's defaults (`revert_tangled` capture; layers:
    ///   `CfMeshOptimise` as recorded);
    /// - `us`: + `untangle_surface` (gh:#1259);
    /// - `os`: `base` with `optimise_surface` inside `optimiseLayer`
    ///   (gh:#1259's third wiring);
    /// - `bm`: + boundary moves (gh:#1255), `revert_tangled` capture;
    /// - `bmE` / `bmA`: `bm` with the projection `EachPass` /
    ///   `AfterUntangle`;
    /// - `up`: upstream's whole sequence: `untangle_surface`,
    ///   `optimise_final_mesh` (`optimizeSurface` first), the Laplace pass,
    ///   boundary moves and the LP untangler, no projection;
    /// - `upE`: `up` with `EachPass`; `upEnl`: `upE` without the Laplace
    ///   pass.
    pub(crate) fn variant(name: &str, h: f64, n_layers: usize) -> TetDualOptions {
        let base = TetDualOptions {
            cell_size: h,
            n_layers,
            first_layer_thickness: 3e-4,
            expansion: 1.2,
            layer_patches: vec!["walls".into()],
            layer_strategy: LayerQualityStrategy::CfMeshOptimise,
            dual_kind: DualKind::CellCentre,
            dual_feature_points: FeaturePointRule::Corners,
            ..Default::default()
        };
        let c = base.cfmesh_layer_optimisation.clone();
        let bm = BoundaryLayerOptimisationControls { untangle_boundary_moves: true, ..c.clone() };
        let up = BoundaryLayerOptimisationControls { untangle_boundary_moves: true, untangle_mesh_untangler: true, optimize_mesh_fv_laplace: true, ..c };
        let with = |c: BoundaryLayerOptimisationControls, us: bool, fin: bool| TetDualOptions { untangle_surface: us, optimise_final_mesh: fin, cfmesh_layer_optimisation: c, ..base.clone() };
        let proj = |c: &BoundaryLayerOptimisationControls, p: BoundaryProjection| BoundaryLayerOptimisationControls { boundary_projection: p, ..c.clone() };
        match name {
            "base" => base.clone(),
            "us" => TetDualOptions { untangle_surface: true, ..base.clone() },
            "os" => with(BoundaryLayerOptimisationControls { optimise_surface: true, ..BoundaryLayerOptimisationControls::default() }, false, false),
            "bm" => with(bm, false, false),
            "bmE" => with(proj(&bm, BoundaryProjection::EachPass), false, false),
            "bmA" => with(proj(&bm, BoundaryProjection::AfterUntangle), false, false),
            "up" => with(up, true, true),
            "upE" => with(proj(&up, BoundaryProjection::EachPass), true, true),
            "upEnl" => with(BoundaryLayerOptimisationControls { optimize_mesh_fv_laplace: false, ..proj(&up, BoundaryProjection::EachPass) }, true, true),
            _ => panic!("unknown variant {name}"),
        }
    }

    /// V&V (gh:#1286): the mesh with zero-area faces is refused. The case
    /// is gh:#1283's `bmE` at h 4 mm with 3 layers (Foam Studio pipe,
    /// `CellCentre` + `Corners`, boundary moves, `EachPass`, the default
    /// `revert_tangled` capture), which the pipeline kept before the fix:
    /// two internal triangles in the inlet planes (faces 33638 and 33674 of
    /// 78 110; owner 1.3e-9 m³, neighbour 9e-14 / 7e-14 m³), each with three
    /// collinear points on the rim, area exactly 0, so non-orthogonality
    /// NaN; `checkFaceAreas(VSMALL)` fails and `checkGeometry` fails 4
    /// checks. **Prediction (written 2026-11-05, before the run with the
    /// fix):** the layer stage is refused, the pipeline returns the bare
    /// mesh with the "boundary layers skipped" note (as `bmA` at h 4 in the
    /// gh:#1283 table), and that mesh has no face below `VSMALL` and no NaN
    /// angle. Result: in the gh:#1286 section of
    /// `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
    #[test]
    fn gh1286_zero_area_layer_mesh_is_refused() {
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.2, 48);
        let o = variant("bmE", 0.004, 3);
        let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        assert!(rep.stage_notes.iter().any(|s| s.starts_with("boundary layers skipped")), "{:?}", rep.stage_notes);
        assert!(!crate::poly_mesh_gen_checks::check_face_areas(&m, 1e-300, None).0);
        let (_, centres) = crate::checks::cell_geometry(&m);
        for f in 0..m.face_count() {
            if let Some(nb) = m.neighbour[f] {
                let (d, s) = (centres[nb].sub(centres[m.owner[f]]), m.face_area_vector(f));
                assert!((d.dot(s) / (d.length() * s.length())).is_finite(), "face {f}");
            }
        }
        println!("gh1286: {} cells, notes {:?}", m.cell_count(), rep.stage_notes);
    }

    #[test]
    #[ignore]
    fn diagnose_final_mesh_differential() {
        let (r, l) = (0.01, 0.2);
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(r, l, 48);
        let env = |k: &str| std::env::var(k).ok();
        let hs: Vec<f64> = env("CFMESH_HS").map_or(vec![0.004, 0.005, 0.006], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        let layers: Vec<usize> = env("CFMESH_LAYERS").map_or(vec![0, 3], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        let all = ["base", "us", "bm", "bmE", "bmA", "up", "upE", "upEnl"];
        let names: Vec<String> = env("CFMESH_VARIANTS").map_or(all.iter().map(|s| s.to_string()).collect(), |v| v.split(',').map(String::from).collect());
        let section = std::f64::consts::PI * r * r;
        for &n in &layers {
            for &h in &hs {
                for name in &names {
                    if n == 0 && name.starts_with("bm") {
                        continue; // no untangle runs on a bare mesh without optimise_final_mesh
                    }
                    let o = variant(name, h, n);
                    let t0 = std::time::Instant::now();
                    let (m, rep) = match surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o) {
                        Ok(x) => x,
                        Err(e) => {
                            println!("h {:.0} n{n} {name}: ERROR {e}", h * 1e3);
                            continue;
                        }
                    };
                    let secs = t0.elapsed().as_secs_f64();
                    let (_, centres) = crate::checks::cell_geometry(&m);
                    let mut no: Vec<f64> = (0..m.face_count())
                        .filter_map(|f| {
                            m.neighbour[f].map(|nb| {
                                let (d, s) = (centres[nb].sub(centres[m.owner[f]]), m.face_area_vector(f));
                                (d.dot(s) / (d.length() * s.length())).clamp(-1.0, 1.0).acos().to_degrees()
                            })
                        })
                        .collect();
                    let n_nan = no.iter().filter(|x| x.is_nan()).count();
                    no.retain(|x| !x.is_nan());
                    no.sort_by(|a, b| a.total_cmp(b));
                    let q99 = no[((no.len() - 1) as f64 * 0.99).round() as usize];
                    let inlet = m.patches.iter().find(|q| q.name == "inlet").unwrap();
                    let ifaces = inlet.start_face..inlet.start_face + inlet.n_faces;
                    let a_in: f64 = ifaces.clone().map(|f| m.face_area_vector(f).z).sum::<f64>().abs();
                    let z_in = ifaces.flat_map(|f| m.faces[f].iter().map(|&v| m.points[v].z.abs())).fold(0.0f64, f64::max);
                    let mut off = 0.0f64;
                    for f in 0..m.face_count() {
                        if m.neighbour[f].is_none() {
                            for &v in &m.faces[f] {
                                let c = crate::snap::closest_point_on_surface(m.points[v], &p, &t);
                                off = off.max(c.sub(m.points[v]).length());
                            }
                        }
                    }
                    let npyr = crate::surface_mapper::cells_with_nonpositive_pyramids(&m).len();
                    let bad = crate::mesh_optimizer::find_bad_faces(&m).iter().filter(|&&b| b).count();
                    let strict = crate::checks::faces_failing_mesh_quality(&m, &crate::checks::MeshQualityControls::default()).iter().filter(|&&b| b).count();
                    let thick = rep.layers.as_ref().map_or(0.0, |lo| 1e3 * lo.mean_total_thickness());
                    let skipped = rep.stage_notes.iter().any(|s| s.contains("skipped") && !s.starts_with("Delaunay"));
                    println!(
                        "h {:.0} n{n} {name}: {} cells, {secs:.1} s, NaN-NO faces {n_nan}, NO max {:.1} p99 {:.1}, pyr<=0 {npyr}, inlet/piR2 {:.5}, inlet |z| max {z_in:.2e}, off-surface max {off:.2e}, findBadFaces {bad}, checkMesh fail {strict}, thick {thick:.3} mm, V/piR2L {:.5}, skipped {skipped}, fp {}",
                        h * 1e3,
                        m.cell_count(),
                        no[no.len() - 1],
                        q99,
                        a_in / section,
                        m.total_volume() / (section * l),
                        crate::pipeline::tests::mesh_fingerprint(&m),
                    );
                    if env("CFMESH_NOTES").is_some() {
                        for s in &rep.stage_notes {
                            println!("    {s}");
                        }
                    }
                }
            }
        }
    }

    /// Diagnostic images for gh:#1283 (`--ignored`): the inlet patch of
    /// the Foam Studio pipe, drawn from the `VolumeMesh` the pipeline
    /// returns with gh:#1259's renderer (black: inlet face edges; red: the
    /// input 48-gon; blue squares: inlet points with |z| > 1e-9 m), whole
    /// and rim close-up (x 6–10.5 mm, y ±2.25 mm). Meshes: h 5 mm, 3
    /// layers, `base`, `bm`, `bmE`, `upE`; h 4 / 5 / 6 mm bare, `us`, `up`
    /// and `upE`. To `CFMESH_IMG_DIR` (default: the gh:#1283 folder of the
    /// V&V record). Drawing glue: nothing is asserted.
    #[test]
    #[ignore]
    fn render_final_mesh_inlets() {
        let dir = std::env::var("CFMESH_IMG_DIR").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155/gh1283").to_string()
        });
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.2, 48);
        let mut jobs: Vec<(usize, usize, &str)> = vec![(5, 3, "base"), (5, 3, "bm"), (5, 3, "bmE"), (5, 3, "upE")];
        for h in [4, 5, 6] {
            jobs.push((h, 0, "us"));
            jobs.push((h, 0, "up"));
            jobs.push((h, 0, "upE"));
        }
        for (h_mm, n, name) in jobs {
            let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &variant(name, h_mm as f64 * 1e-3, n)).unwrap();
            let stem = format!("inlet_{name}_h{h_mm}_n{n}");
            crate::surface_optimizer::render_gh1259::draw(&m, &dir.join(format!("{stem}.png")), -0.0105, -0.0105, 0.021);
            crate::surface_optimizer::render_gh1259::draw(&m, &dir.join(format!("{stem}_rim.png")), 0.006, -0.00225, 0.0045);
            println!("wrote {stem}(.png, _rim.png)");
        }
    }
}
