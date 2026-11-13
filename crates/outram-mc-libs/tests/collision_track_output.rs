// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **Collision tracks and MCPL files, gh:#1392.**
//!
//! # Methodology
//!
//! - **Off by default, no random number drawn.** A two-cell model (an HEU
//!   sphere, cell 22, material 1, inside a water shell, cell 33, material 11,
//!   both in universe 77; CORE-tier U-235/U-238/H-1/O-16) is run by
//!   `CsgPowerIteration` four ways: `step` (no extras), `step_scored` with an
//!   empty `ExtraScoring`, `step_scored` recording collision tracks, and
//!   `step_par` recording them, in the default seeding and in
//!   `SeedingMode::OpenMc`. Pass criterion: every generation's `k`, `k_mean`
//!   and bank size **bit-identical** to the run without recording.
//! - **What is recorded.** The bank holds active generations only, is capped
//!   at `max_collisions`, carries the model's ids (cells 22/33, materials
//!   1/11, universe 77), PDG nuclide ids, MTs of real channels (2, 18, 101, inelastic), weight 0 and
//!   `dE = 0` on an absorption, 1-based particle ids, and the parallel route
//!   records exactly what the sequential one records for the same histories.
//! - **Files.** The bank is written as `collision_track.h5` (njoy codec) and
//!   `.mcpl` and read back. With `OUTRAM_COLLISION_TRACK_OUT=<dir>` the files
//!   are kept there for `openmc.read_collision_track_hdf5` and the `mcpl`
//!   Python reader (`verification_and_validation/collision_track_mcpl/`).
//! - **Upstream's MCPL fixture**
//!   (`tests/regression_tests/surface_source/surface_source_true.mcpl`,
//!   OpenMC `8f79d9d0`, MIT, written by `libmcpl` from OpenMC 0.15.3-dev149):
//!   decoded, re-encoded, compared byte for byte; gzipped and read again; and
//!   taken through OpenMC's own conversions (`mcpl_source_sites`,
//!   `write_mcpl_source_point`).
//!
//! # Results
//!
//! See `verification_and_validation/collision_track_mcpl/README.md`.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::collision_track::{
    write_collision_track_file, CollisionTrackConfig, CollisionTrackSite,
};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{
    CsgGenerationReport, CsgPowerIteration, ExtraScoring, SourceBox,
};
use outram_mc_libs::rng::openmc_seeding::SeedingMode;
use outram_mc_libs::source::mcpl;

const R_CORE: f64 = 7.0;
const R_OUT: f64 = 10.0;

fn sphere(r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc })
}

fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>) {
    let inside = |s| RegionToken::HalfSpace { surface_idx: s, sense: HalfSpaceSense::Inside };
    let outside = |s| RegionToken::HalfSpace { surface_idx: s, sense: HalfSpaceSense::Outside };
    let geom = Geometry {
        surfaces: vec![sphere(R_CORE, BoundaryType::Transmissive), sphere(R_OUT, BoundaryType::Vacuum)],
        cells: vec![
            Cell::material(22, vec![inside(0)], 0, 293.6),
            Cell::material(33, vec![outside(0), inside(1)], 1, 293.6),
        ],
        universes: vec![Universe { id: 77, cell_indices: vec![0, 1] }],
        lattices: vec![],
        root_universe: 0,
    };
    let comp = |i, d| NuclideComponent { nuclide_idx: i, atom_density: d };
    let mats = vec![
        Material {
            id: 1,
            name: "HEU".into(),
            components: vec![comp(0, 4.4994e-2), comp(1, 2.4984e-3)],
            temperature: 293.6,
        },
        Material {
            id: 11,
            name: "water".into(),
            components: vec![comp(2, 6.687e-2), comp(3, 3.3435e-2)],
            temperature: 293.6,
        },
    ];
    let nucs = ["U235", "U238", "H1", "O16"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("CORE nuclide"))
        .collect();
    (geom, mats, nucs)
}

fn settings(seeding: SeedingMode) -> KeffSettings {
    KeffSettings {
        n_particles: 300,
        n_inactive: 2,
        n_active: 4,
        temperature_k: 293.6,
        seed: 1392,
        ..KeffSettings::default()
    }
    .with_seeding(seeding)
}

fn src() -> SourceBox {
    SourceBox {
        lower: Position::new(-R_CORE, -R_CORE, -R_CORE),
        upper: Position::new(R_CORE, R_CORE, R_CORE),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    Step,
    Scored,
    Par,
}

/// One run: per-generation `(k bits, k_mean bits, bank size)` and, when
/// recording, the bank's records.
fn run(
    seeding: SeedingMode,
    route: Route,
    record: Option<CollisionTrackConfig>,
) -> (Vec<(u64, Option<(u64, u64)>, usize)>, Vec<CollisionTrackSite>) {
    let (geom, mats, nucs) = model();
    let mut it = CsgPowerIteration::new(&geom, &mats, &nucs, src(), &settings(seeding));
    let mut extra = match record {
        Some(c) => ExtraScoring::new().with_collision_track(c),
        None => ExtraScoring::new(),
    };
    let mut out = Vec::new();
    loop {
        let rep: Option<CsgGenerationReport> = match route {
            Route::Step => it.step(&geom, &mats, &nucs),
            Route::Scored => it.step_scored(&geom, &mats, &nucs, &mut extra).map(|(r, _)| r),
            Route::Par => it.step_par(&geom, &mats, &nucs, Some(&mut extra)).map(|(r, _)| r),
        };
        let Some(r) = rep else { break };
        if r.active {
            extra.flush();
        } else {
            extra.discard();
        }
        out.push((
            r.k.to_bits(),
            r.k_mean.map(|(m, s)| (m.to_bits(), s.to_bits())),
            r.bank_size,
        ));
    }
    let sites = extra.collision_track().map(|b| b.sites().to_vec()).unwrap_or_default();
    (out, sites)
}

fn all() -> CollisionTrackConfig {
    CollisionTrackConfig { max_collisions: 1_000_000, ..CollisionTrackConfig::default() }
}

/// **Recording is off by default and changes no history.**
#[test]
fn collision_tracks_change_no_history_bit_for_bit() {
    assert!(ExtraScoring::new().collision_track().is_none(), "off by default");
    for seeding in [SeedingMode::Outram, SeedingMode::OpenMc { total_gen: 0 }] {
        let (base, _) = run(seeding, Route::Step, None);
        let (scored, _) = run(seeding, Route::Scored, None);
        let (rec, sites) = run(seeding, Route::Scored, Some(all()));
        assert_eq!(base, scored);
        assert_eq!(base, rec, "recording collision tracks changed the run");
        assert!(sites.len() > 1000, "only {} records", sites.len());

        let (par0, _) = run(seeding, Route::Par, None);
        let (par1, par_sites) = run(seeding, Route::Par, Some(all()));
        assert_eq!(par0, par1, "recording changed the parallel run");
        if seeding != SeedingMode::Outram {
            // Under OpenMC seeding the sequential and parallel routes run the
            // same histories, so they must record the same collisions.
            assert_eq!(base, par0);
            assert_eq!(sites, par_sites, "parallel chunks recorded differently");
        }
        println!("{seeding:?}: {} generations, {} records", base.len(), sites.len());
    }
}

/// **What is recorded is what upstream records.**
#[test]
fn records_carry_upstreams_fields() {
    let (_, sites) = run(SeedingMode::Outram, Route::Scored, Some(all()));
    let pdg = |z: i32, a: i32| 1_000_000_000 + 10_000 * z + 10 * a;
    let mut mts = std::collections::BTreeMap::new();
    for s in &sites {
        *mts.entry(s.event_mt).or_insert(0) += 1;
        let scatter = matches!(s.event_mt, 2 | 5 | 16 | 17 | 51..=91);
        assert!(scatter || matches!(s.event_mt, 18 | 101), "MT {}", s.event_mt);
        assert_eq!(s.universe_id, 77);
        assert!(matches!((s.cell_id, s.material_id), (22, 1) | (33, 11)), "{s:?}");
        assert!([pdg(92, 235), pdg(92, 238), pdg(1, 1), pdg(8, 16)].contains(&s.nuclide_id));
        assert_eq!(s.particle, 2112);
        assert!((1..=300).contains(&s.parent_id));
        assert!(s.n_collision >= 1);
        assert!(s.progeny_id >= 0);
        if !scatter {
            assert_eq!((s.wgt, s.de), (0.0, 0.0), "an absorbed neutron: {s:?}");
        } else {
            assert_eq!(s.wgt, 1.0);
        }
        let n = (s.u[0] * s.u[0] + s.u[1] * s.u[1] + s.u[2] * s.u[2]).sqrt();
        assert!((n - 1.0).abs() < 1e-12);
        assert!(s.e > 0.0 && s.time >= 0.0);
    }
    println!("records by MT: {mts:?}");

    // Filters and the cap, through the same run.
    let only = CollisionTrackConfig {
        max_collisions: 50,
        cell_ids: vec![22],
        nuclides: vec!["U235".into()],
        ..CollisionTrackConfig::default()
    }
    .with_reactions(&["(n,fission)"])
    .unwrap();
    let (_, few) = run(SeedingMode::Outram, Route::Scored, Some(only));
    assert_eq!(few.len(), 50);
    assert!(few.iter().all(|s| s.event_mt == 18 && s.cell_id == 22 && s.nuclide_id == pdg(92, 235)));
    // The same records, in the same order, as the unfiltered run's matching ones.
    let want: Vec<_> = sites
        .iter()
        .filter(|s| s.event_mt == 18 && s.cell_id == 22 && s.nuclide_id == pdg(92, 235))
        .take(50)
        .copied()
        .collect();
    assert_eq!(few, want);
}

fn out_dir() -> (std::path::PathBuf, bool) {
    match std::env::var("OUTRAM_COLLISION_TRACK_OUT") {
        Ok(d) => (std::path::PathBuf::from(d), true),
        Err(_) => (std::env::temp_dir().join("outram_collision_track_test"), false),
    }
}

/// **The bank becomes OpenMC's files at the run boundary, and reads back.**
#[test]
fn the_bank_writes_hdf5_and_mcpl_and_reads_back() {
    let (dir, keep) = out_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let prefix = format!("{}/", dir.display());
    for mcpl_write in [false, true] {
        let cfg = CollisionTrackConfig { max_collisions: 200, mcpl_write, ..CollisionTrackConfig::default() };
        let (geom, mats, nucs) = model();
        let mut it = CsgPowerIteration::new(&geom, &mats, &nucs, src(), &settings(SeedingMode::Outram));
        let mut extra = ExtraScoring::new().with_collision_track(cfg);
        let n_gen = 6;
        let mut files = Vec::new();
        for g in 0..n_gen {
            let (r, _) = it.step_scored(&geom, &mats, &nucs, &mut extra).unwrap();
            if r.active { extra.flush() } else { extra.discard() }
            if let Some(f) = extra.collision_track_mut().unwrap().flush_bank(g + 1 == n_gen) {
                files.push(f);
            }
        }
        assert_eq!(files.len(), 1, "one file at max_files = 1");
        let f = &files[0];
        assert_eq!(f.sites.len(), 200);
        let path = write_collision_track_file(&prefix, f).unwrap();
        if keep && !mcpl_write {
            // The records as the port holds them, every f64 in its shortest
            // round-trip form, for the Python reader to compare bit for bit.
            let mut t = String::new();
            for s in &f.sites {
                let d: Vec<String> = s.r.iter().chain(&s.u).chain(&[s.e, s.de, s.time, s.wgt])
                    .map(|v| format!("{v:?}"))
                    .collect();
                t += &format!(
                    "{},{},{},{},{},{},{},{},{},{},{}\n",
                    d.join(","),
                    s.event_mt,
                    s.delayed_group,
                    s.cell_id,
                    s.nuclide_id,
                    s.material_id,
                    s.universe_id,
                    s.n_collision,
                    s.particle,
                    s.parent_id,
                    s.progeny_id
                );
            }
            std::fs::write(dir.join("collision_track_sites.csv"), t).unwrap();
        }
        if mcpl_write {
            assert!(path.ends_with("collision_track.mcpl"));
            let m = mcpl::read_mcpl_file(&path).unwrap();
            assert_eq!(m.particles.len(), 200);
            assert_eq!(m.header.blobs.len(), 200);
            assert_eq!(m.header.blobs[7].1, mcpl::collision_track_blob(&f.sites[7]).into_bytes());
            for (p, s) in m.particles.iter().zip(&f.sites) {
                assert_eq!(p.position.map(|x| x as f32), s.r.map(|x| x as f32));
                assert_eq!(p.ekin as f32, (s.e * 1e-6) as f32);
                assert_eq!(p.weight as f32, s.wgt as f32);
            }
        } else {
            assert!(path.ends_with("collision_track.h5"));
            let back = njoy_outram_park_fork::hdf5::collision_track::read_collision_track_h5(&path).unwrap();
            assert_eq!(&back, &f.sites, "HDF5 round trip is exact");
        }
    }
    if !keep {
        std::fs::remove_dir_all(&dir).ok();
    }
}

const FIXTURE: &[u8] = include_bytes!("data/openmc_mcpl/surface_source_true.mcpl");

/// **Upstream's MCPL fixture round-trips byte for byte**, plain and gzipped,
/// and through OpenMC's own source conversions.
#[test]
fn upstream_mcpl_fixture_round_trips_byte_for_byte() {
    let f = mcpl::decode_mcpl(FIXTURE).unwrap();
    assert_eq!(f.particles.len(), 1000);
    assert_eq!(f.header.source_name, "OpenMC 0.15.3-dev149");
    assert_eq!(f.header.stat_sum(mcpl::OPENMC_NP1_KEY), Some(10000.0));
    assert!(f.header.single_precision && f.warnings.is_empty());
    assert!(f.particles.iter().all(|p| p.pdgcode == 2112 && p.weight == 1.0));

    let again = mcpl::encode_mcpl(&f).unwrap();
    let differing = again.iter().zip(FIXTURE).filter(|(a, b)| a != b).count();
    println!("re-encoded {} bytes, {differing} differ", again.len());
    assert_eq!(again.len(), FIXTURE.len());
    assert_eq!(again, FIXTURE, "decode then encode must reproduce libmcpl's bytes");

    let gz = mcpl::gzip(FIXTURE);
    assert_eq!(mcpl::decode_mcpl(&gz).unwrap(), f);

    // OpenMC's route: MCPL -> SourceSite (eV, s) -> write_mcpl_source_point.
    let sites = mcpl::mcpl_source_sites(FIXTURE).unwrap();
    let mut out = mcpl::write_mcpl_source_point(&sites, Some(10000)).unwrap();
    out.header.source_name = f.header.source_name.clone();
    let bytes = mcpl::encode_mcpl(&out).unwrap();
    let differing = bytes.iter().zip(FIXTURE).filter(|(a, b)| a != b).count();
    println!("through OpenMC's source conversions: {differing} bytes differ");
    assert_eq!(bytes, FIXTURE, "the eV/MeV and s/ms conversions must not move an f32");

    let (dir, keep) = out_dir();
    if keep {
        std::fs::create_dir_all(&dir).unwrap();
        mcpl::write_mcpl_file(&format!("{}/surface_source_port", dir.display()), &f).unwrap();
        let mut sp = mcpl::write_mcpl_source_point(&sites, Some(10000)).unwrap();
        sp.header.single_precision = false;
        mcpl::write_mcpl_file(&format!("{}/source_point_double", dir.display()), &sp).unwrap();
        std::fs::write(dir.join("surface_source_port.mcpl.gz"), mcpl::gzip(&again)).unwrap();
    }
}
