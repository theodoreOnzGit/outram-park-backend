// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/triSurfaceTools/triSurfaceDetectFeatureEdges/
//     triSurfaceDetectFeatureEdges{.C,.H,Functions.C}
//   meshLibrary/utilities/triSurfaceTools/triSurfacePatchManipulator/
//     triSurfacePatchManipulator{.C,.H,Functions.C}
//   meshLibrary/utilities/triSurfaceTools/triSurfaceClassifyEdges/
//     triSurfaceClassifyEdges{.C,.H,Functions.C}
//   meshLibrary/utilities/triSurfaceTools/triSurfacePartitioner/
//     triSurfacePartitioner{.C,.H,CreateAddressing.C}
//   utilities/surfaceFeatureEdges/surfaceFeatureEdges.C
//   utilities/patchesToSubsets/patchesToSubsets.C
//   utilities/subsetToPatch/subsetToPatch.C
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

//! Feature edges, patching and edge classification of a [`TriSurf`]
//! (gh:#1276, `docs/upstream_coverage.md` §4 item 22).

use crate::mesh_octree::{help, BoundBox, CubeType, MeshOctree, SurfacePatch, TriSurf, VSMALL};
use petir::mathf::RealMath;
use std::collections::{BTreeMap, BTreeSet};

/// `triSurfaceDetectFeatureEdges`' flag bits on `featureEdges_`.
pub struct FeatureFlag;

impl FeatureFlag {
    /// Set by the angle criterion.
    pub const ANGLE: u8 = 1;
    /// Set on every edge that does not have exactly two facets (open and
    /// non-manifold edges): upstream stores these as feature edges too.
    pub const NOT_MANIFOLD: u8 = 8;
}

/// Do the two facets of an edge share their two points traversed in the
/// **same** direction (inconsistent orientation)? `None` unless they share
/// exactly two points. The shared-index rule of
/// `detectFeatureEdgesAngleCriterion` and `checkOrientation`.
fn same_direction(t0: [usize; 3], t1: [usize; 3]) -> Option<bool> {
    let mut shared: Vec<(usize, usize)> = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            if t0[i] == t1[j] {
                shared.push((i, j));
            }
        }
    }
    if shared.len() != 2 {
        return None;
    }
    let (p0, p1) = (shared[0], shared[1]);
    Some(if (p0.0 + 1) % 3 == p1.0 { (p0.1 + 1) % 3 == p1.1 } else { (p1.1 + 1) % 3 == p0.1 })
}

/// `triSurfaceDetectFeatureEdges`: flags the surface edges whose two facet
/// normals deviate by more than `angle_tolerance` degrees, and every edge
/// without exactly two facets.
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurfaceDetectFeatureEdges {
    /// `featureEdges_`: [`FeatureFlag`] bits per surface edge.
    pub feature_edges: Vec<u8>,
    /// `angleTolerance_` (degrees).
    pub angle_tolerance: f64,
}

impl TriSurfaceDetectFeatureEdges {
    /// The constructor, which runs `detectFeatureEdgesAngleCriterion`.
    ///
    /// The cosine of the facet normals is negated when the two facets
    /// traverse their shared edge in the same direction (an inconsistently
    /// oriented pair), as upstream does.
    pub fn new(surf: &TriSurf, angle_deviation: f64) -> Self {
        let tol = (angle_deviation * std::f64::consts::PI / 180.0).r_cos();
        let normals = surf.facet_normals();
        let mut flags = vec![0u8; surf.edges.len()];
        for (e, ef) in surf.edge_facets.iter().enumerate() {
            if ef.len() != 2 {
                flags[e] |= FeatureFlag::NOT_MANIFOLD;
                continue;
            }
            let (n0, n1) = (normals[ef[0]], normals[ef[1]]);
            let mut cos_angle = n0.dot(n1) / (n0.length() * n1.length() + VSMALL);
            if same_direction(surf.triangles[ef[0]], surf.triangles[ef[1]]) == Some(true) {
                cos_angle *= -1.0;
            }
            if cos_angle < tol {
                flags[e] |= FeatureFlag::ANGLE;
            }
        }
        TriSurfaceDetectFeatureEdges { feature_edges: flags, angle_tolerance: angle_deviation }
    }

    /// `detectFeatureEdges()`: replace the surface's stored feature edges
    /// with the flagged surface edges, in edge order.
    pub fn detect_feature_edges(&self, surf: &mut TriSurf) {
        surf.feature_edges = self
            .feature_edges
            .iter()
            .enumerate()
            .filter(|(_, &f)| f != 0)
            .map(|(e, _)| surf.edges[e])
            .collect();
    }
}

/// `triSurfacePatchManipulator`: the connected regions of facets bounded by
/// the stored feature edges, by patch changes and by edges without exactly
/// two facets.
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurfacePatchManipulator {
    /// `featureEdges_`: the stored feature edges flagged on the surface
    /// edges (`allocateFeatureEdges`).
    pub feature_edges: Vec<bool>,
    /// `facetInPatch_`.
    pub facet_in_patch: Vec<usize>,
    /// `nPatches_`.
    pub n_patches: usize,
}

impl TriSurfacePatchManipulator {
    /// The constructor: `allocateFeatureEdges`, then `createPatches` (a
    /// depth-first front, `LongList::remove` popping the last entry).
    pub fn new(surf: &TriSurf) -> Self {
        let feature_edges = surf.stored_feature_edge_flags();
        let mut facet_in_patch: Vec<Option<usize>> = vec![None; surf.size()];
        let mut n_patches = 0;
        for t in 0..surf.size() {
            if facet_in_patch[t].is_some() {
                continue;
            }
            facet_in_patch[t] = Some(n_patches);
            let mut front = vec![t];
            while let Some(f) = front.pop() {
                for &e in &surf.facet_edges[f] {
                    if feature_edges[e] {
                        continue;
                    }
                    let ef = &surf.edge_facets[e];
                    if ef.len() != 2 {
                        continue;
                    }
                    let nei = if ef[0] == f { ef[1] } else { ef[0] };
                    if surf.regions[f] != surf.regions[nei] || facet_in_patch[nei].is_some() {
                        continue;
                    }
                    facet_in_patch[nei] = Some(n_patches);
                    front.push(nei);
                }
            }
            n_patches += 1;
        }
        TriSurfacePatchManipulator {
            feature_edges,
            facet_in_patch: facet_in_patch.into_iter().map(|p| p.unwrap_or(0)).collect(),
            n_patches,
        }
    }

    /// `detectedSurfaceRegions`: the facets of each new patch, ascending.
    pub fn detected_surface_regions(&self) -> Vec<Vec<usize>> {
        let mut g = vec![Vec::new(); self.n_patches];
        for (t, &p) in self.facet_in_patch.iter().enumerate() {
            g[p].push(t);
        }
        g
    }

    /// `surfaceWithPatches(nullptr, prefix, forceOverwrite)`: a new surface
    /// with one patch per detected region, facets ordered by patch.
    ///
    /// With `force_overwrite` the patches are named `prefix + i` and typed
    /// `"patch"`; otherwise each takes `<original name>_<i>` and the
    /// original's type. Facet subsets are renumbered and point subsets
    /// copied; as upstream, the stored feature edges and the feature-edge
    /// subsets are **not** carried over. The `meshDict` update is
    /// [`Self::patches_for_patch`].
    pub fn surface_with_patches(&self, surf: &TriSurf, prefix: &str, force_overwrite: bool) -> TriSurf {
        let fip = self.detected_surface_regions();
        let mut patches = vec![SurfacePatch::new(""); self.n_patches];
        for (p, facets) in fip.iter().enumerate() {
            if force_overwrite {
                patches[p] = SurfacePatch::new(&format!("{prefix}{p}"));
            } else if let Some(&f) = facets.last() {
                // Upstream assigns once per facet; the last one wins.
                let o = &surf.patches[surf.regions[f]];
                patches[p] = SurfacePatch { name: format!("{}_{p}", o.name), geometric_type: o.geometric_type.clone() };
            }
        }
        let mut tris = Vec::with_capacity(surf.size());
        let mut regions = Vec::with_capacity(surf.size());
        let mut new_label = vec![None; surf.size()];
        for (p, facets) in fip.iter().enumerate() {
            for &f in facets {
                new_label[f] = Some(tris.len());
                tris.push(surf.triangles[f]);
                regions.push(p);
            }
        }
        let mut s = TriSurf::from_parts(surf.points.clone(), tris, regions, patches, Vec::new());
        for id in surf.facet_subsets.indices() {
            let nid = s.facet_subsets.add(surf.facet_subsets.name(id).unwrap_or(""));
            for f in surf.facet_subsets.elements(id) {
                if let Some(nf) = new_label[f] {
                    s.facet_subsets.add_element(nid, nf);
                }
            }
        }
        for id in surf.point_subsets.indices() {
            let nid = s.point_subsets.add(surf.point_subsets.name(id).unwrap_or(""));
            for p in surf.point_subsets.elements(id) {
                s.point_subsets.add_element(nid, p);
            }
        }
        s
    }

    /// The `meshDict` half of `surfaceWithPatches`: for each original patch,
    /// the names of the new patches it was split into (upstream's
    /// `patchesForPatch`, handed to `checkMeshDict::updateDictionaries`),
    /// with the new names as [`Self::surface_with_patches`] gives them with
    /// `force_overwrite == false`. New patches are listed ascending
    /// (upstream: `labelHashSet` order). Upstream's fatal case, a
    /// `symmetryPlane` patch split into several parts, is an `Err`.
    pub fn patches_for_patch(&self, surf: &TriSurf) -> Result<BTreeMap<String, Vec<String>>, String> {
        let new = self.surface_with_patches(surf, "", false);
        let mut to_new: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); surf.patches.len()];
        for (f, &p) in self.facet_in_patch.iter().enumerate() {
            to_new[surf.regions[f]].insert(p);
        }
        let mut out = BTreeMap::new();
        let mut problems = Vec::new();
        for (p, set) in to_new.iter().enumerate() {
            let o = &surf.patches[p];
            if o.geometric_type == "symmetryPlane" && set.len() > 1 {
                problems.push(format!(
                    "Symmetry plane patch with a name {} is decomposed into {} separate parts",
                    o.name,
                    set.len()
                ));
            }
            out.insert(o.name.clone(), set.iter().map(|&n| new.patches[n].name.clone()).collect());
        }
        if problems.is_empty() {
            Ok(out)
        } else {
            Err(problems.join("; "))
        }
    }
}

/// What the `surfaceFeatureEdges` utility writes, chosen upstream by the
/// output file's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureEdgesOutput {
    /// `.fms`: the input surface with the detected feature edges stored.
    StoreFeatureEdges,
    /// Any other format: the surface split into patches at them
    /// (`surfaceWithPatches()`, default prefix `"patch"`, names
    /// `<original>_<i>`).
    SplitIntoPatches,
}

/// The `surfaceFeatureEdges` utility (file I/O not ported): detect feature
/// edges at `angle` degrees (upstream's default 45) and either store them
/// or split the surface into patches at them.
pub fn surface_feature_edges(surf: &TriSurf, angle: f64, output: FeatureEdgesOutput) -> TriSurf {
    let mut s = surf.clone();
    TriSurfaceDetectFeatureEdges::new(&s, angle).detect_feature_edges(&mut s);
    match output {
        FeatureEdgesOutput::StoreFeatureEdges => s,
        FeatureEdgesOutput::SplitIntoPatches => TriSurfacePatchManipulator::new(&s).surface_with_patches(&s, "patch", false),
    }
}

/// `triSurfaceClassifyEdges::edgeType_` bits.
pub struct EdgeType;

impl EdgeType {
    /// `NONE` (also every edge without exactly two facets).
    pub const NONE: u8 = 0;
    /// `CONVEXEDGE`.
    pub const CONVEX: u8 = 1;
    /// `CONCAVEEDGE`.
    pub const CONCAVE: u8 = 2;
    /// `FLATSURFACEEDGE`.
    pub const FLAT: u8 = 4;
    /// `FEATUREEDGE`: a stored feature edge or a patch boundary.
    pub const FEATURE: u8 = 8;
}

/// `triSurfaceClassifyEdges`: the orientation of each facet (outward,
/// inward or unknown) from the octree's `OUTSIDE` leaves, then each edge's
/// type from the sign of the tetrahedron its two facets make.
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurfaceClassifyEdges {
    /// `edgeTypes_`: [`EdgeType`] bits per surface edge.
    pub edge_types: Vec<u8>,
    /// `facetOrientation_`: 1 outward, 2 inward, 0 undecided.
    pub facet_orientation: Vec<u8>,
    /// The number of consistently oriented facet groups.
    pub n_orientation_groups: usize,
}

impl TriSurfaceClassifyEdges {
    /// The constructor: `checkOrientation`, then `classifyEdgesTypes`. The
    /// octree must have its leaves typed (`meshOctreeCreator`'s
    /// `createOctreeBoxes`); with no `OUTSIDE` leaf every facet stays
    /// undecided and only `FEATURE` bits are set.
    ///
    /// Port note: upstream reads `returnLeaf(leafJ)` for every entry of
    /// `findAllLeafNeighbours`, including the `-1` of a neighbour outside
    /// the root box (out-of-bounds read); the port skips those.
    pub fn new(octree: &MeshOctree) -> Self {
        let surf = &*octree.surface;
        let (facet_orientation, n_groups) = check_orientation(octree);
        let pts = &surf.points;
        let mut types = vec![EdgeType::NONE; surf.edge_facets.len()];
        for (e, flag) in surf.stored_feature_edge_flags().into_iter().enumerate() {
            if flag {
                types[e] |= EdgeType::FEATURE;
            }
        }
        for (e, ef) in surf.edge_facets.iter().enumerate() {
            if ef.len() != 2 {
                types[e] = EdgeType::NONE;
                continue;
            }
            let (f0, f1) = (ef[0], ef[1]);
            let (t0, t1) = (surf.triangles[f0], surf.triangles[f1]);
            if surf.regions[f0] != surf.regions[f1] {
                types[e] |= EdgeType::FEATURE;
            }
            let Some(apex) = t1.iter().copied().find(|p| !t0.contains(p)) else {
                // Upstream would read points[-1]; two facets on the same
                // three points have no apex.
                continue;
            };
            let (a, b, c, d) = (pts[t0[0]], pts[t0[1]], pts[t0[2]], pts[apex]);
            let vol = b.sub(a).cross(c.sub(a)).dot(d.sub(a)) / 6.0;
            let sign = match facet_orientation[f0] {
                1 => -1.0,
                2 => 1.0,
                _ => continue,
            };
            let v = sign * vol;
            types[e] |= if v > VSMALL {
                EdgeType::CONVEX
            } else if v < -VSMALL {
                EdgeType::CONCAVE
            } else {
                EdgeType::FLAT
            };
        }
        TriSurfaceClassifyEdges { edge_types: types, facet_orientation, n_orientation_groups: n_groups }
    }
}

/// `triSurfaceClassifyEdges::checkOrientation`: the facet orientation and
/// the number of consistently oriented groups.
fn check_orientation(octree: &MeshOctree) -> (Vec<u8>, usize) {
    let surf = &*octree.surface;
    let n = surf.size();
    let mut group: Vec<Option<usize>> = vec![None; n];
    let mut n_groups = 0;
    for t in 0..n {
        if group[t].is_some() {
            continue;
        }
        group[t] = Some(n_groups);
        let mut front = vec![t];
        while let Some(tl) = front.pop() {
            for &e in &surf.facet_edges[tl] {
                let ef = &surf.edge_facets[e];
                if ef.len() != 2 {
                    continue;
                }
                for &nei in ef {
                    if group[nei].is_some() || nei == tl {
                        continue;
                    }
                    // Same group when the shared edge is traversed in
                    // opposite directions (consistent orientation).
                    if same_direction(surf.triangles[tl], surf.triangles[nei]) == Some(false) {
                        group[nei] = Some(n_groups);
                        front.push(nei);
                    }
                }
            }
        }
        n_groups += 1;
    }
    // Leaves holding each triangle, in leaf order.
    let mut tri_leaves: Vec<Vec<usize>> = vec![Vec::new(); n];
    for l in 0..octree.number_of_leaves() {
        for t in octree.contained_triangles(l) {
            tri_leaves[t].push(l);
        }
    }
    let root = &octree.root_box;
    let mut votes = vec![(0usize, 0usize); n_groups];
    for t in 0..n {
        let g = group[t].unwrap_or(0);
        let tr = surf.triangles[t];
        let c = surf.points[tr[0]].add(surf.points[tr[1]]).add(surf.points[tr[2]]).scale(1.0 / 3.0);
        let nv = surf.facet_area_vector(t);
        let mag_n = nv.length();
        if mag_n < VSMALL {
            continue;
        }
        let nn = nv.scale(1.0 / mag_n);
        for &l in &tri_leaves[t] {
            let cc = octree.return_leaf(l).coords;
            for nei in octree.find_all_leaf_neighbours(&cc).into_iter().flatten() {
                let oc = octree.return_leaf(nei);
                if oc.cube_type & CubeType::OUTSIDE == 0 {
                    continue;
                }
                let length = 3.0 * oc.coords.size(root);
                let bb: BoundBox = oc.coords.cube_box(root);
                let end_pos = c.add(nn.scale(length));
                let end_neg = c.sub(nn.scale(length));
                if help::bound_box_line_intersection(c, end_pos, &bb) {
                    votes[g].0 += 1;
                } else if help::bound_box_line_intersection(c, end_neg, &bb) {
                    votes[g].1 += 1;
                }
            }
        }
    }
    let outward: Vec<u8> = votes
        .iter()
        .map(|&(p, m)| match p.cmp(&m) {
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Less => 2,
            std::cmp::Ordering::Equal => 0,
        })
        .collect();
    ((0..n).map(|t| outward[group[t].unwrap_or(0)]).collect(), n_groups)
}

/// `triSurfacePartitioner`: corners (points with three or more patch-
/// boundary edges), patch adjacency, groups of patch-boundary edges
/// between corners, and their addressing.
///
/// Containers that are `labelHashSet`s upstream are `BTreeSet`s here, so
/// their iteration is ascending.
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurfacePartitioner {
    /// `corners_`: point labels, ascending.
    pub corners: Vec<usize>,
    /// `cornerPatches_`: the patches at each corner, in `pointFacets` order.
    pub corner_patches: Vec<Vec<usize>>,
    /// `patchPatches_`: the patches each patch borders.
    pub patch_patches: Vec<BTreeSet<usize>>,
    /// `edgeGroups_`: per surface edge, its group (`None` for an edge that
    /// is not on a patch boundary).
    pub edge_groups: Vec<Option<usize>>,
    /// `edgeGroupEdgeGroups_`: groups meeting each group at a corner.
    pub edge_group_edge_groups: Vec<BTreeSet<usize>>,
    /// `patchesEdgeGroups_`: the edge groups between two patches.
    pub patches_edge_groups: BTreeMap<(usize, usize), BTreeSet<usize>>,
    /// `edgeGroupsCorners_`: the corners (indices into `corners`) shared by
    /// two edge groups.
    pub edge_groups_corners: BTreeMap<(usize, usize), BTreeSet<usize>>,
}

impl TriSurfacePartitioner {
    /// The constructor (`calculatePatchAddressing`).
    pub fn new(surf: &TriSurf) -> Self {
        let n_edges = surf.edges.len();
        // calculateCornersAndAddressing: only two-facet edges count.
        let mut n_at = vec![0usize; surf.n_points()];
        let mut patch_patches = vec![BTreeSet::new(); surf.patches.len()];
        for (e, ef) in surf.edge_facets.iter().enumerate() {
            if ef.len() != 2 {
                continue;
            }
            let (s, t) = (surf.regions[ef[0]], surf.regions[ef[1]]);
            if s != t {
                n_at[surf.edges[e][0]] += 1;
                n_at[surf.edges[e][1]] += 1;
                // calculatePatchPatches.
                patch_patches[s].insert(t);
                patch_patches[t].insert(s);
            }
        }
        let mut corners = Vec::new();
        let mut corner_patches = Vec::new();
        for (p, pf) in surf.point_facets.iter().enumerate() {
            if n_at[p] < 3 {
                continue;
            }
            let mut patches: Vec<usize> = Vec::new();
            for &f in pf {
                if !patches.contains(&surf.regions[f]) {
                    patches.push(surf.regions[f]);
                }
            }
            corners.push(p);
            corner_patches.push(patches);
        }
        // calculateEdgeGroups: edges with more than one patch, any facet
        // count.
        let patches_of = |e: usize| {
            let mut ps: Vec<usize> = Vec::new();
            for &f in &surf.edge_facets[e] {
                if !ps.contains(&surf.regions[f]) {
                    ps.push(surf.regions[f]);
                }
            }
            ps
        };
        let mut feature: Vec<bool> = (0..n_edges).map(|e| patches_of(e).len() > 1).collect();
        let corner_set: BTreeSet<usize> = corners.iter().copied().collect();
        let mut edge_groups: Vec<Option<usize>> = vec![None; n_edges];
        let mut n_groups = 0;
        for e in 0..n_edges {
            if !feature[e] || edge_groups[e].is_some() {
                continue;
            }
            edge_groups[e] = Some(n_groups);
            feature[e] = false;
            let mut front = vec![e];
            while let Some(el) = front.pop() {
                for &p in &surf.edges[el] {
                    if corner_set.contains(&p) {
                        continue;
                    }
                    for &ej in &surf.point_edges[p] {
                        if feature[ej] && edge_groups[ej].is_none() {
                            edge_groups[ej] = Some(n_groups);
                            feature[ej] = false;
                            front.push(ej);
                        }
                    }
                }
            }
            n_groups += 1;
        }
        // calculatePatchToEdgeGroups.
        let mut patches_edge_groups: BTreeMap<(usize, usize), BTreeSet<usize>> = BTreeMap::new();
        for e in 0..n_edges {
            let Some(g) = edge_groups[e] else { continue };
            let ps = patches_of(e);
            for i in 0..ps.len() {
                for j in i + 1..ps.len() {
                    patches_edge_groups.entry((ps[i].min(ps[j]), ps[i].max(ps[j]))).or_default().insert(g);
                }
            }
        }
        // calculateEdgeGroupsToCorners.
        let mut edge_group_edge_groups = vec![BTreeSet::new(); n_groups];
        let mut edge_groups_corners: BTreeMap<(usize, usize), BTreeSet<usize>> = BTreeMap::new();
        for (ci, &p) in corners.iter().enumerate() {
            let mut at: Vec<Option<usize>> = Vec::new();
            for &e in &surf.point_edges[p] {
                if !at.contains(&edge_groups[e]) {
                    at.push(edge_groups[e]);
                }
            }
            for i in 0..at.len() {
                let Some(gi) = at[i] else { continue };
                for gj in at[i + 1..].iter().flatten() {
                    let (a, b) = (gi.min(*gj), gi.max(*gj));
                    edge_group_edge_groups[a].insert(b);
                    edge_group_edge_groups[b].insert(a);
                    edge_groups_corners.entry((a, b)).or_default().insert(ci);
                }
            }
        }
        TriSurfacePartitioner {
            corners,
            corner_patches,
            patch_patches,
            edge_groups,
            edge_group_edge_groups,
            patches_edge_groups,
            edge_groups_corners,
        }
    }

    /// `edgeGroupsSharedByPatches(patch1, patch2)`.
    pub fn edge_groups_shared_by_patches(&self, p1: usize, p2: usize) -> Vec<usize> {
        self.patches_edge_groups.get(&(p1.min(p2), p1.max(p2))).map_or_else(Vec::new, |s| s.iter().copied().collect())
    }

    /// `cornersSharedByEdgeGroups(group1, group2)`.
    pub fn corners_shared_by_edge_groups(&self, g1: usize, g2: usize) -> Vec<usize> {
        self.edge_groups_corners.get(&(g1.min(g2), g1.max(g2))).map_or_else(Vec::new, |s| s.iter().copied().collect())
    }
}

/// The `patchesToSubsets` utility (file I/O not ported): add one facet
/// subset per patch, named after it, holding its facets.
pub fn patches_to_subsets(surf: &mut TriSurf) {
    for (p, name) in surf.patch_names().iter().enumerate() {
        let facets: Vec<usize> = (0..surf.size()).filter(|&t| surf.regions[t] == p).collect();
        let id = surf.facet_subsets.add(name);
        for t in facets {
            surf.facet_subsets.add_element(id, t);
        }
    }
}

/// The `subsetToPatch` utility (file I/O not ported): make a patch of each
/// facet subset whose name starts with `subset_name` (upstream's prefix
/// match), drop the patches left empty, renumber the facets' regions and
/// remove the converted subsets.
///
/// **Port of the intent, not the text.** Upstream's `makePatchFromSubset`
/// builds the new triangles and patch list and then discards them (it
/// never writes them back to the surface), and calls
/// `updateFacetsSubsets` with the *patch* renumbering map as if it were a
/// facet map; and its `main` takes the prefix branch only when a subset
/// with exactly that name exists, else appends the name of a subset that
/// does not exist. Here the new facets and patches are stored, the other
/// facet subsets are left unchanged (facet labels do not change), and the
/// prefix match is applied in both cases. Returns the converted subsets'
/// names.
pub fn subset_to_patch(surf: &mut TriSurf, subset_name: &str) -> Vec<String> {
    let names: Vec<String> = surf
        .facet_subsets
        .indices()
        .into_iter()
        .filter_map(|id| surf.facet_subsets.name(id).map(str::to_string))
        .filter(|n| n.starts_with(subset_name))
        .collect();
    let n_orig = surf.patches.len();
    let mut new_patches: Vec<SurfacePatch> = surf.patches.clone();
    for n in &names {
        new_patches.push(SurfacePatch::new(n));
    }
    let mut regions = surf.regions.clone();
    for (i, n) in names.iter().enumerate() {
        if let Some(id) = surf.facet_subsets.index(n) {
            for f in surf.facet_subsets.elements(id) {
                regions[f] = n_orig + i;
            }
        }
    }
    let mut count = vec![0usize; new_patches.len()];
    for &r in &regions {
        count[r] += 1;
    }
    let mut new_label = vec![None; new_patches.len()];
    let mut kept = Vec::new();
    for (p, &c) in count.iter().enumerate() {
        if c > 0 {
            new_label[p] = Some(kept.len());
            kept.push(new_patches[p].clone());
        }
    }
    let regions: Vec<usize> = regions.iter().map(|&r| new_label[r].unwrap_or(0)).collect();
    for n in &names {
        if let Some(id) = surf.facet_subsets.index(n) {
            surf.facet_subsets.remove(id);
        }
    }
    let mut s = TriSurf::from_parts(surf.points.clone(), surf.triangles.clone(), regions, kept, surf.feature_edges.clone());
    s.point_subsets = surf.point_subsets.clone();
    s.facet_subsets = surf.facet_subsets.clone();
    s.edge_subsets = surf.edge_subsets.clone();
    *surf = s;
    names
}
