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
//!    │  ↑ │      ┌─────┐        │↑  │  1. DOWN the annulus to the bottom
//!    │  ↑ │      │ bed │        │↑  │     cavity
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
//! The annulus is nonetheless drawn cold over its **whole** height, because it
//! is: section 4.2 records it as *"filled with 250 degC cold helium to hold
//! vessel temperature below limit"*. Being full of cold helium is what
//! protects the pressure boundary, and that is true whether or not gas is
//! moving through a given part of it — so the fill and the tracers cover
//! deliberately different extents.
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
//! Four optional tracer trains, all driven by the **primary** loop mass flow
//! and obeying the crate's "ANIMATION IS DERIVED FROM PHYSICS, NEVER
//! HARDCODED" hard rule. Each pass's inlet end is geometry; the direction the
//! marks then travel comes from the sign of the flow the caller advanced the
//! train with, so a reversed or stalled loop reverses or freezes all four
//! together.
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

use crate::animation::TracerTrain;
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
/// vessel wall.
pub fn radius_fraction(radius_cm: f32) -> f32 {
    radius_cm / VESSEL_INNER_RADIUS_CM
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

/// Point at fraction `t` of a polyline's total length, `t` in `[0, 1]`.
///
/// By **arc length**, not by vertex index: spacing marks evenly over vertices
/// would bunch them wherever the path is finely sampled, which for a U-bend is
/// exactly at the corner.
fn point_along(points: &[Pos2], t: f32) -> Pos2 {
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

/// Letterbox `available` to the vessel's real proportions.
pub fn fit_native_aspect(available: Rect) -> Rect {
    let target = HTR10_RPV_ASPECT_RATIO;
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
/// rod drives.
///
/// The drives stand proud of the head on the real machine, so they need room
/// outside the pressure boundary. See [`Htr10ReactorSchematic::native_size`],
/// which adds this on top of the vessel's own aspect.
const DRIVE_BAND_FRACTION: f32 = 0.12;

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
        }
    }

    /// The box this widget wants for a given vessel width, including the room
    /// the control rod drives need above the head.
    pub fn native_size(vessel_width: f32) -> Vec2 {
        let vessel_h = vessel_width / HTR10_RPV_ASPECT_RATIO;
        Vec2::new(vessel_width, vessel_h * (1.0 + DRIVE_BAND_FRACTION))
    }

    /// On-screen size, in points.
    pub fn size(&self) -> Vec2 {
        self.size
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

    fn colour(&self, t: ThermodynamicTemperature) -> Color32 {
        temperature_colour(t, self.min_temp, self.max_temp)
    }

    fn tag(&self, painter: &Painter, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        painter.text(
            at,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(9.0),
            LABEL,
        );
    }
}

impl Widget for Htr10ReactorSchematic {
    /// Draws the vessel, the reflector's real radial bands, the three-pass
    /// helium path, the bed with its conus, the hot plenum, the discharge tube
    /// and the control rods with their drives.
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(self.size, Sense::hover());
        self.control_rod_insertion_frac = self.control_rod_insertion_frac.clamp(0.0, 1.0);

        // The drives stand above the head, so the vessel gets the lower part
        // of the box and the drives the band above it.
        let full = response.rect;
        let drive_band = full.height() * DRIVE_BAND_FRACTION / (1.0 + DRIVE_BAND_FRACTION);
        let rect = fit_native_aspect(Rect::from_min_max(
            Pos2::new(full.left(), full.top() + drive_band),
            full.max,
        ));
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let bore = 0.5 * w * (1.0 - WALL_FRACTION);

        // Plant centimetres to screen, the only two conversions in the body.
        let rx = |radius_cm: f32, side: f32| cx + side * bore * radius_fraction(radius_cm);
        let zy = |z_cm: f32| rect.top() + h * axial_fraction(z_cm);

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
                Pos2::new(rx(VESSEL_INNER_RADIUS_CM, -1.0), rect.top() + dome * 0.30),
                Pos2::new(rx(VESSEL_INNER_RADIUS_CM, 1.0), rect.bottom() - dome * 0.30),
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
        let rod_top_z = 40.0;
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
        // Concentric cylinders: the housing, the pressure-boundary extension
        // inside it, and the stem inside that. Each rod's drive sits directly
        // over its boring, so the drives inherit the same cited radius as the
        // channels below them.
        for x in &rod_xs {
            let head_y = rect.top() + dome * 0.16;
            let stages: [(f32, f32); 3] = [
                (1.35, 0.30),
                (0.95, 0.68),
                (0.45, 1.00),
            ];
            for (width_mult, height_mult) in stages {
                let half = rod_half * width_mult;
                let top = head_y - drive_band * height_mult;
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
        }
        if self.show_labels {
            self.tag(
                &painter,
                Pos2::new(cx, rect.top() - drive_band * 0.55),
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
        self.tag(
            &painter,
            Pos2::new(cx, zy(COLD_PLENUM_TOP_Z_CM - 11.0)),
            "cold plenum",
        );

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

        let pebble_r = (w * 0.014).max(1.0);
        let rows = ((bed.height() / (pebble_r * 2.4)).floor() as usize).max(1);
        let cols = ((bed.width() / (pebble_r * 2.4)).floor() as usize).max(1);
        for r in 0..rows {
            for c in 0..cols {
                let stagger = if r % 2 == 0 { 0.0 } else { pebble_r * 1.2 };
                let px = bed.left() + pebble_r * 1.2 + c as f32 * pebble_r * 2.4 + stagger;
                let py = bed.top() + pebble_r * 1.2 + r as f32 * pebble_r * 2.4;
                if px + pebble_r > bed.right() || py + pebble_r > bed.bottom() {
                    continue;
                }
                painter.circle_stroke(
                    Pos2::new(px, py),
                    pebble_r,
                    Stroke::new(0.9, Color32::from_black_alpha(150)),
                );
            }
        }

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
        self.tag(&painter, Pos2::new(cx, bed.center().y), "pebble bed");

        // ── Hot helium plenum, in the bottom reflector ─────────────────────
        let plenum = Rect::from_min_max(
            Pos2::new(rx(CORE_RADIUS_CM + 18.0, -1.0), zy(CONUS_BOTTOM_Z_CM + 14.0)),
            Pos2::new(rx(CORE_RADIUS_CM + 18.0, 1.0), zy(CONUS_BOTTOM_Z_CM + 60.0)),
        );
        painter.rect_filled(plenum, 2, hot);
        self.tag(
            &painter,
            Pos2::new(cx, zy(CONUS_BOTTOM_Z_CM + 80.0)),
            "hot plenum",
        );

        // ── The coaxial duct nozzle ────────────────────────────────────────
        //
        // ONE connection carrying both streams. Its elevation is why the
        // coolant path is shaped as it is: the hot inner tube has to meet the
        // hot plenum, and the hot plenum is in the BOTTOM reflector — so the
        // duct, cold annulus included, attaches LOW, and cold helium returns
        // near the foot of the vessel rather than at the top.
        let coax = Rect::from_min_max(
            Pos2::new(rx(CORE_RADIUS_CM + 18.0, 1.0), plenum.top() - plenum.height() * 0.45),
            Pos2::new(rect.right() + w * 0.16, plenum.bottom() + plenum.height() * 0.45),
        );
        painter.rect_filled(coax, 2, cold);
        painter.rect_stroke(coax, 2, Stroke::new(1.0, INTERNALS), StrokeKind::Middle);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(coax.left(), plenum.top()),
                Pos2::new(coax.right(), plenum.bottom()),
            ),
            2,
            hot,
        );

        // ── Pass 1: the annulus ────────────────────────────────────────────
        //
        // Drawn cold over its FULL height, because it is: section 4.2 records
        // it as "filled with 250 degC cold helium to hold vessel temperature
        // below limit". Being full of cold helium is what protects the
        // pressure boundary, whether or not gas moves through a given part.
        // The MOVING part is shorter — from the duct down to the bottom
        // cavity — so fill and tracers cover deliberately different extents.
        let annulus_top = zy(-6.0);
        let annulus_bottom = zy(bottom_z + 44.0);
        let mut downcomer_rects = Vec::new();
        for side in [-1.0_f32, 1.0] {
            let a = rx(REFLECTOR_OUTER_RADIUS_CM, side);
            let b = rx(VESSEL_INNER_RADIUS_CM, side);
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(a.min(b), annulus_top),
                    Pos2::new(a.max(b), annulus_bottom),
                ),
                1,
                cold,
            );
            // The MOVING descent runs from the duct only as far as the
            // boreholes pick the gas up. Below that the annulus is still
            // full of cold helium, but the gas is in the channels.
            downcomer_rects.push(Rect::from_min_max(
                Pos2::new(a.min(b), coax.center().y),
                Pos2::new(a.max(b), borehole_pickup_y),
            ));
        }

        // ── Fuel discharge tube — the pebble-handling route ────────────────
        let tube_bottom = zy(CONUS_BOTTOM_Z_CM) + h * DISCHARGE_TUBE_LENGTH_FRACTION;
        let tube = Rect::from_min_max(
            Pos2::new(cx - tube_half, zy(CONUS_BOTTOM_Z_CM)),
            Pos2::new(cx + tube_half, tube_bottom.min(rect.bottom() - h * 0.01)),
        );
        painter.rect_filled(tube, 2, self.colour(self.pebble_temp));
        painter.rect_stroke(tube, 2, Stroke::new(1.2, INTERNALS), StrokeKind::Middle);
        self.tag(
            &painter,
            Pos2::new(cx + w * 0.16, tube.center().y),
            "discharge",
        );

        // ── Tracers ────────────────────────────────────────────────────────
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

        if let Some(train) = &self.downcomer_tracer {
            for r in &downcomer_rects {
                vertical_marks(*r, train, true);
            }
        }

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

        if self.show_labels {
            self.tag(
                &painter,
                Pos2::new(cx, rect.bottom() - h * 0.022),
                &format!(
                    "bed {:.0} cm of {:.0} cm cavity · {COOLANT_BOREHOLES} boreholes (3/side drawn)",
                    bed_height, CORE_CAVITY_HEIGHT_CM
                ),
            );
        }

        painter.rect_stroke(shell, 0, Stroke::new(1.5, OUTLINE), StrokeKind::Middle);

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

    /// The vessel keeps its slenderness whatever box it is given, and the
    /// native box leaves room above it for the drives.
    #[test]
    fn the_vessel_letterboxes_and_leaves_room_for_drives() {
        for size in [Vec2::new(900.0, 300.0), Vec2::new(100.0, 900.0)] {
            let r = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, size));
            assert!((r.width() / r.height() - HTR10_RPV_ASPECT_RATIO).abs() < 1e-4);
        }
        let native = Htr10ReactorSchematic::native_size(220.0);
        let vessel_only = 220.0 / HTR10_RPV_ASPECT_RATIO;
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

    /// Centimetres map onto the drawing monotonically, with the vessel wall at
    /// 1.0 and the internals top at the reserved offset.
    #[test]
    fn the_cm_to_drawing_mapping_is_consistent() {
        assert!((radius_fraction(VESSEL_INNER_RADIUS_CM) - 1.0).abs() < 1e-6);
        assert!(radius_fraction(CORE_RADIUS_CM) < radius_fraction(COOLANT_CHANNEL_RADIUS_CM));
        assert!((axial_fraction(0.0) - INTERNALS_TOP_FRACTION).abs() < 1e-6);
        assert!(axial_fraction(INTERNALS_HEIGHT_CM) < 1.0);
    }
}
