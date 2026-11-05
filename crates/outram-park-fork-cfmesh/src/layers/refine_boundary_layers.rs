// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f):
//   meshLibrary/utilities/boundaryLayers/detectBoundaryLayers/
//     detectBoundaryLayers.{C,H}, detectBoundaryLayersFunctions.C
//       (meshBndLayerNeighbourOperator, meshBndLayerSelectorOperator,
//        analyseLayers, findHairsForFace, generateHairEdges)
//   meshLibrary/utilities/boundaryLayers/refineBoundaryLayers/
//     refineBoundaryLayers.{C,H}       (the settings and their defaults:
//                                       1 layer, ratio 1, VGREAT)
//     refineBoundaryLayersFunctions.C   (analyseLayers, generateNewVertices)
// Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! **`detectBoundaryLayers` and `refineBoundaryLayers`** (gh:#1271): find
//! the one-cell layers at the boundary of any mesh and split them into
//! several, as cfMesh does after `boundaryLayers` (and on the first cells of
//! `cartesianMesh`).
//!
//! # `detectBoundaryLayers` (17b), ported whole (serial)
//!
//! A boundary face is a layer face when its owner is a hexahedron, or a
//! prism over it: one boundary face, the others all quads that share an
//! edge with it except one face that shares none
//! (`meshBndLayerSelectorOperator`). Layer faces are grouped across
//! boundary edges whose owners share a face (`meshBndLayerNeighbourOperator`,
//! `help::groupMarking` = [`crate::mesh_octree_creator::group_marking`]). A
//! patch with any face outside a layer has none. Each layer face gives one
//! hair per point, the cell edge shared by the two side faces at that
//! point, oriented from the boundary point inward (`findHairsForFace`);
//! duplicates are removed (`generateHairEdges`).
//!
//! # `refineBoundaryLayers` (17a)
//!
//! Ported: the settings (global and per-patch number of layers, thickness
//! ratio, maximum first-layer thickness, and `setInteruptForPatch`, which
//! keeps a patch's own count), `analyseLayers` (each layer group takes the
//! most layers any of its patches asks for; a patch outside every layer
//! has one) and `generateNewVertices`: on a hair of length `L` with `n`
//! layers and ratio `r`, the first layer is `L (1 − r) / (1 − r^n)` (`L/n`
//! for `r ≤ 1 + SMALL`), capped by the maximum first-layer thickness, and
//! the split points are at `t1 (1 + r + … + r^(j−1))` from the boundary.
//! A hair's count is the most over the boundary faces at its start that do
//! not hold it; its ratio is that of the last such face's patch with one
//! (upstream's loop overwrites); its cap the smallest.
//!
//! **Not ported (the cell split of `refineBoundaryLayersCells.C` and
//! `refineBoundaryLayersFaces.C` in general):** this port splits **prism
//! layer cells** (one base face, its hairs all with the same count) and
//! inserts the split points into every other face holding a hair. A cell
//! that is a layer cell for two base faces whose hairs both carry more than
//! one layer (upstream's `refineEdgeHexCell`, `refineCornerHexCell`) or whose
//! hairs carry different counts is refused with
//! [`RefineError::NotPorted`]. `activateSpecialMode`, `pointsInBndLayer`,
//! the cell subset, the 2D mode and the MPI branches are not ported.
//!
//! The split cells follow [`super::build_layered_variable`]'s face winding
//! (wall ring, top ring reversed, side quads); the layers of one cell are
//! numbered consecutively in its place, so later cell labels shift.

use crate::math::Vec3;
use crate::surface_engine::{SurfaceEngine, VSMALL};
use crate::topology::TopoMesh;
use crate::volume_mesh::{from_cell_faces_wound, VolumeMesh};
use std::collections::{BTreeMap, BTreeSet};

/// OpenFOAM `SMALL` for `double`.
const SMALL: f64 = 1.0e-15;

/// The layers [`detect_boundary_layers`] found.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedLayers {
    /// Layer group of each boundary face ([`SurfaceEngine::boundary_faces`]
    /// order), `None` outside every layer (`faceInLayer`).
    pub layer_at_bnd_face: Vec<Option<usize>>,
    /// Number of layer groups (`nDistinctLayers`).
    pub n_layers: usize,
    /// Layer groups of each patch (`layerAtPatch`).
    pub layer_at_patch: Vec<Vec<usize>>,
    /// Hair edges, boundary point first (`hairEdges`), mesh labels.
    pub hair_edges: Vec<[usize; 2]>,
    /// Owner cells of the layer faces, ascending.
    pub layer_cells: BTreeSet<usize>,
}

/// Faces of each cell of `mesh` (ascending), and each cell's faces as rings
/// wound out of it.
fn cell_faces(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut cf = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.face_count() {
        cf[mesh.owner[f]].push(f);
        if let Some(n) = mesh.neighbour[f] {
            cf[n].push(f);
        }
    }
    cf
}

fn same_edge(a: (usize, usize), b: (usize, usize)) -> bool {
    (a.0 == b.0 && a.1 == b.1) || (a.0 == b.1 && a.1 == b.0)
}

fn share_an_edge(f1: &[usize], f2: &[usize]) -> bool {
    let (k1, k2) = (f1.len(), f2.len());
    (0..k1).any(|i| (0..k2).any(|j| same_edge((f1[i], f1[(i + 1) % k1]), (f2[j], f2[(j + 1) % k2]))))
}

/// `meshBndLayerSelectorOperator` for the boundary face at mesh face `bfm`
/// with owner `c`.
fn is_layer_cell(mesh: &VolumeMesh, cf: &[Vec<usize>], c: usize, bfm: usize) -> bool {
    let faces = &cf[c];
    let n_quads = faces.iter().filter(|&&f| mesh.faces[f].len() == 4).count();
    if n_quads == 6 {
        return true;
    }
    if n_quads + 2 != faces.len() {
        return false;
    }
    let n_bnd = faces.iter().filter(|&&f| f == bfm).count();
    if n_bnd != 1 {
        return false;
    }
    let base = &mesh.faces[bfm];
    let mut attached = 0usize;
    let mut other: Option<usize> = None;
    for &f in faces {
        if f == bfm {
            continue;
        }
        let s = share_an_edge(base, &mesh.faces[f]);
        if mesh.faces[f].len() == 4 && s {
            attached += 1;
        } else if !s {
            if other.is_some() {
                return false;
            }
            other = Some(f);
        }
    }
    attached + 2 == faces.len() && other.is_some_and(|o| !share_an_edge(base, &mesh.faces[o]))
}

/// `findHairsForFace`: the hair at each point of the boundary face `bfm`
/// (owner `c`), oriented from the face's point, or `None` if the cell is not
/// a prism over it.
fn hairs_for_face(mesh: &VolumeMesh, cf: &[Vec<usize>], c: usize, bfm: usize) -> Option<Vec<[usize; 2]>> {
    let faces = &cf[c];
    if faces.len() != mesh.faces[bfm].len() + 2 {
        return None;
    }
    // Cell edges in order of appearance and the cell faces on each.
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut edge_faces: Vec<Vec<usize>> = Vec::new();
    for &f in faces {
        let ring = &mesh.faces[f];
        let k = ring.len();
        for i in 0..k {
            let e = (ring[i], ring[(i + 1) % k]);
            let pos = match edges.iter().position(|&x| same_edge(x, e)) {
                Some(p) => p,
                None => {
                    edges.push(e);
                    edge_faces.push(Vec::new());
                    edges.len() - 1
                }
            };
            edge_faces[pos].push(f);
        }
    }
    let find = |a: usize, b: usize| edges.iter().position(|&x| same_edge(x, (a, b)));
    let bf = &mesh.faces[bfm];
    let k = bf.len();
    let mut hairs = Vec::with_capacity(k);
    for p in 0..k {
        let next = find(bf[p], bf[(p + 1) % k])?;
        let prev = find(bf[(p + k - 1) % k], bf[p])?;
        if edge_faces[next].len() != 2 || edge_faces[prev].len() != 2 {
            return None;
        }
        let other_next = if edge_faces[next][0] == bfm { edge_faces[next][1] } else { edge_faces[next][0] };
        let other_prev = if edge_faces[prev][0] == bfm { edge_faces[prev][1] } else { edge_faces[prev][0] };
        let common = (0..edges.len()).find(|&e| edge_faces[e].contains(&other_next) && edge_faces[e].contains(&other_prev))?;
        let e = edges[common];
        hairs.push(if e.0 == bf[p] { [e.0, e.1] } else { [e.1, e.0] });
    }
    Some(hairs)
}

/// cfMesh `detectBoundaryLayers` (serial): see the module documentation.
pub fn detect_boundary_layers(mesh: &VolumeMesh) -> DetectedLayers {
    let engine = SurfaceEngine::new(mesh);
    let cf = cell_faces(mesh);
    let nbf = engine.boundary_faces.len();
    let owner = &engine.face_owners;
    let shares_a_face = |a: usize, b: usize| cf[a].iter().any(|f| cf[b].contains(f));
    let neighbours = |bf: usize| -> Vec<usize> {
        let mut out = Vec::new();
        for &e in &engine.face_edges[bf] {
            let ef = &engine.edge_faces[e];
            if ef.len() != 2 {
                continue;
            }
            let nei = if ef[0] == bf { ef[1] } else { ef[0] };
            if owner[nei] == owner[bf] {
                continue;
            }
            if shares_a_face(owner[bf], owner[nei]) {
                out.push(nei);
            }
        }
        out
    };
    let selector = |bf: usize| is_layer_cell(mesh, &cf, owner[bf], engine.boundary_faces[bf]);
    let (mut layer, n_layers) = crate::mesh_octree_creator::group_marking(nbf, neighbours, selector);
    let n_patches = engine.face_patches.iter().copied().max().map_or(0, |m| m + 1);
    // patchToLayer, in patch order: a patch with any face outside a layer
    // has no layer.
    let mut layer_at_patch: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
    let mut cleared = vec![false; n_patches];
    for bf in 0..nbf {
        let pi = engine.face_patches[bf];
        match layer[bf] {
            None => cleared[pi] = true,
            Some(g) => {
                if !layer_at_patch[pi].contains(&g) {
                    layer_at_patch[pi].push(g);
                }
            }
        }
    }
    for pi in 0..n_patches {
        if cleared[pi] {
            layer_at_patch[pi].clear();
        }
    }
    for bf in 0..nbf {
        if layer_at_patch[engine.face_patches[bf]].is_empty() {
            layer[bf] = None;
        }
    }
    // generateHairEdges.
    let mut hairs: Vec<[usize; 2]> = Vec::new();
    for bf in 0..nbf {
        if layer[bf].is_none() {
            continue;
        }
        if let Some(h) = hairs_for_face(mesh, &cf, owner[bf], engine.boundary_faces[bf]) {
            hairs.extend(h);
        }
    }
    let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
    hairs.retain(|h| seen.insert((h[0].min(h[1]), h[0].max(h[1]))));
    let layer_cells = (0..nbf).filter(|&bf| layer[bf].is_some()).map(|bf| owner[bf]).collect();
    DetectedLayers { layer_at_bnd_face: layer, n_layers, layer_at_patch, hair_edges: hairs, layer_cells }
}

/// Per-patch `refineBoundaryLayers` settings (`meshDict`'s
/// `boundaryLayers/patchBoundaryLayers/<patch>`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PatchLayerSettings {
    /// `nLayers`.
    pub n_layers: Option<usize>,
    /// `thicknessRatio` (ignored below 1, as upstream).
    pub thickness_ratio: Option<f64>,
    /// `maxFirstLayerThickness` [m] (ignored if not positive).
    pub max_first_layer_thickness: Option<f64>,
    /// `allowDiscontinuity`: the patch keeps its own count
    /// (`setInteruptForPatch`).
    pub allow_discontinuity: bool,
}

/// `refineBoundaryLayers` settings; the defaults are upstream's (1 layer,
/// ratio 1, no cap).
#[derive(Debug, Clone, PartialEq)]
pub struct RefineBoundaryLayersSettings {
    /// `nLayers` (values below 2 are ignored, as upstream).
    pub n_layers: usize,
    /// `thicknessRatio`.
    pub thickness_ratio: f64,
    /// `maxFirstLayerThickness` [m].
    pub max_first_layer_thickness: f64,
    /// Per-patch settings by patch name.
    pub patches: BTreeMap<String, PatchLayerSettings>,
}

impl Default for RefineBoundaryLayersSettings {
    fn default() -> Self {
        RefineBoundaryLayersSettings { n_layers: 1, thickness_ratio: 1.0, max_first_layer_thickness: f64::MAX, patches: BTreeMap::new() }
    }
}

/// Why [`refine_boundary_layers`] did not refine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefineError {
    /// A case this port does not split (see the module documentation).
    NotPorted(String),
}

/// What [`refine_boundary_layers`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct RefineReport {
    /// The layers it found.
    pub detected: DetectedLayers,
    /// Layers of each boundary face (`nLayersAtBndFace`), boundary-face
    /// order.
    pub n_layers_at_bnd_face: Vec<usize>,
    /// Each hair's points, boundary first: positions in the output mesh
    /// (`newVerticesForSplitEdge`).
    pub hair_points: Vec<Vec<Vec3>>,
    /// Cells split.
    pub n_cells_split: usize,
}

/// cfMesh `refineBoundaryLayers::refineLayers` on `mesh` (see the module
/// documentation for what is ported). With no patch or global count above
/// one, the mesh comes back unchanged (`Ok` with no split).
pub fn refine_boundary_layers(mesh: &VolumeMesh, s: &RefineBoundaryLayersSettings) -> Result<(VolumeMesh, RefineReport), RefineError> {
    let engine = SurfaceEngine::new(mesh);
    let detected = detect_boundary_layers(mesh);
    let names: Vec<String> = mesh.patches.iter().map(|p| p.name.clone()).collect();
    let global_n = if s.n_layers < 2 { 1 } else { s.n_layers };
    let global_r = if s.thickness_ratio < 1.0 { 1.0 } else { s.thickness_ratio };
    let global_t = if s.max_first_layer_thickness <= 0.0 { f64::MAX } else { s.max_first_layer_thickness };

    // analyseLayers: patches of each layer group, then the count of each.
    let n_patches = detected.layer_at_patch.len();
    let mut patches_in_layer: Vec<Vec<usize>> = vec![Vec::new(); detected.n_layers];
    for pi in 0..n_patches {
        for &g in &detected.layer_at_patch[pi] {
            patches_in_layer[g].push(pi);
        }
    }
    let mut n_at_patch: Vec<i64> = vec![-1; n_patches];
    let mut protected = vec![false; n_patches];
    let setting = |pi: usize| names.get(pi).and_then(|n| s.patches.get(n));
    for lp in &patches_in_layer {
        let mut max_n = 1usize;
        let mut local = false;
        for &pi in lp {
            if let Some(ps) = setting(pi) {
                if let Some(n) = ps.n_layers {
                    if ps.allow_discontinuity {
                        n_at_patch[pi] = n as i64;
                        protected[pi] = true;
                    } else {
                        max_n = max_n.max(n);
                    }
                    local = true;
                }
            }
        }
        if !local {
            max_n = global_n;
        }
        for &pi in lp {
            if !protected[pi] {
                n_at_patch[pi] = max_n as i64;
            }
        }
    }
    let n_at_bf: Vec<usize> = engine.face_patches.iter().map(|&pi| if pi < n_patches && n_at_patch[pi] >= 0 { n_at_patch[pi] as usize } else { 1 }).collect();

    // generateNewVertices.
    let hairs = &detected.hair_edges;
    let mut points = mesh.points.clone();
    let mut hair_labels: Vec<Vec<usize>> = Vec::with_capacity(hairs.len());
    for h in hairs {
        let b = engine.bp[h[0]];
        let mut n_layers = 1usize;
        let mut ratio = global_r;
        let mut thickness = global_t;
        let mut overridden = false;
        for &bf in &engine.point_faces[b] {
            let ring = &engine.face_rings[bf];
            let k = ring.len();
            if (0..k).any(|i| same_edge((ring[i], ring[(i + 1) % k]), (h[0], h[1]))) {
                continue;
            }
            n_layers = n_layers.max(n_at_bf[bf]);
            let pi = engine.face_patches[bf];
            if let Some(ps) = setting(pi) {
                if let Some(r) = ps.thickness_ratio.filter(|&r| r >= 1.0) {
                    ratio = r;
                }
                if let Some(t) = ps.max_first_layer_thickness.filter(|&t| t > 0.0) {
                    thickness = if overridden { thickness.min(t) } else { t };
                    overridden = true;
                }
            }
        }
        let (start, end) = (mesh.points[h[0]], mesh.points[h[1]]);
        let v = end.sub(start);
        let magv = v.length();
        let mut first = magv / n_layers as f64;
        if ratio > 1.0 + SMALL {
            first = magv / ((1.0 - ratio.powi(n_layers as i32)) / (1.0 - ratio));
        }
        let first = thickness.max(SMALL).min(first);
        let dir = v.scale(1.0 / (magv + VSMALL));
        let mut labels = vec![h[0]];
        let mut param = first;
        for j in 1..n_layers {
            labels.push(points.len());
            points.push(start.add(dir.scale(param)));
            param += first * ratio.powi(j as i32);
        }
        labels.push(h[1]);
        hair_labels.push(labels);
    }
    let hair_points: Vec<Vec<Vec3>> = hair_labels.iter().map(|l| l.iter().map(|&p| points[p]).collect()).collect();
    let empty = |detected: DetectedLayers, n_at_bf: Vec<usize>| RefineReport { detected, n_layers_at_bnd_face: n_at_bf, hair_points: Vec::new(), n_cells_split: 0 };
    if hair_labels.iter().all(|l| l.len() == 2) {
        let w = TopoMesh::from_volume_mesh(mesh);
        return Ok((w.into_volume_mesh().0, empty(detected, n_at_bf)));
    }

    // The split: each layer cell into the count of its hairs.
    let hair_of: BTreeMap<(usize, usize), usize> = hairs.iter().enumerate().map(|(i, h)| ((h[0].min(h[1]), h[0].max(h[1])), i)).collect();
    let cf = cell_faces(mesh);
    let mut base_of: BTreeMap<usize, usize> = BTreeMap::new();
    for bf in 0..engine.boundary_faces.len() {
        if detected.layer_at_bnd_face[bf].is_none() {
            continue;
        }
        let c = engine.face_owners[bf];
        let fm = engine.boundary_faces[bf];
        let Some(h) = hairs_for_face(mesh, &cf, c, fm) else { continue };
        let counts: BTreeSet<usize> = h.iter().map(|e| hair_labels[hair_of[&(e[0].min(e[1]), e[0].max(e[1]))]].len() - 1).collect();
        if counts.len() != 1 {
            return Err(RefineError::NotPorted(format!("layer cell {c}: its hairs carry {counts:?} layers")));
        }
        if counts.contains(&1) {
            continue;
        }
        if let Some(prev) = base_of.insert(c, fm) {
            return Err(RefineError::NotPorted(format!("cell {c} is a layer cell for two base faces ({prev}, {fm}): refineEdgeHexCell / refineCornerHexCell")));
        }
    }
    // A split cell's hairs must not lie in another split cell's base.
    for (&c, &fm) in &base_of {
        let ring = &mesh.faces[fm];
        let k = ring.len();
        for i in 0..k {
            let key = (ring[i].min(ring[(i + 1) % k]), ring[i].max(ring[(i + 1) % k]));
            if let Some(&hi) = hair_of.get(&key) {
                if hair_labels[hi].len() > 2 {
                    return Err(RefineError::NotPorted(format!("the base of layer cell {c} holds a split hair")));
                }
            }
        }
    }
    // Rings with the split points of every hair inserted.
    let subdivide = |ring: &[usize]| -> Vec<usize> {
        let k = ring.len();
        let mut out = Vec::with_capacity(k);
        for i in 0..k {
            let (a, b) = (ring[i], ring[(i + 1) % k]);
            out.push(a);
            if let Some(&hi) = hair_of.get(&(a.min(b), a.max(b))) {
                let l = &hair_labels[hi];
                if l[0] == a {
                    out.extend(l[1..l.len() - 1].iter().copied());
                } else {
                    out.extend(l[1..l.len() - 1].iter().rev().copied());
                }
            }
        }
        out
    };
    let outward = |c: usize, f: usize| -> Vec<usize> {
        let mut r = mesh.faces[f].clone();
        if mesh.owner[f] != c {
            r.reverse();
        }
        r
    };
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::new();
    let mut patch_of_key: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    let key = |ring: &[usize]| -> Vec<usize> {
        let mut k = ring.to_vec();
        k.sort_unstable();
        k
    };
    let mut region_of_face = vec![None; mesh.face_count()];
    for (pi, p) in mesh.patches.iter().enumerate() {
        for r in region_of_face.iter_mut().skip(p.start_face).take(p.n_faces) {
            *r = Some(pi);
        }
    }
    for c in 0..mesh.n_cells {
        if let Some(&fm) = base_of.get(&c) {
            let base = &mesh.faces[fm];
            let k = base.len();
            let hs = hairs_for_face(mesh, &cf, c, fm).expect("a split cell is a prism over its base");
            let col: Vec<Vec<usize>> = hs
                .iter()
                .enumerate()
                .map(|(p, h)| {
                    let mut l = hair_labels[hair_of[&(h[0].min(h[1]), h[0].max(h[1]))]].clone();
                    if l[0] != base[p] {
                        l.reverse();
                    }
                    l
                })
                .collect();
            let n = col[0].len() - 1;
            // Side faces of the cell on the boundary keep their patch.
            for &f in &cf[c] {
                if f == fm || mesh.neighbour[f].is_some() {
                    continue;
                }
                if let Some(pi) = region_of_face[f] {
                    let ring = &mesh.faces[f];
                    if let Some(e) = (0..k).find(|&e| {
                        let (a, b) = (base[e], base[(e + 1) % k]);
                        ring.contains(&a) && ring.contains(&b)
                    }) {
                        for i in 0..n {
                            let (p, q) = (e, (e + 1) % k);
                            patch_of_key.insert(key(&[col[p][i], col[p][i + 1], col[q][i + 1], col[q][i]]), pi);
                        }
                    }
                }
            }
            for i in 0..n {
                let mut faces: Vec<Vec<usize>> = Vec::with_capacity(k + 2);
                faces.push((0..k).map(|p| col[p][i]).collect());
                faces.push((0..k).rev().map(|p| col[p][i + 1]).collect());
                for e in 0..k {
                    let (p, q) = (e, (e + 1) % k);
                    faces.push(vec![col[p][i], col[p][i + 1], col[q][i + 1], col[q][i]]);
                }
                cells.push(faces);
            }
        } else {
            cells.push(cf[c].iter().map(|&f| subdivide(&outward(c, f))).collect());
        }
    }
    for f in 0..mesh.face_count() {
        if mesh.neighbour[f].is_none() {
            if let Some(pi) = region_of_face[f] {
                patch_of_key.entry(key(&subdivide(&mesh.faces[f]))).or_insert(pi);
            }
        }
    }
    let layered = from_cell_faces_wound(points, &cells);
    let mut w = TopoMesh::from_volume_mesh(&layered);
    w.patch_names = names.clone();
    for f in 0..w.faces.len() {
        w.region[f] = if w.neighbour[f].is_none() { patch_of_key.get(&key(&w.faces[f])).copied() } else { None };
    }
    let (out, _) = w.into_volume_mesh();
    let n_split = base_of.len();
    Ok((out, RefineReport { detected, n_layers_at_bnd_face: n_at_bf, hair_points, n_cells_split: n_split }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::check_quality;
    use crate::layers::boundary_layers::tests::{bare_pipe, named_box};
    use crate::layers::boundary_layers::{boundary_layers_add_layer, BoundaryLayersControls, BoundaryLayersOutcome};

    fn walls(n: usize, r: Option<f64>, t: Option<f64>) -> RefineBoundaryLayersSettings {
        let mut s = RefineBoundaryLayersSettings::default();
        s.patches.insert("walls".into(), PatchLayerSettings { n_layers: Some(n), thickness_ratio: r, max_first_layer_thickness: t, allow_discontinuity: false });
        s
    }

    /// The short pipe (L 60 mm, h 6 mm) with cfMesh's layer on `walls`.
    fn layered_pipe() -> (VolumeMesh, VolumeMesh, BoundaryLayersOutcome) {
        let bare = bare_pipe(0.06, 0.006);
        let (l, o) = boundary_layers_add_layer(&bare, &["walls"], &BoundaryLayersControls::default()).unwrap();
        (bare, l, o)
    }

    fn bits(p: Vec3) -> [u64; 3] {
        [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
    }

    /// V&V (gh:#1271, 17b) — `detectBoundaryLayers` on a mesh from item 16
    /// (the L 60 mm pipe, h 6 mm, cfMesh's layer on `walls`). Predicted
    /// before the first run: the layer cells found are exactly the cells
    /// the layer added (the last `n_wall` cells); `walls` is in a layer,
    /// `inlet` and `outlet` are not (their faces' owners include dual
    /// polyhedra, which are not prisms, so the whole patch is cleared);
    /// one hair per wall point, from the point's boundary copy (at its old
    /// position, bit for bit) to the moved point, of the column's length.
    ///
    /// Result (2026-11-05): as predicted (1 420 layer cells, 1 456 hairs,
    /// only `walls` in a layer). The group count is 51, of which 50 are
    /// empty (`diagnose_layer_groups_on_the_pipe`).
    #[test]
    fn detect_finds_exactly_the_cfmesh_layer() {
        let (_, l, o) = layered_pipe();
        let d = detect_boundary_layers(&l);
        let expect: BTreeSet<usize> = (o.first_layer_cell..l.cell_count()).collect();
        println!("{} layer groups, {} layer cells, {} hairs, patches {:?}", d.n_layers, d.layer_cells.len(), d.hair_edges.len(), d.layer_at_patch);
        assert_eq!(d.layer_cells, expect);
        let names: Vec<&str> = l.patches.iter().map(|p| p.name.as_str()).collect();
        for (pi, name) in names.iter().enumerate() {
            assert_eq!(!d.layer_at_patch[pi].is_empty(), *name == "walls", "{name}");
        }
        assert_eq!(d.hair_edges.len(), o.points.len());
        let by_wall: BTreeMap<[u64; 3], f64> = o.points.iter().map(|p| (bits(p.wall), p.length)).collect();
        for h in &d.hair_edges {
            let len = by_wall.get(&bits(l.points[h[0]])).expect("a hair starts at a wall point");
            assert!((l.points[h[1]].sub(l.points[h[0]]).length() - len).abs() < 1e-12 * len.max(1e-300));
        }
    }

    /// Diagnostic (gh:#1271): why the pipe's one wall gives many layer
    /// groups. For each pair of wall faces across a boundary edge in
    /// different groups: do their owners differ, and share a face?
    /// Result (2026-11-05): 51 groups; one holds all 1 420 wall faces and
    /// the other 50 are empty. They were numbered by `groupMarking` for
    /// inlet/outlet faces whose owners pass the selector, before the patch
    /// clearing emptied them; upstream's `nFirstLayers_` is taken at the same
    /// point, so the count is upstream's. No edge joins two groups.
    #[test]
    fn diagnose_layer_groups_on_the_pipe() {
        let (_, l, _) = layered_pipe();
        let d = detect_boundary_layers(&l);
        let engine = SurfaceEngine::new(&l);
        let cf = cell_faces(&l);
        let (mut same_owner, mut no_shared, mut shared) = (0, 0, 0);
        for e in 0..engine.edges.len() {
            let ef = &engine.edge_faces[e];
            if ef.len() != 2 {
                continue;
            }
            let (a, b) = (ef[0], ef[1]);
            if d.layer_at_bnd_face[a].is_none() || d.layer_at_bnd_face[b].is_none() || d.layer_at_bnd_face[a] == d.layer_at_bnd_face[b] {
                continue;
            }
            let (oa, ob) = (engine.face_owners[a], engine.face_owners[b]);
            if oa == ob {
                same_owner += 1;
            } else if cf[oa].iter().any(|f| cf[ob].contains(f)) {
                shared += 1;
            } else {
                no_shared += 1;
                if no_shared <= 3 {
                    let ring = &engine.face_rings[a];
                    println!("  edge {:?}: owners {oa} ({} faces) and {ob} ({} faces) share no face; face a ring {:?}", engine.edges[e], cf[oa].len(), cf[ob].len(), ring);
                }
            }
        }
        let mut sizes = vec![0usize; d.n_layers];
        for g in d.layer_at_bnd_face.iter().flatten() {
            sizes[*g] += 1;
        }
        sizes.sort_unstable();
        println!("{} groups, sizes {:?}; edges between groups: same owner {same_owner}, owners sharing a face {shared}, sharing none {no_shared}", d.n_layers, sizes);
        assert_eq!((same_owner, shared, no_shared), (0, 0, 0));
        assert_eq!(sizes.iter().filter(|&&n| n > 0).count(), 1);
    }

    /// V&V (gh:#1271, 17a) — differential: the split points of
    /// `refineBoundaryLayers` against `build_layered_variable` for the same
    /// columns. Methodology: the layered L 60 mm pipe split into 3 layers
    /// at ratio 1.2 on `walls` (1 elsewhere); the crate's builder given the
    /// bare pipe, the same wall faces, the columns of item 16 (direction,
    /// length), `d` = 0, 1, 2.2, 3.64 (first 1, expansion 1.2) and 3 layers
    /// per point. Predicted before the first run:
    /// - refined: item 16's cells + 2 x wall faces, volume unchanged
    ///   (1e-12 relative), closed, no inverted cell, every boundary face in
    ///   a named patch;
    /// - every split point agrees with the builder's to 1e-12 x the column
    ///   length (upstream: `t1 (1 − r^j)/(1 − r)` along the hair, the
    ///   builder: `L d_j / d_n` along the column, equal analytically), and
    ///   not every one bit for bit (different operation order).
    ///
    /// Results (2026-11-05): 2 025 -> 4 865 cells (1 420 split), volume
    /// unchanged (1e-12), closed, every face named; split points within
    /// 1e-12 of the column length (held), not all bit for bit (held).
    /// **Miss:** 56 inverted cells, not 0. They are the input's: item 16's
    /// cfMesh layer on this pipe has 56 inverted cells already (gh:#1270
    /// differential, written after this prediction), and the split does
    /// not change them. The assertion is now "as many as the input".
    #[test]
    fn split_points_match_build_layered_variable() {
        let (bare, l, o) = layered_pipe();
        let n_wall = o.wall_faces.len();
        let (r, rep) = refine_boundary_layers(&l, &walls(3, Some(1.2), None)).unwrap();
        r.validate().unwrap();
        assert_eq!(r.cell_count(), l.cell_count() + 2 * n_wall);
        assert_eq!(rep.n_cells_split, n_wall);
        assert!(((r.total_volume() - l.total_volume()) / l.total_volume()).abs() < 1e-12);
        assert_eq!(check_quality(&r).n_negative_volume_cells, check_quality(&l).n_negative_volume_cells);
        assert!(r.patches.iter().all(|p| p.name != "unassigned"), "{:?}", r.patches);

        let wall_pts: BTreeSet<usize> = o.points.iter().map(|p| p.point).collect();
        let inward: BTreeMap<usize, Vec3> = o.points.iter().map(|p| (p.point, p.direction)).collect();
        let totals: BTreeMap<usize, f64> = o.points.iter().map(|p| (p.point, p.length)).collect();
        let counts: BTreeMap<usize, usize> = o.points.iter().map(|p| (p.point, 3)).collect();
        let d = [0.0, 1.0, 1.0 + 1.2, 1.0 + 1.2 + 1.2 * 1.2];
        let (built, column) = super::super::build_layered_variable_with_columns(&bare, &o.wall_faces, &wall_pts, &inward, &d, &totals, &counts);
        let by_wall: BTreeMap<[u64; 3], &Vec<Vec3>> = rep.hair_points.iter().map(|h| (bits(h[0]), h)).collect();
        let (mut worst, mut n_bit, mut n_all) = (0.0f64, 0usize, 0usize);
        for p in &o.points {
            let hp = by_wall[&bits(p.wall)];
            let col = &column[&p.point];
            assert_eq!(hp.len(), col.len());
            for (a, &b) in hp.iter().zip(col) {
                let q = built.points[b];
                worst = worst.max(a.sub(q).length() / p.length);
                n_all += 1;
                if bits(*a) == bits(q) {
                    n_bit += 1;
                }
            }
        }
        println!("split points: worst |difference| / column length {worst:.3e}; {n_bit} of {n_all} bit for bit; refined {} cells", r.cell_count());
        assert!(worst < 1e-12);
        assert!(n_bit < n_all);
    }

    /// V&V (gh:#1271, 17a) — `maxFirstLayerThickness`. The layered L 60 mm
    /// pipe, 3 layers at ratio 1.2 on `walls`, the first layer capped at
    /// 0.1 x the shortest column. Predicted before the first run: on every
    /// hair the first layer is the cap and the second 1.2 x the cap (to
    /// 1e-12 relative), and the third takes the rest of the column.
    ///
    /// Result (2026-11-05): **miss on the tolerance only.** The first two
    /// layers equal the cap and 1.2 x the cap to 3e-12 relative, not 1e-12:
    /// the cap is 1.6e-7 m and the points sit about 1e-2 m from the origin,
    /// so a difference of two positions carries ~1e-18 m of round-off
    /// (1e-11 of the layer). The check is now absolute, 1e-14 x the pipe
    /// length (6e-16 m), the scale of the coordinates.
    #[test]
    fn max_first_layer_thickness_caps_the_first_layer() {
        let (_, l, o) = layered_pipe();
        let cap = 0.1 * o.points.iter().map(|p| p.length).fold(f64::MAX, f64::min);
        let (r, rep) = refine_boundary_layers(&l, &walls(3, Some(1.2), Some(cap))).unwrap();
        r.validate().unwrap();
        for h in &rep.hair_points {
            let (t1, t2) = (h[1].sub(h[0]).length(), h[2].sub(h[1]).length());
            let tol = 1e-14 * 0.06;
            assert!((t1 - cap).abs() < tol && (t2 - 1.2 * cap).abs() < tol, "{t1} {t2} {cap}");
            assert!(h[3].sub(h[2]).length() > t2);
        }
    }

    /// V&V (gh:#1271) — cfMesh's layer on a cartesian box, refined. cfMesh's layer on the carved unit box (h 0.25): every boundary
    /// cell is a hexahedron, so `detectBoundaryLayers` counts the `inlet`
    /// and `outlet` patches as layers too, in the same group as `walls`;
    /// with 3 layers on `walls` the group takes 3, and the prism at the rim
    /// is a layer cell for both its wall face and its inlet side face
    /// (upstream's `refineEdgeHexCell`). Predicted before the first run:
    /// all three patches in a layer, one group; `NotPorted`.
    ///
    /// Result (2026-11-05): **both missed.** Three groups, one per patch:
    /// `meshBndLayerNeighbourOperator` joins two layer faces only across a
    /// boundary edge whose two faces have *different* owners that share a
    /// face; a wall face and the inlet side face of the same rim prism
    /// have the same owner, and that is the only place the patches meet.
    /// So `walls` alone takes 3 layers and `inlet`/`outlet` keep 1. The rim
    /// prism is still a layer cell for its inlet side face, but that
    /// face's hairs (wall edges along x) carry one layer: the count skips
    /// the boundary faces that hold the hair, and both wall faces there
    /// hold it. Nothing needs a two-direction split; the walls refine:
    /// 256 cells, 64 split, volume 1 (1e-12), closed.
    #[test]
    fn cartesian_box_layer_refines_the_walls_only() {
        let (l, _) = boundary_layers_add_layer(&named_box(0.25), &["walls"], &BoundaryLayersControls::default()).unwrap();
        let d = detect_boundary_layers(&l);
        println!("box: {} groups, patches {:?}", d.n_layers, d.layer_at_patch);
        assert_eq!(d.n_layers, 3);
        assert_eq!(d.layer_at_patch.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![1, 1, 1]);
        let (r, rep) = refine_boundary_layers(&l, &walls(3, None, None)).unwrap();
        println!("refined: {} cells, {} split, volume {:.15}", r.cell_count(), rep.n_cells_split, r.total_volume());
        assert_eq!((r.cell_count(), rep.n_cells_split), (256, 64));
        assert!((r.total_volume() - 1.0).abs() < 1e-12);
        r.validate().unwrap();
    }

    /// Images (gh:#1270, #1271), `--ignored`: drawn from the assembled
    /// meshes into `verification_and_validation/boundary_layers_gh1270/`.
    /// Box (h 0.25): y-z cut at x = 0.375 after cfMesh's layer and after
    /// the 3-layer refinement (ratio 1.2). Pipe (L 60 mm, h 6 mm): x-y cut
    /// at mid-length (whole and a wall zoom) and x-z rim zoom at the inlet
    /// (cut at y = 0.1 mm; the x-y cut at z = 31.7 mm, off the 6 mm grid planes) for cfMesh's layer, the crate's back-off layer
    /// and cfMesh's layer refined into 3; faces of inverted cells orange.
    #[test]
    #[ignore]
    fn draw_layer_images() {
        use crate::topology::tests::write_slice_png;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/boundary_layers_gh1270");
        let none = |_f: usize| None;
        let (bl, _) = boundary_layers_add_layer(&named_box(0.25), &["walls"], &BoundaryLayersControls::default()).unwrap();
        let (br, _) = refine_boundary_layers(&bl, &walls(3, Some(1.2), None)).unwrap();
        let bw = [-0.05, 1.05, -0.05, 1.05];
        write_slice_png(&bl, 0, 0.375, bw, &[], &none, "box h 0.25: cfMesh boundaryLayers, walls on y/z sides", &dir.join("box_cfmesh_layer_yz.png"));
        write_slice_png(&br, 0, 0.375, bw, &[], &none, "box: cfMesh layer refined into 3 (ratio 1.2)", &dir.join("box_refined_3_yz.png"));
        let (bare, l, _) = layered_pipe();
        let (b, _) = super::super::add_boundary_layers_back_off(&bare, &["walls"], 1, 1.0, 1.0);
        let (r, _) = refine_boundary_layers(&l, &walls(3, Some(1.2), None)).unwrap();
        for (name, m) in [("bare", &bare), ("cfmesh_layer", &l), ("backoff_layer", &b), ("cfmesh_refined_3", &r)] {
            let (vols, _) = crate::checks::cell_geometry(m);
            let inv = |f: usize| (vols[m.owner[f]] <= 0.0 || m.neighbour[f].is_some_and(|n| vols[n] <= 0.0)).then_some([255u8, 127, 14]);
            let n_inv = vols.iter().filter(|&&v| v <= 0.0).count();
            let t = format!("pipe R 10 mm L 60 mm h 6 mm, {name}: {} cells, {n_inv} inverted (orange)", m.cell_count());
            write_slice_png(m, 2, 0.0317, [-0.0108, 0.0108, -0.0108, 0.0108], &[(0.0, 0.0, 0.01)], &inv, &t, &dir.join(format!("pipe_{name}_xy.png")));
            write_slice_png(m, 2, 0.0317, [0.0065, 0.0105, -0.002, 0.002], &[(0.0, 0.0, 0.01)], &inv, &t, &dir.join(format!("pipe_{name}_xy_wall.png")));
            write_slice_png(m, 1, 1e-4, [0.0065, 0.0105, -0.0005, 0.0035], &[], &inv, &t, &dir.join(format!("pipe_{name}_xz_rim.png")));
        }
    }
}
