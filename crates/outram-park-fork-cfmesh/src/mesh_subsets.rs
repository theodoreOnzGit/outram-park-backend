// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/meshes/primitives/meshSubsets/{meshSubset.H,
//     meshSubsetI.H}
//   meshLibrary/utilities/meshes/polyMeshGen/{polyMeshGenPoints.C,
//     polyMeshGenPointsI.H, polyMeshGenFaces.C, polyMeshGenFacesI.H,
//     polyMeshGenCells.C, polyMeshGenCellsI.H} (the subset members)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae (2026-02-17).
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

//! cfMesh's **mesh subsets** (gh:#1261): named sets of points, faces or
//! cells, as `polyMeshGen` carries them (`meshSubset`, `addPointSubset`,
//! `addPointToSubset`, …).
//!
//! Upstream keeps the subsets inside `polyMeshGen`. Here they are a value
//! beside the mesh, [`MeshSubsets`], because [`crate::volume_mesh::VolumeMesh`]
//! is built by struct literal in 31 places across the workspace and a new
//! field would touch all of them. The operations, the id rule (one more than
//! the largest id in use) and the ordering (`std::map` by id, `std::set`
//! elements, both ascending) are upstream's. ~~The `meshDict`/file reading and
//! writing of subsets (`polyMeshGen::read`/`write`) are not ported.~~
//! **UPDATED 2026-11-11 (gh:#1476):** `polyMeshGen::write`/`read` with the
//! subsets as set files are `crate::poly_mesh_gen_io` (feature `foam-export`).

use std::collections::{BTreeMap, BTreeSet};

/// `meshSubset::subsetType_`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsetKind {
    /// `CELLSUBSET`.
    Cell,
    /// `FACESUBSET`.
    Face,
    /// `POINTSUBSET`.
    Point,
    /// `FEATUREEDGESUBSET` (a `triSurf`'s feature-edge subsets, gh:#1276).
    FeatureEdge,
}

/// `meshSubset`: a name, a kind and an ascending set of labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshSubset {
    /// The subset's name.
    pub name: String,
    /// What its labels index.
    pub kind: SubsetKind,
    /// The labels (`std::set<label> data_`).
    pub data: BTreeSet<usize>,
}

impl MeshSubset {
    /// `updateSubset(newLabels)`: renumber; a label mapped to `None`
    /// (upstream `< 0`) is dropped.
    pub fn update(&mut self, new_labels: &[Option<usize>]) {
        self.data = self.data.iter().filter_map(|&e| new_labels[e]).collect();
    }

    /// `updateSubset(const VRWGraph&)`: each label becomes all the labels in
    /// its row (an element split into several).
    pub fn update_graph(&mut self, new_labels: &[Vec<usize>]) {
        self.data = self.data.iter().flat_map(|&e| new_labels[e].iter().copied()).collect();
    }
}

/// The subsets of one kind (`std::map<label, meshSubset>` in
/// `polyMeshGenPoints`, `polyMeshGenFaces` or `polyMeshGenCells`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsetGroup {
    kind: SubsetKind,
    subsets: BTreeMap<usize, MeshSubset>,
}

impl SubsetGroup {
    /// An empty group of `kind`.
    pub fn new(kind: SubsetKind) -> Self {
        SubsetGroup { kind, subsets: BTreeMap::new() }
    }

    /// `add*Subset(name)`: the id of the subset called `name`, created
    /// empty (id one more than the largest in use, 0 first) if there is
    /// none (upstream warns and returns the existing one).
    pub fn add(&mut self, name: &str) -> usize {
        if let Some(id) = self.index(name) {
            return id;
        }
        let id = self.subsets.keys().next_back().map_or(0, |&k| k + 1);
        self.subsets.insert(id, MeshSubset { name: name.to_string(), kind: self.kind, data: BTreeSet::new() });
        id
    }

    /// `remove*Subset(id)`; nothing if there is no such subset.
    pub fn remove(&mut self, id: usize) {
        self.subsets.remove(&id);
    }

    /// `*SubsetName(id)` (upstream warns and returns an empty word).
    pub fn name(&self, id: usize) -> Option<&str> {
        self.subsets.get(&id).map(|s| s.name.as_str())
    }

    /// `*SubsetIndex(name)` (upstream returns −1 for none).
    pub fn index(&self, name: &str) -> Option<usize> {
        self.subsets.iter().find(|(_, s)| s.name == name).map(|(&id, _)| id)
    }

    /// `add*ToSubset(id, element)`; nothing if there is no such subset.
    pub fn add_element(&mut self, id: usize, element: usize) {
        if let Some(s) = self.subsets.get_mut(&id) {
            s.data.insert(element);
        }
    }

    /// `remove*FromSubset(id, element)`.
    pub fn remove_element(&mut self, id: usize, element: usize) {
        if let Some(s) = self.subsets.get_mut(&id) {
            s.data.remove(&element);
        }
    }

    /// `*InSubsets(element)`: the ids of the subsets holding it, ascending.
    pub fn in_subsets(&self, element: usize) -> Vec<usize> {
        self.subsets.iter().filter(|(_, s)| s.data.contains(&element)).map(|(&id, _)| id).collect()
    }

    /// `*SubsetIndices()`: every id, ascending.
    pub fn indices(&self) -> Vec<usize> {
        self.subsets.keys().copied().collect()
    }

    /// `*sInSubset(id)`: the elements, ascending (empty for no such subset).
    pub fn elements(&self, id: usize) -> Vec<usize> {
        self.subsets.get(&id).map_or_else(Vec::new, |s| s.data.iter().copied().collect())
    }

    /// `update*Subsets(newLabels)`: renumber every subset.
    pub fn update(&mut self, new_labels: &[Option<usize>]) {
        for s in self.subsets.values_mut() {
            s.update(new_labels);
        }
    }

    /// `update*Subsets(const VRWGraph&)`.
    pub fn update_graph(&mut self, new_labels: &[Vec<usize>]) {
        for s in self.subsets.values_mut() {
            s.update_graph(new_labels);
        }
    }
}

/// The point, face and cell subsets of one mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshSubsets {
    /// `pointSubsets_`.
    pub points: SubsetGroup,
    /// `faceSubsets_`.
    pub faces: SubsetGroup,
    /// `cellSubsets_`.
    pub cells: SubsetGroup,
}

impl Default for MeshSubsets {
    fn default() -> Self {
        MeshSubsets { points: SubsetGroup::new(SubsetKind::Point), faces: SubsetGroup::new(SubsetKind::Face), cells: SubsetGroup::new(SubsetKind::Cell) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V (gh:#1261) — upstream's subset semantics. Predictions (written
    /// 2026-11-05, before the first run): ids are 0, 1, 2 in order of
    /// creation; adding an existing name returns its id; after removing id
    /// 1 the next new subset gets id 3 (max + 1, not the gap); elements come
    /// back ascending and once each; `update` drops labels mapped to `None`
    /// and renumbers the rest; `update_graph` splits one label into several.
    #[test]
    fn subsets_follow_upstream() {
        let mut s = MeshSubsets::default();
        let a = s.faces.add("badFaces");
        let b = s.faces.add("lockedFaces");
        let c = s.faces.add("other");
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(s.faces.add("lockedFaces"), 1);
        s.faces.remove(1);
        assert_eq!(s.faces.add("again"), 3);
        assert_eq!(s.faces.indices(), vec![0, 2, 3]);
        for e in [7, 3, 7, 1] {
            s.faces.add_element(a, e);
        }
        s.faces.add_element(c, 3);
        assert_eq!(s.faces.elements(a), vec![1, 3, 7]);
        assert_eq!(s.faces.in_subsets(3), vec![0, 2]);
        assert_eq!(s.faces.index("badFaces"), Some(0));
        assert_eq!(s.faces.name(2), Some("other"));
        assert_eq!(s.faces.index("missing"), None);
        s.faces.remove_element(a, 3);
        let mut map = vec![None; 8];
        map[1] = Some(10);
        map[7] = Some(4);
        s.faces.update(&map);
        assert_eq!(s.faces.elements(a), vec![4, 10]);
        let mut graph = vec![Vec::new(); 11];
        graph[4] = vec![20, 21];
        graph[10] = vec![5];
        s.faces.update_graph(&graph);
        assert_eq!(s.faces.elements(a), vec![5, 20, 21]);
        assert_eq!(s.points.add("p"), 0);
        assert_eq!(s.cells.indices(), Vec::<usize>::new());
    }
}
