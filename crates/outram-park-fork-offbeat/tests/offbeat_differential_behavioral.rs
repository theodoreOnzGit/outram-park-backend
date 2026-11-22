// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Densification, relocation and swelling vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_behavioral.txt` written by `dump_behavioral.cpp`
//! (verbatim `correct()` bodies of OFFBEAT's `densification*.C`,
//! `relocationFRAPCON.C` (`setRelocation`) and `swelling*.C`; transcription, not
//! a run of the compiled library).  Predictions and derived gates:
//! `PREDICTIONS_round2.md` section B.  Single-step cases (old fields zero) are
//! gated by G1/G2; multi-step swelling cases (MATPRO UO2, FBR-MOX, BISON-Zy
//! growth) by the bounds derived there.
//!
//! ## Results
//!
//! See `verification_and_validation/behavioral_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, Gates, Row, Stats};
use outram_park_fork_offbeat::error::OffbeatError;
use outram_park_fork_offbeat::materials::behavioral::densification::DensificationModel;
use outram_park_fork_offbeat::materials::behavioral::relocation::{FrapconRelocationForm, RelocationModel};
use outram_park_fork_offbeat::materials::behavioral::swelling::{
    BisonZircaloyCladType, SwellingModel, SwellingStrain,
};
use outram_park_fork_offbeat::materials::MaterialState;
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!(
    "../verification_and_validation/offbeat_differential_gh1393/reference_behavioral.txt"
);
const EPS: f64 = f64::EPSILON;

/// What evaluating the port produced.
enum Port {
    Value(f64),
    Skipped,
    Refused(String),
}

fn classify(r: Result<f64, OffbeatError>, unchecked: impl FnOnce() -> f64) -> Port {
    match r {
        Ok(_) => Port::Value(unchecked()),
        Err(OffbeatError::OutOfRange { .. }) => Port::Skipped,
        Err(e) => Port::Refused(e.to_string()),
    }
}

fn strain_comp(model: &SwellingModel, s: &MaterialState, pick: usize) -> Port {
    let pick_of = |st: SwellingStrain| [st.radial, st.hoop, st.axial][pick];
    match model.strain_checked(s) {
        Ok(_) => Port::Value(pick_of(model.strain(s))),
        Err(OffbeatError::OutOfRange { .. }) => Port::Skipped,
        Err(e) => Port::Refused(e.to_string()),
    }
}

fn clad(type_index: f64) -> BisonZircaloyCladType {
    match type_index as i32 {
        0 => BisonZircaloyCladType::Escore,
        1 => BisonZircaloyCladType::Sra,
        2 => BisonZircaloyCladType::Rxa,
        3 => BisonZircaloyCladType::Pra,
        4 => BisonZircaloyCladType::Zirlo,
        _ => BisonZircaloyCladType::M5,
    }
}

/// Port value for the single-step rows.  Columns before `scale` are the inputs.
fn port_single(name: &str, c: &[f64]) -> Option<Port> {
    let mut s = MaterialState::fresh(900.0);
    Some(match name {
        "DensEmpirical" => {
            s.burnup = c[0] / 1000.0 / 0.881;
            let m = DensificationModel::Empirical {
                density_fraction: c[1],
                density_change: c[2],
                saturation_burnup: c[3],
            };
            classify(m.linear_checked(&s), || m.linear(&s))
        }
        "DensFrapcon" => {
            s.temperature = c[0];
            s.burnup = c[1] / 1000.0 / 0.881;
            let m = DensificationModel::Uo2Frapcon {
                density_fraction: c[2],
                resintering_density_change: c[3],
                sintering_temperature: c[4],
            };
            classify(m.linear_checked(&s), || m.linear(&s))
        }
        "Reloc" => {
            s.burnup = c[3] / 1000.0 / 0.881;
            let m = RelocationModel::Uo2Frapcon {
                cold_gap: c[1],
                cold_pellet_diameter: c[2],
                linear_power: c[0],
                form: if c[4] == 1.0 { FrapconRelocationForm::Modified } else { FrapconRelocationForm::Legacy },
            };
            classify(m.value_checked(&s), || m.value(&s))
        }
        "SwConstant" => {
            s.fast_fluence = c[1] * 1e4;
            strain_comp(&SwellingModel::Constant { swelling_rate: c[0] }, &s, 0)
        }
        "SwFrCrAl" => {
            s.fast_fluence = c[1] * 1e4;
            strain_comp(&SwellingModel::FeCrAl { rate: c[0] }, &s, 0)
        }
        "SwFrapcon" => {
            s.burnup = c[0] / 1000.0 / 0.881;
            s.porosity = c[1];
            strain_comp(&SwellingModel::Uo2Frapcon { theoretical_density: 10960.0 }, &s, 0)
        }
        "SwAim1" | "SwGen1515" | "SwWrightSham" => {
            s.temperature = c[0];
            s.fast_fluence = c[1] * 1e4;
            let m = match name {
                "SwAim1" => SwellingModel::GrowthAim11515Ti,
                "SwGen1515" => SwellingModel::GrowthGeneralized1515Ti,
                _ => SwellingModel::WrightShamHastelloyN,
            };
            strain_comp(&m, &s, 0)
        }
        _ => return None,
    })
}

#[derive(Default)]
struct Family {
    stats: Stats,
    gates: Gates,
    skipped: usize,
}

fn run_single(rows: &[Row]) -> (BTreeMap<String, Family>, Vec<String>) {
    let mut fam: BTreeMap<String, Family> = BTreeMap::new();
    let mut refused = Vec::new();
    for r in rows {
        let n = r.col.len();
        let (inputs, scale) = (&r.col[..n - 1], r.col[n - 1]);
        let (inputs, scale) = if r.name == "DensFrapcon" {
            // last input is the upstream iteration count
            assert!(inputs[5] <= 10.0, "upstream needed {} iterations", inputs[5]);
            (&inputs[..5], scale)
        } else {
            (inputs, scale)
        };
        let Some(p) = port_single(&r.name, inputs) else { continue };
        let f = fam.entry(r.name.clone()).or_default();
        match p {
            Port::Skipped => f.skipped += 1,
            Port::Refused(m) => refused.push(format!("{} {:?}: {m}", r.name, inputs)),
            Port::Value(v) => {
                f.stats.add(v, r.reference, &format!("{:?}", inputs));
                f.gates.add(v, r.reference, scale);
            }
        }
    }
    (fam, refused)
}

// ---------------------------------------------------------------- tests

#[test]
fn single_step_models_agree_with_the_transcribed_kernels() {
    let rows = parse_rows(FIXTURE);
    let (fam, refused) = run_single(&rows);
    println!("{:<14} {:>6} {:>7} {:>7} {:>9} {:>9} {:>9}", "model", "n", "skipped", "bit-id", "worst rel", "G1 fail", "G2 fail");
    for (n, f) in &fam {
        println!(
            "{:<14} {:>6} {:>7} {:>7} {:>9.2e} {:>9} {:>9}   (G2 checked {}, worst rel near cancellation {:.2e}, worst G1 ratio {:.3})",
            n, f.stats.compared, f.skipped, f.stats.bit_identical, f.stats.worst_rel, f.gates.g1_fail,
            f.gates.g2_fail, f.gates.g2_checked, f.gates.worst_cond_rel, f.gates.worst_g1_ratio
        );
    }
    println!("refused by the port: {}", refused.len());
    for m in refused.iter().take(10) {
        println!("  {m}");
    }
    assert!(refused.is_empty(), "port refused rows the upstream evaluates");
    for (n, f) in &fam {
        assert!(f.stats.compared > 0, "{n}: nothing compared");
        assert_eq!(f.gates.g1_fail, 0, "{n}: G1");
        assert_eq!(f.gates.g2_fail, 0, "{n}: G2");
    }
}

#[test]
fn correlation_pyc_agrees_radial_and_tangential() {
    let rows = parse_rows(FIXTURE);
    let ar0 = [-1.24080, 0.00175, 0.08533, -0.01253, 0.0, 0.0];
    let at0 = [-1.24080, 0.05307, 0.07620, -0.01245, 0.0, 0.0];
    let ar1 = [0.1, -0.2, 0.3, -0.4, 0.5, -0.6];
    let at1 = [-0.6, 0.5, -0.4, 0.3, -0.2, 0.1];
    let mut gates = Gates::default();
    let mut stats = Stats::default();
    for r in rows.iter().filter(|r| r.name.starts_with("SwPyC")) {
        let (f, fcf, set, scale) = (r.col[0], r.col[1], r.col[2], r.col[3]);
        let (ar, at) = if set == 0.0 { (ar0, at0) } else { (ar1, at1) };
        let m = SwellingModel::PyroCarbonCorrelation {
            radial_coefficients: ar,
            tangential_coefficients: at,
            flux_conversion_factor: fcf,
        };
        let mut s = MaterialState::fresh(900.0);
        s.fast_fluence = f * 1e4;
        let st = m.strain(&s);
        assert_eq!(st.hoop, st.axial, "tangential goes to hoop and axial");
        let v = if r.name == "SwPyC_r" { st.radial } else { st.hoop };
        stats.add(v, r.reference, &format!("{:?}", r.col));
        gates.add(v, r.reference, scale);
    }
    println!(
        "PyC n {} bit-id {} worst rel {:.2e}; G1 fail {}, G2 checked {} fail {}, worst near cancellation {:.2e}",
        stats.compared, stats.bit_identical, stats.worst_rel, gates.g1_fail, gates.g2_checked, gates.g2_fail,
        gates.worst_cond_rel
    );
    assert!(stats.compared > 0);
    assert_eq!(gates.g1_fail, 0);
    assert_eq!(gates.g2_fail, 0);
}

#[test]
fn bison_zy_growth_one_step_agrees_and_n_steps_follow_the_predicted_path_dependence() {
    let rows = parse_rows(FIXTURE);
    let mut gates_n1 = Gates::default();
    let mut compared_n1 = 0;
    let mut worst_pred_ratio = 0.0_f64;
    let mut multi = 0;
    let mut worst_zz_multi = 0.0_f64;
    for r in rows.iter().filter(|r| r.name.starts_with("SwZy")) {
        let (ty, f, n, s_total) = (r.col[0], r.col[1], r.col[2] as usize, r.col[3]);
        let m = SwellingModel::GrowthBisonZircaloy { clad_type: clad(ty) };
        let mut st = MaterialState::fresh(600.0);
        st.fast_fluence = f * 1e4;
        let is_xx = r.name == "SwZy_xx";
        // `strain` clamps to the port's validity window, so rows outside it are skipped.
        let port = match strain_comp(&m, &st, if is_xx { 0 } else { 2 }) {
            Port::Value(v) => v,
            _ => continue,
        };
        if n == 1 {
            compared_n1 += 1;
            // xx = -(1 - (1+d)^-0.5): the "1 - ..." difference has absolute rounding of order eps.
            gates_n1.add(port, r.reference, if is_xx { 1.0 } else { r.reference.abs() * 2.0 });
        } else if is_xx {
            if s_total <= 0.02 {
                multi += 1;
                let rel = (r.reference - port) / port;
                let predicted = 0.75 * s_total * (1.0 - 1.0 / n as f64);
                worst_pred_ratio = worst_pred_ratio.max((rel.abs() - predicted).abs() / predicted);
            }
        } else {
            worst_zz_multi = worst_zz_multi.max(((port - r.reference) / r.reference).abs());
        }
    }
    println!(
        "Zy N=1: n {compared_n1}, G1 fail {}, G2 checked {} fail {}; N>1 transverse: {multi} rows, worst |observed-predicted|/predicted = {worst_pred_ratio:.3}; axial (telescoping) worst rel {worst_zz_multi:.2e}",
        gates_n1.g1_fail, gates_n1.g2_checked, gates_n1.g2_fail
    );
    assert_eq!(gates_n1.g1_fail, 0);
    assert!(multi > 0);
    assert!(worst_pred_ratio <= 0.2, "path dependence misses the 20 % prediction: {worst_pred_ratio}");
    assert!(worst_zz_multi < 72.0 * EPS * 4.0, "axial growth telescopes: {worst_zz_multi:e}");
}

#[test]
fn matpro_uo2_steps_converge_to_the_port_integral_within_the_derived_bound() {
    let rows = parse_rows(FIXTURE);
    // (T, p, Bu) -> list of (N, relative error)
    let mut by_case: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    let mut violations = 0;
    let mut worst_ratio = 0.0_f64;
    for r in rows.iter().filter(|r| r.name == "SwMatpro") {
        let (t, p, bu, n, relbound) = (r.col[0], r.col[1], r.col[2], r.col[3], r.col[4]);
        let mut s = MaterialState::fresh(t);
        s.porosity = p;
        s.burnup = bu / 1000.0 / 0.881;
        let m = SwellingModel::Uo2Matpro { theoretical_density: 10960.0 };
        let port = match strain_comp(&m, &s, 0) {
            Port::Value(v) => v,
            _ => continue,
        };
        let rel = ((r.reference - port) / port).abs();
        let bound = relbound + (n + 8.0) * EPS;
        worst_ratio = worst_ratio.max(rel / bound);
        if rel > bound {
            violations += 1;
        }
        by_case.entry(format!("T={t} p={p} Bu={bu}")).or_default().push((n, rel));
    }
    // order of convergence from N=64 -> N=1024 (the last two decades are > rounding)
    let mut worst_slope_dev = 0.0_f64;
    for (case, v) in &by_case {
        let e = |n: f64| v.iter().find(|x| x.0 == n).map(|x| x.1).unwrap();
        let slope = (e(1024.0) / e(64.0)).ln() / (1024.0_f64 / 64.0).ln();
        worst_slope_dev = worst_slope_dev.max((slope + 1.0).abs());
        if worst_slope_dev == (slope + 1.0).abs() {
            println!("  worst slope so far {slope:.4} at {case}");
        }
    }
    println!(
        "MATPRO UO2 stepping: {} cases, bound violations {violations}, worst error/bound {worst_ratio:.3}, worst |slope+1| over N 64..1024 = {worst_slope_dev:.4}",
        by_case.len()
    );
    assert_eq!(violations, 0);
    assert!(worst_slope_dev < 0.05, "first-order convergence expected");
}

#[test]
fn fbr_mox_steps_stay_within_the_straddle_bound_of_the_port_closed_form() {
    let rows = parse_rows(FIXTURE);
    let (mut n, mut violations, mut exact) = (0, 0, 0);
    let mut worst_ratio = 0.0_f64;
    for r in rows.iter().filter(|r| r.name == "SwFbr") {
        let (bu, steps, open, absbound) = (r.col[0], r.col[1], r.col[2] == 1.0, r.col[3]);
        let mut s = MaterialState::fresh(900.0);
        s.burnup = bu * 1e-3;
        let m = SwellingModel::FbrMox { gap_open: open };
        let port = match strain_comp(&m, &s, 0) {
            Port::Value(v) => v,
            _ => continue,
        };
        n += 1;
        let d = (r.reference - port).abs();
        let bound = absbound + (steps + 8.0) * EPS * port.abs();
        worst_ratio = worst_ratio.max(d / bound.max(f64::MIN_POSITIVE));
        if d > bound {
            violations += 1;
        }
        if d <= (steps + 8.0) * EPS * port.abs() {
            exact += 1;
        }
    }
    println!("FBR-MOX: {n} cases, {exact} agree to rounding only, bound violations {violations}, worst d/bound {worst_ratio:.3}");
    assert!(n > 0);
    assert_eq!(violations, 0);
}
