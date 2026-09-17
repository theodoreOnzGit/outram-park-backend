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
/// Pebble-bed filling fraction stated by Li, Yu & Wei (2014) for the HTR-10
/// core — the fraction of bed volume occupied by pebbles. Sets the fuel per
/// unit volume, so the assembled geometry is solved to realise it.
pub const PAPER_FILLING_FRACTION: f64 = 0.61;

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
    // One ball per tile -- see assemble_explicit_triso for why the lattice
    // pitch is the ball diameter and not the paper cell's 6.6106/9.798.
    // ONE ball per tile, in a tile that is HALF the paper's two-ball prism.
    //
    // The ball (6.0 cm across) is taller than the tile (4.899 cm), so the
    // lattice CLIPS it axially -- and that clipping is not cosmetic: it removes
    // two spherical caps of 2.6816 cm^3 each from a 113.0973 cm^3 ball, leaving
    // 107.7342 cm^3. At the paper's own pitch the realised filling fraction is
    // therefore 0.5811, NOT the 0.610 an unclipped ball would give. (An earlier
    // comment here claimed the halved height "reproduces the packing exactly";
    // it does not, and that claim was wrong by 4.74 %.)
    //
    // Filling fraction is what sets the fuel per unit volume, so it is the
    // quantity that must be right. The pitch is therefore solved so the CLIPPED
    // ball realises the paper's stated 0.61, giving 6.4520 cm -- 5 % more balls,
    // each 5 % smaller, for the same fuel density. Inradius 3.2260 cm still
    // clears the 3.0 cm ball, so nothing is tangent.
    //
    // Setting the pitch to the ball DIAMETER instead also gives ~0.61, but makes
    // the ball exactly tangent to all six prism faces. That degeneracy is real
    // and was measured; it is not what caused the k = 0 failure (see
    // verification_and_validation/htr10_rmc/README.md), but it is avoided here.
    let lat_height = cell.height * 0.5;
    let r_ball = 0.5 * cell.ball_diameter;
    let cap = r_ball - 0.5 * lat_height; // axial cap removed at each end
    let v_ball = 4.0 / 3.0 * std::f64::consts::PI * r_ball.powi(3)
        - 2.0 * std::f64::consts::PI * cap * cap * (3.0 * r_ball - cap) / 3.0;
    // packing = v_ball / ((sqrt(3)/2) * pitch^2 * height)  ->  solve for pitch.
    let lat_pitch =
        (v_ball / (PAPER_FILLING_FRACTION * (3.0_f64.sqrt() / 2.0) * lat_height)).sqrt();

    // Bed envelope: a cylinder just containing the tiles, then the reflector out
    // to the published 1 m thickness.
    let bed_radius = lat_pitch * (n_rings as f64 + 0.5);
    let bed_half_height = 0.5 * lat_height * n_axial as f64;
    // OUTRAM_HTR10_NOREFL=1 collapses the reflector to zero thickness. With
    // OUTRAM_HTR10_REFLECTIVE=1 and OUTRAM_HTR10_ALLFUEL=1 that makes the model
    // an INFINITE MEDIUM of fuel pebbles at the paper's filling fraction -- the
    // one configuration directly comparable to the independently measured
    // single-pebble k_inf from the DhUniverse path. Two implementations, one
    // physical problem: they must agree or one of them is wrong.
    let refl_thickness = if std::env::var("OUTRAM_HTR10_NOREFL").is_ok() { 0.0 } else { 100.0 };
    let refl_radius = bed_radius + refl_thickness;
    let refl_half_height = bed_half_height + refl_thickness;

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
    let bed_cell = Cell::fill(1, bed_region.clone(), CellFill::Lattice(0), Position::ZERO);
    let bed = if majorant_index == usize::MAX {
        bed_cell
    } else {
        bed_cell.delta_tracked(majorant_index)
    };

    // The reflector: everything else inside the vacuum boundary, SURFACE-tracked.
    let mut refl_region = vec![
        ins(5), out(6), RegionToken::Intersection, ins(7), RegionToken::Intersection,
    ];
    refl_region.extend(bed_region);
    refl_region.push(RegionToken::Complement);
    refl_region.push(RegionToken::Intersection);
    let reflector = Cell::material(2, refl_region, mat::REFLECTOR, 293.6);

    // OUTRAM_HTR10_ALLFUEL=1 makes EVERY tile a fuelled pebble. Not physical --
    // the first critical core is 57 % fuel / 43 % graphite dummies -- but it is
    // an ABLATION that bounds how much of a k deficit the dilution can explain.
    let mod_universe = if std::env::var("OUTRAM_HTR10_ALLFUEL").is_ok() { 1 } else { 2 };
    let levels = bed_tile_levels(n_rings, n_axial, 1, mod_universe);
    let tiles: usize = levels.iter().flatten().map(|r| r.len()).sum();
    let lattice = HexLattice::from_rings_3d(
        0,
        HexOrientation::Y,
        Position::new(0.0, 0.0, -bed_half_height + 0.5 * lat_height),
        lat_pitch,
        lat_height,
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

    // ONE BALL PER TILE, so the lattice pitch is NOT the paper's cell pitch.
    //
    // `HexBedCell::from_paper()` describes a two-layer prism holding TWO balls
    // (half-spheres on each face plus full balls between), pitch 6.6106 and
    // height 9.798. A hex LATTICE places one universe per tile, so using those
    // dimensions directly gives packing 0.3050 -- exactly half the stated 0.61,
    // and the source of a 2x fuel deficit that made the first assembled core
    // too subcritical to bank a single fission neutron.
    //
    // A one-ball-per-prism lattice cannot reach 0.61 without overlap: its
    // maximum is pitch = height = one ball diameter, giving
    //
    //     packing = V_ball / ((sqrt(3)/2) d^3) = 0.6046
    //
    // with neighbouring balls exactly touching. That is 0.9 % below the stated
    // 0.61, and it is the honest ceiling for this representation rather than an
    // approximation chosen for convenience. Reaching 0.61 needs the paper's
    // offset half-sphere arrangement, which a single-universe tile cannot hold.
    // ONE ball per tile, in a tile that is HALF the paper's two-ball prism.
    //
    // The ball (6.0 cm across) is taller than the tile (4.899 cm), so the
    // lattice CLIPS it axially -- and that clipping is not cosmetic: it removes
    // two spherical caps of 2.6816 cm^3 each from a 113.0973 cm^3 ball, leaving
    // 107.7342 cm^3. At the paper's own pitch the realised filling fraction is
    // therefore 0.5811, NOT the 0.610 an unclipped ball would give. (An earlier
    // comment here claimed the halved height "reproduces the packing exactly";
    // it does not, and was wrong by 4.74 %.)
    //
    // Filling fraction sets the fuel per unit volume, so it is the quantity that
    // must be right. The pitch is solved so the CLIPPED ball realises the
    // paper's stated 0.61, giving 6.4520 cm -- 5 % more balls, each 5 % smaller,
    // for the same fuel density. Inradius 3.2260 cm clears the 3.0 cm ball.
    //
    // Setting the pitch to the ball DIAMETER instead also gives ~0.61, but makes
    // the ball exactly tangent to all six prism faces. That degeneracy is real
    // and was measured; it is NOT what caused the k = 0 failure (that was the
    // HexLattice axial-frame defect, see the V&V record), but it is avoided.
    let lat_height = cell.height * 0.5;
    let r_ball = 0.5 * cell.ball_diameter;
    let cap = r_ball - 0.5 * lat_height; // axial cap removed at each end
    let v_ball = 4.0 / 3.0 * std::f64::consts::PI * r_ball.powi(3)
        - 2.0 * std::f64::consts::PI * cap * cap * (3.0 * r_ball - cap) / 3.0;
    // packing = v_ball / ((sqrt(3)/2) * pitch^2 * height)  ->  solve for pitch.
    let lat_pitch =
        (v_ball / (PAPER_FILLING_FRACTION * (3.0_f64.sqrt() / 2.0) * lat_height)).sqrt();

    // Adjudicated radii (op-867c.12): TECDOC-1382, 90 um buffer.
    let tr = [0.0250_f64, 0.0340, 0.0380, 0.0415, 0.0455];
    let r_part = tr[4];
    let (pitch_triso, _n_particles) = cubic_pitch_for_count(r_part, r_fuel_zone, 8335, [0.5, 0.5, 0.0]);
    // A cubic lattice spanning the fuel zone; tiles outside it fall through to
    // `outer` = matrix graphite, which is exactly the "not occupied is filled
    // with graphite" the paper specifies.
    // The lattice spans the fuel-zone DIAMETER, but its tiles are assigned
    // per-tile: a particle universe where the tile centre lies within the
    // whole-particle radius, matrix graphite elsewhere. That is how a cube
    // lattice expresses a sphere-clipped array -- the same technique the bed
    // uses for its 57:43 split.
    //
    // Two earlier attempts were wrong and are recorded because each failed
    // differently. Sizing the lattice to the zone DIAMETER leaves its corner
    // tiles poking outside the sphere (26 tiles x 0.194963 = 5.069 cm against
    // 5.0 cm), which produced a stuck crossing loop -- 90 s for 400 histories.
    // Inscribing it instead (half = r/sqrt(3)) removed the loop but cut the
    // fuel from 8340 particles to 2744, making the model too dilute to multiply.
    // CEIL, not floor. The lattice must COVER the fuel-zone sphere it fills:
    // with `floor` the lattice half-width is 2.4370 cm inside a 2.5 cm sphere,
    // so the shell between them lies inside the cell but OUTSIDE the lattice.
    // `Lattice::distance` then reports a tile boundary BEHIND the particle -- a
    // negative distance-to-boundary, measured at 3.3e8 occurrences (worst
    // -2.8e4 cm), which steps the neutron backwards until it oscillates and the
    // event budget kills it. That single sign error was ending 83 % of all
    // histories. The extra ring of tiles simply carries the matrix universe.
    let n_triso = ((2.0 * r_fuel_zone / pitch_triso).ceil() as usize).max(1);
    let lattice_half = 0.5 * n_triso as f64 * pitch_triso;

    let bed_radius = lat_pitch * (n_rings as f64 + 0.5);
    let bed_half_height = 0.5 * lat_height * n_axial as f64;
    // OUTRAM_HTR10_NOREFL=1 collapses the reflector to zero thickness. With
    // OUTRAM_HTR10_REFLECTIVE=1 and OUTRAM_HTR10_ALLFUEL=1 that makes the model
    // an INFINITE MEDIUM of fuel pebbles at the paper's filling fraction -- the
    // one configuration directly comparable to the independently measured
    // single-pebble k_inf from the DhUniverse path. Two implementations, one
    // physical problem: they must agree or one of them is wrong.
    let refl_thickness = if std::env::var("OUTRAM_HTR10_NOREFL").is_ok() { 0.0 } else { 100.0 };
    let refl_radius = bed_radius + refl_thickness;
    let refl_half_height = bed_half_height + refl_thickness;

    // Surfaces 0..4 are the TRISO shells, in PARTICLE-local coordinates.
    let mut surfaces: Vec<SurfaceKind> = tr
        .iter()
        .map(|&r| SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc: BoundaryType::Transmissive }))
        .collect();
    // 5: fuel zone, 6: pebble -- in TILE-local coordinates.
    surfaces.push(SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_fuel_zone, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_pebble, bc: BoundaryType::Transmissive }));
    // 7..9 bed envelope, 10..12 reflector vacuum boundary, 13..14 tile clip.
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: bed_radius, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: -bed_half_height, bc: BoundaryType::Transmissive }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: bed_half_height, bc: BoundaryType::Transmissive }));
    // OUTRAM_HTR10_REFLECTIVE=1 closes the outer boundary. NOT physical -- it is
    // a DIAGNOSTIC that separates the two ways k can be low: with no leakage at
    // all, whatever k remains is pure in-model absorption or lost histories.
    let obc = if std::env::var("OUTRAM_HTR10_REFLECTIVE").is_ok() {
        BoundaryType::Reflective
    } else {
        BoundaryType::Vacuum
    };
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: refl_radius, bc: obc }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: -refl_half_height, bc: obc }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: refl_half_height, bc: obc }));
    // OUTRAM_HTR10_REFLECTIVE=1 closes the outer boundary. NOT physical -- it is
    // a DIAGNOSTIC that separates the two ways k can be low: with no leakage at
    // all, whatever k remains is pure in-model absorption or lost histories.

    let ins = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside };
    let out = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside };

    let shell = |inner: usize, outer: usize, m: usize, id: i32| {
        Cell::material(id, vec![out(inner), ins(outer), RegionToken::Intersection], m, 293.6)
    };

    // Universe 3 -- one TRISO particle: five shells then matrix.
    let cells = vec![
        // 0: the bed (delta-tracked)
        {
            let bed = Cell::fill(
                1,
                vec![ins(7), out(8), RegionToken::Intersection, ins(9), RegionToken::Intersection],
                CellFill::Lattice(0),
                Position::ZERO,
            );
            // `usize::MAX` means "surface-track the bed too", so the SAME
            // geometry can be run both ways. That is the discriminator for the
            // k = 0 failure: if surface tracking gives a sensible k on this
            // model, the delta path is at fault; if it does not, the model is.
            if majorant_index == usize::MAX { bed } else { bed.delta_tracked(majorant_index) }
        },
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
        // 13, 14: pure matrix, the TRISO lattice's `outer` universe.
        //
        // TWO cells, because a cell region cannot be "everything": an empty
        // token stream evaluates to FALSE, not true. A single `out(4)` cell
        // leaves an undefined 0.0455 cm hole at the centre of every
        // non-particle tile, where `locate` finds no cell and the history is
        // lost. Splitting it into inside/outside the same surface covers the
        // tile completely with one material.
        Cell::material(14, vec![out(4)], mat::GRAPHITE, 293.6),
        Cell::material(15, vec![ins(4)], mat::GRAPHITE, 293.6),
    ];

    // OUTRAM_HTR10_ALLFUEL=1 makes EVERY tile a fuelled pebble. Not physical --
    // the first critical core is 57 % fuel / 43 % graphite dummies -- but it is
    // an ABLATION that bounds how much of a k deficit the dilution can explain.
    let mod_universe = if std::env::var("OUTRAM_HTR10_ALLFUEL").is_ok() { 1 } else { 2 };
    let levels = bed_tile_levels(n_rings, n_axial, 1, mod_universe);
    let tiles: usize = levels.iter().flatten().map(|r| r.len()).sum();
    let bed_lattice = HexLattice::from_rings_3d(
        0, HexOrientation::Y,
        Position::new(0.0, 0.0, -bed_half_height + 0.5 * lat_height),
        lat_pitch, lat_height, &levels, Some(2),
    );
    let triso_lattice = RectLattice {
        id: 1,
        n: [n_triso, n_triso, n_triso],
        lower_left: Position::new(-lattice_half, -lattice_half, -lattice_half),
        pitch: [pitch_triso; 3],
        universes: {
            // Whole-particle rejection, the benchmark's own rule, applied per
            // tile: keep a particle only where it lies wholly inside the zone.
            let r_keep = r_fuel_zone - r_part;
            let mut v = Vec::with_capacity(n_triso.pow(3));
            for k in 0..n_triso {
                for j in 0..n_triso {
                    for i in 0..n_triso {
                        let c = |n: usize| -lattice_half + (n as f64 + 0.5) * pitch_triso;
                        let (x, y, z) = (c(i), c(j), c(k));
                        v.push(if x * x + y * y + z * z <= r_keep * r_keep { 3 } else { 4 });
                    }
                }
            }
            v
        },
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
            Universe { id: 4, cell_indices: vec![13, 14] },         // matrix (lattice outer)
        ],
        lattices: vec![Lattice::Hex(bed_lattice), Lattice::Rect(triso_lattice)],
        root_universe: 0,
    };
    let (c, u) = (geometry.cells.len(), geometry.universes.len());
    AssembledCore { geometry, tiles, cells: c, universes: u }
}
