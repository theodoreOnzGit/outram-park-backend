// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeModifier/
//     meshOctreeModifier.{C,H}, meshOctreeModifierI.H (createListOfLeaves),
//     meshOctreeModifierEnsureCorrectRegularity.C (ensureCorrectRegularity,
//       ensureCorrectRegularitySons),
//     meshOctreeModifierRefineSelectedBoxes.C (markAdditionalLayers (both
//       forms), markAdditionalLayersOfFaceNeighbours, refineSelectedBoxes,
//       refineSelectedBoxesAndAdditionalLayers),
//     meshOctreeModifierReduceMemoryConsumption.C
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

//! `meshOctreeModifier` (gh:#1263): refinement of selected leaves with the
//! **1-irregularity** rule (neighbours over faces, edges and nodes differ by
//! at most one level), and the layer-marking helpers the creator uses.
//!
//! Upstream's modifier is a friend class with access to the octree's
//! private data; here its routines are methods on [`MeshOctree`]. The
//! `refineBox` lists are `&mut [i32]` indexed by leaf label: `0` not
//! selected, `> 0` selected (the layer routines store the layer number).
//!
//! [`crate::octree::balance_2to1`] balances over **faces only**; upstream's
//! `ensureCorrectRegularity` balances over all 26 neighbours. The two are
//! kept side by side.
//!
//! Not ported: the MPI routines (`distributeLeavesToProcessors`,
//! `loadDistribution`, `refineTreeForCoordinates`,
//! `updateCommunicationPattern`, `addLayerFromNeighbouringProcessors`).

use crate::mesh_octree::MeshOctree;
use std::collections::BTreeMap;

impl MeshOctree {
    /// `ensureCorrectRegularity`: grow the selection until refining it keeps
    /// the octree 1-irregular. Front propagation: a selected leaf selects
    /// every coarser leaf among its node, edge and face neighbours.
    pub fn ensure_correct_regularity(&self, refine_box: &mut [i32]) {
        let mut front: Vec<usize> = (0..refine_box.len()).filter(|&l| refine_box[l] != 0).collect();
        while !front.is_empty() {
            let mut t_front = Vec::new();
            for &l in &front {
                let cc = self.return_leaf(l).coords;
                for nei in self.find_all_leaf_neighbours(&cc).into_iter().flatten() {
                    if self.return_leaf(nei).coords.level >= cc.level {
                        continue;
                    }
                    if refine_box[nei] == 0 {
                        refine_box[nei] = 1;
                        t_front.push(nei);
                    }
                }
            }
            front = t_front;
        }
    }

    /// `ensureCorrectRegularitySons`: if one son of a father is selected,
    /// select its leaf siblings. Returns whether anything was added.
    pub fn ensure_correct_regularity_sons(&self, refine_box: &mut [i32]) -> bool {
        let mut n_marked = 0;
        for l in 0..self.number_of_leaves() {
            if refine_box[l] == 0 {
                continue;
            }
            let father = self.return_leaf(l).coords.reduce_level_by(1);
            for sc in 0..8 {
                if let Some(nei) = self.find_leaf_label_for_position(&father.refine_for_position(sc)) {
                    if refine_box[nei] == 0 {
                        n_marked += 1;
                        refine_box[nei] = 1;
                    }
                }
            }
        }
        n_marked != 0
    }

    /// `markAdditionalLayers(refineBox, nLayers)`: for `i` in `1..=n_layers`,
    /// every unselected node/edge/face neighbour of a leaf marked `i` is
    /// marked `i + 1`.
    pub fn mark_additional_layers(&self, refine_box: &mut [i32], n_layers: i32) {
        self.mark_layers(refine_box, n_layers, false);
    }

    /// `markAdditionalLayersOfFaceNeighbours`: as
    /// [`MeshOctree::mark_additional_layers`], over faces only.
    pub fn mark_additional_layers_of_face_neighbours(&self, refine_box: &mut [i32], n_layers: i32) {
        self.mark_layers(refine_box, n_layers, true);
    }

    fn mark_layers(&self, refine_box: &mut [i32], n_layers: i32, faces_only: bool) {
        for i in 1..=n_layers {
            let active: Vec<usize> = (0..refine_box.len()).filter(|&l| refine_box[l] == i).collect();
            for l in active {
                let cc = self.return_leaf(l).coords;
                let neis =
                    if faces_only { self.find_neighbours_for_leaf(&cc) } else { self.find_all_leaf_neighbours(&cc) };
                for nei in neis.into_iter().flatten() {
                    if refine_box[nei] == 0 {
                        refine_box[nei] = i + 1;
                    }
                }
            }
        }
    }

    /// `markAdditionalLayers(refineBox, nLayers, targetRefLevel)`: per-leaf
    /// layer counts and target levels. For each layer count (largest first)
    /// and each target level (highest first), mark the face-neighbour layers
    /// around the leaves with that count and target, and select every marked
    /// leaf coarser than the target. Returns the number newly selected.
    pub fn mark_additional_layers_per_box(&self, refine_box: &mut [i32], n_layers: &[i32], target_ref_level: &[u8]) -> usize {
        let n = self.number_of_leaves();
        let max_layer = n_layers.iter().copied().max().unwrap_or(0).max(0) as usize;
        let mut for_layer: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
        for l in 0..n {
            if n_layers[l] >= 1 {
                for_layer[n_layers[l] as usize].push(l);
            }
        }
        let mut n_marked = 0;
        for layer in (0..for_layer.len()).rev() {
            let active = &for_layer[layer];
            if active.is_empty() {
                continue;
            }
            let max_level = active.iter().map(|&l| target_ref_level[l]).max().unwrap_or(0);
            for level in (1..=max_level).rev() {
                let mut marked = vec![0i32; n];
                let mut counter = 0;
                for &l in active {
                    if target_ref_level[l] == level {
                        marked[l] = 1;
                        counter += 1;
                    }
                }
                if counter == 0 {
                    continue;
                }
                self.mark_additional_layers_of_face_neighbours(&mut marked, layer as i32);
                for l in 0..n {
                    if marked[l] < 2 || self.return_leaf(l).coords.level >= level {
                        continue;
                    }
                    if refine_box[l] == 0 {
                        refine_box[l] |= 1;
                        n_marked += 1;
                    }
                }
            }
        }
        n_marked
    }

    /// `refineSelectedBoxes`: make the selection 1-irregular
    /// (`ensureCorrectRegularity`, and with `hex_refinement` also select the
    /// siblings of every selected leaf, repeating until stable), refine every
    /// selected leaf in leaf order, then rebuild the leaf list.
    pub fn refine_selected_boxes(&mut self, refine_box: &mut [i32], hex_refinement: bool) {
        loop {
            self.ensure_correct_regularity(refine_box);
            if !(hex_refinement && self.ensure_correct_regularity_sons(refine_box)) {
                break;
            }
        }
        let leaves = self.leaves.clone();
        for (l, &ci) in leaves.iter().enumerate() {
            if refine_box[l] != 0 {
                self.refine_cube(ci);
            }
        }
        self.create_list_of_leaves();
    }

    /// `refineSelectedBoxesAndAdditionalLayers`: refine the selected leaves
    /// together with `ceil(ref_thickness / size)` (at least 1) layers of
    /// neighbours around them, level by level, so the refined region is
    /// `ref_thickness` thick.
    pub fn refine_selected_boxes_and_additional_layers(&mut self, refine_box: &[i32], ref_thickness: &[f64]) {
        // (level, nLayers) -> the cubes (slot indices) to refine; a BTreeMap,
        // as upstream's std::map.
        let mut leaves_map: BTreeMap<(u8, i32), Vec<usize>> = BTreeMap::new();
        let n = self.number_of_leaves();
        let max_level = (0..n).filter(|&l| refine_box[l] != 0).map(|l| self.return_leaf(l).coords.level).max();
        let Some(max_level) = max_level else {
            return;
        };
        let mut for_level: Vec<Vec<usize>> = vec![Vec::new(); max_level as usize + 1];
        for l in 0..n {
            if refine_box[l] != 0 {
                for_level[self.return_leaf(l).coords.level as usize].push(l);
            }
        }
        for level in (0..for_level.len()).rev() {
            let active = &for_level[level];
            if active.is_empty() {
                continue;
            }
            let n_for: Vec<i32> = active
                .iter()
                .map(|&l| {
                    let cs = self.return_leaf(l).coords.size(&self.root_box);
                    ((ref_thickness[l] / cs).ceil() as i32).max(1)
                })
                .collect();
            let max_n = *n_for.iter().max().unwrap_or(&0);
            for layer in 0..=max_n {
                leaves_map.insert((level as u8, layer), Vec::new());
            }
            for (i, &l) in active.iter().enumerate() {
                leaves_map.get_mut(&(level as u8, n_for[i])).map(|v| v.push(self.leaves[l]));
            }
        }
        for ((level, n_layers), selected) in leaves_map {
            if selected.is_empty() {
                continue;
            }
            loop {
                let mut marked = vec![0i32; self.number_of_leaves()];
                let mut n_marked = 0;
                for &ci in &selected {
                    if let Some(l) = self.cube(ci).cube_label {
                        marked[l] = 1;
                        n_marked += 1;
                    }
                }
                if n_marked == 0 {
                    break;
                }
                self.mark_additional_layers(&mut marked, n_layers);
                let active: Vec<usize> = (0..marked.len()).filter(|&l| marked[l] != 0).collect();
                let mut has_lower = false;
                for &l in &active {
                    let lv = self.return_leaf(l).coords.level;
                    if lv < level {
                        has_lower = true;
                    } else if lv > level {
                        marked[l] = 0;
                    }
                }
                if has_lower {
                    for &l in &active {
                        if self.return_leaf(l).coords.level == level {
                            marked[l] = 0;
                        }
                    }
                }
                self.refine_selected_boxes(&mut marked, false);
            }
        }
    }

    /// `reduceMemoryConsumption`: empty the contained-triangle and
    /// contained-edge rows that no leaf refers to. Row numbers are kept (as
    /// upstream's `setRowSize(i, 0)`), so the leaves' row labels stay valid.
    pub fn reduce_memory_consumption(&mut self) {
        let mut keep_t = vec![false; self.slot.contained_triangles.len()];
        let mut keep_e = vec![false; self.slot.contained_edges.len()];
        for &ci in &self.leaves {
            let c = &self.slot.cubes[ci];
            if let Some(r) = c.contained_elements {
                keep_t[r] = true;
            }
            if let Some(r) = c.contained_edges {
                keep_e[r] = true;
            }
        }
        for (r, k) in keep_t.iter().enumerate() {
            if !k {
                self.slot.contained_triangles[r] = Vec::new();
            }
        }
        for (r, k) in keep_e.iter().enumerate() {
            if !k {
                self.slot.contained_edges[r] = Vec::new();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::math::Vec3;
    use crate::mesh_octree::tests::data_octree;
    use crate::mesh_octree_queries::tests::pipe;
    use std::sync::Arc;

    /// Refine the leaves holding triangles `level` times through
    /// `refineSelectedBoxes` (so with the 1-irregularity rule), as
    /// `meshOctreeCreator::refineBoundary` does without its size control.
    pub(crate) fn balanced_data_octree(surf: Arc<crate::mesh_octree::TriSurf>, level: u8) -> crate::mesh_octree::MeshOctree {
        let mut o = crate::mesh_octree::MeshOctree::new(surf);
        for _ in 0..level {
            let mut mark: Vec<i32> =
                (0..o.number_of_leaves()).map(|l| o.has_contained_triangles(l) as i32).collect();
            o.refine_selected_boxes(&mut mark, false);
        }
        o
    }

    fn max_level_jump(o: &crate::mesh_octree::MeshOctree) -> i32 {
        let mut j = 0;
        for l in 0..o.number_of_leaves() {
            let cc = o.return_leaf(l).coords;
            for m in o.find_all_leaf_neighbours(&cc).into_iter().flatten() {
                j = j.max((o.return_leaf(m).coords.level as i32 - cc.level as i32).abs());
            }
        }
        j
    }

    /// V&V (gh:#1263): `refineSelectedBoxes` keeps the octree 1-irregular.
    /// Selecting one level-5 leaf on the pipe wall of a level-5 DATA octree
    /// and refining it three times (each time selecting the finest leaf
    /// nearest the same point) forces `ensureCorrectRegularity` to refine a
    /// growing ring of coarser neighbours; after each pass every leaf
    /// differs from each of its 26-neighbours by at most one level. With
    /// `hex_refinement` every refined father has all 8 sons at the same
    /// level. `reduceMemoryConsumption` keeps every leaf's triangles and
    /// edges and empties some ancestor rows.
    ///
    /// Prediction: jump <= 1 after every pass; without the regularity pass
    /// (refining the single leaf with `refine_cube` directly) the jump grows
    /// to 2 at the second pass.
    #[test]
    fn refine_selected_boxes_keeps_one_irregularity() {
        let surf = pipe(0.01, 0.06, 48);
        let mut o = balanced_data_octree(Arc::clone(&surf), 5);
        assert!(max_level_jump(&o) <= 1, "the creator-style base is 1-irregular");
        let mut raw = o.clone();
        let p = Vec3::new(0.01, 0.0, 0.03);
        let mut raw_jumps = Vec::new();
        for _ in 0..3 {
            let l = o.find_leaf_containing_vertex(p).unwrap();
            let mut mark = vec![0i32; o.number_of_leaves()];
            mark[l] = 1;
            o.refine_selected_boxes(&mut mark, false);
            assert!(max_level_jump(&o) <= 1);
            let rl = raw.find_leaf_containing_vertex(p).unwrap();
            let ci = raw.leaves[rl];
            raw.refine_cube(ci);
            raw.create_list_of_leaves();
            raw_jumps.push(max_level_jump(&raw));
        }
        println!("without ensureCorrectRegularity, jumps per pass {raw_jumps:?}");
        assert!(raw_jumps[1] >= 2, "the regularity pass is doing work");

        let before: Vec<Vec<usize>> = (0..o.number_of_leaves()).map(|l| o.contained_triangles(l)).collect();
        let rows_before = o.slot.contained_triangles.iter().filter(|r| !r.is_empty()).count();
        o.reduce_memory_consumption();
        let after: Vec<Vec<usize>> = (0..o.number_of_leaves()).map(|l| o.contained_triangles(l)).collect();
        assert_eq!(before, after);
        assert!(o.slot.contained_triangles.iter().filter(|r| !r.is_empty()).count() < rows_before);

        let mut h = balanced_data_octree(pipe(0.01, 0.06, 48), 4);
        let l = h.find_leaf_containing_vertex(p).unwrap();
        let mut mark = vec![0i32; h.number_of_leaves()];
        mark[l] = 1;
        h.refine_selected_boxes(&mut mark, true);
        for c in 0..h.slot.cubes.len() {
            if let Some(subs) = h.sub_cubes(c) {
                let leafy = subs.iter().filter(|&&s| h.cube(s).is_leaf()).count();
                assert!(leafy == 8 || subs.iter().any(|&s| !h.cube(s).is_leaf()));
            }
        }
        assert!(max_level_jump(&h) <= 1);
    }

    /// V&V (gh:#1263): the layer markers. Marking one leaf and adding 2
    /// layers over all neighbours marks exactly the leaves within 2 steps of
    /// it in the all-neighbour graph (breadth-first search as the oracle),
    /// with labels 2 and 3; the face-only form uses the face graph.
    /// `refineSelectedBoxesAndAdditionalLayers` with a thickness of 2 cells
    /// refines a region at least 2 old cells thick around the leaf.
    #[test]
    fn additional_layers_match_breadth_first_search() {
        let o = data_octree(pipe(0.01, 0.06, 48), 4);
        let p = Vec3::new(0.0, 0.0, 0.03);
        let l0 = o.find_leaf_containing_vertex(p).unwrap();
        for faces_only in [false, true] {
            let mut mark = vec![0i32; o.number_of_leaves()];
            mark[l0] = 1;
            if faces_only {
                o.mark_additional_layers_of_face_neighbours(&mut mark, 2);
            } else {
                o.mark_additional_layers(&mut mark, 2);
            }
            let mut dist = vec![i32::MAX; o.number_of_leaves()];
            dist[l0] = 0;
            let mut q = std::collections::VecDeque::from([l0]);
            while let Some(l) = q.pop_front() {
                if dist[l] == 2 {
                    continue;
                }
                let cc = o.return_leaf(l).coords;
                let ns = if faces_only { o.find_neighbours_for_leaf(&cc) } else { o.find_all_leaf_neighbours(&cc) };
                for m in ns.into_iter().flatten() {
                    if dist[m] == i32::MAX {
                        dist[m] = dist[l] + 1;
                        q.push_back(m);
                    }
                }
            }
            for l in 0..o.number_of_leaves() {
                let want = if dist[l] == i32::MAX { 0 } else { dist[l] + 1 };
                assert_eq!(mark[l], want, "leaf {l} faces_only {faces_only}");
            }
        }
        let mut o2 = o.clone();
        let size = o2.return_leaf(l0).coords.size(&o2.root_box);
        let mut sel = vec![0i32; o2.number_of_leaves()];
        sel[l0] = 1;
        let thick = vec![2.0 * size; o2.number_of_leaves()];
        let lev0 = o2.return_leaf(l0).coords.level;
        o2.refine_selected_boxes_and_additional_layers(&sel, &thick);
        for d in [-1.5, 0.0, 1.5] {
            let q = p.add(Vec3::new(d * size, 0.0, 0.0));
            let l = o2.find_leaf_containing_vertex(q).unwrap();
            assert!(o2.return_leaf(l).coords.level > lev0, "refined {d} cells away");
        }
        assert!(max_level_jump(&o2) <= 1);
        let mut pb = vec![0i32; o.number_of_leaves()];
        let nl = vec![1i32; o.number_of_leaves()];
        let tl: Vec<u8> = (0..o.number_of_leaves()).map(|l| if l == l0 { o.return_leaf(l).coords.level + 1 } else { 0 }).collect();
        let n = o.mark_additional_layers_per_box(&mut pb, &nl, &tl);
        assert_eq!(n, pb.iter().filter(|&&x| x != 0).count());
    }
}
