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
//! | `patchAssignment` | `edgeExtractor` | **stand-in** ([`extract_patches`]): `patches::assign_patches_by_region` (nearest triangle), as `tet_mesh`; the hook for gh:#1269 |
//! | `edgeExtraction` | `meshSurfaceEdgeExtractorNonTopo`, then `meshSurfaceOptimizer::optimizeSurface` | `correct_edges` + `surface_mapper` (as `tet_mesh`), `surface_optimizer::optimise_mesh_surface` |
//! | `boundaryLayerGeneration` | `boundaryLayers::addLayerForAllPatches` (**always**, no dict needed) | `layers::boundary_layers::boundary_layers_add_layer` on every patch as **one group**: upstream's route for a single patch; on several patches upstream's patch-wise groups are not ported (noted) |
//! | `meshOptimisation` | `meshSurfaceOptimizer::optimizeSurface`; `meshOptimizer::optimizeMeshFV`, `optimizeLowQualityFaces`, `optimizeBoundaryLayer(true)`, `untangleMeshFV`; `projectSurfaceAfterBackScaling` | `optimise_mesh_surface`; `MeshOptimizer` with boundary moves and the LP untangler; `optimizeBoundaryLayer` acts only with `optimiseLayer` (default off, not offered); no `anisotropicSources`, so no back-scaling |
//! | `boundaryLayerRefinement` | with a `boundaryLayers` dict: `refineBoundaryLayers`, lock `pointsInBndLayer`, `untangleBoundaryLayer` | `layers::refine_boundary_layers`, `points_in_bnd_layer`, `MeshOptimizer` (`optimizeLowQualityFaces`, `removeUserConstraints`, `untangleMeshFV(2, 50, 1, true)`); a [`crate::layers::refine_boundary_layers::RefineError::NotPorted`] is a note and leaves the mesh unrefined |
//! | (always) | `renumberMesh`, `renameBoundaryPatches` | [`crate::renumber_mesh`], [`crate::rename_boundary_patches`] |
//!
//! Every gap is a line in [`CartesianMeshReport::notes`]. The one ablation
//! is explicit: [`CartesianMeshGeneratorOptions::without_boundary_layer`].

use crate::cartesian_mesh::{cartesian_mesh_extractor, CartesianMeshOptions};
use crate::correct_edges::correct_edges_between_patches;
use crate::layers::boundary_layers::{boundary_layers_add_layer, BoundaryLayersControls};
use crate::layers::refine_boundary_layers::{points_in_bnd_layer, refine_boundary_layers, RefineBoundaryLayersSettings, RefineError};
use crate::math::Vec3;
use crate::mesh_octree_creator::MeshDict;
use crate::mesh_optimizer::{MeshOptimizer, UntangleControls};
use crate::patches::{assign_patches_by_region, SurfaceRegions};
use crate::pipeline::tet_mesh::{create_octree, face_regions, SavedMesh};
use crate::premap_vertices::pre_map_vertices;
use crate::rename_boundary_patches::{rename_boundary_patches, RenameBoundary};
use crate::renumber_mesh::{cells_from_mesh, renumber_mesh};
use crate::surface_mapper::map_vertices_onto_surface_patches_given;
use crate::surface_optimizer::{map_mesh_to_surface, optimise_mesh_surface};
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
    /// **Ablation, not upstream:** skip `boundaryLayerGeneration` (upstream
    /// always adds the layer). Default `false`.
    pub without_boundary_layer: bool,
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

/// `cartesianMeshGenerator::extractPatches`: upstream runs `edgeExtractor`
/// (gh:#1269, being ported). **Hook:** when it lands, call it here; until
/// then the nearest triangle stands in, as in `tet_mesh`. Returns the note.
pub fn extract_patches(m: &mut VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>) -> Result<String, String> {
    match regions {
        Some(r) => {
            *m = assign_patches_by_region(m, points, tris, r)?;
            Ok("patchAssignment: edgeExtractor not ported (gh:#1269); nearest-triangle patches (patches::assign_patches_by_region)".into())
        }
        None => Ok("patchAssignment: no regions given; one patch".into()),
    }
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
    let o = create_octree(points, tris, regions, &opts.dict);
    let single = SurfaceRegions::single("defaultFaces", tris.len());
    let reg = regions.unwrap_or(&single);
    let meta = saved.map(|s| s.meta.clone()).unwrap_or_default();
    let mut w = WorkflowControls::new(opts.workflow, meta);
    let mut rep = CartesianMeshReport::default();
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
            let cm = cartesian_mesh_extractor(&o, &cmo);
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
                let pr = pre_map_vertices(m, &o, points, tris, reg, 3);
                rep.premap_mean_distance = pr.mean_distance;
                let (mm, u) = map_mesh_to_surface(m, points, tris, reg);
                *m = mm;
                rep.n_inverted_after_projection = u.n_inverted_last;
                rep.notes.push(format!(
                    "surfaceProjection: preMapVertices (mean distance {:.3e} -> {:.3e}), mapVerticesOntoSurface + untangleSurface: inverted points {} -> {}",
                    pr.mean_distance.0, pr.mean_distance.1, u.n_inverted_first, u.n_inverted_last
                ));
            }
            WorkflowStep::PatchAssignment => {
                let note = extract_patches(m, points, tris, regions)?;
                types = vec!["patch".to_string(); m.patches.len()];
                rep.notes.push(note);
            }
            WorkflowStep::EdgeExtraction => {
                let mut given = face_regions(m, reg);
                if let Some(r) = regions {
                    // meshSurfaceEdgeExtractorNonTopo: decomposeBoundaryFaces
                    // (correctEdgesBetweenPatches), then remapBoundaryPoints
                    let (corrected, g, cr) = correct_edges_between_patches(m, &given, &r.names);
                    given = g;
                    *m = map_vertices_onto_surface_patches_given(&corrected, points, tris, r, &given);
                    types = vec!["patch".to_string(); m.patches.len()];
                    rep.notes.push(format!(
                        "edgeExtraction: meshSurfaceEdgeExtractorNonTopo (correct_edges + surface_mapper): {} boundary faces split, {} faces fanned, {} cells into pyramids",
                        cr.n_patch_corrected_faces, cr.n_connection_faces, cr.n_decomposed_cells
                    ));
                }
                let u = optimise_mesh_surface(m, points, tris, reg, &given);
                rep.n_inverted_after_edge_extraction = u.n_inverted_last;
                rep.notes.push(format!("edgeExtraction: optimiseMeshSurface: inverted points {} -> {}", u.n_inverted_first, u.n_inverted_last));
            }
            WorkflowStep::BoundaryLayerGeneration => {
                if opts.without_boundary_layer {
                    rep.notes.push("boundaryLayerGeneration: ABLATED (without_boundary_layer); upstream always adds the layer".into());
                } else {
                    let names: Vec<String> = m.patches.iter().map(|p| p.name.clone()).collect();
                    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                    match boundary_layers_add_layer(m, &refs, &opts.layer_heights) {
                        Some((mm, out)) => {
                            *m = mm;
                            types = vec!["patch".to_string(); m.patches.len()];
                            rep.n_layer_cells = out.wall_faces.len();
                            rep.n_nonpositive_after_layer = n_nonpositive_cells(m);
                            rep.notes.push(format!(
                                "boundaryLayerGeneration: addLayerForAllPatches as one group over {names:?}: {} layer cells, {} non-positive cells{}",
                                rep.n_layer_cells,
                                rep.n_nonpositive_after_layer,
                                if names.len() > 1 { "; upstream's patch-wise groups (findPatchesToBeTreatedTogether, otherVrts_) are NOT ported, so the layer at feature edges between patches differs from upstream" } else { "" }
                            ));
                        }
                        None => rep.notes.push("boundaryLayerGeneration: boundaryLayers refused (no treated face, or a point at three untreated patches); no layer".into()),
                    }
                }
            }
            WorkflowStep::MeshOptimisation => {
                let given = face_regions(m, reg);
                // meshSurfaceOptimizer(mse, octree).optimizeSurface()
                optimise_mesh_surface(m, points, tris, reg, &given);
                let mut opt = MeshOptimizer::new(m);
                if opts.enforce_geometry_constraints {
                    opt.set_enforce_constraints("badPoints");
                }
                let r1 = opt.optimize_mesh_fv(m, 5, &controls).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
                let r2 = opt.optimize_low_quality_faces(m, 10);
                let r3 = opt.untangle_mesh_fv(m, &controls).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
                rep.bad_faces_mesh_optimisation = (r1.flagged_before, r3.flagged_after);
                rep.notes.push(format!(
                    "meshOptimisation: optimizeSurface, then optimizeMeshFV bad faces {} -> {}, optimizeLowQualityFaces {} -> {}, untangleMeshFV {} -> {}; optimizeBoundaryLayer: optimiseLayer off (upstream default), nothing to do; no anisotropicSources, no back-scaling",
                    r1.flagged_before, r1.flagged_after, r2.flagged_before, r2.flagged_after, r3.flagged_before, r3.flagged_after
                ));
            }
            WorkflowStep::BoundaryLayerRefinement => match &opts.boundary_layers {
                None => rep.notes.push("boundaryLayerRefinement: no boundaryLayers dict; nothing to do".into()),
                Some(s) => match refine_boundary_layers(m, s) {
                    Ok((mm, rr)) => {
                        *m = mm;
                        types.resize(m.patches.len(), "patch".to_string());
                        let lp = points_in_bnd_layer(m, &rr);
                        rep.n_points_in_bnd_layer = lp.len();
                        // untangleBoundaryLayer (untangleLayers, default true)
                        let mut opt = MeshOptimizer::new(m);
                        opt.lock_points(&lp);
                        let a = opt.optimize_low_quality_faces(m, 10);
                        opt.remove_user_constraints();
                        let c = UntangleControls { max_global_iterations: 2, max_iterations: 50, max_surface_iterations: 1, relaxed: true, ..controls };
                        let b = opt.untangle_mesh_fv(m, &c).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
                        rep.notes.push(format!(
                            "boundaryLayerRefinement: refineBoundaryLayers split {} cells; untangleBoundaryLayer: optimizeLowQualityFaces {} -> {} ({} points locked), untangleMeshFV(2, 50, 1, relaxed) {} -> {}",
                            rr.n_cells_split, a.flagged_before, a.flagged_after, lp.len(), b.flagged_before, b.flagged_after
                        ));
                    }
                    Err(RefineError::NotPorted(msg)) => {
                        rep.notes.push(format!("boundaryLayerRefinement: refineBoundaryLayers NOT PORTED for this mesh ({msg}); the layer is left unrefined"));
                        rep.refine_error = Some(msg);
                    }
                },
            },
            _ => {}
        }
    }
    let m = mesh.ok_or("the workflow produced no mesh")?;
    let r = renumber_mesh(&m, &cells_from_mesh(&m));
    let renamed = rename_boundary_patches(&r.mesh, &types, opts.rename_boundary.as_ref(), false);
    if !renamed.symmetry_planes_not_optimised.is_empty() {
        rep.notes.push(format!("renameBoundaryPatches: symmetryPlaneOptimisation not ported; {:?} not flattened", renamed.symmetry_planes_not_optimised));
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
    /// 5 mm), same differential. Results: the README.
    #[test]
    fn diagnose_cartesian_mesh_on_the_pipes() {
        for (name, l, h) in [("pipe 60 mm", 0.06, 0.002), ("Foam Studio pipe 200 mm", 0.2, 0.005)] {
            let (p, t, r) = pipe_surface(0.01, l, 48);
            let exact = pipe_exact(0.01, l, 48);
            let opts = CartesianMeshGeneratorOptions { dict: MeshDict::with_max_cell_size(h), ..Default::default() };
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
    /// docs say. Results: the README.
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
    }

    /// V&V (gh:#1274): `stopAfter` and restart. A run stopped after
    /// `edgeExtraction`, restarted from its saved mesh, gives the
    /// uninterrupted run's mesh bit for bit; two runs agree bit for bit.
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
}
