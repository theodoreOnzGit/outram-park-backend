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
//! runtime. Paint [`PACKED_PEBBLES`] **in order**; **never** regenerate a
//! packing at runtime.
//!
//! Each entry is a whole **sphere centre** `(x, y, z)`, not a flat cut. The
//! bed is monodisperse, so there is no per-pebble radius: every pebble draws
//! at [`SPHERE_RADIUS`]. What varies is `z`, how far the pebble sits *behind*
//! the cut plane — which is what lets a widget draw a bed with depth (overlap,
//! shading, slight foreshortening) instead of a flat slice.
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
//! | Depth window kept | `-0.3 <= z <= 0` — 2.0 pebble diameters behind the cut plane |
//! | Pebbles in this baked window | **523** |
//! | Vessel silhouette they cover | **91.9 %** |
//! | Generator wall clock | 169 s |
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
//! # Which way `z` points — get this backwards and the bed draws inside-out
//!
//! The frame is right-handed, so with `+x` right and `+y` up, **`+z` points
//! out of the screen, toward the viewer**. The bed was sawn open on the
//! vertical plane `z = 0` and the half in front of it (`z > 0`, between the
//! cut and the viewer) was thrown away, which is what makes the interior
//! visible. So:
//!
//! - **every baked `z` is negative or zero** — the pebbles recede *into* the
//!   screen, away from the viewer;
//! - `z = 0` is the **nearest** pebble, sitting on the cut face;
//! - `z = -`[`DEPTH_WINDOW`] is the **farthest** pebble kept.
//!
//! A renderer that treats `z` as growing away from the viewer will shade the
//! near pebbles as if they were far and paint them in the wrong order — the
//! bed will look hollow rather than solid.
//!
//! # Painting order — the table is already sorted for you
//!
//! [`PACKED_PEBBLES`] is sorted **back to front** (`z` ascending: most
//! negative, i.e. farthest, first). Paint it straight through in the order
//! given, first entry first, and the painter's algorithm does the occlusion
//! for you — each nearer pebble covers the ones behind it, with no depth
//! buffer and no per-frame sorting. Do **not** reorder the table (e.g. by
//! `y`) unless you are prepared to re-sort by `z` before drawing.
//!
//! # Why only a window of depth
//!
//! Only the first few pebble layers behind the cut are visible; the rest are
//! occluded. Baking the whole half-bed would therefore cost draw calls for
//! pixels nobody sees, and each pebble carries a TRISO speckle of order 50
//! dots, so the circle count is ~50x the pebble count. The window was chosen
//! from a measured sweep in the generator (retained count versus the fraction
//! of the vessel silhouette actually covered); the numbers for the baked
//! choice are in the table above.
//!
//! # Drawing it
//!
//! ```
//! use outram_park_digital_twin_engine::components::pebble_packing::{
//!     depth_fraction, BARREL_HEIGHT, CONE_HEIGHT, PACKED_PEBBLES, SPHERE_RADIUS,
//! };
//!
//! // Map the bed's normalised box onto a screen rect, y flipped (screen y
//! // grows downward), preserving aspect ratio via a single scale factor.
//! let (rect_x, rect_y, rect_w) = (10.0_f32, 10.0_f32, 120.0_f32);
//! let scale = rect_w / 2.0; // the barrel spans x in [-1, 1]
//! let top_y = rect_y; // screen y of the bed coordinate y = BARREL_HEIGHT
//!
//! // Already sorted farthest-first: just paint straight through.
//! for pebble in PACKED_PEBBLES {
//!     let cx = rect_x + rect_w / 2.0 + pebble.x * scale;
//!     let cy = top_y + (BARREL_HEIGHT - pebble.y) * scale;
//!     let cr = SPHERE_RADIUS * scale; // one radius for every pebble
//!     // 0 at the back of the window, 1 on the cut face: darken the far ones.
//!     let lit = 0.45 + 0.55 * pebble.depth();
//!     let _ = (cx, cy, cr, lit); // paint a filled circle here
//! }
//!
//! assert!((depth_fraction(0.0) - 1.0).abs() < 1e-6); // the cut face is nearest
//! let _total_height = BARREL_HEIGHT + CONE_HEIGHT;
//! ```

/// One pebble in the baked cut-away bed — a whole **sphere centre**.
///
/// All three fields are in the normalised vessel frame documented at the
/// module level: barrel inner radius `R = 1`, origin on the vessel axis at
/// the cone/barrel junction, `+x` right, `+y` up, `+z` **toward the viewer**.
/// There is no radius field — the bed is monodisperse, so every pebble draws
/// at [`SPHERE_RADIUS`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedPebble {
    /// Horizontal centre coordinate, in vessel radii. `x = 0` is the axis.
    pub x: f32,
    /// Vertical centre coordinate, in vessel radii. `y = 0` is the
    /// cone/barrel junction; `+y` is up.
    pub y: f32,
    /// Depth centre coordinate, in vessel radii — how far the pebble sits
    /// **behind the cut plane**, so `-`[`DEPTH_WINDOW`]` <= z <= 0`. `z = 0`
    /// is nearest the viewer (on the cut face) and more negative is farther
    /// away. For shading, prefer [`PackedPebble::depth`] over raw `z`.
    pub z: f32,
}

impl PackedPebble {
    /// Construct a pebble from its normalised centre `(x, y, z)`.
    ///
    /// Used by the generated table below; also useful for tests.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// This pebble's dimensionless depth cue in `[0, 1]` — `0` at the back of
    /// the baked window, `1` on the cut face nearest the viewer.
    ///
    /// Shorthand for [`depth_fraction`]`(self.z)`; see that function for what
    /// the number does and does not mean.
    #[must_use]
    pub fn depth(&self) -> f32 {
        depth_fraction(self.z)
    }
}

/// Radius of every packed pebble, in vessel radii (`0.075 R`).
///
/// The bed is monodisperse, so this one value is the drawn radius of every
/// entry in [`PACKED_PEBBLES`] — there is no per-pebble radius to look up.
/// (An earlier bake stored a per-pebble *chord* radius from a strict flat cut;
/// it drew as a distracting mix of large and tiny circles and was replaced by
/// this depth-window bake.)
pub const SPHERE_RADIUS: f32 = 0.075;

/// Depth of the baked slab behind the cut plane, in vessel radii.
///
/// Every entry in [`PACKED_PEBBLES`] has `-DEPTH_WINDOW <= z <= 0`. This is
/// 2.0 pebble diameters — deep enough that overlapping pebbles read as a solid
/// bed with depth, shallow enough that the widget is not paying to draw
/// pebbles the front layers occlude. See the module docs for the measured
/// count/coverage trade behind the number.
pub const DEPTH_WINDOW: f32 = 0.3;

/// Measured `[min_z, max_z]` of the baked pebble centres, in vessel radii.
///
/// Both lie inside `[-`[`DEPTH_WINDOW`]`, 0]` by construction; this records
/// where the data actually landed, which is not exactly the window bounds
/// because it is a finite sample of discrete sphere centres.
pub const DEPTH_BOUNDS: [f32; 2] = [-0.29805, -0.00003];

/// Map a pebble's depth `z` to a dimensionless fraction in `[0, 1]`:
/// `0` at the far edge of the baked window, `1` on the cut face nearest the
/// viewer. Values outside the window clamp.
///
/// **This is a display cue, not physics.** It carries no units and means
/// nothing thermally, neutronically, or mechanically — it exists so a widget
/// can shade, tint, or slightly shrink a pebble by how far back it sits
/// without having to know [`DEPTH_WINDOW`] itself. Typical use: multiply a
/// base colour's brightness by `0.45 + 0.55 * depth`, so the back of the bed
/// falls into shadow and the cut face reads as lit.
///
/// Monotone non-decreasing in `z`, so ordering the table by `z` (as it is
/// baked) also orders it by this fraction.
#[must_use]
pub fn depth_fraction(z: f32) -> f32 {
    (1.0 + z / DEPTH_WINDOW).clamp(0.0, 1.0)
}

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
/// a fill-level indicator against. It is an upper bound for every pebble in
/// [`PACKED_PEBBLES`] (the depth window may not contain the tallest sphere,
/// so the window's own top, [`BED_BOUNDS`]`[3]`, can be slightly lower).
pub const BED_TOP: f32 = 2.18117;

/// Tight bounding box of the baked pebbles as drawn, in the plane of the
/// screen: `[min_x, max_x, min_y, max_y]`, each centre expanded by
/// [`SPHERE_RADIUS`]. Measured from the data below. For the out-of-plane
/// extent see [`DEPTH_BOUNDS`].
pub const BED_BOUNDS: [f32; 4] = [-1.00090, 0.99984, -0.89900, 2.16819];

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

/// The baked packing: 523 sphere centres from the settled pebble bed, taken
/// from the slab just behind the cut plane and **sorted back to front**
/// (`z` ascending — farthest first).
///
/// Paint them in this order and the painter's algorithm handles occlusion
/// for you. Every pebble draws at [`SPHERE_RADIUS`]; use
/// [`PackedPebble::depth`] for the depth shading.
///
/// See the module documentation for the coordinate convention, the `z`
/// sign convention, and the honest-scope caveat (artwork, not validated
/// physics).
pub const PACKED_PEBBLES: &[PackedPebble] = &[
    PackedPebble::new(-0.87573, 1.15157, -0.29805),
    PackedPebble::new(0.60915, 0.41436, -0.29674),
    PackedPebble::new(0.60112, 1.91764, -0.29629),
    PackedPebble::new(0.32840, -0.31200, -0.29613),
    PackedPebble::new(-0.87714, 0.69567, -0.29595),
    PackedPebble::new(0.87741, 0.03293, -0.29431),
    PackedPebble::new(0.39428, 1.85829, -0.29376),
    PackedPebble::new(-0.75561, 0.93922, -0.29366),
    PackedPebble::new(-0.31037, 1.29527, -0.29328),
    PackedPebble::new(-0.55095, 0.75904, -0.29321),
    PackedPebble::new(0.74287, 1.71324, -0.29223),
    PackedPebble::new(-0.75652, 1.33025, -0.29154),
    PackedPebble::new(-0.05360, -0.45966, -0.29137),
    PackedPebble::new(-0.45575, 1.85705, -0.29054),
    PackedPebble::new(0.43674, 1.30834, -0.29005),
    PackedPebble::new(0.61568, 0.78633, -0.28973),
    PackedPebble::new(-0.76118, 0.78965, -0.28945),
    PackedPebble::new(0.24537, 0.70405, -0.28904),
    PackedPebble::new(-0.34005, 1.95245, -0.28875),
    PackedPebble::new(-0.51831, 0.90524, -0.28863),
    PackedPebble::new(-0.48451, 1.61387, -0.28784),
    PackedPebble::new(-0.49853, 0.32091, -0.28733),
    PackedPebble::new(0.08518, -0.31941, -0.28627),
    PackedPebble::new(0.88019, 0.36131, -0.28623),
    PackedPebble::new(-0.05643, 1.59366, -0.28615),
    PackedPebble::new(-0.60314, 1.88429, -0.28604),
    PackedPebble::new(-0.59193, 1.34715, -0.28509),
    PackedPebble::new(0.40700, -0.43878, -0.28479),
    PackedPebble::new(-0.04186, 0.87946, -0.28454),
    PackedPebble::new(-0.03227, 0.59199, -0.28423),
    PackedPebble::new(-0.45905, 1.27801, -0.28356),
    PackedPebble::new(0.66763, -0.00582, -0.28351),
    PackedPebble::new(0.14084, 1.60176, -0.28295),
    PackedPebble::new(-0.81023, -0.04235, -0.28275),
    PackedPebble::new(0.22839, 0.55544, -0.28160),
    PackedPebble::new(0.88122, 1.87161, -0.28150),
    PackedPebble::new(-0.47253, -0.38170, -0.28121),
    PackedPebble::new(-0.47167, -0.03553, -0.28106),
    PackedPebble::new(-0.34217, 0.05197, -0.28085),
    PackedPebble::new(-0.13396, 1.09821, -0.28047),
    PackedPebble::new(-0.22484, -0.59190, -0.28042),
    PackedPebble::new(0.88238, 0.74211, -0.27992),
    PackedPebble::new(-0.57645, 0.09287, -0.27990),
    PackedPebble::new(-0.15506, 0.42942, -0.27970),
    PackedPebble::new(-0.88271, 0.33850, -0.27940),
    PackedPebble::new(-0.44128, -0.22603, -0.27867),
    PackedPebble::new(0.20710, 0.26380, -0.27764),
    PackedPebble::new(0.24418, 0.85348, -0.27683),
    PackedPebble::new(-0.27521, 0.59489, -0.27673),
    PackedPebble::new(-0.42272, 1.13273, -0.27621),
    PackedPebble::new(-0.32373, -0.38851, -0.27537),
    PackedPebble::new(0.28804, 1.09758, -0.27532),
    PackedPebble::new(0.23697, -0.42881, -0.27527),
    PackedPebble::new(0.49623, 1.74995, -0.27513),
    PackedPebble::new(0.05968, 2.02382, -0.27429),
    PackedPebble::new(-0.27398, 1.15152, -0.27327),
    PackedPebble::new(0.88398, 1.32585, -0.27269),
    PackedPebble::new(0.75870, 0.82719, -0.27236),
    PackedPebble::new(-0.88428, 1.40623, -0.27185),
    PackedPebble::new(0.75182, 1.56507, -0.27121),
    PackedPebble::new(-0.88466, 1.69078, -0.27046),
    PackedPebble::new(-0.50552, 1.99705, -0.27017),
    PackedPebble::new(0.34670, 1.42653, -0.26998),
    PackedPebble::new(0.00703, 1.14816, -0.26991),
    PackedPebble::new(0.06050, 0.10935, -0.26987),
    PackedPebble::new(-0.57349, 1.49485, -0.26938),
    PackedPebble::new(-0.15483, 1.70515, -0.26687),
    PackedPebble::new(-0.88644, 1.00582, -0.26634),
    PackedPebble::new(0.31894, 1.71196, -0.26592),
    PackedPebble::new(0.52087, -0.34369, -0.26563),
    PackedPebble::new(0.48172, 0.24157, -0.26540),
    PackedPebble::new(0.65237, 0.64300, -0.26507),
    PackedPebble::new(-0.29245, 1.81223, -0.26495),
    PackedPebble::new(-0.34221, -0.10875, -0.26297),
    PackedPebble::new(-0.71713, 1.08056, -0.26282),
    PackedPebble::new(-0.73311, 1.52913, -0.26184),
    PackedPebble::new(-0.77319, 0.23801, -0.26144),
    PackedPebble::new(-0.02663, 1.42833, -0.26001),
    PackedPebble::new(0.03199, 0.25610, -0.25877),
    PackedPebble::new(-0.55105, 1.74507, -0.25867),
    PackedPebble::new(0.76532, 1.41642, -0.25846),
    PackedPebble::new(0.88842, 1.72372, -0.25767),
    PackedPebble::new(-0.88892, 0.08281, -0.25751),
    PackedPebble::new(-0.58296, -0.28434, -0.25657),
    PackedPebble::new(-0.00765, 0.41718, -0.25606),
    PackedPebble::new(0.17926, 1.32657, -0.25603),
    PackedPebble::new(0.30523, 1.24520, -0.25590),
    PackedPebble::new(-0.63788, 0.61920, -0.25583),
    PackedPebble::new(0.49831, 0.90728, -0.25355),
    PackedPebble::new(0.23611, 0.40844, -0.25310),
    PackedPebble::new(-0.89005, 0.85656, -0.25283),
    PackedPebble::new(-0.52509, 0.52040, -0.25282),
    PackedPebble::new(0.46796, 0.43697, -0.25212),
    PackedPebble::new(-0.66323, 1.21983, -0.25178),
    PackedPebble::new(0.72214, 1.09875, -0.25119),
    PackedPebble::new(-0.12800, 0.75925, -0.25096),
    PackedPebble::new(-0.17214, 1.39323, -0.25092),
    PackedPebble::new(-0.74321, 0.38445, -0.25082),
    PackedPebble::new(-0.25400, 2.06923, -0.25066),
    PackedPebble::new(-0.08089, 2.07044, -0.25058),
    PackedPebble::new(0.17945, 0.02080, -0.24991),
    PackedPebble::new(0.64608, 0.94127, -0.24768),
    PackedPebble::new(0.40876, 0.11235, -0.24726),
    PackedPebble::new(0.56742, -0.11559, -0.24567),
    PackedPebble::new(-0.26718, 0.17645, -0.24507),
    PackedPebble::new(0.89297, 0.88674, -0.24304),
    PackedPebble::new(0.03708, 1.70243, -0.24296),
    PackedPebble::new(0.79152, -0.07805, -0.24230),
    PackedPebble::new(-0.00478, -0.16996, -0.24229),
    PackedPebble::new(0.09586, 0.92059, -0.24170),
    PackedPebble::new(-0.57047, 1.10270, -0.24079),
    PackedPebble::new(0.54900, 1.22305, -0.23926),
    PackedPebble::new(0.36458, -0.17899, -0.23836),
    PackedPebble::new(-0.19332, -0.45191, -0.23798),
    PackedPebble::new(0.36603, 0.77589, -0.23756),
    PackedPebble::new(-0.14272, -0.10012, -0.23530),
    PackedPebble::new(0.32487, -0.01057, -0.23497),
    PackedPebble::new(0.12497, 0.77403, -0.23385),
    PackedPebble::new(0.36322, 0.33140, -0.23366),
    PackedPebble::new(0.53092, 0.02834, -0.23292),
    PackedPebble::new(0.54550, 1.39421, -0.23289),
    PackedPebble::new(0.62269, 0.27986, -0.23201),
    PackedPebble::new(-0.15825, -0.24907, -0.23131),
    PackedPebble::new(0.89578, 1.57637, -0.23073),
    PackedPebble::new(0.63948, 0.13100, -0.23005),
    PackedPebble::new(0.11604, 1.46304, -0.22994),
    PackedPebble::new(-0.11962, 0.26667, -0.22935),
    PackedPebble::new(0.32325, -0.54931, -0.22905),
    PackedPebble::new(0.08916, -0.71640, -0.22867),
    PackedPebble::new(0.72133, 1.97645, -0.22856),
    PackedPebble::new(-0.20254, 0.03723, -0.22841),
    PackedPebble::new(0.53740, 1.54385, -0.22791),
    PackedPebble::new(-0.34872, -0.52766, -0.22765),
    PackedPebble::new(0.63055, -0.24920, -0.22711),
    PackedPebble::new(0.77606, 0.45117, -0.22665),
    PackedPebble::new(0.76052, 0.21787, -0.22582),
    PackedPebble::new(-0.05930, -0.73025, -0.22517),
    PackedPebble::new(-0.24362, 1.01436, -0.22190),
    PackedPebble::new(0.15823, -0.20620, -0.22109),
    PackedPebble::new(-0.29292, 1.66882, -0.22108),
    PackedPebble::new(0.17446, 1.18100, -0.22082),
    PackedPebble::new(0.29318, 1.99034, -0.22044),
    PackedPebble::new(0.38484, 1.58309, -0.21981),
    PackedPebble::new(-0.75456, 0.53264, -0.21854),
    PackedPebble::new(0.26475, 0.13952, -0.21754),
    PackedPebble::new(-0.42235, 0.81539, -0.21667),
    PackedPebble::new(-0.55002, -0.14450, -0.21523),
    PackedPebble::new(0.90018, 1.03339, -0.21399),
    PackedPebble::new(-0.67762, -0.03499, -0.21356),
    PackedPebble::new(-0.75310, 1.89454, -0.21264),
    PackedPebble::new(0.51773, 1.06690, -0.21151),
    PackedPebble::new(-0.90088, 1.27044, -0.21072),
    PackedPebble::new(-0.27500, 0.32175, -0.21002),
    PackedPebble::new(-0.03643, 1.92846, -0.20976),
    PackedPebble::new(0.27169, 1.84235, -0.20889),
    PackedPebble::new(0.63200, 1.79988, -0.20870),
    PackedPebble::new(0.90140, 2.00055, -0.20762),
    PackedPebble::new(-0.40071, 1.51033, -0.20722),
    PackedPebble::new(0.90169, 0.15219, -0.20673),
    PackedPebble::new(-0.40568, 0.40593, -0.20652),
    PackedPebble::new(0.35679, 0.92305, -0.20648),
    PackedPebble::new(-0.60039, 0.39535, -0.20634),
    PackedPebble::new(-0.69125, -0.19410, -0.20623),
    PackedPebble::new(0.75917, 1.27617, -0.20606),
    PackedPebble::new(0.18041, 1.72353, -0.20482),
    PackedPebble::new(-0.15846, 1.84143, -0.20440),
    PackedPebble::new(-0.63311, 1.62431, -0.20332),
    PackedPebble::new(-0.56771, 0.21983, -0.20178),
    PackedPebble::new(-0.63938, 0.91914, -0.20126),
    PackedPebble::new(0.08473, 0.63392, -0.20098),
    PackedPebble::new(0.22327, -0.65743, -0.20089),
    PackedPebble::new(-0.90308, 1.55938, -0.20068),
    PackedPebble::new(0.64221, 1.65048, -0.20057),
    PackedPebble::new(0.12169, 0.48931, -0.19980),
    PackedPebble::new(-0.45185, 0.11399, -0.19952),
    PackedPebble::new(0.24490, 1.53327, -0.19946),
    PackedPebble::new(-0.15169, 0.56082, -0.19903),
    PackedPebble::new(-0.46049, 1.01012, -0.19894),
    PackedPebble::new(-0.05599, 0.04751, -0.19891),
    PackedPebble::new(0.15504, 1.93605, -0.19882),
    PackedPebble::new(0.78458, 0.66443, -0.19789),
    PackedPebble::new(0.10192, -0.57100, -0.19636),
    PackedPebble::new(-0.69501, 0.12840, -0.19579),
    PackedPebble::new(0.50646, 0.74784, -0.19488),
    PackedPebble::new(0.77204, 0.07035, -0.19472),
    PackedPebble::new(-0.27104, 0.47035, -0.19391),
    PackedPebble::new(-0.90521, 0.21763, -0.19383),
    PackedPebble::new(-0.15957, 0.89400, -0.19330),
    PackedPebble::new(0.43920, 2.01004, -0.19237),
    PackedPebble::new(0.50288, 1.87426, -0.19154),
    PackedPebble::new(-0.27521, 0.79911, -0.19145),
    PackedPebble::new(-0.04750, -0.56909, -0.18880),
    PackedPebble::new(-0.39451, 0.55527, -0.18785),
    PackedPebble::new(-0.40759, 0.25642, -0.18729),
    PackedPebble::new(-0.90608, 1.90578, -0.18620),
    PackedPebble::new(-0.65507, 0.77082, -0.18606),
    PackedPebble::new(-0.68464, 1.76395, -0.18517),
    PackedPebble::new(0.41681, 1.17441, -0.18511),
    PackedPebble::new(0.09479, 1.05920, -0.18506),
    PackedPebble::new(-0.25325, 1.52510, -0.18502),
    PackedPebble::new(-0.63916, 1.98763, -0.18347),
    PackedPebble::new(-0.02139, -0.30442, -0.18025),
    PackedPebble::new(0.34241, 0.48480, -0.18019),
    PackedPebble::new(0.12455, 0.34160, -0.17852),
    PackedPebble::new(0.78559, 1.80102, -0.17838),
    PackedPebble::new(-0.08075, 1.25880, -0.17834),
    PackedPebble::new(0.50497, 0.59885, -0.17833),
    PackedPebble::new(-0.19634, -0.69617, -0.17747),
    PackedPebble::new(-0.49673, 0.68124, -0.17719),
    PackedPebble::new(-0.43270, 1.69956, -0.17625),
    PackedPebble::new(-0.03366, 0.98249, -0.17601),
    PackedPebble::new(0.90827, 1.43751, -0.17554),
    PackedPebble::new(-0.78610, 1.18088, -0.17533),
    PackedPebble::new(0.23249, 1.00078, -0.17522),
    PackedPebble::new(-0.49424, 1.37506, -0.17480),
    PackedPebble::new(-0.37030, 1.22828, -0.17343),
    PackedPebble::new(0.05544, 1.33875, -0.17206),
    PackedPebble::new(-0.34434, 1.37595, -0.17167),
    PackedPebble::new(0.90918, 0.58420, -0.17109),
    PackedPebble::new(-0.77914, 1.63791, -0.16951),
    PackedPebble::new(-0.85807, 0.43647, -0.16905),
    PackedPebble::new(0.03401, 1.57243, -0.16846),
    PackedPebble::new(0.13328, 0.19236, -0.16846),
    PackedPebble::new(-0.22912, 1.27839, -0.16839),
    PackedPebble::new(0.10308, -0.07795, -0.16748),
    PackedPebble::new(-0.10491, 1.51617, -0.16738),
    PackedPebble::new(0.03049, -0.44346, -0.16573),
    PackedPebble::new(0.35965, 0.63287, -0.16453),
    PackedPebble::new(-0.28239, -0.29807, -0.16366),
    PackedPebble::new(-0.16690, 1.98509, -0.16218),
    PackedPebble::new(0.44537, -0.07974, -0.16140),
    PackedPebble::new(-0.52101, 1.89532, -0.16104),
    PackedPebble::new(0.06108, 1.82551, -0.16099),
    PackedPebble::new(0.27780, -0.27322, -0.16057),
    PackedPebble::new(-0.83186, 1.77801, -0.16053),
    PackedPebble::new(-0.00175, 0.80657, -0.16051),
    PackedPebble::new(0.91144, 0.43493, -0.15960),
    PackedPebble::new(0.82710, 1.15157, -0.15805),
    PackedPebble::new(0.42469, -0.30412, -0.15765),
    PackedPebble::new(-0.39110, -0.01598, -0.15675),
    PackedPebble::new(-0.51932, 1.22593, -0.15670),
    PackedPebble::new(-0.54485, 0.00460, -0.15653),
    PackedPebble::new(-0.05453, 0.66652, -0.15600),
    PackedPebble::new(0.79007, 1.65280, -0.15600),
    PackedPebble::new(0.66138, 1.44922, -0.15537),
    PackedPebble::new(0.86964, 0.29121, -0.15443),
    PackedPebble::new(-0.34439, 0.92695, -0.15423),
    PackedPebble::new(0.25779, -0.11683, -0.15382),
    PackedPebble::new(0.76159, 0.95890, -0.15376),
    PackedPebble::new(-0.80607, 1.45523, -0.15374),
    PackedPebble::new(0.43605, 1.45880, -0.15365),
    PackedPebble::new(0.39621, 1.77452, -0.15361),
    PackedPebble::new(0.32269, -0.41586, -0.15359),
    PackedPebble::new(0.68372, -0.13048, -0.15292),
    PackedPebble::new(0.69504, 0.55344, -0.15284),
    PackedPebble::new(0.59245, 1.98812, -0.15272),
    PackedPebble::new(-0.79801, 0.02903, -0.15183),
    PackedPebble::new(0.26267, 1.39271, -0.15062),
    PackedPebble::new(0.50049, 1.66678, -0.15050),
    PackedPebble::new(-0.01561, 0.52216, -0.14965),
    PackedPebble::new(-0.40377, -0.38412, -0.14844),
    PackedPebble::new(0.46854, -0.44712, -0.14833),
    PackedPebble::new(0.61003, 0.43034, -0.14808),
    PackedPebble::new(-0.91314, 1.09375, -0.14793),
    PackedPebble::new(-0.65697, 1.46837, -0.14789),
    PackedPebble::new(-0.87033, 0.68379, -0.14721),
    PackedPebble::new(0.02637, -0.82400, -0.14594),
    PackedPebble::new(-0.79491, 0.81330, -0.14537),
    PackedPebble::new(-0.63515, 1.32015, -0.14402),
    PackedPebble::new(-0.25050, 0.65913, -0.14369),
    PackedPebble::new(0.52268, 0.16564, -0.14308),
    PackedPebble::new(0.50505, 0.32404, -0.14285),
    PackedPebble::new(-0.16070, 0.15189, -0.14249),
    PackedPebble::new(0.46014, 1.31129, -0.14196),
    PackedPebble::new(0.24964, 0.73216, -0.14186),
    PackedPebble::new(0.72752, 0.33743, -0.14181),
    PackedPebble::new(-0.31427, 1.96619, -0.14163),
    PackedPebble::new(0.38136, 0.21471, -0.14160),
    PackedPebble::new(0.12112, -0.32686, -0.14127),
    PackedPebble::new(-0.06413, 1.74395, -0.14042),
    PackedPebble::new(0.62736, 1.30395, -0.14034),
    PackedPebble::new(0.63818, 1.15442, -0.14028),
    PackedPebble::new(-0.01333, 0.17793, -0.13931),
    PackedPebble::new(-0.69442, 0.26649, -0.13777),
    PackedPebble::new(0.58186, 0.86363, -0.13693),
    PackedPebble::new(-0.01850, 0.32702, -0.13678),
    PackedPebble::new(-0.47889, -0.25482, -0.13674),
    PackedPebble::new(0.91490, 1.27121, -0.13649),
    PackedPebble::new(0.91504, 1.86352, -0.13563),
    PackedPebble::new(0.19530, 2.06595, -0.13558),
    PackedPebble::new(0.05218, 1.19366, -0.13467),
    PackedPebble::new(-0.23516, -0.55846, -0.13462),
    PackedPebble::new(-0.52763, 1.54286, -0.13460),
    PackedPebble::new(0.91530, 0.72922, -0.13428),
    PackedPebble::new(-0.91556, 0.94360, -0.13346),
    PackedPebble::new(-0.12977, 0.42650, -0.13228),
    PackedPebble::new(-0.22603, 1.13298, -0.13223),
    PackedPebble::new(0.18245, -0.46307, -0.13214),
    PackedPebble::new(-0.38671, 1.83524, -0.13197),
    PackedPebble::new(-0.17516, 1.64356, -0.13173),
    PackedPebble::new(0.18328, 0.86619, -0.13121),
    PackedPebble::new(-0.72523, 0.65050, -0.13113),
    PackedPebble::new(0.21998, 0.57927, -0.13093),
    PackedPebble::new(0.73188, 0.78740, -0.13040),
    PackedPebble::new(-0.15028, -0.35849, -0.12924),
    PackedPebble::new(0.29139, 1.65721, -0.12904),
    PackedPebble::new(-0.65691, 1.05546, -0.12782),
    PackedPebble::new(-0.07691, 1.11767, -0.12778),
    PackedPebble::new(-0.04113, -0.08269, -0.12667),
    PackedPebble::new(-0.35063, -0.17032, -0.12648),
    PackedPebble::new(0.57170, -0.34097, -0.12487),
    PackedPebble::new(-0.49926, 2.03918, -0.12467),
    PackedPebble::new(-0.25103, -0.05852, -0.12428),
    PackedPebble::new(0.61936, 0.67985, -0.12404),
    PackedPebble::new(0.24724, 0.27791, -0.12235),
    PackedPebble::new(0.07857, 0.06158, -0.12063),
    PackedPebble::new(0.52745, -0.19791, -0.11932),
    PackedPebble::new(-0.78182, -0.11552, -0.11771),
    PackedPebble::new(0.61750, 1.00788, -0.11663),
    PackedPebble::new(-0.53093, 0.81296, -0.11420),
    PackedPebble::new(-0.50402, 0.47023, -0.11341),
    PackedPebble::new(0.47258, 0.96334, -0.11301),
    PackedPebble::new(-0.36879, 1.09131, -0.11282),
    PackedPebble::new(0.35623, 1.05789, -0.11274),
    PackedPebble::new(-0.62294, -0.28818, -0.11218),
    PackedPebble::new(0.85368, -0.03851, -0.11216),
    PackedPebble::new(-0.11847, -0.80734, -0.11099),
    PackedPebble::new(0.91847, 1.71564, -0.11095),
    PackedPebble::new(-0.25354, 1.76963, -0.11076),
    PackedPebble::new(-0.76809, 0.95652, -0.11041),
    PackedPebble::new(0.35941, 1.91327, -0.11013),
    PackedPebble::new(-0.55541, 1.75881, -0.10944),
    PackedPebble::new(0.32283, 1.26210, -0.10806),
    PackedPebble::new(-0.71157, 0.41200, -0.10713),
    PackedPebble::new(0.22335, 0.43136, -0.10640),
    PackedPebble::new(0.47203, 0.47148, -0.10635),
    PackedPebble::new(0.17380, 1.27576, -0.10469),
    PackedPebble::new(0.40978, 0.82764, -0.10388),
    PackedPebble::new(0.80226, 1.95711, -0.10379),
    PackedPebble::new(-0.15340, 0.76602, -0.10348),
    PackedPebble::new(-0.28897, 0.21813, -0.10286),
    PackedPebble::new(-0.91973, 1.37210, -0.10252),
    PackedPebble::new(-0.91940, 1.67120, -0.10210),
    PackedPebble::new(-0.59048, 0.59203, -0.10142),
    PackedPebble::new(-0.37437, 0.73235, -0.10132),
    PackedPebble::new(-0.16903, 1.39795, -0.10109),
    PackedPebble::new(0.04872, -0.19937, -0.10051),
    PackedPebble::new(-0.77829, 1.31830, -0.09949),
    PackedPebble::new(-0.80122, 0.16895, -0.09831),
    PackedPebble::new(0.65814, 0.06359, -0.09771),
    PackedPebble::new(0.86439, 0.86530, -0.09767),
    PackedPebble::new(-0.34676, 1.60289, -0.09765),
    PackedPebble::new(-0.92065, 0.55173, -0.09727),
    PackedPebble::new(0.80030, 1.36977, -0.09663),
    PackedPebble::new(0.32143, -0.61825, -0.09648),
    PackedPebble::new(0.65671, 0.21340, -0.09628),
    PackedPebble::new(-0.63150, -0.11214, -0.09396),
    PackedPebble::new(-0.41111, -0.52288, -0.09229),
    PackedPebble::new(-0.18254, -0.20130, -0.09159),
    PackedPebble::new(0.77815, 1.51799, -0.09137),
    PackedPebble::new(0.10848, 0.73287, -0.09133),
    PackedPebble::new(0.05154, 1.95786, -0.09111),
    PackedPebble::new(0.07598, 1.69374, -0.09089),
    PackedPebble::new(0.59102, 1.56407, -0.08938),
    PackedPebble::new(0.80073, 0.17280, -0.08905),
    PackedPebble::new(-0.54864, 0.95936, -0.08900),
    PackedPebble::new(0.19584, -0.75086, -0.08699),
    PackedPebble::new(-0.64953, 1.87335, -0.08689),
    PackedPebble::new(-0.82637, 0.31577, -0.08668),
    PackedPebble::new(0.33904, -0.00606, -0.08599),
    PackedPebble::new(-0.48206, -0.11169, -0.08573),
    PackedPebble::new(0.68781, 1.73623, -0.08492),
    PackedPebble::new(0.51343, 0.02836, -0.08420),
    PackedPebble::new(0.37169, -0.18484, -0.08406),
    PackedPebble::new(-0.18773, 0.99560, -0.08399),
    PackedPebble::new(-0.92159, 0.07872, -0.08394),
    PackedPebble::new(0.92137, 1.56147, -0.08283),
    PackedPebble::new(-0.27554, -0.42439, -0.08267),
    PackedPebble::new(0.19244, 1.12833, -0.08191),
    PackedPebble::new(-0.00853, 1.44006, -0.08186),
    PackedPebble::new(0.37114, 0.35156, -0.08157),
    PackedPebble::new(-0.15165, 0.28767, -0.08031),
    PackedPebble::new(-0.08730, -0.66405, -0.08028),
    PackedPebble::new(-0.11253, -0.49350, -0.07771),
    PackedPebble::new(0.54756, 1.78858, -0.07687),
    PackedPebble::new(-0.60732, 0.11640, -0.07518),
    PackedPebble::new(-0.13444, 0.02144, -0.07392),
    PackedPebble::new(0.92303, 0.08859, -0.07332),
    PackedPebble::new(-0.49331, 0.21350, -0.07228),
    PackedPebble::new(0.66349, 1.88363, -0.07191),
    PackedPebble::new(0.40665, 1.58072, -0.07146),
    PackedPebble::new(-0.42852, 0.34819, -0.07091),
    PackedPebble::new(-0.88704, 1.21936, -0.07041),
    PackedPebble::new(0.25478, 0.11750, -0.07007),
    PackedPebble::new(-0.53028, -0.39794, -0.07002),
    PackedPebble::new(0.17504, -0.60350, -0.06989),
    PackedPebble::new(-0.27485, 0.37219, -0.06985),
    PackedPebble::new(0.06868, 0.42784, -0.06942),
    PackedPebble::new(0.87486, 1.01181, -0.06806),
    PackedPebble::new(-0.52813, 1.10597, -0.06749),
    PackedPebble::new(0.27490, 1.79283, -0.06734),
    PackedPebble::new(-0.08075, 0.89201, -0.06618),
    PackedPebble::new(0.45653, 0.69065, -0.06567),
    PackedPebble::new(-0.79714, 1.88836, -0.06508),
    PackedPebble::new(0.05654, -0.70166, -0.06499),
    PackedPebble::new(0.54739, 1.40578, -0.06489),
    PackedPebble::new(0.30352, 0.92594, -0.06477),
    PackedPebble::new(0.03427, -0.55355, -0.06443),
    PackedPebble::new(0.74182, 1.23154, -0.06412),
    PackedPebble::new(0.92280, 2.03714, -0.06375),
    PackedPebble::new(0.19161, -0.01781, -0.06317),
    PackedPebble::new(0.21892, 1.93677, -0.06313),
    PackedPebble::new(0.73955, 1.07593, -0.06271),
    PackedPebble::new(-0.22085, 0.52204, -0.06271),
    PackedPebble::new(-0.32551, 2.09319, -0.06263),
    PackedPebble::new(-0.92289, 0.81204, -0.06261),
    PackedPebble::new(0.66956, -0.24760, -0.06049),
    PackedPebble::new(-0.15900, 1.87681, -0.05872),
    PackedPebble::new(-0.43371, 1.29933, -0.05784),
    PackedPebble::new(-0.29791, 1.46080, -0.05724),
    PackedPebble::new(0.82641, 0.63536, -0.05706),
    PackedPebble::new(-0.58576, 0.33001, -0.05663),
    PackedPebble::new(0.19137, -0.20910, -0.05656),
    PackedPebble::new(-0.36690, 0.48932, -0.05602),
    PackedPebble::new(0.80371, 0.43313, -0.05561),
    PackedPebble::new(-0.79527, 1.09287, -0.05443),
    PackedPebble::new(0.53007, 1.21217, -0.05408),
    PackedPebble::new(-0.77890, 0.53438, -0.05324),
    PackedPebble::new(-0.25383, 0.86463, -0.05283),
    PackedPebble::new(-0.04828, 1.61926, -0.05213),
    PackedPebble::new(-0.45054, 0.61336, -0.05193),
    PackedPebble::new(0.25123, 1.51186, -0.05121),
    PackedPebble::new(-0.41849, 0.08600, -0.05101),
    PackedPebble::new(0.15086, 0.98807, -0.05011),
    PackedPebble::new(-0.84806, 1.55148, -0.04676),
    PackedPebble::new(-0.92395, 1.81050, -0.04676),
    PackedPebble::new(-0.70597, 1.59927, -0.04443),
    PackedPebble::new(0.92420, 1.15116, -0.04390),
    PackedPebble::new(-0.63345, 0.72997, -0.04362),
    PackedPebble::new(0.05474, 0.60130, -0.04347),
    PackedPebble::new(-0.73256, 1.75473, -0.04343),
    PackedPebble::new(-0.29628, 1.23086, -0.04309),
    PackedPebble::new(-0.14949, 1.26129, -0.04245),
    PackedPebble::new(-0.47775, 1.64982, -0.04215),
    PackedPebble::new(0.10294, 1.53186, -0.04178),
    PackedPebble::new(-0.00372, 1.29611, -0.04089),
    PackedPebble::new(0.22797, -0.35351, -0.04071),
    PackedPebble::new(0.06439, 0.86619, -0.03994),
    PackedPebble::new(-0.06381, -0.27639, -0.03929),
    PackedPebble::new(-0.66325, 0.87655, -0.03925),
    PackedPebble::new(-0.55349, 1.38729, -0.03762),
    PackedPebble::new(-0.41228, 0.86247, -0.03735),
    PackedPebble::new(0.55130, 0.57840, -0.03731),
    PackedPebble::new(-0.23163, -0.72954, -0.03661),
    PackedPebble::new(0.92448, 0.51962, -0.03658),
    PackedPebble::new(0.07691, 0.27018, -0.03601),
    PackedPebble::new(-0.27062, 0.06938, -0.03417),
    PackedPebble::new(-0.01654, 1.83741, -0.03323),
    PackedPebble::new(-0.16789, 1.53104, -0.03210),
    PackedPebble::new(-0.92461, 0.41441, -0.03141),
    PackedPebble::new(0.77037, -0.13745, -0.03128),
    PackedPebble::new(0.40500, 0.11674, -0.03102),
    PackedPebble::new(-0.92492, 0.21833, -0.02970),
    PackedPebble::new(0.13227, 1.81914, -0.02921),
    PackedPebble::new(0.92484, 0.23177, -0.02898),
    PackedPebble::new(-0.70774, 2.00321, -0.02891),
    PackedPebble::new(0.04156, 1.08841, -0.02868),
    PackedPebble::new(0.33818, 0.48716, -0.02777),
    PackedPebble::new(0.41726, -0.52736, -0.02768),
    PackedPebble::new(0.62468, 0.34249, -0.02743),
    PackedPebble::new(-0.02652, 0.74235, -0.02736),
    PackedPebble::new(0.92470, 1.41536, -0.02643),
    PackedPebble::new(-0.13497, 0.63906, -0.02590),
    PackedPebble::new(-0.44200, 1.48672, -0.02499),
    PackedPebble::new(-0.40837, 1.97406, -0.02470),
    PackedPebble::new(-0.32696, -0.27757, -0.02259),
    PackedPebble::new(0.73758, 0.89109, -0.02232),
    PackedPebble::new(0.66813, 0.48595, -0.02205),
    PackedPebble::new(0.79744, 1.81625, -0.02116),
    PackedPebble::new(-0.73519, 0.06067, -0.02004),
    PackedPebble::new(0.62597, -0.09374, -0.01974),
    PackedPebble::new(0.11580, 1.38433, -0.01943),
    PackedPebble::new(0.32521, 0.63578, -0.01910),
    PackedPebble::new(0.63157, 0.78596, -0.01909),
    PackedPebble::new(0.47622, -0.10053, -0.01748),
    PackedPebble::new(0.46443, 0.25338, -0.01737),
    PackedPebble::new(0.27092, -0.50018, -0.01711),
    PackedPebble::new(0.37137, -0.38545, -0.01549),
    PackedPebble::new(0.38019, 1.44428, -0.01543),
    PackedPebble::new(-0.85828, -0.03958, -0.01438),
    PackedPebble::new(0.19572, 1.64617, -0.01416),
    PackedPebble::new(-0.36641, -0.04945, -0.01361),
    PackedPebble::new(0.39628, 1.71861, -0.01338),
    PackedPebble::new(0.49293, 1.07293, -0.01285),
    PackedPebble::new(-0.92509, 1.03103, -0.01227),
    PackedPebble::new(-0.66339, 1.14975, -0.01149),
    PackedPebble::new(0.30104, 0.78338, -0.01066),
    PackedPebble::new(0.82662, 1.65254, -0.01059),
    PackedPebble::new(-0.61278, 0.46968, -0.01054),
    PackedPebble::new(-0.77772, 0.75179, -0.00986),
    PackedPebble::new(0.51650, -0.41801, -0.00962),
    PackedPebble::new(-0.06202, 0.49684, -0.00951),
    PackedPebble::new(-0.48848, 1.84821, -0.00940),
    PackedPebble::new(-0.92590, 0.67244, -0.00901),
    PackedPebble::new(-0.30784, 1.86408, -0.00769),
    PackedPebble::new(0.37333, 1.16330, -0.00748),
    PackedPebble::new(-0.53586, 0.00394, -0.00692),
    PackedPebble::new(0.03437, -0.03432, -0.00687),
    PackedPebble::new(0.34058, 2.02035, -0.00683),
    PackedPebble::new(-0.43307, 1.00747, -0.00636),
    PackedPebble::new(0.26181, 1.35346, -0.00617),
    PackedPebble::new(-0.00415, 0.11045, -0.00597),
    PackedPebble::new(-0.33509, -0.61815, -0.00565),
    PackedPebble::new(-0.72081, -0.19168, -0.00429),
    PackedPebble::new(-0.01171, -0.41186, -0.00308),
    PackedPebble::new(-0.56213, 1.97868, -0.00267),
    PackedPebble::new(-0.68623, 1.44760, -0.00251),
    PackedPebble::new(-0.29443, 0.63677, -0.00224),
    PackedPebble::new(0.47194, -0.27502, -0.00185),
    PackedPebble::new(-0.24748, 1.66145, -0.00174),
    PackedPebble::new(0.56383, 0.14234, -0.00097),
    PackedPebble::new(-0.15620, 1.11748, -0.00052),
    PackedPebble::new(0.76090, 0.28638, -0.00003),
];

#[cfg(test)]
mod tests {
    //! # Verification of the baked packing table
    //!
    //! ## Methodology
    //!
    //! These are **verification** checks — "is the committed table the thing
    //! the module documentation says it is?" — and deliberately **not**
    //! validation of the packing physics: the packing is artwork and is not
    //! validated against anything (see the module-level scope note). Each test
    //! re-derives one documented property directly from [`PACKED_PEBBLES`]
    //! instead of trusting the generator that wrote it:
    //!
    //! 1. the table is non-empty and within the per-frame drawing budget;
    //! 2. every coordinate is finite and every `z` lies in the stated depth
    //!    window `[-`[`DEPTH_WINDOW`]`, 0]`;
    //! 3. every pebble, drawn at [`SPHERE_RADIUS`], lies inside the vessel
    //!    outline ([`vessel_half_width`]) at its own height, above the chute
    //!    plug and below [`BED_TOP`];
    //! 4. the table is sorted **back to front** (`z` ascending) — the property
    //!    a painter's-algorithm consumer relies on for occlusion;
    //! 5. [`BED_BOUNDS`] agrees with the data (one test) and so does
    //!    [`DEPTH_BOUNDS`] (a second);
    //! 6. [`depth_fraction`] stays in `[0, 1]`, is monotone non-decreasing in
    //!    `z`, and hits its documented endpoints;
    //! 7. [`vessel_half_width`] reproduces the documented taper at its three
    //!    defining heights.
    //!
    //! Reference: the module-level coordinate and ordering contract. Pass
    //! criterion: exact for the ordering and range checks, and within [`TOL`]
    //! (justified from the measured DEM wall overlap) for the geometric ones.
    //!
    //! ## Results — measured on the 2026-08-06 bake
    //!
    //! All **8** tests pass on the committed table. The numbers they were run
    //! against, straight from the generator:
    //!
    //! | Quantity | Measured |
    //! |---|---|
    //! | Pebbles in the table | 523 of 2525 settled spheres |
    //! | Depth window / data span | `-0.3 <= z <= 0` / `[-0.29805, -0.00003]` |
    //! | Vessel silhouette covered | 91.9 % |
    //! | Deepest wall penetration, 3-D | `1.58e-3 R` |
    //! | Deepest outline penetration, as drawn | `9.01e-4 R` |
    //! | Containment tolerance used | `5.0e-3 R` |
    //! | Interior solid fraction of the parent packing | 0.6112 |
    //!
    //! **Interpretation.** The committed artwork is internally consistent and
    //! sits inside the outline it is drawn against. The penetrations above are
    //! the soft-sphere contact overlap of the DEM run, not a bookkeeping
    //! error, and they are small but **not** negligible-by-orders-of-magnitude:
    //! the drawn-outline figure `9.01e-4 R` is 5.5x below the `5.0e-3 R`
    //! tolerance and 1.2 % of a pebble radius, and the 3-D figure `1.58e-3 R`
    //! (the chute plug, where the whole column's weight concentrates) is 3.2x
    //! below it. Both are invisible at drawing resolution, so a widget laying
    //! out from [`BED_BOUNDS`] and [`BED_TOP`] cannot visibly clip the bed —
    //! but the margin is single-digit, so a future re-bake with a softer
    //! contact spring could legitimately need [`TOL`] revisited rather than
    //! the data being wrong. Because the ordering check passes, a consumer may
    //! paint the table straight through and get correct occlusion with no
    //! sorting of its own. None of this says the *packing* is physically
    //! right; it says the table is the artwork it claims to be.

    use super::*;

    /// Tolerance for the containment checks, in vessel radii.
    ///
    /// A settled soft-sphere DEM bed presses very slightly into its walls:
    /// the linear contact spring (`k_n = 1e6 N/m`) yields
    /// `m g / k_n ≈ 3e-5 R` under one pebble's own weight and a few tens of
    /// times that where the column load concentrates on the chute plug. The
    /// deepest such overlap measured in this bake was `1.58e-3 R`
    /// (3-D) and `9.01e-4 R` for a drawn circle against the
    /// vessel outline. `5e-3 R` is one fifteenth of a pebble radius: it clears
    /// the measured values by 3.2x and 5.5x respectively — enough headroom that the
    /// test is not brittle, while still being invisible when drawn.
    const TOL: f32 = 5.0e-3;

    /// Upper bound on the table size, as a drawing-cost regression guard.
    ///
    /// Each pebble is painted with a TRISO speckle of order 50 dots, so the
    /// per-repaint circle count is roughly 50x the table length. 600 pebbles
    /// (~30 000 circles) is the point past which a deeper window buys occluded
    /// pebbles at the expense of frame rate. A re-bake that blows through this
    /// should be a deliberate, argued decision — not a silent regression.
    const MAX_PEBBLES_FOR_FRAME_BUDGET: usize = 600;

    /// The baked table is non-empty and inside the drawing budget. A silently
    /// empty bake would draw an empty vessel with no error anywhere; a silently
    /// huge one would just drop the frame rate.
    #[test]
    fn table_size_is_sane_and_within_the_frame_budget() {
        assert!(
            PACKED_PEBBLES.len() > 100,
            "expected a few hundred baked pebbles, got {}",
            PACKED_PEBBLES.len()
        );
        assert!(
            PACKED_PEBBLES.len() <= MAX_PEBBLES_FOR_FRAME_BUDGET,
            "table of {} pebbles exceeds the {MAX_PEBBLES_FOR_FRAME_BUDGET}-pebble drawing budget",
            PACKED_PEBBLES.len()
        );
    }

    /// Every coordinate is finite, and every pebble sits in the documented
    /// depth window: behind the cut plane (`z <= 0`) and no farther back than
    /// [`DEPTH_WINDOW`]. A positive `z` would mean a pebble in the half of the
    /// bed that was supposed to have been cut away.
    #[test]
    fn every_pebble_is_finite_and_inside_the_depth_window() {
        for (i, p) in PACKED_PEBBLES.iter().enumerate() {
            assert!(
                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                "pebble {i} has a non-finite coordinate"
            );
            assert!(
                p.z <= TOL,
                "pebble {i} is in front of the cut plane: z = {}",
                p.z
            );
            assert!(
                p.z >= -DEPTH_WINDOW - TOL,
                "pebble {i} is behind the depth window: z = {} < -{DEPTH_WINDOW}",
                p.z
            );
        }
    }

    /// Every pebble, drawn as a full circle of [`SPHERE_RADIUS`], lies inside
    /// the vessel outline: within the barrel/cone half-width at its own height,
    /// above the chute plug, and below the recorded bed top.
    #[test]
    fn every_pebble_is_inside_the_vessel_outline() {
        for (i, p) in PACKED_PEBBLES.iter().enumerate() {
            assert!(
                p.y - SPHERE_RADIUS >= -CONE_HEIGHT - TOL,
                "pebble {i} pokes below the chute plug: y - r = {}",
                p.y - SPHERE_RADIUS
            );
            assert!(
                p.y + SPHERE_RADIUS <= BED_TOP + TOL,
                "pebble {i} is above the recorded bed top: y + r = {}",
                p.y + SPHERE_RADIUS
            );
            let half_width = vessel_half_width(p.y);
            assert!(
                p.x.abs() + SPHERE_RADIUS <= half_width + TOL,
                "pebble {i} at y = {} pokes through the wall: |x| + r = {} > {half_width}",
                p.y,
                p.x.abs() + SPHERE_RADIUS
            );
        }
    }

    /// The table is sorted **back to front** (`z` ascending), as the module doc
    /// promises. This is the property a consumer relies on to paint straight
    /// through the table with the painter's algorithm: break it and near
    /// pebbles get buried behind far ones.
    #[test]
    fn table_is_sorted_back_to_front() {
        for (i, w) in PACKED_PEBBLES.windows(2).enumerate() {
            assert!(
                w[0].z <= w[1].z,
                "table is not back-to-front at {i}: z = {} then {}",
                w[0].z,
                w[1].z
            );
        }
    }

    /// The recorded [`BED_BOUNDS`] really is the tight bounding box of the
    /// drawn table, so a widget that lays out from it cannot clip the artwork.
    #[test]
    fn bed_bounds_match_the_table() {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for p in PACKED_PEBBLES {
            min_x = min_x.min(p.x - SPHERE_RADIUS);
            max_x = max_x.max(p.x + SPHERE_RADIUS);
            min_y = min_y.min(p.y - SPHERE_RADIUS);
            max_y = max_y.max(p.y + SPHERE_RADIUS);
        }
        for (got, want) in [min_x, max_x, min_y, max_y].iter().zip(BED_BOUNDS.iter()) {
            assert!((got - want).abs() < TOL, "bounds drift: {got} vs {want}");
        }
    }

    /// The recorded [`DEPTH_BOUNDS`] really is the `z` range of the table, and
    /// it sits inside the declared [`DEPTH_WINDOW`].
    #[test]
    fn depth_bounds_match_the_table() {
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for p in PACKED_PEBBLES {
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }
        assert!((min_z - DEPTH_BOUNDS[0]).abs() < TOL, "min z drift");
        assert!((max_z - DEPTH_BOUNDS[1]).abs() < TOL, "max z drift");
        assert!(DEPTH_BOUNDS[0] >= -DEPTH_WINDOW - TOL);
        assert!(DEPTH_BOUNDS[1] <= TOL);
    }

    /// [`depth_fraction`] is a normalised, monotone display cue: in `[0, 1]`
    /// for every baked pebble, non-decreasing in `z` (so nearer is never
    /// darker than farther), and hitting its documented endpoints — `0` at the
    /// back of the window, `1` on the cut face.
    #[test]
    fn depth_fraction_is_a_normalised_monotone_cue() {
        for (i, p) in PACKED_PEBBLES.iter().enumerate() {
            let d = p.depth();
            assert!(
                (0.0..=1.0).contains(&d),
                "pebble {i} has out-of-range depth fraction {d}"
            );
        }

        // Endpoints, and clamping outside the window.
        assert!((depth_fraction(0.0) - 1.0).abs() < 1e-6);
        assert!(depth_fraction(-DEPTH_WINDOW).abs() < 1e-6);
        assert!((depth_fraction(1.0) - 1.0).abs() < 1e-6);
        assert!(depth_fraction(-10.0).abs() < 1e-6);

        // Monotone non-decreasing across the window and beyond it.
        const SAMPLES: usize = 64;
        let mut previous = -1.0_f32;
        for i in 0..=SAMPLES {
            // Sweep z from two windows *behind* the far edge to one window
            // in front of the cut plane, so the clamped tails are covered too.
            let t = (i as f32) / (SAMPLES as f32);
            let z = DEPTH_WINDOW * (3.0 * t - 2.0);
            let d = depth_fraction(z);
            assert!(d >= previous - 1e-6, "depth fraction dips at z = {z}");
            previous = d;
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
