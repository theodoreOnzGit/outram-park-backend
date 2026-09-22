//! HTR-10 steam generator — a schematic of its *general structure*.
//!
//! A dedicated widget rather than a variant of
//! [`crate::components::SteamGeneratorVisual`], because it draws a specific
//! internal arrangement rather than a generic once-through helical unit. The
//! generic widget stays as it was and keeps serving every other caller.
//!
//! ## The arrangement drawn here
//!
//! **Maintainer specification, 2026-09-21, in these words:** *"a general
//! structure, a central hot gas riser taking up about 40% of the diameter, and
//! two peripheral helical steam generators. The helical coils will be angled
//! around ~~20~~ **7** degrees, ~~represented by short parallel strokes~~."*
//!
//! **REVISED the same day, also at the maintainer's request:** the coils are
//! drawn as a **continuous winding** — *"some swirly things, so it looks
//! simplified in style, and not too much like the actual schematic"* — rather
//! than as parallel strokes. The pitch angle survives the change and still
//! drives the drawing, now by setting how many turns the helix makes — and it
//! was lowered from 20 to 7 degrees at the same time, because a winding needs
//! a shallower pitch than parallel strokes did to read as a coil. See
//! [`DEFAULT_COIL_ANGLE_DEGREES`].
//!
//! ```text
//!        ┌─────────────────────────┐
//!        │ ⌇⌇⌇ │             │ ⌇⌇⌇ │   coil as a winding, turns set
//!        │ ⌇⌇⌇ │   central   │ ⌇⌇⌇ │   by the ~7 deg pitch angle
//!        │ ⌇⌇⌇ │  hot gas    │ ⌇⌇⌇ │
//!        │ ⌇⌇⌇ │   riser     │ ⌇⌇⌇ │   two peripheral bundles
//!        │ ⌇⌇⌇ │   ~40% D    │ ⌇⌇⌇ │
//!        └─────────────────────────┘
//! ```
//!
//! The winding is drawn in the same stylisation as
//! [`crate::components::SteamGeneratorVisual`]'s generic helical-coil unit, so
//! the two read as the same kind of machine.
//!
//! ## What this is and is not
//!
//! **It is a general-structure schematic, drawn to a specification, not a
//! reproduction of any published figure and not the plant's phase-one
//! internals.** Two points where it deliberately differs from what this
//! workspace's own source review records, so nobody reads the picture as data:
//!
//! - `docs/reactor-scoping/htr10-plant-data.md` section 5 records the 30
//!   helical modules as installed in an **annular space**, with the vessel
//!   centre **reserved for a N2-He intermediate heat exchanger and empty in
//!   the first stage**. The central riser drawn here is a schematic device for
//!   showing where the hot gas goes, specified by the maintainer; it is not a
//!   claim that a riser occupies that cavity.
//! - The **40 % diameter** and **7 degree** coil angle are the maintainer's
//!   drawing parameters. Neither is a plant dimension — section 5 of that
//!   sheet lists the coil pitch as *Unknown*, and records only a 112 mm bundle
//!   diameter per module, which this schematic does not attempt to resolve.
//!
//! Dimensions that *are* cited are marked as such at the point of use.
//!
//! ## Animation — five streams on two loops
//!
//! Five optional tracer trains, all obeying the crate's "ANIMATION IS DERIVED
//! FROM PHYSICS, NEVER HARDCODED" hard rule:
//!
//! | Train | Stream | Advance it with |
//! |---|---|---|
//! | [`Htr10SteamGeneratorVisual::with_riser_tracer`] | helium climbing the central riser | **primary** loop mass flow |
//! | [`Htr10SteamGeneratorVisual::with_shell_gas_tracer`] | helium descending across the coil | **primary** loop mass flow, shell-side residence time |
//! | [`Htr10SteamGeneratorVisual::with_coil_water_tracer`] | water rising through the coil | **secondary** loop mass flow |
//! | [`Htr10SteamGeneratorVisual::with_feedwater_tracer`] | feedwater entering at the inlet nozzle | **secondary** loop mass flow, nozzle residence time |
//! | [`Htr10SteamGeneratorVisual::with_steam_tracer`] | superheated steam leaving at the outlet nozzle | **secondary** loop mass flow, nozzle residence time |
//!
//! **Where each stream's inlet is, is geometry; which way the marks then
//! travel, is physics.** The riser fills from the bottom because the hot gas
//! duct enters at the foot of the vessel; the shell side fills from the top
//! because the gas turns at the head of the riser and descends; the coil fills
//! from the bottom because a once-through generator takes feedwater in low and
//! delivers steam high; the feedwater nozzle fills from its outboard end and
//! the steam nozzle from its inboard end, because one brings fluid in and the
//! other takes it out. Those are facts about the machine. Direction of travel
//! along each path is **not** set here at all — [`TracerTrain::advance`] takes
//! it from the sign of the mass flow the caller supplies, so a reversed or
//! stalled loop reverses or freezes the marks it owns, and the two loops can
//! disagree.
//!
//! Because the two nozzles sit on the same loop but face opposite ways, a
//! positive secondary flow drives their marks in **opposite screen
//! directions** — in at the bottom, out at the top — with no sign handling
//! anywhere in this widget.
//!
//! **Draw order is load-bearing.** The shell-gas marks are drawn *between* the
//! two halves of the winding — back half, gas, front half — so the near side
//! of the coil occludes them as they pass. That is what makes the gas read as
//! moving down the middle of the helix rather than sliding across in front of
//! it. The water marks ride the helix curve itself and are drawn only on the
//! near half, for the same reason.
//!
//! Colour is derived too: every region is graded by a temperature the caller
//! supplies.

use crate::animation::TracerTrain;
use crate::components::htr10_reactor_schematic::{
    point_along, DRAWN_ASPECT_RATIO, LABEL_REFERENCE_VESSEL_WIDTH,
};
use crate::components::temperature_colour;
use std::f32::consts::PI;
use egui::{Color32, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    Widget};
use uom::si::angle::degree;
use uom::si::f64::{Angle, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;

/// Outer proportions of the HTR-10 steam-generator pressure vessel,
/// width / height.
///
/// From `docs/reactor-scoping/htr10-plant-data.md` section 4.2: SG pressure
/// vessel diameter **2.6 m** (*Quoted*), height **more than 11 m** (*Quoted*
/// as a bound, and flagged *Uncertain* there because the scan is poor). 11 m
/// is used as the height, which makes this a lower bound on slenderness — the
/// real vessel is at least this slender, never squatter.
pub const HTR10_SG_ASPECT_RATIO: f32 = 2.6 / 11.0;

/// Share of the vessel diameter taken by the central hot gas riser.
///
/// **Maintainer-specified drawing parameter** (2026-09-21: *"about 40% of the
/// diameter"*), not a plant dimension.
pub const DEFAULT_RISER_DIAMETER_FRACTION: f32 = 0.40;

/// Pitch angle of the drawn coil, from the horizontal.
///
/// **Maintainer-specified drawing parameter.** First given as ~~20 degrees~~
/// (2026-09-21), then **CHANGED the same day to 7 degrees** once the coils
/// were drawn as a continuous winding rather than as parallel strokes —
/// *"coil angle should also be 7 degrees by default, i think that looks
/// good"*. The real coil pitch is recorded as *Unknown* in the plant-data
/// sheet, so this is a representation of a helix, not a measurement of one,
/// and it is chosen on how it reads.
///
/// It sets how many turns the winding makes over the bundle height, through
/// the helix relation `tan(theta) = p / (2 pi r)` — so a **shallower** angle
/// winds **more** turns, as a shallower helix genuinely does. That is why the
/// number came down when the style changed: at 20 degrees the winding was too
/// open to read as a coil, and 7 degrees tightens it by roughly a factor of
/// three (`tan 20 / tan 7 = 2.96`).
pub const DEFAULT_COIL_ANGLE_DEGREES: f64 = 7.0;

const STEEL: Color32 = Color32::from_rgb(96, 100, 108);
const OUTLINE: Color32 = Color32::from_rgb(150, 154, 162);
const INTERNALS: Color32 = Color32::from_rgb(64, 68, 76);
const VOID: Color32 = Color32::from_rgb(28, 30, 34);

/// Label text size, points: 1.5x the original 9 pt (maintainer direction,
/// 2026-09-22), matching the HTR-10 vessel schematic. This is the size when the steam
/// generator is drawn at the height of a reactor vessel
/// [`LABEL_REFERENCE_VESSEL_WIDTH`] wide; labels scale with the drawing.
const LABEL_FONT_SIZE: f32 = 13.5;

/// Thinnest drawn coil stroke, points: below this a line stops reading as a
/// tube. A drawing parameter, nothing physical.
const MIN_COIL_STROKE: f32 = 1.0;

/// Least clear space between adjacent coil turns, points. egui feathers each
/// stroke edge by about a point, eating into the gap from both sides, so 3 pt
/// leaves at least a point visibly clear and the turns never touch. A drawing
/// parameter, nothing physical.
const MIN_COIL_GAP: f32 = 3.0;

/// Fill of every boxed label, matching the HTR-10 vessel schematic's.
const LABEL_BOX_GREY: Color32 = Color32::from_rgb(88, 90, 96);
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);

/// Bottom of the riser and coil bundles, as a fraction of the vessel height
/// from the top. A drawing choice.
///
/// **CHANGED 2026-09-21** from 0.90, to leave an inlet space below the bundles
/// where the hot gas duct's three streams turn up into the riser and the two
/// bundles (see [`Htr10SteamGeneratorVisual::with_duct_inlet`]).
const BUNDLE_BOTTOM_FRACTION: f32 = 0.80;

/// Elevation of the hot gas duct's centreline on the vessel's left wall, as a
/// fraction of the vessel height from the top: in the inlet space, between the
/// bundle bottom and the cavity bottom. A drawing choice.
const GAS_PORT_FRACTION: f32 = 0.90;

/// Water-side nozzles' inner and outer ends, as fractions of the vessel width
/// from the axis: they run from inside the shell out to the right. Drawing
/// choices.
const NOZZLE_INNER_X: f32 = 0.30;
const NOZZLE_OUTER_X: f32 = 0.70;

/// Steam outlet nozzle, top and bottom, as fractions of the vessel height from
/// the top: high, where a once-through generator delivers steam.
const STEAM_NOZZLE_SPAN: (f32, f32) = (0.085, 0.110);

/// Feedwater inlet nozzle, top and bottom, fractions of the vessel height: just
/// above the bundle bottom, where the coil takes feedwater in.
fn feedwater_nozzle_span() -> (f32, f32) {
    (
        BUNDLE_BOTTOM_FRACTION - 0.010,
        BUNDLE_BOTTOM_FRACTION + 0.015,
    )
}

/// Bottom of the interior cavity, as a fraction of the vessel height from the
/// top. A drawing choice.
const INTERIOR_BOTTOM_FRACTION: f32 = 0.95;

/// Letterbox `available` to the vessel's real proportions.
///
/// Keeps the vessel's slenderness at any box size, so a wide panel does not
/// draw a squat generator that misrepresents the machine.
pub fn fit_native_aspect(available: Rect) -> Rect {
    let target = HTR10_SG_ASPECT_RATIO;
    let have = available.width() / available.height().max(1.0);
    if have > target {
        let w = available.height() * target;
        Rect::from_center_size(available.center(), Vec2::new(w, available.height()))
    } else {
        let h = available.width() / target;
        Rect::from_center_size(available.center(), Vec2::new(available.width(), h))
    }
}


/// Centreline of a pipe that runs horizontally from `start`, turns up through
/// a 90-degree bend of centreline radius `radius`, and rises to `end_y`.
///
/// Used for the hot gas duct's streams inside the vessel. `turn_x` is where
/// the vertical leg runs. The radius is reduced if the run or the rise is too
/// short for it, so the bend never overshoots either leg.
fn elbow_up(start: Pos2, turn_x: f32, end_y: f32, radius: f32) -> Vec<Pos2> {
    let r = radius
        .min((turn_x - start.x).max(0.0))
        .min((start.y - end_y).max(0.0));
    let centre = Pos2::new(turn_x - r, start.y - r);
    let mut out = vec![start];
    const SEGMENTS: usize = 12;
    for i in 0..=SEGMENTS {
        // pi/2 is the point level with the run, 0 the point above the turn.
        let theta = 0.5 * PI * (1.0 - i as f32 / SEGMENTS as f32);
        out.push(Pos2::new(
            centre.x + r * theta.cos(),
            centre.y + r * theta.sin(),
        ));
    }
    out.push(Pos2::new(turn_x, end_y));
    out
}
/// The HTR-10 steam generator, drawn as a general-structure schematic.
///
/// Four temperatures drive the colouring, all supplied by the caller from its
/// own model and all graded through the shared
/// [`crate::components::temperature_colour`] map, so this widget reads on the
/// same colour scale as every other one in the library:
///
/// | Field | Physical quantity |
/// |---|---|
/// | [`Self::helium_inlet_temp`] | hot helium entering the riser from the reactor, K |
/// | [`Self::helium_outlet_temp`] | cooled helium leaving the bundles, K |
/// | [`Self::feedwater_temp`] | feedwater entering the coils at the bottom, K |
/// | [`Self::steam_temp`] | superheated steam leaving at the top, K |
///
/// At the HTR-10 design point those are 700 degC, 250 degC, 104 degC and
/// 440 degC respectively (`docs/reactor-scoping/htr10-plant-data.md` section
/// 6), but the widget imposes none of them — it draws what it is given.
pub struct Htr10SteamGeneratorVisual {
    size: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    helium_inlet_temp: ThermodynamicTemperature,
    helium_outlet_temp: ThermodynamicTemperature,
    feedwater_temp: ThermodynamicTemperature,
    steam_temp: ThermodynamicTemperature,
    riser_diameter_fraction: f32,
    coil_angle: Angle,
    show_labels: bool,
    riser_tracer: Option<TracerTrain>,
    coil_water_tracer: Option<TracerTrain>,
    shell_gas_tracer: Option<TracerTrain>,
    feedwater_tracer: Option<TracerTrain>,
    steam_tracer: Option<TracerTrain>,
    /// `(outer_height, inner_height)` of a connected duct, points. See
    /// [`Self::with_duct_inlet`].
    duct_inlet: Option<(f32, f32)>,
    /// Marks through the hot elbow, riser-bound. See
    /// [`Self::with_duct_inlet_tracers`].
    hot_elbow_tracer: Option<TracerTrain>,
    /// Marks through both cold elbows, duct-bound.
    cold_elbow_tracer: Option<TracerTrain>,
}

impl Htr10SteamGeneratorVisual {
    /// Build the schematic.
    ///
    /// `min_temp`/`max_temp` bound the colour scale. The map is diverging, so
    /// set them about a meaningful midpoint rather than to the extremes seen —
    /// for this unit a range spanning roughly 300 K to 1200 K puts the water
    /// side on the cool half and the helium side on the warm half.
    pub fn new(
        size: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        helium_inlet_temp: ThermodynamicTemperature,
        helium_outlet_temp: ThermodynamicTemperature,
        feedwater_temp: ThermodynamicTemperature,
        steam_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            size,
            min_temp,
            max_temp,
            helium_inlet_temp,
            helium_outlet_temp,
            feedwater_temp,
            steam_temp,
            riser_diameter_fraction: DEFAULT_RISER_DIAMETER_FRACTION,
            coil_angle: Angle::new::<degree>(DEFAULT_COIL_ANGLE_DEGREES),
            show_labels: true,
            riser_tracer: None,
            coil_water_tracer: None,
            shell_gas_tracer: None,
            feedwater_tracer: None,
            steam_tracer: None,
            duct_inlet: None,
            hot_elbow_tracer: None,
            cold_elbow_tracer: None,
        }
    }

    /// Where the hot gas duct meets the vessel, for a widget whose box is
    /// `widget_rect` (the rect it will be placed in, of size [`Self::size`]).
    ///
    /// It is the duct centreline on the vessel's **left wall**, in the inlet
    /// space below the bundles. The duct stops here, OUTSIDE the vessel;
    /// inside, [`Self::with_duct_inlet`] draws its streams turning up into the
    /// riser and the bundles (**CHANGED 2026-09-21**, from a port at the foot
    /// of the riser on the axis). Computed from the same fractions the paint
    /// code uses.
    pub fn gas_port(&self, widget_rect: Rect) -> Pos2 {
        let rect = fit_native_aspect(widget_rect);
        Pos2::new(rect.left(), rect.top() + GAS_PORT_FRACTION * rect.height())
    }

    /// Outer end of the **steam** outlet nozzle, on the right, for a widget
    /// whose box is `widget_rect`: where the main steam line to the turbine
    /// connects. From the same constants the nozzle is painted with.
    pub fn steam_port(&self, widget_rect: Rect) -> Pos2 {
        let rect = fit_native_aspect(widget_rect);
        let f = 0.5 * (STEAM_NOZZLE_SPAN.0 + STEAM_NOZZLE_SPAN.1);
        Pos2::new(
            rect.center().x + rect.width() * NOZZLE_OUTER_X,
            rect.top() + f * rect.height(),
        )
    }

    /// Outer end of the **feedwater** inlet nozzle, on the right, for a widget
    /// whose box is `widget_rect`: where the feed line from the pump connects.
    pub fn feedwater_port(&self, widget_rect: Rect) -> Pos2 {
        let rect = fit_native_aspect(widget_rect);
        let (a, b) = feedwater_nozzle_span();
        Pos2::new(
            rect.center().x + rect.width() * NOZZLE_OUTER_X,
            rect.top() + 0.5 * (a + b) * rect.height(),
        )
    }

    /// Draw the coaxial hot gas duct's streams inside the vessel, entering at
    /// [`Self::gas_port`] with the duct's own band heights, in points:
    /// `outer_height` for the whole duct and `inner_height` for its hot inner
    /// tube.
    ///
    /// Inside, each stream turns up through a 90-degree bend (maintainer
    /// specification, 2026-09-21):
    ///
    /// ```text
    ///      left bundle   riser   right bundle
    ///          ▲           ▲          ▲
    ///   ═══════╯ (top cold band)      │
    ///   ━━━━━━━━━━━━━━━━━━━╯ (hot)     │
    ///   ══════════════════════════════╯ (bottom cold band)
    /// ```
    ///
    /// - the **hot inner tube** runs to the axis and bends up into the central
    ///   riser, where the hot gas climbs;
    /// - the **top cold band** bends up into the **left** bundle;
    /// - the **bottom cold band** passes under the hot bend and turns up into
    ///   the **right** bundle.
    ///
    /// Physically the cold helium leaves the bundles at the bottom and returns
    /// down these legs to the duct's annulus; the drawing only fixes the
    /// geometry. Without this call the vessel is drawn with no helium
    /// connections, as before.
    pub fn with_duct_inlet(mut self, outer_height: f32, inner_height: f32) -> Self {
        self.duct_inlet = Some((outer_height.max(0.0), inner_height.max(0.0)));
        self
    }

    /// Tracer marks through the duct inlet's elbows (only drawn with
    /// [`Self::with_duct_inlet`]). Advance both with the **primary** loop
    /// mass flow and a residence time for the elbows.
    ///
    /// - `hot`: its inlet is the **wall** end, so positive flow carries the
    ///   marks in along the hot tube and up into the riser.
    /// - `cold`: drawn on both cold legs; its inlet is the **bundle** end,
    ///   because the cooled helium leaves the bundles at the bottom, so
    ///   positive flow carries the marks down and out to the duct's annulus.
    ///
    /// Marks are placed by arc length, so they follow each bend smoothly.
    pub fn with_duct_inlet_tracers(mut self, hot: TracerTrain, cold: TracerTrain) -> Self {
        self.hot_elbow_tracer = Some(hot);
        self.cold_elbow_tracer = Some(cold);
        self
    }

    /// Tracer marks for the **primary** helium rising in the central riser.
    ///
    /// Advance this train with the **primary loop** mass flow and the riser's
    /// residence time. Direction follows the flow's sign, because
    /// [`TracerTrain::advance`] takes it from there — a reversed primary loop
    /// runs these marks downward, and a stalled circulator freezes them.
    ///
    /// The train is **advanced by the application**, once per frame, and
    /// copied in here at widget-build time: widgets are rebuilt every repaint,
    /// so a train owned by the widget would reset its phase each frame. See
    /// [`crate::animation`].
    pub fn with_riser_tracer(mut self, tracer: TracerTrain) -> Self {
        self.riser_tracer = Some(tracer);
        self
    }

    /// Tracer marks for the **secondary** water travelling up through the
    /// coil.
    ///
    /// Advance with the **secondary loop** (feedwater) mass flow and the
    /// coil's residence time. The marks follow the drawn helix itself rather
    /// than sliding up a straight line, so they read as fluid inside the tube.
    pub fn with_coil_water_tracer(mut self, tracer: TracerTrain) -> Self {
        self.coil_water_tracer = Some(tracer);
        self
    }

    /// Tracer marks for the **shell-side helium** descending across the coil.
    ///
    /// Advance with the primary mass flow and the shell side's own residence
    /// time — which is not the riser's, since the shell side is a far larger
    /// volume at a lower velocity.
    pub fn with_shell_gas_tracer(mut self, tracer: TracerTrain) -> Self {
        self.shell_gas_tracer = Some(tracer);
        self
    }

    /// Tracer marks on the **feedwater inlet** nozzle.
    ///
    /// Advance with the secondary loop mass flow. The nozzle's inlet is its
    /// outboard end — feedwater arrives from the turbine hall — so at a
    /// positive flow these marks run **inward**, opposite to the steam marks
    /// above them. Give it a short residence time: a nozzle is a far smaller
    /// volume than the coil it feeds, so its marks should visibly outrun the
    /// coil's.
    pub fn with_feedwater_tracer(mut self, tracer: TracerTrain) -> Self {
        self.feedwater_tracer = Some(tracer);
        self
    }

    /// Tracer marks on the **superheated steam outlet** nozzle.
    ///
    /// Advance with the secondary loop mass flow. The nozzle's inlet is its
    /// inboard end — steam leaves the vessel — so at a positive flow these
    /// marks run **outward**.
    pub fn with_steam_tracer(mut self, tracer: TracerTrain) -> Self {
        self.steam_tracer = Some(tracer);
        self
    }

    /// On-screen size, in points.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Set the riser's share of the vessel diameter, dimensionless.
    ///
    /// Clamped to `[0.1, 0.8]` at render time: outside that the drawing stops
    /// being readable as a riser flanked by two bundles, which is the whole
    /// structure this widget exists to show.
    pub fn with_riser_diameter_fraction(mut self, fraction: f32) -> Self {
        self.riser_diameter_fraction = fraction;
        self
    }

    /// Set the angle of the coil strokes from the horizontal. Builder-style.
    pub fn with_coil_angle(mut self, angle: Angle) -> Self {
        self.coil_angle = angle;
        self
    }

    /// Turn the internal labels off — for thumbnails. Builder-style.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// The riser fraction actually used when drawing.
    pub fn drawn_riser_fraction(&self) -> f32 {
        self.riser_diameter_fraction.clamp(0.1, 0.8)
    }

    fn colour(&self, t: ThermodynamicTemperature) -> Color32 {
        temperature_colour(t, self.min_temp, self.max_temp)
    }

    /// How much the labels are scaled. This vessel is drawn at the height of
    /// the reactor vessel beside it, so its labels scale by the same factor as
    /// the reactor's: drawn height over the height of a reactor vessel
    /// [`LABEL_REFERENCE_VESSEL_WIDTH`] wide. Labels therefore keep their
    /// proportion to the artwork at every zoom (maintainer direction,
    /// 2026-09-22). A drawing scale, nothing physical.
    fn label_scale(&self) -> f32 {
        (self.size.y * DRAWN_ASPECT_RATIO / LABEL_REFERENCE_VESSEL_WIDTH).max(0.05)
    }

    /// A label in a grey box, the same format as the HTR-10 vessel's boxed
    /// labels, so the whole plant page matches (maintainer direction,
    /// 2026-09-22): grey fill, internals-coloured edge, text centred on `at`.
    fn tag(&self, painter: &Painter, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        let s = self.label_scale();
        let galley = painter.layout_no_wrap(
            text.to_owned(),
            FontId::proportional(LABEL_FONT_SIZE * s),
            LABEL,
        );
        let rect = Rect::from_center_size(at, galley.size() + Vec2::new(8.0, 4.0) * s);
        painter.rect_filled(rect, 2.0 * s, LABEL_BOX_GREY);
        painter.rect_stroke(
            rect,
            2.0 * s,
            Stroke::new(1.2 * s.max(0.5), INTERNALS),
            StrokeKind::Middle,
        );
        painter.galley(rect.center() - 0.5 * galley.size(), galley, LABEL);
    }

    /// Water-side temperature at height fraction `f`, `0` at the top.
    ///
    /// Feedwater enters low and superheated steam leaves high, so the water
    /// side is coldest at the bottom. Linear between the two supplied ends:
    /// the real profile is not linear — it is flat through the evaporating
    /// region — and interpolating for *colour* is artwork, stated as such.
    fn water_temperature_at(&self, f: f32) -> ThermodynamicTemperature {
        let rise = (1.0 - f).clamp(0.0, 1.0) as f64;
        ThermodynamicTemperature::new::<kelvin>(
            self.feedwater_temp.get::<kelvin>() * (1.0 - rise)
                + self.steam_temp.get::<kelvin>() * rise,
        )
    }
}

impl Widget for Htr10SteamGeneratorVisual {
    /// Draws the vessel, the central hot gas riser and the two peripheral
    /// helical bundles.
    ///
    /// The riser is filled at the helium **inlet** temperature over its whole
    /// height: it carries reactor-outlet gas up to the top of the unit before
    /// any heat has been given up. The bundles are graded from the helium
    /// inlet at the top to the helium outlet at the bottom, because the gas
    /// turns at the top and flows back down across the coil. The coil strokes
    /// themselves carry the **water-side** temperature at their elevation, so
    /// the two sides of the heat exchange are separately visible.
    fn ui(self, ui: &mut Ui) -> Response {
        // The water-side nozzles reach 0.70 of the vessel width from the axis,
        // past the widget's own box. A painter clipped to the box (what
        // `allocate_painter` gives) cut them off at the wall, leaving the pipes
        // that connect to them nothing to meet. So the box is allocated for
        // layout, and painting is clipped only by the surrounding panel
        // (corrected 2026-09-22).
        let response = ui.allocate_response(self.size, Sense::hover());
        let painter = ui.painter().clone();
        // Labels on the FOREGROUND layer, clipped to the panel, so no part of
        // the plant schematic covers them (maintainer direction, 2026-09-22).
        let labels = ui
            .ctx()
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                response.id.with("htr10_sg_labels"),
            ))
            .with_clip_rect(ui.clip_rect());
        let rect = fit_native_aspect(response.rect);
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let y = |f: f32| rect.top() + f * h;

        let helium_in = self.colour(self.helium_inlet_temp);

        // ── Pressure vessel: a capsule with domed heads ─────────────────────
        let dome = w * 0.5;
        let shell = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + dome * 0.62),
            Pos2::new(rect.right(), rect.bottom() - dome * 0.62),
        );
        painter.rect_filled(shell, 0, STEEL);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.right(), shell.top() + dome * 0.4),
            ),
            (dome * 0.6).round().clamp(0.0, 255.0) as u8,
            STEEL,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), shell.bottom() - dome * 0.4),
                Pos2::new(rect.right(), rect.bottom()),
            ),
            (dome * 0.6).round().clamp(0.0, 255.0) as u8,
            STEEL,
        );

        // Interior cavity the internals sit in.
        let interior = Rect::from_min_max(
            Pos2::new(cx - w * 0.45, y(0.05)),
            Pos2::new(cx + w * 0.45, y(INTERIOR_BOTTOM_FRACTION)),
        );
        painter.rect_filled(interior, (w * 0.06).round() as u8, VOID);

        // ── Vertical extent of the internals ────────────────────────────────
        let top_f = 0.10_f32;
        let bottom_f = BUNDLE_BOTTOM_FRACTION;
        let bundle_top = y(top_f);
        let bundle_bottom = y(bottom_f);

        // ── Central hot gas riser ───────────────────────────────────────────
        //
        // Maintainer-specified at ~40 % of the vessel diameter. Measured on
        // the interior cavity's width, so the two peripheral bundles get the
        // remainder split evenly between them.
        let cavity_half = interior.width() * 0.5;
        let riser_half = cavity_half * self.drawn_riser_fraction();
        let riser = Rect::from_min_max(
            Pos2::new(cx - riser_half, bundle_top),
            Pos2::new(cx + riser_half, bundle_bottom),
        );
        painter.rect_filled(riser, 2, helium_in);
        painter.rect_stroke(riser, 2, Stroke::new(1.2, INTERNALS), StrokeKind::Middle);

        // Riser tracers.
        //
        // The riser's INLET is at the bottom — gas arrives from the hot gas
        // duct at the foot of the vessel and climbs to the turn at the top.
        // That is the machine's geometry, not an animation choice: a mark at
        // train position 0 sits at the bottom and position 1 at the top, and
        // which way it then travels comes from the sign of the mass flow the
        // caller advanced the train with.
        if let Some(train) = &self.riser_tracer {
            let mark_h = ((bundle_bottom - bundle_top) * 0.035).max(2.0);
            for position in train.positions() {
                let yc = bundle_bottom - (bundle_bottom - bundle_top) * position as f32;
                let a = (yc - 0.5 * mark_h).max(bundle_top);
                let b = (yc + 0.5 * mark_h).min(bundle_bottom);
                if b - a < 0.5 {
                    continue;
                }
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(riser.left() + 1.5, a),
                        Pos2::new(riser.right() - 1.5, b),
                    ),
                    1,
                    Color32::WHITE,
                );
            }
        }

        self.tag(&labels, Pos2::new(cx, y(0.5)), "hot gas riser");

        // ── Two peripheral helical bundles ──────────────────────────────────
        //
        // Each occupies the gap between the riser and the vessel wall. The gas
        // side is graded top-to-bottom from helium inlet to helium outlet,
        // since the gas turns at the top of the riser and descends across the
        // coil.
        let gas_bands = 12;
        for side in [-1.0_f32, 1.0] {
            let inner = cx + side * riser_half;
            let outer = cx + side * cavity_half;
            let (left, right) = if side < 0.0 {
                (outer, inner)
            } else {
                (inner, outer)
            };

            for b in 0..gas_bands {
                let t0 = b as f32 / gas_bands as f32;
                let t1 = (b + 1) as f32 / gas_bands as f32;
                let mid = 0.5 * (t0 + t1);
                let band = Rect::from_min_max(
                    Pos2::new(left, bundle_top + (bundle_bottom - bundle_top) * t0),
                    Pos2::new(right, bundle_top + (bundle_bottom - bundle_top) * t1),
                );
                let gas = ThermodynamicTemperature::new::<kelvin>(
                    self.helium_inlet_temp.get::<kelvin>() * (1.0 - mid as f64)
                        + self.helium_outlet_temp.get::<kelvin>() * mid as f64,
                );
                let c = self.colour(gas);
                painter.rect_filled(
                    band,
                    0,
                    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 90),
                );
            }

            // The coil, drawn as a continuous winding rather than as separate
            // tube sections — the same stylisation
            // `SteamGeneratorVisual::draw_helical_coil` uses, so the two read
            // as the same kind of machine. Deliberately simplified: it says
            // "helix" without pretending to be a tube layout.
            //
            // Two passes give it depth. The half of each turn passing BEHIND
            // the bundle axis is drawn first and translucent, the front half
            // afterwards at full strength, so the winding reads as wrapping
            // something rather than as a flat zigzag.
            //
            // Each segment carries the WATER-side temperature at its own
            // elevation, so the coil reads as the heated stream while the band
            // behind it reads as the gas.
            let bundle_w = (right - left).max(1.0);
            let mid_x = 0.5 * (left + right);
            let coil_radius = bundle_w * 0.34;
            let span = bundle_bottom - bundle_top;

            // The specified pitch angle still drives the drawing, now through
            // the helix geometry instead of a stroke slope: for a helix of
            // radius `r` climbing an axial pitch `p` per turn,
            // `tan(theta) = p / (2 pi r)`, so the number of turns over a
            // height `H` is `H / (2 pi r tan theta)`. A shallower angle winds
            // more turns, which is what a shallower helix actually does.
            let theta = (self.coil_angle.get::<degree>() as f32).to_radians();
            let (turns, width) = coil_turns_and_stroke(span, bundle_w, coil_radius, theta);
            // Enough samples per turn that the winding stays a smooth curve
            // at every turn count.
            let samples = ((turns * 32.0).ceil() as usize).max(64);

            // One pass of the winding: `behind` selects the half of each turn
            // on the far side of the bundle axis.
            let coil_pass = |behind: bool| {
                for k in 0..samples {
                    let t0 = k as f32 / samples as f32;
                    let t1 = (k + 1) as f32 / samples as f32;
                    let a0 = turns * 2.0 * PI * t0;
                    let a1 = turns * 2.0 * PI * t1;
                    // Sign of the cosine says which side of the axis this bit
                    // of the turn is on, so it selects the pass.
                    if ((0.5 * (a0 + a1)).cos() < 0.0) != behind {
                        continue;
                    }
                    let p0 = Pos2::new(mid_x + coil_radius * a0.sin(), bundle_top + span * t0);
                    let p1 = Pos2::new(mid_x + coil_radius * a1.sin(), bundle_top + span * t1);
                    let water = self.water_temperature_at(t0);
                    let mut c = self.colour(water);
                    if behind {
                        c = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120);
                    }
                    painter.line_segment([p0, p1], Stroke::new(width, c));
                }
            };

            // ── Draw order inside a bundle, and why it is this order ────────
            //
            // The shell-side gas goes *through* the bundle, so its marks are
            // sandwiched between the two halves of the winding: back half,
            // then the gas, then the front half over the top of it. The front
            // of the coil therefore occludes the gas marks as they pass, which
            // is what makes them read as travelling down the middle of the
            // helix rather than sliding across in front of it.
            coil_pass(true);

            // Shell-side helium tracers.
            //
            // The shell side's INLET is at the TOP: gas turns at the head of
            // the riser and descends across the coil. Geometry again, not an
            // animation choice — position 0 is the top, and the direction of
            // travel comes from the sign of the mass flow the caller advanced
            // this train with.
            if let Some(train) = &self.shell_gas_tracer {
                let mark_h = (span * 0.030).max(2.0);
                let mark_w = (bundle_w * 0.34).max(2.0);
                for position in train.positions() {
                    let yc = bundle_top + span * position as f32;
                    let a = (yc - 0.5 * mark_h).max(bundle_top);
                    let b = (yc + 0.5 * mark_h).min(bundle_bottom);
                    if b - a < 0.5 {
                        continue;
                    }
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(mid_x - 0.5 * mark_w, a),
                            Pos2::new(mid_x + 0.5 * mark_w, b),
                        ),
                        1,
                        // Slightly translucent: it is inside the bundle, seen
                        // between the turns, not sitting on top of them.
                        Color32::from_white_alpha(205),
                    );
                }
            }

            coil_pass(false);

            // Secondary-water tracers, riding the helix itself.
            //
            // A once-through generator takes feedwater in at the BOTTOM and
            // delivers steam at the top, so the coil's inlet is the bottom and
            // train position 0 belongs there. The drawing parameter `t` runs
            // from 0 at the top, hence `t = 1 - position`.
            //
            // Each mark is a short arc of the winding rather than a dot, so it
            // reads as a plug of fluid inside the tube, and it is drawn only
            // where the helix is on the near side — a mark on the hidden half
            // would otherwise float in front of the coil it is supposed to be
            // inside.
            if let Some(train) = &self.coil_water_tracer {
                let arc_samples = 7;
                let arc_span = 0.012_f32;
                for position in train.positions() {
                    let centre_t = 1.0 - position as f32;
                    for j in 0..arc_samples {
                        let f0 = j as f32 / arc_samples as f32;
                        let f1 = (j + 1) as f32 / arc_samples as f32;
                        let t0 = (centre_t + arc_span * (f0 - 0.5)).clamp(0.0, 1.0);
                        let t1 = (centre_t + arc_span * (f1 - 0.5)).clamp(0.0, 1.0);
                        let a0 = turns * 2.0 * PI * t0;
                        let a1 = turns * 2.0 * PI * t1;
                        if (0.5 * (a0 + a1)).cos() < 0.0 {
                            continue;
                        }
                        let p0 =
                            Pos2::new(mid_x + coil_radius * a0.sin(), bundle_top + span * t0);
                        let p1 =
                            Pos2::new(mid_x + coil_radius * a1.sin(), bundle_top + span * t1);
                        painter.line_segment([p0, p1], Stroke::new(width, Color32::WHITE));
                    }
                }
            }

            self.tag(
                &labels,
                Pos2::new(mid_x, y(top_f) - 9.0 * self.label_scale()),
                "helical coil",
            );
        }

        // ── Hot gas duct inlet: three streams turning up (if connected) ────
        //
        // The duct stops at the left wall; inside, the hot inner tube bends up
        // into the riser, the top cold band into the left bundle, and the
        // bottom cold band passes under the hot bend into the right bundle.
        // See `with_duct_inlet`. Drawn after the internals and before the
        // nozzles.
        if let Some((outer_h, inner_h)) = self.duct_inlet {
            let yc = y(GAS_PORT_FRACTION);
            let outer_top = yc - 0.5 * outer_h;
            let hot_top = yc - 0.5 * inner_h;
            let hot_bottom = yc + 0.5 * inner_h;
            let outer_bottom = yc + 0.5 * outer_h;
            let cold_upper_h = hot_top - outer_top;
            let cold_lower_h = outer_bottom - hot_bottom;
            // Where each vertical leg rises: the middle of each bundle, and
            // the axis for the riser.
            let bundle_mid = 0.5 * (riser_half + cavity_half);
            let wall = rect.left();
            let helium_out = self.colour(self.helium_outlet_temp);
            let legs = [
                (
                    Pos2::new(wall, 0.5 * (outer_top + hot_top)),
                    cx - bundle_mid,
                    cold_upper_h,
                    helium_out,
                ),
                (
                    Pos2::new(wall, 0.5 * (hot_bottom + outer_bottom)),
                    cx + bundle_mid,
                    cold_lower_h,
                    helium_out,
                ),
                (Pos2::new(wall, yc), cx, inner_h, helium_in),
            ];
            // Walls of all three first, then the fills, so where the legs run
            // side by side at the wall they read as one duct.
            let paths: Vec<Vec<Pos2>> = legs
                .iter()
                .map(|(start, turn_x, thick, _)| {
                    elbow_up(*start, *turn_x, bundle_bottom, 1.2 * thick)
                })
                .collect();
            for (path, (_, _, thick, _)) in paths.iter().zip(&legs) {
                painter.add(egui::Shape::line(
                    path.clone(),
                    Stroke::new(thick + 2.0, INTERNALS),
                ));
            }
            for (path, (_, _, thick, colour)) in paths.iter().zip(&legs) {
                painter.add(egui::Shape::line(
                    path.clone(),
                    Stroke::new(*thick, *colour),
                ));
            }

            // Tracer marks: short bars across each leg, placed by arc length
            // so they follow the bend. The hot leg (index 2) fills from the
            // wall; the cold legs (0, 1) fill from the bundle end, so their
            // position runs backwards along the path.
            let bar = |path: &[Pos2], t: f32, thick: f32| {
                let at = point_along(path, t);
                let ahead = point_along(path, (t + 0.01).min(1.0));
                let behind = point_along(path, (t - 0.01).max(0.0));
                let tangent = (ahead - behind).normalized();
                let across = egui::vec2(-tangent.y, tangent.x) * (0.5 * thick - 0.5).max(0.5);
                painter.line_segment([at - across, at + across], Stroke::new(2.0, Color32::WHITE));
            };
            if let Some(train) = &self.hot_elbow_tracer {
                for position in train.positions() {
                    bar(&paths[2], position as f32, legs[2].2);
                }
            }
            if let Some(train) = &self.cold_elbow_tracer {
                for leg in 0..2 {
                    for position in train.positions() {
                        bar(&paths[leg], 1.0 - position as f32, legs[leg].2);
                    }
                }
            }
        }

        // ── Nozzles ─────────────────────────────────────────────────────────
        //
        // Water side on the right, facing the turbine hall: feedwater in low,
        // superheated steam out high. Helium connections are deliberately NOT
        // drawn here — where they attach is the coaxial duct's business, and
        // `CoaxialDuctVisual` owns that.
        // Marks along a horizontal nozzle run.
        //
        // `inlet_at_left` says which end the stream ENTERS by — geometry of
        // the connection, not a direction of travel. Position 0 sits at that
        // end, and which way the marks then move comes from the sign of the
        // mass flow the caller advanced the train with.
        let nozzle_marks = |run: Rect, train: &TracerTrain, inlet_at_left: bool| {
            let mark_w = (run.width() * 0.10).max(1.5);
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

        let steam_run = Rect::from_min_max(
            Pos2::new(cx + w * NOZZLE_INNER_X, y(STEAM_NOZZLE_SPAN.0)),
            Pos2::new(cx + w * NOZZLE_OUTER_X, y(STEAM_NOZZLE_SPAN.1)),
        );
        painter.rect_filled(steam_run, 2, self.colour(self.steam_temp));
        // Steam LEAVES the vessel, so its inlet is the left (vessel) end.
        if let Some(train) = &self.steam_tracer {
            nozzle_marks(steam_run, train, true);
        }
        self.tag(&labels, Pos2::new(cx + w * 0.86, y(0.0975)), "steam");

        let feedwater_run = Rect::from_min_max(
            // Just above the bundle bottom, where the coil takes feedwater in.
            Pos2::new(cx + w * NOZZLE_INNER_X, y(feedwater_nozzle_span().0)),
            Pos2::new(cx + w * NOZZLE_OUTER_X, y(feedwater_nozzle_span().1)),
        );
        painter.rect_filled(feedwater_run, 2, self.colour(self.feedwater_temp));
        // Feedwater ENTERS the vessel from the turbine hall, so its inlet is
        // the right (outboard) end and its marks run right-to-left at a
        // positive flow — the opposite way to the steam above it, which is the
        // whole point of drawing both.
        if let Some(train) = &self.feedwater_tracer {
            nozzle_marks(feedwater_run, train, false);
        }
        self.tag(
            &labels,
            Pos2::new(cx + w * 0.88, y(BUNDLE_BOTTOM_FRACTION + 0.0025)),
            "feedwater",
        );

        painter.rect_stroke(shell, 0, Stroke::new(1.5, OUTLINE), StrokeKind::Middle);

        response
    }
}


/// Number of drawn turns and the stroke width, points, for a coil of drawn
/// height `span`, bundle width `bundle_w` and helix radius `coil_radius`,
/// wound at pitch angle `theta` (radians).
///
/// The turn count comes from the helix relation `tan(theta) = p / (2 pi r)`,
/// so over a height `H` the coil makes `H / (2 pi r tan theta)` turns and a
/// shallower angle winds more of them. The stroke is 15 % of the bundle width.
///
/// **Adjacent turns never touch, at any drawn size** (maintainer direction,
/// 2026-09-22). Two limits are applied. First, at least [`MIN_COIL_GAP`] of
/// clear space is kept between turns by thinning the stroke. Second, where
/// even a [`MIN_COIL_STROKE`] line will not fit, fewer turns are drawn. The
/// second limit is a level-of-detail choice: on a very small drawing the turn
/// count is set by what can be seen, not by the coil angle.
fn coil_turns_and_stroke(span: f32, bundle_w: f32, coil_radius: f32, theta: f32) -> (f32, f32) {
    let tan_theta = theta.tan().abs().max(0.02);
    let mut turns = (span / (2.0 * PI * coil_radius * tan_theta)).clamp(2.0, 40.0);
    let min_pitch = MIN_COIL_STROKE + MIN_COIL_GAP;
    if span / turns < min_pitch {
        turns = (span / min_pitch).max(1.0);
    }
    let pitch = span / turns;
    let width = (bundle_w * 0.15)
        .min(pitch - MIN_COIL_GAP)
        .max(MIN_COIL_STROKE);
    (turns, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kelvins(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    fn visual() -> Htr10SteamGeneratorVisual {
        Htr10SteamGeneratorVisual::new(
            Vec2::new(120.0, 480.0),
            kelvins(300.0),
            kelvins(1200.0),
            kelvins(973.15),
            kelvins(523.15),
            kelvins(377.15),
            kelvins(713.15),
        )
    }

    /// The gas port is on the vessel's left wall, in the inlet space below
    /// the bundles and above the bottom of the cavity.
    #[test]
    fn the_gas_port_is_on_the_left_wall_below_the_bundles() {
        let v = visual();
        let r = Rect::from_min_size(Pos2::new(15.0, 30.0), v.size());
        let port = v.gas_port(r);
        let vessel = fit_native_aspect(r);
        assert!((port.x - vessel.left()).abs() < 1e-4, "on the left wall");
        let f = (port.y - vessel.top()) / vessel.height();
        assert!(f > BUNDLE_BOTTOM_FRACTION && f < INTERIOR_BOTTOM_FRACTION);
    }

    /// Both water ports are at the outer ends of their nozzles, right of the
    /// vessel; steam high, feedwater low, just above the bundle bottom.
    #[test]
    fn water_ports_are_right_of_the_vessel_steam_above_feedwater() {
        let v = visual();
        let r = Rect::from_min_size(Pos2::new(15.0, 30.0), v.size());
        let vessel = fit_native_aspect(r);
        let (s, f) = (v.steam_port(r), v.feedwater_port(r));
        assert!(s.x > vessel.right() && (s.x - f.x).abs() < 1e-4);
        assert!(s.y < f.y, "steam leaves above the feedwater inlet");
        let ff = (f.y - vessel.top()) / vessel.height();
        assert!((ff - BUNDLE_BOTTOM_FRACTION).abs() < 0.02);
    }

    /// Each elbow runs level from its start, turns up once, and ends exactly
    /// above the turn at `end_y`.
    #[test]
    fn an_elbow_runs_level_then_rises_at_the_turn() {
        let start = Pos2::new(0.0, 100.0);
        let path = elbow_up(start, 40.0, 20.0, 10.0);
        assert_eq!(path[0], start);
        let end = *path.last().unwrap();
        assert_eq!(
            end,
            Pos2::new(40.0, 20.0),
            "vertical leg at turn_x to end_y"
        );
        for p in &path {
            assert!(
                p.x >= start.x - 1e-4 && p.x <= 40.0 + 1e-4,
                "never overshoots the turn"
            );
            assert!(
                p.y <= start.y + 1e-4 && p.y >= 20.0 - 1e-4,
                "never dips or overshoots"
            );
        }
        // A radius too big for the rise is reduced, not overshot.
        let tight = elbow_up(start, 40.0, 95.0, 50.0);
        assert!(tight.iter().all(|p| p.y >= 95.0 - 1e-4));
    }

    /// The riser defaults to the maintainer-specified 40 % of the diameter.
    ///
    /// Pinned because it is a *specification*, not an implementation detail:
    /// if it drifts, the drawing no longer matches what was asked for.
    #[test]
    fn riser_defaults_to_forty_percent_of_the_diameter() {
        assert!((visual().drawn_riser_fraction() - 0.40).abs() < 1e-6);
        assert!((DEFAULT_RISER_DIAMETER_FRACTION - 0.40).abs() < 1e-6);
    }

    /// Adjacent coil turns keep a clear gap at every drawn size (maintainer
    /// direction, 2026-09-22: the turns overlapped at some zooms).
    ///
    /// Method: sweep the bundle width from 2 to 400 pt at three height
    /// ratios, including the drawn one (a 0.7-height span over a bundle
    /// 0.064 of the height wide, about 11:1), and check that the clear space
    /// between turns, pitch minus stroke, is at least [`MIN_COIL_GAP`]. Then
    /// check that a large drawing still takes its turn count from the helix
    /// relation, so the level-of-detail limit only acts on small drawings.
    ///
    /// Before the fix, the drawn ratio at the plant view's smallest zoom gave
    /// about 2.4 pt between turns, roughly 1.4 pt after edge feathering, and
    /// less on smaller drawings.
    #[test]
    fn coil_turns_never_touch_at_any_size() {
        let theta = (DEFAULT_COIL_ANGLE_DEGREES as f32).to_radians();
        for ratio in [3.0_f32, 11.0, 20.0] {
            for step in 0..200 {
                let bundle_w = 2.0 + 2.0 * step as f32;
                let span = ratio * bundle_w;
                let (turns, width) =
                    coil_turns_and_stroke(span, bundle_w, 0.34 * bundle_w, theta);
                let gap = span / turns - width;
                assert!(
                    gap >= MIN_COIL_GAP - 1e-3,
                    "bundle {bundle_w} pt, span {span} pt: {turns:.1} turns, \
                     stroke {width:.2} pt leaves {gap:.2} pt"
                );
            }
        }
        // A large drawing is unaffected: turns follow tan(theta) = p / (2 pi r).
        let (bundle_w, span) = (100.0_f32, 300.0_f32);
        let (turns, width) = coil_turns_and_stroke(span, bundle_w, 34.0, theta);
        let expected = span / (2.0 * PI * 34.0 * theta.tan());
        assert!((turns - expected).abs() < 1e-3, "{turns} vs {expected}");
        assert!((width - 15.0).abs() < 1e-3, "15 % of the bundle: {width}");
    }

    /// The coil angle defaults to the specified 7 degrees (revised down from
    /// 20 when the coils became a continuous winding).
    #[test]
    fn coil_angle_defaults_to_seven_degrees() {
        assert!((DEFAULT_COIL_ANGLE_DEGREES - 7.0).abs() < 1e-9);
    }

    /// An out-of-range riser fraction is clamped rather than drawn, so a
    /// caller that oversteers gets a readable picture instead of a bundle of
    /// zero width or a riser wider than the vessel.
    #[test]
    fn riser_fraction_is_clamped_to_a_readable_range() {
        assert!((visual().with_riser_diameter_fraction(5.0).drawn_riser_fraction() - 0.8).abs() < 1e-6);
        assert!((visual().with_riser_diameter_fraction(-1.0).drawn_riser_fraction() - 0.1).abs() < 1e-6);
    }

    /// The water side is coldest at the bottom: feedwater enters low and
    /// superheated steam leaves at the top. `f` is measured from the TOP, so
    /// `f = 1` must be the feedwater end.
    #[test]
    fn water_side_is_coldest_at_the_bottom() {
        let v = visual();
        let top = v.water_temperature_at(0.0).get::<kelvin>();
        let bottom = v.water_temperature_at(1.0).get::<kelvin>();
        assert!(
            bottom < top,
            "feedwater end ({bottom} K) must be colder than the steam end ({top} K)"
        );
        assert!((top - 713.15).abs() < 1e-6);
        assert!((bottom - 377.15).abs() < 1e-6);
    }

    /// The vessel keeps its real slenderness whatever box it is given, so a
    /// wide panel cannot draw a squat generator.
    #[test]
    fn the_vessel_letterboxes_to_its_own_proportions() {
        let wide = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 300.0)));
        let tall = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 900.0)));
        for r in [wide, tall] {
            let aspect = r.width() / r.height();
            assert!(
                (aspect - HTR10_SG_ASPECT_RATIO).abs() < 1e-4,
                "aspect {aspect} should match {HTR10_SG_ASPECT_RATIO}"
            );
        }
    }
}
