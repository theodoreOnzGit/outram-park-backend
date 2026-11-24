//! **k is bit-identical on every CPU route with the union-grid lookup**
//! (gh:#1295).
//!
//! # Methodology
//!
//! HIGH-tier Godiva (ENDF/B-VIII.0 U-234, U-235, U-238 from
//! `reference-data/endf/`, RECONR/BROADR at 0.001, 293.6 K, ICSBEP atom
//! densities from `vv::godiva`) is run twice per route at one seed, with the
//! nuclides at `SpeedTier::Fast` (the union-grid lookup, `material::log_grid`)
//! and at `SpeedTier::Standard` (the per-reaction search, the oracle). The
//! routes are the ones the issue names, which all read cross sections through
//! the same `Nuclide` lookup:
//!
//! - `run_keff` on `CpuSingleThread` (`run_keff_cpu_single`) and on
//!   `CpuMultiThread(3)` (`run_keff_cpu_multi`);
//! - `run_keff_csg_par` (3 threads) on a vacuum sphere of the same radius;
//! - `run_keff_delta_par_in` (3 threads) on a sphere of the same radius,
//!   delta tracking against a majorant built from each tier's own nuclides.
//!   `DeltaDomain::Sphere` is **reflective**, so this route's k is near
//!   Godiva's `k_inf` (~2.27), not its `k_eff`; it is here for the lookup, not
//!   the benchmark.
//!
//! Pass: `k_mean` and `k_std` bit-identical between the tiers on each route.
//! 400 histories x [4 inactive + 6 active]: enough collisions to reach every
//! lookup path many thousands of times; the k value itself is not judged.
//!
//! # Results
//!
//! 2026-11-24 (Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB, Linux 6.18, CPU
//! only): all four routes bit-identical, `Fast` = `Standard`:
//!
//! | route | k_mean +- k_std |
//! |---|---|
//! | `run_keff` single | 0.992178 +- 0.013743 |
//! | `run_keff` multi(3) | 1.033633 +- 0.020705 |
//! | `run_keff_csg_par`(3) | 1.033160 +- 0.024461 |
//! | `run_keff_delta_par_in`(3), reflective | 2.280468 +- 0.037692 |
//!
//! (400 histories: the values are noisy and are not a benchmark result.) The
//! timing on the full 5000 x [40 + 120] run is in
//! `docs/profiling/log_grid_index_1295.md`.
#![cfg(feature = "std")] // reads ENDF tapes, threads (gh:#1605)

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_par_in, DeltaDomain};
use outram_mc_libs::physics::keff::{run_keff, ComputeType, KeffResult, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_par, SourceBox};
use outram_mc_libs::vv::godiva;
use petir::mathf::RealMath;

fn sphere(r_cm: f64) -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_cm, bc: BoundaryType::Vacuum })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            godiva::TEMPERATURE_K,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn grid() -> Vec<f64> {
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    (0..512).map(|i| (lo.r_ln() + (hi.r_ln() - lo.r_ln()) * i as f64 / 511.0).r_exp()).collect()
}

/// `(k_mean, k_std)` of each route for one tier's nuclides.
fn routes(nucs: &[Nuclide]) -> Vec<(&'static str, KeffResult)> {
    let r = godiva::RADIUS_CM;
    let mat = godiva::material();
    let base = KeffSettings {
        n_particles: 400,
        n_inactive: 4,
        n_active: 6,
        temperature_k: godiva::TEMPERATURE_K,
        ..KeffSettings::default()
    };
    let single = KeffSettings { compute: ComputeType::CpuSingleThread, ..base.clone() };
    let multi = KeffSettings { compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)), ..base.clone() };
    let src = SourceBox { lower: Position::new(-3.0, -3.0, -3.0), upper: Position::new(3.0, 3.0, 3.0) };
    let mats = [mat.clone()];
    let maj = Majorant::over_indices(&mats, &[0], nucs, &grid(), 0.1);
    let zone = move |p: Position| if p.x * p.x + p.y * p.y + p.z * p.z <= r * r { Some(0) } else { None };
    vec![
        ("run_keff single", run_keff(r, &mat, nucs, &single)),
        ("run_keff multi(3)", run_keff(r, &mat, nucs, &multi)),
        ("run_keff_csg_par(3)", run_keff_csg_par(&sphere(r), &mats, nucs, &[], None, src, &base, None, &[], None, ThreadCount::Fixed(3))),
        (
            "run_keff_delta_par_in(3)",
            run_keff_delta_par_in(DeltaDomain::Sphere { radius: r }, &mats, nucs, &maj, zone, &base, ThreadCount::Fixed(3)),
        ),
    ]
}

#[test]
fn every_cpu_route_gives_the_same_k_with_the_union_grid_lookup() {
    let mut fast = Vec::new();
    for (name, file, _) in godiva::NUCLIDES.iter() {
        let Some(p) = reference_endf(file) else {
            eprintln!("SKIP: {file} absent");
            return;
        };
        fast.push(Nuclide::from_endf_file_with_speed(&p, name, godiva::TEMPERATURE_K, SpeedTier::Fast).expect("nuclide"));
    }
    let standard: Vec<Nuclide> = fast.iter().cloned().map(|n| n.with_speed(SpeedTier::Standard)).collect();
    for ((name, a), (_, b)) in routes(&fast).into_iter().zip(routes(&standard)) {
        println!("{name}: fast k = {:.6} +- {:.6}, standard k = {:.6} +- {:.6}", a.k_mean, a.k_std, b.k_mean, b.k_std);
        assert_eq!(a.k_mean.to_bits(), b.k_mean.to_bits(), "{name}: k_mean differs");
        assert_eq!(a.k_std.to_bits(), b.k_std.to_bits(), "{name}: k_std differs");
    }
}
