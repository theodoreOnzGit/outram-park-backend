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
//!    ┌───────────────────────────┐
//!    │   ┌───┐           ┌───┐   │  cold helium RETURNS LOW, through the
//!    │   │ ↑ │  ┌─────┐  │ ↑ │   │  annulus of the coaxial duct
//!    │   │ ↑ │  │ bed │  │ ↑ │   │
//!    │   │ ↑ │  │  ↓  │  │ ↑ │   │  1. DOWN the short remaining annulus
//!    │   └───┘  └──┬──┘  └───┘   │     to the bottom cavity
//!    │             ▼             │  2. UP the coolant boreholes in the
//!    │   ═══ hot plenum ═══╪═════▶     side reflector
//!    │ ↓                 ↓ │ cold │ 3. DOWN through the pebble bed to the
//!    │ ↓   bottom cavity  ↓│ hot  │    hot plenum, out through the duct's
//!    │ └────────↓─────────┘│      │    inner tube
//!    └─────────────────────┴──────┘
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
//! Proportions come from the plant-data sheet wherever it has a number, and
//! every one is marked at its constant. Where it records *Unknown*, this
//! widget says so rather than inventing a figure. **It is a schematic, not a
//! scale drawing and not a reproduction of any published figure.**
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
//! - **The boreholes are drawn as one continuous run**, U-bend and vertical
//!   climb together, with marks placed along it by ARC LENGTH. The bend is
//!   where the gas actually reverses — it arrives at the bottom cavity going
//!   down and leaves going up — and drawing it as a bend rather than two
//!   disconnected lines is what makes that reversal legible. Placing marks by
//!   vertex index instead would bunch them at the corner, where the path is
//!   finely sampled, and they would appear to jump it.
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

/// Outer proportions of the HTR-10 reactor pressure vessel, width / height.
///
/// From `docs/reactor-scoping/htr10-plant-data.md` section 4.2: RPV height
/// **more than 11 m** and diameter **more than 4 m**, both *Quoted* as bounds
/// rather than figures. Using 4 m / 11 m makes this a **lower bound on
/// slenderness** — the real vessel is at least this slender, never squatter.
pub const HTR10_RPV_ASPECT_RATIO: f32 = 4.0 / 11.0;

/// Pebble-bed diameter as a fraction of the vessel diameter.
///
/// Derived from *Quoted* figures: core diameter **1.8 m** (section 4.3, three
/// sources agree) over the 4 m vessel bound above.
pub const CORE_DIAMETER_FRACTION: f32 = 1.8 / 4.0;

/// Average bed height as a fraction of the vessel height.
///
/// Derived from *Quoted* figures: average core height **1.97 m** (section 4.3)
/// over the 11 m vessel bound. Worth seeing drawn: the bed is under a fifth of
/// the vessel's height, which a schematic that fills the vessel with pebbles
/// badly misrepresents.
pub const CORE_HEIGHT_FRACTION: f32 = 1.97 / 11.0;

/// Fuel discharge tube diameter as a fraction of the vessel diameter.
///
/// Derived from a *Quoted* **500 mm** tube (section 4.3, two sources agree)
/// over the 4 m vessel bound.
pub const DISCHARGE_TUBE_DIAMETER_FRACTION: f32 = 0.5 / 4.0;

/// Fuel discharge tube length as a fraction of the vessel height.
///
/// Derived from a *Quoted* **about 3.3 m** (section 4.3) over the 11 m bound.
/// The tube is longer than the bed is tall, which is the detail that makes the
/// pebble-handling route read as a real part of the machine.
pub const DISCHARGE_TUBE_LENGTH_FRACTION: f32 = 3.3 / 11.0;

/// Number of coolant boreholes drawn in the side reflector, per side.
///
/// The real count is **20** around the full circumference (section 4.3, three
/// sources agree). A longitudinal section cannot show twenty without becoming
/// a smear, so three per side are drawn and the real number is stated here and
/// in the widget's label rather than implied by counting.
const DRAWN_RISERS_PER_SIDE: usize = 3;

/// Real number of coolant boreholes, for the label.
pub const COOLANT_BOREHOLES: usize = 20;

const OUTLINE: Color32 = Color32::from_rgb(150, 154, 162);
const GRAPHITE: Color32 = Color32::from_rgb(58, 60, 66);
const INTERNALS: Color32 = Color32::from_rgb(64, 68, 76);
const VOID: Color32 = Color32::from_rgb(28, 30, 34);
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);

/// A U-bend pick-up: down the annulus, round the turn, up the borehole.
///
/// `entry` is where the run starts in the annulus, `riser_x` the borehole's
/// centreline, `bend_y` how deep the turn reaches, and `top_y` the top of the
/// climb. Returned as a polyline so a tracer can be placed along it by arc
/// length — the mark then travels the bend at the same speed as the straights,
/// which is what stops it appearing to jump the corner.
fn u_bend_path(entry_x: f32, entry_y: f32, riser_x: f32, bend_y: f32, top_y: f32) -> Vec<Pos2> {
    let mut points = vec![Pos2::new(entry_x, entry_y), Pos2::new(entry_x, bend_y)];
    let mid_x = 0.5 * (entry_x + riser_x);
    let bulge = (0.45 * (riser_x - entry_x).abs()).max(2.0);
    let arc_samples = 14;
    for i in 1..arc_samples {
        let a = PI * i as f32 / arc_samples as f32;
        points.push(Pos2::new(
            mid_x + (entry_x - mid_x) * a.cos(),
            bend_y + bulge * a.sin(),
        ));
    }
    points.push(Pos2::new(riser_x, bend_y));
    points.push(Pos2::new(riser_x, top_y));
    points
}

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
    /// Control rods default to **fully inserted**, so a caller that forgets to
    /// drive them draws a shut-down core rather than a critical one.
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
            control_rod_insertion_frac: 1.0,
            show_labels: true,
            downcomer_tracer: None,
            riser_tracer: None,
            plenum_tracer: None,
            cold_plenum_tracer: None,
        }
    }

    /// On-screen size, in points.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Where the control-rod bank is **drawn**, `0.0` out to `1.0` in.
    /// Clamped at render time.
    pub fn with_control_rod_frac(mut self, frac: f32) -> Self {
        self.control_rod_insertion_frac = frac;
        self
    }

    /// Turn the labels off — for thumbnails. Builder-style.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Pass 1: cold helium descending the RPV-to-core-barrel annulus.
    ///
    /// Advance with the primary mass flow. Its inlet is the **top** of the
    /// annulus — cold helium arrives from the coaxial duct and runs down the
    /// vessel wall, which is what holds the pressure boundary near 250 degC.
    pub fn with_downcomer_tracer(mut self, tracer: TracerTrain) -> Self {
        self.downcomer_tracer = Some(tracer);
        self
    }

    /// Pass 2: cold helium climbing the side-reflector boreholes.
    ///
    /// Advance with the primary mass flow. Its inlet is the **bottom**.
    pub fn with_riser_tracer(mut self, tracer: TracerTrain) -> Self {
        self.riser_tracer = Some(tracer);
        self
    }

    /// Cold helium converging in the upper cold plenum.
    ///
    /// Advance with the primary mass flow. Its inlets are the plenum's
    /// **outer ends**, where the boreholes deliver, and the two streams run
    /// inward to meet on the axis before turning down into the bed. At a
    /// reversed flow they run outward instead, which is what a reversed loop
    /// would really do.
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
    /// Draws the vessel, the three-pass helium path, the bed with its
    /// discharge cone, the hot plenum and the fuel discharge tube.
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(self.size, Sense::hover());
        self.control_rod_insertion_frac = self.control_rod_insertion_frac.clamp(0.0, 1.0);

        let rect = fit_native_aspect(response.rect);
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;
        let y = |f: f32| rect.top() + f * h;

        let cold = self.colour(self.inlet_temp);
        let hot = self.colour(self.outlet_temp);

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

        // Interior void the internals sit in.
        let interior = Rect::from_min_max(
            Pos2::new(cx - w * 0.44, y(0.045)),
            Pos2::new(cx + w * 0.44, y(0.955)),
        );
        painter.rect_filled(interior, (w * 0.05).round() as u8, VOID);

        // ── The cold annulus, between the RPV and the core barrel ──────────
        //
        // Drawn cold over its FULL height, because it is: section 4.2 records
        // it as "filled with 250 degC cold helium to hold vessel temperature
        // below limit". Being full of cold helium is what protects the
        // pressure boundary, and that is true whether or not gas is moving
        // through a given part of it.
        //
        // The moving part is a different, shorter thing — see
        // `downcomer_rects` below.
        let annulus_outer = w * 0.44;
        let annulus_inner = w * 0.375;
        let annulus_top = y(0.085);
        let annulus_bottom = y(0.845);
        for side in [-1.0_f32, 1.0] {
            let a = cx + side * annulus_outer;
            let b = cx + side * annulus_inner;
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(a.min(b), annulus_top),
                    Pos2::new(a.max(b), annulus_bottom),
                ),
                1,
                cold,
            );
        }

        // Core barrel: the metal the annulus runs against.
        for side in [-1.0_f32, 1.0] {
            let x = cx + side * annulus_inner;
            painter.line_segment(
                [
                    Pos2::new(x, annulus_top),
                    Pos2::new(x, annulus_bottom),
                ],
                Stroke::new(1.6, INTERNALS),
            );
        }

        // ── Graphite reflector ─────────────────────────────────────────────
        //
        // Graphite base with the reflector's own temperature washed over it:
        // it has to keep reading as graphite rather than as another fluid, but
        // it is a real thermal mass — it carries most of the core's heat
        // capacity during a transient — so a reader must be able to see it
        // heat up. A flat fill would make `reflector_temp` a field the widget
        // accepts and ignores.
        let reflector = Rect::from_min_max(
            Pos2::new(cx - w * 0.355, y(0.135)),
            Pos2::new(cx + w * 0.355, y(0.815)),
        );
        painter.rect_filled(reflector, 2, GRAPHITE);
        let refl = self.colour(self.reflector_temp);
        painter.rect_filled(
            reflector,
            2,
            Color32::from_rgba_unmultiplied(refl.r(), refl.g(), refl.b(), 110),
        );
        painter.rect_stroke(reflector, 2, Stroke::new(1.0, INTERNALS), StrokeKind::Middle);

        // ── Pass 2: side-reflector coolant boreholes ───────────────────────
        //
        // Each borehole is drawn as ONE continuous run: a U-bend picking the
        // gas up from the foot of the annulus, then the vertical climb. The
        // bend is the real turn — gas reaches the bottom cavity going down and
        // has to reverse to go up the reflector — and drawing it as a bend
        // rather than two disconnected lines is what makes the reversal
        // legible. Marks then travel the whole path without teleporting.
        //
        // The three per side nest, shallowest nearest the wall, so they read
        // as a manifold rather than as three lines crossing.
        let riser_top = y(0.215);
        let annulus_mid = 0.5 * (annulus_outer + annulus_inner);
        let mut riser_paths: Vec<Vec<Pos2>> = Vec::new();
        for side in [-1.0_f32, 1.0] {
            for k in 0..DRAWN_RISERS_PER_SIDE {
                let f = 0.255 + 0.030 * k as f32;
                let riser_x = cx + side * w * f;
                let entry_x = cx + side * annulus_mid;
                let bend_y = y(0.800) + h * 0.018 * k as f32;
                riser_paths.push(u_bend_path(entry_x, y(0.760), riser_x, bend_y, riser_top));
            }
        }
        let riser_width = (w * 0.020).max(1.5);
        for path in &riser_paths {
            for seg in path.windows(2) {
                painter.line_segment([seg[0], seg[1]], Stroke::new(riser_width, cold));
            }
        }

        // ── Cold helium plenum, where pass 2 turns into pass 3 ─────────────
        let cold_plenum = Rect::from_min_max(
            Pos2::new(cx - w * 0.30, y(0.170)),
            Pos2::new(cx + w * 0.30, y(0.208)),
        );
        painter.rect_filled(cold_plenum, 2, cold);
        self.tag(&painter, cold_plenum.center(), "cold plenum");

        // ── Pebble bed ─────────────────────────────────────────────────────
        //
        // Drawn at its real share of the vessel: under a fifth of the height,
        // and under half the diameter.
        let bed_half_w = w * 0.5 * CORE_DIAMETER_FRACTION;
        let bed_top = y(0.235);
        let bed_bottom = bed_top + h * CORE_HEIGHT_FRACTION;
        let bed = Rect::from_min_max(
            Pos2::new(cx - bed_half_w, bed_top),
            Pos2::new(cx + bed_half_w, bed_bottom),
        );
        painter.rect_filled(bed, 2, self.colour(self.pebble_temp));

        // Pebbles, as a simple stipple — enough to read as a packed bed
        // without pretending to be a packing calculation.
        let pebble_r = (w * 0.016).max(1.0);
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

        // Discharge cone, funnelling the bed into the tube.
        let cone_bottom = bed_bottom + h * 0.075;
        let tube_half_w = w * 0.5 * DISCHARGE_TUBE_DIAMETER_FRACTION;
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(bed.left(), bed_bottom),
                Pos2::new(bed.right(), bed_bottom),
                Pos2::new(cx + tube_half_w, cone_bottom),
                Pos2::new(cx - tube_half_w, cone_bottom),
            ],
            self.colour(self.pebble_temp),
            Stroke::new(1.0, INTERNALS),
        ));
        self.tag(&painter, Pos2::new(cx, bed.center().y), "pebble bed");

        // ── Hot helium plenum, in the bottom reflector ─────────────────────
        let plenum = Rect::from_min_max(
            Pos2::new(cx - w * 0.30, cone_bottom + h * 0.012),
            Pos2::new(cx + w * 0.30, cone_bottom + h * 0.058),
        );
        painter.rect_filled(plenum, 2, hot);
        self.tag(&painter, Pos2::new(cx, plenum.center().y - h * 0.030), "hot plenum");

        // ── The coaxial duct nozzle ────────────────────────────────────────
        //
        // ONE connection, carrying both streams, and its elevation is the
        // reason the coolant path is shaped the way it is: the hot inner tube
        // has to meet the hot plenum, and the hot plenum is in the BOTTOM
        // reflector — so the whole duct, cold annulus included, attaches LOW
        // on the vessel.
        //
        // Cold helium therefore RETURNS NEAR THE BOTTOM, not at the top. It is
        // drawn as the outer body here with the hot tube inside it, matching
        // `CoaxialDuctVisual`.
        let coax = Rect::from_min_max(
            Pos2::new(cx + w * 0.30, plenum.top() - plenum.height() * 0.45),
            Pos2::new(cx + w * 0.62, plenum.bottom() + plenum.height() * 0.45),
        );
        painter.rect_filled(coax, 2, cold);
        painter.rect_stroke(coax, 2, Stroke::new(1.0, INTERNALS), StrokeKind::Middle);
        let nozzle = Rect::from_min_max(
            Pos2::new(cx + w * 0.30, plenum.top() + plenum.height() * 0.22),
            Pos2::new(cx + w * 0.62, plenum.bottom() - plenum.height() * 0.22),
        );
        painter.rect_filled(nozzle, 2, hot);

        // The moving part of the annulus: from where the cold gas actually
        // enters, down to the bottom cavity.
        //
        // Section 4.4 step 4 — into the RPV through the space between the
        // vessel and the core barrel, "down to the bottom of the reactor
        // support structure, cooling the support structure first" — then step
        // 5 turns it up the reflector boreholes. That descent starts at the
        // duct, which is low, so it is a SHORT run near the foot of the
        // vessel, not a full-height downcomer.
        let cold_entry_y = coax.center().y;
        let mut downcomer_rects = Vec::new();
        for side in [-1.0_f32, 1.0] {
            let a = cx + side * annulus_outer;
            let b = cx + side * annulus_inner;
            downcomer_rects.push(Rect::from_min_max(
                Pos2::new(a.min(b), cold_entry_y),
                Pos2::new(a.max(b), annulus_bottom),
            ));
        }

        // ── Fuel discharge tube — the pebble-handling route ────────────────
        let tube_top = cone_bottom;
        let tube_bottom = tube_top + h * DISCHARGE_TUBE_LENGTH_FRACTION;
        let tube = Rect::from_min_max(
            Pos2::new(cx - tube_half_w, tube_top),
            Pos2::new(cx + tube_half_w, tube_bottom.min(rect.bottom() - h * 0.01)),
        );
        painter.rect_filled(tube, 2, self.colour(self.pebble_temp));
        painter.rect_stroke(tube, 2, Stroke::new(1.2, INTERNALS), StrokeKind::Middle);
        self.tag(
            &painter,
            Pos2::new(cx + w * 0.20, tube.center().y),
            "discharge",
        );

        // ── Control rods, in the side reflector ────────────────────────────
        //
        // Ten channels in the real vessel, all in the side reflector — there
        // are no in-core rods. Two are drawn, one per side.
        let rod_travel_top = y(0.150);
        let rod_travel_bottom = y(0.640);
        for side in [-1.0_f32, 1.0] {
            let x = cx + side * w * 0.335;
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x - w * 0.016, rod_travel_top),
                    Pos2::new(x + w * 0.016, rod_travel_bottom),
                ),
                1,
                VOID,
            );
            let tip = rod_travel_top
                + (rod_travel_bottom - rod_travel_top) * self.control_rod_insertion_frac;
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x - w * 0.016, rod_travel_top),
                    Pos2::new(x + w * 0.016, tip),
                ),
                1,
                Color32::from_rgb(196, 200, 208),
            );
        }

        // ── Tracers ────────────────────────────────────────────────────────
        //
        // Every pass below takes its DIRECTION from the sign of the mass flow
        // the caller advanced its train with; only the inlet END is fixed
        // here, and that is geometry.
        let vertical_marks =
            |run: Rect, train: &TracerTrain, inlet_at_top: bool, colour: Color32| {
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
                        colour,
                    );
                }
            };

        // Pass 1, inlet at the TOP: cold helium enters high and runs down.
        if let Some(train) = &self.downcomer_tracer {
            for r in &downcomer_rects {
                vertical_marks(*r, train, true, Color32::WHITE);
            }
        }

        // Pass 2, inlet at the ANNULUS end of the U: the gas is picked up at
        // the foot, turns through the bend, and climbs. Marks are placed by
        // arc length along the whole run, so they round the corner at the same
        // speed they travel the straights.
        if let Some(train) = &self.riser_tracer {
            let mark = (riser_width * 0.85).max(1.5);
            for path in &riser_paths {
                for position in train.positions() {
                    let p = point_along(path, position as f32);
                    painter.circle_filled(p, mark * 0.5, Color32::WHITE);
                }
            }
        }

        // Cold plenum, inlet at BOTH OUTER ENDS: the boreholes deliver up the
        // sides and the gas converges on the axis before turning down into the
        // bed. Drawn as two opposed streams running inward, which is the one
        // place in the vessel where flow visibly meets itself.
        if let Some(train) = &self.cold_plenum_tracer {
            let mark_w = (cold_plenum.width() * 0.045).max(1.5);
            let half = cold_plenum.width() * 0.5;
            for position in train.positions() {
                for side in [-1.0_f32, 1.0] {
                    // position 0 at the outer end, 1 at the centre.
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

        // Hot plenum, inlet at the AXIS: flow arrives from the bed above and
        // leaves sideways through the nozzle. Drawn on both halves, running
        // outward from the centre.
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
                Pos2::new(cx, y(0.965)),
                &format!("{COOLANT_BOREHOLES} boreholes (3/side drawn)"),
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
            Vec2::new(200.0, 550.0),
            kelvins(300.0),
            kelvins(1200.0),
            kelvins(1100.0),
            kelvins(523.15),
            kelvins(973.15),
            kelvins(900.0),
            kelvins(530.0),
        )
    }

    /// The cited proportions are what the sheet says, not eyeballed values.
    ///
    /// Pinned because they are the widget's only claim to representing the
    /// real machine: if one drifts, the drawing silently stops matching the
    /// dimensions it cites.
    #[test]
    fn proportions_match_the_cited_dimensions() {
        assert!((HTR10_RPV_ASPECT_RATIO - 4.0 / 11.0).abs() < 1e-6);
        assert!((CORE_DIAMETER_FRACTION - 1.8 / 4.0).abs() < 1e-6);
        assert!((CORE_HEIGHT_FRACTION - 1.97 / 11.0).abs() < 1e-6);
        assert!((DISCHARGE_TUBE_DIAMETER_FRACTION - 0.5 / 4.0).abs() < 1e-6);
        assert!((DISCHARGE_TUBE_LENGTH_FRACTION - 3.3 / 11.0).abs() < 1e-6);
    }

    /// The bed occupies well under a fifth of the vessel height, and the
    /// discharge tube is LONGER than the bed is tall.
    ///
    /// Both are easy to get wrong by eye — a schematic drawn from intuition
    /// tends to fill the vessel with pebbles and stub the tube — so they are
    /// asserted rather than left to inspection.
    #[test]
    fn the_bed_is_a_small_part_of_the_vessel() {
        assert!(
            CORE_HEIGHT_FRACTION < 0.20,
            "bed height fraction {CORE_HEIGHT_FRACTION} should be under 0.20"
        );
        assert!(
            DISCHARGE_TUBE_LENGTH_FRACTION > CORE_HEIGHT_FRACTION,
            "the discharge tube ({DISCHARGE_TUBE_LENGTH_FRACTION}) should be longer \
             than the bed is tall ({CORE_HEIGHT_FRACTION})"
        );
    }

    /// Control rods default to fully inserted: a caller that forgets to drive
    /// them draws a shut-down core, never a critical one.
    #[test]
    fn control_rods_default_to_inserted() {
        let v = visual();
        assert!((v.control_rod_insertion_frac - 1.0).abs() < 1e-6);
    }

    /// The vessel keeps its slenderness whatever box it is given.
    #[test]
    fn the_vessel_letterboxes_to_its_own_proportions() {
        let wide = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 300.0)));
        let tall = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 900.0)));
        for r in [wide, tall] {
            let aspect = r.width() / r.height();
            assert!((aspect - HTR10_RPV_ASPECT_RATIO).abs() < 1e-4);
        }
    }

    /// Only three boreholes per side are drawn, and the real count is carried
    /// separately so the label can state it rather than a reader counting.
    #[test]
    fn the_drawn_borehole_count_is_not_the_real_one() {
        assert_eq!(COOLANT_BOREHOLES, 20);
        assert!(DRAWN_RISERS_PER_SIDE * 2 < COOLANT_BOREHOLES);
    }
}
