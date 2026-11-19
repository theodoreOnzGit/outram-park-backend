// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # MATPRO fuel creep and power-law creep vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_creep.txt` written by `dump_creep.cpp` (verbatim
//! pointwise kernels of `MatproCreepModel::correctCreep` (rate and Newton
//! derivative) and `powerLaw::correctCreep`; a transcription, not a run of the
//! compiled library).  Predictions and derived gates: `PREDICTIONS_round2.md`
//! section F.  The stress is an input (`sigmaEff >= 1e5 Pa`, so the lower clamp
//! the port deliberately omits does not enter); the stress history, the implicit
//! loop and the relaxation are not exercised.
//!
//! ## Results
//!
//! See `verification_and_validation/creep_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, Gates, Stats};
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::mechanics::LinearElastic;
use outram_park_fork_offbeat::rheology::{CreepModel, IrradiationState, RheologyInputs, RheologyState};
use outram_foam_basic_lib::primitives::SymmTensor;
use std::collections::BTreeMap;

const FIXTURE: &str =
    include_str!("../verification_and_validation/offbeat_differential_gh1393/reference_creep.txt");

#[derive(Default)]
struct Fam {
    stats: Stats,
    gates: Gates,
    refused: usize,
}

#[test]
fn creep_kernels_agree_with_the_transcribed_offbeat_kernels() {
    let mut fam: BTreeMap<String, Fam> = BTreeMap::new();
    for r in parse_rows(FIXTURE) {
        let n = r.col.len();
        let (c, scale) = (&r.col[..n - 1], r.col[n - 1]);
        let f = fam.entry(r.name.clone()).or_default();
        let port = match r.name.as_str() {
            "CreepMatproRate" | "CreepMatproDeriv" => {
                // T, sigma, Q, grain radius, porosity, OM, sakai, rho
                let mut material = MaterialState::fresh(c[0]);
                material.porosity = c[4];
                material.oxygen_deviation = c[5] - 2.0;
                let elastic = LinearElastic::new(1.0e11, 0.3).unwrap();
                let mut inputs = RheologyInputs::quasi_static(elastic, SymmTensor::ZERO, material);
                inputs.irradiation = IrradiationState {
                    fast_flux: 0.0,
                    fission_rate: c[2] / 312e-13,
                    grain_radius: c[3],
                };
                let model = CreepModel::Matpro { sakai_correction: c[6] == 1.0 };
                match model.rate_and_derivative(c[1], &inputs, &RheologyState::pristine()) {
                    Ok((rate, deriv)) => {
                        if r.name == "CreepMatproRate" { rate } else { deriv }
                    }
                    Err(_) => {
                        f.refused += 1;
                        continue;
                    }
                }
            }
            "CreepPowerLaw" => {
                let inputs = RheologyInputs::quasi_static(
                    LinearElastic::new(1.0e11, 0.3).unwrap(),
                    SymmTensor::ZERO,
                    MaterialState::fresh(900.0),
                );
                let model = CreepModel::PowerLaw { b: c[0], sigma_c: c[1], n: c[2] };
                model.rate_and_derivative(c[3], &inputs, &RheologyState::pristine()).expect("power law").0
            }
            other => panic!("unknown row {other}"),
        };
        f.stats.add(port, r.reference, &format!("{c:?}"));
        f.gates.add(port, r.reference, scale);
    }
    println!("{:<18} {:>6} {:>7} {:>7} {:>9}  gates", "model", "n", "refused", "bit-id", "worst rel");
    for (n, f) in &fam {
        println!(
            "{:<18} {:>6} {:>7} {:>7} {:>9.2e}  G1 fail {} | G2 checked {} fail {} | worst G1 ratio {:.3}",
            n, f.stats.compared, f.refused, f.stats.bit_identical, f.stats.worst_rel, f.gates.g1_fail,
            f.gates.g2_checked, f.gates.g2_fail, f.gates.worst_g1_ratio
        );
    }
    for (n, f) in &fam {
        assert!(f.stats.compared > 0, "{n}");
        assert_eq!(f.gates.g1_fail, 0, "{n}: G1");
        assert_eq!(f.gates.g2_fail, 0, "{n}: G2");
    }
}
