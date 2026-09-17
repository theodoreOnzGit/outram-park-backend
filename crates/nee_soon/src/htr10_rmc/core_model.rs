// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Assembled HTR-10 core geometry** — `bn:op-867c.14`, gh #214.
//!
//! Puts together the pieces gated separately elsewhere: the hex-prism bed
//! ([`super::bed`]), the reflector ([`super::reflector`]), the TRISO array
//! (`outram_mc_libs::pebble_beds::sphere_packing::cubic_array_in_ball`) and
//! hybrid delta/surface tracking (`outram_mc_libs` `TrackingMethod`).
//!
//! # Parameterised by SIZE, deliberately
//!
//! The full core is ~24,000 hex tiles with a nested TRISO lattice in each
//! fuelled one. The largest lattice ever built in this crate before today was
//! **37 tiles**, so full scale is ~650x beyond anything exercised, and the
//! runtime, the memory and whether depth-3 transport holds up there are all
//! unmeasured.
//!
//! Building it only at full size would be one all-or-nothing attempt. Taking
//! `n_rings` and `n_axial` as parameters makes the measurement incremental and
//! lets the cost be extrapolated rather than guessed — which is what
//! `op-867c.14` actually asks for.
//!
//! # What this is NOT
//!
//! Not yet a benchmark model. The pebbles here carry a **homogenised** fuel
//! sphere rather than an explicit TRISO lattice, so the double heterogeneity is
//! absent and `k` from this is not comparable to the paper. That is deliberate:
//! this exists to measure the cost of the BED, which is the part at unprecedented
//! scale. The TRISO nesting multiplies on top and is priced separately.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;

use super::bed::{bed_tile_levels, HexBedCell};

/// Material slots the assembled geometry expects, in order.
pub mod mat {
    /// Fuelled pebble (homogenised fuel zone).
    pub const FUEL: usize = 0;
    /// Pebble graphite shell / dummy pebble.
    pub const GRAPHITE: usize = 1;
    /// Helium between pebbles.
    pub const HELIUM: usize = 2;
    /// Reflector graphite.
    pub const REFLECTOR: usize = 3;
}

/// A built core and the sizes that describe it.
pub struct AssembledCore {
    /// The geometry.
    pub geometry: Geometry,
    /// Hex tiles in the bed lattice.
    pub tiles: usize,
    /// Cells in the geometry.
    pub cells: usize,
    /// Universes in the geometry.
    pub universes: usize,
}

/// **Assemble a delta-tracked pebble bed inside a surface-tracked reflector.**
///
/// The bed cell's pitch and layer height come from [`HexBedCell::from_paper`],
/// so the geometry is the paper's even when the size is scaled down.
///
/// # Parameters
/// - `n_rings`, `n_axial` — bed size. Full HTR-10 is roughly 14 rings x 21 layers.
/// - `majorant_index` — which entry of the caller's majorant table the bed uses.
pub fn assemble(n_rings: usize, n_axial: usize, majorant_index: usize) -> AssembledCore {
    let cell = HexBedCell::from_paper();
    let r_pebble = cell.ball_diameter * 0.5;
    let r_fuel_zone = 2.5;

    // Bed envelope: a cylinder just containing the tiles, then the reflector out
    // to the published 1 m thickness.
    let bed_radius = cell.pitch * (n_rings as f64 + 0.5);
    let bed_half_height = 0.5 * cell.height * n_axial as f64;
    let refl_radius = bed_radius + 100.0;
    let refl_half_height = bed_half_height + 100.0;

    let surfaces = vec![
        // 0,1: pebble and its fuel zone, in TILE-LOCAL coordinates
        SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_pebble, bc: BoundaryType::Transmissive }),
        SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_fuel_zone, bc: BoundaryType::Transmissive }),
        // 2..4: the bed envelope
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: bed_radius, bc: BoundaryType::Transmissive }),
        SurfaceKind::ZPlane(ZPlane { z0: -bed_half_height, bc: BoundaryType::Transmissive }),
        SurfaceKind::ZPlane(ZPlane { z0: bed_half_height, bc: BoundaryType::Transmissive }),
        // 5..7: the reflector outer boundary -- VACUUM, this is a bare core
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: refl_radius, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: -refl_half_height, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: refl_half_height, bc: BoundaryType::Vacuum }),
    ];
    let ins = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside };
    let out = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside };

    // Universe 1 -- fuelled pebble. Universe 2 -- dummy pebble. Both sit in
    // tile-local coordinates, so the spheres are shared.
    let fuel_zone = Cell::material(10, vec![ins(1)], mat::FUEL, 293.6);
    let fuel_shell = Cell::material(11, vec![out(1), ins(0), RegionToken::Intersection], mat::GRAPHITE, 293.6);
    let fuel_helium = Cell::material(12, vec![out(0)], mat::HELIUM, 293.6);
    let dummy_ball = Cell::material(13, vec![ins(0)], mat::GRAPHITE, 293.6);
    let dummy_helium = Cell::material(14, vec![out(0)], mat::HELIUM, 293.6);

    // The bed: a cylinder, DELTA-tracked, filled by the hex lattice.
    let bed_region = vec![
        ins(2), out(3), RegionToken::Intersection, ins(4), RegionToken::Intersection,
    ];
    let bed = Cell::fill(1, bed_region.clone(), CellFill::Lattice(0), Position::ZERO)
        .delta_tracked(majorant_index);

    // The reflector: everything else inside the vacuum boundary, SURFACE-tracked.
    let mut refl_region = vec![
        ins(5), out(6), RegionToken::Intersection, ins(7), RegionToken::Intersection,
    ];
    refl_region.extend(bed_region);
    refl_region.push(RegionToken::Complement);
    refl_region.push(RegionToken::Intersection);
    let reflector = Cell::material(2, refl_region, mat::REFLECTOR, 293.6);

    let levels = bed_tile_levels(n_rings, n_axial, 1, 2);
    let tiles: usize = levels.iter().flatten().map(|r| r.len()).sum();
    let lattice = HexLattice::from_rings_3d(
        0,
        HexOrientation::Y,
        Position::new(0.0, 0.0, -bed_half_height + 0.5 * cell.height),
        cell.pitch,
        cell.height,
        &levels,
        Some(2), // outside the hexagon: a dummy pebble universe
    );

    let geometry = Geometry {
        surfaces,
        cells: vec![bed, reflector, fuel_zone, fuel_shell, fuel_helium, dummy_ball, dummy_helium],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0, 1] },
            Universe { id: 1, cell_indices: vec![2, 3, 4] },
            Universe { id: 2, cell_indices: vec![5, 6] },
        ],
        lattices: vec![Lattice::Hex(lattice)],
        root_universe: 0,
    };
    let (cells, universes) = (geometry.cells.len(), geometry.universes.len());
    AssembledCore { geometry, tiles, cells, universes }
}
