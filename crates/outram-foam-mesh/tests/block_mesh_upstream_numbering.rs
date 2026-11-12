// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.

//! `blockMesh` topological merge, upstream point/face/cell numbering,
//! `blockMeshCheck` and degenerate (collapsed) hexes (gh:#1464).
//!
//! # Methodology
//!
//! Upstream: OpenFOAM-dev `852527377c7e` (`blockMeshMerge.C`,
//! `polyMeshFromShapeMesh.C`, `cellMatcher/*`). No OpenFOAM binary is
//! available here, so the numbering is checked against values derived by hand
//! from the upstream source (the predictions in
//! `verification_and_validation/block_mesh_topology_numbering.md`, committed
//! before the first run) and against invariants every `polyMesh` from
//! `blockMesh` has: faces ordered by owner then neighbour, owner below
//! neighbour, patches contiguous, every point used. The full record is in that
//! file.

use outram_foam_mesh::block_mesh::{block_mesh, PolyMesh};

const CAVITY: &str = include_str!("blockMeshDict.cavity");
const T3A: &str = include_str!("fixtures/T3A_blockMeshDict");
const PITZ: &str = include_str!("fixtures/pitzDailySteady_blockMeshDict");

/// Invariants of any polyMesh built by `polyMesh::setTopology`.
fn assert_poly_mesh_invariants(m: &PolyMesh) {
    // Internal faces first, ordered by (owner, neighbour) with owner < neighbour.
    let mut last = (0usize, 0usize);
    for (i, f) in m.faces[..m.n_internal_faces].iter().enumerate() {
        let nb = f.neighbour.expect("internal face has a neighbour");
        assert!(f.owner < nb, "face {i}: owner {} >= neighbour {nb}", f.owner);
        assert!((f.owner, nb) >= last, "face {i} out of (owner, neighbour) order");
        last = (f.owner, nb);
    }
    // Upper-triangular ordering as upstream: for one owner, neighbours rise.
    for f in &m.faces[m.n_internal_faces..] {
        assert!(f.neighbour.is_none());
    }
    // Patches are contiguous and cover the boundary.
    let mut cursor = m.n_internal_faces;
    for p in &m.patches {
        assert_eq!(p.start, cursor, "patch {} not contiguous", p.name);
        cursor += p.size;
    }
    assert_eq!(cursor, m.faces.len());
    // Every point is used by some face.
    let mut used = vec![false; m.points.len()];
    for f in &m.faces {
        for &v in &f.verts {
            used[v] = true;
        }
    }
    assert!(used.iter().all(|&u| u), "unused (unmerged/orphan) point");
    // Each cell is closed: sum of outward face-area vectors vanishes.
    let mut sum = vec![[0.0f64; 3]; m.n_cells];
    for f in &m.faces {
        let a = area(m, &f.verts);
        for d in 0..3 {
            sum[f.owner][d] += a[d];
            if let Some(nb) = f.neighbour {
                sum[nb][d] -= a[d];
            }
        }
    }
    for (c, s) in sum.iter().enumerate() {
        let mag = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        assert!(mag < 1e-9, "cell {c} not closed: {s:?}");
    }
}

fn area(m: &PolyMesh, verts: &[usize]) -> [f64; 3] {
    // Newell's formula (area vector, winding order).
    let mut a = [0.0; 3];
    for i in 0..verts.len() {
        let p = m.points[verts[i]];
        let q = m.points[verts[(i + 1) % verts.len()]];
        a[0] += 0.5 * (p.y * q.z - p.z * q.y);
        a[1] += 0.5 * (p.z * q.x - p.x * q.z);
        a[2] += 0.5 * (p.x * q.y - p.y * q.x);
    }
    a
}

/// Prediction 2 (committed before the first run): the cavity's numbering.
#[test]
fn cavity_numbering_follows_upstream() {
    let m = block_mesh(CAVITY).unwrap();
    assert_eq!(m.points.len(), 882);
    assert_eq!(m.n_internal_faces, 760);
    // Points by first appearance in the block: n = i + 21 j + 441 k.
    for (n, p) in m.points.iter().enumerate() {
        let (i, j, k) = (n % 21, (n / 21) % 21, n / 441);
        assert!((p.x - 0.1 * i as f64 / 20.0).abs() < 1e-15, "point {n}");
        assert!((p.y - 0.1 * j as f64 / 20.0).abs() < 1e-15, "point {n}");
        assert!((p.z - 0.01 * k as f64).abs() < 1e-15, "point {n}");
    }
    // First internal faces: cell 0's face shared with cell 1, then cell 0's
    // face shared with cell 20, then cell 1's with cell 2 ...
    assert_eq!(m.faces[0].verts, vec![1, 22, 463, 442]);
    assert_eq!((m.faces[0].owner, m.faces[0].neighbour), (0, Some(1)));
    assert_eq!(m.faces[1].verts, vec![21, 462, 463, 22]);
    assert_eq!((m.faces[1].owner, m.faces[1].neighbour), (0, Some(20)));
    assert_eq!(m.faces[2].verts, vec![2, 23, 464, 443]);
    assert_eq!((m.faces[2].owner, m.faces[2].neighbour), (1, Some(2)));
    assert_poly_mesh_invariants(&m);
}

#[test]
fn tutorial_meshes_satisfy_polymesh_invariants() {
    for (name, dict) in [("pitzDailySteady", PITZ), ("T3A", T3A)] {
        let m = block_mesh(dict).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_poly_mesh_invariants(&m);
    }
}

fn two_blocks(second_vertices: &str, second_cells: &str) -> String {
    format!(
        "convertToMeters 1;
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1)
          (2 0 0) (2 1 0) (2 0 1) (2 1 1) (1 0 0) (1 1 0) (1 0 1) (1 1 1));
blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1)
        hex {second_vertices} {second_cells} simpleGrading (1 1 1));
boundary ();"
    )
}

/// The merge is topological: two blocks that touch geometrically but do not
/// share vertex labels are NOT merged (upstream needs `mergePatchPairs`), and
/// blocks that share labels merge; mismatched cell counts across the shared
/// face are an error.
#[test]
fn merge_follows_block_topology_not_coordinates() {
    // Sharing vertices 1 2 5 6: merged.
    let shared = block_mesh(&two_blocks("(1 8 9 2 5 10 11 6)", "(2 2 2)")).unwrap();
    assert_eq!(shared.points.len(), 27 + 27 - 9);
    assert_poly_mesh_invariants(&shared);
    // Same coordinates through duplicate vertices 12..15: not merged.
    let apart = block_mesh(&two_blocks("(12 8 9 13 14 10 11 15)", "(2 2 2)")).unwrap();
    assert_eq!(apart.points.len(), 54);
    assert_eq!(apart.n_internal_faces, 24, "12 per block, none across the gap");
    // 3 cells across the shared face where the other block has 2: error.
    let bad = block_mesh(&two_blocks("(1 8 9 2 5 10 11 6)", "(2 3 2)"));
    assert!(bad.is_err(), "inconsistent face correspondence must fail");
}

/// A hex with a collapsed edge (`v3 = v0`, `v7 = v4`) becomes prisms
/// (`degenerateMatcher`): the triangle `(0,0) (1,0) (1,1)` extruded by 1.
#[test]
fn collapsed_hex_gives_prisms() {
    let dict = "convertToMeters 1;
vertices ((0 0 0) (1 0 0) (1 1 0) (0 0 1) (1 0 1) (1 1 1));
blocks (hex (0 1 2 0 3 4 5 3) (2 2 1) simpleGrading (1 1 1));
boundary ();";
    let m = block_mesh(dict).unwrap();
    assert_eq!(m.n_cells, 4);
    // 3*2*2 block points, the x = 0 column of 3 per layer merged to one.
    assert_eq!(m.points.len(), 18 - 4);
    let mut n_faces = vec![0usize; m.n_cells];
    for f in &m.faces {
        n_faces[f.owner] += 1;
        if let Some(nb) = f.neighbour {
            n_faces[nb] += 1;
        }
    }
    n_faces.sort_unstable();
    assert_eq!(n_faces, vec![5, 5, 6, 6], "two prisms and two hexes");
    assert!(m.faces.iter().any(|f| f.verts.len() == 3));
    assert!((m.total_volume() - 0.5).abs() < 1e-12, "volume {}", m.total_volume());
    assert_poly_mesh_invariants(&m);
}

/// `blockMesh::check`: a curved edge that repeats another, or matches no
/// block edge, is refused.
#[test]
fn block_mesh_check_rejects_bad_edges() {
    let base = |edges: &str| {
        format!(
            "convertToMeters 1;
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));
blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));
edges ({edges});
boundary ();"
        )
    };
    let dup = base("arc 0 1 (0.5 -0.1 0) arc 1 0 (0.5 -0.1 0)");
    let e = block_mesh(&dup).unwrap_err().to_string();
    assert!(e.contains("duplicate"), "{e}");
    let stray = base("line 0 2");
    let e = block_mesh(&stray).unwrap_err().to_string();
    assert!(e.contains("does not correspond to a block edge"), "{e}");
    assert!(block_mesh(&base("arc 0 1 (0.5 -0.1 0)")).is_ok());
}

/// A patch face that belongs to no block is an upstream fatal error.
#[test]
fn patch_face_outside_every_block_is_refused() {
    let dict = "convertToMeters 1;
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));
blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));
boundary (a { type patch; faces ((0 1 6 7)); });";
    assert!(block_mesh(dict).is_err());
}

/// gh:#1463: named vertices, named blocks and `(block face)` patch entries
/// give the same mesh as the numeric form.
#[test]
fn named_vertices_blocks_and_block_face_entries() {
    let named = "convertToMeters 1;
vertices (name a (0 0 0) name b (1 0 0) name c (1 1 0) name d (0 1 0)
          name e (0 0 1) name f (1 0 1) name g (1 1 1) name h (0 1 1));
blocks (name blk hex (a b c d e f g h) (2 2 2) simpleGrading (1 1 1));
edges (arc a b (0.5 -0.05 0));
boundary (
  xmin { type patch; faces ((blk 0)); }
  rest { type wall; faces ((blk 1) (blk 3) (a d c b) (e f g h)); }
);";
    let numeric = "convertToMeters 1;
vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));
blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));
edges (arc 0 1 (0.5 -0.05 0));
boundary (
  xmin { type patch; faces ((0 4 7 3)); }
  rest { type wall; faces ((1 2 6 5) (3 7 6 2) (0 3 2 1) (4 5 6 7)); }
);";
    let a = block_mesh(named).unwrap();
    let b = block_mesh(numeric).unwrap();
    assert_eq!(a.points.len(), b.points.len());
    assert_eq!(a.faces.len(), b.faces.len());
    let names = |m: &PolyMesh| -> Vec<(String, usize)> {
        m.patches.iter().map(|p| (p.name.clone(), p.size)).collect()
    };
    assert_eq!(names(&a), names(&b));
    assert_eq!(names(&a)[0], ("xmin".to_string(), 4));
    for (fa, fb) in a.faces.iter().zip(&b.faces) {
        assert_eq!(fa.verts, fb.verts);
    }
    // Out-of-range block and block-face indices are refused.
    assert!(block_mesh(&named.replace("(blk 1)", "(7 1)")).is_err());
    assert!(block_mesh(&named.replace("(blk 1)", "(blk 9)")).is_err());
    // An undefined name is refused.
    assert!(block_mesh(&named.replace("(a d c b)", "(a d c zz)")).is_err());
}
