// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/tetMesh/tetMeshGenerator/tetMeshGenerator.{C,H}
//   executables/tetMesh/tetMesh.C
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

//! cfMesh's **`tetMesh`** workflow (`tetMeshGenerator`, gh:#1272): a new
//! entry point beside [`crate::pipeline::surface_to_tet_dual_mesh`], whose
//! output it does not change.
//!
//! The constructor builds the surface octree (`meshOctreeCreator::
//! createOctreeBoxes` with the [`MeshDict`]), then `generateMesh` runs the
//! steps below, each behind [`WorkflowControls::run_current_step`]
//! (`stopAfter`, restart), then `renumberMesh` and `renameBoundaryPatches`.
//!
//! | step | upstream | here |
//! |---|---|---|
//! | `templateGeneration` | `tetMeshExtractorOctree` | [`crate::tet_creator_octree`], [`crate::tet_mesh_extractor_octree`] (ported) |
//! | `surfaceTopology` | `surfaceMorpherCells` + `topologicalCleaner` (repeated while the cleaner changes the mesh) | `topology::{surface_morpher_cells, topological_cleaner}` (gh:#1267) |
//! | `surfaceProjection` | `meshSurfaceMapper::mapVerticesOntoSurface`, `meshSurfaceOptimizer::untangleSurface` | `surface_optimizer::map_mesh_to_surface` (gh:#1259) |
//! | `patchAssignment` | `edgeExtractor` | ~~**not ported** (not this track): `patches::assign_patches_by_region` (nearest triangle) stands in~~ **UPDATED 2026-11-05 (gh:#1269):** [`crate::edge_extractor::extract_patches`]; the nearest triangle stays callable as the ablation [`PatchAssignment::NearestTriangle`] |
//! | `edgeExtraction` | `meshSurfaceEdgeExtractorNonTopo`, `meshSurfaceOptimizer::optimizeSurface` | `correct_edges::correct_edges_between_patches` then `surface_mapper::map_vertices_onto_surface_patches_given` (`decomposeBoundaryFaces`, `remapBoundaryPoints`); `surface_optimizer::optimise_mesh_surface` |
//! | `boundaryLayerGeneration` | `boundaryLayers`, only with a `boundaryLayers` dict | ~~no such dict here: nothing to do, as upstream without one~~ **UPDATED 2026-11-05 (gh:#1270 remainder):** with [`TetMeshOptions::boundary_layers`], `layers::boundary_layers::generate_layer` (`nLayers > 0`: `addLayerForAllPatches`, patch-wise; else `addLayerForPatch` per `patchBoundaryLayers` patch, in name order); none by default |
//! | `meshOptimisation` | `meshOptimizer`: `optimizeSurface`, `optimizeMeshFV`, `optimizeLowQualityFaces`, `optimizeBoundaryLayer(false)`, `untangleMeshFV` | `mesh_optimizer` (`optimize_surface`, and the volume drivers with upstream's boundary moves and LP untangler on); `optimizeBoundaryLayer` has no layer to act on |
//! | `boundaryLayerRefinement` | `refineBoundaryLayers`, `pointsInBndLayer`, `untangleBoundaryLayer`, only with a `boundaryLayers` dict | ~~nothing to do~~ with the dict, `cartesian_mesh`'s `ref_boundary_layers` (gh:#1270 remainder) |
//! | (always) | `renumberMesh`, `renameBoundaryPatches` | [`crate::renumber_mesh`], [`crate::rename_boundary_patches`] (ported) |
//!
//! Every gap is a line in [`TetMeshReport::notes`]. ~~`anisotropicSources`
//! (`projectSurfaceAfterBackScaling`, the geometry modification) and the
//! surface's feature-edge patches (`triSurfacePatchManipulator`) are not
//! supported: the surface carries no feature edges and the `MeshDict` no
//! anisotropic sources.~~ **UPDATED 2026-11-05 (gh:#1279 remainder):**
//! [`MeshDict::anisotropic_sources`] is applied as upstream in both
//! generators: the octree on the modified surface
//! ([`crate::anisotropic::modified_surface_points`]), every step up to the
//! revert against it; at the end of `meshOptimisation`'s
//! `optimiseFinalMesh`, `revertGeometryModification`, then
//! `projectSurfaceAfterBackScaling` ([`crate::anisotropic::revert_and_project`],
//! `optimiseFinalMesh` again on the original surface). A run stopped
//! before `meshOptimisation` returns the mesh in the modified space, as
//! upstream. Record: `verification_and_validation/anisotropic_gh1279/`.
//! The surface's feature-edge patches (`triSurfacePatchManipulator`) are
//! still not supported: the surface carries no feature edges.

use crate::math::Vec3;
use crate::mesh_octree::{MeshOctree, TriSurf};
use crate::mesh_octree_creator::{MeshDict, MeshOctreeCreator};
use crate::mesh_optimizer::{MeshOptimizer, UntangleControls};
use crate::patches::{assign_patches_by_region, SurfaceRegions};
use crate::rename_boundary_patches::{rename_boundary_patches, RenameBoundary};
use crate::renumber_mesh::renumber_mesh;
use crate::correct_edges::correct_edges_between_patches;
use crate::surface_mapper::map_vertices_onto_surface_patches_given_octree;
use crate::surface_optimizer::{map_mesh_to_surface_octree, optimise_mesh_surface_octree, optimise_mesh_surface};
use crate::topology::{surface_morpher_cells, topological_cleaner, TopoMesh};
use crate::tet_creator_octree::{tet_creator_octree, TetCreatorDefects};
use crate::tet_mesh_extractor_octree::tet_mesh_extractor_octree;
use crate::volume_mesh::VolumeMesh;
use crate::workflow_controls::{MeshMetaData, StepDecision, WorkflowControls, WorkflowSettings, WorkflowStep};
use std::sync::Arc;

/// Settings of [`tet_mesh`] (and [`crate::pipeline::p_mesh::p_mesh`]): the
/// `meshDict` entries the workflow reads.
#[derive(Debug, Clone, PartialEq)]
pub struct TetMeshOptions {
    /// The octree settings (`maxCellSize`, `boundaryCellSize`, refinement).
    pub dict: MeshDict,
    /// `workflowControls`.
    pub workflow: WorkflowSettings,
    /// `renameBoundary`.
    pub rename_boundary: Option<RenameBoundary>,
    /// `enforceGeometryConstraints` (upstream default `false`).
    pub enforce_geometry_constraints: bool,
    /// How `patchAssignment` distributes the boundary faces (gh:#1269):
    /// upstream's `edgeExtractor` by default.
    pub patch_assignment: PatchAssignment,
    /// `pMesh`'s `meshSurfaceEdgeExtractorFUN`: whether its
    /// `createFundamentalSheetsJFS` runs (gh:#1281). Upstream (`Jfs`) by
    /// default; `Without` is the gh:#1269 behaviour, kept for the
    /// differential.
    pub fun_sheets: crate::mesh_surface_edge_extractor_fun::FunSheets,
    /// `meshDict`'s `boundaryLayers` dictionary (`None`: absent, no layer
    /// and no refinement, upstream's default).
    pub boundary_layers: Option<BoundaryLayersDict>,
    /// `boundaryLayers/optimiseLayer` with its `optimisationParameters`
    /// (gh:#1476): `Some` runs `optimizeBoundaryLayer(false)` in
    /// `optimiseFinalMesh`; `None` (upstream's default) does not.
    pub optimise_layer: Option<crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls>,
}

/// The `patchAssignment` step's rule (gh:#1269).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatchAssignment {
    /// Upstream: [`crate::edge_extractor`] (`edgeExtractor::extractEdges`,
    /// `updateMeshPatches`).
    #[default]
    EdgeExtractor,
    /// Ablation: each boundary face to the region of the triangle nearest
    /// its vertex average (`patches::assign_patches_by_region`, the stand-in
    /// before gh:#1269), kept for the differential.
    NearestTriangle,
}

impl Default for TetMeshOptions {
    /// `maxCellSize` 1, upstream's defaults elsewhere.
    fn default() -> Self {
        TetMeshOptions {
            dict: MeshDict::with_max_cell_size(1.0),
            workflow: WorkflowSettings::default(),
            rename_boundary: None,
            enforce_geometry_constraints: false,
            patch_assignment: PatchAssignment::EdgeExtractor,
            fun_sheets: crate::mesh_surface_edge_extractor_fun::FunSheets::Jfs,
            boundary_layers: None,
            optimise_layer: None,
        }
    }
}

/// `meshDict`'s `boundaryLayers` dictionary as `tetMesh`, `pMesh` and the
/// `generateBoundaryLayers` utility read it (gh:#1270 remainder).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoundaryLayersDict {
    /// `nLayers`, if the entry is present.
    pub n_layers: Option<usize>,
    /// `refineBoundaryLayers::readSettings`: `nLayers` (again: keep it equal
    /// to [`BoundaryLayersDict::n_layers`], or use
    /// [`BoundaryLayersDict::with_n_layers`]), `thicknessRatio`,
    /// `maxFirstLayerThickness`, `patchBoundaryLayers`.
    pub settings: crate::layers::refine_boundary_layers::RefineBoundaryLayersSettings,
    /// `boundaryLayerHeightFactor`, `boundaryLayerMinHeight`.
    pub heights: crate::layers::boundary_layers::BoundaryLayersControls,
    /// `patchBoundaryLayers.toc()` as written, the names
    /// `generateBoundaryLayers` passes to `addLayerForPatch(word)`, which
    /// compares them **exactly** with the mesh's patch names (gh:#1476: the
    /// `meshDict` reader keeps them, while [`Self::settings`]' patches are
    /// the regular-expression matches `refineBoundaryLayers` makes). `None`
    /// (the dicts built in code before gh:#1476): the keys of
    /// `settings.patches` are used instead.
    pub patch_layer_names: Option<Vec<String>>,
}

impl BoundaryLayersDict {
    /// A dict with the global `nLayers` entry `n`.
    pub fn with_n_layers(n: usize) -> Self {
        let settings = crate::layers::refine_boundary_layers::RefineBoundaryLayersSettings { n_layers: n, ..Default::default() };
        BoundaryLayersDict { n_layers: Some(n), settings, heights: Default::default(), patch_layer_names: None }
    }

    /// The generators' `generateBoundaryLayers`: `nLayers` present and
    /// above 0, every patch; present and 0, none; absent, each
    /// `patchBoundaryLayers` patch: [`Self::patch_layer_names`] in
    /// dictionary order when given (gh:#1476), else the keys of
    /// `settings.patches` (name order).
    pub fn request(&self) -> Option<crate::layers::boundary_layers::LayerRequest> {
        use crate::layers::boundary_layers::LayerRequest;
        match (self.n_layers, &self.patch_layer_names) {
            (Some(n), _) => (n > 0).then_some(LayerRequest::AllPatches),
            (None, Some(names)) => (!names.is_empty()).then(|| LayerRequest::Patches(names.clone())),
            (None, None) => (!self.settings.patches.is_empty()).then(|| LayerRequest::Patches(self.settings.patches.keys().cloned().collect())),
        }
    }
}

/// A mesh saved after a step, with its meta data: what a restart reads
/// (upstream reads it from disk).
#[derive(Debug, Clone)]
pub struct SavedMesh {
    /// The mesh.
    pub mesh: VolumeMesh,
    /// The patch types.
    pub patch_types: Vec<String>,
    /// The workflow meta data.
    pub meta: MeshMetaData,
}

/// What the workflow did.
#[derive(Debug, Clone, PartialEq)]
pub struct TetMeshReport {
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
    /// Cells of the template mesh (tets, or Voronoi cells).
    pub n_template_cells: usize,
    /// Volume of the template mesh.
    pub template_volume: f64,
    /// Tets the creator made.
    pub n_tets: usize,
    /// Reads upstream leaves undefined (zero on every recorded run).
    pub creator_defects: TetCreatorDefects,
    /// Passes of `surfacePreparation` (the morpher, then the cleaner while
    /// it changes the mesh).
    pub n_surface_preparation_passes: usize,
    /// Cells `surfaceMorpherCells` removed, over all passes.
    pub n_cells_removed_by_morpher: usize,
    /// Inverted boundary points `untangleSurface` left after the projection.
    pub n_inverted_after_projection: usize,
    /// Inverted boundary points left after `optimiseMeshSurface`.
    pub n_inverted_after_edge_extraction: usize,
    /// What `edgeExtractor` did (`None` with
    /// [`PatchAssignment::NearestTriangle`]).
    pub edge_extractor: Option<crate::edge_extractor::EdgeExtractorReport>,
    /// `pMesh`'s `preMapVertices`: mean boundary-point distance to the
    /// surface before and after (`None` for `tetMesh`, which does not call
    /// it).
    pub premap_mean_distance: Option<(f64, f64)>,
    /// Layer cells `boundaryLayers` added (0 without a dict).
    pub n_layer_cells: usize,
    /// `refineBoundaryLayers` refused (this port's `NotPorted`).
    pub refine_error: Option<String>,
    /// With `anisotropicSources`: the revert and the projection of
    /// `projectSurfaceAfterBackScaling` (gh:#1279 remainder).
    pub back_scaling: Option<crate::anisotropic::BackScalingReport>,
}

/// The surface region of every boundary face of `m`, from its patch name
/// (`None` for internal faces). With one region, every boundary face is in
/// it (the whole boundary is one patch before `extractPatches`).
pub(crate) fn face_regions(m: &VolumeMesh, reg: &SurfaceRegions) -> Vec<Option<usize>> {
    let mut out = vec![None; m.face_count()];
    for p in &m.patches {
        let r = reg.names.iter().position(|n| *n == p.name).or(if reg.names.len() == 1 { Some(0) } else { None });
        for x in out.iter_mut().skip(p.start_face).take(p.n_faces) {
            *x = r;
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Template {
    /// `tetMeshExtractorOctree`.
    Tet,
    /// `voronoiMeshExtractor`.
    Voronoi,
}

/// cfMesh `tetMesh`: the workflow on the surface `(points, tris)` with
/// optional named regions (module docs).
pub fn tet_mesh(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &TetMeshOptions) -> Result<(VolumeMesh, TetMeshReport), String> {
    run_generator(Template::Tet, points, tris, regions, opts, None)
}

/// [`tet_mesh`] restarted from a mesh saved after a step
/// (`restartFromLatestStep` must be set in `opts.workflow`).
pub fn tet_mesh_restart(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &TetMeshOptions, saved: &SavedMesh) -> Result<(VolumeMesh, TetMeshReport), String> {
    run_generator(Template::Tet, points, tris, regions, opts, Some(saved))
}

/// The generator's surface octree: `meshOctree(surf)` and
/// `createOctreeBoxes`.
pub fn create_octree(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, dict: &MeshDict) -> MeshOctree {
    let surf = TriSurf::new(points.to_vec(), tris.to_vec(), regions.map(|r| r.region_of_tri.clone()));
    let mut o = MeshOctree::new(Arc::new(surf));
    let mut c = MeshOctreeCreator::new(&o, Some(dict.clone()));
    c.create_octree_boxes(&mut o);
    o
}

/// `tetMeshGenerator::optimiseFinalMesh` / `voronoiMeshGenerator::
/// optimiseFinalMesh` without the revert (the caller does that, gh:#1279
/// remainder), against the surface `(points, tris)`: `pMesh` runs
/// `meshSurfaceOptimizer::optimizeSurface` first, `tetMesh`
/// `meshOptimizer::optimizeSurface(octree)`; then `optimizeMeshFV`,
/// `optimizeLowQualityFaces`, `optimizeBoundaryLayer(false)` (acts only
/// with `optimiseLayer`, off by default; gh:#1476), `untangleMeshFV`.
/// Returns the report line.
fn generator_optimise_final_mesh(kind: Template, m: &mut VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], reg: &SurfaceRegions, enforce_geometry_constraints: bool, optimise_layer: Option<&crate::boundary_layer_optimisation::BoundaryLayerOptimisationControls>, octree: Option<&MeshOctree>) -> Result<String, String> {
    let given = face_regions(m, reg);
    if kind == Template::Voronoi {
        // meshSurfaceOptimizer(mse, octree)::optimizeSurface on its own
        // (gh:#1385: the searches go through the octree, as upstream's)
        match octree {
            Some(o) => crate::surface_optimizer::optimise_mesh_surface_octree(m, o, points, tris, reg, &given),
            None => optimise_mesh_surface(m, points, tris, reg, &given),
        };
    }
    let mut opt = MeshOptimizer::new(m);
    if enforce_geometry_constraints {
        opt.set_enforce_constraints("badPoints");
    }
    if kind == Template::Tet {
        // meshOptimizer::optimizeSurface(octree)
        match octree {
            Some(o) => crate::mesh_optimizer::optimize_surface_octree(m, o, points, tris, reg, &given),
            None => crate::mesh_optimizer::optimize_surface(m, points, tris, reg, &given),
        };
    }
    let controls = UntangleControls { boundary_moves: true, mesh_untangler: true, ..UntangleControls::default() };
    // gh:#1385: `M1`..`M5` point dumps after each step, as the instrumented `voronoiMeshGenerator::optimiseFinalMesh` prints
    let dump = |tag: &str, m: &VolumeMesh| {
        if kind == Template::Voronoi && std::env::var("GH1385_TRACE").is_ok() {
            for (i, p) in m.points.iter().enumerate() {
                eprintln!("{tag}P {i} {:?} {:?} {:?}", p.x, p.y, p.z);
            }
        }
    };
    dump("M1", m);
    let r1 = opt.optimize_mesh_fv(m, 5, &controls).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
    dump("M2", m);
    let r2 = opt.optimize_low_quality_faces(m, 10);
    dump("M3", m);
    let bl = crate::pipeline::cartesian_mesh::optimize_boundary_layer_step(m, optimise_layer, false, &controls, "optimizeBoundaryLayer(false): acts only with optimiseLayer (off, upstream default)");
    dump("M4", m);
    let r3 = opt.untangle_mesh_fv(m, &controls).map_err(|e| format!("Cannot untangle mesh!! ({} bad points)", e.points.len()))?;
    dump("M5", m);
    Ok(format!(
        "optimizeSurface, then optimizeMeshFV bad faces {} -> {}, optimizeLowQualityFaces {} -> {}, untangleMeshFV {} -> {}; {bl}",
        r1.flagged_before, r1.flagged_after, r2.flagged_before, r2.flagged_after, r3.flagged_before, r3.flagged_after
    ))
}

/// The `generateMesh` both generators share; `kind` picks the template and
/// the few steps where `voronoiMeshGenerator` differs.
pub(crate) fn run_generator(
    kind: Template,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: Option<&SurfaceRegions>,
    opts: &TetMeshOptions,
    saved: Option<&SavedMesh>,
) -> Result<(VolumeMesh, TetMeshReport), String> {
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
    // without named regions the surface is one patch
    let single = SurfaceRegions::single("defaultFaces", tris.len());
    let reg = regions.unwrap_or(&single);
    let meta = saved.map(|s| s.meta.clone()).unwrap_or_default();
    let mut w = WorkflowControls::new(opts.workflow, meta);
    let mut rep = TetMeshReport {
        notes: Vec::new(),
        steps_run: Vec::new(),
        stopped_after: None,
        meta: MeshMetaData::default(),
        patch_types: Vec::new(),
        n_template_cells: 0,
        template_volume: 0.0,
        n_tets: 0,
        creator_defects: TetCreatorDefects::default(),
        n_surface_preparation_passes: 0,
        n_cells_removed_by_morpher: 0,
        n_inverted_after_projection: 0,
        n_inverted_after_edge_extraction: 0,
        edge_extractor: None,
        premap_mean_distance: None,
        n_layer_cells: 0,
        refine_error: None,
        back_scaling: None,
    };
    if mod_active {
        rep.notes.push("constructor: anisotropicSources: surfaceMeshGeometryModification, the octree on the modified surface".into());
    }
    let mut mesh: Option<VolumeMesh> = None;
    let mut types: Vec<String> = Vec::new();
    // generateMesh's order (both generators)
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
            let tc = tet_creator_octree(&o);
            rep.n_tets = tc.tets.len();
            rep.creator_defects = tc.defects;
            let m = match kind {
                Template::Tet => tet_mesh_extractor_octree(&tc)?.mesh,
                Template::Voronoi => crate::voronoi_mesh_extractor::voronoi_mesh_extractor(&tc)?.mesh,
            };
            rep.n_template_cells = m.n_cells;
            rep.template_volume = m.total_volume();
            types = vec!["patch".to_string(); m.patches.len()];
            mesh = Some(m);
            continue;
        }
        let m = mesh.as_mut().ok_or("no mesh: the workflow resumed after a step that saved none")?;
        match step {
            WorkflowStep::SurfaceTopology => {
                // surfacePreparation: tetMesh repeats surfaceMorpherCells
                // while topologicalCleaner changes the mesh; pMesh runs the
                // morpher once
                let mut w = TopoMesh::from_volume_mesh(m);
                let mut passes = 0;
                let mut removed = 0;
                loop {
                    passes += 1;
                    let sm = surface_morpher_cells(&mut w).map_err(|e| format!("surfaceMorpherCells: {e:?}"))?;
                    removed += sm.n_cells_removed;
                    if kind == Template::Voronoi || !topological_cleaner(&mut w).changed {
                        break;
                    }
                }
                let (mm, _) = w.into_volume_mesh();
                *m = mm;
                types = vec!["patch".to_string(); m.patches.len()];
                rep.n_surface_preparation_passes = passes;
                rep.n_cells_removed_by_morpher = removed;
                rep.notes.push(format!(
                    "surfaceTopology: {} ({passes} passes, {removed} cells removed by the morpher)",
                    if kind == Template::Tet { "surfaceMorpherCells + topologicalCleaner" } else { "surfaceMorpherCells" }
                ));
            }
            WorkflowStep::SurfaceProjection => {
                if kind == Template::Voronoi {
                    // meshSurfaceMapper::preMapVertices() (pMesh only;
                    // gh:#1274 remainder, was skipped)
                    let pr = crate::premap_vertices::pre_map_vertices(m, &o, spts, tris, reg, 3);
                    rep.premap_mean_distance = Some(pr.mean_distance);
                    rep.notes.push(format!("surfaceProjection: preMapVertices (mean distance {:.3e} -> {:.3e})", pr.mean_distance.0, pr.mean_distance.1));
                }
                // mapVerticesOntoSurface, then untangleSurface
                let (mm, u) = map_mesh_to_surface_octree(m, &o, spts, tris, reg);
                *m = mm;
                rep.n_inverted_after_projection = u.n_inverted_last;
                rep.notes.push(format!("surfaceProjection: mapVerticesOntoSurface + untangleSurface: inverted points {} -> {}", u.n_inverted_first, u.n_inverted_last));
            }
            WorkflowStep::PatchAssignment if opts.patch_assignment == PatchAssignment::EdgeExtractor => {
                // extractPatches: edgeExtractor(mesh, octree), extractEdges,
                // updateMeshPatches (one patch per surface patch)
                let (mm, ty, er) = crate::edge_extractor::extract_patches(m, &o, reg)?;
                *m = mm;
                types = ty;
                rep.notes.push(format!(
                    "patchAssignment: edgeExtractor: {} faces moved from the nearest-patch distribution (normal alignment {:?}, topology {:?}, geometry: inverted {:?}, moved {:?}); unset reads {} / {}",
                    er.n_faces_moved_from_initial,
                    er.normal_alignment_moves,
                    er.topology_kept,
                    er.geometry_inverted,
                    er.geometry_moves,
                    er.n_unset_neighbour_patch_reads,
                    er.n_failed_region_searches
                ));
                rep.edge_extractor = Some(er);
            }
            WorkflowStep::PatchAssignment => match regions {
                Some(r) => {
                    *m = assign_patches_by_region(m, spts, tris, r)?;
                    types = r.types_for(m.patches.len());
                    rep.notes.push("patchAssignment: PatchAssignment::NearestTriangle (ablation of edgeExtractor): nearest-triangle patches (patches::assign_patches_by_region)".into());
                }
                None => rep.notes.push("patchAssignment: no regions given; one patch".into()),
            },
            WorkflowStep::EdgeExtraction => {
                let mut given = face_regions(m, reg);
                // gh:#1385: upstream runs these on a surface of one region too
                // (the port skipped them without `regions`).
                let r = regions.unwrap_or(reg);
                if kind == Template::Tet {
                    // meshSurfaceEdgeExtractorNonTopo: decomposeBoundaryFaces
                    // (correctEdgesBetweenPatches), then remapBoundaryPoints
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
                } else {
                    // meshSurfaceEdgeExtractorFUN(mesh, octree, false):
                    // createFundamentalSheetsJFS, smoothMeshSurface,
                    // remapBoundaryPoints (gh:#1281)
                    let old_types: Vec<(String, String)> = m.patches.iter().map(|p| p.name.clone()).zip(types.iter().cloned()).collect();
                    let (fr, g) = crate::mesh_surface_edge_extractor_fun::mesh_surface_edge_extractor_fun(m, Some(&o), spts, tris, r, &given, opts.fun_sheets, false);
                    given = g;
                    // The sheets rebuild the patches; each keeps its type by name.
                    types = m.patches.iter().map(|p| old_types.iter().find(|(n, _)| *n == p.name).map_or("patch".to_string(), |(_, t)| t.clone())).collect();
                    let sheets = match &fr.sheets {
                        None => "without createFundamentalSheetsJFS (FunSheets::Without)".to_string(),
                        Some(Ok(s)) => format!(
                            "createFundamentalSheetsJFS: topology ok {}, wrapper sheet {}, feature-edge sheet {} faces / {} new points / {} edge + {} corner cells",
                            s.topology_ok,
                            s.initial_sheet.as_ref().map_or("none".to_string(), |i| format!("{} faces", i.n_extruded_faces)),
                            s.feature_edge_sheets.iter().map(|f| f.n_extruded_faces).sum::<usize>(),
                            s.feature_edge_sheets.iter().map(|f| f.n_new_points).sum::<usize>(),
                            s.feature_edge_sheets.iter().map(|f| f.n_edge_cells).sum::<usize>(),
                            s.feature_edge_sheets.iter().map(|f| f.n_corner_cells).sum::<usize>()
                        ),
                        Some(Err(e)) => format!("createFundamentalSheetsJFS stopped ({e}); went on without the sheets"),
                    };
                    rep.notes.push(format!(
                        "edgeExtraction: meshSurfaceEdgeExtractorFUN: {sheets}; optimizeSurface (inverted {} -> {}), then mapVerticesOntoSurfacePatches",
                        fr.smoothing.n_inverted_first, fr.smoothing.n_inverted_last
                    ));
                }
                // optimiseMeshSurface: tetMesh optimises the surface; pMesh also
                // untangles it, on the same optimiser (gh:#1385 round 4)
                let (u, last) = if kind == Template::Voronoi {
                    let (a, b) = crate::surface_optimizer::optimise_mesh_surface_then_untangle_octree(m, &o, spts, tris, reg, &given);
                    (a, b.n_inverted_last)
                } else {
                    let a = optimise_mesh_surface_octree(m, &o, spts, tris, reg, &given);
                    let last = a.n_inverted_last;
                    (a, last)
                };
                rep.n_inverted_after_edge_extraction = last;
                rep.notes.push(format!("edgeExtraction: optimiseMeshSurface: inverted points {} -> {last}", u.n_inverted_first));
            }
            WorkflowStep::BoundaryLayerGeneration => match opts.boundary_layers.as_ref().map(|d| (d.request(), d)) {
                None => rep.notes.push("boundaryLayerGeneration: no boundaryLayers dict; nothing to do".into()),
                Some((None, _)) => rep.notes.push("boundaryLayerGeneration: boundaryLayers dict with nLayers 0 and no patchBoundaryLayers; nothing to do".into()),
                Some((Some(req), d)) => {
                    let (mm, n, line) = crate::pipeline::cartesian_mesh::boundary_layer_generation(m, &req, &d.heights, crate::pipeline::cartesian_mesh::LayerTopology::PatchWise, false)?;
                    *m = mm;
                    types.resize(m.patches.len(), "patch".to_string());
                    rep.n_layer_cells = n;
                    rep.notes.push(format!("boundaryLayerGeneration: {line}; {} non-positive cells", crate::pipeline::cartesian_mesh::n_nonpositive_cells(m)));
                }
            },
            WorkflowStep::MeshOptimisation => {
                // optimiseFinalMesh on the octree's surface (the modified
                // one with anisotropicSources)
                let line = generator_optimise_final_mesh(kind, m, spts, tris, reg, opts.enforce_geometry_constraints, opts.optimise_layer.as_ref(), Some(&o))?;
                rep.notes.push(format!("meshOptimisation: {line}"));
                // its end: revertGeometryModification while modSurfacePtr_
                // is set, then projectSurfaceAfterBackScaling
                if let (true, Some(cm)) = (mod_active, opts.dict.anisotropic_sources.as_ref()) {
                    mod_active = false;
                    let b = crate::anisotropic::revert_and_project(m, cm, points, tris);
                    let line2 = generator_optimise_final_mesh(kind, m, points, tris, reg, opts.enforce_geometry_constraints, opts.optimise_layer.as_ref(), None)?;
                    rep.back_scaling = Some(b);
                    rep.notes.push(format!(
                        "meshOptimisation: revertGeometryModification ({} points); projectSurfaceAfterBackScaling: mapVerticesOntoSurface on the original surface (largest move {:.3e}), optimiseFinalMesh again: {line2}",
                        b.n_points_reverted, b.max_projection_move
                    ));
                }
            }
            WorkflowStep::BoundaryLayerRefinement => match &opts.boundary_layers {
                None => rep.notes.push("boundaryLayerRefinement: no boundaryLayers dict; nothing to do".into()),
                Some(d) => {
                    let controls = UntangleControls { boundary_moves: true, mesh_untangler: true, ..UntangleControls::default() };
                    let (line, refused, _) = crate::pipeline::cartesian_mesh::ref_boundary_layers(m, &d.settings, &controls)?;
                    types.resize(m.patches.len(), "patch".to_string());
                    rep.refine_error = refused;
                    rep.notes.push(line);
                }
            },
            _ => {}
        }
    }
    let m = mesh.ok_or("the workflow produced no mesh")?;
    let r = renumber_mesh(&m, &m.cell_face_lists());
    let renamed = rename_boundary_patches(&r.mesh, &types, opts.rename_boundary.as_ref(), false);
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
    if kind == Template::Tet {
        w.workflow_completed();
    }
    rep.meta = w.meta;
    rep.patch_types = renamed.patch_types;
    Ok((renamed.mesh, rep))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::checks::check_quality;
    use crate::pipeline::{surface_to_tet_dual_mesh, TetDualOptions};
    use crate::shapes::{cylinder_surface, sphere_surface};
    use crate::tet_creator_octree::{tet_creator_octree, tet_volume};
    use crate::tet_mesh_extractor_octree::tet_mesh_extractor_octree;
    use crate::cartesian_mesh::{BoxType, MeshOctreeAddressing};
    use std::f64::consts::PI;
    use std::time::Instant;

    /// The pipe of the octree tests: R, L, n sides; regions walls (0),
    /// inlet (z = 0), outlet (z = L).
    pub(crate) fn pipe_surface(r: f64, l: f64, n: usize) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
        let (p, t) = cylinder_surface(Vec3::ZERO, r, l, n);
        let labels: Vec<&str> = t
            .iter()
            .map(|tri| {
                let z = [p[tri[0]].z, p[tri[1]].z, p[tri[2]].z];
                let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
                match (flat, z[0] < 0.5 * l) {
                    (true, true) => "inlet",
                    (true, false) => "outlet",
                    _ => "walls",
                }
            })
            .collect();
        let regions = SurfaceRegions::from_labels(&labels);
        (p, t, regions)
    }

    fn n_nonpositive_cells(m: &VolumeMesh) -> usize {
        let (vols, _) = crate::checks::cell_geometry(m);
        vols.iter().filter(|&&v| v <= 0.0).count()
    }

    /// V&V (gh:#1272), predictions 1–3 of
    /// `verification_and_validation/tet_mesh_gh1272/README.md`:
    /// `tetCreatorOctree` and `tetMeshExtractorOctree` on the sphere
    /// (uniform 0.5 m) and the graded pipe (2 mm, 1 mm boundary cells).
    ///
    /// Checks: every tet positive; no undefined read (`TetCreatorDefects`
    /// zero); the extracted mesh closed; on the uniform octree only
    /// around-edge tets (point count = nodes + centres) and the volume
    /// equals the `MESHCELL` leaves minus `h^3/6` per region-boundary leaf
    /// face. Results (2026-11-05): sphere 13 968 tets, 0 negative, 0 defects,
    /// 144 dropped; **missed** the uniform premise (coarse BOUNDARY leaves give
    /// sub-nodes) and the volume (145.5 against 142.5 m^3: 288 tets, 3.0 m^3,
    /// lie in BOUNDARY leaves, pinned); pipe 195 136 tets, 0 negative, closed.
    /// Full record: `README.md` of that folder.
    #[test]
    fn tet_creator_on_the_sphere_and_the_pipe() {
        // sphere, uniform
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let o = create_octree(&p, &t, None, &MeshDict::with_max_cell_size(0.5));
        let tc = tet_creator_octree(&o);
        let a = MeshOctreeAddressing::new(&o, true);
        let n_cells = (0..o.number_of_leaves()).filter(|&l| a.box_type[l] & BoxType::MESHCELL != 0).count();
        let n_neg = tc.tets.iter().filter(|x| tet_volume(&tc.tet_points, x) <= 0.0).count();
        let mut n_bnd_faces = 0;
        let mut h = 0.0;
        for l in 0..o.number_of_leaves() {
            if a.box_type[l] & BoxType::MESHCELL == 0 {
                continue;
            }
            let cc = o.return_leaf(l).coords;
            h = cc.size(&o.root_box);
            for d in 0..6 {
                let mut n = Vec::new();
                o.find_neighbours_in_direction(&cc, d, &mut n);
                let mesh_nei = n.len() == 1 && n[0].is_some_and(|x| a.box_type[x] & BoxType::MESHCELL != 0);
                if !mesh_nei {
                    n_bnd_faces += 1;
                }
            }
        }
        let ex = tet_mesh_extractor_octree(&tc).unwrap();
        ex.mesh.validate().unwrap();
        let expect = n_cells as f64 * h * h * h - n_bnd_faces as f64 * h * h * h / 6.0;
        let vol = ex.mesh.total_volume();
        println!(
            "sphere: leaves {} MESHCELL {n_cells} h {h} nodes {} points {} tets {} (dropped {}) negative {n_neg} defects {:?}; \
             mesh cells {} faces {} points {}; volume {vol:.9} expected {expect:.9} (rel {:.2e}), region-boundary faces {n_bnd_faces}",
            o.number_of_leaves(),
            tc.n_nodes,
            tc.tet_points.len(),
            tc.tets.len(),
            tc.n_dropped_tets,
            tc.defects,
            ex.mesh.n_cells,
            ex.mesh.face_count(),
            ex.mesh.point_count(),
            ((vol - expect) / expect).abs()
        );
        // levels of the used leaves, and where the tets lie
        let mut hist = std::collections::BTreeMap::new();
        for l in 0..o.number_of_leaves() {
            if a.box_type[l] != BoxType::NONE {
                *hist.entry((a.box_type[l] & BoxType::MESHCELL != 0, o.return_leaf(l).coords.level)).or_insert(0usize) += 1;
            }
        }
        // tets whose centroid lies in a leaf that is not a MESHCELL
        let mut outside_vol = 0.0;
        let mut n_outside = 0;
        for x in &tc.tets {
            let c = x.iter().fold(Vec3::ZERO, |s, &v| s.add(tc.tet_points[v])).scale(0.25);
            let leaf = (0..o.number_of_leaves()).find(|&l| o.return_leaf(l).coords.cube_box(&o.root_box).contains(c));
            if leaf.is_some_and(|l| a.box_type[l] & BoxType::MESHCELL == 0) {
                n_outside += 1;
                outside_vol += tet_volume(&tc.tet_points, x);
            }
        }
        println!("sphere: used leaves by (MESHCELL, level): {hist:?}; tets with centroid outside MESHCELL leaves {n_outside}, volume {outside_vol:.6}");
        // MISSED prediction 1 (uniform octree): the OUTSIDE leaves next to
        // the DATA band are coarser, so sub-nodes appear; recorded in the
        // V&V README, not asserted.
        assert_eq!(n_neg, 0);
        assert_eq!(tc.defects, TetCreatorDefects::default());
        assert_eq!(n_nonpositive_cells(&ex.mesh), 0);
        ex.mesh.validate().unwrap();
        // MISSED prediction 2, explained: the extra volume is exactly the
        // tets createTetsFromFacesWithCentreNode builds inside the coarser
        // BOUNDARY leaves (288 tets, 3.0 m^3 = 24 h^3).
        assert!(((vol - (expect + outside_vol)) / vol).abs() < 1e-12);
        assert_eq!((tc.tets.len(), n_outside, tc.n_dropped_tets), (13968, 288, 144), "pinned");

        // pipe, graded
        let (p, t, r) = pipe_surface(0.01, 0.06, 48);
        let dict = MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.002) };
        let o = create_octree(&p, &t, Some(&r), &dict);
        let tc = tet_creator_octree(&o);
        let n_neg = tc.tets.iter().filter(|x| tet_volume(&tc.tet_points, x) <= 0.0).count();
        let ex = tet_mesh_extractor_octree(&tc).unwrap();
        let v = ex.mesh.validate();
        println!(
            "pipe: leaves {} nodes {} points {} tets {} (dropped {}) negative {n_neg} defects {:?}; mesh cells {} volume {:.6e} validate {:?}",
            o.number_of_leaves(),
            tc.n_nodes,
            tc.tet_points.len(),
            tc.tets.len(),
            tc.n_dropped_tets,
            tc.defects,
            ex.mesh.n_cells,
            ex.mesh.total_volume(),
            v
        );
        assert!(tc.tet_points.len() > tc.n_nodes + 1000, "graded: sub-nodes and centres appear");
        assert_eq!(n_neg, 0);
        assert_eq!(tc.defects, TetCreatorDefects::default());
        v.unwrap();
    }

    /// Baseline of the differential: the crate's own tets at `cell_size`.
    fn baseline(p: &[Vec3], t: &[[usize; 3]], cell_size: f64) -> (VolumeMesh, f64) {
        let opts = TetDualOptions { cell_size, dual: false, n_layers: 0, delaunay: true, feature_edges: false, ..Default::default() };
        let t0 = Instant::now();
        let (m, _) = surface_to_tet_dual_mesh(p, t, &opts).unwrap();
        (m, t0.elapsed().as_secs_f64())
    }

    /// V&V (gh:#1272), differential and predictions 4–8: `tet_mesh` against
    /// the crate's `tetrahedralize` + `flip_to_delaunay` on the sphere
    /// (0.5 m) and the pipe (2 mm), at equal cell size. Records tet count,
    /// volume error against the surface, `check_quality`, run time.
    /// Results (2026-11-05), with gh:#1267 / #1259 wired in: volume error
    /// sphere −1.36 % (baseline −1.72 %), pipe −0.29 % (−0.54 %), 0
    /// non-positive cells, closed, 0 non-manifold boundary points; cells
    /// 12 840 / 31 136 against 21 888 / 57 600. (Before those tracks landed:
    /// +26.69 % / +22.33 %, the missing steps.) **Re-measured 2026-11-05
    /// (gh:#1269)** with `edgeExtractor` wired in: sphere unchanged
    /// (−1.362 %, one patch), pipe −0.291 % (was −0.289 %), 31 136 cells,
    /// max non-orthogonality 67.16° (unchanged), patches inlet 330 / outlet
    /// 331 / walls 3 427 (were 332 / 332 / 3 424). Record:
    /// `verification_and_validation/tet_mesh_gh1272/README.md`,
    /// `edge_extractor_gh1269/README.md`.
    #[test]
    fn diagnose_tet_mesh_against_the_crate_tets() {
        let (sp, st) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let (pp, pt, pr) = pipe_surface(0.01, 0.06, 48);
        let pipe_vol = 0.5 * 48.0 * 0.01f64.powi(2) * (2.0 * PI / 48.0).sin() * 0.06;
        let cases: [(&str, &[Vec3], &[[usize; 3]], Option<&SurfaceRegions>, f64, f64); 2] =
            [("sphere", &sp, &st, None, 0.5, 4.0 / 3.0 * PI * 27.0), ("pipe", &pp, &pt, Some(&pr), 0.002, pipe_vol)];
        for (name, p, t, r, h, exact) in cases {
            let opts = TetMeshOptions { dict: MeshDict::with_max_cell_size(h), ..Default::default() };
            let t0 = Instant::now();
            let (m, rep) = tet_mesh(p, t, r, &opts).unwrap();
            let dt = t0.elapsed().as_secs_f64();
            let (b, bt) = baseline(p, t, h);
            let q = check_quality(&m);
            let qb = check_quality(&b);
            let v = m.validate();
            let err = (m.total_volume() - exact) / exact * 100.0;
            let berr = (b.total_volume() - exact) / exact * 100.0;
            // template quality, before mapping
            let o = create_octree(p, t, r, &opts.dict);
            let tm = tet_mesh_extractor_octree(&tet_creator_octree(&o)).unwrap().mesh;
            let qt = check_quality(&tm);
            println!("{name}: tet_mesh cells {} vol {:.6e} err {err:+.3} % nonpos {} validate {v:?} non-orth {:.2} skew {:.3} time {dt:.2} s", m.n_cells, m.total_volume(), n_nonpositive_cells(&m), q.max_non_orthogonality_deg, q.max_skewness);
            println!("{name}: template cells {} vol {:.6e} non-orth {:.2} skew {:.3}", tm.n_cells, tm.total_volume(), qt.max_non_orthogonality_deg, qt.max_skewness);
            println!("{name}: baseline cells {} vol {:.6e} err {berr:+.3} % nonpos {} non-orth {:.2} skew {:.3} time {bt:.2} s", b.n_cells, b.total_volume(), n_nonpositive_cells(&b), qb.max_non_orthogonality_deg, qb.max_skewness);
            println!("{name}: patches {:?} types {:?}; non-manifold boundary points {}", m.patches.iter().map(|x| (x.name.clone(), x.n_faces)).collect::<Vec<_>>(), rep.patch_types, crate::pipeline::p_mesh::tests::non_manifold_boundary_points(&m));
            for n in &rep.notes {
                println!("  {n}");
            }
            v.unwrap();
            assert_eq!(n_nonpositive_cells(&m), 0, "{name}");
        }
    }

    /// V&V (gh:#1272): `stopAfter` and restart through the generator. A run
    /// stopped after `surfaceProjection`, restarted from its saved mesh, gives
    /// the same mesh bit for bit as an uninterrupted run; the stopped run is
    /// neither renumbered nor renamed; the template is deterministic.
    #[test]
    fn tet_mesh_stop_and_restart_reproduce_the_full_run() {
        let (p, t, r) = pipe_surface(0.01, 0.06, 24);
        let base = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.004), ..Default::default() };
        let (full, rep) = tet_mesh(&p, &t, Some(&r), &base).unwrap();
        assert!(rep.meta == MeshMetaData::default(), "workflowCompleted clears the meta data");
        let stop = TetMeshOptions { workflow: WorkflowSettings { stop_after: Some(WorkflowStep::SurfaceProjection), restart_from_latest_step: false }, ..base.clone() };
        let (stopped, srep) = tet_mesh(&p, &t, Some(&r), &stop).unwrap();
        assert_eq!(srep.stopped_after, Some(WorkflowStep::SurfaceProjection));
        assert_eq!(stopped.patches.len(), 1);
        assert_eq!(stopped.patches[0].name, "defaultFaces");
        let saved = SavedMesh { mesh: stopped, patch_types: srep.patch_types.clone(), meta: srep.meta.clone() };
        let restart = TetMeshOptions { workflow: WorkflowSettings { stop_after: None, restart_from_latest_step: true }, ..base.clone() };
        let (resumed, rrep) = tet_mesh_restart(&p, &t, Some(&r), &restart, &saved).unwrap();
        assert_eq!(rrep.steps_run.first(), Some(&WorkflowStep::PatchAssignment));
        assert!(crate::tet_mesh_extractor_octree::same_mesh(&resumed, &full));
        let (again, _) = tet_mesh(&p, &t, Some(&r), &base).unwrap();
        assert!(crate::tet_mesh_extractor_octree::same_mesh(&again, &full));
        let names: Vec<&str> = full.patches.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["inlet", "outlet", "walls"]);
    }

    /// V&V (gh:#1270 remainder), prediction P10 of
    /// `verification_and_validation/patch_wise_layers_gh1270/README.md`:
    /// `tetMesh` and `pMesh` on the sphere R 3 (24 x 48, `maxCellSize`
    /// 0.75) with a `boundaryLayers` dict of `nLayers` 1 against the same
    /// run without: one layer cell per boundary face, closed, no
    /// non-positive cell at the end; with `nLayers` 0 nothing is added.
    #[test]
    fn tet_and_p_mesh_with_a_boundary_layers_dict() {
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let base = TetMeshOptions { dict: MeshDict::with_max_cell_size(0.75), ..Default::default() };
        let with = TetMeshOptions { boundary_layers: Some(BoundaryLayersDict::with_n_layers(1)), ..base.clone() };
        let zero = TetMeshOptions { boundary_layers: Some(BoundaryLayersDict::with_n_layers(0)), ..base.clone() };
        for kind in [Template::Tet, Template::Voronoi] {
            let (m0, _) = run_generator(kind, &p, &t, None, &base, None).unwrap();
            let (m1, rep) = run_generator(kind, &p, &t, None, &with, None).unwrap();
            let (mz, _) = run_generator(kind, &p, &t, None, &zero, None).unwrap();
            let nbf = m0.n_boundary_faces();
            for n in rep.notes.iter().filter(|n| n.starts_with("boundaryLayer")) {
                println!("  {kind:?}: {n}");
            }
            let q = check_quality(&m1);
            println!(
                "{kind:?}: without {} cells, {nbf} boundary faces, V {:.4}; with nLayers 1: {} cells ({} layer cells), V {:.4}, non-positive {}, non-ortho {:.1}",
                m0.n_cells,
                m0.total_volume(),
                m1.n_cells,
                rep.n_layer_cells,
                m1.total_volume(),
                crate::pipeline::cartesian_mesh::n_nonpositive_cells(&m1),
                q.max_non_orthogonality_deg
            );
            m1.validate().unwrap();
            assert_eq!((m1.n_cells, rep.n_layer_cells), (m0.n_cells + nbf, nbf));
            assert_eq!(crate::pipeline::cartesian_mesh::n_nonpositive_cells(&m1), 0);
            assert!(crate::tet_mesh_extractor_octree::same_mesh(&mz, &m0));
        }
    }
}
