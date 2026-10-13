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
//! **Fission score and eigenvalue check, 2026-10-12 (gh:#848).** At the
//! test's 400 × \[10 + 40\]: fuel `nu-bar` 2.4400 / 2.4400 / 2.4322 / 2.6097
//! (group 0 lowest), `Sigma_f <= Sigma_a` in every group, moderator
//! `Sigma_f = 0`, and the tallied production/absorption `k_inf = 1.859698`
//! against the driver's `k = 1.871697 ± 0.008067` (−1200 pcm, 1.5σ). Rerun
//! once at 4000 × \[10 + 100\] to tell noise from bias: `k_inf = 1.864458`
//! against `1.865059 ± 0.001954` (**−60 pcm, 0.3σ**), and the balance
//! residuals fell from 1.4e-3 to 9.2e-3 down to 1.9e-4 to 1.5e-3, i.e. with
//! the statistics. The −1200 pcm was noise.
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
    let mc = run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut scalar));

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

    // ── The fission spectrum must be a spectrum ──────────────────────────────
    //
    // chi is MEASURED from the energies fission neutrons were actually born
    // with, not assumed from a Watt form, so it is worth checking it behaves
    // like a probability distribution and puts its neutrons where fission
    // physics puts them.
    let fuel = &lib.zones[0];
    let chi_sum: f64 = fuel.chi.iter().sum();
    println!(
        "\nfission spectrum chi (zone '{}'): {:?}",
        fuel.name, fuel.chi
    );
    assert!(
        (chi_sum - 1.0).abs() < 1e-12,
        "chi must normalise to 1, got {chi_sum}"
    );
    for (g, c) in fuel.chi.iter().enumerate() {
        assert!(
            c.is_finite() && *c >= 0.0,
            "chi[{g}] = {c} is not a valid probability"
        );
    }
    // Group 3 is the highest-energy group (0.1 MeV .. 20 MeV on this grid) and
    // a fission spectrum peaks around 1-2 MeV, so the great majority of births
    // belong there. A chi that came out flat, or peaked thermal, would mean the
    // birth energy was not being read.
    assert!(
        fuel.chi[N_GROUPS - 1] > 0.8,
        "fission neutrons are born fast: expected chi in the top group > 0.8, got {:.4} (full chi {:?})",
        fuel.chi[N_GROUPS - 1],
        fuel.chi
    );
    // The moderator never fissions, so its chi is all zeros -- and that must be
    // visible rather than silently normalised into a uniform distribution.
    let mod_chi: f64 = lib.zones[1].chi.iter().sum();
    assert_eq!(
        mod_chi, 0.0,
        "a non-fissioning zone must report an all-zero chi, not a fabricated one"
    );
    println!(
        "chi sums to {chi_sum:.12}; top-group fraction {:.4}",
        fuel.chi[N_GROUPS - 1]
    );
    println!("non-fissioning moderator chi sums to {mod_chi} (correctly empty)");

    // ── The fission score (added 2026-10-12, gh:#848) ────────────────────────
    //
    // Sigma_f <= Sigma_a pointwise, and both are scored on the same track, so
    // the condensed ratio must obey it exactly. nu-bar = nu Sigma_f / Sigma_f
    // must sit in uranium's physical range (about 2.4 thermal to 2.7 at a few
    // MeV on this 4-group grid). A Fission score wired to the wrong rate
    // fails one or the other.
    for g in 0..N_GROUPS {
        assert!(
            fuel.fission[g] <= fuel.absorption[g] * (1.0 + 1e-12),
            "group {g}: Sigma_f {:.6e} > Sigma_a {:.6e}",
            fuel.fission[g],
            fuel.absorption[g]
        );
        if fuel.fission[g] > 0.0 {
            let nu = fuel.nu_fission[g] / fuel.fission[g];
            println!("fuel g{g}: Sigma_f = {:.4e}, nu-bar = {nu:.4}", fuel.fission[g]);
            assert!(
                (2.3..=3.2).contains(&nu),
                "group {g}: nu-bar {nu} is outside uranium's range"
            );
        }
    }
    assert!(
        lib.zones[1].fission.iter().all(|f| *f == 0.0),
        "the hydrogen moderator cannot fission"
    );

    // ── The tallies reproduce the eigenvalue ─────────────────────────────────
    //
    // The cell is reflective on every side, so nothing leaks and the
    // constants' own production / absorption is a second estimate of k, made
    // from the track-length tallies rather than the driver's collision
    // estimator. They share histories, so agreement within 3 sigma of k is a
    // loose bound; a miss beyond it means a tally scores the wrong rate.
    let k_inf = lib.k_inf();
    println!(
        "tallied k_inf = {k_inf:.6}, driver k = {:.6} +/- {:.6} ({:+.0} pcm)",
        mc.k_mean,
        mc.k_std,
        (k_inf - mc.k_mean) * 1e5
    );
    assert!(
        (k_inf - mc.k_mean).abs() <= 3.0 * mc.k_std,
        "production/absorption from the tallies ({k_inf:.6}) disagrees with the driver's k ({:.6} +/- {:.6})",
        mc.k_mean,
        mc.k_std
    );
}

/// **Round trip through the multigroup kernel** (gh:#848): the pin cell's
/// own condensed constants, converted by `MgxsLibrary::to_physics_mg`, run
/// through `physics_mg::run_keff_mg` on the same CSG geometry.
///
/// # Methodology
///
/// The two CE passes of the test above (400 × \[10 + 40\], seed 20260919,
/// 4 log-spaced groups, one zone per material) are condensed and converted
/// with no rebalancing; the converter's own consistency check must accept
/// them (standard errors are carried since 2026-10-13). The MG eigenvalue is
/// run on the same geometry at 2000 × \[20 + 50\] and compared with the CE
/// driver's `k`.
///
/// **This is a wiring check, not the gate (#850).** Four groups and one
/// zone per material are too coarse for agreement to mean much, so the
/// bound is loose: `|k_MG - k_CE| < 5000 pcm`, fixed before the first run
/// with a prediction of `|dk| < 3000 pcm`, sign not predicted. What it
/// catches is a wiring defect: unreversed groups put fission births in the
/// thermal group and a transposed matrix up-scatters the moderator, and
/// either moves `k` by far more than 5000 pcm. The moderator is
/// non-fissile with an all-zero `chi`, so this also transports through the
/// gh:#848 non-fissile case of `physics_mg::Mgxs::new`.
///
/// # Results (2026-10-13)
///
/// The converter accepted the raw library (no group outside 4 sigma). CE
/// `k = 1.87170 ± 0.00807`, MG `k = 1.86376 ± 0.00310`: **−794 ± 864 pcm
/// (0.9σ)**, inside the predicted 3000 pcm. Both arms are single seeds at a
/// quick budget, so this resolves nothing finer than about ±2600 pcm at 3σ;
/// it shows the chain is wired, not how well 4-group constants do.
#[test]
fn condensed_constants_run_through_the_multigroup_kernel() {
    use outram_mc_libs::physics::physics_mg::{run_keff_mg, MgSettings};
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
    let mut scalar = scalar_tally(1, &groups, vec![0, 1]);
    let ce = run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut scalar));
    let mut matrix = matrix_tally(2, &groups, vec![0, 1]);
    run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut matrix));
    let lib = condense(&groups, &names, &scalar, &matrix, settings().n_active as u64)
        .expect("condensation");
    assert!(lib.zones.iter().all(|z| z.sigma.is_some()), "condense carries sigma");

    let mg_lib = lib.to_physics_mg().expect("the raw condensed library is consistent");
    assert!(mg_lib.material(0).is_fissile());
    assert!(!mg_lib.material(1).is_fissile());
    let mg_settings = MgSettings {
        n_particles: 2000,
        n_inactive: 20,
        n_active: 50,
        seed: SEED,
    };
    let mg = run_keff_mg(&geom, &mg_lib, src, &mg_settings);
    let dk_pcm = (mg.k_mean - ce.k_mean) * 1e5;
    let sigma_pcm = (mg.k_std.powi(2) + ce.k_std.powi(2)).sqrt() * 1e5;
    println!(
        "CE k = {:.5} +/- {:.5}, MG k ({N_GROUPS} groups) = {:.5} +/- {:.5}: {dk_pcm:+.0} +/- {sigma_pcm:.0} pcm",
        ce.k_mean, ce.k_std, mg.k_mean, mg.k_std
    );
    assert!(mg.k_mean.is_finite() && mg.k_mean > 0.0);
    assert!(
        dk_pcm.abs() < 5000.0,
        "MG k {:.5} is {dk_pcm:+.0} pcm from CE k {:.5}: a wiring defect (group order, matrix layout)",
        mg.k_mean,
        ce.k_mean
    );
}
