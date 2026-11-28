// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The shipped `1D_MSR_pointKinetics` `transient` stage COUPLED (cyclic `onePhase` fluid + liquid-fuel point kinetics) against the
//! real binary (gh:#1905, README round 80, steps 2-4)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, unmodified, serial. `scripts/run_msr1d_transient_r80.sh` runs Allrun's
//! `setTransient` stage exactly (from the real `steadyState` stage's written `100`: `0/neutroRegion` and `0/uniform` copied in, `endTime 400`,
//! `regionSolvers.neutroRegion pointKinetics`, `multiRegionCouplingDict_Coupled`, `GeN-Foam -initializeMappedFields`) plus
//! `writePrecision 17` (and, `first`, every step written for 60 steps). The port reads the SAME start: `cases/msr1d_root` with the same
//! controlDict edits and the real `100` directory after `-initializeMappedFields` (`refs/msr1d_r80_tr_start100.txt.gz`,
//! `scripts/pack_start_dir_r66.py`), and runs its own coupled loop (`RegionSolvers`: the fluid `onePhase` region of round 80 step 1, the
//! mapping of `multiRegionCouplingDict_Coupled`, the liquid-fuel `pointKineticNeutronics` of round 48 read from the case). Nothing of the
//! real run's output is an input. References: `refs/msr1d_r80_tr_{first,full}{,_log}.ref.txt.gz` (`scripts/extract_msr1d_transient_r80.sh`).
//!
//! Criteria (README "Round 80", step 4): written before the first port run of this test; results in each test's doc comment.
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::{CouplingSwitches, RegionSolver, RegionSolvers};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 8 != 0 {
        while bytes[at] != 0 {
            at += 1;
        }
        at += 1;
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

/// Unpack `FILE\t<rel>\t<bytes>\n<bytes>\n` records into `dir`.
fn unpack(packed: &[u8], dir: &Path) {
    let mut at = 0;
    while at < packed.len() {
        let nl = at + packed[at..].iter().position(|&b| b == b'\n').expect("header");
        let header = std::str::from_utf8(&packed[at..nl]).expect("utf8");
        let parts: Vec<&str> = header.split('\t').collect();
        assert_eq!(parts[0], "FILE");
        let len: usize = parts[2].parse().expect("len");
        let path = dir.join(parts[1]);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, &packed[nl + 1..nl + 1 + len]).expect("write");
        at = nl + 1 + len + 1;
    }
}

/// Allrun's `setTransient` controlDict edits on the shipped rootCase, the coupling dictionary, and the real `100` start directory.
/// `every_step`: the `first` protocol of `run_msr1d_transient_r80.sh` (`writeControl timeStep; writeInterval 1`), under which `setDeltaT` does
/// NOT align the step to a write time; otherwise Allrun's `adjustableRunTime` every 10 s, which does (`Time::adjustDeltaT`).
fn stage(tag: &str, every_step: bool) -> PathBuf {
    stage_case(&MSR, tag, every_step)
}

/// A shipped 1-D MSR tutorial (round 81, gh:#1962: the transient harness serves `1D_MSR` and `1D_thermalMSR`).
struct Case {
    /// `cases/<dir>`: the shipped `rootCase`.
    dir: &'static str,
    /// The references' prefix in `refs/` (`<prefix>tr_first`, `<prefix>tr_full`, `<prefix>tr_start100.txt.gz`).
    refs: &'static str,
    /// Allrun's `setTransient` also sets `startFrom startTime; startTime 100` (1D_thermalMSR, whose rootCase reads `startTime 0`).
    start_time_edit: bool,
}

const MSR: Case = Case { dir: "msr1d_root", refs: "msr1d_r80_", start_time_edit: false };
const THERMAL: Case = Case { dir: "msr1d_thermal_root", refs: "msr1d_r81_th_", start_time_edit: true };

/// [`stage`] of any [`Case`].
fn stage_case(c: &Case, tag: &str, every_step: bool) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1905_tr_{}{tag}", c.refs));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join(format!("cases/{}", c.dir)), &root);
    std::fs::copy(root.join("constant/multiRegionCouplingDict_Coupled"), root.join("constant/multiRegionCouplingDict")).expect("coupling dict");
    let cd = root.join("system/controlDict");
    let t = std::fs::read_to_string(&cd).expect("controlDict");
    let t = t
        .replacen("endTime         1000;", "endTime         400;", 1)
        .replacen("writeInterval   1000;", "writeInterval   10;", 1)
        .replacen("    fluidRegion     onePhase;\n", "    fluidRegion     onePhase;\n    neutroRegion    pointKinetics;\n", 1)
        .replacen("    fluidRegion     false;\n", "    fluidRegion     false;\n    neutroRegion    false;\n", 1);
    assert!(t.contains("neutroRegion    pointKinetics;") && t.contains("neutroRegion    false;"), "controlDict edits");
    let t = if c.start_time_edit {
        assert!(t.contains("startFrom       startTime;") && t.contains("startTime       0;"), "startTime edit");
        t.replacen("startTime       0;", "startTime       100;", 1)
    } else {
        t
    };
    let t = if every_step {
        let t = t.replacen("writeControl    adjustableRunTime;", "writeControl    timeStep;", 1).replacen("writeInterval   10;", "writeInterval   1;", 1);
        assert!(t.contains("writeControl    timeStep;") && t.contains("writeInterval   1;"), "first-protocol edits");
        t
    } else {
        t
    };
    std::fs::write(&cd, t).expect("write controlDict");
    unpack(&gunzip(&vv_dir().join(format!("refs/{}tr_start100.txt.gz", c.refs))), &root.join("100"));
    root
}

#[derive(Default)]
struct Log {
    power: HashMap<usize, f64>,
    react: HashMap<usize, f64>,
    lfbeta: HashMap<usize, f64>,
    peq: HashMap<usize, f64>,
    nres: HashMap<usize, Vec<f64>>,
    prec: HashMap<usize, Vec<(f64, i64)>>,
    dt_next: HashMap<usize, f64>,
    step_time: HashMap<usize, String>,
    prgh: HashMap<usize, Vec<(f64, i64)>>,
    tstat: HashMap<usize, [f64; 3]>,
}

fn read_log(name: &str) -> Log {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/{name}")))).expect("utf8");
    let mut l = Log::default();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() < 3 {
            continue;
        }
        let s: usize = w[1].parse().unwrap_or(0);
        let f = |i: usize| w[i].parse::<f64>().expect("float");
        match w[0] {
            "POWER" => {
                l.power.insert(s, f(2));
            }
            "REACT" => {
                l.react.insert(s, f(2));
            }
            "LFBETA" => {
                l.lfbeta.insert(s, f(2));
            }
            "PEQ" => {
                l.peq.insert(s, f(2));
            }
            "NRES" => l.nres.entry(s).or_default().push(f(3)),
            "PREC" => l.prec.entry(s).or_default().push((f(3), w[4].parse().expect("int"))),
            "PRGH" => l.prgh.entry(s).or_default().push((f(4), w[5].parse().expect("int"))),
            "TSTAT" => {
                l.tstat.insert(s, [f(3), f(4), f(5)]);
            }
            "DTNEXT" => {
                l.dt_next.insert(s, f(2));
            }
            "STEPTIME" => {
                l.step_time.insert(s, w[2].to_string());
            }
            _ => {}
        }
    }
    l
}

fn read_fields(name: &str) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/{name}")))).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("F") {
            let time = it.next().expect("time");
            let name = it.next().expect("name");
            let v: Vec<f64> = it.map(|x| x.parse().expect("number")).collect();
            if !v.is_empty() {
                m.insert(format!("{name} {time}"), v);
            }
        }
    }
    m
}

fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a.abs()
    } else {
        ((a - b) / b).abs()
    }
}

fn rel_max(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let s = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let d = a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
    if s == 0.0 {
        d
    } else {
        d / s
    }
}

#[derive(Debug, Default)]
struct Report {
    steps: usize,
    power: f64,
    react: f64,
    lfbeta: f64,
    peq: f64,
    dt: f64,
    nres_count_diff: usize,
    prec_its_diff: i64,
    prgh_its_diff: i64,

    fields: Vec<(String, f64)>,
    first_nonzero: Option<(usize, String, f64)>,
    first_over: Option<(usize, String, f64)>,
    trace: Vec<String>,
    final_time: f64,
}

fn note(rep: &mut Report, step: usize, what: &str, d: f64) {
    if d > 0.0 && rep.first_nonzero.is_none() {
        rep.first_nonzero = Some((step, what.to_string(), d));
    }
    if d > 1e-9 && rep.first_over.is_none() {
        rep.first_over = Some((step, what.to_string(), d));
    }
    match rep.fields.iter_mut().find(|(n, _)| n == what) {
        Some(e) => e.1 = e.1.max(d),
        None => rep.fields.push((what.to_string(), d)),
    }
}

/// `zero_prec`: the control, the precursors started from 0 (the port before round 80) instead of `defaultPrec` (upstream).
fn run(tag: &str, refname: &str, n_steps: usize, every_step: bool, zero_prec: bool) -> Report {
    run_case(&MSR, tag, refname, n_steps, every_step, zero_prec)
}

/// [`run`] of any [`Case`].
fn run_case(c: &Case, tag: &str, refname: &str, n_steps: usize, every_step: bool, zero_prec: bool) -> Report {
    let lg = read_log(&format!("{}{refname}_log.ref.txt.gz", c.refs));
    let rf = read_fields(&format!("{}{refname}.ref.txt.gz", c.refs));
    let root = stage_case(c, tag, every_step);
    let mut run = RegionSolvers::read(&root, CouplingSwitches::default()).unwrap_or_else(|e| panic!("the staged transient is not readable: {e}"));
    if zero_prec {
        for r in run.regions.iter_mut() {
            if let RegionSolver::PointKinetics(p) = &mut r.solver {
                let lf = p.liquid.take().expect("the liquid-fuel class");
                let n = lf.transport_mesh().n_cells;
                p.liquid = Some(lf.with_default_prec(&vec![0.0; n]).expect("zero precursors"));
            }
        }
    }
    let mut rep = Report::default();
    for k in 1..=n_steps {
        let rec = run.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        rep.steps = k;
        if k > 1 {
            if let Some(&up) = lg.dt_next.get(&(k - 1)) {
                rep.dt = rep.dt.max(rel(rec.dt, up));
                if k <= 6 {
                    rep.trace.push(format!("step {k} dt port {:e} up {up:e}", rec.dt));
                }
            }
        }
        let (mut power, mut lfb, mut peq, mut react) = (0.0, 0.0, 0.0, 0.0);
        let (mut nres, mut prec): (Vec<f64>, Vec<(f64, usize)>) = (Vec::new(), Vec::new());
        let mut pk_fields: Vec<(String, Vec<f64>)> = Vec::new();
        let mut fl_fields: Vec<(String, Vec<f64>)> = Vec::new();
        let mut prgh: Vec<(f64, usize)> = Vec::new();
        let mut tline = [0.0; 3];
        for r in &run.regions {
            match &r.solver {
                RegionSolver::PointKinetics(p) => {
                    power = p.pk.power();
                    react = p.pk.total_reactivity() * 1e5;
                    if let Some(l) = &p.liquid_last {
                        lfb = l.liquid_fuel_beta * 1e5;
                        peq = l.prec_equilibrium_reactivity * 1e5;
                        nres = l.neutro_residuals.clone();
                        prec = l.solves.iter().map(|s| (s.1, s.0)).collect();
                    }
                    if let Some(lf) = &p.liquid {
                        for g in 0..8 {
                            pk_fields.push((format!("neutroRegion/precPKStar{g}"), lf.prec_pk_star(g).to_vec()));
                        }
                    }
                    pk_fields.push(("neutroRegion/powerDensity".to_string(), p.pk.power_density.clone()));
                    for nm in ["TFuel", "TCool", "rhoCool"] {
                        if let Some(v) = r.fields.scalar(nm) {
                            pk_fields.push((format!("neutroRegion/{nm}"), v.to_vec()));
                        }
                    }
                }
                RegionSolver::OnePhase(f) => {
                    let s = &f.solver;
                    fl_fields.push(("fluidRegion/U".to_string(), s.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect()));
                    fl_fields.push(("fluidRegion/p".to_string(), s.pressure().to_vec()));
                    fl_fields.push(("fluidRegion/T".to_string(), s.temperature().to_vec()));
                    if let Some(kk) = s.turbulent_kinetic_energy() {
                        fl_fields.push(("fluidRegion/k".to_string(), kk.to_vec()));
                    }
                    fl_fields.extend(structure_fields(s));
                    prgh = s.pressure_solves().iter().map(|x| (x.initial_residual, x.iterations)).collect();
                    let _ = &mut tline;
                    rep.final_time = s.time();
                }
                _ => {}
            }
        }
        if let Some(&up) = lg.power.get(&k) {
            let d = rel(power, up);
            rep.power = rep.power.max(d);
            note(&mut rep, k, "power", d);
            if k <= 5 || k % 10 == 0 {
                rep.trace.push(format!("step {k} power {power:e} up {up:e} react {react:e} up {:e}", lg.react.get(&k).copied().unwrap_or(f64::NAN)));
            }
        }
        if let Some(&up) = lg.react.get(&k) {
            rep.react = rep.react.max(rel(react, up));
        }
        if let Some(&up) = lg.lfbeta.get(&k) {
            rep.lfbeta = rep.lfbeta.max(rel(lfb, up));
        }
        if let Some(&up) = lg.peq.get(&k) {
            rep.peq = rep.peq.max(rel(peq, up));
        }
        if let Some(up) = lg.nres.get(&k) {
            if up.len() != nres.len() {
                rep.nres_count_diff += 1;
            }
        }
        if let Some(up) = lg.prec.get(&k) {
            for (a, b) in up.iter().zip(&prec) {
                rep.prec_its_diff = rep.prec_its_diff.max((a.1 - b.1 as i64).abs());
            }
        }
        if let Some(up) = lg.prgh.get(&k) {
            for (a, b) in up.iter().zip(&prgh) {
                rep.prgh_its_diff = rep.prgh_its_diff.max((a.1 - b.1 as i64).abs());
            }
        }
        if let Some(time) = lg.step_time.get(&k) {
            for (nm, v) in pk_fields.iter().chain(&fl_fields) {
                if let Some(up) = rf.get(&format!("{nm} {time}")) {
                    if up.len() == v.len() {
                        let d = rel_max(v, up);
                        if k <= 3 {
                            let nd = v.iter().zip(up).filter(|(a, b)| a != b).count();
                            let wi = (0..v.len()).max_by(|&i, &j| (v[i] - up[i]).abs().total_cmp(&(v[j] - up[j]).abs())).unwrap_or(0);
                            rep.trace.push(format!("step {k} {nm} {d:.3e} cells differing {nd}/{} worst cell {wi}: {:e} up {:e}", v.len(), v[wi], up[wi]));
                        }
                        note(&mut rep, k, nm, d);
                    }
                }
            }
        }
    }
    rep
}

/// Round 81: the `lumpedNuclearStructure` fields of 1D_thermalMSR (`T.fuelAvForNeutronics`, `Tmax`, `Tsurface`); none for 1D_MSR.
fn structure_fields(s: &OnePhaseCcSolver) -> Vec<(String, Vec<f64>)> {
    let mut out = Vec::new();
    for m in s.radial_power_models() {
        if let RadialPowerModel::LumpedNuclearStructure(l) = m {
            let n = s.temperature().len();
            let mut tmax = vec![0.0; n];
            for &c in l.cells() {
                tmax[c] = l.tmax(c);
            }
            out.push(("fluidRegion/T.fuelAvForNeutronics".to_string(), s.fuel_clad_temperatures().0));
            out.push(("fluidRegion/Tmax.lumpedNuclearStructure".to_string(), tmax));
            out.push(("fluidRegion/Tsurface.lumpedNuclearStructure".to_string(), l.tsurface().to_vec()));
        }
    }
    out
}

fn print(name: &str, r: &Report) {
    println!("{name}: {} steps; power {:.3e} reactivity {:.3e} liquidFuelBeta {:.3e} precEq {:.3e} dt {:.3e}; neutron-iteration count differs at {} steps; precPKStar iterations worst {}; p_rgh iterations worst {}", r.steps, r.power, r.react, r.lfbeta, r.peq, r.dt, r.nres_count_diff, r.prec_its_diff, r.prgh_its_diff);
    println!("{name}: fields {:?}", r.fields);
    println!("{name}: first not bit-identical {:?}; first over 1e-9 {:?}", r.first_nonzero, r.first_over);
    for t in &r.trace {
        println!("{name}: {t}");
    }
}

/// The first 60 steps, every step written (the `first` protocol of `run_msr1d_transient_r80.sh`).
///
/// Results (2026-11-28, port and reference on this machine; the run predates its written predictions, README round 80, so these are
/// post-hoc and gated here at the bit-identity / 1e-9 rule, not at a prediction):
/// - steps 1 and 2 are BIT-IDENTICAL in every compared quantity (power, reactivity, `liquidFuelBeta`, `precEquilibriumReactivity`, every
///   field of both regions, every `PBiCG`/`p_rgh` iteration count and `neutroResidual` count, `deltaT`);
/// - the first difference is at step 3, in the fluid (`p` 2.6e-13 in all 640 cells, `U` 2.1e-15, `T` 2.0e-16 in 6 cells), while the
///   neutronics stays bit-identical at step 3; its cause is NOT located (gh:#1960);
/// - over 60 steps: power 4.2e-13, reactivity 2.9e-13, `liquidFuelBeta` 2.4e-13, `precEq` bit-identical, `deltaT` 3.5e-14, fields
///   <= 2.7e-12 (fluid `p`), every iteration count equal; nothing over 1e-9.
///
/// Two upstream behaviours were found by this test and are now the port's default (earlier forms kept callable): (1) `precPKStar_`
/// starts from `defaultPrec_` (`uniform 1` here), not 0 (`LiquidFuelPk::with_default_prec`; control below); (2) not a port defect, a
/// protocol one: the `first` reference is run with `writeControl timeStep`, under which `setDeltaT` does not align to a write time; the
/// first port run staged `adjustableRunTime` and differed in `deltaT` by 4.6e-4 from step 2 (struck through in the README).
#[test]
fn coupled_transient_first_60_steps_against_the_real_binary() {
    let r = run("first60", "tr_first", 60, true, false);
    print("tr_first", &r);
    assert_eq!(r.steps, 60);
    let first = r.first_nonzero.as_ref().map_or(usize::MAX, |f| f.0);
    assert!(first >= 3, "steps 1-2 are bit-identical: first difference {:?}", r.first_nonzero);
    assert!(r.first_over.is_none(), "nothing over 1e-9 in 60 steps: {:?}", r.first_over);
    assert!(r.power <= 1e-9 && r.react <= 1e-9 && r.lfbeta <= 1e-9 && r.peq <= 1e-9, "{r:?}");
    assert!(r.dt <= 1e-12, "deltaT {:e}", r.dt);
    assert_eq!((r.nres_count_diff, r.prec_its_diff, r.prgh_its_diff), (0, 0, 0), "iteration counts");
}

/// Control: the precursors started from 0 (the port's form before round 80) break the bit-identity AT STEP 1, in the equilibrium solve
/// (measured 2026-11-28: `precPKStar7` 2.9e-10 of its max-norm, `precPKStar0` 1.6e-12, every cell; the iteration counts stay equal).
#[test]
fn control_zero_initial_precursors_differ_at_step_one() {
    let r = run("zeroprec", "tr_first", 1, true, true);
    let (step, what, d) = r.first_nonzero.clone().expect("the control differs");
    println!("control zero precursors: first difference step {step} {what} {d:.3e}");
    assert_eq!(step, 1);
    assert!(what.contains("precPKStar"), "{what}");
    let worst = r.fields.iter().filter(|(n, _)| n.contains("precPKStar")).fold(0.0_f64, |m, (_, d)| m.max(*d));
    assert!(worst > 1e-11, "precPKStar worst {worst:e}");
}

#[derive(Debug, Default)]
struct FullReport {
    steps: usize,
    up_steps: usize,
    final_time: f64,
    final_power: f64,
    up_final_power: f64,
    /// `(write time, power rel, worst field and its rel-to-max-norm, T, U)` at every written time.
    writes: Vec<(String, f64, String, f64, f64, f64)>,
}

/// The whole shipped `transient` stage (`adjustableRunTime`, written every 10 s) against `tr_full`: the step count, and at every written
/// time the power and the fields.
fn run_full() -> FullReport {
    run_full_case(&MSR, None)
}

/// [`run_full`] of any [`Case`]; `start`: a case directory staged elsewhere (the chain of gh:#1961) instead of [`stage_case`].
fn run_full_case(c: &Case, start: Option<PathBuf>) -> FullReport {
    let lg = read_log(&format!("{}tr_full_log.ref.txt.gz", c.refs));
    let rf = read_fields(&format!("{}tr_full.ref.txt.gz", c.refs));
    let up_step_of: HashMap<String, usize> = lg.step_time.iter().map(|(k, t)| (t.clone(), *k)).collect();
    let root = start.unwrap_or_else(|| stage_case(c, "full", false));
    let mut run = RegionSolvers::read(&root, CouplingSwitches::default()).unwrap_or_else(|e| panic!("the staged transient is not readable: {e}"));
    let mut rep = FullReport { up_steps: lg.step_time.len(), up_final_power: lg.power.get(&lg.step_time.len()).copied().unwrap_or(f64::NAN), ..FullReport::default() };
    for k in 1..=30_000 {
        let rec = run.step().unwrap_or_else(|e| panic!("step {k} (t = {}): {e}", run.time()));
        rep.steps = k;
        rep.final_time = rec.time;
        let mut power = 0.0;
        let mut fields: Vec<(String, Vec<f64>)> = Vec::new();
        for r in &run.regions {
            match &r.solver {
                RegionSolver::PointKinetics(p) => {
                    power = p.pk.power();
                    if let Some(lf) = &p.liquid {
                        for g in 0..8 {
                            fields.push((format!("neutroRegion/precPKStar{g}"), lf.prec_pk_star(g).to_vec()));
                        }
                    }
                    fields.push(("neutroRegion/powerDensity".to_string(), p.pk.power_density.clone()));
                }
                RegionSolver::OnePhase(f) => {
                    let s = &f.solver;
                    fields.push(("fluidRegion/U".to_string(), s.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect()));
                    fields.push(("fluidRegion/p".to_string(), s.pressure().to_vec()));
                    fields.push(("fluidRegion/T".to_string(), s.temperature().to_vec()));
                    if let Some(kk) = s.turbulent_kinetic_energy() {
                        fields.push(("fluidRegion/k".to_string(), kk.to_vec()));
                    }
                    fields.extend(structure_fields(s));
                }
                _ => {}
            }
        }
        rep.final_power = power;
        let w = (rec.time / 10.0).round() * 10.0;
        if (rec.time - w).abs() < 1e-9 * w {
            let name = format!("{w}");
            let up_power = up_step_of.get(&name).and_then(|s| lg.power.get(s)).copied().unwrap_or(f64::NAN);
            let (mut worst, mut wd, mut t_d, mut u_d) = (String::new(), 0.0_f64, f64::NAN, f64::NAN);
            for (nm, v) in &fields {
                if let Some(up) = rf.get(&format!("{nm} {name}")) {
                    if up.len() == v.len() {
                        let d = rel_max(v, up);
                        if d > wd {
                            (worst, wd) = (nm.clone(), d);
                        }
                        if nm == "fluidRegion/T" {
                            t_d = d;
                        }
                        if nm == "fluidRegion/U" {
                            u_d = d;
                        }
                    }
                }
            }
            rep.writes.push((name, rel(power, up_power), worst, wd, t_d, u_d));
        }
        if rec.time >= 400.0 - 1e-9 {
            break;
        }
    }
    rep
}

/// The 400 s stage (README round 80, criteria T1-T4, predictions P1-P4 committed in 375dc1d5e before this test first ran).
///
/// Results (2026-11-28, about 31 s): **T1** the whole stage runs; **T2** 13 021 steps, the same count as the binary; **T3** at the 30
/// written times `totalPower` within 4.0e-5 (worst t = 360) and the fluid `T` within 1.6e-6, `U` within 5.0e-7 of their max-norms
/// (worst field anywhere: `precPKStar2` 4.5e-5 at t = 360); **T4** final power 6.13788e8 W against 6.13807e8 W, 3.0e-5. All four predictions
/// (P1 60 %, P2 50 %, P3 45 %, P4 55 %) held. The differences stay <= 1.6e-9 to t = 230 and grow to ~1e-5 after t = 240, the stage of the
/// loss of flow, where solves stopped at the shipped tolerances (1e-6 absolute; one PIMPLE outer and one pressure corrector) carry the
/// step-3 round-off forward (not located further); a propagated difference, inside its 1e-4 allowance.
#[test]
fn coupled_transient_full_400_s_against_the_real_binary() {
    let r = run_full();
    println!("tr_full: {} steps (upstream {}), final time {}, final power {:e} up {:e} (rel {:.3e})", r.steps, r.up_steps, r.final_time, r.final_power, r.up_final_power, rel(r.final_power, r.up_final_power));
    for w in &r.writes {
        println!("tr_full: t = {} power {:.3e} worst {} {:.3e} T {:.3e} U {:.3e}", w.0, w.1, w.2, w.3, w.4, w.5);
    }
    // T1, T2
    assert!((r.final_time - 400.0).abs() < 1e-9, "final time {}", r.final_time);
    assert!((r.steps as f64 - r.up_steps as f64).abs() <= 0.01 * r.up_steps as f64, "steps {} against {}", r.steps, r.up_steps);
    // T3
    assert_eq!(r.writes.len(), 30, "written times");
    for w in &r.writes {
        assert!(w.1 <= 1e-3 && w.4 <= 1e-3 && w.5 <= 1e-3, "t = {}: power {:e} T {:e} U {:e}", w.0, w.1, w.4, w.5);
    }
    // T4
    assert!(rel(r.final_power, r.up_final_power) <= 1e-3);
}

/// Step 2 of the issue: the fields `mapAllFields` writes on the neutronics mesh at step 1 (`U`, `alpha`, `mu`, `alphat` = `thermo:alpha`,
/// `rhoCool` = `thermo:rho`, `TFuel`, `TCool`) against the real `-initializeMappedFields` mapping of the same fluid start (the
/// `100/neutroRegion` files of the start directory, written at 17 digits). Result (2026-11-28): all seven bit-identical, 640 of 640 cells.
#[test]
fn step_one_mapped_neutronics_inputs_against_the_real_mapping() {
    let root = stage("mapped", true);
    let mut run = RegionSolvers::read(&root, CouplingSwitches::default()).expect("read");
    run.step().expect("step 1");
    let r = run.regions.iter().find(|r| r.name == "neutroRegion").expect("neutroRegion");
    let n = r.fields.scalar("TFuel").expect("TFuel").len();
    let up_u = outram_foam_appbuilder_lib::io::field_reader::read_vol_vector_field(&root.join("100/neutroRegion/U"), n).expect("U");
    let u = r.fields.vector("U").expect("U");
    let du = u.iter().zip(&up_u).filter(|(a, b)| a.x != b.x || a.y != b.y || a.z != b.z).count();
    let wu = u.iter().zip(&up_u).fold(0.0_f64, |m, (a, b)| m.max((a.z - b.z).abs()));
    println!("mapped U: {du}/{n} cells differ, worst |dUz| {wu:e}");
    assert_eq!(du, 0, "mapped U is bit-identical to the real mapping");
    for nm in ["alpha", "mu", "alphat", "rhoCool", "TFuel", "TCool"] {
        let up = outram_foam_appbuilder_lib::io::field_reader::read_vol_scalar_field(&root.join(format!("100/neutroRegion/{nm}")), n).expect(nm);
        match r.fields.scalar(nm) {
            Some(v) => {
                let d = v.iter().zip(&up).filter(|(a, b)| a != b).count();
                let w = v.iter().zip(&up).fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
                println!("mapped {nm}: {d}/{n} cells differ, worst {w:e}");
                assert_eq!(d, 0, "mapped {nm} is bit-identical to the real mapping");
            }
            None => panic!("mapped {nm}: absent"),
        }
    }
}

/// Round 81 (gh:#1962), predictions P5t: the shipped `1D_thermalMSR_pointKinetics` `transient`, the first 60 steps (every step written).
#[test]
fn thermal_coupled_transient_first_60_steps_against_the_real_binary() {
    let r = run_case(&THERMAL, "first60", "tr_first", 60, true, false);
    print("th_tr_first", &r);
    // Results (2026-11-28): every compared quantity of both regions bit-identical through step 38 (power, reactivity, `liquidFuelBeta`,
    // `precEq`, `deltaT`, `precPKStar0..7`, `powerDensity`, `TFuel`, `TCool`, fluid `U`, `p`, `T`, the structure's `Tmax`,
    // `T.fuelAvForNeutronics`), every iteration count equal; the only non-identical quantity over 60 steps is `Tsurface` (from step 39,
    // 3.4e-16); P5t HELD (both parts)
    assert_eq!(r.steps, 60);
    assert!(r.first_over.is_none(), "{:?}", r.first_over);
    assert!(r.fields.iter().all(|(n, d)| *d == 0.0 || n.contains("Tsurface")), "{:?}", r.fields);
    assert_eq!((r.nres_count_diff, r.prec_its_diff, r.prgh_its_diff), (0, 0, 0));
    assert_eq!(r.dt, 0.0);
}

/// Round 81 (gh:#1962), predictions P1t-P4t: the whole shipped thermal `transient` (400 s, written every 10 s).
#[test]
fn thermal_coupled_transient_full_400_s_against_the_real_binary() {
    let r = run_full_case(&THERMAL, None);
    println!("th_tr_full: {} steps (upstream {}), final time {}, final power {:e} up {:e} (rel {:.3e})", r.steps, r.up_steps, r.final_time, r.final_power, r.up_final_power, rel(r.final_power, r.up_final_power));
    for w in &r.writes {
        println!("th_tr_full: t = {} power {:.3e} worst {} {:.3e} T {:.3e} U {:.3e}", w.0, w.1, w.2, w.3, w.4, w.5);
    }
    // Results (2026-11-28, 14 s): T1 runs; T2 7712 steps, the binary's count; T3 at the 30 written times power <= 4.0e-10, fluid `T`
    // <= 9.5e-12, `U` <= 2.3e-10, any field <= 1.5e-9 (`precPKStar0`, t = 180); T4 final power 4.15250720e8 W, 1.7e-10. P1t-P4t HELD,
    // five orders of magnitude inside T3 (1D_MSR, round 80: 4.0e-5; see gh:#1960). Pinned at 1e-8 for any field (measured 1.5e-9).
    assert!((r.final_time - 400.0).abs() < 1e-9);
    assert_eq!(r.steps, r.up_steps);
    assert_eq!(r.writes.len(), 30);
    for w in &r.writes {
        assert!(w.1 <= 1e-3 && w.4 <= 1e-3 && w.5 <= 1e-3, "T3 t = {}", w.0);
        assert!(w.3 <= 1e-8, "pin: t = {} {} {:e}", w.0, w.2, w.3);
    }
    assert!(rel(r.final_power, r.up_final_power) <= 1e-3);
}
