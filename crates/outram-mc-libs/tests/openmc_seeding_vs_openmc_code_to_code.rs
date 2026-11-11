// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **OpenMC-compatible seeding against OpenMC `8f79d9d0`, compiled and run**
//! (gh:#1386, section 2 item 1).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_seeding/run.sh` compiles OpenMC's
//! `src/random_lcg.cpp` unmodified and the verbatim extract of
//! `overall_generation`, `compute_particle_id`, `compute_transport_seed`
//! (`src/simulation.cpp`), and writes `reference.txt` (hex bits). This test
//! compares:
//!
//! - `init_particle_seeds`: 5 master seeds x 12 ids x 4 streams, plus the first
//!   and third `prn` of stream 0 and the first of the URR stream; 4 ids at a
//!   non-default stride;
//! - `OpenMcRunLayout::{compute_particle_id, compute_transport_seed}` over
//!   shared bank, rank, `gen_per_batch`, `total_gen`, batch and generation;
//! - the identity `init_particle_seeds == init_seed` per stream.
//!
//! Plus an integration run: a bare HEU sphere under `run_keff_csg` on the
//! parallel backend in three arms: the default, explicit `SeedingMode::Outram`
//! (must be bit-identical to the default), and `SeedingMode::OpenMc` (must
//! differ, be reproducible, and not depend on the thread count).
//!
//! **Pass criterion:** bit identity of every integer and `f64` bit pattern, no
//! tolerance. **Prediction (written before the run):** all bit-identical, since
//! the arithmetic is integer except one exact multiply by 2^-64.
//!
//! # Results (2026-11-11, rustc 1.99.1, g++ 13.3 -O2 -ffp-contract=off)
//!
//! See `verification_and_validation/upstream_openmc_seeding/README.md`.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::keff::{ComputeType, KeffResult, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::rng::lcg::{init_seed, prn};
use outram_mc_libs::rng::openmc_seeding::{
    init_particle_seeds, OpenMcRunLayout, SeedingMode, DEFAULT_STRIDE, N_STREAMS,
    STREAM_TRACKING, STREAM_URR_PTABLE,
};
use petir::mathf::RealMath;

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_seeding/reference.txt");

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split_whitespace()
        .find_map(|t| t.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .unwrap_or_else(|| panic!("no field `{key}` in `{line}`"))
}
fn hex(line: &str, key: &str) -> u64 {
    u64::from_str_radix(field(line, key), 16).unwrap()
}
fn dec(line: &str, key: &str) -> i64 {
    field(line, key).parse().unwrap()
}
fn lines_with(prefix: &str) -> Vec<&'static str> {
    REFERENCE
        .lines()
        .filter(|l| l.split_whitespace().next() == Some(prefix))
        .collect()
}

/// `init_particle_seeds`: all four stream seeds and the draws of the tracking
/// and URR streams, 60 (master, id) pairs.
#[test]
fn init_particle_seeds_matches_openmc() {
    let lines = lines_with("seeds");
    assert_eq!(lines.len(), 60);
    for l in &lines {
        let (m, id) = (dec(l, "master"), dec(l, "id"));
        let s = init_particle_seeds(id, m, DEFAULT_STRIDE);
        for i in 0..N_STREAMS {
            assert_eq!(s[i], hex(l, &format!("s{i}")), "{l} stream {i}");
        }
        let mut t = s[STREAM_TRACKING];
        let a = prn(&mut t);
        prn(&mut t);
        let b = prn(&mut t);
        let mut u = s[STREAM_URR_PTABLE];
        let c = prn(&mut u);
        assert_eq!(a.to_bits(), hex(l, "trk1"), "{l}");
        assert_eq!(b.to_bits(), hex(l, "trk3"), "{l}");
        assert_eq!(c.to_bits(), hex(l, "urr1"), "{l}");
    }
}

/// Non-default stride (`openmc_set_stride`): 4 ids.
#[test]
fn init_particle_seeds_honours_the_stride() {
    let lines = lines_with("seeds_stride1000003");
    assert_eq!(lines.len(), 4);
    for l in lines {
        let s = init_particle_seeds(dec(l, "id"), dec(l, "master"), 1_000_003);
        for i in 0..N_STREAMS {
            assert_eq!(s[i], hex(l, &format!("s{i}")), "{l}");
        }
    }
}

/// `compute_particle_id` and `compute_transport_seed` over every combination
/// the driver printed (shared bank on/off, rank 0 / rank 1 of 4,
/// `gen_per_batch` 1 and 5, `total_gen` 0 and 30, batches 1/2/7).
#[test]
fn particle_id_and_transport_seed_match_openmc() {
    let lines = lines_with("ids");
    assert!(lines.len() > 500, "{} lines", lines.len());
    for l in &lines {
        let np = dec(l, "np");
        let rank = dec(l, "rank");
        let shared = dec(l, "shared") != 0;
        let layout = OpenMcRunLayout {
            master_seed: 1,
            stride: DEFAULT_STRIDE,
            n_particles: np,
            gen_per_batch: dec(l, "gpb"),
            total_gen: dec(l, "total_gen"),
            // The driver's rank 1 of 4 has work_index[1] = np / 4.
            rank_work_index: if rank == 0 { 0 } else { np / 4 },
            shared_secondary_bank: shared,
            simulation_tracks_completed: if shared { 17 } else { 0 },
        };
        let pid = layout.compute_particle_id(dec(l, "index"));
        assert_eq!(pid, dec(l, "pid"), "{l}");
        let ts = layout.compute_transport_seed(pid, dec(l, "batch"), dec(l, "gen"));
        assert_eq!(ts, dec(l, "tseed"), "{l}");
    }
}

/// `init_particle_seeds(id)[k] == init_seed(id, k)` (the identity OpenMC's
/// driver also checks for 64 ids), for 2 masters here.
#[test]
fn init_particle_seeds_equals_init_seed_per_stream() {
    assert!(REFERENCE.contains("identity init_particle_seeds==init_seed ok ids=64 streams=4"));
    for m in [1i64, 42] {
        for id in 0..64 {
            let s = init_particle_seeds(id, m, DEFAULT_STRIDE);
            for k in 0..N_STREAMS {
                assert_eq!(s[k], init_seed(id, k as i64, m));
            }
        }
    }
}

/// The layout the CSG driver uses: history `i` of generation `g` (0-based) is
/// OpenMC's `current_batch = g + 1`, `current_gen = 1`, `index_source = i + 1`.
#[test]
fn history_indexing_is_openmc_one_based() {
    let layout = OpenMcRunLayout::single_rank(1, 500, 1);
    // generation 3, history 5: transport seed 3 * 500 + 6.
    assert_eq!(
        layout.particle_seeds(4, 1, 6),
        init_particle_seeds(3 * 500 + 6, 1, DEFAULT_STRIDE)
    );
}

const R: f64 = 8.7407;

fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>) {
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            293.6,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.4984e-3,
            },
        ],
        temperature: 293.6,
    }];
    let nucs = vec![
        Nuclide::from_core("U235").expect("U235 in CORE"),
        Nuclide::from_core("U238").expect("U238 in CORE"),
    ];
    (geom, mats, nucs)
}

fn run(settings: &KeffSettings) -> KeffResult {
    let (geom, mats, nucs) = model();
    let src = SourceBox {
        lower: Position::new(-R, -R, -R),
        upper: Position::new(R, R, R),
    };
    run_keff_csg(&geom, &mats, &nucs, src, settings, None)
}

fn base(threads: usize, seeding: Option<SeedingMode>) -> KeffSettings {
    let s = KeffSettings {
        n_particles: 400,
        n_inactive: 3,
        n_active: 6,
        temperature_k: 293.6,
        seed: 20261111,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };
    match seeding {
        Some(m) => s.with_seeding(m),
        None => s,
    }
}

fn bits(r: &KeffResult) -> Vec<u64> {
    r.k_by_generation.iter().map(|k| k.to_bits()).collect()
}

/// Integration: the default is `SeedingMode::Outram` and bit-identical to
/// naming it; `SeedingMode::OpenMc` is a different, reproducible,
/// thread-count-independent run of the same physics.
#[test]
fn openmc_seeding_mode_is_named_pinned_and_not_the_default() {
    assert_eq!(KeffSettings::default().seeding, SeedingMode::Outram);
    let default = run(&base(2, None));
    let outram = run(&base(2, Some(SeedingMode::Outram)));
    assert_eq!(bits(&default), bits(&outram), "naming the default changed bits");

    let mode = SeedingMode::OpenMc { total_gen: 0 };
    let a = run(&base(2, Some(mode)));
    let b = run(&base(2, Some(mode)));
    let one = run(&base(1, Some(mode)));
    assert_eq!(bits(&a), bits(&b), "OpenMc seeding is not reproducible");
    assert_eq!(bits(&a), bits(&one), "OpenMc seeding depends on the thread count");
    assert_ne!(bits(&a), bits(&default), "OpenMc seeding did not reach the histories");
    println!(
        "default k = {:.5} +/- {:.5}; OpenMc seeding k = {:.5} +/- {:.5}",
        default.k_mean, default.k_std, a.k_mean, a.k_std
    );
    let z = (a.k_mean - default.k_mean) / (default.k_std.r_powi(2) + a.k_std.r_powi(2)).sqrt();
    assert!(z.abs() < 4.0, "same physics, different streams: {z:.2} sigma");
}
