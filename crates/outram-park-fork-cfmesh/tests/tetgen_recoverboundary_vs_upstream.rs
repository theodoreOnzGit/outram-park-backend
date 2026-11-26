// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1793 (stage 4 of the TetGen port, epic gh:#1789): boundary recovery
//! (`tetgen::tetrahedralize_plc`, upstream's `recoverboundary()`) against an
//! instrumented upstream build, **slot for slot**.
//!
//! # Methodology
//!
//! Reference: TetGen 1.6.0 (libigl/tetgen `e05aca7df74e`), `tetgen.cxx` built
//! with g++ 13.3.0 `-O2 -ffp-contract=off`, `predicates.cxx` `-O0`, x86_64,
//! glibc 2.39, with `recoverboundary_dump.patch`
//! (`verification_and_validation/tetgen_recoverboundary_gh1793/`): right after
//! `recoverboundary()` returns, `tetrahedralize()` writes the whole mesh state
//! (`TG_DUMP4`) and exits; per-routine and per-branch call counters
//! (`tg_reach`, reset when `recoverboundary` starts) are written to `TG_REACH`.
//! The reference runs are `tetgen -p` and `tetgen -pY` **without `-Q`**
//! (upstream skips a few `terminatetetgen` calls inside its `!b->quiet`
//! warning blocks; the port follows the default, non-quiet control flow).
//!
//! The dump lists, in pool order and including dead slots: every point
//! (mark, type, flags, coordinates as bits, `point2tet`, `point2sh`);
//! `dummypoint`; every subface and subsegment (vertices, marker, flags,
//! facet / segment index, every link with its version); every tetrahedron
//! (vertices, neighbours with versions, the marker word, the six
//! segment and four subface links); the dead-item stacks; `recentsh`,
//! `recenttet`, the `randomnation` seed; the Steiner counters
//! `st_segref_count`, `st_facref_count`, `st_volref_count`; the flip
//! counters; the flip link levels; the work stacks; the next glibc `rand()`.
//! The port's `TetgenMesh::dump_recoverboundary` writes the same text.
//!
//! **Pass criterion** per case and switch: the two dumps are equal line for
//! line (exact text), and the port's 55 reach counters equal upstream's.
//!
//! Inputs: the 13 stage 3 cases (`test_data/tetgen_meshsurface/`), plus six
//! made to need Steiner points and deep flips
//! (`test_data/tetgen_recoverboundary/`): `schoenhardt` (a twisted triangular
//! prism, one volume Steiner point), `thin_slab` (a 0.005-thick plate with
//! jittered 8 x 8 grids top and bottom), `facet_wall` and `facet_wall2` (a
//! box with an internal wall and free vertices just above and below it:
//! facet Steiner points, inserted and removed), `skew_segments` (five
//! nearly touching skew segments in a box with free vertices near them:
//! segment Steiner points, removed again by `sremovevertex`) and
//! `skew_points` (the full-search flips). `-pY` additionally keeps
//! coplanar facets unmerged, which gives `fuel_channel` its 33 volume
//! Steiner points and `slit_box` a segment Steiner point.
//!
//! # Results (2026-11-26)
//!
//! **19 cases x {`-p`, `-pY`} = 38 runs, all identical: 86 291 of 86 291 dump
//! lines, and every reach counter equal.** The comparison can fail: making
//! `finddirection` take the other branch of one `randomnation(2)` choice
//! breaks `box_hole -p` at dump line 52 (tried and reverted);
//! `the_comparison_can_fail` pins a switch change. Per-case table, reach
//! counts and times: the V&V README.
use outram_park_fork_cfmesh::tetgen::mesh::{
    decode, Face, ShPool, TriFace, VertType, DUMMYPOINT, NULL_POINT, NULL_TET,
};
use outram_park_fork_cfmesh::tetgen::{tetrahedralize_plc, Behavior, PlcInput, TetgenMesh};
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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_data")
}

fn read_gz(dir: &str, name: &str) -> String {
    let p = root().join(dir).join(format!("{name}.gz"));
    let b = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    String::from_utf8(gunzip(&b)).unwrap()
}

/// The committed input of a case: stage 3's inputs, else this stage's.
fn input(case: &str) -> PlcInput {
    for dir in ["tetgen_meshsurface", "tetgen_recoverboundary"] {
        let d = root().join(dir);
        let mut plc = if d.join(format!("{case}.poly.gz")).exists() {
            PlcInput::read_poly(&read_gz(dir, &format!("{case}.poly"))).unwrap()
        } else if d.join(format!("{case}.smesh.gz")).exists() {
            PlcInput::read_smesh(&read_gz(dir, &format!("{case}.smesh"))).unwrap()
        } else {
            continue;
        };
        if d.join(format!("{case}.edge.gz")).exists() {
            plc.read_edge(&read_gz(dir, &format!("{case}.edge")))
                .unwrap();
        }
        return plc;
    }
    panic!("no input for {case}");
}

const CASES: [&str; 19] = [
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
    "schoenhardt",
    "thin_slab",
    "facet_wall",
    "facet_wall2",
    "skew_segments",
    "skew_points",
];

fn switches(y: bool) -> Behavior {
    Behavior {
        plc: true,
        nobisect: y as i32,
        ..Default::default()
    }
}

/// Run the port to the end of `recoverboundary` with `-p` (`y == false`)
/// or `-pY`; return the mesh, its dump and the time of the recovery.
fn run(case: &str, y: bool) -> (TetgenMesh, String, std::time::Duration) {
    let plc = input(case);
    let mut m =
        TetgenMesh::from_plc(&plc, &switches(y)).unwrap_or_else(|e| panic!("{case}: {e:?}"));
    let t0 = std::time::Instant::now();
    m.recoverboundary()
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));
    let dt = t0.elapsed();
    let d = m.dump_recoverboundary();
    (m, d, dt)
}

fn check(case: &str, y: bool) -> (TetgenMesh, String, std::time::Duration) {
    let (m, ours, dt) = run(case, y);
    let sw = if y { "pY" } else { "p" };
    let theirs = read_gz("tetgen_recoverboundary", &format!("{case}.{sw}.dump"));
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), theirs.lines().collect());
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(
            x,
            y,
            "{case} -{sw}: first difference at dump line {}",
            i + 1
        );
    }
    assert_eq!(a.len(), b.len(), "{case} -{sw}: dump lengths differ");
    // The routine and branch reach counts equal upstream's (`tg_reach`).
    let reach: Vec<u64> = read_gz("tetgen_recoverboundary", &format!("{case}.{sw}.reach"))
        .split_whitespace()
        .map(|x| x.parse().unwrap())
        .collect();
    assert_eq!(
        &m.reach[..55],
        &reach[..55],
        "{case} -{sw}: reach counts differ"
    );
    (m, ours, dt)
}

#[test]
fn recoverboundary_matches_upstream_on_every_case() {
    let mut lines = 0;
    for case in CASES {
        for y in [false, true] {
            let (m, dump, dt) = check(case, y);
            lines += dump.lines().count();
            println!(
                "{case:13} -{:2} lines {:6} identical  Steiner seg/fac/vol {}/{}/{}  \
                 recovery {:.3} ms",
                if y { "pY" } else { "p" },
                dump.lines().count(),
                m.st_segref_count,
                m.st_facref_count,
                m.st_volref_count,
                dt.as_secs_f64() * 1e3,
            );
        }
    }
    println!("{lines} dump lines identical");
}

/// The comparison can fail: the port run with another flip-link increment
/// (`fliplinklevelinc = 2`, upstream's default is 1) differs from upstream's
/// default dump on `fuel_channel -pY` (seven segment rounds).
#[test]
fn the_comparison_can_fail() {
    let theirs = read_gz("tetgen_recoverboundary", "fuel_channel.pY.dump");
    let b = Behavior {
        fliplinklevelinc: 2,
        ..switches(true)
    };
    let other = tetrahedralize_plc(&input("fuel_channel"), &b).unwrap();
    assert_ne!(other.mesh.dump_recoverboundary(), theirs);
}

/// Upstream-free checks after recovery, on every case: every tetrahedron is
/// positively oriented (exact `orient3d < 0`, TetGen's convention); every
/// subsegment is a mesh edge whose every tetrahedron links it, and the
/// subsegments of each input segment form a chain between its endpoints;
/// every subface is a face of the mesh, bonded on both sides.
#[test]
fn recovered_boundary_is_in_the_mesh() {
    for case in CASES {
        for y in [false, true] {
            let r = tetrahedralize_plc(&input(case), &switches(y)).unwrap();
            assert_eq!(r.missing_segments, 0, "{case}");
            let m = &r.mesh;
            // Tetrahedra.
            let mut ntet = 0;
            for t in m.tets.iter().take(m.tetpool.maxitems) {
                if t.v[0] == NULL_POINT || t.v[3] == DUMMYPOINT {
                    continue;
                }
                let o = m
                    .pred
                    .orient3d(m.xyz(t.v[0]), m.xyz(t.v[1]), m.xyz(t.v[2]), m.xyz(t.v[3]));
                assert!(o < 0.0, "{case}: tet {:?} not positively oriented", t.v);
                ntet += 1;
            }
            // Subsegments: mesh edges, linked from every tet at the edge.
            let mut chains: std::collections::BTreeMap<i32, Vec<[usize; 2]>> = Default::default();
            for (k, s) in m.subsegs.iter().enumerate().take(m.subsegpool.maxitems) {
                if s.v[0] == NULL_POINT {
                    continue;
                }
                let f = Face {
                    pool: ShPool::Seg,
                    sh: k,
                    shver: 0,
                };
                let t = m.sstpivot1(&f);
                assert_ne!(t.tet, NULL_TET, "{case}: subsegment {k} is not in the mesh");
                let e = [m.org(&t), m.dest(&t)];
                assert!(
                    (e == [s.v[0], s.v[1]]) || (e == [s.v[1], s.v[0]]),
                    "{case}: subsegment {k} linked to another edge"
                );
                let mut spin = t;
                loop {
                    assert_eq!(
                        m.tsspivot1(&spin).sh,
                        k,
                        "{case}: a tet at subsegment {k} lacks the link"
                    );
                    m.fnextself(&mut spin);
                    if spin.tet == t.tet {
                        break;
                    }
                }
                chains
                    .entry(s.facetindex)
                    .or_default()
                    .push([s.v[0], s.v[1]]);
            }
            for (idx, segs) in &chains {
                let (a, b) = (
                    m.segmentendpointslist[*idx as usize * 2],
                    m.segmentendpointslist[*idx as usize * 2 + 1],
                );
                // Walk from a to b.
                let mut cur = a;
                let mut used = vec![false; segs.len()];
                for _ in 0..segs.len() {
                    let j = (0..segs.len())
                        .find(|&j| !used[j] && (segs[j][0] == cur || segs[j][1] == cur))
                        .unwrap_or_else(|| panic!("{case}: segment {idx} is not a chain"));
                    used[j] = true;
                    cur = if segs[j][0] == cur {
                        segs[j][1]
                    } else {
                        segs[j][0]
                    };
                }
                assert_eq!(
                    cur, b,
                    "{case}: segment {idx} chain does not end at its endpoint"
                );
            }
            // Subfaces: faces of the mesh, bonded on both sides.
            let mut nsub = 0;
            for (k, s) in m.subfaces.iter().enumerate().take(m.subfacepool.maxitems) {
                if s.v[0] == NULL_POINT {
                    continue;
                }
                for shver in [0, 1] {
                    let f = Face {
                        pool: ShPool::Sub,
                        sh: k,
                        shver,
                    };
                    let t = m.stpivot(&f);
                    assert_ne!(
                        t.tet, NULL_TET,
                        "{case}: subface {k} side {shver} is not bonded"
                    );
                    let mut tv = [m.org(&t), m.dest(&t), m.apex(&t)];
                    let mut sv = s.v;
                    tv.sort();
                    sv.sort();
                    assert_eq!(tv, sv, "{case}: subface {k} bonded to another face");
                    assert_eq!(
                        m.tspivot(&t).sh,
                        k,
                        "{case}: the tet at subface {k} lacks the link"
                    );
                }
                nsub += 1;
            }
            let steiner = m
                .points
                .iter()
                .skip(1)
                .take(m.pointpool.maxitems)
                .filter(|p| {
                    matches!(
                        p.vtype,
                        VertType::FreeSeg | VertType::FreeFacet | VertType::FreeVol
                    )
                })
                .count();
            println!(
                "{case:13} -{:2} tets {ntet:6} subfaces {nsub:5} segments {:4} Steiner points left {steiner}",
                if y { "pY" } else { "p" },
                chains.len()
            );
        }
    }
}

/// `scoutsegment` and `scoutsubface` (the CDT path's, not reached by
/// `recoverboundary`): on a point-set Delaunay mesh, a mesh edge is
/// `SHAREEDGE` and a missing edge reports a crossing; on `unit_box` after
/// `meshsurface`, the subfaces that are Delaunay faces are found and bonded.
#[test]
fn scout_routines_find_edges_and_faces() {
    use outram_park_fork_cfmesh::tetgen::flip::{ACROSSEDGE, ACROSSFACE, SHAREEDGE};
    let mut pts = Vec::new();
    let mut s: u64 = 7;
    for _ in 0..60 {
        let mut c = [0.0; 3];
        for x in c.iter_mut() {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *x = (s >> 11) as f64 / (1u64 << 53) as f64;
        }
        pts.push(c);
    }
    let mut m = TetgenMesh::new(Behavior::default());
    m.transfernodes(&pts).unwrap();
    m.incrementaldelaunay().unwrap();
    // A mesh edge.
    let t = (0..m.tetpool.maxitems)
        .find(|&k| m.tets[k].v[0] != NULL_POINT && m.tets[k].v[3] != DUMMYPOINT)
        .unwrap();
    let (a, b) = (m.tets[t].v[0], m.tets[t].v[1]);
    let mut st = TriFace::default();
    assert_eq!(m.scoutsegment(a, b, &mut st, None).unwrap(), SHAREEDGE);
    assert_eq!((m.org(&st), m.dest(&st)), (a, b));
    // A missing edge (the two points furthest apart).
    let (mut a, mut b, mut dmax) = (1, 2, 0.0);
    for i in 1..=pts.len() {
        for j in i + 1..=pts.len() {
            let (p, q) = (m.xyz(i), m.xyz(j));
            let d = (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>();
            if d > dmax {
                (a, b, dmax) = (i, j, d);
            }
        }
    }
    let mut refpt = NULL_POINT;
    let dir = m.scoutsegment(a, b, &mut st, Some(&mut refpt)).unwrap();
    assert!(dir == ACROSSFACE || dir == ACROSSEDGE, "dir {dir}");
    assert_ne!(refpt, NULL_POINT, "a vertex encroaches upon the diagonal");

    let plc = input("unit_box");
    let mut m = TetgenMesh::from_plc(&plc, &switches(false)).unwrap();
    let mut found = 0;
    for k in 0..m.subfacepool.maxitems {
        if m.subfaces[k].v[0] == NULL_POINT {
            continue;
        }
        let mut f = Face {
            pool: ShPool::Sub,
            sh: k,
            shver: 0,
        };
        let mut t = TriFace::default();
        if m.scoutsubface(&mut f, &mut t, true).unwrap() {
            found += 1;
            assert_ne!(decode(m.subfaces[k].tet[0]).tet, NULL_TET);
        }
    }
    assert!(found > 0, "no subface of unit_box is a Delaunay face");
}

/// Development aid: write the port's dumps for the cases named in
/// `GH1793_CASES` (comma-separated `case.p` / `case.pY`) to `GH1793_OUT`.
#[test]
#[ignore]
fn write_port_dumps() {
    let out = PathBuf::from(std::env::var("GH1793_OUT").expect("GH1793_OUT"));
    for c in std::env::var("GH1793_CASES").unwrap().split(',') {
        let (case, sw) = c.split_once('.').unwrap();
        let (_, d, _) = run(case, sw == "pY");
        std::fs::write(out.join(format!("{case}.{sw}.port")), d).unwrap();
    }
}
