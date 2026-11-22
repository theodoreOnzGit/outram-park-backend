// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Young/Poisson: MATPRO Zircaloy, PARFUME buffer/PyC/SiC, Snead SiC, constants
//! vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_zy_parfume.txt` written by `dump_zy_parfume.cpp`
//! (verbatim `correct()` bodies of `YoungModulusMatproZy.C`, `PoissonRatioMatproZy.C`,
//! `YoungModulusPARFUME{Buffer,PyC,SiC}.C`, `YoungModulusSneadSiC.C`; a
//! transcription, not a run of the compiled library).  Predictions and derived
//! gates: `PREDICTIONS_round2.md` section C.  Poisson MATPRO is compared twice:
//! `consistent` (the shear modulus uses the same fluence unit as the Young's
//! modulus, which is what the port computes; gated G1/G2) and `as written`
//! (upstream's two classes use different fluence units; the difference to the
//! port is predicted in closed form and checked).  The four constant Poisson
//! classes are compared against their literal upstream defaults.
//!
//! ## Results
//!
//! See `verification_and_validation/zy_parfume_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, Gates, Stats};
use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
use outram_park_fork_offbeat::materials::MaterialState;
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!(
    "../verification_and_validation/offbeat_differential_gh1393/reference_zy_parfume.txt"
);
const EPS: f64 = f64::EPSILON;

fn zy_state(c: &[f64]) -> MaterialState {
    let mut s = MaterialState::fresh(c[0]);
    s.fast_fluence = c[1] * 1e4; // n/cm2 -> n/m2
    s.oxygen_content = c[2];
    s.cold_work = c[3];
    s
}

fn young_port(name: &str, c: &[f64]) -> Option<f64> {
    let mut s = MaterialState::fresh(c[0]);
    let m = match name {
        "EMatproZy" => {
            s = zy_state(c);
            YoungModulusModel::MatproZircaloy
        }
        "EParfumeBuffer" => {
            s.fast_fluence = c[1] * 1e4;
            YoungModulusModel::ParfumeBuffer { density: c[2] }
        }
        "EParfumePyC" => {
            s.fast_fluence = c[1] * 1e4;
            YoungModulusModel::ParfumePyC {
                density: c[2],
                bacon_anisotropy_factor: c[3],
                crystallite_diameter: c[4],
            }
        }
        "EParfumeSiC" => YoungModulusModel::ParfumeSiC,
        "ESneadSiC" => {
            s.porosity = c[1];
            YoungModulusModel::SneadSiC { porosity_coefficient: c[2] }
        }
        _ => return None,
    };
    m.value_checked(&s).ok().map(|_| m.value(&s))
}

#[derive(Default)]
struct Fam {
    stats: Stats,
    gates: Gates,
    skipped: usize,
}

#[test]
fn young_models_agree_with_the_transcribed_kernels() {
    let mut fam: BTreeMap<String, Fam> = BTreeMap::new();
    for r in parse_rows(FIXTURE) {
        let n = r.col.len();
        let (inputs, scale) = (&r.col[..n - 1], r.col[n - 1]);
        if !r.name.starts_with('E') {
            continue;
        }
        let f = fam.entry(r.name.clone()).or_default();
        match young_port(&r.name, inputs) {
            None => f.skipped += 1,
            Some(v) => {
                f.stats.add(v, r.reference, &format!("{inputs:?}"));
                f.gates.add(v, r.reference, scale);
            }
        }
    }
    println!("{:<16} {:>5} {:>7} {:>7} {:>9}  gates", "model", "n", "skipped", "bit-id", "worst rel");
    for (n, f) in &fam {
        println!(
            "{:<16} {:>5} {:>7} {:>7} {:>9.2e}  G1 fail {} | G2 checked {} fail {} | worst near-cancellation {:.2e} | worst G1 ratio {:.3}",
            n, f.stats.compared, f.skipped, f.stats.bit_identical, f.stats.worst_rel, f.gates.g1_fail,
            f.gates.g2_checked, f.gates.g2_fail, f.gates.worst_cond_rel, f.gates.worst_g1_ratio
        );
    }
    for (n, f) in &fam {
        assert!(f.stats.compared > 0, "{n}");
        assert_eq!(f.gates.g1_fail, 0, "{n}: G1");
        assert_eq!(f.gates.g2_fail, 0, "{n}: G2");
    }
}

fn k3(phi_over_par6: f64) -> f64 {
    0.88 + 0.12 * (-phi_over_par6).exp()
}

#[test]
fn poisson_matpro_zircaloy_consistent_units_agree_and_as_written_differs_as_predicted() {
    let model = PoissonRatioModel::MatproZircaloy;
    let mut cons = Gates::default();
    let mut cons_stats = Stats::default();
    let mut asw_same = Gates::default();
    let (mut asw_alpha_checked, mut asw_alpha_fail, mut worst_alpha_dev) = (0, 0, 0.0_f64);
    let mut max_blend_dev = 0.0_f64;
    let mut max_alpha_ratio = 1.0_f64;
    for r in parse_rows(FIXTURE) {
        let consistent = r.name == "NuMatproZy_cons";
        let as_written = r.name == "NuMatproZy_asw";
        if !consistent && !as_written {
            continue;
        }
        let n = r.col.len();
        let (inputs, scale) = (&r.col[..n - 1], r.col[n - 1]);
        let s = zy_state(inputs);
        let (t, f_cm2) = (inputs[0], inputs[1]);
        if model.value_checked(&s).is_err() {
            continue;
        }
        let port = model.value(&s);
        if consistent {
            cons_stats.add(port, r.reference, &format!("{inputs:?}"));
            // scale was emitted as (nu+1)*(condE+condG); the gate multiplies by 8 eps.
            cons.add(port, r.reference, scale);
        } else if f_cm2 == 0.0 || t >= 1273.0 {
            asw_same.add(port, r.reference, scale);
        } else if t < 1073.0 {
            // predicted closed form: (nu_up + 1)/(nu_port + 1) = K3(raw)/K3(m2)
            let predicted = k3(f_cm2 / 1e25) / k3(f_cm2 * 1e4 / 1e25);
            let observed = (r.reference + 1.0) / (port + 1.0);
            let dev = (observed - predicted).abs() / predicted;
            asw_alpha_checked += 1;
            worst_alpha_dev = worst_alpha_dev.max(dev);
            max_alpha_ratio = max_alpha_ratio.max(observed);
            // derived gate 16 eps (condE + condG); scale = (nu+1)(condE+condG)
            if dev > 16.0 * EPS * scale / (r.reference + 1.0) {
                asw_alpha_fail += 1;
            }
        } else {
            max_blend_dev = max_blend_dev.max(((r.reference + 1.0) / (port + 1.0) - 1.0).abs());
        }
    }
    println!(
        "Poisson MATPRO Zy consistent: n {} bit-id {} worst rel {:.2e}; G1 fail {}, G2 checked {} fail {}",
        cons_stats.compared, cons_stats.bit_identical, cons_stats.worst_rel, cons.g1_fail, cons.g2_checked,
        cons.g2_fail
    );
    println!(
        "as written, zero fluence or beta phase: G1 fail {}, G2 checked {} fail {}",
        asw_same.g1_fail, asw_same.g2_checked, asw_same.g2_fail
    );
    println!(
        "as written, alpha phase with fluence: {asw_alpha_checked} rows, predicted-ratio failures {asw_alpha_fail}, worst deviation from prediction {worst_alpha_dev:.2e}, largest (nu_up+1)/(nu_port+1) = {max_alpha_ratio:.6}"
    );
    println!("as written, blend band with fluence: largest |(nu_up+1)/(nu_port+1) - 1| = {max_blend_dev:.4e} (reported, not gated)");
    assert!(cons_stats.compared > 0);
    assert_eq!(cons.g1_fail, 0);
    assert_eq!(cons.g2_fail, 0);
    assert_eq!(asw_same.g1_fail, 0);
    assert_eq!(asw_same.g2_fail, 0);
    assert!(asw_alpha_checked > 0);
    assert_eq!(asw_alpha_fail, 0);
}

/// The four constant Poisson classes, compared against the literal defaults of
/// the upstream constructors (`constantPoissonRatioUO2.C` line 63 / 80: 0.316;
/// `constantPoissonRatioUPuO2.C`: 0.276; `constantPoissonRatioZy.C`: 0.3;
/// `constantPoissonRatioMolybdenum.C`: member initialiser 0.31, dictionary
/// default 0.316, see the port doc on `ConstantMolybdenum`).  Uncracked
/// (`nCracks = 0`, so `pow(0.5, 0) = 1`).
#[test]
fn constant_poisson_ratios_match_the_upstream_defaults() {
    let s = MaterialState::fresh(900.0);
    assert_eq!(PoissonRatioModel::MatproUo2.value(&s), 0.316 * 0.5_f64.powf(0.0));
    assert_eq!(PoissonRatioModel::MatproMox.value(&s), 0.276 * 0.5_f64.powf(0.0));
    assert_eq!(PoissonRatioModel::ConstantZircaloy.value(&s), 0.3);
    assert_eq!(PoissonRatioModel::ConstantMolybdenum.value(&s), 0.31);
}
