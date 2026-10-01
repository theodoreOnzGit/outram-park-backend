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
//! > plane and six surrounding spheres. These six spheres are not centered at
//! > the corners of the hexagons, but rather, hexagonal prism side surfaces
//! > surround these balls. The intermediate section of each hexagonal prism
//! > contains three full balls as well as partial contributions from the
//! > neighboring hexagonal prism cells from all six sides."*
//!
//! **CORRECTED 2026-10-01 (gh:#429):** the quotation above was previously
//! elided (`...`) exactly across the "not centered at the corners" sentence,
//! which is restored. The text is Li, Yu & Wei (2014) p.3, and it is
//! **verbatim from Şeker & Çolak (2003), p.266**, the MCNP model Li follows:
//! Şeker, V., Çolak, Ü. (2003), *HTR-10 full core first criticality analysis
//! with MCNP*, Nucl. Eng. Des. 222, 263–270, doi:10.1016/S0029-5493(03)00031-1.
//! Şeker is therefore the primary source for the cell; see "What Şeker & Çolak
//! (2003) settles" below.
//!
//! — ~~does not determine a cell: it describes sharing between neighbours
//! without saying how much, and the figures carry no extractable dimensions.~~
//! **CORRECTED 2026-10-01:** the prose alone does not determine a cell, but the
//! figures do carry dimensions (the 6 cm ball is a built-in scale bar, gh:#429),
//! and Şeker's Table 3 ball counts constrain the cell independently (below).
//! The cell here is derived from the invariants the paper **does** state.
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
//! # ~~The check that makes it evidence~~ A consistency check, not evidence
//!
//! The cell above was fitted to **two** stated quantities — layer height and
//! filling fraction. Tiling it through the stated core (180 cm diameter,
//! 197 cm high) predicts **~27 038 balls** against the paper's stated **27 000**,
//! a 0.14 % difference. ~~Nothing was tuned to hit that, which is what makes the
//! reconstruction evidence rather than a fit.~~ **CORRECTED 2026-10-01
//! (gh:#430):** [`HexBedCell::balls_in_core`] reduces to
//! `V_core × 0.61 / V_ball` for **any** pitch and height, so the 27 038 follows
//! from the 0.61 put in and cannot fail. It checks arithmetic, not the cell.
//! The 27 000 is also the **equilibrium** (all-fuel) core, not the 57:43
//! initial core. The independent count the check lacked is Şeker & Çolak
//! (2003) Table 3 (below).
//!
//! # What this is not
//!
//! It reproduces the paper's stated *invariants*. It does **not** claim to be
//! their exact unit-cell tiling, which their text does not determine. Any
//! write-up must say so.
//!
//! # What Şeker & Çolak (2003) settles (read 2026-10-01)
//!
//! Şeker & Çolak (2003), NED 222:263–270 (full citation above), is the MCNP
//! model whose cell text Li (2014) reproduces. What it adds, and where **this
//! module departs from it**:
//!
//! **1. An independent ball count (Table 3).** For N = 9…20 layers, every row
//! satisfies, exactly (checked arithmetically on all 12 rows):
//!
//! | quantity | Şeker Table 3 |
//! |---|---|
//! | loading height | `9.798 N + 6.0` cm |
//! | total balls | `1346 N + 733` |
//! | fuel balls | `767 N + 418` (0.570 of the total) |
//!
//! 1346 balls per 9.798 cm layer in the r = 90 cm core is a filling fraction
//! of **0.6106**, Şeker's *"61%"*, measured **after** the wall rejection in
//! item 3. The constant 733 is one extra **basal** plane: both the top and the
//! bottom basal planes are whole balls. At the critical row (N = 12) the model
//! holds 16 885 balls against the experiment's 16 890 at 123.06 cm.
//!
//! **2. The cell is clustered, not uniformly diluted (gh:#429).** The counts
//! split each layer into **733 balls in the basal plane and 613 in the central
//! plane**: areal fractions through the ball centres of **0.814 / 0.681** over
//! the whole bed. **This module's cell is uniform: 0.747 / 0.747.** A
//! reconstruction consistent with the text, the counts and Li's Fig. 3 (an
//! inference, medium confidence, not built): **13 balls per prism**, i.e. the 7
//! touching basal balls (7:6 = 1.167 against the counted 733:613 = 1.196) plus
//! 3 full + 6 half central balls, with a hexagon circumradius ≈ 9.4–9.7 cm.
//! Fuel is assigned **per layer** to 0.57:0.43 (p.267), not as a fixed per-cell
//! pattern.
//!
//! **3. Wall-crossing balls are rejected everywhere (gh:#331).** p.267:
//! *"Outer boundary of the array is the inner surface of the side reflector. If
//! any ball intersects with the reflector surface, it is rejected."* The same
//! applies at the cone and the discharge tube. **This module cuts balls at the
//! side wall by default** and rejects them only at the cone and tube; see
//! [`TwoBallBed::rejecting_side_wall_crossers`].
//!
//! **4. The top layer is whole.** p.267: *"The top layer is formed by adding
//! half spheres to each ball present in this layer."* The count (+733) says
//! the bottom plane is whole too. **This module clips the top layer** at the
//! bed plane (gh:#429 item 3; see [`TwoBallBed`], "Axial layout").
//!
//! **5. The cone and discharge tube hold graphite balls only**, hexagonally
//! arranged and rejected at the surfaces (p.267), which is what
//! [`TwoBallBed`] builds.
//!
//! **6. The heights (gh:#333).** Şeker's height runs from the bottom of the
//! lowest ball to the top of the highest, but the same balls at 0.61 fill
//! **H − 0.55 cm** of volume-equivalent bed (0.58 cm at N = 9, 0.48 cm at
//! N = 20), not H − 6 cm. Matching a model to a row is therefore done
//! correctly by **ball count**. See [`super::RMC_KEFF_VS_HEIGHT`].
//!
//! # Where it is built (2026-09-25)
//!
//! [`TwoBallBed`] builds this cell as the transported bed of
//! `core_model::assemble_explicit_triso`: the hex tile IS the cell (two whole
//! balls per tile, A-B stacked), with fuel/dummy assigned per ball (gh:#309
//! step 2, gh:#310). Until then the lattice held one ball per half-height tile,
//! which dropped the A-B offset and made the pebbles interpenetrate.
//! [`bed_tile_levels`] (one identity per TILE) remains only for the
//! homogenised `core_model::assemble`, itself withdrawn 2026-10-01 (it cuts
//! pebbles, which is wrong physics).

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
    ///
    /// **Not an independent check (gh:#430, 2026-10-01):** algebraically this is
    /// `V_core × packing_fraction / V_ball`, so for [`Self::from_paper`] it is
    /// `V_core × 0.61 / V_ball` whatever the pitch or height. An independent
    /// count is Şeker & Çolak (2003) Table 3: `1346 N + 733` balls at loading
    /// height `9.798 N + 6.0` cm (module docs).
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
/// **A departure from the source, stated 2026-10-01 (gh:#429 item 3):** Şeker
/// & Çolak (2003) p.267 build the top layer from whole balls (*"formed by
/// adding half spheres to each ball present in this layer"*), and their Table 3
/// count (`1346 N + 733`) implies a whole bottom basal plane as well. The clip
/// here is volume-equivalent, not ball-for-ball.
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
    /// The discharge tube below the conus, and with it Li's whole-ball
    /// rejection, or `None` (see [`DischargeTube`]).
    pub tube: Option<DischargeTube>,
    /// Balls removed by the rejection rule (0 without a tube).
    pub rejected_balls: usize,
    /// Of [`Self::rejected_balls`], those removed at the bed side wall by
    /// [`Self::rejecting_side_wall_crossers`] (0 by default).
    pub side_wall_rejected: usize,
    a_fuel: Vec<bool>,
    b_fuel: Vec<bool>,
    a_present: Vec<bool>,
    b_present: Vec<bool>,
}

/// **The fuel discharge tube, filled with whole graphite balls, and the
/// rejection rule at the cone and tube surfaces.**
///
/// Li, Yu & Wei (2014), section on the RMC model: the cone region and the
/// discharge tube hold graphite balls only, arranged in the same hexagonal
/// geometry, and balls that intersect the cone or discharge-tube surface are
/// rejected. So a ball there is either wholly inside and kept, or removed, and
/// the space it would have taken is helium. The balls are all dummies (Terry
/// et al. 2005, section 2).
///
/// The container is the conus frustum (bed radius at the bed floor down to
/// `radius` at the conus floor) on top of a cylinder of `radius`, `depth`
/// deep. A ball is rejected when it crosses the cone, the tube wall or the
/// tube bottom. The bottom is where the model ends (TECDOC-1382 p. 242 gives
/// the tube to z = 6100 mm, the model bottom); rejecting there too keeps every
/// ball whole, as the rule intends.
///
/// **What this does NOT reject:** balls crossing the bed's side wall
/// (r = 90 cm) above the conus. Li says only that the array's outer boundary is
/// the side reflector's inner surface, not whether wall-crossing balls are cut
/// or removed. Those keep the CSG cut (the treatment before this), and the
/// ~~choice is the maintainer's.~~
/// **DECIDED 2026-09-27 (maintainer, gh:#331): keep the cut.** Li states the
/// finished core's filling fraction is 61 %; the cut bed measures 0.6089, while
/// rejecting wall-crossers ([`TwoBallBed::rejecting_side_wall_crossers`], kept
/// as an ablation) drops it to 0.5737. *"Packing fraction wrong already changes
/// too much."*
///
/// `None` in [`TwoBallBed::new_with_tube`] (the `OUTRAM_HTR10_HOMOG_TUBE`
/// ablation) builds no tube balls and applies no rejection: the cone then cuts
/// partial balls, as it did before 2026-09-25.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DischargeTube {
    /// Tube radius \[cm\] (25 cm, TECDOC-1382 p. 242).
    pub radius: f64,
    /// Tube depth below the conus floor \[cm\], i.e. to the model bottom.
    pub depth: f64,
}

/// Distance from `(px, pz)` to the segment `a`-`b` in the meridional plane.
fn segment_distance(px: f64, pz: f64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dz * dz;
    let t = if l2 > 0.0 {
        (((px - a[0]) * dx + (pz - a[1]) * dz) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - a[0] - t * dx).hypot(pz - a[1] - t * dz)
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
        Self::new_with_tube(cell, n_rings, n_axial, conus_height, bed_radius, None, assignment)
    }

    /// Build the bed, with the discharge tube below the conus and Li's
    /// whole-ball rejection when `tube` is `Some` (see [`DischargeTube`]).
    /// Otherwise as [`Self::new`].
    #[must_use]
    pub fn new_with_tube(
        cell: HexBedCell,
        n_rings: usize,
        n_axial: usize,
        conus_height: f64,
        bed_radius: f64,
        tube: Option<DischargeTube>,
        assignment: FuelAssignment,
    ) -> Self {
        let h = cell.height;
        let bed_top = 0.25 * h * n_axial as f64;
        let bed_bottom = -bed_top;
        let conus_floor = bed_bottom - conus_height;
        let deepest = conus_floor - tube.map_or(0.0, |t| t.depth);
        // Faces (A layers) at bed_bottom + h/4 + m h. One margin level below
        // the deepest ball region and above the bed top, so no face can
        // coincide with either plane and the `outer` universe is never
        // reached in the bed.
        let m_lo = ((deepest - bed_bottom - 0.25 * h) / h).floor() as i64 - 1;
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
            tube,
            rejected_balls: 0,
            side_wall_rejected: 0,
            a_fuel: vec![false; w * w * (n_levels + 1)],
            b_fuel: vec![false; w * w * n_levels],
            a_present: vec![true; w * w * (n_levels + 1)],
            b_present: vec![true; w * w * n_levels],
        };
        bed.reject();
        bed.assign();
        bed
    }

    /// Distance \[cm\] from a point to the boundary of the conus-and-tube
    /// container (cone, tube wall, tube bottom), in the meridional plane; `None`
    /// without a tube.
    ///
    /// A sphere of radius `R` centred there crosses that boundary exactly when
    /// this is below `R`: the set of `(rho, z)` a sphere covers is its
    /// meridional disk, so the sphere meets a surface of revolution exactly
    /// when the disk meets the surface's generating curve.
    #[must_use]
    pub fn container_boundary_distance(&self, centre: [f64; 3]) -> Option<f64> {
        let t = self.tube?;
        let (rho, z) = (centre[0].hypot(centre[1]), centre[2]);
        let bottom = self.conus_floor - t.depth;
        let cone = segment_distance(
            rho,
            z,
            [self.bed_radius, self.bed_bottom],
            [t.radius, self.conus_floor],
        );
        let wall = segment_distance(rho, z, [t.radius, self.conus_floor], [t.radius, bottom]);
        let floor = segment_distance(rho, z, [t.radius, bottom], [0.0, bottom]);
        Some(cone.min(wall).min(floor))
    }

    /// Whether ball `id` is in the model (not removed by the rejection rule).
    /// Balls outside the lattice's range count as present.
    #[must_use]
    pub fn is_present(&self, id: BallId) -> bool {
        match self.slot(id) {
            Some((true, i)) => self.a_present[i],
            Some((false, i)) => self.b_present[i],
            None => true,
        }
    }

    /// Presence mask of tile `(a, b, level)`: bit `i` set when the ball at
    /// `BallSite::ALL[i]` is present. Selects the tile's universe variant
    /// together with [`Self::tile_mask`].
    #[must_use]
    pub fn tile_present_mask(&self, a: i32, b: i32, level: i32) -> u8 {
        self.tile_balls(a, b, level)
            .iter()
            .enumerate()
            .fold(0u8, |m, (i, id)| m | (u8::from(self.is_present(*id)) << i))
    }

    /// Li (2014)'s rule, taken from Şeker & Çolak (2003) p.267 (*"Balls
    /// intersect with cone or discharge tube surface are rejected"*): remove
    /// every ball that crosses the cone, the tube wall or the tube bottom. Runs before [`Self::assign`], so a removed ball is
    /// never counted in the fuel split.
    fn reject(&mut self) {
        if self.tube.is_none() {
            return;
        }
        let r = 0.5 * self.cell.ball_diameter;
        let mut n = 0;
        for id in self.all_balls() {
            let d = self
                .container_boundary_distance(self.centre(id))
                .expect("tube is Some");
            if d < r {
                n += 1;
                match self.slot(id) {
                    Some((true, i)) => self.a_present[i] = false,
                    Some((false, i)) => self.b_present[i] = false,
                    None => unreachable!("every enumerated ball has a slot"),
                }
            }
        }
        self.rejected_balls = n;
    }

    /// **Side-wall rejection (gh:#331 ablation, 2026-09-27).** Also remove
    /// every ball in the bed cylinder that crosses its side wall
    /// (`rho + R > bed_radius`, centre above the bed floor), then re-draw the
    /// 57:43 fuel assignment over the balls that remain, exactly as the tube
    /// rejection does. The default keeps the CSG cut at the wall; Li, Yu & Wei
    /// (2014) say the array's outer boundary is the reflector's inner surface
    /// and reject balls at the cone and tube, but are silent on the side wall.
    /// This is the other reading, built so the two can be priced against each
    /// other. It is not a claim that it is the right one. **Decided against
    /// 2026-09-27 (maintainer, gh:#331):** it drops the bed's filling fraction
    /// to 0.5737 against Li's stated 61 % (the cut gives 0.6089), so the cut
    /// stays the default and this stays an ablation only.
    ///
    /// **New source evidence, 2026-10-01 (for the maintainer to re-decide on
    /// gh:#331; the default is unchanged):** Li's silence is filled by the model
    /// Li follows. Şeker & Çolak (2003) p.267: *"If any ball intersects with the
    /// reflector surface, it is rejected."* And their 61 % is measured **after**
    /// that rejection (Table 3: 1346 balls per 9.798 cm layer over the whole
    /// r = 90 cm cylinder is 0.6106). So rejection and 61 % are compatible in the
    /// source. They are incompatible here only because this lattice is diluted
    /// uniformly to 0.61 *before* rejecting (gh:#429).
    #[must_use]
    pub fn rejecting_side_wall_crossers(mut self) -> Self {
        let r = 0.5 * self.cell.ball_diameter;
        let mut n = 0;
        for id in self.all_balls() {
            let [x, y, z] = self.centre(id);
            let rho = x.hypot(y);
            let crosses = rho + r > self.bed_radius && rho - r < self.bed_radius;
            if z >= self.bed_bottom && z - r < self.bed_top && crosses && self.is_present(id) {
                n += 1;
                match self.slot(id) {
                    Some((true, i)) => self.a_present[i] = false,
                    Some((false, i)) => self.b_present[i] = false,
                    None => {}
                }
            }
        }
        self.rejected_balls += n;
        self.side_wall_rejected = n;
        self.a_fuel.iter_mut().for_each(|f| *f = false);
        self.b_fuel.iter_mut().for_each(|f| *f = false);
        self.assign();
        self
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
            if ok && self.is_present(id) {
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

// ---------------------------------------------------------------------------
// ŞEKER & ÇOLAK (2003)'S 13-BALL PRISM CELL (gh:#429, gh:#472)
// ---------------------------------------------------------------------------

/// **Şeker & Çolak (2003)'s hexagonal-prism unit cell**, the default HTR-10 bed
/// cell since 2026-10-01 (gh:#472).
///
/// NEW WORK, not a port. The cell is reconstructed from the source's text and
/// Fig. 3; no dimension is fitted. Source: Şeker, V., Çolak, Ü. (2003), *HTR-10
/// full core first criticality analysis with MCNP*, Nucl. Eng. Des. 222,
/// 263–270, doi:10.1016/S0029-5493(03)00031-1, pp. 266–267. Li, Yu & Wei
/// (2014) reproduce its text verbatim.
///
/// # What the source states
///
/// > *"The height of a layer is 9.798 cm. Top and bottom planes of hexagonal
/// > prisms are flat and contain half spheres. There are seven balls at these
/// > faces; one at the center of the basal plane and six surrounding spheres.
/// > These six spheres are not centered at the corners of the hexagons, but
/// > rather, hexagonal prism side surfaces surround these balls. The
/// > intermediate section of each hexagonal prism contains three full balls as
/// > well as partial contributions from the neighboring hexagonal prism cells
/// > from all six sides."*
///
/// # What Fig. 3 adds (read at 600 dpi, 2026-10-01, the 6 cm ball as scale)
///
/// Both panels show the same flat-topped hexagon (vertices at 0°, 60°, …; the
/// `HexOrientation::Y` convention), with circumradius ≈ 9.3–9.5 cm.
/// - **Basal plane:** a flower of 7 **touching** balls. The six outer balls
///   point at the vertices and are tangent to the two side faces beside
///   them. Neighbouring flowers touch.
/// - **Central plane:** a touching 6-ball triangle:
///   - 3 balls at `d/√3` (90°, 210°, 330°), in the flower's hollows;
///   - 3 at `2d/√3` (270°, 30°, 150°), each crossing a side face into the
///     neighbouring cell;
///   - through the other three faces, the neighbours' corner balls enter.
///
/// # What follows, with no free parameter
///
/// - apothem = `d cos 30° + d/2` = **8.196 cm** (3√3 + 3), pitch (flat to
///   flat) **16.392 cm**;
/// - height = two close-packed layer spacings, `2 d sqrt(2/3)` = 9.798 cm, which
///   is the stated layer;
/// - every contact is exactly `d`: the central balls sit `d/√3` from their
///   three supporting balls laterally and `d sqrt(2/3)` vertically;
/// - **13 balls per cell** (7 basal + 6 central), filling fraction **0.6448**,
///   areal fractions through the ball centres **0.851 basal / 0.729 central**.
///
/// The 0.6448 is the interior value. Şeker's *"61%"* is measured after the wall
/// rejection (see [`SekerBed`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SekerCell {
    /// Hexagon apothem (centre to side face) \[cm\].
    pub apothem: f64,
    /// Cell height \[cm\]: one basal-to-basal layer.
    pub height: f64,
    /// Ball diameter \[cm\].
    pub ball_diameter: f64,
}

/// One of the 23 ball sites a Şeker tile holds a piece of. See [`SekerCell`].
///
/// | site | tile-local centre | shared with |
/// |---|---|---|
/// | `BottomFlower(k)` | flower ball `k` at `z = -h/2` | the tile below (half each) |
/// | `TopFlower(k)` | flower ball `k` at `z = +h/2` | the tile above (half each) |
/// | `Inner(k)` | `d/√3` at `90° + 120° k`, `z = 0` | nobody: wholly inside |
/// | `Outer(k)` | `2d/√3` at `270° + 120° k`, `z = 0` | the neighbour it crosses into |
/// | `Entering(k)` | `pitch - 2d/√3` at `90° + 120° k`, `z = 0` | the neighbour at `90° + 120° k`, whose `Outer(k)` it is |
///
/// Flower ball `k = 0` is the centre; `k = 1..=6` lie at `d` and angle
/// `60° (k - 1)`, pointing at the vertices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SekerSite {
    /// Basal flower ball `k` (0..7) on the tile's bottom face.
    BottomFlower(u8),
    /// Basal flower ball `k` (0..7) on the tile's top face.
    TopFlower(u8),
    /// Central-plane ball `k` (0..3) in the flower's hollows.
    Inner(u8),
    /// Central-plane corner ball `k` (0..3) of this tile's triangle.
    Outer(u8),
    /// A neighbour's central corner ball `k` (0..3) entering this tile.
    Entering(u8),
}

impl SekerSite {
    /// The 23 sites, in the bit order of [`SekerBed::tile_masks`].
    pub const ALL: [SekerSite; 23] = [
        SekerSite::BottomFlower(0),
        SekerSite::BottomFlower(1),
        SekerSite::BottomFlower(2),
        SekerSite::BottomFlower(3),
        SekerSite::BottomFlower(4),
        SekerSite::BottomFlower(5),
        SekerSite::BottomFlower(6),
        SekerSite::TopFlower(0),
        SekerSite::TopFlower(1),
        SekerSite::TopFlower(2),
        SekerSite::TopFlower(3),
        SekerSite::TopFlower(4),
        SekerSite::TopFlower(5),
        SekerSite::TopFlower(6),
        SekerSite::Inner(0),
        SekerSite::Inner(1),
        SekerSite::Inner(2),
        SekerSite::Outer(0),
        SekerSite::Outer(1),
        SekerSite::Outer(2),
        SekerSite::Entering(0),
        SekerSite::Entering(1),
        SekerSite::Entering(2),
    ];
}

impl SekerCell {
    /// The cell from Şeker & Çolak (2003) text and Fig. 3, for 6 cm balls. See
    /// the type docs; nothing here is fitted.
    #[must_use]
    pub fn from_paper() -> Self {
        let d = 6.0;
        Self {
            apothem: d * (PI / 6.0).cos() + 0.5 * d,
            height: 2.0 * close_packed_layer_spacing(d),
            ball_diameter: d,
        }
    }

    /// Flat-to-flat pitch \[cm\], the lattice pitch: `2 * apothem`.
    #[must_use]
    pub fn pitch(&self) -> f64 {
        2.0 * self.apothem
    }

    /// Balls per cell: 7 basal (2 x 7 halves) + 6 central (3 whole, 3 x 2
    /// pieces shared with neighbours) = 13.
    #[must_use]
    pub fn balls_per_cell(&self) -> f64 {
        13.0
    }

    /// Hexagon area \[cm^2\], `2 sqrt(3) apothem^2`.
    #[must_use]
    pub fn area(&self) -> f64 {
        2.0 * 3.0_f64.sqrt() * self.apothem * self.apothem
    }

    /// Ball volume fraction of the (interior) cell \[-\]: 0.6448.
    #[must_use]
    pub fn packing_fraction(&self) -> f64 {
        let r = 0.5 * self.ball_diameter;
        self.balls_per_cell() * (4.0 / 3.0 * PI * r.powi(3)) / (self.area() * self.height)
    }

    /// Tile-local centre \[cm\] of `site`. See [`SekerSite`].
    #[must_use]
    pub fn site_centre(&self, site: SekerSite) -> [f64; 3] {
        let d = self.ball_diameter;
        let h = self.height;
        let polar = |rho: f64, deg: f64, z: f64| {
            let t = deg.to_radians();
            [rho * t.cos(), rho * t.sin(), z]
        };
        let flower = |k: u8, z: f64| {
            if k == 0 {
                [0.0, 0.0, z]
            } else {
                polar(d, 60.0 * f64::from(k - 1), z)
            }
        };
        let s3 = 3.0_f64.sqrt();
        match site {
            SekerSite::BottomFlower(k) => flower(k, -0.5 * h),
            SekerSite::TopFlower(k) => flower(k, 0.5 * h),
            SekerSite::Inner(k) => polar(d / s3, 90.0 + 120.0 * f64::from(k), 0.0),
            SekerSite::Outer(k) => polar(2.0 * d / s3, 270.0 + 120.0 * f64::from(k), 0.0),
            SekerSite::Entering(k) => {
                polar(self.pitch() - 2.0 * d / s3, 90.0 + 120.0 * f64::from(k), 0.0)
            }
        }
    }
}

/// A ball of the Şeker bed, named by the tile that owns it. Every piece of one
/// ball, in every tile that holds a piece of it, resolves to the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SekerBallId {
    /// Basal flower ball `k` of column `(a, b)` on lattice face `face` (the
    /// bottom face of level `face`).
    Flower {
        /// Skewed hex coordinate.
        a: i32,
        /// Skewed hex coordinate.
        b: i32,
        /// Lattice face index.
        face: i32,
        /// Flower ball, 0..7.
        k: u8,
    },
    /// Central-plane ball `k` of tile `(a, b, level)`: 0..3 `Inner`, 3..6
    /// `Outer`.
    Central {
        /// Skewed hex coordinate.
        a: i32,
        /// Skewed hex coordinate.
        b: i32,
        /// Lattice level.
        level: i32,
        /// 0..6.
        k: u8,
    },
}

/// The ball at `site` of Şeker tile `(a, b, level)`.
///
/// `Entering(k)` is the `Outer(k)` ball of the neighbour at `90° + 120° k`:
/// in the `HexOrientation::Y` skewed coordinates of [`tile_xy`] those are
/// `(a, b+1)`, `(a-1, b)` and `(a+1, b-1)`. Checked numerically by
/// `every_seker_tile_resolves_a_shared_ball_to_one_position`.
#[must_use]
pub fn seker_tile_ball(a: i32, b: i32, level: i32, site: SekerSite) -> SekerBallId {
    match site {
        SekerSite::BottomFlower(k) => SekerBallId::Flower { a, b, face: level, k },
        SekerSite::TopFlower(k) => SekerBallId::Flower {
            a,
            b,
            face: level + 1,
            k,
        },
        SekerSite::Inner(k) => SekerBallId::Central { a, b, level, k },
        SekerSite::Outer(k) => SekerBallId::Central {
            a,
            b,
            level,
            k: 3 + k,
        },
        SekerSite::Entering(k) => {
            let (da, db) = [(0, 1), (-1, 0), (1, -1)][usize::from(k)];
            SekerBallId::Central {
                a: a + da,
                b: b + db,
                level,
                k: 3 + k,
            }
        }
    }
}

/// **The HTR-10 bed as Şeker & Çolak (2003) build it** (gh:#472): the
/// [`SekerCell`] lattice, every ball whole, rejected wherever it would cross a
/// boundary, and a fuel/dummy identity per ball.
///
/// NEW WORK, not a port.
///
/// # Axial layout: the loading height is Şeker's
///
/// A loading of `n_layers` = N holds basal planes `0..=N` and central planes
/// `0..N`. The lowest basal plane's balls sit **on** the bed floor (the top of
/// the conus) and the highest's top is the bed top. So the bed is
/// `9.798 N + 6.0` cm tall, **exactly Şeker's and Li's tabulated height**, and
/// the top and bottom layers are whole balls, as Şeker p.267 says (*"The top
/// layer is formed by adding half spheres to each ball present in this
/// layer"*). Şeker's Table 3 count (`1346 N + 733`) is one extra basal plane,
/// which is this layout.
///
/// Below the bed floor the same lattice continues through the conus and the
/// discharge tube. Şeker p.267: *"The cone region and discharge tube are
/// formed by only graphite balls ... also made by balls arranged in hexagonal
/// geometry"*.
///
/// # Rejection: every kept ball is whole
///
/// Şeker p.267: *"If any ball intersects with the reflector surface, it is
/// rejected"*, and *"Balls intersect with cone or discharge tube surface are
/// rejected."* A ball is kept only if it lies wholly inside the container, i.e.
/// the solid of revolution of the meridional profile:
/// - the bed top (`z = bed_top`, `rho < bed_radius`);
/// - the side wall;
/// - the cone from `(bed_radius, bed_bottom)` to `(tube_radius, conus_floor)`;
/// - the tube wall and the tube bottom.
///
/// That includes the **side wall** (gh:#331, decided by the maintainer
/// 2026-10-01), where the two-ball bed cuts.
///
/// # Fuel/dummy identity
///
/// As [`TwoBallBed`]:
/// - the balls centred above the bed floor take the 57:43 split, by the
///   low-discrepancy rule in layer order from the floor up (Şeker p.267:
///   *"Fuel and moderator balls are selected in each layer such that 0.57:0.43
///   ratio is established"*);
/// - the conus and the tube are all dummy.
///
/// # Checked against Şeker's own counts
///
/// `the_seker_bed_reproduces_table_3_per_plane_counts` compares the kept balls
/// per basal and per central plane with Şeker Table 3 (733 and 613).
#[derive(Debug, Clone)]
pub struct SekerBed {
    /// The unit cell.
    pub cell: SekerCell,
    /// Hex rings in the lattice (including the central tile).
    pub n_rings: usize,
    /// Loading in Şeker layers N: bed height `cell.height * N + ball_diameter`.
    pub n_layers: usize,
    /// Axial lattice levels.
    pub n_levels: usize,
    /// z \[cm\] of the bottom face of lattice level 0.
    pub z_bottom: f64,
    /// Lattice face index of the lowest bed basal plane (the one on the floor).
    pub floor_face: i32,
    /// Bed cylinder radius \[cm\].
    pub bed_radius: f64,
    /// Bed floor \[cm\] (= top of the conus).
    pub bed_bottom: f64,
    /// Bed top \[cm\] (= top of the highest ball).
    pub bed_top: f64,
    /// Conus floor \[cm\].
    pub conus_floor: f64,
    /// Discharge-tube radius \[cm\].
    pub tube_radius: f64,
    /// Bottom of the container \[cm\]: the tube bottom, or the conus floor when
    /// the tube is not built (the `OUTRAM_HTR10_HOMOG_TUBE` ablation).
    pub container_bottom: f64,
    /// The rule the identities were assigned by.
    pub assignment: FuelAssignment,
    /// Balls that took part in the 57:43 split.
    pub eligible_balls: usize,
    /// Of which fuelled.
    pub fuel_balls: usize,
    /// Lattice balls rejected (not wholly inside the container).
    pub rejected_balls: usize,
    /// Of [`Self::rejected_balls`], those whose centre is inside the container
    /// but which cross its boundary: the balls Şeker's rule actually removes.
    pub boundary_rejected: usize,
    flower_fuel: Vec<bool>,
    central_fuel: Vec<bool>,
    flower_present: Vec<bool>,
    central_present: Vec<bool>,
}

impl SekerBed {
    /// Build the bed.
    ///
    /// # Parameters
    /// - `cell` — normally [`SekerCell::from_paper`].
    /// - `n_rings` — hex rings of the lattice; must tile `bed_radius`.
    /// - `n_layers` — Şeker layers N; the bed is `cell.height * N +
    ///   ball_diameter` tall, centred on z = 0.
    /// - `conus_height` — depth \[cm\] of the conus below the bed floor.
    /// - `bed_radius`, `tube_radius` — \[cm\].
    /// - `tube_depth` — depth \[cm\] of the discharge tube below the conus
    ///   floor, or `None` to end the container at the conus floor.
    /// - `assignment` — see [`FuelAssignment`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cell: SekerCell,
        n_rings: usize,
        n_layers: usize,
        conus_height: f64,
        bed_radius: f64,
        tube_radius: f64,
        tube_depth: Option<f64>,
        assignment: FuelAssignment,
    ) -> Self {
        let h = cell.height;
        let r = 0.5 * cell.ball_diameter;
        let bed_top = 0.5 * (h * n_layers as f64 + cell.ball_diameter);
        let bed_bottom = -bed_top;
        let conus_floor = bed_bottom - conus_height;
        let container_bottom = conus_floor - tube_depth.unwrap_or(0.0);
        // Basal planes at bed_bottom + r + m h, m = 0..=N in the bed. One
        // margin level below the container bottom and one above the top plane.
        let face0 = bed_bottom + r;
        let m_lo = ((container_bottom - face0) / h).floor() as i32 - 1;
        let m_hi = n_layers as i32 + 2;
        let n_levels = (m_hi - m_lo) as usize;
        let w = 2 * n_rings + 3;
        let mut bed = Self {
            cell,
            n_rings,
            n_layers,
            n_levels,
            z_bottom: face0 + f64::from(m_lo) * h,
            floor_face: -m_lo,
            bed_radius,
            bed_bottom,
            bed_top,
            conus_floor,
            tube_radius,
            container_bottom,
            assignment,
            eligible_balls: 0,
            fuel_balls: 0,
            rejected_balls: 0,
            boundary_rejected: 0,
            flower_fuel: vec![false; w * w * (n_levels + 1) * 7],
            central_fuel: vec![false; w * w * n_levels * 6],
            flower_present: vec![true; w * w * (n_levels + 1) * 7],
            central_present: vec![true; w * w * n_levels * 6],
        };
        bed.reject();
        bed.assign();
        bed
    }

    /// z \[cm\] of the lattice centre, to pass to `HexLattice::from_rings_3d`.
    #[must_use]
    pub fn lattice_centre_z(&self) -> f64 {
        self.z_bottom + 0.5 * self.n_levels as f64 * self.cell.height
    }

    fn slot(&self, id: SekerBallId) -> Option<(bool, usize)> {
        // The slot range reaches one ring beyond the lattice, where the owners
        // of entering balls can sit.
        let nr = self.n_rings as i32 + 1;
        let w = (2 * nr + 1) as usize;
        let (a, b, k_ax, is_flower, n_ax, k, per) = match id {
            SekerBallId::Flower { a, b, face, k } => (a, b, face, true, self.n_levels + 1, k, 7),
            SekerBallId::Central { a, b, level, k } => (a, b, level, false, self.n_levels, k, 6),
        };
        if a < -nr || a > nr || b < -nr || b > nr || k_ax < 0 || k_ax as usize >= n_ax {
            return None;
        }
        let col = ((k_ax as usize) * w + (b + nr) as usize) * w + (a + nr) as usize;
        Some((is_flower, col * per + usize::from(k)))
    }

    /// Global centre \[cm\] of ball `id`.
    #[must_use]
    pub fn centre(&self, id: SekerBallId) -> [f64; 3] {
        let (a, b, zc, site) = match id {
            SekerBallId::Flower { a, b, face, k } => (
                a,
                b,
                self.z_bottom + f64::from(face) * self.cell.height,
                SekerSite::BottomFlower(k),
            ),
            SekerBallId::Central { a, b, level, k } => (
                a,
                b,
                self.z_bottom + (f64::from(level) + 0.5) * self.cell.height,
                if k < 3 {
                    SekerSite::Inner(k)
                } else {
                    SekerSite::Outer(k - 3)
                },
            ),
        };
        let [tx, ty] = tile_xy(a, b, self.cell.pitch());
        let [lx, ly, _] = self.cell.site_centre(site);
        [tx + lx, ty + ly, zc]
    }

    /// Whether ball `id` is kept. Balls outside the lattice's range are absent.
    #[must_use]
    pub fn is_present(&self, id: SekerBallId) -> bool {
        match self.slot(id) {
            Some((true, i)) => self.flower_present[i],
            Some((false, i)) => self.central_present[i],
            None => false,
        }
    }

    /// Whether ball `id` is fuelled.
    #[must_use]
    pub fn is_fuel(&self, id: SekerBallId) -> bool {
        match self.slot(id) {
            Some((true, i)) => self.flower_fuel[i],
            Some((false, i)) => self.central_fuel[i],
            None => false,
        }
    }

    /// The 23 balls tile `(a, b, level)` holds pieces of, in
    /// [`SekerSite::ALL`] order.
    #[must_use]
    pub fn tile_balls(&self, a: i32, b: i32, level: i32) -> [SekerBallId; 23] {
        SekerSite::ALL.map(|s| seker_tile_ball(a, b, level, s))
    }

    /// `(fuel, present)` masks of tile `(a, b, level)`: bit `i` for the ball at
    /// `SekerSite::ALL[i]`. Selects the tile's universe.
    #[must_use]
    pub fn tile_masks(&self, a: i32, b: i32, level: i32) -> (u32, u32) {
        self.tile_balls(a, b, level)
            .iter()
            .enumerate()
            .fold((0, 0), |(f, p), (i, id)| {
                (
                    f | (u32::from(self.is_fuel(*id)) << i),
                    p | (u32::from(self.is_present(*id)) << i),
                )
            })
    }

    /// Every ball owned by a tile within the lattice's rings, deterministic
    /// order (flowers by face, then centrals by level).
    #[must_use]
    pub fn all_balls(&self) -> Vec<SekerBallId> {
        let nr = self.n_rings as i32;
        let mut cols = Vec::new();
        for a in -(nr - 1)..=(nr - 1) {
            for b in -(nr - 1)..=(nr - 1) {
                if hex_ring(a, b) <= self.n_rings - 1 {
                    cols.push((a, b));
                }
            }
        }
        let mut out = Vec::new();
        for face in 0..=self.n_levels as i32 {
            for &(a, b) in &cols {
                out.extend((0..7).map(|k| SekerBallId::Flower { a, b, face, k }));
            }
        }
        for level in 0..self.n_levels as i32 {
            for &(a, b) in &cols {
                out.extend((0..6).map(|k| SekerBallId::Central { a, b, level, k }));
            }
        }
        out
    }

    /// The container's meridional profile, a closed polyline `(rho, z)` from the
    /// axis at the top round to the axis at the bottom.
    fn profile(&self) -> Vec<[f64; 2]> {
        let mut p = vec![
            [0.0, self.bed_top],
            [self.bed_radius, self.bed_top],
            [self.bed_radius, self.bed_bottom],
            [self.tube_radius, self.conus_floor],
        ];
        if self.container_bottom < self.conus_floor {
            p.push([self.tube_radius, self.container_bottom]);
        }
        p.push([0.0, self.container_bottom]);
        p
    }

    /// Whether the ball centred at `c` lies wholly inside the container, and
    /// whether its centre does. A sphere lies inside a solid of revolution
    /// exactly when its meridional disk lies inside the profile (the argument in
    /// [`TwoBallBed::container_boundary_distance`]).
    fn containment(&self, c: [f64; 3]) -> (bool, bool) {
        let (rho, z) = (c[0].hypot(c[1]), c[2]);
        let p = self.profile();
        // The profile, closed along the axis, is a simple polygon.
        let mut inside = false;
        let mut dmin = f64::INFINITY;
        for i in 0..p.len() {
            let (u, v) = (p[i], p[(i + 1) % p.len()]);
            if (u[1] > z) != (v[1] > z) && rho < u[0] + (z - u[1]) / (v[1] - u[1]) * (v[0] - u[0]) {
                inside = !inside;
            }
            // The closing segment is the axis, which is not a boundary.
            if i + 1 < p.len() {
                dmin = dmin.min(segment_distance(rho, z, u, v));
            }
        }
        let r = 0.5 * self.cell.ball_diameter;
        (inside && dmin >= r - 1e-9, inside)
    }

    fn reject(&mut self) {
        let mut n = 0;
        let mut nb = 0;
        for id in self.all_balls() {
            let (whole, centre_in) = self.containment(self.centre(id));
            if !whole {
                n += 1;
                nb += usize::from(centre_in);
                match self.slot(id) {
                    Some((true, i)) => self.flower_present[i] = false,
                    Some((false, i)) => self.central_present[i] = false,
                    None => unreachable!("every enumerated ball has a slot"),
                }
            }
        }
        // Balls owned by tiles one ring out (entering pieces) are outside
        // `all_balls`; they lie beyond the bed cylinder, so reject them too.
        let nr = self.n_rings as i32 + 1;
        for level in 0..self.n_levels as i32 {
            for a in -nr..=nr {
                for b in -nr..=nr {
                    if hex_ring(a, b) != self.n_rings {
                        continue;
                    }
                    for k in 3..6 {
                        let id = SekerBallId::Central { a, b, level, k };
                        if !self.containment(self.centre(id)).0 {
                            if let Some((false, i)) = self.slot(id) {
                                self.central_present[i] = false;
                            }
                        }
                    }
                }
            }
        }
        self.rejected_balls = n;
        self.boundary_rejected = nb;
    }

    /// Half-layer index of a ball from lattice face 0 (basal even, central odd).
    fn layer(id: SekerBallId) -> i32 {
        match id {
            SekerBallId::Flower { face, .. } => 2 * face,
            SekerBallId::Central { level, .. } => 2 * level + 1,
        }
    }

    fn assign(&mut self) {
        let f = super::table1::FUEL_BALL_FRACTION;
        let mut eligible: Vec<(i32, f64, f64, SekerBallId)> = Vec::new();
        for id in self.all_balls() {
            let [x, y, z] = self.centre(id);
            let ok = match self.assignment {
                FuelAssignment::AllFuel => true,
                FuelAssignment::Paper => z > self.bed_bottom,
                FuelAssignment::FuelledConus => true,
            };
            if ok && self.is_present(id) {
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
                    Some((true, i)) => self.flower_fuel[i] = true,
                    Some((false, i)) => self.central_fuel[i] = true,
                    None => unreachable!("every enumerated ball has a slot"),
                }
            }
        }
    }

    /// Kept balls in the bed (centre above the floor) per plane: `(basal
    /// counts for bed planes 0..=N, central counts for planes 0..N)`. Compare
    /// with Şeker Table 3: 733 per basal and 613 per central plane.
    #[must_use]
    pub fn plane_counts(&self) -> (Vec<usize>, Vec<usize>) {
        let n = self.n_layers;
        let mut basal = vec![0usize; n + 1];
        let mut central = vec![0usize; n];
        for id in self.all_balls() {
            if !self.is_present(id) {
                continue;
            }
            match id {
                SekerBallId::Flower { face, .. } => {
                    let j = face - self.floor_face;
                    if (0..=n as i32).contains(&j) {
                        basal[j as usize] += 1;
                    }
                }
                SekerBallId::Central { level, .. } => {
                    let j = level - self.floor_face;
                    if (0..n as i32).contains(&j) {
                        central[j as usize] += 1;
                    }
                }
            }
        }
        (basal, central)
    }
}

/// **The pebble bed an explicit-TRISO core was built on**: Şeker & Çolak's
/// cell (the default since 2026-10-01, gh:#472) or the two-ball prism cell
/// (~~the `OUTRAM_HTR10_TWO_BALL_CELL` ablation~~ **withdrawn 2026-10-01**: it
/// cuts pebbles, which is wrong physics, and must never be run, not even as an
/// ablation; kept only as the record of earlier numbers).
#[derive(Debug, Clone)]
pub enum PebbleBed {
    /// [`SekerBed`], the default.
    Seker(SekerBed),
    /// [`TwoBallBed`], an ablation.
    TwoBall(TwoBallBed),
}

/// A ball of a [`PebbleBed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BedBall {
    /// A ball of a [`SekerBed`].
    Seker(SekerBallId),
    /// A ball of a [`TwoBallBed`].
    TwoBall(BallId),
}

impl PebbleBed {
    /// Bed floor \[cm\] (= top of the conus).
    #[must_use]
    pub fn bed_bottom(&self) -> f64 {
        match self {
            Self::Seker(b) => b.bed_bottom,
            Self::TwoBall(b) => b.bed_bottom,
        }
    }

    /// Bed top \[cm\].
    #[must_use]
    pub fn bed_top(&self) -> f64 {
        match self {
            Self::Seker(b) => b.bed_top,
            Self::TwoBall(b) => b.bed_top,
        }
    }

    /// Conus floor \[cm\].
    #[must_use]
    pub fn conus_floor(&self) -> f64 {
        match self {
            Self::Seker(b) => b.conus_floor,
            Self::TwoBall(b) => b.conus_floor,
        }
    }

    /// Bed cylinder radius \[cm\].
    #[must_use]
    pub fn bed_radius(&self) -> f64 {
        match self {
            Self::Seker(b) => b.bed_radius,
            Self::TwoBall(b) => b.bed_radius,
        }
    }

    /// Hex rings of the lattice.
    #[must_use]
    pub fn n_rings(&self) -> usize {
        match self {
            Self::Seker(b) => b.n_rings,
            Self::TwoBall(b) => b.n_rings,
        }
    }

    /// Axial lattice levels.
    #[must_use]
    pub fn n_levels(&self) -> usize {
        match self {
            Self::Seker(b) => b.n_levels,
            Self::TwoBall(b) => b.n_levels,
        }
    }

    /// z \[cm\] of the bottom face of lattice level 0.
    #[must_use]
    pub fn z_bottom(&self) -> f64 {
        match self {
            Self::Seker(b) => b.z_bottom,
            Self::TwoBall(b) => b.z_bottom,
        }
    }

    /// z \[cm\] of the lattice centre.
    #[must_use]
    pub fn lattice_centre_z(&self) -> f64 {
        match self {
            Self::Seker(b) => b.lattice_centre_z(),
            Self::TwoBall(b) => b.lattice_centre_z(),
        }
    }

    /// Lattice pitch (flat to flat) and tile height \[cm\].
    #[must_use]
    pub fn tile_pitch_and_height(&self) -> (f64, f64) {
        match self {
            Self::Seker(b) => (b.cell.pitch(), b.cell.height),
            Self::TwoBall(b) => (b.cell.pitch, b.cell.height),
        }
    }

    /// Ball diameter \[cm\].
    #[must_use]
    pub fn ball_diameter(&self) -> f64 {
        match self {
            Self::Seker(b) => b.cell.ball_diameter,
            Self::TwoBall(b) => b.cell.ball_diameter,
        }
    }

    /// Tile-local centres \[cm\] of the ball sites, in tile-mask bit order.
    #[must_use]
    pub fn site_centres(&self) -> Vec<[f64; 3]> {
        match self {
            Self::Seker(b) => SekerSite::ALL.iter().map(|&s| b.cell.site_centre(s)).collect(),
            Self::TwoBall(b) => BallSite::ALL.iter().map(|&s| b.cell.site_centre(s)).collect(),
        }
    }

    /// `(fuel, present)` masks of tile `(a, b, level)`, bit `i` for site `i` of
    /// [`Self::site_centres`].
    #[must_use]
    pub fn tile_masks(&self, a: i32, b: i32, level: i32) -> (u32, u32) {
        match self {
            Self::Seker(s) => s.tile_masks(a, b, level),
            Self::TwoBall(t) => (
                u32::from(t.tile_mask(a, b, level)),
                u32::from(t.tile_present_mask(a, b, level)),
            ),
        }
    }

    /// Every ball of the lattice.
    #[must_use]
    pub fn all_balls(&self) -> Vec<BedBall> {
        match self {
            Self::Seker(b) => b.all_balls().into_iter().map(BedBall::Seker).collect(),
            Self::TwoBall(b) => b.all_balls().into_iter().map(BedBall::TwoBall).collect(),
        }
    }

    /// Global centre \[cm\] of `ball`.
    ///
    /// # Panics
    /// If `ball` belongs to the other kind of bed.
    #[must_use]
    pub fn centre(&self, ball: BedBall) -> [f64; 3] {
        match (self, ball) {
            (Self::Seker(b), BedBall::Seker(id)) => b.centre(id),
            (Self::TwoBall(b), BedBall::TwoBall(id)) => b.centre(id),
            _ => panic!("a {ball:?} is not a ball of this bed"),
        }
    }

    /// Whether `ball` is fuelled (see [`Self::centre`] for the panic).
    #[must_use]
    pub fn is_fuel(&self, ball: BedBall) -> bool {
        match (self, ball) {
            (Self::Seker(b), BedBall::Seker(id)) => b.is_fuel(id),
            (Self::TwoBall(b), BedBall::TwoBall(id)) => b.is_fuel(id),
            _ => panic!("a {ball:?} is not a ball of this bed"),
        }
    }

    /// Whether `ball` is kept (see [`Self::centre`] for the panic).
    #[must_use]
    pub fn is_present(&self, ball: BedBall) -> bool {
        match (self, ball) {
            (Self::Seker(b), BedBall::Seker(id)) => b.is_present(id),
            (Self::TwoBall(b), BedBall::TwoBall(id)) => b.is_present(id),
            _ => panic!("a {ball:?} is not a ball of this bed"),
        }
    }

    /// Balls that took part in the 57:43 split, and of which fuelled.
    #[must_use]
    pub fn eligible_and_fuel_balls(&self) -> (usize, usize) {
        match self {
            Self::Seker(b) => (b.eligible_balls, b.fuel_balls),
            Self::TwoBall(b) => (b.eligible_balls, b.fuel_balls),
        }
    }

    /// The discharge tube below the conus as `(radius, bottom z)` \[cm\], or
    /// `None` when it is not built with balls (the `OUTRAM_HTR10_HOMOG_TUBE`
    /// ablation).
    #[must_use]
    pub fn tube_radius_and_bottom(&self) -> Option<(f64, f64)> {
        match self {
            Self::Seker(b) => (b.container_bottom < b.conus_floor)
                .then_some((b.tube_radius, b.container_bottom)),
            Self::TwoBall(b) => b.tube.map(|t| (t.radius, b.conus_floor - t.depth)),
        }
    }

    /// The bed's ball inventory: kept balls centred above the bed floor (the
    /// count Şeker's Table 3 tabulates). The reference is matched to a model
    /// by this, through [`super::rmc_keff_at_ball_count`]. `None` for the
    /// withdrawn two-ball bed.
    #[must_use]
    pub fn core_balls(&self) -> Option<usize> {
        match self {
            Self::Seker(b) => {
                let (basal, central) = b.plane_counts();
                Some(basal.iter().sum::<usize>() + central.iter().sum::<usize>())
            }
            Self::TwoBall(_) => None,
        }
    }

    /// Balls removed by a rejection rule.
    #[must_use]
    pub fn rejected_balls(&self) -> usize {
        match self {
            Self::Seker(b) => b.boundary_rejected,
            Self::TwoBall(b) => b.rejected_balls,
        }
    }
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

#[cfg(test)]
mod seker_cell_tests {
    //! **V&V of Şeker & Çolak (2003)'s cell and bed (gh:#472).**
    //!
    //! Methodology: the cell is built from the source's text and Fig. 3 with no
    //! fitted dimension (see [`SekerCell`]). These tests check that it is a
    //! valid packing (no overlaps, the contacts the figure shows), that the tile
    //! bookkeeping is complete and consistent, and, the one comparison that can
    //! fail on the physics, that the kept balls per plane match Şeker Table 3.
    //! Results are in each test's doc comment.
    use super::super::core_model::{
        HTR10_BOTTOM_REFLECTOR_CM, HTR10_CONUS_HEIGHT_CM, HTR10_CORE_RADIUS_CM,
        HTR10_DISCHARGE_TUBE_RADIUS_CM,
    };
    use super::*;

    fn htr10_bed(n_layers: usize) -> SekerBed {
        let cell = SekerCell::from_paper();
        let reach = 0.5 * 3.0_f64.sqrt() * cell.pitch();
        let n_rings = (HTR10_CORE_RADIUS_CM / reach).ceil() as usize + 1;
        SekerBed::new(
            cell,
            n_rings,
            n_layers,
            HTR10_CONUS_HEIGHT_CM,
            HTR10_CORE_RADIUS_CM,
            HTR10_DISCHARGE_TUBE_RADIUS_CM,
            Some(HTR10_BOTTOM_REFLECTOR_CM),
            FuelAssignment::Paper,
        )
    }

    /// The derived dimensions. **Results (2026-10-01):** apothem 8.196152 cm,
    /// pitch 16.392305 cm, height 9.797959 cm (Şeker: 9.798), interior filling
    /// fraction 0.644834.
    #[test]
    fn the_seker_cell_has_the_derived_dimensions() {
        let c = SekerCell::from_paper();
        println!(
            "apothem {:.6}, pitch {:.6}, height {:.6}, packing {:.6}",
            c.apothem,
            c.pitch(),
            c.height,
            c.packing_fraction()
        );
        assert!((c.apothem - (3.0 * 3.0_f64.sqrt() + 3.0)).abs() < 1e-12);
        assert!((c.height - 9.798).abs() < 5e-4, "the stated layer is 9.798 cm");
        assert!((c.packing_fraction() - 0.6448).abs() < 1e-4);
    }

    /// **No two balls overlap, and the contacts Fig. 3 shows are there.** Every
    /// pair of lattice balls (present or not) in a small bed is at least one
    /// diameter apart. The number of pairs exactly in contact is printed.
    /// **Result (2026-10-01):** minimum centre distance 6.000000 cm.
    #[test]
    fn seker_balls_never_overlap_and_touch_where_the_figure_shows() {
        let cell = SekerCell::from_paper();
        let bed = SekerBed::new(cell, 3, 2, 5.0, 1.0e3, 900.0, None, FuelAssignment::Paper);
        let c: Vec<[f64; 3]> = bed
            .all_balls()
            .into_iter()
            .map(|id| bed.centre(id))
            .filter(|p| p[0].hypot(p[1]) < 20.0)
            .collect();
        let (mut dmin, mut contacts) = (f64::INFINITY, 0);
        for i in 0..c.len() {
            for j in i + 1..c.len() {
                let d = ((c[i][0] - c[j][0]).powi(2)
                    + (c[i][1] - c[j][1]).powi(2)
                    + (c[i][2] - c[j][2]).powi(2))
                .sqrt();
                dmin = dmin.min(d);
                contacts += usize::from((d - 6.0).abs() < 1e-9);
            }
        }
        println!("{} balls, min distance {dmin:.9} cm, {contacts} contacts", c.len());
        assert!(dmin > 6.0 - 1e-9, "balls overlap: {dmin}");
        assert!(contacts > c.len(), "the packing should be held by contacts");
    }

    /// Every piece of a shared ball resolves to the same global position (the
    /// `Entering` ownership rule is geometry, not bookkeeping).
    #[test]
    fn every_seker_tile_resolves_a_shared_ball_to_one_position() {
        let cell = SekerCell::from_paper();
        let bed = SekerBed::new(cell, 3, 2, 5.0, 40.0, 10.0, None, FuelAssignment::Paper);
        let mut checked = 0;
        for level in 0..bed.n_levels as i32 {
            for a in -2..=2 {
                for b in -2..=2 {
                    if hex_ring(a, b) > 2 {
                        continue;
                    }
                    let [tx, ty] = tile_xy(a, b, cell.pitch());
                    let tz = bed.z_bottom + (f64::from(level) + 0.5) * cell.height;
                    for s in SekerSite::ALL {
                        let l = cell.site_centre(s);
                        let g = bed.centre(seker_tile_ball(a, b, level, s));
                        let d = (tx + l[0] - g[0]).hypot(ty + l[1] - g[1]).hypot(tz + l[2] - g[2]);
                        assert!(d < 1e-9, "tile ({a},{b},{level}) site {s:?} is {d} cm off");
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 500);
    }

    /// Distance \[cm\] from a point to the hexagonal prism of the tile centred
    /// at the origin (`HexOrientation::Y`: vertices at 0°, 60°, ...).
    fn distance_to_tile(p: [f64; 3], cell: &SekerCell) -> f64 {
        let rv = cell.pitch() / 3.0_f64.sqrt();
        let v: Vec<[f64; 2]> = (0..6)
            .map(|i| {
                let t = (60.0 * f64::from(i)).to_radians();
                [rv * t.cos(), rv * t.sin()]
            })
            .collect();
        let inside = (0..6).all(|i| {
            let t = (30.0 + 60.0 * f64::from(i)).to_radians();
            p[0] * t.cos() + p[1] * t.sin() <= cell.apothem
        });
        let lateral = if inside {
            0.0
        } else {
            (0..6)
                .map(|i| segment_distance(p[0], p[1], v[i], v[(i + 1) % 6]))
                .fold(f64::INFINITY, f64::min)
        };
        let vertical = (p[2].abs() - 0.5 * cell.height).max(0.0);
        lateral.hypot(vertical)
    }

    /// **The 23 sites are complete**: every lattice ball that reaches into a
    /// tile's prism is one of that tile's 23 sites, and every site reaches in.
    /// A missing site would leave a piece of a ball as helium.
    #[test]
    fn the_23_sites_are_every_ball_that_reaches_into_a_tile() {
        let cell = SekerCell::from_paper();
        let bed = SekerBed::new(cell, 4, 3, 5.0, 1.0e3, 900.0, None, FuelAssignment::Paper);
        let level = 2;
        let tz = bed.z_bottom + (f64::from(level) + 0.5) * cell.height;
        let mine: std::collections::BTreeSet<SekerBallId> =
            bed.tile_balls(0, 0, level).into_iter().collect();
        assert_eq!(mine.len(), 23, "23 distinct balls");
        for id in bed.all_balls() {
            let [x, y, z] = bed.centre(id);
            let d = distance_to_tile([x, y, z - tz], &cell);
            let reaches = d < 3.0 - 1e-9;
            assert_eq!(reaches, mine.contains(&id), "{id:?} at {d:.6} cm from the tile");
        }
    }

    /// **The bed's height axis is Şeker's, by construction.** For every row of
    /// Li's (= Şeker's) table, N layers give a bed exactly as tall as the
    /// tabulated loading height.
    #[test]
    fn a_seker_bed_of_n_layers_is_as_tall_as_the_tabulated_height() {
        for (i, &(h, _)) in super::super::RMC_KEFF_VS_HEIGHT.iter().enumerate() {
            let n = 9 + i;
            let cell = SekerCell::from_paper();
            let built = cell.height * n as f64 + cell.ball_diameter;
            assert!((built - h).abs() < 2.5e-3, "N = {n}: {built:.4} cm against {h} cm");
        }
    }

    /// **Kept balls per plane against Şeker & Çolak (2003) Table 3**, the
    /// independent check (gh:#430, gh:#472).
    ///
    /// Methodology: the full HTR-10 bed (r = 90 cm, conus 36.946 cm, tube r = 25
    /// cm) at N = 12, every ball wholly inside the container kept (Şeker's
    /// rejection rule). Şeker's rows imply 733 balls per basal plane and 613 per
    /// central plane (total `1346 N + 733`).
    ///
    /// Pass criterion: set **before** building, from the prediction of a
    /// Python replica of this cell over 34 lattice offsets (2026-10-01, gh:#429:
    /// basal 710–721, central 609–619). Central within 1 % of 613 and basal
    /// within 4 % of 733. The criterion checks the cell, not a fit: nothing
    /// is tuned.
    ///
    /// **Results (2026-10-01), N = 12, lattice centred on the axis:**
    ///
    /// | quantity | this bed | Şeker Table 3 | diff |
    /// |---|---|---|---|
    /// | balls per basal plane | 721 (every plane) | 733 | −1.6 % |
    /// | balls per central plane | 609 (every plane) | 613 | −0.7 % |
    /// | total | 16 681 | 16 885 | −1.2 % |
    /// | filling over the 123.576 cm bed | 0.5999 | 0.6073 | −1.2 % |
    /// | fuel | 9508 of 16 681 | 9622 of 16 885 | |
    ///
    /// Inside the predicted ranges.
    ///
    /// **Explained 2026-10-01 (gh:#472):** the missing 12 + 4 balls per layer
    /// are the next shells out:
    /// - 12 basal balls centred at ρ = 87.209 cm, which cross the r = 90 cm
    ///   reflector by 0.21 cm;
    /// - 6 central balls at ρ = 87.080 cm, which cross it by 0.08 cm.
    ///
    /// Keeping them gives 733 (exact) and 615 (+2). Şeker therefore kept
    /// wall-crossing balls, cut in MCNP or kept by a tolerance. Cut pebbles
    /// are wrong physics, so they are not added. The model is compared to the
    /// reference at equal ball count instead ([`super::super::rmc_keff_at_ball_count`]).
    #[test]
    fn the_seker_bed_reproduces_table_3_per_plane_counts() {
        let bed = htr10_bed(12);
        let (basal, central) = bed.plane_counts();
        let mean = |v: &[usize]| v.iter().sum::<usize>() as f64 / v.len() as f64;
        let (mb, mc) = (mean(&basal), mean(&central));
        let total: usize = basal.iter().sum::<usize>() + central.iter().sum::<usize>();
        let v_ball = 4.0 / 3.0 * PI * 27.0;
        let height = bed.bed_top - bed.bed_bottom;
        let ff = total as f64 * v_ball / (PI * 90.0 * 90.0 * height);
        println!("basal per plane {basal:?}, mean {mb:.1} (Seker 733)");
        println!("central per plane {central:?}, mean {mc:.1} (Seker 613)");
        println!(
            "total {total} (Seker 1346 x 12 + 733 = 16885), bed {height:.3} cm, filling {ff:.4} \
             (Seker's 16885 balls in the same bed: 0.6073)"
        );
        println!("fuel {} of {} eligible", bed.fuel_balls, bed.eligible_balls);
        assert!((mc - 613.0).abs() / 613.0 < 0.01, "central {mc}");
        assert!((mb - 733.0).abs() / 733.0 < 0.04, "basal {mb}");
        // Every bed ball is eligible, and the split is 57:43 within one ball.
        assert_eq!(bed.eligible_balls, total);
        let f = super::super::table1::FUEL_BALL_FRACTION;
        assert!((bed.fuel_balls as f64 - f * total as f64).abs() <= 1.0);
    }

    /// Every kept ball lies wholly inside the container, re-checked here
    /// without the profile polygon (the bed cylinder above the floor; below
    /// it, inside the cone or the tube).
    #[test]
    fn every_kept_seker_ball_is_whole() {
        let bed = htr10_bed(10);
        let slope = (90.0 - 25.0) / HTR10_CONUS_HEIGHT_CM;
        let mut kept = 0;
        for id in bed.all_balls() {
            if !bed.is_present(id) {
                continue;
            }
            kept += 1;
            let [x, y, z] = bed.centre(id);
            let rho = x.hypot(y);
            assert!(z + 3.0 <= bed.bed_top + 1e-9, "{id:?} above the bed top");
            if z + 3.0 > bed.bed_bottom {
                assert!(rho + 3.0 <= 90.0 + 1e-9, "{id:?} crosses the side wall");
            }
            if z < bed.bed_bottom && z > bed.conus_floor {
                let r_cone = 25.0 + slope * (z - bed.conus_floor);
                assert!(rho < r_cone, "{id:?} centred outside the cone");
            }
            if z < bed.conus_floor {
                assert!(rho + 3.0 <= 25.0 + 1e-9, "{id:?} crosses the tube wall");
            }
        }
        assert!(kept > 15_000);
    }
}
