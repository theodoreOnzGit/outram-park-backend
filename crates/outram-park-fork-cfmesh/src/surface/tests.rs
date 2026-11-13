// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Tests of the cfMesh surface-tool ports (gh:#1276, gh:#1277). Original
// OUTRAM PARK test code; the routines under test are ported from cfMesh
// (DAFoam/cfmesh e944f6ba695f), see `surface/*.rs`.
//
// This file is part of OUTRAM PARK, GPL-3.0-only (see the crate's LICENSE).

//! V&V for [`crate::surface`]. Every prediction below was written and
//! committed before the first run (gh:#1276/#1277 commits); results are in
//! `verification_and_validation/surface_tools_gh1276/README.md`.

use super::checks::*;
use super::cleanup::*;
use super::features::*;
use crate::math::Vec3;
use crate::mesh_octree::{MeshOctree, SurfacePatch, TriSurf};
use crate::mesh_octree_creator::{MeshDict, MeshOctreeCreator};
use crate::shapes::{box_surface, cylinder_surface, surface_volume};
use std::collections::BTreeSet;
use std::sync::Arc;

/// The unlabelled pipe: R 1, L 4, 48 segments, fan caps.
pub(crate) fn pipe48() -> TriSurf {
    let (p, t) = cylinder_surface(Vec3::ZERO, 1.0, 4.0, 48);
    TriSurf::new(p, t, None)
}

/// The box [-1, 1]^3 with one region per face (`patch0` … `patch5`).
pub(crate) fn box6() -> TriSurf {
    let (p, t) = box_surface(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    let r = (0..t.len()).map(|i| i / 2).collect();
    TriSurf::new(p, t, Some(r))
}

/// An L-shaped prism (reflex corner at the origin), caps fanned from the
/// reflex vertex, height 1.
pub(crate) fn l_prism() -> TriSurf {
    let poly = [(0.0, 0.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (1.0, 0.0)];
    let n = poly.len();
    let mut p: Vec<Vec3> = poly.iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect();
    p.extend(poly.iter().map(|&(x, y)| Vec3::new(x, y, 1.0)));
    // The polygon is counter-clockwise seen from +z (area +3), so the
    // bottom fan (outward −z) runs 0, i+1, i, the top fan 0, i, i+1, and a
    // side quad i, j, n+j / i, n+j, n+i.
    let mut t = Vec::new();
    for i in 1..n - 1 {
        t.push([0, i + 1, i]);
        t.push([n, n + i, n + i + 1]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        t.push([i, j, n + j]);
        t.push([i, n + j, n + i]);
    }
    let s = TriSurf::new(p, t, None);
    assert!(surface_volume(&s.points, &s.triangles) > 0.0, "L prism wound outward");
    s
}

/// Endpoints of an edge as a sorted key of rounded coordinates.
fn key(a: Vec3, b: Vec3) -> [(i64, i64, i64); 2] {
    let k = |v: Vec3| ((v.x * 1e9).round() as i64, (v.y * 1e9).round() as i64, (v.z * 1e9).round() as i64);
    let (x, y) = (k(a), k(b));
    if x <= y {
        [x, y]
    } else {
        [y, x]
    }
}

/// `outram-foam-mesh`'s `detect_feature_edges` on the same surface.
fn foam_mesh_features(s: &TriSurf, angle: f64) -> BTreeSet<[(i64, i64, i64); 2]> {
    use outram_foam_basic_lib::primitives::Vector3;
    use outram_foam_mesh::snappy_hex_mesh::snapping::detect_feature_edges;
    use outram_foam_mesh::snappy_hex_mesh::stl::{Triangle, TriangleSoup};
    let v = |p: Vec3| Vector3::new(p.x, p.y, p.z);
    let tris = s.triangles.iter().map(|t| Triangle::new(v(s.points[t[0]]), v(s.points[t[1]]), v(s.points[t[2]]))).collect();
    let soup = TriangleSoup::new("s", tris);
    detect_feature_edges(&soup, angle)
        .iter()
        .map(|e| key(Vec3::new(e.a.x, e.a.y, e.a.z), Vec3::new(e.b.x, e.b.y, e.b.z)))
        .collect()
}

/// The port's feature edges as the same keys.
fn cfmesh_features(s: &TriSurf, angle: f64) -> BTreeSet<[(i64, i64, i64); 2]> {
    let mut c = s.clone();
    TriSurfaceDetectFeatureEdges::new(&c, angle).detect_feature_edges(&mut c);
    c.feature_edges.iter().map(|e| key(c.points[e[0]], c.points[e[1]])).collect()
}

/// V&V (gh:#1276) — the item's test. Predictions (written before the first
/// run): at 45° the unlabelled 48-sided pipe has exactly 2 × 48 = 96
/// feature edges (the rims; side edges turn by 7.5°, cap and quad
/// diagonals by 0°) and splits into exactly 3 patches, in facet order
/// bottom cap (48 facets, `patch_0`), top cap (48, `patch_1`), wall
/// (~~192~~ 96, `patch_2`); at 5° each side quad becomes its own patch:
/// 48 + 2 = 50.
///
/// **Result (2026-11-05): 3 patches, sizes [48, 48, 96]; 96 feature edges;
/// 50 patches at 5°.** The wall prediction of 192 was my arithmetic error
/// (192 is the whole surface: 48 segments × 4 triangles, 2 of them on the
/// wall), not a port defect; corrected after the run.
#[test]
fn unlabelled_pipe_splits_into_three_patches() {
    let s = pipe48();
    let d = TriSurfaceDetectFeatureEdges::new(&s, 45.0);
    let n_angle = d.feature_edges.iter().filter(|&&f| f & FeatureFlag::ANGLE != 0).count();
    let n_open = d.feature_edges.iter().filter(|&&f| f & FeatureFlag::NOT_MANIFOLD != 0).count();
    assert_eq!((n_angle, n_open), (96, 0));
    let out = surface_feature_edges(&s, 45.0, FeatureEdgesOutput::SplitIntoPatches);
    assert_eq!(out.patches.len(), 3);
    let mut sizes = vec![0usize; 3];
    for &r in &out.regions {
        sizes[r] += 1;
    }
    assert_eq!(sizes, vec![48, 48, 96]);
    assert_eq!(out.patch_names(), vec!["patch_0", "patch_1", "patch_2"]);
    // Every wall facet spans z; every cap facet is flat.
    for (f, t) in out.triangles.iter().enumerate() {
        let z: Vec<f64> = t.iter().map(|&p| out.points[p].z).collect();
        let flat = z.iter().all(|&v| v == z[0]);
        assert_eq!(flat, out.regions[f] != 2, "facet {f}");
    }
    // Stored-feature output keeps the facets and stores the 96 rims.
    let fms = surface_feature_edges(&s, 45.0, FeatureEdgesOutput::StoreFeatureEdges);
    assert_eq!((fms.feature_edges.len(), fms.size()), (96, s.size()));
    let fine = surface_feature_edges(&s, 5.0, FeatureEdgesOutput::SplitIntoPatches);
    assert_eq!(fine.patches.len(), 50);
}

/// V&V (gh:#1276) — differential against `outram-foam-mesh`'s
/// `detect_feature_edges` (same angle, same surface; compared as sets of
/// endpoint pairs). Predictions (before the first run):
/// - pipe at 45° and 5°: identical sets (96 and 96 + 48 = 144);
/// - box at 45°: identical, 12;
/// - box with facet 4 flipped: the port 12 (upstream negates the cosine of
///   an inconsistently oriented pair, so the flipped facet's coplanar
///   partner is not a crease), `outram-foam-mesh` 13 (that diagonal reads
///   as a 180° fold);
/// - pipe with the bottom cap removed: the port 96 (48 top rims by angle +
///   48 open edges, which upstream stores as feature edges too),
///   `outram-foam-mesh` 48 (it skips open edges); the 48 shared are equal.
#[test]
fn feature_edges_against_outram_foam_mesh() {
    let pipe = pipe48();
    for a in [45.0, 5.0] {
        let (c, f) = (cfmesh_features(&pipe, a), foam_mesh_features(&pipe, a));
        assert_eq!(c, f, "pipe at {a}");
        assert_eq!(c.len(), if a == 45.0 { 96 } else { 144 });
    }
    let bx = box6();
    let (c, f) = (cfmesh_features(&bx, 45.0), foam_mesh_features(&bx, 45.0));
    assert_eq!((c.len(), c == f), (12, true));
    let mut flipped = bx.clone();
    flipped.triangles[4].swap(1, 2);
    let flipped = TriSurf::new(flipped.points.clone(), flipped.triangles.clone(), Some(flipped.regions.clone()));
    let (c, f) = (cfmesh_features(&flipped, 45.0), foam_mesh_features(&flipped, 45.0));
    assert_eq!((c.len(), f.len()), (12, 13));
    assert!(c.is_subset(&f));
    // Open pipe: drop the bottom cap (z = 0 flat facets).
    let keep: Vec<[usize; 3]> = pipe.triangles.iter().copied().filter(|t| !t.iter().all(|&p| pipe.points[p].z == 0.0)).collect();
    let open = TriSurf::new(pipe.points.clone(), keep, None);
    let (c, f) = (cfmesh_features(&open, 45.0), foam_mesh_features(&open, 45.0));
    assert_eq!((c.len(), f.len()), (96, 48));
    assert!(f.is_subset(&c));
}

/// V&V (gh:#1276) — `triSurfacePartitioner`. Predictions: the six-patch
/// box has 8 corners (3 patches each), 12 edge groups (one per box edge),
/// every patch borders 4 others, every group meets 4 others at its two
/// corners, 12 patch pairs with one group each and 24 group pairs with one
/// corner each. The labelled pipe (wall 0, caps 1, 2) has no corner, 2
/// edge groups (the rims), patch pairs (0,1) and (0,2).
#[test]
fn partitioner_on_box_and_pipe() {
    let p = TriSurfacePartitioner::new(&box6());
    assert_eq!(p.corners.len(), 8);
    assert!(p.corner_patches.iter().all(|c| c.len() == 3));
    assert_eq!(p.edge_group_edge_groups.len(), 12);
    assert!(p.patch_patches.iter().all(|s| s.len() == 4));
    assert!(p.edge_group_edge_groups.iter().all(|s| s.len() == 4));
    assert_eq!(p.patches_edge_groups.len(), 12);
    assert!(p.patches_edge_groups.values().all(|s| s.len() == 1));
    assert_eq!(p.edge_groups_corners.len(), 24);
    assert!(p.edge_groups_corners.values().all(|s| s.len() == 1));
    assert_eq!(p.edge_groups_shared_by_patches(1, 0).len(), 0, "-Z and +Z do not touch");
    let g = p.edge_groups_shared_by_patches(0, 2);
    assert_eq!(g.len(), 1);
    let pipe = crate::mesh_octree_queries::tests::pipe(1.0, 4.0, 48);
    let q = TriSurfacePartitioner::new(&pipe);
    assert_eq!(q.corners.len(), 0);
    assert_eq!(q.edge_group_edge_groups.len(), 2);
    assert_eq!(q.patches_edge_groups.keys().copied().collect::<Vec<_>>(), vec![(0, 1), (0, 2)]);
}

/// Classify with an octree whose leaves are typed (`createOctreeBoxes`).
fn classify(s: TriSurf, max_cell: f64) -> TriSurfaceClassifyEdges {
    let mut o = MeshOctree::new(Arc::new(s));
    let mut c = MeshOctreeCreator::new(&o, Some(MeshDict::with_max_cell_size(max_cell)));
    c.create_octree_boxes(&mut o);
    TriSurfaceClassifyEdges::new(&o)
}

fn count(types: &[u8], bit: u8) -> usize {
    types.iter().filter(|&&t| t & bit != 0).count()
}

/// V&V (gh:#1276) — `triSurfaceClassifyEdges`. Predictions (before the
/// first run), octree `maxCellSize` 0.25:
/// - box, outward: 1 orientation group, every facet outward (1); 12 convex,
///   6 flat (diagonals), 0 concave, 12 `FEATURE` (patch boundaries);
/// - the same box wound inward: every facet inward (2), the same 12/6/0;
/// - L prism (one region): 17 convex (5 vertical + 12 rim), 1 concave (the
///   reflex vertical edge), 12 flat (6 cap diagonals + 6 side diagonals),
///   0 `FEATURE`.
#[test]
fn classify_edges_box_and_l_prism() {
    let c = classify(box6(), 0.25);
    assert_eq!(c.n_orientation_groups, 1);
    assert!(c.facet_orientation.iter().all(|&o| o == 1));
    let t = &c.edge_types;
    assert_eq!((count(t, EdgeType::CONVEX), count(t, EdgeType::FLAT), count(t, EdgeType::CONCAVE), count(t, EdgeType::FEATURE)), (12, 6, 0, 12));
    let mut inward = box6();
    for tr in inward.triangles.iter_mut() {
        tr.swap(1, 2);
    }
    let inward = TriSurf::new(inward.points.clone(), inward.triangles.clone(), Some(inward.regions.clone()));
    let c = classify(inward, 0.25);
    assert!(c.facet_orientation.iter().all(|&o| o == 2));
    let t = &c.edge_types;
    assert_eq!((count(t, EdgeType::CONVEX), count(t, EdgeType::FLAT), count(t, EdgeType::CONCAVE)), (12, 6, 0));
    let c = classify(l_prism(), 0.25);
    let t = &c.edge_types;
    assert_eq!((count(t, EdgeType::CONVEX), count(t, EdgeType::CONCAVE), count(t, EdgeType::FLAT), count(t, EdgeType::FEATURE)), (17, 1, 12, 0));
}

/// V&V (gh:#1276) — `patchesToSubsets` then `subsetToPatch` is the
/// identity (item 26's test, here because both utilities are in item 22's
/// list), and the `symmetryPlane` split is refused. Predictions: regions
/// and patch names unchanged, all six subsets gone, 6 names converted;
/// a pipe whose one patch is a `symmetryPlane` split at 45° is an `Err`.
#[test]
fn patches_to_subsets_and_back() {
    let orig = box6();
    let mut s = orig.clone();
    patches_to_subsets(&mut s);
    assert_eq!(s.facet_subsets.indices().len(), 6);
    assert_eq!(s.facet_subsets.elements(s.facet_subsets.index("patch3").unwrap()), vec![6, 7]);
    let names = subset_to_patch(&mut s, "patch");
    assert_eq!(names.len(), 6);
    assert_eq!(s.regions, orig.regions);
    assert_eq!(s.patch_names(), orig.patch_names());
    assert!(s.facet_subsets.indices().is_empty());
    // A subset carved out of one patch becomes a new patch.
    let mut t = orig.clone();
    let id = t.facet_subsets.add("lid");
    t.facet_subsets.add_element(id, 2);
    subset_to_patch(&mut t, "lid");
    assert_eq!(t.patches.len(), 7);
    assert_eq!(t.regions[2], 6);
    // symmetryPlane split.
    let mut p = pipe48();
    p.patches[0].geometric_type = "symmetryPlane".into();
    TriSurfaceDetectFeatureEdges::new(&p.clone(), 45.0).detect_feature_edges(&mut p);
    let m = TriSurfacePatchManipulator::new(&p);
    assert_eq!(m.n_patches, 3);
    assert!(m.patches_for_patch(&p).is_err());
    p.patches[0].geometric_type = "wall".into();
    let map = m.patches_for_patch(&p).unwrap();
    assert_eq!(map["patch"], vec!["patch_0", "patch_1", "patch_2"]);
}

/// The defect-seeded box of gh:#1277 (see the test).
fn seeded_box() -> TriSurf {
    let b = box6();
    let mut p = b.points.clone();
    let mut t = b.triangles.clone();
    let mut r = b.regions.clone();
    // 1. Duplicate point: a copy of point 6, used by the first facet that
    //    uses 6.
    p.push(p[6]);
    let k = t.iter().position(|tr| tr.contains(&6)).unwrap();
    for v in t[k].iter_mut() {
        if *v == 6 {
            *v = 8;
        }
    }
    // 2. Duplicate facet: a copy of facet 0.
    t.push(t[0]);
    r.push(r[0]);
    // 3. Flipped facet: facet 4 (on the -Y face).
    t[4].swap(1, 2);
    // 4. Stray part: one small triangle far away, in region 0.
    let n = p.len();
    p.push(Vec3::new(5.0, 5.0, 5.0));
    p.push(Vec3::new(5.1, 5.0, 5.0));
    p.push(Vec3::new(5.0, 5.1, 5.0));
    t.push([n, n + 1, n + 2]);
    r.push(0);
    let mut s = TriSurf::new(p, t, Some(r));
    s.patches = b.patches.clone();
    s
}

/// The port's own facet flip (not upstream: cfMesh has no tool that
/// reorients facets): flip every facet outside the largest consistently
/// oriented group.
fn flip_minority_orientation(s: &mut TriSurf) -> usize {
    let (g, n) = check_orientation(s);
    let mut size = vec![0usize; n];
    for x in g.iter().flatten() {
        size[*x] += 1;
    }
    let big = (0..n).max_by_key(|&i| (size[i], std::cmp::Reverse(i))).unwrap_or(0);
    let mut tris = s.triangles.clone();
    let mut flipped = 0;
    for (f, gi) in g.iter().enumerate() {
        if *gi != Some(big) {
            tris[f].swap(1, 2);
            flipped += 1;
        }
    }
    let mut ns = super::rebuild(s, s.points.clone(), tris, s.regions.clone());
    ns.patches = s.patches.clone();
    *s = ns;
    flipped
}

/// V&V (gh:#1277) — the item's test. A box with seeded defects: a
/// duplicated point (point 8 = point 6, used by facet k), a duplicated
/// facet (copy of facet 0), a flipped facet (4), a stray part (one
/// triangle at (5, 5, 5)); open edges come from the duplicated point and
/// the stray triangle. Predictions (before the first run):
/// - holes: 7 entries (4 open edges around point 8, 3 on the stray);
/// - non-manifold: 9 entries (facet 0's 3 edges × 3 facets);
/// - disconnected parts 2; collocated points `[6, 8]`;
/// - orientation groups: 5 ({0}, {its copy}, {4}, {stray}, the rest);
/// - `cleanup_duplicates` (tol 1e-6): 1 duplicate facet, 1 merged point,
///   0 degenerate facets; then holes 3, non-manifold 0;
/// - the port's flip (not upstream) flips facet 4 only (orientation groups
///   after the merge: the box and the stray, plus {4}: the stray is
///   oriented on its own and is the smaller group, so it is flipped too:
///   2 facets flipped);
/// - `checkDisconnectedParts` → subsets `region_0` (box), `region_1`
///   (stray); removing `region_1` leaves 12 facets, 8 points;
/// - then: no holes, no non-manifold edges, 1 orientation group, 1 part,
///   no collocated points, and the volume is **exactly 8**.
#[test]
fn seeded_box_is_cleaned_to_volume_eight() {
    let mut s = seeded_box();
    assert_eq!(check_for_holes(&s).len(), 7);
    assert_eq!(check_for_non_manifold_edges(&s).len(), 9);
    assert_eq!(check_disconnected_parts(&s).1, 2);
    assert_eq!(check_collocated_points(&s, 1e-6), vec![6, 8]);
    assert_eq!(check_orientation(&s).1, 5);

    let mut o = MeshOctree::new(Arc::new(s.clone()));
    MeshOctreeCreator::new(&o, None).create_octree_with_refined_boundary(&mut o, 20, 2);
    let (c, rep) = cleanup_duplicates(&o, 1e-6);
    assert_eq!((rep.n_duplicate_triangles, rep.n_merged_points, rep.n_degenerate_triangles), (1, 1, 0));
    s = c;
    assert_eq!(check_for_holes(&s).len(), 3);
    assert_eq!(check_for_non_manifold_edges(&s).len(), 0);

    assert_eq!(flip_minority_orientation(&mut s), 2);

    assert_eq!(check_disconnected_parts_subset(&mut s, "region_"), 2);
    let mut rf = TriSurfaceRemoveFacets::default();
    rf.select_facets_in_subset("region_1");
    rf.remove_facets(&mut s);
    assert_eq!((s.size(), s.n_points()), (12, 8));

    assert!(check_for_holes(&s).is_empty());
    assert!(check_for_non_manifold_edges(&s).is_empty());
    assert_eq!(check_orientation(&s).1, 1);
    assert_eq!(check_disconnected_parts(&s).1, 1);
    assert!(check_collocated_points(&s, 1e-6).is_empty());
    assert_eq!(surface_volume(&s.points, &s.triangles), 8.0);
    // Differential: the regions still validate as SurfaceRegions.
    assert!(s.surface_regions().validate(s.size()).is_ok());
}

/// V&V (gh:#1277) — the orientation check against the volume sign
/// (`shapes::surface_volume`) and `SurfaceRegions::validate`. Predictions:
/// box outward 1 group, volume +8; wound inward 1 group, volume −8; one
/// facet flipped 2 groups, volume 8 − 2·(that facet's term) ≠ ±8; the
/// region labelling validates in all three.
#[test]
fn orientation_against_volume_sign() {
    let b = box6();
    assert_eq!((check_orientation(&b).1, surface_volume(&b.points, &b.triangles)), (1, 8.0));
    let inv: Vec<[usize; 3]> = b.triangles.iter().map(|t| [t[0], t[2], t[1]]).collect();
    let i = TriSurf::new(b.points.clone(), inv, Some(b.regions.clone()));
    assert_eq!((check_orientation(&i).1, surface_volume(&i.points, &i.triangles)), (1, -8.0));
    let mut one = b.triangles.clone();
    one[4].swap(1, 2);
    let f = TriSurf::new(b.points.clone(), one, Some(b.regions.clone()));
    assert_eq!(check_orientation(&f).1, 2);
    assert!(surface_volume(&f.points, &f.triangles).abs() != 8.0);
    for s in [&b, &i, &f] {
        assert!(s.surface_regions().validate(s.size()).is_ok());
    }
}

/// V&V (gh:#1277) — `checkSurfaceMesh` on the clean box and the pipe.
/// Predictions: no holes, no non-manifold edges, 1 manifold, 1 part, no
/// small (near-180°) angles, no collocated points, no self-intersections.
/// Overlaps: I expect upstream's `doTrianglesOverlap` to report every
/// coplanar facet pair sharing an edge (the clipped polygon degenerates to
/// the shared edge, which is kept because its signed distances are 0 and
/// the test is `>= 0`), so 12 of 12 box facets flagged and `n_failed` 1;
/// on the pipe the cap fans and the side quads are coplanar pairs too, so
/// every facet is flagged (288). Recorded either way.
///
/// **Result (2026-11-05): MISSED.** The box gives 0 overlapping facets and
/// `n_failed` 0 ("Surface passes all checks"): the clip of a coplanar,
/// same-wound neighbour sharing an edge comes out empty, so the degenerate
/// polygon I predicted does not survive. (288 was also wrong as a count:
/// the pipe has 192 facets.) Re-predicted after the box run, before the
/// pipe's: 0 on the pipe too, `n_failed` 0. Consistent with
/// `triangle_pair_helpers`: for two same-wound coplanar facets upstream's
/// overlap test does not report even a genuine overlap.
#[test]
fn check_surface_mesh_on_clean_surfaces() {
    let mut b = box6();
    let r = check_surface_mesh(&mut b);
    eprintln!("box: {r:?}");
    assert_eq!((r.n_hole_facets, r.n_non_manifold_facets, r.n_manifolds, r.n_disconnected_parts), (0, 0, 1, 1));
    assert_eq!((r.n_small_angle_facets, r.n_collocated_points, r.n_self_intersecting_facets), (0, 0, 0));
    assert_eq!(r.n_orientation_groups, 1);
    assert_eq!(r.n_overlapping_facets, 0);
    assert_eq!(r.n_failed, 0);
    let mut p = pipe48();
    let r = check_surface_mesh(&mut p);
    eprintln!("pipe: {r:?}");
    assert_eq!((r.n_hole_facets, r.n_non_manifold_facets, r.n_self_intersecting_facets), (0, 0, 0));
    assert_eq!((r.n_overlapping_facets, r.n_failed), (0, 0));
}

/// V&V (gh:#1277) — `doTrianglesIntersect` / `doTrianglesOverlap` on
/// hand-made pairs. Predictions: two triangles crossing like a plus sign
/// intersect; the same pair moved 2 apart do not; perpendicular triangles
/// do not overlap. For t0 = (0,0),(1,0),(0,1) and t1 = (0.5,0),(1.5,0),
/// (0.5,1) in one plane the true overlap is the triangle (0.5,0),(1,0),
/// (0.5,0.5), area 0.125.
/// ~~First written: the overlap polygon has area 0.125.~~ **Revised before
/// the first run**, on re-reading the clipping sign: upstream keeps the
/// points with `distance = v.y·p.x − v.x·p.y ≥ 0`, which is the **outside**
/// of an edge of a t1 wound counter-clockwise about the common normal
/// `vec` (the frame `x, y = vec × x` is right-handed). So for t0 and t1
/// wound the same way the clip is not the overlap: predicted `None`; with
/// t1 reversed (clockwise in the frame) predicted area 0.125.
#[test]
fn triangle_pair_helpers() {
    let t0 = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
    let x = [Vec3::new(0.2, 0.2, -0.5), Vec3::new(0.2, 0.2, 0.5), Vec3::new(0.6, -0.5, 0.0)];
    assert!(do_triangles_intersect(t0, x, 1e-9));
    let far = x.map(|p| p.add(Vec3::new(0.0, 0.0, 2.0)));
    assert!(!do_triangles_intersect(t0, far, 1e-9));
    let t1 = [Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0), Vec3::new(0.5, 1.0, 0.0)];
    let same = do_triangles_overlap(t0, t1, 1e-9, 0.99);
    eprintln!("same winding: {same:?}");
    assert!(same.is_none());
    let t1r = [t1[0], t1[2], t1[1]];
    let poly = do_triangles_overlap(t0, t1r, 1e-9, 0.99).expect("overlap");
    let mut a = Vec3::ZERO;
    for i in 1..poly.len().saturating_sub(1) {
        a = a.add(poly[i].sub(poly[0]).cross(poly[i + 1].sub(poly[0])).scale(0.5));
    }
    assert!((a.length() - 0.125).abs() < 1e-12, "overlap area {}", a.length());
    let perp = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    assert!(do_triangles_overlap(t0, perp, 1e-9, 0.99).is_none());
}

/// V&V (gh:#1277) — the other tools. Predictions:
/// - `cleanup_duplicate_triangles` removes a copy and a reversed copy (2);
/// - removing `patch5` (−X) leaves 10 facets, 8 points (all still used),
///   4 open edges, and the 12 stored box feature edges (each keeps a
///   facet);
/// - copying `patch0` gives 2 facets, 4 points, all 6 patches listed;
/// - importing the +Z face (re-triangulated into 8 facets) marks exactly
///   the box's two +Z facets, 2 and 3;
/// - merging patches 0 and 1 into `zfaces` gives 5 patches, the last
///   `zfaces` with 4 facets, and a facet subset on facet 0 follows it to
///   its new label 8.
#[test]
fn removal_copy_import_merge() {
    let b = box6();
    let mut d = b.clone();
    d.triangles.push(b.triangles[0]);
    d.triangles.push([b.triangles[1][0], b.triangles[1][2], b.triangles[1][1]]);
    d.regions.extend([b.regions[0], b.regions[1]]);
    let mut d = TriSurf::new(d.points.clone(), d.triangles.clone(), Some(d.regions.clone()));
    assert_eq!(cleanup_duplicate_triangles(&mut d), 2);
    assert_eq!(d.size(), 12);

    let mut r = b.clone();
    TriSurfaceDetectFeatureEdges::new(&b, 45.0).detect_feature_edges(&mut r);
    assert_eq!(r.feature_edges.len(), 12);
    remove_surface_facets(&mut r, "patch5");
    assert_eq!((r.size(), r.n_points(), check_for_holes(&r).len(), r.feature_edges.len()), (10, 8, 4, 12));

    let c = copy_surface_parts(&b, &["patch0".to_string()]);
    assert_eq!((c.size(), c.n_points(), c.patches.len()), (2, 4, 6));

    let mut s = b.clone();
    let mut ip = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            ip.push(Vec3::new(-1.0 + i as f64, -1.0 + j as f64, 1.0));
        }
    }
    let mut it = Vec::new();
    for j in 0..2 {
        for i in 0..2 {
            let a = j * 3 + i;
            it.push([a, a + 1, a + 4]);
            it.push([a, a + 4, a + 3]);
        }
    }
    let imp = TriSurf::new(ip, it, None);
    import_surface_as_subset(&mut s, &imp, "lid", IMPORT_SUBSET_ANGLE_TOL);
    assert_eq!(s.facet_subsets.elements(s.facet_subsets.index("lid").unwrap()), vec![2, 3]);

    let mut m = b.clone();
    let id = m.facet_subsets.add("first");
    m.facet_subsets.add_element(id, 0);
    let (ids, all) = patch_ids(&m, &["patch0".to_string(), "patch1".to_string()]);
    assert!(all);
    let merged = merge_surface_patches(&m, &ids, "zfaces", false);
    assert_eq!(merged.patches.len(), 5);
    assert_eq!(merged.patches[4], SurfacePatch::new("zfaces"));
    assert_eq!(merged.regions.iter().filter(|&&r| r == 4).count(), 4);
    assert_eq!(merged.facet_subsets.elements(0), vec![8]);
    assert!((surface_volume(&merged.points, &merged.triangles) - 8.0).abs() == 0.0);
}

/// Relative errors `|k R + 1|` of the per-point mean, min and max
/// curvature on the sphere R 3 (one patch), as (max, median) each.
pub(crate) fn sphere_curvature_errors(n_lat: usize, n_lon: usize) -> [(f64, f64); 3] {
    use super::curvature::TriSurfaceCurvatureEstimator;
    let r = 3.0;
    let (p, t) = crate::shapes::sphere_surface(Vec3::ZERO, r, n_lat, n_lon);
    let s = TriSurf::new(p, t, None);
    let c = TriSurfaceCurvatureEstimator::new(&s);
    let stats = |field: &Vec<Vec<f64>>| {
        let mut e: Vec<f64> = field.iter().map(|v| (v[0] * r + 1.0).abs()).collect();
        e.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (e[e.len() - 1], e[e.len() / 2])
    };
    [stats(&c.mean), stats(&c.min), stats(&c.max)]
}

/// V&V (gh:#1278) — `triSurfaceCurvatureEstimator` on the sphere R 3
/// against 1/R. Predictions (before the first run): the surface is wound
/// outward, so the fitted quadric opens downward and every curvature is
/// negative, `k ≈ −1/R`. The quadric error from the next term of the
/// sphere, `r⁴/(8R³)`, is about `h²/(4R²)` relative (h the neighbour
/// distance, πR/24 ≈ 0.39 at 24 × 48): about 0.4 %; the pole points and
/// their thin neighbour triangles are the worst. Predicted: the median
/// mean-curvature error below 1 % at 24 × 48 and below 0.3 % at 48 × 96,
/// the maximum below 3 % and 1 %, and the error falling by about 4× with
/// the doubling. Recorded either way.
#[test]
fn sphere_curvature_against_one_over_r() {
    let a = sphere_curvature_errors(24, 48);
    let b = sphere_curvature_errors(48, 96);
    eprintln!("24x48 (max, median) mean/min/max: {a:?}");
    eprintln!("48x96 (max, median) mean/min/max: {b:?}");
    assert!(a[0].1 < 0.01 && b[0].1 < 0.003);
    assert!(a[0].0 < 0.03 && b[0].0 < 0.01);
}

/// V&V (gh:#1278) — `calculateEdgeCurvature` on the labelled pipe (R 1,
/// 48 sides; wall, two caps). Predictions (before the first run): a rim
/// point has exactly two patch-boundary edges, whose turn is 2π/48 over
/// the chord 2 sin(π/48), so `k = (π/24)/(2 sin(π/48)) = 1.000714 / R`;
/// but upstream measures the angle with the edges' stored directions
/// (lower to higher point label), so at the two points per rim where the
/// numbering wraps (the first and last rim point) both edges start (or
/// end) there and the angle is π − 2π/48: `k ≈ 22.98 / R` there (measured: 23.016, my 22.98 was a slip in the hand arithmetic; the test computes the value from the formula). So 92 of
/// the 96 rim points give 1.000714 (to 1e-9) and 4 give the wrapped value; every other
/// point 0. The flat cap points' mean curvature in the cap patch is 0 to
/// 1e-9.
#[test]
fn pipe_rim_edge_curvature() {
    use super::curvature::TriSurfaceCurvatureEstimator;
    let s = crate::mesh_octree_queries::tests::pipe(1.0, 4.0, 48);
    let c = TriSurfaceCurvatureEstimator::new(&s);
    let chord = 2.0 * (std::f64::consts::PI / 48.0).sin();
    let good = (std::f64::consts::PI / 24.0) / chord;
    let wrong = (std::f64::consts::PI - std::f64::consts::PI / 24.0) / chord;
    let k = &c.edge_point_curvature;
    let n_good = k.iter().filter(|&&v| (v - good).abs() < 1e-9).count();
    let n_wrong = k.iter().filter(|&&v| (v - wrong).abs() < 1e-9).count();
    let n_zero = k.iter().filter(|&&v| v == 0.0).count();
    eprintln!("good {n_good} wrong {n_wrong} zero {n_zero} of {}; values {good} {wrong}", k.len());
    assert_eq!((n_good, n_wrong, n_zero), (92, 4, k.len() - 96));
    for (p, pp) in c.point_patches.iter().enumerate() {
        for (i, &r) in pp.iter().enumerate() {
            if r != 0 {
                assert!(c.mean[p][i].abs() < 1e-9, "cap point {p}: {}", c.mean[p][i]);
            }
        }
    }
}

/// V&V (gh:#1278) — the 5 × 5 `LUsolve` on a known system. Prediction:
/// the solution of `A x = A·[1, 2, 3, 4, 5]` for a non-symmetric `A` that
/// needs a row swap (zero leading entry) is `[1, 2, 3, 4, 5]` to 1e-12.
#[test]
fn lu_solve_known_system() {
    let a = vec![
        vec![0.0, 2.0, 1.0, 0.0, 3.0],
        vec![4.0, 1.0, 0.0, 2.0, 1.0],
        vec![1.0, 0.0, 5.0, 1.0, 0.0],
        vec![2.0, 3.0, 1.0, 6.0, 1.0],
        vec![1.0, 1.0, 0.0, 2.0, 7.0],
    ];
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let b: Vec<f64> = a.iter().map(|r| r.iter().zip(&x).map(|(p, q)| p * q).sum()).collect();
    let y = super::curvature::lu_solve(a, b);
    for i in 0..5 {
        assert!((y[i] - x[i]).abs() < 1e-12, "{y:?}");
    }
}
