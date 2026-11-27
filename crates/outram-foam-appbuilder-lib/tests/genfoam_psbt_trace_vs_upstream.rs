// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `twoPhase` with `multiRegimeBoilingTRACE`, `twoParameters` and `byRegime` vs the real binary (gh:#1361, round 8)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions (committed BEFORE the first port run) and results:
//! `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 8". The case is the CLOSED variant of
//! `featureCases/1D_PSBT_SC/Phase_Ex1_12223` (`tests/data/genfoam_psbt_closed_gh1361`, see its `PROVENANCE.md`: only
//! boundary-condition files differ from the shipped tutorial). Protocol (`scripts/psbt_closed_protocol.sh`, mirrored by
//! [`protocol_copy`]): `writePrecision 17`, every step written, every linear-solver tolerance in `solvers` 1e-14 with
//! `relTol 0`. The reference is a fresh run of the installed real binary (`refs/psbt_closed_*.ref.txt.gz`).

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

/// Copy of the vendored closed case with `scripts/psbt_closed_protocol.sh` applied (EVERY_STEP=1, TIGHT=1).
fn protocol_copy(test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_psbt_closed_gh1361");
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1361_psbt_{test}_{}_{k}", std::process::id()));
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
        ("areaDensity.interface", s.interfacial_area.clone()),
        ("alphaRhoMagU.liquid", s.fluids[0].alpha_rho_mag_u.clone()),
    ]
}

/// (step, relative step-size difference, port outer iterations, worst relative difference per field).
type Row = (usize, f64, usize, Vec<(&'static str, f64)>);

fn run_rows(test: &str, refs: &[&str], n_steps: usize) -> Vec<Row> {
    run_rows_with(test, refs, n_steps, &round13_flags())
}

/// [`run_rows`] with the round-13 translations named in `off` switched back to the earlier forms (see [`round13_off`]).
fn run_rows_with(test: &str, refs: &[&str], n_steps: usize, off: &[String]) -> Vec<Row> {
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let dir = protocol_copy(test);
    let case = TwoPhaseCase::read(&dir).expect("read the closed PSBT case");
    // upstream's customPimpleControl defect is the specification (as in rounds 3-7)
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
        rows.push((n, dt_rel, rep.outer_iterations, worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[Row]) {
    for (n, dt_rel, iters, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} iters={iters} {}", cells.join(" "));
    }
}

/// Diagnostic: print every compared step (`--ignored --nocapture`); `REFS` names the reference files (comma separated),
/// `STEPS` the number of steps.
#[test]
#[ignore]
fn psbt_closed_dump_rows() {
    let refs = std::env::var("REFS").unwrap_or_else(|_| "psbt_closed_head".into());
    let refs: Vec<&str> = refs.split(',').collect();
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    print_rows(&run_rows("dump", &refs, n));
}

/// Noise-floor control (round-3 technique): the port against ITSELF with the initial liquid temperature scaled by
/// `1 + eps`, a disturbance of the size port-vs-upstream shows in the single-phase steps (1e-12).
fn noise_rows(eps: f64, n_steps: usize) -> Vec<(usize, Vec<(&'static str, f64)>)> {
    let dir = protocol_copy("noise");
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

/// Diagnostic: print the noise-floor control (`EPS`, default 1e-12; `STEPS`, default 40).
#[test]
#[ignore]
fn psbt_closed_noise_floor_dump() {
    let eps: f64 = std::env::var("EPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-12);
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    for (n, w) in noise_rows(eps, n) {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("NOISE step {n:4} {}", cells.join(" "));
    }
}

/// Diagnostic: the TRACE branch of the port's cells after `STEP` steps (default 9): `Twall`, `T_sat`, `T_ONB` (Basu) and the
/// htc the port stored, next to upstream's written `htc.liquid.structure`.
#[test]
#[ignore]
fn psbt_closed_branch_dump() {
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::chf::OnsetOfNucleateBoilingTemperature as Onb;
    use uom::si::f64::{Angle, HeatTransfer, MassDensity, SurfaceTension, ThermalConductivity, ThermodynamicTemperature};
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(9);
    let r = load_reference("psbt_closed_head");
    let dir = protocol_copy("branch");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    for _ in 1..=step {
        s.step().expect("step");
    }
    let tname = &r.times[step - r.first];
    let up = &r.fields[&(tname.clone(), "htc.liquid.structure".to_string())];
    let onb = Onb::Basu {
        surface_tension: SurfaceTension::new::<uom::si::surface_tension::newton_per_meter>(8e-3),
        contact_angle: Angle::new::<uom::si::angle::radian>(38.0 * 2.0 * std::f64::consts::PI / 360.0),
    };
    for c in [5usize, 10, 20, 40, 60, 74] {
        let tw = s.structure.t_wall[c];
        let tf = s.fluids[0].t.c[c];
        let tsat = s.t_interface[c];
        let t_onb = onb
            .temperature(
                HeatTransfer::new::<uom::si::heat_transfer::watt_per_square_meter_kelvin>(s.htc_fs[0][c]),
                ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(tf),
                ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(tsat),
                MassDensity::new::<uom::si::mass_density::kilogram_per_cubic_meter>(s.fluids[1].rho.c[c]),
                uom::si::f64::AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(s.latent_heat[c].abs()),
                ThermalConductivity::new::<uom::si::thermal_conductivity::watt_per_meter_kelvin>(s.fluids[0].kappa.c[c]),
            )
            .get::<uom::si::thermodynamic_temperature::kelvin>();
        {
            use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace::{
                multi_regime_boiling_trace, TraceBoilingInputs,
            };
            use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::PoolBoilingHtc;
            let pool_model = PoolBoilingHtc::Gorenflo { surface_roughness: uom::si::f64::Length::new::<uom::si::length::meter>(4e-7) };
            let p = s.p.c[c];
            let pool = |twall: f64| {
                pool_model
                    .heat_transfer_coefficient_from_delta_t(
                        uom::si::f64::TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(twall - tsat),
                        uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(p),
                    )
                    .get::<uom::si::heat_transfer::watt_per_square_meter_kelvin>()
            };
            let inp = TraceBoilingInputs {
                alpha_other: 0.0,
                t_wall: tw,
                t_fluid: tf,
                t_sat: tsat,
                htc_2p_fc: s.htc_fs[0][c], // approximately the FC value: the NB part is far smaller
                htc_film_condensation: None,
                has_onb_model: true,
            };
            let h = |tonb: f64, tsat_shift: f64| {
                let mut i2 = inp;
                i2.t_sat += tsat_shift;
                multi_regime_boiling_trace(&i2, |_| tonb, pool, None::<fn(f64) -> f64>).htc
            };
            let h0 = h(t_onb, 0.0);
            let dh_donb = (h(t_onb + 1e-3, 0.0) - h0) / 1e-3 / h0;
            let dh_dsat = (h(t_onb, 1e-3) - h0) / 1e-3 / h0;
            eprintln!(
                "SENS cell {c:3} htc(model of port inputs)={h0:.12e} (1/htc)dhtc/dTONB={dh_donb:.3e} /K  (1/htc)dhtc/dTsat={dh_dsat:.3e} /K  q_FC={:.3e}",
                s.htc_fs[0][c] * (tw - tf)
            );
        }
        eprintln!(
            "BRANCH cell {c:3} Twall={tw:.9} Tsat={tsat:.9} Tf={tf:.9} TONB(of port htc)={t_onb:.9} Tw-Tsat={:.6} Tw-TONB={:.6} htc port={:.12e} up={:.12e} rel={:.2e}",
            tw - tsat,
            tw - t_onb,
            s.htc_fs[0][c],
            up[c],
            (s.htc_fs[0][c] - up[c]).abs() / up[c]
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 13 diagnostic: every evaluation of `multiRegimeBoilingTRACE` (all cells, all outer iterations) of the first `STEPS`
/// steps (default 14), with its intermediates at 17 digits, written to the file `PROBE_OUT` (one line per cell evaluation:
/// `PORT t= corr= cell= name=value ...`), for comparison with the instrumented upstream's dump
/// (`upstream_driver/probe_boiling`). Gate part: the recorded `ret` equals the stored htc of the same step.
#[test]
#[ignore]
fn psbt_closed_trace_probe_dump() {
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(14);
    let dir = protocol_copy("probe");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    s.trace_probe = Some(Vec::new());
    for _ in 1..=n {
        s.step().expect("step");
    }
    let mut out = String::new();
    for r in s.trace_probe.as_ref().expect("probe on") {
        let tag = match r.kind {
            "structure" => "PORTS",
            "eqn" => "PORTE",
            "eqnf" => "PORTEF",
            "pc" => "PORT_PC",
            "S0" => "PORT_S0",
            "S1" => "PORT_S1",
            "S2" => "PORT_S2",
            "S3" => "PORT_S3",
            _ => "PORT",
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

/// Diagnostic: the port's per-outer-iteration residuals (p_rgh initial, h.liquid initial) of step `STEP`
/// (default 1), as upstream prints them ("abs tol") at the start of the next iteration.
#[test]
#[ignore]
fn psbt_closed_pimple_trace() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let dir = protocol_copy("trace");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    for _ in 1..=step {
        s.step().expect("step");
    }
    for (i, (p, h)) in s.pimple_trace.iter().enumerate() {
        eprintln!("TRACE iteration {} p_rgh={p:.17e} h.liquid={h:.17e}", i + 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic: the port's state after `k` outer iterations of step 1 (`controls.n_outer = k`), as the (avg min max)
/// lines upstream's log prints after each outer iteration (`T.interface`, `T.liquid`, `T.vapour`).
#[test]
#[ignore]
fn psbt_closed_outer_iteration_states() {
    let kmax: usize = std::env::var("KMAX").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    let stats = |v: &[f64]| {
        let avg = v.iter().sum::<f64>() / v.len() as f64;
        let min = v.iter().fold(f64::MAX, |m, x| m.min(*x));
        let max = v.iter().fold(f64::MIN, |m, x| m.max(*x));
        format!("{avg:.17} {min:.17} {max:.17}")
    };
    for k in 1..=kmax {
        let dir = protocol_copy("iters");
        let mut case = TwoPhaseCase::read(&dir).expect("read");
        case.controls.n_outer = k;
        let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
        s.step().expect("step");
        eprintln!("ITER {k} T.interface {}", stats(&s.t_interface));
        eprintln!("ITER {k} T.liquid {}", stats(&s.fluids[0].t.c));
        eprintln!("ITER {k} T.vapour {}", stats(&s.fluids[1].t.c));
        eprintln!("ITER {k} dmdt {}", stats(&s.dmdt));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Diagnostic: per-cell port vs upstream for `STEP` (default 2) and the fields in `FIELDS`.
#[test]
#[ignore]
fn psbt_closed_cells_dump() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let fields = std::env::var("FIELDS").unwrap_or_else(|_| "htc.liquid.structure,T.fixedPower".into());
    let r = load_reference("psbt_closed_head");
    let dir = protocol_copy("cells");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    for _ in 1..=step {
        s.step().expect("step");
    }
    let tname = &r.times[step - r.first];
    for (name, v) in port_fields(&s) {
        if !fields.split(',').any(|f| f == name) {
            continue;
        }
        let u = &r.fields[&(tname.clone(), name.to_string())];
        for c in 0..v.len() {
            eprintln!("CELL {name} {c:3} port={:.17e} up={:.17e} rel={:.2e}", v[c], u[c], (v[c] - u[c]).abs() / u[c].abs().max(1e-300));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------------------
// The gates (measured 2026-11-26, release, against the installed binary's fresh run; README "Round 8").
// ---------------------------------------------------------------------------------------------------------------

const REFS: [&str; 1] = ["psbt_closed_head"];

fn gate_rows() -> Vec<Row> {
    run_rows("gate", &REFS, 20)
}

/// Assert `bound(field)` on every compared field of the rows of steps `lo..=hi`.
fn assert_span(rows: &[Row], lo: usize, hi: usize, bound: &dyn Fn(&str) -> f64) {
    let mut seen = 0;
    for (n, dt_rel, _, w) in rows {
        if *n < lo || *n > hi {
            continue;
        }
        seen += 1;
        assert!(*dt_rel <= 1e-3, "step {n}: the step size differs from upstream's by {dt_rel:.3e}");
        for (k, v) in w {
            assert!(*v <= bound(k), "step {n}: {k} differs from upstream by {v:.3e}, bound {:.1e}", bound(k));
        }
    }
    assert_eq!(seen, hi - lo + 1, "every step of {lo}..={hi} must be compared");
}

/// T1: the closed PSBT case is read with no deviation (the reader accepts `multiRegimeBoilingTRACE`, `twoParameters`,
/// `byRegime` diameter, area and dispersion, `LockhartMartinelli`, `waterTRACE`).
#[test]
fn the_closed_psbt_case_is_read_without_deviations() {
    let dir = protocol_copy("read");
    let case = TwoPhaseCase::read(&dir).expect("read");
    assert!(case.deviations.is_empty(), "{:?}", case.deviations);
    let _ = std::fs::remove_dir_all(&dir);
}

/// T2, T3, T4, T5 (single phase, wall below the onset of nucleate boiling): steps 1-8 agree with upstream to
/// the round-off floor of the two p_rgh solvers (measured: p 7.9e-12, T.liquid 2.0e-11, T.fixedPower 4.2e-12,
/// U.liquid 4.7e-10, U.vapour 9.5e-9, htc 2.1e-10, alphaRhoMagU 4.7e-10); the step sizes agree to 1e-12.
/// Step 2 is SKIPPED by both codes (upstream's `customPimpleControl` leaves `converged_` set when its criteria
/// are met in the last outer iteration: "PIMPLE: converged in 0 iterations"): zero outer iterations in the port too.
#[test]
fn single_phase_steps_agree_and_step_two_is_skipped_as_upstream() {
    let rows = gate_rows();
    assert_span(&rows, 1, 8, &|k| match k {
        "U.vapour" => 1e-7,
        "U.liquid" | "alphaRhoMagU.liquid" => 1e-8,
        "htc.liquid.structure" => 1e-8,
        "alpha.vapour" | "dmdt.liquid.vapour" | "areaDensity.interface" => 0.0,
        _ => 1e-9,
    });
    // upstream's outer-iteration counts of steps 1-3, from its log: 12, 0 (skipped), 11
    assert_eq!(rows[0].2, 12);
    assert_eq!(rows[1].2, 0);
    assert_eq!(rows[2].2, 11);
    for (n, dt_rel, _, _) in &rows {
        assert!(*dt_rel <= 1e-12 || *n > 14, "step {n}: step size");
    }
}

/// Round 13 (README "Round 13"): once the wall passes the onset of nucleate boiling (step 9) the TRACE htc now agrees with
/// upstream to the noise floor of the two `p_rgh` solvers. Round 8 had 7.8e-9 at step 9 and 6.2e-7 at step 12 (`htc`),
/// `U.liquid` 1.7e-7 at step 13; the causes were the vapour gas constant (9e-7 in the vapour density the Basu onset model
/// reads) and `species::thermo::T`'s one-iterate stopping rule (3e-6 K in `T` at every iteration). Measured 2026-11-26
/// (release), steps 9-13: p 2.1e-13 to 9.0e-13, T.liquid 2.3e-11, T.fixedPower 5.5e-11, U.liquid <= 1.2e-10,
/// htc 1.0e-8 at step 13 (the noise-floor control of round 8 gave 1.1e-8 at step 13 for a 1e-12 disturbance).
#[test]
fn nucleate_boiling_branch_agrees_with_upstream_through_step_13() {
    let rows = gate_rows();
    assert_span(&rows, 9, 13, &|k| match k {
        "htc.liquid.structure" => 5e-8,
        "U.liquid" | "U.vapour" | "alphaRhoMagU.liquid" => 1e-9,
        "T.fixedPower" | "T.liquid" | "T.vapour" | "T.interface" => 1e-9,
        "p" | "p_rgh" => 1e-10,
        "alpha.vapour" | "dmdt.liquid.vapour" | "areaDensity.interface" => 0.0,
        _ => 1e-9,
    });
}

/// T6-T8, round 13: the vapour appears at step 14 in both codes (both have alpha exactly 0 through step 13) and the onset
/// step and the 26 steps after it agree with upstream (measured, steps 14-40: alpha 3.5e-8, dmdt 8.5e-8, areaDensity 3.5e-8,
/// U.vapour 1.4e-8, U.liquid 4.7e-9, htc 5.9e-8, p 1.1e-9, T.liquid 3.2e-11). Round 8 had alpha 2.0e-3 and p 5.3e-7 at step 14;
/// the cause was `phaseChangeModel::correct`'s interfacial flux ratio `dmdtIi/dmdtIi0` (the port divided the total `dmdt`).
#[test]
fn vapour_onset_and_the_steps_after_it_agree_with_upstream_to_step_40() {
    let rows = run_rows("gate40", &REFS, 40);
    let first = rows.iter().find(|r| r.3.iter().any(|(k, v)| *k == "alpha.vapour" && *v > 0.0)).map(|r| r.0);
    assert_eq!(first, Some(14), "the first step at which the port's alpha.vapour differs from upstream's (both 0 before)");
    assert_span(&rows, 14, 40, &|k| match k {
        "alpha.vapour" | "dmdt.liquid.vapour" | "areaDensity.interface" => 1e-6,
        "U.vapour" | "U.liquid" | "alphaRhoMagU.liquid" => 1e-7,
        "htc.liquid.structure" => 5e-7,
        "p" | "p_rgh" => 1e-8,
        "T.liquid" | "T.fixedPower" | "T.vapour" | "T.interface" => 1e-9,
        _ => 1e-8,
    });
}

/// The four round-13 translations are each a named, deprecated option that restores the earlier form: with all four back, the
/// onset step is where round 8 found it (p 5.3e-7, alpha.vapour 2.0e-3 at step 14), and `T` from `he` differs from the converged
/// root (`PhaseThermo::t_from_he` against `t_from_he_converged`), the vapour density from the CODATA-2019 one by 8.96e-7.
#[test]
fn the_earlier_forms_stay_selectable_and_reproduce_the_round_8_onset() {
    use outram_foam_basic_lib::prelude::GasConstant;
    let all: Vec<String> = ["gas", "newton", "markers", "flux"].iter().map(|x| x.to_string()).collect();
    let rows = run_rows_with("earlier", &REFS, 14, &all);
    let step14 = &rows[13];
    let get = |name: &str| step14.3.iter().find(|(k, _)| *k == name).expect("field").1;
    assert!(get("p") > 1e-7, "p at the onset step with the earlier forms: {}", get("p"));
    assert!(get("alpha.vapour") > 1e-4, "alpha.vapour at the onset step with the earlier forms: {}", get("alpha.vapour"));

    let dir = protocol_copy("forms");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let vapour = case.fluids[1].thermo.clone();
    let codata = case.clone().with_codata2019_gas_constant().fluids[1].thermo.clone();
    let rho = |t: &outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::PhaseThermo| t.props(1.47e7, 613.0).rho;
    let ratio = rho(&codata) / rho(&vapour);
    assert!((ratio - 1.0 - 8.96e-7).abs() < 2e-8, "CODATA-2019 / ESI vapour density ratio {ratio:.12}");
    let esi_only = vapour.clone().with_gas_constant(GasConstant::EsiV2506);
    assert_eq!(rho(&esi_only), rho(&vapour), "the default vapour package already has the ESI gas constant");

    // the liquid package: one Newton iterate against the converged root
    let liquid = &case.fluids[0].thermo;
    let (p, t_true) = (1.47e7, 598.0);
    let he = liquid.he(p, t_true);
    let converged = liquid.t_from_he_converged(he, p, t_true - 0.05).expect("converged");
    let esi = liquid.t_from_he(he, p, t_true - 0.05).expect("esi");
    assert!((converged - t_true).abs() < 1e-8, "converged root {converged}");
    assert!((esi - t_true).abs() > 1e-8 && (esi - t_true).abs() < 1e-3, "one Newton iterate: error {}", (esi - t_true).abs());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `fluid::correctThermoResidualMarkers`: `pos(x)` is 1 for `x >= 0` and `neg0(x)` 1 for `x <= 0`, set at the START of an
/// outer iteration from the fractions then. At step 1 the vapour fraction is 0 (below its `thermoResidualAlpha` 1e-2) and the
/// liquid's is 1 (above 1e-6).
#[test]
fn thermo_residual_markers_follow_upstream_pos_and_neg0() {
    let dir = protocol_copy("markers");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    s.step().expect("step");
    assert!(s.fluids[0].above_thermo_residual_alpha.iter().all(|v| *v == 1.0));
    assert!(s.fluids[0].below_thermo_residual_alpha.iter().all(|v| *v == 0.0));
    assert!(s.fluids[1].above_thermo_residual_alpha.iter().all(|v| *v == 0.0));
    assert!(s.fluids[1].below_thermo_residual_alpha.iter().all(|v| *v == 1.0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The diagnostic probe records every evaluation of `multiRegimeBoilingTRACE` and changes nothing: two steps with and without it
/// give bit-identical fields, and the records carry the boiling-branch intermediates (`TONB`, `hPB`, `ret`) once the wall is
/// above the onset (not yet in two steps, so only the inputs and `ret` are required here).
#[test]
fn the_trace_probe_records_without_changing_the_result() {
    let run = |probe: bool| {
        let dir = protocol_copy(if probe { "probe_on" } else { "probe_off" });
        let case = TwoPhaseCase::read(&dir).expect("read");
        let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
        if probe {
            s.trace_probe = Some(Vec::new());
        }
        for _ in 0..2 {
            s.step().expect("step");
        }
        let _ = std::fs::remove_dir_all(&dir);
        s
    };
    let (a, b) = (run(false), run(true));
    assert_eq!(a.htc_fs[0], b.htc_fs[0]);
    assert_eq!(a.fluids[0].t.c, b.fluids[0].t.c);
    let rec = b.trace_probe.as_ref().expect("probe on");
    assert!(rec.iter().any(|r| r.kind == "trace" && r.values.iter().any(|(k, _)| *k == "ret")));
    assert!(rec.iter().any(|r| r.kind == "structure"));
    assert!(rec.iter().any(|r| r.kind == "eqn") && rec.iter().any(|r| r.kind == "eqnf"));
    assert!(rec.iter().any(|r| r.kind == "S0") && rec.iter().any(|r| r.kind == "S3"));
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
