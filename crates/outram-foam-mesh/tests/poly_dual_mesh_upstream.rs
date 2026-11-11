// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Verification of the upstream-mode `polyDualMesh` (src/poly_dual_mesh_upstream.rs;
// gh:#1460 children #1471, #1472, #1473). Methodology, the predictions that
// were committed before the first run, and the results:
// verification_and_validation/poly_dual_mesh_gh1460.md.
//
// The file-by-file comparison with an OpenFOAM.com v2506 run is the
// `compare_polymesh` example (dual mode); these tests are the in-tree checks:
// hash-set order, counts, closure, volume, ordering, and each option.

use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::poly_dual_mesh::{poly_dual_mesh_with, DualMesh, FeaturePoints, PolyMesh};
use outram_foam_mesh::poly_dual_mesh_upstream::{
    cell_centres_and_volumes, face_area_and_centre, poly_dual_mesh_upstream, primal_edges, CentreFormula,
    FaceZoneInput, FoamLabelHashSet, UpstreamOptions,
};

fn check_valid(d: &DualMesh, expect_volume: f64) {
    assert!(d.max_closure_residual() < 1e-12, "closure {}", d.max_closure_residual());
    assert_eq!(d.first_bad_euler(), None);
    assert!(
        (d.total_volume() - expect_volume).abs() < 1e-9,
        "volume {} vs {expect_volume}",
        d.total_volume()
    );
}

fn n_internal(d: &DualMesh) -> usize {
    d.neighbour.iter().filter(|n| n.is_some()).count()
}

#[test]
fn hash_set_toc_follows_the_openfoam_table() {
    // P1: size 4; the 4th insert (load 1.0 > 0.8) doubles the table.
    let mut s = FoamLabelHashSet::with_size(4);
    for k in [1, 5, 2, 6] {
        assert!(s.insert(k));
    }
    assert_eq!(s.toc(), vec![1, 2, 5, 6]);
    assert!(!s.insert(5), "duplicate insert is refused");
    // Collisions chain with the newest key at the head: 5 and 1 share bucket 1
    // of a 4-table.
    let mut t = FoamLabelHashSet::with_size(4);
    t.insert(1);
    t.insert(5);
    assert_eq!(t.toc(), vec![5, 1]);
    // P1b
    let mut u = FoamLabelHashSet::with_size(2);
    u.insert(0);
    assert_eq!(u.toc(), vec![0]);
    u.insert(2);
    assert_eq!(u.toc(), vec![0, 2]);
    // erase keeps the order of the others; found() agrees.
    assert!(u.erase(0));
    assert!(!u.found(0) && u.found(2));
    assert_eq!(u.toc(), vec![2]);
    assert_eq!(u.len(), 1);
    assert!(!u.is_empty());
    // default-constructed sets start at 128 buckets: 3 and 131 collide.
    let mut d = FoamLabelHashSet::default();
    d.insert(3);
    d.insert(131);
    assert_eq!(d.toc(), vec![131, 3]);
}

#[test]
fn esi_reserve_sizes_the_table_for_twice_the_entries() {
    // dev `HashSet(3)`: 4 buckets (1 and 5 share bucket 1, newest first);
    // v2506 `reserve(3)`: canonicalSize(6) = 8 buckets, no collision.
    let mut d = FoamLabelHashSet::with_size(3);
    let mut e = FoamLabelHashSet::with_reserve_esi2506(3);
    for k in [1, 5, 2] {
        d.insert(k);
        e.insert(k);
    }
    assert_eq!(d.toc(), vec![5, 1, 2]);
    assert_eq!(e.toc(), vec![1, 2, 5]);
    assert_eq!(FoamLabelHashSet::with_reserve_esi2506(0).len(), 0);
    // an unreserved ESI set of 0 still grows on first insert
    let mut z = FoamLabelHashSet::with_reserve_esi2506(0);
    assert!(z.insert(3));
    assert_eq!(z.toc(), vec![3]);
}

#[test]
fn centre_formulas_agree_on_planar_faces_and_differ_in_how_they_round() {
    // A planar quad: both lines give its centre to round-off; a triangle is
    // the same expression in both.
    let q = [
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
        Vector3::new(2.0, 1.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    ];
    for f in [CentreFormula::Dev, CentreFormula::Esi2506] {
        let (a, c) = face_area_and_centre(f, &q);
        assert!((a.z - 2.0).abs() < 1e-15 && a.x == 0.0 && a.y == 0.0);
        assert!((c.x - 1.0).abs() < 1e-15 && (c.y - 0.5).abs() < 1e-15);
    }
    let tri = &q[..3];
    assert_eq!(
        face_area_and_centre(CentreFormula::Dev, tri).1.x.to_bits(),
        face_area_and_centre(CentreFormula::Esi2506, tri).1.x.to_bits()
    );
    // A warped quad: the two lines' centres are different points (the reason
    // gh:#1473 exists), by a small but non-round-off amount.
    let w = [
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(3.0, 0.0, 0.0),
        Vector3::new(3.0, 1.0, 0.7),
        Vector3::new(0.0, 5.0, 0.0),
    ];
    let cd = face_area_and_centre(CentreFormula::Dev, &w).1;
    let ce = face_area_and_centre(CentreFormula::Esi2506, &w).1;
    let d = ((cd.x - ce.x).powi(2) + (cd.y - ce.y).powi(2) + (cd.z - ce.z).powi(2)).sqrt();
    assert!(d > 1e-6, "formulas should differ on a warped face, d = {d}");
}

#[test]
fn cell_centres_of_a_two_cell_block_are_the_cell_midpoints() {
    let m = PolyMesh::structured_hex_block(2, 1, 1, 2.0, 1.0, 1.0);
    let (fc, fa): (Vec<_>, Vec<_>) = m
        .faces
        .iter()
        .map(|f| {
            let pts: Vec<Vector3> = f.iter().map(|&p| m.points[p]).collect();
            face_area_and_centre(CentreFormula::Dev, &pts)
        })
        .map(|(a, c)| (c, a))
        .unzip();
    let (cc, vol) = cell_centres_and_volumes(m.n_cells, &m.owner, &m.neighbour, &fc, &fa);
    assert!((cc[0].x - 0.5).abs() < 1e-14 && (cc[1].x - 1.5).abs() < 1e-14);
    assert!((vol[0] - 1.0).abs() < 1e-14 && (vol[1] - 1.0).abs() < 1e-14);
}

#[test]
fn block_3x3x3_upstream_mode_counts_and_geometry() {
    let m = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let d = poly_dual_mesh_upstream(&m, &UpstreamOptions::new(60.0)).unwrap();
    assert_eq!(d.n_cells, 64, "P2: one dual cell per primal point");
    assert_eq!(d.points.len(), 197, "P2: 27 + 54 + 32 + 84 dual points");
    assert_eq!(n_internal(&d), 144, "P2: one internal dual face per primal edge");
    check_valid(&d, 27.0);
    // internal faces come first and are upper-triangular
    let ni = n_internal(&d);
    assert!(d.neighbour[..ni].iter().all(|n| n.is_some()));
    assert!(d.neighbour[ni..].iter().all(|n| n.is_none()));
    let pairs: Vec<(usize, usize)> = (0..ni).map(|f| (d.owner[f], d.neighbour[f].unwrap())).collect();
    assert!(pairs.iter().all(|&(o, n)| o < n));
    assert!(pairs.windows(2).all(|w| w[0] <= w[1]), "upper-triangular order");
    // every boundary face has a patch and patches are contiguous
    assert!(d.face_patch[ni..].iter().all(|p| p.is_some()));
    assert!(d.face_patch[ni..].windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn split_all_faces_gives_several_faces_between_two_cells() {
    let m = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let d = poly_dual_mesh_upstream(&m, &UpstreamOptions::new(60.0).with_split_all_faces()).unwrap();
    assert_eq!(d.points.len(), 311, "P3: 27 + 108 + 32 + 144");
    assert_eq!(n_internal(&d), 324, "P3: one face per cell around each edge");
    check_valid(&d, 27.0);
    let mut pairs: Vec<(usize, usize)> = (0..d.faces.len())
        .filter_map(|f| d.neighbour[f].map(|n| (d.owner[f], n)))
        .collect();
    let before = pairs.len();
    pairs.sort_unstable();
    pairs.dedup();
    assert!(pairs.len() < before, "some dual cell pair shares more than one face");
}

/// Keep the cells of `m` flagged in `keep`; faces between a kept and a dropped
/// cell become a new last patch `cut`. Points are compacted.
fn subset(m: &PolyMesh, keep: &[bool]) -> PolyMesh {
    let mut cell_new = vec![usize::MAX; m.n_cells];
    let mut nc = 0;
    for c in 0..m.n_cells {
        if keep[c] {
            cell_new[c] = nc;
            nc += 1;
        }
    }
    let mut used = vec![false; m.points.len()];
    let mut int_faces = Vec::new();
    let mut int_own = Vec::new();
    let mut int_nei = Vec::new();
    let mut patch_faces: Vec<Vec<(Vec<usize>, usize)>> = vec![Vec::new(); m.patches.len() + 1];
    for f in 0..m.faces.len() {
        let own = m.owner[f];
        if f < m.n_internal_faces {
            let nei = m.neighbour[f];
            match (keep[own], keep[nei]) {
                (true, true) => {
                    int_faces.push(m.faces[f].clone());
                    int_own.push(cell_new[own]);
                    int_nei.push(cell_new[nei]);
                }
                (true, false) => patch_faces[m.patches.len()].push((m.faces[f].clone(), cell_new[own])),
                (false, true) => {
                    let mut v = m.faces[f].clone();
                    v.reverse();
                    patch_faces[m.patches.len()].push((v, cell_new[nei]));
                }
                _ => {}
            }
        } else if keep[own] {
            let pi = m.patches.iter().position(|p| f >= p.start && f < p.start + p.size).unwrap();
            patch_faces[pi].push((m.faces[f].clone(), cell_new[own]));
        }
    }
    let mut faces = int_faces;
    let mut owner = int_own;
    let n_internal_faces = faces.len();
    let mut patches = Vec::new();
    for (pi, pf) in patch_faces.iter().enumerate() {
        let name = if pi < m.patches.len() { m.patches[pi].name.clone() } else { "cut".to_string() };
        patches.push(BoundaryPatch::new(name, faces.len(), pf.len(), PatchKind::Wall));
        for (v, o) in pf {
            faces.push(v.clone());
            owner.push(*o);
        }
    }
    for f in &faces {
        for &p in f {
            used[p] = true;
        }
    }
    let mut pnew = vec![usize::MAX; m.points.len()];
    let mut points = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            pnew[i] = points.len();
            points.push(m.points[i]);
        }
    }
    for f in faces.iter_mut() {
        for p in f.iter_mut() {
            *p = pnew[*p];
        }
    }
    PolyMesh { points, faces, owner, neighbour: int_nei, n_internal_faces, patches, n_cells: nc }
}

/// A 2x2x1 block (cells 0..4 as x fastest) without its last cell, one patch.
fn l_shape() -> PolyMesh {
    let m = PolyMesh::structured_hex_block(2, 2, 1, 2.0, 2.0, 1.0);
    let mut l = subset(&m, &[true, true, true, false]);
    // one patch only, so the seams come from the feature angle alone
    let total = l.faces.len() - l.n_internal_faces;
    l.patches = vec![BoundaryPatch::new("walls", l.n_internal_faces, total, PatchKind::Wall)];
    l
}

#[test]
fn concave_multi_cells_split_the_inner_corner_points() {
    let l = l_shape();
    assert_eq!(l.n_cells, 3);
    assert_eq!(l.points.len(), 16);
    let plain = poly_dual_mesh_upstream(&l, &UpstreamOptions::new(60.0)).unwrap();
    assert_eq!(plain.n_cells, 16);
    check_valid(&plain, 3.0);
    let multi =
        poly_dual_mesh_upstream(&l, &UpstreamOptions::new(60.0).with_concave_multi_cells()).unwrap();
    assert_eq!(multi.n_cells, 20, "P4: the two inner-corner points become 3 cells each");
    check_valid(&multi, 3.0);
}

#[test]
fn face_zones_are_preserved_by_default_and_ignored_when_disabled() {
    let m = PolyMesh::structured_hex_block(2, 1, 1, 2.0, 1.0, 1.0);
    assert_eq!(m.n_internal_faces, 1);
    let zone = FaceZoneInput { name: "mid".into(), faces: vec![0] };
    let on = poly_dual_mesh_upstream(&m, &UpstreamOptions::new(60.0).with_face_zones(vec![zone.clone()])).unwrap();
    assert_eq!(on.n_cells, 16, "P5: 12 points + 4 zone points counted twice");
    check_valid(&on, 2.0);
    let off = poly_dual_mesh_upstream(
        &m,
        &UpstreamOptions::new(60.0).with_face_zones(vec![zone]).without_face_zone_preservation(),
    )
    .unwrap();
    assert_eq!(off.n_cells, 12);
    check_valid(&off, 2.0);
}

#[test]
fn bad_face_zone_label_is_rejected() {
    let m = PolyMesh::structured_hex_block(2, 1, 1, 2.0, 1.0, 1.0);
    let z = FaceZoneInput { name: "bad".into(), faces: vec![10_000] };
    assert!(poly_dual_mesh_upstream(&m, &UpstreamOptions::new(60.0).with_face_zones(vec![z])).is_err());
}

#[test]
fn esi_centres_option_runs_and_gives_a_valid_dual() {
    let m = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let d = poly_dual_mesh_upstream(&m, &UpstreamOptions::new(60.0).with_centre_formula(CentreFormula::Esi2506)).unwrap();
    check_valid(&d, 27.0);
}

#[test]
fn edge_numbering_follows_calc_edges_for_ordered_points() {
    // Ground truth: OpenFOAM.com v2506 `mesh.edges()` printed for the cavity
    // tutorial's blockMesh (20x20x1, every point on the boundary so the
    // points count as ordered with nInternalPoints = 0): edges not on a
    // boundary face come first (the z-edges), the edges of boundary faces
    // last. Entries read from the v2506 run (see the V&V record).
    let m = PolyMesh::structured_hex_block(20, 20, 1, 1.0, 1.0, 0.1);
    let e = primal_edges(&m);
    assert_eq!(e.len(), 2121);
    assert_eq!(e[0], (22, 463));
    assert_eq!(e[1], (23, 464));
    assert_eq!(e[560], (87, 108));
    assert_eq!(e[1280], (440, 881));
    assert_eq!(e[1281], (441, 442));
    // A mesh whose points are NOT ordered (an interior point numbered before
    // a boundary point) falls back to the upper-triangular order.
    let mut u = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let eu = primal_edges(&u);
    assert!(eu.windows(2).all(|w| w[0] < w[1]) || eu.len() == 144, "edge list is well formed");
    u.points.swap(0, 21); // geometry only; connectivity untouched
    assert_eq!(primal_edges(&u).len(), 144);
}

#[test]
fn default_mode_is_unchanged_by_the_upstream_mode_existing() {
    // P6 (in-tree part): the default's published counts for the 3x3x3 block.
    let m = PolyMesh::structured_hex_block(3, 3, 3, 3.0, 3.0, 3.0);
    let a = poly_dual_mesh_with(&m, 45.0, FeaturePoints::OnFeatureEdge).unwrap();
    let b = poly_dual_mesh_with(&m, 45.0, FeaturePoints::OnFeatureEdge).unwrap();
    assert_eq!(a.faces, b.faces);
    assert_eq!(a.n_cells, 64);
    assert!(a.points.len() > 197, "the default adds a point on every boundary edge");
}
