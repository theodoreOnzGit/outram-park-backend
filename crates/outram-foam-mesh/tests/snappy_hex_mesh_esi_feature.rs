// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1467 / #1459 / #1336 round 6: explicit feature snapping of OpenFOAM.com ESI v2506
//! `snappyHexMesh` on the flange, morph iteration by morph iteration, from ESI's own meshes.
//!
//! Methodology, predictions P36-P43 (committed before the port code) and the results:
//! `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`, "Round 6". Reference
//! data: `flange.extendedFeatureEdgeMesh.gz` (input of ESI and port), exact binary doubles of
//! the v2506 binary (`feat_*.bin.gz`, `extract_feature_fixtures.py`) and the log numbers of
//! `esi_feature_log.txt.gz`.

use outram_foam_mesh::check_mesh::EsiQualityControls;
use outram_foam_mesh::snappy_hex_mesh::esi_feature::{
    patch_topology, read_extended_edge_mesh, FeatureControls, FeatureSet, FeatureSnap, FeatureSurface,
};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{
    calc_snap_distance, mesh_edges, patch_addressing, scale_topology, PatchPointScaling, ScaleControls, SnapMesh, V,
};
use outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes;
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

fn esi_mesh(points_fixture: &str) -> SnapMesh {
    let text = String::from_utf8(fixture("esi_final_faces.txt.gz")).unwrap();
    let mut faces = Vec::new();
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for l in text.lines() {
        let v: Vec<i64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
        owner.push(v[0] as usize);
        if v[1] >= 0 {
            neighbour.push(v[1] as usize);
        }
        faces.push(v[2..].iter().map(|&p| p as usize).collect());
    }
    SnapMesh { points: vectors(points_fixture), faces, owner, neighbour }
}

/// (bit-identical components, components, worst difference).
fn compare(a: &[V], b: &[V]) -> (usize, usize, f64) {
    assert_eq!(a.len(), b.len());
    let mut same = 0usize;
    let mut worst = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        for c in 0..3 {
            if x[c].to_bits() == y[c].to_bits() {
                same += 1;
            }
            worst = worst.max((x[c] - y[c]).abs());
        }
    }
    (same, 3 * a.len(), worst)
}

fn feature_set() -> FeatureSet {
    let text = String::from_utf8(fixture("flange.extendedFeatureEdgeMesh.gz")).unwrap();
    FeatureSet::new(vec![read_extended_edge_mesh(&text).expect("eMesh")])
}

/// The feature snap of `snappyHexMeshDict.feature` on ESI's castellated mesh.
fn snap_setup(multi_region: bool) -> FeatureSnap {
    let mesh0 = esi_mesh("snap_points_32.bin.gz");
    let pa = patch_addressing(&mesh0);
    let topo = patch_topology(&pa);
    let zoned = read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap();
    let rad = |deg: f64| deg * std::f64::consts::PI / 180.0;
    FeatureSnap {
        features: feature_set(),
        surface: FeatureSurface::from_zoned(&zoned),
        snap_dist: calc_snap_distance(&mesh0, &pa, 1.0),
        mesh_edges: mesh_edges(&mesh0),
        scale_topo: scale_topology(&mesh0),
        pa,
        topo,
        controls: FeatureControls {
            feature_cos: rad(30.0).r_cos(),
            baffle_feature_cos: rad(110.0).r_cos(),
            multi_region,
            string_features: true,
        },
        scale_controls: ScaleControls {
            error_reduction: 0.75,
            n_smooth_scale: 4,
            quality: EsiQualityControls::default(),
            n_allowable_errors: 0,
            patch_scaling: PatchPointScaling::Scaled,
        },
        n_feature_iter: 10,
        n_solve_iter: 300,
        n_relax_iter: 5,
    }
}

/// P36: the eMesh reader.
#[test]
fn extended_edge_mesh_reader() {
    let fs = feature_set();
    let em = &fs.features[0];
    assert_eq!((em.points.len(), em.edges.len(), em.region_edges.len()), (767, 776, 376));
    assert_eq!((em.concave_start, em.mixed_start, em.non_feature_start), (4, 8, 18));
    assert_eq!(em.points[0], [0.006500000134110451, 0.007499999832361937, -0.02374999970197678]);
    assert_eq!(em.edges[0], [72, 73]);
}

/// P38 + P44 (iteration 0): every count of the ESI log row 0 is reproduced and the smoothed
/// displacement equals `34/displacement` and the scaled points `35/polyMesh/points` bit for bit
/// (79986 of 79986 each). Round 6 measured 37365 / 79986 here; the first divergent operation
/// was the nearest-feature-edge search order at four equidistant-edge points (README, "Round 7").
#[test]
fn iteration_zero_against_esi() {
    use outram_foam_mesh::snappy_hex_mesh::esi_feature::AttractStats;
    let snap = snap_setup(false);
    let mut mesh = esi_mesh("snap_points_33_n3.bin.gz");
    let it = snap.morph_iteration(&mut mesh, 0).expect("iteration 0");
    let r = &it.report;
    // The wanted displacement of ESI's log: min and max exactly, the mean to the printed digits.
    assert_eq!(it.wanted.1, 6.548561578869996e-8);
    assert_eq!(it.wanted.2, 7.516794439290358e-4);
    assert!((it.wanted.0 - 1.688540950712846e-4).abs() < 1e-18);
    let stats = |p, e, s, rest| AttractStats { total: 12130, feature_point: p, feature_edge: e, nearest_surface: s, rest };
    assert_eq!(r.raw, stats(24, 1446, 10660, 0));
    assert_eq!(r.baffle_edges, 0);
    assert_eq!((r.reverse_initial, r.reverse_grown), (1470, 4312));
    assert_eq!(r.reverse, stats(18, 1306, 0, 10806));
    assert_eq!(r.stringing, vec![101, 1, 1, 2, 1, 1, 0]);
    assert_eq!(r.fin, stats(26, 1405, 0, 10699));
    // `Attraction: linear` and the max of `feature` equal the log exactly.
    assert_eq!(r.linear.0, [-6.647026854583117e-5, 0.0, 0.0007487347259376377]);
    assert_eq!(r.linear.1, [1.323444465430503e-7, 9.031669137972028e-6, -2.418829367874308e-5]);
    assert_eq!(r.feature.0, [-0.0002020570415173081, 0.0005649122627242589, -0.0009189710170982059]);
    // The `feature` average agrees to about 1e-16 absolute, not in the last digits (ESI prints
    // -8.023800239813117e-9 4.235093043878689e-6 -7.356210052712481e-6).
    assert!((r.feature.1[1] - 4.235093043878689e-6).abs() < 1e-18);
    // P44: bit-identical displacement and scaled points.
    let esi = vectors("feat_disp_34.bin.gz");
    assert_eq!(compare(&it.displacement, &esi), (79986, 79986, 0.0));
    let mut by_class = [(0usize, 0usize); 4];
    for (i, &mp) in snap.pa.mesh_points.iter().enumerate() {
        let c = r.constraints[i].n as usize;
        by_class[c].0 += 1;
        by_class[c].1 += (0..3).all(|k| esi[mp][k].to_bits() == r.patch_disp[i][k].to_bits()) as usize;
    }
    assert_eq!(by_class, [(10699, 10699), (0, 0), (1405, 1405), (26, 26)]);
    assert_eq!(compare(&mesh.points, &vectors("feat_points_35.bin.gz")), (79986, 79986, 0.0));
    assert_eq!(it.scale_passes.len(), 1);
}

/// P45: the octree search and the round-6 brute-force search differ exactly where two shapes
/// are equidistant: counted over every patch point of iteration 0 for the triangle search
/// (the feature-edge search is covered end to end by `iteration_zero_against_esi`: its four
/// mirror valence-3 corner points, mesh 6704, 6710, 6738 and 6744, decided round 6's miss).
#[test]
fn octree_and_brute_force_agree_except_at_ties() {
    use outram_foam_mesh::snappy_hex_mesh::esi_feature::TieCounts;
    let snap = snap_setup(false);
    let mesh = esi_mesh("snap_points_33_n3.bin.gz");
    let mut ties = TieCounts::default();
    let (mut tri_diff_index, mut tri_diff_point) = (0usize, 0usize);
    for (i, &mp) in snap.pa.mesh_points.iter().enumerate() {
        let p = mesh.points[mp];
        let d2 = snap.snap_dist[i] * snap.snap_dist[i];
        let a = snap.surface.nearest(p, d2, &mut ties);
        let b = snap.surface.nearest_brute(p, d2, &mut ties);
        match (a, b) {
            (Some(x), Some(y)) => {
                tri_diff_index += (x.0 != y.0) as usize;
                tri_diff_point += (x.1 != y.1) as usize;
            }
            (None, None) => {}
            _ => panic!("hit/miss disagreement"),
        }
    }
    eprintln!("triangle queries at patch points: index differs {tri_diff_index}, nearest point differs {tri_diff_point}");
    assert_eq!(tri_diff_point, 0, "equidistant triangles give the same nearest point");
}

/// ESI's binary doubles, 64-bit FNV-1a over the little-endian bytes (the per-iteration ESI
/// meshes are 640 KB each and not committed: the digest of ESI's file is pinned instead, so a
/// match is bit-identity of all 79986 components). Computed from the binary run's
/// `N/displacement` (internalField) and `N/polyMesh/points`; see `extract_feature_fixtures.py`.
fn digest(v: &[V]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for p in v {
        for c in p {
            for b in c.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

/// ESI digests, iteration 0..9: `(displacement N, points N+1)`.
const ESI_DIGESTS: [(u64, u64); 10] = [
    (0x377dea4c5c7590e3, 0xb676249ef57889b6),
    (0x1f1bfba5ea4cca74, 0x99a39683b2f1cdf1),
    (0xf8d6915dd7a9d3ba, 0x8860ff55ccc179da),
    (0x0b0bab52aadf5bf1, 0x21b9306d5d011f6b),
    (0x15141f2920127337, 0x04af52ff57981d91),
    (0x122d1dccca48a5d1, 0xfb00a1660664f5ce),
    (0x8e248393aa2e8373, 0xcb37fffd79dced99),
    (0xf8a4a9792b298d64, 0x17035043a05c80ea),
    (0x5f82c36eafabe57a, 0xb254000a4c349e5d),
    (0x1f2836a903739762, 0x4341765330e9acae),
];

/// The ten morph iterations chained (each started from the port's own mesh and displacement
/// field): returns the final scaled mesh (ESI time 77) and checks every iteration against ESI.
fn chained_iterations() -> SnapMesh {
    let snap = snap_setup(false);
    let mut mesh = esi_mesh("snap_points_33_n3.bin.gz");
    let mut prev: Option<Vec<V>> = None;
    for k in 0..10usize {
        let it = snap.morph_iteration_from(&mut mesh, k, prev.as_deref()).expect("iteration");
        assert_eq!(digest(&it.displacement), ESI_DIGESTS[k].0, "iteration {k}: displacement differs from ESI");
        assert_eq!(digest(&mesh.points), ESI_DIGESTS[k].1, "iteration {k}: scaled points differ from ESI");
        let expected_passes = if k < 6 { 1 } else { 7 };
        assert_eq!(it.scale_passes.len(), expected_passes, "iteration {k}: scale passes");
        prev = Some(it.displacement);
    }
    mesh
}

/// P46 and P47 (README, "Round 7"): iterations 0..9 chained are bit-identical to ESI in the
/// smoothed displacement (79986 components) and the scaled points, with 1 scale pass for
/// iterations 0-5 and 7 for 6-9, WITHOUT `detectNearSurfaces` (which overrides the nearest
/// displacement of 2 points in iteration 9 only).
#[test]
fn ten_morph_iterations_chained_against_esi() {
    let mesh = chained_iterations();
    assert_eq!(mesh.points.len(), 26662);
}

/// P48: after the ten iterations, `repatchToSurface` (26 faces) and `mergePatchFacesUndo` +
/// `mergeEdgesUndo` give ESI's final mesh (time 81): 60813 faces, 24311 points, all faces
/// equal per index, owner and neighbour equal, patches 7217 / 1174 / 240 / 1124.
#[test]
fn final_mesh_against_esi() {
    use outram_foam_mesh::snappy_hex_mesh::esi_merge::{merge_edges_undo, merge_patch_faces_undo, repatch, MergeMesh};
    use outram_foam_mesh::snappy_hex_mesh::esi_snap::repatch_to_surface;
    use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
    let mesh = chained_iterations();
    let starts = [51058usize, 60368, 61550, 61798, 63194];
    let m0 = MergeMesh::from_patch_starts(mesh, &starts);
    let pa = patch_addressing(&m0.mesh);
    let face_patch: Vec<usize> = pa.face_ids.iter().map(|&f| m0.face_patch[f]).collect();
    let zoned = read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap();
    let surface = TriangleSoup::from_zoned(&zoned);
    let region_patch: Vec<usize> = (0..zoned.n_zones).collect();
    let changes = repatch_to_surface(&m0.mesh, &pa, &face_patch, 1.0, &surface, &zoned.zones, &region_patch);
    assert_eq!(changes.len(), 26, "ESI: Repatched 26 faces");
    let (m, _) = repatch(&m0, &changes);
    let controls = EsiQualityControls::default();
    let preserve = vec![false; m.mesh.faces.len()];
    let patches: Vec<usize> = (0..4).collect();
    let min_cos = (30.0f64 * std::f64::consts::PI / 180.0).r_cos();
    let (m1, st1) = merge_patch_faces_undo(&m, min_cos, min_cos, &patches, &controls, &preserve, false).expect("no undo");
    let mut union: Vec<usize> = st1.face_sets.iter().flatten().copied().collect();
    union.sort_unstable();
    let esi_sets: Vec<usize> = lines("esi_feat_all_face_sets.txt.gz").iter().map(|v| v[0] as usize).collect();
    eprintln!("sets {} faces in sets {} (ESI {})", st1.face_sets.len(), union.len(), esi_sets.len());
    assert_eq!(union, esi_sets, "faces in merge sets");
    assert_eq!(st1.error_faces_per_iteration, vec![0]);
    let (m2, st2) = merge_edges_undo(&m1, min_cos, &controls).expect("no undo");
    assert_eq!(st2.error_faces_per_iteration, vec![0]);
    // ESI final mesh (time 81)
    let esi_final = lines("esi_feat_final_faces.txt.gz");
    let final_to77: Vec<usize> = lines("esi_feat_final_points.txt.gz").iter().map(|v| v[0] as usize).collect();
    assert_eq!(esi_final.len(), 60813);
    assert_eq!(final_to77.len(), 24311);
    assert_eq!((m2.mesh.faces.len(), m2.mesh.points.len()), (60813, 24311));
    // Surviving points: ESI's, by time-77 label, coordinates unchanged (bit-identical).
    let removed1: std::collections::HashSet<usize> = st1.removed_points.iter().copied().collect();
    let kept1: Vec<usize> = (0..m.mesh.points.len()).filter(|p| !removed1.contains(p)).collect();
    let removed2: std::collections::HashSet<usize> = st2.removed_points.iter().copied().collect();
    let kept2: Vec<usize> = (0..m1.mesh.points.len()).filter(|p| !removed2.contains(p)).map(|p| kept1[p]).collect();
    assert_eq!(kept2, final_to77, "surviving points are ESI's");
    for (i, &p77) in kept2.iter().enumerate() {
        assert_eq!(m2.mesh.points[i], m.mesh.points[p77]);
    }
    let (mut eq_set, mut eq_rot) = (0usize, 0usize);
    for f in 0..m2.mesh.faces.len() {
        let want: Vec<usize> = esi_final[f][2..].iter().map(|&p| p as usize).collect();
        if canon(&m2.mesh.faces[f]) == canon(&want) {
            eq_set += 1;
            eq_rot += (m2.mesh.faces[f] == want) as usize;
        }
    }
    let owners = (0..60813).filter(|&f| m2.mesh.owner[f] == esi_final[f][0] as usize).count();
    let nbrs = (0..m2.mesh.neighbour.len()).filter(|&f| m2.mesh.neighbour[f] as i64 == esi_final[f][1]).count();
    let patches_ok = (m2.mesh.neighbour.len()..60813).filter(|&f| m2.face_patch[f] as i64 == -1 - esi_final[f][1]).count();
    eprintln!("final: ring equal up to rotation {eq_set}, same start vertex {eq_rot}, owner {owners}, neighbour {nbrs} of {}, patch {patches_ok}", m2.mesh.neighbour.len());
    assert_eq!(m2.patch_starts(), vec![51058, 58275, 59449, 59689, 60813]);
    assert_eq!(eq_set, 60813);
    assert_eq!(eq_rot, 60813);
    assert_eq!(owners, 60813);
    assert_eq!(nbrs, m2.mesh.neighbour.len());
    assert_eq!(patches_ok, 60813 - m2.mesh.neighbour.len());
}

/// Ring as a sequence starting at its smallest vertex (rotation removed, orientation kept).
fn canon(ring: &[usize]) -> Vec<usize> {
    let i = ring.iter().enumerate().min_by_key(|(_, &p)| p).unwrap().0;
    (0..ring.len()).map(|k| ring[(i + k) % ring.len()]).collect()
}

fn lines(name: &str) -> Vec<Vec<i64>> {
    String::from_utf8(fixture(name))
        .unwrap()
        .lines()
        .map(|l| l.split_whitespace().map(|x| x.parse().unwrap()).collect())
        .collect()
}

/// `multiRegionFeatureSnap true`, iteration 0: the counts of ESI's log of the same case with
/// the switch on (`esi_feature_multiregion_log.txt.gz`: raw 24 / 1651, reverse selection 1675
/// then 4745, after reverse 18 / 1495, stringing 119 1 1 2 1 1 0, final 26 / 1612).
#[test]
fn iteration_zero_multi_region_counts_against_esi() {
    let snap = snap_setup(true);
    let mut mesh = esi_mesh("snap_points_33_n3.bin.gz");
    let it = snap.morph_iteration(&mut mesh, 0).expect("iteration 0");
    let r = &it.report;
    eprintln!("multi-region: raw {:?} reverse {} -> {} {:?} stringing {:?} final {:?}", r.raw, r.reverse_initial, r.reverse_grown, r.reverse, r.stringing, r.fin);
    assert_eq!((r.raw.feature_point, r.raw.feature_edge), (24, 1651));
    assert_eq!((r.reverse_initial, r.reverse_grown), (1675, 4745));
    assert_eq!((r.reverse.feature_point, r.reverse.feature_edge), (18, 1495));
    assert_eq!(r.stringing, vec![119, 1, 1, 2, 1, 1, 0]);
    assert_eq!((r.fin.feature_point, r.fin.feature_edge), (26, 1612));
}
