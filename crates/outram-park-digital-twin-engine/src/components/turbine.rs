//! Visual **steam** turbine.
//!
//! Renders a multi-stage axial turbine as rows of blades whose height grows
//! monotonically from inlet to exhaust — the annulus opens out along the flow,
//! as it does in a real machine where steam expands from short high-pressure
//! blades onto long low-pressure ones. A spinning rotor is drawn on top; the
//! angle is not an animation constant: it is `theta = omega * t`, where
//! `omega` is read from a real torque-balance model, so what you see is the
//! machine's actual shaft speed.
//!
//! ## Where the physics comes from
//!
//! [`TurbineVisualState`] is an enum (not a trait object, per the workspace's
//! mandatory design rules) because the set of state sources is closed and
//! growing:
//!
//! - [`TurbineVisualState::SteamGenerator`] wraps
//!   [`ThreePhaseElectricGeneratorTurbine`] from `tampines-steam-tables`. This
//!   is a **working** lumped model: an explicit torque balance advances rotor
//!   angular velocity, and per-phase EMF, current and total electrical power
//!   are read off it. It is the only variant that can report a real shaft
//!   speed, so it is the only one whose blades genuinely spin.
//! - [`TurbineVisualState::SteamThermo`] wraps
//!   [`tampines::components::Turbine`], which carries the inlet steam state and
//!   an adiabatic efficiency. It supplies the casing colour but **no** rotation
//!   — `Turbine::expand_to` is not implemented yet, so there is no shaft speed
//!   to draw and the rotor is rendered stationary rather than at a fabricated
//!   speed.
//!
//! Both variants are steam turbines. Gas and supercritical-CO2 turbines are
//! future work and deliberately have no variant here yet; the blade artwork
//! itself is working-fluid agnostic (axial machines look alike), so they are
//! expected to add state variants rather than a new widget.
//!
//! ## Simulation time is application-owned
//!
//! Like [`crate::animation::TracerTrain`], the rotor phase depends on elapsed
//! simulation time, and widgets are rebuilt every repaint — a clock owned by
//! the widget would reset to zero each frame and the turbine would never turn.
//! The **application** owns the clock and passes it in via
//! [`TurbineVisual::at_time`].
//!
//! ## Provenance
//!
//! The blade geometry is ported from this crate's own `fhr_sim_v2` example
//! (`app/local_widgets_and_buttons/turbine_widget.rs`), generalised so the
//! rotation angle derives from a physics model instead of being set by the
//! caller.

use crate::color_maps::steam_quality_colour_mark_1;
use crate::components::temperature_colour;
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};
use std::f64::consts::PI;
use tampines::components::Turbine;
use tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine;
use uom::si::angle::radian;
use uom::si::f64::{Angle, ThermodynamicTemperature, Time};
use uom::si::ratio::ratio;
use uom::ConstZero;

/// Number of rotor blades drawn per blade row.
const BLADES_PER_ROW: usize = 20;

/// Number of blade rows drawn from inlet to outlet.
///
/// Blade height grows **monotonically** across these rows, which is what makes
/// an axial machine read as an axial machine: steam enters at high pressure
/// against short blades and leaves at low pressure against long ones, so the
/// annulus opens out along the flow.
const BLADE_ROWS: usize = 11;

/// Hub (shaft) radius as a fraction of the widget's half-height. The first
/// blade row starts here rather than at zero, so the inlet stage has real
/// blades rather than degenerating to a point.
const HUB_RADIUS_FRACTION: f32 = 0.18;

/// Stator-bar stroke width as a fraction of the row pitch. Below 0.5 the rows
/// stay visually separated instead of merging into a solid block.
const STATOR_WIDTH_FRACTION: f32 = 0.34;

/// Rotor-blade stroke width as a fraction of the row pitch.
const ROTOR_WIDTH_FRACTION: f32 = 0.09;

/// Rotor-blade tilt as a fraction of the row pitch.
///
/// Deliberately keyed to the **pitch**, not to the blade radius: tying it to
/// radius made outer blades grow diagonal slashes that swamped the drawing at
/// wide aspect ratios.
const ROTOR_TILT_FRACTION: f32 = 0.10;

/// Number of fixed stator (nozzle) blades drawn per row, on each side of the
/// shaft. Fewer than the rotor count so the two rows stay distinguishable.
const STATOR_BLADES_PER_ROW: usize = 4;

/// How far the rotor row's blade height is advanced towards the NEXT stator
/// row, as a fraction of the gap between them.
///
/// A rotor row belongs to the same stage as the stator in front of it, and in
/// a real machine the annulus opens gradually across many stages rather than
/// stepping up between the two halves of one. Taking the radius at the rotor's
/// own axial station (0.5) makes adjacent stator and rotor blades visibly
/// different heights — worst near a double-flow admission plane, where the
/// expansion ramp turns and the half-step is at its steepest. Keeping this
/// small pairs each rotor with its stator.
const ROTOR_RADIUS_BLEND: f32 = 0.2;

/// Radial clearance between the blade tips and the casing wall, as a fraction
/// of the hub-to-tip span. Real machines run a small tip clearance; drawing it
/// keeps the blades visibly inside the shell rather than touching it.
const CASING_CLEARANCE_FRACTION: f32 = 0.12;

/// Casing wall stroke width as a fraction of the row pitch.
const CASING_WIDTH_FRACTION: f32 = 0.12;

/// Index of the single blade painted white, so rotation direction and speed
/// stay readable when all the others are identical.
const MARKER_BLADE: usize = 10;

/// How steam is admitted to, and exhausted from, the machine.
///
/// Enum dispatch per the workspace's "no trait objects" rule — the set of flow
/// paths is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurbineFlowPath {
    /// Steam is admitted at the centre and expands **outward in both
    /// directions**, exhausting at each end. The rotor is therefore mirrored
    /// about the admission plane.
    ///
    /// This is the default because it is standard practice for large PWR
    /// turbine-generators — both the HP cylinder and the LP cylinders are
    /// commonly double-flow. Two reasons drive it: a PWR steam generator
    /// delivers *saturated* steam with a small enthalpy drop per kilogram, so
    /// the mass (and hence volumetric) flow is large and needs a lot of
    /// annulus area even at the HP inlet; and splitting the flow symmetrically
    /// cancels most of the axial thrust that a single-flow rotor would dump
    /// into its thrust bearing.
    #[default]
    DoubleFlow,
    /// Steam is admitted at one end and expands monotonically to an exhaust at
    /// the other. More typical of fossil plant, where superheated steam is
    /// denser at inlet so one flow path carries the volume.
    SingleFlow,
}

/// Placeholder blade angles for one stage.
///
/// # These numbers are placeholders, not turbine design
///
/// A real multi-stage steam turbine is a *mixture* of impulse and reaction
/// stages, and the blade angles differ stage by stage. The **degree of
/// reaction** is the fraction of a stage's enthalpy drop taken across the
/// rotor: an impulse stage is near 0 (essentially all the expansion happens in
/// the fixed nozzle, and the rotor only turns the flow), while a reaction
/// stage is typically around 0.5 (expansion split between stator and rotor).
/// Classic practice puts impulse stages at the high-pressure admission end and
/// reaction stages towards the low-pressure exhaust.
///
/// [`TurbineVisual`] reproduces that *trend* so the drawing is not uniform
/// nonsense, but the specific angles below are illustrative and are **not
/// derived from any turbine design**. They must not be presented, cited, or
/// re-used as though they were. When the detailed turbine model lands
/// (workspace bead `op-dt3.18`), the angles are to be taken from it and this
/// schedule deleted — see bead `op-wqk.14.11`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageAngles {
    /// Stator (nozzle) blade angle, degrees from the axial direction.
    pub stator_deg: f32,
    /// Rotor blade angle, degrees from the axial direction.
    pub rotor_deg: f32,
    /// Degree of reaction, dimensionless: 0.0 = pure impulse, 0.5 = typical
    /// reaction stage.
    pub reaction: f32,
}

/// Placeholder angle schedule across the machine.
///
/// `stage_fraction` runs 0.0 at admission to 1.0 at exhaust. Interpolates from
/// an impulse-like stage (high nozzle turning, reaction 0) towards a reaction
/// stage (reaction 0.5), matching the HP-impulse / LP-reaction trend described
/// on [`StageAngles`]. **Placeholder values — see that type's docs.**
fn placeholder_stage_angles(stage_fraction: f32) -> StageAngles {
    let f = stage_fraction.clamp(0.0, 1.0);
    StageAngles {
        // Nozzle turning eases off as the stages become reaction stages.
        stator_deg: 68.0 - 18.0 * f,
        // Rotor blades take up more turning as reaction rises.
        rotor_deg: 32.0 + 20.0 * f,
        reaction: 0.5 * f,
    }
}

/// Number of points used to draw one curved blade. Six reads as a smooth
/// curve without flooding the tessellator — blade count is the thing that
/// multiplies here, so this stays small.
const BLADE_CURVE_POINTS: usize = 6;

/// Build the polyline for one **cambered** blade running from `root` to `tip`.
///
/// Turbine blades are curved aerofoils, not flat plates: the camber is what
/// turns the flow, and a straight blade would direct nothing. The curve is a
/// quadratic Bezier whose control point is displaced sideways from the
/// root-to-tip chord by `camber`, so the sign of `camber` sets which way the
/// blade turns the steam — stator and rotor camber in opposite senses, which
/// is exactly how a stage extracts work.
///
/// `camber` is in screen points, and is derived by the caller from the stage's
/// blade angle (illustrative placeholders — see [`StageAngles`]).
fn cambered_blade(root: Pos2, tip: Pos2, camber: f32) -> Vec<Pos2> {
    // Control point: chord midpoint, pushed along the chord normal.
    let chord = tip - root;
    let len = chord.length();
    let normal = if len > f32::EPSILON {
        egui::vec2(-chord.y / len, chord.x / len)
    } else {
        egui::vec2(0.0, 0.0)
    };
    let control = Pos2::new(
        0.5 * (root.x + tip.x) + normal.x * camber,
        0.5 * (root.y + tip.y) + normal.y * camber,
    );

    (0..BLADE_CURVE_POINTS)
        .map(|i| {
            let s = i as f32 / (BLADE_CURVE_POINTS - 1) as f32;
            let inv = 1.0 - s;
            // Quadratic Bezier B(s) = (1-s)^2 P0 + 2(1-s)s C + s^2 P1
            Pos2::new(
                inv * inv * root.x + 2.0 * inv * s * control.x + s * s * tip.x,
                inv * inv * root.y + 2.0 * inv * s * control.y + s * s * tip.y,
            )
        })
        .collect()
}

/// Opacity of the per-stage internal fill, 0-255. High enough for the tint to
/// read as a colour rather than a grey wash, low enough that the blade rows
/// stay legible on top of it.
const STAGE_FILL_ALPHA: u8 = 165;

/// Steam quality mapped to the **bluest** displayable tint.
///
/// [`steam_quality_colour_mark_1`] runs blue at `x = 0` to white at `x = 1`,
/// so a wet-steam machine living between roughly `x = 0.86` and `x = 1.0`
/// would render as barely-tinted white across its whole length. These bounds
/// normalise quality onto the map's full range so the expansion is actually
/// visible — exactly the role `min_temp`/`max_temp` already play for the
/// temperature-coloured widgets in this crate.
///
/// This is a DISPLAY range. It changes how the number is shown, never the
/// number itself.
const STAGE_QUALITY_DISPLAY_MIN: f32 = 0.80;

/// Steam quality mapped to the whitest displayable tint. See
/// [`STAGE_QUALITY_DISPLAY_MIN`].
const STAGE_QUALITY_DISPLAY_MAX: f32 = 1.0;

/// Placeholder steam quality for a stage, admission (0.0) to exhaust (1.0).
///
/// # Placeholder, not a steam calculation
///
/// One control volume is one stage, and each stage's internals are tinted by
/// its steam quality via [`steam_quality_colour_mark_1`]. Until a steam path
/// exists there is nothing to read a real quality from, so this stands in.
///
/// The *trend* is right for a PWR: the steam generators deliver saturated (not
/// superheated) steam, so the machine runs wet throughout and quality falls
/// steadily as the steam expands and gives up energy. The specific numbers are
/// illustrative and are **not** the output of any flash calculation — do not
/// cite or re-use them. They are replaced by the real per-stage quality when
/// the turbine is coupled to a steam path (workspace bead `op-dt3.18`).
/// Which way the blading leans at axial station `i`, where `i` runs from `0.0`
/// at the left-hand row to `BLADE_ROWS - 1` at the right-hand row.
///
/// Returns `+1.0` or `-1.0`, a multiplier on the blade tilt.
///
/// # Why double flow mirrors
///
/// A double-flow cylinder admits steam at the **centre** and exhausts at both
/// ends, so the axial flow runs in **opposite directions** through the two
/// halves. For one shaft rotation sense to extract work from both, the blading
/// in one half must be the geometric **mirror image** of the other — the lean
/// flips at the admission plane, exactly as blade height does.
///
/// Leaning both halves the same way draws one half being driven backwards by
/// its own steam, which is what this function exists to prevent.
///
/// Single flow has a single axial direction, so the lean is uniform and this
/// returns `+1.0` everywhere.
fn lean_sign(flow_path: TurbineFlowPath, i: f32) -> f32 {
    let across = i / (BLADE_ROWS - 1) as f32; // 0..1 left to right
    match flow_path {
        TurbineFlowPath::SingleFlow => 1.0,
        TurbineFlowPath::DoubleFlow => {
            if across < 0.5 {
                -1.0
            } else {
                1.0
            }
        }
    }
}

fn placeholder_stage_quality(stage_fraction: f32) -> f32 {
    let f = stage_fraction.clamp(0.0, 1.0);
    // Saturated vapour at admission, progressively wetter towards exhaust.
    1.0 - 0.14 * f
}

/// Where a [`TurbineVisual`] gets the physics it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule. See the module docs for why each variant exists
/// and what it can and cannot show.
#[derive(Debug, Clone, PartialEq)]
pub enum TurbineVisualState {
    /// Steam turbine coupled to a three-phase synchronous generator. Reports a
    /// real, torque-balance-derived shaft speed, so the rotor spins.
    SteamGenerator(ThreePhaseElectricGeneratorTurbine),
    /// Steam turbine known only by its thermodynamic inlet state. Colours the
    /// casing; cannot report a shaft speed, so the rotor is drawn stationary.
    SteamThermo(Turbine),
}

/// Visual representation of a steam turbine.
///
/// Placement follows the same convention as every other widget in
/// [`crate::components`]: `screen_position` is the on-screen centre and
/// `screen_vector` the box size, so the machine can be positioned absolutely
/// on a schematic canvas. Blade-row radii and the stepped silhouette are
/// derived from that box; the *rotation* is derived from physics.
pub struct TurbineVisual {
    /// The physics state this widget renders.
    pub state: TurbineVisualState,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size of the whole machine, in points.
    pub screen_vector: Vec2,
    /// How steam is admitted and exhausted. Defaults to
    /// [`TurbineFlowPath::DoubleFlow`], standard for large PWR cylinders.
    pub flow_path: TurbineFlowPath,
    /// Elapsed simulation time, owned and advanced by the application.
    ///
    /// Combined with the model's angular velocity to give the rotor phase
    /// `theta = omega * simulation_time`. See the module docs for why this is
    /// not owned by the widget.
    pub simulation_time: Time,
    /// Temperature mapped to `hotness = 0.0` (coldest displayable colour).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0` (hottest displayable colour).
    pub max_temp: ThermodynamicTemperature,
}

impl TurbineVisual {
    /// Wrap a [`ThreePhaseElectricGeneratorTurbine`] — the variant with a real
    /// shaft speed, so the rotor spins at `omega * t`.
    ///
    /// `min_temp`/`max_temp` bound the casing colour map; a generator-backed
    /// turbine has no steam temperature of its own, so the casing is drawn in
    /// neutral grey and the range is carried only so the two variants stay
    /// interchangeable in a gallery.
    pub fn new_generator(
        generator: ThreePhaseElectricGeneratorTurbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::SteamGenerator(generator),
            screen_position,
            screen_vector,
            flow_path: TurbineFlowPath::default(),
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// Wrap a [`tampines::components::Turbine`] — colour only, no rotation
    /// (see the module docs).
    ///
    /// Argument order matches the other visual components, so this is a
    /// drop-in for the former `TurbineVisual::new`.
    pub fn new_thermo(
        physics: Turbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::SteamThermo(physics),
            screen_position,
            screen_vector,
            flow_path: TurbineFlowPath::default(),
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// Set the application-owned simulation clock. Builder-style, so it chains
    /// onto either constructor.
    pub fn at_time(mut self, simulation_time: Time) -> Self {
        self.simulation_time = simulation_time;
        self
    }

    /// Override the flow path. Builder-style; the default is
    /// [`TurbineFlowPath::DoubleFlow`].
    pub fn with_flow_path(mut self, flow_path: TurbineFlowPath) -> Self {
        self.flow_path = flow_path;
        self
    }

    /// Placeholder steam quality at a given stage, admission (0.0) to exhaust
    /// (1.0).
    ///
    /// **Illustrative only** — see [`placeholder_stage_quality`] for why this
    /// is not a steam calculation and what replaces it.
    pub fn stage_quality(&self, stage_fraction: f32) -> f32 {
        placeholder_stage_quality(stage_fraction)
    }

    /// Placeholder blade angles at a given stage, admission (0.0) to exhaust
    /// (1.0).
    ///
    /// **Illustrative only** — see [`StageAngles`] for why these are not
    /// turbine design data and what replaces them.
    pub fn stage_angles(&self, stage_fraction: f32) -> StageAngles {
        placeholder_stage_angles(stage_fraction)
    }

    /// Current rotor phase angle, `theta = omega * t`.
    ///
    /// Returns zero for [`TurbineVisualState::SteamThermo`], which has no
    /// shaft-speed model — a stationary rotor is the honest rendering of "not
    /// known", and is visibly distinct from a turning one.
    pub fn rotor_angle(&self) -> Angle {
        match &self.state {
            TurbineVisualState::SteamGenerator(g) => (g.get_omega() * self.simulation_time).into(),
            TurbineVisualState::SteamThermo(_) => Angle::ZERO,
        }
    }

    /// Casing colour source: the inlet steam temperature, when the variant
    /// knows one.
    ///
    /// [`TurbineVisualState::SteamGenerator`] is an electromechanical model
    /// with no steam path, so it returns `None` and the casing renders neutral
    /// grey rather than a fabricated temperature colour.
    pub fn casing_temperature(&self) -> Option<ThermodynamicTemperature> {
        match &self.state {
            TurbineVisualState::SteamGenerator(_) => None,
            TurbineVisualState::SteamThermo(t) => Some(t.inlet.get_temperature()),
        }
    }
}

impl Widget for TurbineVisual {
    /// Draws `2 * BLADE_ROWS_PER_SIDE + 1` stator rows of growing radius, then
    /// the rotor blades on top at the current [`TurbineVisual::rotor_angle`].
    ///
    /// Only blades on the far half of their circular path are painted (those
    /// with `sin(theta + phase) > 0`), which is what reads as a rotor turning
    /// behind the stator rather than a flat ring of ticks. Blades are angled
    /// downstream-consistent: downwards on the outlet side, upwards on the
    /// inlet side.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();
        let centre = rect.center();

        // Axial pitch between blade rows, and the annulus that opens out along
        // the flow. Stroke widths are derived from the PITCH, never from the
        // widget width directly, so the drawing survives any aspect ratio.
        let pitch = rect.width() / BLADE_ROWS as f32;
        let tip_radius = 0.5 * rect.height();
        let hub_radius = tip_radius * HUB_RADIUS_FRACTION;

        let casing_colour = match self.casing_temperature() {
            Some(t) => temperature_colour(t, self.min_temp, self.max_temp),
            None => Color32::GRAY,
        };
        let stator_stroke = Stroke::new(pitch * STATOR_WIDTH_FRACTION, casing_colour);
        let stator_blade_stroke =
            Stroke::new(pitch * ROTOR_WIDTH_FRACTION * 0.9, Color32::from_gray(120));
        let rotor_stroke = Stroke::new(pitch * ROTOR_WIDTH_FRACTION, Color32::from_gray(35));
        let marker_stroke = Stroke::new(pitch * ROTOR_WIDTH_FRACTION, Color32::WHITE);
        let tilt = pitch * ROTOR_TILT_FRACTION;

        let theta = self.rotor_angle();

        // How far through the expansion each row sits: 0.0 at admission,
        // 1.0 at exhaust. For a double-flow machine steam enters at the CENTRE
        // and expands outward to an exhaust at each end, so the fraction is
        // measured from the mid-plane and the drawing is mirrored about it.
        // For single flow it runs straight across, left to right.
        let stage_fraction_at = |i: f32| -> f32 {
            let across = i / (BLADE_ROWS - 1) as f32; // 0..1 left to right
            match self.flow_path {
                TurbineFlowPath::SingleFlow => across,
                TurbineFlowPath::DoubleFlow => (2.0 * across - 1.0).abs(),
            }
        };
        // Which way the blading leans at station `i` — see `lean_sign`.
        let lean_sign_at = |i: f32| -> f32 { lean_sign(self.flow_path, i) };
        // Blade height grows with the expansion, so the annulus opens out
        // towards each exhaust.
        let radius_at =
            |i: f32| -> f32 { hub_radius + (tip_radius - hub_radius) * stage_fraction_at(i) };
        // Row `i` sits at the centre of its pitch slot.
        let x_at = |i: f32| -> f32 { rect.left() + (i + 0.5) * pitch };

        // ── Per-stage internals ───────────────────────────────────────────
        // One control volume is one stage, so each axial slab between
        // consecutive stator rows is tinted by that stage's steam quality.
        // Drawn FIRST, so the blade rows and the casing read on top of it.
        // The quality is a placeholder today — see `placeholder_stage_quality`.
        let fill_clearance = (tip_radius - hub_radius) * CASING_CLEARANCE_FRACTION;
        for stage in 0..BLADE_ROWS - 1 {
            let (i0, i1) = (stage as f32, (stage + 1) as f32);
            let (r0, r1) = (
                radius_at(i0) + fill_clearance,
                radius_at(i1) + fill_clearance,
            );
            let (x0, x1) = (x_at(i0), x_at(i1));
            // Quality at the middle of this stage.
            let quality = placeholder_stage_quality(stage_fraction_at(i0 + 0.5));
            // Normalised onto the colour map's full range so the wet end reads
            // blue instead of near-white (see STAGE_QUALITY_DISPLAY_MIN).
            let shade = ((quality - STAGE_QUALITY_DISPLAY_MIN)
                / (STAGE_QUALITY_DISPLAY_MAX - STAGE_QUALITY_DISPLAY_MIN))
                .clamp(0.0, 1.0);
            let c = steam_quality_colour_mark_1(shade);
            let fill = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), STAGE_FILL_ALPHA);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(x0, centre.y - r0),
                    Pos2::new(x1, centre.y - r1),
                    Pos2::new(x1, centre.y + r1),
                    Pos2::new(x0, centre.y + r0),
                ],
                fill,
                Stroke::NONE,
            ));
        }

        // The shaft, drawn BEFORE the blade rows. The rotor blades are mounted
        // on it and stand in front of it in this side view, so they must
        // occlude the shaft — drawing the shaft last hides the blade roots and
        // makes the rows look like they float over the rotor.
        painter.line_segment(
            [
                Pos2::new(rect.left(), centre.y),
                Pos2::new(rect.right(), centre.y),
            ],
            Stroke::new(hub_radius, casing_colour),
        );

        // A stage is a FIXED stator row followed by a MOVING rotor row, and the
        // two sit at different axial stations — drawing them at the same station
        // superimposes the moving blades on the fixed ones, which is not what a
        // turbine looks like. Stator rows sit on integer stations; rotor rows sit
        // half a pitch downstream, between them.
        for row in 0..BLADE_ROWS {
            let row_f = row as f32;
            let radius = radius_at(row_f);
            let x = x_at(row_f);
            let angles = placeholder_stage_angles(stage_fraction_at(row_f));

            // Fixed STATOR (nozzle) row: a vertical bar spanning the annulus,
            // plus its own fixed blades. The stator is what turns the flow onto
            // the following rotor row; drawing it is what makes the stage
            // structure legible rather than showing a bare spinning rotor.
            painter.line_segment(
                [
                    Pos2::new(x, centre.y - radius),
                    Pos2::new(x, centre.y + radius),
                ],
                stator_stroke,
            );

            // Stator (nozzle) blades. Like the rotor blades below, each is
            // ROOTED AT THE HUB and runs radially out to the tip; seen from the
            // side both ends project by the same cos(phi), so a blade shortens
            // onto the shaft as it turns edge-on instead of floating free in
            // the annulus. These are fixed: no `theta` term, so they do not
            // move. Tilted OPPOSITE to the rotor blades, because the stator
            // turns the flow one way and the rotor takes it back the other.
            // Blade LEAN mirrors about the admission plane on a double-flow
            // machine, exactly as blade HEIGHT does — see `lean_sign_at` for
            // why, and the double-flow symmetry tests for the pinned contract.
            // Negative: the nozzle angles DOWN across the row (verified on
            // screen, 2026-08-04). The rotor carries the same sign, so the two
            // rows lean the same way rather than opposing; the opposition that
            // turns the flow is carried by the CAMBER, which still runs in
            // opposite senses for the two rows.
            let stator_tilt = -tilt * (angles.stator_deg / 60.0) * lean_sign_at(row_f);
            for k in 0..STATOR_BLADES_PER_ROW * 2 {
                let phi = Angle::new::<radian>(
                    k as f64 * (2.0 * PI) / ((STATOR_BLADES_PER_ROW * 2) as f64),
                );
                // The whole ring is drawn, unlike the rotor. Hiding the far
                // half is what makes the ROTOR read as turning; a fixed row has
                // no rotation to suggest, and hiding half of it just makes the
                // nozzle ring look half-built.
                let c = phi.cos().get::<ratio>() as f32;
                // Camber turns the flow onto the following rotor row. Sign is
                // opposite to the rotor's, and magnitude grows with the
                // placeholder nozzle angle.
                let camber = stator_tilt * 1.6;
                painter.add(egui::Shape::line(
                    cambered_blade(
                        Pos2::new(x - stator_tilt, centre.y + hub_radius * c),
                        Pos2::new(x + stator_tilt, centre.y + radius * c),
                        camber,
                    ),
                    stator_blade_stroke,
                ));
            }

            // The rotor row for this stage, half a pitch downstream. The last
            // stator row has no rotor after it (it would fall outside the
            // machine), so stop there.
            if row + 1 >= BLADE_ROWS {
                continue;
            }
            // The rotor sits half a pitch downstream AXIALLY, but its blade
            // height stays close to its own stator's (see ROTOR_RADIUS_BLEND).
            //
            // The blend must run from the UPSTREAM stator, which is not simply
            // the row at smaller x: on a double-flow machine steam expands
            // OUTWARD from the centre, so downstream is +x on one half and -x
            // on the other. Blending toward `row + 1` on both halves resolves
            // mirrored stations to different radii and visibly breaks the
            // symmetry. The upstream neighbour is whichever of the two adjacent
            // rows is LESS expanded — nearer the admission plane — which is
            // correct for single flow too.
            let rotor_x = x_at(row_f + 0.5);
            let f_here = stage_fraction_at(row_f);
            let f_next = stage_fraction_at(row_f + 1.0);
            let f_upstream = f_here.min(f_next);
            let f_station = stage_fraction_at(row_f + 0.5);
            let rotor_fraction = f_upstream + ROTOR_RADIUS_BLEND * (f_station - f_upstream);
            let rotor_radius = hub_radius + (tip_radius - hub_radius) * rotor_fraction;
            let rotor_angles = placeholder_stage_angles(rotor_fraction);

            for blade in 0..BLADES_PER_ROW {
                let phase = theta
                    + Angle::new::<radian>(blade as f64 * (2.0 * PI) / (BLADES_PER_ROW as f64));

                // Only the half-revolution facing the viewer is drawn — this
                // is what reads as a rotor turning behind the stator rather
                // than a static ring of ticks.
                if phase.sin() <= <uom::si::f64::Ratio as ConstZero>::ZERO {
                    continue;
                }

                // A rotor blade is ROOTED AT THE HUB and runs radially out to
                // the tip. Seen from the side, root and tip project by the same
                // cos(phi), so the blade shortens onto the shaft as it turns
                // edge-on. Drawing it as a free-floating tick at mid-span makes
                // the blades look like they hang in the air, unattached.
                let c = phase.cos().get::<ratio>() as f32;
                let root_y = centre.y + hub_radius * c;
                let tip_y = centre.y + rotor_radius * c;

                // Tilt sets the blade angle: the tip leads the root, tilted
                // opposite to the stator blades. Follows this stage's
                // placeholder rotor angle, and mirrors about the admission
                // plane on a double-flow machine (see `lean_sign_at`) — the
                // rotor sits half a pitch downstream, so its lean is evaluated
                // at `row_f + 0.5`, the same station its radius uses.
                // Note this leans the SAME way as the stator rather than
                // against it — see the comment on `stator_tilt`.
                let rotor_tilt =
                    -tilt * (rotor_angles.rotor_deg / 40.0) * lean_sign_at(row_f + 0.5);
                let stroke = if blade == MARKER_BLADE {
                    marker_stroke
                } else {
                    rotor_stroke
                };
                // Cambered the OPPOSITE way to the stator blades: the stator
                // turns the steam one way, the rotor turns it back, and that
                // change of momentum is the work the stage extracts. Impulse
                // stages (low reaction, at admission) are the more strongly
                // curved buckets, so camber eases off as reaction rises.
                let camber = -rotor_tilt * (1.8 - rotor_angles.reaction);
                painter.add(egui::Shape::line(
                    cambered_blade(
                        Pos2::new(rotor_x + rotor_tilt, root_y),
                        Pos2::new(rotor_x - rotor_tilt, tip_y),
                        camber,
                    ),
                    stroke,
                ));
            }
        }

        // ── Casing ────────────────────────────────────────────────────────
        // The pressure shell enclosing the machine, drawn AFTER the blade rows
        // so it reads as the outer boundary containing them. It follows the
        // annulus envelope with a clearance margin, so it opens out with the
        // expansion exactly as the blades do — for a double-flow machine that
        // means it flares towards an exhaust at each end.
        let clearance = (tip_radius - hub_radius) * CASING_CLEARANCE_FRACTION;
        let casing_stroke = Stroke::new(pitch * CASING_WIDTH_FRACTION, casing_colour);
        let envelope: Vec<f32> = (0..BLADE_ROWS)
            .map(|i| radius_at(i as f32) + clearance)
            .collect();

        for sign in [-1.0_f32, 1.0] {
            let wall: Vec<Pos2> = envelope
                .iter()
                .enumerate()
                .map(|(i, r)| Pos2::new(x_at(i as f32), centre.y + sign * r))
                .collect();
            painter.add(egui::Shape::line(wall, casing_stroke));
        }

        // End walls, closing the shell at each end of the machine.
        for (i, x_end) in [
            (0usize, x_at(0.0)),
            (BLADE_ROWS - 1, x_at((BLADE_ROWS - 1) as f32)),
        ] {
            let r = envelope[i];
            painter.line_segment(
                [
                    Pos2::new(x_end, centre.y - r),
                    Pos2::new(x_end, centre.y + r),
                ],
                casing_stroke,
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angular_velocity::radian_per_second;
    use uom::si::f64::AngularVelocity;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    fn range() -> (ThermodynamicTemperature, ThermodynamicTemperature) {
        (
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )
    }

    fn generator_visual(omega_rad_s: f64) -> TurbineVisual {
        let mut g = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        g.set_omega(AngularVelocity::new::<radian_per_second>(omega_rad_s));
        let (min, max) = range();
        TurbineVisual::new_generator(g, Pos2::ZERO, Vec2::new(200.0, 100.0), min, max)
    }

    /// The rotor phase must be the physical product `theta = omega * t`, not a
    /// frame counter — this is the whole point of coupling the widget to the
    /// generator rather than animating it.
    ///
    /// **Methodology:** set the generator's angular velocity to a known
    /// 10.0 rad/s, advance the application clock to 3.0 s, and compare the
    /// reported rotor angle against the analytical 30.0 rad.
    ///
    /// **Result (2026-08-04):** 30.0 rad exactly, to within 1e-9 rad. The
    /// identity is exact in `uom`'s type algebra, so the only error is
    /// floating-point round-off.
    #[test]
    fn rotor_angle_is_omega_times_time() {
        let v = generator_visual(10.0).at_time(Time::new::<second>(3.0));
        assert!((v.rotor_angle().get::<radian>() - 30.0).abs() < 1e-9);
    }

    /// A stationary shaft must render stationary, at exactly zero phase.
    #[test]
    fn zero_angular_velocity_gives_zero_rotor_angle() {
        let v = generator_visual(0.0).at_time(Time::new::<second>(12.5));
        assert_eq!(v.rotor_angle().get::<radian>(), 0.0);
    }

    /// The generator variant has no steam path, so it must report no casing
    /// temperature rather than inventing one. A fabricated colour would make an
    /// electromechanical model look like it knew a steam state.
    #[test]
    fn generator_variant_reports_no_casing_temperature() {
        assert!(generator_visual(5.0).casing_temperature().is_none());
    }

    /// Double flow is the default, because both the HP and the LP cylinders of
    /// a large PWR turbine-generator are commonly double-flow. A widget built
    /// without saying otherwise must not silently draw a single-flow machine.
    #[test]
    fn default_flow_path_is_double_flow() {
        assert_eq!(TurbineFlowPath::default(), TurbineFlowPath::DoubleFlow);
        assert_eq!(generator_visual(1.0).flow_path, TurbineFlowPath::DoubleFlow);
    }

    /// The flow path must be overridable for the single-flow (fossil-style)
    /// case without disturbing anything else.
    #[test]
    fn flow_path_can_be_overridden_to_single_flow() {
        let v = generator_visual(1.0).with_flow_path(TurbineFlowPath::SingleFlow);
        assert_eq!(v.flow_path, TurbineFlowPath::SingleFlow);
    }

    /// The placeholder schedule must reproduce the impulse-to-reaction trend of
    /// a real multi-stage machine: admission end near pure impulse, exhaust end
    /// towards a reaction stage, with nozzle turning easing off and rotor
    /// turning taking over as reaction rises.
    ///
    /// **Methodology:** evaluate the schedule at the admission end
    /// (`stage_fraction = 0.0`) and the exhaust end (`1.0`) and assert the
    /// direction of each trend, plus the reaction end points.
    ///
    /// **Result (2026-08-04):** admission gives reaction 0.0, stator 68.0 deg,
    /// rotor 32.0 deg; exhaust gives reaction 0.5, stator 50.0 deg, rotor
    /// 52.0 deg. These are ILLUSTRATIVE placeholder values, not turbine design
    /// data — the test pins the trend and the labelled end points so a future
    /// edit cannot quietly invert the physics, not the numbers' correctness.
    #[test]
    fn placeholder_angles_trend_from_impulse_to_reaction() {
        let inlet = placeholder_stage_angles(0.0);
        let exhaust = placeholder_stage_angles(1.0);

        assert_eq!(inlet.reaction, 0.0, "admission end should be impulse-like");
        assert_eq!(
            exhaust.reaction, 0.5,
            "exhaust end should be a reaction stage"
        );
        assert!(
            exhaust.stator_deg < inlet.stator_deg,
            "nozzle turning should ease off as reaction rises"
        );
        assert!(
            exhaust.rotor_deg > inlet.rotor_deg,
            "rotor should take up more turning as reaction rises"
        );
    }

    /// The schedule is only defined on [0, 1]; out-of-range stage fractions
    /// must clamp rather than extrapolate into nonsense angles.
    #[test]
    fn placeholder_angles_clamp_outside_the_machine() {
        assert_eq!(
            placeholder_stage_angles(-1.0),
            placeholder_stage_angles(0.0)
        );
        assert_eq!(placeholder_stage_angles(2.0), placeholder_stage_angles(1.0));
    }

    /// A double-flow machine admits steam at the centre and expands outward to
    /// an exhaust at each end, so its geometry must be mirror-symmetric about
    /// the admission plane. This pins that symmetry.
    ///
    /// **Methodology:** the drawing derives every blade height and stage angle
    /// from `stage_fraction`, so symmetry of the drawing reduces to symmetry of
    /// that function. Evaluate it at mirrored stations across the machine —
    /// both stator stations (integers) and rotor stations (half-integers) — and
    /// require exact equality.
    ///
    /// **Result (2026-08-04):** all mirrored pairs agree to within 1e-6.
    /// Regression guard: the rotor radius previously blended toward `row + 1`
    /// on both halves, which is downstream on one side and UPSTREAM on the
    /// other, and broke this symmetry.
    #[test]
    fn double_flow_geometry_is_symmetric_about_the_admission_plane() {
        let span = (BLADE_ROWS - 1) as f32;
        let fraction = |i: f32| -> f32 {
            let across = i / span;
            (2.0 * across - 1.0).abs()
        };
        // Stator stations and the rotor stations between them.
        for step in 0..=(2 * (BLADE_ROWS - 1)) {
            let station = step as f32 * 0.5;
            let mirrored = span - station;
            assert!(
                (fraction(station) - fraction(mirrored)).abs() < 1e-6,
                "station {station} and its mirror {mirrored} disagree: {} vs {}",
                fraction(station),
                fraction(mirrored)
            );
        }
    }

    /// Single flow is the opposite case and must NOT be symmetric — steam
    /// enters one end and leaves the other, so the machine expands one way.
    #[test]
    fn single_flow_geometry_is_not_symmetric() {
        let span = (BLADE_ROWS - 1) as f32;
        let fraction = |i: f32| -> f32 { i / span };
        assert!(
            (fraction(0.0) - fraction(span)).abs() > 0.5,
            "single flow should expand monotonically, not mirror"
        );
    }

    /// A fresh 250 MW preset starts from rest, so its rotor must not turn until
    /// the model is actually advanced.
    #[test]
    fn fresh_preset_starts_at_rest() {
        let (min, max) = range();
        let g = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        let v = TurbineVisual::new_generator(g, Pos2::ZERO, Vec2::new(200.0, 100.0), min, max)
            .at_time(Time::new::<second>(60.0));
        assert_eq!(v.rotor_angle().get::<radian>(), 0.0);
    }
}

#[cfg(test)]
mod double_flow_lean_tests {
    use super::*;

    /// A double-flow cylinder's two halves must be geometric MIRROR IMAGES,
    /// blade lean included — not just blade height.
    ///
    /// **Methodology.** Steam is admitted at the centre and exhausts at both
    /// ends, so the axial flow runs in opposite directions through the two
    /// halves; one shaft rotation sense can only extract work from both if the
    /// blading mirrors. Evaluate [`lean_sign`] at mirrored stations across the
    /// machine — `i` against `(BLADE_ROWS - 1) - i` — and require the two to
    /// carry OPPOSITE signs, and each to be exactly unit magnitude.
    ///
    /// **Result (2026-08-06):** every mirrored pair is opposite-signed, and no
    /// station returns anything but +/-1. Interpretation: the two halves lean
    /// away from the admission plane, so neither half is drawn being driven
    /// backwards by its own steam. Before this, lean was uniform across the
    /// machine and the halves read as translated rather than mirrored.
    #[test]
    fn double_flow_blade_lean_mirrors_about_the_admission_plane() {
        let span = (BLADE_ROWS - 1) as f32;
        for step in 0..=(BLADE_ROWS * 2) {
            let i = span * (step as f32) / ((BLADE_ROWS * 2) as f32);
            let mirrored = span - i;
            let a = lean_sign(TurbineFlowPath::DoubleFlow, i);
            let b = lean_sign(TurbineFlowPath::DoubleFlow, mirrored);

            assert!(
                a.abs() == 1.0 && b.abs() == 1.0,
                "lean sign must be unit magnitude, got {a} and {b}"
            );
            // Stations exactly on the admission plane are their own mirror;
            // everything else must oppose.
            if (i - mirrored).abs() > 1e-6 {
                assert_eq!(
                    a, -b,
                    "station {i} and its mirror {mirrored} must lean oppositely, got {a} vs {b}"
                );
            }
        }
    }

    /// Single flow has one axial direction, so the lean must NOT flip — a
    /// mirrored single-flow machine would be wrong in the other direction.
    #[test]
    fn single_flow_blade_lean_is_uniform() {
        let span = (BLADE_ROWS - 1) as f32;
        for step in 0..=BLADE_ROWS {
            let i = span * (step as f32) / (BLADE_ROWS as f32);
            assert_eq!(
                lean_sign(TurbineFlowPath::SingleFlow, i),
                1.0,
                "single flow must not mirror, but station {i} flipped"
            );
        }
    }

    /// The two halves must be the same SIZE — if the split were off-centre one
    /// half would carry more rows than the other and the mirror would be
    /// visibly lopsided.
    #[test]
    fn double_flow_halves_are_balanced() {
        let span = (BLADE_ROWS - 1) as f32;
        let (mut left, mut right) = (0, 0);
        for row in 0..BLADE_ROWS {
            let i = row as f32;
            // Skip a row sitting exactly on the plane; it belongs to neither.
            if (i - (span - i)).abs() < 1e-6 {
                continue;
            }
            if lean_sign(TurbineFlowPath::DoubleFlow, i) < 0.0 {
                left += 1;
            } else {
                right += 1;
            }
        }
        assert_eq!(
            left, right,
            "double-flow halves are lopsided: {left} vs {right}"
        );
    }
}
