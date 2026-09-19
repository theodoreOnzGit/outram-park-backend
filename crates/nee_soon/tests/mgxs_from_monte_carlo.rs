// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # MGXS condensed from a real Monte Carlo run, checked against neutron balance
//!
//! [`nee_soon::mgxs`]'s unit tests prove the index arithmetic on synthetic
//! tallies. This checks the whole path on an actual transport run: two
//! k-eigenvalue passes over a reflective HEU/H-1 pin cell at a fixed seed, then
//! condensation into per-material multigroup constants.
//!
//! ## The identity being tested
//!
//! A neutron that interacts is either absorbed or scattered, so for every zone
//! and group
//!
//! ```text
//!   Sigma_t,g  =  Sigma_a,g  +  sum over g' of Sigma_s,g->g'
//! ```
//!
//! This is a genuine check rather than a restatement, because the two sides
//! come from **different estimators**: the left and the absorption term are
//! track-length tallies (`w * d * Sigma_x`), while the scattering matrix is the
//! analog event estimator added for this work. They are independent estimates
//! of the same physical quantity and agree only if both are right. A
//! mis-normalised matrix, a wrong flux denominator, or a transposed axis all
//! break it.
//!
//! Agreement is therefore statistical, not exact, and the tolerance is set on
//! the relative residual of the balance in groups carrying enough flux to have
//! converged. Groups with negligible flux are excluded explicitly rather than
//! quietly passing on noise.
//!
//! ## Results (measured 2026-09-19)
//!
//! Printed by the test at run time. This is a **harness and wiring check, not
//! physics V&V**: it shows the condensation is self-consistent against neutron
//! balance, not that the group constants reproduce any reference eigenvalue.

use nee_soon::mgxs::{condense, matrix_tally, scalar_tally, GroupStructure};
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

const SEED: u64 = 20_260_919;
const N_GROUPS: usize = 4;

fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
        Nuclide::from_core("H1").expect("H1 in CORE WMP library"),
    ]
}

fn materials() -> Vec<Material> {
    vec![
        Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.9184e-4,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 4.4994e-2,
                },
                NuclideComponent {
                    nuclide_idx: 2,
                    atom_density: 2.4984e-3,
                },
            ],
        },
        Material {
            id: 2,
            name: "H moderator".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 3,
                atom_density: 6.6e-2,
            }],
        },
    ]
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

fn settings() -> KeffSettings {
    KeffSettings {
        n_particles: 400,
        n_inactive: 10,
        n_active: 40,
        seed: SEED,
        ..KeffSettings::default()
    }
}

#[test]
fn mgxs_from_a_monte_carlo_run_satisfies_neutron_balance() {
    let (r_fuel, half) = (0.4, 0.63);
    let geom = pincell(r_fuel, half);
    let mats = materials();
    let nucs = nuclides();
    let groups =
        GroupStructure::log_spaced(1.0e-3, 2.0e7, N_GROUPS).expect("valid group structure");
    let names: Vec<String> = mats.iter().map(|m| m.name.clone()).collect();
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -1.0),
        upper: Position::new(r_fuel, r_fuel, 1.0),
    };

    // Two passes over the same model at the same seed.
    let mut scalar = scalar_tally(1, &groups, vec![0, 1]);
    run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut scalar));

    let mut matrix = matrix_tally(2, &groups, vec![0, 1]);
    run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut matrix));

    let lib = condense(
        &groups,
        &names,
        &scalar,
        &matrix,
        settings().n_active as u64,
    )
    .expect("condensation");

    assert_eq!(lib.zones.len(), 2);
    println!("MGXS from Monte Carlo, {N_GROUPS} groups, seed {SEED}, 400p x 40 active");
    println!("(group 0 = lowest energy)\n");

    // Total flux, to decide which groups carry enough statistics to judge.
    let total_flux: f64 = lib.zones.iter().flat_map(|z| z.flux.iter()).sum();
    assert!(total_flux > 0.0, "no flux tallied at all");

    let mut checked = 0usize;
    for zone in &lib.zones {
        println!("zone '{}':", zone.name);
        for g in 0..N_GROUPS {
            let phi = zone.flux[g];
            let st = zone.total[g];
            let sa = zone.absorption[g];
            let ss = zone.scatter_out(g).unwrap();
            let frac = phi / total_flux;
            println!(
                "  g{g}  phi={phi:.4e} ({:.3} of total)  Sigma_t={st:.4e}  Sigma_a={sa:.4e}  Sigma_s,out={ss:.4e}",
                frac
            );

            for (label, v) in [
                ("flux", phi),
                ("total", st),
                ("absorption", sa),
                ("scatter", ss),
            ] {
                assert!(
                    v.is_finite() && v >= 0.0,
                    "zone '{}' g{g} {label} = {v}",
                    zone.name
                );
            }

            // Judge the balance only where there is enough flux for the two
            // estimators to have converged. Groups below the cut are reported
            // above but not asserted on -- asserting on noise would make the
            // test pass for the wrong reason.
            if frac < 0.02 || st <= 0.0 {
                continue;
            }
            checked += 1;
            let residual = (st - (sa + ss)).abs() / st;
            println!("        balance residual = {residual:.4e}");
            assert!(
                residual < 0.10,
                "neutron balance broken in zone '{}' group {g}: \
                 Sigma_t={st:.6e} vs Sigma_a+Sigma_s,out={:.6e} (relative {residual:.3e}). \
                 The track-length and analog estimators disagree, so the matrix normalisation, \
                 the flux denominator, or the matrix orientation is wrong.",
                zone.name,
                sa + ss
            );
        }
    }

    assert!(
        checked >= 2,
        "only {checked} group(s) carried enough flux to judge; the test proved almost nothing"
    );
    println!("\nneutron balance checked in {checked} well-populated group(s)");
}
