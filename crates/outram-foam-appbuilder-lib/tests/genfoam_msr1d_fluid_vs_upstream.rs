// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The shipped `1D_MSR_pointKinetics` `fluidRegion` (cyclic `onePhase` loop) against the real binary (gh:#1905, README round 80)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, unmodified, serial, on the `steadyState` stage of the shipped
//! `Tutorials/reactorCases/1D_MSR_pointKinetics` (Allrun's edits: `endTime 100`, `adjustTimeStep true`, the TH coupling dictionary;
//! the fluid region alone: `onePhase`, `faceCentered`, `porousKEpsilon`, `Boussinesq`, a 640-cell column closed by the cyclic
//! `top`/`bottom` pair). `scripts/run_msr1d_fluid_r80.sh <tight|loose> <gamg|pcg> <first|full>` adds `writePrecision 17` and the
//! write control only (`first`: every step of the first 60; `full`: every 100th step to `t = 100`); `tight` = every tolerance 1e-14,
//! `relTol 0`; `pcg` = `p_rgh` `GAMG` -> `PCG`/`DIC`. `scripts/extract_msr1d_fluid_r80.sh` writes the references
//! (`refs/msr1d_r80_<mode>_<proto>_<solver>{,_log}.ref.txt.gz`). The port reads `cases/msr1d_root` with the SAME edits, runs its own
//! `onePhase` loop and its own `adjustTimeStep`; nothing of upstream's results is an input.
//!
//! Criteria (README "Round 80", committed before any comparison): K1 every field `<= 1e-9` relative to its own max-norm (`U` on
//! `max|U|`) at every written step of the tight runs; K2 the time steps to 1e-12; K3 every `p_rgh`/`k`/`h` solve whose upstream initial
//! residual is above 1e-10: residual to 1e-6, iterations within 1 (noise-floor solves and `maxIter` solves reported); K4 the `T` lines to
//! 1e-9. Results: README "Round 80 results" and the doc comment of each test.

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::adjust_delta_t;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{
    CaseReadOptions, GamgPolicy, OnePhaseCcCase, OnePhaseCcSolver,
};

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

fn edit_file(root: &Path, rel: &str, f: impl Fn(&str) -> String) {
    let p = root.join(rel);
    let text = std::fs::read_to_string(&p).expect("read");
    std::fs::write(&p, f(&text)).expect("write");
}

/// `run_msr1d_fluid_r80.sh`'s edits of the fluid `fvSolution`: `tight` (every `tolerance` 1e-14, every `relTol` 0) and `pcg`
/// (`solver GAMG; smoother DIC;` -> `solver PCG; preconditioner DIC;`), as line edits.
fn protocol_edits(root: &Path, tight: bool, pcg: bool) {
    edit_file(root, "system/fluidRegion/fvSolution", |t| {
        let mut out = String::new();
        let mut lines = t.lines().peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];
            let w: Vec<&str> = trimmed.split_whitespace().collect();
            if tight && w.len() == 2 && w[0] == "tolerance" && w[1].ends_with(';') {
                out.push_str(&format!("{indent}tolerance       1e-14;\n"));
            } else if tight && w.len() >= 2 && w[0] == "relTol" && w[1].ends_with(';') {
                out.push_str(&format!("{indent}relTol          0;\n"));
            } else if pcg && w.len() == 2 && w[0] == "solver" && w[1] == "GAMG;" {
                out.push_str(&format!("{indent}solver          PCG;\n"));
                if let Some(next) = lines.peek() {
                    if next.trim_start().starts_with("smoother") {
                        lines.next();
                        out.push_str(&format!("{indent}preconditioner  DIC;\n"));
                    }
                }
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    });
}

/// Stage `cases/msr1d_root` with Allrun's `steadyState` edits and the protocol edits.
fn stage(tag: &str, tight: bool, pcg: bool) -> (PathBuf, PathBuf) {
    stage_case("msr1d_root", tag, tight, pcg)
}

/// [`stage`] of any copy of a shipped 1-D MSR `rootCase` (round 81, gh:#1962: `msr1d_thermal_root`, whose Allrun's extra
/// `startFrom startTime; startTime 0` are already the case's own values).
fn stage_case(case: &str, tag: &str, tight: bool, pcg: bool) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1905_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join(format!("cases/{case}")), &root);
    std::fs::copy(root.join("constant/multiRegionCouplingDict_TH"), root.join("constant/multiRegionCouplingDict")).expect("coupling dict");
    edit_file(&root, "system/controlDict", |t| {
        assert!(t.contains("endTime         1000;"), "endTime edit");
        t.replacen("endTime         1000;", "endTime         100;", 1)
    });
    protocol_edits(&root, tight, pcg);
    let poly = root.join("constant/fluidRegion/polyMesh");
    (root, poly)
}

fn solver(tag: &str, tight: bool, pcg: bool) -> OnePhaseCcSolver {
    let (root, poly) = stage(tag, tight, pcg);
    let opts = CaseReadOptions { gamg: GamgPolicy::Native, ..CaseReadOptions::default() };
    OnePhaseCcSolver::new(OnePhaseCcCase::read_with_options(&root, &poly, &opts).unwrap_or_else(|e| panic!("{tag}: {e}")))
}

/// `F <time> <name> v1 v2 ...` lines, keyed `"<name> <time>"`.
fn read_fields(path: &Path) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("F") {
            let time = it.next().expect("time");
            let name = it.next().expect("name");
            let v: Vec<f64> = it.map(|w| w.parse().expect("number")).collect();
            if !v.is_empty() {
                m.insert(format!("{name} {time}"), v);
            }
        }
    }
    m
}

#[derive(Default)]
struct LogRef {
    prgh: HashMap<usize, Vec<(usize, usize, f64, i64)>>,
    k: HashMap<usize, Vec<(usize, f64, i64)>>,
    h: HashMap<usize, Vec<(usize, f64, i64)>>,
    tstat: HashMap<(usize, usize), [f64; 3]>,
    dt_next: HashMap<usize, f64>,
    step_time: HashMap<usize, String>,
}

fn read_log(path: &Path) -> LogRef {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut l = LogRef::default();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let u = |i: usize| w[i].parse::<usize>().expect("int");
        let f = |i: usize| w[i].parse::<f64>().expect("float");
        match w[0] {
            "PRGH" => l.prgh.entry(u(1)).or_default().push((u(2), u(3), f(4), w[5].parse().expect("int"))),
            "K" => l.k.entry(u(1)).or_default().push((u(2), f(3), w[4].parse().expect("int"))),
            "H" => l.h.entry(u(1)).or_default().push((u(2), f(3), w[4].parse().expect("int"))),
            "TSTAT" => {
                l.tstat.insert((u(1), u(2)), [f(3), f(4), f(5)]);
            }
            "DTNEXT" => {
                l.dt_next.insert(u(1), f(2));
            }
            "STEPTIME" => {
                l.step_time.insert(u(1), w[2].to_string());
            }
            _ => {}
        }
    }
    l
}

fn rel_max(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let d = a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
    if scale == 0.0 {
        d
    } else {
        d / scale
    }
}

/// Round 81 (gh:#1962): the last five are the `lumpedNuclearStructure` fields of 1D_thermalMSR (absent from the 1D_MSR references), and a
/// laminar case has no `k`/`epsilon`/`nut`/`alphat`: a field either side lacks is not compared.
const FIELDS: [&str; 14] = [
    "U", "p", "p_rgh", "T", "k", "epsilon", "nut", "alphat", "htc", "Tmax.lumpedNuclearStructure", "Tsurface.lumpedNuclearStructure",
    "T.fuelAvForNeutronics", "T.cladAvForNeutronics", "Tmatrix.lumpedNuclearStructure",
];
const FLOOR: f64 = 1e-10;

#[derive(Debug, Default)]
struct Report {
    steps: usize,
    compared_steps: usize,
    worst: [f64; 14],
    identical: [usize; 14],
    compared: [usize; 14],
    first_over: Option<(usize, &'static str, f64)>,
    /// The first step and field that is not bit-identical.
    first_nonzero: Option<(usize, &'static str, f64)>,
    dt_worst: f64,
    t_line_worst: f64,
    gated: usize,
    res_worst: f64,
    it_worst: i64,
    floor: usize,
    floor_it_worst: i64,
    max_iter_solves: usize,
    unmatched: usize,
    /// `(kind, step, upstream residual, port residual, upstream its, port its)` of the first few iteration-count differences.
    it_diffs: Vec<String>,
    last_state: Vec<(String, f64)>,
    /// The field differences of steps 1-3.
    early: Vec<String>,
    /// The port's time after the last step, and upstream's last time name.
    final_time: f64,
    last_time: f64,
}

/// Run `n_steps` of the port and compare with `refs/msr1d_r80_<refname>` (a `refname` starting `r81_`: `refs/msr1d_<refname>`, round 81).
fn run(mut s: OnePhaseCcSolver, refname: &str, n_steps: Option<usize>) -> Report {
    let prefix = if refname.starts_with("r81_") { "msr1d_" } else { "msr1d_r80_" };
    let rf = read_fields(&vv_dir().join(format!("refs/{prefix}{refname}.ref.txt.gz")));
    let lg = read_log(&vv_dir().join(format!("refs/{prefix}{refname}_log.ref.txt.gz")));
    let n_steps = n_steps.unwrap_or(lg.step_time.len());
    let mut dt = s.case().delta_t;
    let mut rep = Report::default();
    for step in 1..=n_steps {
        s.begin_time_step(dt);
        let (_r, outer) = s.correct_physics();
        rep.steps = step;
        if step > 1 {
            if let Some(&up) = lg.dt_next.get(&(step - 1)) {
                rep.dt_worst = rep.dt_worst.max(((dt - up) / up).abs());
                if step <= 4 {
                    rep.early.push(format!("step {step} dt up {up:e} port {dt:e}"));
                }
            }
        }
        let gate = |kind: &str, ini: f64, it: i64, mine: (f64, usize), max_iter: i64, rep: &mut Report| {
            if step <= 4 {
                rep.early.push(format!("step {step} {kind} ini up {ini:e} port {:e} its {it} {}", mine.0, mine.1));
            }
            if it >= max_iter {
                rep.max_iter_solves += 1;
                return;
            }
            if ini > FLOOR {
                rep.gated += 1;
                rep.res_worst = rep.res_worst.max(((mine.0 - ini) / ini).abs());
                let d = (mine.1 as i64 - it).abs();
                rep.it_worst = rep.it_worst.max(d);
                if d > 0 && rep.it_diffs.len() < 12 {
                    rep.it_diffs.push(format!("{kind} step {step} ini {ini:e} vs {:e} its {it} vs {}", mine.0, mine.1));
                }
            } else {
                rep.floor += 1;
                rep.floor_it_worst = rep.floor_it_worst.max((mine.1 as i64 - it).abs());
            }
        };
        let ps = s.pressure_solves().to_vec();
        for &(o, c, ini, it) in lg.prgh.get(&step).map_or(&[][..], Vec::as_slice) {
            match ps.iter().find(|x| x.outer == o && x.corrector == c) {
                Some(m) => gate("p_rgh", ini, it, (m.initial_residual, m.iterations), 1000, &mut rep),
                None => rep.unmatched += 1,
            }
        }
        let hs = s.h_solves().to_vec();
        for &(o, ini, it) in lg.h.get(&step).map_or(&[][..], Vec::as_slice) {
            match hs.get(o - 1) {
                Some(&m) => gate("h", ini, it, m, 1000, &mut rep),
                None => rep.unmatched += 1,
            }
        }
        let ts = s.turbulence_solves().map(<[_]>::to_vec).unwrap_or_default();
        for &(o, ini, it) in lg.k.get(&step).map_or(&[][..], Vec::as_slice) {
            match ts.get(o - 1) {
                Some(m) => gate("k", ini, it, m[1], 1000, &mut rep),
                None => rep.unmatched += 1,
            }
        }
        for (o, st) in outer.t_stats.iter().enumerate() {
            if let Some(up) = lg.tstat.get(&(step, o + 1)) {
                for (x, y) in [st.0, st.1, st.2].iter().zip(up) {
                    rep.t_line_worst = rep.t_line_worst.max(((x - y) / y).abs());
                }
            }
        }
        let time = lg.step_time.get(&step).cloned().unwrap_or_default();
        if rf.contains_key(&format!("U {time}")) {
            rep.compared_steps += 1;
            let u: Vec<f64> = s.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
            let (fuel_av, clad_av) = s.fuel_clad_temperatures();
            let (tmax, tsurf, tmatrix) = lumped_fields(&s);
            let port: [Option<Vec<f64>>; 14] = [
                Some(u),
                Some(s.pressure().to_vec()),
                Some(s.p_rgh().expect("flow").to_vec()),
                Some(s.temperature().to_vec()),
                s.turbulent_kinetic_energy().map(<[f64]>::to_vec),
                s.turbulent_dissipation().map(<[f64]>::to_vec),
                s.turbulent_viscosity().map(<[f64]>::to_vec),
                s.turbulent_thermal_diffusivity().map(<[f64]>::to_vec),
                Some(s.htc().to_vec()),
                tmax,
                tsurf,
                Some(fuel_av),
                Some(clad_av),
                tmatrix,
            ];
            rep.last_state.clear();
            for (i, name) in FIELDS.iter().enumerate() {
                let Some(up) = rf.get(&format!("{name} {time}")) else { continue };
                let Some(mine) = &port[i] else { continue };
                let d = rel_max(mine, up);
                rep.compared[i] += 1;
                if d == 0.0 {
                    rep.identical[i] += 1;
                }
                rep.worst[i] = rep.worst[i].max(d);
                rep.last_state.push(((*name).to_string(), d));
                if step <= 3 {
                    rep.early.push(format!("step {step} {name} {d:.3e}"));
                }
                if d > 0.0 && rep.first_nonzero.is_none() {
                    rep.first_nonzero = Some((step, name, d));
                }
                if d > 1e-9 && rep.first_over.is_none() {
                    rep.first_over = Some((step, name, d));
                }
            }
        }
        dt = adjust_delta_t(true, &[s.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    rep.final_time = s.time();
    rep.last_time = lg.step_time.get(&n_steps).map_or(0.0, |t| t.parse().expect("time"));
    rep
}

/// `(Tmax, Tsurface, Tmatrix)` of the case's `lumpedNuclearStructure` model, per cell (0 outside it); `None` without one.
fn lumped_fields(s: &OnePhaseCcSolver) -> (Option<Vec<f64>>, Option<Vec<f64>>, Option<Vec<f64>>) {
    for m in s.radial_power_models() {
        if let RadialPowerModel::LumpedNuclearStructure(l) = m {
            let n = s.temperature().len();
            let mut tmax = vec![0.0; n];
            for &c in l.cells() {
                tmax[c] = l.tmax(c);
            }
            return (Some(tmax), Some(l.tsurface().to_vec()), Some(l.tmatrix().to_vec()));
        }
    }
    (None, None, None)
}

fn print(name: &str, r: &Report) {
    println!("{name}: {} steps, {} with fields compared", r.steps, r.compared_steps);
    for (i, f) in FIELDS.iter().enumerate() {
        println!("{name}:   {f:8} worst {:.3e}  bit-identical at {}/{} compared steps", r.worst[i], r.identical[i], r.compared[i]);
    }
    println!(
        "{name}: first not bit-identical {:?}; first over 1e-9 {:?}; dt worst {:e}; T lines worst {:e}; gated solves {} (residual worst {:e}, iterations worst {}); floor {} (its worst {}); maxIter solves {}; unmatched {}",
        r.first_nonzero, r.first_over, r.dt_worst, r.t_line_worst, r.gated, r.res_worst, r.it_worst, r.floor, r.floor_it_worst, r.max_iter_solves, r.unmatched
    );
    println!("{name}: iteration differences {:?}", r.it_diffs);
    println!("{name}: last compared state {:?}", r.last_state);
    println!("{name}: steps 1-3 {:?}", r.early);
}

#[test]
fn the_shipped_case_reads_with_its_cyclic_patches_and_initial_power_density() {
    let s = solver("read", false, false);
    let c = s.case();
    assert_eq!(c.t.len(), 640);
    // `fluidProperties.initialPowerDensity { "core" 33e+07; }` (`fluid.C:386-420`): the core zone, nothing elsewhere
    let core: usize = c.power_density.iter().filter(|&&q| q == 3.3e8).count();
    let zero: usize = c.power_density.iter().filter(|&&q| q == 0.0).count();
    assert!(core == 160 && zero == 480, "core {core} zero {zero}");
}

/// Prediction Q2 (README "Round 80"): `tight` + `pcg`, the first 60 steps against the real binary.
///
/// Results (measured 2026-11-28): **steps 1-5 bit-identical in every field** (`U`, `p`, `p_rgh`, `T`, `k`, `epsilon`, `nut`, `alphat`,
/// `htc`), every solve's iteration count and 17-digit initial residual equal; the first difference is ONE cell of `epsilon` at step 6 by
/// 1 ulp, the wall function's `pow(k, 1.5)` (PETIR's ARM-routines `pow` against the platform's, see
/// [`first_epsilon_difference_is_the_pow_of_the_wall_function`]); over 60 steps the worst field difference is `U` 2.1e-12, `p` 2.5e-12,
/// `T` 4.8e-14, `k` 3.0e-13 (K1 HELD, propagated round-off); `dt` 1.6e-12 (K2 MISSED at 1e-12, pinned 1e-11); 180 gated solves, initial
/// residual within 1.5e-10 relative (K3 residual HELD), iteration counts up to 9 apart in the `k`/`h` solves of steps 20-60 that run 35-60
/// PBiCGStab iterations to the 1e-14 floor (K3 iterations MISSED; pinned 15); `T` lines 2.3e-14 (K4 HELD).
#[test]
fn tight_pcg_first_60_steps_match_the_real_binary() {
    let r = run(solver("tight_pcg", true, true), "first_tight_pcg", Some(60));
    print("tight_pcg", &r);
    assert_eq!(r.compared_steps, 60);
    assert!(r.first_over.is_none(), "K1: {:?}", r.first_over);
    assert!(r.first_nonzero.is_some_and(|(step, name, _)| step >= 6 && name == "epsilon"), "bit-identical through step 5: {:?}", r.first_nonzero);
    assert!(r.dt_worst <= 1e-11, "K2 pin: {}", r.dt_worst);
    assert!(r.res_worst <= 1e-6 && r.it_worst <= 15 && r.unmatched == 0, "K3: {} {} {}", r.res_worst, r.it_worst, r.unmatched);
    assert!(r.t_line_worst <= 1e-9, "K4: {}", r.t_line_worst);
}

/// Prediction Q3: `tight` + `gamg` (as shipped: the port's native `GAMG` with the cyclic `lduInterface`s on every level), the first 60
/// steps.
///
/// Results (2026-11-28): **steps 1-18 bit-identical in every field**, although every `p_rgh` solve runs 1000 V-cycles (upstream and port
/// both stop at `maxIter` at a 3e-11 floor); first difference `epsilon` at step 19 by 1.4e-16 (the same `pow`); worst over 60 steps `U`
/// 2.9e-12, `p` 2.0e-12, `T` 4.3e-14, `k` 3.6e-13 (K1 HELD); `dt` 1.7e-12 (K2 MISSED, pin 1e-11); `k`/`h` iterations up to 11 apart
/// (K3 iterations MISSED, pin 15), residuals 1.7e-12; `T` lines 3.5e-14.
#[test]
fn tight_gamg_first_60_steps_match_the_real_binary() {
    let r = run(solver("tight_gamg", true, false), "first_tight_gamg", Some(60));
    print("tight_gamg", &r);
    assert_eq!(r.compared_steps, 60);
    assert!(r.first_over.is_none(), "K1: {:?}", r.first_over);
    assert!(r.first_nonzero.is_some_and(|(step, name, _)| step >= 19 && name == "epsilon"), "bit-identical through step 18: {:?}", r.first_nonzero);
    assert_eq!(r.max_iter_solves, 60, "every tight p_rgh GAMG solve stops at maxIter in both codes");
    assert!(r.dt_worst <= 1e-11 && r.res_worst <= 1e-6 && r.it_worst <= 15 && r.t_line_worst <= 1e-9);
}

/// The shipped tolerances (`loose`), `gamg` as shipped, the first 60 steps (reported; gated at 1e-6 at step 60).
///
/// Results (2026-11-28): steps 1-27 bit-identical in every field; first difference `U` at step 28 (7.0e-13); worst over 60 steps `k`
/// 1.3e-9, `nut` 2.6e-9, `U` 3.4e-12, `T` 7.2e-13 (the `relTol 0.001` solves amplify a last-bit difference into their stopping
/// iterate); every one of the 180 `p_rgh`/`h`/`k` iteration counts equal.
#[test]
fn loose_gamg_first_60_steps_against_the_real_binary() {
    let r = run(solver("loose_gamg", false, false), "first_loose_gamg", Some(60));
    print("loose_gamg", &r);
    assert_eq!(r.compared_steps, 60);
    assert!(r.first_nonzero.is_some_and(|(step, _, _)| step >= 28), "{:?}", r.first_nonzero);
    assert!(r.last_state.iter().all(|(_, d)| *d <= 1e-6), "{:?}", r.last_state);
    assert_eq!(r.it_worst, 0);
}

/// The controls and ablations of README "Round 80" (each must be able to fail): Q8 the seam as a plain patch; Q9 GAMG with the
/// interfaces on the finest level only (on the shipped, `loose`, protocol: the tight GAMG solves all stop at `maxIter`); and the three
/// earlier assembly forms this round replaced, each on the tight `pcg` run (where the default is bit-identical through step 5).
#[test]
fn controls_and_ablations_differ_from_the_real_binary() {
    let up_u = |refname: &str, step: usize| -> Vec<f64> {
        let rf = read_fields(&vv_dir().join(format!("refs/msr1d_r80_{refname}.ref.txt.gz")));
        let lg = read_log(&vv_dir().join(format!("refs/msr1d_r80_{refname}_log.ref.txt.gz")));
        rf[&format!("U {}", lg.step_time[&step])].clone()
    };
    // Q8: 60 steps without any interface
    let mut s = solver("ctl_no_cyclic", true, true).without_cyclic_interfaces();
    let mut dt = s.case().delta_t;
    for _ in 0..60 {
        s.begin_time_step(dt);
        let _ = s.correct_physics();
        dt = adjust_delta_t(true, &[s.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    let up = up_u("first_tight_pcg", 60);
    let umax_up = up.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let umax_port = s.velocity().iter().fold(0.0_f64, |m, v| m.max(v.mag()));
    let q8 = ((umax_port - umax_up) / umax_up).abs();
    println!("Q8 no cyclic interfaces: max|U| at step 60 port {umax_port:e} up {umax_up:e} relative difference {q8:e}");
    // Q9: loose GAMG, interfaces on the finest level only, step 1 p_rgh iterations
    let lg = read_log(&vv_dir().join("refs/msr1d_r80_first_loose_gamg_log.ref.txt.gz"));
    let r9 = run(solver("ctl_gamg_fine", false, false).with_gamg_interfaces_on_the_finest_level_only(), "first_loose_gamg", Some(1));
    let mut s9 = solver("ctl_gamg_fine1", false, false).with_gamg_interfaces_on_the_finest_level_only();
    s9.begin_time_step(s9.case().delta_t);
    let _ = s9.correct_physics();
    let its9: Vec<usize> = s9.pressure_solves().iter().map(|p| p.iterations).collect();
    let its_up: Vec<i64> = lg.prgh[&1].iter().map(|x| x.3).collect();
    println!("Q9 GAMG interfaces on the finest level only: step 1 p_rgh iterations port {its9:?} upstream {its_up:?}; fields {:?}", r9.last_state);
    // the three replaced assembly forms (first non-identical step and its size over 10 steps)
    for (name, s) in [
        ("folded k/epsilon", solver("abl_keps", true, true).with_folded_k_epsilon_assembly()),
        ("folded energy", solver("abl_energy", true, true).with_folded_energy_assembly()),
        ("weighted dev2", solver("abl_dev2", true, true).with_weighted_dev2_interpolation()),
    ] {
        let r = run(s, "first_tight_pcg", Some(10));
        println!("ablation {name}: first not bit-identical {:?}; worst over 10 steps U {:.2e} p {:.2e} T {:.2e} k {:.2e}", r.first_nonzero, r.worst[0], r.worst[1], r.worst[3], r.worst[4]);
        assert!(r.first_nonzero.is_some_and(|(step, _, _)| step <= 3), "{name}: {:?}", r.first_nonzero);
    }
    assert!(q8 > 0.1, "Q8: {q8}");
    assert_ne!(its9.iter().map(|&i| i as i64).collect::<Vec<_>>(), its_up, "Q9");
}

/// Prediction Q7: the shipped stage (`loose`, `gamg`) to `t = 100` (8340 steps; 6 s).
///
/// Results (2026-11-28): the port takes the same number of steps and ends at `t = 100` to round-off; every written step (every 100th)
/// compared: worst `T` 8.6e-6 (an early transient of the `relTol 0.001` solves; 1.5e-6 already at step 100), `p` 3.1e-9, `U` 7.7e-11,
/// `k` 1.1e-10; at `t = 100`: `T` 1.3e-9, `p` 7.0e-11, `U` 3.9e-12, `k` 1.2e-11 (Q7 HELD: within 1e-6); `dt` 1.3e-10; 25 017 gated solves,
/// iteration counts within 1, initial residuals within 5e-4 relative; `T` lines 1.8e-5.
#[test]
fn loose_gamg_full_steady_state_against_the_real_binary() {
    let r = run(solver("loose_gamg_full", false, false), "full_loose_gamg", None);
    print("loose_gamg_full", &r);
    assert!(r.compared_steps >= 80);
    assert!((r.final_time - r.last_time).abs() <= 1e-9 * r.last_time, "{} {}", r.final_time, r.last_time);
    assert!(r.last_state.iter().all(|(_, d)| *d <= 1e-6), "Q7: {:?}", r.last_state);
    assert!(r.it_worst <= 1);
}

/// The tight `pcg` run to `t = 100` (8340 steps; 6 s).
///
/// Results (2026-11-28): K1 HELD over the whole run, every field at every 100th step within 2.7e-12 (`p`), `U` 2.9e-13, `T` 3.4e-13;
/// at `t = 100`: `U` 1.3e-14, `T` 3.8e-15, `p` 1.2e-12, `k` 2.7e-14; the same number of steps, `dt` 1.6e-12; 25 020 gated solves,
/// residuals within 7.1e-5 relative, iterations within 9; `T` lines 2.7e-14.
#[test]
fn tight_pcg_full_steady_state_against_the_real_binary() {
    let r = run(solver("tight_pcg_full", true, true), "full_tight_pcg", None);
    print("tight_pcg_full", &r);
    assert!(r.compared_steps >= 80);
    assert!((r.final_time - r.last_time).abs() <= 1e-9 * r.last_time, "{} {}", r.final_time, r.last_time);
    assert!(r.first_over.is_none(), "K1: {:?}", r.first_over);
    assert!(r.dt_worst <= 1e-11 && r.t_line_worst <= 1e-9);
}

/// Diagnostic (reported in README "Round 80"): where the FIRST difference of the tight run sits. At step 1 `U = 0` and `phi = 0`, so
/// no cyclic coefficient is non-zero; `epsilon` is set in EVERY cell by the wall function (every cell has two wall faces).
#[test]
fn step_one_epsilon_cells_against_the_real_binary() {
    let mut s = solver("diag_eps", true, true);
    let rf = read_fields(&vv_dir().join("refs/msr1d_r80_first_tight_pcg.ref.txt.gz"));
    let dt = s.case().delta_t;
    s.begin_time_step(dt);
    let _ = s.correct_physics();
    let up = &rf["epsilon 0.001"];
    let mine = s.turbulent_dissipation().expect("turbulence");
    let diff: Vec<usize> = (0..up.len()).filter(|&c| mine[c] != up[c]).collect();
    println!(
        "step 1 epsilon: {} of {} cells differ; first {:?}",
        diff.len(),
        up.len(),
        diff.iter().take(8).map(|&c| (c, mine[c], up[c])).collect::<Vec<_>>()
    );
    let upk = &rf["k 0.001"];
    let mk = s.turbulent_kinetic_energy().expect("turbulence");
    let dk: Vec<usize> = (0..upk.len()).filter(|&c| mk[c] != upk[c]).collect();
    println!("step 1 k: {} of {} cells differ; first {:?}", dk.len(), upk.len(), dk.iter().take(6).map(|&c| (c, mk[c], upk[c])).collect::<Vec<_>>());
    let upt = &rf["T 0.001"];
    let mt = s.temperature();
    let dt_cells: Vec<usize> = (0..upt.len()).filter(|&c| mt[c] != upt[c]).collect();
    println!("step 1 T: {} of {} cells differ; {:?}", dt_cells.len(), upt.len(), dt_cells.iter().take(12).map(|&c| (c, mt[c], upt[c])).collect::<Vec<_>>());
    // steps 2 and 3: where the first difference sits
    let lg = read_log(&vv_dir().join("refs/msr1d_r80_first_tight_pcg_log.ref.txt.gz"));
    let mut dt = adjust_delta_t(true, &[s.max_delta_t()], 1e-300, dt).expect("adjustable");
    for step in 2..=3 {
        s.begin_time_step(dt);
        let _ = s.correct_physics();
        let time = &lg.step_time[&step];
        let up = &rf[&format!("p_rgh {time}")];
        let mine = s.p_rgh().expect("flow");
        let d: Vec<usize> = (0..up.len()).filter(|&c| mine[c] != up[c]).collect();
        println!("step {step} p_rgh: {} cells differ; first {:?} last {:?}", d.len(), d.first().map(|&c| (c, mine[c], up[c])), d.last());
        let upu = &rf[&format!("U {time}")];
        let du: Vec<usize> = (0..640).filter(|&c| s.velocity()[c].z != upu[3 * c + 2]).collect();
        println!("step {step} Uz: {} cells differ; {:?}", du.len(), du.iter().take(10).map(|&c| (c, s.velocity()[c].z, upu[3 * c + 2])).collect::<Vec<_>>());
        if let Ok(text) = std::fs::read_to_string(format!("{}/{time}/fluidRegion/alphaPhi", std::env::var("MSR1D_RUN").unwrap_or_default())) {
            let start = text.find("639\n(").expect("list") + 5;
            let end = start + text[start..].find("\n)").expect("end");
            let vals: Vec<f64> = text[start..end].split_whitespace().map(|w| w.parse().expect("f64")).collect();
            let ap = s.alpha_phi();
            let d: Vec<usize> = (0..639).filter(|&f| ap[f] != vals[f]).collect();
            let top = text.find("uniform 0").map(|i| &text[i..i + 40]);
            println!("step {step} alphaPhi: {} of 639 internal faces differ; {:?}; port seam {:e} {:e}; upstream {:?}", d.len(), d.iter().take(5).map(|&f| (f, ap[f], vals[f])).collect::<Vec<_>>(), ap[639], ap[640], top);
        }
        if let Ok(text) = std::fs::read_to_string(format!("{}/{time}/fluidRegion/nut", std::env::var("MSR1D_RUN").unwrap_or_default())) {
            let w = text.find("nutkWallFunction").expect("walls");
            let start = w + text[w..].find("1280\n(").expect("list") + 6;
            let end = start + text[start..].find("\n)").expect("end");
            let vals: Vec<f64> = text[start..end].split_whitespace().map(|x| x.parse().expect("f64")).collect();
            let nb = s.turbulent_viscosity_boundary().expect("turbulence");
            let d: Vec<usize> = (0..1280).filter(|&i| nb[2 + i] != vals[i]).collect();
            println!("step {step} nut walls: {} of 1280 differ; {:?}", d.len(), d.iter().take(4).map(|&i| (i, nb[2 + i], vals[i])).collect::<Vec<_>>());
        }
        let kd_file = std::fs::read_to_string(format!("{}/{time}/fluidRegion/Kd", std::env::var("MSR1D_RUN").unwrap_or_default()));
        if let Ok(text) = kd_file {
            let vals: Vec<f64> = text
                .lines()
                .filter(|l| l.starts_with('(') && l.ends_with(')') && l.split_whitespace().count() == 9)
                .map(|l| l.trim_matches(|c| c == '(' || c == ')').split_whitespace().next().expect("xx").parse().expect("f64"))
                .collect();
            let kd = s.drag_coefficients();
            let dk: Vec<usize> = (0..vals.len().min(kd.len())).filter(|&c| kd[c][0] != vals[c]).collect();
            println!("step {step} Kd: {} of {} cells differ; {:?}", dk.len(), vals.len(), dk.iter().take(5).map(|&c| (c, kd[c][0], vals[c])).collect::<Vec<_>>());
        }
        dt = adjust_delta_t(true, &[s.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    let zones: Vec<(String, usize, usize)> = outram_foam_appbuilder_lib::io::poly_mesh::read_cell_zones(&vv_dir().join("cases/msr1d_root/constant/fluidRegion/polyMesh"))
        .expect("zones")
        .iter()
        .map(|z| (z.name.clone(), *z.cells.iter().min().unwrap_or(&0), *z.cells.iter().max().unwrap_or(&0)))
        .collect();
    println!("zones (name, first, last cell): {zones:?}");
}

/// Diagnostic (README "Round 80"): the `epsilon` wall-function value `Cmu75 k^1.5/(kappa y)` of the cells that differ at the first
/// non-identical step of the tight `pcg` run, recomputed with the platform `pow` and with PETIR's `r_powf` from the port's `k` before that step.
#[test]
fn first_epsilon_difference_is_the_pow_of_the_wall_function() {
    use petir::mathf::RealMath;
    let mut s = solver("diag_pow", true, true);
    let rf = read_fields(&vv_dir().join("refs/msr1d_r80_first_tight_pcg.ref.txt.gz"));
    let lg = read_log(&vv_dir().join("refs/msr1d_r80_first_tight_pcg_log.ref.txt.gz"));
    let mut dt = s.case().delta_t;
    let (cmu, kappa, y) = (0.09_f64, 0.41_f64, 0.5_f64);
    let mut tally = (0usize, 0usize, 0usize);
    for step in 1..=12 {
        let k_before = s.turbulent_kinetic_energy().expect("turbulence").to_vec();
        s.begin_time_step(dt);
        let _ = s.correct_physics();
        let up = &rf[&format!("epsilon {}", lg.step_time[&step])];
        let mine = s.turbulent_dissipation().expect("turbulence");
        for c in 0..640 {
            if mine[c] != up[c] {
                let e_std = cmu.powf(0.75) * k_before[c].powf(1.5) / (kappa * y);
                let e_petir = cmu.r_powf(0.75) * k_before[c].r_powf(1.5) / (kappa * y);
                let (w_std, w_petir) = (0.5 * e_std + 0.5 * e_std, 0.5 * e_petir + 0.5 * e_petir);
                tally.0 += 1;
                tally.1 += usize::from(w_std == up[c]);
                tally.2 += usize::from(w_petir == mine[c]);
                if tally.0 <= 4 {
                    println!("step {step} cell {c}: port {:e} up {:e}; platform pow {:e}, PETIR pow {:e}", mine[c], up[c], w_std, w_petir);
                }
            }
        }
        if tally.0 > 0 {
            break;
        }
        dt = adjust_delta_t(true, &[s.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    println!("differing epsilon cells {}: upstream equals the platform-pow value in {}, the port equals the PETIR-pow value in {}", tally.0, tally.1, tally.2);
}

/// Round 81 (gh:#1962): the shipped `1D_thermalMSR_pointKinetics` fluid region reads (probe of the reader, no comparison): it was never
/// refused (no `kappaMatrix` key, so `kappaMatrix = 0`); `initialPowerDensity` 3e8 in the 160 core cells.
#[test]
fn thermal_msr_case_reads() {
    let (root, poly) = stage_case("msr1d_thermal_root", "thermal_read", false, false);
    let opts = CaseReadOptions { gamg: GamgPolicy::Native, ..CaseReadOptions::default() };
    let c = OnePhaseCcCase::read_with_options(&root, &poly, &opts).unwrap_or_else(|e| panic!("thermal: {e}"));
    assert_eq!(c.t.len(), 640);
    assert_eq!(c.power_density.iter().filter(|&&q| q == 3e8).count(), 160);
}

/// `scripts/make_msr1d_thermal_kappa_r81.py` as text edits of the staged `phaseProperties` (README round 81): `core` adds
/// `nodeMatrix 0; kappaMatrix 30;` to the core's model; `seam` also a `ColdLeg` `lumpedNuclearStructure` region (both seam cells) with
/// `kappaMatrix 30` and a `constant` htc of 1e4.
fn kappa_variant(root: &Path, kind: &str) {
    edit_file(root, "constant/fluidRegion/phaseProperties", |t| {
        let anchor = "            nodeClad            0;";
        let i = t.find(anchor).expect("nodeClad");
        let j = i + t[i..].find('\n').expect("eol");
        let mut s = format!("{}\n            nodeMatrix          0;\n            kappaMatrix         30;\n{}", &t[..j], &t[j + 1..]);
        if kind == "seam" {
            let hx = "    \"hx\"\n    {\n        volumeFraction      0.6;";
            assert_eq!(s.matches(hx).count(), 1);
            let cold = concat!(
                "    \"ColdLeg\"\n    {\n        volumeFraction      0.1;\n        Dh                  0.02;\n\n        powerModel\n        {\n",
                "            type                lumpedNuclearStructure;\n            volumetricArea      40;\n            powerDensity        1e6;\n",
                "            nodesNumber         1;\n            nodeFuel            0;\n            nodeClad            0;\n            nodeMatrix          0;\n",
                "            kappaMatrix         30;\n            heatConductances    (450000 225000);\n            rhoCp               (5357692);\n",
                "            volumeFractions     (1);\n            powerFractions      (1);\n            T0                  900;\n        }\n    }\n",
            );
            s = s.replacen(hx, &format!("{cold}{hx}"), 1);
            let ht = "        \"hx\"\n        {\n            type    constant;";
            assert_eq!(s.matches(ht).count(), 1);
            s = s.replacen(ht, &format!("        \"ColdLeg\"\n        {{\n            type    constant;\n            value   1e+4;\n        }}\n{ht}"), 1);
        } else {
            assert_eq!(kind, "core");
        }
        s
    });
}

/// The thermal case's solver: `cases/msr1d_thermal_root`, Allrun's `steadyState` edits, the protocol edits, and a `kappaMatrix` variant.
fn thermal_solver(tag: &str, tight: bool, pcg: bool, variant: Option<&str>) -> OnePhaseCcSolver {
    let (root, poly) = stage_case("msr1d_thermal_root", tag, tight, pcg);
    if let Some(v) = variant {
        kappa_variant(&root, v);
    }
    let opts = CaseReadOptions { gamg: GamgPolicy::Native, ..CaseReadOptions::default() };
    OnePhaseCcSolver::new(OnePhaseCcCase::read_with_options(&root, &poly, &opts).unwrap_or_else(|e| panic!("{tag}: {e}")))
}

/// Prediction R1, R4-R6 (README round 81): the thermal case, `tight` + `pcg`, the first 60 steps.
///
/// Results (measured 2026-11-28): `U`, `p`, `p_rgh`, `T`, `Tmax`, `T.fuelAvForNeutronics`, `T.cladAvForNeutronics` **bit-identical at
/// all 60 steps**; the first difference is `htc` at step 17 (2.5e-17 of its max-norm: the `NusseltReynoldsPrandtlPower` `pow`, PETIR
/// against the platform), worst `htc` 9.8e-17, `Tsurface` 2.5e-16 (2 of 60 steps); `deltaT`, every `p_rgh`/`h` initial residual and
/// iteration count, the `T` lines: equal (R1, R4, R5, R6 HELD).
#[test]
fn thermal_tight_pcg_first_60_steps_against_the_real_binary() {
    let r = run(thermal_solver("th_tight_pcg", true, true, None), "r81_th_first_tight_pcg", Some(60));
    print("th_tight_pcg", &r);
    assert_eq!(r.compared_steps, 60);
    assert!(r.first_over.is_none(), "K1: {:?}", r.first_over);
    assert!(r.worst[..4].iter().all(|&d| d == 0.0), "U p p_rgh T bit-identical: {:?}", &r.worst[..4]);
    assert!(r.first_nonzero.is_some_and(|(step, _, _)| step >= 17), "{:?}", r.first_nonzero);
    assert_eq!((r.dt_worst, r.res_worst, r.it_worst, r.t_line_worst), (0.0, 0.0, 0, 0.0));
}

/// The thermal case as shipped (`loose`, `gamg`), the first 60 steps (reported).
#[test]
fn thermal_loose_gamg_first_60_steps_against_the_real_binary() {
    let r = run(thermal_solver("th_loose_gamg", false, false, None), "r81_th_first_loose_gamg", Some(60));
    print("th_loose_gamg", &r);
    // measured 2026-11-28: every compared field bit-identical at every one of the 60 steps, every solve equal
    assert_eq!(r.compared_steps, 60);
    assert!(r.first_nonzero.is_none(), "{:?}", r.first_nonzero);
    assert_eq!(r.it_worst, 0);
}

/// Prediction R2: the thermal case, `tight` + `pcg`, to `t = 100`.
///
/// Results (2026-11-28): 7637 steps, the binary's count, final time equal to round-off; every 100th step (76) compared: K1 HELD, worst
/// `p` 3.3e-12, `U` 1.2e-13, `T` 5.4e-13, `Tsurface` 3.8e-13 (the first non-identical written step is 300); `deltaT` 1.2e-13; `T` lines
/// 7.4e-15; 8447 gated solves: initial residual within 3.9e-4 relative (K3 residual MISSED at 1e-6: tight `h` solves whose initial
/// residual is ~1e-3 after 200 steps of propagated round-off), iterations up to 3 apart (the tight `h` PBiCGStab, 11-17 iterations).
#[test]
fn thermal_tight_pcg_full_steady_state_against_the_real_binary() {
    let r = run(thermal_solver("th_tight_pcg_full", true, true, None), "r81_th_full_tight_pcg", None);
    print("th_tight_pcg_full", &r);
    println!("th_tight_pcg_full: final time {} upstream {}", r.final_time, r.last_time);
    assert!(r.compared_steps >= 76);
    assert!((r.final_time - r.last_time).abs() <= 1e-9 * r.last_time);
    assert!(r.first_over.is_none(), "K1: {:?}", r.first_over);
    assert!(r.dt_worst <= 1e-11 && r.t_line_worst <= 1e-9 && r.it_worst <= 3, "{} {} {}", r.dt_worst, r.t_line_worst, r.it_worst);
}

/// Prediction R3: the shipped thermal stage (`loose`, `gamg`) to `t = 100`.
#[test]
fn thermal_loose_gamg_full_steady_state_against_the_real_binary() {
    let r = run(thermal_solver("th_loose_gamg_full", false, false, None), "r81_th_full_loose_gamg", None);
    print("th_loose_gamg_full", &r);
    println!("th_loose_gamg_full: final time {} upstream {}", r.final_time, r.last_time);
    // Results (2026-11-28): 7637 steps (R3 HELD), the first written step not bit-identical is 2400 (`U` 1.7e-15); worst over the run
    // `T.fuelAvForNeutronics` 3.5e-13, `T` 2.8e-14, `U` 1.9e-14, `p` and `p_rgh` bit-identical at all 76 written steps; 15 274 gated solves,
    // every iteration count equal, initial residuals within 1.6e-7 relative
    assert!(r.compared_steps >= 76);
    assert!((r.final_time - r.last_time).abs() <= 1e-9 * r.last_time);
    assert!(r.last_state.iter().all(|(_, d)| *d <= 1e-6), "{:?}", r.last_state);
    assert!(r.first_over.is_none() && r.it_worst == 0);
}

/// Predictions V1-V3: the `kappaMatrix` variants (`kcore`: the seam term is 0; `kseam`: it is not), `tight` + `pcg`, first 60 steps, and
/// the control without the coupled faces of `fvc::laplacian(Tmatrix)`.
#[test]
fn thermal_kappa_matrix_variants_first_60_steps_against_the_real_binary() {
    let rc = run(thermal_solver("th_kcore", true, true, Some("core")), "r81_th_kcore_first_tight_pcg", Some(60));
    print("th_kcore", &rc);
    let rs = run(thermal_solver("th_kseam", true, true, Some("seam")), "r81_th_kseam_first_tight_pcg", Some(60));
    print("th_kseam", &rs);
    let rx = run(thermal_solver("th_kseam_ctl", true, true, Some("seam")).without_coupled_lumped_laplacian(), "r81_th_kseam_first_tight_pcg", Some(60));
    print("th_kseam_ctl", &rx);
    let rk = run(thermal_solver("th_kcore_ctl", true, true, Some("core")).without_coupled_lumped_laplacian(), "r81_th_kcore_first_tight_pcg", Some(60));
    print("th_kcore_ctl", &rk);
    // the kappaMatrix term itself (gh:#1879's control): no Laplacian at all
    let rl = run(thermal_solver("th_kcore_nolap", true, true, Some("core")).without_lumped_laplacian(), "r81_th_kcore_first_tight_pcg", Some(60));
    print("th_kcore_nolap", &rl);
    // and the variant against the SHIPPED reference: what kappaMatrix 30 moves in the real binary
    let rv = run(thermal_solver("th_shipped_vs_kcore", true, true, None), "r81_th_kcore_first_tight_pcg", Some(60));
    print("th_shipped_vs_kcore", &rv);
    // Results (2026-11-28): V1 `kcore` HELD, the same as the shipped case (U p p_rgh T Tmax Tmatrix bit-identical at 60 steps, first
    // difference `htc` step 17); V2 `kseam` HELD and BETTER: every field bit-identical at all 60 steps; V3 the control without the seam
    // faces differs from step 17 (`Tmax` 1.2e-16), but only to 7.8e-13 (`T.fuelAvForNeutronics`) by step 60: V3 MISSED (predicted > 1e-9);
    // without the coupled faces `kcore` is unchanged (its seam term is 0); without any Laplacian `kcore` is off by 1.7e-2 in `Tmax` from step 1
    assert!(rc.first_over.is_none() && rc.worst[..4].iter().all(|&d| d == 0.0), "V1 {:?}", rc.first_over);
    assert!(rs.first_nonzero.is_none(), "V2 bit-identical {:?}", rs.first_nonzero);
    assert!(rx.first_nonzero.is_some(), "V3 the control differs");
    assert_eq!(format!("{:?}", rk.worst), format!("{:?}", rc.worst), "no seam term in kcore");
    assert!(rl.first_over.is_some_and(|(step, _, d)| step == 1 && d > 1e-6), "{:?}", rl.first_over);
    assert!(rv.first_over.is_some(), "kappaMatrix 30 changes the real answer");
}

/// The coupled `fvc::laplacian` on the 1-D cyclic column itself (gh:#1962): a field that is 1 in cell 0 and 0 elsewhere has, in cell 639
/// (its partner across the seam), `nonOrthDeltaCoeffs |Sf| / V` from the seam face alone, where the internal-face form gives 0; a field
/// equal on both sides of the seam gives the internal-face form bit for bit.
#[test]
fn coupled_laplacian_on_the_cyclic_column() {
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::fv::Geometry;
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{fvc_laplacian_uncorrected, fvc_laplacian_uncorrected_coupled};
    let s = thermal_solver("lap_unit", false, false, None);
    let g = Geometry::new_openfoam(s.case().mesh.clone());
    assert!(g.has_cyclic());
    let mut vf = vec![0.0; 640];
    vf[0] = 1.0;
    let (a, b) = (fvc_laplacian_uncorrected_coupled(&g, &vf), fvc_laplacian_uncorrected(&g, &vf));
    let seam = (0..g.n_faces()).find(|&f| g.cyclic_nbr(f) == Some(0)).expect("the seam face of cell 639");
    let m = &g.mesh;
    assert_eq!(m.owner[seam], 639);
    assert_eq!(b[639], 0.0);
    assert_eq!(a[639], g.delta[seam] * 1.0 * m.face_areas[seam] / m.cell_volumes[639]);
    assert!(a[0] < b[0], "cell 0 loses through the seam too: {} {}", a[0], b[0]);
    let lin: Vec<f64> = (0..640).map(|c| if c == 0 || c == 639 { 900.0 } else { 900.0 + (c as f64) * 1e-3 }).collect();
    assert_eq!(fvc_laplacian_uncorrected_coupled(&g, &lin), fvc_laplacian_uncorrected(&g, &lin));
}

/// Prediction V4: the variants to `t = 100`.
#[test]
fn thermal_kappa_matrix_variants_full_steady_state_against_the_real_binary() {
    for (v, refname) in [("core", "r81_th_kcore_full_tight_pcg"), ("seam", "r81_th_kseam_full_tight_pcg")] {
        let r = run(thermal_solver(&format!("th_k{v}_full"), true, true, Some(v)), refname, None);
        print(&format!("th_k{v}_full"), &r);
        println!("th_k{v}_full: final time {} upstream {}", r.final_time, r.last_time);
        // Results (2026-11-28): V4 HELD: `kcore` 7637 steps, worst `p` 3.3e-12, `T` 4.0e-13; `kseam` 7653 steps (the binary's), worst `p`
        // 3.7e-12, `T` 4.4e-13, `Tmatrix` 1.3e-14; final times equal to round-off
        assert!(r.first_over.is_none(), "V4 {v}: {:?}", r.first_over);
        assert!((r.final_time - r.last_time).abs() <= 1e-9 * r.last_time);
    }
    // the seam control over the whole stage
    let rx = run(thermal_solver("th_kseam_full_ctl", true, true, Some("seam")).without_coupled_lumped_laplacian(), "r81_th_kseam_full_tight_pcg", None);
    print("th_kseam_full_ctl", &rx);
    // Results (2026-11-28): the control leaves K1 at step 300 (`Tmax` 1.1e-9) and reaches 5.3e-7 (`T.fuelAvForNeutronics`, `Tmatrix`),
    // `Tsurface` 2.8e-8, `T` 1.3e-10, against 1.3e-14 / 2.7e-13 / 4.4e-13 with the coupled faces: over the whole stage the seam term is
    // what holds `kseam` inside 1e-9
    assert!(rx.first_over.is_some(), "the control leaves 1e-9 over the stage");
}
