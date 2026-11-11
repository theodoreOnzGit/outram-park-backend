// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/cartesianMesh/cartesianMeshGenerator/cartesianMeshGenerator.{C,H}
//   executables/cartesianMesh/cartesianMesh.C
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

//! cfMesh's **`cartesianMesh`** workflow (`cartesianMeshGenerator`,
//! gh:#1274): a new entry point beside
//! [`crate::pipeline::surface_to_tet_dual_mesh`] and
//! [`crate::pipeline::tet_mesh::tet_mesh`], whose outputs it does not change.
//!
//! The constructor builds the surface octree
//! ([`crate::pipeline::tet_mesh::create_octree`], `createOctreeBoxes`), then
//! `generateMesh` runs the steps below, each behind
//! [`WorkflowControls::run_current_step`], then `renumberMesh` and
//! `renameBoundaryPatches`, as [`crate::pipeline::tet_mesh`] does.
//!
//! | step | upstream | here |
//! |---|---|---|
//! | `templateGeneration` | `cartesianMeshExtractor` (`decomposePolyhedraIntoTetsAndPyrs`, default off) | [`crate::cartesian_mesh::cartesian_mesh_extractor`] (gh:#1266) |
//! | `surfaceTopology` | repeat `checkIrregularSurfaceConnections`, `checkNonMappableCellConnections::removeCells`, `checkCellConnectionsOverFaces::checkCellGroups` while any changes the mesh; then `checkBoundaryFacesSharingTwoEdges::improveTopology` | `topology::{check_and_fix_irregular_connections, remove_non_mappable_cells, check_cell_connections_over_faces, BoundaryFacesSharingTwoEdges}` (gh:#1268). **No `surfaceMorpherCells`**: upstream's `cartesianMeshGenerator` does not call it |
//! | `surfaceProjection` | `meshSurfaceMapper::preMapVertices` (3 iterations), `mapVerticesOntoSurface`, `meshSurfaceOptimizer::untangleSurface` | [`crate::premap_vertices`] (ported here), `surface_optimizer::map_mesh_to_surface` |
//! | `patchAssignment` | `edgeExtractor` | [`extract_patches`]: [`crate::edge_extractor::extract_patches`] (gh:#1269); [`PatchAssignment::NearestTriangle`] is the ablation, as in `tet_mesh` |
//! | `edgeExtraction` | `meshSurfaceEdgeExtractorNonTopo`, then `meshSurfaceOptimizer::optimizeSurface` | `correct_edges` + `surface_mapper` (as `tet_mesh`), `surface_optimizer::optimise_mesh_surface` |
//! | `boundaryLayerGeneration` | `boundaryLayers::addLayerForAllPatches` (**always**, no dict needed) | ~~`layers::boundary_layers::boundary_layers_add_layer` on every patch as **one group**: upstream's route for a single patch; on several patches upstream's patch-wise groups are not ported (noted)~~ **UPDATED 2026-11-05 (gh:#1270 remainder):** `layers::boundary_layers::BoundaryLayers::add_layer_for_all_patches`, patch-wise as upstream (groups, edge and corner cells); [`LayerTopology::OTopology`] keeps the one-group layer callable |
//! | `meshOptimisation` | `meshSurfaceOptimizer::optimizeSurface`; `meshOptimizer::optimizeMeshFV`, `optimizeLowQualityFaces`, `optimizeBoundaryLayer(true)`, `untangleMeshFV`; `projectSurfaceAfterBackScaling` | `optimise_mesh_surface`; `MeshOptimizer` with boundary moves and the LP untangler; `optimizeBoundaryLayer` acts only with `optimiseLayer` (default off, not offered); ~~no `anisotropicSources`, so no back-scaling~~ **UPDATED 2026-11-05 (gh:#1279 remainder):** with [`MeshDict::anisotropic_sources`], [`cartesian_optimise_final_mesh`] on the modified surface, then `revertGeometryModification` and `projectSurfaceAfterBackScaling` ([`crate::anisotropic::revert_and_project`], `optimiseFinalMesh` again on the original surface) |
//! | `boundaryLayerRefinement` | with a `boundaryLayers` dict: `refineBoundaryLayers`, lock `pointsInBndLayer`, `untangleBoundaryLayer` | `layers::refine_boundary_layers`, `points_in_bnd_layer`, `MeshOptimizer` (`optimizeLowQualityFaces`, `removeUserConstraints`, `untangleMeshFV(2, 50, 1, true)`); a [`crate::layers::refine_boundary_layers::RefineError::NotPorted`] is a note and leaves the mesh unrefined |
//! | (always) | `renumberMesh`, `renameBoundaryPatches` | [`crate::renumber_mesh`], [`crate::rename_boundary_patches`] |
//!
//! **`anisotropicSources` (gh:#1279 remainder):** as upstream's
//! constructor, the octree is built on the modified surface
//! ([`crate::anisotropic::modified_surface_points`]) and every step up to
//! the revert maps, extracts and optimises against that surface. A run
//! stopped before `meshOptimisation` returns the mesh still in the
//! modified space, as upstream. Record:
//! `verification_and_validation/anisotropic_gh1279/`.
//!
//! Every gap is a line in [`CartesianMeshReport::notes`]. The ablations are
//! explicit: [`CartesianMeshGeneratorOptions::without_boundary_layer`] and
//! [`CartesianMeshGeneratorOptions::patch_assignment`]. gh:#1283's
//! `final_mesh::optimise_final_mesh` (the tet-dual pipeline's opt-in, with a
//! projection that is not upstream) is not used: `meshOptimisation` calls
//! `MeshOptimizer` as `tet_mesh` does.

use crate::cartesian_mesh::{cartesian_mesh_extractor_from_dict, CartesianMeshOptions};
use crate::correct_edges::correct_edges_between_patches;
use crate::layers::boundary_layers::{generate_layer, BoundaryLayers, BoundaryLayersControls, LayerRequest};
use crate::layers::refine_boundary_layers::{points_in_bnd_layer, refine_boundary_layers, RefineBoundaryLayersSettings, RefineError};
use crate::math::Vec3;
use crate::mesh_octree_creator::MeshDict;
use crate::mesh_optimizer::{MeshOptimizer, UntangleControls};
use crate::patches::{assign_patches_by_region, SurfaceRegions};
use crate::mesh_octree::MeshOctree;
use crate::pipeline::tet_mesh::{create_octree, face_regions, PatchAssignment, SavedMesh};
use crate::premap_vertices::pre_map_vertices;
use crate::rename_boundary_patches::{rename_boundary_patches, RenameBoundary};
use crate::renumber_mesh::{cells_from_mesh, renumber_mesh};
use crate::surface_mapper::map_vertices_onto_surface_patches_given_octree;
use crate::surface_optimizer::{map_mesh_to_surface_octree, optimise_mesh_surface_octree, optimise_mesh_surface};
use crate::topology::{check_and_fix_irregular_connections, check_cell_connections_over_faces, remove_non_mappable_cells, BoundaryFacesSharingTwoEdges, TopoMesh};
use crate::volume_mesh::VolumeMesh;
use crate::workflow_controls::{MeshMetaData, StepDecision, WorkflowControls, WorkflowSettings, WorkflowStep};

/// Settings of [`cartesian_mesh`]: the `meshDict` entries the workflow
/// reads.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianMeshGeneratorOptions {
    /// The octree settings (`maxCellSize`, `boundaryCellSize`, refinement,
    /// `keepCellsIntersectingBoundary`).
    pub dict: MeshDict,
    /// `workflowControls`.
    pub workflow: WorkflowSettings,
    /// `renameBoundary`.
    pub rename_boundary: Option<RenameBoundary>,
    /// `enforceGeometryConstraints` (upstream default `false`).
    pub enforce_geometry_constraints: bool,
    /// `decomposePolyhedraIntoTetsAndPyrs` (upstream default `false`).
    pub decompose_polyhedra_into_tets_and_pyrs: bool,
    /// The `boundaryLayers` dictionary's refinement settings (`nLayers`,
    /// `thicknessRatio`, `maxFirstLayerThickness`, `patchBoundaryLayers`).
    /// `None`: no dictionary, `boundaryLayerRefinement` does nothing.
    pub boundary_layers: Option<RefineBoundaryLayersSettings>,
    /// `boundaryLayers`' creation settings (`blHeightFactor`, minimum).
    pub layer_heights: BoundaryLayersControls,
    /// How `patchAssignment` distributes the boundary faces: upstream's
    /// `edgeExtractor` by default (gh:#1269).
    pub patch_assignment: PatchAssignment,
    /// **Ablation, not upstream:** skip `boundaryLayerGeneration` (upstream
    /// always adds the layer). Default `false`.
    pub without_boundary_layer: bool,
    /// `boundaryLayers`' grouping: patch-wise (upstream's default, gh:#1270
    /// remainder) or [`LayerTopology::OTopology`] (all patches one group:
    /// this route's layer before the patch-wise port, kept for the
    /// differential).
    pub layer_topology: LayerTopology,
    /// `boundaryLayers/optimiseLayer` with its `optimisationParameters`
    /// (gh:#1476): `Some` runs `optimizeBoundaryLayer` in
    /// `optimiseFinalMesh`, `None` (upstream's default, `optimiseLayer` off)
    /// does not.
    pub optimise_layer: Option<crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls>,
}

/// How `boundaryLayers::addLayerForAllPatches` groups the patches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerTopology {
    /// Upstream's default: groups by `findPatchesToBeTreatedTogether`, edge
    /// and corner cells where groups meet.
    #[default]
    PatchWise,
    /// `createOTopologyLayers`: every patch in one group.
    OTopology,
}

/// The `boundaryLayerGeneration` step shared by the workflows:
/// [`generate_layer`] (patch-wise) or, for [`LayerTopology::OTopology`],
/// the same with `createOTopologyLayers` first. Returns the mesh, the
/// layer cells added and the report line.
pub(crate) fn boundary_layer_generation(m: &VolumeMesh, request: &LayerRequest, c: &BoundaryLayersControls, topology: LayerTopology, layers_2d: bool) -> Result<(VolumeMesh, usize, String), String> {
    match topology {
        LayerTopology::PatchWise => {
            let (mm, g) = generate_layer(m, request, c, layers_2d)?;
            Ok((mm, g.n_layer_cells, g.summary()))
        }
        LayerTopology::OTopology => {
            let mut bl = BoundaryLayers::new(m.clone(), *c);
            if layers_2d {
                bl.activate_2d_mode()?;
            }
            bl.create_o_topology_layers();
            let o = bl.add_layer_for_all_patches().map_err(|e| format!("boundaryLayers: {e:?}"))?;
            let n = bl.mesh.n_cells - m.n_cells;
            Ok((bl.mesh, n, format!("createOTopologyLayers (ablation of the patch-wise layer): one group {:?}, {} layer cells", o.groups, o.n_face_cells)))
        }
    }
}

/// `refBoundaryLayers` of `cartesianMesh`, `tetMesh` and `pMesh`:
/// `refineBoundaryLayers` with the dict's settings, lock
/// `pointsInBndLayer`, `untangleBoundaryLayer` (`optimizeLowQualityFaces`,
/// `removeUserConstraints`, `untangleMeshFV(2, 50, 1, true)`). Returns the
/// report line and the refusal, if any (the mesh is then unchanged).
pub(crate) fn ref_boundary_layers(m: &mut VolumeMesh, s: &RefineBoundaryLayersSettings, controls: &UntangleControls) -> Result<(String, Option<String>, usize), String> {
    match refine_boundary_layers(m, s) {
        Ok((mm, rr)) => {
            *m = mm;
            let lp = points_in_bnd_layer(m, &rr);
            let mut opt = MeshOptimizer::new(m);
            opt.lock_points(&lp);
            let a = opt.optimize_low_quality_faces(m, 10);
            opt.remove_user_constraints();
            let c = UntangleControls { max_global_iterations: 2, max_iterations: 50, max_surface_iterations: 1, relaxed: true, ..*controls };
            let b = opt.untangle_mesh_fv(m, &c).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
            Ok((
                format!(
                    "boundaryLayerRefinement: refineBoundaryLayers split {} cells; untangleBoundaryLayer: optimizeLowQualityFaces {} -> {} ({} points locked), untangleMeshFV(2, 50, 1, relaxed) {} -> {}",
                    rr.n_cells_split, a.flagged_before, a.flagged_after, lp.len(), b.flagged_before, b.flagged_after
                ),
                None,
                lp.len(),
            ))
        }
        Err(RefineError::NotPorted(msg)) => Ok((format!("boundaryLayerRefinement: refineBoundaryLayers NOT PORTED for this mesh ({msg}); the layer is left unrefined"), Some(msg), 0)),
    }
}

impl Default for CartesianMeshGeneratorOptions {
    /// `maxCellSize` 1, upstream's defaults elsewhere.
    fn default() -> Self {
        CartesianMeshGeneratorOptions {
            dict: MeshDict::with_max_cell_size(1.0),
            workflow: WorkflowSettings::default(),
            rename_boundary: None,
            enforce_geometry_constraints: false,
            decompose_polyhedra_into_tets_and_pyrs: false,
            boundary_layers: None,
            layer_heights: BoundaryLayersControls::default(),
            without_boundary_layer: false,
            layer_topology: LayerTopology::PatchWise,
            patch_assignment: PatchAssignment::EdgeExtractor,
            optimise_layer: None,
        }
    }
}

/// What [`cartesian_mesh`] did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CartesianMeshReport {
    /// One line per step: what ran, what stood in, what is not ported.
    pub notes: Vec<String>,
    /// The steps that ran (including a step resumed after loading).
    pub steps_run: Vec<WorkflowStep>,
    /// `stopAfter` hit: the workflow stopped after this step.
    pub stopped_after: Option<WorkflowStep>,
    /// The mesh's workflow meta data at the end.
    pub meta: MeshMetaData,
    /// The type of each patch of the mesh.
    pub patch_types: Vec<String>,
    /// Cells of the template.
    pub n_template_cells: usize,
    /// Volume of the template.
    pub template_volume: f64,
    /// Cells `surfaceTopology` removed.
    pub n_cells_removed_by_topology: usize,
    /// Passes of `surfaceTopology`'s loop.
    pub n_surface_topology_passes: usize,
    /// Mean boundary-point distance to the surface before and after
    /// `preMapVertices`.
    pub premap_mean_distance: (f64, f64),
    /// Inverted boundary points `untangleSurface` left after the projection.
    pub n_inverted_after_projection: usize,
    /// Inverted boundary points left after `optimiseMeshSurface`.
    pub n_inverted_after_edge_extraction: usize,
    /// Layer cells `boundaryLayers` added.
    pub n_layer_cells: usize,
    /// Cells of non-positive volume right after the layer (before the
    /// optimiser).
    pub n_nonpositive_after_layer: usize,
    /// Bad faces (`findBadFaces`) before and after `untangleMeshFV` of
    /// `meshOptimisation`.
    pub bad_faces_mesh_optimisation: (usize, usize),
    /// `refineBoundaryLayers` refused (this port's `NotPorted`).
    pub refine_error: Option<String>,
    /// Points locked for `untangleBoundaryLayer`.
    pub n_points_in_bnd_layer: usize,
    /// With `anisotropicSources`: the revert and the projection of
    /// `projectSurfaceAfterBackScaling` (gh:#1279 remainder).
    pub back_scaling: Option<crate::anisotropic::BackScalingReport>,
    /// With `anisotropicSources`: bad faces of the second
    /// `optimiseFinalMesh` (before `optimizeMeshFV`, after `untangleMeshFV`).
    pub bad_faces_after_back_scaling: Option<(usize, usize)>,
}

/// Cells of `m` with non-positive volume.
pub(crate) fn n_nonpositive_cells(m: &VolumeMesh) -> usize {
    let (vols, _) = crate::checks::cell_geometry(m);
    vols.iter().filter(|&&v| v <= 0.0).count()
}

/// `cartesianMeshGenerator::surfacePreparation` on `m` (see the module
/// table). Returns (passes, cells removed). Upstream loops until no check
/// changes the mesh, with no cap; the port stops with an error after 1000
/// passes instead of looping for ever.
pub fn cartesian_surface_preparation(m: &mut VolumeMesh) -> Result<(usize, usize), String> {
    let n0 = m.n_cells;
    let mut w = TopoMesh::from_volume_mesh(m);
    let mut passes = 0;
    loop {
        passes += 1;
        if passes > 1000 {
            return Err("surfacePreparation: no fixed point after 1000 passes".into());
        }
        let mut changed = false;
        if !check_and_fix_irregular_connections(&mut w).bad_vertices.is_empty() {
            changed = true;
        }
        if remove_non_mappable_cells(&mut w) > 0 {
            changed = true;
        }
        if check_cell_connections_over_faces(&mut w) {
            changed = true;
        }
        if !changed {
            break;
        }
    }
    BoundaryFacesSharingTwoEdges::improve_topology(&mut w);
    let (mm, _) = w.into_volume_mesh();
    *m = mm;
    Ok((passes, n0 - m.n_cells))
}

/// `cartesianMeshGenerator::extractPatches`: `edgeExtractor(mesh,
/// octree)`, `extractEdges`, `updateMeshPatches`
/// ([`crate::edge_extractor::extract_patches`], gh:#1269), or the
/// nearest-triangle ablation [`PatchAssignment::NearestTriangle`]. Returns
/// the patch types (`None`: keep one `patch` type per patch) and the note.
pub fn extract_patches(
    m: &mut VolumeMesh,
    o: &MeshOctree,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: Option<&SurfaceRegions>,
    reg: &SurfaceRegions,
    rule: PatchAssignment,
) -> Result<(Option<Vec<String>>, String), String> {
    match rule {
        PatchAssignment::EdgeExtractor => {
            let (mm, ty, er) = crate::edge_extractor::extract_patches(m, o, reg)?;
            *m = mm;
            Ok((
                Some(ty),
                format!(
                    "patchAssignment: edgeExtractor: {} faces moved from the nearest-patch distribution (normal alignment {:?}, topology {:?}, geometry: inverted {:?}, moved {:?}); unset reads {} / {}",
                    er.n_faces_moved_from_initial, er.normal_alignment_moves, er.topology_kept, er.geometry_inverted, er.geometry_moves, er.n_unset_neighbour_patch_reads, er.n_failed_region_searches
                ),
            ))
        }
        PatchAssignment::NearestTriangle => match regions {
            Some(r) => {
                *m = assign_patches_by_region(m, points, tris, r)?;
                Ok((None, "patchAssignment: PatchAssignment::NearestTriangle (ablation of edgeExtractor): nearest-triangle patches".into()))
            }
            None => Ok((None, "patchAssignment: no regions given; one patch".into())),
        },
    }
}

/// `cartesianMeshGenerator::optimiseFinalMesh` without its revert (the
/// caller does that, gh:#1279 remainder): `meshSurfaceOptimizer(mse,
/// octree).optimizeSurface()` against the surface `(points, tris)`, then
/// `meshOptimizer`'s `optimizeMeshFV`, `optimizeLowQualityFaces`,
/// `untangleMeshFV`, with `optimizeBoundaryLayer(modSurfacePtr_ ==
/// nullptr)` between the last two ([`optimize_boundary_layer_step`]; acts
/// only with `optimiseLayer`, off by default; `add_buffer_layer` is the
/// argument). Returns (bad faces before `optimizeMeshFV`, after
/// `untangleMeshFV`) and the report line.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cartesian_optimise_final_mesh(
    m: &mut VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    reg: &SurfaceRegions,
    enforce_geometry_constraints: bool,
    controls: &UntangleControls,
    optimise_layer: Option<&crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls>,
    add_buffer_layer: bool,
    octree: Option<&MeshOctree>,
) -> Result<((usize, usize), String), String> {
    let given = face_regions(m, reg);
    match octree {
        Some(o) => optimise_mesh_surface_octree(m, o, points, tris, reg, &given),
        None => optimise_mesh_surface(m, points, tris, reg, &given),
    };
    let mut opt = MeshOptimizer::new(m);
    if enforce_geometry_constraints {
        opt.set_enforce_constraints("badPoints");
    }
    let r1 = opt.optimize_mesh_fv(m, 5, controls).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
    let r2 = opt.optimize_low_quality_faces(m, 10);
    let bl = optimize_boundary_layer_step(m, optimise_layer, add_buffer_layer, controls, "optimizeBoundaryLayer: optimiseLayer off (upstream default), nothing to do");
    let r3 = opt.untangle_mesh_fv(m, controls).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
    Ok((
        (r1.flagged_before, r3.flagged_after),
        format!(
            "optimizeSurface, then optimizeMeshFV bad faces {} -> {}, optimizeLowQualityFaces {} -> {}, untangleMeshFV {} -> {}; {bl}",
            r1.flagged_before, r1.flagged_after, r2.flagged_before, r2.flagged_after, r3.flagged_before, r3.flagged_after
        ),
    ))
}

/// `meshOptimizer::optimizeBoundaryLayer(addBufferLayer)` inside
/// `optimiseFinalMesh` (gh:#1476): with `optimiseLayer` on
/// (`Some(controls)`, `boundaryLayerOptimisation::readSettings`' values),
/// [`crate::boundary_layer_optimisation::optimize_boundary_layer`]; with it
/// off (`None`, upstream's default) nothing, and the report line is the one
/// the generators wrote before gh:#1476. Returns the report text.
pub(crate) fn optimize_boundary_layer_step(m: &mut VolumeMesh, optimise_layer: Option<&crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls>, add_buffer_layer: bool, untangle: &UntangleControls, off_text: &str) -> String {
    let Some(c) = optimise_layer else { return off_text.to_string() };
    let s = crate::boundary_layer_optimisation::OptimizeBoundaryLayerSettings { optimise_layer: true, add_buffer_layer, controls: c.clone(), untangle: *untangle };
    let r = crate::boundary_layer_optimisation::optimize_boundary_layer(m, &s);
    format!("optimizeBoundaryLayer({add_buffer_layer}): {} hairs, {} layer cells, {} passes, {} hairs thinned; {}", r.n_hairs, r.n_layer_cells, r.passes, r.n_thinned, r.notes.join("; "))
}

/// cfMesh `cartesianMesh`: the workflow on the surface `(points, tris)`
/// with optional named regions (module docs).
pub fn cartesian_mesh(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &CartesianMeshGeneratorOptions) -> Result<(VolumeMesh, CartesianMeshReport), String> {
    run_cartesian(points, tris, regions, opts, None)
}

/// [`cartesian_mesh`] restarted from a mesh saved after a step
/// (`restartFromLatestStep` must be set in `opts.workflow`).
pub fn cartesian_mesh_restart(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &CartesianMeshGeneratorOptions, saved: &SavedMesh) -> Result<(VolumeMesh, CartesianMeshReport), String> {
    run_cartesian(points, tris, regions, opts, Some(saved))
}

/// `cartesianMeshGenerator::generateMesh`.
fn run_cartesian(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &CartesianMeshGeneratorOptions, saved: Option<&SavedMesh>) -> Result<(VolumeMesh, CartesianMeshReport), String> {
    if let Some(r) = regions {
        r.validate(tris.len())?;
    }
    if opts.dict.max_cell_size <= 0.0 {
        return Err(format!("maxCellSize must be > 0 (got {})", opts.dict.max_cell_size));
    }
    // the constructor: with anisotropicSources the octree is built on the
    // modified surface (surfaceMeshGeometryModification), and every step
    // up to the revert works against it
    let mod_points = crate::anisotropic::modified_surface_points(points, opts.dict.anisotropic_sources.as_ref());
    let mut mod_active = mod_points.is_some();
    let spts: &[Vec3] = mod_points.as_deref().unwrap_or(points);
    let o = create_octree(spts, tris, regions, &opts.dict);
    let single = SurfaceRegions::single("defaultFaces", tris.len());
    let reg = regions.unwrap_or(&single);
    let meta = saved.map(|s| s.meta.clone()).unwrap_or_default();
    let mut w = WorkflowControls::new(opts.workflow, meta);
    let mut rep = CartesianMeshReport::default();
    if mod_active {
        rep.notes.push("constructor: anisotropicSources: surfaceMeshGeometryModification, the octree on the modified surface".into());
    }
    let mut mesh: Option<VolumeMesh> = None;
    let mut types: Vec<String> = Vec::new();
    let steps = [
        WorkflowStep::TemplateGeneration,
        WorkflowStep::SurfaceTopology,
        WorkflowStep::SurfaceProjection,
        WorkflowStep::PatchAssignment,
        WorkflowStep::EdgeExtraction,
        WorkflowStep::BoundaryLayerGeneration,
        WorkflowStep::MeshOptimisation,
        WorkflowStep::BoundaryLayerRefinement,
    ];
    let controls = UntangleControls { boundary_moves: true, mesh_untangler: true, ..UntangleControls::default() };
    for step in steps {
        match w.run_current_step(step) {
            StepDecision::Skip => continue,
            StepDecision::Stop => {
                rep.stopped_after = Some(w.current_step);
                rep.meta = w.meta.clone();
                rep.patch_types = types;
                let m = mesh.ok_or("stopped before a mesh existed")?;
                return Ok((m, rep));
            }
            StepDecision::RunAfterLoading => {
                let s = saved.ok_or("restart requested without a saved mesh")?;
                mesh = Some(s.mesh.clone());
                types = s.patch_types.clone();
                rep.notes.push(format!("restart: loaded the mesh saved after {}", w.meta.last_step.map_or("?", |x| x.name())));
            }
            StepDecision::Run => {}
        }
        rep.steps_run.push(step);
        if step == WorkflowStep::TemplateGeneration {
            let cmo = CartesianMeshOptions { decompose_split_hexes: opts.decompose_polyhedra_into_tets_and_pyrs, use_data_boxes: opts.dict.keep_cells_intersecting_boundary };
            let cm = cartesian_mesh_extractor_from_dict(&o, &cmo, &opts.dict);
            rep.n_template_cells = cm.mesh.n_cells;
            rep.template_volume = cm.mesh.total_volume();
            rep.notes.push(format!("templateGeneration: cartesianMeshExtractor: {} cells, {} decomposed", cm.mesh.n_cells, cm.n_decomposed_cells));
            types = vec!["patch".to_string(); cm.mesh.patches.len()];
            mesh = Some(cm.mesh);
            continue;
        }
        let m = mesh.as_mut().ok_or("no mesh: the workflow resumed after a step that saved none")?;
        match step {
            WorkflowStep::SurfaceTopology => {
                let (passes, removed) = cartesian_surface_preparation(m)?;
                types = vec!["patch".to_string(); m.patches.len()];
                rep.n_surface_topology_passes = passes;
                rep.n_cells_removed_by_topology = removed;
                rep.notes.push(format!("surfaceTopology: checkIrregularSurfaceConnections / NonMappable / CellConnectionsOverFaces ({passes} passes, {removed} cells removed), checkBoundaryFacesSharingTwoEdges"));
            }
            WorkflowStep::SurfaceProjection => {
                let pr = pre_map_vertices(m, &o, spts, tris, reg, 3);
                rep.premap_mean_distance = pr.mean_distance;
                let (mm, u) = map_mesh_to_surface_octree(m, &o, spts, tris, reg);
                *m = mm;
                rep.n_inverted_after_projection = u.n_inverted_last;
                rep.notes.push(format!(
                    "surfaceProjection: preMapVertices (mean distance {:.3e} -> {:.3e}), mapVerticesOntoSurface + untangleSurface: inverted points {} -> {}",
                    pr.mean_distance.0, pr.mean_distance.1, u.n_inverted_first, u.n_inverted_last
                ));
            }
            WorkflowStep::PatchAssignment => {
                let (ty, note) = extract_patches(m, &o, spts, tris, regions, reg, opts.patch_assignment)?;
                types = ty.unwrap_or_else(|| reg.types_for(m.patches.len()));
                rep.notes.push(note);
            }
            WorkflowStep::EdgeExtraction => {
                let mut given = face_regions(m, reg);
                {
                    // meshSurfaceEdgeExtractorNonTopo: decomposeBoundaryFaces
                    // (correctEdgesBetweenPatches), then remapBoundaryPoints.
                    // gh:#1385: upstream runs both always, also on a surface
                    // of one region (the port skipped them without `regions`).
                    let r = regions.unwrap_or(reg);
                    let (corrected, g, cr) = correct_edges_between_patches(m, &given, &r.names);
                    given = g;
                    *m = map_vertices_onto_surface_patches_given_octree(&corrected, &o, spts, tris, r, &given);
                    // `replaceBoundary` with as many patches as before keeps
                    // each patch's type (gh:#1483)
                    types.resize(m.patches.len(), "patch".to_string());
                    rep.notes.push(format!(
                        "edgeExtraction: meshSurfaceEdgeExtractorNonTopo (correct_edges + surface_mapper): {} boundary faces split, {} faces fanned, {} cells into pyramids",
                        cr.n_patch_corrected_faces, cr.n_connection_faces, cr.n_decomposed_cells
                    ));
                }
                let u = optimise_mesh_surface_octree(m, &o, spts, tris, reg, &given);
                rep.n_inverted_after_edge_extraction = u.n_inverted_last;
                rep.notes.push(format!("edgeExtraction: optimiseMeshSurface: inverted points {} -> {}", u.n_inverted_first, u.n_inverted_last));
            }
            WorkflowStep::BoundaryLayerGeneration => {
                if opts.without_boundary_layer {
                    rep.notes.push("boundaryLayerGeneration: ABLATED (without_boundary_layer); upstream always adds the layer".into());
                } else {
                    match boundary_layer_generation(m, &LayerRequest::AllPatches, &opts.layer_heights, opts.layer_topology, false) {
                        Ok((mm, n, line)) => {
                            *m = mm;
                            types.resize(m.patches.len(), "patch".to_string());
                            rep.n_layer_cells = n;
                            rep.n_nonpositive_after_layer = n_nonpositive_cells(m);
                            rep.notes.push(format!("boundaryLayerGeneration: {line}; {} non-positive cells", rep.n_nonpositive_after_layer));
                        }
                        Err(e) => rep.notes.push(format!("boundaryLayerGeneration: boundaryLayers stopped ({e}); no layer")),
                    }
                }
            }
            WorkflowStep::MeshOptimisation => {
                // optimiseFinalMesh on the octree's surface (the modified
                // one with anisotropicSources)
                let (bad, line) = cartesian_optimise_final_mesh(m, spts, tris, reg, opts.enforce_geometry_constraints, &controls, opts.optimise_layer.as_ref(), !mod_active, Some(&o))?;
                rep.bad_faces_mesh_optimisation = bad;
                rep.notes.push(format!("meshOptimisation: {line}"));
                // its end: revertGeometryModification while modSurfacePtr_
                // is set (always, with anisotropicSources: the constructor
                // sets it), then projectSurfaceAfterBackScaling
                if let (true, Some(cm)) = (mod_active, opts.dict.anisotropic_sources.as_ref()) {
                    mod_active = false;
                    let b = crate::anisotropic::revert_and_project(m, cm, points, tris);
                    let (bad2, line2) = cartesian_optimise_final_mesh(m, points, tris, reg, opts.enforce_geometry_constraints, &controls, opts.optimise_layer.as_ref(), true, None)?;
                    rep.back_scaling = Some(b);
                    rep.bad_faces_after_back_scaling = Some(bad2);
                    rep.notes.push(format!(
                        "meshOptimisation: revertGeometryModification ({} points); projectSurfaceAfterBackScaling: mapVerticesOntoSurface on the original surface (largest move {:.3e}), optimiseFinalMesh again: {line2}",
                        b.n_points_reverted, b.max_projection_move
                    ));
                }
            }
            WorkflowStep::BoundaryLayerRefinement => match &opts.boundary_layers {
                None => rep.notes.push("boundaryLayerRefinement: no boundaryLayers dict; nothing to do".into()),
                Some(s) => {
                    let (line, refused, n_locked) = ref_boundary_layers(m, s, &controls)?;
                    types.resize(m.patches.len(), "patch".to_string());
                    rep.n_points_in_bnd_layer = n_locked;
                    rep.refine_error = refused;
                    rep.notes.push(line);
                }
            },
            _ => {}
        }
    }
    let m = mesh.ok_or("the workflow produced no mesh")?;
    let r = renumber_mesh(&m, &cells_from_mesh(&m));
    let renamed = rename_boundary_patches(&r.mesh, &types, opts.rename_boundary.as_ref(), false);
    // gh:#1280: renameBoundaryPatches now runs symmetryPlaneOptimisation.
    if !renamed.symmetry.planes.is_empty() {
        let sy = &renamed.symmetry;
        rep.notes.push(format!(
            "renameBoundaryPatches: symmetryPlaneOptimisation on {} plane(s): {} points moved{}, {} invalid faces",
            sy.planes.len(),
            sy.n_points_moved,
            if sy.problematic { " (skipped: a patch is not planar)" } else { "" },
            sy.invalid_faces.len()
        ));
    }
    w.workflow_completed();
    rep.meta = w.meta;
    rep.patch_types = renamed.patch_types;
    Ok((renamed.mesh, rep))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::checks::check_quality;
    use crate::pipeline::tet_mesh::tests::pipe_surface;
    use crate::pipeline::{surface_to_tet_dual_mesh, TetDualOptions};
    use crate::shapes::sphere_surface;
    use crate::snap::snap_to_surface;
    use crate::surface_mapper::cells_with_nonpositive_pyramids;
    use std::f64::consts::PI;
    use std::time::Instant;

    /// Cells of `m` that are hexahedra (6 quads) and touch no boundary point;
    /// and all cells that touch no boundary point.
    pub(crate) fn hexes_away_from_the_wall(m: &VolumeMesh) -> (usize, usize) {
        let mut on_bnd = vec![false; m.points.len()];
        for f in m.n_internal_faces()..m.face_count() {
            for &p in &m.faces[f] {
                on_bnd[p] = true;
            }
        }
        let cf = crate::volume_mesh::cells_faces(m);
        let mut n_hex = 0;
        let mut n_away = 0;
        for c in &cf {
            if c.iter().flatten().any(|&p| on_bnd[p]) {
                continue;
            }
            n_away += 1;
            if c.len() == 6 && c.iter().all(|f| f.len() == 4) {
                n_hex += 1;
            }
        }
        (n_hex, n_away)
    }

    /// Volume enclosed by the triangulated surface (divergence theorem):
    /// the floor any mesh of it can reach.
    pub(crate) fn surface_volume(p: &[Vec3], t: &[[usize; 3]]) -> f64 {
        t.iter().map(|x| p[x[0]].dot(p[x[1]].cross(p[x[2]])) / 6.0).sum()
    }

    fn sphere_exact() -> f64 {
        4.0 / 3.0 * PI * 27.0
    }

    fn pipe_exact(r: f64, l: f64, n: usize) -> f64 {
        0.5 * n as f64 * r * r * (2.0 * PI / n as f64).sin() * l
    }

    /// One row of the differential: route, cells, volume error %, non-positive
    /// cells, non-positive pyramids, max non-orth, max skew, time, validate.
    fn row(name: &str, route: &str, m: &VolumeMesh, exact: f64, dt: f64) -> String {
        let q = check_quality(m);
        format!(
            "| {name} | {route} | {} | {:+.3} % | {} | {} | {:.2}° | {:.3} | {dt:.2} s | {} |",
            m.n_cells,
            (m.total_volume() - exact) / exact * 100.0,
            n_nonpositive_cells(m),
            cells_with_nonpositive_pyramids(m).len(),
            q.max_non_orthogonality_deg,
            q.max_skewness,
            if m.validate().is_ok() { "ok" } else { "FAILS" }
        )
    }

    /// V&V (gh:#1274), predictions C1, C2, C4–C7 of
    /// `verification_and_validation/cartesian_mesh_gh1274/README.md`: the
    /// workflow on the sphere R 3 m (24 x 48, `maxCellSize` 0.5) against
    /// `carve_box` + `snap_to_surface` and the tet-dual pipeline at the same
    /// cell size, with the layer ablated as a control. Asserts what held
    /// (closed, hexes away from the wall); the volume error and the layer's
    /// inverted cells are recorded, not asserted. Results: the README.
    #[test]
    fn diagnose_cartesian_mesh_on_the_sphere() {
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let exact = sphere_exact();
        println!("faceted surface volume {:.6} ({:+.3} % of analytic)", surface_volume(&p, &t), (surface_volume(&p, &t) - exact) / exact * 100.0);
        let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
        let t0 = Instant::now();
        let (m, rep) = cartesian_mesh(&p, &t, None, &opts).unwrap();
        let dt = t0.elapsed().as_secs_f64();
        for n in &rep.notes {
            println!("  {n}");
        }
        println!("template cells {} volume {:.6} ({:+.3} %)", rep.n_template_cells, rep.template_volume, (rep.template_volume - exact) / exact * 100.0);
        let (n_hex, n_away) = hexes_away_from_the_wall(&m);
        println!("hexes away from the wall {n_hex} of {n_away}");
        println!("{}", row("sphere", "cartesian_mesh", &m, exact, dt));
        let ablated = CartesianMeshGeneratorOptions { without_boundary_layer: true, ..opts.clone() };
        let t0 = Instant::now();
        let (ma, _) = cartesian_mesh(&p, &t, None, &ablated).unwrap();
        println!("{}", row("sphere", "cartesian_mesh without layer", &ma, exact, t0.elapsed().as_secs_f64()));
        println!("layer volume change (with - without) / without: {:.3e}", (m.total_volume() - ma.total_volume()) / ma.total_volume());
        let t0 = Instant::now();
        let cs = snap_to_surface(&carve_box(&p, &t, 0.5), &p, &t);
        println!("{}", row("sphere", "carve_box + snap", &cs, exact, t0.elapsed().as_secs_f64()));
        let t0 = Instant::now();
        let td = TetDualOptions { cell_size: 0.5, n_layers: 0, ..Default::default() };
        let (tdm, _) = surface_to_tet_dual_mesh(&p, &t, &td).unwrap();
        println!("{}", row("sphere", "tet-dual (no layers)", &tdm, exact, t0.elapsed().as_secs_f64()));
        m.validate().unwrap();
        assert_eq!(n_hex, n_away, "every cell away from the wall is a hex");
        assert!(n_away > 0);
    }

    /// V&V (gh:#1274), prediction C8: the pipe (R 10 mm, L 60 mm, 48 sides,
    /// `maxCellSize` 2 mm) and the Foam Studio pipe (L 200 mm, `maxCellSize`
    /// 5 mm, and again with `boundaryCellSize` 1.25 mm), same differential;
    /// the baselines run at the boundary cell size. Results: the README.
    #[test]
    fn diagnose_cartesian_mesh_on_the_pipes() {
        for (name, l, h, hb) in [("pipe 60 mm", 0.06, 0.002, None), ("Foam Studio pipe 200 mm", 0.2, 0.005, None), ("Foam Studio pipe 200 mm, boundaryCellSize 1.25 mm", 0.2, 0.005, Some(0.00125))] {
            let (p, t, r) = pipe_surface(0.01, l, 48);
            let exact = pipe_exact(0.01, l, 48);
            println!("{name}: faceted surface volume rel. to the 48-gon prism {:+.3e}", (surface_volume(&p, &t) - exact) / exact);
            let opts = CartesianMeshGeneratorOptions { dict: MeshDict { boundary_cell_size: hb, ..MeshDict::with_max_cell_size(h) }, ..Default::default() };
            let t0 = Instant::now();
            let (m, rep) = cartesian_mesh(&p, &t, Some(&r), &opts).unwrap();
            let dt = t0.elapsed().as_secs_f64();
            println!("{name}:");
            for n in &rep.notes {
                println!("  {n}");
            }
            println!("  patches {:?}", m.patches.iter().map(|x| (x.name.clone(), x.n_faces)).collect::<Vec<_>>());
            let (n_hex, n_away) = hexes_away_from_the_wall(&m);
            println!("  hexes away from the wall {n_hex} of {n_away}");
            println!("{}", row(name, "cartesian_mesh", &m, exact, dt));
            let ablated = CartesianMeshGeneratorOptions { without_boundary_layer: true, ..opts.clone() };
            let t0 = Instant::now();
            let (ma, _) = cartesian_mesh(&p, &t, Some(&r), &ablated).unwrap();
            println!("{}", row(name, "cartesian_mesh without layer", &ma, exact, t0.elapsed().as_secs_f64()));
            println!("  layer volume change (with - without) / without: {:.3e}; non-positive right after the layer {}", (m.total_volume() - ma.total_volume()) / ma.total_volume(), rep.n_nonpositive_after_layer);
            let h = hb.unwrap_or(h);
            let t0 = Instant::now();
            let cs = snap_to_surface(&carve_box(&p, &t, h), &p, &t);
            println!("{}", row(name, "carve_box + snap", &cs, exact, t0.elapsed().as_secs_f64()));
            let t0 = Instant::now();
            let td = TetDualOptions { cell_size: h, n_layers: 0, ..Default::default() };
            let (tdm, _) = surface_to_tet_dual_mesh(&p, &t, &td).unwrap();
            println!("{}", row(name, "tet-dual (no layers)", &tdm, exact, t0.elapsed().as_secs_f64()));
            m.validate().unwrap();
        }
    }

    /// V&V (gh:#1274), prediction C9: with a `boundaryLayers` dict of
    /// `nLayers` 3, `refineBoundaryLayers` splits the sphere's one-group
    /// layer (one base face per layer cell); on the pipe the rim cells
    /// carry two base faces and this port refuses (`NotPorted`), as its
    /// docs say. Results: the README. **UPDATED 2026-11-05 (gh:#1271
    /// remainder):** the patch-wise layer's rim edge cells are split by
    /// `refineEdgeHexCell` now (prediction R3 of
    /// `patch_wise_layers_gh1270/README.md`): 5 248 cells, closed, 0
    /// non-positive, asserted.
    #[test]
    fn cartesian_mesh_with_refined_layers() {
        let s = RefineBoundaryLayersSettings { n_layers: 3, thickness_ratio: 1.2, ..Default::default() };
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), boundary_layers: Some(s.clone()), ..Default::default() };
        let (m, rep) = cartesian_mesh(&p, &t, None, &opts).unwrap();
        for n in &rep.notes {
            println!("  sphere: {n}");
        }
        println!("{}", row("sphere", "cartesian_mesh nLayers 3", &m, sphere_exact(), 0.0));
        m.validate().unwrap();
        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.002), boundary_layers: Some(s), ..Default::default() };
        let (pm, prep) = cartesian_mesh(&pp, &pt, Some(&pr), &opts).unwrap();
        for n in &prep.notes {
            println!("  pipe: {n}");
        }
        println!("{}", row("pipe 60 mm", "cartesian_mesh nLayers 3", &pm, pipe_exact(0.01, 0.06, 48), 0.0));
        pm.validate().unwrap();
        println!("refine errors: sphere {:?}, pipe {:?}", rep.refine_error, prep.refine_error);
        assert_eq!((rep.refine_error.as_ref(), prep.refine_error.as_ref()), (None, None));
        assert_eq!((m.n_cells, pm.n_cells), (2184, 5248));
        assert_eq!(n_nonpositive_cells(&pm), 0);
        let e = (pm.total_volume() - pipe_exact(0.01, 0.06, 48)) / pipe_exact(0.01, 0.06, 48);
        println!("pipe volume error {:+.6e}", e);
    }

    /// V&V (gh:#1274): `stopAfter` and restart. A run stopped after
    /// `edgeExtraction`, restarted from its saved mesh, gives the
    /// uninterrupted run's mesh bit for bit; two runs agree bit for bit.
    #[test]
    fn optimise_layer_runs_optimize_boundary_layer() {
        // gh:#1476: `optimiseLayer on` runs optimizeBoundaryLayer inside
        // optimiseFinalMesh (cartesianMesh: with the buffer layer asked for,
        // which the port reports it does not build; tetMesh: without).
        let (p, t, r) = pipe_surface(0.01, 0.06, 24);
        let ctl = crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls::default();
        let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.004), optimise_layer: Some(ctl.clone()), ..Default::default() };
        let (m, rep) = cartesian_mesh(&p, &t, Some(&r), &opts).unwrap();
        let line = rep.notes.iter().find(|n| n.starts_with("meshOptimisation")).unwrap();
        println!("{line}");
        assert!(line.contains("optimizeBoundaryLayer(true):") && line.contains("addBufferLayer"), "{line}");
        assert!(m.n_cells > 0);
        let to = crate::pipeline::tet_mesh::TetMeshOptions { dict: MeshDict::with_max_cell_size(0.006), optimise_layer: Some(ctl), ..Default::default() };
        let (_, trep) = crate::pipeline::tet_mesh::tet_mesh(&p, &t, Some(&r), &to).unwrap();
        let tline = trep.notes.iter().find(|n| n.starts_with("meshOptimisation")).unwrap();
        assert!(tline.contains("optimizeBoundaryLayer(false):"), "{tline}");
    }

    #[test]
    fn cartesian_mesh_stop_and_restart_reproduce_the_full_run() {
        let (p, t, r) = pipe_surface(0.01, 0.06, 24);
        let base = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.004), ..Default::default() };
        let (full, rep) = cartesian_mesh(&p, &t, Some(&r), &base).unwrap();
        assert!(rep.meta == MeshMetaData::default());
        let stop = CartesianMeshGeneratorOptions { workflow: WorkflowSettings { stop_after: Some(WorkflowStep::EdgeExtraction), restart_from_latest_step: false }, ..base.clone() };
        let (stopped, srep) = cartesian_mesh(&p, &t, Some(&r), &stop).unwrap();
        assert_eq!(srep.stopped_after, Some(WorkflowStep::EdgeExtraction));
        let saved = SavedMesh { mesh: stopped, patch_types: srep.patch_types.clone(), meta: srep.meta.clone() };
        let restart = CartesianMeshGeneratorOptions { workflow: WorkflowSettings { stop_after: None, restart_from_latest_step: true }, ..base.clone() };
        let (resumed, rrep) = cartesian_mesh_restart(&p, &t, Some(&r), &restart, &saved).unwrap();
        assert_eq!(rrep.steps_run.first(), Some(&WorkflowStep::BoundaryLayerGeneration));
        assert!(crate::tet_mesh_extractor_octree::same_mesh(&resumed, &full));
        let (again, _) = cartesian_mesh(&p, &t, Some(&r), &base).unwrap();
        assert!(crate::tet_mesh_extractor_octree::same_mesh(&again, &full));
        let names: Vec<&str> = full.patches.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["inlet", "outlet", "walls"]);
    }

    /// V&V (gh:#1270 remainder), prediction P7 of
    /// `verification_and_validation/patch_wise_layers_gh1270/README.md`:
    /// the patch-wise layer (upstream's default) against the one-group
    /// O-topology ablation. On the sphere (one patch) the two are the same
    /// mesh bit for bit; on the 60 mm pipe at 2 mm the rows of both are
    /// recorded. Results: the README.
    #[test]
    fn cartesian_mesh_patch_wise_against_o_topology() {
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.5), ..Default::default() };
        let (m, _) = cartesian_mesh(&p, &t, None, &opts).unwrap();
        let o_top = CartesianMeshGeneratorOptions { layer_topology: LayerTopology::OTopology, ..opts.clone() };
        let (mo, _) = cartesian_mesh(&p, &t, None, &o_top).unwrap();
        println!("{}", row("sphere", "cartesian_mesh (patch-wise)", &m, sphere_exact(), 0.0));
        // Not `same_mesh`: the patch-wise route numbers its points as
        // upstream (new labels appended), the one-group route as
        // `build_layered_variable` does (columns last), so the labels differ.
        // The geometry: the sorted point set and the sorted cell volumes.
        let sorted_points = |x: &VolumeMesh| {
            let mut v: Vec<[u64; 3]> = x.points.iter().map(|q| [q.x.to_bits(), q.y.to_bits(), q.z.to_bits()]).collect();
            v.sort_unstable();
            v
        };
        let sorted_vols = |x: &VolumeMesh| {
            let mut v = crate::checks::cell_geometry(x).0;
            v.sort_by(f64::total_cmp);
            v
        };
        let (a, b) = (sorted_vols(&m), sorted_vols(&mo));
        let worst = a.iter().zip(&b).map(|(x, y)| (x - y).abs() / x.abs().max(1e-300)).fold(0.0f64, f64::max);
        let same_points = sorted_points(&m) == sorted_points(&mo);
        println!("sphere: patch-wise against O-topology: same mesh (labels) {}, same point set bit for bit {same_points}, worst relative cell-volume difference {worst:.3e}", crate::tet_mesh_extractor_octree::same_mesh(&m, &mo));
        assert_eq!(m.n_cells, mo.n_cells);
        assert!(worst < 1e-9);
        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        let exact = pipe_exact(0.01, 0.06, 48);
        for topo in [LayerTopology::PatchWise, LayerTopology::OTopology] {
            let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.002), layer_topology: topo, ..Default::default() };
            let t0 = Instant::now();
            let (pm, rep) = cartesian_mesh(&pp, &pt, Some(&pr), &opts).unwrap();
            for n in rep.notes.iter().filter(|n| n.starts_with("boundaryLayerGeneration") || n.starts_with("meshOptimisation")) {
                println!("  {topo:?}: {n}");
            }
            println!("{}", row("pipe 60 mm, 2 mm", &format!("cartesian_mesh {topo:?}"), &pm, exact, t0.elapsed().as_secs_f64()));
            pm.validate().unwrap();
            assert_eq!(n_nonpositive_cells(&pm), 0);
        }
    }

    /// Images (gh:#1270 remainder), `--ignored`: drawn from the assembled
    /// meshes into `verification_and_validation/patch_wise_layers_gh1270/`.
    /// Boundary faces cut by the plane are coloured by patch (inlet red,
    /// walls blue, outlet green, others grey); faces of non-positive cells
    /// orange.
    #[test]
    #[ignore]
    fn draw_patch_wise_layer_images() {
        use crate::layers::boundary_layers::tests::{named_box, six_patch_box};
        use crate::layers::boundary_layers::{BoundaryLayers, BoundaryLayersControls};
        use crate::topology::tests::write_slice_png;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/patch_wise_layers_gh1270");
        let colour_of = |m: &VolumeMesh| {
            let (vols, _) = crate::checks::cell_geometry(m);
            let mut patch = vec![None; m.face_count()];
            for p in &m.patches {
                let c = match p.name.as_str() {
                    "inlet" | "x0" => [214u8, 39, 40],
                    "walls" | "y0" | "y1" | "z0" | "z1" => [31, 119, 180],
                    "outlet" | "x1" => [44, 160, 44],
                    _ => [127, 127, 127],
                };
                for f in p.start_face..p.start_face + p.n_faces {
                    patch[f] = Some(c);
                }
            }
            let per_face: Vec<Option<[u8; 3]>> = (0..m.face_count())
                .map(|f| if vols[m.owner[f]] <= 0.0 || m.neighbour[f].is_some_and(|n| vols[n] <= 0.0) { Some([255u8, 127, 14]) } else { patch[f] })
                .collect();
            move |f: usize| per_face[f]
        };
        let bw = [-0.05, 1.05, -0.05, 1.05];
        let mut b6 = BoundaryLayers::new(six_patch_box(0.25), BoundaryLayersControls::default());
        b6.add_layer_for_all_patches().unwrap();
        let c = colour_of(&b6.mesh);
        write_slice_png(&b6.mesh, 0, 0.375, bw, &[], &c, "box, one patch per side, patch-wise layer: y-z at x 0.375 (216 cells)", &dir.join("box6_patch_wise_yz.png"));
        let mut b3 = BoundaryLayers::new(named_box(0.25), BoundaryLayersControls::default());
        b3.add_layer_for_all_patches().unwrap();
        let c = colour_of(&b3.mesh);
        write_slice_png(&b3.mesh, 1, 0.375, bw, &[], &c, "box walls/inlet/outlet, patch-wise: x-z at y 0.375 (192 cells)", &dir.join("box3_patch_wise_xz.png"));
        let mut b3o = BoundaryLayers::new(named_box(0.25), BoundaryLayersControls::default());
        b3o.create_o_topology_layers();
        b3o.add_layer_for_all_patches().unwrap();
        let c = colour_of(&b3o.mesh);
        write_slice_png(&b3o.mesh, 1, 0.375, bw, &[], &c, "box walls/inlet/outlet, O-topology (one group): x-z at y 0.375 (160 cells)", &dir.join("box3_o_topology_xz.png"));

        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        for topo in [LayerTopology::PatchWise, LayerTopology::OTopology] {
            for (stage, stop) in [("layer_stage", Some(WorkflowStep::BoundaryLayerGeneration)), ("final", None)] {
                let opts = CartesianMeshGeneratorOptions {
                    dict: MeshDict::with_max_cell_size(0.002),
                    layer_topology: topo,
                    workflow: WorkflowSettings { stop_after: stop, restart_from_latest_step: false },
                    ..Default::default()
                };
                let (m, _) = cartesian_mesh(&pp, &pt, Some(&pr), &opts).unwrap();
                let c = colour_of(&m);
                let tag = format!("{topo:?}").to_lowercase();
                let t = format!("cartesian_mesh pipe 60 mm at 2 mm, {topo:?}, {stage}: {} cells, {} non-positive (orange)", m.n_cells, n_nonpositive_cells(&m));
                write_slice_png(&m, 1, 1e-4, [0.0065, 0.0105, -0.0005, 0.0035], &[], &c, &t, &dir.join(format!("pipe_{tag}_{stage}_xz_rim.png")));
                write_slice_png(&m, 2, 0.0317, [-0.0108, 0.0108, -0.0108, 0.0108], &[(0.0, 0.0, 0.01)], &c, &t, &dir.join(format!("pipe_{tag}_{stage}_xy.png")));
            }
        }
        // gh:#1271 remainder: the refined layers (nLayers 3, ratio 1.2)
        let s3 = RefineBoundaryLayersSettings { n_layers: 3, thickness_ratio: 1.2, ..Default::default() };
        let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(0.002), boundary_layers: Some(s3.clone()), ..Default::default() };
        let (m, _) = cartesian_mesh(&pp, &pt, Some(&pr), &opts).unwrap();
        let c = colour_of(&m);
        let t = format!("cartesian_mesh pipe 60 mm, patch-wise, nLayers 3 ratio 1.2: {} cells, {} non-positive (orange)", m.n_cells, n_nonpositive_cells(&m));
        write_slice_png(&m, 1, 1e-4, [0.0065, 0.0105, -0.0005, 0.0035], &[], &c, &t, &dir.join("pipe_refined_3_xz_rim.png"));
        write_slice_png(&m, 2, 0.0317, [-0.0108, 0.0108, -0.0108, 0.0108], &[(0.0, 0.0, 0.01)], &c, &t, &dir.join("pipe_refined_3_xy.png"));
        let (b6r, _) = crate::layers::refine_boundary_layers::refine_boundary_layers(&b6.mesh, &RefineBoundaryLayersSettings { n_layers: 3, ..Default::default() }).unwrap();
        let c = colour_of(&b6r);
        write_slice_png(&b6r, 0, 0.0625, bw, &[], &c, "six-patch box, patch-wise layer refined into 3: y-z at x 0.0625 (through the x0 layer: edge and corner hexes)", &dir.join("box6_refined_3_yz.png"));
    }
}
