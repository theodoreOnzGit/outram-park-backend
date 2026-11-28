// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `reactorCases/3D_gFHR` AS SHIPPED against the real GeN-Foam binary (gh:#1876, README round 89)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! The case is the shipped `rootCase` (292 500 hexahedra, one `fluidRegion`, `onePhase` `faceCentered`, FLiBe `icoPolynomial` +
//! `hPolynomial` + `polynomial`, `porousKEpsilon`, `nuclearSteadyStatePebble` with a central core; `stopAt nextWrite`, `writeInterval
//! 50`, `maxCo 1`), staged from the refs submodule (`refs/gfhr_r89/case`, written by `scripts/make_gfhr_r89_refs.py case`), and run as
//! the tutorial's `Allrun` runs it: `GeN-Foam -initializeMappedFields` ([`RegionSolvers::initialize_mapped_fields`]), then `GeN-Foam`
//! ([`RegionSolvers::step`] while [`RegionSolvers::running`]). The real binary (GeN-Foam 652b3da on OpenFOAM-ESI v2506, serial,
//! `scripts/run_gfhr_r89_real.sh`) ran on the same files. Variants (their `controlDict`/`fvSolution`/`phaseProperties` are the real run's
//! own, `refs/gfhr_r89/<tag>_files`): `nssp` (the shipped NSSP case unedited, to `t = 50`), `t3` (tight: every tolerance 1e-14,
//! `relTol 0`, `maxIter 5000`, `stopAt endTime`, `endTime 0.3`, `writeControl timeStep`, `writePrecision 17`), `lps` (the shipped LPS case:
//! `phaseProperties_lumpedParam`).
//!
//! Compared: every `deltaT`, every `T.nuclearSteadyStatePebble.<X> (avg min max)` number and the `T (avg min max)` min and max, number
//! by number against the real log; the `p_rgh` GAMG initial residuals (printed, not gated); every 7th cell of the last written time
//! (`T`, `p_rgh`, `U`, `k`, `epsilon`, `nut`, `alphat`, the pebble `TpS`, `Tfmax`), by the max-norm difference over the reference's max.
//!
//! Gates (README round 89, stated before the first port run): tight `t3` <= 1e-9 relative everywhere (G4); the shipped run: every log
//! number equal at the printed 6 digits (G5) or within 1e-5 relative (G5b, the asserted criterion), the written fields within 1e-5 (G6).
//!
//! ## Results (measured 2026-11-28)
//!
//! Printed by `--nocapture`; recorded in the README, "Round 89 results". `nssp`: 266 = 266 steps to `t = 50`, all 6118 compared log numbers
//! (4788 pebble, 798 `T`, 266 `deltaT`, 266 `Time`) equal at the printed 6 digits, the `t = 50` fields within 4.9e-6 (the reference's 6-digit
//! write); port 352.7 s against the real binary's 464.66 s. `lps`: 265 = 265 steps, 1325 of 1325 equal, fields within 4.7e-6. `t3`: worst
//! 6.6e-16 on the log numbers, 1.24e-14 on `deltaT`, 3.7e-13 on the fields (both codes' GAMG stop at `maxIter 5000` on a 5e-14..3e-13 floor).
//! The first port run gave NaN everywhere: the `porousKEpsilon` constructor's `bound(k)` was missing (fixed, README round 89).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;

use outram_foam_appbuilder_lib::genfoam::multi_region::region_solvers::{CouplingSwitches, RegionSolver, RegionSolvers};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::nuclear_steady_state_pebble::coupled::AvgMinMax;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;
use outram_foam_appbuilder_lib::io::foam_ascii::WriteOptions;

/// The refs of this round.
fn refs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs/gfhr_r89")
}

/// A gzip member's payload (the same reader as `tests/genfoam_htr10_vs_upstream.rs`).
fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => panic!("read {}: {e}", path.display()),
    };
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
    match decompress_to_vec(&bytes[at..bytes.len() - 8]) {
        Ok(v) => v,
        Err(_) => panic!("inflate {}", path.display()),
    }
}

/// Copy `src` to `dst`, inflating every `*.gz`.
fn stage_dir(src: &Path, dst: &Path) {
    if let Err(e) = std::fs::create_dir_all(dst) {
        panic!("mkdir {}: {e}", dst.display());
    }
    let entries = match std::fs::read_dir(src) {
        Ok(r) => r,
        Err(e) => panic!("read_dir {}: {e}", src.display()),
    };
    for e in entries {
        let e = match e {
            Ok(e) => e,
            Err(err) => panic!("entry: {err}"),
        };
        let s = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if s.is_dir() {
            stage_dir(&s, &dst.join(&name));
        } else if let Some(stem) = name.strip_suffix(".gz") {
            if let Err(err) = std::fs::write(dst.join(stem), gunzip(&s)) {
                panic!("write {stem}: {err}");
            }
        } else if let Err(err) = std::fs::copy(&s, dst.join(&name)) {
            panic!("copy {name}: {err}");
        }
    }
}

/// Calls of [`stage`] so far in this process.
static STAGED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A scratch copy of the shipped case at a path unique to the call, with the variant's own files (`<tag>_files`) laid over it.
fn stage(tag: &str) -> PathBuf {
    let call = STAGED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1876_r89_{tag}_{}_{call}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    stage_dir(&refs_dir().join("case"), &root);
    let over = refs_dir().join(format!("{tag}_files"));
    if over.is_dir() {
        stage_dir(&over, &root);
    }
    root
}

/// Every file under `dir` with its bytes (to show a stage changed nothing).
fn snapshot(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => panic!("read_dir {}: {e}", dir.display()),
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            snapshot(&p, out);
        } else {
            match std::fs::read(&p) {
                Ok(b) => out.push((p, b)),
                Err(e) => panic!("read {}: {e}", p.display()),
            }
        }
    }
}

/// `max |a - b| / max |b|` (the absolute difference where `b` is all zero).
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

/// The values of a `<tag>_<field>_s7.txt.gz` reference (a vector: its three components in a row).
fn sample_ref(tag: &str, field: &str) -> Option<Vec<f64>> {
    let p = refs_dir().join(format!("{tag}_{field}_s7.txt.gz"));
    if !p.exists() {
        return None;
    }
    let text = String::from_utf8_lossy(&gunzip(&p)).to_string();
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        match tok.parse::<f64>() {
            Ok(v) => out.push(v),
            Err(_) => panic!("{tag}_{field}: `{tok}`"),
        }
    }
    Some(out)
}

/// The real log of a reference run.
fn real_log(tag: &str) -> String {
    String::from_utf8_lossy(&gunzip(&refs_dir().join(format!("{tag}_log.gz")))).to_string()
}

/// The numbers after `key` on every line of `log` that starts with it.
fn log_numbers(log: &str, key: &str) -> Vec<Vec<f64>> {
    log.lines()
        .filter_map(|l| l.strip_prefix(key))
        .map(|rest| rest.split(|c: char| c.is_whitespace() || c == ',').filter_map(|x| x.parse::<f64>().ok()).collect())
        .collect()
}

/// `x` printed as OpenFOAM's `Info` prints a scalar at the default precision 6 (`%g`), read back.
fn six_digits(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let s = format!("{:.*e}", 5, x);
    match s.parse::<f64>() {
        Ok(v) => v,
        Err(_) => panic!("{s}"),
    }
}

/// The fluid region's solver.
fn fluid(rs: &RegionSolvers) -> &OnePhaseCcSolver {
    match rs.region("fluidRegion").map(|r| &r.solver) {
        Some(RegionSolver::OnePhase(f)) => &f.solver,
        _ => panic!("no onePhase fluidRegion"),
    }
}

/// The six `(avg min max)` lines of the pebble model, if the case has one and it has reported.
fn pebble_lines(s: &OnePhaseCcSolver) -> Option<[[f64; 3]; 6]> {
    for m in s.radial_power_models() {
        if let RadialPowerModel::NuclearSteadyStatePebble(pm) = m {
            return pm.report().map(|r| {
                let q = |m: &AvgMinMax| -> [f64; 3] { [m.avg, m.min, m.max] };
                [q(&r.tps), q(&r.tmout), q(&r.tmav), q(&r.tfs), q(&r.tfav), q(&r.tfmax)]
            });
        }
    }
    None
}

/// Every 7th value of a cell field.
fn every_7th(v: &[f64]) -> Vec<f64> {
    v.iter().step_by(7).copied().collect()
}

/// What one comparison measured.
#[derive(Default)]
struct Outcome {
    steps: usize,
    real_steps: usize,
    /// (name, step, port, printed)
    log: Vec<(String, usize, f64, f64)>,
    fields: Vec<(String, f64)>,
    prgh: Vec<(f64, f64, usize)>,
    wall: f64,
    last_time: f64,
}

impl Outcome {
    /// Worst relative difference of the log numbers to the printed values, and how many are equal at the printed digits.
    fn log_worst(&self) -> (f64, usize) {
        let mut worst = 0.0_f64;
        let mut equal = 0;
        for (_, _, p, r) in &self.log {
            let d = if *r == 0.0 { p.abs() } else { ((p - r) / r).abs() };
            worst = worst.max(d);
            if six_digits(*p) == *r {
                equal += 1;
            }
        }
        (worst, equal)
    }

    fn field_worst(&self) -> f64 {
        self.fields.iter().fold(0.0_f64, |m, r| m.max(r.1))
    }

    fn print(&self, label: &str) {
        let (lw, eq) = self.log_worst();
        println!("== {label}: port {} steps (real {}), last time {:.9}, port wall {:.1} s ({:.2} s per step)", self.steps, self.real_steps, self.last_time, self.wall, self.wall / self.steps.max(1) as f64);
        println!("   log numbers: {} compared, {eq} equal at the printed digits, worst {lw:.3e} relative to the printed text", self.log.len());
        let mut by: HashMap<&str, (usize, usize, f64)> = HashMap::new();
        for (n, _, p, r) in &self.log {
            let key = n.split(' ').next().unwrap_or("");
            let e = by.entry(key).or_insert((0, 0, 0.0));
            e.0 += 1;
            if six_digits(*p) == *r {
                e.1 += 1;
            }
            let d = if *r == 0.0 { p.abs() } else { ((p - r) / r).abs() };
            e.2 = e.2.max(d);
        }
        let mut keys: Vec<_> = by.keys().copied().collect();
        keys.sort();
        for k in keys {
            let (n, e, w) = by[k];
            println!("     {k:10} {n:6} compared, {e:6} equal, worst {w:.3e}");
        }
        let shown: Vec<_> = self.log.iter().filter(|(n, s, _, _)| *s <= 3 || n == "deltaT" && *s % 50 == 0).take(80).collect();
        for (n, s, p, r) in shown {
            println!("     step {s:4} {n:16} port {p:.17e} printed {r}");
        }
        let worst_prgh = self.prgh.iter().fold(0.0_f64, |m, (a, b, _)| m.max(((a - b) / b).abs()));
        println!("   p_rgh initial residuals: {} compared, worst {worst_prgh:.3e} (not gated)", self.prgh.len());
        for (i, (a, b, it)) in self.prgh.iter().enumerate().take(8) {
            println!("     p_rgh solve {:2}: port {a:.6e} real {b:.6e}, port iterations {it}", i + 1);
        }
        for (n, d) in &self.fields {
            println!("   field {n:32} {d:.3e}");
        }
    }
}

/// Run a staged variant as the Allrun does (`-initializeMappedFields` first when `init`), step to the end or `max_steps`, compare.
fn run_variant(tag: &str, init: bool, max_steps: usize) -> Outcome {
    let root = stage(tag);
    if init {
        // `GeN-Foam -initializeMappedFields -case steadyStateNSSP`: a process of its own (the regions are constructed, then dropped)
        let mut before = Vec::new();
        snapshot(&root, &mut before);
        let mut first = match RegionSolvers::read(&root, CouplingSwitches::default()) {
            Ok(r) => r,
            Err(e) => panic!("-initializeMappedFields: the port refuses the case: {e}"),
        };
        let written = match first.initialize_mapped_fields(&root, &WriteOptions::precision_17()) {
            Ok(w) => w,
            Err(e) => panic!("-initializeMappedFields: {e}"),
        };
        let mut after = Vec::new();
        snapshot(&root, &mut after);
        println!("-initializeMappedFields: wrote {written:?}; case files unchanged: {}", before == after);
        assert!(written.is_empty() && before == after, "the shipped gFHR maps nothing (empty mappings), as the real stage (md5 unchanged)");
    }
    let mut rs = match RegionSolvers::read(&root, CouplingSwitches::default()) {
        Ok(r) => r,
        Err(e) => panic!("the port refuses the case: {e}"),
    };
    let log = real_log(tag);
    let real_dts: Vec<f64> = log_numbers(&log, "deltaT = ").into_iter().filter_map(|v| v.first().copied()).collect();
    let real_times: Vec<f64> = log_numbers(&log, "Time = ").into_iter().filter_map(|v| v.first().copied()).collect();
    let real_prgh: Vec<f64> = log
        .lines()
        .filter(|l| l.contains("Solving for p_rgh"))
        .filter_map(|l| l.split("Initial residual = ").nth(1).and_then(|r| r.split(',').next()).and_then(|x| x.trim().parse::<f64>().ok()))
        .collect();
    let names = ["TpS", "Tmout", "Tmav", "TfS", "Tfav", "Tfmax"];
    let real_peb: Vec<Vec<Vec<f64>>> = names.iter().map(|n| log_numbers(&log, &format!("T.nuclearSteadyStatePebble.{n} (avg min max) = "))).collect();
    let real_t: Vec<Vec<f64>> = log_numbers(&log, "T (avg min max) = ");
    let mut o = Outcome { real_steps: real_times.len(), ..Outcome::default() };
    let mut solve_i = 0usize;
    let clock = std::time::Instant::now();
    while rs.running() && o.steps < max_steps {
        if let Err(e) = rs.step() {
            panic!("step {}: {e}", o.steps + 1);
        }
        o.steps += 1;
        let s = fluid(&rs);
        for ps in s.pressure_solves() {
            if let Some(&r) = real_prgh.get(solve_i) {
                o.prgh.push((ps.initial_residual, r, ps.iterations));
            }
            solve_i += 1;
        }
        if let Some(lines) = pebble_lines(s) {
            for (i, l) in lines.iter().enumerate() {
                if let Some(r) = real_peb[i].get(o.steps - 1) {
                    for j in 0..3 {
                        o.log.push((format!("{} {}", names[i], ["avg", "min", "max"][j]), o.steps, l[j], r[j]));
                    }
                }
            }
        }
        if let Some(r) = real_t.get(o.steps - 1) {
            // `EEqn_1p.H:112-116`: `T.weightedAverage(V)` of the cells, `min(T)`/`max(T)` of the cells AND the patch values (~~the cells
            // only~~: CORRECTED 2026-11-28, the first whole run compared the cell min with the printed 823.15 K of the inlet patch)
            let t = s.temperature();
            let v = &s.case().mesh.cell_volumes;
            let avg = t.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>();
            let (mn, mx) = t.iter().chain(s.temperature_boundary().iter()).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
            o.log.push(("T avg".to_string(), o.steps, avg, r[0]));
            o.log.push(("T min".to_string(), o.steps, mn, r[1]));
            o.log.push(("T max".to_string(), o.steps, mx, r[2]));
        }
        if let Some(&r) = real_times.get(o.steps - 1) {
            o.log.push(("Time".to_string(), o.steps, rs.time(), r));
        }
        if let Some(&r) = real_dts.get(o.steps - 1) {
            o.log.push(("deltaT".to_string(), o.steps, rs.delta_t(), r));
        }
    }
    o.wall = clock.elapsed().as_secs_f64();
    o.last_time = rs.time();
    let s = fluid(&rs);
    let mut mine: Vec<(&str, Vec<f64>)> = vec![
        ("T", every_7th(s.temperature())),
        ("U", s.velocity().iter().step_by(7).flat_map(|v| [v.x, v.y, v.z]).collect()),
    ];
    if let Some(p) = s.p_rgh() {
        mine.push(("p_rgh", every_7th(p)));
    }
    mine.push(("p", every_7th(s.pressure())));
    mine.push(("htc", every_7th(s.htc())));
    if let Some(k) = s.turbulent_kinetic_energy() {
        mine.push(("k", every_7th(k)));
    }
    if let Some(e) = s.turbulent_dissipation() {
        mine.push(("epsilon", every_7th(e)));
    }
    if let Some(v) = s.turbulent_viscosity() {
        mine.push(("nut", every_7th(v)));
    }
    if let Some(v) = s.turbulent_thermal_diffusivity() {
        mine.push(("alphat", every_7th(v)));
    }
    for m in s.radial_power_models() {
        if let RadialPowerModel::NuclearSteadyStatePebble(pm) = m {
            mine.push(("TpS.nuclearSteadyStatePebble", every_7th(&pm.fields().tps)));
            mine.push(("Tfmax.nuclearSteadyStatePebble", every_7th(&pm.fields().tfmax)));
        }
        if let RadialPowerModel::LumpedNuclearStructure(lm) = m {
            // `Tmax.lumpedNuclearStructure` per cell of the mesh (0 outside the model's cells, as the field's constructor value)
            let mut tmax = vec![0.0; s.temperature().len()];
            for &c in lm.cells() {
                tmax[c] = lm.tmax(c);
            }
            mine.push(("Tmax.lumpedNuclearStructure", every_7th(&tmax)));
            if lm.tsurface().len() == tmax.len() {
                mine.push(("Tsurface.lumpedNuclearStructure", every_7th(lm.tsurface())));
            }
        }
    }
    // the reference fields are those of the real run's LAST written time: compare only when the port stopped at it
    if o.steps == o.real_steps {
        for (k, v) in &mine {
            if let Some(r) = sample_ref(tag, k) {
                o.fields.push((k.to_string(), max_norm(v, &r)));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    o
}

/// G3 (README round 89): the shipped case reads after the `stopAt` port, and `-initializeMappedFields` writes nothing (the real stage changes
/// no file: `refs/gfhr_r89/nssp_md5_{before,after}.txt.gz`). One step, to show the run starts.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "heavy reference data (a 10 MB staged case): enable the long-tests feature")]
fn r89_shipped_case_reads_and_initialize_mapped_fields_writes_nothing() {
    let o = run_variant("nssp", true, 1);
    o.print("nssp, step 1");
    assert_eq!(o.steps, 1);
}

/// G4 (README round 89): the tight protocol, 3 steps, gate 1e-9 relative on every compared number.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "heavy reference data (a 10 MB staged case): enable the long-tests feature")]
fn r89_t3_tight_protocol_matches_the_real_binary() {
    if !refs_dir().join("t3_log.gz").exists() {
        println!("no t3 reference");
        return;
    }
    let o = run_variant("t3", false, usize::MAX);
    o.print("t3 (tight)");
    assert_eq!(o.steps, o.real_steps, "step count");
    // the gate of G4 (stated before the run): every pebble number, `T` number and `deltaT` (printed at 17 digits here) and every field.
    // The `Time = ` line is the time NAME at `timePrecision 6`, compared as printed and not part of the gate.
    let lw = o.log.iter().filter(|r| r.0 != "Time").fold(0.0_f64, |m, (_, _, p, r)| m.max(((p - r) / r).abs()));
    println!("   t3 worst: log {lw:.3e}, fields {:.3e} (gate 1e-9)", o.field_worst());
    assert!(lw <= 1e-9 && o.field_worst() <= 1e-9, "G4 gate 1e-9: log {lw:.3e}, fields {:.3e}", o.field_worst());
}

/// G3, G5, G5b, G6 (README round 89): the shipped NSSP case AS SHIPPED, the whole run to its `stopAt nextWrite` end, against the real run.
/// `GFHR_SHIPPED_STEPS` caps the steps (default: the whole run).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (about 10 min): enable the long-tests feature")]
fn r89_shipped_nssp_whole_run_follows_the_real_binary() {
    let cap = match std::env::var("GFHR_SHIPPED_STEPS").map(|s| s.parse::<usize>()) {
        Ok(Ok(n)) => n,
        _ => usize::MAX,
    };
    let o = run_variant("nssp", true, cap);
    o.print("nssp AS SHIPPED");
    let (lw, _) = o.log_worst();
    if cap == usize::MAX {
        assert_eq!(o.steps, o.real_steps, "G3: the same number of steps to stopAt nextWrite");
        assert!(!o.fields.is_empty(), "the t = 50 fields compared");
        assert!(o.field_worst() <= 1e-5, "G6 criterion 1e-5: {:.3e}", o.field_worst());
    }
    assert!(lw <= 1e-5, "G5b criterion 1e-5 relative to the printed values: {lw:.3e}");
}

/// G8, G8b (README round 89): the shipped LPS case (`phaseProperties_lumpedParam`, `lumpedNuclearStructure`), the whole run to its
/// `stopAt nextWrite` end. `GFHR_SHIPPED_STEPS` caps the steps.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (about 10 min): enable the long-tests feature")]
fn r89_shipped_lps_whole_run_follows_the_real_binary() {
    if !refs_dir().join("lps_log.gz").exists() {
        println!("no lps reference");
        return;
    }
    let cap = match std::env::var("GFHR_SHIPPED_STEPS").map(|s| s.parse::<usize>()) {
        Ok(Ok(n)) => n,
        _ => usize::MAX,
    };
    let o = run_variant("lps", false, cap);
    o.print("lps AS SHIPPED");
    let (lw, _) = o.log_worst();
    if cap == usize::MAX {
        assert_eq!(o.steps, o.real_steps, "the same number of steps to stopAt nextWrite");
        assert!(!o.fields.is_empty(), "the t = 50 fields compared");
        assert!(o.field_worst() <= 1e-5, "criterion 1e-5: {:.3e}", o.field_worst());
    }
    assert!(lw <= 1e-5, "criterion 1e-5 relative to the printed values: {lw:.3e}");
}

/// Diagnostic (prints; runs only with `GFHR_PROBE=1`): the NaN count of each field after step 1 through `RegionSolvers`.
#[test]
fn probe_step1_fields() {
    if std::env::var("GFHR_PROBE").is_err() {
        return;
    }
    let root = stage("nssp");
    let mut rs = match RegionSolvers::read(&root, CouplingSwitches::default()) {
        Ok(r) => r,
        Err(e) => panic!("{e}"),
    };
    let nan = |v: &[f64]| v.iter().filter(|x| !x.is_finite()).count();
    {
        let s = fluid(&rs);
        println!("before: T nan {}, k {:?}, eps {:?}, nut {:?}", nan(s.temperature()), s.turbulent_kinetic_energy().map(nan), s.turbulent_dissipation().map(nan), s.turbulent_viscosity().map(nan));
    }
    if let Err(e) = rs.step() {
        panic!("{e}");
    }
    let s = fluid(&rs);
    let u: Vec<f64> = s.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
    println!(
        "after: T nan {}, U {}, p_rgh {:?}, k {:?}, eps {:?}, nut {:?}, alphat {:?}, htc {}",
        nan(s.temperature()),
        nan(&u),
        s.p_rgh().map(nan),
        s.turbulent_kinetic_energy().map(nan),
        s.turbulent_dissipation().map(nan),
        s.turbulent_viscosity().map(nan),
        s.turbulent_thermal_diffusivity().map(nan),
        nan(s.htc())
    );
    if let Some(ts) = s.turbulence_solves() {
        println!("turbulence solves {ts:?}");
    }
    for ps in s.pressure_solves() {
        println!("p_rgh {ps:?}");
    }
    println!("h solves {:?}", s.h_solves());
    let _ = std::fs::remove_dir_all(&root);
}
