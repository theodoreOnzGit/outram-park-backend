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
//
// GENERATED FILE — DO NOT HAND-EDIT THE DATA TABLE.
// Regenerate with:
//   cargo run --release -p outram-park-fork-liggghts \
//       --example bake_pebble_packing \
//       > crates/outram-park-digital-twin-engine/src/components/pebble_packing.rs

//! Baked **pebble-bed packing artwork** for the reactor-vessel widgets.
//!
//! A single settled, cut-away pebble packing, computed **once** offline and
//! committed here as a `const` table so widget painting costs nothing at
//! runtime. Draw [`PACKED_PEBBLES`] directly; **never** regenerate a packing
//! at runtime.
//!
//! # How it was generated
//!
//! | | |
//! |---|---|
//! | Generator | `crates/outram-park-fork-liggghts/examples/bake_pebble_packing.rs` |
//! | Engine | `outram-park-fork-liggghts` `DemSimulation` (soft-sphere DEM, velocity-Verlet, linked-cell neighbours) |
//! | Contact model | `ContactModel::Hooke` — linear spring-dashpot, `k_n = 1.0e6 N/m`, `γ_n = 2500 N·s/m`, `k_t = 8.0e5 N/m`, `γ_t = 2500 N·s/m`, `μ = 0.4` |
//! | Integration | `dt = 1.0e-4 s`, **74000 steps** (7.40 s simulated) |
//! | Spheres settled (3-D) | **2525** monodisperse, radius `0.075 R`, graphite density 1750 kg/m³ |
//! | Solid fraction (interior control volume) | **0.6112** |
//! | Solid fraction (whole filled vessel) | **0.5751** |
//! | Reference (monodisperse RCP, Scott & Kilgour 1969) | 0.6366 |
//! | Residual motion | over a final 0.5 s window: **3.7 %** of a pebble radius rms, 20.2 % worst case; residual kinetic energy `1.2e-4 J` per pebble |
//! | Circles in this baked slice | **261** |
//! | Generator wall clock | 171 s |
//! | Baked on | 2026-08-06 |
//!
//! # ⚠️ Artwork data, NOT a validated physics result
//!
//! `outram-park-fork-liggghts` is a **scaffold** crate with no human V&V.
//! These coordinates exist so an offline demonstration GUI can draw a
//! believable cut-away pebble bed — pebbles resting on one another rather
//! than floating on a jittered lattice. They are **not** a validated packing
//! prediction, must not be cited as one, and must not inform any facility,
//! licensing, safety, or operational decision. The measured solid fraction is
//! recorded above precisely so a reader can see how far it sits from the
//! literature value instead of having to trust it.
//!
//! One known limitation is worth stating outright, because it bounds what
//! "settled" can mean here. The DEM engine's tangential contact is
//! **history-free** (its own `simulation` module documents this): it carries
//! no accumulated tangential spring between steps, so it has a
//! Coulomb-capped dashpot but **no static friction**. A grain resting on an
//! inclined contact therefore creeps at a small terminal velocity forever,
//! and a strict zero-velocity rest state is unreachable no matter how long
//! the run. The generator confirmed this by measuring two back-to-back
//! windows: the creep was steady, not decaying, while the local solid
//! fraction was unchanged between them. So the *structure* below is a
//! genuinely settled packing; the coordinates are a valid instantaneous
//! snapshot of it, and because the bake is a still image the residual creep
//! does not appear in it at all.
//!
//! # Coordinate convention (read this before drawing)
//!
//! Lengths are **normalised to the vessel barrel inner radius**, `R = 1`.
//! The origin sits **on the vessel axis, at the plane where the conical
//! bottom meets the cylindrical barrel**; `+x` is to the right and `+y` is
//! up. So the vessel outline the widget should draw is:
//!
//! - **Barrel** — `|x| <= 1` for `0 <= y <= 2.2` ([`BARREL_HEIGHT`]).
//! - **Cone** — for `-0.9 <= y <= 0` ([`CONE_HEIGHT`]) the half-width
//!   tapers linearly from `0.18` ([`CHUTE_RADIUS`]) at the bottom to `1`
//!   at `y = 0`. Use [`vessel_half_width`].
//!
//! # What `r` means — it is a *chord*, not the sphere radius
//!
//! The 3-D packing was sliced by the vertical mid-plane `z = 0` (`z` is the
//! out-of-page depth axis). Every sphere whose centre lay within one sphere
//! radius of that plane (`|z| <= 0.075`) is kept, and its drawn radius is the
//! **chord** of the cut, `r = sqrt(r_sphere² − z²)`. A sphere cut through its
//! equator draws at the full [`SPHERE_RADIUS`]; one cut near its pole draws
//! small. That is what a real saw-cut through a packed bed looks like, and it
//! is why the table contains a spread of radii rather than one value.
//!
//! Entries are ordered bottom-up (`y` ascending, then `x`).
//!
//! # Drawing it
//!
//! ```
//! use outram_park_digital_twin_engine::components::pebble_packing::{
//!     PACKED_PEBBLES, BARREL_HEIGHT, CONE_HEIGHT,
//! };
//!
//! // Map the bed's normalised box onto a screen rect, y flipped (screen y
//! // grows downward), preserving aspect ratio via a single scale factor.
//! let (rect_x, rect_y, rect_w) = (10.0_f32, 10.0_f32, 120.0_f32);
//! let scale = rect_w / 2.0; // the barrel spans x in [-1, 1]
//! let top_y = rect_y; // screen y of the bed coordinate y = BARREL_HEIGHT
//! for pebble in PACKED_PEBBLES {
//!     let cx = rect_x + rect_w / 2.0 + pebble.x * scale;
//!     let cy = top_y + (BARREL_HEIGHT - pebble.y) * scale;
//!     let cr = pebble.r * scale;
//!     let _ = (cx, cy, cr); // paint a filled circle here
//! }
//! let _total_height = BARREL_HEIGHT + CONE_HEIGHT;
//! ```

/// One pebble in the baked cut-away slice.
///
/// All three fields are in the normalised vessel frame documented at the
/// module level (barrel inner radius `R = 1`, origin on the axis at the
/// cone/barrel junction, `+y` up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedPebble {
    /// Horizontal centre coordinate, in vessel radii. `x = 0` is the axis.
    pub x: f32,
    /// Vertical centre coordinate, in vessel radii. `y = 0` is the
    /// cone/barrel junction; `+y` is up.
    pub y: f32,
    /// Drawn radius, in vessel radii — the **chord** of the sphere cut by the
    /// mid-plane, `sqrt(r_sphere² − z²)`, so `0 < r <= `[`SPHERE_RADIUS`].
    pub r: f32,
}

impl PackedPebble {
    /// Construct a pebble from its normalised centre and drawn radius.
    ///
    /// Used by the generated table below; also useful for tests.
    #[must_use]
    pub const fn new(x: f32, y: f32, r: f32) -> Self {
        Self { x, y, r }
    }
}

/// Sphere radius of the packed pebbles, in vessel radii (`0.075 R`).
///
/// The maximum any [`PackedPebble::r`] can take — reached only by a sphere
/// cut exactly through its equator.
pub const SPHERE_RADIUS: f32 = 0.075;

/// Height of the cylindrical barrel above the cone junction, in vessel radii.
pub const BARREL_HEIGHT: f32 = 2.2;

/// Height of the conical bottom below the cone junction, in vessel radii.
/// The cone occupies `-CONE_HEIGHT <= y <= 0`.
pub const CONE_HEIGHT: f32 = 0.9;

/// Radius of the discharge chute at the very bottom of the cone, in vessel
/// radii. The bed rests on a plug at that level (no discharge is modelled).
pub const CHUTE_RADIUS: f32 = 0.18;

/// Height of the top of the settled bed, in vessel radii — measured from the
/// full 3-D packing (the top edge of its highest sphere), not assumed.
///
/// This is the bed's free-surface level, so it is the right thing to compare
/// a fill-level indicator against. It is an upper bound for every circle in
/// [`PACKED_PEBBLES`] (the mid-plane slice may not contain the tallest
/// sphere, so the slice's own top, [`BED_BOUNDS`]`[3]`, can be slightly lower).
pub const BED_TOP: f32 = 2.18117;

/// Tight bounding box of the baked slice, `[min_x, max_x, min_y, max_y]`,
/// including each circle's drawn radius. Measured from the data below.
pub const BED_BOUNDS: [f32; 4] = [-1.00036, 1.00010, -0.90011, 2.13445];

/// Inner half-width of the vessel outline at height `y`, in vessel radii.
///
/// This is the silhouette the widget should stroke around the pebbles: `1`
/// throughout the barrel (`y >= 0`), tapering linearly to [`CHUTE_RADIUS`] at
/// the bottom of the cone (`y = -`[`CONE_HEIGHT`]). Outside the vessel
/// (`y < -CONE_HEIGHT`) it clamps to [`CHUTE_RADIUS`].
#[must_use]
pub fn vessel_half_width(y: f32) -> f32 {
    if y >= 0.0 {
        1.0
    } else if y <= -CONE_HEIGHT {
        CHUTE_RADIUS
    } else {
        CHUTE_RADIUS + (y + CONE_HEIGHT) * (1.0 - CHUTE_RADIUS) / CONE_HEIGHT
    }
}

/// The baked packing: 261 circles of the settled pebble bed cut on its
/// vertical mid-plane, ordered bottom-up.
///
/// See the module documentation for the coordinate convention, the chord
/// meaning of `r`, and the honest-scope caveat (artwork, not validated
/// physics).
pub const PACKED_PEBBLES: &[PackedPebble] = &[
    PackedPebble::new(0.00295, -0.82512, 0.07499),
    PackedPebble::new(-0.14252, -0.82505, 0.06587),
    PackedPebble::new(-0.23163, -0.72954, 0.06546),
    PackedPebble::new(0.22696, -0.72828, 0.04789),
    PackedPebble::new(0.05654, -0.70166, 0.03743),
    PackedPebble::new(-0.07927, -0.69260, 0.03481),
    PackedPebble::new(0.32979, -0.61931, 0.05392),
    PackedPebble::new(-0.33509, -0.61815, 0.07479),
    PackedPebble::new(0.17504, -0.60350, 0.02722),
    PackedPebble::new(-0.18857, -0.59287, 0.07491),
    PackedPebble::new(0.03427, -0.55355, 0.03838),
    PackedPebble::new(-0.05705, -0.54315, 0.05366),
    PackedPebble::new(0.41726, -0.52736, 0.06971),
    PackedPebble::new(0.27092, -0.50018, 0.07302),
    PackedPebble::new(-0.28493, -0.48946, 0.05413),
    PackedPebble::new(0.12868, -0.46235, 0.07460),
    PackedPebble::new(0.51650, -0.41801, 0.07438),
    PackedPebble::new(-0.15353, -0.41793, 0.05998),
    PackedPebble::new(-0.01171, -0.41186, 0.07494),
    PackedPebble::new(-0.39822, -0.40706, 0.07500),
    PackedPebble::new(-0.53028, -0.39794, 0.02688),
    PackedPebble::new(0.37137, -0.38545, 0.07338),
    PackedPebble::new(0.22797, -0.35351, 0.06299),
    PackedPebble::new(0.60455, -0.31992, 0.04679),
    PackedPebble::new(0.08791, -0.30031, 0.07497),
    PackedPebble::new(-0.62382, -0.29810, 0.06516),
    PackedPebble::new(-0.47300, -0.27915, 0.07422),
    PackedPebble::new(-0.32696, -0.27757, 0.07152),
    PackedPebble::new(-0.06381, -0.27639, 0.06389),
    PackedPebble::new(0.47194, -0.27502, 0.07498),
    PackedPebble::new(-0.19098, -0.27298, 0.06356),
    PackedPebble::new(0.31819, -0.25573, 0.06549),
    PackedPebble::new(0.66956, -0.24760, 0.04433),
    PackedPebble::new(0.19137, -0.20910, 0.04925),
    PackedPebble::new(-0.72081, -0.19168, 0.07488),
    PackedPebble::new(0.04396, -0.17460, 0.02394),
    PackedPebble::new(-0.56180, -0.16173, 0.06841),
    PackedPebble::new(0.77037, -0.13745, 0.06817),
    PackedPebble::new(-0.08534, -0.13558, 0.07475),
    PackedPebble::new(0.28227, -0.11107, 0.07415),
    PackedPebble::new(-0.23233, -0.10908, 0.07343),
    PackedPebble::new(0.47622, -0.10053, 0.07293),
    PackedPebble::new(0.62597, -0.09374, 0.07235),
    PackedPebble::new(-0.66489, -0.05553, 0.06310),
    PackedPebble::new(-0.36641, -0.04945, 0.07375),
    PackedPebble::new(-0.85828, -0.03958, 0.07361),
    PackedPebble::new(0.03437, -0.03432, 0.07468),
    PackedPebble::new(0.19161, -0.01781, 0.04044),
    PackedPebble::new(0.88555, -0.01057, 0.06815),
    PackedPebble::new(0.38827, -0.00395, 0.05061),
    PackedPebble::new(0.73733, 0.00136, 0.07391),
    PackedPebble::new(-0.53586, 0.00394, 0.07468),
    PackedPebble::new(0.54740, 0.00625, 0.04548),
    PackedPebble::new(-0.17538, 0.01858, 0.02708),
    PackedPebble::new(-0.13444, 0.02144, 0.01270),
    PackedPebble::new(0.25105, 0.05502, 0.03567),
    PackedPebble::new(-0.73519, 0.06067, 0.07227),
    PackedPebble::new(-0.92427, 0.06798, 0.03665),
    PackedPebble::new(-0.27062, 0.06938, 0.06676),
    PackedPebble::new(-0.41849, 0.08600, 0.05498),
    PackedPebble::new(0.92303, 0.08859, 0.01577),
    PackedPebble::new(-0.00415, 0.11045, 0.07476),
    PackedPebble::new(0.40500, 0.11674, 0.06828),
    PackedPebble::new(0.25478, 0.11750, 0.02675),
    PackedPebble::new(0.56383, 0.14234, 0.07499),
    PackedPebble::new(-0.50038, 0.14459, 0.04487),
    PackedPebble::new(0.13922, 0.14627, 0.07200),
    PackedPebble::new(0.71083, 0.14793, 0.06955),
    PackedPebble::new(-0.64958, 0.14937, 0.03793),
    PackedPebble::new(-0.14594, 0.15602, 0.07473),
    PackedPebble::new(-0.36994, 0.20740, 0.07162),
    PackedPebble::new(-0.49331, 0.21350, 0.02001),
    PackedPebble::new(-0.92492, 0.21833, 0.06887),
    PackedPebble::new(0.92484, 0.23177, 0.06917),
    PackedPebble::new(0.31644, 0.23278, 0.07496),
    PackedPebble::new(0.46443, 0.25338, 0.07296),
    PackedPebble::new(0.07691, 0.27018, 0.06579),
    PackedPebble::new(-0.71704, 0.27124, 0.07435),
    PackedPebble::new(-0.22747, 0.27441, 0.05726),
    PackedPebble::new(0.76090, 0.28638, 0.07500),
    PackedPebble::new(-0.48819, 0.29355, 0.05098),
    PackedPebble::new(-0.08106, 0.30665, 0.05556),
    PackedPebble::new(-0.58576, 0.33001, 0.04917),
    PackedPebble::new(0.62468, 0.34249, 0.06981),
    PackedPebble::new(-0.42852, 0.34819, 0.02442),
    PackedPebble::new(-0.35225, 0.35677, 0.04877),
    PackedPebble::new(0.26070, 0.36987, 0.07097),
    PackedPebble::new(-0.27485, 0.37219, 0.02731),
    PackedPebble::new(0.92572, 0.37644, 0.07438),
    PackedPebble::new(0.06336, 0.38281, 0.01545),
    PackedPebble::new(-0.78322, 0.40642, 0.07093),
    PackedPebble::new(-0.92461, 0.41441, 0.06811),
    PackedPebble::new(-0.18046, 0.41767, 0.06544),
    PackedPebble::new(0.06868, 0.42784, 0.02839),
    PackedPebble::new(0.80371, 0.43313, 0.05032),
    PackedPebble::new(-0.47382, 0.44132, 0.06504),
    PackedPebble::new(0.53146, 0.44540, 0.06935),
    PackedPebble::new(-0.61278, 0.46968, 0.07426),
    PackedPebble::new(0.66813, 0.48595, 0.07169),
    PackedPebble::new(0.33818, 0.48716, 0.06967),
    PackedPebble::new(-0.36690, 0.48932, 0.04987),
    PackedPebble::new(-0.06202, 0.49684, 0.07439),
    PackedPebble::new(0.16610, 0.51318, 0.07485),
    PackedPebble::new(0.92448, 0.51962, 0.06547),
    PackedPebble::new(-0.22085, 0.52204, 0.04114),
    PackedPebble::new(-0.77890, 0.53438, 0.05282),
    PackedPebble::new(-0.92352, 0.53706, 0.05132),
    PackedPebble::new(0.79144, 0.53931, 0.05353),
    PackedPebble::new(-0.53080, 0.57554, 0.03057),
    PackedPebble::new(0.55130, 0.57840, 0.06506),
    PackedPebble::new(0.05474, 0.60130, 0.06112),
    PackedPebble::new(-0.67097, 0.60263, 0.07084),
    PackedPebble::new(-0.45054, 0.61336, 0.05411),
    PackedPebble::new(-0.80873, 0.63154, 0.01171),
    PackedPebble::new(0.68811, 0.63462, 0.07500),
    PackedPebble::new(0.82641, 0.63536, 0.04867),
    PackedPebble::new(0.32521, 0.63578, 0.07253),
    PackedPebble::new(0.92339, 0.63647, 0.04857),
    PackedPebble::new(-0.29443, 0.63677, 0.07497),
    PackedPebble::new(-0.13497, 0.63906, 0.07038),
    PackedPebble::new(0.18220, 0.66092, 0.07330),
    PackedPebble::new(-0.92590, 0.67244, 0.07446),
    PackedPebble::new(0.45653, 0.69065, 0.03623),
    PackedPebble::new(-0.63345, 0.72997, 0.06101),
    PackedPebble::new(0.78994, 0.73186, 0.05426),
    PackedPebble::new(-0.02652, 0.74235, 0.06983),
    PackedPebble::new(-0.77772, 0.75179, 0.07435),
    PackedPebble::new(-0.50235, 0.75862, 0.07163),
    PackedPebble::new(-0.32290, 0.77549, 0.05953),
    PackedPebble::new(0.92537, 0.77757, 0.07466),
    PackedPebble::new(0.30104, 0.78338, 0.07424),
    PackedPebble::new(0.63157, 0.78596, 0.07253),
    PackedPebble::new(0.48902, 0.80804, 0.07176),
    PackedPebble::new(-0.14710, 0.81046, 0.06330),
    PackedPebble::new(-0.92289, 0.81204, 0.04129),
    PackedPebble::new(-0.41228, 0.86247, 0.06504),
    PackedPebble::new(-0.25383, 0.86463, 0.05323),
    PackedPebble::new(0.06439, 0.86619, 0.06348),
    PackedPebble::new(-0.66325, 0.87655, 0.06391),
    PackedPebble::new(0.20749, 0.88663, 0.06040),
    PackedPebble::new(0.73758, 0.89109, 0.07160),
    PackedPebble::new(-0.08075, 0.89201, 0.03528),
    PackedPebble::new(-0.92325, 0.89937, 0.04599),
    PackedPebble::new(-0.52945, 0.90361, 0.05710),
    PackedPebble::new(0.92418, 0.92321, 0.06210),
    PackedPebble::new(-0.77623, 0.92575, 0.05911),
    PackedPebble::new(0.30352, 0.92594, 0.03782),
    PackedPebble::new(0.56877, 0.93340, 0.07491),
    PackedPebble::new(0.42095, 0.94150, 0.06996),
    PackedPebble::new(-0.20198, 0.94857, 0.04543),
    PackedPebble::new(0.07234, 0.96173, 0.00362),
    PackedPebble::new(0.15086, 0.98807, 0.05580),
    PackedPebble::new(-0.06521, 1.00322, 0.06747),
    PackedPebble::new(0.79334, 1.00436, 0.04813),
    PackedPebble::new(-0.43307, 1.00747, 0.07473),
    PackedPebble::new(-0.63511, 1.00783, 0.06990),
    PackedPebble::new(0.87486, 1.01181, 0.03151),
    PackedPebble::new(-0.92509, 1.03103, 0.07399),
    PackedPebble::new(0.27696, 1.05049, 0.07361),
    PackedPebble::new(-0.29285, 1.05739, 0.07409),
    PackedPebble::new(0.49293, 1.07293, 0.07389),
    PackedPebble::new(0.73955, 1.07593, 0.04113),
    PackedPebble::new(0.04156, 1.08841, 0.06930),
    PackedPebble::new(-0.79527, 1.09287, 0.05160),
    PackedPebble::new(-0.52813, 1.10597, 0.03270),
    PackedPebble::new(-0.15620, 1.11748, 0.07500),
    PackedPebble::new(-0.66339, 1.14975, 0.07411),
    PackedPebble::new(0.92420, 1.15116, 0.06081),
    PackedPebble::new(-0.40804, 1.15305, 0.07253),
    PackedPebble::new(0.80029, 1.15391, 0.05602),
    PackedPebble::new(0.64578, 1.15586, 0.07148),
    PackedPebble::new(-0.92313, 1.16072, 0.04116),
    PackedPebble::new(0.37333, 1.16330, 0.07463),
    PackedPebble::new(0.53007, 1.21217, 0.05196),
    PackedPebble::new(0.06103, 1.21363, 0.03556),
    PackedPebble::new(-0.88704, 1.21936, 0.02583),
    PackedPebble::new(0.20741, 1.22067, 0.06632),
    PackedPebble::new(-0.53868, 1.22655, 0.07210),
    PackedPebble::new(-0.29628, 1.23086, 0.06139),
    PackedPebble::new(0.74182, 1.23154, 0.03890),
    PackedPebble::new(-0.79782, 1.24206, 0.05549),
    PackedPebble::new(-0.14949, 1.26129, 0.06183),
    PackedPebble::new(0.86721, 1.28056, 0.07481),
    PackedPebble::new(-0.00372, 1.29611, 0.06287),
    PackedPebble::new(-0.66837, 1.29892, 0.07500),
    PackedPebble::new(-0.43371, 1.29933, 0.04774),
    PackedPebble::new(0.63942, 1.31312, 0.07448),
    PackedPebble::new(0.45411, 1.31857, 0.07250),
    PackedPebble::new(-0.24431, 1.34669, 0.06558),
    PackedPebble::new(0.26181, 1.35346, 0.07475),
    PackedPebble::new(-0.45056, 1.37249, 0.02171),
    PackedPebble::new(0.76630, 1.38246, 0.05703),
    PackedPebble::new(0.11580, 1.38433, 0.07244),
    PackedPebble::new(-0.09975, 1.38577, 0.06820),
    PackedPebble::new(-0.55349, 1.38729, 0.06488),
    PackedPebble::new(0.54739, 1.40578, 0.03760),
    PackedPebble::new(-0.82976, 1.40802, 0.07412),
    PackedPebble::new(0.92470, 1.41536, 0.07019),
    PackedPebble::new(0.38019, 1.44428, 0.07340),
    PackedPebble::new(-0.68623, 1.44760, 0.07496),
    PackedPebble::new(-0.29791, 1.46080, 0.04846),
    PackedPebble::new(0.65497, 1.47640, 0.07370),
    PackedPebble::new(-0.44200, 1.48672, 0.07071),
    PackedPebble::new(0.25123, 1.51186, 0.05479),
    PackedPebble::new(-0.02084, 1.51203, 0.05671),
    PackedPebble::new(0.83811, 1.51405, 0.05924),
    PackedPebble::new(0.50409, 1.51844, 0.07109),
    PackedPebble::new(-0.16789, 1.53104, 0.06778),
    PackedPebble::new(0.10294, 1.53186, 0.06228),
    PackedPebble::new(-0.57520, 1.54742, 0.07462),
    PackedPebble::new(-0.84806, 1.55148, 0.05864),
    PackedPebble::new(0.40665, 1.58072, 0.02277),
    PackedPebble::new(-0.37233, 1.59635, 0.05599),
    PackedPebble::new(-0.70597, 1.59927, 0.06042),
    PackedPebble::new(0.32764, 1.60347, 0.05215),
    PackedPebble::new(-0.04828, 1.61926, 0.05392),
    PackedPebble::new(0.07683, 1.62300, 0.01022),
    PackedPebble::new(0.68155, 1.62393, 0.07365),
    PackedPebble::new(0.19572, 1.64617, 0.07365),
    PackedPebble::new(-0.47775, 1.64982, 0.06203),
    PackedPebble::new(0.82662, 1.65254, 0.07425),
    PackedPebble::new(-0.92384, 1.65410, 0.05858),
    PackedPebble::new(-0.24748, 1.66145, 0.07498),
    PackedPebble::new(0.53461, 1.66428, 0.07468),
    PackedPebble::new(-0.77627, 1.66720, 0.02657),
    PackedPebble::new(-0.60816, 1.69561, 0.07329),
    PackedPebble::new(0.39628, 1.71861, 0.07380),
    PackedPebble::new(0.27143, 1.74119, 0.01518),
    PackedPebble::new(-0.38873, 1.74443, 0.06747),
    PackedPebble::new(-0.12842, 1.75082, 0.07314),
    PackedPebble::new(-0.73256, 1.75473, 0.06115),
    PackedPebble::new(0.92445, 1.75762, 0.06742),
    PackedPebble::new(0.27490, 1.79283, 0.03303),
    PackedPebble::new(0.55954, 1.79688, 0.01967),
    PackedPebble::new(-0.92395, 1.81050, 0.05864),
    PackedPebble::new(0.79744, 1.81625, 0.07195),
    PackedPebble::new(0.13227, 1.81914, 0.06908),
    PackedPebble::new(-0.01654, 1.83741, 0.06724),
    PackedPebble::new(-0.63018, 1.83795, 0.04807),
    PackedPebble::new(-0.48848, 1.84821, 0.07441),
    PackedPebble::new(-0.30784, 1.86408, 0.07460),
    PackedPebble::new(0.44973, 1.87066, 0.07498),
    PackedPebble::new(-0.15900, 1.87681, 0.04665),
    PackedPebble::new(0.66349, 1.88363, 0.02130),
    PackedPebble::new(0.30631, 1.88461, 0.05844),
    PackedPebble::new(-0.79714, 1.88836, 0.03728),
    PackedPebble::new(0.92497, 1.90547, 0.07457),
    PackedPebble::new(0.21892, 1.93677, 0.04049),
    PackedPebble::new(0.60968, 1.93744, 0.04835),
    PackedPebble::new(-0.92482, 1.94569, 0.07277),
    PackedPebble::new(0.76264, 1.96826, 0.06318),
    PackedPebble::new(-0.06598, 1.97217, 0.07430),
    PackedPebble::new(-0.40837, 1.97406, 0.07082),
    PackedPebble::new(-0.56213, 1.97868, 0.07495),
    PackedPebble::new(0.12093, 1.98732, 0.06433),
    PackedPebble::new(-0.23254, 1.99355, 0.07500),
    PackedPebble::new(-0.70774, 2.00321, 0.06920),
    PackedPebble::new(0.47796, 2.00899, 0.05376),
    PackedPebble::new(0.34058, 2.02035, 0.07469),
    PackedPebble::new(0.92280, 2.03714, 0.03950),
    PackedPebble::new(-0.32551, 2.09319, 0.04126),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for the containment checks, in vessel radii.
    ///
    /// A settled soft-sphere DEM bed presses very slightly into its walls:
    /// the linear contact spring (`k_n = 1e6 N/m`) yields
    /// `m g / k_n ≈ 3e-5 R` under one pebble's own weight and a few tens of
    /// times that where the column load concentrates on the chute plug. The
    /// deepest such overlap measured in this bake was `1.58e-3 R`
    /// (3-D) and `3.58e-4 R` on the drawn mid-plane outline.
    /// `5e-3 R` is one fifteenth of a pebble radius — well above the measured
    /// values (so the test is not brittle), and invisible when drawn.
    const TOL: f32 = 5.0e-3;

    /// The baked table is non-empty — a silently empty bake would draw an
    /// empty vessel with no error anywhere.
    #[test]
    fn table_is_not_empty() {
        assert!(
            PACKED_PEBBLES.len() > 100,
            "expected a few hundred baked pebbles, got {}",
            PACKED_PEBBLES.len()
        );
    }

    /// Every drawn radius is a physically meaningful chord: strictly
    /// positive, never larger than the sphere radius it was cut from, and
    /// finite.
    #[test]
    fn radii_are_positive_chords() {
        for (i, p) in PACKED_PEBBLES.iter().enumerate() {
            assert!(p.x.is_finite() && p.y.is_finite() && p.r.is_finite());
            assert!(p.r > 0.0, "pebble {i} has non-positive radius {}", p.r);
            assert!(
                p.r <= SPHERE_RADIUS + TOL,
                "pebble {i} radius {} exceeds the sphere radius {SPHERE_RADIUS}",
                p.r
            );
        }
    }

    /// Every drawn circle lies inside the vessel outline: within the barrel/
    /// cone half-width at its own height, above the chute plug, and below the
    /// recorded bed top.
    #[test]
    fn every_pebble_is_inside_the_vessel_outline() {
        for (i, p) in PACKED_PEBBLES.iter().enumerate() {
            assert!(
                p.y - p.r >= -CONE_HEIGHT - TOL,
                "pebble {i} pokes below the chute plug: y - r = {}",
                p.y - p.r
            );
            assert!(
                p.y + p.r <= BED_TOP + TOL,
                "pebble {i} is above the recorded bed top: y + r = {}",
                p.y + p.r
            );
            let half_width = vessel_half_width(p.y);
            assert!(
                p.x.abs() + p.r <= half_width + TOL,
                "pebble {i} at y = {} pokes through the wall: |x| + r = {} > {half_width}",
                p.y,
                p.x.abs() + p.r
            );
        }
    }

    /// The recorded [`BED_BOUNDS`] really is the tight bounding box of the
    /// table, so a widget that lays out from it cannot clip the artwork.
    #[test]
    fn bed_bounds_match_the_table() {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for p in PACKED_PEBBLES {
            min_x = min_x.min(p.x - p.r);
            max_x = max_x.max(p.x + p.r);
            min_y = min_y.min(p.y - p.r);
            max_y = max_y.max(p.y + p.r);
        }
        for (got, want) in [min_x, max_x, min_y, max_y].iter().zip(BED_BOUNDS.iter()) {
            assert!((got - want).abs() < TOL, "bounds drift: {got} vs {want}");
        }
    }

    /// The table is ordered bottom-up, as the module doc states — widgets may
    /// rely on that for back-to-front painting.
    #[test]
    fn table_is_ordered_bottom_up() {
        for w in PACKED_PEBBLES.windows(2) {
            assert!(w[0].y <= w[1].y + TOL, "table is not sorted by y");
        }
    }

    /// [`vessel_half_width`] reproduces the documented outline at its three
    /// defining heights and is monotone in between.
    #[test]
    fn vessel_outline_is_the_documented_taper() {
        assert!((vessel_half_width(0.0) - 1.0).abs() < 1e-6);
        assert!((vessel_half_width(BARREL_HEIGHT) - 1.0).abs() < 1e-6);
        assert!((vessel_half_width(-CONE_HEIGHT) - CHUTE_RADIUS).abs() < 1e-6);
        let mid = vessel_half_width(-CONE_HEIGHT / 2.0);
        assert!(mid > CHUTE_RADIUS && mid < 1.0);
    }
}
