//! **P1 scattering moment of hydrogen through real transport (gh:#676).**
//!
//! # Methodology
//!
//! For elastic scattering off a nucleus of mass ratio `A` with isotropic
//! centre-of-mass scattering, the mean LAB scattering cosine is `2/(3A)`
//! (the standard result; derived here for `A = 1`:
//! `mu_lab = sqrt((1 + mu_c)/2)` averages to exactly `2/3` over isotropic `mu_c`). H-1 scatters
//! isotropically in the CM frame below ~100 keV, so the ratio of the first to the
//! zeroth Legendre moment of the scatter matrix, `P1/P0 = <mu_lab>`, must be 2/3.
//!
//! A source of 10 keV neutrons in a reflective sphere of pure H-1 (so every
//! neutron keeps scattering), a tally `[Energy(1 keV-20 keV), EnergyOut(all),
//! Legendre(1)]` scoring `NuScatter`, raw (un-normalised) sums so that the
//! `P0` bin is the number of scatters `N` and the `P1` bin is `sum(mu)`. Incident
//! energies are kept above 1 keV so thermal free-gas motion is not a factor.
//! The lab-cosine standard deviation for `A = 1` is `sqrt(1/2 - 4/9) = 0.2357`
//! (`<mu^2> = 1/2`), so `sigma(<mu>) = 0.2357 / sqrt(N)`; the gate is 5 sigma.
//!
//! # Results (2026-11-20)
//!
//! One run, seed 676 (the first seed tried; not selected): **N = 13046 scatters,
//! P1/P0 = 0.66256 against 2/3 = 0.66667**, a difference of -0.0041, i.e. -2.0
//! sigma (sigma = 0.2357/sqrt(N) = 0.0021), inside the 5-sigma gate of +-0.0103.
//! A -2 sigma draw is unremarkable for one run; it is not a bias claim, and one
//! seed cannot separate the two. Interpretation: the lab cosine reaches the
//! scatter matrix and the Legendre expansion is right through real transport.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::fixed_source::{run_fixed_source, FixedSource, FixedSourceSettings};
use outram_mc_libs::tally::filter::{EnergyFilter, EnergyOutFilter, FilterKind};
use outram_mc_libs::tally::filter_extra::LegendreFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

#[test]
fn hydrogen_p1_over_p0_is_two_thirds_through_transport() {
    let nuclides = vec![Nuclide::from_core("H1").unwrap()];
    let mats = vec![Material {
        id: 1,
        name: "H".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 6.6e-2 }],
    }];
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: 30.0, bc: BoundaryType::Reflective })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            CellFill::Material(0),
            Position::ZERO,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };
    let mut tally = Tally {
        id: 676,
        name: "P_l scatter matrix".into(),
        filters: vec![
            FilterKind::Energy(EnergyFilter { bins: vec![1.0e3, 2.0e4] }),
            FilterKind::EnergyOut(EnergyOutFilter { bins: vec![1.0e-5, 2.0e7] }),
            FilterKind::Legendre(LegendreFilter { order: 1 }),
        ],
        scores: vec![ScoreType::NuScatter],
        bins: vec![TallyBin::default(); 2],
        derivative: None,
    };
    tally.validate().expect("a Legendre filter on the scatter matrix is accepted");
    let src = FixedSource::Point { r: Position::ZERO, energy_ev: 1.0e4 };
    let settings = FixedSourceSettings {
        n_particles: 4000,
        n_batches: 10,
        seed: 676,
        normalise_tallies: false,
        ..Default::default()
    };
    run_fixed_source(&geom, &mats, &nuclides, &src, &settings, Some(&mut tally));

    let (p0, p1) = (tally.bins[0].sum, tally.bins[1].sum);
    assert!(p0 > 1000.0, "scatters counted: {p0}");
    let ratio = p1 / p0;
    let gate = 5.0 * 0.2357 / p0.sqrt();
    println!("H-1 slab: N = {p0:.0} scatters, P1/P0 = {ratio:.5}, expected {:.5}, gate +-{gate:.5}", 2.0 / 3.0);
    assert!((ratio - 2.0 / 3.0).abs() < gate, "P1/P0 = {ratio} vs 2/3 (gate {gate})");
}
