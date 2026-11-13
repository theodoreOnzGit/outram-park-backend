// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeCreator/
//     meshOctreeCreator.{C,H}, meshOctreeCreatorFrontalMarking.C,
//     meshOctreeCreatorCreateOctreeBoxes.C (setRootCubeSizeAndRefParameters,
//       refineInsideAndUnknownBoxes, createOctreeBoxes,
//       createOctreeWithRefinedBoundary),
//     meshOctreeCreatorAdjustOctreeToSurface.C (refineBoundary,
//       refineBoxesNearDataBoxes, refineBoxes)
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeInsideOutside/
//     meshOctreeInsideOutside.{C,H}
//   meshLibrary/utilities/helperFunctions/helperFunctionsFrontalMarking.C
//     (frontalMarking, groupMarking)
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

//! `meshOctreeCreator` and `meshOctreeInsideOutside` (gh:#1264): build the
//! octree to the sizes a [`MeshDict`] asks for, then mark every leaf
//! `INSIDE`, `OUTSIDE` or `DATA` by **frontal marking** from the root's
//! boundary.
//!
//! # The sizing
//!
//! `setRootCubeSizeAndRefParameters` grows the root box to
//! `maxCellSize x 2^L` (the smallest `L` whose cubes are below
//! `maxCellSize`), so level `L` cubes are the background cells. Each surface
//! triangle carries requests `(level, thickness)`: the global level, or the
//! `boundaryCellSize` level, plus any `patchCellSize` and `localRefinement`
//! entries for its region. `refineBoundary` refines every leaf holding a
//! triangle whose request is finer than the leaf (with
//! `refineSelectedBoxesAndAdditionalLayers` when a thickness is set).
//!
//! # Inside and outside
//!
//! [`mesh_octree_inside_outside`] is upstream's algorithm, serial: leaves
//! with triangles are `DATA`, the rest are flooded into groups over face
//! neighbours; a group that reaches the root boundary is `OUTSIDE`; a `DATA`
//! leaf next to an `OUTSIDE` group "has an outside neighbour"; a single-patch
//! `DATA` leaf without one but next to one that has becomes `INSIDE`
//! (`reviseDataBoxes`); a group touching such a `DATA` leaf becomes
//! `INSIDE`. Groups that are neither stay `UNKNOWN`. There is no ray
//! casting: [`crate::octree::inside`]'s ray parity is a different test, and
//! the two are compared in the tests.
//!
//! # The dictionary
//!
//! [`MeshDict`] holds the `meshDict` entries this module reads, as typed
//! fields; it is not a dictionary reader. Patches are named by **region
//! index** of the [`crate::mesh_octree::TriSurf`] (upstream matches patch
//! names, with regular expressions). **Since gh:#1476**,
//! [`crate::mesh_dict`] reads a `meshDict` file into it, matching the names
//! as upstream's `findPatches` does (full regular-expression match).
//!
//! ~~Not ported: `subsetCellSize` (no facet subsets on `TriSurf`; corrected
//! by gh:#1276: the subsets exist, the refinement did not),
//! `surfaceMeshRefinement` and `edgeMeshRefinement` sources,
//! `meshOctreeAutomaticRefinement` (curvature and proximity, its own item in
//! `docs/upstream_coverage.md`), `anisotropicSources`, and the MPI load
//! balancing.~~ **CORRECTED 2026-11-05 (gh:#1264/#1265 remainders,
//! gh:#1278):** `subsetCellSize` and subset `localRefinement` entries, the
//! `surfaceMeshRefinement` / `edgeMeshRefinement` sources
//! ([`MeshOctreeCreator::refine_boxes_intersecting_surfaces`],
//! [`MeshOctreeCreator::refine_boxes_intersecting_edge_meshes`]) and the
//! automatic refinement ([`crate::mesh_octree_automatic_refinement`], on
//! with `minCellSize`) are ported; each is off unless the dict asks.
//! `anisotropicSources` moves the sources ([`source_surface_geometry`] /
//! [`source_edge_mesh_geometry`], through gh:#1279's
//! [`crate::anisotropic`]). Still not ported: the MPI load balancing. Object refinement is [`crate::object_refinement`].

use crate::mesh_octree::{BoundBox, CubeType, MeshOctree, TriSurf, SMALL, VSMALL};
use crate::mesh_octree_automatic_refinement::{AutomaticRefinementReport, MeshOctreeAutomaticRefinement};
use crate::object_refinement::ObjectRefinement;
use std::sync::Arc;
use std::collections::BTreeMap;

/// A `patchCellSize` entry (`patchRefinement`): the cell size on one
/// surface region.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchRefinement {
    /// Region index in the surface.
    pub region: usize,
    /// Requested cell size.
    pub cell_size: f64,
}

/// A `localRefinement` entry: extra levels on one surface region, either
/// given directly or from a cell size, and an optional thickness of the
/// refined band.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalRefinement {
    /// Region index in the surface.
    pub region: usize,
    /// `additionalRefinementLevels`; takes precedence over `cell_size`.
    pub additional_levels: Option<u8>,
    /// `cellSize`.
    pub cell_size: Option<f64>,
    /// `refinementThickness` (0 for none).
    pub refinement_thickness: f64,
}

/// The `meshDict` entries the octree creator reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshDict {
    /// `maxCellSize` (required).
    pub max_cell_size: f64,
    /// `boundaryCellSize`.
    pub boundary_cell_size: Option<f64>,
    /// `boundaryCellSizeRefinementThickness` (0 for none).
    pub boundary_cell_size_refinement_thickness: f64,
    /// `patchCellSize`.
    pub patch_cell_size: Vec<PatchRefinement>,
    /// `localRefinement`.
    pub local_refinement: Vec<LocalRefinement>,
    /// `objectRefinements` ([`crate::object_refinement`], gh:#1265).
    pub object_refinements: Vec<ObjectRefinement>,
    /// `keepCellsIntersectingBoundary` (read by the extractor, gh:#1266).
    pub keep_cells_intersecting_boundary: bool,
    /// `minCellSize`: turns on `meshOctreeAutomaticRefinement`
    /// ([`crate::mesh_octree_automatic_refinement`], gh:#1278); `None` (the
    /// default) leaves it off, as upstream without the entry.
    pub min_cell_size: Option<f64>,
    /// `subsetCellSize`: a cell size on a facet subset of the surface, by
    /// subset name (gh:#1264 remainder).
    pub subset_cell_size: Vec<SubsetRefinement>,
    /// `localRefinement` entries naming a **facet subset** rather than a
    /// patch (upstream matches an entry's name against both).
    pub local_refinement_subsets: Vec<SubsetLocalRefinement>,
    /// `surfaceMeshRefinement`: surfaces acting as refinement sources.
    pub surface_mesh_refinement: Vec<SurfaceMeshRefinement>,
    /// `edgeMeshRefinement`: edge meshes acting as refinement sources.
    pub edge_mesh_refinement: Vec<EdgeMeshRefinement>,
    /// `nonManifoldMeshing`: `UNKNOWN` leaves become cells too, and DATA
    /// boxes are used (read by [`crate::cartesian_mesh`]'s addressing).
    pub non_manifold_meshing: bool,
    /// `checkForGluedMesh` (`checkGluedRegions`, with DATA boxes only).
    pub check_for_glued_mesh: bool,
    /// `keepCellsIntersectingPatches`: patch or facet-subset names whose
    /// leaves become cells (when DATA boxes are not used).
    pub keep_cells_intersecting_patches: Vec<String>,
    /// `removeCellsIntersectingPatches`: patch or facet-subset names whose
    /// leaves are dropped (when DATA boxes are used).
    pub remove_cells_intersecting_patches: Vec<String>,
    /// `anisotropicSources` ([`crate::anisotropic::CoordinateModifier`],
    /// gh:#1279). Read here by the refinement sources, which upstream maps
    /// into the modified space. ~~The generators do not read it.~~
    /// **UPDATED 2026-11-05 (gh:#1279 remainder):** the four generators
    /// (`pipeline::{cartesian_mesh, tet_mesh, p_mesh, cartesian_2d_mesh}`)
    /// read it as upstream: the octree on the modified surface, then the
    /// revert (and, in 3D, `projectSurfaceAfterBackScaling`).
    pub anisotropic_sources: Option<crate::anisotropic::CoordinateModifier>,
}

/// A `subsetCellSize` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SubsetRefinement {
    /// Facet subset name in the surface.
    pub subset: String,
    /// Requested cell size.
    pub cell_size: f64,
}

/// A `localRefinement` entry that names a facet subset.
#[derive(Debug, Clone, PartialEq)]
pub struct SubsetLocalRefinement {
    /// Facet subset name in the surface.
    pub subset: String,
    /// `additionalRefinementLevels`; takes precedence over `cell_size`.
    pub additional_levels: Option<u8>,
    /// `cellSize`.
    pub cell_size: Option<f64>,
    /// `refinementThickness` (0 for none).
    pub refinement_thickness: f64,
}

/// `edgeMesh`: points and edges (a refinement source). Upstream reads it
/// from a file; here it is a value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgeMesh {
    /// Points.
    pub points: Vec<crate::math::Vec3>,
    /// Edges (point labels).
    pub edges: Vec<[usize; 2]>,
}

/// A `surfaceMeshRefinement` entry. Upstream reads `surfaceFile`; here the
/// surface is given.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceMeshRefinement {
    /// The entry's name.
    pub name: String,
    /// The source surface.
    pub surface: Arc<TriSurf>,
    /// `cellSize` (takes precedence over `additional_levels`).
    pub cell_size: Option<f64>,
    /// `additionalRefinementLevels`.
    pub additional_levels: Option<u8>,
    /// `refinementThickness` (0 for none).
    pub refinement_thickness: f64,
}

/// An `edgeMeshRefinement` entry. Upstream reads `edgeFile`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeMeshRefinement {
    /// The entry's name.
    pub name: String,
    /// The source edge mesh.
    pub edge_mesh: Arc<EdgeMesh>,
    /// `cellSize` (takes precedence over `additional_levels`).
    pub cell_size: Option<f64>,
    /// `additionalRefinementLevels`.
    pub additional_levels: Option<u8>,
    /// `refinementThickness` (0 for none).
    pub refinement_thickness: f64,
}

/// A surface refinement source as `refineBoxesIntersectingSurfaces` uses
/// it: with `anisotropicSources` in the dict,
/// `surfaceMeshGeometryModification(origSurf, meshDict).modifyGeometry()`
/// ([`crate::anisotropic::modify_surface`], gh:#1279), else the source
/// unchanged.
pub fn source_surface_geometry(dict: &MeshDict, surf: &Arc<TriSurf>) -> Arc<TriSurf> {
    match &dict.anisotropic_sources {
        Some(cm) => Arc::new(crate::anisotropic::modify_surface(surf, cm, true)),
        None => Arc::clone(surf),
    }
}

/// An edge-mesh source as `refineBoxesIntersectingEdgeMeshes` uses it:
/// with `anisotropicSources`, `edgeMeshGeometryModification::modifyGeometry`
/// ([`crate::anisotropic::modify_points`]), else unchanged.
pub fn source_edge_mesh_geometry(dict: &MeshDict, em: &Arc<EdgeMesh>) -> Arc<EdgeMesh> {
    match &dict.anisotropic_sources {
        Some(cm) => {
            let mut m = (**em).clone();
            crate::anisotropic::modify_points(&mut m.points, cm);
            Arc::new(m)
        }
        None => Arc::clone(em),
    }
}

/// The additional levels of a refinement source's `cellSize`, by the loop
/// `refineBoxesIntersectingSurfaces` / `EdgeMeshes` use: count the halvings
/// `s = maxCellSize, maxCellSize/2, ...` with `cs <= s (1 + SMALL)`. **Not**
/// [`levels_to`]: at a `cellSize` equal to `maxCellSize / 2^k` it gives one
/// level more (upstream as written; recorded in gh:#1264's V&V).
pub fn source_add_levels(max_cell_size: f64, cs: f64) -> u8 {
    let mut add = 0u8;
    let mut s = max_cell_size;
    while cs <= s * (1.0 + SMALL) {
        add += 1;
        s /= 2.0;
    }
    add
}

impl MeshDict {
    /// Only `maxCellSize` set.
    pub fn with_max_cell_size(max_cell_size: f64) -> Self {
        MeshDict { max_cell_size, ..Default::default() }
    }
}

/// `meshOctreeCreator`.
#[derive(Debug, Clone)]
pub struct MeshOctreeCreator {
    /// `scalingFactor_`.
    pub scaling_factor: f64,
    /// The dictionary (`meshDictPtr_`); `None` creates no refinement.
    pub dict: Option<MeshDict>,
    /// `hexRefinement_`: refine all 8 sons when one is selected.
    pub hex_refinement: bool,
    /// `globalRefLevel_`: the level of `maxCellSize`.
    pub global_ref_level: u8,
    /// `surfRefLevel_`: per triangle, the `(level, thickness)` requests.
    pub surf_ref_level: Vec<Vec<(u8, f64)>>,
}

/// The smallest `n` with `max_size / 2^n <= cs` (upstream's `addLevel`
/// loop).
fn levels_to(max_size: f64, cs: f64) -> u8 {
    let mut n = 0u8;
    while max_size / (1u64 << n) as f64 > cs {
        n += 1;
    }
    n
}

impl MeshOctreeCreator {
    /// `meshOctreeCreator(meshOctree&, const IOdictionary&)`.
    pub fn new(octree: &MeshOctree, dict: Option<MeshDict>) -> Self {
        MeshOctreeCreator {
            scaling_factor: 1.0,
            dict,
            hex_refinement: false,
            global_ref_level: 0,
            surf_ref_level: vec![Vec::new(); octree.surface.size()],
        }
    }

    /// `activateHexRefinement`.
    pub fn activate_hex_refinement(&mut self) {
        self.hex_refinement = true;
    }

    /// `setRootCubeSizeAndRefParameters`. See the module docs.
    pub fn set_root_cube_size_and_ref_parameters(&mut self, o: &mut MeshOctree) {
        if o.is_root_initialised {
            return;
        }
        let Some(dict) = self.dict.clone() else {
            return;
        };
        let max_size = self.scaling_factor * dict.max_cell_size;
        o.search_range = 0.25 * max_size;
        let rb = o.root_box;
        let c = rb.max.add(rb.min).scale(0.5);
        let s = rb.span();
        let mut size = s.x;
        if s.y > size {
            size = s.y;
        }
        if s.z > size {
            size = s.z;
        }
        size += 0.5 * max_size;
        self.global_ref_level = 0;
        loop {
            let l_size = size / (1u64 << self.global_ref_level) as f64;
            if l_size < max_size * (1.0 - SMALL) {
                let bb = 0.5 * max_size * (1u64 << self.global_ref_level) as f64;
                let h = crate::math::Vec3::new(bb, bb, bb);
                o.root_box = crate::mesh_octree::BoundBox::new(c.sub(h), c.add(h));
                break;
            }
            self.global_ref_level += 1;
        }
        o.is_root_initialised = true;
        for r in self.surf_ref_level.iter_mut() {
            r.push((self.global_ref_level, 0.0));
        }
        if let Some(bcs) = dict.boundary_cell_size {
            let cs = bcs * (self.scaling_factor + SMALL);
            o.search_range = cs;
            let level = self.global_ref_level + levels_to(max_size, cs);
            let thickness = dict.boundary_cell_size_refinement_thickness.abs();
            for r in self.surf_ref_level.iter_mut() {
                r[0] = (level, thickness);
            }
        }
        for pr in &dict.patch_cell_size {
            let cs = pr.cell_size * (self.scaling_factor + SMALL);
            let level = self.global_ref_level + levels_to(max_size, cs);
            for (t, r) in self.surf_ref_level.iter_mut().enumerate() {
                if o.surface.regions[t] == pr.region {
                    r.push((level, 0.0));
                }
            }
        }
        // subsetCellSize: upstream warns and skips an unknown subset.
        for sr in &dict.subset_cell_size {
            let Some(id) = o.surface.facet_subsets.index(&sr.subset) else {
                continue;
            };
            let cs = sr.cell_size * (self.scaling_factor + SMALL);
            let level = self.global_ref_level + levels_to(max_size, cs);
            for t in o.surface.facet_subsets.elements(id) {
                self.surf_ref_level[t].push((level, 0.0));
            }
        }
        for lr in &dict.local_refinement {
            let n = match (lr.additional_levels, lr.cell_size) {
                (Some(n), _) => n,
                (None, Some(cs)) => levels_to(max_size, cs),
                (None, None) => 0,
            };
            let level = self.global_ref_level + n;
            for (t, r) in self.surf_ref_level.iter_mut().enumerate() {
                if o.surface.regions[t] == lr.region {
                    r.push((level, lr.refinement_thickness));
                }
            }
        }
        // localRefinement entries naming a facet subset.
        for lr in &dict.local_refinement_subsets {
            let Some(id) = o.surface.facet_subsets.index(&lr.subset) else {
                continue;
            };
            let n = match (lr.additional_levels, lr.cell_size) {
                (Some(n), _) => n,
                (None, Some(cs)) => levels_to(max_size, cs),
                (None, None) => 0,
            };
            let level = self.global_ref_level + n;
            for t in o.surface.facet_subsets.elements(id) {
                self.surf_ref_level[t].push((level, lr.refinement_thickness));
            }
        }
    }

    /// `refineBoundary`: refine leaves holding triangles until every
    /// triangle's finest request is met, adding layers where a thickness is
    /// requested. Repeats until nothing changes.
    pub fn refine_boundary(&self, o: &mut MeshOctree) {
        loop {
            let n = o.number_of_leaves();
            let mut refine = vec![0i32; n];
            let mut thick = vec![0.0f64; n];
            let mut use_n_layers = false;
            let mut changed = false;
            for l in 0..n {
                let oc = o.return_leaf(l);
                let Some(row) = oc.contained_elements else {
                    continue;
                };
                let lvl = oc.coords.level;
                let mut want = false;
                let mut min_level = 255u8;
                let mut max_thick = 0.0f64;
                for &t in &o.slot.contained_triangles[row] {
                    for &(rl, rt) in self.surf_ref_level[t].iter().rev() {
                        if rl <= lvl {
                            continue;
                        }
                        if rl < min_level {
                            want = true;
                            min_level = rl;
                            max_thick = rt;
                        } else if rl == min_level && rt > max_thick {
                            max_thick = rt;
                        }
                    }
                }
                if want {
                    refine[l] = 1;
                    if max_thick > VSMALL {
                        thick[l] = max_thick;
                        use_n_layers = true;
                    }
                    changed = true;
                }
            }
            if use_n_layers && changed {
                o.refine_selected_boxes_and_additional_layers(&refine, &thick);
            } else if changed {
                o.refine_selected_boxes(&mut refine, self.hex_refinement);
            }
            if !changed {
                break;
            }
        }
    }

    /// `refineBoxes(refLevel, cubeType)`: refine leaves of any of the types
    /// in `cube_type` below `ref_level`, until none is left.
    pub fn refine_boxes(&self, o: &mut MeshOctree, ref_level: u8, cube_type: u8) {
        loop {
            let n = o.number_of_leaves();
            let mut refine = vec![0i32; n];
            let mut n_refined = 0;
            for (l, r) in refine.iter_mut().enumerate() {
                let oc = o.return_leaf(l);
                if oc.cube_type & cube_type != 0 && oc.coords.level < ref_level {
                    *r = 1;
                    n_refined += 1;
                }
            }
            o.refine_selected_boxes(&mut refine, self.hex_refinement);
            if n_refined == 0 {
                break;
            }
        }
    }

    /// `refineInsideAndUnknownBoxes`: `INSIDE` and `UNKNOWN` leaves to the
    /// global level.
    pub fn refine_inside_and_unknown_boxes(&self, o: &mut MeshOctree) {
        self.refine_boxes(o, self.global_ref_level, CubeType::INSIDE + CubeType::UNKNOWN);
    }

    /// `refineBoxesNearDataBoxes(nLayers)`: refine every non-`OUTSIDE` leaf
    /// at a different (coarser) level among the 26 neighbours of a leaf with
    /// triangles, and `n_layers - 1` further rings; then re-mark
    /// inside/outside.
    pub fn refine_boxes_near_data_boxes(&self, o: &mut MeshOctree, n_layers: i32) {
        let n = o.number_of_leaves();
        let mut refine = vec![0i32; n];
        let mark_from = |o: &MeshOctree, l: usize, val: i32, refine: &mut Vec<i32>| {
            let cc = o.return_leaf(l).coords;
            for k in 0..26 {
                let Some(nei) = o.find_leaf_label_for_position(&cc.plus(&o.regularity_positions[k])) else {
                    continue;
                };
                let nc = o.return_leaf(nei);
                if nc.coords.level == cc.level || nc.cube_type & CubeType::OUTSIDE != 0 {
                    continue;
                }
                refine[nei] = val;
            }
        };
        for l in 0..n {
            if o.return_leaf(l).has_contained_elements() {
                mark_from(o, l, 1, &mut refine);
            }
        }
        for i in 1..n_layers {
            for l in 0..n {
                if refine[l] == i {
                    mark_from(o, l, i + 1, &mut refine);
                }
            }
        }
        o.refine_selected_boxes(&mut refine, self.hex_refinement);
        mesh_octree_inside_outside(o);
    }

    /// `refineBoxesContainedInObjects`: see [`crate::object_refinement`].
    pub fn refine_boxes_contained_in_objects(&self, o: &mut MeshOctree) {
        let Some(dict) = &self.dict else {
            return;
        };
        if dict.object_refinements.is_empty() {
            return;
        }
        crate::object_refinement::refine_boxes_contained_in_objects(
            o,
            &dict.object_refinements,
            dict.max_cell_size,
            self.global_ref_level,
            self.hex_refinement,
        );
    }

    /// The refinement level of each source, `globalRefLevel_` plus the
    /// source's `cellSize` levels ([`source_add_levels`]) or
    /// `additionalRefinementLevels`.
    fn source_level(&self, max_cell_size: f64, cell_size: Option<f64>, additional: Option<u8>) -> u8 {
        let add = match cell_size {
            Some(cs) => source_add_levels(max_cell_size, cs),
            None => additional.unwrap_or(0),
        };
        self.global_ref_level + add
    }

    /// Refine with layers when any marked leaf asks for a thickness, else
    /// plainly (the tail of both source routines).
    fn refine_marked(&self, o: &mut MeshOctree, refine: &mut [i32], thick: &[f64], use_n_layers: bool) {
        if use_n_layers {
            o.refine_selected_boxes_and_additional_layers(refine, thick);
        } else {
            o.refine_selected_boxes(refine, self.hex_refinement);
        }
    }

    /// `refineBoxesIntersectingSurfaces`: refine every leaf a source
    /// surface's triangle intersects (`intersectsTriangleExact`, among the
    /// leaves overlapping the triangle's box grown by `SMALL x` the root
    /// span) below the source's level, with `refinementThickness` layers,
    /// until nothing changes. Returns the passes that refined.
    pub fn refine_boxes_intersecting_surfaces(&self, o: &mut MeshOctree) -> usize {
        let Some(dict) = &self.dict else {
            return 0;
        };
        if dict.surface_mesh_refinement.is_empty() {
            return 0;
        }
        let sources: Vec<(Arc<TriSurf>, u8, f64)> = dict
            .surface_mesh_refinement
            .iter()
            .map(|r| (source_surface_geometry(dict, &r.surface), self.source_level(dict.max_cell_size, r.cell_size, r.additional_levels), r.refinement_thickness))
            .collect();
        let tol = o.root_box.span().scale(SMALL);
        let mut n_passes = 0;
        loop {
            let n = o.number_of_leaves();
            let mut refine = vec![0i32; n];
            let mut thick = vec![0.0f64; n];
            let (mut changed, mut use_n_layers) = (false, false);
            for (surf, level, th) in &sources {
                for (t, tri) in surf.triangles.iter().enumerate() {
                    let pts = [surf.points[tri[0]], surf.points[tri[1]], surf.points[tri[2]]];
                    let bb = BoundBox::of_points(&pts);
                    let bb = BoundBox::new(bb.min.sub(tol), bb.max.add(tol));
                    for leaf in o.find_leaves_contained_in_box(&bb) {
                        let oc = o.return_leaf(leaf).coords;
                        if !oc.intersects_triangle_exact(surf, &o.root_box, t) {
                            continue;
                        }
                        if oc.level < *level {
                            changed = true;
                            refine[leaf] = 1;
                        }
                        if *th > VSMALL {
                            use_n_layers = true;
                            thick[leaf] = thick[leaf].max(*th);
                        }
                    }
                }
            }
            if !changed {
                break;
            }
            self.refine_marked(o, &mut refine, &thick, use_n_layers);
            n_passes += 1;
        }
        n_passes
    }

    /// `refineBoxesIntersectingEdgeMeshes`: as
    /// [`MeshOctreeCreator::refine_boxes_intersecting_surfaces`] for the
    /// edges of the source edge meshes (`intersectsLine`). Returns the
    /// passes that refined.
    pub fn refine_boxes_intersecting_edge_meshes(&self, o: &mut MeshOctree) -> usize {
        let Some(dict) = &self.dict else {
            return 0;
        };
        if dict.edge_mesh_refinement.is_empty() {
            return 0;
        }
        let sources: Vec<(Arc<EdgeMesh>, u8, f64)> = dict
            .edge_mesh_refinement
            .iter()
            .map(|r| (source_edge_mesh_geometry(dict, &r.edge_mesh), self.source_level(dict.max_cell_size, r.cell_size, r.additional_levels), r.refinement_thickness))
            .collect();
        let tol = o.root_box.span().scale(SMALL);
        let mut n_passes = 0;
        loop {
            let n = o.number_of_leaves();
            let mut refine = vec![0i32; n];
            let mut thick = vec![0.0f64; n];
            let (mut changed, mut use_n_layers) = (false, false);
            for (em, level, th) in &sources {
                for e in &em.edges {
                    let (sp, ep) = (em.points[e[0]], em.points[e[1]]);
                    let bb = BoundBox::of_points(&[sp, ep]);
                    let bb = BoundBox::new(bb.min.sub(tol), bb.max.add(tol));
                    let mut intersected = Vec::new();
                    for leaf in o.find_leaves_contained_in_box(&bb) {
                        let oc = o.return_leaf(leaf).coords;
                        if oc.intersects_line(&o.root_box, sp, ep) {
                            intersected.push(leaf);
                            if oc.level < *level {
                                refine[leaf] = 1;
                                changed = true;
                            }
                        }
                    }
                    if *th > VSMALL {
                        use_n_layers = true;
                        for leaf in intersected {
                            thick[leaf] = thick[leaf].max(*th);
                        }
                    }
                }
            }
            if !changed {
                break;
            }
            self.refine_marked(o, &mut refine, &thick, use_n_layers);
            n_passes += 1;
        }
        n_passes
    }

    /// The automatic refinement step of `createOctreeBoxes` (upstream runs
    /// it in serial runs only; the port is serial):
    /// `meshOctreeAutomaticRefinement` with `useDATABoxes = false`; nothing
    /// unless the dict sets `minCellSize`.
    pub fn automatic_refinement(&self, o: &mut MeshOctree) -> AutomaticRefinementReport {
        let Some(dict) = &self.dict else {
            return AutomaticRefinementReport::default();
        };
        let mut a = MeshOctreeAutomaticRefinement::new(o, dict.min_cell_size, dict.max_cell_size, dict.keep_cells_intersecting_boundary, false);
        if self.hex_refinement {
            a.activate_hex_refinement();
        }
        a.automatic_refinement(o);
        a.report()
    }

    /// `createOctreeBoxes`: size the root, refine to the boundary requests,
    /// refine the leaves the source surfaces and edge meshes cross, run the
    /// automatic (curvature and proximity) refinement, mark inside/outside,
    /// refine `INSIDE` and `UNKNOWN` leaves to `maxCellSize`, refine inside
    /// the refinement objects, and refine the coarser neighbours of the
    /// `DATA` leaves once. The three new middle steps do nothing unless the
    /// dict asks for them (all off by default).
    pub fn create_octree_boxes(&mut self, o: &mut MeshOctree) {
        self.set_root_cube_size_and_ref_parameters(o);
        self.refine_boundary(o);
        trace_octree("refineBoundary", o);
        self.refine_boxes_intersecting_surfaces(o);
        self.refine_boxes_intersecting_edge_meshes(o);
        trace_octree("sources", o);
        self.automatic_refinement(o);
        trace_octree("auto", o);
        mesh_octree_inside_outside(o);
        self.refine_inside_and_unknown_boxes(o);
        trace_octree("inside", o);
        self.refine_boxes_contained_in_objects(o);
        self.refine_boxes_near_data_boxes(o, 1);
        trace_octree("nearData", o);
    }

    /// `createOctreeWithRefinedBoundary`: refine every leaf holding more
    /// than `n_triangles_in_leaf` triangles, down to `max_level` (no
    /// regularity rule; for searches).
    pub fn create_octree_with_refined_boundary(&self, o: &mut MeshOctree, max_level: u8, n_triangles_in_leaf: usize) {
        loop {
            let mut n_marked = 0;
            for l in 0..o.number_of_leaves() {
                let ci = o.leaves[l];
                let c = o.cube(ci);
                let nt = c.contained_elements.map_or(0, |r| o.slot.contained_triangles[r].len());
                if c.coords.level < max_level && nt > n_triangles_in_leaf {
                    o.refine_cube(ci);
                    n_marked += 1;
                }
            }
            if n_marked == 0 {
                break;
            }
            o.create_list_of_leaves();
        }
    }
}

/// With `GH1385_TRACE` set, print the leaf count and the leaves per level as
/// upstream's instrumented `createOctreeBoxes` does (`OCT <step> leaves <n>
/// levels <l>:<count> ...`, gh:#1385).
fn trace_octree(tag: &str, o: &MeshOctree) {
    if std::env::var("GH1385_TRACE").is_err() {
        return;
    }
    let mut hist = [0usize; 32];
    for l in 0..o.number_of_leaves() {
        hist[o.cube(o.leaves[l]).coords.level as usize] += 1;
    }
    let levels: Vec<String> = hist.iter().enumerate().filter(|(_, &n)| n > 0).map(|(l, n)| format!("{l}:{n}")).collect();
    eprintln!("OCT {tag} leaves {} levels {}", o.number_of_leaves(), levels.join(" "));
}

/// Leaf counts by type after inside/outside marking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InsideOutsideCounts {
    /// `INSIDE` leaves.
    pub inside: usize,
    /// `UNKNOWN` leaves.
    pub unknown: usize,
    /// `DATA` leaves.
    pub data: usize,
    /// `OUTSIDE` leaves.
    pub outside: usize,
}

/// `meshOctreeInsideOutside`, serial: set every leaf's type (module docs).
/// Returns upstream's summary counts.
pub fn mesh_octree_inside_outside(o: &mut MeshOctree) -> InsideOutsideCounts {
    let n = o.number_of_leaves();
    // initialiseBoxes
    for l in 0..n {
        let ci = o.leaves[l];
        let t = if o.cube(ci).has_contained_elements() { CubeType::DATA } else { CubeType::UNKNOWN };
        o.slot.cubes[ci].cube_type = t;
    }
    // frontalMarking (one thread: every neighbour is local).
    let mut cube_group: Vec<Option<usize>> = vec![None; n];
    let mut group_type: BTreeMap<usize, u8> = BTreeMap::new();
    let mut boundary_data: Vec<Vec<usize>> = Vec::new();
    let mut n_group = 0;
    for l in 0..n {
        if o.return_leaf(l).has_contained_elements() || cube_group[l].is_some() {
            continue;
        }
        let g = n_group;
        n_group += 1;
        let mut c_type = CubeType::UNKNOWN;
        let mut front = vec![l];
        cube_group[l] = Some(g);
        let mut nei_data: Vec<usize> = Vec::new();
        while let Some(f) = front.pop() {
            let cc = o.return_leaf(f).coords;
            for nei in o.find_neighbours_for_leaf(&cc) {
                match nei {
                    None => c_type = CubeType::OUTSIDE,
                    Some(m) => {
                        if cube_group[m].is_some() {
                            continue;
                        }
                        if o.return_leaf(m).has_contained_elements() {
                            nei_data.push(m);
                        } else {
                            front.push(m);
                            cube_group[m] = Some(g);
                        }
                    }
                }
            }
        }
        boundary_data.push(nei_data);
        group_type.insert(g, c_type);
    }
    let mut cubes_in_group: Vec<Vec<usize>> = vec![Vec::new(); n_group];
    for (l, g) in cube_group.iter().enumerate() {
        if let Some(g) = g {
            cubes_in_group[*g].push(l);
        }
    }
    // markOutsideCubes (no inter-thread or inter-processor groups).
    let mut has_outside_nei = vec![false; n];
    for (&g, &t) in &group_type {
        if t & CubeType::OUTSIDE != 0 {
            for &l in &cubes_in_group[g] {
                let ci = o.leaves[l];
                o.slot.cubes[ci].cube_type = CubeType::OUTSIDE;
            }
            for &d in &boundary_data[g] {
                has_outside_nei[d] = true;
            }
        }
    }
    // reviseDataBoxes
    loop {
        let mut n_marked = 0;
        for l in 0..n {
            let ci = o.leaves[l];
            if o.cube(ci).cube_type & CubeType::DATA == 0 || has_outside_nei[l] {
                continue;
            }
            if o.find_boundary_patches_for_leaf(l).len() > 1 {
                continue;
            }
            let cc = o.cube(ci).coords;
            if o.find_all_leaf_neighbours(&cc).into_iter().flatten().any(|m| has_outside_nei[m]) {
                o.slot.cubes[ci].cube_type = CubeType::INSIDE;
                n_marked += 1;
            }
        }
        if n_marked == 0 {
            break;
        }
    }
    // markInsideCubes
    for (&g, t) in group_type.iter_mut() {
        if *t & CubeType::UNKNOWN != 0
            && boundary_data[g]
                .iter()
                .any(|&d| has_outside_nei[d] || o.return_leaf(d).cube_type & CubeType::INSIDE != 0)
        {
            *t = CubeType::INSIDE;
        }
    }
    for (&g, &t) in &group_type {
        if t & CubeType::INSIDE != 0 {
            for &l in &cubes_in_group[g] {
                let ci = o.leaves[l];
                o.slot.cubes[ci].cube_type = CubeType::INSIDE;
            }
        }
    }
    let mut c = InsideOutsideCounts::default();
    for l in 0..n {
        let t = o.return_leaf(l).cube_type;
        if t & CubeType::INSIDE != 0 {
            c.inside += 1;
        } else if t & CubeType::UNKNOWN != 0 {
            c.unknown += 1;
        } else if t & CubeType::DATA != 0 {
            c.data += 1;
        } else if t & CubeType::OUTSIDE != 0 {
            c.outside += 1;
        }
    }
    c
}

/// `help::frontalMarking`: the elements reachable from `start` through
/// `neighbours`, keeping only those `selector` accepts. `start` is listed
/// first; as upstream, it is not marked as visited, so it is listed again if
/// it is reachable from one of its neighbours.
pub fn frontal_marking<N, S>(start: usize, n: usize, neighbours: N, selector: S) -> Vec<usize>
where
    N: Fn(usize) -> Vec<usize>,
    S: Fn(usize) -> bool,
{
    let mut result = vec![start];
    let mut front = vec![start];
    let mut selected = vec![false; n];
    while let Some(e) = front.pop() {
        for nei in neighbours(e) {
            if nei >= n || selected[nei] {
                continue;
            }
            if selector(nei) {
                selected[nei] = true;
                front.push(nei);
                result.push(nei);
            }
        }
    }
    result
}

/// `help::groupMarking`, serial: number the connected components of the
/// elements `selector` accepts, in order of their lowest element. Returns
/// the group of each element (`None` if not selected) and the group count.
pub fn group_marking<N, S>(n: usize, neighbours: N, selector: S) -> (Vec<Option<usize>>, usize)
where
    N: Fn(usize) -> Vec<usize>,
    S: Fn(usize) -> bool,
{
    let mut group: Vec<Option<usize>> = vec![None; n];
    let mut n_groups = 0;
    for e in 0..n {
        if group[e].is_some() || !selector(e) {
            continue;
        }
        group[e] = Some(n_groups);
        let mut front = vec![e];
        while let Some(f) = front.pop() {
            for nei in neighbours(f) {
                if nei >= n || group[nei].is_some() {
                    continue;
                }
                if selector(nei) {
                    group[nei] = Some(n_groups);
                    front.push(nei);
                }
            }
        }
        n_groups += 1;
    }
    (group, n_groups)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::math::Vec3;
    use crate::mesh_octree::TriSurf;
    use crate::mesh_octree_queries::tests::pipe;
    use std::sync::Arc;

    pub(crate) fn sphere3() -> Arc<TriSurf> {
        let (p, t) = crate::shapes::sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        Arc::new(TriSurf::new(p, t, None))
    }

    /// Build with `createOctreeBoxes`.
    pub(crate) fn create(surf: Arc<TriSurf>, dict: MeshDict) -> (MeshOctree, MeshOctreeCreator) {
        let mut o = MeshOctree::new(surf);
        let mut c = MeshOctreeCreator::new(&o, Some(dict));
        c.create_octree_boxes(&mut o);
        (o, c)
    }

    /// Classify every leaf against ray parity
    /// ([`crate::octree::inside`]) at its centre. Returns
    /// (non-DATA leaves disagreeing, DATA leaves typed INSIDE whose centre
    /// is outside, UNKNOWN leaves).
    fn compare_with_ray_parity(o: &MeshOctree) -> (usize, usize, usize) {
        let (p, t) = (&o.surface.points, &o.surface.triangles);
        let (mut bad, mut data_bad, mut unknown) = (0, 0, 0);
        for l in 0..o.number_of_leaves() {
            let c = o.return_leaf(l);
            let ins = crate::octree::inside(c.coords.centre(&o.root_box), p, t);
            if c.cube_type & CubeType::UNKNOWN != 0 {
                unknown += 1;
            }
            if c.has_contained_elements() {
                if c.cube_type & CubeType::INSIDE != 0 && !ins {
                    data_bad += 1;
                }
                continue;
            }
            let typed_inside = c.cube_type & CubeType::INSIDE != 0;
            let typed_outside = c.cube_type & CubeType::OUTSIDE != 0;
            if (ins && !typed_inside) || (!ins && !typed_outside) {
                bad += 1;
            }
        }
        (bad, data_bad, unknown)
    }

    /// V&V (gh:#1264): `createOctreeBoxes` sizing and the frontal
    /// inside/outside marking, differential against ray parity.
    ///
    /// # Methodology
    ///
    /// Sphere R 3 m (24 x 48) with `maxCellSize` 1.0 and `boundaryCellSize`
    /// 0.25; the 48-sided pipe (R 10 mm, L 60 mm) with `maxCellSize` 4 mm
    /// and `boundaryCellSize` 1 mm. Every leaf's centre is classified by
    /// `octree::inside` (ray parity) and compared with the type the frontal
    /// marking gave it.
    ///
    /// **Predictions (written before the run):**
    /// - sphere: root box side 8 (`L` = 3: 6.5 / 8 < 1), every leaf holding
    ///   triangles has size exactly 0.25 (level 5);
    /// - pipe: root side 0.032 m (`L` = 3), triangle leaves 1 mm (level 5);
    /// - **both sizing predictions missed** (first run): upstream sizes from
    ///   the *initial root box*, which is already 1.5 x the surface's
    ///   extent, not from the extent. Sphere: 9 + 0.5 = 9.5, `L` = 4, root
    ///   side 16, triangle leaves at level 6 (still 0.25 m, as requested).
    ///   Pipe: 0.09 + 0.002, `L` = 5, root side 0.128 m, triangle leaves at
    ///   level 7 (1 mm). The test asserts these corrected values;
    /// - every leaf without triangles is `INSIDE` iff ray parity says its
    ///   centre is inside, else `OUTSIDE` — 0 disagreements, 0 `UNKNOWN`
    ///   (a non-DATA leaf does not cross the surface, so its centre's side
    ///   is the whole leaf's side);
    /// - every non-`OUTSIDE` leaf touching a triangle leaf is at least as
    ///   fine as it (`refineBoxesNearDataBoxes`), and the octree is
    ///   1-irregular;
    /// - `DATA` leaves revised to `INSIDE` have inside centres (no
    ///   prediction of how many).
    ///
    /// # Results (2026-11-05)
    ///
    /// | surface | leaves | INSIDE | DATA | OUTSIDE | UNKNOWN | DATA revised to INSIDE | of those, centre outside | non-DATA vs ray parity |
    /// |---|---|---|---|---|---|---|---|---|
    /// | sphere | 10 816 | 4 312 | 1 584 | 4 920 | 0 | 1 064 | **48** | 0 disagreements |
    /// | pipe 48 | 20 952 | 8 512 | 4 352 | 8 088 | 0 | 1 928 | 0 | 0 disagreements |
    ///
    /// (INSIDE counts include the revised DATA leaves.) Pinned in
    /// [`CREATOR_PINS`].
    /// - Sizing: as corrected above (the first predictions missed).
    /// - Frontal marking against ray parity: **as predicted**, 0
    ///   disagreements and 0 `UNKNOWN` on both surfaces.
    /// - `refineBoxesNearDataBoxes` and 1-irregularity: as predicted.
    /// - Revised `DATA` leaves: **48 on the sphere have their centre outside
    ///   the surface.** `reviseDataBoxes` retypes a single-patch `DATA` leaf
    ///   with no `OUTSIDE` face neighbour that touches (over a node, edge or
    ///   face) a `DATA` leaf with one; on a curved wall at 0.25 m such a
    ///   leaf can still have most of its volume outside. This is upstream's
    ///   rule; it is pinned, not fixed.
    /// (leaves, INSIDE, DATA, OUTSIDE, DATA leaves revised to INSIDE, of
    /// those with an outside centre), sphere then pipe.
    const CREATOR_PINS: [(usize, usize, usize, usize, usize, usize); 2] = [(10816, 4312, 1584, 4920, 1064, 48), (20952, 8512, 4352, 8088, 1928, 0)];

    #[test]
    fn creator_sizes_and_frontal_marking_match_ray_parity() {
        for (name, surf, dict, side, lvl) in [
            ("sphere", sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) }, 16.0, 6),
            ("pipe", pipe(0.01, 0.06, 48), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) }, 0.128, 7),
        ] {
            let (o, c) = create(surf, dict);
            assert!((o.root_box.span().x - side).abs() < 1e-12 * side, "{name}: root side {}", o.root_box.span().x);
            assert_eq!(c.global_ref_level, lvl - 2, "{name}");
            let mut counts = InsideOutsideCounts::default();
            for l in 0..o.number_of_leaves() {
                let lf = o.return_leaf(l);
                if lf.has_contained_elements() {
                    assert_eq!(lf.coords.level, lvl, "{name}: triangle leaf level");
                }
                let t = lf.cube_type;
                if t & CubeType::INSIDE != 0 {
                    counts.inside += 1;
                } else if t & CubeType::DATA != 0 {
                    counts.data += 1;
                } else if t & CubeType::OUTSIDE != 0 {
                    counts.outside += 1;
                } else {
                    counts.unknown += 1;
                }
                if lf.has_contained_elements() {
                    let cc = lf.coords;
                    for m in o.find_all_leaf_neighbours(&cc).into_iter().flatten() {
                        let nl = o.return_leaf(m);
                        if nl.cube_type & CubeType::OUTSIDE == 0 {
                            assert!(nl.coords.level >= cc.level, "{name}: coarse neighbour of DATA leaf");
                        }
                    }
                }
                for m in o.find_all_leaf_neighbours(&lf.coords).into_iter().flatten() {
                    assert!((o.return_leaf(m).coords.level as i32 - lf.coords.level as i32).abs() <= 1);
                }
            }
            let (bad, data_bad, unknown) = compare_with_ray_parity(&o);
            println!("{name}: {} leaves, {counts:?}; ray-parity disagreements {bad}, revised DATA outside {data_bad}", o.number_of_leaves());
            let n_revised = (0..o.number_of_leaves())
                .filter(|&l| o.return_leaf(l).has_contained_elements() && o.return_leaf(l).cube_type & CubeType::INSIDE != 0)
                .count();
            println!("{name}: DATA leaves revised to INSIDE {n_revised}");
            assert_eq!((bad, unknown), (0, 0), "{name}");
            let pins = if name == "sphere" { CREATOR_PINS[0] } else { CREATOR_PINS[1] };
            assert_eq!(
                (o.number_of_leaves(), counts.inside, counts.data, counts.outside, n_revised, data_bad),
                pins,
                "{name}"
            );
        }
    }

    /// V&V (gh:#1264), differential: leaf counts of the ported creator
    /// against [`crate::octree::refine_near_boundary_banded`] on the sphere
    /// R 3 m at the README's graded arm (base 1.0 m, one level, band 1.0):
    /// the creator with `maxCellSize` 1.0 and `boundaryCellSize` 0.5.
    ///
    /// The two keep different cells: `octree.rs` keeps the cells with an
    /// inside centre; upstream's cartesian mesh keeps `INSIDE` leaves (and
    /// `DATA` ones with `keepCellsIntersectingBoundary`). **Prediction
    /// (written before the run):** the two counts within a factor 2 of
    /// each other, no closer prediction. Result (2026-11-05): `octree.rs`
    /// 864 cells; creator 792 `INSIDE` leaves, 1 224 with the `DATA`
    /// leaves. As predicted (0.92x and 1.42x). Pinned.
    #[test]
    fn creator_leaf_counts_against_octree_rs_banded() {
        let (p, t) = crate::shapes::sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let old = crate::octree::refine_near_boundary_banded(&p, &t, 1.0, 1, 1.0).cell_count();
        let dict = MeshDict { boundary_cell_size: Some(0.5), ..MeshDict::with_max_cell_size(1.0) };
        let (o, _) = create(sphere3(), dict);
        let mut inside = 0;
        let mut kept_with_data = 0;
        for l in 0..o.number_of_leaves() {
            let t = o.return_leaf(l).cube_type;
            if t & CubeType::INSIDE != 0 {
                inside += 1;
            }
            if t & (CubeType::INSIDE | CubeType::DATA) != 0 {
                kept_with_data += 1;
            }
        }
        println!("octree.rs banded {old}; creator INSIDE {inside}, INSIDE + DATA {kept_with_data}");
        assert!(inside * 2 >= old && inside <= old * 2, "within a factor 2");
        assert_eq!((old, inside, kept_with_data), BANDED_PINS);
    }
    const BANDED_PINS: (usize, usize, usize) = (864, 792, 1224);

    /// V&V (gh:#1264): `help::groupMarking` / `frontalMarking` on a small
    /// graph (a path 0-1-2, an isolated 3, a path 4-5, with 2 excluded by
    /// the selector): groups {0, 1}, {3}, {4, 5}; the frontal marking from
    /// 4 lists 4, 5 and then 4 again (upstream does not mark the start as
    /// visited).
    #[test]
    fn group_and_frontal_marking_on_a_small_graph() {
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![0, 2], vec![1], vec![], vec![5], vec![4]];
        let (g, n) = group_marking(6, |e| adj[e].clone(), |e| e != 2);
        assert_eq!(n, 3);
        assert_eq!(g, vec![Some(0), Some(0), None, Some(1), Some(2), Some(2)]);
        assert_eq!(frontal_marking(4, 6, |e| adj[e].clone(), |_| true), vec![4, 5, 4]);
    }

    /// V&V (gh:#1264): `createOctreeWithRefinedBoundary` leaves no leaf
    /// with more than the limit unless at the level cap, and the sizing
    /// helper `levels_to` matches upstream's loop at its boundaries.
    #[test]
    fn refined_boundary_octree_and_level_helper() {
        let mut o = MeshOctree::new(sphere3());
        let c = MeshOctreeCreator::new(&o, None);
        c.create_octree_with_refined_boundary(&mut o, 6, 15);
        for l in 0..o.number_of_leaves() {
            let lf = o.return_leaf(l);
            assert!(lf.coords.level == 6 || o.contained_triangles(l).len() <= 15);
        }
        assert_eq!(levels_to(1.0, 1.0), 0);
        assert_eq!(levels_to(1.0, 0.5), 1);
        assert_eq!(levels_to(1.0, 0.49), 2);
        assert_eq!(levels_to(1.0, 0.25), 2);
    }

    fn leaf_coords(o: &MeshOctree) -> Vec<crate::mesh_octree::CubeCoordinates> {
        (0..o.number_of_leaves()).map(|l| o.return_leaf(l).coords).collect()
    }

    /// V&V (gh:#1264/#1265 remainders), differential:
    /// `refineBoxesIntersectingSurfaces` against `refineBoundary` on the
    /// **same triangles**.
    ///
    /// # Methodology
    ///
    /// Sphere R 3 m, `maxCellSize` 1.0. Route A: `boundaryCellSize` 0.25
    /// (level 4 + 2 = 6, gh:#1264). Route B: no boundary size, the sphere
    /// itself as a `surfaceMeshRefinement` source with
    /// `additionalRefinementLevels` 2. Both run the whole
    /// `createOctreeBoxes`.
    ///
    /// **Predictions (written before the first run):**
    /// - A and B give the **same leaves in the same order**: B refines the
    ///   leaves its triangles cross (`intersectsTriangleExact` over the
    ///   leaves in each triangle's box), A the leaves holding them (the
    ///   same test, applied when the parent was split), both through
    ///   `refineSelectedBoxes` pass by pass;
    /// - the source's `cellSize` loop (`source_add_levels`) differs from
    ///   `boundaryCellSize`'s at exact powers of two: `cellSize` 0.25 gives
    ///   3 levels (triangle leaves at level 7), not 2. Upstream as written.
    ///
    /// **Result (2026-11-05): as predicted.** Both routes give the same
    /// 10 816 leaves in the same order (the gh:#1264 pin); the `cellSize`
    /// 0.25 source puts triangle leaves at level 7 (40 944 leaves).
    #[test]
    fn surface_source_matches_boundary_refinement_on_the_same_triangles() {
        let a = create(sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) }).0;
        let src = |cell_size: Option<f64>, levels: Option<u8>| MeshDict {
            surface_mesh_refinement: vec![SurfaceMeshRefinement {
                name: "self".into(),
                surface: sphere3(),
                cell_size,
                additional_levels: levels,
                refinement_thickness: 0.0,
            }],
            ..MeshDict::with_max_cell_size(1.0)
        };
        let b = create(sphere3(), src(None, Some(2))).0;
        println!("boundaryCellSize route {} leaves, surface-source route {} leaves", a.number_of_leaves(), b.number_of_leaves());
        assert_eq!(leaf_coords(&a), leaf_coords(&b));
        assert_eq!(source_add_levels(1.0, 0.25), 3);
        assert_eq!(source_add_levels(1.0, 0.3), 2);
        assert_eq!(levels_to(1.0, 0.3), 2);
        let c = create(sphere3(), src(Some(0.25), None)).0;
        let max_data = (0..c.number_of_leaves()).filter(|&l| c.has_contained_triangles(l)).map(|l| c.return_leaf(l).coords.level).max();
        println!("cellSize 0.25 source: triangle leaves up to level {max_data:?}, {} leaves", c.number_of_leaves());
        assert_eq!(max_data, Some(7));
    }

    /// V&V (gh:#1264/#1265 remainders): `refineBoxesIntersectingEdgeMeshes`
    /// and `refinementThickness`.
    ///
    /// **Predictions (written before the first run):** cube `[0, 8]^3`,
    /// `maxCellSize` 1 (global level 4, root side 16), one edge-mesh segment
    /// from (1.3, 2.1, 4.1) to (6.7, 5.9, 4.1) with 2 additional levels:
    /// every leaf the segment crosses (`intersectsLine`) is at level >= 6,
    /// and without the source no leaf off the boundary is finer than 4.
    /// With `refinementThickness` 1.0 more leaves are refined than without.
    ///
    /// **Result (2026-11-05): as predicted.** Leaves: 2 024 without the
    /// source, 2 892 with it, 9 220 with the 1.0 thickness; the 38 leaves
    /// the segment crosses are all at level 6.
    #[test]
    fn edge_mesh_source_refines_the_leaves_it_crosses() {
        let (p, t) = crate::shapes::box_surface(Vec3::ZERO, Vec3::new(8.0, 8.0, 8.0));
        let cube = Arc::new(TriSurf::new(p, t, None));
        let (s, e) = (Vec3::new(1.3, 2.1, 4.1), Vec3::new(6.7, 5.9, 4.1));
        let dict = |th: f64| MeshDict {
            edge_mesh_refinement: vec![EdgeMeshRefinement {
                name: "line".into(),
                edge_mesh: Arc::new(EdgeMesh { points: vec![s, e], edges: vec![[0, 1]] }),
                cell_size: None,
                additional_levels: Some(2),
                refinement_thickness: th,
            }],
            ..MeshDict::with_max_cell_size(1.0)
        };
        let plain = create(Arc::clone(&cube), MeshDict::with_max_cell_size(1.0)).0;
        let thin = create(Arc::clone(&cube), dict(0.0)).0;
        let thick = create(Arc::clone(&cube), dict(1.0)).0;
        let crossed: Vec<u8> = (0..thin.number_of_leaves())
            .filter(|&l| thin.return_leaf(l).coords.intersects_line(&thin.root_box, s, e))
            .map(|l| thin.return_leaf(l).coords.level)
            .collect();
        println!(
            "edge source: leaves plain {}, source {}, source + thickness 1.0 {}; crossed leaves {} at levels {:?}..{:?}",
            plain.number_of_leaves(),
            thin.number_of_leaves(),
            thick.number_of_leaves(),
            crossed.len(),
            crossed.iter().min(),
            crossed.iter().max()
        );
        assert!(!crossed.is_empty() && crossed.iter().all(|&lv| lv >= 6));
        assert!((0..plain.number_of_leaves()).all(|l| plain.has_contained_triangles(l) || plain.return_leaf(l).coords.level <= 4));
        assert!(thick.number_of_leaves() > thin.number_of_leaves());
    }

    /// gh:#1264/#1265 remainders: `anisotropicSources` on the refinement
    /// sources. **Prediction:** without it the sources are passed through
    /// (the same `Arc`); with a box scaling the source surface and edge mesh
    /// are exactly `anisotropic::modify_surface` / `modify_points` of the
    /// originals, and the segment moves (its first point, (1.3, 2.1, 4.1),
    /// is outside the box, the second too, so only points inside
    /// `[3, 5]^3` would move by the box alone: recorded, not predicted).
    ///
    /// **Result (2026-11-05):** pass-through and equality held. The
    /// segment's end moved from x 6.7 to 5.7 although it is outside the
    /// box: gh:#1279's box scaling displaces every point beyond the box by
    /// the box's whole stretch (its semantics, not this hook's).
    #[test]
    fn anisotropic_sources_move_the_refinement_sources() {
        use crate::anisotropic::{BoxScaling, CoordinateModifier, Modification};
        let surf = sphere3();
        let em = Arc::new(EdgeMesh { points: vec![Vec3::new(1.3, 2.1, 4.1), Vec3::new(6.7, 5.9, 4.1)], edges: vec![[0, 1]] });
        let plain = MeshDict::with_max_cell_size(1.0);
        assert!(Arc::ptr_eq(&source_surface_geometry(&plain, &surf), &surf));
        assert!(Arc::ptr_eq(&source_edge_mesh_geometry(&plain, &em), &em));
        let cm = CoordinateModifier::new(vec![Modification::Box(BoxScaling {
            name: "b".into(),
            centre: Vec3::new(4.0, 4.0, 4.0),
            length: Vec3::new(2.0, 2.0, 2.0),
            scale: Vec3::new(2.0, 1.0, 1.0),
        })])
        .expect("modifier");
        let d = MeshDict { anisotropic_sources: Some(cm.clone()), ..plain };
        assert_eq!(*source_surface_geometry(&d, &surf), crate::anisotropic::modify_surface(&surf, &cm, true));
        let mut pts = em.points.clone();
        crate::anisotropic::modify_points(&mut pts, &cm);
        let moved = source_edge_mesh_geometry(&d, &em);
        println!("edge source under the box scaling: {:?} -> {:?}", em.points, moved.points);
        assert_eq!(moved.points, pts);
        assert_ne!(moved.points, em.points);
    }

    /// V&V (gh:#1264 remainder), differential: `subsetCellSize` and a
    /// subset `localRefinement` entry against `patchCellSize` and a patch
    /// `localRefinement` entry on the **same facets**.
    ///
    /// **Prediction (written before the first run):** the pipe's inlet
    /// (region 1) and a facet subset `inlet_set` holding exactly the
    /// region-1 facets give identical octrees, leaf for leaf, for a cell
    /// size of 0.5 mm, and for 1 additional level with a 1 mm thickness.
    /// An unknown subset name refines nothing (upstream warns).
    ///
    /// **Result (2026-11-05): as predicted.** 20 952 leaves plain; 27 672 by
    /// `patchCellSize` and by `subsetCellSize`, leaf for leaf; 23 864 by
    /// either `localRefinement` form; the unknown subset leaves the plain
    /// octree.
    #[test]
    fn subset_cell_size_matches_patch_cell_size_on_the_same_facets() {
        let pipe = crate::mesh_octree_queries::tests::pipe(0.01, 0.06, 48);
        let mut s = (*pipe).clone();
        let id = s.facet_subsets.add("inlet_set");
        for t in 0..s.size() {
            if s.regions[t] == 1 {
                s.facet_subsets.add_element(id, t);
            }
        }
        let s = Arc::new(s);
        let base = MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) };
        let by_patch = create(Arc::clone(&s), MeshDict { patch_cell_size: vec![PatchRefinement { region: 1, cell_size: 0.0005 }], ..base.clone() }).0;
        let by_subset = create(Arc::clone(&s), MeshDict { subset_cell_size: vec![SubsetRefinement { subset: "inlet_set".into(), cell_size: 0.0005 }], ..base.clone() }).0;
        let lr_patch = create(
            Arc::clone(&s),
            MeshDict { local_refinement: vec![LocalRefinement { region: 1, additional_levels: Some(1), cell_size: None, refinement_thickness: 0.001 }], ..base.clone() },
        )
        .0;
        let lr_subset = create(
            Arc::clone(&s),
            MeshDict {
                local_refinement_subsets: vec![SubsetLocalRefinement { subset: "inlet_set".into(), additional_levels: Some(1), cell_size: None, refinement_thickness: 0.001 }],
                ..base.clone()
            },
        )
        .0;
        let plain = create(Arc::clone(&s), base.clone()).0;
        let unknown = create(Arc::clone(&s), MeshDict { subset_cell_size: vec![SubsetRefinement { subset: "nope".into(), cell_size: 0.0005 }], ..base }).0;
        println!(
            "pipe leaves: plain {}, patchCellSize {}, subsetCellSize {}, localRefinement patch {} / subset {}",
            plain.number_of_leaves(),
            by_patch.number_of_leaves(),
            by_subset.number_of_leaves(),
            lr_patch.number_of_leaves(),
            lr_subset.number_of_leaves()
        );
        assert!(by_patch.number_of_leaves() > plain.number_of_leaves());
        assert_eq!(leaf_coords(&by_patch), leaf_coords(&by_subset));
        assert_eq!(leaf_coords(&lr_patch), leaf_coords(&lr_subset));
        assert_eq!(leaf_coords(&plain), leaf_coords(&unknown));
    }
}
