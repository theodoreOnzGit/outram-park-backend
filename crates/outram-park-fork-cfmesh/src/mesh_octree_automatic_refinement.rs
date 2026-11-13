// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeAutomaticRefinement/
//     meshOctreeAutomaticRefinement.{C,H}, meshOctreeAutomaticRefinementRef.C
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (numberOfFaceGroups, numberOfEdgeGroups)
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

//! `meshOctreeAutomaticRefinement` (gh:#1278, second half): refine the
//! octree's surface leaves that are too coarse for the surface's
//! **curvature** or for the **proximity** of other parts of the surface.
//!
//! Upstream runs it inside `createOctreeBoxes`, after the boundary and the
//! refinement sources and before inside/outside marking, and only when the
//! `meshDict` sets `minCellSize` ([`crate::mesh_octree_creator::MeshDict::min_cell_size`]);
//! without it `maxRefLevel_` stays 0 and nothing happens. Every refinement
//! stops at `maxRefLevel_`, the first level whose cubes are smaller than
//! `minCellSize` (with one exception, below).
//!
//! # The criteria, in upstream's order
//!
//! 1. **Curvature** (`refineBasedOnCurvature`): a leaf holding triangles is
//!    refined while its size exceeds `0.2835 / κ`, `κ` the largest
//!    `|meanCurvatureAtTriangle|` of its triangles and
//!    `|curvatureAtEdge|` of its feature edges
//!    ([`crate::surface::curvature::TriSurfaceCurvatureEstimator`]).
//! 2. **Contained corners** (`refineBasedOnContainedCorners`): a leaf with
//!    two surface corners ([`crate::surface::features::TriSurfacePartitioner`]),
//!    or a corner leaf with another corner leaf's corner within
//!    `1.732 x` its size of its centre.
//! 3. **Contained partitions** (`refineBasedOnContainedPartitions`): within
//!    `1.733 x` the leaf's size, two patches that do not border each other,
//!    or two feature-edge groups that do not meet at a corner.
//! 4. **Proximity** (`refineBasedOnProximityTests`): within `1.732 x` the
//!    leaf's size, the triangles (or feature edges) form more than one
//!    connected group ([`number_of_face_groups`], [`number_of_edge_groups`]):
//!    two parts of the surface closer than that are resolved by refining.
//!
//! Each criterion loops: mark, add one layer of neighbours
//! (`markAdditionalLayers(refineBox, 1)`), refine 1-irregularly
//! (`refineSelectedBoxes`), and re-test only the children of what was
//! refined, until nothing is marked.
//!
//! # Upstream defect kept
//!
//! `refineBasedOnContainedCorners` first marks every leaf holding two
//! corners **without** the `maxRefLevel_` test the rest of the routine
//! applies, so two corners closer than `minCellSize` are separated below it
//! (and two coincident corners would never terminate). Transcribed as
//! written; the test surfaces have no such corners.
//!
//! # Not ported
//!
//! `octreeAddressing()` (constructed, never used by these routines),
//! the OpenMP and MPI branches (`loadDistribution`,
//! `exchangeRequestsWithNeighbourProcessors`).

use crate::math::Vec3;
use crate::mesh_octree::help::{nearest_point_on_the_edge, nearest_point_on_the_edge_exact, nearest_point_on_the_triangle};
use crate::mesh_octree::{BoundBox, CubeCoordinates, MeshOctree, TriSurf, SMALL};
use crate::surface::curvature::TriSurfaceCurvatureEstimator;
use crate::surface::features::TriSurfacePartitioner;
use std::collections::{BTreeMap, BTreeSet};

/// How many boxes each criterion marked, summed over its passes, and the
/// number of passes (`refineSelectedBoxes` calls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AutomaticRefinementReport {
    /// `maxRefLevel_`.
    pub max_ref_level: u8,
    /// Boxes marked by the curvature criterion.
    pub n_curvature: usize,
    /// Boxes marked by the corner criterion.
    pub n_corners: usize,
    /// Boxes marked by the partition criterion.
    pub n_partitions: usize,
    /// Boxes marked by the proximity criterion.
    pub n_proximity: usize,
    /// Refinement passes.
    pub n_passes: usize,
}

/// `meshOctreeAutomaticRefinement`.
#[derive(Debug, Clone)]
pub struct MeshOctreeAutomaticRefinement {
    /// `useDATABoxes_` (only read by the addressing upstream constructs and
    /// never uses here; kept for the constructor's semantics).
    pub use_data_boxes: bool,
    /// `hexRefinement_`.
    pub hex_refinement: bool,
    /// `maxRefLevel_`.
    pub max_ref_level: u8,
    partitioner: Option<TriSurfacePartitioner>,
    curvature: Option<TriSurfaceCurvatureEstimator>,
    report: AutomaticRefinementReport,
}

impl MeshOctreeAutomaticRefinement {
    /// `meshOctreeAutomaticRefinement(octree, dict, useDATABoxes)` with the
    /// dictionary entries it reads: `minCellSize`, `maxCellSize` and
    /// `keepCellsIntersectingBoundary`. `setMaxRefLevel` runs here: the
    /// first level of the octree's (already sized) root box whose cubes are
    /// below `min_cell_size x (1 + SMALL)`; `None`, or a `minCellSize` above
    /// `maxCellSize`, leaves it 0 (automatic refinement off).
    pub fn new(o: &MeshOctree, min_cell_size: Option<f64>, max_cell_size: f64, keep_cells_intersecting_boundary: bool, use_data_boxes: bool) -> Self {
        let mut a = MeshOctreeAutomaticRefinement {
            use_data_boxes: use_data_boxes || keep_cells_intersecting_boundary,
            hex_refinement: false,
            max_ref_level: 0,
            partitioner: None,
            curvature: None,
            report: AutomaticRefinementReport::default(),
        };
        if let Some(min) = min_cell_size {
            let size = o.root_box.max.x - o.root_box.min.x;
            let cs = min * (1.0 + SMALL);
            if cs <= max_cell_size {
                loop {
                    let l_size = size / (1u64 << a.max_ref_level) as f64;
                    if l_size < cs {
                        break;
                    }
                    a.max_ref_level += 1;
                }
                a.use_data_boxes = true;
            }
        }
        a.report.max_ref_level = a.max_ref_level;
        a
    }

    /// `activateHexRefinement`.
    pub fn activate_hex_refinement(&mut self) {
        self.hex_refinement = true;
    }

    /// What has been marked so far.
    pub fn report(&self) -> AutomaticRefinementReport {
        self.report
    }

    fn partitioner(&mut self, surf: &TriSurf) -> &TriSurfacePartitioner {
        self.partitioner.get_or_insert_with(|| TriSurfacePartitioner::new(surf))
    }

    fn curvature(&mut self, surf: &TriSurf) -> &TriSurfaceCurvatureEstimator {
        self.curvature.get_or_insert_with(|| TriSurfaceCurvatureEstimator::new(surf))
    }

    /// `automaticRefinement`: curvature, then proximity; nothing when
    /// `maxRefLevel_` is 0.
    pub fn automatic_refinement(&mut self, o: &mut MeshOctree) {
        if self.max_ref_level == 0 {
            return;
        }
        self.curvature_refinement(o);
        self.proximity_refinement(o);
    }

    fn all_leaves(o: &MeshOctree) -> (Vec<i32>, Vec<usize>) {
        let n = o.number_of_leaves();
        (vec![0; n], (0..n).collect())
    }

    /// `curvatureRefinement`.
    pub fn curvature_refinement(&mut self, o: &mut MeshOctree) -> bool {
        let (mut refine_box, mut cands) = Self::all_leaves(o);
        while self.refine_based_on_curvature(o, &mut refine_box, &cands) {
            self.refine_selected_boxes(o, &mut refine_box, &mut cands);
        }
        false
    }

    /// `proximityRefinement`: corners, partitions, proximity, each to
    /// convergence over all leaves.
    pub fn proximity_refinement(&mut self, o: &mut MeshOctree) -> bool {
        let mut refine = false;
        let (mut refine_box, mut cands) = Self::all_leaves(o);
        while self.refine_based_on_contained_corners(o, &mut refine_box, &cands) {
            self.refine_selected_boxes(o, &mut refine_box, &mut cands);
            refine = true;
        }
        cands = (0..refine_box.len()).collect();
        while self.refine_based_on_contained_partitions(o, &mut refine_box, &cands) {
            self.refine_selected_boxes(o, &mut refine_box, &mut cands);
            refine = true;
        }
        cands = (0..refine_box.len()).collect();
        while self.refine_based_on_proximity_tests(o, &mut refine_box, &cands) {
            self.refine_selected_boxes(o, &mut refine_box, &mut cands);
            refine = true;
        }
        refine
    }

    /// `refineBasedOnContainedCorners` (module docs, including the defect).
    pub fn refine_based_on_contained_corners(&mut self, o: &MeshOctree, refine_box: &mut [i32], cands: &[usize]) -> bool {
        let surf = std::sync::Arc::clone(&o.surface);
        let corners = self.partitioner(&surf).corners.clone();
        let mut corner_in_leaf: Vec<Option<usize>> = vec![None; refine_box.len()];
        let mut n_marked = 0;
        for &c in &corners {
            let Some(l) = o.find_leaf_containing_vertex(surf.points[c]) else {
                continue;
            };
            if corner_in_leaf[l].is_some() {
                n_marked += 1;
                refine_box[l] = 1;
            } else {
                corner_in_leaf[l] = Some(c);
            }
        }
        for &leaf in cands {
            let oc = o.return_leaf(leaf);
            if oc.coords.level >= self.max_ref_level || corner_in_leaf[leaf].is_none() {
                continue;
            }
            let c = oc.coords.centre(&o.root_box);
            let r = 1.732 * oc.coords.size(&o.root_box);
            let h = Vec3::new(r, r, r);
            for nei in o.find_leaves_contained_in_box(&BoundBox::new(c.sub(h), c.add(h))) {
                if nei == leaf {
                    continue;
                }
                let Some(cn) = corner_in_leaf[nei] else {
                    continue;
                };
                if surf.points[cn].sub(c).length() < r {
                    n_marked += 1;
                    refine_box[nei] = 1;
                    refine_box[leaf] = 1;
                    break;
                }
            }
        }
        self.report.n_corners += n_marked;
        if std::env::var("GH1385_TRACE").is_ok() {
            eprintln!("AUTOREF corner {n_marked}");
        }
        n_marked != 0
    }

    /// `refineBasedOnContainedPartitions` (module docs).
    pub fn refine_based_on_contained_partitions(&mut self, o: &MeshOctree, refine_box: &mut [i32], cands: &[usize]) -> bool {
        let surf = std::sync::Arc::clone(&o.surface);
        let max_ref_level = self.max_ref_level;
        let part = self.partitioner(&surf);
        let mut n_marked = 0;
        for &leaf in cands {
            let oc = o.return_leaf(leaf);
            if !oc.has_contained_elements() || oc.coords.level >= max_ref_level {
                continue;
            }
            let c = oc.coords.centre(&o.root_box);
            let s = 1.733 * oc.coords.size(&o.root_box);
            let h = Vec3::new(s, s, s);
            let bb = BoundBox::new(c.sub(h), c.add(h));
            let mut patches: Vec<usize> = Vec::new();
            for t in o.find_triangles_in_box(&bb) {
                if !patches.contains(&surf.regions[t]) {
                    patches.push(surf.regions[t]);
                }
            }
            let mut groups: Vec<usize> = Vec::new();
            for e in o.find_edges_in_box(&bb) {
                // `findEdgesInBox` returns patch-boundary edges only, which
                // all have a group.
                if let Some(g) = part.edge_groups[e] {
                    if !groups.contains(&g) {
                        groups.push(g);
                    }
                }
            }
            let mut refine = false;
            for i in 0..patches.len() {
                for j in i + 1..patches.len() {
                    if !part.patch_patches[patches[i]].contains(&patches[j]) {
                        refine = true;
                        break;
                    }
                }
            }
            for i in 0..groups.len() {
                for j in i + 1..groups.len() {
                    if !part.edge_group_edge_groups[groups[i]].contains(&groups[j]) {
                        refine = true;
                        break;
                    }
                }
            }
            if refine {
                n_marked += 1;
                refine_box[leaf] = 1;
            }
        }
        self.report.n_partitions += n_marked;
        if std::env::var("GH1385_TRACE").is_ok() {
            eprintln!("AUTOREF partition {n_marked}");
        }
        n_marked != 0
    }

    /// `refineBasedOnCurvature` (module docs).
    pub fn refine_based_on_curvature(&mut self, o: &MeshOctree, refine_box: &mut [i32], cands: &[usize]) -> bool {
        let surf = std::sync::Arc::clone(&o.surface);
        let max_ref_level = self.max_ref_level;
        let curv = self.curvature(&surf);
        let mut n_marked = 0;
        for &leaf in cands {
            if !o.has_contained_triangles(leaf) {
                continue;
            }
            let oc = o.return_leaf(leaf);
            if oc.coords.level >= max_ref_level {
                continue;
            }
            let max_curv = max_curvature_in_leaf(o, curv, leaf);
            if oc.coords.size(&o.root_box) > 0.2835 / (max_curv + SMALL) {
                refine_box[leaf] = 1;
                n_marked += 1;
            }
        }
        self.report.n_curvature += n_marked;
        if std::env::var("GH1385_TRACE").is_ok() {
            eprintln!("AUTOREF curvature {n_marked}");
        }
        n_marked != 0
    }

    /// `refineBasedOnProximityTests` (module docs).
    pub fn refine_based_on_proximity_tests(&mut self, o: &MeshOctree, refine_box: &mut [i32], cands: &[usize]) -> bool {
        let mut n_marked = 0;
        for &leaf in cands {
            if !o.has_contained_triangles(leaf) {
                continue;
            }
            let oc = o.return_leaf(leaf);
            if oc.coords.level >= self.max_ref_level {
                continue;
            }
            let (n_faces, n_edges) = proximity_groups(o, leaf);
            if n_faces > 1 || n_edges > 1 {
                n_marked += 1;
                refine_box[leaf] = 1;
            }
        }
        self.report.n_proximity += n_marked;
        if std::env::var("GH1385_TRACE").is_ok() {
            eprintln!("AUTOREF proximity {n_marked}");
        }
        n_marked != 0
    }

    /// `refineSelectedBoxes`: one layer of neighbours, refine (1-irregular),
    /// and the children of every refined box become the next candidates.
    pub fn refine_selected_boxes(&mut self, o: &mut MeshOctree, refine_box: &mut Vec<i32>, cands: &mut Vec<usize>) {
        o.mark_additional_layers(refine_box, 1);
        let before: Vec<CubeCoordinates> = (0..o.number_of_leaves()).map(|l| o.return_leaf(l).coords).collect();
        o.refine_selected_boxes(refine_box, self.hex_refinement);
        cands.clear();
        for (l, cc) in before.iter().enumerate() {
            if refine_box[l] == 0 {
                continue;
            }
            for sc in 0..8 {
                let Some(ci) = o.find_cube_for_position(&cc.refine_for_position(sc)) else {
                    continue;
                };
                let cube = o.cube(ci);
                if !cube.is_leaf() {
                    continue;
                }
                if let Some(label) = cube.cube_label {
                    cands.push(label);
                }
            }
        }
        *refine_box = vec![0; o.number_of_leaves()];
        self.report.n_passes += 1;
    }
}

/// The largest `|meanCurvatureAtTriangle|` over leaf `leaf`'s triangles and
/// `|curvatureAtEdge|` over its feature edges (`refineBasedOnCurvature`).
pub fn max_curvature_in_leaf(o: &MeshOctree, curv: &TriSurfaceCurvatureEstimator, leaf: usize) -> f64 {
    let surf = &o.surface;
    let mut max_curv = 0.0f64;
    for t in o.contained_triangles(leaf) {
        max_curv = max_curv.max(curv.mean_curvature_at_triangle(surf, t).abs());
    }
    if o.has_contained_edges(leaf) {
        for e in o.contained_edges(leaf) {
            max_curv = max_curv.max(curv.curvature_at_edge(surf, e).abs());
        }
    }
    max_curv
}

/// The two group counts `refineBasedOnProximityTests` tests for leaf
/// `leaf`: (face groups, edge groups) within `1.732 x` its size of its
/// centre.
pub fn proximity_groups(o: &MeshOctree, leaf: usize) -> (usize, usize) {
    let oc = o.return_leaf(leaf);
    let c = oc.coords.centre(&o.root_box);
    let s = 1.732 * oc.coords.size(&o.root_box);
    let h = Vec3::new(s, s, s);
    let bb = BoundBox::new(c.sub(h), c.add(h));
    let tria: BTreeSet<usize> = o.find_triangles_in_box(&bb).into_iter().collect();
    let edges: BTreeSet<usize> = o.find_edges_in_box(&bb).into_iter().collect();
    (number_of_face_groups(&tria, c, s, &o.surface), number_of_edge_groups(&edges, c, s, &o.surface))
}

/// `help::numberOfFaceGroups`: of `contained`, the triangles whose nearest
/// point is within `range` of `centre`, grouped over shared edges whose
/// (infinite-line) nearest point is within `range` too. Returns the number
/// of groups. (The count does not depend on the visiting order, so
/// upstream's hash order and the ascending order here agree.)
pub fn number_of_face_groups(contained: &BTreeSet<usize>, centre: Vec3, range: f64, surf: &TriSurf) -> usize {
    let p = &surf.points;
    let range_sq = range * range;
    let in_range: BTreeSet<usize> = contained
        .iter()
        .copied()
        .filter(|&t| {
            let tr = surf.triangles[t];
            let np = nearest_point_on_the_triangle(p[tr[0]], p[tr[1]], p[tr[2]], centre);
            crate::mesh_octree::mag_sqr(np.sub(centre)) < range_sq
        })
        .collect();
    let mut group: BTreeMap<usize, usize> = BTreeMap::new();
    let mut test_edge: BTreeMap<usize, bool> = BTreeMap::new();
    let mut n_groups = 0;
    for &t in &in_range {
        if group.contains_key(&t) {
            continue;
        }
        group.insert(t, n_groups);
        let mut front = vec![t];
        while let Some(f) = front.pop() {
            for &e in &surf.facet_edges[f] {
                let ok = *test_edge.entry(e).or_insert_with(|| {
                    let [a, b] = surf.edges[e];
                    let np = nearest_point_on_the_edge(p[a], p[b], centre);
                    crate::mesh_octree::mag_sqr(np.sub(centre)) < range_sq
                });
                if !ok {
                    continue;
                }
                for &nei in &surf.edge_facets[e] {
                    if in_range.contains(&nei) && !group.contains_key(&nei) {
                        group.insert(nei, n_groups);
                        front.push(nei);
                    }
                }
            }
        }
        n_groups += 1;
    }
    n_groups
}

/// `help::numberOfEdgeGroups`: of `contained`, the edges whose (segment)
/// nearest point is within `range` of `centre`, grouped over shared points
/// within `range`. Returns the number of groups.
pub fn number_of_edge_groups(contained: &BTreeSet<usize>, centre: Vec3, range: f64, surf: &TriSurf) -> usize {
    let p = &surf.points;
    let range_sq = range * range;
    let in_range: BTreeSet<usize> = contained
        .iter()
        .copied()
        .filter(|&e| {
            let [a, b] = surf.edges[e];
            let np = nearest_point_on_the_edge_exact(p[a], p[b], centre);
            crate::mesh_octree::mag_sqr(np.sub(centre)) < range_sq
        })
        .collect();
    let mut group: BTreeMap<usize, usize> = BTreeMap::new();
    let mut point_test: BTreeMap<usize, bool> = BTreeMap::new();
    let mut n_groups = 0;
    for &e0 in &in_range {
        if group.contains_key(&e0) {
            continue;
        }
        group.insert(e0, n_groups);
        let mut front = vec![e0];
        while let Some(e) = front.pop() {
            for &pt in &surf.edges[e] {
                let ok = *point_test.entry(pt).or_insert_with(|| crate::mesh_octree::mag_sqr(p[pt].sub(centre)) < range_sq);
                if !ok {
                    continue;
                }
                for &nei in &surf.point_edges[pt] {
                    if in_range.contains(&nei) && !group.contains_key(&nei) {
                        group.insert(nei, n_groups);
                        front.push(nei);
                    }
                }
            }
        }
        n_groups += 1;
    }
    n_groups
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mesh_octree_creator::tests::sphere3;
    use crate::mesh_octree_creator::{MeshDict, MeshOctreeCreator};
    use crate::shapes::box_surface_subdivided;
    use std::sync::Arc;

    /// The thin slab `[0, 1] x [0, 1] x [0, g]`: two parallel plates `g`
    /// apart, joined by narrow side walls, triangles of side 0.02, one
    /// patch.
    pub(crate) fn slab(g: f64) -> Arc<TriSurf> {
        let (p, t, _) = box_surface_subdivided(Vec3::ZERO, Vec3::new(1.0, 1.0, g), 0.02);
        Arc::new(TriSurf::new(p, t, None))
    }

    /// Size the root and refine to the boundary (the steps of
    /// `createOctreeBoxes` before automatic refinement).
    pub(crate) fn boundary_octree(surf: Arc<TriSurf>, dict: &MeshDict) -> (MeshOctree, MeshOctreeCreator) {
        let mut o = MeshOctree::new(surf);
        let mut c = MeshOctreeCreator::new(&o, Some(dict.clone()));
        c.set_root_cube_size_and_ref_parameters(&mut o);
        c.refine_boundary(&mut o);
        (o, c)
    }

    /// Levels of the leaves holding triangles, as (level, count).
    pub(crate) fn data_levels(o: &MeshOctree) -> BTreeMap<u8, usize> {
        let mut m = BTreeMap::new();
        for l in 0..o.number_of_leaves() {
            if o.has_contained_triangles(l) {
                *m.entry(o.return_leaf(l).coords.level).or_insert(0) += 1;
            }
        }
        m
    }

    /// V&V (gh:#1278, second half): **proximity refinement** on two parallel
    /// plates closer than the cell size.
    ///
    /// # Methodology
    ///
    /// The slab `[0, 1] x [0, 1] x [0, 0.1]` (plates at `z = 0` and
    /// `z = 0.1`, gap `g = 0.1`; triangles of side 0.02; one patch, so no
    /// corners, partitions or feature edges), `maxCellSize` 0.25 (2.5 x the
    /// gap), `minCellSize` 0.01. The octree is sized and refined to the
    /// boundary, then only `proximityRefinement` runs (curvature is tested
    /// below; on this box it would refine the rims). Upstream's criterion:
    /// a leaf holding triangles is refined while the triangles within
    /// `s = 1.732 h` of its centre form more than one connected group.
    ///
    /// **Predictions (written before the first run), derived from the
    /// criterion and the lattice:** the root is sized from `1.5 x 1 + 0.125`
    /// to side 2.0 (global level 3, `h` 0.25), centred at `z = 0.05`; level
    /// planes sit at `z = 0.05 + k h`. `maxRefLevel_` = 8 (2 / 2^8 < 0.01).
    /// - A plate leaf away from the rims has its centre `d` from its plate
    ///   and `g + d` or `g - d` from the other. Level 3: other plate 0.175
    ///   from the centre, `s` 0.433, refined. Level 4: 0.1125 < 0.217,
    ///   refined. Level 5: 0.08125 < 0.108, refined. Level 6 (`h`
    ///   0.03125): 0.096875 > 0.0541, **stops**. So every plate leaf whose
    ///   centre is more than 0.15 from the side walls ends at **level 6**,
    ///   none finer: the gap then holds `g / h` = 3.2 leaves.
    /// - The stopping rule holds on exit: every leaf holding triangles below
    ///   level 8 has at most one face group within `s`.
    /// - The upstream bound in general: the criterion stops once the other
    ///   plate is beyond `1.732 h` of a centre at most `h / 2` from a plate
    ///   (axis-aligned plates), i.e. it guarantees `g >= 1.232 h`: at least
    ///   1.232 leaves across any gap it can see.
    /// - Ablation: without automatic refinement the plate leaves stay at
    ///   level 3 (`h` 0.25, 0.4 leaves across the gap).
    ///
    /// # Results (2026-11-05)
    ///
    /// **First run: the level-6 prediction missed; nothing was refined**
    /// (plate leaves stayed at level 3, `n_proximity` 0). The prediction
    /// ignored the side walls: the plate is 1 m wide and at level 3 every
    /// leaf centre lies within 0.375 of a side wall, inside `s` = 0.433, so
    /// the wall joins the two plates into **one** group within range and
    /// upstream's criterion sees one part of the surface. Refinement only
    /// starts where a leaf's range misses the walls, and the candidates of
    /// later passes are only the children of refined leaves, so nothing
    /// ever starts. This is upstream's criterion as written (pinned below as
    /// the first case). Ablation and stopping rule held.
    ///
    /// **Second case (predictions written after the miss, before its run):**
    /// the same plate width with `maxCellSize` 0.125 (`W / 8`) and gap
    /// 0.05: root side 2.0 (`1.5625 / 2^4 < 0.125`, global level 4),
    /// centred at `z = 0.025`, `maxRefLevel_` 9 (`minCellSize` 0.005).
    /// Interior level-4 centres are 0.4375 from the walls, beyond `s`
    /// 0.217. Per level, the other plate from a plate leaf's centre: level
    /// 4, 0.0875 < 0.217; level 5, 0.05625 < 0.108; level 6, 0.040625 <
    /// 0.0541; level 7 (`h` 0.015625), 0.0484 > 0.0271, **stops**. So the
    /// plate leaves with centres in `[0.3, 0.7]^2` end at **level 7**, the
    /// gap holds 3.2 leaves, and the stopping rule holds everywhere below
    /// level 9.
    ///
    /// **Second case result: as predicted.** Plate leaves before: 200 at
    /// level 4; after: 264 at 6, 8 320 at 7, 1 504 at 8; the 1 352 interior
    /// ones all at level 7, gap / h = 3.2; 2 224 boxes marked in 4 passes;
    /// stopping rule held for every leaf. The 1 504 level-8 leaves lie
    /// outside the interior square (not predicted; where exactly and why is
    /// in the images, `slab_xz_level.png`, and was not investigated
    /// further).
    #[test]
    fn proximity_refines_two_close_plates_until_the_gap_is_resolved() {
        // (gap, maxCellSize, minCellSize, root side, global level, maxRefLevel, interior levels expected)
        let cases: [(f64, f64, f64, f64, u8, u8, Vec<u8>); 2] =
            [(0.1, 0.25, 0.01, 2.0, 3, 8, vec![3]), (0.05, 0.125, 0.005, 2.0, 4, 9, vec![7])];
        for (g, max, min, side, global, max_ref, expect) in cases {
            let dict = MeshDict { min_cell_size: Some(min), ..MeshDict::with_max_cell_size(max) };
            let (mut o, _) = boundary_octree(slab(g), &dict);
            assert_eq!(o.root_box.span().x, side);
            let before = data_levels(&o);
            let mut a = MeshOctreeAutomaticRefinement::new(&o, dict.min_cell_size, dict.max_cell_size, false, false);
            assert_eq!(a.max_ref_level, max_ref);
            a.proximity_refinement(&mut o);
            let after = data_levels(&o);
            let r = a.report();
            println!("slab g {g}, maxCellSize {max}: DATA levels before {before:?} after {after:?}; report {r:?}");
            assert_eq!(before.keys().copied().collect::<Vec<_>>(), vec![global], "ablation: plate leaves at the global level");
            let mut interior: BTreeMap<u8, usize> = BTreeMap::new();
            let mut h_interior = f64::MAX;
            for l in 0..o.number_of_leaves() {
                if !o.has_contained_triangles(l) {
                    continue;
                }
                let oc = o.return_leaf(l).coords;
                let c = oc.centre(&o.root_box);
                if c.x > 0.3 && c.x < 0.7 && c.y > 0.3 && c.y < 0.7 {
                    *interior.entry(oc.level).or_insert(0) += 1;
                    h_interior = h_interior.min(oc.size(&o.root_box));
                }
                if oc.level < a.max_ref_level {
                    let (nf, ne) = proximity_groups(&o, l);
                    assert!(nf <= 1 && ne <= 1, "leaf {l}: stopping rule ({nf}, {ne})");
                }
            }
            println!("  interior plate leaves by level: {interior:?}; gap / h = {}", g / h_interior);
            assert_eq!(interior.keys().copied().collect::<Vec<_>>(), expect);
            assert_eq!(r.n_corners + r.n_partitions, 0);
            if global == 3 {
                assert_eq!(r.n_proximity, 0, "first case: the walls join the plates within range");
            } else {
                assert!(g / h_interior >= 1.232);
                assert!(r.n_proximity > 0);
            }
        }
    }

    /// V&V (gh:#1278, second half): **curvature refinement** on a sphere.
    ///
    /// # Methodology
    ///
    /// The sphere R 3 m (24 x 48, one patch), `maxCellSize` 1.0,
    /// `minCellSize` 0.1. Sized and refined to the boundary, then only
    /// `curvatureRefinement` runs. Upstream's criterion: a leaf holding
    /// triangles is refined while its size exceeds `0.2835 / κ`, `κ` the
    /// largest `|mean curvature|` of its triangles
    /// (`triSurfaceCurvatureEstimator`, gh:#1278 first half; exact value
    /// `1 / R` = 0.333).
    ///
    /// **Predictions (written before the first run):** root side 16
    /// (`L` = 4, as gh:#1264 recorded), `maxRefLevel_` 8 (16 / 2^8 < 0.1).
    /// With `κ = 1/3` the threshold is 0.85: level 4 (`h` 1.0) is refined,
    /// level 5 (`h` 0.5) is not. This holds for any per-leaf estimate
    /// `κ` in (0.2835, 0.567], so it takes a 15 % low or a 70 % high
    /// estimate to move it. So: before, every triangle leaf at level 4;
    /// after, **every triangle leaf at level 5**; and on exit every
    /// triangle leaf below level 8 satisfies `h <= 0.2835 / (κ + SMALL)`.
    ///
    /// **Result (2026-11-05): as predicted.** 176 triangle leaves at level 4
    /// before, 680 at level 5 after, in one pass (176 marked); per-leaf `κ`
    /// 0.3343–0.3354 (+0.3 % to +0.6 % against `1 / R`); criterion held
    /// on every leaf.
    #[test]
    fn curvature_refines_a_sphere_to_the_curvature_criterion() {
        let dict = MeshDict { min_cell_size: Some(0.1), ..MeshDict::with_max_cell_size(1.0) };
        let (mut o, _) = boundary_octree(sphere3(), &dict);
        assert_eq!(o.root_box.span().x, 16.0);
        let before = data_levels(&o);
        let mut a = MeshOctreeAutomaticRefinement::new(&o, dict.min_cell_size, dict.max_cell_size, false, false);
        assert_eq!(a.max_ref_level, 8);
        a.curvature_refinement(&mut o);
        let after = data_levels(&o);
        let curv = TriSurfaceCurvatureEstimator::new(&o.surface);
        let (mut kmin, mut kmax) = (f64::MAX, 0.0f64);
        for l in 0..o.number_of_leaves() {
            if !o.has_contained_triangles(l) {
                continue;
            }
            let k = max_curvature_in_leaf(&o, &curv, l);
            kmin = kmin.min(k);
            kmax = kmax.max(k);
            let oc = o.return_leaf(l).coords;
            if oc.level < a.max_ref_level {
                assert!(oc.size(&o.root_box) <= 0.2835 / (k + SMALL), "leaf {l}: h {} κ {k}", oc.size(&o.root_box));
            }
        }
        println!("sphere R3: DATA levels before {before:?} after {after:?}; leaf κ {kmin:.4}..{kmax:.4}; report {:?}", a.report());
        assert_eq!(before.keys().copied().collect::<Vec<_>>(), vec![4]);
        assert_eq!(after.keys().copied().collect::<Vec<_>>(), vec![5]);
    }

    /// V&V (gh:#1278): automatic refinement through `createOctreeBoxes`.
    /// **Predictions:** with `minCellSize` the creator runs it and the
    /// sphere's triangle leaves end at level 5 (as above; proximity adds
    /// nothing on a sphere: one face group in range everywhere); without,
    /// `maxRefLevel_` is 0 and the report is empty. On the 3-patch pipe
    /// (corners at the rims) the corner and partition criteria are reached;
    /// counts recorded, not predicted.
    ///
    /// **Result (2026-11-05):** sphere as predicted (5 104 leaves, 680
    /// triangle leaves, all at level 5); without `minCellSize` the report is
    /// empty. Pipe (R 10 mm, `maxCellSize` 4 mm, `minCellSize` 0.5 mm,
    /// `maxRefLevel_` 8): 1 576 -> 11 824 leaves, triangle leaves at levels
    /// 6 (1 516), 7 (264), 8 (518) and **9 (64)**; curvature marked 229,
    /// proximity 4, corners 0, partitions 0, in 4 passes. The pipe has no
    /// corners (each rim point has two patch-boundary edges, a corner needs
    /// three), and no leaf sees two patches that do not border, so the
    /// corner and partition criteria were reached but marked nothing. The
    /// 64 leaves at level 9 are one past `maxRefLevel_`: upstream's
    /// `markAdditionalLayers(refineBox, 1)` and the 1-irregularity
    /// closure refine neighbours without the level test (as written).
    #[test]
    fn create_octree_boxes_runs_automatic_refinement_only_with_min_cell_size() {
        let on = MeshDict { min_cell_size: Some(0.1), ..MeshDict::with_max_cell_size(1.0) };
        let mut o = MeshOctree::new(sphere3());
        let mut c = MeshOctreeCreator::new(&o, Some(on));
        c.create_octree_boxes(&mut o);
        let lv = data_levels(&o);
        println!("sphere createOctreeBoxes with minCellSize: {} leaves, DATA levels {lv:?}", o.number_of_leaves());
        assert_eq!(lv.keys().copied().collect::<Vec<_>>(), vec![5]);
        let mut o2 = MeshOctree::new(sphere3());
        let mut c2 = MeshOctreeCreator::new(&o2, Some(MeshDict::with_max_cell_size(1.0)));
        c2.set_root_cube_size_and_ref_parameters(&mut o2);
        assert_eq!(c2.automatic_refinement(&mut o2), AutomaticRefinementReport::default());
        let pipe = crate::mesh_octree_queries::tests::pipe(0.01, 0.06, 48);
        let pd = MeshDict { min_cell_size: Some(0.0005), ..MeshDict::with_max_cell_size(0.004) };
        let (mut op, _) = boundary_octree(pipe, &pd);
        let mut a = MeshOctreeAutomaticRefinement::new(&op, pd.min_cell_size, pd.max_cell_size, false, false);
        let n0 = op.number_of_leaves();
        a.automatic_refinement(&mut op);
        println!("pipe automatic refinement: leaves {n0} -> {}, DATA levels {:?}, report {:?}", op.number_of_leaves(), data_levels(&op), a.report());
        assert!(op.number_of_leaves() > n0);
    }

    /// `help::numberOfFaceGroups` / `numberOfEdgeGroups` on hand-built
    /// cases: a far triangle is its own group; two triangles sharing an
    /// in-range edge are one; a triangle out of range does not count; two
    /// edges sharing an in-range point are one group; nothing is in a zero
    /// range.
    #[test]
    fn group_counts_by_hand() {
        let p = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(1.0, 0.0, 5.0),
            Vec3::new(0.0, 1.0, 5.0),
        ];
        let s = TriSurf::new(p, vec![[0, 1, 2], [1, 3, 2], [4, 5, 6]], Some(vec![0, 1, 0]));
        let all: BTreeSet<usize> = [0, 1, 2].into_iter().collect();
        assert_eq!(number_of_face_groups(&all, Vec3::new(0.5, 0.5, 2.5), 10.0, &s), 2);
        let two: BTreeSet<usize> = [0, 1].into_iter().collect();
        assert_eq!(number_of_face_groups(&two, Vec3::new(0.5, 0.5, 0.0), 1.0, &s), 1);
        assert_eq!(number_of_face_groups(&two, Vec3::new(1.0, 1.0, 0.0), 0.2, &s), 1);
        let es: BTreeSet<usize> = [s.find_edge(1, 2).unwrap(), s.find_edge(0, 1).unwrap()].into_iter().collect();
        assert_eq!(number_of_edge_groups(&es, Vec3::new(1.0, 0.0, 0.0), 0.5, &s), 1);
        assert_eq!(number_of_edge_groups(&es, Vec3::new(1.0, 0.0, 0.0), 0.0, &s), 0);
    }
}
