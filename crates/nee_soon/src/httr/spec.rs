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

//! # The HTTR start-up core benchmark specification, as numbers
//!
//! Every dimension here is the IRPhEP evaluation's: Bess, Fujimoto, Dolphin,
//! Snoj & Zukeran, *Evaluation of the Start-Up Core Physics Tests at Japan's
//! HTTR (Fully-Loaded Core)*, HTTR-GCR-RESR-001, INL/EXT-08-14767 Rev. 2
//! (March 2010), cited as **R1**. Page numbers are R1's printed pages.
//!
//! Where R1's §3 text and its sample MCNP5 input (App. A.1.8, pp. 235-246,
//! "the deck") give the same number, the deck's surface or cell card is
//! named as well. Where they disagree, this module follows the deck, says
//! so, and names the text it departs from: the deck is the model behind
//! every reference `k_eff` R1 tabulates, so following it keeps the
//! code-to-code comparison like for like. Each such case is listed in the
//! module docs of [`super`].
//!
//! All lengths are cm, in the frame the deck uses: `z = 0` at the core's
//! axial mid-plane, the core 522 cm tall, the fuel from `z = -145` to
//! `z = +145`.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// TRISO particle and compact (R1 §3.1.2.1, Fig. 3.1 p.192; deck surfaces 1-6,
// 12-13, 911-916)
// ---------------------------------------------------------------------------

/// Outer radii of the coated particle's six regions \[cm\]: kernel (600 µm
/// diameter), buffer (60 µm), IPyC (30 µm), SiC (25 µm), OPyC (45 µm) and
/// the resinated-graphite overcoat (200 µm). R1 Fig. 3.1; deck surfaces 1-6.
pub const TRISO_RADII_CM: [f64; 6] = [0.03, 0.036, 0.039, 0.0415, 0.046, 0.066];

/// The ordered TRISO lattice's pitch \[cm\]: x, y, z. R1 §4.1.1 (the
/// sample-calculation approach; R1 §3.1.1 specifies a random distribution);
/// deck surfaces 911-916.
///
/// The overcoat sphere (r = 0.066 cm) is larger than half the pitch, so each
/// tile clips it; the rest of the tile is compact matrix. That is what the
/// deck builds (cell 7 is the matrix beyond surface 6 inside the tile).
pub const TRISO_PITCH_CM: [f64; 3] = [0.106, 0.106, 0.1];

/// One 0.1 cm layer of the fuel compact as the deck tiles it (cell 163,
/// `fill=-13:13 -13:13 0:0`): `'o'` is a TRISO particle, `'.'` compact
/// matrix. Row `k` is `y = (k - 13) * 0.106` cm, column `i` is
/// `x = (i - 13) * 0.106` cm, both in the fuel pin's frame.
///
/// **333 particles per layer**, 546 layers per 54.6 cm compact stack:
/// 181 818 per rod, which is R1's 12 987 per 3.9 cm compact x 14 (R1
/// §3.1.1). Every particle's OPyC (r = 0.046 cm) lies wholly inside the
/// 0.5-1.3 cm annulus: its centres run from r = 0.5708 to 1.2407 cm.
/// Pinned by `tests::the_layer_pattern_holds_333_whole_particles`.
pub const TRISO_LAYER: [&str; 27] = [
    "...........................",
    "...........................",
    "..........ooooooo..........",
    "........ooooooooooo........",
    "......ooooooooooooooo......",
    ".....ooooooooooooooooo.....",
    "....ooooooooooooooooooo....",
    "....ooooooooooooooooooo....",
    "...ooooooooo...ooooooooo...",
    "..oooooooo.......ooooooo...",
    "..ooooooo.........ooooooo..",
    "..ooooooo.........ooooooo..",
    "..oooooo...........oooooo..",
    "..oooooo...........oooooo..",
    "..oooooo...........oooooo..",
    "..ooooooo.........ooooooo..",
    "..ooooooo.........ooooooo..",
    "...ooooooo.......ooooooo...",
    "...ooooooooo...ooooooooo...",
    "....ooooooooooooooooooo....",
    "....ooooooooooooooooooo....",
    ".....ooooooooooooooooo.....",
    "......ooooooooooooooo......",
    "........ooooooooooo........",
    "..........ooooooo..........",
    "...........................",
    "...........................",
];

/// Particles per 0.1 cm layer, the count of `'o'` in [`TRISO_LAYER`].
pub const TRISO_PER_LAYER: usize = 333;

/// Layers in one fuel rod's compact stack: 54.6 cm / 0.1 cm.
pub const TRISO_LAYERS: usize = 546;

/// The compact stack, modelled as one annulus (R1 §3.1.2.1, Fig. 3.3 p.194;
/// deck surfaces 12, 13): inner radius 0.5 cm (helium inside), outer 1.3 cm,
/// 54.6 cm tall, centred on the block.
pub const COMPACT_INNER_R: f64 = 0.5;
/// Outer radius of the compact annulus \[cm\].
pub const COMPACT_OUTER_R: f64 = 1.3;
/// Half-height of the compact stack \[cm\].
pub const COMPACT_HALF_HEIGHT: f64 = 27.3;

// ---------------------------------------------------------------------------
// Fuel rod, burnable-poison pin, fuel block (R1 §3.1.2.1-3.1.2.3, Figs.
// 3.3-3.6 pp.194-197; deck surfaces 21, 22, 31, 41-44, 501, 502)
// ---------------------------------------------------------------------------

/// Graphite sleeve inner radius and half-height of its cavity \[cm\]
/// (deck surface 21: r 1.325, 54.9 cm). The cavity holds the compacts with a
/// 0.025 cm helium gap and 0.15 cm of helium above and below.
pub const SLEEVE_INNER_R: f64 = 1.325;
/// Half-height of the sleeve's inner cavity \[cm\].
pub const SLEEVE_INNER_HALF_HEIGHT: f64 = 27.45;
/// Sleeve outer radius and half-height \[cm\] (deck surface 22: r 1.7,
/// 57.7 cm; the 1.4 cm above and below the cavity are the end caps).
pub const SLEEVE_OUTER_R: f64 = 1.7;
/// Half-height of the whole sleeve, end caps included \[cm\].
pub const SLEEVE_HALF_HEIGHT: f64 = 28.85;
/// Fuel-hole radius \[cm\]: the 41 mm hole through the block (deck surface
/// 31). Helium fills it outside the sleeve, top to bottom of the block.
pub const FUEL_HOLE_R: f64 = 2.05;

/// Burnable-poison pellet radius \[cm\] (deck surfaces 41-43).
pub const BP_R: f64 = 0.7;
/// Burnable-poison hole radius \[cm\] (deck surface 44).
pub const BP_HOLE_R: f64 = 0.75;
/// Axial layout of a BP pin in the block frame \[cm\] (deck surfaces 41-44):
/// poison from -25 to -5, graphite disks from -5 to +5, poison from +5 to
/// +25, the hole from -25 to +25. R1 Fig. 3.4: two 200 mm poison stacks
/// either side of a 100 mm disk stack.
pub const BP_Z: [f64; 4] = [-25.0, -5.0, 5.0, 25.0];

/// Pitch of the fuel-hole lattice in a block \[cm\]: 51.5 mm, triangular
/// (R1 §3.1.2.3; deck surface 501, hexagon apothem 2.575 cm). The deck's
/// pin hexagons have flats perpendicular to x.
pub const PIN_PITCH: f64 = 5.15;

/// Fuel and reflector blocks: apothem of the hexagonal prism \[cm\], i.e.
/// half the 360 mm across-flats width (R1 §1.1.2.3; deck surfaces 502,
/// 550). Flats perpendicular to y.
pub const BLOCK_APOTHEM: f64 = 18.0;
/// Block height \[cm\]: the column lattice pitch (deck surface 503). R1
/// gives 580 mm (Table 1.13); the deck's block body (59 cm, surface 502) is
/// truncated by the 58 cm lattice element, so the blocks touch.
pub const BLOCK_HEIGHT: f64 = 58.0;
/// Layers in a column: 2 bottom reflector, 5 fuel, 2 top reflector (R1
/// §3.1.2.9, Fig. 3.19 p.210). Layer `k` (`-4..=4`) is centred at
/// `z = 58 k`.
pub const LAYERS_PER_COLUMN: i32 = 9;
/// Half-height of the core: 9 x 58 / 2 = 261 cm (deck surfaces 551, 651).
pub const CORE_HALF_HEIGHT: f64 = 261.0;
/// Bottom of the fuel (the bottom of layer 5), the zero of every rod
/// position \[cm\] (R1 §3.1.2.11).
pub const FUEL_BOTTOM_Z: f64 = -145.0;

/// Reflector-block coolant channel radius \[cm\]: 23 mm diameter (R1 Figs.
/// 3.12-3.13; deck surface 201). **R1's text (§3.1.1) says 21 mm**; the
/// figures and the deck say 23 mm, and the deck is followed. R1 Table 2.15
/// finds the difference negligible.
pub const REFLECTOR_CHANNEL_R: f64 = 1.15;

// ---------------------------------------------------------------------------
// Core (R1 §3.1.2.7-3.1.2.10, Fig. 3.17 p.208; deck surfaces 551, 602, 651)
// ---------------------------------------------------------------------------

/// Column pitch \[cm\]: 362 mm (R1 §1.1.2.3; deck surface 551, apothem 18.1).
/// Every column is 360 mm across flats, so the 2 mm between columns is
/// helium. Columns have flats perpendicular to y.
pub const COLUMN_PITCH: f64 = 36.2;
/// Radius of the permanent-reflector cylinder \[cm\] (R1 §3.1.2.10; deck
/// surface 651). Vacuum beyond it and above and below `|z| = 261`.
pub const PERMANENT_REFLECTOR_R: f64 = 212.5;

// ---------------------------------------------------------------------------
// Control rods and their columns (R1 §3.1.2.5, Figs. 3.7-3.9 pp.198-200;
// deck surfaces 101-105, 151-156)
// ---------------------------------------------------------------------------

/// Control-rod hole radius \[cm\] (deck surfaces 154-156: 123 mm diameter).
pub const CR_HOLE_R: f64 = 6.15;
/// Distance of each hole's axis from the column's \[cm\] (deck: 108 mm).
pub const CR_HOLE_OFFSET: f64 = 10.8;
/// Angles of the three holes in an unrotated column \[deg\], in the order
/// rod, rod, reserve-shutdown (RSS) hole: deck surfaces 154 at (10.8, 0),
/// 155 at (-5.4, -9.35307) and 156 at (-5.4, 9.35307). Rods sit in 154 and
/// 155 (deck cells 611, 612); 156 is the empty RSS hole (cell 613).
pub const CR_HOLE_ANGLES_DEG: [f64; 3] = [0.0, 240.0, 120.0];
/// Bottom of every control-rod hole \[cm\]: 1060 mm above the core bottom
/// (R1 §3.1.2.5; deck surfaces 154-156 start at z = -155).
pub const CR_HOLE_BOTTOM_Z: f64 = -155.0;

/// Control-rod radii \[cm\] (R1 Figs. 3.7-3.8; deck surfaces 101-105):
/// spine, helium, inner clad, absorber, outer clad.
pub const ROD_SPINE_R: f64 = 0.5;
/// Inner clad's inner radius \[cm\] (helium from the spine to here).
pub const ROD_INNER_CLAD_IN_R: f64 = 3.25;
/// Inner clad's outer radius = absorber inner radius \[cm\].
pub const ROD_ABSORBER_IN_R: f64 = 3.75;
/// Absorber outer radius = outer clad inner radius \[cm\].
pub const ROD_ABSORBER_OUT_R: f64 = 5.25;
/// Outer clad outer radius \[cm\].
pub const ROD_OUTER_R: f64 = 5.65;
/// One rod section is 31 cm: a 1 cm Alloy 800H end plate, 29 cm of
/// absorber annulus, a 1 cm end plate (deck surfaces 101-105, lattice 151).
pub const ROD_SECTION: f64 = 31.0;
/// End-plate thickness \[cm\].
pub const ROD_END_PLATE: f64 = 1.0;
/// Sections per rod (R1 §3.1.2.5: ten).
pub const ROD_SECTIONS: usize = 10;

/// Which rod bank a control column belongs to (R1 §3.1.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RodBank {
    /// The centre column A01.
    C,
    /// The six columns of ring C between the fuel.
    R1,
    /// Six columns of ring E.
    R2,
    /// Three columns of ring E.
    R3,
}

/// Rod positions of a benchmark state: the height of each bank's rod bottom
/// above the bottom of the fuel \[cm\] (R1 §3.1.2.11-12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RodPositions {
    /// C bank.
    pub c: f64,
    /// R1 bank.
    pub r1: f64,
    /// R2 bank.
    pub r2: f64,
    /// R3 bank.
    pub r3: f64,
}

impl RodPositions {
    /// The fully-loaded **critical** state (RESR-001 case 6): C, R1, R2 at
    /// 1775 mm, R3 at 4049 mm (R1 §3.1.2.11, Table 1.17; deck cells 610,
    /// 614, 618, 622).
    pub const CRITICAL: Self = Self {
        c: 177.5,
        r1: 177.5,
        r2: 177.5,
        r3: 404.9,
    };
    /// The fully-loaded **subcritical** state: every rod at -55 mm (R1
    /// §3.1.2.12; the deck's subcritical cards 610-622 at z = -5.5).
    pub const SUBCRITICAL: Self = Self {
        c: -5.5,
        r1: -5.5,
        r2: -5.5,
        r3: -5.5,
    };

    /// The rod-bottom height of `bank` above the fuel bottom \[cm\].
    #[must_use]
    pub fn of(&self, bank: RodBank) -> f64 {
        match bank {
            RodBank::C => self.c,
            RodBank::R1 => self.r1,
            RodBank::R2 => self.r2,
            RodBank::R3 => self.r3,
        }
    }
}

// ---------------------------------------------------------------------------
// Loading (R1 Table 1.11, Fig. 3.14 p.205; deck cells 401-416, 501-504)
// ---------------------------------------------------------------------------

/// The twelve enrichments \[wt.% U-235\] in R1 Table 3.1's order. Kernel
/// material `i` is enrichment `ENRICHMENTS[i]`.
pub const ENRICHMENTS: [f64; 12] = [3.4, 3.9, 4.3, 4.8, 5.2, 5.9, 6.3, 6.7, 7.2, 7.9, 9.4, 9.9];

/// Burnable-poison natural-boron content of a fuel block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BpLoading {
    /// 2.0 wt.% natural boron (R1 Table 3.5).
    Two,
    /// 2.5 wt.% natural boron.
    TwoAndHalf,
}

/// The fuel layer of a block, top (`L1`) to bottom (`L5`). L4 and L5 carry
/// the same loading (R1 Table 1.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuelLayer {
    /// Top fuel layer.
    L1,
    /// Second.
    L2,
    /// Third.
    L3,
    /// Fourth or fifth (same loading).
    L45,
}

/// Fuel zone 1-4 (R1 Fig. 3.17), by its column ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuelZone {
    /// Ring B.
    Z1,
    /// Ring C, between the R1 columns.
    Z2,
    /// Ring D, except its corners.
    Z3,
    /// The six corners of ring D.
    Z4,
}

impl FuelZone {
    /// Index of the enrichment ([`ENRICHMENTS`]) in a block of this zone at
    /// `layer` (R1 Table 1.11; deck cells 401-416 fill universes 37-48).
    #[must_use]
    pub fn enrichment_index(self, layer: FuelLayer) -> usize {
        // Rows: Z1..Z4. Columns: L1, L2, L3, L45.
        const T: [[usize; 4]; 4] = [
            [7, 4, 2, 0],   // 6.7 / 5.2 / 4.3 / 3.4
            [9, 6, 4, 1],   // 7.9 / 6.3 / 5.2 / 3.9
            [10, 8, 5, 2],  // 9.4 / 7.2 / 5.9 / 4.3
            [11, 9, 6, 3],  // 9.9 / 7.9 / 6.3 / 4.8
        ];
        let col = match layer {
            FuelLayer::L1 => 0,
            FuelLayer::L2 => 1,
            FuelLayer::L3 => 2,
            FuelLayer::L45 => 3,
        };
        T[self as usize][col]
    }

    /// Whether this zone's blocks have 31 fuel holes (zones 3 and 4) rather
    /// than 33 (R1 §3.1.2.3).
    #[must_use]
    pub fn has_31_holes(self) -> bool {
        matches!(self, Self::Z3 | Self::Z4)
    }
}

impl FuelLayer {
    /// Burnable-poison loading of a block in this layer (R1 Table 1.11: 2.0
    /// in L1, L4, L5; 2.5 in L2, L3). The same in every zone.
    #[must_use]
    pub fn bp(self) -> BpLoading {
        match self {
            Self::L1 | Self::L45 => BpLoading::Two,
            Self::L2 | Self::L3 => BpLoading::TwoAndHalf,
        }
    }

    /// The fuel layer at column layer `k` (`-2..=2`, bottom to top), or
    /// `None` for a reflector layer.
    #[must_use]
    pub fn at(k: i32) -> Option<Self> {
        match k {
            -2 | -1 => Some(Self::L45),
            0 => Some(Self::L3),
            1 => Some(Self::L2),
            2 => Some(Self::L1),
            _ => None,
        }
    }
}

/// What occupies a hole position of a fuel block, in the block's own
/// (unrotated) frame. Index `(i, j)` is the deck's `lat=2` index
/// (cells 401-416): position `i * a1 + j * a2` with `a1 = (5.15, 0)` and
/// `a2 = 5.15 (cos 60, sin 60)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinSlot {
    /// A fuel rod.
    Fuel,
    /// A burnable-poison pin.
    Bp,
    /// The empty third BP hole (helium).
    EmptyBp,
    /// Solid block graphite (the centre, and in 31-hole blocks two corners).
    Graphite,
}

/// The pin slot at block-lattice index `(i, j)` of an unrotated fuel block
/// (deck cells 401-416). `None` outside the three rings of holes.
#[must_use]
pub fn fuel_block_slot(i: i32, j: i32, holes_31: bool) -> Option<PinSlot> {
    let ring = hex_ring(i, j);
    if ring > 3 {
        return None;
    }
    Some(match (i, j) {
        (0, 0) => PinSlot::Graphite,
        (0, -3) | (-3, 3) => PinSlot::Bp,
        (3, 0) => PinSlot::EmptyBp,
        (3, -3) | (0, 3) if holes_31 => PinSlot::Graphite,
        _ => PinSlot::Fuel,
    })
}

/// Hex distance of index `(i, j)` from the centre, on a lattice whose
/// second axis is 60 degrees from the first.
#[must_use]
pub fn hex_ring(i: i32, j: i32) -> i32 {
    (i.abs() + j.abs() + (i + j).abs()) / 2
}

// ---------------------------------------------------------------------------
// The 61-column core map (R1 Fig. 3.17; deck cell 701 and cells 525-673)
// ---------------------------------------------------------------------------

/// What a column is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnRole {
    /// A fuel column of the given zone.
    Fuel(FuelZone),
    /// A control-rod guide column of the given bank.
    Control(RodBank),
    /// An instrumentation column (three empty holes).
    Instrumentation,
    /// A replaceable reflector column (solid graphite).
    Reflector,
}

/// One column of the core map: its deck lattice index `(i, j)`, its role,
/// its rotation \[deg\] and its R1 name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnSpec {
    /// Deck core-lattice index along `a1 = (0, 36.2)` cm.
    pub i: i32,
    /// Deck core-lattice index along `a2 = 36.2 (cos 150, sin 150)` cm.
    pub j: i32,
    /// Role.
    pub role: ColumnRole,
    /// Rotation of the column's contents about its axis \[deg\], the angle
    /// of its local x axis in the core frame (deck `*trcl`).
    pub rotation_deg: f64,
    /// R1's column name (Fig. 3.17), as the deck's comments give it; ""
    /// where the deck names none (reflector and instrumentation columns).
    pub name: &'static str,
}

impl ColumnSpec {
    /// Centre of the column \[cm\] in the core frame.
    #[must_use]
    pub fn centre(&self) -> (f64, f64) {
        let (ax, ay) = (0.0, COLUMN_PITCH);
        let (bx, by) = (
            COLUMN_PITCH * (150.0 * PI / 180.0).cos(),
            COLUMN_PITCH * (150.0 * PI / 180.0).sin(),
        );
        let (i, j) = (f64::from(self.i), f64::from(self.j));
        (i * ax + j * bx, i * ay + j * by)
    }
}

/// The deck's core map (cell 701), one row per lattice index `j` from -4
/// to 4, as `(i, deck universe)` pairs. Decoded in [`core_map`].
const CORE_ROWS: [(i32, [u16; 9]); 9] = [
    (-4, [175, 184, 170, 184, 182, 0, 0, 0, 0]),   // j = -4, i = 0..4
    (-3, [184, 143, 138, 137, 145, 184, 0, 0, 0]), // j = -3, i = -1..4
    (-2, [171, 140, 164, 132, 163, 139, 169, 0, 0]), // j = -2, i = -2..4
    (-1, [184, 137, 133, 128, 127, 131, 138, 184, 0]), // j = -1, i = -3..4
    (0, [182, 146, 165, 125, 161, 126, 162, 144, 174]), // j = 0, i = -4..4
    (1, [184, 141, 134, 129, 130, 136, 142, 184, 0]), // j = 1, i = -4..3
    (2, [172, 140, 166, 135, 167, 139, 168, 0, 0]), // j = 2, i = -4..2
    (3, [184, 147, 142, 141, 148, 184, 0, 0, 0]),  // j = 3, i = -4..1
    (4, [176, 184, 173, 184, 182, 0, 0, 0, 0]),    // j = 4, i = -4..0
];

/// Role, rotation and name of a deck column universe (deck cells 525-548,
/// 628-649, 666, 673). The rotation is the angle of the `*trcl` x' axis.
fn deck_universe(u: u16) -> (ColumnRole, f64, &'static str) {
    use ColumnRole::{Control, Fuel, Instrumentation, Reflector};
    use FuelZone::{Z1, Z2, Z3, Z4};
    use RodBank::{C, R1, R2, R3};
    match u {
        125 => (Fuel(Z1), 0.0, "B04"),
        126 => (Fuel(Z1), 180.0, "B01"),
        127 => (Fuel(Z1), 120.0, "B02"),
        128 => (Fuel(Z1), 60.0, "B03"),
        129 => (Fuel(Z1), 300.0, "B05"),
        130 => (Fuel(Z1), 240.0, "B06"),
        131 => (Fuel(Z2), 0.0, "C02"),
        132 => (Fuel(Z2), 300.0, "C04"),
        133 => (Fuel(Z2), 240.0, "C06"),
        134 => (Fuel(Z2), 180.0, "C08"),
        135 => (Fuel(Z2), 120.0, "C10"),
        136 => (Fuel(Z2), 60.0, "C12"),
        137 => (Fuel(Z3), 0.0, "D05/D09"),
        138 => (Fuel(Z3), 60.0, "D02/D06"),
        139 => (Fuel(Z3), 120.0, "D03/D17"),
        140 => (Fuel(Z3), 300.0, "D08/D12"),
        141 => (Fuel(Z3), 240.0, "D11/D15"),
        142 => (Fuel(Z3), 180.0, "D14/D18"),
        143 => (Fuel(Z4), 0.0, "D07"),
        144 => (Fuel(Z4), 120.0, "D01"),
        145 => (Fuel(Z4), 60.0, "D04"),
        146 => (Fuel(Z4), 300.0, "D10"),
        147 => (Fuel(Z4), 240.0, "D13"),
        148 => (Fuel(Z4), 180.0, "D16"),
        161 => (Control(C), 0.0, "A01"),
        162 => (Control(R1), 0.0, "C01"),
        163 => (Control(R1), 300.0, "C03"),
        164 => (Control(R1), 240.0, "C05"),
        165 => (Control(R1), 180.0, "C07"),
        166 => (Control(R1), 120.0, "C09"),
        167 => (Control(R1), 60.0, "C11"),
        168 => (Control(R2), 0.0, "E23"),
        169 => (Control(R2), 300.0, "E03"),
        170 => (Control(R2), 240.0, "E07"),
        171 => (Control(R2), 180.0, "E11"),
        172 => (Control(R2), 120.0, "E15"),
        173 => (Control(R2), 60.0, "E19"),
        174 => (Control(R3), 0.0, "E01"),
        175 => (Control(R3), 240.0, "E09"),
        176 => (Control(R3), 120.0, "E17"),
        182 => (Instrumentation, 0.0, ""),
        184 => (Reflector, 0.0, ""),
        other => panic!("deck universe {other} is not a column"),
    }
}

/// The 61 columns of the fully-loaded core, decoded from the deck's core
/// map. Pinned by `tests::the_core_map_has_rings_a_to_e`.
#[must_use]
pub fn core_map() -> Vec<ColumnSpec> {
    let mut out = Vec::with_capacity(61);
    for (j, row) in CORE_ROWS {
        // The first column index of each row: rows j < 0 start at i = -4 - j
        // when that is >= -4, i.e. the hexagon of radius 4.
        let i0 = (-4 - j).max(-4);
        for (n, &u) in row.iter().enumerate() {
            if u == 0 {
                continue;
            }
            let i = i0 + n as i32;
            let (role, rotation_deg, name) = deck_universe(u);
            out.push(ColumnSpec {
                i,
                j,
                role,
                rotation_deg,
                name,
            });
        }
    }
    out
}
