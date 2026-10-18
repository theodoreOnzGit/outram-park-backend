//! **The beginner `model` API is a shell, not new physics** (gh #938, #939).
//!
//! # Methodology
//!
//! 1. **Bit for bit.** Godiva (ICSBEP HEU-MET-FAST-001, `vv::godiva`) built
//!    through `model::Model` and run with `MCSettings::Custom` gives the
//!    same `k` per generation, to the last bit, as the hand-built CSG model
//!    (one vacuum sphere, `vv::godiva::material()`) through `run_keff_csg` on
//!    ~~the single-thread backend~~ the matching backend (**since
//!    2026-10-18, gh #955**: the default multi-thread run against
//!    `ComputeType::CpuMultiThread`, and `.single_thread()` against
//!    `ComputeType::CpuSingleThread`) at the same seed. Data: the embedded LOW
//!    tier (`Nuclide::from_core`), so it runs anywhere; the data are the same
//!    on both sides, which is all this test is about.
//! 2. **The preset path runs.** `MCSettings::Fast` on the same model: the
//!    watcher starts at 1 000 neutrons per cycle, raises the population to
//!    5 000, runs 120 active cycles, stays under the 100-cycle inactive cap,
//!    and the result carries the smoke-test statement and the physics
//!    provenance (the LOW tier has no DBRC, and the provenance says so rather
//!    than hiding it).
//! 3. **Correct physics is the default through the beginner API**
//!    (`tests/correct_physics_is_default.rs` pattern; #925 decision "physics
//!    on and hidden"): Godiva from the workspace's ENDF/B-VIII.0 tapes
//!    carries URR probability tables on U-235 and U-238 and DBRC on all three
//!    nuclides, with nothing asked for. `long-tests` (processes three
//!    nuclides, ~2 min).
//!
//! # Results (2026-10-17)
//!
//! 1 and 2 pass (identical `k` traces; Fast: watcher passed and the run
//! completed, numbers printed by the test). 3 passes: URR on U-235/U-238,
//! DBRC on all three. The k values from these runs are not benchmark
//! results (LOW tier; tiny runs).

use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere as SphereSurface, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings as LowKeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

/// A library of three uranium headers (masses only), enough to assemble.
fn header_only_library() -> NuclearData {
    let tape = |za: &str, awr: &str| {
        format!(
            " TPID\n{za:>11}{awr:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u234".to_string(), tape("9.223400+4", "2.320304+2")),
            ("u235".to_string(), tape("9.223500+4", "2.330248+2")),
            ("u238".to_string(), tape("9.223800+4", "2.360058+2")),
        ],
    )
    .expect("three incident-neutron headers")
}

fn godiva_model(data: NuclearData) -> Model {
    let mut m = Model::new(data);
    let core = m.component("Core");
    let heu = Material::from_atom_densities(
        godiva::NUCLIDES
            .iter()
            .map(|(n, _, d)| (Isotope::from_name(n).unwrap(), atoms_per_barn_cm(*d))),
    )
    .named("Godiva HEU");
    m.add(
        Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM))
            .filled_with(heu)
            .in_component(core),
    );
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
    ));
    m
}

fn low_tier() -> Vec<Nuclide> {
    ["U234", "U235", "U238"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("CORE nuclide"))
        .collect()
}

#[test]
fn godiva_through_the_model_matches_the_hand_built_csg_bit_for_bit() {
    let r = godiva::RADIUS_CM;
    let hand = Geometry {
        surfaces: vec![SurfaceKind::Sphere(SphereSurface {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            godiva::TEMPERATURE_K,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let nuclides = low_tier();
    let (n, inactive, active, seed) = (400, 5, 10, 7);
    let low = LowKeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: godiva::TEMPERATURE_K,
        seed,
        ..LowKeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-r, -r, -r),
        upper: Position::new(r, r, r),
    };
    let reference_seq = run_keff_csg(&hand, &[godiva::material()], &nuclides, src, &low, None);
    let low_par = LowKeffSettings {
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)),
        ..low.clone()
    };
    let reference = run_keff_csg(&hand, &[godiva::material()], &nuclides, src, &low_par, None);

    let assembled = godiva_model(header_only_library())
        .assemble()
        .expect("Godiva assembles");
    let custom = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: n,
            inactive,
            active,
            seed,
        }),
        None,
    );
    let run = assembled
        .keff_with(&nuclides, &custom)
        .expect("Godiva runs");
    assert_eq!(
        run.k_by_cycle, reference.k_by_generation,
        "per-generation k must be identical bit for bit"
    );
    assert_eq!(run.keff().mean.to_bits(), reference.k_mean.to_bits());
    assert_eq!(run.keff().std.to_bits(), reference.k_std.to_bits());
    assert!(run.provenance.backend.contains("multi-thread"));
    // The single-thread choice is the single-thread reference, bit for bit.
    let single = assembled
        .keff_with(&nuclides, &custom.clone().single_thread())
        .expect("Godiva runs on one thread");
    assert_eq!(single.k_by_cycle, reference_seq.k_by_generation);
    assert_eq!(single.keff().std.to_bits(), reference_seq.k_std.to_bits());
    assert_eq!(single.provenance.threads, 1);
    assert!(single.provenance.backend.contains("single thread"));
    assert_eq!(run.provenance.active_cycles, active);
    assert_eq!(run.provenance.inactive_cycles, inactive);
    assert!(matches!(
        run.provenance.convergence,
        Convergence::NotWatched { .. }
    ));
    println!(
        "bit-for-bit: model {} vs hand-built {:.5} ± {:.5}",
        run.keff(),
        reference.k_mean,
        reference.k_std
    );

    // The wrong number of nuclides is refused, not indexed past.
    assert!(matches!(
        assembled.keff_with(&nuclides[..2], &KeffSettings::new(MCSettings::Fast, None)),
        Err(RunError::NuclideCount { got: 2, wanted: 3 })
    ));
}

#[test]
fn the_fast_preset_runs_the_watcher_and_says_it_is_a_smoke_test() {
    let assembled = godiva_model(header_only_library()).assemble().unwrap();
    let run = assembled
        .keff_with(&low_tier(), &KeffSettings::new(MCSettings::Fast, None))
        .unwrap();
    let p = &run.provenance;
    println!("{run}");
    assert_eq!(run.particles_by_cycle[0], 1_000);
    assert_eq!(*run.particles_by_cycle.last().unwrap(), 5_000);
    assert_eq!(p.active_cycles, 120);
    assert!(
        p.inactive_cycles <= 100 && p.inactive_cycles >= 40,
        "{}",
        p.inactive_cycles
    );
    eprintln!("INACTIVE_CYCLES {}", p.inactive_cycles);
    assert_eq!(run.k_by_cycle.len(), p.inactive_cycles + p.active_cycles);
    assert!(run
        .entropy_by_cycle
        .iter()
        .all(|h| h.is_some_and(|h| h > 0.0)));
    assert!(p.notes.iter().any(|n| n.contains("SMOKE TEST")));
    assert!(p.notes.iter().any(|n| n.contains("#927")));
    match &p.convergence {
        Convergence::Watched { normality, .. } => match normality {
            outram_mc_libs::model::NormalityCheck::Tested(r) => {
                assert_eq!(r.n, outram_mc_libs::model::run::WATCH_WINDOW_CYCLES);
                assert!(r.p_value.is_finite() && r.k2.is_finite());
                eprintln!("deciding window normality: {r:?}");
            }
            other => panic!("a watched run must carry a normality test: {other:?}"),
        },
        other => panic!("{other:?}"),
    }
    assert_eq!(p.seed, outram_mc_libs::model::run::PRESET_SEED);
    assert!(
        p.physics.iter().all(|n| n.dbrc.is_err()),
        "LOW tier carries no 0 K elastic, and the provenance must say so"
    );
    assert!(
        run.keff().mean > 0.9 && run.keff().mean < 1.1,
        "LOW-tier Godiva is near critical (+~1000 pcm recorded)"
    );
    assert!(p.sigma_batch_means.is_some());
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-234/235/238 from ENDF (~2 min); runs by default, skipped under --no-default-features"
)]
fn godiva_through_the_model_carries_urr_and_dbrc_without_being_asked() {
    if !reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf")
        .exists()
    {
        eprintln!("SKIP: reference tapes not present");
        return;
    }
    let m = godiva_model(NuclearData::workspace_reference().unwrap());
    let a = m.assemble().unwrap();
    let nuclides = a.load_nuclides(m.data()).unwrap();
    let run = a
        .keff_with(
            &nuclides,
            &KeffSettings::new(
                MCSettings::Custom(CustomRun {
                    particles: 200,
                    inactive: 2,
                    active: 3,
                    seed: 1,
                }),
                None,
            ),
        )
        .unwrap();
    for n in &run.provenance.physics {
        assert!(n.dbrc.is_ok(), "{}: DBRC off: {:?}", n.nuclide, n.dbrc);
    }
    for name in ["U235", "U238"] {
        let n = run
            .provenance
            .physics
            .iter()
            .find(|n| n.nuclide == name)
            .unwrap();
        assert!(
            n.urr_probability_tables,
            "{name} has an unresolved range and must carry its probability tables"
        );
    }
    println!("{run}");
}
