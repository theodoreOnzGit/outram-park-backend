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
///
/// The first five mirror `DhUniverse::pebble`'s TRISO layer order so
/// `pebble_beds::htr10::fuel_pebble_materials` can be used directly.
pub mod mat {
    /// TRISO UO2 kernel.
    pub const KERNEL: usize = 0;
    /// Buffer PyC.
    pub const BUFFER: usize = 1;
    /// Inner PyC.
    pub const IPYC: usize = 2;
    /// SiC.
    pub const SIC: usize = 3;
    /// Outer PyC.
    pub const OPYC: usize = 4;
    /// Matrix graphite inside the fuel zone, and the pebble shell.
    pub const GRAPHITE: usize = 5;
    /// Helium between pebbles.
    pub const HELIUM: usize = 6;
    /// Reflector graphite (TECDOC Table 4-3).
    pub const REFLECTOR: usize = 7;
    /// Homogenised fuel zone, used only by [`super::assemble`].
    pub const FUEL: usize = KERNEL;
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

/// **Assemble the core with an EXPLICIT TRISO lattice in each fuelled pebble** —
/// the double-heterogeneous model the benchmark actually specifies.
///
/// Four coordinate levels: root → bed hex lattice → pebble universe → TRISO
/// rect lattice → TRISO particle universe. Depth-3 descent was gated in
/// `outram-mc-libs` `tests/nested_lattice_depth3.rs`; this is depth 4.
///
/// # The TRISO lattice
///
/// A cubic array clipped to the fuel zone, keeping only whole particles, per
/// `cubic_array_in_ball`. Expressed as a `RectLattice` whose tiles hold either a
/// particle universe or matrix graphite, with `outer` = matrix so anything
/// beyond the array's extent is graphite.
///
/// The realised particle count is **8340**, not the paper's stated 8335 — see
/// `cubic_array_in_ball`'s docs for why 8335 is unattainable (the count moves in
/// symmetry shells). That is +0.060 % in fuel volume.
pub fn assemble_explicit_triso(
    n_rings: usize,
    n_axial: usize,
    majorant_index: usize,
) -> AssembledCore {
    use outram_mc_libs::geometry::lattice::RectLattice;
    use outram_mc_libs::pebble_beds::sphere_packing::cubic_pitch_for_count;

    let cell = HexBedCell::from_paper();
    let r_pebble = cell.ball_diameter * 0.5;
    let r_fuel_zone = 2.5;
    // Adjudicated radii (op-867c.12): TECDOC-1382, 90 um buffer.
    let tr = [0.0250_f64, 0.0340, 0.0380, 0.0415, 0.0455];
    let r_part = tr[4];
    let (pitch_triso, _n_particles) = cubic_pitch_for_count(r_part, r_fuel_zone, 8335, [0.5, 0.5, 0.0]);
    // A cubic lattice spanning the fuel zone; tiles outside it fall through to
    // `outer` = matrix graphite, which is exactly the "not occupied is filled
    // with graphite" the paper specifies.
    let n_triso = ((2.0 * r_fuel_zone / pitch_triso).ceil() as usize).max(1);

    let bed_radius = cell.pitch * (n_rings as f64 + 0.5);
    let bed_half_height = 0.5 * cell.height * n_axial as f64;
    let refl_radius = bed_radius + 100.0;
    let refl_half_height = bed_half_height + 100.0;

    // Surfaces 0..4 are the TRISO shells, in PARTICLE-local coordinates.
    let mut surfaces: Vec<SurfaceKind> = tr
        .iter()
        .map(|&r| SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc: BoundaryType::Transmissive }))
        .collect();
    // 5: fuel zone, 6: pebble -- in TILE-local coordinates.
    surfaces.push(SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_fuel_zone, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_pebble, bc: BoundaryType::Transmissive }));
    // 7..9 bed envelope, 10..12 reflector vacuum boundary.
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: bed_radius, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: -bed_half_height, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: bed_half_height, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: refl_radius, bc: BoundaryType::Vacuum }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: -refl_half_height, bc: BoundaryType::Vacuum }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: refl_half_height, bc: BoundaryType::Vacuum }));

    let ins = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside };
    let out = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside };
    let shell = |inner: usize, outer: usize, m: usize, id: i32| {
        Cell::material(id, vec![out(inner), ins(outer), RegionToken::Intersection], m, 293.6)
    };

    // Universe 3 -- one TRISO particle: five shells then matrix.
    let cells = vec![
        // 0: the bed (delta-tracked)
        Cell::fill(1, vec![ins(7), out(8), RegionToken::Intersection, ins(9), RegionToken::Intersection],
                   CellFill::Lattice(0), Position::ZERO).delta_tracked(majorant_index),
        // 1: reflector
        Cell::material(2, vec![ins(10), out(11), RegionToken::Intersection, ins(12), RegionToken::Intersection,
                               ins(7), out(8), RegionToken::Intersection, ins(9), RegionToken::Intersection,
                               RegionToken::Complement, RegionToken::Intersection],
                       mat::REFLECTOR, 293.6),
        // 2: fuelled pebble -- fuel zone holds the TRISO lattice
        Cell::fill(3, vec![ins(5)], CellFill::Lattice(1), Position::ZERO),
        // 3: pebble shell, 4: helium around the fuelled pebble
        shell(5, 6, mat::GRAPHITE, 4),
        Cell::material(5, vec![out(6)], mat::HELIUM, 293.6),
        // 5,6: dummy pebble and its helium
        Cell::material(6, vec![ins(6)], mat::GRAPHITE, 293.6),
        Cell::material(7, vec![out(6)], mat::HELIUM, 293.6),
        // 7..12: the TRISO particle's shells, then matrix beyond it
        Cell::material(8, vec![ins(0)], mat::KERNEL, 293.6),
        shell(0, 1, mat::BUFFER, 9),
        shell(1, 2, mat::IPYC, 10),
        shell(2, 3, mat::SIC, 11),
        shell(3, 4, mat::OPYC, 12),
        Cell::material(13, vec![out(4)], mat::GRAPHITE, 293.6),
        // 13: pure matrix, the TRISO lattice's `outer`
        Cell::material(14, vec![out(4)], mat::GRAPHITE, 293.6),
    ];

    let levels = bed_tile_levels(n_rings, n_axial, 1, 2);
    let tiles: usize = levels.iter().flatten().map(|r| r.len()).sum();
    let bed_lattice = HexLattice::from_rings_3d(
        0, HexOrientation::Y,
        Position::new(0.0, 0.0, -bed_half_height + 0.5 * cell.height),
        cell.pitch, cell.height, &levels, Some(2),
    );
    let triso_lattice = RectLattice {
        id: 1,
        n: [n_triso, n_triso, n_triso],
        lower_left: Position::new(-r_fuel_zone, -r_fuel_zone, -r_fuel_zone),
        pitch: [pitch_triso; 3],
        universes: vec![3; n_triso * n_triso * n_triso],
        outer: Some(4),
    };

    let geometry = Geometry {
        surfaces,
        cells,
        universes: vec![
            Universe { id: 0, cell_indices: vec![0, 1] },          // root
            Universe { id: 1, cell_indices: vec![2, 3, 4] },        // fuelled pebble
            Universe { id: 2, cell_indices: vec![5, 6] },           // dummy pebble
            Universe { id: 3, cell_indices: vec![7, 8, 9, 10, 11, 12] }, // TRISO particle
            Universe { id: 4, cell_indices: vec![13] },             // matrix (lattice outer)
        ],
        lattices: vec![Lattice::Hex(bed_lattice), Lattice::Rect(triso_lattice)],
        root_universe: 0,
    };
    let (c, u) = (geometry.cells.len(), geometry.universes.len());
    AssembledCore { geometry, tiles, cells: c, universes: u }
}
