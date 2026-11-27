// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1792 (stage 3 of the TetGen port, epic gh:#1789): PLC surface
//! meshing (`tetgen::plc_meshsurface`, upstream's `meshsurface()` after the
//! `-p` Delaunay) against an instrumented upstream build, **slot for slot**.
//!
//! # Methodology
//!
//! Reference: TetGen 1.6.0 (libigl/tetgen `e05aca7df74e`) built as its
//! `makefile` builds it (g++ 13.3.0 `-O3`, predicates `-O0`, x86_64, glibc
//! 2.39) plus `-ffp-contract=off`, with one addition: `tetrahedralize()`
//! calls `dump_meshsurface()` right after `meshsurface()` returns and exits
//! (the patch and the regeneration steps are in
//! `verification_and_validation/tetgen_meshsurface_gh1792/`). Without
//! `TG_DUMP` the instrumented build writes the same `.node`, `.ele`,
//! `.face` and `.edge` as the unmodified binary (checked on `example.poly`).
//!
//! The dump lists, in pool traversal order and **including dead slots**:
//! every point (index, mark, type, flag bits, coordinates bit for bit, the
//! parent of a duplicate), `dummypoint`'s coordinates (the last facet's
//! above point), every subface (three vertices in order, marker, flag bits,
//! the three neighbour links with their edge versions, the three segment
//! links, the two tetrahedron links), every subsegment (two vertices,
//! marker, flags, its three links, whether `sh[6]` is set), every
//! tetrahedron (four vertices, four neighbour links, marker, segment and
//! subface arrays), the three dead-item stacks top first, `recentsh`,
//! `recenttet`, TetGen's `randomnation` seed, `insegments`,
//! `minfacetdihed` (bits) and the next glibc `rand()` value.
//!
//! The port's `TetgenMesh::dump_meshsurface` writes the same format. Pass
//! criterion per case: the two texts are equal, line for line (exact; the
//! failure message names the first differing line).
//!
//! Inputs (committed gzipped in `test_data/tetgen_meshsurface/`): upstream's
//! `example.poly` (facet holes, firstnumber 1); `unit_box.poly` (six quads);
//! `box_hole.poly` (a box with a box-shaped cavity and a volume hole);
//! `box_edges.poly` + `.edge` (a duplicated vertex substituted in a facet,
//! input edges on and off the surface, a degenerate edge); four stress
//! cases for the flip and insertion paths (`comb_prism`: narrow slots and
//! collinear hull vertices; `grid_box`: collinear and cocircular facet
//! vertices; `jitter_soup`: non-Delaunay coplanar soups, merged then
//! flipped; `slit_box`: an interior segment recovered by flips);
//! `fin_box` (three facets at one segment); the Mesh Studio reactor
//! surfaces at size 2 as `.smesh` triangle soups with one marker per patch
//! (`pipe`, `fuel_channel`, `pebble`; Mesh Studio's `reactor_surface`); and a
//! UV `sphere` (two hemisphere markers). All but `example.poly` are written
//! by `write_inputs`.
//!
//! # Results (2026-11-26)
//!
//! | case | points | subfaces | subsegs | tets (with hull) | dump lines identical |
//! |---|---|---|---|---|---|
//! | `example` | 28 | 52 | 44 | 108 | 319 / 319 |
//! | `unit_box` | 8 | 12 | 12 | 18 | 83 / 83 |
//! | `box_hole` | 16 | 24 | 24 | 54 | 171 / 171 |
//! | `box_edges` | 10 | 12 | 14 | 24 | 92 / 92 |
//! | `comb_prism` | 78 | 152 | 110 | 305 | 799 / 799 |
//! | `grid_box` | 74 | 144 | 24 | 336 | 631 / 631 |
//! | `jitter_soup` | 98 | 192 | 48 | 436 | 1 326 / 1 326 |
//! | `slit_box` | 26 | 48 | 12 | 103 | 238 / 238 |
//! | `fin_box` | 10 | 18 | 17 | 25 | 113 / 113 |
//! | `pipe` | 66 | 128 | 96 | 224 | 925 / 925 |
//! | `fuel_channel` | 128 | 256 | 192 | 534 | 1 711 / 1 711 |
//! | `pebble` | 1 060 | 2 112 | 2 070 | 6 836 | 16 613 / 16 613 |
//! | `sphere` | 1 106 | 2 208 | 2 189 | 6 202 | 16 186 / 16 186 |
//!
//! **13 of 13 identical, 39 207 of 39 207 lines.** The comparison can
//! fail: `flip22` setting `recentsh` to `flipfaces[1]` breaks `jitter_soup`
//! at line 1 320 (tried and reverted). Where upstream segfaults
//! (`fin_crash.poly`), the port returns `InternalError`. Times and the
//! regeneration steps: the V&V README.
use outram_park_fork_cfmesh::tetgen::{plc_meshsurface, Behavior, PlcInput, TetgenMesh};
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

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_data/tetgen_meshsurface")
}

fn read_gz(name: &str) -> String {
    let p = data_dir().join(format!("{name}.gz"));
    let b = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    String::from_utf8(gunzip(&b)).unwrap()
}

/// The committed input of a case (`.poly` or `.smesh`, plus `.edge`).
fn input(case: &str) -> PlcInput {
    let dir = data_dir();
    let mut plc = if dir.join(format!("{case}.poly.gz")).exists() {
        PlcInput::read_poly(&read_gz(&format!("{case}.poly"))).unwrap()
    } else {
        PlcInput::read_smesh(&read_gz(&format!("{case}.smesh"))).unwrap()
    };
    if dir.join(format!("{case}.edge.gz")).exists() {
        plc.read_edge(&read_gz(&format!("{case}.edge"))).unwrap();
    }
    plc
}

const CASES: [&str; 13] = [
    "example",
    "unit_box",
    "box_hole",
    "box_edges",
    "comb_prism",
    "grid_box",
    "jitter_soup",
    "slit_box",
    "fin_box",
    "pipe",
    "fuel_channel",
    "pebble",
    "sphere",
];

/// Line counts by kind: `(points, subfaces, subsegs, tets)` live records.
fn counts(dump: &str) -> [usize; 4] {
    let mut c = [0; 4];
    for l in dump.lines() {
        let dead = l.ends_with(" dead");
        match l.as_bytes().first() {
            Some(b'p') if l.starts_with("p ") => c[0] += 1,
            Some(b'f') if l.starts_with("f ") && !dead => c[1] += 1,
            Some(b's') if l.starts_with("s ") && !dead => c[2] += 1,
            Some(b't') if l.starts_with("t ") && !dead => c[3] += 1,
            _ => {}
        }
    }
    c
}

/// Compare the port's dump with upstream's; panic at the first difference.
/// Returns the port's dump and the times of the Delaunay phase
/// (`transfernodes` + `incrementaldelaunay`) and of `meshsurface`.
fn check(case: &str) -> (String, [std::time::Duration; 2]) {
    let plc = input(case);
    let b = Behavior {
        plc: true,
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let mut m = TetgenMesh::new(b);
    m.firstnumber = plc.firstnumber;
    m.transfernodes(&plc.points).unwrap();
    m.incrementaldelaunay().unwrap();
    let t1 = std::time::Instant::now();
    let mut work = plc.clone();
    m.meshsurface(&mut work)
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));
    let dt = [t1 - t0, t1.elapsed()];
    let ours = m.dump_meshsurface();
    let theirs = read_gz(&format!("{case}.dump"));
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), theirs.lines().collect());
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x, y, "{case}: first difference at dump line {}", i + 1);
    }
    assert_eq!(a.len(), b.len(), "{case}: dump lengths differ");
    (ours, dt)
}

#[test]
fn meshsurface_matches_upstream_on_every_case() {
    for case in CASES {
        let (dump, dt) = check(case);
        let [p, f, s, t] = counts(&dump);
        println!(
            "{case:13} points {p:5}  subfaces {f:6}  subsegs {s:5}  tets {t:6}  identical  \
             delaunay {:.3} ms  meshsurface {:.3} ms",
            dt[0].as_secs_f64() * 1e3,
            dt[1].as_secs_f64() * 1e3
        );
    }
}

/// The comparison can fail: the same dump with one subface vertex changed
/// is rejected, and so is the port run with a flipped `facet_small_ang_tol`.
#[test]
fn the_comparison_can_fail() {
    let (dump, _) = check("pebble");
    let theirs = read_gz("pebble.dump");
    assert_eq!(dump, theirs);
    let plc = input("pebble");
    let b = Behavior {
        nomergefacet: true,
        ..Default::default()
    };
    let other = outram_park_fork_cfmesh::tetgen::plc_meshsurface_with(&plc, &b).unwrap();
    assert_ne!(other.mesh.dump_meshsurface(), theirs);
}

/// Where upstream crashes (`fin_crash.poly`: a fin whose edges lie inside
/// two side facets; the instrumented build stops with a segmentation fault
/// in `unifysegments` at `testseg.sh[3]`, upstream `tetgen.cxx:14187`,
/// found with gdb on a `-O0 -g` build), the port returns
/// `TetgenError::InternalError`.
#[test]
fn upstream_crash_is_an_internal_error() {
    let plc = PlcInput::read_poly(&read_gz("fin_crash.poly")).unwrap();
    assert_eq!(
        plc_meshsurface(&plc).unwrap_err(),
        outram_park_fork_cfmesh::tetgen::TetgenError::InternalError
    );
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn crossv(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Upstream-free: every input facet is covered by subfaces of total area
/// equal to the facet's own (polygon area minus hole polygons, by the
/// polygon areas, 1e-12 relative), and every input polygon edge is a chain
/// of subsegments (each polygon edge's endpoints joined by a path of
/// collinear subsegments) unless `mergefacets` removed it between two
/// coplanar facets of one marker.
#[test]
fn surface_covers_facets_and_segments_cover_polygon_edges() {
    for case in CASES {
        let plc = input(case);
        let surf = plc_meshsurface(&plc).unwrap();
        let first = plc.firstnumber;
        let p = |i: i32| plc.points[(i - first) as usize];
        // Total area of the input facets: outer polygon(s) minus inner
        // ones, by the vector area (all facets here are planar).
        let poly_area = |poly: &Vec<i32>| {
            let mut s = [0.0; 3];
            for k in 0..poly.len() {
                let c = crossv(p(poly[k]), p(poly[(k + 1) % poly.len()]));
                s = [s[0] + c[0], s[1] + c[1], s[2] + c[2]];
            }
            0.5 * norm(s)
        };
        let mut want = 0.0;
        for f in &plc.facets {
            let polys: Vec<&Vec<i32>> = f.polygons.iter().filter(|q| q.len() > 2).collect();
            if polys.is_empty() {
                continue;
            }
            // The largest polygon bounds the facet; the others with holes
            //   in the facet are cut out (example.poly's facet 3).
            let mut areas: Vec<f64> = polys.iter().map(|q| poly_area(q)).collect();
            areas.sort_by(|a, b| b.partial_cmp(a).unwrap());
            let mut a = areas[0];
            if !f.holes.is_empty() {
                a -= areas[1..].iter().sum::<f64>();
            }
            want += a;
        }
        let pts = &plc.points;
        let got: f64 = surf
            .subfaces
            .iter()
            .map(|(v, _)| 0.5 * norm(crossv(sub(pts[v[1]], pts[v[0]]), sub(pts[v[2]], pts[v[0]]))))
            .sum();
        assert!(
            (got - want).abs() <= 1e-12 * want,
            "{case}: subface area {got} vs facet area {want}"
        );

        // Polygon edges: a chain of collinear subsegments, or an edge
        //   between two coplanar facets of the same marker (merged).
        let mut segs: std::collections::HashMap<usize, Vec<usize>> = Default::default();
        for (v, _) in &surf.subsegs {
            segs.entry(v[0]).or_default().push(v[1]);
            segs.entry(v[1]).or_default().push(v[0]);
        }
        let mut tri_edges: std::collections::HashMap<(usize, usize), usize> = Default::default();
        for (v, _) in &surf.subfaces {
            for k in 0..3 {
                let (a, b) = (v[k], v[(k + 1) % 3]);
                *tri_edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut chained = 0;
        let mut merged = 0;
        for f in &plc.facets {
            for poly in &f.polygons {
                for k in 0..poly.len() {
                    let (a, b) = (
                        (poly[k] - first) as usize,
                        (poly[(k + 1) % poly.len()] - first) as usize,
                    );
                    if a == b || poly.len() < 2 {
                        continue;
                    }
                    // Walk from a towards b along collinear subsegments.
                    let d = sub(pts[b], pts[a]);
                    let mut cur = a;
                    let mut ok = false;
                    for _ in 0..pts.len() {
                        if cur == b {
                            ok = true;
                            break;
                        }
                        let next = segs.get(&cur).and_then(|n| {
                            n.iter().copied().find(|&q| {
                                let e = sub(pts[q], pts[cur]);
                                let c = crossv(d, e);
                                let along = d[0] * e[0] + d[1] * e[1] + d[2] * e[2];
                                norm(c) <= 1e-12 * norm(d) * norm(e) && along > 0.0
                            })
                        });
                        match next {
                            Some(q) => cur = q,
                            None => break,
                        }
                    }
                    if ok {
                        chained += 1;
                    } else {
                        // Merged away: the edge is interior to the surface.
                        assert_eq!(
                            tri_edges.get(&(a.min(b), a.max(b))).copied().unwrap_or(0) % 2,
                            0,
                            "{case}: polygon edge {a}-{b} is neither a segment chain nor interior"
                        );
                        merged += 1;
                    }
                }
            }
        }
        println!("{case:13} area {got:.6} = {want:.6}; polygon edges: {chained} segment chains, {merged} merged");
        assert!(chained > 0);
    }
}

/// Regenerate the test inputs (run once, by hand):
/// `GH1792_OUT=<dir> cargo test --release -p outram-park-fork-cfmesh --test
/// tetgen_meshsurface_vs_upstream -- --ignored write_inputs`.
/// `example.poly` is upstream's own and is copied from the clone.
#[test]
#[ignore]
fn write_inputs() {
    let out = PathBuf::from(std::env::var("GH1792_OUT").expect("GH1792_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let w = |name: &str, s: &str| std::fs::write(out.join(name), s).unwrap();

    // A unit box, six quadrilateral facets (markers 1..6), 1-based.
    let box_pts = |lo: f64, hi: f64| {
        vec![
            [lo, lo, lo],
            [hi, lo, lo],
            [hi, hi, lo],
            [lo, hi, lo],
            [lo, lo, hi],
            [hi, lo, hi],
            [hi, hi, hi],
            [lo, hi, hi],
        ]
    };
    let quads = [
        [1, 4, 3, 2],
        [5, 6, 7, 8],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 4, 8, 7],
        [4, 1, 5, 8],
    ];
    let write_poly = |pts: &[[f64; 3]], facets: &[(Vec<i32>, i32)], holes: &[[f64; 3]]| {
        let mut s = format!("# gh:#1792 test input\n{} 3 0 0\n", pts.len());
        for (i, p) in pts.iter().enumerate() {
            s += &format!("{} {:?} {:?} {:?}\n", i + 1, p[0], p[1], p[2]);
        }
        s += &format!("{} 1\n", facets.len());
        for (f, m) in facets {
            s += &format!("1 0 {m}\n{}", f.len());
            for v in f {
                s += &format!(" {v}");
            }
            s += "\n";
        }
        s += &format!("{}\n", holes.len());
        for (i, h) in holes.iter().enumerate() {
            s += &format!("{} {:?} {:?} {:?}\n", i + 1, h[0], h[1], h[2]);
        }
        s += "0\n";
        s
    };
    let unit: Vec<(Vec<i32>, i32)> = quads
        .iter()
        .enumerate()
        .map(|(k, q)| (q.to_vec(), k as i32 + 1))
        .collect();
    w("unit_box.poly", &write_poly(&box_pts(0.0, 1.0), &unit, &[]));

    // A 2-unit box with a box-shaped cavity [0.5, 1.5]^3 (inner facets
    // marker 7, a volume hole at the centre).
    let mut pts = box_pts(0.0, 2.0);
    pts.extend(box_pts(0.5, 1.5));
    let mut fac = unit.clone();
    for q in &quads {
        fac.push((q.iter().rev().map(|v| v + 8).collect(), 7));
    }
    w("box_hole.poly", &write_poly(&pts, &fac, &[[1.0, 1.0, 1.0]]));

    // The unit box with a duplicate of vertex 3 (vertex 9, used by the top
    // facet's neighbour 4), an interior point 10, and input edges: a bottom
    // diagonal, an existing boundary edge, a dangling edge and a degenerate
    // one, with markers.
    let mut pts = box_pts(0.0, 1.0);
    pts.push([1.0, 1.0, 0.0]);
    pts.push([0.25, 0.5, 0.5]);
    let mut fac = unit.clone();
    fac[3].0 = vec![2, 9, 7, 6];
    w("box_edges.poly", &write_poly(&pts, &fac, &[]));
    w(
        "box_edges.edge",
        "4 1\n1 1 3 11\n2 1 2 12\n3 10 1 13\n4 5 5 14\n",
    );

    // A prism whose caps are a comb with seven narrow slots (0.1 wide, 4
    // deep) and seven collinear vertices on its straight edge: the slot
    // walls are not Delaunay edges (recovered by `sscoutsegment`'s flips),
    // and the collinear vertices insert on hull edges (`ONEDGE`).
    let mut ring: Vec<[f64; 2]> = Vec::new();
    for k in 0..8 {
        ring.push([k as f64, 0.0]);
    }
    ring.push([8.0, 0.0]);
    ring.push([8.0, 5.0]);
    for k in (1..8).rev() {
        let x = k as f64;
        ring.push([x + 0.05, 5.0]);
        ring.push([x + 0.05, 1.0]);
        ring.push([x - 0.05, 1.0]);
        ring.push([x - 0.05, 5.0]);
    }
    ring.push([0.0, 5.0]);
    let nr = ring.len();
    let mut pts: Vec<[f64; 3]> = ring.iter().map(|q| [q[0], q[1], 0.0]).collect();
    pts.extend(ring.iter().map(|q| [q[0], q[1], 1.0]));
    let mut fac: Vec<(Vec<i32>, i32)> = Vec::new();
    fac.push(((1..=nr as i32).rev().collect(), 1));
    fac.push(((1..=nr as i32).map(|v| v + nr as i32).collect(), 2));
    for k in 0..nr {
        let (a, b) = (k as i32 + 1, ((k + 1) % nr) as i32 + 1);
        fac.push((vec![a, b, b + nr as i32, a + nr as i32], 3));
    }
    w("comb_prism.poly", &write_poly(&pts, &fac, &[]));

    // The unit box whose faces are octagons (corners and edge midpoints,
    // collinear triples) with a 3 x 3 grid of interior facet vertices
    // (one-vertex polygons): cocircular and collinear insertions.
    let mut pts = box_pts(0.0, 1.0);
    let mut key: std::collections::HashMap<[u64; 3], i32> = Default::default();
    for (i, q) in pts.iter().enumerate() {
        key.insert(
            [q[0].to_bits(), q[1].to_bits(), q[2].to_bits()],
            i as i32 + 1,
        );
    }
    let mut id = |q: [f64; 3], pts: &mut Vec<[f64; 3]>| -> i32 {
        *key.entry([q[0].to_bits(), q[1].to_bits(), q[2].to_bits()])
            .or_insert_with(|| {
                pts.push(q);
                pts.len() as i32
            })
    };
    let mut text_facets = String::new();
    let mut nf = 0;
    for (m, q) in quads.iter().enumerate() {
        let c: Vec<[f64; 3]> = q.iter().map(|&v| pts[v as usize - 1]).collect();
        let mid = |a: [f64; 3], b: [f64; 3]| {
            [
                0.5 * (a[0] + b[0]),
                0.5 * (a[1] + b[1]),
                0.5 * (a[2] + b[2]),
            ]
        };
        let mut oct = Vec::new();
        for k in 0..4 {
            oct.push(id(c[k], &mut pts));
            oct.push(id(mid(c[k], c[(k + 1) % 4]), &mut pts));
        }
        let mut inner = Vec::new();
        for (u, v) in [
            (0.25, 0.25),
            (0.5, 0.25),
            (0.75, 0.25),
            (0.25, 0.5),
            (0.5, 0.5),
            (0.75, 0.5),
            (0.25, 0.75),
            (0.5, 0.75),
            (0.75, 0.75),
        ] {
            let mut r = [0.0; 3];
            for d in 0..3 {
                r[d] = c[0][d] + u * (c[1][d] - c[0][d]) + v * (c[3][d] - c[0][d]);
            }
            inner.push(id(r, &mut pts));
        }
        text_facets += &format!("{} 0 {}\n8", 1 + inner.len(), m + 1);
        for v in &oct {
            text_facets += &format!(" {v}");
        }
        text_facets += "\n";
        for v in &inner {
            text_facets += &format!("1 {v}\n");
        }
        nf += 1;
    }
    let mut s = format!("# gh:#1792 test input\n{} 3 0 0\n", pts.len());
    for (i, p) in pts.iter().enumerate() {
        s += &format!("{} {:?} {:?} {:?}\n", i + 1, p[0], p[1], p[2]);
    }
    s += &format!("{nf} 1\n{text_facets}0\n0\n");
    w("grid_box.poly", &s);

    // The unit box as a triangle soup: each face a 5 x 5 grid, interior
    // points jittered in the face's plane, every cell split along the same
    // diagonal (not Delaunay): `mergefacets` removes the coplanar segments
    // and `lawsonflip` flips the non-Delaunay edges.
    let mut pts: Vec<[f64; 3]> = Vec::new();
    let mut key: std::collections::HashMap<[u64; 3], usize> = Default::default();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut marks: Vec<i32> = Vec::new();
    let mut seed: u64 = 1792;
    let mut jit = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.16
    };
    let corners = box_pts(0.0, 1.0);
    for (m, q) in quads.iter().enumerate() {
        let c: Vec<[f64; 3]> = q.iter().map(|&v| corners[v as usize - 1]).collect();
        let n = 4;
        let mut gid = vec![vec![0usize; n + 1]; n + 1];
        for (i, row) in gid.iter_mut().enumerate() {
            for (j, g) in row.iter_mut().enumerate() {
                let (mut u, mut v) = (i as f64 / n as f64, j as f64 / n as f64);
                if i > 0 && i < n && j > 0 && j < n {
                    u += jit();
                    v += jit();
                }
                let mut r = [0.0; 3];
                for d in 0..3 {
                    r[d] = c[0][d] + u * (c[1][d] - c[0][d]) + v * (c[3][d] - c[0][d]);
                }
                *g = *key
                    .entry([r[0].to_bits(), r[1].to_bits(), r[2].to_bits()])
                    .or_insert_with(|| {
                        pts.push(r);
                        pts.len() - 1
                    });
            }
        }
        for i in 0..n {
            for j in 0..n {
                let (a, b, cc, d) = (gid[i][j], gid[i + 1][j], gid[i + 1][j + 1], gid[i][j + 1]);
                tris.push([a, b, cc]);
                tris.push([a, cc, d]);
                marks.push(m as i32 + 1);
                marks.push(m as i32 + 1);
            }
        }
    }
    let plc = PlcInput::from_triangle_soup(&pts, &tris, &marks);
    w("jitter_soup.smesh", &plc.to_smesh().unwrap());

    // The unit box whose top facet also holds a jittered 4 x 4 grid of
    // vertices and an interior two-vertex polygon (a slit) across it: the
    // slit crosses Delaunay edges and is recovered by `sscoutsegment`'s
    // flips (`ACROSSEDGE`, `flip22`, `lawsonflip`).
    let mut pts = box_pts(0.0, 1.0);
    let mut seed: u64 = 9;
    let mut jit2 = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.1
    };
    let mut top = String::new();
    let mut extra = 0;
    for i in 1..=4 {
        for j in 1..=4 {
            let (x, y) = (0.2 * i as f64 + jit2(), 0.2 * j as f64 + jit2());
            pts.push([x, y, 1.0]);
            top += &format!("1 {}\n", pts.len());
            extra += 1;
        }
    }
    pts.push([0.1, 0.47, 1.0]);
    pts.push([0.9, 0.53, 1.0]);
    top += &format!("2 {} {}\n", pts.len() - 1, pts.len());
    extra += 1;
    let mut s = format!("# gh:#1792 test input\n{} 3 0 0\n", pts.len());
    for (i, p) in pts.iter().enumerate() {
        s += &format!("{} {:?} {:?} {:?}\n", i + 1, p[0], p[1], p[2]);
    }
    s += "6 1\n";
    for (k, q) in quads.iter().enumerate() {
        if k == 1 {
            s += &format!(
                "{} 0 2\n4 {} {} {} {}\n{top}",
                1 + extra,
                q[0],
                q[1],
                q[2],
                q[3]
            );
        } else {
            s += &format!("1 0 {}\n4 {} {} {} {}\n", k + 1, q[0], q[1], q[2], q[3]);
        }
    }
    s += "0\n0\n";
    w("slit_box.poly", &s);

    // The unit box with an internal fin from its edge 1-2 to the middle
    // of its top face: three facets at one segment (`unifysegments` orders
    // the face ring by `facedihedral`; `mergefacets` skips the segment).
    let mut pts = box_pts(0.0, 1.0);
    pts.push([1.0, 0.5, 1.0]);
    pts.push([0.0, 0.5, 1.0]);
    let mut fac = unit.clone();
    fac[1].0 = vec![5, 6, 9, 10];
    fac.push((vec![10, 9, 7, 8], 2));
    let mut crash = fac.clone();
    // The side faces split along the fin's edges 2-9 and 1-10, so that
    //   every fin edge is a facet boundary edge.
    fac[3].0 = vec![2, 3, 7, 9];
    fac.push((vec![2, 9, 6], 4));
    fac[5].0 = vec![4, 1, 10, 8];
    fac.push((vec![1, 5, 10], 6));
    fac.push((vec![1, 2, 9, 10], 8));
    w("fin_box.poly", &write_poly(&pts, &fac, &[]));
    // Unsplit side faces: the fin edges 2-9 and 1-10 lie inside the side
    //   facets, not on a segment, and upstream reads a NULL segment in
    //   `unifysegments` (a segmentation fault; the port's InternalError).
    crash[3].0 = vec![2, 3, 7, 9, 6];
    crash[5].0 = vec![4, 1, 5, 10, 8];
    crash.push((vec![1, 2, 9, 10], 8));
    w("fin_crash.poly", &write_poly(&pts, &crash, &[]));

    // The Mesh Studio reactor surfaces at size 2 (a copy of
    // dhoby-ghaut/examples/mesh_studio/model.rs `reactor_surface` and
    // outram-blender `revolved_profile_surface`, which this crate cannot
    // depend on), one marker per patch.
    for (name, kind) in [("pipe", 0), ("fuel_channel", 1), ("pebble", 2)] {
        let (p, t, m) = reactor_surface(kind, 2.0);
        let plc = PlcInput::from_triangle_soup(&p, &t, &m);
        w(&format!("{name}.smesh"), &plc.to_smesh().unwrap());
    }

    // A UV sphere (radius 1, 24 x 48), markers 1 (z >= 0) and 2 (z < 0).
    let (nlat, nlon) = (24usize, 48usize);
    let mut p = vec![[0.0, 0.0, 1.0]];
    for i in 1..nlat {
        let th = std::f64::consts::PI * i as f64 / nlat as f64;
        for j in 0..nlon {
            let ph = 2.0 * std::f64::consts::PI * j as f64 / nlon as f64;
            p.push([th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()]);
        }
    }
    p.push([0.0, 0.0, -1.0]);
    let ring = |i: usize, j: usize| 1 + (i - 1) * nlon + (j % nlon);
    let south = p.len() - 1;
    let mut t = Vec::new();
    for j in 0..nlon {
        t.push([0, ring(1, j), ring(1, j + 1)]);
        t.push([south, ring(nlat - 1, j + 1), ring(nlat - 1, j)]);
    }
    for i in 1..nlat - 1 {
        for j in 0..nlon {
            t.push([ring(i, j), ring(i + 1, j), ring(i + 1, j + 1)]);
            t.push([ring(i, j), ring(i + 1, j + 1), ring(i, j + 1)]);
        }
    }
    let m: Vec<i32> = t
        .iter()
        .map(|tr| {
            let z = (p[tr[0]][2] + p[tr[1]][2] + p[tr[2]][2]) / 3.0;
            if z >= 0.0 {
                1
            } else {
                2
            }
        })
        .collect();
    let plc = PlcInput::from_triangle_soup(&p, &t, &m);
    w("sphere.smesh", &plc.to_smesh().unwrap());
}

/// Mesh Studio's `reactor_surface(kind, size)` (kind 0 pipe, 1 fuel
/// channel, 2 pebble) with patch markers: inlet 1, outlet 2, walls / pin /
/// pebble 3, channel 4.
fn reactor_surface(kind: u8, size: f64) -> (Vec<[f64; 3]>, Vec<[usize; 3]>, Vec<i32>) {
    use outram_park_fork_cfmesh::math::Vec3;
    use petir::mathf::RealMath;
    const N_SEG: usize = 32;
    let rc = 0.5 * size;
    let (profile, height) = match kind {
        0 => {
            let l = 3.0 * size;
            (vec![[0.0, 0.0], [rc, 0.0], [rc, l], [0.0, l]], l)
        }
        1 => {
            let (rp, h) = (0.25 * size, 2.0 * size);
            (vec![[rp, 0.0], [rc, 0.0], [rc, h], [rp, h]], h)
        }
        _ => {
            let (rb, h) = (0.3 * size, 1.5 * size);
            let zc = 0.5 * h;
            let mut prof = vec![
                [0.0, zc + rb],
                [0.0, h],
                [rc, h],
                [rc, 0.0],
                [0.0, 0.0],
                [0.0, zc - rb],
            ];
            for k in 1..32 {
                let a = -core::f64::consts::FRAC_PI_2 + core::f64::consts::PI * k as f64 / 32.0;
                prof.push([rb * a.cos(), zc + rb * a.sin()]);
            }
            (prof, h)
        }
    };
    // outram-blender `revolved_profile_surface(profile, N_SEG)`.
    let n = profile.len();
    let area2: f64 = (0..n)
        .map(|i| {
            let (a, b) = (profile[i], profile[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let ccw = area2 > 0.0;
    let on_axis = |p: [f64; 2]| p[0].abs() < 1e-12;
    let mut pts: Vec<Vec3> = Vec::new();
    let mut ring: Vec<Vec<usize>> = Vec::with_capacity(n);
    for p in &profile {
        if on_axis(*p) {
            pts.push(Vec3::new(0.0, 0.0, p[1]));
            ring.push(vec![pts.len() - 1; N_SEG]);
        } else {
            let mut r = Vec::with_capacity(N_SEG);
            for j in 0..N_SEG {
                let phi = 2.0 * core::f64::consts::PI * j as f64 / N_SEG as f64;
                pts.push(Vec3::new(p[0] * phi.r_cos(), p[0] * phi.r_sin(), p[1]));
                r.push(pts.len() - 1);
            }
            ring.push(r);
        }
    }
    let mut tris: Vec<[usize; 3]> = Vec::new();
    for i in 0..n {
        let (a, b) = (profile[i], profile[(i + 1) % n]);
        if on_axis(a) && on_axis(b) {
            continue;
        }
        let (dr, dz) = (b[0] - a[0], b[1] - a[1]);
        let (nr, nz) = if ccw { (dz, -dr) } else { (-dz, dr) };
        let (ra, rb) = (ring[i].clone(), ring[(i + 1) % n].clone());
        for j in 0..N_SEG {
            let k = (j + 1) % N_SEG;
            for t in [[ra[j], ra[k], rb[k]], [ra[j], rb[k], rb[j]]] {
                if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                    continue;
                }
                let (p0, p1, p2) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                let nrm = p1.sub(p0).cross(p2.sub(p0));
                let c = p0.add(p1).add(p2).scale(1.0 / 3.0);
                let rcc = (c.x * c.x + c.y * c.y).sqrt().max(1e-300);
                let out = Vec3::new(nr * c.x / rcc, nr * c.y / rcc, nz);
                if nrm.dot(out) < 0.0 {
                    tris.push([t[0], t[2], t[1]]);
                } else {
                    tris.push(t);
                }
            }
        }
    }
    // model.rs labels.
    let tol = 1e-9 * size;
    let markers = tris
        .iter()
        .map(|tri| {
            let (a, b, c) = (pts[tri[0]], pts[tri[1]], pts[tri[2]]);
            let flat = (a.z - b.z).abs() < tol && (b.z - c.z).abs() < tol;
            let (cx, cy, cz) = (
                (a.x + b.x + c.x) / 3.0,
                (a.y + b.y + c.y) / 3.0,
                (a.z + b.z + c.z) / 3.0,
            );
            let r = (cx * cx + cy * cy).sqrt();
            match kind {
                _ if flat && cz.abs() < tol => 1,
                _ if flat && (cz - height).abs() < tol => 2,
                0 => 3,
                1 => {
                    if r < 0.375 * size {
                        3
                    } else {
                        4
                    }
                }
                _ => {
                    if r < 0.45 * size {
                        3
                    } else {
                        4
                    }
                }
            }
        })
        .collect();
    let p = pts.iter().map(|v| [v.x, v.y, v.z]).collect();
    (p, tris, markers)
}
