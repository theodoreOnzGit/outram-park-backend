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

//! # The Haiku dogfood result, compiled
//!
//! The workspace's API rule is *"if it is too complex for Haiku, it is a bad
//! API"*, and it is a test, not a slogan: a small model has no budget to read
//! the source, so it can only use what the interface itself makes
//! discoverable.
//!
//! The `main` below is **the program a Haiku agent wrote on 2026-09-19** given
//! the rendered [`nee_soon::coupling`] and [`nee_soon::mgxs`] API reference and
//! nothing else — no repository access, no source, no examples. It is kept here
//! compiled so the claim "this API is usable from the docs alone" is checked by
//! the build rather than asserted.
//!
//! ## Result: one round trip, zero exceptions
//!
//! Everything resolved first time: the four-argument constructor, the chained
//! `with_*` setters returning `Self`, `GroupStructure::two_group` as the way to
//! get a two-group split, `MgxsRun`'s three public fields, and passing
//! `&run.library` (not `&run`) to `solve_infinite_medium`.
//!
//! That last one is the informative pass. The natural wrong guess is
//! `solve_infinite_medium(&run)`, because `run` is what the previous call
//! returned; the field documentation naming `library` as the group constants is
//! what made the right call findable. Compare the ten wrong guesses the
//! workspace `CLAUDE.md` records from an Opus session against the Python wheel
//! **with** full source access.
//!
//! The helper functions it was told to assume exist are supplied here so the
//! file builds; only `main` is the agent's.

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

// ── The agent's program, verbatim in shape ───────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let coupling = McToGenFoam::new(geometry(), materials(), nuclides(), source())
        .with_groups(GroupStructure::two_group(50000.0)?)
        .with_particles(800);

    let run = coupling.generate_mgxs()?;
    println!("Monte Carlo k_eff = {:.6} +/- {:.6}", run.k_eff, run.k_std);

    let k_inf = coupling.solve_infinite_medium(&run.library)?;
    println!("GeN-Foam k_inf    = {k_inf:.6}");

    let diff_pcm = (k_inf - run.k_eff) / run.k_eff * 1.0e5;
    println!("difference        = {diff_pcm:+.1} pcm");

    Ok(())
}

// ── Helpers the agent was told to assume ─────────────────────────────────────

fn geometry() -> Geometry {
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
    Geometry {
        surfaces,
        cells: vec![cell],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn materials() -> Vec<Material> {
    vec![Material {
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
    }]
}

fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ]
}

fn source() -> SourceBox {
    SourceBox {
        lower: Position::new(-HALF, -HALF, -HALF),
        upper: Position::new(HALF, HALF, HALF),
    }
}
