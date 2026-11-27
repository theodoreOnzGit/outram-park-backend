// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `1D_CHF/imposedPower` AS SHIPPED vs the real binary (gh:#1361 round 12; gh:#1815, gh:#1801)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The shipped case (`tests/data/genfoam_chf_shipped_gh1815`): through-flow boundary conditions (gh:#1801) and a `fixedPower`
//! `powerTimeProfile` (gh:#1815, upstream's recursion). Protocol `scripts/chf_shipped_protocol.sh` (mirrored by [`protocol_copy`]):
//! `writeInterval 1`, `endTime 13`, linear solvers 1e-14. Reference: `refs/chf_shipped_t13.ref.txt.gz` (the written times 1..13).
//! Methodology, predictions and results: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 12".

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

/// Copy of the vendored SHIPPED 1D_CHF imposedPower case with `scripts/chf_shipped_protocol.sh` applied (writeInterval 1, endTime 13, TIGHT).
fn protocol_copy(test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_chf_shipped_gh1815");
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1815_chf_{test}_{}_{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let cd = dst.join("system/controlDict");
    let text = std::fs::read_to_string(&cd).expect("controlDict");
    let text: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("endTime ") {
                "endTime         13;".to_string()
            } else if l.starts_with("writeInterval ") {
                format!("writeInterval   {};", std::env::var("CHF_WRITE_INTERVAL").unwrap_or_else(|_| "1".into()))
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
    // a field whose cells are all equal is written `uniform` and was expanded to 40 values by the extractor
    let up = if up.len() == 40 && port.len() < 40 { &up[..port.len()] } else { up };
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
        ("areaDensity.interface", s.interfacial_area.clone()),
        ("alphaRhoMagU.liquid", s.fluids[0].alpha_rho_mag_u.clone()),
        ("powerDensity.fixedPower", s.structure.power.iter().map(|p| p.as_ref().map_or(0.0, |m| m.current_power_density().get::<uom::si::volumetric_power_density::watt_per_cubic_meter>())).collect()),
        ("phi.liquid", s.fluids[0].phi[..s.geom.n_internal()].to_vec()),
        ("phi.vapour", s.fluids[1].phi[..s.geom.n_internal()].to_vec()),
    ]
}

/// Rows `(time, port field -> worst relative difference)` at every integer time the reference holds.
fn run_rows(test: &str, end: usize) -> Vec<(usize, usize, Vec<(&'static str, f64)>)> {
    let r = load_reference("chf_shipped_t13");
    let dir = protocol_copy(test);
    let case = TwoPhaseCase::read(&dir).expect("read the SHIPPED CHF case");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    let mut rows = Vec::new();
    let mut steps = 0usize;
    for k in 1..=end {
        while s.time < k as f64 - 1e-9 {
            s.step().expect("step");
            steps += 1;
        }
        let tname = k.to_string();
        let mut worst = Vec::new();
        for (name, v) in port_fields(&s) {
            if let Some(u) = r.fields.get(&(tname.clone(), name.to_string())) {
                worst.push((name, worst_rel(&v, u)));
            }
        }
        rows.push((k, steps, worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

/// Diagnostic: print the comparison at every written time (`--ignored --nocapture`).
#[test]
#[ignore]
fn chf_shipped_dump_rows() {
    for (k, steps, w) in run_rows("dump", 13) {
        let cells: Vec<String> = w.iter().map(|(n, v)| format!("{n}={v:.2e}")).collect();
        eprintln!("t={k:3} steps={steps:5} {}", cells.join(" "));
    }
}

/// Round 12 part B gates (README "Round 12", predictions W2 and W3): through t = 10 s (~950 single-phase steps, constant power) every compared
/// field of the SHIPPED through-flow case agrees with the real binary to 1e-6 of its maximum (`htc` 7.7e-9, `p` 1.7e-11, `U` 6e-8, measured), and
/// at t = 11 s `powerDensity.fixedPower` equals upstream's recursion value to 1e-12 (upstream's 9.975e8, 0.25 % below the profile's 1e9).
#[test]
fn shipped_chf_case_matches_upstream_to_t10_and_the_power_recursion_at_t11() {
    let rows = run_rows("gate", 11);
    for (k, _, w) in &rows {
        for (name, v) in w {
            if *k <= 10 {
                assert!(*v < 1e-6, "t = {k} s {name}: {v:e}");
            } else if *name == "powerDensity.fixedPower" {
                assert!(*v < 1e-12, "t = {k} s powerDensity: {v:e}");
            }
        }
    }
}

/// The deprecated closed form (`initial * prof(t)`) is NOT upstream's: the table holds absolute values, so it multiplies the
/// initial 5e8 by the table's 5e8 (measured at t = 1 s: 2.5e17 against upstream's 5e8).
#[test]
fn closed_form_power_profile_is_not_upstreams_recursion() {
    let dir = protocol_copy("closed_form");
    let case = TwoPhaseCase::read(&dir).expect("read").with_closed_form_power_profile();
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    s.step().expect("step");
    let r = load_reference("chf_shipped_t13");
    let up = &r.fields[&("1".to_string(), "powerDensity.fixedPower".to_string())];
    let up_max = up.iter().fold(0.0_f64, |m, x| m.max(*x));
    let port_max = s
        .structure
        .power
        .iter()
        .map(|p| p.as_ref().map_or(0.0, |m| m.current_power_density().get::<uom::si::volumetric_power_density::watt_per_cubic_meter>()))
        .fold(0.0_f64, f64::max);
    assert!(port_max > 1e3 * up_max, "closed form {port_max:e} vs upstream {up_max:e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic: the comparison at every written time of a finer reference (`REF`, default `chf_shipped_fine`; `CHF_WRITE_INTERVAL` the
/// matching write interval, default 0.125; `END` the last time). Prints the fields that matter for the onset.
#[test]
#[ignore]
fn chf_shipped_fine_dump() {
    let name = std::env::var("REF").unwrap_or_else(|_| "chf_shipped_fine".into());
    let end: f64 = std::env::var("END").ok().and_then(|v| v.parse().ok()).unwrap_or(11.0);
    std::env::set_var("CHF_WRITE_INTERVAL", std::env::var("CHF_WRITE_INTERVAL").unwrap_or_else(|_| "0.125".into()));
    let r = load_reference(&name);
    let dir = protocol_copy("fine");
    let case = TwoPhaseCase::read(&dir).expect("read the SHIPPED CHF case");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    let mut times: Vec<(f64, String)> = r.times.iter().map(|t| (t.parse::<f64>().expect("time"), t.clone())).collect();
    times.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("order"));
    let mut steps = 0usize;
    for (t, tname) in times {
        if t > end {
            break;
        }
        while s.time < t - 1e-9 {
            s.step().expect("step");
            steps += 1;
        }
        let mut cells = Vec::new();
        for (n, v) in port_fields(&s) {
            if let Some(u) = r.fields.get(&(tname.clone(), n.to_string())) {
                if matches!(n, "p" | "T.liquid" | "T.fixedPower" | "U.liquid" | "alpha.vapour" | "htc.liquid.structure" | "powerDensity.fixedPower") {
                    cells.push(format!("{n}={:.2e}", worst_rel(&v, u)));
                }
            }
        }
        let amax = s.fluids[1].alpha.c.iter().fold(0.0_f64, |m, x| m.max(*x));
        let up_amax = r.fields[&(tname.clone(), "alpha.vapour".to_string())].iter().fold(0.0_f64, |m, x| m.max(*x));
        let up_pd = r.fields[&(tname.clone(), "powerDensity.fixedPower".to_string())].iter().fold(0.0_f64, |m, x| m.max(*x));
        let pd = port_fields(&s).into_iter().find(|(k, _)| *k == "powerDensity.fixedPower").map_or(0.0, |(_, v)| v.iter().fold(0.0_f64, |m, x| m.max(*x)));
        eprintln!("t={t:8.4} steps={steps:5} alpha_max port={amax:.4e} up={up_amax:.4e} pd port={pd:.10e} up={up_pd:.10e} {}", cells.join(" "));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
