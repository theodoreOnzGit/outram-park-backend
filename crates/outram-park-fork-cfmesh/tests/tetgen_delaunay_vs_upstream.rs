// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1791 (stage 2 of the TetGen port, epic gh:#1789): point-set Delaunay
//! tetrahedralisation (`tetgen::delaunay_tetrahedralize`) against the upstream
//! `tetgen` binary, **element for element and vertex for vertex**.
//!
//! # Methodology
//!
//! Reference: the TetGen 1.6.0 binary (libigl/tetgen `e05aca7df74e`, built
//! with upstream's `makefile`: g++ 13.3.0 `-O3`, predicates `-O0`, x86_64,
//! glibc 2.39), run as `tetgen -zQ <case>.node` (`-zQb0` for the
//! `random_1000_b0` case, BRIO/Hilbert off, so `randomsample` and
//! `randomnation` choose each walk's start; `-zQb/1` for `sphere_centre_last`,
//! input order). Inputs are written by
//! `verification_and_validation/tetgen_delaunay_gh1791/make_cases.py`
//! (`%.17g`, read back exactly by `strtod`); the inputs and upstream's
//! `.1.ele` (and, for the duplicate case, `.1.node`) are committed gzipped
//! in `test_data/tetgen_delaunay/`, so this test needs no compiler.
//!
//! Pass criterion, per case: the same tetrahedron count, every tetrahedron's
//! four vertex indices equal **in the same order within the tetrahedron**,
//! and the tetrahedra in the same order (exact integer equality); the output
//! points equal upstream's `.node` bit for bit (`to_bits`).
//!
//! # Results (2026-11-26)
//!
//! | case | points | tets | identical tets | order identical |
//! |---|---|---|---|---|
//! | `random_10` | 10 | 14 | 14 | yes |
//! | `random_100` | 100 | 529 | 529 | yes |
//! | `random_1000` | 1 000 | 6 388 | 6 388 | yes |
//! | `random_1000_b0` (`-b0`) | 1 000 | 6 388 | 6 388 | yes |
//! | `random_20000` | 20 000 | 133 332 | 133 332 | yes |
//! | `lattice_5x5x5` | 125 | 384 | 384 | yes |
//! | `sphere_exact` (144 integer points, r² = 125) | 144 | 275 | 275 | yes |
//! | `sphere_unit` (400, normalised) | 400 | 1 168 | 1 168 | yes |
//! | `duplicates` (340 in, 40 copies; 300 out) | 340 | 1 754 | 1 754 | yes |
//! | `nearly_flat` (z in [0, 1e-4)) | 500 | 2 900 | 2 900 | yes |
//! | `sphere_centre` (3000 normalised + the centre) | 3 001 | 5 996 | 5 996 | yes |
//! | `sphere_centre_last` (`-b/1`, centre inserted last) | 3 001 | 5 996 | 5 996 | yes |
//!
//! **All twelve cases are identical: 165 124 of 165 124 tetrahedra, order and
//! vertex order included, and every output point bit for bit.** The three
//! cavity-filling branches of `insert_vertex_bw` are all reached (`v_out < 64`
//! everywhere, `64 <= v_out < 1024` once in `sphere_centre`, `v_out >= 1024`
//! once in `sphere_centre_last`, counted with a temporary trace). The
//! comparison can fail: shifting the new `recenttet` choice by one
//! (`(rand() % f_out + 1) % f_out`) breaks `random_10` at its second
//! tetrahedron. No difference was found, so there is none to
//! explain. The run times are in the V&V README.
use outram_park_fork_cfmesh::tetgen::{delaunay_tetrahedralize_with, Behavior};
use std::path::PathBuf;

/// Inflate a `gzip -n` file (10-byte header, 8-byte trailer).
fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(
        bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b,
        "not gzip"
    );
    assert_eq!(bytes[3], 0, "gzip -n expected (no name/extra flags)");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn read_gz(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_data/tetgen_delaunay")
        .join(name);
    let b = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    String::from_utf8(gunzip(&b)).unwrap()
}

/// The data lines of a TetGen file (comments and blank lines dropped).
fn lines(s: &str) -> Vec<Vec<&str>> {
    s.lines()
        .map(|l| l.split('#').next().unwrap())
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|w| !w.is_empty())
        .collect()
}

fn read_node(s: &str) -> Vec<[f64; 3]> {
    let l = lines(s);
    let n: usize = l[0][0].parse().unwrap();
    (0..n)
        .map(|i| {
            assert_eq!(l[i + 1][0].parse::<usize>().unwrap(), i, "0-based .node");
            [
                l[i + 1][1].parse().unwrap(),
                l[i + 1][2].parse().unwrap(),
                l[i + 1][3].parse().unwrap(),
            ]
        })
        .collect()
}

fn read_ele(s: &str) -> Vec<[usize; 4]> {
    let l = lines(s);
    let n: usize = l[0][0].parse().unwrap();
    (0..n)
        .map(|i| {
            let w = &l[i + 1];
            assert_eq!(w[0].parse::<usize>().unwrap(), i);
            [
                w[1].parse().unwrap(),
                w[2].parse().unwrap(),
                w[3].parse().unwrap(),
                w[4].parse().unwrap(),
            ]
        })
        .collect()
}

/// Compare one case; returns (tets, identical, order identical).
fn check(case: &str, input: &str, b: &Behavior, out_node: Option<&str>) {
    let pts = read_node(&read_gz(&format!("{input}.node.gz")));
    let want = read_ele(&read_gz(&format!("{case}.1.ele.gz")));
    let t0 = std::time::Instant::now();
    let got = delaunay_tetrahedralize_with(&pts, b).expect(case);
    let dt = t0.elapsed();
    let identical = got.tets.iter().zip(&want).filter(|(a, b)| a == b).count();
    let first_diff = got.tets.iter().zip(&want).position(|(a, b)| a != b);
    println!(
        "{case}: points {} tets {} (upstream {}), identical {identical}, first difference {:?}, {:.3} ms",
        pts.len(),
        got.tets.len(),
        want.len(),
        first_diff,
        dt.as_secs_f64() * 1e3
    );
    assert_eq!(got.tets.len(), want.len(), "{case}: tet count");
    assert_eq!(first_diff, None, "{case}: first differing tet");
    let want_pts = match out_node {
        Some(f) => read_node(&read_gz(f)),
        None => pts.clone(),
    };
    assert_eq!(got.points.len(), want_pts.len(), "{case}: point count");
    for (a, b) in got.points.iter().zip(&want_pts) {
        for k in 0..3 {
            assert_eq!(a[k].to_bits(), b[k].to_bits(), "{case}: output point");
        }
    }
}

#[test]
fn random_clouds_match_upstream() {
    let b = Behavior::default();
    for n in [10, 100, 1000, 20000] {
        let c = format!("random_{n}");
        check(&c, &c, &b, None);
    }
}

#[test]
fn random_cloud_without_brio_matches_upstream() {
    let b = Behavior {
        brio_hilbert: false,
        ..Behavior::default()
    };
    check("random_1000_b0", "random_1000", &b, None);
}

/// `-b/1`: input order (`no_sort`); the centre, inserted last, has every
/// sphere point on its cavity boundary (`insert_vertex_bw`'s `v_out >= 1024`
/// branch).
#[test]
fn input_order_large_cavity_matches_upstream() {
    let b = Behavior {
        brio_hilbert: false,
        no_sort: true,
        brio_ratio: 1.0,
        ..Behavior::default()
    };
    check("sphere_centre_last", "sphere_centre_last", &b, None);
}

#[test]
fn degenerate_sets_match_upstream() {
    let b = Behavior::default();
    for c in [
        "lattice_5x5x5",
        "sphere_exact",
        "sphere_unit",
        "nearly_flat",
        "sphere_centre",
    ] {
        check(c, c, &b, None);
    }
}

#[test]
fn duplicates_match_upstream() {
    let b = Behavior::default();
    check("duplicates", "duplicates", &b, Some("duplicates.1.node.gz"));
    let pts = read_node(&read_gz("duplicates.node.gz"));
    let r = delaunay_tetrahedralize_with(&pts, &b).unwrap();
    assert_eq!(r.duplicates, 40);
    for (i, p) in pts.iter().enumerate() {
        assert_eq!(&r.points[r.input_to_output[i]], p);
    }
}

/// A check that does not depend on upstream: every tetrahedron is positively
/// oriented by TetGen's convention (`orient3d(a, b, c, d) < 0`); every
/// interior face is locally Delaunay (`insphere(a, b, c, d, e) >= 0` for the
/// vertex `e` across each face: upstream treats `insphere < 0` as "inside the
/// sphere"); every face seen once is a convex-hull face with all points on or
/// inside it; and the tetrahedra's volumes sum to the hull's volume (cones
/// from the centroid to the hull faces), to 1e-12 relative (a float sum, not
/// a comparison with upstream).
#[test]
fn delaunay_properties_hold() {
    use outram_park_fork_cfmesh::tetgen::predicates::Predicates;
    use std::collections::HashMap;
    let b = Behavior::default();
    for case in [
        "random_1000",
        "lattice_5x5x5",
        "sphere_exact",
        "duplicates",
        "nearly_flat",
        "sphere_unit",
    ] {
        let pts = read_node(&read_gz(&format!("{case}.node.gz")));
        let r = delaunay_tetrahedralize_with(&pts, &b).unwrap();
        let p = &r.points;
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for q in p {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let pr = Predicates::exactinit(false, false, hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
        let vol = |a: &[f64; 3], b: &[f64; 3], c: &[f64; 3], d: &[f64; 3]| {
            pr.orient3dfast(a, b, c, d) / 6.0
        };
        // face (sorted) -> (tet, opposite vertex)
        let mut faces: HashMap<[usize; 3], Vec<(usize, usize)>> = HashMap::new();
        let mut tet_vol = 0.0;
        for (ti, t) in r.tets.iter().enumerate() {
            let o = pr.orient3d(&p[t[0]], &p[t[1]], &p[t[2]], &p[t[3]]);
            assert!(o < 0.0, "{case}: tet {ti} not positively oriented");
            tet_vol += -vol(&p[t[0]], &p[t[1]], &p[t[2]], &p[t[3]]);
            for k in 0..4 {
                let mut f = [t[(k + 1) % 4], t[(k + 2) % 4], t[(k + 3) % 4]];
                f.sort();
                faces.entry(f).or_default().push((ti, t[k]));
            }
        }
        let mut hull = Vec::new();
        for (f, s) in &faces {
            match s.len() {
                2 => {
                    for (own, (ti, _)) in s.iter().enumerate() {
                        let t = r.tets[*ti];
                        let e = s[1 - own].1;
                        let ins = pr.insphere(&p[t[0]], &p[t[1]], &p[t[2]], &p[t[3]], &p[e]);
                        assert!(ins >= 0.0, "{case}: face {f:?} not Delaunay");
                    }
                }
                1 => hull.push((*f, s[0].1)),
                n => panic!("{case}: face {f:?} shared by {n} tets"),
            }
        }
        let mut c = [0.0; 3];
        for q in p {
            for k in 0..3 {
                c[k] += q[k] / p.len() as f64;
            }
        }
        let mut hull_vol = 0.0;
        for (f, opp) in &hull {
            let side = pr.orient3d(&p[f[0]], &p[f[1]], &p[f[2]], &p[*opp]);
            assert!(side != 0.0);
            for q in p {
                let s = pr.orient3d(&p[f[0]], &p[f[1]], &p[f[2]], q);
                assert!(
                    s == 0.0 || s.signum() == side.signum(),
                    "{case}: hull face {f:?} not convex"
                );
            }
            hull_vol += vol(&p[f[0]], &p[f[1]], &p[f[2]], &c).abs();
        }
        let rel = (tet_vol - hull_vol).abs() / hull_vol;
        println!(
            "{case}: {} tets, {} hull faces, volume {tet_vol} vs hull {hull_vol} (rel {rel:.1e})",
            r.tets.len(),
            hull.len()
        );
        assert!(rel < 1e-12, "{case}: volume");
    }
}
