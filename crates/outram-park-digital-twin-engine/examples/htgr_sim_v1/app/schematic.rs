//! Plant schematic panel, drawn on the engine's **real component artwork**
//! ([`outram_park_digital_twin_engine::components`]) in the arrangement an
//! HTR-10-style pebble-bed HTGR actually has.
//!
//! ## The arrangement is the point
//!
//! The defining feature of this plant is not the equipment list, it is the
//! layout. Following the description in IAEA-TECDOC-1382 (ingested into this
//! workspace's literature layer at
//! `crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md`)
//! and the corrected topology of GitHub issue #154:
//!
//! - The reactor and the steam generator sit in **two separate pressure
//!   vessels, side by side**, tied together by a **compact horizontal duct
//!   bundle** carrying the hot and the cold helium *immediately beside one
//!   another, counter-flowing*. There is no large rectangular loop around the
//!   plant. They are *not* stacked, and the steam generator is not inside the
//!   reactor vessel.
//! - The steam-generator pressure vessel is **offset upward** relative to the
//!   reactor: its bottom sits roughly 20 % of the reactor-vessel height above
//!   the reactor bottom, and its top projects a similar amount above the
//!   reactor top. The two vessels are of comparable height. (Authoritative
//!   layout: the maintainer's #154 comment.)
//! - The **helium circulator is installed in the steam-generator pressure
//!   vessel, above the steam generator**, connected to it by a connecting
//!   tube -- so it is drawn there, not at an arbitrary point in the loop.
//!
//! ### Primary-helium path, reactor side
//!
//! Cold helium arrives from the SG through the cold half of the horizontal
//! duct bundle, enters the reactor low in the **side-reflector** region, rises
//! through channels in the graphite side reflector, reaches the **upper
//! plenum** above the bed, passes **downward** through the pebble bed (the
//! geometry alone says so -- **no flow graphics are drawn over the bed**),
//! collects in the **hot-gas plenum** below the core, and leaves sideways
//! through the hot half of the duct bundle.
//!
//! ### Primary-helium path, steam-generator side
//!
//! Hot helium enters the SG vessel from the bundle and rises through the
//! **centre passage**, turns at the top, flows **down** past the helical-coil
//! bundle (drawn as two winding layers -- the two-channel character of the
//! real unit), turns at the bottom, and returns **up the vessel-wall
//! periphery** to the circulator, which sends the cooled helium **down** to
//! the cold half of the duct bundle. Feedwater enters the coil low and rises;
//! superheated steam leaves at the top -- opposite to the helium through the
//! coil region, i.e. counter-current, matching the physics
//! (`FlowArrangement::CounterCurrent`, guarded by
//! `steam_generator::counter_flow_index_map_is_its_own_inverse`).
//!
//! ```text
//!                    [circulator]  (in the SG vessel, above the SG)
//!                     |        ^
//!            cold He  v        |  periphery return (up)
//!   +--- cold He <====+   +----+----+                     steam
//!   |   (SG -> RPV)       |  ^   v  |  --------------------> [turbine] --+
//!   |                     | centre  |    (helical coil, counter-current)  |
//! [HTR-10 RPV] ===========+  up  down                                     v
//!   ^   hot He (RPV -> SG) |  (2 coil channels)                     [condenser]
//!   |   HORIZONTAL BUNDLE  +---------+                                    |
//!   +--- feedwater <--- [feed pump] <--- condensate <--------------------+
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
//! component artwork's own drawn rectangle -- [`Htr10FlowAnchors`] for the
//! reactor (from the engine crate, so the schematic's overlays cannot drift
//! from the cut-away) and [`SgNozzles`] for the steam generator -- using the
//! same fractions the widget paints the nozzle stub at. Move a component,
//! resize its box, or change its aspect ratio, and the pipe ends follow. A
//! hardcoded screen position that happens to line up today is exactly how the
//! hot gas duct once came to terminate on the steam generator's *left flank*
//! at an elevation set by the reactor -- see [`primary_hot_duct_path`].
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
//! snapping to it, so a drag produces visible rod travel.
//!
//! On a **scram** the drawn depth is floored at `scram_insertion_fraction`, the
//! protection system's own demand, matching the physics -- which uses the deeper
//! of operator command and scram demand. That floor bypasses the display slew
//! because the scram ramp is already rate-limited to a 2 s bank drop by
//! [`crate::physics::protection`], and slewing it again would draw the rods
//! still falling long after the reactivity had gone in.
//!
//! Two honesty notes on the drive:
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
    self, Htr10FlowAnchors, Htr10ReactorVesselVisual,
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

/// Colour of the sparse flow-direction chevrons drawn over paths the artwork
/// only *implies* -- the reflector risers, the SG centre passage, the coil
/// channels, the duct bundle. High-contrast and NOT temperature-coded: the
/// pipes and components underneath already carry the temperature colour, so
/// these only have to say which way the flow goes.
const FLOW_CUE: Color32 = Color32::from_rgb(232, 236, 244);

/// Colour of the water/steam-side flow chevrons in the steam generator, so the
/// counter-current relationship (helium down the coil, water up it) reads at a
/// glance without filling the vessel with arrows.
const WATER_CUE: Color32 = Color32::from_rgb(150, 198, 232);

// ── Canvas geometry ─────────────────────────────────────────────────────────
//
// Every position below is in schematic-local points and is shifted into canvas
// coordinates by `draw_schematic`. The two vessels are laid out at ONE common
// real scale so their relative slenderness is honest: 400 points stands for the
// reactor vessel's published 11.1 m, which makes the steam-generator vessel's
// 11.3 m come out at 407 points. Each widget then letterboxes to its own
// published aspect ratio inside the box it is given.
//
// The steam-generator vessel is deliberately OFFSET UPWARD from the reactor
// (GitHub #154, maintainer's authoritative layout comment): its bottom sits
// roughly 0.20 of the reactor-vessel height above the reactor bottom, and its
// top projects a similar amount above the reactor top, with the two vessels of
// comparable height. `steam_generator_is_offset_upward_from_the_reactor` pins
// this.

/// Canvas size reserved for the whole schematic, in points.
const CANVAS: Vec2 = Vec2::new(1180.0, 810.0);

/// Points per metre of real vessel height, shared by both pressure vessels.
const POINTS_PER_METRE: f32 = 400.0 / 11.1;

/// Centre of the reactor-vessel box.
const REACTOR_CENTRE: Pos2 = pos2(150.0, 430.0);
/// Size of the reactor-vessel box (the artwork letterboxes inside it).
const REACTOR_BOX: Vec2 = Vec2::new(190.0, 400.0);

/// Centre of the steam-generator box. Chosen so [`sg_rect`] sits offset upward
/// from [`reactor_rect`] -- SG bottom about 0.20 H above the reactor bottom, SG
/// top a similar amount above the reactor top (H = reactor-vessel height).
const SG_CENTRE: Pos2 = pos2(560.0, 342.0);
/// Size of the steam-generator box; height is 11.3 m at [`POINTS_PER_METRE`].
const SG_BOX: Vec2 = Vec2::new(120.0, 11.3 * POINTS_PER_METRE);

/// Centre of the helium circulator, in the SG pressure vessel above the SG.
const CIRCULATOR_CENTRE: Pos2 = pos2(500.0, 82.0);
/// Size of the circulator box.
const CIRCULATOR_BOX: Vec2 = Vec2::new(72.0, 88.0);

/// Centre of the turbine.
const TURBINE_CENTRE: Pos2 = pos2(840.0, 197.0);
/// Size of the turbine.
const TURBINE_BOX: Vec2 = Vec2::new(156.0, 92.0);

/// Centre of the condenser.
const CONDENSER_CENTRE: Pos2 = pos2(990.0, 300.0);
/// Size of the condenser.
const CONDENSER_BOX: Vec2 = Vec2::new(86.0, 62.0);

/// Centre of the feedwater pump.
const FEED_PUMP_CENTRE: Pos2 = pos2(820.0, 690.0);
/// Size of the feedwater pump box.
const FEED_PUMP_BOX: Vec2 = Vec2::new(70.0, 84.0);

/// The dashed boundary standing for the steam-generator pressure vessel, which
/// houses the steam generator, the IHX and the helium circulator.
const SG_VESSEL_BOUNDARY: Rect = Rect {
    min: pos2(438.0, 32.0),
    max: pos2(628.0, 548.0),
};

/// Vertical gap between the hot and the cold duct centrelines where they run
/// together as the horizontal cross-vessel bundle, in schematic-local points.
///
/// Small on purpose: the two runs read as one bundle carrying counter-flowing
/// helium, not as two unrelated pipes. See [`primary_hot_duct_path`] /
/// [`primary_cold_duct_path`].
const BUNDLE_DUCT_GAP: f32 = 24.0;

/// Vertical centreline of the cold-helium down-run on the steam-generator side,
/// from the circulator down to the cold half of the horizontal bundle, in
/// schematic-local points. Just inside [`SG_VESSEL_BOUNDARY`]'s left edge -- the
/// circulator and its discharge run are inside the SG pressure vessel -- so the
/// cold run drops down the inside of the vessel wall and crosses the boundary
/// once, heading back to the reactor.
const COLD_DUCT_SG_DROP_X: f32 = 456.0;

/// Vertical centreline of the feedwater riser into the steam generator's
/// bottom-**right** nozzle, in schematic-local points.
///
/// It sits *outside* [`SG_VESSEL_BOUNDARY`] on the turbine-hall side, so the
/// feedwater run crosses the vessel boundary horizontally at the nozzle -- and
/// never has to cross the primary-helium bundle, which is all on the reactor
/// side.
const FEEDWATER_HEADER_RISER_X: f32 = 672.0;

/// Elevation of the feedwater header -- the horizontal run along the bottom of
/// the plant, from the feed pump across to [`FEEDWATER_HEADER_RISER_X`] -- in
/// schematic-local points.
const FEEDWATER_HEADER_Y: f32 = 628.0;

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

/// Draw `count` small arrowheads evenly along the segment `from` -> `to`, each
/// pointing the way the segment runs, in `colour`.
///
/// A restrained flow-direction cue for a path the artwork *implies* -- a
/// reflector riser, the SG centre passage, the coil channels -- rather than one
/// drawn as its own pipe. The issue #154 spec asks for "a small number of
/// directional indicators" and "sparse directional indication", not arrows over
/// everything, and explicitly **no flow graphics inside the pebble bed**.
fn chevrons(ui: &Ui, from: Pos2, to: Pos2, count: usize, colour: Color32, size: f32) {
    let seg = to - from;
    let len = seg.length();
    if len < 1.0 || count == 0 {
        return;
    }
    let dir = seg / len;
    let normal = Vec2::new(-dir.y, dir.x);
    let painter = ui.painter();
    for i in 0..count {
        let t = (i as f32 + 0.5) / count as f32;
        let tip = from + seg * (t * len);
        let back = tip - dir * size;
        let stroke = Stroke::new(2.0_f32, colour);
        painter.line_segment([back + normal * size * 0.62, tip], stroke);
        painter.line_segment([back - normal * size * 0.62, tip], stroke);
    }
}

/// The steam generator's internal primary-helium path, in schematic-local
/// points, as GitHub issue #154 describes it: in at the hot-gas nozzle, **up**
/// the centre passage, over at the top, **down** past the helical coil (two
/// channels), turn at the bottom, **up** the vessel-wall periphery, and out at
/// the cold-gas nozzle.
///
/// `SteamGeneratorVisual::draw_helical_coil` already paints the centre tube,
/// the coil layers and the cold-return annulus; this is the ordered centreline
/// the schematic hangs its sparse flow chevrons on, and the reference the
/// counter-current test asserts against. The water/steam side runs the other
/// way through the coil region (feedwater in low, superheated steam out high),
/// which is the counter-current arrangement the physics uses
/// (`FlowArrangement::CounterCurrent`).
struct SgHeliumRoute {
    /// Ordered centreline, hot-gas inlet -> cold-gas outlet.
    path: Vec<Pos2>,
    /// The central-passage leg: helium rising up the centre of the unit.
    centre_ascent: (Pos2, Pos2),
    /// The coil-region legs, one per drawn coil channel: helium descending
    /// past the helical bundle, opposite the rising water/steam.
    coil_descent: [(Pos2, Pos2); 2],
    /// The peripheral-return leg: cooled helium rising up the vessel wall to
    /// the circulator.
    periphery_ascent: (Pos2, Pos2),
}

/// Build [`SgHeliumRoute`] for the box the steam generator is drawn in here.
///
/// The fractions sit on the widget's own drawn features: the centre column is
/// `|x| < 0.06 w`, the two coil layers are at about `0.17 w` and `0.30 w`, and
/// the cold-return annulus is near `0.40 w`.
fn sg_helium_route() -> SgHeliumRoute {
    let sg = sg_rect();
    let x = |f: f32| sg.center().x + f * sg.width();
    let y = |f: f32| sg.top() + f * sg.height();
    let n = sg_nozzles();

    let centre_top = y(0.17);
    let centre_bot = y(0.88);
    let coil_top = y(0.22);
    let coil_bot = y(0.80);
    let periph_top = y(0.12);
    let periph_bot = y(0.85);

    SgHeliumRoute {
        path: vec![
            n.hot_gas_in,
            pos2(x(0.0), centre_bot),
            pos2(x(0.0), centre_top),
            pos2(x(0.24), centre_top),
            pos2(x(0.24), coil_bot),
            pos2(x(0.40), periph_bot),
            pos2(x(0.40), periph_top),
            pos2(x(-0.20), periph_top),
            n.cold_gas_out,
        ],
        centre_ascent: (pos2(x(0.0), centre_bot), pos2(x(0.0), centre_top)),
        coil_descent: [
            (pos2(x(-0.24), coil_top), pos2(x(-0.24), coil_bot)),
            (pos2(x(0.24), coil_top), pos2(x(0.24), coil_bot)),
        ],
        periphery_ascent: (pos2(x(0.40), periph_bot), pos2(x(0.40), periph_top)),
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

/// Where a centrifugal pump's **suction** nozzle meets the volute casing.
///
/// The companion to [`pump_discharge`], and it exists for the same reason: a
/// pipe end has to be derived from the artwork's own geometry or it drifts off
/// the glyph. Two things make the naive `rect.right()` at `rect.center().y`
/// wrong, and together they were leaving the feedwater line visibly floating
/// clear of the pump:
///
/// - **Vertically**, `PumpVisual`'s volute is centred at
///   `rect.bottom() - 0.42 h`, which is *below* the box centre. A pipe drawn at
///   the box centre sits about `0.08 h` above the impeller axis.
/// - **Horizontally**, the volute casing only reaches about `0.44 w` from its
///   centre at the throat, so the box's right edge is roughly `0.06 w` outside
///   the casing -- before any additional clearance is added.
///
/// This returns the point on the casing at the impeller axis, using the same
/// mid-throat/cutwater radius [`pump_discharge`] uses, so the suction line
/// lands on the pump instead of near it.
fn pump_suction(rect: Rect) -> Pos2 {
    Pos2::new(
        rect.center().x + 0.3775 * rect.width(),
        rect.bottom() - 0.42 * rect.height(),
    )
}

/// Where steam meets the turbine's **casing**, rather than its shaft.
///
/// `TurbineVisual` draws a single-flow machine as an annulus that opens out
/// along the expansion: the blade tip radius runs from `hub_radius`
/// (`0.18 x` the half-height) at admission to the full half-height at exhaust,
/// with the rotor shaft on the centre line throughout. So the *centre line is
/// the shaft*, and a pipe terminated there appears to grow out of the rotor
/// rather than out of the steam path.
///
/// Both connections are therefore taken to the edge of the flow annulus at the
/// relevant end:
///
/// - **admission** (left) -- the annulus is still narrow there, so the casing
///   edge sits just off the hub at `0.09 h` above the centre line. Going all
///   the way to the box edge would float in empty space, because the annulus
///   has not opened yet.
/// - **exhaust** (right) -- the annulus is fully open, so the casing edge is
///   the box edge.
///
/// Returned in schematic-local points; `x` is inset by half a blade pitch so
/// the pipe meets the end blade row rather than the corner of the box.
struct TurbineNozzles {
    /// Main-steam admission, on the casing at the inlet end.
    steam_in: Pos2,
    /// Exhaust to the condenser, on the casing at the exhaust end.
    exhaust_out: Pos2,
}

/// Blade rows drawn by `TurbineVisual`; the end rows sit half a pitch inside
/// the box, which is where a nozzle should meet them.
const TURBINE_BLADE_ROWS: f32 = 11.0;

/// Hub radius as a fraction of the half-height, matching `TurbineVisual`'s
/// own `HUB_RADIUS_FRACTION`.
const TURBINE_HUB_RADIUS_FRACTION: f32 = 0.18;

fn turbine_nozzles() -> TurbineNozzles {
    let rect = Rect::from_center_size(TURBINE_CENTRE, TURBINE_BOX);
    let half_pitch = 0.5 * rect.width() / TURBINE_BLADE_ROWS;
    let tip_radius = 0.5 * rect.height();
    let hub_radius = tip_radius * TURBINE_HUB_RADIUS_FRACTION;

    TurbineNozzles {
        // Admission: top of the (still narrow) annulus at the inlet end.
        steam_in: pos2(rect.left() + half_pitch, TURBINE_CENTRE.y - hub_radius),
        // Exhaust: bottom of the fully opened annulus, i.e. the casing edge,
        // pointing down towards the condenser.
        exhaust_out: pos2(rect.right() - half_pitch, rect.bottom()),
    }
}

/// The reactor artwork's own interior flow anchors -- the side-reflector
/// risers, the upper-plenum elevation, the hot-gas plenum and the hot-gas duct
/// nozzle -- computed by the engine crate from the same fractions
/// [`Htr10ReactorVesselVisual`] paints with, for the box this schematic draws
/// the reactor in. The schematic's reactor-side flow overlays are aligned to
/// these rather than eyeballed, so they cannot drift from the cut-away.
fn reactor_flow_anchors() -> Htr10FlowAnchors {
    htr10_reactor_vessel::flow_anchors(Rect::from_center_size(REACTOR_CENTRE, REACTOR_BOX))
}

/// The outboard tip of the reactor vessel's hot gas duct nozzle, in
/// schematic-local points -- where the horizontal hot-gas duct to the steam
/// generator starts. Its centreline elevation is the hot-gas plenum's
/// mid-height (`0.66 h`).
fn reactor_duct_nozzle() -> Pos2 {
    reactor_flow_anchors().hot_gas_duct_nozzle
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
    /// Cold-helium outlet to the circulator: **upper left** (reactor side).
    /// Artwork stub `-0.68 w .. -0.10 w` over `0.045 h .. 0.075 h`, so tip
    /// `-0.68 w`, centreline `0.06 h`.
    cold_gas_out: Pos2,
    /// Superheated-steam outlet to the turbine: **upper right** (turbine-hall
    /// side). Artwork stub `+0.30 w .. +0.68 w` over `0.10 h .. 0.125 h`, so
    /// tip `+0.68 w`, centreline `0.1125 h`.
    steam_out: Pos2,
    /// Feedwater inlet: **lower right** (turbine-hall side). Artwork stub
    /// `+0.30 w .. +0.68 w` over `0.885 h .. 0.91 h`, so tip `+0.68 w`,
    /// centreline `0.8975 h`.
    feed_in: Pos2,
    /// Hot-gas duct inlet, feeding the foot of the centre tube: **lower left**
    /// (reactor side). Artwork stub `-0.68 w .. -0.02 w` over
    /// `0.925 h .. 0.955 h`, so tip `-0.68 w`, centreline `0.94 h`.
    ///
    /// It faces the reactor, so the horizontal duct bundle runs straight in to
    /// it -- see [`primary_hot_duct_path`].
    hot_gas_in: Pos2,
}

/// The steam generator's nozzle anchors for the box it is drawn in here.
///
/// Primary helium (`cold_gas_out`, `hot_gas_in`) is on the **left**, facing
/// the reactor; water/steam (`steam_out`, `feed_in`) is on the **right**,
/// facing the turbine hall. The fractions match
/// `SteamGeneratorVisual::draw_helical_coil`'s nozzle stubs exactly, so the
/// pipe ends move with the artwork.
fn sg_nozzles() -> SgNozzles {
    let sg = sg_rect();
    let x = |f: f32| sg.center().x + f * sg.width();
    let y = |f: f32| sg.top() + f * sg.height();
    SgNozzles {
        cold_gas_out: pos2(x(-0.68), y(0.06)),
        steam_out: pos2(x(0.68), y(0.1125)),
        feed_in: pos2(x(0.68), y(0.8975)),
        hot_gas_in: pos2(x(-0.68), y(0.94)),
    }
}

/// Elevation of the **hot** half of the horizontal cross-vessel duct bundle,
/// in schematic-local points: the reactor hot-gas plenum's own mid-height, so
/// the run leaves the plenum level.
fn primary_bundle_hot_y() -> f32 {
    reactor_duct_nozzle().y
}

/// Centreline of the **hot** helium duct, reactor hot-gas plenum -> steam
/// generator, corner to corner in schematic-local points.
///
/// A compact, near-horizontal run: it leaves the reactor at the plenum
/// elevation, runs across the gap between the vessels as the hot half of the
/// duct bundle, and steps down into the SG's bottom-left hot-gas nozzle (the
/// foot of the centre tube). It never wraps around a vessel and never crosses
/// the feedwater run, which is entirely on the far side of the SG.
fn primary_hot_duct_path() -> [Pos2; 4] {
    let start = reactor_duct_nozzle();
    let hot_in = sg_nozzles().hot_gas_in;
    let bundle_y = primary_bundle_hot_y();
    // Step down to the nozzle elevation a short way before the SG, so the
    // last leg runs straight in to the stub from the left.
    let step_x = hot_in.x - 34.0;
    [
        pos2(start.x - 6.0, bundle_y),
        pos2(step_x, bundle_y),
        pos2(step_x, hot_in.y),
        pos2(hot_in.x + NOZZLE_SEAM_OVERLAP, hot_in.y),
    ]
}

/// Centreline of the **cold** helium duct, steam-generator circulator ->
/// reactor, corner to corner in schematic-local points.
///
/// The counterpart of [`primary_hot_duct_path`] and its immediate neighbour:
/// it drops from the circulator down the outside of the SG vessel
/// ([`COLD_DUCT_SG_DROP_X`]), then runs back to the reactor as the **cold**
/// half of the bundle, [`BUNDLE_DUCT_GAP`] above the hot run and flowing the
/// opposite way, ending at the reactor wall just above the hot-gas nozzle
/// where the cold return annulus enters in the real plant.
fn primary_cold_duct_path() -> [Pos2; 4] {
    let discharge = pump_discharge(circulator_rect());
    let cold_y = primary_bundle_hot_y() - BUNDLE_DUCT_GAP;
    let reactor = reactor_rect();
    [
        pos2(discharge.x, discharge.y),
        pos2(COLD_DUCT_SG_DROP_X, discharge.y),
        pos2(COLD_DUCT_SG_DROP_X, cold_y),
        pos2(reactor.right() + 2.0, cold_y),
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
    // Suction nozzle on the volute casing, not the bounding box -- see
    // `pump_suction` for why the box edge left the line floating.
    let feed_suction = pump_suction(feed_pump);

    // Nozzle anchors, taken from each artwork's own drawn rectangle (see
    // `reactor_flow_anchors` and `sg_nozzles`), so a pipe end cannot drift off
    // its nozzle when a component is moved or re-proportioned.
    let anchors = reactor_flow_anchors();
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
    // A COMPACT HORIZONTAL DUCT BUNDLE between the two vessels (GitHub #154):
    // the hot half (reactor -> SG) and the cold half (SG -> reactor) run
    // immediately beside one another, `BUNDLE_DUCT_GAP` apart, helium flowing
    // the opposite way in each. No large rectangular loop around the plant.

    // Hot half: reactor hot-gas plenum -> foot of the SG centre tube.
    let hot_path: Vec<Pos2> = primary_hot_duct_path().into_iter().map(at).collect();
    route(ui, &hot_helium, &hot_path, false, false);

    // Cold half, part 1: SG cold-gas outlet rising to the circulator suction.
    route(
        ui,
        &cold_helium,
        &[
            at(pos2(sg_cold_out.x + NOZZLE_SEAM_OVERLAP, sg_cold_out.y)),
            at(pos2(sg_cold_out.x, circulator.bottom() - 2.0)),
        ],
        false,
        false,
    );
    // Cold half, part 2: circulator discharge -> down the vessel wall -> back
    // to the reactor as the cold half of the bundle, ending at the wall just
    // above the hot-gas nozzle (the cold return annulus, in the real plant).
    let cold_path: Vec<Pos2> = primary_cold_duct_path().into_iter().map(at).collect();
    route(ui, &core_inlet_helium, &cold_path, false, true);

    // Big counter-flow arrows on the bundle: the single clearest read of the
    // primary loop, so it gets explicit direction cues on top of the tracers.
    let bundle_hot_y = primary_bundle_hot_y();
    let bundle_cold_y = bundle_hot_y - BUNDLE_DUCT_GAP;
    let bundle_mid_x = 0.5 * (reactor.right() + COLD_DUCT_SG_DROP_X);
    chevrons(
        ui,
        at(pos2(bundle_mid_x - 46.0, bundle_hot_y)),
        at(pos2(bundle_mid_x + 46.0, bundle_hot_y)),
        2,
        FLOW_CUE,
        9.0,
    ); // hot: reactor -> SG
    chevrons(
        ui,
        at(pos2(bundle_mid_x + 46.0, bundle_cold_y)),
        at(pos2(bundle_mid_x - 46.0, bundle_cold_y)),
        2,
        FLOW_CUE,
        9.0,
    ); // cold: SG -> reactor

    // NB: the reactor-side and SG-side flow overlays (reflector risers, upper
    // plenum, hot-gas plenum, SG centre/coil/periphery chevrons) are drawn
    // AFTER their component widgets, in sections 4b and 5b, or the widget would
    // paint over them.

    // ── 3. Secondary steam circuit ──────────────────────────────────────
    let turbine_rect = Rect::from_center_size(TURBINE_CENTRE, TURBINE_BOX);
    let condenser_rect = Rect::from_center_size(CONDENSER_CENTRE, CONDENSER_BOX);

    // Admission joins the casing just off the hub, not the shaft centre line
    // -- see `turbine_nozzles`.
    let turbine_nozzle = turbine_nozzles();
    route(
        ui,
        &main_steam,
        &[
            at(pos2(sg_steam_out.x - 5.0, sg_steam_out.y)),
            at(pos2(turbine_nozzle.steam_in.x, sg_steam_out.y)),
            at(turbine_nozzle.steam_in),
        ],
        false,
        false,
    );
    // Exhaust leaves the BOTTOM of the casing at the exhaust end, where the
    // annulus has fully opened -- not the shaft centre line it used to start
    // from, which made the pipe look like it grew out of the rotor.
    let exhaust_drop_y = turbine_rect.bottom() + 26.0;
    route(
        ui,
        &exhaust,
        &[
            at(turbine_nozzle.exhaust_out),
            at(pos2(turbine_nozzle.exhaust_out.x, exhaust_drop_y)),
            at(pos2(CONDENSER_CENTRE.x, exhaust_drop_y)),
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
            at(pos2(CONDENSER_CENTRE.x, feed_suction.y)),
            at(feed_suction),
        ],
        false,
        false,
    );

    // Feedwater: pump -> header along the bottom -> up the turbine-hall-side
    // riser -> in to the SG's bottom-right nozzle. It stays entirely on the
    // turbine side and never crosses the primary-helium bundle.
    let feed_discharge = pump_discharge(feed_pump);
    route(
        ui,
        &feed,
        &[
            at(feed_discharge),
            at(pos2(feed_discharge.x, FEEDWATER_HEADER_Y)),
            at(pos2(FEEDWATER_HEADER_RISER_X, FEEDWATER_HEADER_Y)),
            at(pos2(FEEDWATER_HEADER_RISER_X, sg_feed_in.y)),
            at(pos2(sg_feed_in.x - NOZZLE_SEAM_OVERLAP, sg_feed_in.y)),
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
    //
    // Pebble colour is `bed_temperature_k` (the bed's own solid-phase node),
    // NOT `fuel_temperature_k` (the kinetics reactivity-feedback node) --
    // see that field's doc comment. Reading the kinetics node here was
    // GitHub issue #22's "unphysically cold pebbles" symptom: that node has
    // no decay-heat source but is charged the same coolant sink as the bed,
    // so it drifts well below the bed's real temperature after a trip while
    // this widget painted it as though it WERE the bed.
    let mut vessel = Htr10ReactorVesselVisual::new(
        REACTOR_BOX,
        k(DISPLAY_MIN_K),
        k(DISPLAY_MAX_K),
        k(snapshot.bed_temperature_k),
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
    // A SCRAM must be visible. The physics uses
    // `ReactorProtectionSystem::effective_rod_insertion`, i.e. the DEEPER of the
    // operator's command and the scram demand, so drawing the operator's command
    // alone left the rods sitting withdrawn on screen while the model had
    // already driven them in -- the plant shutting down with no rod motion to
    // explain it.
    //
    // The floor is taken AFTER the slew, and deliberately not fed through it:
    // `scram_insertion_fraction` is already a rate-limited travel, the
    // protection system's 2 s gravity-assisted bank drop
    // (`protection::SCRAM_INSERTION_TIME_S`). Passing it through the 20 s
    // motor-drive slew as well would rate-limit it twice and draw the bank still
    // travelling ten times longer than the reactivity it inserted took to
    // arrive. The operator's own command keeps the motor slew, because that
    // slider IS a step input.
    let drawn_rod_insertion = drawn_rod_insertion.max(snapshot.scram_insertion_fraction as f32);
    vessel.set_control_rod_frac(drawn_rod_insertion);
    ui.put(
        at_rect(Rect::from_center_size(REACTOR_CENTRE, REACTOR_BOX)),
        vessel,
    );

    // ── 4b. Reactor-side helium routing, aligned to the cut-away ────────
    //
    // Drawn ON TOP of the vessel widget. `reactor_flow_anchors` gives the drawn
    // reflector risers, the upper-plenum elevation and the hot-gas plenum. NO
    // flow graphics are drawn over the pebble bed (GitHub #154 §4) -- the
    // geometry between the upper plenum and the hot-gas plenum is left to carry
    // the downward core flow.

    // Cold helium rising through the side-reflector channels (one channel per
    // side; the widget already paints all four at the inlet colour).
    for &rx in &[anchors.reflector_riser_x[1], anchors.reflector_riser_x[3]] {
        chevrons(
            ui,
            at(pos2(rx, anchors.reflector_channel_bottom_y - 4.0)),
            at(pos2(rx, anchors.reflector_channel_top_y + 8.0)),
            3,
            FLOW_CUE,
            6.0,
        );
    }
    // A short cue linking the cold-duct entry at the wall down to the foot of
    // the reflector channels.
    chevrons(
        ui,
        at(pos2(reactor.right() - 4.0, bundle_cold_y + 6.0)),
        at(pos2(
            anchors.reflector_riser_x[3],
            anchors.reflector_channel_bottom_y - 4.0,
        )),
        2,
        FLOW_CUE,
        5.0,
    );

    // Upper plenum: a bracket across the top of the bed, cold helium turning
    // inward and down into the core. The label sits clear of the vessel, left.
    let plenum_bar_y = anchors.bed_top_y - 12.0;
    let plenum_l = anchors.reflector_riser_x[1];
    let plenum_r = anchors.reflector_riser_x[3];
    ui.painter().line_segment(
        [
            at(pos2(plenum_l, plenum_bar_y)),
            at(pos2(plenum_r, plenum_bar_y)),
        ],
        Stroke::new(2.5_f32, FLOW_CUE),
    );
    chevrons(
        ui,
        at(pos2(plenum_l + 4.0, plenum_bar_y)),
        at(pos2(anchors.axis_x - 10.0, plenum_bar_y)),
        1,
        FLOW_CUE,
        6.0,
    );
    chevrons(
        ui,
        at(pos2(plenum_r - 4.0, plenum_bar_y)),
        at(pos2(anchors.axis_x + 10.0, plenum_bar_y)),
        1,
        FLOW_CUE,
        6.0,
    );
    ui.painter().text(
        at(pos2(reactor.left() - 6.0, plenum_bar_y)),
        Align2::RIGHT_CENTER,
        "upper plenum",
        FontId::proportional(9.5),
        ANNOTATION,
    );

    // Hot helium collecting below the core, into the hot-gas plenum (the
    // widget draws and labels the plenum itself).
    chevrons(
        ui,
        at(pos2(anchors.axis_x, anchors.bed_bottom_y + 3.0)),
        at(pos2(anchors.axis_x, anchors.hot_gas_plenum.center().y)),
        1,
        FLOW_CUE,
        7.0,
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

    // ── 5b. SG-side helium routing ─────────────────────────────────────
    //
    // Drawn ON TOP of the SG widget, which already paints the centre tube, the
    // coil layers and the cold-return annulus; these chevrons make the
    // direction explicit without filling the vessel (GitHub #154 §6-10):
    // centre UP, two coil channels DOWN, periphery UP.
    let sg_he = sg_helium_route();
    // A faint guide along the full helium thread, so the connected route --
    // in, up the centre, over, down the coil, round, up the periphery, out --
    // reads as one path without filling the vessel with arrows.
    ui.painter().add(egui::Shape::line(
        sg_he.path.iter().map(|p| at(*p)).collect(),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(232, 236, 244, 60)),
    ));
    chevrons(
        ui,
        at(sg_he.centre_ascent.0),
        at(sg_he.centre_ascent.1),
        3,
        FLOW_CUE,
        6.0,
    );
    for (a, b) in sg_he.coil_descent {
        chevrons(ui, at(a), at(b), 3, FLOW_CUE, 6.0);
    }
    chevrons(
        ui,
        at(sg_he.periphery_ascent.0),
        at(sg_he.periphery_ascent.1),
        3,
        FLOW_CUE,
        6.0,
    );

    // Water/steam through the coil runs the OTHER way -- feedwater in low,
    // superheated steam out high -- so its up-chevrons sit beside the helium's
    // down-chevrons and the counter-current relationship reads directly.
    {
        let sg = sg_rect();
        let wx = |f: f32| sg.center().x + f * sg.width();
        let wy = |f: f32| sg.top() + f * sg.height();
        for f in [-0.14_f32, 0.14] {
            chevrons(
                ui,
                at(pos2(wx(f), wy(0.80))),
                at(pos2(wx(f), wy(0.24))),
                3,
                WATER_CUE,
                5.0,
            );
        }
        ui.painter().text(
            at(pos2(sg.right() + 10.0, wy(0.60))),
            Align2::LEFT_CENTER,
            "helical coil: counter-current\nhelium down, water up",
            FontId::proportional(9.0),
            ANNOTATION,
        );
    }

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
    // Stacked short lines to the RIGHT of the steam generator, clear of the
    // helium routing on its left and below the water-side readouts.
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
            pos2(sg_rect().right() + 12.0, 410.0 + 12.0 * i as f32),
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
    // On the horizontal cross-vessel bundle, between the two vessels.
    let hot_duct = primary_hot_duct_path();
    tag(
        ui,
        pos2(
            0.5 * (hot_duct[0].x + COLD_DUCT_SG_DROP_X),
            primary_bundle_hot_y() + 14.0,
        ),
        Align2::CENTER_TOP,
        "hot / cold helium duct bundle (counter-flow)",
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
        pos2(FEEDWATER_HEADER_RISER_X + 8.0, 560.0),
        Align2::LEFT_CENTER,
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
            pos2(58.0, 648.0),
            "Power",
            format!("{:.1} MWth", snapshot.reactor_power_mw),
        ),
        (
            pos2(58.0, 666.0),
            "T_fuel",
            // bed_temperature_k, not fuel_temperature_k -- see this
            // function's "T_fuel" comment above the vessel widget.
            temp(snapshot.bed_temperature_k),
        ),
        (
            pos2(58.0, 684.0),
            "T_He,out",
            temp(snapshot.core_outlet_temp_k),
        ),
        (
            pos2(58.0, 702.0),
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
        at(pos2(58.0, 722.0)),
        "real-time",
        match snapshot.real_time_ratio {
            Some(ratio) => format!("{ratio:.2}x"),
            None => "--".to_string(),
        },
    ));
    if snapshot.behind_real_time {
        ui.painter().text(
            at(pos2(58.0, 742.0)),
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
        at(pos2(8.0, 766.0)),
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
    //! ── Layout invariants (bead `op-z6bk` / gh #154) ──────────────────────
    //!
    //! The schematic drifted away from the physics and nothing caught it: the
    //! model flows helium downward through the core and runs the steam
    //! generator counter-current, while the drawing said otherwise, and every
    //! test passed because the tests check numbers and the picture was checked
    //! by eye.
    //!
    //! GitHub #154 corrected the drawing: a compact horizontal hot/cold duct
    //! bundle between the two vessels (no rectangular loop), the SG offset
    //! upward from the reactor, upward cold-helium flow in the side reflector
    //! into an upper plenum, downward flow through the core (geometry only, no
    //! graphics over the bed), and the SG-internal helium path centre-up ->
    //! coil-down -> periphery-up counter-current to the rising water/steam.
    //!
    //! These assert on the layout functions directly. They are pure geometry —
    //! `sg_rect()`, `sg_nozzles()`, `primary_hot_duct_path()`,
    //! `sg_helium_route()`, `reactor_flow_anchors()` all take no `Ui` — so a
    //! flow-direction bug is a few floats, not a screenshot. See
    //! `outram_park_digital_twin_engine::ascii` for rendering the same geometry
    //! when you want to look at it rather than assert on it.

    use super::*;

    /// **Helium and water/steam must traverse the SG coil region in OPPOSITE
    /// vertical directions** -- the counter-current arrangement the physics
    /// uses.
    ///
    /// The physics is counter-current and defended:
    /// `temperature_cross::lmtd_profile` uses `FlowArrangement::CounterCurrent`,
    /// and `steam_generator`'s `counter_flow_index_map_is_its_own_inverse`
    /// warns that getting it wrong makes the exchanger *"silently become
    /// co-current, which is a different (and worse) machine that still runs and
    /// still produces plausible-looking numbers."*
    ///
    /// **This replaces the `#[ignore]`d `sg_gas_and_water_flow_in_opposite_directions`
    /// (gh #154 item 5).** That test compared nozzle-to-nozzle elevation, which
    /// is the wrong quantity: in a once-through helical unit the helium goes
    /// *up* the centre tube and *down* over the coil, so its two nozzles both
    /// end up at the bottom. What matters is the direction through the
    /// **heat-transfer region** -- the coil -- and GitHub #154's spec settles
    /// it: helium **down** the two coil channels, feedwater **up** the coil to
    /// superheated steam. [`sg_helium_route`] is the drawn helium path;
    /// [`sg_nozzles`] gives the water endpoints.
    ///
    /// Screen y grows DOWN, so "descends" is `y` increasing and "rises" is `y`
    /// decreasing.
    #[test]
    fn sg_helium_and_water_are_counter_current_through_the_coil() {
        let r = sg_helium_route();
        let n = sg_nozzles();

        for (i, (a, b)) in r.coil_descent.iter().enumerate() {
            assert!(
                b.y > a.y,
                "helium coil channel {i} must descend: {:.1} -> {:.1}",
                a.y,
                b.y
            );
        }
        assert!(
            r.centre_ascent.1.y < r.centre_ascent.0.y,
            "the SG centre passage must rise: {:.1} -> {:.1}",
            r.centre_ascent.0.y,
            r.centre_ascent.1.y
        );
        assert!(
            r.periphery_ascent.1.y < r.periphery_ascent.0.y,
            "the SG peripheral return must rise: {:.1} -> {:.1}",
            r.periphery_ascent.0.y,
            r.periphery_ascent.1.y
        );

        let water_rises = n.steam_out.y < n.feed_in.y;
        assert!(
            water_rises,
            "water/steam must rise through the coil: feed y={:.1}, steam y={:.1}",
            n.feed_in.y, n.steam_out.y
        );

        // Counter-current: helium down the coil, water up it.
        let helium_descends_coil = r.coil_descent.iter().all(|(a, b)| b.y > a.y);
        assert!(
            helium_descends_coil && water_rises,
            "SG must read counter-current through the coil"
        );
        println!(
            "coil: helium {:.1}->{:.1} (down), water {:.1}->{:.1} (up)",
            r.coil_descent[0].0.y, r.coil_descent[0].1.y, n.feed_in.y, n.steam_out.y
        );
    }

    /// The steam generator's internal helium route must connect its two primary
    /// nozzles: in at the hot-gas nozzle, out at the cold-gas nozzle.
    #[test]
    fn sg_helium_route_connects_the_primary_nozzles() {
        let r = sg_helium_route();
        let n = sg_nozzles();
        assert!(
            (r.path[0].x - n.hot_gas_in.x).abs() < 0.01
                && (r.path[0].y - n.hot_gas_in.y).abs() < 0.01
        );
        let last = *r.path.last().unwrap();
        assert!(
            (last.x - n.cold_gas_out.x).abs() < 0.01 && (last.y - n.cold_gas_out.y).abs() < 0.01
        );
    }

    /// Hot gas enters the SG low (foot of the centre tube) and the cold gas
    /// leaves high (to the circulator).
    #[test]
    fn sg_hot_gas_enters_below_the_cold_gas_outlet() {
        let n = sg_nozzles();
        assert!(
            n.hot_gas_in.y > n.cold_gas_out.y,
            "hot gas should enter below the cold outlet: in y={:.1}, out y={:.1}",
            n.hot_gas_in.y,
            n.cold_gas_out.y
        );
    }

    /// **The primary-helium connections are on the reactor-facing (left) side
    /// of the SG; the water/steam connections face the turbine hall (right).**
    ///
    /// This is what lets the horizontal duct bundle run straight in from the
    /// reactor without crossing the feedwater run (gh #154 §5).
    #[test]
    fn sg_primary_helium_is_on_the_reactor_side() {
        let sg = sg_rect();
        let n = sg_nozzles();
        assert!(
            n.hot_gas_in.x < sg.center().x,
            "hot gas in should be on the left"
        );
        assert!(
            n.cold_gas_out.x < sg.center().x,
            "cold gas out should be on the left"
        );
        assert!(
            n.feed_in.x > sg.center().x,
            "feedwater in should be on the right"
        );
        assert!(
            n.steam_out.x > sg.center().x,
            "steam out should be on the right"
        );
    }

    /// **The steam generator vessel is offset UPWARD from the reactor vessel.**
    ///
    /// GitHub #154, the maintainer's authoritative "STEAM-GENERATOR VERTICAL
    /// POSITION" comment: the SG bottom sits roughly 0.20 of the reactor-vessel
    /// height H above the reactor bottom, the SG top a similar amount above the
    /// reactor top, and the two vessels are of comparable height. This
    /// **replaces `steam_generator_sits_lower_than_the_reactor`**, which
    /// encoded the superseded instruction from the issue body (§12, "lower it")
    /// before that comment corrected it.
    ///
    /// Screen y grows down, so "above" is a smaller y and "offset upward" means
    /// the SG centre y is smaller than the reactor centre y.
    #[test]
    fn steam_generator_is_offset_upward_from_the_reactor() {
        let (r, sg) = (reactor_rect(), sg_rect());
        let h = r.height();

        assert!(
            sg.center().y < r.center().y,
            "SG should sit higher (offset upward) than the reactor: SG centre y={:.1}, reactor centre y={:.1}",
            sg.center().y,
            r.center().y
        );
        let bottom_offset = r.bottom() - sg.bottom();
        let top_offset = r.top() - sg.top();
        assert!(
            bottom_offset > 0.0,
            "SG bottom must be above the reactor bottom (offset {bottom_offset:.1})"
        );
        assert!(
            top_offset > 0.0,
            "SG top must be above the reactor top (offset {top_offset:.1})"
        );
        // ~0.20 H, taken loosely -- the maintainer said prefer visual judgement
        // over treating the percentage as a dimension.
        for (name, off) in [("bottom", bottom_offset), ("top", top_offset)] {
            assert!(
                (0.10 * h..=0.35 * h).contains(&off),
                "SG {name} offset {off:.1} pt is not near 0.20 H ({:.1} pt)",
                0.20 * h
            );
        }
        // Comparable heights.
        assert!(
            (sg.height() / r.height() - 1.0).abs() < 0.15,
            "vessels should be of comparable height: SG {:.1}, reactor {:.1}",
            sg.height(),
            r.height()
        );
        println!(
            "reactor y {:.1}..{:.1} (H={h:.1}); SG y {:.1}..{:.1}; offsets bottom {bottom_offset:.1}, top {top_offset:.1}",
            r.top(), r.bottom(), sg.top(), sg.bottom()
        );
    }

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
    ///
    /// Primary helium (`cold gas out`, `hot gas in`) is on the left, so
    /// "inboard" is `+x` (`1.0`); water/steam (`steam out`, `feedwater in`) is
    /// on the right, so "inboard" is `-x` (`-1.0`).
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
                sg_stub(0.30, 0.68, 0.885, 0.91),
                n.feed_in,
                -1.0,
            ),
            (
                "hot gas in",
                sg_stub(-0.68, -0.02, 0.925, 0.955),
                n.hot_gas_in,
                1.0,
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
    /// **Results (2026-09-09, after the gh #154 nozzle-side swap).** All four
    /// anchors match their stub tip and centreline to better than 0.01 pt
    /// (figures printed by the test). Primary helium (`cold gas out`,
    /// `hot gas in`) sits on the SG's left flank, water/steam (`steam out`,
    /// `feedwater in`) on the right. Interpretation: the four pipe ends are
    /// still pinned to the artwork after the swap.
    #[test]
    fn steam_generator_anchors_sit_on_the_artwork_nozzle_tips() {
        println!("SG artwork rect: {:?}", sg_rect());
        for (name, _, anchor, _) in stubs_and_anchors() {
            println!("  {name}: {anchor:?}");
        }
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
    /// **Results (2026-09-09).** All four passed. Interpretation: the seam
    /// overlap is safe for all four nozzles after the nozzle-side swap.
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

    /// **The hot half of the primary duct bundle must land on the SG's hot-gas
    /// nozzle, approaching horizontally from the reactor (left) side.**
    ///
    /// The nozzle now faces the reactor (gh #154 §5), so the run goes straight
    /// in from the left -- no wrapping round the vessel. Require the last point
    /// of [`primary_hot_duct_path`] to be [`NOZZLE_SEAM_OVERLAP`] inboard of
    /// [`SgNozzles::hot_gas_in`] on its centreline, and the final leg to be
    /// horizontal and arriving from the left.
    #[test]
    fn primary_hot_duct_lands_on_the_sg_hot_gas_nozzle() {
        let path = primary_hot_duct_path();
        let nozzle = sg_nozzles().hot_gas_in;
        let end = *path.last().unwrap();
        let prev = path[path.len() - 2];

        println!("SG artwork rect: {:?}", sg_rect());
        println!("reactor artwork rect: {:?}", reactor_rect());
        println!("hot gas nozzle tip: {nozzle:?}");
        println!("hot duct path: {path:?}");

        // Inboard of the tip is +x (the stub extends to the right of its tip).
        assert!(
            (end.x - (nozzle.x + NOZZLE_SEAM_OVERLAP)).abs() < 0.01,
            "duct ends at x {}, expected {}",
            end.x,
            nozzle.x + NOZZLE_SEAM_OVERLAP
        );
        assert!(
            (end.y - nozzle.y).abs() < 0.01,
            "duct ends at y {}, off the nozzle centreline {}",
            end.y,
            nozzle.y
        );
        // Final leg horizontal, arriving from the left.
        assert!((prev.y - end.y).abs() < 0.01, "final leg is not horizontal");
        assert!(prev.x < end.x, "final leg does not arrive from the left");
    }

    /// **The hot duct starts at the reactor hot-gas plenum elevation and every
    /// leg is axis-aligned** ([`elbow`] assumes right-angle turns, so a
    /// diagonal leg would silently draw a broken corner).
    #[test]
    fn primary_hot_duct_is_rectilinear_off_the_reactor_plenum() {
        let path = primary_hot_duct_path();
        let plenum_y = reactor_flow_anchors().hot_gas_plenum.center().y;

        assert!(
            (path[0].y - plenum_y).abs() < 0.01,
            "hot duct starts at y {}, not the plenum mid-height {plenum_y}",
            path[0].y
        );
        for leg in path.windows(2) {
            let axis_aligned =
                (leg[0].x - leg[1].x).abs() < 0.01 || (leg[0].y - leg[1].y).abs() < 0.01;
            assert!(axis_aligned, "leg {:?} -> {:?} is diagonal", leg[0], leg[1]);
        }
    }

    /// **The hot and cold ducts form one compact bundle: adjacent, close, and
    /// counter-flowing** (gh #154 §5). No leg of either goes over the pebble
    /// bed, and neither crosses the feedwater run.
    ///
    /// **Methodology.** The hot run (reactor -> SG) and the cold run
    /// (SG -> reactor) must run at nearly the same elevation over the
    /// cross-vessel span, [`BUNDLE_DUCT_GAP`] apart; their cross-vessel legs
    /// must run opposite ways in x. The whole bundle sits on the reactor side
    /// of [`FEEDWATER_HEADER_RISER_X`]. And no leg intersects the pebble-bed
    /// rectangle from [`reactor_flow_anchors`].
    #[test]
    fn primary_duct_bundle_is_compact_and_counter_flowing() {
        let hot = primary_hot_duct_path();
        let cold = primary_cold_duct_path();
        let a = reactor_flow_anchors();

        // Counter-flowing: the hot cross-vessel leg runs +x (reactor -> SG),
        // the cold cross-vessel leg runs -x (SG -> reactor).
        assert!(
            hot[1].x > hot[0].x,
            "hot bundle leg does not run toward the SG"
        );
        let cold_last = *cold.last().unwrap();
        let cold_prev = cold[cold.len() - 2];
        assert!(
            cold_last.x < cold_prev.x,
            "cold bundle leg does not run back toward the reactor"
        );

        // Adjacent and close: the hot bundle leg and the cold bundle leg are
        // within ~1.5 * BUNDLE_DUCT_GAP of each other in elevation.
        let hot_y = hot[0].y;
        let cold_y = cold_last.y;
        assert!(
            (hot_y - cold_y).abs() <= 1.6 * BUNDLE_DUCT_GAP,
            "hot ({hot_y:.1}) and cold ({cold_y:.1}) duct elevations are not bundled"
        );

        // Entirely on the reactor side of the feedwater run.
        let bundle_max_x = hot
            .iter()
            .chain(cold.iter())
            .map(|p| p.x)
            .fold(f32::MIN, f32::max);
        assert!(
            bundle_max_x < FEEDWATER_HEADER_RISER_X,
            "the primary bundle ({bundle_max_x:.1}) reaches past the feedwater riser ({FEEDWATER_HEADER_RISER_X})"
        );

        // No leg over the pebble bed. The bed is the axis-centred rectangle
        // between bed_top_y and bed_bottom_y, half-width 0.30 of the artwork.
        let bed = Rect::from_min_max(
            pos2(a.axis_x - 0.30 * a.artwork_rect.width(), a.bed_top_y),
            pos2(a.axis_x + 0.30 * a.artwork_rect.width(), a.bed_bottom_y),
        );
        for path in [&hot[..], &cold[..]] {
            for leg in path.windows(2) {
                let swept = Rect::from_two_pos(leg[0], leg[1]).expand(0.5 * HELIUM_PIPE_THICKNESS);
                assert!(
                    !swept.intersects(bed),
                    "duct leg {:?} -> {:?} runs over the pebble bed {bed:?}",
                    leg[0],
                    leg[1]
                );
            }
        }
        println!("hot {hot:?}\ncold {cold:?}\nbed {bed:?}");
    }

    /// **The reactor-side helium overlays stay clear of the pebble bed**
    /// (gh #154 §4: no flow graphics over the bed).
    ///
    /// The schematic draws the upward cold-helium chevrons on the reflector
    /// risers, an upper-plenum bar above the bed and a down-chevron below it.
    /// The risers must sit outside the bed's half-width, the plenum bar above
    /// the bed top, and the below-core cue below the bed's cone bottom.
    #[test]
    fn reactor_helium_overlays_stay_off_the_pebble_bed() {
        let a = reactor_flow_anchors();
        let bed_half = 0.30 * a.artwork_rect.width();

        for x in a.reflector_riser_x {
            assert!(
                (x - a.axis_x).abs() > bed_half,
                "reflector riser at x {x} is over the bed (half-width {bed_half:.1} from axis {:.1})",
                a.axis_x
            );
        }
        // The upper-plenum bar the schematic draws is at bed_top_y - 12.
        assert!(
            a.bed_top_y - 12.0 < a.bed_top_y,
            "upper-plenum bar must sit above the bed top"
        );
        // The below-core cue starts at bed_bottom_y + 3 and runs to the plenum.
        assert!(
            a.bed_bottom_y + 3.0 < a.hot_gas_plenum.center().y,
            "the below-core cue must run in clear space below the bed"
        );
        assert!(a.hot_gas_plenum.top() > a.bed_bottom_y);
        println!(
            "bed x +-{bed_half:.1} of axis {:.1}; bed y {:.1}..{:.1}; plenum {:?}; risers {:?}",
            a.axis_x, a.bed_top_y, a.bed_bottom_y, a.hot_gas_plenum, a.reflector_riser_x
        );
    }

    /// **The feedwater run stays on the turbine-hall side and never crosses the
    /// primary-helium bundle.**
    #[test]
    fn feedwater_stays_clear_of_the_primary_bundle() {
        let sg = sg_rect();
        assert!(
            FEEDWATER_HEADER_RISER_X > sg.center().x,
            "the feedwater riser should be on the turbine side of the SG"
        );
        assert!(
            FEEDWATER_HEADER_RISER_X > sg.right(),
            "the feedwater riser should be outboard of the SG shell"
        );
        assert!(
            COLD_DUCT_SG_DROP_X < FEEDWATER_HEADER_RISER_X,
            "the cold-duct drop and the feedwater riser should be on opposite sides of the SG"
        );
        assert!(
            sg_nozzles().feed_in.x > sg.center().x,
            "the feedwater nozzle should be on the turbine side"
        );
    }
}
