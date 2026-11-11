//! Paired diagnostic for gh:#1386 / #1444 (c): Godiva (CSG, entropy mesh, NO trigger)
//! with OpenMC per-collision `create_fission_sites` banking (the default) against
//! the legacy bank-at-fission mode, same settings.
//!
//! **Methodology.** The model of `godiva_csg_kernel_sets_paired` (bare U-235/238/234
//! sphere, R = 8.7407 cm, vacuum, 4x4x4 entropy mesh, source box +-3 cm) run with
//! `run_keff_csg_hybrid`, 1000 particles, 20 inactive, 100 ACTIVE generations fixed.
//! Arms switch `physics::scatter::set_fission_banking_override` (PerCollision vs
//! AtFission). Seeds 1, 2, 3. Statistic `d = k_percoll - k_atfission` over
//! `sqrt(se_a^2 + se_b^2)`.
//!
//! **Prediction written before running (2026-11-11):** both estimators bank the same
//! expected number of sites per collision with fissionable material, so the
//! eigenvalue is unchanged in expectation: |d| < 3 sigma for the seed mean (and per
//! seed, ~0.3 % chance of one exceedance). Not a gate on physics: PRINTS the table
//! (`--nocapture`); asserts only that both arms ran 120 generations.
//!
//! # Result
//! 2026-11-11, rustc 1.99.1 release, 2 s, 4 vCPU shared:
//!
//! | seed | per-collision | at-fission | d | sigma |
//! |---|---|---|---|---|
//! | 1 | 1.01575 +/- 0.00355 | 1.01312 +/- 0.00408 | +263 pcm | +0.49 |
//! | 2 | 1.01613 +/- 0.00338 | 1.01116 +/- 0.00397 | +497 pcm | +0.95 |
//! | 3 | 1.01344 +/- 0.00293 | 1.00304 +/- 0.00408 | +1041 pcm | +2.07 |
//! | mean | 1.01511 | 1.00911 | +600 pcm | +1.99 |
//!
//! Prediction met (every |d| < 3 sigma), but all three differences are positive and
//! the mean is +1.99 sigma: not resolved, not dismissed. The two modes consume the
//! stream differently, so the arms are independent; a systematic shift below ~1000
//! pcm is not excluded at this statistics. A longer run would settle it.

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

fn nuclides(_legacy: bool) -> Vec<Nuclide> {
    ["U234", "U235", "U238"]
        .iter()
        .map(|n| {
            let nu = Nuclide::from_core(n).unwrap();
            nu
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

/// Methodology and prediction in the module doc; prints the paired table.
#[test]
fn godiva_csg_per_collision_vs_at_fission_banking_paired() {
    use outram_mc_libs::physics::scatter::{set_fission_banking_override, FissionBanking};
    let mut ds = Vec::new();
    let (mut sa, mut sb) = (0.0, 0.0);
    for seed in [1u64, 2, 3] {
        set_fission_banking_override(Some(FissionBanking::PerCollision));
        let (ka, ea, na) = run(false, seed);
        set_fission_banking_override(Some(FissionBanking::AtFission));
        let (kb, eb, nb) = run(true, seed);
        set_fission_banking_override(None);
        assert_eq!((na, nb), (120, 120));
        let sig = (ea * ea + eb * eb).sqrt();
        eprintln!(
            "PAIRED seed {seed}: per-collision {ka:.5} +/- {ea:.5}  at-fission {kb:.5} +/- {eb:.5}  d = {:+.0} pcm = {:+.2} sigma",
            (ka - kb) * 1e5,
            (ka - kb) / sig
        );
        ds.push((ka - kb, sig));
        sa += ka / 3.0;
        sb += kb / 3.0;
    }
    let sm = (ds.iter().map(|(_, s)| s * s).sum::<f64>()).sqrt() / 3.0;
    eprintln!(
        "PAIRED mean of 3 seeds: per-collision {sa:.5} at-fission {sb:.5} d = {:+.0} pcm = {:+.2} sigma",
        (sa - sb) * 1e5,
        (sa - sb) / sm
    );
}
