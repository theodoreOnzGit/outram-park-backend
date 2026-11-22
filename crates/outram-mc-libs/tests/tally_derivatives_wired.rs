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
//! | same, NU-FISSION rate per source particle | 2.2074 +- 0.0111 | +0.258 +- 0.730 |
//! | fixed source, H-1 (WMP `Core` tier), 20 x 5000, seed 13, TEMPERATURE derivative | 0.9996 +- 0.0040 | +0.00000 +- 0.00003 per K |
//!
//! The temperature row's discriminating power: the flux itself moves, d flux/dT =
//! 0.0070 +- 0.0017 per K on a flux of 74.70, so a missing explicit
//! `dSigma_a/dT / Sigma_a` term would leave an absorption-rate derivative of about
//! `(d flux/dT)/flux = 9.4e-5`, three standard errors from the measured value.
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
use outram_mc_libs::physics::derivative_run::{run_fixed_source_derivative, run_keff_derivative, run_keff_derivative_with_majorants, DerivativeSpec};
use outram_mc_libs::physics::delta_tracking::Majorant;
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
    let bad = DerivativeSpec { score: ScoreType::Current, ..spec.clone() };
    assert!(run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &bad).is_err());
    // Nu-fission under a nuclide-density derivative: refused (no microscopic nu-fission).
    let nu = DerivativeSpec {
        derivative: TallyDerivative { material_idx: 0, variable: DerivativeVariable::NuclideDensity { nuclide_idx: 0 } },
        score: ScoreType::NuFission,
        score_material: 0,
    };
    assert!(run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &nu).is_err());
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

/// Nu-fission under a DENSITY derivative: in an infinite fissile medium the
/// nu-fission rate per source particle is the medium's multiplication and is
/// density-independent (the spectrum shape does not depend on density), so its
/// derivative is zero. Same medium and criterion as the absorption test.
#[test]
fn the_nu_fission_rate_derivative_is_zero_in_an_infinite_keff_medium() {
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
        score: ScoreType::NuFission,
        score_material: 0,
    };
    let r = run_keff_derivative(&geom, &mats, &nuclides, src, &settings, &spec).unwrap();
    println!(
        "k-eigenvalue, HEU, nu-fission rate = {:.4} +- {:.4}; d/d rho = {:.3} +- {:.3} (expected 0)",
        r.flux, r.flux_std, r.d_flux, r.d_flux_std
    );
    assert!(r.flux > 1.0 && r.flux < 4.0, "nu-fission rate per source particle {}", r.flux);
    assert!(r.d_flux.abs() < 4.0 * r.d_flux_std + 1e-9, "d/d rho {} +- {}", r.d_flux, r.d_flux_std);
}

/// Temperature derivative (gh:#263): in the infinite hydrogen medium the
/// absorption rate per source particle is 1 whatever the cross sections do, so its
/// TEMPERATURE derivative is zero too, while the flux derivative is not. That
/// tests the temperature track term, the collision term and the explicit term
/// `dSigma_a/dT / Sigma_a` together on the windowed-multipole tier (`Core`).
#[test]
fn the_absorption_rate_temperature_derivative_is_zero_in_an_infinite_medium() {
    let nuclides = vec![Nuclide::from_core("H1").unwrap()];
    let mats = vec![Material {
        id: 1,
        name: "H".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 6.6e-2 }],
    }];
    let geom = reflective_sphere(30.0);
    let src = FixedSource::Point { r: Position::ZERO, energy_ev: 2.0e6 };
    let settings = FixedSourceSettings { n_particles: 100000, n_batches: 20, seed: 13, ..Default::default() };
    let var = DerivativeVariable::Temperature;
    let abs = DerivativeSpec {
        derivative: TallyDerivative { material_idx: 0, variable: var },
        score: ScoreType::Absorption,
        score_material: 0,
    };
    let a = run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &abs).unwrap();
    let flux = DerivativeSpec { score: ScoreType::Flux, ..abs.clone() };
    let f = run_fixed_source_derivative(&geom, &mats, &nuclides, &src, &settings, &flux).unwrap();
    println!(
        "T derivative, H-1: absorption rate {:.4} +- {:.4}, d/dT = {:.5} +- {:.5} (expected 0); flux {:.3}, d flux/dT = {:.4} +- {:.4}",
        a.flux, a.flux_std, a.d_flux, a.d_flux_std, f.flux, f.d_flux, f.d_flux_std
    );
    assert!((a.flux - 1.0).abs() < 4.0 * a.flux_std + 0.02);
    assert!(a.d_flux.abs() < 4.0 * a.d_flux_std + 1e-12, "d abs/dT = {} +- {}", a.d_flux, a.d_flux_std);
}

/// **Delta-tracked region (gh:#263).** The same zero-derivative identity as above
/// in an infinite reflective HEU medium, but tracked by delta tracking against a
/// majorant with 50 % headroom: the derivative then comes only from the
/// accept/reject terms at tentative sites plus the real-collision term, and the
/// absorption rate per source particle must still be a density-independent 1.
/// Result: see the printed line (run with `--nocapture`); gate is 4 sigma.
#[test]
fn the_absorption_rate_derivative_is_zero_in_an_infinite_delta_tracked_medium() {
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
    let mut geom = reflective_sphere(8.7407);
    geom.cells[0] = geom.cells[0].clone().delta_tracked(0);
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..512).map(|i| (lo.ln() + (hi.ln() - lo.ln()) * i as f64 / 511.0).exp()).collect();
    let maj = Majorant::over_indices(&mats, &[0], &nuclides, &grid, 0.5);
    let settings = KeffSettings { n_particles: 2000, n_inactive: 10, n_active: 20, seed: 5, ..KeffSettings::default() };
    let src = SourceBox { lower: Position::new(-3.0, -3.0, -3.0), upper: Position::new(3.0, 3.0, 3.0) };
    let spec = DerivativeSpec {
        derivative: TallyDerivative { material_idx: 0, variable: DerivativeVariable::Density },
        score: ScoreType::Absorption,
        score_material: 0,
    };
    let r = run_keff_derivative_with_majorants(&geom, &mats, &nuclides, &[maj], src, &settings, &spec).unwrap();
    println!(
        "delta-tracked k-eigenvalue, HEU, absorption rate = {:.4} +- {:.4}; d/d rho = {:.3} +- {:.3} (expected 0)",
        r.flux, r.flux_std, r.d_flux, r.d_flux_std
    );
    assert!(r.flux >= 0.99 && r.flux < 1.3, "absorption rate per source particle {}", r.flux);
    assert!(r.d_flux.abs() < 4.0 * r.d_flux_std + 1e-9, "d/d rho {} +- {}", r.d_flux, r.d_flux_std);
}
