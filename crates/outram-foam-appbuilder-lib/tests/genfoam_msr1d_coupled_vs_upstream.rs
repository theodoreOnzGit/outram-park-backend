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
use outram_foam_appbuilder_lib::io::foam_ascii::WriteOptions;
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
/// - ~~the first difference is at step 3, in the fluid (`p` 2.6e-13 in all 640 cells, `U` 2.1e-15, `T` 2.0e-16 in 6 cells), while the
///   neutronics stays bit-identical at step 3; its cause is NOT located (gh:#1960)~~ **CORRECTED 2026-11-28 (round 81,
///   [`gh1960_step_three_fluid_difference_diagnostic`]):** the first difference of the compared quantities is at step 3 (`p` 2.6e-13 in all
///   640 cells, `U` 2.1e-15, `T` 2.0e-16 in 6 cells), but the first difference of the run is at STEP 1, in `epsilon`, `nut`, `alphat`
///   (not compared here), 2 cells by 1 ulp: PETIR's `pow` in the wall function (gh:#1963); located;
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
/// step-3 round-off forward ~~(not located further)~~ (**CORRECTED 2026-11-28, round 81:** located, the step-1 `epsilon` `pow` of the wall
/// function, gh:#1960/#1963); a propagated difference, inside its 1e-4 allowance.
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

/// gh:#1960 (README round 81): which fluid quantity of the 1D_MSR transient differs FIRST, field by field and solve by solve, over steps
/// 1-3 (`U`, `p`, `p_rgh`, `T`, `k`, `epsilon`, `nut`, `alphat`; the `p_rgh` solves' initial residuals and iterations).
///
/// Result (2026-11-28): **located.** The first non-identical number is at STEP 1, not 3: `epsilon` in 2 of 640 cells (368, 536) by 1
/// ulp, and with it `nut` and `alphat` in the same 2 cells; `U`, `p`, `p_rgh`, `T`, `k` and the `p_rgh` solve are bit-identical. Both
/// cells are the wall function `Cmu^0.75 k^1.5/(kappa y)` recomputed from the start `k`: the upstream value is the platform `pow`'s, the
/// port's is PETIR's `r_powf` (2 of 2 cells, bit for bit), the cause round 80 found in the standalone fluid stage (gh:#1963). Step 2:
/// every one of the eight fields bit-identical again (`epsilon` is re-set from the equal `k`); step 3: the `p_rgh` solve's INITIAL residual
/// already differs (0.03520831767608689 against 0.035208317676087335), then `p` in all 640 cells (2.6e-13). Round 80's comparison did not
/// include `epsilon`, `nut`, `alphat`, so its "steps 1 and 2 bit-identical in every compared quantity" was true of what it compared and hid
/// the step-1 difference. The transient-only candidates of gh:#1960 (the pump table, the variable `deltaT`) are excluded also by the
/// laminar 1D_thermalMSR transient, which has both and stays bit-identical in `p` through 60 steps.
#[test]
fn gh1960_step_three_fluid_difference_diagnostic() {
    let lg = read_log("msr1d_r80_tr_first_log.ref.txt.gz");
    let rf = read_fields("msr1d_r80_tr_first.ref.txt.gz");
    let root = stage("gh1960", true);
    let mut run = RegionSolvers::read(&root, CouplingSwitches::default()).expect("read");
    let k_start = outram_foam_appbuilder_lib::io::field_reader::read_vol_scalar_field(&root.join("100/fluidRegion/k"), 640).expect("k");
    let mut pow_tally = (0usize, 0usize, 0usize);
    for k in 1..=3 {
        run.step().expect("step");
        let time = &lg.step_time[&k];
        if k == 1 {
            // the step-1 `epsilon` cells that differ: the wall function `Cmu^0.75 k^1.5/(kappa y)` (two wall faces, `y = 0.5`) recomputed
            // from the START `k` with the platform `pow` and with PETIR's `r_powf` (round 80's test, here on the restart)
            use petir::mathf::RealMath;
            let up = &rf[&format!("fluidRegion/epsilon {time}")];
            for r in &run.regions {
                if let RegionSolver::OnePhase(f) = &r.solver {
                    let mine = f.solver.turbulent_dissipation().expect("epsilon");
                    for c in 0..640 {
                        if mine[c] != up[c] {
                            let (cmu, kappa, y) = (0.09_f64, 0.41_f64, 0.5_f64);
                            let e_std = cmu.powf(0.75) * k_start[c].powf(1.5) / (kappa * y);
                            let e_petir = cmu.r_powf(0.75) * k_start[c].r_powf(1.5) / (kappa * y);
                            let (w_std, w_petir) = (0.5 * e_std + 0.5 * e_std, 0.5 * e_petir + 0.5 * e_petir);
                            pow_tally.0 += 1;
                            pow_tally.1 += usize::from(w_std == up[c]);
                            pow_tally.2 += usize::from(w_petir == mine[c]);
                            println!("gh1960 step 1 epsilon cell {c}: port {:e} up {:e}; platform pow {w_std:e}, PETIR pow {w_petir:e}", mine[c], up[c]);
                        }
                    }
                }
            }
            println!("gh1960 step 1: {} epsilon cells differ; upstream = platform pow in {}, port = PETIR pow in {}", pow_tally.0, pow_tally.1, pow_tally.2);
            assert_eq!(pow_tally, (2, 2, 2), "every step-1 epsilon difference is the pow of the wall function");
        }
        for r in &run.regions {
            if let RegionSolver::OnePhase(f) = &r.solver {
                let s = &f.solver;
                let fields: Vec<(&str, Vec<f64>)> = vec![
                    ("U", s.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect()),
                    ("p", s.pressure().to_vec()),
                    ("p_rgh", s.p_rgh().expect("flow").to_vec()),
                    ("T", s.temperature().to_vec()),
                    ("k", s.turbulent_kinetic_energy().expect("k").to_vec()),
                    ("epsilon", s.turbulent_dissipation().expect("epsilon").to_vec()),
                    ("nut", s.turbulent_viscosity().expect("nut").to_vec()),
                    ("alphat", s.turbulent_thermal_diffusivity().expect("alphat").to_vec()),
                ];
                for (nm, v) in &fields {
                    let up = &rf[&format!("fluidRegion/{nm} {time}")];
                    let nd = v.iter().zip(up).filter(|(a, b)| a != b).count();
                    let first: Vec<(usize, f64, f64)> = (0..v.len()).filter(|&i| v[i] != up[i]).take(3).map(|i| (i, v[i], up[i])).collect();
                    println!("gh1960 step {k} {nm}: {:.3e} ({nd}/{} differ) {first:?}", rel_max(v, up), v.len());
                    match (k, *nm) {
                        (1, "epsilon" | "nut" | "alphat") => assert_eq!(nd, 2, "step 1 {nm}"),
                        (1 | 2, _) => assert_eq!(nd, 0, "step {k} {nm}"),
                        (3, "p") => assert_eq!(nd, 640),
                        _ => {}
                    }
                }
                let ps: Vec<(f64, usize)> = s.pressure_solves().iter().map(|x| (x.initial_residual, x.iterations)).collect();
                println!("gh1960 step {k} p_rgh solves port {ps:?} up {:?}", lg.prgh.get(&k));
                println!("gh1960 step {k} h solves port {:?}", s.h_solves());
                if let Some(ts) = s.turbulence_solves() {
                    println!("gh1960 step {k} turbulence solves port {ts:?}");
                }
            }
        }
    }
}

/// gh:#1961 (README round 81): Allrun's `steadyState` of 1D_MSR in the port through [`RegionSolvers`] (fluid only), from `cases/msr1d_root`
/// with Allrun's edits (`endTime 100`, `writeInterval 100`, `multiRegionCouplingDict_TH`; `adjustTimeStep true` and
/// `writeControl adjustableRunTime` are the rootCase's), writing every write time as GeN-Foam does (`writePrecision 6`, the rootCase's).
fn chain_steady_state(tag: &str) -> (PathBuf, usize, f64) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1961_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases/msr1d_root"), &root);
    std::fs::copy(root.join("constant/multiRegionCouplingDict_TH"), root.join("constant/multiRegionCouplingDict")).expect("coupling dict");
    let cd = root.join("system/controlDict");
    let t = std::fs::read_to_string(&cd).expect("controlDict");
    assert!(t.contains("endTime         1000;") && t.contains("writeInterval   1000;") && t.contains("writePrecision  6;"));
    let t = t.replacen("endTime         1000;", "endTime         100;", 1).replacen("writeInterval   1000;", "writeInterval   100;", 1);
    std::fs::write(&cd, t).expect("write controlDict");
    let mut run = RegionSolvers::read(&root, CouplingSwitches::default()).unwrap_or_else(|e| panic!("steadyState: {e}"));
    let mut steps = 0;
    while run.running() {
        run.step().unwrap_or_else(|e| panic!("steadyState step {}: {e}", steps + 1));
        steps += 1;
        if run.is_write_time() {
            let st = run.write_time_directory(&root, &WriteOptions { precision: 6 }).unwrap_or_else(|e| panic!("write at {}: {e}", run.time()));
            println!("gh1961 steadyState: wrote {} files at t = {}; not written {:?}", st.files.len(), run.time(), st.not_written);
        }
    }
    (root, steps, run.time())
}

/// `FILE\t<rel>\t<bytes>` records of a packed directory, as `(rel, text)`.
fn unpack_texts(packed: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < packed.len() {
        let nl = at + packed[at..].iter().position(|&b| b == b'\n').expect("header");
        let parts: Vec<String> = std::str::from_utf8(&packed[at..nl]).expect("utf8").split('\t').map(str::to_string).collect();
        let len: usize = parts[2].parse().expect("len");
        out.push((parts[1].clone(), String::from_utf8(packed[nl + 1..nl + 1 + len].to_vec()).expect("utf8")));
        at = nl + 1 + len + 1;
    }
    out
}

/// The number tokens of a file body after its `FoamFile` header, and the text with every number replaced by `#`.
fn numbers_and_skeleton(text: &str) -> (Vec<f64>, String) {
    let body = match text.find("// * * *") {
        Some(i) => &text[i..],
        None => text,
    };
    let (mut nums, mut skel, mut tok) = (Vec::new(), String::new(), String::new());
    let flush = |tok: &mut String, nums: &mut Vec<f64>, skel: &mut String| {
        if !tok.is_empty() {
            match tok.parse::<f64>() {
                Ok(v) => {
                    nums.push(v);
                    skel.push('#');
                }
                Err(_) => skel.push_str(tok),
            }
            tok.clear();
        }
    };
    for ch in body.chars() {
        if ch.is_whitespace() || "(){};[]".contains(ch) {
            flush(&mut tok, &mut nums, &mut skel);
            if !ch.is_whitespace() {
                skel.push(ch);
            } else if !skel.ends_with(' ') {
                skel.push(' ');
            }
        } else {
            tok.push(ch);
        }
    }
    flush(&mut tok, &mut nums, &mut skel);
    (nums, skel)
}

#[derive(Debug, Default)]
struct ChainReport {
    ss_steps: usize,
    ss_time: f64,
    /// files of the real raw `100` the port did not write
    missing: Vec<String>,
    /// files the port wrote that the real raw `100` does not hold
    extra: Vec<String>,
    byte_identical: usize,
    skeleton_differs: Vec<String>,
    /// `(file, numbers differing, worst |port - up| / max|up|)`
    w2: Vec<(String, usize, f64)>,
    /// `(mapped file, worst relative-to-max-norm difference)` against the real `-initializeMappedFields` (round 80's `tr_start100`)
    w4: Vec<(String, f64)>,
    mapped_written: Vec<String>,
}

/// gh:#1961: the whole Allrun chain of 1D_MSR in the port, up to the transient's start (steadyState, its written `100`, `setTransient`'s
/// edits, `-initializeMappedFields`), with W1, W2, W4 measured on the way. Returns the case root, ready for `GeN-Foam` of the transient.
fn chain_to_transient(tag: &str) -> (PathBuf, ChainReport) {
    let (root, ss_steps, ss_time) = chain_steady_state(tag);
    let mut rep = ChainReport { ss_steps, ss_time, ..ChainReport::default() };
    // W1/W2: the port-written `100` against the real Allrun `100` (raw, BEFORE the transient's `-initializeMappedFields`)
    let real = unpack_texts(&gunzip(&vv_dir().join("refs/msr1d_r81_ss_written100.txt.gz")));
    for (rel, up) in &real {
        let path = root.join("100").join(rel);
        let Ok(mine) = std::fs::read_to_string(&path) else {
            rep.missing.push(rel.clone());
            continue;
        };
        if &mine == up {
            rep.byte_identical += 1;
        }
        let ((a, sa), (b, sb)) = (numbers_and_skeleton(&mine), numbers_and_skeleton(up));
        if sa != sb || a.len() != b.len() {
            rep.skeleton_differs.push(rel.clone());
            continue;
        }
        let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let nd = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        let worst = a.iter().zip(&b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / if scale > 0.0 { scale } else { 1.0 };
        rep.w2.push((rel.clone(), nd, worst));
    }
    let mut stack = vec![root.join("100")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("read_dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(root.join("100")).expect("prefix").to_string_lossy().to_string();
                if !real.iter().any(|(r, _)| *r == rel) {
                    rep.extra.push(rel);
                }
            }
        }
    }
    // setTransient (Allrun): `cp -r 0/neutroRegion 100/`, `cp -r 0/uniform 100/`, the controlDict edits, the coupled dictionary; plus the
    // reference protocol's `writePrecision 17` (`run_msr1d_transient_r80.sh`, set before its `-initializeMappedFields`)
    copy_dir(&root.join("0/neutroRegion"), &root.join("100/neutroRegion"));
    copy_dir(&root.join("0/uniform"), &root.join("100/uniform"));
    std::fs::copy(root.join("constant/multiRegionCouplingDict_Coupled"), root.join("constant/multiRegionCouplingDict")).expect("coupling dict");
    let cd = root.join("system/controlDict");
    let t = std::fs::read_to_string(&cd).expect("controlDict");
    let t = t
        .replacen("endTime         100;", "endTime         400;", 1)
        .replacen("writeInterval   100;", "writeInterval   10;", 1)
        .replacen("writePrecision  6;", "writePrecision  17;", 1)
        .replacen("    fluidRegion     onePhase;\n", "    fluidRegion     onePhase;\n    neutroRegion    pointKinetics;\n", 1)
        .replacen("    fluidRegion     false;\n", "    fluidRegion     false;\n    neutroRegion    false;\n", 1);
    assert!(t.contains("endTime         400;") && t.contains("writeInterval   10;") && t.contains("writePrecision  17;") && t.contains("neutroRegion    pointKinetics;"));
    std::fs::write(&cd, t).expect("controlDict");
    // `GeN-Foam -initializeMappedFields`
    let mut init = RegionSolvers::read(&root, CouplingSwitches::default()).unwrap_or_else(|e| panic!("-initializeMappedFields: {e}"));
    rep.mapped_written = init.initialize_mapped_fields(&root, &WriteOptions::precision_17()).expect("mapped fields");
    // W4: each mapped file against the real one (round 80's start directory, after the real -initializeMappedFields)
    let start = unpack_texts(&gunzip(&vv_dir().join("refs/msr1d_r80_tr_start100.txt.gz")));
    for rel in &rep.mapped_written {
        let key = rel.trim_start_matches("100/");
        let Some((_, up)) = start.iter().find(|(r, _)| r == key) else {
            rep.w4.push((rel.clone(), f64::NAN));
            continue;
        };
        let mine = std::fs::read_to_string(root.join(rel)).expect("mapped file");
        let ((a, _), (b, _)) = (numbers_and_skeleton(&mine), numbers_and_skeleton(up));
        let d = if a.len() == b.len() {
            let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            a.iter().zip(&b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / if scale > 0.0 { scale } else { 1.0 }
        } else {
            f64::INFINITY
        };
        rep.w4.push((rel.clone(), d));
    }
    (root, rep)
}

/// gh:#1961 (README round 81), predictions W1-W4: the Allrun chain of `1D_MSR_pointKinetics` in the port, and the chained transient
/// against the real binary (`refs/msr1d_r80_tr_full*`, criteria T1-T4).
#[test]
fn gh1961_allrun_chain_against_the_real_binary() {
    let (root, rep) = chain_to_transient("chain");
    println!("gh1961 chain: steadyState {} steps to t = {}", rep.ss_steps, rep.ss_time);
    println!("gh1961 W1: missing {:?}; extra {:?}", rep.missing, rep.extra);
    println!("gh1961 W2: {} byte-identical files; skeleton differs {:?}", rep.byte_identical, rep.skeleton_differs);
    for w in &rep.w2 {
        println!("gh1961 W2: {} {} numbers differ, worst {:.3e}", w.0, w.1, w.2);
    }
    for w in &rep.w4 {
        println!("gh1961 W4: {} {:.3e}", w.0, w.1);
    }
    let r = run_full_case(&MSR, Some(root));
    println!("gh1961 W3: {} steps (upstream {}), final time {}, final power {:e} up {:e} (rel {:.3e})", r.steps, r.up_steps, r.final_time, r.final_power, r.up_final_power, rel(r.final_power, r.up_final_power));
    for w in &r.writes {
        println!("gh1961 W3: t = {} power {:.3e} worst {} {:.3e} T {:.3e} U {:.3e}", w.0, w.1, w.2, w.3, w.4, w.5);
    }
    // the same transient started from the REAL `100` (round 80's `coupled_transient_full_400_s_against_the_real_binary`)
    let r80 = run_full();
    let same = r.writes.iter().zip(&r80.writes).filter(|(a, b)| a.1 == b.1 && a.3 == b.3).count();
    println!("gh1961 W3: written times whose power and worst field difference equal round 80's run from the real 100: {same} of {}", r.writes.len());
    // Results (2026-11-28, ~70 s; the gates below are the committed criteria and the measured state):
    // * steadyState: 8340 steps to t = 100 (the binary's count);
    // * W1 MISSED: the port's `100` holds 23 of the real 24 fluid files and both `uniform` files; `alphaRhoMagU` is refused by name
    //   (faceCentered updates it every corrector, not tracked); the transient reads it `READ_IF_PRESENT` and computes it instead;
    // * W2 MISSED: 20 of 26 files byte-identical; `dgdt` (1866 numbers at 1e-16 absolute, 5.6e-18 of max) and `p_rgh` (6 wall-patch
    //   `gradient` numbers at 1e-22 absolute) differ in round-off of near-zero values; `alpha.passiveStructure` (the port writes the structure
    //   fraction where the real file is `uniform 0`) and `heatFlux.structure` (the port writes 0 in the `hx` cells of the fixedTemperature
    //   structure, the real file -4.5e6) are WRONG content, and so is `Re` in the 320 cells without a structure (`HotLeg`, `ColdLeg`: the
    //   port writes the fluid's `|U| Dh / nu` = 3.4e5, the real file `minRe` 10, `FSPair.C:344-345` with `Dh = 0` there); none of the three
    //   changes the transient (filed);
    // * the chained transient equals, at all 30 written times, round 80's run from the REAL `100` (power and worst field difference);
    // * W3 HELD: 13 021 steps, power within 4.0e-5 (t = 360), final power 3.0e-5, fluid `T` 1.6e-6, `U` 5.0e-7;
    // * W4 HELD: the 11 mapped files (`TFuel`, `TCool`, `TClad`, `TStructMech`, `rhoCool`, `U`, `alpha`, `mu`, `alphat` of the neutronics,
    //   `powerDensityNeutronics`, `powerDensityNeutronicsToLiquid` of the fluid) number for number equal to the real ones.
    // Found by this chain (fixed, default now): `RegionSolvers::read` set both fluid coupling fields to 0, overriding the case's
    // `initialPowerDensity` in a fluid-only stage (`T` 21 % off at t = 100 before the fix).
    // ~~W1 MISSED / W2 MISSED (above)~~ **CORRECTED 2026-11-28 (gh:#1964, README round 82, re-run):** `Dh = SMALL` outside the structures
    // (`Re` = `minRe` there), `alphapas_` as upstream builds it, `heatFlux.structure` on every structure cell and `correctAlphaRhoMagU()`
    // under faceCentered: all 24 fluid files written; 24 of the 26 files byte-identical (22 of 24 fluid + both `uniform`); the two that are
    // not are `dgdt` (1866 numbers, 5.6e-18 of max) and `p_rgh` (6 `gradient` numbers, 8.1e-28 of max): round-off of near-zero values of an
    // 8340-step state that is not bit-identical (gh:#1963, PETIR `pow`); the chained transient unchanged (30 of 30 equal to round 80's).
    assert_eq!(rep.ss_steps, 8340);
    assert!(rep.missing.is_empty(), "W1: {:?}", rep.missing);
    assert!(rep.extra.is_empty());
    assert_eq!(rep.byte_identical, 24, "{}", rep.byte_identical);
    assert!(rep.w2.iter().all(|w| w.2 <= 5e-6), "W2: {:?}", rep.w2);
    let not_identical: Vec<&str> = rep.w2.iter().filter(|w| w.1 > 0).map(|w| w.0.as_str()).collect();
    assert_eq!(not_identical, vec!["fluidRegion/dgdt", "fluidRegion/p_rgh"], "{:?}", rep.w2);
    assert!(rep.skeleton_differs.is_empty(), "{:?}", rep.skeleton_differs);
    assert_eq!(same, 30, "the chain reproduces round 80's run from the real 100 at every written time");
    assert!(rep.w4.len() == 11 && rep.w4.iter().all(|w| w.1 == 0.0), "W4: {:?}", rep.w4);
    assert!((r.final_time - 400.0).abs() < 1e-9 && r.steps == r.up_steps && r.writes.len() == 30, "T1 T2");
    for w in &r.writes {
        assert!(w.1 <= 1e-3 && w.4 <= 1e-3 && w.5 <= 1e-3, "T3 t = {}", w.0);
    }
    assert!(rel(r.final_power, r.up_final_power) <= 1e-3, "T4");
}
