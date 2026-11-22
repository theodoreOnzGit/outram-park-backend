// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Oxidation kinetics vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_corrosion.txt` written by `dump_corrosion.cpp` (verbatim
//! `EPRI_KWU_CE`, `CathcartPawel` and `lowHighOxidationKineticsModel` `correctOxideThickness`;
//! a transcription, not a run of the compiled library).  Predictions and derived
//! gates: `PREDICTIONS_round2.md` section G.  Upstream branches on the in/out
//! variable `S`, so two modes are emitted: mode 1 (`S = S0` on entry, the first
//! nonlinear iteration) and mode 2 (`S` = the first call's result, the later
//! iterations).  The stateless port must equal mode 2 (gate G1), and differ from
//! mode 1 exactly on the rows that cross the 2 um transition (pinned count).
//!
//! ## Results
//!
//! See `verification_and_validation/corrosion_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, Gates, Stats};
use outram_park_fork_offbeat::corrosion::kinetics::OxidationKinetics;
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!(
    "../verification_and_validation/offbeat_differential_gh1393/reference_corrosion.txt"
);
const TRANSITION: f64 = 2.0e-6;

#[derive(Default)]
struct Fam {
    stats: Stats,
    gates: Gates,
}

#[test]
fn oxidation_kinetics_agree_with_the_transcribed_kernels_and_mode_one_differs_only_when_crossing() {
    let mut fam: BTreeMap<String, Fam> = BTreeMap::new();
    let (mut mode1_rows, mut mode1_differ, mut crossing_rows, mut crossing_and_differ) = (0, 0, 0, 0);
    let mut non_crossing_differ = 0;
    for r in parse_rows(FIXTURE) {
        let n = r.col.len();
        let (c, scale) = (&r.col[..n - 1], r.col[n - 1]);
        let (s0, t, phi_cm2, dt, mode) = (c[0], c[1], c[2], c[3], c[4] as u32);
        let model = match r.name.as_str() {
            "OxEpri" => OxidationKinetics::EpriKwuCe,
            "OxCathcart" => OxidationKinetics::CathcartPawel,
            "OxLowHigh" => OxidationKinetics::EpriKwuCeCathcartPawel,
            other => panic!("unknown row {other}"),
        };
        let flux_si = phi_cm2 * 1e4; // n/cm2/s -> n/m2/s
        let port = model.thickness(s0, t, flux_si, dt);
        if mode == 2 || r.name == "OxCathcart" {
            let f = fam.entry(r.name.clone()).or_default();
            f.stats.add(port, r.reference, &format!("{c:?}"));
            f.gates.add(port, r.reference, scale);
        } else {
            // mode 1 of the two EPRI-bearing models: S = S0 on entry
            mode1_rows += 1;
            let differs = (port - r.reference).abs() > 8.0 * f64::EPSILON * scale;
            if differs {
                mode1_differ += 1;
            }
            // Crossing rule from the grid: S0 < 2 um and the cubic overshoot exceeds 2 um (mode 1 value, EPRI branch only).
            let epri_branch = r.name == "OxEpri" || t < 673.0;
            // Added after the first run (prediction 2 was incomplete): with S0 exactly at the
            // transition thickness, `S <= S_trans` still selects the cubic branch on entry
            // (mode 1) whereas the port, and mode 2, are in the linear regime.
            let crossing = epri_branch && ((s0 < TRANSITION && r.reference > TRANSITION) || s0 == TRANSITION);
            if crossing {
                crossing_rows += 1;
                if differs {
                    crossing_and_differ += 1;
                }
            } else if differs {
                non_crossing_differ += 1;
            }
        }
    }
    println!("{:<12} {:>6} {:>7} {:>9}  gates", "model", "n", "bit-id", "worst rel");
    for (n, f) in &fam {
        println!(
            "{:<12} {:>6} {:>7} {:>9.2e}  G1 fail {} | G2 checked {} fail {} | worst G1 ratio {:.3}",
            n, f.stats.compared, f.stats.bit_identical, f.stats.worst_rel, f.gates.g1_fail, f.gates.g2_checked,
            f.gates.g2_fail, f.gates.worst_g1_ratio
        );
    }
    println!(
        "mode 1 (first nonlinear iteration, EPRI-bearing): {mode1_rows} rows, port differs on {mode1_differ}; crossing rows {crossing_rows} (differ {crossing_and_differ}); non-crossing rows that differ {non_crossing_differ}"
    );
    for (n, f) in &fam {
        assert!(f.stats.compared > 0, "{n}");
        assert_eq!(f.gates.g1_fail, 0, "{n}: G1");
        assert_eq!(f.gates.g2_fail, 0, "{n}: G2");
    }
    assert_eq!(non_crossing_differ, 0, "mode 1 may differ only on crossing rows");
    assert_eq!(crossing_rows, crossing_and_differ, "every crossing row must differ in mode 1");
    assert!(crossing_rows > 0);
}
