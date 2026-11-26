// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! TetGen port, stage 6 (gh:#1795, epic gh:#1789): Delaunay refinement,
//! `-q` and `-a` (`tetgen::refine`), through the whole `-p` route, against
//! the **unmodified** upstream binary.
//!
//! **Methodology.** Reference: TetGen 1.6.0 (libigl/tetgen `e05aca7df74e`),
//! the `tetgen` binary of the upstream clone, run as `tetgen -<sw> <file>`
//! **without `-Q`**, x86_64, glibc 2.39, on the stage 5 cases under `-pqz`,
//! a subset under `-pq1.2z`, `-pq1.414/12z` and `-pqa<V>z`, `-pqAz` on
//! `fuel_pin_regions`, and the Mesh Studio pipe, fuel channel and pebble
//! under `-pq1.414a<V>z` ([`runs`]). Its `.node`, `.ele`, `.face`, `.edge`
//! files and the 64 stage 6 reach counters of the instrumented build
//! (`tg6`, `verification_and_validation/tetgen_quality_gh1795/stage6_reach.patch`,
//! which writes the same files) are committed gzipped in
//! `test_data/tetgen_quality/` (a `/` in a switch string is `_` in the
//! file names). **Pass criterion: every file equal byte for byte and every
//! reach counter equal.** No tolerance.
//!
//! **Results (2026-11-26):** see
//! `verification_and_validation/tetgen_quality_gh1795/README.md` (the
//! per-run table, the reach counts, the quality statistics).

use outram_park_fork_cfmesh::tetgen::{tetrahedralize_plc, PlcInput, Switches, TetgenOutput};
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

fn read_gz(dir: &str, name: &str) -> Option<String> {
    let p = root().join(dir).join(format!("{name}.gz"));
    let b = std::fs::read(&p).ok()?;
    Some(String::from_utf8(gunzip(&b)).unwrap())
}

/// A reference file: from `TETGEN_QUALITY_REF=<dir>` (plain files, for
/// regenerating) if set, else the committed one.
fn reference(name: &str) -> Option<String> {
    if let Ok(d) = std::env::var("TETGEN_QUALITY_REF") {
        return std::fs::read_to_string(format!("{d}/{name}")).ok();
    }
    read_gz("tetgen_quality", name)
}

/// The committed input of a case and its file name.
fn input(case: &str) -> (PlcInput, String) {
    for dir in ["tetgen_meshsurface", "tetgen_recoverboundary", "tetgen_plc"] {
        let (mut plc, name) = if let Some(t) = read_gz(dir, &format!("{case}.poly")) {
            (PlcInput::read_poly(&t).unwrap(), format!("{case}.poly"))
        } else if let Some(t) = read_gz(dir, &format!("{case}.smesh")) {
            (PlcInput::read_smesh(&t).unwrap(), format!("{case}.smesh"))
        } else {
            continue;
        };
        if let Some(e) = read_gz(dir, &format!("{case}.edge")) {
            plc.read_edge(&e).unwrap();
        }
        return (plc, name);
    }
    panic!("no input for {case}");
}

/// Stage 5's 21 cases.
const CASES: [&str; 21] = [
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
    "fuel_pin_hole",
    "fuel_pin_regions",
];

/// Every compared run: `(case, switches)`.
fn runs() -> Vec<(&'static str, &'static str)> {
    let mut v = Vec::new();
    for c in CASES {
        v.push((c, "pqz"));
    }
    for c in ["example", "comb_prism", "pipe", "sphere", "skew_points"] {
        v.push((c, "pq1.2z"));
    }
    for c in [
        "example",
        "box_hole",
        "comb_prism",
        "slit_box",
        "fuel_channel",
    ] {
        v.push((c, "pq1.414/12z"));
    }
    v.push(("example", "pqa0.01z"));
    v.push(("unit_box", "pqa0.002z"));
    v.push(("box_hole", "pqa0.01z"));
    v.push(("pipe", "pqa0.005z"));
    v.push(("fuel_pin_regions", "pqAz"));
    v.extend(mesh_studio_runs());
    if std::env::var("TETGEN_QUALITY_REF").is_ok() && std::env::var("TETGEN_QUALITY_EXTRA").is_ok()
    {
        // Run in the README record only (their files would take the
        //   committed data past ~5 MB): read from TETGEN_QUALITY_REF.
        v.push(("pebble", "pq1.414/12z"));
        v.push(("facet_wall", "pq1.414/12z"));
    }
    v
}

/// The Mesh Studio shapes (size 2) at a real mesh's density.
fn mesh_studio_runs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("pipe", "pq1.414a0.0026z"),
        ("fuel_channel", "pq1.414a0.0026z"),
        ("pebble", "pq1.414a0.0026z"),
    ]
}

/// The file-name form of a switch string.
fn tag(sw: &str) -> String {
    sw.replace('/', "_")
}

fn run(case: &str, sw: &str) -> (TetgenOutput, String, std::time::Duration, [u64; 64]) {
    let (plc, file) = input(case);
    let b = Switches::parse(sw).unwrap().behavior();
    let t0 = std::time::Instant::now();
    let r = tetrahedralize_plc(&plc, &b).unwrap_or_else(|e| panic!("{case} -{sw}: {e:?}"));
    let dt = t0.elapsed();
    (
        r.output.unwrap(),
        format!("tetgen -{sw} {file} "),
        dt,
        r.mesh.reach6,
    )
}

/// The four files, as the port writes them.
fn files(out: &TetgenOutput, cmd: &str) -> [(&'static str, Option<String>); 4] {
    [
        ("node", Some(out.node_text(cmd))),
        ("ele", Some(out.ele_text(cmd))),
        ("face", out.face_text(cmd)),
        ("edge", Some(out.edge_text(cmd))),
    ]
}

/// Every run's four files equal upstream's byte for byte and its 64 reach
/// counters equal the instrumented upstream's. `TETGEN_QUALITY_WRITE=<dir>`
/// writes the port's files; `TETGEN_QUALITY_ONLY=<sw>` runs one switch
/// string only.
#[test]
fn quality_output_matches_upstream_byte_for_byte() {
    let write = std::env::var("TETGEN_QUALITY_WRITE").ok();
    let only = std::env::var("TETGEN_QUALITY_ONLY").ok();
    let mut bad = Vec::new();
    let mut nfiles = 0;
    for (case, sw) in runs() {
        if only.as_deref().is_some_and(|o| o != sw && o != case) {
            continue;
        }
        let (out, cmd, dt, reach) = run(case, sw);
        let t = tag(sw);
        match reference(&format!("{case}.{t}.reach6")) {
            Some(r) => {
                let theirs: Vec<u64> = r.split_whitespace().map(|x| x.parse().unwrap()).collect();
                if theirs[..] != reach[..] {
                    bad.push(format!(
                        "{case}.{t}.reach6: ours {:?} theirs {:?}",
                        &reach[..48],
                        &theirs[..48]
                    ));
                }
            }
            None => bad.push(format!("{case}.{t}.reach6 missing")),
        }
        let mut same = 0;
        for (ext, text) in files(&out, &cmd) {
            let theirs = reference(&format!("{case}.{t}.{ext}"));
            if let (Some(d), Some(x)) = (&write, &text) {
                std::fs::write(format!("{d}/{case}.{t}.{ext}"), x).unwrap();
            }
            nfiles += 1;
            if text == theirs {
                same += 1;
            } else {
                bad.push(format!("{case}.{t}.{ext}"));
            }
        }
        println!(
            "{case:17} -{sw:15} nodes {:6} tets {:7} faces {:6} edges {:5} files {same}/4 {:9.3} ms",
            out.points.len(),
            out.tets.len(),
            out.faces.as_ref().map_or(0, |f| f.len()),
            out.edges.len(),
            dt.as_secs_f64() * 1e3
        );
    }
    assert!(
        bad.is_empty(),
        "{} of {nfiles} files differ: {bad:?}",
        bad.len()
    );
}

/// The `-V` "Mesh quality statistics" block of the Mesh Studio runs (and
/// their `-pqz` runs): the port's `TetgenMesh::qualitystatistics` text
/// equals upstream's (`tetgen -<sw>V`, which writes the same files) byte
/// for byte. The block is printed here for the README record.
#[test]
fn quality_statistics_match_upstream_dash_v() {
    let mut runs = mesh_studio_runs();
    runs.extend([("pipe", "pqz"), ("fuel_channel", "pqz"), ("pebble", "pqz")]);
    for (case, sw) in runs {
        let (plc, _) = input(case);
        let b = Switches::parse(sw).unwrap().behavior();
        let mut r = tetrahedralize_plc(&plc, &b).unwrap();
        let ours = r.mesh.qualitystatistics();
        let q = r.mesh.quality_numbers();
        let theirs = read_gz("tetgen_quality", &format!("{case}.{}.quality", tag(sw))).unwrap();
        println!(
            "{case} -{sw}: largest radius-edge ratio {:.6}, dihedral {:.4} .. {:.4}\n{ours}",
            q.radius_edge_ratio[1], q.dihedral[0], q.dihedral[1]
        );
        assert_eq!(ours, theirs, "{case} -{sw}");
    }
}

/// The comparison can fail: the volume bound perturbed from 0.0026 to
/// 0.0027 changes the Mesh Studio pipe's `.ele`. (Perturbing the ratio
/// bound to 1.415 does not: every tetrahedron of that mesh is below 1.29,
/// the volume bound governs it.)
#[test]
fn the_comparison_can_fail() {
    let (plc, file) = input("pipe");
    let sw = "pq1.414a0.0026z";
    let mut b = Switches::parse(sw).unwrap().behavior();
    b.maxvolume = 0.0027;
    let out = tetrahedralize_plc(&plc, &b).unwrap().output.unwrap();
    let cmd = format!("tetgen -{sw} {file} ");
    assert_ne!(
        Some(out.ele_text(&cmd)),
        read_gz("tetgen_quality", &format!("pipe.{}.ele", tag(sw)))
    );
}

/// The circumradius over the shortest edge, and the volume, of a
/// tetrahedron (plain `f64`, independent of the port).
fn ratio_and_volume(p: [[f64; 3]; 4]) -> (f64, f64) {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let (a, b, c) = (sub(p[1], p[0]), sub(p[2], p[0]), sub(p[3], p[0]));
    let det = dot(a, cross(b, c));
    // Circumcentre offset: (|a|^2 (b x c) + |b|^2 (c x a) + |c|^2 (a x b)) / (2 det).
    let (bc, ca, ab) = (cross(b, c), cross(c, a), cross(a, b));
    let (la, lb, lc) = (dot(a, a), dot(b, b), dot(c, c));
    let o = [
        (la * bc[0] + lb * ca[0] + lc * ab[0]) / (2.0 * det),
        (la * bc[1] + lb * ca[1] + lc * ab[1]) / (2.0 * det),
        (la * bc[2] + lb * ca[2] + lc * ab[2]) / (2.0 * det),
    ];
    let r = dot(o, o).sqrt();
    let mut emin = f64::MAX;
    for i in 0..4 {
        for j in i + 1..4 {
            let e = sub(p[i], p[j]);
            emin = emin.min(dot(e, e).sqrt());
        }
    }
    (r / emin, det.abs() / 6.0)
}

/// Upstream-free sanity of the refined meshes. After `-pq1.414` (and
/// `-pq1.2`) no tetrahedron's radius-edge ratio is above the bound, and
/// with `-a<V>` none is larger than `V`, **except** tetrahedra upstream's
/// own `.ele` also holds: TetGen leaves a tetrahedron whose circumcentre
/// would encroach upon a sharp segment or corner, or is not visible from
/// it, and retries it only a bounded number of times. **Finding:** the
/// volume bound is not a guarantee either: every `-a` run leaves some
/// tetrahedra above it (Mesh Studio pipe `-pq1.414a0.0026z`: 91 of 13 217,
/// the largest 3.45e-3), all of them upstream's too. The counts are
/// printed for the README record.
#[test]
fn refined_meshes_meet_their_bounds() {
    let parse_ele = |t: &str| -> std::collections::BTreeSet<[i32; 4]> {
        t.lines()
            .skip(1)
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                let w: Vec<i32> = l
                    .split_whitespace()
                    .skip(1)
                    .take(4)
                    .map(|x| x.parse().unwrap())
                    .collect();
                let mut k = [w[0], w[1], w[2], w[3]];
                k.sort();
                k
            })
            .collect()
    };
    for (case, sw) in runs() {
        let bound = if sw.starts_with("pq1.414") {
            Some(1.414)
        } else if sw.starts_with("pq1.2") {
            Some(1.2)
        } else {
            None
        };
        let vmax = sw
            .split('a')
            .nth(1)
            .and_then(|s| s.trim_end_matches('z').parse::<f64>().ok());
        if bound.is_none() && vmax.is_none() {
            continue;
        }
        let (out, ..) = run(case, sw);
        let pt = |i: i32| out.points[(i - out.firstindex) as usize];
        let mut above = Vec::new();
        let mut larger = Vec::new();
        let (mut worst, mut biggest) = (0.0f64, 0.0f64);
        for t in &out.tets {
            let (ratio, vol) = ratio_and_volume([pt(t[0]), pt(t[1]), pt(t[2]), pt(t[3])]);
            worst = worst.max(ratio);
            biggest = biggest.max(vol);
            if let Some(b) = bound {
                if ratio > b * (1.0 + 1e-9) {
                    let mut k = *t;
                    k.sort();
                    above.push(k);
                }
            }
            if let Some(v) = vmax {
                if vol > v * (1.0 + 1e-9) {
                    let mut k = *t;
                    k.sort();
                    larger.push(k);
                }
            }
        }
        let n_above = above.len();
        let n_larger = larger.len();
        above.extend(larger);
        if !above.is_empty() {
            let theirs =
                parse_ele(&read_gz("tetgen_quality", &format!("{case}.{}.ele", tag(sw))).unwrap());
            for k in &above {
                assert!(
                    theirs.contains(k),
                    "{case} -{sw}: tet {k:?} above the bound is not upstream's"
                );
            }
        }
        println!(
            "{case:13} -{sw:16} tets {:6}  above the ratio bound {n_above:4}  above the volume bound {n_larger:3} (all upstream's too)  worst ratio {worst:.4}  largest volume {biggest:.4e}",
            out.tets.len(),
        );
    }
}

/// The builders give what the parser gives.
#[test]
fn switch_builders_equal_the_parser() {
    let b = Switches::new().quality(1.414, 12.0).zero_index().behavior();
    assert_eq!(b, Switches::parse("pq1.414/12z").unwrap().behavior());
    let b = Switches::new()
        .quality(1.414, 3.5)
        .max_volume(0.0026)
        .zero_index()
        .behavior();
    assert_eq!(b, Switches::parse("pq1.414a0.0026z").unwrap().behavior());
    let b = Switches::new().max_volume(0.01).zero_index().behavior();
    let p = Switches::parse("pqa0.01z").unwrap().behavior();
    assert_eq!(b, p);
    assert!(p.quality && p.fixedvolume && p.minratio == 2.0 && p.mindihedral == 3.5);
    assert!(Switches::parse("pqaaz").is_err());
}
