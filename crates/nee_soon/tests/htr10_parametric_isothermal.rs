// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **Every zone at one temperature transports exactly as the
//! single-temperature model** (gh:#1138, #1143 isothermal regression).
//!
//! # Methodology
//!
//! `ParametricStudy` at N = 10 with every zone at 300.15 K and the published
//! tapes' graphite and SiC laws (`ThermalLawSource::Tapes`, the recorded
//! runs' laws) processes its jobs once. The same nuclide array then runs
//! twice with `run_keff_csg_hybrid`, 300 particles × [2 + 3] cycles, seed
//! 20 260 917, one thread:
//!
//! - **record path**: `htr10_material_set` at 300.15 K, the record's
//!   geometry `assemble_explicit_triso(14, 10, 0)`, `keff_vs_height::bed_majorant`;
//! - **zoned path**: the study's materials (zone copies, each at its zone's
//!   300.15 K), its zoned geometry (graphite balls on `MODERATOR_BALL`, a
//!   reflector copy per region, cell temperatures set), its majorant (the
//!   bed's materials plus `MODERATOR_BALL`).
//!
//! **Pass:** every cycle's k and the mean, bit for bit. The zone split moves
//! material indices only, and indices do not enter the arithmetic.
//!
//! # Results
//!
//! Recorded in `verification_and_validation/htr10_zone_temperatures/README.md`
//! (2026-11-03): identical, see there for the numbers.
//!
//! Gated behind `long-tests`: it processes all 36 HTR-10 jobs (minutes).

#![cfg(feature = "long-tests")]

use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use nee_soon::htr10_rmc::data::ThermalLawSource;
use nee_soon::htr10_rmc::keff_vs_height::{
    bed_majorant, fissile_entropy_mesh, fissile_source_box, SweepStatistics,
};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use nee_soon::htr10_rmc::parametric::{ParametricStudy, StudyBed};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;

#[test]
fn one_zone_temperature_transports_as_the_record_bit_for_bit() {
    let mut study = ParametricStudy {
        bed: StudyBed::Seker { layers: 10 },
        statistics: SweepStatistics {
            name: "isothermal",
            particles: 300,
            inactive: 2,
            active: 3,
            seed: 20_260_917,
        },
        ..ParametricStudy::default()
    };
    study.data.thermal_laws = ThermalLawSource::Tapes;
    let plan = study.plan().expect("plan");
    if plan
        .jobs
        .iter()
        .enumerate()
        .any(|(i, _)| matches!(study.job_tape(&plan, i), Ok(Some(t)) if !t.path().exists()))
    {
        eprintln!("SKIP: HTR-10 tapes not in this checkout");
        return;
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let nuclides = study
        .process_all(&plan, threads, |l| eprintln!("  {l}"))
        .expect("process");

    let zoned = study.transport(&plan, &nuclides, 1).expect("zoned run");

    let mats = htr10_material_set(&plan.layout, Htr10MaterialConfig::benchmark_default(300.15));
    let core = assemble_explicit_triso(14, 10, 0);
    let majorant = bed_majorant(&mats, &nuclides);
    let mesh = fissile_entropy_mesh(&core);
    let record = run_keff_csg_hybrid(
        &core.geometry,
        &mats,
        &nuclides,
        std::slice::from_ref(&majorant),
        Some(&mesh),
        fissile_source_box(&core),
        &study.statistics.keff_settings(1),
        None,
    );
    eprintln!(
        "record k = {:.17} ± {:.5}, zoned k = {:.17} ± {:.5}",
        record.k_mean, record.k_std, zoned.k_mean, zoned.k_std
    );
    assert_eq!(record.k_mean.to_bits(), zoned.k_mean.to_bits());
    assert_eq!(format!("{:?}", record.k_by_generation), format!("{:?}", zoned.k_by_generation));
    assert_eq!(record.histories, zoned.histories);
    assert_eq!(format!("{:?}", record.entropy), format!("{:?}", zoned.entropy));
    assert_eq!(record.majorant_violations, zoned.majorant_violations);
}
