//! **Tally derivatives wired into the transport (gh:#683, #263).**
//!
//! # Methodology
//!
//! `physics::derivative_run` accumulates the logarithmic flux derivative along
//! each history through the kernel's `TraceEvent` observer and multiplies each
//! segment's score by it (plus the score's explicit dependence). The checks use
//! quantities whose density derivative is known exactly:
//!
//! - In an infinite (reflective) medium every neutron ends in absorption, and
//!   scaling every density leaves the spectrum shape unchanged, so the
//!   ABSORPTION rate per source particle is `1` (plus any (n,xn) gain) whatever
//!   the density: its derivative is **zero**. This tests the track term, the
//!   collision term and the explicit `1/rho` together: dropping the explicit
//!   term gives `-Phi_total/rho`-sized values, dropping the track or collision
//!   term gives a non-zero drift.
//! - The same holds for a fissile medium in a k-eigenvalue run (fixed fission
//!   source): all source neutrons are absorbed.
//!
//! # Results (2026-11-20, release)
//!
//! | run | absorption rate per source particle | `d/d rho` (expected 0) |
//! |---|---|---|
//! | fixed source, H-1 reflective sphere, 20 x 1000 histories, seed 11 | 1.0125 +- 0.0063 | -1.26 +- 1.63 |
//! | k-eigenvalue, HEU reflective sphere, 2000 x (10 + 20), seed 5 | 1.0004 +- 0.0054 | +0.114 +- 0.358 |
//!
//! Both derivatives are zero within one standard error. The scale that matters:
//! with the explicit `1/rho` term dropped the fixed-source value would be about
//! `+1/rho = +15`, and a dropped track or collision term shifts it by order
//! `Phi/rho ~ 1100`, so the check can fail. The pass criterion is `|d| < 4`
//! standard errors of zero and the rate within `4 sigma + 0.02` of 1 (fixed
//! source) or in `[0.99, 1.3)` (the eigenvalue medium also counts (n,xn) gain).

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::derivative_run::{run_fixed_source_derivative, run_keff_derivative, DerivativeSpec};
use outram_mc_libs::physics::fixed_source::{FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::tally::derivative::{DerivativeVariable, TallyDerivative};
use outram_mc_libs::tally::tally::ScoreType;

fn reflective_sphere(r: f64) -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc: BoundaryType::Reflective })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            CellFill::Material(0),
            Position::ZERO,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[test]
fn the_absorption_rate_derivative_is_zero_in_an_infinite_fixed_source_medium() {
    let nuclides = vec![Nuclide::from_core("H1").unwrap()];
    let rho = 6.6e-2;
    let mats = vec![Material {
        id: 1,
        name: "H".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: rho }],
    }];
    let geom = reflective_sphere(30.0);
    let src = FixedSource::Point { r: Position::ZERO, energy_ev: 2.0e6 };
    let settings = FixedSourceSettings { n_particles: 20000, n_batches: 20, seed: 11, ..Default::default() };
    let spec = DerivativeSpec {
        derivative: TallyDerivative { material_idx: 0, variable: DerivativeVariable::Density },
        score: ScoreType::Absorption,
        score_material: 0,
    };
    let r = run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &spec).unwrap();
    println!(
        "fixed source, H, absorption rate = {:.4} +- {:.4} (expected 1); d/d rho = {:.3} +- {:.3} (expected 0)",
        r.flux, r.flux_std, r.d_flux, r.d_flux_std
    );
    assert!((r.flux - 1.0).abs() < 4.0 * r.flux_std + 0.02, "absorption rate {} +- {}", r.flux, r.flux_std);
    assert!(r.d_flux.abs() < 4.0 * r.d_flux_std, "d/d rho {} +- {}", r.d_flux, r.d_flux_std);
    // Unsupported scores and temperature are refused, not guessed.
    let bad = DerivativeSpec { score: ScoreType::NuFission, ..spec.clone() };
    assert!(run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &bad).is_err());
}

#[test]
fn the_absorption_rate_derivative_is_zero_in_an_infinite_keff_medium() {
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    }];
    let geom = reflective_sphere(8.7407);
    let settings = KeffSettings { n_particles: 2000, n_inactive: 10, n_active: 20, seed: 5, ..KeffSettings::default() };
    let src = SourceBox { lower: Position::new(-3.0, -3.0, -3.0), upper: Position::new(3.0, 3.0, 3.0) };
    let spec = DerivativeSpec {
        derivative: TallyDerivative { material_idx: 0, variable: DerivativeVariable::Density },
        score: ScoreType::Absorption,
        score_material: 0,
    };
    let r = run_keff_derivative(&geom, &mats, &nuclides, src, &settings, &spec).unwrap();
    println!(
        "k-eigenvalue, HEU, absorption rate = {:.4} +- {:.4}; d/d rho = {:.3} +- {:.3} (expected 0)",
        r.flux, r.flux_std, r.d_flux, r.d_flux_std
    );
    assert!(r.flux >= 0.99 && r.flux < 1.3, "absorption rate per source particle {}", r.flux);
    assert!(r.d_flux.abs() < 4.0 * r.d_flux_std + 1e-9, "d/d rho {} +- {}", r.d_flux, r.d_flux_std);
}
