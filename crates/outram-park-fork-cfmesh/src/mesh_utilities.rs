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
}

/// What [`generate_boundary_layers`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateBoundaryLayersReport {
    /// `generateLayer`; `None` where `boundaryLayers` stops (no face, or a
    /// point at three untreated patches). **Since gh:#1270's remainder**
    /// the patch-wise route (was the one-group `BoundaryLayersOutcome`).
    pub layer: Option<GeneratedLayer>,
    /// `meshOptimisation`: `optimizeMeshFV`, `optimizeLowQualityFaces`,
    /// `untangleMeshFV`, (`optimizeBoundaryLayer`: not run, see
    /// [`generate_boundary_layers`]), `untangleMeshFV`.
    pub optimisation: Vec<OptimiserReport>,
    /// `layerRefinement`'s `refineBoundaryLayers`.
    pub refinement: Option<Result<RefineReport, RefineError>>,
}

/// `generateBoundaryLayers` (not 2D): `generateLayer`
/// (~~`boundary_layers_add_layer`, all patches as one group: upstream's
/// patch-wise groups are not ported, see `layers::boundary_layers`~~
/// **UPDATED 2026-11-05, gh:#1270 remainder:** [`generate_layer`],
/// patch-wise as upstream: `addLayerForAllPatches`, or
/// `addLayerForPatch` per named patch),
/// `meshOptimisation`, `layerRefinement` ([`refine_boundary_layers`]).
///
/// **Partial.** `meshOptimizer::optimizeBoundaryLayer` and
/// `untangleBoundaryLayer` exist in this crate only inside the
/// `CfMeshOptimise` layer strategy (`boundary_layer_optimisation`), built
/// around its own layer columns; they are not callable on an arbitrary
/// mesh, so they are skipped and the report says so. The other steps are
/// upstream's.
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
    optimisation.push(opt.untangle_mesh_fv(&mut m, &c).unwrap_or_else(|e| e.report));
    // layerRefinement
    let refinement = s.refine.as_ref().map(|r| match refine_boundary_layers(&m, r) {
        Ok((refined, rep)) => {
            m = refined;
            Ok(rep)
        }
        Err(e) => Err(e),
    });
    (m, GenerateBoundaryLayersReport { layer, optimisation, refinement })
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
}
