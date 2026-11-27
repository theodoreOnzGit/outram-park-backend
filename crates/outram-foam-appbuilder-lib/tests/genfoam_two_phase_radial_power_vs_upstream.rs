// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The four radial power models WIRED into the GeN-Foam `twoPhase` region, vs the real binary (gh:#1884, round 41)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions and criteria (committed BEFORE any run) and results: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`,
//! "Round 41". The cases are overlays (`cases/boiling_radial_r41/<variant>`, made by `scripts/make_boiling_radial_variants.sh`) over the vendored
//! shipped `1D_boiling`, with the round-7 protocol (`writePrecision 17`, every step written, every linear tolerance 1e-14, `relTol 0`). The references
//! are runs of the installed unmodified upstream binary (`refs/boiling_radial_r41_<variant>_<lo>_<hi>.ref.txt.gz`, a contiguous window of steps each).
//!
//! Comparison criteria (README "Protocol and comparison criteria"): per field `max |port - up| / max |up|` over the compared cells; flow gates 1e-11 (`p`,
//! `p_rgh`), 1e-12 (`T.*`), 1e-10 (the rest), pin fields 1e-10, `heatFlux.structure` on the model cells 1e-9; steps are gated up to `S - 1`, `S` being the
//! first step at which the port against ITSELF with `T.liquid x (1 + 1e-9)` leaves 1e-6 in a flow field.

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{TwoPhaseCase, TwoPhaseSolver};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;

const MID: std::ops::Range<usize> = 10..30;

struct Reference {
    dt: Vec<f64>,
    fields: HashMap<(String, String), Vec<f64>>,
    times: Vec<String>,
    first: usize,
}

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn load_reference(name: &str) -> Reference {
    let path = vv_dir().join("refs").join(format!("{name}.ref.txt.gz"));
    let bytes = std::fs::read(&path).expect("reference present (git submodule update --init refs)");
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

/// The vendored case, the variant's overlay files (`""` = the shipped case), then the round-7 protocol (`boiling_mules_protocol.sh`: EVERY_STEP, TIGHT).
fn variant_copy(variant: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1884_{variant}_{}_{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling"), &dst);
    if !variant.is_empty() {
        copy_dir(&vv_dir().join("cases/boiling_radial_r41").join(variant), &dst);
    }
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

/// `max |a - b| / max |b|` (an exactly-zero reference compares absolutely).
fn worst_rel(port: &[f64], up: &[f64]) -> f64 {
    // a face field written `uniform v` is expanded by the extractor to the cell count (one more than the internal faces)
    let up = if up.len() == port.len() + 1 && up.iter().all(|x| *x == up[0]) { &up[..port.len()] } else { up };
    assert_eq!(port.len(), up.len(), "reference field has the wrong length");
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter().zip(up).map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale }).fold(0.0, f64::max)
}

/// The 40-cell array of a pin scalar field: `f(c)` on the model's cells, 0 elsewhere (upstream's zero-initialised `zeroGradient` field).
fn fill<F: Fn(usize) -> f64>(n: usize, cells: &[usize], f: F) -> Vec<f64> {
    let mut v = vec![0.0; n];
    for &c in cells {
        v[c] = f(c);
    }
    v
}

fn flow_fields(s: &TwoPhaseSolver) -> Vec<(String, Vec<f64>)> {
    let ni = s.geom.n_internal();
    let v: Vec<(String, Vec<f64>)> = vec![
        ("p".into(), s.p.c.clone()),
        ("p_rgh".into(), s.p_rgh.c.clone()),
        ("T.liquid".into(), s.fluids[0].t.c.clone()),
        ("T.vapour".into(), s.fluids[1].t.c.clone()),
        ("T.interface".into(), s.t_interface.clone()),
        ("U.liquid".into(), s.fluids[0].u.c.iter().map(|u| u.x).collect()),
        ("U.vapour".into(), s.fluids[1].u.c.iter().map(|u| u.x).collect()),
        ("alpha.vapour".into(), s.fluids[1].alpha.c.clone()),
        ("alpha.liquid".into(), s.fluids[0].alpha.c.clone()),
        ("dmdt.liquid.vapour".into(), s.dmdt.clone()),
        ("alphaPhi.vapour".into(), s.fluids[1].alpha_phi[..ni].to_vec()),
        ("alphaPhi.liquid".into(), s.fluids[0].alpha_phi[..ni].to_vec()),
        ("phi.vapour".into(), s.fluids[1].phi[..ni].to_vec()),
        ("phi.liquid".into(), s.fluids[0].phi[..ni].to_vec()),
    ];
    v
}

/// The written fields of the radial model(s) of the case, with the names upstream writes them under; profiles flattened over the `mid` cells.
fn pin_fields(s: &TwoPhaseSolver) -> Vec<(String, Vec<f64>)> {
    let n = s.geom.n_cells();
    let mut out: Vec<(String, Vec<f64>)> = Vec::new();
    if s.structure.radial.is_empty() {
        return out;
    }
    let (fuel, clad) = s.fuel_clad_temperatures();
    out.push(("T.fuelAvForNeutronics".into(), fuel));
    out.push(("T.cladAvForNeutronics".into(), clad));
    out.push(("heatFlux.structure".into(), MID.map(|c| s.structure.heat_flux[c]).collect()));
    for m in &s.structure.radial {
        let cells = m.cells();
        match m {
            RadialPowerModel::HeatedPin(h) => {
                out.push(("Ti.heatedPin".into(), fill(n, cells, |c| h.ti(c))));
                out.push(("To.heatedPin".into(), fill(n, cells, |c| h.to(c))));
                out.push(("Tav.heatedPin".into(), fill(n, cells, |c| h.tav(c))));
                out.push(("Trad.heatedPin".into(), MID.flat_map(|c| h.trad(c).to_vec()).collect()));
            }
            RadialPowerModel::NuclearFuelPin(p) => {
                out.push(("Tfi.nuclearFuelPin".into(), fill(n, cells, |c| p.boundary_temperatures(c).0)));
                out.push(("Tfo.nuclearFuelPin".into(), fill(n, cells, |c| p.boundary_temperatures(c).1)));
                out.push(("Tci.nuclearFuelPin".into(), fill(n, cells, |c| p.boundary_temperatures(c).2)));
                out.push(("Tco.nuclearFuelPin".into(), fill(n, cells, |c| p.boundary_temperatures(c).3)));
                out.push(("Tfav.nuclearFuelPin".into(), fill(n, cells, |c| p.averages(c).0)));
                out.push(("Tcav.nuclearFuelPin".into(), fill(n, cells, |c| p.averages(c).1)));
                out.push(("Trad.nuclearFuelPin".into(), MID.flat_map(|c| p.trad(c).to_vec()).collect()));
            }
            RadialPowerModel::LumpedNuclearStructure(l) => {
                out.push(("Tsurface.lumpedNuclearStructure".into(), fill(n, cells, |c| l.surface_temperature(c))));
                out.push(("Tmax.lumpedNuclearStructure".into(), fill(n, cells, |c| l.tmax(c))));
                out.push(("Tmatrix.lumpedNuclearStructure".into(), l.tmatrix().to_vec()));
                out.push(("T.lumpedNuclearStructure".into(), MID.flat_map(|c| l.nodes(c).to_vec()).collect()));
            }
            RadialPowerModel::InterpolatedNuclearFuelPin(_) => {}
        }
    }
    out
}

fn bound(field: &str) -> f64 {
    match field {
        "p" | "p_rgh" => 1e-11,
        "T.liquid" | "T.vapour" | "T.interface" => 1e-12,
        "heatFlux.structure" => 1e-9,
        f if f.contains("heatedPin") || f.contains("nuclearFuelPin") || f.contains("lumpedNuclearStructure") || f.contains("ForNeutronics") => 1e-10,
        _ => 1e-10,
    }
}

const FLOW: [&str; 7] = ["p", "p_rgh", "U.liquid", "U.vapour", "T.liquid", "T.vapour", "T.interface"];

/// (step, relative step-size difference, onset flags agree, worst relative difference per field).
type Row = (usize, f64, bool, Vec<(String, f64)>);

/// Ways to break the port, for the controls.
#[derive(Clone, Copy, Default)]
struct Ablation {
    pre_solve_ht: bool,
    no_laplacian: bool,
    freeze_tact: bool,
    no_ia: bool,
    corrected_rings: bool,
    never_power_off: bool,
}

fn build(case_variant: &str, ab: Ablation) -> (TwoPhaseSolver, PathBuf) {
    let dir = variant_copy(case_variant);
    let mut case = TwoPhaseCase::read_with_radial_ring_areas(&dir, ab.corrected_rings).expect("read the variant");
    assert!(case.deviations.is_empty(), "MULES runs as selected: {:?}", case.deviations);
    if ab.never_power_off {
        case.structure.power_off = None;
    }
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    if ab.pre_solve_ht {
        s = s.with_pre_solve_temperature_for_radial_ht();
    }
    if ab.no_laplacian {
        s = s.without_radial_lumped_laplacian();
    }
    if ab.freeze_tact {
        s = s.without_radial_tact_hand_back();
    }
    if ab.no_ia {
        s = s.without_radial_interfacial_area();
    }
    (s, dir)
}

fn run_rows(case_variant: &str, refs: &[&str], n_steps: usize, ab: Ablation) -> Vec<Row> {
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let (mut s, dir) = build(case_variant, ab);
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = match s.step() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("DIVERGED at step {n}: {e:?}");
                break;
            }
        };
        let Some(r) = refs.iter().find(|r| n >= r.first && n - r.first < r.times.len()) else { continue };
        let dt_up = if n == 1 { 1e-3 } else { r.dt[n - 2] };
        let dt_rel = (rep.dt - dt_up).abs() / dt_up;
        let tname = &r.times[n - r.first];
        let mut worst = Vec::new();
        let mut port_max_alpha = 0.0_f64;
        for (name, v) in flow_fields(&s).into_iter().chain(pin_fields(&s)) {
            if name == "alpha.vapour" {
                port_max_alpha = v.iter().fold(0.0_f64, |m, x| m.max(*x));
            }
            if let Some(u) = r.fields.get(&(tname.clone(), name.clone())) {
                let u: &[f64] = if name == "heatFlux.structure" { &u[MID] } else { u };
                worst.push((name, worst_rel(&v, u)));
            }
        }
        let up_max_alpha = r.fields[&(tname.clone(), "alpha.vapour".to_string())].iter().fold(0.0_f64, |m, x| m.max(*x));
        rows.push((n, dt_rel, (port_max_alpha > 1e-9) == (up_max_alpha > 1e-9), worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[Row]) {
    for (n, dt_rel, onset_ok, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} onset_ok={onset_ok} {}", cells.join(" "));
    }
}

/// The port against ITSELF with the initial liquid temperature x (1 + 1e-9): the first step at which a flow field leaves 1e-6 (`None`: never in `n_steps`).
fn noise_first(variant: &str, n_steps: usize) -> Option<usize> {
    let (a, dir) = build(variant, Ablation::default());
    let mut pert_case = TwoPhaseCase::read(&dir).expect("read");
    for v in pert_case.fluids[0].t.c.iter_mut().chain(pert_case.fluids[0].t.b.iter_mut()) {
        *v *= 1.0 + 1e-9;
    }
    let mut a = a;
    let mut b = TwoPhaseSolver::new(pert_case).with_upstream_stale_pimple_converged_flag();
    let mut first = None;
    for n in 1..=n_steps {
        if a.step().is_err() || b.step().is_err() {
            first = Some(n);
            break;
        }
        let fa = flow_fields(&a);
        let fb = flow_fields(&b);
        if fa.iter().zip(&fb).any(|((k, x), (_, y))| FLOW.contains(&k.as_str()) && worst_rel(y, x) > 1e-6) {
            first = Some(n);
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    first
}

/// First step at which a flow field leaves 1e-6 from upstream's (None: never in the rows).
fn first_flow_divergence(rows: &[Row]) -> Option<usize> {
    rows.iter().find(|r| r.3.iter().any(|(k, v)| FLOW.contains(&k.as_str()) && *v > 1e-6)).map(|r| r.0)
}

/// Every row with `n < gate_end` must have upstream's step size, the same onset flag, and every field within `bound`. Returns the worst per field.
fn assert_gated(rows: &[Row], gate_end: usize) -> HashMap<String, f64> {
    let mut worst: HashMap<String, f64> = HashMap::new();
    let mut seen = 0;
    for (n, dt_rel, onset_ok, w) in rows {
        if *n >= gate_end {
            continue;
        }
        seen += 1;
        assert!(*dt_rel <= 1e-12, "step {n}: the step size differs from upstream's by {dt_rel:.3e}");
        assert!(*onset_ok, "step {n}: the boiling-onset flag differs from upstream's");
        for (k, v) in w {
            assert!(*v <= bound(k), "step {n}: {k} differs from upstream by {v:.3e}, bound {:.1e}", bound(k));
            let e = worst.entry(k.clone()).or_insert(0.0);
            *e = e.max(*v);
        }
    }
    assert!(seen > 0, "no step was compared");
    worst
}

fn report(name: &str, rows: &[Row], gate_end: usize, worst: &HashMap<String, f64>) {
    let mut v: Vec<_> = worst.iter().collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    let cells: Vec<String> = v.iter().map(|(k, x)| format!("{k}={x:.2e}")).collect();
    eprintln!(
        "{name}: {} steps compared, gated < {gate_end}, first flow divergence {:?}; worst: {}",
        rows.len(),
        first_flow_divergence(rows),
        cells.join(" ")
    );
}

/// Post-run bound (README round 41, the one step before `S` of `hp3`): measured worst 5.3e-9 (`dmdt`), asserted at 1e-8.
const POST_RUN_BOUND: f64 = 1e-8;

const NO_ONSET_WINDOWS: [&str; 4] = ["_1_60", "_200_225", "_400_425", "_600_625"];

fn refs_of(variant: &str, windows: &[&str]) -> Vec<String> {
    windows.iter().map(|w| format!("boiling_radial_r41_{variant}{w}")).collect()
}

/// `post_run_missed`: how many of the LAST pre-registered gated steps (`n < S`) missed the pre-registered gates, and are asserted instead at
/// [`POST_RUN_BOUND`] (marked post-run in the README: they sit inside the exponential amplification that ends at `S`).
fn gated_variant(variant: &str, windows: &[&str], n_steps: usize, post_run_missed: usize) {
    let names = refs_of(variant, windows);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let rows = run_rows(variant, &refs, n_steps, Ablation::default());
    let gate_end = noise_first(variant, n_steps).unwrap_or(usize::MAX);
    let worst = assert_gated(&rows, gate_end - post_run_missed);
    for (n, _, onset_ok, w) in rows.iter().filter(|r| r.0 >= gate_end - post_run_missed && r.0 < gate_end) {
        assert!(*onset_ok, "step {n}: onset flag");
        for (k, v) in w {
            assert!(*v <= POST_RUN_BOUND, "step {n} (post-run bound): {k} differs by {v:.3e}");
        }
    }
    report(variant, &rows, gate_end, &worst);
    assert!(
        worst.keys().any(|k| k.contains("heatedPin") || k.contains("nuclearFuelPin") || k.contains("lumpedNuclearStructure")),
        "the pin fields must be compared (they would silently vanish if the names did not match)"
    );
}

/// Diagnostic: every compared row of variant `VARIANT` against its windows (`--ignored --nocapture`).
#[test]
#[ignore]
fn radial_two_phase_dump_rows() {
    let v = std::env::var("VARIANT").unwrap_or_else(|_| "hp".into());
    let w = std::env::var("WINDOWS").unwrap_or_else(|_| "_1_60".into());
    let windows: Vec<&str> = w.split(',').collect();
    let n: usize = std::env::var("STEPS").ok().and_then(|x| x.parse().ok()).unwrap_or(60);
    let names = refs_of(&v, &windows);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    print_rows(&run_rows(&v, &refs, n, Ablation::default()));
    eprintln!("noise first = {:?}", noise_first(&v, n));
}

/// heatedPin (`hp`, KNS37 numbers, `outerRadius 0.01`: `iA` = the shipped 100 1/m), single phase to t = 12 s (no onset), steps 1-60 and 200-225, 400-425,
/// 600-625, plus 690-740 (an added window: the SHIPPED `powerOffCriterionModel timer 7` switches the pin power off at step ~715).
#[test]
fn heated_pin_in_the_two_phase_region_matches_upstream() {
    gated_variant("hp", &["_1_60", "_200_225", "_400_425", "_600_625", "_690_740"], 740, 0);
}

/// heatedPin with KNS37's own 3 mm radius (`iA` 333), to and through the boiling onset (upstream: first vapour at step 628; its run aborts at t = 9.34 s).
#[test]
fn heated_pin_3mm_matches_upstream_through_the_boiling_onset() {
    gated_variant("hp3", &["_1_60", "_603_653"], 653, 1);
}

/// `powerDensity.heatedPin` read per cell (no `powerDensity` key in the dictionary, a nonuniform field file), as `2D_KNS37-L22` supplies it.
#[test]
fn heated_pin_power_density_field_matches_upstream() {
    gated_variant("hp_field", &NO_ONSET_WINDOWS, 625, 0);
}

/// The shipped `powerOffCriterionModel` timer set to 0.3 s with a heatedPin: `structure::checkPowerOff` -> `heatedPin::powerOff`.
#[test]
fn heated_pin_power_off_matches_upstream() {
    gated_variant("hp_off", &NO_ONSET_WINDOWS, 625, 0);
}

/// nuclearFuelPin (2D_fullCoupling dictionary, `gapH 3000`).
#[test]
fn nuclear_fuel_pin_in_the_two_phase_region_matches_upstream() {
    gated_variant("nfp", &NO_ONSET_WINDOWS, 625, 0);
}

/// nuclearFuelPin with the `gapHPowerDensity` table.
#[test]
fn nuclear_fuel_pin_with_a_gap_table_matches_upstream() {
    gated_variant("nfp_tab", &NO_ONSET_WINDOWS, 625, 0);
}

/// nuclearFuelPin at twice the power density (`nfp_hot`, a case-design choice recorded in the README): to and through the boiling onset
/// (upstream: first vapour at step 600; its run aborts at t = 6.44 s).
#[test]
fn nuclear_fuel_pin_matches_upstream_through_the_boiling_onset() {
    gated_variant("nfp_hot", &["_1_60", "_575_625"], 625, 0);
}

/// lumpedNuclearStructure (1D_thermalMSR dictionary, `kappaMatrix 12`): single phase, exercises `fvc::laplacian(Tmatrix)` on the two-phase mesh.
#[test]
fn lumped_nuclear_structure_in_the_two_phase_region_matches_upstream() {
    gated_variant("lns", &NO_ONSET_WINDOWS, 625, 0);
}

fn worst_of(rows: &[Row], f: impl Fn(&str) -> bool) -> f64 {
    rows.iter().flat_map(|r| r.3.iter()).filter(|(k, _)| f(k)).fold(0.0_f64, |m, (_, v)| m.max(*v))
}

fn pin_name(k: &str) -> bool {
    k.contains("heatedPin") || k.contains("nuclearFuelPin") || k.contains("lumpedNuclearStructure")
}

/// Controls: each deliberately broken ingredient FAILS its comparison, so the gates above can fail.
#[test]
fn the_comparison_can_fail_each_ingredient_of_the_wiring() {
    let hp = ["boiling_radial_r41_hp_1_60"];
    // (a) the shipped fixedPower case against the heatedPin reference: the flow differs
    let shipped = run_rows("", &hp, 60, Ablation::default());
    assert!(worst_of(&shipped, |k| k == "T.liquid") > 1e-9, "{:.2e}", worst_of(&shipped, |k| k == "T.liquid"));
    // (b) HT from the pre-solve temperatures
    let pre = run_rows("hp", &hp, 60, Ablation { pre_solve_ht: true, ..Ablation::default() });
    assert!(worst_of(&pre, pin_name) > 1e-9, "pre-solve HT: {:.2e}", worst_of(&pre, pin_name));
    // (c) iAact without the radial iA
    let no_ia = run_rows("hp", &hp, 60, Ablation { no_ia: true, ..Ablation::default() });
    assert!(worst_of(&no_ia, |k| k == "T.liquid") > 1e-9, "no iA: {:.2e}", worst_of(&no_ia, |k| k == "T.liquid"));
    // (d) Tact never handed back
    let frozen = run_rows("hp", &hp, 60, Ablation { freeze_tact: true, ..Ablation::default() });
    assert!(worst_of(&frozen, |k| k == "T.liquid") > 1e-9, "frozen Tact: {:.2e}", worst_of(&frozen, |k| k == "T.liquid"));
    // (e) corrected ring areas (nuclearFuelPin clad)
    let nfp = ["boiling_radial_r41_nfp_1_60"];
    let rings = run_rows("nfp", &nfp, 60, Ablation { corrected_rings: true, ..Ablation::default() });
    assert!(worst_of(&rings, pin_name) > 1e-9, "ring areas: {:.2e}", worst_of(&rings, pin_name));
    // (f) constant gapH against the table reference
    let tab = run_rows("nfp", &["boiling_radial_r41_nfp_tab_1_60"], 60, Ablation::default());
    assert!(worst_of(&tab, pin_name) > 1e-9, "gap table: {:.2e}", worst_of(&tab, pin_name));
    // P3: powerOff never applied; P4: the uniform dictionary power instead of the field
    let off = run_rows("hp_off", &["boiling_radial_r41_hp_off_200_225"], 225, Ablation { never_power_off: true, ..Ablation::default() });
    assert!(worst_of(&off, pin_name) > 1e-9, "no powerOff: {:.2e}", worst_of(&off, pin_name));
    let uniform = run_rows("hp", &["boiling_radial_r41_hp_field_1_60"], 60, Ablation::default());
    assert!(worst_of(&uniform, pin_name) > 1e-9, "uniform power: {:.2e}", worst_of(&uniform, pin_name));
    // (g) the lumped Laplacian forced to 0
    let lns = run_rows("lns", &["boiling_radial_r41_lns_1_60"], 60, Ablation { no_laplacian: true, ..Ablation::default() });
    assert!(worst_of(&lns, pin_name) > 1e-9, "no Laplacian: {:.2e}", worst_of(&lns, pin_name));
    eprintln!("control (g): lumped Laplacian = 0, pin fields {:.2e}", worst_of(&lns, pin_name));
    eprintln!(
        "controls: shipped fixedPower T.liquid {:.2e}; pre-solve HT pin {:.2e}; no iA T.liquid {:.2e}; frozen Tact T.liquid {:.2e}; corrected rings pin {:.2e}; gap const vs table pin {:.2e}; no powerOff pin {:.2e}; uniform power pin {:.2e}",
        worst_of(&shipped, |k| k == "T.liquid"),
        worst_of(&pre, pin_name),
        worst_of(&no_ia, |k| k == "T.liquid"),
        worst_of(&frozen, |k| k == "T.liquid"),
        worst_of(&rings, pin_name),
        worst_of(&tab, pin_name),
        worst_of(&off, pin_name),
        worst_of(&uniform, pin_name)
    );
}

/// The wiring's own observable facts (no upstream needed): the `mid` zone has the pin's interfacial area (100 1/m for hp, 333 for hp3), the
/// structure's `iAact` carries it, `Tact` starts at the surface temperature of the model, and the hand-over accessors exist.
#[test]
fn the_structure_carries_the_radial_models_interfacial_area_and_initial_surface_temperature() {
    let (s, dir) = build("hp", Ablation::default());
    assert_eq!(s.structure.radial.len(), 1);
    for c in MID {
        assert!((s.structure.ia_act[c] - 100.0).abs() < 1e-9, "iA = alpha 2/ro = 0.5 * 2/0.01");
        assert!((s.structure.t_act[c] - 652.15).abs() < 1e-9, "Tact starts at the pin's T");
    }
    assert_eq!(s.structure.ia_act[0], 0.0);
    let _ = std::fs::remove_dir_all(&dir);
    let (s3, dir3) = build("hp3", Ablation::default());
    assert!((s3.structure.ia_act[15] - 0.5 * 2.0 / 0.003).abs() < 1e-9);
    let _ = std::fs::remove_dir_all(&dir3);
    let (mut sn, dirn) = build("nfp", Ablation::default());
    sn.set_structure_power_density(&vec![1.0e8; 40]);
    assert_eq!(sn.fuel_clad_temperatures().0.len(), 40);
    let _ = std::fs::remove_dir_all(&dirn);
}
