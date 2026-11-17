//! `get_microxs_and_flux` on this crate's transport (gh:#1625): a pin cell
//! (CORE U-235, U-238 and H-1, reflective box), tallied by material and
//! energy, then collapsed per material.
//!
//! Predictions, written before the run:
//! 1. both domains carry flux, and the returned flux is the sum of the
//!    group fluxes;
//! 2. a constant 4 b cross section collapses to 4 b in each domain, to
//!    round-off (the flux normalisation cancels);
//! 3. the moderator's spectrum is softer than the fuel's (the fuel
//!    absorbs thermal neutrons), so a `1/sqrt(E)` cross section collapses
//!    **higher** in the moderator.
//!
//! Measured 2026-11-17 (400 × [8 + 20], 30 log groups 1e-3 eV-20 MeV): all
//! three hold. Fuel flux 3.97e3, 13.9 % below 1 eV, `1/sqrt(E)` collapse
//! 0.6534; moderator 8.60e3, 16.8 %, 0.8196. Constant 4 b to 1e-12 in both.

use petir::mathf::RealMath;
use outram_mc_libs::depletion::microxs::{NuclideReaction, ReactionTable};
use outram_mc_libs::depletion::microxs_transport::get_microxs_and_flux;
use outram_mc_libs::prelude::*;

fn log_grid(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (0..=n)
        .map(|i| (lo.r_ln() + (hi.r_ln() - lo.r_ln()) * i as f64 / n as f64).r_exp())
        .collect()
}

fn pincell(r_fuel: f64, half: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: r_fuel,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: half,
            bc: BoundaryType::Reflective,
        }),
    ];
    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    );
    let moder = Cell::material(
        2,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        1,
        293.6,
    );
    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[test]
fn microxs_and_flux_by_material_from_transport() {
    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 in CORE"),
        Nuclide::from_core("U238").expect("U238 in CORE"),
        Nuclide::from_core("H1").expect("H1 in CORE"),
    ];
    let (r_fuel, half) = (0.4_f64, 0.63_f64);
    let geom = pincell(r_fuel, half);
    let comp = |i: usize, d: f64| NuclideComponent { nuclide_idx: i, atom_density: d };
    let materials = vec![
        Material { id: 1, name: "fuel".into(), temperature: 293.6, components: vec![comp(0, 1.0e-3), comp(1, 2.2e-2)] },
        Material { id: 2, name: "moderator".into(), temperature: 293.6, components: vec![comp(2, 6.6e-2)] },
    ];
    let ks = KeffSettings { n_particles: 400, n_inactive: 8, n_active: 20, ..KeffSettings::default() };
    let src = SourceBox { lower: Position::new(-r_fuel, -r_fuel, -1.0), upper: Position::new(r_fuel, r_fuel, 1.0) };
    let edges = log_grid(1.0e-3, 2.0e7, 30);
    let grid = log_grid(1.0e-4, 3.0e7, 400);
    let constant = |_: &str, _: i32| Some(NuclideReaction { low: ReactionTable { grid: grid.clone(), xs: vec![4.0; grid.len()], threshold: 0 }, high: None });
    let out = get_microxs_and_flux(&geom, &materials, &nuclides, src, &ks, &[0, 1], &["U235"], &["(n,gamma)"], &edges, constant).unwrap();
    for d in &out {
        assert!(d.flux > 0.0 && (d.flux - d.group_flux.iter().sum::<f64>()).abs() == 0.0);
        let s = d.micro.get("U235", "(n,gamma)", 0).unwrap();
        assert!((s - 4.0).abs() < 1e-12, "{s}");
    }
    let one_over_v = |_: &str, _: i32| Some(NuclideReaction { low: ReactionTable { grid: grid.clone(), xs: grid.iter().map(|e| 1.0 / e.sqrt()).collect(), threshold: 0 }, high: None });
    let out = get_microxs_and_flux(&geom, &materials, &nuclides, src, &ks, &[0, 1], &["U235"], &["(n,gamma)"], &edges, one_over_v).unwrap();
    let (f, m) = (out[0].micro.get("U235", "(n,gamma)", 0).unwrap(), out[1].micro.get("U235", "(n,gamma)", 0).unwrap());
    let thermal = |d: &outram_mc_libs::depletion::microxs_transport::DomainMicroXs| {
        d.group_flux.iter().zip(edges.windows(2)).filter(|(_, e)| e[1] <= 1.0).map(|(p, _)| *p).sum::<f64>() / d.flux
    };
    println!("fuel: flux {:.4e}, thermal {:.3}, 1/sqrt(E) {:.4}", out[0].flux, thermal(&out[0]), f);
    println!("moderator: flux {:.4e}, thermal {:.3}, 1/sqrt(E) {:.4}", out[1].flux, thermal(&out[1]), m);
    assert!(thermal(&out[1]) > thermal(&out[0]));
    assert!(m > f);
}
