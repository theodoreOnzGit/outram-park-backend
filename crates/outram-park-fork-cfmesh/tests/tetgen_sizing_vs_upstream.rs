// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! TetGen port, `-m` mesh sizes (gh:#1825, epic gh:#1789): `.mtr` point
//! sizes (`tetgen::sizing`, and the size branches of `insertpoint`,
//! `checktet4split`, the refinement splits, the Steiner-point routines and
//! `meshcoarsening`), through the whole `-p` route, against the
//! **unmodified** upstream binary.
//!
//! **Methodology.** Reference: TetGen 1.6.0 (libigl/tetgen `e05aca7df74e`),
//! the `tetgen` binary of the upstream clone, run as `tetgen -<sw> <file>`
//! **without `-Q`**, x86_64, glibc 2.39, with a `<file base>.mtr` beside the
//! input ([`runs`]): stage 5/6 cases with sizes from three rules (`grad`:
//! growing along x, `holes`: `grad` with every fifth point unsized, `coarse`:
//! a quarter of the box diagonal, so `meshcoarsening` removes interior
//! points), under `-pqmz`, `-pmz`, `-pqm2z`, `-pqAmz`, `-pq1.414a<V>mz`,
//! and without `-m` (`-pz`, `-pqz`: a `.mtr` is read but only the
//! insertion radius sees it); the stage 3 Mesh Studio surfaces with the
//! `grad` sizes; and the Mesh Studio pipe, fuel channel and pebble with the
//! wall grading of `pipeline::tet_source` (`*_graded`: seed points and
//! sizes from `tetgen_plc`). Its `.node`, `.ele`, `.face`, `.edge`, `.mtr`
//! and `.p2t` files, and the reach counters of the instrumented build
//! (`tg5`, `tg6` and the 32 `tgm` counters of
//! `verification_and_validation/tetgen_sizing_gh1825/sizing_reach.patch`, which
//! writes the same files), are committed gzipped in
//! `test_data/tetgen_sizing/` with the `.mtr` inputs (a `/` in a switch
//! string is `_` in the file names). **Pass criterion: every file equal
//! byte for byte and every reach counter equal.** No tolerance.
//!
//! **Results (2026-11-26):** see
//! `verification_and_validation/tetgen_sizing_gh1825/README.md`.

use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::tet_source::{tetgen_plc, TetSource, WallGrade};
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

/// A reference file: from `TETGEN_SIZING_REF=<dir>` (plain files, for
/// regenerating) if set, else the committed one.
fn reference(name: &str) -> Option<String> {
    if let Ok(d) = std::env::var("TETGEN_SIZING_REF") {
        return std::fs::read_to_string(format!("{d}/{name}")).ok();
    }
    read_gz("tetgen_sizing", name)
}

/// The committed geometry of a case and its extension.
fn geometry(case: &str) -> (PlcInput, &'static str) {
    for dir in [
        "tetgen_sizing",
        "tetgen_meshsurface",
        "tetgen_recoverboundary",
        "tetgen_plc",
    ] {
        let (mut plc, ext) = if let Some(t) = read_gz(dir, &format!("{case}.poly")) {
            (PlcInput::read_poly(&t).unwrap(), "poly")
        } else if let Some(t) = read_gz(dir, &format!("{case}.smesh")) {
            (PlcInput::read_smesh(&t).unwrap(), "smesh")
        } else {
            continue;
        };
        if let Some(e) = read_gz(dir, &format!("{case}.edge")) {
            plc.read_edge(&e).unwrap();
        }
        return (plc, ext);
    }
    panic!("no input for {case}");
}

/// The input `<case>_<rule>`: the case's geometry and the committed
/// `<case>_<rule>.mtr` (as `load_plc` reads it), and the file name the
/// upstream run was given.
fn input(name: &str) -> (PlcInput, String) {
    let (case, _) = name.rsplit_once('_').unwrap();
    let (mut plc, ext) = geometry(if name.ends_with("_graded") {
        name
    } else {
        case
    });
    let mtr =
        read_gz("tetgen_sizing", &format!("{name}.mtr")).unwrap_or_else(|| panic!("no {name}.mtr"));
    assert!(plc.read_mtr(&mtr).unwrap(), "{name}.mtr ignored");
    (plc, format!("{name}.{ext}"))
}

/// Every compared run: `(input, switches)`.
fn runs() -> Vec<(&'static str, &'static str)> {
    let mut v = Vec::new();
    for c in [
        "example_grad",
        "box_hole_grad",
        "comb_prism_grad",
        "slit_box_grad",
        "skew_points_grad",
        "sphere_grad",
        "example_holes",
        "fuel_pin_hole_grad",
    ] {
        v.push((c, "pqmz"));
    }
    for c in [
        "skew_points_coarse",
        "skew_segments_coarse",
        "facet_wall_coarse",
    ] {
        v.push((c, "pmz"));
        v.push((c, "pqmz"));
    }
    v.push(("example_grad", "pmz"));
    v.push(("example_grad", "pqm2z"));
    v.push(("example_grad", "pq1.414a0.01mz"));
    v.push(("example_grad", "pz"));
    v.push(("example_grad", "pqz"));
    v.push(("fuel_pin_regions_grad", "pqAmz"));
    // Point markers -1: `collectremovepoints` lists them too.
    v.push(("skew_points_m1_grad", "pmz"));
    v.push(("skew_points_m1_grad", "pqmz"));
    for c in ["pipe_grad", "fuel_channel_grad", "pebble_grad"] {
        v.push((c, "pq1.414a0.0026mz"));
    }
    v.extend(mesh_studio_runs());
    v
}

/// The Mesh Studio shapes (size 2) wall-graded as the page grades them
/// (`?grade=on`: walls half the cell size h, growth 1.2, 2h inside; `-a`
/// the regular tetrahedron of 2h, rounded; h is 0.25 m for the pipe, 1/6 m
/// for the others).
fn mesh_studio_runs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("pipe_graded", "pq1.414a0.0147mz"),
        ("fuel_channel_graded", "pq1.414a0.00436mz"),
        ("pebble_graded", "pq1.414a0.00436mz"),
    ]
}

/// The file-name form of a switch string.
fn tag(sw: &str) -> String {
    sw.replace('/', "_")
}

struct Run {
    out: TetgenOutput,
    cmd: String,
    dt: std::time::Duration,
    reach5: [u64; 64],
    reach6: [u64; 64],
    reachm: [u64; 32],
}

fn run(name: &str, sw: &str) -> Run {
    let (plc, file) = input(name);
    let b = Switches::parse(sw).unwrap().behavior();
    let t0 = std::time::Instant::now();
    let r = tetrahedralize_plc(&plc, &b).unwrap_or_else(|e| panic!("{name} -{sw}: {e:?}"));
    let dt = t0.elapsed();
    Run {
        out: r.output.unwrap(),
        cmd: format!("tetgen -{sw} {file} "),
        dt,
        reach5: r.mesh.reach5,
        reach6: r.mesh.reach6,
        reachm: r.mesh.reachm,
    }
}

/// The six files, as the port writes them.
fn files(out: &TetgenOutput, cmd: &str) -> [(&'static str, Option<String>); 6] {
    [
        ("node", Some(out.node_text(cmd))),
        ("ele", Some(out.ele_text(cmd))),
        ("face", out.face_text(cmd)),
        ("edge", Some(out.edge_text(cmd))),
        ("mtr", out.mtr_text()),
        ("p2t", out.p2t_text(cmd)),
    ]
}

fn counters(r: &str) -> Vec<u64> {
    r.split_whitespace().map(|x| x.parse().unwrap()).collect()
}

/// Every run's files equal upstream's byte for byte (a file upstream does
/// not write, `.mtr` and `.p2t` without `-m`, the port does not either)
/// and its `tg5`, `tg6` and `tgm` counters equal the instrumented
/// upstream's. `TETGEN_SIZING_WRITE=<dir>` writes the port's files;
/// `TETGEN_SIZING_ONLY=<input or sw>` runs a subset.
#[test]
fn sizing_output_matches_upstream_byte_for_byte() {
    let write = std::env::var("TETGEN_SIZING_WRITE").ok();
    let only = std::env::var("TETGEN_SIZING_ONLY").ok();
    let mut bad = Vec::new();
    let mut nfiles = 0;
    for (name, sw) in runs() {
        if only.as_deref().is_some_and(|o| o != sw && o != name) {
            continue;
        }
        let r = run(name, sw);
        let t = tag(sw);
        for (ext, ours) in [
            ("reach5", &r.reach5[..]),
            ("reach6", &r.reach6[..]),
            ("reachm", &r.reachm[..]),
        ] {
            match reference(&format!("{name}.{t}.{ext}")) {
                Some(x) => {
                    let theirs = counters(&x);
                    if theirs[..] != ours[..] {
                        bad.push(format!("{name}.{t}.{ext}: ours {ours:?} theirs {theirs:?}"));
                    }
                }
                None => bad.push(format!("{name}.{t}.{ext} missing")),
            }
        }
        let mut same = 0;
        let mut n = 0;
        for (ext, text) in files(&r.out, &r.cmd) {
            let theirs = reference(&format!("{name}.{t}.{ext}"));
            if let (Some(d), Some(x)) = (&write, &text) {
                std::fs::write(format!("{d}/{name}.{t}.{ext}"), x).unwrap();
            }
            if text.is_none() && theirs.is_none() {
                continue;
            }
            n += 1;
            nfiles += 1;
            if text == theirs {
                same += 1;
            } else {
                bad.push(format!("{name}.{t}.{ext}"));
            }
        }
        println!(
            "{name:22} -{sw:18} nodes {:6} tets {:7} faces {:6} edges {:5} files {same}/{n} \
             size-split tets {:6} ENCVERTEX {:4} coarsened {:3}/{:3} {:9.3} ms",
            r.out.points.len(),
            r.out.tets.len(),
            r.out.faces.as_ref().map_or(0, |f| f.len()),
            r.out.edges.len(),
            r.reachm[7],
            r.reachm[6],
            r.reachm[11],
            r.reachm[9] + r.reachm[13],
            r.dt.as_secs_f64() * 1e3
        );
    }
    assert!(
        bad.is_empty(),
        "{} differences in {nfiles} files: {bad:?}",
        bad.len()
    );
}

/// The comparison can fail: the size of one wall point of the Mesh Studio
/// graded pipe raised by 1 % changes its `.ele` (not only the `.mtr`,
/// which holds the size itself), and the run without `-m` differs from the
/// `-m` reference.
#[test]
fn the_comparison_can_fail() {
    let (name, sw) = mesh_studio_runs()[0];
    let (mut plc, file) = input(name);
    plc.point_mtrs[1] *= 1.01; // a rim point of the inlet: on the wall
    let b = Switches::parse(sw).unwrap().behavior();
    let out = tetrahedralize_plc(&plc, &b).unwrap().output.unwrap();
    let cmd = format!("tetgen -{sw} {file} ");
    let t = tag(sw);
    assert_ne!(
        Some(out.ele_text(&cmd)),
        read_gz("tetgen_sizing", &format!("{name}.{t}.ele"))
    );
    let (plc, _) = input(name);
    let mut b = Switches::parse(sw).unwrap().behavior();
    b.metric = false;
    let out = tetrahedralize_plc(&plc, &b).unwrap().output.unwrap();
    assert_ne!(
        Some(out.ele_text(&cmd)),
        read_gz("tetgen_sizing", &format!("{name}.{t}.ele"))
    );
}

/// `-m` parses as upstream's (`-m#` is the scale) and does not imply `-q`;
/// `read_mtr` ignores what `load_mtr` ignores and refuses anisotropic
/// sizes.
#[test]
fn switches_and_mtr_reading_follow_upstream() {
    let b = Switches::parse("pqm2.5e-1z").unwrap().behavior();
    assert!(b.metric && b.quality && b.zeroindex);
    assert_eq!(b.metric_scale, 0.25);
    let b = Switches::parse("pm").unwrap().behavior();
    assert!(b.metric && !b.quality && b.metric_scale == 1.0);
    assert_eq!(b, Switches::new().metric(1.0).behavior());
    let (mut plc, _) = geometry("example");
    let n = plc.points.len();
    let one: String = format!("{n} 1\n") + &"0.5\n".repeat(n);
    assert!(plc.read_mtr(&one).unwrap());
    assert_eq!(plc.point_mtrs, vec![0.5; n]);
    let mut p2 = plc.clone();
    p2.point_mtrs.clear();
    assert!(!p2.read_mtr(&format!("{} 1\n0.5\n", n + 1)).unwrap());
    assert!(!p2.read_mtr(&format!("{n} 2\n")).unwrap());
    assert!(p2.point_mtrs.is_empty());
    assert!(p2.read_mtr(&format!("{n} 3\n")).is_err());
    assert!(p2.read_mtr(&format!("{n} 1\n0.5\n")).is_err());
}

// ---------------------------------------------------------------------------
// The inputs (written once; the committed `.mtr` files are what the test
// reads).
// ---------------------------------------------------------------------------

/// The size of a point under a rule, `bb` the box `[min, max]` of the
/// case's points.
fn rule_size(rule: &str, i: usize, p: [f64; 3], bb: ([f64; 3], [f64; 3])) -> f64 {
    let d = [bb.1[0] - bb.0[0], bb.1[1] - bb.0[1], bb.1[2] - bb.0[2]];
    let diag = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let fx = if d[0] > 0.0 {
        (p[0] - bb.0[0]) / d[0]
    } else {
        0.0
    };
    match rule {
        "grad" => diag * (0.04 + 0.12 * fx),
        "holes" if i % 5 == 0 => 0.0,
        "holes" => diag * (0.04 + 0.12 * fx),
        "coarse" => 0.25 * diag,
        _ => panic!("rule {rule}"),
    }
}

/// The surface of a stage 3 `.smesh` (one triangle per facet) as a triangle
/// soup with its markers as regions (`names[m]` = `m`).
fn soup(plc: &PlcInput) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
    let p: Vec<Vec3> = plc
        .points
        .iter()
        .map(|q| Vec3::new(q[0], q[1], q[2]))
        .collect();
    let f = plc.firstnumber as usize;
    let t: Vec<[usize; 3]> = plc
        .facets
        .iter()
        .map(|fa| {
            let q = &fa.polygons[0];
            [q[0] as usize - f, q[1] as usize - f, q[2] as usize - f]
        })
        .collect();
    let m: Vec<usize> = plc
        .facet_markers
        .as_ref()
        .unwrap()
        .iter()
        .map(|&x| x as usize)
        .collect();
    let nm = m.iter().max().unwrap() + 1;
    let r = SurfaceRegions {
        names: (0..nm).map(|k| k.to_string()).collect(),
        region_of_tri: m,
        types: vec!["patch".into(); nm],
    };
    (p, t, r)
}

/// The wall size of the Mesh Studio runs, as a fraction of the cell size.
const WALL_FRACTION: f64 = 0.5;

/// Writes every `.mtr` input, and the graded Mesh Studio `.smesh` inputs,
/// to `TETGEN_SIZING_INPUTS=<dir>` (plain files; gzip them into
/// `test_data/tetgen_sizing/`). Run with `--ignored`.
#[test]
#[ignore]
fn write_sizing_inputs() {
    let dir = std::env::var("TETGEN_SIZING_INPUTS").expect("TETGEN_SIZING_INPUTS");
    let mut names: Vec<&str> = runs().iter().map(|r| r.0).collect();
    names.dedup();
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        if !seen.insert(name) || name.ends_with("_graded") {
            continue;
        }
        let (case, rule) = name.rsplit_once('_').unwrap();
        let (plc, ext) = geometry(case);
        let (mut lo, mut hi) = (plc.points[0], plc.points[0]);
        for q in &plc.points {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let mut s = format!("{} 1\n", plc.points.len());
        for (i, q) in plc.points.iter().enumerate() {
            s += &format!("{:?}\n", rule_size(rule, i, *q, (lo, hi)));
        }
        std::fs::write(format!("{dir}/{name}.mtr"), s).unwrap();
        // The geometry, under the input's name, for the upstream run.
        let src = [
            "tetgen_sizing",
            "tetgen_meshsurface",
            "tetgen_recoverboundary",
            "tetgen_plc",
        ]
        .iter()
        .find_map(|d| read_gz(d, &format!("{case}.{ext}")))
        .unwrap();
        std::fs::write(format!("{dir}/{name}.{ext}"), src).unwrap();
        if let Some(e) = ["tetgen_meshsurface", "tetgen_recoverboundary", "tetgen_plc"]
            .iter()
            .find_map(|d| read_gz(d, &format!("{case}.edge")))
        {
            std::fs::write(format!("{dir}/{name}.edge"), e).unwrap();
        }
    }
    // The Mesh Studio shapes: markers inlet 1, outlet 2, walls / pin /
    //   pebble 3, channel 4 (stage 3's `reactor_surface`); walls 3 and 4.
    for (shape, h, sw) in [
        ("pipe", 0.25, mesh_studio_runs()[0].1),
        ("fuel_channel", 1.0 / 6.0, mesh_studio_runs()[1].1),
        ("pebble", 1.0 / 6.0, mesh_studio_runs()[2].1),
    ] {
        let (plc, _) = geometry(shape);
        let (p, t, r) = soup(&plc);
        let v: f64 = sw[sw.find('a').unwrap() + 1..sw.len() - 2].parse().unwrap();
        // `TETGEN_SIZING_GRADE=<wall fraction>,<growth>` explores others.
        let (frac, growth) = std::env::var("TETGEN_SIZING_GRADE")
            .ok()
            .and_then(|s| {
                let (a, b) = s.split_once(',')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .unwrap_or((WALL_FRACTION, WallGrade::DEFAULT_GROWTH));
        let grade = WallGrade {
            wall_size: frac * h,
            growth,
            interior_size: 2.0 * h,
        };
        let src = TetSource::tetgen(1.414, Some(v)).with_wall_grade(Some(grade));
        let walls = vec!["3".to_string(), "4".to_string()];
        let run = tetgen_plc(&p, &t, Some(&r), "walls", &walls, src).unwrap();
        let name = format!("{shape}_graded");
        std::fs::write(format!("{dir}/{name}.smesh"), run.plc.to_smesh().unwrap()).unwrap();
        let mut s = format!("{} 1\n", run.plc.points.len());
        for m in &run.plc.point_mtrs {
            s += &format!("{m:?}\n");
        }
        std::fs::write(format!("{dir}/{name}.mtr"), s).unwrap();
        println!(
            "{name}: {} points, {} seeds",
            run.plc.points.len(),
            run.n_seeds
        );
    }
}
