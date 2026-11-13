// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1467 / #1459 round 5: `mergePatchFacesUndo` and `mergeEdgesUndo` of ESI v2506
//! `snappyHexMesh` on the reduced flange, from ESI's snapped mesh (time 36) through
//! `repatchToSurface`, face merging and straight-edge point removal, against ESI's
//! final mesh (time 40; 63194 faces and 26662 points to 61034 faces and 24615 points).
//!
//! Methodology, predictions P30-P35 (committed before the port, 9329d01df) and the
//! results: `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`,
//! "Round 5". References: `tests/fixtures/esi_v2506_flange/esi_merge_*.txt.gz`
//! (`extract_merge_fixtures.py`, ASCII precision 17 run, point coordinates of the final
//! mesh verified equal to time 36 as exact text).

use outram_foam_mesh::check_mesh::EsiQualityControls;
use outram_foam_mesh::snappy_hex_mesh::esi_merge::{
    merge_edges_undo, merge_patch_faces_undo, repatch, MergeMesh,
};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{patch_addressing, repatch_to_surface, SnapMesh, V};
use outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes;
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
use petir::mathf::RealMath;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b, "not gzip");
    assert_eq!(bytes[3], 0, "gzip -n expected");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn fixture(name: &str) -> Vec<u8> {
    gunzip(&std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture"))
}

fn vectors(name: &str) -> Vec<V> {
    let raw = fixture(name);
    assert_eq!(raw.len() % 24, 0);
    raw.chunks_exact(24)
        .map(|c| {
            let f = |i: usize| f64::from_le_bytes(c[8 * i..8 * i + 8].try_into().unwrap());
            [f(0), f(1), f(2)]
        })
        .collect()
}

fn lines(name: &str) -> Vec<Vec<i64>> {
    String::from_utf8(fixture(name))
        .unwrap()
        .lines()
        .map(|l| l.split_whitespace().map(|x| x.parse().unwrap()).collect())
        .collect()
}

/// ESI's snapped flange mesh (time 36): castellated faces and numbering, snapped points.
fn esi_mesh36() -> SnapMesh {
    let mut faces = Vec::new();
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for v in lines("esi_final_faces.txt.gz") {
        owner.push(v[0] as usize);
        if v[1] >= 0 {
            neighbour.push(v[1] as usize);
        }
        faces.push(v[2..].iter().map(|&p| p as usize).collect());
    }
    SnapMesh { points: vectors("snap_points_36.bin.gz"), faces, owner, neighbour }
}

/// `cos(degToRad(30))`, ESI's `featureCos` (`resolveFeatureAngle 30`).
fn feature_cos() -> f64 {
    (30.0 * std::f64::consts::PI / 180.0).r_cos()
}

/// Ring as a sequence starting at its smallest vertex (rotation removed, orientation kept).
fn canon(ring: &[usize]) -> Vec<usize> {
    let i = ring.iter().enumerate().min_by_key(|(_, &p)| p).unwrap().0;
    (0..ring.len()).map(|k| ring[(i + k) % ring.len()]).collect()
}

/// The repatched time-36 mesh: `repatch_to_surface` (P29) then the polyTopoChange renumbering.
fn repatched() -> (MergeMesh, usize) {
    let mesh = esi_mesh36();
    let starts = [51058usize, 60368, 61550, 61798, 63194];
    let m0 = MergeMesh::from_patch_starts(mesh, &starts);
    let pa = patch_addressing(&m0.mesh);
    let face_patch: Vec<usize> = pa.face_ids.iter().map(|&f| m0.face_patch[f]).collect();
    let zoned = read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap();
    let surface = TriangleSoup::from_zoned(&zoned);
    let region_patch: Vec<usize> = (0..zoned.n_zones).collect();
    let changes = repatch_to_surface(&m0.mesh, &pa, &face_patch, 1.0, &surface, &zoned.zones, &region_patch);
    let n = changes.len();
    let (m, _) = repatch(&m0, &changes);
    (m, n)
}

/// P30-P35 on the flange: sets, stage 1 (time 37), stage 2 (time 40), per-face equality.
#[test]
fn merge_faces_and_edges_against_esi() {
    let (m, n_repatched) = repatched();
    assert_eq!(n_repatched, 42);
    assert_eq!(m.mesh.faces.len(), 63194);
    let controls = EsiQualityControls::default();
    let preserve = vec![false; m.mesh.faces.len()];
    let patches: Vec<usize> = (0..4).collect();
    let min_cos = feature_cos();

    // ---- stage 1: mergePatchFacesUndo ----
    let (m1, st1) = merge_patch_faces_undo(&m, min_cos, min_cos, &patches, &controls, &preserve, false).expect("no undo");
    eprintln!(
        "stage 1: {} sets, {} points removed, {} faces, {} points, undo iterations (faces in error) {:?}",
        st1.face_sets.len(),
        st1.n_points_removed,
        m1.mesh.faces.len(),
        m1.mesh.points.len(),
        st1.error_faces_per_iteration
    );
    let mut union: Vec<usize> = st1.face_sets.iter().flatten().copied().collect();
    union.sort_unstable();
    let esi_union: Vec<usize> = lines("esi_merge_all_face_sets.txt.gz").iter().map(|v| v[0] as usize).collect();
    eprintln!("P30: port sets union {} faces, ESI {} faces, equal {}", union.len(), esi_union.len(), union == esi_union);
    assert_eq!(st1.face_sets.len(), 2118, "P30: sets");
    assert_eq!(union, esi_union, "P30: faces in sets");
    assert_eq!(m1.mesh.faces.len(), 61034, "P31");
    assert_eq!(m1.mesh.points.len(), 26620, "P31");
    let esi_removed1: Vec<usize> = lines("esi_merge_stage1_removed.txt.gz").iter().map(|v| v[0] as usize).collect();
    assert_eq!(st1.removed_points, esi_removed1, "P31: unused points removed");
    assert_eq!(st1.error_faces_per_iteration, vec![0], "P35: faces step");

    // ESI faces: final (time 40) in final labels, owner/neighbour/patch.
    let esi_final = lines("esi_merge_final_faces.txt.gz");
    let final_to36: Vec<usize> = lines("esi_merge_final_points.txt.gz").iter().map(|v| v[0] as usize).collect();
    assert_eq!(esi_final.len(), 61034);
    assert_eq!(final_to36.len(), 24615);
    let boundary = String::from_utf8(fixture("esi_merge_final_boundary.txt.gz")).unwrap();
    assert_eq!(boundary.lines().count(), 4);
    // ESI time 37 faces in time-36 point labels: the final ring, overridden by the diff.
    let mut esi37: Vec<Vec<usize>> =
        esi_final.iter().map(|v| v[2..].iter().map(|&p| final_to36[p as usize]).collect()).collect();
    for v in lines("esi_merge_stage1_diff.txt.gz") {
        esi37[v[0] as usize] = v[1..].iter().map(|&p| p as usize).collect();
    }
    // Port stage-1 mesh in time-36 labels.
    let kept36: Vec<usize> = (0..m.mesh.points.len()).filter(|p| esi_removed1.binary_search(p).is_err()).collect();
    let (mut same_set, mut same_rot) = (0usize, 0usize);
    let mut first_bad: Option<usize> = None;
    for f in 0..m1.mesh.faces.len() {
        let ring: Vec<usize> = m1.mesh.faces[f].iter().map(|&p| kept36[p]).collect();
        if canon(&ring) == canon(&esi37[f]) {
            same_set += 1;
            if ring == esi37[f] {
                same_rot += 1;
            }
        } else if first_bad.is_none() {
            first_bad = Some(f);
        }
    }
    eprintln!("stage 1 vs ESI time 37: ring equal up to rotation {same_set} of 61034, same start vertex {same_rot}, first bad face {first_bad:?}");
    let owners_equal = (0..61034).all(|f| m1.mesh.owner[f] == esi_final[f][0] as usize);
    let nbrs_equal = (0..m1.mesh.neighbour.len()).all(|f| m1.mesh.neighbour[f] as i64 == esi_final[f][1]);
    eprintln!("owner equal {owners_equal}, neighbour equal {nbrs_equal}, internal faces {}", m1.mesh.neighbour.len());
    assert_eq!(same_set, 61034, "P32 (stage 1)");

    // ---- stage 2: mergeEdgesUndo ----
    let (m2, st2) = merge_edges_undo(&m1, min_cos, &controls).expect("no undo");
    eprintln!(
        "stage 2: {} points removed, {} faces, {} points, faces in error {:?}",
        st2.n_points_removed,
        m2.mesh.faces.len(),
        m2.mesh.points.len(),
        st2.error_faces_per_iteration
    );
    assert_eq!(st2.n_points_removed, 2005, "P34");
    assert_eq!(m2.mesh.points.len(), 24615, "P34");
    assert_eq!(st2.error_faces_per_iteration, vec![0], "P35: edges step");
    // Surviving points: ESI's, by time-36 label.
    let removed2: std::collections::HashSet<usize> = st2.removed_points.iter().copied().collect();
    let kept2: Vec<usize> = (0..m1.mesh.points.len()).filter(|p| !removed2.contains(p)).map(|p| kept36[p]).collect();
    assert_eq!(kept2, final_to36, "P34: surviving points are ESI's");
    // Coordinates are unchanged by merging: bit-identical to ESI's (which equal time 36 as text).
    for (i, &p36) in kept2.iter().enumerate() {
        assert_eq!(m2.mesh.points[i], m.mesh.points[p36]);
    }
    let (mut eq_set, mut eq_rot) = (0usize, 0usize);
    let mut first_bad: Option<usize> = None;
    for f in 0..m2.mesh.faces.len() {
        let want: Vec<usize> = esi_final[f][2..].iter().map(|&p| p as usize).collect();
        if canon(&m2.mesh.faces[f]) == canon(&want) {
            eq_set += 1;
            if m2.mesh.faces[f] == want {
                eq_rot += 1;
            }
        } else if first_bad.is_none() {
            first_bad = Some(f);
        }
    }
    eprintln!("final vs ESI time 40: ring equal up to rotation {eq_set} of 61034, same start vertex {eq_rot}, first bad {first_bad:?}");
    let owners = (0..61034).filter(|&f| m2.mesh.owner[f] == esi_final[f][0] as usize).count();
    let nbrs = (0..m2.mesh.neighbour.len()).filter(|&f| m2.mesh.neighbour[f] as i64 == esi_final[f][1]).count();
    let patches_ok = (m2.mesh.neighbour.len()..61034)
        .filter(|&f| m2.face_patch[f] as i64 == -1 - esi_final[f][1])
        .count();
    eprintln!("owner equal {owners}, neighbour equal {nbrs} of {}, patch equal {patches_ok} of {}", m2.mesh.neighbour.len(), 61034 - m2.mesh.neighbour.len());
    assert_eq!(m2.patch_starts(), vec![51058, 58490, 59652, 59890, 61034], "patch sizes 7432/1162/238/1144");
    assert_eq!(eq_set, 61034, "P32, P34");
    assert_eq!(owners, 61034);
    assert_eq!(nbrs, m2.mesh.neighbour.len());
    assert_eq!(patches_ok, 61034 - m2.mesh.neighbour.len());
    assert!(eq_rot * 10 >= 61034 * 9, "P33: same start vertex {eq_rot}");
}

/// Negative control for P32: the master face is decided by the iteration order of
/// OpenFOAM's `Map<label>`. Taking the lowest face label as master instead (the obvious
/// alternative) gives a different numbering: it must NOT equal ESI's time-37 mesh, so
/// the equality above is not an accident of the data.
#[test]
fn lowest_label_master_is_refuted_by_esi_numbering() {
    use outram_foam_mesh::snappy_hex_mesh::esi_merge::{
        boundary_cells_of, change_mesh, combine_faces_set_refinement, get_merge_sets, MeshTopo,
    };
    let (m, _) = repatched();
    let min_cos = feature_cos();
    let topo = MeshTopo::new(&m.mesh);
    let cells = boundary_cells_of(&m, &[0, 1, 2, 3]);
    let sets = get_merge_sets(&m, &topo, min_cos, min_cos, &cells, false);
    let mut n_other_master = 0usize;
    let alt: Vec<Vec<usize>> = sets
        .iter()
        .map(|s| {
            let mut t = s.clone();
            t.sort_unstable();
            if t[0] != s[0] {
                n_other_master += 1;
            }
            t
        })
        .collect();
    let (m_alt, _) = change_mesh(&m, &combine_faces_set_refinement(&m, &topo, &alt));
    let (m_hash, _) = change_mesh(&m, &combine_faces_set_refinement(&m, &topo, &sets));
    let differing = (0..m_hash.mesh.faces.len())
        .filter(|&f| canon(&m_hash.mesh.faces[f]) != canon(&m_alt.mesh.faces[f]))
        .count();
    eprintln!("{n_other_master} of {} sets have a master that is not the lowest label; {differing} faces differ", sets.len());
    assert!(n_other_master > 0 && differing > 0);
}
