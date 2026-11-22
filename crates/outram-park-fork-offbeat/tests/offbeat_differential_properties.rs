// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Heat capacity, density, Young's modulus, Poisson ratio and emissivity vs
//! OFFBEAT 80e84450 (gh:#1393, case 1)
//!
//! ## Methodology
//!
//! Reference: `verification_and_validation/offbeat_differential_gh1393/reference_properties.txt`
//! written by `dump_properties.cpp` (verbatim kernels of OFFBEAT's
//! `heatCapacity*.C`, `density*.C`, `YoungModulus*.C`, `PoissonRatioTobbe1515Ti.C`,
//! `emissivityRelapUO2.C`; route and caveats in
//! `verification_and_validation/properties_vs_offbeat.md`).  Each row is
//! evaluated by the port at the same point.  Points outside a model's
//! validity window are skipped.
//!
//! Gas constant: OFFBEAT's MATPRO oxide heat capacities use OpenFOAM's
//! `physicoChemical::R`, which in ESI v2506 is `NA*k = 6.0221417930e23 *
//! 1.38065e-23`; the port uses CODATA 2018.  The fixture holds both
//! (`*_RFoam`, `*_R2018`).  The `*_R2018` rows are gated at 1e-14 like all
//! others; the `*_RFoam` rows measure the effect of the constant and are
//! gated at the *derived* first-order bound `|dR/R| (ED/(R T) + 1)` (see
//! `worst_rfoam_ratio`), not at 1e-14.  (A first attempt at a flat 2e-6 bound
//! was a guess and was exceeded, 2.27e-6 observed; the derived bound replaced
//! it before any other change, and the observed value is recorded unchanged.)
//!
//! Pass criterion: relative difference < 1e-14 on every compared point except
//! the `*_RFoam` rows (above).
//!
//! ## Results
//!
//! See `verification_and_validation/properties_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, report, Stats};
use outram_park_fork_offbeat::materials::properties::density::DensityModel;
use outram_park_fork_offbeat::materials::properties::emissivity::EmissivityModel;
use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
use outram_park_fork_offbeat::materials::MaterialState;
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!(
    "../verification_and_validation/offbeat_differential_gh1393/reference_properties.txt"
);

/// A port model of any of the five families (an enum, not a trait object).
enum AnyModel {
    HeatCapacity(HeatCapacityModel),
    Density(DensityModel),
    Young(YoungModulusModel),
    Poisson(PoissonRatioModel),
    Emissivity(EmissivityModel),
}

impl AnyModel {
    fn range(&self) -> (f64, f64) {
        match self {
            Self::HeatCapacity(m) => m.temperature_range(),
            Self::Density(m) => m.validity_range(),
            Self::Young(m) => m.temperature_range(),
            Self::Poisson(m) => m.temperature_range(),
            Self::Emissivity(m) => m.validity_range(),
        }
    }

    fn value(&self, s: &MaterialState) -> f64 {
        match self {
            Self::HeatCapacity(m) => m.value(s),
            Self::Density(m) => m.value(s),
            Self::Young(m) => m.value(s),
            Self::Poisson(m) => m.value(s),
            Self::Emissivity(m) => m.value(s),
        }
    }
}

/// Columns: T, p, OM, w.
fn model_and_state(name: &str, c: &[f64]) -> AnyModel {
    let _ = (c[1], c[2], c[3]);
    match name {
        "HcMatproUo2_RFoam" | "HcMatproUo2_R2018" => AnyModel::HeatCapacity(HeatCapacityModel::MatproUo2),
        "HcMatproMox_RFoam" | "HcMatproMox_R2018" => AnyModel::HeatCapacity(HeatCapacityModel::MatproMox),
        "HcFinkMox" => AnyModel::HeatCapacity(HeatCapacityModel::FinkMox),
        "HcMatproZircaloy" => AnyModel::HeatCapacity(HeatCapacityModel::MatproZircaloy),
        "HcIaeaZircaloy" => AnyModel::HeatCapacity(HeatCapacityModel::IaeaZircaloy),
        "HcBanerjee1515Ti" => AnyModel::HeatCapacity(HeatCapacityModel::Banerjee1515Ti),
        "HcMolybdenum" => AnyModel::HeatCapacity(HeatCapacityModel::Molybdenum),
        "HcSneadSiC" => AnyModel::HeatCapacity(HeatCapacityModel::SneadSiC),
        "RhoIaeaZy" => AnyModel::Density(DensityModel::IAEAZy),
        "RhoSchumann1515Ti" => AnyModel::Density(DensityModel::Schumann1515Ti {
            reference_density: 7900.0,
        }),
        "EMatproUo2" => AnyModel::Young(YoungModulusModel::MatproUo2),
        "EMatproMox" => AnyModel::Young(YoungModulusModel::MatproMox),
        "ESckCenMox" => AnyModel::Young(YoungModulusModel::SckCenMox),
        "EHofmanD9" => AnyModel::Young(YoungModulusModel::HofmanD9),
        "EBisonMolybdenum" => AnyModel::Young(YoungModulusModel::BisonMolybdenum),
        "ETobbe1515Ti" => AnyModel::Young(YoungModulusModel::Tobbe1515Ti),
        "EWatrousHastelloyN" => AnyModel::Young(YoungModulusModel::WatrousHastelloyN),
        "NuTobbe1515Ti" => AnyModel::Poisson(PoissonRatioModel::Tobbe1515Ti),
        "EmRelapUo2" => AnyModel::Emissivity(EmissivityModel::RelapUO2),
        other => panic!("unknown fixture model {other}"),
    }
}

fn state_for(c: &[f64]) -> MaterialState {
    let mut s = MaterialState::fresh(c[0]);
    s.porosity = c[1];
    s.oxygen_deviation = c[2] - 2.0;
    s.pu_fraction = c[3];
    s
}

fn run_fixture() -> BTreeMap<String, Stats> {
    let mut stats: BTreeMap<String, Stats> = BTreeMap::new();
    for row in parse_rows(FIXTURE) {
        let model = model_and_state(&row.name, &row.col);
        let state = state_for(&row.col);
        let entry = stats.entry(row.name.clone()).or_default();
        let (lo, hi) = model.range();
        if state.temperature < lo || state.temperature > hi {
            entry.skipped_outside_window += 1;
            continue;
        }
        entry.add(model.value(&state), row.reference, &format!("{:?}", row.col));
    }
    stats
}

/// OpenFOAM-ESI v2506 gas constant `NA*k` (see the module doc).
const R_FOAM: f64 = 6.0221417930e+23 * 1.38065e-23;
/// CODATA 2018 value used by the port.
const R_2018: f64 = 8.314_462_618_153_24;
/// Largest Schottky-defect activation energy among the two oxides \[J/mol\]
/// (PuO2), for the first-order sensitivity bound.
const ED_MAX: f64 = 1.967e5;

/// For the `*_RFoam` rows: the worst ratio of the observed relative
/// difference to the first-order bound `|dR/R| * (ED/(R T) + 1)`.
///
/// Derivation: Cp = (regular terms) + S with S proportional to
/// `(1/R) exp(-ED/(R T))`, so `d ln S / d ln R = ED/(R T) - 1` in magnitude at
/// most `ED/(R T) + 1`, and `S <= Cp`, hence `|dCp/Cp| <= |dR/R| (ED/(R T) + 1)`.
/// The bound is derived from the model, not fitted to the observed value.
fn worst_rfoam_ratio() -> f64 {
    let dr = ((R_FOAM - R_2018) / R_2018).abs();
    let mut worst = 0.0_f64;
    for row in parse_rows(FIXTURE) {
        if !row.name.ends_with("_RFoam") {
            continue;
        }
        let model = model_and_state(&row.name, &row.col);
        let state = state_for(&row.col);
        let (lo, hi) = model.range();
        if state.temperature < lo || state.temperature > hi {
            continue;
        }
        let rel = ((model.value(&state) - row.reference) / row.reference).abs();
        let bound = dr * (ED_MAX / (R_2018 * state.temperature) + 1.0);
        worst = worst.max(rel / bound);
    }
    worst
}

#[test]
fn properties_agree_with_the_transcribed_offbeat_kernels() {
    let stats = run_fixture();
    report(&stats);
    for (name, s) in &stats {
        assert!(s.compared > 0, "{name}: nothing compared");
        if !name.ends_with("_RFoam") {
            assert!(s.worst_rel < 1e-14, "{name}: worst relative {:e} exceeds 1e-14", s.worst_rel);
        }
    }
    let ratio = worst_rfoam_ratio();
    println!("RFoam rows: worst observed/first-order-bound = {ratio:.4}");
    assert!(ratio <= 1.0, "R-constant effect exceeds its first-order bound: {ratio}");
}
