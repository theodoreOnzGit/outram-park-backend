// SPDX-License-Identifier: GPL-3.0-only
//! A whole pyDOSEIA run through the Rust API: the replacement for upstream's
//! `python main.py --config_file input.yaml`.
//!
//! > **Research, education and V&V only. Every table here is SYNTHETIC** (the
//! > code-to-code fixture's `tests/data/pydoseia_synthetic_*.csv`); the numbers
//! > are not doses to anyone and not properties of any real nuclide
//! > (`RESPONSIBLE_USE.md`).
//!
//! Upstream: pyDOSEIA (MIT, Copyright (c) 2024 Dr. Biswajit Sadhu), commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`; see `crates/buangkok/NOTICE`.
//!
//! ```text
//! cargo run --release -p buangkok --example pydoseia_assessment
//! ```

use buangkok::pydoseia::assessment::{
    plant_boundary_totals, run_assessment, summary_rows, AssessmentTables, SummaryIngestion,
    Zeroing,
};
use buangkok::pydoseia::config::{PyDoseiaConfig, ReleaseScenario};
use buangkok::pydoseia::dcf::{ExternalDcfTable, InhalationDcfTable, LungAbsorptionType, ProgenyChains};
use buangkok::pydoseia::ingestion::{EcoParamTable, IngestionDcfTable};
use buangkok::pydoseia::nuclide;
use buangkok::pydoseia::plume_shine::{AttenuationTable, GammaLineTable};

fn main() {
    let d = |s: &str| s.to_string();
    let tables = AssessmentTables {
        inhalation: InhalationDcfTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_inhalation_dcf.csv"
        ))
        .unwrap(),
        surface: ExternalDcfTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_surface_dcf.csv"
        ))
        .unwrap(),
        submersion: ExternalDcfTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_submersion_dcf.csv"
        ))
        .unwrap(),
        chains: ProgenyChains::from_csv(
            include_str!("../tests/data/pydoseia_synthetic_progeny_chains.csv"),
            include_str!("../tests/data/pydoseia_synthetic_progeny_half_lives.csv"),
        )
        .unwrap(),
        ingestion: IngestionDcfTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_ingestion_dcf.csv"
        ))
        .unwrap(),
        eco: EcoParamTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_eco_param.csv"
        ))
        .unwrap(),
        gamma: GammaLineTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_gamma_lines.csv"
        ))
        .unwrap(),
        attenuation: AttenuationTable::from_csv(include_str!(
            "../tests/data/pydoseia_synthetic_attenuation.csv"
        ))
        .unwrap(),
    };
    let mut cfg = PyDoseiaConfig::input_generator_defaults(ReleaseScenario::LongTerm {
        annual_discharge_bq: vec![3.0e11, 4.2e11],
    });
    cfg.nuclides = vec![d("SYN-1"), d("SYN-3")];
    cfg.elements = vec![d("Co"), d("Cs")];
    cfg.absorption_types = vec![LungAbsorptionType::Max; 2];
    cfg.release_height_m = 30.0;
    cfg.downwind_distances_m = vec![150.0, 800.0];
    cfg.plant_boundary_m = 500.0;
    cfg.run_plume_shine_dose = true;
    let lambdas: Vec<f64> = ["5.0 y", "30.0 y"]
        .iter()
        .map(|t| nuclide::upstream_decay_constant(nuclide::parse_primary_half_life(t).unwrap()))
        .collect();

    let r = run_assessment(&cfg, &tables, &lambdas, None).expect("valid configuration");
    println!("SYNTHETIC DATA - research demonstration only\n");
    println!("distance (m)  max chi/Q (s/m^3)");
    for (x, c) in r.distances_m.iter().zip(&r.max_chi_over_q) {
        println!("{x:>12}  {:.4e}", c.seconds_per_cubic_meter());
    }
    println!(
        "\nsummed doses per (distance, age), upstream's summary (ingestion counted twice: D20)"
    );
    for s in summary_rows(&r, SummaryIngestion::UpstreamDoubleCounted) {
        println!(
            "  {:>6} m, age {:>4}: inh {:.3e}  gs {:.3e}  sub {:.3e}  ing {:.3e}  total {:.3e} (mSv/y)",
            s.distance_m, s.age, s.inhalation, s.ground_shine, s.submersion, s.ingestion, s.total
        );
    }
    println!("\nplant-boundary totals per nuclide (plume shine not included, as upstream)");
    for (age, rows) in cfg.age_group.iter().zip(plant_boundary_totals(
        &cfg,
        &r,
        Zeroing::ChainedAssignmentNoOp,
        &[],
    )) {
        for (n, t) in cfg.nuclides.iter().zip(rows.unwrap_or_default()) {
            println!("  age {age:>4} {n}: total {:.3e} mSv/y", t.total);
        }
    }
    if let Some(ps) = &r.plume_shine {
        println!("\nplume shine per class A-F (upstream's units; see plume_shine docs)");
        for (x, per_nuclide) in r.distances_m.iter().zip(ps) {
            for (n, v) in cfg.nuclides.iter().zip(per_nuclide) {
                let cells: Vec<String> = v.iter().map(|c| format!("{c:.3e}")).collect();
                println!("  {x:>6} m {n}: {}", cells.join("  "));
            }
        }
    }
}
