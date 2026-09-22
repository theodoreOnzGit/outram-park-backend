//! HTR-10 reactor vessel — a simplified schematic of its *general structure*.
//!
//! A second HTR-10 vessel widget, deliberately. [`Htr10ReactorVesselVisual`]
//! is a detailed cut-away with a published-artwork feel and a set of flow
//! anchors the `htgr_sim_v1` schematic pins its pipe runs to; changing it
//! would move those runs. This one is the **simplified** counterpart, drawn in
//! the same visual language as
//! [`crate::components::Htr10SteamGeneratorVisual`] so the two read as parts
//! of one plant, and free to be re-proportioned without breaking a consumer.
//!
//! [`Htr10ReactorVesselVisual`]: crate::components::Htr10ReactorVesselVisual
//!
//! ## The three-pass helium path, which is the point of the drawing
//!
//! HTR-10's coolant does not simply fall through the core. It makes three
//! passes, and a schematic that shows only the last one hides most of the
//! vessel:
//!
//! ```text
//!    ┌─────────────────────────────┐
//!    │  ╭───────╮       ╭───────╮  │  cold helium RETURNS LOW, through the
//!    │  │ ╰═╗           ╔═════╯ │  │  annulus of the coaxial duct
//!    │  │  ══ cold plenum (9.7 cm) ══ │
//!    │    │      ┌─────┐        │   │  1. DOWN the annulus, duct to the
//!    │    │      │ bed │        │   │     borehole pick-up only
//!    │  ↑ │      │  ↓  │        │↑  │  2. U-bend at the foot, UP the
//!    │  ↑ │      └──┬──┘        │↑  │     boreholes, then a 90-degree turn
//!    │  ╰─╮         ▼         ╭─╯  │     inward, level into the plenum
//!    │    │ ══ hot plenum ════╪════▶  3. DOWN through the bed to the hot
//!    │ ↓  ╰───────╮  ╭────────╯ cold│     plenum, out through the duct's
//!    │ ↓  bottom  ╰──╯      hot │   │     inner tube
//!    └──────────────────────────┴───┘
//! ```
//!
//! **Where the cold gas comes back in is set by where the duct is**, and the
//! duct is **low**: its hot inner tube has to meet the hot plenum, which sits
//! in the *bottom* reflector. So the cold annulus delivers near the foot of
//! the vessel, and pass 1 is a **short descent to the bottom cavity** — not a
//! full-height downcomer fed from the top.
//!
//! **The annulus is drawn only where the gas moves** — from the duct centreline
//! down to where the boreholes pick it up (maintainer direction, 2026-09-21).
//!
//! Worth knowing what that leaves out: section 4.2 records the annulus as
//! *"filled with 250 degC cold helium to hold vessel temperature below limit"*
//! over its **whole** height, so the real one is cold end to end regardless of
//! where gas is flowing. Drawing only the live segment reads far better — the
//! eye follows one path instead of a tall block with a short active part
//! inside it — at the cost of no longer showing the whole boundary bathed in
//! cold helium. A deliberate trade, not an oversight.
//!
//! Source for the sequence: `docs/reactor-scoping/htr10-plant-data.md`
//! section 4.4, from two sources that agree — step 4 takes the cold helium
//! into the RPV between the vessel and the core barrel *"down to the bottom of
//! the reactor support structure, cooling the support structure first"*, and
//! step 5 turns it up the reflector boreholes.
//!
//! **CORRECTED 2026-09-21.** The first version of this widget ran the
//! downcomer over nearly the full vessel height with its inlet at the ~~top~~,
//! which would have the cold gas arriving high and falling the length of the
//! vessel. It returns **low**. Caught by the maintainer.
//!
//! ## What is cited and what is drawing
//!
//! Proportions come from the plant-data sheet and the r-z partition wherever
//! they have a number, and every one is marked at its constant. Where they
//! record *Unknown*, this widget says so rather than inventing a figure.
//! **It is a schematic, not a scale drawing and not a reproduction of any
//! published figure.**
//!
//! ### Specifically NOT to scale, and easy to mistake for it
//!
//! Asked directly whether the cold plenum is to scale (maintainer, 2026-09-21)
//! — it is not, and the distinction is worth stating because the numbers
//! around it *are* real:
//!
//! - **The cold plenum's band is an assignment, not an attested zone.** The
//!   r-z dataset gives axial boundaries at 95.0, 105.0 and 114.7 cm, but it
//!   carries **no material labels for the axial bands** — it identifies only
//!   the core cavity, the conus, the bed and the gas space above it. What is
//!   attested elsewhere is merely that the cold helium plenum sits *in the top
//!   reflector*. Its elevation and extent here are therefore chosen, using
//!   real boundary values, and should not be quoted as zone identities.
//! - **The turn at the top of each borehole is a drawing device.** In the real
//!   vessel these are channels drilled through graphite that open into a
//!   plenum cavity. It is drawn as one shallow 90-degree bend
//!   ([`TURN_RADIUS_FRACTION`]) that stays level with the plenum — **CHANGED
//!   2026-09-21** from an inverted U, which drew the channel arching over the
//!   plenum like external pipework.
//! - **Where the internals sit within the vessel** ([`INTERNALS_TOP_FRACTION`])
//!   is likewise a choice; no source reviewed gives their elevation.
//!
//! ## Animation
//!
//! Six optional coolant tracer trains, all driven by the **primary** loop mass flow
//! and obeying the crate's "ANIMATION IS DERIVED FROM PHYSICS, NEVER
//! HARDCODED" hard rule. Each pass's inlet end is geometry; the direction the
//! marks then travel comes from the sign of the flow the caller advanced the
//! train with, so a reversed or stalled loop reverses or freezes all six
//! together. (Four were added first; the two coaxial-duct trains, hot out
//! through the inner tube and cold back through the annulus, were added on
//! 2026-09-21.)
//!
//! **The coaxial duct is painted in front of the annulus.** The annulus run
//! starts at the duct centreline, where the cold return enters, and painted
//! after the duct it cut vertically through the middle of it (maintainer,
//! 2026-09-21). The annulus and its marks are now painted first, then the duct
//! and its own two trains over them.
//!
//! Two of them are worth calling out:
//!
//! - **Each borehole is ONE continuous run, annulus to cold plenum**: down the
//!   annulus, a U-bend at the foot, the climb, then a single **90-degree**
//!   turn inward and level into the plenum. The foot is a real reversal — the
//!   gas arrives going down and leaves going up — and drawing it as a bend
//!   rather than butted segments is what makes that legible. The top is one
//!   quarter turn and **never rises above the plenum**; an arch there would
//!   draw pipework standing over it, and these are channels through graphite.
//!
//!   Marks travel the whole run, placed by ARC LENGTH, so they go down, round,
//!   up and in without jumping a corner or stopping at a joint. **The marks
//!   arriving in the cold plenum are the same ones that left the annulus.**
//!   Placing them by vertex index instead would bunch them at the corners,
//!   where the path is finely sampled.
//!
//!   **The marks are painted with the channels, not with the other tracers.**
//!   On the right-hand side the boreholes pass behind the coaxial duct, so
//!   drawing them last would slide them across it as if the channel ran in
//!   front. Painted early, the duct covers them and they pass behind it.
//! - **The upper cold plenum runs two opposed streams inward.** The boreholes
//!   deliver up both sides and the gas converges on the axis before turning
//!   down into the bed, so this is the one place in the vessel where flow
//!   visibly meets itself.
//!
//! **The discharge tube continues as a defuelling chute**: straight down the
//! tube, then an inverted-L leg to the left dipping 15 degrees below
//! horizontal, then a thin exit tube one pebble wide down through the bottom
//! head, left open: the opening to the outside. All of it is filled with
//! pebbles drawn exactly like the bed's. Only the ~3.3 m tube is cited; the
//! rest is a drawing choice (maintainer direction, 2026-09-21). Since
//! 2026-09-22 each straight piece is exactly one DEM segment, with greyed-out
//! elbows at the joints where no pebbles are.
//!
//! **A refuelling chute** closes the recirculation loop, one pebble wide: in
//! through the bottom head from outside the vessel, up the left-hand side, a leg dipping about 15
//! degrees to the centreline, then straight down past the upper plenum to the
//! top of the core cavity, ending in the gas space above the pebbles (not at
//! the bed surface; corrected 2026-09-22). **It is drawn EMPTY**: day to day,
//! pebbles are lifted
//! pneumatically up it one at a time, so the only pebble shown in it is the
//! one in transit (`with_refuel_pebbles`, a `PebbleTransits` launched by the
//! studio's [add pebble] button and driven by the LIFT gas flow, not the
//! primary loop). [remove pebble] likewise sends a highlighted pebble down the
//! defuelling route and out (`with_defuel_pebbles`). The vessel is DRAWN
//! wider than the real one to make room for the refuelling chute
//! ([`REFUEL_CHUTE_ALLOWANCE_CM`]); the cited radius and aspect are unchanged.
//! Its route is a drawing choice (maintainer direction, 2026-09-21).
//!
//! ~~**Pebbles are not to scale.** Every drawn pebble has one radius, a fixed
//! fraction of the vessel width, which comes out roughly twice the real 6 cm
//! pebble.~~ **CORRECTED 2026-09-22**: pebbles are now drawn **to scale**, at
//! the real 6 cm, and **every pebble is DEM**: the bed, the conus and the top
//! 0.25 m of the tube from a one-diameter cut-away slab of the settled HTR-10
//! conus bed that this workspace's DEM port and LIGGGHTS agree on; the rest of
//! the cited ~3.3 m tube from a DEM column baked below it; and the dog-leg and
//! exit tube from DEM segments settled in place along the drawn route (1 828
//! pebbles in all, `htr10_conus_packing`). The reducer bend is an approximate
//! joint between the leg and exit segments. The widget says "(DEM packing, to
//! scale)" on screen.
//!
//! **The fuel discharge tube carries no tracer, deliberately.** Pebbles are
//! not coolant: they cross the core over weeks on a 5-pass recirculation
//! route, and no source in the plant-data sheet gives a throughput to derive a
//! rate from. Animating them would have meant inventing one, and at any speed
//! that read well on screen it would have implied fuel moving at something
//! like the gas velocity.
//!
//! **Nothing is drawn over the pebble bed**, matching the convention
//! [`crate::components::htr10_reactor_vessel::Htr10FlowAnchors`] already sets:
//! the geometry between the bed surface and the hot plenum says the flow goes
//! down, and marks over the pebbles obscure the one region a reader most wants
//! to see.

use crate::animation::{PebbleTransits, TracerTrain};
use crate::components::htr10_conus_packing::{CONUS_SLAB, PEBBLE_RADIUS_M, SLAB_DEPTH_M};
use crate::components::htr10_reactor_vessel::{
    blend_rgb, depth_shade, draw_triso_pebble, BED_BACKDROP, PEBBLE_MATRIX,
};
use crate::components::temperature_colour;
use std::f32::consts::PI;
use egui::{
    Color32, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};
use uom::si::f64::ThermodynamicTemperature;

// ── Radial partition, in centimetres ────────────────────────────────────────
//
// From `crates/kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md`,
// the axisymmetric r-z zone map of the HTR-10 benchmark model. Six of its ten
// radial boundaries are independently reproduced there from Table 2 channel
// arithmetic, and those six are the ones used here.
//
// Recorded in centimetres, as the source authored them, so they stay
// byte-comparable against the dataset for anyone re-checking. Conversion to
// drawing fractions happens in one place, at `radius_fraction`.
//
// **Provenance caution.** That dataset was hand-read from a figure by the
// maintainer and carries no calibration record or uncertainty, and its source
// is a preprint marked "should not be cited or reproduced without permission".
// For publication cite the IAEA TECDOCs or the IRPhEP handbook, never the
// preprint.

/// Fuel discharge tube radius. Corroborated against Table 2.
pub const DISCHARGE_TUBE_RADIUS_CM: f32 = 25.0;

/// Outer radius of the pebble-bed core. Corroborated: "core radius".
pub const CORE_RADIUS_CM: f32 = 90.0;

/// Centreline radius of the ten control-rod borings.
///
/// **Inboard of the coolant boreholes**, which is the ordering a schematic
/// most easily gets backwards: rods at 102.1 cm, coolant at 144.6 cm.
pub const CONTROL_ROD_CHANNEL_RADIUS_CM: f32 = 102.1;

/// Diameter of a control-rod boring.
pub const CONTROL_ROD_CHANNEL_DIAMETER_CM: f32 = 13.0;

/// Centreline radius of the twenty cold-coolant boreholes.
pub const COOLANT_CHANNEL_RADIUS_CM: f32 = 144.6;

/// Diameter of a coolant borehole.
pub const COOLANT_CHANNEL_DIAMETER_CM: f32 = 8.0;

/// Inner radius of the boronated carbon brick band.
///
/// A **material** interface, so unlike the others it is not derivable from
/// channel arithmetic — it is read from the figure only.
pub const BORONATED_BRICK_INNER_RADIUS_CM: f32 = 167.793;

/// Outer radius of the ceramic internals. Corroborated: reflector outer
/// diameter 380 cm / 2.
pub const REFLECTOR_OUTER_RADIUS_CM: f32 = 190.0;

/// Inner radius of the reactor pressure vessel, used as the drawing's radial
/// datum.
///
/// The sheet gives the RPV diameter only as **"more than 4 m"** (section 4.2,
/// *Quoted* as a bound). Taking exactly 4 m is the **conservative** reading
/// for this drawing: it is the smallest vessel consistent with the source, so
/// it yields the **narrowest possible annulus** between the ceramics at 190 cm
/// and the vessel wall. A real HTR-10 annulus is at least this wide, never
/// tighter. Nothing here should be read as a claim that the vessel is 4 m.
pub const VESSEL_INNER_RADIUS_CM: f32 = 200.0;

// ── Axial partition, in centimetres, z increasing DOWNWARD ─────────────────
//
// Same dataset. `z = 0` is the top of the ceramic internals and `z = 610` the
// bottom; four of its spans are reproduced exactly from Table 2 text.

/// Full height of the ceramic internals (reflector and discharge tube).
/// Corroborated: 610 − 0.
pub const INTERNALS_HEIGHT_CM: f32 = 610.0;

/// Top of the core cavity.
pub const CORE_CAVITY_TOP_Z_CM: f32 = 130.0;

/// Top of the band drawn as the cold-helium plenum, model z, centimetres.
///
/// **An assignment, not an attested zone.** 105.0 and 114.7 are real boundary
/// values from the r-z partition, but that dataset carries no material labels
/// for the axial bands. What is attested elsewhere is only that the cold
/// helium plenum sits in the top reflector. One **single** band is used, at
/// its true 9.7 cm thickness, rather than a thicker span picked for
/// legibility — so the plenum is drawn to scale even though which band it is
/// remains a choice.
pub const COLD_PLENUM_TOP_Z_CM: f32 = 105.0;

/// Bottom of that band, model z, centimetres.
pub const COLD_PLENUM_BOTTOM_Z_CM: f32 = 114.7;

/// Zero core height — the top of the conus, and the datum the bed is measured
/// up from. IAEA-TECDOC-1382 part 2 states this explicitly.
pub const CORE_ZERO_HEIGHT_Z_CM: f32 = 351.818;

/// Bottom of the conus. Corroborated: conus height 388.764 − 351.818 = 36.946.
pub const CONUS_BOTTOM_Z_CM: f32 = 388.764;

/// Height of the core cavity. Corroborated against Table 2 exactly.
pub const CORE_CAVITY_HEIGHT_CM: f32 = CORE_ZERO_HEIGHT_Z_CM - CORE_CAVITY_TOP_Z_CM;

/// Pebble-bed height at **first criticality** in the B1 benchmark.
/// Corroborated: 351.818 − 228.758 = 123.06.
pub const CRITICAL_BED_HEIGHT_CM: f32 = 123.06;

/// Average pebble-bed height at the **equilibrium** full-power core, from
/// `docs/reactor-scoping/htr10-plant-data.md` section 4.3 (*Quoted*, three
/// sources agree). Nearly twice the first-criticality loading — which is why
/// the bed height is a parameter here rather than a constant.
pub const EQUILIBRIUM_BED_HEIGHT_CM: f32 = 197.0;

/// Where the ceramic internals sit inside the vessel, as a fraction of vessel
/// height, measured to the top of the internals.
///
/// **A drawing choice.** The internals are 610 cm of a vessel stated only as
/// "more than 11 m", and no source reviewed here gives their elevation within
/// it. Chosen so the upper plenum above and the support structure and
/// discharge route below both have room.
const INTERNALS_TOP_FRACTION: f32 = 0.105;

/// Vessel height used as the axial datum, centimetres.
///
/// As with the radius, the sheet gives **"more than 11 m"** only, and 11 m is
/// taken as the conservative reading.
pub const VESSEL_HEIGHT_CM: f32 = 1100.0;

/// Outer proportions of the reactor pressure vessel, width / height.
///
/// Both dimensions are the conservative readings of "more than" bounds, so
/// this is a **lower bound on slenderness** — the real vessel is at least this
/// slender, never squatter.
pub const HTR10_RPV_ASPECT_RATIO: f32 =
    2.0 * VESSEL_INNER_RADIUS_CM / VESSEL_HEIGHT_CM;

/// Extra radius the DRAWN vessel is given beyond the real one, centimetres,
/// to make room for the refuelling chute between the cold annulus and the
/// wall on the left.
///
/// **A drawing choice** (maintainer direction, 2026-09-21: "make the vessel
/// slightly fatter to accommodate this chute"). It does NOT change
/// [`VESSEL_INNER_RADIUS_CM`] or [`HTR10_RPV_ASPECT_RATIO`], which stay the
/// real values; the cold annulus still ends at the real wall line, and the gap
/// beyond it is the allowance. On the right, where there is no chute, the same
/// gap is left empty.
pub const REFUEL_CHUTE_ALLOWANCE_CM: f32 = 25.0;

/// Radius of the DRAWN vessel interior, centimetres: the real inner radius
/// plus [`REFUEL_CHUTE_ALLOWANCE_CM`]. The drawing scale and the vessel's
/// drawn proportions use this, never the plant data.
pub const DRAWN_VESSEL_RADIUS_CM: f32 = VESSEL_INNER_RADIUS_CM + REFUEL_CHUTE_ALLOWANCE_CM;

/// Proportions the vessel is DRAWN at, width / height: [`HTR10_RPV_ASPECT_RATIO`]
/// widened by the refuelling-chute allowance. Not a plant proportion.
pub const DRAWN_ASPECT_RATIO: f32 = 2.0 * DRAWN_VESSEL_RADIUS_CM / VESSEL_HEIGHT_CM;

/// Fuel discharge tube length as a fraction of vessel height.
///
/// From `docs/reactor-scoping/htr10-plant-data.md` section 4.3: *"about
/// 3.3 m"* (*Quoted*), over the 11 m vessel bound. This is the tube **below
/// the core**, which is a different quantity from the r-z model's 610 cm
/// innermost band — that band is the full model extent, not a tube length.
pub const DISCHARGE_TUBE_LENGTH_FRACTION: f32 = 330.0 / VESSEL_HEIGHT_CM;

/// Number of coolant boreholes drawn in the side reflector, per side.
///
/// The real count is **20** around the full circumference. A longitudinal
/// section cannot show twenty without becoming a smear.
///
/// **Confirmed by the maintainer, 2026-09-21: *"boreholes are fine, 3
/// boreholes, otherwise overcrowded"*.** Three is a deliberate legibility
/// choice, not a placeholder on the way to twenty — do not raise it toward the
/// real count. Each borehole carries its own nested U-bend, so the crowding
/// this avoids is of the *bends* at the foot of the vessel as much as of the
/// vertical runs.
const DRAWN_RISERS_PER_SIDE: usize = 3;

/// Real number of coolant boreholes, for the label.
pub const COOLANT_BOREHOLES: usize = 20;

/// Real number of control-rod borings, all in the side reflector.
///
/// HTR-10 has **no in-core rods** — every one sits in the reflector, which is
/// why none is ever drawn entering the bed.
pub const CONTROL_ROD_CHANNELS: usize = 10;

/// A radius in centimetres, as a fraction of the vessel's **half**-width.
///
/// The single place plant centimetres become drawing units. `1.0` is the
/// DRAWN vessel wall, [`DRAWN_VESSEL_RADIUS_CM`]; the real wall line,
/// [`VESSEL_INNER_RADIUS_CM`], sits just inside it (**CHANGED 2026-09-21**, when
/// the vessel was drawn wider for the refuelling chute; `1.0` was previously
/// the real wall).
pub fn radius_fraction(radius_cm: f32) -> f32 {
    radius_cm / DRAWN_VESSEL_RADIUS_CM
}

/// A model `z` in centimetres, as a fraction of the vessel's height.
///
/// `z = 0` is the top of the ceramic internals, which sit
/// [`INTERNALS_TOP_FRACTION`] down the vessel.
pub fn axial_fraction(z_cm: f32) -> f32 {
    INTERNALS_TOP_FRACTION + z_cm / VESSEL_HEIGHT_CM
}

const OUTLINE: Color32 = Color32::from_rgb(150, 154, 162);
const GRAPHITE: Color32 = Color32::from_rgb(58, 60, 66);
const BORONATED: Color32 = Color32::from_rgb(44, 46, 52);
const INTERNALS: Color32 = Color32::from_rgb(64, 68, 76);
const VOID: Color32 = Color32::from_rgb(28, 30, 34);

/// Fill of every boxed label and greyed-out joint, so they all match
/// (maintainer direction, 2026-09-22).
const LABEL_BOX_GREY: Color32 = Color32::from_rgb(88, 90, 96);

/// Label text size, points: 1.5x the original 9 pt (maintainer direction,
/// 2026-09-22).
const LABEL_FONT_SIZE: f32 = 13.5;
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);
/// Append a **quarter** turn: a vertical at `from_x` swinging into a
/// horizontal at `to_y`, heading toward `to_x`.
///
/// Enters tangent to the vertical and leaves tangent to the horizontal, so a
/// climbing run turns once through 90 degrees and then travels level — which
/// is what a channel meeting a plenum does. It does **not** arch over the top
/// first; that would draw pipework standing above the plenum.
fn push_quarter_bend(from_x: f32, to_x: f32, to_y: f32, radius: f32, out: &mut Vec<Pos2>) {
    let inward = (to_x - from_x).signum();
    let r = radius.max(1.0);
    let centre = Pos2::new(from_x + inward * r, to_y + r);
    let samples = 10;
    for i in 0..=samples {
        let a = 0.5 * PI * i as f32 / samples as f32;
        out.push(Pos2::new(
            centre.x - inward * r * a.cos(),
            centre.y - r * a.sin(),
        ));
    }
}

/// Append a half-turn joining two verticals at `y`, bulging by `bulge`.
///
/// A positive `bulge` reaches **downward** (a U), a negative one **upward**
/// (an inverted U). Both ends leave tangent to the vertical they join, so a
/// run built from these reads as one pipe rather than as segments butted
/// together.
fn push_bend(from_x: f32, to_x: f32, y: f32, bulge: f32, out: &mut Vec<Pos2>) {
    let mid_x = 0.5 * (from_x + to_x);
    let arc_samples = 14;
    out.push(Pos2::new(from_x, y));
    for i in 1..arc_samples {
        let a = PI * i as f32 / arc_samples as f32;
        out.push(Pos2::new(
            mid_x + (from_x - mid_x) * a.cos(),
            y + bulge * a.sin(),
        ));
    }
    out.push(Pos2::new(to_x, y));
}

/// One coolant borehole's whole run, annulus to cold plenum.
///
/// Five pieces, because the gas turns twice and both turns are real:
///
/// ```text
///        plenum ←──╮          (4) inverted-U over the top,
///                  │              inward into the plenum
///                  ↑ (3) climb
///     annulus │    │
///        (1)  ↓    │
///             ╰────╯          (2) U-bend pick-up at the foot
/// ```
///
/// Returned as a single polyline so a tracer placed along it by **arc length**
/// travels the whole route — down, round, up, over and in — without jumping a
/// corner or stopping at a joint. That continuity is the point: the marks
/// arriving in the cold plenum are the same marks that left the annulus.
#[allow(clippy::too_many_arguments)]
fn borehole_path(
    entry_x: f32,
    entry_y: f32,
    riser_x: f32,
    bottom_bend_y: f32,
    plenum_x: f32,
    plenum_y: f32,
) -> Vec<Pos2> {
    let mut points = vec![Pos2::new(entry_x, entry_y)];

    // (1) down the annulus, (2) round the foot, (3) up the borehole.
    //
    // The foot is a genuine U: the gas arrives going down and leaves going up,
    // reversing in the open bottom cavity.
    points.push(Pos2::new(entry_x, bottom_bend_y));
    let foot_bulge = (FOOT_BULGE_FRACTION * (riser_x - entry_x).abs()).max(2.0);
    push_bend(entry_x, riser_x, bottom_bend_y, foot_bulge, &mut points);

    // (4) a single 90-degree turn into the plenum.
    //
    // **Not** an inverted U. An arch would draw the channel rising past the
    // plenum and coming back down into it, which is pipework standing above
    // the plenum that does not exist — these are channels drilled through
    // graphite that open into a cavity. One quarter turn, then level.
    let turn_radius = TURN_RADIUS_FRACTION * (plenum_x - riser_x).abs();
    points.push(Pos2::new(riser_x, plenum_y + turn_radius.max(1.0)));
    push_quarter_bend(riser_x, plenum_x, plenum_y, turn_radius, &mut points);
    points.push(Pos2::new(plenum_x, plenum_y));

    points
}

/// How far the foot U reaches below its turn elevation, as a fraction of the
/// horizontal distance it spans. A drawing choice.
const FOOT_BULGE_FRACTION: f32 = 0.30;

/// Radius of the quarter turn into the plenum, as a fraction of the horizontal
/// distance it covers. A drawing choice, kept small so the turn reads as a
/// corner rather than a sweep.
const TURN_RADIUS_FRACTION: f32 = 0.35;
/// Elevation of the control rods' TOP when fully withdrawn, centimetres, z
/// increasing downward from the top of the internals. A drawing choice: no
/// source reviewed gives the rods' parked position.
const ROD_TOP_Z_CM: f32 = 40.0;

/// Full rod stroke, centimetres: from the parked top, [`ROD_TOP_Z_CM`], down
/// to zero core height, where a fully inserted rod's tip sits.
const ROD_STROKE_CM: f32 = CORE_ZERO_HEIGHT_Z_CM - ROD_TOP_Z_CM;

/// Length of a control rod standing out above the head, in the same units as
/// `stroke`, for an insertion fraction in `[0, 1]`.
///
/// **Exactly the length that has come out of the core** (maintainer direction,
/// 2026-09-22): the rod is rigid, so every centimetre withdrawn from the core
/// is a centimetre more above the head. Fully inserted, nothing stands above;
/// fully withdrawn, the whole stroke does. Drawn at the SAME scale as the
/// tip's travel inside the vessel, so the two lengths always add up to the
/// stroke. Out-of-range insertions clamp.
fn rod_above_head(insertion: f32, stroke: f32) -> f32 {
    (1.0 - insertion.clamp(0.0, 1.0)) * stroke
}

/// Heights of the two drive stages above the head (the housing, then the
/// pressure-boundary extension inside it), as fractions of the vessel height.
/// Drawing choices, fixed so they do not grow with the drive band.
const DRIVE_STAGE_HEIGHTS: [(f32, f32); 2] = [(1.35, 0.036), (0.95, 0.082)];

/// Paint a pipe along `line` as filled shapes, with its width given at every
/// point (`widths[i]` at `line[i]`): one trapezoid per segment and a disc at
/// every interior joint, sized to the local width.
///
/// A thick polyline stroke leaves notches at each corner, where one segment's
/// square end meets the next at an angle, and it cannot change width at all.
/// This draws corners as smooth round joints and lets a pipe taper, which is
/// what the reducer from the defuelling chute into its thin exit tube needs.
/// The two ends stay square, which is what keeps an open end looking open.
fn paint_tapered_pipe(painter: &Painter, line: &[Pos2], widths: &[f32], colour: Color32) {
    debug_assert_eq!(line.len(), widths.len());
    for (i, seg) in line.windows(2).enumerate() {
        let along = seg[1] - seg[0];
        if along.length() < 1e-3 {
            continue;
        }
        let normal = egui::vec2(-along.y, along.x).normalized();
        let (a, b) = (0.5 * widths[i], 0.5 * widths[i + 1]);
        painter.add(egui::Shape::convex_polygon(
            vec![
                seg[0] + normal * a,
                seg[1] + normal * b,
                seg[1] - normal * b,
                seg[0] - normal * a,
            ],
            colour,
            Stroke::NONE,
        ));
    }
    if line.len() > 2 {
        for i in 1..line.len() - 1 {
            painter.circle_filled(line[i], 0.5 * widths[i], colour);
        }
    }
}

/// Centreline of the refuelling chute, from its start low in the vessel to
/// its end in the core.
///
/// ```text
///   ┌────────────────╮      2. from the top-left, a leg to the centreline,
///   │                 ╲        dipping `dip_deg` below horizontal
///   │                  │    3. straight down the centreline, past the upper
///   │                  │       plenum, into the core, ending at `end_y`
///   │  1. up the left-hand gap, from `start` to `top_y`
///   ╵ start
/// ```
///
/// It starts **outside** the vessel, below the bottom head (maintainer
/// correction, 2026-09-21), and everything above the bottom head is inside.
/// The route is a drawing choice
/// (maintainer direction, 2026-09-21: "starts inside the vessel, goes to the
/// top left of the inner vessel, and from the top left, angles down about 15
/// degrees to the centreline of the core, at the centreline, chute goes
/// vertically down straight into the core ... it will go past the upper
/// plenum"). The "starts inside the vessel" part was corrected the
/// same day: the chute starts OUTSIDE, below the bottom head.
fn refuelling_route(start: Pos2, top_y: f32, centre_x: f32, dip_deg: f32, end_y: f32) -> Vec<Pos2> {
    let corner = Pos2::new(start.x, top_y);
    let over_axis = Pos2::new(
        centre_x,
        top_y + (centre_x - start.x).abs() * dip_deg.to_radians().tan(),
    );
    let into_core = Pos2::new(centre_x, end_y.max(over_axis.y));
    vec![start, corner, over_axis, into_core]
}

/// Point at fraction `t` of a polyline's total length, `t` in `[0, 1]`.
///
/// By **arc length**, not by vertex index: spacing marks evenly over vertices
/// would bunch them wherever the path is finely sampled, which for a U-bend is
/// exactly at the corner.
pub(crate) fn point_along(points: &[Pos2], t: f32) -> Pos2 {
    match points.len() {
        0 => Pos2::ZERO,
        1 => points[0],
        _ => {
            let total: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
            if total <= f32::EPSILON {
                return points[0];
            }
            let target = t.clamp(0.0, 1.0) * total;
            let mut walked = 0.0;
            for w in points.windows(2) {
                let seg = (w[1] - w[0]).length();
                if walked + seg >= target {
                    let f = if seg > f32::EPSILON {
                        (target - walked) / seg
                    } else {
                        0.0
                    };
                    return w[0] + (w[1] - w[0]) * f;
                }
                walked += seg;
            }
            points[points.len() - 1]
        }
    }
}

/// Where the vessel and its reserved bands sit inside the widget's box, and
/// the plant-centimetre to screen conversions that follow from it.
///
/// Shared by the paint code and [`Htr10ReactorSchematic::duct_port`], so an
/// anchor a caller connects to is computed by the same arithmetic that draws
/// the duct, and the two cannot drift apart.
struct VesselLayout {
    /// The vessel itself, letterboxed to [`DRAWN_ASPECT_RATIO`].
    rect: Rect,
    /// Band above the vessel for the control rod drives, points.
    drive_band: f32,
    /// Band below the vessel for the chutes' open ends, points.
    chute_band: f32,
}

impl VesselLayout {
    /// Lay the vessel out inside the widget's full box. The rightmost
    /// `duct_reach` points of the box belong to the coaxial duct, so the
    /// vessel is fitted into what is left, on the left.
    fn new(full: Rect, duct_reach: f32) -> Self {
        let bands = 1.0 + DRIVE_BAND_FRACTION + CHUTE_BAND_FRACTION;
        let drive_band = full.height() * DRIVE_BAND_FRACTION / bands;
        let chute_band = full.height() * CHUTE_BAND_FRACTION / bands;
        let rect = fit_native_aspect(Rect::from_min_max(
            Pos2::new(full.left(), full.top() + drive_band),
            Pos2::new(
                (full.right() - duct_reach).max(full.left()),
                full.bottom() - chute_band,
            ),
        ));
        // Anchor the vessel at the LEFT of its space, so the duct to its right
        // stays attached whatever box the caller gives; fitting alone would
        // centre a height-limited vessel and leave the duct short.
        let rect = rect.translate(Vec2::new(full.left() - rect.left(), 0.0));
        Self {
            rect,
            drive_band,
            chute_band,
        }
    }

    /// Half-width of the bore inside the vessel wall, points.
    fn bore(&self) -> f32 {
        0.5 * self.rect.width() * (1.0 - WALL_FRACTION)
    }

    /// Screen x of a plant radius, on `side` -1 (left) or +1 (right).
    fn rx(&self, radius_cm: f32, side: f32) -> f32 {
        self.rect.center().x + side * self.bore() * radius_fraction(radius_cm)
    }

    /// Screen y of a plant elevation, z increasing downward.
    fn zy(&self, z_cm: f32) -> f32 {
        self.rect.top() + self.rect.height() * axial_fraction(z_cm)
    }

    /// The hot helium plenum in the bottom reflector.
    fn hot_plenum(&self) -> Rect {
        Rect::from_min_max(
            Pos2::new(
                self.rx(CORE_RADIUS_CM + 18.0, -1.0),
                self.zy(CONUS_BOTTOM_Z_CM + 14.0),
            ),
            Pos2::new(
                self.rx(CORE_RADIUS_CM + 18.0, 1.0),
                self.zy(CONUS_BOTTOM_Z_CM + 60.0),
            ),
        )
    }

    /// The coaxial duct's outer body and its hot inner tube, the body running
    /// [`DUCT_STUB_FRACTION`] vessel widths past the vessel plus `extension`
    /// points.
    fn coax(&self, extension: f32) -> (Rect, Rect) {
        let plenum = self.hot_plenum();
        let coax = Rect::from_min_max(
            Pos2::new(
                self.rx(CORE_RADIUS_CM + 18.0, 1.0),
                plenum.top() - plenum.height() * 0.45,
            ),
            Pos2::new(
                self.rect.right() + self.rect.width() * DUCT_STUB_FRACTION + extension.max(0.0),
                plenum.bottom() + plenum.height() * 0.45,
            ),
        );
        // The hot inner tube, level with the hot plenum it drains.
        let hot = Rect::from_min_max(
            Pos2::new(coax.left(), plenum.top()),
            Pos2::new(coax.right(), plenum.bottom()),
        );
        (coax, hot)
    }
}

/// How far the coaxial duct runs past the vessel by default, as a fraction of
/// the vessel width. A drawing choice; no source gives the duct length.
const DUCT_STUB_FRACTION: f32 = 0.16;

/// Where the coaxial duct leaves [`Htr10ReactorSchematic`]: its outboard end,
/// for connecting it to a steam generator drawn beside the vessel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuctPort {
    /// Centre of the duct's outboard end, screen points.
    pub end: Pos2,
    /// Height of the duct's outer body (the cold annulus), points.
    pub outer_height: f32,
    /// Height of the hot inner tube, points.
    pub inner_height: f32,
}
/// Letterbox `available` to the vessel's DRAWN proportions,
/// [`DRAWN_ASPECT_RATIO`] (the real ones widened for the refuelling chute).
pub fn fit_native_aspect(available: Rect) -> Rect {
    let target = DRAWN_ASPECT_RATIO;
    let have = available.width() / available.height().max(1.0);
    if have > target {
        let w = available.height() * target;
        Rect::from_center_size(available.center(), Vec2::new(w, available.height()))
    } else {
        let h = available.width() / target;
        Rect::from_center_size(available.center(), Vec2::new(available.width(), h))
    }
}

/// Drawn thickness of the pressure-vessel wall, as a fraction of the vessel's
/// half-width. The bore inside it is what [`radius_fraction`] maps onto.
const WALL_FRACTION: f32 = 0.035;

/// Share of the widget's height reserved **above** the vessel for the control
/// rod drives, as a fraction of the vessel's own height.
///
/// The drives stand proud of the head on the real machine, so they need room
/// outside the pressure boundary. See [`Htr10ReactorSchematic::native_size`],
/// which adds this on top of the vessel's own aspect.
///
/// **CHANGED 2026-09-22** from 0.12: a withdrawn rod now stands above the head
/// by exactly the length it has come out of the core, at true scale (see
/// [`rod_above_head`]), so the band must hold the whole stroke plus a margin
/// for the label.
const DRIVE_BAND_FRACTION: f32 = ROD_STROKE_CM / VESSEL_HEIGHT_CM + 0.04;

/// Share of the widget reserved **below** the vessel, as a fraction of the
/// vessel's own height, for the defuelling chute's open end.
///
/// **A drawing choice** (maintainer direction, 2026-09-21). The chute's last
/// leg passes through the bottom head and ends just outside it, so like the
/// drives above, it needs a little room outside the vessel.
const CHUTE_BAND_FRACTION: f32 = 0.04;

/// Dip of the chute's inverted-L leg below horizontal, degrees. Maintainer
/// direction, 2026-09-21: "an inverted L with a 15 degree dip".
const CHUTE_DIP_DEG: f32 = 15.0;

/// Horizontal run of the inverted-L leg, as a fraction of the vessel bore's
/// half-width. A drawing choice.
const CHUTE_LEG_RUN_FRACTION: f32 = 0.55;

/// Dip of the refuelling chute's leg from the top-left to the centreline,
/// degrees below horizontal. Maintainer direction, 2026-09-21: "angles down
/// about 15 degrees to the centreline of the core".
const REFUEL_DIP_DEG: f32 = 15.0;

/// Simplified HTR-10 reactor vessel.
///
/// Five temperatures drive the colouring, all supplied by the caller:
///
/// | Field | Physical quantity |
/// |---|---|
/// | `pebble_temp` | fuel (pebble) temperature, K — the hottest region |
/// | `inlet_temp` | cold helium entering the vessel, K (250 degC at design) |
/// | `outlet_temp` | hot helium in the bottom plenum, K (700 degC at design) |
/// | `reflector_temp` | graphite reflector bulk temperature, K |
/// | `vessel_temp` | pressure-vessel wall temperature, K |
pub struct Htr10ReactorSchematic {
    size: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    pebble_temp: ThermodynamicTemperature,
    inlet_temp: ThermodynamicTemperature,
    outlet_temp: ThermodynamicTemperature,
    reflector_temp: ThermodynamicTemperature,
    vessel_temp: ThermodynamicTemperature,
    bed_height_cm: f32,
    control_rod_insertion_frac: f32,
    show_labels: bool,
    downcomer_tracer: Option<TracerTrain>,
    riser_tracer: Option<TracerTrain>,
    plenum_tracer: Option<TracerTrain>,
    cold_plenum_tracer: Option<TracerTrain>,
    hot_duct_tracer: Option<TracerTrain>,
    cold_duct_tracer: Option<TracerTrain>,
    /// Extra length of the coaxial duct past its default end, points. See
    /// [`Self::with_duct_extension`].
    duct_extension: f32,
    refuel_pebbles: Option<PebbleTransits>,
    defuel_pebbles: Option<PebbleTransits>,
}

impl Htr10ReactorSchematic {
    /// Build the schematic.
    ///
    /// The bed defaults to the **equilibrium** height and the rods to **fully
    /// inserted**, so a caller that drives neither draws a shut-down core at
    /// its normal loading rather than a critical one.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        size: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        pebble_temp: ThermodynamicTemperature,
        inlet_temp: ThermodynamicTemperature,
        outlet_temp: ThermodynamicTemperature,
        reflector_temp: ThermodynamicTemperature,
        vessel_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            size,
            min_temp,
            max_temp,
            pebble_temp,
            inlet_temp,
            outlet_temp,
            reflector_temp,
            vessel_temp,
            bed_height_cm: EQUILIBRIUM_BED_HEIGHT_CM,
            control_rod_insertion_frac: 1.0,
            show_labels: true,
            downcomer_tracer: None,
            riser_tracer: None,
            plenum_tracer: None,
            cold_plenum_tracer: None,
            hot_duct_tracer: None,
            cold_duct_tracer: None,
            duct_extension: 0.0,
            refuel_pebbles: None,
            defuel_pebbles: None,
        }
    }

    /// The box this widget wants for a given vessel width, including the room
    /// the control rod drives need above the head and the defuelling chute's
    /// open end needs below the bottom head.
    pub fn native_size(vessel_width: f32) -> Vec2 {
        let vessel_h = vessel_width / DRAWN_ASPECT_RATIO;
        Vec2::new(
            vessel_width,
            vessel_h * (1.0 + DRIVE_BAND_FRACTION + CHUTE_BAND_FRACTION),
        )
    }

    /// On-screen size, in points: the vessel box ([`Self::native_size`]) plus
    /// the run of the coaxial duct to its right.
    ///
    /// **CHANGED 2026-09-21.** This was the vessel box alone, and because a
    /// widget's painter is clipped to its own box, the duct beyond the vessel
    /// was never visible: it was cut off at the vessel's edge.
    pub fn size(&self) -> Vec2 {
        self.size + Vec2::new(self.duct_reach(), 0.0)
    }

    /// Width the coaxial duct takes to the right of the vessel box, points.
    fn duct_reach(&self) -> f32 {
        DUCT_STUB_FRACTION * self.size.x + self.duct_extension
    }

    /// Pebble-bed height, centimetres, measured up from zero core height.
    ///
    /// Clamped to the core cavity at render time — a bed taller than its
    /// cavity is not a thing to draw. Two values worth knowing:
    /// [`CRITICAL_BED_HEIGHT_CM`] (123.06, first criticality) and
    /// [`EQUILIBRIUM_BED_HEIGHT_CM`] (197, full-power average).
    pub fn with_bed_height_cm(mut self, height_cm: f32) -> Self {
        self.bed_height_cm = height_cm;
        self
    }

    /// Where the control-rod bank is **drawn**, `0.0` out to `1.0` in.
    pub fn with_control_rod_frac(mut self, frac: f32) -> Self {
        self.control_rod_insertion_frac = frac;
        self
    }

    /// Turn the labels off — for thumbnails. Builder-style.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// The bed height actually drawn, clamped into the core cavity.
    pub fn drawn_bed_height_cm(&self) -> f32 {
        self.bed_height_cm.clamp(0.0, CORE_CAVITY_HEIGHT_CM)
    }

    /// Pass 1: cold helium descending the annulus to the bottom cavity.
    ///
    /// Advance with the primary mass flow. Its inlet is the duct elevation —
    /// **low** on the vessel, because the duct's hot inner tube has to meet
    /// the hot plenum in the bottom reflector.
    pub fn with_downcomer_tracer(mut self, tracer: TracerTrain) -> Self {
        self.downcomer_tracer = Some(tracer);
        self
    }

    /// Pass 2: cold helium picked up at the foot and climbing the boreholes.
    ///
    /// Advance with the primary mass flow. Its inlet is the annulus end of
    /// the U-bend.
    pub fn with_riser_tracer(mut self, tracer: TracerTrain) -> Self {
        self.riser_tracer = Some(tracer);
        self
    }

    /// Cold helium converging in the upper cold plenum.
    ///
    /// Advance with the primary mass flow. Its inlets are the plenum's
    /// **outer ends**, where the boreholes deliver, and the two streams run
    /// inward to meet on the axis before turning down into the bed.
    pub fn with_cold_plenum_tracer(mut self, tracer: TracerTrain) -> Self {
        self.cold_plenum_tracer = Some(tracer);
        self
    }

    /// Hot helium crossing the bottom plenum to the duct nozzle.
    ///
    /// Advance with the primary mass flow. Its inlet is the vessel **axis**,
    /// where flow arrives from the bed, and it leaves sideways.
    pub fn with_plenum_tracer(mut self, tracer: TracerTrain) -> Self {
        self.plenum_tracer = Some(tracer);
        self
    }

    /// Hot helium leaving through the coaxial duct's **inner tube**, towards
    /// the steam generator.
    ///
    /// Advance with the primary mass flow. Its inlet is the **vessel** end of
    /// the duct, where the inner tube drains the hot plenum.
    pub fn with_hot_duct_tracer(mut self, tracer: TracerTrain) -> Self {
        self.hot_duct_tracer = Some(tracer);
        self
    }

    /// Cold helium returning through the coaxial duct's **annulus**, from the
    /// steam generator, on both sides of the inner tube.
    ///
    /// Advance with the primary mass flow. Its inlet is the **outboard** end of
    /// the duct, so with positive flow its marks run towards the vessel,
    /// opposite to the hot inner tube's.
    pub fn with_cold_duct_tracer(mut self, tracer: TracerTrain) -> Self {
        self.cold_duct_tracer = Some(tracer);
        self
    }

    /// Run the coaxial duct `points` further past the vessel than its default
    /// `0.16` vessel widths, to reach a steam generator drawn beside it.
    /// Negative values are treated as zero. The duct's own tracer marks span
    /// the whole length, so they run continuously to the far end.
    pub fn with_duct_extension(mut self, points: f32) -> Self {
        self.duct_extension = points.max(0.0);
        self
    }

    /// Where the coaxial duct ends, for a widget whose box is `widget_rect`
    /// (the rect it will be placed in, of size [`Self::size`]). Computed by the same layout code that
    /// paints the duct, so it matches the drawing exactly.
    pub fn duct_port(&self, widget_rect: Rect) -> DuctPort {
        let (coax, hot) =
            VesselLayout::new(widget_rect, self.duct_reach()).coax(self.duct_extension);
        DuctPort {
            end: Pos2::new(coax.right(), coax.center().y),
            outer_height: coax.height(),
            inner_height: hot.height(),
        }
    }

    /// Pebbles being lifted pneumatically up the refuelling chute into the
    /// core, one per launch. Day to day the chute is otherwise EMPTY.
    ///
    /// Launch one with [`PebbleTransits::launch`] (the studio's **[add
    /// pebble]** button) and advance it with the **lift gas** flow and the
    /// lift's transit time. Its inlet is the low end of the chute on the left,
    /// so positive flow carries each pebble up, across and down into the core,
    /// where it leaves; zero flow parks it.
    pub fn with_refuel_pebbles(mut self, pebbles: PebbleTransits) -> Self {
        self.refuel_pebbles = Some(pebbles);
        self
    }

    /// Pebbles being discharged down the defuelling route and out of the
    /// opening, one per launch (the studio's **[remove pebble]** button).
    ///
    /// Drawn over the packed column as a highlighted pebble, so the one
    /// leaving is visible. Its inlet is the foot of the conus; advance it with
    /// the **discharge** rate and a transit time, so positive flow carries it
    /// down the tube, along the dipping leg, and out of the exit tube.
    pub fn with_defuel_pebbles(mut self, pebbles: PebbleTransits) -> Self {
        self.defuel_pebbles = Some(pebbles);
        self
    }

    fn colour(&self, t: ThermodynamicTemperature) -> Color32 {
        temperature_colour(t, self.min_temp, self.max_temp)
    }

    /// A label in a box: grey fill, internals-coloured edge, the text centred.
    /// Every boxed label on the widget uses this, so they all match
    /// (maintainer direction, 2026-09-22). The box grows to fit the text if
    /// `rect` is thinner than the font, and widens to fit the text.
    fn label_box(&self, painter: &Painter, rect: Rect, text: &str) {
        // Wide enough for the text too, when labels are shown.
        let text_w = if self.show_labels {
            painter
                .layout_no_wrap(
                    text.to_owned(),
                    FontId::proportional(LABEL_FONT_SIZE),
                    LABEL,
                )
                .size()
                .x
        } else {
            0.0
        };
        let rect = Rect::from_center_size(
            rect.center(),
            Vec2::new(
                rect.width().max(text_w + 8.0),
                rect.height().max(LABEL_FONT_SIZE + 4.0),
            ),
        );
        painter.rect_filled(rect, 2, LABEL_BOX_GREY);
        painter.rect_stroke(rect, 2, Stroke::new(1.2, INTERNALS), StrokeKind::Middle);
        self.tag(painter, rect.center(), text);
    }

    /// Label text centred on `at`. A multi-line `text` (lines split on `\n`)
    /// is drawn with EACH line centred, one above the other.
    fn tag(&self, painter: &Painter, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        let lines: Vec<&str> = text.lines().collect();
        let line_h = 1.2 * LABEL_FONT_SIZE;
        let first = at.y - 0.5 * line_h * (lines.len() as f32 - 1.0);
        for (i, line) in lines.iter().enumerate() {
            painter.text(
                Pos2::new(at.x, first + i as f32 * line_h),
                egui::Align2::CENTER_CENTER,
                *line,
                FontId::proportional(LABEL_FONT_SIZE),
                LABEL,
            );
        }
    }
}

impl Widget for Htr10ReactorSchematic {
    /// Draws the vessel, the reflector's real radial bands, the three-pass
    /// helium path, the bed with its conus, the hot plenum, the coaxial duct,
    /// the discharge tube and defuelling chute (with pebbles), and
    /// the control rods with their drives.
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(self.size(), Sense::hover());
        self.control_rod_insertion_frac = self.control_rod_insertion_frac.clamp(0.0, 1.0);

        // The drives stand above the head and the defuelling chute exits just
        // below the bottom head, so the vessel gets the middle of the box.
        let layout = VesselLayout::new(response.rect, self.duct_reach());
        // Labels go on the FOREGROUND layer, clipped to the surrounding
        // panel, so no part of the plant schematic can cover them, not even
        // pieces other widgets paint later (maintainer direction, 2026-09-22).
        let labels = ui
            .ctx()
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                response.id.with("htr10_schematic_labels"),
            ))
            .with_clip_rect(ui.clip_rect());
        let (rect, drive_band, chute_band) = (layout.rect, layout.drive_band, layout.chute_band);
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let bore = layout.bore();

        // Plant centimetres to screen, the only two conversions in the body.
        let rx = |radius_cm: f32, side: f32| layout.rx(radius_cm, side);
        let zy = |z_cm: f32| layout.zy(z_cm);

        let cold = self.colour(self.inlet_temp);
        let hot = self.colour(self.outlet_temp);
        let refl = self.colour(self.reflector_temp);

        // ── Pressure vessel ────────────────────────────────────────────────
        let dome = w * 0.5;
        let shell = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + dome * 0.42),
            Pos2::new(rect.right(), rect.bottom() - dome * 0.42),
        );
        let vessel_col = self.colour(self.vessel_temp);
        painter.rect_filled(shell, 0, vessel_col);
        for cap in [
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.right(), shell.top() + dome * 0.30),
            ),
            Rect::from_min_max(
                Pos2::new(rect.left(), shell.bottom() - dome * 0.30),
                Pos2::new(rect.right(), rect.bottom()),
            ),
        ] {
            painter.rect_filled(cap, (dome * 0.5).round().clamp(0.0, 255.0) as u8, vessel_col);
        }
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rx(DRAWN_VESSEL_RADIUS_CM, -1.0), rect.top() + dome * 0.30),
                Pos2::new(rx(DRAWN_VESSEL_RADIUS_CM, 1.0), rect.bottom() - dome * 0.30),
            ),
            (w * 0.04).round() as u8,
            VOID,
        );

        // ── The ceramic internals, by their real radial bands ──────────────
        //
        // Outermost first, so each band is laid over the one outside it. The
        // boronated carbon bricks are a distinct material — B4C in graphite,
        // acting as thermal insulation AND neutron absorber — so they get
        // their own darker tone rather than reading as more reflector.
        let top_z = 0.0;
        let bottom_z = INTERNALS_HEIGHT_CM;
        let band = |r_cm: f32, z0: f32, z1: f32| {
            Rect::from_min_max(
                Pos2::new(rx(r_cm, -1.0), zy(z0)),
                Pos2::new(rx(r_cm, 1.0), zy(z1)),
            )
        };
        painter.rect_filled(
            band(REFLECTOR_OUTER_RADIUS_CM, top_z, bottom_z),
            2,
            BORONATED,
        );
        let graphite_band = band(BORONATED_BRICK_INNER_RADIUS_CM, top_z, bottom_z);
        painter.rect_filled(graphite_band, 2, GRAPHITE);
        painter.rect_filled(
            graphite_band,
            2,
            Color32::from_rgba_unmultiplied(refl.r(), refl.g(), refl.b(), 105),
        );
        painter.rect_stroke(
            band(REFLECTOR_OUTER_RADIUS_CM, top_z, bottom_z),
            2,
            Stroke::new(1.0, INTERNALS),
            StrokeKind::Middle,
        );

        // ── Pass 2: the twenty cold-coolant boreholes ──────────────────────
        //
        // At r = 144.6 cm, each 8 cm across. Drawn as one continuous run per
        // borehole: a U-bend picking the gas up at the foot of the annulus,
        // then the climb. Three per side, nested.
        let channel_half = radius_fraction(COOLANT_CHANNEL_DIAMETER_CM * 0.5) * bore;
        // Where the boreholes deliver: the outer end of the cold plenum. The
        // runs finish inside it, so the marks arriving there are the same ones
        // that left the annulus.
        let plenum_mid_y = zy(0.5 * (COLD_PLENUM_TOP_Z_CM + COLD_PLENUM_BOTTOM_Z_CM));
        // Where the boreholes pick the gas up out of the annulus. The cold
        // descent from the duct ends HERE — past this point the gas is in the
        // boreholes, not still falling down the annulus.
        let borehole_pickup_y = zy(bottom_z - 14.0);
        let mut riser_paths: Vec<Vec<Pos2>> = Vec::new();
        for side in [-1.0_f32, 1.0] {
            for k in 0..DRAWN_RISERS_PER_SIDE {
                // Spread the three about the real centreline radius so the
                // group straddles where the boreholes actually are.
                let offset = (k as f32 - 1.0) * COOLANT_CHANNEL_DIAMETER_CM * 1.6;
                let riser_x = rx(COOLANT_CHANNEL_RADIUS_CM + offset, side);
                let entry_x = rx(
                    0.5 * (REFLECTOR_OUTER_RADIUS_CM + VESSEL_INNER_RADIUS_CM),
                    side,
                );
                // Both ends nest, and in OPPOSITE senses: at the foot the
                // outermost run turns shallowest, at the crown it arches
                // highest. That keeps the three from crossing at either end
                // and lets each one's turn stay visible.
                // Kept close to the internals: the channels are inside the
                // ceramics, and a long overhang draws pipework standing
                // outside the reflector that is not there.
                let bend_y = zy(bottom_z + 8.0 + 11.0 * k as f32);
                let plenum_x = rx(
                    CONTROL_ROD_CHANNEL_RADIUS_CM - 8.0 * k as f32,
                    side,
                );
                riser_paths.push(borehole_path(
                    entry_x,
                    borehole_pickup_y,
                    riser_x,
                    bend_y,
                    plenum_x,
                    plenum_mid_y,
                ));
            }
        }
        let riser_width = (channel_half * 2.0).max(2.0);
        for path in &riser_paths {
            for seg in path.windows(2) {
                painter.line_segment([seg[0], seg[1]], Stroke::new(riser_width, cold));
            }
        }

        // Borehole tracers are drawn HERE, with the channels, rather than with
        // the other tracers at the end.
        //
        // The boreholes run at r = 144.6 cm, which on the right-hand side puts
        // them squarely behind the coaxial duct where it leaves the vessel.
        // Drawn last, their marks would slide ACROSS the duct as though the
        // channel passed in front of it. Drawn now, the duct is painted over
        // them and they pass behind it, which is where the channel is.
        if let Some(train) = &self.riser_tracer {
            for path in &riser_paths {
                for position in train.positions() {
                    painter.circle_filled(
                        point_along(path, position as f32),
                        riser_width * 0.42,
                        Color32::WHITE,
                    );
                }
            }
        }

        // ── Control-rod borings, INBOARD of the coolant channels ───────────
        //
        // r = 102.1 cm against the coolant channels' 144.6 cm. That ordering
        // is the one a schematic most easily gets backwards, so it is drawn
        // from the cited radii rather than by eye, and asserted in the tests.
        let rod_half = radius_fraction(CONTROL_ROD_CHANNEL_DIAMETER_CM * 0.5) * bore;
        let rod_top_z = ROD_TOP_Z_CM;
        let rod_bottom_z = CORE_ZERO_HEIGHT_Z_CM;
        let mut rod_xs = Vec::new();
        for side in [-1.0_f32, 1.0] {
            let x = rx(CONTROL_ROD_CHANNEL_RADIUS_CM, side);
            rod_xs.push(x);
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x - rod_half, zy(rod_top_z)),
                    Pos2::new(x + rod_half, zy(rod_bottom_z)),
                ),
                1,
                VOID,
            );
            let tip_y = zy(rod_top_z + (rod_bottom_z - rod_top_z) * self.control_rod_insertion_frac);
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x - rod_half * 0.8, zy(rod_top_z)),
                    Pos2::new(x + rod_half * 0.8, tip_y),
                ),
                1,
                Color32::from_rgb(196, 200, 208),
            );
        }

        // ── Control rod DRIVES, standing above the head ────────────────────
        //
        // Concentric cylinders: the housing and the pressure-boundary
        // extension inside it, then the ROD itself standing out of them. Each
        // rod's drive sits directly over its boring, so the drives inherit the
        // same cited radius as the channels below them.
        //
        // **The rod stands above the head by exactly the length that has come
        // out of the core** (maintainer direction, 2026-09-22), at the SAME
        // screen scale as the tip's travel inside the vessel, so the length
        // inside and the length outside always add up to the stroke. Both come
        // from `control_rod_insertion_frac`, so they move together.
        let stroke_px = zy(rod_bottom_z) - zy(rod_top_z);
        for x in &rod_xs {
            let head_y = rect.top() + dome * 0.16;
            for (width_mult, height_frac) in DRIVE_STAGE_HEIGHTS {
                let half = rod_half * width_mult;
                let top = head_y - h * height_frac;
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x - half, top),
                        Pos2::new(x + half, head_y),
                    ),
                    (half * 0.5).round() as u8,
                    vessel_col,
                );
                painter.rect_stroke(
                    Rect::from_min_max(
                        Pos2::new(x - half, top),
                        Pos2::new(x + half, head_y),
                    ),
                    (half * 0.5).round() as u8,
                    Stroke::new(1.0, OUTLINE),
                    StrokeKind::Middle,
                );
            }
            // The rod, in the same colour as its length inside the vessel.
            let rod_top = head_y - rod_above_head(self.control_rod_insertion_frac, stroke_px);
            let rod = Rect::from_min_max(
                Pos2::new(x - rod_half * 0.45, rod_top),
                Pos2::new(x + rod_half * 0.45, head_y),
            );
            painter.rect_filled(rod, 1, Color32::from_rgb(196, 200, 208));
            painter.rect_stroke(rod, 1, Stroke::new(1.0, OUTLINE), StrokeKind::Middle);
        }
        if self.show_labels {
            // Boxed like every other label; at the top of the band, clear of a
            // fully withdrawn rod.
            self.label_box(
                &labels,
                Rect::from_center_size(Pos2::new(cx, rect.top() - drive_band + 10.0), Vec2::ZERO),
                &format!("{CONTROL_ROD_CHANNELS} control rod drives"),
            );
        }

        // ── Upper cold plenum ──────────────────────────────────────────────
        // Drawn at the TRUE thickness of one real band from the r-z
        // partition, 9.7 cm, rather than a thicker span chosen to be easy to
        // see. Which band it is remains an assignment — see the module docs.
        let cold_plenum = Rect::from_min_max(
            Pos2::new(rx(CONTROL_ROD_CHANNEL_RADIUS_CM, -1.0), zy(COLD_PLENUM_TOP_Z_CM)),
            Pos2::new(rx(CONTROL_ROD_CHANNEL_RADIUS_CM, 1.0), zy(COLD_PLENUM_BOTTOM_Z_CM)),
        );
        painter.rect_filled(cold_plenum, 1, cold);
        // A boxed label on the band, above the pebble bed, matching the "hot
        // plenum" box below the conus (maintainer direction, 2026-09-22). Its
        // tracers are painted later, so they still run across it.
        let cold_box = Rect::from_center_size(
            cold_plenum.center(),
            Vec2::new(
                cold_plenum.width(),
                (zy(16.0) - zy(0.0)).max(cold_plenum.height()),
            ),
        );
        self.label_box(&labels, cold_box, "cold plenum");

        // ── Core cavity, the pebble bed inside it, and the conus ───────────
        //
        // The bed is measured UP from zero core height (the top of the conus),
        // which is how the benchmark defines a loading. An under-loaded core
        // therefore leaves a visible gas space at the top of the cavity —
        // exactly what the B1 problem is about.
        let cavity = Rect::from_min_max(
            Pos2::new(rx(CORE_RADIUS_CM, -1.0), zy(CORE_CAVITY_TOP_Z_CM)),
            Pos2::new(rx(CORE_RADIUS_CM, 1.0), zy(CORE_ZERO_HEIGHT_Z_CM)),
        );
        painter.rect_filled(cavity, 2, VOID);

        let bed_height = self.drawn_bed_height_cm();
        let bed = Rect::from_min_max(
            Pos2::new(
                rx(CORE_RADIUS_CM, -1.0),
                zy(CORE_ZERO_HEIGHT_Z_CM - bed_height),
            ),
            Pos2::new(rx(CORE_RADIUS_CM, 1.0), zy(CORE_ZERO_HEIGHT_Z_CM)),
        );
        painter.rect_filled(bed, 2, self.colour(self.pebble_temp));

        // Pebbles are drawn TO SCALE: the real 6 cm pebble, the size the DEM
        // bed below was run at (**CHANGED 2026-09-22** from a representative
        // size about 2.2x real). At studio sizes that is around 1.4 pt, where
        // `draw_triso_pebble` draws a plain tinted disc rather than a speckle
        // too small to see.
        let pebble_r = radius_fraction(PEBBLE_RADIUS_M * 100.0) * bore;
        // One pebble, drawn identically everywhere: the SAME design as
        // `htgr_sim_v1`'s HTR-10 vessel, a graphite body speckled with TRISO
        // kernels at the fuel colour (`draw_triso_pebble`, maintainer
        // direction 2026-09-21). Each pebble gets its own index so its
        // speckle differs from its neighbours' but is stable across repaints.
        let kernel = self.colour(self.pebble_temp);
        let pebble_index = std::cell::Cell::new(0_i32);
        let draw_pebble = |at: Pos2| {
            let i = pebble_index.get();
            pebble_index.set(i + 1);
            draw_triso_pebble(&painter, at, pebble_r, PEBBLE_MATRIX, kernel, i);
        };
        // Conus, narrowing to the discharge tube.
        let tube_half = radius_fraction(DISCHARGE_TUBE_RADIUS_CM) * bore;
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(cavity.left(), zy(CORE_ZERO_HEIGHT_Z_CM)),
                Pos2::new(cavity.right(), zy(CORE_ZERO_HEIGHT_Z_CM)),
                Pos2::new(cx + tube_half, zy(CONUS_BOTTOM_Z_CM)),
                Pos2::new(cx - tube_half, zy(CONUS_BOTTOM_Z_CM)),
            ],
            self.colour(self.pebble_temp),
            Stroke::new(1.0, INTERNALS),
        ));
        // Pebbles from DEM, not a pattern (maintainer direction, 2026-09-22):
        // a cut-away slab of the settled HTR-10 conus bed, the run on which
        // this workspace's DEM port and LIGGGHTS agree, at exactly this
        // vessel's dimensions, plus a DEM column baked for the rest of the
        // discharge tube below that run's shortened 0.25 m, and DEM segments
        // settled in place along the dog-leg and exit tube. Together they fill
        // the bed, the conus and the whole chute. See `htr10_conus_packing` for
        // the sources and the cut.
        //
        // Painted straight through the table, farthest first, with the far
        // layer shaded toward the backdrop as `htgr_sim_v1`'s bed is. The
        // bed-height slider crops it: pebbles reaching above the drawn bed
        // top are left out, so an under-loaded bed shows a cut top rather
        // than a settled free surface at that height.
        let bed_top_m = bed_height / 100.0;
        // Where the conus ends and the discharge tube begins, metres below zero
        // core height. Pebbles below it are painted LATER, over the tube's
        // fill: the tube is drawn at the end (it passes through the bottom
        // head), and painting its pebbles here put them under that fill, where
        // they could not be seen (corrected 2026-09-22).
        let tube_top_m = -(CONUS_BOTTOM_Z_CM - CORE_ZERO_HEIGHT_Z_CM) / 100.0;
        let draw_dem_pebbles = |in_tube: bool| {
            for (index, pebble) in CONUS_SLAB.iter().enumerate() {
                let [x, z, y] = *pebble;
                if z + PEBBLE_RADIUS_M > bed_top_m || (z < tube_top_m) != in_tube {
                    continue;
                }
                let at = Pos2::new(
                    rx(x.abs() * 100.0, x.signum()),
                    zy(CORE_ZERO_HEIGHT_Z_CM - z * 100.0),
                );
                let shade = depth_shade(1.0 + y / SLAB_DEPTH_M);
                draw_triso_pebble(
                    &painter,
                    at,
                    pebble_r,
                    blend_rgb(BED_BACKDROP, PEBBLE_MATRIX, shade),
                    blend_rgb(BED_BACKDROP, kernel, shade),
                    index as i32,
                );
            }
        };
        // The bed and the conus now; the tube's pebbles after the tube.
        draw_dem_pebbles(false);
        // Boxed, and kept inside the core boundary (maintainer direction,
        // 2026-09-22). The "DEM packing, to scale" note is on the summary line
        // at the bottom, since it does not fit inside the core.
        self.label_box(
            &labels,
            Rect::from_center_size(bed.center(), Vec2::new(0.0, 0.0)),
            "pebble bed",
        );

        // ── Hot helium plenum, in the bottom reflector ─────────────────────
        let plenum = layout.hot_plenum();
        painter.rect_filled(plenum, 2, hot);
        // Its label is on the boxed band drawn over the tube (see below).

        // ── The coaxial duct nozzle ────────────────────────────────────────
        //
        // ONE connection carrying both streams. Its elevation is why the
        // coolant path is shaped as it is: the hot inner tube has to meet the
        // hot plenum, and the hot plenum is in the BOTTOM reflector — so the
        // duct, cold annulus included, attaches LOW, and cold helium returns
        // near the foot of the vessel rather than at the top.
        //
        // Only its GEOMETRY is fixed here. It is PAINTED further down, after
        // the annulus and the annulus tracers, so that it sits IN FRONT of
        // them: the annulus run starts at the duct centreline, and painted
        // last it would cut vertically across the middle of the duct
        // (maintainer direction, 2026-09-21).
        // Shared with `duct_port`, so a caller connecting to the duct's end
        // gets exactly the geometry painted here.
        let (coax, coax_hot) = layout.coax(self.duct_extension);

        // ── Pass 1: the annulus ────────────────────────────────────────────
        //
        // Drawn ONLY where the gas is actually moving: from the centreline of
        // the coaxial duct, where the cold return enters, down to where the
        // boreholes pick it up. **Maintainer direction, 2026-09-21.**
        //
        // Note what this deliberately does NOT draw. Section 4.2 records the
        // annulus as "filled with 250 degC cold helium to hold vessel
        // temperature below limit" over its whole height, so the real annulus
        // is cold from end to end whether or not gas flows through a given
        // part of it. An earlier version drew that full extent. Showing only
        // the moving segment reads far better — the eye follows one path
        // instead of a tall block with a short live section inside it — at the
        // cost of no longer showing that the whole boundary is bathed in cold
        // helium. That trade is the maintainer's call; do not "restore" the
        // full-height fill as a correctness fix.
        let mut downcomer_rects = Vec::new();
        for side in [-1.0_f32, 1.0] {
            let a = rx(REFLECTOR_OUTER_RADIUS_CM, side);
            let b = rx(VESSEL_INNER_RADIUS_CM, side);
            let run = Rect::from_min_max(
                Pos2::new(a.min(b), coax.center().y),
                Pos2::new(a.max(b), borehole_pickup_y),
            );
            painter.rect_filled(run, 1, cold);
            downcomer_rects.push(run);
        }

        // Tracer marks on a vertical run. `inlet_at_top` is geometry (which
        // end the gas enters); the direction of travel comes from the train.
        let vertical_marks = |run: Rect, train: &TracerTrain, inlet_at_top: bool| {
            let mark_h = (run.height() * 0.045).max(2.0);
            for position in train.positions() {
                let f = if inlet_at_top {
                    position as f32
                } else {
                    1.0 - position as f32
                };
                let yc = run.top() + run.height() * f;
                let a = (yc - 0.5 * mark_h).max(run.top());
                let b = (yc + 0.5 * mark_h).min(run.bottom());
                if b - a < 0.5 {
                    continue;
                }
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(run.left() + 0.5, a),
                        Pos2::new(run.right() - 0.5, b),
                    ),
                    0,
                    Color32::WHITE,
                );
            }
        };

        // Tracer marks on a horizontal run. `inlet_at_left` is geometry; the
        // direction of travel comes from the train.
        let horizontal_marks = |run: Rect, train: &TracerTrain, inlet_at_left: bool| {
            let mark_w = (run.width() * 0.035).max(2.0);
            for position in train.positions() {
                let f = if inlet_at_left {
                    position as f32
                } else {
                    1.0 - position as f32
                };
                let xc = run.left() + run.width() * f;
                let a = (xc - 0.5 * mark_w).max(run.left());
                let b = (xc + 0.5 * mark_w).min(run.right());
                if b - a < 0.5 {
                    continue;
                }
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(a, run.top() + 0.5),
                        Pos2::new(b, run.bottom() - 0.5),
                    ),
                    0,
                    Color32::WHITE,
                );
            }
        };

        // Annulus marks are painted HERE, before the duct, for the same reason
        // the borehole marks are painted with the channels: the right-hand run
        // starts behind the duct, and the duct must cover it.
        if let Some(train) = &self.downcomer_tracer {
            for r in &downcomer_rects {
                vertical_marks(*r, train, true);
            }
        }

        // ── The coaxial duct, painted now so it sits in front ──────────────
        painter.rect_filled(coax, 2, cold);
        painter.rect_filled(coax_hot, 2, hot);

        // Hot helium leaves through the inner tube: its inlet is the VESSEL
        // end (left), where it drains the hot plenum.
        if let Some(train) = &self.hot_duct_tracer {
            horizontal_marks(coax_hot, train, true);
        }
        // Cold helium returns through the annulus, above and below the inner
        // tube: its inlet is the OUTBOARD end (right), from the steam
        // generator.
        if let Some(train) = &self.cold_duct_tracer {
            let upper = Rect::from_min_max(coax.min, Pos2::new(coax.right(), coax_hot.top()));
            let lower = Rect::from_min_max(Pos2::new(coax.left(), coax_hot.bottom()), coax.max);
            horizontal_marks(upper, train, false);
            horizontal_marks(lower, train, false);
        }
        painter.rect_stroke(coax, 2, Stroke::new(1.0, INTERNALS), StrokeKind::Middle);

        // (The fuel discharge tube and defuelling chute are drawn at the end,
        // in front of the vessel outline they pass through.)

        // ── Tracers (the annulus and duct marks were painted above) ────────
        if let Some(train) = &self.cold_plenum_tracer {
            let mark_w = (cold_plenum.width() * 0.045).max(1.5);
            let half = cold_plenum.width() * 0.5;
            for position in train.positions() {
                for side in [-1.0_f32, 1.0] {
                    let xc = cx + side * half * (1.0 - position as f32);
                    let a = (xc - 0.5 * mark_w).max(cold_plenum.left());
                    let b = (xc + 0.5 * mark_w).min(cold_plenum.right());
                    if b - a < 0.5 {
                        continue;
                    }
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(a, cold_plenum.top() + 1.0),
                            Pos2::new(b, cold_plenum.bottom() - 1.0),
                        ),
                        0,
                        Color32::WHITE,
                    );
                }
            }
        }

        if let Some(train) = &self.plenum_tracer {
            let mark_w = (plenum.width() * 0.05).max(1.5);
            for position in train.positions() {
                for side in [-1.0_f32, 1.0] {
                    let xc = cx + side * (plenum.width() * 0.5) * position as f32;
                    let a = (xc - 0.5 * mark_w).max(plenum.left());
                    let b = (xc + 0.5 * mark_w).min(plenum.right());
                    if b - a < 0.5 {
                        continue;
                    }
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(a, plenum.top() + 1.0),
                            Pos2::new(b, plenum.bottom() - 1.0),
                        ),
                        0,
                        Color32::WHITE,
                    );
                }
            }
        }

        painter.rect_stroke(shell, 0, Stroke::new(1.5, OUTLINE), StrokeKind::Middle);

        // ── Fuel discharge tube and defuelling chute ───────────────────────
        //
        // Drawn AFTER the vessel outline, because the exit tube passes through
        // the bottom head: the cited ~3.3 m discharge tube, an inverted-L leg
        // to the left dipping 15 degrees, then a narrow exit tube down through
        // the bottom head, left OPEN just outside the vessel. Only the 3.3 m
        // tube is cited; the rest is a drawing choice (maintainer direction,
        // 2026-09-21). Each piece matches one DEM segment (2026-09-22).
        //
        // Each piece is filled shapes with square ends, so the exit reads as
        // open with no extra drawing.
        let pebble_fill = self.colour(self.pebble_temp);
        // One piece of the route: its walls, its fill, and pebbles placed by
        // arc length in rows across it, the same size as the bed's. Like the
        // bed's, they show WHAT fills the channel, not how many pebbles do.
        // `filled` false draws an EMPTY pipe (walls and a void bore), for a
        // route pebbles only pass through one at a time.
        let draw_route_piece = |line: &[Pos2], widths: &[f32], filled: bool| {
            // Filled quads with round joints, not a thick stroke, so the
            // bends are smooth rather than notched (maintainer, 2026-09-22).
            let walls: Vec<f32> = widths.iter().map(|w| w + 2.4).collect();
            paint_tapered_pipe(&painter, line, &walls, INTERNALS);
            paint_tapered_pipe(
                &painter,
                line,
                widths,
                if filled { pebble_fill } else { VOID },
            );
            // No pattern pebbles: the whole chute is filled by the DEM table,
            // painted over this fill (maintainer direction, 2026-09-22).
        };
        // THREE straight pieces, each exactly one DEM segment, rather than one
        // smooth path the DEM did not fill (maintainer direction, 2026-09-22):
        // the vertical tube down to the tube column's floor, the dog-leg pipe
        // with square ends cut perpendicular to it (the DEM leg's end caps),
        // and the narrow exit tube from below the leg to the opening. Where
        // two segments meet there are no pebbles, so each joint is drawn as a
        // GREYED-OUT elbow instead, and carries the chute labels.
        //
        // Positions are in the DEM's frame (cm, x signed, z up from zero core
        // height) and mapped with the same `rx`/`zy` as everything else; the
        // table test checks the bake matches these pieces.
        let dem = |x_cm: f32, z_cm: f32| {
            Pos2::new(
                rx(x_cm.abs(), x_cm.signum()),
                zy(CORE_ZERO_HEIGHT_Z_CM - z_cm),
            )
        };
        let cone_h = CONUS_BOTTOM_Z_CM - CORE_ZERO_HEIGHT_Z_CM;
        let tube_end_z = -(cone_h + DISCHARGE_TUBE_LENGTH_FRACTION * VESSEL_HEIGHT_CM);
        let dip = CHUTE_DIP_DEG.to_radians();
        let leg_run = CHUTE_LEG_RUN_FRACTION * DRAWN_VESSEL_RADIUS_CM;
        let (leg_end_x, leg_end_z) = (-leg_run, tube_end_z - leg_run * dip.tan());
        // Unit normal to the leg, in the DEM frame: the caps run along it.
        let (nx, nz) = (dip.sin(), -dip.cos());
        let r_tube = DISCHARGE_TUBE_RADIUS_CM;
        // The exit tube starts below the leg's lower wall at its end.
        let exit_top_z = leg_end_z - r_tube / dip.cos();
        // The opening: just outside the bottom head, below the leg's end.
        let exit_floor = Pos2::new(dem(leg_end_x, 0.0).x, rect.bottom() + 0.8 * chute_band);
        let wide = 2.0 * tube_half;
        // 1.2 pebble diameters: the DEM exit tube's width, so its single file
        // of pebbles does not jam.
        let narrow = 2.4 * pebble_r;

        let tube = [dem(0.0, -cone_h), dem(0.0, tube_end_z)];
        let leg = [dem(0.0, tube_end_z), dem(leg_end_x, leg_end_z)];
        let exit = [dem(leg_end_x, exit_top_z), exit_floor];
        draw_route_piece(&tube, &[wide, wide], true);
        draw_route_piece(&leg, &[wide, wide], true);
        draw_route_piece(&exit, &[narrow, narrow], true);

        // The two joints: convex quads spanning the gap between one
        // segment's end and the next one's start.
        let tube_leg_joint = [
            dem(-r_tube, tube_end_z),
            dem(-r_tube * nx, tube_end_z - r_tube * nz),
            dem(r_tube, tube_end_z),
            dem(r_tube * nx, tube_end_z + r_tube * nz),
        ];
        let leg_exit_joint = [
            dem(leg_end_x - r_tube * nx, leg_end_z - r_tube * nz),
            dem(leg_end_x + r_tube * nx, leg_end_z + r_tube * nz),
            dem(leg_end_x + 1.2 * PEBBLE_RADIUS_M * 100.0, exit_top_z),
            dem(leg_end_x - 1.2 * PEBBLE_RADIUS_M * 100.0, exit_top_z),
        ];
        // The chute's DEM pebbles, over its fill (see `draw_dem_pebbles`).
        draw_dem_pebbles(true);

        // ONE rectangular boxed label covering the whole elbow, from the
        // tube-to-leg joint along the leg to the leg-to-exit joint, drawn over
        // the pebbles so none of the joints or seams show (maintainer
        // direction, 2026-09-22).
        let elbow = Rect::from_points(&tube_leg_joint)
            .union(Rect::from_points(&leg_exit_joint))
            .expand(3.0);
        // Two lines, as the maintainer asked: "discharge tube" / "to opening".
        self.label_box(&labels, elbow, "discharge tube\nto opening");

        // The seam under the conus, between the bed run's shortened tube and
        // the column baked below it, is covered by the hot plenum band the
        // tube passes through, drawn as a labelled box (maintainer direction,
        // 2026-09-22).
        let plenum_box = Rect::from_min_max(
            Pos2::new(plenum.left(), zy(CONUS_BOTTOM_Z_CM + 18.0)),
            Pos2::new(plenum.right(), zy(CONUS_BOTTOM_Z_CM + 34.0)),
        );
        self.label_box(&labels, plenum_box, "hot plenum");

        // The whole discharge path, for a pebble being removed.
        let discharge = [tube[0], tube[1], leg[1], exit[0], exit[1]];

        // ── Refuelling chute ───────────────────────────────────────────────
        //
        // One pebble wide. It enters through the bottom head from OUTSIDE the
        // vessel (maintainer correction, 2026-09-21), then runs up the gap the
        // widened vessel leaves between the cold annulus and the wall on the
        // LEFT, to the top-left of the inner vessel, then a leg to the
        // centreline dipping about 15 degrees, then straight down the axis,
        // past the upper plenum, ending at the top of the core cavity, in the
        // gas space above the bed (corrected 2026-09-22; it went down onto the
        // bed surface before)
        // (maintainer direction, 2026-09-21). Drawn last, so it passes in
        // front of the plenum. With the defuelling chute it closes the pebble
        // recirculation loop. The route is a drawing choice; see
        // `refuelling_route` and `REFUEL_CHUTE_ALLOWANCE_CM`.
        let refuel_x = rx(
            0.5 * (VESSEL_INNER_RADIUS_CM + DRAWN_VESSEL_RADIUS_CM),
            -1.0,
        );
        let refuel = refuelling_route(
            // Starts OUTSIDE the vessel, below the bottom head, where pebbles
            // are fed to the lift (maintainer correction, 2026-09-21).
            Pos2::new(refuel_x, rect.bottom() + 0.8 * chute_band),
            rect.top() + dome * 0.35,
            cx,
            REFUEL_DIP_DEG,
            // Ends at the top of the core cavity, opening into the gas space
            // ABOVE the pebbles; it never reaches down to the bed surface
            // (maintainer correction, 2026-09-22). A pebble leaving it falls
            // through the gas onto the bed.
            zy(CORE_CAVITY_TOP_Z_CM),
        );
        // EMPTY day to day: pebbles are lifted pneumatically one at a time,
        // so the chute is drawn as a bare pipe and only the pebble in transit
        // is shown (maintainer direction, 2026-09-21).
        draw_route_piece(&refuel, &vec![2.0 * pebble_r; refuel.len()], false);
        // A pebble in transit, highlighted so it reads against a packed
        // column as well as in an empty chute.
        // Same TRISO design, plus a white ring so the moving one stands out.
        let draw_moving_pebble = |at: Pos2| {
            draw_pebble(at);
            painter.circle_stroke(at, pebble_r, Stroke::new(1.6, Color32::WHITE));
        };
        if let Some(pebbles) = &self.refuel_pebbles {
            for position in pebbles.positions() {
                draw_moving_pebble(point_along(&refuel, position as f32));
            }
        }
        if let Some(pebbles) = &self.defuel_pebbles {
            // The same discharge path the pipe is painted along.
            for position in pebbles.positions() {
                draw_moving_pebble(point_along(&discharge, position as f32));
            }
        }
        // Boxed like every other label, on two lines (maintainer direction,
        // 2026-09-22).
        self.label_box(
            &labels,
            Rect::from_center_size(
                Pos2::new(refuel_x + w * 0.13, rect.top() + dome * 0.35 - 7.0),
                Vec2::ZERO,
            ),
            "refuelling\nchute",
        );

        if self.show_labels {
            self.tag(
                &labels,
                Pos2::new(cx, rect.bottom() - h * 0.022),
                &format!(
                    "bed {:.0} cm of {:.0} cm cavity · pebbles DEM, to scale · {COOLANT_BOREHOLES} boreholes (3/side drawn)",
                    bed_height, CORE_CAVITY_HEIGHT_CM
                ),
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    fn kelvins(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    fn visual() -> Htr10ReactorSchematic {
        Htr10ReactorSchematic::new(
            Vec2::new(200.0, 620.0),
            kelvins(300.0),
            kelvins(1200.0),
            kelvins(1100.0),
            kelvins(523.15),
            kelvins(973.15),
            kelvins(900.0),
            kelvins(530.0),
        )
    }

    /// **The control rods sit INBOARD of the coolant boreholes.**
    ///
    /// This is the single most invertible fact in the radial partition, and an
    /// earlier version of this widget had it backwards — rods drawn outboard
    /// of the risers. Asserted from the cited radii so it cannot silently flip
    /// back.
    #[test]
    fn control_rods_are_inboard_of_the_coolant_boreholes() {
        assert!(
            CONTROL_ROD_CHANNEL_RADIUS_CM < COOLANT_CHANNEL_RADIUS_CM,
            "rods at {CONTROL_ROD_CHANNEL_RADIUS_CM} cm must be inboard of \
             coolant channels at {COOLANT_CHANNEL_RADIUS_CM} cm"
        );
        // And both sit outside the core, inside the ceramics.
        assert!(CORE_RADIUS_CM < CONTROL_ROD_CHANNEL_RADIUS_CM);
        assert!(COOLANT_CHANNEL_RADIUS_CM < BORONATED_BRICK_INNER_RADIUS_CM);
        assert!(BORONATED_BRICK_INNER_RADIUS_CM < REFLECTOR_OUTER_RADIUS_CM);
    }

    /// The radial partition is strictly increasing and fits inside the vessel.
    #[test]
    fn the_radial_partition_is_ordered_and_fits() {
        let boundaries = [
            DISCHARGE_TUBE_RADIUS_CM,
            CORE_RADIUS_CM,
            CONTROL_ROD_CHANNEL_RADIUS_CM,
            COOLANT_CHANNEL_RADIUS_CM,
            BORONATED_BRICK_INNER_RADIUS_CM,
            REFLECTOR_OUTER_RADIUS_CM,
            VESSEL_INNER_RADIUS_CM,
        ];
        for pair in boundaries.windows(2) {
            assert!(pair[0] < pair[1], "{} should be inside {}", pair[0], pair[1]);
        }
        // The ceramics nearly fill the bore: 190 cm of a 200 cm radius. The
        // annulus is the remainder, and it is genuinely thin.
        let annulus = VESSEL_INNER_RADIUS_CM - REFLECTOR_OUTER_RADIUS_CM;
        assert!(annulus > 0.0 && annulus < 0.1 * VESSEL_INNER_RADIUS_CM);
    }

    /// The axial spans reproduce the three Table 2 heights the source
    /// corroborates exactly.
    #[test]
    fn the_axial_spans_match_table_two() {
        assert!((CORE_CAVITY_HEIGHT_CM - 221.818).abs() < 1e-3);
        assert!((CONUS_BOTTOM_Z_CM - CORE_ZERO_HEIGHT_Z_CM - 36.946).abs() < 1e-3);
        assert!((CRITICAL_BED_HEIGHT_CM - 123.06).abs() < 1e-3);
        assert!((INTERNALS_HEIGHT_CM - 610.0).abs() < 1e-6);
    }

    /// The equilibrium bed is far taller than the first-criticality loading,
    /// and both fit the cavity.
    #[test]
    fn both_loadings_fit_the_core_cavity() {
        assert!(CRITICAL_BED_HEIGHT_CM < EQUILIBRIUM_BED_HEIGHT_CM);
        assert!(EQUILIBRIUM_BED_HEIGHT_CM < CORE_CAVITY_HEIGHT_CM);
    }

    /// An over- or under-set bed height is clamped into the cavity rather than
    /// drawn spilling out of it.
    #[test]
    fn the_bed_height_is_clamped_to_the_cavity() {
        assert!(
            (visual().with_bed_height_cm(9_000.0).drawn_bed_height_cm()
                - CORE_CAVITY_HEIGHT_CM)
                .abs()
                < 1e-3
        );
        assert!(visual().with_bed_height_cm(-5.0).drawn_bed_height_cm() == 0.0);
        assert!(
            (visual().drawn_bed_height_cm() - EQUILIBRIUM_BED_HEIGHT_CM).abs() < 1e-6
        );
    }

    /// Control rods default to fully inserted: a caller that forgets to drive
    /// them draws a shut-down core, never a critical one.
    #[test]
    fn control_rods_default_to_inserted() {
        assert!((visual().control_rod_insertion_frac - 1.0).abs() < 1e-6);
    }

    /// The vessel keeps its DRAWN slenderness whatever box it is given, and
    /// the native box leaves room above it for the drives. (**CHANGED
    /// 2026-09-21**: the drawn aspect is now [`DRAWN_ASPECT_RATIO`], widened
    /// for the refuelling chute; it was the real [`HTR10_RPV_ASPECT_RATIO`].)
    #[test]
    fn the_vessel_letterboxes_and_leaves_room_for_drives() {
        for size in [Vec2::new(900.0, 300.0), Vec2::new(100.0, 900.0)] {
            let r = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, size));
            assert!((r.width() / r.height() - DRAWN_ASPECT_RATIO).abs() < 1e-4);
        }
        let native = Htr10ReactorSchematic::native_size(220.0);
        let vessel_only = 220.0 / DRAWN_ASPECT_RATIO;
        assert!(
            native.y > vessel_only,
            "the native box must be taller than the vessel to fit the drives"
        );
    }

    /// Only three boreholes per side are drawn, and the real count is carried
    /// separately so the label can state it rather than a reader counting.
    #[test]
    fn the_drawn_borehole_count_is_not_the_real_one() {
        assert_eq!(COOLANT_BOREHOLES, 20);
        assert_eq!(CONTROL_ROD_CHANNELS, 10);
        assert!(DRAWN_RISERS_PER_SIDE * 2 < COOLANT_BOREHOLES);
    }

    /// The foot is a real U — it overshoots its turn elevation.
    ///
    /// Screen coordinates, so `y` grows downward and the foot bend must reach
    /// *below* where it turns. Asserting the overshoot rather than just the
    /// endpoints catches a bend that has silently collapsed into a straight
    /// joint, which looks mitred and animates like a mark teleporting.
    #[test]
    fn the_foot_bend_overshoots_its_turn() {
        let (entry_x, entry_y) = (190.0_f32, 600.0_f32);
        let (riser_x, bottom_bend_y) = (140.0_f32, 640.0_f32);
        let (plenum_x, plenum_y) = (100.0_f32, 104.0_f32);
        let path = borehole_path(
            entry_x,
            entry_y,
            riser_x,
            bottom_bend_y,
            plenum_x,
            plenum_y,
        );

        assert_eq!(*path.first().unwrap(), Pos2::new(entry_x, entry_y));
        assert_eq!(*path.last().unwrap(), Pos2::new(plenum_x, plenum_y));

        let lowest = path.iter().fold(f32::MIN, |m, p| m.max(p.y));
        assert!(
            lowest > bottom_bend_y,
            "the foot bend must reach below {bottom_bend_y}, got {lowest}"
        );
    }

    /// **The run never rises above the plenum it enters.**
    ///
    /// It turns through 90 degrees and goes in level. An earlier version
    /// arched over the top like pipework and came back down, which drew plant
    /// that is not there — these are channels through graphite opening into a
    /// cavity. Asserted so the arch cannot creep back.
    #[test]
    fn the_run_turns_into_the_plenum_without_arching_over_it() {
        let plenum_y = 104.0_f32;
        let path = borehole_path(190.0, 600.0, 140.0, 640.0, 100.0, plenum_y);
        let highest = path.iter().fold(f32::MAX, |m, p| m.min(p.y));
        assert!(
            highest >= plenum_y - 1e-3,
            "nothing may rise above the plenum at {plenum_y}, got {highest}"
        );
    }

    /// The native box must leave room below the vessel for the chute's open
    /// end, as well as above it for the drives.
    #[test]
    fn the_native_box_has_room_for_the_chute_below_the_vessel() {
        let native = Htr10ReactorSchematic::native_size(220.0);
        let vessel_only = 220.0 / DRAWN_ASPECT_RATIO;
        let expected = vessel_only * (1.0 + DRIVE_BAND_FRACTION + CHUTE_BAND_FRACTION);
        assert!((native.y - expected).abs() < 1e-3);
    }

    /// Arc-length placement hits both ends exactly and stays on the path.
    #[test]
    fn point_along_spans_the_whole_run() {
        let path = borehole_path(190.0, 600.0, 140.0, 640.0, 100.0, 104.0);
        assert_eq!(point_along(&path, 0.0), *path.first().unwrap());
        assert_eq!(point_along(&path, 1.0), *path.last().unwrap());
        // Clamped, not wrapped or extrapolated.
        assert_eq!(point_along(&path, -3.0), *path.first().unwrap());
        assert_eq!(point_along(&path, 9.0), *path.last().unwrap());
    }

    /// The refuelling route rises up the left, dips 15 degrees to the axis,
    /// then drops straight down the axis to `end_y`, staying inside the box
    /// it was given.
    #[test]
    fn the_refuelling_route_rises_left_dips_to_the_axis_then_drops_in() {
        let start = Pos2::new(20.0, 500.0);
        let (top_y, centre_x, dip, end_y) = (40.0, 110.0, 15.0_f32, 200.0);
        let route = refuelling_route(start, top_y, centre_x, dip, end_y);
        assert_eq!(route.len(), 4);

        assert_eq!(
            route[1],
            Pos2::new(start.x, top_y),
            "first leg rises vertically"
        );
        let leg = route[2] - route[1];
        assert!(leg.x > 0.0 && leg.y > 0.0, "the leg runs in and down");
        let measured = (leg.y / leg.x).atan().to_degrees();
        assert!((measured - dip).abs() < 1e-3, "dip {measured} deg");
        assert_eq!(route[2].x, centre_x, "the leg ends on the centreline");
        assert_eq!(
            route[3],
            Pos2::new(centre_x, end_y),
            "then straight down the axis"
        );
    }

    /// The duct port is the painted duct's own end: extending the duct moves
    /// the end right by exactly the extension, at the same height, and the
    /// hot inner tube is narrower than the body around it.
    #[test]
    fn the_duct_port_follows_the_extension_exactly() {
        let origin = Pos2::new(40.0, 10.0);
        let v = visual();
        let base = v.duct_port(Rect::from_min_size(origin, v.size()));
        let l = visual().with_duct_extension(55.0);
        let longer = l.duct_port(Rect::from_min_size(origin, l.size()));
        let r = Rect::from_min_size(origin, v.size());
        assert!(
            (l.size().x - v.size().x - 55.0).abs() < 1e-3,
            "the widget box grows with the duct, so the duct is never clipped"
        );
        assert!(
            base.end.x <= r.right() + 1e-3,
            "the duct never runs past the widget box, where it would be clipped"
        );
        // In a box of the widget's native proportions it ends exactly at the
        // edge.
        let native = Htr10ReactorSchematic {
            size: Htr10ReactorSchematic::native_size(200.0),
            ..visual()
        };
        let nr = Rect::from_min_size(origin, native.size());
        assert!(
            (native.duct_port(nr).end.x - nr.right()).abs() < 1e-3,
            "native box: the duct ends exactly at the edge"
        );
        assert!((longer.end.x - base.end.x - 55.0).abs() < 1e-3);
        assert!((longer.end.y - base.end.y).abs() < 1e-6);
        assert!(base.inner_height < base.outer_height);
        assert!(base.end.x > r.center().x, "the duct leaves on the right");
        let s = visual().with_duct_extension(-10.0);
        let shorter = s.duct_port(Rect::from_min_size(origin, s.size()));
        assert_eq!(shorter, base, "a negative extension is treated as zero");
    }

    /// How many circles one repaint costs, at the studio's full size and at
    /// the mini card's, now that every pebble uses the TRISO design.
    ///
    /// A measurement, not a performance gate: `htgr_sim_v1`'s vessel moved
    /// its bed to a baked texture once direct circles reached ~20 000 per
    /// frame and the GUI was reported laggy (see `pebble_bed_texture`). The
    /// numbers are printed so the cost of this widget is known rather than
    /// guessed; run with `-- --nocapture`. The assertion only checks that
    /// pebbles were drawn at all.
    ///
    /// **Measured 2026-09-22, all-DEM chute** (equilibrium bed): **1 859** (1 848
    /// with the tube column and pattern leg; 1 646 before the tube column)
    /// circles at both a 220 pt vessel (the studio page) and 130 pt (the mini
    /// card). The pebbles are now real size, about 1.4 pt at 220 pt, which is
    /// below the size where `draw_triso_pebble` draws a speckle, so each is a
    /// single disc. (Earlier the same day, representative-size pebbles with
    /// speckles: 8 038 and 556; 2026-09-21: 8 122 and 524.) The full-size figure is
    /// about 40 % of the ~20 000 that made `htgr_sim_v1` laggy; if this page
    /// is reported slow, the bed can move to the same baked-texture path.
    #[test]
    fn circle_count_per_repaint_is_measured() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 1400.0))),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let _ = ctx.run_ui(input.clone(), |_| {});
        for vessel_width in [220.0_f32, 130.0] {
            let size = Htr10ReactorSchematic::native_size(vessel_width);
            let output = ctx.run_ui(input.clone(), |ui| {
                let v = Htr10ReactorSchematic {
                    size,
                    ..visual().with_bed_height_cm(EQUILIBRIUM_BED_HEIGHT_CM)
                };
                ui.put(Rect::from_min_size(Pos2::new(10.0, 10.0), size), v);
            });
            let circles = output
                .shapes
                .iter()
                .filter(|c| matches!(c.shape, egui::Shape::Circle(_)))
                .count();
            println!("vessel width {vessel_width} pt: {circles} circles per repaint");
            assert!(circles > 100, "pebbles should be drawn: {circles}");
        }
    }

    /// The rod above the head is exactly the length withdrawn from the core:
    /// inside plus outside always equals the stroke, fully inserted leaves
    /// nothing above, and the drive band is tall enough to hold a fully
    /// withdrawn rod without clipping it.
    #[test]
    fn the_rod_above_the_head_is_exactly_the_length_withdrawn() {
        let stroke = 311.8_f32;
        for i in 0..=10 {
            let f = i as f32 / 10.0;
            let inside = f * stroke;
            let outside = rod_above_head(f, stroke);
            assert!((inside + outside - stroke).abs() < 1e-3, "at f = {f}");
        }
        assert_eq!(rod_above_head(1.0, stroke), 0.0, "inserted: nothing above");
        assert_eq!(
            rod_above_head(0.0, stroke),
            stroke,
            "withdrawn: whole stroke"
        );
        assert_eq!(rod_above_head(-1.0, stroke), stroke);
        assert_eq!(rod_above_head(2.0, stroke), 0.0);
        assert!(
            DRIVE_BAND_FRACTION > ROD_STROKE_CM / VESSEL_HEIGHT_CM,
            "the drive band must hold the full stroke"
        );
    }

    /// The DEM table fills this vessel and its whole chute, cm for cm, and sits
    /// on the ROUTE THE SCHEMATIC DRAWS: every pebble is inside the barrel,
    /// the conus taper or the vertical tube; or inside the 25 cm dog-leg pipe
    /// along the drawn 15-degree leg; or in the narrow exit tube down to the
    /// drawn opening. The route here is computed from this widget's own
    /// drawing constants, so if the bake and the drawing ever disagree, this
    /// fails. Also: sorted farthest first, and the leg and exit are populated.
    #[test]
    fn the_dem_table_fills_the_drawn_vessel_and_chute() {
        let slack = 0.05;
        let cone_h = CONUS_BOTTOM_Z_CM - CORE_ZERO_HEIGHT_Z_CM;
        let tube_end_z = -(cone_h + DISCHARGE_TUBE_LENGTH_FRACTION * VESSEL_HEIGHT_CM);
        let leg_run = CHUTE_LEG_RUN_FRACTION * DRAWN_VESSEL_RADIUS_CM;
        let leg_end = (
            -leg_run,
            tube_end_z - leg_run * CHUTE_DIP_DEG.to_radians().tan(),
        );
        let opening_below_top = (1.0 - INTERNALS_TOP_FRACTION) * VESSEL_HEIGHT_CM
            + 0.8 * CHUTE_BAND_FRACTION * VESSEL_HEIGHT_CM;
        let exit_floor_z = -(opening_below_top - CORE_ZERO_HEIGHT_Z_CM);
        let exit_radius = 1.2 * PEBBLE_RADIUS_M * 100.0;

        let in_vessel = |x: f32, z: f32| {
            if z < tube_end_z - slack {
                return false;
            }
            let wall = if z >= 0.0 {
                CORE_RADIUS_CM
            } else if z >= -cone_h {
                CORE_RADIUS_CM + (DISCHARGE_TUBE_RADIUS_CM - CORE_RADIUS_CM) * (-z / cone_h)
            } else {
                DISCHARGE_TUBE_RADIUS_CM
            };
            x.abs() <= wall + slack
        };
        let in_leg = |x: f32, z: f32| {
            // Distance from (x, z) to the leg's centreline segment.
            let (ax, az, bx, bz) = (0.0, tube_end_z, leg_end.0, leg_end.1);
            let (dx, dz) = (bx - ax, bz - az);
            let t = (((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
            let (px, pz) = (ax + t * dx, az + t * dz);
            ((x - px).powi(2) + (z - pz).powi(2)).sqrt() <= DISCHARGE_TUBE_RADIUS_CM + slack
        };
        let in_exit = |x: f32, z: f32| {
            (x - leg_end.0).abs() <= exit_radius + slack
                && z >= exit_floor_z - slack
                && z <= leg_end.1
        };

        let (mut on_leg, mut on_exit) = (0, 0);
        for p in CONUS_SLAB {
            let (x, z, y) = (p[0] * 100.0, p[1] * 100.0, p[2] * 100.0);
            let (v, l, e) = (in_vessel(x, z), in_leg(x, z), in_exit(x, z));
            assert!(
                v || l || e,
                "off the drawn vessel and chute: x = {x}, z = {z}"
            );
            if l && !v {
                on_leg += 1;
            }
            if e && !l {
                on_exit += 1;
            }
            assert!(
                y <= 0.0 && y >= -SLAB_DEPTH_M * 100.0,
                "outside the slab: {y}"
            );
        }
        assert!(on_leg > 50, "the dog-leg is populated: {on_leg}");
        assert!(on_exit > 20, "the exit tube is populated: {on_exit}");
        assert!(
            CONUS_SLAB.windows(2).all(|w| w[0][2] <= w[1][2]),
            "sorted farthest first"
        );
        assert_eq!(CONUS_SLAB.len(), 1828);
    }

    /// The discharge tube's DEM pebbles are painted OVER the tube's fill, not
    /// under it. They were once drawn with the bed, before the tube, and the
    /// tube's solid fill then hid every one of them (2026-09-22). Renders the
    /// widget headlessly and checks that, at a tube pebble's centre, its
    /// circle comes after every filled polygon covering that point.
    #[test]
    fn tube_dem_pebbles_are_painted_over_the_tube_fill() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 1600.0))),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let _ = ctx.run_ui(input.clone(), |_| {});
        let size = Htr10ReactorSchematic::native_size(220.0);
        let origin = Pos2::new(10.0, 10.0);
        let output = ctx.run_ui(input, |ui| {
            let v = Htr10ReactorSchematic {
                size,
                ..visual().with_bed_height_cm(EQUILIBRIUM_BED_HEIGHT_CM)
            };
            let r = Rect::from_min_size(origin, v.size());
            ui.put(r, v);
        });

        // Where one tube pebble lands on screen, by the widget's own layout.
        let probe = visual();
        let widget = Htr10ReactorSchematic { size, ..probe };
        let layout = VesselLayout::new(
            Rect::from_min_size(origin, widget.size()),
            widget.duct_reach(),
        );
        let tube_top_m = -(CONUS_BOTTOM_Z_CM - CORE_ZERO_HEIGHT_Z_CM) / 100.0;
        let p = CONUS_SLAB
            .iter()
            .find(|p| p[1] < tube_top_m - 0.5)
            .expect("a pebble well down the tube");
        let at = Pos2::new(
            layout.rx(p[0].abs() * 100.0, p[0].signum()),
            layout.zy(CORE_ZERO_HEIGHT_Z_CM - p[1] * 100.0),
        );

        let mut last_fill_over = None;
        let mut pebble_at = None;
        for (i, clipped) in output.shapes.iter().enumerate() {
            match &clipped.shape {
                egui::Shape::Path(path)
                    if path.closed
                        && path.fill != Color32::TRANSPARENT
                        && Rect::from_points(&path.points).contains(at) =>
                {
                    last_fill_over = Some(i);
                }
                egui::Shape::Circle(c) if c.center.distance(at) < 0.5 => pebble_at = Some(i),
                _ => {}
            }
        }
        let pebble_at = pebble_at.expect("the tube pebble is drawn");
        let fill = last_fill_over.expect("the tube is filled at that point");
        assert!(
            pebble_at > fill,
            "tube pebble painted at {pebble_at}, under a fill painted at {fill}"
        );
    }

    /// Widening the drawn vessel for the refuelling chute must leave the
    /// cited proportions alone, and leave room for a one-pebble chute.
    #[test]
    fn widening_for_the_refuelling_chute_keeps_the_real_vessel_data() {
        assert_eq!(
            VESSEL_INNER_RADIUS_CM, 200.0,
            "the cited radius is unchanged"
        );
        assert!(
            (HTR10_RPV_ASPECT_RATIO - 2.0 * 200.0 / VESSEL_HEIGHT_CM).abs() < 1e-6,
            "the real aspect ratio is unchanged"
        );
        assert!(
            DRAWN_ASPECT_RATIO > HTR10_RPV_ASPECT_RATIO,
            "the drawing is wider"
        );
        // A drawn pebble is roughly 12 cm across (about twice the real 6 cm),
        // so the allowance must be wider than that.
        assert!(REFUEL_CHUTE_ALLOWANCE_CM > 12.0);
    }

    /// Centimetres map onto the drawing monotonically, with the DRAWN vessel
    /// wall at 1.0, the real wall line inside it, and the internals top at the
    /// reserved offset.
    #[test]
    fn the_cm_to_drawing_mapping_is_consistent() {
        assert!((radius_fraction(DRAWN_VESSEL_RADIUS_CM) - 1.0).abs() < 1e-6);
        assert!(
            radius_fraction(VESSEL_INNER_RADIUS_CM) < 1.0,
            "the real wall line sits inside the drawn one"
        );
        assert!(radius_fraction(CORE_RADIUS_CM) < radius_fraction(COOLANT_CHANNEL_RADIUS_CM));
        assert!((axial_fraction(0.0) - INTERNALS_TOP_FRACTION).abs() < 1e-6);
        assert!(axial_fraction(INTERNALS_HEIGHT_CM) < 1.0);
    }
}
