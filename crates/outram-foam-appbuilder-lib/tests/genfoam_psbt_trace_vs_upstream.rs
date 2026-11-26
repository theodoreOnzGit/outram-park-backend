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
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let dir = protocol_copy(test);
    let case = TwoPhaseCase::read(&dir).expect("read the closed PSBT case");
    // upstream's customPimpleControl defect is the specification (as in rounds 3-7)
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
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
    let mut a = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    let mut b = TwoPhaseSolver::new(pert).with_upstream_stale_pimple_converged_flag();
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
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
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

/// Diagnostic: the port's per-outer-iteration residuals (p_rgh initial, h.liquid initial) of step `STEP`
/// (default 1), as upstream prints them ("abs tol") at the start of the next iteration.
#[test]
#[ignore]
fn psbt_closed_pimple_trace() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let dir = protocol_copy("trace");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
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
        let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
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
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
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

/// T5 missed in kind (README "Round 8", results): once the wall passes the onset of nucleate boiling (step 9) the
/// TRACE htc leaves round-off agreement (7.8e-9 at step 9, 6.2e-7 at step 12) and the structure temperature follows,
/// ~100x the noise-floor control. This gate pins the MEASURED size so that it can only move for a stated reason;
/// the cause is NOT identified (the suspects are listed in the README).
#[test]
fn nucleate_boiling_branch_differs_from_upstream_by_the_pinned_amount() {
    let rows = gate_rows();
    assert_span(&rows, 9, 13, &|k| match k {
        "htc.liquid.structure" => 5e-6,
        "U.liquid" | "U.vapour" | "alphaRhoMagU.liquid" => 1e-6,
        "T.fixedPower" => 5e-8,
        "T.liquid" => 1e-7,
        "p" | "p_rgh" => 1e-8,
        "alpha.vapour" | "dmdt.liquid.vapour" | "areaDensity.interface" => 0.0,
        _ => 1e-8,
    });
}

/// T6, T7, T8: the vapour appears at step 14 in both codes (both have alpha exactly 0 through step 13); in the onset
/// window the vapour fields differ by up to 2e-3 (relative to the field maximum; the prediction was 1e-4: MISSED).
#[test]
fn vapour_onset_is_at_the_same_step_and_in_the_pinned_window() {
    let rows = gate_rows();
    let first = rows.iter().find(|r| r.3.iter().any(|(k, v)| *k == "alpha.vapour" && *v > 0.0)).map(|r| r.0);
    assert_eq!(first, Some(14), "the first step at which the port's alpha.vapour differs from upstream's (both 0 before)");
    assert_span(&rows, 14, 20, &|k| match k {
        "alpha.vapour" => 1e-2,
        "dmdt.liquid.vapour" | "areaDensity.interface" => 5e-3,
        "U.vapour" => 5e-3,
        "U.liquid" => 1e-3,
        "htc.liquid.structure" => 5e-3,
        "T.liquid" | "T.fixedPower" | "p" | "p_rgh" | "T.vapour" | "T.interface" => 1e-4,
        _ => 1e-2,
    });
}
