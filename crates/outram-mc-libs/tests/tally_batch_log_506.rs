// SPDX-License-Identifier: GPL-3.0

//! **The report-only per-batch tally log leaves transport untouched, and
//! hands `TallyBatchRecorder` the per-batch sums** (GitHub #506).
//!
//! # Methodology
//!
//! `KeffSettings::tally_batch_log` / `FixedSourceSettings::tally_batch_log`
//! take an optional `TallyBatchLog`; the drivers append the per-batch flat
//! accumulator of the attached tally just before `flush_batch`. Scoring
//! draws no random number, so (predictions fixed before running):
//!
//! 1. `k_mean`, `k_by_generation` and every tally bin's `sum`, `sum_sq` and
//!    `count` are **bit-identical** with and without a log, on the
//!    sequential driver, the parallel driver and the fixed-source driver;
//! 2. the log holds exactly one batch per active generation (fixed source: per
//!    batch), each with one value per bin;
//! 3. adding the logged batches in order reproduces the tally bin's `sum`
//!    **bit for bit** (the flush adds the same values in the same order), and
//!    `TallyBatchRecorder::from_log` gives the series a caller-stepped
//!    recorder would have recorded, with a finite corrected uncertainty.
//!
//! Model: bare Godiva-composition HEU sphere (r = 8.7407 cm, `Core`
//! U-234/235/238), a cell-flux tally; 200 histories x [2 + 6], fixed seed.
//!
//! # Results (2026-11-14, release)
//!
//! All three predictions held; see the assertions.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::fixed_source::{run_fixed_source, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::keff::{KeffResult, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_par, run_keff_csg_seq, SourceBox};
use outram_mc_libs::stats::correlated_sigma::{TallyBatchLog, TallyBatchRecorder};
use outram_mc_libs::tally::filter::{CellFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const R: f64 = 8.7407;
const N_ACTIVE: usize = 6;

fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ]
}

fn heu() -> Material {
    Material {
        id: 1,
        name: "HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    }
}

fn geometry() -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            None,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn flux_tally() -> Tally {
    Tally {
        id: 1,
        name: "flux".into(),
        filters: vec![FilterKind::Cell(CellFilter { cell_indices: vec![0] })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 1],
    }
}

fn settings() -> KeffSettings {
    KeffSettings { n_particles: 200, n_inactive: 2, n_active: N_ACTIVE, seed: 11, ..Default::default() }
}

fn source_box() -> SourceBox {
    let h = R / 3f64.sqrt();
    SourceBox { lower: Position::new(-h, -h, -h), upper: Position::new(h, h, h) }
}

type Driver = fn(&KeffSettings, &mut Tally) -> KeffResult;

fn seq(s: &KeffSettings, t: &mut Tally) -> KeffResult {
    run_keff_csg_seq(&geometry(), &[heu()], &nuclides(), &[], None, source_box(), s, Some(t), &[], None)
}

fn par(s: &KeffSettings, t: &mut Tally) -> KeffResult {
    run_keff_csg_par(
        &geometry(),
        &[heu()],
        &nuclides(),
        &[],
        None,
        source_box(),
        s,
        Some(t),
        &[],
        None,
        ThreadCount::Fixed(2),
    )
}

fn bits(t: &Tally) -> Vec<(u64, u64, u64)> {
    t.bins.iter().map(|b| (b.sum.to_bits(), b.sum_sq.to_bits(), b.count)).collect()
}

#[test]
fn the_log_changes_no_bit_of_a_keff_run_and_holds_the_batch_sums() {
    for (name, run) in [("seq", seq as Driver), ("par", par as Driver)] {
        let mut plain_tally = flux_tally();
        let plain = run(&settings(), &mut plain_tally);

        let log = TallyBatchLog::new();
        let mut logged_tally = flux_tally();
        let logged = run(&settings().with_tally_batch_log(log.clone()), &mut logged_tally);

        assert_eq!(plain.k_mean.to_bits(), logged.k_mean.to_bits(), "{name}: k_mean");
        let kbits = |r: &KeffResult| r.k_by_generation.iter().map(|k| k.to_bits()).collect::<Vec<_>>();
        assert_eq!(kbits(&plain), kbits(&logged), "{name}: k by generation");
        assert_eq!(bits(&plain_tally), bits(&logged_tally), "{name}: tally bins");

        assert_eq!(log.len(), N_ACTIVE, "{name}: one batch per active generation");
        let batches = log.batches();
        assert!(batches.iter().all(|b| b.len() == 1 && b[0] > 0.0), "{name}: {batches:?}");
        let mut sum = 0.0_f64;
        for b in &batches {
            sum += b[0];
        }
        assert_eq!(sum.to_bits(), logged_tally.bins[0].sum.to_bits(), "{name}: sum of the log");

        let rec = TallyBatchRecorder::from_log(&log, 1).expect("lengths agree");
        assert_eq!(rec.series[0].len(), N_ACTIVE);
        let est = rec.bin_estimate(0).expect("bin 0");
        println!("{name}: batches {:?}, corrected estimate {est:?}", rec.series[0]);
        assert!(TallyBatchRecorder::from_log(&log, 2).is_err());
    }
}

#[test]
fn a_run_without_a_log_records_nothing() {
    assert!(KeffSettings::default().tally_batch_log.is_none());
    assert!(FixedSourceSettings::default().tally_batch_log.is_none());
}

#[test]
fn the_fixed_source_driver_logs_each_batch_without_changing_the_tally() {
    let src = FixedSource::Point { r: Position::ZERO, energy_ev: 1.0e6 };
    let s = FixedSourceSettings { n_particles: 400, n_batches: 5, seed: 3, ..Default::default() };
    let mut plain = flux_tally();
    run_fixed_source(&geometry(), &[heu()], &nuclides(), &src, &s, Some(&mut plain));

    let log = TallyBatchLog::new();
    let logged_settings = FixedSourceSettings { tally_batch_log: Some(log.clone()), ..s };
    let mut logged = flux_tally();
    run_fixed_source(&geometry(), &[heu()], &nuclides(), &src, &logged_settings, Some(&mut logged));

    assert_eq!(bits(&plain), bits(&logged));
    assert_eq!(log.len(), 5);
    let mut sum = 0.0_f64;
    for b in log.batches() {
        sum += b[0];
    }
    assert_eq!(sum.to_bits(), logged.bins[0].sum.to_bits());
}
