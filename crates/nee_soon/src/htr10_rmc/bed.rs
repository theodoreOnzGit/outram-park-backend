// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header for licence terms.

//! The HTR-10 pebble bed as a hexagonal lattice, reconstructed from the paper.
//!
//! # Why this had to be reconstructed rather than read off
//!
//! The paper describes the core as hexagonal prism unit cells but **never states
//! the pitch**. Its prose —
//!
//! > *"There are seven balls at these faces; one at the center of the basal
//! > plane and six surrounding spheres ... The intermediate section of each
//! > hexagonal prism contains three full balls as well as partial contributions
//! > from the neighboring hexagonal prism cells from all six sides."*
//!
//! — does not determine a cell: it describes sharing between neighbours without
//! saying how much, and the figures carry no extractable dimensions. So the cell
//! is derived from the invariants the paper **does** state, and then checked
//! against one it states that was *not* used in the derivation.
//!
//! # The decode
//!
//! The paper gives a layer height of 9.798 cm. For 6 cm spheres the close-packed
//! layer spacing is `d*sqrt(2/3)` = 4.8990 cm, and `9.798 / 4.8990 = 2.00001`.
//! The "layer" is exactly **two close-packed sphere layers**. That fixes the
//! axial structure and is not plausibly a coincidence.
//!
//! The lattice is then **diluted** laterally until the filling matches the
//! paper's 61 %: ordered close packing would be 74.05 %, so the balls do not
//! touch. Expanding the pitch from the touching value `d` by
//! `sqrt(0.7405/0.61)` gives 6.6106 cm.
//!
//! # The check that makes it evidence
//!
//! The cell above was fitted to **two** stated quantities — layer height and
//! filling fraction. Tiling it through the stated core (180 cm diameter,
//! 197 cm high) predicts **~27 038 balls** against the paper's stated **27 000**,
//! a 0.14 % difference. Nothing was tuned to hit that, which is what makes the
//! reconstruction evidence rather than a fit.
//!
//! # What this is not
//!
//! It reproduces the paper's stated *invariants*. It does **not** claim to be
//! their exact unit-cell tiling, which their text does not determine. Any
//! write-up must say so.
//!
//! # Where it is built (2026-09-25)
//!
//! [`TwoBallBed`] builds this cell as the transported bed of
//! `core_model::assemble_explicit_triso`: the hex tile IS the cell (two whole
//! balls per tile, A-B stacked), with fuel/dummy assigned per ball (gh:#309
//! step 2, gh:#310). Until then the lattice held one ball per half-height tile,
//! which dropped the A-B offset and made the pebbles interpenetrate.
//! [`bed_tile_levels`] (one identity per TILE) remains only for the
//! homogenised `core_model::assemble`.

use std::f64::consts::PI;

/// Close-packed layer spacing for spheres of diameter `d` \[cm\]: `d*sqrt(2/3)`.
#[must_use]
pub fn close_packed_layer_spacing(d: f64) -> f64 {
    d * (2.0_f64 / 3.0).sqrt()
}

/// Packing fraction of ordered close packing (FCC/HCP), `pi/(3*sqrt(2))`.
#[must_use]
pub fn close_packed_fraction() -> f64 {
    PI / (3.0 * 2.0_f64.sqrt())
}

/// One hexagonal-prism unit cell of the HTR-10 bed.
///
/// Flat-to-flat `pitch` is the centre-to-centre distance between neighbouring
/// cells, so the hexagon's area is `(sqrt(3)/2) * pitch^2`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexBedCell {
    /// Flat-to-flat pitch \[cm\].
    pub pitch: f64,
    /// Cell height \[cm\] — one "layer" in the paper's sense.
    pub height: f64,
    /// Balls per cell (one per close-packed layer).
    pub balls: f64,
    /// Ball diameter \[cm\].
    pub ball_diameter: f64,
}

impl HexBedCell {
    /// The cell reconstructed from the paper — see the module docs.
    #[must_use]
    pub fn from_paper() -> Self {
        let d = 6.0;
        let height = 2.0 * close_packed_layer_spacing(d);
        // Dilute from the touching (close-packed) pitch to the stated 61 %.
        let pitch = d * (close_packed_fraction() / 0.61).sqrt();
        Self {
            pitch,
            height,
            balls: 2.0,
            ball_diameter: d,
        }
    }

    /// Cell volume \[cm^3\].
    #[must_use]
    pub fn volume(&self) -> f64 {
        (3.0_f64.sqrt() / 2.0) * self.pitch * self.pitch * self.height
    }

    /// Ball volume fraction in the cell \[-\].
    #[must_use]
    pub fn packing_fraction(&self) -> f64 {
        let r = self.ball_diameter / 2.0;
        self.balls * (4.0 / 3.0 * PI * r.powi(3)) / self.volume()
    }

    /// Nearest-neighbour centre distance **within** a layer \[cm\] — simply the
    /// pitch.
    #[must_use]
    pub fn in_plane_spacing(&self) -> f64 {
        self.pitch
    }

    /// Nearest-neighbour centre distance **between** layers \[cm\], a ball to
    /// the hollow above it.
    #[must_use]
    pub fn interlayer_spacing(&self) -> f64 {
        let lateral = self.pitch / 3.0_f64.sqrt();
        let axial = self.height / 2.0;
        (lateral * lateral + axial * axial).sqrt()
    }

    /// Whether any two balls overlap. A lattice that overlaps is not a packing,
    /// and a packing fraction computed from an overlapping lattice is a fiction.
    #[must_use]
    pub fn is_non_overlapping(&self) -> bool {
        self.in_plane_spacing() > self.ball_diameter
            && self.interlayer_spacing() > self.ball_diameter
    }

    /// Balls in a cylindrical core of the given diameter and height \[cm\].
    ///
    /// Continuum estimate — cells per unit area times the core's footprint —
    /// rather than an integer tiling, because the boundary cells are partial and
    /// the paper's own 27 000 is a round design figure, not a count.
    #[must_use]
    pub fn balls_in_core(&self, core_diameter_cm: f64, core_height_cm: f64) -> f64 {
        let footprint = PI * (core_diameter_cm / 2.0).powi(2);
        let cells_per_area = 1.0 / ((3.0_f64.sqrt() / 2.0) * self.pitch * self.pitch);
        let layers = core_height_cm / self.height;
        footprint * cells_per_area * layers * self.balls
    }

    /// Fuel and moderator balls in the core at the paper's 0.57/0.43 ratio.
    #[must_use]
    pub fn fuel_and_moderator_balls(
        &self,
        core_diameter_cm: f64,
        core_height_cm: f64,
    ) -> (f64, f64) {
        let total = self.balls_in_core(core_diameter_cm, core_height_cm);
        (total * 0.57, total * 0.43)
    }
}

// ---------------------------------------------------------------------------
// THE HEX-PRISM LATTICE (bn:op-867c.9)
// ---------------------------------------------------------------------------

/// Which universe each tile of the bed lattice is filled with.
///
/// The RMC paper selects fuel and moderator balls **per layer** to give the
/// 57:43 ratio, so the assignment is a property of the whole stack rather than
/// of any one tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BedTile {
    /// A fuel pebble (TRISO in graphite).
    Fuel,
    /// A moderator / dummy pebble (graphite only).
    Moderator,
}

/// **Tile assignment for the hex-prism pebble bed**, `levels[axial][ring][elem]`,
/// in the layout [`outram_mc_libs::geometry::lattice::HexLattice::from_rings_3d`]
/// expects.
///
/// NEW WORK, not a port.
///
/// **Used only by the one-ball-per-tile `core_model::assemble` since
/// 2026-09-25.** The explicit-TRISO bed assigns identities per BALL through
/// [`TwoBallBed`], because in the two-ball cell a tile holds pieces of five
/// balls and a per-tile identity would make one pebble part fuel, part
/// graphite.
///
/// # The paper's recipe
///
/// > hexagonal prism unit cells assembled as layers, layer height 9.798 cm …
/// > fuel and moderator selected per layer to give **0.57 : 0.43** … the fuel
/// > step size is one layer precisely so no fractional balls appear
///
/// # How the split is realised
///
/// Deterministically, by a **low-discrepancy rule** rather than a random draw:
/// tile `n` (counting through the whole stack in ring-major order) is fuel when
/// `floor((n+1) * f) > floor(n * f)` for `f = 0.57`. That is Bresenham's line
/// algorithm, and it gives the closest achievable ratio at **every prefix**,
/// not just overall — so a partially built or truncated core still carries the
/// right mixture, which a block assignment would not.
///
/// A random draw at 0.57 would also converge, but it would make the geometry
/// seed-dependent, and a code-to-code comparison should not be.
///
/// # Parameters
/// - `n_rings` — hexagonal rings, including the central tile.
/// - `n_axial` — layers in the stack.
/// - `fuel_universe`, `moderator_universe` — universe indices to place.
pub fn bed_tile_levels(
    n_rings: usize,
    n_axial: usize,
    fuel_universe: usize,
    moderator_universe: usize,
) -> Vec<Vec<Vec<usize>>> {
    let f = super::table1::FUEL_BALL_FRACTION;
    let mut n: usize = 0;
    let mut out = Vec::with_capacity(n_axial);
    for _ in 0..n_axial {
        let mut level = Vec::with_capacity(n_rings);
        // OUTER-FIRST. `HexLattice::from_rings_3d` hard-asserts this ordering:
        // for an n-ring lattice, `levels[z][0]` is the OUTERMOST ring with
        // 6*(n-1) tiles and the last entry is the single central tile. Emitting
        // centre-first panics with "ring 0 (outer-first) has 1 elements,
        // expected 6" -- which is how this was found.
        for ring in (0..n_rings).rev() {
            // The central ring holds one tile; ring r holds 6r.
            let count = if ring == 0 { 1 } else { 6 * ring };
            let mut elems = Vec::with_capacity(count);
            for _ in 0..count {
                let a = ((n as f64) * f).floor();
                let b = (((n + 1) as f64) * f).floor();
                elems.push(if b > a {
                    fuel_universe
                } else {
                    moderator_universe
                });
                n += 1;
            }
            level.push(elems);
        }
        out.push(level);
    }
    out
}

// ---------------------------------------------------------------------------
// THE TWO-BALL PRISM CELL (gh:#309 step 2, gh:#310)
// ---------------------------------------------------------------------------

/// One of the five ball sites a **two-ball** hex tile holds a piece of.
///
/// The tile is the paper's prism ([`HexBedCell::from_paper`]): flat-to-flat
/// `pitch`, height `height` = one A-B layer pair. In tile-local coordinates
/// (tile centre at the origin, a `HexOrientation::Y` lattice, whose vertices
/// sit at polar angles 0, 60, ..., 300 degrees and distance `pitch/sqrt(3)`):
///
/// | site | centre | shared by |
/// |---|---|---|
/// | `ABottom` | `(0, 0, -height/2)` | this tile and the one below (half each) |
/// | `ATop` | `(0, 0, +height/2)` | this tile and the one above (half each) |
/// | `BEast` | `(pitch/sqrt(3), 0, 0)` — the 0 degree vertex | the three tiles meeting there (a third each) |
/// | `BNorthWest` | the 120 degree vertex | three tiles |
/// | `BSouthWest` | the 240 degree vertex | three tiles |
///
/// `2 x 1/2 + 3 x 1/3 = 2` balls per tile. The B layer uses **alternate**
/// vertices only; the other three (60, 180, 300 degrees) are empty, which is
/// what makes the stacking A-B rather than a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BallSite {
    /// A-layer ball on the tile axis at the bottom face.
    ABottom,
    /// A-layer ball on the tile axis at the top face.
    ATop,
    /// B-layer ball at the 0 degree vertex, mid-height.
    BEast,
    /// B-layer ball at the 120 degree vertex, mid-height.
    BNorthWest,
    /// B-layer ball at the 240 degree vertex, mid-height.
    BSouthWest,
}

impl BallSite {
    /// The five sites, in the bit order of [`TwoBallBed::tile_mask`].
    pub const ALL: [BallSite; 5] = [
        BallSite::ABottom,
        BallSite::ATop,
        BallSite::BEast,
        BallSite::BNorthWest,
        BallSite::BSouthWest,
    ];
}

impl HexBedCell {
    /// Tile centre to vertex distance \[cm\], `pitch/sqrt(3)` — the lateral
    /// offset between the A and B layers.
    #[must_use]
    pub fn vertex_radius(&self) -> f64 {
        self.pitch / 3.0_f64.sqrt()
    }

    /// Tile-local centre \[cm\] of `site` in a `HexOrientation::Y` lattice of
    /// this cell. See [`BallSite`].
    #[must_use]
    pub fn site_centre(&self, site: BallSite) -> [f64; 3] {
        let s = self.vertex_radius();
        let c = 0.5 * 3.0_f64.sqrt() * s;
        match site {
            BallSite::ABottom => [0.0, 0.0, -0.5 * self.height],
            BallSite::ATop => [0.0, 0.0, 0.5 * self.height],
            BallSite::BEast => [s, 0.0, 0.0],
            BallSite::BNorthWest => [-0.5 * s, c, 0.0],
            BallSite::BSouthWest => [-0.5 * s, -c, 0.0],
        }
    }
}

/// A ball of the global bed, named by the tile that **owns** it.
///
/// `(a, b)` are the skewed axial hex coordinates of a tile, with the central
/// tile at `(0, 0)` (`a = ix - (n_rings-1)`, `b = iy - (n_rings-1)` in
/// `HexLattice`'s index triplet). An A ball belongs to a column and a face; a
/// B ball to the tile whose `BEast` vertex it sits on. Every piece of one ball,
/// in every tile that holds a piece of it, resolves to the SAME `BallId` — that
/// is what makes the fuel/dummy identity per ball rather than per tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BallId {
    /// A-layer ball on the axis of column `(a, b)`, on the bottom face of
    /// lattice level `face` (so the top face of level `face - 1`).
    A {
        /// Skewed hex coordinate.
        a: i32,
        /// Skewed hex coordinate.
        b: i32,
        /// Face index, 0 = bottom face of lattice level 0.
        face: i32,
    },
    /// B-layer ball at the `BEast` vertex of tile `(a, b)`, mid-height of
    /// lattice level `level`.
    B {
        /// Skewed hex coordinate of the owning tile.
        a: i32,
        /// Skewed hex coordinate of the owning tile.
        b: i32,
        /// Lattice level.
        level: i32,
    },
}

/// The ball at `site` of tile `(a, b)` in lattice level `level`.
///
/// The vertex ownership follows from the Y-orientation tile centres
/// `(sqrt(3)/2 a, b + a/2) * pitch`: tile `(a, b)`'s 120 degree vertex is the
/// 0 degree vertex of tile `(a-1, b+1)`, and its 240 degree vertex that of
/// `(a-1, b)`. Checked numerically by
/// `every_tile_resolves_a_shared_ball_to_one_position` below.
#[must_use]
pub fn tile_ball(a: i32, b: i32, level: i32, site: BallSite) -> BallId {
    match site {
        BallSite::ABottom => BallId::A { a, b, face: level },
        BallSite::ATop => BallId::A {
            a,
            b,
            face: level + 1,
        },
        BallSite::BEast => BallId::B { a, b, level },
        BallSite::BNorthWest => BallId::B {
            a: a - 1,
            b: b + 1,
            level,
        },
        BallSite::BSouthWest => BallId::B { a: a - 1, b, level },
    }
}

/// Planar centre \[cm\] of tile `(a, b)` in a `HexOrientation::Y` lattice
/// centred on the origin — the same arithmetic as `HexLattice::center_offset`.
#[must_use]
pub fn tile_xy(a: i32, b: i32, pitch: f64) -> [f64; 2] {
    let (a, b) = (f64::from(a), f64::from(b));
    [0.5 * 3.0_f64.sqrt() * a * pitch, (b + 0.5 * a) * pitch]
}

/// Hexagonal ring index (0 = centre) of skewed coordinates `(a, b)`.
#[must_use]
pub fn hex_ring(a: i32, b: i32) -> usize {
    ((a.abs() + b.abs() + (a + b).abs()) / 2) as usize
}

/// How fuel and dummy identities are handed out over the balls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelAssignment {
    /// The paper and Terry (2005): 57:43 over every ball with volume inside
    /// the bed cylinder, the conus (every ball centred below the bed floor)
    /// all dummy.
    Paper,
    /// ABLATION: the conus takes the 57:43 split too (`OUTRAM_HTR10_FUEL_CONUS`).
    FuelledConus,
    /// ABLATION: every ball is fuelled (`OUTRAM_HTR10_ALLFUEL`).
    AllFuel,
}

/// **The HTR-10 bed as the paper's two-ball prism cell**, with a fuel/dummy
/// identity for every BALL (gh:#309 step 2, gh:#310).
///
/// NEW WORK, not a port.
///
/// # Axial layout
///
/// Ball layers sit every `height/2` = 4.899 cm, alternating A (on the tile
/// axis, at the tile faces) and B (at alternate vertices, at mid-height). The
/// bed of `n_axial` half-layers spans `[-n_axial*height/4, +n_axial*height/4]`
/// and its ball layers are centred at `bed_bottom + (j + 1/2) * height/2`,
/// `j = 0 .. n_axial-1`, so each layer is centred in its own 4.899 cm slab and
/// the laterally-averaged filling fraction of the bed slab is exactly the
/// cell's 0.61 (the lateral average is periodic with period `height/2`, and
/// the bed is a whole number of periods).
///
/// **The stacking phase is anchored at the bed FLOOR**: layer `j = 0` is
/// always an A layer. The bed floor is fixed hardware (the top of the conus),
/// so a taller loading only adds layers on top and everything below is
/// identical at every loading, exactly as in the reactor's loading sequence.
/// An odd `n_axial` therefore ends on an A layer and an even one on a B layer;
/// nothing else distinguishes them.
///
/// The lattice runs from below the conus floor to above the bed top, so every
/// ball that has volume in the bed or the conus is present — including the
/// layer centred 2.449 cm above the bed top, whose lower 0.55 cm is inside the
/// bed and replaces the top layer's upper 0.55 cm, which the bed plane clips
/// off. (Without it the top slab would be under-packed.)
///
/// # Fuel/dummy identity
///
/// Assigned to BALLS, never to tiles: every tile holding a piece of a ball
/// (2 for an A ball, 3 for a B ball) sees the same identity, because each
/// reads it from [`Self::is_fuel`] through the ball's [`BallId`].
///
/// Under [`FuelAssignment::Paper`] the eligible balls are those centred above
/// the bed floor with volume inside the bed cylinder (centre within one ball
/// radius of it, radially and at the top). Balls centred below the floor are
/// the conus and are all dummy (Terry 2005 s2). The eligible balls are taken
/// in **layer order from the floor up, then by distance from the axis, then by
/// angle**, and ball `n` is fuel when `floor((n+1) f) > floor(n f)`, `f = 0.57`
/// — the same low-discrepancy (Bresenham) rule [`bed_tile_levels`] used for
/// tiles. That order makes the split right in every prefix of layers and of
/// every annulus, and, because it runs from the floor up, a taller loading
/// never reshuffles the balls below it.
#[derive(Debug, Clone)]
pub struct TwoBallBed {
    /// The unit cell (pitch 6.6106 cm, height 9.798 cm, 6 cm balls).
    pub cell: HexBedCell,
    /// Hex rings in the lattice (including the central tile).
    pub n_rings: usize,
    /// Axial lattice levels, each one A-B pair (`cell.height`) tall.
    pub n_levels: usize,
    /// z \[cm\] of the bottom face of lattice level 0.
    pub z_bottom: f64,
    /// Bed cylinder radius \[cm\].
    pub bed_radius: f64,
    /// Bed floor \[cm\] (= top of the conus).
    pub bed_bottom: f64,
    /// Bed top \[cm\].
    pub bed_top: f64,
    /// Conus floor \[cm\].
    pub conus_floor: f64,
    /// The rule the identities were assigned by.
    pub assignment: FuelAssignment,
    /// Balls that took part in the 57:43 split.
    pub eligible_balls: usize,
    /// Of which fuelled.
    pub fuel_balls: usize,
    a_fuel: Vec<bool>,
    b_fuel: Vec<bool>,
}

impl TwoBallBed {
    /// Build the bed.
    ///
    /// # Parameters
    /// - `cell` — the unit cell, normally [`HexBedCell::from_paper`].
    /// - `n_rings` — hex rings of the lattice; must tile `bed_radius`.
    /// - `n_axial` — fuel loading height in half-layers (`cell.height/2`
    ///   each); the bed is `n_axial * cell.height / 2` tall, centred on z = 0.
    /// - `conus_height` — depth \[cm\] of the conus below the bed floor.
    /// - `bed_radius` — \[cm\].
    /// - `assignment` — see [`FuelAssignment`].
    #[must_use]
    pub fn new(
        cell: HexBedCell,
        n_rings: usize,
        n_axial: usize,
        conus_height: f64,
        bed_radius: f64,
        assignment: FuelAssignment,
    ) -> Self {
        let h = cell.height;
        let bed_top = 0.25 * h * n_axial as f64;
        let bed_bottom = -bed_top;
        let conus_floor = bed_bottom - conus_height;
        // Faces (A layers) at bed_bottom + h/4 + m h. One margin level below
        // the conus floor and above the bed top, so no face can coincide with
        // either plane and the `outer` universe is never reached in the bed.
        let m_lo = ((conus_floor - bed_bottom - 0.25 * h) / h).floor() as i64 - 1;
        let m_hi = ((bed_top - bed_bottom - 0.25 * h) / h).ceil() as i64 + 1;
        let n_levels = (m_hi - m_lo) as usize;
        let z_bottom = bed_bottom + 0.25 * h + m_lo as f64 * h;
        let w = 2 * n_rings + 1;
        let mut bed = Self {
            cell,
            n_rings,
            n_levels,
            z_bottom,
            bed_radius,
            bed_bottom,
            bed_top,
            conus_floor,
            assignment,
            eligible_balls: 0,
            fuel_balls: 0,
            a_fuel: vec![false; w * w * (n_levels + 1)],
            b_fuel: vec![false; w * w * n_levels],
        };
        bed.assign();
        bed
    }

    /// z \[cm\] of the lattice centre, to pass to `HexLattice::from_rings_3d`.
    #[must_use]
    pub fn lattice_centre_z(&self) -> f64 {
        self.z_bottom + 0.5 * self.n_levels as f64 * self.cell.height
    }

    fn width(&self) -> usize {
        2 * self.n_rings + 1
    }

    fn slot(&self, id: BallId) -> Option<(bool, usize)> {
        let nr = self.n_rings as i32;
        let w = self.width();
        let (a, b, k, is_a, nk) = match id {
            BallId::A { a, b, face } => (a, b, face, true, self.n_levels + 1),
            BallId::B { a, b, level } => (a, b, level, false, self.n_levels),
        };
        if a < -nr || a > nr || b < -nr || b > nr || k < 0 || k as usize >= nk {
            return None;
        }
        let i = ((k as usize) * w + (b + nr) as usize) * w + (a + nr) as usize;
        Some((is_a, i))
    }

    /// Global centre \[cm\] of ball `id`.
    #[must_use]
    pub fn centre(&self, id: BallId) -> [f64; 3] {
        let p = self.cell.pitch;
        let h = self.cell.height;
        match id {
            BallId::A { a, b, face } => {
                let [x, y] = tile_xy(a, b, p);
                [x, y, self.z_bottom + f64::from(face) * h]
            }
            BallId::B { a, b, level } => {
                let [x, y] = tile_xy(a, b, p);
                [
                    x + self.cell.vertex_radius(),
                    y,
                    self.z_bottom + (f64::from(level) + 0.5) * h,
                ]
            }
        }
    }

    /// Whether ball `id` is fuelled. Balls outside the lattice's range are
    /// dummy.
    #[must_use]
    pub fn is_fuel(&self, id: BallId) -> bool {
        match self.slot(id) {
            Some((true, i)) => self.a_fuel[i],
            Some((false, i)) => self.b_fuel[i],
            None => false,
        }
    }

    /// The five balls tile `(a, b, level)` holds pieces of, in
    /// [`BallSite::ALL`] order.
    #[must_use]
    pub fn tile_balls(&self, a: i32, b: i32, level: i32) -> [BallId; 5] {
        BallSite::ALL.map(|s| tile_ball(a, b, level, s))
    }

    /// Fuel mask of tile `(a, b, level)`: bit `i` set when the ball at
    /// `BallSite::ALL[i]` is fuelled. Selects the tile's universe variant.
    #[must_use]
    pub fn tile_mask(&self, a: i32, b: i32, level: i32) -> u8 {
        self.tile_balls(a, b, level)
            .iter()
            .enumerate()
            .fold(0u8, |m, (i, id)| m | (u8::from(self.is_fuel(*id)) << i))
    }

    /// Every ball any tile of the lattice holds a piece of, in a deterministic
    /// order (A balls, then B balls, each by index).
    #[must_use]
    pub fn all_balls(&self) -> Vec<BallId> {
        let nr = self.n_rings as i32;
        let mut a_cols = std::collections::BTreeSet::new();
        let mut b_owners = std::collections::BTreeSet::new();
        for a in -(nr - 1)..=(nr - 1) {
            for b in -(nr - 1)..=(nr - 1) {
                if hex_ring(a, b) > self.n_rings - 1 {
                    continue;
                }
                a_cols.insert((a, b));
                for s in [BallSite::BEast, BallSite::BNorthWest, BallSite::BSouthWest] {
                    if let BallId::B { a, b, .. } = tile_ball(a, b, 0, s) {
                        b_owners.insert((a, b));
                    }
                }
            }
        }
        let mut out = Vec::new();
        for face in 0..=self.n_levels as i32 {
            out.extend(a_cols.iter().map(|&(a, b)| BallId::A { a, b, face }));
        }
        for level in 0..self.n_levels as i32 {
            out.extend(b_owners.iter().map(|&(a, b)| BallId::B { a, b, level }));
        }
        out
    }

    /// Axial layer index of a ball, counted in half-layers from lattice face
    /// 0 (A layers even, B layers odd).
    fn layer(id: BallId) -> i32 {
        match id {
            BallId::A { face, .. } => 2 * face,
            BallId::B { level, .. } => 2 * level + 1,
        }
    }

    fn assign(&mut self) {
        let r = 0.5 * self.cell.ball_diameter;
        let f = super::table1::FUEL_BALL_FRACTION;
        let mut eligible: Vec<(i32, f64, f64, BallId)> = Vec::new();
        let all = self.all_balls();
        for &id in &all {
            let [x, y, z] = self.centre(id);
            let rxy = x.hypot(y);
            let ok = match self.assignment {
                FuelAssignment::AllFuel => true,
                FuelAssignment::Paper => {
                    z > self.bed_bottom && z - r < self.bed_top && rxy - r < self.bed_radius
                }
                FuelAssignment::FuelledConus => {
                    z + r > self.conus_floor && z - r < self.bed_top && rxy - r < self.bed_radius
                }
            };
            if ok {
                eligible.push((Self::layer(id), x * x + y * y, y.atan2(x), id));
            }
        }
        eligible.sort_by(|p, q| {
            p.0.cmp(&q.0)
                .then(p.1.total_cmp(&q.1))
                .then(p.2.total_cmp(&q.2))
                .then(p.3.cmp(&q.3))
        });
        self.eligible_balls = eligible.len();
        self.fuel_balls = 0;
        for (n, &(_, _, _, id)) in eligible.iter().enumerate() {
            let fuel = self.assignment == FuelAssignment::AllFuel
                || ((n + 1) as f64 * f).floor() > (n as f64 * f).floor();
            if fuel {
                self.fuel_balls += 1;
                match self.slot(id) {
                    Some((true, i)) => self.a_fuel[i] = true,
                    Some((false, i)) => self.b_fuel[i] = true,
                    None => unreachable!("every enumerated ball has a slot"),
                }
            }
        }
    }
}

/// Count `(fuel, moderator)` tiles in an assignment from [`bed_tile_levels`].
#[must_use]
pub fn count_tiles(levels: &[Vec<Vec<usize>>], fuel_universe: usize) -> (usize, usize) {
    let mut fuel = 0;
    let mut total = 0;
    for lvl in levels {
        for ring in lvl {
            for &u in ring {
                total += 1;
                if u == fuel_universe {
                    fuel += 1;
                }
            }
        }
    }
    (fuel, total - fuel)
}

#[cfg(test)]
mod hex_lattice_tests {
    use super::*;

    /// The 57:43 split must be met to within one tile — the closest achievable,
    /// since tiles are integral.
    ///
    /// **Results (2026-09-17):** printed below; the realised fuel fraction is
    /// within a single tile of 0.57 at every size tried.
    #[test]
    fn the_fuel_fraction_matches_the_paper_to_within_one_tile() {
        for (rings, axial) in [(5, 10), (9, 20), (12, 21), (15, 30)] {
            let levels = bed_tile_levels(rings, axial, 1, 2);
            let (fuel, moderator) = count_tiles(&levels, 1);
            let total = fuel + moderator;
            let frac = fuel as f64 / total as f64;
            let target = super::super::table1::FUEL_BALL_FRACTION;
            println!(
                "{rings} rings x {axial} layers: {total:>7} tiles, {fuel:>7} fuel \
                 -> {frac:.6} (target {target})"
            );
            assert!(
                (frac * total as f64 - target * total as f64).abs() <= 1.0,
                "{rings}x{axial}: realised {frac:.6} is more than one tile from {target}"
            );
        }
    }

    /// **The split must be right at EVERY PREFIX, not only overall.**
    ///
    /// This is what the low-discrepancy rule buys over a block assignment, and
    /// why it was chosen: a partially loaded core -- which is exactly what the
    /// paper's 12-point loading curve sweeps -- must carry the right mixture at
    /// each height, not only when complete.
    #[test]
    fn every_prefix_carries_the_right_mixture() {
        let levels = bed_tile_levels(12, 21, 1, 2);
        let flat: Vec<usize> = levels.iter().flatten().flatten().copied().collect();
        let target = super::super::table1::FUEL_BALL_FRACTION;
        let mut fuel = 0usize;
        let mut worst = 0.0_f64;
        for (i, &u) in flat.iter().enumerate() {
            if u == 1 {
                fuel += 1;
            }
            let n = i + 1;
            // Deviation in TILES, which is the meaningful unit.
            worst = worst.max((fuel as f64 - target * n as f64).abs());
        }
        println!(
            "{} tiles, worst prefix deviation {worst:.4} tiles",
            flat.len()
        );
        assert!(
            worst < 1.0,
            "a low-discrepancy split must stay within one tile of the target at \
             every prefix; worst {worst:.4}. A block assignment would reach the \
             right total while being wrong everywhere in between."
        );
    }

    /// Ring sizes must be 1, 6, 12, 18 … or `HexLattice::from_rings_3d` will
    /// reject the levels — it hard-asserts them.
    #[test]
    fn ring_sizes_match_what_the_lattice_expects() {
        let levels = bed_tile_levels(6, 3, 1, 2);
        assert_eq!(levels.len(), 3, "three axial levels");
        for lvl in &levels {
            assert_eq!(lvl.len(), 6, "six rings");
            // Outer-first: entry i is ring (n_rings - 1 - i).
            for (i, ring) in lvl.iter().enumerate() {
                let r = lvl.len() - 1 - i;
                let want = if r == 0 { 1 } else { 6 * r };
                assert_eq!(
                    ring.len(),
                    want,
                    "entry {i} is ring {r}, must hold {want} tiles"
                );
            }
        }
    }

    /// The assignment is deterministic: same inputs, same tiles. A code-to-code
    /// comparison cannot rest on a seed.
    #[test]
    fn the_assignment_is_deterministic() {
        assert_eq!(bed_tile_levels(9, 12, 1, 2), bed_tile_levels(9, 12, 1, 2));
    }

    /// **The vertex-ownership rule in [`tile_ball`] is geometry, not
    /// bookkeeping.** For every tile of a small lattice, the global position of
    /// each of its five sites (tile centre + [`HexBedCell::site_centre`]) must
    /// equal [`TwoBallBed::centre`] of the ball [`tile_ball`] names for it. A
    /// wrong owner offset puts a tile's corner piece on a different ball from
    /// its neighbours', which is exactly the per-tile identity defect the
    /// two-ball construction exists to avoid.
    #[test]
    fn every_tile_resolves_a_shared_ball_to_one_position() {
        let cell = HexBedCell::from_paper();
        let bed = TwoBallBed::new(cell, 4, 6, 10.0, 20.0, FuelAssignment::Paper);
        let nr = bed.n_rings as i32;
        let mut checked = 0;
        for level in 0..bed.n_levels as i32 {
            for a in -(nr - 1)..=(nr - 1) {
                for b in -(nr - 1)..=(nr - 1) {
                    if hex_ring(a, b) > bed.n_rings - 1 {
                        continue;
                    }
                    let [tx, ty] = tile_xy(a, b, cell.pitch);
                    let tz = bed.z_bottom + (f64::from(level) + 0.5) * cell.height;
                    for s in BallSite::ALL {
                        let l = cell.site_centre(s);
                        let g = bed.centre(tile_ball(a, b, level, s));
                        let d = ((tx + l[0] - g[0]).powi(2)
                            + (ty + l[1] - g[1]).powi(2)
                            + (tz + l[2] - g[2]).powi(2))
                        .sqrt();
                        assert!(d < 1e-9, "tile ({a},{b},{level}) site {s:?} is {d} cm off");
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 100);
    }

    /// Identities are a property of the BALL SET, so they must not depend on
    /// the loading height below the top layers: a taller loading adds balls on
    /// top and must not reshuffle the ones beneath (the order runs from the
    /// floor up). Checked on every ball centred in the lower bed.
    #[test]
    fn a_taller_loading_does_not_reshuffle_the_balls_below() {
        let cell = HexBedCell::from_paper();
        let lo = TwoBallBed::new(cell, 6, 8, 12.0, 30.0, FuelAssignment::Paper);
        let hi = TwoBallBed::new(cell, 6, 13, 12.0, 30.0, FuelAssignment::Paper);
        let mut compared = 0;
        for id in lo.all_balls() {
            let c = lo.centre(id);
            if c[2] > lo.bed_top - 3.0 {
                continue;
            }
            // The same physical ball in the taller bed: same position
            // relative to the bed floor.
            let shift = hi.bed_bottom - lo.bed_bottom;
            let twin = hi
                .all_balls()
                .into_iter()
                .find(|&j| {
                    let d = hi.centre(j);
                    (d[0] - c[0]).abs() < 1e-9
                        && (d[1] - c[1]).abs() < 1e-9
                        && (d[2] - c[2] - shift).abs() < 1e-9
                })
                .expect("the taller bed holds every lower ball");
            assert_eq!(lo.is_fuel(id), hi.is_fuel(twin), "{id:?} changed identity");
            compared += 1;
            if compared > 400 {
                break;
            }
        }
        assert!(compared > 100);
    }
}
