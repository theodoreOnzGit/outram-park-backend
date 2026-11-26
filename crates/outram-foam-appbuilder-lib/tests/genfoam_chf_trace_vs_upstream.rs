// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `twoPhase` with `multiRegimeBoilingTRACECHF`, `multiRegimeBoilingVapourTRACE`, `CachardLiquid`/`CachardVapour` and
//! `lookUpTableCHF` vs the real binary (gh:#1361 round 10; gh:#1223, gh:#1224)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions (committed BEFORE the first port run) and results:
//! `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 10". The cases are CLOSED variants of
//! `featureCases/1D_CHF/imposedPower` (`tests/data/genfoam_chf_closed_gh1361/{constant_chf,lookup_table_chf}`, see its
//! `PROVENANCE.md`): NOT the shipped cases (through-flow boundary conditions, power time profiles and the 77-180 s run are
//! not wired); a code-to-code comparison only. Protocol (`scripts/chf_closed_run.sh`, mirrored by [`protocol_copy`]):
//! `writePrecision 17`, every step written, every linear-solver tolerance in `solvers` 1e-14 with `relTol 0`. The references
//! are fresh runs of the installed real binary (`refs/chf_*.ref.txt.gz`).

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{TwoPhaseCase, TwoPhaseSolver};

struct Reference {
    dt: Vec<f64>,
    fields: HashMap<(String, String), Vec<f64>>,
    times: Vec<String>,
    first: usize,
}

fn load_reference(name: &str) -> Reference {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
        .join(format!("{name}.ref.txt.gz"));
    let bytes = std::fs::read(&path).expect("reference present");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { dt: vec![], fields: HashMap::new(), times: vec![], first: 1 };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("FIRST") => r.first = w.next().expect("first").parse().expect("first"),
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("F") => {
                let t = w.next().expect("time").to_string();
                let f = w.next().expect("field").to_string();
                let v: Vec<f64> = w.map(|x| x.parse().expect("value")).collect();
                if !r.times.contains(&t) {
                    r.times.push(t.clone());
                }
                r.fields.insert((t, f), v);
            }
            _ => {}
        }
    }
    r
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for e in std::fs::read_dir(from).expect("read_dir") {
        let e = e.expect("entry");
        let dest = to.join(e.file_name());
        if e.file_type().expect("type").is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), &dest).expect("copy");
        }
    }
}

/// Copy of the vendored closed case with the protocol of `scripts/chf_closed_run.sh` applied (every step written, tight tolerances).
fn protocol_copy(variant: &str, test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_chf_closed_gh1361").join(variant);
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1361_chf_{test}_{}_{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let cd = dst.join("system/controlDict");
    let text = std::fs::read_to_string(&cd).expect("controlDict");
    let text: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("writeControl ") {
                "writeControl    timeStep;".to_string()
            } else if l.starts_with("writeInterval ") {
                "writeInterval   1;".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    std::fs::write(&cd, text.join("\n") + "\n").expect("write controlDict");
    let f = dst.join("system/fluidRegion/fvSolution");
    let text = std::fs::read_to_string(&f).expect("fvSolution");
    let mut out = String::new();
    let mut in_solvers = false;
    for line in text.lines() {
        let t = line.trim_start();
        if line.starts_with("solvers") {
            in_solvers = true;
        }
        if line.starts_with("PIMPLE") {
            in_solvers = false;
        }
        if in_solvers && t.starts_with("tolerance") {
            out.push_str("        tolerance       1e-14;\n");
        } else if in_solvers && t.starts_with("relTol") {
            out.push_str("        relTol          0;\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(&f, out).expect("write fvSolution");
    dst
}

/// Largest `|a - b|` over a field divided by the field's largest `|b|` (an exact zero compares exactly).
fn worst_rel(port: &[f64], up: &[f64]) -> f64 {
    assert_eq!(port.len(), up.len(), "reference field has the wrong length");
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter()
        .zip(up)
        .map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale })
        .fold(0.0, f64::max)
}

fn port_fields(s: &TwoPhaseSolver) -> Vec<(&'static str, Vec<f64>)> {
    vec![
        ("p", s.p.c.clone()),
        ("p_rgh", s.p_rgh.c.clone()),
        ("T.liquid", s.fluids[0].t.c.clone()),
        ("T.vapour", s.fluids[1].t.c.clone()),
        ("T.interface", s.t_interface.clone()),
        ("T.fixedPower", s.structure.t_act.clone()),
        ("U.liquid", s.fluids[0].u.c.iter().map(|u| u.x).collect()),
        ("U.vapour", s.fluids[1].u.c.iter().map(|u| u.x).collect()),
        ("alpha.vapour", s.fluids[1].alpha.c.clone()),
        ("dmdt.liquid.vapour", s.dmdt.clone()),
        ("htc.liquid.structure", s.htc_fs[0].clone()),
        ("htc.vapour.structure", s.htc_fs[1].clone()),
        ("areaDensity.interface", s.interfacial_area.clone()),
        ("alphaRhoMagU.liquid", s.fluids[0].alpha_rho_mag_u.clone()),
    ]
}

/// (step, relative step-size difference, port outer iterations, regime code of cell 5, worst relative difference per field).
type Row = (usize, f64, usize, f64, Vec<(&'static str, f64)>);

fn run_rows(variant: &str, refs: &[&str], n_steps: usize) -> Vec<Row> {
    run_rows_with(variant, refs, n_steps, &round13_flags())
}

/// [`run_rows`] with the round-13 translations named in `off` (`gas`, `newton`, `markers`, `flux`) switched back to the earlier forms.
fn run_rows_with(variant: &str, refs: &[&str], n_steps: usize, off: &[String]) -> Vec<Row> {
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let dir = protocol_copy(variant, "rows");
    let case = TwoPhaseCase::read(&dir).expect("read the closed 1D_CHF case");
    assert!(case.deviations.is_empty(), "{:?}", case.deviations);
    // upstream's customPimpleControl defect is the specification (as in rounds 3-9)
    let mut s = round13_solver_f(TwoPhaseSolver::new(round13_case_f(case, off)).with_upstream_stale_pimple_converged_flag(), off);
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = s.step().expect("step");
        let Some(r) = refs.iter().find(|r| n >= r.first && n - r.first < r.times.len()) else { continue };
        let dt_up = if n == 1 { 5e-3 } else { r.dt[n - 2] };
        let dt_rel = (rep.dt - dt_up).abs() / dt_up;
        let tname = &r.times[n - r.first];
        let mut worst = Vec::new();
        for (name, v) in port_fields(&s) {
            if let Some(u) = r.fields.get(&(tname.clone(), name.to_string())) {
                worst.push((name, worst_rel(&v, u)));
            }
        }
        rows.push((n, dt_rel, rep.outer_iterations, s.chf_bool[5], worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[Row]) {
    for (n, dt_rel, iters, regime, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} iters={iters} chfBool5={regime} {}", cells.join(" "));
    }
}

/// Diagnostic: print every compared step (`--ignored --nocapture`); `VARIANT` (`constant_chf`, `lookup_table_chf`), `REFS`, `STEPS`.
#[test]
#[ignore]
fn chf_closed_dump_rows() {
    let variant = std::env::var("VARIANT").unwrap_or_else(|_| "constant_chf".into());
    let refs = std::env::var("REFS").unwrap_or_else(|_| "chf_constant_all".into());
    let refs: Vec<&str> = refs.split(',').collect();
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    print_rows(&run_rows(&variant, &refs, n));
}

/// Diagnostic: per-cell port and upstream values of the listed fields after `STEP` steps (default 1), with the regime code the port's
/// `multiRegimeBoilingTRACECHF` left in the cell (`VARIANT`, `REFS` as above; `FIELDS` comma separated).
#[test]
#[ignore]
fn chf_closed_cell_dump() {
    let variant = std::env::var("VARIANT").unwrap_or_else(|_| "constant_chf".into());
    let refs = std::env::var("REFS").unwrap_or_else(|_| "chf_constant_all".into());
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let fields = std::env::var("FIELDS").unwrap_or_else(|_| "htc.liquid.structure,T.fixedPower,alpha.vapour,U.liquid".into());
    let r = load_reference(&refs);
    let dir = protocol_copy(&variant, "cells");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    for _ in 1..=step {
        s.step().expect("step");
    }
    let tname = &r.times[step - r.first];
    let port = port_fields(&s);
    for f in fields.split(',') {
        let Some((_, v)) = port.iter().find(|(n, _)| *n == f) else { continue };
        let u = &r.fields[&(tname.clone(), f.to_string())];
        for (c, (a, b)) in v.iter().zip(u).enumerate() {
            eprintln!(
                "CELL step {step} {f:24} cell {c:2} chfBool={} port={a:.17e} up={b:.17e} rel={:.2e}",
                s.chf_bool[c],
                if *b == 0.0 { (a - b).abs() } else { (a - b).abs() / b.abs() }
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Noise-floor control (round-3 technique): the port against ITSELF with the initial liquid temperature scaled by `1 + eps`.
fn noise_rows(variant: &str, eps: f64, n_steps: usize) -> Vec<(usize, Vec<(&'static str, f64)>)> {
    let dir = protocol_copy(variant, "noise");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut pert = case.clone();
    for v in pert.fluids[0].t.c.iter_mut() {
        *v *= 1.0 + eps;
    }
    for v in pert.fluids[0].t.b.iter_mut() {
        *v *= 1.0 + eps;
    }
    let mut a = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    let mut b = round13_solver(TwoPhaseSolver::new(round13_case(pert)).with_upstream_stale_pimple_converged_flag());
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        a.step().expect("step a");
        b.step().expect("step b");
        let fa = port_fields(&a);
        let fb = port_fields(&b);
        rows.push((n, fa.iter().zip(&fb).map(|((k, x), (_, y))| (*k, worst_rel(y, x))).collect()));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

/// Round 13 diagnostic: every evaluation of `multiRegimeBoilingTRACECHF` (all cells, all outer iterations) of the first `STEPS`
/// steps (default 3) of `VARIANT`, with its intermediates at 17 digits, written to the file `PROBE_OUT` (one line per cell
/// evaluation: `PORT_CHF t= corr= cell= name=value ...`), for comparison with the instrumented upstream's dump
/// (`upstream_driver/probe_boiling`).
#[test]
#[ignore]
fn chf_closed_trace_probe_dump() {
    let variant = std::env::var("VARIANT").unwrap_or_else(|_| "constant_chf".into());
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let dir = protocol_copy(&variant, "probe");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    s.trace_probe = Some(Vec::new());
    for _ in 1..=n {
        s.step().expect("step");
    }
    let mut out = String::new();
    for r in s.trace_probe.as_ref().expect("probe on") {
        let tag = match r.kind {
            "chf" => "PORT_CHF",
            "eqn" => "PORTE",
            "eqnf" => "PORTEF",
            "pc" => "PORT_PC",
            "structure" => "PORTS",
            _ => continue,
        };
        out.push_str(&format!("{tag} t={:.17e} corr={} cell={}", r.time, r.outer_iteration, r.cell));
        for (k, v) in &r.values {
            out.push_str(&format!(" {k}={v:.17e}"));
        }
        out.push('\n');
    }
    if let Ok(path) = std::env::var("PROBE_OUT") {
        std::fs::write(path, out).expect("write probe");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic: the noise-floor control (`VARIANT`, `EPS` default 1e-12, `STEPS` default 10).
#[test]
#[ignore]
fn chf_closed_noise_floor_dump() {
    let variant = std::env::var("VARIANT").unwrap_or_else(|_| "constant_chf".into());
    let eps: f64 = std::env::var("EPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-12);
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    for (n, w) in noise_rows(&variant, eps, n) {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("NOISE step {n:4} {}", cells.join(" "));
    }
}

/// Diagnostic: the port on the vendored `VARIANT` against the port on the case directory `OTHER_CASE` (only its physics files
/// matter; the protocol files are copied from the variant), step by step. Used to measure how a small change of an input (e.g. the
/// CHF table scaled by `1 + 1e-6`) is amplified by the post-CHF transition branch.
#[test]
#[ignore]
fn chf_closed_two_case_dump() {
    let variant = std::env::var("VARIANT").unwrap_or_else(|_| "lookup_table_chf".into());
    let other = PathBuf::from(std::env::var("OTHER_CASE").expect("OTHER_CASE"));
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let dir_a = protocol_copy(&variant, "twoA");
    let dir_b = std::env::temp_dir().join(format!("gh1361_chf_twoB_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_b);
    copy_dir(&other, &dir_b);
    for f in ["system/controlDict", "system/fluidRegion/fvSolution"] {
        std::fs::copy(dir_a.join(f), dir_b.join(f)).expect("copy protocol file");
    }
    let mut a = TwoPhaseSolver::new(TwoPhaseCase::read(&dir_a).expect("read a")).with_upstream_stale_pimple_converged_flag();
    let mut b = TwoPhaseSolver::new(TwoPhaseCase::read(&dir_b).expect("read b")).with_upstream_stale_pimple_converged_flag();
    for k in 1..=n {
        a.step().expect("step a");
        b.step().expect("step b");
        let cells: Vec<String> = port_fields(&a)
            .iter()
            .zip(port_fields(&b))
            .map(|((name, x), (_, y))| format!("{name}={:.2e}", worst_rel(&y, x)))
            .collect();
        eprintln!("TWO step {k:3} {}", cells.join(" "));
    }
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

/// Diagnostic: the critical heat flux the real binary USED in a transition-boiling cell, inferred from what it wrote, next to the one the
/// port computes there. In the transition branch `htc = q wf / (Tw - Tf) + (1 - wf) h_min` with `wf = ((Tw - T_LF)/(T_CHF(q) - T_LF))^2`
/// and `h_min` the (radiation-dominated) Cachard coefficient at `T_LF`: one equation in `q`, solved by bisection from upstream's
/// `htc.liquid.structure`, `T.fixedPower` (`Twall` of the last outer iteration, equal to it to ~1e-9) and the port's own `p`, `T_sat`, `T_f`
/// (agreeing to <= 1e-8 with upstream's); the written htc is multiplied by the contact partition (0.7 in `lookup_table_chf`), which is divided out. `VARIANT`, `REFS`, `STEP`, `CELL`.
#[test]
#[ignore]
fn chf_closed_implied_q_chf() {
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace_chf::gorenflo_t_chf;
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::cachard::{
        CachardLiquid, CachardLiquidInputs,
    };
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::chf::LeidenfrostTemperature;
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::chf_table::LookUpTableChfInputs;
    use uom::si::f64::{Pressure, ThermodynamicTemperature};
    let variant = std::env::var("VARIANT").unwrap_or_else(|_| "constant_chf".into());
    let refs = std::env::var("REFS").unwrap_or_else(|_| "chf_constant_all".into());
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let r = load_reference(&refs);
    let dir = protocol_copy(&variant, "implied");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    for _ in 1..=step {
        s.step().expect("step");
    }
    let tname = &r.times[step - r.first];
    let up = |f: &str| r.fields[&(tname.clone(), f.to_string())].clone();
    let (htc_up, tw_up) = (up("htc.liquid.structure"), up("T.fixedPower"));
    let cachard = CachardLiquid::new(0.84, 0.95);
    for c in [2usize, 5, 10, 16] {
        let (p, tsat, tf) = (s.p.c[c], s.t_interface[c], s.fluids[0].t.c[c]);
        let tw = tw_up[c];
        let tlf = LeidenfrostTemperature::GroeneveldStewart { critical_pressure: Pressure::new::<uom::si::pressure::pascal>(2.209e7) }
            .temperature(Pressure::new::<uom::si::pressure::pascal>(p), ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(tsat))
            .get::<uom::si::thermodynamic_temperature::kelvin>();
        let cin = |t_wall: f64| CachardLiquidInputs {
            t_wall,
            t_sat: tsat,
            t_liquid: tf,
            alpha_vapour: s.fluids[1].normalized[c],
            p,
            rho_liquid: s.fluids[0].rho.c[c],
            rho_vapour: s.fluids[1].rho.c[c],
            kappa_vapour: s.fluids[1].kappa.c[c],
            mu_vapour: s.fluids[1].mu.c[c],
            dh_vapour: s.fluids[1].dh[c],
        };
        let h_min = cachard.value(&cin(tlf)).0;
        let dt = tw - tf;
        let model = |q: f64| {
            let t_chf = gorenflo_t_chf(tsat, p, q, 4e-7);
            let ratio_w = (tw - tlf) / (t_chf - tlf);
            let wf = ratio_w * ratio_w;
            (q * wf / dt + (1.0 - wf) * h_min, t_chf, wf)
        };
        let (mut lo, mut hi) = (1e4_f64, 1e8_f64);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if model(mid).0 < htc_up[c] / s.contact[0][c] {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let q_implied = 0.5 * (lo + hi);
        let (_, t_chf, wf) = model(q_implied);
        let q_port = match s.models.chf_tables.first() {
            Some(t) => t
                .value(&LookUpTableChfInputs {
                    flow_quality: s.fluids[0].quality[c],
                    p,
                    rho_liquid: s.fluids[0].rho.c[c],
                    rho_vapour: s.fluids[1].rho.c[c],
                    mag_u_liquid: s.fluids[0].mag_u[c],
                    mag_u_vapour: s.fluids[1].mag_u[c],
                    alpha_liquid: s.fluids[0].normalized[c],
                    dh: s.structure.dh[c],
                    h_liquid_saturated: s.fluids[0].thermo.he(p, tsat),
                    latent_heat: s.latent_heat[c].abs(),
                    h_liquid: s.fluids[0].he.c[c],
                })
                .expect("table"),
            None => 1.66e6,
        };
        eprintln!(
            "IMPLIED-CHECK cell {c:2} htc(model at q_port)={:.9e} htc_port={:.9e} htc_up={:.9e} G={:.6} quality={:.6e} xe={:.6e}",
            model(q_port).0 * s.contact[0][c],
            s.htc_fs[0][c],
            htc_up[c],
            s.fluids[0].rho.c[c] * s.fluids[0].mag_u[c] * s.fluids[0].normalized[c]
                + s.fluids[1].rho.c[c] * s.fluids[1].mag_u[c] * (1.0 - s.fluids[0].normalized[c]),
            s.fluids[0].quality[c],
            (s.fluids[0].quality[c] * s.fluids[0].he.c[c]
                + (1.0 - s.fluids[0].quality[c]) * (s.fluids[0].thermo.he(p, tsat) + s.latent_heat[c].abs())
                - s.fluids[0].thermo.he(p, tsat))
                / s.latent_heat[c].abs()
        );
        eprintln!(
            "IMPLIED step {step} cell {c:2} Tw={tw:.9} Tsat={tsat:.9} TLF={tlf:.9} TCHF(implied)={t_chf:.9} wf={wf:.6} q_implied={q_implied:.9e} q_port={q_port:.9e} rel={:.3e}",
            (q_port - q_implied) / q_implied
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn worst_of(rows: &[Row], lo: usize, hi: usize, field: &str) -> f64 {
    rows.iter()
        .filter(|(n, ..)| *n >= lo && *n <= hi)
        .flat_map(|(_, _, _, _, w)| w.iter().filter(|(k, _)| *k == field).map(|(_, v)| *v))
        .fold(0.0, f64::max)
}

/// Round 10, gate 1 (`constant_chf`): the shipped `constantCHF 1.66e6`, `GroeneveldStewart`, `CachardLiquid`/`CachardVapour` (upstream's
/// pressure-as-pitch binding) against the real binary, steps 1-40: the transition-boiling branch at steps 1-3 and the film-boiling
/// branch after it. Measured 2026-11-26 (README "Round 10"): worst over steps 1-40 `p` 2.4e-8, `T.liquid` 3.3e-9, `T.fixedPower` 2.9e-7,
/// `U.liquid` 3.9e-6, `U.vapour` 4.3e-5, `alpha.vapour` 7.3e-6, `dmdt` 3.9e-5, `htc.liquid.structure` 7.7e-7, `dt` 3.6e-6 relative; the
/// bounds are about ten times the measurement. Not bit-identical anywhere (the `p_rgh` solves differ, as in rounds 3-9), and the first
/// step is already above the 1e-9 allowance (README: cause not identified).
#[test]
fn constant_chf_transition_and_film_boiling_agree_with_upstream_to_step_40() {
    let rows = run_rows("constant_chf", &["chf_constant_all"], 40);
    assert_eq!(rows.len(), 40);
    let bounds: [(&str, f64); 9] = [
        ("p", 1e-6),
        ("T.liquid", 1e-7),
        ("T.fixedPower", 1e-5),
        ("U.liquid", 1e-4),
        ("U.vapour", 5e-4),
        ("alpha.vapour", 1e-4),
        ("dmdt.liquid.vapour", 5e-4),
        ("htc.liquid.structure", 1e-5),
        ("htc.vapour.structure", 0.0),
    ];
    for (name, bound) in bounds {
        let w = worst_of(&rows, 1, 40, name);
        assert!(w <= bound, "{name}: worst {w:.3e} over steps 1-40 exceeds {bound:.0e}");
    }
    let dt = rows.iter().map(|r| r.1).fold(0.0, f64::max);
    assert!(dt <= 1e-5, "dt relative difference {dt:.3e}");
    // the port's regime in cell 5 (upstream does not write it): transition boiling (2) at steps 1-3, film boiling (1) after
    for (n, _, _, regime, _) in &rows {
        assert_eq!(*regime, if *n <= 3 { 2.0 } else { 1.0 }, "step {n}");
    }
}

/// Round 13 (README "Round 13"): the vapour-threshold event of step 41 no longer parts the two codes. Round 10 had the port keep a
/// solved vapour enthalpy (`T.vapour` 631.9 K) where upstream's `cutoff` left it at saturation (613.9 K) from step 41 on (`T.vapour`
/// 2.9e-2, `alpha.vapour` 2.7e-2 at step 41, `p` 5.6e-3 and `alpha` 2.1 by step 86). Cause (ablation: only switching `flux` back
/// restores it): the interfacial flux ratio `dmdtIi/dmdtIi0` of `phaseChangeModel::correct`, which the port computed from the total `dmdt`;
/// in film boiling the wall share `fw` is far from 0, so the energy split of the cells that cross the vapour threshold was wrong. Measured over steps 41-86
/// (the whole reference, release 2026-11-26): `p` 2.7e-9, `T.liquid` 4.5e-9, `T.vapour` 6.1e-10, `T.fixedPower` 2.9e-7, `U.liquid` 2.0e-6,
/// `U.vapour` 1.8e-6, `alpha.vapour` 3.3e-6, `dmdt` 2.2e-6, `htc.liquid.structure` 2.2e-7, `dt` 8.1e-7. The step-1 seed of gate 1 (1e-6 in `U`
/// and `alpha`, transition boiling at once) is NOT explained: the TRACECHF closure intermediates agree to <= 1e-9 through the 4th outer
/// iteration (README "Round 13", `chf_closed_trace_probe_dump`).
#[test]
fn constant_chf_agrees_with_upstream_through_the_whole_reference_to_step_86() {
    let rows = run_rows("constant_chf", &["chf_constant_all"], 86);
    assert_eq!(rows.len(), 86);
    let bounds: [(&str, f64); 10] = [
        ("p", 1e-7),
        ("T.liquid", 1e-7),
        ("T.vapour", 1e-7),
        ("T.fixedPower", 3e-6),
        ("U.liquid", 1e-4),
        ("U.vapour", 1e-4),
        ("alpha.vapour", 1e-4),
        ("dmdt.liquid.vapour", 1e-4),
        ("htc.liquid.structure", 1e-5),
        ("areaDensity.interface", 1e-4),
    ];
    for (name, bound) in bounds {
        let w = worst_of(&rows, 41, 86, name);
        assert!(w <= bound, "{name}: worst {w:.3e} over steps 41-86 exceeds {bound:.0e}");
    }
    let dt = rows.iter().map(|r| r.1).fold(0.0, f64::max);
    assert!(dt <= 1e-5, "dt relative difference {dt:.3e}");
}

/// The earlier form of the flux ratio stays selectable: with `dmdt/dmdtIi0` back the constant-CHF run parts at the vapour-threshold
/// event of step 41 as in round 10 (`T.vapour` 2.9e-2 and `alpha.vapour` 2.7e-2 of their maxima there; steps 1-40 agree to 1e-6).
#[test]
fn constant_chf_with_the_earlier_flux_ratio_parts_at_step_41_as_in_round_10() {
    let off: Vec<String> = vec!["flux".to_string()];
    let rows = run_rows_with("constant_chf", &["chf_constant_all"], 41, &off);
    let t_vap = worst_of(&rows, 41, 41, "T.vapour");
    let alpha = worst_of(&rows, 41, 41, "alpha.vapour");
    assert!(t_vap > 1e-2 && t_vap < 1e-1, "T.vapour step 41 with the earlier flux ratio: {t_vap:.3e}");
    assert!(alpha > 1e-2 && alpha < 1e-1, "alpha.vapour step 41 with the earlier flux ratio: {alpha:.3e}");
    assert!(worst_of(&rows, 1, 40, "T.vapour") < 1e-7);
}

/// Round 10, gate 3 (`lookup_table_chf`): `lookUpTableCHF` with the PSBT table read through `#include "chfTables"`, `PitchToDiameter
/// 1.32631578947`, contact partition 0.7 (so `multiRegimeBoilingVapourTRACE` shows in `htc.vapour.structure`), steps 1-3 (transition
/// boiling in every heated cell). Measured: worst `p` 3.3e-6, `T.fixedPower` 2.6e-6, `alpha.vapour` 5.6e-4, `dmdt` 6.0e-4,
/// `htc.liquid.structure` 8.0e-4, `htc.vapour.structure` 4.3e-4, `dt` 8.7e-5. The transition branch amplifies a small wall-temperature
/// difference (1e-3 K) by `1/(T_CHF - T_LF)`; the critical heat flux the real binary used, inferred from its written htc, equals the port's
/// to the method's resolution (3e-3; README). At step 4 the two codes part (`alpha.vapour` 9e-2): not gated, recorded.
#[test]
fn lookup_table_chf_agrees_with_upstream_for_the_first_three_steps() {
    let dir = protocol_copy("lookup_table_chf", "read");
    let case = TwoPhaseCase::read(&dir).expect("the lookUpTableCHF case reads (the table follows the #include)");
    assert_eq!(case.models.chf_tables.len(), 1);
    assert!(case.deviations.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
    let rows = run_rows("lookup_table_chf", &["chf_lookup_head"], 3);
    assert_eq!(rows.len(), 3);
    let bounds: [(&str, f64); 7] = [
        ("p", 1e-4),
        ("T.fixedPower", 1e-4),
        ("alpha.vapour", 5e-3),
        ("dmdt.liquid.vapour", 5e-3),
        ("htc.liquid.structure", 5e-3),
        ("htc.vapour.structure", 5e-3),
        ("U.liquid", 1e-3),
    ];
    for (name, bound) in bounds {
        let w = worst_of(&rows, 1, 3, name);
        assert!(w <= bound, "{name}: worst {w:.3e} over steps 1-3 exceeds {bound:.0e}");
    }
    let dt = rows.iter().map(|r| r.1).fold(0.0, f64::max);
    assert!(dt <= 1e-3, "dt relative difference {dt:.3e}");
}

/// Round 10: the constant-CHF case needs no table and reads with none.
#[test]
fn constant_chf_case_reads_without_a_table() {
    let dir = protocol_copy("constant_chf", "read");
    let case = TwoPhaseCase::read(&dir).expect("read");
    assert!(case.models.chf_tables.is_empty());
    assert!(case.deviations.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 13 ablation (diagnostic): `ROUND13_OFF=gas,newton,markers,flux` (any subset) switches the named round-13 translations back
/// to the port's earlier forms, to measure how much of the agreement each one carries. Unset: upstream's forms (the defaults).
fn round13_flags() -> Vec<String> {
    std::env::var("ROUND13_OFF").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default()
}

fn round13_case_f(case: TwoPhaseCase, off: &[String]) -> TwoPhaseCase {
    if off.iter().any(|x| x == "gas") {
        case.with_codata2019_gas_constant()
    } else {
        case
    }
}

fn round13_solver_f(mut s: TwoPhaseSolver, off: &[String]) -> TwoPhaseSolver {
    if off.iter().any(|x| x == "newton") {
        s = s.with_converged_temperature_newton();
    }
    if off.iter().any(|x| x == "markers") {
        s = s.with_live_thermo_residual_markers();
    }
    if off.iter().any(|x| x == "flux") {
        s = s.with_earlier_interfacial_flux_ratio();
    }
    if off.iter().any(|x| x == "hsolver") {
        s = s.with_smooth_solver_for_enthalpy();
    }
    s
}

fn round13_case(case: TwoPhaseCase) -> TwoPhaseCase {
    round13_case_f(case, &round13_flags())
}

fn round13_solver(s: TwoPhaseSolver) -> TwoPhaseSolver {
    round13_solver_f(s, &round13_flags())
}
