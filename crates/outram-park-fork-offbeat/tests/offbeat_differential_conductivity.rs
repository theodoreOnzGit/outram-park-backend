// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Thermal conductivity vs OFFBEAT 80e84450 (gh:#1393, case 1)
//!
//! ## Methodology
//!
//! Reference: `verification_and_validation/offbeat_differential_gh1393/reference_conductivity.txt`,
//! written by `dump_conductivity.cpp`, a standalone C++ program whose kernels
//! are verbatim copies of the `correct()` loop bodies of OFFBEAT's
//! `conductivity*.C` (see `check_verbatim.sh`; route and caveats in
//! `verification_and_validation/conductivity_vs_offbeat.md`).  Each fixture
//! row is evaluated by the port's `ConductivityModel::value` at the same
//! point, with the port's inputs derived from upstream's by the documented
//! unit conventions (burnup in MWd/kgHM, oxygen deviation, density
//! fraction).  Points outside the port's temperature window are skipped
//! (the port clamps, upstream extrapolates).
//!
//! Pass criterion (issue prediction): relative difference < 1e-14 on every
//! compared point.  Bit-identical and ulp counts are reported, not gated.
//!
//! ## Results
//!
//! See `verification_and_validation/conductivity_vs_offbeat.md` for the
//! recorded table.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, report, Stats};
use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
use outram_park_fork_offbeat::materials::MaterialState;
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!(
    "../verification_and_validation/offbeat_differential_gh1393/reference_conductivity.txt"
);

/// Columns: T, Bu[MWd/t], por, OM, Gd, pu, am, sw, epsd.
fn model_and_state(name: &str, c: &[f64]) -> Option<(ConductivityModel, MaterialState)> {
    let (t, bu, por, om, gd, pu, am, sw, epsd) =
        (c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8]);
    let mut s = MaterialState::fresh(t);
    s.porosity = por;
    s.gadolinia_fraction = gd;
    s.swelling = sw;
    s.densification = 3.0 * epsd;
    s.oxygen_deviation = om - 2.0;
    s.pu_fraction = pu;
    let base = name.trim_end_matches("_vol");
    let model = match base {
        "MatproUo2" => {
            s.burnup = bu / 1000.0 / 0.881;
            ConductivityModel::MatproUo2
        }
        "NfirUo2" => {
            s.burnup = bu / 1000.0 / 0.881;
            ConductivityModel::NfirUo2
        }
        "Ifa601Uo2" => {
            s.burnup = bu / 1000.0;
            ConductivityModel::Ifa601Uo2
        }
        "BrancheriaMox" => ConductivityModel::BrancheriaMox,
        "KatoMox" => ConductivityModel::KatoMox {
            americium_atom_fraction: am,
            neptunium_atom_fraction: 0.0,
        },
        "LanningBeyerMox" => {
            s.burnup = bu * 1e-3;
            ConductivityModel::LanningBeyerMox
        }
        "MagniMamox" => {
            s.burnup = bu * 1e-3;
            ConductivityModel::MagniMamox {
                americium_atom_fraction: am,
                neptunium_atom_fraction: 0.0,
            }
        }
        "PhilipponneauMox" => {
            s.burnup = bu * 1e-3;
            ConductivityModel::PhilipponneauMox
        }
        "RelapZircaloy" => ConductivityModel::RelapZircaloy,
        "Molybdenum" => ConductivityModel::Molybdenum,
        "SwindemanHastelloyN" => ConductivityModel::SwindemanHastelloyN,
        "Tobbe1515Ti" => ConductivityModel::Tobbe1515Ti,
        "ParfumeBuffer" => ConductivityModel::ParfumeBuffer {
            initial_density: 1000.0,
            theoretical_density: 2250.0,
            initial_conductivity: 0.5,
            theoretical_conductivity: 4.0,
        },
        "ParfumeSiC" => ConductivityModel::ParfumeSiC,
        _ => return None,
    };
    Some((model, s))
}

/// Run the whole fixture; returns the per-model statistics.
fn run_fixture() -> BTreeMap<String, Stats> {
    let mut stats: BTreeMap<String, Stats> = BTreeMap::new();
    for row in parse_rows(FIXTURE) {
        let (model, state) = model_and_state(&row.name, &row.col).expect("known model");
        let entry = stats.entry(row.name.clone()).or_default();
        let (lo, hi) = model.temperature_range();
        if state.temperature < lo || state.temperature > hi {
            entry.skipped_outside_window += 1;
            continue;
        }
        let port = model.value(&state);
        let desc = format!("{:?}", row.col);
        entry.add(port, row.reference, &desc);
    }
    stats
}

#[test]
fn conductivity_agrees_with_the_transcribed_offbeat_kernels_to_1e_14() {
    let stats = run_fixture();
    let worst = report(&stats);
    for (name, s) in &stats {
        assert!(s.compared > 0, "{name}: nothing compared");
    }
    assert!(worst < 1e-14, "worst relative difference {worst:e} exceeds 1e-14");
}
