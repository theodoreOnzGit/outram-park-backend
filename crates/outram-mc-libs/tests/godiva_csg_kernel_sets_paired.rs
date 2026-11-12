//! Paired diagnostic for gh:#1457: Godiva (CSG, entropy mesh, NO trigger) under the
//! OpenMC-faithful kernels and under the legacy kernels, same settings.
//!
//! **Methodology.** The model of `stats_move_fingerprints::godiva_csg_entropy_trigger`
//! (bare U-235/238/234 sphere, R = 8.7407 cm, vacuum, 4x4x4 entropy mesh, source box
//! +-3 cm) run with `run_keff_csg_hybrid`, 1000 particles, 20 inactive, 100 ACTIVE
//! generations fixed (no trigger, so the standard error is from 100 values, not 3).
//! The legacy arm applies `Nuclide::with_legacy_scatter_kernels` to every nuclide
//! (per-nuclide, so both arms run in one process). Seeds 1, 2, 3 per arm; the seed
//! fixes the stream, but the two kernel sets draw differently so the arms are
//! statistically independent. Statistic: `d = k_openmc - k_legacy` over
//! `sqrt(se_a^2 + se_b^2)`, per seed and for the seed-averaged pair.
//!
//! **Prediction written before running (2026-11-11):** if the kernel sets are
//! statistically equivalent, `|d| < 3 sigma_combined` for the seed-averaged pair
//! (and for each seed, with ~0.3 % chance of a single exceedance). The test does
//! not gate the physics; it PRINTS the numbers (use `--nocapture`), because the
//! finding is the measured difference. It asserts both arms ran 120 generations so
//! the denominator is as stated.
//!
//!
//! # Result (2026-11-11; Intel Xeon 4 vCPU shared, rustc 1.99.1, release, 2.5 s)
//!
//! | seed | OpenMC kernels | legacy kernels | d | sigma |
//! |---|---|---|---|---|
//! | 1 | 1.01389 +/- 0.00366 | 1.01276 +/- 0.00448 | +113 pcm | +0.19 |
//! | 2 | 1.00899 +/- 0.00448 | 1.00635 +/- 0.00390 | +264 pcm | +0.44 |
//! | 3 | 1.01797 +/- 0.00358 | 1.01500 +/- 0.00366 | +297 pcm | +0.58 |
//! | mean of 3 | 1.01362 | 1.01137 | +225 pcm | +0.69 |
//!
//! Prediction met: every |d| < 3 sigma. All three differences are positive
//! (sign agrees with `godiva_run_keff`'s +75 pcm), but jointly +0.69 sigma, so no
//! kernel shift is resolved at this statistics (sigma of the mean ~330 pcm). The
//! -3.52 sigma of `godiva_csg_entropy_trigger` came from the 3-active-generation
//! standard error, not from the kernels; one-kernel-at-a-time localisation was
//! therefore not triggered. This does not exclude a shift below ~1000 pcm (3 sigma).

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::tally::mesh::RegularMesh;

fn nuclides(legacy: bool) -> Vec<Nuclide> {
    ["U234", "U235", "U238"]
        .iter()
        .map(|n| {
            let nu = Nuclide::from_core(n).unwrap();
            if legacy {
                nu.with_legacy_scatter_kernels()
            } else {
                nu
            }
        })
        .collect()
}

fn run(legacy: bool, seed: u64) -> (f64, f64, usize) {
    let material = Material {
        id: 1,
        name: "Godiva".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    };
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 8.7407,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            293.6,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };
    let mesh = RegularMesh {
        lower_left: [-8.7407; 3],
        upper_right: [8.7407; 3],
        dimension: [4, 4, 4],
    };
    let settings = KeffSettings {
        n_particles: 1000,
        n_inactive: 20,
        n_active: 100,
        seed,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-3.0, -3.0, -3.0),
        upper: Position::new(3.0, 3.0, 3.0),
    };
    let r = run_keff_csg_hybrid(&geom, &[material], &nuclides(legacy), &[], Some(&mesh), src, &settings, None);
    (r.k_mean, r.k_std, r.k_by_generation.len())
}

/// Methodology and prediction in the module doc; this prints the paired table.
#[test]
fn godiva_csg_openmc_vs_legacy_kernels_paired() {
    let mut ds = Vec::new();
    let (mut sa, mut sb) = (0.0, 0.0);
    for seed in [1u64, 2, 3] {
        let (ka, ea, na) = run(false, seed);
        let (kb, eb, nb) = run(true, seed);
        assert_eq!((na, nb), (120, 120));
        let sig = (ea * ea + eb * eb).sqrt();
        eprintln!(
            "PAIRED seed {seed}: openmc {ka:.5} +/- {ea:.5}  legacy {kb:.5} +/- {eb:.5}  d = {:+.0} pcm = {:+.2} sigma",
            (ka - kb) * 1e5,
            (ka - kb) / sig
        );
        ds.push((ka - kb, sig));
        sa += ka / 3.0;
        sb += kb / 3.0;
    }
    // Mean of three independent differences: sigma_mean = sqrt(sum sig^2)/3.
    let sm = (ds.iter().map(|(_, s)| s * s).sum::<f64>()).sqrt() / 3.0;
    eprintln!(
        "PAIRED mean of 3 seeds: openmc {sa:.5} legacy {sb:.5} d = {:+.0} pcm = {:+.2} sigma",
        (sa - sb) * 1e5,
        (sa - sb) / sm
    );
}
