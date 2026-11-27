// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `NusseltThermalBaffle1D` arithmetic against a literal transcription (gh:#1871, round 51)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! **What this is and is not.** The upstream CLASS needs a mapped baffle pair, a `fluid` and an `FSPair`
//! (a solver) and was NOT run. `upstream_driver/nb1871_dump/nb1871Dump.C` is a literal transcription of the class's
//! arithmetic (constructor `hw_`, `calcH`, both `updateCoeffs()` branches, `evaluate()`) on real OpenFOAM
//! `scalarField`s with closed-form inputs (`refs/nb1871.ref.txt.gz`, `scripts/build_and_run_nb1871_dump.sh`). This
//! test checks that the Rust translation equals that transcription bit for bit: a check of the translation and of
//! OpenFOAM's field operators, not of the upstream class. Pass criterion: `to_bits` equality.

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::boundary_conditions::nusselt_baffle::{
    calc_h_upstream, evaluate_upstream, hw_upstream, update_one_phase, update_two_phase, NusseltCorrelationCoefficients,
    TwoPhaseInputs,
};

fn gunzip(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
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
    String::from_utf8(decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap()).unwrap()
}

type Key = (String, usize, String);

fn load() -> HashMap<Key, Vec<f64>> {
    let text = gunzip(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs/nb1871.ref.txt.gz"),
    );
    let mut me = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("N") {
            let v = it.next().unwrap().to_string();
            let s: usize = it.next().unwrap().parse().unwrap();
            let name = it.next().unwrap().to_string();
            me.insert((v, s, name), it.map(|x| x.parse().unwrap()).collect());
        }
    }
    me
}

struct Tally {
    n: usize,
    exact: usize,
    first: Option<String>,
}

impl Tally {
    fn cmp(&mut self, what: &str, port: &[f64], up: &[f64]) {
        assert_eq!(port.len(), up.len(), "{what}: length");
        for i in 0..port.len() {
            self.n += 1;
            if port[i].to_bits() == up[i].to_bits() {
                self.exact += 1;
            } else if self.first.is_none() {
                self.first = Some(format!("{what}[{i}]: port {:?} transcription {:?}", port[i], up[i]));
            }
        }
    }
}

#[test]
fn the_baffle_arithmetic_equals_the_transcription() {
    let r = load();
    let g = |v: &str, s: usize, n: &str| r[&(v.to_string(), s, n.to_string())].clone();
    let c1 = NusseltCorrelationCoefficients { const_term: 0.0, coeff: 0.023, exp_reynolds: 0.8, exp_prandtl: 0.4 };
    let c2 = NusseltCorrelationCoefficients { const_term: 4.36, coeff: 0.01, exp_reynolds: 0.7, exp_prandtl: 0.33 };
    let mut t = Tally { n: 0, exact: 0, first: None };
    let mut control_diff = 0usize;
    for (v, thickness) in [("onePhase", 0.002), ("twoPhase", 0.002), ("thin", 0.0)] {
        let hw = hw_upstream(15.0, thickness);
        t.cmp(&format!("{v} hw"), &[hw], &g(v, 0, "hw"));
        for s in 1..=12 {
            let hf = calc_h_upstream(&c1, &g(v, s, "Re1"), &g(v, s, "Pr1"), &g(v, s, "k1"), &g(v, s, "Dh"), None);
            let nbr_hf = calc_h_upstream(&c2, &g(v, s, "Re2"), &g(v, s, "Pr2"), &g(v, s, "k2"), &g(v, s, "Dh"), None);
            let hf_f = calc_h_upstream(&c1, &g(v, s, "Re1"), &g(v, s, "Pr1"), &g(v, s, "k1"), &g(v, s, "Dh"), Some(&g(v, s, "fF")));
            t.cmp(&format!("{v} {s} hf"), &hf, &g(v, s, "hf"));
            t.cmp(&format!("{v} {s} nbrHf"), &nbr_hf, &g(v, s, "nbrHf"));
            t.cmp(&format!("{v} {s} hfF"), &hf_f, &g(v, s, "hfF"));
            if v != "twoPhase" {
                let u = update_one_phase(hw, &hf, &nbr_hf, &g(v, s, "Ti"), &g(v, s, "nbrTi"), &g(v, s, "kfEff"), &g(v, s, "nbrKfEff"));
                t.cmp(&format!("{v} {s} Tw"), &u.tw, &g(v, s, "Tw"));
                t.cmp(&format!("{v} {s} nbrTw"), &u.nbr_tw, &g(v, s, "nbrTw"));
                t.cmp(&format!("{v} {s} refGrad"), &u.ref_grad, &g(v, s, "refGrad"));
                t.cmp(&format!("{v} {s} nbrRefGrad"), &u.nbr_ref_grad, &g(v, s, "nbrRefGrad"));
                let n = hf.len();
                let ref_value: Vec<f64> = {
                    // the driver prints only the evaluated value: rebuild refValue's closed form is not needed,
                    // because valueFraction = 0 (upstream's constructor default): refValue*0 is exactly 0.
                    vec![300.0; n]
                };
                let val = evaluate_upstream(&vec![0.0; n], &ref_value, &g(v, s, "Ti"), &u.ref_grad, &g(v, s, "dc"));
                t.cmp(&format!("{v} {s} value"), &val, &g(v, s, "value"));
                // control: dividing by hw twice as a "simplification" (A = 1 + hf/hw written as (hw + hf)/hw)
                let a_alt: Vec<f64> = hf.iter().map(|h| (hw + h) / hw).collect();
                let a_up: Vec<f64> = hf.iter().map(|h| 1.0 + h / hw).collect();
                if a_alt.iter().zip(a_up.iter()).any(|(x, y)| x.to_bits() != y.to_bits()) {
                    control_diff += 1;
                }
            } else {
                let x = TwoPhaseInputs {
                    hf: hf.clone(),
                    nbr_hf: nbr_hf.clone(),
                    other_hf: g(v, s, "oHf"),
                    other_nbr_hf: g(v, s, "oNbrHf"),
                    ti: g(v, s, "Ti"),
                    nbr_ti: g(v, s, "nbrTi"),
                    other_ti: g(v, s, "oTi"),
                    other_nbr_ti: g(v, s, "oNbrTi"),
                    kf_eff: g(v, s, "kfEff"),
                    nbr_kf_eff: g(v, s, "nbrKfEff"),
                    other_kf_eff: g(v, s, "oKfEff"),
                    other_nbr_kf_eff: g(v, s, "oNbrKfEff"),
                };
                let u = update_two_phase(hw, &x);
                t.cmp(&format!("{v} {s} Tw"), &u.tw, &g(v, s, "Tw"));
                t.cmp(&format!("{v} {s} nbrTw"), &u.nbr_tw, &g(v, s, "nbrTw"));
                t.cmp(&format!("{v} {s} refGrad"), &u.ref_grad, &g(v, s, "refGrad"));
                t.cmp(&format!("{v} {s} nbrRefGrad"), &u.nbr_ref_grad, &g(v, s, "nbrRefGrad"));
                t.cmp(&format!("{v} {s} oRefGrad"), &u.other_ref_grad, &g(v, s, "oRefGrad"));
                t.cmp(&format!("{v} {s} oNbrRefGrad"), &u.other_nbr_ref_grad, &g(v, s, "oNbrRefGrad"));
            }
        }
    }
    println!(
        "G4 NusseltThermalBaffle1D arithmetic vs literal transcription: {}/{} bit-identical{}; control (reordered A) differs in {control_diff} of 24 step sets",
        t.exact,
        t.n,
        t.first.as_ref().map(|d| format!(", first: {d}")).unwrap_or_default()
    );
    assert_eq!(t.exact, t.n, "{:?}", t.first);
}
