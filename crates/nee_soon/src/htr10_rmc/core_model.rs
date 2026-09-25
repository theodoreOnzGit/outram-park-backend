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
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCone, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;

use super::bed::{
    bed_tile_levels, hex_ring, BallSite, DischargeTube, FuelAssignment, HexBedCell, TwoBallBed,
};
use super::reflector_geometry::{
    build_reflector, ReflectorFrame, ReflectorMaterials, ReflectorOptions, Rgn,
};

/// Material slots the assembled geometry expects, in order.
///
/// The first five mirror `DhUniverse::pebble`'s TRISO layer order so
/// `pebble_beds::htr10::fuel_pebble_materials` can be used directly.
/// Pebble-bed filling fraction stated by Li, Yu & Wei (2014) for the HTR-10
/// core — the fraction of bed volume occupied by pebbles. Sets the fuel per
/// unit volume. ~~so the assembled geometry is solved to realise it~~
/// **CORRECTED 2026-09-25:** [`assemble_explicit_triso`] realises it by
/// construction (the paper's two-ball cell, [`HexBedCell::from_paper`], whose
/// pitch is derived from it); only [`assemble`] still solves its pitch for it.
/// Also the discharge-tube smear (`mat::HOMOG_DUMMY`), which since the
/// two-ball cell agrees with the bed it homogenises (sampled 0.6096-0.6097).
pub const PAPER_FILLING_FRACTION: f64 = 0.61;

/// HTR-10 active core radius \[cm\] — 180 cm diameter (IAEA-TECDOC-1382).
/// The bed cylinder is built at this radius and the hex lattice is sized to
/// tile it completely.
pub const HTR10_CORE_RADIUS_CM: f64 = 90.0;

/// Outer radius \[cm\] of the graphite reflector, where the boronated carbon
/// bricks begin — Terry et al. (2005) Fig. 2, via
/// `kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md`.
pub const HTR10_GRAPHITE_OUTER_CM: f64 = 167.793;

/// Inner radius \[cm\] of the cold coolant flow annulus — Terry (2005) Fig. 2,
/// independently corroborated as channel r 144.6 − diameter 8.0 / 2.
pub const HTR10_COOLANT_INNER_CM: f64 = 140.6;

/// Outer radius \[cm\] of the cold coolant flow annulus (144.6 + 8.0 / 2).
pub const HTR10_COOLANT_OUTER_CM: f64 = 148.6;

/// Total height \[cm\] of the **core cavity**, conus top to cavity top.
///
/// Terry (2005) Fig. 2 / IAEA-TECDOC-1382: `z = 130.0` to `351.818`. This is
/// **fixed geometry** — it does not depend on how much fuel is loaded.
pub const HTR10_CORE_CAVITY_CM: f64 = 221.818;

/// Void height \[cm\] above a bed of `bed_full_height` cm.
///
/// # There is only one treatment, and this is it
///
/// The core cavity is **fixed hardware** ([`HTR10_CORE_CAVITY_CM`], Terry 2005
/// Fig. 2: `z = 130.0` to `351.818`). It does not grow when fuel is added — it
/// is the *void above the bed* that shrinks. So the void is simply whatever
/// the bed does not occupy.
///
/// ## What was here before, and why it is gone
///
/// ~~The model applied a constant 98.758 cm of void at every loading, behind
/// an `OUTRAM_HTR10_FIXED_CAVITY=1` opt-in, "so no committed result moves
/// silently".~~ **REMOVED 2026-09-24 (maintainer direction).** That constant
/// is the void at **one** loading — the benchmark's 123.06 cm, where
/// `221.818 - 123.06 = 98.758` — and applying it everywhere silently grows the
/// whole cavity with the bed.
///
/// It was wrong in a known direction away from that point: at lower loading
/// the model had too little void, so reflector graphite sat where the reactor
/// has helium and `k` read HIGH; at higher loading it had too much void and
/// `k` read LOW. Measured 2026-09-18 across four loadings (dk vs RMC,
/// height-matched): `-54` at 102.9 cm, `-1259` at 122.5 cm, `-2194` at
/// 147.0 cm, `-1640` at 171.5 cm — exactly that signature. **The -54 pcm
/// agreement at 102.9 cm was two errors cancelling, not correctness.**
///
/// Keeping the correct treatment behind an opt-in meant nobody passed it and a
/// loading sweep could be run incoherently with nothing to catch it (gh:#292);
/// it also violated the workspace rule that correct physics is the default and
/// an ablation must be the explicit act. Both the flag and the constant are
/// now deleted, so every caller gets the fixed cavity with no way to opt out.
///
/// **This changes results measured under the old default.** Any recorded
/// number that did not set `OUTRAM_HTR10_FIXED_CAVITY=1` was computed with the
/// constant void and must be re-measured before it is quoted again; the shift
/// is ~zero at the benchmark loading and grows with distance from it.
#[must_use]
pub fn cavity_above_bed(bed_full_height_cm: f64) -> f64 {
    (HTR10_CORE_CAVITY_CM - bed_full_height_cm).max(0.0)
}

/// Axial reflector thickness \[cm\] beyond the core cavity / bed.
///
/// The full benchmark model is 610 cm tall (Terry 2005, corroborated against
/// Table 2), with the core cavity ending 130 cm below the model top — so there
/// is ~130 cm of graphite above the cavity, not the ~1 cm an unextended
/// `bed_half_height + 100` leaves once the cavity is carved out of it.
pub const HTR10_AXIAL_REFLECTOR_CM: f64 = 130.0;

/// Bottom reflector thickness \[cm\] below the conus floor.
///
/// Terry (2005) Fig. 2: the conus ends at `z = 388.764` and the model at
/// `z = 610.0`, so **221.236 cm, fixed** -- it does not depend on the loading.
///
/// ## What was here before, and why it is gone
///
/// ~~The outer box was symmetric: its bottom plane was the mirror of the top,
/// `-(bed_half_height + cavity + 130)`.~~ **REMOVED 2026-09-25.** Because the
/// origin is the bed's MID-HEIGHT, the mirrored bottom rode up with the bed, so
/// the bottom reflector was `314.872 - bed_height` cm: 216.9 cm at the lowest
/// loading (97.98 cm), 192.4 cm at the benchmark (122.47 cm), **114.0 cm** at
/// the tallest (200.86 cm) -- up to 107 cm thinner than the reactor's, by an
/// amount that grew with the bed, and a model 581 cm tall rather than 610. Same
/// failure class as the constant-cavity void: a loading-dependent geometry
/// error hidden by a construction that is exact at one point. Every result
/// computed before this change used the mirrored bottom.
pub const HTR10_BOTTOM_REFLECTOR_CM: f64 = 610.0 - 388.764;

/// Full axial extent \[cm\] of the benchmark model, Terry (2005) Fig. 2
/// (`z = 0` to `610`): top reflector + core cavity + conus + bottom reflector.
pub const HTR10_MODEL_HEIGHT_CM: f64 = 610.0;

/// Height \[cm\] of the **conus** — the sloping bottom of the pebble bed,
/// tapering from the core radius to the discharge tube.
///
/// Terry (2005) Fig. 2: z = 351.818 (conus top, "zero core height") to
/// z = 388.764, corroborated against TECDOC-1382 Table 2's stated 36.946.
/// **It is full of pebbles**, so omitting it omits fuel.
pub const HTR10_CONUS_HEIGHT_CM: f64 = 36.946;

/// Fuel-discharge-tube radius \[cm\] — the conus's lower radius.
pub const HTR10_DISCHARGE_TUBE_RADIUS_CM: f64 = 25.0;

/// Outer radius \[cm\] of the boronated carbon bricks = reflector outer
/// boundary (380 cm diameter / 2).
pub const HTR10_REFLECTOR_OUTER_CM: f64 = 190.0;

/// Inner radius \[cm\] of the side-reflector band carrying the control-rod
/// borings — Terry (2005) Fig. 2, corroborated as channel r 102.1 − 13/2.
pub const HTR10_CONTROL_ROD_INNER_CM: f64 = 95.6;

/// Outer radius \[cm\] of that band (102.1 + 13/2).
pub const HTR10_CONTROL_ROD_OUTER_CM: f64 = 108.6;

/// Carbon atom density \[atoms/b·cm\] of the **bored** side-reflector band.
///
/// IAEA-TECDOC-1382 Table 4-3 zones 31–40 — ten consecutive zones sharing one
/// reduced density, which is what a homogenised boring region looks like.
/// Against zone 22's 8.82418e-2 this is **28.1 % less carbon**.
///
/// Modelling that band as solid zone-22 graphite (as this did) over-reflects
/// and over-moderates in the reflector band *nearest the core*, which is the
/// highest-leverage place in the whole reflector to get wrong.
pub const HTR10_BORED_CARBON: f64 = 0.634459E-01;

/// Natural-boron atom density \[atoms/b·cm\] of the same zones 31–40.
pub const HTR10_BORED_BORON: f64 = 0.340640E-06;

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
    /// Boronated carbon brick — the outermost reflector annulus.
    pub const BORONATED: usize = 8;
    /// Side-reflector graphite homogenised with its control-rod borings
    /// (TECDOC zones 31–40): 28 % less carbon than solid zone-22 graphite.
    ///
    /// **No longer placed by [`super::assemble_explicit_triso`] (2026-09-25).**
    /// The borings are explicit geometry there (`super::super::reflector_geometry`),
    /// and the smeared band and its `OUTRAM_HTR10_BORINGS` knob are gone. The
    /// slot is kept so every index after it stays put.
    pub const BORED_GRAPHITE: usize = 9;
    /// **Homogenised dummy pebbles** — pebble graphite at the bed's filling
    /// fraction.
    ///
    /// ~~What the discharge tube actually contains.~~ **CORRECTED 2026-09-25:**
    /// the tube contains whole graphite balls, and since then
    /// [`super::assemble_explicit_triso`] places them explicitly (Li, Yu & Wei
    /// 2014: the cone region and discharge tube are formed by graphite balls
    /// in hexagonal geometry, and balls intersecting the cone or tube surface
    /// are rejected). This smear is only the `OUTRAM_HTR10_HOMOG_TUBE`
    /// ablation now.
    pub const HOMOG_DUMMY: usize = 10;
    /// Homogenised fuel zone, used only by [`super::assemble`].
    pub const FUEL: usize = KERNEL;

    /// First slot of the IAEA-TECDOC-1382 Table 4-3 zone materials that keep a
    /// composition of their own in the Monte Carlo model.
    pub const ZONE_TABLE_FIRST: usize = 11;

    /// Table 4-3 zones that keep a composition of their own once the borings
    /// are explicit, in slot order from [`ZONE_TABLE_FIRST`], with the factor
    /// TECDOC p. 242 applies to each (1.0 = the table value unchanged).
    ///
    /// Which zones, and the factors, are p. 242's: zones 29 and 42 are
    /// multiplied by 1.29978 and zone 60 by 1.16051, which puts back exactly
    /// the boring void those zones had homogenised (see
    /// `kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`,
    /// § 3). Zone 18 is the plain carbon brick; zones 51 and 68 share zone 24's
    /// row of the table and use its slot.
    ///
    /// Zones 0-4, 8-16, 19-21, 48 and 57 are **homogenised by the source**:
    /// it gives no geometry for what is inside them (the cold helium chamber,
    /// the hot-gas borings under the conus, the bottom structures). They are
    /// explicit regions at their Fig. 4.10 positions carrying the source's
    /// composition. That is an open item, not a modelling choice made here.
    pub const TABLE_4_3_ZONES: [(usize, f64); 24] = [
        (0, 1.0),
        (1, 1.0),
        (2, 1.0),
        (3, 1.0),
        (4, 1.0),
        (8, 1.0),
        (9, 1.0),
        (10, 1.0),
        (11, 1.0),
        (12, 1.0),
        (13, 1.0),
        (14, 1.0),
        (15, 1.0),
        (16, 1.0),
        (18, 1.0),
        (19, 1.0),
        (20, 1.0),
        (21, 1.0),
        (24, 1.0),
        (29, 1.29978),
        (42, 1.29978),
        (48, 1.0),
        (57, 1.0),
        (60, 1.16051),
    ];
    /// B4C of the control-rod absorber rings (TECDOC § 4.1.2: 1.7 g/cm³,
    /// natural boron).
    pub const ROD_B4C: usize = ZONE_TABLE_FIRST + TABLE_4_3_ZONES.len();
    /// Stainless steel of the control-rod sleeves (TECDOC § 4.1.2: 7.9 g/cm³,
    /// Cr 18 / Fe 68.1 / Ni 10 / Si 1 / Mn 2 / C 0.1 / Ti 0.8 wt%).
    pub const ROD_STEEL: usize = ROD_B4C + 1;
    /// Iron of the control-rod joints and ends (TECDOC § 4.1.2: Fe only,
    /// 0.04 atoms/(b cm), for 27.5 mm < R < 55 mm).
    pub const ROD_IRON: usize = ROD_STEEL + 1;
    /// Number of material slots.
    pub const COUNT: usize = ROD_IRON + 1;

    /// Slot of a Table 4-3 zone listed in [`TABLE_4_3_ZONES`].
    #[must_use]
    pub fn table_zone_slot(zone: usize) -> Option<usize> {
        TABLE_4_3_ZONES
            .iter()
            .position(|&(z, _)| z == zone)
            .map(|i| ZONE_TABLE_FIRST + i)
    }

    /// **Material slot of a Fig. 4.10 zone in the Monte Carlo model**, with
    /// IAEA-TECDOC-1382 p. 242's corrections for explicit borings applied.
    ///
    /// - zones 23, 25-26, 28, 30-41, 43-45, 49-50, 52-54, 58-59, 61-63, 66-67,
    ///   69-71, 80, 82 take zone 22's density: [`REFLECTOR`];
    /// - zones 27, 46, 55, 64, 72, 74-79 take zone 17's: [`BORONATED`];
    /// - zones 47, 56, 65, 73 take zone 18's (carbon brick);
    /// - zones 29, 42 and 60 are scaled, and every other zone keeps its own
    ///   Table 4-3 value: see [`TABLE_4_3_ZONES`].
    ///
    /// # Panics
    ///
    /// For zone 5 (the void cavity) and zones 6, 7 and 81 (the discharge tube,
    /// which holds explicit graphite balls), none of which is a material zone
    /// in this model, and for zones that do not exist.
    #[must_use]
    pub fn for_zone_mc(zone: usize) -> usize {
        match zone {
            22 | 23 | 25 | 26 | 28 | 30..=41 | 43..=45 | 49 | 50 | 52..=54 | 58 | 59
            | 61..=63 | 66 | 67 | 69..=71 | 80 | 82 => REFLECTOR,
            17 | 27 | 46 | 55 | 64 | 72 | 74..=79 => BORONATED,
            47 | 56 | 65 | 73 => table_zone_slot(18).expect("zone 18 is listed"),
            51 | 68 => table_zone_slot(24).expect("zone 24 is listed"),
            z => table_zone_slot(z)
                .unwrap_or_else(|| panic!("zone {z} is not a material zone of the MC model")),
        }
    }

    /// **ABLATION** (`OUTRAM_HTR10_NO_ZONE_MAP`): the reflector this model had
    /// before the zone map, i.e. zone-22 graphite everywhere except the
    /// boronated bricks at r > 167.793 cm (zones 75-79).
    #[must_use]
    pub fn for_zone_uniform(zone: usize) -> usize {
        if (75..=79).contains(&zone) {
            BORONATED
        } else {
            REFLECTOR
        }
    }
}

/// A built core and the sizes that describe it.
pub struct AssembledCore {
    /// The geometry.
    pub geometry: Geometry,
    /// Hex tiles in the bed lattice (every axial level, conus included).
    pub tiles: usize,
    /// Cells in the geometry.
    pub cells: usize,
    /// Universes in the geometry.
    pub universes: usize,
    /// Bed cylinder radius \[cm\].
    pub bed_radius: f64,
    /// Bed half-height \[cm\].
    pub bed_half_height: f64,
    /// Hex pitch \[cm\] of the bed lattice. [`assemble_explicit_triso`]: the
    /// paper's two-ball prism, 6.6106 cm ([`HexBedCell::from_paper`]).
    /// [`assemble`]: ~~solved from the fuel-zone target~~ still solved so its
    /// axially clipped one-ball tile realises the paper's fuel-zone fraction,
    /// 6.6086 cm (gh:#308: that path keeps the one-ball construction).
    pub lat_pitch: f64,
    /// Axial tile height \[cm\]. [`assemble_explicit_triso`]: 9.798 cm, one A-B
    /// layer pair holding two balls. [`assemble`]: 4.899 cm, one ball. In both
    /// the bed is `n_axial x 4.899` cm tall, i.e. `2 * bed_half_height`, NOT
    /// `n_axial * lat_height`.
    pub lat_height: f64,
    /// Bottom of the conus \[cm\] — the deepest fuelled z. Equal to
    /// `-bed_half_height` when no conus is modelled.
    pub conus_floor: f64,
    /// Top of the empty core cavity \[cm\], i.e. where the axial reflector
    /// begins. Equals `bed_half_height` when no reflector is built.
    pub cavity_top: f64,
    /// Top of the whole assembled model \[cm\]: cavity top + the 130 cm axial
    /// reflector. Equals `bed_half_height` when no reflector is built.
    pub refl_top: f64,
    /// Bottom of the whole assembled model \[cm\] (negative): conus floor less
    /// the fixed [`HTR10_BOTTOM_REFLECTOR_CM`]. Equals `-bed_half_height` when
    /// no reflector is built.
    ///
    /// The model is **not** symmetric about `z = 0` (the bed mid-height): see
    /// [`HTR10_BOTTOM_REFLECTOR_CM`] for why the old mirrored bottom was wrong.
    /// With a reflector, `refl_top - refl_bottom` is [`HTR10_MODEL_HEIGHT_CM`]
    /// at every loading.
    pub refl_bottom: f64,
}

/// **Assemble a delta-tracked pebble bed inside a surface-tracked reflector.**
///
/// # NOT a benchmark model, and not only because the fuel is homogenised
///
/// **This is a COST INSTRUMENT.** Beyond the homogenised fuel zone its
/// reflector is a single zone-22 graphite cell filling everything inside
/// r = 190 cm that is not the bed, so it is missing the **empty core cavity**
/// (solid graphite sits there), the **boronated carbon bricks**, the **cold
/// coolant annulus**, the **conus**, the **discharge tube** and the **bored
/// control-rod band**. Materials 8-10 of [`super::materials::htr10_material_set`]
/// are never referenced. Against [`assemble_explicit_triso`] that is roughly
/// **+15 500 pcm** in terms the V&V record has already priced individually.
/// Use [`assemble_explicit_triso`] for anything that reports `k` or feeds
/// group constants to another solver. gh:#308.
///
/// ~~The bed cell's pitch and layer height come from [`HexBedCell::from_paper`],
/// so the geometry is the paper's even when the size is scaled down.~~
/// **CORRECTED 2026-09-25:** only the layer height does, halved (one ball per
/// 4.899 cm tile); the pitch is SOLVED (6.6086 cm) so the clipped one-ball
/// tile realises the paper's fuel-zone fraction, and the pebbles
/// interpenetrate (gh:#309/#310, fixed in [`assemble_explicit_triso`] only).
///
/// # Parameters
/// - `n_rings` — a FLOOR on the lattice ring count. The bed radius is fixed at
///   the physical [`HTR10_CORE_RADIUS_CM`] and the lattice is sized to tile it
///   completely, so this cannot shrink the core; it can only over-tile.
/// - `n_axial` — axial layers, each [`HexBedCell::height`]/2 tall.
/// - `majorant_index` — which entry of the caller's majorant table the bed uses.
pub fn assemble(n_rings: usize, n_axial: usize, majorant_index: usize) -> AssembledCore {
    let cell = HexBedCell::from_paper();
    let r_pebble = cell.ball_diameter * 0.5;
    let r_fuel_zone = 2.5;
    // ~~One ball per tile -- see assemble_explicit_triso for why the lattice
    // pitch is the ball diameter and not the paper cell's 6.6106/9.798.~~
    // **CORRECTED 2026-09-25:** `assemble_explicit_triso` no longer uses one
    // ball per tile -- it builds the paper's two-ball prism (gh:#309 step 2,
    // gh:#310). THIS function still does, with every defect described below
    // (interpenetrating pebbles, 4.76 % of core carbon missing); it is the
    // homogenised cost instrument of gh:#308 and is not used for k.
    // ONE ball per tile, in a tile that is HALF the paper's two-ball prism.
    //
    // **WHAT THE CLIP ACTUALLY IS (corrected 2026-09-25, gh:#310).** The paper's
    // cell is a TWO-ball prism with A-B stacking: each layer sits in the hollow
    // of the one below, offset laterally by pitch/sqrt(3) = 3.8166 cm, so the
    // interlayer centre distance is hypot(3.8166, 2.4495) = 6.2102 cm -- clear
    // of the 6.0 cm diameter. One ball per tile DROPS that offset: every ball in
    // a column sits at the same (x, y), so the axial neighbour distance is just
    // the tile height, **4.8990 cm, i.e. 1.101 cm LESS than a diameter. The
    // pebbles interpenetrate.**
    //
    // Two spheres at 4.8990 cm centres cross on a circle of radius
    // sqrt(3.0^2 - 2.4495^2) = 1.7321 cm, and the tile boundary sits exactly on
    // that mid-plane -- so the cut is the CORRECT union of the two spheres, and
    // each 2.6816 cm^3 "cap" removed IS the interpenetration lens. Nothing falls
    // into a void. What is wrong is upstream: pebbles at 4.899 cm centres cannot
    // occupy 0.61 of the volume, because 4.74 % of each is inside its neighbour.
    //
    // `bed.rs`'s `the_reconstructed_cell_is_a_real_packing` would catch this and
    // does not: it asserts `is_non_overlapping()` on `HexBedCell::from_paper()`,
    // which passes. `HexBedCell::interlayer_spacing` assumes the A-B offset, so
    // no method on it returns the columnar 4.8990 -- the type models the paper
    // while this builds something else.
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
    // Volume of a sphere of radius `r` after the tile clips it at +/- height/2.
    let clipped = |r: f64| {
        let cap = r - 0.5 * lat_height;
        let full = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
        if cap <= 0.0 {
            full
        } else {
            full - 2.0 * std::f64::consts::PI * cap * cap * (3.0 * r - cap) / 3.0
        }
    };
    // TARGET THE FUEL-ZONE VOLUME FRACTION, NOT THE BALL PACKING.
    //
    // A 6.0 cm ball cannot sit whole in a 4.899 cm tile, so one-ball-per-tile
    // MUST clip -- and the clip is wildly uneven: it removes 4.74 % of the ball
    // but only 0.06 % of the fuel zone, because the caps come off the outer
    // graphite shell. Solving the pitch so the CLIPPED BALL realises 0.61
    // therefore over-fuels the bed by +4.91 %, which is spurious reactivity.
    // (That is exactly what an earlier version of this code did.)
    //
    // What the paper's 0.61 actually pins down is fuel per unit volume:
    // 0.61 * V_fuelzone / V_ball = 0.35301. Solving for THAT gives pitch
    // 6.6086 cm -- within 0.03 % of the paper's own 6.6106 cm, which is the
    // check that this is the right target rather than a second arbitrary one.
    //
    // Cost, stated rather than hidden: the shell graphite is then clipped
    // without compensation.
    //
    // ~~the bed carries ~4.7 % less pebble-shell moderator than a whole-ball
    // bed would~~ **CORRECTED 2026-09-25 -- 4.7 % is the deficit in BALL
    // volume, not in shell volume, and the caps come off the shell almost
    // entirely.** Recomputed from this function's own numbers: the two caps
    // remove 5.363 cm^3, of which only 0.080 cm^3 is fuel zone, so
    // **11.2 % of the pebble-shell graphite is removed**, and the shell's
    // share of core volume falls from the paper's 0.25699 to 0.22842 --
    // a **11.1 %** deficit.
    //
    // **And that is only the FUEL pebble.** 43 % of the tiles are solid
    // graphite dummy pebbles with no shell and no fuel zone, and the lattice
    // clips them identically -- the whole 5.363 cm^3 is graphite there. The
    // core-level number, which is the one to quote, is: every pebble loses
    // 4.74 % of its volume, essentially all of it carbon in both types, while
    // the fuel zone loses 0.061 %. Core graphite volume fraction goes
    // **0.59988 -> 0.57131, i.e. -4.76 %**, helium 39.0 % -> 41.9 %, and the
    // heavy metal stays exact to +0.060 % -- so **C/U is 4.8 % low**. Sign on
    // k is NOT predictable a priori (an over-moderated core loses parasitic
    // capture as well as moderation) and has not been measured -- gh:#309.
    let target_fuel_zone_fraction = PAPER_FILLING_FRACTION * (r_fuel_zone / r_ball).powi(3);
    let lat_pitch = (clipped(r_fuel_zone)
        / (target_fuel_zone_fraction * (3.0_f64.sqrt() / 2.0) * lat_height))
        .sqrt();

    // Bed envelope: a cylinder just containing the tiles, then the reflector out
    // to the published 1 m thickness.
    // The bed cylinder must be INSCRIBED in the hexagon the lattice actually
    // tiles, not circumscribed about it.
    //
    // `n_rings` rings of tiles cover a hexagon of INRADIUS (n_rings - 0.5)*pitch.
    // This was `(n_rings + 0.5)*pitch`, which puts the cylinder OUTSIDE the
    // tiled region: at 14 rings the cylinder was 93.55 cm against a tiled
    // inradius of 87.10 cm, so 28.3 % of the bed area fell outside every tile
    // and picked up the lattice's `outer` universe -- the DUMMY graphite
    // pebble. The realised fuel fraction was therefore 0.409 against the
    // intended 0.570, a 28 % fuel deficit, and it was silent: no history is
    // lost, no distance is negative, the geometry simply contains less fuel
    // than the model says it does.
    //
    // `n_rings` is now derived from the physical core radius rather than the
    // radius from `n_rings`, so the cylinder is fully tiled by construction.
    // The caller's `n_rings` is now a FLOOR, not the size: the radius is
    // physical and the lattice is sized to tile it. A larger request simply
    // over-tiles; rings beyond the cylinder lie outside the bed cell and are
    // inert, so the knob stays useful for scaling tests without being able to
    // silently shrink the core.
    let bed_radius = HTR10_CORE_RADIUS_CM;
    // Ring count needed to tile the bed cylinder.
    //
    // A Y-oriented hex lattice steps (sqrt(3)/2)*pitch in x (see
    // `HexLattice::center_offset`), so ring `n_rings-1` reaches only
    // `(n_rings-1) * (sqrt(3)/2) * pitch` along that axis -- NOT
    // `(n_rings-1) * pitch`. Using the pitch directly overstates the tiled
    // radius by 15 %: at 15 rings it claimed 95.8 cm where the lattice
    // actually reached ~80 cm, leaving the outer bed untiled and filled with
    // dummy pebbles. Measured, not derived -- `examples/htr10_fuel_fraction.rs`
    // reports the untiled fraction against radius.
    let ring_reach = (3.0_f64.sqrt() / 2.0) * lat_pitch;
    let n_rings = n_rings.max((bed_radius / ring_reach).ceil() as usize + 1);
    let bed_half_height = 0.5 * lat_height * n_axial as f64;
    // OUTRAM_HTR10_NOREFL=1 collapses the reflector to zero thickness.
    //
    // ~~With OUTRAM_HTR10_REFLECTIVE=1 and OUTRAM_HTR10_ALLFUEL=1 that makes
    // the model an INFINITE MEDIUM of fuel pebbles.~~
    // **CORRECTED 2026-09-19 -- that was NOT true in this function.** It is
    // true of `assemble_explicit_triso`, which reads OUTRAM_HTR10_REFLECTIVE.
    // `assemble` hardcoded `BoundaryType::Vacuum` on all three outer surfaces
    // and ignored the variable, so NOREFL here gave a BARE BED VENTING TO
    // VACUUM -- maximum leakage, not an infinite medium.
    //
    // Measured 2026-09-19 while building `examples/htr10_deterministic_vs_mc`:
    // that configuration returned `k = 0.2726`, against a bed `k_inf` of order
    // 1.3. The comment had been copied from the explicit-TRISO path and was
    // never true here. The variable is now honoured below, so the claim holds
    // for both functions.
    let refl_thickness = if std::env::var("OUTRAM_HTR10_NOREFL").is_ok() {
        0.0
    } else {
        100.0
    };
    let outer_bc = if std::env::var("OUTRAM_HTR10_REFLECTIVE").is_ok() {
        BoundaryType::Reflective
    } else {
        BoundaryType::Vacuum
    };
    // WHERE the outer boundary lives depends on whether a reflector exists.
    //
    // With a reflector, surfaces 5/6/7 (refl_radius, refl_bottom, refl_top) are
    // the edge of the model and carry `outer_bc`, while the bed surfaces 2/3/4
    // are interior and must stay TRANSMISSIVE so neutrons pass into the
    // reflector.
    //
    // With `OUTRAM_HTR10_NOREFL=1` the reflector collapses -- `refl_radius`
    // becomes `bed_radius` and `refl_bottom`/`refl_top` become `-/+bed_half_height` --
    // so 5/6/7 land exactly on 2/3/4 and the shell between them has ZERO
    // VOLUME. A boundary condition on a zero-volume shell does nothing:
    // neutrons cross the transmissive bed surfaces and are simply lost. That is
    // why `NOREFL=1 REFLECTIVE=1` returned k = 0.2726 rather than an infinite
    // medium -- measured 2026-09-19, identical to six digits with and without
    // `REFLECTIVE`, which is what exposed it.
    //
    // So when there is no reflector, the BED surfaces are the edge of the model
    // and must carry `outer_bc` themselves.
    let bed_bc = if refl_thickness > 0.0 {
        BoundaryType::Transmissive
    } else {
        outer_bc
    };
    // ~~Radial reflector structure is PHYSICAL, from Terry (2005) Fig. 2 ...
    // BORONATED CARBON BRICKS to the 190 cm outer boundary.~~
    // **CORRECTED 2026-09-25 -- that comment was pasted from
    // `assemble_explicit_triso` and is FALSE here.** This function builds ONE
    // reflector cell of `mat::REFLECTOR` (zone-22 graphite) filling everything
    // inside r = 190 cm that is not the bed. It has no boronated brick, no
    // coolant annulus, no bored band, no conus, no discharge tube -- and no
    // core cavity either: the region above the bed is SOLID GRAPHITE. The
    // giveaway was `_graphite_outer`, computed and then discarded, which is
    // now deleted. Against `assemble_explicit_triso` that is roughly
    // **+15 500 pcm** of already-measured error (the V&V record prices the
    // empty cavity at -14 108 pcm, the bricks at -1 260 and the annulus at
    // -122). See gh:#308 -- do not use this function for a k comparison.
    let refl_radius = if refl_thickness > 0.0 {
        HTR10_REFLECTOR_OUTER_CM
    } else {
        bed_radius
    };
    // The outer extent reserves room for cavity + axial reflector so the model's
    // HEIGHT matches `assemble_explicit_triso`'s. Only the extent matches: the
    // contents of that room are graphite here, not helium.
    let refl_top = if refl_thickness > 0.0 {
        bed_half_height + cavity_above_bed(2.0 * bed_half_height) + HTR10_AXIAL_REFLECTOR_CM
    } else {
        bed_half_height
    };
    // This homogenised model has no conus (reflector graphite fills it), but
    // the outer extent is still the reactor's: conus depth + fixed bottom
    // reflector below the bed, NOT a mirror of the top.
    let refl_bottom = if refl_thickness > 0.0 {
        -bed_half_height - HTR10_CONUS_HEIGHT_CM - HTR10_BOTTOM_REFLECTOR_CM
    } else {
        -bed_half_height
    };

    let surfaces = vec![
        // 0,1: pebble and its fuel zone, in TILE-LOCAL coordinates
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: r_pebble,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: r_fuel_zone,
            bc: BoundaryType::Transmissive,
        }),
        // 2..4: the bed envelope
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: bed_radius,
            bc: bed_bc,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: -bed_half_height,
            bc: bed_bc,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: bed_half_height,
            bc: bed_bc,
        }),
        // 5..7: the reflector outer boundary -- VACUUM, this is a bare core
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: refl_radius,
            bc: outer_bc,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: refl_bottom,
            bc: outer_bc,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: refl_top,
            bc: outer_bc,
        }),
    ];
    let ins = |i: usize| RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Inside,
    };
    let out = |i: usize| RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Outside,
    };

    // Universe 1 -- fuelled pebble. Universe 2 -- dummy pebble. Both sit in
    // tile-local coordinates, so the spheres are shared.
    let fuel_zone = Cell::material(10, vec![ins(1)], mat::FUEL, 293.6);
    let fuel_shell = Cell::material(
        11,
        vec![out(1), ins(0), RegionToken::Intersection],
        mat::GRAPHITE,
        293.6,
    );
    let fuel_helium = Cell::material(12, vec![out(0)], mat::HELIUM, 293.6);
    let dummy_ball = Cell::material(13, vec![ins(0)], mat::GRAPHITE, 293.6);
    let dummy_helium = Cell::material(14, vec![out(0)], mat::HELIUM, 293.6);

    // The bed: a cylinder, DELTA-tracked, filled by the hex lattice.
    let bed_region = vec![
        ins(2),
        out(3),
        RegionToken::Intersection,
        ins(4),
        RegionToken::Intersection,
    ];
    let bed_cell = Cell::fill(1, bed_region.clone(), CellFill::Lattice(0), Position::ZERO);
    let bed = if majorant_index == usize::MAX {
        bed_cell
    } else {
        bed_cell.delta_tracked(majorant_index)
    };

    // The reflector: everything else inside the vacuum boundary, SURFACE-tracked.
    let mut refl_region = vec![
        ins(5),
        out(6),
        RegionToken::Intersection,
        ins(7),
        RegionToken::Intersection,
    ];
    refl_region.extend(bed_region);
    refl_region.push(RegionToken::Complement);
    refl_region.push(RegionToken::Intersection);
    let reflector = Cell::material(2, refl_region, mat::REFLECTOR, 293.6);

    // OUTRAM_HTR10_ALLFUEL=1 makes EVERY tile a fuelled pebble. Not physical --
    // the first critical core is 57 % fuel / 43 % graphite dummies -- but it is
    // an ABLATION that bounds how much of a k deficit the dilution can explain.
    let mod_universe = if std::env::var("OUTRAM_HTR10_ALLFUEL").is_ok() {
        1
    } else {
        2
    };
    let levels = bed_tile_levels(n_rings, n_axial, 1, mod_universe);
    let tiles: usize = levels.iter().flatten().map(|r| r.len()).sum();
    let lattice = HexLattice::from_rings_3d(
        0,
        HexOrientation::Y,
        // The lattice CENTRE, not its bottom tile.
        //
        // `HexLattice::center_offset` already centres the axial stack about
        // this point -- tile `i` sits at `center.z - (n_axial/2 - i - 0.5)*h`
        // -- so passing a bottom-referenced z shifts the WHOLE stack down by
        // `bed_half_height - h/2`. At 25 layers that put the lattice in
        // z = [-120.03, +2.45] against a bed cell of [-61.24, +61.24]: they
        // overlapped over only 52 % of the bed, and the other 48 % silently
        // took the lattice's `outer` universe, i.e. DUMMY GRAPHITE PEBBLES.
        //
        // Measured with `examples/htr10_fuel_fraction.rs` (which leaves the
        // untiled region empty so it can be counted): the untiled fraction was
        // a flat 0.48 at EVERY radius including r = 0, which is what
        // distinguishes an axial offset from a radial coverage shortfall.
        Position::ZERO,
        lat_pitch,
        lat_height,
        &levels,
        // OUTRAM_HTR10_NO_OUTER=1 leaves the region outside the tiled hexagon
        // EMPTY instead of filling it with dummy pebbles. Not physical -- it is
        // a measurement: with no filler, `locate` reports those points as lost,
        // so the lost fraction IS the fraction of the bed cylinder the lattice
        // fails to tile. Formulas for that have been wrong twice here.
        if std::env::var("OUTRAM_HTR10_NO_OUTER").is_ok() {
            None
        } else {
            Some(2)
        },
    );

    let geometry = Geometry {
        surfaces,
        cells: vec![
            bed,
            reflector,
            fuel_zone,
            fuel_shell,
            fuel_helium,
            dummy_ball,
            dummy_helium,
        ],
        universes: vec![
            Universe {
                id: 0,
                cell_indices: vec![0, 1],
            },
            Universe {
                id: 1,
                cell_indices: vec![2, 3, 4],
            },
            Universe {
                id: 2,
                cell_indices: vec![5, 6],
            },
        ],
        lattices: vec![Lattice::Hex(lattice)],
        root_universe: 0,
    };
    let (cells, universes) = (geometry.cells.len(), geometry.universes.len());
    AssembledCore {
        geometry,
        tiles,
        cells,
        universes,
        bed_radius,
        bed_half_height,
        lat_pitch,
        lat_height,
        // NOT features of this geometry. `conus_floor` is the bed bottom
        // because there is no conus, and `cavity_top` is an arithmetic z at
        // which NOTHING changes -- the graphite runs straight through it. Both
        // are reported only so a caller can size a source box or entropy mesh
        // the same way for either function. gh:#308.
        conus_floor: -bed_half_height,
        cavity_top: if refl_thickness > 0.0 {
            bed_half_height + cavity_above_bed(2.0 * bed_half_height)
        } else {
            bed_half_height
        },
        refl_top,
        refl_bottom,
    }
}

/// **Assemble the core with an EXPLICIT TRISO lattice in each fuelled pebble** —
/// the double-heterogeneous model the benchmark actually specifies.
///
/// ~~**Open defects in this construction:** gh:#309 (pebble-shell carbon
/// clipped), gh:#310 (pebbles in axial contact).~~ **FIXED 2026-09-25 — the
/// bed is now the paper's two-ball prism cell** (see "The bed" below): no ball
/// is clipped by its own tile, no two balls overlap, and every pebble is the
/// whole 6 cm sphere. ~~gh:#311 (no B-11)~~ **CORRECTED 2026-09-25:** #311 is
/// closed and B-11 is placed (`materials::htr10_material_set`, the `b11`
/// closure). Results from it are tentative — see the module docs,
/// "Verification status".
///
/// # The bed: two balls per tile (gh:#309 step 2, gh:#310)
///
/// The hex lattice tile IS the paper's prism, [`HexBedCell::from_paper`]:
/// pitch 6.6106 cm, height 9.798 cm = one A-B layer pair, two balls per tile,
/// packing 0.610. Each tile universe holds pieces of five balls
/// ([`super::bed::BallSite`]): two A balls on its axis at its top and bottom
/// faces (half each), and three B balls at alternate vertices at mid-height (a
/// third each). The spheres are centred OUTSIDE or ON the tile boundary; that
/// is exact here, because `Geometry::locate` evaluates a tile universe's cell
/// regions in tile-local coordinates only for points the lattice has already
/// placed in that tile, so each tile draws exactly its own piece and the
/// neighbours holding the rest of the same ball draw theirs.
///
/// Fuel/dummy is a property of the BALL ([`super::bed::TwoBallBed`]), and a
/// tile's universe is the variant for its five balls' identities (up to 2^5,
/// only those used are built), so a ball split across 2 or 3 tiles is the same
/// kind of pebble in all of them.
///
/// Nearest centre distances: in-plane 6.6106 cm, A to B
/// `hypot(pitch/sqrt(3), height/2)` = 6.2102 cm, A to A (axial) 9.798 cm, all
/// greater than the 6.0 cm diameter; the smallest gap is 0.210 cm.
/// `htr10_rmc::tests::no_two_balls_of_the_built_bed_overlap` checks it on the
/// built geometry.
///
/// # Parameters
/// - `n_rings` — a FLOOR on the lattice ring count (the bed radius is the
///   physical 90 cm and the lattice is sized to tile it).
/// - `n_axial` — the fuel LOADING HEIGHT in half-layers of 4.899 cm (one ball
///   layer each), so the bed is `n_axial x 4.899` cm: 20 / 25 / 41 give
///   97.980 / 122.474 / 200.858 cm, the same heights as before the two-ball
///   change. The A-B stacking is anchored at the bed floor (layer 0 is an A
///   layer), so an odd `n_axial` simply ends on an A layer and an even one on
///   a B layer; see [`super::bed::TwoBallBed`].
/// - `majorant_index` — which entry of the caller's majorant table the bed
///   uses; `usize::MAX` surface-tracks the bed.
///
/// Universes: 0 root, [`TRISO_PARTICLE_UNIVERSE`], [`TRISO_MATRIX_UNIVERSE`],
/// then one per bed-tile fuel mask in use (35 in all at 14 rings, all 32 masks
/// occur). Tile cell ids encode their role, see [`tile_cell_role`].
///
/// ~~Four coordinate levels~~ **CORRECTED 2026-09-25 — three coordinate
/// levels**: root → (bed hex lattice) → bed-tile universe (pieces of five
/// pebbles since 2026-09-25; one pebble before) → (TRISO rect lattice, entered
/// through the fuel-zone cell's translation to its ball centre) → TRISO
/// particle universe. A lattice selects the next level's universe but is not a
/// level itself. Verified by locating a kernel in the assembled 14 x 25 core:
/// `path.levels.len() == 3`, lattices `[None, Some(0), Some(1)]`
/// (`examples/htr10_geometry_images.rs` prints it; re-checked on the two-ball
/// cell 2026-09-25). Depth-3 descent was gated
/// in `outram-mc-libs` `tests/nested_lattice_depth3.rs`, which also counts
/// `levels.len()`; ~~this is depth 4~~ this is the **same** depth.
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

    // TWO BALLS PER TILE: the lattice tile IS the paper's prism (gh:#309 step
    // 2, gh:#310, 2026-09-25).
    //
    // ~~ONE BALL PER TILE, so the lattice pitch is NOT the paper's cell
    // pitch.~~ **REPLACED 2026-09-25.** Until then each tile held one 6 cm
    // ball in a tile HALF the paper's prism (height 4.899 cm), with the pitch
    // solved (6.6086 cm) so the clipped fuel zone realised 0.61 (2.5/3)^3.
    // That construction dropped the paper's A-B lateral offset, so every ball
    // in a column sat at the same (x, y), 4.899 cm from its axial neighbours:
    // **the pebbles interpenetrated by 1.101 cm**, the tile face cut each one
    // on the plane where the two spheres cross, and each sphere lost two
    // 2.6816 cm^3 lenses -- 4.74 % of every pebble, essentially all of it
    // carbon. Core graphite was 4.76 % low (0.59988 -> 0.57131), helium 39.0 %
    // -> 41.9 %, the heavy metal exact to +0.06 %, so C/U was 4.8 % low
    // (gh:#309); the fuel zones of axial neighbours met on a 1 cm disc
    // (gh:#310). A shrunk-pebble ablation that removed the overlap changed k
    // by -6.88 +/- 1.48 pcm/cm across 98-201 cm, equal and opposite to the
    // gh:#218 height drift.
    //
    // Now: pitch and height are the paper's cell, taken straight from
    // `HexBedCell::from_paper` -- pitch 6.6106 cm (the touching pitch diluted
    // to the stated 0.61), height 9.798 cm (two close-packed layers, the
    // paper's stated layer). Nothing is solved or clipped: two whole 6 cm
    // balls per tile give 0.610 exactly, the fuel zone 0.61 (2.5/3)^3 exactly,
    // and the shell and dummy graphite with them. The ball sites, the per-ball
    // identity and the axial phase are `super::bed::TwoBallBed`'s.
    let lat_pitch = cell.pitch;
    let lat_height = cell.height;

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
    const TRISO_OFFSET: [f64; 3] = [0.5, 0.5, 0.0];
    let (pitch_triso, n_particles) = cubic_pitch_for_count(r_part, r_fuel_zone, 8335, TRISO_OFFSET);
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

    // The bed cylinder must be INSCRIBED in the hexagon the lattice actually
    // tiles, not circumscribed about it.
    //
    // `n_rings` rings of tiles cover a hexagon of INRADIUS (n_rings - 0.5)*pitch.
    // This was `(n_rings + 0.5)*pitch`, which puts the cylinder OUTSIDE the
    // tiled region: at 14 rings the cylinder was 93.55 cm against a tiled
    // inradius of 87.10 cm, so 28.3 % of the bed area fell outside every tile
    // and picked up the lattice's `outer` universe -- the DUMMY graphite
    // pebble. The realised fuel fraction was therefore 0.409 against the
    // intended 0.570, a 28 % fuel deficit, and it was silent: no history is
    // lost, no distance is negative, the geometry simply contains less fuel
    // than the model says it does.
    //
    // `n_rings` is now derived from the physical core radius rather than the
    // radius from `n_rings`, so the cylinder is fully tiled by construction.
    // The caller's `n_rings` is now a FLOOR, not the size: the radius is
    // physical and the lattice is sized to tile it. A larger request simply
    // over-tiles; rings beyond the cylinder lie outside the bed cell and are
    // inert, so the knob stays useful for scaling tests without being able to
    // silently shrink the core.
    let bed_radius = HTR10_CORE_RADIUS_CM;
    // Ring count needed to tile the bed cylinder.
    //
    // A Y-oriented hex lattice steps (sqrt(3)/2)*pitch in x (see
    // `HexLattice::center_offset`), so ring `n_rings-1` reaches only
    // `(n_rings-1) * (sqrt(3)/2) * pitch` along that axis -- NOT
    // `(n_rings-1) * pitch`. Using the pitch directly overstates the tiled
    // radius by 15 %: at 15 rings it claimed 95.8 cm where the lattice
    // actually reached ~80 cm, leaving the outer bed untiled and filled with
    // dummy pebbles. Measured, not derived -- `examples/htr10_fuel_fraction.rs`
    // reports the untiled fraction against radius.
    let ring_reach = (3.0_f64.sqrt() / 2.0) * lat_pitch;
    let n_rings = n_rings.max((bed_radius / ring_reach).ceil() as usize + 1);
    // `n_axial` counts HALF-layers (one ball layer, 4.899 cm, each) -- the
    // loading step the old one-ball tile had -- so the loading heights, and
    // every comparison against RMC, are unchanged: n 20 / 25 / 41 are still
    // 97.980 / 122.474 / 200.858 cm. The tile is now a whole layer pair.
    let bed_half_height = 0.25 * lat_height * n_axial as f64;
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
    let site_sphere = |site: BallSite, r: f64| {
        let [x0, y0, z0] = cell.site_centre(site);
        SurfaceKind::Sphere(Sphere {
            x0,
            y0,
            z0,
            r,
            bc: BoundaryType::Transmissive,
        })
    };
    surfaces.push(site_sphere(BallSite::ABottom, r_fuel_zone));
    surfaces.push(site_sphere(BallSite::ABottom, r_pebble));
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
    // 14, 15: the COLD COOLANT FLOW annulus, 140.6 -> 148.6 cm. Modelling it as
    // solid graphite (as this did) overstates the reflector: it is a helium
    // flow path, i.e. effectively void, and leaving it solid suppresses
    // leakage that the real reactor has.
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
    // 22..29: (fuel zone, pebble) sphere pairs of the ball sites ATop, BEast,
    // BNorthWest, BSouthWest, in tile-local coordinates. ABottom's pair is 5/6.
    let mut site_surfaces = [(5usize, 6usize); 5];
    for (i, &site) in BallSite::ALL.iter().enumerate().skip(1) {
        site_surfaces[i] = (surfaces.len(), surfaces.len() + 1);
        surfaces.push(site_sphere(site, r_fuel_zone));
        surfaces.push(site_sphere(site, r_pebble));
    }

    let ins = |i: usize| RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Inside,
    };
    let out = |i: usize| RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Outside,
    };

    let shell = |inner: usize, outer: usize, m: usize, id: i32| {
        Cell::material(
            id,
            vec![out(inner), ins(outer), RegionToken::Intersection],
            m,
            293.6,
        )
    };

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
    let has_refl = refl_thickness > 0.0;
    let explicit_tube = has_refl && std::env::var("OUTRAM_HTR10_HOMOG_TUBE").is_err();
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
            // exactly as `ins(7)` clips it to r < 90 cm. With Li's rejection
            // (`DischargeTube`) no kept ball reaches the cone or the tube, so
            // there the cut only ever passes through helium.
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

    // FUEL / DUMMY IDENTITY IS PER BALL (gh:#309 step 2).
    //
    // `TwoBallBed` hands the 57:43 split out over the BALLS with volume in the
    // bed, from the floor up, by the same low-discrepancy rule the tiles used
    // to get; the conus (every ball centred below the bed floor) is all dummy,
    // Terry et al. (2005) section 2: *"the conus and discharge tube contained
    // only dummy pebbles"* (quoted in
    // `kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md:256`).
    // Filling the conus with fuel was once worth +4578 +/- 158 pcm.
    //
    // OUTRAM_HTR10_ALLFUEL=1 makes EVERY ball fuelled and
    // OUTRAM_HTR10_FUEL_CONUS=1 gives the conus the 57:43 split too. Neither is
    // physical; both are ablation arms, kept from the one-ball construction.
    let assignment = if std::env::var("OUTRAM_HTR10_ALLFUEL").is_ok() {
        FuelAssignment::AllFuel
    } else if std::env::var("OUTRAM_HTR10_FUEL_CONUS").is_ok() {
        FuelAssignment::FuelledConus
    } else {
        FuelAssignment::Paper
    };
    // AXIAL EXTENT: the lattice reaches below the conus floor -- to the model
    // bottom, through the discharge tube, when the tube holds explicit balls --
    // and above the bed top; `TwoBallBed` places its faces on the A layers
    // (anchored at the bed floor) and its centre accordingly. `n_axial` stays
    // the fuel LOADING HEIGHT. Levels outside the bed cell's region are never
    // reached.
    //
    // The lattice centre is NOT z = 0 any more, and that is deliberate, not
    // the old "bottom-referenced centre" defect (which put the lattice 58.79 cm
    // low and replaced 48 % of the bed with dummies): the centre passed here is
    // the true mid-height of the stack `TwoBallBed` laid out, and
    // `the_built_bed_matches_the_two_ball_description` checks every tile centre
    // of the built lattice against it.
    let bed = TwoBallBed::new_with_tube(
        cell,
        n_rings,
        n_axial,
        HTR10_CONUS_HEIGHT_CM,
        bed_radius,
        explicit_tube.then_some(DischargeTube {
            radius: HTR10_DISCHARGE_TUBE_RADIUS_CM,
            depth: HTR10_BOTTOM_REFLECTOR_CM,
        }),
        assignment,
    );
    debug_assert!((bed.bed_top - bed_half_height).abs() < 1e-9);

    // One universe per (fuel mask, presence mask) in use: bit i = ball site i
    // fuelled / present. A rejected ball (Li's rule, `DischargeTube`) is simply
    // absent from its tiles, and its space is helium. The all-dummy,
    // all-present variant is always built, as the lattice `outer`.
    let nr = n_rings as i32;
    const ALL_PRESENT: u8 = 0b1_1111;
    let mut keys_used: Vec<(u8, u8)> = vec![(0, ALL_PRESENT)];
    let mut tile_keys: Vec<(i32, i32, i32, (u8, u8))> = Vec::new();
    for level in 0..bed.n_levels as i32 {
        for a in -(nr - 1)..=(nr - 1) {
            for b in -(nr - 1)..=(nr - 1) {
                if hex_ring(a, b) > n_rings - 1 {
                    continue;
                }
                let key = (bed.tile_mask(a, b, level), bed.tile_present_mask(a, b, level));
                if !keys_used.contains(&key) {
                    keys_used.push(key);
                }
                tile_keys.push((a, b, level, key));
            }
        }
    }
    keys_used.sort_unstable();
    let mut universes = vec![
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
    let mut key_universe: std::collections::BTreeMap<(u8, u8), usize> =
        std::collections::BTreeMap::new();
    assert!(keys_used.len() < 1000, "tile cell ids hold at most 1000 variants");
    for (v, &(m, p)) in keys_used.iter().enumerate() {
        let u = universes.len();
        let base = 10 * v as i32;
        let mut idx = Vec::new();
        let mut helium_region: Option<Rgn> = None;
        for (i, &site) in BallSite::ALL.iter().enumerate() {
            if p & (1 << i) == 0 {
                continue; // rejected: no ball here, its space is helium
            }
            let (fz, pb) = site_surfaces[i];
            let [x, y, z] = cell.site_centre(site);
            let id = base + i as i32;
            if m & (1 << i) != 0 {
                // Fuel zone: the TRISO lattice, translated to the ball centre.
                idx.push(cells.len());
                cells.push(Cell::fill(
                    TILE_FUEL_ZONE_CELL_ID + id,
                    vec![ins(fz)],
                    CellFill::Lattice(1),
                    Position::new(x, y, z),
                ));
                idx.push(cells.len());
                cells.push(shell(fz, pb, mat::GRAPHITE, TILE_FUEL_SHELL_CELL_ID + id));
            } else {
                idx.push(cells.len());
                cells.push(Cell::material(
                    TILE_DUMMY_BALL_CELL_ID + id,
                    vec![ins(pb)],
                    mat::GRAPHITE,
                    293.6,
                ));
            }
            helium_region = Some(match helium_region {
                None => Rgn::out(pb),
                Some(r) => r.and(Rgn::out(pb)),
            });
        }
        match helium_region {
            Some(r) => {
                idx.push(cells.len());
                cells.push(Cell::material(TILE_HELIUM_CELL_ID + base, r.0, mat::HELIUM, 293.6));
            }
            None => {
                // Every ball of the tile rejected: all helium. Two cells,
                // because an empty region is FALSE (see the TRISO matrix).
                let (_, pb) = site_surfaces[0];
                for r in [Rgn::ins(pb), Rgn::out(pb)] {
                    idx.push(cells.len());
                    cells.push(Cell::material(TILE_HELIUM_CELL_ID + base, r.0, mat::HELIUM, 293.6));
                }
            }
        }
        key_universe.insert((m, p), u);
        universes.push(Universe {
            id: u as i32,
            cell_indices: idx,
        });
    }
    let outer_universe = key_universe[&(0, ALL_PRESENT)];

    // Placeholder levels in `from_rings_3d`'s ring layout (it validates the ring
    // sizes), then every tile's universe written by its (a, b, level) index, so
    // no ring/element ordering convention sits between a tile and its balls.
    let placeholder: Vec<Vec<Vec<usize>>> = (0..bed.n_levels)
        .map(|_| {
            (0..n_rings)
                .rev()
                .map(|r| vec![outer_universe; if r == 0 { 1 } else { 6 * r }])
                .collect()
        })
        .collect();
    let mut bed_lattice = HexLattice::from_rings_3d(
        0,
        HexOrientation::Y,
        Position::new(0.0, 0.0, bed.lattice_centre_z()),
        lat_pitch,
        lat_height,
        &placeholder,
        // See the OUTRAM_HTR10_NO_OUTER note in `assemble`.
        if std::env::var("OUTRAM_HTR10_NO_OUTER").is_ok() {
            None
        } else {
            Some(outer_universe)
        },
    );
    for &(a, b, level, key) in &tile_keys {
        let i = [a + nr - 1, b + nr - 1, level];
        debug_assert!(bed_lattice.are_valid_indices(i));
        let flat = bed_lattice.flat_index(i);
        bed_lattice.universes[flat] = key_universe[&key] as i32;
    }
    let tiles = tile_keys.len();

    // THE REFLECTOR, explicit: every boring at its own position in solid
    // graphite, inside TECDOC Fig. 4.10's zone map. See
    // `super::reflector_geometry` for the specification and what it leaves
    // open.
    if has_refl {
        let refl_root = build_reflector(
            &mut surfaces,
            &mut cells,
            &mut universes,
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
    }
}

/// Universe index of one TRISO particle (kernel, four coatings, matrix
/// beyond) in [`assemble_explicit_triso`]'s geometry — what the TRISO
/// `RectLattice` places where a whole particle fits.
pub const TRISO_PARTICLE_UNIVERSE: usize = 1;

/// Universe index of plain matrix graphite, the TRISO lattice's `outer` and
/// its non-particle tiles.
pub const TRISO_MATRIX_UNIVERSE: usize = 2;

/// Cell-id bases of the pebble cells inside a bed tile universe of
/// [`assemble_explicit_triso`]. A tile cell's id is `base + 10*v + site`
/// (`site` the index in [`BallSite::ALL`], `v` the ordinal of the tile's
/// universe variant -- its (fuel mask, presence mask) pair, see
/// `TwoBallBed::tile_present_mask`), or `base + 10*v` for the helium cell.
/// Read back with [`tile_cell_role`].
///
/// ~~`base + 10*mask`, bases 1000-4000~~ **CHANGED 2026-09-25**: a variant
/// is now a pair of 5-bit masks (up to 1024 combinations), so the bases moved
/// to 10 000-40 000 and `v` counts the variants actually built.
pub const TILE_FUEL_ZONE_CELL_ID: i32 = 10_000;
/// See [`TILE_FUEL_ZONE_CELL_ID`].
pub const TILE_FUEL_SHELL_CELL_ID: i32 = 20_000;
/// See [`TILE_FUEL_ZONE_CELL_ID`].
pub const TILE_DUMMY_BALL_CELL_ID: i32 = 30_000;
/// See [`TILE_FUEL_ZONE_CELL_ID`].
pub const TILE_HELIUM_CELL_ID: i32 = 40_000;

/// What a cell of a bed tile universe is: which kind of pebble a point in it
/// belongs to. Lets a sampler measure the realised fuel-BALL fraction from the
/// assembled geometry rather than from the assignment that built it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileCellRole {
    /// Inside a fuelled pebble's 2.5 cm fuel zone (the TRISO lattice).
    FuelZone,
    /// In a fuelled pebble's fuel-free graphite shell.
    FuelShell,
    /// Inside a dummy (all-graphite) pebble.
    DummyBall,
    /// Helium between pebbles.
    Helium,
}

/// The role of a bed-tile cell from its id, or `None` for any other cell.
#[must_use]
pub fn tile_cell_role(cell_id: i32) -> Option<TileCellRole> {
    match cell_id.div_euclid(10_000) {
        1 => Some(TileCellRole::FuelZone),
        2 => Some(TileCellRole::FuelShell),
        3 => Some(TileCellRole::DummyBall),
        4 => Some(TileCellRole::Helium),
        _ => None,
    }
}
