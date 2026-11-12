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

/// P38 (iteration 0): every count of the ESI log row 0 is reproduced. P39 is NOT met: the
/// smoothed displacement differs from `34/displacement`; the measured state is pinned here
/// (first run, 2026-11-12, release) so that a change in either direction is noticed, and
/// the open divergence is described in the V&V record.
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
    // Tie counts (P37): brute force vs octree differ only here.
    assert_eq!((r.ties.triangle, r.ties.edge, r.ties.feature_point, r.ties.patch_point), (190, 14, 0, 0));
    // P39: measured divergence. All 26 feature-point and 1401 of 1405 feature-edge patch
    // displacements and 10231 of 10699 free ones are bit-identical.
    let esi = vectors("feat_disp_34.bin.gz");
    let (same, n, worst) = compare(&it.displacement, &esi);
    assert_eq!((same, n), (37365, 79986), "measured; ESI equality (79986) is NOT reached");
    assert!(worst < 1.5e-6, "worst {worst:e}");
    let mut by_class = [(0usize, 0usize); 4];
    for (i, &mp) in snap.pa.mesh_points.iter().enumerate() {
        let c = r.constraints[i].n as usize;
        by_class[c].0 += 1;
        by_class[c].1 += (0..3).all(|k| esi[mp][k].to_bits() == r.patch_disp[i][k].to_bits()) as usize;
    }
    assert_eq!(by_class, [(10699, 10231), (0, 0), (1405, 1401), (26, 26)]);
    let (same, n, worst) = compare(&mesh.points, &vectors("feat_points_35.bin.gz"));
    assert_eq!((same, n, it.scale_passes.len()), (42862, 79986, 1));
    assert!(worst < 1.5e-6);
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
