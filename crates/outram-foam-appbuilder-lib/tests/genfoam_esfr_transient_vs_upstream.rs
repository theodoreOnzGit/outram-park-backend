// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # 3D_SmallESFR_old transient (`energyNeutroMechLoop multiPhysicsSolver`) vs the real binary (gh:#1950 S4, README round 75)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, criteria, predictions: README "Round 75, Part B", committed before each run. The case is staged by
//! `scripts/make_esfr_old_case.sh` (`transient` mode: the shipped `setTransient` edits on the real binary's written steady state); the directory
//! is given by `ESFR_T_CASE` (an INFLATED copy holding `constant`, `system`, `0` and the written `200`), the reference by `ESFR_T_REF`
//! (`tight` or `loose`, files `refs/esfr_r75_<tag>_log.ref.txt.gz` and, for `tight`, `esfr_r75_tight_V_<region>.ref.txt.gz`; written by
//! `scripts/extract_esfr_transient.py`) and the number of steps by `ESFR_T_STEPS` (default 3). No assertion on the real-binary numbers: the
//! criteria of the README are judged on the printed numbers; the default-run test of this file pins the reference parser only.
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::{CouplingSwitches, RegionSolver, RegionSolvers};
use outram_foam_basic_lib::primitives::Vector3;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip_bytes(bytes: &[u8]) -> Vec<u8> {
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
    decompress_to_vec(&bytes[at..bytes.len() - 8]).expect("inflate")
}

fn gunzip(path: &Path) -> Vec<u8> {
    gunzip_bytes(&std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())))
}

/// One step of the real log (`scripts/extract_esfr_transient.py`).
#[derive(Default, Debug, Clone, PartialEq)]
struct RefStep {
    time_name: String,
    /// `deltaT` printed after the step (the next step's).
    next_dt: f64,
    iterations: usize,
    converged: bool,
    h: Vec<(f64, i64)>,
    /// Per multiphysics pass: `(power, neutroResidual of each neutron iteration)`.
    neut: Vec<(f64, Vec<f64>)>,
    /// Per pass: the thermo-mechanics solves `(field, initial residual, iterations)`.
    tm: Vec<Vec<(String, f64, i64)>>,
}

#[derive(Default)]
struct Reference {
    steps: Vec<RefStep>,
    /// `(step, "region field")` -> `[n, sum, max, sumsq]`
    fp: HashMap<(usize, String), [f64; 4]>,
    full: HashMap<(usize, String), Vec<f64>>,
}

fn parse_log(text: &str) -> Reference {
    let mut r = Reference::default();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let f = |i: usize| w[i].parse::<f64>().expect("float");
        match w[0] {
            "STEP" => r.steps.push(RefStep { time_name: w[2].to_string(), ..RefStep::default() }),
            "DT" => r.steps.last_mut().expect("step").next_dt = f(2),
            "ITER" => {
                let s = r.steps.last_mut().expect("step");
                s.iterations = w[2].parse().expect("n");
                s.converged = w[3] == "1";
            }
            "H" => {
                let n: usize = w[2].parse().expect("n");
                r.steps.last_mut().expect("step").h = (0..n).map(|i| (f(3 + 2 * i), w[4 + 2 * i].parse().expect("it"))).collect();
            }
            "NEUT" => {
                let n: usize = w[4].parse().expect("n");
                r.steps.last_mut().expect("step").neut.push((f(3), (0..n).map(|i| f(5 + i)).collect()));
            }
            "TM" => {
                let n: usize = w[3].parse().expect("n");
                r.steps.last_mut().expect("step").tm.push((0..n).map(|i| (w[4 + 3 * i].to_string(), f(5 + 3 * i), w[6 + 3 * i].parse().expect("it"))).collect());
            }
            "FP" => {
                let k: usize = w[1].parse().expect("k");
                r.fp.insert((k, format!("{} {}", w[2], w[3])), [f(4), f(5), f(6), f(7)]);
            }
            _ => {}
        }
    }
    r
}

fn load_reference(tag: &str) -> Reference {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/esfr_r75_{tag}_log.ref.txt.gz")))).expect("utf8");
    let mut r = parse_log(&text);
    if tag.starts_with("tight") {
        for region in ["fluidRegion", "neutroRegion", "thermoMechanicalRegion"] {
            let p = vv_dir().join(format!("refs/esfr_r75_{tag}_V_{region}.ref.txt.gz"));
            if !p.exists() {
                continue;
            }
            let t = String::from_utf8(gunzip(&p)).expect("utf8");
            for line in t.lines() {
                let w: Vec<&str> = line.split_whitespace().collect();
                let k: usize = w[1].parse().expect("k");
                r.full.insert((k, format!("{} {}", w[2], w[3])), w[4..].iter().map(|x| x.parse().expect("float")).collect());
            }
        }
    }
    r
}

fn vec_flat(v: &[Vector3]) -> Vec<f64> {
    v.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

/// The port's value of `region field` as a flat list, if it has one.
fn port_field(run: &RegionSolvers, region: &str, name: &str) -> Option<Vec<f64>> {
    let r = run.region(region)?;
    match (&r.solver, name) {
        (RegionSolver::OnePhase(f), "T") => Some(f.solver.temperature().to_vec()),
        (RegionSolver::OnePhase(f), "U") => Some(vec_flat(f.solver.velocity())),
        (RegionSolver::OnePhase(f), "p") => Some(f.solver.pressure().to_vec()),
        (RegionSolver::OnePhase(f), "T.passiveStructure") => Some(f.solver.passive_structure_temperature().to_vec()),
        (RegionSolver::OnePhase(f), "T.fuelAvForNeutronics") => Some(f.solver.fuel_clad_temperatures().0),
        (RegionSolver::OnePhase(f), "T.cladAvForNeutronics") => Some(f.solver.fuel_clad_temperatures().1),
        (RegionSolver::Neutronics(n), "flux0") => Some(n.solver.state().flux()[0].internal.as_slice().to_vec()),
        (RegionSolver::ThermoMechanics(t), "disp") => Some(vec_flat(t.solver.displacement())),
        (RegionSolver::ThermoMechanics(t), "meshDisp") => Some(vec_flat(t.solver.mesh_displacement())),
        (RegionSolver::ThermoMechanics(t), "TStruct") => Some(t.solver.t_struct().to_vec()),
        (RegionSolver::ThermoMechanics(t), "fuelDisp") => Some(t.solver.fuel_displacement().to_vec()),
        (RegionSolver::ThermoMechanics(t), "CRDisp") => Some(t.solver.control_rod_displacement().to_vec()),
        _ => r.fields.scalar(name).map(<[f64]>::to_vec).or_else(|| r.fields.vector(name).map(vec_flat)),
    }
}

const FIELDS: [(&str, &[&str]); 3] = [
    ("fluidRegion", &["T", "U", "p", "T.passiveStructure", "T.fuelAvForNeutronics", "T.cladAvForNeutronics", "powerDensityNeutronics"]),
    ("neutroRegion", &["disp", "powerDensity", "flux0", "TCool", "TFuel", "TClad", "TStructMech", "rhoCool"]),
    ("thermoMechanicalRegion", &["disp", "meshDisp", "TStruct", "TStructFromTH", "TFuel", "powerDensityNeutronics", "fuelDisp", "CRDisp"]),
];

fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a.abs() * 1e30
    } else {
        ((a - b) / b).abs()
    }
}

/// Initial residuals of a solve list against the real one: `(count/order equal, worst relative above 1e-10 absolute, worst absolute, iteration
/// differences above 1, worst iteration difference)`.
fn compare_solves(mine: &[(String, f64, i64)], up: &[(String, f64, i64)]) -> (bool, f64, f64, usize, i64) {
    if mine.len() != up.len() || mine.iter().zip(up).any(|(a, b)| a.0 != b.0) {
        return (false, 0.0, 0.0, 0, 0);
    }
    let (mut worst_rel, mut worst_abs, mut over1, mut worst_it) = (0.0_f64, 0.0_f64, 0usize, 0i64);
    for (m, u) in mine.iter().zip(up) {
        if u.1 > 1e-10 {
            worst_rel = worst_rel.max(((m.1 - u.1) / u.1).abs());
        }
        worst_abs = worst_abs.max((m.1 - u.1).abs());
        let d = (m.2 - u.2).abs();
        if d > 1 {
            over1 += 1;
        }
        worst_it = worst_it.max(d);
    }
    (true, worst_rel, worst_abs, over1, worst_it)
}

/// The reference parser reads the per-step records of `extract_esfr_transient.py` (a synthetic two-step log; default-run pin of the parser and
/// of `compare_solves`).
#[test]
fn the_reference_parser_and_the_solve_comparison_read_the_extractor_format() {
    let text = "STEP 1 200.00001\nDT 1 1.2e-05\nITER 1 2 0\nH 1 2 1e-11 0 2e-11 1\nNEUT 1 0 8.1e8 2 1e-3 1e-6\nFLUX 1 0 2 1e-3 20 1e-6 0\nTM 1 0 2 dispx 0.1 29 CRDisp 1e-5 0\nNEUT 1 1 8.2e8 1 2e-5\nFLUX 1 1 0\nTM 1 1 1 dispx 0.2 30\nFP 1 fluidRegion T 3 6.0 3.0 14.0\nSTEP 2 200.000022\nDT 2 1.44e-05\nITER 2 1 1\nH 2 0 \nNEUT 2 0 8.3e8 0 \nFLUX 2 0 0\nTM 2 0 0 \n";
    let r = parse_log(text);
    assert_eq!(r.steps.len(), 2);
    assert_eq!(r.steps[0].iterations, 2);
    assert!(!r.steps[0].converged && r.steps[1].converged);
    assert_eq!(r.steps[0].h, vec![(1e-11, 0), (2e-11, 1)]);
    assert_eq!(r.steps[0].neut[0], (8.1e8, vec![1e-3, 1e-6]));
    assert_eq!(r.steps[0].tm[1], vec![("dispx".to_string(), 0.2, 30)]);
    assert_eq!(r.steps[1].next_dt, 1.44e-5);
    assert_eq!(r.fp[&(1, "fluidRegion T".to_string())], [3.0, 6.0, 3.0, 14.0]);
    let a = vec![("dispx".to_string(), 0.1, 29)];
    let b = vec![("dispx".to_string(), 0.1 * (1.0 + 1e-9), 31)];
    let (eq, worst_rel, _abs, over1, worst_it) = compare_solves(&a, &b);
    assert!(eq && worst_rel < 2e-9 && over1 == 1 && worst_it == 2);
    assert!(!compare_solves(&a, &[]).0);
}

/// README round 75 Part B: the first `ESFR_T_STEPS` steps (default 3) of the port's multiPhysicsSolver transient, read from the real binary's
/// written steady state, against the real log; prints every number. Not run without `ESFR_T_CASE`.
#[test]
fn esfr_transient_steps_against_the_real_binary() {
    let Some(dir) = std::env::var("ESFR_T_CASE").ok().map(PathBuf::from) else {
        println!("ESFR_T_CASE not set: skipped");
        return;
    };
    let n_steps: usize = std::env::var("ESFR_T_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
    let tag = std::env::var("ESFR_T_REF").unwrap_or_else(|_| "tight".to_string());
    let reference = load_reference(&tag);
    // CONTROLS for isolating a difference (one named edit each; never the comparison itself): `ESFR_T_SWITCH=no_deform|no_disp_map|no_xs_feedback`
    let switches = match std::env::var("ESFR_T_SWITCH").as_deref() {
        Ok("no_deform") => CouplingSwitches::default().without_mesh_deformation(),
        Ok("no_disp_map") => CouplingSwitches::default().without_mapping_into("neutroRegion", "disp"),
        _ => CouplingSwitches::default(),
    };
    let mut run = RegionSolvers::read(&dir, switches).unwrap_or_else(|e| panic!("read: {e}"));
    if std::env::var("ESFR_T_FP0").is_ok() {
        // the fingerprints of the fields right after `read` (to compare with the written start directory)
        for (region, names) in FIELDS {
            for name in names {
                let Some(mine) = port_field(&run, region, name) else { continue };
                let (mut s, mut m, mut q) = (0.0, 0.0_f64, 0.0);
                for &x in &mine {
                    s += x;
                    m = m.max(x.abs());
                    q += x * x;
                }
                println!("FP0 {region} {name} {} {s:?} {m:?} {q:?}", mine.len());
            }
        }
    }
    let mut first_diverging: Option<(usize, String, f64)> = None;
    let (mut worst_power, mut worst_dt) = ((0.0_f64, 0usize), (0.0_f64, 0usize));
    let (mut iter_mismatch, mut h_mismatch, mut tm_mismatch, mut neut_mismatch) = (0usize, 0usize, 0usize, 0usize);
    let (mut h_worst, mut tm_worst, mut neut_worst) = ((0.0_f64, 0usize), (0.0_f64, 0usize), (0.0_f64, 0usize));
    let (mut tm_it_over1, mut h_it_over1) = (0usize, 0usize);
    let mut k = 0usize;
    while run.running() && k < n_steps {
        k += 1;
        let t0 = std::time::Instant::now();
        let rec = run.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        let wall = t0.elapsed().as_secs_f64();
        let Some(up) = reference.steps.get(k - 1) else {
            println!("STEP {k}: no reference step");
            break;
        };
        let dt_up = if k == 1 { 1e-5 } else { reference.steps[k - 2].next_dt };
        let dt_rel = rel(rec.dt, dt_up);
        if dt_rel > worst_dt.0 {
            worst_dt = (dt_rel, k);
        }
        let p_up = up.neut.last().map_or(f64::NAN, |n| n.0);
        let p_rel = rel(rec.power, p_up);
        if p_rel > worst_power.0 {
            worst_power = (p_rel, k);
        }
        if rec.multi_iterations != up.iterations {
            iter_mismatch += 1;
        }
        // the fluid `h` solves of the whole step
        let mine_h: Vec<(String, f64, i64)> = rec.h_solves.iter().map(|&(a, b)| ("h".to_string(), a, b as i64)).collect();
        let up_h: Vec<(String, f64, i64)> = up.h.iter().map(|&(a, b)| ("h".to_string(), a, b)).collect();
        if std::env::var("ESFR_T_VERBOSE").is_ok() {
            println!("  H solves step {k}: port {:?} ... upstream {:?}", rec.h_solves.iter().take(3).collect::<Vec<_>>(), up.h.iter().take(3).collect::<Vec<_>>());
        }
        let (eq, wr, _wa, o1, _wi) = compare_solves(&mine_h, &up_h);
        if !eq {
            h_mismatch += 1;
            println!("H SOLVES DIFFER in count: step {k}: port {} upstream {}", mine_h.len(), up_h.len());
        } else {
            if wr > h_worst.0 {
                h_worst = (wr, k);
            }
            h_it_over1 += o1;
        }
        // per pass: neutron residuals and the thermo-mechanics solves
        for (j, it) in rec.iterations.iter().enumerate() {
            if let (Some(ns), Some(un)) = (&it.neutronics, up.neut.get(j)) {
                if std::env::var("ESFR_T_VERBOSE").is_ok() {
                    println!("  PASS {j}: power {:e} vs {:e} (rel {:e}); neutroResidual port {:?} upstream {:?}; TM first solve port {:?}", ns.power, un.0, rel(ns.power, un.0), ns.residual_history, un.1, it.thermo_mechanics.first());
                }
                if ns.residual_history.len() != un.1.len() {
                    neut_mismatch += 1;
                    println!("NEUTRON ITERATIONS DIFFER: step {k} pass {j}: port {} upstream {}", ns.residual_history.len(), un.1.len());
                } else {
                    for (a, b) in ns.residual_history.iter().zip(&un.1) {
                        if *b > 1e-10 {
                            let r = rel(*a, *b);
                            if r > neut_worst.0 {
                                neut_worst = (r, k);
                            }
                        }
                    }
                }
            } else {
                neut_mismatch += 1;
            }
            let mine_tm: Vec<(String, f64, i64)> = it.thermo_mechanics.iter().map(|(f, a, b)| (f.clone(), *a, *b as i64)).collect();
            match up.tm.get(j) {
                Some(u) => {
                    let (eq, wr, _wa, o1, _wi) = compare_solves(&mine_tm, u);
                    if !eq {
                        tm_mismatch += 1;
                        println!("TM SOLVES DIFFER in count/order: step {k} pass {j}: port {} upstream {}", mine_tm.len(), u.len());
                        println!("  port initial residuals {:?}", mine_tm.iter().map(|x| (x.0.as_str(), x.1, x.2)).collect::<Vec<_>>());
                        println!("  upstream initial residuals {:?}", u.iter().map(|x| (x.0.as_str(), x.1, x.2)).collect::<Vec<_>>());
                    } else {
                        if wr > tm_worst.0 {
                            tm_worst = (wr, k);
                        }
                        tm_it_over1 += o1;
                    }
                }
                None => tm_mismatch += 1,
            }
        }
        println!(
            "STEP {k} ({wall:.1} s): t {} (real {}), dt {:e} vs {:e} (rel {:e}); power {:e} vs {:e} (rel {:e}); passes {} vs {}; residual {:e}",
            run.time(), up.time_name, rec.dt, dt_up, dt_rel, rec.power, p_up, p_rel, rec.multi_iterations, up.iterations, rec.multi_residual
        );
        if k == 1 && std::env::var("ESFR_T_FPK").is_ok() {
            if let Some(r) = run.region("neutroRegion") {
                if let RegionSolver::Neutronics(n) = &r.solver {
                    if let Some(tf) = r.fields.scalar("TFuel") {
                        println!("NEUTRO TFuel: nonzero cells {} (real 200.00001: 7762; at 200: 6540), sum {:e} (real 9.0540526661e6)", tf.iter().filter(|x| **x != 0.0).count(), tf.iter().sum::<f64>());
                    }
                    println!("NEUTRO mesh volume {:e}", n.mesh.mesh().cell_volumes.iter().sum::<f64>());
                    if let Some(d) = r.fields.vector("disp") {
                        let e = outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::expansion::expansion_from_displacement(n.mesh.mesh(), d, Vector3::new(0.0, 0.0, 1.0));
                        println!("EXPANSION port: axExp sum {:e} min {:e} max {:e}; radExp sum {:e} (real 200.00001: axExp 5.1813426898e1, radExp 1.7655721113e1)", e.ax_exp.iter().sum::<f64>(), e.ax_exp.iter().cloned().fold(f64::MAX, f64::min), e.ax_exp.iter().cloned().fold(f64::MIN, f64::max), e.rad_exp.iter().sum::<f64>());
                    }
                }
            }
            for (region, names) in FIELDS {
                for name in names {
                    let Some(mine) = port_field(&run, region, name) else { continue };
                    let (mut s, mut m, mut q) = (0.0, 0.0_f64, 0.0);
                    for &x in &mine {
                        s += x;
                        m = m.max(x.abs());
                        q += x * x;
                    }
                    println!("FP0 {region} {name} {} {s:?} {m:?} {q:?}", mine.len());
                }
            }
        }
        for (region, names) in FIELDS {
            for name in names {
                let key = format!("{region} {name}");
                let Some(mine) = port_field(&run, region, name) else { continue };
                let Some(u) = reference.fp.get(&(k, key.clone())) else { continue };
                let (mut s, mut m, mut q) = (0.0, 0.0_f64, 0.0);
                for &x in &mine {
                    s += x;
                    m = m.max(x.abs());
                    q += x * x;
                }
                let worst = if *name == "U" { rel(m, u[2]) } else { rel(s, u[1]).max(rel(m, u[2])).max(rel(q, u[3])) };
                let mut line = format!("  FP {key}: {worst:e}");
                if let Some(full) = reference.full.get(&(k, key.clone())) {
                    if full.len() == mine.len() {
                        let scale = full.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
                        let diff = mine.iter().zip(full).fold(0.0_f64, |a, (x, y)| a.max((x - y).abs()));
                        line.push_str(&format!("; full-field max|d|/max|up| {:e}", if scale > 0.0 { diff / scale } else { diff }));
                        if std::env::var("ESFR_T_WORST").is_ok_and(|w| w == key) && scale > 0.0 && diff / scale > 1e-9 {
                            let dim = if *name == "U" || *name == "disp" || *name == "meshDisp" { 3 } else { 1 };
                            let mut cells: Vec<(f64, usize)> = (0..mine.len()).map(|i| ((mine[i] - full[i]).abs(), i)).collect();
                            cells.sort_by(|a, b| b.0.total_cmp(&a.0));
                            let fuel = port_field(&run, "fluidRegion", "T.fuelAvForNeutronics").unwrap_or_default();
                            for (d, i) in cells.iter().take(6) {
                                let c = i / dim;
                                println!("    WORST {key} step {k} cell {c}: port {:e} real {:e} diff {d:e}; fuelAv {:e}", mine[*i], full[*i], fuel.get(c).copied().unwrap_or(f64::NAN));
                            }
                            let in_fuel = cells.iter().take(200).filter(|(_, i)| fuel.get(i / dim).is_some_and(|f| *f != 0.0)).count();
                            println!("    of the 200 worst cells {in_fuel} are fuel-pin cells (T.fuelAvForNeutronics != 0)");
                        }
                    }
                }
                println!("{line}");
                if worst > 1e-9 && first_diverging.is_none() {
                    first_diverging = Some((k, key, worst));
                }
            }
        }
    }
    println!("STEPS {k}: worst power {:e} (step {}); worst dt {:e} (step {}); pass-count mismatches {iter_mismatch}", worst_power.0, worst_power.1, worst_dt.0, worst_dt.1);
    println!("H: step solve-count mismatches {h_mismatch}; worst relative initial residual (upstream above 1e-10) {:e} (step {}); iteration differences above 1: {h_it_over1}", h_worst.0, h_worst.1);
    println!("NEUTRON: pass mismatches {neut_mismatch}; worst relative neutroResidual {:e} (step {})", neut_worst.0, neut_worst.1);
    println!("TM: pass solve-count/order mismatches {tm_mismatch}; worst relative initial residual (upstream above 1e-10) {:e} (step {}); iteration differences above 1: {tm_it_over1}", tm_worst.0, tm_worst.1);
    println!("FIRST DIVERGING (fingerprint > 1e-9): {first_diverging:?}");
}

/// Diagnostic of README round 75 (not a criterion): the total overlap volumes `cellVolumeWeight` finds between the neutronics mesh as the restart
/// reads it (the written moved points) and the other regions, against the lines `Overlap volume:` of the real log (`ESFR_T_CASE`).
#[test]
fn esfr_restart_overlap_volumes_against_the_real_log() {
    use outram_foam_appbuilder_lib::genfoam::multi_region::mesh_deformation::PointCellsOrder;
    use outram_foam_appbuilder_lib::genfoam::multi_region::{CellVolumeWeightAddressing, DeformableMesh, OverlapMesh};
    let Some(dir) = std::env::var("ESFR_T_CASE").ok().map(PathBuf::from) else {
        println!("ESFR_T_CASE not set: skipped");
        return;
    };
    let read = |name: &str, moved: bool| {
        let dm = DeformableMesh::read(&dir.join(format!("constant/{name}/polyMesh")), PointCellsOrder::CellAscending).expect("mesh");
        let dm = if moved {
            let text = std::fs::read_to_string(dir.join(format!("200/{name}/polyMesh/points"))).expect("points");
            let pts = outram_foam_appbuilder_lib::io::poly_mesh::parse_points(&text, "points").expect("parse");
            let pts: Vec<Vector3> = pts.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
            dm.restarted_at(&pts).expect("restart")
        } else {
            dm
        };
        OverlapMesh::from_deformable(&dm).expect("overlap mesh")
    };
    let (fluid, neutro, tm) = (read("fluidRegion", false), read("neutroRegion", true), read("thermoMechanicalRegion", false));
    let v = |a: &OverlapMesh, b: &OverlapMesh| CellVolumeWeightAddressing::calculate(a, b).v;
    println!("fluid,neutro   {:.15} (real 14.990306743599364)", v(&fluid, &neutro));
    println!("neutro,fluid   {:.15} (real neutro,fluid.baffleLess 14.995428355239389)", v(&neutro, &fluid));
    let bl = OverlapMesh::from_deformable(&DeformableMesh::read(&dir.join("200/fluidRegion.baffleLess/polyMesh"), PointCellsOrder::CellAscending).expect("baffleLess")).expect("overlap");
    println!("neutro,fluid.baffleLess {:.15} (real 14.995428355239389); tm,baffleLess {:.15} (real 15.024604045610079)", v(&neutro, &bl), v(&tm, &bl));
    println!("neutro,tm      {:.15} (real 14.99542835523939)", v(&neutro, &tm));
    println!("tm,neutro      {:.15} (real 14.991589596377745)", v(&tm, &neutro));
    println!("tm,fluid       {:.15} (real tm,fluid.baffleLess 15.024604045610079)", v(&tm, &fluid));
}
