// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/triSurfaceTools/triSurfaceCleanupDuplicates/
//     triSurfaceCleanupDuplicates{.C,.H,Functions.C}
//   meshLibrary/utilities/triSurfaceTools/triSurfaceCleanupDuplicateTriangles/
//     triSurfaceCleanupDuplicateTriangles{.C,.H,Functions.C}
//   meshLibrary/utilities/triSurfaceTools/triSurfaceRemoveFacets/
//     triSurfaceRemoveFacets{.C,.H,Functions.C}
//   meshLibrary/utilities/triSurfaceTools/triSurfaceCopyParts/
//     triSurfaceCopyParts{.C,.H}
//   meshLibrary/utilities/triSurfaceTools/triSurfaceImportSurfaceAsSubset/
//     triSurfaceImportSurfaceAsSubset{.C,.H}
//   utilities/copySurfaceParts/copySurfaceParts.C
//   utilities/removeSurfaceFacets/removeSurfaceFacets.C
//   utilities/importSurfaceAsSubset/importSurfaceAsSubset.C
//   utilities/mergeSurfacePatches/mergeSurfacePatches.C
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only. mergeSurfacePatches.C is
// Copyright (C) the OpenFOAM/cfMesh contributors named in that file, same
// licence.
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

//! Surface clean-up and part tools (gh:#1277, `docs/upstream_coverage.md`
//! §4 item 23).

use super::rebuild;
use crate::mesh_octree::{help, mag_sqr, BoundBox, MeshOctree, SurfacePatch, TriSurf, VGREAT, VSMALL};
use std::collections::BTreeSet;

/// `labelledTri::operator==` as the duplicate tests use it: the same three
/// points in the same or the reversed cyclic order (`triFace::compare`
/// non-zero) and the same region.
///
/// Not re-checked: OpenFOAM's `labelledTri`/`triFace` sources are not
/// vendored in this workspace; this is the ESI-OpenFOAM definition as
/// recalled. A reversed copy of a facet is therefore a duplicate.
pub fn same_labelled_tri(a: [usize; 3], ra: usize, b: [usize; 3], rb: usize) -> bool {
    if ra != rb {
        return false;
    }
    (0..3).any(|k| {
        let fwd = a[0] == b[k] && a[1] == b[(k + 1) % 3] && a[2] == b[(k + 2) % 3];
        let rev = a[0] == b[k] && a[1] == b[(k + 2) % 3] && a[2] == b[(k + 1) % 3];
        fwd || rev
    })
}

/// `checkDuplicateTriangles` (both classes): the new label of each facet,
/// a duplicate taking the label of its first copy, and the new count.
fn duplicate_triangle_labels(surf: &TriSurf) -> (Vec<usize>, usize) {
    let mut lbl: Vec<Option<usize>> = vec![None; surf.size()];
    let mut counter = 0;
    for t in 0..surf.size() {
        if lbl[t].is_some() {
            continue;
        }
        lbl[t] = Some(counter);
        counter += 1;
        let tri = surf.triangles[t];
        for &j in &surf.point_facets[tri[0]] {
            if j <= t {
                continue;
            }
            if same_labelled_tri(tri, surf.regions[t], surf.triangles[j], surf.regions[j]) {
                lbl[j] = lbl[t];
            }
        }
    }
    (lbl.into_iter().map(|l| l.unwrap_or(0)).collect(), counter)
}

/// Apply [`duplicate_triangle_labels`]: the facets in their new order (for
/// a duplicate, the last copy written wins, as upstream), with the facet
/// subsets renumbered. `None` if there is no duplicate.
fn remove_duplicate_triangles(surf: &TriSurf) -> Option<(TriSurf, Vec<usize>)> {
    let (lbl, counter) = duplicate_triangle_labels(surf);
    if counter == surf.size() {
        return None;
    }
    let mut tris = vec![[0usize; 3]; counter];
    let mut regions = vec![0usize; counter];
    for t in 0..surf.size() {
        tris[lbl[t]] = surf.triangles[t];
        regions[lbl[t]] = surf.regions[t];
    }
    let mut s = rebuild(surf, surf.points.clone(), tris, regions);
    let map: Vec<Option<usize>> = lbl.iter().map(|&l| Some(l)).collect();
    s.facet_subsets.update(&map);
    Some((s, lbl))
}

/// `triSurfaceCleanupDuplicateTriangles`: remove repeated facets (see
/// [`same_labelled_tri`]). Returns the number removed.
pub fn cleanup_duplicate_triangles(surf: &mut TriSurf) -> usize {
    match remove_duplicate_triangles(surf) {
        Some((s, _)) => {
            let n = surf.size() - s.size();
            *surf = s;
            n
        }
        None => 0,
    }
}

/// What [`cleanup_duplicates`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct CleanupDuplicatesReport {
    /// `newTriangleLabel_`: each original facet's label in the cleaned
    /// surface, `None` if it was removed.
    pub new_triangle_label: Vec<Option<usize>>,
    /// Duplicate facets removed (all passes).
    pub n_duplicate_triangles: usize,
    /// Points merged (all passes).
    pub n_merged_points: usize,
    /// Facets removed because merging made them degenerate.
    pub n_degenerate_triangles: usize,
}

/// `triSurfaceCleanupDuplicates::mergeIdentities`: alternately remove
/// duplicate facets and merge points closer than `tol` (searched per leaf
/// of `octree`, which must be built on the surface being cleaned), until
/// neither changes anything. Returns the cleaned surface.
///
/// Port notes:
/// - Upstream's duplicate-facet step changes the facets **without**
///   clearing the cached addressing, so when it finds duplicates and the
///   point step then finds none, the next pass reads stale `pointFacets`
///   (labels of the old facet list). The port rebuilds the addressing after
///   every change.
/// - Upstream's point merge renumbers the facets and facet subsets only,
///   leaving the stored feature edges and point subsets pointing at the old
///   point labels. The port renumbers them too (a feature edge or subset
///   point that merged keeps its merged label).
pub fn cleanup_duplicates(octree: &MeshOctree, tol: f64) -> (TriSurf, CleanupDuplicatesReport) {
    let mut surf = (*octree.surface).clone();
    let mut ntl: Vec<Option<usize>> = (0..surf.size()).map(Some).collect();
    let mut rep = CleanupDuplicatesReport { new_triangle_label: Vec::new(), n_duplicate_triangles: 0, n_merged_points: 0, n_degenerate_triangles: 0 };
    loop {
        let mut finished = true;
        if let Some((s, lbl)) = remove_duplicate_triangles(&surf) {
            rep.n_duplicate_triangles += surf.size() - s.size();
            for l in ntl.iter_mut() {
                *l = l.map(|o| lbl[o]);
            }
            surf = s;
            finished = false;
        }
        if let Some((s, map, n_merged)) = merge_duplicate_points(octree, &surf, &ntl, tol) {
            rep.n_merged_points += n_merged;
            rep.n_degenerate_triangles += surf.size() - s.size();
            for l in ntl.iter_mut() {
                *l = l.and_then(|o| map[o]);
            }
            surf = s;
            finished = false;
        }
        if finished {
            break;
        }
    }
    rep.new_triangle_label = ntl;
    (surf, rep)
}

/// `mergeDuplicatePoints`: the merged surface, the facet renumbering and
/// the number of points merged; `None` if no two points are closer than
/// `tol`.
fn merge_duplicate_points(
    octree: &MeshOctree,
    surf: &TriSurf,
    ntl: &[Option<usize>],
    tol: f64,
) -> Option<(TriSurf, Vec<Option<usize>>, usize)> {
    let pts = &surf.points;
    let mut npl: Vec<usize> = (0..pts.len()).collect();
    let mut found = false;
    for l in 0..octree.number_of_leaves() {
        let mut set: BTreeSet<usize> = BTreeSet::new();
        for t in octree.contained_triangles(l) {
            if let Some(cur) = ntl[t] {
                set.extend(surf.triangles[cur]);
            }
        }
        let v: Vec<usize> = set.into_iter().collect();
        for i in 0..v.len() {
            for &j in &v[i + 1..] {
                if mag_sqr(pts[v[i]].sub(pts[j])) < tol * tol {
                    found = true;
                    npl[j] = v[i];
                }
            }
        }
    }
    if !found {
        return None;
    }
    let mut new_pts = Vec::new();
    for p in 0..pts.len() {
        if npl[p] == p {
            npl[p] = new_pts.len();
            new_pts.push(pts[p]);
        } else {
            npl[p] = npl[npl[p]];
        }
    }
    let n_merged = pts.len() - new_pts.len();
    let mut tris = Vec::new();
    let mut regions = Vec::new();
    let mut ntl_step = vec![None; surf.size()];
    for (t, tri) in surf.triangles.iter().enumerate() {
        let nt = [npl[tri[0]], npl[tri[1]], npl[tri[2]]];
        if nt[0] == nt[1] || nt[0] == nt[2] || nt[1] == nt[2] {
            continue;
        }
        ntl_step[t] = Some(tris.len());
        tris.push(nt);
        regions.push(surf.regions[t]);
    }
    let mut s = rebuild(surf, new_pts, tris, regions);
    s.facet_subsets.update(&ntl_step);
    s.feature_edges = surf.feature_edges.iter().map(|e| [npl[e[0]], npl[e[1]]]).collect();
    let pmap: Vec<Option<usize>> = npl.iter().map(|&p| Some(p)).collect();
    s.point_subsets.update(&pmap);
    Some((s, ntl_step, n_merged))
}

/// `triSurfaceRemoveFacets`: remove the facets of the selected patches and
/// facet subsets, with the points no remaining facet uses.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TriSurfaceRemoveFacets {
    /// `selectedEntities_`: patch and subset names.
    pub selected: Vec<String>,
}

impl TriSurfaceRemoveFacets {
    /// `selectFacetsInPatch`.
    pub fn select_facets_in_patch(&mut self, name: &str) {
        self.selected.push(name.to_string());
    }

    /// `selectFacetsInSubset`.
    pub fn select_facets_in_subset(&mut self, name: &str) {
        self.selected.push(name.to_string());
    }

    /// `markFacetsForRemoval`.
    pub fn mark_facets_for_removal(&self, surf: &TriSurf) -> Vec<bool> {
        mark_by_names(surf, &self.selected)
    }

    /// `removeFacets`. Points, facets, point and facet subsets, the stored
    /// feature edges (kept if both points survive and a facet of the edge
    /// survives) and the feature-edge subsets are renumbered; the selection
    /// is cleared.
    ///
    /// Port note: upstream finds the feature edges' surface edges through
    /// `pointEdges`/`edgeFacets` **after** replacing the facets, so it reads
    /// whichever addressing happens to be cached (the old surface's if it
    /// was computed before, the new one's if not), while indexing with old
    /// labels. The port uses the old surface's addressing, which is what the
    /// indexing assumes.
    pub fn remove_facets(&mut self, surf: &mut TriSurf) {
        let remove = self.mark_facets_for_removal(surf);
        let mut npl: Vec<Option<usize>> = vec![None; surf.n_points()];
        let mut nfl: Vec<Option<usize>> = vec![None; surf.size()];
        let (mut pc, mut fc) = (0, 0);
        for t in 0..surf.size() {
            if remove[t] {
                continue;
            }
            for &p in &surf.triangles[t] {
                if npl[p].is_none() {
                    npl[p] = Some(pc);
                    pc += 1;
                }
            }
            nfl[t] = Some(fc);
            fc += 1;
        }
        let mut pts = vec![crate::math::Vec3::ZERO; pc];
        for (p, n) in npl.iter().enumerate() {
            if let Some(n) = n {
                pts[*n] = surf.points[p];
            }
        }
        let mut tris = vec![[0usize; 3]; fc];
        let mut regions = vec![0usize; fc];
        for t in 0..surf.size() {
            if let Some(n) = nfl[t] {
                let tr = surf.triangles[t];
                tris[n] = [npl[tr[0]].unwrap_or(0), npl[tr[1]].unwrap_or(0), npl[tr[2]].unwrap_or(0)];
                regions[n] = surf.regions[t];
            }
        }
        let (fe, nfe) = renumber_feature_edges(surf, &npl, &nfl);
        let mut s = rebuild(surf, pts, tris, regions);
        s.point_subsets.update(&npl);
        s.facet_subsets.update(&nfl);
        s.feature_edges = fe;
        s.edge_subsets.update(&nfe);
        *surf = s;
        self.selected.clear();
    }
}

/// The stored feature edges that survive a point and facet renumbering:
/// both points kept, the edge on the surface, and one of its facets kept.
fn renumber_feature_edges(surf: &TriSurf, npl: &[Option<usize>], nfl: &[Option<usize>]) -> (Vec<[usize; 2]>, Vec<Option<usize>>) {
    let mut out = Vec::new();
    let mut map = vec![None; surf.feature_edges.len()];
    for (i, fe) in surf.feature_edges.iter().enumerate() {
        let (Some(a), Some(b)) = (npl[fe[0]], npl[fe[1]]) else { continue };
        let Some(e) = surf.find_edge(fe[0], fe[1]) else { continue };
        if !surf.edge_facets[e].iter().any(|&f| nfl[f].is_some()) {
            continue;
        }
        map[i] = Some(out.len());
        out.push([a, b]);
    }
    (out, map)
}

/// Facets of the patches and facet subsets named in `names`.
fn mark_by_names(surf: &TriSurf, names: &[String]) -> Vec<bool> {
    let sel_patch: Vec<bool> = surf.patches.iter().map(|p| names.contains(&p.name)).collect();
    let mut mark: Vec<bool> = surf.regions.iter().map(|&r| sel_patch[r]).collect();
    for id in surf.facet_subsets.indices() {
        if surf.facet_subsets.name(id).is_some_and(|n| names.iter().any(|s| s == n)) {
            for f in surf.facet_subsets.elements(id) {
                mark[f] = true;
            }
        }
    }
    mark
}

/// The `removeSurfaceFacets` utility (file I/O not ported): remove the
/// facets of one patch (or facet subset).
pub fn remove_surface_facets(surf: &mut TriSurf, patch_name: &str) {
    let mut r = TriSurfaceRemoveFacets::default();
    r.select_facets_in_patch(patch_name);
    r.remove_facets(surf);
}

/// `triSurfaceCopyParts::copySurface(parts)`: a new surface holding the
/// facets of the named patches and facet subsets, points renumbered in
/// order of first use, the **whole** patch list, and the subsets and
/// feature edges restricted to what was copied.
///
/// Port note: upstream reads `edgeFacets(eI)` with `eI = -1` for a stored
/// feature edge that is not a surface edge; the port skips such an edge.
pub fn copy_surface_parts(surf: &TriSurf, parts: &[String]) -> TriSurf {
    let copy = mark_by_names(surf, parts);
    let mut npl: Vec<Option<usize>> = vec![None; surf.n_points()];
    let mut nfl: Vec<Option<usize>> = vec![None; surf.size()];
    let mut n_points = 0;
    let mut tris = Vec::new();
    let mut regions = Vec::new();
    for t in 0..surf.size() {
        if !copy[t] {
            continue;
        }
        let tr = surf.triangles[t];
        let mut nt = [0usize; 3];
        for i in 0..3 {
            if npl[tr[i]].is_none() {
                npl[tr[i]] = Some(n_points);
                n_points += 1;
            }
            nt[i] = npl[tr[i]].unwrap_or(0);
        }
        nfl[t] = Some(tris.len());
        tris.push(nt);
        regions.push(surf.regions[t]);
    }
    let mut pts = vec![crate::math::Vec3::ZERO; n_points];
    for (p, n) in npl.iter().enumerate() {
        if let Some(n) = n {
            pts[*n] = surf.points[p];
        }
    }
    let (fe, nfe) = renumber_feature_edges(surf, &npl, &nfl);
    let mut s = rebuild(surf, pts, tris, regions);
    s.point_subsets.update(&npl);
    s.facet_subsets.update(&nfl);
    s.feature_edges = fe;
    s.edge_subsets.update(&nfe);
    s
}

/// `triSurfaceImportSurfaceAsSubset::addSurfaceAsSubset(importSurf,
/// subsetName, angleTol)`: mark as the facet subset `subset_name` the facets
/// of `surf` that `import` lies on.
///
/// Two passes, as upstream: each imported facet's centre is matched to the
/// nearest facet of `surf` (octree `findNearestSurfacePoint`), then each
/// facet of `surf` to the nearest imported facet in its box; a match counts
/// when its squared distance is at most `0.09 ×` the longest squared edge
/// and the two unit normals satisfy `n·m > angle_tol`.
///
/// Note: upstream's default `angle_tol` is `5π/180 ≈ 0.087`, compared with
/// a **cosine**, so the effective rule is "normals within about 85°".
/// Ported as written; upstream's default is [`IMPORT_SUBSET_ANGLE_TOL`].
pub fn import_surface_as_subset(surf: &mut TriSurf, import: &TriSurf, subset_name: &str, angle_tol: f64) {
    use super::checks::search_octree;
    let octree = search_octree(surf, 20, 15);
    let fn_surf = surf.facet_normals();
    let fc_surf = surf.facet_centres();
    let fc_imp = import.facet_centres();
    let fn_imp = import.facet_normals();
    let longest = |s: &TriSurf, t: usize| {
        let tr = s.triangles[t];
        (0..3).map(|i| mag_sqr(s.points[tr[(i + 1) % 3]].sub(s.points[tr[i]]))).fold(0.0, f64::max)
    };
    let mut nearest: Vec<Option<usize>> = vec![None; import.size()];
    for t in 0..import.size() {
        let Some(hit) = octree.find_nearest_surface_point(fc_imp[t]) else { continue };
        if hit.dist_sq > 0.09 * longest(import, t) {
            continue;
        }
        let n_tri = fn_imp[t];
        let m1 = mag_sqr(n_tri);
        if m1 < VSMALL {
            continue;
        }
        let normal = fn_surf[hit.triangle];
        let m2 = mag_sqr(normal);
        if m2 < VSMALL {
            continue;
        }
        if n_tri.dot(normal) / (m1 * m2) > angle_tol {
            nearest[t] = Some(hit.triangle);
        }
    }
    let other = search_octree(import, 20, 15);
    for t in 0..surf.size() {
        let tr = surf.triangles[t];
        let mut bb = BoundBox::new(fc_surf[t], fc_surf[t]);
        for &p in &tr {
            let q = surf.points[p];
            bb.min = crate::math::Vec3::new(bb.min.x.min(q.x), bb.min.y.min(q.y), bb.min.z.min(q.z));
            bb.max = crate::math::Vec3::new(bb.max.x.max(q.x), bb.max.y.max(q.y), bb.max.z.max(q.z));
        }
        let max_edge = longest(surf, t);
        let mut nt = None;
        let mut d_sq = VGREAT;
        for c in other.find_triangles_in_box(&bb) {
            let it = import.triangles[c];
            let p = help::nearest_point_on_the_triangle(import.points[it[0]], import.points[it[1]], import.points[it[2]], fc_surf[t]);
            let d = mag_sqr(p.sub(fc_surf[t]));
            if d < d_sq {
                nt = Some(c);
                d_sq = d;
            }
        }
        let Some(nt) = nt else { continue };
        if d_sq > 0.09 * max_edge || nearest[nt] == Some(t) {
            continue;
        }
        let n_tri = fn_surf[t];
        let m1 = mag_sqr(n_tri);
        if m1 < VSMALL {
            continue;
        }
        let normal = fn_imp[nt];
        let m2 = mag_sqr(normal);
        if m2 < VSMALL {
            continue;
        }
        if n_tri.dot(normal) / (m1 * m2) > angle_tol {
            nearest[nt] = Some(t);
        }
    }
    let id = surf.facet_subsets.add(subset_name);
    for n in nearest.into_iter().flatten() {
        surf.facet_subsets.add_element(id, n);
    }
}

/// Upstream's default `angleTol` of `addSurfaceAsSubset`, `5π/180`.
pub const IMPORT_SUBSET_ANGLE_TOL: f64 = 5.0 * std::f64::consts::PI / 180.0;

/// `getPatchIds`: the ids of the patches named in `names`, in patch order.
/// The second value is `false` where upstream warns that not every name
/// was found.
pub fn patch_ids(surf: &TriSurf, names: &[String]) -> (Vec<usize>, bool) {
    let ids: Vec<usize> = (0..surf.patches.len()).filter(|&p| names.contains(&surf.patches[p].name)).collect();
    let all = ids.len() == names.len();
    (ids, all)
}

/// The `mergeSurfacePatches` utility's `mergeSurfacePatches` (file I/O not
/// ported): move the facets of `patch_ids` onto a new last patch
/// `new_name` (geometric type of the first merged patch), dropping the
/// patches left empty unless `keep_patches`. Facets are reordered patch by
/// patch; facet subsets follow them.
///
/// Port notes:
/// - Upstream copies the subsets by **loop index** (`facetSubsetName(i)`)
///   instead of by subset id, which is wrong once an id is missing (a
///   removed subset); the port copies by id.
/// - A kept empty patch is default-constructed upstream; here it keeps its
///   original name and type (not re-checked against `geometricSurfacePatch`'s
///   default constructor, which is not vendored).
///
/// # Panics
///
/// If `patch_ids` is empty or names a patch out of range (upstream exits
/// with "No patches specified").
pub fn merge_surface_patches(surf: &TriSurf, patch_ids: &[usize], new_name: &str, keep_patches: bool) -> TriSurf {
    assert!(!patch_ids.is_empty(), "No patches specified");
    let np = surf.patches.len();
    let new_type = surf.patches[patch_ids[0]].geometric_type.clone();
    let mut addr: Vec<Vec<usize>> = vec![Vec::new(); np + 1];
    for (f, &r) in surf.regions.iter().enumerate() {
        addr[r].push(f);
    }
    for &p in patch_ids {
        let moved = std::mem::take(&mut addr[p]);
        addr[np].extend(moved);
    }
    let mut tris = Vec::with_capacity(surf.size());
    let mut regions = Vec::with_capacity(surf.size());
    let mut nfa = vec![None; surf.size()];
    let mut patches = Vec::new();
    for (p, a) in addr.iter().enumerate() {
        if a.is_empty() && !keep_patches {
            continue;
        }
        let pc = patches.len();
        for &f in a {
            nfa[f] = Some(tris.len());
            tris.push(surf.triangles[f]);
            regions.push(pc);
        }
        patches.push(if p < np { surf.patches[p].clone() } else { SurfacePatch { name: new_name.to_string(), geometric_type: new_type.clone() } });
    }
    let mut s = TriSurf::from_parts(surf.points.clone(), tris, regions, patches, surf.feature_edges.clone());
    s.facet_subsets = surf.facet_subsets.clone();
    s.facet_subsets.update(&nfa);
    s.edge_subsets = surf.edge_subsets.clone();
    s.point_subsets = surf.point_subsets.clone();
    s
}
