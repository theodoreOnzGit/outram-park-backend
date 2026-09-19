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

//! # Group-to-group scattering matrix: does the estimator measure what it claims?
//!
//! The scattering matrix `Sigma_s,g->g'` is the one MGXS quantity that could not
//! be tallied before an outgoing-energy filter existed, and it is what the
//! deterministic solvers consume (GeN-Foam's `ZoneNuclearData` stores
//! `scattering[moment][g_out][g_in]`). This file checks the estimator against
//! properties that can **fail**, rather than against itself.
//!
//! ## Methodology
//!
//! A reflective HEU/H-1 pin cell (the same model as the `flux_spectrum`
//! notebook test) is run twice at a **fixed seed**, so the two runs transport
//! byte-identical histories and any difference is the tally wiring, not noise:
//!
//! 1. **Matrix run** — filters `[Energy(8 groups), EnergyOut(8 groups)]`,
//!    score `ScatterN`. 64 bins, one per `(g_in, g_out)` pair.
//! 2. **Total run** — filter `[EnergyOut(1 bin spanning everything)]`, score
//!    `ScatterN`. One bin: every scatter event in the problem.
//!
//! Both use the same analog estimator (`score_scatter_matrix`), which fires only
//! for a tally carrying an outgoing-energy filter, so the comparison is between
//! two binnings of one event stream rather than between two estimators.
//!
//! ## What is asserted
//!
//! - **Conservation.** The 64 matrix elements must sum to the single total bin
//!   **exactly**. A misbinned, dropped or double-counted event breaks this.
//! - **Up-scatter is representable.** Hydrogen at 293.6 K produces thermal
//!   up-scatter through the free-gas kernel, so the upper triangle must not be
//!   identically zero. A matrix clamped to the diagonal, or one that silently
//!   read the incoming energy for both axes, fails here.
//! - **Down-scatter dominates.** A fission-born neutron slowing down in hydrogen
//!   must lose more energy than it gains overall.
//!
//! ## Results (measured 2026-09-19, seed fixed, 300 particles x 25 active)
//!
//! Recorded by the test itself at run time; see the printed summary. The
//! conservation identity holds exactly, which is the load-bearing check.
//!
//! **This is a harness/wiring check, not physics V&V.** It shows the estimator
//! bins and conserves events correctly. It does **not** validate the scattering
//! physics, and no cross section is produced here — converting these rates to
//! `Sigma_s,g->g'` needs division by the group flux, which is the MGXS layer's
//! job.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::{EnergyFilter, EnergyOutFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const N_GROUPS: usize = 8;
/// Fixed seed: both runs must transport identical histories.
const SEED: u64 = 20_260_919;

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

/// `n+1` ascending log-spaced edges in eV.
fn log_edges(e_lo: f64, e_hi: f64, n: usize) -> Vec<f64> {
    let (l0, l1) = (e_lo.ln(), e_hi.ln());
    (0..=n)
        .map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp())
        .collect()
}

fn settings() -> KeffSettings {
    KeffSettings {
        n_particles: 300,
        n_inactive: 10,
        n_active: 25,
        seed: SEED,
        ..KeffSettings::default()
    }
}

fn source(r_fuel: f64) -> SourceBox {
    SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -1.0),
        upper: Position::new(r_fuel, r_fuel, 1.0),
    }
}

#[test]
fn scatter_matrix_conserves_events_and_admits_upscatter() {
    let (r_fuel, half) = (0.4, 0.63);
    let geom = pincell(r_fuel, half);
    let mats = materials();
    let nucs = nuclides();
    let edges = log_edges(1.0e-3, 2.0e7, N_GROUPS);

    // ── Run 1: the (g_in, g_out) matrix ──────────────────────────────────────
    let mut matrix = Tally {
        id: 1,
        name: "scatter matrix".into(),
        filters: vec![
            FilterKind::Energy(EnergyFilter {
                bins: edges.clone(),
            }),
            FilterKind::EnergyOut(EnergyOutFilter {
                bins: edges.clone(),
            }),
        ],
        scores: vec![ScoreType::ScatterN],
        bins: vec![TallyBin::default(); N_GROUPS * N_GROUPS],
    };
    run_keff_csg(
        &geom,
        &mats,
        &nucs,
        source(r_fuel),
        &settings(),
        Some(&mut matrix),
    );

    // ── Run 2: one bin spanning everything, same seed ────────────────────────
    //
    // BOTH filters are present and span the same outer range as the matrix.
    // That is deliberate and was learned the hard way: with only an outgoing
    // filter this tally also counts scatters whose INCOMING energy falls
    // outside the group structure, which the matrix legitimately cannot bin
    // because it requires both filters to pass. Comparing the two then fails by
    // ~1.2e-4 (measured 2026-09-19) through no fault of the estimator. Keeping
    // the accepted event sets identical is what makes the sum an identity.
    let mut total = Tally {
        id: 2,
        name: "scatter total".into(),
        filters: vec![
            FilterKind::Energy(EnergyFilter {
                bins: vec![edges[0], edges[N_GROUPS]],
            }),
            FilterKind::EnergyOut(EnergyOutFilter {
                bins: vec![edges[0], edges[N_GROUPS]],
            }),
        ],
        scores: vec![ScoreType::ScatterN],
        bins: vec![TallyBin::default(); 1],
    };
    run_keff_csg(
        &geom,
        &mats,
        &nucs,
        source(r_fuel),
        &settings(),
        Some(&mut total),
    );

    let n_active = settings().n_active as u64;
    let m: Vec<f64> = matrix.bins.iter().map(|b| b.mean(n_active)).collect();
    let t = total.bins[0].mean(n_active);

    // Every bin must be finite and non-negative before anything else is believed.
    for (i, v) in m.iter().enumerate() {
        assert!(
            v.is_finite() && *v >= 0.0,
            "matrix element (g_in={}, g_out={}) is {v}, not finite/non-negative",
            i / N_GROUPS,
            i % N_GROUPS
        );
    }
    assert!(
        t.is_finite() && t > 0.0,
        "total scatter rate {t} is not positive-finite"
    );

    // ── Conservation: the matrix must sum to the total, exactly ──────────────
    // Same seed, same histories, same estimator -- only the binning differs, so
    // this is an identity and not a statistical comparison. Floating-point
    // summation order differs between 64 bins and 1, hence a relative rather
    // than bit-exact tolerance.
    let sum: f64 = m.iter().sum();
    let rel = (sum - t).abs() / t;
    assert!(
        rel < 1e-9,
        "scatter matrix does not conserve events: sum of 64 elements {sum:.9e} vs single-bin total {t:.9e} (rel {rel:.3e}). \
         An element is being misbinned, dropped, or double-counted."
    );

    // ── Up-scatter must be representable ─────────────────────────────────────
    // Group index ascends with energy here (edges are ascending), so g_out >
    // g_in is a gain in energy. Free-gas hydrogen at 293.6 K genuinely produces
    // it; a matrix that read the incoming energy on both axes would be purely
    // diagonal and fail this.
    let up: f64 = (0..N_GROUPS)
        .flat_map(|gi| ((gi + 1)..N_GROUPS).map(move |go| (gi, go)))
        .map(|(gi, go)| m[gi * N_GROUPS + go])
        .sum();
    let down: f64 = (0..N_GROUPS)
        .flat_map(|gi| (0..gi).map(move |go| (gi, go)))
        .map(|(gi, go)| m[gi * N_GROUPS + go])
        .sum();
    let diag: f64 = (0..N_GROUPS).map(|g| m[g * N_GROUPS + g]).sum();

    assert!(
        up > 0.0,
        "no up-scatter recorded at all -- the matrix is not reading the outgoing energy \
         (up={up:.6e}, down={down:.6e}, diag={diag:.6e})"
    );
    assert!(
        down > up,
        "down-scatter {down:.6e} must dominate up-scatter {up:.6e} for fission neutrons \
         slowing down in hydrogen"
    );

    println!("scatter matrix ({N_GROUPS}x{N_GROUPS}), seed {SEED}, 300p x 25 active:");
    println!("  total scatter rate   = {t:.6e} (single bin)");
    println!("  sum of matrix        = {sum:.6e}  (rel diff {rel:.3e})");
    println!("  down / diag / up     = {down:.6e} / {diag:.6e} / {up:.6e}");
    println!("  up fraction          = {:.4}", up / t);
}
