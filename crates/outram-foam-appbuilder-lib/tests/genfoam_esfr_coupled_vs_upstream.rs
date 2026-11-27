// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # 3D_SmallESFR coupled (fluid + diffusion + legacy thermo-mechanics) vs the real binary (gh:#1944, README round 74)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, criteria, predictions: README "Round 74 (gh:#1944, 3D_SmallESFR coupled)", committed before each run. The case is staged by
//! `scripts/make_esfr_coupled_case.sh` (the shipped `legacySolver` plus the protocol edits named there); the directory is given by the
//! environment variable `ESFR_COUPLED_CASE` (an INFLATED copy: `INFLATE=1`), because the three 3-D meshes are too large to copy per test.
#![cfg(feature = "std")]
use std::path::PathBuf;

use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::{CouplingSwitches, RegionSolvers};

fn case_dir() -> Option<PathBuf> {
    std::env::var("ESFR_COUPLED_CASE").ok().map(PathBuf::from)
}

/// Round 74 probe: the first refusal (if any) of [`RegionSolvers::read`] on the shipped `legacySolver`. A read, not a run.
#[test]
fn probe_esfr_coupled_first_refusal() {
    let Some(dir) = case_dir() else {
        println!("ESFR_COUPLED_CASE not set: probe skipped");
        return;
    };
    match RegionSolvers::read(&dir, CouplingSwitches::default()) {
        Ok(s) => println!("READS: {} regions, end time {}", ["fluidRegion", "neutroRegion", "thermoMechanicalRegion"].iter().filter(|r| s.region(r).is_some()).count(), s.end_time()),
        Err(e) => println!("FIRST REFUSAL: {e}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The real references of README round 74 (`refs/esfr_r74_tight_*`, `scripts/extract_esfr_coupled_stream.py`) and the port's steps.

use std::collections::HashMap;
use std::path::Path;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::RegionSolver;
use outram_foam_basic_lib::primitives::Vector3;

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

#[derive(Default)]
struct Reference {
    /// `[keff_prev, keff, power, sigma]` per step.
    log: Vec<[f64; 4]>,
    dt: Vec<f64>,
    /// Per step: `p_rgh` / `h` solves `(initial residual, iterations)` and thermo-mechanics solves `(field, initial residual, iterations)`.
    prgh: Vec<Vec<(f64, i64)>>,
    h: Vec<Vec<(f64, i64)>>,
    tm: Vec<Vec<(String, f64, i64)>>,
    /// `(step, "region field")` -> `[n, sum, max, sumsq]`
    fp: HashMap<(usize, String), [f64; 4]>,
    full: HashMap<(usize, String), Vec<f64>>,
}

fn load_reference(tag: &str) -> Reference {
    let with_full = tag == "tight";
    let mut r = Reference::default();
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/esfr_r74_{tag}_log.ref.txt.gz")))).expect("utf8");
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let f = |i: usize| w[i].parse::<f64>().expect("float");
        match w[0] {
            "LOG" => r.log.push([f(2), f(3), f(4), f(5)]),
            "DT" => r.dt.push(f(2)),
            "PRGH" | "H" => {
                let n: usize = w[2].parse().unwrap();
                let v: Vec<(f64, i64)> = (0..n).map(|i| (f(3 + 2 * i), w[4 + 2 * i].parse().unwrap())).collect();
                if w[0] == "PRGH" {
                    r.prgh.push(v);
                } else {
                    r.h.push(v);
                }
            }
            "TM" => {
                let n: usize = w[2].parse().unwrap();
                r.tm.push((0..n).map(|i| (w[3 + 3 * i].to_string(), f(4 + 3 * i), w[5 + 3 * i].parse().unwrap())).collect());
            }
            "FP" => {
                let k: usize = w[1].parse().unwrap();
                r.fp.insert((k, format!("{} {}", w[2], w[3])), [f(4), f(5), f(6), f(7)]);
            }
            _ => {}
        }
    }
    if with_full {
        for region in ["fluidRegion", "neutroRegion", "thermoMechanicalRegion"] {
            let t = String::from_utf8(gunzip(&vv_dir().join(format!("refs/esfr_r74_tight_V_{region}.ref.txt.gz")))).expect("utf8");
            for line in t.lines() {
                let w: Vec<&str> = line.split_whitespace().collect();
                let k: usize = w[1].parse().unwrap();
                r.full.insert((k, format!("{} {}", w[2], w[3])), w[4..].iter().map(|x| x.parse().unwrap()).collect());
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
        (RegionSolver::OnePhase(f), "p_rgh") => f.solver.p_rgh().map(<[f64]>::to_vec),
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
    ("fluidRegion", &["T", "U", "p", "p_rgh", "T.passiveStructure", "T.fuelAvForNeutronics", "T.cladAvForNeutronics", "powerDensityNeutronics", "powerDensityNeutronicsToLiquid"]),
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

/// Round 74: the first `ESFR_STEPS` (default 3) steps of the coupled port against the real tight reference, printed per step; set
/// `ESFR_COUPLED_CASE` to an inflated `make_esfr_coupled_case.sh` directory (`TIGHT=0` for `ESFR_REF=loose`); the whole 200 steps take about
/// 6 minutes (`ESFR_STEPS=200`), over the 5-minute limit, so they are not run by default. No assertion: the criteria of README "Round 74" are judged on the
/// printed numbers (a miss is recorded in the README, not hidden in a loosened gate).
#[test]
fn esfr_coupled_steps_against_the_real_binary() {
    let Some(dir) = case_dir() else { return };
    let n_steps: usize = std::env::var("ESFR_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
    // `ESFR_REF=loose` compares with the real AS-SHIPPED (loose) run, macroscopic quantities and solve counts only (no field values were kept)
    let tag = std::env::var("ESFR_REF").unwrap_or_else(|_| "tight".to_string());
    let reference = load_reference(&tag);
    let mut run = RegionSolvers::read(&dir, CouplingSwitches::default()).unwrap_or_else(|e| panic!("read: {e}"));
    let mut first_diverging: Option<(usize, String, f64)> = None;
    // solve bookkeeping: [count mismatches, solves compared, worst relative initial residual (upstream above 1e-10), iteration differences > 1, worst iteration difference]
    let mut sol = [[0.0_f64; 8]; 3];
    for k in 1..=n_steps {
        let t0 = std::time::Instant::now();
        let rec = run.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        let wall = t0.elapsed().as_secs_f64();
        let lg = reference.log[k - 1];
        println!(
            "STEP {k} ({wall:.1} s): keff {:e} vs {:e} (rel {:e}); power {:e} vs {:e} (rel {:e}); sigma {:?} vs {:e}; dt {} vs {}",
            rec.k_eff, lg[1], rel(rec.k_eff, lg[1]), rec.power, lg[2], rel(rec.power, lg[2]), rec.max_sigma_eq, lg[3], rec.dt, reference.dt.get(k.wrapping_sub(2)).copied().unwrap_or(1.0)
        );
        let mut mine_solves: [Vec<(f64, i64)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        let mut tm_mine: Vec<(String, f64, i64)> = Vec::new();
        if let Some(r) = run.region("fluidRegion") {
            if let RegionSolver::OnePhase(f) = &r.solver {
                mine_solves[0] = f.solver.pressure_solves().iter().map(|m| (m.initial_residual, m.iterations as i64)).collect();
                mine_solves[1] = f.solver.h_solves().iter().map(|&(a, b)| (a, b as i64)).collect();
            }
        }
        if let Some(r) = run.region("thermoMechanicalRegion") {
            if let RegionSolver::ThermoMechanics(t) = &r.solver {
                if let Some(l) = &t.last_log {
                    tm_mine = l.solves.iter().map(|x| (x.field.to_string(), x.initial_residual, x.n_iterations as i64)).collect();
                }
            }
        }
        for (i, (mine, up)) in [(&mine_solves[0], &reference.prgh[k - 1]), (&mine_solves[1], &reference.h[k - 1])].into_iter().enumerate() {
            if mine.len() != up.len() {
                sol[i][0] += 1.0;
                println!("SOLVE COUNT DIFFERS: {} step {k}: port {} upstream {}", ["p_rgh", "h"][i], mine.len(), up.len());
                continue;
            }
            for (m, u) in mine.iter().zip(up) {
                sol[i][1] += 1.0;
                if u.0 > 1e-10 {
                    let r = ((m.0 - u.0) / u.0).abs();
                    if r > sol[i][2] {
                        // the worst relative difference, with the upstream residual it is of and the step
                        sol[i][2] = r;
                        sol[i][5] = u.0;
                        sol[i][6] = k as f64;
                    }
                    sol[i][7] = sol[i][7].max((m.0 - u.0).abs());
                }
                let di = (m.1 - u.1).abs() as f64;
                if di > 1.0 {
                    sol[i][3] += 1.0;
                }
                sol[i][4] = sol[i][4].max(di);
            }
        }
        {
            let up = &reference.tm[k - 1];
            if tm_mine.len() != up.len() || tm_mine.iter().zip(up).any(|(a, b)| a.0 != b.0) {
                sol[2][0] += 1.0;
                println!("SOLVE COUNT/ORDER DIFFERS: thermo-mechanics step {k}: port {} upstream {}", tm_mine.len(), up.len());
            } else {
                for (m, u) in tm_mine.iter().zip(up) {
                    sol[2][1] += 1.0;
                    if u.1 > 1e-10 {
                        let r = ((m.1 - u.1) / u.1).abs();
                        if r > sol[2][2] {
                            sol[2][2] = r;
                            sol[2][5] = u.1;
                            sol[2][6] = k as f64;
                        }
                        sol[2][7] = sol[2][7].max((m.1 - u.1).abs());
                    }
                    let di = (m.2 - u.2).abs() as f64;
                    if di > 1.0 {
                        sol[2][3] += 1.0;
                    }
                    sol[2][4] = sol[2][4].max(di);
                }
            }
        }
        for (region, names) in FIELDS {
            for name in names {
                let key = format!("{region} {name}");
                let Some(mine) = port_field(&run, region, name) else { continue };
                let Some(up) = reference.fp.get(&(k, key.clone())) else { continue };
                let (mut s, mut m, mut q) = (0.0, 0.0_f64, 0.0);
                for &x in &mine {
                    s += x;
                    m = m.max(x.abs());
                    q += x * x;
                }
                let worst = if *name == "U" { rel(m, up[2]) } else { rel(s, up[1]).max(rel(m, up[2])).max(rel(q, up[3])) };
                let mut line = format!("  FP {key}: {worst:e}");
                if let Some(full) = reference.full.get(&(k, key.clone())) {
                    if full.len() == mine.len() {
                        let scale = full.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
                        let diff = mine.iter().zip(full).fold(0.0_f64, |a, (x, y)| a.max((x - y).abs()));
                        line.push_str(&format!("; full-field max|d|/max|up| {:e}", if scale > 0.0 { diff / scale } else { diff }));
                    }
                }
                println!("{line}");
                if worst > 1e-9 && first_diverging.is_none() {
                    first_diverging = Some((k, key, worst));
                }
            }
        }
    }
    for (name, a) in ["p_rgh", "h", "thermo-mechanics"].iter().zip(sol) {
        println!(
            "SOLVES {name}: steps with a different solve count/order {}; solves compared {}; worst relative initial residual (upstream above 1e-10) {:e} (upstream value {:e}, step {}); worst ABSOLUTE initial-residual difference {:e}; iteration differences above 1: {}; worst iteration difference {}",
            a[0], a[1], a[2], a[5], a[6], a[7], a[3], a[4]
        );
    }
    println!("FIRST DIVERGING (fingerprint > 1e-9): {first_diverging:?}");
}
