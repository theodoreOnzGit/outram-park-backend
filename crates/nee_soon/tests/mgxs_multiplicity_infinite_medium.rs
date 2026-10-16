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

//! # Scattering multiplicity end to end, in an infinite medium (gh:#682)
//!
//! In an **infinite homogeneous medium** flux-weighted group constants are
//! exact: the group balance equations hold with the tallied rates, so a
//! multigroup eigenvalue solve on the condensed constants reproduces the
//! continuous-energy `k_inf` in expectation, whatever the group structure.
//! That holds only if every neutron the constants describe is transported,
//! including the second neutron of an (n,2n). So this is a sharp test of the
//! whole multiplicity chain: the CE scatter-only score, `condense`,
//! `to_physics_mg`'s per-transfer multiplicity, and `run_keff_mg`'s weight.
//!
//! ## Methodology
//!
//! - **Medium:** HEU (`U234`/`U235`/`U238` from the CORE library, Godiva's
//!   densities scaled by 1/10) mixed with beryllium-9 at 0.1 atoms/b-cm, Be-9
//!   from ENDF/B-VIII.0 (`reference-data/endf/n-004_Be_009-ENDF8.0.endf`,
//!   RECONR+BROADR at 293.6 K, tolerance 0.001). Be-9's (n,2n) opens at
//!   1.85 MeV, inside the fission spectrum; the CORE uranium carries no
//!   (n,2n), so the gain comes from beryllium.
//! - **Geometry:** a reflective 10 cm cube, one material.
//! - **CE:** two passes at one seed (`scalar_tally`, `matrix_tally`),
//!   2000 × \[20 + 80\], 4 log-spaced groups, 1e-3 eV to 20 MeV.
//! - **MG:** `to_physics_mg()` (multiplicity on, the default) and
//!   `to_physics_mg_without_multiplicity()` (ablation), each
//!   4000 × \[30 + 100\] on the same cube.
//! - **Prediction, recorded before the first run:** the default arm matches
//!   CE `k` within 3σ combined; the ablation is LOW by the (n,2n) gain, of
//!   order 1000–5000 pcm. After the first run showed the constants' own
//!   `k_inf` missing the gain, a second prediction: the corrected `k_inf`
//!   matches CE within 3σ.
//! - **Gates:** `|k_MG - k_CE| < 3σ` for the default arm; the ablation below
//!   the default arm by more than 3σ combined; `|k_inf - k_CE| < 3σ_CE`.
//!
//! ## Results (2026-10-13, one seed, the budgets above; 81 s)
//!
//! | quantity | value |
//! |---|---|
//! | CE `k` | 1.90314 ± 0.00381 |
//! | MG `k`, multiplicity on (default) | 1.90416 ± 0.00206: **+102 ± 433 pcm** from CE (0.2σ) |
//! | MG `k`, ablated (same set, multiplicity removed) | 1.71539 ± 0.00170 |
//! | worth of the multiplicity (paired arms) | **+18 878 ± 267 pcm** |
//! | `MgxsLibrary::k_inf()` with the (n,xn) gain | 1.90394 (+80 pcm from CE, 0.2σ) |
//! | `k_inf` without it (the pre-2026-10-13 formula) | 1.71638 |
//! | fastest-group (53 keV–20 MeV) row multiplicity | 1.007571 |
//!
//! The prediction held: the default arm reproduces CE, and the ablation is
//! low. The magnitude was underestimated: a 0.76 % gain per fast scatter
//! compounds over the ~18 collisions beryllium needs to slow a neutron
//! through the 53 keV–20 MeV group. The ablated arm lands on the gain-free
//! algebraic `k_inf`, as it should: without multiplicity the MG solve
//! reproduces exactly the incomplete balance.
//!
//! In the thermal group (about 1e-6 of the flux here) the scatter row and
//! `Sigma_t - Sigma_a` differ by tens of percent in either direction from
//! run to run; the converter's statistical check passes it, which is
//! correct at that population.
//!
//! **Not covered:** a heterogeneous geometry, where flux-weighted constants
//! are no longer exact; that is the gate's question (#850).

use nee_soon::mgxs::{condense, matrix_tally, scalar_tally, GroupStructure};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::physics_mg::{run_keff_mg, MgSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

const SEED: u64 = 20_261_013;
const HALF: f64 = 5.0;

fn cube() -> Geometry {
    let p = |k: usize, v: f64| -> SurfaceKind {
        let bc = BoundaryType::Reflective;
        match k {
            0 => SurfaceKind::XPlane(XPlane { x0: v, bc }),
            1 => SurfaceKind::YPlane(YPlane { y0: v, bc }),
            _ => SurfaceKind::ZPlane(ZPlane { z0: v, bc }),
        }
    };
    let surfaces = vec![
        p(0, -HALF),
        p(0, HALF),
        p(1, -HALF),
        p(1, HALF),
        p(2, -HALF),
        p(2, HALF),
    ];
    let mut region = Vec::new();
    for i in 0..6 {
        region.push(RegionToken::HalfSpace {
            surface_idx: i,
            sense: if i % 2 == 0 {
                HalfSpaceSense::Outside
            } else {
                HalfSpaceSense::Inside
            },
        });
        if i > 0 {
            region.push(RegionToken::Intersection);
        }
    }
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Behind `long-tests` for its reference data (the Be-9 ENDF tape), not its
/// runtime: 81 s on a shared 4-core Xeon @ 2.10 GHz (2026-10-13).
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "needs the reference ENDF data (Be-9), docs/claude-md/long-tests-and-ci.md"
)]
fn multiplicity_closes_the_infinite_medium_eigenvalue() {
    let Some(be9) = reference_endf_or_skip("n-004_Be_009-ENDF8.0.endf", "Be-9 (n,2n)") else {
        return;
    };
    let nucs = vec![
        Nuclide::from_core("U234").expect("U234"),
        Nuclide::from_core("U235").expect("U235"),
        Nuclide::from_core("U238").expect("U238"),
        Nuclide::from_endf_file(&be9, "Be9", 293.6, 1.0e-3).expect("Be-9 processes"),
    ];
    let comp = |i: usize, n: f64| NuclideComponent {
        nuclide_idx: i,
        atom_density: n,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU + Be".into(),
        temperature: 293.6,
        components: vec![
            comp(0, 4.9184e-5),
            comp(1, 4.4994e-3),
            comp(2, 2.4984e-4),
            comp(3, 0.1),
        ],
    }];
    let geom = cube();
    let src = SourceBox {
        lower: Position::new(-HALF, -HALF, -HALF),
        upper: Position::new(HALF, HALF, HALF),
    };
    let ce_settings = KeffSettings {
        n_particles: 2000,
        n_inactive: 20,
        n_active: 80,
        seed: SEED,
        ..KeffSettings::default()
    };
    let groups = GroupStructure::log_spaced(1.0e-3, 2.0e7, 4).expect("groups");
    let mut scalar = scalar_tally(1, &groups, vec![0]);
    let ce = run_keff_csg(&geom, &mats, &nucs, src, &ce_settings, Some(&mut scalar));
    let mut matrix = matrix_tally(2, &groups, vec![0]);
    run_keff_csg(&geom, &mats, &nucs, src, &ce_settings, Some(&mut matrix));
    let lib = condense(
        &groups,
        &["HEU + Be".to_string()],
        &scalar,
        &matrix,
        ce_settings.n_active as u64,
    )
    .expect("condense");

    let z = &lib.zones[0];
    for g in (0..4).rev() {
        let nu: f64 = z.scatter[g].iter().sum();
        let s: f64 = z.scatter_only[g].iter().sum();
        println!(
            "group {g} (ascending): nu-scatter row {nu:.6e}, scatter row {s:.6e}, Sigma_t-Sigma_a {:.6e}, multiplicity {:.6}",
            z.total[g] - z.absorption[g],
            if s > 0.0 { nu / s } else { 1.0 }
        );
    }

    let mg_settings = MgSettings {
        n_particles: 4000,
        n_inactive: 30,
        n_active: 100,
        seed: SEED,
    };
    let on_lib = lib.to_physics_mg().expect("consistent library converts");
    assert!(on_lib.material(0).multiplicity.is_some(), "on by default");
    // The ablation is the SAME converted set with the multiplicity removed,
    // so the two arms differ in that and nothing else.
    let off_lib = outram_mc_libs::physics::physics_mg::MgxsLibrary::new(
        on_lib
            .materials
            .iter()
            .cloned()
            .map(|m| m.without_scattering_multiplicity())
            .collect(),
    );
    let on = run_keff_mg(&geom, &on_lib, src, &mg_settings);
    let off = run_keff_mg(&geom, &off_lib, src, &mg_settings);
    let pcm = |a: f64| a * 1e5;
    let sig = |a: f64, b: f64| (a * a + b * b).sqrt();
    println!(
        "CE k = {:.5} ± {:.5}; MG (multiplicity) k = {:.5} ± {:.5}: {:+.0} ± {:.0} pcm",
        ce.k_mean,
        ce.k_std,
        on.k_mean,
        on.k_std,
        pcm(on.k_mean - ce.k_mean),
        pcm(sig(on.k_std, ce.k_std))
    );
    println!(
        "MG ablated k = {:.5} ± {:.5}: worth of the multiplicity {:+.0} ± {:.0} pcm",
        off.k_mean,
        off.k_std,
        pcm(on.k_mean - off.k_mean),
        pcm(sig(on.k_std, off.k_std))
    );
    println!("tallied k_inf (with the (n,xn) gain) = {:.5}", lib.k_inf());
    assert!(
        (on.k_mean - ce.k_mean).abs() < 3.0 * sig(on.k_std, ce.k_std),
        "the multiplicity arm must reproduce the CE k_inf"
    );
    assert!(
        (lib.k_inf() - ce.k_mean).abs() < 3.0 * ce.k_std,
        "the constants' own k_inf must count the (n,xn) gain"
    );
    assert!(
        on.k_mean - off.k_mean > 3.0 * sig(on.k_std, off.k_std),
        "the ablation must lose the (n,2n) gain"
    );
}
