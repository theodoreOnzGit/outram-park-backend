// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header of `htr10_rmc` for
// licence terms.

//! # The HTR-10 core AROUND the pebble bed
//!
//! Everything [`super::core_model::assemble_explicit_triso`] builds that does
//! not depend on how the pebbles are arranged: the TRISO particle and its
//! lattice, the bed envelope (cylinder, conus, discharge tube), the empty core
//! cavity, TECDOC zone 0, the explicit reflector with its borings and rods,
//! and the final `Geometry`.
//!
//! **Extracted 2026-10-05, not rewritten.** The code and its comments were
//! moved verbatim out of `assemble_explicit_triso` so that a second bed
//! builder, the explicit (DEM) bed of [`super::explicit_bed`], reuses the same
//! construction instead of duplicating it. Comments written for the Şeker
//! builder still say so; "the ABottom ball site" for surfaces 5/6 is now
//! "whatever site 0 the bed builder passes". Surface indices 0..21, the root
//! cells and universes 0..2 are exactly those the Şeker path always built,
//! in the same order, so every surface index callers and tests use is
//! unchanged.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, Lattice, RectLattice};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCone, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::pebble_beds::sphere_packing::cubic_pitch_for_count;

use super::bed::PebbleBed;
use super::core_model::{
    cavity_above_bed, mat, AssembledCore, HTR10_AXIAL_REFLECTOR_CM, HTR10_BOTTOM_REFLECTOR_CM,
    HTR10_CONTROL_ROD_INNER_CM, HTR10_CONTROL_ROD_OUTER_CM, HTR10_CONUS_HEIGHT_CM,
    HTR10_COOLANT_INNER_CM, HTR10_COOLANT_OUTER_CM, HTR10_CORE_RADIUS_CM,
    HTR10_DISCHARGE_TUBE_RADIUS_CM, HTR10_GRAPHITE_OUTER_CM, HTR10_REFLECTOR_OUTER_CM,
    TRISO_MATRIX_UNIVERSE, TRISO_PARTICLE_UNIVERSE,
};
use super::reflector_geometry::{
    build_reflector, ReflectorFrame, ReflectorMaterials, ReflectorOptions, Rgn,
};

/// Inside half-space of surface `i`.
pub(super) fn ins(i: usize) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Inside,
    }
}

/// Outside half-space of surface `i`.
pub(super) fn out(i: usize) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Outside,
    }
}

/// A material shell between two surfaces: outside `inner`, inside `outer`.
pub(super) fn shell(inner: usize, outer: usize, m: usize, id: i32) -> Cell {
    Cell::material(
        id,
        vec![out(inner), ins(outer), RegionToken::Intersection],
        m,
        293.6,
    )
}

/// The radial and axial layout of the model around a bed of half-height
/// `bed_half_height` \[cm\] (bed floor at `-bed_half_height`), with the
/// environment ablations (`OUTRAM_HTR10_NOREFL`, `OUTRAM_HTR10_HOMOG_TUBE`)
/// read once.
#[derive(Debug, Clone, Copy)]
pub(super) struct CoreFrame {
    pub bed_radius: f64,
    pub bed_half_height: f64,
    pub refl_thickness: f64,
    pub refl_radius: f64,
    pub graphite_outer: f64,
    pub refl_top: f64,
    pub refl_bottom: f64,
    pub has_refl: bool,
    pub explicit_tube: bool,
}

impl CoreFrame {
    pub(super) fn new(bed_radius: f64, bed_half_height: f64) -> Self {
        // OUTRAM_HTR10_NOREFL=1 collapses the reflector to zero thickness. With
        // OUTRAM_HTR10_REFLECTIVE=1 and OUTRAM_HTR10_ALLFUEL=1 that makes the model
        // an INFINITE MEDIUM of fuel pebbles at the paper's filling fraction -- the
        // one configuration directly comparable to the independently measured
        // single-pebble k_inf from the DhUniverse path. Two implementations, one
        // physical problem: they must agree or one of them is wrong.
        let refl_thickness = if std::env::var("OUTRAM_HTR10_NOREFL").is_ok() {
            0.0
        } else {
            100.0
        };
        // Radial reflector structure is PHYSICAL, from Terry (2005) Fig. 2, not
        // `bed_radius + 100`: graphite out to 167.793 cm, then BORONATED CARBON
        // BRICKS to the 190 cm outer boundary. Modelling the whole reflector as
        // clean graphite omits that absorber entirely and is optimistic -- measured
        // at +8496 pcm against RMC with it missing.
        let refl_radius = if refl_thickness > 0.0 {
            HTR10_REFLECTOR_OUTER_CM
        } else {
            bed_radius
        };
        let graphite_outer = if refl_thickness > 0.0 {
            HTR10_GRAPHITE_OUTER_CM
        } else {
            bed_radius
        };
        // The axial reflector must sit ABOVE the cavity, not be consumed by it.
        // With `bed_half_height + 100` the cavity top (bed + 98.758) left barely a
        // centimetre of graphite before vacuum, so the cavity vented almost
        // directly to the outside -- measured at 15.7 % leakage.
        let refl_top = if refl_thickness > 0.0 {
            bed_half_height + cavity_above_bed(2.0 * bed_half_height) + HTR10_AXIAL_REFLECTOR_CM
        } else {
            bed_half_height
        };
        // The bottom is placed from the reactor, NOT mirrored from the top: the
        // conus floor, then the fixed 221.236 cm bottom reflector (Terry 2005
        // Fig. 2, z 388.764 -> 610). See `HTR10_BOTTOM_REFLECTOR_CM`.
        let refl_bottom = if refl_thickness > 0.0 {
            -bed_half_height - HTR10_CONUS_HEIGHT_CM - HTR10_BOTTOM_REFLECTOR_CM
        } else {
            -bed_half_height
        };

        let has_refl = refl_thickness > 0.0;
        let explicit_tube = has_refl && std::env::var("OUTRAM_HTR10_HOMOG_TUBE").is_err();
        Self {
            bed_radius,
            bed_half_height,
            refl_thickness,
            refl_radius,
            graphite_outer,
            refl_top,
            refl_bottom,
            has_refl,
            explicit_tube,
        }
    }
}

/// The built core minus the bed lattice: surfaces 0..21, the root cells, the
/// TRISO cells, universes 0..2 and the TRISO lattice. A bed builder appends
/// its own surfaces, tile cells and universes, then calls
/// [`Self::add_reflector`] and [`Self::finish`].
pub(super) struct CoreShell {
    pub surfaces: Vec<SurfaceKind>,
    pub cells: Vec<Cell>,
    pub universes: Vec<Universe>,
    pub frame: CoreFrame,
    pub conus_floor: f64,
    pub cavity_top: f64,
    zone_map: bool,
    withdrawn_rods: bool,
    triso_lattice: RectLattice,
    reflector_built: bool,
}

impl CoreShell {
    /// Build everything but the bed tiles and the reflector. `site0` is the
    /// tile-local centre \[cm\] given to surfaces 5 (fuel zone) and 6
    /// (pebble); `r_fuel_zone` and `r_pebble` \[cm\]; `majorant_index` as
    /// for `assemble_explicit_triso`.
    pub(super) fn new(
        frame: &CoreFrame,
        site0: [f64; 3],
        r_fuel_zone: f64,
        r_pebble: f64,
        majorant_index: usize,
    ) -> Self {
        let CoreFrame {
            bed_radius,
            bed_half_height,
            refl_thickness,
            refl_radius,
            graphite_outer,
            refl_top,
            refl_bottom,
            has_refl,
            explicit_tube,
        } = *frame;
        // Adjudicated radii (op-867c.12): TECDOC-1382, 90 um buffer.
        let tr = [0.0250_f64, 0.0340, 0.0380, 0.0415, 0.0455];
        let r_part = tr[4];
        // ONE offset, used both to COUNT the particles and to BUILD the lattice
        // (gh:#316). Until 2026-09-25 the count used [0.5, 0.5, 0.0] -- 8340
        // particles -- while the RectLattice below was built with (i + 0.5) centres
        // on ALL three axes (26 cells each), which holds only 8240. Every fuel
        // pebble carried 1.2 % less heavy metal than reported; sampling the built
        // core measured 0.9875 +/- 0.0014 of the paper-implied kernel fraction,
        // against 8240/8340 = 0.9880.
        //
        // ~~`[0.5, 0.5, 0.0]`, 8340 particles (+0.06 %)~~ **CHANGED 2026-10-01
        // (gh:#430):** a generic offset reaches the stated 8335 exactly. Şeker &
        // Çolak (2003) p.266 build the same whole-particle cubic lattice and state
        // that the count *"is verified to be 8335"*. The symmetric offsets tried
        // before can only move the count in symmetry shells; this one breaks the
        // symmetry, and the pitch search finds 8335 at 0.1951235 cm. That is the
        // source's specification realised, not a comparison tuned: 8335 is an
        // INPUT (Li Table 2, Şeker p.266), and the `assert_eq!` below checks the
        // built lattice holds exactly the counted number.
        const TRISO_OFFSET: [f64; 3] = [0.13, 0.37, 0.71];
        let (pitch_triso, n_particles) =
            cubic_pitch_for_count(r_part, r_fuel_zone, 8335, TRISO_OFFSET);
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
        //
        // Per axis, the cells whose centres sit at `(k + offset) * pitch` and
        // together COVER [-r_fuel_zone, r_fuel_zone]: an offset of 0.5 gives an
        // even count centred on a cell face (26 here), 0.0 an odd count centred on
        // a cell (27 here). The covering rule is the CEIL rule above, per axis.
        let triso_axis = |off: f64| -> (usize, f64) {
            let k_lo = (-r_fuel_zone / pitch_triso - off + 0.5).floor();
            let k_hi = (r_fuel_zone / pitch_triso - off - 0.5).ceil();
            ((k_hi - k_lo) as usize + 1, (k_lo + off - 0.5) * pitch_triso)
        };
        let triso_axes = TRISO_OFFSET.map(triso_axis);

        // Surfaces 0..4 are the TRISO shells, in PARTICLE-local coordinates.
        let mut surfaces: Vec<SurfaceKind> = tr
            .iter()
            .map(|&r| {
                SurfaceKind::Sphere(Sphere {
                    x0: 0.0,
                    y0: 0.0,
                    z0: 0.0,
                    r,
                    bc: BoundaryType::Transmissive,
                })
            })
            .collect();
        // 5: fuel zone, 6: pebble -- in TILE-local coordinates, of the ABottom
        // ball site (0, 0, -height/2). The other four sites' pairs are appended
        // after surface 21 (`site_surfaces` below), so that 7..21 keep the indices
        // every caller and test already uses.
        let site_sphere = |_site: usize, r: f64| {
            let [x0, y0, z0] = site0;
            SurfaceKind::Sphere(Sphere {
                x0,
                y0,
                z0,
                r,
                bc: BoundaryType::Transmissive,
            })
        };
        surfaces.push(site_sphere(0, r_fuel_zone));
        surfaces.push(site_sphere(0, r_pebble));
        // 7..9 bed envelope, 10..12 reflector vacuum boundary, 13..14 tile clip.
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: bed_radius,
            bc: BoundaryType::Transmissive,
        }));
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: -bed_half_height,
            bc: BoundaryType::Transmissive,
        }));
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: bed_half_height,
            bc: BoundaryType::Transmissive,
        }));
        // OUTRAM_HTR10_REFLECTIVE=1 closes the outer boundary. NOT physical -- it is
        // a DIAGNOSTIC that separates the two ways k can be low: with no leakage at
        // all, whatever k remains is pure in-model absorption or lost histories.
        let obc = if std::env::var("OUTRAM_HTR10_REFLECTIVE").is_ok() {
            BoundaryType::Reflective
        } else {
            BoundaryType::Vacuum
        };
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: refl_radius,
            bc: obc,
        }));
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: refl_bottom,
            bc: obc,
        }));
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: refl_top,
            bc: obc,
        }));
        // 13: graphite / boronated-brick interface (Terry 2005 Fig. 2).
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: graphite_outer,
            bc: BoundaryType::Transmissive,
        }));
        // 14, 15: the COLD COOLANT FLOW annulus, 140.6 -> 148.6 cm. ~~Modelling it as
        // solid graphite (as this did) overstates the reflector: it is a helium
        // flow path, i.e. effectively void, and leaving it solid suppresses
        // leakage that the real reactor has.~~ CORRECTED 2026-10-01 (gh:#428): no
        // annulus is built. Since 2026-09-25 the twenty coolant channels are
        // explicit 8 cm bores in solid graphite (`reflector_geometry`), and no cell
        // in this function references surfaces 14 or 15 (checked by search). They
        // are kept, like 19/20, so every later surface index stays put.
        let (cool_in, cool_out) = if refl_thickness > 0.0 {
            (HTR10_COOLANT_INNER_CM, HTR10_COOLANT_OUTER_CM)
        } else {
            (graphite_outer, graphite_outer)
        };
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: cool_in,
            bc: BoundaryType::Transmissive,
        }));
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: cool_out,
            bc: BoundaryType::Transmissive,
        }));
        // 17, 18: the CONUS — the sloping bottom of the pebble bed.
        //
        // A cone from r = 90 cm at the bed bottom down to the 25 cm discharge tube
        // over 36.946 cm. `ZCone` is the OpenMC quadric
        // `(x-x0)^2 + (y-y0)^2 - r_sq*(z-z0)^2`, so `z0` is the APEX (where r = 0)
        // and `r_sq` is the slope squared. The apex sits below the conus bottom
        // because the conus is a frustum, not a full cone: with slope
        // (90-25)/36.946 = 1.759324 the apex is 90/1.759324 = 51.156 cm below the
        // bed bottom. `conus_floor` then truncates it at the discharge-tube radius.
        let conus_slope =
            (HTR10_CORE_RADIUS_CM - HTR10_DISCHARGE_TUBE_RADIUS_CM) / HTR10_CONUS_HEIGHT_CM;
        let conus_apex_z = -bed_half_height - HTR10_CORE_RADIUS_CM / conus_slope;
        let conus_floor = -bed_half_height - HTR10_CONUS_HEIGHT_CM;
        // 16: top of the empty core cavity above the pebble bed.
        let cavity_top = if refl_thickness > 0.0 {
            bed_half_height + cavity_above_bed(2.0 * bed_half_height)
        } else {
            bed_half_height
        };
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: cavity_top,
            bc: BoundaryType::Transmissive,
        }));
        surfaces.push(SurfaceKind::ZCone(ZCone {
            x0: 0.0,
            y0: 0.0,
            z0: conus_apex_z,
            r_sq: conus_slope * conus_slope,
            bc: BoundaryType::Transmissive,
        }));
        surfaces.push(SurfaceKind::ZPlane(ZPlane {
            z0: conus_floor,
            bc: BoundaryType::Transmissive,
        }));
        // 19, 20: the side-reflector band carrying the control-rod borings,
        // 95.6 -> 108.6 cm. ~~A homogenised band of TECDOC zones 31-40 behind
        // `OUTRAM_HTR10_BORINGS` (default off: its core-height placement was
        // unrecorded).~~ **REPLACED 2026-09-25:** the borings are explicit
        // geometry (`super::reflector_geometry`), placed from TECDOC-1382 p. 242
        // and Fig. 4.10, and the knob is gone. These two surfaces are kept only so
        // every surface index after them stays put; the reflector builder reuses
        // them as the band's boundaries.
        for r in [HTR10_CONTROL_ROD_INNER_CM, HTR10_CONTROL_ROD_OUTER_CM] {
            surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.0,
                y0: 0.0,
                r,
                bc: BoundaryType::Transmissive,
            }));
        }
        // 21: the FUEL DISCHARGE TUBE wall, r = 25 cm (TECDOC-1382 p. 242:
        // 0 < R < 250 mm, from the conus floor to the model bottom). It holds
        // whole graphite balls: see `DischargeTube`. ~~`OUTRAM_HTR10_NO_DISCHARGE=1`
        // collapsed it to restore graphite.~~ **REMOVED 2026-09-25**: the tube is
        // explicit; `OUTRAM_HTR10_HOMOG_TUBE=1` is the ablation now.
        surfaces.push(SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: HTR10_DISCHARGE_TUBE_RADIUS_CM,
            bc: BoundaryType::Transmissive,
        }));

        // Ablations of the explicit reflector and discharge tube (2026-09-25).
        // Each is a named, visible act; the default builds everything.
        //
        // - OUTRAM_HTR10_HOMOG_TUBE=1: the discharge tube is the old smear
        //   (`mat::HOMOG_DUMMY`) and the cone CUTS partial balls, instead of whole
        //   graphite balls with Li (2014)'s rejection at the cone and tube.
        // - OUTRAM_HTR10_NO_ZONE_MAP=1: every Fig. 4.10 zone is zone-22 graphite
        //   except the boronated bricks at r > 167.793 cm (the reflector before the
        //   zone map), the borings still explicit.
        // - OUTRAM_HTR10_NO_WITHDRAWN_RODS=1: the rod channels are empty.
        let zone_map = std::env::var("OUTRAM_HTR10_NO_ZONE_MAP").is_err();
        let withdrawn_rods = std::env::var("OUTRAM_HTR10_NO_WITHDRAWN_RODS").is_err();
        let zone_material = move |z: usize| {
            if zone_map {
                mat::for_zone_mc(z)
            } else {
                mat::for_zone_uniform(z)
            }
        };

        // With no reflector (OUTRAM_HTR10_NOREFL=1) the bed cylinder IS the edge
        // of the model, so its surfaces carry the outer boundary condition.
        // ~~The reflector surfaces were collapsed onto the bed's, leaving
        // zero-volume shells whose boundary condition acted on nothing.~~
        // **CHANGED 2026-09-25**: with no reflector, no reflector cells are built.
        if !has_refl {
            for i in [7, 8, 9] {
                match &mut surfaces[i] {
                    SurfaceKind::ZCylinder(c) => c.bc = obc,
                    SurfaceKind::ZPlane(p) => p.bc = obc,
                    _ => unreachable!("surfaces 7, 8, 9 are the bed cylinder and planes"),
                }
            }
        }

        // Root cells, in search order: the bed first (every history starts there
        // and spends most of its time there), then the cavity and zone 0, then
        // the reflector (appended by `build_reflector`).
        let bed_region = {
            let cyl = Rgn::ins(7).and(Rgn::out(8)).and(Rgn::ins(9));
            if has_refl {
                // The bed is the cylinder UNION the conus (UNION the discharge
                // tube, when it holds explicit balls).
                //
                // This is the whole mechanism -- no per-tile omission is needed.
                // `Geometry::locate` calls `find_cell` (a CSG region test) BEFORE
                // descending into the lattice, so a point is only given a tile if
                // it is inside this region. The cone clips the pebble lattice
                // exactly as `ins(7)` clips it to r < 90 cm. With the rejection rule
                // (Şeker & Çolak 2003 p.267, which Li follows; `SekerBed` since
                // 2026-10-01) no kept ball reaches the cone, the tube or the side
                // wall, so there the cut only ever passes through helium.
                let conus = Rgn::ins(17).and(Rgn::ins(8)).and(Rgn::out(18));
                let r = cyl.or(conus);
                if explicit_tube {
                    r.or(Rgn::ins(21).and(Rgn::ins(18)).and(Rgn::out(11)))
                } else {
                    r
                }
            } else {
                cyl
            }
        };
        let mut cells = vec![{
            let bed = Cell::fill(1, bed_region.0, CellFill::Lattice(0), Position::ZERO);
            // `usize::MAX` means "surface-track the bed too", so the SAME
            // geometry can be run both ways. That is the discriminator for the
            // k = 0 failure: if surface tracking gives a sensible k on this
            // model, the delta path is at fault; if it does not, the model is.
            if majorant_index == usize::MAX {
                bed
            } else {
                bed.delta_tracked(majorant_index)
            }
        }];
        if has_refl {
            // The EMPTY CORE CAVITY above the pebble bed -- helium, not graphite.
            cells.push(Cell::material(
                5,
                Rgn::ins(7).and(Rgn::out(9)).and(Rgn::ins(16)).0,
                mat::HELIUM,
                293.6,
            ));
            // TECDOC zone 0: between the cone and r = 90 cm, from the conus top to
            // the conus floor (Fig. 4.10) -- the bottom reflector with its hot
            // helium borings, whose geometry the source does not give.
            cells.push(Cell::material(
                2,
                Rgn::ins(7)
                    .and(Rgn::out(17))
                    .and(Rgn::ins(8))
                    .and(Rgn::out(18))
                    .0,
                zone_material(0),
                293.6,
            ));
            if !explicit_tube {
                // ABLATION: the smeared discharge tube, conus floor to model bottom.
                cells.push(Cell::material(
                    17,
                    Rgn::ins(21).and(Rgn::ins(18)).and(Rgn::out(11)).0,
                    mat::HOMOG_DUMMY,
                    293.6,
                ));
            }
        }
        let n_root_fixed = cells.len();
        let triso_first = cells.len();
        cells.extend([
            // 7..12: the TRISO particle's shells, then matrix beyond it
            // (universe TRISO_PARTICLE_UNIVERSE)
            Cell::material(8, vec![ins(0)], mat::KERNEL, 293.6),
            shell(0, 1, mat::BUFFER, 9),
            shell(1, 2, mat::IPYC, 10),
            shell(2, 3, mat::SIC, 11),
            shell(3, 4, mat::OPYC, 12),
            Cell::material(13, vec![out(4)], mat::GRAPHITE, 293.6),
            // 13, 14: pure matrix, the TRISO lattice's `outer` universe
            // (TRISO_MATRIX_UNIVERSE).
            //
            // TWO cells, because a cell region cannot be "everything": an empty
            // token stream evaluates to FALSE, not true. A single `out(4)` cell
            // leaves an undefined 0.0455 cm hole at the centre of every
            // non-particle tile, where `locate` finds no cell and the history is
            // lost. Splitting it into inside/outside the same surface covers the
            // tile completely with one material.
            Cell::material(14, vec![out(4)], mat::GRAPHITE, 293.6),
            Cell::material(15, vec![ins(4)], mat::GRAPHITE, 293.6),
        ]);
        let universes = vec![
            Universe {
                id: 0,
                cell_indices: (0..n_root_fixed).collect(),
            },
            Universe {
                id: TRISO_PARTICLE_UNIVERSE as i32,
                cell_indices: (triso_first..triso_first + 6).collect(),
            },
            Universe {
                id: TRISO_MATRIX_UNIVERSE as i32,
                cell_indices: vec![triso_first + 6, triso_first + 7],
            },
        ];

        let (tp, tm) = (TRISO_PARTICLE_UNIVERSE, TRISO_MATRIX_UNIVERSE);
        let triso_lattice = RectLattice {
            id: 1,
            n: triso_axes.map(|(n, _)| n),
            lower_left: Position::new(triso_axes[0].1, triso_axes[1].1, triso_axes[2].1),
            pitch: [pitch_triso; 3],
            universes: {
                // Whole-particle rejection, the benchmark's own rule, applied per
                // tile: keep a particle only where it lies wholly inside the zone.
                let r_keep = r_fuel_zone - r_part;
                let [(nx, x0), (ny, y0), (nz, z0)] = triso_axes;
                let c = |lo: f64, n: usize| lo + (n as f64 + 0.5) * pitch_triso;
                let mut v = Vec::with_capacity(nx * ny * nz);
                for k in 0..nz {
                    for j in 0..ny {
                        for i in 0..nx {
                            let (x, y, z) = (c(x0, i), c(y0, j), c(z0, k));
                            v.push(if x * x + y * y + z * z <= r_keep * r_keep {
                                tp
                            } else {
                                tm
                            });
                        }
                    }
                }
                // The built count MUST be the counted one -- the whole of #316.
                let built = v.iter().filter(|&&u| u == tp).count();
                assert_eq!(
                    built, n_particles,
                    "TRISO lattice holds {built} particles but {n_particles} were counted"
                );
                v
            },
            outer: Some(tm),
        };

        Self {
            surfaces,
            cells,
            universes,
            frame: *frame,
            conus_floor,
            cavity_top,
            zone_map,
            withdrawn_rods,
            triso_lattice,
            reflector_built: false,
        }
    }

    /// Append the explicit reflector (no-op without one). Call once, after
    /// or before the bed tiles; its root cells go to the end of universe 0.
    pub(super) fn add_reflector(&mut self) {
        assert!(!self.reflector_built, "the reflector is built once");
        self.reflector_built = true;
        let zone_map = self.zone_map;
        let withdrawn_rods = self.withdrawn_rods;
        let refl_top = self.frame.refl_top;
        let has_refl = self.frame.has_refl;
        let (surfaces, cells, universes) =
            (&mut self.surfaces, &mut self.cells, &mut self.universes);
        let zone_material = move |z: usize| {
            if zone_map {
                mat::for_zone_mc(z)
            } else {
                mat::for_zone_uniform(z)
            }
        };
        // THE REFLECTOR, explicit: every boring at its own position in solid
        // graphite, inside TECDOC Fig. 4.10's zone map. See
        // `super::reflector_geometry` for the specification and what it leaves
        // open.
        if has_refl {
            let refl_root = build_reflector(
                surfaces,
                cells,
                universes,
                ReflectorFrame { refl_top },
                ReflectorOptions { withdrawn_rods },
                ReflectorMaterials {
                    helium: mat::HELIUM,
                    b4c: mat::ROD_B4C,
                    steel: mat::ROD_STEEL,
                    iron: mat::ROD_IRON,
                },
                zone_material,
            );
            universes[0].cell_indices.extend(refl_root);
        }
    }

    /// Close the geometry around the bed lattice (lattice 0) and return the
    /// assembled core.
    pub(super) fn finish(
        self,
        bed_lattice: HexLattice,
        tiles: usize,
        lat_pitch: f64,
        lat_height: f64,
        bed: PebbleBed,
    ) -> AssembledCore {
        assert!(self.reflector_built, "call add_reflector before finish");
        let Self {
            surfaces,
            cells,
            universes,
            frame,
            conus_floor,
            cavity_top,
            triso_lattice,
            ..
        } = self;
        let CoreFrame {
            bed_radius,
            bed_half_height,
            refl_top,
            refl_bottom,
            ..
        } = frame;
        let geometry = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![Lattice::Hex(bed_lattice), Lattice::Rect(triso_lattice)],
            root_universe: 0,
        };
        let (c, u) = (geometry.cells.len(), geometry.universes.len());
        AssembledCore {
            geometry,
            tiles,
            cells: c,
            universes: u,
            bed_radius,
            bed_half_height,
            lat_pitch,
            lat_height,
            conus_floor,
            cavity_top,
            refl_top,
            refl_bottom,
            bed: Some(bed),
        }
    }
}
