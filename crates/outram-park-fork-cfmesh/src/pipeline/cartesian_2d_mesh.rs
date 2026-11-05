// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/cartesian2DMesh/cartesian2DMeshGenerator/cartesian2DMeshGenerator.{C,H}
//   executables/cartesian2DMesh/cartesian2DMesh.C
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

//! cfMesh's **`cartesian2DMesh`** workflow (`cartesian2DMeshGenerator`,
//! gh:#1275): a mesh one cell thick in z, from a **ribbon** surface (a
//! closed x-y outline extruded in z, [`crate::tri_surface_2d`]).
//!
//! The constructor checks the surface (`triSurface2DCheck`: a failure is a
//! note; upstream writes `badSurfaceWithSubsets.fms` and carries on) and
//! builds a **quadtree** ([`MeshOctree::new_quadtree`], `createOctreeBoxes`).
//! `generateMesh`:
//!
//! | step | upstream | here |
//! |---|---|---|
//! | `templateGeneration` | `cartesianMeshExtractor` on the quadtree, with the empty patches | [`crate::cartesian_mesh::cartesian_mesh_extractor`], [`crate::cartesian_mesh::create_empty_patches`] |
//! | `surfaceTopology` | as `cartesianMesh` | [`super::cartesian_mesh::cartesian_surface_preparation`] |
//! | `surfaceProjection` | `meshSurfaceMapper2D`: `adjustZCoordinates`, `preMapVertices`, `mapVerticesOntoSurface` | [`crate::surface_mapper_2d`] |
//! | `patchAssignment` | `meshSurfaceEdgeExtractor2D::distributeBoundaryFaces` | [`crate::surface_mapper_2d::distribute_boundary_faces`] |
//! | `edgeExtraction` | `remapBoundaryPoints`, then `optimiseMeshSurface` (`optimizeSurface2D`, `untangleSurface2D`) | [`crate::surface_mapper_2d::remap_boundary_points`]; **`optimizeSurface2D` / `untangleSurface2D` not ported** (noted) |
//! | `boundaryLayerGeneration` | `boundaryLayers` with `activate2DMode`, `addLayerForAllPatches` (always) | `layers::boundary_layers` on every non-empty patch as **one group**, the empty patches untreated (what 2D mode does to them); patch-wise groups not ported |
//! | `meshOptimisation` | `optimiseMeshSurface` again | **not ported** (noted): nothing moves |
//! | `boundaryLayerRefinement` | with a dict: `refineBoundaryLayers` in 2D mode, `untangleSurface2D` | **not ported** (2D mode): a dict gives a note, no split |
//! | (always) | `renumberMesh`, `renameBoundaryPatches(mesh, dict, true)` | as `cartesian_mesh`, empty patches allowed |
//!
//! Reused from the 3D workflow where upstream reuses it: the extractor, the
//! four connectivity checks, `boundaryLayers`, `renumberMesh`,
//! `renameBoundaryPatches`, `workflowControls`. `anisotropicSources`,
//! `triSurfacePatchManipulator` and `triSurfaceCleanupDuplicateTriangles`
//! (only run on a surface with feature edges) are not supported.

use crate::cartesian_mesh::{cartesian_mesh_extractor, create_empty_patches, CartesianMeshOptions};
use crate::layers::boundary_layers::{boundary_layers_add_layer, BoundaryLayersControls};
use crate::math::Vec3;
use crate::mesh_octree::{MeshOctree, TriSurf};
use crate::mesh_octree_creator::{MeshDict, MeshOctreeCreator};
use crate::patches::SurfaceRegions;
use crate::pipeline::cartesian_mesh::{cartesian_surface_preparation, n_nonpositive_cells};
use crate::pipeline::tet_mesh::SavedMesh;
use crate::rename_boundary_patches::{rename_boundary_patches, RenameBoundary};
use crate::renumber_mesh::{cells_from_mesh, renumber_mesh};
use crate::surface_mapper_2d::{adjust_z_coordinates, distribute_boundary_faces, map_vertices_onto_surface_2d, pre_map_vertices_2d, remap_boundary_points};
use crate::volume_mesh::VolumeMesh;
use crate::workflow_controls::{MeshMetaData, StepDecision, WorkflowControls, WorkflowSettings, WorkflowStep};
use std::sync::Arc;

/// Settings of [`cartesian_2d_mesh`].
#[derive(Debug, Clone, PartialEq)]
pub struct Cartesian2DMeshOptions {
    /// The quadtree settings (`maxCellSize`, `boundaryCellSize`, ...).
    pub dict: MeshDict,
    /// `workflowControls`.
    pub workflow: WorkflowSettings,
    /// `renameBoundary`.
    pub rename_boundary: Option<RenameBoundary>,
    /// `decomposePolyhedraIntoTetsAndPyrs` (upstream default `false`).
    pub decompose_polyhedra_into_tets_and_pyrs: bool,
    /// `boundaryLayers` creation settings.
    pub layer_heights: BoundaryLayersControls,
    /// A `boundaryLayers` dict with refinement settings is present (the 2D
    /// refinement is not ported: a note).
    pub refine_boundary_layers_requested: bool,
    /// **Ablation, not upstream:** skip `boundaryLayerGeneration`.
    pub without_boundary_layer: bool,
}

impl Default for Cartesian2DMeshOptions {
    fn default() -> Self {
        Cartesian2DMeshOptions {
            dict: MeshDict::with_max_cell_size(1.0),
            workflow: WorkflowSettings::default(),
            rename_boundary: None,
            decompose_polyhedra_into_tets_and_pyrs: false,
            layer_heights: BoundaryLayersControls::default(),
            refine_boundary_layers_requested: false,
            without_boundary_layer: false,
        }
    }
}

/// What [`cartesian_2d_mesh`] did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cartesian2DMeshReport {
    /// One line per step.
    pub notes: Vec<String>,
    /// The steps that ran.
    pub steps_run: Vec<WorkflowStep>,
    /// `stopAfter` hit.
    pub stopped_after: Option<WorkflowStep>,
    /// Meta data at the end.
    pub meta: MeshMetaData,
    /// Patch types.
    pub patch_types: Vec<String>,
    /// `triSurface2DCheck`'s warning, if the surface failed it.
    pub surface_check: Option<String>,
    /// Cells of the template.
    pub n_template_cells: usize,
    /// Cells removed by `surfaceTopology`.
    pub n_cells_removed_by_topology: usize,
    /// Corner edges `mapCorners` mapped in `edgeExtraction`.
    pub n_corner_edges: usize,
    /// Layer cells added.
    pub n_layer_cells: usize,
    /// Non-positive cells right after the layer.
    pub n_nonpositive_after_layer: usize,
}

/// `meshOctree(surf, true)` and `createOctreeBoxes`: the quadtree.
pub fn create_quadtree(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, dict: &MeshDict) -> MeshOctree {
    let surf = TriSurf::new(points.to_vec(), tris.to_vec(), regions.map(|r| r.region_of_tri.clone()));
    let mut o = MeshOctree::new_quadtree(Arc::new(surf));
    let mut c = MeshOctreeCreator::new(&o, Some(dict.clone()));
    c.create_octree_boxes(&mut o);
    o
}

/// cfMesh `cartesian2DMesh` on the ribbon `(points, tris)` with optional
/// named regions (module docs).
pub fn cartesian_2d_mesh(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &Cartesian2DMeshOptions) -> Result<(VolumeMesh, Cartesian2DMeshReport), String> {
    run_2d(points, tris, regions, opts, None)
}

/// [`cartesian_2d_mesh`] restarted from a saved mesh.
pub fn cartesian_2d_mesh_restart(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &Cartesian2DMeshOptions, saved: &SavedMesh) -> Result<(VolumeMesh, Cartesian2DMeshReport), String> {
    run_2d(points, tris, regions, opts, Some(saved))
}

/// `cartesian2DMeshGenerator::generateMesh`.
fn run_2d(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &Cartesian2DMeshOptions, saved: Option<&SavedMesh>) -> Result<(VolumeMesh, Cartesian2DMeshReport), String> {
    if let Some(r) = regions {
        r.validate(tris.len())?;
    }
    if opts.dict.max_cell_size <= 0.0 {
        return Err(format!("maxCellSize must be > 0 (got {})", opts.dict.max_cell_size));
    }
    let mut rep = Cartesian2DMeshReport::default();
    if let Err(w) = crate::tri_surface_2d::is_2d_surface(points, tris) {
        rep.notes.push(format!("triSurface2DCheck: {w} (upstream writes badSurfaceWithSubsets.fms and continues)"));
        rep.surface_check = Some(w);
    }
    let o = create_quadtree(points, tris, regions, &opts.dict);
    let single = SurfaceRegions::single("defaultFaces", tris.len());
    let reg = regions.unwrap_or(&single);
    let meta = saved.map(|s| s.meta.clone()).unwrap_or_default();
    let mut w = WorkflowControls::new(opts.workflow, meta);
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
            let (m, ty) = create_empty_patches(&cm.mesh, &o.root_box)?;
            rep.n_template_cells = m.n_cells;
            rep.notes.push(format!(
                "templateGeneration: cartesianMeshExtractor on the quadtree: {} cells; empty patches bottom {} / top {} faces, defaultFaces {}",
                m.n_cells, m.patches[1].n_faces, m.patches[2].n_faces, m.patches[0].n_faces
            ));
            types = ty;
            mesh = Some(m);
            continue;
        }
        let m = mesh.as_mut().ok_or("no mesh: the workflow resumed after a step that saved none")?;
        match step {
            WorkflowStep::SurfaceTopology => {
                let (passes, removed) = cartesian_surface_preparation(m)?;
                types.resize(m.patches.len(), "patch".into());
                rep.n_cells_removed_by_topology = removed;
                rep.notes.push(format!("surfaceTopology: the four connectivity checks ({passes} passes, {removed} cells removed)"));
            }
            WorkflowStep::SurfaceProjection => {
                adjust_z_coordinates(m, &o)?;
                let missed = pre_map_vertices_2d(m, &o, 3)?;
                map_vertices_onto_surface_2d(m, &o)?;
                rep.notes.push(format!("surfaceProjection: adjustZCoordinates, preMapVertices (3 iterations, {missed} searches found nothing), mapVerticesOntoSurface"));
            }
            WorkflowStep::PatchAssignment => {
                let (mm, ty) = distribute_boundary_faces(m, &o, &reg.names)?;
                *m = mm;
                types = ty;
                rep.notes.push(format!("patchAssignment: distributeBoundaryFaces: {:?}", m.patches.iter().map(|p| (p.name.clone(), p.n_faces)).collect::<Vec<_>>()));
            }
            WorkflowStep::EdgeExtraction => {
                let n = remap_boundary_points(m, &o)?;
                rep.n_corner_edges = n;
                rep.notes.push(format!("edgeExtraction: remapBoundaryPoints ({n} corner edges); optimiseMeshSurface (optimizeSurface2D, untangleSurface2D) NOT PORTED, skipped"));
            }
            WorkflowStep::BoundaryLayerGeneration => {
                if opts.without_boundary_layer {
                    rep.notes.push("boundaryLayerGeneration: ABLATED (without_boundary_layer)".into());
                } else {
                    let names: Vec<String> = m.patches.iter().enumerate().filter(|(i, p)| types.get(*i).is_none_or(|t| t != "empty") && p.n_faces > 0).map(|(_, p)| p.name.clone()).collect();
                    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                    match boundary_layers_add_layer(m, &refs, &opts.layer_heights) {
                        Some((mm, out)) => {
                            *m = mm;
                            rep.n_layer_cells = out.wall_faces.len();
                            rep.n_nonpositive_after_layer = n_nonpositive_cells(m);
                            rep.notes.push(format!(
                                "boundaryLayerGeneration: activate2DMode + addLayerForAllPatches as one group over {names:?} (empty patches untreated): {} layer cells, {} non-positive cells{}",
                                rep.n_layer_cells,
                                rep.n_nonpositive_after_layer,
                                if names.len() > 1 { "; patch-wise groups NOT ported" } else { "" }
                            ));
                        }
                        None => rep.notes.push("boundaryLayerGeneration: boundaryLayers refused; no layer".into()),
                    }
                }
            }
            WorkflowStep::MeshOptimisation => rep.notes.push("meshOptimisation: optimiseMeshSurface (optimizeSurface2D, untangleSurface2D) NOT PORTED; nothing moves".into()),
            WorkflowStep::BoundaryLayerRefinement => {
                if opts.refine_boundary_layers_requested {
                    rep.notes.push("boundaryLayerRefinement: refineBoundaryLayers::activate2DMode NOT PORTED; the layer is left unrefined".into());
                } else {
                    rep.notes.push("boundaryLayerRefinement: no boundaryLayers dict; nothing to do".into());
                }
            }
            _ => {}
        }
    }
    let m = mesh.ok_or("the workflow produced no mesh")?;
    let r = renumber_mesh(&m, &cells_from_mesh(&m));
    let renamed = rename_boundary_patches(&r.mesh, &types, opts.rename_boundary.as_ref(), true);
    w.workflow_completed();
    rep.meta = w.meta;
    rep.patch_types = renamed.patch_types;
    Ok((renamed.mesh, rep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
    use crate::poly_mesh_gen_2d_engine::PolyMeshGen2DEngine;
    use crate::tri_surface_2d::extrude_2d_edges;

    const L: f64 = 1.0;
    const H: f64 = 0.25;

    /// The channel outline `[0, L] x [0, H]`, extruded, with regions walls
    /// (y = 0, y = H), inlet (x = 0), outlet (x = L).
    fn channel_ribbon() -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
        let p = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(L, 0.0, 0.0), Vec3::new(L, H, 0.0), Vec3::new(0.0, H, 0.0)];
        let e = [[0, 1], [1, 2], [2, 3], [3, 0]];
        let (pts, tris, r) = extrude_2d_edges(&p, &e, Some(&[0, 2, 0, 1])).unwrap();
        let names = ["walls", "inlet", "outlet"];
        let labels: Vec<&str> = r.iter().map(|&i| names[i]).collect();
        (pts, tris, SurfaceRegions::from_labels(&labels))
    }

    /// The same channel as a closed 3D box, `z` in `[0, zt]`, the front and
    /// back in `frontAndBack`.
    fn channel_box(zt: f64) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
        let (p, t) = crate::shapes::box_surface(Vec3::ZERO, Vec3::new(L, H, zt));
        let labels: Vec<&str> = t
            .iter()
            .map(|x| {
                let c = p[x[0]].add(p[x[1]]).add(p[x[2]]).scale(1.0 / 3.0);
                let n = p[x[1]].sub(p[x[0]]).cross(p[x[2]].sub(p[x[0]]));
                if n.z.abs() > 0.5 * n.length() {
                    "frontAndBack"
                } else if n.x.abs() > 0.5 * n.length() {
                    if c.x < 0.5 * L { "inlet" } else { "outlet" }
                } else {
                    "walls"
                }
            })
            .collect();
        let r = SurfaceRegions::from_labels(&labels);
        (p, t, r)
    }

    /// Sorted x-y positions, rounded to 1e-9.
    fn xy_set(pts: impl Iterator<Item = Vec3>) -> Vec<(i64, i64)> {
        let mut v: Vec<(i64, i64)> = pts.map(|p| ((p.x * 1e9).round() as i64, (p.y * 1e9).round() as i64)).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// V&V (gh:#1275), predictions D2–D6 of
    /// `verification_and_validation/cartesian_2d_mesh_gh1275/README.md`:
    /// the channel `[0, 1] x [0, 0.25]` at `maxCellSize` 0.05. Checks: one
    /// cell thick (every point at z min or z max, half each, the 2D engine
    /// builds); the front and back exactly on the surface's z = 0 and z =
    /// 0.1 |diag|; closed; positive; patches; area against L x H; the
    /// template's x-y layout against `cartesian_mesh`'s mid-plane on the
    /// box channel (z in [0, 0.5]), and the final layouts. Results: the
    /// README.
    #[test]
    fn channel_2d_against_the_3d_mid_plane() {
        let (p, t, r) = channel_ribbon();
        let dz = p[4].z;
        let dict = MeshDict::with_max_cell_size(0.05);
        let opts = Cartesian2DMeshOptions { dict: dict.clone(), ..Default::default() };
        let (m, rep) = cartesian_2d_mesh(&p, &t, Some(&r), &opts).unwrap();
        for n in &rep.notes {
            println!("  2D: {n}");
        }
        m.validate().unwrap();
        let e2d = PolyMeshGen2DEngine::new(&m).unwrap();
        let zs: Vec<f64> = m.points.iter().map(|q| q.z).collect();
        let n_front = zs.iter().filter(|&&z| z == 0.0).count();
        let n_back = zs.iter().filter(|&&z| z == dz).count();
        println!("2D: cells {} points {} front {n_front} back {n_back} dz {dz}", m.n_cells, m.points.len());
        assert_eq!(n_front + n_back, m.points.len(), "every point exactly on z = 0 or z = dz");
        assert_eq!(n_front, n_back);
        let area = m.total_volume() / dz;
        println!("2D: area {area:.12} against {:.12} ({:+.3e})", L * H, (area - L * H) / (L * H));
        let nonpos = n_nonpositive_cells(&m);
        println!("2D: non-positive cells {nonpos}; patches {:?} types {:?}", m.patches.iter().map(|x| (x.name.clone(), x.n_faces)).collect::<Vec<_>>(), rep.patch_types);
        let q = crate::checks::check_quality(&m);
        println!("2D: max non-orth {:.2} max skew {:.3}", q.max_non_orthogonality_deg, q.max_skewness);
        // one cell thick: every cell has exactly one face in each empty patch
        let mut per_cell = vec![[0usize; 2]; m.n_cells];
        for (k, name) in ["bottomEmptyFaces", "topEmptyFaces"].iter().enumerate() {
            let pt = m.patches.iter().find(|x| x.name == *name).unwrap();
            for f in pt.start_face..pt.start_face + pt.n_faces {
                per_cell[m.owner[f]][k] += 1;
            }
        }
        let one_thick = per_cell.iter().filter(|c| c[0] == 1 && c[1] == 1).count();
        println!("2D: cells with one bottom and one top face: {one_thick} of {}", m.n_cells);

        // the template's x-y layout against the 3D mid-plane
        let o2 = create_quadtree(&p, &t, Some(&r), &dict);
        let t2 = cartesian_mesh_extractor(&o2, &CartesianMeshOptions::default()).mesh;
        let (bp, bt, br) = channel_box(0.5);
        let o3 = crate::pipeline::tet_mesh::create_octree(&bp, &bt, Some(&br), &dict);
        let t3 = cartesian_mesh_extractor(&o3, &CartesianMeshOptions::default()).mesh;
        let zmid = 0.25;
        let near = |q: &Vec3| (q.z - zmid).abs() < 1e-9;
        let s2 = xy_set(t2.points.iter().copied());
        let s3 = xy_set(t3.points.iter().copied().filter(near));
        println!("template: 2D cells {} x-y nodes {}; 3D cells {} mid-plane nodes {}; root boxes 2D {:?} 3D {:?}", t2.n_cells, s2.len(), t3.n_cells, s3.len(), o2.root_box, o3.root_box);
        println!("template x-y layouts identical: {}", s2 == s3);
        // the finished meshes
        let (m3, rep3) = cartesian_mesh(&bp, &bt, Some(&br), &CartesianMeshGeneratorOptions { dict: dict.clone(), ..Default::default() }).unwrap();
        for n in &rep3.notes {
            println!("  3D: {n}");
        }
        let f2 = xy_set(m.points.iter().copied().filter(|q| q.z == 0.0));
        let f3 = xy_set(m3.points.iter().copied().filter(near));
        let common = f2.iter().filter(|x| f3.binary_search(x).is_ok()).count();
        println!("final: 2D x-y nodes {}; 3D mid-plane nodes {}; in both (1e-9) {common}", f2.len(), f3.len());
        // the 3D mid-plane after the optimiser: nodes within 0.01 of z = 0.25
        let slab: Vec<Vec3> = m3.points.iter().copied().filter(|q| (q.z - zmid).abs() < 0.01).collect();
        let zdev = slab.iter().fold(0.0f64, |a, q| a.max((q.z - zmid).abs()));
        let mut dmax = 0.0f64;
        let mut dsum = 0.0;
        let bottoms: Vec<Vec3> = m.points.iter().copied().filter(|q| q.z == 0.0).collect();
        for q in &bottoms {
            let d = slab.iter().map(|s| ((s.x - q.x).powi(2) + (s.y - q.y).powi(2)).sqrt()).fold(f64::MAX, f64::min);
            dmax = dmax.max(d);
            dsum += d;
        }
        println!(
            "final: 3D slab |z - 0.25| < 0.01 holds {} nodes (max |z - 0.25| {zdev:.3e}); 2D node to nearest slab node in x-y: mean {:.3e}, max {dmax:.3e} (cell 0.047)",
            slab.len(),
            dsum / bottoms.len() as f64
        );
        assert!(s2 == s3, "template x-y layout equals the 3D mid-plane");
        assert_eq!(nonpos, 0);
        assert_eq!(one_thick, m.n_cells);
        assert!(e2d.z_min_point_labels.len() * 2 == m.points.len());
    }

    /// V&V (gh:#1275): the quadtree and its 2D searches. Every leaf spans
    /// the root box in z; leaves have at most 4 face neighbours; the
    /// 2D mesh's points pair up across the thickness (2D engine).
    #[test]
    fn quadtree_leaves_span_the_root_in_z() {
        let (p, t, r) = channel_ribbon();
        let o = create_quadtree(&p, &t, Some(&r), &MeshDict::with_max_cell_size(0.05));
        for l in 0..o.number_of_leaves() {
            let c = o.return_leaf(l).coords;
            assert_eq!(c.z, -10);
            let b = c.cube_box(&o.root_box);
            assert_eq!((b.min.z, b.max.z), (o.root_box.min.z, o.root_box.max.z));
            assert!(o.find_neighbours_for_leaf(&c).len() <= 8);
            let mid = c.centre(&o.root_box);
            assert_eq!(o.find_leaf_containing_vertex(mid), Some(l));
        }
        println!("quadtree leaves {}", o.number_of_leaves());
        let cm = cartesian_mesh_extractor(&o, &CartesianMeshOptions::default()).mesh;
        let (m, ty) = create_empty_patches(&cm, &o.root_box).unwrap();
        m.validate().unwrap();
        assert_eq!(ty, vec!["patch", "empty", "empty"]);
        assert_eq!(m.patches[1].n_faces, m.n_cells);
        assert_eq!(m.patches[2].n_faces, m.n_cells);
        let e = PolyMeshGen2DEngine::new(&m).unwrap();
        let mut moved = m.points.clone();
        for v in moved.iter_mut() {
            v.x += 1e-3 * v.z;
        }
        e.correct_points(&mut moved);
        for (i, &a) in e.z_min_point_labels.iter().enumerate() {
            let b = e.z_min_to_z_max[i];
            assert_eq!((moved[a].x, moved[a].y), (moved[b].x, moved[b].y));
        }
    }
}
