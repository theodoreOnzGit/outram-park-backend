//! Plant schematic panel, drawn on the engine's **real component artwork**
//! ([`outram_park_digital_twin_engine::components`]) in the arrangement an
//! HTR-10-style pebble-bed HTGR actually has.
//!
//! ## The arrangement is the point
//!
//! The defining feature of this plant is not the equipment list, it is the
//! layout. Following the description in IAEA-TECDOC-1382 (ingested into this
//! workspace's literature layer at
//! `crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md`):
//!
//! - The reactor and the steam generator sit in **two separate pressure
//!   vessels, side by side**, tied together by a **hot gas duct pressure
//!   vessel** (a horizontal cross-vessel). They are *not* stacked, and the
//!   steam generator is not inside the reactor vessel.
//! - The **helium circulator is installed in the steam-generator pressure
//!   vessel, above the steam generator**, connected to it by a connecting
//!   tube -- so it is drawn there, not at an arbitrary point in the loop.
//! - Hot helium leaves the core sideways through the duct, gives up its heat
//!   to the helical coil, and the cooled helium is lifted by the circulator
//!   and returned to the reactor through the **annulus around the hot gas
//!   duct** -- which is why the cold return run comes back alongside the hot
//!   one where it enters the reactor vessel wall.
//!
//! ```text
//!                     [circulator]                 (in the SG vessel,
//!                          |                        above the SG)
//!   +------ cold He -------+                                    steam
//!   |                 +---------+   +---[SG]---+ --------------> [turbine] -+
//!   v                                    ^                                  |
//! [HTR-10 vessel] ==== hot gas duct ======+                                  v
//!        ^                                                             [condenser]
//!        |                                                                   |
//!        +--- feedwater <--- [feed pump] <--- condensate <-------------------+
//! ```
//!
//! ## Widgets used
//!
//! All artwork comes from the engine crate; nothing is re-drawn locally except
//! text annotations and the dashed boundary marking the steam-generator
//! pressure vessel.
//!
//! | Widget | Role here |
//! |---|---|
//! | [`Htr10ReactorVesselVisual`] | the reactor -- capsule vessel, graphite reflector, settled pebble bed, discharge cone, hot gas plenum and duct nozzle |
//! | [`SteamGeneratorVisual`] ([`SteamGeneratorKind::HelicalCoil`]) | the once-through helical-coil SG, the correct architecture for this plant |
//! | [`PumpVisual`] ([`PumpKind::Centrifugal`]) | the helium circulator and the feedwater pump |
//! | [`TurbineVisual`] ([`TurbineFlowPath::SingleFlow`]) | the steam turbine -- single flow, matching a superheated once-through supply |
//! | [`PipeVisual`] / [`PipeBendVisual`] | every connector run and every elbow |
//! | [`CondenserVisual`] | the condenser closing the Rankine cycle |
//! | [`TemperatureLegend`] | the one colour scale every widget above is graded against |
//! | [`InstrumentationVisual`] | the numeric readouts |
//!
//! ## One colour scale for the whole plant
//!
//! Every widget is given the same display range, [`DISPLAY_MIN_K`] to
//! [`DISPLAY_MAX_K`], so a colour means the same thing wherever it appears and
//! a single [`TemperatureLegend`] explains all of it. The map is diverging
//! (blue / neutral white / red), so the midpoint carries meaning: it is set at
//! 800 K, between the helium loop's cold and hot ends, which puts the cold
//! helium and the whole water side on the blue half and the hot helium and the
//! fuel on the red half. That the secondary side reads cooler than the primary
//! is not a drawing choice -- it is true, and worth being able to see.
//!
//! The range is chosen against the plant's actual operating point rather than
//! guessed: helium 523 K to 973 K (250 to 700 degC), main steam 713 K
//! (440 degC), feedwater 377 K (104 degC), hotwell condensate about 312 K. All
//! of those land strictly inside 300-1300 K, so nothing renders pinned at
//! either end, and the headroom above 973 K is deliberate -- it is where the
//! core goes during a reactivity insertion, which is the one thing the
//! operator most needs to see move.
//!
//! ## Connectors are real pipe widgets, with flow tracers
//!
//! ## Pipe ends are derived from the artwork, never eyeballed
//!
//! Every connector run terminates on a nozzle anchor computed from the
//! component artwork's own drawn rectangle -- [`reactor_duct_nozzle`] for the
//! reactor and [`SgNozzles`] for the steam generator -- using the same
//! fractions the widget paints the nozzle stub at. Move a component, resize its
//! box, or change its aspect ratio, and the pipe ends follow. A hardcoded
//! screen position that happens to line up today is exactly how the hot gas
//! duct came to terminate on the steam generator's *left flank*, at an
//! elevation set by the reactor, when the artwork's hot-gas nozzle is on the
//! **bottom right** -- see [`hot_gas_duct_path`].
//!
//! Connector runs are [`PipeVisual`]s built through
//! [`PipeVisual::from_scalars`], not raw `painter` lines. A full
//! `tampines::components::Pipe` would need a `SinglePhaseFluidArray` or
//! `CompressibleFluidArray` per connector, which is far more machinery than a
//! schematic line needs; the scalar path takes this plant's own real
//! temperature, mass flow, and residence time instead. Elbows are real
//! [`PipeBendVisual`] sectors, so a turn is a piece of geometry shared by two
//! runs rather than two rectangles butted together.
//!
//! Each run carries a [`TracerTrain`] whose marks travel at `1/residence_time`
//! of the run per second, so the animation is a direct readout of the physical
//! transport time: raise the helium flow and the primary tracers visibly speed
//! up. The trains live in [`SchematicTracers`], owned by the app and advanced
//! once per frame -- widgets are rebuilt every repaint, so a train owned by a
//! widget would reset its phase each frame.
//!
//! ## Control rods
//!
//! The side panel's rod-bank slider writes `control_rod_insertion_fraction` on
//! [`HtgrSnapshot`], and the reactor widget draws the bank in HTR-10's **side
//! reflector** borings, where the real rods are -- not in the pebble bed.
//!
//! The drawn depth is **slewed** toward the slider's setpoint rather than
//! snapping to it, so a drag produces visible rod travel. Two honesty notes:
//!
//! - The **drive speed is ILLUSTRATIVE**, not a plant figure. No published
//!   HTR-10 rod drive speed was found in this project's scoping notes or
//!   literature archive (searched 2026-08-12), so the engine uses a full stroke
//!   in 20 s purely so the motion is watchable. See
//!   `animation::control_rod_drive::htr10_illustrative_rod_drive_speed`.
//! - **Only two rods are drawn.** HTR-10 has ten rod borings; a vertical
//!   cut-away shows one each side and the rest are out of the section plane.
//!
//! ## What is deliberately *not* drawn
//!
//! [`HtgrSnapshot`] is the only source of state here, and nothing is invented
//! to feed a widget that wants more than it carries:
//!
//! - **No PUMP shaft rotation.** Neither the helium circulator nor the
//!   feedwater pump has a shaft-speed model in this plant, so both are built
//!   with `AngularVelocity::ZERO` and draw stationary. Deriving an rpm from
//!   flow or power would be inventing physics, which this crate's `CLAUDE.md`
//!   forbids.
//!
//!   The **turbine** is the exception, and it is not an exception to the rule
//!   -- it has a real shaft-speed model. [`crate::physics::turbine_generator`]
//!   runs a torque balance on the turbine-generator rotor, driven by the same
//!   enthalpy-drop power the secondary loop computes, so `TurbineVisual` is
//!   built through `new_generator` and its blades turn at the computed
//!   `omega`. Read that module's docs before quoting the speed: the machine is
//!   islanded and ungoverned, and its electrical load was sized at the rated
//!   point, so the model reproduces near-synchronous speed by construction
//!   rather than predicting it.
//!
//! - **No turbine casing colour.** A consequence of the above.
//!   `TurbineVisualState` has no variant carrying both a shaft speed and a
//!   steam state, and the generator variant reports no casing temperature, so
//!   the turbine draws neutral grey. Rotation was judged the more informative
//!   of the two; the steam temperature is still shown as the `T_steam`
//!   readout.
//! - **No feedwater control valve.** The secondary loop controls feed *flow*,
//!   and exposes no valve position; a `ValveVisual` would have to be fed a
//!   fabricated opening, so it is omitted.
//! - **No separate IHX box.** In this plant model the "IHX" duty *is* the
//!   steam-generator duty (one heat exchanger between helium and water), and
//!   in HTR-10 the SG and IHX share the one pressure vessel. Drawing a second
//!   heat-exchanger symbol would imply a stage the model does not have.
//! - **The pebble bed is coloured by the kinetics fuel temperature.** That is
//!   the only core temperature [`HtgrSnapshot`] carries. It is *not* the
//!   bed-average graphite temperature the pebble-bed model separately tracks
//!   (`HtgrPlant::pebble_temperature`), which the snapshot does not publish --
//!   so the `T_fuel` readout beside the vessel is labelled for what it is and
//!   must not be read as a bed average.
//! - **The graphite reflector is tinted at the core inlet temperature.** The
//!   plant model carries no separate graphite temperature; the reflector's
//!   channels are the ones the cold inlet helium rises through, which is what
//!   the artwork already draws them at. It is not a computed reflector
//!   temperature and must not be read as one.

use egui::{pos2, Align2, Color32, FontId, Pos2, Rect, Stroke, Ui, Vec2};

use outram_park_digital_twin_engine::animation::control_rod_drive::ControlRodDrive;
use outram_park_digital_twin_engine::animation::TracerTrain;
use outram_park_digital_twin_engine::components::htr10_reactor_vessel::{
    self, Htr10ReactorVesselVisual,
};
use outram_park_digital_twin_engine::components::pump::PumpKind;
use outram_park_digital_twin_engine::components::steam_generator::{
    SteamGeneratorKind, SteamGeneratorScalars,
};
use outram_park_digital_twin_engine::components::control_rod_drive::slewed_control_rod_insertion;
use outram_park_digital_twin_engine::components::{
    CondenserVisual, InstrumentationVisual, LegendUnit, PipeBendVisual, PipeScalars, PipeScale,
    PipeVisual, PumpVisual, SteamGeneratorVisual, TemperatureLegend, TurbineFlowPath,
    TurbineVisual,
};

use tampines::components::Condenser;
use tampines::hem::HemSteamCv;
use uom::si::angular_velocity::radian_per_second;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AngularVelocity, AvailableEnergy, MassRate, Power, Pressure, ThermodynamicTemperature, Time,
    Volume,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::{kilopascal, megapascal};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;
use uom::ConstZero;

use crate::app::state::HtgrSnapshot;
use crate::physics::turbine_generator;

// ── Display scale ───────────────────────────────────────────────────────────

/// Coldest temperature on the schematic's single colour scale \[K\].
///
/// Just below the condenser hotwell, so the cold end of the Rankine cycle
/// reads as genuinely cold rather than clipped.
pub const DISPLAY_MIN_K: f64 = 300.0;

/// Hottest temperature on the schematic's single colour scale \[K\].
///
/// Above the fuel temperatures this plant model reaches in a normal
/// manoeuvre, so the pebble bed has somewhere to go before it saturates.
pub const DISPLAY_MAX_K: f64 = 1300.0;

/// Drawn thickness of the helium (primary) runs, in screen points.
///
/// Scalar-backed runs carry no bore, so [`PipeVisual`] falls back to
/// [`PipeScale::min_thickness_points`]. This is therefore a **legibility
/// choice, not a scaled diameter** -- the primary runs are drawn heavier than
/// the water/steam runs because a gas duct really is the larger pipe, but the
/// ratio is not to scale and must not be read as one.
const HELIUM_PIPE_THICKNESS: f32 = 13.0;

/// Drawn thickness of the water/steam (secondary) runs, in screen points.
/// See [`HELIUM_PIPE_THICKNESS`] for why this is a display choice.
const STEAM_PIPE_THICKNESS: f32 = 9.0;

/// Number of tracer marks drawn on each connector run.
const TRACER_MARKS: usize = 5;

/// Colour of the annotation text and the pressure-vessel boundary.
const ANNOTATION: Color32 = Color32::from_rgb(150, 154, 162);

// ── Canvas geometry ─────────────────────────────────────────────────────────
//
// Every position below is in schematic-local points and is shifted into canvas
// coordinates by `draw_schematic`. The two vessels are laid out at ONE common
// real scale so their relative slenderness is honest: 400 points stands for the
// reactor vessel's published 11.1 m, which makes the steam-generator vessel's
// 11.3 m come out at 407 points. Each widget then letterboxes to its own
// published aspect ratio inside the box it is given.

/// Canvas size reserved for the whole schematic, in points.
const CANVAS: Vec2 = Vec2::new(1180.0, 740.0);

/// Points per metre of real vessel height, shared by both pressure vessels.
const POINTS_PER_METRE: f32 = 400.0 / 11.1;

/// Centre of the reactor-vessel box.
const REACTOR_CENTRE: Pos2 = pos2(150.0, 340.0);
/// Size of the reactor-vessel box (the artwork letterboxes inside it).
const REACTOR_BOX: Vec2 = Vec2::new(190.0, 400.0);

/// Centre of the steam-generator box.
const SG_CENTRE: Pos2 = pos2(560.0, 355.0);
/// Size of the steam-generator box; height is 11.3 m at [`POINTS_PER_METRE`].
const SG_BOX: Vec2 = Vec2::new(120.0, 11.3 * POINTS_PER_METRE);

/// Centre of the helium circulator, above the steam generator.
const CIRCULATOR_CENTRE: Pos2 = pos2(450.0, 100.0);
/// Size of the circulator box.
const CIRCULATOR_BOX: Vec2 = Vec2::new(76.0, 92.0);

/// Centre of the turbine.
const TURBINE_CENTRE: Pos2 = pos2(840.0, 197.0);
/// Size of the turbine.
const TURBINE_BOX: Vec2 = Vec2::new(156.0, 92.0);

/// Centre of the condenser.
const CONDENSER_CENTRE: Pos2 = pos2(990.0, 300.0);
/// Size of the condenser.
const CONDENSER_BOX: Vec2 = Vec2::new(86.0, 62.0);

/// Centre of the feedwater pump.
const FEED_PUMP_CENTRE: Pos2 = pos2(820.0, 655.0);
/// Size of the feedwater pump box.
const FEED_PUMP_BOX: Vec2 = Vec2::new(70.0, 84.0);

/// The dashed boundary standing for the steam-generator pressure vessel, which
/// houses the steam generator, the IHX and the helium circulator.
const SG_VESSEL_BOUNDARY: Rect = Rect {
    min: pos2(402.0, 44.0),
    max: pos2(632.0, 574.0),
};

/// Vertical centreline of the cold-helium return riser, in schematic-local
/// points.
///
/// The circulator's discharge crosses the top of the plant and drops here
/// before running in to the reactor alongside the hot gas duct.
const COLD_RETURN_RISER_X: f32 = 370.0;

/// Vertical centreline of the feedwater riser into the steam generator's
/// bottom-left nozzle, in schematic-local points.
///
/// It sits *outside* [`SG_VESSEL_BOUNDARY`], so the feedwater run crosses the
/// vessel boundary horizontally at the nozzle, which is where it crosses in
/// the plant.
const FEEDWATER_HEADER_RISER_X: f32 = 390.0;

/// Elevation of the feedwater header -- the horizontal run along the bottom of
/// the plant, from the feed pump across to [`FEEDWATER_HEADER_RISER_X`] -- in
/// schematic-local points.
const FEEDWATER_HEADER_Y: f32 = 585.0;

/// How far a connector run is pushed past a nozzle tip so the joint shows no
/// hairline gap, in points.
///
/// The nozzle anchors in [`SgNozzles`] are the *tips* of the stubs the artwork
/// paints. A run that stops exactly on a tip can leave a one-pixel seam once
/// egui rounds both rectangles to the pixel grid, so every run overlaps its
/// nozzle by this much.
const NOZZLE_SEAM_OVERLAP: f32 = 3.0;

// ── Tracer state ────────────────────────────────────────────────────────────

/// Flow-tracer state for the schematic's connector runs, owned by the app and
/// advanced once per frame.
///
/// Two trains, one per loop: every primary run shares the helium train and
/// every secondary run shares the steam train, so marks stay in step around
/// each loop. See [`crate::app::schematic`]'s module docs for why these are
/// app-owned rather than widget-owned.
#[derive(Debug, Clone, Copy)]
pub struct SchematicTracers {
    /// Marks on the helium primary runs.
    pub primary: TracerTrain,
    /// Marks on the water/steam secondary runs.
    pub secondary: TracerTrain,
}

impl SchematicTracers {
    /// Fresh, stagnant trains.
    pub fn new() -> Self {
        Self {
            primary: TracerTrain::new(TRACER_MARKS),
            secondary: TracerTrain::new(TRACER_MARKS),
        }
    }

    /// Advance both trains by one animation frame of `dt`, using the loop
    /// residence times and mass flows the physics thread published.
    ///
    /// Because the marks move at `1/residence_time` of a run per second, the
    /// on-screen speed is the real transport speed: at zero flow the residence
    /// time is unbounded and the trains freeze.
    pub fn advance(&mut self, dt: Time, snapshot: &HtgrSnapshot) {
        self.primary.advance(
            dt,
            Time::new::<second>(snapshot.helium_residence_time_s),
            MassRate::new::<kilogram_per_second>(snapshot.helium_mass_flow_kg_per_s),
        );
        self.secondary.advance(
            dt,
            Time::new::<second>(snapshot.secondary_residence_time_s),
            MassRate::new::<kilogram_per_second>(snapshot.secondary_mass_flow_kg_per_s),
        );
    }
}

impl Default for SchematicTracers {
    fn default() -> Self {
        Self::new()
    }
}

// ── Pipe routing ────────────────────────────────────────────────────────────

/// A kelvin temperature, spelled once so the routing code stays readable.
fn k(value_k: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(value_k)
}

/// One fluid stream's real state, everything a connector run needs to draw
/// itself.
///
/// Every field is read from [`HtgrSnapshot`] -- this is the caller-supplies-
/// real-state contract [`PipeVisual::from_scalars`] documents, not a stub.
#[derive(Debug, Clone, Copy)]
struct Stream {
    /// Bulk fluid temperature \[K\].
    temperature_k: f64,
    /// Mass flow \[kg/s\], which sets the tracer direction.
    mass_flow_kg_per_s: f64,
    /// Loop residence time \[s\], which sets the tracer speed.
    residence_time_s: f64,
    /// Drawn pipe thickness in points -- see [`HELIUM_PIPE_THICKNESS`].
    thickness: f32,
    /// The app-owned tracer train this stream's runs carry.
    tracer: TracerTrain,
}

impl Stream {
    /// Build one straight run of this stream, from `from` to `to`.
    fn run(&self, from: Pos2, to: Pos2) -> PipeVisual {
        PipeVisual::from_scalars(
            PipeScalars {
                temperature: k(self.temperature_k),
                mass_flow: MassRate::new::<kilogram_per_second>(self.mass_flow_kg_per_s),
                residence_time: Time::new::<second>(self.residence_time_s),
            },
            from,
            to - from,
            k(DISPLAY_MIN_K),
            k(DISPLAY_MAX_K),
        )
        .with_scale(PipeScale {
            min_thickness_points: self.thickness,
            ..PipeScale::default()
        })
        .with_tracer(self.tracer)
    }
}

/// Draw one elbow at `corner`, turning from `d_in` to `d_out`.
///
/// The two runs' inner corners are made coincident at a single point, which is
/// the construction [`PipeBendVisual`] documents: each run's centreline stops
/// half a thickness short of the geometric corner, and the inner corner sits
/// half a thickness inboard of that on the inside of the turn. Getting this
/// wrong is what makes elbows read as two rectangles butted together.
fn elbow(
    ui: &mut Ui,
    corner: Pos2,
    d_in: Vec2,
    d_out: Vec2,
    thickness: f32,
    upstream_k: f64,
    downstream_k: f64,
) {
    let half = 0.5 * thickness;
    // Outward normal of the incoming run, on the OUTSIDE of the turn. Screen y
    // grows downward, so a positive cross product is a clockwise turn.
    let cross = d_in.x * d_out.y - d_in.y * d_out.x;
    let sign = if cross >= 0.0 { -1.0 } else { 1.0 };
    let normal_in = Vec2::new(-d_in.y, d_in.x) * sign;

    let inlet_end = corner - d_in * half;
    let inner_corner = inlet_end - normal_in * half;

    ui.add(PipeBendVisual::new(
        inner_corner,
        d_in,
        d_out,
        thickness,
        k(upstream_k),
        k(downstream_k),
        k(DISPLAY_MIN_K),
        k(DISPLAY_MAX_K),
    ));
}

/// Draw a whole routed pipe path: one [`PipeVisual`] per straight leg and one
/// [`PipeBendVisual`] per interior corner.
///
/// `path` is the run's **centreline**, corner to corner. Every leg is trimmed
/// back by half a pipe thickness at each interior corner so the elbow sector
/// meets it flush. `trim_start`/`trim_end` do the same at the two ends, for a
/// path that is continued by another path through a shared elbow.
fn route(ui: &mut Ui, stream: &Stream, path: &[Pos2], trim_start: bool, trim_end: bool) {
    if path.len() < 2 {
        return;
    }
    let half = 0.5 * stream.thickness;
    let directions: Vec<Vec2> = path
        .windows(2)
        .map(|leg| (leg[1] - leg[0]).normalized())
        .collect();
    let last = directions.len() - 1;

    for (i, leg) in path.windows(2).enumerate() {
        let direction = directions[i];
        let mut from = leg[0];
        let mut to = leg[1];
        if i > 0 || trim_start {
            from += direction * half;
        }
        if i < last || trim_end {
            to -= direction * half;
        }
        ui.add(stream.run(from, to));
    }

    for i in 1..directions.len() {
        elbow(
            ui,
            path[i],
            directions[i - 1],
            directions[i],
            stream.thickness,
            stream.temperature_k,
            stream.temperature_k,
        );
    }
}

// ── Widget anchor points ────────────────────────────────────────────────────
//
// The vessel artworks letterbox to their own published aspect ratios inside
// the boxes above, so the nozzle positions are computed from the SAME
// letterbox functions the widgets use rather than eyeballed. If a widget's
// proportions change, these follow.

/// The reactor artwork's actual drawn rectangle inside [`REACTOR_BOX`].
fn reactor_rect() -> Rect {
    htr10_reactor_vessel::fit_native_aspect(Rect::from_center_size(REACTOR_CENTRE, REACTOR_BOX))
}

/// The steam-generator artwork's actual drawn rectangle inside [`SG_BOX`].
fn sg_rect() -> Rect {
    SteamGeneratorKind::HelicalCoil.fit_native_aspect(Rect::from_center_size(SG_CENTRE, SG_BOX))
}

/// The circulator artwork's actual drawn rectangle inside [`CIRCULATOR_BOX`].
fn circulator_rect() -> Rect {
    PumpKind::Centrifugal
        .fit_native_aspect(Rect::from_center_size(CIRCULATOR_CENTRE, CIRCULATOR_BOX))
}

/// The feed pump's actual drawn rectangle inside [`FEED_PUMP_BOX`].
fn feed_pump_rect() -> Rect {
    PumpKind::Centrifugal.fit_native_aspect(Rect::from_center_size(FEED_PUMP_CENTRE, FEED_PUMP_BOX))
}

/// Where a centrifugal pump's discharge nozzle leaves the top of its artwork.
///
/// The volute is wrapped so its throat lies at the cutwater's angular position
/// one full turn later, which puts the discharge on the upper left of the
/// casing rising vertically (see `PumpVisual`'s volute geometry). Reproducing
/// that fraction here keeps the connecting pipe on the nozzle rather than
/// beside it.
fn pump_discharge(rect: Rect) -> Pos2 {
    // Midway between the volute's throat radius (0.44 w) and its cutwater
    // radius (0.315 w), left of the casing centre.
    Pos2::new(rect.center().x - 0.3775 * rect.width(), rect.top())
}

/// The outboard tip of the reactor vessel's hot gas duct nozzle, in
/// schematic-local points.
///
/// Taken from the reactor artwork's own drawn rectangle with the fractions
/// `Htr10ReactorVesselVisual` paints the nozzle at: it spans `right - 0.02 w`
/// to `right + 0.16 w` horizontally and `0.62 h` to `0.70 h` vertically, so the
/// tip is at `+0.16 w` and the centreline elevation is `0.66 h`.
fn reactor_duct_nozzle() -> Pos2 {
    let reactor = reactor_rect();
    pos2(
        reactor.right() + 0.16 * reactor.width(),
        reactor.top() + 0.66 * reactor.height(),
    )
}

/// The steam generator's four nozzle anchors, in schematic-local points.
///
/// Each is the free **tip** of a nozzle stub the helical-coil artwork paints,
/// derived from [`sg_rect`] with the same fractions
/// `SteamGeneratorVisual::draw_helical_coil` uses. Deriving them means moving,
/// resizing or re-proportioning the steam generator carries every connected
/// pipe end with it, instead of leaving the pipes behind on the old geometry.
#[derive(Debug, Clone, Copy)]
struct SgNozzles {
    /// Cold-helium outlet to the circulator: **upper left**. Artwork stub
    /// `-0.68 w .. -0.10 w` over `0.045 h .. 0.075 h`, so tip `-0.68 w`,
    /// centreline `0.06 h`.
    cold_gas_out: Pos2,
    /// Superheated-steam outlet to the turbine: **upper right**. Artwork stub
    /// `+0.30 w .. +0.68 w` over `0.10 h .. 0.125 h`, so tip `+0.68 w`,
    /// centreline `0.1125 h`.
    steam_out: Pos2,
    /// Feedwater inlet: **lower left**. Artwork stub `-0.68 w .. -0.30 w` over
    /// `0.885 h .. 0.91 h`, so tip `-0.68 w`, centreline `0.8975 h`.
    feed_in: Pos2,
    /// Hot-gas duct inlet, feeding the bottom of the centre tube: **lower
    /// right**. Artwork stub `+0.02 w .. +0.68 w` over `0.925 h .. 0.955 h`,
    /// so tip `+0.68 w`, centreline `0.94 h`.
    ///
    /// This one faces *away* from the reactor, which is why the hot gas duct
    /// has to come round the vessel -- see [`hot_gas_duct_path`].
    hot_gas_in: Pos2,
}

/// The steam generator's nozzle anchors for the box it is drawn in here.
fn sg_nozzles() -> SgNozzles {
    let sg = sg_rect();
    let x = |f: f32| sg.center().x + f * sg.width();
    let y = |f: f32| sg.top() + f * sg.height();
    SgNozzles {
        cold_gas_out: pos2(x(-0.68), y(0.06)),
        steam_out: pos2(x(0.68), y(0.1125)),
        feed_in: pos2(x(-0.68), y(0.8975)),
        hot_gas_in: pos2(x(0.68), y(0.94)),
    }
}

/// Centreline of the hot gas duct, from the reactor vessel's duct nozzle to the
/// steam generator's, corner to corner in schematic-local points.
///
/// **Why it is not a straight line.** The helical-coil artwork puts its hot-gas
/// nozzle on the **bottom right** of the steam-generator vessel, because that
/// is where the duct meets the bottom of the centre tube the hot helium rises
/// through (see [`SgNozzles::hot_gas_in`]). The reactor is on the *left*, so
/// the stub points away from it and the run has to come round. It therefore:
///
/// 1. leaves the reactor horizontally at the hot-gas plenum elevation -- the
///    cross-vessel run proper, and the only part the "hot gas duct" label sits
///    on;
/// 2. turns down clear of the cold-helium return riser
///    ([`COLD_RETURN_RISER_X`]), so the two verticals do not read as one broken
///    pipe;
/// 3. crosses beneath the feedwater header ([`FEEDWATER_HEADER_Y`]);
/// 4. rises on the far side of the steam-generator vessel; and
/// 5. runs back in to the nozzle from the right, the side the stub faces,
///    overlapping the tip by [`NOZZLE_SEAM_OVERLAP`].
///
/// **Two properties worth keeping.** The run stays *outside*
/// [`SG_VESSEL_BOUNDARY`] until the last leg, so it crosses the
/// steam-generator pressure-vessel boundary exactly once, at the nozzle. And
/// it crosses exactly one other run, the feedwater header, where the riser
/// climbs past it -- unavoidable, because that header sweeps the whole width of
/// the plant at an elevation the duct has to climb through.
fn hot_gas_duct_path() -> [Pos2; 6] {
    let reactor_nozzle = reactor_duct_nozzle();
    let sg_hot_in = sg_nozzles().hot_gas_in;

    let turn_x = COLD_RETURN_RISER_X - 30.0;
    let under_y = FEEDWATER_HEADER_Y + 20.0;
    let riser_x = sg_hot_in.x + 32.0;

    [
        pos2(reactor_nozzle.x - 10.0, reactor_nozzle.y),
        pos2(turn_x, reactor_nozzle.y),
        pos2(turn_x, under_y),
        pos2(riser_x, under_y),
        pos2(riser_x, sg_hot_in.y),
        pos2(sg_hot_in.x - NOZZLE_SEAM_OVERLAP, sg_hot_in.y),
    ]
}

// ── The panel ───────────────────────────────────────────────────────────────

/// Draw the whole schematic from `snapshot` into `ui`, animating the connector
/// runs with the app-owned `tracers`.
///
/// Reserves a fixed canvas so the absolute widget positions have room; the
/// engine widgets paint at their own `screen_position`, independent of the egui
/// layout cursor, and the panel is inside a scroll area so a small window
/// scrolls rather than overlapping the artwork.
pub fn draw_schematic(
    ui: &mut Ui,
    snapshot: &HtgrSnapshot,
    tracers: &SchematicTracers,
    display_unit: LegendUnit,
) {
    let (canvas_rect, _response) = ui.allocate_exact_size(CANVAS, egui::Sense::hover());
    let origin = canvas_rect.min.to_vec2();

    // Shift a schematic-local point into canvas coordinates.
    let at = |p: Pos2| -> Pos2 { p + origin };
    let at_rect = |r: Rect| -> Rect { r.translate(origin) };

    // ── Real steam/water states, flashed from the snapshot ──────────────
    //
    // The snapshot is scalar-only, so the states the widgets colour by are
    // rebuilt here from the pressures and enthalpies the plant model actually
    // published. These are genuine IAPWS-IF97 flashes of real model state, not
    // stand-ins.
    let reference_volume = Volume::new::<cubic_meter>(1.0);
    let steam_pressure = Pressure::new::<megapascal>(snapshot.steam_pressure_mpa);
    let condenser_pressure = Pressure::new::<kilopascal>(snapshot.condenser_pressure_kpa);

    // (The live steam state used to be flashed here to colour the turbine
    // casing, when the turbine was built through `TurbineVisual::new_thermo`.
    // It is now generator-backed so the rotor can turn, and that variant
    // carries no steam path -- see section 7. The steam temperature reaches the
    // screen through the `T_steam` readout instead.)
    let feedwater: HemSteamCv = HemSteamCv::new_from_ph(
        steam_pressure,
        AvailableEnergy::new::<joule_per_kilogram>(snapshot.feedwater_enthalpy_j_per_kg),
        reference_volume,
    );
    // The hotwell condensate is the saturated liquid at condenser pressure, so
    // its temperature is also the turbine exhaust temperature: the exhaust is
    // two-phase at that same pressure.
    let condensate: HemSteamCv = HemSteamCv::new_from_ph(
        condenser_pressure,
        AvailableEnergy::new::<joule_per_kilogram>(snapshot.condensate_enthalpy_j_per_kg),
        reference_volume,
    );
    let feedwater_temp = feedwater.get_temperature();
    let condensate_temp = condensate.get_temperature();

    // ── Streams ─────────────────────────────────────────────────────────
    let hot_helium = Stream {
        temperature_k: snapshot.core_outlet_temp_k,
        mass_flow_kg_per_s: snapshot.helium_mass_flow_kg_per_s,
        residence_time_s: snapshot.helium_residence_time_s,
        thickness: HELIUM_PIPE_THICKNESS,
        tracer: tracers.primary,
    };
    // Leaving the steam generator: this is what the circulator lifts.
    let cold_helium = Stream {
        temperature_k: snapshot.ihx_outlet_temp_k,
        ..hot_helium
    };
    // Arriving at the core: the model's core inlet lags the SG outlet by the
    // loop transport time, so the last leg of the return really is a different
    // temperature during a transient.
    let core_inlet_helium = Stream {
        temperature_k: snapshot.core_inlet_temp_k,
        ..hot_helium
    };

    let main_steam = Stream {
        temperature_k: snapshot.sg_steam_outlet_temp_k,
        mass_flow_kg_per_s: snapshot.secondary_mass_flow_kg_per_s,
        residence_time_s: snapshot.secondary_residence_time_s,
        thickness: STEAM_PIPE_THICKNESS,
        tracer: tracers.secondary,
    };
    let exhaust = Stream {
        temperature_k: condensate_temp.get::<kelvin>(),
        ..main_steam
    };
    let feed = Stream {
        temperature_k: feedwater_temp.get::<kelvin>(),
        ..main_steam
    };

    let reactor = reactor_rect();
    let circulator = circulator_rect();
    let feed_pump = feed_pump_rect();

    // Nozzle anchors, taken from each artwork's own drawn rectangle (see
    // `reactor_duct_nozzle` and `sg_nozzles`), so a pipe end cannot drift off
    // its nozzle when a component is moved or re-proportioned.
    let reactor_duct = reactor_duct_nozzle();
    let duct_y = reactor_duct.y;
    let duct_tip_x = reactor_duct.x;
    let cold_return_y = duct_y - 24.0;
    let nozzles = sg_nozzles();
    let sg_cold_out = nozzles.cold_gas_out;
    let sg_steam_out = nozzles.steam_out;
    let sg_feed_in = nozzles.feed_in;

    // ── 1. Steam-generator pressure vessel boundary ─────────────────────
    //
    // Drawn first so every widget and pipe reads on top of it. This dashed
    // outline is an ANNOTATION, not component art: it marks the second
    // pressure vessel -- the one that houses the SG, the IHX and the
    // circulator -- because the side-by-side two-vessel arrangement is the
    // defining feature of this plant and nothing else on screen states it.
    let boundary = at_rect(SG_VESSEL_BOUNDARY);
    ui.painter().extend(egui::Shape::dashed_line(
        &[
            boundary.left_top(),
            boundary.right_top(),
            boundary.right_bottom(),
            boundary.left_bottom(),
            boundary.left_top(),
        ],
        Stroke::new(1.0_f32, ANNOTATION),
        6.0,
        5.0,
    ));

    // ── 2. Primary helium circuit ───────────────────────────────────────
    //
    // Hot gas duct: the cross-vessel run at the reactor's own hot-gas plenum
    // elevation, then round to the steam generator's hot-gas nozzle on the
    // BOTTOM RIGHT of its vessel, which is where the duct meets the bottom of
    // the centre tube. See `hot_gas_duct_path` for why it is not a straight
    // line and what the routing is required to clear.
    let duct_path: Vec<Pos2> = hot_gas_duct_path().into_iter().map(at).collect();
    route(ui, &hot_helium, &duct_path, false, false);

    // Cold helium leaving the SG through the connecting tube to the blower
    // above it.
    route(
        ui,
        &cold_helium,
        &[
            at(pos2(sg_cold_out.x + NOZZLE_SEAM_OVERLAP, sg_cold_out.y)),
            at(pos2(CIRCULATOR_CENTRE.x, sg_cold_out.y)),
            at(pos2(CIRCULATOR_CENTRE.x, circulator.bottom() - 2.0)),
        ],
        false,
        false,
    );

    // Circulator discharge, over the top and back down to the reactor. The
    // last leg runs in alongside the hot gas duct, which is where the cold
    // return annulus actually is.
    let discharge = pump_discharge(circulator);
    let return_corner = pos2(COLD_RETURN_RISER_X, cold_return_y);
    route(
        ui,
        &cold_helium,
        &[
            at(discharge),
            at(pos2(discharge.x, 30.0)),
            at(pos2(return_corner.x, 30.0)),
            at(return_corner),
        ],
        false,
        true,
    );
    route(
        ui,
        &core_inlet_helium,
        &[
            at(return_corner),
            at(pos2(duct_tip_x - 20.0, cold_return_y)),
        ],
        true,
        false,
    );
    elbow(
        ui,
        at(return_corner),
        Vec2::new(0.0, 1.0),
        Vec2::new(-1.0, 0.0),
        HELIUM_PIPE_THICKNESS,
        snapshot.ihx_outlet_temp_k,
        snapshot.core_inlet_temp_k,
    );

    // ── 3. Secondary steam circuit ──────────────────────────────────────
    let turbine_rect = Rect::from_center_size(TURBINE_CENTRE, TURBINE_BOX);
    let condenser_rect = Rect::from_center_size(CONDENSER_CENTRE, CONDENSER_BOX);

    route(
        ui,
        &main_steam,
        &[
            at(pos2(sg_steam_out.x - 5.0, sg_steam_out.y)),
            at(pos2(turbine_rect.left(), sg_steam_out.y)),
        ],
        false,
        false,
    );
    route(
        ui,
        &exhaust,
        &[
            at(pos2(turbine_rect.right(), TURBINE_CENTRE.y)),
            at(pos2(CONDENSER_CENTRE.x, TURBINE_CENTRE.y)),
            at(pos2(CONDENSER_CENTRE.x, condenser_rect.top() + 3.0)),
        ],
        false,
        false,
    );
    route(
        ui,
        &exhaust,
        &[
            at(pos2(CONDENSER_CENTRE.x, condenser_rect.bottom() - 3.0)),
            at(pos2(CONDENSER_CENTRE.x, FEED_PUMP_CENTRE.y)),
            at(pos2(feed_pump.right() + 3.0, FEED_PUMP_CENTRE.y)),
        ],
        false,
        false,
    );

    let feed_discharge = pump_discharge(feed_pump);
    route(
        ui,
        &feed,
        &[
            at(feed_discharge),
            at(pos2(feed_discharge.x, FEEDWATER_HEADER_Y)),
            at(pos2(FEEDWATER_HEADER_RISER_X, FEEDWATER_HEADER_Y)),
            at(pos2(FEEDWATER_HEADER_RISER_X, sg_feed_in.y)),
            at(pos2(sg_feed_in.x + NOZZLE_SEAM_OVERLAP, sg_feed_in.y)),
        ],
        false,
        false,
    );

    // ── 4. The reactor ──────────────────────────────────────────────────
    //
    // Pebble, inlet and outlet temperatures are the plant model's own. The
    // reflector is tinted at the core inlet temperature -- the model carries no
    // graphite temperature, and the reflector's channels are the ones the cold
    // inlet helium rises through. See the module docs.
    let mut vessel = Htr10ReactorVesselVisual::new(
        REACTOR_BOX,
        k(DISPLAY_MIN_K),
        k(DISPLAY_MAX_K),
        k(snapshot.fuel_temperature_k),
        k(snapshot.core_inlet_temp_k),
        k(snapshot.core_outlet_temp_k),
        k(snapshot.core_inlet_temp_k),
    );
    // ── Control rods ────────────────────────────────────────────────────
    //
    // The rod bank the side panel's slider drives. HTR-10's rods sit in ten
    // borings in the graphite SIDE REFLECTOR, not in the pebble bed, and the
    // vessel widget draws them there.
    //
    // The slider writes a setpoint, but a real rod drive takes time to get
    // there, so the DRAWN depth is slewed toward the setpoint at a bounded
    // speed. `slewed_control_rod_insertion` owns that state outside the widget
    // -- widgets here are rebuilt every repaint, so a rod that stored its own
    // position would reset to zero each frame and never move.
    //
    // The drive speed is ILLUSTRATIVE: no published HTR-10 rod drive speed was
    // found in this project's scoping notes or literature archive, so the
    // engine uses a full stroke in 20 s purely so the travel is watchable. See
    // `animation::control_rod_drive::htr10_illustrative_rod_drive_speed`.
    let commanded_rod_insertion = snapshot.control_rod_insertion_fraction;
    let drawn_rod_insertion = slewed_control_rod_insertion(
        ui.ctx(),
        ui.id().with("htr10_control_rod_bank"),
        commanded_rod_insertion,
        ControlRodDrive::htr10(commanded_rod_insertion),
    );
    vessel.set_control_rod_frac(drawn_rod_insertion);
    ui.put(
        at_rect(Rect::from_center_size(REACTOR_CENTRE, REACTOR_BOX)),
        vessel,
    );

    // ── 5. The steam generator ──────────────────────────────────────────
    //
    // Once-through helical coil, the architecture this plant actually uses:
    // hot helium down the shell over the coil, water up the coil, no water
    // level. The level fraction is ignored by this kind and is passed as zero
    // rather than invented.
    ui.add(SteamGeneratorVisual::from_scalars(
        SteamGeneratorKind::HelicalCoil,
        at(SG_CENTRE),
        SG_BOX,
        k(DISPLAY_MIN_K),
        k(DISPLAY_MAX_K),
        SteamGeneratorScalars {
            primary_inlet_temp: k(snapshot.core_outlet_temp_k),
            primary_outlet_temp: k(snapshot.ihx_outlet_temp_k),
            feedwater_temp,
            steam_temp: k(snapshot.sg_steam_outlet_temp_k),
            water_level_frac: 0.0,
        },
    ));

    // ── 6. The helium circulator, in the SG vessel above the SG ─────────
    //
    // A single-stage centrifugal machine. Shaft speed is `ZERO` because the
    // plant model has none -- a stationary rotor is the honest rendering of
    // "not known" (see the module docs). The passages are still coloured by
    // the real cold-helium temperature it handles.
    ui.add(PumpVisual::from_scalars(
        PumpKind::Centrifugal,
        at(CIRCULATOR_CENTRE),
        CIRCULATOR_BOX,
        AngularVelocity::ZERO,
        Time::new::<second>(snapshot.sim_time_s),
        Some(k(snapshot.ihx_outlet_temp_k)),
        k(DISPLAY_MIN_K),
        k(DISPLAY_MAX_K),
    ));

    // ── 7. Turbine ──────────────────────────────────────────────────────
    //
    // Single flow: a once-through generator delivers superheated steam, which
    // is dense enough at admission for one flow path to carry the volume.
    //
    // GENERATOR-BACKED, so the rotor turns. The plant runs a real torque
    // balance on the turbine-generator shaft (`physics::turbine_generator`),
    // driven by the same enthalpy-drop power the secondary loop computes, and
    // publishes the resulting angular velocity on the snapshot. The generator
    // model is rebuilt here from that scalar -- the snapshot is scalar-only by
    // design, and `TurbineVisual` reads nothing from the model but `omega`.
    // The rotor phase is `theta = omega * t`, so this is the shaft's own speed
    // and not an animation constant.
    //
    // TRADE-OFF, stated because it is a real loss of information: the widget's
    // `TurbineVisualState` enum has no variant carrying BOTH a shaft speed and
    // a steam state, and the generator variant deliberately reports no casing
    // temperature (it is an electromechanical model with no steam path). So
    // the casing here draws neutral grey instead of at the live steam
    // temperature, which is what the thermo-backed variant used to give. The
    // steam temperature is still on screen as the `T_steam` readout, and the
    // shaft speed is added beside it. Restoring the colour needs a new widget
    // variant in the engine crate, which is the maintainer's call.
    let generator = turbine_generator::generator_at_speed(
        AngularVelocity::new::<radian_per_second>(snapshot.shaft_speed_rad_per_s),
        Power::new::<megawatt>(snapshot.generator_rating_mw.max(f64::MIN_POSITIVE)),
    );
    ui.add(
        TurbineVisual::new_generator(
            generator,
            at(TURBINE_CENTRE),
            TURBINE_BOX,
            k(DISPLAY_MIN_K),
            k(DISPLAY_MAX_K),
        )
        .with_flow_path(TurbineFlowPath::SingleFlow)
        .at_time(Time::new::<second>(snapshot.sim_time_s)),
    );

    // ── 8. Condenser ────────────────────────────────────────────────────
    let condenser = Condenser::new(condenser_pressure, snapshot.steam_quality_after_turbine);
    ui.add(CondenserVisual::new(
        condenser,
        at(CONDENSER_CENTRE),
        CONDENSER_BOX,
    ));

    // ── 9. Feedwater pump ───────────────────────────────────────────────
    ui.add(PumpVisual::from_scalars(
        PumpKind::Centrifugal,
        at(FEED_PUMP_CENTRE),
        FEED_PUMP_BOX,
        AngularVelocity::ZERO,
        Time::new::<second>(snapshot.sim_time_s),
        Some(feedwater_temp),
        k(DISPLAY_MIN_K),
        k(DISPLAY_MAX_K),
    ));

    // ── 10. Annotations ─────────────────────────────────────────────────
    let tag = |ui: &Ui, p: Pos2, align: Align2, text: &str, size: f32| {
        ui.painter()
            .text(at(p), align, text, FontId::proportional(size), ANNOTATION);
    };
    tag(
        ui,
        pos2(REACTOR_CENTRE.x, reactor.top() - 14.0),
        Align2::CENTER_BOTTOM,
        "reactor pressure vessel  4.2 m x 11.1 m",
        10.0,
    );
    // Stacked short lines in the empty column inside the boundary, left of the
    // steam generator: one long line across the top would be crossed by the
    // cold-return riser, which reads as a mistake rather than as a label.
    for (i, line) in [
        "steam-generator",
        "pressure vessel",
        "2.5 m x 11.3 m",
        "SG + IHX + circulator",
    ]
    .into_iter()
    .enumerate()
    {
        tag(
            ui,
            pos2(SG_VESSEL_BOUNDARY.min.x + 6.0, 296.0 + 12.0 * i as f32),
            Align2::LEFT_TOP,
            line,
            9.5,
        );
    }
    tag(
        ui,
        pos2(circulator.right() + 8.0, CIRCULATOR_CENTRE.y - 8.0),
        Align2::LEFT_CENTER,
        "helium circulator",
        10.0,
    );
    // On the horizontal cross-vessel leg, which is the part of the run the
    // label actually names -- the rest of the path is the detour round to the
    // steam generator's bottom-right nozzle.
    let duct_path_local = hot_gas_duct_path();
    tag(
        ui,
        pos2(
            0.5 * (duct_path_local[0].x + duct_path_local[1].x),
            duct_y + 12.0,
        ),
        Align2::CENTER_TOP,
        "hot gas duct (cross-vessel)",
        10.0,
    );
    tag(
        ui,
        pos2(300.0, cold_return_y - 11.0),
        Align2::CENTER_BOTTOM,
        "cold helium return",
        10.0,
    );
    tag(
        ui,
        pos2(
            0.5 * (sg_steam_out.x + turbine_rect.left()),
            sg_steam_out.y - 9.0,
        ),
        Align2::CENTER_BOTTOM,
        "main steam",
        10.0,
    );
    tag(
        ui,
        pos2(
            CONDENSER_CENTRE.x + 8.0,
            0.5 * (condenser_rect.bottom() + FEED_PUMP_CENTRE.y),
        ),
        Align2::LEFT_CENTER,
        "condensate",
        10.0,
    );
    tag(
        ui,
        pos2(620.0, 580.0),
        Align2::CENTER_BOTTOM,
        "feedwater",
        10.0,
    );
    tag(
        ui,
        pos2(FEED_PUMP_CENTRE.x, feed_pump.bottom() + 4.0),
        Align2::CENTER_TOP,
        "feedwater pump",
        10.0,
    );

    // ── 11. Colour legend ───────────────────────────────────────────────
    //
    // One legend, because every widget above shares one display range. It is
    // drawn by the same colour function the widgets use, so it cannot drift
    // from what it explains.
    ui.put(
        at_rect(Rect::from_min_size(
            pos2(1055.0, 80.0),
            Vec2::new(110.0, 290.0),
        )),
        TemperatureLegend::new(k(DISPLAY_MIN_K), k(DISPLAY_MAX_K))
            .with_caption("colour = temperature")
            // Follows the operator's degC/K toggle, so the legend's tick labels
            // and the numeric readouts below it cannot end up in different
            // units. Display only -- see `crate::app::panels::temperature_display`.
            .with_unit(display_unit)
            .with_bar_size(Vec2::new(26.0, 240.0)),
    );

    // ── 12. Instrumentation readouts ────────────────────────────────────
    //
    // Every temperature goes through the operator's display-unit formatter, so
    // the readouts, the colour legend above and the diagnostics panel always
    // agree. Zero decimals here: these are small tags on a schematic, and the
    // Diagnostics panel is where a tenth of a kelvin belongs.
    let temp = |value_k: f64| crate::app::panels::temperature_display(display_unit, k(value_k), 0);
    let readouts: [(Pos2, &str, String); 11] = [
        (
            pos2(58.0, 556.0),
            "Power",
            format!("{:.1} MWth", snapshot.reactor_power_mw),
        ),
        (
            pos2(58.0, 574.0),
            "T_fuel",
            temp(snapshot.fuel_temperature_k),
        ),
        (
            pos2(58.0, 592.0),
            "T_He,out",
            temp(snapshot.core_outlet_temp_k),
        ),
        (
            pos2(58.0, 610.0),
            "T_He,in",
            temp(snapshot.core_inlet_temp_k),
        ),
        (
            pos2(640.0, 300.0),
            "Q_SG",
            format!("{:.1} MW", snapshot.ihx_duty_mw),
        ),
        (
            pos2(640.0, 318.0),
            "T_steam",
            temp(snapshot.sg_steam_outlet_temp_k),
        ),
        (
            pos2(640.0, 336.0),
            "T_feed",
            temp(feedwater_temp.get::<kelvin>()),
        ),
        (
            pos2(768.0, 262.0),
            "P_turb",
            format!("{:.1} MW", snapshot.turbine_power_mw),
        ),
        (
            pos2(768.0, 280.0),
            "x_exhaust",
            format!("{:.3}", snapshot.steam_quality_after_turbine),
        ),
        // The shaft speed the rotor above is actually drawn turning at, and
        // the electrical power that comes out of the same torque balance.
        // Both are computed, not display constants -- see
        // `crate::physics::turbine_generator`.
        (
            pos2(768.0, 298.0),
            "n_shaft",
            format!("{:.0} rpm", snapshot.shaft_speed_rpm),
        ),
        (
            pos2(768.0, 316.0),
            "P_elec",
            format!("{:.1} MWe", snapshot.generator_electrical_power_mw),
        ),
    ];
    for (p, label, value) in readouts {
        ui.add(InstrumentationVisual::new(at(p), label, value));
    }

    // ── 12b. Sim-clock pacing ───────────────────────────────────────────
    //
    // Simulated seconds per wall-clock second, published by the physics
    // thread's pacer. On screen because a simulator that has quietly dropped
    // below real time while still counting "seconds" is misleading -- the
    // operator has no other way to know the plant clock and their wristwatch
    // have parted company. Reads "--" until the first tick.
    //
    // This is the SIM-CLOCK rate, not a frame rate. A smooth window with a
    // 0.4x plant clock is a physics cost problem; a stuttering window at 1.00x
    // is a rendering problem. They are different faults and this readout only
    // speaks to the first.
    ui.add(InstrumentationVisual::new(
        at(pos2(58.0, 628.0)),
        "real-time",
        match snapshot.real_time_ratio {
            Some(ratio) => format!("{ratio:.2}x"),
            None => "--".to_string(),
        },
    ));
    if snapshot.behind_real_time {
        ui.painter().text(
            at(pos2(58.0, 648.0)),
            Align2::LEFT_TOP,
            format!(
                "SLOWER THAN REAL TIME -- plant clock {:.1} s behind",
                snapshot.real_time_deficit_s
            ),
            FontId::proportional(11.0),
            Color32::from_rgb(226, 138, 70),
        );
    }

    // ── 13. Standing caveat, on screen ──────────────────────────────────
    ui.painter().text(
        at(pos2(8.0, 714.0)),
        Align2::LEFT_TOP,
        "Arrangement follows the published HTR-10 description (IAEA-TECDOC-1382). The plant data \
         shown are illustrative demonstration values -- not HTR-10's, and not any specific \
         licensed design. Offline demonstration only; not for operational, licensing or safety use.",
        FontId::proportional(10.0),
        ANNOTATION,
    );
}

// ── Geometry checks ─────────────────────────────────────────────────────────
//
// These are pure-geometry tests over the anchor and routing functions above.
// They need no display, no egui context and no physics: the whole point is
// that a pipe end landing on the wrong place is a NUMERIC fact about two
// positions, checkable headlessly, rather than something only an eyeball can
// catch. They cannot check that the result *looks* right -- line weights,
// overlaps with text, whether the detour reads clearly -- which still needs a
// human at a screen.

#[cfg(test)]
mod tests {
    use super::*;

    /// Half the drawn width of a helium run: the clearance every leg of the hot
    /// gas duct has to keep from artwork it is not connecting to.
    const HALF_DUCT: f32 = 0.5 * HELIUM_PIPE_THICKNESS;

    /// The drawn rectangle of one of the steam generator's nozzle stubs, from
    /// the fractions `SteamGeneratorVisual::draw_helical_coil` paints it at.
    fn sg_stub(x_from: f32, x_to: f32, y_from: f32, y_to: f32) -> Rect {
        let sg = sg_rect();
        Rect::from_min_max(
            pos2(
                sg.center().x + x_from * sg.width(),
                sg.top() + y_from * sg.height(),
            ),
            pos2(
                sg.center().x + x_to * sg.width(),
                sg.top() + y_to * sg.height(),
            ),
        )
    }

    /// The four stubs, paired with the anchor that is supposed to sit on each
    /// one's outboard tip, and the sign of "inboard" along the stub.
    fn stubs_and_anchors() -> [(&'static str, Rect, Pos2, f32); 4] {
        let n = sg_nozzles();
        [
            (
                "cold gas out",
                sg_stub(-0.68, -0.10, 0.045, 0.075),
                n.cold_gas_out,
                1.0,
            ),
            (
                "steam out",
                sg_stub(0.30, 0.68, 0.10, 0.125),
                n.steam_out,
                -1.0,
            ),
            (
                "feedwater in",
                sg_stub(-0.68, -0.30, 0.885, 0.91),
                n.feed_in,
                1.0,
            ),
            (
                "hot gas in",
                sg_stub(0.02, 0.68, 0.925, 0.955),
                n.hot_gas_in,
                -1.0,
            ),
        ]
    }

    /// Every steam-generator anchor must be the outboard tip of the stub the
    /// artwork paints, on that stub's centreline.
    ///
    /// **Methodology.** Rebuild each nozzle stub's rectangle from [`sg_rect`]
    /// with the fractions `SteamGeneratorVisual::draw_helical_coil` uses, then
    /// require the corresponding [`SgNozzles`] anchor to sit on the outboard
    /// end of that rectangle, at its mid-height, to within 0.01 pt. This is the
    /// property that makes the anchors *derived* rather than eyeballed: it
    /// fails the moment the artwork's proportions and the routing code drift
    /// apart.
    ///
    /// **Results (2026-08-13).** Steam-generator artwork rectangle
    /// `x 514.955..605.045`, `y 151.396..558.604` pt (90.090 x 407.207 pt,
    /// letterboxed to the published 2.5 m x 11.3 m aspect inside a 120 x 407.207
    /// pt box). All four anchors matched their stub tip and centreline to
    /// 0.000 pt: cold gas out (514.955, 175.828), steam out (621.261, 197.207),
    /// feedwater in (514.955, 516.865), hot gas in (621.261, 534.171).
    /// Interpretation: the four pipe ends are pinned to the artwork, so moving
    /// or re-proportioning the steam generator moves them with it.
    #[test]
    fn steam_generator_anchors_sit_on_the_artwork_nozzle_tips() {
        for (name, stub, anchor, inboard) in stubs_and_anchors() {
            let tip_x = if inboard > 0.0 {
                stub.left()
            } else {
                stub.right()
            };
            assert!(
                (anchor.x - tip_x).abs() < 0.01,
                "{name}: anchor x {} is not the stub tip {tip_x}",
                anchor.x
            );
            assert!(
                (anchor.y - stub.center().y).abs() < 0.01,
                "{name}: anchor y {} is not the stub centreline {}",
                anchor.y,
                stub.center().y
            );
        }
    }

    /// Pushing a run inboard by [`NOZZLE_SEAM_OVERLAP`] must land it on metal.
    ///
    /// **Methodology.** Every connector here stops [`NOZZLE_SEAM_OVERLAP`] past
    /// its nozzle tip so no hairline gap shows at the joint. That is only
    /// correct if the overlap point is still *inside* the drawn stub — if the
    /// stub were shorter than the overlap, the run would end in mid-air with
    /// the seam hidden behind nothing. Require the overlapped point to be
    /// strictly inside each stub rectangle.
    ///
    /// **Results (2026-08-13).** All four passed. Shortest stub is the steam
    /// outlet at 34.234 pt of drawn length, against a 3.0 pt overlap — a
    /// factor of 11 of margin, so the convention is in no danger from a modest
    /// re-proportioning. Interpretation: the seam overlap is safe for all four
    /// nozzles, hot gas duct included.
    #[test]
    fn the_nozzle_seam_overlap_stays_on_the_stub() {
        for (name, stub, anchor, inboard) in stubs_and_anchors() {
            let seam = pos2(anchor.x + inboard * NOZZLE_SEAM_OVERLAP, anchor.y);
            assert!(
                stub.contains(seam),
                "{name}: seam point {seam:?} fell outside the stub {stub:?}"
            );
        }
    }

    /// The hot gas duct must terminate on the steam generator's hot-gas nozzle,
    /// approaching from the right, which is the side the stub faces.
    ///
    /// **Methodology.** This is the regression test for the reported defect
    /// (kopi-beans `op-88kl`): the duct used to end at `sg_rect().left() + 1.0`
    /// at the *reactor's* plenum elevation, on the steam generator's blank left
    /// flank, while the artwork's hot-gas nozzle is the stub on its bottom
    /// right. Require the last point of [`hot_gas_duct_path`] to be
    /// [`NOZZLE_SEAM_OVERLAP`] inboard of [`SgNozzles::hot_gas_in`] along the
    /// stub, exactly on its centreline, and the final leg to be horizontal and
    /// arriving from the right.
    ///
    /// **Results (2026-08-13).** Nozzle tip (621.261, 534.171) pt; duct end
    /// (618.261, 534.171) pt — 3.000 pt inboard in x, 0.000 pt off the
    /// centreline. Final leg runs from x 653.261 to x 618.261 at a constant
    /// y 534.171. The old endpoint (515.955, 404.000) missed the nozzle by
    /// 105.31 pt horizontally and 130.17 pt vertically, which is what the
    /// maintainer saw. Interpretation: the duct now lands on the port.
    #[test]
    fn the_hot_gas_duct_lands_on_the_steam_generator_nozzle() {
        let path = hot_gas_duct_path();
        let nozzle = sg_nozzles().hot_gas_in;
        let end = path[5];

        // Printed so the figures recorded above can be re-read rather than
        // taken on trust, the way the plant tests print theirs.
        println!("SG artwork rect: {:?}", sg_rect());
        println!("reactor artwork rect: {:?}", reactor_rect());
        println!("hot gas nozzle tip: {nozzle:?}");
        println!("hot gas duct path: {path:?}");

        assert!(
            (end.x - (nozzle.x - NOZZLE_SEAM_OVERLAP)).abs() < 0.01,
            "duct ends at x {}, expected {}",
            end.x,
            nozzle.x - NOZZLE_SEAM_OVERLAP
        );
        assert!(
            (end.y - nozzle.y).abs() < 0.01,
            "duct ends at y {}, off the nozzle centreline {}",
            end.y,
            nozzle.y
        );
        // Arrives horizontally, from the right.
        assert!((path[4].y - end.y).abs() < 0.01);
        assert!(path[4].x > end.x);
    }

    /// The hot gas duct must start on the reactor's duct nozzle, and every leg
    /// must be axis-aligned.
    ///
    /// **Methodology.** [`elbow`] builds a `PipeBendVisual` assuming a
    /// right-angle turn, so a diagonal leg would silently draw a broken corner.
    /// Require consecutive path points to share an x or a y, and the first
    /// point to sit on the reactor artwork's own duct nozzle (10 pt inboard of
    /// its tip, the overlap that run has always used).
    ///
    /// **Results (2026-08-13).** Reactor artwork rectangle `x 74.324..225.676`,
    /// `y 140.000..540.000` pt; duct nozzle tip (249.892, 404.000) pt; path
    /// start (239.892, 404.000) pt. All five legs were axis-aligned: three
    /// horizontal (y 404.000, 605.000, 534.171) and two vertical (x 340.000,
    /// 653.261). Interpretation: four clean right-angle elbows, no diagonal.
    #[test]
    fn the_hot_gas_duct_is_a_rectilinear_run_off_the_reactor_nozzle() {
        let path = hot_gas_duct_path();
        let reactor_nozzle = reactor_duct_nozzle();

        assert!((path[0].y - reactor_nozzle.y).abs() < 0.01);
        assert!((path[0].x - (reactor_nozzle.x - 10.0)).abs() < 0.01);

        for leg in path.windows(2) {
            let axis_aligned =
                (leg[0].x - leg[1].x).abs() < 0.01 || (leg[0].y - leg[1].y).abs() < 0.01;
            assert!(axis_aligned, "leg {:?} -> {:?} is diagonal", leg[0], leg[1]);
        }
    }

    /// No leg of the hot gas duct may run over the steam-generator artwork, the
    /// cold-helium return riser, or the feedwater riser.
    ///
    /// **Methodology.** Inflate each leg by half the drawn pipe width
    /// ([`HALF_DUCT`]) and require it not to intersect the steam generator's
    /// drawn rectangle — the nozzle stub sticks out beyond that rectangle, so
    /// connecting to it does not require overlapping the shell. Then require
    /// the duct's vertical leg to stand at least one full pipe width clear of
    /// the two risers it passes, and its horizontal under-run to pass below the
    /// foot of the feedwater riser.
    ///
    /// **Results (2026-08-13).** Closest approach to the steam-generator
    /// rectangle (`x 514.955..605.045`, `y 151.396..558.604`) was the under-run
    /// at y 605.000, clearing the bottom head by 46.40 pt, and the riser at
    /// x 653.261, clearing the right shell by 48.22 pt; no leg intersected.
    /// The duct's vertical leg stands at x 340.000: 30.00 pt from the
    /// cold-return riser at x 370.000 and 50.00 pt from the feedwater riser at
    /// x 390.000, both greater than the 13.0 pt pipe width. The under-run at
    /// y 605.000 passes 20.00 pt below the feedwater header at y 585.000.
    ///
    /// **Known and accepted:** the duct's riser at x 653.261 *does* cross the
    /// feedwater header at y 585.000, which spans the width of the plant. That
    /// crossing is unavoidable — the header sits between the under-run and the
    /// nozzle elevation — and is the only run-over-run crossing in the
    /// schematic. It is not asserted against.
    #[test]
    fn the_hot_gas_duct_clears_the_artwork_and_the_risers() {
        let path = hot_gas_duct_path();
        let sg = sg_rect();

        for leg in path.windows(2) {
            let swept = Rect::from_two_pos(leg[0], leg[1]).expand(HALF_DUCT);
            assert!(
                !swept.intersects(sg),
                "leg {:?} -> {:?} runs over the steam generator {sg:?}",
                leg[0],
                leg[1]
            );
        }

        let drop_x = path[1].x;
        assert!((drop_x - COLD_RETURN_RISER_X).abs() > HELIUM_PIPE_THICKNESS);
        assert!((drop_x - FEEDWATER_HEADER_RISER_X).abs() > HELIUM_PIPE_THICKNESS);
        assert!(path[2].y > FEEDWATER_HEADER_Y + HALF_DUCT);
    }

    /// The cold-leg runs must still land where they did: this fix must not have
    /// moved the counterpart connections.
    ///
    /// **Methodology.** The maintainer asked specifically whether the cold
    /// return shares the anchoring convention and survived the change. The
    /// cold-helium outlet, the feedwater inlet and the steam outlet were all
    /// already derived from [`sg_rect`] with the artwork's own fractions; the
    /// change moved them behind [`sg_nozzles`] without altering the arithmetic.
    /// Pin the resulting positions numerically so a future edit to the anchor
    /// helper cannot silently move them.
    ///
    /// **Results (2026-08-13).** cold gas out (514.955, 175.828) pt, steam out
    /// (621.261, 197.207) pt, feedwater in (514.955, 516.865) pt — identical to
    /// the values the previous inline expressions produced (`center.x -+ 0.68 w`
    /// with `0.06 h`, `0.1125 h`, `0.8975 h`). The cold-return riser is
    /// unchanged at x 370.000, and it is the duct — not the cold leg — that was
    /// rerouted. Interpretation: the cold leg is untouched by this fix.
    #[test]
    fn the_cold_leg_anchors_are_unchanged() {
        let sg = sg_rect();
        let n = sg_nozzles();

        let expect =
            |dx: f32, fy: f32| pos2(sg.center().x + dx * sg.width(), sg.top() + fy * sg.height());
        for (name, got, want) in [
            ("cold gas out", n.cold_gas_out, expect(-0.68, 0.06)),
            ("steam out", n.steam_out, expect(0.68, 0.1125)),
            ("feedwater in", n.feed_in, expect(-0.68, 0.8975)),
        ] {
            assert!(
                (got.x - want.x).abs() < 1e-4 && (got.y - want.y).abs() < 1e-4,
                "{name}: {got:?} != {want:?}"
            );
        }
        assert!((COLD_RETURN_RISER_X - 370.0).abs() < f32::EPSILON);
    }
}
