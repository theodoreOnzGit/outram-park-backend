// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Case 3 of gh:#1434: MOOSE J2 radial return with linear hardening
//! against the port's [`Material::J2`].**
//!
//! Reference: `reference_moose_j2.txt`, written by `dump_moose_j2.cpp`, whose
//! `UPSTREAM` lines are verbatim copies of MOOSE `fe80d24f`
//! (`RadialReturnStressUpdate.C`, `IsotropicPlasticityStressUpdate.C`,
//! `SingleVariableReturnMappingSolution.C`, `RankFourTensorImplementation.h`;
//! checked by `check_verbatim.sh`).  MOOSE was not built; Eigen's mat-vec and
//! libMesh's `contract` summation orders are not reproduced, so bit-identity
//! is not the criterion here.
//!
//! Methodology: E = 200 GPa, nu = 0.3, sigma_y0 = 250 MPa, H = 1 GPa
//! (synthetic, not a material).  Sixty incremental steps of a combined
//! axial/shear strain history `a(t) * Shape` that ramps to a = 0.006 in 24
//! steps and reverses to a = -0.004 in 36 steps, so it passes through elastic
//! loading, first yield, plastic loading, elastic unloading and reverse
//! yield.  MOOSE advances an incremental elastic strain; the port is given the
//! total strain `a_n * Shape` (engineering shear = 2 x tensor shear) and
//! carries its own plastic strain.
//!
//! Pass criteria (set before the run, `PREDICTIONS.md` P3.1): per step,
//! `||d stress||_inf / max over history of ||stress||_inf < 1e-12`, same for
//! plastic strain, and
//! the equivalent plastic strain within 1e-12 relative; yield-onset step and
//! the yielding/elastic flag identical.  Results: `RESULTS.md`.

use farrer_park::material::{Material, MaterialState, PlaneCondition};
use farrer_park::tensor::Voigt6;

const REFERENCE: &str =
    include_str!("../verification_and_validation/prisms_moose_differential_gh1434/reference_moose_j2.txt");

fn f(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("hex bit pattern"))
}

fn linf(a: &[f64; 6]) -> f64 {
    a.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
}

#[test]
fn j2_radial_return_matches_moose_transcription() {
    let mut params = [0.0; 4];
    let mut steps: Vec<(f64, [f64; 6], [f64; 6], f64, bool)> = Vec::new();
    for line in REFERENCE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t[0] {
            "JP" => {
                for k in 0..4 {
                    params[k] = f(t[1 + k]);
                }
            }
            "J" => {
                let a = f(t[2]);
                let mut s = [0.0; 6];
                let mut e = [0.0; 6];
                for k in 0..6 {
                    s[k] = f(t[3 + k]);
                    e[k] = f(t[9 + k]);
                }
                steps.push((a, s, e, f(t[15]), t[16] == "1"));
            }
            _ => panic!("unknown line {line}"),
        }
    }
    assert_eq!(steps.len(), 60);
    let material = Material::j2_linear_hardening(params[0], params[1], params[2], params[3]).unwrap();
    // Shape tensor of the driver: [[1, .2, 0], [.2, -.35, .1], [0, .1, -.3]].
    // Voigt order xx yy zz yz xz xy, engineering shear = 2 x tensor shear.
    let shape = [1.0, -0.35, -0.3, 2.0 * 0.1, 2.0 * 0.0, 2.0 * 0.2];
    let mut state: MaterialState = material.initial_state();
    let (mut worst_s, mut worst_e, mut worst_p) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut onset_up = None;
    let mut onset_port = None;
    let mut flags_agree = true;
    // Metric scales: the maxima over the whole history (PREDICTIONS.md P3.1 says
    // "max|stress|" and "max|eps_p|"); a per-step norm would blow up where the
    // plastic strain passes through zero on reverse loading (step 50 here).
    let scale_s = steps.iter().fold(0.0_f64, |m, st| m.max(linf(&st.1)));
    let scale_e = steps.iter().fold(0.0_f64, |m, st| m.max(linf(&st.2)));
    let mut worst_e_instantaneous = 0.0_f64;
    for (n, (a, s_up, e_up, p_up, yielding_up)) in steps.iter().enumerate() {
        let strain = Voigt6::new(a * shape[0], a * shape[1], a * shape[2], a * shape[3], a * shape[4], a * shape[5]);
        let out = material.update(strain, &state, PlaneCondition::PlaneStrain).unwrap();
        state = out.state;
        let s = out.stress.0;
        let mut e = state.plastic_strain.0;
        // Engineering shear -> tensor shear, as MOOSE stores it.
        for k in 3..6 {
            e[k] *= 0.5;
        }
        let ds = linf(&[s[0] - s_up[0], s[1] - s_up[1], s[2] - s_up[2], s[3] - s_up[3], s[4] - s_up[4], s[5] - s_up[5]]);
        worst_s = worst_s.max(ds / scale_s);
        let de = linf(&[e[0] - e_up[0], e[1] - e_up[1], e[2] - e_up[2], e[3] - e_up[3], e[4] - e_up[4], e[5] - e_up[5]]);
        worst_e = worst_e.max(de / scale_e);
        if linf(e_up) > 0.0 {
            worst_e_instantaneous = worst_e_instantaneous.max(de / linf(e_up));
        } else {
            assert_eq!(de, 0.0, "step {n}: plastic strain must be exactly zero before first yield");
        }
        if *p_up > 0.0 {
            worst_p = worst_p.max((state.equivalent_plastic_strain - p_up).abs() / p_up);
        } else {
            assert_eq!(state.equivalent_plastic_strain, 0.0);
        }
        if *yielding_up && onset_up.is_none() {
            onset_up = Some(n);
        }
        if out.yielding && onset_port.is_none() {
            onset_port = Some(n);
        }
        if out.yielding != *yielding_up {
            flags_agree = false;
        }
    }
    println!("J2 vs MOOSE transcription: 60 steps");
    println!("  worst ||d stress||inf / ||stress||inf     = {worst_s:.3e}");
    println!("  worst ||d plastic strain||inf / history max = {worst_e:.3e}");
    println!("  (same, divided by the instantaneous norm   = {worst_e_instantaneous:.3e}, at the step where eps_p crosses zero)");
    println!("  worst |d p| / p                           = {worst_p:.3e}");
    println!("  yield onset: MOOSE step {onset_up:?}, port step {onset_port:?}; flags agree on every step: {flags_agree}");
    assert!(flags_agree, "yielding flag differs on some step");
    assert_eq!(onset_up, onset_port);
    assert!(worst_s < 1.0e-12, "stress {worst_s:e}");
    assert!(worst_e < 1.0e-12, "plastic strain {worst_e:e}");
    assert!(worst_p < 1.0e-12, "equivalent plastic strain {worst_p:e}");
}
