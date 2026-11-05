// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   utilities/improveMeshQuality/improveMeshQuality.C
//   utilities/improveSymmetryPlanes/improveSymmetryPlanes.C
//   utilities/generateBoundaryLayers/generateBoundaryLayers.C
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's mesh **utilities** of item 26 (gh:#1280): the executables'
//! `main`s as functions on a mesh (upstream reads and writes a case on
//! disk; here the mesh is passed in and returned).
//!
//! | upstream utility | here |
//! |---|---|
//! | `improveMeshQuality` | [`improve_mesh_quality`] |
//! | `improveSymmetryPlanes` | [`improve_symmetry_planes`] |
//! | `generateBoundaryLayers` | [`generate_boundary_layers`] (partial: see there) |
//! | `patchesToSubsets`, `subsetToPatch` | `surface::features` (gh:#1276, the surface track) |
//!
//! Each is a composition of ported classes; nothing here re-implements a
//! step.

use crate::boundary_layer_optimisation::{optimize_boundary_layer, untangle_boundary_layer, BoundaryLayerOptimisationControls, OptimizeBoundaryLayerReport, OptimizeBoundaryLayerSettings};
use crate::layers::boundary_layers::{generate_layer, BoundaryLayersControls, GeneratedLayer, LayerRequest};
use crate::layers::refine_boundary_layers::{refine_boundary_layers, RefineBoundaryLayersSettings, RefineError, RefineReport};
use crate::mesh_optimizer::{optimize_mesh_fv_best_quality, MeshOptimizer, OptimiserReport, UntangleControls};
use crate::symmetry_plane_optimisation::{symmetry_plane_optimisation, SymmetryReport};
use crate::volume_mesh::VolumeMesh;

/// `improveMeshQuality`'s options (upstream's defaults).
#[derive(Debug, Clone, PartialEq)]
pub struct ImproveMeshQualitySettings {
    /// `nLoops` (10): Laplace passes and global untangle iterations.
    pub n_loops: usize,
    /// `nIterations` (50).
    pub n_iterations: usize,
    /// `nSurfaceIterations` (2).
    pub n_surface_iterations: usize,
    /// `qualityThreshold` (0.1).
    pub quality_threshold: f64,
    /// `constrainedCellsSet`: cells whose points are locked.
    pub constrained_cells: Option<Vec<usize>>,
}

impl Default for ImproveMeshQualitySettings {
    fn default() -> Self {
        ImproveMeshQualitySettings { n_loops: 10, n_iterations: 50, n_surface_iterations: 2, quality_threshold: 0.1, constrained_cells: None }
    }
}

/// What [`improve_mesh_quality`] did, step by step.
#[derive(Debug, Clone, PartialEq)]
pub struct ImproveMeshQualityReport {
    /// `optimizeMeshFV(nLoops, nLoops, nIterations, nSurfaceIterations)`.
    pub optimize_mesh_fv: OptimiserReport,
    /// `optimizeMeshFVBestQuality(nLoops, qualityThreshold)`.
    pub best_quality: OptimiserReport,
    /// `untangleMeshFV(nLoops, nIterations, nSurfaceIterations)`.
    pub untangle: OptimiserReport,
}

/// The untangle controls a plain `meshOptimizer` gets: upstream always runs
/// the boundary loop and the LP untangler.
fn upstream_untangle(global: usize, iterations: usize, surface: usize) -> UntangleControls {
    UntangleControls { max_global_iterations: global, max_iterations: iterations, max_surface_iterations: surface, boundary_moves: true, mesh_untangler: true, ..UntangleControls::default() }
}

/// `improveMeshQuality`: `meshOptimizer` with the constrained cells
/// locked, then `optimizeMeshFV(nLoops, nLoops, nIterations,
/// nSurfaceIterations)`, `optimizeMeshFVBestQuality(nLoops,
/// qualityThreshold)` and `untangleMeshFV(nLoops, nIterations,
/// nSurfaceIterations)`. (Upstream also collects the constrained set's
/// boundary faces into `lockedBndFaces` and never uses them; nothing to
/// port.)
pub fn improve_mesh_quality(mesh: &mut VolumeMesh, s: &ImproveMeshQualitySettings) -> ImproveMeshQualityReport {
    let mut opt = MeshOptimizer::new(mesh);
    if let Some(cells) = &s.constrained_cells {
        opt.lock_cells(mesh, cells);
    }
    let c = upstream_untangle(s.n_loops, s.n_iterations, s.n_surface_iterations);
    // enforce_constraints is off, so the untangles cannot fail.
    let r1 = opt.optimize_mesh_fv(mesh, s.n_loops, &c).unwrap_or_else(|e| e.report);
    let r2 = optimize_mesh_fv_best_quality(mesh, &opt.locked_points(), s.n_loops, s.quality_threshold);
    let r3 = opt.untangle_mesh_fv(mesh, &c).unwrap_or_else(|e| e.report);
    ImproveMeshQualityReport { optimize_mesh_fv: r1, best_quality: r2, untangle: r3 }
}

/// `improveSymmetryPlanes`: `symmetryPlaneOptimisation(mesh)
/// .optimizeSymmetryPlanes()`, with the patch types beside the mesh.
pub fn improve_symmetry_planes(mesh: &mut VolumeMesh, patch_types: &[String]) -> SymmetryReport {
    symmetry_plane_optimisation(mesh, patch_types)
}

/// `generateBoundaryLayers`' settings (`meshDict`'s `boundaryLayers`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenerateBoundaryLayersSettings {
    /// The patches to treat (`patchBoundaryLayers`' keys); `None` is
    /// `addLayerForAllPatches`.
    pub patches: Option<Vec<String>>,
    /// `boundaryLayerHeightFactor` / `boundaryLayerMinHeight`.
    pub controls: BoundaryLayersControls,
    /// `refineBoundaryLayers` settings; `None` when `meshDict` has no
    /// `boundaryLayers` dictionary (no refinement).
    pub refine: Option<RefineBoundaryLayersSettings>,
    /// `boundaryLayers/optimiseLayer`: whether `meshOptimisation`'s
    /// `optimizeBoundaryLayer` does anything. Upstream's default is
    /// `false` (gh:#1280).
    pub optimise_layer: bool,
    /// `boundaryLayers/optimisationParameters` for `optimizeBoundaryLayer`.
    pub layer_optimisation: BoundaryLayerOptimisationControls,
}

/// What [`generate_boundary_layers`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateBoundaryLayersReport {
    /// `generateLayer`; `None` where `boundaryLayers` stops (no face, or a
    /// point at three untreated patches). **Since gh:#1270's remainder**
    /// the patch-wise route (was the one-group `BoundaryLayersOutcome`).
    pub layer: Option<GeneratedLayer>,
    /// `meshOptimisation`: `optimizeMeshFV`, `optimizeLowQualityFaces`,
    /// `untangleMeshFV`, then (after `optimizeBoundaryLayer`)
    /// `untangleMeshFV`.
    pub optimisation: Vec<OptimiserReport>,
    /// `meshOptimisation`'s `optimizeBoundaryLayer` (gh:#1280).
    pub boundary_layer: OptimizeBoundaryLayerReport,
    /// `layerRefinement`'s `refineBoundaryLayers`.
    pub refinement: Option<Result<RefineReport, RefineError>>,
    /// `layerRefinement`'s `untangleBoundaryLayer`, after a refinement
    /// (gh:#1280): `optimizeLowQualityFaces`, `untangleMeshFV(2, 50, 1,
    /// relaxed)`.
    pub untangle_layer: Vec<OptimiserReport>,
}

/// `generateBoundaryLayers` (not 2D): `generateLayer`
/// (~~`boundary_layers_add_layer`, all patches as one group: upstream's
/// patch-wise groups are not ported, see `layers::boundary_layers`~~
/// **UPDATED 2026-11-05, gh:#1270 remainder:** [`generate_layer`],
/// patch-wise as upstream: `addLayerForAllPatches`, or
/// `addLayerForPatch` per named patch),
/// `meshOptimisation`, `layerRefinement` ([`refine_boundary_layers`]).
///
/// ~~**Partial.** `meshOptimizer::optimizeBoundaryLayer` and
/// `untangleBoundaryLayer` exist in this crate only inside the
/// `CfMeshOptimise` layer strategy (`boundary_layer_optimisation`), built
/// around its own layer columns; they are not callable on an arbitrary
/// mesh, so they are skipped and the report says so.~~ **UPDATED
/// 2026-11-05 (gh:#1280):** both are now callable on any mesh
/// ([`optimize_boundary_layer`], [`untangle_boundary_layer`], reusing the
/// strategy's `optimiseLayer`) and run where upstream runs them:
/// `optimizeBoundaryLayer` between `meshOptimisation`'s two
/// `untangleMeshFV`s (it does nothing unless
/// [`GenerateBoundaryLayersSettings::optimise_layer`], as upstream), and
/// `untangleBoundaryLayer` after `refineBoundaryLayers`. Still missing:
/// `optimizeBoundaryLayer`'s buffer layer (`activateSpecialMode`), and 2D.
pub fn generate_boundary_layers(mesh: &VolumeMesh, s: &GenerateBoundaryLayersSettings) -> (VolumeMesh, GenerateBoundaryLayersReport) {
    let request = match &s.patches {
        Some(p) => LayerRequest::Patches(p.clone()),
        None => LayerRequest::AllPatches,
    };
    let (mut m, layer) = match generate_layer(mesh, &request, &s.controls, false) {
        Ok((m, g)) => (m, Some(g)),
        Err(_) => (mesh.clone(), None),
    };
    // meshOptimisation(mesh)
    let opt = MeshOptimizer::new(&m);
    let c = upstream_untangle(10, 50, 2);
    let mut optimisation = Vec::new();
    optimisation.push(opt.optimize_mesh_fv(&mut m, 5, &c).unwrap_or_else(|e| e.report));
    optimisation.push(opt.optimize_low_quality_faces(&mut m, 10));
    optimisation.push(opt.untangle_mesh_fv(&mut m, &c).unwrap_or_else(|e| e.report));
    // optimizeBoundaryLayer() (addBufferLayer = true)
    let bl = OptimizeBoundaryLayerSettings { optimise_layer: s.optimise_layer, add_buffer_layer: true, controls: s.layer_optimisation.clone(), untangle: c };
    let boundary_layer = optimize_boundary_layer(&mut m, &bl);
    let opt = MeshOptimizer::new(&m);
    optimisation.push(opt.untangle_mesh_fv(&mut m, &c).unwrap_or_else(|e| e.report));
    // layerRefinement: refineLayers, then untangleBoundaryLayer
    let mut untangle_layer = Vec::new();
    let refinement = s.refine.as_ref().map(|r| match refine_boundary_layers(&m, r) {
        Ok((refined, rep)) => {
            m = refined;
            untangle_layer = untangle_boundary_layer(&mut m, &c);
            Ok(rep)
        }
        Err(e) => Err(e),
    });
    (m, GenerateBoundaryLayersReport { layer, optimisation, boundary_layer, refinement, untangle_layer })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::checks::cell_geometry;
    use crate::math::Vec3;
    use crate::mesh_optimizer::find_bad_faces;

    fn dented_box() -> VolumeMesh {
        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [8, 8, 8]);
        let p = m.points.iter().position(|q| q.sub(Vec3::new(0.5, 0.5, 0.0)).length() < 1e-12).unwrap();
        m.points[p] = Vec3::new(0.7, 0.6, 0.0);
        m
    }

    /// V&V (gh:#1280), the plan's test, differentially: `improveMeshQuality`
    /// on `mesh_optimizer`'s dented box (gh:#1255's case: an 8×8×8 box with
    /// the boundary point (0.5, 0.5, 0) moved to (0.7, 0.6, 0)) against
    /// the same three calls made by hand from `mesh_optimizer`, and against
    /// item 1's result (`untangleMeshFV` with the boundary loop: 0 bad faces
    /// left). **Predictions (written 2026-11-05, before the first run):**
    /// the utility equals the hand composition bit for bit; it leaves 0
    /// `findBadFaces` faces, as item 1 does; every cell positive.
    /// **Results (2026-11-05): held.** Bit for bit equal; bad faces 3 → 0
    /// (`optimizeMeshFV` 3 → 0 with one boundary pass; best quality worked
    /// on 2 worst faces; the last untangle found none).
    #[test]
    fn improve_mesh_quality_on_the_dented_box() {
        let m0 = dented_box();
        let mut a = m0.clone();
        let rep = improve_mesh_quality(&mut a, &ImproveMeshQualitySettings::default());
        let mut b = m0.clone();
        let opt = MeshOptimizer::new(&b);
        let c = UntangleControls { max_global_iterations: 10, max_iterations: 50, max_surface_iterations: 2, boundary_moves: true, mesh_untangler: true, ..UntangleControls::default() };
        opt.optimize_mesh_fv(&mut b, 10, &c).unwrap();
        optimize_mesh_fv_best_quality(&mut b, &opt.locked_points(), 10, 0.1);
        opt.untangle_mesh_fv(&mut b, &c).unwrap();
        assert_eq!(a.points, b.points);
        let bad = find_bad_faces(&a).iter().filter(|&&x| x).count();
        let bad0 = find_bad_faces(&m0).iter().filter(|&&x| x).count();
        let (vols, _) = cell_geometry(&a);
        println!("improveMeshQuality: bad faces {bad0} -> {bad}; {rep:?}");
        assert_eq!(bad, 0);
        assert!(vols.iter().all(|&v| v > 0.0));
    }

    /// `improveSymmetryPlanes` is `symmetryPlaneOptimisation` (the plan's
    /// flattening test is `symmetry_plane_optimisation`'s): a box whose
    /// `zMin` points are lifted by 1e-3 alternately is flattened to 1e-12.
    #[test]
    fn improve_symmetry_planes_flattens() {
        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let zi = m.patches.iter().position(|p| p.name == "zMin").unwrap();
        let p = m.patches[zi].clone();
        let mut pts: Vec<usize> = (p.start_face..p.start_face + p.n_faces).flat_map(|f| m.faces[f].clone()).collect();
        pts.sort_unstable();
        pts.dedup();
        for (k, &v) in pts.iter().enumerate() {
            m.points[v].z += if k % 2 == 0 { 1e-3 } else { -1e-3 };
        }
        let types: Vec<String> = m.patches.iter().map(|q| if q.name == "zMin" { "symmetryPlane".into() } else { "patch".into() }).collect();
        let r = improve_symmetry_planes(&mut m, &types);
        let pl = r.planes[&zi];
        assert!(pts.iter().all(|&v| pl.distance(m.points[v]) < 1e-12));
    }

    /// V&V (gh:#1280), `generateBoundaryLayers` on a 4×4×4 unit box, all
    /// patches, no refinement. **Predictions (written 2026-11-05, before the
    /// first run):** `boundaryLayers` treats all six patches as one group
    /// (96 faces, one layer cell each): 64 + 96 cells; the optimisation
    /// steps leave 0 bad faces; closed, every cell positive, volume 1 to
    /// 1e-12 (no boundary point moves: the layer is grown inward).
    /// **Results (2026-11-05): held.** ~~160 cells~~, volume 1, smallest cell
    /// 3.08e-3, 0 bad faces (no optimiser step had anything to do).
    /// **CORRECTED 2026-11-05 (gh:#1270 remainder):** "one group" was the
    /// port's stand-in, not upstream: `addLayerForAllPatches` is
    /// patch-wise, and the six faces of a box meet at convex 90° edges, so
    /// they are six groups with edge and corner cells: 64 + 96 + 48 + 8 =
    /// **216 cells** (the patch-wise box test, prediction P1 of
    /// `verification_and_validation/patch_wise_layers_gh1270/`).
    #[test]
    fn generate_boundary_layers_on_a_box() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let (m, rep) = generate_boundary_layers(&m0, &GenerateBoundaryLayersSettings::default());
        m.validate().unwrap();
        let (vols, _) = cell_geometry(&m);
        let bad = find_bad_faces(&m).iter().filter(|&&x| x).count();
        println!("generateBoundaryLayers: cells {} volume {} min vol {:e} bad {bad}; layer {:?} faces; optimisation {:?}", m.n_cells, m.total_volume(), vols.iter().cloned().fold(f64::MAX, f64::min), rep.layer.as_ref().map(|l| l.n_layer_cells), rep.optimisation);
        assert_eq!(m.n_cells, 216);
        assert_eq!(bad, 0);
        assert!(vols.iter().all(|&v| v > 0.0));
        assert!((m.total_volume() - 1.0).abs() < 1e-12);
    }

    /// Hair-length statistics (min, mean, max) of the layer
    /// `detectBoundaryLayers` finds, and the bad-face count.
    fn layer_stats(m: &VolumeMesh) -> (usize, f64, f64, f64, usize) {
        let d = crate::layers::refine_boundary_layers::detect_boundary_layers(m);
        let t: Vec<f64> = d.hair_edges.iter().map(|h| m.points[h[1]].sub(m.points[h[0]]).length()).collect();
        let (mn, mx) = t.iter().fold((f64::MAX, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
        let bad = find_bad_faces(m).iter().filter(|&&x| x).count();
        (t.len(), mn, t.iter().sum::<f64>() / t.len().max(1) as f64, mx, bad)
    }

    /// `generateBoundaryLayers` before (`optimiseLayer` off, upstream's
    /// default) and after (`optimiseLayer` on) on `m0`'s `patches`.
    fn before_after(m0: &VolumeMesh, patches: Option<Vec<String>>, label: &str) -> [(VolumeMesh, GenerateBoundaryLayersReport); 2] {
        let off = GenerateBoundaryLayersSettings { patches: patches.clone(), ..Default::default() };
        let on = GenerateBoundaryLayersSettings { patches, optimise_layer: true, ..Default::default() };
        let a = generate_boundary_layers(m0, &off);
        let b = generate_boundary_layers(m0, &on);
        for (name, (m, r)) in [("off", &a), ("on", &b)] {
            let (n, mn, mean, mx, bad) = layer_stats(m);
            let (vols, _) = cell_geometry(m);
            let vmin = vols.iter().cloned().fold(f64::MAX, f64::min);
            println!(
                "{label} optimiseLayer {name}: cells {} volume {:.12e} min cell vol {vmin:e}; hairs {n}, thickness min {mn:e} mean {mean:e} max {mx:e}; bad faces {bad}; boundary layer {:?}",
                m.n_cells,
                m.total_volume(),
                (r.boundary_layer.ran, r.boundary_layer.n_hairs, r.boundary_layer.n_shared_start, r.boundary_layer.n_layer_cells, r.boundary_layer.passes, r.boundary_layer.n_thinned, &r.boundary_layer.notes)
            );
        }
        [a, b]
    }

    /// V&V (gh:#1280 remainder): `generateBoundaryLayers` now runs
    /// `meshOptimizer::optimizeBoundaryLayer` where upstream does
    /// (`meshOptimisation`, between the two `untangleMeshFV`s), on the 4³
    /// unit box, all patches, with `optimiseLayer` off (upstream's default)
    /// and on.
    ///
    /// **Predictions (written 2026-11-05, before the first run):**
    /// (1) off: identical to `generate_boundary_layers_on_a_box` (160
    /// cells, 0 bad faces, volume 1), and `optimizeBoundaryLayer` does not
    /// run; (2) on: it runs, 160 cells, volume 1 to 1e-12 (wall points do
    /// not move; there is no bad face for an untangle to act on), 0 bad
    /// faces; (3) `optimiseLayer` only shortens hairs (its thickness step
    /// only lowers a length; the normal smoothing keeps lengths), so the
    /// on-run's max and mean hair length are at most the off-run's.
    ///
    /// **Results, first run (2026-11-05, one-group layer): all three
    /// held.** Off: bit for bit the recorded run (160 cells, 0 bad faces,
    /// 98 hairs, thickness min 0.145 / mean 0.186 / max 0.267, smallest
    /// cell 3.08e-3). On: 98 hairs, 96 layer cells, one `optimiseLayer`
    /// pass, 92 hairs thinned; thickness min 0.129 / mean 0.146 / max 0.184;
    /// 0 bad faces; smallest cell 5.01e-3; volume 1.
    ///
    /// **Re-measured 2026-11-05 after merging gh:#1270's remainder**, which
    /// made `generateLayer` patch-wise (216 cells: edge and corner cells
    /// at the box's edges). The predictions were written against the
    /// one-group layer; against this one **(2) missed on the volume**,
    /// (1) and (3) held. Off: 216 cells, 294 hairs (76 share a start point
    /// with another), thickness min 0.125 / mean 0.138 / max 0.167, 0 bad
    /// faces, smallest cell 2.10e-3, volume 1. On: 152 layer cells, 5
    /// `optimiseLayer` passes (the exiting-face loop runs to
    /// `maxNumIterations`), 236 thinned; thickness min **0.0225** / mean
    /// 0.106 / max 0.151; smallest cell **1.12e-4**; 0 bad faces at the end
    /// but **volume 1.00373**. Traced (`diagnose_optimise_layer_on_the_patch_wise_box`):
    /// the edge cells' `BOUNDARY | ATEDGE` hairs lie on a box face and end
    /// where the other face's layer hairs start; `optimiseLayer` thins them
    /// from 0.125 to 2.5e-4 over the passes, two of their end points leave
    /// the box planes (by up to 4e-4), 262 faces go bad, and the untangles
    /// repair them, the last one (`untangleMeshFV(10, 50, 2)`, boundary
    /// moves on) moving boundary points off the box. Whether upstream does
    /// the same on a patch-wise box is **not established** (cfMesh was not
    /// run); a port defect in the hair handling at shared start points
    /// (`Hairs::at_point` keeps one hair per point, upstream keeps all) is
    /// the first suspect. ~~The volume is pinned below as measured, so a fix
    /// shows; it is not a pass criterion.~~ The buffer layer is not ported
    /// and the report says so.
    ///
    /// **FIXED 2026-11-05 (gh:#1293)**, predictions committed before the
    /// run (V&V README, gh:#1293 section): the suspect was the defect.
    /// `optimiseThicknessVariation` compared each hair with the *first*
    /// hair at each point of a base face, of any cell; upstream compares
    /// it with the hairs of the face's own cell (`hairEdgesAtBndFace`).
    /// With upstream's rule: **one** `optimiseLayer` pass, 48 of 294 hairs
    /// thinned, thickness min 0.125 / mean 0.1289 / max 0.1475 (off-run
    /// 0.125 / 0.1382 / 0.1666), 0 bad faces at every stage (was 122 ->
    /// 262), smallest cell 1.95e-3 (was 1.12e-4), **volume 1 to 1e-12 and
    /// every boundary point on its plane**, now asserted as the gate.
    #[test]
    fn generate_boundary_layers_with_optimise_layer_on_the_box() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let [(a, ra), (b, rb)] = before_after(&m0, None, "box 4^3");
        let (plain, _) = generate_boundary_layers(&m0, &GenerateBoundaryLayersSettings::default());
        assert_eq!(a.points, plain.points, "off must be the recorded run");
        assert!(!ra.boundary_layer.ran);
        assert!(rb.boundary_layer.ran);
        b.validate().unwrap();
        assert_eq!(b.n_cells, a.n_cells, "optimizeBoundaryLayer moves points only");
        // ~~Measured, not a criterion: prediction (2) missed on the
        // patch-wise layer (1.003_731_410_684).~~ gh:#1293: the gate, as
        // predicted before the first run: volume 1 and every boundary
        // point on a box plane.
        assert!((b.total_volume() - 1.0).abs() < 1e-12, "volume {}", b.total_volume());
        let eng = crate::surface_engine::SurfaceEngine::new(&b);
        let off_plane = eng
            .boundary_points
            .iter()
            .filter(|&&p| {
                let q = b.points[p];
                [q.x, q.y, q.z].iter().all(|&c| c.abs() > 1e-12 && (c - 1.0).abs() > 1e-12)
            })
            .count();
        assert_eq!(off_plane, 0, "boundary points off the box planes");
        let (sa, sb) = (layer_stats(&a), layer_stats(&b));
        assert_eq!(sb.4, 0, "bad faces");
        assert!(sb.3 <= sa.3 && sb.2 <= sa.2, "optimiseLayer only thins: {sa:?} -> {sb:?}");
    }

    /// V&V (gh:#1280 remainder): as the box test, on the bare pipe (R 10
    /// mm, L 60 mm, h 6 mm, default dual), layer on `walls`.
    ///
    /// **Predictions (written 2026-11-05, before the first run):**
    /// (1) off: `optimizeBoundaryLayer` does not run; the layer has bad
    /// faces left (upstream's layer is not quality-checked and inverts
    /// cells on the pipe, gh:#1270); (2) on: it runs, and leaves fewer bad
    /// faces than off, with mean hair length no larger; not necessarily 0.
    ///
    /// **Results, first run (2026-11-05, one-group layer): (1) missed, (2)
    /// held only in part.** Off: 2025 cells, **0 bad faces** (prediction 1
    /// missed: `meshOptimisation`'s untangles repair what the raw layer
    /// inverts), 1456 hairs, thickness mean 1.160e-3. On: one pass, 1042
    /// thinned, 0 bad faces (not fewer: there were none), mean 9.73e-4,
    /// smallest cell 6.61e-10 -> 3.11e-10.
    ///
    /// **Re-measured 2026-11-05 after merging gh:#1270's remainder**
    /// (patch-wise `generateLayer`), same verdicts. Off: 2261 cells, 1658
    /// hairs, thickness min 5.27e-4 / mean 1.264e-3 / max 2.631e-3, 0 bad
    /// faces, smallest cell 9.37e-10. On: 1656 layer cells, one pass, 1241
    /// thinned; thickness min 5.12e-4 / mean 1.062e-3 (-16 %) / max
    /// 2.448e-3 (-7 %); 0 bad faces; smallest cell 4.77e-10 (**halved**);
    /// volume unchanged to 13 digits.
    #[test]
    fn generate_boundary_layers_with_optimise_layer_on_the_pipe() {
        let m0 = crate::layers::boundary_layers::tests::bare_pipe(0.06, 0.006);
        let [(a, ra), (b, rb)] = before_after(&m0, Some(vec!["walls".into()]), "pipe");
        assert!(!ra.boundary_layer.ran);
        assert!(rb.boundary_layer.ran);
        b.validate().unwrap();
        let (sa, sb) = (layer_stats(&a), layer_stats(&b));
        assert!(sb.2 <= sa.2, "mean hair length {sa:?} -> {sb:?}");
        assert!(sb.4 <= sa.4, "bad faces {} -> {}", sa.4, sb.4);
    }
}
