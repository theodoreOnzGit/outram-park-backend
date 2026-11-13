// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # FRAPCON fuel-rod gap conductance vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_gap.txt` written by `dump_gap.cpp` (verbatim
//! `gapFRAPCON::kappa`, `gapFRAPCON::a` and the per-face body of
//! `fuelRodGapFvPatchScalarField::hGap`; a transcription, not a run of the
//! compiled library).  Predictions and derived gates: `PREDICTIONS_round2.md`
//! section E.  `hRad` is emitted with OpenFOAM-ESI v2506's `physicoChemical::sigma`
//! (`_Foam`) and with the CODATA 2018 value the port uses (`_2018`); the second is
//! gated G1/G2, the first is checked to differ by the relative difference of the two
//! sigmas (`h_rad` is linear in sigma).
//!
//! ## Results
//!
//! See `verification_and_validation/gap_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, Gates, Stats};
use outram_park_fork_offbeat::gap::gas::GapGasMixture;
use outram_park_fork_offbeat::gap::{GapConductanceModel, GapConductanceScaling, GapSurfaces};
use std::collections::BTreeMap;

const FIXTURE: &str =
    include_str!("../verification_and_validation/offbeat_differential_gh1393/reference_gap.txt");

/// Mole fractions in the port's species order [He, Ne, Ar, Kr, Xe, Rn] from (He, Ar, Kr, Xe).
fn mixture(he: f64, ar: f64, kr: f64, xe: f64) -> GapGasMixture {
    GapGasMixture::from_mole_fractions([he, 0.0, ar, kr, xe, 0.0], 1.0e-5).expect("valid dyadic mixture")
}

fn surfaces(c_fuel_t: f64, c_clad_t: f64) -> GapSurfaces {
    GapSurfaces::lwr_open_gap(c_fuel_t, c_clad_t)
}

fn model(width: f64) -> GapConductanceModel {
    GapConductanceModel::FuelRodFrapcon { radial_gap_width: width, scaling: GapConductanceScaling::default() }
}

#[derive(Default)]
struct Fam {
    stats: Stats,
    gates: Gates,
}

/// 2018 value of the Stefan-Boltzmann constant used by the port, and OpenFOAM-ESI v2506's.
fn sigma_foam() -> f64 {
    let pi = std::f64::consts::PI;
    let hr = 6.62607e-34 / (2.0 * pi);
    pi * pi / 60.0 * 1.38065e-23_f64.powi(4) / (hr.powi(3) * 2.99792e8_f64 * 2.99792e8_f64)
}
const SIGMA_2018: f64 = 5.670_374_419e-8;

#[test]
fn gap_conductance_agrees_with_the_transcribed_kernels() {
    let mut fam: BTreeMap<String, Fam> = BTreeMap::new();
    let mut rad_foam_worst_dev = 0.0_f64;
    let mut rad_foam_n = 0;
    let expected_ratio = sigma_foam() / SIGMA_2018;
    for r in parse_rows(FIXTURE) {
        let n = r.col.len();
        let (c, scale) = (&r.col[..n - 1], r.col[n - 1]);
        let port = match r.name.as_str() {
            "GapKappa" => mixture(c[1], c[2], c[3], c[4]).conductivity(c[0]),
            "GapA" => mixture(c[1], c[2], c[3], c[4]).accommodation_coefficient(c[0]),
            "GapHGas" => {
                // pT, nbrT, width, R, nbrR, P, gasP, mix(4)
                let mut s = surfaces(c[0], c[1]);
                s.fuel_roughness = c[3];
                s.clad_roughness = c[4];
                s.interface_pressure = c[5];
                model(c[2]).evaluate(&mixture(c[7], c[8], c[9], c[10]), c[6], &s).gas
            }
            "GapHRad_Foam" | "GapHRad_2018" => {
                let mut s = surfaces(c[0], c[1]);
                s.fuel_emissivity = c[2];
                s.clad_emissivity = c[3];
                model(1e-5).evaluate(&mixture(1.0, 0.0, 0.0, 0.0), 2e6, &s).radiation
            }
            "GapHContact" => {
                let mut s = surfaces(800.0, 600.0);
                s.interface_pressure = c[0];
                s.fuel_conductivity = c[1];
                s.clad_conductivity = c[2];
                s.fuel_roughness = c[3];
                s.clad_roughness = c[4];
                model(0.0).evaluate(&mixture(1.0, 0.0, 0.0, 0.0), 2e6, &s).contact
            }
            other => panic!("unknown row {other}"),
        };
        if r.name == "GapHRad_Foam" {
            // upstream = port * (sigma_foam / sigma_2018) to first order (linear in sigma)
            let observed = r.reference / port;
            rad_foam_worst_dev = rad_foam_worst_dev.max((observed - expected_ratio).abs() / expected_ratio);
            rad_foam_n += 1;
            continue;
        }
        let f = fam.entry(r.name.clone()).or_default();
        f.stats.add(port, r.reference, &format!("{c:?}"));
        f.gates.add(port, r.reference, scale);
    }
    println!("{:<14} {:>5} {:>7} {:>9}  gates", "model", "n", "bit-id", "worst rel");
    for (n, f) in &fam {
        println!(
            "{:<14} {:>5} {:>7} {:>9.2e}  G1 fail {} | G2 checked {} fail {} | worst near-cancellation {:.2e} | worst G1 ratio {:.3}",
            n, f.stats.compared, f.stats.bit_identical, f.stats.worst_rel, f.gates.g1_fail,
            f.gates.g2_checked, f.gates.g2_fail, f.gates.worst_cond_rel, f.gates.worst_g1_ratio
        );
    }
    println!(
        "hRad with OpenFOAM's sigma: {rad_foam_n} rows, upstream/port = {expected_ratio:.9} predicted (sigma ratio {:.3e}); worst deviation from it {rad_foam_worst_dev:.2e}",
        expected_ratio - 1.0
    );
    for (n, f) in &fam {
        assert!(f.stats.compared > 0, "{n}");
        assert_eq!(f.gates.g1_fail, 0, "{n}: G1");
        assert_eq!(f.gates.g2_fail, 0, "{n}: G2");
    }
    assert!(rad_foam_n > 0);
    assert!(rad_foam_worst_dev < 4.0 * f64::EPSILON, "the sigma effect is not the whole difference");
}
