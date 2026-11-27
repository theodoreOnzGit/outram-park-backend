// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The heat-exchanger WEIGHTED zone-to-zone mapping in the cell-centred `onePhase` region, on a 3-D mesh, against the real binary
//! (gh:#1903, README round 52)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! `cases/esfr_hx`: the shipped `3D_SmallESFR` fluid mesh (23322 hexahedra) with the `2D_MSFR` `steadyStateTH` settings, the core split into two
//! halves `hxA`, `hxB` of equal volume, two mirrored exchangers (`ABHX`, `BAHX`), a nonuniform initial `T` (900 K, 800 K in `hxB`), three
//! steps, tight 1e-14 solvers. Reference `refs/esfr_hx_r52.ref.txt.gz` (real binary: every 11th cell of `U p_rgh T`, every 7th core cell of
//! `T T.HX htc`, every linear solve) and `refs/esfr_hx_r52_mapping.ref.txt.gz` (`upstream_driver/hx_mapping_dump`: ESI's own `meshToMesh`
//! built as `heatExchanger::constructSecondaryHX` builds it; every 4th primary cell, `delta` of both exchangers). Methodology, criteria and
//! predictions (committed before the first run): README "Round 52"; results: README "Round 52 results".

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::adjust_delta_t;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, OnePhaseCcCase, OnePhaseCcSolver};
use outram_foam_appbuilder_lib::io::poly_mesh::read_cell_zones;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copy_dir(&s, &d);
        } else {
            std::fs::copy(&s, &d).expect("copy");
        }
    }
}

/// Stage `cases/esfr_hx` (constant, system, 0) and the gunzipped polyMesh; returns `(case root, polyMesh dir)`.
fn stage(tag: &str) -> (PathBuf, PathBuf) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1903_r52_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("case");
    let src = vv_dir().join("cases/esfr_hx");
    for d in ["constant", "system", "0"] {
        copy_dir(&src.join(d), &root.join(d));
    }
    let poly = tmp.join("polyMesh");
    std::fs::create_dir_all(&poly).expect("mkdir");
    let pm = src.join("constant/fluidRegion/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        std::fs::write(poly.join(f), gunzip(&pm.join(format!("{f}.gz")))).expect("polyMesh");
    }
    std::fs::copy(pm.join("boundary"), poly.join("boundary")).expect("boundary");
    (root, poly)
}

/// The core cells in the order of the reference: `hxA` then `hxB`.
fn core_cells(poly: &Path) -> (Vec<usize>, usize) {
    let zones = read_cell_zones(poly).expect("zones");
    let a = zones.iter().find(|z| z.name == "hxA").expect("hxA").cells.clone();
    let b = zones.iter().find(|z| z.name == "hxB").expect("hxB").cells.clone();
    let na = a.len();
    (a.into_iter().chain(b).collect(), na)
}

type Solve = (f64, i64);

struct Reference {
    times: Vec<String>,
    global: HashMap<String, Vec<f64>>,
    zone: HashMap<String, Vec<f64>>,
    prgh: HashMap<(usize, usize, usize), Solve>,
    h: HashMap<(usize, usize), Solve>,
}

impl Reference {
    fn outer_iterations(&self, step: usize) -> usize {
        self.h.keys().filter(|(s, _)| *s == step).count()
    }
}

fn read_reference() -> Reference {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/esfr_hx_r52.ref.txt.gz"))).expect("utf8");
    let mut r = Reference { times: Vec::new(), global: HashMap::new(), zone: HashMap::new(), prgh: HashMap::new(), h: HashMap::new() };
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let p = |i: usize| -> usize { w[i].parse().expect("integer") };
        let f = |i: usize| -> f64 { w[i].parse().expect("number") };
        match w[0] {
            "TIMES" => r.times = w[1..].iter().map(|s| (*s).to_string()).collect(),
            "G" => {
                r.global.insert(format!("{} {}", w[2], w[1]), w[3..].iter().map(|x| x.parse().expect("number")).collect());
            }
            "Z" => {
                let v = if w[3] == "UNIFORM" { vec![f(4)] } else { w[3..].iter().map(|x| x.parse().expect("number")).collect() };
                r.zone.insert(format!("{} {}", w[2], w[1]), v);
            }
            "PRGH" => {
                r.prgh.insert((p(1), p(2), p(3)), (f(4), p(5) as i64));
            }
            "H" => {
                r.h.insert((p(1), p(2)), (f(3), p(4) as i64));
            }
            _ => {}
        }
    }
    r
}

/// `max |a - b| / max |b|` (absolute when `b` is identically 0).
fn max_norm(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let d = a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
    if scale == 0.0 {
        d
    } else {
        d / scale
    }
}

fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()))
}

fn sample(v: &[f64], width: usize, stride: usize) -> Vec<f64> {
    v.chunks(width).step_by(stride).flatten().copied().collect()
}

/// Committed gate (README round 52).
const GATE: f64 = 1e-9;
/// Upstream solves that start below this are at the round-off floor (round 44 part 2): their iteration counts are not gated.
const NOISE_FLOOR: f64 = 1e-10;
/// `volumetricArea` of both exchangers (`cases/esfr_hx/.../phaseProperties`); `iAHX` = 250 on every core cell (the primaries partition the core).
const IA_HX: f64 = 250.0;

struct Outcome {
    worst_field: f64,
    first_diverging_step: Option<usize>,
    outer: Vec<usize>,
    outer_ref: Vec<usize>,
    worst_iter_diff: i64,
    noise_solves: usize,
    solves: usize,
    worst_residual: f64,
    exact: (usize, usize),
    per_field: Vec<(usize, String, f64)>,
    /// `(step, T change of hxA cells in K (min over sample), T change of hxB cells (max))` of the port, for E8.
    t_move: Vec<(usize, f64, f64)>,
    zone_t_abs: Vec<(usize, f64, f64)>,
    umax: Vec<f64>,
}

impl Outcome {
    fn counts_equal(&self) -> bool {
        self.outer == self.outer_ref
    }
    fn worst_of(&self, field: &str) -> f64 {
        self.per_field.iter().filter(|x| x.1 == field).fold(0.0_f64, |m, x| m.max(x.2))
    }
}

fn run(solver: &mut OnePhaseCcSolver, reference: &Reference, cells: &[usize], n_a: usize) -> Outcome {
    let mut dt = solver.case().delta_t;
    let mut o = Outcome {
        worst_field: 0.0,
        first_diverging_step: None,
        outer: Vec::new(),
        outer_ref: Vec::new(),
        worst_iter_diff: 0,
        noise_solves: 0,
        solves: 0,
        worst_residual: 0.0,
        exact: (0, 0),
        per_field: Vec::new(),
        t_move: Vec::new(),
        zone_t_abs: Vec::new(),
        umax: Vec::new(),
    };
    let sampled: Vec<usize> = (0..cells.len()).step_by(7).collect();
    let is_a: Vec<bool> = sampled.iter().map(|&i| i < n_a).collect();
    for s in 1..=3usize {
        solver.begin_time_step(dt);
        let (_r, rep) = solver.correct_physics();
        o.outer.push(rep.outer_iterations);
        o.outer_ref.push(reference.outer_iterations(s));
        let mut check = |mine: Solve, theirs: Option<&Solve>| {
            o.solves += 1;
            match theirs {
                Some(t) => {
                    if t.0 > NOISE_FLOOR {
                        o.worst_iter_diff = o.worst_iter_diff.max((mine.1 - t.1).abs());
                    } else {
                        o.noise_solves += 1;
                    }
                    if t.0 > 1e-7 {
                        o.worst_residual = o.worst_residual.max(((mine.0 - t.0) / t.0).abs());
                    }
                }
                None => o.worst_iter_diff = i64::MAX,
            }
        };
        for ps in solver.pressure_solves() {
            check((ps.initial_residual, ps.iterations as i64), reference.prgh.get(&(s, ps.outer, ps.corrector)));
        }
        for (i, (hres, hit)) in solver.h_solves().iter().enumerate() {
            check((*hres, *hit as i64), reference.h.get(&(s, i + 1)));
        }
        let t = &reference.times[s - 1];
        let compare = |o: &mut Outcome, name: &str, mine: Vec<f64>, theirs: &[f64]| {
            let d = max_norm(&mine, theirs);
            o.worst_field = o.worst_field.max(d);
            o.per_field.push((s, name.to_string(), d));
            if d > GATE && o.first_diverging_step.is_none() {
                o.first_diverging_step = Some(s);
            }
            o.exact.0 += mine.iter().zip(theirs).filter(|(a, b)| a == b).count();
            o.exact.1 += mine.len();
        };
        let u: Vec<f64> = solver.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
        o.umax.push(u.iter().fold(0.0_f64, |m, v| m.max(v.abs())));
        let named: [(&str, Vec<f64>, usize); 3] = [("U", u, 3), ("p_rgh", solver.p_rgh().expect("p_rgh").to_vec(), 1), ("T", solver.temperature().to_vec(), 1)];
        for (name, v, w) in named {
            compare(&mut o, &format!("G {name}"), sample(&v, w, 11), &reference.global[&format!("{name} {t}")]);
        }
        let t_hx_empty = solver.heat_exchanger_surface_temperature().is_empty();
        let mine_t: Vec<f64> = sampled.iter().map(|&i| solver.temperature()[cells[i]]).collect();
        let mine_x: Vec<f64> = sampled.iter().map(|&i| if t_hx_empty { 0.0 } else { solver.heat_exchanger_surface_temperature()[cells[i]] }).collect();
        let mine_h: Vec<f64> = sampled.iter().map(|&i| solver.htc()[cells[i]]).collect();
        let (ref_t, ref_x, ref_h) = (&reference.zone[&format!("T {t}")], &reference.zone[&format!("T.HX {t}")], &reference.zone[&format!("htc {t}")]);
        compare(&mut o, "Z T", mine_t.clone(), ref_t);
        compare(&mut o, "Z T.HX", mine_x.clone(), ref_x);
        compare(&mut o, "Z htc", mine_h.clone(), ref_h);
        // exchanged heat per sampled core cell, Q = iAHX H (THX - T), and the per-zone sums of the sample
        let q = |tt: &[f64], xx: &[f64], hh: &[f64]| -> Vec<f64> { (0..tt.len()).map(|i| IA_HX * hh[i] * (xx[i] - tt[i])).collect() };
        let (q_mine, q_ref) = (q(&mine_t, &mine_x, &mine_h), q(ref_t, ref_x, ref_h));
        compare(&mut o, "Q", q_mine.clone(), &q_ref);
        for (zname, want_a) in [("QsumA", true), ("QsumB", false)] {
            let sum = |v: &[f64]| -> f64 { v.iter().zip(&is_a).filter(|(_, a)| **a == want_a).map(|(x, _)| *x).sum() };
            let (m, r) = (sum(&q_mine), sum(&q_ref));
            let d = if r == 0.0 { (m - r).abs() } else { ((m - r) / r).abs() };
            o.worst_field = o.worst_field.max(d);
            o.per_field.push((s, zname.to_string(), d));
            if d > GATE && o.first_diverging_step.is_none() {
                o.first_diverging_step = Some(s);
            }
        }
        // T in the two zones separately (absolute, K)
        let da = max_abs_diff(&mine_t.iter().zip(&is_a).filter(|(_, a)| **a).map(|(x, _)| *x).collect::<Vec<_>>(), &ref_t.iter().zip(&is_a).filter(|(_, a)| **a).map(|(x, _)| *x).collect::<Vec<_>>());
        let db = max_abs_diff(&mine_t.iter().zip(&is_a).filter(|(_, a)| !**a).map(|(x, _)| *x).collect::<Vec<_>>(), &ref_t.iter().zip(&is_a).filter(|(_, a)| !**a).map(|(x, _)| *x).collect::<Vec<_>>());
        o.zone_t_abs.push((s, da, db));
        let a_min = mine_t.iter().zip(&is_a).filter(|(_, a)| **a).map(|(x, _)| *x).fold(f64::MAX, f64::min);
        let b_max = mine_t.iter().zip(&is_a).filter(|(_, a)| !**a).map(|(x, _)| *x).fold(f64::MIN, f64::max);
        o.t_move.push((s, 900.0 - a_min, b_max - 800.0));
        dt = adjust_delta_t(true, &[solver.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    o
}

fn build(tag: &str, options: &CaseReadOptions) -> (OnePhaseCcSolver, Vec<usize>, usize) {
    let (root, poly) = stage(tag);
    let case = OnePhaseCcCase::read_with_options(&root, &poly, options).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let (cells, n_a) = core_cells(&poly);
    (OnePhaseCcSolver::new(case), cells, n_a)
}

fn print(label: &str, o: &Outcome) {
    let mut names: Vec<String> = o.per_field.iter().map(|x| x.1.clone()).collect();
    names.sort();
    names.dedup();
    let worst: Vec<String> = names.iter().map(|f| format!("{f} {:.1e}", o.worst_of(f))).collect();
    println!("    worst by field: {}", worst.join(", "));
    println!(
        "  {label}: outer port {:?} upstream {:?} | {} solves, worst iteration difference {} above the floor ({} below it), worst relative initial residual {:.1e} | worst field {:.2e}, first step above 1e-9: {:?} | bit-equal {}/{}",
        o.outer, o.outer_ref, o.solves, o.worst_iter_diff, o.noise_solves, o.worst_residual, o.worst_field, o.first_diverging_step, o.exact.0, o.exact.1
    );
    println!("    |U| max per step {:?}; T change (K) hxA fall / hxB rise per step {:?}; |dT| in zones (K, A/B) {:?}", o.umax, o.t_move, o.zone_t_abs);
}

fn control(options: &CaseReadOptions, tweak: impl Fn(OnePhaseCcSolver) -> OnePhaseCcSolver) -> Outcome {
    let reference = read_reference();
    let (solver, cells, n_a) = build("ctl", options);
    let mut solver = tweak(solver);
    run(&mut solver, &reference, &cells, n_a)
}

/// The port against the real binary on `esfr_hx`: E4-E8 (criteria and predictions: README round 52).
#[test]
fn esfr_hx_matches_the_real_binary() {
    let reference = read_reference();
    let (mut solver, cells, n_a) = build("main", &CaseReadOptions::default());
    let o = run(&mut solver, &reference, &cells, n_a);
    print("port", &o);
    assert!(o.counts_equal(), "outer iterations {:?} against {:?}", o.outer, o.outer_ref);
    assert!(o.worst_iter_diff <= 1, "worst iteration difference {}", o.worst_iter_diff);
    assert!(o.worst_field <= GATE, "field {:e}, first diverging step {:?}", o.worst_field, o.first_diverging_step);
}

/// Controls C1-C3 (README round 52): each must differ from the references.
#[test]
fn controls_differ() {
    let off = control(&CaseReadOptions::default(), |s| s.without_heat_exchangers());
    print("control C1: exchangers off", &off);
    println!("    C1: T (global) worst {:.3e}, T.HX worst {:.3e}", off.worst_of("G T"), off.worst_of("Z T.HX"));
    let near = control(&CaseReadOptions::default(), |s| s.with_nearest_cell_heat_exchanger_mapping());
    print("control C2: nearest-cell mapping", &near);
    println!("    C2: T.HX worst {:.3e}, T worst {:.3e}", near.worst_of("Z T.HX"), near.worst_of("G T"));
    let prev = control(&CaseReadOptions::default(), |s| s.with_heat_exchanger_secondary_prev_iter_temperature());
    print("control C3: secondary htc*T with T.prevIter()", &prev);
    println!("    C3: T.HX worst {:.3e}", prev.worst_of("Z T.HX"));
    assert!(off.worst_of("Z T.HX") >= 0.5);
    assert!(near.worst_of("Z T.HX") > GATE, "C2: the nearest-cell mapping must differ");
    assert!(prev.worst_of("Z T.HX") > GATE, "C3: the prevIter secondary must differ");
}

/// E2/E3: the port's `meshToMesh` mapping of both exchangers against `hxMappingDump` (ESI's own class built as
/// `heatExchanger::constructSecondaryHX` builds it): translation bit-identical, entry counts and labels identical, weights compared.
#[test]
fn mapping_equals_the_upstream_probe() {
    let (root, poly) = stage("map");
    let case = OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()).expect("read");
    let zs = &case.heat_exchangers.zones;
    assert_eq!(zs.len(), 2);
    let (core, n_a) = core_cells(&poly);
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/esfr_hx_r52_mapping.ref.txt.gz"))).expect("utf8");
    let mut deltas: HashMap<String, [f64; 3]> = HashMap::new();
    let mut rows: Vec<(String, usize, Vec<(usize, f64)>)> = Vec::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.first() {
            Some(&"D") => {
                deltas.insert(w[1].to_string(), [w[2].parse().expect("x"), w[3].parse().expect("y"), w[4].parse().expect("z")]);
            }
            Some(&"L") => {
                let n: usize = w[3].parse().expect("n");
                let ent = (0..n).map(|k| (w[4 + 2 * k].parse().expect("addr"), w[5 + 2 * k].parse().expect("wght"))).collect();
                rows.push((w[1].to_string(), w[2].parse().expect("cell"), ent));
            }
            _ => {}
        }
    }
    let name_of = |z: &outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::heat_exchanger::HeatExchangerZone| -> &'static str {
        if core[..n_a].contains(&z.primary_cells[0]) {
            "ABHX"
        } else {
            "BAHX"
        }
    };
    let (mut cmp, mut label_diff, mut count_diff, mut worst_w, mut exact_w) = (0usize, 0usize, 0usize, 0.0_f64, 0usize);
    for z in zs {
        let name = name_of(z);
        let d = deltas[name];
        println!("{name}: translation port ({:e} {:e} {:e}) upstream ({:e} {:e} {:e})", z.translation.x, z.translation.y, z.translation.z, d[0], d[1], d[2]);
        assert_eq!([z.translation.x, z.translation.y, z.translation.z], d, "{name}: delta must be bit-identical");
        let (mut none, mut one, mut two, mut more, mut sum_worst) = (0usize, 0usize, 0usize, 0usize, 0.0_f64);
        for &c in &z.primary_cells {
            let w = &z.mapping.tgt_to_src_wght[c];
            match w.len() {
                0 => none += 1,
                1 => one += 1,
                2 => two += 1,
                _ => more += 1,
            }
            if !w.is_empty() {
                sum_worst = sum_worst.max((w.iter().sum::<f64>() - 1.0).abs());
            }
        }
        let n = z.primary_cells.len();
        println!("{name}: {n} primary cells: {none} without a source entry, {one} with one entry, {two} with two, {more} with three or more ({:.1} % fractional); worst |sum - 1| {sum_worst:e}", 100.0 * (two + more) as f64 / n as f64);
        assert!(two + more >= n / 2, "{name}: E2 (at least 50 % fractional)");
        assert!(sum_worst <= 4.0 * f64::EPSILON);
    }
    for (name, cell, ent) in &rows {
        let z = zs.iter().find(|z| name_of(z) == name.as_str()).expect("exchanger");
        let (addr, wght) = (&z.mapping.tgt_to_src_addr[*cell], &z.mapping.tgt_to_src_wght[*cell]);
        cmp += 1;
        if addr.len() != ent.len() {
            count_diff += 1;
            continue;
        }
        for (k, (a, w)) in ent.iter().enumerate() {
            if addr[k] != *a {
                label_diff += 1;
            }
            let d = ((wght[k] - w) / w).abs();
            worst_w = worst_w.max(d);
            if wght[k] == *w {
                exact_w += 1;
            }
        }
    }
    println!("mapping: {cmp} sampled primary cells compared; entry-count differences {count_diff}, label differences {label_diff}; weights bit-identical {exact_w}, worst relative difference {worst_w:e}");
    assert_eq!((count_diff, label_diff), (0, 0));
    assert!(worst_w <= GATE);
}
