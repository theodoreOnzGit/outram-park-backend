// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **`SeedingMode::OpenMc` in the `physics::keff` sphere drivers and in
//! `FixedSource`** (gh:#1386, #1444; test added 2026-11-11).
//!
//! # Methodology
//!
//! 1. *Seed layout, sphere drivers:* `PowerIteration::new` under
//!    `SeedingMode::OpenMc` must sample source particle `i` on stream
//!    `STREAM_SOURCE` of `petir::rng::lcg::init_particle_seeds(i + 1, seed)`
//!    (OpenMC `8f79d9d0`, checked against the compiled OpenMC by
//!    `openmc_seeding_vs_openmc_code_to_code.rs`). The expected site is
//!    recomputed here from `init_particle_seeds` with the sphere sampler's draw
//!    order (isotropic direction, `cbrt` radius, independent direction, Watt).
//!    The per-history tracking/URR seeds are pinned by the unit test
//!    `rng::openmc_seeding::history_seed_tests`.
//! 2. *Seed layout, FixedSource:* a box source in a void sphere, 6 particles in
//!    2 batches; the birth state of the `p`-th track must equal the sample from
//!    `STREAM_SOURCE` of `init_particle_seeds(p + 1, seed)` (batch `b`, particle
//!    `i`: transport seed `b * 3 + i + 1`).
//! 3. *Serial vs parallel:* the sphere drivers `run_keff_cpu_single` and
//!    `run_keff_cpu_multi` (1 and 3 threads) must give the same `k` in every
//!    generation **bit for bit** under OpenMc seeding.
//!
//! **Pass criterion:** bit identity, no tolerance. **Prediction (written before
//! the run):** all bit-identical; each history owns its seeds, the source and
//! bank comb are seeded per generation, nothing depends on scheduling.
//!
//! # Results
//!
//! See the `println!` lines (`--nocapture`) and the 2026-11-11 run recorded in
//! the commit message.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source_traced, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::physics::keff::{
    run_keff_cpu_multi, run_keff_cpu_single, ComputeType, KeffResult, KeffSettings,
    PowerIteration,
};
use outram_mc_libs::physics::track_output::TrackRecorder;
use outram_mc_libs::rng::distributions::{isotropic_direction, watt};
use outram_mc_libs::rng::lcg::prn;
use outram_mc_libs::rng::openmc_seeding::{
    init_particle_seeds, SeedingMode, DEFAULT_STRIDE, STREAM_SOURCE,
};
use petir::mathf::RealMath;

const R: f64 = 8.7407;
const SEED: u64 = 20261111;

fn heu() -> (Material, Vec<Nuclide>) {
    (
        Material {
            id: 1,
            name: "HEU".into(),
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 1, atom_density: 2.4984e-3 },
            ],
            temperature: 293.6,
        },
        vec![
            Nuclide::from_core("U235").expect("U235 in CORE"),
            Nuclide::from_core("U238").expect("U238 in CORE"),
        ],
    )
}

fn settings(compute: ComputeType) -> KeffSettings {
    KeffSettings {
        n_particles: 300,
        n_inactive: 3,
        n_active: 5,
        temperature_k: 293.6,
        seed: SEED,
        compute,
        ..KeffSettings::default()
    }
    .with_seeding(SeedingMode::OpenMc { total_gen: 0 })
}

/// Sphere drivers, seed layout (methodology item 1).
#[test]
fn sphere_source_sits_on_openmc_source_stream_seeds() {
    let s = settings(ComputeType::CpuSingleThread);
    let it = PowerIteration::new(R, &s);
    let got = it.source_positions();
    assert_eq!(got.len(), s.n_particles);
    for (i, g) in got.iter().enumerate() {
        let mut seed = init_particle_seeds(i as i64 + 1, SEED as i64, DEFAULT_STRIDE)[STREAM_SOURCE];
        let (dx, dy, dz) = isotropic_direction(&mut seed);
        let rr = R * prn(&mut seed).r_cbrt();
        let _ = isotropic_direction(&mut seed);
        let _ = watt(&mut seed, s.watt_a, s.watt_b);
        let want = Position::new(rr * dx, rr * dy, rr * dz);
        assert_eq!(
            (g.x.to_bits(), g.y.to_bits(), g.z.to_bits()),
            (want.x.to_bits(), want.y.to_bits(), want.z.to_bits()),
            "source particle {i}"
        );
    }
    println!("sphere source: {}/{} bit-identical", got.len(), got.len());
    // the Outram default is a different layout
    let mut d = s.clone();
    d.seeding = SeedingMode::Outram;
    let d = PowerIteration::new(R, &d);
    assert_ne!(d.source_positions()[0].x.to_bits(), got[0].x.to_bits());
}

/// Sphere drivers, serial vs parallel (methodology item 3).
#[test]
fn sphere_serial_and_parallel_drivers_are_bit_identical_under_openmc_seeding() {
    let (mat, nucs) = heu();
    let bits = |r: &KeffResult| -> Vec<u64> {
        r.k_by_generation.iter().map(|k| k.to_bits()).collect()
    };
    let seq = run_keff_cpu_single(R, &mat, &nucs, &settings(ComputeType::CpuSingleThread));
    for n in [1usize, 3] {
        let par = run_keff_cpu_multi(
            R,
            &mat,
            &nucs,
            &settings(ComputeType::CpuMultiThread(ThreadCount::Fixed(n))),
            ThreadCount::Fixed(n),
        );
        println!(
            "sphere k: serial {:.5} +/- {:.5}, {n}-thread {:.5} +/- {:.5}; per-generation bit-identical: {}",
            seq.k_mean, seq.k_std, par.k_mean, par.k_std, bits(&seq) == bits(&par)
        );
        assert_eq!(bits(&seq), bits(&par), "serial and {n}-thread sphere drivers differ");
    }
    // OpenMc seeding really changed the run
    let mut d = settings(ComputeType::CpuSingleThread);
    d.seeding = SeedingMode::Outram;
    let def = run_keff_cpu_single(R, &mat, &nucs, &d);
    assert_ne!(bits(&seq), bits(&def));
}

/// FixedSource seed layout (methodology item 2).
#[test]
fn fixed_source_samples_on_openmc_source_stream_seeds() {
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 50.0,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            CellFill::Void,
            Position::ZERO,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };
    let (lo, hi) = (Position::new(-2.0, -3.0, -4.0), Position::new(2.0, 3.0, 4.0));
    let src = FixedSource::Box { lower: lo, upper: hi, energy_ev: 2.0e6 };
    let st = FixedSourceSettings {
        n_particles: 6,
        n_batches: 2,
        seed: SEED,
        ..Default::default()
    }
    .with_seeding(SeedingMode::OpenMc { total_gen: 0 });
    let mut rec = TrackRecorder::new(100, 100);
    run_fixed_source_traced(&geom, &[], &[], &src, &st, None, Some(&mut rec), None, None);
    assert_eq!(rec.tracks.len(), 6);
    for (p, t) in rec.tracks.iter().enumerate() {
        let mut seed = init_particle_seeds(p as i64 + 1, SEED as i64, DEFAULT_STRIDE)[STREAM_SOURCE];
        let (dx, _dy, _dz) = isotropic_direction(&mut seed);
        let want = Position::new(
            lo.x + prn(&mut seed) * (hi.x - lo.x),
            lo.y + prn(&mut seed) * (hi.y - lo.y),
            lo.z + prn(&mut seed) * (hi.z - lo.z),
        );
        let b = &t.states[0];
        assert_eq!(
            [b.r.x.to_bits(), b.r.y.to_bits(), b.r.z.to_bits(), b.u.u.to_bits()],
            [want.x.to_bits(), want.y.to_bits(), want.z.to_bits(), dx.to_bits()],
            "particle {p}"
        );
    }
    println!("FixedSource: 6/6 birth states bit-identical to STREAM_SOURCE samples");
}
