// SPDX-License-Identifier: GPL-3.0

//! **GitHub #461: the CSG k path honours variance reduction on every compute
//! path, and analog fission banks by weight.** Found by the OpenMC-vs-outram-mc
//! transport audit (#407), 2026-09-30.
//!
//! Two defects, both dormant in analog runs (every recorded ICSBEP campaign):
//!
//! 1. `run_keff_csg_par` called an analog-only wrapper, so under
//!    `CpuMultiThread` the run's survival biasing and weight windows were
//!    silently dropped, and `keff_trigger` was checked on the sequential path
//!    only.
//! 2. With survival biasing off and weight windows on, the fission arm banked
//!    `ν̄/k` neutrons whatever the particle's weight. OpenMC's
//!    `create_fission_sites` banks `wgt · ν̄/k` (`src/physics.cpp:180`).
//!
//! # Methodology
//!
//! A bare HEU sphere (Godiva radius 8.7407 cm, U-235/U-238 at the benchmark
//! densities) from the embedded multipole (`Core`) data, so the test needs no
//! reference data. Pass criteria fixed before running:
//!
//! - **parallel honours VR**: `CpuMultiThread(Fixed(2))` with survival biasing
//!   gives a k that is not bit-identical to the analog one. Before the fix the
//!   two were identical, because the setting never reached the kernel.
//! - **parallel honours the trigger**: a trigger any two active generations
//!   satisfy (σ ≤ 1) stops the run after `n_inactive + 2` generations. Before
//!   the fix the parallel run went the full `n_inactive + n_active`.
//! - **weighted analog fission is unbiased**: energy-resolved weight windows
//!   (below 100 keV the window sits ten times lower than above it, so every
//!   fission neutron that slows below 100 keV is split about 7 ways) give a k
//!   within 4σ of the analog k. Before the fix each split daughter banked the
//!   full `ν̄/k`, which inflates k by a large factor.
//!
//! # Results
//!
//! Recorded in the printout; see the #461 comment of 2026-09-30.

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
use outram_mc_libs::physics::variance_reduction::VarianceReduction;
use outram_mc_libs::physics::weight_windows::WeightWindows;
use outram_mc_libs::tally::mesh::RegularMesh;
use outram_mc_libs::tally::trigger::{Trigger, TriggerMetric};

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

fn base(compute: ComputeType) -> KeffSettings {
    KeffSettings {
        n_particles: 2000,
        n_inactive: 10,
        n_active: 30,
        temperature_k: 293.6,
        seed: 20260930,
        compute,
        ..KeffSettings::default()
    }
}

fn par() -> ComputeType {
    ComputeType::CpuMultiThread(ThreadCount::Fixed(2))
}

#[test]
fn parallel_path_applies_survival_biasing() {
    let analog = run(&base(par()));
    let biased = run(&KeffSettings {
        variance_reduction: VarianceReduction {
            survival_biasing: true,
            ..VarianceReduction::default()
        },
        ..base(par())
    });
    println!(
        "parallel analog k = {:.5} +/- {:.5}; survival-biased k = {:.5} +/- {:.5}",
        analog.k_mean, analog.k_std, biased.k_mean, biased.k_std
    );
    assert_ne!(
        analog.k_mean.to_bits(),
        biased.k_mean.to_bits(),
        "survival biasing changed nothing on the parallel path: the setting is not \
         reaching the kernel (GitHub #461)"
    );
    let z = (biased.k_mean - analog.k_mean) / (analog.k_std.powi(2) + biased.k_std.powi(2)).sqrt();
    assert!(z.abs() < 4.0, "survival-biased parallel k differs from analog by {z:.2} sigma");
}

#[test]
fn parallel_path_honours_the_keff_trigger() {
    let s = KeffSettings {
        keff_trigger: Some(Trigger {
            metric: TriggerMetric::StandardDeviation,
            threshold: 1.0,
            ignore_zeros: false,
        }),
        ..base(par())
    };
    let r = run(&s);
    assert_eq!(
        r.k_by_generation.len(),
        s.n_inactive + 2,
        "a trigger met by any two active generations did not stop the parallel run \
         (GitHub #461)"
    );
}

#[test]
fn analog_fission_under_weight_windows_is_unbiased() {
    let mesh = RegularMesh {
        lower_left: [-R, -R, -R],
        upper_right: [R, R, R],
        dimension: [1, 1, 1],
    };
    // [energy bin][mesh bin]: below 100 keV the window is ten times lower.
    let ww = WeightWindows::new(mesh, vec![1.0e-5, 1.0e5, 2.0e7], vec![0.05, 0.5], vec![0.15, 1.5])
        .expect("valid windows");
    let vr = VarianceReduction::default().with_weight_windows(ww);
    vr.validate().expect("valid VR");
    let seq = ComputeType::CpuSingleThread;
    let analog = run(&base(seq));
    let windowed = run(&KeffSettings {
        variance_reduction: vr,
        ..base(seq)
    });
    let z = (windowed.k_mean - analog.k_mean)
        / (analog.k_std.powi(2) + windowed.k_std.powi(2)).sqrt();
    println!(
        "analog k = {:.5} +/- {:.5}; weight-windowed k = {:.5} +/- {:.5}; z = {z:+.2}",
        analog.k_mean, analog.k_std, windowed.k_mean, windowed.k_std
    );
    assert!(
        z.abs() < 4.0,
        "weight-windowed k differs from analog by {z:+.2} sigma: fission is not banking \
         by weight (GitHub #461)"
    );
}
