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

//! # The coupling API, used the way a caller would use it
//!
//! [`nee_soon::coupling::McToGenFoam`] is the one type a user is meant to learn
//! for Monte Carlo to deterministic coupling. This exercises it through its
//! public surface only — construct, configure, generate, solve — with no
//! reaching into `mgxs` or `genfoam_xs` internals, which is the test of whether
//! the facade is actually sufficient.
//!
//! The workspace's API rule is *"if it is too complex for Haiku, it is a bad
//! API"*. This file is the Rust half of that: if writing it required knowing
//! anything the type's own documentation does not say, the API is wrong.

use nee_soon::coupling::McToGenFoam;
use nee_soon::mgxs::GroupStructure;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::transport_csg::SourceBox;

const HALF: f64 = 10.0;

fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>, SourceBox) {
    let refl = BoundaryType::Reflective;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane {
            x0: -HALF,
            bc: refl,
        }),
        SurfaceKind::XPlane(XPlane { x0: HALF, bc: refl }),
        SurfaceKind::YPlane(YPlane {
            y0: -HALF,
            bc: refl,
        }),
        SurfaceKind::YPlane(YPlane { y0: HALF, bc: refl }),
        SurfaceKind::ZPlane(ZPlane {
            z0: -HALF,
            bc: refl,
        }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF, bc: refl }),
    ];
    let cell = Cell::material(
        1,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        0,
        293.6,
    );
    let geometry = Geometry {
        surfaces,
        cells: vec![cell],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let materials = vec![Material {
        id: 1,
        name: "HEU".into(),
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
    }];
    let nuclides = vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ];
    let source = SourceBox {
        lower: Position::new(-HALF, -HALF, -HALF),
        upper: Position::new(HALF, HALF, HALF),
    };
    (geometry, materials, nuclides, source)
}

/// The documented three-call path works and the two codes agree.
#[test]
fn the_documented_three_call_path_works() {
    let (geometry, materials, nuclides, source) = model();

    let coupling = McToGenFoam::new(geometry, materials, nuclides, source)
        .with_groups(GroupStructure::two_group(5.0e4).expect("valid split"))
        // The statistics matter here: at 600 particles / 40 batches the
        // deterministic-vs-Monte-Carlo gap measured 1487 pcm against a 248 pcm
        // one-sigma, which is a 6-sigma bias and not noise. Raised to the
        // configuration the dedicated mc_to_genfoam_kinf test verifies at,
        // rather than widening the tolerance to admit it.
        .with_particles(800)
        .with_batches(20, 60);

    let run = coupling.generate_mgxs().expect("MGXS generation");
    let k_det = coupling
        .solve_infinite_medium(&run.library)
        .expect("deterministic solve");

    println!(
        "Monte Carlo k = {:.6} +/- {:.6}   GeN-Foam k_inf = {k_det:.6}",
        run.k_eff, run.k_std
    );

    assert!(run.k_eff > 0.0 && run.k_eff.is_finite());
    assert!(
        run.k_std > 0.0,
        "an eigenvalue must come with its uncertainty"
    );
    assert_eq!(run.library.zones.len(), 1);
    assert_eq!(run.library.groups.n_groups(), 2);

    let diff_pcm = (k_det - run.k_eff) / run.k_eff * 1.0e5;
    println!("difference = {diff_pcm:+.1} pcm");
    assert!(
        diff_pcm.abs() < 1000.0,
        "leakage-free deterministic k {k_det:.6} should reproduce Monte Carlo {:.6}, \
         got {diff_pcm:+.1} pcm",
        run.k_eff
    );
}

/// The defaults alone produce a usable result — no setter is required.
///
/// This matters for discoverability: a caller who finds the type and calls the
/// two verbs must get somewhere, rather than hitting a panic about an unset
/// field. The default two-group split at the cadmium cut-off is wrong for a
/// bare fast assembly (nothing is thermal), so this asserts the FAILURE is the
/// clear, actionable one rather than a crash or a silent wrong number.
#[test]
fn defaults_are_usable_and_failures_are_actionable() {
    let (geometry, materials, nuclides, source) = model();
    let coupling = McToGenFoam::new(geometry, materials, nuclides, source).with_particles(400);

    assert_eq!(coupling.groups().n_groups(), 2, "default is two groups");

    let run = coupling
        .generate_mgxs()
        .expect("MGXS generation with defaults");
    match coupling.solve_infinite_medium(&run.library) {
        Ok(k) => {
            // If this medium did populate both default groups, the answer must
            // still be sane.
            assert!(k > 0.0 && k.is_finite(), "k = {k}");
            println!("defaults gave k_inf = {k:.6}");
        }
        Err(e) => {
            // The expected path for a bare fast assembly: nothing is thermal,
            // so the thermal group is empty and the bridge says so in words
            // that name the fix.
            let msg = e.to_string();
            println!("defaults correctly refused: {msg}");
            assert!(
                msg.contains("group") && (msg.contains("flux") || msg.contains("measured")),
                "the error must explain WHICH group and WHY, got: {msg}"
            );
        }
    }
}
